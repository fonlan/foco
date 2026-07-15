//! In-process HTML preview sessions and static origin serving.
//!
//! Capability tokens live only in process memory. Preview pages are served from
//! `<token>.preview.localhost` so workspace scripts cannot share Foco cookies,
//! localStorage, or same-origin API access with the host UI.

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
use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::{
    ApiError, AppState, config_snapshot, normalize_workspace_relative_path, workspace_by_id,
};

/// Idle sessions expire after this duration without access.
pub(crate) const PREVIEW_SESSION_IDLE_TTL: Duration = Duration::from_secs(30 * 60);
/// Hard cap on concurrent preview sessions in one process.
pub(crate) const PREVIEW_SESSION_MAX_COUNT: usize = 64;
/// DNS label length for the token subdomain (a–z0–9 only).
const PREVIEW_TOKEN_LEN: usize = 32;
const PREVIEW_HOST_SUFFIX: &str = ".preview.localhost";

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
    let mut rng = rand::rng();
    for _ in 0..32 {
        let mut token = String::with_capacity(PREVIEW_TOKEN_LEN);
        for _ in 0..PREVIEW_TOKEN_LEN {
            let idx = rng.random_range(0..ALPHABET.len());
            token.push(ALPHABET[idx] as char);
        }
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

/// Recommended iframe sandbox: scripts + same-origin within the preview origin only.
/// Does not include top-navigation, popups, downloads, forms, or pointer-lock.
pub(crate) const PREVIEW_IFRAME_SANDBOX: &str = "allow-scripts allow-same-origin";

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
    // Allow the page's own scripts/styles/images/fonts/fetch (same preview origin).
    // frame-ancestors only Foco UI origin so arbitrary sites cannot embed the capability host.
    format!(
        "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self'; media-src 'self' blob:; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors {foco_origin}"
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
) -> PreviewSessionResponse {
    let port = state.listen_addr.port();
    PreviewSessionResponse {
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
        iframe_sandbox: PREVIEW_IFRAME_SANDBOX.to_string(),
    }
}

pub(crate) async fn create_preview_session(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<CreatePreviewSessionRequest>,
) -> Result<Json<PreviewSessionResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    if workspace.is_remote() {
        // Phase 2 adds remote/sidecar file reads; phase 1 is local-only.
        return Err(ApiError::bad_request(
            "HTML preview for remote SSH workspaces is not available yet",
        ));
    }

    let entry_rel = normalize_workspace_relative_path(&request.path)?;
    if !is_html_entry_path(&entry_rel) {
        return Err(ApiError::bad_request(
            "preview entry must be an .html or .htm file",
        ));
    }

    let entry_abs = crate::http::workspaces::workspace_file_path(&workspace.path, &entry_rel)?;
    let metadata = fs::metadata(&entry_abs)
        .map_err(|_| ApiError::bad_request(format!("workspace file was not found: {entry_rel}")))?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "workspace path is not a file: {entry_rel}"
        )));
    }

    // Reject symlink entry that leaves the workspace.
    let workspace_root = fs::canonicalize(&workspace.path).map_err(|source| {
        ApiError::internal(format!("failed to resolve workspace root: {source}"))
    })?;
    let entry_canonical = fs::canonicalize(&entry_abs)
        .map_err(|_| ApiError::bad_request(format!("workspace file was not found: {entry_rel}")))?;
    if !entry_canonical.starts_with(&workspace_root) {
        return Err(ApiError::bad_request(
            "preview entry escapes the workspace (symlink)",
        ));
    }

    let root_path = preview_root_for_entry(&entry_rel);
    let session = state
        .preview_sessions
        .create(workspace.id.clone(), entry_rel, root_path)?;

    tracing::info!(
        workspace_id = %session.workspace_id,
        entry_path = %session.entry_path,
        root_path = %session.root_path,
        token = %redact_preview_token(&session.token),
        "created HTML preview session"
    );

    Ok(Json(build_preview_session_response(&state, &session)))
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

/// Host-aware middleware: serve preview origin before SPA/API handling.
/// Preview Host requests never fall through to Foco SPA or /api routes.
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

    let Some(token) = parse_preview_host_token(host) else {
        return next.run(request).await;
    };

    let redacted_host = redact_preview_host_for_log(host);
    let method = request.method().clone();
    if method != Method::GET && method != Method::HEAD {
        tracing::info!(
            host = %redacted_host,
            %method,
            "preview origin rejected non-GET/HEAD method"
        );
        return preview_error_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "method not allowed",
            &state,
        );
    }

    let path = request.uri().path().to_string();
    match serve_preview_resource(&state, &token, &path, method == Method::HEAD).await {
        Ok(response) => response,
        Err(error) => {
            tracing::info!(
                host = %redacted_host,
                path = %path,
                status = error.status.as_u16(),
                "preview resource error"
            );
            preview_error_response(error.status, error.message(), &state)
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
        // ApiError fields are private; re-map via into_response is heavy —
        // use message + a status helper by reconstructing common cases.
        let message = error.message().to_string();
        let status = if message.contains("not found")
            || message.contains("disabled")
            || message.contains("is not a file")
        {
            StatusCode::NOT_FOUND
        } else if message.contains("escapes") || message.contains("invalid") {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::BAD_REQUEST
        };
        Self { status, message }
    }
}

