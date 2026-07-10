use std::path::Path;

use foco_store::workspace::{
    NewPlan, NewPlanPhase, NewPlanStep, PlanListFilter, PlanListOrder, PlanPatch, PlanRecord,
    PlanStepPatch, WorkspaceDatabase,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    BuiltinToolContext, DEFAULT_PLAN_TOOL_TIMEOUT_MS,
    errors::{ToolRuntimeError, tool_timeout_ms},
    parse_arguments,
};

pub(crate) fn create_plan(
    workspace_path: &Path,
    context: BuiltinToolContext<'_>,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: CreatePlanInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_PLAN_TOOL_TIMEOUT_MS)?;
    validate_plan_mode_create_status(context, request.status.as_deref())?;
    let mut database = open_plan_database(workspace_path)?;
    let phase_storage = request
        .phases
        .into_iter()
        .map(|phase| CreatePhaseStorage {
            id: phase.id,
            title: phase.title,
            summary: phase.summary.unwrap_or_default(),
            steps: phase
                .steps
                .into_iter()
                .map(|step| CreateStepStorage {
                    id: step.id,
                    title: step.title,
                    detail: step.detail.unwrap_or_default(),
                    acceptance: step.acceptance,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
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
        .collect();
    let source_chat_id = context
        .chat_id
        .or(request.source_chat_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let plan = database.create_plan(NewPlan {
        id: request.id.as_str(),
        title: request.title.as_str(),
        overview: request.overview.as_str(),
        status: request.status.as_deref().unwrap_or("ready"),
        source_chat_id,
        phases,
    })?;

    Ok(plan_json(plan, timeout_ms))
}

pub(crate) fn get_plans(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GetPlansInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_PLAN_TOOL_TIMEOUT_MS)?;
    let view = request.view.unwrap_or_else(|| "active".to_string());
    let page = request.page.unwrap_or(1).max(1);
    let page_size = request
        .page_size
        .or(request.limit)
        .unwrap_or(10)
        .clamp(1, 10);
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    let database = open_plan_database(workspace_path)?;
    let page_record = database.plans(PlanListFilter {
        view: view.trim(),
        status: request
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        order: PlanListOrder::NewestFirst,
        limit: page_size,
        offset,
    })?;

    Ok(json!({
        "plans": page_record.plans,
        "page": page,
        "pageSize": page_size,
        "totalCount": page_record.total_count,
        "totalPages": total_pages(page_record.total_count, page_size),
        "timeoutMs": timeout_ms
    }))
}

pub(crate) fn update_plan(
    workspace_path: &Path,
    context: BuiltinToolContext<'_>,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: UpdatePlanInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_PLAN_TOOL_TIMEOUT_MS)?;
    validate_plan_mode_update_plan(context, &request)?;
    let mut database = open_plan_database(workspace_path)?;
    let error_message = request.error_message.as_deref().map(|message| {
        if message.trim().is_empty() {
            None
        } else {
            Some(message)
        }
    });
    let plan = database.update_plan(
        &request.plan_id,
        PlanPatch {
            title: request.title.as_deref(),
            overview: request.overview.as_deref(),
            status: request.status.as_deref(),
            error_message,
        },
    )?;

    Ok(plan_json(plan, timeout_ms))
}

pub(crate) fn update_plan_step(
    workspace_path: &Path,
    context: BuiltinToolContext<'_>,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: UpdatePlanStepInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_PLAN_TOOL_TIMEOUT_MS)?;
    validate_plan_mode_update_plan_step(context, &request)?;
    let mut database = open_plan_database(workspace_path)?;
    let plan = database.update_plan_step(
        &request.plan_id,
        &request.step_id,
        PlanStepPatch {
            title: request.title.as_deref(),
            detail: request.detail.as_deref(),
            acceptance: request.acceptance,
            status: request.status.as_deref(),
        },
    )?;

    Ok(plan_json(plan, timeout_ms))
}

pub(crate) fn delete_plan(
    workspace_path: &Path,
    context: BuiltinToolContext<'_>,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: DeletePlanInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_PLAN_TOOL_TIMEOUT_MS)?;
    let chat_id = context
        .chat_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ToolRuntimeError::InvalidArguments(
                "delete_plan requires the current chat id to check plan ownership".to_string(),
            )
        })?;
    let mut database = open_plan_database(workspace_path)?;
    let plan = database.plan(&request.plan_id)?.ok_or_else(|| {
        ToolRuntimeError::InvalidArguments(format!(
            "plan was not found: {}",
            request.plan_id.trim()
        ))
    })?;

    if plan.source_chat_id.as_deref() != Some(chat_id) {
        return Err(ToolRuntimeError::InvalidArguments(
            "delete_plan can only delete plans created by the current chat".to_string(),
        ));
    }

    if !database.delete_plan(&request.plan_id)? {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "plan was not found: {}",
            request.plan_id.trim()
        )));
    }

    Ok(json!({
        "deleted": true,
        "planId": request.plan_id,
        "timeoutMs": timeout_ms
    }))
}

