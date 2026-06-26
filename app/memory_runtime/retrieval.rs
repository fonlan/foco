use std::collections::{HashMap, HashSet};

use foco_agent::estimate_text_tokens;
use foco_providers::{NeutralChatMessage, NeutralChatRequest, NeutralChatRole};
use foco_store::{
    config::GlobalConfig,
    memory::{MemoryDatabase, MemoryFactRecord, MemoryScope},
    workspace::{
        MessageRecord, PromptContextInjectionRecord, ToolCallWithResultRecord, WorkspaceDatabase,
    },
};

use crate::prompt::{neutral_message_estimated_tokens, neutral_tool_call_from_record};
use crate::*;

#[derive(Clone, Debug)]
pub(crate) struct RetrievedMemoryFact {
    pub(crate) fact: MemoryFactRecord,
    pub(crate) source: RetrievedMemorySource,
    pub(crate) rank: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetrievedMemorySource {
    Direct,
    Related,
}

impl RetrievedMemorySource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Related => "related",
        }
    }

    pub(crate) fn rank(self) -> usize {
        match self {
            Self::Direct => 0,
            Self::Related => 1,
        }
    }
}

pub(crate) fn splice_resolved_memory(
    messages: &mut Vec<NeutralChatMessage>,
    message_source_sequences: &mut Vec<Option<i64>>,
    message_context_sources: &mut Vec<PromptContextSource>,
    active_tool_start_index: &mut usize,
    pending: &PendingMemoryRetrieval,
    memory_context: &MemoryPromptContext,
) {
    let mut inserted = 0usize;

    if pending.split_stable_memory {
        if let Some(stable_message) = memory_context.stable_message.clone() {
            let index = pending.stable_insert_index;
            messages.insert(index, stable_message);
            message_source_sequences.insert(index, None);
            message_context_sources.insert(index, PromptContextSource::StableInjection);
            inserted += 1;
        }
    }

    let turn_messages = memory_context
        .turn_message
        .clone()
        .into_iter()
        .collect::<Vec<_>>();
    let turn_insert_count = turn_messages.len();
    if turn_insert_count > 0 {
        let mut index = pending.turn_insert_index + inserted;
        for turn_message in turn_messages {
            messages.insert(index, turn_message);
            message_source_sequences.insert(index, Some(pending.user_sequence));
            message_context_sources.insert(
                index,
                PromptContextSource::TurnMemory {
                    sequence: pending.user_sequence,
                },
            );
            index += 1;
        }
        inserted += turn_insert_count;
    }

    *active_tool_start_index += inserted;
}

/// Resolves deferred memory retrieval for a `PreparedPromptContext` in place.
/// Used by tests that exercise the deferred-retrieval path without going
/// through the full chat context (which owns persistence and cache-key steps).
#[cfg(test)]
pub(crate) async fn resolve_prompt_context_memory(
    context: &mut PreparedPromptContext,
    memory_database_file: &Path,
    config: &GlobalConfig,
) -> Result<(), ApiError> {
    let pending = match context.pending_memory_retrieval.take() {
        Some(pending) => pending,
        None => return Ok(()),
    };

    let memory_context = memory_prompt_context(
        memory_database_file,
        config,
        &pending.workspace,
        pending.chat_id_for_retrieval.as_deref(),
        pending.query_text.as_deref(),
        &pending.chat_model,
        &pending.chat_provider,
        &context.context_budget,
        pending.purpose,
        &pending.excluded_memory_keys,
        pending.split_stable_memory,
    )
    .await?;

    splice_resolved_memory(
        &mut context.provider_request.messages,
        &mut context.message_source_sequences,
        &mut context.message_context_sources,
        &mut context.active_tool_start_index,
        &pending,
        &memory_context,
    );
    context.memory_context_tokens = memory_context.context_tokens;
    context.memory_budget_tokens = memory_context.budget_tokens;
    context.memories_used = memory_context.memories_used;

    Ok(())
}

