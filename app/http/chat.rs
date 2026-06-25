use std::{
    collections::{BTreeMap, HashSet},
    convert::Infallible,
    pin::Pin,
    time::{Duration, Instant},
};

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
    response::sse::{Event, KeepAlive, KeepAliveStream, Sse},
};
use foco_store::{
    memory::MemoryDatabase,
    workspace::{
        LlmRequestAuditFilters, LlmRequestAuditModelBreakdown, LlmRequestAuditProviderBreakdown,
        LlmRequestAuditSummaryRow, LlmRequestAuditTrendPoint, TodoGraphFilter, WorkspaceDatabase,
        workspace_database_path,
    },
};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc};

use crate::*;

type BoxedChatEventStream =
    Pin<Box<dyn futures_util::Stream<Item = Result<Event, Infallible>> + Send>>;
type BoxedChatSse = Sse<KeepAliveStream<BoxedChatEventStream>>;

const DEFAULT_AGENT_DEFINITION_ID: &str = "agent-definition-default";
const DEFAULT_AGENT_SYSTEM_PROMPT: &str = "You are Foco's default coding agent. Complete simple tasks directly. For complex tasks, consider creating and coordinating multiple worker agents when they can help with parallel investigation, implementation, review, or verification.";
const TEAM_CHAT_TASK_STREAM_POLL_INTERVAL: Duration = Duration::from_millis(100);
const MAX_CHAT_MESSAGES_PAGE_LIMIT: usize = 500;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatMessagesQuery {
    limit: Option<usize>,
    before_sequence: Option<i64>,
}

#[derive(Clone, Debug)]
pub(crate) enum QueuedChatMessageOrigin {
    User,
    #[allow(dead_code)] // Phase 5 scheduler dispatch will construct this outside tests.
    ScheduledTask {
        task_id: String,
        run_id: String,
        trigger_reason: String,
    },
}

impl QueuedChatMessageOrigin {
    fn metadata_value(&self) -> Option<serde_json::Value> {
        match self {
            Self::User => None,
            Self::ScheduledTask {
                task_id,
                run_id,
                trigger_reason,
            } => Some(serde_json::json!({
                "source": "scheduled_task",
                "scheduledTaskId": task_id,
                "scheduledTaskRunId": run_id,
                "triggerReason": trigger_reason,
            })),
        }
    }
}

pub(crate) struct QueueChatMessageInput {
    pub(crate) chat_id: Option<String>,
    pub(crate) model_id: String,
    pub(crate) provider_id: Option<String>,
    pub(crate) thinking_level: Option<String>,
    pub(crate) skill_ids: Option<Vec<String>>,
    pub(crate) message: String,
    pub(crate) team_mode_enabled: bool,
    pub(crate) defer_start: bool,
    pub(crate) attachments: Vec<ChatAttachmentInput>,
    pub(crate) agent_definition_id: Option<String>,
    pub(crate) origin: QueuedChatMessageOrigin,
}

pub(crate) struct QueuedChatMessageArtifacts {
    pub(crate) chat_id: String,
    pub(crate) chat_title: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) user_message_id: String,
    pub(crate) assistant_message_id: String,
    pub(crate) content: String,
    pub(crate) parts: Vec<ChatMessagePart>,
    pub(crate) agent_team_id: Option<foco_agent::AgentTeamId>,
    pub(crate) agent_task_id: Option<foco_agent::AgentTaskId>,
}

pub(crate) async fn queue_chat_message(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<QueueChatMessageRequest>,
) -> Result<Json<QueueChatMessageResponse>, ApiError> {
    let queued = queue_chat_message_internal(
        &state,
        &workspace_id,
        QueueChatMessageInput {
            chat_id: request.chat_id,
            model_id: request.model_id,
            provider_id: request.provider_id,
            thinking_level: request.thinking_level,
            skill_ids: request.skill_ids,
            message: request.message,
            team_mode_enabled: request.team_mode_enabled,
            defer_start: request.defer_start,
            attachments: request.attachments,
            agent_definition_id: None,
            origin: QueuedChatMessageOrigin::User,
        },
    )
    .await?;

    Ok(Json(QueueChatMessageResponse {
        chat_id: queued.chat_id,
        chat_title: queued.chat_title,
        created_at: queued.created_at,
        updated_at: queued.updated_at,
        user_message_id: queued.user_message_id,
        assistant_message_id: queued.assistant_message_id,
        content: queued.content,
        parts: queued.parts,
        agent_team_id: queued.agent_team_id,
        agent_task_id: queued.agent_task_id,
    }))
}

