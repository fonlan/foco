use std::{fs, path::Path, time::Instant};

const WORKSPACE_FILE_TREE_MAX_DEPTH: usize = 12;
const WORKSPACE_FILE_TREE_MAX_NODES: usize = 8_000;
const WORKSPACE_FILE_TREE_INITIAL_DEPTH: usize = 2;

use axum::{
    Json,
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use foco_store::workspace::WorkspaceDatabase;
use foco_tools::set_ripgrep_path;

use crate::http::settings::SettingsResponse;
use crate::runtime::download_and_install_ripgrep;
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspacePathRequest {
    pub(crate) name: String,
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) content_base64: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualWorkspaceRequest {
    id: String,
    name: String,
    path: String,
    pinned: bool,
    terminal_shell: String,
    #[serde(default)]
    common_commands: Vec<WorkspaceCommonCommandRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceCommonCommandRequest {
    pub(crate) name: String,
    pub(crate) command: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceLogoRequest {
    pub(crate) content_base64: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceOrderRequest {
    workspace_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceChatSearchQuery {
    query: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceFileRequest {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceFileChildrenQuery {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceFileBlobQuery {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveWorkspaceFileRequest {
    path: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenameWorkspaceFileRequest {
    path: String,
    new_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceFileSaveResponse {
    content: String,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceFileContentResponse {
    content: String,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceFilesResponse {
    root: WorkspaceFileTreeNode,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceFileChildrenResponse {
    path: String,
    children: Vec<WorkspaceFileTreeNode>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceFileTreeNode {
    name: String,
    path: String,
    kind: WorkspaceFileTreeNodeKind,
    size_bytes: u64,
    has_children: bool,
    children_loaded: bool,
    children: Vec<WorkspaceFileTreeNode>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum WorkspaceFileTreeNodeKind {
    Directory,
    File,
}
pub(crate) async fn workspace_files(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Json<WorkspaceFilesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    Ok(Json(WorkspaceFilesResponse {
        root: workspace_file_tree_response(&workspace.path)?,
    }))
}

pub(crate) async fn workspace_file_children(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Query(query): Query<WorkspaceFileChildrenQuery>,
) -> Result<Json<WorkspaceFileChildrenResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = workspace_file_list_path(&workspace.path, &query.path)?;
    let metadata = fs::metadata(&path).map_err(|source| {
        ApiError::bad_request(format!(
            "workspace file was not found: {}: {}",
            query.path, source
        ))
    })?;

    if !metadata.is_dir() {
        return Err(ApiError::bad_request(format!(
            "workspace path is not a directory: {}",
            query.path
        )));
    }

    Ok(Json(WorkspaceFileChildrenResponse {
        path: query.path,
        children: workspace_file_tree_children(&workspace.path, &path, 0, false)?,
    }))
}

pub(crate) async fn delete_workspace_file(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<WorkspaceFileRequest>,
) -> Result<Json<WorkspaceFileChildrenResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = workspace_file_path(&workspace.path, &request.path)?;
    let metadata = fs::metadata(&path).map_err(|source| {
        ApiError::bad_request(format!(
            "workspace file was not found: {}: {}",
            request.path, source
        ))
    })?;

    if metadata.is_dir() {
        fs::remove_dir_all(&path).map_err(|source| {
            ApiError::internal(format!(
                "failed to delete workspace directory {}: {}",
                request.path, source
            ))
        })?;
    } else {
        fs::remove_file(&path).map_err(|source| {
            ApiError::internal(format!(
                "failed to delete workspace file {}: {}",
                request.path, source
            ))
        })?;
    }

    let parent = workspace_file_parent_path(&request.path);
    let parent_path = workspace_file_list_path(&workspace.path, &parent)?;

    Ok(Json(WorkspaceFileChildrenResponse {
        path: parent,
        children: workspace_file_tree_children(&workspace.path, &parent_path, 0, false)?,
    }))
}

pub(crate) async fn workspace_file_content(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<WorkspaceFileRequest>,
) -> Result<Json<WorkspaceFileContentResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = workspace_file_path(&workspace.path, &request.path)?;
    let metadata = fs::metadata(&path).map_err(|source| {
        ApiError::bad_request(format!(
            "workspace file was not found: {}: {}",
            request.path, source
        ))
    })?;

    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "workspace path is not a file: {}",
            request.path
        )));
    }

    let content = fs::read_to_string(&path).map_err(|source| {
        ApiError::bad_request(format!(
            "failed to read workspace file {} as UTF-8 text: {}",
            request.path, source
        ))
    })?;

    Ok(Json(WorkspaceFileContentResponse {
        content,
        path: request.path,
    }))
}

pub(crate) async fn workspace_file_blob(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Query(query): Query<WorkspaceFileBlobQuery>,
) -> Result<Response, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = workspace_file_path(&workspace.path, &query.path)?;
    let metadata = fs::metadata(&path).map_err(|source| {
        ApiError::bad_request(format!(
            "workspace file was not found: {}: {}",
            query.path, source
        ))
    })?;

    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "workspace path is not a file: {}",
            query.path
        )));
    }

    let bytes = fs::read(&path).map_err(|source| {
        ApiError::bad_request(format!(
            "failed to read workspace file {}: {}",
            query.path, source
        ))
    })?;
    let content_type = workspace_image_content_type(&bytes)?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "private, max-age=60")
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; img-src data:; style-src 'unsafe-inline'",
        )
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Body::from(bytes))
        .expect("workspace file blob response is valid"))
}

