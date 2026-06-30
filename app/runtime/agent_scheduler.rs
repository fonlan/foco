use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, Utc};
use foco_agent::{
    AgentAttemptId, AgentCollaborationTool, AgentExecutionWorkspaceMode, AgentInstanceStatus,
    AgentPermissions, AgentRole, AgentRunAssociations, AgentRunOutcome, AgentTaskId,
    AgentTaskStatus, AgentTaskTransition, build_subagents_prompt_section, estimate_text_tokens,
};
use foco_providers::{NeutralChatMessage, NeutralChatRole, NeutralToolCall};
use foco_store::{
    config::{AGENT_DEFINITION_SYSTEM_PROMPT_MAX_CHARS, AgentDefinitionSettings},
    workspace::{
        AgentContextEntryRecord, AgentInstanceRecord, AgentMessageRecord,
        AgentTaskDependencyRecord, AgentTaskRecord, AgentTaskStateUpdate, AgentTeamRecord,
        NewAgentContextEntry, NewAgentContextSnapshot, NewAgentEvent, WorkspaceDatabase,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    sync::{Semaphore, mpsc},
    task::{JoinHandle, JoinSet},
    time,
};

use crate::git_backend::{
    agent_instance_worktree_path, agent_worktree_diff_id, git_diff_response,
    resolve_agent_worktree_path,
};
use crate::*;

// ponytail: fixed first-slice limits avoid new config surface; make them configurable when
// production workload data shows a different ceiling is needed.
pub(crate) const AGENT_MAX_QUEUED_TASKS_PER_TEAM: i64 = 64;
pub(crate) const AGENT_MAX_QUEUED_TASKS_PER_INSTANCE: i64 = 64;
pub(crate) const AGENT_MAX_QUEUED_TASKS_PER_CHAT: i64 = 64;
pub(crate) const AGENT_MAX_INSTANCES_PER_TEAM: i64 = 10;
pub(crate) const AGENT_MAX_CREATE_INSTANCES_PER_REQUEST: u32 = 16;
const AGENT_SCHEDULER_WAKE_CAPACITY: usize = 1;
const AGENT_SCHEDULER_SCAN_LIMIT: i64 = 64;
const AGENT_SCHEDULER_MIN_DEADLINE_DELAY_MS: u64 = 1_000;
const AGENT_SCHEDULER_ERROR_RETRY_SECS: i64 = 30;
const AGENT_GLOBAL_MAX_CONCURRENT_RUNS: usize = 10;
const RESTART_INTERRUPTION_REASON: &str = "backend restarted while Agent attempt was active";
const AGENT_TEAM_PROTOCOL_VERSION: u32 = 2;
const AGENT_CONTEXT_SNAPSHOT_VERSION: u32 = 1;
const AGENT_CONTEXT_RECENT_MESSAGE_LIMIT: usize = 8;
const AGENT_CONTEXT_SUMMARY_ENTRY_LIMIT: usize = 16;
const AGENT_CONTEXT_SUMMARY_MAX_CHARS: usize = 320;
const AGENT_MAX_TASK_OUTCOME_BYTES: usize = 64 * 1024;
const AGENT_WAIT_RESUME_INSTRUCTION: &str = "<agent_wait_resume>\n<source>Foco Agent wait resume</source>\n<instructions>the following agent_wait_tasks tool result contains completed child task results. Continue the current parent task from this result, synthesize the child output as needed, and do not treat a child task's final text as the main chat reply by itself.</instructions>\n</agent_wait_resume>";

#[derive(Clone)]
pub(crate) struct AgentScheduler {
    wake_tx: mpsc::Sender<()>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CoordinatorTaskInput {
    pub(crate) queued_user_message_id: String,
    #[serde(default)]
    pub(crate) visible_assistant_message_id: Option<String>,
    #[serde(default)]
    pub(crate) visible_assistant_sequence: Option<i64>,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) attachments: Vec<ChatAttachmentInput>,
    #[serde(default)]
    pub(crate) skill_ids: Vec<String>,
    #[serde(default)]
    pub(crate) session_mode: Option<String>,
    #[serde(default = "default_collaboration_tools_enabled")]
    pub(crate) collaboration_tools_enabled: bool,
    #[serde(default)]
    pub(crate) defer_until_workspace_idle: bool,
    #[serde(default)]
    pub(crate) delegated_input: Option<Value>,
    #[serde(default)]
    pub(crate) correlation_id: Option<String>,
}

struct AgentTaskModelSelection {
    model_id: String,
    provider_id: Option<String>,
    thinking_level: Option<String>,
}

fn default_collaboration_tools_enabled() -> bool {
    true
}

impl AgentScheduler {
    pub(crate) fn new() -> (Self, mpsc::Receiver<()>) {
        let (wake_tx, wake_rx) = mpsc::channel(AGENT_SCHEDULER_WAKE_CAPACITY);
        (Self { wake_tx }, wake_rx)
    }

    pub(crate) fn wake(&self) -> Result<(), ApiError> {
        match self.wake_tx.try_send(()) {
            Ok(()) | Err(mpsc::error::TrySendError::Full(())) => Ok(()),
            Err(mpsc::error::TrySendError::Closed(())) => {
                Err(ApiError::internal("Agent scheduler is not running"))
            }
        }
    }

    pub(crate) fn spawn(&self, state: AppState, wake_rx: mpsc::Receiver<()>) -> JoinHandle<()> {
        tokio::spawn(run_agent_scheduler(state, wake_rx))
    }
}

async fn run_agent_scheduler(state: AppState, mut wake_rx: mpsc::Receiver<()>) {
    if let Err(error) = reconcile_agent_runtime(&state) {
        tracing::error!(error = %error.message, "Agent scheduler startup reconciliation failed");
    }

    let permits = Arc::new(Semaphore::new(AGENT_GLOBAL_MAX_CONCURRENT_RUNS));
    let mut runs = JoinSet::new();
    let mut shutdown_rx = state.app_shutdown_rx.clone();
    let mut scan = true;
    let mut next_deadline_at: Option<DateTime<Utc>> = None;

    loop {
        if scan {
            scan = false;
            match schedule_runnable_tasks(&state, &permits, &mut runs).await {
                Ok(result) => next_deadline_at = result.next_deadline_at,
                Err(error) => {
                    next_deadline_at = Some(
                        Utc::now() + chrono::Duration::seconds(AGENT_SCHEDULER_ERROR_RETRY_SECS),
                    );
                    tracing::error!(error = %error.message, "Agent scheduler scan failed");
                }
            }
        }
        let deadline_sleep = time::sleep(agent_scheduler_deadline_delay(next_deadline_at.as_ref()));
        tokio::pin!(deadline_sleep);

        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            wake = wake_rx.recv() => {
                if wake.is_none() {
                    break;
                }
                scan = true;
            }
            completed = runs.join_next(), if !runs.is_empty() => {
                if let Some(Err(error)) = completed {
                    tracing::error!(error = %error, "Agent scheduler run task panicked");
                }
                scan = true;
            }
            _ = &mut deadline_sleep, if next_deadline_at.is_some() => {
                scan = true;
            }
        }
    }

    while let Some(result) = runs.join_next().await {
        if let Err(error) = result {
            tracing::error!(error = %error, "Agent scheduler run failed during shutdown");
        }
    }
}

pub(crate) fn reconcile_agent_runtime(state: &AppState) -> Result<(), ApiError> {
    let config = config_snapshot(state)?;
    for workspace in &config.workspaces {
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        for record in database
            .startup_agent_reconciliation()
            .map_err(ApiError::from_workspace_error)?
        {
            let expected_status = record.task.status;
            if expected_status != AgentTaskStatus::Running {
                continue;
            }
            if database
                .suspend_running_agent_task_with_wait_dependencies(
                    &record.task.team_id,
                    &record.task.id,
                )
                .map_err(ApiError::from_workspace_error)?
            {
                insert_agent_event(
                    &mut database,
                    &record.task.team_id,
                    "task_suspended",
                    Some(&record.task.owner_instance_id),
                    Some(&record.task.id),
                    Some(&record.attempt.id),
                    json!({ "reason": "startup_wait_dependency_recovery" }),
                )?;
                crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task(
                    &workspace.path,
                    &record.task.id,
                )?;
                continue;
            }
            database
                .update_agent_task_state(AgentTaskStateUpdate {
                    team_id: &record.task.team_id,
                    task_id: &record.task.id,
                    expected_status,
                    transition: AgentTaskTransition::Interrupt,
                    result_json: None,
                    error_json: Some(
                        r#"{"message":"backend restarted while Agent attempt was active"}"#,
                    ),
                    interruption_reason: Some(RESTART_INTERRUPTION_REASON),
                })
                .map_err(ApiError::from_workspace_error)?;
            database
                .transition_agent_instance_status(
                    &record.task.owner_instance_id,
                    AgentInstanceStatus::Paused,
                )
                .map_err(ApiError::from_workspace_error)?;
            insert_agent_event(
                &mut database,
                &record.task.team_id,
                "attempt_interrupted",
                Some(&record.task.owner_instance_id),
                Some(&record.task.id),
                Some(&record.attempt.id),
                json!({ "reason": RESTART_INTERRUPTION_REASON }),
            )?;
            database
                .fail_plan_phase_run(&record.task.id, RESTART_INTERRUPTION_REASON)
                .map_err(ApiError::from_workspace_error)?;
            crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task(
                &workspace.path,
                &record.task.id,
            )?;
        }
        database
            .fail_running_plan_phases_for_interrupted_agent_tasks(RESTART_INTERRUPTION_REASON)
            .map_err(ApiError::from_workspace_error)?;
        for instance in database
            .isolated_agent_instances()
            .map_err(ApiError::from_workspace_error)?
        {
            if instance.worktree_status.as_deref() == Some("deleted") {
                continue;
            }
            let root_path = agent_instance_execution_root(&workspace.path, &instance);
            if root_path.exists() {
                continue;
            }
            let updated = database
                .update_agent_instance_worktree_status(&instance.id, "deleted")
                .map_err(ApiError::from_workspace_error)?;
            insert_agent_event(
                &mut database,
                &instance.team_id,
                "worktree_reconciled",
                Some(&instance.id),
                None,
                None,
                json!({
                    "reason": "isolated worktree path was not found during startup reconciliation",
                    "executionRootPath": root_path.display().to_string(),
                    "worktreeStatus": updated.worktree_status,
                }),
            )?;
        }
    }
    Ok(())
}

