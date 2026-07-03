use std::time::Duration;

#[cfg(test)]
use foco_store::workspace::PlanAutoRunCandidateRecord;
use foco_store::{
    config::WorkspaceConfig,
    workspace::{PlanAutoRunStateRecord, PlanPatch},
};
use tokio::{sync::mpsc, task::JoinHandle, time};

use crate::*;

const PLAN_AUTO_RUN_WAKE_CAPACITY: usize = 1;
const PLAN_AUTO_RUN_MIN_SCAN_DELAY_MS: u64 = 1_000;
const PLAN_AUTO_RUN_IDLE_SCAN_INTERVAL_SECS: u64 = 300;
const PLAN_AUTO_RUN_SCAN_LIMIT: usize = 16;

#[derive(Clone)]
pub(crate) struct PlanAutoRunScheduler {
    wake_tx: mpsc::Sender<()>,
}

impl PlanAutoRunScheduler {
    pub(crate) fn new() -> (Self, mpsc::Receiver<()>) {
        let (wake_tx, wake_rx) = mpsc::channel(PLAN_AUTO_RUN_WAKE_CAPACITY);
        (Self { wake_tx }, wake_rx)
    }

    pub(crate) fn wake(&self) -> Result<(), ApiError> {
        match self.wake_tx.try_send(()) {
            Ok(()) | Err(mpsc::error::TrySendError::Full(())) => Ok(()),
            Err(mpsc::error::TrySendError::Closed(())) => Ok(()),
        }
    }

    pub(crate) fn spawn(&self, state: AppState, wake_rx: mpsc::Receiver<()>) -> JoinHandle<()> {
        tokio::spawn(run_plan_auto_run_scheduler(state, wake_rx))
    }
}

async fn run_plan_auto_run_scheduler(state: AppState, mut wake_rx: mpsc::Receiver<()>) {
    let mut shutdown_rx = state.app_shutdown_rx.clone();
    let mut scan = true;
    let mut scan_delay = Duration::from_secs(PLAN_AUTO_RUN_IDLE_SCAN_INTERVAL_SECS);

    loop {
        if scan {
            scan = false;
            match dispatch_plan_auto_run(&state).await {
                Ok(dispatched) => {
                    scan_delay = if dispatched {
                        Duration::from_millis(PLAN_AUTO_RUN_MIN_SCAN_DELAY_MS)
                    } else {
                        Duration::from_secs(PLAN_AUTO_RUN_IDLE_SCAN_INTERVAL_SECS)
                    };
                }
                Err(error) => {
                    tracing::error!(error = %error.message, "Plan auto-run scheduler scan failed");
                    scan_delay = Duration::from_secs(PLAN_AUTO_RUN_IDLE_SCAN_INTERVAL_SECS);
                }
            }
        }
        let delay = time::sleep(scan_delay);
        tokio::pin!(delay);

        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            wake = wake_rx.recv() => {
                if wake.is_none() {
                    break;
                }
                scan = true;
            }
            _ = &mut delay => {
                scan = true;
            }
        }
    }
}

pub(crate) async fn dispatch_plan_auto_run(state: &AppState) -> Result<bool, ApiError> {
    let config = config_snapshot(state)?;
    let mut dispatched_any = false;

    for workspace in &config.workspaces {
        for _ in 0..PLAN_AUTO_RUN_SCAN_LIMIT {
            match dispatch_next_plan_auto_run(state, workspace).await? {
                PlanAutoRunDispatch::Dispatched => dispatched_any = true,
                PlanAutoRunDispatch::Idle | PlanAutoRunDispatch::Blocked => break,
            }
        }
    }

    Ok(dispatched_any)
}

enum PlanAutoRunDispatch {
    Dispatched,
    Idle,
    Blocked,
}

