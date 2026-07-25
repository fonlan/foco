use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
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
use foco_store::config::{
    GlobalConfig, HookConfig, SKILL_SCOPE_GLOBAL, SKILL_SCOPE_WORKSPACE, WebSearchSettings,
};
use foco_store::workspace::{
    AgentInstanceRecord, AgentTaskRecord, NewAgentEvent, NewAgentInstance, NewAgentMessage,
    NewAgentTask, RegisterAgentTaskWaitDependencies, WorkspaceDatabase,
};
use foco_tools::{
    AGENT_CANCEL_TASK_TOOL, AGENT_CREATE_INSTANCES_TOOL, AGENT_DELEGATE_TASK_TOOL,
    AGENT_GET_TASK_TOOL, AGENT_LIST_TOOL, AGENT_SEND_MESSAGE_TOOL, AGENT_TRANSFER_TASK_TOOL,
    AGENT_WAIT_TASKS_TOOL, ASK_QUESTION_TOOL, BuiltinToolContext, BuiltinToolExecutionOptions,
    BuiltinToolRuntime, RUN_COMMAND_TOOL, SLEEP_TOOL, ToolCancellationToken, ToolExecution,
    ToolOutputSink, builtin_tool_timeout_ms,
    execute_builtin_tool_with_context_and_execution_options,
    execute_builtin_tool_with_context_and_options, find_files_target_outside_workspace,
    read_file_target_outside_workspace, search_text_target_outside_workspace,
};
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::timeout;

use super::{
    AGENT_MAX_CREATE_INSTANCES_PER_REQUEST, AGENT_MAX_INSTANCES_PER_TEAM,
    AGENT_MAX_QUEUED_TASKS_PER_CHAT, AGENT_MAX_QUEUED_TASKS_PER_INSTANCE,
    AGENT_MAX_QUEUED_TASKS_PER_TEAM, AGENT_MESSAGE_GUIDANCE_SOURCE, ActiveChatRunRegistry,
    AgentMessageGuidanceDelivery, AgentScheduler, AskQuestionInput, CodeGraphIndexState,
    CodeGraphReadinessError, QuestionAnswer, QuestionItem, QuestionItemAnswer, QuestionOption,
    QuestionRegistry, QuestionRequest, ToolOutputDeltaSink, ToolResourceLease,
    ToolResourceLockOwner, ToolResourceLockRegistry, execute_image_tool, execute_web_tool,
    image_tool_timeout_ms, is_image_tool_name, is_web_tool_name, wait_for_code_graph_ready,
    web_tool_timeout_ms,
};
use crate::*;

use foco_providers::NeutralToolCall;
use foco_tools::{
    CREATE_PLAN_TOOL, CREATE_TODO_GRAPH_TOOL, DELETE_PLAN_TOOL, EDIT_FILE_TOOL, FIND_FILES_TOOL,
    GET_PLANS_TOOL, GET_TODO_GRAPH_TOOL, GRAPH_EXPLORE_TOOL, GRAPH_FIND_CALLEES_TOOL,
    GRAPH_FIND_CALLERS_TOOL, GRAPH_FIND_CHILDREN_TOOL, GRAPH_FIND_IMPORTERS_TOOL,
    GRAPH_FIND_IMPORTS_TOOL, GRAPH_FIND_REFERENCES_TOOL, GRAPH_FIND_SYMBOLS_TOOL,
    GRAPH_RELATED_FILES_TOOL, READ_FILE_TOOL, READ_SPEC_TOOL, SEARCH_TEXT_TOOL,
    UPDATE_PLAN_STEP_TOOL, UPDATE_PLAN_TOOL, UPDATE_SPEC_TOOL, UPDATE_TODO_GRAPH_TOOL,
    WRITE_FILE_TOOL,
};
use serde_json::Value;

use crate::git_backend::{
    agent_worktree_relative_path, create_agent_worktree, delete_agent_worktree,
};
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

/// Pre-execution classification shared by local and remote tool loops.
///
/// `Err` is reserved for fatal transport/structure validation failures.
/// Recoverable repeated-batch loops return [`ToolLoopBeforeExecutionAction::RecoverRepeatedBatch`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ToolLoopBeforeExecutionAction {
    Continue,
    RecoverRepeatedBatch { message: String, tool_names: String },
}

#[derive(Default)]
pub(crate) struct RepeatedToolCallDetector {
    previous_batch: Option<Vec<ToolCallLoopSignature>>,
    consecutive_count: usize,
}

impl RepeatedToolCallDetector {
    pub(crate) fn check(
        &mut self,
        tool_calls: &[NeutralToolCall],
    ) -> Result<ToolLoopBeforeExecutionAction, String> {
        validate_tool_call_transport_fields(tool_calls)?;
        let batch = tool_call_loop_signatures(tool_calls);
        if self.previous_batch.as_ref() == Some(&batch) {
            self.consecutive_count += 1;
        } else {
            self.previous_batch = Some(batch);
            self.consecutive_count = 1;
        }

        if self.consecutive_count < MAX_REPEATED_TOOL_CALL_BATCHES {
            return Ok(ToolLoopBeforeExecutionAction::Continue);
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

        // Keep detector state so the same batch remains blocked after recovery.
        Ok(ToolLoopBeforeExecutionAction::RecoverRepeatedBatch {
            message: format!(
                "agent run repeated the same tool call batch {MAX_REPEATED_TOOL_CALL_BATCHES} times ({tool_names}); possible tool-call loop"
            ),
            tool_names,
        })
    }
}

pub(crate) fn validate_tool_call_transport_fields(
    tool_calls: &[NeutralToolCall],
) -> Result<(), String> {
    let limit = foco_tools::output_budget::TOOL_TRANSPORT_DYNAMIC_FIELD_BYTE_LIMIT;
    if let Some(tool_call) = tool_calls
        .iter()
        .find(|tool_call| tool_call.call_id.len() > limit)
    {
        let (tool_name, _) = foco_tools::output_budget::bounded_utf8_prefix(&tool_call.name, limit);
        return Err(format!(
            "provider tool call id for '{tool_name}' exceeds the {limit}-byte transport limit"
        ));
    }
    Ok(())
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
            | GET_TODO_GRAPH_TOOL
            | READ_SPEC_TOOL
            | MEMORY_SEARCH_TOOL_NAME
    ) || is_code_graph_tool_name(tool_name)
}

/// Central list of built-in Code Graph tools that query the execution-root index.
pub(crate) fn is_code_graph_tool_name(tool_name: &str) -> bool {
    matches!(
        tool_name,
        GRAPH_FIND_SYMBOLS_TOOL
            | GRAPH_FIND_CALLERS_TOOL
            | GRAPH_FIND_CALLEES_TOOL
            | GRAPH_FIND_CHILDREN_TOOL
            | GRAPH_FIND_REFERENCES_TOOL
            | GRAPH_FIND_IMPORTS_TOOL
            | GRAPH_FIND_IMPORTERS_TOOL
            | GRAPH_RELATED_FILES_TOOL
            | GRAPH_EXPLORE_TOOL
    )
}