#[derive(Default)]
struct AgentSchedulerScan {
    next_deadline_at: Option<DateTime<Utc>>,
}

async fn schedule_runnable_tasks(
    state: &AppState,
    permits: &Arc<Semaphore>,
    runs: &mut JoinSet<()>,
) -> Result<AgentSchedulerScan, ApiError> {
    let config = config_snapshot(state)?;
    let mut scan = AgentSchedulerScan::default();
    'scan: for workspace in &config.workspaces {
        loop {
            let Ok(permit) = permits.clone().try_acquire_owned() else {
                break 'scan;
            };
            let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            for recovered_task in database
                .recover_interrupted_agent_wait_tasks(
                    RESTART_INTERRUPTION_REASON,
                    AGENT_SCHEDULER_SCAN_LIMIT,
                )
                .map_err(ApiError::from_workspace_error)?
            {
                insert_agent_event(
                    &mut database,
                    &recovered_task.team_id,
                    "task_suspended",
                    Some(&recovered_task.owner_instance_id),
                    Some(&recovered_task.id),
                    None,
                    json!({ "reason": "interrupted_wait_dependency_recovery" }),
                )?;
            }
            for resumed_task in database
                .resume_satisfied_agent_tasks(AGENT_SCHEDULER_SCAN_LIMIT)
                .map_err(ApiError::from_workspace_error)?
            {
                insert_agent_event(
                    &mut database,
                    &resumed_task.team_id,
                    "task_resumed",
                    Some(&resumed_task.owner_instance_id),
                    Some(&resumed_task.id),
                    None,
                    json!({}),
                )?;
            }
            let Some(task) = database
                .runnable_agent_tasks(AGENT_SCHEDULER_SCAN_LIMIT)
                .map_err(ApiError::from_workspace_error)?
                .into_iter()
                .next()
            else {
                record_next_agent_deadline(&mut scan, &database)?;
                drop(permit);
                break;
            };
            let attempt_id = AgentAttemptId::new(unique_id("agent-attempt"))
                .map_err(|error| ApiError::internal(error.to_string()))?;
            let Some(claimed) = database
                .claim_runnable_agent_task(&task.team_id, &task.id, &attempt_id)
                .map_err(ApiError::from_workspace_error)?
            else {
                drop(permit);
                continue;
            };
            if let Err(error) =
                crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task(
                    &workspace.path,
                    &claimed.id,
                )
            {
                let _ = fail_claimed_task(&workspace.path, &claimed.id, &error.message);
                drop(permit);
                continue;
            }
            if let Err(error) = insert_agent_event(
                &mut database,
                &claimed.team_id,
                "attempt_started",
                Some(&claimed.owner_instance_id),
                Some(&claimed.id),
                Some(&attempt_id),
                json!({}),
            ) {
                let _ = fail_claimed_task(&workspace.path, &claimed.id, &error.message);
                drop(permit);
                continue;
            }
            if let Err(error) = insert_agent_event(
                &mut database,
                &claimed.team_id,
                "task_started",
                Some(&claimed.owner_instance_id),
                Some(&claimed.id),
                Some(&attempt_id),
                json!({
                    "queueWaitMs": timestamp_delta_ms(
                        Some(&claimed.created_at),
                        claimed.started_at.as_deref()
                    ),
                    "schedulerLatencyMs": timestamp_delta_ms(
                        Some(&task.updated_at),
                        claimed.started_at.as_deref()
                    ),
                }),
            ) {
                let _ = fail_claimed_task(&workspace.path, &claimed.id, &error.message);
                drop(permit);
                continue;
            }
            let state = state.clone();
            let workspace = workspace.clone();
            runs.spawn(async move {
                let _permit = permit;
                run_coordinator_task(state, workspace, claimed.id, attempt_id).await;
            });
        }
    }
    Ok(scan)
}

fn record_next_agent_deadline(
    scan: &mut AgentSchedulerScan,
    database: &WorkspaceDatabase,
) -> Result<(), ApiError> {
    let Some(value) = database
        .next_waiting_agent_task_dependency_deadline()
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(());
    };
    let deadline = DateTime::parse_from_rfc3339(&value)
        .map_err(|source| {
            ApiError::internal(format!(
                "Agent task dependency deadline is invalid: {source}"
            ))
        })?
        .with_timezone(&Utc);
    match scan.next_deadline_at.as_ref() {
        Some(current) if current <= &deadline => {}
        _ => scan.next_deadline_at = Some(deadline),
    }
    Ok(())
}

fn agent_scheduler_deadline_delay(next_deadline_at: Option<&DateTime<Utc>>) -> Duration {
    let Some(next_deadline_at) = next_deadline_at else {
        return Duration::from_secs(86_400);
    };
    let now = Utc::now();
    if next_deadline_at <= &now {
        return Duration::from_millis(AGENT_SCHEDULER_MIN_DEADLINE_DELAY_MS);
    }
    let millis_until_deadline = next_deadline_at
        .signed_duration_since(now)
        .num_milliseconds();
    if millis_until_deadline <= 0 {
        return Duration::from_millis(AGENT_SCHEDULER_MIN_DEADLINE_DELAY_MS);
    }
    Duration::from_millis(millis_until_deadline as u64)
}

async fn run_coordinator_task(
    state: AppState,
    workspace: WorkspaceConfig,
    task_id: AgentTaskId,
    attempt_id: AgentAttemptId,
) {
    if let Err(error) = run_coordinator_task_inner(&state, &workspace, &task_id, &attempt_id).await
    {
        tracing::error!(
            workspace_id = %workspace.id,
            task_id = %task_id,
            attempt_id = %attempt_id,
            error = %error.message,
            "Coordinator task failed"
        );
        let _ = fail_claimed_task(&workspace.path, &task_id, &error.message);
    }
    let _ = state.agent_scheduler.wake();
}

async fn run_coordinator_task_inner(
    state: &AppState,
    workspace: &WorkspaceConfig,
    task_id: &AgentTaskId,
    attempt_id: &AgentAttemptId,
) -> Result<(), ApiError> {
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let task = database
        .agent_task(task_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::internal(format!("Agent task '{task_id}' was not found")))?;
    let team = database
        .agent_team(&task.team_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::internal(format!("Agent team '{}' was not found", task.team_id))
        })?;
    let instance = database
        .agent_instance(&task.owner_instance_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::internal(format!(
                "Agent instance '{}' was not found",
                task.owner_instance_id
            ))
        })?;
    let task_input =
        serde_json::from_str::<CoordinatorTaskInput>(&task.input_json).map_err(|source| {
            ApiError::internal(format!("invalid Coordinator task input: {source}"))
        })?;
    let session_upload_paths = task_input
        .attachments
        .iter()
        .filter_map(|attachment| attachment.path.clone())
        .collect::<Vec<_>>();
    let model_selection = agent_task_model_selection(&database, &team, &instance, &task_input)?;
    drop(database);

    let config = config_snapshot(state)?;
    validate_agent_snapshot_for_workspace(&config, workspace, &instance.definition_snapshot)?;
    let mut chat_context = prepare_chat_context(
        state,
        &config,
        &workspace.id,
        ChatStreamRequest {
            chat_id: Some(team.chat_id.clone()),
            queued_user_message_id: Some(task_input.queued_user_message_id.clone()),
            run_id_override: Some(task.id.to_string()),
            visible_assistant_message_id: task_input.visible_assistant_message_id.clone(),
            visible_assistant_sequence: task_input.visible_assistant_sequence,
            model_id: model_selection.model_id,
            provider_id: model_selection.provider_id,
            thinking_level: model_selection.thinking_level,
            skill_ids: Some(task_input.skill_ids.clone()),
            session_mode: task_input.session_mode.clone(),
            message: task_input.message.clone(),
            attachments: task_input.attachments.clone(),
        },
    )
    .await?;
    chat_context.tool_workspace_path = match instance.execution_workspace_mode {
        AgentExecutionWorkspaceMode::Shared => workspace.path.clone(),
        AgentExecutionWorkspaceMode::IsolatedWorktree => {
            agent_instance_execution_root(&workspace.path, &instance)
        }
    };
    let allowed_tools = instance
        .definition_snapshot
        .allowed_tools
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    chat_context
        .provider_request
        .tools
        .retain(|tool| allowed_tools.contains(&tool.name));
    let collaboration_permissions = if task_input.collaboration_tools_enabled {
        instance.definition_snapshot.permissions.clone()
    } else {
        AgentPermissions::default()
    };
    if task_input.collaboration_tools_enabled {
        append_agent_collaboration_tools(&mut chat_context, &collaboration_permissions);
    }
    if let Some(max_output_tokens) = instance.definition_snapshot.model_options.max_output_tokens {
        chat_context.provider_request.max_output_tokens = Some(max_output_tokens);
    }
    let (agent_unread_messages, consumed_agent_message_ids) = apply_agent_prompt_layers(
        &workspace.path,
        &mut chat_context,
        &team,
        &instance,
        &task,
        attempt_id,
        &allowed_tools,
        &collaboration_permissions,
        &config.agent_definitions,
    )?;
    chat_context.agent_primary_chat_output = instance.role == AgentRole::Coordinator;
    chat_context.agent_unread_messages = agent_unread_messages;
    if chat_context.pending_memory_retrieval.is_none() {
        chat_context.provider_request.prompt_cache_key = Some(prompt_cache_key(
            &chat_context.workspace_id,
            &chat_context.chat_id,
            &chat_context.provider_id,
            &chat_context.model_id,
            &chat_context.provider_request,
            &chat_context.message_source_sequences,
            &chat_context.message_context_sources,
        )?);
        chat_context.provider_request.prompt_cache_retention =
            Some(PROMPT_CACHE_RETENTION_24H.to_string());
    }
    chat_context.request_body_json = serialize_provider_request(&chat_context.provider_request)?;
    consume_agent_messages(&workspace.path, &consumed_agent_message_ids)?;
    chat_context.agent_associations = AgentRunAssociations {
        team_id: Some(task.team_id.clone()),
        instance_id: Some(task.owner_instance_id.clone()),
        task_id: Some(task.id.clone()),
        attempt_id: Some(attempt_id.clone()),
    };
    chat_context.agent_definition_snapshot = Some(
        serde_json::to_value(&instance.definition_snapshot).map_err(|source| {
            ApiError::internal(format!(
                "failed to serialize Agent definition snapshot: {source}"
            ))
        })?,
    );
    chat_context.agent_task_input = Some(serde_json::from_str::<Value>(&task.input_json).map_err(
        |source| ApiError::internal(format!("failed to parse Agent task input: {source}")),
    )?);
    chat_context.agent_allowed_tools = Some(allowed_tools);
    chat_context.agent_tool_context = Some(AgentToolContext {
        workspace_path: workspace.path.clone(),
        associations: chat_context.agent_associations.clone(),
        collaboration_tools_enabled: task_input.collaboration_tools_enabled,
        permissions: collaboration_permissions,
        agent_definitions: config.agent_definitions.clone(),
        scheduler: state.agent_scheduler.clone(),
    });
    chat_context.session_upload_paths = Some(session_upload_paths);

    let (guidance_tx, guidance_rx) = mpsc::unbounded_channel();
    let next_run_event_sequence = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?
        .next_run_event_sequence(task.id.as_str())
        .map_err(ApiError::from_workspace_error)?;
    let registration = state.active_chat_runs.register(
        task.id.to_string(),
        workspace.id.clone(),
        team.chat_id.clone(),
        chat_context.assistant_message_id.clone(),
        chat_context.assistant_sequence,
        chat_context.memories_used.clone(),
        chat_context.agent_primary_chat_output,
        next_run_event_sequence,
        guidance_tx,
    )?;
    let outcome = run_chat_context_in_background(chat_context, registration, guidance_rx).await;
    persist_agent_task_context(&workspace.path, &task, &instance, attempt_id, &outcome)?;
    finish_claimed_task(&workspace.path, &task, attempt_id, outcome)?;
    crate::plan_runtime::sync_plan_phase_for_agent_task(state, workspace, &task.id).await
}

