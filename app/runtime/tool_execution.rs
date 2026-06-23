use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{Duration as ChronoDuration, SecondsFormat, Utc};

use foco_agent::{
    AgentCollaborationTool, AgentDefinitionId, AgentExecutionWorkspaceMode, AgentInstanceId,
    AgentMessageId, AgentMessageKind, AgentPermissions, AgentRunAssociations, AgentTaskId,
    AgentTaskStatus, AgentTaskWaitMode, PendingToolCall, ToolExecutionMode, ToolExecutionPlan,
    ToolResourceLock, tool_resource_locks,
};
use foco_mcp::{McpRegistry, is_mcp_tool_name};
use foco_providers::ProviderConnectionConfig;
use foco_store::config::{HookConfig, WebSearchSettings};
use foco_store::workspace::{
    AgentInstanceRecord, AgentTaskRecord, NewAgentEvent, NewAgentInstance, NewAgentMessage,
    NewAgentTask, NewAgentTaskDependency, WorkspaceDatabase,
};
use foco_tools::{
    AGENT_CANCEL_TASK_TOOL, AGENT_CREATE_INSTANCES_TOOL, AGENT_DELEGATE_TASK_TOOL,
    AGENT_GET_TASK_TOOL, AGENT_LIST_TOOL, AGENT_SEND_MESSAGE_TOOL, AGENT_TRANSFER_TASK_TOOL,
    AGENT_WAIT_TASKS_TOOL, ASK_QUESTION_TOOL, RUN_COMMAND_TOOL, SLEEP_TOOL, ToolCancellationToken,
    ToolExecution, ToolOutputSink, builtin_tool_timeout_ms,
    execute_builtin_tool_for_chat_with_cancellation_and_output_sink,
};
use futures_util::future::join_all;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::timeout;

use super::{
    AGENT_MAX_CREATE_INSTANCES_PER_REQUEST, AGENT_MAX_INSTANCES_PER_TEAM,
    AGENT_MAX_QUEUED_TASKS_PER_CHAT, AGENT_MAX_QUEUED_TASKS_PER_INSTANCE,
    AGENT_MAX_QUEUED_TASKS_PER_TEAM, AgentScheduler, AskQuestionInput, QuestionAnswer,
    QuestionItem, QuestionItemAnswer, QuestionOption, QuestionRegistry, QuestionRequest,
    ToolOutputDeltaSink, ToolResourceLease, ToolResourceLockOwner, ToolResourceLockRegistry,
    execute_web_tool, is_web_tool_name, web_tool_timeout_ms,
};
use crate::*;

use foco_providers::NeutralToolCall;
use foco_tools::{
    FIND_FILES_TOOL, GET_TODO_GRAPH_TOOL, GRAPH_EXPLORE_TOOL, GRAPH_FIND_CALLEES_TOOL,
    GRAPH_FIND_CALLERS_TOOL, GRAPH_FIND_REFERENCES_TOOL, GRAPH_FIND_SYMBOLS_TOOL,
    GRAPH_RELATED_FILES_TOOL, READ_FILE_TOOL, SEARCH_TEXT_TOOL,
};
use serde_json::Value;

use crate::git_backend::{create_agent_worktree, delete_agent_worktree};
use crate::{
    MAX_REPEATED_TOOL_CALL_BATCHES, MEMORY_SEARCH_TOOL_NAME, READ_ONLY_TOOL_BATCH_WARNING_THRESHOLD,
};

use foco_store::config::AgentDefinitionSettings;

const AGENT_MAX_CHILD_TASKS_PER_TASK: usize = 64;
const AGENT_MAX_DELEGATION_DEPTH: usize = 8;
const AGENT_MAX_MESSAGE_CONTENT_CHARS: usize = 16_384;
const AGENT_MAX_TASK_INPUT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq)]
struct ToolCallLoopSignature {
    name: String,
    arguments: Value,
}

#[derive(Default)]
pub(crate) struct RepeatedToolCallDetector {
    previous_batch: Option<Vec<ToolCallLoopSignature>>,
    consecutive_count: usize,
}