pub(crate) async fn queue_chat_message_internal(
    state: &AppState,
    workspace_id: &str,
    input: QueueChatMessageInput,
) -> Result<QueuedChatMessageArtifacts, ApiError> {
    let config = config_snapshot(state)?;
    let workspace = workspace_by_id(&config, workspace_id)?;
    let QueueChatMessageInput {
        chat_id,
        model_id,
        provider_id,
        thinking_level,
        skill_ids,
        message: task_message,
        team_mode_enabled,
        defer_start,
        attachments,
        agent_definition_id,
        origin,
    } = input;
    let mut team = if let Some(chat_id) = chat_id.as_deref() {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        database
            .agent_team_for_chat(chat_id)
            .map_err(ApiError::from_workspace_error)?
    } else {
        None
    };
    let mut coordinator = if let Some(team) = team.as_ref() {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        let instance = database
            .agent_instance(&team.coordinator_instance_id)
            .map_err(ApiError::from_workspace_error)?
            .ok_or_else(|| ApiError::internal("Agent team Coordinator instance was not found"))?;
        validate_agent_snapshot_for_workspace(&config, workspace, &instance.definition_snapshot)?;
        Some(instance)
    } else {
        None
    };
    let requested_model_id = model_id.clone();
    let requested_provider_id = provider_id.clone();
    let requested_thinking_level = thinking_level.clone();
    let requested_skill_ids = skill_ids.clone().unwrap_or_default();
    let origin_metadata = origin.metadata_value();
    let preallocated_chat_id = if chat_id.is_none() {
        Some(unique_id("chat"))
    } else {
        None
    };
    let prompt_context = prepare_prompt_context(
        state,
        &config,
        workspace_id,
        PromptContextRequest {
            chat_id,
            queued_user_message_id: None,
            model_id,
            provider_id,
            thinking_level,
            skill_ids,
            message: Some(task_message.clone()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments,
        },
        preallocated_chat_id,
        PromptAssemblyPurpose::ChatRun,
    )
    .await?;
    let raw_message = prompt_context.raw_message.as_deref().unwrap_or("");
    let message = prompt_context
        .message
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("message must not be empty"))?;
    let task_attachments = prompt_context
        .attachments
        .iter()
        .map(|attachment| ChatAttachmentInput {
            id: attachment.id.clone(),
            name: attachment.name.clone(),
            content_type: attachment.content_type.clone(),
            content_base64: attachment.content_base64.clone(),
            path: attachment.path.clone(),
            size_bytes: attachment.size_bytes,
        })
        .collect::<Vec<_>>();
    let mut database = WorkspaceDatabase::open_or_create(&prompt_context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let user_message_id = unique_id("msg-user");
    let assistant_message_id = unique_id("msg-assistant");
    let assistant_sequence = prompt_context.next_message_sequence + 1;
    let user_metadata_json = queued_user_message_metadata_json(
        &prompt_context.attachments,
        &assistant_message_id,
        assistant_sequence,
        &requested_model_id,
        requested_provider_id.as_deref(),
        requested_thinking_level.as_deref(),
        &requested_skill_ids,
        origin_metadata.as_ref(),
    )?;

    let (chat_id, chat_title) = if prompt_context.is_new_chat {
        let chat_id = prompt_context
            .chat_id
            .clone()
            .ok_or_else(|| ApiError::internal("new chat is missing preallocated id"))?;
        let title = chat_title_for_prompt(raw_message, &prompt_context.attachments);
        let chat_metadata_json = queued_chat_metadata_json(
            &user_message_id,
            &assistant_message_id,
            assistant_sequence,
            &requested_model_id,
            requested_provider_id.as_deref(),
            requested_thinking_level.as_deref(),
            &requested_skill_ids,
            message,
            origin_metadata.as_ref(),
        )?;
        database
            .insert_chat_with_metadata(&chat_id, &title, &chat_metadata_json)
            .map_err(ApiError::from_workspace_error)?;
        (chat_id, title)
    } else {
        let chat_id = prompt_context
            .chat_id
            .clone()
            .ok_or_else(|| ApiError::bad_request("chat id must not be empty"))?;
        let chat = database
            .chat(&chat_id)
            .map_err(ApiError::from_workspace_error)?
            .ok_or_else(|| ApiError::bad_request(format!("chat was not found: {chat_id}")))?;
        (chat_id, chat.title)
    };
    persist_pending_chat_spec_snapshot(
        &mut database,
        &chat_id,
        prompt_context.pending_spec_snapshot.as_ref(),
    )?;

    if team.is_none() {
        let definition = match agent_definition_id.as_deref() {
            Some(id) => configured_agent_definition(&config, id)?,
            None => default_agent_definition(&state, &config, &prompt_context).await?,
        };
        validate_agent_snapshot_for_workspace(&config, workspace, &definition)?;
        let team_id = foco_agent::AgentTeamId::new(unique_id("agent-team"))
            .map_err(|error| ApiError::internal(error.to_string()))?;
        let instance_id = foco_agent::AgentInstanceId::new(unique_id("agent-instance"))
            .map_err(|error| ApiError::internal(error.to_string()))?;
        let (created_team, created_coordinator) = database
            .create_agent_team(foco_store::workspace::NewAgentTeam {
                id: &team_id,
                chat_id: &chat_id,
                coordinator_instance_id: &instance_id,
                coordinator_definition: &definition,
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
            serde_json::json!({ "coordinatorDefinitionId": definition.id, "defaultAgent": true }),
        )?;
        team = Some(created_team);
        coordinator = Some(created_coordinator);
    }

    if let (Some(team), Some(coordinator)) = (&team, &mut coordinator) {
        resume_chat_coordinator_for_new_message(&mut database, team, coordinator)?;
    }

    let agent_task_id = if let (Some(team), Some(coordinator)) = (&team, &coordinator) {
        let task_id = foco_agent::AgentTaskId::new(unique_id("agent-task"))
            .map_err(|error| ApiError::internal(error.to_string()))?;
        let input_json = serde_json::to_string(&CoordinatorTaskInput {
            queued_user_message_id: user_message_id.clone(),
            visible_assistant_message_id: Some(assistant_message_id.clone()),
            visible_assistant_sequence: Some(assistant_sequence),
            message: task_message,
            attachments: task_attachments,
            skill_ids: requested_skill_ids.clone(),
            collaboration_tools_enabled: team_mode_enabled,
            defer_until_workspace_idle: defer_start,
            delegated_input: None,
            correlation_id: None,
        })
        .map_err(|source| {
            ApiError::internal(format!("failed to serialize Coordinator task: {source}"))
        })?;
        database
            .enqueue_agent_task_with_limits(
                foco_store::workspace::NewAgentTask {
                    id: &task_id,
                    team_id: &team.id,
                    owner_instance_id: &coordinator.id,
                    origin_instance_id: None,
                    parent_task_id: None,
                    input_json: &input_json,
                },
                AGENT_MAX_QUEUED_TASKS_PER_TEAM,
                AGENT_MAX_QUEUED_TASKS_PER_INSTANCE,
                AGENT_MAX_QUEUED_TASKS_PER_CHAT,
            )
            .map_err(ApiError::from_workspace_error)?;
        insert_agent_event(
            &mut database,
            &team.id,
            "task_queued",
            Some(&coordinator.id),
            Some(&task_id),
            None,
            serde_json::json!({ "userMessageId": user_message_id }),
        )?;
        Some(task_id)
    } else {
        None
    };
    let agent_team_id = team.as_ref().map(|team| team.id.clone());

    database
        .insert_message(NewMessage {
            id: &user_message_id,
            chat_id: &chat_id,
            role: "user",
            content: message,
            sequence: prompt_context.next_message_sequence,
            metadata_json: Some(&user_metadata_json),
        })
        .map_err(ApiError::from_workspace_error)?;
    if agent_task_id.is_some() && !defer_start {
        state.agent_scheduler.wake()?;
    }
    let chat = database
        .chat(&chat_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("chat was not found: {chat_id}")))?;

    Ok(QueuedChatMessageArtifacts {
        chat_id,
        chat_title,
        created_at: chat.created_at,
        updated_at: chat.updated_at,
        user_message_id,
        assistant_message_id,
        content: message.to_string(),
        parts: user_message_response_parts(message, &prompt_context.attachments),
        agent_team_id,
        agent_task_id,
    })
}

fn configured_agent_definition(
    config: &GlobalConfig,
    id: &str,
) -> Result<foco_store::config::AgentDefinitionSettings, ApiError> {
    let id = foco_agent::AgentDefinitionId::new(id.to_string())
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    config
        .agent_definitions
        .iter()
        .find(|definition| definition.id == id)
        .cloned()
        .ok_or_else(|| ApiError::bad_request(format!("AgentDefinition '{id}' was not found")))
}

async fn default_agent_definition(
    state: &AppState,
    config: &GlobalConfig,
    prompt_context: &PreparedPromptContext,
) -> Result<foco_store::config::AgentDefinitionSettings, ApiError> {
    let allowed_tools = default_agent_allowed_tools(state, config, prompt_context).await?;
    let default_id = foco_agent::AgentDefinitionId::new(DEFAULT_AGENT_DEFINITION_ID)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let allowed_agent_definition_ids = config
        .agent_definitions
        .iter()
        .filter(|definition| definition.id != default_id)
        .map(|definition| definition.id.clone())
        .collect::<Vec<_>>();

    Ok(foco_store::config::AgentDefinitionSettings {
        id: default_id,
        revision: foco_store::config::AGENT_DEFINITION_INITIAL_REVISION,
        name: "Default agent".to_string(),
        description: "Default runtime agent for chat and Team mode.".to_string(),
        provider_id: prompt_context.provider_id.clone(),
        model_id: prompt_context.model_id.clone(),
        model_options: foco_store::config::AgentModelOptions {
            thinking_level: prompt_context.provider_request.thinking_level.clone(),
            max_output_tokens: None,
        },
        system_prompt: DEFAULT_AGENT_SYSTEM_PROMPT.to_string(),
        allowed_tools,
        max_instances: 1,
        allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
        permissions: foco_agent::AgentPermissions {
            can_create_instances: true,
            can_delegate: true,
            allowed_agent_definition_ids,
        },
    })
}

async fn default_agent_allowed_tools(
    state: &AppState,
    config: &GlobalConfig,
    prompt_context: &PreparedPromptContext,
) -> Result<Vec<String>, ApiError> {
    let ripgrep_available = {
        let status = state
            .ripgrep_status
            .lock()
            .map_err(|_| ApiError::internal("ripgrep status lock was poisoned"))?;
        status.available
    };
    let mut tools = builtin_tool_definitions_for_runtime(
        ripgrep_available,
        crate::runtime::web_search_enabled(&config.web_search),
    )
    .into_iter()
    .map(|definition| definition.name.to_string())
    .collect::<Vec<_>>();

    if config.memory.enabled {
        tools.extend(
            memory_tool_definitions()
                .into_iter()
                .map(|definition| definition.name),
        );
    }

    tools.extend(
        state
            .mcp_registry
            .tool_definitions(&prompt_context.workspace_id)
            .await
            .into_iter()
            .map(|definition| definition.name),
    );
    tools.sort();
    tools.dedup();
    Ok(tools)
}

fn resume_chat_coordinator_for_new_message(
    database: &mut WorkspaceDatabase,
    team: &foco_store::workspace::AgentTeamRecord,
    coordinator: &mut foco_store::workspace::AgentInstanceRecord,
) -> Result<(), ApiError> {
    if team.status != foco_agent::AgentTeamStatus::Active
        || coordinator.status != foco_agent::AgentInstanceStatus::Paused
    {
        return Ok(());
    }
    let has_active_coordinator_task = database
        .agent_tasks_for_team(&team.id)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .any(|task| {
            task.owner_instance_id == coordinator.id
                && matches!(
                    task.status,
                    foco_agent::AgentTaskStatus::Queued
                        | foco_agent::AgentTaskStatus::Running
                        | foco_agent::AgentTaskStatus::Waiting
                )
        });
    if has_active_coordinator_task {
        return Ok(());
    }

    database
        .transition_agent_instance_status(&coordinator.id, foco_agent::AgentInstanceStatus::Idle)
        .map_err(ApiError::from_workspace_error)?;
    coordinator.status = foco_agent::AgentInstanceStatus::Idle;
    insert_agent_event(
        database,
        &team.id,
        "instance_status_changed",
        Some(&coordinator.id),
        None,
        None,
        serde_json::json!({
            "status": foco_agent::AgentInstanceStatus::Idle,
            "reason": "chat_message_resume",
        }),
    )?;
    Ok(())
}

pub(crate) async fn stream_chat_response(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<ChatStreamRequest>,
) -> Result<BoxedChatSse, ApiError> {
    let config = config_snapshot(&state)?;
    if let (Some(chat_id), Some(user_message_id)) = (
        request.chat_id.as_deref(),
        request.queued_user_message_id.as_deref(),
    ) {
        let workspace = workspace_by_id(&config, &workspace_id)?;
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        if let Some(team) = database
            .agent_team_for_chat(chat_id)
            .map_err(ApiError::from_workspace_error)?
        {
            let task = database
                .agent_task_for_queued_user_message(&team.id, user_message_id)
                .map_err(ApiError::from_workspace_error)?
                .ok_or_else(|| {
                    ApiError::bad_request(format!(
                        "Coordinator task was not found for queued user message '{user_message_id}'"
                    ))
                })?;
            drop(database);
            state.agent_scheduler.wake()?;
            return team_chat_task_sse(&state, workspace, &task.id).await;
        }
    }
    let chat_context = prepare_chat_context(&state, &config, &workspace_id, request).await?;
    let run_id = chat_context.llm_request_id.clone();
    let (guidance_tx, guidance_rx) = mpsc::unbounded_channel();
    let active_run_registration = state.active_chat_runs.register(
        run_id.clone(),
        chat_context.workspace_id.clone(),
        chat_context.chat_id.clone(),
        chat_context.assistant_message_id.clone(),
        chat_context.assistant_sequence,
        chat_context.memories_used.clone(),
        chat_context.agent_primary_chat_output,
        0,
        guidance_tx,
    )?;
    let subscription = state
        .active_chat_runs
        .subscribe(&workspace_id, &run_id, Some(-1))?;

    tokio::spawn(run_chat_context_in_background(
        chat_context,
        active_run_registration,
        guidance_rx,
    ));

    Ok(boxed_chat_run_sse(subscription))
}

fn boxed_chat_run_sse(subscription: ActiveChatRunSubscription) -> BoxedChatSse {
    let stream: BoxedChatEventStream = Box::pin(chat_run_subscription_stream(subscription));
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    )
}

