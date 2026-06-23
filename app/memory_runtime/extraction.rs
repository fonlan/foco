use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use foco_providers::{NeutralChatRequest, NeutralChatRole, ProviderConnectionConfig};
use foco_store::{
    config::{GlobalConfig, MemorySettings},
    memory::{
        MemoryDatabase, MemoryExtractionJobStatus, MemoryFactRecord, MemoryKind,
        MemoryRelationKind, MemoryScope, MemorySourceType, MemoryStatus, NewMemoryEdge,
        NewMemoryExtractionJob, NewMemoryFact, NewMemorySource,
    },
    workspace::{WorkspaceDatabase, workspace_database_path},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::http::memory::refresh_memory_profile;
use crate::memory_runtime::tools::memory_extraction_tool_definition;
use crate::memory_runtime::{
    apply_memory_expiration_to_fact, chat_extracted_memory_summary, memory_fact_key,
    memory_fact_prompt_order,
};
use crate::*;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemoryRetrievalOutput {
    pub(crate) fact_keys: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct MemoryExtractionTask {
    pub(crate) job_id: String,
    pub(crate) workspace_id: String,
    pub(crate) workspace_path: PathBuf,
    pub(crate) global_memory_database_file: PathBuf,
    pub(crate) chat_id: String,
    pub(crate) run_id: String,
    pub(crate) user_message_id: String,
    pub(crate) assistant_message_id: String,
    pub(crate) model_id: String,
    pub(crate) target_status: MemoryStatus,
    pub(crate) config: GlobalConfig,
}

#[derive(Clone, Debug)]
pub(crate) struct MemoryExtractionEvidenceCandidate {
    pub(crate) evidence_id: String,
    pub(crate) source_type: MemorySourceType,
    pub(crate) source_id: String,
    pub(crate) title: String,
    pub(crate) content: String,
    pub(crate) metadata: Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemoryExtractionOutput {
    pub(crate) facts: Vec<ExtractedMemoryFact>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExtractedMemoryFact {
    pub(crate) scope: String,
    pub(crate) kind: String,
    pub(crate) fact: String,
    pub(crate) confidence: Option<f64>,
    pub(crate) relation_candidates: Vec<ExtractedMemoryRelationCandidate>,
    pub(crate) evidence_references: Vec<ExtractedMemoryEvidenceReference>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExtractedMemoryRelationCandidate {
    pub(crate) relation: String,
    pub(crate) target_fact_id: Option<String>,
    pub(crate) target_fact: Option<String>,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExtractedMemoryEvidenceReference {
    pub(crate) evidence_id: String,
    pub(crate) quote: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ValidatedExtractedMemoryFact {
    pub(crate) scope: MemoryScope,
    pub(crate) kind: MemoryKind,
    pub(crate) fact: String,
    pub(crate) confidence: Option<f64>,
    pub(crate) evidence_ids: Vec<String>,
    pub(crate) relation_candidates: Vec<ValidatedExtractedMemoryRelationCandidate>,
    pub(crate) metadata_json: String,
}

#[derive(Debug)]
pub(crate) struct ValidatedExtractedMemoryRelationCandidate {
    pub(crate) relation: MemoryRelationKind,
    pub(crate) target_fact_id: String,
    pub(crate) target_fact: Option<String>,
    pub(crate) reason: Option<String>,
}

pub(crate) fn queue_memory_extraction_job(
    context: &PreparedChatContext,
    final_state: &str,
) -> Result<(), ApiError> {
    if final_state != "succeeded" || !should_queue_memory_extraction(&context.memory_settings) {
        return Ok(());
    }

    let target_status = memory_extraction_target_status(
        &context.memory_settings.extraction_mode,
        context.memory_target_status,
    );
    let model_id = context
        .memory_settings
        .extraction_model_id
        .as_deref()
        .unwrap_or(&context.model_id);
    let input_json = json!({
        "trigger": "chat_completed",
        "targetStatus": target_status.as_str(),
        "workspaceId": context.workspace_id,
        "chatId": context.chat_id,
        "runId": context.llm_request_id,
        "userMessageId": context.user_message_id,
        "assistantMessageId": context.assistant_message_id,
        "chatModelId": context.model_id,
        "extractionModelId": model_id,
        "providerId": context.provider_id,
    })
    .to_string();
    let mut memory_database =
        MemoryDatabase::open_workspace_at(workspace_database_path(&context.workspace_path))
            .map_err(ApiError::from_memory_error)?;
    let job_id = unique_id("memory-extraction");

    memory_database
        .insert_extraction_job(NewMemoryExtractionJob {
            id: &job_id,
            scope: MemoryScope::Chat,
            chat_id: Some(&context.chat_id),
            status: MemoryExtractionJobStatus::Queued,
            model_id: Some(model_id),
            input_json: &input_json,
            output_json: None,
            error_message: None,
        })
        .map_err(ApiError::from_memory_error)?;

    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::warn!(
            job_id = %job_id,
            workspace_id = %context.workspace_id,
            chat_id = %context.chat_id,
            "memory extraction job queued without an active async runtime"
        );
        return Ok(());
    };
    let task = MemoryExtractionTask {
        job_id: job_id.clone(),
        workspace_id: context.workspace_id.clone(),
        workspace_path: context.workspace_path.clone(),
        global_memory_database_file: context.memory_database_file.clone(),
        chat_id: context.chat_id.clone(),
        run_id: context.llm_request_id.clone(),
        user_message_id: context.user_message_id.clone(),
        assistant_message_id: context.assistant_message_id.clone(),
        model_id: model_id.to_string(),
        target_status,
        config: context.global_config.clone(),
    };
    handle.spawn(async move {
        let job_id = task.job_id.clone();
        let workspace_id = task.workspace_id.clone();
        let chat_id = task.chat_id.clone();
        if let Err(error) = run_memory_extraction_job(task).await {
            tracing::warn!(
                job_id = %job_id,
                workspace_id = %workspace_id,
                chat_id = %chat_id,
                error = %error.message,
                "memory extraction background job failed"
            );
        }
    });

    Ok(())
}

pub(crate) fn should_queue_memory_extraction(settings: &MemorySettings) -> bool {
    settings.enabled
        && matches!(
            settings.extraction_mode.as_str(),
            "pending_review" | "automatic"
        )
}

pub(crate) fn memory_extraction_target_status(
    extraction_mode: &str,
    prompt_target_status: MemoryStatus,
) -> MemoryStatus {
    if extraction_mode == "automatic" {
        MemoryStatus::Active
    } else {
        prompt_target_status
    }
}

pub(crate) fn memory_target_status_for_prompt(message: &str) -> MemoryStatus {
    let normalized = message.trim().to_ascii_lowercase();

    if normalized.starts_with("remember this")
        || normalized.starts_with("remember:")
        || normalized.starts_with("please remember")
    {
        MemoryStatus::Active
    } else {
        MemoryStatus::Pending
    }
}

pub(crate) async fn run_memory_extraction_job(
    task: MemoryExtractionTask,
) -> Result<Vec<ChatExtractedMemorySummary>, ApiError> {
    let workspace_memory_path = workspace_database_path(&task.workspace_path);
    let mut workspace_memory_database = MemoryDatabase::open_workspace_at(&workspace_memory_path)
        .map_err(ApiError::from_memory_error)?;
    workspace_memory_database
        .mark_extraction_job_running(&task.job_id)
        .map_err(ApiError::from_memory_error)?;
    drop(workspace_memory_database);

    let mut attempt = 1;
    let extraction_result = loop {
        let result = run_memory_extraction_job_inner(&task).await;
        let Err(error) = &result else {
            break result;
        };
        if !memory_extraction_error_should_be_ignored(Some(&error.message)) {
            break result;
        }
        if attempt >= MEMORY_EXTRACTION_MAX_ATTEMPTS {
            tracing::warn!(
                job_id = %task.job_id,
                workspace_id = %task.workspace_id,
                chat_id = %task.chat_id,
                model_id = %task.model_id,
                attempt,
                error = %error.message,
                "memory extraction model output stayed invalid after retry; ignoring extraction"
            );
            break result;
        }
        tracing::warn!(
            job_id = %task.job_id,
            workspace_id = %task.workspace_id,
            chat_id = %task.chat_id,
            model_id = %task.model_id,
            attempt,
            error = %error.message,
            "memory extraction model output was invalid; retrying"
        );
        attempt += 1;
    };
    let mut workspace_memory_database = MemoryDatabase::open_workspace_at(&workspace_memory_path)
        .map_err(ApiError::from_memory_error)?;

    let extracted_memories = match extraction_result {
        Ok((output_json, extracted_memories)) => {
            workspace_memory_database
                .complete_extraction_job(&task.job_id, &output_json)
                .map_err(ApiError::from_memory_error)?;
            extracted_memories
        }
        Err(error) => {
            if memory_extraction_error_should_be_ignored(Some(&error.message)) {
                workspace_memory_database
                    .complete_extraction_job(&task.job_id, r#"{"facts":[]}"#)
                    .map_err(ApiError::from_memory_error)?;
                Vec::new()
            } else {
                workspace_memory_database
                    .fail_extraction_job(&task.job_id, &error.message, None)
                    .map_err(ApiError::from_memory_error)?;
                Vec::new()
            }
        }
    };

    Ok(extracted_memories)
}

pub(crate) async fn run_memory_extraction_job_inner(
    task: &MemoryExtractionTask,
) -> Result<(String, Vec<ChatExtractedMemorySummary>), ApiError> {
    let workspace_database = WorkspaceDatabase::open_or_create(&task.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let evidence_candidates = memory_extraction_evidence_candidates(
        &workspace_database,
        &task.chat_id,
        &task.run_id,
        &task.user_message_id,
        &task.assistant_message_id,
    )?;
    let workspace_memory =
        MemoryDatabase::open_workspace_at(workspace_database_path(&task.workspace_path))
            .map_err(ApiError::from_memory_error)?;
    let global_memory = MemoryDatabase::open_or_create_global_at(&task.global_memory_database_file)
        .map_err(ApiError::from_memory_error)?;
    let existing_memory_candidates = memory_extraction_existing_memory_candidates(
        &global_memory,
        &workspace_memory,
        &task.chat_id,
    )?;
    let (provider_id, provider_config, max_output_tokens) =
        extraction_provider_for_model(&task.config, &task.model_id)?;
    let request = memory_extraction_provider_request(
        &task.model_id,
        &task.workspace_id,
        &task.chat_id,
        &task.run_id,
        &provider_id,
        max_output_tokens,
        &evidence_candidates,
        &existing_memory_candidates,
    )?;
    let tool_arguments = call_memory_extraction_provider(
        &task.workspace_path,
        &task.workspace_id,
        Some(&task.chat_id),
        &provider_id,
        &provider_config,
        request,
        task.config.app.llm_request_retry_count,
        api_audit_save_details(&task.config),
    )
    .await?;
    let output = parse_memory_extraction_output(tool_arguments)?;
    let extracted_memories = store_extracted_memory_facts(task, &evidence_candidates, &output)?;
    let output_json = serde_json::to_string(&output).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize memory extraction output: {source}"
        ))
    })?;

    Ok((output_json, extracted_memories))
}

pub(crate) fn memory_extraction_evidence_candidates(
    database: &WorkspaceDatabase,
    chat_id: &str,
    run_id: &str,
    user_message_id: &str,
    assistant_message_id: &str,
) -> Result<Vec<MemoryExtractionEvidenceCandidate>, ApiError> {
    let messages = database
        .messages_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let mut evidence = Vec::new();
    let mut found_user_message = false;
    let mut found_assistant_message = false;

    for message in messages {
        if message.id == user_message_id {
            found_user_message = true;
            evidence.push(MemoryExtractionEvidenceCandidate {
                evidence_id: "user_message".to_string(),
                source_type: MemorySourceType::ChatMessage,
                source_id: message.id,
                title: "User message".to_string(),
                content: message.content,
                metadata: json!({
                    "role": &message.role,
                    "sequence": message.sequence,
                    "createdAt": &message.created_at,
                }),
            });
            continue;
        }

        if message.id == assistant_message_id {
            found_assistant_message = true;
            evidence.push(MemoryExtractionEvidenceCandidate {
                evidence_id: "assistant_message".to_string(),
                source_type: MemorySourceType::AssistantMessage,
                source_id: message.id.clone(),
                title: "Assistant message".to_string(),
                content: message.content.clone(),
                metadata: json!({
                    "role": &message.role,
                    "sequence": message.sequence,
                    "createdAt": &message.created_at,
                }),
            });
            let tool_calls = database
                .tool_calls_for_message(&message.id)
                .map_err(ApiError::from_workspace_error)?;
            for (index, tool_call) in tool_calls
                .into_iter()
                .filter(|tool_call| tool_call.run_id == run_id)
                .enumerate()
            {
                let call_evidence_id = format!("tool_call_{index}");
                evidence.push(MemoryExtractionEvidenceCandidate {
                    evidence_id: call_evidence_id,
                    source_type: MemorySourceType::ToolCall,
                    source_id: tool_call.id.clone(),
                    title: format!("Tool call {}", tool_call.tool_name),
                    content: tool_call.input_json.clone(),
                    metadata: json!({
                        "toolName": &tool_call.tool_name,
                        "status": &tool_call.status,
                        "startedAt": &tool_call.started_at,
                        "completedAt": &tool_call.completed_at,
                    }),
                });

                if let Some(result) = tool_call.result {
                    evidence.push(MemoryExtractionEvidenceCandidate {
                        evidence_id: format!("tool_result_{index}"),
                        source_type: MemorySourceType::ToolResult,
                        source_id: result.id,
                        title: format!("Tool result {}", tool_call.tool_name),
                        content: result.output_json,
                        metadata: json!({
                            "toolCallId": &tool_call.id,
                            "toolName": &tool_call.tool_name,
                            "isError": result.is_error,
                            "createdAt": &result.created_at,
                        }),
                    });
                }
            }
        }
    }

    if !found_user_message || !found_assistant_message {
        return Err(ApiError::internal(
            "memory extraction evidence was not found for completed chat run",
        ));
    }

    Ok(evidence)
}

pub(crate) fn memory_extraction_existing_memory_candidates(
    global_memory: &MemoryDatabase,
    workspace_memory: &MemoryDatabase,
    chat_id: &str,
) -> Result<Vec<MemoryFactRecord>, ApiError> {
    let mut candidates = Vec::new();
    for status in [MemoryStatus::Active, MemoryStatus::Pending] {
        candidates.extend(
            workspace_memory
                .list_facts_for_scope(
                    Some(chat_id),
                    status,
                    None,
                    None,
                    MEMORY_EXTRACTION_EXISTING_FACT_LIMIT,
                )
                .map_err(ApiError::from_memory_error)?,
        );
        candidates.extend(
            global_memory
                .list_facts_for_scope(
                    None,
                    status,
                    None,
                    None,
                    MEMORY_EXTRACTION_EXISTING_FACT_LIMIT,
                )
                .map_err(ApiError::from_memory_error)?,
        );
    }

    candidates.sort_by(memory_fact_prompt_order);
    let mut seen = HashSet::new();
    candidates.retain(|fact| seen.insert(memory_fact_key(fact)));
    candidates.truncate(MEMORY_EXTRACTION_EXISTING_FACT_LIMIT as usize);

    Ok(candidates)
}

pub(crate) fn extraction_provider_for_model(
    config: &GlobalConfig,
    model_id: &str,
) -> Result<(String, ProviderConnectionConfig, u32), ApiError> {
    let model = config
        .models
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!("memory extraction model was not found: {model_id}"))
        })?;

    if !model.enabled {
        return Err(ApiError::bad_request(format!(
            "memory extraction model '{}' is disabled",
            model.id
        )));
    }
    let limits = model.limits.as_ref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "memory extraction model '{}' is missing limits",
            model.id
        ))
    })?;

    let provider_id = model.active_provider_id.as_deref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "memory extraction model '{}' has no active provider selected",
            model.id
        ))
    })?;
    if !model.provider_ids.iter().any(|id| id == provider_id) {
        return Err(ApiError::bad_request(format!(
            "active provider '{}' is not associated with memory extraction model '{}'",
            provider_id, model.id
        )));
    }
    let provider = config
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "memory extraction provider '{}' was not found",
                provider_id
            ))
        })?;
    if !provider.enabled {
        return Err(ApiError::bad_request(format!(
            "memory extraction provider '{}' is disabled",
            provider.id
        )));
    }

    let max_output_tokens = u32::try_from(limits.max_output_tokens)
        .map_err(|_| {
            ApiError::bad_request(format!(
                "memory extraction model '{}' max output tokens exceed u32: {}",
                model.id, limits.max_output_tokens
            ))
        })?
        .min(MEMORY_EXTRACTION_MAX_OUTPUT_TOKENS);

    Ok((
        provider.id.clone(),
        provider_connection_config(provider)?,
        max_output_tokens,
    ))
}