impl RepeatedToolCallDetector {
    pub(crate) fn check(&mut self, tool_calls: &[NeutralToolCall]) -> Result<(), String> {
        let batch = tool_call_loop_signatures(tool_calls);
        if self.previous_batch.as_ref() == Some(&batch) {
            self.consecutive_count += 1;
        } else {
            self.previous_batch = Some(batch);
            self.consecutive_count = 1;
        }

        if self.consecutive_count < MAX_REPEATED_TOOL_CALL_BATCHES {
            return Ok(());
        }

        let tool_names = self
            .previous_batch
            .as_ref()
            .map(|batch| {
                batch
                    .iter()
                    .map(|signature| signature.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        Err(format!(
            "agent run repeated the same tool call batch {MAX_REPEATED_TOOL_CALL_BATCHES} times ({tool_names}); possible tool-call loop"
        ))
    }
}

fn tool_call_loop_signatures(tool_calls: &[NeutralToolCall]) -> Vec<ToolCallLoopSignature> {
    tool_calls
        .iter()
        .map(|tool_call| ToolCallLoopSignature {
            name: tool_call.name.clone(),
            arguments: tool_call.arguments.clone(),
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReadOnlyToolProgressAction {
    Continue,
    Warn(String),
}

#[derive(Default)]
pub(crate) struct ReadOnlyToolProgressDetector {
    consecutive_read_only_batches: usize,
    warned: bool,
}

impl ReadOnlyToolProgressDetector {
    pub(crate) fn check(&mut self, tool_calls: &[NeutralToolCall]) -> ReadOnlyToolProgressAction {
        if tool_calls.is_empty()
            || !tool_calls
                .iter()
                .all(|tool_call| is_read_only_tool(&tool_call.name))
        {
            self.consecutive_read_only_batches = 0;
            self.warned = false;
            return ReadOnlyToolProgressAction::Continue;
        }

        self.consecutive_read_only_batches = self.consecutive_read_only_batches.saturating_add(1);

        if !self.warned
            && self.consecutive_read_only_batches >= READ_ONLY_TOOL_BATCH_WARNING_THRESHOLD
        {
            self.warned = true;
            return ReadOnlyToolProgressAction::Warn(format!(
                "Runtime progress guard: you have made {} consecutive read-only exploration tool batches without editing, asking a question, or finishing. Do not call more read-only exploration tools now. Either make the needed edit, ask one blocking question, or provide the final diagnosis/answer using the evidence already gathered.",
                self.consecutive_read_only_batches
            ));
        }

        ReadOnlyToolProgressAction::Continue
    }
}

fn is_read_only_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        READ_FILE_TOOL
            | FIND_FILES_TOOL
            | SEARCH_TEXT_TOOL
            | GRAPH_FIND_SYMBOLS_TOOL
            | GRAPH_FIND_CALLERS_TOOL
            | GRAPH_FIND_CALLEES_TOOL
            | GRAPH_FIND_REFERENCES_TOOL
            | GRAPH_RELATED_FILES_TOOL
            | GRAPH_EXPLORE_TOOL
            | GET_TODO_GRAPH_TOOL
            | MEMORY_SEARCH_TOOL_NAME
    )
}

pub(crate) fn pending_tool_calls(tool_calls: &[NeutralToolCall]) -> Vec<PendingToolCall> {
    tool_calls
        .iter()
        .map(|tool_call| PendingToolCall {
            id: tool_call.call_id.clone(),
            name: tool_call.name.clone(),
            arguments: tool_call.arguments.clone(),
        })
        .collect()
}

#[derive(Clone)]
pub(crate) struct AgentToolContext {
    pub(crate) workspace_path: PathBuf,
    pub(crate) associations: AgentRunAssociations,
    pub(crate) collaboration_tools_enabled: bool,
    pub(crate) permissions: AgentPermissions,
    pub(crate) agent_definitions: Vec<AgentDefinitionSettings>,
    pub(crate) scheduler: AgentScheduler,
}

pub(crate) async fn execute_tool_calls_parallel(
    mcp_registry: Arc<McpRegistry>,
    hook_runtime: HookRuntime,
    global_hooks: HookConfig,
    api_audit_save_details: bool,
    provider_config: ProviderConnectionConfig,
    web_search_settings: WebSearchSettings,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    memory_tool_context: MemoryToolContext,
    agent_tool_context: Option<AgentToolContext>,
    workspace_id: &str,
    workspace_path: &Path,
    tool_workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    assistant_message_id: &str,
    llm_request_retry_count: u32,
    tool_calls: Vec<NeutralToolCall>,
    execution_plan: ToolExecutionPlan,
    tool_resource_lock_registry: ToolResourceLockRegistry,
    cancellation_token: ToolCancellationToken,
    tool_output_delta_tx: mpsc::UnboundedSender<ToolOutputDeltaEvent>,
) -> Result<Vec<ToolHookOutcome>, ApiError> {
    let mut executed_by_index = (0..tool_calls.len())
        .map(|_| None)
        .collect::<Vec<Option<ToolHookOutcome>>>();

    for group in execution_plan.groups {
        match group.mode {
            ToolExecutionMode::Sequential => {
                for tool_index in group.call_indices {
                    let tool_call = tool_calls.get(tool_index).cloned().ok_or_else(|| {
                        ApiError::internal("tool execution plan referenced an unknown tool call")
                    })?;
                    let outcome = execute_tool_call(
                        mcp_registry.clone(),
                        hook_runtime.clone(),
                        global_hooks.clone(),
                        api_audit_save_details,
                        provider_config.clone(),
                        web_search_settings.clone(),
                        question_registry.clone(),
                        question_event_tx.clone(),
                        memory_tool_context.clone(),
                        agent_tool_context.clone(),
                        tool_resource_lock_registry.clone(),
                        cancellation_token.clone(),
                        tool_output_delta_tx.clone(),
                        assistant_message_id,
                        workspace_id,
                        workspace_path,
                        tool_workspace_path,
                        chat_id,
                        run_id,
                        model_id,
                        provider_id,
                        llm_request_retry_count,
                        tool_call,
                    )
                    .await;
                    executed_by_index[tool_index] = Some(outcome);
                }
            }
            ToolExecutionMode::Parallel => {
                let tasks = group.call_indices.into_iter().map(|tool_index| {
                    let workspace_path = workspace_path.to_path_buf();
                    let tool_workspace_path = tool_workspace_path.to_path_buf();
                    let workspace_id = workspace_id.to_string();
                    let chat_id = chat_id.to_string();
                    let run_id = run_id.to_string();
                    let model_id = model_id.to_string();
                    let provider_id = provider_id.to_string();
                    let assistant_message_id = assistant_message_id.to_string();
                    let llm_request_retry_count = llm_request_retry_count;
                    let mcp_registry = mcp_registry.clone();
                    let hook_runtime = hook_runtime.clone();
                    let global_hooks = global_hooks.clone();
                    let api_audit_save_details = api_audit_save_details;
                    let provider_config = provider_config.clone();
                    let web_search_settings = web_search_settings.clone();
                    let question_registry = question_registry.clone();
                    let question_event_tx = question_event_tx.clone();
                    let memory_tool_context = memory_tool_context.clone();
                    let agent_tool_context = agent_tool_context.clone();
                    let tool_resource_lock_registry = tool_resource_lock_registry.clone();
                    let cancellation_token = cancellation_token.clone();
                    let tool_output_delta_tx = tool_output_delta_tx.clone();
                    let tool_call = tool_calls.get(tool_index).cloned();

                    tokio::spawn(async move {
                        let tool_call = tool_call.ok_or_else(|| {
                            ApiError::internal(
                                "tool execution plan referenced an unknown tool call",
                            )
                        })?;
                        Ok::<_, ApiError>((
                            tool_index,
                            execute_tool_call(
                                mcp_registry,
                                hook_runtime,
                                global_hooks,
                                api_audit_save_details,
                                provider_config,
                                web_search_settings,
                                question_registry,
                                question_event_tx,
                                memory_tool_context,
                                agent_tool_context,
                                tool_resource_lock_registry,
                                cancellation_token,
                                tool_output_delta_tx,
                                &assistant_message_id,
                                &workspace_id,
                                &workspace_path,
                                &tool_workspace_path,
                                &chat_id,
                                &run_id,
                                &model_id,
                                &provider_id,
                                llm_request_retry_count,
                                tool_call,
                            )
                            .await,
                        ))
                    })
                });
                let results = join_all(AbortOnDropJoinHandle::new_each(tasks)).await;

                for result in results {
                    let (tool_index, outcome) = result.map_err(|source| {
                        ApiError::internal(format!("tool execution worker failed: {source}"))
                    })??;
                    executed_by_index[tool_index] = Some(outcome);
                }
            }
        }
    }

    executed_by_index
        .into_iter()
        .map(|outcome| {
            outcome.ok_or_else(|| {
                ApiError::internal("tool execution plan did not execute every tool call")
            })
        })
        .collect()
}

async fn execute_tool_call(
    mcp_registry: Arc<McpRegistry>,
    hook_runtime: HookRuntime,
    global_hooks: HookConfig,
    api_audit_save_details: bool,
    provider_config: ProviderConnectionConfig,
    web_search_settings: WebSearchSettings,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    mut memory_tool_context: MemoryToolContext,
    agent_tool_context: Option<AgentToolContext>,
    tool_resource_lock_registry: ToolResourceLockRegistry,
    cancellation_token: ToolCancellationToken,
    tool_output_delta_tx: mpsc::UnboundedSender<ToolOutputDeltaEvent>,
    assistant_message_id: &str,
    workspace_id: &str,
    workspace_path: &Path,
    tool_workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    llm_request_retry_count: u32,
    tool_call: NeutralToolCall,
) -> ToolHookOutcome {
    let started_at_text = utc_timestamp();
    memory_tool_context.tool_call_id = tool_call.call_id.clone();
    let tool_execution = execute_tool(
        mcp_registry,
        hook_runtime.clone(),
        &global_hooks,
        api_audit_save_details,
        &provider_config,
        &web_search_settings,
        question_registry,
        question_event_tx,
        memory_tool_context,
        agent_tool_context,
        tool_resource_lock_registry,
        cancellation_token.clone(),
        tool_output_delta_tx,
        assistant_message_id,
        workspace_id,
        workspace_path,
        tool_workspace_path,
        chat_id,
        run_id,
        model_id,
        provider_id,
        llm_request_retry_count,
        &tool_call.call_id,
        &tool_call.name,
        tool_call.arguments.clone(),
    )
    .await;
    let completed_at_text = utc_timestamp();
    let mut hook_summary = tool_execution.hook_summary;

    let executed = executed_tool_call(
        tool_call,
        tool_execution.execution,
        started_at_text,
        completed_at_text,
    );
    let post_event = if executed.is_error {
        "PostToolUseFailure"
    } else {
        "PostToolUse"
    };
    let post_summary = hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &global_hooks,
            api_audit_save_details,
            workspace_id,
            workspace_path,
            event: post_event,
            match_value: Some(executed.name.clone()),
            chat_id: Some(chat_id),
            run_id: Some(run_id),
            session_id: Some(chat_id),
            tool_call_id: Some(&executed.id),
            model_id: Some(model_id),
            provider_id: Some(provider_id),
            provider_config: Some(&provider_config),
            llm_request_retry_count,
            permission_mode: None,
            payload: json!({
                "toolName": executed.name.clone(),
                "toolInput": executed.input.clone(),
                "toolOutput": executed.output.clone(),
                "isError": executed.is_error,
            }),
        })
        .await;
    merge_hook_summaries(&mut hook_summary, post_summary);

    ToolHookOutcome {
        tool_call: executed,
        hook_summary,
    }
}

pub(crate) async fn execute_tool(
    mcp_registry: Arc<McpRegistry>,
    hook_runtime: HookRuntime,
    global_hooks: &HookConfig,
    api_audit_save_details: bool,
    provider_config: &ProviderConnectionConfig,
    web_search_settings: &WebSearchSettings,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    memory_tool_context: MemoryToolContext,
    agent_tool_context: Option<AgentToolContext>,
    tool_resource_lock_registry: ToolResourceLockRegistry,
    cancellation_token: ToolCancellationToken,
    tool_output_delta_tx: mpsc::UnboundedSender<ToolOutputDeltaEvent>,
    assistant_message_id: &str,
    workspace_id: &str,
    workspace_path: &Path,
    tool_workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    llm_request_retry_count: u32,
    tool_call_id: &str,
    tool_name: &str,
    mut arguments: Value,
) -> ToolExecutionWithHooks {
    if cancellation_token.is_cancelled() {
        return cancelled_tool_execution();
    }

    let pre_summary = hook_runtime
        .run_hooks(HookRunRequest {
            global_config: global_hooks,
            api_audit_save_details,
            workspace_id,
            workspace_path,
            event: "PreToolUse",
            match_value: Some(tool_name.to_string()),
            chat_id: Some(chat_id),
            run_id: Some(run_id),
            session_id: Some(chat_id),
            tool_call_id: Some(tool_call_id),
            model_id: Some(model_id),
            provider_id: Some(provider_id),
            provider_config: Some(provider_config),
            llm_request_retry_count,
            permission_mode: None,
            payload: json!({
                "toolName": tool_name,
                "toolInput": arguments.clone(),
            }),
        })
        .await;
    let blocking_decision = pre_summary
        .decisions
        .iter()
        .find(|decision| {
            matches!(
                decision,
                HookDecision::Block { .. } | HookDecision::Deny { .. } | HookDecision::Ask { .. }
            )
        })
        .cloned();
    let mut hook_summary = pre_summary;
    if let Some(updated_input) = hook_updated_input(&hook_summary) {
        arguments = updated_input;
    }
    if let Some(decision) = blocking_decision {
        match decision {
            HookDecision::Allow => {}
            HookDecision::Block { reason } | HookDecision::Deny { reason } => {
                return ToolExecutionWithHooks {
                    execution: ToolExecution {
                        output: json!({ "error": format!("PreToolUse hook blocked '{tool_name}': {reason}") }),
                        is_error: true,
                    },
                    hook_summary,
                };
            }
            HookDecision::Ask { reason } => {
                let permission_request_summary = hook_runtime
                    .run_hooks(HookRunRequest {
                        global_config: global_hooks,
                        api_audit_save_details,
                        workspace_id,
                        workspace_path,
                        event: "PermissionRequest",
                        match_value: Some(tool_name.to_string()),
                        chat_id: Some(chat_id),
                        run_id: Some(run_id),
                        session_id: Some(chat_id),
                        tool_call_id: Some(tool_call_id),
                        model_id: Some(model_id),
                        provider_id: Some(provider_id),
                        provider_config: Some(provider_config),
                        llm_request_retry_count,
                        permission_mode: Some("ask"),
                        payload: json!({
                            "toolName": tool_name,
                            "toolInput": arguments.clone(),
                            "reason": reason,
                        }),
                    })
                    .await;
                let permission_request_decision = permission_request_summary
                    .decisions
                    .iter()
                    .find(|decision| {
                        matches!(
                            decision,
                            HookDecision::Allow
                                | HookDecision::Block { .. }
                                | HookDecision::Deny { .. }
                                | HookDecision::Ask { .. }
                        )
                    })
                    .cloned();
                merge_hook_summaries(&mut hook_summary, permission_request_summary);

                if let Some(updated_input) = hook_updated_input(&hook_summary) {
                    arguments = updated_input;
                }

                let prompt_reason = match permission_request_decision {
                    Some(HookDecision::Allow) => None,
                    Some(HookDecision::Block { reason }) | Some(HookDecision::Deny { reason }) => {
                        let denied_summary = hook_runtime
                            .run_hooks(HookRunRequest {
                                global_config: global_hooks,
                                api_audit_save_details,
                                workspace_id,
                                workspace_path,
                                event: "PermissionDenied",
                                match_value: Some(tool_name.to_string()),
                                chat_id: Some(chat_id),
                                run_id: Some(run_id),
                                session_id: Some(chat_id),
                                tool_call_id: Some(tool_call_id),
                                model_id: Some(model_id),
                                provider_id: Some(provider_id),
                                provider_config: Some(provider_config),
                                llm_request_retry_count,
                                permission_mode: Some("deny"),
                                payload: json!({
                                    "toolName": tool_name,
                                    "toolInput": arguments.clone(),
                                    "reason": reason,
                                }),
                            })
                            .await;
                        let retry_message = permission_denied_retry_message(&denied_summary);
                        merge_hook_summaries(&mut hook_summary, denied_summary);
                        return ToolExecutionWithHooks {
                            execution: ToolExecution {
                                output: json!({
                                    "error": format!("PermissionRequest hook denied '{tool_name}': {reason}"),
                                    "retry": retry_message,
                                }),
                                is_error: true,
                            },
                            hook_summary,
                        };
                    }
                    Some(HookDecision::Ask { reason }) => Some(reason),
                    None => Some(reason),
                };

                if let Some(prompt_reason) = prompt_reason {
                    let permission = execute_hook_permission_question(
                        question_registry.clone(),
                        question_event_tx.clone(),
                        workspace_id,
                        chat_id,
                        tool_call_id,
                        tool_name,
                        &prompt_reason,
                    )
                    .await;
                    if let Err(reason) = permission {
                        let denied_summary = hook_runtime
                            .run_hooks(HookRunRequest {
                                global_config: global_hooks,
                                api_audit_save_details,
                                workspace_id,
                                workspace_path,
                                event: "PermissionDenied",
                                match_value: Some(tool_name.to_string()),
                                chat_id: Some(chat_id),
                                run_id: Some(run_id),
                                session_id: Some(chat_id),
                                tool_call_id: Some(tool_call_id),
                                model_id: Some(model_id),
                                provider_id: Some(provider_id),
                                provider_config: Some(provider_config),
                                llm_request_retry_count,
                                permission_mode: Some("deny"),
                                payload: json!({
                                    "toolName": tool_name,
                                    "toolInput": arguments.clone(),
                                    "reason": reason,
                                }),
                            })
                            .await;
                        let retry_message = permission_denied_retry_message(&denied_summary);
                        merge_hook_summaries(&mut hook_summary, denied_summary);
                        return ToolExecutionWithHooks {
                            execution: ToolExecution {
                                output: json!({
                                    "error": format!("PreToolUse hook permission denied for '{tool_name}': {reason}"),
                                    "retry": retry_message,
                                }),
                                is_error: true,
                            },
                            hook_summary,
                        };
                    }
                }
            }
        }
    }

    let tool_timeout_ms = match execution_tool_timeout_ms(tool_name, &arguments) {
        Ok(timeout_ms) => timeout_ms,
        Err(error) => {
            return ToolExecutionWithHooks {
                execution: ToolExecution {
                    output: json!({ "error": error }),
                    is_error: true,
                },
                hook_summary,
            };
        }
    };
    let tool_deadline =
        tool_timeout_ms.map(|timeout_ms| Instant::now() + Duration::from_millis(timeout_ms));
    let resource_lock_request = PendingToolCall {
        id: tool_call_id.to_string(),
        name: tool_name.to_string(),
        arguments: arguments.clone(),
    };
    let resource_locks = match tool_resource_locks(&resource_lock_request) {
        Ok(locks) => locks,
        Err(error) => {
            return ToolExecutionWithHooks {
                execution: ToolExecution {
                    output: json!({ "error": error.to_string() }),
                    is_error: true,
                },
                hook_summary,
            };
        }
    };
    let resource_lock_owner =
        tool_resource_lock_owner(agent_tool_context.as_ref(), tool_call_id, tool_name);
    let _resource_lease = match wait_for_tool_resource_lock(
        &tool_resource_lock_registry,
        workspace_id,
        resource_locks,
        tool_name,
        tool_timeout_ms,
        tool_deadline,
        cancellation_token.clone(),
        resource_lock_owner,
    )
    .await
    {
        Ok(lease) => lease,
        Err(error) => {
            return ToolExecutionWithHooks {
                execution: ToolExecution {
                    output: json!({ "error": error }),
                    is_error: true,
                },
                hook_summary,
            };
        }
    };
    if cancellation_token.is_cancelled() {
        return cancelled_tool_execution_with_hooks(hook_summary);
    }

    if is_agent_tool_name(tool_name) {
        let Some(agent_tool_context) = agent_tool_context else {
            return ToolExecutionWithHooks {
                execution: ToolExecution {
                    output: json!({ "error": format!("Agent tool '{tool_name}' requires an active Agent team run") }),
                    is_error: true,
                },
                hook_summary,
            };
        };
        let timeout_ms = tool_timeout_ms.expect("Agent tools must use timeoutMs");
        let remaining_timeout = tool_deadline
            .and_then(remaining_duration_until)
            .unwrap_or(Duration::ZERO);
        set_tool_timeout_ms(&mut arguments, remaining_timeout);
        let tool_name = tool_name.to_string();
        let worker_tool_name = tool_name.clone();
        let worker_tool_call_id = tool_call_id.to_string();
        let workspace_path = workspace_path.to_path_buf();
        let worker = tokio::task::spawn_blocking(move || {
            execute_agent_tool(
                &agent_tool_context,
                &workspace_path,
                &worker_tool_name,
                &worker_tool_call_id,
                arguments,
            )
        });
        let execution = timeout(remaining_timeout, worker)
            .await
            .map_err(|_| format!("tool '{tool_name}' timed out after {timeout_ms} ms"))
            .and_then(|result| {
                result.map_err(|source| format!("tool execution worker failed: {source}"))
            });
        let execution = match execution {
            Ok(Ok(output)) => ToolExecution {
                output,
                is_error: false,
            },
            Ok(Err(error)) | Err(error) => ToolExecution {
                output: agent_tool_error_output(&error),
                is_error: true,
            },
        };
        return ToolExecutionWithHooks {
            execution,
            hook_summary,
        };
    }

    if tool_name == ASK_QUESTION_TOOL {
        let ask_question = execute_ask_question(
            hook_runtime,
            global_hooks,
            api_audit_save_details,
            provider_config,
            question_registry,
            question_event_tx,
            workspace_id,
            workspace_path,
            chat_id,
            run_id,
            model_id,
            provider_id,
            llm_request_retry_count,
            tool_call_id,
            arguments,
            cancellation_token.clone(),
        )
        .await;
        merge_hook_summaries(&mut hook_summary, ask_question.hook_summary);
        return ToolExecutionWithHooks {
            execution: ask_question.execution,
            hook_summary,
        };
    }

    if is_memory_tool_name(tool_name) {
        let timeout_ms = tool_timeout_ms.expect("memory tools must use timeoutMs");
        let remaining_timeout = tool_deadline
            .and_then(remaining_duration_until)
            .unwrap_or(Duration::ZERO);
        set_tool_timeout_ms(&mut arguments, remaining_timeout);
        let tool_name = tool_name.to_string();
        let worker_tool_name = tool_name.clone();
        let worker_cancellation_token = cancellation_token.clone();
        let worker = tokio::task::spawn_blocking(move || {
            if worker_cancellation_token.is_cancelled() {
                return Err("tool execution cancelled".to_string());
            }
            execute_memory_tool(&memory_tool_context, &worker_tool_name, arguments)
        });
        let execution = timeout(remaining_timeout, worker)
            .await
            .map_err(|_| format!("tool '{tool_name}' timed out after {timeout_ms} ms"))
            .and_then(|result| {
                result.map_err(|source| format!("tool execution worker failed: {source}"))
            });
        let execution = match execution {
            Ok(Ok(output)) => ToolExecution {
                output,
                is_error: false,
            },
            Ok(Err(error)) | Err(error) => ToolExecution {
                output: json!({ "error": error }),
                is_error: true,
            },
        };

        return ToolExecutionWithHooks {
            execution,
            hook_summary,
        };
    }

    if is_web_tool_name(tool_name) {
        let remaining_timeout = tool_deadline
            .and_then(remaining_duration_until)
            .unwrap_or(Duration::ZERO);
        set_tool_timeout_ms(&mut arguments, remaining_timeout);
        let execution = tokio::select! {
            _ = cancellation_token_cancelled(cancellation_token.clone()) => {
                Err("tool execution cancelled".to_string())
            }
            execution = execute_web_tool(web_search_settings, tool_name, arguments, remaining_timeout) => execution,
        };
        let execution = match execution {
            Ok(output) => ToolExecution {
                output,
                is_error: false,
            },
            Err(error) => ToolExecution {
                output: json!({ "error": error }),
                is_error: true,
            },
        };

        return ToolExecutionWithHooks {
            execution,
            hook_summary,
        };
    }

    let execution = if is_mcp_tool_name(tool_name) {
        let tool_future = mcp_registry.execute_tool(workspace_id, tool_name, arguments);
        match tokio::select! {
            _ = cancellation_token_cancelled(cancellation_token.clone()) => {
                Err("tool execution cancelled".to_string())
            }
            execution = tool_future => {
                execution.map_err(|error| error.to_string())
            }
        } {
            Ok(execution) => ToolExecution {
                output: execution.output,
                is_error: execution.is_error,
            },
            Err(error) => ToolExecution {
                output: json!({ "error": error.to_string() }),
                is_error: true,
            },
        }
    } else {
        let timeout_ms = tool_timeout_ms.expect("built-in tools must use timeoutMs");
        let remaining_timeout = tool_deadline
            .and_then(remaining_duration_until)
            .unwrap_or(Duration::ZERO);
        set_tool_timeout_ms(&mut arguments, remaining_timeout);
        let tool_name = tool_name.to_string();
        let worker = tokio::task::spawn_blocking({
            let workspace_path = tool_workspace_path.to_path_buf();
            let chat_id = chat_id.to_string();
            let assistant_message_id = assistant_message_id.to_string();
            let tool_call_id = tool_call_id.to_string();
            let tool_name = tool_name.clone();
            let cancellation_token = cancellation_token.clone();
            move || {
                execute_builtin_tool_for_chat_with_cancellation_and_output_sink(
                    &workspace_path,
                    Some(&chat_id),
                    &tool_name,
                    arguments,
                    Some(cancellation_token),
                    if tool_name == RUN_COMMAND_TOOL {
                        Some(Arc::new(ToolOutputDeltaSink {
                            assistant_message_id: assistant_message_id.clone(),
                            tool_call_id: tool_call_id.clone(),
                            tx: tool_output_delta_tx,
                        }) as Arc<dyn ToolOutputSink>)
                    } else {
                        None
                    },
                )
            }
        });
        let execution: Result<ToolExecution, String> = tokio::select! {
            _ = cancellation_token_cancelled(cancellation_token.clone()) => {
                Err("tool execution cancelled".to_string())
            }
            execution = wait_for_builtin_tool_worker(worker, &tool_name, timeout_ms, remaining_timeout) => execution,
        };

        match execution {
            Ok(execution) => execution,
            Err(error) => ToolExecution {
                output: json!({ "error": error }),
                is_error: true,
            },
        }
    };

    ToolExecutionWithHooks {
        execution,
        hook_summary,
    }
}

pub(crate) fn is_agent_tool_name(tool_name: &str) -> bool {
    matches!(
        tool_name,
        AGENT_LIST_TOOL
            | AGENT_GET_TASK_TOOL
            | AGENT_SEND_MESSAGE_TOOL
            | AGENT_DELEGATE_TASK_TOOL
            | AGENT_CANCEL_TASK_TOOL
            | AGENT_WAIT_TASKS_TOOL
            | AGENT_TRANSFER_TASK_TOOL
            | AGENT_CREATE_INSTANCES_TOOL
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentListInput {
    #[serde(rename = "timeoutMs")]
    _timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentGetTaskInput {
    task_id: AgentTaskId,
    #[serde(rename = "timeoutMs")]
    _timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentSendMessageInput {
    receiver_instance_id: AgentInstanceId,
    kind: AgentMessageKind,
    content: String,
    reply_to_message_id: Option<AgentMessageId>,
    related_task_id: Option<AgentTaskId>,
    #[serde(rename = "timeoutMs")]
    _timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentDelegateTaskInput {
    target_instance_id: Option<AgentInstanceId>,
    target_definition_id: Option<AgentDefinitionId>,
    input: Value,
    correlation_id: Option<String>,
    #[serde(rename = "timeoutMs")]
    _timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentCancelTaskInput {
    task_id: AgentTaskId,
    #[serde(rename = "timeoutMs")]
    _timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentWaitTasksInput {
    task_ids: Vec<AgentTaskId>,
    mode: AgentTaskWaitMode,
    deadline_ms: Option<u64>,
    #[serde(rename = "timeoutMs")]
    _timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentTransferTaskInput {
    task_id: AgentTaskId,
    target_instance_id: AgentInstanceId,
    #[serde(rename = "timeoutMs")]
    _timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentCreateInstancesInput {
    definition_id: AgentDefinitionId,
    count: u32,
    execution_workspace_mode: AgentExecutionWorkspaceMode,
    #[serde(rename = "timeoutMs")]
    _timeout_ms: Option<u64>,
}

fn execute_agent_tool(
    context: &AgentToolContext,
    _workspace_path: &Path,
    tool_name: &str,
    tool_call_id: &str,
    arguments: Value,
) -> Result<Value, String> {
    if !context.collaboration_tools_enabled {
        return Err(agent_tool_error(
            "permission_denied",
            format!("Agent tool '{tool_name}' is not enabled for this run"),
        ));
    }
    let workspace_path = context.workspace_path.as_path();
    match tool_name {
        AGENT_LIST_TOOL => execute_agent_list(context, workspace_path, arguments),
        AGENT_GET_TASK_TOOL => execute_agent_get_task(context, workspace_path, arguments),
        AGENT_SEND_MESSAGE_TOOL => execute_agent_send_message(context, workspace_path, arguments),
        AGENT_DELEGATE_TASK_TOOL => execute_agent_delegate_task(context, workspace_path, arguments),
        AGENT_CANCEL_TASK_TOOL => execute_agent_cancel_task(context, workspace_path, arguments),
        AGENT_WAIT_TASKS_TOOL => {
            execute_agent_wait_tasks(context, workspace_path, tool_call_id, arguments)
        }
        AGENT_TRANSFER_TASK_TOOL => execute_agent_transfer_task(context, workspace_path, arguments),
        AGENT_CREATE_INSTANCES_TOOL => {
            execute_agent_create_instances(context, workspace_path, arguments)
        }
        _ => Err(agent_tool_error(
            "unknown_tool",
            format!("unknown Agent tool '{tool_name}'"),
        )),
    }
}

fn execute_agent_list(
    context: &AgentToolContext,
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, String> {
    let _input = serde_json::from_value::<AgentListInput>(arguments).map_err(|source| {
        agent_tool_error(
            "invalid_arguments",
            format!("agent_list arguments do not match schema: {source}"),
        )
    })?;
    let team_id = agent_tool_team_id(context)?;
    let database = WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
    let team = database
        .agent_team(team_id)
        .map_err(agent_store_error)?
        .ok_or_else(|| {
            agent_tool_error("not_found", format!("Agent team '{team_id}' was not found"))
        })?;
    let instances = database
        .agent_instances_for_team(team_id)
        .map_err(agent_store_error)?;
    let tasks = database
        .agent_tasks_for_team(team_id)
        .map_err(agent_store_error)?;
    let workload = database
        .agent_team_workload(team_id)
        .map_err(agent_store_error)?;
    let definitions = instances
        .iter()
        .map(|instance| {
            let definition = &instance.definition_snapshot;
            json!({
                "id": definition.id.to_string(),
                "revision": definition.revision,
                "name": definition.name,
                "description": definition.description,
                "providerId": definition.provider_id,
                "modelId": definition.model_id,
                "allowedTools": definition.allowed_tools,
                "permissions": definition.permissions,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "team": {
            "id": team.id.to_string(),
            "chatId": team.chat_id,
            "status": team.status.as_str(),
            "coordinatorInstanceId": team.coordinator_instance_id.to_string(),
            "maxConcurrentRuns": team.max_concurrent_runs,
        },
        "definitions": definitions,
        "instances": instances.iter().map(agent_instance_value).collect::<Vec<_>>(),
        "queue": {
            "queued": workload.queued_tasks,
            "running": workload.running_tasks,
            "waiting": workload.waiting_tasks,
            "byInstance": agent_queue_by_instance(&instances, &tasks),
        }
    }))
}

fn execute_agent_get_task(
    context: &AgentToolContext,
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, String> {
    let input = serde_json::from_value::<AgentGetTaskInput>(arguments).map_err(|source| {
        agent_tool_error(
            "invalid_arguments",
            format!("agent_get_task arguments do not match schema: {source}"),
        )
    })?;
    let team_id = agent_tool_team_id(context)?;
    let database = WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
    let task = database
        .agent_task_for_team(team_id, &input.task_id)
        .map_err(agent_store_error)?
        .ok_or_else(|| {
            agent_tool_error(
                "not_found",
                format!(
                    "Agent task '{}' was not found in team '{team_id}'",
                    input.task_id
                ),
            )
        })?;
    authorize_agent_task_visibility(context, &task)?;
    Ok(agent_task_value(&task))
}

fn execute_agent_send_message(
    context: &AgentToolContext,
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, String> {
    let input = serde_json::from_value::<AgentSendMessageInput>(arguments).map_err(|source| {
        agent_tool_error(
            "invalid_arguments",
            format!("agent_send_message arguments do not match schema: {source}"),
        )
    })?;
    context
        .permissions
        .authorize_collaboration_tool(
            AgentCollaborationTool::SendMessage,
            agent_tool_instance_id(context)?.clone(),
        )
        .map_err(|source| agent_tool_error("permission_denied", source.to_string()))?;
    if input.content.trim().is_empty() {
        return Err(agent_tool_error(
            "invalid_arguments",
            "agent_send_message content must not be empty",
        ));
    }
    if input.content.chars().count() > AGENT_MAX_MESSAGE_CONTENT_CHARS {
        return Err(agent_tool_error(
            "payload_too_large",
            format!(
                "agent_send_message content exceeds {AGENT_MAX_MESSAGE_CONTENT_CHARS} characters"
            ),
        ));
    }
    let team_id = agent_tool_team_id(context)?;
    let sender_instance_id = agent_tool_instance_id(context)?;
    let task_id = agent_tool_task_id(context)?;
    if let Some(related_task_id) = &input.related_task_id {
        let database =
            WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
        let related = database
            .agent_task_for_team(team_id, related_task_id)
            .map_err(agent_store_error)?
            .ok_or_else(|| {
                agent_tool_error(
                    "not_found",
                    format!(
                        "related Agent task '{related_task_id}' was not found in team '{team_id}'"
                    ),
                )
            })?;
        authorize_agent_task_visibility(context, &related)?;
    }
    let message_id =
        AgentMessageId::new(unique_id("agent-message")).map_err(|source| source.to_string())?;
    let mut database =
        WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
    let message = database
        .insert_agent_message(NewAgentMessage {
            id: &message_id,
            team_id,
            sender_instance_id: Some(sender_instance_id),
            receiver_instance_id: &input.receiver_instance_id,
            related_task_id: input.related_task_id.as_ref(),
            reply_to_message_id: input.reply_to_message_id.as_ref(),
            kind: input.kind,
            content: input.content.trim(),
        })
        .map_err(agent_store_error)?;
    append_agent_tool_event(
        &mut database,
        team_id,
        "message_created",
        Some(sender_instance_id),
        Some(task_id),
        Some(&message.id),
        json!({
            "receiverInstanceId": message.receiver_instance_id.to_string(),
            "kind": message.kind.as_str(),
            "relatedTaskId": message.related_task_id.as_ref().map(ToString::to_string),
            "replyToMessageId": message.reply_to_message_id.as_ref().map(ToString::to_string),
        }),
    )?;
    Ok(json!({
        "messageId": message.id.to_string(),
        "receiverInstanceId": message.receiver_instance_id.to_string(),
        "kind": message.kind.as_str(),
        "sequence": message.sequence,
        "createdAt": message.created_at,
    }))
}

fn execute_agent_delegate_task(
    context: &AgentToolContext,
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, String> {
    let input = serde_json::from_value::<AgentDelegateTaskInput>(arguments).map_err(|source| {
        agent_tool_error(
            "invalid_arguments",
            format!("agent_delegate_task arguments do not match schema: {source}"),
        )
    })?;
    context
        .permissions
        .authorize_collaboration_tool(
            AgentCollaborationTool::DelegateTask,
            agent_tool_instance_id(context)?.clone(),
        )
        .map_err(|source| agent_tool_error("permission_denied", source.to_string()))?;
    let target_instance_id = select_delegate_target_instance(context, workspace_path, &input)?;
    let team_id = agent_tool_team_id(context)?;
    let origin_instance_id = agent_tool_instance_id(context)?;
    let parent_task_id = agent_tool_task_id(context)?;
    validate_agent_delegate_limits(workspace_path, team_id, parent_task_id, &input.input)?;
    let child_task_id =
        AgentTaskId::new(unique_id("agent-task")).map_err(|source| source.to_string())?;
    let child_input = json!({
        "queuedUserMessageId": format!("{}:{}", parent_task_id, child_task_id),
        "message": agent_delegate_task_message(&input.input, input.correlation_id.as_deref())?,
        "attachments": [],
        "skillIds": [],
        "collaborationToolsEnabled": true,
        "delegatedInput": input.input,
        "correlationId": input.correlation_id,
    });
    let input_json = child_input.to_string();
    let mut database =
        WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
    let child = database
        .enqueue_agent_task_with_limits(
            NewAgentTask {
                id: &child_task_id,
                team_id,
                owner_instance_id: &target_instance_id,
                origin_instance_id: Some(origin_instance_id),
                parent_task_id: Some(parent_task_id),
                input_json: &input_json,
            },
            i64::from(AGENT_MAX_QUEUED_TASKS_PER_TEAM),
            i64::from(AGENT_MAX_QUEUED_TASKS_PER_INSTANCE),
            i64::from(AGENT_MAX_QUEUED_TASKS_PER_CHAT),
        )
        .map_err(agent_store_error)?;
    append_agent_tool_event(
        &mut database,
        team_id,
        "task_delegated",
        Some(origin_instance_id),
        Some(parent_task_id),
        None,
        json!({
            "childTaskId": child.id.to_string(),
            "targetInstanceId": child.owner_instance_id.to_string(),
            "targetDefinitionId": input.target_definition_id.as_ref().map(ToString::to_string),
            "correlationId": input.correlation_id,
        }),
    )?;
    append_agent_tool_event(
        &mut database,
        team_id,
        "task_queued",
        Some(&child.owner_instance_id),
        Some(&child.id),
        None,
        json!({
            "originInstanceId": child.origin_instance_id.as_ref().map(ToString::to_string),
            "parentTaskId": child.parent_task_id.as_ref().map(ToString::to_string),
            "correlationId": input.correlation_id,
        }),
    )?;
    context.scheduler.wake().map_err(|source| source.message)?;
    Ok(json!({
        "taskId": child.id.to_string(),
        "targetInstanceId": child.owner_instance_id.to_string(),
        "status": child.status.as_str(),
        "sequence": child.sequence,
        "correlationId": input.correlation_id,
    }))
}

fn execute_agent_cancel_task(
    context: &AgentToolContext,
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, String> {
    let input = serde_json::from_value::<AgentCancelTaskInput>(arguments).map_err(|source| {
        agent_tool_error(
            "invalid_arguments",
            format!("agent_cancel_task arguments do not match schema: {source}"),
        )
    })?;
    context
        .permissions
        .authorize_collaboration_tool(
            AgentCollaborationTool::DelegateTask,
            agent_tool_instance_id(context)?.clone(),
        )
        .map_err(|source| agent_tool_error("permission_denied", source.to_string()))?;
    let team_id = agent_tool_team_id(context)?;
    let actor_instance_id = agent_tool_instance_id(context)?;
    let parent_task_id = agent_tool_task_id(context)?;
    let mut database =
        WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
    let task = database
        .agent_task_for_team(team_id, &input.task_id)
        .map_err(agent_store_error)?
        .ok_or_else(|| {
            agent_tool_error(
                "not_found",
                format!(
                    "Agent task '{}' was not found in team '{team_id}'",
                    input.task_id
                ),
            )
        })?;
    if task.parent_task_id.as_ref() != Some(parent_task_id)
        || task.origin_instance_id.as_ref() != Some(actor_instance_id)
    {
        return Err(agent_tool_error(
            "permission_denied",
            format!(
                "Agent task '{}' is not a child task delegated by the current task",
                task.id
            ),
        ));
    }
    if task.status != AgentTaskStatus::Queued {
        return Err(agent_tool_error(
            "invalid_task_status",
            format!(
                "Agent task '{}' cannot be cancelled by agent_cancel_task while {}",
                task.id,
                task.status.as_str()
            ),
        ));
    }
    let error = json!({
        "message": "cancelled by delegating Agent task",
        "cancelledByInstanceId": actor_instance_id.to_string(),
        "cancelledByTaskId": parent_task_id.to_string(),
    });
    let updated = database
        .cancel_queued_agent_task(team_id, &task.id, &error.to_string())
        .map_err(agent_store_error)?;
    if !updated {
        return Err(agent_tool_error(
            "state_changed",
            format!("Agent task '{}' changed state before cancellation", task.id),
        ));
    }
    append_agent_tool_event(
        &mut database,
        team_id,
        "task_cancelled",
        Some(actor_instance_id),
        Some(&task.id),
        None,
        error,
    )?;
    context.scheduler.wake().map_err(|source| source.message)?;
    Ok(json!({
        "taskId": task.id.to_string(),
        "status": AgentTaskStatus::Cancelled.as_str(),
    }))
}

fn execute_agent_wait_tasks(
    context: &AgentToolContext,
    workspace_path: &Path,
    tool_call_id: &str,
    arguments: Value,
) -> Result<Value, String> {
    let input = serde_json::from_value::<AgentWaitTasksInput>(arguments).map_err(|source| {
        agent_tool_error(
            "invalid_arguments",
            format!("agent_wait_tasks arguments do not match schema: {source}"),
        )
    })?;
    context
        .permissions
        .authorize_collaboration_tool(
            AgentCollaborationTool::WaitTasks,
            agent_tool_instance_id(context)?.clone(),
        )
        .map_err(|source| agent_tool_error("permission_denied", source.to_string()))?;
    if input.mode != AgentTaskWaitMode::All {
        return Err(agent_tool_error(
            "invalid_arguments",
            "agent_wait_tasks currently supports mode 'all' only",
        ));
    }
    if input.task_ids.is_empty() {
        return Err(agent_tool_error(
            "invalid_arguments",
            "agent_wait_tasks taskIds must not be empty",
        ));
    }
    if input.task_ids.len() > AGENT_MAX_CHILD_TASKS_PER_TASK {
        return Err(agent_tool_error(
            "limit_exceeded",
            format!("agent_wait_tasks taskIds exceeds {AGENT_MAX_CHILD_TASKS_PER_TASK} tasks"),
        ));
    }

    let team_id = agent_tool_team_id(context)?;
    let actor_instance_id = agent_tool_instance_id(context)?;
    let current_task_id = agent_tool_task_id(context)?;
    let deadline_at = input
        .deadline_ms
        .map(agent_wait_deadline_timestamp)
        .transpose()?;
    let mut seen = HashSet::new();
    let mut database =
        WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
    let current_task = database
        .agent_task_for_team(team_id, current_task_id)
        .map_err(agent_store_error)?
        .ok_or_else(|| {
            agent_tool_error(
                "not_found",
                format!("current Agent task '{current_task_id}' was not found"),
            )
        })?;
    let mut dependencies = Vec::with_capacity(input.task_ids.len());
    for dependency_task_id in &input.task_ids {
        if dependency_task_id == current_task_id {
            return Err(agent_tool_error(
                "dependency_cycle",
                format!("Agent task '{current_task_id}' cannot wait on itself"),
            ));
        }
        if !seen.insert(dependency_task_id.as_str().to_string()) {
            return Err(agent_tool_error(
                "invalid_arguments",
                format!("duplicate dependency task id '{dependency_task_id}'"),
            ));
        }
        let dependency_task = database
            .agent_task_for_team(team_id, dependency_task_id)
            .map_err(agent_store_error)?
            .ok_or_else(|| {
                agent_tool_error(
                    "not_found",
                    format!(
                        "Agent dependency task '{dependency_task_id}' was not found in team '{team_id}'"
                    ),
                )
            })?;
        authorize_agent_task_visibility(context, &dependency_task)?;
        if dependency_task.owner_instance_id == *actor_instance_id
            && dependency_task.sequence > current_task.sequence
            && dependency_task.status.holds_queue_head()
        {
            return Err(agent_tool_error(
                "queue_deadlock",
                format!(
                    "Agent task '{current_task_id}' cannot wait on later queued task '{}' in the same instance queue",
                    dependency_task.id
                ),
            ));
        }
        dependencies.push(dependency_task);
    }

    for dependency_task in &dependencies {
        database
            .insert_agent_task_dependency(NewAgentTaskDependency {
                team_id,
                waiting_task_id: current_task_id,
                dependency_task_id: &dependency_task.id,
                wait_mode: AgentTaskWaitMode::All,
                pending_tool_call_id: Some(tool_call_id),
                deadline_at: deadline_at.as_deref(),
            })
            .map_err(agent_store_error)?;
    }
    append_agent_tool_event(
        &mut database,
        team_id,
        "task_waiting_requested",
        Some(actor_instance_id),
        Some(current_task_id),
        None,
        json!({
            "pendingToolCallId": tool_call_id,
            "dependencyTaskIds": dependencies.iter().map(|task| task.id.to_string()).collect::<Vec<_>>(),
            "mode": AgentTaskWaitMode::All.as_str(),
            "deadlineAt": deadline_at,
        }),
    )?;
    Ok(json!({
        "waiting": true,
        "taskId": current_task_id.to_string(),
        "mode": AgentTaskWaitMode::All.as_str(),
        "taskIds": dependencies.iter().map(|task| task.id.to_string()).collect::<Vec<_>>(),
        "deadlineAt": deadline_at,
        "suspend": {
            "kind": "agent_wait_tasks",
            "pendingToolCallId": tool_call_id,
            "taskIds": dependencies.iter().map(|task| task.id.to_string()).collect::<Vec<_>>(),
            "mode": AgentTaskWaitMode::All.as_str(),
            "deadlineAt": deadline_at,
        }
    }))
}

fn execute_agent_transfer_task(
    context: &AgentToolContext,
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, String> {
    let input = serde_json::from_value::<AgentTransferTaskInput>(arguments).map_err(|source| {
        agent_tool_error(
            "invalid_arguments",
            format!("agent_transfer_task arguments do not match schema: {source}"),
        )
    })?;
    context
        .permissions
        .authorize_collaboration_tool(
            AgentCollaborationTool::TransferTask,
            agent_tool_instance_id(context)?.clone(),
        )
        .map_err(|source| agent_tool_error("permission_denied", source.to_string()))?;
    let team_id = agent_tool_team_id(context)?;
    let actor_instance_id = agent_tool_instance_id(context)?;
    let current_task_id = agent_tool_task_id(context)?;
    let mut database =
        WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
    let task = database
        .agent_task_for_team(team_id, &input.task_id)
        .map_err(agent_store_error)?
        .ok_or_else(|| {
            agent_tool_error(
                "not_found",
                format!(
                    "Agent task '{}' was not found in team '{team_id}'",
                    input.task_id
                ),
            )
        })?;
    authorize_agent_task_visibility(context, &task)?;
    if task.status != AgentTaskStatus::Queued {
        return Err(agent_tool_error(
            "invalid_task_status",
            format!(
                "Agent task '{}' cannot be transferred while {}",
                task.id,
                task.status.as_str()
            ),
        ));
    }
    let target = database
        .agent_instance(&input.target_instance_id)
        .map_err(agent_store_error)?
        .ok_or_else(|| {
            agent_tool_error(
                "not_found",
                format!(
                    "Agent target instance '{}' was not found",
                    input.target_instance_id
                ),
            )
        })?;
    if target.team_id != *team_id {
        return Err(agent_tool_error(
            "cross_team_reference",
            format!(
                "Agent target instance '{}' does not belong to team '{team_id}'",
                input.target_instance_id
            ),
        ));
    }
    let transferred = database
        .transfer_queued_agent_task_with_limits(
            team_id,
            &task.id,
            &target.id,
            i64::from(AGENT_MAX_QUEUED_TASKS_PER_TEAM),
            i64::from(AGENT_MAX_QUEUED_TASKS_PER_INSTANCE),
            i64::from(AGENT_MAX_QUEUED_TASKS_PER_CHAT),
        )
        .map_err(agent_store_error)?
        .ok_or_else(|| {
            agent_tool_error(
                "state_changed",
                format!("Agent task '{}' changed state before transfer", task.id),
            )
        })?;
    append_agent_tool_event(
        &mut database,
        team_id,
        "task_transferred",
        Some(actor_instance_id),
        Some(current_task_id),
        None,
        json!({
            "taskId": transferred.id.to_string(),
            "previousOwnerInstanceId": task.owner_instance_id.to_string(),
            "targetInstanceId": transferred.owner_instance_id.to_string(),
            "sequence": transferred.sequence,
        }),
    )?;
    context.scheduler.wake().map_err(|source| source.message)?;
    Ok(json!({
        "taskId": transferred.id.to_string(),
        "previousOwnerInstanceId": task.owner_instance_id.to_string(),
        "targetInstanceId": transferred.owner_instance_id.to_string(),
        "status": transferred.status.as_str(),
        "sequence": transferred.sequence,
    }))
}

fn execute_agent_create_instances(
    context: &AgentToolContext,
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, String> {
    let input =
        serde_json::from_value::<AgentCreateInstancesInput>(arguments).map_err(|source| {
            agent_tool_error(
                "invalid_arguments",
                format!("agent_create_instances arguments do not match schema: {source}"),
            )
        })?;
    context
        .permissions
        .authorize_instance_definition(
            &input.definition_id,
            agent_tool_instance_id(context)?.clone(),
        )
        .map_err(|source| agent_tool_error("permission_denied", source.to_string()))?;
    if input.count == 0 {
        return Err(agent_tool_error(
            "invalid_arguments",
            "agent_create_instances count must be greater than 0",
        ));
    }
    if input.count > AGENT_MAX_CREATE_INSTANCES_PER_REQUEST {
        return Err(agent_tool_error(
            "limit_exceeded",
            format!(
                "agent_create_instances count exceeds process limit {AGENT_MAX_CREATE_INSTANCES_PER_REQUEST}"
            ),
        ));
    }
    let definition = context
        .agent_definitions
        .iter()
        .find(|definition| definition.id == input.definition_id)
        .ok_or_else(|| {
            agent_tool_error(
                "not_found",
                format!("Agent definition '{}' was not found", input.definition_id),
            )
        })?;
    let team_id = agent_tool_team_id(context)?;
    let actor_instance_id = agent_tool_instance_id(context)?;
    let task_id = agent_tool_task_id(context)?;
    let mut database =
        WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
    validate_agent_create_instance_capacity(&database, team_id, definition, input.count)?;
    if !definition
        .allowed_execution_workspace_modes
        .contains(&input.execution_workspace_mode)
    {
        return Err(agent_tool_error(
            "permission_denied",
            format!(
                "executionWorkspaceMode '{}' is not allowed for Agent definition '{}'",
                input.execution_workspace_mode.as_str(),
                definition.id
            ),
        ));
    }
    let instance_ids = (0..input.count)
        .map(|_| {
            AgentInstanceId::new(unique_id("agent-instance")).map_err(|source| source.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let worktrees = match input.execution_workspace_mode {
        AgentExecutionWorkspaceMode::Shared => Vec::new(),
        AgentExecutionWorkspaceMode::IsolatedWorktree => instance_ids
            .iter()
            .map(|id| {
                create_agent_worktree(workspace_path, id.as_str())
                    .map_err(|source| agent_tool_error("worktree_error", source.message))
            })
            .collect::<Result<Vec<_>, _>>()?,
    };
    let worktree_root_paths = worktrees
        .iter()
        .map(|worktree| worktree.root_path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let new_instances = instance_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let worktree = worktrees.get(index);
            let worktree_root_path = worktree_root_paths.get(index);
            NewAgentInstance {
                id,
                team_id,
                definition,
                role: foco_agent::AgentRole::Worker,
                execution_workspace_mode: input.execution_workspace_mode,
                execution_root_path: worktree_root_path.map(String::as_str),
                worktree_base_revision: worktree.map(|worktree| worktree.base_revision.as_str()),
                worktree_branch: worktree.map(|worktree| worktree.branch.as_str()),
                worktree_status: worktree.map(|_| "active"),
            }
        })
        .collect::<Vec<_>>();
    let created = match database.create_agent_instances_with_limits(
        &new_instances,
        AGENT_MAX_INSTANCES_PER_TEAM,
        i64::from(definition.max_instances),
    ) {
        Ok(created) => created,
        Err(error) => {
            for worktree in &worktrees {
                let _ = delete_agent_worktree(workspace_path, &worktree.root_path, true);
            }
            return Err(agent_store_error(error));
        }
    };
    for instance in &created {
        append_agent_tool_event(
            &mut database,
            team_id,
            "instance_created",
            Some(&instance.id),
            Some(task_id),
            None,
            json!({
                "createdByInstanceId": actor_instance_id.to_string(),
                "definitionId": instance.definition_id.to_string(),
                "definitionRevision": instance.definition_revision,
                "role": instance.role.as_str(),
                "status": instance.status.as_str(),
                "executionWorkspaceMode": instance.execution_workspace_mode.as_str(),
                "executionRootPath": instance.execution_root_path,
                "worktreeBaseRevision": instance.worktree_base_revision,
                "worktreeBranch": instance.worktree_branch,
                "worktreeStatus": instance.worktree_status,
            }),
        )?;
    }
    Ok(json!({
        "instances": created.iter().map(agent_instance_value).collect::<Vec<_>>(),
        "definitionId": input.definition_id.to_string(),
        "definitionRevision": definition.revision,
        "count": created.len(),
    }))
}

fn validate_agent_create_instance_capacity(
    database: &WorkspaceDatabase,
    team_id: &foco_agent::AgentTeamId,
    definition: &AgentDefinitionSettings,
    count: u32,
) -> Result<(), String> {
    let instances = database
        .agent_instances_for_team(team_id)
        .map_err(agent_store_error)?;
    let current_team_instances = i64::try_from(instances.len()).map_err(|_| {
        agent_tool_error(
            "limit_exceeded",
            "agent_create_instances team instance count exceeds integer range",
        )
    })?;
    let current_definition_instances = i64::try_from(
        instances
            .iter()
            .filter(|instance| instance.definition_id == definition.id)
            .count(),
    )
    .map_err(|_| {
        agent_tool_error(
            "limit_exceeded",
            "agent_create_instances definition instance count exceeds integer range",
        )
    })?;
    let requested = i64::from(count);
    let remaining_team_slots = (AGENT_MAX_INSTANCES_PER_TEAM - current_team_instances).max(0);
    if requested > remaining_team_slots {
        return Err(agent_tool_error(
            "limit_exceeded",
            format!(
                "agent_create_instances count {count} exceeds team capacity: currentTeamInstances={current_team_instances}, maxInstancesPerTeam={AGENT_MAX_INSTANCES_PER_TEAM}, remainingTeamSlots={remaining_team_slots}"
            ),
        ));
    }
    let max_definition_instances = i64::from(definition.max_instances);
    let remaining_definition_slots =
        (max_definition_instances - current_definition_instances).max(0);
    if requested > remaining_definition_slots {
        return Err(agent_tool_error(
            "limit_exceeded",
            format!(
                "agent_create_instances count {count} exceeds definition capacity: definitionId={}, currentTeamDefinitionInstances={current_definition_instances}, maxInstancesForDefinition={max_definition_instances}, remainingTeamDefinitionSlots={remaining_definition_slots}",
                definition.id
            ),
        ));
    }
    Ok(())
}

fn agent_wait_deadline_timestamp(deadline_ms: u64) -> Result<String, String> {
    let millis = i64::try_from(deadline_ms).map_err(|_| {
        agent_tool_error(
            "invalid_arguments",
            "agent_wait_tasks deadlineMs is too large",
        )
    })?;
    Ok((Utc::now() + ChronoDuration::milliseconds(millis))
        .to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn agent_tool_team_id(context: &AgentToolContext) -> Result<&foco_agent::AgentTeamId, String> {
    context
        .associations
        .team_id
        .as_ref()
        .ok_or_else(|| "Agent tool requires a team association".to_string())
}

fn agent_tool_instance_id(context: &AgentToolContext) -> Result<&AgentInstanceId, String> {
    context
        .associations
        .instance_id
        .as_ref()
        .ok_or_else(|| "Agent tool requires an instance association".to_string())
}

fn agent_tool_task_id(context: &AgentToolContext) -> Result<&AgentTaskId, String> {
    context
        .associations
        .task_id
        .as_ref()
        .ok_or_else(|| "Agent tool requires a task association".to_string())
}

fn select_delegate_target_instance(
    context: &AgentToolContext,
    workspace_path: &Path,
    input: &AgentDelegateTaskInput,
) -> Result<AgentInstanceId, String> {
    match (&input.target_instance_id, &input.target_definition_id) {
        (Some(_), Some(_)) | (None, None) => {
            return Err(agent_tool_error(
                "invalid_arguments",
                "provide exactly one of targetInstanceId or targetDefinitionId",
            ));
        }
        (Some(instance_id), None) => {
            let database =
                WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
            let instance = database
                .agent_instance(instance_id)
                .map_err(agent_store_error)?
                .ok_or_else(|| {
                    agent_tool_error(
                        "not_found",
                        format!("Agent instance '{instance_id}' was not found"),
                    )
                })?;
            if instance.team_id != *agent_tool_team_id(context)? {
                return Err(agent_tool_error(
                    "cross_team_reference",
                    format!(
                        "Agent instance '{instance_id}' does not belong to team '{}'",
                        agent_tool_team_id(context)?
                    ),
                ));
            }
            Ok(instance.id)
        }
        (None, Some(definition_id)) => {
            if !context
                .permissions
                .allowed_agent_definition_ids
                .iter()
                .any(|allowed_id| allowed_id == definition_id)
            {
                return Err(agent_tool_error(
                    "permission_denied",
                    format!(
                        "Agent definition '{definition_id}' is not allowed for delegation by this Agent"
                    ),
                ));
            }
            let database =
                WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
            let instance = database
                .route_agent_instance_for_definition(agent_tool_team_id(context)?, definition_id)
                .map_err(agent_store_error)?;
            let instance = instance.ok_or_else(|| {
                    agent_tool_error(
                        "not_found",
                        format!(
                            "Agent definition '{definition_id}' has no existing runnable instance in team '{}'",
                            agent_tool_team_id(context).map(ToString::to_string).unwrap_or_default()
                        ),
                    )
                })?;
            Ok(instance.id)
        }
    }
}

fn authorize_agent_task_visibility(
    context: &AgentToolContext,
    task: &AgentTaskRecord,
) -> Result<(), String> {
    let instance_id = agent_tool_instance_id(context)?;
    let current_task_id = agent_tool_task_id(context)?;
    if &task.owner_instance_id == instance_id
        || task.origin_instance_id.as_ref() == Some(instance_id)
        || &task.id == current_task_id
        || task.parent_task_id.as_ref() == Some(current_task_id)
    {
        Ok(())
    } else {
        Err(agent_tool_error(
            "permission_denied",
            format!(
                "Agent task '{}' is not visible to instance '{}'",
                task.id, instance_id
            ),
        ))
    }
}

fn validate_agent_delegate_limits(
    workspace_path: &Path,
    team_id: &foco_agent::AgentTeamId,
    parent_task_id: &AgentTaskId,
    input: &Value,
) -> Result<(), String> {
    let input_json = serde_json::to_string(input).map_err(|source| {
        agent_tool_error(
            "invalid_arguments",
            format!("failed to serialize delegated task input: {source}"),
        )
    })?;
    if input_json.len() > AGENT_MAX_TASK_INPUT_BYTES {
        return Err(agent_tool_error(
            "payload_too_large",
            format!("agent_delegate_task input exceeds {AGENT_MAX_TASK_INPUT_BYTES} bytes"),
        ));
    }
    let database = WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
    let child_count = database
        .agent_tasks_for_parent(team_id, parent_task_id)
        .map_err(agent_store_error)?
        .len();
    if child_count >= AGENT_MAX_CHILD_TASKS_PER_TASK {
        return Err(agent_tool_error(
            "limit_exceeded",
            format!(
                "Agent task '{parent_task_id}' already has {child_count} child tasks; limit is {AGENT_MAX_CHILD_TASKS_PER_TASK}"
            ),
        ));
    }
    let depth = agent_task_depth(&database, team_id, parent_task_id)?;
    if depth >= AGENT_MAX_DELEGATION_DEPTH {
        return Err(agent_tool_error(
            "limit_exceeded",
            format!(
                "Agent task '{parent_task_id}' delegation depth {depth} reached limit {AGENT_MAX_DELEGATION_DEPTH}"
            ),
        ));
    }
    Ok(())
}

fn agent_task_depth(
    database: &WorkspaceDatabase,
    team_id: &foco_agent::AgentTeamId,
    task_id: &AgentTaskId,
) -> Result<usize, String> {
    let mut depth = 0usize;
    let mut current_task_id = task_id.clone();
    loop {
        let task = database
            .agent_task_for_team(team_id, &current_task_id)
            .map_err(agent_store_error)?
            .ok_or_else(|| {
                agent_tool_error(
                    "not_found",
                    format!("Agent task '{current_task_id}' was not found in team '{team_id}'"),
                )
            })?;
        let Some(parent_task_id) = task.parent_task_id else {
            return Ok(depth);
        };
        depth = depth.saturating_add(1);
        if depth > AGENT_MAX_DELEGATION_DEPTH {
            return Ok(depth);
        }
        current_task_id = parent_task_id;
    }
}

fn agent_instance_value(instance: &AgentInstanceRecord) -> Value {
    json!({
        "id": instance.id.to_string(),
        "definitionId": instance.definition_id.to_string(),
        "definitionRevision": instance.definition_revision,
        "role": instance.role.as_str(),
        "status": instance.status.as_str(),
        "nextTaskSequence": instance.next_task_sequence,
        "nextMessageSequence": instance.next_message_sequence,
        "contextGeneration": instance.context_generation,
        "lastScheduledAt": instance.last_scheduled_at,
    })
}

fn agent_task_value(task: &AgentTaskRecord) -> Value {
    json!({
        "id": task.id.to_string(),
        "teamId": task.team_id.to_string(),
        "ownerInstanceId": task.owner_instance_id.to_string(),
        "originInstanceId": task.origin_instance_id.as_ref().map(ToString::to_string),
        "parentTaskId": task.parent_task_id.as_ref().map(ToString::to_string),
        "sequence": task.sequence,
        "status": task.status.as_str(),
        "result": task.result_json.as_deref().and_then(|value| serde_json::from_str::<Value>(value).ok()),
        "error": task.error_json.as_deref().and_then(|value| serde_json::from_str::<Value>(value).ok()),
        "createdAt": task.created_at,
        "updatedAt": task.updated_at,
        "startedAt": task.started_at,
        "completedAt": task.completed_at,
    })
}

fn agent_queue_by_instance(
    instances: &[AgentInstanceRecord],
    tasks: &[AgentTaskRecord],
) -> Vec<Value> {
    instances
        .iter()
        .map(|instance| {
            let queued = tasks
                .iter()
                .filter(|task| {
                    task.owner_instance_id == instance.id && task.status == AgentTaskStatus::Queued
                })
                .count();
            let running = tasks
                .iter()
                .filter(|task| {
                    task.owner_instance_id == instance.id && task.status == AgentTaskStatus::Running
                })
                .count();
            let waiting = tasks
                .iter()
                .filter(|task| {
                    task.owner_instance_id == instance.id && task.status == AgentTaskStatus::Waiting
                })
                .count();
            json!({
                "instanceId": instance.id.to_string(),
                "queued": queued,
                "running": running,
                "waiting": waiting,
            })
        })
        .collect()
}

fn agent_delegate_task_message(
    input: &Value,
    correlation_id: Option<&str>,
) -> Result<String, String> {
    if let Some(message) = input.get("message").and_then(Value::as_str) {
        if !message.trim().is_empty() {
            return Ok(message.trim().to_string());
        }
    }
    let input_json = serde_json::to_string(input)
        .map_err(|source| format!("failed to serialize delegated task input: {source}"))?;
    Ok(match correlation_id {
        Some(correlation_id) => format!("Delegated Agent task {correlation_id}: {input_json}"),
        None => format!("Delegated Agent task: {input_json}"),
    })
}

fn append_agent_tool_event(
    database: &mut WorkspaceDatabase,
    team_id: &foco_agent::AgentTeamId,
    event_type: &'static str,
    instance_id: Option<&AgentInstanceId>,
    task_id: Option<&AgentTaskId>,
    message_id: Option<&AgentMessageId>,
    payload: Value,
) -> Result<(), String> {
    database
        .append_agent_event(NewAgentEvent {
            team_id,
            event_type,
            instance_id,
            task_id,
            attempt_id: None,
            message_id,
            payload_json: &payload.to_string(),
        })
        .map(|_| ())
        .map_err(agent_store_error)
}

fn agent_store_error(error: foco_store::workspace::WorkspaceDatabaseError) -> String {
    agent_tool_error("store_error", error.to_string())
}

fn agent_tool_error(code: &'static str, message: impl Into<String>) -> String {
    format!("{code}: {}", message.into())
}

fn agent_tool_error_output(error: &str) -> Value {
    let (code, message) = error
        .split_once(": ")
        .map(|(code, message)| (code, message))
        .unwrap_or(("agent_tool_error", error));
    json!({ "code": code, "error": message })
}

fn execution_tool_timeout_ms(tool_name: &str, arguments: &Value) -> Result<Option<u64>, String> {
    if tool_name == ASK_QUESTION_TOOL {
        Ok(None)
    } else if is_memory_tool_name(tool_name) {
        memory_tool_timeout_ms(arguments).map(Some)
    } else if is_web_tool_name(tool_name) {
        web_tool_timeout_ms(arguments).map(Some)
    } else if is_mcp_tool_name(tool_name) {
        Ok(None)
    } else {
        builtin_tool_timeout_ms(tool_name, arguments).map(Some)
    }
}

pub(crate) async fn wait_for_tool_resource_lock(
    registry: &ToolResourceLockRegistry,
    workspace_id: &str,
    resource_locks: Vec<ToolResourceLock>,
    tool_name: &str,
    timeout_ms: Option<u64>,
    deadline: Option<Instant>,
    cancellation_token: ToolCancellationToken,
    owner: ToolResourceLockOwner,
) -> Result<ToolResourceLease, String> {
    let acquire = registry.acquire_with_owner(workspace_id, resource_locks.clone(), owner);
    match (timeout_ms, deadline.and_then(remaining_duration_until)) {
        (Some(timeout_ms), Some(remaining)) => {
            tokio::select! {
                _ = cancellation_token_cancelled(cancellation_token) => {
                    Err("tool execution cancelled".to_string())
                }
                lease = timeout(remaining, acquire) => {
                    lease.map_err(|_| resource_lock_timeout_error(registry, workspace_id, &resource_locks, tool_name, timeout_ms))
                }
            }
        }
        (Some(timeout_ms), None) => Err(resource_lock_timeout_error(
            registry,
            workspace_id,
            &resource_locks,
            tool_name,
            timeout_ms,
        )),
        (None, _) => {
            tokio::select! {
                _ = cancellation_token_cancelled(cancellation_token) => {
                    Err("tool execution cancelled".to_string())
                }
                lease = acquire => Ok(lease),
            }
        }
    }
}

fn tool_resource_lock_owner(
    agent_tool_context: Option<&AgentToolContext>,
    tool_call_id: &str,
    tool_name: &str,
) -> ToolResourceLockOwner {
    let associations = agent_tool_context.map(|context| &context.associations);
    ToolResourceLockOwner {
        instance_id: associations
            .and_then(|associations| associations.instance_id.as_ref())
            .map(ToString::to_string),
        task_id: associations
            .and_then(|associations| associations.task_id.as_ref())
            .map(ToString::to_string),
        tool_call_id: Some(tool_call_id.to_string()),
        tool_name: Some(tool_name.to_string()),
    }
}

fn resource_lock_timeout_error(
    registry: &ToolResourceLockRegistry,
    workspace_id: &str,
    resource_locks: &[ToolResourceLock],
    tool_name: &str,
    timeout_ms: u64,
) -> String {
    let blockers = registry.blocking_owners(workspace_id, resource_locks);
    if blockers.is_empty() {
        return format!(
            "tool '{tool_name}' timed out waiting for resource lock after {timeout_ms} ms"
        );
    }

    let blockers = blockers
        .into_iter()
        .map(|blocker| {
            let owner = blocker.owner;
            format!(
                "toolCallId={}, toolName={}, instanceId={}, taskId={}, activeMs={}, waitedBeforeAcquireMs={}",
                owner.tool_call_id.as_deref().unwrap_or("unknown"),
                owner.tool_name.as_deref().unwrap_or("unknown"),
                owner.instance_id.as_deref().unwrap_or("none"),
                owner.task_id.as_deref().unwrap_or("none"),
                blocker.active_ms,
                blocker.wait_ms,
            )
        })
        .collect::<Vec<_>>()
        .join("; ");

    format!(
        "tool '{tool_name}' timed out waiting for resource lock after {timeout_ms} ms; blocked by {blockers}"
    )
}

fn remaining_duration_until(deadline: Instant) -> Option<Duration> {
    deadline.checked_duration_since(Instant::now())
}

fn set_tool_timeout_ms(arguments: &mut Value, timeout: Duration) {
    if let Value::Object(map) = arguments {
        let timeout_ms = timeout.as_millis().min(u128::from(u64::MAX)) as u64;
        map.insert("timeoutMs".to_string(), json!(timeout_ms));
    }
}

fn cancelled_tool_execution() -> ToolExecutionWithHooks {
    cancelled_tool_execution_with_hooks(HookRunSummary::default())
}

fn cancelled_tool_execution_with_hooks(hook_summary: HookRunSummary) -> ToolExecutionWithHooks {
    ToolExecutionWithHooks {
        execution: ToolExecution {
            output: json!({
                "error": "tool execution cancelled",
                "cancelled": true,
            }),
            is_error: true,
        },
        hook_summary,
    }
}

async fn cancellation_token_cancelled(cancellation_token: ToolCancellationToken) {
    while !cancellation_token.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn wait_for_builtin_tool_worker(
    worker: tokio::task::JoinHandle<ToolExecution>,
    tool_name: &str,
    timeout_ms: u64,
    remaining_timeout: Duration,
) -> Result<ToolExecution, String> {
    if matches!(tool_name, RUN_COMMAND_TOOL | SEARCH_TEXT_TOOL | SLEEP_TOOL) {
        timeout(remaining_timeout, worker)
            .await
            .map_err(|_| format!("tool '{tool_name}' timed out after {timeout_ms} ms"))
            .and_then(|result| {
                result.map_err(|source| format!("tool execution worker failed: {source}"))
            })
    } else {
        timeout(remaining_timeout, worker)
            .await
            .map_err(|_| format!("tool '{tool_name}' timed out after {timeout_ms} ms"))
            .and_then(|result| {
                result.map_err(|source| format!("tool execution worker failed: {source}"))
            })
    }
}

async fn execute_ask_question(
    hook_runtime: HookRuntime,
    global_hooks: &HookConfig,
    api_audit_save_details: bool,
    provider_config: &ProviderConnectionConfig,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    workspace_id: &str,
    workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    llm_request_retry_count: u32,
    tool_call_id: &str,
    arguments: Value,
    cancellation_token: ToolCancellationToken,
) -> ToolExecutionWithHooks {
    let mut hook_summary = HookRunSummary::default();
    let input = match serde_json::from_value::<AskQuestionInput>(arguments) {
        Ok(input) => input,
        Err(source) => {
            return ToolExecutionWithHooks {
                execution: ToolExecution {
                    output: json!({
                        "error": format!("ask_question arguments do not match schema: {source}")
                    }),
                    is_error: true,
                },
                hook_summary,
            };
        }
    };
    let request = match question_request_from_input(workspace_id, chat_id, tool_call_id, input) {
        Ok(request) => request,
        Err(error) => {
            return ToolExecutionWithHooks {
                execution: ToolExecution {
                    output: json!({ "error": error.message }),
                    is_error: true,
                },
                hook_summary,
            };
        }
    };
    let elicitation_summary = hook_runtime
        .run_hooks(HookRunRequest {
            global_config: global_hooks,
            api_audit_save_details,
            workspace_id,
            workspace_path,
            event: "Elicitation",
            match_value: Some(ASK_QUESTION_TOOL.to_string()),
            chat_id: Some(chat_id),
            run_id: Some(run_id),
            session_id: Some(chat_id),
            tool_call_id: Some(tool_call_id),
            model_id: Some(model_id),
            provider_id: Some(provider_id),
            provider_config: Some(provider_config),
            llm_request_retry_count,
            permission_mode: None,
            payload: json!({
                "questionRequest": request.clone(),
            }),
        })
        .await;
    let block_reason = elicitation_summary.first_block_reason();
    let elicitation_action = elicitation_action(&elicitation_summary, &request);
    merge_hook_summaries(&mut hook_summary, elicitation_summary);
    if let Some(reason) = block_reason {
        return ToolExecutionWithHooks {
            execution: ToolExecution {
                output: json!({ "error": format!("Elicitation hook blocked question '{}': {reason}", request.id) }),
                is_error: true,
            },
            hook_summary,
        };
    }
    if let Some(action) = elicitation_action {
        match action {
            ElicitationAction::Accept(answer) => {
                let execution = ToolExecution {
                    output: question_answer_output(&request, answer),
                    is_error: false,
                };
                let result_summary = hook_runtime
                    .run_hooks(HookRunRequest {
                        global_config: global_hooks,
                        api_audit_save_details,
                        workspace_id,
                        workspace_path,
                        event: "ElicitationResult",
                        match_value: Some(ASK_QUESTION_TOOL.to_string()),
                        chat_id: Some(chat_id),
                        run_id: Some(run_id),
                        session_id: Some(chat_id),
                        tool_call_id: Some(tool_call_id),
                        model_id: Some(model_id),
                        provider_id: Some(provider_id),
                        provider_config: Some(provider_config),
                        llm_request_retry_count,
                        permission_mode: None,
                        payload: json!({
                            "questionRequest": request,
                            "questionResult": execution.output.clone(),
                            "isError": execution.is_error,
                        }),
                    })
                    .await;
                let execution = apply_elicitation_result_action(execution, &result_summary);
                merge_hook_summaries(&mut hook_summary, result_summary);
                return ToolExecutionWithHooks {
                    execution,
                    hook_summary,
                };
            }
            ElicitationAction::Decline(reason) | ElicitationAction::Cancel(reason) => {
                return ToolExecutionWithHooks {
                    execution: ToolExecution {
                        output: json!({ "error": reason }),
                        is_error: true,
                    },
                    hook_summary,
                };
            }
        }
    }

    let registration = match question_registry.register(request.clone()) {
        Ok(registration) => registration,
        Err(error) => {
            return ToolExecutionWithHooks {
                execution: ToolExecution {
                    output: json!({ "error": error.message }),
                    is_error: true,
                },
                hook_summary,
            };
        }
    };

    if question_event_tx.send(request.clone()).is_err() {
        return ToolExecutionWithHooks {
            execution: ToolExecution {
                output: json!({
                    "error": format!("failed to show question '{}' because the chat stream is closed", request.id)
                }),
                is_error: true,
            },
            hook_summary,
        };
    }

    let execution = match tokio::select! {
        _ = cancellation_token_cancelled(cancellation_token.clone()) => None,
        answer = registration.answer_rx => Some(answer),
    } {
        Some(Ok(answer)) => {
            let output = question_answer_output(&request, answer);
            ToolExecution {
                output,
                is_error: false,
            }
        }
        Some(Err(_)) => ToolExecution {
            output: json!({
                "error": format!("question '{}' was cancelled before the user answered", request.id)
            }),
            is_error: true,
        },
        None => ToolExecution {
            output: json!({
                "error": format!("question '{}' was cancelled because the chat run was cancelled", request.id),
                "cancelled": true,
            }),
            is_error: true,
        },
    };
    let result_summary = hook_runtime
        .run_hooks(HookRunRequest {
            global_config: global_hooks,
            api_audit_save_details,
            workspace_id,
            workspace_path,
            event: "ElicitationResult",
            match_value: Some(ASK_QUESTION_TOOL.to_string()),
            chat_id: Some(chat_id),
            run_id: Some(run_id),
            session_id: Some(chat_id),
            tool_call_id: Some(tool_call_id),
            model_id: Some(model_id),
            provider_id: Some(provider_id),
            provider_config: Some(provider_config),
            llm_request_retry_count,
            permission_mode: None,
            payload: json!({
                "questionRequest": request,
                "questionResult": execution.output.clone(),
                "isError": execution.is_error,
            }),
        })
        .await;
    let execution = apply_elicitation_result_action(execution, &result_summary);
    merge_hook_summaries(&mut hook_summary, result_summary);

    ToolExecutionWithHooks {
        execution,
        hook_summary,
    }
}

fn question_answer_output(request: &QuestionRequest, answer: QuestionAnswer) -> Value {
    let mut answers_by_id = answer
        .answers
        .into_iter()
        .map(|answer| (answer.id.clone(), answer))
        .collect::<HashMap<_, _>>();
    let answers = request
        .questions
        .iter()
        .filter_map(|question| {
            answers_by_id.remove(&question.id).map(|answer| {
                json!({
                    "id": question.id,
                    "question": question.question,
                    "answer": answer.answer,
                    "selectedOptionValue": answer.selected_option_value,
                })
            })
        })
        .collect::<Vec<_>>();

    json!({
        "questionId": request.id,
        "answers": answers,
    })
}

enum ElicitationAction {
    Accept(QuestionAnswer),
    Decline(String),
    Cancel(String),
}

fn hook_updated_input(summary: &HookRunSummary) -> Option<Value> {
    summary
        .hook_specific_outputs
        .iter()
        .rev()
        .find_map(|output| {
            output
                .get("updatedInput")
                .or_else(|| output.get("input"))
                .or_else(|| {
                    output
                        .get("decision")
                        .and_then(|decision| decision.get("updatedInput"))
                })
                .cloned()
        })
}

fn permission_denied_retry_message(summary: &HookRunSummary) -> Option<String> {
    summary.hook_specific_outputs.iter().find_map(|output| {
        if output
            .get("retry")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let suffix = output
                .get("updatedInput")
                .or_else(|| output.get("input"))
                .or_else(|| {
                    output
                        .get("decision")
                        .and_then(|decision| decision.get("updatedInput"))
                })
                .map(|_| " with updated input")
                .unwrap_or_default();
            Some(format!("PermissionDenied hook requested retry{suffix}."))
        } else {
            None
        }
    })
}

fn elicitation_action(
    summary: &HookRunSummary,
    request: &QuestionRequest,
) -> Option<ElicitationAction> {
    summary
        .hook_specific_outputs
        .iter()
        .find_map(|output| match hook_action(output).as_deref() {
            Some("accept") | Some("accepted") => {
                hook_question_answer(output.get("content"), request).map(ElicitationAction::Accept)
            }
            Some("decline") | Some("declined") => Some(ElicitationAction::Decline(
                hook_action_reason(output, "Elicitation hook declined the question"),
            )),
            Some("cancel") | Some("cancelled") | Some("canceled") => {
                Some(ElicitationAction::Cancel(hook_action_reason(
                    output,
                    "Elicitation hook cancelled the question",
                )))
            }
            _ => None,
        })
}

fn apply_elicitation_result_action(
    mut execution: ToolExecution,
    summary: &HookRunSummary,
) -> ToolExecution {
    for output in &summary.hook_specific_outputs {
        match hook_action(output).as_deref() {
            Some("accept") | Some("accepted") => {
                if let Some(content) = output.get("content") {
                    execution.output = content.clone();
                    execution.is_error = false;
                }
            }
            Some("decline") | Some("declined") | Some("cancel") | Some("cancelled")
            | Some("canceled") => {
                execution.output = json!({ "error": hook_action_reason(output, "ElicitationResult hook rejected the question result") });
                execution.is_error = true;
            }
            _ => {}
        }
    }

    execution
}

fn hook_action(output: &Value) -> Option<String> {
    output
        .get("action")
        .and_then(Value::as_str)
        .map(|action| action.trim().to_ascii_lowercase())
}

fn hook_action_reason(output: &Value, default_reason: &str) -> String {
    output
        .get("reason")
        .or_else(|| output.get("message"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(default_reason)
        .to_string()
}

fn hook_question_answer(
    content: Option<&Value>,
    request: &QuestionRequest,
) -> Option<QuestionAnswer> {
    let content = content?;

    if let Ok(answer) = serde_json::from_value::<QuestionAnswer>(content.clone()) {
        return Some(answer);
    }

    let answers = request
        .questions
        .iter()
        .map(|question| {
            let answer = hook_answer_for_question(content, question);
            QuestionItemAnswer {
                id: question.id.clone(),
                selected_option_value: matching_option_value(question, &answer),
                answer,
            }
        })
        .collect::<Vec<_>>();

    Some(QuestionAnswer { answers })
}

fn hook_answer_for_question(content: &Value, question: &QuestionItem) -> String {
    if let Some(value) = content.get(&question.id) {
        return hook_answer_text(value);
    }

    if let Some(value) = content.get(&question.question) {
        return hook_answer_text(value);
    }

    if let Some(value) = content.get("answer") {
        return hook_answer_text(value);
    }

    if let Some(value) = content.get("value") {
        return hook_answer_text(value);
    }

    hook_answer_text(content)
}

fn hook_answer_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn matching_option_value(question: &QuestionItem, answer: &str) -> Option<String> {
    question
        .options
        .iter()
        .find(|option| option.value == answer || option.label == answer)
        .map(|option| option.value.clone())
}

async fn execute_hook_permission_question(
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    workspace_id: &str,
    chat_id: &str,
    tool_call_id: &str,
    tool_name: &str,
    reason: &str,
) -> Result<(), String> {
    let request_id = unique_id("hook-question");
    let request = QuestionRequest {
        id: request_id.clone(),
        tool_call_id: tool_call_id.to_string(),
        workspace_id: workspace_id.to_string(),
        chat_id: chat_id.to_string(),
        questions: vec![QuestionItem {
            id: format!("{request_id}-item-1"),
            question: format!("Hook asks whether to allow tool '{tool_name}': {reason}"),
            options: vec![
                QuestionOption {
                    label: "Allow".to_string(),
                    value: "allow".to_string(),
                    description: Some("Run the tool once.".to_string()),
                },
                QuestionOption {
                    label: "Deny".to_string(),
                    value: "deny".to_string(),
                    description: Some("Block this tool call.".to_string()),
                },
            ],
            allow_free_text: false,
        }],
    };
    let registration = question_registry
        .register(request.clone())
        .map_err(|source| source.message)?;

    if question_event_tx.send(request.clone()).is_err() {
        return Err(format!(
            "failed to show hook permission question '{}' because the chat stream is closed",
            request.id
        ));
    }

    let answer = registration
        .answer_rx
        .await
        .map_err(|_| format!("hook permission question '{}' was cancelled", request.id))?;
    let selected = answer
        .answers
        .first()
        .and_then(|answer| answer.selected_option_value.as_deref())
        .unwrap_or_default();

    if selected == "allow" {
        Ok(())
    } else {
        Err("user denied hook permission request".to_string())
    }
}

fn question_request_from_input(
    workspace_id: &str,
    chat_id: &str,
    tool_call_id: &str,
    input: AskQuestionInput,
) -> Result<QuestionRequest, ApiError> {
    if input.questions.is_empty() {
        return Err(ApiError::bad_request(
            "ask_question requires at least one question",
        ));
    }

    let request_id = unique_id("question");
    let mut questions = Vec::with_capacity(input.questions.len());

    for (index, item) in input.questions.into_iter().enumerate() {
        let item_number = index + 1;
        let question = non_empty_trimmed(item.question, &format!("question {item_number}"))?;
        let options = normalize_question_options(item.options.unwrap_or_default())?;

        if !item.allow_free_text && options.is_empty() {
            return Err(ApiError::bad_request(format!(
                "ask_question item {item_number} requires options when allowFreeText is false"
            )));
        }

        questions.push(QuestionItem {
            id: format!("{request_id}-item-{item_number}"),
            question,
            options,
            allow_free_text: item.allow_free_text,
        });
    }

    Ok(QuestionRequest {
        id: request_id,
        tool_call_id: tool_call_id.to_string(),
        workspace_id: workspace_id.to_string(),
        chat_id: chat_id.to_string(),
        questions,
    })
}

fn normalize_question_options(
    options: Vec<QuestionOption>,
) -> Result<Vec<QuestionOption>, ApiError> {
    let mut seen_values = HashSet::new();
    let mut normalized = Vec::with_capacity(options.len());

    for option in options {
        let label = non_empty_trimmed(option.label, "option label")?;
        let value = non_empty_trimmed(option.value, "option value")?;
        let description = option
            .description
            .map(|description| description.trim().to_string())
            .filter(|description| !description.is_empty());

        if !seen_values.insert(value.clone()) {
            return Err(ApiError::bad_request(format!(
                "ask_question option value is duplicated: {value}"
            )));
        }

        normalized.push(QuestionOption {
            label,
            value,
            description,
        });
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use foco_agent::{AgentDefinitionId, AgentTeamId};
    use foco_store::{
        config::{AgentDefinitionSettings, AgentModelOptions},
        workspace::{NewAgentTeam, WorkspaceDatabase},
    };

    fn test_agent_definition(
        suffix: &str,
        permissions: AgentPermissions,
    ) -> AgentDefinitionSettings {
        AgentDefinitionSettings {
            id: AgentDefinitionId::new(format!("agent-definition-{suffix}"))
                .expect("definition id"),
            revision: 1,
            name: format!("Agent {suffix}"),
            description: String::new(),
            provider_id: "provider-test".to_string(),
            model_id: "model-test".to_string(),
            model_options: AgentModelOptions::default(),
            system_prompt: "Be precise.".to_string(),
            allowed_tools: vec![READ_FILE_TOOL.to_string()],
            max_instances: 1,
            allowed_execution_workspace_modes: AgentExecutionWorkspaceMode::all(),
            permissions,
        }
    }

    fn create_agent_tool_fixture(
        permissions: AgentPermissions,
    ) -> (
        tempfile::TempDir,
        AgentToolContext,
        AgentTeamId,
        AgentInstanceId,
        AgentTaskId,
    ) {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        database
            .insert_chat("chat-agent-tool-test", "Agent tool test")
            .expect("chat insert");
        let team_id = AgentTeamId::new("agent-team-tool-test").expect("team id");
        let instance_id = AgentInstanceId::new("agent-instance-tool-test").expect("instance id");
        let definition = test_agent_definition("tool-test", permissions.clone());
        database
            .create_agent_team(NewAgentTeam {
                id: &team_id,
                chat_id: "chat-agent-tool-test",
                coordinator_instance_id: &instance_id,
                coordinator_definition: &definition,
                max_concurrent_runs: 1,
            })
            .expect("team create");
        let task_id = AgentTaskId::new("agent-task-tool-test-parent").expect("task id");
        database
            .enqueue_agent_task(NewAgentTask {
                id: &task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: r#"{"message":"parent"}"#,
            })
            .expect("parent task enqueue");
        let (_scheduler, _wake_rx) = AgentScheduler::new();
        let context = AgentToolContext {
            workspace_path: workspace.path().to_path_buf(),
            associations: AgentRunAssociations {
                team_id: Some(team_id.clone()),
                instance_id: Some(instance_id.clone()),
                task_id: Some(task_id.clone()),
                attempt_id: None,
            },
            collaboration_tools_enabled: true,
            permissions,
            agent_definitions: Vec::new(),
            scheduler: _scheduler,
        };
        (workspace, context, team_id, instance_id, task_id)
    }

    #[test]
    fn phase6_agent_tool_permission_and_payload_errors_have_codes() {
        let (workspace, context, _team_id, instance_id, _task_id) =
            create_agent_tool_fixture(AgentPermissions::default());

        let no_delegate_error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-no-delegate",
            json!({
                "targetInstanceId": instance_id.to_string(),
                "targetDefinitionId": null,
                "input": { "message": "child" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("delegation must require canDelegate");
        assert_eq!(
            agent_tool_error_output(&no_delegate_error)["code"],
            "permission_denied"
        );

        let oversized_message_error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_SEND_MESSAGE_TOOL,
            "call-oversized-message",
            json!({
                "receiverInstanceId": instance_id.to_string(),
                "kind": "notification",
                "content": "x".repeat(AGENT_MAX_MESSAGE_CONTENT_CHARS + 1),
                "replyToMessageId": null,
                "relatedTaskId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("oversized message must fail");
        assert_eq!(
            agent_tool_error_output(&oversized_message_error)["code"],
            "payload_too_large"
        );
    }

    #[test]
    fn agent_tool_run_gate_disables_collaboration_tools() {
        let permissions = AgentPermissions {
            can_create_instances: true,
            can_delegate: true,
            allowed_agent_definition_ids: Vec::new(),
        };
        let (workspace, mut context, _team_id, instance_id, _task_id) =
            create_agent_tool_fixture(permissions);
        context.collaboration_tools_enabled = false;

        let error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_SEND_MESSAGE_TOOL,
            "call-disabled-agent-tool",
            json!({
                "receiverInstanceId": instance_id.to_string(),
                "kind": "notification",
                "content": "hello",
                "replyToMessageId": null,
                "relatedTaskId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("run gate should disable Agent tools");
        let output = agent_tool_error_output(&error);
        assert_eq!(output["code"], "permission_denied");
        assert!(
            output["error"]
                .as_str()
                .expect("error text")
                .contains("is not enabled for this run")
        );
    }

    #[test]
    fn agent_create_instances_uses_runtime_capacity_limits() {
        let mut worker_definition =
            test_agent_definition("tool-test-worker", AgentPermissions::default());
        worker_definition.max_instances = 2;
        let permissions = AgentPermissions {
            can_create_instances: true,
            allowed_agent_definition_ids: vec![worker_definition.id.clone()],
            ..AgentPermissions::default()
        };
        let (workspace, mut context, team_id, _instance_id, _task_id) =
            create_agent_tool_fixture(permissions);
        context.agent_definitions = vec![worker_definition.clone()];

        let created = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_CREATE_INSTANCES_TOOL,
            "call-create-worker",
            json!({
                "definitionId": worker_definition.id.to_string(),
                "count": 1,
                "executionWorkspaceMode": "shared",
                "timeoutMs": null,
            }),
        )
        .expect("create should use runtime limits");
        assert_eq!(created["count"], json!(1));

        let limit_error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_CREATE_INSTANCES_TOOL,
            "call-create-worker-limit",
            json!({
                "definitionId": worker_definition.id.to_string(),
                "count": 2,
                "executionWorkspaceMode": "shared",
                "timeoutMs": null,
            }),
        )
        .expect_err("create should reject over definition capacity");
        let output = agent_tool_error_output(&limit_error);
        assert_eq!(output["code"], "limit_exceeded");
        assert!(
            output["error"]
                .as_str()
                .expect("error text")
                .contains("remainingTeamDefinitionSlots=1")
        );
        assert_eq!(
            WorkspaceDatabase::open_or_create(workspace.path())
                .expect("database")
                .agent_instances_for_team(&team_id)
                .expect("instances")
                .len(),
            2
        );
    }

    #[test]
    fn agent_create_instances_rejects_disallowed_workspace_mode() {
        let mut worker_definition =
            test_agent_definition("tool-test-shared-worker", AgentPermissions::default());
        worker_definition.allowed_execution_workspace_modes =
            vec![AgentExecutionWorkspaceMode::Shared];
        let permissions = AgentPermissions {
            can_create_instances: true,
            allowed_agent_definition_ids: vec![worker_definition.id.clone()],
            ..AgentPermissions::default()
        };
        let (workspace, mut context, _team_id, _instance_id, _task_id) =
            create_agent_tool_fixture(permissions);
        context.agent_definitions = vec![worker_definition.clone()];

        let error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_CREATE_INSTANCES_TOOL,
            "call-create-worker-worktree",
            json!({
                "definitionId": worker_definition.id.to_string(),
                "count": 1,
                "executionWorkspaceMode": "isolated_worktree",
                "timeoutMs": null,
            }),
        )
        .expect_err("create should reject a disallowed workspace mode");
        let output = agent_tool_error_output(&error);
        assert_eq!(output["code"], "permission_denied");
        assert!(
            output["error"]
                .as_str()
                .expect("error text")
                .contains("is not allowed")
        );
    }

    #[test]
    fn phase6_agent_delegate_errors_cover_definition_and_limits() {
        let missing_definition_id =
            AgentDefinitionId::new("agent-definition-tool-test-missing").expect("definition id");
        let permissions = AgentPermissions {
            can_delegate: true,
            allowed_agent_definition_ids: vec![missing_definition_id.clone()],
            ..AgentPermissions::default()
        };
        let (workspace, context, team_id, instance_id, parent_task_id) =
            create_agent_tool_fixture(permissions);

        let no_instance_error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-no-instance",
            json!({
                "targetInstanceId": null,
                "targetDefinitionId": missing_definition_id.to_string(),
                "input": { "message": "child" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("definition without instance must fail");
        assert_eq!(
            agent_tool_error_output(&no_instance_error)["code"],
            "not_found"
        );

        let oversized_input_error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-oversized-input",
            json!({
                "targetInstanceId": instance_id.to_string(),
                "targetDefinitionId": null,
                "input": { "message": "x".repeat(AGENT_MAX_TASK_INPUT_BYTES + 1) },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("oversized child input must fail");
        assert_eq!(
            agent_tool_error_output(&oversized_input_error)["code"],
            "payload_too_large"
        );

        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        for index in 0..AGENT_MAX_CHILD_TASKS_PER_TASK {
            let child_task_id =
                AgentTaskId::new(format!("agent-task-tool-test-child-{index}")).expect("task id");
            database
                .enqueue_agent_task(NewAgentTask {
                    id: &child_task_id,
                    team_id: &team_id,
                    owner_instance_id: &instance_id,
                    origin_instance_id: Some(&instance_id),
                    parent_task_id: Some(&parent_task_id),
                    input_json: r#"{"message":"child"}"#,
                })
                .expect("child task enqueue");
        }
        drop(database);

        let child_limit_error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-child-limit",
            json!({
                "targetInstanceId": instance_id.to_string(),
                "targetDefinitionId": null,
                "input": { "message": "child" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("child limit must fail");
        assert_eq!(
            agent_tool_error_output(&child_limit_error)["code"],
            "limit_exceeded"
        );
    }
}