pub(crate) async fn team_chat_task_sse(
    state: &AppState,
    workspace: &WorkspaceConfig,
    task_id: &foco_agent::AgentTaskId,
) -> Result<BoxedChatSse, ApiError> {
    let stream = team_chat_task_event_stream(state.clone(), workspace.clone(), task_id.clone());
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    ))
}

fn team_chat_task_event_stream(
    state: AppState,
    workspace: WorkspaceConfig,
    task_id: foco_agent::AgentTaskId,
) -> BoxedChatEventStream {
    let stream = async_stream::stream! {
        // ponytail: polling avoids a new task-status broadcast; switch to notifications if queued streams become numerous.
        let mut last_agent_event_sequence: Option<i64> = None;
        loop {
            let mut streamed_active_run = false;
            if let Ok(subscription) =
                state
                    .active_chat_runs
                    .subscribe(&workspace.id, task_id.as_str(), Some(-1))
            {
                streamed_active_run = true;
                let mut subscription = subscription;
                let mut last_sequence = subscription.after_sequence;
                for event in subscription.replay {
                    if event.sequence > last_sequence {
                        last_sequence = event.sequence;
                    }
                    yield Ok(sse_event_payload(&event.payload_json));
                }

                if !*subscription.completed_rx.borrow() {
                    loop {
                        tokio::select! {
                            changed = subscription.completed_rx.changed() => {
                                if changed.is_err() || *subscription.completed_rx.borrow() {
                                    while let Ok(event) = subscription.event_rx.try_recv() {
                                        if event.sequence > last_sequence {
                                            last_sequence = event.sequence;
                                            yield Ok(sse_event_payload(&event.payload_json));
                                        }
                                    }
                                    break;
                                }
                            }
                            event = subscription.event_rx.recv() => {
                                match event {
                                    Ok(event) => {
                                        if event.sequence > last_sequence {
                                            last_sequence = event.sequence;
                                            yield Ok(sse_event_payload(&event.payload_json));
                                        }
                                    }
                                    Err(broadcast::error::RecvError::Lagged(_)) => {
                                        yield Ok(sse_event(&ChatSseEvent::Error {
                                            message: "chat run event subscriber lagged behind; refresh to replay the run".to_string(),
                                        }));
                                        return;
                                    }
                                    Err(broadcast::error::RecvError::Closed) => break,
                                }
                            }
                        }
                    }
                }
            }

            let database = match WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)
            {
                Ok(database) => database,
                Err(error) => {
                    yield Ok(sse_event(&ChatSseEvent::Error { message: error.message }));
                    yield Ok(sse_event(&ChatSseEvent::StreamEnd));
                    return;
                }
            };
            let task = match database
                .agent_task(&task_id)
                .map_err(ApiError::from_workspace_error)
            {
                Ok(Some(task)) => task,
                Ok(None) => {
                    yield Ok(sse_event(&ChatSseEvent::Error {
                        message: format!("Agent task '{task_id}' was not found"),
                    }));
                    yield Ok(sse_event(&ChatSseEvent::StreamEnd));
                    return;
                }
                Err(error) => {
                    yield Ok(sse_event(&ChatSseEvent::Error { message: error.message }));
                    yield Ok(sse_event(&ChatSseEvent::StreamEnd));
                    return;
                }
            };
            let team = match database
                .agent_team(&task.team_id)
                .map_err(ApiError::from_workspace_error)
            {
                Ok(Some(team)) => team,
                Ok(None) => {
                    yield Ok(sse_event(&ChatSseEvent::Error {
                        message: format!("Agent team '{}' was not found", task.team_id),
                    }));
                    yield Ok(sse_event(&ChatSseEvent::StreamEnd));
                    return;
                }
                Err(error) => {
                    yield Ok(sse_event(&ChatSseEvent::Error { message: error.message }));
                    yield Ok(sse_event(&ChatSseEvent::StreamEnd));
                    return;
                }
            };
            let new_agent_events = match database
                .agent_events_after(&task.team_id, last_agent_event_sequence.unwrap_or(-1))
                .map_err(ApiError::from_workspace_error)
            {
                Ok(events) => events,
                Err(error) => {
                    yield Ok(sse_event(&ChatSseEvent::Error { message: error.message }));
                    yield Ok(sse_event(&ChatSseEvent::StreamEnd));
                    return;
                }
            };
            if let Some(event) = new_agent_events.last() {
                let should_emit = last_agent_event_sequence.is_some() || streamed_active_run;
                last_agent_event_sequence = Some(event.sequence);
                if should_emit {
                    yield Ok(sse_event(&agent_team_refresh_event_from_agent_event(
                        &workspace.id,
                        &team.chat_id,
                        &task,
                        event,
                    )));
                }
            }
            if !agent_task_keeps_team_stream_open(task.status) {
                yield Ok(sse_event(&agent_team_refresh_event_for_task(
                    &workspace.id,
                    &team.chat_id,
                    &task,
                    "agent_task_settled",
                    false,
                )));
                if streamed_active_run {
                    yield Ok(sse_event(&ChatSseEvent::StreamEnd));
                    return;
                }
                let events = match database
                    .run_events_for_run(task_id.as_str())
                    .map_err(ApiError::from_workspace_error)
                {
                    Ok(events) => events,
                    Err(error) => {
                        yield Ok(sse_event(&ChatSseEvent::Error { message: error.message }));
                        yield Ok(sse_event(&ChatSseEvent::StreamEnd));
                        return;
                    }
                };
                for event in events {
                    yield Ok(Event::default().data(event.payload_json));
                }
                yield Ok(sse_event(&ChatSseEvent::StreamEnd));
                return;
            }
            tokio::time::sleep(TEAM_CHAT_TASK_STREAM_POLL_INTERVAL).await;
        }
    };
    Box::pin(stream)
}

