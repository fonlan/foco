//! In-process HTML preview sessions and static origin serving.
//!
//! Capability tokens live only in process memory. Preview pages are served from
//! either:
//! - host mode: `<token>.preview.localhost` (local loopback UI) so scripts cannot
//!   share Foco cookies/localStorage with the host UI; or
//! - path mode: `/__preview/<token>/...` on the same public origin (reverse proxy),
//!   authorized only by the path token and framed with a tighter iframe sandbox
//!   (`allow-scripts` without `allow-same-origin`).

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json,
    body::Body,
    extract::{Path as AxumPath, Request, State},
    http::{HeaderMap, Method, StatusCode, header},
    response::Response,
};
use serde::{Deserialize, Serialize};

use crate::{
    ApiError, AppState, config_snapshot, normalize_workspace_relative_path, workspace_by_id,
};

/// Idle sessions expire after this duration without access.
pub(crate) const PREVIEW_SESSION_IDLE_TTL: Duration = Duration::from_secs(30 * 60);
/// Hard cap on concurrent preview sessions in one process.
pub(crate) const PREVIEW_SESSION_MAX_COUNT: usize = 64;
/// Timeout for main-process → sidecar preview file HEAD/GET.
const PREVIEW_SIDECAR_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
/// DNS label length for the token subdomain (a–z0–9 only).
const PREVIEW_TOKEN_LEN: usize = 32;
const PREVIEW_HOST_SUFFIX: &str = ".preview.localhost";
/// Path-mode prefix on the Foco origin when `*.preview.localhost` is unreachable
/// (reverse proxies / non-loopback UI hosts).
pub(crate) const PREVIEW_PATH_PREFIX: &str = "/__preview/";