pub(crate) async fn memory_prompt_context(
    memory_database_file: &Path,
    config: &GlobalConfig,
    workspace: &WorkspaceConfig,
    chat_id: Option<&str>,
    query_text: Option<&str>,
    chat_model: &ModelSettings,
    chat_provider: &ProviderSettings,
    context_budget: &foco_agent::ContextBudget,
    purpose: PromptAssemblyPurpose,
    excluded_memory_keys: &HashSet<String>,
    split_stable_memory: bool,
) -> Result<MemoryPromptContext, ApiError> {
    let budget_tokens = if config.memory.enabled {
        context_budget
            .available_message_tokens
            .saturating_mul(MEMORY_CONTEXT_BUDGET_PERCENT)
            / 100
    } else {
        0
    };

    if !config.memory.enabled || budget_tokens == 0 {
        return Ok(MemoryPromptContext {
            stable_message: None,
            turn_message: None,
            memories_used: Vec::new(),
            context_tokens: 0,
            budget_tokens,
            stable_memory_keys: Vec::new(),
            turn_memory_keys: Vec::new(),
        });
    }

    WorkspaceDatabase::open_or_create(&workspace.path).map_err(ApiError::from_workspace_error)?;
    let mut workspace_memory =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace.path))
            .map_err(ApiError::from_memory_error)?;
    let mut global_memory = MemoryDatabase::open_or_create_global_at(memory_database_file)
        .map_err(ApiError::from_memory_error)?;

    if purpose.allows_memory_mutation() {
        expire_due_memories(&mut workspace_memory)?;
        refresh_memory_profile(&mut workspace_memory, MemoryScope::Workspace, None)?;
        if let Some(chat_id) = chat_id {
            refresh_memory_profile(&mut workspace_memory, MemoryScope::Chat, Some(chat_id))?;
        }
        expire_due_memories(&mut global_memory)?;
        refresh_memory_profile(&mut global_memory, MemoryScope::Global, None)?;
    }

    let mut relevant_facts = if !purpose.allows_llm_memory_retrieval() {
        relevant_memory_facts_fts(
            &mut global_memory,
            &mut workspace_memory,
            chat_id,
            query_text,
        )?
    } else {
        match config.memory.retrieval_mode.as_str() {
            "fts" => relevant_memory_facts_fts(
                &mut global_memory,
                &mut workspace_memory,
                chat_id,
                query_text,
            )?,
            "llm" => {
                let candidates = llm_memory_retrieval_candidates(
                    &global_memory,
                    &workspace_memory,
                    chat_id,
                    query_text,
                )?;
                drop(workspace_memory);
                drop(global_memory);
                relevant_memory_facts_llm(
                    config,
                    &workspace.id,
                    &workspace.path,
                    memory_database_file,
                    candidates,
                    query_text,
                    chat_model,
                    chat_provider,
                    chat_id,
                )
                .await?
            }
            other => {
                return Err(ApiError::bad_request(format!(
                    "memory retrieval mode '{other}' is unsupported"
                )));
            }
        }
    };
    relevant_facts
        .facts
        .retain(|fact| !excluded_memory_keys.contains(&memory_fact_key(&fact.fact)));
    let mut remaining_tokens = budget_tokens;
    let (stable_facts, turn_facts) = if split_stable_memory {
        split_stable_retrieved_memory_facts(relevant_facts.facts)
    } else {
        (Vec::new(), relevant_facts.facts)
    };
    let stable_context = retrieved_memory_context_message(
        &stable_facts,
        &mut remaining_tokens,
        NeutralChatRole::System,
    );
    let turn_context =
        retrieved_memory_context_message(&turn_facts, &mut remaining_tokens, NeutralChatRole::User);
    let context_tokens = stable_context
        .message
        .as_ref()
        .map(neutral_message_estimated_tokens)
        .unwrap_or(0)
        .saturating_add(
            turn_context
                .message
                .as_ref()
                .map(neutral_message_estimated_tokens)
                .unwrap_or(0),
        );
    let mut memories_used = stable_context.memories_used;
    memories_used.extend(turn_context.memories_used);

    Ok(MemoryPromptContext {
        stable_message: stable_context.message,
        turn_message: turn_context.message,
        memories_used,
        context_tokens,
        budget_tokens,
        stable_memory_keys: stable_context.memory_keys,
        turn_memory_keys: turn_context.memory_keys,
    })
}

fn split_stable_retrieved_memory_facts(
    facts: Vec<RetrievedMemoryFact>,
) -> (Vec<RetrievedMemoryFact>, Vec<RetrievedMemoryFact>) {
    facts
        .into_iter()
        .partition(|fact| is_stable_prompt_memory(&fact.fact))
}

fn is_stable_prompt_memory(fact: &MemoryFactRecord) -> bool {
    fact.pinned
        || matches!(fact.scope.as_str(), "global" | "workspace")
        || fact
            .confidence
            .is_some_and(|confidence| confidence >= STABLE_MEMORY_CONFIDENCE_THRESHOLD)
}

pub(crate) fn memory_retrieval_query_text(
    current_user_request: Option<&str>,
    existing_messages: &[MessageRecord],
) -> Option<String> {
    let current_user_request = current_user_request
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let mut query = format!("{MEMORY_RETRIEVAL_CURRENT_REQUEST_LABEL}\n{current_user_request}");

    if let Some(previous_assistant_response) = existing_messages
        .iter()
        .rev()
        .find(|message| message.role == "assistant" && !message.content.trim().is_empty())
        .map(|message| message.content.trim())
    {
        query.push_str("\n\n");
        query.push_str(MEMORY_RETRIEVAL_PREVIOUS_ASSISTANT_LABEL);
        query.push('\n');
        query.push_str(previous_assistant_response);
    }

    Some(query)
}

fn relevant_memory_facts_fts(
    global_memory: &mut MemoryDatabase,
    workspace_memory: &mut MemoryDatabase,
    chat_id: Option<&str>,
    query_text: Option<&str>,
) -> Result<RelevantMemoryFacts, ApiError> {
    let Some(search) = query_text.and_then(memory_prompt_search) else {
        return Ok(RelevantMemoryFacts { facts: Vec::new() });
    };

    let workspace_facts = workspace_memory
        .search_active_facts_for_scope(&search.fts_query, chat_id, None, MEMORY_CONTEXT_FACT_LIMIT)
        .map_err(ApiError::from_memory_error)?;
    let global_facts = global_memory
        .search_active_facts_for_scope(&search.fts_query, None, None, MEMORY_CONTEXT_FACT_LIMIT)
        .map_err(ApiError::from_memory_error)?;
    let workspace_containing_facts = workspace_memory
        .find_active_facts_containing_any_for_scope(
            &search.contains_terms,
            chat_id,
            MEMORY_CONTEXT_FACT_LIMIT,
        )
        .map_err(ApiError::from_memory_error)?;
    let global_containing_facts = global_memory
        .find_active_facts_containing_any_for_scope(
            &search.contains_terms,
            None,
            MEMORY_CONTEXT_FACT_LIMIT,
        )
        .map_err(ApiError::from_memory_error)?;
    let facts = ranked_memory_facts(
        merged_relevant_memory_search_matches(
            workspace_facts,
            workspace_containing_facts,
            &search.contains_terms,
        ),
        merged_relevant_memory_search_matches(
            global_facts,
            global_containing_facts,
            &search.contains_terms,
        ),
    );
    finish_relevant_memory_facts(facts, global_memory, workspace_memory)
}

