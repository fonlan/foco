use std::collections::HashSet;

use foco_agent::{ContextPackItem, context_compression_trigger_tokens, estimate_text_tokens};
use foco_providers::{NeutralChatMessage, NeutralChatRole, NeutralToolCall};
use foco_store::workspace::{ContextCompressionSnapshotRecord, ToolCallWithResultRecord};
use serde_json::{Value, json};

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

pub(crate) async fn ensure_context_compression(
    context: &mut PreparedChatContext,
) -> Result<usize, ApiError> {
    validate_prompt_context_lengths(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
    )?;

    let compressed_runtime_tool_state = compress_runtime_tool_state_if_needed(context, false)?;

    let message_groups = context_message_groups(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
        context.active_tool_start_index,
    )?;
    let used_tokens = message_groups
        .iter()
        .map(|group| group.estimated_tokens)
        .sum::<u64>();
    if used_tokens
        > llm_context_compression_trigger_tokens(context.context_budget.available_message_tokens)
        && ensure_llm_context_compression(context, &message_groups).await?
    {
        return Ok(context.active_tool_start_index);
    }
    let pack_items = pack_items_from_message_groups(&message_groups);

    let Some(plan) = plan_context_compression(
        &pack_items,
        context.context_budget.available_message_tokens,
        active_tool_start_group_index(&message_groups, context.active_tool_start_index),
        CONTEXT_COMPRESSION_PRESERVE_RECENT_MESSAGES,
    ) else {
        if !compressed_runtime_tool_state {
            let breakdown = context_token_breakdown(&message_groups);
            if breakdown.required_tokens > context.context_budget.available_message_tokens {
                compress_runtime_tool_state_if_needed(context, true)?;
            }
        }
        return Ok(context.active_tool_start_index);
    };
    let covered_indices = message_group_indices(&message_groups, &plan.covered_indices)?;

    let summary = context_compression_summary(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &covered_indices,
    )?;
    let summary_token_count = estimate_text_tokens(&summary);

    if summary_token_count >= plan.original_tokens {
        return Ok(context.active_tool_start_index);
    }

    let covered_sequences =
        compression_covered_sequences(&context.message_source_sequences, &covered_indices)?;
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
                "coveredSequences": covered_sequences,
                "originalTokenCount": plan.original_tokens,
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
        return Ok(context.active_tool_start_index);
    }
    let snapshot_id = unique_id("ctx");
    let snapshot_sequence = next_context_snapshot_sequence(&context.compression_snapshots)?;
    let metadata_json = json!({
        "kind": CONTEXT_COMPRESSION_KIND_RULE,
        "coveredSequences": covered_sequences,
        "triggerTokens": plan.trigger_tokens,
        "availableMessageTokens": context.context_budget.available_message_tokens
    })
    .to_string();
    let original_token_count = i64::try_from(plan.original_tokens)
        .map_err(|_| ApiError::internal("context compression original token count exceeds i64"))?;
    let summary_token_count_i64 = i64::try_from(summary_token_count)
        .map_err(|_| ApiError::internal("context compression summary token count exceeds i64"))?;
    let source_message_start_sequence = covered_sequences
        .first()
        .copied()
        .ok_or_else(|| ApiError::internal("context compression has no source message sequence"))?;
    let source_message_end_sequence = covered_sequences
        .last()
        .copied()
        .ok_or_else(|| ApiError::internal("context compression has no source message sequence"))?;

    let mut database = WorkspaceDatabase::open_or_create(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    database
        .insert_context_compression_snapshot(NewContextCompressionSnapshot {
            id: &snapshot_id,
            chat_id: &context.chat_id,
            run_id: &context.llm_request_id,
            sequence: snapshot_sequence,
            summary: &summary,
            source_message_start_sequence,
            source_message_end_sequence,
            original_token_count,
            summary_token_count: summary_token_count_i64,
            metadata_json: Some(&metadata_json),
        })
        .map_err(ApiError::from_workspace_error)?;

    let created_at = utc_timestamp();
    let snapshot = ContextCompressionSnapshotRecord {
        id: snapshot_id,
        chat_id: context.chat_id.clone(),
        run_id: context.llm_request_id.clone(),
        sequence: snapshot_sequence,
        summary: summary.clone(),
        source_message_start_sequence,
        source_message_end_sequence,
        original_token_count,
        summary_token_count: summary_token_count_i64,
        created_at,
        metadata_json,
    };

    context.provider_request.messages = replace_covered_messages_with_snapshot(
        &context.provider_request.messages,
        &covered_indices,
        compression_snapshot_message(&snapshot),
    );
    context.message_source_sequences = replace_covered_sequences_with_snapshot(
        &context.message_source_sequences,
        &covered_indices,
    );
    context.message_context_sources = replace_covered_sources_with_snapshot(
        &context.message_context_sources,
        &covered_indices,
        PromptContextSource::CompressionSnapshot,
    );
    context.active_tool_start_index =
        compressed_active_tool_start_index(context.active_tool_start_index, &covered_indices);
    context.compression_snapshots.push(snapshot);

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

    let message_groups = context_message_groups(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
        context.active_tool_start_index,
    )?;
    let breakdown = context_token_breakdown(&message_groups);
    if breakdown.required_tokens > context.context_budget.available_message_tokens {
        compress_runtime_tool_state_if_needed(context, true)?;
    }

    Ok(context.active_tool_start_index)
}