#[derive(Clone, Default)]
pub(crate) struct PreviewSessionRegistry {
    inner: Arc<Mutex<HashMap<String, PreviewSession>>>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreviewSession {
    pub(crate) token: String,
    pub(crate) workspace_id: String,
    /// Workspace-relative entry HTML path (posix separators).
    pub(crate) entry_path: String,
    /// Workspace-relative preview root (parent of entry; empty string = workspace root).
    pub(crate) root_path: String,
    /// Wall-clock session birth; kept for idle diagnostics and future API fields.
    #[allow(dead_code)]
    pub(crate) created_at: Instant,
    pub(crate) last_accessed_at: Instant,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreatePreviewSessionRequest {
    pub(crate) path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviewSessionResponse {
    pub(crate) token: String,
    pub(crate) workspace_id: String,
    pub(crate) entry_path: String,
    pub(crate) root_path: String,
    pub(crate) preview_url: String,
    pub(crate) preview_origin: String,
    /// Recommended iframe sandbox attribute for the Foco host page.
    pub(crate) iframe_sandbox: String,
}

impl PreviewSessionRegistry {
    pub(crate) fn create(
        &self,
        workspace_id: String,
        entry_path: String,
        root_path: String,
    ) -> Result<PreviewSession, ApiError> {
        let mut sessions = self
            .inner
            .lock()
            .map_err(|_| ApiError::internal("preview session registry lock is poisoned"))?;
        expire_idle_sessions_locked(&mut sessions);

        if sessions.len() >= PREVIEW_SESSION_MAX_COUNT {
            return Err(ApiError::bad_request(format!(
                "preview session limit reached ({PREVIEW_SESSION_MAX_COUNT}); close idle previews and retry"
            )));
        }

        let now = Instant::now();
        let token = generate_preview_token(&sessions)?;
        let session = PreviewSession {
            token: token.clone(),
            workspace_id,
            entry_path,
            root_path,
            created_at: now,
            last_accessed_at: now,
        };
        sessions.insert(token, session.clone());
        Ok(session)
    }

    pub(crate) fn release(&self, token: &str) -> Result<bool, ApiError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(ApiError::bad_request("preview token must not be empty"));
        }
        if !is_dns_safe_preview_token(token) {
            return Err(ApiError::bad_request("invalid preview token"));
        }

        let mut sessions = self
            .inner
            .lock()
            .map_err(|_| ApiError::internal("preview session registry lock is poisoned"))?;
        expire_idle_sessions_locked(&mut sessions);
        Ok(sessions.remove(token).is_some())
    }

    /// Touch last-access and return a clone of the active session.
    pub(crate) fn touch(&self, token: &str) -> Result<PreviewSession, ApiError> {
        let token = token.trim();
        if !is_dns_safe_preview_token(token) {
            return Err(ApiError::from_status_message(
                StatusCode::NOT_FOUND,
                "preview session not found",
            ));
        }

        let mut sessions = self
            .inner
            .lock()
            .map_err(|_| ApiError::internal("preview session registry lock is poisoned"))?;
        expire_idle_sessions_locked(&mut sessions);

        let session = sessions.get_mut(token).ok_or_else(|| {
            ApiError::from_status_message(StatusCode::NOT_FOUND, "preview session not found")
        })?;
        session.last_accessed_at = Instant::now();
        Ok(session.clone())
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.inner
            .lock()
            .map(|sessions| sessions.len())
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn force_expire_token_for_test(&self, token: &str) {
        let mut sessions = self.inner.lock().expect("preview registry lock");
        if let Some(session) = sessions.get_mut(token) {
            session.last_accessed_at = Instant::now()
                .checked_sub(PREVIEW_SESSION_IDLE_TTL + Duration::from_secs(1))
                .unwrap_or_else(Instant::now);
        }
        expire_idle_sessions_locked(&mut sessions);
    }
}

fn expire_idle_sessions_locked(sessions: &mut HashMap<String, PreviewSession>) {
    let now = Instant::now();
    sessions.retain(|_, session| {
        now.duration_since(session.last_accessed_at) < PREVIEW_SESSION_IDLE_TTL
    });
}

fn generate_preview_token(existing: &HashMap<String, PreviewSession>) -> Result<String, ApiError> {
    // 32 lowercase alnum labels are DNS-safe and hard to guess (~160 bits).
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    for _ in 0..32 {
        let mut bytes = [0u8; PREVIEW_TOKEN_LEN];
        getrandom::fill(&mut bytes).map_err(|source| {
            ApiError::internal(format!(
                "failed to generate preview session token: {source}"
            ))
        })?;
        let token: String = bytes
            .iter()
            .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
            .collect();
        if !existing.contains_key(&token) {
            return Ok(token);
        }
    }
    Err(ApiError::internal(
        "failed to allocate a unique preview session token",
    ))
}

pub(crate) fn is_dns_safe_preview_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 63
        && token
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9'))
}

/// Parse `Host` for `<token>.preview.localhost` (optional port). Returns lowercase token.
pub(crate) fn parse_preview_host_token(host: &str) -> Option<String> {
    let host = host.trim();
    let host = host.split(':').next().unwrap_or(host).trim();
    let host = host.strip_suffix('.').unwrap_or(host);
    let lower = host.to_ascii_lowercase();
    if lower == "preview.localhost" {
        return None;
    }
    if !lower.ends_with(PREVIEW_HOST_SUFFIX) {
        return None;
    }
    let token = &lower[..lower.len() - PREVIEW_HOST_SUFFIX.len()];
    if !is_dns_safe_preview_token(token) {
        return None;
    }
    // Reject extra subdomain nesting: only one label before .preview.localhost
    if token.contains('.') {
        return None;
    }
    Some(token.to_string())
}

pub(crate) fn redact_preview_host_for_log(host: &str) -> String {
    match parse_preview_host_token(host) {
        Some(token) => {
            let redacted = if token.len() <= 4 {
                "****".to_string()
            } else {
                format!("{}…{}", &token[..2], &token[token.len() - 2..])
            };
            format!("{redacted}.preview.localhost")
        }
        None => host.to_string(),
    }
}

pub(crate) fn preview_origin(token: &str, port: u16) -> String {
    format!("http://{token}.preview.localhost:{port}")
}

/// Entry path as served under the preview origin (relative to preview root).
pub(crate) fn entry_path_within_preview_root(entry_path: &str, root_path: &str) -> String {
    let entry_path = entry_path.trim_matches('/');
    if root_path.is_empty() {
        return entry_path.to_string();
    }
    let prefix = format!("{root_path}/");
    entry_path
        .strip_prefix(&prefix)
        .unwrap_or(entry_path)
        .to_string()
}

pub(crate) fn preview_entry_url(
    token: &str,
    port: u16,
    entry_path: &str,
    root_path: &str,
) -> String {
    let entry = entry_path_within_preview_root(entry_path, root_path);
    format!(
        "{}/{}",
        preview_origin(token, port),
        entry.trim_start_matches('/')
    )
}

/// Path-mode resource base: `{public_origin}/__preview/{token}`.
pub(crate) fn preview_path_origin(public_origin: &str, token: &str) -> String {
    format!(
        "{}/__preview/{}",
        public_origin.trim_end_matches('/'),
        token
    )
}

pub(crate) fn preview_path_entry_url(
    public_origin: &str,
    token: &str,
    entry_path: &str,
    root_path: &str,
) -> String {
    let entry = entry_path_within_preview_root(entry_path, root_path);
    format!(
        "{}/{}",
        preview_path_origin(public_origin, token),
        entry.trim_start_matches('/')
    )
}

/// Parse `/__preview/{token}` or `/__preview/{token}/...` into (token, resource_path).
/// `resource_path` always starts with `/` ("/" means entry HTML).
pub(crate) fn parse_preview_path(path: &str) -> Option<(String, String)> {
    let rest = path.strip_prefix(PREVIEW_PATH_PREFIX)?;
    if rest.is_empty() {
        return None;
    }
    let (token, resource) = match rest.find('/') {
        Some(idx) => {
            let token = &rest[..idx];
            let resource = &rest[idx..];
            (token, if resource.is_empty() { "/" } else { resource })
        }
        None => (rest, "/"),
    };
    if !is_dns_safe_preview_token(token) {
        return None;
    }
    Some((token.to_string(), resource.to_string()))
}

/// Hostname (no port) is loopback / local-only so `*.preview.localhost` is usable.
pub(crate) fn preview_request_uses_host_origin(host_header: &str) -> bool {
    let host = host_header.trim();
    if host.is_empty() {
        return true;
    }
    // Strip optional port; handle bracketed IPv6 Host values.
    let host = if let Some(inner) = host.strip_prefix('[').and_then(|h| h.split(']').next()) {
        inner
    } else {
        host.split(':').next().unwrap_or(host)
    };
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "0.0.0.0"
}

/// Public origin for path-mode URLs and CSP frame-ancestors (respects X-Forwarded-Proto).
pub(crate) fn request_public_origin(
    headers: &HeaderMap,
    listen_addr: std::net::SocketAddr,
) -> String {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| {
            let host = match listen_addr.ip() {
                std::net::IpAddr::V4(ip) if ip.is_unspecified() || ip.is_loopback() => {
                    "127.0.0.1".to_string()
                }
                std::net::IpAddr::V6(ip) if ip.is_unspecified() || ip.is_loopback() => {
                    "127.0.0.1".to_string()
                }
                other => other.to_string(),
            };
            format!("{host}:{}", listen_addr.port())
        });

    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| value.eq_ignore_ascii_case("http") || value.eq_ignore_ascii_case("https"))
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "http".to_string());

    format!("{proto}://{host}")
}

/// Host-mode iframe sandbox: scripts + same-origin within the preview origin only.
/// Does not include top-navigation, popups, downloads, forms, or pointer-lock.
pub(crate) const PREVIEW_IFRAME_SANDBOX_HOST: &str = "allow-scripts allow-same-origin";
/// Path-mode sandbox: scripts only. Omit allow-same-origin so the framed document
/// gets an opaque origin and cannot read Foco cookies/localStorage/DOM even though
/// the URL is on the Foco host.
pub(crate) const PREVIEW_IFRAME_SANDBOX_PATH: &str = "allow-scripts";
/// Backward-compatible alias for host-mode sandbox (tests + older call sites).
pub(crate) const PREVIEW_IFRAME_SANDBOX: &str = PREVIEW_IFRAME_SANDBOX_HOST;

fn is_html_entry_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".html") || lower.ends_with(".htm")
}

fn preview_root_for_entry(entry_path: &str) -> String {
    let entry_path = entry_path.trim_matches('/');
    match entry_path.rsplit_once('/') {
        Some((parent, _)) => parent.to_string(),
        None => String::new(),
    }
}

