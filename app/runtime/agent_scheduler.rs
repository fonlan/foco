use std::{collections::HashSet, sync::Arc};

use foco_agent::{
    AgentAttemptId, AgentInstanceStatus, AgentRole, AgentRunAssociations, AgentRunOutcome,
    AgentTaskId, AgentTaskStatus, AgentTaskTransition, estimate_text_tokens,
};
use foco_providers::{NeutralChatMessage, NeutralChatRole};
use foco_store::{
    config::AGENT_DEFINITION_SYSTEM_PROMPT_MAX_CHARS,
    workspace::{
        AgentContextEntryRecord, AgentInstanceRecord, AgentMessageRecord, AgentTaskRecord,
        AgentTaskStateUpdate, AgentTeamRecord, NewAgentContextEntry, NewAgentContextSnapshot,
        NewAgentEvent, WorkspaceDatabase,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    sync::{Semaphore, mpsc},
    task::{JoinHandle, JoinSet},
};

use crate::*;

// ponytail: fixed first-slice limits avoid new config surface; make them configurable when
// production workload data shows a different ceiling is needed.
pub(crate) const AGENT_MAX_QUEUED_TASKS_PER_TEAM: i64 = 64;
pub(crate) const AGENT_MAX_QUEUED_TASKS_PER_INSTANCE: i64 = 64;
pub(crate) const AGENT_MAX_QUEUED_TASKS_PER_CHAT: i64 = 64;
const AGENT_SCHEDULER_WAKE_CAPACITY: usize = 1;
const AGENT_SCHEDULER_SCAN_LIMIT: i64 = 64;
const AGENT_GLOBAL_MAX_CONCURRENT_RUNS: usize = 4;
const RESTART_INTERRUPTION_REASON: &str = "backend restarted while Agent attempt was active";
const AGENT_TEAM_PROTOCOL_VERSION: u32 = 1;
const AGENT_CONTEXT_SNAPSHOT_VERSION: u32 = 1;
const AGENT_CONTEXT_RECENT_MESSAGE_LIMIT: usize = 8;
const AGENT_CONTEXT_SUMMARY_ENTRY_LIMIT: usize = 16;
const AGENT_CONTEXT_SUMMARY_MAX_CHARS: usize = 320;

#[derive(Clone)]
pub(crate) struct AgentScheduler {
    wake_tx: mpsc::Sender<()>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CoordinatorTaskInput {
    pub(crate) queued_user_message_id: String,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) attachments: Vec<ChatAttachmentInput>,
    #[serde(default)]
    pub(crate) skill_ids: Vec<String>,
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

    loop {
        if scan {
            scan = false;
            if let Err(error) = schedule_runnable_tasks(&state, &permits, &mut runs).await {
                tracing::error!(error = %error.message, "Agent scheduler scan failed");
            }
        }

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
            if !matches!(
                expected_status,
                AgentTaskStatus::Running | AgentTaskStatus::Waiting
            ) {
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
        }
    }
    Ok(())
}

async fn schedule_runnable_tasks(
    state: &AppState,
    permits: &Arc<Semaphore>,
    runs: &mut JoinSet<()>,
) -> Result<(), ApiError> {
    let config = config_snapshot(state)?;
    'scan: for workspace in &config.workspaces {
        loop {
            let Ok(permit) = permits.clone().try_acquire_owned() else {
                break 'scan;
            };
            let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            let Some(task) = database
                .runnable_agent_tasks(AGENT_SCHEDULER_SCAN_LIMIT)
                .map_err(ApiError::from_workspace_error)?
                .into_iter()
                .next()
            else {
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
            let state = state.clone();
            let workspace = workspace.clone();
            runs.spawn(async move {
                let _permit = permit;
                run_coordinator_task(state, workspace, claimed.id, attempt_id).await;
            });
        }
    }
    Ok(())
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
            model_id: instance.definition_snapshot.model_id.clone(),
            provider_id: Some(instance.definition_snapshot.provider_id.clone()),
            thinking_level: instance
                .definition_snapshot
                .model_options
                .thinking_level
                .clone(),
            skill_ids: Some(task_input.skill_ids.clone()),
            message: task_input.message.clone(),
            attachments: task_input.attachments.clone(),
        },
    )
    .await?;
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
    chat_context.session_upload_paths = Some(session_upload_paths);

    let (guidance_tx, guidance_rx) = mpsc::unbounded_channel();
    let registration = state.active_chat_runs.register(
        task.id.to_string(),
        workspace.id.clone(),
        team.chat_id.clone(),
        chat_context.assistant_message_id.clone(),
        chat_context.assistant_sequence,
        chat_context.memories_used.clone(),
        guidance_tx,
    )?;
    let outcome = run_chat_context_in_background(chat_context, registration, guidance_rx).await;
    persist_agent_task_context(&workspace.path, &task, &instance, attempt_id, &outcome)?;
    finish_claimed_task(&workspace.path, &task, attempt_id, outcome)
}

