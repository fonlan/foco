use std::{collections::HashSet, time::Duration};

use foco_agent::{ContextPackItem, context_compression_trigger_tokens, estimate_text_tokens};
use foco_providers::{
    NeutralChatMessage, NeutralChatRole, NeutralToolCall, stream_chat_with_capture_observer,
};
use foco_store::config::PromptSettings;
use foco_store::workspace::{
    ContextCompressionSnapshotRecord, NewPlanPhaseDerivedEffects, ToolCallWithResultRecord,
};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::context_compression_policy::{
    ContextCompressionAttemptDeadline, ContextCompressionFailureAction, ContextCompressionMode,
    ContextCompressionRetryBudget, context_compression_failure_action,
};
use crate::http::chat::{ContextUsageResponse, ContextUsageSegments};
use crate::*;

pub(crate) fn neutral_tool_call_from_record(
    record: &ToolCallWithResultRecord,
) -> Result<NeutralToolCall, ApiError> {
    Ok(NeutralToolCall {
        call_id: record.id.clone(),
        name: record.tool_name.clone(),
        arguments: parse_json_value(&record.input_json, "tool call input")?,
        thought_signatures: None,
    })
}

fn neutral_tool_message_from_executed_tool_call(
    tool_result: &ExecutedToolCall,
) -> NeutralChatMessage {
    NeutralChatMessage {
        role: NeutralChatRole::Tool,
        content: serde_json::to_string(&tool_result.output)
            .expect("tool outputs are always JSON serializable"),
        attachments: Vec::new(),
        reasoning: None,
        tool_calls: Vec::new(),
        tool_call_id: Some(tool_result.id.clone()),
        tool_name: Some(tool_result.name.clone()),
    }
}

pub(crate) fn neutral_assistant_tool_call_message(
    tool_call: NeutralToolCall,
    assistant_text: String,
    assistant_reasoning: Option<String>,
) -> NeutralChatMessage {
    NeutralChatMessage {
        role: NeutralChatRole::Assistant,
        content: assistant_text,
        attachments: Vec::new(),
        reasoning: assistant_reasoning,
        tool_calls: vec![tool_call],
        tool_call_id: None,
        tool_name: None,
    }
}

pub(crate) fn interleaved_tool_state_messages(
    tool_calls: Vec<NeutralToolCall>,
    tool_results: &[ExecutedToolCall],
    assistant_text: String,
    assistant_reasoning: Option<String>,
) -> Vec<NeutralChatMessage> {
    let mut messages = Vec::with_capacity(tool_calls.len() * 2);
    let mut assistant_text = Some(assistant_text);
    let mut assistant_reasoning = assistant_reasoning;

    for tool_call in tool_calls {
        messages.push(neutral_assistant_tool_call_message(
            tool_call.clone(),
            assistant_text.take().unwrap_or_default(),
            assistant_reasoning.take(),
        ));

        let tool_result = tool_results
            .iter()
            .find(|tool_result| tool_result.id == tool_call.call_id)
            .expect("executed tool results must match completed tool calls");
        messages.push(neutral_tool_message_from_executed_tool_call(tool_result));
    }

    messages
}

fn validate_prompt_context_lengths(
    messages: &[NeutralChatMessage],
    message_source_sequences: &[Option<i64>],
    message_context_sources: &[PromptContextSource],
) -> Result<(), ApiError> {
    if messages.len() != message_source_sequences.len() {
        return Err(ApiError::internal(
            "context message source sequence count does not match prompt message count",
        ));
    }
    if messages.len() != message_context_sources.len() {
        return Err(ApiError::internal(
            "context message source classification count does not match prompt message count",
        ));
    }

    Ok(())
}

/// Ensure runtime tool-state + optional LLM checkpoint before the next provider request.
///
/// Overflow matrix for `runtime_tool_state_compression_enabled`:
///
/// | switch | 80% force=false | required overflow force | RequiredOverflow LLM checkpoint |
/// | --- | --- | --- | --- |
/// | OFF | skip | skip (force does not bypass switch) | full conversation checkpoint if still over budget |
/// | ON | may trigger | force local tool-state first | checkpoint only if still over budget after force |
///
/// LLM checkpoint candidates include raw `RuntimeToolState` (no local snapshot required). The 80%
/// local tool-state path remains an optional independent optimization that still preserves the
/// recent two tool batches. Tool-round-cap recovery via `compress_all_runtime_tool_state` is not
/// gated by this switch.
///
/// When `event_tx` is provided, compression events (especially LLM `start` before the summary
/// provider request and `completed` after snapshot persistence) are also pushed immediately so
/// the chat SSE loop can yield them while the compression future is still running.
pub(crate) async fn ensure_context_compression(
    context: &mut PreparedChatContext,
    event_tx: Option<mpsc::UnboundedSender<ContextCompressionEventDetail>>,
) -> Result<ContextCompressionResult, ApiError> {
    // Runtime tool-state and LLM compression can themselves issue an LLM request. Resolve
    // the model's active provider at that boundary instead of reusing a queued run snapshot.
    context.refresh_model_route()?;
    validate_prompt_context_lengths(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
    )?;

    let mut events = Vec::new();
    let mut runtime_tool_state_compressed = compress_runtime_tool_state_with_events_if_needed(
        context,
        false,
        &mut events,
        event_tx.as_ref(),
    )?;

    let mut message_groups = context_message_groups(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
        context.active_tool_start_index,
    )?;
    let segments = context_usage_segments(&context.context_budget, &message_groups);
    let total_used_context_tokens = context_usage_segments_total(&segments);
    if should_trigger_normal_llm_context_compression(
        total_used_context_tokens,
        context.last_chat_completion_input_tokens,
        context.context_budget.context_window,
    ) && ensure_llm_context_compression(
        context,
        &message_groups,
        &mut events,
        event_tx.as_ref(),
        LlmContextCompressionMode::Normal,
        total_used_context_tokens,
    )
    .await?
    {
        return Ok(ContextCompressionResult {
            active_tool_start_index: context.active_tool_start_index,
            runtime_tool_state_compressed,
            events,
        });
    }

    let mut breakdown = context_token_breakdown(&message_groups);
    if breakdown.required_tokens > context.context_budget.available_message_tokens {
        // force=true only bypasses the 80%/threshold trigger when the switch is ON; when OFF this
        // call is a no-op so overflow goes straight to RequiredOverflow LLM without a tool-state
        // prefix rewrite (avoids stacked prompt-cache invalidation).
        if !runtime_tool_state_compressed {
            runtime_tool_state_compressed |= compress_runtime_tool_state_with_events_if_needed(
                context,
                true,
                &mut events,
                event_tx.as_ref(),
            )?;
        }
        message_groups = context_message_groups(
            &context.provider_request.messages,
            &context.message_source_sequences,
            &context.message_context_sources,
            context.active_tool_start_index,
        )?;
        breakdown = context_token_breakdown(&message_groups);
        if breakdown.required_tokens > context.context_budget.available_message_tokens
            && ensure_llm_context_compression(
                context,
                &message_groups,
                &mut events,
                event_tx.as_ref(),
                LlmContextCompressionMode::RequiredOverflow,
                total_used_context_tokens,
            )
            .await?
        {
            return Ok(ContextCompressionResult {
                active_tool_start_index: context.active_tool_start_index,
                runtime_tool_state_compressed,
                events,
            });
        }
    }

    Ok(ContextCompressionResult {
        active_tool_start_index: context.active_tool_start_index,
        runtime_tool_state_compressed,
        events,
    })
}

fn context_compression_event_detail(
    status: &str,
    kind: &str,
    compression_id: Option<String>,
    snapshot_id: Option<String>,
    original_token_count: Option<i64>,
    summary_token_count: Option<i64>,
    started_at: Option<String>,
    completed_at: Option<String>,
    context: &PreparedChatContext,
) -> ContextCompressionEventDetail {
    ContextCompressionEventDetail {
        status: status.to_string(),
        kind: kind.to_string(),
        compression_id,
        snapshot_id,
        original_token_count,
        summary_token_count,
        started_at,
        completed_at,
        provider_id: context.provider_id.clone(),
        model_id: context.model_id.clone(),
        provider_request_id: None,
        compression_mode: None,
        attempt_index: None,
        outcome: None,
        action: None,
        error_message: None,
    }
}

fn context_compression_no_snapshot_event(
    mut event: ContextCompressionEventDetail,
    status: &str,
    provider_request_id: &str,
    mode: LlmContextCompressionMode,
    attempt_index: u32,
    action: &str,
    error_message: &str,
) -> ContextCompressionEventDetail {
    event.status = status.to_string();
    event.snapshot_id = None;
    event.summary_token_count = None;
    event.completed_at = Some(utc_timestamp());
    event.provider_request_id = Some(provider_request_id.to_string());
    event.compression_mode = Some(ContextCompressionMode::from(mode).as_str().to_string());
    event.attempt_index = Some(attempt_index);
    event.outcome = Some(status.to_string());
    event.action = Some(action.to_string());
    event.error_message = Some(error_message.to_string());
    event
}

/// Record a compression event for the returned batch and, when present, push it on the live sink.
fn push_context_compression_event(
    events: &mut Vec<ContextCompressionEventDetail>,
    event_tx: Option<&mpsc::UnboundedSender<ContextCompressionEventDetail>>,
    detail: ContextCompressionEventDetail,
) {
    if let Some(tx) = event_tx {
        let _ = tx.send(detail.clone());
    }
    events.push(detail);
}

/// Sleep between isolated compression attempts while allowing application shutdown to preempt the
/// wait. The enclosing chat stream handles user cancellation before its next provider turn.
async fn wait_context_compression_retry_backoff_cancellable(
    app_shutdown_rx: &mut watch::Receiver<bool>,
    delay: Duration,
) -> bool {
    if *app_shutdown_rx.borrow() {
        return true;
    }
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        changed = app_shutdown_rx.changed() => changed.is_err() || *app_shutdown_rx.borrow(),
    }
}

fn compress_runtime_tool_state_with_events_if_needed(
    context: &mut PreparedChatContext,
    force: bool,
    events: &mut Vec<ContextCompressionEventDetail>,
    event_tx: Option<&mpsc::UnboundedSender<ContextCompressionEventDetail>>,
) -> Result<bool, ApiError> {
    let compression_started_at = utc_timestamp();
    let compressed = compress_runtime_tool_state_if_needed(context, force)?;
    if !compressed {
        return Ok(false);
    }
    let compression_id = unique_id("compression");

    push_context_compression_event(
        events,
        event_tx,
        context_compression_event_detail(
            "start",
            CONTEXT_COMPRESSION_KIND_RUNTIME_TOOL_STATE,
            Some(compression_id.clone()),
            None,
            None,
            None,
            Some(compression_started_at.clone()),
            None,
            context,
        ),
    );
    push_context_compression_event(
        events,
        event_tx,
        context_compression_event_detail(
            "completed",
            CONTEXT_COMPRESSION_KIND_RUNTIME_TOOL_STATE,
            Some(compression_id),
            None,
            None,
            None,
            Some(compression_started_at),
            Some(utc_timestamp()),
            context,
        ),
    );
    Ok(true)
}

#[derive(Clone, Copy)]
pub(crate) enum LlmContextCompressionMode {
    Normal,
    RequiredOverflow,
}

/// Pure planning input for LLM context checkpoint (local + remote).
#[derive(Clone, Debug)]
pub(crate) struct LlmContextCompressionPlan {
    pub covered_message_indices: Vec<usize>,
    pub original_tokens: u64,
    /// Ordered Neutral messages for the dedicated checkpoint request (raw conversation state).
    pub checkpoint_messages: Vec<NeutralChatMessage>,
    pub covered_sequences: Vec<i64>,
    pub covered_snapshot_ids: Vec<String>,
}

/// Message arrays after replacing covered indices with a compression snapshot message.
#[derive(Clone, Debug)]
pub(crate) struct ReplacedPromptMessages {
    pub messages: Vec<NeutralChatMessage>,
    pub message_source_sequences: Vec<Option<i64>>,
    pub message_context_sources: Vec<PromptContextSource>,
    pub active_tool_start_index: usize,
}

/// Select covered groups/messages and build checkpoint message list without I/O or provider calls.
pub(crate) fn plan_llm_context_compression(
    messages: &[NeutralChatMessage],
    message_source_sequences: &[Option<i64>],
    message_context_sources: &[PromptContextSource],
    compression_snapshots: &[ContextCompressionSnapshotRecord],
    message_groups: &[ContextMessageGroup],
    available_message_tokens: u64,
    mode: LlmContextCompressionMode,
) -> Result<Option<LlmContextCompressionPlan>, ApiError> {
    validate_prompt_context_lengths(messages, message_source_sequences, message_context_sources)?;
    let covered_group_indices =
        llm_context_compression_group_indices(message_groups, available_message_tokens, mode);
    if covered_group_indices.is_empty() {
        return Ok(None);
    }
    let mut covered_message_indices =
        message_group_indices(message_groups, &covered_group_indices)?;
    covered_message_indices =
        trim_covered_indices_to_complete_tool_pairs(messages, covered_message_indices);
    if covered_message_indices.is_empty() {
        return Ok(None);
    }
    let original_tokens = covered_message_indices
        .iter()
        .map(|index| neutral_message_estimated_tokens(&messages[*index]))
        .sum::<u64>();
    if original_tokens == 0 {
        return Ok(None);
    }
    let checkpoint_messages = build_checkpoint_messages(
        messages,
        message_context_sources,
        compression_snapshots,
        &covered_message_indices,
    )?;
    let covered_snapshot_ids = compression_covered_snapshot_ids(
        messages,
        message_context_sources,
        compression_snapshots,
        &covered_message_indices,
    );
    let covered_sequences = compression_covered_sequences_allowing_snapshots(
        messages,
        message_source_sequences,
        compression_snapshots,
        &covered_message_indices,
    );
    Ok(Some(LlmContextCompressionPlan {
        covered_message_indices,
        original_tokens,
        checkpoint_messages,
        covered_sequences,
        covered_snapshot_ids,
    }))
}

/// Replace covered messages with a snapshot system message; keeps parallel arrays aligned.
pub(crate) fn apply_compression_snapshot_to_messages(
    messages: &[NeutralChatMessage],
    message_source_sequences: &[Option<i64>],
    message_context_sources: &[PromptContextSource],
    active_tool_start_index: usize,
    covered_indices: &[usize],
    snapshot_message: NeutralChatMessage,
) -> ReplacedPromptMessages {
    ReplacedPromptMessages {
        messages: replace_covered_messages_with_snapshot(
            messages,
            covered_indices,
            snapshot_message,
        ),
        message_source_sequences: replace_covered_sequences_with_snapshot(
            message_source_sequences,
            covered_indices,
        ),
        message_context_sources: replace_covered_sources_with_snapshot(
            message_context_sources,
            covered_indices,
            PromptContextSource::CompressionSnapshot,
        ),
        active_tool_start_index: compressed_active_tool_start_index(
            active_tool_start_index,
            covered_indices,
        ),
    }
}

/// Build a snapshot record shell (metadata_json left empty for the caller to fill).
pub(crate) fn build_context_compression_snapshot_record(
    id: String,
    chat_id: String,
    run_id: String,
    sequence: i64,
    summary: String,
    message_source_sequences: &[Option<i64>],
    covered_indices: &[usize],
    original_tokens: u64,
    summary_token_count: u64,
    created_at: String,
) -> Result<ContextCompressionSnapshotRecord, ApiError> {
    let original_token_count = i64::try_from(original_tokens)
        .map_err(|_| ApiError::internal("context compression original token count exceeds i64"))?;
    let summary_token_count_i64 = i64::try_from(summary_token_count)
        .map_err(|_| ApiError::internal("context compression summary token count exceeds i64"))?;
    let (source_message_start_sequence, source_message_end_sequence) =
        compression_source_sequence_range(message_source_sequences, covered_indices);
    Ok(ContextCompressionSnapshotRecord {
        id,
        chat_id,
        run_id,
        sequence,
        summary,
        source_message_start_sequence,
        source_message_end_sequence,
        original_token_count,
        summary_token_count: summary_token_count_i64,
        created_at,
        metadata_json: String::new(),
    })
}