/// Resolve a request path under the preview root.
/// Request paths are absolute URL paths under the preview site; they map onto
/// `preview_root` inside the workspace (not the workspace root).
pub(crate) fn resolve_preview_resource_path(
    preview_root: &str,
    request_path: &str,
) -> Result<String, ApiError> {
    let request_path = request_path.split('?').next().unwrap_or(request_path);
    let request_path = request_path.split('#').next().unwrap_or(request_path);
    let trimmed = request_path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return Err(ApiError::from_status_message(
            StatusCode::NOT_FOUND,
            "preview path not found",
        ));
    }

    let relative = trimmed.trim_start_matches('/');
    // Reject encoded traversal before decode-style normalization.
    if relative.contains('\\') {
        return Err(ApiError::bad_request("invalid preview path"));
    }

    let mut components = Vec::new();
    for part in relative.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(ApiError::bad_request("preview path escapes preview root"));
        }
        if part.contains('\0') {
            return Err(ApiError::bad_request("invalid preview path"));
        }
        components.push(part);
    }
    if components.is_empty() {
        return Err(ApiError::from_status_message(
            StatusCode::NOT_FOUND,
            "preview path not found",
        ));
    }

    let under_root = components.join("/");
    let full = if preview_root.is_empty() {
        under_root
    } else {
        format!("{preview_root}/{under_root}")
    };
    normalize_workspace_relative_path(&full)
}

/// Build a preview static response with shared MIME / cache / CSP headers.
/// Used by both local disk serving and remote sidecar so semantics stay identical.
pub(crate) fn build_preview_file_response(
    resource_rel: &str,
    body: Vec<u8>,
    content_length: u64,
    head_only: bool,
    foco_origin: &str,
    preview_resource_source: Option<&str>,
) -> Response {
    let mime = preview_mime_type(resource_rel);
    let builder = preview_response_builder(
        StatusCode::OK,
        mime,
        cache_control_for_preview(mime),
        foco_origin,
        preview_resource_source,
    );

    // CSP frame-ancestors is the embed control (Foco origin only).
    // Do not set X-Frame-Options: SAMEORIGIN — that would block the Foco parent iframe.
    if head_only {
        return builder
            .header(header::CONTENT_LENGTH, content_length.to_string())
            .body(Body::empty())
            .expect("preview HEAD response is valid");
    }

    builder
        .header(header::CONTENT_LENGTH, content_length.to_string())
        .body(Body::from(body))
        .expect("preview GET response is valid")
}

fn preview_response_builder(
    status: StatusCode,
    content_type: &str,
    cache_control: &str,
    foco_origin: &str,
    preview_resource_source: Option<&str>,
) -> axum::http::response::Builder {
    let csp = preview_content_security_policy_for_resource(
        foco_origin,
        preview_resource_source.unwrap_or("'self'"),
    );
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, cache_control)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CONTENT_SECURITY_POLICY, csp)
        .header(
            "Cross-Origin-Resource-Policy",
            if preview_resource_source.is_some() {
                // Path-mode documents have an opaque sandbox origin, so no-cors
                // images, styles, and classic scripts must be embeddable by it.
                "cross-origin"
            } else {
                "same-origin"
            },
        );

    if preview_resource_source.is_some() {
        // ES modules, fonts, and fetch() send `Origin: null` from a sandboxed
        // path-mode iframe. Scope this CORS grant to capability-token responses.
        builder = builder.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "null");
    }
    builder
}

/// Read a local preview file after confinement checks and build the HTTP response.
pub(crate) fn serve_local_preview_file(
    workspace_root: &Path,
    preview_root_rel: &str,
    resource_rel: &str,
    head_only: bool,
    foco_origin: &str,
    preview_resource_source: Option<&str>,
) -> Result<Response, ApiError> {
    let file_path = open_preview_file(workspace_root, preview_root_rel, resource_rel)?;
    if head_only {
        let content_length = fs::metadata(&file_path).map(|meta| meta.len()).unwrap_or(0);
        return Ok(build_preview_file_response(
            resource_rel,
            Vec::new(),
            content_length,
            true,
            foco_origin,
            preview_resource_source,
        ));
    }
    let bytes = fs::read(&file_path).map_err(|_| {
        ApiError::from_status_message(StatusCode::NOT_FOUND, "preview file was not found")
    })?;
    let content_length = bytes.len() as u64;
    Ok(build_preview_file_response(
        resource_rel,
        bytes,
        content_length,
        false,
        foco_origin,
        preview_resource_source,
    ))
}

/// Validate absolute path is a regular file under both workspace and preview root.
/// Rejects directories, missing files, and symlink targets that escape the preview root.
pub(crate) fn open_preview_file(
    workspace_root: &Path,
    preview_root_rel: &str,
    resource_rel: &str,
) -> Result<PathBuf, ApiError> {
    let workspace_root = fs::canonicalize(workspace_root).map_err(|source| {
        ApiError::internal(format!("failed to resolve workspace root: {source}"))
    })?;

    let preview_root_abs = if preview_root_rel.is_empty() {
        workspace_root.clone()
    } else {
        let candidate = workspace_root.join(preview_root_rel);
        fs::canonicalize(&candidate).map_err(|_| {
            ApiError::from_status_message(StatusCode::NOT_FOUND, "preview root was not found")
        })?
    };

    if !preview_root_abs.starts_with(&workspace_root) {
        return Err(ApiError::bad_request("preview root escapes workspace"));
    }

    let joined = workspace_root.join(resource_rel);
    let metadata = fs::symlink_metadata(&joined).map_err(|_| {
        ApiError::from_status_message(StatusCode::NOT_FOUND, "preview file was not found")
    })?;

    if metadata.file_type().is_dir() {
        return Err(ApiError::from_status_message(
            StatusCode::NOT_FOUND,
            "preview directory listing is disabled",
        ));
    }

    // Resolve symlinks then re-check confinement under preview root.
    let canonical = fs::canonicalize(&joined).map_err(|_| {
        ApiError::from_status_message(StatusCode::NOT_FOUND, "preview file was not found")
    })?;

    if !canonical.starts_with(&preview_root_abs) {
        return Err(ApiError::bad_request(
            "preview path escapes preview root (symlink or path traversal)",
        ));
    }
    if !canonical.starts_with(&workspace_root) {
        return Err(ApiError::bad_request("preview path escapes workspace"));
    }

    let file_meta = fs::metadata(&canonical).map_err(|_| {
        ApiError::from_status_message(StatusCode::NOT_FOUND, "preview file was not found")
    })?;
    if !file_meta.is_file() {
        return Err(ApiError::from_status_message(
            StatusCode::NOT_FOUND,
            "preview path is not a file",
        ));
    }

    Ok(canonical)
}

