use axum::{
    Json,
    extract::{Path as AxumPath, State},
};
use foco_agent::{
    AgentAttemptId, AgentInstanceId, AgentInstanceStatus, AgentTaskId, AgentTaskStatus,
    AgentTaskTransition, AgentTeamId, AgentTeamStatus, TeamActivationRequest,
};
use foco_store::workspace::{
    AgentInstanceRecord, AgentTaskRecord, AgentTaskStateUpdate, AgentTeamRecord, NewAgentTeam,
    WorkspaceDatabase,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentRuntimeActionRequest {
    scope: AgentRuntimeScope,
    action: AgentRuntimeAction,
    instance_id: Option<AgentInstanceId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AgentTaskAction {
    Cancel,
    Retry,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentTaskActionRequest {
    action: AgentTaskAction,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentTeamSnapshotResponse {
    team: AgentTeamView,
    instances: Vec<AgentInstanceView>,
    tasks: Vec<AgentTaskView>,
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
    definition_snapshot: AgentDefinitionSettings,
    role: foco_agent::AgentRole,
    status: AgentInstanceStatus,
    next_task_sequence: i64,
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
            } else {
                let target = match request.action {
                    AgentRuntimeAction::Pause => AgentInstanceStatus::Paused,
                    AgentRuntimeAction::Resume => AgentInstanceStatus::Idle,
                    AgentRuntimeAction::Drain => AgentInstanceStatus::Draining,
                    AgentRuntimeAction::Stop => AgentInstanceStatus::Stopped,
                    AgentRuntimeAction::Delete => unreachable!(),
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
            insert_agent_event(
                &mut database,
                &task.team_id,
                "task_cancelled",
                Some(&task.owner_instance_id),
                Some(&task.id),
                None,
                json!({ "reason": "explicit" }),
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
        &database,
        &task.team_id,
    )?))
}

fn agent_team_snapshot_from_database(
    database: &WorkspaceDatabase,
    team_id: &AgentTeamId,
) -> Result<AgentTeamSnapshotResponse, ApiError> {
    let team = database
        .agent_team(team_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("Agent team '{team_id}' was not found")))?;
    let instances = database
        .agent_instances_for_team(team_id)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(AgentInstanceView::from)
        .collect();
    let mut tasks = Vec::new();
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
        tasks.push(AgentTaskView::from_record(task, attempts)?);
    }
    Ok(AgentTeamSnapshotResponse {
        team: AgentTeamView::from(team),
        instances,
        tasks,
    })
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
            definition_snapshot: instance.definition_snapshot,
            role: instance.role,
            status: instance.status,
            next_task_sequence: instance.next_task_sequence,
            last_scheduled_at: instance.last_scheduled_at,
            created_at: instance.created_at,
            updated_at: instance.updated_at,
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