fn llm_context_compression_trigger_tokens(available_tokens: u64) -> u64 {
    available_tokens.saturating_mul(LLM_CONTEXT_COMPRESSION_TRIGGER_NUMERATOR)
        / LLM_CONTEXT_COMPRESSION_TRIGGER_DENOMINATOR
}

async fn ensure_llm_context_compression(
    context: &mut PreparedChatContext,
    message_groups: &[ContextMessageGroup],
) -> Result<bool, ApiError> {
    let covered_group_indices = llm_context_compression_group_indices(message_groups);
    if covered_group_indices.is_empty() {
        return Ok(false);
    }
    let covered_indices = message_group_indices(message_groups, &covered_group_indices)?;
    let original_tokens = covered_indices
        .iter()
        .map(|index| neutral_message_estimated_tokens(&context.provider_request.messages[*index]))
        .sum::<u64>();
    if original_tokens == 0 {
        return Ok(false);
    }

    let source_summary = context_compression_summary_allowing_snapshots(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &covered_indices,
    )?;
    let summary = llm_context_compression_summary(context, &source_summary).await?;
    let summary_token_count = estimate_text_tokens(&summary);
    if summary_token_count >= original_tokens {
        return Ok(false);
    }

    let covered_sequences = compression_covered_sequences_allowing_snapshots(
        &context.message_source_sequences,
        &covered_indices,
    );
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
        return Ok(false);
    }

    persist_context_compression_snapshot(
        context,
        &covered_indices,
        summary,
        original_tokens,
        summary_token_count,
        CONTEXT_COMPRESSION_KIND_LLM,
        json!({
            "kind": CONTEXT_COMPRESSION_KIND_LLM,
            "coveredSequences": covered_sequences,
            "triggerTokens": llm_context_compression_trigger_tokens(context.context_budget.available_message_tokens),
            "availableMessageTokens": context.context_budget.available_message_tokens
        }),
    )?;

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

