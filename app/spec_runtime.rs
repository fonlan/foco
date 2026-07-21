use std::{fs, future::Future, path::PathBuf, time::Duration};

use chrono::{DateTime, Utc};

use foco_providers::{
    NeutralChatRequest, NeutralChatRole, NeutralToolDefinition, ProviderConnectionConfig,
};
use foco_store::{
    config::{GlobalConfig, ModelSettings, SpecSettings},
    memory::MemoryDatabase,
    workspace::{
        CodeChangeStats, CodeGraphFileSummaryRecord, CodeGraphSymbolRecord,
        LLM_REQUEST_KIND_WORKSPACE_SPEC_COMPACTION, LLM_REQUEST_KIND_WORKSPACE_SPEC_GENERATION,
        LLM_REQUEST_KIND_WORKSPACE_SPEC_UPDATE, LLM_REQUEST_KIND_WORKSPACE_SPEC_UPDATE_COMPACTION,
        NewWorkspaceSpecJob, WORKSPACE_SPEC_MAX_MARKDOWN_BYTES,
        WORKSPACE_SPEC_STALE_REVISION_SKIP_REASON, WorkspaceDatabase, WorkspaceSpecJobRecord,
        WorkspaceSpecJobStatus, WorkspaceSpecRecord, WorkspaceSpecTriggerType,
        WorkspaceSpecWriteDecision, workspace_database_path,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    ApiError, AppState, PlanPhaseDerivedEffectsContext, PreparedChatContext,
    api_audit_save_details, audited_provider_tool_request, config_snapshot, markdown_code_block,
    neutral_text_message, provider_connection_config, unique_id, workspace_by_id,
};
use foco_tools::{SpecPatchError, SpecTextEdit, apply_spec_text_edits};

const WORKSPACE_SPEC_TOOL_NAME: &str = "submit_workspace_spec";
const WORKSPACE_SPEC_UPDATE_TOOL_NAME: &str = "submit_workspace_spec_update";
const WORKSPACE_SPEC_UPDATE_COMPACTION_TOOL_NAME: &str = "submit_workspace_spec_update_compaction";
// Stale when the lease (or started/created fallback) has not been renewed for this long.
const WORKSPACE_SPEC_STALE_RUNNING_AFTER_MS: i64 = 30 * 60 * 1000;
// Renew significantly more often than the stale window so slow multi-turn jobs stay live.
const WORKSPACE_SPEC_LEASE_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
const WORKSPACE_SPEC_MAX_OUTPUT_TOKENS: u32 = 4_000;
const WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES: usize = 56 * 1024;
// ponytail: multi-pass LLM compaction; raise attempts only if models keep missing the budget.
const WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS: u32 = 3;
const WORKSPACE_SPEC_COMPACTION_AGGRESSIVE_TARGET_BYTES: usize = 48 * 1024;
const WORKSPACE_SPEC_COMPACTION_EMERGENCY_TARGET_BYTES: usize = 40 * 1024;
// Compaction must rewrite a near-limit document; generation's 4k cap is too small for full rewrites.
const WORKSPACE_SPEC_COMPACTION_MAX_OUTPUT_TOKENS: u32 = 16_384;
const WORKSPACE_SPEC_FILE_SUMMARY_LIMIT: i64 = 24;
const WORKSPACE_SPEC_SYMBOL_LIMIT: i64 = 48;
const WORKSPACE_SPEC_MEMORY_PROFILE_LIMIT: u32 = 4;
const WORKSPACE_SPEC_ROOT_FILE_LIMIT: usize = 6;
const WORKSPACE_SPEC_SOURCE_FILE_MAX_CHARS: usize = 6_000;
const WORKSPACE_SPEC_MEMORY_PROFILE_MAX_CHARS: usize = 2_000;
const WORKSPACE_SPEC_CHAT_EXCERPT_MAX_CHARS: usize = 2_000;

// ponytail: root-file heuristic; replace with graph centrality only if generated specs need better recall.
const ROOT_SOURCE_FILE_CANDIDATES: &[&str] = &[
    "README.md",
    "README",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "deno.json",
    "vite.config.ts",
];

#[derive(Clone, Debug)]
pub(crate) struct PreparedWorkspaceSpecJob {
    pub(crate) workspace_id: String,
    pub(crate) workspace_path: PathBuf,
    pub(crate) job_id: String,
    pub(crate) chat_id: Option<String>,
    pub(crate) base_revision: u64,
    pub(crate) provider_id: String,
    pub(crate) provider_config: ProviderConnectionConfig,
    pub(crate) request: NeutralChatRequest,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecGenerationInput {
    pub(crate) workspace_id: String,
    pub(crate) base_revision: u64,
    pub(crate) code_graph: WorkspaceSpecCodeGraphInput,
    pub(crate) memory_profiles: Vec<WorkspaceSpecMemoryProfileInput>,
    pub(crate) source_files: Vec<WorkspaceSpecSourceFileInput>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecUpdateInput {
    pub(crate) workspace_id: String,
    pub(crate) chat_id: String,
    pub(crate) current_spec_revision: u64,
    pub(crate) user_message_id: String,
    pub(crate) assistant_message_id: String,
    pub(crate) run_id: String,
    pub(crate) code_change_stats: Option<CodeChangeStats>,
    pub(crate) chat_excerpt: WorkspaceSpecChatExcerptInput,
    pub(crate) current_spec_markdown: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecChatExcerptInput {
    pub(crate) user: String,
    pub(crate) user_truncated: bool,
    pub(crate) assistant: String,
    pub(crate) assistant_truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecCodeGraphInput {
    pub(crate) indexed_files: i64,
    pub(crate) symbol_count: i64,
    pub(crate) reference_count: i64,
    pub(crate) edge_count: i64,
    pub(crate) languages: Vec<String>,
    pub(crate) files: Vec<WorkspaceSpecFileSummaryInput>,
    pub(crate) symbols: Vec<WorkspaceSpecSymbolInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecFileSummaryInput {
    pub(crate) path: String,
    pub(crate) language: Option<String>,
    pub(crate) symbol_count: i64,
    pub(crate) import_count: i64,
    pub(crate) import_modules: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecSymbolInput {
    pub(crate) path: String,
    pub(crate) language: Option<String>,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) signature: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecMemoryProfileInput {
    pub(crate) id: String,
    pub(crate) scope: String,
    pub(crate) profile_text: String,
    pub(crate) truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecSourceFileInput {
    pub(crate) path: String,
    pub(crate) size_bytes: u64,
    pub(crate) content: String,
    pub(crate) truncated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceSpecToolOutput {
    content_markdown: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceSpecUpdateToolOutput {
    update_needed: bool,
    edits: Option<Vec<SpecTextEdit>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceSpecUpdateCompactionToolOutput {
    edits: Vec<SpecTextEdit>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceSpecUpdateOutput {
    NoUpdateNeeded,
    Patch {
        edits: Vec<SpecTextEdit>,
        content_markdown: String,
    },
}

#[derive(Debug)]
struct WorkspaceSpecModelSelection {
    model_id: String,
    provider_id: String,
    provider_config: ProviderConnectionConfig,
    max_output_tokens: u32,
}

/// Remote sidecar only needs provider/model ids for brokered LLM calls; secrets stay local.
#[derive(Clone, Debug)]
pub(crate) struct PreparedRemoteWorkspaceSpecJob {
    pub(crate) job_id: String,
    pub(crate) chat_id: Option<String>,
    pub(crate) base_revision: u64,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) request: NeutralChatRequest,
    pub(crate) workspace_path: PathBuf,
}

pub(crate) async fn run_workspace_spec_job(
    state: AppState,
    workspace_id: String,
    _job_id: String,
) -> Result<(), ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_path = workspace_by_id(&config, &workspace_id)?.path.clone();
    wake_workspace_spec_runner(config, workspace_id, workspace_path).await
}

pub(crate) fn wake_workspace_spec_runners_for_startup(state: &AppState) -> Result<(), ApiError> {
    let config = config_snapshot(state)?;
    for workspace in config.local_workspaces() {
        if !workspace_database_path(&workspace.path).exists() {
            continue;
        }
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        if let Some(job) = database
            .running_workspace_spec_job()
            .map_err(ApiError::from_workspace_error)?
        {
            database
                .mark_workspace_spec_job_failed(
                    &job.id,
                    "workspace spec job was interrupted by process restart",
                )
                .map_err(ApiError::from_workspace_error)?;
            log_workspace_spec_job_status_from_database(&database, &workspace.id, &job.id);
        }
        drop(database);
        spawn_workspace_spec_job(
            config.clone(),
            workspace.id.clone(),
            workspace.path.clone(),
            "startup-recovery".to_string(),
        );
    }
    Ok(())
}

async fn wake_workspace_spec_runner(
    config: GlobalConfig,
    workspace_id: String,
    workspace_path: PathBuf,
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(&workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    recover_stale_running_workspace_spec_job(&mut database, &workspace_id)?;
    drop(database);
    run_workspace_spec_jobs(config, workspace_id, workspace_path).await
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceSpecUpdateSpecState {
    pub(crate) exists: bool,
    pub(crate) enabled: bool,
    pub(crate) content_empty: bool,
}

impl WorkspaceSpecUpdateSpecState {
    fn from_record(spec: Option<&WorkspaceSpecRecord>) -> Self {
        match spec {
            Some(spec) => Self {
                exists: true,
                enabled: spec.enabled,
                content_empty: spec.content_markdown.trim().is_empty(),
            },
            None => Self {
                exists: false,
                enabled: false,
                content_empty: true,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceSpecUpdateQueueDecision {
    NeedsSpecState,
    Queue,
    Skip { reason: &'static str },
}

pub(crate) fn workspace_spec_update_queue_decision(
    final_state: &str,
    agent_primary_chat_output: bool,
    session_mode: Option<&str>,
    spec_auto_enabled: bool,
    spec_state: Option<WorkspaceSpecUpdateSpecState>,
) -> WorkspaceSpecUpdateQueueDecision {
    // ponytail: only models queue gating; tracing field coverage stays in thin caller logs.
    if final_state != "succeeded" {
        return WorkspaceSpecUpdateQueueDecision::Skip {
            reason: "final_state_not_succeeded",
        };
    }
    if !agent_primary_chat_output {
        return WorkspaceSpecUpdateQueueDecision::Skip {
            reason: "not_agent_primary_chat_output",
        };
    }
    if session_mode == Some("plan") {
        return WorkspaceSpecUpdateQueueDecision::Skip {
            reason: "plan_mode_session_not_plan_phase_implementation",
        };
    }
    if !spec_auto_enabled {
        return WorkspaceSpecUpdateQueueDecision::Skip {
            reason: "spec_auto_disabled",
        };
    }
    let Some(spec_state) = spec_state else {
        return WorkspaceSpecUpdateQueueDecision::NeedsSpecState;
    };
    if !spec_state.exists {
        return WorkspaceSpecUpdateQueueDecision::Skip {
            reason: "workspace_spec_missing",
        };
    }
    if !spec_state.enabled {
        return WorkspaceSpecUpdateQueueDecision::Skip {
            reason: "workspace_spec_disabled",
        };
    }
    if spec_state.content_empty {
        return WorkspaceSpecUpdateQueueDecision::Skip {
            reason: "workspace_spec_content_empty",
        };
    }
    WorkspaceSpecUpdateQueueDecision::Queue
}

fn log_workspace_spec_update_queue_skip(
    context: &PreparedChatContext,
    final_state: &str,
    reason: &str,
    spec_state: Option<WorkspaceSpecUpdateSpecState>,
) {
    let spec_exists = spec_state.map(|state| state.exists);
    let spec_enabled = spec_state.map(|state| state.enabled);
    let spec_content_empty = spec_state.map(|state| state.content_empty);
    tracing::debug!(
        workspace_id = %context.workspace_id,
        chat_id = %context.chat_id,
        run_id = %context.llm_request_id,
        final_state = %final_state,
        session_mode = ?context.session_mode.as_deref(),
        agent_primary_chat_output = context.agent_primary_chat_output,
        spec_auto_enabled = context.global_config.spec.auto_enabled,
        spec_exists = ?spec_exists,
        spec_enabled = ?spec_enabled,
        spec_content_empty = ?spec_content_empty,
        skip_reason = reason,
        "workspace spec update job skipped"
    );
}

pub(crate) fn workspace_spec_running_job_is_stale(
    job: &WorkspaceSpecJobRecord,
    now: DateTime<Utc>,
) -> bool {
    let lease_at = job.lease_or_started_or_created_at();
    let Ok(lease_at) = DateTime::parse_from_rfc3339(lease_at) else {
        return false;
    };
    now.signed_duration_since(lease_at.with_timezone(&Utc))
        .num_milliseconds()
        > WORKSPACE_SPEC_STALE_RUNNING_AFTER_MS
}

fn recover_stale_running_workspace_spec_job(
    database: &mut WorkspaceDatabase,
    workspace_id: &str,
) -> Result<bool, ApiError> {
    let Some(job) = database
        .running_workspace_spec_job()
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(false);
    };
    let now = Utc::now();
    let lease_at = job.lease_or_started_or_created_at().to_string();
    let elapsed_ms = DateTime::parse_from_rfc3339(&lease_at)
        .ok()
        .map(|lease_at| {
            now.signed_duration_since(lease_at.with_timezone(&Utc))
                .num_milliseconds()
        });
    // Fast path: skip opening a fail transaction when the snapshot is clearly live.
    if !workspace_spec_running_job_is_stale(&job, now) {
        return Ok(false);
    }

    let error_message = format!(
        "workspace spec job lease was not renewed for {} ms and was recovered as failed",
        WORKSPACE_SPEC_STALE_RUNNING_AFTER_MS
    );
    // Atomic re-check under IMMEDIATE: heartbeat renewal between the snapshot
    // read and this write must keep the job running.
    let failed = database
        .fail_stale_running_workspace_spec_job(
            &job.id,
            now,
            WORKSPACE_SPEC_STALE_RUNNING_AFTER_MS,
            &error_message,
        )
        .map_err(ApiError::from_workspace_error)?;
    if !failed {
        return Ok(false);
    }
    tracing::warn!(
        workspace_id = %workspace_id,
        job_id = %job.id,
        trigger_type = %job.trigger_type,
        lease_renewed_at = ?job.lease_renewed_at,
        started_at = ?job.started_at,
        created_at = %job.created_at,
        last_lease_at = %lease_at,
        elapsed_ms = ?elapsed_ms,
        stale_threshold_ms = WORKSPACE_SPEC_STALE_RUNNING_AFTER_MS,
        "stale running workspace spec job marked failed"
    );
    log_workspace_spec_job_status_from_database(database, workspace_id, &job.id);
    Ok(true)
}

#[cfg(test)]
pub(crate) fn recover_workspace_spec_queue_for_test(
    workspace_path: &std::path::Path,
    workspace_id: &str,
    now: DateTime<Utc>,
) -> Result<Vec<String>, ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    if let Some(job) = database
        .running_workspace_spec_job()
        .map_err(ApiError::from_workspace_error)?
    {
        let failed = database
            .fail_stale_running_workspace_spec_job(
                &job.id,
                now,
                WORKSPACE_SPEC_STALE_RUNNING_AFTER_MS,
                "stale running test recovery",
            )
            .map_err(ApiError::from_workspace_error)?;
        if failed {
            tracing::warn!(
                workspace_id = %workspace_id,
                job_id = %job.id,
                "stale running workspace spec job recovered in test drain"
            );
        }
    }
    let mut claimed = Vec::new();
    while let Some(job) = database
        .claim_next_workspace_spec_job()
        .map_err(ApiError::from_workspace_error)?
    {
        claimed.push(job.id.clone());
        database
            .mark_workspace_spec_job_completed(&job.id, None)
            .map_err(ApiError::from_workspace_error)?;
    }
    Ok(claimed)
}

pub(crate) fn queue_workspace_spec_update_job(
    context: &PreparedChatContext,
    final_state: &str,
) -> Result<(), ApiError> {
    queue_workspace_spec_update_job_with_id(context, final_state, None)
}

pub(crate) fn queue_workspace_spec_update_job_with_id(
    context: &PreparedChatContext,
    final_state: &str,
    job_id: Option<&str>,
) -> Result<(), ApiError> {
    match workspace_spec_update_queue_decision(
        final_state,
        context.agent_primary_chat_output,
        context.session_mode.as_deref(),
        context.global_config.spec.auto_enabled,
        None,
    ) {
        WorkspaceSpecUpdateQueueDecision::NeedsSpecState => {}
        WorkspaceSpecUpdateQueueDecision::Queue => {
            unreachable!("workspace spec state is required before queueing an update job")
        }
        WorkspaceSpecUpdateQueueDecision::Skip { reason } => {
            log_workspace_spec_update_queue_skip(context, final_state, reason, None);
            return Ok(());
        }
    }

    let mut database = WorkspaceDatabase::open_or_create_critical(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let spec = database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?;
    let spec_state = WorkspaceSpecUpdateSpecState::from_record(spec.as_ref());
    match workspace_spec_update_queue_decision(
        final_state,
        context.agent_primary_chat_output,
        context.session_mode.as_deref(),
        context.global_config.spec.auto_enabled,
        Some(spec_state),
    ) {
        WorkspaceSpecUpdateQueueDecision::NeedsSpecState => {
            unreachable!("workspace spec state remained unknown after loading it")
        }
        WorkspaceSpecUpdateQueueDecision::Queue => {}
        WorkspaceSpecUpdateQueueDecision::Skip { reason } => {
            log_workspace_spec_update_queue_skip(context, final_state, reason, Some(spec_state));
            return Ok(());
        }
    }
    let spec = spec.expect("spec state checked before queueing workspace spec update");

    let stale_running_job_recovered =
        recover_stale_running_workspace_spec_job(&mut database, &context.workspace_id)?;
    let running_job_exists = database
        .running_workspace_spec_job()
        .map_err(ApiError::from_workspace_error)?
        .is_some();
    tracing::debug!(
        workspace_id = %context.workspace_id,
        chat_id = %context.chat_id,
        run_id = %context.llm_request_id,
        final_state = %final_state,
        session_mode = ?context.session_mode.as_deref(),
        agent_primary_chat_output = context.agent_primary_chat_output,
        spec_auto_enabled = context.global_config.spec.auto_enabled,
        spec_exists = spec_state.exists,
        spec_enabled = spec_state.enabled,
        spec_content_empty = spec_state.content_empty,
        stale_running_job_recovered,
        running_job_exists,
        "workspace spec update job queueing"
    );

    let input =
        workspace_spec_update_input(context, &database, spec.revision, &spec.content_markdown)?;
    let input_summary_json = serde_json::to_string(&input).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize workspace spec update input: {source}"
        ))
    })?;
    let job_id = job_id
        .map(str::to_string)
        .unwrap_or_else(|| unique_id("workspace-spec-job"));
    let job = database
        .insert_workspace_spec_job_if_absent(NewWorkspaceSpecJob {
            id: &job_id,
            trigger_type: WorkspaceSpecTriggerType::ChatCompleted.as_str(),
            chat_id: Some(&context.chat_id),
            run_id: Some(&context.llm_request_id),
            model_id: context
                .global_config
                .spec
                .generation_model_id
                .as_deref()
                .or(Some(context.model_id.as_str())),
            base_revision: Some(spec.revision),
            input_summary_json: Some(&input_summary_json),
        })
        .map_err(ApiError::from_workspace_error)?;
    log_workspace_spec_job_status(&context.workspace_id, &job);
    let job_id = job.id;
    drop(database);

    spawn_workspace_spec_job(
        context.global_config.clone(),
        context.workspace_id.clone(),
        context.workspace_path.clone(),
        job_id,
    );

    Ok(())
}

pub(crate) fn queue_integrated_plan_workspace_spec_update(
    context: &PlanPhaseDerivedEffectsContext,
    workspace_path: &std::path::Path,
    config: &GlobalConfig,
    job_id: &str,
    spawn_runner: bool,
) -> Result<(), ApiError> {
    if !config.spec.auto_enabled {
        return Ok(());
    }
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let Some(spec) = database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(());
    };
    if !spec.enabled || spec.content_markdown.trim().is_empty() {
        return Ok(());
    }
    let input = WorkspaceSpecUpdateInput {
        workspace_id: context.workspace_id.clone(),
        chat_id: context.chat_id.clone(),
        current_spec_revision: spec.revision,
        user_message_id: context.user_message_id.clone(),
        assistant_message_id: context.assistant_message_id.clone(),
        run_id: context.run_id.clone(),
        code_change_stats: (context.code_change_stats.additions > 0
            || context.code_change_stats.deletions > 0)
            .then_some(context.code_change_stats.clone()),
        chat_excerpt: WorkspaceSpecChatExcerptInput {
            user: compact_text(
                &message_content(&database, &context.user_message_id)?,
                WORKSPACE_SPEC_CHAT_EXCERPT_MAX_CHARS,
            )
            .0,
            user_truncated: false,
            assistant: compact_text(
                &message_content(&database, &context.assistant_message_id)?,
                WORKSPACE_SPEC_CHAT_EXCERPT_MAX_CHARS,
            )
            .0,
            assistant_truncated: false,
        },
        current_spec_markdown: spec.content_markdown,
    };
    let input_summary_json = serde_json::to_string(&input).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize workspace spec update input: {source}"
        ))
    })?;
    database
        .insert_workspace_spec_job_if_absent(NewWorkspaceSpecJob {
            id: job_id,
            trigger_type: WorkspaceSpecTriggerType::ChatCompleted.as_str(),
            chat_id: Some(&context.chat_id),
            run_id: Some(&context.run_id),
            model_id: config
                .spec
                .generation_model_id
                .as_deref()
                .or(Some(context.model_id.as_str())),
            base_revision: Some(spec.revision),
            input_summary_json: Some(&input_summary_json),
        })
        .map_err(ApiError::from_workspace_error)?;
    drop(database);
    if spawn_runner {
        spawn_workspace_spec_job(
            config.clone(),
            context.workspace_id.clone(),
            workspace_path.to_path_buf(),
            job_id.to_string(),
        );
    }
    Ok(())
}

fn spawn_workspace_spec_job(
    config: GlobalConfig,
    workspace_id: String,
    workspace_path: PathBuf,
    wake_job_id: String,
) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::warn!(
            job_id = %wake_job_id,
            workspace_id = %workspace_id,
            "workspace spec update job queued without an active async runtime"
        );
        return;
    };
    handle.spawn(async move {
        let runtime_workspace_id = workspace_id.clone();
        if let Err(error) = wake_workspace_spec_runner(config, workspace_id, workspace_path).await {
            tracing::warn!(
                workspace_id = %runtime_workspace_id,
                wake_job_id = %wake_job_id,
                error = %error.message,
                "workspace spec background runner failed"
            );
        }
    });
}

async fn run_workspace_spec_jobs(
    config: GlobalConfig,
    workspace_id: String,
    workspace_path: PathBuf,
) -> Result<(), ApiError> {
    loop {
        let Some(job) = claim_next_workspace_spec_job(&workspace_path)? else {
            return Ok(());
        };
        let job_id = job.id.clone();
        log_workspace_spec_job_status(&workspace_id, &job);
        if let Err(error) = run_workspace_spec_job_with_lease_heartbeat(
            &workspace_path,
            &workspace_id,
            &job_id,
            run_workspace_spec_job_inner(&config, &workspace_id, &workspace_path, job),
        )
        .await
        {
            mark_workspace_spec_job_failed_at_path(
                &workspace_path,
                &workspace_id,
                &job_id,
                &error.message,
            );
        }
    }
}

/// Runs a claimed Spec job while periodically renewing its DB lease.
///
/// The heartbeat uses short open/touch transactions only and never holds a
/// connection across the job future (LLM/network). Local and remote runners
/// share this helper so stale recovery means "lost liveness", not "ran long".
pub(crate) async fn run_workspace_spec_job_with_lease_heartbeat<F>(
    workspace_path: &std::path::Path,
    workspace_id: &str,
    job_id: &str,
    job_future: F,
) -> Result<(), ApiError>
where
    F: Future<Output = Result<(), ApiError>>,
{
    run_workspace_spec_job_with_lease_heartbeat_interval(
        workspace_path,
        workspace_id,
        job_id,
        WORKSPACE_SPEC_LEASE_HEARTBEAT_INTERVAL,
        job_future,
    )
    .await
}

pub(crate) async fn run_workspace_spec_job_with_lease_heartbeat_interval<F>(
    workspace_path: &std::path::Path,
    workspace_id: &str,
    job_id: &str,
    interval: Duration,
    job_future: F,
) -> Result<(), ApiError>
where
    F: Future<Output = Result<(), ApiError>>,
{
    let mut job_future = std::pin::pin!(job_future);
    let mut heartbeat = tokio::time::interval(interval);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // Skip the immediate first tick; claim/mark-running already set the lease.
    heartbeat.tick().await;

    loop {
        tokio::select! {
            biased;
            result = &mut job_future => {
                return result;
            }
            _ = heartbeat.tick() => {
                match renew_workspace_spec_job_lease(workspace_path, job_id) {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::debug!(
                            workspace_id = %workspace_id,
                            job_id = %job_id,
                            "workspace spec job lease renew skipped; job no longer running"
                        );
                        // Job already left running (completed/failed/skipped).
                        // Keep waiting for the job future so we do not drop work.
                    }
                    Err(error) => {
                        tracing::warn!(
                            workspace_id = %workspace_id,
                            job_id = %job_id,
                            error = %error,
                            "workspace spec job lease renew failed; stale recovery may reclaim"
                        );
                    }
                }
            }
        }
    }
}

fn renew_workspace_spec_job_lease(
    workspace_path: &std::path::Path,
    job_id: &str,
) -> Result<bool, foco_store::workspace::WorkspaceDatabaseError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)?;
    database.touch_workspace_spec_job_lease(job_id)
}

async fn run_workspace_spec_job_inner(
    config: &GlobalConfig,
    workspace_id: &str,
    workspace_path: &std::path::Path,
    job: WorkspaceSpecJobRecord,
) -> Result<(), ApiError> {
    if job.trigger_type == WorkspaceSpecTriggerType::ChatCompleted.as_str() {
        return run_workspace_spec_update_job_inner(config, workspace_id, workspace_path, job)
            .await;
    }

    let Some(prepared) =
        prepare_workspace_spec_generation_job(config, workspace_id, workspace_path, &job.id)?
    else {
        return Ok(());
    };

    let tool_result = audited_provider_tool_request(
        &prepared.workspace_path,
        &prepared.workspace_id,
        prepared.chat_id.as_deref(),
        &prepared.provider_id,
        &prepared.provider_config,
        prepared.request.clone(),
        LLM_REQUEST_KIND_WORKSPACE_SPEC_GENERATION,
        WORKSPACE_SPEC_TOOL_NAME,
        "submit workspace spec tool",
        config.spec.llm_timeout_ms,
        config.app.llm_request_retry_count,
        api_audit_save_details(config),
    )
    .await?;
    let content_markdown = match parse_workspace_spec_output(tool_result.arguments.clone()) {
        Ok(content) => content,
        Err(error) => {
            if let Some(classification) = crate::structured_llm_outcome::classification_for_caller_failure(
                &error.message,
                i64::from(tool_result.attempt_index),
            ) {
                let _ = crate::structured_llm_outcome::persist_structured_classification(
                    &prepared.workspace_path,
                    &tool_result.request_id,
                    classification,
                );
            }
            return Err(error);
        }
    };
    let content_markdown = ensure_workspace_spec_markdown_fits_limit(
        config,
        &prepared.workspace_path,
        &prepared.workspace_id,
        &prepared.provider_id,
        &prepared.provider_config,
        &prepared.request.model_id,
        prepared.request.max_output_tokens,
        &content_markdown,
        prepared.chat_id.as_deref(),
    )
    .await?;
    let result = apply_workspace_spec_job_output(
        &prepared.workspace_path,
        &prepared.job_id,
        prepared.base_revision,
        &content_markdown,
    );
    if result.is_ok() {
        log_workspace_spec_job_status_at_path(workspace_path, workspace_id, &prepared.job_id);
    }
    result
}

async fn run_workspace_spec_update_job_inner(
    config: &GlobalConfig,
    workspace_id: &str,
    workspace_path: &std::path::Path,
    job: WorkspaceSpecJobRecord,
) -> Result<(), ApiError> {
    let Some((base_revision, input_summary)) =
        prepare_workspace_spec_update_job_input(workspace_id, workspace_path, &job)?
    else {
        return Ok(());
    };

    let model = resolve_workspace_spec_model(config, job.model_id.as_deref())?;
    let request = workspace_spec_update_provider_request(
        &model.model_id,
        &config.app.language,
        config.spec.update_system_prompt.as_deref(),
        model.max_output_tokens,
        &input_summary,
    )?;
    let tool_result = audited_provider_tool_request(
        workspace_path,
        workspace_id,
        job.chat_id.as_deref(),
        &model.provider_id,
        &model.provider_config,
        request,
        LLM_REQUEST_KIND_WORKSPACE_SPEC_UPDATE,
        WORKSPACE_SPEC_UPDATE_TOOL_NAME,
        "submit workspace spec update tool",
        config.spec.llm_timeout_ms,
        config.app.llm_request_retry_count,
        api_audit_save_details(config),
    )
    .await?;

    let update_output = match parse_workspace_spec_update_output(
        tool_result.arguments.clone(),
        &input_summary.current_spec_markdown,
    ) {
        Ok(output) => output,
        Err(error) => {
            if let Some(classification) = crate::structured_llm_outcome::classification_for_caller_failure(
                &error.message,
                i64::from(tool_result.attempt_index),
            ) {
                let _ = crate::structured_llm_outcome::persist_structured_classification(
                    workspace_path,
                    &tool_result.request_id,
                    classification,
                );
            }
            return Err(error);
        }
    };
    let update_output = ensure_workspace_spec_update_fits_limit(
        config,
        workspace_path,
        workspace_id,
        &model.provider_id,
        &model.provider_config,
        &model.model_id,
        model.max_output_tokens,
        update_output,
        job.chat_id.as_deref(),
    )
    .await?;
    let result = apply_workspace_spec_update_job_parsed_output(
        workspace_path,
        &job.id,
        base_revision,
        update_output,
    );
    if result.is_ok() {
        log_workspace_spec_job_status_at_path(workspace_path, workspace_id, &job.id);
    }
    result
}

#[cfg(test)]
pub(crate) fn prepare_workspace_spec_update_job(
    workspace_id: &str,
    workspace_path: &std::path::Path,
    job: WorkspaceSpecJobRecord,
) -> Result<Option<(u64, WorkspaceSpecUpdateInput)>, ApiError> {
    prepare_workspace_spec_update_job_input(workspace_id, workspace_path, &job)
}

/// Prepare a chat-completed Spec update job for remote sidecar execution.
/// Models/prompts come from the synced runtime config; LLM runs via broker.
pub(crate) fn prepare_remote_workspace_spec_update_job(
    workspace_id: &str,
    workspace_path: &std::path::Path,
    job: &WorkspaceSpecJobRecord,
    models: &[ModelSettings],
    generation_model_id: Option<&str>,
    update_system_prompt: Option<&str>,
    app_language: &str,
) -> Result<Option<PreparedRemoteWorkspaceSpecUpdateJob>, ApiError> {
    let Some((base_revision, input_summary)) =
        prepare_workspace_spec_update_job_input(workspace_id, workspace_path, job)?
    else {
        return Ok(None);
    };
    let model = resolve_workspace_spec_model_from_models(
        models,
        job.model_id.as_deref().or(generation_model_id),
    )?;
    let request = workspace_spec_update_provider_request(
        &model.model_id,
        app_language,
        update_system_prompt,
        model.max_output_tokens,
        &input_summary,
    )?;
    Ok(Some(PreparedRemoteWorkspaceSpecUpdateJob {
        job_id: job.id.clone(),
        chat_id: job.chat_id.clone(),
        base_revision,
        provider_id: model.provider_id,
        model_id: model.model_id,
        max_output_tokens: model.max_output_tokens,
        request,
        base_markdown: input_summary.current_spec_markdown,
        workspace_path: workspace_path.to_path_buf(),
    }))
}

/// Remote sidecar update job: provider/model ids for brokered LLM; secrets stay local.
#[derive(Clone, Debug)]
pub(crate) struct PreparedRemoteWorkspaceSpecUpdateJob {
    pub(crate) job_id: String,
    pub(crate) chat_id: Option<String>,
    pub(crate) base_revision: u64,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) max_output_tokens: u32,
    pub(crate) request: NeutralChatRequest,
    pub(crate) base_markdown: String,
    pub(crate) workspace_path: PathBuf,
}

fn prepare_workspace_spec_update_job_input(
    workspace_id: &str,
    workspace_path: &std::path::Path,
    job: &WorkspaceSpecJobRecord,
) -> Result<Option<(u64, WorkspaceSpecUpdateInput)>, ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let Some(spec) = database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?
        .filter(|spec| spec.enabled && !spec.content_markdown.trim().is_empty())
    else {
        database
            .mark_workspace_spec_job_skipped(&job.id, "workspace_spec_disabled")
            .map_err(ApiError::from_workspace_error)?;
        log_workspace_spec_job_status_from_database(&database, workspace_id, &job.id);
        return Ok(None);
    };
    let base_revision = spec.revision;
    let mut input_summary: WorkspaceSpecUpdateInput = serde_json::from_str(&job.input_summary_json)
        .map_err(|source| {
            ApiError::internal(format!(
                "invalid persisted workspace spec update input: {source}"
            ))
        })?;
    input_summary.workspace_id = workspace_id.to_string();
    input_summary.current_spec_revision = base_revision;
    input_summary.current_spec_markdown = spec.content_markdown;
    let input_summary_json = serde_json::to_string(&input_summary).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize workspace spec update input: {source}"
        ))
    })?;
    database
        .update_workspace_spec_job_prepared_input(&job.id, base_revision, &input_summary_json)
        .map_err(ApiError::from_workspace_error)?;
    if job.status == WorkspaceSpecJobStatus::Queued.as_str() {
        database
            .mark_workspace_spec_job_running(&job.id)
            .map_err(ApiError::from_workspace_error)?;
        log_workspace_spec_job_status_from_database(&database, workspace_id, &job.id);
    }

    Ok(Some((base_revision, input_summary)))
}

