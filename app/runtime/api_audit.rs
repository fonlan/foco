use std::{path::Path, time::Duration};

use chrono::{Duration as ChronoDuration, SecondsFormat, Utc};
use foco_store::{
    config::GlobalConfig,
    workspace::{WorkspaceDatabase, WorkspaceDatabaseSpaceStats},
};
use tokio::sync::watch;

use crate::{ApiError, AppState};

const API_AUDIT_CLEANUP_INTERVAL_SECS: u64 = 6 * 60 * 60;
const API_AUDIT_VACUUM_MIN_FREE_BYTES: u64 = 256 * 1024 * 1024;
const API_AUDIT_VACUUM_MIN_FREE_RATIO_NUMERATOR: u64 = 1;
const API_AUDIT_VACUUM_MIN_FREE_RATIO_DENOMINATOR: u64 = 4;

pub(crate) fn spawn_api_audit_cleanup_scheduler(
    state: AppState,
    startup_delay: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut shutdown_rx = state.app_shutdown_rx.clone();
        if sleep_until_shutdown(&mut shutdown_rx, startup_delay).await {
            return;
        }

        loop {
            let config = match crate::config_snapshot(&state) {
                Ok(config) => config,
                Err(error) => {
                    tracing::warn!(error = %error.message, "API audit cleanup skipped");
                    if sleep_until_shutdown(
                        &mut shutdown_rx,
                        Duration::from_secs(API_AUDIT_CLEANUP_INTERVAL_SECS),
                    )
                    .await
                    {
                        return;
                    }
                    continue;
                }
            };
            run_api_audit_cleanup_in_background(config).await;
            if sleep_until_shutdown(
                &mut shutdown_rx,
                Duration::from_secs(API_AUDIT_CLEANUP_INTERVAL_SECS),
            )
            .await
            {
                return;
            }
        }
    })
}

pub(crate) fn spawn_api_audit_cleanup_once(state: AppState, config: GlobalConfig) {
    if *state.app_shutdown_rx.borrow() {
        return;
    }
    tokio::spawn(async move {
        run_api_audit_cleanup_in_background(config).await;
    });
}

async fn run_api_audit_cleanup_in_background(config: GlobalConfig) {
    match tokio::task::spawn_blocking(move || prune_api_audit_details_for_config(&config)).await {
        Ok(Ok(summary)) => {
            tracing::info!(
                pruned_count = summary.pruned_count,
                vacuumed_workspace_count = summary.vacuumed_workspace_count,
                vacuum_reclaimed_bytes = summary.vacuum_reclaimed_bytes,
                "API audit cleanup completed"
            );
        }
        Ok(Err(error)) => {
            tracing::warn!(error = %error.message, "API audit cleanup failed");
        }
        Err(error) => {
            tracing::warn!(error = %error, "API audit cleanup task failed");
        }
    }
}

#[derive(Default)]
struct ApiAuditCleanupSummary {
    pruned_count: i64,
    vacuumed_workspace_count: usize,
    vacuum_reclaimed_bytes: u64,
}

async fn sleep_until_shutdown(shutdown_rx: &mut watch::Receiver<bool>, duration: Duration) -> bool {
    if *shutdown_rx.borrow() {
        return true;
    }
    tokio::select! {
        _ = tokio::time::sleep(duration) => false,
        changed = shutdown_rx.changed() => changed.is_err() || *shutdown_rx.borrow(),
    }
}

fn prune_api_audit_details_for_config(
    config: &GlobalConfig,
) -> Result<ApiAuditCleanupSummary, ApiError> {
    let cutoff = api_audit_detail_cutoff(config);
    let mut summary = ApiAuditCleanupSummary::default();

    for workspace in &config.workspaces {
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        let pruned = database
            .prune_llm_request_details_before(&cutoff)
            .map_err(ApiError::from_workspace_error)?;
        summary.pruned_count = summary.pruned_count.saturating_add(pruned);
        if pruned > 0 {
            tracing::info!(
                workspace_id = %workspace.id,
                workspace_path = %workspace.path.display(),
                pruned,
                cutoff,
                "pruned API request details"
            );
        }
        match vacuum_workspace_database_if_needed(&mut database, &workspace.id, &workspace.path) {
            Ok(Some(reclaimed_bytes)) => {
                summary.vacuumed_workspace_count =
                    summary.vacuumed_workspace_count.saturating_add(1);
                summary.vacuum_reclaimed_bytes = summary
                    .vacuum_reclaimed_bytes
                    .saturating_add(reclaimed_bytes);
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    workspace_id = %workspace.id,
                    workspace_path = %workspace.path.display(),
                    error = %error.message,
                    "workspace database compaction skipped"
                );
            }
        }
    }

    Ok(summary)
}

fn vacuum_workspace_database_if_needed(
    database: &mut WorkspaceDatabase,
    workspace_id: &str,
    workspace_path: &Path,
) -> Result<Option<u64>, ApiError> {
    let before = database
        .space_stats()
        .map_err(ApiError::from_workspace_error)?;
    if !should_vacuum_workspace_database(before) {
        return Ok(None);
    }

    tracing::info!(
        workspace_id,
        workspace_path = %workspace_path.display(),
        database_path = %database.database_path().display(),
        file_bytes = before.file_bytes(),
        free_bytes = before.free_bytes(),
        freelist_count = before.freelist_count,
        page_count = before.page_count,
        "compacting workspace database"
    );
    database.vacuum().map_err(ApiError::from_workspace_error)?;
    let after = database
        .space_stats()
        .map_err(ApiError::from_workspace_error)?;
    let reclaimed_bytes = before.file_bytes().saturating_sub(after.file_bytes());
    tracing::info!(
        workspace_id,
        workspace_path = %workspace_path.display(),
        database_path = %database.database_path().display(),
        reclaimed_bytes,
        file_bytes_before = before.file_bytes(),
        file_bytes_after = after.file_bytes(),
        free_bytes_after = after.free_bytes(),
        "compacted workspace database"
    );

    Ok(Some(reclaimed_bytes))
}

pub(crate) fn should_vacuum_workspace_database(stats: WorkspaceDatabaseSpaceStats) -> bool {
    stats.page_count > 0
        && stats.free_bytes() >= API_AUDIT_VACUUM_MIN_FREE_BYTES
        && stats
            .freelist_count
            .saturating_mul(API_AUDIT_VACUUM_MIN_FREE_RATIO_DENOMINATOR)
            >= stats
                .page_count
                .saturating_mul(API_AUDIT_VACUUM_MIN_FREE_RATIO_NUMERATOR)
}

fn api_audit_detail_cutoff(config: &GlobalConfig) -> String {
    let now = Utc::now();
    let cutoff = if config.app.api_audit.save_request_response_details {
        now - ChronoDuration::days(i64::from(
            config.app.api_audit.request_detail_retention_days,
        ))
    } else {
        now
    };
    cutoff.to_rfc3339_opts(SecondsFormat::Millis, true)
}
