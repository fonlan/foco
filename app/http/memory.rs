use std::collections::HashSet;

use axum::{
    Json,
    extract::{Query, State},
};
use foco_store::memory::{
    MemoryDatabase, MemoryExtractionJobStatus, MemoryFactRecord, MemoryKind, MemoryScope,
    MemorySourceRecord, MemoryStatus, NewMemoryFact, NewMemorySource, UpdateMemoryFact,
    UpdateMemorySource,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::memory_runtime::memory_extraction_error_should_be_ignored;
use crate::memory_runtime::{apply_memory_expiration_to_fact, expire_due_memories};
use crate::*;

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
