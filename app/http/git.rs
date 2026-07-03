use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::git_backend::{
    commit_staged_changes as commit_staged_changes_in_workspace,
    create_git_branch as create_git_branch_in_workspace,
    discard_git_file as discard_git_file_in_workspace, git_branches_response, git_diff_response,
    git_status_response, is_git_workspace, resolve_git_worktree_target,
    stage_git_file as stage_git_file_in_workspace,
    switch_git_branch as switch_git_branch_in_workspace,
    unstage_git_file as unstage_git_file_in_workspace,
};
use crate::{
    ApiError, AppState, GitBranchesResponse, GitCommitMessageResponse, GitDiffResponse,
    GitStatusResponse, config_snapshot, generate_git_commit_message,
    normalize_workspace_relative_path, workspace_by_id,
};

pub(crate) async fn git_status(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Query(query): Query<GitTargetQuery>,
) -> Result<Json<GitStatusResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let target_path = resolve_git_request_target(&workspace.path, query.worktree_path.as_deref())?;

    Ok(Json(git_status_response(&target_path)?))
}

pub(crate) async fn git_diff(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Query(query): Query<GitDiffQuery>,
) -> Result<Json<GitDiffResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = query
        .path
        .as_deref()
        .map(normalize_workspace_relative_path)
        .transpose()?;
    let target_path = resolve_git_request_target(&workspace.path, query.worktree_path.as_deref())?;

    Ok(Json(git_diff_response(&target_path, path)?))
}

pub(crate) async fn stage_git_file(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitFileRequest>,
) -> Result<Json<GitDiffResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = normalize_workspace_relative_path(&request.path)?;
    let target_path =
        resolve_git_request_target(&workspace.path, request.worktree_path.as_deref())?;

    stage_git_file_in_workspace(&target_path, &path)?;

    Ok(Json(git_diff_response(&target_path, None)?))
}

pub(crate) async fn unstage_git_file(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitFileRequest>,
) -> Result<Json<GitDiffResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = normalize_workspace_relative_path(&request.path)?;
    let target_path =
        resolve_git_request_target(&workspace.path, request.worktree_path.as_deref())?;

    unstage_git_file_in_workspace(&target_path, &path)?;

    Ok(Json(git_diff_response(&target_path, None)?))
}

pub(crate) async fn discard_git_file(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitFileRequest>,
) -> Result<Json<GitDiffResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = normalize_workspace_relative_path(&request.path)?;
    let target_path =
        resolve_git_request_target(&workspace.path, request.worktree_path.as_deref())?;

    discard_git_file_in_workspace(&target_path, &path)?;

    Ok(Json(git_diff_response(&target_path, None)?))
}
pub(crate) async fn commit_staged_changes(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitCommitRequest>,
) -> Result<Json<GitDiffResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let target_path =
        resolve_git_request_target(&workspace.path, request.worktree_path.as_deref())?;

    commit_staged_changes_in_workspace(&target_path, request.message)?;

    Ok(Json(git_diff_response(&target_path, None)?))
}

pub(crate) async fn generate_commit_message(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitGenerateCommitMessageRequest>,
) -> Result<Json<GitCommitMessageResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let target_path =
        resolve_git_request_target(&workspace.path, request.worktree_path.as_deref())?;
    let diff = git_diff_response(&target_path, None)?;

    if diff.staged_files.is_empty() || diff.staged_diff.trim().is_empty() {
        return Err(ApiError::bad_request("no staged git changes to summarize"));
    }

    Ok(Json(
        generate_git_commit_message(
            &target_path,
            &workspace.id,
            &config,
            request.model_id,
            request.provider_id,
            &diff.staged_files,
            &diff.staged_diff,
        )
        .await?,
    ))
}

pub(crate) async fn git_branches(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Json<GitBranchesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    if !is_git_workspace(&workspace.path)? {
        return Ok(Json(GitBranchesResponse {
            is_git_repository: false,
            current_branch: None,
            branches: Vec::new(),
            worktrees: Vec::new(),
        }));
    }

    Ok(Json(git_branches_response(&workspace.path)?))
}

pub(crate) async fn switch_git_branch(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitBranchRequest>,
) -> Result<Json<GitBranchesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    switch_git_branch_in_workspace(&workspace.path, request.name)?;

    Ok(Json(git_branches_response(&workspace.path)?))
}

pub(crate) async fn create_git_branch(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitBranchRequest>,
) -> Result<Json<GitBranchesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    create_git_branch_in_workspace(&workspace.path, request.name)?;

    Ok(Json(git_branches_response(&workspace.path)?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitTargetQuery {
    worktree_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitDiffQuery {
    path: Option<String>,
    worktree_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitFileRequest {
    path: String,
    worktree_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitCommitRequest {
    message: String,
    worktree_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitGenerateCommitMessageRequest {
    model_id: String,
    provider_id: String,
    worktree_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitBranchRequest {
    name: String,
}

fn resolve_git_request_target(
    workspace_path: &Path,
    worktree_path: Option<&str>,
) -> Result<PathBuf, ApiError> {
    match worktree_path.map(str::trim).filter(|path| !path.is_empty()) {
        Some(path) => resolve_git_worktree_target(workspace_path, path),
        None => Ok(workspace_path.to_path_buf()),
    }
}
