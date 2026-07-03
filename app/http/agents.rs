use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
};
use foco_agent::{
    AgentAttemptId, AgentDefinitionId, AgentExecutionWorkspaceMode, AgentInstanceId,
    AgentInstanceStatus, AgentMessageId, AgentRole, AgentTaskId, AgentTaskStatus,
    AgentTaskTransition, AgentTeamId, AgentTeamStatus, TeamActivationRequest, TeamWorkload,
    ToolResource, ToolResourceAccess, ToolResourceLock,
};
use foco_store::workspace::{
    AgentEventRecord, AgentInstanceRecord, AgentMessageRecord, AgentTaskDependencyRecord,
    AgentTaskRecord, AgentTaskStateUpdate, AgentTeamRecord, NewAgentInstance, NewAgentTeam,
    RunEventRecord, WorkspaceDatabase,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::LazyLock,
};

static AGENT_URL_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"https?://[^\s<>\"']+"#).expect("valid Agent URL redaction regex")
});
static AGENT_SECRET_ASSIGNMENT_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r#"(?i)(?P<key>\b(?:authorization|api[-_ ]?key|password|cookie|access[-_ ]?token|refresh[-_ ]?token|client[-_ ]?secret)\b)\s*[:=]\s*(?:bearer\s+)?[^,\s;]+"#,
    )
    .expect("valid Agent secret redaction regex")
});
const AGENT_REDACTED_VALUE: &str = "<redacted>";

use crate::git_backend::{
    agent_instance_worktree_path, agent_instance_worktree_relative_path, agent_worktree_diff_id,
    create_agent_worktree, delete_agent_worktree, git_diff_response, git_status_response,
    merge_agent_worktree, resolve_agent_worktree_path,
};
use crate::runtime::{
    AGENT_MAX_CREATE_INSTANCES_PER_REQUEST, AGENT_MAX_INSTANCES_PER_TEAM,
    ToolResourceLockOwnerSnapshot,
};
use crate::*;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Clone, Copy)]
enum AgentRuntimeScope {
    Team,
    Instance,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Clone, Copy)]