fn builtin_tool_uses_workspace_database(tool_name: &str) -> bool {
    matches!(
        tool_name,
        CREATE_TODO_GRAPH_TOOL
            | UPDATE_TODO_GRAPH_TOOL
            | GET_TODO_GRAPH_TOOL
            | CREATE_PLAN_TOOL
            | GET_PLANS_TOOL
            | UPDATE_PLAN_TOOL
            | UPDATE_PLAN_STEP_TOOL
            | DELETE_PLAN_TOOL
            | READ_SPEC_TOOL
            | UPDATE_SPEC_TOOL
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
    pub(crate) workspace_id: String,
    pub(crate) workspace_path: PathBuf,
    pub(crate) associations: AgentRunAssociations,
    pub(crate) collaboration_tools_enabled: bool,
    pub(crate) permissions: AgentPermissions,
    pub(crate) agent_definitions: Vec<AgentDefinitionSettings>,
    pub(crate) scheduler: AgentScheduler,
    pub(crate) active_chat_runs: ActiveChatRunRegistry,
}

pub(crate) async fn execute_tool_calls_parallel(
    mcp_registry: Arc<McpRegistry>,
    hook_runtime: HookRuntime,
    global_hooks: HookConfig,
    api_audit_save_details: bool,
    global_config: GlobalConfig,
    provider_config: ProviderConnectionConfig,
    web_search_settings: WebSearchSettings,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    memory_tool_context: MemoryToolContext,
    agent_tool_context: Option<AgentToolContext>,
    skill_read_root_dirs: Vec<PathBuf>,
    attachment_read_allowlist: Vec<PathBuf>,
    workspace_id: &str,
    workspace_path: &Path,
    tool_workspace_path: &Path,
    chat_id: &str,
    session_mode: Option<&str>,
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
    builtin_tool_runtime: BuiltinToolRuntime,
    code_graph_indexes: Arc<Mutex<CodeGraphIndexState>>,
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
                        global_config.clone(),
                        provider_config.clone(),
                        web_search_settings.clone(),
                        question_registry.clone(),
                        question_event_tx.clone(),
                        memory_tool_context.clone(),
                        agent_tool_context.clone(),
                        skill_read_root_dirs.clone(),
                        attachment_read_allowlist.clone(),
                        tool_resource_lock_registry.clone(),
                        cancellation_token.clone(),
                        tool_output_delta_tx.clone(),
                        builtin_tool_runtime.clone(),
                        code_graph_indexes.clone(),
                        assistant_message_id,
                        workspace_id,
                        workspace_path,
                        tool_workspace_path,
                        chat_id,
                        session_mode,
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
                    let session_mode = session_mode.map(str::to_string);
                    let run_id = run_id.to_string();
                    let model_id = model_id.to_string();
                    let provider_id = provider_id.to_string();
                    let assistant_message_id = assistant_message_id.to_string();
                    let llm_request_retry_count = llm_request_retry_count;
                    let mcp_registry = mcp_registry.clone();
                    let hook_runtime = hook_runtime.clone();
                    let global_hooks = global_hooks.clone();
                    let api_audit_save_details = api_audit_save_details;
                    let global_config = global_config.clone();
                    let provider_config = provider_config.clone();
                    let web_search_settings = web_search_settings.clone();
                    let question_registry = question_registry.clone();
                    let question_event_tx = question_event_tx.clone();
                    let memory_tool_context = memory_tool_context.clone();
                    let agent_tool_context = agent_tool_context.clone();
                    let skill_read_root_dirs = skill_read_root_dirs.clone();
                    let attachment_read_allowlist = attachment_read_allowlist.clone();
                    let tool_resource_lock_registry = tool_resource_lock_registry.clone();
                    let cancellation_token = cancellation_token.clone();
                    let tool_output_delta_tx = tool_output_delta_tx.clone();
                    let builtin_tool_runtime = builtin_tool_runtime.clone();
                    let code_graph_indexes = code_graph_indexes.clone();
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
                                global_config,
                                provider_config,
                                web_search_settings,
                                question_registry,
                                question_event_tx,
                                memory_tool_context,
                                agent_tool_context,
                                skill_read_root_dirs,
                                attachment_read_allowlist,
                                tool_resource_lock_registry,
                                cancellation_token,
                                tool_output_delta_tx,
                                builtin_tool_runtime,
                                code_graph_indexes,
                                &assistant_message_id,
                                &workspace_id,
                                &workspace_path,
                                &tool_workspace_path,
                                &chat_id,
                                session_mode.as_deref(),
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
    global_config: GlobalConfig,
    provider_config: ProviderConnectionConfig,
    web_search_settings: WebSearchSettings,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    mut memory_tool_context: MemoryToolContext,
    agent_tool_context: Option<AgentToolContext>,
    skill_read_root_dirs: Vec<PathBuf>,
    attachment_read_allowlist: Vec<PathBuf>,
    tool_resource_lock_registry: ToolResourceLockRegistry,
    cancellation_token: ToolCancellationToken,
    tool_output_delta_tx: mpsc::UnboundedSender<ToolOutputDeltaEvent>,
    builtin_tool_runtime: BuiltinToolRuntime,
    code_graph_indexes: Arc<Mutex<CodeGraphIndexState>>,
    assistant_message_id: &str,
    workspace_id: &str,
    workspace_path: &Path,
    tool_workspace_path: &Path,
    chat_id: &str,
    session_mode: Option<&str>,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    llm_request_retry_count: u32,
    tool_call: NeutralToolCall,
) -> ToolHookOutcome {
    let started_at_text = utc_timestamp();
    memory_tool_context.tool_call_id = tool_call.call_id.clone();
    let mut tool_execution = execute_tool_with_runtime(
        mcp_registry,
        hook_runtime.clone(),
        &global_hooks,
        api_audit_save_details,
        &global_config,
        Some(&provider_config),
        &web_search_settings,
        question_registry,
        question_event_tx,
        memory_tool_context,
        agent_tool_context,
        skill_read_root_dirs,
        attachment_read_allowlist,
        tool_resource_lock_registry,
        cancellation_token.clone(),
        tool_output_delta_tx,
        assistant_message_id,
        workspace_id,
        workspace_path,
        tool_workspace_path,
        chat_id,
        session_mode,
        run_id,
        model_id,
        provider_id,
        llm_request_retry_count,
        &tool_call.call_id,
        &tool_call.name,
        tool_call.arguments.clone(),
        builtin_tool_runtime,
        code_graph_indexes,
    )
    .await;
    let completed_at_text = utc_timestamp();
    tool_execution.execution = budget_tool_result_envelope(
        assistant_message_id,
        &tool_call.call_id,
        &tool_call.name,
        &started_at_text,
        &completed_at_text,
        tool_execution.execution,
    )
    .execution;
    let mut hook_summary = tool_execution.hook_summary;
    let post_summary = run_post_tool_hooks(
        &hook_runtime,
        &global_hooks,
        api_audit_save_details,
        workspace_id,
        workspace_path,
        chat_id,
        run_id,
        model_id,
        provider_id,
        Some(&provider_config),
        llm_request_retry_count,
        &tool_call,
        &tool_execution.execution,
    )
    .await;
    merge_hook_summaries(&mut hook_summary, post_summary);

    ToolHookOutcome {
        tool_call: executed_tool_call(
            tool_call,
            tool_execution.execution,
            started_at_text,
            completed_at_text,
        ),
        hook_summary,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_post_tool_hooks(
    hook_runtime: &HookRuntime,
    global_hooks: &HookConfig,
    api_audit_save_details: bool,
    workspace_id: &str,
    workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    provider_config: Option<&ProviderConnectionConfig>,
    llm_request_retry_count: u32,
    tool_call: &NeutralToolCall,
    execution: &ToolExecution,
) -> HookRunSummary {
    let post_event = if execution.is_error {
        "PostToolUseFailure"
    } else {
        "PostToolUse"
    };
    hook_runtime
        .run_hooks(HookRunRequest {
            global_config: global_hooks,
            api_audit_save_details,
            workspace_id,
            workspace_path,
            event: post_event,
            match_value: Some(tool_call.name.clone()),
            chat_id: Some(chat_id),
            run_id: Some(run_id),
            session_id: Some(chat_id),
            tool_call_id: Some(&tool_call.call_id),
            model_id: Some(model_id),
            provider_id: Some(provider_id),
            provider_config,
            llm_request_retry_count,
            permission_mode: None,
            payload: json!({
                "toolName": tool_call.name,
                "toolInput": tool_call.arguments,
                "toolOutput": execution.output,
                "isError": execution.is_error,
            }),
        })
        .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolResultBudgetEnvelope<'a> {
    #[serde(rename = "type")]
    event_type: &'static str,
    assistant_message_id: &'a str,
    tool_call_id: &'a str,
    output: &'a Value,
    is_error: bool,
    started_at: &'a str,
    completed_at: &'a str,
}

pub(crate) fn tool_output_semantics(
    tool_name: &str,
) -> foco_tools::output_budget::ToolOutputSemantics {
    match foco_agent::tool_effect(tool_name).retry_safety() {
        foco_agent::ToolRetrySafety::RetrySafe => {
            foco_tools::output_budget::ToolOutputSemantics::ReadOnly
        }
        foco_agent::ToolRetrySafety::RetryUnsafe => {
            foco_tools::output_budget::ToolOutputSemantics::RetryUnsafe
        }
    }
}

pub(crate) fn budget_tool_execution(
    tool_name: &str,
    execution: ToolExecution,
) -> foco_tools::output_budget::BudgetedToolExecution {
    foco_tools::output_budget::normalize_tool_execution(
        tool_name,
        tool_output_semantics(tool_name),
        execution,
    )
}

fn budget_tool_result_envelope(
    assistant_message_id: &str,
    tool_call_id: &str,
    tool_name: &str,
    started_at: &str,
    completed_at: &str,
    execution: ToolExecution,
) -> foco_tools::output_budget::BudgetedToolExecution {
    foco_tools::output_budget::normalize_tool_execution_for_envelope(
        tool_name,
        tool_output_semantics(tool_name),
        execution,
        |execution| {
            foco_tools::output_budget::serialized_json_size(&ToolResultBudgetEnvelope {
                event_type: "toolResult",
                assistant_message_id,
                tool_call_id,
                output: &execution.output,
                is_error: execution.is_error,
                started_at,
                completed_at,
            })
        },
    )
}

/// Executes a tool with an ephemeral runtime for callers that do not own host state.
///
/// Production local and sidecar paths must use [`execute_tool_with_runtime`] so managed
/// background command handles stay scoped to their owning host.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_tool(
    mcp_registry: Arc<McpRegistry>,
    hook_runtime: HookRuntime,
    global_hooks: &HookConfig,
    api_audit_save_details: bool,
    global_config: &GlobalConfig,
    provider_config: Option<&ProviderConnectionConfig>,
    web_search_settings: &WebSearchSettings,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    memory_tool_context: MemoryToolContext,
    agent_tool_context: Option<AgentToolContext>,
    skill_read_root_dirs: Vec<PathBuf>,
    attachment_read_allowlist: Vec<PathBuf>,
    tool_resource_lock_registry: ToolResourceLockRegistry,
    cancellation_token: ToolCancellationToken,
    tool_output_delta_tx: mpsc::UnboundedSender<ToolOutputDeltaEvent>,
    assistant_message_id: &str,
    workspace_id: &str,
    workspace_path: &Path,
    tool_workspace_path: &Path,
    chat_id: &str,
    session_mode: Option<&str>,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    llm_request_retry_count: u32,
    tool_call_id: &str,
    tool_name: &str,
    arguments: Value,
) -> ToolExecutionWithHooks {
    execute_tool_with_runtime(
        mcp_registry,
        hook_runtime,
        global_hooks,
        api_audit_save_details,
        global_config,
        provider_config,
        web_search_settings,
        question_registry,
        question_event_tx,
        memory_tool_context,
        agent_tool_context,
        skill_read_root_dirs,
        attachment_read_allowlist,
        tool_resource_lock_registry,
        cancellation_token,
        tool_output_delta_tx,
        assistant_message_id,
        workspace_id,
        workspace_path,
        tool_workspace_path,
        chat_id,
        session_mode,
        run_id,
        model_id,
        provider_id,
        llm_request_retry_count,
        tool_call_id,
        tool_name,
        arguments,
        BuiltinToolRuntime::default(),
        Arc::new(Mutex::new(CodeGraphIndexState::default())),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_tool_with_runtime(
    mcp_registry: Arc<McpRegistry>,
    hook_runtime: HookRuntime,
    global_hooks: &HookConfig,
    api_audit_save_details: bool,
    global_config: &GlobalConfig,
    provider_config: Option<&ProviderConnectionConfig>,
    web_search_settings: &WebSearchSettings,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    memory_tool_context: MemoryToolContext,
    agent_tool_context: Option<AgentToolContext>,
    skill_read_root_dirs: Vec<PathBuf>,
    attachment_read_allowlist: Vec<PathBuf>,
    tool_resource_lock_registry: ToolResourceLockRegistry,
    cancellation_token: ToolCancellationToken,
    tool_output_delta_tx: mpsc::UnboundedSender<ToolOutputDeltaEvent>,
    assistant_message_id: &str,
    workspace_id: &str,
    workspace_path: &Path,
    tool_workspace_path: &Path,
    chat_id: &str,
    session_mode: Option<&str>,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    llm_request_retry_count: u32,
    tool_call_id: &str,
    tool_name: &str,
    arguments: Value,
    builtin_tool_runtime: BuiltinToolRuntime,
    code_graph_indexes: Arc<Mutex<CodeGraphIndexState>>,
) -> ToolExecutionWithHooks {
    let mut result = execute_tool_unbudgeted(
        mcp_registry,
        hook_runtime,
        global_hooks,
        api_audit_save_details,
        global_config,
        provider_config,
        web_search_settings,
        question_registry,
        question_event_tx,
        memory_tool_context,
        agent_tool_context,
        skill_read_root_dirs,
        attachment_read_allowlist,
        tool_resource_lock_registry,
        cancellation_token,
        tool_output_delta_tx,
        assistant_message_id,
        workspace_id,
        workspace_path,
        tool_workspace_path,
        chat_id,
        session_mode,
        run_id,
        model_id,
        provider_id,
        llm_request_retry_count,
        tool_call_id,
        tool_name,
        arguments,
        builtin_tool_runtime,
        code_graph_indexes,
    )
    .await;
    let budgeted = budget_tool_execution(tool_name, result.execution);
    if budgeted.state != foco_tools::output_budget::ToolOutputBudgetState::WithinBudget {
        let (original_bytes, original_lines) = budgeted
            .original_measurement
            .map(|measurement| (measurement.serialized_bytes, measurement.text_lines))
            .unwrap_or_default();
        tracing::warn!(
            tool_name,
            original_bytes,
            original_lines,
            budget_state = ?budgeted.state,
            "tool output was normalized to the shared output budget"
        );
    }
    result.execution = budgeted.execution;
    result
}

async fn execute_tool_unbudgeted(
    mcp_registry: Arc<McpRegistry>,
    hook_runtime: HookRuntime,
    global_hooks: &HookConfig,
    api_audit_save_details: bool,
    global_config: &GlobalConfig,
    provider_config: Option<&ProviderConnectionConfig>,
    web_search_settings: &WebSearchSettings,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    memory_tool_context: MemoryToolContext,
    agent_tool_context: Option<AgentToolContext>,
    skill_read_root_dirs: Vec<PathBuf>,
    attachment_read_allowlist: Vec<PathBuf>,
    tool_resource_lock_registry: ToolResourceLockRegistry,
    cancellation_token: ToolCancellationToken,
    tool_output_delta_tx: mpsc::UnboundedSender<ToolOutputDeltaEvent>,
    assistant_message_id: &str,
    workspace_id: &str,
    workspace_path: &Path,
    tool_workspace_path: &Path,
    chat_id: &str,
    session_mode: Option<&str>,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    llm_request_retry_count: u32,
    tool_call_id: &str,
    tool_name: &str,
    mut arguments: Value,
    builtin_tool_runtime: BuiltinToolRuntime,
    code_graph_indexes: Arc<Mutex<CodeGraphIndexState>>,
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
            provider_config,
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
                        provider_config,
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
                                provider_config,
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
                                provider_config,
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

    normalize_read_file_skill_alias_arguments(
        tool_name,
        workspace_path,
        tool_workspace_path,
        &skill_read_root_dirs,
        &mut arguments,
    );

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
        let tool_workspace_path = tool_workspace_path.to_path_buf();
        let worker = tokio::task::spawn_blocking(move || {
            execute_agent_tool(
                &agent_tool_context,
                &tool_workspace_path,
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
            execution = execute_web_tool(
                web_search_settings,
                tool_name,
                arguments,
                remaining_timeout,
                tool_workspace_path,
            ) => execution,
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

    if is_image_tool_name(tool_name) {
        let remaining_timeout = tool_deadline
            .and_then(remaining_duration_until)
            .unwrap_or(Duration::ZERO);
        set_tool_timeout_ms(&mut arguments, remaining_timeout);
        let execution = tokio::select! {
            _ = cancellation_token_cancelled(cancellation_token.clone()) => {
                Err("tool execution cancelled".to_string())
            }
            execution = execute_image_tool(
                global_config,
                workspace_path,
                chat_id,
                run_id,
                tool_name,
                arguments,
                remaining_timeout,
            ) => execution,
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
        let allow_external_read_access = match ensure_read_file_external_access(
            &global_config,
            &skill_read_root_dirs,
            &attachment_read_allowlist,
            question_registry.clone(),
            question_event_tx.clone(),
            workspace_id,
            workspace_path,
            tool_workspace_path,
            chat_id,
            tool_call_id,
            tool_name,
            &arguments,
            cancellation_token.clone(),
        )
        .await
        {
            Ok(allow_external_read_access) => allow_external_read_access,
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
        set_tool_timeout_ms(&mut arguments, remaining_timeout);
        let tool_name = tool_name.to_string();
        let builtin_workspace_path = if builtin_tool_uses_workspace_database(&tool_name) {
            workspace_path
        } else {
            tool_workspace_path
        };
        if is_code_graph_tool_name(&tool_name) {
            let indexes = code_graph_indexes.clone();
            let execution_root = builtin_workspace_path.to_path_buf();
            let wait_cancellation = cancellation_token.clone();
            let wait_deadline = tool_deadline;
            let readiness = tokio::task::spawn_blocking(move || {
                wait_for_code_graph_ready(
                    &indexes,
                    &execution_root,
                    wait_deadline,
                    Some(&wait_cancellation),
                )
            })
            .await;
            match readiness {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    return ToolExecutionWithHooks {
                        execution: code_graph_readiness_tool_execution(error),
                        hook_summary,
                    };
                }
                Err(join_error) => {
                    return ToolExecutionWithHooks {
                        execution: ToolExecution {
                            output: json!({
                                "error": format!(
                                    "code graph readiness wait failed: {join_error}"
                                ),
                                "retryable": true,
                            }),
                            is_error: true,
                        },
                        hook_summary,
                    };
                }
            }
            if cancellation_token.is_cancelled() {
                return cancelled_tool_execution_with_hooks(hook_summary);
            }
        }
        let worker = tokio::task::spawn_blocking({
            let workspace_path = builtin_workspace_path.to_path_buf();
            let chat_id = chat_id.to_string();
            let run_id = run_id.to_string();
            let session_mode = session_mode.map(str::to_string);
            let assistant_message_id = assistant_message_id.to_string();
            let tool_call_id = tool_call_id.to_string();
            let tool_name = tool_name.clone();
            let cancellation_token = cancellation_token.clone();
            let builtin_tool_runtime = builtin_tool_runtime.clone();
            move || {
                execute_builtin_tool_with_context_and_execution_options(
                    &workspace_path,
                    BuiltinToolContext {
                        chat_id: Some(&chat_id),
                        run_id: Some(&run_id),
                        session_mode: session_mode.as_deref(),
                    },
                    &tool_name,
                    arguments,
                    BuiltinToolExecutionOptions {
                        runtime: builtin_tool_runtime,
                        cancellation_token: Some(cancellation_token),
                        output_sink: if tool_name == RUN_COMMAND_TOOL {
                            Some(Arc::new(ToolOutputDeltaSink {
                                assistant_message_id: assistant_message_id.clone(),
                                tool_call_id: tool_call_id.clone(),
                                tx: tool_output_delta_tx,
                            }) as Arc<dyn ToolOutputSink>)
                        } else {
                            None
                        },
                        allow_external_read_access,
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
#[serde(rename_all = "snake_case")]
enum AgentDelegateTargetKind {
    Instance,
    Definition,
}

impl AgentDelegateTargetKind {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Instance => "instance",
            Self::Definition => "definition",
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentDelegateTaskInput {
    target_kind: AgentDelegateTargetKind,
    /// Raw string so illegal ids map to recoverable `invalid_arguments` before strong typing.
    target_id: String,
    input: Value,
    correlation_id: Option<String>,
    #[serde(rename = "timeoutMs")]
    _timeout_ms: Option<u64>,
}

/// Strongly typed target after `targetKind` selects the corresponding id contract.
enum AgentDelegateTarget {
    Instance(AgentInstanceId),
    Definition(AgentDefinitionId),
}

impl AgentDelegateTarget {
    fn target_definition_id(&self) -> Option<&AgentDefinitionId> {
        match self {
            Self::Instance(_) => None,
            Self::Definition(definition_id) => Some(definition_id),
        }
    }
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
    /// Raw string so illegal ids map to recoverable `invalid_arguments` before strong typing.
    target_instance_id: String,
    #[serde(rename = "timeoutMs")]
    _timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentCreateInstancesInput {
    /// Raw string so illegal ids map to recoverable `invalid_arguments` before strong typing.
    definition_id: String,
    count: u32,
    execution_workspace_mode: AgentExecutionWorkspaceMode,
    #[serde(rename = "timeoutMs")]
    _timeout_ms: Option<u64>,
}

fn execute_agent_tool(
    context: &AgentToolContext,
    tool_workspace_path: &Path,
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
            execute_agent_create_instances(context, workspace_path, tool_workspace_path, arguments)
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
    drop(database);
    let delivery = context.active_chat_runs.deliver_agent_message_guidance(
        &context.workspace_id,
        team_id,
        &message.receiver_instance_id,
        message.related_task_id.as_ref(),
        GuidanceMessage {
            id: message.id.to_string(),
            content: message.content.clone(),
            attachments: Vec::new(),
            source: AGENT_MESSAGE_GUIDANCE_SOURCE.to_string(),
            interrupted_assistant_id: None,
        },
    );
    Ok(json!({
        "messageId": message.id.to_string(),
        "receiverInstanceId": message.receiver_instance_id.to_string(),
        "kind": message.kind.as_str(),
        "sequence": message.sequence,
        "createdAt": message.created_at,
        "delivery": match delivery {
            AgentMessageGuidanceDelivery::Guidance => "guidance",
            AgentMessageGuidanceDelivery::Queued => "queued",
        },
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
    let target = resolve_agent_delegate_target(&input)?;
    context
        .permissions
        .authorize_collaboration_tool(
            AgentCollaborationTool::DelegateTask,
            agent_tool_instance_id(context)?.clone(),
        )
        .map_err(|source| agent_tool_error("permission_denied", source.to_string()))?;
    let target_instance_id = select_delegate_target_instance(context, workspace_path, &target)?;
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
            "targetDefinitionId": target.target_definition_id().map(ToString::to_string),
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

    let dependency_task_ids: Vec<AgentTaskId> =
        dependencies.iter().map(|task| task.id.clone()).collect();
    // Full wait-round registration is atomic in the store (deps + task_waiting_requested).
    database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id,
            waiting_task_id: current_task_id,
            dependency_task_ids: &dependency_task_ids,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some(tool_call_id),
            deadline_at: deadline_at.as_deref(),
            event_instance_id: Some(actor_instance_id),
        })
        .map_err(agent_store_error)?;

    // Explicit waits may read already-terminal tasks immediately. Only suspend when at
    // least one dependency is still outstanding so the same tool_call_id can complete later.
    if dependencies.iter().all(|task| task.status.is_terminal()) {
        return Ok(agent_wait_terminal_tool_result(
            &dependencies,
            AgentTaskWaitMode::All,
            deadline_at.as_deref(),
        ));
    }

    Ok(agent_wait_suspend_tool_output(
        current_task_id,
        tool_call_id,
        &dependency_task_ids,
        deadline_at.as_deref(),
        false,
    ))
}

/// Outcome of an implicit wait registered when a parent run would otherwise finalize.
#[derive(Clone, Debug)]
pub(crate) struct ImplicitAgentWait {
    pub(crate) tool_call_id: String,
    pub(crate) task_ids: Vec<String>,
    pub(crate) output: Value,
    /// When true, all undelivered children were already terminal and the output is the
    /// final wait result (no suspend). The parent turn should continue in-process.
    pub(crate) immediate: bool,
}

/// If the current agent task still has child tasks whose results were never delivered via a wait
/// round, register an implicit `agent_wait_tasks` dependency set and return a wait payload.
///
/// "Delivered" means the child was already covered by a wait round for this parent: either the
/// current dependency rows or a historical `task_waiting_requested` event. Sequential wait-round
/// replacement deletes current rows, so history keeps already-landed children from re-entering
/// finalize waits. Explicit waits may still re-read terminal tasks on demand.
pub(crate) fn try_register_implicit_wait_for_undelivered_children(
    context: &AgentToolContext,
) -> Result<Option<ImplicitAgentWait>, String> {
    let team_id = agent_tool_team_id(context)?;
    let actor_instance_id = agent_tool_instance_id(context)?;
    let current_task_id = agent_tool_task_id(context)?;
    let mut database =
        WorkspaceDatabase::open_or_create(&context.workspace_path).map_err(agent_store_error)?;
    let current_task = database
        .agent_task_for_team(team_id, current_task_id)
        .map_err(agent_store_error)?
        .ok_or_else(|| {
            agent_tool_error(
                "not_found",
                format!("current Agent task '{current_task_id}' was not found"),
            )
        })?;
    let children = database
        .agent_tasks_for_parent(team_id, current_task_id)
        .map_err(agent_store_error)?;
    if children.is_empty() {
        return Ok(None);
    }
    let delivered = database
        .agent_wait_covered_dependency_task_ids(team_id, current_task_id)
        .map_err(agent_store_error)?;
    let undelivered: Vec<AgentTaskRecord> = children
        .into_iter()
        .filter(|task| !delivered.contains(&task.id))
        .collect();
    if undelivered.is_empty() {
        return Ok(None);
    }
    if undelivered.len() > AGENT_MAX_CHILD_TASKS_PER_TASK {
        return Err(agent_tool_error(
            "limit_exceeded",
            format!(
                "implicit agent wait exceeds {AGENT_MAX_CHILD_TASKS_PER_TASK} undelivered child tasks"
            ),
        ));
    }
    for dependency_task in &undelivered {
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
    }
    let dependency_task_ids: Vec<AgentTaskId> =
        undelivered.iter().map(|task| task.id.clone()).collect();
    let tool_call_id = unique_id("implicit-wait");
    database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id,
            waiting_task_id: current_task_id,
            dependency_task_ids: &dependency_task_ids,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some(&tool_call_id),
            deadline_at: None,
            event_instance_id: Some(actor_instance_id),
        })
        .map_err(agent_store_error)?;
    let task_ids: Vec<String> = dependency_task_ids
        .iter()
        .map(|task_id| task_id.to_string())
        .collect();
    let all_terminal = undelivered.iter().all(|task| task.status.is_terminal());
    if all_terminal {
        return Ok(Some(ImplicitAgentWait {
            tool_call_id,
            task_ids,
            output: agent_wait_terminal_tool_result(
                &undelivered,
                AgentTaskWaitMode::All,
                None,
            ),
            immediate: true,
        }));
    }
    Ok(Some(ImplicitAgentWait {
        tool_call_id: tool_call_id.clone(),
        task_ids: task_ids.clone(),
        output: agent_wait_suspend_tool_output(
            current_task_id,
            &tool_call_id,
            &dependency_task_ids,
            None,
            true,
        ),
        immediate: false,
    }))
}

/// True when a tool output is the non-terminal suspend control for `agent_wait_tasks`.
///
/// Suspend outputs must not complete the tool call; the matching terminal resume result
/// reuses the same `tool_call_id`.
pub(crate) fn is_agent_wait_suspend_output(output: &Value) -> bool {
    output
        .get("suspend")
        .and_then(|control| control.get("kind"))
        .and_then(Value::as_str)
        == Some("agent_wait_tasks")
}

fn agent_wait_suspend_tool_output(
    current_task_id: &AgentTaskId,
    tool_call_id: &str,
    dependency_task_ids: &[AgentTaskId],
    deadline_at: Option<&str>,
    implicit: bool,
) -> Value {
    let task_ids: Vec<String> = dependency_task_ids
        .iter()
        .map(|task_id| task_id.to_string())
        .collect();
    let mut output = json!({
        "waiting": true,
        "taskId": current_task_id.to_string(),
        "mode": AgentTaskWaitMode::All.as_str(),
        "taskIds": task_ids.clone(),
        "deadlineAt": deadline_at,
        "suspend": {
            "kind": "agent_wait_tasks",
            "pendingToolCallId": tool_call_id,
            "taskIds": task_ids,
            "mode": AgentTaskWaitMode::All.as_str(),
            "deadlineAt": deadline_at,
        }
    });
    if implicit {
        output["implicit"] = Value::Bool(true);
    }
    output
}

fn agent_wait_terminal_tool_result(
    dependencies: &[AgentTaskRecord],
    wait_mode: AgentTaskWaitMode,
    deadline_at: Option<&str>,
) -> Value {
    let dependency_values = dependencies
        .iter()
        .map(|task| {
            json!({
                "taskId": task.id.to_string(),
                "status": task.status.as_str(),
                "result": task
                    .result_json
                    .as_deref()
                    .and_then(|value| serde_json::from_str::<Value>(value).ok()),
                "error": task
                    .error_json
                    .as_deref()
                    .and_then(|value| serde_json::from_str::<Value>(value).ok()),
                "completedAt": task.completed_at,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "waiting": false,
        "mode": wait_mode.as_str(),
        "deadlineAt": deadline_at,
        "dependencies": dependency_values,
    })
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
    let target_instance_id = parse_required_agent_instance_id(
        AGENT_TRANSFER_TASK_TOOL,
        "targetInstanceId",
        &input.target_instance_id,
    )?;
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
        .agent_instance(&target_instance_id)
        .map_err(agent_store_error)?
        .ok_or_else(|| {
            agent_tool_error(
                "not_found",
                format!("Agent target instance '{target_instance_id}' was not found"),
            )
        })?;
    if target.team_id != *team_id {
        return Err(agent_tool_error(
            "cross_team_reference",
            format!(
                "Agent target instance '{target_instance_id}' does not belong to team '{team_id}'"
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
    tool_workspace_path: &Path,
    arguments: Value,
) -> Result<Value, String> {
    let input =
        serde_json::from_value::<AgentCreateInstancesInput>(arguments).map_err(|source| {
            agent_tool_error(
                "invalid_arguments",
                format!("agent_create_instances arguments do not match schema: {source}"),
            )
        })?;
    let definition_id = parse_required_agent_definition_id(
        AGENT_CREATE_INSTANCES_TOOL,
        "definitionId",
        &input.definition_id,
    )?;
    context
        .permissions
        .authorize_instance_definition(&definition_id, agent_tool_instance_id(context)?.clone())
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
        .find(|definition| definition.id == definition_id)
        .ok_or_else(|| {
            agent_tool_error(
                "not_found",
                format!("Agent definition '{definition_id}' was not found"),
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
    let worktree_root_paths = match input.execution_workspace_mode {
        AgentExecutionWorkspaceMode::Shared if tool_workspace_path != workspace_path => {
            let shared_root_path =
                agent_worktree_relative_path(workspace_path, tool_workspace_path)
                    .map_err(|source| agent_tool_error("worktree_error", source.message))?;
            vec![shared_root_path; instance_ids.len()]
        }
        _ => worktrees
            .iter()
            .map(|worktree| agent_worktree_relative_path(workspace_path, &worktree.root_path))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| agent_tool_error("worktree_error", source.message))?,
    };
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
        "definitionId": definition_id.to_string(),
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

fn resolve_agent_delegate_target(
    input: &AgentDelegateTaskInput,
) -> Result<AgentDelegateTarget, String> {
    match &input.target_kind {
        AgentDelegateTargetKind::Instance => AgentInstanceId::new(&input.target_id)
            .map(AgentDelegateTarget::Instance)
            .map_err(|_| {
                agent_delegate_target_id_error(
                    input.target_kind.as_str(),
                    AgentInstanceId::PREFIX,
                    "instances[].id",
                    &input.target_id,
                )
            }),
        AgentDelegateTargetKind::Definition => AgentDefinitionId::new(&input.target_id)
            .map(AgentDelegateTarget::Definition)
            .map_err(|_| {
                agent_delegate_target_id_error(
                    input.target_kind.as_str(),
                    AgentDefinitionId::PREFIX,
                    "definitions[].id",
                    &input.target_id,
                )
            }),
    }
}

fn agent_delegate_target_id_error(
    target_kind: &str,
    prefix: &str,
    agent_list_path: &str,
    value: &str,
) -> String {
    agent_tool_error(
        "invalid_arguments",
        format!(
            "agent_delegate_task targetKind {target_kind:?} requires a targetId starting with \
'{prefix}' (got {value:?}). Copy the exact id from agent_list.{agent_list_path}; \
do not invent ids or use display names."
        ),
    )
}

fn parse_required_agent_definition_id(
    tool: &str,
    field: &str,
    value: &str,
) -> Result<AgentDefinitionId, String> {
    AgentDefinitionId::new(value).map_err(|_| {
        agent_invalid_id_error(
            tool,
            field,
            AgentDefinitionId::PREFIX,
            "definitions[].id",
            value,
        )
    })
}

fn parse_required_agent_instance_id(
    tool: &str,
    field: &str,
    value: &str,
) -> Result<AgentInstanceId, String> {
    AgentInstanceId::new(value).map_err(|_| {
        agent_invalid_id_error(
            tool,
            field,
            AgentInstanceId::PREFIX,
            "instances[].id",
            value,
        )
    })
}

/// Stable, recoverable invalid-id hint for collaboration tools when providers bypass schema.
fn agent_invalid_id_error(
    tool: &str,
    field: &str,
    prefix: &str,
    agent_list_path: &str,
    value: &str,
) -> String {
    agent_tool_error(
        "invalid_arguments",
        format!(
            "{tool} {field} is not a valid Agent id (got {value:?}). \
Must start with '{prefix}', use only lowercase ASCII letters/digits/hyphens after the prefix, \
and be at most 128 characters total. \
Copy the exact id from agent_list.{agent_list_path}; do not invent ids or use display names."
        ),
    )
}

fn select_delegate_target_instance(
    context: &AgentToolContext,
    workspace_path: &Path,
    target: &AgentDelegateTarget,
) -> Result<AgentInstanceId, String> {
    match target {
        AgentDelegateTarget::Instance(instance_id) => {
            let team_id = agent_tool_team_id(context)?;
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
            if instance.team_id != *team_id {
                return Err(agent_tool_error(
                    "cross_team_reference",
                    format!("Agent instance '{instance_id}' does not belong to team '{team_id}'"),
                ));
            }
            Ok(instance.id)
        }
        AgentDelegateTarget::Definition(definition_id) => {
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
            let team_id = agent_tool_team_id(context)?;
            let database =
                WorkspaceDatabase::open_or_create(workspace_path).map_err(agent_store_error)?;
            let instance = database
                .route_agent_instance_for_definition(team_id, definition_id)
                .map_err(agent_store_error)?;
            let instance = instance.ok_or_else(|| {
                agent_tool_error(
                    "not_found",
                    format!(
                        "Agent definition '{definition_id}' has no existing runnable instance in team '{team_id}'. \
Call agent_list, then agent_create_instances when allowed, then agent_delegate_task \
with targetKind instance and a returned targetId (or a definition that already has an instance). \
targetKind definition never auto-creates instances.",
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
    use foco_agent::AgentDomainErrorCode;
    use foco_store::workspace::WorkspaceDatabaseError;

    match error {
        WorkspaceDatabaseError::AgentDomain { source } => {
            let code = match source.code() {
                AgentDomainErrorCode::DependencyCycle => "dependency_cycle",
                AgentDomainErrorCode::CrossTeamReference => "cross_team_reference",
                AgentDomainErrorCode::PermissionDenied => "permission_denied",
                AgentDomainErrorCode::QueueConflict => "queue_conflict",
                AgentDomainErrorCode::InstanceLimitExceeded => "limit_exceeded",
                AgentDomainErrorCode::MutationLeaseConflict => "mutation_lease_conflict",
                AgentDomainErrorCode::InvalidId => "invalid_arguments",
                AgentDomainErrorCode::InvalidStateTransition => "invalid_task_status",
                AgentDomainErrorCode::MissingCoordinatorDefinition => "not_found",
                AgentDomainErrorCode::TeamBusy => "team_busy",
            };
            agent_tool_error(code, source.message().to_string())
        }
        WorkspaceDatabaseError::InvalidAgentRuntimeData { message } => {
            let lower = message.to_ascii_lowercase();
            let code = if lower.contains("active wait round") {
                "wait_round_active"
            } else if lower.contains("conflicts with existing registration")
                || lower.contains("cannot change dependency set")
            {
                "wait_round_conflict"
            } else if lower.contains("must not be empty")
                || lower.contains("pending tool call id")
                || lower.contains("duplicate dependency")
            {
                "invalid_arguments"
            } else {
                "invalid_agent_runtime"
            };
            agent_tool_error(code, message)
        }
        WorkspaceDatabaseError::Sqlite { .. } => agent_tool_error(
            "store_error",
            "workspace database operation failed; retry the agent tool call",
        ),
        other => agent_tool_error("store_error", other.to_string()),
    }
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
    } else if is_image_tool_name(tool_name) {
        image_tool_timeout_ms(arguments).map(Some)
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

fn code_graph_readiness_tool_execution(error: CodeGraphReadinessError) -> ToolExecution {
    match error {
        CodeGraphReadinessError::Cancelled => ToolExecution {
            output: json!({
                "error": "tool execution cancelled",
                "cancelled": true,
            }),
            is_error: true,
        },
        CodeGraphReadinessError::TimedOut { execution_root } => ToolExecution {
            output: json!({
                "error": format!(
                    "code graph index is still initializing for execution root '{}'; retry after indexing completes",
                    execution_root.display()
                ),
                "retryable": true,
                "codeGraphPhase": "initializing",
                "executionRoot": execution_root.display().to_string(),
            }),
            is_error: true,
        },
        CodeGraphReadinessError::Failed {
            execution_root,
            stage,
            error,
        } => ToolExecution {
            output: json!({
                "error": format!(
                    "code graph index failed for execution root '{}' during {stage}: {error}",
                    execution_root.display()
                ),
                "retryable": true,
                "codeGraphPhase": "failed",
                "executionRoot": execution_root.display().to_string(),
                "failedStage": stage,
            }),
            is_error: true,
        },
        CodeGraphReadinessError::InvalidPath { path, error } => ToolExecution {
            output: json!({
                "error": format!(
                    "failed to resolve code graph execution root '{}': {error}",
                    path.display()
                ),
                "retryable": false,
            }),
            is_error: true,
        },
    }
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
    provider_config: Option<&ProviderConnectionConfig>,
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
            provider_config,
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
                        provider_config,
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
            provider_config,
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

static EXTERNAL_READONLY_ACCESS_CHATS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static EXTERNAL_READONLY_ACCESS_PROMPT_LOCKS: OnceLock<
    Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
> = OnceLock::new();

fn external_readonly_access_chats() -> &'static Mutex<HashSet<String>> {
    EXTERNAL_READONLY_ACCESS_CHATS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn external_readonly_access_prompt_lock(chat_id: &str) -> Arc<tokio::sync::Mutex<()>> {
    let locks = EXTERNAL_READONLY_ACCESS_PROMPT_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks
        .lock()
        .expect("external readonly access prompt lock table");

    // ponytail: one lock per chat is tiny for process lifetime; add cleanup if chat churn becomes large.
    locks
        .entry(chat_id.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

fn chat_allows_external_readonly(chat_id: &str) -> bool {
    external_readonly_access_chats()
        .lock()
        .expect("external readonly access lock")
        .contains(chat_id)
}

fn allow_external_readonly_for_chat(chat_id: &str) {
    // ponytail: process memory is enough for this session-scoped grant; persist per-chat auth if it must survive restarts.
    external_readonly_access_chats()
        .lock()
        .expect("external readonly access lock")
        .insert(chat_id.to_string());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExternalReadonlyAccessDecision {
    AllowOnce,
    AllowAll,
    Deny,
}

/// Target kind shown in confirmation copy (file vs directory vs path).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExternalReadonlyTargetKind {
    File,
    Directory,
    Path,
}

/// Rewrites a missing worktree-relative Skill path to the matching real Skill file
/// from the run-scoped snapshot. This is deliberately narrower than external-read
/// authorization: it only corrects `read_file` inputs and leaves all other tools on
/// their execution-root resolvers.
fn normalize_read_file_skill_alias_arguments(
    tool_name: &str,
    workspace_path: &Path,
    tool_workspace_path: &Path,
    skill_read_root_dirs: &[PathBuf],
    arguments: &mut Value,
) {
    let Some(path) = arguments.get("path").and_then(Value::as_str) else {
        return;
    };
    let Some(alias_target) = resolve_worktree_read_file_skill_alias(
        tool_name,
        workspace_path,
        tool_workspace_path,
        skill_read_root_dirs,
        path,
    ) else {
        return;
    };
    let Some(alias_target) = alias_target.to_str() else {
        return;
    };
    let Some(path) = arguments.get_mut("path") else {
        return;
    };

    *path = Value::String(alias_target.to_string());
}

fn resolve_worktree_read_file_skill_alias(
    tool_name: &str,
    workspace_path: &Path,
    tool_workspace_path: &Path,
    skill_read_root_dirs: &[PathBuf],
    path: &str,
) -> Option<PathBuf> {
    if tool_name != READ_FILE_TOOL || workspace_path == tool_workspace_path {
        return None;
    }

    let shared_workspace_root = std::fs::canonicalize(workspace_path).ok()?;
    let worktree_root = std::fs::canonicalize(tool_workspace_path).ok()?;
    if shared_workspace_root == worktree_root {
        return None;
    }

    let relative_path = worktree_skill_relative_path(path, tool_workspace_path)?;
    if !is_workspace_skill_relative_path(&relative_path) {
        return None;
    }

    let worktree_target = tool_workspace_path.join(&relative_path);
    match std::fs::symlink_metadata(&worktree_target) {
        Ok(_) => return None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return None,
    }
    if worktree_path_has_symlink_component(tool_workspace_path, &relative_path) {
        return None;
    }

    let canonical_candidate =
        std::fs::canonicalize(shared_workspace_root.join(relative_path)).ok()?;
    if !std::fs::metadata(&canonical_candidate).ok()?.is_file()
        || !path_is_within_runtime_skill_read_roots(&canonical_candidate, skill_read_root_dirs)
    {
        return None;
    }

    Some(canonical_candidate)
}

fn worktree_skill_relative_path(path: &str, tool_workspace_path: &Path) -> Option<PathBuf> {
    let input = Path::new(path);
    if input.as_os_str().is_empty()
        || input
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return None;
    }

    let relative_path = if input.is_absolute() {
        if tool_workspace_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return None;
        }
        input.strip_prefix(tool_workspace_path).ok()?.to_path_buf()
    } else {
        input.to_path_buf()
    };

    if relative_path.as_os_str().is_empty()
        || relative_path.components().any(|component| {
            matches!(
                component,
                std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
                    | std::path::Component::ParentDir
            )
        })
    {
        return None;
    }

    Some(relative_path)
}

fn is_workspace_skill_relative_path(path: &Path) -> bool {
    let mut components = path.components();
    let Some(std::path::Component::Normal(location)) = components.next() else {
        return false;
    };
    let Some(std::path::Component::Normal(skills)) = components.next() else {
        return false;
    };
    let Some(std::path::Component::Normal(_skill_name)) = components.next() else {
        return false;
    };

    (location == ".agents" || location == ".claude")
        && skills == "skills"
        && components.next().is_some()
}

fn worktree_path_has_symlink_component(worktree_root: &Path, relative_path: &Path) -> bool {
    let mut current = worktree_root.to_path_buf();
    for component in relative_path.components() {
        let std::path::Component::Normal(segment) = component else {
            return true;
        };
        current.push(segment);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return true,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
            Err(_) => return true,
        }
    }
    false
}

fn path_is_within_runtime_skill_read_roots(target_path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| {
        std::fs::canonicalize(root)
            .ok()
            .is_some_and(|root| target_path.starts_with(root))
    })
}

fn is_external_readonly_access_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        READ_FILE_TOOL | FIND_FILES_TOOL | SEARCH_TEXT_TOOL
    )
}

/// Classify whether `arguments.path` is outside the execution workspace for a
/// restricted readonly tool. Returns `None` for internal paths (no prompt).
fn classify_external_readonly_target(
    tool_name: &str,
    tool_workspace_path: &Path,
    path: &str,
) -> Result<Option<(PathBuf, ExternalReadonlyTargetKind)>, String> {
    match tool_name {
        READ_FILE_TOOL => Ok(
            read_file_target_outside_workspace(tool_workspace_path, path)?
                .map(|target| (target, ExternalReadonlyTargetKind::File)),
        ),
        FIND_FILES_TOOL => Ok(
            find_files_target_outside_workspace(tool_workspace_path, path)?
                .map(|target| (target, ExternalReadonlyTargetKind::Directory)),
        ),
        SEARCH_TEXT_TOOL => Ok(
            search_text_target_outside_workspace(tool_workspace_path, path)?
                .map(|target| (target, ExternalReadonlyTargetKind::Path)),
        ),
        _ => Ok(None),
    }
}

fn external_readonly_target_label(kind: ExternalReadonlyTargetKind) -> &'static str {
    match kind {
        ExternalReadonlyTargetKind::File => "文件",
        ExternalReadonlyTargetKind::Directory => "目录",
        ExternalReadonlyTargetKind::Path => "路径",
    }
}

/// Restricted readonly external-path authorization for `read_file`, `find_files`,
/// and `search_text` only. Other tools always get `Ok(false)` (no external grant).
///
/// Contract summary (see also `docs/readonly-tools-external-path-contract.md`):
/// - `search_text.path` may be a file or directory (rg); not directory-only.
/// - External match/entry paths are absolute; internal remain workspace-relative.
/// - `search_text` snapshots / `fullResultPath` always live under execution workspace
///   `.foco/search-results/` (never the external root); reading `fullResultPath` is
///   an ordinary internal `read_file` and needs no external grant.
/// - Non-empty continuation skips external re-auth here; tool layer binds query/path.
/// - Graph / write / edit / run never receive this grant; chat `allow_all` is readonly-only.
async fn ensure_read_file_external_access(
    global_config: &GlobalConfig,
    skill_read_root_dirs: &[PathBuf],
    attachment_read_allowlist: &[PathBuf],
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    workspace_id: &str,
    shared_workspace_path: &Path,
    tool_workspace_path: &Path,
    chat_id: &str,
    tool_call_id: &str,
    tool_name: &str,
    arguments: &Value,
    cancellation_token: ToolCancellationToken,
) -> Result<bool, String> {
    // Tool audit: only read_file / find_files / search_text participate in
    // ask-before-external-readonly. graph_* stay on execution-root resolvers;
    // write_file / edit_file / run_command never receive this auto-grant.
    // Todo/Plan/Spec use the shared workspace database path via
    // `builtin_tool_uses_workspace_database`, not this helper.
    if !is_external_readonly_access_tool(tool_name) {
        return Ok(false);
    }

    // search_text continuation pages only read an execution-workspace snapshot that was
    // written after an authorized initial search. Do not re-prompt or re-check external path
    // grants; keep allow_external_read_access=false so the tool layer takes the snapshot path.
    if tool_name == SEARCH_TEXT_TOOL
        && arguments
            .get("continuation")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|token| !token.is_empty())
    {
        return Ok(false);
    }

    let path = arguments
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{tool_name} requires string path"))?;
    // External-read mode is relative to the execution root (worktree when isolated).
    let Some((target_path, target_kind)) =
        classify_external_readonly_target(tool_name, tool_workspace_path, path)?
    else {
        return Ok(false);
    };

    // Plan/agent isolated worktrees may still read the real shared workspace without
    // ask_question. Membership uses the canonical target (symlink escapes stay outside).
    // Checked before Skill/chat grants so shared-root trust is the first fast path.
    if path_is_within_shared_workspace(&target_path, shared_workspace_path) {
        return Ok(true);
    }

    // Prefer the run-scoped Skill snapshot from prompt assembly so routing-table
    // visibility and readonly grants stay identical for shared + isolated worktrees.
    // Global / other non-shared skill roots still use this path.
    if path_is_within_skill_read_roots(&target_path, skill_read_root_dirs)
        || read_file_target_is_configured_skill(global_config, workspace_id, &target_path)
    {
        return Ok(true);
    }

    // Exact chat attachment path grants (not parent dirs / siblings / lookalikes).
    // Attachment allowlist authorizes only read_file on the file itself — never
    // find_files / search_text on a parent directory or sibling path.
    if tool_name == READ_FILE_TOOL
        && path_is_exact_attachment_allowlist_match(&target_path, attachment_read_allowlist)
    {
        return Ok(true);
    }

    if chat_allows_external_readonly(chat_id) {
        return Ok(true);
    }

    let prompt_lock = external_readonly_access_prompt_lock(chat_id);
    let _prompt_guard = prompt_lock.lock().await;

    if chat_allows_external_readonly(chat_id) {
        return Ok(true);
    }

    match ask_external_readonly_access(
        question_registry,
        question_event_tx,
        workspace_id,
        chat_id,
        tool_call_id,
        tool_name,
        target_kind,
        &target_path,
        cancellation_token,
    )
    .await?
    {
        ExternalReadonlyAccessDecision::AllowOnce => Ok(true),
        ExternalReadonlyAccessDecision::AllowAll => {
            allow_external_readonly_for_chat(chat_id);
            Ok(true)
        }
        ExternalReadonlyAccessDecision::Deny => Err(format!(
            "user denied {tool_name} access to workspace-external {}: {}",
            external_readonly_target_label(target_kind),
            target_path.display()
        )),
    }
}

/// Returns true when `target_path` (already canonical) lies under the current shared workspace root.
fn path_is_within_shared_workspace(target_path: &Path, shared_workspace_path: &Path) -> bool {
    let Ok(shared_root) = std::fs::canonicalize(shared_workspace_path) else {
        return false;
    };
    target_path.starts_with(&shared_root)
}

fn read_file_target_is_configured_skill(
    config: &GlobalConfig,
    workspace_id: &str,
    target_path: &Path,
) -> bool {
    // Fallback when a call site has no prompt-assembly snapshot (tests, remote
    // sidecar paths). Prefer skill_read_root_dirs from AvailableSkillsSnapshot.
    let roots = skill_read_root_dirs_from_settings(
        &config
            .skills
            .detected
            .iter()
            .filter(|skill| {
                skill.scope == SKILL_SCOPE_GLOBAL
                    || (skill.scope == SKILL_SCOPE_WORKSPACE
                        && skill.workspace_id.as_deref() == Some(workspace_id))
            })
            .cloned()
            .collect::<Vec<_>>(),
    );
    path_is_within_skill_read_roots(target_path, &roots)
}

async fn ask_external_readonly_access(
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    workspace_id: &str,
    chat_id: &str,
    tool_call_id: &str,
    tool_name: &str,
    target_kind: ExternalReadonlyTargetKind,
    target_path: &Path,
    cancellation_token: ToolCancellationToken,
) -> Result<ExternalReadonlyAccessDecision, String> {
    let target_label = external_readonly_target_label(target_kind);
    let request_id = unique_id("external-readonly-question");
    let request = QuestionRequest {
        id: request_id.clone(),
        tool_call_id: tool_call_id.to_string(),
        workspace_id: workspace_id.to_string(),
        chat_id: chat_id.to_string(),
        questions: vec![QuestionItem {
            id: format!("{request_id}-item-1"),
            question: format!(
                "{tool_name} 想要访问 workspace 外的{target_label}:\n{}",
                target_path.display()
            ),
            options: vec![
                QuestionOption {
                    label: "允许".to_string(),
                    value: "allow".to_string(),
                    description: Some(format!("仅允许本次 {tool_name} 访问。")),
                },
                QuestionOption {
                    label: "全部允许".to_string(),
                    value: "allow_all".to_string(),
                    description: Some(
                        "允许当前聊天会话内所有 workspace 外的 read_file / find_files / search_text 只读访问。"
                            .to_string(),
                    ),
                },
                QuestionOption {
                    label: "拒绝".to_string(),
                    value: "deny".to_string(),
                    description: Some(format!("阻止本次 {tool_name} 访问。")),
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
            "failed to show {tool_name} external access question '{}' because the chat stream is closed",
            request.id
        ));
    }

    let answer = match tokio::select! {
        _ = cancellation_token_cancelled(cancellation_token) => None,
        answer = registration.answer_rx => Some(answer),
    } {
        Some(Ok(answer)) => answer,
        Some(Err(_)) => {
            return Err(format!(
                "{tool_name} external access question '{}' was cancelled before the user answered",
                request.id
            ));
        }
        None => {
            return Err(format!(
                "{tool_name} external access question '{}' was cancelled because the chat run was cancelled",
                request.id
            ));
        }
    };

    external_readonly_access_decision_from_answer(tool_name, &answer)
}

fn external_readonly_access_decision_from_answer(
    tool_name: &str,
    answer: &QuestionAnswer,
) -> Result<ExternalReadonlyAccessDecision, String> {
    let selected = answer
        .answers
        .first()
        .and_then(|answer| answer.selected_option_value.as_deref())
        .unwrap_or_default();

    match selected {
        "allow" => Ok(ExternalReadonlyAccessDecision::AllowOnce),
        "allow_all" => Ok(ExternalReadonlyAccessDecision::AllowAll),
        "deny" => Ok(ExternalReadonlyAccessDecision::Deny),
        other => Err(format!(
            "{tool_name} external access question returned unknown option: {other}"
        )),
    }
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
    use foco_agent::{AgentAttemptId, AgentDefinitionId, AgentTeamId};
    use foco_store::{
        config::{AgentDefinitionSettings, AgentModelOptions, MemorySettings},
        memory::MemoryStatus,
        workspace::{NewAgentTeam, WorkspaceDatabase, workspace_database_path},
    };
    use std::fs;

    fn worktree_skill_alias_fixture() -> (tempfile::TempDir, tempfile::TempDir, PathBuf, PathBuf) {
        let shared_workspace = tempfile::tempdir().expect("shared workspace");
        let worktree = tempfile::tempdir().expect("worktree");
        let skill_root = shared_workspace.path().join(".agents/skills/example");
        let reference = skill_root.join("references/guide.md");
        fs::create_dir_all(reference.parent().expect("reference parent"))
            .expect("create skill reference directory");
        fs::write(skill_root.join("SKILL.md"), "skill instructions").expect("write skill");
        fs::write(&reference, "reference instructions").expect("write reference");
        (shared_workspace, worktree, skill_root, reference)
    }

    #[test]
    fn read_file_skill_alias_redirects_missing_relative_skill_file() {
        let (shared_workspace, worktree, skill_root, _) = worktree_skill_alias_fixture();
        let expected = fs::canonicalize(skill_root.join("SKILL.md")).expect("canonical skill");
        let roots = vec![fs::canonicalize(&skill_root).expect("canonical skill root")];
        let mut arguments = json!({
            "path": ".agents/skills/example/SKILL.md",
            "startLine": null,
            "endLine": null,
        });

        normalize_read_file_skill_alias_arguments(
            READ_FILE_TOOL,
            shared_workspace.path(),
            worktree.path(),
            &roots,
            &mut arguments,
        );

        assert_eq!(arguments["path"], expected.to_string_lossy().as_ref());
        let execution = execute_builtin_tool_with_context_and_options(
            worktree.path(),
            BuiltinToolContext::for_chat(None),
            READ_FILE_TOOL,
            arguments,
            None,
            None,
            true,
        );
        assert!(!execution.is_error, "{:?}", execution.output);
        assert_eq!(
            execution.output["path"],
            expected.to_string_lossy().as_ref()
        );
    }

    #[test]
    fn read_file_skill_alias_redirects_missing_worktree_absolute_nested_file() {
        let (shared_workspace, worktree, skill_root, reference) = worktree_skill_alias_fixture();
        let roots = vec![fs::canonicalize(&skill_root).expect("canonical skill root")];
        let missing_worktree_path = worktree
            .path()
            .join(".agents/skills/example/references/guide.md");

        let resolved = resolve_worktree_read_file_skill_alias(
            READ_FILE_TOOL,
            shared_workspace.path(),
            worktree.path(),
            &roots,
            &missing_worktree_path.to_string_lossy(),
        );

        assert_eq!(
            resolved,
            Some(fs::canonicalize(reference).expect("canonical reference"))
        );
    }

    #[test]
    fn read_file_skill_alias_redirects_missing_claude_skill_asset() {
        let shared_workspace = tempfile::tempdir().expect("shared workspace");
        let worktree = tempfile::tempdir().expect("worktree");
        let skill_root = shared_workspace.path().join(".claude/skills/example");
        let asset = skill_root.join("assets/template.txt");
        fs::create_dir_all(asset.parent().expect("asset parent")).expect("create asset directory");
        fs::write(&asset, "template").expect("write asset");
        let roots = vec![fs::canonicalize(&skill_root).expect("canonical skill root")];

        let resolved = resolve_worktree_read_file_skill_alias(
            READ_FILE_TOOL,
            shared_workspace.path(),
            worktree.path(),
            &roots,
            ".claude/skills/example/assets/template.txt",
        );

        assert_eq!(
            resolved,
            Some(fs::canonicalize(asset).expect("canonical asset"))
        );
    }

    #[test]
    fn read_file_skill_alias_requires_a_current_snapshot_root() {
        let (shared_workspace, worktree, _, _) = worktree_skill_alias_fixture();

        let resolved = resolve_worktree_read_file_skill_alias(
            READ_FILE_TOOL,
            shared_workspace.path(),
            worktree.path(),
            &[],
            ".agents/skills/example/SKILL.md",
        );

        assert_eq!(resolved, None);
    }

    #[cfg(unix)]
    #[test]
    fn read_file_skill_alias_rejects_source_symlink_escape() {
        let (shared_workspace, worktree, skill_root, _) = worktree_skill_alias_fixture();
        let outside_file = tempfile::NamedTempFile::new().expect("outside file");
        let escaped_file = skill_root.join("assets/escaped.txt");
        fs::create_dir_all(escaped_file.parent().expect("asset parent"))
            .expect("create asset directory");
        std::os::unix::fs::symlink(outside_file.path(), &escaped_file)
            .expect("create source symlink");
        let roots = vec![fs::canonicalize(&skill_root).expect("canonical skill root")];

        let resolved = resolve_worktree_read_file_skill_alias(
            READ_FILE_TOOL,
            shared_workspace.path(),
            worktree.path(),
            &roots,
            ".agents/skills/example/assets/escaped.txt",
        );

        assert_eq!(resolved, None);
    }

    #[test]
    fn read_file_skill_alias_keeps_existing_worktree_file_and_rejects_unrouted_inputs() {
        let (shared_workspace, worktree, skill_root, _) = worktree_skill_alias_fixture();
        let roots = vec![fs::canonicalize(&skill_root).expect("canonical skill root")];
        let worktree_skill = worktree.path().join(".agents/skills/example/SKILL.md");
        fs::create_dir_all(worktree_skill.parent().expect("worktree skill parent"))
            .expect("create worktree skill directory");
        fs::write(&worktree_skill, "worktree instructions").expect("write worktree skill");

        let existing = resolve_worktree_read_file_skill_alias(
            READ_FILE_TOOL,
            shared_workspace.path(),
            worktree.path(),
            &roots,
            ".agents/skills/example/SKILL.md",
        );
        let traversal = resolve_worktree_read_file_skill_alias(
            READ_FILE_TOOL,
            shared_workspace.path(),
            worktree.path(),
            &roots,
            "../.agents/skills/example/SKILL.md",
        );
        let lookalike = resolve_worktree_read_file_skill_alias(
            READ_FILE_TOOL,
            shared_workspace.path(),
            worktree.path(),
            &roots,
            ".agents/skills-lookalike/example/SKILL.md",
        );
        let other_tool = resolve_worktree_read_file_skill_alias(
            FIND_FILES_TOOL,
            shared_workspace.path(),
            worktree.path(),
            &roots,
            ".agents/skills/example/SKILL.md",
        );

        assert_eq!(existing, None);
        assert_eq!(traversal, None);
        assert_eq!(lookalike, None);
        assert_eq!(other_tool, None);
    }

    #[cfg(unix)]
    #[test]
    fn read_file_skill_alias_rejects_worktree_symlink_components() {
        let (shared_workspace, worktree, skill_root, _) = worktree_skill_alias_fixture();
        let roots = vec![fs::canonicalize(&skill_root).expect("canonical skill root")];
        std::os::unix::fs::symlink(shared_workspace.path(), worktree.path().join(".agents"))
            .expect("create worktree symlink");

        let resolved = resolve_worktree_read_file_skill_alias(
            READ_FILE_TOOL,
            shared_workspace.path(),
            worktree.path(),
            &roots,
            ".agents/skills/example/missing.md",
        );

        assert_eq!(resolved, None);
    }

    #[test]
    fn repeated_tool_call_detector_rejects_oversized_transport_id_before_execution() {
        let mut detector = RepeatedToolCallDetector::default();
        let tool_calls = vec![NeutralToolCall {
            call_id: "x"
                .repeat(foco_tools::output_budget::TOOL_TRANSPORT_DYNAMIC_FIELD_BYTE_LIMIT + 1),
            name: SEARCH_TEXT_TOOL.to_string(),
            arguments: json!({ "query": "needle" }),
            thought_signatures: None,
        }];

        let error = detector
            .check(&tool_calls)
            .expect_err("oversized tool call id must be rejected");

        assert!(error.contains("transport limit"));
    }

    #[test]
    fn budget_tool_execution_returns_retryable_failure_for_large_read_only_output() {
        let budgeted = budget_tool_execution(
            SEARCH_TEXT_TOOL,
            ToolExecution {
                output: json!({
                    "matches": "x".repeat(
                        foco_tools::output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT
                    )
                }),
                is_error: false,
            },
        );

        assert!(budgeted.execution.is_error);
        assert_eq!(budgeted.execution.output["retryable"], true);
    }

    #[test]
    fn budget_tool_result_envelope_measures_complete_sse_record() {
        let assistant_message_id = "a".repeat(120 * 1024);
        let budgeted = budget_tool_result_envelope(
            &assistant_message_id,
            "tool-call",
            SEARCH_TEXT_TOOL,
            "2026-07-16T00:00:00Z",
            "2026-07-16T00:00:01Z",
            ToolExecution {
                output: json!({ "matches": "x".repeat(20 * 1024) }),
                is_error: false,
            },
        );
        let envelope = ToolResultBudgetEnvelope {
            event_type: "toolResult",
            assistant_message_id: &assistant_message_id,
            tool_call_id: "tool-call",
            output: &budgeted.execution.output,
            is_error: budgeted.execution.is_error,
            started_at: "2026-07-16T00:00:00Z",
            completed_at: "2026-07-16T00:00:01Z",
        };

        assert_ne!(
            budgeted.state,
            foco_tools::output_budget::ToolOutputBudgetState::WithinBudget
        );
        assert!(
            foco_tools::output_budget::serialized_json_size(&envelope)
                .expect("measure SSE tool result")
                <= foco_tools::output_budget::TOOL_EXECUTION_HARD_BYTE_LIMIT
        );
    }

    #[test]
    fn budget_tool_execution_preserves_success_for_large_retry_unsafe_output() {
        let budgeted = budget_tool_execution(
            WRITE_FILE_TOOL,
            ToolExecution {
                output: json!({
                    "result": "x".repeat(
                        foco_tools::output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT
                    )
                }),
                is_error: false,
            },
        );

        assert!(!budgeted.execution.is_error);
        assert_eq!(budgeted.execution.output["outputOmitted"], true);
        assert_eq!(budgeted.execution.output["retryUnsafe"], true);
    }

    #[test]
    fn budget_tool_execution_budgets_mixed_results_independently() {
        // One oversized read-only result becomes a recoverable error; a small sibling stays intact.
        let large = budget_tool_execution(
            SEARCH_TEXT_TOOL,
            ToolExecution {
                output: json!({
                    "matches": "x".repeat(
                        foco_tools::output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT + 1
                    )
                }),
                is_error: false,
            },
        );
        let small = budget_tool_execution(
            READ_FILE_TOOL,
            ToolExecution {
                output: json!({ "content": "ok", "path": "a.txt" }),
                is_error: false,
            },
        );

        assert!(large.execution.is_error);
        assert_eq!(large.execution.output["retryable"], true);
        assert!(!small.execution.is_error);
        assert_eq!(small.execution.output["content"], "ok");
        assert!(
            foco_tools::output_budget::serialized_json_size(&large.execution)
                .expect("measure large")
                <= foco_tools::output_budget::TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
        );
        assert!(
            foco_tools::output_budget::serialized_json_size(&small.execution)
                .expect("measure small")
                <= foco_tools::output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT
        );
    }

    #[test]
    fn repeated_tool_call_detector_does_not_treat_output_omission_as_auto_retry() {
        // Side-effect success with outputOmitted must remain a completed result; the detector only
        // fires on identical *call* batches from the model, never on budget omission shape.
        let omitted = budget_tool_execution(
            RUN_COMMAND_TOOL,
            ToolExecution {
                output: json!({
                    "stdout": "x".repeat(
                        foco_tools::output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT + 8
                    ),
                    "exitCode": 0
                }),
                is_error: false,
            },
        );
        assert!(!omitted.execution.is_error);
        assert_eq!(omitted.execution.output["outputOmitted"], true);
        assert_eq!(omitted.execution.output["retryUnsafe"], true);

        let mut detector = RepeatedToolCallDetector::default();
        let batch = vec![NeutralToolCall {
            call_id: "call-1".to_string(),
            name: RUN_COMMAND_TOOL.to_string(),
            arguments: json!({ "command": "echo", "args": ["hi"] }),
            thought_signatures: None,
        }];
        // First two identical batches are allowed (count < MAX); omission status is irrelevant.
        assert!(matches!(
            detector.check(&batch),
            Ok(ToolLoopBeforeExecutionAction::Continue)
        ));
        assert!(matches!(
            detector.check(&batch),
            Ok(ToolLoopBeforeExecutionAction::Continue)
        ));
        // Only the N-th identical batch trips the loop detector.
        let action = detector
            .check(&batch)
            .expect("identical batch should classify as recoverable loop");
        match action {
            ToolLoopBeforeExecutionAction::RecoverRepeatedBatch { message, .. } => {
                assert!(
                    message.contains("repeated the same tool call batch"),
                    "{message}"
                );
            }
            ToolLoopBeforeExecutionAction::Continue => {
                panic!("identical batch should eventually trip the loop detector");
            }
        }
    }

    #[tokio::test]
    async fn side_effect_tool_with_omitted_output_executes_once_and_stays_success() {
        // Real execute_tool path: a successful command whose captured stdout exceeds the soft
        // budget must keep is_error=false + outputOmitted, proving the side effect completed once.
        let workspace = tempfile::tempdir().expect("workspace");
        let marker = workspace.path().join("side-effect-once.marker");
        let huge_len = foco_tools::output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT + 64;

        let mcp_registry = Arc::new(McpRegistry::default());
        let first = execute_tool(
            mcp_registry.clone(),
            HookRuntime::new(mcp_registry.clone()),
            &HookConfig::default(),
            true,
            &GlobalConfig::first_run(workspace.path().to_path_buf()),
            Some(&ProviderConnectionConfig {
                kind: foco_providers::parse_provider_kind(foco_providers::OPENAI_RESPONSES_KIND)
                    .expect("provider kind"),
                base_url: None,
                api_key: Some("test-key".to_string()),
                proxy_url: None,
                request_overrides: Vec::new(),
                model_redirects: Vec::new(),
            }),
            &WebSearchSettings::default(),
            QuestionRegistry::default(),
            mpsc::unbounded_channel().0,
            MemoryToolContext {
                enabled: false,
                workspace_path: workspace.path().to_path_buf(),
                global_memory_database_file: workspace.path().join("memory.sqlite"),
                chat_id: "chat-omit".to_string(),
                run_id: "run-omit".to_string(),
                tool_call_id: "call-omit-1".to_string(),
                target_status: MemoryStatus::Pending,
                memory_settings: MemorySettings::default(),
            },
            None,
            Vec::new(),
            Vec::new(),
            ToolResourceLockRegistry::default(),
            ToolCancellationToken::default(),
            mpsc::unbounded_channel().0,
            "assistant-omit",
            "workspace-omit",
            workspace.path(),
            workspace.path(),
            "chat-omit",
            None,
            "run-omit",
            "model-1",
            "provider-1",
            0,
            "call-omit-1",
            RUN_COMMAND_TOOL,
            json!({
                "command": "python3",
                "args": [
                    "-c",
                    format!(
                        "from pathlib import Path; Path('side-effect-once.marker').write_text('done'); print('x'*{huge_len}, end='')"
                    )
                ],
                "cwd": null,
                "timeoutMs": 30_000
            }),
        )
        .await;

        assert!(!first.execution.is_error, "{:?}", first.execution.output);
        assert_eq!(first.execution.output["outputOmitted"], true);
        assert_eq!(first.execution.output["retryUnsafe"], true);
        assert!(
            foco_tools::output_budget::serialized_json_size(&first.execution).expect("measure")
                <= foco_tools::output_budget::TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
        );
        assert_eq!(
            fs::read_to_string(&marker).expect("side effect applied once"),
            "done"
        );

        // Detector only counts model call signatures; omission must not imply auto re-execution.
        let mut detector = RepeatedToolCallDetector::default();
        let batch = vec![NeutralToolCall {
            call_id: "call-omit-1".to_string(),
            name: RUN_COMMAND_TOOL.to_string(),
            arguments: json!({
                "command": "python3",
                "args": ["-c", "print('nope')"],
                "cwd": null,
                "timeoutMs": 30_000
            }),
            thought_signatures: None,
        }];
        assert!(matches!(
            detector.check(&batch),
            Ok(ToolLoopBeforeExecutionAction::Continue)
        ));
        assert_eq!(
            fs::read_to_string(&marker).expect("file unchanged by detector"),
            "done"
        );
        let mtime_before = fs::metadata(&marker)
            .and_then(|meta| meta.modified())
            .expect("mtime");
        assert!(matches!(
            detector.check(&batch),
            Ok(ToolLoopBeforeExecutionAction::Continue)
        ));
        let mtime_after = fs::metadata(&marker)
            .and_then(|meta| meta.modified())
            .expect("mtime after");
        assert_eq!(mtime_before, mtime_after);
    }

    #[test]
    fn budget_tool_result_envelope_exact_hard_limit_boundary() {
        // Construct payload so the full SSE toolResult record is exactly at / one over 128 KiB.
        let assistant_message_id = "asst-boundary";
        let tool_call_id = "call-boundary";
        let started_at = "2026-07-16T00:00:00Z";
        let completed_at = "2026-07-16T00:00:01Z";

        let measure = |execution: &ToolExecution| {
            foco_tools::output_budget::serialized_json_size(&ToolResultBudgetEnvelope {
                event_type: "toolResult",
                assistant_message_id,
                tool_call_id,
                output: &execution.output,
                is_error: execution.is_error,
                started_at,
                completed_at,
            })
        };

        let mut content = "x".repeat(foco_tools::output_budget::TOOL_EXECUTION_HARD_BYTE_LIMIT);
        let mut over = ToolExecution {
            output: json!({ "matches": content.clone() }),
            is_error: false,
        };
        while measure(&over).expect("grow")
            > foco_tools::output_budget::TOOL_EXECUTION_HARD_BYTE_LIMIT + 1
        {
            content.pop();
            over.output = json!({ "matches": content.clone() });
        }
        while measure(&over).expect("grow")
            < foco_tools::output_budget::TOOL_EXECUTION_HARD_BYTE_LIMIT + 1
        {
            content.push('x');
            over.output = json!({ "matches": content.clone() });
        }
        assert_eq!(
            measure(&over).expect("over"),
            foco_tools::output_budget::TOOL_EXECUTION_HARD_BYTE_LIMIT + 1
        );
        content.pop();
        let at = ToolExecution {
            output: json!({ "matches": content }),
            is_error: false,
        };
        assert_eq!(
            measure(&at).expect("at"),
            foco_tools::output_budget::TOOL_EXECUTION_HARD_BYTE_LIMIT
        );

        let within = budget_tool_result_envelope(
            assistant_message_id,
            tool_call_id,
            SEARCH_TEXT_TOOL,
            started_at,
            completed_at,
            at,
        );
        // Exact hard envelope size is still under the hard gate (`>` not `>=`): only soft recovery.
        assert_eq!(
            within.state,
            foco_tools::output_budget::ToolOutputBudgetState::ReadOnlyRecoverableFailure
        );
        assert!(within.execution.is_error);
        assert_eq!(within.execution.output["reason"], "softByteLimit");
        assert!(
            measure(&within.execution).expect("within envelope")
                <= foco_tools::output_budget::TOOL_EXECUTION_HARD_BYTE_LIMIT
        );

        let budgeted_over = budget_tool_result_envelope(
            assistant_message_id,
            tool_call_id,
            SEARCH_TEXT_TOOL,
            started_at,
            completed_at,
            over,
        );
        assert_ne!(
            budgeted_over.state,
            foco_tools::output_budget::ToolOutputBudgetState::WithinBudget
        );
        assert!(budgeted_over.execution.is_error);
        assert_eq!(budgeted_over.execution.output["reason"], "hardByteLimit");
        assert!(
            measure(&budgeted_over.execution).expect("over envelope")
                <= foco_tools::output_budget::TOOL_EXECUTION_HARD_BYTE_LIMIT
        );
    }

    #[test]
    fn builtin_workspace_database_tools_are_routed_to_canonical_workspace() {
        for tool_name in [
            CREATE_TODO_GRAPH_TOOL,
            UPDATE_TODO_GRAPH_TOOL,
            GET_TODO_GRAPH_TOOL,
            CREATE_PLAN_TOOL,
            GET_PLANS_TOOL,
            UPDATE_PLAN_TOOL,
            UPDATE_PLAN_STEP_TOOL,
            READ_SPEC_TOOL,
            UPDATE_SPEC_TOOL,
        ] {
            assert!(builtin_tool_uses_workspace_database(tool_name));
        }

        for tool_name in [
            READ_FILE_TOOL,
            WRITE_FILE_TOOL,
            RUN_COMMAND_TOOL,
            GRAPH_EXPLORE_TOOL,
        ] {
            assert!(!builtin_tool_uses_workspace_database(tool_name));
        }
    }

    #[tokio::test]
    async fn isolated_create_todo_graph_writes_canonical_workspace_database() {
        let workspace = tempfile::tempdir().expect("canonical workspace");
        let isolated_workspace = tempfile::tempdir().expect("isolated workspace");
        let chat_id = format!("chat-isolated-todo-{}", unique_id("case"));

        {
            let mut database =
                WorkspaceDatabase::open_or_create(workspace.path()).expect("canonical database");
            database
                .insert_chat(&chat_id, "Isolated todo graph test")
                .expect("canonical chat");
        }

        let isolated_database_path = workspace_database_path(isolated_workspace.path());
        assert!(!isolated_database_path.exists());

        let mcp_registry = Arc::new(McpRegistry::default());
        let output = execute_tool(
            mcp_registry.clone(),
            HookRuntime::new(mcp_registry),
            &HookConfig::default(),
            true,
            &GlobalConfig::first_run(workspace.path().to_path_buf()),
            Some(&ProviderConnectionConfig {
                kind: foco_providers::parse_provider_kind(foco_providers::OPENAI_RESPONSES_KIND)
                    .expect("provider kind"),
                base_url: None,
                api_key: Some("test-key".to_string()),
                proxy_url: None,
                request_overrides: Vec::new(),
                model_redirects: Vec::new(),
            }),
            &WebSearchSettings::default(),
            QuestionRegistry::default(),
            mpsc::unbounded_channel().0,
            MemoryToolContext {
                enabled: false,
                workspace_path: workspace.path().to_path_buf(),
                global_memory_database_file: workspace.path().join("memory.sqlite"),
                chat_id: chat_id.clone(),
                run_id: "run-1".to_string(),
                tool_call_id: "call-1".to_string(),
                target_status: MemoryStatus::Pending,
                memory_settings: MemorySettings::default(),
            },
            None,
            Vec::new(),
            Vec::new(),
            ToolResourceLockRegistry::default(),
            ToolCancellationToken::default(),
            mpsc::unbounded_channel().0,
            "assistant-1",
            "workspace-1",
            workspace.path(),
            isolated_workspace.path(),
            &chat_id,
            None,
            "run-1",
            "model-1",
            "provider-1",
            0,
            "call-1",
            CREATE_TODO_GRAPH_TOOL,
            json!({
                "tasks": [{
                    "id": "task-1",
                    "title": "Task 1",
                    "status": "ready",
                    "dependsOn": [],
                    "acceptance": [],
                    "summary": "",
                    "createdAt": null,
                    "updatedAt": null,
                    "subtasks": []
                }],
                "timeoutMs": 1000
            }),
        )
        .await;

        assert!(!output.execution.is_error, "{:?}", output.execution.output);
        assert!(!isolated_database_path.exists());

        let database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("canonical database");
        let graph = database
            .todo_graph(&chat_id)
            .expect("canonical todo graph query")
            .expect("canonical todo graph");
        assert_eq!(graph.tasks.len(), 1);
        assert_eq!(graph.tasks[0].id, "task-1");
    }

    #[test]
    fn parses_read_file_external_access_decisions() {
        let answer = |selected: &str| QuestionAnswer {
            answers: vec![QuestionItemAnswer {
                id: "question-item".to_string(),
                answer: selected.to_string(),
                selected_option_value: Some(selected.to_string()),
            }],
        };

        assert_eq!(
            external_readonly_access_decision_from_answer(READ_FILE_TOOL, &answer("allow"))
                .expect("allow decision"),
            ExternalReadonlyAccessDecision::AllowOnce
        );
        assert_eq!(
            external_readonly_access_decision_from_answer(FIND_FILES_TOOL, &answer("allow_all"))
                .expect("allow all decision"),
            ExternalReadonlyAccessDecision::AllowAll
        );
        assert_eq!(
            external_readonly_access_decision_from_answer(SEARCH_TEXT_TOOL, &answer("deny"))
                .expect("deny decision"),
            ExternalReadonlyAccessDecision::Deny
        );
        assert!(
            external_readonly_access_decision_from_answer(READ_FILE_TOOL, &answer("other"))
                .is_err()
        );
    }

    #[test]
    fn tracks_read_file_external_access_by_chat() {
        let chat_id = format!("chat-external-access-test-{}", unique_id("case"));

        assert!(!chat_allows_external_readonly(&chat_id));
        allow_external_readonly_for_chat(&chat_id);
        assert!(chat_allows_external_readonly(&chat_id));
        assert!(!chat_allows_external_readonly(
            "chat-external-access-test-other"
        ));
    }

    fn external_read_file_answer(selected: &str, item_id: &str) -> QuestionAnswer {
        QuestionAnswer {
            answers: vec![QuestionItemAnswer {
                id: item_id.to_string(),
                answer: selected.to_string(),
                selected_option_value: Some(selected.to_string()),
            }],
        }
    }

    async fn answer_next_external_read_question(
        registry: QuestionRegistry,
        event_rx: &mut mpsc::UnboundedReceiver<QuestionRequest>,
        selected: &str,
    ) -> QuestionRequest {
        let request = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
            .await
            .expect("external read question event")
            .expect("external read question request");
        let item_id = request.questions[0].id.clone();
        registry
            .answer(
                &request.id,
                external_read_file_answer(selected, item_id.as_str()),
            )
            .expect("answer external read question");
        request
    }

    #[tokio::test]
    async fn read_file_external_access_skips_question_for_workspace_file() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("inside.txt"), "inside").expect("write inside");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-external-access-inside-{}", unique_id("case"));

        let allowed = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry,
            event_tx,
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-1",
            READ_FILE_TOOL,
            &json!({ "path": "inside.txt", "startLine": null, "endLine": null }),
            ToolCancellationToken::default(),
        )
        .await
        .expect("inside workspace access check");

        assert!(!allowed);
        assert!(event_rx.try_recv().is_err());
    }

    /// Absolute path inside the current execution root is internal: no ask_question and
    /// `allow_external_read_access=false` when shared root equals tool root. Guards against
    /// reclassifying internal absolute paths as external (which would reintroduce the
    /// "path must be relative to the workspace" / dual-resolver gap).
    #[tokio::test]
    async fn read_file_external_access_skips_question_for_absolute_internal_workspace_file() {
        let workspace = tempfile::tempdir().expect("workspace");
        let inside = workspace.path().join("inside-abs.txt");
        fs::write(&inside, "absolute inside").expect("write inside");
        let absolute = fs::canonicalize(&inside).expect("canonicalize inside");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-external-access-abs-inside-{}", unique_id("case"));
        let arguments = json!({
            "path": absolute.to_string_lossy(),
            "startLine": null,
            "endLine": null,
            "timeoutMs": 5000
        });

        // shared == tool: classifier must treat absolute internal path as non-external.
        let allowed = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-abs-inside",
            READ_FILE_TOOL,
            &arguments,
            ToolCancellationToken::default(),
        )
        .await
        .expect("absolute internal access check");
        assert!(
            !allowed,
            "absolute internal path must not set allow_external_read_access"
        );
        assert!(
            event_rx.try_recv().is_err(),
            "absolute internal path must not emit questionRequest"
        );

        // End-to-end execute_tool: reads without external flag and without question.
        let mcp_registry = Arc::new(McpRegistry::default());
        let output = execute_tool(
            mcp_registry.clone(),
            HookRuntime::new(mcp_registry),
            &HookConfig::default(),
            true,
            &config,
            Some(&ProviderConnectionConfig {
                kind: foco_providers::parse_provider_kind(foco_providers::OPENAI_RESPONSES_KIND)
                    .expect("provider kind"),
                base_url: None,
                api_key: Some("test-key".to_string()),
                proxy_url: None,
                request_overrides: Vec::new(),
                model_redirects: Vec::new(),
            }),
            &WebSearchSettings::default(),
            registry,
            event_tx,
            MemoryToolContext {
                enabled: false,
                workspace_path: workspace.path().to_path_buf(),
                global_memory_database_file: workspace.path().join("memory.sqlite"),
                chat_id: chat_id.clone(),
                run_id: "run-abs-inside".to_string(),
                tool_call_id: "call-abs-inside-exec".to_string(),
                target_status: MemoryStatus::Pending,
                memory_settings: MemorySettings::default(),
            },
            None,
            Vec::new(),
            Vec::new(),
            ToolResourceLockRegistry::default(),
            ToolCancellationToken::default(),
            mpsc::unbounded_channel().0,
            "assistant-1",
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            None,
            "run-abs-inside",
            "model-1",
            "provider-1",
            0,
            "call-abs-inside-exec",
            READ_FILE_TOOL,
            arguments,
        )
        .await;

        assert!(!output.execution.is_error, "{:?}", output.execution.output);
        assert_eq!(output.execution.output["content"], "1\tabsolute inside");
        assert!(
            event_rx.try_recv().is_err(),
            "execute_tool absolute internal read must not emit questionRequest"
        );
    }

    /// Workspace-surface symlink that canonicalizes outside must stay external (prompted)
    /// even when shared == tool and the absolute path string is under the workspace root.
    #[cfg(unix)]
    #[tokio::test]
    async fn read_file_external_access_treats_absolute_workspace_symlink_escape_as_external() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        let outside_file = outside.path().join("escaped.txt");
        fs::write(&outside_file, "escaped").expect("write outside");
        let link_path = workspace.path().join("escape-link.txt");
        symlink(&outside_file, &link_path).expect("create symlink");
        // Absolute path string lives under workspace; canonicalize follows the link.
        let absolute_under_workspace = fs::canonicalize(workspace.path())
            .expect("canon workspace")
            .join("escape-link.txt");

        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-external-access-symlink-escape-{}", unique_id("case"));
        let arguments = json!({
            "path": absolute_under_workspace.to_string_lossy(),
            "startLine": null,
            "endLine": null
        });

        let access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx,
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-escape",
            READ_FILE_TOOL,
            &arguments,
            ToolCancellationToken::default(),
        );
        let (request, denied) = tokio::join!(
            answer_next_external_read_question(registry, &mut event_rx, "deny"),
            access
        );
        let outside_display = outside_file.display().to_string();
        assert!(
            request.questions[0].question.contains(&outside_display)
                || request.questions[0]
                    .question
                    .contains(&absolute_under_workspace.display().to_string()),
            "symlink escape must prompt for external target"
        );
        assert!(
            denied
                .expect_err("symlink escape must not auto-allow as internal")
                .contains("user denied")
        );
    }

    #[tokio::test]
    async fn read_file_external_access_skips_question_for_exact_attachment_file() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside_dir = tempfile::tempdir().expect("outside");
        let attached = outside_dir.path().join("attached.txt");
        let sibling = outside_dir.path().join("sibling.txt");
        fs::write(&attached, "attached-secret").expect("write attached");
        fs::write(&sibling, "sibling").expect("write sibling");
        let attached_canonical = fs::canonicalize(&attached).expect("canonicalize attached");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-external-access-attachment-{}", unique_id("case"));
        let allowlist = vec![attached_canonical.clone()];

        let attached_args = json!({
            "path": attached.to_string_lossy(),
            "startLine": null,
            "endLine": null
        });
        let allowed = ensure_read_file_external_access(
            &config,
            &[],
            &allowlist,
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-1",
            READ_FILE_TOOL,
            &attached_args,
            ToolCancellationToken::default(),
        )
        .await
        .expect("attached file access check");
        assert!(allowed);
        assert!(event_rx.try_recv().is_err());

        // Sibling in same directory must still prompt.
        let sibling_args = json!({
            "path": sibling.to_string_lossy(),
            "startLine": null,
            "endLine": null
        });
        let sibling_access = ensure_read_file_external_access(
            &config,
            &[],
            &allowlist,
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-2",
            READ_FILE_TOOL,
            &sibling_args,
            ToolCancellationToken::default(),
        );
        let (question, denied) = tokio::join!(
            answer_next_external_read_question(registry.clone(), &mut event_rx, "deny"),
            sibling_access
        );
        assert!(
            question.questions[0]
                .question
                .contains(&sibling.display().to_string())
        );
        assert!(denied.expect_err("sibling denied").contains("user denied"));

        // Prefix lookalike path must not match.
        let lookalike = PathBuf::from(format!("{}-extra", attached_canonical.display()));
        let lookalike_args = json!({
            "path": lookalike.to_string_lossy(),
            "startLine": null,
            "endLine": null
        });
        let lookalike_result = ensure_read_file_external_access(
            &config,
            &[],
            &allowlist,
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-4",
            READ_FILE_TOOL,
            &lookalike_args,
            ToolCancellationToken::default(),
        )
        .await;
        match lookalike_result {
            Ok(true) => panic!("lookalike path must not auto-allow"),
            Ok(false) | Err(_) => {
                let _ = event_rx.try_recv();
            }
        }

        // Empty allowlist for another chat still asks (chat isolation at prepare time).
        let chat_b = format!("chat-external-access-attachment-b-{}", unique_id("case"));
        let other_args = json!({
            "path": attached.to_string_lossy(),
            "startLine": null,
            "endLine": null
        });
        let other_chat_access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_b,
            "call-5",
            READ_FILE_TOOL,
            &other_args,
            ToolCancellationToken::default(),
        );
        let (q, denied_b) = tokio::join!(
            answer_next_external_read_question(registry.clone(), &mut event_rx, "deny"),
            other_chat_access
        );
        assert_eq!(q.chat_id, chat_b);
        assert!(denied_b.expect_err("chat B denied").contains("user denied"));
    }

    #[test]
    fn collect_attachment_read_allowlist_exact_paths_and_skips_missing() {
        let outside = tempfile::tempdir().expect("outside");
        let present = outside.path().join("present.txt");
        fs::write(&present, "ok").expect("write");
        let missing = outside.path().join("missing.txt");
        let present_canonical = fs::canonicalize(&present).expect("canonical");

        let history = vec![foco_store::workspace::MessageRecord {
            id: "user-1".to_string(),
            chat_id: "chat-1".to_string(),
            role: "user".to_string(),
            content: "hi".to_string(),
            sequence: 0,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            metadata_json: serde_json::to_string(&json!({
                "attachments": [{
                    "id": "a1",
                    "name": "present.txt",
                    "contentType": "text/plain",
                    "sizeBytes": 2,
                    "path": present.to_string_lossy(),
                }]
            }))
            .expect("meta"),
        }];
        let current = vec![NeutralChatAttachment {
            id: "a2".to_string(),
            name: "missing.txt".to_string(),
            content_type: "text/plain".to_string(),
            size_bytes: 1,
            content_base64: None,
            path: Some(missing.to_string_lossy().to_string()),
        }];
        let allowlist = collect_attachment_read_allowlist(&history, &current, None);
        assert_eq!(allowlist, vec![present_canonical]);
    }

    #[test]
    fn collect_attachment_read_allowlist_includes_queued_and_excludes_deleted_suffix() {
        let outside = tempfile::tempdir().expect("outside");
        let kept = outside.path().join("kept.txt");
        let removed = outside.path().join("removed.txt");
        fs::write(&kept, "kept").expect("write kept");
        fs::write(&removed, "removed").expect("write removed");
        let kept_canonical = fs::canonicalize(&kept).expect("canonical kept");

        // After edit-rerun suffix delete, only kept history remains in existing_messages.
        let history = vec![foco_store::workspace::MessageRecord {
            id: "user-kept".to_string(),
            chat_id: "chat-1".to_string(),
            role: "user".to_string(),
            content: "kept turn".to_string(),
            sequence: 0,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            metadata_json: serde_json::to_string(&json!({
                "attachments": [{
                    "id": "a-kept",
                    "name": "kept.txt",
                    "contentType": "text/plain",
                    "sizeBytes": 4,
                    "path": kept.to_string_lossy(),
                }]
            }))
            .expect("meta"),
        }];
        // Queued user is excluded from history but must still contribute grants.
        let queued = foco_store::workspace::MessageRecord {
            id: "user-queued".to_string(),
            chat_id: "chat-1".to_string(),
            role: "user".to_string(),
            content: "queued turn".to_string(),
            sequence: 2,
            created_at: "2026-01-01T00:00:02Z".to_string(),
            metadata_json: serde_json::to_string(&json!({
                "attachments": [{
                    "id": "a-queued",
                    "name": "kept.txt",
                    "contentType": "text/plain",
                    "sizeBytes": 4,
                    "path": kept.to_string_lossy(),
                }]
            }))
            .expect("queued meta"),
        };
        // Removed suffix attachment is NOT in history/queued/current → must not grant.
        let allowlist = collect_attachment_read_allowlist(&history, &[], Some(&queued));
        assert_eq!(allowlist, vec![kept_canonical.clone()]);

        // Provider-visible window can be empty (e.g. after compression summary) while
        // full chat history still rebuilds the exact allowlist.
        let allowlist_from_history_only = collect_attachment_read_allowlist(&history, &[], None);
        assert_eq!(allowlist_from_history_only, vec![kept_canonical]);
        assert!(
            !allowlist_from_history_only
                .iter()
                .any(|p| p.ends_with("removed.txt")),
            "deleted suffix attachments must not grant"
        );
    }

    #[test]
    fn append_guidance_events_extends_attachment_read_allowlist() {
        let outside = tempfile::tempdir().expect("outside");
        let attached = outside.path().join("guide.txt");
        fs::write(&attached, "guidance body").expect("write");
        let attached_canonical = fs::canonicalize(&attached).expect("canonical");

        // Path attachments are normalized/canonicalized before guidance is applied
        // (push_guidance → normalized_chat_attachments).
        let guidance = GuidanceMessage {
            id: "msg-guidance-1".to_string(),
            content: "read the attached file".to_string(),
            attachments: vec![NeutralChatAttachment {
                id: "g1".to_string(),
                name: "guide.txt".to_string(),
                content_type: "text/plain".to_string(),
                size_bytes: 13,
                content_base64: None,
                path: Some(attached_canonical.display().to_string()),
            }],
            source: crate::runtime::MANUAL_GUIDANCE_SOURCE.to_string(),
            interrupted_assistant_id: None,
        };

        let mut messages = Vec::new();
        let mut sequences = Vec::new();
        let mut sources = Vec::new();
        let mut events = Vec::new();
        let mut allowlist = Vec::new();
        let sse = append_guidance_events(
            &mut messages,
            &mut sequences,
            &mut sources,
            &mut events,
            vec![guidance],
            None,
            &mut allowlist,
        );
        assert_eq!(sse.len(), 1);
        assert_eq!(allowlist, vec![attached_canonical.clone()]);

        // Exact match grant path used by ensure_read_file_external_access.
        assert!(path_is_exact_attachment_allowlist_match(
            &attached_canonical,
            &allowlist
        ));
        let sibling = outside.path().join("other.txt");
        fs::write(&sibling, "nope").expect("sibling");
        let sibling_canonical = fs::canonicalize(&sibling).expect("sibling canonical");
        assert!(!path_is_exact_attachment_allowlist_match(
            &sibling_canonical,
            &allowlist
        ));
    }

    #[tokio::test]
    async fn read_file_external_access_skips_question_for_configured_global_skill_files() {
        let workspace = tempfile::tempdir().expect("workspace");
        let profile = tempfile::tempdir().expect("profile");
        let skill_dir = profile
            .path()
            .join(".agents")
            .join("skills")
            .join("gitmemo");
        let reference_dir = skill_dir.join("references");
        fs::create_dir_all(&reference_dir).expect("reference directory");
        let skill_file = skill_dir.join("SKILL.md");
        let reference_file = reference_dir.join("details.md");
        fs::write(
            &skill_file,
            "---\nname: gitmemo\ndescription: memory\n---\n\nUse it.",
        )
        .expect("write skill file");
        fs::write(&reference_file, "details").expect("write reference file");
        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config
            .skills
            .detected
            .push(foco_store::config::SkillSettings {
                key: "global:gitmemo".to_string(),
                id: "gitmemo".to_string(),
                name: "gitmemo".to_string(),
                description: "memory".to_string(),
                path: skill_file.clone(),
                scope: SKILL_SCOPE_GLOBAL.to_string(),
                workspace_id: None,
                workspace_name: None,
            });
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-external-access-global-skill-{}", unique_id("case"));

        for (call_id, path) in [("call-1", &skill_file), ("call-2", &reference_file)] {
            let allowed = ensure_read_file_external_access(
                &config,
                &[],
                &[],
                registry.clone(),
                event_tx.clone(),
                "workspace-1",
                workspace.path(),
                workspace.path(),
                &chat_id,
                call_id,
                READ_FILE_TOOL,
                &json!({ "path": path.to_string_lossy(), "startLine": null, "endLine": null }),
                ToolCancellationToken::default(),
            )
            .await
            .expect("global skill file access check");
            assert!(allowed);
            assert!(event_rx.try_recv().is_err());
        }

        let result = execute_builtin_tool_with_context_and_options(
            workspace.path(),
            BuiltinToolContext::for_chat(Some(&chat_id)),
            READ_FILE_TOOL,
            json!({ "path": reference_file.to_string_lossy(), "startLine": null, "endLine": null }),
            None,
            None,
            true,
        );
        assert!(!result.is_error);
        assert_eq!(result.output["content"], "1\tdetails");
    }

    #[tokio::test]
    async fn read_file_external_access_skips_question_for_prompt_skill_snapshot() {
        let workspace = tempfile::tempdir().expect("workspace");
        let isolated_worktree = tempfile::tempdir().expect("isolated worktree");
        let skill_dir = workspace.path().join(".agents").join("skills").join("live");
        fs::create_dir_all(&skill_dir).expect("skill directory");
        let skill_file = skill_dir.join("SKILL.md");
        fs::write(&skill_file, "live skill").expect("write skill");
        // Stale/missing detected list: grant must come from the run snapshot only.
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let skill_root = fs::canonicalize(&skill_dir).expect("canonicalize skill");
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-external-access-snapshot-{}", unique_id("case"));

        let allowed = ensure_read_file_external_access(
            &config,
            &[skill_root],
            &[],
            registry,
            event_tx,
            "workspace-1",
            workspace.path(),
            isolated_worktree.path(),
            &chat_id,
            "call-1",
            READ_FILE_TOOL,
            &json!({ "path": skill_file.to_string_lossy(), "startLine": null, "endLine": null }),
            ToolCancellationToken::default(),
        )
        .await
        .expect("snapshot skill access check");

        assert!(allowed);
        assert!(event_rx.try_recv().is_err());
    }

    /// Isolated worktree: absolute paths under the shared workspace skip ask_question;
    /// relative paths still resolve inside the worktree; escaping symlinks and other
    /// workspaces still prompt.
    #[tokio::test]
    async fn read_file_external_access_trusts_shared_workspace_from_isolated_worktree() {
        let workspace = tempfile::tempdir().expect("shared workspace");
        let isolated_worktree = tempfile::tempdir().expect("isolated worktree");
        let other_workspace = tempfile::tempdir().expect("other workspace");

        let shared_file = workspace.path().join("shared-src.txt");
        fs::write(&shared_file, "shared content").expect("write shared file");
        let worktree_file = isolated_worktree.path().join("shared-src.txt");
        fs::write(&worktree_file, "worktree content").expect("write worktree file");
        let other_file = other_workspace.path().join("other.txt");
        fs::write(&other_file, "other workspace").expect("write other file");
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        fs::write(outside.path(), "plain outside").expect("write outside");

        // Symlink inside shared workspace that escapes outside → must not auto-allow.
        let escape_link = workspace.path().join("escape-link.txt");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.path(), &escape_link)
                .expect("create escape symlink");
        }
        #[cfg(not(unix))]
        {
            // On non-unix CI, skip symlink escape case by writing a normal shared file
            // with a distinct name so the loop below still compiles.
            let _ = &escape_link;
        }

        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-shared-workspace-trust-{}", unique_id("case"));

        // Absolute path into shared workspace → allow without question.
        let allowed = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            isolated_worktree.path(),
            &chat_id,
            "call-shared",
            READ_FILE_TOOL,
            &json!({
                "path": shared_file.to_string_lossy(),
                "startLine": null,
                "endLine": null
            }),
            ToolCancellationToken::default(),
        )
        .await
        .expect("shared workspace absolute path");
        assert!(allowed);
        assert!(event_rx.try_recv().is_err());

        // Relative path stays inside tool worktree → no external access flag.
        let relative_allowed = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            isolated_worktree.path(),
            &chat_id,
            "call-relative",
            READ_FILE_TOOL,
            &json!({
                "path": "shared-src.txt",
                "startLine": null,
                "endLine": null
            }),
            ToolCancellationToken::default(),
        )
        .await
        .expect("relative worktree path");
        assert!(!relative_allowed);
        assert!(event_rx.try_recv().is_err());

        // End-to-end: authorized external read returns shared content with line numbers.
        let result = execute_builtin_tool_with_context_and_options(
            isolated_worktree.path(),
            BuiltinToolContext::for_chat(Some(&chat_id)),
            READ_FILE_TOOL,
            json!({
                "path": shared_file.to_string_lossy(),
                "startLine": null,
                "endLine": null
            }),
            None,
            None,
            true,
        );
        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["content"], "1\tshared content");

        // Relative path from worktree tool root still reads worktree version.
        let relative_result = execute_builtin_tool_with_context_and_options(
            isolated_worktree.path(),
            BuiltinToolContext::for_chat(Some(&chat_id)),
            READ_FILE_TOOL,
            json!({
                "path": "shared-src.txt",
                "startLine": null,
                "endLine": null
            }),
            None,
            None,
            false,
        );
        assert!(!relative_result.is_error, "{:?}", relative_result.output);
        assert_eq!(relative_result.output["content"], "1\tworktree content");

        // Other workspace + plain outside still prompt.
        for (call_id, path) in [
            ("call-other", other_file.as_path()),
            ("call-plain", outside.path()),
        ] {
            let prompted_chat_id = format!("{chat_id}-{call_id}");
            let arguments = json!({
                "path": path.to_string_lossy(),
                "startLine": null,
                "endLine": null
            });
            let access = ensure_read_file_external_access(
                &config,
                &[],
                &[],
                registry.clone(),
                event_tx.clone(),
                "workspace-1",
                workspace.path(),
                isolated_worktree.path(),
                &prompted_chat_id,
                call_id,
                READ_FILE_TOOL,
                &arguments,
                ToolCancellationToken::default(),
            );
            let (request, denied) = tokio::join!(
                answer_next_external_read_question(registry.clone(), &mut event_rx, "deny"),
                access
            );
            assert!(
                request.questions[0]
                    .question
                    .contains(&path.display().to_string())
            );
            assert!(denied.expect_err("must prompt").contains("user denied"));
        }

        #[cfg(unix)]
        {
            let escape_chat = format!("{chat_id}-escape");
            let escape_arguments = json!({
                "path": escape_link.to_string_lossy(),
                "startLine": null,
                "endLine": null
            });
            let access = ensure_read_file_external_access(
                &config,
                &[],
                &[],
                registry.clone(),
                event_tx.clone(),
                "workspace-1",
                workspace.path(),
                isolated_worktree.path(),
                &escape_chat,
                "call-escape",
                READ_FILE_TOOL,
                &escape_arguments,
                ToolCancellationToken::default(),
            );
            let (request, denied) = tokio::join!(
                answer_next_external_read_question(registry, &mut event_rx, "deny"),
                access
            );
            // Canonical target is outside the shared workspace.
            let outside_display = outside.path().display().to_string();
            assert!(
                request.questions[0].question.contains(&outside_display)
                    || request.questions[0]
                        .question
                        .contains(&escape_link.display().to_string()),
                "escape symlink question should surface outside target"
            );
            assert!(
                denied
                    .expect_err("escape symlink must not auto-allow")
                    .contains("user denied")
            );
        }
    }

    /// Audit fixture: only the three restricted readonly tools participate in
    /// external-path confirmation. write/edit/run/graph/todo/spec never get a grant.
    #[tokio::test]
    async fn only_readonly_path_tools_enable_external_read_access_helper() {
        let workspace = tempfile::tempdir().expect("shared workspace");
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        fs::write(outside.path(), "secret").expect("write outside");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-audit-only-readonly-{}", unique_id("case"));
        let path = outside.path().to_string_lossy().to_string();
        let arguments = json!({ "path": path, "startLine": null, "endLine": null });

        for tool_name in [
            WRITE_FILE_TOOL,
            EDIT_FILE_TOOL,
            RUN_COMMAND_TOOL,
            GRAPH_EXPLORE_TOOL,
            CREATE_TODO_GRAPH_TOOL,
            READ_SPEC_TOOL,
        ] {
            let allowed = ensure_read_file_external_access(
                &config,
                &[],
                &[],
                registry.clone(),
                event_tx.clone(),
                "workspace-1",
                workspace.path(),
                workspace.path(),
                &chat_id,
                "call-audit",
                tool_name,
                &arguments,
                ToolCancellationToken::default(),
            )
            .await
            .expect("non-readonly tools short-circuit");
            assert!(
                !allowed,
                "{tool_name} must not receive external-read auto-grant"
            );
            assert!(event_rx.try_recv().is_err());
        }
    }

    /// Canonical membership: string-prefix lookalikes and `..` escapes are not trusted.
    #[tokio::test]
    async fn read_file_external_access_rejects_prefix_lookalike_and_parent_escape() {
        let parent = tempfile::tempdir().expect("parent");
        let workspace_name = "shared-ws";
        let lookalike_name = "shared-ws-evil";
        let workspace = parent.path().join(workspace_name);
        let lookalike = parent.path().join(lookalike_name);
        fs::create_dir_all(&workspace).expect("shared workspace");
        fs::create_dir_all(&lookalike).expect("lookalike workspace");
        let nested_worktree = workspace
            .join(".foco")
            .join("agent-worktrees")
            .join("phase-wt");
        fs::create_dir_all(&nested_worktree).expect("nested worktree");

        let shared_file = workspace.join("inside.txt");
        fs::write(&shared_file, "inside shared").expect("write shared");
        let lookalike_file = lookalike.join("evil.txt");
        fs::write(&lookalike_file, "lookalike").expect("write lookalike");
        let outside = parent.path().join("plain-outside.txt");
        fs::write(&outside, "plain outside").expect("write outside");

        let config = GlobalConfig::first_run(workspace.clone());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-prefix-boundary-{}", unique_id("case"));

        // True shared path still auto-allows.
        let shared_allowed = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            &workspace,
            &nested_worktree,
            &chat_id,
            "call-shared",
            READ_FILE_TOOL,
            &json!({
                "path": shared_file.to_string_lossy(),
                "startLine": null,
                "endLine": null
            }),
            ToolCancellationToken::default(),
        )
        .await
        .expect("shared path");
        assert!(shared_allowed);
        assert!(event_rx.try_recv().is_err());

        // Prefix-similar directory name must still prompt (component starts_with, not str prefix).
        let lookalike_chat = format!("{chat_id}-lookalike");
        let lookalike_arguments = json!({
            "path": lookalike_file.to_string_lossy(),
            "startLine": null,
            "endLine": null
        });
        let lookalike_access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            &workspace,
            &nested_worktree,
            &lookalike_chat,
            "call-lookalike",
            READ_FILE_TOOL,
            &lookalike_arguments,
            ToolCancellationToken::default(),
        );
        let (request, denied) = tokio::join!(
            answer_next_external_read_question(registry.clone(), &mut event_rx, "deny"),
            lookalike_access
        );
        assert!(
            request.questions[0]
                .question
                .contains(&lookalike_file.display().to_string())
        );
        assert!(
            denied
                .expect_err("lookalike must prompt")
                .contains("user denied")
        );

        // Absolute path with `..` that lands outside shared after canonicalize still prompts.
        let escaped_via_parent = workspace.join("..").join("plain-outside.txt");
        let escape_chat = format!("{chat_id}-parent");
        let escape_arguments = json!({
            "path": escaped_via_parent.to_string_lossy(),
            "startLine": null,
            "endLine": null
        });
        let escape_access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            &workspace,
            &nested_worktree,
            &escape_chat,
            "call-parent",
            READ_FILE_TOOL,
            &escape_arguments,
            ToolCancellationToken::default(),
        );
        let (request, denied) = tokio::join!(
            answer_next_external_read_question(registry, &mut event_rx, "deny"),
            escape_access
        );
        let outside_display = outside.display().to_string();
        assert!(
            request.questions[0].question.contains(&outside_display)
                || request.questions[0]
                    .question
                    .contains(&escaped_via_parent.display().to_string()),
            "parent-escape question should surface outside target"
        );
        assert!(
            denied
                .expect_err(".. escape must not auto-allow")
                .contains("user denied")
        );
    }

    /// Full execute_tool chain: shared absolute path from isolated worktree, no questionRequest.
    #[tokio::test]
    async fn execute_tool_reads_shared_workspace_from_isolated_worktree_without_question() {
        let workspace = tempfile::tempdir().expect("shared workspace");
        let worktree_dir = workspace
            .path()
            .join(".foco")
            .join("agent-worktrees")
            .join("exec-wt");
        fs::create_dir_all(&worktree_dir).expect("nested worktree");
        let shared_file = workspace.path().join("shared-note.txt");
        fs::write(&shared_file, "shared via execute_tool").expect("write shared");
        fs::write(worktree_dir.join("shared-note.txt"), "worktree shadow").expect("write worktree");

        let chat_id = format!("chat-execute-tool-shared-{}", unique_id("case"));
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let mcp_registry = Arc::new(McpRegistry::default());
        let output = execute_tool(
            mcp_registry.clone(),
            HookRuntime::new(mcp_registry),
            &HookConfig::default(),
            true,
            &GlobalConfig::first_run(workspace.path().to_path_buf()),
            Some(&ProviderConnectionConfig {
                kind: foco_providers::parse_provider_kind(foco_providers::OPENAI_RESPONSES_KIND)
                    .expect("provider kind"),
                base_url: None,
                api_key: Some("test-key".to_string()),
                proxy_url: None,
                request_overrides: Vec::new(),
                model_redirects: Vec::new(),
            }),
            &WebSearchSettings::default(),
            registry,
            event_tx,
            MemoryToolContext {
                enabled: false,
                workspace_path: workspace.path().to_path_buf(),
                global_memory_database_file: workspace.path().join("memory.sqlite"),
                chat_id: chat_id.clone(),
                run_id: "run-shared-read".to_string(),
                tool_call_id: "call-shared-read".to_string(),
                target_status: MemoryStatus::Pending,
                memory_settings: MemorySettings::default(),
            },
            None,
            Vec::new(),
            Vec::new(),
            ToolResourceLockRegistry::default(),
            ToolCancellationToken::default(),
            mpsc::unbounded_channel().0,
            "assistant-1",
            "workspace-1",
            workspace.path(),
            &worktree_dir,
            &chat_id,
            None,
            "run-shared-read",
            "model-1",
            "provider-1",
            0,
            "call-shared-read",
            READ_FILE_TOOL,
            json!({
                "path": shared_file.to_string_lossy(),
                "startLine": null,
                "endLine": null,
                "timeoutMs": 5000
            }),
        )
        .await;

        assert!(!output.execution.is_error, "{:?}", output.execution.output);
        assert_eq!(
            output.execution.output["content"],
            "1\tshared via execute_tool"
        );
        assert!(
            event_rx.try_recv().is_err(),
            "shared workspace read must not emit questionRequest"
        );

        // Relative path still resolves under the worktree execution root.
        let relative = execute_builtin_tool_with_context_and_options(
            &worktree_dir,
            BuiltinToolContext::for_chat(Some(&chat_id)),
            READ_FILE_TOOL,
            json!({
                "path": "shared-note.txt",
                "startLine": null,
                "endLine": null
            }),
            None,
            None,
            false,
        );
        assert!(!relative.is_error, "{:?}", relative.output);
        assert_eq!(relative.output["content"], "1\tworktree shadow");
    }

    /// Isolation: write/edit/run from a worktree root cannot modify shared via absolute/parent paths.
    /// `allow_external_read_access=true` must not become a write grant.
    #[tokio::test]
    async fn isolated_worktree_write_edit_run_cannot_escape_to_shared_workspace() {
        let workspace = tempfile::tempdir().expect("shared workspace");
        let worktree_dir = workspace
            .path()
            .join(".foco")
            .join("agent-worktrees")
            .join("write-wt");
        fs::create_dir_all(&worktree_dir).expect("nested worktree");
        let shared_file = workspace.path().join("protected.txt");
        fs::write(&shared_file, "do not touch").expect("write shared");
        fs::write(worktree_dir.join("local.txt"), "worktree local").expect("write worktree");
        let chat_id = format!("chat-write-isolation-{}", unique_id("case"));

        // Even with external-read flag true (mis-set), write_file stays on execution root.
        let write_absolute = execute_builtin_tool_with_context_and_options(
            &worktree_dir,
            BuiltinToolContext::for_chat(Some(&chat_id)),
            WRITE_FILE_TOOL,
            json!({
                "path": shared_file.to_string_lossy(),
                "content": "hacked absolute",
                "startLine": null,
                "endLine": null
            }),
            None,
            None,
            true,
        );
        assert!(write_absolute.is_error, "{:?}", write_absolute.output);

        let write_parent = execute_builtin_tool_with_context_and_options(
            &worktree_dir,
            BuiltinToolContext::for_chat(Some(&chat_id)),
            WRITE_FILE_TOOL,
            json!({
                "path": "../../../protected.txt",
                "content": "hacked parent",
                "startLine": null,
                "endLine": null
            }),
            None,
            None,
            true,
        );
        assert!(write_parent.is_error, "{:?}", write_parent.output);

        let edit_absolute = execute_builtin_tool_with_context_and_options(
            &worktree_dir,
            BuiltinToolContext::for_chat(Some(&chat_id)),
            EDIT_FILE_TOOL,
            json!({
                "path": shared_file.to_string_lossy(),
                "oldStr": "do not touch",
                "newStr": "edited absolute",
                "replaceAll": false
            }),
            None,
            None,
            true,
        );
        assert!(edit_absolute.is_error, "{:?}", edit_absolute.output);

        let run_escape = execute_builtin_tool_with_context_and_options(
            &worktree_dir,
            BuiltinToolContext::for_chat(Some(&chat_id)),
            RUN_COMMAND_TOOL,
            json!({
                "command": "true",
                "args": [],
                "cwd": workspace.path().to_string_lossy(),
                "timeoutMs": 5000
            }),
            None,
            None,
            true,
        );
        assert!(run_escape.is_error, "{:?}", run_escape.output);

        // find_files may list an absolute shared path only when the app grants
        // allow_external_read_access (same flag as read_file). Without the grant
        // it stays execution-root-relative. With the grant it must not affect
        // write isolation of shared content.
        let find_without_grant = execute_builtin_tool_with_context_and_options(
            &worktree_dir,
            BuiltinToolContext::for_chat(Some(&chat_id)),
            FIND_FILES_TOOL,
            json!({
                "path": workspace.path().to_string_lossy(),
                "include": null,
                "exclude": null,
                "timeoutMs": 5000
            }),
            None,
            None,
            false,
        );
        assert!(
            find_without_grant.is_error,
            "find_files without external grant must not escape: {:?}",
            find_without_grant.output
        );

        let find_with_grant = execute_builtin_tool_with_context_and_options(
            &worktree_dir,
            BuiltinToolContext::for_chat(Some(&chat_id)),
            FIND_FILES_TOOL,
            json!({
                "path": workspace.path().to_string_lossy(),
                "include": ["protected.txt"],
                "exclude": null,
                "timeoutMs": 5000
            }),
            None,
            None,
            true,
        );
        assert!(
            !find_with_grant.is_error,
            "find_files with external grant may list shared: {:?}",
            find_with_grant.output
        );

        assert_eq!(
            fs::read_to_string(&shared_file).expect("read shared after attempts"),
            "do not touch",
            "shared file must remain unmodified"
        );
    }

    /// Plan isolated worktree + live prompt snapshot: `.agents` SKILL.md and
    /// `.claude` nested references skip ask_question. Files under the shared
    /// workspace (including disabled skill paths) are trusted without prompting.
    /// Other-workspace skills and plain external files still prompt.
    #[tokio::test]
    async fn read_file_external_access_plan_worktree_uses_prompt_skill_snapshot_boundaries() {
        let workspace = tempfile::tempdir().expect("workspace");
        let profile = tempfile::tempdir().expect("profile");
        let isolated_worktree = tempfile::tempdir().expect("isolated worktree");
        let other_workspace = tempfile::tempdir().expect("other workspace");

        let agents_skill_dir = workspace
            .path()
            .join(".agents")
            .join("skills")
            .join("build");
        let agents_reference_dir = agents_skill_dir.join("references");
        fs::create_dir_all(&agents_reference_dir).expect("agents skill refs");
        let agents_skill_file = agents_skill_dir.join("SKILL.md");
        let agents_reference_file = agents_reference_dir.join("notes.md");
        fs::write(
            &agents_skill_file,
            "---\nname: build\ndescription: build helpers\n---\n\nUse build.",
        )
        .expect("write agents skill");
        fs::write(&agents_reference_file, "agents notes").expect("write agents ref");

        let claude_skill_dir = workspace
            .path()
            .join(".claude")
            .join("skills")
            .join("deploy");
        let claude_reference_dir = claude_skill_dir.join("references");
        fs::create_dir_all(&claude_reference_dir).expect("claude skill refs");
        let claude_skill_file = claude_skill_dir.join("SKILL.md");
        let claude_reference_file = claude_reference_dir.join("details.md");
        fs::write(
            &claude_skill_file,
            "---\nname: deploy\ndescription: deploy helpers\n---\n\nUse deploy.",
        )
        .expect("write claude skill");
        fs::write(&claude_reference_file, "claude details").expect("write claude ref");

        let disabled_skill_dir = workspace
            .path()
            .join(".agents")
            .join("skills")
            .join("legacy");
        fs::create_dir_all(&disabled_skill_dir).expect("disabled skill dir");
        let disabled_skill_file = disabled_skill_dir.join("SKILL.md");
        fs::write(
            &disabled_skill_file,
            "---\nname: legacy\ndescription: disabled skill\n---\n\nDo not use.",
        )
        .expect("write disabled skill");

        let other_skill_dir = other_workspace
            .path()
            .join(".agents")
            .join("skills")
            .join("foreign");
        fs::create_dir_all(&other_skill_dir).expect("other skill dir");
        let other_skill_file = other_skill_dir.join("SKILL.md");
        fs::write(
            &other_skill_file,
            "---\nname: foreign\ndescription: other workspace\n---\n\nForeign.",
        )
        .expect("write other skill");

        let plain_outside = tempfile::NamedTempFile::new().expect("outside file");
        fs::write(plain_outside.path(), "plain outside").expect("write outside");

        let workspace_id = "workspace-plan-skills";
        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config.workspaces[0].id = workspace_id.to_string();
        config.workspaces[0].name = "Plan Skills".to_string();
        config.workspaces[0].path = workspace.path().to_path_buf();
        // Stale detected list must not authorize; grants come from the snapshot.
        config.skills.detected.clear();

        let snapshot_with_legacy =
            available_skills_snapshot_for_workspace(profile.path(), &config, workspace_id);
        assert!(
            snapshot_with_legacy
                .prompt_entries
                .iter()
                .any(|entry| entry.name == "legacy"),
            "legacy skill should be discoverable before disable"
        );
        assert_eq!(snapshot_with_legacy.prompt_entries.len(), 3);

        let legacy_key = format!("workspace:{workspace_id}:legacy");
        config.skills.disabled.push(legacy_key);
        let snapshot =
            available_skills_snapshot_for_workspace(profile.path(), &config, workspace_id);
        assert_eq!(snapshot.prompt_entries.len(), 2);
        assert!(
            snapshot
                .prompt_entries
                .iter()
                .all(|entry| entry.name == "build" || entry.name == "deploy")
        );

        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-plan-worktree-skill-snapshot-{}", unique_id("case"));

        for (call_id, path) in [
            ("call-agents-skill", agents_skill_file.as_path()),
            ("call-agents-ref", agents_reference_file.as_path()),
            ("call-claude-skill", claude_skill_file.as_path()),
            ("call-claude-ref", claude_reference_file.as_path()),
        ] {
            let allowed = ensure_read_file_external_access(
                &config,
                &snapshot.read_root_dirs,
                &[],
                registry.clone(),
                event_tx.clone(),
                workspace_id,
                workspace.path(),
                isolated_worktree.path(),
                &chat_id,
                call_id,
                READ_FILE_TOOL,
                &json!({
                    "path": path.to_string_lossy(),
                    "startLine": null,
                    "endLine": null
                }),
                ToolCancellationToken::default(),
            )
            .await
            .expect("granted skill path should not prompt");
            assert!(allowed, "{call_id} should be granted");
            assert!(
                event_rx.try_recv().is_err(),
                "{call_id} must not emit ask_question"
            );
        }

        // Disabled location removes `.claude` roots from the live snapshot.
        let claude_location_id = format!("workspace:{workspace_id}:claude");
        config.skills.disabled_locations.push(claude_location_id);
        let agents_only_snapshot =
            available_skills_snapshot_for_workspace(profile.path(), &config, workspace_id);
        assert_eq!(agents_only_snapshot.prompt_entries.len(), 1);
        assert_eq!(agents_only_snapshot.prompt_entries[0].name, "build");
        assert_eq!(agents_only_snapshot.read_root_dirs.len(), 1);

        // Shared-workspace paths are trusted even when skill snapshot no longer
        // lists them (disabled skill key / disabled location). Skill grants are
        // not required for in-workspace files.
        for (call_id, path) in [
            ("call-disabled-skill", disabled_skill_file.as_path()),
            ("call-disabled-loc", claude_reference_file.as_path()),
        ] {
            let allowed = ensure_read_file_external_access(
                &config,
                &agents_only_snapshot.read_root_dirs,
                &[],
                registry.clone(),
                event_tx.clone(),
                workspace_id,
                workspace.path(),
                isolated_worktree.path(),
                &format!("{chat_id}-{call_id}"),
                call_id,
                READ_FILE_TOOL,
                &json!({
                    "path": path.to_string_lossy(),
                    "startLine": null,
                    "endLine": null
                }),
                ToolCancellationToken::default(),
            )
            .await
            .expect("shared workspace path should not prompt");
            assert!(
                allowed,
                "{call_id} should be granted via shared workspace trust"
            );
            assert!(
                event_rx.try_recv().is_err(),
                "{call_id} must not emit ask_question"
            );
        }

        // Truly external targets still require user confirmation.
        for (call_id, path) in [
            ("call-other-ws", other_skill_file.as_path()),
            ("call-plain", plain_outside.path()),
        ] {
            let prompted_chat_id = format!("{chat_id}-{call_id}");
            let arguments = json!({
                "path": path.to_string_lossy(),
                "startLine": null,
                "endLine": null
            });
            let access = ensure_read_file_external_access(
                &config,
                &agents_only_snapshot.read_root_dirs,
                &[],
                registry.clone(),
                event_tx.clone(),
                workspace_id,
                workspace.path(),
                isolated_worktree.path(),
                &prompted_chat_id,
                call_id,
                READ_FILE_TOOL,
                &arguments,
                ToolCancellationToken::default(),
            );

            let (request, denied) = tokio::join!(
                answer_next_external_read_question(registry.clone(), &mut event_rx, "deny"),
                access
            );
            assert!(
                request.questions[0]
                    .question
                    .contains(&path.display().to_string()),
                "{call_id} question should mention path"
            );
            assert!(
                denied
                    .expect_err("external path should prompt and deny")
                    .contains("user denied"),
                "{call_id} should be denied after ask_question"
            );
        }

        // Nested claude resource is readable under a worktree tool root when
        // the prompt snapshot still includes that skill.
        config.skills.disabled_locations.clear();
        let full_snapshot =
            available_skills_snapshot_for_workspace(profile.path(), &config, workspace_id);
        let allowed = ensure_read_file_external_access(
            &config,
            &full_snapshot.read_root_dirs,
            &[],
            registry,
            event_tx,
            workspace_id,
            workspace.path(),
            isolated_worktree.path(),
            &format!("{chat_id}-final"),
            "call-final-claude-ref",
            READ_FILE_TOOL,
            &json!({
                "path": claude_reference_file.to_string_lossy(),
                "startLine": null,
                "endLine": null
            }),
            ToolCancellationToken::default(),
        )
        .await
        .expect("re-enabled claude ref");
        assert!(allowed);
        assert!(event_rx.try_recv().is_err());

        let result = execute_builtin_tool_with_context_and_options(
            isolated_worktree.path(),
            BuiltinToolContext::for_chat(Some(&chat_id)),
            READ_FILE_TOOL,
            json!({
                "path": claude_reference_file.to_string_lossy(),
                "startLine": null,
                "endLine": null
            }),
            None,
            None,
            true,
        );
        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["content"], "1\tclaude details");
    }

    #[tokio::test]
    async fn read_file_external_access_skips_question_for_current_workspace_skill() {
        let workspace = tempfile::tempdir().expect("workspace");
        let isolated_worktree = tempfile::tempdir().expect("isolated worktree");
        let other_workspace = tempfile::tempdir().expect("other workspace");
        let current_skill_dir = workspace
            .path()
            .join(".agents")
            .join("skills")
            .join("build");
        let other_skill_dir = other_workspace
            .path()
            .join(".agents")
            .join("skills")
            .join("deploy");
        fs::create_dir_all(&current_skill_dir).expect("current skill directory");
        fs::create_dir_all(&other_skill_dir).expect("other skill directory");
        let current_skill_file = current_skill_dir.join("SKILL.md");
        let other_skill_file = other_skill_dir.join("SKILL.md");
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        fs::write(&current_skill_file, "current skill").expect("write current skill");
        fs::write(&other_skill_file, "other skill").expect("write other skill");
        fs::write(outside.path(), "plain outside").expect("write outside");

        let workspace_id = "workspace-current";
        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config
            .skills
            .detected
            .push(foco_store::config::SkillSettings {
                key: "workspace:workspace-current:build".to_string(),
                id: "build".to_string(),
                name: "build".to_string(),
                description: "build".to_string(),
                path: current_skill_file.clone(),
                scope: SKILL_SCOPE_WORKSPACE.to_string(),
                workspace_id: Some(workspace_id.to_string()),
                workspace_name: Some("Current".to_string()),
            });
        config
            .skills
            .detected
            .push(foco_store::config::SkillSettings {
                key: "workspace:workspace-other:deploy".to_string(),
                id: "deploy".to_string(),
                name: "deploy".to_string(),
                description: "deploy".to_string(),
                path: other_skill_file.clone(),
                scope: SKILL_SCOPE_WORKSPACE.to_string(),
                workspace_id: Some("workspace-other".to_string()),
                workspace_name: Some("Other".to_string()),
            });

        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-external-access-workspace-skill-{}", unique_id("case"));
        let allowed = ensure_read_file_external_access(
            &config, &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            workspace_id,
            workspace.path(),
            isolated_worktree.path(),
            &chat_id,
            "call-1",
            READ_FILE_TOOL,
            &json!({ "path": current_skill_file.to_string_lossy(), "startLine": null, "endLine": null }),
            ToolCancellationToken::default(),
        )
        .await
        .expect("current workspace skill access check");
        assert!(allowed);
        assert!(event_rx.try_recv().is_err());

        for (call_id, path) in [
            ("call-2", other_skill_file.as_path()),
            ("call-3", outside.path()),
        ] {
            let prompted_chat_id = format!("{chat_id}-{call_id}");
            let arguments =
                json!({ "path": path.to_string_lossy(), "startLine": null, "endLine": null });
            let access = ensure_read_file_external_access(
                &config,
                &[],
                &[],
                registry.clone(),
                event_tx.clone(),
                workspace_id,
                workspace.path(),
                isolated_worktree.path(),
                &prompted_chat_id,
                call_id,
                READ_FILE_TOOL,
                &arguments,
                ToolCancellationToken::default(),
            );

            let (request, denied) = tokio::join!(
                answer_next_external_read_question(registry.clone(), &mut event_rx, "deny"),
                access
            );
            assert!(
                request.questions[0]
                    .question
                    .contains(&path.display().to_string())
            );
            assert!(
                denied
                    .expect_err("external path should prompt and deny")
                    .contains("user denied")
            );
        }
    }

    #[tokio::test]
    async fn read_file_external_access_allows_once_and_reads_file() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        fs::write(outside.path(), "outside once").expect("write outside");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-external-access-allow-{}", unique_id("case"));
        let path = outside.path().to_string_lossy().to_string();
        let arguments = json!({ "path": path, "startLine": null, "endLine": null });
        let access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx,
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-1",
            READ_FILE_TOOL,
            &arguments,
            ToolCancellationToken::default(),
        );

        let (_, allowed) = tokio::join!(
            answer_next_external_read_question(registry, &mut event_rx, "allow"),
            access
        );
        assert!(allowed.expect("allow once access"));

        let result = execute_builtin_tool_with_context_and_options(
            workspace.path(),
            BuiltinToolContext::for_chat(Some(&chat_id)),
            READ_FILE_TOOL,
            json!({ "path": outside.path().to_string_lossy(), "startLine": null, "endLine": null }),
            None,
            None,
            true,
        );
        assert!(!result.is_error);
        assert_eq!(result.output["content"], "1\toutside once");
    }

    #[tokio::test]
    async fn read_file_external_access_denies_without_reading_file() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        fs::write(outside.path(), "outside denied").expect("write outside");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-external-access-deny-{}", unique_id("case"));
        let path = outside.path().to_string_lossy().to_string();
        let arguments = json!({ "path": path, "startLine": null, "endLine": null });
        let access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx,
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-1",
            READ_FILE_TOOL,
            &arguments,
            ToolCancellationToken::default(),
        );

        let (_, denied) = tokio::join!(
            answer_next_external_read_question(registry, &mut event_rx, "deny"),
            access
        );
        let error = denied.expect_err("deny should block access");
        assert!(error.contains("user denied"));

        let result = execute_builtin_tool_with_context_and_options(
            workspace.path(),
            BuiltinToolContext::for_chat(Some(&chat_id)),
            READ_FILE_TOOL,
            json!({ "path": outside.path().to_string_lossy(), "startLine": null, "endLine": null }),
            None,
            None,
            false,
        );
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn read_file_external_access_allow_all_skips_second_question() {
        let workspace = tempfile::tempdir().expect("workspace");
        let first = tempfile::NamedTempFile::new().expect("first outside file");
        let second = tempfile::NamedTempFile::new().expect("second outside file");
        fs::write(first.path(), "first outside").expect("write first outside");
        fs::write(second.path(), "second outside").expect("write second outside");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-external-access-all-{}", unique_id("case"));
        let first_path = first.path().to_string_lossy().to_string();
        let first_arguments = json!({ "path": first_path, "startLine": null, "endLine": null });
        let access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-1",
            READ_FILE_TOOL,
            &first_arguments,
            ToolCancellationToken::default(),
        );

        let (request, allowed) = tokio::join!(
            answer_next_external_read_question(registry.clone(), &mut event_rx, "allow_all"),
            access
        );
        assert!(
            request.questions[0]
                .question
                .contains(&first.path().display().to_string())
        );
        assert!(allowed.expect("allow all access"));

        let second_arguments =
            json!({ "path": second.path().to_string_lossy(), "startLine": null, "endLine": null });
        let second_allowed = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry,
            event_tx,
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-2",
            READ_FILE_TOOL,
            &second_arguments,
            ToolCancellationToken::default(),
        )
        .await
        .expect("second outside access check");

        assert!(second_allowed);
        assert!(event_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn read_file_external_access_serializes_concurrent_questions_for_chat() {
        let workspace = tempfile::tempdir().expect("workspace");
        let first = tempfile::NamedTempFile::new().expect("first outside file");
        let second = tempfile::NamedTempFile::new().expect("second outside file");
        let third = tempfile::NamedTempFile::new().expect("third outside file");
        fs::write(first.path(), "first outside").expect("write first outside");
        fs::write(second.path(), "second outside").expect("write second outside");
        fs::write(third.path(), "third outside").expect("write third outside");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-external-access-concurrent-{}", unique_id("case"));
        let first_arguments =
            json!({ "path": first.path().to_string_lossy(), "startLine": null, "endLine": null });
        let second_arguments =
            json!({ "path": second.path().to_string_lossy(), "startLine": null, "endLine": null });
        let third_arguments =
            json!({ "path": third.path().to_string_lossy(), "startLine": null, "endLine": null });

        let first_access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-1",
            READ_FILE_TOOL,
            &first_arguments,
            ToolCancellationToken::default(),
        );
        let second_access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-2",
            READ_FILE_TOOL,
            &second_arguments,
            ToolCancellationToken::default(),
        );
        let third_access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-3",
            READ_FILE_TOOL,
            &third_arguments,
            ToolCancellationToken::default(),
        );

        let answer_two_questions = async {
            let first_request = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .expect("first external read question event")
                .expect("first external read question request");
            assert!(event_rx.try_recv().is_err());
            let first_item_id = first_request.questions[0].id.clone();
            registry
                .answer(
                    &first_request.id,
                    external_read_file_answer("allow", first_item_id.as_str()),
                )
                .expect("answer first external read question");

            let second_request =
                answer_next_external_read_question(registry.clone(), &mut event_rx, "allow_all")
                    .await;
            (first_request, second_request)
        };

        let ((first_request, second_request), first_allowed, second_allowed, third_allowed) = tokio::join!(
            answer_two_questions,
            first_access,
            second_access,
            third_access
        );

        assert_ne!(first_request.id, second_request.id);
        assert!(first_allowed.expect("first concurrent access"));
        assert!(second_allowed.expect("second concurrent access"));
        assert!(third_allowed.expect("third concurrent access"));
        assert!(chat_allows_external_readonly(&chat_id));
        assert!(event_rx.try_recv().is_err());
    }

    /// find_files / search_text share the same confirmation flow as read_file.
    #[tokio::test]
    async fn find_files_and_search_text_external_access_prompt_and_deny() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside_dir = tempfile::tempdir().expect("outside dir");
        fs::write(outside_dir.path().join("hit.txt"), "needle-outside").expect("write outside");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-find-search-deny-{}", unique_id("case"));
        let outside_path = outside_dir.path().to_string_lossy().to_string();
        let find_arguments = json!({
            "path": outside_path,
            "include": null,
            "exclude": null,
            "timeoutMs": 5000
        });

        let find_access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-find",
            FIND_FILES_TOOL,
            &find_arguments,
            ToolCancellationToken::default(),
        );
        let (find_request, find_denied) = tokio::join!(
            answer_next_external_read_question(registry.clone(), &mut event_rx, "deny"),
            find_access
        );
        assert!(find_request.questions[0].question.contains(FIND_FILES_TOOL));
        assert!(
            find_request.questions[0]
                .question
                .contains(&outside_dir.path().display().to_string())
        );
        let find_error = find_denied.expect_err("deny find_files");
        assert!(find_error.contains("user denied"));
        assert!(find_error.contains(FIND_FILES_TOOL));

        let search_chat = format!("{chat_id}-search");
        let search_arguments = json!({
            "query": "needle",
            "path": outside_path,
            "continuation": null,
            "timeoutMs": 5000
        });
        let search_access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx,
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &search_chat,
            "call-search",
            SEARCH_TEXT_TOOL,
            &search_arguments,
            ToolCancellationToken::default(),
        );
        let (search_request, search_denied) = tokio::join!(
            answer_next_external_read_question(registry, &mut event_rx, "deny"),
            search_access
        );
        assert!(
            search_request.questions[0]
                .question
                .contains(SEARCH_TEXT_TOOL)
        );
        let search_error = search_denied.expect_err("deny search_text");
        assert!(search_error.contains("user denied"));
        assert!(search_error.contains(SEARCH_TEXT_TOOL));
    }

    /// allow_all on one readonly tool skips confirmation for the other two in the same chat.
    #[tokio::test]
    async fn external_readonly_allow_all_covers_read_find_search() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside_dir = tempfile::tempdir().expect("outside dir");
        let outside_file = outside_dir.path().join("note.txt");
        fs::write(&outside_file, "shared-grant-body").expect("write outside");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-allow-all-three-{}", unique_id("case"));
        let outside_dir_path = outside_dir.path().to_string_lossy().to_string();
        let first_arguments = json!({
            "path": outside_dir_path,
            "include": null,
            "exclude": null
        });

        let first = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-1",
            FIND_FILES_TOOL,
            &first_arguments,
            ToolCancellationToken::default(),
        );
        let (_, allowed) = tokio::join!(
            answer_next_external_read_question(registry.clone(), &mut event_rx, "allow_all"),
            first
        );
        assert!(allowed.expect("allow_all find_files"));

        for (tool_name, arguments) in [
            (
                READ_FILE_TOOL,
                json!({
                    "path": outside_file.to_string_lossy(),
                    "startLine": null,
                    "endLine": null
                }),
            ),
            (
                SEARCH_TEXT_TOOL,
                json!({
                    "query": "shared-grant",
                    "path": outside_dir.path().to_string_lossy(),
                    "continuation": null
                }),
            ),
            (
                FIND_FILES_TOOL,
                json!({
                    "path": outside_dir.path().to_string_lossy(),
                    "include": null,
                    "exclude": null
                }),
            ),
        ] {
            let second = ensure_read_file_external_access(
                &config,
                &[],
                &[],
                registry.clone(),
                event_tx.clone(),
                "workspace-1",
                workspace.path(),
                workspace.path(),
                &chat_id,
                "call-followup",
                tool_name,
                &arguments,
                ToolCancellationToken::default(),
            )
            .await
            .expect("follow-up should skip question");
            assert!(second, "{tool_name} should be covered by allow_all");
            assert!(
                event_rx.try_recv().is_err(),
                "{tool_name} must not re-prompt after allow_all"
            );
        }

        // Different chat does not inherit allow_all.
        let other_chat = format!("{chat_id}-other");
        let other_arguments = json!({
            "query": "shared-grant",
            "path": outside_dir_path,
            "continuation": null
        });
        let other_access = ensure_read_file_external_access(
            &config,
            &[],
            &[],
            registry.clone(),
            event_tx,
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &other_chat,
            "call-other",
            SEARCH_TEXT_TOOL,
            &other_arguments,
            ToolCancellationToken::default(),
        );
        let (_, other_denied) = tokio::join!(
            answer_next_external_read_question(registry, &mut event_rx, "deny"),
            other_access
        );
        assert!(
            other_denied
                .expect_err("other chat must re-prompt")
                .contains("user denied")
        );
    }

    /// Attachment exact allowlist never authorizes find_files/search_text on a parent dir.
    #[tokio::test]
    async fn attachment_allowlist_does_not_grant_find_or_search_on_parent() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside_dir = tempfile::tempdir().expect("outside dir");
        let attachment = outside_dir.path().join("attach.txt");
        fs::write(&attachment, "attach-body").expect("write attachment");
        let attachment_canonical = fs::canonicalize(&attachment).expect("canonicalize");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-attach-no-dir-{}", unique_id("case"));
        let allowlist = vec![attachment_canonical];
        let attachment_path = attachment.to_string_lossy().to_string();
        let parent_path = outside_dir.path().to_string_lossy().to_string();
        let read_arguments = json!({
            "path": attachment_path,
            "startLine": null,
            "endLine": null
        });
        let find_arguments = json!({
            "path": parent_path,
            "include": null,
            "exclude": null
        });
        let search_arguments = json!({
            "query": "attach",
            "path": parent_path,
            "continuation": null
        });

        // read_file exact hit still auto-allows.
        let read_allowed = ensure_read_file_external_access(
            &config,
            &[],
            &allowlist,
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-read",
            READ_FILE_TOOL,
            &read_arguments,
            ToolCancellationToken::default(),
        )
        .await
        .expect("attachment read");
        assert!(read_allowed);
        assert!(event_rx.try_recv().is_err());

        // find_files on the parent directory must still prompt (not attachment-granted).
        let find_access = ensure_read_file_external_access(
            &config,
            &[],
            &allowlist,
            registry.clone(),
            event_tx.clone(),
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-find",
            FIND_FILES_TOOL,
            &find_arguments,
            ToolCancellationToken::default(),
        );
        let (_, find_denied) = tokio::join!(
            answer_next_external_read_question(registry.clone(), &mut event_rx, "deny"),
            find_access
        );
        assert!(
            find_denied
                .expect_err("attachment must not grant find_files parent")
                .contains("user denied")
        );

        // search_text on the same parent also prompts.
        let search_access = ensure_read_file_external_access(
            &config,
            &[],
            &allowlist,
            registry.clone(),
            event_tx,
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-search",
            SEARCH_TEXT_TOOL,
            &search_arguments,
            ToolCancellationToken::default(),
        );
        let (_, search_denied) = tokio::join!(
            answer_next_external_read_question(registry, &mut event_rx, "deny"),
            search_access
        );
        assert!(
            search_denied
                .expect_err("attachment must not grant search_text parent")
                .contains("user denied")
        );
    }

    /// Full execute_tool: external find_files prompts; allow once lists absolute paths;
    /// deny produces zero traversal success. Shared-root absolute path from worktree
    /// skips the question (same trust as read_file).
    #[tokio::test]
    async fn execute_tool_find_files_external_and_shared_workspace() {
        let workspace = tempfile::tempdir().expect("shared workspace");
        let worktree_dir = workspace
            .path()
            .join(".foco")
            .join("agent-worktrees")
            .join("find-wt");
        fs::create_dir_all(&worktree_dir).expect("nested worktree");
        fs::write(workspace.path().join("shared-list.txt"), "shared").expect("shared file");
        let outside_dir = tempfile::tempdir().expect("outside dir");
        fs::write(outside_dir.path().join("ext.txt"), "ext").expect("outside file");

        let chat_id = format!("chat-execute-find-{}", unique_id("case"));
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let mcp_registry = Arc::new(McpRegistry::default());
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let hook_config = HookConfig::default();
        let web_search_settings = WebSearchSettings::default();
        let shared_path = workspace.path().to_string_lossy().to_string();
        let outside_path = outside_dir.path().to_string_lossy().to_string();

        // Shared absolute path from isolated worktree: no question.
        let shared_list = execute_tool(
            mcp_registry.clone(),
            HookRuntime::new(mcp_registry.clone()),
            &hook_config,
            true,
            &config,
            None,
            &web_search_settings,
            registry.clone(),
            event_tx.clone(),
            MemoryToolContext {
                enabled: false,
                workspace_path: workspace.path().to_path_buf(),
                global_memory_database_file: workspace.path().join("memory.sqlite"),
                chat_id: chat_id.clone(),
                run_id: "run-find-shared".to_string(),
                tool_call_id: "call-find-shared".to_string(),
                target_status: MemoryStatus::Pending,
                memory_settings: MemorySettings::default(),
            },
            None,
            Vec::new(),
            Vec::new(),
            ToolResourceLockRegistry::default(),
            ToolCancellationToken::default(),
            mpsc::unbounded_channel().0,
            "assistant-1",
            "workspace-1",
            workspace.path(),
            &worktree_dir,
            &chat_id,
            None,
            "run-find-shared",
            "model-1",
            "provider-1",
            0,
            "call-find-shared",
            FIND_FILES_TOOL,
            json!({
                "path": shared_path,
                "include": ["shared-list.txt"],
                "exclude": null,
                "timeoutMs": 5000
            }),
        )
        .await;
        assert!(
            !shared_list.execution.is_error,
            "{:?}",
            shared_list.execution.output
        );
        assert!(
            event_rx.try_recv().is_err(),
            "shared find_files must not emit questionRequest"
        );

        // True external: prompt then allow.
        let outside_chat = format!("{chat_id}-outside");
        let external_future = execute_tool(
            mcp_registry.clone(),
            HookRuntime::new(mcp_registry.clone()),
            &hook_config,
            true,
            &config,
            None,
            &web_search_settings,
            registry.clone(),
            event_tx.clone(),
            MemoryToolContext {
                enabled: false,
                workspace_path: workspace.path().to_path_buf(),
                global_memory_database_file: workspace.path().join("memory.sqlite"),
                chat_id: outside_chat.clone(),
                run_id: "run-find-ext".to_string(),
                tool_call_id: "call-find-ext".to_string(),
                target_status: MemoryStatus::Pending,
                memory_settings: MemorySettings::default(),
            },
            None,
            Vec::new(),
            Vec::new(),
            ToolResourceLockRegistry::default(),
            ToolCancellationToken::default(),
            mpsc::unbounded_channel().0,
            "assistant-1",
            "workspace-1",
            workspace.path(),
            &worktree_dir,
            &outside_chat,
            None,
            "run-find-ext",
            "model-1",
            "provider-1",
            0,
            "call-find-ext",
            FIND_FILES_TOOL,
            json!({
                "path": outside_path,
                "include": ["ext.txt"],
                "exclude": null,
                "timeoutMs": 5000
            }),
        );
        let (request, external_output) = tokio::join!(
            answer_next_external_read_question(registry.clone(), &mut event_rx, "allow"),
            external_future
        );
        assert!(request.questions[0].question.contains(FIND_FILES_TOOL));
        assert!(
            !external_output.execution.is_error,
            "{:?}",
            external_output.execution.output
        );
        let entries = external_output.execution.output["entries"]
            .as_array()
            .expect("entries");
        assert!(
            entries.iter().any(|entry| {
                entry["path"]
                    .as_str()
                    .is_some_and(|p| Path::new(p).is_absolute() && p.ends_with("ext.txt"))
            }),
            "external find_files should return absolute paths: {:?}",
            external_output.execution.output
        );

        // Deny: error, no success listing.
        let deny_chat = format!("{chat_id}-deny");
        let deny_future = execute_tool(
            mcp_registry.clone(),
            HookRuntime::new(mcp_registry),
            &hook_config,
            true,
            &config,
            None,
            &web_search_settings,
            registry.clone(),
            event_tx,
            MemoryToolContext {
                enabled: false,
                workspace_path: workspace.path().to_path_buf(),
                global_memory_database_file: workspace.path().join("memory.sqlite"),
                chat_id: deny_chat.clone(),
                run_id: "run-find-deny".to_string(),
                tool_call_id: "call-find-deny".to_string(),
                target_status: MemoryStatus::Pending,
                memory_settings: MemorySettings::default(),
            },
            None,
            Vec::new(),
            Vec::new(),
            ToolResourceLockRegistry::default(),
            ToolCancellationToken::default(),
            mpsc::unbounded_channel().0,
            "assistant-1",
            "workspace-1",
            workspace.path(),
            &worktree_dir,
            &deny_chat,
            None,
            "run-find-deny",
            "model-1",
            "provider-1",
            0,
            "call-find-deny",
            FIND_FILES_TOOL,
            json!({
                "path": outside_path,
                "include": null,
                "exclude": null,
                "timeoutMs": 5000
            }),
        );
        let (_, denied) = tokio::join!(
            answer_next_external_read_question(registry, &mut event_rx, "deny"),
            deny_future
        );
        assert!(denied.execution.is_error);
        assert!(
            denied.execution.output["error"]
                .as_str()
                .is_some_and(|e| e.contains("user denied")),
            "{:?}",
            denied.execution.output
        );
    }

    /// Full execute_tool: external search_text prompts; allow once matches with absolute
    /// paths; deny has no snapshot under execution workspace; continuation stays local.
    #[tokio::test]
    async fn execute_tool_search_text_external_allow_and_deny() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside_dir = tempfile::tempdir().expect("outside dir");
        fs::write(
            outside_dir.path().join("hit.txt"),
            "unique-search-token-xyz",
        )
        .expect("write");
        let chat_id = format!("chat-execute-search-{}", unique_id("case"));
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let mcp_registry = Arc::new(McpRegistry::default());
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let hook_config = HookConfig::default();
        let web_search_settings = WebSearchSettings::default();
        let outside_path = outside_dir.path().to_string_lossy().to_string();

        let allow_future = execute_tool(
            mcp_registry.clone(),
            HookRuntime::new(mcp_registry.clone()),
            &hook_config,
            true,
            &config,
            None,
            &web_search_settings,
            registry.clone(),
            event_tx.clone(),
            MemoryToolContext {
                enabled: false,
                workspace_path: workspace.path().to_path_buf(),
                global_memory_database_file: workspace.path().join("memory.sqlite"),
                chat_id: chat_id.clone(),
                run_id: "run-search-allow".to_string(),
                tool_call_id: "call-search-allow".to_string(),
                target_status: MemoryStatus::Pending,
                memory_settings: MemorySettings::default(),
            },
            None,
            Vec::new(),
            Vec::new(),
            ToolResourceLockRegistry::default(),
            ToolCancellationToken::default(),
            mpsc::unbounded_channel().0,
            "assistant-1",
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            None,
            "run-search-allow",
            "model-1",
            "provider-1",
            0,
            "call-search-allow",
            SEARCH_TEXT_TOOL,
            json!({
                "query": "unique-search-token-xyz",
                "path": outside_path,
                "continuation": null,
                "timeoutMs": 10000
            }),
        );
        let (request, allowed) = tokio::join!(
            answer_next_external_read_question(registry.clone(), &mut event_rx, "allow"),
            allow_future
        );
        assert!(request.questions[0].question.contains(SEARCH_TEXT_TOOL));
        assert!(
            !allowed.execution.is_error,
            "{:?}",
            allowed.execution.output
        );
        if let Some(full) = allowed.execution.output["fullResultPath"].as_str() {
            assert!(
                full.contains(".foco/search-results") || full.contains(".foco\\search-results"),
                "snapshot must stay under execution workspace: {full}"
            );
        }

        let deny_chat = format!("{chat_id}-deny");
        let before_snapshots = list_search_result_snapshot_count(workspace.path());
        let deny_future = execute_tool(
            mcp_registry.clone(),
            HookRuntime::new(mcp_registry),
            &hook_config,
            true,
            &config,
            None,
            &web_search_settings,
            registry.clone(),
            event_tx,
            MemoryToolContext {
                enabled: false,
                workspace_path: workspace.path().to_path_buf(),
                global_memory_database_file: workspace.path().join("memory.sqlite"),
                chat_id: deny_chat.clone(),
                run_id: "run-search-deny".to_string(),
                tool_call_id: "call-search-deny".to_string(),
                target_status: MemoryStatus::Pending,
                memory_settings: MemorySettings::default(),
            },
            None,
            Vec::new(),
            Vec::new(),
            ToolResourceLockRegistry::default(),
            ToolCancellationToken::default(),
            mpsc::unbounded_channel().0,
            "assistant-1",
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &deny_chat,
            None,
            "run-search-deny",
            "model-1",
            "provider-1",
            0,
            "call-search-deny",
            SEARCH_TEXT_TOOL,
            json!({
                "query": "unique-search-token-xyz",
                "path": outside_path,
                "continuation": null,
                "timeoutMs": 10000
            }),
        );
        let (_, denied) = tokio::join!(
            answer_next_external_read_question(registry, &mut event_rx, "deny"),
            deny_future
        );
        assert!(denied.execution.is_error);
        assert_eq!(
            list_search_result_snapshot_count(workspace.path()),
            before_snapshots,
            "denied search_text must not create a new snapshot"
        );
    }

    /// Allow-once external search_text then continue from snapshot without a second prompt.
    #[tokio::test]
    async fn execute_tool_search_text_external_continuation_skips_second_prompt() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside_dir = tempfile::tempdir().expect("outside dir");
        // Enough long matches that soft budget forces a multi-page snapshot.
        let total = 400;
        let mut content = String::new();
        for index in 0..total {
            content.push_str(&format!("needle {index} {}\n", "x".repeat(200)));
        }
        fs::write(outside_dir.path().join("big.txt"), content).expect("write big");
        let chat_id = format!("chat-execute-search-cont-{}", unique_id("case"));
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let mcp_registry = Arc::new(McpRegistry::default());
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let hook_config = HookConfig::default();
        let web_search_settings = WebSearchSettings::default();
        let outside_path = outside_dir.path().to_string_lossy().to_string();

        let initial_future = execute_tool(
            mcp_registry.clone(),
            HookRuntime::new(mcp_registry.clone()),
            &hook_config,
            true,
            &config,
            None,
            &web_search_settings,
            registry.clone(),
            event_tx.clone(),
            MemoryToolContext {
                enabled: false,
                workspace_path: workspace.path().to_path_buf(),
                global_memory_database_file: workspace.path().join("memory.sqlite"),
                chat_id: chat_id.clone(),
                run_id: "run-search-cont-1".to_string(),
                tool_call_id: "call-search-cont-1".to_string(),
                target_status: MemoryStatus::Pending,
                memory_settings: MemorySettings::default(),
            },
            None,
            Vec::new(),
            Vec::new(),
            ToolResourceLockRegistry::default(),
            ToolCancellationToken::default(),
            mpsc::unbounded_channel().0,
            "assistant-1",
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            None,
            "run-search-cont-1",
            "model-1",
            "provider-1",
            0,
            "call-search-cont-1",
            SEARCH_TEXT_TOOL,
            json!({
                "query": "needle",
                "path": outside_path,
                "continuation": null,
                "timeoutMs": 15000
            }),
        );
        let (request, initial) = tokio::join!(
            answer_next_external_read_question(registry.clone(), &mut event_rx, "allow"),
            initial_future
        );
        assert!(request.questions[0].question.contains(SEARCH_TEXT_TOOL));
        assert!(
            !initial.execution.is_error,
            "{:?}",
            initial.execution.output
        );
        assert_eq!(
            initial.execution.output["truncated"], true,
            "expected soft-truncated first page for continuation test: {:?}",
            initial.execution.output
        );
        let continuation = initial.execution.output["continuation"]
            .as_str()
            .expect("continuation token")
            .to_string();
        assert!(!continuation.trim().is_empty());
        if let Some(full) = initial.execution.output["fullResultPath"].as_str() {
            assert!(
                full.contains(".foco/search-results") || full.contains(".foco\\search-results"),
                "snapshot must stay under execution workspace: {full}"
            );
        }

        // Second page: must not emit another questionRequest (allow-once already used).
        let continue_future = execute_tool(
            mcp_registry.clone(),
            HookRuntime::new(mcp_registry),
            &hook_config,
            true,
            &config,
            None,
            &web_search_settings,
            registry,
            event_tx,
            MemoryToolContext {
                enabled: false,
                workspace_path: workspace.path().to_path_buf(),
                global_memory_database_file: workspace.path().join("memory.sqlite"),
                chat_id: chat_id.clone(),
                run_id: "run-search-cont-2".to_string(),
                tool_call_id: "call-search-cont-2".to_string(),
                target_status: MemoryStatus::Pending,
                memory_settings: MemorySettings::default(),
            },
            None,
            Vec::new(),
            Vec::new(),
            ToolResourceLockRegistry::default(),
            ToolCancellationToken::default(),
            mpsc::unbounded_channel().0,
            "assistant-1",
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            None,
            "run-search-cont-2",
            "model-1",
            "provider-1",
            0,
            "call-search-cont-2",
            SEARCH_TEXT_TOOL,
            json!({
                "query": "needle",
                "path": outside_path,
                "continuation": continuation,
                "timeoutMs": 15000
            }),
        );
        let page = continue_future.await;
        assert!(
            !page.execution.is_error,
            "continuation must not re-prompt or fail after allow once: {:?}",
            page.execution.output
        );
        assert!(
            page.execution.output["matches"]
                .as_array()
                .is_some_and(|matches| !matches.is_empty()),
            "{:?}",
            page.execution.output
        );
        // No second question should have been queued for this chat.
        assert!(
            event_rx.try_recv().is_err(),
            "continuation must not emit a second external-access question"
        );
    }

    fn list_search_result_snapshot_count(workspace_path: &Path) -> usize {
        let dir = workspace_path.join(".foco").join("search-results");
        if !dir.exists() {
            return 0;
        }
        fs::read_dir(&dir)
            .map(|entries| entries.filter_map(Result::ok).count())
            .unwrap_or(0)
    }

    /// Skill read roots auto-allow find_files / search_text the same as read_file.
    #[tokio::test]
    async fn find_files_external_access_skips_question_for_skill_root() {
        let workspace = tempfile::tempdir().expect("workspace");
        let skill_root = tempfile::tempdir().expect("skill root");
        fs::write(skill_root.path().join("SKILL.md"), "skill").expect("write skill");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let registry = QuestionRegistry::default();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let chat_id = format!("chat-skill-find-{}", unique_id("case"));
        let skill_roots = vec![fs::canonicalize(skill_root.path()).expect("canonicalize skill")];
        let skill_path = skill_root.path().to_string_lossy().to_string();
        let skill_arguments = json!({
            "path": skill_path,
            "include": null,
            "exclude": null
        });

        let allowed = ensure_read_file_external_access(
            &config,
            &skill_roots,
            &[],
            registry,
            event_tx,
            "workspace-1",
            workspace.path(),
            workspace.path(),
            &chat_id,
            "call-skill-find",
            FIND_FILES_TOOL,
            &skill_arguments,
            ToolCancellationToken::default(),
        )
        .await
        .expect("skill find_files");
        assert!(allowed);
        assert!(event_rx.try_recv().is_err());
    }

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
        mpsc::Receiver<()>,
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
                coordinator_execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
                coordinator_execution_root_path: None,
                coordinator_worktree_base_revision: None,
                coordinator_worktree_branch: None,
                coordinator_worktree_status: None,
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
        // Keep wake_rx alive so successful collaboration tools can call scheduler.wake().
        let (scheduler, wake_rx) = AgentScheduler::new();
        let context = AgentToolContext {
            workspace_id: "workspace-agent-tool-test".to_string(),
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
            scheduler,
            active_chat_runs: ActiveChatRunRegistry::default(),
        };
        (workspace, context, team_id, instance_id, task_id, wake_rx)
    }

    fn delegated_child_task_count(
        workspace_path: &std::path::Path,
        team_id: &AgentTeamId,
        parent_task_id: &AgentTaskId,
    ) -> usize {
        WorkspaceDatabase::open_or_create(workspace_path)
            .expect("database")
            .agent_tasks_for_parent(team_id, parent_task_id)
            .expect("child tasks")
            .len()
    }

    #[test]
    fn phase6_agent_tool_permission_and_payload_errors_have_codes() {
        let (workspace, context, _team_id, instance_id, _task_id, _wake_rx) =
            create_agent_tool_fixture(AgentPermissions::default());

        let no_delegate_error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-no-delegate",
            json!({
                "targetKind": "instance",
                "targetId": instance_id.to_string(),
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
        let (workspace, mut context, _team_id, instance_id, _task_id, _wake_rx) =
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
    fn agent_send_message_persists_live_guidance_then_consumes_it_when_applied() {
        let (workspace, context, team_id, instance_id, _task_id, _wake_rx) =
            create_agent_tool_fixture(AgentPermissions::default());
        let (guidance_tx, mut guidance_rx) = mpsc::unbounded_channel();
        let mut registration = context
            .active_chat_runs
            .register_agent(
                "run-agent-message-live".to_string(),
                context.workspace_id.clone(),
                "chat-agent-tool-test".to_string(),
                "assistant-agent-message-live".to_string(),
                1,
                Vec::new(),
                false,
                crate::runtime::ActiveAgentRunIdentity {
                    team_id: team_id.clone(),
                    instance_id: instance_id.clone(),
                    task_id: context
                        .associations
                        .task_id
                        .clone()
                        .expect("fixture task id"),
                    _attempt_id: AgentAttemptId::new("agent-attempt-tool-message-live")
                        .expect("attempt id"),
                },
                0,
                guidance_tx,
            )
            .expect("register active Agent run");

        let output = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_SEND_MESSAGE_TOOL,
            "call-agent-message-live",
            json!({
                "receiverInstanceId": instance_id.to_string(),
                "kind": "notification",
                "content": "apply this immediately",
                "replyToMessageId": null,
                "relatedTaskId": null,
                "timeoutMs": null,
            }),
        )
        .expect("send Agent message");
        assert_eq!(output["delivery"], "guidance");

        let guidance = guidance_rx.try_recv().expect("live guidance");
        assert_eq!(guidance.id, output["messageId"]);
        assert_eq!(guidance.content, "apply this immediately");
        assert_eq!(guidance.source, AGENT_MESSAGE_GUIDANCE_SOURCE);

        let message_id = foco_agent::AgentMessageId::new(
            output["messageId"].as_str().expect("message id string"),
        )
        .expect("Agent message id");
        let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        assert_eq!(
            database
                .agent_message(&message_id)
                .expect("message read")
                .expect("message")
                .consumed_at,
            None
        );
        assert!(
            database
                .agent_events_after(&team_id, -1)
                .expect("Agent events")
                .iter()
                .any(|event| {
                    event.event_type == "message_created"
                        && event.message_id.as_ref() == Some(&message_id)
                })
        );
        drop(database);

        registration
            .record_event(
                workspace.path(),
                "chat-agent-tool-test",
                &ChatSseEvent::GuidanceApplied {
                    id: guidance.id,
                    content: guidance.content,
                    parts: Vec::new(),
                    interrupted_assistant_metrics: None,
                    source: guidance.source,
                    interrupted_assistant_id: None,
                },
            )
            .expect("persist applied Agent guidance");

        let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        assert!(
            database
                .agent_message(&message_id)
                .expect("message read")
                .expect("message")
                .consumed_at
                .is_some()
        );
        let run_events = database
            .run_events_for_run("run-agent-message-live")
            .expect("run events");
        assert_eq!(run_events.len(), 1);
        assert!(
            run_events[0]
                .payload_json
                .contains(AGENT_MESSAGE_GUIDANCE_SOURCE)
        );
        assert!(
            database
                .agent_events_after(&team_id, -1)
                .expect("Agent events")
                .iter()
                .any(|event| {
                    event.event_type == "message_consumed"
                        && event.message_id.as_ref() == Some(&message_id)
                })
        );
    }

    #[test]
    fn agent_send_message_queues_unread_message_when_receiver_is_idle() {
        let (workspace, context, team_id, instance_id, _task_id, _wake_rx) =
            create_agent_tool_fixture(AgentPermissions::default());

        let output = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_SEND_MESSAGE_TOOL,
            "call-agent-message-idle",
            json!({
                "receiverInstanceId": instance_id.to_string(),
                "kind": "notification",
                "content": "apply this on the next attempt",
                "replyToMessageId": null,
                "relatedTaskId": null,
                "timeoutMs": null,
            }),
        )
        .expect("persist queued Agent message");
        assert_eq!(output["delivery"], "queued");

        let message_id = foco_agent::AgentMessageId::new(
            output["messageId"].as_str().expect("message id string"),
        )
        .expect("Agent message id");
        let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        assert_eq!(
            database
                .agent_message(&message_id)
                .expect("message read")
                .expect("message")
                .consumed_at,
            None
        );
        assert!(
            database
                .agent_events_after(&team_id, -1)
                .expect("Agent events")
                .iter()
                .all(|event| {
                    event.message_id.as_ref() != Some(&message_id)
                        || event.event_type != "message_consumed"
                })
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
        let (workspace, mut context, team_id, _instance_id, _task_id, _wake_rx) =
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
    fn agent_create_instances_shared_worker_inherits_current_execution_root() {
        let mut worker_definition =
            test_agent_definition("tool-test-review", AgentPermissions::default());
        worker_definition.max_instances = 2;
        let permissions = AgentPermissions {
            can_create_instances: true,
            allowed_agent_definition_ids: vec![worker_definition.id.clone()],
            ..AgentPermissions::default()
        };
        let (workspace, mut context, team_id, _instance_id, _task_id, _wake_rx) =
            create_agent_tool_fixture(permissions);
        context.agent_definitions = vec![worker_definition.clone()];
        let phase_worktree_path = workspace
            .path()
            .join(".foco")
            .join("agent-worktrees")
            .join("phase-worktree");
        std::fs::create_dir_all(&phase_worktree_path).expect("phase worktree dir");

        let created = execute_agent_tool(
            &context,
            &phase_worktree_path,
            AGENT_CREATE_INSTANCES_TOOL,
            "call-create-review-worker",
            json!({
                "definitionId": worker_definition.id.to_string(),
                "count": 1,
                "executionWorkspaceMode": "shared",
                "timeoutMs": null,
            }),
        )
        .expect("shared worker creation should inherit current execution root");

        assert_eq!(created["count"], json!(1));
        let instances = WorkspaceDatabase::open_or_create(workspace.path())
            .expect("database")
            .agent_instances_for_team(&team_id)
            .expect("instances");
        let worker = instances
            .iter()
            .find(|instance| instance.definition_id == worker_definition.id)
            .expect("created worker");
        assert_eq!(
            worker.execution_workspace_mode,
            AgentExecutionWorkspaceMode::Shared
        );
        assert_eq!(
            worker.execution_root_path.as_deref(),
            Some(".foco/agent-worktrees/phase-worktree")
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
        let (workspace, mut context, _team_id, _instance_id, _task_id, _wake_rx) =
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
        let (workspace, context, team_id, instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(permissions);

        let no_instance_error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-no-instance",
            json!({
                "targetKind": "definition",
                "targetId": missing_definition_id.to_string(),
                "input": { "message": "child" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("definition without instance must fail");
        let no_instance_output = agent_tool_error_output(&no_instance_error);
        assert_eq!(no_instance_output["code"], "not_found");
        let no_instance_message = no_instance_output["error"].as_str().expect("error text");
        assert!(
            no_instance_message.contains("no existing runnable instance"),
            "expected missing-instance guidance, got {no_instance_message}"
        );
        assert!(
            no_instance_message.contains("agent_create_instances")
                && no_instance_message.contains("never auto-creates"),
            "expected create-instance recovery path, got {no_instance_message}"
        );

        let oversized_input_error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-oversized-input",
            json!({
                "targetKind": "instance",
                "targetId": instance_id.to_string(),
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
                "targetKind": "instance",
                "targetId": instance_id.to_string(),
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

    #[test]
    fn agent_collaboration_tools_recover_from_illegal_ids() {
        let permissions = AgentPermissions {
            can_create_instances: true,
            can_delegate: true,
            allowed_agent_definition_ids: Vec::new(),
        };
        let (workspace, context, team_id, _instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(permissions);

        let child_count = || {
            WorkspaceDatabase::open_or_create(workspace.path())
                .expect("database")
                .agent_tasks_for_parent(&team_id, &parent_task_id)
                .expect("child tasks")
                .len()
        };
        assert_eq!(child_count(), 0, "fixture starts with no child tasks");

        // Cover the same illegal shapes the public schema pattern rejects so runtime
        // recovery stays aligned when a provider bypasses tool schema constraints.
        for (call_id, target_kind, target_id) in [
            ("call-illegal-display-name", "definition", "Review"),
            ("call-illegal-missing-prefix", "definition", "definition-1"),
            (
                "call-illegal-empty-suffix",
                "definition",
                "agent-definition-",
            ),
            (
                "call-illegal-uppercase",
                "definition",
                "agent-definition-UPPER",
            ),
            (
                "call-illegal-underscore",
                "definition",
                "agent-definition-with_underscore",
            ),
            (
                "call-illegal-wrong-prefix",
                "definition",
                "agent-instance-1",
            ),
            ("call-illegal-instance-name", "instance", "worker-1"),
            ("call-illegal-instance-empty", "instance", "agent-instance-"),
            (
                "call-illegal-instance-upper",
                "instance",
                "agent-instance-UPPER",
            ),
        ] {
            let error = execute_agent_tool(
                &context,
                workspace.path(),
                AGENT_DELEGATE_TASK_TOOL,
                call_id,
                json!({
                    "targetKind": target_kind,
                    "targetId": target_id,
                    "input": { "message": "child" },
                    "correlationId": null,
                    "timeoutMs": null,
                }),
            )
            .expect_err("illegal agent id must fail");
            let output = agent_tool_error_output(&error);
            assert_eq!(
                output["code"], "invalid_arguments",
                "illegal id {call_id} must use invalid_arguments, got {output}"
            );
            let message = output["error"].as_str().expect("error text");
            assert!(
                message.contains("agent_list")
                    && message.contains("do not invent ids")
                    && message.contains("targetId"),
                "expected recoverable id guidance for {call_id}, got {message}"
            );
            assert_eq!(
                child_count(),
                0,
                "illegal id {call_id} must not enqueue child tasks"
            );
        }

        let too_long_definition = format!("agent-definition-{}", "a".repeat(128));
        assert!(too_long_definition.len() > 128);
        let oversized = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-illegal-oversized",
            json!({
                "targetKind": "definition",
                "targetId": too_long_definition,
                "input": { "message": "child" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("oversized definition id must fail");
        let oversized_output = agent_tool_error_output(&oversized);
        assert_eq!(oversized_output["code"], "invalid_arguments");
        assert!(
            oversized_output["error"]
                .as_str()
                .expect("error text")
                .contains("at most 128")
        );
        assert_eq!(child_count(), 0);

        let illegal_create = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_CREATE_INSTANCES_TOOL,
            "call-illegal-create",
            json!({
                "definitionId": "Review",
                "count": 1,
                "executionWorkspaceMode": "shared",
                "timeoutMs": null,
            }),
        )
        .expect_err("create with display-name definition id must fail");
        let illegal_create_output = agent_tool_error_output(&illegal_create);
        assert_eq!(illegal_create_output["code"], "invalid_arguments");
        let illegal_create_message = illegal_create_output["error"].as_str().expect("error text");
        assert!(
            illegal_create_message.contains("definitionId")
                && illegal_create_message.contains("agent-definition-")
                && illegal_create_message.contains("agent_list"),
            "expected recoverable create definition guidance, got {illegal_create_message}"
        );

        let illegal_transfer = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_TRANSFER_TASK_TOOL,
            "call-illegal-transfer",
            json!({
                "taskId": "agent-task-tool-test-parent",
                "targetInstanceId": "Review Worker",
                "timeoutMs": null,
            }),
        )
        .expect_err("transfer with illegal instance id must fail");
        let illegal_transfer_output = agent_tool_error_output(&illegal_transfer);
        assert_eq!(illegal_transfer_output["code"], "invalid_arguments");
        let illegal_transfer_message = illegal_transfer_output["error"]
            .as_str()
            .expect("error text");
        assert!(
            illegal_transfer_message.contains("targetInstanceId")
                && illegal_transfer_message.contains("agent-instance-")
                && illegal_transfer_message.contains("agent_list"),
            "expected recoverable transfer instance guidance, got {illegal_transfer_message}"
        );

        // Well-formed but missing id stays not_found, not invalid_arguments.
        let missing_well_formed = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-missing-well-formed",
            json!({
                "targetKind": "instance",
                "targetId": "agent-instance-does-not-exist",
                "input": { "message": "child" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("missing well-formed instance must fail");
        assert_eq!(
            agent_tool_error_output(&missing_well_formed)["code"],
            "not_found"
        );
        assert_eq!(child_count(), 0);
    }

    #[test]
    fn agent_delegate_task_rejects_unknown_target_kind_without_enqueuing() {
        let permissions = AgentPermissions {
            can_delegate: true,
            ..AgentPermissions::default()
        };
        let (workspace, context, team_id, instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(permissions);

        let error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-unknown-target-kind",
            json!({
                "targetKind": "queue",
                "targetId": instance_id.to_string(),
                "input": { "message": "unknown kind" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("unknown target kind must be rejected");
        let output = agent_tool_error_output(&error);
        assert_eq!(output["code"], "invalid_arguments");
        assert!(
            output["error"]
                .as_str()
                .expect("error text")
                .contains("unknown variant")
        );
        assert_eq!(
            delegated_child_task_count(workspace.path(), &team_id, &parent_task_id),
            0
        );
    }

    #[test]
    fn agent_delegate_task_rejects_target_kind_and_id_prefix_mismatch_without_enqueuing() {
        let permissions = AgentPermissions {
            can_delegate: true,
            ..AgentPermissions::default()
        };
        let (workspace, context, team_id, _instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(permissions);
        let definition_id = "agent-definition-tool-test-worker";

        let error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-mismatched-target-kind",
            json!({
                "targetKind": "instance",
                "targetId": definition_id,
                "input": { "message": "mismatched" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("instance target kind must reject a definition id");
        let output = agent_tool_error_output(&error);
        assert_eq!(output["code"], "invalid_arguments");
        let message = output["error"].as_str().expect("error text");
        assert!(
            message.contains("targetKind \"instance\"")
                && message.contains("agent-instance-")
                && message.contains("agent_list.instances[].id"),
            "expected target-kind-specific recovery guidance, got {message}"
        );
        assert_eq!(
            delegated_child_task_count(workspace.path(), &team_id, &parent_task_id),
            0
        );
    }

    #[test]
    fn agent_delegate_task_rejects_missing_instance_without_enqueuing() {
        let permissions = AgentPermissions {
            can_delegate: true,
            ..AgentPermissions::default()
        };
        let (workspace, context, team_id, _instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(permissions);

        let error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-missing-instance",
            json!({
                "targetKind": "instance",
                "targetId": "agent-instance-does-not-exist",
                "input": { "message": "missing" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("missing instance must be rejected");
        let output = agent_tool_error_output(&error);
        assert_eq!(output["code"], "not_found");
        assert!(
            output["error"]
                .as_str()
                .expect("error text")
                .contains("was not found")
        );
        assert_eq!(
            delegated_child_task_count(workspace.path(), &team_id, &parent_task_id),
            0
        );
    }

    #[test]
    fn agent_delegate_task_rejects_disallowed_definition_without_enqueuing() {
        let permissions = AgentPermissions {
            can_delegate: true,
            ..AgentPermissions::default()
        };
        let (workspace, context, team_id, _instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(permissions);
        let definition_id = "agent-definition-tool-test-disallowed";

        let error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-disallowed-definition",
            json!({
                "targetKind": "definition",
                "targetId": definition_id,
                "input": { "message": "denied" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("disallowed definition must be rejected");
        let output = agent_tool_error_output(&error);
        assert_eq!(output["code"], "permission_denied");
        assert!(
            output["error"]
                .as_str()
                .expect("error text")
                .contains("is not allowed for delegation")
        );
        assert_eq!(
            delegated_child_task_count(workspace.path(), &team_id, &parent_task_id),
            0
        );
    }

    #[test]
    fn agent_delegate_task_rejects_definition_without_runnable_instance_without_enqueuing() {
        let definition_id = AgentDefinitionId::new("agent-definition-tool-test-no-instance")
            .expect("definition id");
        let permissions = AgentPermissions {
            can_delegate: true,
            allowed_agent_definition_ids: vec![definition_id.clone()],
            ..AgentPermissions::default()
        };
        let (workspace, context, team_id, _instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(permissions);

        let error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-definition-no-instance",
            json!({
                "targetKind": "definition",
                "targetId": definition_id.to_string(),
                "input": { "message": "no instance" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("definition without a runnable instance must be rejected");
        let output = agent_tool_error_output(&error);
        assert_eq!(output["code"], "not_found");
        let message = output["error"].as_str().expect("error text");
        assert!(
            message.contains("no existing runnable instance")
                && message.contains("agent_create_instances")
                && message.contains("never auto-creates"),
            "expected creation recovery guidance, got {message}"
        );
        assert_eq!(
            delegated_child_task_count(workspace.path(), &team_id, &parent_task_id),
            0
        );
    }

    #[test]
    fn agent_delegate_task_rejects_legacy_target_fields_without_enqueuing() {
        let permissions = AgentPermissions {
            can_delegate: true,
            ..AgentPermissions::default()
        };
        let (workspace, context, team_id, instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(permissions);

        let error = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-legacy-target-fields",
            json!({
                "targetKind": "instance",
                "targetId": instance_id.to_string(),
                "targetInstanceId": instance_id.to_string(),
                "targetDefinitionId": null,
                "input": { "message": "legacy" },
                "correlationId": null,
                "timeoutMs": null,
            }),
        )
        .expect_err("legacy target fields must be rejected");
        let output = agent_tool_error_output(&error);
        assert_eq!(output["code"], "invalid_arguments");
        assert!(
            output["error"]
                .as_str()
                .expect("error text")
                .contains("unknown field")
        );

        assert_eq!(
            delegated_child_task_count(workspace.path(), &team_id, &parent_task_id),
            0
        );
    }

    #[test]
    fn agent_delegate_task_routes_instance_with_stable_output_and_event_payload() {
        let permissions = AgentPermissions {
            can_delegate: true,
            ..AgentPermissions::default()
        };
        let (workspace, context, team_id, instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(permissions);

        let output = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-instance-target",
            json!({
                "targetKind": "instance",
                "targetId": instance_id.to_string(),
                "input": { "message": "instance child" },
                "correlationId": "instance-correlation",
                "timeoutMs": null,
            }),
        )
        .expect("instance target must enqueue a child task");
        assert_eq!(output["targetInstanceId"], instance_id.to_string());
        assert_eq!(output["status"], "queued");
        assert_eq!(output["correlationId"], "instance-correlation");

        let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        let child_tasks = database
            .agent_tasks_for_parent(&team_id, &parent_task_id)
            .expect("child tasks");
        assert_eq!(child_tasks.len(), 1);
        assert_eq!(child_tasks[0].owner_instance_id, instance_id);
        let delegated_event = database
            .agent_events_after(&team_id, -1)
            .expect("Agent events")
            .into_iter()
            .find(|event| event.event_type == "task_delegated")
            .expect("task delegated event");
        let payload = serde_json::from_str::<Value>(&delegated_event.payload_json)
            .expect("task_delegated payload JSON");
        assert_eq!(payload["targetInstanceId"], output["targetInstanceId"]);
        assert!(payload["targetDefinitionId"].is_null());
    }

    #[test]
    fn agent_delegate_task_routes_definition_with_stable_event_payload() {
        let mut worker_definition =
            test_agent_definition("tool-test-definition-route", AgentPermissions::default());
        worker_definition.max_instances = 2;
        let permissions = AgentPermissions {
            can_create_instances: true,
            can_delegate: true,
            allowed_agent_definition_ids: vec![worker_definition.id.clone()],
        };
        let (workspace, mut context, team_id, _instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(permissions);
        context.agent_definitions = vec![worker_definition.clone()];

        let created = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_CREATE_INSTANCES_TOOL,
            "call-create-definition-target",
            json!({
                "definitionId": worker_definition.id.to_string(),
                "count": 1,
                "executionWorkspaceMode": "shared",
                "timeoutMs": null,
            }),
        )
        .expect("worker instance creation");
        let worker_instance_id = created["instances"][0]["id"]
            .as_str()
            .expect("created worker id")
            .to_owned();

        let output = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_DELEGATE_TASK_TOOL,
            "call-definition-target",
            json!({
                "targetKind": "definition",
                "targetId": worker_definition.id.to_string(),
                "input": { "message": "definition child" },
                "correlationId": "definition-correlation",
                "timeoutMs": null,
            }),
        )
        .expect("definition target must route to its worker");
        assert_eq!(output["targetInstanceId"], worker_instance_id);
        assert_eq!(output["status"], "queued");
        assert_eq!(output["correlationId"], "definition-correlation");

        let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        let child_tasks = database
            .agent_tasks_for_parent(&team_id, &parent_task_id)
            .expect("child tasks");
        assert_eq!(child_tasks.len(), 1);
        assert_eq!(
            child_tasks[0].owner_instance_id.to_string(),
            worker_instance_id
        );
        let delegated_event = database
            .agent_events_after(&team_id, -1)
            .expect("Agent events")
            .into_iter()
            .find(|event| event.event_type == "task_delegated")
            .expect("task delegated event");
        let payload = serde_json::from_str::<Value>(&delegated_event.payload_json)
            .expect("task_delegated payload JSON");
        assert_eq!(payload["targetInstanceId"], output["targetInstanceId"]);
        assert_eq!(
            payload["targetDefinitionId"],
            worker_definition.id.to_string()
        );
    }

    fn claim_parent_running(
        workspace_path: &std::path::Path,
        team_id: &AgentTeamId,
        task_id: &AgentTaskId,
        attempt_suffix: &str,
    ) {
        let mut database = WorkspaceDatabase::open_or_create(workspace_path).expect("database");
        let attempt_id =
            AgentAttemptId::new(format!("agent-attempt-{attempt_suffix}")).expect("attempt id");
        database
            .claim_runnable_agent_task(team_id, task_id, &attempt_id)
            .expect("claim parent")
            .expect("parent claimed");
    }

    fn create_worker_child(
        workspace_path: &std::path::Path,
        team_id: &AgentTeamId,
        parent_instance_id: &AgentInstanceId,
        parent_task_id: &AgentTaskId,
        child_suffix: &str,
    ) -> AgentTaskId {
        let mut database = WorkspaceDatabase::open_or_create(workspace_path).expect("database");
        let worker_id =
            AgentInstanceId::new(format!("agent-instance-{child_suffix}")).expect("worker id");
        let worker_definition = test_agent_definition(
            child_suffix,
            AgentPermissions {
                can_create_instances: false,
                can_delegate: false,
                allowed_agent_definition_ids: Vec::new(),
            },
        );
        database
            .create_agent_instances_with_limits(
                &[NewAgentInstance {
                    id: &worker_id,
                    team_id,
                    definition: &worker_definition,
                    role: foco_agent::AgentRole::Worker,
                    execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
                    execution_root_path: None,
                    worktree_base_revision: None,
                    worktree_branch: None,
                    worktree_status: None,
                }],
                4,
                2,
            )
            .expect("create worker");
        let child_task_id =
            AgentTaskId::new(format!("agent-task-{child_suffix}")).expect("child task id");
        database
            .enqueue_agent_task(NewAgentTask {
                id: &child_task_id,
                team_id,
                owner_instance_id: &worker_id,
                origin_instance_id: Some(parent_instance_id),
                parent_task_id: Some(parent_task_id),
                input_json: r#"{"message":"child"}"#,
            })
            .expect("enqueue child");
        child_task_id
    }

    #[test]
    fn agent_wait_tasks_registers_round_idempotently_and_maps_active_conflict() {
        let (workspace, context, team_id, instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(AgentPermissions {
                can_create_instances: true,
                can_delegate: true,
                allowed_agent_definition_ids: Vec::new(),
            });
        claim_parent_running(
            workspace.path(),
            &team_id,
            &parent_task_id,
            "wait-idempotent-parent",
        );
        let child_a = create_worker_child(
            workspace.path(),
            &team_id,
            &instance_id,
            &parent_task_id,
            "wait-idempotent-a",
        );
        let child_b = create_worker_child(
            workspace.path(),
            &team_id,
            &instance_id,
            &parent_task_id,
            "wait-idempotent-b",
        );

        let wait_args = json!({
            "taskIds": [child_a.to_string(), child_b.to_string()],
            "mode": "all",
            "deadlineMs": null,
        });
        let first = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_WAIT_TASKS_TOOL,
            "call-wait-round-1",
            wait_args.clone(),
        )
        .expect("first wait must register");
        assert_eq!(first["waiting"], true);
        assert_eq!(first["suspend"]["kind"], "agent_wait_tasks");
        assert_eq!(first["suspend"]["pendingToolCallId"], "call-wait-round-1");
        assert!(
            is_agent_wait_suspend_output(&first),
            "outstanding children must produce a non-terminal suspend control"
        );
        assert_eq!(
            first["taskIds"].as_array().map(|items| items.len()),
            Some(2)
        );

        let replay = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_WAIT_TASKS_TOOL,
            "call-wait-round-1",
            wait_args,
        )
        .expect("identical wait round must replay");
        assert_eq!(replay["waiting"], true);
        assert_eq!(replay["suspend"]["pendingToolCallId"], "call-wait-round-1");
        assert!(is_agent_wait_suspend_output(&replay));

        let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        let deps = database
            .agent_task_dependencies(&parent_task_id)
            .expect("dependencies");
        assert_eq!(deps.len(), 2);
        assert!(
            deps.iter()
                .all(|dep| dep.pending_tool_call_id.as_deref() == Some("call-wait-round-1"))
        );
        let waiting_events = database
            .agent_events_after(&team_id, -1)
            .expect("events")
            .into_iter()
            .filter(|event| event.event_type == "task_waiting_requested")
            .filter(|event| event.task_id.as_ref() == Some(&parent_task_id))
            .count();
        assert_eq!(
            waiting_events, 1,
            "replay must not emit a second task_waiting_requested event"
        );

        let conflict = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_WAIT_TASKS_TOOL,
            "call-wait-round-2",
            json!({
                "taskIds": [child_a.to_string()],
                "mode": "all",
                "deadlineMs": null,
            }),
        )
        .expect_err("active wait round must reject a different tool call");
        let conflict_output = agent_tool_error_output(&conflict);
        assert_eq!(conflict_output["code"], "wait_round_active");
        let message = conflict_output["error"].as_str().unwrap_or_default();
        assert!(
            message.contains("active wait round"),
            "expected active wait message, got {message}"
        );
        assert!(
            !message.contains("foco.sqlite"),
            "must not leak database path: {message}"
        );
        assert!(
            !message.contains("1555"),
            "must not leak SQLite constraint code: {message}"
        );

        // Prior wait set and pending tool call stay unchanged on conflict.
        let deps_after = database
            .agent_task_dependencies(&parent_task_id)
            .expect("dependencies after conflict");
        assert_eq!(deps_after.len(), 2);
        assert!(
            deps_after
                .iter()
                .all(|dep| dep.pending_tool_call_id.as_deref() == Some("call-wait-round-1"))
        );
        drop(database);

        // Two-phase lifecycle: durable registration can still reach Waiting and resume
        // with the same pending tool call id after a replayed registration.
        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        database
            .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &parent_task_id,
                expected_status: AgentTaskStatus::Running,
                transition: foco_agent::AgentTaskTransition::Wait,
                result_json: Some(
                    r#"{"control":{"kind":"agent_wait_tasks","pendingToolCallId":"call-wait-round-1"}}"#,
                ),
                error_json: None,
                interruption_reason: None,
            })
            .expect("suspend parent after durable wait registration");
        for (child_id, suffix) in [
            (&child_a, "wait-idempotent-a"),
            (&child_b, "wait-idempotent-b"),
        ] {
            let child_attempt =
                AgentAttemptId::new(format!("agent-attempt-{suffix}")).expect("child attempt");
            database
                .claim_runnable_agent_task(&team_id, child_id, &child_attempt)
                .expect("claim child")
                .expect("child claimed");
            database
                .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                    team_id: &team_id,
                    task_id: child_id,
                    expected_status: AgentTaskStatus::Running,
                    transition: foco_agent::AgentTaskTransition::Complete,
                    result_json: Some(r#"{"text":"done"}"#),
                    error_json: None,
                    interruption_reason: None,
                })
                .expect("complete child");
        }
        let resumed = database
            .resume_satisfied_agent_tasks(10)
            .expect("resume satisfied wait round");
        assert!(
            resumed
                .iter()
                .any(|task| task.id == parent_task_id && task.status == AgentTaskStatus::Queued),
            "satisfied wait round must re-queue the parent for claim"
        );
        let parent_attempt =
            AgentAttemptId::new("agent-attempt-wait-idempotent-resume").expect("parent attempt");
        database
            .claim_runnable_agent_task(&team_id, &parent_task_id, &parent_attempt)
            .expect("claim resumed parent")
            .expect("parent claimed after resume");
        let deps = database
            .agent_task_dependencies(&parent_task_id)
            .expect("dependencies for resume messages");
        let dependency_tasks = deps
            .iter()
            .map(|dep| {
                database
                    .agent_task(&dep.dependency_task_id)
                    .expect("dependency task lookup")
                    .expect("dependency task")
            })
            .collect::<Vec<_>>();
        drop(database);

        let resume_messages = crate::agent_wait_resume_messages(&deps, &dependency_tasks)
            .expect("wait resume messages after replayed registration");
        assert_eq!(resume_messages.len(), 3);
        assert_eq!(
            resume_messages[1].tool_calls[0].call_id,
            "call-wait-round-1"
        );
        assert_eq!(
            resume_messages[2].tool_call_id.as_deref(),
            Some("call-wait-round-1")
        );
        let _ = context;
    }

    #[test]
    fn agent_wait_tasks_replaces_terminal_round_for_sequential_wait() {
        let (workspace, context, team_id, instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(AgentPermissions {
                can_create_instances: true,
                can_delegate: true,
                allowed_agent_definition_ids: Vec::new(),
            });
        claim_parent_running(
            workspace.path(),
            &team_id,
            &parent_task_id,
            "wait-replace-parent",
        );
        let child_a = create_worker_child(
            workspace.path(),
            &team_id,
            &instance_id,
            &parent_task_id,
            "wait-replace-a",
        );
        let child_b = create_worker_child(
            workspace.path(),
            &team_id,
            &instance_id,
            &parent_task_id,
            "wait-replace-b",
        );

        execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_WAIT_TASKS_TOOL,
            "call-wait-first",
            json!({
                "taskIds": [child_a.to_string()],
                "mode": "all",
                "deadlineMs": null,
            }),
        )
        .expect("first wait");

        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        // Parent is still Running (two-phase wait). Free team concurrency so the child
        // can be claimed, then complete it to make the first wait round terminal.
        database
            .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &parent_task_id,
                expected_status: AgentTaskStatus::Running,
                transition: foco_agent::AgentTaskTransition::Wait,
                result_json: Some(
                    r#"{"control":{"kind":"agent_wait_tasks","pendingToolCallId":"call-wait-first"}}"#,
                ),
                error_json: None,
                interruption_reason: None,
            })
            .expect("suspend parent after durable wait registration");
        let child_attempt =
            AgentAttemptId::new("agent-attempt-wait-replace-a").expect("child attempt");
        database
            .claim_runnable_agent_task(&team_id, &child_a, &child_attempt)
            .expect("claim child")
            .expect("child claimed");
        database
            .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &child_a,
                expected_status: AgentTaskStatus::Running,
                transition: foco_agent::AgentTaskTransition::Complete,
                result_json: Some(r#"{"text":"first child done"}"#),
                error_json: None,
                interruption_reason: None,
            })
            .expect("complete first child");
        let resumed = database
            .resume_satisfied_agent_tasks(10)
            .expect("resume first wait round through scheduler store path");
        assert!(
            resumed
                .iter()
                .any(|task| task.id == parent_task_id && task.status == AgentTaskStatus::Queued)
        );
        let first_resume_attempt =
            AgentAttemptId::new("agent-attempt-wait-replace-resume-1").expect("parent attempt");
        database
            .claim_runnable_agent_task(&team_id, &parent_task_id, &first_resume_attempt)
            .expect("claim parent after first wait")
            .expect("parent claimed after first wait");
        let first_round_deps = database
            .agent_task_dependencies(&parent_task_id)
            .expect("first-round dependencies");
        let first_round_tasks = first_round_deps
            .iter()
            .map(|dep| {
                database
                    .agent_task(&dep.dependency_task_id)
                    .expect("dependency task lookup")
                    .expect("dependency task")
            })
            .collect::<Vec<_>>();
        let first_resume_messages =
            crate::agent_wait_resume_messages(&first_round_deps, &first_round_tasks)
                .expect("first-round wait resume messages");
        assert_eq!(
            first_resume_messages[1].tool_calls[0].call_id,
            "call-wait-first"
        );
        assert_eq!(
            first_resume_messages[2].tool_call_id.as_deref(),
            Some("call-wait-first")
        );
        assert!(
            first_resume_messages[2]
                .content
                .contains("first child done")
        );
        drop(database);

        // Parent is Running again; prior dependency is terminal so a new tool call may replace.
        let second = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_WAIT_TASKS_TOOL,
            "call-wait-second",
            json!({
                "taskIds": [child_b.to_string()],
                "mode": "all",
                "deadlineMs": null,
            }),
        )
        .expect("second wait after terminal first round");
        assert_eq!(second["suspend"]["pendingToolCallId"], "call-wait-second");
        assert_eq!(second["taskIds"], json!([child_b.to_string()]));

        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        let deps = database
            .agent_task_dependencies(&parent_task_id)
            .expect("dependencies");
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].dependency_task_id, child_b);
        assert_eq!(
            deps[0].pending_tool_call_id.as_deref(),
            Some("call-wait-second")
        );

        database
            .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &parent_task_id,
                expected_status: AgentTaskStatus::Running,
                transition: foco_agent::AgentTaskTransition::Wait,
                result_json: Some(
                    r#"{"control":{"kind":"agent_wait_tasks","pendingToolCallId":"call-wait-second"}}"#,
                ),
                error_json: None,
                interruption_reason: None,
            })
            .expect("suspend parent for second wait round");
        let child_b_attempt =
            AgentAttemptId::new("agent-attempt-wait-replace-b").expect("child b attempt");
        database
            .claim_runnable_agent_task(&team_id, &child_b, &child_b_attempt)
            .expect("claim second child")
            .expect("second child claimed");
        database
            .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &child_b,
                expected_status: AgentTaskStatus::Running,
                transition: foco_agent::AgentTaskTransition::Complete,
                result_json: Some(r#"{"text":"second child done"}"#),
                error_json: None,
                interruption_reason: None,
            })
            .expect("complete second child");
        let resumed_second = database
            .resume_satisfied_agent_tasks(10)
            .expect("resume second wait round");
        assert!(
            resumed_second
                .iter()
                .any(|task| task.id == parent_task_id && task.status == AgentTaskStatus::Queued)
        );
        let second_resume_attempt =
            AgentAttemptId::new("agent-attempt-wait-replace-resume-2").expect("parent attempt");
        database
            .claim_runnable_agent_task(&team_id, &parent_task_id, &second_resume_attempt)
            .expect("claim parent after second wait")
            .expect("parent claimed after second wait");
        let second_round_deps = database
            .agent_task_dependencies(&parent_task_id)
            .expect("second-round dependencies");
        let second_round_tasks = second_round_deps
            .iter()
            .map(|dep| {
                database
                    .agent_task(&dep.dependency_task_id)
                    .expect("dependency task lookup")
                    .expect("dependency task")
            })
            .collect::<Vec<_>>();
        drop(database);

        let second_resume_messages =
            crate::agent_wait_resume_messages(&second_round_deps, &second_round_tasks)
                .expect("second-round wait resume messages");
        assert_eq!(second_resume_messages.len(), 3);
        assert_eq!(
            second_resume_messages[1].tool_calls[0].call_id, "call-wait-second",
            "second resume assistant tool-call must use the new pending tool call id"
        );
        assert_eq!(
            second_resume_messages[2].tool_call_id.as_deref(),
            Some("call-wait-second"),
            "second resume tool message must pair with the new pending tool call id"
        );
        assert!(
            second_resume_messages[2]
                .content
                .contains("second child done")
        );
        assert!(
            !second_resume_messages[2]
                .content
                .contains("first child done"),
            "second resume must not include the prior wait-round child payload"
        );
        let _ = context;
    }

    #[test]
    fn implicit_wait_registers_undelivered_children_and_skips_delivered() {
        let (workspace, context, team_id, instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(AgentPermissions {
                can_create_instances: true,
                can_delegate: true,
                allowed_agent_definition_ids: Vec::new(),
            });
        claim_parent_running(
            workspace.path(),
            &team_id,
            &parent_task_id,
            "implicit-wait-parent",
        );
        assert!(
            try_register_implicit_wait_for_undelivered_children(&context)
                .expect("implicit wait with no children")
                .is_none()
        );

        let child_a = create_worker_child(
            workspace.path(),
            &team_id,
            &instance_id,
            &parent_task_id,
            "implicit-wait-a",
        );
        let child_b = create_worker_child(
            workspace.path(),
            &team_id,
            &instance_id,
            &parent_task_id,
            "implicit-wait-b",
        );

        let first = try_register_implicit_wait_for_undelivered_children(&context)
            .expect("implicit wait with undelivered children")
            .expect("must register wait");
        assert!(!first.immediate, "queued children require suspend");
        assert_eq!(first.output["implicit"], true);
        assert_eq!(first.output["suspend"]["kind"], "agent_wait_tasks");
        assert_eq!(
            first.output["suspend"]["pendingToolCallId"],
            first.tool_call_id
        );
        assert!(is_agent_wait_suspend_output(&first.output));
        let mut registered = first.task_ids.clone();
        registered.sort();
        let mut expected = vec![child_a.to_string(), child_b.to_string()];
        expected.sort();
        assert_eq!(registered, expected);

        let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        let deps = database
            .agent_task_dependencies(&parent_task_id)
            .expect("dependencies");
        assert_eq!(deps.len(), 2);
        drop(database);

        assert!(
            try_register_implicit_wait_for_undelivered_children(&context)
                .expect("second implicit wait while deps cover children")
                .is_none(),
            "children already in the wait round must not re-register"
        );

        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        let parent_wait_result = format!(
            r#"{{"control":{{"kind":"agent_wait_tasks","pendingToolCallId":"{}"}}}}"#,
            first.tool_call_id
        );
        database
            .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &parent_task_id,
                expected_status: AgentTaskStatus::Running,
                transition: foco_agent::AgentTaskTransition::Wait,
                result_json: Some(&parent_wait_result),
                error_json: None,
                interruption_reason: None,
            })
            .expect("suspend parent after implicit wait");
        for (child_id, suffix) in [(&child_a, "implicit-wait-a"), (&child_b, "implicit-wait-b")] {
            let child_attempt =
                AgentAttemptId::new(format!("agent-attempt-{suffix}")).expect("child attempt");
            database
                .claim_runnable_agent_task(&team_id, child_id, &child_attempt)
                .expect("claim child")
                .expect("child claimed");
            database
                .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                    team_id: &team_id,
                    task_id: child_id,
                    expected_status: AgentTaskStatus::Running,
                    transition: foco_agent::AgentTaskTransition::Complete,
                    result_json: Some(r#"{"text":"implicit child done"}"#),
                    error_json: None,
                    interruption_reason: None,
                })
                .expect("complete child");
        }
        let resumed = database
            .resume_satisfied_agent_tasks(10)
            .expect("resume after implicit wait");
        assert!(
            resumed
                .iter()
                .any(|task| task.id == parent_task_id && task.status == AgentTaskStatus::Queued)
        );
        drop(database);

        assert!(
            try_register_implicit_wait_for_undelivered_children(&context)
                .expect("implicit wait after delivered children")
                .is_none(),
            "delivered children must not block finalize again"
        );

        // Create and finish a new child while the parent is still queued so the team
        // concurrent-run slot is free, then claim the parent and confirm implicit wait
        // picks up this undelivered (already terminal) child with an immediate final result.
        let child_c = create_worker_child(
            workspace.path(),
            &team_id,
            &instance_id,
            &parent_task_id,
            "implicit-wait-c",
        );
        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        let child_c_attempt =
            AgentAttemptId::new("agent-attempt-implicit-wait-c").expect("child c attempt");
        database
            .claim_runnable_agent_task(&team_id, &child_c, &child_c_attempt)
            .expect("claim child c")
            .expect("child c claimed");
        database
            .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &child_c,
                expected_status: AgentTaskStatus::Running,
                transition: foco_agent::AgentTaskTransition::Complete,
                result_json: Some(r#"{"text":"late child done"}"#),
                error_json: None,
                interruption_reason: None,
            })
            .expect("complete already-finished undelivered child");
        let resume_attempt =
            AgentAttemptId::new("agent-attempt-implicit-wait-resume").expect("parent attempt");
        database
            .claim_runnable_agent_task(&team_id, &parent_task_id, &resume_attempt)
            .expect("claim resumed parent")
            .expect("parent claimed");
        drop(database);

        let late = try_register_implicit_wait_for_undelivered_children(&context)
            .expect("implicit wait for terminal undelivered child")
            .expect("must register wait for undelivered terminal child");
        assert_eq!(late.task_ids, vec![child_c.to_string()]);
        assert!(
            late.immediate,
            "already-terminal undelivered children land immediately without suspend"
        );
        assert_eq!(late.output["waiting"], false);
        assert!(!is_agent_wait_suspend_output(&late.output));
        assert!(
            late.output["dependencies"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| {
                    item.get("taskId").and_then(Value::as_str) == Some(child_c.as_str())
                        && item.get("result").and_then(|result| result.get("text"))
                            == Some(&json!("late child done"))
                })),
            "immediate implicit wait must include the terminal child result once"
        );

        // After a sequential wait-round replacement, previously covered children must not
        // re-enter implicit finalize waits even though current dependency rows changed.
        execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_WAIT_TASKS_TOOL,
            "call-wait-replace-covered",
            json!({
                "taskIds": [child_a.to_string()],
                "mode": "all",
                "deadlineMs": null,
            }),
        )
        .expect("explicit wait replaces prior terminal round with historically covered child");
        assert!(
            try_register_implicit_wait_for_undelivered_children(&context)
                .expect("implicit wait after sequential replacement")
                .is_none(),
            "historically covered children must stay delivered across wait-round replacement"
        );
    }

    #[test]
    fn agent_wait_tasks_returns_terminal_result_when_all_dependencies_finished() {
        let (workspace, context, team_id, instance_id, parent_task_id, _wake_rx) =
            create_agent_tool_fixture(AgentPermissions {
                can_create_instances: true,
                can_delegate: true,
                allowed_agent_definition_ids: Vec::new(),
            });
        claim_parent_running(
            workspace.path(),
            &team_id,
            &parent_task_id,
            "wait-terminal-parent",
        );
        let child = create_worker_child(
            workspace.path(),
            &team_id,
            &instance_id,
            &parent_task_id,
            "wait-terminal-child",
        );

        // Register a first wait round, suspend, complete the child, resume, then claim the
        // parent again so an explicit wait can re-read the already-terminal child.
        execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_WAIT_TASKS_TOOL,
            "call-wait-first-round",
            json!({
                "taskIds": [child.to_string()],
                "mode": "all",
                "deadlineMs": null,
            }),
        )
        .expect("first wait registers outstanding child");
        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        database
            .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &parent_task_id,
                expected_status: AgentTaskStatus::Running,
                transition: foco_agent::AgentTaskTransition::Wait,
                result_json: Some(
                    r#"{"control":{"kind":"agent_wait_tasks","pendingToolCallId":"call-wait-first-round"}}"#,
                ),
                error_json: None,
                interruption_reason: None,
            })
            .expect("suspend parent");
        let child_attempt =
            AgentAttemptId::new("agent-attempt-wait-terminal-child").expect("child attempt");
        database
            .claim_runnable_agent_task(&team_id, &child, &child_attempt)
            .expect("claim child")
            .expect("child claimed");
        database
            .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &child,
                expected_status: AgentTaskStatus::Running,
                transition: foco_agent::AgentTaskTransition::Complete,
                result_json: Some(r#"{"text":"already done"}"#),
                error_json: None,
                interruption_reason: None,
            })
            .expect("complete child");
        database
            .resume_satisfied_agent_tasks(10)
            .expect("resume parent after child completion");
        let parent_attempt =
            AgentAttemptId::new("agent-attempt-wait-terminal-parent-resume").expect("parent attempt");
        database
            .claim_runnable_agent_task(&team_id, &parent_task_id, &parent_attempt)
            .expect("claim parent")
            .expect("parent claimed");
        drop(database);

        // Explicit wait may re-read the terminal child and must complete in one terminal result.
        let output = execute_agent_tool(
            &context,
            workspace.path(),
            AGENT_WAIT_TASKS_TOOL,
            "call-wait-already-terminal",
            json!({
                "taskIds": [child.to_string()],
                "mode": "all",
                "deadlineMs": null,
            }),
        )
        .expect("explicit wait on terminal child");
        assert_eq!(output["waiting"], false);
        assert!(output.get("suspend").is_none());
        assert!(!is_agent_wait_suspend_output(&output));
        assert_eq!(
            output["dependencies"][0]["result"]["text"],
            json!("already done")
        );
    }

    #[test]
    fn agent_store_error_hides_sqlite_path_and_maps_wait_conflicts() {
        let path = std::path::PathBuf::from("/tmp/example/.foco/foco.sqlite");
        let sqlite = agent_store_error(foco_store::workspace::WorkspaceDatabaseError::Sqlite {
            path: path.clone(),
            source: rusqlite::Error::InvalidQuery,
        });
        let sqlite_output = agent_tool_error_output(&sqlite);
        assert_eq!(sqlite_output["code"], "store_error");
        let message = sqlite_output["error"].as_str().unwrap_or_default();
        assert!(!message.contains("foco.sqlite"));
        assert!(!message.contains(path.to_string_lossy().as_ref()));
        assert!(message.contains("workspace database operation failed"));

        let active = agent_store_error(
            foco_store::workspace::WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message:
                    "agent task 'agent-task-x' already has an active wait round that has not finished; cannot register a different wait round"
                        .to_string(),
            },
        );
        assert_eq!(
            agent_tool_error_output(&active)["code"],
            "wait_round_active"
        );
    }
}