async fn relevant_memory_facts_llm(
    config: &GlobalConfig,
    workspace_id: &str,
    workspace_path: &Path,
    global_memory_database_file: &Path,
    candidates: Vec<MemoryFactRecord>,
    query_text: Option<&str>,
    chat_model: &ModelSettings,
    chat_provider: &ProviderSettings,
    chat_id: Option<&str>,
) -> Result<RelevantMemoryFacts, ApiError> {
    let query_text = query_text.map(str::trim).filter(|value| !value.is_empty());
    let Some(query_text) = query_text else {
        return Ok(RelevantMemoryFacts { facts: Vec::new() });
    };

    if candidates.is_empty() {
        return Ok(RelevantMemoryFacts { facts: Vec::new() });
    }

    let (model_id, provider_id, provider_config, max_output_tokens) =
        memory_retrieval_provider_for_model(config, chat_model, chat_provider)?;
    let request =
        memory_retrieval_provider_request(&model_id, max_output_tokens, query_text, &candidates)?;
    let output = call_memory_retrieval_provider(
        workspace_path,
        workspace_id,
        chat_id,
        &provider_id,
        &provider_config,
        request,
        config.app.llm_request_retry_count,
        api_audit_save_details(config),
    )
    .await?;
    let selected = parse_memory_retrieval_output(output)?;
    let mut by_key = candidates
        .into_iter()
        .map(|fact| (memory_fact_key(&fact), fact))
        .collect::<HashMap<_, _>>();
    let mut facts = Vec::new();
    let mut seen = HashSet::new();

    for fact_key in selected.fact_keys {
        let fact_key = fact_key.trim();
        if fact_key.is_empty() || !seen.insert(fact_key.to_string()) {
            continue;
        }
        let fact = by_key.remove(fact_key).ok_or_else(|| {
            ApiError::bad_request(format!(
                "memory retrieval model returned unknown fact key '{fact_key}'"
            ))
        })?;
        facts.push(RetrievedMemoryFact {
            fact,
            source: RetrievedMemorySource::Direct,
            rank: facts.len(),
        });
    }

    let mut workspace_memory =
        MemoryDatabase::open_workspace_at(workspace_database_path(workspace_path))
            .map_err(ApiError::from_memory_error)?;
    let mut global_memory = MemoryDatabase::open_or_create_global_at(global_memory_database_file)
        .map_err(ApiError::from_memory_error)?;

    finish_relevant_memory_facts(facts, &mut global_memory, &mut workspace_memory)
}

pub(crate) fn llm_memory_retrieval_candidates(
    global_memory: &MemoryDatabase,
    workspace_memory: &MemoryDatabase,
    chat_id: Option<&str>,
    query_text: Option<&str>,
) -> Result<Vec<MemoryFactRecord>, ApiError> {
    let limit = MEMORY_RETRIEVAL_LLM_FACT_LIMIT as usize;
    let mut candidates = Vec::with_capacity(limit);
    let mut seen = HashSet::new();

    if let Some(search) = query_text.and_then(memory_prompt_search) {
        let workspace_facts = workspace_memory
            .search_active_facts_for_scope(
                &search.fts_query,
                chat_id,
                None,
                MEMORY_RETRIEVAL_LLM_FACT_LIMIT,
            )
            .map_err(ApiError::from_memory_error)?;
        let workspace_containing_facts = workspace_memory
            .find_active_facts_containing_any_for_scope(
                &search.contains_terms,
                chat_id,
                MEMORY_RETRIEVAL_LLM_FACT_LIMIT,
            )
            .map_err(ApiError::from_memory_error)?;
        push_llm_memory_candidates(
            merged_relevant_memory_search_matches(
                workspace_facts,
                workspace_containing_facts,
                &search.contains_terms,
            ),
            &mut seen,
            &mut candidates,
            limit,
        );

        let global_facts = global_memory
            .search_active_facts_for_scope(
                &search.fts_query,
                None,
                None,
                MEMORY_RETRIEVAL_LLM_FACT_LIMIT,
            )
            .map_err(ApiError::from_memory_error)?;
        let global_containing_facts = global_memory
            .find_active_facts_containing_any_for_scope(
                &search.contains_terms,
                None,
                MEMORY_RETRIEVAL_LLM_FACT_LIMIT,
            )
            .map_err(ApiError::from_memory_error)?;
        push_llm_memory_candidates(
            merged_relevant_memory_search_matches(
                global_facts,
                global_containing_facts,
                &search.contains_terms,
            ),
            &mut seen,
            &mut candidates,
            limit,
        );
    }

    let workspace_facts = workspace_memory
        .list_active_facts_for_scope(chat_id, MEMORY_RETRIEVAL_LLM_FACT_LIMIT)
        .map_err(ApiError::from_memory_error)?;
    push_llm_memory_candidates(workspace_facts, &mut seen, &mut candidates, limit);

    let global_facts = global_memory
        .list_active_facts_for_scope(None, MEMORY_RETRIEVAL_LLM_FACT_LIMIT)
        .map_err(ApiError::from_memory_error)?;
    push_llm_memory_candidates(global_facts, &mut seen, &mut candidates, limit);

    Ok(candidates)
}

fn push_llm_memory_candidates(
    facts: Vec<MemoryFactRecord>,
    seen: &mut HashSet<(String, String)>,
    candidates: &mut Vec<MemoryFactRecord>,
    limit: usize,
) {
    for fact in facts {
        if candidates.len() >= limit {
            break;
        }
        if seen.insert((fact.scope.clone(), fact.id.clone())) {
            candidates.push(fact);
        }
    }
}