pub(crate) fn preview_mime_type(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" | "cjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "eot" => "application/vnd.ms-fontobject",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn is_html_mime(mime: &str) -> bool {
    mime.starts_with("text/html")
}

fn cache_control_for_preview(mime: &str) -> &'static str {
    if is_html_mime(mime) {
        "no-store"
    } else {
        // Revalidatable static assets; no long immutable cache for live editing.
        "private, max-age=0, must-revalidate"
    }
}

/// Content-Security-Policy for preview documents.
/// frame-ancestors restricts embedding to the Foco page origin (same host, not preview).
pub(crate) fn preview_content_security_policy(foco_origin: &str) -> String {
    preview_content_security_policy_for_resource(foco_origin, "'self'")
}

fn preview_content_security_policy_for_resource(
    foco_origin: &str,
    preview_resource_source: &str,
) -> String {
    // Allow the page's own scripts/styles/images/fonts/fetch (same preview origin).
    // frame-ancestors only Foco UI origin so arbitrary sites cannot embed the capability host.
    format!(
        "default-src {preview_resource_source}; script-src {preview_resource_source} 'unsafe-inline' 'unsafe-eval'; style-src {preview_resource_source} 'unsafe-inline'; img-src {preview_resource_source} data: blob:; font-src {preview_resource_source} data:; connect-src {preview_resource_source}; media-src {preview_resource_source} blob:; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors {foco_origin}"
    )
}

fn foco_page_origin(listen_addr: std::net::SocketAddr) -> String {
    let host = match listen_addr.ip() {
        std::net::IpAddr::V4(ip) if ip.is_unspecified() || ip.is_loopback() => {
            "127.0.0.1".to_string()
        }
        std::net::IpAddr::V6(ip) if ip.is_unspecified() || ip.is_loopback() => {
            "127.0.0.1".to_string()
        }
        other => other.to_string(),
    };
    format!("http://{host}:{}", listen_addr.port())
}

fn build_preview_session_response(
    state: &AppState,
    session: &PreviewSession,
    headers: &HeaderMap,
) -> PreviewSessionResponse {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");

    if preview_request_uses_host_origin(host) {
        let port = state.listen_addr.port();
        return PreviewSessionResponse {
            token: session.token.clone(),
            workspace_id: session.workspace_id.clone(),
            entry_path: session.entry_path.clone(),
            root_path: session.root_path.clone(),
            preview_url: preview_entry_url(
                &session.token,
                port,
                &session.entry_path,
                &session.root_path,
            ),
            preview_origin: preview_origin(&session.token, port),
            iframe_sandbox: PREVIEW_IFRAME_SANDBOX_HOST.to_string(),
        };
    }

    let public_origin = request_public_origin(headers, state.listen_addr);
    PreviewSessionResponse {
        token: session.token.clone(),
        workspace_id: session.workspace_id.clone(),
        entry_path: session.entry_path.clone(),
        root_path: session.root_path.clone(),
        preview_url: preview_path_entry_url(
            &public_origin,
            &session.token,
            &session.entry_path,
            &session.root_path,
        ),
        preview_origin: preview_path_origin(&public_origin, &session.token),
        iframe_sandbox: PREVIEW_IFRAME_SANDBOX_PATH.to_string(),
    }
}

pub(crate) async fn create_preview_session(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    headers: HeaderMap,
    Json(request): Json<CreatePreviewSessionRequest>,
) -> Result<Json<PreviewSessionResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    let entry_rel = normalize_workspace_relative_path(&request.path)?;
    if !is_html_entry_path(&entry_rel) {
        return Err(ApiError::bad_request(
            "preview entry must be an .html or .htm file",
        ));
    }

    let root_path = preview_root_for_entry(&entry_rel);

    if workspace.is_remote() {
        ensure_remote_entry_exists(&state, &workspace_id, &entry_rel, &root_path).await?;
    } else {
        validate_local_preview_entry(&workspace.path, &entry_rel)?;
    }

    let session = state
        .preview_sessions
        .create(workspace.id.clone(), entry_rel, root_path)?;

    tracing::info!(
        workspace_id = %session.workspace_id,
        entry_path = %session.entry_path,
        root_path = %session.root_path,
        remote = workspace.is_remote(),
        token = %redact_preview_token(&session.token),
        "created HTML preview session"
    );

    Ok(Json(build_preview_session_response(
        &state, &session, &headers,
    )))
}

fn validate_local_preview_entry(workspace_path: &Path, entry_rel: &str) -> Result<(), ApiError> {
    let entry_abs = crate::http::workspaces::workspace_file_path(workspace_path, entry_rel)?;
    let metadata = fs::metadata(&entry_abs)
        .map_err(|_| ApiError::bad_request(format!("workspace file was not found: {entry_rel}")))?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "workspace path is not a file: {entry_rel}"
        )));
    }

    // Reject symlink entry that leaves the workspace.
    let workspace_root = fs::canonicalize(workspace_path).map_err(|source| {
        ApiError::internal(format!("failed to resolve workspace root: {source}"))
    })?;
    let entry_canonical = fs::canonicalize(&entry_abs)
        .map_err(|_| ApiError::bad_request(format!("workspace file was not found: {entry_rel}")))?;
    if !entry_canonical.starts_with(&workspace_root) {
        return Err(ApiError::bad_request(
            "preview entry escapes the workspace (symlink)",
        ));
    }
    Ok(())
}

/// Confirm the remote entry is a regular file under the preview root via sidecar.
async fn ensure_remote_entry_exists(
    state: &AppState,
    workspace_id: &str,
    entry_rel: &str,
    root_path: &str,
) -> Result<(), ApiError> {
    crate::remote_workspace::ensure_remote_workspace_connected(state, workspace_id).await?;
    let (base, token) = match crate::remote_workspace::sidecar_proxy_target(state, workspace_id)? {
        crate::remote_workspace::SidecarProxyTarget::Connected { base, token } => (base, token),
        crate::remote_workspace::SidecarProxyTarget::Local => {
            return Err(ApiError::internal(
                "workspace became local while preparing remote preview",
            ));
        }
        crate::remote_workspace::SidecarProxyTarget::Disconnected => {
            return Err(ApiError::bad_gateway(format!(
                "remote workspace sidecar is not connected: {workspace_id}"
            )));
        }
    };

    let url = format!(
        "{}/api/remote/workspace/preview/file?path={}&root={}",
        base.trim_end_matches('/'),
        urlencoding_encode(entry_rel),
        urlencoding_encode(root_path),
    );
    let client = preview_sidecar_http_client().map_err(|source| {
        ApiError::internal(format!("failed to create preview HTTP client: {source}"))
    })?;
    let response = client
        .request(reqwest::Method::HEAD, &url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|source| {
            ApiError::bad_gateway(format!("failed to verify remote preview entry: {source}"))
        })?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let body = response.text().await.unwrap_or_default();
    let message = sidecar_error_message(&body).unwrap_or_else(|| {
        if status.as_u16() == 404 {
            format!("workspace file was not found: {entry_rel}")
        } else {
            format!(
                "failed to verify remote preview entry (HTTP {})",
                status.as_u16()
            )
        }
    });
    if status.as_u16() == 404 {
        return Err(ApiError::bad_request(message));
    }
    if status.as_u16() == 400 {
        return Err(ApiError::bad_request(message));
    }
    Err(ApiError::from_status_message(
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
        message,
    ))
}