fn agent_team_refresh_event_from_agent_event(
    workspace_id: &str,
    chat_id: &str,
    task: &foco_store::workspace::AgentTaskRecord,
    event: &foco_store::workspace::AgentEventRecord,
) -> ChatSseEvent {
    ChatSseEvent::AgentTeamRefresh {
        workspace_id: workspace_id.to_string(),
        chat_id: chat_id.to_string(),
        team_id: task.team_id.to_string(),
        instance_id: event.instance_id.as_ref().map(ToString::to_string),
        reason: event.event_type.clone(),
        reveal_panel: event.event_type == "instance_created",
    }
}

fn agent_team_refresh_event_for_task(
    workspace_id: &str,
    chat_id: &str,
    task: &foco_store::workspace::AgentTaskRecord,
    reason: &str,
    reveal_panel: bool,
) -> ChatSseEvent {
    ChatSseEvent::AgentTeamRefresh {
        workspace_id: workspace_id.to_string(),
        chat_id: chat_id.to_string(),
        team_id: task.team_id.to_string(),
        instance_id: Some(task.owner_instance_id.to_string()),
        reason: reason.to_string(),
        reveal_panel,
    }
}

fn agent_task_keeps_team_stream_open(status: foco_agent::AgentTaskStatus) -> bool {
    matches!(
        status,
        foco_agent::AgentTaskStatus::Queued
            | foco_agent::AgentTaskStatus::Running
            | foco_agent::AgentTaskStatus::Waiting
    )
}

