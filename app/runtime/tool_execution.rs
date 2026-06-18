use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use foco_agent::{
    PendingToolCall, ToolExecutionMode, ToolExecutionPlan, ToolResourceLock, tool_resource_locks,
};
use foco_mcp::{McpRegistry, is_mcp_tool_name};
use foco_providers::ProviderConnectionConfig;
use foco_store::config::{HookConfig, WebSearchSettings};
use foco_tools::{
    ASK_QUESTION_TOOL, RUN_COMMAND_TOOL, SLEEP_TOOL, ToolCancellationToken, ToolExecution,
    ToolOutputSink, builtin_tool_timeout_ms,
    execute_builtin_tool_for_chat_with_cancellation_and_output_sink,
};
use futures_util::future::join_all;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::timeout;

use super::{
    AskQuestionInput, QuestionAnswer, QuestionItem, QuestionItemAnswer, QuestionOption,
    QuestionRegistry, QuestionRequest, ToolOutputDeltaSink, ToolResourceLease,
    ToolResourceLockRegistry, execute_web_tool, is_web_tool_name, web_tool_timeout_ms,
};
use crate::*;

use foco_providers::NeutralToolCall;
use foco_tools::{
    FIND_FILES_TOOL, GET_TODO_GRAPH_TOOL, GRAPH_EXPLORE_TOOL, GRAPH_FIND_CALLEES_TOOL,
    GRAPH_FIND_CALLERS_TOOL, GRAPH_FIND_REFERENCES_TOOL, GRAPH_FIND_SYMBOLS_TOOL,
    GRAPH_RELATED_FILES_TOOL, READ_FILE_TOOL, SEARCH_TEXT_TOOL,
};
use serde_json::Value;

use crate::{
    MAX_REPEATED_TOOL_CALL_BATCHES, MEMORY_SEARCH_TOOL_NAME, READ_ONLY_TOOL_BATCH_WARNING_THRESHOLD,
};

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

pub(crate) async fn execute_tool_calls_parallel(
    mcp_registry: Arc<McpRegistry>,
    hook_runtime: HookRuntime,
    global_hooks: HookConfig,
    provider_config: ProviderConnectionConfig,
    web_search_settings: WebSearchSettings,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    memory_tool_context: MemoryToolContext,
    workspace_id: &str,
    workspace_path: &Path,
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
                        provider_config.clone(),
                        web_search_settings.clone(),
                        question_registry.clone(),
                        question_event_tx.clone(),
                        memory_tool_context.clone(),
                        tool_resource_lock_registry.clone(),
                        cancellation_token.clone(),
                        tool_output_delta_tx.clone(),
                        assistant_message_id,
                        workspace_id,
                        workspace_path,
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
                    let provider_config = provider_config.clone();
                    let web_search_settings = web_search_settings.clone();
                    let question_registry = question_registry.clone();
                    let question_event_tx = question_event_tx.clone();
                    let memory_tool_context = memory_tool_context.clone();
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
                                provider_config,
                                web_search_settings,
                                question_registry,
                                question_event_tx,
                                memory_tool_context,
                                tool_resource_lock_registry,
                                cancellation_token,
                                tool_output_delta_tx,
                                &assistant_message_id,
                                &workspace_id,
                                &workspace_path,
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
    provider_config: ProviderConnectionConfig,
    web_search_settings: WebSearchSettings,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    mut memory_tool_context: MemoryToolContext,
    tool_resource_lock_registry: ToolResourceLockRegistry,
    cancellation_token: ToolCancellationToken,
    tool_output_delta_tx: mpsc::UnboundedSender<ToolOutputDeltaEvent>,
    assistant_message_id: &str,
    workspace_id: &str,
    workspace_path: &Path,
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
        &provider_config,
        &web_search_settings,
        question_registry,
        question_event_tx,
        memory_tool_context,
        tool_resource_lock_registry,
        cancellation_token.clone(),
        tool_output_delta_tx,
        assistant_message_id,
        workspace_id,
        workspace_path,
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
    provider_config: &ProviderConnectionConfig,
    web_search_settings: &WebSearchSettings,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    memory_tool_context: MemoryToolContext,
    tool_resource_lock_registry: ToolResourceLockRegistry,
    cancellation_token: ToolCancellationToken,
    tool_output_delta_tx: mpsc::UnboundedSender<ToolOutputDeltaEvent>,
    assistant_message_id: &str,
    workspace_id: &str,
    workspace_path: &Path,
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
    let _resource_lease = match wait_for_tool_resource_lock(
        &tool_resource_lock_registry,
        workspace_id,
        resource_locks,
        tool_name,
        tool_timeout_ms,
        tool_deadline,
        cancellation_token.clone(),
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

    if tool_name == ASK_QUESTION_TOOL {
        let ask_question = execute_ask_question(
            hook_runtime,
            global_hooks,
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
            let workspace_path = workspace_path.to_path_buf();
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
) -> Result<ToolResourceLease, String> {
    let acquire = registry.acquire(workspace_id, resource_locks);
    match (timeout_ms, deadline.and_then(remaining_duration_until)) {
        (Some(timeout_ms), Some(remaining)) => {
            tokio::select! {
                _ = cancellation_token_cancelled(cancellation_token) => {
                    Err("tool execution cancelled".to_string())
                }
                lease = timeout(remaining, acquire) => {
                    lease.map_err(|_| format!("tool '{tool_name}' timed out waiting for resource lock after {timeout_ms} ms"))
                }
            }
        }
        (Some(timeout_ms), None) => Err(format!(
            "tool '{tool_name}' timed out waiting for resource lock after {timeout_ms} ms"
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
