use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use foco_providers::{NeutralChatMessage, NeutralUsage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::{AgentAttemptId, AgentInstanceId, AgentTaskId, AgentTeamId};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRunAssociations {
    pub team_id: Option<AgentTeamId>,
    pub instance_id: Option<AgentInstanceId>,
    pub task_id: Option<AgentTaskId>,
    pub attempt_id: Option<AgentAttemptId>,
}

#[derive(Clone, Debug, Default)]
pub struct AgentRunCancellation {
    cancelled: Arc<AtomicBool>,
}

impl AgentRunCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Debug)]
pub struct AgentRunContext {
    pub chat_id: String,
    pub workspace_id: String,
    pub workspace_path: PathBuf,
    pub provider_id: String,
    pub model_id: String,
    pub associations: AgentRunAssociations,
    pub definition_snapshot: Value,
    pub cancellation: AgentRunCancellation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRunRecovery {
    pub checkpoint: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRunInput {
    pub messages: Vec<NeutralChatMessage>,
    pub current_task: Option<Value>,
    #[serde(default)]
    pub unread_messages: Vec<Value>,
    pub recovery: Option<AgentRunRecovery>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunEventKind {
    Reasoning,
    Text,
    Usage,
    Completion,
    Error,
    ToolCall,
    ToolResult,
    ControlOutcome,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRunEvent<E> {
    pub associations: AgentRunAssociations,
    pub kind: AgentRunEventKind,
    pub payload: E,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum AgentRunOutcome {
    Completed {
        text: String,
        reasoning: Option<String>,
        usage: Option<NeutralUsage>,
    },
    Failed {
        message: String,
        retryable: bool,
    },
    Cancelled {
        message: String,
    },
    Suspended {
        control: Value,
    },
}

pub trait AgentRunEventSink<E> {
    fn emit(&mut self, event: AgentRunEvent<E>) -> Result<(), String>;
}

impl<E, F> AgentRunEventSink<E> for F
where
    F: FnMut(AgentRunEvent<E>) -> Result<(), String>,
{
    fn emit(&mut self, event: AgentRunEvent<E>) -> Result<(), String> {
        self(event)
    }
}

pub type AgentRunFuture = Pin<Box<dyn Future<Output = AgentRunOutcome> + Send + 'static>>;

pub trait AgentRunTask<E>: Send + 'static {
    fn run(
        self,
        context: AgentRunContext,
        input: AgentRunInput,
        events: AgentRunEventEmitter<E>,
    ) -> AgentRunFuture;
}

pub struct AgentRunEventEmitter<E> {
    associations: AgentRunAssociations,
    tx: mpsc::UnboundedSender<AgentRunEvent<E>>,
}

impl<E> AgentRunEventEmitter<E> {
    pub fn emit(&self, kind: AgentRunEventKind, payload: E) -> Result<(), String> {
        self.tx
            .send(AgentRunEvent {
                associations: self.associations.clone(),
                kind,
                payload,
            })
            .map_err(|_| "AgentRun event sink closed before execution completed".to_string())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AgentRunExecutor;

impl AgentRunExecutor {
    pub async fn execute<E, T, S>(
        self,
        context: AgentRunContext,
        input: AgentRunInput,
        task: T,
        mut sink: S,
    ) -> AgentRunOutcome
    where
        E: Send + 'static,
        T: AgentRunTask<E>,
        S: AgentRunEventSink<E>,
    {
        let cancellation = context.cancellation.clone();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let events = AgentRunEventEmitter {
            associations: context.associations.clone(),
            tx: event_tx,
        };
        let run = task.run(context, input, events);
        tokio::pin!(run);

        loop {
            tokio::select! {
                outcome = &mut run => {
                    while let Ok(event) = event_rx.try_recv() {
                        if let Err(message) = sink.emit(event) {
                            cancellation.cancel();
                            return AgentRunOutcome::Failed { message, retryable: false };
                        }
                    }
                    return outcome;
                }
                event = event_rx.recv() => {
                    let Some(event) = event else {
                        return run.as_mut().await;
                    };
                    if let Err(message) = sink.emit(event) {
                        cancellation.cancel();
                        return AgentRunOutcome::Failed { message, retryable: false };
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentInstanceId, AgentTaskId, AgentTeamId};
    use foco_providers::{NeutralChatRole, NeutralUsage};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    struct FixtureRun;

    impl AgentRunTask<&'static str> for FixtureRun {
        fn run(
            self,
            _context: AgentRunContext,
            input: AgentRunInput,
            events: AgentRunEventEmitter<&'static str>,
        ) -> AgentRunFuture {
            Box::pin(async move {
                assert_eq!(input.messages.len(), 1);
                events
                    .emit(AgentRunEventKind::Text, "text")
                    .expect("text event");
                events
                    .emit(AgentRunEventKind::ToolCall, "tool_call")
                    .expect("tool call event");
                events
                    .emit(AgentRunEventKind::ToolResult, "tool_result")
                    .expect("tool result event");
                events
                    .emit(AgentRunEventKind::Completion, "completion")
                    .expect("completion event");
                AgentRunOutcome::Completed {
                    text: "done".to_string(),
                    reasoning: None,
                    usage: Some(NeutralUsage {
                        input_tokens: Some(2),
                        output_tokens: Some(1),
                        ..NeutralUsage::default()
                    }),
                }
            })
        }
    }

    fn fixture_context() -> AgentRunContext {
        AgentRunContext {
            chat_id: "chat-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            workspace_path: PathBuf::from("workspace"),
            provider_id: "provider-1".to_string(),
            model_id: "model-1".to_string(),
            associations: AgentRunAssociations {
                team_id: Some(AgentTeamId::new("agent-team-1").expect("team id")),
                instance_id: Some(AgentInstanceId::new("agent-instance-1").expect("instance id")),
                task_id: Some(AgentTaskId::new("agent-task-1").expect("task id")),
                attempt_id: None,
            },
            definition_snapshot: json!({ "revision": 1 }),
            cancellation: AgentRunCancellation::default(),
        }
    }

    fn fixture_input() -> AgentRunInput {
        AgentRunInput {
            messages: vec![NeutralChatMessage {
                role: NeutralChatRole::User,
                content: "work".to_string(),
                attachments: Vec::new(),
                reasoning: None,
                tool_calls: Vec::new(),
                tool_call_id: None,
                tool_name: None,
            }],
            current_task: Some(json!({ "prompt": "work" })),
            unread_messages: Vec::new(),
            recovery: None,
        }
    }

    #[tokio::test]
    async fn executor_preserves_event_order_and_agent_associations() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let sink_received = received.clone();

        let outcome = AgentRunExecutor
            .execute(
                fixture_context(),
                fixture_input(),
                FixtureRun,
                move |event: AgentRunEvent<&'static str>| {
                    sink_received.lock().expect("events lock").push(event);
                    Ok(())
                },
            )
            .await;

        assert!(matches!(outcome, AgentRunOutcome::Completed { .. }));
        let received = received.lock().expect("events lock");
        assert_eq!(
            received
                .iter()
                .map(|event| event.payload)
                .collect::<Vec<_>>(),
            vec!["text", "tool_call", "tool_result", "completion"]
        );
        assert!(received.iter().all(|event| {
            event.associations.team_id.as_ref().map(AgentTeamId::as_str) == Some("agent-team-1")
        }));
    }

    #[tokio::test]
    async fn sink_failure_cancels_run_without_retrying() {
        let context = fixture_context();
        let cancellation = context.cancellation.clone();

        let outcome = AgentRunExecutor
            .execute(
                context,
                fixture_input(),
                FixtureRun,
                |_event: AgentRunEvent<&'static str>| Err("sink failed".to_string()),
            )
            .await;

        assert_eq!(
            outcome,
            AgentRunOutcome::Failed {
                message: "sink failed".to_string(),
                retryable: false,
            }
        );
        assert!(cancellation.is_cancelled());
    }
}