pub(crate) async fn subscribe_chat_run(
    State(state): State<AppState>,
    AxumPath((workspace_id, run_id)): AxumPath<(String, String)>,
    Query(query): Query<ChatRunStreamQuery>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let subscription =
        state
            .active_chat_runs
            .subscribe(&workspace_id, &run_id, query.after_sequence)?;

    Ok(chat_run_sse(subscription))
}

pub(crate) async fn cancel_chat_run(
    State(state): State<AppState>,
    AxumPath((workspace_id, run_id)): AxumPath<(String, String)>,
) -> Result<Json<CancelChatRunResponse>, ApiError> {
    state.active_chat_runs.cancel(&workspace_id, &run_id)?;

    Ok(Json(CancelChatRunResponse { ok: true, run_id }))
}

pub(crate) fn chat_run_sse(
    subscription: ActiveChatRunSubscription,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    Sse::new(chat_run_subscription_stream(subscription)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    )
}

pub(crate) async fn add_chat_guidance(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<ChatGuidanceRequest>,
) -> Result<Json<ChatGuidanceResponse>, ApiError> {
    let guidance = state
        .active_chat_runs
        .push_guidance(&workspace_id, request)?;

    Ok(Json(ChatGuidanceResponse {
        id: guidance.id,
        content: guidance.content,
        parts: user_guidance_message_parts(&guidance.attachments),
    }))
}

pub(crate) async fn context_usage(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<ContextUsageRequest>,
) -> Result<Json<ContextUsageResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let latest_response_usage = request.latest_response_usage.clone();
    let prompt_context = prepare_prompt_context(
        &state,
        &config,
        &workspace_id,
        request.into_prompt_request(),
        None,
        PromptAssemblyPurpose::ContextPreview,
    )
    .await?;

    Ok(Json(context_usage_response(
        &prompt_context,
        &latest_response_usage,
    )?))
}

