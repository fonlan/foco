use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
};
use foco_store::{
    config::WorkspaceConfig,
    workspace::{
        LlmRequestAuditSummaryRow, NewScheduledTask, ScheduledTaskListFilter, ScheduledTaskRecord,
        ScheduledTaskRunRecord, ScheduledTaskUpdate, WorkspaceDatabase, WorkspaceDatabaseError,
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
const DEFAULT_PAGE_SIZE: usize = 25;
const DEFAULT_RUN_PAGE_SIZE: usize = 20;
const MAX_PAGE_SIZE: usize = 100;
const MAX_RUN_PAGE_SIZE: usize = 100;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledTasksQuery {
    workspace_id: Option<String>,
    status: Option<String>,
    q: Option<String>,
    page: Option<usize>,
    page_size: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledTaskRunsQuery {
    page: Option<usize>,
    page_size: Option<usize>,
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
    page: usize,
    page_size: usize,
    total_count: usize,
    total_pages: usize,
    status_counts: HashMap<String, usize>,
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
    page: usize,
    page_size: usize,
    total_count: usize,
    total_pages: usize,
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
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let (page, page_size, offset) = pagination(
        query.page,
        query.page_size,
        DEFAULT_PAGE_SIZE,
        MAX_PAGE_SIZE,
    )?;
    let page_window = i64::try_from(offset + page_size)
        .map_err(|_| ApiError::bad_request("scheduled task pagination window is too large"))?;
    let mut total_count = 0usize;
    let mut status_counts: HashMap<String, usize> = HashMap::new();
    let mut candidates: Vec<(
        &WorkspaceConfig,
        WorkspaceDatabaseHandle,
        Vec<ScheduledTaskRecord>,
    )> = Vec::new();

    for workspace in scheduled_task_workspaces(&config, query.workspace_id.as_deref())? {
        let database = match open_scheduled_task_workspace_database(
            workspace,
            query.workspace_id.is_some(),
        ) {
            Ok(Some(database)) => database,
            Ok(None) => continue,
            Err(error) => return Err(error),
        };
        let workspace_search_matches = search
            .as_deref()
            .map(|search| scheduled_workspace_matches_search(workspace, search))
            .unwrap_or(false);
        let store_search = if workspace_search_matches {
            None
        } else {
            search.as_deref()
        };
        let filter = ScheduledTaskListFilter {
            status: status.as_deref(),
            search: store_search,
            limit: page_window.max(1),
            offset: 0,
        };
        total_count += usize::try_from(
            database
                .scheduled_task_count(filter)
                .map_err(ApiError::from_workspace_error)?,
        )
        .map_err(|_| ApiError::internal("scheduled task count is too large"))?;
        for count in database
            .scheduled_task_status_counts(store_search)
            .map_err(ApiError::from_workspace_error)?
        {
            *status_counts.entry(count.status).or_insert(0) += usize::try_from(count.count)
                .map_err(|_| ApiError::internal("scheduled task status count is too large"))?;
        }
        let tasks = database
            .scheduled_tasks_page(filter)
            .map_err(ApiError::from_workspace_error)?;
        candidates.push((workspace, database, tasks));
    }

    let mut tasks_with_workspace = candidates
        .iter()
        .flat_map(|(workspace, database, tasks)| {
            tasks
                .iter()
                .cloned()
                .map(|task| (*workspace, database, task))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    tasks_with_workspace.sort_by(|left, right| compare_scheduled_tasks(&left.2, &right.2));

    let page_tasks = tasks_with_workspace
        .into_iter()
        .skip(offset)
        .take(page_size)
        .collect::<Vec<_>>();
    let mut task_ids_by_workspace: HashMap<String, Vec<String>> = HashMap::new();
    for (workspace, _, task) in &page_tasks {
        task_ids_by_workspace
            .entry(workspace.id.clone())
            .or_default()
            .push(task.id.clone());
    }
    let mut usage_by_workspace: HashMap<String, HashMap<String, LlmRequestAuditSummaryRow>> =
        HashMap::new();
    for (workspace, database, _) in &candidates {
        if let Some(task_ids) = task_ids_by_workspace.get(&workspace.id) {
            usage_by_workspace.insert(
                workspace.id.clone(),
                database
                    .scheduled_task_usage_summaries(task_ids)
                    .map_err(ApiError::from_workspace_error)?,
            );
        }
    }

    let mut tasks = Vec::with_capacity(page_tasks.len());
    for (workspace, _, task) in page_tasks {
        let usage = usage_by_workspace
            .get(&workspace.id)
            .and_then(|summaries| summaries.get(&task.id))
            .cloned()
            .unwrap_or_default();
        tasks.push(scheduled_task_view_with_usage(workspace, task, usage)?);
    }

    Ok(Json(ScheduledTasksResponse {
        tasks,
        page,
        page_size,
        total_count,
        total_pages: total_pages(total_count, page_size),
        status_counts,
    }))
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
        .map(|status| normalize_initial_task_status("status", &status))
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

pub(crate) async fn duplicate_scheduled_task(
    State(state): State<AppState>,
    AxumPath((workspace_id, task_id)): AxumPath<(String, String)>,
) -> Result<Json<ScheduledTaskResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let existing = require_scheduled_task(&database, &task_id)?;
    let title = format!("{} copy", existing.title);
    let task = database
        .insert_scheduled_task(NewScheduledTask {
            id: &unique_id("scheduled-task"),
            title: &title,
            description: existing.description.as_deref(),
            schedule_json: &existing.schedule_json,
            action_json: &existing.action_json,
            status: STATUS_PAUSED,
            next_run_at: None,
            metadata_json: Some(&existing.metadata_json),
        })
        .map_err(ApiError::from_workspace_error)?;

    Ok(Json(ScheduledTaskResponse {
        task: scheduled_task_view(workspace, &database, task)?,
    }))
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
    Query(query): Query<ScheduledTaskRunsQuery>,
) -> Result<Json<ScheduledTaskRunsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    require_scheduled_task(&database, &task_id)?;
    let (page, page_size, offset) = pagination(
        query.page,
        query.page_size,
        DEFAULT_RUN_PAGE_SIZE,
        MAX_RUN_PAGE_SIZE,
    )?;
    let total_count = usize::try_from(
        database
            .scheduled_task_run_count(&task_id)
            .map_err(ApiError::from_workspace_error)?,
    )
    .map_err(|_| ApiError::internal("scheduled task run count is too large"))?;
    let runs = database
        .scheduled_task_runs_for_task_page(&task_id, page_size as i64, offset as i64)
        .map_err(ApiError::from_workspace_error)?;
    let mut views = Vec::with_capacity(runs.len());
    for run in runs {
        let run = crate::scheduled_tasks::scheduler::sync_scheduled_task_run(&mut database, run)?;
        views.push(scheduled_task_run_view(&workspace.id, run)?);
    }

    Ok(Json(ScheduledTaskRunsResponse {
        runs: views,
        page,
        page_size,
        total_count,
        total_pages: total_pages(total_count, page_size),
    }))
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

/// Opens a workspace DB for cross-workspace scheduled-task aggregation.
///
/// When `require_path` is false (all-workspace list), missing/invalid local
/// directories and remote workspaces are skipped so one stale configured path
/// cannot fail the entire scheduled-tasks list UI. Explicit single-workspace
/// filters still surface clear errors (missing path or remote unsupported).
fn open_scheduled_task_workspace_database(
    workspace: &WorkspaceConfig,
    require_path: bool,
) -> Result<Option<WorkspaceDatabaseHandle>, ApiError> {
    if workspace.is_remote() {
        // Local process has no SQLite for remote workspaces; remote scheduled tasks
        // are not supported on this aggregate path.
        if require_path {
            return Err(ApiError::bad_request(
                "scheduled tasks are not available for remote workspaces",
            ));
        }
        return Ok(None);
    }
    match WorkspaceDatabase::open_or_create(&workspace.path) {
        Ok(database) => Ok(Some(database)),
        Err(WorkspaceDatabaseError::WorkspaceNotDirectory { path }) if !require_path => {
            tracing::debug!(
                workspace_id = %workspace.id,
                workspace_path = %path.display(),
                "skipping scheduled task list for workspace whose path does not exist or is not a directory"
            );
            Ok(None)
        }
        // Explicit filter: stale configured path is a client-facing config problem, not a 500.
        Err(WorkspaceDatabaseError::WorkspaceNotDirectory { path }) => {
            Err(ApiError::bad_request(format!(
                "workspace path does not exist or is not a directory: {}",
                path.display()
            )))
        }
        Err(error) => Err(ApiError::from_workspace_error(error)),
    }
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
    scheduled_task_view_with_usage(workspace, task, usage)
}

fn scheduled_task_view_with_usage(
    workspace: &WorkspaceConfig,
    task: ScheduledTaskRecord,
    usage: LlmRequestAuditSummaryRow,
) -> Result<ScheduledTaskView, ApiError> {
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

fn scheduled_workspace_matches_search(workspace: &WorkspaceConfig, search: &str) -> bool {
    let search = search.to_ascii_lowercase();
    workspace.id.to_ascii_lowercase().contains(&search)
        || workspace.name.to_ascii_lowercase().contains(&search)
}

fn compare_scheduled_tasks(
    left: &ScheduledTaskRecord,
    right: &ScheduledTaskRecord,
) -> std::cmp::Ordering {
    left.next_run_at
        .is_none()
        .cmp(&right.next_run_at.is_none())
        .then_with(|| left.next_run_at.cmp(&right.next_run_at))
        .then_with(|| right.updated_at.cmp(&left.updated_at))
        .then_with(|| left.id.cmp(&right.id))
}

fn pagination(
    page: Option<usize>,
    page_size: Option<usize>,
    default_page_size: usize,
    max_page_size: usize,
) -> Result<(usize, usize, usize), ApiError> {
    let page = page.unwrap_or(1);
    let page_size = page_size.unwrap_or(default_page_size).min(max_page_size);
    if page == 0 || page_size == 0 {
        return Err(ApiError::bad_request("page and pageSize must be positive"));
    }
    let offset = page
        .checked_sub(1)
        .and_then(|page_index| page_index.checked_mul(page_size))
        .ok_or_else(|| ApiError::bad_request("pagination offset is too large"))?;
    Ok((page, page_size, offset))
}

fn total_pages(total_count: usize, page_size: usize) -> usize {
    if total_count == 0 {
        0
    } else {
        (total_count + page_size - 1) / page_size
    }
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

fn normalize_initial_task_status(field: &str, status: &str) -> Result<String, ApiError> {
    let status = status.trim();
    match status {
        STATUS_ENABLED | STATUS_PAUSED => Ok(status.to_string()),
        _ => Err(ApiError::bad_request(format!(
            "{field} must be one of enabled, paused"
        ))),
    }
}

fn task_next_run_at(schedule: &ScheduleSpec, status: &str) -> Result<Option<String>, ApiError> {
    if status != STATUS_ENABLED {
        return Ok(None);
    }

    preview_next_run(PreviewNextRunRequest {
        count: None,
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

        let metadata = scheduled_task_metadata_json(
            "workspace-1",
            None,
            Some(ScheduledConcurrencyPolicy::ForceRun),
            None,
        )
        .expect("force metadata json");
        let value: Value = serde_json::from_str(&metadata).expect("force metadata value");

        assert_eq!(value["concurrencyPolicy"], "force_run");
    }

    #[test]
    fn initial_task_status_only_allows_runnable_states() {
        assert_eq!(
            normalize_initial_task_status("status", "enabled").expect("enabled"),
            STATUS_ENABLED
        );
        assert_eq!(
            normalize_initial_task_status("status", "paused").expect("paused"),
            STATUS_PAUSED
        );
        assert!(normalize_initial_task_status("status", "completed").is_err());
        assert!(normalize_initial_task_status("status", "archived").is_err());
    }
}
