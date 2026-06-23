use super::{
    compression_snapshot_message, neutral_message_estimated_tokens, snapshot_covered_sequences,
};
use crate::memory_runtime::{
    memory_retrieval_query_text, neutral_messages_from_record,
    stored_turn_memory_messages_by_sequence,
};
use crate::runtime::web_search_enabled;
use crate::*;
use foco_store::memory::MEMORY_DREAM_TRANSCRIPT_CHAT_KIND;

pub(crate) async fn prepare_prompt_context(
    state: &AppState,
    config: &GlobalConfig,
    workspace_id: &str,
    request: PromptContextRequest,
    preallocated_chat_id: Option<String>,
    purpose: PromptAssemblyPurpose,
) -> Result<PreparedPromptContext, ApiError> {
    let workspace_id = workspace_id.trim();
    let model_id = request.model_id.trim();
    let requested_provider_id = optional_trimmed_string(request.provider_id);
    let thinking_level = optional_trimmed_string(request.thinking_level);
    let requested_skill_ids = request.skill_ids;
    let queued_user_message_id = normalized_optional_text(request.queued_user_message_id);
    let attachment_inputs = request.attachments;
    let raw_message = optional_trimmed_string(request.message);
    let assistant_draft = request
        .assistant_draft
        .filter(|value| !value.trim().is_empty());
    let assistant_draft_reasoning = request
        .assistant_draft_reasoning
        .filter(|value| !value.trim().is_empty());

    if workspace_id.is_empty() {
        return Err(ApiError::bad_request("workspace id must not be empty"));
    }

    if model_id.is_empty() {
        return Err(ApiError::bad_request("model id must not be empty"));
    }

    let workspace = config
        .workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| ApiError::bad_request(format!("workspace was not found: {workspace_id}")))?;
    if purpose.allows_code_graph_initialization() {
        spawn_code_graph_workspace_initialization_if_needed(state, workspace);
    }
    let model = config
        .models
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| ApiError::bad_request(format!("model was not found: {model_id}")))?;

    if !model.enabled {
        return Err(ApiError::bad_request(format!(
            "model '{}' is disabled",
            model.id
        )));
    }

    let limits = model.limits.as_ref().ok_or_else(|| {
        ApiError::bad_request(format!("enabled model '{}' is missing limits", model.id))
    })?;
    let max_output_tokens = u32::try_from(limits.max_output_tokens).map_err(|_| {
        ApiError::bad_request(format!(
            "model '{}' max output tokens exceed u32: {}",
            model.id, limits.max_output_tokens
        ))
    })?;
    let active_provider_id = requested_provider_id
        .as_deref()
        .or(model.active_provider_id.as_deref())
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "model '{}' has no active provider selected",
                model.id
            ))
        })?;

    if !model
        .provider_ids
        .iter()
        .any(|provider_id| provider_id == active_provider_id)
    {
        return Err(ApiError::bad_request(format!(
            "provider '{}' is not associated with model '{}'",
            active_provider_id, model.id
        )));
    }

    let provider = config
        .providers
        .iter()
        .find(|provider| provider.id == active_provider_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!("provider '{}' was not found", active_provider_id))
        })?;

    if !provider.enabled {
        return Err(ApiError::bad_request(format!(
            "provider '{}' is disabled",
            provider.id
        )));
    }

    let provider_config = provider_connection_config(provider)?;
    sync_mcp_workspace(&state.mcp_registry, workspace, config)
        .await
        .map_err(ApiError::from_mcp_error)?;
    let mcp_tools = state.mcp_registry.tool_definitions(&workspace.id).await;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let requested_chat_id = optional_trimmed_string(request.chat_id);
    let is_new_chat = requested_chat_id.is_none();
    let chat_id = match requested_chat_id {
        Some(chat_id) => {
            let chat = database
                .chat(&chat_id)
                .map_err(ApiError::from_workspace_error)?
                .ok_or_else(|| ApiError::bad_request(format!("chat was not found: {chat_id}")))?;
            let metadata =
                serde_json::from_str::<Value>(&chat.metadata_json).map_err(|source| {
                    ApiError::bad_request(format!("chat metadata was invalid: {source}"))
                })?;
            if metadata.get("kind").and_then(Value::as_str)
                == Some(MEMORY_DREAM_TRANSCRIPT_CHAT_KIND)
            {
                return Err(ApiError::bad_request(
                    "memory Dream transcript chats are read-only",
                ));
            }
            Some(chat_id)
        }
        None => preallocated_chat_id,
    };
    let attachments = normalized_chat_attachments_for_workspace(
        Some(&workspace.path),
        chat_id.as_deref(),
        attachment_inputs,
    )?;
    let has_user_turn = raw_message.is_some() || !attachments.is_empty();
    let message = if has_user_turn {
        Some(message_with_selected_skills(
            &state.user_profile_dir,
            config,
            &workspace.id,
            requested_skill_ids,
            raw_message.as_deref().unwrap_or(""),
        )?)
    } else {
        None
    };
    let existing_messages = if is_new_chat {
        Vec::new()
    } else {
        match chat_id.as_deref() {
            Some(chat_id) => {
                let messages = database
                    .messages_for_chat(chat_id)
                    .map_err(ApiError::from_workspace_error)?;
                match queued_user_message_id.as_deref() {
                    Some(queued_user_message_id) => messages
                        .into_iter()
                        .filter(|message| message.id != queued_user_message_id)
                        .collect(),
                    None => messages,
                }
            }
            None => Vec::new(),
        }
    };
    let compression_snapshots = if is_new_chat {
        Vec::new()
    } else {
        match chat_id.as_deref() {
            Some(chat_id) => database
                .context_compression_snapshots_for_chat(chat_id)
                .map_err(ApiError::from_workspace_error)?,
            None => Vec::new(),
        }
    };
    let prompt_context_injections = if is_new_chat {
        Vec::new()
    } else {
        match chat_id.as_deref() {
            Some(chat_id) => database
                .prompt_context_injections_for_chat(chat_id)
                .map_err(ApiError::from_workspace_error)?,
            None => Vec::new(),
        }
    };
    let todo_graph_context_message = if is_new_chat {
        None
    } else {
        match chat_id.as_deref() {
            Some(chat_id) => database
                .todo_graph(chat_id)
                .map_err(ApiError::from_workspace_error)?
                .map(todo_graph_context_message)
                .transpose()?,
            None => None,
        }
    };
    let user_sequence = if is_new_chat {
        0
    } else {
        match chat_id.as_deref() {
            Some(chat_id) => match queued_user_message_id.as_deref() {
                Some(queued_user_message_id) => database
                    .message(queued_user_message_id)
                    .map_err(ApiError::from_workspace_error)?
                    .filter(|message| message.chat_id == chat_id)
                    .map(|message| message.sequence),
                None => None,
            }
            .map_or_else(
                || {
                    database
                        .next_message_sequence_for_chat(chat_id)
                        .map_err(ApiError::from_workspace_error)
                },
                Ok,
            )?,
            None => next_message_sequence(&existing_messages),
        }
    };
    drop(database);

    let ripgrep_available = {
        let status = state
            .ripgrep_status
            .lock()
            .map_err(|_| ApiError::internal("ripgrep status lock was poisoned"))?;
        status.available
    };
    let web_search_available = web_search_enabled(&config.web_search);
    let builtin_tool_definitions =
        builtin_tool_definitions_for_runtime(ripgrep_available, web_search_available);
    let memory_tool_definitions = if config.memory.enabled {
        memory_tool_definitions()
    } else {
        Vec::new()
    };
    let mut neutral_tools = builtin_tool_definitions
        .iter()
        .cloned()
        .map(neutral_tool_definition)
        .collect::<Vec<_>>();
    neutral_tools.extend(memory_tool_definitions.iter().cloned());
    neutral_tools.extend(mcp_tools.iter().map(neutral_mcp_tool_definition));
    let tool_prompt_infos = tool_prompt_infos(
        &builtin_tool_definitions,
        &memory_tool_definitions,
        &mcp_tools,
    );
    let system_prompt = active_system_prompt(&config.prompts, &model.system_prompt_name)?;
    let available_tools_prompt = build_available_tools_prompt(tool_prompt_infos);
    let extra_prompt_message = configured_extra_prompt_message(&config.prompts);
    let system_prompt_tokens = estimate_text_tokens(&system_prompt)
        + available_tools_prompt
            .as_ref()
            .map(|prompt| estimate_text_tokens(prompt))
            .unwrap_or(0)
        + extra_prompt_message
            .as_ref()
            .map(neutral_message_estimated_tokens)
            .unwrap_or(0);
    let context_budget = calculate_context_budget(
        limits.context_window,
        limits.max_output_tokens,
        system_prompt_tokens,
        estimate_tool_schema_tokens(&neutral_tools),
    )
    .map_err(|source| ApiError::bad_request(source.to_string()))?;

    let covered_sequences = snapshot_covered_sequences(&compression_snapshots);
    let agents_messages = if is_new_chat {
        agents_prompt_messages(&workspace.path)?
    } else {
        Vec::new()
    };
    let configured_prompt_messages = if is_new_chat {
        configured_prompt_messages(&config.prompts)?
    } else {
        Vec::new()
    };
    let environment_messages = if is_new_chat {
        vec![environment_context_message(&workspace.path)?]
    } else {
        Vec::new()
    };
    let skill_messages = if is_new_chat {
        enabled_skill_frontmatter_messages(&state.user_profile_dir, config, &workspace.id)?
    } else {
        Vec::new()
    };
    let active_stored_memory_keys = active_prompt_context_memory_keys(
        state,
        config,
        workspace,
        config.memory.enabled,
        &prompt_context_injections,
    )?;
    let existing_stable_context_messages = stored_stable_prompt_context_messages(
        &prompt_context_injections,
        &active_stored_memory_keys,
    )?;
    let existing_turn_memory_messages = stored_turn_memory_messages_by_sequence(
        &prompt_context_injections,
        &active_stored_memory_keys,
    )?;
    let memory_retrieval_query_text =
        memory_retrieval_query_text(raw_message.as_deref(), &existing_messages);
    // For chat runs, memory retrieval is deferred to a later phase so that chat
    // record creation and the stream's `start` event are not blocked by
    // potentially slow memory lookups (e.g. LLM-based retrieval). The retrieval
    // inputs are captured here and applied after the chat has been persisted.
    // Context preview builds must keep memory inline so the usage estimate stays
    // accurate.
    let defer_memory = config.memory.enabled && matches!(purpose, PromptAssemblyPurpose::ChatRun);
    let memory_context = if defer_memory {
        None
    } else {
        Some(
            memory_prompt_context(
                &state.memory_database_file,
                config,
                workspace,
                chat_id.as_deref(),
                memory_retrieval_query_text.as_deref(),
                model,
                provider,
                &context_budget,
                purpose,
                &active_stored_memory_keys,
                is_new_chat,
            )
            .await?,
        )
    };
    let mut stable_context_messages = if is_new_chat {
        let mut messages = Vec::new();
        if let Some(message) = memory_context
            .as_ref()
            .and_then(|context| context.stable_message.clone())
        {
            messages.push(message);
        }
        messages.extend(agents_messages);
        messages.extend(configured_prompt_messages);
        messages.extend(environment_messages);
        messages.extend(skill_messages);
        messages
    } else {
        existing_stable_context_messages
    };
    let current_turn_memory_messages = memory_context
        .as_ref()
        .and_then(|context| context.turn_message.clone())
        .into_iter()
        .collect::<Vec<_>>();
    let mut pending_context_injections = Vec::new();
    if !defer_memory && is_new_chat && !stable_context_messages.is_empty() {
        pending_context_injections.push(PendingPromptContextInjection {
            kind: "stable",
            sequence: None,
            messages: stable_context_messages.clone(),
            memory_keys: memory_context
                .as_ref()
                .map(|context| context.stable_memory_keys.clone())
                .unwrap_or_default(),
        });
    }
    if !defer_memory && !current_turn_memory_messages.is_empty() {
        pending_context_injections.push(PendingPromptContextInjection {
            kind: "turn_memory",
            sequence: Some(user_sequence),
            messages: current_turn_memory_messages.clone(),
            memory_keys: memory_context
                .as_ref()
                .map(|context| context.turn_memory_keys.clone())
                .unwrap_or_default(),
        });
    }
    let mut neutral_messages = Vec::with_capacity(
        existing_messages.len()
            + compression_snapshots.len()
            + stable_context_messages.len()
            + usize::from(extra_prompt_message.is_some())
            + usize::from(todo_graph_context_message.is_some())
            + existing_turn_memory_messages.len()
            + current_turn_memory_messages.len()
            + usize::from(assistant_draft.is_some() || assistant_draft_reasoning.is_some())
            + 2,
    );
    let mut message_source_sequences = Vec::with_capacity(neutral_messages.capacity());
    let mut message_context_sources = Vec::with_capacity(neutral_messages.capacity());
    neutral_messages.push(neutral_text_message(NeutralChatRole::System, system_prompt));
    message_source_sequences.push(None);
    message_context_sources.push(PromptContextSource::ReservedPrompt);
    if let Some(available_tools_prompt) = available_tools_prompt {
        neutral_messages.push(neutral_text_message(
            NeutralChatRole::System,
            available_tools_prompt,
        ));
        message_source_sequences.push(None);
        message_context_sources.push(PromptContextSource::ReservedPrompt);
    }
    if let Some(extra_prompt_message) = extra_prompt_message {
        neutral_messages.push(extra_prompt_message);
        message_source_sequences.push(None);
        message_context_sources.push(PromptContextSource::ReservedPrompt);
    }
    let stable_insert_index = neutral_messages.len();
    for stable_context_message in stable_context_messages.drain(..) {
        neutral_messages.push(stable_context_message);
        message_source_sequences.push(None);
        message_context_sources.push(PromptContextSource::StableInjection);
    }
    if let Some(todo_graph_context_message) = todo_graph_context_message {
        neutral_messages.push(todo_graph_context_message);
        message_source_sequences.push(None);
        message_context_sources.push(PromptContextSource::TodoGraph);
    }
    for snapshot in &compression_snapshots {
        neutral_messages.push(compression_snapshot_message(snapshot));
        message_source_sequences.push(None);
        message_context_sources.push(PromptContextSource::CompressionSnapshot);
    }
    let replay_database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    for existing_message in existing_messages {
        if covered_sequences.contains(&existing_message.sequence) {
            continue;
        }

        let sequence = existing_message.sequence;
        for neutral_message in neutral_messages_from_record(&replay_database, existing_message)? {
            neutral_messages.push(neutral_message);
            message_source_sequences.push(Some(sequence));
            message_context_sources.push(PromptContextSource::StoredMessage { sequence });
        }
        if let Some(turn_memory_messages) = existing_turn_memory_messages.get(&sequence) {
            for turn_memory_message in turn_memory_messages {
                neutral_messages.push(turn_memory_message.clone());
                message_source_sequences.push(Some(sequence));
                message_context_sources.push(PromptContextSource::TurnMemory { sequence });
            }
        }
    }
    if assistant_draft.is_some() || assistant_draft_reasoning.is_some() {
        neutral_messages.push(neutral_assistant_message(
            assistant_draft.unwrap_or_default(),
            assistant_draft_reasoning,
        ));
        message_source_sequences.push(None);
        message_context_sources.push(PromptContextSource::AssistantDraft);
    }
    if message.is_some() || !attachments.is_empty() {
        neutral_messages.push(neutral_user_message(
            message.clone().unwrap_or_default(),
            attachments.clone(),
        ));
        message_source_sequences.push(Some(user_sequence));
        message_context_sources.push(PromptContextSource::CurrentUser {
            sequence: user_sequence,
        });
    }
    let turn_insert_index = neutral_messages.len();
    for turn_memory_message in current_turn_memory_messages {
        neutral_messages.push(turn_memory_message);
        message_source_sequences.push(Some(user_sequence));
        message_context_sources.push(PromptContextSource::TurnMemory {
            sequence: user_sequence,
        });
    }
    let active_tool_start_index = neutral_messages.len();

    let pending_memory_retrieval = if defer_memory {
        Some(PendingMemoryRetrieval {
            workspace: workspace.clone(),
            chat_id_for_retrieval: chat_id.clone(),
            query_text: memory_retrieval_query_text.clone(),
            chat_model: model.clone(),
            chat_provider: provider.clone(),
            purpose,
            excluded_memory_keys: active_stored_memory_keys,
            split_stable_memory: is_new_chat,
            stable_insert_index,
            turn_insert_index,
            user_sequence,
        })
    } else {
        None
    };

    let provider_request = NeutralChatRequest {
        model_id: model.id.clone(),
        messages: neutral_messages,
        tools: neutral_tools,
        thinking_level: thinking_level.or_else(|| model.thinking_level.clone()),
        max_output_tokens: Some(max_output_tokens),
        prompt_cache_key: None,
        prompt_cache_retention: None,
    };
    Ok(PreparedPromptContext {
        workspace_id: workspace.id.clone(),
        workspace_path: workspace.path.clone(),
        chat_id,
        is_new_chat,
        provider_id: provider.id.clone(),
        model_id: model.id.clone(),
        provider_config,
        provider_request,
        context_budget,
        memory_context_tokens: memory_context
            .as_ref()
            .map(|context| context.context_tokens)
            .unwrap_or(0),
        memory_budget_tokens: memory_context
            .as_ref()
            .map(|context| context.budget_tokens)
            .unwrap_or(0),
        memories_used: memory_context
            .map(|context| context.memories_used)
            .unwrap_or_default(),
        compression_snapshots,
        message_source_sequences,
        message_context_sources,
        active_tool_start_index,
        raw_message,
        message,
        attachments,
        next_message_sequence: user_sequence,
        pending_context_injections,
        pending_memory_retrieval,
    })
}
