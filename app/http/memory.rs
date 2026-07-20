use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};

use axum::{
    Json,
    extract::{Path, Query, State},
};
use foco_store::memory::{
    MemoryDatabase, MemoryDreamChangeRecord, MemoryDreamChangeStatus, MemoryDreamJobRecord,
    MemoryDreamJobStatus, MemoryDreamRunMode, MemoryDreamScope, MemoryDreamTriggerType,
    MemoryExtractionJobRecord, MemoryExtractionJobStatus, MemoryFactRecord, MemoryKind,
    MemoryScope, MemorySourceRecord, MemoryStatus, NewMemoryFact, NewMemorySource,
    UpdateMemoryFact, UpdateMemorySource,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::memory_runtime::{
    MemoryExtractionTask, memory_extraction_error_should_be_ignored, run_memory_extraction_job,
    spawn_manual_memory_dream_for_state,
};
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
pub(crate) struct MemoryEnabledRequest {
    scope: String,
    workspace_id: Option<String>,
    chat_id: Option<String>,
    fact_id: String,
    enabled: bool,
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
    page: Option<u32>,
    page_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamChangesQuery {
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryExtractionJobActionRequest {
    workspace_id: String,
    job_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryExtractionJobInput {
    workspace_id: String,
    chat_id: String,
    run_id: String,
    user_message_id: String,
    assistant_message_id: String,
    extraction_model_id: String,
    #[serde(default)]
    chat_model_id: Option<String>,
    target_status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryExtractionJobActionResponse {
    job: MemoryExtractionJobSummary,
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
    job: MemoryDreamJobSummary,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamJobsResponse {
    jobs: Vec<MemoryDreamJobSummary>,
    page: u32,
    page_size: u32,
    total_count: u32,
    total_pages: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    partial_unavailable: Vec<MemoryDreamPartialUnavailable>,
}

/// A bounded, safe diagnostic for a remote source that could not contribute.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamPartialUnavailable {
    workspace_id: String,
    reason: MemoryDreamRemoteUnavailableReason,
    message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum MemoryDreamRemoteUnavailableReason {
    NotConnected,
    RequestFailed,
    InvalidResponse,
}

impl MemoryDreamRemoteUnavailableReason {
    fn safe_message(self) -> &'static str {
        match self {
            Self::NotConnected => {
                "Remote Dream history is unavailable because the workspace is not connected."
            }
            Self::RequestFailed => "Remote Dream history is temporarily unavailable.",
            Self::InvalidResponse => "Remote Dream history returned an invalid response.",
        }
    }
}

impl MemoryDreamPartialUnavailable {
    fn new(workspace_id: &str, reason: MemoryDreamRemoteUnavailableReason) -> Self {
        Self {
            workspace_id: workspace_id.to_string(),
            reason,
            message: reason.safe_message().to_string(),
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

pub(crate) async fn retry_memory_extraction_job(
    State(state): State<AppState>,
    Json(request): Json<MemoryExtractionJobActionRequest>,
) -> Result<Json<MemoryExtractionJobActionResponse>, ApiError> {
    let workspace_id = request.workspace_id.trim();
    let job_id = request.job_id.trim();
    if job_id.is_empty() {
        return Err(ApiError::bad_request(
            "memory extraction job id must not be empty",
        ));
    }

    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, workspace_id)?;
    // Memory open runs workspace migrations under the shared gate; no separate
    // WorkspaceDatabase open needed (and would risk nested ordinary permits).
    let mut database = MemoryDatabase::open_or_create_workspace(&workspace.path)
        .map_err(ApiError::from_memory_error)?;
    let job = database
        .extraction_job(job_id)
        .map_err(ApiError::from_memory_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!("memory extraction job was not found: {job_id}"))
        })?;
    if job.status != MemoryExtractionJobStatus::Failed.as_str() {
        return Err(ApiError::conflict(
            "only failed memory extraction jobs can be retried",
        ));
    }

    let task = memory_extraction_task_from_job(
        &job,
        workspace_id,
        workspace.path.clone(),
        state.memory_database_file.clone(),
        config,
    )?;
    let handle = tokio::runtime::Handle::try_current()
        .map_err(|_| ApiError::internal("memory extraction retry requires an async runtime"))?;
    if !database
        .retry_failed_extraction_job(job_id, &task.model_id)
        .map_err(ApiError::from_memory_error)?
    {
        return Err(ApiError::conflict(
            "memory extraction job is no longer failed",
        ));
    }
    let job = database
        .extraction_job(job_id)
        .map_err(ApiError::from_memory_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!("memory extraction job was not found: {job_id}"))
        })?;
    spawn_memory_extraction_retry(handle, task);

    Ok(Json(MemoryExtractionJobActionResponse {
        job: memory_extraction_job_summary(job),
    }))
}

pub(crate) async fn skip_memory_extraction_job(
    State(state): State<AppState>,
    Json(request): Json<MemoryExtractionJobActionRequest>,
) -> Result<Json<MemoryExtractionJobActionResponse>, ApiError> {
    let workspace_id = request.workspace_id.trim();
    let job_id = request.job_id.trim();
    if job_id.is_empty() {
        return Err(ApiError::bad_request(
            "memory extraction job id must not be empty",
        ));
    }

    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, workspace_id)?;
    // Memory open runs workspace migrations under the shared gate; no separate
    // WorkspaceDatabase open needed (and would risk nested ordinary permits).
    let mut database = MemoryDatabase::open_or_create_workspace(&workspace.path)
        .map_err(ApiError::from_memory_error)?;
    let job = database
        .extraction_job(job_id)
        .map_err(ApiError::from_memory_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!("memory extraction job was not found: {job_id}"))
        })?;
    if job.status != MemoryExtractionJobStatus::Failed.as_str() {
        return Err(ApiError::conflict(
            "only failed memory extraction jobs can be skipped",
        ));
    }
    if !database
        .skip_failed_extraction_job(job_id)
        .map_err(ApiError::from_memory_error)?
    {
        return Err(ApiError::conflict(
            "memory extraction job is no longer failed",
        ));
    }
    let job = database
        .extraction_job(job_id)
        .map_err(ApiError::from_memory_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!("memory extraction job was not found: {job_id}"))
        })?;

    Ok(Json(MemoryExtractionJobActionResponse {
        job: memory_extraction_job_summary(job),
    }))
}