#[cfg(test)]
pub(crate) fn apply_workspace_spec_update_job_output(
    workspace_path: &std::path::Path,
    job_id: &str,
    base_revision: u64,
    value: Value,
) -> Result<(), ApiError> {
    let base_markdown = workspace_spec_update_base_markdown_for_job(workspace_path, job_id)?;
    apply_workspace_spec_update_job_parsed_output(
        workspace_path,
        job_id,
        base_revision,
        parse_workspace_spec_update_output(value, &base_markdown)?,
    )
}

/// Apply a parsed automatic Spec update (patch or no-op) with shared CAS/write semantics.
pub(crate) fn apply_workspace_spec_update_job_parsed_output(
    workspace_path: &std::path::Path,
    job_id: &str,
    base_revision: u64,
    output: WorkspaceSpecUpdateOutput,
) -> Result<(), ApiError> {
    match output {
        WorkspaceSpecUpdateOutput::NoUpdateNeeded => {
            let mut database = WorkspaceDatabase::open_or_create(workspace_path)
                .map_err(ApiError::from_workspace_error)?;
            database
                .mark_workspace_spec_job_skipped(job_id, "no_update_needed")
                .map_err(ApiError::from_workspace_error)?;
            Ok(())
        }
        WorkspaceSpecUpdateOutput::Patch {
            edits,
            content_markdown,
        } => apply_workspace_spec_update_job_patch_output(
            workspace_path,
            job_id,
            base_revision,
            &edits,
            &content_markdown,
        ),
    }
}