pub(crate) async fn answer_question(
    State(state): State<AppState>,
    AxumPath(question_id): AxumPath<String>,
    Json(answer): Json<QuestionAnswer>,
) -> Result<Json<QuestionAnswerResponse>, ApiError> {
    let question_id = question_id.trim().to_string();

    state.question_registry.answer(&question_id, answer)?;

    Ok(Json(QuestionAnswerResponse {
        ok: true,
        question_id,
    }))
}

pub(crate) async fn ai_statistics(
    State(state): State<AppState>,
    Query(query): Query<AiStatisticsQuery>,
) -> Result<Json<AiStatisticsResponse>, ApiError> {
    let started_at = Instant::now();
    tracing::info!("AI statistics request started");
    let config_started_at = Instant::now();
    let config = config_snapshot(&state)?;
    tracing::info!(
        elapsed_ms = config_started_at.elapsed().as_millis() as u64,
        workspace_count = config.workspaces.len(),
        "AI statistics config snapshot loaded"
    );
    let filters = normalized_ai_statistics_query(query)?;

    let response = tokio::task::spawn_blocking(move || {
        load_ai_statistics_response(config, filters, started_at)
    })
    .await
    .map_err(|error| ApiError::internal(format!("AI statistics worker failed: {error}")))??;

    Ok(Json(response))
}

