use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
};
use serde::Deserialize;

use crate::git_backend::{
    commit_staged_changes as commit_staged_changes_in_workspace,
    create_git_branch as create_git_branch_in_workspace,
    discard_git_file as discard_git_file_in_workspace, git_branches_response, git_diff_response,
    git_status_response, is_git_workspace, stage_git_file as stage_git_file_in_workspace,
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
) -> Result<Json<GitStatusResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    Ok(Json(git_status_response(&workspace.path)?))
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

    Ok(Json(git_diff_response(&workspace.path, path)?))
}

pub(crate) async fn stage_git_file(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitFileRequest>,
) -> Result<Json<GitDiffResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = normalize_workspace_relative_path(&request.path)?;

    stage_git_file_in_workspace(&workspace.path, &path)?;

    Ok(Json(git_diff_response(&workspace.path, None)?))
}

pub(crate) async fn unstage_git_file(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitFileRequest>,
) -> Result<Json<GitDiffResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = normalize_workspace_relative_path(&request.path)?;

    unstage_git_file_in_workspace(&workspace.path, &path)?;

    Ok(Json(git_diff_response(&workspace.path, None)?))
}

pub(crate) async fn discard_git_file(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitFileRequest>,
) -> Result<Json<GitDiffResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = normalize_workspace_relative_path(&request.path)?;

    discard_git_file_in_workspace(&workspace.path, &path)?;

    Ok(Json(git_diff_response(&workspace.path, None)?))
}
pub(crate) async fn commit_staged_changes(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitCommitRequest>,
) -> Result<Json<GitDiffResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    commit_staged_changes_in_workspace(&workspace.path, request.message)?;

    Ok(Json(git_diff_response(&workspace.path, None)?))
}

pub(crate) async fn generate_commit_message(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitGenerateCommitMessageRequest>,
) -> Result<Json<GitCommitMessageResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let diff = git_diff_response(&workspace.path, None)?;

    if diff.staged_files.is_empty() || diff.staged_diff.trim().is_empty() {
        return Err(ApiError::bad_request("no staged git changes to summarize"));
    }

    Ok(Json(
        generate_git_commit_message(
            &workspace.path,
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
pub(crate) struct GitDiffQuery {
    path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitFileRequest {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitCommitRequest {
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitGenerateCommitMessageRequest {
    model_id: String,
    provider_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitBranchRequest {
    name: String,
}
