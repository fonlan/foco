use std::{
    collections::HashMap,
    convert::Infallible,
    path::Path,
    sync::{Arc, Mutex},
};

use axum::response::sse::Event;
use foco_agent::{
    AgentAttemptId, AgentInstanceId, AgentMessageId, AgentRunCancellation, AgentTaskId, AgentTeamId,
};
use foco_providers::NeutralChatAttachment;
use foco_store::workspace::{
    CodeChangeStats, NewMessage, NewRunEvent, NewToolCall, NewToolResult, WorkspaceDatabase,
};
use foco_tools::ToolCancellationToken;
use serde::Serialize;
use tokio::sync::{broadcast, mpsc, watch};

use crate::http::chat::ChatGuidanceRequest;
use crate::*;

#[derive(Clone, Default)]
pub(crate) struct ActiveChatRunRegistry {
    runs: Arc<Mutex<HashMap<String, ActiveChatRun>>>,
}

#[derive(Clone, Debug)]
pub(crate) struct ActiveAgentRunIdentity {
    pub(crate) team_id: AgentTeamId,
    pub(crate) instance_id: AgentInstanceId,
    pub(crate) task_id: AgentTaskId,
    pub(crate) _attempt_id: AgentAttemptId,
}

#[derive(Clone)]
struct ActiveChatRun {
    workspace_id: String,
    chat_id: String,
    agent_identity: Option<ActiveAgentRunIdentity>,
    primary_chat_output: bool,
    guidance_tx: mpsc::UnboundedSender<GuidanceMessage>,
    accepting_guidance: bool,
    cancellation: ChatRunCancellation,
    events: Arc<Mutex<Vec<ChatRunEventFrame>>>,
    event_tx: broadcast::Sender<ChatRunEventFrame>,
    pub(crate) completed_rx: watch::Receiver<bool>,
}

#[derive(Clone, Debug)]
pub(crate) struct ChatRunEventFrame {
    pub(crate) sequence: i64,
    pub(crate) event_type: String,
    pub(crate) payload_json: String,
}

