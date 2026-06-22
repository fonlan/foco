use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
};
use foco_store::{
    config::WorkspaceConfig,
    workspace::{
        LlmRequestAuditSummaryRow, NewScheduledTask, ScheduledTaskRecord, ScheduledTaskRunRecord,
        ScheduledTaskUpdate, WorkspaceDatabase,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    scheduled_tasks::{
        service::{PreviewNextRunRequest, PreviewNextRunResponse, preview_next_run},
        types::{
            ScheduleSpec, ScheduledAction, ScheduledConcurrencyPolicy, ScheduledMisfirePolicy,
            ScheduledTaskMetadata,
        },
    },
    *,
};

const STATUS_ENABLED: &str = "enabled";
const STATUS_PAUSED: &str = "paused";
const STATUS_COMPLETED: &str = "completed";
const STATUS_ARCHIVED: &str = "archived";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledTasksQuery {
    workspace_id: Option<String>,
    status: Option<String>,
    q: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreateScheduledTaskRequest {
    title: String,
    description: Option<String>,
    schedule: ScheduleSpec,
    action: ScheduledAction,
    status: Option<String>,
    concurrency_policy: Option<ScheduledConcurrencyPolicy>,
    misfire_policy: Option<ScheduledMisfirePolicy>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UpdateScheduledTaskRequest {
    title: Option<String>,
    #[serde(default)]
    description: Option<Option<String>>,
    schedule: Option<ScheduleSpec>,
    action: Option<ScheduledAction>,
    status: Option<String>,
    concurrency_policy: Option<ScheduledConcurrencyPolicy>,
    misfire_policy: Option<ScheduledMisfirePolicy>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledTasksResponse {
    tasks: Vec<ScheduledTaskView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledTaskResponse {
    task: ScheduledTaskView,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledTaskRunsResponse {
    runs: Vec<ScheduledTaskRunView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledTaskRunResponse {
    run: ScheduledTaskRunView,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledTaskView {
    id: String,
    workspace_id: String,
    workspace_name: String,
    title: String,
    description: Option<String>,
    schedule: Value,
    action: Value,
    status: String,
    next_run_at: Option<String>,
    last_run_at: Option<String>,
    created_at: String,
    updated_at: String,
    metadata: Value,
    usage: ScheduledTaskUsageView,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledTaskUsageView {
    total_requests: i64,
    failed_requests: i64,
    total_input_tokens: i64,
    total_output_tokens: i64,
    total_cache_read_tokens: i64,
    total_cache_write_tokens: i64,
    total_tokens: i64,
    total_latency_ms: i64,
    average_latency_ms: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledTaskRunView {
    id: String,
    workspace_id: String,
    task_id: String,
    trigger_reason: String,
    status: String,
    scheduled_at: String,
    queued_at: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
    chat_id: Option<String>,
    user_message_id: Option<String>,
    assistant_message_id: Option<String>,
    agent_team_id: Option<String>,
    agent_task_id: Option<String>,
    agent_attempt_id: Option<String>,
    active_run_id: Option<String>,
    error_message: Option<String>,
    output_summary: Option<String>,
    created_at: String,
    updated_at: String,
    metadata: Value,
}

pub(crate) async fn scheduled_tasks(
    State(state): State<AppState>,
    Query(query): Query<ScheduledTasksQuery>,
) -> Result<Json<ScheduledTasksResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let status = query
        .status
        .map(|status| normalize_task_status("status", &status))
        .transpose()?;
    let search = query
        .q
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let mut tasks = Vec::new();

    for workspace in scheduled_task_workspaces(&config, query.workspace_id.as_deref())? {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        for task in database
            .scheduled_tasks(status.as_deref())
            .map_err(ApiError::from_workspace_error)?
        {
            if scheduled_task_matches_search(workspace, &task, search.as_deref()) {
                tasks.push(scheduled_task_view(workspace, &database, task)?);
            }
        }
    }

    Ok(Json(ScheduledTasksResponse { tasks }))
}

pub(crate) async fn create_scheduled_task(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<CreateScheduledTaskRequest>,
) -> Result<Json<ScheduledTaskResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let title = normalized_required_text("title", &request.title)?;
    let description = normalized_optional_text(request.description);
    let status = request
        .status
        .map(|status| normalize_task_status("status", &status))
        .transpose()?
        .unwrap_or_else(|| STATUS_ENABLED.to_string());
    let schedule_json = scheduled_json("schedule", &request.schedule)?;
    let action_json = scheduled_json("action", &request.action)?;
    let metadata_json = scheduled_task_metadata_json(
        &workspace.id,
        None,
        request.concurrency_policy,
        request.misfire_policy,
    )?;
    let next_run_at = task_next_run_at(&request.schedule, &status)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let task = database
        .insert_scheduled_task(NewScheduledTask {
            id: &unique_id("scheduled-task"),
            title: &title,
            description: description.as_deref(),
            schedule_json: &schedule_json,
            action_json: &action_json,
            status: &status,
            next_run_at: next_run_at.as_deref(),
            metadata_json: Some(&metadata_json),
        })
        .map_err(ApiError::from_workspace_error)?;

    notify_scheduled_task_change(&state)?;
    Ok(Json(ScheduledTaskResponse {
        task: scheduled_task_view(workspace, &database, task)?,
    }))
}

pub(crate) async fn scheduled_task(
    State(state): State<AppState>,
    AxumPath((workspace_id, task_id)): AxumPath<(String, String)>,
) -> Result<Json<ScheduledTaskResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let task = require_scheduled_task(&database, &task_id)?;

    Ok(Json(ScheduledTaskResponse {
        task: scheduled_task_view(workspace, &database, task)?,
    }))
}

pub(crate) async fn update_scheduled_task(
    State(state): State<AppState>,
    AxumPath((workspace_id, task_id)): AxumPath<(String, String)>,
    Json(request): Json<UpdateScheduledTaskRequest>,
) -> Result<Json<ScheduledTaskResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let existing = require_scheduled_task(&database, &task_id)?;
    let title = match request.title {
        Some(title) => normalized_required_text("title", &title)?,
        None => existing.title.clone(),
    };
    let description = match request.description {
        Some(Some(description)) => normalized_optional_text(Some(description)),
        Some(None) => None,
        None => existing.description.clone(),
    };
    let schedule = match request.schedule.as_ref() {
        Some(schedule) => schedule.clone(),
        None => persisted_schedule(&existing.schedule_json)?,
    };
    let schedule_json = match request.schedule {
        Some(schedule) => scheduled_json("schedule", &schedule)?,
        None => existing.schedule_json.clone(),
    };
    let action_json = match request.action {
        Some(action) => scheduled_json("action", &action)?,
        None => existing.action_json.clone(),
    };
    let status = match request.status {
        Some(status) => normalize_task_status("status", &status)?,
        None => existing.status.clone(),
    };
    let next_run_at = if status == STATUS_ENABLED {
        if schedule_json != existing.schedule_json || status != existing.status {
            task_next_run_at(&schedule, &status)?
        } else {
            existing.next_run_at.clone()
        }
    } else {
        None
    };
    let metadata_json = scheduled_task_metadata_json(
        &workspace.id,
        Some(&existing.metadata_json),
        request.concurrency_policy,
        request.misfire_policy,
    )?;

    let task = database
        .update_scheduled_task(ScheduledTaskUpdate {
            id: &task_id,
            title: &title,
            description: description.as_deref(),
            schedule_json: &schedule_json,
            action_json: &action_json,
            status: &status,
            next_run_at: next_run_at.as_deref(),
            last_run_at: existing.last_run_at.as_deref(),
            metadata_json: &metadata_json,
        })
        .map_err(ApiError::from_workspace_error)?;

    notify_scheduled_task_change(&state)?;
    Ok(Json(ScheduledTaskResponse {
        task: scheduled_task_view(workspace, &database, task)?,
    }))
}

pub(crate) async fn delete_scheduled_task(
    State(state): State<AppState>,
    AxumPath((workspace_id, task_id)): AxumPath<(String, String)>,
) -> Result<Json<ScheduledTaskResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let task = require_scheduled_task(&database, &task_id)?;

    if !database
        .delete_scheduled_task(&task_id)
        .map_err(ApiError::from_workspace_error)?
    {
        return Err(ApiError::bad_request(format!(
            "scheduled task was not found: {task_id}"
        )));
    }

    Ok(Json(ScheduledTaskResponse {
        task: scheduled_task_view(workspace, &database, task)?,
    }))
}

pub(crate) async fn pause_scheduled_task(
    State(state): State<AppState>,
    AxumPath((workspace_id, task_id)): AxumPath<(String, String)>,
) -> Result<Json<ScheduledTaskResponse>, ApiError> {
    set_scheduled_task_status(state, &workspace_id, &task_id, STATUS_PAUSED).map(Json)
}

pub(crate) async fn resume_scheduled_task(
    State(state): State<AppState>,
    AxumPath((workspace_id, task_id)): AxumPath<(String, String)>,
) -> Result<Json<ScheduledTaskResponse>, ApiError> {
    set_scheduled_task_status(state, &workspace_id, &task_id, STATUS_ENABLED).map(Json)
}

pub(crate) async fn archive_scheduled_task(
    State(state): State<AppState>,
    AxumPath((workspace_id, task_id)): AxumPath<(String, String)>,
) -> Result<Json<ScheduledTaskResponse>, ApiError> {
    set_scheduled_task_status(state, &workspace_id, &task_id, STATUS_ARCHIVED).map(Json)
}

pub(crate) async fn run_scheduled_task_now(
    State(state): State<AppState>,
    AxumPath((workspace_id, task_id)): AxumPath<(String, String)>,
) -> Result<Json<ScheduledTaskRunResponse>, ApiError> {
    let run =
        crate::scheduled_tasks::scheduler::run_scheduled_task_now(&state, &workspace_id, &task_id)
            .await?;
    Ok(Json(ScheduledTaskRunResponse {
        run: scheduled_task_run_view(&workspace_id, run)?,
    }))
}

pub(crate) async fn scheduled_task_runs(
    State(state): State<AppState>,
    AxumPath((workspace_id, task_id)): AxumPath<(String, String)>,
) -> Result<Json<ScheduledTaskRunsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    require_scheduled_task(&database, &task_id)?;
    let runs = database
        .scheduled_task_runs_for_task(&task_id)
        .map_err(ApiError::from_workspace_error)?;
    let mut views = Vec::with_capacity(runs.len());
    for run in runs {
        let run = crate::scheduled_tasks::scheduler::sync_scheduled_task_run(&mut database, run)?;
        views.push(scheduled_task_run_view(&workspace.id, run)?);
    }

    Ok(Json(ScheduledTaskRunsResponse { runs: views }))
}

pub(crate) async fn scheduled_task_run(
    State(state): State<AppState>,
    AxumPath((workspace_id, scheduled_run_id)): AxumPath<(String, String)>,
) -> Result<Json<ScheduledTaskRunResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let run = database
        .scheduled_task_run(&scheduled_run_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "scheduled task run was not found: {scheduled_run_id}"
            ))
        })?;
    let run = crate::scheduled_tasks::scheduler::sync_scheduled_task_run(&mut database, run)?;

    Ok(Json(ScheduledTaskRunResponse {
        run: scheduled_task_run_view(&workspace.id, run)?,
    }))
}