pub(crate) async fn save_workspace_file(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<SaveWorkspaceFileRequest>,
) -> Result<Json<WorkspaceFileSaveResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = workspace_file_path(&workspace.path, &request.path)?;
    let metadata = fs::metadata(&path).map_err(|source| {
        ApiError::bad_request(format!(
            "workspace file was not found: {}: {}",
            request.path, source
        ))
    })?;

    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "workspace path is not a file: {}",
            request.path
        )));
    }

    fs::write(&path, request.content.as_bytes()).map_err(|source| {
        ApiError::internal(format!(
            "failed to save workspace file {}: {}",
            request.path, source
        ))
    })?;

    Ok(Json(WorkspaceFileSaveResponse {
        content: request.content,
        path: request.path,
    }))
}

pub(crate) async fn rename_workspace_file(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<RenameWorkspaceFileRequest>,
) -> Result<Json<WorkspaceFileChildrenResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let source_path = workspace_file_path(&workspace.path, &request.path)?;
    let new_name = validate_workspace_file_name(&request.new_name)?;
    let target_path = source_path
        .parent()
        .ok_or_else(|| ApiError::bad_request("workspace root cannot be renamed"))?
        .join(new_name);

    if target_path.exists() {
        return Err(ApiError::bad_request(format!(
            "workspace file already exists: {}",
            target_path.display()
        )));
    }

    fs::rename(&source_path, &target_path).map_err(|source| {
        ApiError::internal(format!(
            "failed to rename workspace file {}: {}",
            request.path, source
        ))
    })?;

    let parent = workspace_file_parent_path(&request.path);
    let parent_path = workspace_file_list_path(&workspace.path, &parent)?;

    Ok(Json(WorkspaceFileChildrenResponse {
        path: parent,
        children: workspace_file_tree_children(&workspace.path, &parent_path, 0, false)?,
    }))
}

pub(crate) async fn workspaces(
    State(state): State<AppState>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let started_at = Instant::now();
    tracing::info!("workspaces API request started");
    let config_started_at = Instant::now();
    let config = config_snapshot(&state)?;
    tracing::info!(
        elapsed_ms = config_started_at.elapsed().as_millis() as u64,
        workspace_count = config.workspaces.len(),
        active_workspace_id = %config.app.active_workspace_id,
        "workspaces API config snapshot loaded"
    );

    let response = workspace_response_from_config(&config, &state.active_chat_runs)?;
    tracing::info!(
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        "workspaces API request completed"
    );
    Ok(response)
}