fn spawn_memory_extraction_retry(handle: tokio::runtime::Handle, task: MemoryExtractionTask) {
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
                "memory extraction retry failed"
            );
        }
    });
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
        .map(memory_extraction_job_summary)
        .collect())
}

fn memory_extraction_job_summary(job: MemoryExtractionJobRecord) -> MemoryExtractionJobSummary {
    MemoryExtractionJobSummary {
        id: job.id,
        scope: job.scope,
        chat_id: job.chat_id,
        status: job.status,
        model_id: job.model_id,
        error_message: job.error_message,
        created_at: job.created_at,
        started_at: job.started_at,
        completed_at: job.completed_at,
    }
}

pub(crate) fn memory_extraction_task_from_job(
    job: &MemoryExtractionJobRecord,
    workspace_id: &str,
    workspace_path: PathBuf,
    global_memory_database_file: PathBuf,
    config: GlobalConfig,
) -> Result<MemoryExtractionTask, ApiError> {
    let input: MemoryExtractionJobInput =
        serde_json::from_str(&job.input_json).map_err(|source| {
            ApiError::bad_request(format!("memory extraction job input is invalid: {source}"))
        })?;
    let target_status =
        MemoryStatus::parse(input.target_status.trim()).map_err(ApiError::from_memory_error)?;

    if input.workspace_id != workspace_id {
        return Err(ApiError::conflict(
            "memory extraction job belongs to another workspace",
        ));
    }
    if job.scope != MemoryScope::Chat.as_str()
        || job.chat_id.as_deref() != Some(input.chat_id.as_str())
    {
        return Err(ApiError::conflict(
            "memory extraction job target does not match its input",
        ));
    }

    let model_id = config
        .memory
        .extraction_model_id
        .as_deref()
        .or(input.chat_model_id.as_deref())
        .unwrap_or(&input.extraction_model_id)
        .to_string();

    Ok(MemoryExtractionTask {
        job_id: job.id.clone(),
        workspace_id: input.workspace_id,
        workspace_path,
        global_memory_database_file,
        chat_id: input.chat_id,
        run_id: input.run_id,
        user_message_id: input.user_message_id,
        assistant_message_id: input.assistant_message_id,
        model_id,
        target_status,
        config,
    })
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

pub(crate) async fn update_memory_enabled(
    State(state): State<AppState>,
    Json(request): Json<MemoryEnabledRequest>,
) -> Result<Json<MemoryMutationResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let scope = MemoryScope::parse(request.scope.trim()).map_err(ApiError::from_memory_error)?;
    let chat_id = normalized_optional_text(request.chat_id);
    if scope == MemoryScope::Chat && chat_id.is_none() {
        return Err(ApiError::bad_request(
            "chat memory enabled updates require chatId",
        ));
    }
    if scope != MemoryScope::Chat && chat_id.is_some() {
        return Err(ApiError::bad_request(
            "chatId is only valid for chat memory enabled updates",
        ));
    }

    let fact_id = normalized_required_text("factId", &request.fact_id)?;
    let mut database =
        open_memory_database(&state, &config, scope, request.workspace_id.as_deref())?;
    let current = database
        .fact(&fact_id)
        .map_err(ApiError::from_memory_error)?
        .ok_or_else(|| {
            ApiError::from_status_message(
                StatusCode::NOT_FOUND,
                format!("memory fact was not found: {fact_id}"),
            )
        })?;
    let current_scope = MemoryScope::parse(&current.scope).map_err(ApiError::from_memory_error)?;
    if current_scope != scope || current.chat_id.as_deref() != chat_id.as_deref() {
        return Err(ApiError::from_status_message(
            StatusCode::NOT_FOUND,
            format!("memory fact was not found: {fact_id}"),
        ));
    }

    let memory = database
        .set_fact_enabled(&fact_id, request.enabled)
        .map_err(ApiError::from_memory_error)?;
    refresh_memory_profile(&mut database, scope, chat_id.as_deref())?;

    Ok(Json(MemoryMutationResponse {
        memory: Some(memory),
    }))
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
    let result =
        spawn_manual_memory_dream_for_state(&state, &config, scope, workspace_id.as_deref(), mode)
            .await?;
    let database = open_dream_memory_database(&state, &config, scope, workspace_id.as_deref())?;
    let transcript_workspace_id = memory_dream_transcript_workspace_id_for_job(&config, &result)?;
    let job = memory_dream_job_summary(&database, result.clone(), transcript_workspace_id)?;

    Ok(Json(MemoryDreamRunResponse {
        job_id: result.id,
        status: result.status,
        transcript_chat_id: result.transcript_chat_id,
        job,
    }))
}