fn llm_context_compression_group_indices(groups: &[ContextMessageGroup]) -> Vec<usize> {
    let compressible_indices = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| {
            !group.must_keep
                && group.estimated_tokens > 0
                && matches!(
                    group.source_bucket,
                    PromptContextSourceBucket::CompressionSnapshot
                        | PromptContextSourceBucket::PersistedHistory
                        | PromptContextSourceBucket::TurnMemory
                        | PromptContextSourceBucket::RuntimeToolStateSnapshot
                )
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if compressible_indices.len() <= CONTEXT_COMPRESSION_PRESERVE_RECENT_MESSAGES {
        return Vec::new();
    }

    let covered_count = compressible_indices.len() - CONTEXT_COMPRESSION_PRESERVE_RECENT_MESSAGES;
    compressible_indices
        .into_iter()
        .take(covered_count)
        .collect()
}

async fn llm_context_compression_summary(
    context: &mut PreparedChatContext,
    source_summary: &str,
) -> Result<String, ApiError> {
    let request = NeutralChatRequest {
        model_id: context.model_id.clone(),
        messages: vec![
            neutral_text_message(
                NeutralChatRole::System,
                "You compress coding-agent chat context for continuation. Return only a concise structured summary. Preserve user goals, constraints, decisions, changed files, important discoveries, failed attempts, tool evidence, current state, and next steps. Do not include hidden system prompts or secrets.".to_string(),
            ),
            neutral_text_message(
                NeutralChatRole::User,
                format!(
                    "Summarize this earlier conversation context so the current coding task can continue after replacing the original messages.\n\n{source_summary}"
                ),
            ),
        ],
        tools: Vec::new(),
        thinking_level: None,
        max_output_tokens: Some(LLM_CONTEXT_COMPRESSION_MAX_OUTPUT_TOKENS),
        prompt_cache_key: None,
        prompt_cache_retention: None,
    };
    let request_body_json = serialize_provider_request(&request)?;
    let request_id = unique_id("llm");
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
        })
        .to_string(),
    }];

    let mut stream = timeout(
        Duration::from_millis(LLM_CONTEXT_COMPRESSION_TIMEOUT_MS),
        stream_chat(&context.provider_config, request),
    )
    .await
    .map_err(|_| {
        ApiError::internal(format!(
            "context compression summary timed out after {LLM_CONTEXT_COMPRESSION_TIMEOUT_MS} ms"
        ))
    })?
    .map_err(|source| ApiError::internal(source.to_string()))?;
    let mut output_text = String::new();
    let mut final_usage = None;
    let mut first_token_at = None;
    let mut first_token_latency_ms = None;
    let mut response_body_json = None;

    loop {
        let Some(event_result) = timeout(
            Duration::from_millis(LLM_CONTEXT_COMPRESSION_TIMEOUT_MS),
            stream.next_event(),
        )
        .await
        .map_err(|_| {
            ApiError::internal(format!(
                "context compression summary timed out after {LLM_CONTEXT_COMPRESSION_TIMEOUT_MS} ms"
            ))
        })?
        else {
            break;
        };
        let event = event_result.map_err(|source| ApiError::internal(source.to_string()))?;
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
                return Err(ApiError::internal(format!(
                    "context compression summary called unsupported tool '{}'",
                    tool_call.name
                )));
            }
            NeutralChatStreamEvent::Complete {
                text,
                usage,
                stop_reason,
                response_id,
                ..
            } => {
                if !text.trim().is_empty() {
                    output_text.push_str(&text);
                }
                if let Some(usage) = usage {
                    final_usage = Some(usage);
                }
                response_body_json = Some(
                    json!({
                        "requestKind": "contextCompression",
                        "kind": CONTEXT_COMPRESSION_KIND_LLM,
                        "text": output_text,
                        "usage": final_usage,
                        "stopReason": stop_reason,
                        "responseId": response_id,
                    })
                    .to_string(),
                );
                break;
            }
            NeutralChatStreamEvent::Error { message } => {
                return Err(ApiError::internal(format!(
                    "context compression summary stream error: {message}"
                )));
            }
        }
    }

    let summary = output_text.trim().to_string();
    if summary.is_empty() {
        return Err(ApiError::internal(
            "context compression summary returned empty text",
        ));
    }
    context.captured_llm_requests.push(CapturedLlmRequest {
        id: request_id,
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
            status_code: Some(200),
            final_state: "succeeded",
            response_body_json,
        },
    });

    Ok(summary)
}