fn finish_relevant_memory_facts(
    mut facts: Vec<RetrievedMemoryFact>,
    global_memory: &mut MemoryDatabase,
    workspace_memory: &mut MemoryDatabase,
) -> Result<RelevantMemoryFacts, ApiError> {
    let workspace_seed_ids = facts
        .iter()
        .filter(|fact| fact.fact.scope != "global")
        .map(|fact| fact.fact.id.clone())
        .collect::<Vec<_>>();
    let global_seed_ids = facts
        .iter()
        .filter(|fact| fact.fact.scope == "global")
        .map(|fact| fact.fact.id.clone())
        .collect::<Vec<_>>();
    let related_rank_start = facts.len();
    facts.extend(
        workspace_memory
            .related_active_facts(
                &workspace_seed_ids,
                MEMORY_CONTEXT_EDGE_EXPANSION_DEPTH,
                MEMORY_CONTEXT_EDGE_EXPANSION_LIMIT,
            )
            .map_err(ApiError::from_memory_error)?
            .into_iter()
            .enumerate()
            .map(|(index, fact)| RetrievedMemoryFact {
                fact,
                source: RetrievedMemorySource::Related,
                rank: related_rank_start + index,
            }),
    );
    let global_related_rank_start = facts.len();
    facts.extend(
        global_memory
            .related_active_facts(
                &global_seed_ids,
                MEMORY_CONTEXT_EDGE_EXPANSION_DEPTH,
                MEMORY_CONTEXT_EDGE_EXPANSION_LIMIT,
            )
            .map_err(ApiError::from_memory_error)?
            .into_iter()
            .enumerate()
            .map(|(index, fact)| RetrievedMemoryFact {
                fact,
                source: RetrievedMemorySource::Related,
                rank: global_related_rank_start + index,
            }),
    );
    facts.sort_by(retrieved_memory_fact_order);
    let mut seen_fact_keys = HashSet::new();
    facts.retain(|fact| seen_fact_keys.insert((fact.fact.scope.clone(), fact.fact.id.clone())));

    Ok(RelevantMemoryFacts { facts })
}

pub(crate) fn memory_fact_key(fact: &MemoryFactRecord) -> String {
    format!("{}:{}", fact.scope, fact.id)
}

fn memory_retrieval_provider_for_model(
    config: &GlobalConfig,
    chat_model: &ModelSettings,
    chat_provider: &ProviderSettings,
) -> Result<(String, String, ProviderConnectionConfig, u32), ApiError> {
    let model = match config.memory.retrieval_model_id.as_deref() {
        Some(model_id) => config
            .models
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| {
                ApiError::bad_request(format!("memory retrieval model was not found: {model_id}"))
            })?,
        None => chat_model,
    };

    if !model.enabled {
        return Err(ApiError::bad_request(format!(
            "memory retrieval model '{}' is disabled",
            model.id
        )));
    }
    let limits = model.limits.as_ref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "memory retrieval model '{}' is missing limits",
            model.id
        ))
    })?;

    let provider = match config.memory.retrieval_model_id.as_deref() {
        None if model.id == chat_model.id => chat_provider,
        _ => {
            let provider_id = model.active_provider_id.as_deref().ok_or_else(|| {
                ApiError::bad_request(format!(
                    "memory retrieval model '{}' has no active provider selected",
                    model.id
                ))
            })?;
            if !model.provider_ids.iter().any(|id| id == provider_id) {
                return Err(ApiError::bad_request(format!(
                    "active provider '{}' is not associated with memory retrieval model '{}'",
                    provider_id, model.id
                )));
            }
            config
                .providers
                .iter()
                .find(|provider| provider.id == provider_id)
                .ok_or_else(|| {
                    ApiError::bad_request(format!(
                        "memory retrieval provider '{}' was not found",
                        provider_id
                    ))
                })?
        }
    };

    if !provider.enabled {
        return Err(ApiError::bad_request(format!(
            "memory retrieval provider '{}' is disabled",
            provider.id
        )));
    }

    let max_output_tokens = u32::try_from(limits.max_output_tokens)
        .map_err(|_| {
            ApiError::bad_request(format!(
                "memory retrieval model '{}' max output tokens exceed u32: {}",
                model.id, limits.max_output_tokens
            ))
        })?
        .min(MEMORY_RETRIEVAL_MAX_OUTPUT_TOKENS);

    Ok((
        model.id.clone(),
        provider.id.clone(),
        provider_connection_config(provider)?,
        max_output_tokens,
    ))
}

fn memory_retrieval_provider_request(
    model_id: &str,
    max_output_tokens: u32,
    query_text: &str,
    candidates: &[MemoryFactRecord],
) -> Result<NeutralChatRequest, ApiError> {
    let memories_json = serde_json::to_string_pretty(
        &candidates
            .iter()
            .map(|fact| {
                json!({
                    "factKey": memory_fact_key(fact),
                    "scope": &fact.scope,
                    "chatId": &fact.chat_id,
                    "kind": &fact.kind,
                    "pinned": fact.pinned,
                    "updatedAt": &fact.updated_at,
                    "fact": &fact.fact,
                })
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize memory retrieval candidates: {source}"
        ))
    })?;

    Ok(NeutralChatRequest {
        model_id: model_id.to_string(),
        messages: vec![
            neutral_text_message(
                NeutralChatRole::System,
                MEMORY_RETRIEVAL_SYSTEM_PROMPT.to_string(),
            ),
            neutral_text_message(
                NeutralChatRole::User,
                format!(
                    "{MEMORY_RETRIEVAL_CURRENT_REQUEST_LABEL}\n{query_text}\n\nMemory candidates JSON:\n{memories_json}"
                ),
            ),
        ],
        tools: vec![memory_retrieval_tool_definition()],
        thinking_level: None,
        max_output_tokens: Some(max_output_tokens),
        prompt_cache_key: None,
        prompt_cache_retention: None,
    })
}