fn workspace_spec_update_base_markdown_for_job(
    workspace_path: &std::path::Path,
    job_id: &str,
) -> Result<String, ApiError> {
    let database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let job = database
        .workspace_spec_job(job_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!("workspace spec job was not found: {job_id}"))
        })?;
    if !job.input_summary_json.trim().is_empty() {
        let input: WorkspaceSpecUpdateInput = serde_json::from_str(&job.input_summary_json)
            .map_err(|source| {
                ApiError::bad_request(format!("malformed workspace spec update input: {source}"))
            })?;
        return Ok(input.current_spec_markdown);
    }
    Ok(database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?
        .map(|spec| spec.content_markdown)
        .unwrap_or_default())
}

fn apply_workspace_spec_update_job_patch_output(
    workspace_path: &std::path::Path,
    job_id: &str,
    base_revision: u64,
    edits: &[SpecTextEdit],
    content_markdown: &str,
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let Some(job) = database
        .workspace_spec_job(job_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(());
    };
    if job.status != WorkspaceSpecJobStatus::Running.as_str() {
        return Ok(());
    }
    let current = database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request("workspace spec row is missing"))?;
    match WorkspaceSpecWriteDecision::for_job_output(base_revision, current.revision) {
        WorkspaceSpecWriteDecision::WriteFullReplacement => {}
        WorkspaceSpecWriteDecision::SkipStaleRevision { reason } => {
            database
                .mark_workspace_spec_job_skipped(job_id, reason)
                .map_err(ApiError::from_workspace_error)?;
            return Ok(());
        }
    }

    let previous_markdown = current.content_markdown;
    let Some(updated) = database
        .update_workspace_spec_generated_content(base_revision, content_markdown)
        .map_err(ApiError::from_workspace_error)?
    else {
        database
            .mark_workspace_spec_job_skipped(job_id, "stale_revision")
            .map_err(ApiError::from_workspace_error)?;
        return Ok(());
    };
    let output_json = json!({
        "updateMode": "patch",
        "editCount": edits.len(),
        "revision": updated.revision,
        "contentBytes": content_markdown.len(),
    })
    .to_string();
    database
        .mark_workspace_spec_job_completed(job_id, Some(&output_json))
        .map_err(ApiError::from_workspace_error)?;

    let Some(job) = database
        .workspace_spec_job(job_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(());
    };
    if job.trigger_type == WorkspaceSpecTriggerType::ChatCompleted.as_str() {
        let assistant_message_id = workspace_spec_update_assistant_message_id(&job)?;
        let completed_at = job
            .completed_at
            .as_deref()
            .unwrap_or(updated.updated_at.as_str());
        let summary = crate::chat_spec_update_summary(
            job_id,
            base_revision,
            updated.revision,
            completed_at,
            &previous_markdown,
            content_markdown,
        );
        crate::append_assistant_spec_update_summary(
            workspace_path,
            &assistant_message_id,
            summary,
        )?;
    }

    Ok(())
}

#[cfg(test)]
pub(crate) fn prepare_workspace_spec_job(
    config: &GlobalConfig,
    workspace_id: &str,
    workspace: &foco_store::config::WorkspaceConfig,
    job_id: &str,
) -> Result<Option<PreparedWorkspaceSpecJob>, ApiError> {
    prepare_workspace_spec_generation_job(config, workspace_id, &workspace.path, job_id)
}

/// Prepare a manual generation job for remote sidecar execution.
/// Uses models/spec prompts from the synced runtime config; LLM runs via broker.
pub(crate) fn prepare_remote_workspace_spec_generation_job(
    workspace_id: &str,
    workspace_path: &std::path::Path,
    job_id: &str,
    models: &[ModelSettings],
    generation_model_id: Option<&str>,
    generation_system_prompt: Option<&str>,
    app_language: &str,
) -> Result<Option<PreparedRemoteWorkspaceSpecJob>, ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let Some(job) = database
        .workspace_spec_job(job_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Err(ApiError::bad_request(format!(
            "workspace spec job was not found: {job_id}"
        )));
    };

    if job.status != WorkspaceSpecJobStatus::Queued.as_str()
        && job.status != WorkspaceSpecJobStatus::Running.as_str()
    {
        return Ok(None);
    }
    let spec = database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?;
    let Some(spec) = spec.filter(|spec| spec.enabled) else {
        database
            .mark_workspace_spec_job_skipped(job_id, "workspace_spec_disabled")
            .map_err(ApiError::from_workspace_error)?;
        log_workspace_spec_job_status_from_database(&database, workspace_id, job_id);
        return Ok(None);
    };
    let base_revision = spec.revision;
    let input_summary =
        collect_workspace_spec_input_without_memory(workspace_id, workspace_path, base_revision)?;
    let input_summary_json = serde_json::to_string(&input_summary).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize workspace spec input: {source}"
        ))
    })?;
    database
        .update_workspace_spec_job_prepared_input(job_id, base_revision, &input_summary_json)
        .map_err(ApiError::from_workspace_error)?;
    if job.status == WorkspaceSpecJobStatus::Queued.as_str() {
        database
            .mark_workspace_spec_job_running(job_id)
            .map_err(ApiError::from_workspace_error)?;
        log_workspace_spec_job_status_from_database(&database, workspace_id, job_id);
    }

    let model = resolve_workspace_spec_model_from_models(
        models,
        job.model_id.as_deref().or(generation_model_id),
    )?;
    let request = workspace_spec_provider_request(
        &model.model_id,
        app_language,
        generation_system_prompt,
        model.max_output_tokens,
        &input_summary,
    )?;

    Ok(Some(PreparedRemoteWorkspaceSpecJob {
        job_id: job.id,
        chat_id: job.chat_id,
        base_revision,
        provider_id: model.provider_id,
        model_id: model.model_id,
        request,
        workspace_path: workspace_path.to_path_buf(),
    }))
}

fn prepare_workspace_spec_generation_job(
    config: &GlobalConfig,
    workspace_id: &str,
    workspace_path: &std::path::Path,
    job_id: &str,
) -> Result<Option<PreparedWorkspaceSpecJob>, ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let Some(job) = database
        .workspace_spec_job(job_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Err(ApiError::bad_request(format!(
            "workspace spec job was not found: {job_id}"
        )));
    };

    if job.status != WorkspaceSpecJobStatus::Queued.as_str()
        && job.status != WorkspaceSpecJobStatus::Running.as_str()
    {
        return Ok(None);
    }
    let spec = database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?;
    let Some(spec) = spec.filter(|spec| spec.enabled) else {
        database
            .mark_workspace_spec_job_skipped(job_id, "workspace_spec_disabled")
            .map_err(ApiError::from_workspace_error)?;
        log_workspace_spec_job_status_from_database(&database, workspace_id, job_id);
        return Ok(None);
    };
    let base_revision = spec.revision;
    let input_summary =
        collect_workspace_spec_input(config, workspace_id, workspace_path, base_revision)?;
    let input_summary_json = serde_json::to_string(&input_summary).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize workspace spec input: {source}"
        ))
    })?;
    database
        .update_workspace_spec_job_prepared_input(job_id, base_revision, &input_summary_json)
        .map_err(ApiError::from_workspace_error)?;
    if job.status == WorkspaceSpecJobStatus::Queued.as_str() {
        database
            .mark_workspace_spec_job_running(job_id)
            .map_err(ApiError::from_workspace_error)?;
        log_workspace_spec_job_status_from_database(&database, workspace_id, job_id);
    }

    let model = resolve_workspace_spec_model(config, job.model_id.as_deref())?;
    let request = workspace_spec_provider_request(
        &model.model_id,
        &config.app.language,
        config.spec.generation_system_prompt.as_deref(),
        model.max_output_tokens,
        &input_summary,
    )?;

    Ok(Some(PreparedWorkspaceSpecJob {
        workspace_id: workspace_id.to_string(),
        workspace_path: workspace_path.to_path_buf(),
        job_id: job.id,
        chat_id: job.chat_id,
        base_revision,
        provider_id: model.provider_id,
        provider_config: model.provider_config,
        request,
    }))
}

