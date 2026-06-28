use foco_agent::{
    AgentRunContext, AgentRunEvent, AgentRunEventEmitter, AgentRunEventKind, AgentRunExecutor,
    AgentRunFuture, AgentRunInput, AgentRunOutcome, AgentRunTask,
};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::*;

pub(crate) async fn run_chat_context_in_background(
    chat_context: PreparedChatContext,
    mut active_run_registration: ActiveChatRunRegistration,
    guidance_rx: mpsc::UnboundedReceiver<GuidanceMessage>,
) -> AgentRunOutcome {
    let workspace_path = chat_context.workspace_path.clone();
    let chat_id = chat_context.chat_id.clone();
    let session_upload_paths = chat_context.session_upload_paths.clone();
    let cancellation = active_run_registration.cancellation().clone();
    let definition_snapshot = chat_context
        .agent_definition_snapshot
        .clone()
        .unwrap_or_else(|| {
            json!({
                "providerId": &chat_context.provider_id,
                "modelId": &chat_context.model_id,
                "thinkingLevel": chat_context.provider_request.thinking_level,
                "maxOutputTokens": chat_context.provider_request.max_output_tokens,
                "allowedTools": chat_context.provider_request.tools.iter().map(|tool| &tool.name).collect::<Vec<_>>(),
            })
        });
    let current_task = chat_context.agent_task_input.clone();
    let unread_messages = chat_context.agent_unread_messages.clone();
    let run_context = AgentRunContext {
        chat_id: chat_context.chat_id.clone(),
        workspace_id: chat_context.workspace_id.clone(),
        workspace_path: chat_context.workspace_path.clone(),
        provider_id: chat_context.provider_id.clone(),
        model_id: chat_context.model_id.clone(),
        associations: chat_context.agent_associations.clone(),
        definition_snapshot,
        cancellation: cancellation.agent_token(),
    };
    let run_input = AgentRunInput {
        messages: chat_context.provider_request.messages.clone(),
        current_task,
        unread_messages,
        recovery: None,
    };
    let task = FocoAgentRunTask {
        chat_context,
        cancellation: cancellation.clone(),
        guidance_rx,
    };
    let mut delivery_error = None;
    let outcome = AgentRunExecutor
        .execute(
            run_context,
            run_input,
            task,
            |event: AgentRunEvent<ChatSseEvent>| {
                active_run_registration
                    .record_event(&workspace_path, &chat_id, &event.payload)
                    .map_err(|error| {
                        delivery_error = Some(error.message.clone());
                        error.message
                    })
            },
        )
        .await;

    if let Some(message) = delivery_error {
        tracing::warn!(
            error = %message,
            run_id = %active_run_registration.run_id,
            "failed to record chat run event"
        );
        cancellation.cancel();
        let error_event = ChatSseEvent::Error { message };
        let _ = active_run_registration.record_event(&workspace_path, &chat_id, &error_event);
    }

    if matches!(&outcome, AgentRunOutcome::Suspended { .. }) {
        if let Err(error) = active_run_registration.finish_suspended(&workspace_path, &chat_id) {
            tracing::warn!(
                error = %error.message,
                run_id = %active_run_registration.run_id,
                "failed to clear suspended chat run draft state"
            );
        }
    } else {
        active_run_registration.finish();
    }
    let cleanup_result = match session_upload_paths {
        Some(paths) => cleanup_chat_session_upload_files(&workspace_path, &chat_id, &paths),
        None => cleanup_chat_session_uploads(&workspace_path, &chat_id),
    };
    if let Err(error) = cleanup_result {
        tracing::warn!(
            error = %error.message,
            chat_id = %chat_id,
            "failed to clean up chat session uploads"
        );
    }
    outcome
}

struct FocoAgentRunTask {
    chat_context: PreparedChatContext,
    cancellation: ChatRunCancellation,
    guidance_rx: mpsc::UnboundedReceiver<GuidanceMessage>,
}

