use std::{
    collections::{BTreeMap, HashSet},
    convert::Infallible,
    time::Duration,
};

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use foco_store::{
    memory::MemoryDatabase,
    workspace::{
        LlmRequestAuditFilters, LlmRequestAuditModelBreakdown, LlmRequestAuditProviderBreakdown,
        LlmRequestAuditSummaryRow, LlmRequestAuditTrendPoint, TodoGraphFilter, WorkspaceDatabase,
        workspace_database_path,
    },
};
use tokio::sync::mpsc;

use crate::*;

pub(crate) async fn queue_chat_message(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<QueueChatMessageRequest>,
) -> Result<Json<QueueChatMessageResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let requested_model_id = request.model_id.clone();
    let requested_provider_id = request.provider_id.clone();
    let requested_thinking_level = request.thinking_level.clone();
    let requested_skill_ids = request.skill_ids.clone().unwrap_or_default();
    let prompt_context = prepare_prompt_context(
        &state,
        &config,
        &workspace_id,
        request.into_prompt_request(),
        None,
        PromptAssemblyPurpose::ChatRun,
    )
    .await?;
    let raw_message = prompt_context.raw_message.as_deref().unwrap_or("");
    let message = prompt_context
        .message
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("message must not be empty"))?;
    let mut database = WorkspaceDatabase::open_or_create(&prompt_context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let user_message_id = unique_id("msg-user");
    let user_metadata_json = queued_user_message_metadata_json(
        &prompt_context.attachments,
        &requested_model_id,
        requested_provider_id.as_deref(),
        requested_thinking_level.as_deref(),
        &requested_skill_ids,
    )?;

    let (chat_id, chat_title) = if prompt_context.is_new_chat {
        let chat_id = unique_id("chat");
        let title = chat_title_for_prompt(raw_message, &prompt_context.attachments);
        let chat_metadata_json = queued_chat_metadata_json(
            &user_message_id,
            &requested_model_id,
            requested_provider_id.as_deref(),
            requested_thinking_level.as_deref(),
            &requested_skill_ids,
            message,
        )?;
        database
            .insert_chat_with_metadata(&chat_id, &title, &chat_metadata_json)
            .map_err(ApiError::from_workspace_error)?;
        (chat_id, title)
    } else {
        let chat_id = prompt_context
            .chat_id
            .clone()
            .ok_or_else(|| ApiError::bad_request("chat id must not be empty"))?;
        let chat = database
            .chat(&chat_id)
            .map_err(ApiError::from_workspace_error)?
            .ok_or_else(|| ApiError::bad_request(format!("chat was not found: {chat_id}")))?;
        (chat_id, chat.title)
    };

    database
        .insert_message(NewMessage {
            id: &user_message_id,
            chat_id: &chat_id,
            role: "user",
            content: message,
            sequence: prompt_context.next_message_sequence,
            metadata_json: Some(&user_metadata_json),
        })
        .map_err(ApiError::from_workspace_error)?;
    let chat = database
        .chat(&chat_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("chat was not found: {chat_id}")))?;

    Ok(Json(QueueChatMessageResponse {
        chat_id,
        chat_title,
        created_at: chat.created_at,
        updated_at: chat.updated_at,
        user_message_id,
        content: message.to_string(),
        parts: user_message_response_parts(message, &prompt_context.attachments),
    }))
}

pub(crate) async fn stream_chat_response(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<ChatStreamRequest>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let config = config_snapshot(&state)?;
    let chat_context = prepare_chat_context(&state, &config, &workspace_id, request).await?;
    let run_id = chat_context.llm_request_id.clone();
    let (guidance_tx, guidance_rx) = mpsc::unbounded_channel();
    let active_run_registration = state.active_chat_runs.register(
        run_id.clone(),
        chat_context.workspace_id.clone(),
        chat_context.chat_id.clone(),
        chat_context.assistant_message_id.clone(),
        chat_context.assistant_sequence,
        chat_context.memories_used.clone(),
        guidance_tx,
    )?;
    let subscription = state
        .active_chat_runs
        .subscribe(&workspace_id, &run_id, Some(-1))?;

    tokio::spawn(run_chat_context_in_background(
        chat_context,
        active_run_registration,
        guidance_rx,
    ));

    Ok(chat_run_sse(subscription))
}