enum AgentRuntimeAction {
    Pause,
    Resume,
    Drain,
    Stop,
    Delete,
    ResetContext,
    WorktreeStatus,
    WorktreeDiff,
    WorktreeKeep,
    WorktreeArchive,
    WorktreeDelete,
    WorktreeMerge,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentRuntimeActionRequest {
    scope: AgentRuntimeScope,
    action: AgentRuntimeAction,
    instance_id: Option<AgentInstanceId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentCreateInstancesRequest {
    definition_id: AgentDefinitionId,
    count: u32,
    max_instances_per_team: u32,
    max_instances_for_definition: u32,
    #[serde(default)]
    execution_workspace_mode: AgentExecutionWorkspaceMode,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AgentTaskAction {
    Cancel,
    Retry,
    Transfer,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentTaskActionRequest {
    action: AgentTaskAction,
    target_instance_id: Option<AgentInstanceId>,
    cascade: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentTeamSnapshotResponse {
    team: AgentTeamView,
    workload: TeamWorkload,
    observability: AgentObservabilityView,
    instances: Vec<AgentInstanceView>,
    tasks: Vec<AgentTaskView>,
    dependencies: Vec<AgentTaskDependencyView>,
    messages: Vec<AgentMessageView>,
    events: Vec<AgentEventView>,
    run_events: Vec<AgentRunEventView>,
    mutation_lease_owners: Vec<AgentMutationLeaseOwnerView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    worktree_action: Option<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentTranscriptQuery {
    page: Option<usize>,
    page_size: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentTranscriptResponse {
    items: Vec<AgentTranscriptItemView>,
    page: usize,
    page_size: usize,
    total_count: usize,
    total_pages: usize,
    has_more: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentTranscriptItemView {
    pub(crate) id: String,
    pub(crate) author: String,
    pub(crate) role: AgentTranscriptRole,
    pub(crate) kind: String,
    pub(crate) created_at: String,
    pub(crate) task_status: Option<AgentTaskStatus>,
    pub(crate) content: String,
    pub(crate) parts: Vec<ChatMessagePart>,
    pub(crate) metrics: Option<ChatReplyMetrics>,
    pub(crate) status: Option<AgentTranscriptItemStatus>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AgentTranscriptRole {
    Assistant,
    User,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AgentTranscriptItemStatus {
    Error,
    Streaming,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentObservabilityView {
    queue_length: i64,
    queue_wait_ms: AgentMetricSummaryView,
    run_duration_ms: AgentMetricSummaryView,
    scheduler_latency_ms: AgentMetricSummaryView,
    mutation_lease_wait_ms: AgentMetricSummaryView,
    failed_tasks: usize,
    cancelled_tasks: usize,
    interrupted_tasks: usize,
    failures_by_type: Vec<AgentFailureClassView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentMetricSummaryView {
    count: usize,
    max: Option<i64>,
    average: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentFailureClassView {
    kind: &'static str,
    count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentTeamView {
    id: AgentTeamId,
    chat_id: String,
    coordinator_instance_id: AgentInstanceId,
    status: AgentTeamStatus,
    max_concurrent_runs: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentInstanceView {
    id: AgentInstanceId,
    team_id: AgentTeamId,
    definition_id: AgentDefinitionId,
    definition_revision: u64,
    definition_snapshot: AgentDefinitionRuntimeView,
    role: foco_agent::AgentRole,
    status: AgentInstanceStatus,
    next_task_sequence: i64,
    context_generation: i64,
    last_scheduled_at: Option<String>,
    execution_workspace_mode: AgentExecutionWorkspaceMode,
    execution_root_path: Option<String>,
    worktree_base_revision: Option<String>,
    worktree_branch: Option<String>,
    worktree_status: Option<String>,
    created_at: String,
    updated_at: String,
}

struct CreatedAgentWorktree {
    root_path: String,
    base_revision: String,
    branch: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentTaskView {
    id: AgentTaskId,
    team_id: AgentTeamId,
    owner_instance_id: AgentInstanceId,
    origin_instance_id: Option<AgentInstanceId>,
    parent_task_id: Option<AgentTaskId>,
    sequence: i64,
    status: AgentTaskStatus,
    input: serde_json::Value,
    result: Option<serde_json::Value>,
    error: Option<serde_json::Value>,
    attempts: Vec<AgentAttemptView>,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentAttemptView {
    id: AgentAttemptId,
    sequence: i64,
    status: foco_agent::AgentAttemptStatus,
    started_at: String,
    completed_at: Option<String>,
    interruption_reason: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentDefinitionRuntimeView {
    id: AgentDefinitionId,
    revision: u64,
    name: String,
    description: String,
    provider_id: String,
    model_id: String,
    model_options: AgentModelOptions,
    allowed_tools: Vec<String>,
    max_instances: u32,
    allowed_execution_workspace_modes: Vec<AgentExecutionWorkspaceMode>,
    permissions: foco_agent::AgentPermissions,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentTaskDependencyView {
    team_id: AgentTeamId,
    waiting_task_id: AgentTaskId,
    dependency_task_id: AgentTaskId,
    wait_mode: foco_agent::AgentTaskWaitMode,
    pending_tool_call_id: Option<String>,
    deadline_at: Option<String>,
    created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentMessageView {
    id: AgentMessageId,
    team_id: AgentTeamId,
    sender_instance_id: Option<AgentInstanceId>,
    receiver_instance_id: AgentInstanceId,
    related_task_id: Option<AgentTaskId>,
    reply_to_message_id: Option<AgentMessageId>,
    kind: foco_agent::AgentMessageKind,
    content: String,
    sequence: i64,
    created_at: String,
    consumed_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentEventView {
    team_id: AgentTeamId,
    sequence: i64,
    event_type: String,
    instance_id: Option<AgentInstanceId>,
    task_id: Option<AgentTaskId>,
    attempt_id: Option<AgentAttemptId>,
    message_id: Option<AgentMessageId>,
    payload: serde_json::Value,
    created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentRunEventView {
    run_id: String,
    sequence: i64,
    event_type: String,
    payload: serde_json::Value,
    created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentMutationLeaseOwnerView {
    instance_id: Option<String>,
    task_id: Option<String>,
    tool_call_id: Option<String>,
    tool_name: Option<String>,
    active_ms: u64,
    wait_ms: u64,
}

pub(crate) async fn enable_agent_team(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
    Json(request): Json<TeamActivationRequest>,
) -> Result<Json<AgentTeamSnapshotResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let definition = config
        .agent_definitions
        .iter()
        .find(|definition| definition.id == request.coordinator_definition_id);
    request
        .validate_definition(definition.is_some())
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let definition = definition.expect("validated Coordinator definition");
    validate_agent_snapshot_for_workspace(&config, workspace, definition)?;

    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    if state
        .active_chat_runs
        .active_run_for_chat(&workspace_id, &chat_id)?
        .is_some()
    {
        return Err(ApiError::bad_request(
            "cannot enable an Agent team while the chat has an active run",
        ));
    }
    if database
        .chat(&chat_id)
        .map_err(ApiError::from_workspace_error)?
        .is_none()
    {
        return Err(ApiError::bad_request(format!(
            "chat was not found: {chat_id}"
        )));
    }
    if database
        .agent_team_for_chat(&chat_id)
        .map_err(ApiError::from_workspace_error)?
        .is_some()
    {
        return Err(ApiError::bad_request(format!(
            "chat '{chat_id}' already has an Agent team"
        )));
    }

    let team_id = AgentTeamId::new(unique_id("agent-team"))
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let instance_id = AgentInstanceId::new(unique_id("agent-instance"))
        .map_err(|error| ApiError::internal(error.to_string()))?;
    database
        .create_agent_team(NewAgentTeam {
            id: &team_id,
            chat_id: &chat_id,
            coordinator_instance_id: &instance_id,
            coordinator_definition: definition,
            coordinator_execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
            coordinator_execution_root_path: None,
            coordinator_worktree_base_revision: None,
            coordinator_worktree_branch: None,
            coordinator_worktree_status: None,
            max_concurrent_runs: DEFAULT_AGENT_TEAM_MAX_CONCURRENT_RUNS,
        })
        .map_err(ApiError::from_workspace_error)?;
    insert_agent_event(
        &mut database,
        &team_id,
        "team_created",
        Some(&instance_id),
        None,
        None,
        json!({ "coordinatorDefinitionId": definition.id }),
    )?;
    state.agent_scheduler.wake()?;
    Ok(Json(agent_team_snapshot_from_database(
        &state,
        &workspace_id,
        &database,
        &team_id,
    )?))
}

pub(crate) async fn agent_team_snapshot(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
) -> Result<Json<AgentTeamSnapshotResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let team = database
        .agent_team_for_chat(&chat_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("chat '{chat_id}' has no Agent team")))?;
    Ok(Json(agent_team_snapshot_from_database(
        &state,
        &workspace_id,
        &database,
        &team.id,
    )?))
}

pub(crate) async fn agent_instance_transcript(
    State(state): State<AppState>,
    AxumPath((workspace_id, instance_id)): AxumPath<(String, String)>,
    Query(query): Query<AgentTranscriptQuery>,
) -> Result<Json<AgentTranscriptResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let instance_id = AgentInstanceId::new(instance_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let instance = database
        .agent_instance(&instance_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!("Agent instance '{instance_id}' was not found"))
        })?;
    let team = database
        .agent_team(&instance.team_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!("Agent team '{}' was not found", instance.team_id))
        })?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(25).clamp(1, 100);
    let items = agent_instance_transcript_items(&database, &team, &instance)?;
    let total_count = items.len();
    let total_pages = total_count.div_ceil(page_size).max(1);
    let offset = (page - 1).saturating_mul(page_size);
    let page_items = items
        .into_iter()
        .skip(offset)
        .take(page_size)
        .collect::<Vec<_>>();

    Ok(Json(AgentTranscriptResponse {
        items: page_items,
        page,
        page_size,
        total_count,
        total_pages,
        has_more: page < total_pages,
    }))
}

pub(crate) async fn create_agent_instances(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
    Json(request): Json<AgentCreateInstancesRequest>,
) -> Result<Json<AgentTeamSnapshotResponse>, ApiError> {
    if request.count == 0 {
        return Err(ApiError::bad_request(
            "count must be greater than 0 when creating Agent instances",
        ));
    }
    if request.count > AGENT_MAX_CREATE_INSTANCES_PER_REQUEST {
        return Err(ApiError::bad_request(format!(
            "count exceeds Agent instance create limit {AGENT_MAX_CREATE_INSTANCES_PER_REQUEST}"
        )));
    }
    if i64::from(request.max_instances_per_team) > AGENT_MAX_INSTANCES_PER_TEAM {
        return Err(ApiError::bad_request(format!(
            "maxInstancesPerTeam exceeds process limit {AGENT_MAX_INSTANCES_PER_TEAM}"
        )));
    }
    if request.count > request.max_instances_per_team
        || request.count > request.max_instances_for_definition
    {
        return Err(ApiError::bad_request(
            "count exceeds the explicit Agent instance limits in the request",
        ));
    }

    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let definition = config
        .agent_definitions
        .iter()
        .find(|definition| definition.id == request.definition_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "Agent definition '{}' was not found",
                request.definition_id
            ))
        })?;
    if request.max_instances_for_definition > definition.max_instances {
        return Err(ApiError::bad_request(format!(
            "maxInstancesForDefinition {} exceeds definition '{}' maxInstances {}",
            request.max_instances_for_definition, definition.id, definition.max_instances
        )));
    }
    if !definition
        .allowed_execution_workspace_modes
        .contains(&request.execution_workspace_mode)
    {
        return Err(ApiError::bad_request(format!(
            "executionWorkspaceMode '{}' is not allowed for Agent definition '{}'",
            request.execution_workspace_mode.as_str(),
            definition.id
        )));
    }
    validate_agent_snapshot_for_workspace(&config, workspace, definition)?;

    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let team = database
        .agent_team_for_chat(&chat_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("chat '{chat_id}' has no Agent team")))?;
    let instance_ids = (0..request.count)
        .map(|_| AgentInstanceId::new(unique_id("agent-instance")))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let worktrees = match request.execution_workspace_mode {
        AgentExecutionWorkspaceMode::Shared => Vec::new(),
        AgentExecutionWorkspaceMode::IsolatedWorktree => instance_ids
            .iter()
            .map(|id| {
                let info = create_agent_worktree(&workspace.path, id.as_str())?;
                Ok(CreatedAgentWorktree {
                    root_path: agent_instance_worktree_relative_path(id),
                    base_revision: info.base_revision,
                    branch: info.branch,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?,
    };
    let instances = instance_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let worktree = worktrees.get(index);
            NewAgentInstance {
                id,
                team_id: &team.id,
                definition,
                role: AgentRole::Worker,
                execution_workspace_mode: request.execution_workspace_mode,
                execution_root_path: worktree.map(|worktree| worktree.root_path.as_str()),
                worktree_base_revision: worktree.map(|worktree| worktree.base_revision.as_str()),
                worktree_branch: worktree.map(|worktree| worktree.branch.as_str()),
                worktree_status: worktree.map(|_| "active"),
            }
        })
        .collect::<Vec<_>>();
    let created = match database.create_agent_instances_with_limits(
        &instances,
        i64::from(request.max_instances_per_team),
        i64::from(request.max_instances_for_definition),
    ) {
        Ok(created) => created,
        Err(error) => {
            for worktree in &worktrees {
                let _ =
                    delete_agent_worktree(&workspace.path, Path::new(&worktree.root_path), true);
            }
            return Err(ApiError::from_workspace_error(error));
        }
    };
    for instance in &created {
        insert_agent_event(
            &mut database,
            &team.id,
            "instance_created",
            Some(&instance.id),
            None,
            None,
            json!({
                "createdBy": "user",
                "definitionId": instance.definition_id,
                "definitionRevision": instance.definition_revision,
                "role": instance.role,
                "status": instance.status,
                "executionWorkspaceMode": instance.execution_workspace_mode,
                "executionRootPath": instance.execution_root_path,
                "worktreeBaseRevision": instance.worktree_base_revision,
                "worktreeBranch": instance.worktree_branch,
                "worktreeStatus": instance.worktree_status,
            }),
        )?;
    }
    state.agent_scheduler.wake()?;
    Ok(Json(agent_team_snapshot_from_database(
        &state,
        &workspace_id,
        &database,
        &team.id,
    )?))
}

pub(crate) async fn agent_runtime_action(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
    Json(request): Json<AgentRuntimeActionRequest>,
) -> Result<Json<AgentTeamSnapshotResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let team = database
        .agent_team_for_chat(&chat_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("chat '{chat_id}' has no Agent team")))?;
    let mut worktree_action = None;

    match request.scope {
        AgentRuntimeScope::Team => {
            let target = match request.action {
                AgentRuntimeAction::Pause => AgentTeamStatus::Paused,
                AgentRuntimeAction::Resume => AgentTeamStatus::Active,
                AgentRuntimeAction::Drain => AgentTeamStatus::Draining,
                AgentRuntimeAction::Stop => AgentTeamStatus::Stopped,
                AgentRuntimeAction::Delete => {
                    return Err(ApiError::bad_request(
                        "Agent teams are stopped, not directly deleted",
                    ));
                }
                AgentRuntimeAction::ResetContext => {
                    return Err(ApiError::bad_request(
                        "resetContext is only valid for Agent instances",
                    ));
                }
                AgentRuntimeAction::WorktreeStatus
                | AgentRuntimeAction::WorktreeDiff
                | AgentRuntimeAction::WorktreeKeep
                | AgentRuntimeAction::WorktreeArchive
                | AgentRuntimeAction::WorktreeDelete
                | AgentRuntimeAction::WorktreeMerge => {
                    return Err(ApiError::bad_request(
                        "worktree actions are only valid for Agent instances",
                    ));
                }
            };
            database
                .transition_agent_team_status(&team.id, target)
                .map_err(ApiError::from_workspace_error)?;
            insert_agent_event(
                &mut database,
                &team.id,
                "team_status_changed",
                None,
                None,
                None,
                json!({ "status": target }),
            )?;
        }
        AgentRuntimeScope::Instance => {
            let instance_id = request.instance_id.ok_or_else(|| {
                ApiError::bad_request("instanceId is required for instance actions")
            })?;
            let instance = database
                .agent_instance(&instance_id)
                .map_err(ApiError::from_workspace_error)?
                .ok_or_else(|| {
                    ApiError::bad_request(format!("Agent instance '{instance_id}' was not found"))
                })?;
            if instance.team_id != team.id {
                return Err(ApiError::bad_request(format!(
                    "Agent instance '{instance_id}' does not belong to team '{}'",
                    team.id
                )));
            }
            if matches!(
                request.action,
                AgentRuntimeAction::WorktreeStatus
                    | AgentRuntimeAction::WorktreeDiff
                    | AgentRuntimeAction::WorktreeKeep
                    | AgentRuntimeAction::WorktreeArchive
                    | AgentRuntimeAction::WorktreeDelete
                    | AgentRuntimeAction::WorktreeMerge
            ) {
                worktree_action = Some(execute_agent_worktree_action(
                    &workspace.path,
                    &mut database,
                    &team.id,
                    &instance,
                    request.action,
                )?);
            } else if matches!(request.action, AgentRuntimeAction::Delete) {
                if instance.execution_workspace_mode
                    == AgentExecutionWorkspaceMode::IsolatedWorktree
                    && instance.worktree_status.as_deref() != Some("deleted")
                {
                    return Err(ApiError::bad_request(
                        "delete Agent instance with isolated worktree requires explicit worktree_delete first",
                    ));
                }
                database
                    .delete_agent_instance(&instance_id)
                    .map_err(ApiError::from_workspace_error)?;
            } else if matches!(request.action, AgentRuntimeAction::ResetContext) {
                let reset_instance = database
                    .reset_agent_instance_context(&instance_id)
                    .map_err(ApiError::from_workspace_error)?;
                insert_agent_event(
                    &mut database,
                    &team.id,
                    "instance_context_reset",
                    Some(&instance_id),
                    None,
                    None,
                    json!({ "contextGeneration": reset_instance.context_generation }),
                )?;
            } else {
                let target = match request.action {
                    AgentRuntimeAction::Pause => AgentInstanceStatus::Paused,
                    AgentRuntimeAction::Resume => AgentInstanceStatus::Idle,
                    AgentRuntimeAction::Drain => AgentInstanceStatus::Draining,
                    AgentRuntimeAction::Stop => AgentInstanceStatus::Stopped,
                    AgentRuntimeAction::Delete => unreachable!(),
                    AgentRuntimeAction::ResetContext => unreachable!(),
                    AgentRuntimeAction::WorktreeStatus
                    | AgentRuntimeAction::WorktreeDiff
                    | AgentRuntimeAction::WorktreeKeep
                    | AgentRuntimeAction::WorktreeArchive
                    | AgentRuntimeAction::WorktreeDelete
                    | AgentRuntimeAction::WorktreeMerge => unreachable!(),
                };
                database
                    .transition_agent_instance_status(&instance_id, target)
                    .map_err(ApiError::from_workspace_error)?;
                insert_agent_event(
                    &mut database,
                    &team.id,
                    "instance_status_changed",
                    Some(&instance_id),
                    None,
                    None,
                    json!({ "status": target }),
                )?;
            }
        }
    }
    state.agent_scheduler.wake()?;
    let mut snapshot =
        agent_team_snapshot_from_database(&state, &workspace_id, &database, &team.id)?;
    snapshot.worktree_action = worktree_action;
    Ok(Json(snapshot))
}

fn execute_agent_worktree_action(
    workspace_path: &Path,
    database: &mut WorkspaceDatabase,
    team_id: &AgentTeamId,
    instance: &AgentInstanceRecord,
    action: AgentRuntimeAction,
) -> Result<serde_json::Value, ApiError> {
    let worktree_path = resolve_agent_instance_worktree_path(workspace_path, instance)?;
    let action_name = match action {
        AgentRuntimeAction::WorktreeStatus => "worktree_status",
        AgentRuntimeAction::WorktreeDiff => "worktree_diff",
        AgentRuntimeAction::WorktreeKeep => "worktree_keep",
        AgentRuntimeAction::WorktreeArchive => "worktree_archive",
        AgentRuntimeAction::WorktreeDelete => "worktree_delete",
        AgentRuntimeAction::WorktreeMerge => "worktree_merge",
        _ => return Err(ApiError::bad_request("invalid Agent worktree action")),
    };
    let result = match action {
        AgentRuntimeAction::WorktreeStatus => {
            let status = git_status_response(&worktree_path)?;
            json!({
                "action": action_name,
                "instanceId": instance.id.to_string(),
                "executionRootPath": display_path(&worktree_path),
                "status": status,
            })
        }
        AgentRuntimeAction::WorktreeDiff => {
            let diff = git_diff_response(&worktree_path, None)?;
            let diff_id = agent_worktree_diff_id(&diff);
            json!({
                "action": action_name,
                "instanceId": instance.id.to_string(),
                "executionRootPath": display_path(&worktree_path),
                "diffId": diff_id,
                "diff": diff,
            })
        }
        AgentRuntimeAction::WorktreeKeep => {
            let updated = database
                .update_agent_instance_worktree_status(&instance.id, "kept")
                .map_err(ApiError::from_workspace_error)?;
            json!({
                "action": action_name,
                "instanceId": updated.id.to_string(),
                "worktreeStatus": updated.worktree_status,
            })
        }
        AgentRuntimeAction::WorktreeArchive => {
            let updated = database
                .update_agent_instance_worktree_status(&instance.id, "archived")
                .map_err(ApiError::from_workspace_error)?;
            json!({
                "action": action_name,
                "instanceId": updated.id.to_string(),
                "worktreeStatus": updated.worktree_status,
            })
        }
        AgentRuntimeAction::WorktreeDelete => {
            delete_agent_worktree(workspace_path, &worktree_path, false)?;
            let updated = database
                .update_agent_instance_worktree_status(&instance.id, "deleted")
                .map_err(ApiError::from_workspace_error)?;
            json!({
                "action": action_name,
                "instanceId": updated.id.to_string(),
                "worktreeStatus": updated.worktree_status,
            })
        }
        AgentRuntimeAction::WorktreeMerge => {
            let base_revision = instance.worktree_base_revision.as_deref().ok_or_else(|| {
                ApiError::bad_request(format!(
                    "Agent instance '{}' has no worktree base revision",
                    instance.id
                ))
            })?;
            let merge = merge_agent_worktree(workspace_path, &worktree_path, base_revision)?;
            let updated = database
                .update_agent_instance_worktree_status(&instance.id, "kept")
                .map_err(ApiError::from_workspace_error)?;
            json!({
                "action": action_name,
                "instanceId": updated.id.to_string(),
                "worktreeStatus": updated.worktree_status,
                "baseRevision": merge.base_revision,
                "changedPaths": merge.changed_paths,
                "diffId": merge.diff_id,
            })
        }
        _ => unreachable!(),
    };
    insert_agent_event(
        database,
        team_id,
        action_name,
        Some(&instance.id),
        None,
        None,
        redact_worktree_action_event_payload(&result),
    )?;
    Ok(result)
}

fn resolve_agent_instance_worktree_path(
    workspace_path: &Path,
    instance: &AgentInstanceRecord,
) -> Result<PathBuf, ApiError> {
    if instance.execution_workspace_mode != AgentExecutionWorkspaceMode::IsolatedWorktree {
        return Err(ApiError::bad_request(format!(
            "Agent instance '{}' does not use an isolated worktree",
            instance.id
        )));
    }
    Ok(instance
        .execution_root_path
        .as_deref()
        .map(|root_path| resolve_agent_worktree_path(workspace_path, root_path))
        .unwrap_or_else(|| agent_instance_worktree_path(workspace_path, &instance.id)))
}

fn display_path(path: &Path) -> String {
    let path = path.display().to_string();
    path.strip_prefix(r"\\?\UNC\")
        .map(|tail| format!(r"\\{tail}"))
        .or_else(|| path.strip_prefix(r"\\?\").map(ToString::to_string))
        .unwrap_or(path)
}

fn redact_worktree_action_event_payload(result: &serde_json::Value) -> serde_json::Value {
    let mut payload = result.clone();
    if let Some(object) = payload.as_object_mut() {
        object.remove("diff");
    }
    payload
}

pub(crate) async fn agent_task_action(
    State(state): State<AppState>,
    AxumPath((workspace_id, task_id)): AxumPath<(String, String)>,
    Json(request): Json<AgentTaskActionRequest>,
) -> Result<Json<AgentTeamSnapshotResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let task_id =
        AgentTaskId::new(task_id).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let task = database
        .agent_task(&task_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("Agent task '{task_id}' was not found")))?;
    let instance = database
        .agent_instance(&task.owner_instance_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "Agent instance '{}' was not found",
                task.owner_instance_id
            ))
        })?;

    match request.action {
        AgentTaskAction::Transfer if task.status == AgentTaskStatus::Queued => {
            let target_instance_id = request.target_instance_id.as_ref().ok_or_else(|| {
                ApiError::bad_request("targetInstanceId is required for task transfer")
            })?;
            let target_instance = database
                .agent_instance(target_instance_id)
                .map_err(ApiError::from_workspace_error)?
                .ok_or_else(|| {
                    ApiError::bad_request(format!(
                        "Agent target instance '{target_instance_id}' was not found"
                    ))
                })?;
            if target_instance.team_id != task.team_id {
                return Err(ApiError::bad_request(format!(
                    "Agent target instance '{target_instance_id}' does not belong to team '{}'",
                    task.team_id
                )));
            }
            let transferred = database
                .transfer_queued_agent_task_with_limits(
                    &task.team_id,
                    &task.id,
                    &target_instance.id,
                    AGENT_MAX_QUEUED_TASKS_PER_TEAM,
                    AGENT_MAX_QUEUED_TASKS_PER_INSTANCE,
                    AGENT_MAX_QUEUED_TASKS_PER_CHAT,
                )
                .map_err(ApiError::from_workspace_error)?
                .ok_or_else(|| {
                    ApiError::bad_request(format!(
                        "Agent task '{}' changed state before transfer",
                        task.id
                    ))
                })?;
            insert_agent_event(
                &mut database,
                &task.team_id,
                "task_transferred",
                Some(&task.owner_instance_id),
                Some(&task.id),
                None,
                json!({
                    "previousOwnerInstanceId": task.owner_instance_id,
                    "targetInstanceId": transferred.owner_instance_id,
                    "sequence": transferred.sequence,
                }),
            )?;
        }
        AgentTaskAction::Transfer => {
            return Err(ApiError::bad_request(format!(
                "Agent task '{}' cannot be transferred while {}",
                task.id,
                task.status.as_str()
            )));
        }
        AgentTaskAction::Cancel if task.status == AgentTaskStatus::Running => {
            state
                .active_chat_runs
                .cancel(&workspace_id, task.id.as_str())?;
            insert_agent_event(
                &mut database,
                &task.team_id,
                "task_cancel_requested",
                Some(&task.owner_instance_id),
                Some(&task.id),
                None,
                json!({}),
            )?;
        }
        AgentTaskAction::Cancel
            if matches!(
                task.status,
                AgentTaskStatus::Queued | AgentTaskStatus::Waiting
            ) =>
        {
            let cascade = if task.status == AgentTaskStatus::Waiting {
                request.cascade.ok_or_else(|| {
                    ApiError::bad_request(
                        "cascade is required when cancelling a waiting Agent task",
                    )
                })?
            } else {
                request.cascade.unwrap_or(false)
            };
            let queued_children = if cascade {
                let children = database
                    .agent_tasks_for_parent(&task.team_id, &task.id)
                    .map_err(ApiError::from_workspace_error)?;
                if let Some(active_child) = children.iter().find(|child| {
                    matches!(
                        child.status,
                        AgentTaskStatus::Running | AgentTaskStatus::Waiting
                    )
                }) {
                    return Err(ApiError::bad_request(format!(
                        "Agent task '{}' cannot cascade-cancel active child task '{}' while {}",
                        task.id,
                        active_child.id,
                        active_child.status.as_str()
                    )));
                }
                children
                    .into_iter()
                    .filter(|child| child.status == AgentTaskStatus::Queued)
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            database
                .update_agent_task_state(AgentTaskStateUpdate {
                    team_id: &task.team_id,
                    task_id: &task.id,
                    expected_status: task.status,
                    transition: AgentTaskTransition::Cancel,
                    result_json: None,
                    error_json: Some(r#"{"message":"cancelled explicitly"}"#),
                    interruption_reason: None,
                })
                .map_err(ApiError::from_workspace_error)?;
            for child in queued_children {
                if database
                    .cancel_queued_agent_task(
                        &child.team_id,
                        &child.id,
                        r#"{"message":"cancelled by parent cascade"}"#,
                    )
                    .map_err(ApiError::from_workspace_error)?
                {
                    insert_agent_event(
                        &mut database,
                        &child.team_id,
                        "task_cancelled",
                        Some(&child.owner_instance_id),
                        Some(&child.id),
                        None,
                        json!({ "reason": "parent_cascade", "parentTaskId": task.id }),
                    )?;
                }
            }
            insert_agent_event(
                &mut database,
                &task.team_id,
                "task_cancelled",
                Some(&task.owner_instance_id),
                Some(&task.id),
                None,
                json!({ "reason": "explicit", "cascade": cascade }),
            )?;
        }
        AgentTaskAction::Cancel => {
            return Err(ApiError::bad_request(format!(
                "Agent task '{}' cannot be cancelled while {}",
                task.id,
                task.status.as_str()
            )));
        }
        AgentTaskAction::Retry
            if matches!(
                task.status,
                AgentTaskStatus::Failed | AgentTaskStatus::Cancelled | AgentTaskStatus::Interrupted
            ) =>
        {
            validate_agent_snapshot_for_workspace(
                &config,
                workspace,
                &instance.definition_snapshot,
            )?;
            database
                .update_agent_task_state(AgentTaskStateUpdate {
                    team_id: &task.team_id,
                    task_id: &task.id,
                    expected_status: task.status,
                    transition: AgentTaskTransition::Retry,
                    result_json: None,
                    error_json: None,
                    interruption_reason: None,
                })
                .map_err(ApiError::from_workspace_error)?;
            if matches!(
                instance.status,
                AgentInstanceStatus::Paused | AgentInstanceStatus::Failed
            ) {
                database
                    .transition_agent_instance_status(&instance.id, AgentInstanceStatus::Idle)
                    .map_err(ApiError::from_workspace_error)?;
            }
            insert_agent_event(
                &mut database,
                &task.team_id,
                "task_retried",
                Some(&task.owner_instance_id),
                Some(&task.id),
                None,
                json!({}),
            )?;
        }
        AgentTaskAction::Retry => {
            return Err(ApiError::bad_request(format!(
                "Agent task '{}' cannot be retried while {}",
                task.id,
                task.status.as_str()
            )));
        }
    }
    state.agent_scheduler.wake()?;
    Ok(Json(agent_team_snapshot_from_database(
        &state,
        &workspace_id,
        &database,
        &task.team_id,
    )?))
}

fn agent_team_snapshot_from_database(
    state: &AppState,
    workspace_id: &str,
    database: &WorkspaceDatabase,
    team_id: &AgentTeamId,
) -> Result<AgentTeamSnapshotResponse, ApiError> {
    let team = database
        .agent_team(team_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("Agent team '{team_id}' was not found")))?;
    let workload = database
        .agent_team_workload(team_id)
        .map_err(ApiError::from_workspace_error)?;
    let instances = database
        .agent_instances_for_team(team_id)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .collect::<Vec<_>>();
    let instance_views = instances
        .iter()
        .cloned()
        .map(AgentInstanceView::from)
        .collect();
    let mut tasks = Vec::new();
    let mut dependencies = Vec::new();
    let mut run_events = Vec::new();
    for task in database
        .agent_tasks_for_team(team_id)
        .map_err(ApiError::from_workspace_error)?
    {
        let attempts = database
            .agent_attempts_for_task(&task.id)
            .map_err(ApiError::from_workspace_error)?
            .into_iter()
            .map(|attempt| AgentAttemptView {
                id: attempt.id,
                sequence: attempt.sequence,
                status: attempt.status,
                started_at: attempt.started_at,
                completed_at: attempt.completed_at,
                interruption_reason: attempt.interruption_reason,
            })
            .collect();
        dependencies.extend(
            database
                .agent_task_dependencies(&task.id)
                .map_err(ApiError::from_workspace_error)?
                .into_iter()
                .map(AgentTaskDependencyView::from),
        );
        run_events.extend(
            database
                .run_events_for_run(task.id.as_str())
                .map_err(ApiError::from_workspace_error)?
                .into_iter()
                .map(AgentRunEventView::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        );
        tasks.push(AgentTaskView::from_record(task, attempts)?);
    }
    let mut messages = Vec::new();
    for instance in &instances {
        messages.extend(
            database
                .agent_messages_after(&instance.id, -1)
                .map_err(ApiError::from_workspace_error)?
                .into_iter()
                .map(AgentMessageView::from),
        );
    }
    messages.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then(
                left.receiver_instance_id
                    .as_str()
                    .cmp(right.receiver_instance_id.as_str()),
            )
            .then(left.sequence.cmp(&right.sequence))
    });
    let events = database
        .agent_events_after(team_id, -1)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(AgentEventView::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let mutation_lease_owners = state
        .tool_resource_locks
        .blocking_owners(workspace_id, &[workspace_mutation_lock()])
        .into_iter()
        .map(AgentMutationLeaseOwnerView::from)
        .collect::<Vec<_>>();
    let observability =
        AgentObservabilityView::from_parts(&workload, &tasks, &events, &mutation_lease_owners);
    Ok(AgentTeamSnapshotResponse {
        team: AgentTeamView::from(team),
        workload,
        observability,
        instances: instance_views,
        tasks,
        dependencies,
        messages,
        events,
        run_events,
        mutation_lease_owners,
        worktree_action: None,
    })
}

pub(crate) fn agent_instance_transcript_items(
    database: &WorkspaceDatabase,
    team: &AgentTeamRecord,
    instance: &AgentInstanceRecord,
) -> Result<Vec<AgentTranscriptItemView>, ApiError> {
    let instances = database
        .agent_instances_for_team(&team.id)
        .map_err(ApiError::from_workspace_error)?;
    let instances_by_id = instances
        .iter()
        .map(|current| (current.id.clone(), current))
        .collect::<HashMap<_, _>>();
    let tasks = database
        .agent_tasks_for_team(&team.id)
        .map_err(ApiError::from_workspace_error)?;
    let tasks_by_id = tasks
        .iter()
        .map(|task| (task.id.clone(), task))
        .collect::<HashMap<_, _>>();
    let mut messages = Vec::new();
    for current in &instances {
        messages.extend(
            database
                .agent_messages_after(&current.id, -1)
                .map_err(ApiError::from_workspace_error)?,
        );
    }
    let incoming_message_task_ids = messages
        .iter()
        .filter(|message| message.receiver_instance_id == instance.id)
        .filter_map(|message| message.related_task_id.clone())
        .collect::<HashSet<_>>();

    let mut items = Vec::new();
    for task in tasks
        .iter()
        .filter(|task| task.owner_instance_id == instance.id)
    {
        let input = parse_json_value(&task.input_json, "Agent task input")?;
        if !incoming_message_task_ids.contains(&task.id) {
            items.push(AgentTranscriptItemView {
                id: format!("task:{}:input", task.id),
                author: agent_instance_display_name(
                    team,
                    &instances_by_id,
                    task.origin_instance_id.as_ref(),
                ),
                role: AgentTranscriptRole::User,
                kind: "Task input".to_string(),
                created_at: task.created_at.clone(),
                task_status: Some(task.status),
                content: agent_task_input_content(&input),
                parts: Vec::new(),
                metrics: None,
                status: None,
            });
        }

        let events = database
            .run_events_for_run(task.id.as_str())
            .map_err(ApiError::from_workspace_error)?;
        if let Some(run_item) =
            agent_task_run_transcript_item(task, &events, &instance.definition_snapshot.name)?
        {
            items.push(run_item);
        } else if let Some(output) = agent_task_output_content(task)? {
            items.push(AgentTranscriptItemView {
                id: format!("task:{}:output", task.id),
                author: instance.definition_snapshot.name.clone(),
                role: AgentTranscriptRole::Assistant,
                kind: output.kind,
                created_at: task
                    .completed_at
                    .clone()
                    .unwrap_or_else(|| task.updated_at.clone()),
                task_status: Some(task.status),
                content: output.content,
                parts: Vec::new(),
                metrics: None,
                status: None,
            });
        }
    }

    for message in messages {
        if message.sender_instance_id.as_ref() != Some(&instance.id)
            && message.receiver_instance_id != instance.id
        {
            continue;
        }
        let is_outgoing = message.sender_instance_id.as_ref() == Some(&instance.id);
        let related_task_status = message
            .related_task_id
            .as_ref()
            .and_then(|task_id| tasks_by_id.get(task_id))
            .map(|task| task.status);
        items.push(AgentTranscriptItemView {
            id: format!("message:{}", message.id),
            author: if is_outgoing {
                instance.definition_snapshot.name.clone()
            } else {
                agent_instance_display_name(
                    team,
                    &instances_by_id,
                    message.sender_instance_id.as_ref(),
                )
            },
            role: if is_outgoing {
                AgentTranscriptRole::Assistant
            } else {
                AgentTranscriptRole::User
            },
            kind: if message.kind == foco_agent::AgentMessageKind::Reply {
                "Reply".to_string()
            } else {
                "Message".to_string()
            },
            created_at: message.created_at,
            task_status: related_task_status,
            content: redact_agent_text_or_json(&message.content),
            parts: Vec::new(),
            metrics: None,
            status: None,
        });
    }

    items.sort_by(compare_agent_transcript_items);
    Ok(items)
}

struct AgentTaskOutputContent {
    kind: String,
    content: String,
}

fn agent_task_run_transcript_item(
    task: &AgentTaskRecord,
    events: &[RunEventRecord],
    author: &str,
) -> Result<Option<AgentTranscriptItemView>, ApiError> {
    if events.is_empty() {
        if task.status != AgentTaskStatus::Running {
            return Ok(None);
        }
        return Ok(Some(AgentTranscriptItemView {
            id: format!("task:{}:run", task.id),
            author: author.to_string(),
            role: AgentTranscriptRole::Assistant,
            kind: "Task run".to_string(),
            created_at: task
                .started_at
                .clone()
                .unwrap_or_else(|| task.updated_at.clone()),
            task_status: Some(task.status),
            content: String::new(),
            parts: Vec::new(),
            metrics: None,
            status: Some(AgentTranscriptItemStatus::Streaming),
        }));
    }

    let mut sorted_events = events.to_vec();
    sorted_events.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then(left.created_at.cmp(&right.created_at))
            .then(left.id.cmp(&right.id))
    });
    let mut content = String::new();
    let mut parts = Vec::new();
    let mut metrics = None;
    let mut status =
        (task.status == AgentTaskStatus::Running).then_some(AgentTranscriptItemStatus::Streaming);

    for event in &sorted_events {
        let payload = parse_json_value(&event.payload_json, "Agent run event payload")?;
        let event_type =
            string_json_field(&payload, "type", "type").unwrap_or(event.event_type.as_str());
        match event_type {
            "textDelta" | "text_delta" => {
                if let Some(delta) = string_json_field(&payload, "delta", "delta") {
                    content.push_str(delta);
                    push_text_part(&mut parts, delta);
                    status.get_or_insert(AgentTranscriptItemStatus::Streaming);
                }
            }
            "reasoningDelta" | "reasoning_delta" => {
                if let Some(delta) = string_json_field(&payload, "delta", "delta") {
                    push_reasoning_part(&mut parts, delta);
                    status.get_or_insert(AgentTranscriptItemStatus::Streaming);
                }
            }
            "toolCall" | "tool_call" => {
                if let Some(tool_call) = payload
                    .get("toolCall")
                    .or_else(|| payload.get("tool_call"))
                    .and_then(agent_tool_call_summary_from_value)
                {
                    upsert_agent_tool_call_part(&mut parts, tool_call);
                    status.get_or_insert(AgentTranscriptItemStatus::Streaming);
                }
            }
            "toolResult" | "tool_result" => {
                let tool_call_id = string_json_field(&payload, "toolCallId", "tool_call_id");
                if let (Some(tool_call_id), Some(output)) = (tool_call_id, payload.get("output")) {
                    apply_agent_tool_result_to_parts(
                        &mut parts,
                        tool_call_id,
                        redact_agent_json(output.clone()),
                        bool_json_field(&payload, "isError", "is_error").unwrap_or(false),
                        string_json_field(&payload, "startedAt", "started_at"),
                        string_json_field(&payload, "completedAt", "completed_at"),
                    );
                }
            }
            "toolOutputDelta" | "tool_output_delta" => {
                let tool_call_id = string_json_field(&payload, "toolCallId", "tool_call_id");
                let stream = string_json_field(&payload, "stream", "stream");
                let delta = string_json_field(&payload, "delta", "delta").unwrap_or_default();
                if let (Some(tool_call_id), Some(stream)) = (tool_call_id, stream)
                    && matches!(stream, "stdout" | "stderr")
                {
                    apply_agent_tool_output_delta_to_parts(&mut parts, tool_call_id, stream, delta);
                }
            }
            "streamReset" | "stream_reset" => {
                content = string_json_field(&payload, "text", "text")
                    .unwrap_or_default()
                    .to_string();
                parts = parts_from_agent_run_snapshot(
                    &content,
                    nullable_string_json_field(&payload, "reasoning", "reasoning"),
                    payload
                        .get("toolCalls")
                        .or_else(|| payload.get("tool_calls"))
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(agent_tool_call_summary_from_value)
                        .collect(),
                );
                status = Some(AgentTranscriptItemStatus::Streaming);
            }
            "complete" | "completion" => {
                let final_text = string_json_field(&payload, "text", "text").unwrap_or(&content);
                append_missing_reasoning(
                    &mut parts,
                    nullable_string_json_field(&payload, "reasoning", "reasoning"),
                );
                append_missing_text(&mut parts, &content, final_text);
                content = final_text.to_string();
                metrics = payload.get("metrics").and_then(|value| {
                    serde_json::from_value::<ChatReplyMetrics>(value.clone()).ok()
                });
                status = None;
            }
            "error" => {
                push_error_part(
                    &mut parts,
                    string_json_field(&payload, "message", "message").unwrap_or("Unknown error"),
                );
                status = Some(AgentTranscriptItemStatus::Error);
            }
            _ => {}
        }
    }

    let terminal_output = if task.status == AgentTaskStatus::Running {
        None
    } else {
        agent_task_output_content(task)?
    };
    match terminal_output.as_ref().map(|output| output.kind.as_str()) {
        Some("Task error")
            if !parts
                .iter()
                .any(|part| matches!(part, ChatMessagePart::Error { .. })) =>
        {
            if let Some(output) = terminal_output {
                push_error_part(&mut parts, &output.content);
                status = Some(AgentTranscriptItemStatus::Error);
            }
        }
        Some("Task result") if content.is_empty() => {
            if let Some(output) = terminal_output {
                content = output.content;
                push_text_part(&mut parts, &content);
            }
        }
        _ => {}
    }

    if parts.is_empty() && content.is_empty() && task.status != AgentTaskStatus::Running {
        return Ok(None);
    }

    Ok(Some(AgentTranscriptItemView {
        id: format!("task:{}:run", task.id),
        author: author.to_string(),
        role: AgentTranscriptRole::Assistant,
        kind: match status {
            Some(AgentTranscriptItemStatus::Streaming) => "Task run",
            Some(AgentTranscriptItemStatus::Error) => "Task error",
            None => "Task result",
        }
        .to_string(),
        created_at: sorted_events
            .first()
            .map(|event| event.created_at.clone())
            .or_else(|| task.started_at.clone())
            .unwrap_or_else(|| task.updated_at.clone()),
        task_status: Some(task.status),
        content,
        parts,
        metrics,
        status,
    }))
}

fn agent_task_input_content(input: &Value) -> String {
    input
        .get("message")
        .and_then(Value::as_str)
        .filter(|message| !message.trim().is_empty())
        .map(str::trim)
        .map(ToString::to_string)
        .or_else(|| input.get("delegatedInput").map(format_json_value))
        .unwrap_or_else(|| format_json_value(input))
}

fn agent_task_output_content(
    task: &AgentTaskRecord,
) -> Result<Option<AgentTaskOutputContent>, ApiError> {
    if let Some(error) = &task.error_json {
        let value = redact_agent_json(parse_json_value(error, "Agent task error")?);
        return Ok(Some(AgentTaskOutputContent {
            kind: "Task error".to_string(),
            content: json_message_text(&value).unwrap_or_else(|| format_json_value(&value)),
        }));
    }

    let Some(result) = &task.result_json else {
        return Ok(None);
    };
    let value = redact_agent_json(parse_json_value(result, "Agent task result")?);
    Ok(Some(AgentTaskOutputContent {
        kind: "Task result".to_string(),
        content: json_message_text(&value).unwrap_or_else(|| format_json_value(&value)),
    }))
}

fn agent_instance_display_name(
    team: &AgentTeamRecord,
    instances_by_id: &HashMap<AgentInstanceId, &AgentInstanceRecord>,
    instance_id: Option<&AgentInstanceId>,
) -> String {
    let Some(instance_id) = instance_id else {
        return "Main agent".to_string();
    };
    instances_by_id
        .get(instance_id)
        .map(|instance| {
            if instance.id == team.coordinator_instance_id
                && instance.definition_snapshot.name.is_empty()
            {
                "Main agent".to_string()
            } else if instance.definition_snapshot.name.is_empty() {
                instance.id.to_string()
            } else {
                instance.definition_snapshot.name.clone()
            }
        })
        .unwrap_or_else(|| instance_id.to_string())
}

fn compare_agent_transcript_items(
    left: &AgentTranscriptItemView,
    right: &AgentTranscriptItemView,
) -> std::cmp::Ordering {
    left.created_at
        .cmp(&right.created_at)
        .then(left.id.cmp(&right.id))
}

fn agent_tool_call_summary_from_value(value: &Value) -> Option<ChatToolCallSummary> {
    let id = string_json_field(value, "id", "callId")
        .or_else(|| string_json_field(value, "call_id", "callId"))?;
    let name = string_json_field(value, "name", "name")?;
    let live_output = value.get("liveOutput").or_else(|| value.get("live_output"));
    Some(ChatToolCallSummary {
        id: id.to_string(),
        name: name.to_string(),
        status: string_json_field(value, "status", "status")
            .unwrap_or("running")
            .to_string(),
        input: value
            .get("input")
            .cloned()
            .map(redact_agent_json)
            .unwrap_or_else(|| json!({})),
        output: value.get("output").cloned().map(redact_agent_json),
        is_error: bool_json_field(value, "isError", "is_error").unwrap_or(false),
        started_at: string_json_field(value, "startedAt", "started_at").map(ToString::to_string),
        completed_at: string_json_field(value, "completedAt", "completed_at")
            .map(ToString::to_string),
        live_output: live_output.and_then(agent_tool_live_output_from_value),
    })
}

fn agent_tool_live_output_from_value(value: &Value) -> Option<ChatToolLiveOutput> {
    let stdout = string_json_field(value, "stdout", "stdout").unwrap_or_default();
    let stderr = string_json_field(value, "stderr", "stderr").unwrap_or_default();
    (!stdout.is_empty() || !stderr.is_empty()).then(|| ChatToolLiveOutput {
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    })
}

fn upsert_agent_tool_call_part(
    parts: &mut Vec<ChatMessagePart>,
    next_tool_call: ChatToolCallSummary,
) {
    if let Some(ChatMessagePart::ToolCall { tool_call }) = parts.iter_mut().find(|part| {
        matches!(part, ChatMessagePart::ToolCall { tool_call } if tool_call.id == next_tool_call.id)
    }) {
        let live_output = next_tool_call
            .live_output
            .clone()
            .or_else(|| tool_call.live_output.clone());
        *tool_call = ChatToolCallSummary {
            live_output,
            ..next_tool_call
        };
    } else {
        parts.push(ChatMessagePart::ToolCall {
            tool_call: next_tool_call,
        });
    }
}

fn apply_agent_tool_result_to_parts(
    parts: &mut [ChatMessagePart],
    tool_call_id: &str,
    output: Value,
    is_error: bool,
    started_at: Option<&str>,
    completed_at: Option<&str>,
) {
    for part in parts {
        if let ChatMessagePart::ToolCall { tool_call } = part
            && tool_call.id == tool_call_id
        {
            tool_call.output = Some(output.clone());
            tool_call.is_error = is_error;
            tool_call.status = if is_error { "error" } else { "completed" }.to_string();
            if let Some(started_at) = started_at {
                tool_call.started_at = Some(started_at.to_string());
            }
            if let Some(completed_at) = completed_at {
                tool_call.completed_at = Some(completed_at.to_string());
            }
            tool_call.live_output = None;
        }
    }
}

fn apply_agent_tool_output_delta_to_parts(
    parts: &mut [ChatMessagePart],
    tool_call_id: &str,
    stream: &str,
    delta: &str,
) {
    for part in parts {
        if let ChatMessagePart::ToolCall { tool_call } = part
            && tool_call.id == tool_call_id
            && tool_call.output.is_none()
        {
            let mut live_output = tool_call.live_output.clone().unwrap_or_default();
            if stream == "stdout" {
                live_output.stdout.push_str(delta);
            } else {
                live_output.stderr.push_str(delta);
            }
            tool_call.live_output = Some(live_output);
        }
    }
}

fn parts_from_agent_run_snapshot(
    text: &str,
    reasoning: Option<&str>,
    tool_calls: Vec<ChatToolCallSummary>,
) -> Vec<ChatMessagePart> {
    let mut parts = Vec::new();
    if let Some(reasoning) = reasoning {
        push_reasoning_part(&mut parts, reasoning);
    }
    push_text_part(&mut parts, text);
    parts.extend(
        tool_calls
            .into_iter()
            .map(|tool_call| ChatMessagePart::ToolCall { tool_call }),
    );
    parts
}

fn append_missing_text(parts: &mut Vec<ChatMessagePart>, current: &str, final_text: &str) {
    if let Some(suffix) = final_text.strip_prefix(current)
        && !suffix.is_empty()
    {
        push_text_part(parts, suffix);
    }
}

fn append_missing_reasoning(parts: &mut Vec<ChatMessagePart>, final_reasoning: Option<&str>) {
    let Some(final_reasoning) = final_reasoning else {
        return;
    };
    let existing = parts
        .iter()
        .filter_map(|part| match part {
            ChatMessagePart::Reasoning { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>();
    if let Some(suffix) = final_reasoning.strip_prefix(&existing)
        && !suffix.is_empty()
    {
        push_reasoning_part(parts, suffix);
    }
}

fn json_message_text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| string_json_field(value, "text", "text").map(ToString::to_string))
        .or_else(|| string_json_field(value, "message", "message").map(ToString::to_string))
        .or_else(|| string_json_field(value, "content", "content").map(ToString::to_string))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn push_error_part(parts: &mut Vec<ChatMessagePart>, text: &str) {
    if text.is_empty() {
        return;
    }
    match parts.last_mut() {
        Some(ChatMessagePart::Error { text: existing }) => existing.push_str(text),
        _ => parts.push(ChatMessagePart::Error {
            text: text.to_string(),
        }),
    }
}

fn format_json_value(value: &Value) -> String {
    serde_json::to_string_pretty(value)
        .map(|value| format!("```json\n{value}\n```"))
        .unwrap_or_else(|_| "```json\nnull\n```".to_string())
}

fn bool_json_field(value: &Value, primary: &str, alternate: &str) -> Option<bool> {
    value
        .get(primary)
        .or_else(|| value.get(alternate))
        .and_then(Value::as_bool)
}

fn workspace_mutation_lock() -> ToolResourceLock {
    ToolResourceLock {
        resource: ToolResource::WorkspaceMutationLease,
        access: ToolResourceAccess::Exclusive,
    }
}

impl From<AgentTeamRecord> for AgentTeamView {
    fn from(team: AgentTeamRecord) -> Self {
        Self {
            id: team.id,
            chat_id: team.chat_id,
            coordinator_instance_id: team.coordinator_instance_id,
            status: team.status,
            max_concurrent_runs: team.max_concurrent_runs,
            created_at: team.created_at,
            updated_at: team.updated_at,
        }
    }
}

impl From<AgentInstanceRecord> for AgentInstanceView {
    fn from(instance: AgentInstanceRecord) -> Self {
        Self {
            id: instance.id,
            team_id: instance.team_id,
            definition_id: instance.definition_id,
            definition_revision: instance.definition_revision,
            definition_snapshot: AgentDefinitionRuntimeView::from(instance.definition_snapshot),
            role: instance.role,
            status: instance.status,
            next_task_sequence: instance.next_task_sequence,
            context_generation: instance.context_generation,
            last_scheduled_at: instance.last_scheduled_at,
            execution_workspace_mode: instance.execution_workspace_mode,
            execution_root_path: instance
                .execution_root_path
                .map(|path| display_path(Path::new(&path))),
            worktree_base_revision: instance.worktree_base_revision,
            worktree_branch: instance.worktree_branch,
            worktree_status: instance.worktree_status,
            created_at: instance.created_at,
            updated_at: instance.updated_at,
        }
    }
}

impl From<AgentDefinitionSettings> for AgentDefinitionRuntimeView {
    fn from(definition: AgentDefinitionSettings) -> Self {
        Self {
            id: definition.id,
            revision: definition.revision,
            name: definition.name,
            description: definition.description,
            provider_id: definition.provider_id,
            model_id: definition.model_id,
            model_options: definition.model_options,
            allowed_tools: definition.allowed_tools,
            max_instances: definition.max_instances,
            allowed_execution_workspace_modes: definition.allowed_execution_workspace_modes,
            permissions: definition.permissions,
        }
    }
}

impl From<AgentTaskDependencyRecord> for AgentTaskDependencyView {
    fn from(dependency: AgentTaskDependencyRecord) -> Self {
        Self {
            team_id: dependency.team_id,
            waiting_task_id: dependency.waiting_task_id,
            dependency_task_id: dependency.dependency_task_id,
            wait_mode: dependency.wait_mode,
            pending_tool_call_id: dependency.pending_tool_call_id,
            deadline_at: dependency.deadline_at,
            created_at: dependency.created_at,
        }
    }
}

impl From<AgentMessageRecord> for AgentMessageView {
    fn from(message: AgentMessageRecord) -> Self {
        Self {
            id: message.id,
            team_id: message.team_id,
            sender_instance_id: message.sender_instance_id,
            receiver_instance_id: message.receiver_instance_id,
            related_task_id: message.related_task_id,
            reply_to_message_id: message.reply_to_message_id,
            kind: message.kind,
            content: redact_agent_text_or_json(&message.content),
            sequence: message.sequence,
            created_at: message.created_at,
            consumed_at: message.consumed_at,
        }
    }
}

impl TryFrom<AgentEventRecord> for AgentEventView {
    type Error = ApiError;

    fn try_from(event: AgentEventRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            team_id: event.team_id,
            sequence: event.sequence,
            event_type: event.event_type,
            instance_id: event.instance_id,
            task_id: event.task_id,
            attempt_id: event.attempt_id,
            message_id: event.message_id,
            payload: redact_agent_json(serde_json::from_str(&event.payload_json).map_err(
                |source| {
                    ApiError::internal(format!("invalid persisted Agent event payload: {source}"))
                },
            )?),
            created_at: event.created_at,
        })
    }
}

impl TryFrom<RunEventRecord> for AgentRunEventView {
    type Error = ApiError;

    fn try_from(event: RunEventRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            run_id: event.run_id,
            sequence: event.sequence,
            event_type: event.event_type,
            payload: redact_agent_json(serde_json::from_str(&event.payload_json).map_err(
                |source| {
                    ApiError::internal(format!(
                        "invalid persisted Agent run event payload: {source}"
                    ))
                },
            )?),
            created_at: event.created_at,
        })
    }
}

impl AgentObservabilityView {
    fn from_parts(
        workload: &TeamWorkload,
        tasks: &[AgentTaskView],
        events: &[AgentEventView],
        mutation_lease_owners: &[AgentMutationLeaseOwnerView],
    ) -> Self {
        let failed_tasks = tasks
            .iter()
            .filter(|task| task.status == AgentTaskStatus::Failed)
            .count();
        let cancelled_tasks = tasks
            .iter()
            .filter(|task| task.status == AgentTaskStatus::Cancelled)
            .count();
        let interrupted_tasks = tasks
            .iter()
            .filter(|task| task.status == AgentTaskStatus::Interrupted)
            .count();
        let failures_by_type = [
            ("failed", failed_tasks),
            ("cancelled", cancelled_tasks),
            ("interrupted", interrupted_tasks),
        ]
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .map(|(kind, count)| AgentFailureClassView { kind, count })
        .collect();

        Self {
            queue_length: i64::from(workload.queued_tasks),
            queue_wait_ms: AgentMetricSummaryView::from_values(
                events
                    .iter()
                    .filter_map(|event| agent_event_i64(event, "queueWaitMs")),
            ),
            run_duration_ms: AgentMetricSummaryView::from_values(
                events
                    .iter()
                    .filter_map(|event| agent_event_i64(event, "runTimeMs")),
            ),
            scheduler_latency_ms: AgentMetricSummaryView::from_values(
                events
                    .iter()
                    .filter_map(|event| agent_event_i64(event, "schedulerLatencyMs")),
            ),
            mutation_lease_wait_ms: AgentMetricSummaryView::from_values(
                mutation_lease_owners
                    .iter()
                    .map(|owner| i64::try_from(owner.wait_ms).unwrap_or(i64::MAX)),
            ),
            failed_tasks,
            cancelled_tasks,
            interrupted_tasks,
            failures_by_type,
        }
    }
}

impl AgentMetricSummaryView {
    fn from_values(values: impl Iterator<Item = i64>) -> Self {
        let mut count = 0usize;
        let mut sum = 0i64;
        let mut max = None;
        for value in values {
            count += 1;
            sum = sum.saturating_add(value);
            max = Some(max.map_or(value, |current: i64| current.max(value)));
        }
        Self {
            count,
            max,
            average: if count == 0 {
                None
            } else {
                Some(sum / i64::try_from(count).unwrap_or(i64::MAX))
            },
        }
    }
}

fn agent_event_i64(event: &AgentEventView, key: &str) -> Option<i64> {
    event.payload.get(key)?.as_i64()
}

impl From<ToolResourceLockOwnerSnapshot> for AgentMutationLeaseOwnerView {
    fn from(snapshot: ToolResourceLockOwnerSnapshot) -> Self {
        Self {
            instance_id: snapshot.owner.instance_id,
            task_id: snapshot.owner.task_id,
            tool_call_id: snapshot.owner.tool_call_id,
            tool_name: snapshot.owner.tool_name,
            active_ms: u64::try_from(snapshot.active_ms).unwrap_or(u64::MAX),
            wait_ms: u64::try_from(snapshot.wait_ms).unwrap_or(u64::MAX),
        }
    }
}

impl AgentTaskView {
    fn from_record(
        task: AgentTaskRecord,
        attempts: Vec<AgentAttemptView>,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            id: task.id,
            team_id: task.team_id,
            owner_instance_id: task.owner_instance_id,
            origin_instance_id: task.origin_instance_id,
            parent_task_id: task.parent_task_id,
            sequence: task.sequence,
            status: task.status,
            input: redact_agent_json(serde_json::from_str(&task.input_json).map_err(|source| {
                ApiError::internal(format!("invalid persisted Agent task input: {source}"))
            })?),
            result: task
                .result_json
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(|source| {
                    ApiError::internal(format!("invalid persisted Agent task result: {source}"))
                })?
                .map(redact_agent_json),
            error: task
                .error_json
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(|source| {
                    ApiError::internal(format!("invalid persisted Agent task error: {source}"))
                })?
                .map(redact_agent_json),
            attempts,
            created_at: task.created_at,
            updated_at: task.updated_at,
            started_at: task.started_at,
            completed_at: task.completed_at,
        })
    }
}

fn redact_agent_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    if is_agent_sensitive_key(&key) {
                        (
                            key,
                            serde_json::Value::String(AGENT_REDACTED_VALUE.to_string()),
                        )
                    } else {
                        (key, redact_agent_json(value))
                    }
                })
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(redact_agent_json).collect())
        }
        serde_json::Value::String(value) => serde_json::Value::String(redact_agent_text(&value)),
        value => value,
    }
}