impl AgentRunTask<ChatSseEvent> for FocoAgentRunTask {
    fn run(
        mut self,
        context: AgentRunContext,
        input: AgentRunInput,
        events: AgentRunEventEmitter<ChatSseEvent>,
    ) -> AgentRunFuture {
        Box::pin(async move {
            self.chat_context.agent_associations = context.associations;
            self.chat_context.provider_request.messages = input.messages;
            let stream = self
                .chat_context
                .into_sse_stream(self.cancellation.clone(), self.guidance_rx);
            tokio::pin!(stream);
            let mut completion = None;
            let mut last_error = None;

            while let Some(event) = stream.next().await {
                let suspend_control = match &event {
                    ChatSseEvent::ToolResult {
                        output,
                        is_error: false,
                        ..
                    } => agent_suspend_control(output),
                    _ => None,
                };
                match &event {
                    ChatSseEvent::Complete {
                        text,
                        reasoning,
                        usage,
                        ..
                    } => {
                        completion = Some((text.clone(), reasoning.clone(), usage.clone()));
                    }
                    ChatSseEvent::Error { message } => last_error = Some(message.clone()),
                    _ => {}
                }
                let kind = agent_run_event_kind(&event);
                if let Err(message) = events.emit(kind, event) {
                    return AgentRunOutcome::Failed {
                        message,
                        retryable: false,
                    };
                }
                if let Some(control) = suspend_control {
                    return AgentRunOutcome::Suspended { control };
                }
            }

            if context.cancellation.is_cancelled() {
                AgentRunOutcome::Cancelled {
                    message: last_error.unwrap_or_else(|| "agent run cancelled".to_string()),
                }
            } else if let Some((text, reasoning, usage)) = completion {
                AgentRunOutcome::Completed {
                    text,
                    reasoning,
                    usage,
                }
            } else {
                AgentRunOutcome::Failed {
                    message: last_error.unwrap_or_else(|| {
                        "agent run ended without a completion event".to_string()
                    }),
                    retryable: false,
                }
            }
        })
    }
}

fn agent_suspend_control(output: &Value) -> Option<Value> {
    let control = output.get("suspend")?;
    if control.get("kind").and_then(Value::as_str) == Some("agent_wait_tasks") {
        Some(control.clone())
    } else {
        None
    }
}

pub(crate) fn agent_run_event_kind(event: &ChatSseEvent) -> AgentRunEventKind {
    match event {
        ChatSseEvent::ReasoningDelta { .. } => AgentRunEventKind::Reasoning,
        ChatSseEvent::TextDelta { .. } => AgentRunEventKind::Text,
        ChatSseEvent::Usage { .. } => AgentRunEventKind::Usage,
        ChatSseEvent::Complete { .. } => AgentRunEventKind::Completion,
        ChatSseEvent::Error { .. } => AgentRunEventKind::Error,
        ChatSseEvent::ToolCall { .. } => AgentRunEventKind::ToolCall,
        ChatSseEvent::ToolResult { .. } => AgentRunEventKind::ToolResult,
        ChatSseEvent::Start { .. }
        | ChatSseEvent::StreamAttemptStart { .. }
        | ChatSseEvent::StreamReset { .. }
        | ChatSseEvent::ContextCompression { .. }
        | ChatSseEvent::ToolOutputDelta { .. }
        | ChatSseEvent::QuestionRequest { .. }
        | ChatSseEvent::HookNotification { .. }
        | ChatSseEvent::GuidanceApplied { .. }
        | ChatSseEvent::GitDiffRefresh { .. }
        | ChatSseEvent::TodoGraphRefresh { .. }
        | ChatSseEvent::PlanRefresh { .. }
        | ChatSseEvent::AgentTeamRefresh { .. }
        | ChatSseEvent::MemoryExtractionComplete { .. }
        | ChatSseEvent::MemoryResolved { .. }
        | ChatSseEvent::StreamEnd => AgentRunEventKind::ControlOutcome,
    }
}