async fn ensure_llm_context_compression(
    context: &mut PreparedChatContext,
    message_groups: &[ContextMessageGroup],
    events: &mut Vec<ContextCompressionEventDetail>,
    event_tx: Option<&mpsc::UnboundedSender<ContextCompressionEventDetail>>,
    mode: LlmContextCompressionMode,
    local_total_used_tokens: u64,
) -> Result<bool, ApiError> {
    let Some(plan) = plan_llm_context_compression(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
        &context.compression_snapshots,
        message_groups,
        context.context_budget.available_message_tokens,
        mode,
    )?
    else {
        return Ok(false);
    };
    let covered_indices = plan.covered_message_indices;
    let original_tokens = plan.original_tokens;
    let checkpoint_messages = plan.checkpoint_messages;
    let covered_snapshot_ids = plan.covered_snapshot_ids;
    let covered_sequences = plan.covered_sequences;
    let compression_id = unique_id("compression");
    let compression_started_at = utc_timestamp();
    // Emit start before the summary provider request so the UI can show the compression block
    // while the dedicated checkpoint LLM call is still in flight.
    let mut start_event = context_compression_event_detail(
        "start",
        CONTEXT_COMPRESSION_KIND_LLM,
        Some(compression_id.clone()),
        None,
        Some(i64::try_from(original_tokens).map_err(|_| {
            ApiError::internal("context compression original token count exceeds i64")
        })?),
        None,
        Some(compression_started_at.clone()),
        None,
        context,
    );
    start_event.compression_mode = Some(ContextCompressionMode::from(mode).as_str().to_string());
    start_event.attempt_index = Some(0);
    start_event.outcome = Some("started".to_string());
    start_event.action = Some("request".to_string());
    push_context_compression_event(events, event_tx, start_event);
    // Index of the live start event; pop only from the batch vec on failure (live already sent).
    let compression_start_event_index = events.len() - 1;

    let retry_budget = ContextCompressionRetryBudget::from_configured_retry_count(
        context.global_config.app.llm_request_retry_count,
    );
    let mut retries_used = 0;
    let summary = loop {
        let attempt_deadline = retry_budget.attempt_deadline();
        match llm_context_compression_summary(context, &checkpoint_messages, attempt_deadline).await
        {
            Ok(summary) => break summary,
            Err(error) => {
                let action = context_compression_failure_action(
                    ContextCompressionMode::from(mode),
                    error.retry_class,
                    retries_used,
                    retry_budget,
                    *context.app_shutdown_rx.borrow(),
                );
                let next_retry_ordinal = retries_used.saturating_add(1);
                let retry_delay = matches!(action, ContextCompressionFailureAction::Retry)
                    .then(|| {
                        retry_budget.retry_backoff(
                            error.retry_class,
                            retries_used,
                            error.retry_after,
                        )
                    })
                    .flatten();
                tracing::warn!(
                    workspace_id = %context.workspace_id,
                    chat_id = %context.chat_id,
                    run_id = %context.llm_request_id,
                    compression_id = %compression_id,
                    provider_id = %context.provider_id,
                    model_id = %context.model_id,
                    compression_mode = ContextCompressionMode::from(mode).as_str(),
                    input_token_count = original_tokens,
                    retry_class = error.retry_class.as_str(),
                    attempt_index = retries_used,
                    action = action.as_str(),
                    "context compression provider attempt failed"
                );
                let mut terminal_event = context_compression_event_detail(
                    match action {
                        ContextCompressionFailureAction::ContinueWithoutCompression => "skipped",
                        ContextCompressionFailureAction::FailRequiredOverflow => "failed",
                        ContextCompressionFailureAction::Stop => "cancelled",
                        ContextCompressionFailureAction::Retry => "retrying",
                    },
                    CONTEXT_COMPRESSION_KIND_LLM,
                    Some(compression_id.clone()),
                    None,
                    Some(i64::try_from(original_tokens).map_err(|_| {
                        ApiError::internal("context compression original token count exceeds i64")
                    })?),
                    None,
                    Some(compression_started_at.clone()),
                    Some(utc_timestamp()),
                    context,
                );
                terminal_event.compression_mode =
                    Some(ContextCompressionMode::from(mode).as_str().to_string());
                terminal_event.attempt_index = Some(
                    if matches!(action, ContextCompressionFailureAction::Retry) {
                        next_retry_ordinal
                    } else {
                        retries_used
                    },
                );
                terminal_event.outcome = Some("failed".to_string());
                terminal_event.action = Some(action.as_str().to_string());
                terminal_event.provider_request_id = error.request_id.clone();
                // Provider diagnostics stay in the audit record. Chat history deliberately
                // carries only a stable, action-oriented summary.
                terminal_event.error_message =
                    Some(context_compression_event_error_summary(action).to_string());
                push_context_compression_event(events, event_tx, terminal_event);

                if matches!(action, ContextCompressionFailureAction::Retry)
                    && let Some(delay) = retry_delay
                {
                    if wait_context_compression_retry_backoff_cancellable(
                        &mut context.app_shutdown_rx,
                        delay,
                    )
                    .await
                    {
                        return Err(ApiError::bad_request(
                            "context compression summary was cancelled",
                        ));
                    }
                    retries_used = next_retry_ordinal;
                    continue;
                }

                if matches!(
                    action,
                    ContextCompressionFailureAction::ContinueWithoutCompression
                ) {
                    return Ok(false);
                }
                return Err(ApiError::bad_request(
                    "context compression is required to fit the next provider request; the compression provider could not produce a checkpoint. Start a new chat or reduce the conversation before retrying.",
                ));
            }
        }
    };
    let provider_request_id = summary.request_id.clone();
    let summary = summary.text;
    if !context_compression_summary_has_benefit(&summary, original_tokens) {
        let skipped_event = context_compression_no_snapshot_event(
            events[compression_start_event_index].clone(),
            "skipped",
            &provider_request_id,
            mode,
            retries_used,
            "summary_not_beneficial",
            "Compression summary did not reduce context enough; continuing without compression",
        );
        events.truncate(compression_start_event_index);
        push_context_compression_event(events, event_tx, skipped_event);
        return Ok(false);
    }
    let summary_token_count = estimate_text_tokens(&summary);
    let pre_summary = context
        .hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &context.global_hooks,
            api_audit_save_details: api_audit_save_details(&context.global_config),
            workspace_id: &context.workspace_id,
            workspace_path: &context.workspace_path,
            event: "PreCompact",
            match_value: None,
            chat_id: Some(&context.chat_id),
            run_id: Some(&context.llm_request_id),
            session_id: Some(&context.chat_id),
            tool_call_id: None,
            model_id: Some(&context.model_id),
            provider_id: Some(&context.provider_id),
            provider_config: Some(&context.provider_config),
            llm_request_retry_count: context.global_config.app.llm_request_retry_count,
            permission_mode: None,
            payload: json!({
                "kind": CONTEXT_COMPRESSION_KIND_LLM,
                "coveredSequences": covered_sequences,
                "originalTokenCount": original_tokens,
                "summaryTokenCount": summary_token_count,
                "summary": summary.clone(),
            }),
        })
        .await;
    context
        .hook_notifications
        .extend(pre_summary.hook_messages("PreCompact"));
    append_hook_context_messages(
        &mut context.provider_request.messages,
        &mut context.message_source_sequences,
        &mut context.message_context_sources,
        &pre_summary.additional_context,
    );
    if pre_summary.first_block_reason().is_some() {
        let skipped_event = context_compression_no_snapshot_event(
            events[compression_start_event_index].clone(),
            "skipped",
            &provider_request_id,
            mode,
            retries_used,
            "pre_compact_blocked",
            "Compression was blocked by a PreCompact hook; continuing without compression",
        );
        events.truncate(compression_start_event_index);
        push_context_compression_event(events, event_tx, skipped_event);
        return Ok(false);
    }

    let mut snapshot_metadata = json!({
        "kind": CONTEXT_COMPRESSION_KIND_LLM,
        "coveredSequences": covered_sequences,
        "coveredSnapshotIds": covered_snapshot_ids,
        "supersededSnapshotIds": covered_snapshot_ids,
        "triggerTokens": llm_context_compression_trigger_tokens(context.context_budget.context_window),
        "availableMessageTokens": context.context_budget.available_message_tokens
    });
    if matches!(mode, LlmContextCompressionMode::Normal)
        && let Some(trigger_source) = llm_context_compression_trigger_source(
            local_total_used_tokens,
            context.last_chat_completion_input_tokens,
            context.context_budget.context_window,
        )
    {
        snapshot_metadata["triggerSource"] = json!(trigger_source);
    }
    let snapshot = match persist_context_compression_snapshot(
        context,
        &covered_indices,
        summary,
        original_tokens,
        summary_token_count,
        CONTEXT_COMPRESSION_KIND_LLM,
        snapshot_metadata,
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            events.truncate(compression_start_event_index);
            let mut failed_event = context_compression_event_detail(
                "failed",
                CONTEXT_COMPRESSION_KIND_LLM,
                Some(compression_id.clone()),
                None,
                Some(i64::try_from(original_tokens).map_err(|_| {
                    ApiError::internal("context compression original token count exceeds i64")
                })?),
                None,
                Some(compression_started_at.clone()),
                Some(utc_timestamp()),
                context,
            );
            failed_event.compression_mode =
                Some(ContextCompressionMode::from(mode).as_str().to_string());
            failed_event.attempt_index = Some(retries_used);
            failed_event.outcome = Some("failed".to_string());
            failed_event.action = Some("snapshot_persistence_failed".to_string());
            failed_event.provider_request_id = Some(provider_request_id.clone());
            failed_event.error_message =
                Some("Checkpoint could not be saved; chat context was left unchanged".to_string());
            push_context_compression_event(events, event_tx, failed_event);
            tracing::warn!(
                workspace_id = %context.workspace_id,
                chat_id = %context.chat_id,
                run_id = %context.llm_request_id,
                compression_id,
                provider_id = %context.provider_id,
                model_id = %context.model_id,
                compression_mode = ContextCompressionMode::from(mode).as_str(),
                input_token_count = original_tokens,
                attempt_index = retries_used,
                outcome = "failed",
                action = "snapshot_persistence_failed",
                "context compression checkpoint persistence failed"
            );
            return Err(error);
        }
    };
    // A full LLM checkpoint covers prior RuntimeToolState snapshots; allow a new 80% local cycle.
    context.runtime_tool_state_compression_count = 0;
    // Pre-compression provider input is stale after a durable checkpoint; drop it so the next
    // Normal gate uses only the post-checkpoint local estimate until a new chat completion lands.
    context.last_chat_completion_input_tokens = None;
    // completed only after snapshot is durable so history reload never sees a half-success block.
    let mut completed_event = context_compression_event_detail(
        "completed",
        CONTEXT_COMPRESSION_KIND_LLM,
        Some(compression_id),
        Some(snapshot.id.clone()),
        Some(snapshot.original_token_count),
        Some(snapshot.summary_token_count),
        Some(compression_started_at),
        Some(utc_timestamp()),
        context,
    );
    completed_event.compression_mode =
        Some(ContextCompressionMode::from(mode).as_str().to_string());
    completed_event.attempt_index = Some(retries_used);
    completed_event.outcome = Some("succeeded".to_string());
    completed_event.action = Some("checkpoint_persisted".to_string());
    completed_event.provider_request_id = Some(provider_request_id);
    push_context_compression_event(events, event_tx, completed_event);
    tracing::info!(
        workspace_id = %context.workspace_id,
        chat_id = %context.chat_id,
        run_id = %context.llm_request_id,
        provider_id = %context.provider_id,
        model_id = %context.model_id,
        compression_mode = ContextCompressionMode::from(mode).as_str(),
        input_token_count = original_tokens,
        summary_token_count,
        attempt_index = retries_used,
        outcome = "succeeded",
        "context compression checkpoint persisted"
    );

    let post_summary = context
        .hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &context.global_hooks,
            api_audit_save_details: api_audit_save_details(&context.global_config),
            workspace_id: &context.workspace_id,
            workspace_path: &context.workspace_path,
            event: "PostCompact",
            match_value: None,
            chat_id: Some(&context.chat_id),
            run_id: Some(&context.llm_request_id),
            session_id: Some(&context.chat_id),
            tool_call_id: None,
            model_id: Some(&context.model_id),
            provider_id: Some(&context.provider_id),
            provider_config: Some(&context.provider_config),
            llm_request_retry_count: context.global_config.app.llm_request_retry_count,
            permission_mode: None,
            payload: json!({
                "kind": CONTEXT_COMPRESSION_KIND_LLM,
                "snapshotId": context.compression_snapshots.last().map(|snapshot| snapshot.id.clone()),
            }),
        })
        .await;
    context
        .hook_notifications
        .extend(post_summary.hook_messages("PostCompact"));
    append_hook_context_messages(
        &mut context.provider_request.messages,
        &mut context.message_source_sequences,
        &mut context.message_context_sources,
        &post_summary.additional_context,
    );

    Ok(true)
}

fn context_compression_event_error_summary(
    action: ContextCompressionFailureAction,
) -> &'static str {
    match action {
        ContextCompressionFailureAction::Retry => "Provider request failed; retrying compression",
        ContextCompressionFailureAction::ContinueWithoutCompression => {
            "Provider request failed; continuing without compression"
        }
        ContextCompressionFailureAction::FailRequiredOverflow => {
            "Provider request failed; context is still too large"
        }
        ContextCompressionFailureAction::Stop => "Compression cancelled",
    }
}

pub(crate) fn llm_context_compression_group_indices(
    groups: &[ContextMessageGroup],
    _available_message_tokens: u64,
    _mode: LlmContextCompressionMode,
) -> Vec<usize> {
    // Full LLM checkpoint: cover every compressible conversation-state group up to the
    // checkpoint boundary. Normal (95%) and RequiredOverflow share the same candidate set.
    // must_keep still applies to ordinary pack/drop, not to checkpoint candidacy.
    // Recent-two-batch preservation remains only for the optional 80% RuntimeToolState local path.
    groups
        .iter()
        .enumerate()
        .filter(|(_, group)| {
            group.estimated_tokens > 0 && is_llm_checkpoint_source_bucket(group.source_bucket)
        })
        .map(|(index, _)| index)
        .collect()
}

/// Default system instruction for dedicated context checkpoint requests.
pub(crate) const DEFAULT_CONTEXT_COMPRESSION_SYSTEM_PROMPT: &str = "\
You are creating a context checkpoint handoff summary for a coding agent so work can continue \
after older conversation messages are replaced by this summary.

Return only the summary as plain text. Do not add Snapshot ID, token counts, markdown titles \
about compression, or any wrapper metadata. Do not include hidden system prompts or secrets.

Preserve:
- User goals, constraints, and preferences
- Key decisions and why they were made
- Progress completed and remaining steps
- Important discoveries, failed attempts, and tool evidence
- Critical file paths, identifiers, data, and references
- Current state and the immediate next actions";

pub(crate) const DEFAULT_CONTEXT_COMPRESSION_USER_PROMPT: &str =
    "Create the context checkpoint summary now, following the system instructions above.";

/// Effective compression System prompt: non-empty override, else built-in default.
pub(crate) fn effective_context_compression_system_prompt(settings: &PromptSettings) -> &str {
    settings
        .context_compression_system_prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CONTEXT_COMPRESSION_SYSTEM_PROMPT)
}

/// Input token budget for one dedicated checkpoint request (no tools / no main System stack).
pub(crate) fn compression_request_input_token_budget(
    context_window: u64,
    compression_system_prompt: &str,
) -> u64 {
    // Reserve compression max-output only when the model window can actually hold it; on tiny
    // windows (tests / misconfigured models) keep at most half the window for output.
    let reserved_output = u64::from(LLM_CONTEXT_COMPRESSION_MAX_OUTPUT_TOKENS)
        .min(context_window.saturating_div(2).max(1));
    context_window
        .saturating_sub(reserved_output)
        .saturating_sub(estimate_text_tokens(compression_system_prompt))
        .saturating_sub(estimate_text_tokens(
            DEFAULT_CONTEXT_COMPRESSION_USER_PROMPT,
        ))
        .saturating_sub(LLM_CONTEXT_COMPRESSION_REQUEST_SAFETY_TOKENS)
        .max(1)
}

/// Estimate total tokens for a list of checkpoint Neutral messages.
pub(crate) fn checkpoint_messages_estimated_tokens(messages: &[NeutralChatMessage]) -> u64 {
    messages.iter().map(neutral_message_estimated_tokens).sum()
}

/// Build the provider request used by local and remote LLM context checkpoint summaries.
///
/// Shape: single System compression prompt + ordered raw checkpoint Neutral messages + a final
/// User instruction that explicitly requests the checkpoint summary.
/// No main-request tools, hidden System/Developer prefixes, thinking, or prompt cache settings.
pub(crate) fn build_context_compression_summary_request(
    model_id: &str,
    checkpoint_messages: &[NeutralChatMessage],
) -> NeutralChatRequest {
    build_context_compression_summary_request_with_prompt(
        model_id,
        checkpoint_messages,
        DEFAULT_CONTEXT_COMPRESSION_SYSTEM_PROMPT,
    )
}

pub(crate) fn build_context_compression_summary_request_with_prompt(
    model_id: &str,
    checkpoint_messages: &[NeutralChatMessage],
    compression_system_prompt: &str,
) -> NeutralChatRequest {
    let mut messages = Vec::with_capacity(checkpoint_messages.len().saturating_add(2));
    messages.push(neutral_text_message(
        NeutralChatRole::System,
        compression_system_prompt.to_string(),
    ));
    messages.extend(checkpoint_messages.iter().cloned());
    messages.push(neutral_text_message(
        NeutralChatRole::User,
        DEFAULT_CONTEXT_COMPRESSION_USER_PROMPT.to_string(),
    ));
    NeutralChatRequest {
        model_id: model_id.to_string(),
        messages,
        tools: Vec::new(),
        thinking_level: None,
        max_output_tokens: Some(LLM_CONTEXT_COMPRESSION_MAX_OUTPUT_TOKENS),
        prompt_cache_key: None,
        prompt_cache_retention: None,
        agent_correlation: None,
        tool_choice: foco_providers::NeutralToolChoice::Auto,
    }
}

/// True when the summary is non-empty pure text strictly smaller than the covered original.
pub(crate) fn context_compression_summary_has_benefit(summary: &str, original_tokens: u64) -> bool {
    let summary = summary.trim();
    !summary.is_empty() && estimate_text_tokens(summary) < original_tokens
}

/// Split checkpoint messages into request-sized batches when they exceed one model window.
///
/// Preference order: whole tool batches / message groups → single-message content fragments.
/// Every original character is covered by some chunk (no silent truncation).
pub(crate) fn plan_context_compression_checkpoint_chunks(
    checkpoint_messages: &[NeutralChatMessage],
    input_token_budget: u64,
) -> Result<Vec<Vec<NeutralChatMessage>>, ApiError> {
    if checkpoint_messages.is_empty() {
        return Ok(vec![Vec::new()]);
    }
    let budget = input_token_budget.max(1);
    if checkpoint_messages_estimated_tokens(checkpoint_messages) <= budget {
        return Ok(vec![checkpoint_messages.to_vec()]);
    }

    let units = checkpoint_message_units(checkpoint_messages);
    let mut chunks: Vec<Vec<NeutralChatMessage>> = Vec::new();
    let mut current: Vec<NeutralChatMessage> = Vec::new();
    let mut current_tokens = 0u64;

    for unit in units {
        let unit_tokens = checkpoint_messages_estimated_tokens(&unit);
        if unit_tokens <= budget {
            if !current.is_empty() && current_tokens.saturating_add(unit_tokens) > budget {
                chunks.push(std::mem::take(&mut current));
                current_tokens = 0;
            }
            current_tokens = current_tokens.saturating_add(unit_tokens);
            current.extend(unit);
            continue;
        }

        // Flush partial chunk before splitting an oversized unit.
        if !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
            current_tokens = 0;
        }
        for fragment in split_oversized_checkpoint_unit(&unit, budget)? {
            chunks.push(fragment);
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }
    if chunks.is_empty() {
        return Err(ApiError::internal(
            "context compression checkpoint chunk planner produced no chunks",
        ));
    }
    Ok(chunks)
}