fn persist_context_compression_snapshot(
    context: &mut PreparedChatContext,
    covered_indices: &[usize],
    summary: String,
    original_tokens: u64,
    summary_token_count: u64,
    kind: &str,
    metadata: Value,
) -> Result<(), ApiError> {
    let snapshot_id = unique_id("ctx");
    let snapshot_sequence = next_context_snapshot_sequence(&context.compression_snapshots)?;
    let metadata_json = metadata.to_string();
    let original_token_count = i64::try_from(original_tokens)
        .map_err(|_| ApiError::internal("context compression original token count exceeds i64"))?;
    let summary_token_count_i64 = i64::try_from(summary_token_count)
        .map_err(|_| ApiError::internal("context compression summary token count exceeds i64"))?;
    let (source_message_start_sequence, source_message_end_sequence) =
        compression_source_sequence_range(&context.message_source_sequences, covered_indices);

    let mut database = WorkspaceDatabase::open_or_create(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    database
        .insert_context_compression_snapshot(NewContextCompressionSnapshot {
            id: &snapshot_id,
            chat_id: &context.chat_id,
            run_id: &context.llm_request_id,
            sequence: snapshot_sequence,
            summary: &summary,
            source_message_start_sequence,
            source_message_end_sequence,
            original_token_count,
            summary_token_count: summary_token_count_i64,
            metadata_json: Some(&metadata_json),
        })
        .map_err(ApiError::from_workspace_error)?;

    let snapshot = ContextCompressionSnapshotRecord {
        id: snapshot_id,
        chat_id: context.chat_id.clone(),
        run_id: context.llm_request_id.clone(),
        sequence: snapshot_sequence,
        summary: summary.clone(),
        source_message_start_sequence,
        source_message_end_sequence,
        original_token_count,
        summary_token_count: summary_token_count_i64,
        created_at: utc_timestamp(),
        metadata_json,
    };

    context.provider_request.messages = replace_covered_messages_with_snapshot(
        &context.provider_request.messages,
        covered_indices,
        compression_snapshot_message(&snapshot),
    );
    context.message_source_sequences =
        replace_covered_sequences_with_snapshot(&context.message_source_sequences, covered_indices);
    context.message_context_sources = replace_covered_sources_with_snapshot(
        &context.message_context_sources,
        covered_indices,
        PromptContextSource::CompressionSnapshot,
    );
    context.active_tool_start_index =
        compressed_active_tool_start_index(context.active_tool_start_index, covered_indices);
    context.compression_snapshots.push(snapshot);

    tracing::debug!(kind = kind, "created context compression snapshot");
    Ok(())
}

fn compression_source_sequence_range(
    message_source_sequences: &[Option<i64>],
    covered_indices: &[usize],
) -> (i64, i64) {
    let sequences =
        compression_covered_sequences_allowing_snapshots(message_source_sequences, covered_indices);
    let start = sequences.first().copied().unwrap_or(0);
    let end = sequences.last().copied().unwrap_or(start);
    (start, end)
}

pub(crate) fn compress_runtime_tool_state_if_needed(
    context: &mut PreparedChatContext,
    force: bool,
) -> Result<bool, ApiError> {
    validate_prompt_context_lengths(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
    )?;

    let message_groups = context_message_groups(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
        context.active_tool_start_index,
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

    let used_tokens = message_groups
        .iter()
        .map(|group| group.estimated_tokens)
        .sum::<u64>();
    let breakdown = context_token_breakdown(&message_groups);
    let should_compress = force
        || used_tokens
            > context_compression_trigger_tokens(context.context_budget.available_message_tokens)
        || breakdown.required_tokens > context.context_budget.available_message_tokens;
    if !should_compress {
        return Ok(false);
    }

    let covered_tool_group_count =
        runtime_tool_groups.len() - CONTEXT_COMPRESSION_PRESERVE_RECENT_TOOL_BATCHES;
    let mut covered_group_indices = message_groups
        .iter()
        .enumerate()
        .filter_map(|(group_index, group)| {
            if group.source_bucket == PromptContextSourceBucket::RuntimeToolStateSnapshot {
                Some(group_index)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    covered_group_indices.extend(
        runtime_tool_groups
            .iter()
            .take(covered_tool_group_count)
            .map(|(group_index, _, _)| *group_index),
    );
    covered_group_indices.sort_unstable();
    covered_group_indices.dedup();
    if covered_group_indices.is_empty() {
        return Ok(false);
    }
    let covered_message_indices = message_group_indices(&message_groups, &covered_group_indices)?;
    let original_tokens = covered_message_indices
        .iter()
        .map(|index| neutral_message_estimated_tokens(&context.provider_request.messages[*index]))
        .sum::<u64>();
    if original_tokens == 0 {
        return Ok(false);
    }

    let summary = runtime_tool_state_summary(
        &context.provider_request.messages,
        &covered_message_indices,
        true,
    )?;
    let summary_tokens = estimate_text_tokens(&summary);
    if summary_tokens >= original_tokens {
        return Ok(false);
    }

    let snapshot = neutral_text_message(NeutralChatRole::User, summary);
    context.provider_request.messages = replace_covered_messages_with_snapshot(
        &context.provider_request.messages,
        &covered_message_indices,
        snapshot,
    );
    context.message_source_sequences = replace_covered_sequences_with_snapshot(
        &context.message_source_sequences,
        &covered_message_indices,
    );
    context.message_context_sources = replace_covered_sources_with_snapshot(
        &context.message_context_sources,
        &covered_message_indices,
        PromptContextSource::RuntimeToolStateSnapshot,
    );
    context.active_tool_start_index = compressed_active_tool_start_index(
        context.active_tool_start_index,
        &covered_message_indices,
    );

    Ok(true)
}

pub(crate) fn compress_all_runtime_tool_state(
    context: &mut PreparedChatContext,
) -> Result<bool, ApiError> {
    validate_prompt_context_lengths(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
    )?;

    let covered_message_indices = context
        .message_context_sources
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
    if covered_message_indices.is_empty() {
        return Ok(false);
    }

    let summary = runtime_tool_state_summary(
        &context.provider_request.messages,
        &covered_message_indices,
        false,
    )?;
    let snapshot = neutral_text_message(NeutralChatRole::User, summary);
    context.provider_request.messages = replace_covered_messages_with_snapshot(
        &context.provider_request.messages,
        &covered_message_indices,
        snapshot,
    );
    context.message_source_sequences = replace_covered_sequences_with_snapshot(
        &context.message_source_sequences,
        &covered_message_indices,
    );
    context.message_context_sources = replace_covered_sources_with_snapshot(
        &context.message_context_sources,
        &covered_message_indices,
        PromptContextSource::RuntimeToolStateSnapshot,
    );
    context.active_tool_start_index = compressed_active_tool_start_index(
        context.active_tool_start_index,
        &covered_message_indices,
    );

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

pub(crate) fn snapshot_covered_sequences(
    snapshots: &[ContextCompressionSnapshotRecord],
) -> HashSet<i64> {
    let mut sequences = HashSet::new();

    for snapshot in snapshots {
        if let Ok(metadata) = serde_json::from_str::<Value>(&snapshot.metadata_json) {
            if let Some(covered_sequences) =
                metadata.get("coveredSequences").and_then(Value::as_array)
            {
                for sequence in covered_sequences.iter().filter_map(Value::as_i64) {
                    sequences.insert(sequence);
                }
                continue;
            }
        }

        for sequence in
            snapshot.source_message_start_sequence..=snapshot.source_message_end_sequence
        {
            sequences.insert(sequence);
        }
    }

    sequences
}

pub(crate) fn compression_snapshot_message(
    snapshot: &ContextCompressionSnapshotRecord,
) -> NeutralChatMessage {
    neutral_text_message(
        NeutralChatRole::System,
        format!(
            "{CONTEXT_COMPRESSION_PROMPT_PREFIX}\n\
             - snapshot id: {}\n\
             - source message sequence range: {}..={}\n\
             - original tokens: {}\n\
             - summary tokens: {}\n\n{}",
            snapshot.id,
            snapshot.source_message_start_sequence,
            snapshot.source_message_end_sequence,
            snapshot.original_token_count,
            snapshot.summary_token_count,
            snapshot.summary
        ),
    )
}

fn context_compression_summary(
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
        "Structured summary of earlier chat messages that were removed from the live prompt."
            .to_string(),
    ];

    for index in covered_indices
        .iter()
        .copied()
        .take(CONTEXT_COMPRESSION_MAX_MESSAGE_ENTRIES)
    {
        let message = messages.get(index).ok_or_else(|| {
            ApiError::internal("context compression covered message index is out of bounds")
        })?;
        let sequence = message_source_sequences
            .get(index)
            .and_then(|sequence| *sequence)
            .ok_or_else(|| {
                ApiError::internal(
                    "context compression can only cover messages with database sequences",
                )
            })?;

        lines.push(format!(
            "- sequence {sequence}, role {}: {}",
            neutral_role_label(&message.role),
            compact_message_for_compression(message)
        ));
    }

    if covered_indices.len() > CONTEXT_COMPRESSION_MAX_MESSAGE_ENTRIES {
        lines.push(format!(
            "- {} additional older messages were omitted from this snapshot.",
            covered_indices.len() - CONTEXT_COMPRESSION_MAX_MESSAGE_ENTRIES
        ));
    }

    Ok(lines.join("\n"))
}

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

fn compression_covered_sequences(
    message_source_sequences: &[Option<i64>],
    covered_indices: &[usize],
) -> Result<Vec<i64>, ApiError> {
    let mut sequences = Vec::with_capacity(covered_indices.len());

    for index in covered_indices {
        let sequence = message_source_sequences
            .get(*index)
            .and_then(|sequence| *sequence)
            .ok_or_else(|| {
                ApiError::internal(
                    "context compression can only cover messages with database sequences",
                )
            })?;
        sequences.push(sequence);
    }

    Ok(sequences)
}
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
    let next = snapshots
        .iter()
        .map(|snapshot| snapshot.sequence)
        .max()
        .unwrap_or(-1)
        + 1;

    if next < 0 {
        return Err(ApiError::internal(
            "context compression snapshot sequence overflowed",
        ));
    }

    Ok(next)
}

pub(crate) fn serialize_provider_request(request: &NeutralChatRequest) -> Result<String, ApiError> {
    serde_json::to_string(request).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize provider-neutral chat request: {source}"
        ))
    })
}