fn agent_task_model_selection(
    database: &WorkspaceDatabase,
    team: &AgentTeamRecord,
    instance: &AgentInstanceRecord,
    task_input: &CoordinatorTaskInput,
) -> Result<AgentTaskModelSelection, ApiError> {
    let queued_run = match database
        .message(&task_input.queued_user_message_id)
        .map_err(ApiError::from_workspace_error)?
    {
        Some(message) if message.chat_id == team.chat_id => {
            queued_run_summary_from_message_metadata(&message.metadata_json)?
        }
        Some(message) => {
            return Err(ApiError::internal(format!(
                "Queued user message '{}' belongs to chat '{}' instead of Agent team chat '{}'",
                task_input.queued_user_message_id, message.chat_id, team.chat_id
            )));
        }
        None => None,
    };

    Ok(match queued_run {
        Some(queued_run) => AgentTaskModelSelection {
            model_id: queued_run.model_id,
            provider_id: queued_run.provider_id,
            thinking_level: queued_run.thinking_level,
        },
        None => AgentTaskModelSelection {
            model_id: instance.definition_snapshot.model_id.clone(),
            provider_id: Some(instance.definition_snapshot.provider_id.clone()),
            thinking_level: instance
                .definition_snapshot
                .model_options
                .thinking_level
                .clone(),
        },
    })
}