/// Group messages into atomic units: non-tool alone, or assistant tool_calls + following tool results.
fn checkpoint_message_units(messages: &[NeutralChatMessage]) -> Vec<Vec<NeutralChatMessage>> {
    let mut units = Vec::new();
    let mut index = 0;
    while index < messages.len() {
        let message = &messages[index];
        if !message.tool_calls.is_empty() {
            let mut unit = vec![message.clone()];
            index += 1;
            while index < messages.len() && messages[index].role == NeutralChatRole::Tool {
                unit.push(messages[index].clone());
                index += 1;
            }
            units.push(unit);
            continue;
        }
        units.push(vec![message.clone()]);
        index += 1;
    }
    units
}

fn split_oversized_checkpoint_unit(
    unit: &[NeutralChatMessage],
    budget: u64,
) -> Result<Vec<Vec<NeutralChatMessage>>, ApiError> {
    let mut fragments: Vec<Vec<NeutralChatMessage>> = Vec::new();
    for message in unit {
        let message_tokens = neutral_message_estimated_tokens(message);
        if message_tokens <= budget {
            // Prefer attaching small messages to the previous fragment when room remains.
            if let Some(last) = fragments.last_mut() {
                let last_tokens = checkpoint_messages_estimated_tokens(last);
                if last_tokens.saturating_add(message_tokens) <= budget {
                    last.push(message.clone());
                    continue;
                }
            }
            fragments.push(vec![message.clone()]);
            continue;
        }
        for piece in split_oversized_checkpoint_message(message, budget)? {
            fragments.push(vec![piece]);
        }
    }
    if fragments.is_empty() {
        return Err(ApiError::internal(
            "context compression failed to split oversized checkpoint unit",
        ));
    }
    Ok(fragments)
}

/// Stable text form of tool-call arguments for fragment coverage (no silent truncation).
fn checkpoint_arguments_payload_text(arguments: &Value) -> String {
    match arguments {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| other.to_string()),
    }
}

/// Split a single oversized message into auditable fragments that fully cover the original payload.
fn split_oversized_checkpoint_message(
    message: &NeutralChatMessage,
    budget: u64,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    if !message.tool_calls.is_empty() {
        return split_oversized_assistant_tool_call_message(message, budget);
    }

    let payload = if !message.content.is_empty() {
        message.content.clone()
    } else if let Some(reasoning) = message.reasoning.as_ref().filter(|text| !text.is_empty()) {
        // Reasoning-only oversize: shard reasoning text into content-bearing fragments.
        reasoning.clone()
    } else {
        return Err(ApiError::internal(
            "context compression cannot split oversized message without text/tool payload",
        ));
    };

    let tool_name = message.tool_name.as_deref().unwrap_or("message");
    let call_id = message.tool_call_id.as_deref().unwrap_or("none");
    let parts = split_payload_text_into_shells(message, None, &payload, budget)?;
    verify_content_fragment_coverage(&parts, &payload, tool_name, call_id)?;
    Ok(parts)
}

/// Split an assistant message that carries one or more tool calls.
///
/// Each tool call is emitted alone (no sibling full arguments). Oversized arguments are
/// sharded with `contextCheckpointFragment.argumentsText` covering the full args payload.
fn split_oversized_assistant_tool_call_message(
    message: &NeutralChatMessage,
    budget: u64,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    let mut parts = Vec::new();

    // Keep assistant text/reasoning as separate messages so tool-call shards stay under budget.
    if !message.content.is_empty()
        || message
            .reasoning
            .as_ref()
            .is_some_and(|text| !text.is_empty())
    {
        let text_only = NeutralChatMessage {
            role: NeutralChatRole::Assistant,
            content: message.content.clone(),
            attachments: Vec::new(),
            reasoning: message.reasoning.clone(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
        };
        if neutral_message_estimated_tokens(&text_only) <= budget {
            parts.push(text_only);
        } else {
            parts.extend(split_oversized_checkpoint_message(&text_only, budget)?);
        }
    }

    for tool_call in &message.tool_calls {
        let args_text = checkpoint_arguments_payload_text(&tool_call.arguments);
        let single = assistant_tool_call_message_shell(tool_call, tool_call.arguments.clone());
        if neutral_message_estimated_tokens(&single) <= budget {
            parts.push(single);
            continue;
        }

        let sharded = split_payload_text_into_shells(message, Some(tool_call), &args_text, budget)?;
        let mut reconstructed = String::new();
        for fragment in &sharded {
            if fragment.tool_calls.len() != 1 {
                return Err(ApiError::internal(
                    "context compression assistant fragment must carry exactly one tool call",
                ));
            }
            let Some(call) = fragment.tool_calls.first() else {
                return Err(ApiError::internal(
                    "context compression assistant fragment missing tool call",
                ));
            };
            let Some(fragment_meta) = call.arguments.get("contextCheckpointFragment") else {
                return Err(ApiError::internal(
                    "context compression assistant fragment missing fragment metadata",
                ));
            };
            let slice = fragment_meta
                .get("argumentsText")
                .and_then(Value::as_str)
                .unwrap_or("");
            reconstructed.push_str(slice);
        }
        if reconstructed != args_text {
            return Err(ApiError::internal(format!(
                "context compression fragment split lost tool-call arguments coverage for {}",
                tool_call.call_id
            )));
        }
        parts.extend(sharded);
    }

    if parts.is_empty() {
        return Err(ApiError::internal(
            "context compression failed to split oversized assistant tool-call message",
        ));
    }
    Ok(parts)
}

fn assistant_tool_call_message_shell(
    tool_call: &NeutralToolCall,
    arguments: Value,
) -> NeutralChatMessage {
    NeutralChatMessage {
        role: NeutralChatRole::Assistant,
        content: String::new(),
        attachments: Vec::new(),
        reasoning: None,
        tool_calls: vec![NeutralToolCall {
            call_id: tool_call.call_id.clone(),
            name: tool_call.name.clone(),
            arguments,
            thought_signatures: tool_call.thought_signatures.clone(),
        }],
        tool_call_id: None,
        tool_name: None,
    }
}

fn split_payload_text_into_shells(
    message: &NeutralChatMessage,
    tool_call: Option<&NeutralToolCall>,
    payload: &str,
    budget: u64,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    if payload.is_empty() {
        let empty = truncated_checkpoint_message_shell(message, tool_call, 1, 1, "");
        if neutral_message_estimated_tokens(&empty) > budget {
            return Err(ApiError::internal(format!(
                "context compression fragment shell exceeds budget {budget}"
            )));
        }
        return Ok(vec![empty]);
    }

    let chars: Vec<char> = payload.chars().collect();
    let mut max_chars = 1usize;
    let mut lo = 1usize;
    let mut hi = chars.len().max(1);
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let slice: String = chars[..mid.min(chars.len())].iter().collect();
        let probe = truncated_checkpoint_message_shell(message, tool_call, 999, 999, &slice);
        if neutral_message_estimated_tokens(&probe) <= budget {
            max_chars = mid;
            lo = mid.saturating_add(1);
        } else if mid == 0 {
            break;
        } else {
            hi = mid - 1;
        }
    }
    max_chars = max_chars.max(1);

    // Ensure at least a 1-char fragment can fit; otherwise budget is unusable.
    let one_char = truncated_checkpoint_message_shell(message, tool_call, 1, 1, "x");
    if neutral_message_estimated_tokens(&one_char) > budget {
        return Err(ApiError::internal(format!(
            "context compression fragment shell exceeds budget {budget}"
        )));
    }

    let total_parts = chars.len().div_ceil(max_chars).max(1);
    if total_parts > 10_000 {
        return Err(ApiError::internal(format!(
            "context compression cannot split message into {total_parts} fragments under budget {budget}"
        )));
    }

    let mut parts = Vec::with_capacity(total_parts);
    let mut offset = 0usize;
    let mut part_index = 1usize;
    while offset < chars.len() {
        let end = (offset + max_chars).min(chars.len());
        let slice: String = chars[offset..end].iter().collect();
        let fragment =
            truncated_checkpoint_message_shell(message, tool_call, part_index, total_parts, &slice);
        if neutral_message_estimated_tokens(&fragment) > budget && slice.chars().count() > 1 {
            let smaller_end = offset + (end - offset) / 2;
            let smaller_end = smaller_end.max(offset + 1);
            let slice: String = chars[offset..smaller_end].iter().collect();
            parts.push(truncated_checkpoint_message_shell(
                message,
                tool_call,
                part_index,
                total_parts,
                &slice,
            ));
            offset = smaller_end;
        } else {
            parts.push(fragment);
            offset = end;
        }
        part_index += 1;
        if part_index > total_parts.saturating_add(total_parts) {
            return Err(ApiError::internal(
                "context compression fragment split exceeded expected part count",
            ));
        }
    }

    let actual_total = parts.len();
    if actual_total != total_parts {
        for (index, part) in parts.iter_mut().enumerate() {
            let body = fragment_payload_body(part, tool_call);
            *part = truncated_checkpoint_message_shell(
                message,
                tool_call,
                index + 1,
                actual_total,
                &body,
            );
        }
    }

    Ok(parts)
}

fn fragment_payload_body(part: &NeutralChatMessage, tool_call: Option<&NeutralToolCall>) -> String {
    if tool_call.is_some() {
        if let Some(call) = part.tool_calls.first() {
            if let Some(meta) = call.arguments.get("contextCheckpointFragment") {
                if let Some(text) = meta.get("argumentsText").and_then(Value::as_str) {
                    return text.to_string();
                }
            }
        }
    }
    part.content
        .split_once('\n')
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_default()
}

fn verify_content_fragment_coverage(
    parts: &[NeutralChatMessage],
    original: &str,
    _tool_name: &str,
    _call_id: &str,
) -> Result<(), ApiError> {
    let mut reconstructed = String::new();
    for part in parts {
        if let Some((_, body)) = part.content.split_once('\n') {
            reconstructed.push_str(body);
        }
    }
    if reconstructed != original {
        return Err(ApiError::internal(
            "context compression fragment split lost original content coverage",
        ));
    }
    Ok(())
}

fn truncated_checkpoint_message_shell(
    source: &NeutralChatMessage,
    tool_call: Option<&NeutralToolCall>,
    part_index: usize,
    total_parts: usize,
    payload_slice: &str,
) -> NeutralChatMessage {
    let tool_name = tool_call
        .map(|call| call.name.as_str())
        .or(source.tool_name.as_deref())
        .or_else(|| source.tool_calls.first().map(|call| call.name.as_str()))
        .unwrap_or("message");
    let call_id = tool_call
        .map(|call| call.call_id.as_str())
        .or(source.tool_call_id.as_deref())
        .or_else(|| source.tool_calls.first().map(|call| call.call_id.as_str()))
        .unwrap_or("none");
    let header = format!(
        "[context_checkpoint_fragment tool={tool_name} call_id={call_id} part={part_index}/{total_parts}]\n"
    );

    if let Some(tool_call) = tool_call {
        // Single tool-call fragment: full original args replaced by auditable slice metadata.
        NeutralChatMessage {
            role: NeutralChatRole::Assistant,
            content: format!("{header}{payload_slice}"),
            attachments: Vec::new(),
            reasoning: None,
            tool_calls: vec![NeutralToolCall {
                call_id: tool_call.call_id.clone(),
                name: tool_call.name.clone(),
                arguments: json!({
                    "contextCheckpointFragment": {
                        "tool": tool_call.name,
                        "callId": tool_call.call_id,
                        "part": part_index,
                        "totalParts": total_parts,
                        "argumentsText": payload_slice,
                    }
                }),
                thought_signatures: tool_call.thought_signatures.clone(),
            }],
            tool_call_id: None,
            tool_name: None,
        }
    } else if source.role == NeutralChatRole::Tool || source.tool_call_id.is_some() {
        NeutralChatMessage {
            role: NeutralChatRole::Tool,
            content: format!("{header}{payload_slice}"),
            attachments: Vec::new(),
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: Some(call_id.to_string()),
            tool_name: Some(tool_name.to_string()),
        }
    } else {
        NeutralChatMessage {
            role: source.role.clone(),
            content: format!("{header}{payload_slice}"),
            attachments: Vec::new(),
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
        }
    }
}

/// Build merge-level checkpoint messages from intermediate chunk summaries.
pub(crate) fn build_chunk_merge_checkpoint_messages(
    chunk_summaries: &[String],
) -> Vec<NeutralChatMessage> {
    let total = chunk_summaries.len();
    chunk_summaries
        .iter()
        .enumerate()
        .map(|(index, summary)| {
            neutral_text_message(
                NeutralChatRole::User,
                format!(
                    "[context_checkpoint_chunk_summary part={}/{}]\n{}",
                    index + 1,
                    total,
                    summary.trim()
                ),
            )
        })
        .collect()
}

async fn llm_context_compression_summary(
    context: &mut PreparedChatContext,
    checkpoint_messages: &[NeutralChatMessage],
    deadline: ContextCompressionAttemptDeadline,
) -> Result<ContextCompressionSummary, ContextCompressionSummaryError> {
    let compression_prompt =
        effective_context_compression_system_prompt(&context.global_config.prompts).to_string();
    let input_budget = compression_request_input_token_budget(
        context.context_budget.context_window,
        &compression_prompt,
    );
    let mut pending = plan_context_compression_checkpoint_chunks(checkpoint_messages, input_budget)
        .map_err(ContextCompressionSummaryError::non_retryable)?;
    let mut requests_used = 0usize;
    let mut depth = 0usize;

    loop {
        depth = depth.saturating_add(1);
        if depth > LLM_CONTEXT_COMPRESSION_MAX_HIERARCHY_REQUESTS {
            return Err(ContextCompressionSummaryError::non_retryable(
                ApiError::internal(format!(
                    "context compression hierarchy exceeded max depth ({LLM_CONTEXT_COMPRESSION_MAX_HIERARCHY_REQUESTS})"
                )),
            ));
        }
        if pending.len() == 1 {
            requests_used = requests_used.saturating_add(1);
            if requests_used > LLM_CONTEXT_COMPRESSION_MAX_HIERARCHY_REQUESTS {
                return Err(ContextCompressionSummaryError::non_retryable(
                    ApiError::internal(format!(
                        "context compression hierarchy exceeded max requests ({LLM_CONTEXT_COMPRESSION_MAX_HIERARCHY_REQUESTS})"
                    )),
                ));
            }
            return llm_context_compression_summary_once(
                context,
                &pending[0],
                &compression_prompt,
                deadline,
            )
            .await;
        }

        // Fail early when chunk summaries + one merge cannot fit the remaining request budget.
        let remaining =
            LLM_CONTEXT_COMPRESSION_MAX_HIERARCHY_REQUESTS.saturating_sub(requests_used);
        if pending.len().saturating_add(1) > remaining {
            return Err(ContextCompressionSummaryError::non_retryable(
                ApiError::internal(format!(
                    "context compression hierarchy needs at least {} requests for {} chunks plus merge, but only {remaining} remain (max {LLM_CONTEXT_COMPRESSION_MAX_HIERARCHY_REQUESTS})",
                    pending.len().saturating_add(1),
                    pending.len()
                )),
            ));
        }

        let mut chunk_summaries = Vec::with_capacity(pending.len());
        for chunk in &pending {
            requests_used = requests_used.saturating_add(1);
            if requests_used > LLM_CONTEXT_COMPRESSION_MAX_HIERARCHY_REQUESTS {
                return Err(ContextCompressionSummaryError::non_retryable(
                    ApiError::internal(format!(
                        "context compression hierarchy exceeded max requests ({LLM_CONTEXT_COMPRESSION_MAX_HIERARCHY_REQUESTS})"
                    )),
                ));
            }
            chunk_summaries.push(
                llm_context_compression_summary_once(context, chunk, &compression_prompt, deadline)
                    .await?
                    .text,
            );
        }

        let merge_messages = build_chunk_merge_checkpoint_messages(&chunk_summaries);
        if checkpoint_messages_estimated_tokens(&merge_messages) <= input_budget {
            requests_used = requests_used.saturating_add(1);
            if requests_used > LLM_CONTEXT_COMPRESSION_MAX_HIERARCHY_REQUESTS {
                return Err(ContextCompressionSummaryError::non_retryable(
                    ApiError::internal(format!(
                        "context compression hierarchy exceeded max requests ({LLM_CONTEXT_COMPRESSION_MAX_HIERARCHY_REQUESTS})"
                    )),
                ));
            }
            return llm_context_compression_summary_once(
                context,
                &merge_messages,
                &compression_prompt,
                deadline,
            )
            .await;
        }
        pending = plan_context_compression_checkpoint_chunks(&merge_messages, input_budget)
            .map_err(ContextCompressionSummaryError::non_retryable)?;
    }
}

/// A completed summary keeps the final audit ID for safe UI and audit correlation.
#[derive(Debug)]
struct ContextCompressionSummary {
    text: String,
    request_id: String,
}

/// A compression request failure keeps the provider classifier intact until the retry policy
/// decides whether another isolated summary request is safe to start.
#[derive(Debug)]
struct ContextCompressionSummaryError {
    message: String,
    request_id: Option<String>,
    retry_class: crate::provider_retry::ProviderRetryClass,
    retry_after: Option<Duration>,
}

impl ContextCompressionSummaryError {
    fn non_retryable(error: ApiError) -> Self {
        Self {
            message: error.message,
            request_id: None,
            retry_class: crate::provider_retry::ProviderRetryClass::NonRetryable,
            retry_after: None,
        }
    }

    fn provider(
        error: &foco_providers::ProviderConfigError,
        retry_after: Option<Duration>,
        request_id: Option<String>,
    ) -> Self {
        Self {
            message: error.user_message(),
            request_id,
            retry_class: crate::provider_retry::classify_provider_retry_class(error),
            retry_after,
        }
    }
}

impl From<ApiError> for ContextCompressionSummaryError {
    fn from(error: ApiError) -> Self {
        Self::non_retryable(error)
    }
}

fn captured_context_compression_request_body(
    context: &PreparedChatContext,
    capture: &ProviderAuditCapture,
    request_id: &str,
) -> String {
    match capture.captured_request_json() {
        Ok(request_body_json) => request_body_json.unwrap_or_default(),
        Err(error) => {
            tracing::warn!(
                workspace_id = %context.workspace_id,
                request_id,
                request_kind = "contextCompression",
                error_category = "llm_audit_request_wire_read_failed",
                error = %error.message,
                "failed to read provider request wire before terminal LLM audit"
            );
            String::new()
        }
    }
}