pub(crate) async fn cancel_scheduled_task_run(
    State(state): State<AppState>,
    AxumPath((workspace_id, scheduled_run_id)): AxumPath<(String, String)>,
) -> Result<Json<ScheduledTaskRunResponse>, ApiError> {
    let run = crate::scheduled_tasks::scheduler::cancel_scheduled_task_run(
        &state,
        &workspace_id,
        &scheduled_run_id,
    )?;
    Ok(Json(ScheduledTaskRunResponse {
        run: scheduled_task_run_view(&workspace_id, run)?,
    }))
}

pub(crate) async fn preview_scheduled_task_next_run(
    Json(request): Json<PreviewNextRunRequest>,
) -> Result<Json<PreviewNextRunResponse>, ApiError> {
    preview_next_run(request)
        .map(Json)
        .map_err(scheduled_task_error)
}

fn set_scheduled_task_status(
    state: AppState,
    workspace_id: &str,
    task_id: &str,
    status: &str,
) -> Result<ScheduledTaskResponse, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let existing = require_scheduled_task(&database, task_id)?;
    let schedule = persisted_schedule(&existing.schedule_json)?;
    let next_run_at = task_next_run_at(&schedule, status)?;
    let task = database
        .update_scheduled_task(ScheduledTaskUpdate {
            id: task_id,
            title: &existing.title,
            description: existing.description.as_deref(),
            schedule_json: &existing.schedule_json,
            action_json: &existing.action_json,
            status,
            next_run_at: next_run_at.as_deref(),
            last_run_at: existing.last_run_at.as_deref(),
            metadata_json: &existing.metadata_json,
        })
        .map_err(ApiError::from_workspace_error)?;

    if status == STATUS_ENABLED {
        notify_scheduled_task_change(&state)?;
    }

    Ok(ScheduledTaskResponse {
        task: scheduled_task_view(workspace, &database, task)?,
    })
}

