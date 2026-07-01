use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
};
use foco_store::config::WorkspaceConfig;
use foco_store::workspace::{
    NewWorkspaceSpecJob, WorkspaceDatabase, WorkspaceDatabaseError, WorkspaceSpecJobRecord,
    WorkspaceSpecRecord, WorkspaceSpecTriggerType, workspace_database_path,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::spec_runtime::{log_workspace_spec_job_status, run_workspace_spec_job};
use crate::*;

const DEFAULT_SPEC_JOB_LIMIT: i64 = 50;
const MAX_SPEC_JOB_LIMIT: i64 = 100;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WorkspaceSpecSettingsRequest {
    enabled: bool,
    inject_enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SaveWorkspaceSpecRequest {
    expected_revision: u64,
    content_markdown: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GenerateWorkspaceSpecRequest {
    model_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecJobsQuery {
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecResponse {
    settings: WorkspaceSpecSettingsView,
    content_markdown: String,
    revision: u64,
    generated_at: Option<String>,
    updated_at: Option<String>,
    latest_job: Option<WorkspaceSpecJobSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenerateWorkspaceSpecResponse {
    job: WorkspaceSpecJobSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecJobsResponse {
    jobs: Vec<WorkspaceSpecJobSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsWorkspaceSpecJobsResponse {
    jobs: Vec<WorkspaceSpecJobWithWorkspaceSummary>,
    errors: Vec<SettingsWorkspaceSpecJobError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecJobRetryResponse {
    job: WorkspaceSpecJobSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecJobWithWorkspaceSummary {
    job: WorkspaceSpecJobSummary,
    workspace_id: String,
    workspace_name: String,
    workspace_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsWorkspaceSpecJobError {
    workspace_id: String,
    workspace_name: String,
    workspace_path: String,
    error: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSpecSettingsView {
    enabled: bool,
    inject_enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecJobSummary {
    id: String,
    trigger_type: String,
    status: String,
    chat_id: Option<String>,
    run_id: Option<String>,
    model_id: Option<String>,
    base_revision: Option<u64>,
    input_summary: Value,
    output: Option<Value>,
    error_message: Option<String>,
    created_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
}

pub(crate) async fn workspace_spec(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Json<WorkspaceSpecResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let database =
        WorkspaceDatabase::open_or_create(&workspace.path).map_err(spec_workspace_error)?;

    Ok(Json(workspace_spec_response(&database)?))
}

pub(crate) async fn save_workspace_spec_settings(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<WorkspaceSpecSettingsRequest>,
) -> Result<Json<WorkspaceSpecResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace.path).map_err(spec_workspace_error)?;

    database
        .upsert_workspace_spec_settings(request.enabled, request.inject_enabled)
        .map_err(spec_workspace_error)?;

    Ok(Json(workspace_spec_response(&database)?))
}

pub(crate) async fn save_workspace_spec(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<SaveWorkspaceSpecRequest>,
) -> Result<Json<WorkspaceSpecResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace.path).map_err(spec_workspace_error)?;

    if !database
        .workspace_spec()
        .map_err(spec_workspace_error)?
        .is_some_and(|spec| spec.enabled)
    {
        return Err(ApiError::bad_request("workspace spec is disabled"));
    }

    database
        .update_workspace_spec_content(request.expected_revision, &request.content_markdown)
        .map_err(spec_workspace_error)?
        .ok_or_else(|| ApiError::conflict("workspace spec revision changed; reload and retry"))?;

    Ok(Json(workspace_spec_response(&database)?))
}

pub(crate) async fn generate_workspace_spec(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GenerateWorkspaceSpecRequest>,
) -> Result<Json<GenerateWorkspaceSpecResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace.path).map_err(spec_workspace_error)?;
    let spec = database.workspace_spec().map_err(spec_workspace_error)?;

    let Some(spec) = spec.filter(|spec| spec.enabled) else {
        return Err(ApiError::bad_request("workspace spec is disabled"));
    };
    if let Some(job) = database
        .running_workspace_spec_job()
        .map_err(spec_workspace_error)?
    {
        return Err(ApiError::conflict(format!(
            "workspace spec job is already running: {}",
            job.id
        )));
    }

    let model_id = request
        .model_id
        .and_then(non_empty_text)
        .or_else(|| config.spec.generation_model_id.clone());
    let trigger_type = if spec.content_markdown.trim().is_empty() {
        WorkspaceSpecTriggerType::ManualInitial
    } else {
        WorkspaceSpecTriggerType::ManualRefresh
    };
    let job = database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: &unique_id("workspace-spec-job"),
            trigger_type: trigger_type.as_str(),
            chat_id: None,
            run_id: None,
            model_id: model_id.as_deref(),
            base_revision: Some(spec.revision),
            input_summary_json: None,
        })
        .map_err(spec_workspace_error)?;
    log_workspace_spec_job_status(&workspace_id, &job);
    let response = workspace_spec_job_summary(job.clone())?;
    let runtime_state = state.clone();
    let runtime_workspace_id = workspace_id.clone();
    let runtime_job_id = job.id.clone();
    tokio::spawn(async move {
        if let Err(error) = run_workspace_spec_job(
            runtime_state,
            runtime_workspace_id.clone(),
            runtime_job_id.clone(),
        )
        .await
        {
            tracing::error!(
                workspace_id = %runtime_workspace_id,
                job_id = %runtime_job_id,
                error = %error.message,
                "workspace spec generation job failed"
            );
        }
    });

    Ok(Json(GenerateWorkspaceSpecResponse { job: response }))
}

pub(crate) async fn workspace_spec_jobs(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Query(query): Query<WorkspaceSpecJobsQuery>,
) -> Result<Json<WorkspaceSpecJobsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let database =
        WorkspaceDatabase::open_or_create(&workspace.path).map_err(spec_workspace_error)?;
    let limit = spec_job_limit(query.limit)?;

    let jobs = database
        .workspace_spec_jobs(limit)
        .map_err(spec_workspace_error)?
        .into_iter()
        .map(workspace_spec_job_summary)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(WorkspaceSpecJobsResponse { jobs }))
}

pub(crate) async fn settings_workspace_spec_jobs(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceSpecJobsQuery>,
) -> Result<Json<SettingsWorkspaceSpecJobsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let limit = spec_job_limit(query.limit)?;
    let mut jobs = Vec::new();
    let mut errors = Vec::new();

    for workspace in &config.workspaces {
        if !workspace_database_path(&workspace.path).exists() {
            continue;
        }
        match workspace_spec_jobs_for_workspace(workspace, limit) {
            Ok(mut workspace_jobs) => jobs.append(&mut workspace_jobs),
            Err(error) => errors.push(SettingsWorkspaceSpecJobError {
                workspace_id: workspace.id.clone(),
                workspace_name: workspace.name.clone(),
                workspace_path: workspace.path.display().to_string(),
                error: error.to_string(),
            }),
        }
    }

    jobs.sort_by(|left, right| {
        right
            .job
            .created_at
            .cmp(&left.job.created_at)
            .then_with(|| right.job.id.cmp(&left.job.id))
    });
    jobs.truncate(limit as usize);

    Ok(Json(SettingsWorkspaceSpecJobsResponse { jobs, errors }))
}

pub(crate) async fn retry_workspace_spec_job(
    State(state): State<AppState>,
    AxumPath((workspace_id, job_id)): AxumPath<(String, String)>,
) -> Result<Json<WorkspaceSpecJobRetryResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace.path).map_err(spec_workspace_error)?;
    let should_spawn = database
        .running_workspace_spec_job()
        .map_err(spec_workspace_error)?
        .is_none();
    let new_job = database
        .retry_failed_workspace_spec_job(&job_id, &unique_id("workspace-spec-job"))
        .map_err(spec_workspace_error)?
        .ok_or_else(|| {
            ApiError::bad_request("workspace spec job is not failed or was not found")
        })?;
    log_workspace_spec_job_status(&workspace_id, &new_job);
    let response = workspace_spec_job_summary(new_job.clone())?;
    drop(database);

    if should_spawn {
        let runtime_state = state.clone();
        let runtime_workspace_id = workspace_id.clone();
        let runtime_job_id = new_job.id.clone();
        tokio::spawn(async move {
            if let Err(error) = run_workspace_spec_job(
                runtime_state,
                runtime_workspace_id.clone(),
                runtime_job_id.clone(),
            )
            .await
            {
                tracing::error!(
                    workspace_id = %runtime_workspace_id,
                    job_id = %runtime_job_id,
                    error = %error.message,
                    "workspace spec retry job failed"
                );
            }
        });
    }

    Ok(Json(WorkspaceSpecJobRetryResponse { job: response }))
}