// Access ApiError status via a thin wrapper using existing constructors only.
// We store status on PreviewServeError when building from known paths.

async fn serve_preview_resource(
    state: &AppState,
    token: &str,
    request_path: &str,
    head_only: bool,
) -> Result<Response, PreviewServeError> {
    let session = state
        .preview_sessions
        .touch(token)
        .map_err(|error| PreviewServeError {
            status: StatusCode::NOT_FOUND,
            message: error.message().to_string(),
        })?;

    let config = config_snapshot(state).map_err(|error| PreviewServeError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: error.message().to_string(),
    })?;
    let workspace =
        workspace_by_id(&config, &session.workspace_id).map_err(|error| PreviewServeError {
            status: StatusCode::NOT_FOUND,
            message: error.message().to_string(),
        })?;

    if workspace.is_remote() {
        return Err(PreviewServeError {
            status: StatusCode::BAD_GATEWAY,
            message: "remote preview is not available yet".to_string(),
        });
    }

    // `/` or empty → entry HTML under the preview root.
    let resource_rel = if request_path == "/" || request_path.is_empty() {
        session.entry_path.clone()
    } else {
        resolve_preview_resource_path(&session.root_path, request_path).map_err(|error| {
            let lower = error.message().to_lowercase();
            let status = if lower.contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::BAD_REQUEST
            };
            PreviewServeError {
                status,
                message: error.message().to_string(),
            }
        })?
    };
    let file_path =
        open_preview_file(&workspace.path, &session.root_path, &resource_rel).map_err(|error| {
            let lower = error.message().to_lowercase();
            let status = if lower.contains("not found") || lower.contains("disabled") {
                StatusCode::NOT_FOUND
            } else if lower.contains("escapes") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::BAD_REQUEST
            };
            PreviewServeError {
                status,
                message: error.message().to_string(),
            }
        })?;

    let mime = preview_mime_type(&resource_rel);
    let bytes = if head_only {
        Vec::new()
    } else {
        fs::read(&file_path).map_err(|_| PreviewServeError {
            status: StatusCode::NOT_FOUND,
            message: "preview file was not found".to_string(),
        })?
    };
    let content_length = if head_only {
        fs::metadata(&file_path).map(|meta| meta.len()).unwrap_or(0)
    } else {
        bytes.len() as u64
    };

    let foco_origin = foco_page_origin(state.listen_addr);
    let csp = preview_content_security_policy(&foco_origin);

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, cache_control_for_preview(mime))
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CONTENT_SECURITY_POLICY, csp)
        // Preview origin must not accept Foco auth cookies (different host).
        .header("Cross-Origin-Resource-Policy", "same-origin");

    // CSP frame-ancestors is the embed control (Foco origin only).
    // Do not set X-Frame-Options: SAMEORIGIN — that would block the Foco parent iframe.
    if head_only {
        builder = builder.header(header::CONTENT_LENGTH, content_length.to_string());
        return Ok(builder
            .body(Body::empty())
            .expect("preview HEAD response is valid"));
    }

    Ok(builder
        .header(header::CONTENT_LENGTH, content_length.to_string())
        .body(Body::from(bytes))
        .expect("preview GET response is valid"))
}

fn preview_error_response(status: StatusCode, message: &str, state: &AppState) -> Response {
    // Never leak absolute filesystem paths.
    let safe = sanitize_preview_error_message(message);
    let foco_origin = foco_page_origin(state.listen_addr);
    let csp = preview_content_security_policy(&foco_origin);
    let body = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Preview error</title></head><body><h1>{}</h1><p>{}</p></body></html>",
        status.as_u16(),
        html_escape(&safe)
    );
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CONTENT_SECURITY_POLICY, csp)
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

/// True when this request Host is a preview virtual site (used by auth bypass).
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
        assert!(!PREVIEW_IFRAME_SANDBOX.contains("allow-top-navigation"));
        assert!(!PREVIEW_IFRAME_SANDBOX.contains("allow-popups"));
        assert!(!PREVIEW_IFRAME_SANDBOX.contains("allow-downloads"));
        assert!(!PREVIEW_IFRAME_SANDBOX.contains("allow-forms"));
        assert!(!PREVIEW_IFRAME_SANDBOX.contains("allow-pointer-lock"));
    }

    #[test]
    fn csp_frame_ancestors_limits_embedding() {
        let csp = preview_content_security_policy("http://127.0.0.1:3210");
        assert!(csp.contains("frame-ancestors http://127.0.0.1:3210"));
        assert!(csp.contains("form-action 'none'"));
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
}