pub(crate) async fn memory_dream_jobs(
    State(state): State<AppState>,
    Query(query): Query<MemoryDreamJobsQuery>,
) -> Result<Json<MemoryDreamJobsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = normalized_optional_text(query.workspace_id);
    let scope = query
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(MemoryDreamScope::parse)
        .transpose()
        .map_err(ApiError::from_memory_error)?;
    let scope = if workspace_id.is_some() && scope.is_none() {
        Some(MemoryDreamScope::Workspace)
    } else {
        scope
    };
    if scope == Some(MemoryDreamScope::Global) && workspace_id.is_some() {
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
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query
        .page_size
        .or(query.limit)
        .unwrap_or(MEMORY_DREAM_JOBS_LIMIT_DEFAULT)
        .clamp(1, MEMORY_DREAM_JOBS_LIMIT_MAX);
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    let fetch_limit = offset.saturating_add(page_size);
    let mut candidates = Vec::new();
    let mut partial_unavailable = Vec::new();
    let mut total_count = 0_u32;

    if scope.is_none() || scope == Some(MemoryDreamScope::Global) {
        let database = open_dream_memory_database(&state, &config, MemoryDreamScope::Global, None)?;
        total_count = total_count.saturating_add(
            database
                .count_dream_jobs_for_scope(MemoryDreamScope::Global, None, status)
                .map_err(ApiError::from_memory_error)?,
        );
        candidates.extend(
            database
                .dream_jobs_for_scope_page(MemoryDreamScope::Global, None, status, fetch_limit, 0)
                .map_err(ApiError::from_memory_error)?
                .into_iter()
                .map(|job| {
                    MemoryDreamJobCandidate::Local(PendingMemoryDreamJob {
                        job,
                        source_workspace_id: None,
                    })
                }),
        );
    }

    if scope.is_none() || scope == Some(MemoryDreamScope::Workspace) {
        for workspace in memory_dream_workspace_sources(&config, workspace_id.as_deref())? {
            match workspace {
                MemoryDreamWorkspaceSource::Local(workspace) => {
                    let database = open_dream_memory_database(
                        &state,
                        &config,
                        MemoryDreamScope::Workspace,
                        Some(&workspace.id),
                    )?;
                    total_count = total_count.saturating_add(
                        database
                            .count_dream_jobs_for_scope(
                                MemoryDreamScope::Workspace,
                                Some(&workspace.id),
                                status,
                            )
                            .map_err(ApiError::from_memory_error)?,
                    );
                    candidates.extend(
                        database
                            .dream_jobs_for_scope_page(
                                MemoryDreamScope::Workspace,
                                Some(&workspace.id),
                                status,
                                fetch_limit,
                                0,
                            )
                            .map_err(ApiError::from_memory_error)?
                            .into_iter()
                            .map(|job| {
                                MemoryDreamJobCandidate::Local(PendingMemoryDreamJob {
                                    job,
                                    source_workspace_id: Some(workspace.id.clone()),
                                })
                            }),
                    );
                }
                MemoryDreamWorkspaceSource::Remote(workspace) => {
                    match fetch_remote_memory_dream_jobs(&state, &workspace.id, status, fetch_limit)
                        .await
                    {
                        Ok(response) => {
                            total_count = total_count.saturating_add(response.total_count);
                            candidates.extend(
                                response
                                    .jobs
                                    .into_iter()
                                    .map(MemoryDreamJobCandidate::Remote),
                            );
                        }
                        Err(reason) => partial_unavailable
                            .push(MemoryDreamPartialUnavailable::new(&workspace.id, reason)),
                    }
                }
            }
        }
    }

    let page_jobs = select_memory_dream_job_candidates_page(candidates, offset, page_size);
    let jobs = materialize_memory_dream_job_candidates_for_page(&state, &config, page_jobs)?;

    Ok(Json(MemoryDreamJobsResponse {
        jobs,
        page,
        page_size,
        total_count,
        total_pages: if total_count == 0 {
            0
        } else {
            total_count.div_ceil(page_size)
        },
        partial_unavailable,
    }))
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
) -> Result<foco_store::OpenedMemoryDatabase, ApiError> {
    let memory_scope = match scope {
        MemoryDreamScope::Global => MemoryScope::Global,
        MemoryDreamScope::Workspace => MemoryScope::Workspace,
    };
    open_memory_database(state, config, memory_scope, workspace_id)
}