fn spec_job_limit(limit: Option<i64>) -> Result<i64, ApiError> {
    let limit = limit.unwrap_or(DEFAULT_SPEC_JOB_LIMIT);
    if !(1..=MAX_SPEC_JOB_LIMIT).contains(&limit) {
        return Err(ApiError::bad_request(format!(
            "limit must be between 1 and {MAX_SPEC_JOB_LIMIT}"
        )));
    }
    Ok(limit)
}

fn workspace_spec_jobs_for_workspace(
    workspace: &WorkspaceConfig,
    limit: i64,
) -> Result<Vec<WorkspaceSpecJobWithWorkspaceSummary>, WorkspaceDatabaseError> {
    let database = WorkspaceDatabase::open_or_create(&workspace.path)?;
    database
        .workspace_spec_jobs(limit)?
        .into_iter()
        .map(|job| {
            let job = workspace_spec_job_summary(job).map_err(|error| {
                WorkspaceDatabaseError::InvalidWorkspaceSpec {
                    message: error.message,
                }
            })?;
            Ok(WorkspaceSpecJobWithWorkspaceSummary {
                job,
                workspace_id: workspace.id.clone(),
                workspace_name: workspace.name.clone(),
                workspace_path: workspace.path.display().to_string(),
            })
        })
        .collect()
}