fn apply_agent_prompt_layers(
    workspace_path: &Path,
    chat_context: &mut PreparedChatContext,
    team: &AgentTeamRecord,
    instance: &AgentInstanceRecord,
    task: &AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    allowed_tools: &HashSet<String>,
    collaboration_permissions: &AgentPermissions,
    agent_definitions: &[AgentDefinitionSettings],
) -> Result<(Vec<Value>, Vec<foco_agent::AgentMessageId>), ApiError> {
    validate_agent_definition_system_prompt(instance)?;

    let database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let context_snapshot = database
        .latest_agent_context_snapshot(&instance.id, instance.context_generation)
        .map_err(ApiError::from_workspace_error)?;
    let after_context_sequence = context_snapshot
        .as_ref()
        .map(|snapshot| snapshot.sequence)
        .unwrap_or(-1);
    let context_entries = database
        .agent_context_entries(
            &instance.id,
            instance.context_generation,
            after_context_sequence,
        )
        .map_err(ApiError::from_workspace_error)?;
    let unread_messages = database
        .agent_messages_after(&instance.id, -1)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .filter(|message| message.consumed_at.is_none())
        .collect::<Vec<_>>();
    let team_instances = database
        .agent_instances_for_team(&team.id)
        .map_err(ApiError::from_workspace_error)?;
    let wait_dependencies = database
        .agent_task_dependencies(&task.id)
        .map_err(ApiError::from_workspace_error)?;
    let wait_dependency_tasks = wait_dependencies
        .iter()
        .map(|dependency| {
            database
                .agent_task_for_team(&dependency.team_id, &dependency.dependency_task_id)
                .map_err(ApiError::from_workspace_error)?
                .ok_or_else(|| {
                    ApiError::internal(format!(
                        "Agent dependency task '{}' was not found",
                        dependency.dependency_task_id
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    drop(database);

    let definition_index = agent_definition_insert_index(chat_context);
    insert_agent_prompt_message(
        chat_context,
        definition_index,
        neutral_agent_message(
            NeutralChatRole::System,
            instance
                .definition_snapshot
                .system_prompt
                .trim()
                .to_string(),
        ),
        None,
        PromptContextSource::AgentDefinition,
    );

    let protocol_index = agent_team_protocol_insert_index(chat_context);
    insert_agent_prompt_message(
        chat_context,
        protocol_index,
        neutral_agent_message(
            NeutralChatRole::System,
            agent_team_protocol_prompt(
                team,
                instance,
                task,
                attempt_id,
                allowed_tools,
                collaboration_permissions,
                agent_definitions,
                &team_instances,
            )?,
        ),
        None,
        PromptContextSource::AgentTeamProtocol,
    );

    if let Some(private_context) =
        agent_private_context_prompt(context_snapshot.as_ref(), &context_entries)?
    {
        let index = chat_context.active_tool_start_index;
        insert_agent_prompt_message(
            chat_context,
            index,
            neutral_agent_message(NeutralChatRole::System, private_context),
            None,
            PromptContextSource::AgentPrivateContext,
        );
    }

    let current_task =
        agent_current_task_prompt(task, attempt_id, &wait_dependencies, &wait_dependency_tasks)?;
    let index = chat_context.active_tool_start_index;
    insert_agent_prompt_message(
        chat_context,
        index,
        neutral_agent_message(NeutralChatRole::User, current_task),
        Some(task.sequence),
        PromptContextSource::AgentCurrentTask {
            sequence: task.sequence,
        },
    );

    for message in agent_wait_resume_messages(&wait_dependencies, &wait_dependency_tasks)? {
        let index = chat_context.active_tool_start_index;
        insert_agent_prompt_message(
            chat_context,
            index,
            message,
            Some(task.sequence),
            PromptContextSource::AgentCurrentTask {
                sequence: task.sequence,
            },
        );
    }

    let mut run_unread_messages = Vec::with_capacity(unread_messages.len());
    let mut consumed_message_ids = Vec::with_capacity(unread_messages.len());
    for message in unread_messages {
        let payload = agent_message_payload(&message);
        let payload_json = serde_json::to_string_pretty(&payload).map_err(|source| {
            ApiError::internal(format!(
                "failed to serialize Agent message prompt: {source}"
            ))
        })?;
        let prompt = xml_json_section("agent_unread_message", &payload_json);
        let index = chat_context.active_tool_start_index;
        insert_agent_prompt_message(
            chat_context,
            index,
            neutral_agent_message(NeutralChatRole::User, prompt),
            None,
            PromptContextSource::AgentUnreadMessage,
        );
        consumed_message_ids.push(message.id.clone());
        run_unread_messages.push(payload);
    }

    Ok((run_unread_messages, consumed_message_ids))
}

fn validate_agent_definition_system_prompt(instance: &AgentInstanceRecord) -> Result<(), ApiError> {
    let system_prompt = instance.definition_snapshot.system_prompt.trim();
    if system_prompt.is_empty() {
        return Err(ApiError::bad_request(format!(
            "Agent definition snapshot '{}' has an empty system prompt",
            instance.definition_id
        )));
    }
    if system_prompt.chars().count() > AGENT_DEFINITION_SYSTEM_PROMPT_MAX_CHARS {
        return Err(ApiError::bad_request(format!(
            "Agent definition snapshot '{}' system prompt exceeds {AGENT_DEFINITION_SYSTEM_PROMPT_MAX_CHARS} characters",
            instance.definition_id
        )));
    }
    Ok(())
}

fn agent_definition_insert_index(chat_context: &PreparedChatContext) -> usize {
    chat_context
        .message_context_sources
        .iter()
        .position(|source| !matches!(source, PromptContextSource::ReservedPrompt))
        .unwrap_or(chat_context.active_tool_start_index)
}

fn agent_team_protocol_insert_index(chat_context: &PreparedChatContext) -> usize {
    agent_team_protocol_insert_index_for_sources(
        &chat_context.message_context_sources,
        chat_context.active_tool_start_index,
    )
}

fn agent_team_protocol_insert_index_for_sources(
    message_context_sources: &[PromptContextSource],
    fallback_index: usize,
) -> usize {
    message_context_sources
        .iter()
        .position(|source| {
            !matches!(
                source,
                PromptContextSource::ReservedPrompt | PromptContextSource::AgentDefinition
            )
        })
        .unwrap_or(fallback_index)
}

fn insert_agent_prompt_message(
    chat_context: &mut PreparedChatContext,
    index: usize,
    message: NeutralChatMessage,
    source_sequence: Option<i64>,
    source: PromptContextSource,
) {
    chat_context
        .provider_request
        .messages
        .insert(index, message);
    chat_context
        .message_source_sequences
        .insert(index, source_sequence);
    chat_context.message_context_sources.insert(index, source);
    if index <= chat_context.active_tool_start_index {
        chat_context.active_tool_start_index += 1;
    }
    if let Some(pending) = &mut chat_context.pending_memory_retrieval {
        if index <= pending.stable_insert_index {
            pending.stable_insert_index += 1;
        }
        if index <= pending.turn_insert_index {
            pending.turn_insert_index += 1;
        }
    }
}

fn neutral_agent_message(role: NeutralChatRole, content: String) -> NeutralChatMessage {
    NeutralChatMessage {
        role,
        content,
        attachments: Vec::new(),
        reasoning: None,
        tool_calls: Vec::new(),
        tool_call_id: None,
        tool_name: None,
    }
}

fn append_agent_collaboration_tools(
    chat_context: &mut PreparedChatContext,
    permissions: &AgentPermissions,
) {
    for definition in foco_tools::agent_tool_definitions() {
        let include = match definition.name {
            foco_tools::AGENT_LIST_TOOL
            | foco_tools::AGENT_GET_TASK_TOOL
            | foco_tools::AGENT_SEND_MESSAGE_TOOL => true,
            foco_tools::AGENT_DELEGATE_TASK_TOOL
            | foco_tools::AGENT_CANCEL_TASK_TOOL
            | foco_tools::AGENT_WAIT_TASKS_TOOL
            | foco_tools::AGENT_TRANSFER_TASK_TOOL => {
                permissions.collaboration_tool_allowed(AgentCollaborationTool::DelegateTask)
            }
            foco_tools::AGENT_CREATE_INSTANCES_TOOL => {
                permissions.collaboration_tool_allowed(AgentCollaborationTool::CreateInstance)
            }
            _ => false,
        };
        if include
            && !chat_context
                .provider_request
                .tools
                .iter()
                .any(|tool| tool.name == definition.name)
        {
            chat_context
                .provider_request
                .tools
                .push(neutral_tool_definition(definition));
        }
    }
}

fn agent_team_protocol_prompt(
    team: &AgentTeamRecord,
    instance: &AgentInstanceRecord,
    task: &AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    allowed_tools: &HashSet<String>,
    collaboration_permissions: &AgentPermissions,
    agent_definitions: &[AgentDefinitionSettings],
    team_instances: &[AgentInstanceRecord],
) -> Result<String, ApiError> {
    let mut tools = allowed_tools.iter().cloned().collect::<Vec<_>>();
    tools.sort();
    let creatable_agent_definitions = creatable_agent_definitions_prompt(
        team,
        collaboration_permissions,
        agent_definitions,
        team_instances,
    )?;
    let protocol = json!({
        "version": AGENT_TEAM_PROTOCOL_VERSION,
        "teamId": team.id.to_string(),
        "chatId": team.chat_id,
        "instanceId": instance.id.to_string(),
        "definitionId": instance.definition_id.to_string(),
        "definitionRevision": instance.definition_revision,
        "role": instance.role.as_str(),
        "taskId": task.id.to_string(),
        "attemptId": attempt_id.to_string(),
        "contextGeneration": instance.context_generation,
        "executionWorkspace": {
            "mode": instance.execution_workspace_mode.as_str(),
            "rootPath": instance.execution_root_path,
            "baseRevision": instance.worktree_base_revision,
            "branch": instance.worktree_branch,
            "status": instance.worktree_status,
        },
        "permissions": collaboration_permissions,
        "creatableAgentDefinitions": creatable_agent_definitions,
        "allowedRuntimeTools": tools,
        "runtimeLimits": {
            "maxQueuedTasksPerTeam": AGENT_MAX_QUEUED_TASKS_PER_TEAM,
            "maxQueuedTasksPerInstance": AGENT_MAX_QUEUED_TASKS_PER_INSTANCE,
            "maxQueuedTasksPerChat": AGENT_MAX_QUEUED_TASKS_PER_CHAT,
            "maxInstancesPerTeam": AGENT_MAX_INSTANCES_PER_TEAM,
            "maxCreateInstancesPerRequest": AGENT_MAX_CREATE_INSTANCES_PER_REQUEST,
            "maxAgentToolRounds": MAX_AGENT_TOOL_ROUNDS,
        },
        "outputPolicy": {
            "coordinatorWritesMainChat": true,
            "workerWritesMainChat": false,
            "workerAutomaticMemoryExtraction": false,
        },
    });
    let protocol_json = serde_json::to_string_pretty(&protocol).map_err(|source| {
        ApiError::internal(format!("failed to serialize Agent team protocol: {source}"))
    })?;
    Ok(format!(
        "{}\n{}",
        build_subagents_prompt_section(),
        xml_json_section("agent_team_protocol", &protocol_json)
    ))
}

fn creatable_agent_definitions_prompt(
    team: &AgentTeamRecord,
    permissions: &AgentPermissions,
    agent_definitions: &[AgentDefinitionSettings],
    team_instances: &[AgentInstanceRecord],
) -> Result<Vec<Value>, ApiError> {
    if !permissions.can_create_instances {
        return Ok(Vec::new());
    }

    let max_instances_per_team = u32::try_from(AGENT_MAX_INSTANCES_PER_TEAM)
        .map_err(|_| ApiError::internal("Agent max instances per team exceeds u32"))?;
    let current_team_instances = u32::try_from(
        team_instances
            .iter()
            .filter(|instance| instance.team_id == team.id)
            .count(),
    )
    .map_err(|_| ApiError::internal("Agent team instance count exceeds u32"))?;
    let remaining_team_slots = max_instances_per_team.saturating_sub(current_team_instances);
    let mut definitions = Vec::with_capacity(permissions.allowed_agent_definition_ids.len());

    for allowed_id in &permissions.allowed_agent_definition_ids {
        let Some(definition) =
            creatable_agent_definition(allowed_id, agent_definitions, team_instances)
        else {
            // ponytail: stale create permissions should shrink advertised options, not fail
            // the current task; config validation still rejects bad newly-saved definitions.
            continue;
        };
        let current_definition_instances = u32::try_from(
            team_instances
                .iter()
                .filter(|instance| {
                    instance.team_id == team.id && instance.definition_id == *allowed_id
                })
                .count(),
        )
        .map_err(|_| ApiError::internal("Agent definition instance count exceeds u32"))?;
        let remaining_definition_slots = definition
            .max_instances
            .saturating_sub(current_definition_instances);
        let max_create_count = AGENT_MAX_CREATE_INSTANCES_PER_REQUEST
            .min(remaining_team_slots)
            .min(remaining_definition_slots);
        let count_schema = if max_create_count == 0 {
            Value::Null
        } else {
            json!({
                "minimum": 1,
                "maximum": max_create_count,
            })
        };
        let allowed_execution_workspace_modes = definition
            .allowed_execution_workspace_modes
            .iter()
            .map(|mode| mode.as_str())
            .collect::<Vec<_>>();

        definitions.push(json!({
            "definitionId": definition.id.to_string(),
            "revision": definition.revision,
            "name": definition.name,
            "description": definition.description,
            "maxInstances": definition.max_instances,
            "currentTeamInstances": current_team_instances,
            "remainingTeamSlots": remaining_team_slots,
            "currentTeamDefinitionInstances": current_definition_instances,
            "remainingTeamDefinitionSlots": remaining_definition_slots,
            "maxCreateCount": max_create_count,
            "canCreateMore": max_create_count > 0,
            "allowedExecutionWorkspaceModes": allowed_execution_workspace_modes.clone(),
            "agentCreateInstancesSchema": {
                "tool": "agent_create_instances",
                "definitionId": { "const": definition.id.to_string() },
                "count": count_schema,
                "executionWorkspaceMode": { "enum": allowed_execution_workspace_modes },
                "timeoutMs": { "const": null },
            },
        }));
    }

    Ok(definitions)
}

fn creatable_agent_definition<'a>(
    allowed_id: &foco_agent::AgentDefinitionId,
    agent_definitions: &'a [AgentDefinitionSettings],
    team_instances: &'a [AgentInstanceRecord],
) -> Option<&'a AgentDefinitionSettings> {
    agent_definitions
        .iter()
        .find(|definition| definition.id == *allowed_id)
        .or_else(|| {
            team_instances
                .iter()
                .find(|instance| instance.definition_id == *allowed_id)
                .map(|instance| &instance.definition_snapshot)
        })
}

fn agent_private_context_prompt(
    snapshot: Option<&foco_store::workspace::AgentContextSnapshotRecord>,
    entries: &[AgentContextEntryRecord],
) -> Result<Option<String>, ApiError> {
    if snapshot.is_none() && entries.is_empty() {
        return Ok(None);
    }
    let recent_entries = entries
        .iter()
        .rev()
        .take(AGENT_CONTEXT_RECENT_MESSAGE_LIMIT)
        .map(agent_context_entry_prompt_value)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let snapshot_value = snapshot
        .map(|record| {
            serde_json::from_str::<Value>(&record.entries_json).map_err(|source| {
                ApiError::internal(format!("failed to parse Agent context snapshot: {source}"))
            })
        })
        .transpose()?;
    let context = json!({
        "snapshot": snapshot_value,
        "recentEntries": recent_entries,
    });
    let context_json = serde_json::to_string_pretty(&context).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize Agent private context: {source}"
        ))
    })?;
    Ok(Some(xml_json_section(
        "agent_private_context",
        &context_json,
    )))
}

