mod assembly;
mod compression;
mod environment;
mod prompt_files;

pub(crate) use assembly::{plan_mode_builtin_tool_allowed, prepare_prompt_context};
pub(crate) use compression::{
    ContextUsageInput, LlmContextCompressionMode, active_compression_snapshots,
    active_llm_checkpoint_snapshot_ids, apply_compression_snapshot_to_messages,
    assistant_parts_checkpoint_replay_start_index, build_context_compression_summary_request,
    compress_all_runtime_tool_state_messages, compress_runtime_tool_state_messages_if_needed,
    compression_snapshot_message, context_compression_summary_has_benefit, context_message_groups,
    context_token_breakdown, context_usage_response, context_usage_segments,
    context_usage_segments_total, context_window_compression_trigger_tokens,
    ensure_context_compression, interleaved_tool_state_messages,
    llm_context_compression_group_indices, llm_context_compression_trigger_tokens,
    neutral_assistant_tool_call_message, neutral_message_estimated_tokens,
    neutral_tool_call_from_record, pack_neutral_messages, persist_chat_result,
    persist_running_llm_request, plan_llm_context_compression, recover_after_tool_round_cap,
    snapshot_covered_sequences,
};
#[cfg(test)]
pub(crate) use compression::{
    compress_all_runtime_tool_state, compress_runtime_tool_state_if_needed,
};
// Phase 3 remote LLM compression uses snapshot prepare/insert helpers from the sidecar.
#[allow(unused_imports)]
pub(crate) use compression::{
    PreparedContextCompressionSnapshot, build_context_compression_snapshot_record,
    insert_context_compression_snapshot_record, next_context_compression_snapshot_sequence,
    prepare_context_compression_snapshot,
};
pub(crate) use environment::environment_context_message;
#[cfg(all(not(windows), not(target_os = "macos")))]
pub(crate) use environment::is_wsl_environment;
pub(crate) use prompt_files::{
    active_system_prompt, agents_prompt_messages, builtin_tool_definitions_for_runtime,
    configured_extra_prompt_message, configured_prompt_messages, system_prompt_summaries,
    tool_prompt_infos,
};