fn workspace_spec_response(
    database: &WorkspaceDatabase,
) -> Result<WorkspaceSpecResponse, ApiError> {
    let spec = database
        .workspace_spec()
        .map_err(spec_workspace_error)?
        .map(workspace_spec_view)
        .unwrap_or_else(default_workspace_spec_view);
    let latest_job = database
        .workspace_spec_jobs(1)
        .map_err(spec_workspace_error)?
        .into_iter()
        .next()
        .map(workspace_spec_job_summary)
        .transpose()?;

    Ok(WorkspaceSpecResponse { latest_job, ..spec })
}

fn workspace_spec_view(spec: WorkspaceSpecRecord) -> WorkspaceSpecResponse {
    WorkspaceSpecResponse {
        settings: WorkspaceSpecSettingsView {
            enabled: spec.enabled,
            inject_enabled: spec.inject_enabled,
        },
        content_markdown: spec.content_markdown,
        revision: spec.revision,
        generated_at: spec.generated_at,
        updated_at: Some(spec.updated_at),
        latest_job: None,
    }
}

fn default_workspace_spec_view() -> WorkspaceSpecResponse {
    WorkspaceSpecResponse {
        settings: WorkspaceSpecSettingsView {
            enabled: false,
            inject_enabled: false,
        },
        content_markdown: String::new(),
        revision: 0,
        generated_at: None,
        updated_at: None,
        latest_job: None,
    }
}

fn workspace_spec_job_summary(
    job: WorkspaceSpecJobRecord,
) -> Result<WorkspaceSpecJobSummary, ApiError> {
    Ok(WorkspaceSpecJobSummary {
        id: job.id,
        trigger_type: job.trigger_type,
        status: job.status,
        chat_id: job.chat_id,
        run_id: job.run_id,
        model_id: job.model_id,
        base_revision: job.base_revision,
        input_summary: workspace_spec_json(&job.input_summary_json, "input_summary_json")?,
        output: job
            .output_json
            .map(|value| workspace_spec_json(&value, "output_json"))
            .transpose()?,
        error_message: job.error_message,
        created_at: job.created_at,
        started_at: job.started_at,
        completed_at: job.completed_at,
    })
}

fn workspace_spec_json(value: &str, field: &str) -> Result<Value, ApiError> {
    serde_json::from_str(value).map_err(|source| {
        ApiError::internal(format!("workspace spec {field} is invalid: {source}"))
    })
}

fn non_empty_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn spec_workspace_error(error: WorkspaceDatabaseError) -> ApiError {
    match error {
        WorkspaceDatabaseError::InvalidWorkspaceSpec { .. } => {
            ApiError::bad_request(error.to_string())
        }
        _ => ApiError::from_workspace_error(error),
    }
}