pub(crate) async fn search_workspace_chats(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceChatSearchQuery>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let needle = query.query.trim().to_lowercase();

    if needle.is_empty() {
        return Ok(Json(WorkspacesResponse {
            active_workspace_id: config.app.active_workspace_id,
            workspaces: Vec::new(),
        }));
    }

    let mut workspaces = Vec::new();

    for workspace in &config.workspaces {
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        let matched_chats = database
            .chats()
            .map_err(ApiError::from_workspace_error)?
            .into_iter()
            .filter(|chat| chat.title.to_lowercase().contains(&needle))
            .collect::<Vec<_>>();

        if matched_chats.is_empty() {
            continue;
        }

        let code_change_stats_by_chat = database
            .chat_code_change_stats()
            .map_err(ApiError::from_workspace_error)?;
        let chats = matched_chats
            .into_iter()
            .map(|chat| {
                let active_run = state
                    .active_chat_runs
                    .active_run_for_chat(&workspace.id, &chat.id)?;
                let code_change_stats = code_change_stats_by_chat
                    .get(&chat.id)
                    .cloned()
                    .unwrap_or_default();
                chat_summary(&mut database, chat, code_change_stats, active_run)
            })
            .collect::<Result<Vec<_>, ApiError>>()?;

        workspaces.push(WorkspaceSummary {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
            path: display_path(&workspace.path),
            logo_url: workspace_logo_url(workspace)?,
            pinned: workspace.pinned,
            terminal_shell: workspace.terminal_shell.clone(),
            common_commands: settings_runtime::workspace_common_command_summaries(
                &workspace.common_commands,
            ),
            chats,
        });
    }

    Ok(Json(WorkspacesResponse {
        active_workspace_id: config.app.active_workspace_id,
        workspaces,
    }))
}

pub(crate) async fn add_workspace(
    State(state): State<AppState>,
    Json(request): Json<WorkspacePathRequest>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let logo_content_base64 = request.content_base64.clone();
    let logo = if logo_content_base64.is_some() {
        let bytes = workspace_logo_request_bytes(&WorkspaceLogoRequest {
            content_base64: logo_content_base64,
        })?;
        let kind = workspace_logo_kind(&bytes)?;
        Some((bytes, kind))
    } else {
        None
    };
    let (name, requested_path) = validate_workspace_request(request)?;

    if requested_path.exists() && !requested_path.is_dir() {
        return Err(ApiError::bad_request(format!(
            "workspace path exists but is not a directory: {}",
            requested_path.display()
        )));
    }

    if !requested_path.exists() {
        fs::create_dir_all(&requested_path).map_err(|source| {
            ApiError::internal(format!(
                "failed to create workspace directory {}: {}",
                requested_path.display(),
                source
            ))
        })?;
    }

    let path = canonical_workspace_path(&requested_path)?;
    let mut config = config_snapshot(&state)?;

    reject_registered_workspace_path(&config, &path, None)?;
    WorkspaceDatabase::open_or_create(&path).map_err(ApiError::from_workspace_error)?;

    if let Some((bytes, kind)) = logo {
        save_workspace_logo_file(&path, &bytes, kind)?;
    }

    let id = unique_workspace_id(&config, &name);
    config.workspaces.insert(
        0,
        WorkspaceConfig {
            id,
            name,
            path,
            pinned: false,
            terminal_shell: default_terminal_shell_for_current_platform().to_string(),
            common_commands: Vec::new(),
        },
    );
    save_config(&state, config.clone())?;
    sync_all_mcp_workspaces(&state.mcp_registry, &config)
        .await
        .map_err(ApiError::from_mcp_error)?;

    workspace_response_from_config(&config, &state.active_chat_runs)
}

pub(crate) async fn save_workspace_settings(
    State(state): State<AppState>,
    Json(request): Json<ManualWorkspaceRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let workspace_id = request.id.trim();
    let name = request.name.trim();
    let requested_path = validate_workspace_path(&request.path)?;
    let terminal_shell = normalize_terminal_shell(&request.terminal_shell)?;
    let common_commands = normalize_workspace_common_commands(&request.common_commands)?;

    if workspace_id.is_empty() {
        return Err(ApiError::bad_request("workspace id must not be empty"));
    }

    if name.is_empty() {
        return Err(ApiError::bad_request("workspace name must not be empty"));
    }

    if requested_path.exists() && !requested_path.is_dir() {
        return Err(ApiError::bad_request(format!(
            "workspace path exists but is not a directory: {}",
            requested_path.display()
        )));
    }

    if !requested_path.exists() {
        fs::create_dir_all(&requested_path).map_err(|source| {
            ApiError::internal(format!(
                "failed to create workspace directory {}: {}",
                requested_path.display(),
                source
            ))
        })?;
    }

    let path = canonical_workspace_path(&requested_path)?;
    reject_registered_workspace_path(&config, &path, Some(workspace_id))?;

    let workspace = config
        .workspaces
        .iter_mut()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| ApiError::bad_request(format!("workspace was not found: {workspace_id}")))?;

    workspace.name = name.to_string();
    workspace.path = path;
    workspace.pinned = request.pinned;
    workspace.terminal_shell = terminal_shell;
    workspace.common_commands = common_commands;
    group_pinned_workspaces(&mut config.workspaces);

    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