fn urlencoding_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
                out.push(char::from(b"0123456789ABCDEF"[(byte & 0xf) as usize]));
            }
        }
    }
    out
}

fn preview_sidecar_http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(PREVIEW_SIDECAR_HTTP_TIMEOUT)
        .build()
}

fn sidecar_error_message(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
            return Some(error.to_string());
        }
        if let Some(message) = value.get("message").and_then(|v| v.as_str()) {
            return Some(message.to_string());
        }
    }
    // Sidecar preview errors return small HTML pages; strip tags for session create.
    let without_tags = trimmed
        .replace("<h1>", "")
        .replace("</h1>", " ")
        .replace("<p>", "")
        .replace("</p>", "");
    let plain: String = without_tags
        .chars()
        .filter(|c| !matches!(c, '<' | '>'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if plain.is_empty() {
        None
    } else {
        Some(plain.chars().take(200).collect())
    }
}

pub(crate) async fn release_preview_session(
    State(state): State<AppState>,
    AxumPath((workspace_id, token)): AxumPath<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let config = config_snapshot(&state)?;
    let _workspace = workspace_by_id(&config, &workspace_id)?;

    // Only release if the token belongs to this workspace (when still present).
    if let Ok(session) = state.preview_sessions.touch(&token) {
        if session.workspace_id != workspace_id {
            return Err(ApiError::bad_request(
                "preview session does not belong to this workspace",
            ));
        }
    }

    let released = state.preview_sessions.release(&token)?;
    tracing::info!(
        workspace_id = %workspace_id,
        token = %redact_preview_token(&token),
        released,
        "released HTML preview session"
    );
    Ok(Json(serde_json::json!({ "released": released })))
}

fn redact_preview_token(token: &str) -> String {
    if token.len() <= 4 {
        "****".to_string()
    } else {
        format!("{}…{}", &token[..2], &token[token.len() - 2..])
    }
}

/// Preview capability middleware: host-mode (`*.preview.localhost`) or path-mode
/// (`/__preview/{token}/...`) before SPA/API handling. Never falls through.
pub(crate) async fn preview_host_middleware(
    State(state): State<AppState>,
    request: Request,
    next: axum::middleware::Next,
) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let request_path = request.uri().path().to_string();

    let (token, resource_path, redacted_label, foco_origin, preview_resource_source) =
        if let Some(token) = parse_preview_host_token(host) {
            (
                token,
                request_path.clone(),
                redact_preview_host_for_log(host),
                foco_page_origin(state.listen_addr),
                None,
            )
        } else if let Some((token, resource_path)) = parse_preview_path(&request_path) {
            let public_origin = request_public_origin(request.headers(), state.listen_addr);
            let preview_resource_source =
                format!("{}/", preview_path_origin(&public_origin, &token));
            (
                token.clone(),
                resource_path,
                format!("/__preview/{}", redact_preview_token(&token)),
                public_origin,
                Some(preview_resource_source),
            )
        } else {
            return next.run(request).await;
        };

    let method = request.method().clone();
    if method != Method::GET && method != Method::HEAD {
        tracing::info!(
            preview = %redacted_label,
            %method,
            "preview capability rejected non-GET/HEAD method"
        );
        return preview_error_response_with_origin(
            StatusCode::METHOD_NOT_ALLOWED,
            "method not allowed",
            &foco_origin,
            preview_resource_source.as_deref(),
        );
    }

    match serve_preview_resource(
        &state,
        &token,
        &resource_path,
        method == Method::HEAD,
        &foco_origin,
        preview_resource_source.as_deref(),
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::info!(
                preview = %redacted_label,
                path = %resource_path,
                status = error.status.as_u16(),
                "preview resource error"
            );
            preview_error_response_with_origin(
                error.status,
                error.message(),
                &foco_origin,
                preview_resource_source.as_deref(),
            )
        }
    }
}

struct PreviewServeError {
    status: StatusCode,
    message: String,
}

impl PreviewServeError {
    fn message(&self) -> &str {
        &self.message
    }
}

impl From<ApiError> for PreviewServeError {
    fn from(error: ApiError) -> Self {
        Self {
            status: error.status(),
            message: error.message().to_string(),
        }
    }
}

async fn serve_preview_resource(
    state: &AppState,
    token: &str,
    request_path: &str,
    head_only: bool,
    foco_origin: &str,
    preview_resource_source: Option<&str>,
) -> Result<Response, PreviewServeError> {
    let session = state.preview_sessions.touch(token)?;

    let config = config_snapshot(state)?;
    let workspace = workspace_by_id(&config, &session.workspace_id)?;

    // `/` or empty → entry HTML under the preview root.
    let resource_rel = if request_path == "/" || request_path.is_empty() {
        session.entry_path.clone()
    } else {
        resolve_preview_resource_path(&session.root_path, request_path)?
    };

    let foco_origin = foco_origin.to_string();

    if workspace.is_remote() {
        return serve_remote_preview_resource(
            state,
            &session.workspace_id,
            &session.root_path,
            &resource_rel,
            head_only,
            &foco_origin,
            preview_resource_source,
        )
        .await;
    }

    serve_local_preview_file(
        &workspace.path,
        &session.root_path,
        &resource_rel,
        head_only,
        &foco_origin,
        preview_resource_source,
    )
    .map_err(PreviewServeError::from)
}