fn load_ai_statistics_response(
    config: GlobalConfig,
    filters: NormalizedAiStatisticsFilters,
    started_at: Instant,
) -> Result<AiStatisticsResponse, ApiError> {
    let workspace_filter = filters.workspace_id.as_deref().unwrap_or("<all>");
    let workspaces = ai_statistics_workspaces(&config, filters.workspace_id.as_deref())?;
    tracing::info!(
        workspace_filter,
        workspace_count = workspaces.len(),
        chat_filter = filters.chat_id.as_deref().unwrap_or("<none>"),
        provider_filter = filters.provider_id.as_deref().unwrap_or("<none>"),
        model_filter = filters.model_id.as_deref().unwrap_or("<none>"),
        status_filter = filters.status.as_deref().unwrap_or("<none>"),
        page = filters.page,
        page_size = filters.page_size,
        offset = filters.offset,
        "AI statistics filters normalized"
    );
    let mut requests = Vec::new();
    let mut merged_summary: Option<LlmRequestAuditSummaryRow> = None;
    let mut merged_trend: BTreeMap<String, LlmRequestAuditTrendPoint> = BTreeMap::new();
    let mut merged_models: BTreeMap<String, LlmRequestAuditModelBreakdown> = BTreeMap::new();
    let mut merged_providers: BTreeMap<String, LlmRequestAuditProviderBreakdown> = BTreeMap::new();
    let mut total_count = 0_i64;
    let page_limit = filters
        .offset
        .checked_add(filters.page_size)
        .ok_or_else(|| ApiError::bad_request("AI statistics page limit is too large"))?;

    for workspace in workspaces {
        let workspace_started_at = Instant::now();
        tracing::info!(
            workspace_id = %workspace.id,
            workspace_path = %workspace.path.display(),
            "AI statistics workspace scan started"
        );
        let database_started_at = Instant::now();
        tracing::info!(
            workspace_id = %workspace.id,
            workspace_path = %workspace.path.display(),
            "AI statistics workspace database open started"
        );
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        tracing::info!(
            workspace_id = %workspace.id,
            database_path = %database.database_path().display(),
            elapsed_ms = database_started_at.elapsed().as_millis() as u64,
            "AI statistics workspace database opened"
        );
        let audit_filters = LlmRequestAuditFilters {
            workspace_id: None,
            chat_id: filters.chat_id.as_deref(),
            provider_id: filters.provider_id.as_deref(),
            model_id: filters.model_id.as_deref(),
            final_state: filters.status.as_deref(),
            started_after: filters.started_after.as_deref(),
            started_before: filters.started_before.as_deref(),
            limit: Some(page_limit),
            offset: Some(0),
        };

        let summary_started_at = Instant::now();
        tracing::info!(
            workspace_id = %workspace.id,
            "AI statistics summary query started"
        );
        let workspace_summary = database
            .llm_request_audit_summary(audit_filters)
            .map_err(ApiError::from_workspace_error)?;
        tracing::info!(
            workspace_id = %workspace.id,
            request_count = workspace_summary.total_requests,
            elapsed_ms = summary_started_at.elapsed().as_millis() as u64,
            "AI statistics summary query completed"
        );
        let workspace_request_count = workspace_summary.total_requests;
        total_count += workspace_request_count;
        if workspace_request_count > 0 {
            merge_llm_request_audit_summary(&mut merged_summary, &workspace_summary);
            let trend_started_at = Instant::now();
            tracing::info!(
                workspace_id = %workspace.id,
                "AI statistics trend query started"
            );
            let trend = database
                .llm_request_audit_trend_breakdown(audit_filters)
                .map_err(ApiError::from_workspace_error)?;
            tracing::info!(
                workspace_id = %workspace.id,
                trend_points = trend.len(),
                elapsed_ms = trend_started_at.elapsed().as_millis() as u64,
                "AI statistics trend query completed"
            );
            for point in trend {
                merged_trend
                    .entry(point.bucket.clone())
                    .and_modify(|entry| {
                        entry.request_count += point.request_count;
                        entry.total_tokens += point.total_tokens;
                    })
                    .or_insert(point);
            }
            let model_started_at = Instant::now();
            tracing::info!(
                workspace_id = %workspace.id,
                "AI statistics model breakdown query started"
            );
            let model_rows = database
                .llm_request_audit_model_breakdown(audit_filters)
                .map_err(ApiError::from_workspace_error)?;
            tracing::info!(
                workspace_id = %workspace.id,
                model_count = model_rows.len(),
                elapsed_ms = model_started_at.elapsed().as_millis() as u64,
                "AI statistics model breakdown query completed"
            );
            for row in model_rows {
                merged_models
                    .entry(row.model_id.clone())
                    .and_modify(|entry| {
                        entry.request_count += row.request_count;
                        entry.total_tokens += row.total_tokens;
                    })
                    .or_insert(row);
            }
            let provider_started_at = Instant::now();
            tracing::info!(
                workspace_id = %workspace.id,
                "AI statistics provider breakdown query started"
            );
            let provider_rows = database
                .llm_request_audit_provider_breakdown(audit_filters)
                .map_err(ApiError::from_workspace_error)?;
            tracing::info!(
                workspace_id = %workspace.id,
                provider_count = provider_rows.len(),
                elapsed_ms = provider_started_at.elapsed().as_millis() as u64,
                "AI statistics provider breakdown query completed"
            );
            for row in provider_rows {
                merged_providers
                    .entry(row.provider_id.clone())
                    .and_modify(|entry| {
                        entry.request_count += row.request_count;
                        entry.success_count += row.success_count;
                        entry.total_tokens += row.total_tokens;
                        entry.latency_count += row.latency_count;
                        entry.latency_sum += row.latency_sum;
                    })
                    .or_insert(row);
            }
            let rows_started_at = Instant::now();
            tracing::info!(
                workspace_id = %workspace.id,
                limit = page_limit,
                "AI statistics rows query started"
            );
            let rows = database
                .llm_request_audit_rows(audit_filters)
                .map_err(ApiError::from_workspace_error)?;
            tracing::info!(
                workspace_id = %workspace.id,
                row_count = rows.len(),
                elapsed_ms = rows_started_at.elapsed().as_millis() as u64,
                "AI statistics rows query completed"
            );
            let chat_titles_started_at = Instant::now();
            tracing::info!(
                workspace_id = %workspace.id,
                "AI statistics chat title query started"
            );
            let chat_titles = chat_title_map_for_audit_rows(&database, &rows)?;
            tracing::info!(
                workspace_id = %workspace.id,
                chat_count = chat_titles.len(),
                elapsed_ms = chat_titles_started_at.elapsed().as_millis() as u64,
                "AI statistics chat title query completed"
            );

            requests.extend(
                rows.into_iter()
                    .map(|row| ai_request_audit_summary(row, workspace, &chat_titles)),
            );
        }
        tracing::info!(
            workspace_id = %workspace.id,
            elapsed_ms = workspace_started_at.elapsed().as_millis() as u64,
            "AI statistics workspace scan completed"
        );
    }

    let sort_started_at = Instant::now();
    requests.sort_by(|left, right| {
        right
            .request_started_at
            .cmp(&left.request_started_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    tracing::info!(
        request_count = requests.len(),
        elapsed_ms = sort_started_at.elapsed().as_millis() as u64,
        "AI statistics request rows sorted"
    );
    let start = usize::try_from(filters.offset).expect("non-negative offset fits usize");
    let page_size = usize::try_from(filters.page_size).expect("positive page size fits usize");
    let requests: Vec<_> = requests.into_iter().skip(start).take(page_size).collect();
    let total_pages = if total_count == 0 {
        0
    } else {
        (total_count + filters.page_size - 1) / filters.page_size
    };

    let summary_started_at = Instant::now();
    let summary = ai_statistics_summary_from_aggregates(
        merged_summary,
        merged_trend,
        merged_models,
        merged_providers,
    );
    tracing::info!(
        elapsed_ms = summary_started_at.elapsed().as_millis() as u64,
        total_count,
        total_pages,
        "AI statistics response summary built"
    );
    tracing::info!(
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        total_count,
        returned_count = requests.len(),
        "AI statistics request completed"
    );

    Ok(AiStatisticsResponse {
        page: filters.page,
        page_size: filters.page_size,
        requests,
        summary,
        total_count,
        total_pages,
    })
}

pub(crate) async fn ai_statistics_detail(
    State(state): State<AppState>,
    AxumPath((workspace_id, request_id)): AxumPath<(String, String)>,
) -> Result<Json<AiRequestDetailResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let request_id = request_id.trim();

    if request_id.is_empty() {
        return Err(ApiError::bad_request("request id must not be empty"));
    }

    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let request = database
        .llm_request(request_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("LLM request was not found: {request_id}")))?;
    let chat_titles = chat_title_map_for_chat_id(&database, request.chat_id.as_deref())?;
    let events = database
        .llm_request_events(request_id)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(ai_request_audit_event_summary)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(AiRequestDetailResponse {
        request: ai_request_audit_detail(request, workspace, &chat_titles)?,
        events,
    }))
}

pub(crate) async fn chat_messages(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
    Query(query): Query<ChatMessagesQuery>,
) -> Result<Json<ChatMessagesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = workspace_id.trim();
    let chat_id = chat_id.trim();
    let workspace = config
        .workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| ApiError::bad_request(format!("workspace was not found: {workspace_id}")))?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;

    let chat = database
        .chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("chat was not found: {chat_id}")))?;
    let chat_summary = chat_messages_chat_summary(&chat)?;
    let include_memory_dream_transcript_steps =
        chat_summary.kind.as_deref() == Some(MEMORY_DREAM_TRANSCRIPT_CHAT_KIND);

    let (message_records, pagination) = chat_message_records_for_query(&database, chat_id, &query)?;
    let messages = chat_message_summaries_for_chat(
        &mut database,
        &workspace.path,
        Some(&state.memory_database_file),
        chat_id,
        message_records,
        include_memory_dream_transcript_steps,
    )?;

    let active_run = state
        .active_chat_runs
        .active_run_for_chat(workspace_id, chat_id)?;

    Ok(Json(ChatMessagesResponse {
        chat: Some(chat_summary),
        messages,
        pagination,
        active_run,
    }))
}

