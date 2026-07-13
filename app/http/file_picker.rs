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
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePickerReadFilesRequest {
    target: FilePickerTarget,
    paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
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
    Ok(Json(list_local(request, Some(&root))?))
}

pub(crate) async fn remote_sidecar_file_picker_read_files(
    State(state): State<crate::remote_workspace::RemoteSidecarState>,
    Json(request): Json<FilePickerReadFilesRequest>,
) -> Result<Json<FilePickerReadFilesResponse>, ApiError> {
    let root = fs::canonicalize(&state.workspace_path).map_err(|source| {
        ApiError::bad_request(format!("remote workspace root is not readable: {source}"))
    })?;
    let paths = request
        .paths
        .iter()
        .map(|path| canonical_path_within(Path::new(path), Some(&root)))
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
            Ok(Json(list_local(request, Some(&root))?))
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
            let paths = request
                .paths
                .iter()
                .map(|path| canonical_path_within(Path::new(path), Some(&root)))
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
    let path = request
        .path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_local_path);
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
    let parent_path = path.parent().map(|parent| parent.display().to_string());

    Ok(FilePickerListResponse {
        path: path.display().to_string(),
        parent_path,
        entries,
        truncated,
        warnings,
    })
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

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
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
            },
            None,
        )
        .unwrap();

        assert_eq!(response.path, default_path.display().to_string());
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
            },
            None,
        )
        .unwrap();
        let _ = fs::remove_dir_all(&root);
        assert_eq!(response.entries.len(), 1);
        assert_eq!(response.entries[0].name, "folder");
    }
}