fn scheduled_task_workspaces<'a>(
    config: &'a GlobalConfig,
    workspace_id: Option<&str>,
) -> Result<Vec<&'a WorkspaceConfig>, ApiError> {
    if let Some(workspace_id) = workspace_id {
        return Ok(vec![workspace_by_id(config, workspace_id)?]);
    }

    Ok(config.workspaces.iter().collect())
}

fn require_scheduled_task(
    database: &WorkspaceDatabase,
    task_id: &str,
) -> Result<ScheduledTaskRecord, ApiError> {
    let task_id = task_id.trim();
    if task_id.is_empty() {
        return Err(ApiError::bad_request("scheduled task id must not be empty"));
    }

    database
        .scheduled_task(task_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("scheduled task was not found: {task_id}")))
}

fn scheduled_task_view(
    workspace: &WorkspaceConfig,
    database: &WorkspaceDatabase,
    task: ScheduledTaskRecord,
) -> Result<ScheduledTaskView, ApiError> {
    let usage = database
        .scheduled_task_usage_summary(&task.id)
        .map_err(ApiError::from_workspace_error)?;
    Ok(ScheduledTaskView {
        id: task.id,
        workspace_id: workspace.id.clone(),
        workspace_name: workspace.name.clone(),
        title: task.title,
        description: task.description,
        schedule: persisted_json_object("scheduled task schedule", &task.schedule_json)?,
        action: persisted_json_object("scheduled task action", &task.action_json)?,
        status: task.status,
        next_run_at: task.next_run_at,
        last_run_at: task.last_run_at,
        created_at: task.created_at,
        updated_at: task.updated_at,
        metadata: persisted_json_object("scheduled task metadata", &task.metadata_json)?,
        usage: scheduled_task_usage_view(usage),
    })
}

