use std::{
    collections::HashMap,
    convert::Infallible,
    path::Path,
    sync::{Arc, Mutex},
};

use axum::response::sse::Event;
use foco_providers::NeutralChatAttachment;
use foco_store::workspace::{CodeChangeStats, NewMessage, NewRunEvent, WorkspaceDatabase};
use foco_tools::ToolCancellationToken;
use serde::Serialize;
use tokio::sync::{broadcast, mpsc, watch};

use crate::*;

#[derive(Clone, Default)]
pub(crate) struct ActiveChatRunRegistry {
    runs: Arc<Mutex<HashMap<String, ActiveChatRun>>>,
}

#[derive(Clone)]
struct ActiveChatRun {
    workspace_id: String,
    chat_id: String,
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
}

impl ChatRunCancellation {
    pub(crate) fn new() -> Self {
        let (tx, _rx) = watch::channel(false);
        Self {
            tx,
            tool_token: ToolCancellationToken::new(),
        }
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<bool> {
        self.tx.subscribe()
    }

    pub(crate) fn tool_token(&self) -> ToolCancellationToken {
        self.tool_token.clone()
    }

    pub(crate) fn cancel(&self) {
        self.tool_token.cancel();
        let _ = self.tx.send(true);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GuidanceMessage {
    pub(crate) id: String,
    pub(crate) content: String,
    pub(crate) attachments: Vec<NeutralChatAttachment>,
}

impl ActiveChatRunRegistry {
    pub(crate) fn register(
        &self,
        run_id: String,
        workspace_id: String,
        chat_id: String,
        assistant_message_id: String,
        assistant_sequence: i64,
        memories_used: Vec<ChatMemoryUsedSummary>,
        guidance_tx: mpsc::UnboundedSender<GuidanceMessage>,
    ) -> Result<ActiveChatRunRegistration, ApiError> {
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
            cancellation,
            events,
            event_tx,
            completed_tx,
            next_sequence: 0,
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
}

pub(crate) struct ActiveChatRunRegistration {
    registry: ActiveChatRunRegistry,
    pub(crate) run_id: String,
    assistant_message_id: String,
    assistant_sequence: i64,
    memories_used: Vec<ChatMemoryUsedSummary>,
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
        self.next_sequence += 1;

        {
            let mut database = WorkspaceDatabase::open_or_create(workspace_path)
                .map_err(ApiError::from_workspace_error)?;
            let id = format!("{}-event-{}", self.run_id, event_frame.sequence);
            database
                .insert_run_event(NewRunEvent {
                    id: &id,
                    chat_id,
                    run_id: &self.run_id,
                    sequence: event_frame.sequence,
                    event_type: &event_frame.event_type,
                    payload_json: &event_frame.payload_json,
                })
                .map_err(ApiError::from_workspace_error)?;
            self.persist_assistant_draft_for_event(&mut database, chat_id, event)?;
        }

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
            ChatSseEvent::Error { .. }
                if self.assistant_draft.status == StreamingAssistantStatus::Streaming =>
            {
                self.assistant_draft.status = if self.cancellation_is_active() {
                    StreamingAssistantStatus::Cancelled
                } else {
                    StreamingAssistantStatus::Failed
                };
            }
            _ => return Ok(()),
        }

        self.persist_assistant_draft(database, chat_id)
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
        let metadata_json = assistant_message_metadata_json(
            reasoning.as_deref(),
            &self.memories_used,
            &CodeChangeStats::default(),
            self.assistant_draft.status.as_metadata_value(),
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
            }
            yield Ok(sse_event_payload(&event.payload_json));
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
                                yield Ok(sse_event_payload(&event.payload_json));
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
                                yield Ok(sse_event_payload(&event.payload_json));
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