/// Fetch a preview resource from the SSH sidecar (bytes only; token stays on main process).
async fn serve_remote_preview_resource(
    state: &AppState,
    workspace_id: &str,
    preview_root: &str,
    resource_rel: &str,
    head_only: bool,
    foco_origin: &str,
    preview_resource_source: Option<&str>,
) -> Result<Response, PreviewServeError> {
    if let Err(error) =
        crate::remote_workspace::ensure_remote_workspace_connected(state, workspace_id).await
    {
        return Err(PreviewServeError {
            status: StatusCode::BAD_GATEWAY,
            message: error.message().to_string(),
        });
    }

    let (base, token) = match crate::remote_workspace::sidecar_proxy_target(state, workspace_id) {
        Ok(crate::remote_workspace::SidecarProxyTarget::Connected { base, token }) => (base, token),
        Ok(crate::remote_workspace::SidecarProxyTarget::Local) => {
            return Err(PreviewServeError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "workspace became local while serving remote preview".to_string(),
            });
        }
        Ok(crate::remote_workspace::SidecarProxyTarget::Disconnected) => {
            return Err(PreviewServeError {
                status: StatusCode::BAD_GATEWAY,
                message: format!("remote workspace sidecar is not connected: {workspace_id}"),
            });
        }
        Err(error) => {
            return Err(PreviewServeError {
                status: StatusCode::BAD_GATEWAY,
                message: error.message().to_string(),
            });
        }
    };

    let url = format!(
        "{}/api/remote/workspace/preview/file?path={}&root={}",
        base.trim_end_matches('/'),
        urlencoding_encode(resource_rel),
        urlencoding_encode(preview_root),
    );
    let method = if head_only {
        reqwest::Method::HEAD
    } else {
        reqwest::Method::GET
    };

    let client = preview_sidecar_http_client().map_err(|source| PreviewServeError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to create preview HTTP client: {source}"),
    })?;
    let response = client
        .request(method, &url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|source| PreviewServeError {
            status: StatusCode::BAD_GATEWAY,
            message: format!("remote preview fetch failed: {source}"),
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let message = sidecar_error_message(&body).unwrap_or_else(|| {
            if status.as_u16() == 404 {
                "preview file was not found".to_string()
            } else {
                format!("remote preview fetch failed (HTTP {})", status.as_u16())
            }
        });
        let mapped = match status.as_u16() {
            404 => StatusCode::NOT_FOUND,
            400 => StatusCode::BAD_REQUEST,
            401 | 403 => StatusCode::BAD_GATEWAY,
            other => StatusCode::from_u16(other).unwrap_or(StatusCode::BAD_GATEWAY),
        };
        return Err(PreviewServeError {
            status: mapped,
            message,
        });
    }

    // Prefer main-process MIME/cache/CSP so local and remote responses stay aligned.
    // Forward Content-Length when present; stream body for GET so large files are not
    // fully buffered beyond the HTTP client body.
    let content_length_header = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());

    if head_only {
        let content_length = content_length_header.unwrap_or(0);
        return Ok(build_preview_file_response(
            resource_rel,
            Vec::new(),
            content_length,
            true,
            foco_origin,
            preview_resource_source,
        ));
    }

    // Stream remote bytes: dropping the client body typically stops further
    // upstream consumption without unbounded full-file buffering. No explicit
    // cancel channel. Content-Length is taken from upstream when available.
    let content_length = content_length_header;
    let mime = preview_mime_type(resource_rel);
    let mut builder = preview_response_builder(
        StatusCode::OK,
        mime,
        cache_control_for_preview(mime),
        foco_origin,
        preview_resource_source,
    );
    if let Some(len) = content_length {
        builder = builder.header(header::CONTENT_LENGTH, len.to_string());
    }
    Ok(builder
        .body(Body::from_stream(response.bytes_stream()))
        .expect("remote preview stream response is valid"))
}

#[allow(dead_code)]
fn preview_error_response(status: StatusCode, message: &str, state: &AppState) -> Response {
    preview_error_response_with_origin(status, message, &foco_page_origin(state.listen_addr), None)
}

fn preview_error_response_with_origin(
    status: StatusCode,
    message: &str,
    foco_origin: &str,
    preview_resource_source: Option<&str>,
) -> Response {
    // Never leak absolute filesystem paths.
    let safe = sanitize_preview_error_message(message);
    let body = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Preview error</title></head><body><h1>{}</h1><p>{}</p></body></html>",
        status.as_u16(),
        html_escape(&safe)
    );
    preview_response_builder(
        status,
        "text/html; charset=utf-8",
        "no-store",
        foco_origin,
        preview_resource_source,
    )
    .body(Body::from(body))
    .expect("preview error response is valid")
}