pub(crate) fn memory_extraction_provider_request(
    model_id: &str,
    workspace_id: &str,
    chat_id: &str,
    run_id: &str,
    provider_id: &str,
    max_output_tokens: u32,
    evidence: &[MemoryExtractionEvidenceCandidate],
    existing_memory_candidates: &[MemoryFactRecord],
) -> Result<NeutralChatRequest, ApiError> {
    let existing_memories_json = serde_json::to_string_pretty(
        &existing_memory_candidates
            .iter()
            .map(|fact| {
                json!({
                    "factKey": memory_fact_key(fact),
                    "scope": &fact.scope,
                    "chatId": &fact.chat_id,
                    "status": &fact.status,
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
            "failed to serialize extraction memory candidates: {source}"
        ))
    })?;
    let evidence_json = serde_json::to_string_pretty(
        &evidence
            .iter()
            .map(|item| {
                json!({
                    "evidenceId": &item.evidence_id,
                    "sourceType": item.source_type.as_str(),
                    "sourceId": &item.source_id,
                    "title": &item.title,
                    "content": &item.content,
                    "metadata": &item.metadata,
                })
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|source| {
        ApiError::internal(format!("failed to serialize extraction evidence: {source}"))
    })?;

    Ok(NeutralChatRequest {
        model_id: model_id.to_string(),
        messages: vec![
            neutral_text_message(
                NeutralChatRole::System,
                MEMORY_EXTRACTION_SYSTEM_PROMPT.to_string(),
            ),
            neutral_text_message(
                NeutralChatRole::User,
                format!(
                    "workspaceId: {workspace_id}\nchatId: {chat_id}\nrunId: {run_id}\nproviderId: {provider_id}\n\nExisting memory candidates JSON:\n{existing_memories_json}\n\nEvidence JSON:\n{evidence_json}"
                ),
            ),
        ],
        tools: vec![memory_extraction_tool_definition()],
        thinking_level: None,
        max_output_tokens: Some(max_output_tokens),
        prompt_cache_key: None,
        prompt_cache_retention: None,
    })
}

pub(crate) async fn call_memory_extraction_provider(
    workspace_path: &Path,
    workspace_id: &str,
    chat_id: Option<&str>,
    provider_id: &str,
    provider_config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
    retry_count: u32,
    save_details: bool,
) -> Result<Value, ApiError> {
    audited_provider_tool_request(
        workspace_path,
        workspace_id,
        chat_id,
        provider_id,
        provider_config,
        request,
        "memory extraction",
        MEMORY_EXTRACTION_TOOL_NAME,
        "submit tool",
        MEMORY_EXTRACTION_TIMEOUT_MS,
        retry_count,
        save_details,
    )
    .await
}

pub(crate) async fn call_memory_retrieval_provider(
    workspace_path: &Path,
    workspace_id: &str,
    chat_id: Option<&str>,
    provider_id: &str,
    provider_config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
    retry_count: u32,
    save_details: bool,
) -> Result<Value, ApiError> {
    audited_provider_tool_request(
        workspace_path,
        workspace_id,
        chat_id,
        provider_id,
        provider_config,
        request,
        "memory retrieval",
        MEMORY_RETRIEVAL_TOOL_NAME,
        "select tool",
        MEMORY_RETRIEVAL_TIMEOUT_MS,
        retry_count,
        save_details,
    )
    .await
}

pub(crate) fn parse_memory_extraction_output(
    value: Value,
) -> Result<MemoryExtractionOutput, ApiError> {
    serde_json::from_value(value).map_err(|source| {
        ApiError::bad_request(format!("malformed memory extraction JSON: {source}"))
    })
}

pub(crate) fn memory_extraction_error_should_be_ignored(error_message: Option<&str>) -> bool {
    let Some(message) = error_message else {
        return false;
    };
    message.starts_with("malformed memory extraction JSON:")
        || message == "memory extraction did not call submit tool"
        || message.starts_with("memory extraction returned text instead of submit tool:")
        || message.starts_with("memory extraction called unsupported tool ")
        || message.starts_with("memory extraction completed with unsupported tool ")
        || message.starts_with("extracted fact ")
}

pub(crate) fn parse_memory_retrieval_output(
    value: Value,
) -> Result<MemoryRetrievalOutput, ApiError> {
    serde_json::from_value(value).map_err(|source| {
        ApiError::bad_request(format!("malformed memory retrieval JSON: {source}"))
    })
}

pub(crate) fn store_extracted_memory_facts(
    task: &MemoryExtractionTask,
    evidence_candidates: &[MemoryExtractionEvidenceCandidate],
    output: &MemoryExtractionOutput,
) -> Result<Vec<ChatExtractedMemorySummary>, ApiError> {
    let evidence_by_id = evidence_candidates
        .iter()
        .map(|item| (item.evidence_id.as_str(), item))
        .collect::<HashMap<_, _>>();
    let validated_facts = validate_extracted_memory_facts(output, &evidence_by_id)?;
    if validated_facts.is_empty() {
        return Ok(Vec::new());
    }

    let mut global_memory_database: Option<MemoryDatabase> = None;
    let mut workspace_memory_database =
        MemoryDatabase::open_workspace_at(workspace_database_path(&task.workspace_path))
            .map_err(ApiError::from_memory_error)?;
    let mut summaries = Vec::new();

    for fact in validated_facts {
        let source_ids = fact
            .evidence_ids
            .iter()
            .enumerate()
            .map(|(index, _)| format!("{}-source-{index}", unique_id("memory-source")))
            .collect::<Vec<_>>();
        let source_id_refs = source_ids.iter().map(String::as_str).collect::<Vec<_>>();
        let database = match fact.scope {
            MemoryScope::Global => {
                if global_memory_database.is_none() {
                    global_memory_database = Some(
                        MemoryDatabase::open_or_create_global_at(&task.global_memory_database_file)
                            .map_err(ApiError::from_memory_error)?,
                    );
                }
                global_memory_database
                    .as_mut()
                    .expect("global memory database should be initialized")
            }
            MemoryScope::Workspace | MemoryScope::Chat => &mut workspace_memory_database,
        };

        for (index, evidence_id) in fact.evidence_ids.iter().enumerate() {
            let evidence = evidence_by_id
                .get(evidence_id.as_str())
                .expect("validated evidence id should exist");
            let source_metadata_json = serde_json::to_string(&json!({
                "extractionJobId": &task.job_id,
                "evidenceId": &evidence.evidence_id,
                "runId": &task.run_id,
                "metadata": &evidence.metadata,
            }))
            .map_err(|source| {
                ApiError::internal(format!(
                    "failed to serialize memory source metadata: {source}"
                ))
            })?;
            database
                .insert_source(NewMemorySource {
                    id: &source_ids[index],
                    scope: fact.scope,
                    chat_id: (fact.scope == MemoryScope::Chat).then_some(task.chat_id.as_str()),
                    source_type: evidence.source_type,
                    source_id: Some(&evidence.source_id),
                    title: &evidence.title,
                    content: &evidence.content,
                    metadata_json: &source_metadata_json,
                })
                .map_err(ApiError::from_memory_error)?;
        }

        let fact_id = unique_id("memory-fact");
        database
            .insert_fact(NewMemoryFact {
                id: &fact_id,
                scope: fact.scope,
                chat_id: (fact.scope == MemoryScope::Chat).then_some(task.chat_id.as_str()),
                status: task.target_status,
                kind: fact.kind,
                fact: &fact.fact,
                confidence: fact.confidence,
                pinned: false,
                source_ids: &source_id_refs,
                metadata_json: &fact.metadata_json,
            })
            .map_err(ApiError::from_memory_error)?;
        insert_extracted_memory_edges(database, &task.job_id, &fact_id, &fact.relation_candidates)?;
        apply_memory_expiration_to_fact(database, &fact_id, &task.config.memory)?;
        let stored_fact = database
            .fact(&fact_id)
            .map_err(ApiError::from_memory_error)?
            .ok_or_else(|| ApiError::internal(format!("memory fact was not found: {fact_id}")))?;
        summaries.push(chat_extracted_memory_summary(stored_fact));
        refresh_memory_profile(
            database,
            fact.scope,
            (fact.scope == MemoryScope::Chat).then_some(task.chat_id.as_str()),
        )?;
    }

    Ok(summaries)
}

pub(crate) fn validate_extracted_memory_facts(
    output: &MemoryExtractionOutput,
    evidence_by_id: &HashMap<&str, &MemoryExtractionEvidenceCandidate>,
) -> Result<Vec<ValidatedExtractedMemoryFact>, ApiError> {
    let mut validated = Vec::with_capacity(output.facts.len());

    for (index, fact) in output.facts.iter().enumerate() {
        let scope = MemoryScope::parse(fact.scope.trim()).map_err(ApiError::from_memory_error)?;
        let kind = MemoryKind::parse(fact.kind.trim()).map_err(ApiError::from_memory_error)?;
        if kind == MemoryKind::UserNote {
            return Err(ApiError::bad_request(format!(
                "extracted fact {index} must not use user_note kind"
            )));
        }
        let fact_text = fact.fact.trim();
        if fact_text.is_empty() {
            return Err(ApiError::bad_request(format!(
                "extracted fact {index} text must not be empty"
            )));
        }
        if let Some(confidence) = fact.confidence
            && !(0.0..=1.0).contains(&confidence)
        {
            return Err(ApiError::bad_request(format!(
                "extracted fact {index} confidence must be between 0 and 1"
            )));
        }
        if fact.evidence_references.is_empty() {
            return Err(ApiError::bad_request(format!(
                "extracted fact {index} must include at least one evidence reference"
            )));
        }

        let mut evidence_ids = Vec::new();
        for reference in &fact.evidence_references {
            let evidence_id = reference.evidence_id.trim();
            if evidence_id.is_empty() {
                return Err(ApiError::bad_request(format!(
                    "extracted fact {index} evidence id must not be empty"
                )));
            }
            if !evidence_by_id.contains_key(evidence_id) {
                return Err(ApiError::bad_request(format!(
                    "extracted fact {index} references unknown evidence id '{evidence_id}'"
                )));
            }
            if !evidence_ids.iter().any(|id| id == evidence_id) {
                evidence_ids.push(evidence_id.to_string());
            }
        }

        let mut relation_candidates = Vec::new();
        for relation in &fact.relation_candidates {
            let Some(relation_kind) = memory_relation_kind_from_str(&relation.relation) else {
                return Err(ApiError::bad_request(format!(
                    "extracted fact {index} has unsupported relation '{}'",
                    relation.relation
                )));
            };
            if let Some(target_fact_id) = relation
                .target_fact_id
                .as_deref()
                .and_then(|target| memory_relation_target_fact_id(target, scope))
            {
                relation_candidates.push(ValidatedExtractedMemoryRelationCandidate {
                    relation: relation_kind,
                    target_fact_id,
                    target_fact: normalized_optional_text(relation.target_fact.clone()),
                    reason: normalized_optional_text(relation.reason.clone()),
                });
            }
        }

        let metadata_json = serde_json::to_string(&json!({
            "source": "memory_extraction",
            "relationCandidates": &fact.relation_candidates,
            "evidenceReferences": &fact.evidence_references,
        }))
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to serialize extracted memory metadata: {source}"
            ))
        })?;

        validated.push(ValidatedExtractedMemoryFact {
            scope,
            kind,
            fact: fact_text.to_string(),
            confidence: fact.confidence,
            evidence_ids,
            relation_candidates,
            metadata_json,
        });
    }

    Ok(validated)
}