pub(crate) async fn subscribe_chat_run(
    State(state): State<AppState>,
    AxumPath((workspace_id, run_id)): AxumPath<(String, String)>,
    Query(query): Query<ChatRunStreamQuery>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let subscription =
        state
            .active_chat_runs
            .subscribe(&workspace_id, &run_id, query.after_sequence)?;

    Ok(chat_run_sse(subscription))
}

pub(crate) async fn cancel_chat_run(
    State(state): State<AppState>,
    AxumPath((workspace_id, run_id)): AxumPath<(String, String)>,
) -> Result<Json<CancelChatRunResponse>, ApiError> {
    state.active_chat_runs.cancel(&workspace_id, &run_id)?;

    Ok(Json(CancelChatRunResponse { ok: true, run_id }))
}

pub(crate) fn chat_run_sse(
    subscription: ActiveChatRunSubscription,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    Sse::new(chat_run_subscription_stream(subscription)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    )
}

pub(crate) async fn add_chat_guidance(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<ChatGuidanceRequest>,
) -> Result<Json<ChatGuidanceResponse>, ApiError> {
    let guidance = state
        .active_chat_runs
        .push_guidance(&workspace_id, request)?;

    Ok(Json(ChatGuidanceResponse {
        id: guidance.id,
        content: guidance.content,
        parts: user_guidance_message_parts(&guidance.attachments),
    }))
}

pub(crate) async fn context_usage(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<ContextUsageRequest>,
) -> Result<Json<ContextUsageResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let latest_response_usage = request.latest_response_usage.clone();
    let prompt_context = prepare_prompt_context(
        &state,
        &config,
        &workspace_id,
        request.into_prompt_request(),
        None,
        PromptAssemblyPurpose::ContextPreview,
    )
    .await?;

    Ok(Json(context_usage_response(
        &prompt_context,
        latest_response_usage.as_ref(),
    )?))
}

pub(crate) async fn answer_question(
    State(state): State<AppState>,
    AxumPath(question_id): AxumPath<String>,
    Json(answer): Json<QuestionAnswer>,
) -> Result<Json<QuestionAnswerResponse>, ApiError> {
    let question_id = question_id.trim().to_string();

    state.question_registry.answer(&question_id, answer)?;

    Ok(Json(QuestionAnswerResponse {
        ok: true,
        question_id,
    }))
}

