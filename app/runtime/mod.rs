mod agent_scheduler;
mod api_audit;
mod broker_artifacts;
mod chat_run;
mod code_graph;
mod image_tools;
mod native_tools;
mod preview;
mod provider_audit;
mod questions;
pub(crate) mod reasoning_loop_detector;
mod sidecar_config;
mod subscriptions;
mod tool_events;
mod tool_execution;
mod tool_locks;
mod tool_loop;
mod web_tools;
pub(crate) use agent_scheduler::{
    AGENT_MAX_CREATE_INSTANCES_PER_REQUEST, AGENT_MAX_INSTANCES_PER_TEAM,
    AGENT_MAX_QUEUED_TASKS_PER_CHAT, AGENT_MAX_QUEUED_TASKS_PER_INSTANCE,
    AGENT_MAX_QUEUED_TASKS_PER_TEAM, AgentAttemptInterruption, AgentAttemptRecoveryAction,
    AgentScheduler, CoordinatorTaskInput, agent_attempt_recovery_action_for_evidence,
    agent_attempt_recovery_diagnostics, agent_wait_resume_messages, agent_wait_resume_tool_result,
    cancel_agent_task_subtree_runtime, insert_agent_event,
    open_workspace_database_ordinary_with_pre_stream_retry, pre_stream_failure_user_message,
    reconcile_agent_runtime, validate_agent_snapshot_for_workspace,
};
#[cfg(test)]
pub(crate) use agent_scheduler::{
    ActiveAgentAttemptIdentity, AgentAttemptRecoveryContext,
    agent_lifecycle_retry_until_shutdown_for_test, fail_claimed_task_with_retry,
    project_terminal_agent_task_lifecycles, reconcile_agent_attempt_leases,
    recover_panicked_coordinator_for_test, retain_agent_snapshot_tools,
};
#[cfg(test)]
pub(crate) use api_audit::should_vacuum_workspace_database;
pub(crate) use api_audit::{spawn_api_audit_cleanup_once, spawn_api_audit_cleanup_scheduler};
pub(crate) use broker_artifacts::BrokeredTransferFile;
#[cfg(test)]
pub(crate) use chat_run::agent_run_event_kind;
pub(crate) use chat_run::run_chat_context_in_background;
#[cfg(test)]
pub(crate) use code_graph::release_code_graph_execution_root;
pub(crate) use code_graph::{
    CodeGraphIndexState, CodeGraphReadinessError, release_code_graph_then_delete_worktree,
    spawn_code_graph_execution_root_initialization_if_needed, wait_for_code_graph_ready,
};
pub(crate) use foco_store::workspace::AGENT_MESSAGE_GUIDANCE_SOURCE;
pub(crate) use image_tools::{
    BrokeredImageFile, execute_image_tool, image_model_available, image_tool_timeout_ms,
    is_image_tool_name, materialize_brokered_image_result,
};
#[cfg(all(test, windows))]
pub(crate) use native_tools::find_system_ripgrep;
#[cfg(test)]
pub(crate) use native_tools::{
    GithubReleaseAsset, ripgrep_asset_target, ripgrep_executable_name, ripgrep_install_dir,
    select_ripgrep_asset,
};
pub(crate) use native_tools::{
    RipgrepStatus, RipgrepToolSummary, detect_ripgrep, download_and_install_ripgrep,
    ripgrep_tool_summary,
};
pub(crate) use preview::{
    PreviewSessionRegistry, create_preview_session, preview_host_middleware,
    redact_preview_host_for_log, release_preview_session, request_is_preview_capability,
    request_is_preview_host, serve_local_preview_file,
};
pub(crate) use provider_audit::ProviderAuditCapture;
pub(crate) use questions::{
    AskQuestionInput, QuestionAnswer, QuestionAnswerResponse, QuestionItem, QuestionItemAnswer,
    QuestionOption, QuestionRegistry, QuestionRequest,
};
pub(crate) use reasoning_loop_detector::{
    MANUAL_GUIDANCE_SOURCE, MAX_REASONING_LOOP_RECOVERIES_PER_RUN, REASONING_LOOP_GUARD_SOURCE,
    REASONING_LOOP_RECOVERY_USER_TEXT, ReasoningLoopDetector, default_guidance_source,
    is_automatic_guard_source, reasoning_loop_guard_message,
};
pub(crate) use sidecar_config::{SidecarRuntimeConfigBundle, build_sidecar_runtime_config_bundle};
pub(crate) use subscriptions::{
    ActiveAgentRunIdentity, ActiveChatRunRegistration, ActiveChatRunRegistrationResult,
    ActiveChatRunRegistry, ActiveChatRunSubscription, ActiveChatRunSummary,
    AgentMessageGuidanceDelivery, ChatRunCancellation, GuidanceMessage,
    chat_run_subscription_stream,
};
pub(crate) use tool_events::{ToolOutputDeltaEvent, ToolOutputDeltaSink};
#[cfg(test)]
pub(crate) use tool_execution::wait_for_tool_resource_lock;
pub(crate) use tool_execution::{
    AgentToolContext, ReadOnlyToolProgressAction, ReadOnlyToolProgressDetector,
    RepeatedToolCallDetector, ToolLoopBeforeExecutionAction, execute_tool_calls_parallel,
    is_agent_tool_name, is_agent_wait_suspend_output, pending_tool_calls,
    try_register_implicit_wait_for_undelivered_children,
};
pub(crate) use tool_execution::{
    budget_tool_execution, execute_tool, execute_tool_with_runtime, run_post_tool_hooks,
    tool_output_semantics,
};
pub(crate) use tool_locks::{
    ToolResourceLease, ToolResourceLockOwner, ToolResourceLockOwnerSnapshot,
    ToolResourceLockRegistry,
};
pub(crate) use tool_loop::{
    BlockedToolCall, MAX_TOOL_CALL_LOOP_RECOVERIES_PER_RUN, TOOL_CALL_LOOP_GUARD_SOURCE,
    ToolLoopGuard, blocked_tool_calls,
};
pub(crate) use web_tools::{
    execute_web_tool, is_web_tool_name, materialize_brokered_web_result,
    package_brokered_web_result_files, web_search_enabled, web_search_function_execution_allowed,
    web_tool_timeout_ms,
};