fn agent_context_entry_prompt_value(entry: &AgentContextEntryRecord) -> Result<Value, ApiError> {
    let content = serde_json::from_str::<Value>(&entry.content_json).map_err(|source| {
        ApiError::internal(format!("failed to parse Agent context entry: {source}"))
    })?;
    Ok(json!({
        "id": entry.id,
        "sequence": entry.sequence,
        "role": entry.role,
        "sourceTaskId": entry.source_task_id.as_ref().map(ToString::to_string),
        "sourceMessageId": entry.source_message_id.as_ref().map(ToString::to_string),
        "createdAt": entry.created_at,
        "content": content,
    }))
}

fn agent_current_task_prompt(
    task: &AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    wait_dependencies: &[AgentTaskDependencyRecord],
    wait_dependency_tasks: &[AgentTaskRecord],
) -> Result<String, ApiError> {
    let input = serde_json::from_str::<Value>(&task.input_json).map_err(|source| {
        ApiError::internal(format!("failed to parse Agent task input: {source}"))
    })?;
    let mut current_task = json!({
        "taskId": task.id.to_string(),
        "teamId": task.team_id.to_string(),
        "ownerInstanceId": task.owner_instance_id.to_string(),
        "originInstanceId": task.origin_instance_id.as_ref().map(ToString::to_string),
        "parentTaskId": task.parent_task_id.as_ref().map(ToString::to_string),
        "attemptId": attempt_id.to_string(),
        "sequence": task.sequence,
        "status": task.status.as_str(),
        "input": input,
    });
    if task.result_json.is_some() || task.error_json.is_some() {
        current_task["previousAttempt"] = agent_previous_attempt_payload(task)?;
    }
    if !wait_dependencies.is_empty() {
        current_task["resume"] =
            agent_wait_resume_payload(wait_dependencies, wait_dependency_tasks)?;
    }
    let current_task_json = serde_json::to_string_pretty(&current_task).map_err(|source| {
        ApiError::internal(format!("failed to serialize Agent current task: {source}"))
    })?;
    Ok(xml_json_section("agent_current_task", &current_task_json))
}

fn agent_previous_attempt_payload(task: &AgentTaskRecord) -> Result<Value, ApiError> {
    let result = task
        .result_json
        .as_deref()
        .map(|value| {
            serde_json::from_str::<Value>(value).map_err(|source| {
                ApiError::internal(format!(
                    "failed to parse Agent task previous result: {source}"
                ))
            })
        })
        .transpose()?;
    let error = task
        .error_json
        .as_deref()
        .map(|value| {
            serde_json::from_str::<Value>(value).map_err(|source| {
                ApiError::internal(format!(
                    "failed to parse Agent task previous error: {source}"
                ))
            })
        })
        .transpose()?;
    Ok(json!({
        "result": result,
        "error": error,
        "completedAt": task.completed_at,
    }))
}

fn agent_wait_resume_payload(
    dependencies: &[AgentTaskDependencyRecord],
    dependency_tasks: &[AgentTaskRecord],
) -> Result<Value, ApiError> {
    let pending_tool_call_id = dependencies
        .iter()
        .find_map(|dependency| dependency.pending_tool_call_id.clone());
    Ok(json!({
        "kind": "agent_wait_tasks",
        "pendingToolCallId": pending_tool_call_id,
        "toolResult": agent_wait_resume_tool_result(dependencies, dependency_tasks)?,
    }))
}

fn agent_wait_resume_messages(
    dependencies: &[AgentTaskDependencyRecord],
    dependency_tasks: &[AgentTaskRecord],
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    if dependencies.is_empty() {
        return Ok(Vec::new());
    }
    let pending_tool_call_id = dependencies
        .iter()
        .find_map(|dependency| dependency.pending_tool_call_id.clone())
        .ok_or_else(|| {
            ApiError::internal("Agent wait dependency is missing pending tool call id")
        })?;
    let mode = dependencies
        .first()
        .map(|dependency| dependency.wait_mode.as_str())
        .ok_or_else(|| ApiError::internal("Agent wait dependency list is empty"))?;
    let task_ids = dependencies
        .iter()
        .map(|dependency| dependency.dependency_task_id.to_string())
        .collect::<Vec<_>>();
    let tool_result = agent_wait_resume_tool_result(dependencies, dependency_tasks)?;
    let tool_result_content = serde_json::to_string(&tool_result).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize Agent wait tool result: {source}"
        ))
    })?;
    Ok(vec![
        neutral_agent_message(
            NeutralChatRole::System,
            AGENT_WAIT_RESUME_INSTRUCTION.to_string(),
        ),
        NeutralChatMessage {
            role: NeutralChatRole::Assistant,
            content: String::new(),
            attachments: Vec::new(),
            reasoning: None,
            tool_calls: vec![NeutralToolCall {
                call_id: pending_tool_call_id.clone(),
                name: foco_tools::AGENT_WAIT_TASKS_TOOL.to_string(),
                arguments: json!({
                    "taskIds": task_ids,
                    "mode": mode,
                    "deadlineMs": null,
                    "timeoutMs": null,
                }),
                thought_signatures: None,
            }],
            tool_call_id: None,
            tool_name: None,
        },
        NeutralChatMessage {
            role: NeutralChatRole::Tool,
            content: tool_result_content,
            attachments: Vec::new(),
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: Some(pending_tool_call_id),
            tool_name: Some(foco_tools::AGENT_WAIT_TASKS_TOOL.to_string()),
        },
    ])
}