pub(crate) async fn ai_statistics(
    State(state): State<AppState>,
    Query(query): Query<AiStatisticsQuery>,
) -> Result<Json<AiStatisticsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let filters = normalized_ai_statistics_query(query)?;
    let workspaces = ai_statistics_workspaces(&config, filters.workspace_id.as_deref())?;
    let mut requests = Vec::new();
    let mut merged_summary: Option<LlmRequestAuditSummaryRow> = None;
    let mut merged_trend: BTreeMap<String, LlmRequestAuditTrendPoint> = BTreeMap::new();
    let mut merged_models: BTreeMap<String, LlmRequestAuditModelBreakdown> = BTreeMap::new();
    let mut merged_providers: BTreeMap<String, LlmRequestAuditProviderBreakdown> = BTreeMap::new();
    let mut total_count = 0_i64;
    let page_limit = filters
        .offset
        .checked_add(filters.page_size)
        .ok_or_else(|| ApiError::bad_request("AI statistics page limit is too large"))?;

    for workspace in workspaces {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        let chat_titles = chat_title_map(&database)?;
        let audit_filters = LlmRequestAuditFilters {
            workspace_id: None,
            chat_id: filters.chat_id.as_deref(),
            provider_id: filters.provider_id.as_deref(),
            model_id: filters.model_id.as_deref(),
            final_state: filters.status.as_deref(),
            started_after: filters.started_after.as_deref(),
            started_before: filters.started_before.as_deref(),
            limit: Some(page_limit),
            offset: Some(0),
        };

        let workspace_count = database
            .llm_request_audit_count(audit_filters)
            .map_err(ApiError::from_workspace_error)?;
        total_count += workspace_count;
        if workspace_count > 0 {
            let workspace_summary = database
                .llm_request_audit_summary(audit_filters)
                .map_err(ApiError::from_workspace_error)?;
            merge_llm_request_audit_summary(&mut merged_summary, &workspace_summary);
            let trend = database
                .llm_request_audit_trend_breakdown(audit_filters)
                .map_err(ApiError::from_workspace_error)?;
            for point in trend {
                merged_trend
                    .entry(point.bucket.clone())
                    .and_modify(|entry| {
                        entry.request_count += point.request_count;
                        entry.total_tokens += point.total_tokens;
                    })
                    .or_insert(point);
            }
            let model_rows = database
                .llm_request_audit_model_breakdown(audit_filters)
                .map_err(ApiError::from_workspace_error)?;
            for row in model_rows {
                merged_models
                    .entry(row.model_id.clone())
                    .and_modify(|entry| {
                        entry.request_count += row.request_count;
                        entry.total_tokens += row.total_tokens;
                    })
                    .or_insert(row);
            }
            let provider_rows = database
                .llm_request_audit_provider_breakdown(audit_filters)
                .map_err(ApiError::from_workspace_error)?;
            for row in provider_rows {
                merged_providers
                    .entry(row.provider_id.clone())
                    .and_modify(|entry| {
                        entry.request_count += row.request_count;
                        entry.success_count += row.success_count;
                        entry.total_tokens += row.total_tokens;
                        entry.latency_count += row.latency_count;
                        entry.latency_sum += row.latency_sum;
                    })
                    .or_insert(row);
            }
        }
        let rows = database
            .llm_request_audit_rows(audit_filters)
            .map_err(ApiError::from_workspace_error)?;

        requests.extend(
            rows.into_iter()
                .map(|row| ai_request_audit_summary(row, workspace, &chat_titles)),
        );
    }

    requests.sort_by(|left, right| {
        right
            .request_started_at
            .cmp(&left.request_started_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    let start = usize::try_from(filters.offset).expect("non-negative offset fits usize");
    let page_size = usize::try_from(filters.page_size).expect("positive page size fits usize");
    let requests = requests.into_iter().skip(start).take(page_size).collect();
    let total_pages = if total_count == 0 {
        0
    } else {
        (total_count + filters.page_size - 1) / filters.page_size
    };

    let summary = ai_statistics_summary_from_aggregates(
        merged_summary,
        merged_trend,
        merged_models,
        merged_providers,
    );

    Ok(Json(AiStatisticsResponse {
        page: filters.page,
        page_size: filters.page_size,
        requests,
        summary,
        total_count,
        total_pages,
    }))
}

pub(crate) async fn ai_statistics_detail(
    State(state): State<AppState>,
    AxumPath((workspace_id, request_id)): AxumPath<(String, String)>,
) -> Result<Json<AiRequestDetailResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let request_id = request_id.trim();

    if request_id.is_empty() {
        return Err(ApiError::bad_request("request id must not be empty"));
    }

    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let chat_titles = chat_title_map(&database)?;
    let request = database
        .llm_request(request_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("LLM request was not found: {request_id}")))?;
    let events = database
        .llm_request_events(request_id)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(ai_request_audit_event_summary)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(AiRequestDetailResponse {
        request: ai_request_audit_detail(request, workspace, &chat_titles)?,
        events,
    }))
}