fn redact_agent_text_or_json(value: &str) -> String {
    serde_json::from_str::<serde_json::Value>(value)
        .map(redact_agent_json)
        .map(|value| value.to_string())
        .unwrap_or_else(|_| redact_agent_text(value))
}

fn redact_agent_text(value: &str) -> String {
    let value = AGENT_URL_RE.replace_all(value, |captures: &regex::Captures<'_>| {
        redact_agent_url(
            captures
                .get(0)
                .map(|capture| capture.as_str())
                .unwrap_or_default(),
        )
    });
    AGENT_SECRET_ASSIGNMENT_RE
        .replace_all(&value, "${key}=<redacted>")
        .into_owned()
}

fn redact_agent_url(value: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(value) else {
        return value.to_string();
    };
    let has_sensitive_parts = !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some();
    if !has_sensitive_parts {
        return value.to_string();
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

fn is_agent_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "authorization"
            | "apikey"
            | "password"
            | "passwordhash"
            | "cookie"
            | "setcookie"
            | "proxyauthorization"
            | "accesstoken"
            | "refreshtoken"
            | "idtoken"
            | "secret"
            | "clientsecret"
            | "credential"
            | "credentials"
            | "privatekey"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_snapshot_redaction_removes_secrets_and_url_sensitive_parts() {
        let value = redact_agent_json(json!({
            "apiKey": "key-123",
            "inputTokens": 42,
            "nested": {
                "cookie": "session=secret",
                "baseUrl": "https://user:pass@example.test/v1?api_key=secret#frag"
            },
            "message": "authorization: Bearer secret https://example.test/path?token=secret#fragment"
        }));

        assert_eq!(value["apiKey"], AGENT_REDACTED_VALUE);
        assert_eq!(value["inputTokens"], 42);
        assert_eq!(value["nested"]["cookie"], AGENT_REDACTED_VALUE);
        assert_eq!(value["nested"]["baseUrl"], "https://example.test/v1");
        let message = value["message"].as_str().expect("redacted message");
        assert!(message.contains("authorization=<redacted>"));
        assert!(message.contains("https://example.test/path"));
        assert!(!message.contains("secret"));
        assert!(!message.contains("fragment"));
    }
}