fn sanitize_preview_error_message(message: &str) -> String {
    // Drop anything that looks like an absolute path segment dump.
    let mut out = message.to_string();
    for needle in ["/Users/", "/home/", "C:\\", "\\\\"] {
        if let Some(idx) = out.find(needle) {
            out.truncate(idx);
            out.push_str("…");
            break;
        }
    }
    if out.trim().is_empty() {
        "preview request failed".to_string()
    } else {
        out
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// True when this request is a preview capability route (host or path mode).
/// Used by auth bypass so capability tokens alone authorize reads.
pub(crate) fn request_is_preview_capability(headers: &HeaderMap, path: &str) -> bool {
    if headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_preview_host_token)
        .is_some()
    {
        return true;
    }
    parse_preview_path(path).is_some()
}

/// Backward-compatible host-only check (host mode).
pub(crate) fn request_is_preview_host(headers: &HeaderMap) -> bool {
    headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_preview_host_token)
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn dns_safe_token_rejects_uppercase_and_symbols() {
        assert!(is_dns_safe_preview_token("abc123xyz"));
        assert!(!is_dns_safe_preview_token("Abc123"));
        assert!(!is_dns_safe_preview_token("abc-123"));
        assert!(!is_dns_safe_preview_token(""));
        assert!(!is_dns_safe_preview_token("a.b"));
    }

    #[test]
    fn parse_preview_host_token_accepts_port_and_case() {
        assert_eq!(
            parse_preview_host_token("abcd1234.preview.localhost:3210").as_deref(),
            Some("abcd1234")
        );
        // Uppercase host is lowercased; token labels must remain dns-safe alnum.
        assert_eq!(
            parse_preview_host_token("abcd1234.PREVIEW.LOCALHOST").as_deref(),
            Some("abcd1234")
        );
        // Host is lowercased; uppercase token labels become lowercase tokens.
        assert_eq!(
            parse_preview_host_token("ABCD1234.PREVIEW.LOCALHOST").as_deref(),
            Some("abcd1234")
        );
        assert!(parse_preview_host_token("preview.localhost").is_none());
        assert!(parse_preview_host_token("evil.abcd.preview.localhost").is_none());
        assert!(parse_preview_host_token("127.0.0.1:3210").is_none());
    }

    #[test]
    fn resolve_preview_resource_rejects_traversal() {
        let err = resolve_preview_resource_path("demo", "/../secret.txt").unwrap_err();
        assert!(err.message().contains("escapes") || err.message().contains("invalid"));
        let err = resolve_preview_resource_path("demo", "/foo/../../x").unwrap_err();
        assert!(err.message().contains("escapes"));
        let ok = resolve_preview_resource_path("demo", "/assets/app.js").unwrap();
        assert_eq!(ok, "demo/assets/app.js");
        let ok_root = resolve_preview_resource_path("", "/assets/app.js").unwrap();
        assert_eq!(ok_root, "assets/app.js");
    }

    #[test]
    fn open_preview_file_rejects_directory_and_escape() {
        let root = tempdir().unwrap();
        let preview = root.path().join("site");
        fs::create_dir_all(preview.join("assets")).unwrap();
        fs::write(preview.join("index.html"), b"<html></html>").unwrap();
        fs::write(preview.join("assets/app.js"), b"console.log(1)").unwrap();
        fs::write(root.path().join("secret.txt"), b"nope").unwrap();

        let file = open_preview_file(root.path(), "site", "site/index.html").unwrap();
        assert!(file.ends_with("index.html"));

        let dir_err = open_preview_file(root.path(), "site", "site/assets").unwrap_err();
        assert!(
            dir_err.message().contains("directory") || dir_err.message().contains("not a file")
        );

        // Symlink outside preview root
        #[cfg(unix)]
        {
            let link = preview.join("leak.txt");
            std::os::unix::fs::symlink(root.path().join("secret.txt"), &link).unwrap();
            let err = open_preview_file(root.path(), "site", "site/leak.txt").unwrap_err();
            assert!(err.message().contains("escapes") || err.message().contains("not found"));
        }
    }

    #[test]
    fn mime_types_cover_common_web_assets() {
        assert!(preview_mime_type("a.html").starts_with("text/html"));
        assert!(preview_mime_type("a.css").starts_with("text/css"));
        assert!(preview_mime_type("a.js").starts_with("text/javascript"));
        assert!(preview_mime_type("a.mjs").starts_with("text/javascript"));
        assert!(preview_mime_type("a.json").starts_with("application/json"));
        assert_eq!(preview_mime_type("a.wasm"), "application/wasm");
        assert_eq!(preview_mime_type("a.woff2"), "font/woff2");
        assert_eq!(preview_mime_type("a.png"), "image/png");
        assert_eq!(preview_mime_type("a.mp4"), "video/mp4");
        assert!(preview_mime_type("a.js.map").starts_with("application/json"));
    }

    #[test]
    fn session_registry_expires_and_releases() {
        let registry = PreviewSessionRegistry::default();
        let session = registry
            .create("ws1".into(), "index.html".into(), "".into())
            .unwrap();
        assert_eq!(registry.len(), 1);
        assert!(registry.touch(&session.token).is_ok());
        assert!(registry.release(&session.token).unwrap());
        assert_eq!(registry.len(), 0);
        assert!(registry.touch(&session.token).is_err());
    }

    #[test]
    fn session_registry_idle_expiry() {
        let registry = PreviewSessionRegistry::default();
        let session = registry
            .create("ws1".into(), "index.html".into(), "".into())
            .unwrap();
        registry.force_expire_token_for_test(&session.token);
        assert!(registry.touch(&session.token).is_err());
    }

    #[test]
    fn iframe_sandbox_is_minimal() {
        assert_eq!(PREVIEW_IFRAME_SANDBOX, "allow-scripts allow-same-origin");
        assert_eq!(
            PREVIEW_IFRAME_SANDBOX_HOST,
            "allow-scripts allow-same-origin"
        );
        assert_eq!(PREVIEW_IFRAME_SANDBOX_PATH, "allow-scripts");
        assert!(!PREVIEW_IFRAME_SANDBOX_PATH.contains("allow-same-origin"));
        assert!(!PREVIEW_IFRAME_SANDBOX.contains("allow-top-navigation"));
        assert!(!PREVIEW_IFRAME_SANDBOX.contains("allow-popups"));
        assert!(!PREVIEW_IFRAME_SANDBOX.contains("allow-downloads"));
        assert!(!PREVIEW_IFRAME_SANDBOX.contains("allow-forms"));
        assert!(!PREVIEW_IFRAME_SANDBOX.contains("allow-pointer-lock"));
    }

    #[test]
    fn parse_preview_path_accepts_token_and_resource() {
        let token = "abcdefghijklmnopqrstuvwxyz012345";
        assert_eq!(
            parse_preview_path(&format!("/__preview/{token}")),
            Some((token.to_string(), "/".to_string()))
        );
        assert_eq!(
            parse_preview_path(&format!("/__preview/{token}/index.html")),
            Some((token.to_string(), "/index.html".to_string()))
        );
        assert_eq!(
            parse_preview_path(&format!("/__preview/{token}/assets/app.js")),
            Some((token.to_string(), "/assets/app.js".to_string()))
        );
        assert!(parse_preview_path("/__preview/").is_none());
        assert!(parse_preview_path("/api/workspaces/x/preview/sessions").is_none());
        assert!(parse_preview_path("/__preview/BAD_TOKEN/index.html").is_none());
    }

    #[test]
    fn preview_request_uses_host_origin_for_loopback_only() {
        assert!(preview_request_uses_host_origin("127.0.0.1:3210"));
        assert!(preview_request_uses_host_origin("localhost:3210"));
        assert!(preview_request_uses_host_origin("[::1]:3210"));
        assert!(!preview_request_uses_host_origin("foco.fonlan.top"));
        assert!(!preview_request_uses_host_origin("example.com:443"));
    }

    #[test]
    fn request_public_origin_prefers_forwarded_proto() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "foco.fonlan.top".parse().unwrap());
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        let origin =
            request_public_origin(&headers, std::net::SocketAddr::from(([127, 0, 0, 1], 3210)));
        assert_eq!(origin, "https://foco.fonlan.top");
    }

    #[test]
    fn csp_frame_ancestors_limits_embedding() {
        let csp = preview_content_security_policy("http://127.0.0.1:3210");
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("frame-ancestors http://127.0.0.1:3210"));
        assert!(csp.contains("form-action 'none'"));
    }

    #[test]
    fn path_mode_csp_uses_explicit_capability_source() {
        let source = "https://foco.example/__preview/abcdefghijklmnopqrstuvwxyz012345/";
        let csp = preview_content_security_policy_for_resource("https://foco.example", source);
        assert!(csp.contains(&format!("default-src {source}")));
        assert!(csp.contains(&format!("script-src {source}")));
        assert!(csp.contains(&format!("img-src {source} data: blob:")));
        assert!(!csp.contains("script-src 'self'"));
    }

    #[test]
    fn redact_host_hides_token() {
        let redacted =
            redact_preview_host_for_log("abcdefghijklmnopqrstuvwxyz012345.preview.localhost:3210");
        assert!(!redacted.contains("abcdefghijklmnopqrstuvwxyz012345"));
        assert!(redacted.contains("preview.localhost"));
    }

    #[test]
    fn preview_root_for_entry_uses_parent() {
        assert_eq!(preview_root_for_entry("index.html"), "");
        assert_eq!(preview_root_for_entry("demo/index.html"), "demo");
        assert_eq!(preview_root_for_entry("a/b/c.html"), "a/b");
    }

    #[test]
    fn urlencoding_keeps_path_slashes() {
        assert_eq!(
            urlencoding_encode("demo/assets/app.js"),
            "demo/assets/app.js"
        );
        assert!(urlencoding_encode("a b").contains("%20"));
        assert!(!urlencoding_encode("demo/file.js").contains("%2F"));
    }

    #[test]
    fn serve_local_preview_file_sets_shared_headers() {
        let root = tempdir().unwrap();
        let preview = root.path().join("site");
        fs::create_dir_all(&preview).unwrap();
        fs::write(preview.join("index.html"), b"<html>hi</html>").unwrap();

        let response = serve_local_preview_file(
            root.path(),
            "site",
            "site/index.html",
            false,
            "http://127.0.0.1:3210",
            None,
        )
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers();
        assert!(
            headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .starts_with("text/html")
        );
        assert_eq!(
            headers
                .get(header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
            Some("no-store")
        );
        assert!(
            headers
                .get(header::CONTENT_SECURITY_POLICY)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .contains("frame-ancestors http://127.0.0.1:3210")
        );
        assert_eq!(
            headers
                .get("Cross-Origin-Resource-Policy")
                .and_then(|v| v.to_str().ok()),
            Some("same-origin")
        );
        assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());

        let head = serve_local_preview_file(
            root.path(),
            "site",
            "site/index.html",
            true,
            "http://127.0.0.1:3210",
            None,
        )
        .unwrap();
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(
            head.headers()
                .get(header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok()),
            Some("15")
        );
    }

    #[test]
    fn static_assets_use_revalidate_cache_not_no_store() {
        let root = tempdir().unwrap();
        let preview = root.path().join("site");
        fs::create_dir_all(preview.join("assets")).unwrap();
        fs::write(preview.join("assets/app.js"), b"export const n = 1;\n").unwrap();
        fs::write(preview.join("assets/data.json"), b"{\"ok\":true}").unwrap();

        let js = serve_local_preview_file(
            root.path(),
            "site",
            "site/assets/app.js",
            false,
            "http://127.0.0.1:3210",
            None,
        )
        .unwrap();
        assert!(
            js.headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .starts_with("text/javascript")
        );
        assert_eq!(
            js.headers()
                .get(header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
            Some("private, max-age=0, must-revalidate")
        );

        let json = serve_local_preview_file(
            root.path(),
            "site",
            "site/assets/data.json",
            false,
            "http://127.0.0.1:3210",
            None,
        )
        .unwrap();
        assert!(
            json.headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .starts_with("application/json")
        );
    }

    #[test]
    fn generated_token_is_dns_safe_and_unique() {
        let registry = PreviewSessionRegistry::default();
        let a = registry
            .create("ws1".into(), "index.html".into(), "".into())
            .unwrap();
        let b = registry
            .create("ws1".into(), "other.html".into(), "".into())
            .unwrap();
        assert_ne!(a.token, b.token);
        assert_eq!(a.token.len(), PREVIEW_TOKEN_LEN);
        assert!(is_dns_safe_preview_token(&a.token));
        assert!(is_dns_safe_preview_token(&b.token));
    }

    #[test]
    fn session_registry_rejects_invalid_release_token() {
        let registry = PreviewSessionRegistry::default();
        assert!(registry.release("").is_err());
        assert!(registry.release("NOT-dns-safe").is_err());
        // DNS-safe shape but absent: Ok(false), not an error and not a false success.
        assert_eq!(registry.release("abc").unwrap(), false);
        assert_eq!(
            registry
                .release("abcdefghijklmnopqrstuvwxyz012345")
                .unwrap(),
            false
        );
    }

    #[test]
    fn session_registry_enforces_max_count() {
        let registry = PreviewSessionRegistry::default();
        for i in 0..PREVIEW_SESSION_MAX_COUNT {
            registry
                .create("ws1".into(), format!("entry-{i}.html"), "".into())
                .unwrap_or_else(|error| panic!("create {i}: {}", error.message()));
        }
        assert_eq!(registry.len(), PREVIEW_SESSION_MAX_COUNT);
        let err = registry
            .create("ws1".into(), "overflow.html".into(), "".into())
            .unwrap_err();
        assert!(
            err.message().contains("limit reached"),
            "message={}",
            err.message()
        );
        assert_eq!(registry.len(), PREVIEW_SESSION_MAX_COUNT);
    }

    #[test]
    fn entry_url_is_relative_to_preview_root() {
        let url = preview_entry_url(
            "tokentokentokentokentokentoken12",
            3210,
            "demo/index.html",
            "demo",
        );
        assert!(url.starts_with("http://tokentokentokentokentokentoken12.preview.localhost:3210/"));
        assert!(url.ends_with("/index.html"));
        assert!(!url.contains("/demo/index.html"));
    }

    #[test]
    fn error_message_sanitizer_strips_absolute_paths() {
        let msg =
            sanitize_preview_error_message("failed to read /Users/fonlan/secret/project/file.txt");
        assert!(!msg.contains("/Users/"));
        assert!(!msg.contains("secret"));
        assert!(msg.contains("…"));
    }
}