fn insert_extracted_memory_edges(
    database: &mut MemoryDatabase,
    extraction_job_id: &str,
    source_fact_id: &str,
    relation_candidates: &[ValidatedExtractedMemoryRelationCandidate],
) -> Result<(), ApiError> {
    // ponytail: only materialize explicit targetFactId links; add fact-text matching if extraction needs targetFact fallback.
    for relation in relation_candidates {
        if relation.target_fact_id == source_fact_id {
            continue;
        }
        if database
            .fact(&relation.target_fact_id)
            .map_err(ApiError::from_memory_error)?
            .is_none()
        {
            continue;
        }

        let metadata_json = serde_json::to_string(&json!({
            "source": "memory_extraction",
            "extractionJobId": extraction_job_id,
            "targetFact": relation.target_fact,
            "reason": relation.reason,
        }))
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to serialize memory edge metadata: {source}"
            ))
        })?;
        let edge_id = unique_id("memory-edge");
        database
            .insert_edge(NewMemoryEdge {
                id: &edge_id,
                source_fact_id,
                target_fact_id: &relation.target_fact_id,
                relation: relation.relation,
                metadata_json: &metadata_json,
            })
            .map_err(ApiError::from_memory_error)?;
    }

    Ok(())
}

fn memory_relation_kind_from_str(value: &str) -> Option<MemoryRelationKind> {
    match value.trim() {
        "updates" => Some(MemoryRelationKind::Updates),
        "extends" => Some(MemoryRelationKind::Extends),
        "derives" => Some(MemoryRelationKind::Derives),
        _ => None,
    }
}

fn memory_relation_target_fact_id(value: &str, source_scope: MemoryScope) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let Some((scope, fact_id)) = value.split_once(':') else {
        return Some(value.to_string());
    };
    let target_scope = MemoryScope::parse(scope.trim()).ok()?;
    if !memory_scopes_share_database(source_scope, target_scope) {
        return None;
    }
    let fact_id = fact_id.trim();
    if fact_id.is_empty() {
        None
    } else {
        Some(fact_id.to_string())
    }
}

fn memory_scopes_share_database(left: MemoryScope, right: MemoryScope) -> bool {
    match (left, right) {
        (MemoryScope::Global, MemoryScope::Global) => true,
        (MemoryScope::Global, _) | (_, MemoryScope::Global) => false,
        _ => true,
    }
}