fn best_effort_context_compression_audit_detail(
    context: &PreparedChatContext,
    request_id: &str,
    result: Result<Option<String>, ApiError>,
) -> Option<String> {
    match result {
        Ok(detail) => detail,
        Err(error) => {
            tracing::warn!(
                workspace_id = %context.workspace_id,
                request_id,
                request_kind = "contextCompression",
                error_category = "llm_audit_detail_capture_failed",
                error = %error.message,
                "failed to capture provider audit detail; finalizing LLM request without it"
            );
            None
        }
    }
}

async fn llm_context_compression_summary_once(
    context: &mut PreparedChatContext,
    checkpoint_messages: &[NeutralChatMessage],
    compression_system_prompt: &str,
    deadline: ContextCompressionAttemptDeadline,
) -> Result<ContextCompressionSummary, ContextCompressionSummaryError> {
    let Some(request_timeout) = deadline.remaining() else {
        return Err(ContextCompressionSummaryError {
            message: "context compression retry budget was exhausted".to_string(),
            request_id: None,
            retry_class: crate::provider_retry::ProviderRetryClass::Network,
            retry_after: None,
        });
    };
    let mut request = build_context_compression_summary_request_with_prompt(
        &context.model_id,
        checkpoint_messages,
        compression_system_prompt,
    );
    let request_id = unique_id("llm");
    context.attach_agent_correlation(&mut request, &request_id);
    let request_started_at = utc_timestamp();
    let started_at = Instant::now();
    let mut events = vec![CapturedAuditEvent {
        event_at: request_started_at.clone(),
        event_type: "start".to_string(),
        normalized_event_json: json!({
            "type": "start",
            "requestKind": "contextCompression",
            "kind": CONTEXT_COMPRESSION_KIND_LLM,
            "chatId": &context.chat_id,
            "userMessageId": &context.user_message_id,
            "assistantMessageId": &context.assistant_message_id,
            "llmRequestId": &request_id,
            "runId": &context.llm_request_id,
            "latencyMode": foco_providers::LatencyMode::Standard,
        })
        .to_string(),
    }];
    persist_running_llm_request_for_kind(
        context,
        &request_id,
        &request_started_at,
        "contextCompression",
        None,
        &events,
    )?;
    let capture = ProviderAuditCapture::new(
        &context.workspace_path,
        request_id.clone(),
        api_audit_save_details(&context.global_config),
    );
    let observer = capture.observer();
    let capture_details = observer.is_some();
    let mut stream = match timeout(
        request_timeout,
        stream_chat_with_capture_observer(
            &context.provider_config,
            request,
            capture_details,
            observer,
            None,
        ),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        Ok(Err(source)) => {
            if let Err(error) = capture.persist_request_failure(&source) {
                tracing::warn!(
                    workspace_id = %context.workspace_id,
                    request_id = %request_id,
                    request_kind = "contextCompression",
                    error_category = "llm_audit_request_wire_write_failed",
                    error = %error.message,
                    "failed to persist provider request wire before terminal LLM audit"
                );
            }
            let failure = ContextCompressionSummaryError::provider(
                &source.error,
                None,
                Some(request_id.clone()),
            );
            let message = failure.message.clone();
            let request_body_json =
                captured_context_compression_request_body(context, &capture, &request_id);
            let response_body_json = best_effort_context_compression_audit_detail(
                context,
                &request_id,
                capture.failed_response_json(message.clone(), source.status_code(), false),
            );
            context.record_finished_llm_request(CapturedLlmRequest {
                id: request_id.clone(),
                request_kind: "contextCompression",
                request_started_at,
                request_body_json,
                events,
                outcome: ChatAuditOutcome {
                    response_body_json,
                    ..failed_provider_audit_outcome(
                        started_at,
                        &message,
                        source.status_code().map(i64::from),
                    )
                },
                terminal_persisted: false,
            });
            return Err(failure);
        }
        Err(_) => {
            let message =
                format!("context compression summary timed out after {LLM_REQUEST_TIMEOUT_MS} ms");
            let request_body_json =
                captured_context_compression_request_body(context, &capture, &request_id);
            let response_body_json = best_effort_context_compression_audit_detail(
                context,
                &request_id,
                capture.failed_response_json(message.clone(), None, false),
            );
            context.record_finished_llm_request(CapturedLlmRequest {
                id: request_id.clone(),
                request_kind: "contextCompression",
                request_started_at,
                request_body_json,
                events,
                outcome: ChatAuditOutcome {
                    response_body_json,
                    ..failed_provider_audit_outcome(started_at, &message, None)
                },
                terminal_persisted: false,
            });
            return Err(ContextCompressionSummaryError {
                message,
                request_id: Some(request_id.clone()),
                retry_class: crate::provider_retry::ProviderRetryClass::Network,
                retry_after: None,
            });
        }
    };
    let mut output_text = String::new();
    let mut final_usage = None;
    let mut first_token_at = None;
    let mut first_token_latency_ms = None;

    loop {
        let event_result = match timeout(
            deadline.remaining().unwrap_or_default(),
            stream.next_event(),
        )
        .await
        {
            Ok(event_result) => event_result,
            Err(_) => {
                let message = format!(
                    "context compression summary timed out after {LLM_REQUEST_TIMEOUT_MS} ms"
                );
                let audit_status_code = stream.http_status().map(i64::from);
                let request_body_json =
                    captured_context_compression_request_body(context, &capture, &request_id);
                let response_body_json = best_effort_context_compression_audit_detail(
                    context,
                    &request_id,
                    capture.failed_stream_response_json(
                        &stream,
                        message.clone(),
                        stream.http_status(),
                        true,
                    ),
                );
                context.record_finished_llm_request(CapturedLlmRequest {
                    id: request_id.clone(),
                    request_kind: "contextCompression",
                    request_started_at,
                    request_body_json,
                    events,
                    outcome: ChatAuditOutcome {
                        response_body_json,
                        ..failed_provider_audit_outcome(started_at, &message, audit_status_code)
                    },
                    terminal_persisted: false,
                });
                return Err(ContextCompressionSummaryError {
                    message,
                    request_id: Some(request_id.clone()),
                    retry_class: crate::provider_retry::ProviderRetryClass::Network,
                    retry_after: None,
                });
            }
        };
        let Some(event_result) = event_result else {
            let message = "context compression summary stream ended without a completion event";
            let audit_status_code = stream.http_status().map(i64::from);
            let request_body_json =
                captured_context_compression_request_body(context, &capture, &request_id);
            let response_body_json = best_effort_context_compression_audit_detail(
                context,
                &request_id,
                capture.failed_stream_response_json(&stream, message, stream.http_status(), true),
            );
            context.record_finished_llm_request(CapturedLlmRequest {
                id: request_id.clone(),
                request_kind: "contextCompression",
                request_started_at,
                request_body_json,
                events,
                outcome: ChatAuditOutcome {
                    response_body_json,
                    ..failed_provider_audit_outcome(started_at, message, audit_status_code)
                },
                terminal_persisted: false,
            });
            return Err(ContextCompressionSummaryError {
                message: message.to_string(),
                request_id: Some(request_id.clone()),
                retry_class: crate::provider_retry::ProviderRetryClass::Network,
                retry_after: stream.retry_after(),
            });
        };
        let event = match event_result {
            Ok(event) => event,
            Err(source) => {
                let failure = ContextCompressionSummaryError::provider(
                    &source,
                    stream.retry_after(),
                    Some(request_id.clone()),
                );
                let message = failure.message.clone();
                let request_body_json =
                    captured_context_compression_request_body(context, &capture, &request_id);
                let response_body_json = best_effort_context_compression_audit_detail(
                    context,
                    &request_id,
                    capture.response_json(stream.final_response_dump()),
                )
                .or_else(|| {
                    best_effort_context_compression_audit_detail(
                        context,
                        &request_id,
                        capture.failed_response_json(message.clone(), source.status_code(), true),
                    )
                });
                context.record_finished_llm_request(CapturedLlmRequest {
                    id: request_id.clone(),
                    request_kind: "contextCompression",
                    request_started_at,
                    request_body_json,
                    events,
                    outcome: ChatAuditOutcome {
                        response_body_json,
                        ..failed_provider_audit_outcome(
                            started_at,
                            &message,
                            source.status_code().map(i64::from),
                        )
                    },
                    terminal_persisted: false,
                });
                return Err(failure);
            }
        };
        events.push(captured_provider_event(&event));

        match event {
            NeutralChatStreamEvent::Start => {}
            NeutralChatStreamEvent::TextDelta { delta } => {
                capture_first_token(started_at, &mut first_token_at, &mut first_token_latency_ms);
                output_text.push_str(&delta);
            }
            NeutralChatStreamEvent::ReasoningDelta { .. }
            | NeutralChatStreamEvent::ThoughtSignatureDelta { .. } => {
                capture_first_token(started_at, &mut first_token_at, &mut first_token_latency_ms);
            }
            NeutralChatStreamEvent::Usage { usage } => {
                final_usage = Some(usage);
            }
            NeutralChatStreamEvent::ToolCall { tool_call } => {
                let message = format!(
                    "context compression summary called unsupported tool '{}'",
                    tool_call.name
                );
                let request_body_json =
                    captured_context_compression_request_body(context, &capture, &request_id);
                let response_body_json = best_effort_context_compression_audit_detail(
                    context,
                    &request_id,
                    capture.failed_response_json(message.clone(), None, true),
                );
                context.record_finished_llm_request(CapturedLlmRequest {
                    id: request_id.clone(),
                    request_kind: "contextCompression",
                    request_started_at,
                    request_body_json,
                    events,
                    outcome: ChatAuditOutcome {
                        response_body_json,
                        ..failed_provider_audit_outcome(started_at, &message, None)
                    },
                    terminal_persisted: false,
                });
                return Err(ContextCompressionSummaryError::non_retryable(
                    ApiError::internal(message),
                ));
            }
            NeutralChatStreamEvent::Complete { text, usage, .. } => {
                if !text.trim().is_empty() {
                    output_text.push_str(&text);
                }
                if let Some(usage) = usage {
                    final_usage = Some(usage);
                }
                break;
            }
            NeutralChatStreamEvent::Error { message } => {
                let message = format!("context compression summary stream error: {message}");
                let request_body_json =
                    captured_context_compression_request_body(context, &capture, &request_id);
                let response_body_json = best_effort_context_compression_audit_detail(
                    context,
                    &request_id,
                    capture.response_json(stream.final_response_dump()),
                )
                .or_else(|| {
                    best_effort_context_compression_audit_detail(
                        context,
                        &request_id,
                        capture.failed_response_json(message.clone(), None, true),
                    )
                });
                context.record_finished_llm_request(CapturedLlmRequest {
                    id: request_id.clone(),
                    request_kind: "contextCompression",
                    request_started_at,
                    request_body_json,
                    events,
                    outcome: ChatAuditOutcome {
                        response_body_json,
                        ..failed_provider_audit_outcome(started_at, &message, None)
                    },
                    terminal_persisted: false,
                });
                return Err(ContextCompressionSummaryError::non_retryable(
                    ApiError::internal(message),
                ));
            }
        }
    }

    let summary = output_text.trim().to_string();
    if summary.is_empty() {
        let message = "context compression summary returned empty text";
        let request_body_json =
            captured_context_compression_request_body(context, &capture, &request_id);
        let response_body_json = best_effort_context_compression_audit_detail(
            context,
            &request_id,
            capture.failed_response_json(message, None, false),
        );
        context.record_finished_llm_request(CapturedLlmRequest {
            id: request_id.clone(),
            request_kind: "contextCompression",
            request_started_at,
            request_body_json,
            events,
            outcome: ChatAuditOutcome {
                response_body_json,
                ..failed_provider_audit_outcome(started_at, message, None)
            },
            terminal_persisted: false,
        });
        return Err(ContextCompressionSummaryError::non_retryable(
            ApiError::internal(message),
        ));
    }
    let request_body_json =
        captured_context_compression_request_body(context, &capture, &request_id);
    let response_body_json = best_effort_context_compression_audit_detail(
        context,
        &request_id,
        capture.response_json(stream.final_response_dump()),
    );
    context.record_finished_llm_request(CapturedLlmRequest {
        id: request_id.clone(),
        request_kind: "contextCompression",
        request_started_at,
        request_body_json,
        events,
        outcome: ChatAuditOutcome {
            first_token_at,
            completed_at: utc_timestamp(),
            first_token_latency_ms,
            total_latency_ms: elapsed_millis(started_at),
            input_tokens: final_usage.as_ref().and_then(|usage| usage.input_tokens),
            output_tokens: final_usage.as_ref().and_then(|usage| usage.output_tokens),
            cache_read_tokens: final_usage
                .as_ref()
                .and_then(|usage| usage.cache_read_tokens),
            cache_write_tokens: final_usage
                .as_ref()
                .and_then(|usage| usage.cache_write_tokens),
            reasoning_tokens: final_usage
                .as_ref()
                .and_then(|usage| usage.reasoning_tokens),
            status_code: Some(200),
            final_state: "succeeded",
            response_body_json,
        },
        terminal_persisted: false,
    });

    Ok(ContextCompressionSummary {
        text: summary,
        request_id,
    })
}

/// Prepared snapshot write + in-memory replacement. Callers insert into their
/// WorkspaceDatabase first, then apply `replaced` only on success.
#[derive(Clone, Debug)]
pub(crate) struct PreparedContextCompressionSnapshot {
    pub snapshot: ContextCompressionSnapshotRecord,
    pub replaced: ReplacedPromptMessages,
    pub summary: String,
}

/// Build snapshot record + replacement arrays without opening a database.
/// Fills `metadata.contextUsage` from the post-replacement prompt layout.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_context_compression_snapshot(
    chat_id: &str,
    run_id: &str,
    messages: &[NeutralChatMessage],
    message_source_sequences: &[Option<i64>],
    message_context_sources: &[PromptContextSource],
    active_tool_start_index: usize,
    compression_snapshots: &[ContextCompressionSnapshotRecord],
    context_budget: &foco_agent::ContextBudget,
    covered_indices: &[usize],
    summary: String,
    original_tokens: u64,
    summary_token_count: u64,
    mut metadata: Value,
) -> Result<PreparedContextCompressionSnapshot, ApiError> {
    let snapshot_id = unique_id("ctx");
    let snapshot_sequence = next_context_snapshot_sequence(compression_snapshots)?;
    let mut snapshot = build_context_compression_snapshot_record(
        snapshot_id,
        chat_id.to_string(),
        run_id.to_string(),
        snapshot_sequence,
        summary.clone(),
        message_source_sequences,
        covered_indices,
        original_tokens,
        summary_token_count,
        utc_timestamp(),
    )?;
    let replaced = apply_compression_snapshot_to_messages(
        messages,
        message_source_sequences,
        message_context_sources,
        active_tool_start_index,
        covered_indices,
        compression_snapshot_message(&snapshot),
    );
    metadata["contextUsage"] = post_compression_context_usage_metadata_from_budget(
        context_budget,
        &replaced.messages,
        &replaced.message_source_sequences,
        &replaced.message_context_sources,
        replaced.active_tool_start_index,
    )?;
    snapshot.metadata_json = metadata.to_string();
    Ok(PreparedContextCompressionSnapshot {
        snapshot,
        replaced,
        summary,
    })
}

/// Insert a prepared snapshot into an already-open workspace database.
pub(crate) fn insert_context_compression_snapshot_record(
    database: &mut WorkspaceDatabase,
    prepared: &PreparedContextCompressionSnapshot,
) -> Result<(), ApiError> {
    database
        .insert_context_compression_snapshot(NewContextCompressionSnapshot {
            id: &prepared.snapshot.id,
            chat_id: &prepared.snapshot.chat_id,
            run_id: &prepared.snapshot.run_id,
            sequence: prepared.snapshot.sequence,
            summary: &prepared.summary,
            source_message_start_sequence: prepared.snapshot.source_message_start_sequence,
            source_message_end_sequence: prepared.snapshot.source_message_end_sequence,
            original_token_count: prepared.snapshot.original_token_count,
            summary_token_count: prepared.snapshot.summary_token_count,
            metadata_json: Some(&prepared.snapshot.metadata_json),
        })
        .map_err(ApiError::from_workspace_error)
}

