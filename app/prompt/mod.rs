mod assembly;
mod compression;
mod environment;
mod prompt_files;

pub(crate) use assembly::prepare_prompt_context;
#[cfg(test)]
pub(crate) use compression::{
    LlmContextCompressionMode, compress_all_runtime_tool_state,
    compress_runtime_tool_state_if_needed, context_message_groups, context_token_breakdown,
    context_usage_segments, llm_context_compression_group_indices,
    llm_context_compression_trigger_tokens,
};
pub(crate) use compression::{
    active_compression_snapshots, compression_snapshot_message, context_usage_response,
    context_usage_segments_total, context_window_compression_trigger_tokens,
    ensure_context_compression, interleaved_tool_state_messages,
    neutral_assistant_tool_call_message, neutral_message_estimated_tokens,
    neutral_tool_call_from_record, pack_neutral_messages, persist_chat_result,
    persist_running_llm_request, recover_after_tool_round_cap, serialize_provider_request,
    snapshot_covered_sequences,
};
pub(crate) use environment::environment_context_message;
#[cfg(all(not(windows), not(target_os = "macos")))]
pub(crate) use environment::is_wsl_environment;
pub(crate) use prompt_files::{
    active_system_prompt, agents_prompt_messages, builtin_tool_definitions_for_runtime,
    configured_extra_prompt_message, configured_prompt_messages, system_prompt_summaries,
    tool_prompt_infos,
};
