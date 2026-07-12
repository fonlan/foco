use std::collections::HashSet;

use foco_agent::{ContextPackItem, context_compression_trigger_tokens, estimate_text_tokens};
use foco_providers::{
    NeutralChatMessage, NeutralChatRole, NeutralToolCall, stream_chat_with_capture_observer,
};
use foco_store::workspace::{
    ContextCompressionSnapshotRecord, NewPlanPhaseDerivedEffects, ToolCallWithResultRecord,
};
use serde_json::{Value, json};

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

pub(crate) async fn ensure_context_compression(
    context: &mut PreparedChatContext,
) -> Result<ContextCompressionResult, ApiError> {
    validate_prompt_context_lengths(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
    )?;

    let mut events = Vec::new();
    let mut runtime_tool_state_compressed =
        compress_runtime_tool_state_with_events_if_needed(context, false, &mut events)?;

    let mut message_groups = context_message_groups(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
        context.active_tool_start_index,
    )?;
    let segments = context_usage_segments(&context.context_budget, &message_groups);
    let total_used_context_tokens = context_usage_segments_total(&segments);
    if total_used_context_tokens
        >= llm_context_compression_trigger_tokens(context.context_budget.context_window)
        && ensure_llm_context_compression(
            context,
            &message_groups,
            &mut events,
            LlmContextCompressionMode::Normal,
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
        if !runtime_tool_state_compressed {
            runtime_tool_state_compressed |=
                compress_runtime_tool_state_with_events_if_needed(context, true, &mut events)?;
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
                LlmContextCompressionMode::RequiredOverflow,
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
        snapshot_id,
        original_token_count,
        summary_token_count,
        started_at,
        completed_at,
        provider_id: context.provider_id.clone(),
        model_id: context.model_id.clone(),
    }
}

fn compress_runtime_tool_state_with_events_if_needed(
    context: &mut PreparedChatContext,
    force: bool,
    events: &mut Vec<ContextCompressionEventDetail>,
) -> Result<bool, ApiError> {
    let compression_started_at = utc_timestamp();
    let compressed = compress_runtime_tool_state_if_needed(context, force)?;
    if !compressed {
        return Ok(false);
    }

    events.push(context_compression_event_detail(
        "start",
        CONTEXT_COMPRESSION_KIND_RUNTIME_TOOL_STATE,
        None,
        None,
        None,
        Some(compression_started_at.clone()),
        None,
        context,
    ));
    events.push(context_compression_event_detail(
        "completed",
        CONTEXT_COMPRESSION_KIND_RUNTIME_TOOL_STATE,
        None,
        None,
        None,
        Some(compression_started_at),
        Some(utc_timestamp()),
        context,
    ));
    Ok(true)
}

#[derive(Clone, Copy)]
pub(crate) enum LlmContextCompressionMode {
    Normal,
    RequiredOverflow,
}

async fn ensure_llm_context_compression(
    context: &mut PreparedChatContext,
    message_groups: &[ContextMessageGroup],
    events: &mut Vec<ContextCompressionEventDetail>,
    mode: LlmContextCompressionMode,
) -> Result<bool, ApiError> {
    let covered_group_indices = llm_context_compression_group_indices(
        message_groups,
        context.context_budget.available_message_tokens,
        mode,
    );
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
    let compression_started_at = utc_timestamp();
    events.push(context_compression_event_detail(
        "start",
        CONTEXT_COMPRESSION_KIND_LLM,
        None,
        Some(i64::try_from(original_tokens).map_err(|_| {
            ApiError::internal("context compression original token count exceeds i64")
        })?),
        None,
        Some(compression_started_at.clone()),
        None,
        context,
    ));

    let source_summary = context_compression_summary_allowing_snapshots(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &covered_indices,
    )?;
    let summary = llm_context_compression_summary(context, &source_summary).await?;
    let summary_token_count = estimate_text_tokens(&summary);
    if summary_token_count >= original_tokens {
        events.pop();
        return Ok(false);
    }

    let covered_snapshot_ids = compression_covered_snapshot_ids(
        &context.provider_request.messages,
        &context.message_context_sources,
        &covered_indices,
    );
    let covered_sequences = compression_covered_sequences_allowing_snapshots(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.compression_snapshots,
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
        events.pop();
        return Ok(false);
    }

    let snapshot = persist_context_compression_snapshot(
        context,
        &covered_indices,
        summary,
        original_tokens,
        summary_token_count,
        CONTEXT_COMPRESSION_KIND_LLM,
        json!({
            "kind": CONTEXT_COMPRESSION_KIND_LLM,
            "coveredSequences": covered_sequences,
            "coveredSnapshotIds": covered_snapshot_ids,
            "supersededSnapshotIds": covered_snapshot_ids,
            "triggerTokens": llm_context_compression_trigger_tokens(context.context_budget.context_window),
            "availableMessageTokens": context.context_budget.available_message_tokens
        }),
    )?;
    events.push(context_compression_event_detail(
        "completed",
        CONTEXT_COMPRESSION_KIND_LLM,
        Some(snapshot.id.clone()),
        Some(snapshot.original_token_count),
        Some(snapshot.summary_token_count),
        Some(compression_started_at),
        Some(utc_timestamp()),
        context,
    ));

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

pub(crate) fn llm_context_compression_group_indices(
    groups: &[ContextMessageGroup],
    available_message_tokens: u64,
    mode: LlmContextCompressionMode,
) -> Vec<usize> {
    let compressible_indices = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| {
            group.estimated_tokens > 0
                && matches!(
                    group.source_bucket,
                    PromptContextSourceBucket::CompressionSnapshot
                        | PromptContextSourceBucket::PersistedHistory
                        | PromptContextSourceBucket::TurnMemory
                        | PromptContextSourceBucket::RuntimeToolStateSnapshot
                )
                && (!group.must_keep
                    || (matches!(mode, LlmContextCompressionMode::RequiredOverflow)
                        && group.source_bucket
                            == PromptContextSourceBucket::RuntimeToolStateSnapshot))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if matches!(mode, LlmContextCompressionMode::RequiredOverflow) {
        return compressible_indices;
    }

    let normally_covered_count = compressible_indices
        .len()
        .saturating_sub(CONTEXT_COMPRESSION_PRESERVE_RECENT_MESSAGES);
    let mut covered_indices = compressible_indices
        .iter()
        .copied()
        .take(normally_covered_count)
        .collect::<Vec<_>>();

    let pack_items = pack_items_from_message_groups(groups);
    if let Ok(packed) = pack_context(&pack_items, available_message_tokens) {
        let selected_indices = packed.selected_indices.into_iter().collect::<HashSet<_>>();
        covered_indices.extend(
            compressible_indices
                .into_iter()
                .skip(normally_covered_count)
                .filter(|index| !selected_indices.contains(index)),
        );
    }

    covered_indices
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
        Duration::from_millis(LLM_CONTEXT_COMPRESSION_TIMEOUT_MS),
        stream_chat_with_capture_observer(
            &context.provider_config,
            request,
            capture_details,
            observer,
        ),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        Ok(Err(source)) => {
            capture.persist_request_failure(&source)?;
            let message = source.to_string();
            let request_body_json = capture.captured_request_json()?.unwrap_or_default();
            let response_body_json =
                capture.failed_response_json(message.clone(), source.status_code(), false)?;
            context.captured_llm_requests.push(CapturedLlmRequest {
                id: request_id,
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
            });
            return Err(ApiError::internal(message));
        }
        Err(_) => {
            let message = format!(
                "context compression summary timed out after {LLM_CONTEXT_COMPRESSION_TIMEOUT_MS} ms"
            );
            let request_body_json = capture.captured_request_json()?.unwrap_or_default();
            let response_body_json = capture.failed_response_json(message.clone(), None, false)?;
            context.captured_llm_requests.push(CapturedLlmRequest {
                id: request_id,
                request_kind: "contextCompression",
                request_started_at,
                request_body_json,
                events,
                outcome: ChatAuditOutcome {
                    response_body_json,
                    ..failed_provider_audit_outcome(started_at, &message, None)
                },
            });
            return Err(ApiError::internal(message));
        }
    };
    let mut output_text = String::new();
    let mut final_usage = None;
    let mut first_token_at = None;
    let mut first_token_latency_ms = None;

    loop {
        let event_result = match timeout(
            Duration::from_millis(LLM_CONTEXT_COMPRESSION_TIMEOUT_MS),
            stream.next_event(),
        )
        .await
        {
            Ok(event_result) => event_result,
            Err(_) => {
                let message = format!(
                    "context compression summary timed out after {LLM_CONTEXT_COMPRESSION_TIMEOUT_MS} ms"
                );
                let request_body_json = capture.captured_request_json()?.unwrap_or_default();
                let response_body_json =
                    capture.failed_response_json(message.clone(), None, true)?;
                context.captured_llm_requests.push(CapturedLlmRequest {
                    id: request_id,
                    request_kind: "contextCompression",
                    request_started_at,
                    request_body_json,
                    events,
                    outcome: ChatAuditOutcome {
                        response_body_json,
                        ..failed_provider_audit_outcome(started_at, &message, None)
                    },
                });
                return Err(ApiError::internal(message));
            }
        };
        let Some(event_result) = event_result else {
            let message = "context compression summary stream ended without a completion event";
            let request_body_json = capture.captured_request_json()?.unwrap_or_default();
            let response_body_json = capture.failed_response_json(message, None, true)?;
            context.captured_llm_requests.push(CapturedLlmRequest {
                id: request_id,
                request_kind: "contextCompression",
                request_started_at,
                request_body_json,
                events,
                outcome: ChatAuditOutcome {
                    response_body_json,
                    ..failed_provider_audit_outcome(started_at, message, None)
                },
            });
            return Err(ApiError::internal(message));
        };
        let event = match event_result {
            Ok(event) => event,
            Err(source) => {
                let message = source.to_string();
                let request_body_json = capture.captured_request_json()?.unwrap_or_default();
                let response_body_json =
                    capture
                        .response_json(stream.final_response_dump())?
                        .or(capture.failed_response_json(
                            message.clone(),
                            source.status_code(),
                            true,
                        )?);
                context.captured_llm_requests.push(CapturedLlmRequest {
                    id: request_id,
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
                });
                return Err(ApiError::internal(message));
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
                let request_body_json = capture.captured_request_json()?.unwrap_or_default();
                let response_body_json =
                    capture.failed_response_json(message.clone(), None, true)?;
                context.captured_llm_requests.push(CapturedLlmRequest {
                    id: request_id,
                    request_kind: "contextCompression",
                    request_started_at,
                    request_body_json,
                    events,
                    outcome: ChatAuditOutcome {
                        response_body_json,
                        ..failed_provider_audit_outcome(started_at, &message, None)
                    },
                });
                return Err(ApiError::internal(message));
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
                let request_body_json = capture.captured_request_json()?.unwrap_or_default();
                let response_body_json = capture
                    .response_json(stream.final_response_dump())?
                    .or(capture.failed_response_json(message.clone(), None, true)?);
                context.captured_llm_requests.push(CapturedLlmRequest {
                    id: request_id,
                    request_kind: "contextCompression",
                    request_started_at,
                    request_body_json,
                    events,
                    outcome: ChatAuditOutcome {
                        response_body_json,
                        ..failed_provider_audit_outcome(started_at, &message, None)
                    },
                });
                return Err(ApiError::internal(message));
            }
        }
    }

    let summary = output_text.trim().to_string();
    if summary.is_empty() {
        let message = "context compression summary returned empty text";
        let request_body_json = capture.captured_request_json()?.unwrap_or_default();
        let response_body_json = capture.failed_response_json(message, None, false)?;
        context.captured_llm_requests.push(CapturedLlmRequest {
            id: request_id,
            request_kind: "contextCompression",
            request_started_at,
            request_body_json,
            events,
            outcome: ChatAuditOutcome {
                response_body_json,
                ..failed_provider_audit_outcome(started_at, message, None)
            },
        });
        return Err(ApiError::internal(message));
    }
    let request_body_json = capture.captured_request_json()?.unwrap_or_default();
    let response_body_json = capture.response_json(stream.final_response_dump())?;
    context.captured_llm_requests.push(CapturedLlmRequest {
        id: request_id,
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
    mut metadata: Value,
) -> Result<ContextCompressionSnapshotRecord, ApiError> {
    let snapshot_id = unique_id("ctx");
    let snapshot_sequence = next_context_snapshot_sequence(&context.compression_snapshots)?;
    let original_token_count = i64::try_from(original_tokens)
        .map_err(|_| ApiError::internal("context compression original token count exceeds i64"))?;
    let summary_token_count_i64 = i64::try_from(summary_token_count)
        .map_err(|_| ApiError::internal("context compression summary token count exceeds i64"))?;
    let (source_message_start_sequence, source_message_end_sequence) =
        compression_source_sequence_range(&context.message_source_sequences, covered_indices);

    let mut snapshot = ContextCompressionSnapshotRecord {
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
        metadata_json: String::new(),
    };
    let replaced_messages = replace_covered_messages_with_snapshot(
        &context.provider_request.messages,
        covered_indices,
        compression_snapshot_message(&snapshot),
    );
    let replaced_sequences =
        replace_covered_sequences_with_snapshot(&context.message_source_sequences, covered_indices);
    let replaced_sources = replace_covered_sources_with_snapshot(
        &context.message_context_sources,
        covered_indices,
        PromptContextSource::CompressionSnapshot,
    );
    let replaced_active_tool_start_index =
        compressed_active_tool_start_index(context.active_tool_start_index, covered_indices);
    metadata["contextUsage"] = post_compression_context_usage_metadata(
        context,
        &replaced_messages,
        &replaced_sequences,
        &replaced_sources,
        replaced_active_tool_start_index,
    )?;
    let metadata_json = metadata.to_string();
    snapshot.metadata_json = metadata_json.clone();

    let mut database = WorkspaceDatabase::open_or_create(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    database
        .insert_context_compression_snapshot(NewContextCompressionSnapshot {
            id: &snapshot.id,
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

    context.provider_request.messages = replaced_messages;
    context.message_source_sequences = replaced_sequences;
    context.message_context_sources = replaced_sources;
    context.active_tool_start_index = replaced_active_tool_start_index;
    context.compression_snapshots.push(snapshot.clone());

    tracing::debug!(kind = kind, "created context compression snapshot");
    Ok(snapshot)
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
    if !force && !compression_enabled {
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
    neutral_text_message(
        NeutralChatRole::System,
        format!(
            "## Context Compression Snapshot\n\n\
             Source: {CONTEXT_COMPRESSION_PROMPT_PREFIX}\n\n\
             Snapshot ID: `{}`\n\n\
             Source message sequence range: {}-{}\n\n\
             Original tokens: {}\n\n\
             Summary tokens: {}\n\n\
             ### Summary\n\n\
             {}",
            snapshot.id,
            snapshot.source_message_start_sequence,
            snapshot.source_message_end_sequence,
            snapshot.original_token_count,
            snapshot.summary_token_count,
            snapshot.summary
        ),
    )
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
        let Some(snapshot_id) = compression_snapshot_id_from_message(message) else {
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
                .and_then(compression_snapshot_id_from_message)
        })
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn compression_snapshot_id_from_message(message: &NeutralChatMessage) -> Option<String> {
    if let Some(start) = message.content.find("<snapshot_id>") {
        let start = start + "<snapshot_id>".len();
        let end = message.content[start..].find("</snapshot_id>")? + start;
        return Some(message.content[start..end].trim().to_string());
    }

    message
        .content
        .lines()
        .find_map(|line| line.trim().strip_prefix("Snapshot ID: `"))
        .and_then(|rest| rest.split('`').next())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
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
    let mut database = WorkspaceDatabase::open_or_create(&context.workspace_path)
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
        for llm_request in &context.captured_llm_requests {
            persist_llm_request(&mut database, context, llm_request)?;
            if !current_history_run {
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

    if context.agent_primary_chat_output
        && let Some(queued_user_message_id) = &context.queued_user_message_id
    {
        database
            .clear_chat_queued_run(&context.chat_id, queued_user_message_id)
            .map_err(ApiError::from_workspace_error)?;
    }

    if context.agent_primary_chat_output {
        queue_chat_derived_effects(&mut database, context, final_state)?;
    }

    Ok(())
}

fn queue_chat_derived_effects(
    database: &mut WorkspaceDatabase,
    context: &PreparedChatContext,
    final_state: &str,
) -> Result<(), ApiError> {
    if final_state != "succeeded" {
        return Ok(());
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
        return Ok(());
    }

    queue_memory_extraction_job(context, final_state)?;
    crate::spec_runtime::queue_workspace_spec_update_job(context, final_state)
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
    if context.agent_primary_chat_output
        && let Some(queued_user_message_id) = context.queued_user_message_id.as_deref()
        && !queued_chat_run_matches_context(&database, context, queued_user_message_id)?
    {
        return Err(ApiError::conflict(
            "chat run is no longer current because its queued run was replaced",
        ));
    }
    database
        .insert_llm_request(NewLlmRequest {
            id: request_id,
            workspace_id: &context.workspace_id,
            chat_id: database
                .chat(&context.chat_id)
                .map_err(ApiError::from_workspace_error)?
                .is_some()
                .then_some(context.chat_id.as_str()),
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
        PromptContextSourceBucket::ToolCalls => "toolCalls",
        PromptContextSourceBucket::RuntimeToolState => "runtimeToolState",
        PromptContextSourceBucket::RuntimeToolStateSnapshot => "runtimeToolStateSnapshot",
    }
}

fn post_compression_context_usage_metadata(
    context: &PreparedChatContext,
    messages: &[NeutralChatMessage],
    source_sequences: &[Option<i64>],
    sources: &[PromptContextSource],
    active_tool_start_index: usize,
) -> Result<Value, ApiError> {
    let message_groups =
        context_message_groups(messages, source_sequences, sources, active_tool_start_index)?;
    let segments = context_usage_segments(&context.context_budget, &message_groups);
    Ok(json!({
        "contextWindow": context.context_budget.context_window,
        "maxOutputTokens": context.context_budget.max_output_tokens,
        "triggerTokens": context_window_compression_trigger_tokens(context.context_budget.context_window),
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

pub(crate) fn context_usage_response(
    context: &PreparedPromptContext,
) -> Result<ContextUsageResponse, ApiError> {
    let message_groups = context_message_groups(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &context.message_context_sources,
        context.active_tool_start_index,
    )?;
    let assembled_message_tokens = message_groups
        .iter()
        .map(|group| group.estimated_tokens)
        .sum::<u64>();
    let available_message_tokens = context.context_budget.available_message_tokens;
    let context_window = context.context_budget.context_window;
    let max_output_tokens = context.context_budget.max_output_tokens;
    let assembled_segments = context_usage_segments(&context.context_budget, &message_groups);
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
    let segments = context_usage_segments(&context.context_budget, &packed_groups);
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