pub(crate) fn apply_workspace_spec_job_output(
    workspace_path: &std::path::Path,
    job_id: &str,
    base_revision: u64,
    content_markdown: &str,
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let Some(job) = database
        .workspace_spec_job(job_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(());
    };
    if job.status != WorkspaceSpecJobStatus::Running.as_str() {
        return Ok(());
    }
    let current = database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request("workspace spec row is missing"))?;
    match WorkspaceSpecWriteDecision::for_job_output(base_revision, current.revision) {
        WorkspaceSpecWriteDecision::WriteFullReplacement => {}
        WorkspaceSpecWriteDecision::SkipStaleRevision { reason } => {
            database
                .mark_workspace_spec_job_skipped(job_id, reason)
                .map_err(ApiError::from_workspace_error)?;
            return Ok(());
        }
    }

    let previous_markdown = current.content_markdown;
    let Some(updated) = database
        .update_workspace_spec_generated_content(base_revision, content_markdown)
        .map_err(ApiError::from_workspace_error)?
    else {
        database
            .mark_workspace_spec_job_skipped(job_id, "stale_revision")
            .map_err(ApiError::from_workspace_error)?;
        return Ok(());
    };
    let output_json = json!({
        "revision": updated.revision,
        "contentBytes": content_markdown.len(),
    })
    .to_string();
    database
        .mark_workspace_spec_job_completed(job_id, Some(&output_json))
        .map_err(ApiError::from_workspace_error)?;

    let Some(job) = database
        .workspace_spec_job(job_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(());
    };
    if job.trigger_type == WorkspaceSpecTriggerType::ChatCompleted.as_str() {
        let assistant_message_id = workspace_spec_update_assistant_message_id(&job)?;
        let completed_at = job
            .completed_at
            .as_deref()
            .unwrap_or(updated.updated_at.as_str());
        let summary = crate::chat_spec_update_summary(
            job_id,
            base_revision,
            updated.revision,
            completed_at,
            &previous_markdown,
            content_markdown,
        );
        crate::append_assistant_spec_update_summary(
            workspace_path,
            &assistant_message_id,
            summary,
        )?;
    }

    Ok(())
}

async fn ensure_workspace_spec_update_fits_limit(
    config: &GlobalConfig,
    workspace_path: &std::path::Path,
    workspace_id: &str,
    provider_id: &str,
    provider_config: &ProviderConnectionConfig,
    model_id: &str,
    max_output_tokens: u32,
    output: WorkspaceSpecUpdateOutput,
    chat_id: Option<&str>,
) -> Result<WorkspaceSpecUpdateOutput, ApiError> {
    match output {
        WorkspaceSpecUpdateOutput::NoUpdateNeeded => Ok(WorkspaceSpecUpdateOutput::NoUpdateNeeded),
        WorkspaceSpecUpdateOutput::Patch {
            edits,
            content_markdown,
        } => {
            let content_markdown = ensure_workspace_spec_update_markdown_fits_limit(
                config,
                workspace_path,
                workspace_id,
                provider_id,
                provider_config,
                model_id,
                Some(max_output_tokens),
                &content_markdown,
                chat_id,
            )
            .await?;
            Ok(WorkspaceSpecUpdateOutput::Patch {
                edits,
                content_markdown,
            })
        }
    }
}

async fn ensure_workspace_spec_update_markdown_fits_limit(
    config: &GlobalConfig,
    workspace_path: &std::path::Path,
    workspace_id: &str,
    provider_id: &str,
    provider_config: &ProviderConnectionConfig,
    model_id: &str,
    max_output_tokens: Option<u32>,
    content_markdown: &str,
    chat_id: Option<&str>,
) -> Result<String, ApiError> {
    compact_oversized_workspace_spec_update_markdown(
        model_id,
        max_output_tokens,
        content_markdown,
        |request| {
            let workspace_path = workspace_path.to_path_buf();
            let workspace_id = workspace_id.to_string();
            let chat_id = chat_id.map(str::to_string);
            let provider_id = provider_id.to_string();
            let provider_config = provider_config.clone();
            let timeout_ms = config.spec.llm_timeout_ms;
            let retry_count = config.app.llm_request_retry_count;
            let save_details = api_audit_save_details(config);
            async move {
                Ok(audited_provider_tool_request(
                    &workspace_path,
                    &workspace_id,
                    chat_id.as_deref(),
                    &provider_id,
                    &provider_config,
                    request,
                    LLM_REQUEST_KIND_WORKSPACE_SPEC_UPDATE_COMPACTION,
                    WORKSPACE_SPEC_UPDATE_COMPACTION_TOOL_NAME,
                    "submit workspace spec update compaction tool",
                    timeout_ms,
                    retry_count,
                    save_details,
                )
                .await?
                .arguments)
            }
        },
    )
    .await
}

async fn ensure_workspace_spec_markdown_fits_limit(
    config: &GlobalConfig,
    workspace_path: &std::path::Path,
    workspace_id: &str,
    provider_id: &str,
    provider_config: &ProviderConnectionConfig,
    model_id: &str,
    max_output_tokens: Option<u32>,
    content_markdown: &str,
    chat_id: Option<&str>,
) -> Result<String, ApiError> {
    compact_oversized_workspace_spec_markdown(
        model_id,
        max_output_tokens,
        content_markdown,
        |request| {
            let workspace_path = workspace_path.to_path_buf();
            let workspace_id = workspace_id.to_string();
            let chat_id = chat_id.map(str::to_string);
            let provider_id = provider_id.to_string();
            let provider_config = provider_config.clone();
            let timeout_ms = config.spec.llm_timeout_ms;
            let retry_count = config.app.llm_request_retry_count;
            let save_details = api_audit_save_details(config);
            async move {
                Ok(audited_provider_tool_request(
                    &workspace_path,
                    &workspace_id,
                    chat_id.as_deref(),
                    &provider_id,
                    &provider_config,
                    request,
                    LLM_REQUEST_KIND_WORKSPACE_SPEC_COMPACTION,
                    WORKSPACE_SPEC_TOOL_NAME,
                    "submit compacted workspace spec tool",
                    timeout_ms,
                    retry_count,
                    save_details,
                )
                .await?
                .arguments)
            }
        },
    )
    .await
}

/// Multi-round full-Markdown LLM compaction when generated Spec Markdown exceeds the hard store limit.
///
/// Shared by local and remote Spec generation jobs. Automatic update jobs must use
/// [`compact_oversized_workspace_spec_update_markdown`] instead (patch-only shrink edits).
pub(crate) async fn compact_oversized_workspace_spec_markdown<F, Fut>(
    model_id: &str,
    max_output_tokens: Option<u32>,
    content_markdown: &str,
    mut invoke_compaction: F,
) -> Result<String, ApiError>
where
    F: FnMut(NeutralChatRequest) -> Fut,
    Fut: std::future::Future<Output = Result<Value, ApiError>>,
{
    if content_markdown.len() <= WORKSPACE_SPEC_MAX_MARKDOWN_BYTES {
        return Ok(content_markdown.to_string());
    }

    let original_bytes = content_markdown.len();
    let mut current = content_markdown.to_string();
    let mut last_compacted_bytes = None;
    let compaction_max_output_tokens = compaction_max_output_tokens(max_output_tokens);

    for attempt in 1..=WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS {
        let target_bytes = workspace_spec_compaction_target_bytes(attempt);
        let required_cut_percent = required_cut_percent(current.len(), target_bytes);
        tracing::warn!(
            content_bytes = current.len(),
            original_bytes,
            max_bytes = WORKSPACE_SPEC_MAX_MARKDOWN_BYTES,
            target_bytes,
            attempt,
            max_attempts = WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS,
            required_cut_percent,
            "workspace spec exceeded size limit; requesting LLM compaction"
        );

        let request = workspace_spec_compaction_provider_request(
            model_id,
            compaction_max_output_tokens,
            &current,
            attempt,
            target_bytes,
        );
        let tool_arguments = invoke_compaction(request).await?;
        let compacted = parse_workspace_spec_output(tool_arguments)?;
        last_compacted_bytes = Some(compacted.len());

        if compacted.len() <= WORKSPACE_SPEC_MAX_MARKDOWN_BYTES {
            tracing::info!(
                original_bytes,
                compacted_bytes = compacted.len(),
                attempt,
                "workspace spec compaction succeeded"
            );
            return Ok(compacted);
        }

        // Feed the shorter candidate into the next round; ignore expansions.
        if compacted.len() < current.len() {
            current = compacted;
        }
    }

    Err(ApiError::bad_request(workspace_spec_markdown_limit_error(
        original_bytes,
        last_compacted_bytes,
    )))
}

/// Multi-round patch-only LLM compaction when an automatic Spec update candidate exceeds the hard store limit.
///
/// Each attempt must return ordered exact-text edits against the current in-memory candidate.
/// Expansions and no-op patches are rejected for that attempt; later rounds continue from the best
/// shorter candidate. Generation must continue using full-Markdown compaction.
pub(crate) async fn compact_oversized_workspace_spec_update_markdown<F, Fut>(
    model_id: &str,
    max_output_tokens: Option<u32>,
    content_markdown: &str,
    mut invoke_compaction: F,
) -> Result<String, ApiError>
where
    F: FnMut(NeutralChatRequest) -> Fut,
    Fut: std::future::Future<Output = Result<Value, ApiError>>,
{
    if content_markdown.len() <= WORKSPACE_SPEC_MAX_MARKDOWN_BYTES {
        return Ok(content_markdown.to_string());
    }

    let original_bytes = content_markdown.len();
    let mut current = content_markdown.to_string();
    let mut last_candidate_bytes = Some(original_bytes);
    let compaction_max_output_tokens = compaction_max_output_tokens(max_output_tokens);

    for attempt in 1..=WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS {
        let target_bytes = workspace_spec_compaction_target_bytes(attempt);
        let required_cut_percent = required_cut_percent(current.len(), target_bytes);
        tracing::warn!(
            content_bytes = current.len(),
            original_bytes,
            max_bytes = WORKSPACE_SPEC_MAX_MARKDOWN_BYTES,
            target_bytes,
            attempt,
            max_attempts = WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS,
            required_cut_percent,
            "workspace spec update exceeded size limit; requesting patch compaction"
        );

        let request = workspace_spec_update_compaction_provider_request(
            model_id,
            compaction_max_output_tokens,
            &current,
            attempt,
            target_bytes,
        );
        let tool_arguments = invoke_compaction(request).await?;
        match parse_workspace_spec_update_compaction_output(tool_arguments, &current) {
            Ok(candidate) => {
                last_candidate_bytes = Some(candidate.len());
                if candidate.len() <= WORKSPACE_SPEC_MAX_MARKDOWN_BYTES {
                    tracing::info!(
                        original_bytes,
                        compacted_bytes = candidate.len(),
                        attempt,
                        "workspace spec update patch compaction succeeded"
                    );
                    return Ok(candidate);
                }
                // Feed only strictly shorter candidates into the next round.
                if candidate.len() < current.len() {
                    current = candidate;
                } else {
                    tracing::warn!(
                        attempt,
                        candidate_bytes = candidate.len(),
                        current_bytes = current.len(),
                        "workspace spec update patch compaction expanded or left size unchanged; keeping best candidate"
                    );
                }
            }
            Err(error) => {
                tracing::warn!(
                    attempt,
                    error = %error.message,
                    "workspace spec update patch compaction attempt rejected; keeping best candidate"
                );
            }
        }
    }

    Err(ApiError::bad_request(
        workspace_spec_update_markdown_limit_error(original_bytes, last_candidate_bytes),
    ))
}

fn compaction_max_output_tokens(base: Option<u32>) -> Option<u32> {
    Some(
        base.unwrap_or(WORKSPACE_SPEC_COMPACTION_MAX_OUTPUT_TOKENS)
            .max(WORKSPACE_SPEC_COMPACTION_MAX_OUTPUT_TOKENS),
    )
}

fn workspace_spec_compaction_target_bytes(attempt: u32) -> usize {
    match attempt {
        1 => WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES,
        2 => WORKSPACE_SPEC_COMPACTION_AGGRESSIVE_TARGET_BYTES,
        _ => WORKSPACE_SPEC_COMPACTION_EMERGENCY_TARGET_BYTES,
    }
}

fn required_cut_percent(current_bytes: usize, target_bytes: usize) -> u32 {
    if current_bytes <= target_bytes {
        return 0;
    }
    let overage = current_bytes - target_bytes;
    ((overage * 100) / current_bytes.max(1)).max(1) as u32
}