fn scheduled_task_usage_view(summary: LlmRequestAuditSummaryRow) -> ScheduledTaskUsageView {
    ScheduledTaskUsageView {
        total_requests: summary.total_requests,
        failed_requests: summary.failed_requests,
        total_input_tokens: summary.total_input_tokens,
        total_output_tokens: summary.total_output_tokens,
        total_cache_read_tokens: summary.total_cache_read_tokens,
        total_cache_write_tokens: summary.total_cache_write_tokens,
        total_tokens: summary.total_tokens,
        total_latency_ms: summary.latency_sum,
        average_latency_ms: average_i64(summary.latency_sum, summary.latency_count),
    }
}

fn average_i64(sum: i64, count: i64) -> Option<i64> {
    if count == 0 {
        None
    } else {
        Some((sum as f64 / count as f64).round() as i64)
    }
}

fn scheduled_task_run_view(
    workspace_id: &str,
    run: ScheduledTaskRunRecord,
) -> Result<ScheduledTaskRunView, ApiError> {
    Ok(ScheduledTaskRunView {
        id: run.id,
        workspace_id: workspace_id.to_string(),
        task_id: run.task_id,
        trigger_reason: run.trigger_reason,
        status: run.status,
        scheduled_at: run.scheduled_at,
        queued_at: run.queued_at,
        started_at: run.started_at,
        completed_at: run.completed_at,
        chat_id: run.chat_id,
        user_message_id: run.user_message_id,
        assistant_message_id: run.assistant_message_id,
        agent_team_id: run.agent_team_id.map(|id| id.to_string()),
        agent_task_id: run.agent_task_id.map(|id| id.to_string()),
        agent_attempt_id: run.agent_attempt_id.map(|id| id.to_string()),
        active_run_id: run.active_run_id,
        error_message: run.error_message,
        output_summary: run.output_summary,
        created_at: run.created_at,
        updated_at: run.updated_at,
        metadata: persisted_json_object("scheduled task run metadata", &run.metadata_json)?,
    })
}

fn scheduled_task_matches_search(
    workspace: &WorkspaceConfig,
    task: &ScheduledTaskRecord,
    search: Option<&str>,
) -> bool {
    let Some(search) = search else {
        return true;
    };
    task.id.to_ascii_lowercase().contains(search)
        || task.title.to_ascii_lowercase().contains(search)
        || task
            .description
            .as_deref()
            .map(|description| description.to_ascii_lowercase().contains(search))
            .unwrap_or(false)
        || workspace.name.to_ascii_lowercase().contains(search)
        || workspace.id.to_ascii_lowercase().contains(search)
}