enum MemoryDreamWorkspaceSource<'a> {
    Local(&'a WorkspaceConfig),
    Remote(&'a WorkspaceConfig),
}

fn memory_dream_workspace_sources<'a>(
    config: &'a GlobalConfig,
    workspace_id: Option<&str>,
) -> Result<Vec<MemoryDreamWorkspaceSource<'a>>, ApiError> {
    if let Some(workspace_id) = workspace_id {
        let workspace = workspace_by_id(config, workspace_id)?;
        return Ok(vec![if workspace.server_id().is_some() {
            MemoryDreamWorkspaceSource::Remote(workspace)
        } else {
            MemoryDreamWorkspaceSource::Local(workspace)
        }]);
    }

    let mut sources = config
        .local_workspaces()
        .map(MemoryDreamWorkspaceSource::Local)
        .collect::<Vec<_>>();
    sources.extend(
        config
            .workspaces
            .iter()
            .filter(|workspace| workspace.server_id().is_some())
            .map(MemoryDreamWorkspaceSource::Remote),
    );
    Ok(sources)
}

async fn fetch_remote_memory_dream_jobs(
    state: &AppState,
    workspace_id: &str,
    status: Option<MemoryDreamJobStatus>,
    fetch_limit: u32,
) -> Result<MemoryDreamJobsResponse, MemoryDreamRemoteUnavailableReason> {
    let (base, token) = match crate::remote_workspace::sidecar_proxy_target(state, workspace_id) {
        Ok(crate::remote_workspace::SidecarProxyTarget::Connected { base, token }) => (base, token),
        Ok(crate::remote_workspace::SidecarProxyTarget::Disconnected) => {
            return Err(MemoryDreamRemoteUnavailableReason::NotConnected);
        }
        Ok(crate::remote_workspace::SidecarProxyTarget::Local) => {
            return Err(MemoryDreamRemoteUnavailableReason::InvalidResponse);
        }
        Err(_) => return Err(MemoryDreamRemoteUnavailableReason::RequestFailed),
    };
    let status_query = status
        .map(|status| format!("&status={}", status.as_str()))
        .unwrap_or_default();
    let url = format!(
        "{}/api/remote/workspace/memory/dream/jobs?scope=workspace&page=1&pageSize={MEMORY_DREAM_JOBS_LIMIT_MAX}&fetchLimit={fetch_limit}{status_query}",
        base.trim_end_matches('/')
    );
    let response = reqwest::Client::new()
        .get(url)
        .bearer_auth(token)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|_| MemoryDreamRemoteUnavailableReason::RequestFailed)?;
    if !response.status().is_success() {
        return Err(MemoryDreamRemoteUnavailableReason::RequestFailed);
    }
    let response = response
        .json::<MemoryDreamJobsResponse>()
        .await
        .map_err(|_| MemoryDreamRemoteUnavailableReason::InvalidResponse)?;
    validate_remote_memory_dream_jobs_response(&response, workspace_id, status, fetch_limit)?;
    Ok(response)
}