pub(crate) fn workspace_spec_compaction_provider_request(
    model_id: &str,
    max_output_tokens: Option<u32>,
    content_markdown: &str,
    attempt: u32,
    target_bytes: usize,
) -> NeutralChatRequest {
    let current_bytes = content_markdown.len();
    let cut_percent = required_cut_percent(current_bytes, target_bytes);
    let aggression = match attempt {
        1 => {
            "Prefer deletion over paraphrasing. Merge duplicate local/remote or repeated facts. Reconcile Open Questions: keep only currently unresolved decisions that affect future work; delete completed work, optional backlog, residual-risk dumps, and verification logs."
        }
        2 => {
            "Be aggressive: delete whole low-value subsections, collapse long matrices into short bullets, keep only durable contracts and still-unresolved Open Questions. Delete completed tasks, optional backlog, residual-risk dumps, and verification logs from Open Questions."
        }
        _ => {
            "Emergency cut: keep Purpose, Architecture, key contracts, and only still-unresolved Open Questions; ruthlessly drop examples, tables, repeated operational prose, completed work, optional backlog, residual-risk dumps, and verification logs."
        }
    };
    let system_prompt = format!(
        "Compress the provided Project Spec Markdown into a complete replacement document. \
Your ONLY success criterion is that contentMarkdown UTF-8 length is STRICTLY under {target_bytes} bytes \
(hard store limit {WORKSPACE_SPEC_MAX_MARKDOWN_BYTES}). \
Preserve the existing language and section shape. Preserve durable product behavior, architecture, runtime flows, data contracts, commands, settings, UI contracts, agent/tool contracts, and operational constraints. \
For Open Questions, reconcile and preserve only currently unresolved decisions or unknowns that still materially affect future product or implementation; do not unconditionally keep historical Open Questions content. \
If a resolved item still defines current behavior, keep a concise final form in the matching formal section first; delete delivery history, phase status, and verification logs rather than dropping live contracts. \
{aggression} \
Omit low-value details such as long file lists, exhaustive symbol lists, repeated facts, transient task history, implementation blow-by-blow notes, verbose local-vs-SSH dual write-ups, and UI copy minutiae unless they define a contract. \
Do not invent facts. Use the submit_workspace_spec tool exactly once."
    );
    let user_prompt = format!(
        "SIZE BUDGET (UTF-8 bytes):\n\
- current: {current_bytes}\n\
- hard_limit: {WORKSPACE_SPEC_MAX_MARKDOWN_BYTES}\n\
- target: {target_bytes}\n\
- required_cut: ~{cut_percent}%\n\
- attempt: {attempt}/{WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS}\n\n\
Submit a substantially shorter complete Project Spec. contentMarkdown MUST be under {target_bytes} bytes.\n\n{}",
        markdown_code_block("markdown", content_markdown)
    );
    NeutralChatRequest {
        model_id: model_id.to_string(),
        messages: vec![
            neutral_text_message(NeutralChatRole::System, system_prompt),
            neutral_text_message(NeutralChatRole::User, user_prompt),
        ],
        tools: vec![workspace_spec_tool_definition()],
        thinking_level: None,
        max_output_tokens,
        prompt_cache_key: None,
        prompt_cache_retention: None,
        agent_correlation: None,
    }
}

/// Build a patch-only compaction request for an oversized automatic Spec update candidate.
pub(crate) fn workspace_spec_update_compaction_provider_request(
    model_id: &str,
    max_output_tokens: Option<u32>,
    content_markdown: &str,
    attempt: u32,
    target_bytes: usize,
) -> NeutralChatRequest {
    let current_bytes = content_markdown.len();
    let cut_percent = required_cut_percent(current_bytes, target_bytes);
    let aggression = match attempt {
        1 => {
            "Prefer deletion and merge via exact-text edits. Reconcile Open Questions: keep only currently unresolved decisions; delete completed work, optional backlog, residual-risk dumps, and verification logs."
        }
        2 => {
            "Be aggressive: delete whole low-value subsections with edits, collapse long matrices into short bullets, keep only durable contracts and still-unresolved Open Questions."
        }
        _ => {
            "Emergency cut via patches only: keep Purpose, Architecture, key contracts, and only still-unresolved Open Questions; ruthlessly delete examples, tables, repeated operational prose, completed work, optional backlog, residual-risk dumps, and verification logs."
        }
    };
    let system_prompt = format!(
        "Shrink the provided Project Spec Markdown candidate using ordered exact-text edits only. \
Your ONLY success criterion is that the Spec after applying all edits has UTF-8 length STRICTLY under {target_bytes} bytes \
(hard store limit {WORKSPACE_SPEC_MAX_MARKDOWN_BYTES}). \
Do not return a full-document replacement. Never submit a single edit whose oldText is the entire candidate document. \
Each edit.oldText must be a non-empty exact substring of the CURRENT candidate Markdown below and must match exactly once after prior edits in the same list are applied. Apply edits in declaration order. \
Preserve the existing language and section shape. Preserve durable product behavior, architecture, runtime flows, data contracts, commands, settings, UI contracts, agent/tool contracts, and operational constraints. \
For Open Questions, reconcile and preserve only currently unresolved decisions or unknowns that still materially affect future product or implementation; do not unconditionally keep historical Open Questions content. \
If a resolved item still defines current behavior, keep a concise final form in the matching formal section first; delete delivery history, phase status, and verification logs rather than dropping live contracts. \
{aggression} \
Omit low-value details such as long file lists, exhaustive symbol lists, repeated facts, transient task history, implementation blow-by-blow notes, verbose local-vs-SSH dual write-ups, and UI copy minutiae unless they define a contract. \
Do not invent facts. Use the submit_workspace_spec_update_compaction tool exactly once."
    );
    let user_prompt = format!(
        "SIZE BUDGET (UTF-8 bytes):\n\
- current: {current_bytes}\n\
- hard_limit: {WORKSPACE_SPEC_MAX_MARKDOWN_BYTES}\n\
- target: {target_bytes}\n\
- required_cut: ~{cut_percent}%\n\
- attempt: {attempt}/{WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS}\n\n\
Submit ordered exact-text edits that shrink the CURRENT candidate. After all edits, the candidate MUST be under {target_bytes} bytes.\n\nCURRENT CANDIDATE:\n{}",
        markdown_code_block("markdown", content_markdown)
    );
    NeutralChatRequest {
        model_id: model_id.to_string(),
        messages: vec![
            neutral_text_message(NeutralChatRole::System, system_prompt),
            neutral_text_message(NeutralChatRole::User, user_prompt),
        ],
        tools: vec![workspace_spec_update_compaction_tool_definition()],
        thinking_level: None,
        max_output_tokens,
        prompt_cache_key: None,
        prompt_cache_retention: None,
        agent_correlation: None,
    }
}

pub(crate) fn workspace_spec_markdown_limit_error(
    original_bytes: usize,
    compacted_bytes: Option<usize>,
) -> String {
    match compacted_bytes {
        Some(compacted_bytes) => format!(
            "workspace spec generation exceeded {} bytes after {} compression attempts (initial {} bytes, last compressed {} bytes). Regenerate, or manually shorten long file lists, repeated facts, transient task history, and low-value implementation details.",
            WORKSPACE_SPEC_MAX_MARKDOWN_BYTES,
            WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS,
            original_bytes,
            compacted_bytes
        ),
        None => format!(
            "workspace spec generation exceeded {} bytes ({} bytes). Regenerate, or manually shorten long file lists, repeated facts, transient task history, and low-value implementation details.",
            WORKSPACE_SPEC_MAX_MARKDOWN_BYTES, original_bytes
        ),
    }
}

pub(crate) fn workspace_spec_update_markdown_limit_error(
    original_bytes: usize,
    compacted_bytes: Option<usize>,
) -> String {
    match compacted_bytes {
        Some(compacted_bytes) => format!(
            "workspace spec update exceeded {} bytes after {} patch-compression attempts (initial {} bytes, last candidate {} bytes). Shorten the Spec with smaller durable contracts, or manually remove low-value implementation details.",
            WORKSPACE_SPEC_MAX_MARKDOWN_BYTES,
            WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS,
            original_bytes,
            compacted_bytes
        ),
        None => format!(
            "workspace spec update exceeded {} bytes ({} bytes). Shorten the Spec with smaller durable contracts, or manually remove low-value implementation details.",
            WORKSPACE_SPEC_MAX_MARKDOWN_BYTES, original_bytes
        ),
    }
}

fn workspace_spec_update_assistant_message_id(
    job: &WorkspaceSpecJobRecord,
) -> Result<String, ApiError> {
    let input: WorkspaceSpecUpdateInput =
        serde_json::from_str(&job.input_summary_json).map_err(|source| {
            ApiError::internal(format!(
                "invalid persisted workspace spec update input: {source}"
            ))
        })?;
    Ok(input.assistant_message_id)
}
fn collect_workspace_spec_input(
    config: &GlobalConfig,
    workspace_id: &str,
    workspace_path: &std::path::Path,
    base_revision: u64,
) -> Result<WorkspaceSpecGenerationInput, ApiError> {
    let mut input =
        collect_workspace_spec_input_without_memory(workspace_id, workspace_path, base_revision)?;
    input.memory_profiles = workspace_memory_profiles(config, workspace_path)?;
    Ok(input)
}

fn collect_workspace_spec_input_without_memory(
    workspace_id: &str,
    workspace_path: &std::path::Path,
    base_revision: u64,
) -> Result<WorkspaceSpecGenerationInput, ApiError> {
    let database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let context = database
        .code_graph_context()
        .map_err(ApiError::from_workspace_error)?;
    let files = database
        .code_graph_file_summaries(WORKSPACE_SPEC_FILE_SUMMARY_LIMIT)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(file_summary_input)
        .collect();
    let symbols = database
        .find_code_graph_symbols("", None, None, WORKSPACE_SPEC_SYMBOL_LIMIT)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(symbol_input)
        .collect();

    Ok(WorkspaceSpecGenerationInput {
        workspace_id: workspace_id.to_string(),
        base_revision,
        code_graph: WorkspaceSpecCodeGraphInput {
            indexed_files: context.indexed_files,
            symbol_count: context.symbols,
            reference_count: context.references,
            edge_count: context.edges,
            languages: context.languages,
            files,
            symbols,
        },
        memory_profiles: Vec::new(),
        source_files: root_source_files(workspace_path),
    })
}

fn workspace_spec_update_input(
    context: &PreparedChatContext,
    database: &WorkspaceDatabase,
    current_spec_revision: u64,
    current_spec_markdown: &str,
) -> Result<WorkspaceSpecUpdateInput, ApiError> {
    let user_content = message_content(database, &context.user_message_id)?;
    let assistant_content = message_content(database, &context.assistant_message_id)?;
    let (user, user_truncated) = compact_text(&user_content, WORKSPACE_SPEC_CHAT_EXCERPT_MAX_CHARS);
    let (assistant, assistant_truncated) =
        compact_text(&assistant_content, WORKSPACE_SPEC_CHAT_EXCERPT_MAX_CHARS);
    let code_change_stats = (context.code_change_stats.additions > 0
        || context.code_change_stats.deletions > 0)
        .then_some(context.code_change_stats.clone());

    Ok(WorkspaceSpecUpdateInput {
        workspace_id: context.workspace_id.clone(),
        chat_id: context.chat_id.clone(),
        current_spec_revision,
        user_message_id: context.user_message_id.clone(),
        assistant_message_id: context.assistant_message_id.clone(),
        run_id: context.llm_request_id.clone(),
        code_change_stats,
        chat_excerpt: WorkspaceSpecChatExcerptInput {
            user,
            user_truncated,
            assistant,
            assistant_truncated,
        },
        current_spec_markdown: current_spec_markdown.to_string(),
    })
}

fn message_content(database: &WorkspaceDatabase, message_id: &str) -> Result<String, ApiError> {
    database
        .message(message_id)
        .map_err(ApiError::from_workspace_error)
        .map(|message| message.map(|message| message.content).unwrap_or_default())
}

fn workspace_memory_profiles(
    config: &GlobalConfig,
    workspace_path: &std::path::Path,
) -> Result<Vec<WorkspaceSpecMemoryProfileInput>, ApiError> {
    if !config.memory.enabled {
        return Ok(Vec::new());
    }

    let database = MemoryDatabase::open_or_create_workspace(workspace_path)
        .map_err(ApiError::from_memory_error)?;
    database
        .profiles_for_scope(None, WORKSPACE_SPEC_MEMORY_PROFILE_LIMIT)
        .map_err(ApiError::from_memory_error)
        .map(|profiles| {
            profiles
                .into_iter()
                .map(|profile| {
                    let (profile_text, truncated) = compact_text(
                        &profile.profile_text,
                        WORKSPACE_SPEC_MEMORY_PROFILE_MAX_CHARS,
                    );
                    WorkspaceSpecMemoryProfileInput {
                        id: profile.id,
                        scope: profile.scope,
                        profile_text,
                        truncated,
                    }
                })
                .collect()
        })
}

fn root_source_files(workspace_path: &std::path::Path) -> Vec<WorkspaceSpecSourceFileInput> {
    let mut files = Vec::new();
    for relative_path in ROOT_SOURCE_FILE_CANDIDATES {
        if files.len() >= WORKSPACE_SPEC_ROOT_FILE_LIMIT {
            break;
        }
        let path = workspace_path.join(relative_path);
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let (content, truncated) = compact_text(&content, WORKSPACE_SPEC_SOURCE_FILE_MAX_CHARS);
        files.push(WorkspaceSpecSourceFileInput {
            path: (*relative_path).to_string(),
            size_bytes: metadata.len(),
            content,
            truncated,
        });
    }
    files
}

fn workspace_spec_provider_request(
    model_id: &str,
    app_language: &str,
    generation_system_prompt: Option<&str>,
    max_output_tokens: u32,
    input_summary: &WorkspaceSpecGenerationInput,
) -> Result<NeutralChatRequest, ApiError> {
    let input_json = serde_json::to_string_pretty(input_summary).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize workspace spec evidence: {source}"
        ))
    })?;
    let system_prompt = workspace_spec_system_prompt(
        generation_system_prompt,
        default_workspace_spec_generation_system_prompt(),
        app_language,
    );

    Ok(NeutralChatRequest {
        model_id: model_id.to_string(),
        messages: vec![
            neutral_text_message(NeutralChatRole::System, system_prompt),
            neutral_text_message(
                NeutralChatRole::User,
                format!("Evidence JSON:\n{input_json}"),
            ),
        ],
        tools: vec![workspace_spec_tool_definition()],
        thinking_level: None,
        max_output_tokens: Some(max_output_tokens),
        prompt_cache_key: None,
        prompt_cache_retention: None,
        agent_correlation: None,
    })
}

fn workspace_spec_update_provider_request(
    model_id: &str,
    app_language: &str,
    update_system_prompt: Option<&str>,
    max_output_tokens: u32,
    input_summary: &WorkspaceSpecUpdateInput,
) -> Result<NeutralChatRequest, ApiError> {
    let input_json = serde_json::to_string_pretty(input_summary).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize workspace spec update input: {source}"
        ))
    })?;
    let system_prompt = workspace_spec_system_prompt(
        update_system_prompt,
        default_workspace_spec_update_system_prompt(),
        app_language,
    );

    Ok(NeutralChatRequest {
        model_id: model_id.to_string(),
        messages: vec![
            neutral_text_message(NeutralChatRole::System, system_prompt),
            neutral_text_message(
                NeutralChatRole::User,
                format!("Workspace spec update input JSON:\n{input_json}"),
            ),
        ],
        tools: vec![workspace_spec_update_tool_definition()],
        thinking_level: None,
        max_output_tokens: Some(max_output_tokens),
        prompt_cache_key: None,
        prompt_cache_retention: None,
        agent_correlation: None,
    })
}

