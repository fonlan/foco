use std::time::Duration;

use axum::http::StatusCode;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use foco_store::{
    config::{GlobalConfig, WorkspaceConfig},
    memory::{
        MemoryDatabase, MemoryDreamJobStatus, MemoryDreamRunMode, MemoryDreamScope,
        MemoryDreamTriggerType,
    },
    workspace::{WorkspaceDatabase, workspace_database_path},
};
use tokio::{task::JoinHandle, time};

use crate::memory_runtime::dream::{
    MemoryDreamJobRequest, MemoryDreamJobResult, MemoryDreamPlannerRequest,
    MemoryDreamTranscriptRequest, run_memory_dream_job,
};
use crate::*;

const MEMORY_DREAM_SCHEDULER_DEFAULT_SCAN_MINUTES: u32 = 60;

#[derive(Clone, Default)]
pub(crate) struct MemoryDreamScheduler;

impl MemoryDreamScheduler {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn spawn(&self, state: AppState) -> JoinHandle<()> {
        tokio::spawn(run_memory_dream_scheduler(state))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MemoryDreamSchedulerScan {
    pub(crate) next_scan_minutes: u32,
    pub(crate) runs_started: usize,
    pub(crate) skipped_active: usize,
}

async fn run_memory_dream_scheduler(state: AppState) {
    let mut shutdown_rx = state.app_shutdown_rx.clone();

    loop {
        let scan_minutes = match dispatch_auto_memory_dreams_at(&state, Utc::now()).await {
            Ok(scan) => scan.next_scan_minutes,
            Err(error) => {
                tracing::error!(error = %error.message, "Memory Dream scheduler scan failed");
                MEMORY_DREAM_SCHEDULER_DEFAULT_SCAN_MINUTES
            }
        };
        let delay = time::sleep(Duration::from_secs(u64::from(scan_minutes.max(1)) * 60));
        tokio::pin!(delay);

        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            _ = &mut delay => {}
        }
    }
}

pub(crate) async fn dispatch_auto_memory_dreams_at(
    state: &AppState,
    now: DateTime<Utc>,
) -> Result<MemoryDreamSchedulerScan, ApiError> {
    let config = config_snapshot(state)?;
    let mut scan = MemoryDreamSchedulerScan {
        next_scan_minutes: config.memory.dream.scheduler_scan_minutes.max(1),
        runs_started: 0,
        skipped_active: 0,
    };

    if !config.memory.enabled || !config.memory.dream.enabled || !config.memory.dream.auto_enabled {
        return Ok(scan);
    }

    let mode = MemoryDreamRunMode::parse(config.memory.dream.mode.trim())
        .map_err(ApiError::from_memory_error)?;
    run_auto_memory_dream_if_due(
        state,
        &config,
        MemoryDreamScope::Global,
        None,
        mode,
        now,
        &mut scan,
    )
    .await;

    // ponytail: sequential scans avoid backlog fan-out; parallelize only if many workspaces make this slow.
    for workspace in &config.workspaces {
        run_auto_memory_dream_if_due(
            state,
            &config,
            MemoryDreamScope::Workspace,
            Some(workspace),
            mode,
            now,
            &mut scan,
        )
        .await;
    }

    Ok(scan)
}

async fn run_auto_memory_dream_if_due(
    state: &AppState,
    config: &GlobalConfig,
    scope: MemoryDreamScope,
    workspace: Option<&WorkspaceConfig>,
    mode: MemoryDreamRunMode,
    now: DateTime<Utc>,
    scan: &mut MemoryDreamSchedulerScan,
) {
    let workspace_id = workspace.map(|workspace| workspace.id.as_str());
    let database = match open_dream_memory_database(state, config, scope, workspace_id) {
        Ok(database) => database,
        Err(error) => {
            tracing::warn!(
                scope = scope.as_str(),
                workspace_id,
                error = %error.message,
                "Memory Dream scheduler skipped scope because the database could not open"
            );
            return;
        }
    };
    let (interval_days, threshold_facts) = match scope {
        MemoryDreamScope::Global => (
            config.memory.dream.global_interval_days,
            config.memory.dream.global_threshold_facts,
        ),
        MemoryDreamScope::Workspace => (
            config.memory.dream.workspace_interval_days,
            config.memory.dream.workspace_threshold_facts,
        ),
    };
    let trigger = match memory_dream_auto_trigger(
        &database,
        scope,
        workspace_id,
        interval_days,
        threshold_facts,
        now,
    ) {
        Ok(trigger) => trigger,
        Err(error) => {
            tracing::error!(
                scope = scope.as_str(),
                workspace_id,
                error = %error.message,
                "Memory Dream scheduler eligibility check failed"
            );
            return;
        }
    };
    drop(database);
    let Some(trigger) = trigger else {
        return;
    };

    match run_memory_dream_for_state(state, config, scope, workspace_id, trigger, mode).await {
        Ok(result) => {
            scan.runs_started += 1;
            tracing::info!(
                scope = scope.as_str(),
                workspace_id,
                trigger = trigger.as_str(),
                job_id = %result.job.id,
                applied_changes = result.applied_changes,
                failed_changes = result.failed_changes,
                "Auto memory Dream completed"
            );
        }
        Err(error) if error.status == StatusCode::CONFLICT => {
            scan.skipped_active += 1;
            tracing::info!(
                scope = scope.as_str(),
                workspace_id,
                trigger = trigger.as_str(),
                "Auto memory Dream skipped because another run is active"
            );
        }
        Err(error) => {
            tracing::error!(
                scope = scope.as_str(),
                workspace_id,
                trigger = trigger.as_str(),
                error = %error.message,
                "Auto memory Dream failed"
            );
        }
    }
}

pub(crate) fn memory_dream_auto_trigger(
    database: &MemoryDatabase,
    scope: MemoryDreamScope,
    workspace_id: Option<&str>,
    interval_days: u32,
    threshold_facts: u32,
    now: DateTime<Utc>,
) -> Result<Option<MemoryDreamTriggerType>, ApiError> {
    if memory_dream_has_active_job(database, scope, workspace_id)? {
        return Ok(None);
    }

    let latest_success_at = database
        .latest_successful_dream_time(scope, workspace_id)
        .map_err(ApiError::from_memory_error)?;
    let changed_facts = database
        .dream_updated_fact_count_since(scope, workspace_id, latest_success_at.as_deref())
        .map_err(ApiError::from_memory_error)?;
    if changed_facts >= threshold_facts {
        return Ok(Some(MemoryDreamTriggerType::AutoThreshold));
    }
    if memory_dream_interval_due(latest_success_at.as_deref(), interval_days, now)? {
        return Ok(Some(MemoryDreamTriggerType::AutoInterval));
    }

    Ok(None)
}

pub(crate) fn memory_dream_interval_due(
    latest_success_at: Option<&str>,
    interval_days: u32,
    now: DateTime<Utc>,
) -> Result<bool, ApiError> {
    let Some(latest_success_at) = latest_success_at else {
        return Ok(true);
    };
    let latest_success_at = DateTime::parse_from_rfc3339(latest_success_at)
        .map_err(|source| {
            ApiError::internal(format!(
                "memory Dream completed_at timestamp is invalid: {source}"
            ))
        })?
        .with_timezone(&Utc);

    Ok(latest_success_at + ChronoDuration::days(i64::from(interval_days)) <= now)
}

pub(crate) async fn run_memory_dream_for_state(
    state: &AppState,
    config: &GlobalConfig,
    scope: MemoryDreamScope,
    workspace_id: Option<&str>,
    trigger_type: MemoryDreamTriggerType,
    mode: MemoryDreamRunMode,
) -> Result<MemoryDreamJobResult, ApiError> {
    let active_key = memory_dream_active_key(scope, workspace_id);
    {
        let mut active_runs = state.memory_dream_runs.lock().await;
        if !active_runs.insert(active_key.clone()) {
            return Err(ApiError::conflict("memory Dream is already running"));
        }
    }

    let result =
        run_memory_dream_guarded(state, config, scope, workspace_id, trigger_type, mode).await;
    state.memory_dream_runs.lock().await.remove(&active_key);
    result
}

async fn run_memory_dream_guarded(
    state: &AppState,
    config: &GlobalConfig,
    scope: MemoryDreamScope,
    workspace_id: Option<&str>,
    trigger_type: MemoryDreamTriggerType,
    mode: MemoryDreamRunMode,
) -> Result<MemoryDreamJobResult, ApiError> {
    let mut database = open_dream_memory_database(state, config, scope, workspace_id)?;
    ensure_no_active_memory_dream_run(&database, scope, workspace_id)?;
    let needs_runtime_workspace =
        mode == MemoryDreamRunMode::Llm || config.memory.dream.create_transcript_chat;
    let runtime_workspace =
        memory_dream_runtime_workspace(config, scope, workspace_id, needs_runtime_workspace)?;
    let planner = if mode == MemoryDreamRunMode::Llm {
        let workspace = runtime_workspace.ok_or_else(|| {
            ApiError::bad_request("LLM memory Dream requires a workspace for audit logging")
        })?;
        Some(MemoryDreamPlannerRequest {
            config,
            workspace_path: &workspace.path,
            audit_workspace_id: &workspace.id,
            audit_chat_id: None,
            chat_model_id: None,
        })
    } else {
        None
    };
    let transcript = if config.memory.dream.create_transcript_chat {
        let workspace = runtime_workspace.ok_or_else(|| {
            ApiError::bad_request("memory Dream transcript requires at least one workspace")
        })?;
        Some(MemoryDreamTranscriptRequest {
            workspace_path: &workspace.path,
        })
    } else {
        None
    };

    run_memory_dream_job(
        &mut database,
        MemoryDreamJobRequest {
            scope,
            workspace_id,
            trigger_type,
            mode,
            model_id: config.memory.dream.model_id.as_deref(),
            settings: &config.memory.dream,
            global_memory_database_file: Some(&state.memory_database_file),
            planner,
            transcript,
        },
    )
    .await
}

fn ensure_no_active_memory_dream_run(
    database: &MemoryDatabase,
    scope: MemoryDreamScope,
    workspace_id: Option<&str>,
) -> Result<(), ApiError> {
    if memory_dream_has_active_job(database, scope, workspace_id)? {
        return Err(ApiError::conflict("memory Dream is already active"));
    }

    Ok(())
}

fn memory_dream_has_active_job(
    database: &MemoryDatabase,
    scope: MemoryDreamScope,
    workspace_id: Option<&str>,
) -> Result<bool, ApiError> {
    for status in [MemoryDreamJobStatus::Queued, MemoryDreamJobStatus::Running] {
        let active = database
            .dream_jobs_for_scope(scope, workspace_id, Some(status), 1)
            .map_err(ApiError::from_memory_error)?;
        if !active.is_empty() {
            return Ok(true);
        }
    }

    Ok(false)
}

fn open_dream_memory_database(
    state: &AppState,
    config: &GlobalConfig,
    scope: MemoryDreamScope,
    workspace_id: Option<&str>,
) -> Result<MemoryDatabase, ApiError> {
    match scope {
        MemoryDreamScope::Global => {
            MemoryDatabase::open_or_create_global_at(&state.memory_database_file)
                .map_err(ApiError::from_memory_error)
        }
        MemoryDreamScope::Workspace => {
            let workspace_id = workspace_id.ok_or_else(|| {
                ApiError::bad_request("workspace memory Dream requires workspaceId")
            })?;
            let workspace = workspace_by_id(config, workspace_id)?;
            WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace.path))
                .map_err(ApiError::from_memory_error)
        }
    }
}

fn memory_dream_runtime_workspace<'a>(
    config: &'a GlobalConfig,
    scope: MemoryDreamScope,
    workspace_id: Option<&str>,
    required: bool,
) -> Result<Option<&'a WorkspaceConfig>, ApiError> {
    match scope {
        MemoryDreamScope::Workspace => workspace_id
            .map(|workspace_id| workspace_by_id(config, workspace_id).map(Some))
            .unwrap_or_else(|| {
                Err(ApiError::bad_request(
                    "workspace memory Dream requires workspaceId",
                ))
            }),
        MemoryDreamScope::Global if required => {
            config.workspaces.first().map(Some).ok_or_else(|| {
                ApiError::bad_request("global memory Dream requires at least one workspace")
            })
        }
        MemoryDreamScope::Global => Ok(None),
    }
}

fn memory_dream_active_key(scope: MemoryDreamScope, workspace_id: Option<&str>) -> String {
    match scope {
        MemoryDreamScope::Global => "global".to_string(),
        MemoryDreamScope::Workspace => {
            format!("workspace:{}", workspace_id.unwrap_or_default())
        }
    }
}