fn retrieved_memory_context_message(
    facts: &[RetrievedMemoryFact],
    remaining_tokens: &mut u64,
    role: NeutralChatRole,
) -> RetrievedMemoryContext {
    if facts.is_empty() {
        return RetrievedMemoryContext {
            message: None,
            memories_used: Vec::new(),
            memory_keys: Vec::new(),
        };
    }

    let prefix = format!(
        "<memory_context>\n<source>{}</source>",
        xml_text_escape(MEMORY_RETRIEVED_CONTEXT_MESSAGE_PREFIX)
    );
    let prefix_tokens = estimate_text_tokens(&prefix);
    if prefix_tokens > *remaining_tokens {
        return RetrievedMemoryContext {
            message: None,
            memories_used: Vec::new(),
            memory_keys: Vec::new(),
        };
    }
    *remaining_tokens = remaining_tokens.saturating_sub(prefix_tokens);
    let mut content = prefix;
    let mut memories_used = Vec::new();
    let mut memory_keys = Vec::new();
    for retrieved_fact in facts {
        let fact = &retrieved_fact.fact;
        let entry = format!(
            "\n<memory_fact id=\"{}\" scope=\"{}\" chat_id=\"{}\" kind=\"{}\" pinned=\"{}\" source=\"{}\" updated_at=\"{}\">\n{}\n</memory_fact>",
            xml_text_escape(&fact.id),
            xml_text_escape(&fact.scope.to_string()),
            xml_text_escape(fact.chat_id.as_deref().unwrap_or("n/a")),
            xml_text_escape(&fact.kind.to_string()),
            fact.pinned,
            xml_text_escape(retrieved_fact.source.as_str()),
            xml_text_escape(&fact.updated_at),
            xml_cdata_section("fact", &fact.fact)
        );
        let entry_tokens = estimate_text_tokens(&entry);
        if entry_tokens > *remaining_tokens {
            break;
        }
        content.push_str(&entry);
        memories_used.push(chat_memory_used_summary(&retrieved_fact));
        memory_keys.push(memory_fact_key(fact));
        *remaining_tokens = remaining_tokens.saturating_sub(entry_tokens);
    }

    if memories_used.is_empty() {
        return RetrievedMemoryContext {
            message: None,
            memories_used: Vec::new(),
            memory_keys: Vec::new(),
        };
    }
    content.push_str("\n</memory_context>");

    RetrievedMemoryContext {
        message: Some(neutral_text_message(role, content)),
        memories_used,
        memory_keys,
    }
}

pub(crate) fn stored_stable_prompt_context_messages(
    records: &[PromptContextInjectionRecord],
    active_memory_keys: &HashSet<String>,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    records
        .iter()
        .find(|record| record.kind == "stable")
        .map(|record| stored_prompt_context_messages(record, active_memory_keys))
        .transpose()
        .map(Option::unwrap_or_default)
}

pub(crate) fn stored_turn_memory_messages_by_sequence(
    records: &[PromptContextInjectionRecord],
    active_memory_keys: &HashSet<String>,
) -> Result<BTreeMap<i64, Vec<NeutralChatMessage>>, ApiError> {
    let mut by_sequence = BTreeMap::new();

    for record in records.iter().filter(|record| record.kind == "turn_memory") {
        let sequence = record.sequence.ok_or_else(|| {
            ApiError::internal(format!(
                "stored prompt context injection '{}' is missing sequence",
                record.id
            ))
        })?;
        let messages = stored_prompt_context_messages(record, active_memory_keys)?;
        if !messages.is_empty() {
            by_sequence.insert(sequence, messages);
        }
    }

    Ok(by_sequence)
}

pub(crate) fn stored_prompt_context_messages(
    record: &PromptContextInjectionRecord,
    active_memory_keys: &HashSet<String>,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    let messages = serde_json::from_str::<Vec<NeutralChatMessage>>(&record.messages_json).map_err(
        |source| {
            ApiError::internal(format!(
                "failed to parse stored prompt context injection '{}': {source}",
                record.id
            ))
        },
    )?;
    let memory_keys = stored_prompt_context_record_memory_keys(record)?;

    if memory_keys.is_empty()
        || memory_keys
            .iter()
            .all(|key| active_memory_keys.contains(key))
    {
        return Ok(messages);
    }

    Ok(messages
        .into_iter()
        .filter(|message| {
            !message
                .content
                .contains(MEMORY_RETRIEVED_CONTEXT_MESSAGE_PREFIX)
        })
        .collect())
}

pub(crate) fn active_prompt_context_memory_keys(
    state: &AppState,
    config: &GlobalConfig,
    workspace: &WorkspaceConfig,
    memory_enabled: bool,
    records: &[PromptContextInjectionRecord],
) -> Result<HashSet<String>, ApiError> {
    let stored_keys = stored_prompt_context_memory_keys(records)?;
    if !memory_enabled || stored_keys.is_empty() {
        return Ok(HashSet::new());
    }

    let workspace_memory =
        open_memory_database(state, config, MemoryScope::Workspace, Some(&workspace.id))?;
    let global_memory = open_memory_database(state, config, MemoryScope::Global, None)?;
    let mut active_keys = HashSet::new();

    for key in stored_keys {
        let Some((scope, fact_id)) = key.split_once(':') else {
            continue;
        };
        let fact = match scope {
            "global" => global_memory
                .fact(fact_id)
                .map_err(ApiError::from_memory_error)?,
            "workspace" | "chat" => workspace_memory
                .fact(fact_id)
                .map_err(ApiError::from_memory_error)?,
            _ => None,
        };
        if fact
            .as_ref()
            .is_some_and(|fact| fact.status == "active" && fact.is_latest)
        {
            active_keys.insert(key);
        }
    }

    Ok(active_keys)
}

pub(crate) fn stored_prompt_context_memory_keys(
    records: &[PromptContextInjectionRecord],
) -> Result<HashSet<String>, ApiError> {
    let mut keys = HashSet::new();

    for record in records {
        let record_keys = stored_prompt_context_record_memory_keys(record)?;
        keys.extend(record_keys.into_iter().filter(|key| !key.trim().is_empty()));
    }

    Ok(keys)
}

pub(crate) fn stored_prompt_context_record_memory_keys(
    record: &PromptContextInjectionRecord,
) -> Result<Vec<String>, ApiError> {
    serde_json::from_str(&record.memory_keys_json).map_err(|source| {
        ApiError::internal(format!(
            "failed to parse stored prompt context injection '{}' memory keys: {source}",
            record.id
        ))
    })
}