fn workspace_spec_tool_definition() -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: WORKSPACE_SPEC_TOOL_NAME.to_string(),
        description: "Submit the generated Project Spec Markdown.".to_string(),
        strict: true,
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "contentMarkdown": {
                    "type": "string",
                    "maxLength": WORKSPACE_SPEC_MAX_MARKDOWN_BYTES,
                    "description": "Full replacement Markdown for the Project Spec."
                }
            },
            "required": ["contentMarkdown"]
        }),
    }
}

fn workspace_spec_update_tool_definition() -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: WORKSPACE_SPEC_UPDATE_TOOL_NAME.to_string(),
        description: "Submit whether the Project Spec needs an update and, when needed, ordered exact-text edits against the current Spec Markdown from the input.".to_string(),
        strict: true,
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "updateNeeded": {
                    "type": "boolean",
                    "description": "True only when the completed chat turn changed durable project spec content."
                },
                "edits": {
                    "type": ["array", "null"],
                    "description": "Ordered exact-text patches when updateNeeded is true; null when updateNeeded is false. Each oldText must be non-empty and match the current Spec from the input exactly once. Do not use a single edit that replaces the entire document.",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "oldText": {
                                "type": "string",
                                "description": "Exact non-empty substring of the current Project Spec Markdown."
                            },
                            "newText": {
                                "type": "string",
                                "description": "Replacement text for oldText. Use empty string to delete."
                            }
                        },
                        "required": ["oldText", "newText"]
                    }
                }
            },
            "required": ["updateNeeded", "edits"]
        }),
    }
}

fn workspace_spec_update_compaction_tool_definition() -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: WORKSPACE_SPEC_UPDATE_COMPACTION_TOOL_NAME.to_string(),
        description: "Submit ordered exact-text edits that shrink the current oversized Project Spec candidate. Do not return full-document Markdown.".to_string(),
        strict: true,
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "edits": {
                    "type": "array",
                    "description": "Ordered exact-text patches against the current in-memory candidate. Each oldText must be non-empty and match exactly once. Do not use a single edit that replaces the entire document.",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "oldText": {
                                "type": "string",
                                "description": "Exact non-empty substring of the current candidate Markdown."
                            },
                            "newText": {
                                "type": "string",
                                "description": "Replacement text for oldText. Prefer shorter text; empty string deletes."
                            }
                        },
                        "required": ["oldText", "newText"]
                    }
                }
            },
            "required": ["edits"]
        }),
    }
}

/// Shared Project Spec definition, admission, and exclusion baseline for generation and update prompts.
fn workspace_spec_definition_and_admission_baseline() -> &'static str {
    "Project Spec definition: the Spec is the project's current, concise, normative truth. \
It constrains future product behavior, cross-module contracts, data compatibility, security boundaries, \
and deliberately retained architecture and operational constraints. It is not an implementation manual, \
code index, changelog, task log, delivery record, or test report. \
Non-goals / exclude: implementation how-to guides; exhaustive code, file, or symbol indexes; changelogs; \
task history; Phase or delivery status; test reports; commit records; process narratives about how the \
current state was reached. \
Admission test: include an implementation detail only when changing it would violate external behavior, \
compatibility, security, a stable architecture boundary, or an important operational requirement. \
Still accurate alone is not a reason to keep content."
}

fn workspace_spec_fixed_sections_instruction() -> &'static str {
    "Use exactly these sections: # Project Spec, ## Purpose, ## Product Surface, ## Architecture, \
## Data And Persistence, ## Runtime Flows, ## UI Contracts, ## Agent And Tool Contracts, \
## Operational Constraints, ## Open Questions."
}

fn workspace_spec_open_questions_instruction() -> &'static str {
    "Open Questions is only for currently unresolved decisions or unknowns that will materially affect \
future product or implementation. Do not use it as a changelog, completed-work ledger, backlog, or \
residual-risk dump. When evidence is already implemented, decided, explicitly out of scope, or only an \
optional future optimization, do not put it in Open Questions; place durable final behavior in \
Architecture, Data And Persistence, Runtime Flows, UI/Agent Contracts, or Operational Constraints instead. \
If there are no valid unresolved questions, keep the ## Open Questions section and write a short None \
marker; do not invent questions."
}

pub(crate) fn default_workspace_spec_generation_system_prompt() -> String {
    format!(
        "Generate a complete Project Spec Markdown document by distilling normative current state from provided evidence, not by restating evidence as documentation. \
{} \
{} \
Prefer facts evidenced by code graph summaries, workspace memory profiles, or root source reads. Do not invent product claims. \
Place each fact in exactly one best-fit section; do not copy the same contract across sections for completeness. \
Express behavior and boundaries first. Do not restate code structure, function call chains, migration or schema version numbers, test paths, or UI copy wording unless those items themselves are stable contracts. \
Actively merge similar facts, choose high information-density wording, and treat the soft target of {WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES} bytes as a real editing budget from the first draft; do not rely on later hard-limit compaction. \
{} \
Hard limit is {WORKSPACE_SPEC_MAX_MARKDOWN_BYTES} bytes. Use the submit_workspace_spec tool exactly once.",
        workspace_spec_definition_and_admission_baseline(),
        workspace_spec_fixed_sections_instruction(),
        workspace_spec_open_questions_instruction(),
    )
}

pub(crate) fn default_workspace_spec_update_system_prompt() -> String {
    format!(
        "Decide whether the Project Spec needs an update after the latest completed chat turn. \
{} \
Distinguish code changed from normative truth changed. Pure refactors, internal helpers, test fill-in, completion reports, or implementation process alone default to updateNeeded=false and edits=null. \
If the turn did not change durable product behavior, architecture, runtime flows, data contracts, commands, settings, or operational constraints, and Open Questions already comply with the rules below, submit updateNeeded=false and edits=null. \
If an update is needed, re-examine the entire existing Spec in the input (currentSpecMarkdown), not only this turn's delta, and submit the smallest ordered exact-text edits that bring the Spec to the correct normative state. Prefer replace, merge, and delete over append. \
Each edit.oldText must be a non-empty exact substring of the current Spec and match exactly once after prior edits in the same list are applied. Apply edits in declaration order. Never submit a single edit whose oldText is the entire current Spec document to fake a full replacement. \
For every fact, choose keep, merge, replace, or delete. Prefer replace or merge over append when new information overlaps existing wording. Delete content that is accurate but fails the admission test, is duplicated, or is too fine-grained. Still accurate is not a reason to keep. Do not accumulate implementation history in chronological order. \
Default expectation: the updated Spec is not longer than before; offset any additions with dedupe, merge, and delete. Net growth is allowed only for genuine new normative scope that cannot be expressed by editing existing entries, and must still meet the soft target of {WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES} bytes. \
Before submitting edits, re-examine every existing Open Questions item (do not only append this turn's changes). \
{} \
Items already resolved, implemented, or explicitly decided by the latest chat evidence must leave Open Questions; move durable conclusions into the matching formal section as needed, without Phase numbers, delivered status, test commands, commit records, or implementation logs. Optional follow-ups, refactor opportunities, and optimization ideas with no concrete unresolved decision default to deletion; rewrite as a short question only when a real open choice remains. \
A chat turn that only reports completion status with no new durable contracts may still set updateNeeded=true to clean stale Open Questions; if the Spec already complies and has no other durable changes, return false. Do not invent product claims. \
{} \
Hard limit is {WORKSPACE_SPEC_MAX_MARKDOWN_BYTES} bytes for the Spec after all edits. Use the submit_workspace_spec_update tool exactly once.",
        workspace_spec_definition_and_admission_baseline(),
        workspace_spec_open_questions_instruction(),
        workspace_spec_fixed_sections_instruction(),
    )
}

fn workspace_spec_system_prompt(
    custom: Option<&str>,
    default_prompt: String,
    app_language: &str,
) -> String {
    let prompt = custom
        .and_then(non_empty_trimmed)
        .map(str::to_string)
        .unwrap_or(default_prompt);
    format!(
        "{}\n\n{}",
        prompt.trim_end(),
        workspace_spec_language_instruction(app_language)
    )
}

fn workspace_spec_language_instruction(app_language: &str) -> String {
    format!(
        "Language preference: follow the current Foco app language setting ({app_language}); write Project Spec prose in {}. Preserve code identifiers, file paths, commands, API names, and proper nouns when translation would reduce accuracy.",
        workspace_spec_language_name(app_language)
    )
}

// ponytail: local mapping is enough for the two supported app languages; extend with SUPPORTED_APP_LANGUAGES.
fn workspace_spec_language_name(app_language: &str) -> &'static str {
    match app_language {
        "zh-CN" => "Simplified Chinese",
        _ => "English",
    }
}

pub(crate) fn parse_workspace_spec_output(value: Value) -> Result<String, ApiError> {
    let output: WorkspaceSpecToolOutput = serde_json::from_value(value).map_err(|source| {
        ApiError::bad_request(format!(
            "malformed workspace spec generation JSON: {source}"
        ))
    })?;
    let content = output.content_markdown.trim().to_string();
    if content.is_empty() {
        return Err(ApiError::bad_request(
            "workspace spec generation returned empty Markdown",
        ));
    }
    Ok(content)
}

pub(crate) fn mark_workspace_spec_job_failed(
    workspace_path: &std::path::Path,
    workspace_id: &str,
    job_id: &str,
    error_message: &str,
) {
    mark_workspace_spec_job_failed_at_path(workspace_path, workspace_id, job_id, error_message);
}

pub(crate) fn claim_next_workspace_spec_job_for_path(
    workspace_path: &std::path::Path,
) -> Result<Option<WorkspaceSpecJobRecord>, ApiError> {
    claim_next_workspace_spec_job(workspace_path)
}

pub(crate) fn recover_stale_running_workspace_spec_job_for_path(
    workspace_path: &std::path::Path,
    workspace_id: &str,
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    recover_stale_running_workspace_spec_job(&mut database, workspace_id)?;
    Ok(())
}

pub(crate) fn parse_workspace_spec_update_output(
    value: Value,
    base_markdown: &str,
) -> Result<WorkspaceSpecUpdateOutput, ApiError> {
    let output: WorkspaceSpecUpdateToolOutput =
        serde_json::from_value(value).map_err(|source| {
            ApiError::bad_request(format!("malformed workspace spec update JSON: {source}"))
        })?;
    if !output.update_needed {
        if output.edits.is_some() {
            return Err(ApiError::bad_request(
                "workspace spec update with updateNeeded=false must set edits=null",
            ));
        }
        return Ok(WorkspaceSpecUpdateOutput::NoUpdateNeeded);
    }

    let Some(edits) = output.edits else {
        return Err(ApiError::bad_request(
            "workspace spec update with updateNeeded=true requires non-empty edits",
        ));
    };
    if edits.is_empty() {
        return Err(ApiError::bad_request(
            "workspace spec update with updateNeeded=true requires non-empty edits",
        ));
    }

    let content_markdown = apply_spec_text_edits(base_markdown, &edits)
        .map_err(|error| ApiError::bad_request(map_spec_patch_error_message(error)))?;
    Ok(WorkspaceSpecUpdateOutput::Patch {
        edits,
        content_markdown,
    })
}

fn parse_workspace_spec_update_compaction_output(
    value: Value,
    base_markdown: &str,
) -> Result<String, ApiError> {
    let output: WorkspaceSpecUpdateCompactionToolOutput =
        serde_json::from_value(value).map_err(|source| {
            ApiError::bad_request(format!(
                "malformed workspace spec update compaction JSON: {source}"
            ))
        })?;
    if output.edits.is_empty() {
        return Err(ApiError::bad_request(
            "workspace spec update compaction requires non-empty edits",
        ));
    }
    apply_spec_text_edits(base_markdown, &output.edits)
        .map_err(|error| ApiError::bad_request(map_spec_patch_error_message(error)))
}

fn map_spec_patch_error_message(error: SpecPatchError) -> String {
    error.message()
}

fn resolve_workspace_spec_model(
    config: &GlobalConfig,
    requested_model_id: Option<&str>,
) -> Result<WorkspaceSpecModelSelection, ApiError> {
    let model_id = match requested_model_id.and_then(non_empty_trimmed) {
        Some(model_id) => model_id.to_string(),
        None => only_configured_generation_model(&config.models)?.id.clone(),
    };
    let model = config
        .models
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "workspace spec generation model was not found: {model_id}"
            ))
        })?;
    workspace_spec_model_selection(config, model)
}

fn resolve_workspace_spec_model_from_models(
    models: &[ModelSettings],
    requested_model_id: Option<&str>,
) -> Result<RemoteWorkspaceSpecModelSelection, ApiError> {
    let model = match requested_model_id.and_then(non_empty_trimmed) {
        Some(model_id) => models
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "workspace spec generation model was not found: {model_id}"
                ))
            })?,
        None => only_configured_generation_model(models)?,
    };
    remote_workspace_spec_model_selection(model)
}

fn only_configured_generation_model(models: &[ModelSettings]) -> Result<&ModelSettings, ApiError> {
    let candidates = models
        .iter()
        .filter(|model| model.enabled && model.active_provider_id.is_some())
        .collect::<Vec<_>>();
    if candidates.len() == 1 {
        return Ok(candidates[0]);
    }

    Err(ApiError::bad_request(
        "workspace spec generation model is not configured; pass modelId",
    ))
}

#[derive(Debug)]
struct RemoteWorkspaceSpecModelSelection {
    model_id: String,
    provider_id: String,
    max_output_tokens: u32,
}