fn agent_wait_resume_tool_result(
    dependencies: &[AgentTaskDependencyRecord],
    dependency_tasks: &[AgentTaskRecord],
) -> Result<Value, ApiError> {
    let deadline_at = dependencies
        .iter()
        .find_map(|dependency| dependency.deadline_at.clone());
    let dependency_values = dependencies
        .iter()
        .map(|dependency| {
            let task = dependency_tasks
                .iter()
                .find(|task| task.id == dependency.dependency_task_id)
                .ok_or_else(|| {
                    ApiError::internal(format!(
                        "Agent dependency task '{}' was not found",
                        dependency.dependency_task_id
                    ))
                })?;
            Ok(json!({
                "taskId": task.id.to_string(),
                "status": task.status.as_str(),
                "result": agent_optional_json(task.result_json.as_deref(), "Agent dependency task result")?,
                "error": agent_optional_json(task.error_json.as_deref(), "Agent dependency task error")?,
                "completedAt": task.completed_at,
            }))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(json!({
        "waiting": false,
        "mode": dependencies.first().map(|dependency| dependency.wait_mode.as_str()),
        "deadlineAt": deadline_at,
        "dependencies": dependency_values,
    }))
}

fn agent_optional_json(
    value: Option<&str>,
    label: &'static str,
) -> Result<Option<Value>, ApiError> {
    value
        .map(|value| {
            serde_json::from_str::<Value>(value)
                .map_err(|source| ApiError::internal(format!("failed to parse {label}: {source}")))
        })
        .transpose()
}

fn agent_message_payload(message: &AgentMessageRecord) -> Value {
    json!({
        "messageId": message.id.to_string(),
        "teamId": message.team_id.to_string(),
        "senderInstanceId": message.sender_instance_id.as_ref().map(ToString::to_string),
        "receiverInstanceId": message.receiver_instance_id.to_string(),
        "relatedTaskId": message.related_task_id.as_ref().map(ToString::to_string),
        "replyToMessageId": message.reply_to_message_id.as_ref().map(ToString::to_string),
        "kind": message.kind.as_str(),
        "content": message.content,
        "sequence": message.sequence,
        "createdAt": message.created_at,
    })
}

fn consume_agent_messages(
    workspace_path: &Path,
    message_ids: &[foco_agent::AgentMessageId],
) -> Result<(), ApiError> {
    if message_ids.is_empty() {
        return Ok(());
    }
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    for message_id in message_ids {
        let message = database
            .agent_message(message_id)
            .map_err(ApiError::from_workspace_error)?
            .ok_or_else(|| {
                ApiError::internal(format!("Agent message '{message_id}' was not found"))
            })?;
        let consumed = database
            .mark_agent_message_consumed(message_id)
            .map_err(ApiError::from_workspace_error)?;
        if consumed {
            database
                .append_agent_event(NewAgentEvent {
                    team_id: &message.team_id,
                    event_type: "message_consumed",
                    instance_id: Some(&message.receiver_instance_id),
                    task_id: message.related_task_id.as_ref(),
                    attempt_id: None,
                    message_id: Some(&message.id),
                    payload_json: &json!({
                        "senderInstanceId": message.sender_instance_id.as_ref().map(ToString::to_string),
                        "receiverInstanceId": message.receiver_instance_id.to_string(),
                        "kind": message.kind.as_str(),
                    })
                    .to_string(),
                })
                .map_err(ApiError::from_workspace_error)?;
        }
    }
    Ok(())
}

fn persist_agent_task_context(
    workspace_path: &Path,
    task: &AgentTaskRecord,
    instance: &AgentInstanceRecord,
    attempt_id: &AgentAttemptId,
    outcome: &AgentRunOutcome,
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let latest_snapshot = database
        .latest_agent_context_snapshot(&instance.id, instance.context_generation)
        .map_err(ApiError::from_workspace_error)?;
    let after_context_sequence = latest_snapshot
        .as_ref()
        .map(|snapshot| snapshot.sequence)
        .unwrap_or(-1);
    let context_entries = database
        .agent_context_entries(
            &instance.id,
            instance.context_generation,
            after_context_sequence,
        )
        .map_err(ApiError::from_workspace_error)?;
    let previous_sequence = context_entries
        .iter()
        .map(|entry| entry.sequence)
        .chain(latest_snapshot.as_ref().map(|snapshot| snapshot.sequence))
        .max()
        .unwrap_or(-1);
    let sequence = previous_sequence
        .checked_add(1)
        .ok_or_else(|| ApiError::internal("Agent private context sequence overflowed"))?;
    let content = agent_task_context_content(task, attempt_id, outcome);
    let content_json = content.to_string();
    let entry_id = unique_id("agent-context-entry");
    let role = agent_task_context_role(outcome);
    database
        .insert_agent_context_entry(NewAgentContextEntry {
            id: &entry_id,
            team_id: &task.team_id,
            instance_id: &instance.id,
            generation: instance.context_generation,
            sequence,
            role,
            content_json: &content_json,
            source_task_id: Some(&task.id),
            source_message_id: None,
        })
        .map_err(ApiError::from_workspace_error)?;

    let snapshot_entries = agent_context_snapshot_entries(&context_entries, sequence, &content)?;
    let snapshot_value = json!({
        "version": AGENT_CONTEXT_SNAPSHOT_VERSION,
        "teamProtocolVersion": AGENT_TEAM_PROTOCOL_VERSION,
        "buildVersion": "phase5",
        "teamId": task.team_id.to_string(),
        "instanceId": instance.id.to_string(),
        "generation": instance.context_generation,
        "taskId": task.id.to_string(),
        "attemptId": attempt_id.to_string(),
        "latestSequence": sequence,
        "previousSnapshotId": latest_snapshot.as_ref().map(|snapshot| snapshot.id.clone()),
        "entries": snapshot_entries,
    });
    let snapshot_json = snapshot_value.to_string();
    let token_count = i64::try_from(estimate_text_tokens(&snapshot_json)).map_err(|_| {
        ApiError::internal("Agent context snapshot token count exceeds SQLite integer range")
    })?;
    let snapshot_id = unique_id("agent-context-snapshot");
    database
        .insert_agent_context_snapshot(NewAgentContextSnapshot {
            id: &snapshot_id,
            team_id: &task.team_id,
            instance_id: &instance.id,
            generation: instance.context_generation,
            sequence,
            entries_json: &snapshot_json,
            token_count: Some(token_count),
        })
        .map_err(ApiError::from_workspace_error)?;
    Ok(())
}

fn agent_task_context_content(
    task: &AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    outcome: &AgentRunOutcome,
) -> Value {
    match outcome {
        AgentRunOutcome::Completed {
            text,
            reasoning,
            usage,
        } => json!({
            "status": "completed",
            "taskId": task.id.to_string(),
            "attemptId": attempt_id.to_string(),
            "summary": truncate_agent_context_text(text),
            "reasoningSummary": reasoning.as_ref().map(|value| truncate_agent_context_text(value)),
            "usage": usage,
        }),
        AgentRunOutcome::Failed { message, retryable } => json!({
            "status": "failed",
            "taskId": task.id.to_string(),
            "attemptId": attempt_id.to_string(),
            "message": truncate_agent_context_text(message),
            "retryable": retryable,
        }),
        AgentRunOutcome::Cancelled { message } => json!({
            "status": "cancelled",
            "taskId": task.id.to_string(),
            "attemptId": attempt_id.to_string(),
            "message": truncate_agent_context_text(message),
        }),
        AgentRunOutcome::Suspended { control } => json!({
            "status": "suspended",
            "taskId": task.id.to_string(),
            "attemptId": attempt_id.to_string(),
            "control": control,
        }),
    }
}

fn agent_task_context_role(outcome: &AgentRunOutcome) -> &'static str {
    match outcome {
        AgentRunOutcome::Completed { .. } | AgentRunOutcome::Suspended { .. } => "assistant",
        AgentRunOutcome::Failed { .. } | AgentRunOutcome::Cancelled { .. } => "system",
    }
}

fn agent_context_snapshot_entries(
    existing_entries: &[AgentContextEntryRecord],
    new_sequence: i64,
    new_content: &Value,
) -> Result<Vec<Value>, ApiError> {
    let keep_existing = AGENT_CONTEXT_SUMMARY_ENTRY_LIMIT.saturating_sub(1);
    let mut entries = existing_entries
        .iter()
        .rev()
        .take(keep_existing)
        .map(agent_context_snapshot_entry_value)
        .collect::<Result<Vec<_>, _>>()?;
    entries.reverse();
    entries.push(json!({
        "sequence": new_sequence,
        "content": new_content,
    }));
    Ok(entries)
}

fn agent_context_snapshot_entry_value(entry: &AgentContextEntryRecord) -> Result<Value, ApiError> {
    let content = serde_json::from_str::<Value>(&entry.content_json).map_err(|source| {
        ApiError::internal(format!("failed to parse Agent context entry: {source}"))
    })?;
    Ok(json!({
        "sequence": entry.sequence,
        "role": entry.role,
        "sourceTaskId": entry.source_task_id.as_ref().map(ToString::to_string),
        "sourceMessageId": entry.source_message_id.as_ref().map(ToString::to_string),
        "content": content,
    }))
}

fn truncate_agent_context_text(text: &str) -> String {
    if text.chars().count() <= AGENT_CONTEXT_SUMMARY_MAX_CHARS {
        return text.to_string();
    }
    text.chars()
        .take(AGENT_CONTEXT_SUMMARY_MAX_CHARS)
        .collect::<String>()
}

fn timestamp_delta_ms(start: Option<&str>, end: Option<&str>) -> Option<i64> {
    let start = chrono::DateTime::parse_from_rfc3339(start?).ok()?;
    let end = chrono::DateTime::parse_from_rfc3339(end?).ok()?;
    Some((end - start).num_milliseconds())
}

fn finish_claimed_task(
    workspace_path: &Path,
    task: &foco_store::workspace::AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    outcome: AgentRunOutcome,
) -> Result<(), ApiError> {
    let instance = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?
        .agent_instance(&task.owner_instance_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::internal(format!(
                "Agent instance '{}' was not found",
                task.owner_instance_id
            ))
        })?;
    let (transition, result, error, event_type) = match outcome {
        AgentRunOutcome::Completed {
            text,
            reasoning,
            usage,
        } => {
            let mut result = json!({ "text": text, "reasoning": reasoning, "usage": usage });
            if let Some(worktree) = agent_task_worktree_result(workspace_path, &instance)? {
                result["worktree"] = worktree;
            }
            (
                AgentTaskTransition::Complete,
                Some(result),
                None,
                "task_completed",
            )
        }
        AgentRunOutcome::Failed { message, retryable } => (
            AgentTaskTransition::Fail,
            None,
            Some(json!({ "message": message, "retryable": retryable })),
            "task_failed",
        ),
        AgentRunOutcome::Cancelled { message } => (
            AgentTaskTransition::Cancel,
            None,
            Some(json!({ "message": message })),
            "task_cancelled",
        ),
        AgentRunOutcome::Suspended { control } => (
            AgentTaskTransition::Wait,
            Some(json!({ "control": control })),
            None,
            "task_suspended",
        ),
    };
    let result_json = result
        .as_ref()
        .map(|value| agent_task_outcome_json(value, "result_json"))
        .transpose()?;
    let error_json = error
        .as_ref()
        .map(|value| agent_task_outcome_json(value, "error_json"))
        .transpose()?;
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let updated = database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &task.team_id,
            task_id: &task.id,
            expected_status: AgentTaskStatus::Running,
            transition,
            result_json: result_json.as_deref(),
            error_json: error_json.as_deref(),
            interruption_reason: None,
        })
        .map_err(ApiError::from_workspace_error)?;
    if !updated {
        return Err(ApiError::internal(format!(
            "Agent task '{}' changed state before its outcome was persisted",
            task.id
        )));
    }
    let completed_task = database
        .agent_task(&task.id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::internal(format!("Agent task '{}' was not found", task.id)))?;
    let payload = result.or(error).unwrap_or_else(|| json!({}));
    let payload = json!({
        "outcome": payload,
        "originInstanceId": task.origin_instance_id.as_ref().map(ToString::to_string),
        "parentTaskId": task.parent_task_id.as_ref().map(ToString::to_string),
        "runTimeMs": completed_task
            .completed_at
            .as_ref()
            .and_then(|completed_at| timestamp_delta_ms(task.started_at.as_deref(), Some(completed_at))),
    });
    insert_agent_event(
        &mut database,
        &task.team_id,
        event_type,
        Some(&task.owner_instance_id),
        Some(&task.id),
        Some(attempt_id),
        payload,
    )?;
    crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task(
        workspace_path,
        &task.id,
    )?;
    Ok(())
}

fn agent_instance_execution_root(workspace_path: &Path, instance: &AgentInstanceRecord) -> PathBuf {
    instance
        .execution_root_path
        .as_deref()
        .map(|root_path| resolve_agent_worktree_path(workspace_path, root_path))
        .unwrap_or_else(|| agent_instance_worktree_path(workspace_path, &instance.id))
}

