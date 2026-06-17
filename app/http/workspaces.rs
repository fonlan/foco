use std::{fs, path::Path};

use axum::{
    Json,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{StatusCode, header},
    response::Response,
};
use foco_store::workspace::WorkspaceDatabase;
use foco_tools::set_ripgrep_path;

use crate::*;

pub(crate) async fn workspace_files(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Json<WorkspaceFilesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    Ok(Json(WorkspaceFilesResponse {
        root: workspace_file_tree(&workspace.path, &workspace.path, true)?,
    }))
}

pub(crate) async fn delete_workspace_file(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<WorkspaceFileRequest>,
) -> Result<Json<WorkspaceFilesResponse>, ApiError> {
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

    Ok(Json(WorkspaceFilesResponse {
        root: workspace_file_tree(&workspace.path, &workspace.path, true)?,
    }))
}

pub(crate) async fn rename_workspace_file(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<RenameWorkspaceFileRequest>,
) -> Result<Json<WorkspaceFilesResponse>, ApiError> {
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

    Ok(Json(WorkspaceFilesResponse {
        root: workspace_file_tree(&workspace.path, &workspace.path, true)?,
    }))
}

pub(crate) async fn workspaces(
    State(state): State<AppState>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let config = config_snapshot(&state)?;

    workspace_response_from_config(&config, &state.active_chat_runs)
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
            terminal_shell: DEFAULT_TERMINAL_SHELL.to_string(),
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

fn workspace_file_tree(
    workspace_root: &Path,
    path: &Path,
    is_root: bool,
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
    let mut children = Vec::new();

    if metadata.is_dir() {
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

        for entry in entries {
            children.push(workspace_file_tree(workspace_root, &entry.path(), false)?);
        }
    }

    Ok(WorkspaceFileTreeNode {
        name,
        path: path_text,
        kind,
        size_bytes: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
        children,
    })
}

fn workspace_file_path(workspace_root: &Path, input: &str) -> Result<std::path::PathBuf, ApiError> {
    let relative_path = normalize_workspace_relative_path(input)?;
    let path = workspace_root.join(relative_path);

    if path == workspace_root {
        return Err(ApiError::bad_request("workspace root cannot be modified"));
    }

    Ok(path)
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