fn validate_plan_mode_create_status(
    context: BuiltinToolContext<'_>,
    status: Option<&str>,
) -> Result<(), ToolRuntimeError> {
    if context.is_plan_mode()
        && status
            .map(str::trim)
            .is_some_and(|status| status != "ready")
    {
        return Err(plan_mode_status_error());
    }

    Ok(())
}

fn validate_plan_mode_update_plan(
    context: BuiltinToolContext<'_>,
    request: &UpdatePlanInput,
) -> Result<(), ToolRuntimeError> {
    if context.is_plan_mode() && (request.status.is_some() || request.error_message.is_some()) {
        return Err(plan_mode_status_error());
    }

    Ok(())
}

fn validate_plan_mode_update_plan_step(
    context: BuiltinToolContext<'_>,
    request: &UpdatePlanStepInput,
) -> Result<(), ToolRuntimeError> {
    if context.is_plan_mode() && request.status.is_some() {
        return Err(plan_mode_status_error());
    }

    Ok(())
}

fn plan_mode_status_error() -> ToolRuntimeError {
    ToolRuntimeError::InvalidArguments(
        "Plan Mode cannot modify plan, phase, or step status; status is updated by the execution flow, phase outcomes, or manual UI actions".to_string(),
    )
}

fn open_plan_database(workspace_path: &Path) -> Result<WorkspaceDatabase, ToolRuntimeError> {
    WorkspaceDatabase::open_or_create(workspace_path).map_err(ToolRuntimeError::WorkspaceDatabase)
}

fn plan_json(plan: PlanRecord, timeout_ms: u64) -> Value {
    json!({
        "plan": plan,
        "timeoutMs": timeout_ms
    })
}

fn total_pages(total_count: i64, page_size: i64) -> i64 {
    if total_count == 0 {
        0
    } else {
        (total_count + page_size - 1) / page_size
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePlanInput {
    id: String,
    title: String,
    overview: String,
    status: Option<String>,
    source_chat_id: Option<String>,
    phases: Vec<CreatePlanPhaseInput>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePlanPhaseInput {
    id: String,
    title: String,
    summary: Option<String>,
    steps: Vec<CreatePlanStepInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePlanStepInput {
    id: String,
    title: String,
    detail: Option<String>,
    #[serde(default)]
    acceptance: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetPlansInput {
    view: Option<String>,
    status: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
    limit: Option<i64>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePlanInput {
    plan_id: String,
    title: Option<String>,
    overview: Option<String>,
    status: Option<String>,
    error_message: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePlanStepInput {
    plan_id: String,
    step_id: String,
    title: Option<String>,
    detail: Option<String>,
    acceptance: Option<Vec<String>>,
    status: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeletePlanInput {
    plan_id: String,
    timeout_ms: Option<u64>,
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