fn validate_remote_memory_dream_jobs_response(
    response: &MemoryDreamJobsResponse,
    workspace_id: &str,
    status: Option<MemoryDreamJobStatus>,
    fetch_limit: u32,
) -> Result<(), MemoryDreamRemoteUnavailableReason> {
    if !response.partial_unavailable.is_empty()
        || response.total_count < response.jobs.len() as u32
        || response.jobs.len() as u32 > fetch_limit
    {
        return Err(MemoryDreamRemoteUnavailableReason::InvalidResponse);
    }
    for job in &response.jobs {
        if job.scope != "workspace"
            || job.workspace_id.as_deref() != Some(workspace_id)
            || status.is_some_and(|expected| job.status != expected.as_str())
        {
            return Err(MemoryDreamRemoteUnavailableReason::InvalidResponse);
        }
    }
    if response.jobs.windows(2).any(|jobs| {
        jobs[0].created_at < jobs[1].created_at
            || (jobs[0].created_at == jobs[1].created_at && jobs[0].id > jobs[1].id)
    }) {
        return Err(MemoryDreamRemoteUnavailableReason::InvalidResponse);
    }
    Ok(())
}

pub(crate) fn memory_dream_workspace_jobs_response(
    database: &MemoryDatabase,
    workspace_id: &str,
    status: Option<MemoryDreamJobStatus>,
    page: u32,
    page_size: u32,
    fetch_limit: Option<u32>,
) -> Result<MemoryDreamJobsResponse, ApiError> {
    let total_count = database
        .count_dream_jobs_for_scope(MemoryDreamScope::Workspace, Some(workspace_id), status)
        .map_err(ApiError::from_memory_error)?;
    let (limit, offset) = match fetch_limit {
        Some(limit) => (limit, 0),
        None => (page_size, page.saturating_sub(1).saturating_mul(page_size)),
    };
    let jobs = database
        .dream_jobs_for_scope_page(
            MemoryDreamScope::Workspace,
            Some(workspace_id),
            status,
            limit,
            offset,
        )
        .map_err(ApiError::from_memory_error)?
        .into_iter()
        .map(|job| memory_dream_job_summary(database, job, Some(workspace_id.to_string())))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(MemoryDreamJobsResponse {
        jobs,
        page,
        page_size,
        total_count,
        total_pages: if total_count == 0 {
            0
        } else {
            total_count.div_ceil(page_size)
        },
        partial_unavailable: Vec::new(),
    })
}

