mod agent_scheduler;
mod api_audit;
mod chat_run;
mod code_graph;
mod image_tools;
mod native_tools;
mod provider_audit;
mod questions;
pub(crate) mod reasoning_loop_detector;
mod sidecar_config;
mod subscriptions;
mod tool_events;
mod tool_execution;
mod tool_locks;
mod web_tools;
pub(crate) use agent_scheduler::{
    AGENT_MAX_CREATE_INSTANCES_PER_REQUEST, AGENT_MAX_INSTANCES_PER_TEAM,
    AGENT_MAX_QUEUED_TASKS_PER_CHAT, AGENT_MAX_QUEUED_TASKS_PER_INSTANCE,
    AGENT_MAX_QUEUED_TASKS_PER_TEAM, AgentScheduler, CoordinatorTaskInput, insert_agent_event,
    validate_agent_snapshot_for_workspace,
};
#[cfg(test)]
pub(crate) use agent_scheduler::{
    agent_lifecycle_retry_until_shutdown_for_test, fail_claimed_task_with_retry,
    reconcile_agent_runtime, recover_panicked_coordinator_for_test, retain_agent_snapshot_tools,
};
#[cfg(test)]
pub(crate) use api_audit::should_vacuum_workspace_database;
pub(crate) use api_audit::{spawn_api_audit_cleanup_once, spawn_api_audit_cleanup_scheduler};
#[cfg(test)]
pub(crate) use chat_run::agent_run_event_kind;
pub(crate) use chat_run::run_chat_context_in_background;
pub(crate) use code_graph::{
    CodeGraphIndexState, recently_active_code_graph_workspaces,
    spawn_code_graph_index_initialization, spawn_code_graph_workspace_initialization_if_needed,
};
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
pub(crate) use provider_audit::ProviderAuditCapture;
pub(crate) use questions::{
    AskQuestionInput, QuestionAnswer, QuestionAnswerResponse, QuestionItem, QuestionItemAnswer,
    QuestionOption, QuestionRegistry, QuestionRequest,
};
pub(crate) use reasoning_loop_detector::{
    MANUAL_GUIDANCE_SOURCE, MAX_REASONING_LOOP_RECOVERIES_PER_RUN, REASONING_LOOP_GUARD_SOURCE,
    REASONING_LOOP_RECOVERY_USER_TEXT, ReasoningLoopDetector, default_guidance_source,
    reasoning_loop_guard_message,
};
pub(crate) use sidecar_config::{SidecarRuntimeConfigBundle, build_sidecar_runtime_config_bundle};
pub(crate) use subscriptions::{
    ActiveChatRunRegistration, ActiveChatRunRegistry, ActiveChatRunSubscription,
    ActiveChatRunSummary, ChatRunCancellation, GuidanceMessage, chat_run_subscription_stream,
};
pub(crate) use tool_events::{ToolOutputDeltaEvent, ToolOutputDeltaSink};
#[cfg(test)]
pub(crate) use tool_execution::wait_for_tool_resource_lock;
pub(crate) use tool_execution::{
    AgentToolContext, ReadOnlyToolProgressAction, ReadOnlyToolProgressDetector,
    RepeatedToolCallDetector, execute_tool_calls_parallel, is_agent_tool_name, pending_tool_calls,
};
pub(crate) use tool_execution::{execute_tool, run_post_tool_hooks};
pub(crate) use tool_locks::{
    ToolResourceLease, ToolResourceLockOwner, ToolResourceLockOwnerSnapshot,
    ToolResourceLockRegistry,
};
pub(crate) use web_tools::{
    execute_web_tool, is_web_tool_name, web_search_enabled, web_tool_timeout_ms,
};
