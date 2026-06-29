use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
};
use foco_store::workspace::{
    NewPlan, NewPlanPhase, NewPlanStep, PlanListFilter, PlanPatch, PlanPhaseRecord, PlanRecord,
    PlanStepPatch, PlanStepRecord, PlanWorktreeAuditRecord, WorkspaceDatabase,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::{
    git_backend::{agent_worktree_head_commit, delete_agent_worktree},
    *,
};

const DEFAULT_ACTIVE_PLAN_LIMIT: i64 = 50;
const DEFAULT_PLAN_PAGE_SIZE: i64 = 20;
const MAX_PLAN_PAGE_SIZE: i64 = 100;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlansQuery {
    view: Option<String>,
    status: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreatePlanRequest {
    id: Option<String>,
    title: String,
    overview: String,
    status: Option<String>,
    source_chat_id: Option<String>,
    phases: Vec<CreatePlanPhaseRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreatePlanPhaseRequest {
    id: Option<String>,
    title: String,
    summary: Option<String>,
    steps: Vec<CreatePlanStepRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreatePlanStepRequest {
    id: Option<String>,
    title: String,
    detail: Option<String>,
    #[serde(default)]
    acceptance: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UpdatePlanRequest {
    title: Option<String>,
    overview: Option<String>,
    status: Option<String>,
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PlanActionRequest {
    action: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CleanupPlanWorktreeRequest {
    agent_instance_id: String,
    confirm: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlansResponse {
    plans: Vec<PlanSummary>,
    page: i64,
    page_size: i64,
    total_count: i64,
    total_pages: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanResponse {
    plan: PlanSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeletePlanResponse {
    deleted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanWorktreeAuditResponse {
    items: Vec<PlanWorktreeAuditItem>,
    recovery_note: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanWorktreeCleanupResponse {
    deleted: bool,
    item: PlanWorktreeAuditItem,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanWorktreeAuditItem {
    plan_id: String,
    plan_status: String,
    phase_id: String,
    phase_status: String,
    implementation_chat_id: Option<String>,
    agent_task_id: Option<String>,
    agent_task_status: Option<String>,
    agent_instance_id: String,
    worktree_path: String,
    base_revision: Option<String>,
    branch: Option<String>,
    ref_name: Option<String>,
    worktree_status: Option<String>,
    commit_id: Option<String>,
    head_commit_id: Option<String>,
    head_commit_short: Option<String>,
    error_message: Option<String>,
    cleanup_allowed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanSummary {
    id: String,
    title: String,
    overview: String,
    status: String,
    sort_order: i64,
    source_chat_id: Option<String>,
    active_phase_id: Option<String>,
    pause_requested_at: Option<String>,
    completed_at: Option<String>,
    completed_by_user_at: Option<String>,
    error_message: Option<String>,
    shared_merge_commit_id: Option<String>,
    created_at: String,
    updated_at: String,
    phases: Vec<PlanPhaseSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanPhaseSummary {
    id: String,
    plan_id: String,
    sequence: i64,
    title: String,
    summary: String,
    status: String,
    implementation_chat_id: Option<String>,
    agent_team_id: Option<String>,
    agent_task_id: Option<String>,
    commit_id: Option<String>,
    merge_attempt_count: i64,
    error_message: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
    created_at: String,
    updated_at: String,
    steps: Vec<PlanStepSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanStepSummary {
    id: String,
    plan_id: String,
    phase_id: String,
    sequence: i64,
    title: String,
    detail: String,
    acceptance: Vec<String>,
    status: String,
    checked_at: Option<String>,
    created_at: String,
    updated_at: String,
}

pub(crate) async fn plans(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Query(query): Query<PlansQuery>,
) -> Result<Json<PlansResponse>, ApiError> {
    let view = query.view.unwrap_or_else(|| "active".to_string());
    let page = query.page.unwrap_or(1).max(1);
    let page_size = if view == "active" {
        query.limit.unwrap_or(DEFAULT_ACTIVE_PLAN_LIMIT)
    } else {
        query
            .page_size
            .or(query.limit)
            .unwrap_or(DEFAULT_PLAN_PAGE_SIZE)
    }
    .clamp(1, MAX_PLAN_PAGE_SIZE);
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let page_record = database
        .plans(PlanListFilter {
            view: view.trim(),
            status: query
                .status
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            limit: page_size,
            offset,
        })
        .map_err(ApiError::from_workspace_error)?;

    Ok(Json(PlansResponse {
        plans: page_record.plans.into_iter().map(plan_summary).collect(),
        page,
        page_size,
        total_count: page_record.total_count,
        total_pages: total_pages(page_record.total_count, page_size),
    }))
}

pub(crate) async fn create_plan(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<CreatePlanRequest>,
) -> Result<Json<PlanResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let phase_storage = request
        .phases
        .into_iter()
        .enumerate()
        .map(|(phase_index, phase)| {
            let step_storage = phase
                .steps
                .into_iter()
                .enumerate()
                .map(|(step_index, step)| CreateStepStorage {
                    id: step.id.unwrap_or_else(|| {
                        unique_id(&format!("plan-step-{phase_index}-{step_index}"))
                    }),
                    title: step.title,
                    detail: step.detail.unwrap_or_default(),
                    acceptance: step.acceptance,
                })
                .collect::<Vec<_>>();
            CreatePhaseStorage {
                id: phase
                    .id
                    .unwrap_or_else(|| unique_id(&format!("plan-phase-{phase_index}"))),
                title: phase.title,
                summary: phase.summary.unwrap_or_default(),
                steps: step_storage,
            }
        })
        .collect::<Vec<_>>();
    let plan_id = request.id.unwrap_or_else(|| unique_id("plan"));
    let phases = phase_storage
        .iter()
        .map(|phase| NewPlanPhase {
            id: phase.id.as_str(),
            title: phase.title.as_str(),
            summary: phase.summary.as_str(),
            steps: phase
                .steps
                .iter()
                .map(|step| NewPlanStep {
                    id: step.id.as_str(),
                    title: step.title.as_str(),
                    detail: step.detail.as_str(),
                    acceptance: step.acceptance.clone(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let plan = database
        .create_plan(NewPlan {
            id: plan_id.as_str(),
            title: request.title.as_str(),
            overview: request.overview.as_str(),
            status: request.status.as_deref().unwrap_or("ready"),
            source_chat_id: request.source_chat_id.as_deref(),
            phases,
        })
        .map_err(ApiError::from_workspace_error)?;

    Ok(Json(PlanResponse {
        plan: plan_summary(plan),
    }))
}

pub(crate) async fn update_plan(
    State(state): State<AppState>,
    AxumPath((workspace_id, plan_id)): AxumPath<(String, String)>,
    Json(request): Json<UpdatePlanRequest>,
) -> Result<Json<PlanResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let error_message = request.error_message.as_deref().map(|message| {
        if message.trim().is_empty() {
            None
        } else {
            Some(message)
        }
    });
    let plan = database
        .update_plan(
            &plan_id,
            PlanPatch {
                title: request.title.as_deref(),
                overview: request.overview.as_deref(),
                status: request.status.as_deref(),
                error_message,
            },
        )
        .map_err(ApiError::from_workspace_error)?;

    Ok(Json(PlanResponse {
        plan: plan_summary(plan),
    }))
}

pub(crate) async fn delete_plan(
    State(state): State<AppState>,
    AxumPath((workspace_id, plan_id)): AxumPath<(String, String)>,
) -> Result<Json<DeletePlanResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let deleted = database
        .delete_plan(&plan_id)
        .map_err(ApiError::from_workspace_error)?;
    if !deleted {
        return Err(ApiError::bad_request(format!(
            "plan was not found: {}",
            plan_id.trim()
        )));
    }

    Ok(Json(DeletePlanResponse { deleted }))
}

pub(crate) async fn plan_worktree_audit(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Json<PlanWorktreeAuditResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let records = database
        .plan_worktree_audit()
        .map_err(ApiError::from_workspace_error)?;
    let items = records.into_iter().map(plan_worktree_audit_item).collect();

    Ok(Json(PlanWorktreeAuditResponse {
        items,
        recovery_note: PLAN_WORKTREE_RECOVERY_NOTE,
    }))
}

pub(crate) async fn cleanup_plan_worktree(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<CleanupPlanWorktreeRequest>,
) -> Result<Json<PlanWorktreeCleanupResponse>, ApiError> {
    if !request.confirm {
        return Err(ApiError::bad_request(
            "plan worktree cleanup requires confirm=true",
        ));
    }
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let instance_id = request.agent_instance_id.trim();
    let record = database
        .plan_worktree_audit()
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .find(|item| item.agent_instance_id.as_str() == instance_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "plan worktree audit item was not found: {instance_id}"
            ))
        })?;
    let item = plan_worktree_audit_item(record.clone());
    if !item.cleanup_allowed {
        return Err(ApiError::bad_request(
            "plan worktree audit item is not eligible for cleanup",
        ));
    }
    let instance = database
        .agent_instance(&record.agent_instance_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!("Agent instance was not found: {instance_id}"))
        })?;
    let root_path = instance.execution_root_path.as_deref().ok_or_else(|| {
        ApiError::bad_request("Agent instance no longer has an isolated worktree")
    })?;
    if root_path != record.worktree_path {
        return Err(ApiError::bad_request(
            "Agent worktree audit record is stale; refresh before cleanup",
        ));
    }

    delete_agent_worktree(&workspace.path, Path::new(root_path), true)?;
    database
        .switch_agent_instance_to_shared_workspace(&record.agent_instance_id)
        .map_err(ApiError::from_workspace_error)?;

    Ok(Json(PlanWorktreeCleanupResponse {
        deleted: true,
        item,
    }))
}

pub(crate) async fn plan_action(
    State(state): State<AppState>,
    AxumPath((workspace_id, plan_id)): AxumPath<(String, String)>,
    Json(request): Json<PlanActionRequest>,
) -> Result<Json<PlanResponse>, ApiError> {
    let plan = crate::plan_runtime::transition_plan_action(
        &state,
        &workspace_id,
        &plan_id,
        &request.action,
    )
    .await?;

    Ok(Json(PlanResponse {
        plan: plan_summary(plan),
    }))
}

pub(crate) async fn plan_step_action(
    State(state): State<AppState>,
    AxumPath((workspace_id, plan_id, step_id)): AxumPath<(String, String, String)>,
    Json(request): Json<PlanActionRequest>,
) -> Result<Json<PlanResponse>, ApiError> {
    let status = match request.action.trim() {
        "check" | "complete" => "completed",
        "uncheck" | "reset" => "pending",
        "start" => "running",
        "fail" => "failed",
        "cancel" => "cancelled",
        action => {
            return Err(ApiError::bad_request(format!(
                "invalid plan step action: {action}"
            )));
        }
    };
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let plan = database
        .update_plan_step(
            &plan_id,
            &step_id,
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some(status),
            },
        )
        .map_err(ApiError::from_workspace_error)?;

    Ok(Json(PlanResponse {
        plan: plan_summary(plan),
    }))
}

struct CreatePhaseStorage {
    id: String,
    title: String,
    summary: String,
    steps: Vec<CreateStepStorage>,
}

struct CreateStepStorage {
    id: String,
    title: String,
    detail: String,
    acceptance: Vec<String>,
}

const PLAN_WORKTREE_RECOVERY_NOTE: &str = "Plans without sharedMergeCommitId were not merged into the shared workspace. Historical implemented/completed plans in this state need manual cherry-pick/merge from the listed worktree branch or a rerun; Foco does not auto cherry-pick historical commits.";

fn plan_worktree_audit_item(record: PlanWorktreeAuditRecord) -> PlanWorktreeAuditItem {
    let (head_commit_id, head_error) = agent_worktree_head_commit(Path::new(&record.worktree_path))
        .map(|commit| (Some(commit), None))
        .unwrap_or_else(|error| (None, Some(error.message().to_string())));
    let head_commit_short = head_commit_id.as_deref().map(short_commit_id);
    let cleanup_allowed = !matches!(record.plan_status.as_str(), "running")
        && !matches!(record.phase_status.as_str(), "running");

    PlanWorktreeAuditItem {
        plan_id: record.plan_id,
        plan_status: record.plan_status,
        phase_id: record.phase_id,
        phase_status: record.phase_status,
        implementation_chat_id: record.implementation_chat_id,
        agent_task_id: record.agent_task_id,
        agent_task_status: record.agent_task_status,
        agent_instance_id: record.agent_instance_id.to_string(),
        worktree_path: record.worktree_path,
        base_revision: record.base_revision,
        ref_name: record.branch.as_deref().map(branch_ref_name),
        branch: record.branch,
        worktree_status: record.worktree_status,
        commit_id: record.commit_id,
        head_commit_id,
        head_commit_short,
        error_message: first_non_empty([
            record.plan_error_message,
            record.phase_error_message,
            record.task_error_message.and_then(task_error_message),
            head_error,
        ]),
        cleanup_allowed,
    }
}

fn task_error_message(error_json: String) -> Option<String> {
    serde_json::from_str::<Value>(&error_json)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .or_else(|| value.get("error"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| Some(error_json))
}

fn first_non_empty(values: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn short_commit_id(commit_id: &str) -> String {
    commit_id.chars().take(7).collect()
}

fn branch_ref_name(branch: &str) -> String {
    format!("refs/heads/{branch}")
}

fn plan_summary(plan: PlanRecord) -> PlanSummary {
    PlanSummary {
        id: plan.id,
        title: plan.title,
        overview: plan.overview,
        status: plan.status,
        sort_order: plan.sort_order,
        source_chat_id: plan.source_chat_id,
        active_phase_id: plan.active_phase_id,
        pause_requested_at: plan.pause_requested_at,
        completed_at: plan.completed_at,
        completed_by_user_at: plan.completed_by_user_at,
        error_message: plan.error_message,
        shared_merge_commit_id: plan.shared_merge_commit_id,
        created_at: plan.created_at,
        updated_at: plan.updated_at,
        phases: plan.phases.into_iter().map(plan_phase_summary).collect(),
    }
}

fn plan_phase_summary(phase: PlanPhaseRecord) -> PlanPhaseSummary {
    PlanPhaseSummary {
        id: phase.id,
        plan_id: phase.plan_id,
        sequence: phase.sequence,
        title: phase.title,
        summary: phase.summary,
        status: phase.status,
        implementation_chat_id: phase.implementation_chat_id,
        agent_team_id: phase.agent_team_id,
        agent_task_id: phase.agent_task_id,
        commit_id: phase.commit_id,
        merge_attempt_count: phase.merge_attempt_count,
        error_message: phase.error_message,
        started_at: phase.started_at,
        completed_at: phase.completed_at,
        created_at: phase.created_at,
        updated_at: phase.updated_at,
        steps: phase.steps.into_iter().map(plan_step_summary).collect(),
    }
}

fn plan_step_summary(step: PlanStepRecord) -> PlanStepSummary {
    PlanStepSummary {
        id: step.id,
        plan_id: step.plan_id,
        phase_id: step.phase_id,
        sequence: step.sequence,
        title: step.title,
        detail: step.detail,
        acceptance: step.acceptance,
        status: step.status,
        checked_at: step.checked_at,
        created_at: step.created_at,
        updated_at: step.updated_at,
    }
}

fn total_pages(total_count: i64, page_size: i64) -> i64 {
    if total_count == 0 {
        0
    } else {
        (total_count + page_size - 1) / page_size
    }
}