#[derive(Debug, Clone)]
enum MemoryDreamJobCandidate {
    Local(PendingMemoryDreamJob),
    Remote(MemoryDreamJobSummary),
}

impl MemoryDreamJobCandidate {
    fn created_at(&self) -> &str {
        match self {
            Self::Local(pending) => &pending.job.created_at,
            Self::Remote(summary) => &summary.created_at,
        }
    }

    fn id(&self) -> &str {
        match self {
            Self::Local(pending) => &pending.job.id,
            Self::Remote(summary) => &summary.id,
        }
    }
}

fn select_memory_dream_job_candidates_page(
    mut candidates: Vec<MemoryDreamJobCandidate>,
    offset: u32,
    page_size: u32,
) -> Vec<MemoryDreamJobCandidate> {
    candidates.sort_by(|left, right| {
        right
            .created_at()
            .cmp(left.created_at())
            .then_with(|| left.id().cmp(right.id()))
    });
    candidates
        .into_iter()
        .skip(offset as usize)
        .take(page_size as usize)
        .collect()
}

fn materialize_memory_dream_job_candidates_for_page(
    state: &AppState,
    config: &GlobalConfig,
    page_jobs: Vec<MemoryDreamJobCandidate>,
) -> Result<Vec<MemoryDreamJobSummary>, ApiError> {
    let local_jobs = page_jobs
        .iter()
        .filter_map(|candidate| match candidate {
            MemoryDreamJobCandidate::Local(pending) => Some(pending.clone()),
            MemoryDreamJobCandidate::Remote(_) => None,
        })
        .collect();
    let mut local_summaries =
        materialize_memory_dream_job_summaries_for_page(state, config, local_jobs)?.into_iter();
    let mut summaries = Vec::with_capacity(page_jobs.len());

    for candidate in page_jobs {
        match candidate {
            MemoryDreamJobCandidate::Local(_) => {
                let summary = local_summaries.next().ok_or_else(|| {
                    ApiError::internal("Dream job summary materialization lost a local candidate")
                })?;
                summaries.push(summary);
            }
            MemoryDreamJobCandidate::Remote(summary) => summaries.push(summary),
        }
    }
    Ok(summaries)
}

/// Sortable Dream job row collected before summary / legacy transcript resolution.
#[derive(Debug, Clone)]
pub(crate) struct PendingMemoryDreamJob {
    pub(crate) job: MemoryDreamJobRecord,
    /// Workspace that owns the job store for workspace-scoped jobs.
    pub(crate) source_workspace_id: Option<String>,
}

/// Observable counters for legacy transcript workspace resolution (tests).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LegacyTranscriptLookupStats {
    pub(crate) workspace_opens: usize,
    pub(crate) batch_existence_queries: usize,
}

/// Sort by `created_at DESC, id ASC` and take the requested page.
#[cfg(test)]
pub(crate) fn select_memory_dream_jobs_page(
    mut pending: Vec<PendingMemoryDreamJob>,
    offset: u32,
    page_size: u32,
) -> Vec<PendingMemoryDreamJob> {
    pending.sort_by(|left, right| {
        right
            .job
            .created_at
            .cmp(&left.job.created_at)
            .then_with(|| left.job.id.cmp(&right.job.id))
    });
    pending
        .into_iter()
        .skip(offset as usize)
        .take(page_size as usize)
        .collect()
}