fn agent_task_worktree_result(
    workspace_path: &Path,
    instance: &AgentInstanceRecord,
) -> Result<Option<Value>, ApiError> {
    if instance.execution_workspace_mode != AgentExecutionWorkspaceMode::IsolatedWorktree {
        return Ok(None);
    }
    let root_path = agent_instance_execution_root(workspace_path, instance);
    let diff = git_diff_response(&root_path, None)?;
    Ok(Some(json!({
        "mode": instance.execution_workspace_mode.as_str(),
        "rootPath": root_path.display().to_string(),
        "baseRevision": instance.worktree_base_revision,
        "branch": instance.worktree_branch,
        "status": instance.worktree_status,
        "diffId": agent_worktree_diff_id(&diff),
        "changedPaths": diff
            .files
            .iter()
            .chain(diff.staged_files.iter())
            .map(|file| file.path.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    })))
}

fn fail_claimed_task(
    workspace_path: &Path,
    task_id: &AgentTaskId,
    message: &str,
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let Some(task) = database
        .agent_task(task_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(());
    };
    if task.status != AgentTaskStatus::Running {
        return Ok(());
    }
    let mut error = json!({ "message": message });
    let mut error_json = error.to_string();
    if error_json.len() > AGENT_MAX_TASK_OUTCOME_BYTES {
        error = json!({
            "message": format!(
                "Agent task error_json exceeds {AGENT_MAX_TASK_OUTCOME_BYTES} bytes"
            )
        });
        error_json = error.to_string();
    }
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &task.team_id,
            task_id: &task.id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Fail,
            result_json: None,
            error_json: Some(&error_json),
            interruption_reason: None,
        })
        .map_err(ApiError::from_workspace_error)?;
    crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task(
        workspace_path,
        &task.id,
    )?;
    Ok(())
}

fn agent_task_outcome_json(value: &Value, field: &'static str) -> Result<String, ApiError> {
    let json = value.to_string();
    if json.len() > AGENT_MAX_TASK_OUTCOME_BYTES {
        return Err(ApiError::internal(format!(
            "Agent task {field} exceeds {AGENT_MAX_TASK_OUTCOME_BYTES} bytes"
        )));
    }
    Ok(json)
}

pub(crate) fn validate_agent_snapshot_for_workspace(
    config: &GlobalConfig,
    workspace: &WorkspaceConfig,
    definition: &AgentDefinitionSettings,
) -> Result<(), ApiError> {
    if !workspace.path.is_absolute() || !workspace.path.is_dir() {
        return Err(ApiError::bad_request(format!(
            "Agent workspace is no longer a valid directory: {}",
            workspace.path.display()
        )));
    }
    let model = config
        .models
        .iter()
        .find(|model| model.id == definition.model_id && model.enabled)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "Agent definition snapshot references unavailable model '{}'",
                definition.model_id
            ))
        })?;
    let limits = model.limits.as_ref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "Agent definition snapshot model '{}' is missing limits",
            definition.model_id
        ))
    })?;
    if definition
        .model_options
        .max_output_tokens
        .is_some_and(|value| u64::from(value) > limits.max_output_tokens)
    {
        return Err(ApiError::bad_request(format!(
            "Agent definition snapshot max output tokens exceed model '{}' limits",
            definition.model_id
        )));
    }
    if !model
        .provider_ids
        .iter()
        .any(|provider_id| provider_id == &definition.provider_id)
    {
        return Err(ApiError::bad_request(format!(
            "Agent definition snapshot provider '{}' is not associated with model '{}'",
            definition.provider_id, definition.model_id
        )));
    }
    if !config
        .providers
        .iter()
        .any(|provider| provider.id == definition.provider_id && provider.enabled)
    {
        return Err(ApiError::bad_request(format!(
            "Agent definition snapshot references unavailable provider '{}'",
            definition.provider_id
        )));
    }
    Ok(())
}