pub(crate) async fn save_workspace_order(
    State(state): State<AppState>,
    Json(request): Json<WorkspaceOrderRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;

    reorder_workspaces(&mut config.workspaces, request.workspace_ids)?;
    group_pinned_workspaces(&mut config.workspaces);
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

pub(crate) async fn workspace_logo(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Response, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let Some(logo) = workspace_logo_file(&workspace.path)? else {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("workspace logo was not found"))
            .expect("workspace logo response is valid"));
    };
    let (bytes, _) = read_workspace_logo_file(&logo.path)?;
    let kind = workspace_logo_kind(&bytes)?;
    if kind != logo.kind {
        return Err(ApiError::bad_request(format!(
            "workspace logo changed while it was being read: {}",
            logo.path.display()
        )));
    }

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, kind.content_type)
        .header(header::CACHE_CONTROL, "private, max-age=60")
        .body(Body::from(bytes))
        .expect("workspace logo response is valid"))
}

pub(crate) async fn save_workspace_logo(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<WorkspaceLogoRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let bytes = workspace_logo_request_bytes(&request)?;
    let kind = workspace_logo_kind(&bytes)?;

    save_workspace_logo_file(&workspace.path, &bytes, kind)?;

    settings_response(&state, &config).await
}

pub(crate) async fn clear_workspace_logo(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    clear_workspace_logo_file(&workspace.path)?;

    settings_response(&state, &config).await
}

pub(crate) async fn install_ripgrep(
    State(state): State<AppState>,
) -> Result<Json<InstallRipgrepResponse>, ApiError> {
    let _install_guard = state.ripgrep_install_lock.lock().await;
    let install_dir = {
        let status = state
            .ripgrep_status
            .lock()
            .map_err(|_| ApiError::internal("ripgrep status lock was poisoned"))?;
        status.install_dir.clone()
    };
    let status = download_and_install_ripgrep(&install_dir).await?;
    set_ripgrep_path(status.path.clone());
    {
        let mut current = state
            .ripgrep_status
            .lock()
            .map_err(|_| ApiError::internal("ripgrep status lock was poisoned"))?;
        *current = status.clone();
    }

    Ok(Json(InstallRipgrepResponse {
        ripgrep: ripgrep_tool_summary(&status),
    }))
}

fn workspace_file_tree_response(workspace_root: &Path) -> Result<WorkspaceFileTreeNode, ApiError> {
    workspace_file_tree(workspace_root, workspace_root, true, 1, true)
}

fn workspace_file_tree_children(
    workspace_root: &Path,
    path: &Path,
    depth: usize,
    load_grandchildren: bool,
) -> Result<Vec<WorkspaceFileTreeNode>, ApiError> {
    if depth > WORKSPACE_FILE_TREE_MAX_DEPTH {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(path)
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to read workspace directory {}: {}",
                path.display(),
                source
            ))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to read workspace directory entry {}: {}",
                path.display(),
                source
            ))
        })?;

    entries.sort_by(|left, right| {
        let left_path = left.path();
        let right_path = right.path();
        let left_is_dir = left_path.is_dir();
        let right_is_dir = right_path.is_dir();

        right_is_dir
            .cmp(&left_is_dir)
            .then_with(|| left.file_name().cmp(&right.file_name()))
    });

    let mut children = Vec::new();
    for entry in entries.into_iter().take(WORKSPACE_FILE_TREE_MAX_NODES) {
        children.push(workspace_file_tree(
            workspace_root,
            &entry.path(),
            false,
            depth + 1,
            load_grandchildren && depth + 1 < WORKSPACE_FILE_TREE_INITIAL_DEPTH,
        )?);
    }

    Ok(children)
}