fn materialize_memory_dream_job_summaries_for_page(
    state: &AppState,
    config: &GlobalConfig,
    page_jobs: Vec<PendingMemoryDreamJob>,
) -> Result<Vec<MemoryDreamJobSummary>, ApiError> {
    if page_jobs.is_empty() {
        return Ok(Vec::new());
    }

    let transcript_by_job_id =
        resolve_page_transcript_workspace_ids(config, &page_jobs, |workspace| {
            WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)
        })?;

    let mut global_database = None;
    let mut workspace_databases: HashMap<String, foco_store::OpenedMemoryDatabase> = HashMap::new();
    let mut summaries = Vec::with_capacity(page_jobs.len());

    for pending in page_jobs {
        let transcript_workspace_id = transcript_by_job_id.get(&pending.job.id).cloned().flatten();
        let database = if let Some(workspace_id) = pending.source_workspace_id.as_deref() {
            if !workspace_databases.contains_key(workspace_id) {
                let opened = open_dream_memory_database(
                    state,
                    config,
                    MemoryDreamScope::Workspace,
                    Some(workspace_id),
                )?;
                workspace_databases.insert(workspace_id.to_string(), opened);
            }
            workspace_databases
                .get(workspace_id)
                .expect("workspace memory database was just inserted")
        } else {
            if global_database.is_none() {
                global_database = Some(open_dream_memory_database(
                    state,
                    config,
                    MemoryDreamScope::Global,
                    None,
                )?);
            }
            global_database
                .as_ref()
                .expect("global memory database was just opened")
        };
        summaries.push(memory_dream_job_summary(
            database,
            pending.job,
            transcript_workspace_id,
        )?);
    }

    Ok(summaries)
}

/// Resolve transcript workspace ids for the final list page only.
///
/// New Global jobs use persisted `transcriptWorkspaceId`; workspace jobs use their
/// job workspace; only Global legacy jobs missing that field fall back to a bounded
/// per-workspace chat PK/IN lookup.
pub(crate) fn resolve_page_transcript_workspace_ids<F>(
    config: &GlobalConfig,
    page_jobs: &[PendingMemoryDreamJob],
    open_workspace: F,
) -> Result<HashMap<String, Option<String>>, ApiError>
where
    F: FnMut(&WorkspaceConfig) -> Result<WorkspaceDatabaseHandle, ApiError>,
{
    let (resolved, _stats) =
        resolve_page_transcript_workspace_ids_with_stats(config, page_jobs, open_workspace)?;
    Ok(resolved)
}

pub(crate) fn resolve_page_transcript_workspace_ids_with_stats<F>(
    config: &GlobalConfig,
    page_jobs: &[PendingMemoryDreamJob],
    mut open_workspace: F,
) -> Result<(HashMap<String, Option<String>>, LegacyTranscriptLookupStats), ApiError>
where
    F: FnMut(&WorkspaceConfig) -> Result<WorkspaceDatabaseHandle, ApiError>,
{
    let mut resolved: HashMap<String, Option<String>> = HashMap::with_capacity(page_jobs.len());
    let mut legacy_chat_to_job_ids: HashMap<String, Vec<String>> = HashMap::new();

    for pending in page_jobs {
        if let Some(workspace_id) = memory_dream_transcript_workspace_id_from_input(&pending.job) {
            resolved.insert(pending.job.id.clone(), Some(workspace_id));
            continue;
        }
        if pending.job.scope == "workspace" {
            let workspace_id = pending
                .source_workspace_id
                .clone()
                .or_else(|| pending.job.workspace_id.clone());
            resolved.insert(pending.job.id.clone(), workspace_id);
            continue;
        }
        match pending.job.transcript_chat_id.as_deref() {
            Some(chat_id) if !chat_id.trim().is_empty() => {
                legacy_chat_to_job_ids
                    .entry(chat_id.to_string())
                    .or_default()
                    .push(pending.job.id.clone());
            }
            _ => {
                resolved.insert(pending.job.id.clone(), None);
            }
        }
    }

    let mut stats = LegacyTranscriptLookupStats::default();
    if legacy_chat_to_job_ids.is_empty() {
        return Ok((resolved, stats));
    }

    let candidates: Vec<String> = legacy_chat_to_job_ids.keys().cloned().collect();
    let (chat_to_workspace, lookup_stats) = resolve_legacy_transcript_chat_ids_with_stats(
        config.local_workspaces(),
        &candidates,
        &mut open_workspace,
    )?;
    stats = lookup_stats;

    for (chat_id, job_ids) in legacy_chat_to_job_ids {
        let workspace_id = chat_to_workspace.get(&chat_id).cloned();
        for job_id in job_ids {
            resolved.insert(job_id, workspace_id.clone());
        }
    }

    Ok((resolved, stats))
}