fn persist_context_compression_snapshot(
    context: &mut PreparedChatContext,
    covered_indices: &[usize],
    summary: String,
    original_tokens: u64,
    summary_token_count: u64,
    kind: &str,
    metadata: Value,
) -> Result<ContextCompressionSnapshotRecord, ApiError> {
    let prepared = prepare_context_compression_snapshot(
        &context.chat_id,
        &context.llm_request_id,
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
        context.active_tool_start_index,
        &context.compression_snapshots,
        &context.context_budget,
        covered_indices,
        summary,
        original_tokens,
        summary_token_count,
        metadata,
    )?;

    let mut database = WorkspaceDatabase::open_or_create(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    insert_context_compression_snapshot_record(&mut database, &prepared)?;

    context.provider_request.messages = prepared.replaced.messages;
    context.message_source_sequences = prepared.replaced.message_source_sequences;
    context.message_context_sources = prepared.replaced.message_context_sources;
    context.active_tool_start_index = prepared.replaced.active_tool_start_index;
    context
        .compression_snapshots
        .push(prepared.snapshot.clone());

    tracing::debug!(kind = kind, "created context compression snapshot");
    Ok(prepared.snapshot)
}

fn compression_source_sequence_range(
    message_source_sequences: &[Option<i64>],
    covered_indices: &[usize],
) -> (i64, i64) {
    let sequences = direct_covered_sequences(message_source_sequences, covered_indices);
    let start = sequences.first().copied().unwrap_or(0);
    let end = sequences.last().copied().unwrap_or(start);
    (start, end)
}

pub(crate) fn compress_runtime_tool_state_if_needed(
    context: &mut PreparedChatContext,
    force: bool,
) -> Result<bool, ApiError> {
    compress_runtime_tool_state_messages_if_needed(
        &mut context.provider_request.messages,
        &mut context.message_source_sequences,
        &mut context.message_context_sources,
        &mut context.active_tool_start_index,
        &mut context.runtime_tool_state_compression_count,
        &context.context_budget,
        context
            .global_config
            .app
            .runtime_tool_state_compression_enabled,
        force,
    )
}

/// Compress older in-run tool batches into a local `RuntimeToolStateSnapshot`.
///
/// `compression_enabled` is a hard gate: when false, both proactive (80%) and required-overflow
/// `force=true` paths no-op. `force` only skips the usage-threshold check when the switch is on.
/// Round-cap recovery uses `compress_all_runtime_tool_state_messages` and is not gated here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compress_runtime_tool_state_messages_if_needed(
    messages: &mut Vec<NeutralChatMessage>,
    message_source_sequences: &mut Vec<Option<i64>>,
    message_context_sources: &mut Vec<PromptContextSource>,
    active_tool_start_index: &mut usize,
    runtime_tool_state_compression_count: &mut usize,
    context_budget: &foco_agent::ContextBudget,
    compression_enabled: bool,
    force: bool,
) -> Result<bool, ApiError> {
    if !compression_enabled {
        return Ok(false);
    }

    validate_prompt_context_lengths(messages, message_source_sequences, message_context_sources)?;

    if *runtime_tool_state_compression_count >= CONTEXT_COMPRESSION_MAX_RUNTIME_TOOL_STATE_SNAPSHOTS
    {
        return Ok(false);
    }

    let message_groups = context_message_groups(
        messages,
        message_source_sequences,
        message_context_sources,
        *active_tool_start_index,
    )?;
    let runtime_tool_groups = message_groups
        .iter()
        .enumerate()
        .filter_map(|(group_index, group)| {
            group
                .runtime_tool_batch_index
                .map(|batch_index| (group_index, batch_index, group.estimated_tokens))
        })
        .collect::<Vec<_>>();

    if runtime_tool_groups.len() <= CONTEXT_COMPRESSION_PRESERVE_RECENT_TOOL_BATCHES {
        return Ok(false);
    }

    let segments = context_usage_segments(context_budget, &message_groups);
    let total_used_context_tokens = context_usage_segments_total(&segments);
    let breakdown = context_token_breakdown(&message_groups);
    let should_compress = force
        || total_used_context_tokens
            >= context_window_compression_trigger_tokens(context_budget.context_window)
        || breakdown.required_tokens > context_budget.available_message_tokens;
    if !should_compress {
        return Ok(false);
    }

    let covered_tool_group_count =
        runtime_tool_groups.len() - CONTEXT_COMPRESSION_PRESERVE_RECENT_TOOL_BATCHES;
    let covered_group_indices = runtime_tool_groups
        .iter()
        .take(covered_tool_group_count)
        .map(|(group_index, _, _)| *group_index)
        .collect::<Vec<_>>();
    if covered_group_indices.is_empty() {
        return Ok(false);
    }
    let covered_message_indices = message_group_indices(&message_groups, &covered_group_indices)?;
    let original_tokens = covered_message_indices
        .iter()
        .map(|index| neutral_message_estimated_tokens(&messages[*index]))
        .sum::<u64>();
    if original_tokens == 0 {
        return Ok(false);
    }

    let summary = runtime_tool_state_summary(messages, &covered_message_indices, true)?;
    let summary_tokens = estimate_text_tokens(&summary);
    if summary_tokens >= original_tokens {
        return Ok(false);
    }

    let snapshot = neutral_text_message(NeutralChatRole::User, summary);
    *messages =
        replace_covered_messages_with_snapshot(messages, &covered_message_indices, snapshot);
    *message_source_sequences =
        replace_covered_sequences_with_snapshot(message_source_sequences, &covered_message_indices);
    *message_context_sources = replace_covered_sources_with_snapshot(
        message_context_sources,
        &covered_message_indices,
        PromptContextSource::RuntimeToolStateSnapshot,
    );
    *active_tool_start_index =
        compressed_active_tool_start_index(*active_tool_start_index, &covered_message_indices);
    *runtime_tool_state_compression_count += 1;

    Ok(true)
}

pub(crate) fn compress_all_runtime_tool_state(
    context: &mut PreparedChatContext,
) -> Result<bool, ApiError> {
    compress_all_runtime_tool_state_messages(
        &mut context.provider_request.messages,
        &mut context.message_source_sequences,
        &mut context.message_context_sources,
        &mut context.active_tool_start_index,
    )
}

pub(crate) fn compress_all_runtime_tool_state_messages(
    messages: &mut Vec<NeutralChatMessage>,
    message_source_sequences: &mut Vec<Option<i64>>,
    message_context_sources: &mut Vec<PromptContextSource>,
    active_tool_start_index: &mut usize,
) -> Result<bool, ApiError> {
    validate_prompt_context_lengths(messages, message_source_sequences, message_context_sources)?;

    if !message_context_sources
        .iter()
        .any(|source| matches!(source, PromptContextSource::RuntimeToolState { .. }))
    {
        return Ok(false);
    }

    let covered_message_indices = message_context_sources
        .iter()
        .enumerate()
        .filter_map(|(index, source)| {
            matches!(
                source,
                PromptContextSource::RuntimeToolState { .. }
                    | PromptContextSource::RuntimeToolStateSnapshot
            )
            .then_some(index)
        })
        .collect::<Vec<_>>();

    let summary = runtime_tool_state_summary(messages, &covered_message_indices, false)?;
    let snapshot = neutral_text_message(NeutralChatRole::User, summary);
    *messages =
        replace_covered_messages_with_snapshot(messages, &covered_message_indices, snapshot);
    *message_source_sequences =
        replace_covered_sequences_with_snapshot(message_source_sequences, &covered_message_indices);
    *message_context_sources = replace_covered_sources_with_snapshot(
        message_context_sources,
        &covered_message_indices,
        PromptContextSource::RuntimeToolStateSnapshot,
    );
    *active_tool_start_index =
        compressed_active_tool_start_index(*active_tool_start_index, &covered_message_indices);

    Ok(true)
}

pub(crate) fn recover_after_tool_round_cap(
    context: &mut PreparedChatContext,
    tool_calls: Vec<NeutralToolCall>,
    assistant_text: String,
    assistant_reasoning: Option<String>,
) -> Result<bool, ApiError> {
    append_pending_tool_state_messages(
        &mut context.provider_request.messages,
        &mut context.message_source_sequences,
        &mut context.message_context_sources,
        &mut context.next_runtime_tool_batch_index,
        tool_calls,
        assistant_text,
        assistant_reasoning,
    );
    compress_all_runtime_tool_state(context)
}

fn runtime_tool_state_summary(
    messages: &[NeutralChatMessage],
    covered_indices: &[usize],
    preserve_recent_tool_calls: bool,
) -> Result<String, ApiError> {
    let mut lines = if preserve_recent_tool_calls {
        vec![
            "Runtime tool-state compression snapshot: older completed tool calls/results from this same in-progress run were removed from the live prompt.".to_string(),
            "Recent tool calls remain verbatim below this snapshot.".to_string(),
        ]
    } else {
        vec![
            "Runtime tool-state compression snapshot: all prior in-progress tool calls/results from this run were removed from the live prompt.".to_string(),
            "Continue from the summarized tool evidence below without replaying the removed tool-call protocol messages.".to_string(),
        ]
    };
    let mut tool_call_count = 0usize;
    let mut tool_result_count = 0usize;

    for index in covered_indices.iter().copied() {
        let message = messages.get(index).ok_or_else(|| {
            ApiError::internal("runtime tool compression covered message index is out of bounds")
        })?;
        for tool_call in &message.tool_calls {
            tool_call_count += 1;
            lines.push(format!(
                "- tool call {}: {} input {}",
                tool_call.call_id,
                tool_call.name,
                compact_json_for_runtime_tool_summary(&tool_call.arguments)
            ));
        }
        if message.role == NeutralChatRole::Tool {
            tool_result_count += 1;
            let tool_name = message.tool_name.as_deref().unwrap_or("unknown_tool");
            let call_id = message.tool_call_id.as_deref().unwrap_or("unknown_call");
            lines.push(format!(
                "- tool result {call_id}: {tool_name} output {}",
                compact_tool_output_for_runtime_summary(tool_name, &message.content)
            ));
        } else if !message.content.trim().is_empty() && message.tool_calls.is_empty() {
            lines.push(format!(
                "- prior runtime note: {}",
                truncate_for_context_snapshot(&message.content)
            ));
        }
    }

    lines.insert(
        2,
        format!("- compressed tool calls: {tool_call_count}; tool results: {tool_result_count}"),
    );

    Ok(lines.join("\n"))
}

fn compact_json_for_runtime_tool_summary(value: &Value) -> String {
    if let Value::Object(map) = value {
        let mut compact = serde_json::Map::new();
        for key in [
            "path",
            "startLine",
            "endLine",
            "command",
            "args",
            "background",
            "backgroundTimeoutMs",
            "processId",
            "cursor",
            "nextCursor",
            "query",
            "symbol",
            "symbolId",
            "scope",
            "taskId",
            "status",
            "timeoutMs",
        ] {
            if let Some(value) = map.get(key) {
                compact.insert(key.to_string(), compact_large_json_value(value));
            }
        }
        if let Some(content) = map.get("content").and_then(Value::as_str) {
            compact.insert(
                "contentSummary".to_string(),
                json!({
                    "chars": content.chars().count(),
                    "preview": truncate_for_context_snapshot(content),
                }),
            );
        }
        if !compact.is_empty() {
            return Value::Object(compact).to_string();
        }
    }

    truncate_for_context_snapshot(&value.to_string())
}

fn compact_large_json_value(value: &Value) -> Value {
    match value {
        Value::String(text) if text.chars().count() > CONTEXT_COMPRESSION_MAX_MESSAGE_CHARS => {
            json!({
                "chars": text.chars().count(),
                "preview": truncate_for_context_snapshot(text),
            })
        }
        Value::Array(values) if values.len() > 12 => json!({
            "items": values.len(),
            "preview": values.iter().take(12).cloned().collect::<Vec<_>>(),
        }),
        other => other.clone(),
    }
}

fn compact_tool_output_for_runtime_summary(tool_name: &str, content: &str) -> String {
    match serde_json::from_str::<Value>(content) {
        Ok(Value::Object(map)) => {
            if let Some(compact) = compact_managed_command_output(tool_name, &map) {
                return compact.to_string();
            }

            let mut compact = serde_json::Map::new();
            for key in [
                "path",
                "bytes",
                "truncated",
                "exitCode",
                "status",
                "timeoutMs",
                "exists",
            ] {
                if let Some(value) = map.get(key) {
                    compact.insert(key.to_string(), compact_large_json_value(value));
                }
            }
            if let Some(output_content) = map.get("content").and_then(Value::as_str) {
                compact.insert(
                    "contentSummary".to_string(),
                    json!({
                        "chars": output_content.chars().count(),
                        "preview": truncate_for_context_snapshot(output_content),
                    }),
                );
            }
            for key in ["stdout", "stderr", "text", "summary"] {
                if let Some(value) = map.get(key) {
                    compact.insert(key.to_string(), compact_large_json_value(value));
                }
            }
            if !compact.is_empty() {
                Value::Object(compact).to_string()
            } else {
                format!(
                    "{} result {}",
                    tool_name,
                    truncate_for_context_snapshot(content)
                )
            }
        }
        _ => truncate_for_context_snapshot(content),
    }
}

/// Keep managed-command handles and pagination state usable after runtime-tool compression
/// without replaying the command's historical log buffer into later model turns.
fn compact_managed_command_output(
    tool_name: &str,
    map: &serde_json::Map<String, Value>,
) -> Option<Value> {
    let is_managed_command =
        matches!(tool_name, "get_command_output" | "stop_command") || map.contains_key("processId");
    if !is_managed_command {
        return None;
    }

    let mut compact = serde_json::Map::new();
    for key in [
        "processId",
        "pid",
        "status",
        "startedAt",
        "endedAt",
        "exitCode",
        "success",
        "terminationReason",
        "fromCursor",
        "availableFromCursor",
        "nextCursor",
        "cursorExpired",
        "hasMore",
        "outputTruncated",
        "retainedOutputBytes",
    ] {
        if let Some(value) = map.get(key) {
            compact.insert(key.to_string(), compact_large_json_value(value));
        }
    }

    if let Some(chunks) = map.get("chunks").and_then(Value::as_array) {
        let streams = chunks
            .iter()
            .filter_map(|chunk| chunk.get("stream").and_then(Value::as_str))
            .collect::<Vec<_>>();
        let last_cursor = chunks.last().and_then(|chunk| chunk.get("cursor")).cloned();
        compact.insert(
            "chunkSummary".to_string(),
            json!({
                "count": chunks.len(),
                "streams": streams,
                "lastCursor": last_cursor,
            }),
        );
    }

    (!compact.is_empty()).then_some(Value::Object(compact))
}

pub(crate) fn active_compression_snapshots(
    snapshots: &[ContextCompressionSnapshotRecord],
) -> Vec<ContextCompressionSnapshotRecord> {
    let superseded_ids = snapshots
        .iter()
        .flat_map(snapshot_superseded_snapshot_ids)
        .collect::<HashSet<_>>();

    snapshots
        .iter()
        .filter(|snapshot| !superseded_ids.contains(&snapshot.id))
        .cloned()
        .collect()
}

pub(crate) fn snapshot_covered_sequences(
    snapshots: &[ContextCompressionSnapshotRecord],
) -> HashSet<i64> {
    let mut sequences = HashSet::new();

    for snapshot in snapshots {
        sequences.extend(snapshot_covered_sequence_vec(snapshot));
    }

    sequences
}

fn snapshot_covered_sequence_vec(snapshot: &ContextCompressionSnapshotRecord) -> Vec<i64> {
    if let Ok(metadata) = serde_json::from_str::<Value>(&snapshot.metadata_json) {
        if let Some(covered_sequences) = metadata.get("coveredSequences").and_then(Value::as_array)
        {
            return covered_sequences.iter().filter_map(Value::as_i64).collect();
        }
    }

    (snapshot.source_message_start_sequence..=snapshot.source_message_end_sequence).collect()
}

fn snapshot_superseded_snapshot_ids(snapshot: &ContextCompressionSnapshotRecord) -> Vec<String> {
    serde_json::from_str::<Value>(&snapshot.metadata_json)
        .ok()
        .into_iter()
        .flat_map(|metadata| {
            ["supersededSnapshotIds", "coveredSnapshotIds"]
                .into_iter()
                .filter_map(move |key| metadata.get(key).and_then(Value::as_array).cloned())
        })
        .flat_map(|ids| {
            ids.into_iter()
                .filter_map(|id| id.as_str().map(str::to_string))
        })
        .collect()
}

pub(crate) fn compression_snapshot_message(
    snapshot: &ContextCompressionSnapshotRecord,
) -> NeutralChatMessage {
    // Checkpoint summary is a plain User message; no Snapshot ID / token metadata / headings.
    neutral_text_message(NeutralChatRole::User, snapshot.summary.clone())
}

/// Active LLM checkpoint snapshot IDs (excludes superseded and non-LLM kinds).
pub(crate) fn active_llm_checkpoint_snapshot_ids(
    snapshots: &[ContextCompressionSnapshotRecord],
) -> HashSet<String> {
    active_compression_snapshots(snapshots)
        .into_iter()
        .filter(|snapshot| snapshot_metadata_kind(snapshot) == CONTEXT_COMPRESSION_KIND_LLM)
        .map(|snapshot| snapshot.id)
        .collect()
}

fn snapshot_metadata_kind(snapshot: &ContextCompressionSnapshotRecord) -> &'static str {
    serde_json::from_str::<Value>(&snapshot.metadata_json)
        .ok()
        .and_then(|metadata| {
            metadata
                .get("kind")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|kind| kind == CONTEXT_COMPRESSION_KIND_LLM)
        .map(|_| CONTEXT_COMPRESSION_KIND_LLM)
        .unwrap_or(CONTEXT_COMPRESSION_KIND_RULE)
}

/// Index after the last successful active LLM ContextCompression part within assistant parts.
/// Returns 0 when no active LLM checkpoint cutpoint exists (full replay).
pub(crate) fn assistant_parts_checkpoint_replay_start_index(
    parts: &[StoredChatMessagePart],
    active_llm_snapshot_ids: &HashSet<String>,
) -> usize {
    if active_llm_snapshot_ids.is_empty() {
        return 0;
    }
    parts
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, part)| match part {
            StoredChatMessagePart::ContextCompression {
                status,
                kind,
                detail,
                ..
            } if status == "completed"
                && kind == CONTEXT_COMPRESSION_KIND_LLM
                && detail
                    .snapshot_id
                    .as_ref()
                    .is_some_and(|id| active_llm_snapshot_ids.contains(id)) =>
            {
                Some(index.saturating_add(1))
            }
            _ => None,
        })
        .unwrap_or(0)
}

fn is_llm_checkpoint_source_bucket(source: PromptContextSourceBucket) -> bool {
    matches!(
        source,
        PromptContextSourceBucket::PersistedHistory
            | PromptContextSourceBucket::TurnMemory
            | PromptContextSourceBucket::CompressionSnapshot
            | PromptContextSourceBucket::AssistantDraft
            | PromptContextSourceBucket::CurrentUser
            | PromptContextSourceBucket::RuntimeAssistant
            | PromptContextSourceBucket::RuntimeToolState
            | PromptContextSourceBucket::RuntimeToolStateSnapshot
    )
}