fn remote_workspace_spec_model_selection(
    model: &ModelSettings,
) -> Result<RemoteWorkspaceSpecModelSelection, ApiError> {
    if !model.enabled {
        return Err(ApiError::bad_request(format!(
            "workspace spec generation model '{}' is disabled",
            model.id
        )));
    }
    let limits = model.limits.as_ref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "workspace spec generation model '{}' is missing limits",
            model.id
        ))
    })?;
    let provider_id = model.active_provider_id.as_deref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "workspace spec generation model '{}' has no active provider selected",
            model.id
        ))
    })?;
    if !model.provider_ids.iter().any(|id| id == provider_id) {
        return Err(ApiError::bad_request(format!(
            "active provider '{}' is not associated with workspace spec generation model '{}'",
            provider_id, model.id
        )));
    }
    let max_output_tokens = u32::try_from(limits.max_output_tokens)
        .map_err(|_| {
            ApiError::bad_request(format!(
                "workspace spec generation model '{}' max output tokens exceed u32: {}",
                model.id, limits.max_output_tokens
            ))
        })?
        .min(WORKSPACE_SPEC_MAX_OUTPUT_TOKENS);

    Ok(RemoteWorkspaceSpecModelSelection {
        model_id: model.id.clone(),
        provider_id: provider_id.to_string(),
        max_output_tokens,
    })
}

fn workspace_spec_model_selection(
    config: &GlobalConfig,
    model: &ModelSettings,
) -> Result<WorkspaceSpecModelSelection, ApiError> {
    let remote = remote_workspace_spec_model_selection(model)?;
    let provider = config
        .providers
        .iter()
        .find(|provider| provider.id == remote.provider_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "workspace spec generation provider '{}' was not found",
                remote.provider_id
            ))
        })?;
    if !provider.enabled {
        return Err(ApiError::bad_request(format!(
            "workspace spec generation provider '{}' is disabled",
            provider.id
        )));
    }

    Ok(WorkspaceSpecModelSelection {
        model_id: remote.model_id,
        provider_id: remote.provider_id,
        provider_config: provider_connection_config(provider)?,
        max_output_tokens: remote.max_output_tokens,
    })
}

fn claim_next_workspace_spec_job(
    workspace_path: &std::path::Path,
) -> Result<Option<WorkspaceSpecJobRecord>, ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    database
        .claim_next_workspace_spec_job()
        .map_err(ApiError::from_workspace_error)
}

fn mark_workspace_spec_job_failed_at_path(
    workspace_path: &std::path::Path,
    workspace_id: &str,
    job_id: &str,
    error_message: &str,
) {
    let Ok(mut database) = WorkspaceDatabase::open_or_create(workspace_path) else {
        return;
    };
    if let Err(error) = database.mark_workspace_spec_job_failed(job_id, error_message) {
        tracing::warn!(
            job_id,
            error = %error,
            "failed to mark workspace spec job failed"
        );
    }
    log_workspace_spec_job_status_from_database(&database, workspace_id, job_id);
}

pub(crate) fn log_workspace_spec_job_status(workspace_id: &str, job: &WorkspaceSpecJobRecord) {
    let skip_reason = job
        .status
        .eq(WorkspaceSpecJobStatus::Skipped.as_str())
        .then(|| job.error_message.as_deref())
        .flatten()
        .unwrap_or("");
    let stale_skip_reason = if skip_reason == WORKSPACE_SPEC_STALE_REVISION_SKIP_REASON {
        skip_reason
    } else {
        ""
    };
    tracing::info!(
        workspace_id = %workspace_id,
        job_id = %job.id,
        trigger_type = %job.trigger_type,
        status = %job.status,
        skip_reason = %skip_reason,
        stale_skip_reason = %stale_skip_reason,
        "workspace spec job status"
    );
}

fn log_workspace_spec_job_status_at_path(
    workspace_path: &std::path::Path,
    workspace_id: &str,
    job_id: &str,
) {
    let Ok(database) = WorkspaceDatabase::open_or_create(workspace_path) else {
        return;
    };
    log_workspace_spec_job_status_from_database(&database, workspace_id, job_id);
}

fn log_workspace_spec_job_status_from_database(
    database: &WorkspaceDatabase,
    workspace_id: &str,
    job_id: &str,
) {
    match database.workspace_spec_job(job_id) {
        Ok(Some(job)) => log_workspace_spec_job_status(workspace_id, &job),
        Ok(None) => tracing::warn!(
            workspace_id = %workspace_id,
            job_id = %job_id,
            "workspace spec job status could not be logged because the job was not found"
        ),
        Err(error) => tracing::warn!(
            workspace_id = %workspace_id,
            job_id = %job_id,
            error = %error,
            "workspace spec job status could not be logged"
        ),
    }
}

fn file_summary_input(summary: CodeGraphFileSummaryRecord) -> WorkspaceSpecFileSummaryInput {
    WorkspaceSpecFileSummaryInput {
        path: summary.path,
        language: summary.language,
        symbol_count: summary.symbol_count,
        import_count: summary.import_count,
        import_modules: summary.import_modules,
    }
}

fn symbol_input(symbol: CodeGraphSymbolRecord) -> WorkspaceSpecSymbolInput {
    WorkspaceSpecSymbolInput {
        path: symbol.path,
        language: symbol.language,
        name: symbol.name,
        kind: symbol.kind,
        signature: symbol.signature,
    }
}

fn compact_text(value: &str, max_chars: usize) -> (String, bool) {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        return (compact, false);
    }
    let mut clipped = compact.chars().take(max_chars).collect::<String>();
    clipped.push_str("...");
    (clipped, true)
}