struct LocatedMemoryDreamJob {
    database: foco_store::OpenedMemoryDatabase,
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
        let transcript_workspace_id = memory_dream_transcript_workspace_id_for_job(config, &job)?;
        return Ok(LocatedMemoryDreamJob {
            database: global_database,
            job,
            transcript_workspace_id,
        });
    }

    for workspace in config.local_workspaces() {
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

fn memory_dream_transcript_workspace_id_for_job(
    config: &GlobalConfig,
    job: &MemoryDreamJobRecord,
) -> Result<Option<String>, ApiError> {
    if let Some(workspace_id) = memory_dream_transcript_workspace_id_from_input(job) {
        return Ok(Some(workspace_id));
    }
    if job.scope == "workspace" {
        if let Some(workspace_id) = job.workspace_id.clone() {
            return Ok(Some(workspace_id));
        }
    }
    memory_dream_transcript_workspace_id_by_chat_lookup(config, job.transcript_chat_id.as_deref())
}

fn memory_dream_transcript_workspace_id_from_input(job: &MemoryDreamJobRecord) -> Option<String> {
    let value = serde_json::from_str::<Value>(&job.input_summary_json).ok()?;
    value
        .get("transcriptWorkspaceId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Single-job legacy fallback used by detail/manual-run paths.
/// List endpoints must use [`resolve_page_transcript_workspace_ids_with_stats`] instead.
fn memory_dream_transcript_workspace_id_by_chat_lookup(
    config: &GlobalConfig,
    transcript_chat_id: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let Some(transcript_chat_id) = transcript_chat_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let chat_id = transcript_chat_id.to_string();
    let (chat_to_workspace, _stats) = resolve_legacy_transcript_chat_ids_with_stats(
        config.workspaces.iter(),
        std::slice::from_ref(&chat_id),
        |workspace| {
            WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)
        },
    )?;
    Ok(chat_to_workspace.get(&chat_id).cloned())
}

/// Bounded legacy resolution: each workspace is opened at most once; already-found
/// chat ids are dropped from subsequent IN queries. Never calls `dream_transcript_chats()`.
pub(crate) fn resolve_legacy_transcript_chat_ids_with_stats<'a, I, F>(
    workspaces: I,
    chat_ids: &[String],
    mut open_workspace: F,
) -> Result<(HashMap<String, String>, LegacyTranscriptLookupStats), ApiError>
where
    I: IntoIterator<Item = &'a WorkspaceConfig>,
    F: FnMut(&WorkspaceConfig) -> Result<WorkspaceDatabaseHandle, ApiError>,
{
    let mut stats = LegacyTranscriptLookupStats::default();
    if chat_ids.is_empty() {
        return Ok((HashMap::new(), stats));
    }

    let mut unresolved: HashSet<String> = chat_ids
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    if unresolved.is_empty() {
        return Ok((HashMap::new(), stats));
    }

    let mut chat_to_workspace: HashMap<String, String> = HashMap::new();
    for workspace in workspaces {
        if unresolved.is_empty() {
            break;
        }
        let database = open_workspace(workspace)?;
        stats.workspace_opens += 1;
        let candidates: Vec<String> = unresolved.iter().cloned().collect();
        let existing = database
            .existing_chat_ids(&candidates)
            .map_err(ApiError::from_workspace_error)?;
        stats.batch_existence_queries += 1;
        for chat_id in existing {
            unresolved.remove(&chat_id);
            chat_to_workspace.insert(chat_id, workspace.id.clone());
        }
    }

    Ok((chat_to_workspace, stats))
}

fn dream_limit(value: Option<u32>, default: u32, max: u32) -> u32 {
    value.unwrap_or(default).clamp(1, max)
}