pub(crate) fn persist_pending_prompt_context_injections(
    database: &mut WorkspaceDatabase,
    chat_id: &str,
    pending: &[PendingPromptContextInjection],
) -> Result<(), ApiError> {
    for injection in pending {
        if injection.messages.is_empty() {
            continue;
        }

        let messages_json = serde_json::to_string(&injection.messages).map_err(|source| {
            ApiError::internal(format!(
                "failed to serialize prompt context injection: {source}"
            ))
        })?;
        let memory_keys_json = serde_json::to_string(&injection.memory_keys).map_err(|source| {
            ApiError::internal(format!(
                "failed to serialize prompt context injection memory keys: {source}"
            ))
        })?;

        database
            .insert_prompt_context_injection(NewPromptContextInjection {
                id: &unique_id("ctx-inj"),
                chat_id,
                kind: injection.kind,
                sequence: injection.sequence,
                messages_json: &messages_json,
                memory_keys_json: &memory_keys_json,
            })
            .map_err(ApiError::from_workspace_error)?;
    }

    Ok(())
}

pub(crate) fn prompt_cache_key(
    workspace_id: &str,
    chat_id: &str,
    provider_id: &str,
    model_id: &str,
    request: &NeutralChatRequest,
    message_source_sequences: &[Option<i64>],
    message_context_sources: &[PromptContextSource],
) -> Result<String, ApiError> {
    if request.messages.len() != message_source_sequences.len() {
        return Err(ApiError::internal(
            "prompt cache source sequence count does not match prompt message count",
        ));
    }
    if request.messages.len() != message_context_sources.len() {
        return Err(ApiError::internal(
            "prompt cache source classification count does not match prompt message count",
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(workspace_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(chat_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(provider_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(model_id.as_bytes());
    hasher.update(b"\0");
    let stable_messages = request
        .messages
        .iter()
        .zip(message_context_sources)
        .filter(|(_, source)| prompt_context_source_is_stable_for_cache(source))
        .map(|(message, _)| message)
        .collect::<Vec<_>>();
    let stable_messages_json = serde_json::to_string(&stable_messages).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize stable prompt messages for cache key: {source}"
        ))
    })?;
    hasher.update(stable_messages_json.as_bytes());
    hasher.update(b"\0");
    let tools_json = serde_json::to_string(&request.tools).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize tool definitions for cache key: {source}"
        ))
    })?;
    hasher.update(tools_json.as_bytes());
    hasher.update(b"\0");
    let digest = hasher.finalize();

    Ok(format!("foco:{}", hex_encode(&digest[..16])))
}

fn prompt_context_source_is_stable_for_cache(source: &PromptContextSource) -> bool {
    matches!(
        source,
        PromptContextSource::ReservedPrompt
            | PromptContextSource::AgentDefinition
            | PromptContextSource::AgentTeamProtocol
            | PromptContextSource::StableInjection
            | PromptContextSource::ProjectSpec
            | PromptContextSource::TodoGraph
            | PromptContextSource::CompressionSnapshot
            | PromptContextSource::TurnMemory { .. }
    )
}

pub(crate) fn memory_fts_query(text: &str) -> Option<String> {
    let terms = memory_search_terms(text);

    if terms.is_empty() {
        None
    } else {
        Some(memory_fts_query_from_terms(&terms))
    }
}

pub(crate) fn memory_prompt_search(text: &str) -> Option<MemoryPromptSearch> {
    let terms = memory_prompt_search_terms(text);
    if terms.is_empty() {
        return None;
    }
    let contains_terms = memory_prompt_contains_terms(text);

    Some(MemoryPromptSearch {
        fts_query: memory_fts_query_from_terms(&terms),
        contains_terms,
    })
}

pub(crate) fn memory_prompt_search_terms(text: &str) -> Vec<String> {
    memory_search_terms(text)
        .into_iter()
        .filter(|term| !is_memory_prompt_stop_term(term))
        .collect()
}

fn memory_prompt_contains_terms(text: &str) -> Vec<String> {
    let mut terms = memory_prompt_search_terms(text);
    for gram in cjk_memory_prompt_grams(text) {
        if !terms.contains(&gram) {
            terms.push(gram);
        }
    }
    terms
}

fn memory_search_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    let mut seen = HashSet::new();

    for character in text.chars() {
        if character.is_alphanumeric() {
            current.extend(character.to_lowercase());
            if current.chars().count() >= 64 {
                push_memory_fts_term(&mut terms, &mut seen, &mut current);
            }
        } else {
            push_memory_fts_term(&mut terms, &mut seen, &mut current);
        }

        if terms.len() >= 12 {
            break;
        }
    }
    push_memory_fts_term(&mut terms, &mut seen, &mut current);

    terms
}