fn neutral_role_label(role: &NeutralChatRole) -> &'static str {
    match role {
        NeutralChatRole::System => "system",
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
    let mut database = WorkspaceDatabase::open_or_create(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let final_state = outcome.final_state;

    if context.captured_llm_requests.is_empty() {
        let run_request =
            CapturedLlmRequest::from_run_context(context, request_started_at, outcome, events);
        persist_llm_request(&mut database, context, &run_request)?;
    } else {
        for llm_request in &context.captured_llm_requests {
            persist_llm_request(&mut database, context, llm_request)?;
        }
    }

    let assistant_message_id = if !context.agent_primary_chat_output {
        None
    } else if let Some(assistant_text) = assistant_text {
        let tool_call_summaries = tool_calls
            .iter()
            .map(executed_tool_call_summary)
            .collect::<Vec<_>>();
        let parts = finalized_assistant_message_parts(
            &context.assistant_message_id,
            events,
            assistant_text,
            assistant_reasoning,
            &tool_call_summaries,
        )?;
        let metadata_json = assistant_message_metadata_json(
            assistant_reasoning,
            &context.memories_used,
            &context.code_change_stats,
            None,
            Some(&parts),
        )?;
        database
            .upsert_message_content(NewMessage {
                id: &context.assistant_message_id,
                chat_id: &context.chat_id,
                role: "assistant",
                content: assistant_text,
                sequence: context.assistant_sequence,
                metadata_json: Some(&metadata_json),
            })
            .map_err(ApiError::from_workspace_error)?;
        Some(context.assistant_message_id.as_str())
    } else if !tool_calls.is_empty() {
        if database
            .message(&context.assistant_message_id)
            .map_err(ApiError::from_workspace_error)?
            .is_none()
        {
            let streaming_state = match final_state {
                "cancelled" => Some("cancelled"),
                "failed" => Some("failed"),
                _ => None,
            };
            let metadata_json = assistant_message_metadata_json(
                None,
                &context.memories_used,
                &context.code_change_stats,
                streaming_state,
                None,
            )?;
            database
                .upsert_message_content(NewMessage {
                    id: &context.assistant_message_id,
                    chat_id: &context.chat_id,
                    role: "assistant",
                    content: "",
                    sequence: context.assistant_sequence,
                    metadata_json: Some(&metadata_json),
                })
                .map_err(ApiError::from_workspace_error)?;
        }
        Some(context.assistant_message_id.as_str())
    } else {
        None
    };

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

    if let Some(queued_user_message_id) = &context.queued_user_message_id {
        database
            .clear_chat_queued_run(&context.chat_id, queued_user_message_id)
            .map_err(ApiError::from_workspace_error)?;
    }

    drop(database);

    if context.agent_primary_chat_output {
        queue_memory_extraction_job(context, final_state)?;
        crate::spec_runtime::queue_workspace_spec_update_job(context, final_state)?;
    }

    Ok(())
}

pub(crate) fn persist_running_llm_request(
    context: &PreparedChatContext,
    request_id: &str,
    request_started_at: &str,
    request_body_json: &str,
    events: &[CapturedAuditEvent],
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let save_details = api_audit_save_details(&context.global_config);
    database
        .insert_llm_request(NewLlmRequest {
            id: request_id,
            workspace_id: &context.workspace_id,
            chat_id: Some(&context.chat_id),
            agent_team_id: context.agent_associations.team_id.as_ref(),
            agent_instance_id: context.agent_associations.instance_id.as_ref(),
            agent_task_id: context.agent_associations.task_id.as_ref(),
            agent_attempt_id: context.agent_associations.attempt_id.as_ref(),
            provider_id: &context.provider_id,
            model_id: &context.model_id,
            request_started_at,
            first_token_at: None,
            completed_at: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: None,
            final_state: "running",
            request_body_json: api_audit_detail_json(request_body_json, save_details),
            response_body_json: None,
        })
        .map_err(ApiError::from_workspace_error)?;
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
        database
            .update_llm_request_outcome(
                &request.id,
                UpdateLlmRequestOutcome {
                    first_token_at: request.outcome.first_token_at.as_deref(),
                    completed_at: Some(&request.outcome.completed_at),
                    input_tokens: request.outcome.input_tokens,
                    output_tokens: request.outcome.output_tokens,
                    cache_read_tokens: request.outcome.cache_read_tokens,
                    cache_write_tokens: request.outcome.cache_write_tokens,
                    first_token_latency_ms: request.outcome.first_token_latency_ms,
                    total_latency_ms: Some(request.outcome.total_latency_ms),
                    status_code: request.outcome.status_code,
                    final_state: request.outcome.final_state,
                    response_body_json: request
                        .outcome
                        .response_body_json
                        .as_deref()
                        .and_then(|value| api_audit_detail_json(value, save_details)),
                },
            )
            .map_err(ApiError::from_workspace_error)?;
        let next_sequence = database
            .llm_request_event_next_sequence(&request.id)
            .map_err(ApiError::from_workspace_error)?;
        persist_llm_request_events(
            database,
            &request.id,
            &request.events,
            next_sequence,
            save_details,
        )
    } else {
        database
            .insert_llm_request(NewLlmRequest {
                id: &request.id,
                workspace_id: &context.workspace_id,
                chat_id: Some(&context.chat_id),
                agent_team_id: context.agent_associations.team_id.as_ref(),
                agent_instance_id: context.agent_associations.instance_id.as_ref(),
                agent_task_id: context.agent_associations.task_id.as_ref(),
                agent_attempt_id: context.agent_associations.attempt_id.as_ref(),
                provider_id: &context.provider_id,
                model_id: &context.model_id,
                request_started_at: &request.request_started_at,
                first_token_at: request.outcome.first_token_at.as_deref(),
                completed_at: Some(&request.outcome.completed_at),
                input_tokens: request.outcome.input_tokens,
                output_tokens: request.outcome.output_tokens,
                cache_read_tokens: request.outcome.cache_read_tokens,
                cache_write_tokens: request.outcome.cache_write_tokens,
                first_token_latency_ms: request.outcome.first_token_latency_ms,
                total_latency_ms: Some(request.outcome.total_latency_ms),
                status_code: request.outcome.status_code,
                final_state: request.outcome.final_state,
                request_body_json: api_audit_detail_json(&request.request_body_json, save_details),
                response_body_json: request
                    .outcome
                    .response_body_json
                    .as_deref()
                    .and_then(|value| api_audit_detail_json(value, save_details)),
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
            messages[*message_index].role == NeutralChatRole::System
                || prompt_context_source_is_required(&message_context_sources[*message_index])
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
        | PromptContextSource::AgentCurrentTask { sequence }
        | PromptContextSource::TurnMemory { sequence } => {
            Some(PromptContextGroupKey::MessageSequence(*sequence))
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
        PromptContextSourceBucket::TodoGraph,
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
        PromptContextSourceBucket::RuntimeToolState,
        PromptContextSourceBucket::RuntimeToolStateSnapshot,
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
        let entry = by_source
            .iter_mut()
            .find(|entry| entry.source == group.source_bucket)
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

fn context_group_is_compressible(group: &ContextMessageGroup) -> bool {
    group.estimated_tokens > 0
        && matches!(
            group.source_bucket,
            PromptContextSourceBucket::PersistedHistory
                | PromptContextSourceBucket::AgentPrivateContext
                | PromptContextSourceBucket::TurnMemory
                | PromptContextSourceBucket::RuntimeToolState
                | PromptContextSourceBucket::CompressionSnapshot
        )
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
        PromptContextSourceBucket::RuntimeToolState => "runtimeToolState",
        PromptContextSourceBucket::RuntimeToolStateSnapshot => "runtimeToolStateSnapshot",
    }
}

pub(crate) fn context_usage_response(
    context: &PreparedPromptContext,
    latest_response_usage: &NeutralUsage,
) -> Result<ContextUsageResponse, ApiError> {
    let message_groups = context_message_groups(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
        context.active_tool_start_index,
    )?;
    let pack_items = pack_items_from_message_groups(&message_groups);
    let used_message_tokens =
        context_used_message_tokens_from_response_usage(latest_response_usage)?;
    let available_message_tokens = context.context_budget.context_window;
    let compression_trigger_tokens = context
        .context_budget
        .system_prompt_tokens
        .saturating_add(context.context_budget.tool_schema_tokens)
        .saturating_add(context_compression_trigger_tokens(
            context.context_budget.available_message_tokens,
        ));
    let usage_percent = percentage_ceil(used_message_tokens, available_message_tokens);
    let compression_trigger_percent =
        percentage_ceil(compression_trigger_tokens, available_message_tokens);
    let token_breakdown = context_token_breakdown(&message_groups);
    let will_compress_on_next_send = plan_context_compression(
        &pack_items,
        available_message_tokens,
        active_tool_start_group_index(&message_groups, context.active_tool_start_index),
        CONTEXT_COMPRESSION_PRESERVE_RECENT_MESSAGES,
    )
    .is_some();

    Ok(ContextUsageResponse {
        used_message_tokens,
        available_message_tokens,
        memory_context_tokens: context.memory_context_tokens,
        memory_budget_tokens: context.memory_budget_tokens,
        usage_percent,
        compression_trigger_tokens,
        compression_trigger_percent,
        will_compress_on_next_send,
        token_breakdown,
    })
}

fn context_used_message_tokens_from_response_usage(usage: &NeutralUsage) -> Result<u64, ApiError> {
    let input_tokens = usage
        .input_tokens
        .ok_or_else(|| ApiError::bad_request("latestResponseUsage.inputTokens is required"))?;
    let input_tokens =
        non_negative_context_usage_token(input_tokens, "latestResponseUsage.inputTokens")?;
    let output_tokens = usage
        .output_tokens
        .ok_or_else(|| ApiError::bad_request("latestResponseUsage.outputTokens is required"))?;
    let output_tokens =
        non_negative_context_usage_token(output_tokens, "latestResponseUsage.outputTokens")?;
    Ok(input_tokens.saturating_add(output_tokens))
}

fn non_negative_context_usage_token(value: i64, field_name: &str) -> Result<u64, ApiError> {
    if value < 0 {
        return Err(ApiError::bad_request(format!(
            "{field_name} must be greater than or equal to 0"
        )));
    }

    Ok(value as u64)
}

fn percentage_ceil(value: u64, total: u64) -> u64 {
    if total == 0 {
        0
    } else {
        value.saturating_mul(100).div_ceil(total)
    }
}

fn active_tool_start_group_index(
    groups: &[ContextMessageGroup],
    active_tool_start_index: usize,
) -> usize {
    groups
        .iter()
        .position(|group| {
            group
                .message_indices
                .iter()
                .any(|message_index| *message_index >= active_tool_start_index)
        })
        .unwrap_or(groups.len())
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