fn apply_agent_prompt_layers(
    workspace_path: &Path,
    chat_context: &mut PreparedChatContext,
    team: &AgentTeamRecord,
    instance: &AgentInstanceRecord,
    task: &AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    allowed_tools: &HashSet<String>,
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
            agent_team_protocol_prompt(team, instance, task, attempt_id, allowed_tools)?,
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

    let current_task = agent_current_task_prompt(task, attempt_id)?;
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

    let mut run_unread_messages = Vec::with_capacity(unread_messages.len());
    let mut consumed_message_ids = Vec::with_capacity(unread_messages.len());
    for message in unread_messages {
        let payload = agent_message_payload(&message);
        let prompt = format!(
            "Foco Agent unread message:\n{}",
            serde_json::to_string_pretty(&payload).map_err(|source| {
                ApiError::internal(format!(
                    "failed to serialize Agent message prompt: {source}"
                ))
            })?
        );
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
    chat_context
        .message_context_sources
        .iter()
        .position(|source| {
            !matches!(
                source,
                PromptContextSource::ReservedPrompt
                    | PromptContextSource::AgentDefinition
                    | PromptContextSource::StableInjection
            )
        })
        .unwrap_or(chat_context.active_tool_start_index)
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

fn agent_team_protocol_prompt(
    team: &AgentTeamRecord,
    instance: &AgentInstanceRecord,
    task: &AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    allowed_tools: &HashSet<String>,
) -> Result<String, ApiError> {
    let mut tools = allowed_tools.iter().cloned().collect::<Vec<_>>();
    tools.sort();
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
        "permissions": instance.definition_snapshot.permissions,
        "allowedRuntimeTools": tools,
        "runtimeLimits": {
            "maxQueuedTasksPerTeam": AGENT_MAX_QUEUED_TASKS_PER_TEAM,
            "maxQueuedTasksPerInstance": AGENT_MAX_QUEUED_TASKS_PER_INSTANCE,
            "maxQueuedTasksPerChat": AGENT_MAX_QUEUED_TASKS_PER_CHAT,
            "maxAgentToolRounds": MAX_AGENT_TOOL_ROUNDS,
        },
        "outputPolicy": {
            "coordinatorWritesMainChat": true,
            "workerWritesMainChat": false,
            "workerAutomaticMemoryExtraction": false,
        },
    });
    Ok(format!(
        "Foco Agent team protocol:\n{}",
        serde_json::to_string_pretty(&protocol).map_err(|source| {
            ApiError::internal(format!("failed to serialize Agent team protocol: {source}"))
        })?
    ))
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
    Ok(Some(format!(
        "Foco Agent private context:\n{}",
        serde_json::to_string_pretty(&context).map_err(|source| {
            ApiError::internal(format!(
                "failed to serialize Agent private context: {source}"
            ))
        })?
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
) -> Result<String, ApiError> {
    let input = serde_json::from_str::<Value>(&task.input_json).map_err(|source| {
        ApiError::internal(format!("failed to parse Agent task input: {source}"))
    })?;
    let current_task = json!({
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
    Ok(format!(
        "Foco Agent current task:\n{}",
        serde_json::to_string_pretty(&current_task).map_err(|source| {
            ApiError::internal(format!("failed to serialize Agent current task: {source}"))
        })?
    ))
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
        database
            .mark_agent_message_consumed(message_id)
            .map_err(ApiError::from_workspace_error)?;
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

fn finish_claimed_task(
    workspace_path: &Path,
    task: &foco_store::workspace::AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    outcome: AgentRunOutcome,
) -> Result<(), ApiError> {
    let (transition, result, error, event_type) = match outcome {
        AgentRunOutcome::Completed {
            text,
            reasoning,
            usage,
        } => (
            AgentTaskTransition::Complete,
            Some(json!({ "text": text, "reasoning": reasoning, "usage": usage })),
            None,
            "task_completed",
        ),
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
    let result_json = result.as_ref().map(Value::to_string);
    let error_json = error.as_ref().map(Value::to_string);
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
    insert_agent_event(
        &mut database,
        &task.team_id,
        event_type,
        Some(&task.owner_instance_id),
        Some(&task.id),
        Some(attempt_id),
        result.or(error).unwrap_or_else(|| json!({})),
    )?;
    Ok(())
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
    let error_json = json!({ "message": message }).to_string();
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
    Ok(())
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
}