fn memory_fts_query_from_terms(terms: &[String]) -> String {
    terms
        .iter()
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn push_memory_fts_term(terms: &mut Vec<String>, seen: &mut HashSet<String>, current: &mut String) {
    let term = current.trim();
    if term.chars().count() >= 2 && seen.insert(term.to_string()) {
        terms.push(term.to_string());
    }
    current.clear();
}

fn cjk_memory_prompt_grams(text: &str) -> Vec<String> {
    let mut grams = Vec::new();
    let mut seen = HashSet::new();
    let mut run = Vec::new();

    for character in text.chars() {
        if is_cjk_memory_character(character) {
            run.push(character);
        } else {
            push_cjk_memory_prompt_grams(&run, &mut seen, &mut grams);
            run.clear();
        }
    }
    push_cjk_memory_prompt_grams(&run, &mut seen, &mut grams);

    grams
}

fn push_cjk_memory_prompt_grams(run: &[char], seen: &mut HashSet<String>, grams: &mut Vec<String>) {
    for gram_len in [4usize, 3, 2] {
        if run.len() < gram_len {
            continue;
        }
        for window in run.windows(gram_len) {
            if grams.len() >= 24 {
                return;
            }
            let gram = window.iter().collect::<String>();
            if seen.insert(gram.clone()) {
                grams.push(gram);
            }
        }
    }
}

fn is_cjk_memory_character(character: char) -> bool {
    ('\u{3400}'..='\u{4DBF}').contains(&character)
        || ('\u{4E00}'..='\u{9FFF}').contains(&character)
        || ('\u{F900}'..='\u{FAFF}').contains(&character)
}

fn merged_relevant_memory_search_matches(
    fts_facts: Vec<MemoryFactRecord>,
    containing_facts: Vec<MemoryFactRecord>,
    query_terms: &[String],
) -> Vec<MemoryFactRecord> {
    let mut seen = HashSet::new();
    fts_facts
        .into_iter()
        .chain(containing_facts)
        .filter(|fact| memory_fact_matches_prompt_terms(fact, query_terms))
        .filter(|fact| seen.insert((fact.scope.clone(), fact.id.clone())))
        .collect()
}

fn memory_fact_matches_prompt_terms(fact: &MemoryFactRecord, query_terms: &[String]) -> bool {
    if query_terms.is_empty() {
        return false;
    }

    let searchable_text = fact.fact.to_ascii_lowercase();
    query_terms
        .iter()
        .any(|term| searchable_text.contains(term.as_str()))
}

fn is_memory_prompt_stop_term(term: &str) -> bool {
    matches!(
        term,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "but"
            | "by"
            | "can"
            | "do"
            | "does"
            | "for"
            | "from"
            | "how"
            | "i"
            | "in"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "prompt"
            | "that"
            | "the"
            | "this"
            | "to"
            | "with"
            | "what"
            | "when"
            | "where"
            | "why"
            | "you"
    )
}

fn ranked_memory_facts(
    workspace_facts: Vec<MemoryFactRecord>,
    global_facts: Vec<MemoryFactRecord>,
) -> Vec<RetrievedMemoryFact> {
    workspace_facts
        .into_iter()
        .chain(global_facts)
        .enumerate()
        .map(|(rank, fact)| RetrievedMemoryFact {
            fact,
            source: RetrievedMemorySource::Direct,
            rank,
        })
        .collect()
}

fn retrieved_memory_fact_order(
    left: &RetrievedMemoryFact,
    right: &RetrievedMemoryFact,
) -> std::cmp::Ordering {
    left.source
        .rank()
        .cmp(&right.source.rank())
        .then_with(|| left.rank.cmp(&right.rank))
        .then_with(|| memory_fact_prompt_order(&left.fact, &right.fact))
}

pub(crate) fn memory_fact_prompt_order(
    left: &MemoryFactRecord,
    right: &MemoryFactRecord,
) -> std::cmp::Ordering {
    right
        .pinned
        .cmp(&left.pinned)
        .then_with(|| memory_fact_scope_rank(left).cmp(&memory_fact_scope_rank(right)))
        .then_with(|| right.is_latest.cmp(&left.is_latest))
        .then_with(|| {
            right
                .confidence
                .partial_cmp(&left.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| right.updated_at.cmp(&left.updated_at))
        .then_with(|| left.id.cmp(&right.id))
}

fn memory_fact_scope_rank(fact: &MemoryFactRecord) -> u8 {
    match fact.scope.as_str() {
        "chat" => 0,
        "workspace" => 1,
        "global" => 2,
        _ => 3,
    }
}

pub(crate) fn chat_memory_used_summary(
    retrieved_fact: &RetrievedMemoryFact,
) -> ChatMemoryUsedSummary {
    let fact = &retrieved_fact.fact;

    ChatMemoryUsedSummary {
        id: fact.id.clone(),
        scope: fact.scope.clone(),
        chat_id: fact.chat_id.clone(),
        kind: fact.kind.clone(),
        fact: fact.fact.clone(),
        pinned: fact.pinned,
        source: retrieved_fact.source.as_str().to_string(),
    }
}

pub(crate) fn chat_extracted_memory_summary(fact: MemoryFactRecord) -> ChatExtractedMemorySummary {
    ChatExtractedMemorySummary {
        id: fact.id,
        scope: fact.scope,
        chat_id: fact.chat_id,
        status: fact.status,
        kind: fact.kind,
        fact: fact.fact,
    }
}

pub(crate) fn neutral_messages_from_record(
    database: &WorkspaceDatabase,
    message: MessageRecord,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    let role = match message.role.as_str() {
        "system" => NeutralChatRole::System,
        "developer" => NeutralChatRole::Developer,
        "user" => NeutralChatRole::User,
        "assistant" => NeutralChatRole::Assistant,
        "tool" => NeutralChatRole::Tool,
        other => {
            return Err(ApiError::bad_request(format!(
                "chat contains unsupported message role '{other}'"
            )));
        }
    };

    if role != NeutralChatRole::Assistant && role != NeutralChatRole::Tool {
        let attachments = if role == NeutralChatRole::User {
            message_attachments_from_metadata(&message.metadata_json)?
        } else {
            Vec::new()
        };
        if role == NeutralChatRole::User {
            return Ok(vec![neutral_user_message(message.content, attachments)]);
        }

        return Ok(vec![NeutralChatMessage {
            role,
            content: message.content,
            attachments,
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
        }]);
    }

    if role == NeutralChatRole::Assistant {
        let reasoning = if role == NeutralChatRole::Assistant {
            assistant_reasoning_from_metadata(&message.metadata_json)?
        } else {
            None
        };
        let tool_calls = database
            .tool_calls_for_message(&message.id)
            .map_err(ApiError::from_workspace_error)?;

        if let Some(parts) = stored_assistant_parts_from_metadata(&message.metadata_json)? {
            return replay_stored_assistant_parts(&message, parts, &tool_calls, reasoning);
        }

        if tool_calls.is_empty() {
            return Ok(vec![NeutralChatMessage {
                role,
                content: message.content,
                attachments: Vec::new(),
                reasoning,
                tool_calls: Vec::new(),
                tool_call_id: None,
                tool_name: None,
            }]);
        }

        let mut messages = Vec::with_capacity(tool_calls.len() * 2 + 1);
        for tool_call in tool_calls {
            let Some(result) = tool_call.result.clone() else {
                tracing::warn!(
                    assistant_message_id = %message.id,
                    tool_call_id = %tool_call.id,
                    tool_call_status = %tool_call.status,
                    "skipping incomplete persisted tool call while rebuilding provider prompt"
                );
                continue;
            };

            messages.push(NeutralChatMessage {
                role: NeutralChatRole::Assistant,
                content: String::new(),
                attachments: Vec::new(),
                reasoning: None,
                tool_calls: vec![neutral_tool_call_from_record(&tool_call)?],
                tool_call_id: None,
                tool_name: None,
            });
            messages.push(NeutralChatMessage {
                role: NeutralChatRole::Tool,
                content: result.output_json,
                attachments: Vec::new(),
                reasoning: None,
                tool_calls: Vec::new(),
                tool_call_id: Some(tool_call.id),
                tool_name: Some(tool_call.tool_name),
            });
        }

        if !message.content.trim().is_empty() {
            messages.push(NeutralChatMessage {
                role: NeutralChatRole::Assistant,
                content: message.content,
                attachments: Vec::new(),
                reasoning,
                tool_calls: Vec::new(),
                tool_call_id: None,
                tool_name: None,
            });
        }

        return Ok(messages);
    }

    if role != NeutralChatRole::Tool {
        return Err(ApiError::internal(
            "unsupported neutral message role while rebuilding chat history",
        ));
    }

    let metadata = parse_json_value(&message.metadata_json, "tool message metadata")?;
    let tool_call_id = metadata
        .get("toolCallId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "tool message '{}' is missing metadata.toolCallId",
                message.id
            ))
        })?;
    let tool_name = metadata
        .get("toolName")
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(vec![NeutralChatMessage {
        role,
        content: message.content,
        attachments: Vec::new(),
        reasoning: None,
        tool_calls: Vec::new(),
        tool_call_id: Some(tool_call_id),
        tool_name,
    }])
}

