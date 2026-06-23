use std::collections::{HashMap, HashSet};

use axum::{
    Json,
    extract::{Path, Query, State},
};
use foco_store::memory::{
    MemoryDatabase, MemoryDreamChangeRecord, MemoryDreamChangeStatus, MemoryDreamJobRecord,
    MemoryDreamJobStatus, MemoryDreamRunMode, MemoryDreamScope, MemoryDreamTriggerType,
    MemoryExtractionJobStatus, MemoryFactRecord, MemoryKind, MemoryScope, MemorySourceRecord,
    MemoryStatus, NewMemoryFact, NewMemorySource, UpdateMemoryFact, UpdateMemorySource,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::memory_runtime::memory_extraction_error_should_be_ignored;
use crate::memory_runtime::run_memory_dream_for_state;
use crate::memory_runtime::{apply_memory_expiration_to_fact, expire_due_memories};
use crate::*;

const MEMORY_DREAM_JOBS_LIMIT_DEFAULT: u32 = 50;
const MEMORY_DREAM_JOBS_LIMIT_MAX: u32 = 200;
const MEMORY_DREAM_CHANGES_LIMIT_DEFAULT: u32 = 500;
const MEMORY_DREAM_CHANGES_LIMIT_MAX: u32 = 10_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryListQuery {
    scope: String,
    workspace_id: Option<String>,
    chat_id: Option<String>,
    query: Option<String>,
    status: Option<String>,
    kind: Option<String>,
    limit: Option<u32>,
    page: Option<u32>,
    page_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualMemoryRequest {
    scope: String,
    workspace_id: Option<String>,
    chat_id: Option<String>,
    kind: String,
    fact: String,
    confidence: Option<f64>,
    pinned: Option<bool>,
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryStatusRequest {
    scope: String,
    workspace_id: Option<String>,
    memory_id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditMemoryRequest {
    scope: String,
    workspace_id: Option<String>,
    memory_id: String,
    fact: Option<String>,
    kind: Option<String>,
    confidence: Option<f64>,
    pinned: Option<bool>,
    metadata: Option<Value>,
    sources: Option<Vec<EditMemorySourceRequest>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditMemorySourceRequest {
    pub(crate) id: String,
    pub(crate) title: Option<String>,
    pub(crate) content: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForgetMemoryRequest {
    scope: String,
    workspace_id: Option<String>,
    memory_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClearMemoriesRequest {
    scope: String,
    workspace_id: Option<String>,
    chat_id: Option<String>,
    query: Option<String>,
    status: Option<String>,
    kind: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClearMemoriesResponse {
    deleted_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromoteMemoryRequest {
    scope: String,
    workspace_id: Option<String>,
    memory_id: String,
    target_scope: String,
    target_workspace_id: Option<String>,
    target_chat_id: Option<String>,
    target_memory_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemorySourcesQuery {
    scope: String,
    workspace_id: Option<String>,
    memory_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamRunRequest {
    scope: String,
    workspace_id: Option<String>,
    trigger_type: String,
    mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamJobsQuery {
    scope: Option<String>,
    workspace_id: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamChangesQuery {
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryListResponse {
    memories: Vec<MemoryFactRecord>,
    extraction_jobs: Vec<MemoryExtractionJobSummary>,
    page: u32,
    page_size: u32,
    total_count: u32,
    total_pages: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryExtractionJobSummary {
    pub(crate) id: String,
    pub(crate) scope: String,
    pub(crate) chat_id: Option<String>,
    pub(crate) status: String,
    pub(crate) model_id: Option<String>,
    pub(crate) error_message: Option<String>,
    pub(crate) created_at: String,
    pub(crate) started_at: Option<String>,
    pub(crate) completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryMutationResponse {
    memory: Option<MemoryFactRecord>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemorySourcesResponse {
    sources: Vec<MemorySourceRecord>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamRunResponse {
    job_id: String,
    status: String,
    transcript_chat_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamJobsResponse {
    jobs: Vec<MemoryDreamJobSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamJobResponse {
    job: MemoryDreamJobSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamChangesResponse {
    changes: Vec<MemoryDreamChangeSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamJobSummary {
    id: String,
    scope: String,
    workspace_id: Option<String>,
    trigger_type: String,
    mode: String,
    status: String,
    model_id: Option<String>,
    transcript_chat_id: Option<String>,
    transcript_workspace_id: Option<String>,
    error_message: Option<String>,
    summary: Option<String>,
    change_counts: MemoryDreamChangeCounts,
    created_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamChangeCounts {
    added: u32,
    updated: u32,
    superseded: u32,
    expired: u32,
    rejected: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamChangeSummary {
    id: String,
    job_id: String,
    operation: String,
    target_fact_ids: Vec<String>,
    new_fact_id: Option<String>,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: String,
    confidence: Option<f64>,
    risk_level: String,
    status: String,
    evidence: Value,
    error_message: Option<String>,
    created_at: String,
    applied_at: Option<String>,
}

pub(crate) async fn memory_list(
    State(state): State<AppState>,
    Query(query): Query<MemoryListQuery>,
) -> Result<Json<MemoryListResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let scope = MemoryScope::parse(query.scope.trim()).map_err(ApiError::from_memory_error)?;
    let chat_id = normalized_optional_text(query.chat_id);
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.or(query.limit).unwrap_or(20).clamp(1, 200);
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(MemoryStatus::parse)
        .transpose()
        .map_err(ApiError::from_memory_error)?
        .unwrap_or(MemoryStatus::Active);
    let kind = query
        .kind
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(MemoryKind::parse)
        .transpose()
        .map_err(ApiError::from_memory_error)?;
    let mut database = open_memory_database(&state, &config, scope, query.workspace_id.as_deref())?;
    let query_text = normalized_optional_text(query.query);

    if scope == MemoryScope::Chat && chat_id.is_none() {
        return Err(ApiError::bad_request("chat memory listing requires chatId"));
    }

    expire_due_memories(&mut database)?;
    refresh_memory_profile(
        &mut database,
        scope,
        (scope == MemoryScope::Chat)
            .then_some(chat_id.as_deref())
            .flatten(),
    )?;

    let total_count = if status == MemoryStatus::Active {
        if let Some(query_text) = query_text.as_deref() {
            database
                .count_search_active_facts_for_scope(query_text, chat_id.as_deref(), kind)
                .map_err(ApiError::from_memory_error)?
        } else {
            database
                .count_facts_for_scope(chat_id.as_deref(), status, kind, None)
                .map_err(ApiError::from_memory_error)?
        }
    } else {
        database
            .count_facts_for_scope(chat_id.as_deref(), status, kind, query_text.as_deref())
            .map_err(ApiError::from_memory_error)?
    };
    let memories = if status == MemoryStatus::Active {
        if let Some(query_text) = query_text.as_deref() {
            database
                .search_active_facts_for_scope_page(
                    query_text,
                    chat_id.as_deref(),
                    kind,
                    page_size,
                    offset,
                )
                .map_err(ApiError::from_memory_error)?
        } else {
            database
                .list_facts_for_scope_page(
                    chat_id.as_deref(),
                    status,
                    kind,
                    None,
                    page_size,
                    offset,
                )
                .map_err(ApiError::from_memory_error)?
        }
    } else {
        database
            .list_facts_for_scope_page(
                chat_id.as_deref(),
                status,
                kind,
                query_text.as_deref(),
                page_size,
                offset,
            )
            .map_err(ApiError::from_memory_error)?
    };
    let extraction_jobs = memory_extraction_job_summaries(
        scope,
        &database,
        chat_id.as_deref(),
        MemoryExtractionJobStatus::Failed,
        20,
    )?;

    Ok(Json(MemoryListResponse {
        memories,
        extraction_jobs,
        page,
        page_size,
        total_count,
        total_pages: if total_count == 0 {
            0
        } else {
            total_count.div_ceil(page_size)
        },
    }))
}

pub(crate) async fn create_manual_memory(
    State(state): State<AppState>,
    Json(request): Json<ManualMemoryRequest>,
) -> Result<Json<MemoryMutationResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let scope = MemoryScope::parse(request.scope.trim()).map_err(ApiError::from_memory_error)?;
    let kind = MemoryKind::parse(request.kind.trim()).map_err(ApiError::from_memory_error)?;
    let chat_id = normalized_optional_text(request.chat_id);
    let mut database =
        open_memory_database(&state, &config, scope, request.workspace_id.as_deref())?;
    let fact = request.fact.trim().to_string();

    if fact.is_empty() {
        return Err(ApiError::bad_request("memory fact must not be empty"));
    }

    let metadata_json = memory_metadata_json(request.metadata)?;
    let source_id = unique_id("memory-source");
    let memory_id = unique_id("memory-fact");
    database
        .insert_source(NewMemorySource {
            id: &source_id,
            scope,
            chat_id: chat_id.as_deref(),
            source_type: foco_store::memory::MemorySourceType::ManualNote,
            source_id: None,
            title: "Manual memory",
            content: &fact,
            metadata_json: &metadata_json,
        })
        .map_err(ApiError::from_memory_error)?;
    database
        .insert_fact(NewMemoryFact {
            id: &memory_id,
            scope,
            chat_id: chat_id.as_deref(),
            status: MemoryStatus::Active,
            kind,
            fact: &fact,
            confidence: request.confidence,
            pinned: request.pinned.unwrap_or(false),
            source_ids: &[source_id.as_str()],
            metadata_json: &metadata_json,
        })
        .map_err(ApiError::from_memory_error)?;
    apply_memory_expiration_to_fact(&mut database, &memory_id, &config.memory)?;
    refresh_memory_profile(&mut database, scope, chat_id.as_deref())?;
    let memory = database
        .fact(&memory_id)
        .map_err(ApiError::from_memory_error)?;

    Ok(Json(MemoryMutationResponse { memory }))
}

pub(crate) fn memory_extraction_job_summaries(
    scope: MemoryScope,
    database: &MemoryDatabase,
    chat_id: Option<&str>,
    status: MemoryExtractionJobStatus,
    limit: u32,
) -> Result<Vec<MemoryExtractionJobSummary>, ApiError> {
    let fetch_limit = limit.saturating_mul(10).max(limit).min(200);
    let jobs = match scope {
        MemoryScope::Global => Vec::new(),
        MemoryScope::Chat => database
            .extraction_jobs_for_scope(chat_id, Some(status), fetch_limit)
            .map_err(ApiError::from_memory_error)?,
        MemoryScope::Workspace => database
            .extraction_jobs(Some(status), fetch_limit)
            .map_err(ApiError::from_memory_error)?,
    };

    Ok(jobs
        .into_iter()
        .filter(|job| !memory_extraction_error_should_be_ignored(job.error_message.as_deref()))
        .take(limit as usize)
        .map(|job| MemoryExtractionJobSummary {
            id: job.id,
            scope: job.scope,
            chat_id: job.chat_id,
            status: job.status,
            model_id: job.model_id,
            error_message: job.error_message,
            created_at: job.created_at,
            started_at: job.started_at,
            completed_at: job.completed_at,
        })
        .collect())
}

pub(crate) fn refresh_memory_profile(
    database: &mut MemoryDatabase,
    scope: MemoryScope,
    chat_id: Option<&str>,
) -> Result<(), ApiError> {
    database
        .refresh_profile_from_active_facts(scope, chat_id, MEMORY_PROFILE_REFRESH_FACT_LIMIT)
        .map(|_| ())
        .map_err(ApiError::from_memory_error)
}

pub(crate) async fn update_memory_status(
    State(state): State<AppState>,
    Json(request): Json<MemoryStatusRequest>,
) -> Result<Json<MemoryMutationResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let scope = MemoryScope::parse(request.scope.trim()).map_err(ApiError::from_memory_error)?;
    let status = MemoryStatus::parse(request.status.trim()).map_err(ApiError::from_memory_error)?;
    let memory_id = normalized_required_text("memoryId", &request.memory_id)?;
    let mut database =
        open_memory_database(&state, &config, scope, request.workspace_id.as_deref())?;

    database
        .set_fact_status(&memory_id, status)
        .map_err(ApiError::from_memory_error)?;
    let memory = database
        .fact(&memory_id)
        .map_err(ApiError::from_memory_error)?;
    if let Some(memory) = &memory {
        let memory_scope =
            MemoryScope::parse(&memory.scope).map_err(ApiError::from_memory_error)?;
        refresh_memory_profile(&mut database, memory_scope, memory.chat_id.as_deref())?;
    }

    Ok(Json(MemoryMutationResponse { memory }))
}

pub(crate) async fn edit_memory(
    State(state): State<AppState>,
    Json(request): Json<EditMemoryRequest>,
) -> Result<Json<MemoryMutationResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let scope = MemoryScope::parse(request.scope.trim()).map_err(ApiError::from_memory_error)?;
    let memory_id = normalized_required_text("memoryId", &request.memory_id)?;
    let fact = normalized_optional_text(request.fact);
    let metadata_json = optional_memory_metadata_json(request.metadata)?;
    let source_updates = memory_source_updates(request.sources)?;
    let kind = request
        .kind
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(MemoryKind::parse)
        .transpose()
        .map_err(ApiError::from_memory_error)?;
    let mut database =
        open_memory_database(&state, &config, scope, request.workspace_id.as_deref())?;

    if !source_updates.is_empty() {
        let linked_source_ids = database
            .sources_for_fact(&memory_id)
            .map_err(ApiError::from_memory_error)?
            .into_iter()
            .map(|source| source.id)
            .collect::<HashSet<_>>();
        for source_update in &source_updates {
            if !linked_source_ids.contains(&source_update.id) {
                return Err(ApiError::bad_request(format!(
                    "memory source '{}' is not linked to memory '{}'",
                    source_update.id, memory_id
                )));
            }
        }
    }

    database
        .update_fact(UpdateMemoryFact {
            id: &memory_id,
            kind,
            fact: fact.as_deref(),
            confidence: request.confidence,
            pinned: request.pinned,
            metadata_json: metadata_json.as_deref(),
            ..UpdateMemoryFact::default()
        })
        .map_err(ApiError::from_memory_error)?;
    for source_update in &source_updates {
        database
            .update_source(UpdateMemorySource {
                id: &source_update.id,
                title: source_update.title.as_deref(),
                content: source_update.content.as_deref(),
                metadata_json: source_update.metadata_json.as_deref(),
            })
            .map_err(ApiError::from_memory_error)?;
    }
    let memory = database
        .fact(&memory_id)
        .map_err(ApiError::from_memory_error)?;
    if let Some(memory) = &memory {
        let memory_scope =
            MemoryScope::parse(&memory.scope).map_err(ApiError::from_memory_error)?;
        refresh_memory_profile(&mut database, memory_scope, memory.chat_id.as_deref())?;
    }

    Ok(Json(MemoryMutationResponse { memory }))
}

pub(crate) async fn forget_memory(
    State(state): State<AppState>,
    Json(request): Json<ForgetMemoryRequest>,
) -> Result<Json<MemoryMutationResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let scope = MemoryScope::parse(request.scope.trim()).map_err(ApiError::from_memory_error)?;
    let memory_id = normalized_required_text("memoryId", &request.memory_id)?;
    let mut database =
        open_memory_database(&state, &config, scope, request.workspace_id.as_deref())?;
    let existing_memory = database
        .fact(&memory_id)
        .map_err(ApiError::from_memory_error)?;

    database
        .hard_delete_fact(&memory_id)
        .map_err(ApiError::from_memory_error)?;
    if let Some(memory) = &existing_memory {
        let memory_scope =
            MemoryScope::parse(&memory.scope).map_err(ApiError::from_memory_error)?;
        refresh_memory_profile(&mut database, memory_scope, memory.chat_id.as_deref())?;
    }

    Ok(Json(MemoryMutationResponse { memory: None }))
}

pub(crate) async fn clear_filtered_memories(
    State(state): State<AppState>,
    Json(request): Json<ClearMemoriesRequest>,
) -> Result<Json<ClearMemoriesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let scope = MemoryScope::parse(request.scope.trim()).map_err(ApiError::from_memory_error)?;

    if scope == MemoryScope::Global {
        return Err(ApiError::bad_request(
            "clearing filtered memories only supports workspace or chat scope",
        ));
    }

    let chat_id = normalized_optional_text(request.chat_id);
    if scope == MemoryScope::Chat && chat_id.is_none() {
        return Err(ApiError::bad_request(
            "chat memory clearing requires chatId",
        ));
    }

    let status = request
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(MemoryStatus::parse)
        .transpose()
        .map_err(ApiError::from_memory_error)?
        .unwrap_or(MemoryStatus::Active);
    let kind = request
        .kind
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(MemoryKind::parse)
        .transpose()
        .map_err(ApiError::from_memory_error)?;
    let query_text = normalized_optional_text(request.query);
    let mut database =
        open_memory_database(&state, &config, scope, request.workspace_id.as_deref())?;
    let exact_chat_id = (scope == MemoryScope::Chat)
        .then_some(chat_id.as_deref())
        .flatten();

    expire_due_memories(&mut database)?;
    let memory_ids = database
        .list_fact_ids_for_exact_scope(scope, exact_chat_id, status, kind, query_text.as_deref())
        .map_err(ApiError::from_memory_error)?;
    let mut deleted_count = 0;
    for memory_id in memory_ids {
        if database
            .hard_delete_fact(&memory_id)
            .map_err(ApiError::from_memory_error)?
        {
            deleted_count += 1;
        }
    }
    refresh_memory_profile(&mut database, scope, exact_chat_id)?;

    Ok(Json(ClearMemoriesResponse { deleted_count }))
}

pub(crate) async fn promote_memory(
    State(state): State<AppState>,
    Json(request): Json<PromoteMemoryRequest>,
) -> Result<Json<MemoryMutationResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let source_scope =
        MemoryScope::parse(request.scope.trim()).map_err(ApiError::from_memory_error)?;
    let target_scope =
        MemoryScope::parse(request.target_scope.trim()).map_err(ApiError::from_memory_error)?;
    let memory_id = normalized_required_text("memoryId", &request.memory_id)?;
    let target_memory_id = normalized_optional_text(request.target_memory_id)
        .unwrap_or_else(|| unique_id("memory-fact"));
    let target_chat_id = normalized_optional_text(request.target_chat_id);
    let same_workspace = request.workspace_id == request.target_workspace_id;
    let mut source_database = open_memory_database(
        &state,
        &config,
        source_scope,
        request.workspace_id.as_deref(),
    )?;

    let memory = if target_scope != MemoryScope::Global
        && source_scope != MemoryScope::Global
        && same_workspace
    {
        let memory = source_database
            .promote_fact(
                &memory_id,
                &target_memory_id,
                target_scope,
                target_chat_id.as_deref(),
            )
            .map_err(ApiError::from_memory_error)?;
        apply_memory_expiration_to_fact(&mut source_database, &target_memory_id, &config.memory)?;
        refresh_memory_profile(
            &mut source_database,
            target_scope,
            target_chat_id.as_deref(),
        )?;
        source_database
            .fact(&target_memory_id)
            .map_err(ApiError::from_memory_error)?
            .unwrap_or(memory)
    } else {
        let mut target_database = open_memory_database(
            &state,
            &config,
            target_scope,
            request.target_workspace_id.as_deref(),
        )?;
        let memory = source_database
            .promote_fact_to_database(
                &memory_id,
                &mut target_database,
                &target_memory_id,
                target_scope,
                target_chat_id.as_deref(),
            )
            .map_err(ApiError::from_memory_error)?;
        apply_memory_expiration_to_fact(&mut target_database, &target_memory_id, &config.memory)?;
        refresh_memory_profile(
            &mut target_database,
            target_scope,
            target_chat_id.as_deref(),
        )?;
        target_database
            .fact(&target_memory_id)
            .map_err(ApiError::from_memory_error)?
            .unwrap_or(memory)
    };

    let memory = Some(memory);

    Ok(Json(MemoryMutationResponse { memory }))
}

pub(crate) async fn memory_sources(
    State(state): State<AppState>,
    Query(query): Query<MemorySourcesQuery>,
) -> Result<Json<MemorySourcesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let scope = MemoryScope::parse(query.scope.trim()).map_err(ApiError::from_memory_error)?;
    let memory_id = normalized_required_text("memoryId", &query.memory_id)?;
    let database = open_memory_database(&state, &config, scope, query.workspace_id.as_deref())?;
    let sources = database
        .sources_for_fact(&memory_id)
        .map_err(ApiError::from_memory_error)?;

    Ok(Json(MemorySourcesResponse { sources }))
}

pub(crate) async fn run_memory_dream(
    State(state): State<AppState>,
    Json(request): Json<MemoryDreamRunRequest>,
) -> Result<Json<MemoryDreamRunResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    if !config.memory.enabled {
        return Err(ApiError::bad_request("memory is disabled"));
    }
    if !config.memory.dream.enabled {
        return Err(ApiError::bad_request("memory Dream is disabled"));
    }

    let scope =
        MemoryDreamScope::parse(request.scope.trim()).map_err(ApiError::from_memory_error)?;
    let workspace_id = normalized_optional_text(request.workspace_id);
    if scope == MemoryDreamScope::Global && workspace_id.is_some() {
        return Err(ApiError::bad_request(
            "global memory Dream must not include workspaceId",
        ));
    }
    if scope == MemoryDreamScope::Workspace && workspace_id.is_none() {
        return Err(ApiError::bad_request(
            "workspace memory Dream requires workspaceId",
        ));
    }

    let trigger_type = MemoryDreamTriggerType::parse(request.trigger_type.trim())
        .map_err(ApiError::from_memory_error)?;
    if trigger_type != MemoryDreamTriggerType::Manual {
        return Err(ApiError::bad_request(
            "manual memory Dream API only accepts triggerType 'manual'",
        ));
    }
    let mode = request
        .mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(config.memory.dream.mode.as_str());
    let mode = MemoryDreamRunMode::parse(mode).map_err(ApiError::from_memory_error)?;
    let run_state = state.clone();
    let run_config = config.clone();
    let run_workspace_id = workspace_id.clone();
    let result = tokio::spawn(async move {
        run_memory_dream_for_state(
            &run_state,
            &run_config,
            scope,
            run_workspace_id.as_deref(),
            MemoryDreamTriggerType::Manual,
            mode,
        )
        .await
    })
    .await
    .map_err(|source| ApiError::internal(format!("memory Dream task failed: {source}")))??;

    Ok(Json(MemoryDreamRunResponse {
        job_id: result.job.id,
        status: result.job.status,
        transcript_chat_id: result.job.transcript_chat_id,
    }))
}

pub(crate) async fn memory_dream_jobs(
    State(state): State<AppState>,
    Query(query): Query<MemoryDreamJobsQuery>,
) -> Result<Json<MemoryDreamJobsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let scope = query
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(MemoryDreamScope::parse)
        .transpose()
        .map_err(ApiError::from_memory_error)?;
    let scope = if query.workspace_id.is_some() && scope.is_none() {
        Some(MemoryDreamScope::Workspace)
    } else {
        scope
    };
    if scope == Some(MemoryDreamScope::Global) && query.workspace_id.is_some() {
        return Err(ApiError::bad_request(
            "global memory Dream jobs must not include workspaceId",
        ));
    }
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(MemoryDreamJobStatus::parse)
        .transpose()
        .map_err(ApiError::from_memory_error)?;
    let limit = dream_limit(
        query.limit,
        MEMORY_DREAM_JOBS_LIMIT_DEFAULT,
        MEMORY_DREAM_JOBS_LIMIT_MAX,
    );
    let workspace_id = normalized_optional_text(query.workspace_id);
    let mut jobs = Vec::new();

    if scope.is_none() || scope == Some(MemoryDreamScope::Global) {
        let database = open_dream_memory_database(&state, &config, MemoryDreamScope::Global, None)?;
        let transcript_workspaces = memory_dream_transcript_workspace_ids(&config)?;
        for job in database
            .dream_jobs_for_scope(MemoryDreamScope::Global, None, status, limit)
            .map_err(ApiError::from_memory_error)?
        {
            let transcript_workspace_id = job
                .transcript_chat_id
                .as_ref()
                .and_then(|chat_id| transcript_workspaces.get(chat_id).cloned());
            jobs.push(memory_dream_job_summary(
                &database,
                job,
                transcript_workspace_id,
            )?);
        }
    }

    if scope.is_none() || scope == Some(MemoryDreamScope::Workspace) {
        let workspaces = memory_dream_workspaces(&config, workspace_id.as_deref())?;
        for workspace in workspaces {
            let database = open_dream_memory_database(
                &state,
                &config,
                MemoryDreamScope::Workspace,
                Some(&workspace.id),
            )?;
            for job in database
                .dream_jobs_for_scope(
                    MemoryDreamScope::Workspace,
                    Some(&workspace.id),
                    status,
                    limit,
                )
                .map_err(ApiError::from_memory_error)?
            {
                jobs.push(memory_dream_job_summary(
                    &database,
                    job,
                    Some(workspace.id.clone()),
                )?);
            }
        }
    }

    jobs.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    jobs.truncate(limit as usize);

    Ok(Json(MemoryDreamJobsResponse { jobs }))
}

pub(crate) async fn memory_dream_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<MemoryDreamJobResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let job_id = normalized_required_text("jobId", &job_id)?;
    let located = find_memory_dream_job(&state, &config, &job_id)?;
    let job = memory_dream_job_summary(
        &located.database,
        located.job,
        located.transcript_workspace_id,
    )?;

    Ok(Json(MemoryDreamJobResponse { job }))
}

pub(crate) async fn memory_dream_changes(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
    Query(query): Query<MemoryDreamChangesQuery>,
) -> Result<Json<MemoryDreamChangesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let job_id = normalized_required_text("jobId", &job_id)?;
    let located = find_memory_dream_job(&state, &config, &job_id)?;
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(MemoryDreamChangeStatus::parse)
        .transpose()
        .map_err(ApiError::from_memory_error)?;
    let limit = dream_limit(
        query.limit,
        MEMORY_DREAM_CHANGES_LIMIT_DEFAULT,
        MEMORY_DREAM_CHANGES_LIMIT_MAX,
    );
    let changes = located
        .database
        .dream_changes_for_job(&job_id, status, limit)
        .map_err(ApiError::from_memory_error)?
        .into_iter()
        .map(memory_dream_change_summary)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(MemoryDreamChangesResponse { changes }))
}

fn open_dream_memory_database(
    state: &AppState,
    config: &GlobalConfig,
    scope: MemoryDreamScope,
    workspace_id: Option<&str>,
) -> Result<MemoryDatabase, ApiError> {
    let memory_scope = match scope {
        MemoryDreamScope::Global => MemoryScope::Global,
        MemoryDreamScope::Workspace => MemoryScope::Workspace,
    };
    open_memory_database(state, config, memory_scope, workspace_id)
}

fn memory_dream_workspaces<'a>(
    config: &'a GlobalConfig,
    workspace_id: Option<&str>,
) -> Result<Vec<&'a WorkspaceConfig>, ApiError> {
    if let Some(workspace_id) = workspace_id {
        return Ok(vec![workspace_by_id(config, workspace_id)?]);
    }

    Ok(config.workspaces.iter().collect())
}

struct LocatedMemoryDreamJob {
    database: MemoryDatabase,
    job: MemoryDreamJobRecord,
    transcript_workspace_id: Option<String>,
}

fn find_memory_dream_job(
    state: &AppState,
    config: &GlobalConfig,
    job_id: &str,
) -> Result<LocatedMemoryDreamJob, ApiError> {
    let global_database =
        open_dream_memory_database(state, config, MemoryDreamScope::Global, None)?;
    if let Some(job) = global_database
        .dream_job(job_id)
        .map_err(ApiError::from_memory_error)?
    {
        let transcript_workspace_id =
            memory_dream_transcript_workspace_id(config, job.transcript_chat_id.as_deref())?;
        return Ok(LocatedMemoryDreamJob {
            database: global_database,
            job,
            transcript_workspace_id,
        });
    }

    for workspace in &config.workspaces {
        let database = open_dream_memory_database(
            state,
            config,
            MemoryDreamScope::Workspace,
            Some(&workspace.id),
        )?;
        if let Some(job) = database
            .dream_job(job_id)
            .map_err(ApiError::from_memory_error)?
        {
            return Ok(LocatedMemoryDreamJob {
                database,
                job,
                transcript_workspace_id: Some(workspace.id.clone()),
            });
        }
    }

    Err(ApiError::bad_request(format!(
        "memory Dream job was not found: {job_id}"
    )))
}

fn memory_dream_job_summary(
    database: &MemoryDatabase,
    job: MemoryDreamJobRecord,
    transcript_workspace_id: Option<String>,
) -> Result<MemoryDreamJobSummary, ApiError> {
    let applied_changes = database
        .dream_changes_for_job(
            &job.id,
            Some(MemoryDreamChangeStatus::Applied),
            MEMORY_DREAM_CHANGES_LIMIT_MAX,
        )
        .map_err(ApiError::from_memory_error)?;
    let change_counts = memory_dream_change_counts(&applied_changes);
    let summary = memory_dream_job_text_summary(&job)?;

    Ok(MemoryDreamJobSummary {
        id: job.id,
        scope: job.scope,
        workspace_id: job.workspace_id,
        trigger_type: job.trigger_type,
        mode: job.mode,
        status: job.status,
        model_id: job.model_id,
        transcript_chat_id: job.transcript_chat_id,
        transcript_workspace_id,
        error_message: job.error_message,
        summary,
        change_counts,
        created_at: job.created_at,
        started_at: job.started_at,
        completed_at: job.completed_at,
    })
}

fn memory_dream_job_text_summary(job: &MemoryDreamJobRecord) -> Result<Option<String>, ApiError> {
    let output = job
        .output_summary_json
        .as_deref()
        .map(|value| memory_dream_json(value, "output_summary_json"))
        .transpose()?;
    if let Some(summary) = output
        .as_ref()
        .and_then(|value| value.get("summary"))
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        return Ok(Some(summary));
    }
    if let Some(output) = output {
        let applied = output
            .get("changesApplied")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let skipped = output
            .get("changesSkipped")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let failed = output
            .get("changesFailed")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        return Ok(Some(format!(
            "{applied} changes applied, {skipped} skipped, {failed} failed"
        )));
    }

    Ok(job.error_message.clone())
}

fn memory_dream_change_counts(changes: &[MemoryDreamChangeRecord]) -> MemoryDreamChangeCounts {
    let mut counts = MemoryDreamChangeCounts::default();
    for change in changes {
        match change.operation.as_str() {
            "promote_to_global" => counts.added += 1,
            "update" | "repair_updates_chain" | "add_edge" => counts.updated += 1,
            "supersede" | "merge" => counts.superseded += 1,
            "expire" => counts.expired += 1,
            "reject" => counts.rejected += 1,
            _ => {}
        }
    }
    counts
}

fn memory_dream_change_summary(
    change: MemoryDreamChangeRecord,
) -> Result<MemoryDreamChangeSummary, ApiError> {
    Ok(MemoryDreamChangeSummary {
        id: change.id,
        job_id: change.job_id,
        operation: change.operation,
        target_fact_ids: memory_dream_target_fact_ids(&change.target_fact_ids_json)?,
        new_fact_id: change.new_fact_id,
        before_json: optional_memory_dream_json(change.before_json, "before_json")?,
        after_json: optional_memory_dream_json(change.after_json, "after_json")?,
        reason: change.reason,
        confidence: change.confidence,
        risk_level: change.risk_level,
        status: change.status,
        evidence: memory_dream_json(&change.evidence_json, "evidence_json")?,
        error_message: change.error_message,
        created_at: change.created_at,
        applied_at: change.applied_at,
    })
}

fn memory_dream_target_fact_ids(value: &str) -> Result<Vec<String>, ApiError> {
    serde_json::from_str::<Vec<String>>(value).map_err(|source| {
        ApiError::internal(format!(
            "memory Dream target fact ids must be valid JSON: {source}"
        ))
    })
}

fn optional_memory_dream_json(
    value: Option<String>,
    field: &str,
) -> Result<Option<Value>, ApiError> {
    value
        .as_deref()
        .map(|value| memory_dream_json(value, field))
        .transpose()
}

fn memory_dream_json(value: &str, field: &str) -> Result<Value, ApiError> {
    serde_json::from_str::<Value>(value).map_err(|source| {
        ApiError::internal(format!("memory Dream {field} must be valid JSON: {source}"))
    })
}

fn memory_dream_transcript_workspace_ids(
    config: &GlobalConfig,
) -> Result<HashMap<String, String>, ApiError> {
    let mut transcript_workspaces = HashMap::new();
    for workspace in &config.workspaces {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        for chat in database
            .dream_transcript_chats()
            .map_err(ApiError::from_workspace_error)?
        {
            transcript_workspaces.insert(chat.id, workspace.id.clone());
        }
    }

    Ok(transcript_workspaces)
}

fn memory_dream_transcript_workspace_id(
    config: &GlobalConfig,
    transcript_chat_id: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let Some(transcript_chat_id) = transcript_chat_id else {
        return Ok(None);
    };
    Ok(memory_dream_transcript_workspace_ids(config)?.remove(transcript_chat_id))
}

fn dream_limit(value: Option<u32>, default: u32, max: u32) -> u32 {
    value.unwrap_or(default).clamp(1, max)
}