pub(crate) async fn chat_messages(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
) -> Result<Json<ChatMessagesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = workspace_id.trim();
    let chat_id = chat_id.trim();
    let workspace = config
        .workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| ApiError::bad_request(format!("workspace was not found: {workspace_id}")))?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;

    if database
        .chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
        .is_none()
    {
        return Err(ApiError::bad_request(format!(
            "chat was not found: {chat_id}"
        )));
    }

    let llm_request_events = database
        .llm_request_events_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let mut messages = Vec::new();
    for message in database
        .messages_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
    {
        if message.role != "user" && message.role != "assistant" {
            continue;
        }

        messages.push(chat_message_summary(
            &database,
            &workspace.path,
            Some(&state.memory_database_file),
            message,
            &llm_request_events,
        )?);
    }

    let active_run = state
        .active_chat_runs
        .active_run_for_chat(workspace_id, chat_id)?;

    Ok(Json(ChatMessagesResponse {
        messages,
        active_run,
    }))
}

pub(crate) async fn chat_todo_graph(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
    Query(query): Query<TodoGraphQuery>,
) -> Result<Json<TodoGraphResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = workspace_id.trim();
    let chat_id = chat_id.trim();
    let workspace = workspace_by_id(&config, workspace_id)?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;

    if database
        .chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
        .is_none()
    {
        return Err(ApiError::bad_request(format!(
            "chat was not found: {chat_id}"
        )));
    }

    let status = optional_trimmed_string(query.status);
    let task_id = optional_trimmed_string(query.task_id);
    let graph = database
        .filtered_todo_graph(
            chat_id,
            TodoGraphFilter {
                status: status.as_deref(),
                task_id: task_id.as_deref(),
                include_subtasks: query.include_subtasks.unwrap_or(true),
            },
        )
        .map_err(ApiError::from_workspace_error)?;

    Ok(Json(todo_graph_response(chat_id, graph)))
}

pub(crate) async fn chat_statistics(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
) -> Result<Json<ChatStatisticsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = workspace_id.trim();
    let chat_id = chat_id.trim();
    let workspace = workspace_by_id(&config, workspace_id)?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;

    if database
        .chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
        .is_none()
    {
        return Err(ApiError::bad_request(format!(
            "chat was not found: {chat_id}"
        )));
    }

    let messages = database
        .messages_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let llm_rows = database
        .llm_request_audit_count(LlmRequestAuditFilters {
            chat_id: Some(chat_id),
            ..LlmRequestAuditFilters::default()
        })
        .and_then(|request_count| {
            database.llm_request_audit_rows(LlmRequestAuditFilters {
                chat_id: Some(chat_id),
                limit: Some(request_count),
                offset: Some(0),
                ..LlmRequestAuditFilters::default()
            })
        })
        .map_err(ApiError::from_workspace_error)?;
    let prompt_injections = database
        .prompt_context_injections_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let compression_snapshots = database
        .context_compression_snapshots_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let code_change_stats = database
        .code_change_stats_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let tool_breakdown = database
        .tool_call_counts_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(chat_tool_breakdown)
        .collect();
    let created_workspace_memories =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace.path))
            .map_err(ApiError::from_memory_error)?
            .facts_created_from_chat_sources(chat_id)
            .map_err(ApiError::from_memory_error)?
            .len() as i64;
    let run_ids = llm_rows
        .iter()
        .map(|row| row.id.clone())
        .collect::<HashSet<_>>();
    let created_global_memories =
        MemoryDatabase::open_or_create_global_at(&state.memory_database_file)
            .map_err(ApiError::from_memory_error)?
            .facts_created_from_source_run_ids(&run_ids)
            .map_err(ApiError::from_memory_error)?
            .len() as i64;

    Ok(Json(chat_statistics_response(
        workspace_id,
        chat_id,
        messages,
        llm_rows,
        prompt_injections,
        compression_snapshots,
        code_change_stats,
        tool_breakdown,
        created_workspace_memories + created_global_memories,
    )?))
}

pub(crate) async fn delete_chat(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = workspace_id.trim();
    let chat_id = chat_id.trim();
    let workspace = workspace_by_id(&config, workspace_id)?;

    if chat_id.is_empty() {
        return Err(ApiError::bad_request("chat id must not be empty"));
    }

    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;

    if !database
        .delete_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
    {
        return Err(ApiError::bad_request(format!(
            "chat was not found: {chat_id}"
        )));
    }

    workspace_response_from_config(&config, &state.active_chat_runs)
}