#[derive(Clone, Debug, Default)]
struct StreamingAssistantDraft {
    pub(crate) content: String,
    reasoning: String,
    error_message: Option<String>,
    status: StreamingAssistantStatus,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StreamingAssistantStatus {
    #[default]
    Pending,
    Streaming,
    Failed,
    Cancelled,
}

impl StreamingAssistantStatus {
    fn as_metadata_value(self) -> Option<&'static str> {
        match self {
            Self::Pending => None,
            Self::Streaming => Some("streaming"),
            Self::Failed => Some("failed"),
            Self::Cancelled => Some("cancelled"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ChatRunCancellation {
    tx: watch::Sender<bool>,
    tool_token: ToolCancellationToken,
    agent_token: AgentRunCancellation,
}

impl ChatRunCancellation {
    pub(crate) fn new() -> Self {
        let (tx, _rx) = watch::channel(false);
        Self {
            tx,
            tool_token: ToolCancellationToken::default(),
            agent_token: AgentRunCancellation::default(),
        }
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<bool> {
        self.tx.subscribe()
    }

    pub(crate) fn tool_token(&self) -> ToolCancellationToken {
        self.tool_token.clone()
    }

    pub(crate) fn agent_token(&self) -> AgentRunCancellation {
        self.agent_token.clone()
    }

    pub(crate) fn cancel(&self) {
        self.tool_token.cancel();
        self.agent_token.cancel();
        self.tx.send_replace(true);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GuidanceMessage {
    pub(crate) id: String,
    pub(crate) content: String,
    pub(crate) attachments: Vec<NeutralChatAttachment>,
    /// `manualGuidance`, `agentMessage`, or `reasoningLoopGuard`.
    pub(crate) source: String,
    pub(crate) interrupted_assistant_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentMessageGuidanceDelivery {
    Guidance,
    Queued,
}

impl ActiveChatRunRegistry {
    #[cfg(any(test, all(any(windows, target_os = "macos"), not(debug_assertions))))]
    pub(crate) fn active_run_count(&self) -> Result<usize, ApiError> {
        let runs = self
            .runs
            .lock()
            .map_err(|_| ApiError::internal("active chat run registry lock is poisoned"))?;

        Ok(runs
            .values()
            .filter(|run| !*run.completed_rx.borrow())
            .count())
    }

    /// Registers an ordinary chat run without Agent task identity.
    ///
    /// This intentionally preserves the long-standing internal API used by chat handling and
    /// focused runtime tests. Agent task runs must use [`Self::register_agent`] so that only
    /// scheduler-owned runs can receive an Agent message as live guidance.
    pub(crate) fn register(
        &self,
        run_id: String,
        workspace_id: String,
        chat_id: String,
        assistant_message_id: String,
        assistant_sequence: i64,
        memories_used: Vec<ChatMemoryUsedSummary>,
        primary_chat_output: bool,
        next_sequence: i64,
        guidance_tx: mpsc::UnboundedSender<GuidanceMessage>,
    ) -> Result<ActiveChatRunRegistration, ApiError> {
        self.register_with_agent_identity(
            run_id,
            workspace_id,
            chat_id,
            assistant_message_id,
            assistant_sequence,
            memories_used,
            primary_chat_output,
            None,
            next_sequence,
            guidance_tx,
        )
    }

    /// Registers a scheduler-owned Agent task run that may accept live Agent-message guidance.
    pub(crate) fn register_agent(
        &self,
        run_id: String,
        workspace_id: String,
        chat_id: String,
        assistant_message_id: String,
        assistant_sequence: i64,
        memories_used: Vec<ChatMemoryUsedSummary>,
        primary_chat_output: bool,
        agent_identity: ActiveAgentRunIdentity,
        next_sequence: i64,
        guidance_tx: mpsc::UnboundedSender<GuidanceMessage>,
    ) -> Result<ActiveChatRunRegistration, ApiError> {
        self.register_with_agent_identity(
            run_id,
            workspace_id,
            chat_id,
            assistant_message_id,
            assistant_sequence,
            memories_used,
            primary_chat_output,
            Some(agent_identity),
            next_sequence,
            guidance_tx,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the registration boundary keeps run state explicit and preserves the existing chat API"
    )]
    fn register_with_agent_identity(
        &self,
        run_id: String,
        workspace_id: String,
        chat_id: String,
        assistant_message_id: String,
        assistant_sequence: i64,
        memories_used: Vec<ChatMemoryUsedSummary>,
        primary_chat_output: bool,
        agent_identity: Option<ActiveAgentRunIdentity>,
        next_sequence: i64,
        guidance_tx: mpsc::UnboundedSender<GuidanceMessage>,
    ) -> Result<ActiveChatRunRegistration, ApiError> {
        if next_sequence < 0 {
            return Err(ApiError::internal(
                "active chat run sequence must be non-negative",
            ));
        }
        let mut runs = self
            .runs
            .lock()
            .map_err(|_| ApiError::internal("active chat run registry lock is poisoned"))?;

        if runs.contains_key(&run_id) {
            return Err(ApiError::internal(format!(
                "duplicate active chat run id: {run_id}"
            )));
        }

        let cancellation = ChatRunCancellation::new();
        let (event_tx, _event_rx) = broadcast::channel(512);
        let (completed_tx, completed_rx) = watch::channel(false);
        let events = Arc::new(Mutex::new(Vec::new()));
        runs.insert(
            run_id.clone(),
            ActiveChatRun {
                workspace_id,
                chat_id,
                agent_identity,
                primary_chat_output,
                guidance_tx,
                accepting_guidance: true,
                cancellation: cancellation.clone(),
                events: events.clone(),
                event_tx: event_tx.clone(),
                completed_rx,
            },
        );

        Ok(ActiveChatRunRegistration {
            registry: self.clone(),
            run_id,
            assistant_message_id,
            assistant_sequence,
            memories_used,
            primary_chat_output,
            cancellation,
            events,
            event_tx,
            completed_tx,
            next_sequence,
            assistant_draft: StreamingAssistantDraft::default(),
            completed: false,
        })
    }

    fn unregister(&self, run_id: &str) {
        if let Ok(mut runs) = self.runs.lock() {
            runs.remove(run_id);
        }
    }

    fn stop_accepting_guidance(&self, run_id: &str) {
        if let Ok(mut runs) = self.runs.lock() {
            if let Some(run) = runs.get_mut(run_id) {
                run.accepting_guidance = false;
            }
        }
    }

    pub(crate) fn active_run_for_chat(
        &self,
        workspace_id: &str,
        chat_id: &str,
    ) -> Result<Option<ActiveChatRunSummary>, ApiError> {
        let runs = self
            .runs
            .lock()
            .map_err(|_| ApiError::internal("active chat run registry lock is poisoned"))?;
        let mut matches = runs
            .iter()
            .filter(|(_, run)| {
                run.workspace_id == workspace_id
                    && run.chat_id == chat_id
                    && run.primary_chat_output
                    && !*run.completed_rx.borrow()
            })
            .collect::<Vec<_>>();
        matches.sort_by_key(|(_, run)| !run.accepting_guidance);
        let Some((run_id, run)) = matches.into_iter().next() else {
            return Ok(None);
        };

        let last_sequence = run
            .events
            .lock()
            .map_err(|_| ApiError::internal("active chat run event cache lock is poisoned"))?
            .last()
            .map(|event| event.sequence);

        Ok(Some(ActiveChatRunSummary {
            run_id: run_id.clone(),
            workspace_id: run.workspace_id.clone(),
            chat_id: run.chat_id.clone(),
            last_sequence,
            accepting_guidance: run.accepting_guidance,
        }))
    }

    pub(crate) fn subscribe(
        &self,
        workspace_id: &str,
        run_id: &str,
        after_sequence: Option<i64>,
    ) -> Result<ActiveChatRunSubscription, ApiError> {
        let active_run = {
            let runs = self
                .runs
                .lock()
                .map_err(|_| ApiError::internal("active chat run registry lock is poisoned"))?;
            runs.get(run_id).cloned().ok_or_else(|| {
                ApiError::bad_request(format!("active chat run was not found: {run_id}"))
            })?
        };

        if active_run.workspace_id != workspace_id {
            return Err(ApiError::bad_request(format!(
                "active chat run {run_id} belongs to workspace {}, not {workspace_id}",
                active_run.workspace_id
            )));
        }

        let after_sequence = after_sequence.unwrap_or(-1);
        let replay = active_run
            .events
            .lock()
            .map_err(|_| ApiError::internal("active chat run event cache lock is poisoned"))?
            .iter()
            .filter(|event| event.sequence > after_sequence)
            .cloned()
            .collect::<Vec<_>>();

        Ok(ActiveChatRunSubscription {
            replay,
            event_rx: active_run.event_tx.subscribe(),
            completed_rx: active_run.completed_rx.clone(),
            after_sequence,
        })
    }

    pub(crate) fn cancel(&self, workspace_id: &str, run_id: &str) -> Result<(), ApiError> {
        let active_run = {
            let runs = self
                .runs
                .lock()
                .map_err(|_| ApiError::internal("active chat run registry lock is poisoned"))?;
            runs.get(run_id).cloned().ok_or_else(|| {
                ApiError::bad_request(format!("active chat run was not found: {run_id}"))
            })?
        };

        if active_run.workspace_id != workspace_id {
            return Err(ApiError::bad_request(format!(
                "active chat run {run_id} belongs to workspace {}, not {workspace_id}",
                active_run.workspace_id
            )));
        }

        active_run.cancellation.cancel();
        Ok(())
    }

    pub(crate) fn push_guidance(
        &self,
        workspace_id: &str,
        request: ChatGuidanceRequest,
    ) -> Result<GuidanceMessage, ApiError> {
        let workspace_id = normalized_required_text("workspaceId", workspace_id)?;
        let chat_id = normalized_required_text("chatId", &request.chat_id)?;
        let run_id = normalized_required_text("runId", &request.run_id)?;
        let content = normalized_chat_message(&request.message)?;
        let attachments = normalized_chat_attachments(request.attachments)?;
        let guidance = GuidanceMessage {
            id: unique_id("msg-guidance"),
            content,
            attachments,
            source: crate::runtime::MANUAL_GUIDANCE_SOURCE.to_string(),
            interrupted_assistant_id: None,
        };
        let active_run = {
            let runs = self
                .runs
                .lock()
                .map_err(|_| ApiError::internal("active chat run registry lock is poisoned"))?;
            runs.get(&run_id).cloned().ok_or_else(|| {
                ApiError::bad_request(format!("active chat run was not found: {run_id}"))
            })?
        };

        if active_run.workspace_id != workspace_id {
            return Err(ApiError::bad_request(format!(
                "active chat run {run_id} belongs to workspace {}, not {workspace_id}",
                active_run.workspace_id
            )));
        }
        if active_run.chat_id != chat_id {
            return Err(ApiError::bad_request(format!(
                "active chat run {run_id} belongs to chat {}, not {chat_id}",
                active_run.chat_id
            )));
        }
        if !active_run.accepting_guidance {
            return Err(ApiError::bad_request(format!(
                "active chat run is no longer accepting guidance: {run_id}"
            )));
        }

        active_run.guidance_tx.send(guidance.clone()).map_err(|_| {
            ApiError::bad_request(format!(
                "active chat run is no longer accepting guidance: {run_id}"
            ))
        })?;

        Ok(guidance)
    }

    /// Delivers a persisted Agent message to exactly one matching active Agent run.
    ///
    /// This deliberately treats every routing uncertainty as queued delivery: the durable
    /// message remains unread and is injected when the receiver starts its next attempt.
    pub(crate) fn deliver_agent_message_guidance(
        &self,
        workspace_id: &str,
        team_id: &AgentTeamId,
        receiver_instance_id: &AgentInstanceId,
        related_task_id: Option<&AgentTaskId>,
        guidance: GuidanceMessage,
    ) -> AgentMessageGuidanceDelivery {
        let matching_runs = match self.runs.lock() {
            Ok(runs) => runs
                .values()
                .filter(|run| {
                    let Some(identity) = run.agent_identity.as_ref() else {
                        return false;
                    };
                    run.workspace_id == workspace_id
                        && run.accepting_guidance
                        && !*run.completed_rx.borrow()
                        && identity.team_id == *team_id
                        && identity.instance_id == *receiver_instance_id
                        && related_task_id
                            .map(|task_id| identity.task_id == *task_id)
                            .unwrap_or(true)
                })
                .map(|run| run.guidance_tx.clone())
                .collect::<Vec<_>>(),
            Err(_) => return AgentMessageGuidanceDelivery::Queued,
        };

        let [guidance_tx] = matching_runs.as_slice() else {
            return AgentMessageGuidanceDelivery::Queued;
        };

        if guidance_tx.send(guidance).is_ok() {
            AgentMessageGuidanceDelivery::Guidance
        } else {
            AgentMessageGuidanceDelivery::Queued
        }
    }
}

pub(crate) struct ActiveChatRunRegistration {
    registry: ActiveChatRunRegistry,
    pub(crate) run_id: String,
    assistant_message_id: String,
    assistant_sequence: i64,
    memories_used: Vec<ChatMemoryUsedSummary>,
    primary_chat_output: bool,
    cancellation: ChatRunCancellation,
    events: Arc<Mutex<Vec<ChatRunEventFrame>>>,
    event_tx: broadcast::Sender<ChatRunEventFrame>,
    completed_tx: watch::Sender<bool>,
    next_sequence: i64,
    assistant_draft: StreamingAssistantDraft,
    completed: bool,
}

impl ActiveChatRunRegistration {
    pub(crate) fn cancellation(&self) -> &ChatRunCancellation {
        &self.cancellation
    }

    pub(crate) fn record_event(
        &mut self,
        workspace_path: &Path,
        chat_id: &str,
        event: &ChatSseEvent,
    ) -> Result<(), ApiError> {
        let captured = captured_event(event);
        let payload_json = captured.normalized_event_json;
        let event_frame = ChatRunEventFrame {
            sequence: self.next_sequence,
            event_type: captured.event_type,
            payload_json,
        };

        let persisted = {
            let mut database = open_workspace_database(workspace_path)?;
            let run_event_id = format!("{}-event-{}", self.run_id, event_frame.sequence);
            let persisted = match event {
                ChatSseEvent::GuidanceApplied { id, source, .. }
                    if source == crate::runtime::AGENT_MESSAGE_GUIDANCE_SOURCE =>
                {
                    let message_id = AgentMessageId::new(id.clone()).map_err(|source| {
                        ApiError::internal(format!("invalid Agent message guidance id: {source}"))
                    })?;
                    database
                        .insert_agent_message_guidance_run_event_and_consume(
                            NewRunEvent {
                                id: &run_event_id,
                                chat_id,
                                run_id: &self.run_id,
                                sequence: event_frame.sequence,
                                event_type: &event_frame.event_type,
                                payload_json: &event_frame.payload_json,
                            },
                            &message_id,
                            crate::runtime::AGENT_MESSAGE_GUIDANCE_SOURCE,
                        )
                        .map_err(ApiError::from_workspace_error)?
                }
                _ => {
                    database
                        .insert_run_event(NewRunEvent {
                            id: &run_event_id,
                            chat_id,
                            run_id: &self.run_id,
                            sequence: event_frame.sequence,
                            event_type: &event_frame.event_type,
                            payload_json: &event_frame.payload_json,
                        })
                        .map_err(ApiError::from_workspace_error)?;
                    true
                }
            };
            if persisted && self.primary_chat_output {
                self.persist_assistant_draft_for_event(&mut database, chat_id, event)?;
                self.persist_tool_state_for_event(&mut database, chat_id, event)?;
                if matches!(event, ChatSseEvent::ToolCall { .. }) {
                    self.persist_assistant_draft(&mut database, chat_id)?;
                }
            }
            persisted
        };
        if !persisted {
            return Ok(());
        }
        self.next_sequence += 1;

        self.events
            .lock()
            .map_err(|_| ApiError::internal("active chat run event cache lock is poisoned"))?
            .push(event_frame.clone());
        let _ = self.event_tx.send(event_frame);

        if matches!(
            event,
            ChatSseEvent::Complete { .. } | ChatSseEvent::Error { .. }
        ) {
            self.registry.stop_accepting_guidance(&self.run_id);
        }

        Ok(())
    }

    fn persist_assistant_draft_for_event(
        &mut self,
        database: &mut WorkspaceDatabase,
        chat_id: &str,
        event: &ChatSseEvent,
    ) -> Result<(), ApiError> {
        match event {
            ChatSseEvent::TextDelta {
                assistant_message_id,
                delta,
                ..
            } if assistant_message_id == &self.assistant_message_id => {
                self.assistant_draft.content.push_str(delta);
                self.assistant_draft.status = StreamingAssistantStatus::Streaming;
            }
            ChatSseEvent::ReasoningDelta {
                assistant_message_id,
                delta,
            } if assistant_message_id == &self.assistant_message_id => {
                self.assistant_draft.reasoning.push_str(delta);
                self.assistant_draft.status = StreamingAssistantStatus::Streaming;
            }
            ChatSseEvent::ToolCall {
                assistant_message_id,
                ..
            } if assistant_message_id == &self.assistant_message_id => {
                self.assistant_draft.status = StreamingAssistantStatus::Streaming;
            }
            ChatSseEvent::Error { message } => {
                if self.cancellation_is_active() {
                    self.assistant_draft.status = StreamingAssistantStatus::Cancelled;
                    self.assistant_draft.error_message = None;
                } else {
                    self.assistant_draft.status = StreamingAssistantStatus::Failed;
                    self.assistant_draft.error_message = Some(message.clone());
                }
            }
            _ => return Ok(()),
        }

        self.persist_assistant_draft(database, chat_id)
    }

    fn persist_tool_state_for_event(
        &self,
        database: &mut WorkspaceDatabase,
        chat_id: &str,
        event: &ChatSseEvent,
    ) -> Result<(), ApiError> {
        match event {
            ChatSseEvent::ToolCall {
                assistant_message_id,
                tool_call,
                ..
            } if assistant_message_id == &self.assistant_message_id => {
                let input_json = serde_json::to_string(&tool_call.input).map_err(|source| {
                    ApiError::internal(format!("failed to serialize tool input: {source}"))
                })?;
                let started_at = utc_timestamp();
                database
                    .upsert_tool_call(NewToolCall {
                        id: &tool_call.id,
                        chat_id,
                        run_id: &self.run_id,
                        message_id: Some(&self.assistant_message_id),
                        tool_name: &tool_call.name,
                        input_json: &input_json,
                        status: "running",
                        started_at: &started_at,
                        completed_at: None,
                    })
                    .map_err(ApiError::from_workspace_error)?;
            }
            ChatSseEvent::ToolResult {
                assistant_message_id,
                tool_call_id,
                output,
                is_error,
                ..
            } if assistant_message_id == &self.assistant_message_id => {
                let output_json = serde_json::to_string(output).map_err(|source| {
                    ApiError::internal(format!("failed to serialize tool output: {source}"))
                })?;
                let completed_at = utc_timestamp();
                let result_id = format!("{tool_call_id}-result");
                database
                    .upsert_tool_result(NewToolResult {
                        id: &result_id,
                        tool_call_id,
                        output_json: &output_json,
                        is_error: *is_error,
                        created_at: &completed_at,
                    })
                    .map_err(ApiError::from_workspace_error)?;
                database
                    .complete_tool_call(
                        tool_call_id,
                        if *is_error { "error" } else { "completed" },
                        &completed_at,
                    )
                    .map_err(ApiError::from_workspace_error)?;
            }
            ChatSseEvent::StreamReset { .. } => {
                database
                    .delete_running_tool_calls_for_run(&self.run_id)
                    .map_err(ApiError::from_workspace_error)?;
            }
            ChatSseEvent::Complete { .. } => {
                database
                    .delete_incomplete_tool_calls_for_run(&self.run_id)
                    .map_err(ApiError::from_workspace_error)?;
            }
            ChatSseEvent::Error { .. } => {
                let completed_at = utc_timestamp();
                database
                    .complete_running_tool_calls_for_run(
                        &self.run_id,
                        if self.cancellation_is_active() {
                            "cancelled"
                        } else {
                            "error"
                        },
                        &completed_at,
                    )
                    .map_err(ApiError::from_workspace_error)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn cancellation_is_active(&self) -> bool {
        *self.cancellation.subscribe().borrow()
    }

    fn persist_assistant_draft(
        &mut self,
        database: &mut WorkspaceDatabase,
        chat_id: &str,
    ) -> Result<(), ApiError> {
        let reasoning = non_empty_string(&self.assistant_draft.reasoning);
        let events = database
            .run_events_for_run(&self.run_id)
            .map_err(ApiError::from_workspace_error)?
            .into_iter()
            .map(|event| CapturedAuditEvent {
                event_at: event.created_at,
                event_type: event.event_type,
                normalized_event_json: event.payload_json,
            })
            .collect::<Vec<_>>();
        let tool_calls = database
            .tool_calls_for_chat(chat_id)
            .map_err(ApiError::from_workspace_error)?
            .into_iter()
            .filter(|tool_call| {
                tool_call.message_id.as_deref() == Some(self.assistant_message_id.as_str())
            })
            .map(chat_tool_call_summary)
            .collect::<Result<Vec<_>, _>>()?;
        let parts = finalized_assistant_message_parts(
            &self.assistant_message_id,
            &events,
            &self.assistant_draft.content,
            reasoning.as_deref(),
            &tool_calls,
            self.assistant_draft.error_message.as_deref(),
        )?;
        let metadata_json = assistant_message_metadata_json(
            reasoning.as_deref(),
            &self.memories_used,
            &CodeChangeStats::default(),
            self.assistant_draft.status.as_metadata_value(),
            Some(&parts),
            self.assistant_draft.error_message.as_deref(),
        )?;

        database
            .upsert_message_content(NewMessage {
                id: &self.assistant_message_id,
                chat_id,
                role: "assistant",
                content: &self.assistant_draft.content,
                sequence: self.assistant_sequence,
                metadata_json: Some(&metadata_json),
            })
            .map_err(ApiError::from_workspace_error)?;

        Ok(())
    }

    pub(crate) fn finish(&mut self) {
        self.completed = true;
        let _ = self.completed_tx.send(true);
        self.registry.unregister(&self.run_id);
    }

    pub(crate) fn finish_suspended(
        &mut self,
        workspace_path: &Path,
        chat_id: &str,
    ) -> Result<(), ApiError> {
        let result = if self.primary_chat_output
            && self.assistant_draft.status == StreamingAssistantStatus::Streaming
        {
            self.assistant_draft.status = StreamingAssistantStatus::Pending;
            match open_workspace_database(workspace_path) {
                Ok(mut database) => self.persist_assistant_draft(&mut database, chat_id),
                Err(error) => Err(error),
            }
        } else {
            Ok(())
        };
        self.finish();
        result
    }
}

impl Drop for ActiveChatRunRegistration {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self.completed_tx.send(true);
            self.registry.unregister(&self.run_id);
        }
    }
}

pub(crate) struct ActiveChatRunSubscription {
    pub(crate) replay: Vec<ChatRunEventFrame>,
    pub(crate) event_rx: broadcast::Receiver<ChatRunEventFrame>,
    pub(crate) completed_rx: watch::Receiver<bool>,
    pub(crate) after_sequence: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActiveChatRunSummary {
    pub(crate) run_id: String,
    workspace_id: String,
    chat_id: String,
    pub(crate) last_sequence: Option<i64>,
    pub(crate) accepting_guidance: bool,
}

pub(crate) fn chat_run_subscription_stream(
    mut subscription: ActiveChatRunSubscription,
) -> impl futures_util::Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        let mut last_sequence = subscription.after_sequence;
        for event in subscription.replay {
            if event.sequence > last_sequence {
                last_sequence = event.sequence;
                yield Ok(sse_event_frame(&event));
            }
        }


        if *subscription.completed_rx.borrow() {
            yield Ok(sse_event(&ChatSseEvent::StreamEnd));
            return;
        }

        loop {
            tokio::select! {
                changed = subscription.completed_rx.changed() => {
                    if changed.is_err() || *subscription.completed_rx.borrow() {
                        while let Ok(event) = subscription.event_rx.try_recv() {
                            if event.sequence > last_sequence {
                                last_sequence = event.sequence;
                                yield Ok(sse_event_frame(&event));
                            }
                        }
                        yield Ok(sse_event(&ChatSseEvent::StreamEnd));
                        return;
                    }
                }
                event = subscription.event_rx.recv() => {
                    match event {
                        Ok(event) => {
                            if event.sequence > last_sequence {
                                last_sequence = event.sequence;
                                yield Ok(sse_event_frame(&event));
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            let event = ChatSseEvent::Error {
                                message: "chat run event subscriber lagged behind; refresh to replay the run".to_string(),
                            };
                            yield Ok(sse_event(&event));
                            return;
                        }
                        Err(broadcast::error::RecvError::Closed) => return,
                    }
                }
            }
        }
    }
}

fn sse_event_frame(event: &ChatRunEventFrame) -> Event {
    sse_event_payload(&event.payload_json).id(event.sequence.to_string())
}

#[cfg(test)]
mod tests {
    use axum::{
        body::to_bytes,
        response::{IntoResponse, Sse},
    };
    use tokio::sync::{broadcast, mpsc, watch};

    use super::*;

    #[tokio::test]
    async fn chat_run_subscription_stream_replays_after_sequence_with_sse_ids() {
        let (_event_tx, event_rx) = broadcast::channel(1);
        let (_completed_tx, completed_rx) = watch::channel(true);
        let subscription = ActiveChatRunSubscription {
            replay: vec![
                ChatRunEventFrame {
                    sequence: 1,
                    event_type: "textDelta".to_string(),
                    payload_json: r#"{"type":"textDelta","delta":"old"}"#.to_string(),
                },
                ChatRunEventFrame {
                    sequence: 2,
                    event_type: "textDelta".to_string(),
                    payload_json: r#"{"type":"textDelta","delta":"new"}"#.to_string(),
                },
            ],
            event_rx,
            completed_rx,
            after_sequence: 1,
        };

        let body = Sse::new(chat_run_subscription_stream(subscription))
            .into_response()
            .into_body();
        let bytes = to_bytes(body, usize::MAX).await.expect("SSE body reads");
        let text = String::from_utf8(bytes.to_vec()).expect("SSE is utf-8");

        assert!(!text.contains("id: 1"));
        assert!(!text.contains("old"));
        assert!(text.contains("id: 2"));
        assert!(text.contains("new"));
    }

    #[tokio::test]
    async fn agent_message_guidance_delivers_only_to_the_exact_active_task() {
        let registry = ActiveChatRunRegistry::default();
        let team_id = AgentTeamId::new("agent-team-guidance").expect("team id");
        let instance_id = AgentInstanceId::new("agent-instance-guidance").expect("instance id");
        let task_id = AgentTaskId::new("agent-task-guidance").expect("task id");
        let other_task_id = AgentTaskId::new("agent-task-guidance-other").expect("other task id");
        let (guidance_tx, mut guidance_rx) = mpsc::unbounded_channel();
        let _registration = registry
            .register_agent(
                "run-guidance".to_string(),
                "workspace-guidance".to_string(),
                "chat-guidance".to_string(),
                "assistant-guidance".to_string(),
                1,
                Vec::new(),
                false,
                ActiveAgentRunIdentity {
                    team_id: team_id.clone(),
                    instance_id: instance_id.clone(),
                    task_id: task_id.clone(),
                    _attempt_id: AgentAttemptId::new("agent-attempt-guidance").expect("attempt id"),
                },
                0,
                guidance_tx,
            )
            .expect("register active Agent run");
        let guidance = GuidanceMessage {
            id: "agent-message-guidance".to_string(),
            content: "apply this now".to_string(),
            attachments: Vec::new(),
            source: crate::runtime::AGENT_MESSAGE_GUIDANCE_SOURCE.to_string(),
            interrupted_assistant_id: None,
        };

        assert_eq!(
            registry.deliver_agent_message_guidance(
                "workspace-guidance",
                &team_id,
                &instance_id,
                Some(&other_task_id),
                guidance.clone(),
            ),
            AgentMessageGuidanceDelivery::Queued
        );
        assert!(guidance_rx.try_recv().is_err());
        assert_eq!(
            registry.deliver_agent_message_guidance(
                "workspace-guidance",
                &team_id,
                &instance_id,
                Some(&task_id),
                guidance,
            ),
            AgentMessageGuidanceDelivery::Guidance
        );
        assert_eq!(
            guidance_rx.recv().await.expect("guidance delivered").id,
            "agent-message-guidance"
        );
    }

    #[tokio::test]
    async fn agent_message_guidance_queues_when_matching_runs_are_ambiguous() {
        let registry = ActiveChatRunRegistry::default();
        let team_id = AgentTeamId::new("agent-team-guidance-ambiguous").expect("team id");
        let instance_id =
            AgentInstanceId::new("agent-instance-guidance-ambiguous").expect("instance id");
        let first_identity = ActiveAgentRunIdentity {
            team_id: team_id.clone(),
            instance_id: instance_id.clone(),
            task_id: AgentTaskId::new("agent-task-guidance-ambiguous-first")
                .expect("first task id"),
            _attempt_id: AgentAttemptId::new("agent-attempt-guidance-ambiguous-first")
                .expect("first attempt id"),
        };
        let second_identity = ActiveAgentRunIdentity {
            team_id: team_id.clone(),
            instance_id: instance_id.clone(),
            task_id: AgentTaskId::new("agent-task-guidance-ambiguous-second")
                .expect("second task id"),
            _attempt_id: AgentAttemptId::new("agent-attempt-guidance-ambiguous-second")
                .expect("second attempt id"),
        };
        let (first_tx, mut first_rx) = mpsc::unbounded_channel();
        let (second_tx, mut second_rx) = mpsc::unbounded_channel();
        let _first = registry
            .register_agent(
                "run-guidance-first".to_string(),
                "workspace-guidance".to_string(),
                "chat-guidance-first".to_string(),
                "assistant-guidance-first".to_string(),
                1,
                Vec::new(),
                false,
                first_identity,
                0,
                first_tx,
            )
            .expect("register first run");
        let _second = registry
            .register_agent(
                "run-guidance-second".to_string(),
                "workspace-guidance".to_string(),
                "chat-guidance-second".to_string(),
                "assistant-guidance-second".to_string(),
                1,
                Vec::new(),
                false,
                second_identity,
                0,
                second_tx,
            )
            .expect("register second run");

        assert_eq!(
            registry.deliver_agent_message_guidance(
                "workspace-guidance",
                &team_id,
                &instance_id,
                None,
                GuidanceMessage {
                    id: "agent-message-guidance-ambiguous".to_string(),
                    content: "do not misroute".to_string(),
                    attachments: Vec::new(),
                    source: crate::runtime::AGENT_MESSAGE_GUIDANCE_SOURCE.to_string(),
                    interrupted_assistant_id: None,
                },
            ),
            AgentMessageGuidanceDelivery::Queued
        );
        assert!(first_rx.try_recv().is_err());
        assert!(second_rx.try_recv().is_err());
    }

    #[test]
    fn agent_message_guidance_queues_when_workspace_team_or_instance_does_not_match() {
        let registry = ActiveChatRunRegistry::default();
        let team_id = AgentTeamId::new("agent-team-guidance-routing").expect("team id");
        let instance_id =
            AgentInstanceId::new("agent-instance-guidance-routing").expect("instance id");
        let task_id = AgentTaskId::new("agent-task-guidance-routing").expect("task id");
        let (guidance_tx, mut guidance_rx) = mpsc::unbounded_channel();
        let _registration = registry
            .register_agent(
                "run-guidance-routing".to_string(),
                "workspace-guidance".to_string(),
                "chat-guidance".to_string(),
                "assistant-guidance".to_string(),
                1,
                Vec::new(),
                false,
                ActiveAgentRunIdentity {
                    team_id: team_id.clone(),
                    instance_id: instance_id.clone(),
                    task_id: task_id.clone(),
                    _attempt_id: AgentAttemptId::new("agent-attempt-guidance-routing")
                        .expect("attempt id"),
                },
                0,
                guidance_tx,
            )
            .expect("register active Agent run");
        let other_team_id = AgentTeamId::new("agent-team-guidance-other").expect("other team id");
        let other_instance_id =
            AgentInstanceId::new("agent-instance-guidance-other").expect("other instance id");

        assert_eq!(
            registry.deliver_agent_message_guidance(
                "workspace-other",
                &team_id,
                &instance_id,
                Some(&task_id),
                agent_message_guidance("agent-message-guidance-workspace"),
            ),
            AgentMessageGuidanceDelivery::Queued
        );
        assert_eq!(
            registry.deliver_agent_message_guidance(
                "workspace-guidance",
                &other_team_id,
                &instance_id,
                Some(&task_id),
                agent_message_guidance("agent-message-guidance-team"),
            ),
            AgentMessageGuidanceDelivery::Queued
        );
        assert_eq!(
            registry.deliver_agent_message_guidance(
                "workspace-guidance",
                &team_id,
                &other_instance_id,
                Some(&task_id),
                agent_message_guidance("agent-message-guidance-instance"),
            ),
            AgentMessageGuidanceDelivery::Queued
        );
        assert!(guidance_rx.try_recv().is_err());
    }

    #[test]
    fn agent_message_guidance_queues_when_matching_guidance_channel_has_closed() {
        let registry = ActiveChatRunRegistry::default();
        let team_id = AgentTeamId::new("agent-team-guidance-closed").expect("team id");
        let instance_id =
            AgentInstanceId::new("agent-instance-guidance-closed").expect("instance id");
        let task_id = AgentTaskId::new("agent-task-guidance-closed").expect("task id");
        let (guidance_tx, guidance_rx) = mpsc::unbounded_channel();
        let _registration = registry
            .register_agent(
                "run-guidance-closed".to_string(),
                "workspace-guidance".to_string(),
                "chat-guidance".to_string(),
                "assistant-guidance".to_string(),
                1,
                Vec::new(),
                false,
                ActiveAgentRunIdentity {
                    team_id: team_id.clone(),
                    instance_id: instance_id.clone(),
                    task_id: task_id.clone(),
                    _attempt_id: AgentAttemptId::new("agent-attempt-guidance-closed")
                        .expect("attempt id"),
                },
                0,
                guidance_tx,
            )
            .expect("register active Agent run");
        drop(guidance_rx);

        assert_eq!(
            registry.deliver_agent_message_guidance(
                "workspace-guidance",
                &team_id,
                &instance_id,
                Some(&task_id),
                agent_message_guidance("agent-message-guidance-closed"),
            ),
            AgentMessageGuidanceDelivery::Queued
        );
    }

    #[tokio::test]
    async fn ordinary_chat_registration_still_accepts_manual_guidance() {
        let registry = ActiveChatRunRegistry::default();
        let (guidance_tx, mut guidance_rx) = mpsc::unbounded_channel();
        let _registration = registry
            .register(
                "run-manual-guidance".to_string(),
                "workspace-guidance".to_string(),
                "chat-guidance".to_string(),
                "assistant-guidance".to_string(),
                1,
                Vec::new(),
                false,
                0,
                guidance_tx,
            )
            .expect("register ordinary chat run");

        let guidance = registry
            .push_guidance(
                "workspace-guidance",
                ChatGuidanceRequest {
                    chat_id: "chat-guidance".to_string(),
                    run_id: "run-manual-guidance".to_string(),
                    message: "Continue manually.".to_string(),
                    attachments: Vec::new(),
                },
            )
            .expect("manual guidance accepted");

        assert_eq!(guidance.source, crate::runtime::MANUAL_GUIDANCE_SOURCE);
        assert_eq!(
            guidance_rx
                .recv()
                .await
                .expect("manual guidance delivered")
                .content,
            "Continue manually."
        );
    }

    fn agent_message_guidance(id: &str) -> GuidanceMessage {
        GuidanceMessage {
            id: id.to_string(),
            content: "apply this guidance".to_string(),
            attachments: Vec::new(),
            source: crate::runtime::AGENT_MESSAGE_GUIDANCE_SOURCE.to_string(),
            interrupted_assistant_id: None,
        }
    }
}