async fn dispatch_next_plan_auto_run(
    state: &AppState,
    workspace: &WorkspaceConfig,
) -> Result<PlanAutoRunDispatch, ApiError> {
    let candidate = {
        let mut database = open_workspace_database(&workspace.path)?;
        if !database
            .plan_auto_run_state()
            .map_err(ApiError::from_workspace_error)?
            .enabled
        {
            return Ok(PlanAutoRunDispatch::Idle);
        }
        if database
            .plan_auto_run_has_in_flight()
            .map_err(ApiError::from_workspace_error)?
        {
            return Ok(PlanAutoRunDispatch::Blocked);
        }
        let candidate = database
            .next_plan_auto_run_candidate()
            .map_err(ApiError::from_workspace_error)?;
        if candidate.is_none() {
            database
                .disable_plan_auto_run_if_idle()
                .map_err(ApiError::from_workspace_error)?;
        }
        candidate
    };

    let Some(candidate) = candidate else {
        return Ok(PlanAutoRunDispatch::Idle);
    };

    match crate::plan_runtime::transition_plan_action(
        state,
        &workspace.id,
        &candidate.plan_id,
        &candidate.action,
    )
    .await
    {
        Ok(_) => Ok(PlanAutoRunDispatch::Dispatched),
        Err(error) if error.message.starts_with("invalid plan:") => {
            let mut database = open_workspace_database(&workspace.path)?;
            let error_message = format!("Plan auto-run skipped invalid item: {}", error.message);
            match database.update_plan(
                &candidate.plan_id,
                PlanPatch {
                    title: None,
                    overview: None,
                    status: Some("failed"),
                    error_message: Some(Some(&error_message)),
                },
            ) {
                Ok(_) => {
                    tracing::warn!(
                        workspace_id = %workspace.id,
                        plan_id = %candidate.plan_id,
                        error = %error.message,
                        "Plan auto-run marked invalid candidate failed"
                    );
                    Ok(PlanAutoRunDispatch::Blocked)
                }
                Err(mark_error) => {
                    database
                        .set_plan_auto_run_enabled(false)
                        .map_err(ApiError::from_workspace_error)?;
                    tracing::warn!(
                        workspace_id = %workspace.id,
                        plan_id = %candidate.plan_id,
                        error = %error.message,
                        mark_error = %mark_error,
                        "Plan auto-run disabled after invalid candidate could not be marked failed"
                    );
                    Err(error)
                }
            }
        }
        Err(error) => {
            let mut database = open_workspace_database(&workspace.path)?;
            database
                .set_plan_auto_run_enabled(false)
                .map_err(ApiError::from_workspace_error)?;
            Err(error)
        }
    }
}

pub(crate) fn plan_auto_run_state(
    workspace: &WorkspaceConfig,
) -> Result<PlanAutoRunStateRecord, ApiError> {
    let database = open_workspace_database(&workspace.path)?;
    database
        .plan_auto_run_state()
        .map_err(ApiError::from_workspace_error)
}

pub(crate) fn set_plan_auto_run_enabled(
    workspace: &WorkspaceConfig,
    enabled: bool,
) -> Result<PlanAutoRunStateRecord, ApiError> {
    let mut database = open_workspace_database(&workspace.path)?;
    database
        .set_plan_auto_run_enabled(enabled)
        .map_err(ApiError::from_workspace_error)
}

#[cfg(test)]
pub(crate) fn choose_plan_auto_run_candidate(
    plans: &[foco_store::workspace::PlanRecord],
) -> Option<PlanAutoRunCandidateRecord> {
    plans
        .iter()
        .filter_map(|plan| match plan.status.as_str() {
            "draft" | "ready" | "failed" => Some(PlanAutoRunCandidateRecord {
                plan_id: plan.id.clone(),
                action: "start".to_string(),
            }),
            "paused" => Some(PlanAutoRunCandidateRecord {
                plan_id: plan.id.clone(),
                action: "resume".to_string(),
            }),
            _ => None,
        })
        .next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use foco_store::workspace::PlanRecord;

    fn plan(id: &str, status: &str) -> PlanRecord {
        PlanRecord {
            id: id.to_string(),
            title: id.to_string(),
            overview: String::new(),
            status: status.to_string(),
            sort_order: 0,
            source_chat_id: None,
            active_phase_id: None,
            pause_requested_at: None,
            completed_at: None,
            completed_by_user_at: None,
            error_message: None,
            shared_merge_commit_id: None,
            created_at: String::new(),
            updated_at: String::new(),
            phases: Vec::new(),
        }
    }

    #[test]
    fn plan_auto_run_candidate_matches_frontend_order() {
        let plans = vec![
            plan("running", "running"),
            plan("completed", "completed"),
            plan("paused", "paused"),
            plan("ready", "ready"),
        ];

        let candidate = choose_plan_auto_run_candidate(&plans).expect("candidate");
        assert_eq!(candidate.plan_id, "paused");
        assert_eq!(candidate.action, "resume");
    }

    #[test]
    fn plan_auto_run_candidate_starts_draft_ready_and_failed() {
        for status in ["draft", "ready", "failed"] {
            let candidate =
                choose_plan_auto_run_candidate(&[plan(status, status)]).expect("candidate");
            assert_eq!(candidate.plan_id, status);
            assert_eq!(candidate.action, "start");
        }
    }

    #[test]
    fn plan_auto_run_candidate_ignores_running_and_terminal_plans() {
        let plans = vec![
            plan("running", "running"),
            plan("implemented", "implemented"),
            plan("completed", "completed"),
            plan("cancelled", "cancelled"),
        ];

        assert!(choose_plan_auto_run_candidate(&plans).is_none());
    }
}