/// Drop trailing incomplete tool-call/result pairs so the checkpoint request stays provider-valid.
fn trim_covered_indices_to_complete_tool_pairs(
    messages: &[NeutralChatMessage],
    covered_indices: Vec<usize>,
) -> Vec<usize> {
    if covered_indices.is_empty() {
        return covered_indices;
    }

    let covered_set = covered_indices.iter().copied().collect::<HashSet<_>>();
    let result_ids = covered_indices
        .iter()
        .filter_map(|index| {
            let message = messages.get(*index)?;
            if message.role == NeutralChatRole::Tool {
                message.tool_call_id.clone()
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();

    let mut incomplete_from: Option<usize> = None;
    for index in &covered_indices {
        let Some(message) = messages.get(*index) else {
            continue;
        };
        if message.tool_calls.is_empty() {
            continue;
        }
        let all_paired = message.tool_calls.iter().all(|tool_call| {
            result_ids.contains(&tool_call.call_id)
                || messages.iter().enumerate().any(|(other_index, other)| {
                    covered_set.contains(&other_index)
                        && other.role == NeutralChatRole::Tool
                        && other.tool_call_id.as_deref() == Some(tool_call.call_id.as_str())
                })
        });
        if !all_paired {
            incomplete_from = Some(*index);
            break;
        }
    }

    let Some(cut) = incomplete_from else {
        return covered_indices;
    };
    covered_indices
        .into_iter()
        .filter(|index| *index < cut)
        .collect()
}

fn build_checkpoint_messages(
    messages: &[NeutralChatMessage],
    message_context_sources: &[PromptContextSource],
    compression_snapshots: &[ContextCompressionSnapshotRecord],
    covered_indices: &[usize],
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    let mut checkpoint_messages = Vec::with_capacity(covered_indices.len());
    for index in covered_indices.iter().copied() {
        let message = messages.get(index).ok_or_else(|| {
            ApiError::internal("context compression covered message index is out of bounds")
        })?;
        let source = message_context_sources.get(index).ok_or_else(|| {
            ApiError::internal("context compression covered source index is out of bounds")
        })?;
        if matches!(source, PromptContextSource::CompressionSnapshot) {
            // Prior checkpoints join the new request as ordinary User summary content only.
            let summary = compression_snapshot_id_from_message(message, compression_snapshots)
                .and_then(|id| {
                    compression_snapshots
                        .iter()
                        .find(|snapshot| snapshot.id == id)
                        .map(|snapshot| snapshot.summary.clone())
                })
                .unwrap_or_else(|| message.content.clone());
            checkpoint_messages.push(neutral_text_message(NeutralChatRole::User, summary));
            continue;
        }
        checkpoint_messages.push(message.clone());
    }
    Ok(checkpoint_messages)
}

#[allow(dead_code)]
fn compact_message_for_compression(message: &NeutralChatMessage) -> String {
    let mut content = truncate_for_context_snapshot(&message.content);

    if let Some(reasoning) = message.reasoning.as_deref() {
        let reasoning = truncate_for_context_snapshot(reasoning);
        if content.is_empty() {
            content = format!("reasoning: {reasoning}");
        } else {
            content.push_str("; reasoning: ");
            content.push_str(&reasoning);
        }
    }

    if !message.attachments.is_empty() {
        let names = message
            .attachments
            .iter()
            .map(|attachment| attachment.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if content.is_empty() {
            content = format!("attachments: {names}");
        } else {
            content.push_str("; attachments: ");
            content.push_str(&names);
        }
    }

    if !message.tool_calls.is_empty() {
        let names = message
            .tool_calls
            .iter()
            .map(|tool_call| tool_call.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if content.is_empty() {
            content = format!("tool calls: {names}");
        } else {
            content.push_str("; tool calls: ");
            content.push_str(&names);
        }
    }

    if let Some(tool_name) = message.tool_name.as_deref() {
        if content.is_empty() {
            content = format!("tool result for {tool_name}");
        } else {
            content.push_str("; tool result for ");
            content.push_str(tool_name);
        }
    }

    if content.is_empty() {
        "(empty message content)".to_string()
    } else {
        content
    }
}

fn truncate_for_context_snapshot(value: &str) -> String {
    let trimmed = value.trim();
    let mut output = String::new();

    for (index, character) in trimmed.chars().enumerate() {
        if index >= CONTEXT_COMPRESSION_MAX_MESSAGE_CHARS {
            output.push_str("...");
            return output;
        }

        if character.is_control() && character != '\n' && character != '\t' {
            output.push(' ');
        } else {
            output.push(character);
        }
    }

    output
}

#[allow(dead_code)]
fn context_compression_summary_allowing_snapshots(
    messages: &[NeutralChatMessage],
    message_source_sequences: &[Option<i64>],
    covered_indices: &[usize],
) -> Result<String, ApiError> {
    if messages.len() != message_source_sequences.len() {
        return Err(ApiError::internal(
            "context message source sequence count does not match prompt message count",
        ));
    }

    let mut lines = vec![
        "Structured summary of earlier chat context that will be replaced by a model-generated continuation summary."
            .to_string(),
    ];

    for index in covered_indices.iter().copied() {
        let message = messages.get(index).ok_or_else(|| {
            ApiError::internal("context compression covered message index is out of bounds")
        })?;
        let sequence_label = message_source_sequences
            .get(index)
            .and_then(|sequence| *sequence)
            .map(|sequence| sequence.to_string())
            .unwrap_or_else(|| "snapshot".to_string());

        lines.push(format!(
            "- source {sequence_label}, role {}: {}",
            neutral_role_label(&message.role),
            compact_message_for_compression(message)
        ));
    }

    Ok(lines.join("\n"))
}

fn compression_covered_sequences_allowing_snapshots(
    messages: &[NeutralChatMessage],
    message_source_sequences: &[Option<i64>],
    snapshots: &[ContextCompressionSnapshotRecord],
    covered_indices: &[usize],
) -> Vec<i64> {
    let mut sequences = Vec::new();

    for index in covered_indices {
        if let Some(sequence) = message_source_sequences
            .get(*index)
            .and_then(|sequence| *sequence)
        {
            sequences.push(sequence);
            continue;
        }

        let Some(message) = messages.get(*index) else {
            continue;
        };
        let Some(snapshot_id) = compression_snapshot_id_from_message(message, snapshots) else {
            continue;
        };
        let Some(snapshot) = snapshots.iter().find(|snapshot| snapshot.id == snapshot_id) else {
            continue;
        };
        sequences.extend(snapshot_covered_sequence_vec(snapshot));
    }

    sequences.sort_unstable();
    sequences.dedup();
    sequences
}

fn compression_covered_snapshot_ids(
    messages: &[NeutralChatMessage],
    message_context_sources: &[PromptContextSource],
    snapshots: &[ContextCompressionSnapshotRecord],
    covered_indices: &[usize],
) -> Vec<String> {
    let mut ids = covered_indices
        .iter()
        .filter(|index| {
            message_context_sources
                .get(**index)
                .is_some_and(|source| matches!(source, PromptContextSource::CompressionSnapshot))
        })
        .filter_map(|index| {
            messages
                .get(*index)
                .and_then(|message| compression_snapshot_id_from_message(message, snapshots))
        })
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn compression_snapshot_id_from_message(
    message: &NeutralChatMessage,
    snapshots: &[ContextCompressionSnapshotRecord],
) -> Option<String> {
    if let Some(start) = message.content.find("<snapshot_id>") {
        let start = start + "<snapshot_id>".len();
        let end = message.content[start..].find("</snapshot_id>")? + start;
        return Some(message.content[start..end].trim().to_string());
    }

    if let Some(id) = message
        .content
        .lines()
        .find_map(|line| line.trim().strip_prefix("Snapshot ID: `"))
        .and_then(|rest| rest.split('`').next())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
    {
        return Some(id);
    }

    // Pure User summary format: match by summary text against known snapshot records.
    snapshots
        .iter()
        .find(|snapshot| snapshot.summary == message.content)
        .map(|snapshot| snapshot.id.clone())
}

fn direct_covered_sequences(
    message_source_sequences: &[Option<i64>],
    covered_indices: &[usize],
) -> Vec<i64> {
    covered_indices
        .iter()
        .filter_map(|index| {
            message_source_sequences
                .get(*index)
                .and_then(|sequence| *sequence)
        })
        .collect()
}

fn replace_covered_messages_with_snapshot(
    messages: &[NeutralChatMessage],
    covered_indices: &[usize],
    snapshot_message: NeutralChatMessage,
) -> Vec<NeutralChatMessage> {
    let covered = covered_indices.iter().copied().collect::<HashSet<_>>();
    let first_covered = covered_indices.first().copied();
    let mut next_messages = Vec::with_capacity(messages.len() - covered.len() + 1);

    for (index, message) in messages.iter().enumerate() {
        if Some(index) == first_covered {
            next_messages.push(snapshot_message.clone());
        }

        if covered.contains(&index) {
            continue;
        }

        next_messages.push(message.clone());
    }

    next_messages
}

fn replace_covered_sequences_with_snapshot(
    message_source_sequences: &[Option<i64>],
    covered_indices: &[usize],
) -> Vec<Option<i64>> {
    let covered = covered_indices.iter().copied().collect::<HashSet<_>>();
    let first_covered = covered_indices.first().copied();
    let mut next_sequences = Vec::with_capacity(message_source_sequences.len() - covered.len() + 1);

    for (index, sequence) in message_source_sequences.iter().enumerate() {
        if Some(index) == first_covered {
            next_sequences.push(None);
        }

        if covered.contains(&index) {
            continue;
        }

        next_sequences.push(*sequence);
    }

    next_sequences
}

fn replace_covered_sources_with_snapshot(
    message_context_sources: &[PromptContextSource],
    covered_indices: &[usize],
    snapshot_source: PromptContextSource,
) -> Vec<PromptContextSource> {
    let covered = covered_indices.iter().copied().collect::<HashSet<_>>();
    let first_covered = covered_indices.first().copied();
    let mut next_sources = Vec::with_capacity(message_context_sources.len() - covered.len() + 1);

    for (index, source) in message_context_sources.iter().enumerate() {
        if Some(index) == first_covered {
            next_sources.push(snapshot_source.clone());
        }

        if covered.contains(&index) {
            continue;
        }

        next_sources.push(source.clone());
    }

    next_sources
}

fn compressed_active_tool_start_index(
    active_tool_start_index: usize,
    covered_indices: &[usize],
) -> usize {
    let removed_before_active_tool = covered_indices
        .iter()
        .filter(|index| **index < active_tool_start_index)
        .count();

    let inserted_before_active_tool = covered_indices
        .first()
        .is_some_and(|index| *index < active_tool_start_index);

    active_tool_start_index - removed_before_active_tool + usize::from(inserted_before_active_tool)
}

fn next_context_snapshot_sequence(
    snapshots: &[ContextCompressionSnapshotRecord],
) -> Result<i64, ApiError> {
    let last = snapshots
        .iter()
        .map(|snapshot| snapshot.sequence)
        .max()
        .unwrap_or(-1);
    last.checked_add(1)
        .ok_or_else(|| ApiError::internal("context compression snapshot sequence overflowed"))
}

#[allow(dead_code)]
pub(crate) fn next_context_compression_snapshot_sequence(
    snapshots: &[ContextCompressionSnapshotRecord],
) -> Result<i64, ApiError> {
    next_context_snapshot_sequence(snapshots)
}

fn neutral_role_label(role: &NeutralChatRole) -> &'static str {
    match role {
        NeutralChatRole::System => "system",
        NeutralChatRole::Developer => "developer",
        NeutralChatRole::User => "user",
        NeutralChatRole::Assistant => "assistant",
        NeutralChatRole::Tool => "tool",
    }
}

pub(crate) fn persist_chat_result(
    context: &PreparedChatContext,
    request_started_at: &str,
    outcome: ChatAuditOutcome,
    events: &[CapturedAuditEvent],
    assistant_text: Option<&str>,
    assistant_reasoning: Option<&str>,
    tool_calls: &[ExecutedToolCall],
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create_critical(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let final_state = outcome.final_state;

    let current_history_run = if context.agent_primary_chat_output {
        context
            .queued_user_message_id
            .as_deref()
            .map(|queued_user_message_id| {
                queued_chat_run_matches_context(&database, context, queued_user_message_id)
            })
            .transpose()?
            .unwrap_or(true)
    } else {
        true
    };

    if context.captured_llm_requests.is_empty() {
        let run_request =
            CapturedLlmRequest::from_run_context(context, request_started_at, outcome, events);
        persist_llm_request(&mut database, context, &run_request)?;
        if !current_history_run {
            database
                .invalidate_llm_request(&run_request.id, "chat history was rewritten")
                .map_err(ApiError::from_workspace_error)?;
        }
    } else {
        // Terminal writes normally happen at every provider request boundary. Retrying here is
        // limited to a failed boundary write so it is both visible to the caller and does not
        // turn chat completion back into the normal audit-finalization point.
        for llm_request in context
            .captured_llm_requests
            .iter()
            .filter(|request| !request.terminal_persisted)
        {
            persist_llm_request(&mut database, context, llm_request)?;
        }
        if !current_history_run {
            for llm_request in &context.captured_llm_requests {
                database
                    .invalidate_llm_request(&llm_request.id, "chat history was rewritten")
                    .map_err(ApiError::from_workspace_error)?;
            }
        }
    }

    if !current_history_run {
        tracing::info!(
            chat_id = %context.chat_id,
            user_message_id = ?context.queued_user_message_id,
            assistant_message_id = %context.assistant_message_id,
            run_id = %context.llm_request_id,
            "skipping stale chat result after history rewrite"
        );
        return Ok(());
    }

    let failure_message = if final_state == "failed" {
        captured_chat_error_message(events)?
    } else {
        None
    };
    let blocked_tool_calls = blocked_tool_calls_from_events(events)?;

    // Final assistant parts must include durable stream events (especially context_compression
    // start/completed) for success, tool-only, failure, and cancel paths. Relying on browser
    // memory alone would drop compression blocks after refresh.
    let assistant_message_id = if !context.agent_primary_chat_output {
        None
    } else if assistant_text.is_some()
        || !tool_calls.is_empty()
        || events_contain_assistant_history_parts(events)
    {
        let content = assistant_text.unwrap_or("");
        let mut tool_call_summaries = database
            .tool_calls_for_chat(&context.chat_id)
            .map_err(ApiError::from_workspace_error)?
            .into_iter()
            .filter(|tool_call| {
                tool_call.message_id.as_deref() == Some(context.assistant_message_id.as_str())
            })
            .map(chat_tool_call_summary)
            .collect::<Result<Vec<_>, _>>()?;
        let executed_tool_call_summaries = tool_calls
            .iter()
            .map(executed_tool_call_summary)
            .collect::<Vec<_>>();
        for tool_call in executed_tool_call_summaries {
            if !tool_call_summaries
                .iter()
                .any(|persisted| persisted.id == tool_call.id)
            {
                tool_call_summaries.push(tool_call);
            }
        }
        tool_call_summaries.extend(blocked_tool_calls.iter().map(|tool_call| {
            ChatToolCallSummary {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                status: "error".to_string(),
                input: tool_call.input.clone(),
                output: Some(tool_call.output.clone()),
                is_error: true,
                started_at: Some(tool_call.started_at.clone()),
                completed_at: Some(tool_call.completed_at.clone()),
                live_output: None,
            }
        }));
        let parts = finalized_assistant_message_parts(
            &context.assistant_message_id,
            events,
            content,
            assistant_reasoning,
            &tool_call_summaries,
            failure_message.as_deref(),
        )?;
        let streaming_state = match final_state {
            "cancelled" => Some("cancelled"),
            "failed" => Some("failed"),
            _ => None,
        };
        let metadata_json = assistant_message_metadata_json(
            assistant_reasoning,
            &context.memories_used,
            &context.code_change_stats,
            streaming_state,
            Some(&parts),
            failure_message.as_deref(),
        )?;
        database
            .upsert_message_content(NewMessage {
                id: &context.assistant_message_id,
                chat_id: &context.chat_id,
                role: "assistant",
                content,
                sequence: context.assistant_sequence,
                metadata_json: Some(&metadata_json),
            })
            .map_err(ApiError::from_workspace_error)?;
        Some(context.assistant_message_id.as_str())
    } else {
        None
    };

    for tool_call in &blocked_tool_calls {
        let input_json = serde_json::to_string(&tool_call.input).map_err(|source| {
            ApiError::internal(format!("failed to serialize blocked tool input: {source}"))
        })?;
        let output_json = serde_json::to_string(&tool_call.output).map_err(|source| {
            ApiError::internal(format!("failed to serialize blocked tool output: {source}"))
        })?;
        database
            .upsert_tool_call(NewToolCall {
                id: &tool_call.id,
                chat_id: &context.chat_id,
                run_id: &context.llm_request_id,
                message_id: assistant_message_id,
                tool_name: &tool_call.name,
                input_json: &input_json,
                status: "error",
                started_at: &tool_call.started_at,
                completed_at: Some(&tool_call.completed_at),
            })
            .map_err(ApiError::from_workspace_error)?;
        database
            .upsert_tool_result(NewToolResult {
                id: &format!("{}-result", tool_call.id),
                tool_call_id: &tool_call.id,
                output_json: &output_json,
                is_error: true,
                created_at: &tool_call.completed_at,
            })
            .map_err(ApiError::from_workspace_error)?;
    }

    for tool_call in tool_calls {
        let input_json = serde_json::to_string(&tool_call.input).map_err(|source| {
            ApiError::internal(format!("failed to serialize tool input: {source}"))
        })?;
        let output_json = serde_json::to_string(&tool_call.output).map_err(|source| {
            ApiError::internal(format!("failed to serialize tool output: {source}"))
        })?;
        let result_id = format!("{}-result", tool_call.id);

        database
            .upsert_tool_call(NewToolCall {
                id: &tool_call.id,
                chat_id: &context.chat_id,
                run_id: &context.llm_request_id,
                message_id: assistant_message_id,
                tool_name: &tool_call.name,
                input_json: &input_json,
                status: if tool_call.is_error {
                    "error"
                } else {
                    "completed"
                },
                started_at: &tool_call.started_at,
                completed_at: Some(&tool_call.completed_at),
            })
            .map_err(ApiError::from_workspace_error)?;
        database
            .upsert_tool_result(NewToolResult {
                id: &result_id,
                tool_call_id: &tool_call.id,
                output_json: &output_json,
                is_error: tool_call.is_error,
                created_at: &tool_call.completed_at,
            })
            .map_err(ApiError::from_workspace_error)?;
    }

    if context.agent_primary_chat_output
        && let Some(queued_user_message_id) = &context.queued_user_message_id
    {
        if let Some(agent_task_id) = context.agent_associations.task_id.as_ref() {
            let _ = database
                .clear_agent_chat_queued_run_if_owned(
                    &context.chat_id,
                    queued_user_message_id,
                    &context.assistant_message_id,
                    context.assistant_sequence,
                    agent_task_id.as_str(),
                    &context.llm_request_id,
                )
                .map_err(ApiError::from_workspace_error)?;
        } else {
            database
                .clear_chat_queued_run(&context.chat_id, queued_user_message_id)
                .map_err(ApiError::from_workspace_error)?;
        }
    }

    let queue_external_derived_effects = context.agent_primary_chat_output
        && persist_inline_chat_derived_effects(&mut database, context, final_state)?;
    drop(database);

    if queue_external_derived_effects {
        queue_memory_extraction_job(context, final_state)?;
        crate::spec_runtime::queue_workspace_spec_update_job(context, final_state)?;
    }

    Ok(())
}

#[derive(Clone, Debug)]
struct BlockedToolCallObservation {
    id: String,
    name: String,
    input: Value,
    output: Value,
    started_at: String,
    completed_at: String,
}

/// Extracts only the explicit guard marker. No user-facing error text is interpreted here.
fn blocked_tool_calls_from_events(
    events: &[CapturedAuditEvent],
) -> Result<Vec<BlockedToolCallObservation>, ApiError> {
    let blocked_outputs = events
        .iter()
        .filter(|event| event.event_type == "tool_result")
        .filter_map(|event| {
            let payload = serde_json::from_str::<Value>(&event.normalized_event_json).ok()?;
            let id = payload.get("toolCallId")?.as_str()?.to_string();
            let output = payload.get("output")?.clone();
            (output.get("source").and_then(Value::as_str)
                == Some(crate::runtime::TOOL_CALL_LOOP_GUARD_SOURCE)
                && output.get("executed").and_then(Value::as_bool) == Some(false))
            .then_some((id, output))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let mut observations = Vec::new();
    for event in events
        .iter()
        .filter(|event| event.event_type == "tool_call")
    {
        let Ok(payload) = serde_json::from_str::<Value>(&event.normalized_event_json) else {
            continue;
        };
        let Some(tool_call) = payload.get("toolCall") else {
            continue;
        };
        let Some(id) = tool_call.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(output) = blocked_outputs.get(id).cloned() else {
            continue;
        };
        let string = |field: &str| {
            tool_call
                .get(field)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| {
                    ApiError::internal(format!("blocked tool call event is missing {field}"))
                })
        };
        let started_at = tool_call
            .get("startedAt")
            .and_then(Value::as_str)
            .unwrap_or(&event.event_at)
            .to_string();
        let completed_at = tool_call
            .get("completedAt")
            .and_then(Value::as_str)
            .unwrap_or(&started_at)
            .to_string();
        observations.push(BlockedToolCallObservation {
            id: string("id")?,
            name: string("name")?,
            input: tool_call.get("input").cloned().unwrap_or(Value::Null),
            output,
            started_at,
            completed_at,
        });
    }
    Ok(observations)
}

/// True when the captured stream has part-bearing history events that must land in
/// assistant `metadata.parts` even when the turn ends without final text (failure/cancel).
fn events_contain_assistant_history_parts(events: &[CapturedAuditEvent]) -> bool {
    events.iter().any(|event| {
        matches!(
            event.event_type.as_str(),
            "text_delta"
                | "reasoning_delta"
                | "tool_call"
                | "context_compression"
                | "guidance_applied"
                | "error"
        )
    })
}

fn persist_inline_chat_derived_effects(
    database: &mut WorkspaceDatabase,
    context: &PreparedChatContext,
    final_state: &str,
) -> Result<bool, ApiError> {
    if !context.lifecycle.allows_derived_effects() {
        return Ok(false);
    }
    if final_state != "succeeded" {
        return Ok(false);
    }
    if let Some(provenance) = &context.plan_phase_provenance {
        debug_assert_eq!(
            provenance.integration_status,
            PlanPhaseIntegrationStatus::AwaitingIntegration
        );
        let context_json = json!({
            "workspaceId": context.workspace_id,
            "chatId": context.chat_id,
            "runId": context.llm_request_id,
            "userMessageId": context.user_message_id,
            "assistantMessageId": context.assistant_message_id,
            "modelId": context.model_id,
            "providerId": context.provider_id,
            "memoryTargetStatus": context.memory_target_status.as_str(),
            "codeChangeStats": context.code_change_stats,
        })
        .to_string();
        database
            .insert_plan_phase_derived_effects(NewPlanPhaseDerivedEffects {
                attempt_id: &provenance.attempt_id,
                plan_id: &provenance.plan_id,
                phase_id: &provenance.phase_id,
                agent_task_id: &provenance.agent_task_id,
                chat_id: &context.chat_id,
                run_id: &context.llm_request_id,
                user_message_id: &context.user_message_id,
                assistant_message_id: &context.assistant_message_id,
                context_json: &context_json,
            })
            .map_err(ApiError::from_workspace_error)?;
        return Ok(false);
    }

    Ok(true)
}

fn queued_chat_run_matches_context(
    database: &WorkspaceDatabase,
    context: &PreparedChatContext,
    queued_user_message_id: &str,
) -> Result<bool, ApiError> {
    let Some(chat) = database
        .chat(&context.chat_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(false);
    };
    let queued_run = queued_run_summary_from_chat_metadata(&chat.metadata_json)?;
    Ok(queued_run.is_some_and(|queued_run| {
        queued_run.user_message_id == queued_user_message_id
            && queued_run.assistant_message_id.as_deref()
                == Some(context.assistant_message_id.as_str())
            && queued_run
                .assistant_sequence
                .is_none_or(|sequence| sequence == context.assistant_sequence)
    }))
}

pub(crate) fn persist_running_llm_request(
    context: &PreparedChatContext,
    request_id: &str,
    request_started_at: &str,
    request_body_json: Option<&str>,
    events: &[CapturedAuditEvent],
) -> Result<(), ApiError> {
    persist_running_llm_request_for_kind(
        context,
        request_id,
        request_started_at,
        "chat completion",
        request_body_json,
        events,
    )
}

fn persist_running_llm_request_for_kind(
    context: &PreparedChatContext,
    request_id: &str,
    request_started_at: &str,
    request_kind: &str,
    request_body_json: Option<&str>,
    events: &[CapturedAuditEvent],
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let save_details = api_audit_save_details(&context.global_config);
    let chat_id = database
        .chat(&context.chat_id)
        .map_err(ApiError::from_workspace_error)?
        .is_some()
        .then_some(context.chat_id.as_str());
    let request = NewLlmRequest {
        id: request_id,
        workspace_id: &context.workspace_id,
        chat_id,
        request_kind,
        agent_team_id: context.agent_associations.team_id.as_ref(),
        agent_instance_id: context.agent_associations.instance_id.as_ref(),
        agent_task_id: context.agent_associations.task_id.as_ref(),
        agent_attempt_id: context.agent_associations.attempt_id.as_ref(),
        provider_id: &context.provider_id,
        model_id: &context.model_id,
        thinking_level: context.provider_request.thinking_level.as_deref(),
        request_started_at,
        first_token_at: None,
        completed_at: None,
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_write_tokens: None,
        reasoning_tokens: None,
        first_token_latency_ms: None,
        total_latency_ms: None,
        status_code: None,
        final_state: "running",
        request_body_json: request_body_json
            .and_then(|value| api_audit_detail_json(value, save_details)),
        response_body_json: None,
    };
    if context.agent_primary_chat_output
        && let (Some(queued_user_message_id), Some(agent_task_id)) = (
            context.queued_user_message_id.as_deref(),
            context.agent_associations.task_id.as_ref(),
        )
    {
        let inserted = database
            .insert_llm_request_if_agent_chat_run_owned(
                request,
                &context.chat_id,
                queued_user_message_id,
                &context.assistant_message_id,
                context.assistant_sequence,
                agent_task_id.as_str(),
                &context.llm_request_id,
            )
            .map_err(ApiError::from_workspace_error)?;
        if !inserted {
            return Err(ApiError::conflict(
                "chat run is no longer current (queued run missing, replaced, or owned by another task)",
            ));
        }
    } else {
        if context.agent_primary_chat_output
            && let Some(queued_user_message_id) = context.queued_user_message_id.as_deref()
            && !queued_chat_run_matches_context(&database, context, queued_user_message_id)?
        {
            return Err(ApiError::conflict(
                "chat run is no longer current because its queued run was replaced",
            ));
        }
        database
            .insert_llm_request(request)
            .map_err(ApiError::from_workspace_error)?;
    }
    persist_llm_request_events(&mut database, request_id, events, 0, save_details)
}

fn persist_llm_request(
    database: &mut WorkspaceDatabase,
    context: &PreparedChatContext,
    request: &CapturedLlmRequest,
) -> Result<(), ApiError> {
    let save_details = api_audit_save_details(&context.global_config);
    if database
        .llm_request(&request.id)
        .map_err(ApiError::from_workspace_error)?
        .is_some()
    {
        let compact_events = compact_audit_events(&request.events, save_details);
        let event_ids = compact_events
            .iter()
            .map(|(sequence, _)| format!("{}-event-{sequence}", request.id))
            .collect::<Vec<_>>();
        let audit_events = compact_events
            .iter()
            .enumerate()
            .map(|(index, (sequence, event))| {
                let sequence = i64::try_from(*sequence).map_err(|_| {
                    ApiError::internal("too many LLM request events to fit SQLite sequence")
                })?;
                Ok(NewLlmRequestEvent {
                    id: &event_ids[index],
                    llm_request_id: &request.id,
                    sequence,
                    event_at: &event.event_at,
                    event_type: &event.event_type,
                    raw_chunk_json: None,
                    normalized_event_json: &event.normalized_event_json,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        database
            .finalize_llm_request_outcome_with_events(
                &request.id,
                UpdateLlmRequestOutcome {
                    first_token_at: request.outcome.first_token_at.as_deref(),
                    completed_at: Some(&request.outcome.completed_at),
                    input_tokens: request.outcome.input_tokens,
                    output_tokens: request.outcome.output_tokens,
                    cache_read_tokens: request.outcome.cache_read_tokens,
                    cache_write_tokens: request.outcome.cache_write_tokens,
                    reasoning_tokens: request.outcome.reasoning_tokens,
                    first_token_latency_ms: request.outcome.first_token_latency_ms,
                    total_latency_ms: Some(request.outcome.total_latency_ms),
                    status_code: request.outcome.status_code,
                    final_state: request.outcome.final_state,
                    response_body_json: request.outcome.response_body_json.as_deref().and_then(
                        |value| {
                            persistable_audit_response_body_json(
                                value,
                                save_details,
                                request.outcome.final_state,
                            )
                        },
                    ),
                },
                &audit_events,
            )
            .map_err(ApiError::from_workspace_error)
    } else {
        database
            .insert_llm_request(NewLlmRequest {
                id: &request.id,
                workspace_id: &context.workspace_id,
                chat_id: Some(&context.chat_id),
                request_kind: request.request_kind,
                agent_team_id: context.agent_associations.team_id.as_ref(),
                agent_instance_id: context.agent_associations.instance_id.as_ref(),
                agent_task_id: context.agent_associations.task_id.as_ref(),
                agent_attempt_id: context.agent_associations.attempt_id.as_ref(),
                provider_id: &context.provider_id,
                model_id: &context.model_id,
                thinking_level: context.provider_request.thinking_level.as_deref(),
                request_started_at: &request.request_started_at,
                first_token_at: request.outcome.first_token_at.as_deref(),
                completed_at: Some(&request.outcome.completed_at),
                input_tokens: request.outcome.input_tokens,
                output_tokens: request.outcome.output_tokens,
                cache_read_tokens: request.outcome.cache_read_tokens,
                cache_write_tokens: request.outcome.cache_write_tokens,
                reasoning_tokens: request.outcome.reasoning_tokens,
                first_token_latency_ms: request.outcome.first_token_latency_ms,
                total_latency_ms: Some(request.outcome.total_latency_ms),
                status_code: request.outcome.status_code,
                final_state: request.outcome.final_state,
                request_body_json: (!request.request_body_json.is_empty())
                    .then(|| request.request_body_json.as_str())
                    .and_then(|value| api_audit_detail_json(value, save_details)),
                response_body_json: request.outcome.response_body_json.as_deref().and_then(
                    |value| {
                        persistable_audit_response_body_json(
                            value,
                            save_details,
                            request.outcome.final_state,
                        )
                    },
                ),
            })
            .map_err(ApiError::from_workspace_error)?;
        persist_llm_request_events(database, &request.id, &request.events, 0, save_details)
    }
}

fn persist_llm_request_events(
    database: &mut WorkspaceDatabase,
    request_id: &str,
    events: &[CapturedAuditEvent],
    start_index: usize,
    save_details: bool,
) -> Result<(), ApiError> {
    for (index, event) in compact_audit_events(events, save_details)
        .into_iter()
        .filter(|(index, _)| *index >= start_index)
    {
        let sequence = i64::try_from(index).map_err(|_| {
            ApiError::internal("too many LLM request events to fit SQLite sequence")
        })?;
        let id = format!("{request_id}-event-{sequence}");

        database
            .insert_llm_request_event(NewLlmRequestEvent {
                id: &id,
                llm_request_id: request_id,
                sequence,
                event_at: &event.event_at,
                event_type: &event.event_type,
                raw_chunk_json: None,
                normalized_event_json: &event.normalized_event_json,
            })
            .map_err(ApiError::from_workspace_error)?;
    }

    Ok(())
}
pub(crate) fn context_message_groups(
    messages: &[NeutralChatMessage],
    message_source_sequences: &[Option<i64>],
    message_context_sources: &[PromptContextSource],
    active_tool_start_index: usize,
) -> Result<Vec<ContextMessageGroup>, ApiError> {
    if messages.len() != message_source_sequences.len() {
        return Err(ApiError::internal(
            "context message source sequence count does not match prompt message count",
        ));
    }
    if messages.len() != message_context_sources.len() {
        return Err(ApiError::internal(
            "context message source classification count does not match prompt message count",
        ));
    }

    let latest_user_index = messages
        .iter()
        .rposition(|message| message.role == NeutralChatRole::User);
    let mut groups = Vec::new();
    let mut index = 0;

    while index < messages.len() {
        let source_sequence = message_source_sequences[index];
        let group_key = prompt_context_group_key(&message_context_sources[index]);
        let mut message_indices = vec![index];
        index += 1;

        if let Some(group_key) = group_key {
            while index < messages.len()
                && prompt_context_group_key(&message_context_sources[index]).as_ref()
                    == Some(&group_key)
            {
                message_indices.push(index);
                index += 1;
            }
        } else if source_sequence.is_some() {
            while index < messages.len() && message_source_sequences[index] == source_sequence {
                message_indices.push(index);
                index += 1;
            }
        }

        let estimated_tokens = message_indices
            .iter()
            .map(|message_index| {
                if matches!(
                    message_context_sources[*message_index],
                    PromptContextSource::ReservedPrompt
                ) {
                    0
                } else {
                    neutral_message_estimated_tokens(&messages[*message_index])
                }
            })
            .sum();
        let source_bucket =
            prompt_context_source_bucket(&message_context_sources[message_indices[0]]);
        let runtime_tool_batch_index = message_indices.iter().find_map(|message_index| {
            match message_context_sources[*message_index] {
                PromptContextSource::RuntimeToolState { batch_index } => Some(batch_index),
                _ => None,
            }
        });
        let must_keep = message_indices.iter().any(|message_index| {
            matches!(
                messages[*message_index].role,
                NeutralChatRole::System | NeutralChatRole::Developer
            ) || prompt_context_source_is_required(&message_context_sources[*message_index])
                || Some(*message_index) == latest_user_index
                || *message_index >= active_tool_start_index
        });

        groups.push(ContextMessageGroup {
            message_indices,
            estimated_tokens,
            must_keep,
            source_bucket,
            runtime_tool_batch_index,
        });
    }

    Ok(groups)
}

fn prompt_context_group_key(source: &PromptContextSource) -> Option<PromptContextGroupKey> {
    match source {
        PromptContextSource::StoredMessage { sequence }
        | PromptContextSource::TurnMemory { sequence } => {
            Some(PromptContextGroupKey::MessageSequence(*sequence))
        }
        PromptContextSource::AgentCurrentTask { sequence } => {
            Some(PromptContextGroupKey::AgentCurrentTask(*sequence))
        }
        PromptContextSource::RuntimeToolState { batch_index } => {
            Some(PromptContextGroupKey::RuntimeToolBatch(*batch_index))
        }
        _ => None,
    }
}

pub(crate) fn prompt_context_source_bucket(
    source: &PromptContextSource,
) -> PromptContextSourceBucket {
    match source {
        PromptContextSource::ReservedPrompt => PromptContextSourceBucket::ReservedPrompt,
        PromptContextSource::AgentDefinition => PromptContextSourceBucket::AgentDefinition,
        PromptContextSource::AgentTeamProtocol => PromptContextSourceBucket::AgentTeamProtocol,
        PromptContextSource::StableInjection => PromptContextSourceBucket::StableInjection,
        PromptContextSource::ProjectSpec => PromptContextSourceBucket::ProjectSpec,
        PromptContextSource::TodoGraph => PromptContextSourceBucket::TodoGraph,
        PromptContextSource::CompressionSnapshot => PromptContextSourceBucket::CompressionSnapshot,
        PromptContextSource::AgentPrivateContext => PromptContextSourceBucket::AgentPrivateContext,
        PromptContextSource::StoredMessage { .. } => PromptContextSourceBucket::PersistedHistory,
        PromptContextSource::TurnMemory { .. } => PromptContextSourceBucket::TurnMemory,
        PromptContextSource::CurrentUser { .. } => PromptContextSourceBucket::CurrentUser,
        PromptContextSource::AgentCurrentTask { .. } => PromptContextSourceBucket::AgentCurrentTask,
        PromptContextSource::AgentUnreadMessage => PromptContextSourceBucket::AgentUnreadMessage,
        PromptContextSource::AssistantDraft => PromptContextSourceBucket::AssistantDraft,
        PromptContextSource::HookContext => PromptContextSourceBucket::HookContext,
        PromptContextSource::Guidance => PromptContextSourceBucket::Guidance,
        PromptContextSource::RuntimeGuard => PromptContextSourceBucket::RuntimeGuard,
        PromptContextSource::RuntimeAssistant => PromptContextSourceBucket::RuntimeAssistant,
        PromptContextSource::RuntimeToolState { .. } => PromptContextSourceBucket::RuntimeToolState,
        PromptContextSource::RuntimeToolStateSnapshot => {
            PromptContextSourceBucket::RuntimeToolStateSnapshot
        }
    }
}

pub(crate) fn prompt_context_source_is_required(source: &PromptContextSource) -> bool {
    !matches!(
        source,
        PromptContextSource::StoredMessage { .. }
            | PromptContextSource::AgentPrivateContext
            | PromptContextSource::TurnMemory { .. }
            | PromptContextSource::RuntimeToolState { .. }
            | PromptContextSource::RuntimeToolStateSnapshot
    )
}

fn pack_items_from_message_groups(groups: &[ContextMessageGroup]) -> Vec<ContextPackItem> {
    groups
        .iter()
        .enumerate()
        .map(|(index, group)| ContextPackItem {
            id: format!("message-group-{index}"),
            estimated_tokens: group.estimated_tokens,
            must_keep: group.must_keep,
        })
        .collect()
}

pub(crate) fn context_token_breakdown(groups: &[ContextMessageGroup]) -> ContextTokenBreakdown {
    const SOURCES: &[PromptContextSourceBucket] = &[
        PromptContextSourceBucket::ReservedPrompt,
        PromptContextSourceBucket::AgentDefinition,
        PromptContextSourceBucket::AgentTeamProtocol,
        PromptContextSourceBucket::StableInjection,
        PromptContextSourceBucket::ProjectSpec,
        PromptContextSourceBucket::ToolCalls,
        PromptContextSourceBucket::CompressionSnapshot,
        PromptContextSourceBucket::AgentPrivateContext,
        PromptContextSourceBucket::PersistedHistory,
        PromptContextSourceBucket::TurnMemory,
        PromptContextSourceBucket::CurrentUser,
        PromptContextSourceBucket::AgentCurrentTask,
        PromptContextSourceBucket::AgentUnreadMessage,
        PromptContextSourceBucket::AssistantDraft,
        PromptContextSourceBucket::HookContext,
        PromptContextSourceBucket::Guidance,
        PromptContextSourceBucket::RuntimeGuard,
        PromptContextSourceBucket::RuntimeAssistant,
    ];

    let mut by_source = SOURCES
        .iter()
        .copied()
        .map(|source| ContextSourceTokenBreakdown {
            source,
            tokens: 0,
            required_tokens: 0,
            optional_tokens: 0,
            compressible_tokens: 0,
        })
        .collect::<Vec<_>>();

    for group in groups {
        let source = context_token_breakdown_source_bucket(group.source_bucket);
        let entry = by_source
            .iter_mut()
            .find(|entry| entry.source == source)
            .expect("all prompt context source buckets must be listed");
        entry.tokens = entry.tokens.saturating_add(group.estimated_tokens);
        if group.must_keep {
            entry.required_tokens = entry.required_tokens.saturating_add(group.estimated_tokens);
        } else {
            entry.optional_tokens = entry.optional_tokens.saturating_add(group.estimated_tokens);
        }
        if context_group_is_compressible(group) {
            entry.compressible_tokens = entry
                .compressible_tokens
                .saturating_add(group.estimated_tokens);
        }
    }

    by_source.retain(|entry| {
        entry.tokens > 0 || entry.source == PromptContextSourceBucket::ReservedPrompt
    });
    let required_tokens = by_source
        .iter()
        .map(|entry| entry.required_tokens)
        .sum::<u64>();
    let optional_tokens = by_source
        .iter()
        .map(|entry| entry.optional_tokens)
        .sum::<u64>();
    let compressible_tokens = by_source
        .iter()
        .map(|entry| entry.compressible_tokens)
        .sum::<u64>();

    ContextTokenBreakdown {
        required_tokens,
        optional_tokens,
        compressible_tokens,
        by_source,
    }
}

fn context_token_breakdown_source_bucket(
    source: PromptContextSourceBucket,
) -> PromptContextSourceBucket {
    match source {
        PromptContextSourceBucket::TodoGraph
        | PromptContextSourceBucket::RuntimeToolState
        | PromptContextSourceBucket::RuntimeToolStateSnapshot => {
            PromptContextSourceBucket::ToolCalls
        }
        source => source,
    }
}

fn context_group_is_compressible(group: &ContextMessageGroup) -> bool {
    group.estimated_tokens > 0 && is_llm_checkpoint_source_bucket(group.source_bucket)
}

fn required_context_overflow_error(
    required_tokens: u64,
    available_tokens: u64,
    breakdown: &ContextTokenBreakdown,
) -> ApiError {
    ApiError::bad_request(format!(
        "required context messages need {required_tokens} tokens but only {available_tokens} are available; breakdown: {}",
        context_breakdown_summary(breakdown)
    ))
}

fn context_breakdown_summary(breakdown: &ContextTokenBreakdown) -> String {
    breakdown
        .by_source
        .iter()
        .filter(|entry| entry.tokens > 0 || entry.required_tokens > 0)
        .map(|entry| {
            format!(
                "{} total={} required={} optional={} compressible={}",
                prompt_context_source_bucket_label(entry.source),
                entry.tokens,
                entry.required_tokens,
                entry.optional_tokens,
                entry.compressible_tokens
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) fn prompt_context_source_bucket_label(
    source: PromptContextSourceBucket,
) -> &'static str {
    match source {
        PromptContextSourceBucket::ReservedPrompt => "reservedPrompt",
        PromptContextSourceBucket::AgentDefinition => "agentDefinition",
        PromptContextSourceBucket::AgentTeamProtocol => "agentTeamProtocol",
        PromptContextSourceBucket::StableInjection => "stableInjection",
        PromptContextSourceBucket::ProjectSpec => "projectSpec",
        PromptContextSourceBucket::TodoGraph => "todoGraph",
        PromptContextSourceBucket::CompressionSnapshot => "compressionSnapshot",
        PromptContextSourceBucket::AgentPrivateContext => "agentPrivateContext",
        PromptContextSourceBucket::PersistedHistory => "persistedHistory",
        PromptContextSourceBucket::TurnMemory => "turnMemory",
        PromptContextSourceBucket::CurrentUser => "currentUser",
        PromptContextSourceBucket::AgentCurrentTask => "agentCurrentTask",
        PromptContextSourceBucket::AgentUnreadMessage => "agentUnreadMessage",
        PromptContextSourceBucket::AssistantDraft => "assistantDraft",
        PromptContextSourceBucket::HookContext => "hookContext",
        PromptContextSourceBucket::Guidance => "guidance",
        PromptContextSourceBucket::RuntimeGuard => "runtimeGuard",
        PromptContextSourceBucket::RuntimeAssistant => "runtimeAssistant",
        PromptContextSourceBucket::ToolCalls => "toolCalls",
        PromptContextSourceBucket::RuntimeToolState => "runtimeToolState",
        PromptContextSourceBucket::RuntimeToolStateSnapshot => "runtimeToolStateSnapshot",
    }
}

fn post_compression_context_usage_metadata_from_budget(
    budget: &foco_agent::ContextBudget,
    messages: &[NeutralChatMessage],
    source_sequences: &[Option<i64>],
    sources: &[PromptContextSource],
    active_tool_start_index: usize,
) -> Result<Value, ApiError> {
    let message_groups =
        context_message_groups(messages, source_sequences, sources, active_tool_start_index)?;
    let segments = context_usage_segments(budget, &message_groups);
    Ok(json!({
        "contextWindow": budget.context_window,
        "maxOutputTokens": budget.max_output_tokens,
        "triggerTokens": context_window_compression_trigger_tokens(budget.context_window),
        "totalUsedTokens": context_usage_segments_total(&segments),
        "segments": segments,
    }))
}

pub(crate) fn context_usage_segments(
    budget: &foco_agent::ContextBudget,
    groups: &[ContextMessageGroup],
) -> ContextUsageSegments {
    let mut segments = ContextUsageSegments {
        system_prompt: budget.system_prompt_tokens,
        tool_schema: budget.tool_schema_tokens,
        reserved_output: 0,
        ..ContextUsageSegments::default()
    };

    for group in groups {
        match group.source_bucket {
            PromptContextSourceBucket::CompressionSnapshot => {
                segments.compression_snapshot = segments
                    .compression_snapshot
                    .saturating_add(group.estimated_tokens);
            }
            PromptContextSourceBucket::ReservedPrompt => {}
            _ => {
                segments.history = segments.history.saturating_add(group.estimated_tokens);
            }
        }
    }

    segments
}

pub(crate) fn context_usage_segments_total(segments: &ContextUsageSegments) -> u64 {
    segments
        .system_prompt
        .saturating_add(segments.tool_schema)
        .saturating_add(segments.compression_snapshot)
        .saturating_add(segments.history)
}

pub(crate) fn context_window_compression_trigger_tokens(context_window: u64) -> u64 {
    context_compression_trigger_tokens(context_window)
}

pub(crate) fn llm_context_compression_trigger_tokens(context_window: u64) -> u64 {
    context_window.saturating_mul(19) / 20
}

/// Dual-source Normal (95%) LLM checkpoint gate: local heuristic total OR last chat-completion
/// provider `input_tokens`. Shared by local and remote ensure paths.
pub(crate) fn should_trigger_normal_llm_context_compression(
    local_total_used_tokens: u64,
    last_chat_completion_input_tokens: Option<u64>,
    context_window: u64,
) -> bool {
    let trigger = llm_context_compression_trigger_tokens(context_window);
    local_total_used_tokens >= trigger
        || last_chat_completion_input_tokens.is_some_and(|tokens| tokens >= trigger)
}

/// Metadata label for which Normal gate fired. Returns `None` when neither source is at threshold
/// (should not happen on the Normal path after the gate check).
pub(crate) fn llm_context_compression_trigger_source(
    local_total_used_tokens: u64,
    last_chat_completion_input_tokens: Option<u64>,
    context_window: u64,
) -> Option<&'static str> {
    let trigger = llm_context_compression_trigger_tokens(context_window);
    let local = local_total_used_tokens >= trigger;
    let provider = last_chat_completion_input_tokens.is_some_and(|tokens| tokens >= trigger);
    match (local, provider) {
        (true, true) => Some("both"),
        (true, false) => Some("localEstimate"),
        (false, true) => Some("providerInput"),
        (false, false) => None,
    }
}

/// Record provider-reported chat-completion input tokens for the next Normal compression gate.
///
/// Semantics: replace the sample for the most recently completed chat-completion turn.
/// Positive `input_tokens` become the new sample; missing, zero, or non-positive values clear the
/// cache so an earlier high-water mark cannot stick across turns. Call only for chat completion
/// (not contextCompression / hooks / memory / Spec).
pub(crate) fn record_chat_completion_input_tokens(
    last_chat_completion_input_tokens: &mut Option<u64>,
    input_tokens: Option<i64>,
) {
    let Some(tokens) = input_tokens.filter(|tokens| *tokens > 0) else {
        *last_chat_completion_input_tokens = None;
        return;
    };
    let Ok(tokens) = u64::try_from(tokens) else {
        *last_chat_completion_input_tokens = None;
        return;
    };
    *last_chat_completion_input_tokens = Some(tokens);
}

pub(crate) struct ContextUsageInput<'a> {
    pub(crate) messages: &'a [NeutralChatMessage],
    pub(crate) message_source_sequences: &'a [Option<i64>],
    pub(crate) message_context_sources: &'a [PromptContextSource],
    pub(crate) active_tool_start_index: usize,
    pub(crate) context_budget: &'a foco_agent::ContextBudget,
    pub(crate) memory_context_tokens: u64,
    pub(crate) memory_budget_tokens: u64,
}

pub(crate) fn context_usage_response(
    context: ContextUsageInput<'_>,
) -> Result<ContextUsageResponse, ApiError> {
    let message_groups = context_message_groups(
        context.messages,
        context.message_source_sequences,
        context.message_context_sources,
        context.active_tool_start_index,
    )?;
    let assembled_message_tokens = message_groups
        .iter()
        .map(|group| group.estimated_tokens)
        .sum::<u64>();
    let available_message_tokens = context.context_budget.available_message_tokens;
    let context_window = context.context_budget.context_window;
    let max_output_tokens = context.context_budget.max_output_tokens;
    let assembled_segments = context_usage_segments(context.context_budget, &message_groups);
    let assembled_total_used_context_tokens = context_usage_segments_total(&assembled_segments);
    let token_breakdown = context_token_breakdown(&message_groups);
    let (packed_message_tokens, packed_groups) =
        if token_breakdown.required_tokens > available_message_tokens {
            (assembled_message_tokens, message_groups.clone())
        } else {
            let pack_items = pack_items_from_message_groups(&message_groups);
            let packed = pack_context(&pack_items, available_message_tokens)
                .map_err(|source| ApiError::bad_request(source.to_string()))?;
            let packed_message_tokens = packed.used_message_tokens;
            let packed_groups = packed
                .selected_indices
                .into_iter()
                .map(|index| message_groups[index].clone())
                .collect::<Vec<_>>();
            (packed_message_tokens, packed_groups)
        };
    let used_message_tokens = packed_message_tokens;
    let compression_trigger_tokens = context_window_compression_trigger_tokens(context_window);
    let compression_trigger_percent = percentage_ceil(compression_trigger_tokens, context_window);
    let llm_compression_trigger_tokens = llm_context_compression_trigger_tokens(context_window);
    let llm_compression_trigger_percent =
        percentage_ceil(llm_compression_trigger_tokens, context_window);
    let normal_llm_compression_plan = assembled_total_used_context_tokens
        >= llm_compression_trigger_tokens
        && !llm_context_compression_group_indices(
            &message_groups,
            available_message_tokens,
            LlmContextCompressionMode::Normal,
        )
        .is_empty();
    let required_overflow_llm_compression_plan = token_breakdown.required_tokens
        > available_message_tokens
        && !llm_context_compression_group_indices(
            &message_groups,
            available_message_tokens,
            LlmContextCompressionMode::RequiredOverflow,
        )
        .is_empty();
    let has_llm_compression_plan =
        normal_llm_compression_plan || required_overflow_llm_compression_plan;
    let segments = context_usage_segments(context.context_budget, &packed_groups);
    let total_used_context_tokens = context_usage_segments_total(&segments);
    let usage_percent = percentage_ceil(total_used_context_tokens, context_window);
    let assembled_usage_percent =
        percentage_ceil(assembled_total_used_context_tokens, context_window);
    let will_compress_on_next_send = has_llm_compression_plan;

    Ok(ContextUsageResponse {
        used_message_tokens,
        assembled_message_tokens,
        assembled_usage_percent,
        post_compression_message_tokens: assembled_message_tokens,
        packed_message_tokens,
        available_message_tokens,
        context_window,
        max_output_tokens,
        system_prompt_tokens: segments.system_prompt,
        tool_schema_tokens: segments.tool_schema,
        history_tokens: segments.history,
        compression_snapshot_tokens: segments.compression_snapshot,
        total_used_context_tokens,
        memory_context_tokens: context.memory_context_tokens,
        memory_budget_tokens: context.memory_budget_tokens,
        usage_percent,
        compression_trigger_tokens,
        compression_trigger_percent,
        llm_compression_trigger_tokens,
        llm_compression_trigger_percent,
        has_llm_compression_plan,
        will_compress_on_next_send,
        segments,
        token_breakdown,
    })
}

fn percentage_ceil(value: u64, total: u64) -> u64 {
    if total == 0 {
        0
    } else {
        value.saturating_mul(100).div_ceil(total)
    }
}

fn message_group_indices(
    groups: &[ContextMessageGroup],
    group_indices: &[usize],
) -> Result<Vec<usize>, ApiError> {
    let mut message_indices = Vec::new();

    for group_index in group_indices {
        let group = groups.get(*group_index).ok_or_else(|| {
            ApiError::internal("context compression covered group index is out of bounds")
        })?;
        message_indices.extend(group.message_indices.iter().copied());
    }

    Ok(message_indices)
}

pub(crate) fn pack_neutral_messages(
    messages: Vec<NeutralChatMessage>,
    message_source_sequences: &[Option<i64>],
    message_context_sources: &[PromptContextSource],
    budget: &foco_agent::ContextBudget,
    active_tool_start_index: usize,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    if messages.len() != message_source_sequences.len() {
        return Err(ApiError::internal(
            "context message source sequence count does not match prompt message count",
        ));
    }

    let message_groups = context_message_groups(
        &messages,
        message_source_sequences,
        message_context_sources,
        active_tool_start_index,
    )?;
    let pack_items = pack_items_from_message_groups(&message_groups);
    let breakdown = context_token_breakdown(&message_groups);
    if breakdown.required_tokens > budget.available_message_tokens {
        return Err(required_context_overflow_error(
            breakdown.required_tokens,
            budget.available_message_tokens,
            &breakdown,
        ));
    }
    let packed = pack_context(&pack_items, budget.available_message_tokens)
        .map_err(|source| ApiError::bad_request(source.to_string()))?;

    let selected_indices = message_group_indices(&message_groups, &packed.selected_indices)?;
    Ok(selected_indices
        .into_iter()
        .map(|index| messages[index].clone())
        .collect())
}

pub(crate) fn neutral_message_estimated_tokens(message: &NeutralChatMessage) -> u64 {
    let mut tokens = estimate_text_tokens(&message.content);

    if let Some(reasoning) = &message.reasoning {
        tokens += estimate_text_tokens(reasoning);
    }

    for attachment in &message.attachments {
        tokens += neutral_attachment_estimated_tokens(attachment);
    }

    for tool_call in &message.tool_calls {
        tokens += neutral_tool_call_estimated_tokens(tool_call);
    }

    if let Some(tool_call_id) = &message.tool_call_id {
        tokens += estimate_text_tokens(tool_call_id);
    }

    if let Some(tool_name) = &message.tool_name {
        tokens += estimate_text_tokens(tool_name);
    }

    tokens
}

fn neutral_attachment_estimated_tokens(attachment: &NeutralChatAttachment) -> u64 {
    estimate_text_tokens(&attachment.name)
        + estimate_text_tokens(&attachment.content_type)
        + attachment
            .path
            .as_deref()
            .map(estimate_text_tokens)
            .unwrap_or(0)
        + estimate_text_tokens(&format!("{} bytes", attachment.size_bytes))
        + 32
}