fn normalize_task_status(field: &str, status: &str) -> Result<String, ApiError> {
    let status = status.trim();
    match status {
        STATUS_ENABLED | STATUS_PAUSED | STATUS_COMPLETED | STATUS_ARCHIVED => {
            Ok(status.to_string())
        }
        _ => Err(ApiError::bad_request(format!(
            "{field} must be one of enabled, paused, completed, archived"
        ))),
    }
}

fn task_next_run_at(schedule: &ScheduleSpec, status: &str) -> Result<Option<String>, ApiError> {
    if status != STATUS_ENABLED {
        return Ok(None);
    }

    preview_next_run(PreviewNextRunRequest {
        schedule: schedule.clone(),
        now: None,
    })
    .map(|response| response.next_run_at)
    .map_err(scheduled_task_error)
}

fn scheduled_json<T: Serialize>(field: &str, value: &T) -> Result<String, ApiError> {
    serde_json::to_string(value)
        .map_err(|source| ApiError::bad_request(format!("{field} must be valid JSON: {source}")))
}

fn persisted_schedule(schedule_json: &str) -> Result<ScheduleSpec, ApiError> {
    serde_json::from_str(schedule_json).map_err(|source| {
        ApiError::internal(format!(
            "invalid persisted scheduled task schedule JSON: {source}"
        ))
    })
}

fn persisted_json_object(field: &str, json_text: &str) -> Result<Value, ApiError> {
    let value = serde_json::from_str::<Value>(json_text)
        .map_err(|source| ApiError::internal(format!("invalid persisted {field}: {source}")))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(ApiError::internal(format!(
            "invalid persisted {field}: expected object"
        )))
    }
}

fn scheduled_task_metadata_json(
    workspace_id: &str,
    existing_json: Option<&str>,
    concurrency_policy: Option<ScheduledConcurrencyPolicy>,
    misfire_policy: Option<ScheduledMisfirePolicy>,
) -> Result<String, ApiError> {
    let mut existing = match existing_json {
        Some(json_text) => persisted_json_object("scheduled task metadata", json_text)?,
        None => json!({}),
    };
    let current_concurrency_policy = existing
        .get("concurrencyPolicy")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|source| {
            ApiError::bad_request(format!(
                "metadata.concurrencyPolicy must be valid: {source}"
            ))
        })?
        .unwrap_or_default();
    let current_misfire_policy = existing
        .get("misfirePolicy")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|source| {
            ApiError::bad_request(format!("metadata.misfirePolicy must be valid: {source}"))
        })?
        .unwrap_or_default();
    let metadata = ScheduledTaskMetadata {
        workspace_id: workspace_id.to_string(),
        concurrency_policy: concurrency_policy.unwrap_or(current_concurrency_policy),
        misfire_policy: misfire_policy.unwrap_or(current_misfire_policy),
    };
    if let (Some(existing), Value::Object(metadata)) = (
        existing.as_object_mut(),
        serde_json::to_value(metadata).map_err(|source| {
            ApiError::internal(format!(
                "failed to serialize scheduled task metadata: {source}"
            ))
        })?,
    ) {
        existing.extend(metadata);
    }

    scheduled_json("metadata", &existing)
}

fn scheduled_task_error(error: crate::scheduled_tasks::service::ScheduledTaskError) -> ApiError {
    ApiError::bad_request(error.to_string())
}

fn notify_scheduled_task_change(state: &AppState) -> Result<(), ApiError> {
    state.scheduled_task_scheduler.wake()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_json_keeps_default_policies_and_server_workspace() {
        let metadata = scheduled_task_metadata_json(
            "workspace-1",
            Some(
                r#"{"workspaceId":"old","concurrencyPolicy":"queue_after_current","label":"keep"}"#,
            ),
            None,
            None,
        )
        .expect("metadata json");
        let value: Value = serde_json::from_str(&metadata).expect("metadata value");

        assert_eq!(value["workspaceId"], "workspace-1");
        assert_eq!(value["concurrencyPolicy"], "queue_after_current");
        assert_eq!(value["misfirePolicy"], "catch_up_once");
        assert_eq!(value["label"], "keep");
    }
}
