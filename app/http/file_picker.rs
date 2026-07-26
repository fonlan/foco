use std::{
    cmp::Ordering,
    env, fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

use axum::{Json, extract::State};
use base64::{Engine as _, engine::general_purpose};
use foco_store::config::WorkspaceLocation;
use serde::{Deserialize, Serialize};

use crate::{
    ApiError, AppResult, AppState, MAX_CHAT_ATTACHMENT_BYTES, MAX_CHAT_ATTACHMENT_TOTAL_BYTES,
    MAX_CHAT_ATTACHMENTS, attachment_content_type_for_path, config_snapshot,
};

pub(crate) const FILE_PICKER_LIST_COMMAND: &str = "--file-picker-list";
pub(crate) const FILE_PICKER_READ_FILES_COMMAND: &str = "--file-picker-read-files";

const DEFAULT_LIST_LIMIT: usize = 500;
const MAX_LIST_LIMIT: usize = 1_000;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePickerRootsRequest {
    #[serde(default)]
    target: Option<FilePickerTarget>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePickerListRequest {
    target: FilePickerTarget,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    mode: FilePickerMode,
    #[serde(default)]
    include_files: bool,
    #[serde(default)]
    show_hidden: bool,
    #[serde(default)]
    limit: Option<usize>,
    /// Attachment picker only: list any readable absolute path on the host.
    /// Default false keeps workspace-root clamping for other pickers.
    #[serde(default)]
    allow_outside_workspace: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePickerReadFilesRequest {
    target: FilePickerTarget,
    paths: Vec<String>,
    /// Attachment picker only: read any readable absolute file on the host.
    /// Default false keeps workspace-root clamping for other pickers.
    #[serde(default)]
    allow_outside_workspace: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum FilePickerTarget {
    Local,
    RemoteServer { server_id: String },
    Workspace { workspace_id: String },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FilePickerMode {
    File,
    #[default]
    Directory,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePickerRootsResponse {
    roots: Vec<FilePickerRoot>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePickerRoot {
    label: String,
    path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePickerListResponse {
    path: String,
    parent_path: Option<String>,
    entries: Vec<FilePickerEntry>,
    truncated: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePickerEntry {
    name: String,
    path: String,
    is_directory: bool,
    size_bytes: Option<u64>,
    modified_ms: Option<i64>,
    disabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePickerReadFilesResponse {
    files: Vec<NativeSelectedFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeSelectedFile {
    path: String,
    name: String,
    content_type: String,
    size_bytes: u64,
    content_base64: Option<String>,
}

pub(crate) async fn file_picker_roots(
    State(state): State<AppState>,
    Json(request): Json<FilePickerRootsRequest>,
) -> Result<Json<FilePickerRootsResponse>, ApiError> {
    match request.target.unwrap_or(FilePickerTarget::Local) {
        FilePickerTarget::Local => Ok(Json(local_roots_response())),
        FilePickerTarget::Workspace { workspace_id } => {
            let config = config_snapshot(&state)?;
            let workspace = config
                .workspaces
                .iter()
                .find(|workspace| workspace.id == workspace_id)
                .ok_or_else(|| {
                    ApiError::bad_request(format!("workspace not found: {workspace_id}"))
                })?;
            match &workspace.location {
                WorkspaceLocation::Local => Ok(Json(FilePickerRootsResponse {
                    roots: vec![FilePickerRoot {
                        label: workspace.name.clone(),
                        path: workspace.path.display().to_string(),
                    }],
                })),
                WorkspaceLocation::Ssh { remote_path, .. } => Ok(Json(FilePickerRootsResponse {
                    roots: vec![FilePickerRoot {
                        label: workspace.name.clone(),
                        path: remote_path.clone(),
                    }],
                })),
            }
        }
        FilePickerTarget::RemoteServer { server_id } => {
            let config = config_snapshot(&state)?;
            let server = config
                .remote_servers
                .iter()
                .find(|server| server.id == server_id)
                .ok_or_else(|| {
                    ApiError::bad_request(format!("remote server not found: {server_id}"))
                })?
                .clone();
            let root_path = server
                .default_remote_root
                .clone()
                .unwrap_or_else(|| "/".to_string());
            let path = if foco_store::config::needs_remote_home_expansion(&root_path) {
                let profile = crate::ssh_client::resolve_ssh_profile(
                    &server,
                    crate::ssh_client::ResolveSshOptions::default(),
                )
                .map_err(|err| ApiError::bad_request(err.message().to_string()))?;
                let mut session = crate::ssh_client::SshSession::connect(&profile)
                    .await
                    .map_err(|err| ApiError::bad_gateway(err.message().to_string()))?;
                crate::ssh_client::expand_remote_path(&mut session, &root_path)
                    .await
                    .map_err(|err| ApiError::bad_gateway(err.message().to_string()))?
            } else {
                root_path
            };
            Ok(Json(FilePickerRootsResponse {
                roots: vec![FilePickerRoot {
                    label: server.name.clone(),
                    path,
                }],
            }))
        }
    }
}

pub(crate) async fn file_picker_list(
    State(state): State<AppState>,
    Json(request): Json<FilePickerListRequest>,
) -> Result<Json<FilePickerListResponse>, ApiError> {
    match request.target.clone() {
        FilePickerTarget::Local => Ok(Json(list_local(request, None)?)),
        FilePickerTarget::Workspace { workspace_id } => {
            list_workspace(&state, &workspace_id, request).await
        }
        FilePickerTarget::RemoteServer { server_id } => {
            list_remote_server(&state, &server_id, request).await
        }
    }
}

pub(crate) async fn file_picker_read_files(
    State(state): State<AppState>,
    Json(request): Json<FilePickerReadFilesRequest>,
) -> Result<Json<FilePickerReadFilesResponse>, ApiError> {
    match request.target.clone() {
        FilePickerTarget::Local => Ok(Json(FilePickerReadFilesResponse {
            files: selected_files_from_paths(request.paths.iter().map(PathBuf::from).collect())?,
        })),
        FilePickerTarget::Workspace { workspace_id } => {
            read_workspace_files(&state, &workspace_id, request).await
        }
        FilePickerTarget::RemoteServer { server_id } => {
            read_remote_server_files(&state, &server_id, request).await
        }
    }
}

pub(crate) fn run_file_picker_cli_if_requested(command: &str) -> AppResult<bool> {
    match command {
        FILE_PICKER_LIST_COMMAND => {
            let request: FilePickerListRequest = read_stdin_json()?;
            let response = list_local(request, None).map_err(|error| {
                io::Error::new(io::ErrorKind::Other, error.message().to_string())
            })?;
            println!("{}", serde_json::to_string(&response)?);
            Ok(true)
        }
        FILE_PICKER_READ_FILES_COMMAND => {
            let request: FilePickerReadFilesRequest = read_stdin_json()?;
            let files =
                selected_files_from_paths(request.paths.iter().map(PathBuf::from).collect())
                    .map_err(|error| {
                        io::Error::new(io::ErrorKind::Other, error.message().to_string())
                    })?;
            println!(
                "{}",
                serde_json::to_string(&FilePickerReadFilesResponse { files })?
            );
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub(crate) async fn remote_sidecar_file_picker_list(
    State(state): State<crate::remote_workspace::RemoteSidecarState>,
    Json(mut request): Json<FilePickerListRequest>,
) -> Result<Json<FilePickerListResponse>, ApiError> {
    request.target = FilePickerTarget::Local;
    let root = fs::canonicalize(&state.workspace_path).map_err(|source| {
        ApiError::bad_request(format!("remote workspace root is not readable: {source}"))
    })?;
    let restricted_root = if request.allow_outside_workspace {
        None
    } else {
        Some(root.as_path())
    };
    // When unrestricted, still open at the workspace path if the client sent empty path.
    let request = rewrite_unrestricted_empty_path_to_workspace_root(request, &root);
    Ok(Json(list_local(request, restricted_root)?))
}

pub(crate) async fn remote_sidecar_file_picker_read_files(
    State(state): State<crate::remote_workspace::RemoteSidecarState>,
    Json(request): Json<FilePickerReadFilesRequest>,
) -> Result<Json<FilePickerReadFilesResponse>, ApiError> {
    let root = fs::canonicalize(&state.workspace_path).map_err(|source| {
        ApiError::bad_request(format!("remote workspace root is not readable: {source}"))
    })?;
    let restricted_root = if request.allow_outside_workspace {
        None
    } else {
        Some(root.as_path())
    };
    let paths = request
        .paths
        .iter()
        .map(|path| canonical_path_within(Path::new(path), restricted_root))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(FilePickerReadFilesResponse {
        files: selected_files_from_paths(paths)?,
    }))
}

async fn list_workspace(
    state: &AppState,
    workspace_id: &str,
    request: FilePickerListRequest,
) -> Result<Json<FilePickerListResponse>, ApiError> {
    let config = config_snapshot(state)?;
    let workspace = config
        .workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| ApiError::bad_request(format!("workspace not found: {workspace_id}")))?;
    match &workspace.location {
        WorkspaceLocation::Local => {
            let root = fs::canonicalize(&workspace.path).map_err(|source| {
                ApiError::bad_request(format!("workspace path is not readable: {source}"))
            })?;
            let restricted_root = if request.allow_outside_workspace {
                None
            } else {
                Some(root.as_path())
            };
            // Unrestricted attachments: start at workspace root when path is empty,
            // but do not pass a restricted root so parentPath can navigate upward.
            let request = rewrite_unrestricted_empty_path_to_workspace_root(request, &root);
            Ok(Json(list_local(request, restricted_root)?))
        }
        WorkspaceLocation::Ssh { remote_path, .. } => {
            crate::remote_workspace::ensure_remote_workspace_connected(state, workspace_id).await?;
            let request = FilePickerListRequest {
                target: FilePickerTarget::Workspace {
                    workspace_id: workspace_id.to_string(),
                },
                path: request.path.or_else(|| Some(remote_path.clone())),
                ..request
            };
            let value = crate::remote_workspace::proxy_sidecar_json_request(
                state,
                workspace_id,
                reqwest::Method::POST,
                "file-picker/list",
                Some(serde_json::to_value(request).map_err(|source| {
                    ApiError::internal(format!("failed to serialize file picker request: {source}"))
                })?),
            )
            .await?;
            serde_json::from_value(value).map(Json).map_err(|source| {
                ApiError::bad_gateway(format!("invalid sidecar file picker response: {source}"))
            })
        }
    }
}

async fn read_workspace_files(
    state: &AppState,
    workspace_id: &str,
    request: FilePickerReadFilesRequest,
) -> Result<Json<FilePickerReadFilesResponse>, ApiError> {
    let config = config_snapshot(state)?;
    let workspace = config
        .workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| ApiError::bad_request(format!("workspace not found: {workspace_id}")))?;
    match &workspace.location {
        WorkspaceLocation::Local => {
            let root = fs::canonicalize(&workspace.path).map_err(|source| {
                ApiError::bad_request(format!("workspace path is not readable: {source}"))
            })?;
            let restricted_root = if request.allow_outside_workspace {
                None
            } else {
                Some(root.as_path())
            };
            let paths = request
                .paths
                .iter()
                .map(|path| canonical_path_within(Path::new(path), restricted_root))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Json(FilePickerReadFilesResponse {
                files: selected_files_from_paths(paths)?,
            }))
        }
        WorkspaceLocation::Ssh { .. } => {
            crate::remote_workspace::ensure_remote_workspace_connected(state, workspace_id).await?;
            let value = crate::remote_workspace::proxy_sidecar_json_request(
                state,
                workspace_id,
                reqwest::Method::POST,
                "file-picker/read-files",
                Some(serde_json::to_value(request).map_err(|source| {
                    ApiError::internal(format!("failed to serialize file picker request: {source}"))
                })?),
            )
            .await?;
            serde_json::from_value(value).map(Json).map_err(|source| {
                ApiError::bad_gateway(format!("invalid sidecar file picker response: {source}"))
            })
        }
    }
}

async fn list_remote_server(
    state: &AppState,
    server_id: &str,
    request: FilePickerListRequest,
) -> Result<Json<FilePickerListResponse>, ApiError> {
    let value = crate::remote_workspace::run_remote_file_picker_command(
        state,
        server_id,
        FILE_PICKER_LIST_COMMAND,
        serde_json::to_value(request).map_err(|source| {
            ApiError::internal(format!("failed to serialize file picker request: {source}"))
        })?,
    )
    .await?;
    serde_json::from_value(value).map(Json).map_err(|source| {
        ApiError::bad_gateway(format!("invalid remote file picker response: {source}"))
    })
}

async fn read_remote_server_files(
    state: &AppState,
    server_id: &str,
    request: FilePickerReadFilesRequest,
) -> Result<Json<FilePickerReadFilesResponse>, ApiError> {
    let value = crate::remote_workspace::run_remote_file_picker_command(
        state,
        server_id,
        FILE_PICKER_READ_FILES_COMMAND,
        serde_json::to_value(request).map_err(|source| {
            ApiError::internal(format!("failed to serialize file picker request: {source}"))
        })?,
    )
    .await?;
    serde_json::from_value(value).map(Json).map_err(|source| {
        ApiError::bad_gateway(format!("invalid remote file picker response: {source}"))
    })
}

fn list_local(
    request: FilePickerListRequest,
    root: Option<&Path>,
) -> Result<FilePickerListResponse, ApiError> {
    let limit = request
        .limit
        .unwrap_or(DEFAULT_LIST_LIMIT)
        .clamp(1, MAX_LIST_LIMIT);
    let path = resolve_list_path(request.path.as_deref(), root);
    let path = canonical_path_within(&path, root)?;
    let metadata = fs::metadata(&path)
        .map_err(|source| ApiError::bad_request(format!("path is not readable: {source}")))?;
    if !metadata.is_dir() {
        return Err(ApiError::bad_request(format!(
            "path must be a directory: {}",
            path.display()
        )));
    }

    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    for entry in fs::read_dir(&path)
        .map_err(|source| ApiError::bad_request(format!("failed to read directory: {source}")))?
    {
        match entry {
            Ok(entry) => {
                let name = entry.file_name().to_string_lossy().to_string();
                if !request.show_hidden && name.starts_with('.') {
                    continue;
                }
                let entry_path = entry.path();
                match entry.metadata() {
                    Ok(metadata) => {
                        let is_directory = metadata.is_dir();
                        if !is_directory
                            && (!request.include_files
                                || matches!(request.mode, FilePickerMode::Directory))
                        {
                            continue;
                        }
                        entries.push(FilePickerEntry {
                            name,
                            path: entry_path.display().to_string(),
                            is_directory,
                            size_bytes: metadata.is_file().then_some(metadata.len()),
                            modified_ms: metadata.modified().ok().and_then(|time| {
                                time.duration_since(UNIX_EPOCH)
                                    .ok()
                                    .map(|duration| duration.as_millis() as i64)
                            }),
                            disabled: false,
                        });
                    }
                    Err(source) => warnings.push(format!(
                        "skipped unreadable entry {}: {source}",
                        entry_path.display()
                    )),
                }
            }
            Err(source) => warnings.push(format!("skipped unreadable directory entry: {source}")),
        }
    }

    entries.sort_by(
        |left, right| match (left.is_directory, right.is_directory) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
        },
    );
    let truncated = entries.len() > limit;
    entries.truncate(limit);
    let parent_path = parent_path_within(&path, root);

    Ok(FilePickerListResponse {
        path: path.display().to_string(),
        parent_path,
        entries,
        truncated,
        warnings,
    })
}

/// Parent directory for navigation, clamped so restricted pickers never leave `root`.
fn parent_path_within(path: &Path, root: Option<&Path>) -> Option<String> {
    let parent = path.parent()?;
    if let Some(root) = root {
        if !parent.starts_with(root) {
            return None;
        }
    }
    Some(parent.display().to_string())
}

fn selected_files_from_paths(paths: Vec<PathBuf>) -> Result<Vec<NativeSelectedFile>, ApiError> {
    if paths.len() > MAX_CHAT_ATTACHMENTS {
        return Err(ApiError::bad_request(format!(
            "at most {MAX_CHAT_ATTACHMENTS} attachments are allowed"
        )));
    }

    let mut files = Vec::with_capacity(paths.len());
    let mut total_size = 0_u64;
    for path in paths {
        let path = fs::canonicalize(&path).map_err(|source| {
            ApiError::bad_request(format!("selected file is not readable: {source}"))
        })?;
        let metadata = fs::metadata(&path).map_err(|source| {
            ApiError::bad_request(format!("selected file is not readable: {source}"))
        })?;
        if !metadata.is_file() {
            return Err(ApiError::bad_request(format!(
                "selected attachment path must be a file: {}",
                path.display()
            )));
        }

        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ApiError::bad_request(format!("selected file has no name: {}", path.display()))
            })?;
        let size_bytes = metadata.len();
        if size_bytes > MAX_CHAT_ATTACHMENT_BYTES {
            return Err(ApiError::bad_request(format!(
                "attachment {name} exceeds the {} byte limit",
                MAX_CHAT_ATTACHMENT_BYTES
            )));
        }
        total_size = total_size
            .checked_add(size_bytes)
            .ok_or_else(|| ApiError::bad_request("attachment total size exceeds u64"))?;
        if total_size > MAX_CHAT_ATTACHMENT_TOTAL_BYTES {
            return Err(ApiError::bad_request(format!(
                "attachments exceed the {} byte total limit",
                MAX_CHAT_ATTACHMENT_TOTAL_BYTES
            )));
        }

        let content_type = attachment_content_type_for_path(&path);
        let content_base64 = if content_type.starts_with("image/") {
            Some(
                general_purpose::STANDARD.encode(fs::read(&path).map_err(|source| {
                    ApiError::bad_request(format!("failed to read selected image {name}: {source}"))
                })?),
            )
        } else {
            None
        };
        files.push(NativeSelectedFile {
            path: path.display().to_string(),
            name,
            content_type,
            size_bytes,
            content_base64,
        });
    }
    Ok(files)
}

fn canonical_path_within(path: &Path, root: Option<&Path>) -> Result<PathBuf, ApiError> {
    if root.is_none()
        && path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ApiError::bad_request("path must not contain '..' segments"));
    }
    let canonical = fs::canonicalize(path).map_err(|source| {
        ApiError::bad_request(format!(
            "path is not readable: {}: {source}",
            path.display()
        ))
    })?;
    if let Some(root) = root {
        if !canonical.starts_with(root) {
            return Err(ApiError::forbidden(format!(
                "path is outside the allowed root: {}",
                root.display()
            )));
        }
    }
    Ok(canonical)
}

fn local_roots_response() -> FilePickerRootsResponse {
    let mut roots = Vec::new();
    roots.push(FilePickerRoot {
        label: "Home".to_string(),
        path: default_local_path().display().to_string(),
    });
    if let Some(home) = home_dir() {
        for name in ["Desktop", "Documents", "Downloads"] {
            let path = home.join(name);
            if path.is_dir() {
                roots.push(FilePickerRoot {
                    label: name.to_string(),
                    path: path.display().to_string(),
                });
            }
        }
    }
    #[cfg(unix)]
    roots.push(FilePickerRoot {
        label: "/".to_string(),
        path: "/".to_string(),
    });
    roots.dedup_by(|left, right| left.path == right.path);
    FilePickerRootsResponse { roots }
}

fn default_local_path() -> PathBuf {
    home_dir().unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Resolve the initial list path before canonicalization.
///
/// Empty/blank path: restricted pickers open `root`; unrestricted pickers open home.
/// Unrestricted pickers only: expand strict `~` and `~/...` via the process home.
/// Restricted root mode leaves `~` literal so it cannot escape the workspace boundary.
fn resolve_list_path(path: Option<&str>, root: Option<&Path>) -> PathBuf {
    let trimmed = path.map(str::trim).filter(|value| !value.is_empty());
    match trimmed {
        None => match root {
            Some(root) => root.to_path_buf(),
            None => default_local_path(),
        },
        Some(value) if root.is_none() && is_home_shorthand(value) => expand_home_shorthand(value),
        Some(value) => PathBuf::from(value),
    }
}

/// Strict home shorthand: `~` or `~/...` only - not `~user`.
fn is_home_shorthand(value: &str) -> bool {
    value == "~" || value.starts_with("~/")
}

fn expand_home_shorthand(value: &str) -> PathBuf {
    let home = default_local_path();
    if value == "~" {
        return home;
    }
    // `~/rest` - join the remainder under home (leading `/` already stripped by prefix).
    home.join(&value[2..])
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

fn path_is_empty(path: Option<&str>) -> bool {
    path.map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
}

/// When unrestricted attachment browsing is on and the client sent empty path,
/// open at the workspace root instead of process home (still without a restricted root).
fn rewrite_unrestricted_empty_path_to_workspace_root(
    request: FilePickerListRequest,
    workspace_root: &Path,
) -> FilePickerListRequest {
    if request.allow_outside_workspace && path_is_empty(request.path.as_deref()) {
        FilePickerListRequest {
            path: Some(workspace_root.display().to_string()),
            ..request
        }
    } else {
        request
    }
}

fn read_stdin_json<T: for<'de> Deserialize<'de>>() -> AppResult<T> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    Ok(serde_json::from_str(&input)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_files_rejects_more_than_attachment_limit() {
        let paths = (0..=MAX_CHAT_ATTACHMENTS)
            .map(|index| PathBuf::from(format!("/tmp/file-{index}")))
            .collect();
        assert!(selected_files_from_paths(paths).is_err());
    }

    #[test]
    fn empty_local_path_uses_default_directory() {
        let default_path = fs::canonicalize(default_local_path()).unwrap();
        let response = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Local,
                path: Some("".to_string()),
                mode: FilePickerMode::Directory,
                include_files: false,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            None,
        )
        .unwrap();

        assert_eq!(response.path, default_path.display().to_string());
    }

    #[test]
    fn empty_path_with_restricted_root_opens_root() {
        let root = temp_picker_dir("restricted-empty");
        fs::create_dir_all(root.join("folder")).unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();

        let response = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Workspace {
                    workspace_id: "workspace-1".to_string(),
                },
                path: Some("".to_string()),
                mode: FilePickerMode::File,
                include_files: true,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            Some(&canonical_root),
        )
        .unwrap();

        let _ = fs::remove_dir_all(&root);
        assert_eq!(response.path, canonical_root.display().to_string());
        assert!(response.parent_path.is_none());
        assert!(
            response
                .entries
                .iter()
                .any(|entry| entry.name == "folder" && entry.is_directory)
        );
    }

    #[test]
    fn whitespace_path_with_restricted_root_opens_root() {
        let root = temp_picker_dir("restricted-whitespace");
        let canonical_root = fs::canonicalize(&root).unwrap();

        let response = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Workspace {
                    workspace_id: "workspace-1".to_string(),
                },
                path: Some("   ".to_string()),
                mode: FilePickerMode::Directory,
                include_files: false,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            Some(&canonical_root),
        )
        .unwrap();

        let _ = fs::remove_dir_all(&root);
        assert_eq!(response.path, canonical_root.display().to_string());
        assert!(response.parent_path.is_none());
    }

    #[test]
    fn restricted_root_parent_path_clamped_to_root() {
        let root = temp_picker_dir("restricted-parent");
        let child = root.join("child");
        fs::create_dir_all(&child).unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();
        let canonical_child = fs::canonicalize(&child).unwrap();

        let at_root = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Workspace {
                    workspace_id: "workspace-1".to_string(),
                },
                path: Some(canonical_root.display().to_string()),
                mode: FilePickerMode::Directory,
                include_files: false,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            Some(&canonical_root),
        )
        .unwrap();
        assert_eq!(at_root.path, canonical_root.display().to_string());
        assert!(at_root.parent_path.is_none());

        let at_child = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Workspace {
                    workspace_id: "workspace-1".to_string(),
                },
                path: Some(canonical_child.display().to_string()),
                mode: FilePickerMode::Directory,
                include_files: false,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            Some(&canonical_root),
        )
        .unwrap();
        assert_eq!(at_child.path, canonical_child.display().to_string());
        assert_eq!(
            at_child.parent_path,
            Some(canonical_root.display().to_string())
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn restricted_root_rejects_path_outside_root() {
        let root = temp_picker_dir("restricted-outside");
        let outside = temp_picker_dir("restricted-outside-peer");
        let canonical_root = fs::canonicalize(&root).unwrap();

        let error = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Workspace {
                    workspace_id: "workspace-1".to_string(),
                },
                path: Some(outside.display().to_string()),
                mode: FilePickerMode::Directory,
                include_files: false,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            Some(&canonical_root),
        )
        .unwrap_err();

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
        assert_eq!(error.status(), axum::http::StatusCode::FORBIDDEN);
        assert!(error.message().contains("path is outside the allowed root"));
    }

    #[test]
    fn unrestricted_workspace_list_can_navigate_above_root() {
        let root = temp_picker_dir("unrestricted-up");
        let outside = temp_picker_dir("unrestricted-up-peer");
        fs::write(outside.join("secret.txt"), b"secret").unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();
        let canonical_outside = fs::canonicalize(&outside).unwrap();
        let parent = canonical_root.parent().expect("temp root has parent");

        // Empty path under unrestricted workspace mode is rewritten to workspace root
        // (same as list_workspace / remote_sidecar), then listed without restricted root.
        let request = FilePickerListRequest {
            target: FilePickerTarget::Workspace {
                workspace_id: "workspace-1".to_string(),
            },
            path: Some(canonical_root.display().to_string()),
            mode: FilePickerMode::File,
            include_files: true,
            show_hidden: false,
            limit: None,
            allow_outside_workspace: true,
        };
        let at_root = list_local(request, None).unwrap();
        assert_eq!(at_root.path, canonical_root.display().to_string());
        assert_eq!(
            at_root.parent_path,
            Some(parent.display().to_string()),
            "unrestricted picker must expose parent above workspace root"
        );

        let at_outside = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Workspace {
                    workspace_id: "workspace-1".to_string(),
                },
                path: Some(canonical_outside.display().to_string()),
                mode: FilePickerMode::File,
                include_files: true,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: true,
            },
            None,
        )
        .unwrap();
        assert_eq!(at_outside.path, canonical_outside.display().to_string());
        assert!(
            at_outside
                .entries
                .iter()
                .any(|entry| entry.name == "secret.txt" && !entry.is_directory)
        );

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn unrestricted_workspace_read_allows_file_outside_root() {
        let root = temp_picker_dir("unrestricted-read-root");
        let outside = temp_picker_dir("unrestricted-read-peer");
        let file_path = outside.join("note.txt");
        fs::write(&file_path, b"hello").unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();
        let canonical_file = fs::canonicalize(&file_path).unwrap();

        // Restricted: outside file forbidden.
        let restricted_err =
            canonical_path_within(&canonical_file, Some(&canonical_root)).unwrap_err();
        assert_eq!(restricted_err.status(), axum::http::StatusCode::FORBIDDEN);

        // Unrestricted (allowOutsideWorkspace): same path allowed, then attachment validation.
        let unrestricted = canonical_path_within(&canonical_file, None).unwrap();
        let files = selected_files_from_paths(vec![unrestricted]).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "note.txt");
        assert_eq!(files[0].size_bytes, 5);

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn unrestricted_empty_path_rewritten_to_workspace_root() {
        let root = temp_picker_dir("unrestricted-empty-initial");
        fs::create_dir_all(root.join("inside")).unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();

        for empty in [None, Some(String::new()), Some("   ".to_string())] {
            let rewritten = rewrite_unrestricted_empty_path_to_workspace_root(
                FilePickerListRequest {
                    target: FilePickerTarget::Workspace {
                        workspace_id: "workspace-1".to_string(),
                    },
                    path: empty,
                    mode: FilePickerMode::File,
                    include_files: true,
                    show_hidden: false,
                    limit: None,
                    allow_outside_workspace: true,
                },
                &canonical_root,
            );
            let expected_root = canonical_root.display().to_string();
            assert_eq!(rewritten.path.as_deref(), Some(expected_root.as_str()));

            let response = list_local(rewritten, None).unwrap();
            assert_eq!(response.path, expected_root);
            assert!(response.parent_path.is_some());
            assert!(
                response
                    .entries
                    .iter()
                    .any(|entry| entry.name == "inside" && entry.is_directory)
            );
        }

        // Restricted or non-empty path must not be rewritten.
        let kept = rewrite_unrestricted_empty_path_to_workspace_root(
            FilePickerListRequest {
                target: FilePickerTarget::Workspace {
                    workspace_id: "workspace-1".to_string(),
                },
                path: None,
                mode: FilePickerMode::File,
                include_files: true,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            &canonical_root,
        );
        assert!(kept.path.is_none());

        let kept_path = rewrite_unrestricted_empty_path_to_workspace_root(
            FilePickerListRequest {
                target: FilePickerTarget::Workspace {
                    workspace_id: "workspace-1".to_string(),
                },
                path: Some("/tmp/other".to_string()),
                mode: FilePickerMode::File,
                include_files: true,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: true,
            },
            &canonical_root,
        );
        assert_eq!(kept_path.path.as_deref(), Some("/tmp/other"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unrestricted_tilde_path_opens_home() {
        let home = default_local_path();
        let canonical_home = fs::canonicalize(&home).unwrap();

        let response = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Local,
                path: Some("~".to_string()),
                mode: FilePickerMode::Directory,
                include_files: false,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            None,
        )
        .unwrap();

        assert_eq!(response.path, canonical_home.display().to_string());
        assert!(!response.path.contains('~'));
    }

    #[test]
    fn unrestricted_tilde_subdir_expands_under_home() {
        let home = default_local_path();
        let subdir = temp_picker_dir("tilde-subdir");
        // Use a unique name under the real home so expansion hits an existing dir.
        let name = subdir
            .file_name()
            .expect("temp dir name")
            .to_string_lossy()
            .to_string();
        let under_home = home.join(&name);
        let _ = fs::remove_dir_all(&under_home);
        fs::create_dir_all(&under_home).unwrap();
        let canonical = fs::canonicalize(&under_home).unwrap();

        let response = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Local,
                path: Some(format!("~/{name}")),
                mode: FilePickerMode::Directory,
                include_files: false,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            None,
        )
        .unwrap();

        let _ = fs::remove_dir_all(&under_home);
        let _ = fs::remove_dir_all(&subdir);
        assert_eq!(response.path, canonical.display().to_string());
        assert!(!response.path.starts_with('~'));
    }

    #[test]
    fn restricted_root_does_not_expand_tilde_outside_workspace() {
        let root = temp_picker_dir("restricted-tilde");
        let canonical_root = fs::canonicalize(&root).unwrap();

        let error = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Workspace {
                    workspace_id: "workspace-1".to_string(),
                },
                path: Some("~".to_string()),
                mode: FilePickerMode::Directory,
                include_files: false,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            Some(&canonical_root),
        )
        .unwrap_err();

        let _ = fs::remove_dir_all(&root);
        // Must not open process home; literal `~` fails canonicalize or is outside root.
        assert!(
            error.status() == axum::http::StatusCode::BAD_REQUEST
                || error.status() == axum::http::StatusCode::FORBIDDEN
        );
        assert!(
            error.message().contains("path is not readable")
                || error.message().contains("path is outside the allowed root")
        );
    }

    #[test]
    fn restricted_root_rejects_tilde_subdir_escape() {
        let root = temp_picker_dir("restricted-tilde-subdir");
        let canonical_root = fs::canonicalize(&root).unwrap();

        let error = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Workspace {
                    workspace_id: "workspace-1".to_string(),
                },
                path: Some("~/somewhere".to_string()),
                mode: FilePickerMode::Directory,
                include_files: false,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            Some(&canonical_root),
        )
        .unwrap_err();

        let _ = fs::remove_dir_all(&root);
        assert!(
            error.status() == axum::http::StatusCode::BAD_REQUEST
                || error.status() == axum::http::StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn tilde_user_is_not_treated_as_home_shorthand() {
        // `~user` must stay literal (not expand), same as remote path rules.
        assert!(!is_home_shorthand("~user"));
        assert!(!is_home_shorthand("~user/path"));
        assert!(is_home_shorthand("~"));
        assert!(is_home_shorthand("~/a"));
    }

    #[test]
    fn selected_svg_file_includes_content_for_workspace_icon_uploads() {
        let root = temp_picker_dir("selected-svg");
        let svg_path = root.join("workspace-icon.svg");
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"/>"#;
        fs::write(&svg_path, svg).unwrap();

        let files = selected_files_from_paths(vec![svg_path]).unwrap();

        let _ = fs::remove_dir_all(&root);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content_type, "image/svg+xml");
        assert_eq!(
            files[0].content_base64.as_deref(),
            Some(general_purpose::STANDARD.encode(svg).as_str())
        );
    }

    fn temp_picker_dir(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "foco-file-picker-test-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn directory_listing_filters_files_for_directory_mode() {
        let root =
            std::env::temp_dir().join(format!("foco-file-picker-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("folder")).unwrap();
        fs::write(root.join("file.txt"), b"hello").unwrap();
        let response = list_local(
            FilePickerListRequest {
                target: FilePickerTarget::Local,
                path: Some(root.display().to_string()),
                mode: FilePickerMode::Directory,
                include_files: false,
                show_hidden: false,
                limit: None,
                allow_outside_workspace: false,
            },
            None,
        )
        .unwrap();
        let _ = fs::remove_dir_all(&root);
        assert_eq!(response.entries.len(), 1);
        assert_eq!(response.entries[0].name, "folder");
    }

    #[test]
    fn file_picker_target_deserializes_frontend_camel_case_ids() {
        let workspace: FilePickerTarget =
            serde_json::from_str(r#"{"kind":"workspace","workspaceId":"workspace-1"}"#).unwrap();
        match workspace {
            FilePickerTarget::Workspace { workspace_id } => {
                assert_eq!(workspace_id, "workspace-1");
            }
            other => panic!("expected workspace target, got {other:?}"),
        }

        let remote: FilePickerTarget =
            serde_json::from_str(r#"{"kind":"remoteServer","serverId":"server-1"}"#).unwrap();
        match remote {
            FilePickerTarget::RemoteServer { server_id } => {
                assert_eq!(server_id, "server-1");
            }
            other => panic!("expected remoteServer target, got {other:?}"),
        }

        let local: FilePickerTarget = serde_json::from_str(r#"{"kind":"local"}"#).unwrap();
        assert!(matches!(local, FilePickerTarget::Local));
    }

    #[test]
    fn file_picker_target_serializes_camel_case_ids() {
        let workspace = serde_json::to_value(FilePickerTarget::Workspace {
            workspace_id: "workspace-1".to_string(),
        })
        .unwrap();
        assert_eq!(
            workspace,
            serde_json::json!({
                "kind": "workspace",
                "workspaceId": "workspace-1",
            })
        );
        assert!(workspace.get("workspace_id").is_none());

        let remote = serde_json::to_value(FilePickerTarget::RemoteServer {
            server_id: "server-1".to_string(),
        })
        .unwrap();
        assert_eq!(
            remote,
            serde_json::json!({
                "kind": "remoteServer",
                "serverId": "server-1",
            })
        );
        assert!(remote.get("server_id").is_none());

        let local = serde_json::to_value(FilePickerTarget::Local).unwrap();
        assert_eq!(local, serde_json::json!({ "kind": "local" }));
    }

    #[test]
    fn file_picker_list_request_deserializes_workspace_target_from_frontend_json() {
        let request: FilePickerListRequest = serde_json::from_str(
            r#"{
                "target": {"kind":"workspace","workspaceId":"workspace-1"},
                "path": "",
                "mode": "file",
                "includeFiles": true,
                "showHidden": false,
                "limit": 500
            }"#,
        )
        .unwrap();

        match request.target {
            FilePickerTarget::Workspace { workspace_id } => {
                assert_eq!(workspace_id, "workspace-1");
            }
            other => panic!("expected workspace target, got {other:?}"),
        }
        assert!(matches!(request.mode, FilePickerMode::File));
        assert!(request.include_files);
        assert!(!request.show_hidden);
        assert_eq!(request.limit, Some(500));
        assert!(!request.allow_outside_workspace);
    }

    #[test]
    fn file_picker_list_request_deserializes_allow_outside_workspace() {
        let request: FilePickerListRequest = serde_json::from_str(
            r#"{
                "target": {"kind":"workspace","workspaceId":"workspace-1"},
                "path": "",
                "mode": "file",
                "includeFiles": true,
                "showHidden": false,
                "limit": 500,
                "allowOutsideWorkspace": true
            }"#,
        )
        .unwrap();
        assert!(request.allow_outside_workspace);

        let read_request: FilePickerReadFilesRequest = serde_json::from_str(
            r#"{
                "target": {"kind":"workspace","workspaceId":"workspace-1"},
                "paths": ["/tmp/a.txt"],
                "allowOutsideWorkspace": true
            }"#,
        )
        .unwrap();
        assert!(read_request.allow_outside_workspace);
        assert_eq!(read_request.paths, vec!["/tmp/a.txt"]);
    }
}