fn stored_assistant_parts_from_metadata(
    metadata_json: &str,
) -> Result<Option<Vec<StoredChatMessagePart>>, ApiError> {
    let metadata = parse_json_value(metadata_json, "assistant message metadata")?;
    let Some(parts) = metadata.get("parts") else {
        return Ok(None);
    };

    serde_json::from_value::<Vec<StoredChatMessagePart>>(parts.clone())
        .map(Some)
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to parse assistant message metadata.parts: {source}"
            ))
        })
}

fn replay_stored_assistant_parts(
    message: &MessageRecord,
    parts: Vec<StoredChatMessagePart>,
    tool_calls: &[ToolCallWithResultRecord],
    fallback_reasoning: Option<String>,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    let tool_calls_by_id = tool_calls
        .iter()
        .map(|tool_call| (tool_call.id.as_str(), tool_call))
        .collect::<HashMap<_, _>>();
    let mut replayed_tool_call_ids = HashSet::new();
    let mut messages = Vec::new();

    for part in parts {
        match part {
            StoredChatMessagePart::Text { text } => {
                if !text.trim().is_empty() {
                    messages.push(neutral_assistant_message(text, None));
                }
            }
            StoredChatMessagePart::Reasoning { text } => {
                if !text.trim().is_empty() {
                    messages.push(neutral_assistant_message(String::new(), Some(text)));
                }
            }
            StoredChatMessagePart::ToolCall { tool_call_id } => {
                append_replayed_tool_call_pair(
                    &mut messages,
                    message,
                    tool_calls_by_id.get(tool_call_id.as_str()).copied(),
                    &tool_call_id,
                    &mut replayed_tool_call_ids,
                )?;
            }
        }
    }

    for tool_call in tool_calls {
        if replayed_tool_call_ids.contains(&tool_call.id) {
            continue;
        }
        append_replayed_tool_call_pair(
            &mut messages,
            message,
            Some(tool_call),
            &tool_call.id,
            &mut replayed_tool_call_ids,
        )?;
    }

    if messages.is_empty() && (!message.content.trim().is_empty() || fallback_reasoning.is_some()) {
        messages.push(neutral_assistant_message(
            message.content.clone(),
            fallback_reasoning,
        ));
    }

    Ok(messages)
}

fn append_replayed_tool_call_pair(
    messages: &mut Vec<NeutralChatMessage>,
    message: &MessageRecord,
    tool_call: Option<&ToolCallWithResultRecord>,
    tool_call_id: &str,
    replayed_tool_call_ids: &mut HashSet<String>,
) -> Result<(), ApiError> {
    if !replayed_tool_call_ids.insert(tool_call_id.to_string()) {
        return Ok(());
    }

    let Some(tool_call) = tool_call else {
        tracing::warn!(
            assistant_message_id = %message.id,
            tool_call_id,
            "skipping missing persisted tool call while rebuilding provider prompt from stored parts"
        );
        return Ok(());
    };
    let Some(result) = tool_call.result.as_ref() else {
        tracing::warn!(
            assistant_message_id = %message.id,
            tool_call_id = %tool_call.id,
            tool_call_status = %tool_call.status,
            "skipping incomplete persisted tool call while rebuilding provider prompt"
        );
        return Ok(());
    };

    messages.push(NeutralChatMessage {
        role: NeutralChatRole::Assistant,
        content: String::new(),
        attachments: Vec::new(),
        reasoning: None,
        tool_calls: vec![neutral_tool_call_from_record(tool_call)?],
        tool_call_id: None,
        tool_name: None,
    });
    messages.push(NeutralChatMessage {
        role: NeutralChatRole::Tool,
        content: result.output_json.clone(),
        attachments: Vec::new(),
        reasoning: None,
        tool_calls: Vec::new(),
        tool_call_id: Some(tool_call.id.clone()),
        tool_name: Some(tool_call.tool_name.clone()),
    });

    Ok(())
}
