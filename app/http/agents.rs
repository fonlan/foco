use axum::{
    Json,
    extract::{Path as AxumPath, State},
};
use foco_agent::{
    AgentAttemptId, AgentDefinitionId, AgentInstanceId, AgentInstanceStatus, AgentMessageId,
    AgentRole, AgentTaskId, AgentTaskStatus, AgentTaskTransition, AgentTeamId, AgentTeamStatus,
    TeamActivationRequest, TeamWorkload, ToolResource, ToolResourceAccess, ToolResourceLock,
};
use foco_store::workspace::{
    AgentEventRecord, AgentInstanceRecord, AgentMessageRecord, AgentTaskDependencyRecord,
    AgentTaskRecord, AgentTaskStateUpdate, AgentTeamRecord, NewAgentInstance, NewAgentTeam,
    WorkspaceDatabase,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::runtime::{
    AGENT_MAX_CREATE_INSTANCES_PER_REQUEST, AGENT_MAX_INSTANCES_PER_TEAM,
    ToolResourceLockOwnerSnapshot,
};
use crate::*;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AgentRuntimeScope {
    Team,
    Instance,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AgentRuntimeAction {
    Pause,
    Resume,
    Drain,
    Stop,
    Delete,
    ResetContext,
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
    instances: Vec<AgentInstanceView>,
    tasks: Vec<AgentTaskView>,
    dependencies: Vec<AgentTaskDependencyView>,
    messages: Vec<AgentMessageView>,
    events: Vec<AgentEventView>,
    mutation_lease_owners: Vec<AgentMutationLeaseOwnerView>,
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
    created_at: String,
    updated_at: String,
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
            max_concurrent_runs: 1,
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
        &database, &team_id,
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
        &database, &team.id,
    )?))
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
    let instances = instance_ids
        .iter()
        .map(|id| NewAgentInstance {
            id,
            team_id: &team.id,
            definition,
            role: AgentRole::Worker,
        })
        .collect::<Vec<_>>();
    let created = database
        .create_agent_instances_with_limits(
            &instances,
            i64::from(request.max_instances_per_team),
            i64::from(request.max_instances_for_definition),
        )
        .map_err(ApiError::from_workspace_error)?;
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
            }),
        )?;
    }
    state.agent_scheduler.wake()?;
    Ok(Json(agent_team_snapshot_from_database(
        &state,
        &workspace_id,
        &database, &team.id,
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
            if matches!(request.action, AgentRuntimeAction::Delete) {
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
    Ok(Json(agent_team_snapshot_from_database(
        &state,
        &workspace_id,
        &database, &team.id,
    )?))
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
            .then(left.receiver_instance_id.as_str().cmp(right.receiver_instance_id.as_str()))
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
        .collect();
    Ok(AgentTeamSnapshotResponse {
        team: AgentTeamView::from(team),
        workload,
        instances: instance_views,
        tasks,
        dependencies,
        messages,
        events,
        mutation_lease_owners,
    })
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
            content: message.content,
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
            payload: serde_json::from_str(&event.payload_json).map_err(|source| {
                ApiError::internal(format!("invalid persisted Agent event payload: {source}"))
            })?,
            created_at: event.created_at,
        })
    }
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
            input: serde_json::from_str(&task.input_json).map_err(|source| {
                ApiError::internal(format!("invalid persisted Agent task input: {source}"))
            })?,
            result: task
                .result_json
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(|source| {
                    ApiError::internal(format!("invalid persisted Agent task result: {source}"))
                })?,
            error: task
                .error_json
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(|source| {
                    ApiError::internal(format!("invalid persisted Agent task error: {source}"))
                })?,
            attempts,
            created_at: task.created_at,
            updated_at: task.updated_at,
            started_at: task.started_at,
            completed_at: task.completed_at,
        })
    }
}