fn non_empty_trimmed(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_spec_tool_schemas_cap_markdown_length() {
        let generate_tool = workspace_spec_tool_definition();
        assert_eq!(
            generate_tool.input_schema["properties"]["contentMarkdown"]["maxLength"].as_u64(),
            Some(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES as u64)
        );

        let update_tool = workspace_spec_update_tool_definition();
        assert_eq!(
            update_tool.input_schema["properties"]["edits"]["type"],
            json!(["array", "null"])
        );
        assert_eq!(
            update_tool.input_schema["properties"]["edits"]["items"]["required"],
            json!(["oldText", "newText"])
        );
        assert!(
            update_tool.input_schema["properties"]
                .as_object()
                .expect("properties")
                .get("contentMarkdown")
                .is_none()
        );
        assert_eq!(
            update_tool.input_schema["required"],
            json!(["updateNeeded", "edits"])
        );
    }

    #[test]
    fn workspace_spec_prompts_share_definition_and_admission_baseline() {
        let generation = default_workspace_spec_generation_system_prompt();
        let update = default_workspace_spec_update_system_prompt();
        let baseline = workspace_spec_definition_and_admission_baseline();

        assert!(generation.contains(baseline));
        assert!(update.contains(baseline));
        for prompt in [&generation, &update] {
            assert!(prompt.contains("current, concise, normative truth"));
            assert!(prompt.contains("cross-module contracts"));
            assert!(prompt.contains("security boundaries"));
            assert!(prompt.contains("not an implementation manual"));
            assert!(prompt.contains("code index"));
            assert!(prompt.contains("changelog"));
            assert!(prompt.contains("task history"));
            assert!(prompt.contains("Phase or delivery status"));
            assert!(prompt.contains("test reports"));
            assert!(prompt.contains("commit records"));
            assert!(prompt.contains("process narratives"));
            assert!(prompt.contains("Admission test"));
            assert!(prompt.contains("external behavior"));
            assert!(prompt.contains("Still accurate alone is not a reason to keep content"));
        }
    }

    #[test]
    fn workspace_spec_generation_prompt_controls_information_density() {
        let prompt = default_workspace_spec_generation_system_prompt();

        assert!(prompt.contains("distilling normative current state"));
        assert!(prompt.contains("not by restating evidence as documentation"));
        assert!(prompt.contains("exactly one best-fit section"));
        assert!(prompt.contains("do not copy the same contract across sections"));
        assert!(prompt.contains("Express behavior and boundaries first"));
        assert!(prompt.contains("function call chains"));
        assert!(prompt.contains("migration or schema version numbers"));
        assert!(prompt.contains("test paths"));
        assert!(prompt.contains("high information-density wording"));
        assert!(prompt.contains("real editing budget from the first draft"));
        assert!(prompt.contains("do not rely on later hard-limit compaction"));
        assert!(prompt.contains(&WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES.to_string()));
        assert!(prompt.contains(&WORKSPACE_SPEC_MAX_MARKDOWN_BYTES.to_string()));
        assert!(prompt.contains("submit_workspace_spec tool exactly once"));
    }

    #[test]
    fn workspace_spec_generation_prompt_open_questions_are_unresolved_only() {
        let prompt = default_workspace_spec_generation_system_prompt();

        assert!(prompt.contains("Open Questions is only for currently unresolved"));
        assert!(prompt.contains("materially affect future product or implementation"));
        assert!(prompt.contains("changelog"));
        assert!(prompt.contains("completed-work ledger"));
        assert!(prompt.contains("backlog"));
        assert!(prompt.contains("residual-risk dump"));
        assert!(prompt.contains("do not put it in Open Questions"));
        assert!(prompt.contains("write a short None marker"));
        assert!(prompt.contains(&WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES.to_string()));
        assert!(prompt.contains(&WORKSPACE_SPEC_MAX_MARKDOWN_BYTES.to_string()));
    }

    #[test]
    fn workspace_spec_update_prompt_anti_entropy_rules() {
        let prompt = default_workspace_spec_update_system_prompt();

        assert!(prompt.contains("code changed from normative truth changed"));
        assert!(prompt.contains("Pure refactors, internal helpers, test fill-in"));
        assert!(prompt.contains("default to updateNeeded=false"));
        assert!(prompt.contains("edits=null"));
        assert!(prompt.contains("re-examine the entire existing Spec"));
        assert!(prompt.contains("smallest ordered exact-text edits"));
        assert!(prompt.contains("Never submit a single edit whose oldText is the entire"));
        assert!(prompt.contains("Prefer replace, merge, and delete"));
        assert!(prompt.contains("Still accurate is not a reason to keep"));
        assert!(prompt.contains("Do not accumulate implementation history"));
        assert!(prompt.contains("not longer than before"));
        assert!(prompt.contains("Net growth is allowed only for genuine new normative scope"));
        assert!(
            !prompt.contains("Preserve accurate existing facts unless the turn supersedes them")
        );
        assert!(!prompt.contains("full replacement Markdown document"));
        assert!(!prompt.contains("contentMarkdown=null"));
        assert!(prompt.contains(&WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES.to_string()));
        assert!(prompt.contains(&WORKSPACE_SPEC_MAX_MARKDOWN_BYTES.to_string()));
        assert!(prompt.contains("submit_workspace_spec_update tool exactly once"));
    }

    #[test]
    fn workspace_spec_update_prompt_omits_non_normative_content() {
        let prompt = default_workspace_spec_update_system_prompt();

        assert!(prompt.contains("implementation how-to guides"));
        assert!(prompt.contains("task history"));
        assert!(prompt.contains("commit records"));
        assert!(prompt.contains("fails the admission test"));
        assert!(prompt.contains(&WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES.to_string()));
        assert!(prompt.contains(&WORKSPACE_SPEC_MAX_MARKDOWN_BYTES.to_string()));
    }

    #[test]
    fn workspace_spec_update_prompt_reconciles_open_questions() {
        let prompt = default_workspace_spec_update_system_prompt();

        assert!(prompt.contains("re-examine every existing Open Questions item"));
        assert!(prompt.contains("do not only append"));
        assert!(prompt.contains("must leave Open Questions"));
        assert!(prompt.contains("move durable conclusions into the matching formal section"));
        assert!(prompt.contains("without Phase numbers"));
        assert!(prompt.contains("Optional follow-ups"));
        assert!(prompt.contains("default to deletion"));
        assert!(prompt.contains("may still set updateNeeded=true to clean stale Open Questions"));
        assert!(prompt.contains("Open Questions already comply"));
        assert!(prompt.contains("Before submitting edits"));
        assert!(!prompt.contains("Before each full replacement"));
    }

    #[test]
    fn workspace_spec_limit_error_reports_retry_sizes() {
        let message = workspace_spec_markdown_limit_error(67_826, Some(66_000));

        assert!(message.contains("65536 bytes"));
        assert!(message.contains("initial 67826 bytes"));
        assert!(message.contains("last compressed 66000 bytes"));
        assert!(message.contains("3 compression attempts"));
        assert!(message.contains("low-value implementation details"));
    }

    #[test]
    fn workspace_spec_compaction_prompt_includes_size_budget() {
        let request = workspace_spec_compaction_provider_request(
            "model-1",
            Some(8_000),
            &"x".repeat(70_000),
            2,
            WORKSPACE_SPEC_COMPACTION_AGGRESSIVE_TARGET_BYTES,
        );

        let system = request.messages[0].content.as_str();
        let user = request.messages[1].content.as_str();
        assert!(system.contains(&WORKSPACE_SPEC_COMPACTION_AGGRESSIVE_TARGET_BYTES.to_string()));
        assert!(system.contains("STRICTLY under"));
        assert!(system.contains("Be aggressive"));
        assert!(system.contains("reconcile and preserve only currently unresolved"));
        assert!(system.contains("do not unconditionally keep historical Open Questions"));
        assert!(!system.contains("operational constraints, and open questions"));
        assert!(system.contains("completed tasks, optional backlog"));
        assert!(user.contains("SIZE BUDGET"));
        assert!(user.contains("70000"));
        assert!(user.contains("attempt: 2/3"));
        assert_eq!(request.max_output_tokens, Some(8_000));
    }

    #[test]
    fn workspace_spec_compaction_prompts_reconcile_open_questions_per_attempt() {
        for attempt in 1..=3 {
            let request = workspace_spec_compaction_provider_request(
                "model-1",
                None,
                "short",
                attempt,
                workspace_spec_compaction_target_bytes(attempt),
            );
            let system = request.messages[0].content.as_str();
            assert!(
                system.contains("reconcile and preserve only currently unresolved"),
                "attempt {attempt} missing shared open-questions reconciliation"
            );
            assert!(
                !system.contains("operational constraints, and open questions"),
                "attempt {attempt} must not unconditionally preserve open questions"
            );
            assert!(
                system.contains("keep a concise final form in the matching formal section"),
                "attempt {attempt} missing archive-to-formal-section guidance"
            );
        }

        let emergency = workspace_spec_compaction_provider_request(
            "model-1",
            None,
            "short",
            3,
            WORKSPACE_SPEC_COMPACTION_EMERGENCY_TARGET_BYTES,
        );
        let emergency_system = emergency.messages[0].content.as_str();
        assert!(emergency_system.contains("only still-unresolved Open Questions"));
        assert!(!emergency_system.contains("key contracts, and Open Questions;"));
    }

    #[test]
    fn workspace_spec_compaction_targets_tighten_by_attempt() {
        assert_eq!(
            workspace_spec_compaction_target_bytes(1),
            WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES
        );
        assert_eq!(
            workspace_spec_compaction_target_bytes(2),
            WORKSPACE_SPEC_COMPACTION_AGGRESSIVE_TARGET_BYTES
        );
        assert_eq!(
            workspace_spec_compaction_target_bytes(3),
            WORKSPACE_SPEC_COMPACTION_EMERGENCY_TARGET_BYTES
        );
    }

    #[test]
    fn compaction_max_output_tokens_raises_generation_cap() {
        assert_eq!(
            compaction_max_output_tokens(Some(4_000)),
            Some(WORKSPACE_SPEC_COMPACTION_MAX_OUTPUT_TOKENS)
        );
        assert_eq!(compaction_max_output_tokens(Some(32_000)), Some(32_000));
        assert_eq!(
            compaction_max_output_tokens(None),
            Some(WORKSPACE_SPEC_COMPACTION_MAX_OUTPUT_TOKENS)
        );
    }

    #[tokio::test]
    async fn compact_oversized_workspace_spec_retries_until_under_limit() {
        let oversized = "x".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 1_000);
        let mut attempts = 0_u32;
        let result = compact_oversized_workspace_spec_markdown(
            "model-1",
            Some(4_000),
            &oversized,
            |_request| {
                attempts += 1;
                let body = if attempts < 3 {
                    "y".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 500)
                } else {
                    "z".repeat(WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES - 100)
                };
                async move { Ok(json!({ "contentMarkdown": body })) }
            },
        )
        .await
        .expect("compaction should succeed on third attempt");

        assert_eq!(attempts, 3);
        assert_eq!(result.len(), WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES - 100);
        assert!(result.len() <= WORKSPACE_SPEC_MAX_MARKDOWN_BYTES);
    }

    #[tokio::test]
    async fn compact_oversized_workspace_spec_fails_after_max_attempts() {
        let oversized = "x".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 2_000);
        let mut attempts = 0_u32;
        let error =
            compact_oversized_workspace_spec_markdown("model-1", None, &oversized, |_request| {
                attempts += 1;
                let body = "y".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 1_500);
                async move { Ok(json!({ "contentMarkdown": body })) }
            })
            .await
            .expect_err("should fail when every attempt stays over limit");

        assert_eq!(attempts, WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS);
        let message = error.message();
        assert!(message.contains("3 compression attempts"));
        assert!(message.contains(&format!("initial {} bytes", oversized.len())));
        assert!(message.contains(&format!(
            "last compressed {} bytes",
            WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 1_500
        )));
    }

    #[tokio::test]
    async fn compact_oversized_workspace_spec_returns_unchanged_when_within_limit() {
        let content = "short enough".to_string();
        let mut attempts = 0_u32;
        let result =
            compact_oversized_workspace_spec_markdown("model-1", None, &content, |_request| {
                attempts += 1;
                async move { Ok(json!({ "contentMarkdown": "should not run" })) }
            })
            .await
            .expect("under-limit content should pass through");

        assert_eq!(attempts, 0);
        assert_eq!(result, content);
    }

    #[test]
    fn workspace_spec_update_compaction_tool_schema_is_edits_only() {
        let tool = workspace_spec_update_compaction_tool_definition();
        assert_eq!(tool.name, WORKSPACE_SPEC_UPDATE_COMPACTION_TOOL_NAME);
        assert_eq!(tool.input_schema["required"], json!(["edits"]));
        assert!(
            tool.input_schema["properties"]
                .as_object()
                .expect("properties")
                .get("contentMarkdown")
                .is_none()
        );
        assert_eq!(
            tool.input_schema["properties"]["edits"]["items"]["required"],
            json!(["oldText", "newText"])
        );
    }

    #[test]
    fn workspace_spec_update_compaction_prompt_is_patch_only() {
        let request = workspace_spec_update_compaction_provider_request(
            "model-1",
            Some(8_000),
            &"x".repeat(70_000),
            1,
            WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES,
        );
        let system = request.messages[0].content.as_str();
        let user = request.messages[1].content.as_str();
        assert!(system.contains("ordered exact-text edits only"));
        assert!(system.contains("Never submit a single edit whose oldText is the entire"));
        assert!(system.contains(WORKSPACE_SPEC_UPDATE_COMPACTION_TOOL_NAME));
        assert!(!system.contains("complete replacement document"));
        assert!(!system.contains("contentMarkdown MUST"));
        assert!(user.contains("SIZE BUDGET"));
        assert!(user.contains("CURRENT CANDIDATE"));
        assert_eq!(
            request.tools[0].name,
            WORKSPACE_SPEC_UPDATE_COMPACTION_TOOL_NAME
        );
    }

    #[tokio::test]
    async fn compact_oversized_workspace_spec_update_returns_unchanged_when_within_limit() {
        let content = "short enough".to_string();
        let mut attempts = 0_u32;
        let result = compact_oversized_workspace_spec_update_markdown(
            "model-1",
            None,
            &content,
            |_request| {
                attempts += 1;
                async move {
                    Ok(json!({
                        "edits": [{ "oldText": "short", "newText": "x" }]
                    }))
                }
            },
        )
        .await
        .expect("under-limit content should pass through");

        assert_eq!(attempts, 0);
        assert_eq!(result, content);
    }

    #[tokio::test]
    async fn compact_oversized_workspace_spec_update_applies_patch_rounds() {
        let oversized = format!(
            "HEAD\n{}",
            "y".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 500)
        );
        let mut attempts = 0_u32;
        let last_candidate = std::sync::Arc::new(std::sync::Mutex::new(oversized.clone()));
        let result = compact_oversized_workspace_spec_update_markdown(
            "model-1",
            Some(4_000),
            &oversized,
            |request| {
                attempts += 1;
                assert_eq!(
                    request.tools[0].name,
                    WORKSPACE_SPEC_UPDATE_COMPACTION_TOOL_NAME
                );
                assert!(
                    !request
                        .tools
                        .iter()
                        .any(|tool| tool.name == WORKSPACE_SPEC_TOOL_NAME)
                );
                let old_text = last_candidate.lock().expect("candidate lock").clone();
                let new_text = if attempts < 2 {
                    format!(
                        "HEAD\n{}",
                        "z".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 200)
                    )
                } else {
                    format!(
                        "HEAD\n{}",
                        "w".repeat(WORKSPACE_SPEC_TARGET_MARKDOWN_BYTES - 50)
                    )
                };
                *last_candidate.lock().expect("candidate lock") = new_text.clone();
                async move {
                    Ok(json!({
                        "edits": [{
                            "oldText": old_text,
                            "newText": new_text
                        }]
                    }))
                }
            },
        )
        .await
        .expect("patch compaction should succeed");

        assert_eq!(attempts, 2);
        assert!(result.len() <= WORKSPACE_SPEC_MAX_MARKDOWN_BYTES);
        assert!(result.starts_with("HEAD\n"));
    }

    #[tokio::test]
    async fn compact_oversized_workspace_spec_update_ignores_expansion_and_fails_after_max() {
        let oversized = format!(
            "EXPAND_MARKER\n{}",
            "x".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 1_000)
        );
        let mut attempts = 0_u32;
        let error = compact_oversized_workspace_spec_update_markdown(
            "model-1",
            None,
            &oversized,
            |_request| {
                attempts += 1;
                let old = oversized.clone();
                // Each attempt expands; helper must keep the original best candidate.
                let expanded = format!(
                    "EXPAND_MARKER\n{}",
                    "x".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 1_000 + attempts as usize * 10)
                );
                async move {
                    Ok(json!({
                        "edits": [{
                            "oldText": old,
                            "newText": expanded
                        }]
                    }))
                }
            },
        )
        .await
        .expect_err("expansions must not accept a worse candidate");

        assert_eq!(attempts, WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS);
        let message = error.message();
        assert!(message.contains("patch-compression attempts"));
        assert!(message.contains(&format!("initial {} bytes", oversized.len())));
    }

    #[tokio::test]
    async fn compact_oversized_workspace_spec_update_rejects_invalid_edits_without_accepting() {
        let oversized = "x".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 100);
        let mut attempts = 0_u32;
        let error = compact_oversized_workspace_spec_update_markdown(
            "model-1",
            None,
            &oversized,
            |_request| {
                attempts += 1;
                async move {
                    Ok(json!({
                        "edits": [{ "oldText": "missing-substring", "newText": "y" }]
                    }))
                }
            },
        )
        .await
        .expect_err("invalid patches should exhaust attempts without write");

        assert_eq!(attempts, WORKSPACE_SPEC_COMPACTION_MAX_ATTEMPTS);
        assert!(error.message().contains("patch-compression"));
    }

    #[test]
    fn parse_workspace_spec_update_compaction_output_applies_shared_patch_core() {
        let base = "# Project Spec\n\nToo long section.";
        let compacted = parse_workspace_spec_update_compaction_output(
            json!({
                "edits": [{
                    "oldText": "Too long section.",
                    "newText": "Short."
                }]
            }),
            base,
        )
        .expect("valid compaction edits");
        assert_eq!(compacted, "# Project Spec\n\nShort.");
        assert!(
            parse_workspace_spec_update_compaction_output(
                json!({ "contentMarkdown": "# Full" }),
                base
            )
            .is_err()
        );
        assert!(
            parse_workspace_spec_update_compaction_output(json!({ "edits": [] }), base).is_err()
        );
    }

    #[test]
    fn workspace_spec_update_prompt_appends_current_language_to_custom_prompt() {
        let settings = SpecSettings {
            update_system_prompt: Some(
                "Custom update prompt. Use the submit_workspace_spec_update tool exactly once."
                    .to_string(),
            ),
            ..SpecSettings::default()
        };
        let input = WorkspaceSpecUpdateInput {
            workspace_id: "workspace-1".to_string(),
            chat_id: "chat-1".to_string(),
            current_spec_revision: 1,
            user_message_id: "user-1".to_string(),
            assistant_message_id: "assistant-1".to_string(),
            run_id: "run-1".to_string(),
            code_change_stats: Some(CodeChangeStats::default()),
            chat_excerpt: WorkspaceSpecChatExcerptInput {
                user: "Update the scheduler.".to_string(),
                user_truncated: false,
                assistant: "Scheduler updated.".to_string(),
                assistant_truncated: false,
            },
            current_spec_markdown: "# Project Spec\n\nExisting spec.".to_string(),
        };

        let request = workspace_spec_update_provider_request(
            "model",
            "zh-CN",
            settings.update_system_prompt.as_deref(),
            1_024,
            &input,
        )
        .expect("workspace spec update request");
        let system_prompt = &request.messages[0].content;

        assert!(system_prompt.contains("Custom update prompt."));
        assert!(system_prompt.contains("current Foco app language setting (zh-CN)"));
        assert!(system_prompt.contains("write Project Spec prose in Simplified Chinese"));
    }

    #[test]
    fn parse_workspace_spec_update_output_applies_ordered_edits() {
        let base = "# Project Spec\n\nAlpha\nBeta\nGamma";
        let output = parse_workspace_spec_update_output(
            json!({
                "updateNeeded": true,
                "edits": [
                    {
                        "oldText": "Alpha",
                        "newText": "Alpha patched"
                    },
                    {
                        "oldText": "\nGamma",
                        "newText": ""
                    },
                    {
                        "oldText": "Beta",
                        "newText": "Beta then Delta"
                    }
                ]
            }),
            base,
        )
        .expect("patch parse");

        match output {
            WorkspaceSpecUpdateOutput::Patch {
                edits,
                content_markdown,
            } => {
                assert_eq!(edits.len(), 3);
                assert_eq!(
                    content_markdown,
                    "# Project Spec\n\nAlpha patched\nBeta then Delta"
                );
            }
            WorkspaceSpecUpdateOutput::NoUpdateNeeded => panic!("expected patch"),
        }
    }

    #[test]
    fn parse_workspace_spec_update_output_rejects_content_markdown_and_invalid_edits() {
        let base = "# Project Spec\n\nExisting spec. Existing.";
        assert!(
            parse_workspace_spec_update_output(
                json!({
                    "updateNeeded": true,
                    "contentMarkdown": "# Full"
                }),
                base
            )
            .is_err(),
            "full replacement payload must be rejected"
        );
        assert!(
            parse_workspace_spec_update_output(
                json!({
                    "updateNeeded": false,
                    "edits": [{ "oldText": "Existing", "newText": "x" }]
                }),
                base
            )
            .is_err(),
            "no-update must require edits=null"
        );
        assert!(
            parse_workspace_spec_update_output(
                json!({
                    "updateNeeded": true,
                    "edits": null
                }),
                base
            )
            .is_err(),
            "updateNeeded=true requires edits"
        );
        assert!(
            parse_workspace_spec_update_output(
                json!({
                    "updateNeeded": true,
                    "edits": []
                }),
                base
            )
            .is_err(),
            "empty edits must fail"
        );
        assert!(
            parse_workspace_spec_update_output(
                json!({
                    "updateNeeded": true,
                    "edits": [{ "oldText": "", "newText": "x" }]
                }),
                base
            )
            .is_err(),
            "empty oldText must fail"
        );
        assert!(
            parse_workspace_spec_update_output(
                json!({
                    "updateNeeded": true,
                    "edits": [{ "oldText": "Missing", "newText": "x" }]
                }),
                base
            )
            .is_err(),
            "missing match must fail"
        );
        assert!(
            parse_workspace_spec_update_output(
                json!({
                    "updateNeeded": true,
                    "edits": [{ "oldText": "Existing", "newText": "x" }]
                }),
                base
            )
            .is_err(),
            "ambiguous match must fail"
        );
        assert!(
            parse_workspace_spec_update_output(
                json!({
                    "updateNeeded": true,
                    "edits": [{ "oldText": "Existing spec.", "newText": "Existing spec." }]
                }),
                base
            )
            .is_err(),
            "no-op edits must fail"
        );
        assert!(
            parse_workspace_spec_update_output(
                json!({
                    "updateNeeded": true,
                    "edits": [{ "oldText": "Existing spec.", "newText": "Patched" }],
                    "contentMarkdown": "# Full"
                }),
                base
            )
            .is_err(),
            "unknown extra fields must fail"
        );
        assert_eq!(
            parse_workspace_spec_update_output(
                json!({
                    "updateNeeded": false,
                    "edits": null
                }),
                base
            )
            .expect("no update"),
            WorkspaceSpecUpdateOutput::NoUpdateNeeded
        );
    }
}