fn workspace_file_tree(
    workspace_root: &Path,
    path: &Path,
    is_root: bool,
    depth: usize,
    load_children: bool,
) -> Result<WorkspaceFileTreeNode, ApiError> {
    let metadata = fs::metadata(path).map_err(|source| {
        ApiError::internal(format!(
            "failed to read workspace file metadata {}: {}",
            path.display(),
            source
        ))
    })?;
    let kind = if metadata.is_dir() {
        WorkspaceFileTreeNodeKind::Directory
    } else {
        WorkspaceFileTreeNodeKind::File
    };
    let name = if is_root {
        workspace_root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("/")
            .to_string()
    } else {
        path.file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                ApiError::internal(format!("invalid workspace file name: {}", path.display()))
            })?
            .to_string()
    };
    let relative_path = path.strip_prefix(workspace_root).map_err(|_| {
        ApiError::internal(format!("workspace file escaped root: {}", path.display()))
    })?;
    let path_text = relative_path.to_string_lossy().replace('\\', "/");
    let (children, children_loaded, has_children) = if metadata.is_dir() {
        let has_children = workspace_file_tree_directory_has_children(path)?;
        let children = if load_children {
            workspace_file_tree_children(workspace_root, path, depth, true)?
        } else {
            Vec::new()
        };
        (children, load_children, has_children)
    } else {
        (Vec::new(), true, false)
    };

    Ok(WorkspaceFileTreeNode {
        name,
        path: path_text,
        kind,
        size_bytes: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
        has_children,
        children_loaded,
        children,
    })
}

fn workspace_file_tree_directory_has_children(path: &Path) -> Result<bool, ApiError> {
    let mut entries = fs::read_dir(path).map_err(|source| {
        ApiError::internal(format!(
            "failed to read workspace directory {}: {}",
            path.display(),
            source
        ))
    })?;

    entries
        .next()
        .transpose()
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to read workspace directory entry {}: {}",
                path.display(),
                source
            ))
        })
        .map(|entry| entry.is_some())
}

fn workspace_file_parent_path(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

fn workspace_file_list_path(
    workspace_root: &Path,
    input: &str,
) -> Result<std::path::PathBuf, ApiError> {
    if input.trim().is_empty() {
        return Ok(workspace_root.to_path_buf());
    }

    let relative_path = normalize_workspace_relative_path(input)?;
    Ok(workspace_root.join(relative_path))
}

fn workspace_file_path(workspace_root: &Path, input: &str) -> Result<std::path::PathBuf, ApiError> {
    let relative_path = normalize_workspace_relative_path(input)?;
    let path = workspace_root.join(relative_path);

    if path == workspace_root {
        return Err(ApiError::bad_request("workspace root cannot be modified"));
    }

    Ok(path)
}

fn workspace_image_content_type(bytes: &[u8]) -> Result<&'static str, ApiError> {
    if bytes.starts_with(b"\x89PNG\r\n\x1A\n") {
        return Ok("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok("image/gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Ok("image/webp");
    }
    if let Ok(text) = std::str::from_utf8(&bytes[..bytes.len().min(256)]) {
        let trimmed = text.trim_start();
        if trimmed.starts_with("<?xml")
            || trimmed.starts_with("<svg")
            || trimmed.starts_with("<!DOCTYPE")
        {
            return Ok("image/svg+xml");
        }
    }

    Err(ApiError::bad_request(
        "workspace file preview only supports PNG, JPEG, WebP, GIF, or SVG images",
    ))
}

#[cfg(test)]
mod tests {
    use super::workspace_image_content_type;

    #[test]
    fn workspace_image_content_type_accepts_svg_only_as_image_text() {
        assert_eq!(
            workspace_image_content_type(br#"  <svg xmlns="http://www.w3.org/2000/svg"></svg>"#)
                .unwrap(),
            "image/svg+xml"
        );
        assert!(workspace_image_content_type(b"plain text").is_err());
    }
}

fn validate_workspace_file_name(input: &str) -> Result<&str, ApiError> {
    let name = input.trim();

    if name.is_empty() {
        return Err(ApiError::bad_request("file name must not be empty"));
    }

    let path = Path::new(name);
    if path.components().count() != 1 || name == "." || name == ".." {
        return Err(ApiError::bad_request(format!(
            "file name must not contain path separators: {name}"
        )));
    }

    Ok(name)
}