fn chat_message_records_for_query(
    database: &WorkspaceDatabase,
    chat_id: &str,
    query: &ChatMessagesQuery,
) -> Result<(Vec<MessageRecord>, ChatMessagesPaginationSummary), ApiError> {
    if query.before_sequence.is_some_and(|sequence| sequence < 0) {
        return Err(ApiError::bad_request(
            "beforeSequence must be greater than or equal to 0",
        ));
    }
    let Some(limit) = query.limit else {
        let messages = database
            .messages_for_chat(chat_id)
            .map_err(ApiError::from_workspace_error)?;
        return Ok((
            messages,
            ChatMessagesPaginationSummary {
                has_more_before: false,
                next_before_sequence: None,
            },
        ));
    };
    if limit == 0 || limit > MAX_CHAT_MESSAGES_PAGE_LIMIT {
        return Err(ApiError::bad_request(format!(
            "limit must be between 1 and {MAX_CHAT_MESSAGES_PAGE_LIMIT}"
        )));
    }
    let fetch_limit = limit.checked_add(1).ok_or_else(|| {
        ApiError::bad_request("limit is too large to calculate a chat message page")
    })?;
    let mut messages = database
        .messages_for_chat_page(chat_id, query.before_sequence, fetch_limit)
        .map_err(ApiError::from_workspace_error)?;
    let has_more_before = messages.len() > limit;
    if has_more_before {
        messages.remove(0);
    }
    let next_before_sequence = has_more_before
        .then(|| messages.first().map(|message| message.sequence))
        .flatten();

    Ok((
        messages,
        ChatMessagesPaginationSummary {
            has_more_before,
            next_before_sequence,
        },
    ))
}

pub(crate) async fn chat_todo_graph(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
    Query(query): Query<TodoGraphQuery>,
) -> Result<Json<TodoGraphResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = workspace_id.trim();
    let chat_id = chat_id.trim();
    let workspace = workspace_by_id(&config, workspace_id)?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;

    if database
        .chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
        .is_none()
    {
        return Err(ApiError::bad_request(format!(
            "chat was not found: {chat_id}"
        )));
    }

    let status = optional_trimmed_string(query.status);
    let task_id = optional_trimmed_string(query.task_id);
    let graph = database
        .filtered_todo_graph(
            chat_id,
            TodoGraphFilter {
                status: status.as_deref(),
                task_id: task_id.as_deref(),
                include_subtasks: query.include_subtasks.unwrap_or(true),
            },
        )
        .map_err(ApiError::from_workspace_error)?;

    Ok(Json(todo_graph_response(chat_id, graph)))
}

pub(crate) async fn chat_statistics(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
) -> Result<Json<ChatStatisticsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = workspace_id.trim();
    let chat_id = chat_id.trim();
    let workspace = workspace_by_id(&config, workspace_id)?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;

    if database
        .chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
        .is_none()
    {
        return Err(ApiError::bad_request(format!(
            "chat was not found: {chat_id}"
        )));
    }

    let message_counts = database
        .message_role_counts_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let llm_rows = database
        .llm_request_audit_count(LlmRequestAuditFilters {
            chat_id: Some(chat_id),
            ..LlmRequestAuditFilters::default()
        })
        .and_then(|request_count| {
            database.llm_request_audit_rows(LlmRequestAuditFilters {
                chat_id: Some(chat_id),
                limit: Some(request_count),
                offset: Some(0),
                ..LlmRequestAuditFilters::default()
            })
        })
        .map_err(ApiError::from_workspace_error)?;
    let prompt_injections = database
        .prompt_context_injections_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let compression_snapshots = database
        .context_compression_snapshots_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let code_change_stats = database
        .code_change_stats_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let tool_breakdown = database
        .tool_call_counts_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(chat_tool_breakdown)
        .collect();
    let created_workspace_memories =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace.path))
            .map_err(ApiError::from_memory_error)?
            .facts_created_from_chat_sources(chat_id)
            .map_err(ApiError::from_memory_error)?
            .len() as i64;
    let run_ids = llm_rows
        .iter()
        .map(|row| row.id.clone())
        .collect::<HashSet<_>>();
    let created_global_memories =
        MemoryDatabase::open_or_create_global_at(&state.memory_database_file)
            .map_err(ApiError::from_memory_error)?
            .facts_created_from_source_run_ids(&run_ids)
            .map_err(ApiError::from_memory_error)?
            .len() as i64;

    Ok(Json(chat_statistics_response(
        workspace_id,
        chat_id,
        chat_message_role_counts(message_counts),
        llm_rows,
        prompt_injections,
        compression_snapshots,
        code_change_stats,
        tool_breakdown,
        created_workspace_memories + created_global_memories,
    )?))
}

pub(crate) async fn delete_chat(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = workspace_id.trim();
    let chat_id = chat_id.trim();
    let workspace = workspace_by_id(&config, workspace_id)?;

    if chat_id.is_empty() {
        return Err(ApiError::bad_request("chat id must not be empty"));
    }

    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;

    if !database
        .delete_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
    {
        return Err(ApiError::bad_request(format!(
            "chat was not found: {chat_id}"
        )));
    }

    workspace_response_from_config(&config, &state.active_chat_runs)
}