pub(crate) fn insert_agent_event(
    database: &mut WorkspaceDatabase,
    team_id: &foco_agent::AgentTeamId,
    event_type: &str,
    instance_id: Option<&foco_agent::AgentInstanceId>,
    task_id: Option<&AgentTaskId>,
    attempt_id: Option<&AgentAttemptId>,
    payload: Value,
) -> Result<(), ApiError> {
    let payload_json = payload.to_string();
    database
        .append_agent_event(NewAgentEvent {
            team_id,
            event_type,
            instance_id,
            task_id,
            attempt_id,
            message_id: None,
            payload_json: &payload_json,
        })
        .map_err(ApiError::from_workspace_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn wake_signal_is_coalesced_without_blocking() {
        let (scheduler, mut receiver) = AgentScheduler::new();
        scheduler.wake().expect("first wake");
        scheduler.wake().expect("coalesced wake");
        assert_eq!(receiver.recv().await, Some(()));
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn global_run_permits_enforce_the_process_limit() {
        let permits = Arc::new(Semaphore::new(AGENT_GLOBAL_MAX_CONCURRENT_RUNS));
        let held = (0..AGENT_GLOBAL_MAX_CONCURRENT_RUNS)
            .map(|_| permits.clone().try_acquire_owned().expect("run permit"))
            .collect::<Vec<_>>();
        assert!(permits.clone().try_acquire_owned().is_err());
        drop(held);
        assert!(permits.try_acquire_owned().is_ok());
    }

    #[test]
    fn agent_scheduler_deadline_delay_has_idle_and_past_deadline_bounds() {
        let past = Utc::now() - chrono::Duration::seconds(1);

        assert_eq!(
            agent_scheduler_deadline_delay(None),
            Duration::from_secs(86_400)
        );
        assert_eq!(
            agent_scheduler_deadline_delay(Some(&past)),
            Duration::from_millis(AGENT_SCHEDULER_MIN_DEADLINE_DELAY_MS)
        );
    }

    #[test]
    fn agent_task_outcome_json_rejects_oversized_payload() {
        assert!(agent_task_outcome_json(&json!({ "text": "ok" }), "result_json").is_ok());

        let oversized = json!({ "text": "x".repeat(AGENT_MAX_TASK_OUTCOME_BYTES) });
        assert!(agent_task_outcome_json(&oversized, "result_json").is_err());
    }

    fn agent_team_protocol_json_from_prompt(prompt: &str) -> Value {
        assert!(prompt.contains("<subagents>"));
        let protocol_prompt = prompt
            .split_once("<agent_team_protocol>")
            .map(|(_, rest)| format!("<agent_team_protocol>{rest}"))
            .expect("protocol section");
        assert!(protocol_prompt.starts_with("<agent_team_protocol>\n<json><![CDATA[\n"));
        assert!(protocol_prompt.ends_with("\n]]></json>\n</agent_team_protocol>"));
        let json_text = protocol_prompt
            .strip_prefix("<agent_team_protocol>\n<json><![CDATA[\n")
            .expect("protocol prefix")
            .strip_suffix("\n]]></json>\n</agent_team_protocol>")
            .expect("protocol suffix");
        serde_json::from_str(json_text).expect("protocol json")
    }

    #[test]
    fn agent_team_protocol_inserts_before_stable_context() {
        let sources = vec![
            PromptContextSource::ReservedPrompt,
            PromptContextSource::AgentDefinition,
            PromptContextSource::StableInjection,
            PromptContextSource::ProjectSpec,
            PromptContextSource::CurrentUser { sequence: 7 },
        ];

        assert_eq!(agent_team_protocol_insert_index_for_sources(&sources, 5), 2);
    }

    #[test]
    fn team_protocol_expands_creatable_definition_schema() {
        let coordinator_definition = test_agent_definition("coordinator", 1);
        let mut worker_definition = test_agent_definition("worker", 3);
        worker_definition.allowed_execution_workspace_modes =
            vec![AgentExecutionWorkspaceMode::Shared];
        let team_id = foco_agent::AgentTeamId::new("agent-team-protocol").expect("team id");
        let coordinator_id =
            foco_agent::AgentInstanceId::new("agent-instance-coordinator").expect("instance id");
        let worker_id =
            foco_agent::AgentInstanceId::new("agent-instance-worker").expect("instance id");
        let task_id = AgentTaskId::new("agent-task-protocol").expect("task id");
        let attempt_id = AgentAttemptId::new("agent-attempt-protocol").expect("attempt id");
        let now = "2026-01-01T00:00:00Z".to_string();
        let team = AgentTeamRecord {
            id: team_id.clone(),
            chat_id: "chat-protocol".to_string(),
            coordinator_instance_id: coordinator_id.clone(),
            status: foco_agent::AgentTeamStatus::Active,
            max_concurrent_runs: 1,
            next_event_sequence: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let coordinator = test_agent_instance(
            &team_id,
            &coordinator_id,
            coordinator_definition.clone(),
            AgentRole::Coordinator,
            &now,
        );
        let worker = test_agent_instance(
            &team_id,
            &worker_id,
            worker_definition.clone(),
            AgentRole::Worker,
            &now,
        );
        let task = AgentTaskRecord {
            id: task_id,
            team_id: team_id.clone(),
            owner_instance_id: coordinator_id,
            origin_instance_id: None,
            parent_task_id: None,
            sequence: 0,
            status: AgentTaskStatus::Running,
            input_json: "{}".to_string(),
            result_json: None,
            error_json: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            started_at: Some(now.clone()),
            completed_at: None,
        };
        let permissions = AgentPermissions {
            can_create_instances: true,
            can_delegate: false,
            allowed_agent_definition_ids: vec![worker_definition.id.clone()],
        };

        let prompt = agent_team_protocol_prompt(
            &team,
            &coordinator,
            &task,
            &attempt_id,
            &HashSet::new(),
            &permissions,
            &[coordinator_definition, worker_definition],
            &[coordinator.clone(), worker],
        )
        .expect("protocol prompt");
        let protocol = agent_team_protocol_json_from_prompt(&prompt);
        let creatable = protocol["creatableAgentDefinitions"]
            .as_array()
            .expect("creatable definitions");

        assert_eq!(protocol["version"], json!(2));
        assert_eq!(creatable.len(), 1);
        assert_eq!(
            creatable[0]["definitionId"],
            json!("agent-definition-worker")
        );
        assert_eq!(creatable[0]["maxInstances"], json!(3));
        assert_eq!(creatable[0]["currentTeamInstances"], json!(2));
        assert_eq!(creatable[0]["currentTeamDefinitionInstances"], json!(1));
        assert_eq!(creatable[0]["remainingTeamDefinitionSlots"], json!(2));
        assert_eq!(creatable[0]["maxCreateCount"], json!(2));
        assert_eq!(creatable[0]["canCreateMore"], json!(true));
        assert_eq!(
            creatable[0]["agentCreateInstancesSchema"]["count"]["maximum"],
            json!(2)
        );
        assert_eq!(
            creatable[0]["allowedExecutionWorkspaceModes"],
            json!(["shared"])
        );
        assert_eq!(
            creatable[0]["agentCreateInstancesSchema"]["executionWorkspaceMode"]["enum"],
            json!(["shared"])
        );
        assert!(creatable[0]["agentCreateInstancesSchema"]["maxInstancesPerTeam"].is_null());
        assert!(creatable[0]["agentCreateInstancesSchema"]["maxInstancesForDefinition"].is_null());
    }

    #[test]
    fn team_protocol_uses_instance_snapshot_for_stale_creatable_definition() {
        let coordinator_definition = test_agent_definition("stale-coordinator", 1);
        let worker_definition = test_agent_definition("stale-worker", 3);
        let missing_id = foco_agent::AgentDefinitionId::new("agent-definition-stale-missing")
            .expect("missing definition id");
        let team_id = foco_agent::AgentTeamId::new("agent-team-stale-creatable").expect("team id");
        let coordinator_id = foco_agent::AgentInstanceId::new("agent-instance-stale-coordinator")
            .expect("coordinator id");
        let worker_id =
            foco_agent::AgentInstanceId::new("agent-instance-stale-worker").expect("worker id");
        let task_id = AgentTaskId::new("agent-task-stale-creatable").expect("task id");
        let attempt_id = AgentAttemptId::new("agent-attempt-stale-creatable").expect("attempt id");
        let now = "2026-01-01T00:00:00Z".to_string();
        let team = AgentTeamRecord {
            id: team_id.clone(),
            chat_id: "chat-stale-creatable".to_string(),
            coordinator_instance_id: coordinator_id.clone(),
            status: foco_agent::AgentTeamStatus::Active,
            max_concurrent_runs: 1,
            next_event_sequence: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let coordinator = test_agent_instance(
            &team_id,
            &coordinator_id,
            coordinator_definition.clone(),
            AgentRole::Coordinator,
            &now,
        );
        let worker = test_agent_instance(
            &team_id,
            &worker_id,
            worker_definition.clone(),
            AgentRole::Worker,
            &now,
        );
        let task = AgentTaskRecord {
            id: task_id,
            team_id: team_id.clone(),
            owner_instance_id: coordinator_id,
            origin_instance_id: None,
            parent_task_id: None,
            sequence: 0,
            status: AgentTaskStatus::Running,
            input_json: "{}".to_string(),
            result_json: None,
            error_json: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            started_at: Some(now.clone()),
            completed_at: None,
        };
        let permissions = AgentPermissions {
            can_create_instances: true,
            can_delegate: false,
            allowed_agent_definition_ids: vec![worker_definition.id.clone(), missing_id],
        };

        let prompt = agent_team_protocol_prompt(
            &team,
            &coordinator,
            &task,
            &attempt_id,
            &HashSet::new(),
            &permissions,
            &[coordinator_definition],
            &[coordinator.clone(), worker],
        )
        .expect("protocol prompt");
        let protocol = agent_team_protocol_json_from_prompt(&prompt);
        let creatable = protocol["creatableAgentDefinitions"]
            .as_array()
            .expect("creatable definitions");

        assert_eq!(creatable.len(), 1);
        assert_eq!(
            creatable[0]["definitionId"],
            json!("agent-definition-stale-worker")
        );
        assert_eq!(creatable[0]["maxInstances"], json!(3));
    }

    #[test]
    fn wait_resume_messages_include_parent_resume_instruction() {
        let team_id = foco_agent::AgentTeamId::new("agent-team-wait-resume").expect("team id");
        let waiting_task_id = AgentTaskId::new("agent-task-waiting").expect("waiting task id");
        let dependency_task_id =
            AgentTaskId::new("agent-task-dependency").expect("dependency task id");
        let worker_id =
            foco_agent::AgentInstanceId::new("agent-instance-worker").expect("worker id");
        let now = "2026-01-01T00:00:00Z".to_string();
        let dependencies = vec![foco_store::workspace::AgentTaskDependencyRecord {
            team_id: team_id.clone(),
            waiting_task_id: waiting_task_id.clone(),
            dependency_task_id: dependency_task_id.clone(),
            wait_mode: foco_agent::AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait".to_string()),
            deadline_at: None,
            created_at: now.clone(),
        }];
        let dependency_tasks = vec![AgentTaskRecord {
            id: dependency_task_id,
            team_id,
            owner_instance_id: worker_id,
            origin_instance_id: None,
            parent_task_id: Some(waiting_task_id),
            sequence: 0,
            status: AgentTaskStatus::Completed,
            input_json: "{}".to_string(),
            result_json: Some(r#"{"text":"worker result"}"#.to_string()),
            error_json: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            started_at: Some(now.clone()),
            completed_at: Some(now),
        }];

        let messages = agent_wait_resume_messages(&dependencies, &dependency_tasks)
            .expect("wait resume messages");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, NeutralChatRole::System);
        assert_eq!(messages[0].content, AGENT_WAIT_RESUME_INSTRUCTION);
        assert_eq!(messages[1].role, NeutralChatRole::Assistant);
        assert_eq!(messages[1].tool_calls.len(), 1);
        assert_eq!(messages[1].tool_calls[0].call_id, "call-wait");
        assert_eq!(messages[2].role, NeutralChatRole::Tool);
        assert_eq!(messages[2].tool_call_id.as_deref(), Some("call-wait"));
        assert!(messages[2].content.contains("worker result"));
    }

    #[test]
    fn agent_task_model_selection_uses_queued_user_message_override() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        let team_id = foco_agent::AgentTeamId::new("agent-team-model-selection").expect("team id");
        let coordinator_id = foco_agent::AgentInstanceId::new("agent-instance-model-selection")
            .expect("instance id");
        let now = "2026-01-01T00:00:00Z".to_string();
        let team = AgentTeamRecord {
            id: team_id.clone(),
            chat_id: "chat-model-selection".to_string(),
            coordinator_instance_id: coordinator_id.clone(),
            status: foco_agent::AgentTeamStatus::Active,
            max_concurrent_runs: 1,
            next_event_sequence: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let mut definition = test_agent_definition("coordinator", 1);
        definition.model_id = "snapshot-model".to_string();
        definition.provider_id = "snapshot-provider".to_string();
        definition.model_options.thinking_level = Some("snapshot-thinking".to_string());
        let instance = test_agent_instance(
            &team_id,
            &coordinator_id,
            definition,
            AgentRole::Coordinator,
            &now,
        );
        let task_input = CoordinatorTaskInput {
            queued_user_message_id: "user-model-selection".to_string(),
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            message: "Use override".to_string(),
            attachments: Vec::new(),
            skill_ids: Vec::new(),
            session_mode: None,
            collaboration_tools_enabled: false,
            defer_until_workspace_idle: false,
            delegated_input: None,
            correlation_id: None,
        };

        database
            .insert_chat(&team.chat_id, "Model selection")
            .expect("chat insert");
        database
            .insert_message(foco_store::workspace::NewMessage {
                id: &task_input.queued_user_message_id,
                chat_id: &team.chat_id,
                role: "user",
                content: "Use override",
                sequence: 0,
                metadata_json: Some(
                    r#"{"queuedRun":{"status":"queued","modelId":"queued-model","providerId":"queued-provider","thinkingLevel":"queued-thinking","skillIds":[]}}"#,
                ),
            })
            .expect("message insert");

        let selection = agent_task_model_selection(&database, &team, &instance, &task_input)
            .expect("selection");

        assert_eq!(selection.model_id, "queued-model");
        assert_eq!(selection.provider_id.as_deref(), Some("queued-provider"));
        assert_eq!(selection.thinking_level.as_deref(), Some("queued-thinking"));
    }

    fn test_agent_definition(suffix: &str, max_instances: u32) -> AgentDefinitionSettings {
        AgentDefinitionSettings {
            id: foco_agent::AgentDefinitionId::new(format!("agent-definition-{suffix}"))
                .expect("definition id"),
            revision: 1,
            name: suffix.to_string(),
            description: format!("{suffix} definition"),
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            model_options: foco_store::config::AgentModelOptions::default(),
            system_prompt: "Do the task.".to_string(),
            allowed_tools: Vec::new(),
            max_instances,
            allowed_execution_workspace_modes: AgentExecutionWorkspaceMode::all(),
            permissions: AgentPermissions::default(),
        }
    }

    fn test_agent_instance(
        team_id: &foco_agent::AgentTeamId,
        instance_id: &foco_agent::AgentInstanceId,
        definition: AgentDefinitionSettings,
        role: AgentRole,
        now: &str,
    ) -> AgentInstanceRecord {
        AgentInstanceRecord {
            id: instance_id.clone(),
            team_id: team_id.clone(),
            definition_id: definition.id.clone(),
            definition_revision: definition.revision,
            definition_snapshot: definition,
            role,
            status: AgentInstanceStatus::Idle,
            next_task_sequence: 0,
            next_message_sequence: 0,
            context_generation: 0,
            last_scheduled_at: None,
            execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
            execution_root_path: None,
            worktree_base_revision: None,
            worktree_branch: None,
            worktree_status: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        }
    }
}
