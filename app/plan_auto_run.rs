use std::time::Duration;

use foco_store::{
    config::WorkspaceConfig,
    workspace::{PlanAutoRunSelection, PlanAutoRunStateRecord},
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

    for workspace in config.local_workspaces() {
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
    let selection = {
        let mut database = open_workspace_database(&workspace.path)?;
        let auto_run = database
            .plan_auto_run_state()
            .map_err(ApiError::from_workspace_error)?;
        if !auto_run.desired_enabled {
            return Ok(PlanAutoRunDispatch::Idle);
        }
        if !auto_run.enabled {
            return Ok(PlanAutoRunDispatch::Blocked);
        }
        if database
            .plan_auto_run_has_in_flight()
            .map_err(ApiError::from_workspace_error)?
        {
            return Ok(PlanAutoRunDispatch::Blocked);
        }
        let selection = database
            // Candidate selection must preserve the same earliest-incomplete
            // phase boundary enforced by Store start/resume and Retry paths.
            .next_plan_auto_run_candidate()
            .map_err(ApiError::from_workspace_error)?;
        match &selection {
            PlanAutoRunSelection::Idle => {
                database
                    .disable_plan_auto_run_if_idle()
                    .map_err(ApiError::from_workspace_error)?;
            }
            PlanAutoRunSelection::BlockedByCancelledPhase { plan_id, phase_id } => {
                database
                    .block_plan_auto_run("cancelled_phase", Some(plan_id), Some(phase_id))
                    .map_err(ApiError::from_workspace_error)?;
                tracing::info!(
                    workspace_id = %workspace.id,
                    plan_id,
                    phase_id,
                    "Plan auto-run paused at cancelled phase barrier"
                );
            }
            PlanAutoRunSelection::WaitingForReady { plan_id } => {
                tracing::debug!(
                    workspace_id = %workspace.id,
                    plan_id,
                    "Plan auto-run waiting for draft plan to become ready"
                );
            }
            PlanAutoRunSelection::WaitingForRetry { plan_id, phase_id } => {
                tracing::debug!(
                    workspace_id = %workspace.id,
                    plan_id,
                    phase_id,
                    "Plan auto-run waiting for explicit failed phase retry"
                );
            }
            PlanAutoRunSelection::Paused { plan_id, phase_id } => {
                tracing::debug!(
                    workspace_id = %workspace.id,
                    plan_id,
                    phase_id,
                    "Plan auto-run waiting for explicit user resume"
                );
            }
            PlanAutoRunSelection::Running { .. } | PlanAutoRunSelection::Candidate(_) => {}
        }
        selection
    };

    let candidate = match selection {
        PlanAutoRunSelection::Candidate(candidate) => candidate,
        PlanAutoRunSelection::WaitingForReady { .. }
        | PlanAutoRunSelection::WaitingForRetry { .. }
        | PlanAutoRunSelection::BlockedByCancelledPhase { .. }
        | PlanAutoRunSelection::Paused { .. }
        | PlanAutoRunSelection::Running { .. } => {
            return Ok(PlanAutoRunDispatch::Blocked);
        }
        PlanAutoRunSelection::Idle => return Ok(PlanAutoRunDispatch::Idle),
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
        Err(error) if error.status() == StatusCode::BAD_REQUEST => {
            let mut database = open_workspace_database(&workspace.path)?;
            let still_selected = matches!(
                database
                    .next_plan_auto_run_candidate()
                    .map_err(ApiError::from_workspace_error)?,
                PlanAutoRunSelection::Candidate(ref current)
                    if current.plan_id == candidate.plan_id && current.action == candidate.action
            );
            if !still_selected {
                tracing::info!(
                    workspace_id = %workspace.id,
                    plan_id = %candidate.plan_id,
                    status = %error.status(),
                    "Plan auto-run observed a durable state transition while dispatching"
                );
                return Ok(PlanAutoRunDispatch::Blocked);
            }
            let error_message = format!("Plan auto-run skipped invalid item: {}", error.message);
            match database.mark_plan_invalid(&candidate.plan_id, &error_message) {
                Ok(_) => {
                    tracing::warn!(
                        workspace_id = %workspace.id,
                        plan_id = %candidate.plan_id,
                        error = %error.message,
                        "Plan auto-run reconciled structurally invalid candidate as failed"
                    );
                    Ok(PlanAutoRunDispatch::Blocked)
                }
                Err(mark_error) => {
                    database
                        .block_plan_auto_run(
                            "scheduler_error",
                            Some(candidate.plan_id.as_str()),
                            None,
                        )
                        .map_err(ApiError::from_workspace_error)?;
                    tracing::warn!(
                        workspace_id = %workspace.id,
                        plan_id = %candidate.plan_id,
                        error = %error.message,
                        mark_error = %mark_error,
                        "Plan auto-run paused after invalid candidate could not be reconciled"
                    );
                    Err(error)
                }
            }
        }
        Err(error) if error.status() == StatusCode::CONFLICT => {
            tracing::info!(
                workspace_id = %workspace.id,
                plan_id = %candidate.plan_id,
                status = %error.status(),
                "Plan auto-run observed a concurrent Plan phase transition"
            );
            Ok(PlanAutoRunDispatch::Blocked)
        }
        Err(error) => {
            let mut database = open_workspace_database(&workspace.path)?;
            database
                .block_plan_auto_run("scheduler_error", Some(candidate.plan_id.as_str()), None)
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
) -> PlanAutoRunSelection {
    let Some(plan) = plans.iter().find(|plan| {
        matches!(
            plan.status.as_str(),
            "draft" | "ready" | "failed" | "paused" | "running"
        )
    }) else {
        return PlanAutoRunSelection::Idle;
    };
    let phase = plan.phases.iter().find(|phase| phase.status != "completed");
    let phase_id = phase.map(|phase| phase.id.clone());
    let phase_status = phase.map(|phase| phase.status.as_str());
    if phase_status == Some("cancelled") {
        return PlanAutoRunSelection::BlockedByCancelledPhase {
            plan_id: plan.id.clone(),
            phase_id: phase_id.expect("cancelled phase has an id"),
        };
    }
    if plan.status == "draft" {
        return PlanAutoRunSelection::WaitingForReady {
            plan_id: plan.id.clone(),
        };
    }
    if plan.status == "failed" || phase_status == Some("failed") {
        return PlanAutoRunSelection::WaitingForRetry {
            plan_id: plan.id.clone(),
            phase_id,
        };
    }
    if plan.status == "paused" {
        return PlanAutoRunSelection::Paused {
            plan_id: plan.id.clone(),
            phase_id,
        };
    }
    if plan.status == "running" || matches!(phase_status, Some("running" | "queued")) {
        return PlanAutoRunSelection::Running {
            plan_id: plan.id.clone(),
            phase_id,
        };
    }
    PlanAutoRunSelection::Candidate(foco_store::workspace::PlanAutoRunCandidateRecord {
        plan_id: plan.id.clone(),
        action: "start".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use foco_store::workspace::{PlanPhaseRecord, PlanRecord};

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

    fn phase(id: &str, status: &str, sequence: i64) -> PlanPhaseRecord {
        PlanPhaseRecord {
            id: id.to_string(),
            plan_id: "plan".to_string(),
            sequence,
            title: id.to_string(),
            summary: String::new(),
            status: status.to_string(),
            implementation_chat_id: None,
            agent_team_id: None,
            agent_task_id: None,
            commit_id: None,
            merge_attempt_count: 0,
            error_message: None,
            started_at: None,
            completed_at: None,
            created_at: String::new(),
            updated_at: String::new(),
            steps: Vec::new(),
            attempts: Vec::new(),
        }
    }

    fn candidate(
        selection: PlanAutoRunSelection,
    ) -> foco_store::workspace::PlanAutoRunCandidateRecord {
        match selection {
            PlanAutoRunSelection::Candidate(candidate) => candidate,
            other => panic!("expected candidate, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn scheduler_disables_auto_run_after_all_plans_finish() {
        let workspace = tempfile::tempdir().expect("workspace");
        let profile = tempfile::tempdir().expect("profile");
        let config = foco_store::config::GlobalConfig::first_run(workspace.path().to_path_buf());
        let state = crate::tests::test_app_state(config, profile.path().to_path_buf());
        {
            let mut database =
                foco_store::workspace::WorkspaceDatabase::open_or_create(workspace.path())
                    .expect("database");
            database
                .create_plan(foco_store::workspace::NewPlan {
                    id: "finished-plan",
                    title: "Finished plan",
                    overview: "Auto-run should stop after the queue drains.",
                    status: "completed",
                    source_chat_id: None,
                    phases: vec![foco_store::workspace::NewPlanPhase {
                        id: "finished-phase",
                        title: "Finished phase",
                        summary: "Done.",
                        steps: vec![foco_store::workspace::NewPlanStep {
                            id: "finished-step",
                            title: "Finished step",
                            detail: "Done.",
                            acceptance: vec!["done".to_string()],
                        }],
                    }],
                })
                .expect("create completed plan");
            database
                .set_plan_auto_run_enabled(true)
                .expect("enable auto-run");
        }

        assert!(!dispatch_plan_auto_run(&state).await.expect("dispatch scan"));

        let database = foco_store::workspace::WorkspaceDatabase::open_or_create(workspace.path())
            .expect("database");
        let auto_run = database.plan_auto_run_state().expect("auto-run state");
        assert!(!auto_run.desired_enabled);
        assert!(!auto_run.enabled);
        assert!(!auto_run.busy);
    }

    #[tokio::test]
    async fn scheduler_preserves_desired_auto_run_at_cancelled_phase_barrier_without_dispatch() {
        let workspace = tempfile::tempdir().expect("workspace");
        let profile = tempfile::tempdir().expect("profile");
        let config = foco_store::config::GlobalConfig::first_run(workspace.path().to_path_buf());
        let (agent_scheduler, mut agent_scheduler_rx) = AgentScheduler::new();
        let mut state = crate::tests::test_app_state(config.clone(), profile.path().to_path_buf());
        state.agent_scheduler = agent_scheduler;
        {
            let mut database =
                foco_store::workspace::WorkspaceDatabase::open_or_create(workspace.path())
                    .expect("database");
            database
                .create_plan(foco_store::workspace::NewPlan {
                    id: "blocked-plan",
                    title: "Blocked plan",
                    overview: "Cancelled phase blocks queue.",
                    status: "ready",
                    source_chat_id: None,
                    phases: vec![foco_store::workspace::NewPlanPhase {
                        id: "cancelled-phase",
                        title: "Cancelled phase",
                        summary: "Stop here.",
                        steps: vec![foco_store::workspace::NewPlanStep {
                            id: "cancelled-step",
                            title: "Cancelled step",
                            detail: "Do not dispatch.",
                            acceptance: vec!["blocked".to_string()],
                        }],
                    }],
                })
                .expect("create plan");
            database
                .transition_plan("blocked-plan", "start")
                .expect("start plan");
            database
                .cancel_plan_phase_by_id("blocked-plan", "cancelled-phase", "user cancelled")
                .expect("cancel phase");
            database
                .create_plan(foco_store::workspace::NewPlan {
                    id: "later-plan",
                    title: "Later plan",
                    overview: "Must not be skipped to.",
                    status: "ready",
                    source_chat_id: None,
                    phases: vec![foco_store::workspace::NewPlanPhase {
                        id: "later-phase",
                        title: "Later phase",
                        summary: "Wait.",
                        steps: vec![foco_store::workspace::NewPlanStep {
                            id: "later-step",
                            title: "Later step",
                            detail: "Do not dispatch.",
                            acceptance: vec!["pending".to_string()],
                        }],
                    }],
                })
                .expect("create later plan");
            database
                .set_plan_auto_run_enabled(true)
                .expect("enable auto-run");
        }

        assert!(!dispatch_plan_auto_run(&state).await.expect("dispatch scan"));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), agent_scheduler_rx.recv())
                .await
                .is_err()
        );
        let mut database =
            foco_store::workspace::WorkspaceDatabase::open_or_create(workspace.path())
                .expect("database");
        let auto_run = database.plan_auto_run_state().expect("auto-run state");
        assert!(auto_run.desired_enabled);
        assert!(!auto_run.enabled);
        assert!(!auto_run.busy);
        assert_eq!(auto_run.blocked_reason.as_deref(), Some("cancelled_phase"));
        database
            .set_plan_auto_run_enabled(false)
            .expect("disable desired auto-run");
        drop(database);

        assert!(
            !dispatch_plan_auto_run(&state)
                .await
                .expect("repeat dispatch scan")
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(50), agent_scheduler_rx.recv())
                .await
                .is_err()
        );
        let database = foco_store::workspace::WorkspaceDatabase::open_or_create(workspace.path())
            .expect("database");
        let auto_run = database.plan_auto_run_state().expect("auto-run state");
        assert!(!auto_run.desired_enabled);
        assert!(!auto_run.enabled);
        assert!(!auto_run.busy);
        let blocked = database
            .plan("blocked-plan")
            .expect("blocked plan lookup")
            .expect("blocked plan");
        let later = database
            .plan("later-plan")
            .expect("later plan lookup")
            .expect("later plan");
        assert_eq!(blocked.status, "paused");
        assert_eq!(blocked.phases[0].status, "cancelled");
        assert_eq!(later.status, "ready");
        assert_eq!(later.phases[0].status, "pending");
        assert!(later.phases[0].agent_task_id.is_none());
    }

    #[tokio::test]
    async fn scheduler_preserves_user_paused_plan_without_dispatch() {
        let workspace = tempfile::tempdir().expect("workspace");
        let profile = tempfile::tempdir().expect("profile");
        let config = foco_store::config::GlobalConfig::first_run(workspace.path().to_path_buf());
        let (agent_scheduler, mut agent_scheduler_rx) = AgentScheduler::new();
        let mut state = crate::tests::test_app_state(config, profile.path().to_path_buf());
        state.agent_scheduler = agent_scheduler;
        {
            let mut database =
                foco_store::workspace::WorkspaceDatabase::open_or_create(workspace.path())
                    .expect("database");
            database
                .create_plan(foco_store::workspace::NewPlan {
                    id: "paused-plan",
                    title: "Paused plan",
                    overview: "User pause must gate automatic phase dispatch.",
                    status: "ready",
                    source_chat_id: None,
                    phases: vec![foco_store::workspace::NewPlanPhase {
                        id: "paused-phase",
                        title: "Pending phase",
                        summary: "Wait for explicit Resume.",
                        steps: vec![foco_store::workspace::NewPlanStep {
                            id: "paused-step",
                            title: "Do not dispatch",
                            detail: "Only the user may resume this plan.",
                            acceptance: vec!["still pending".to_string()],
                        }],
                    }],
                })
                .expect("create plan");
            database
                .transition_plan("paused-plan", "pause")
                .expect("pause plan");
            database
                .set_plan_auto_run_enabled(true)
                .expect("enable auto-run");
        }

        assert!(!dispatch_plan_auto_run(&state).await.expect("dispatch scan"));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), agent_scheduler_rx.recv())
                .await
                .is_err()
        );

        let database = foco_store::workspace::WorkspaceDatabase::open_or_create(workspace.path())
            .expect("database");
        let plan = database
            .plan("paused-plan")
            .expect("paused plan lookup")
            .expect("paused plan");
        assert_eq!(plan.status, "paused");
        assert_eq!(plan.phases[0].status, "pending");
        assert!(plan.phases[0].agent_task_id.is_none());
        let auto_run = database.plan_auto_run_state().expect("auto-run state");
        assert!(auto_run.desired_enabled);
        assert!(auto_run.enabled);
        assert!(!auto_run.busy);
    }

    #[test]
    fn sidecar_store_action_preserves_cancelled_phase_barrier() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut database =
            foco_store::workspace::WorkspaceDatabase::open_or_create(workspace.path())
                .expect("database");
        database
            .create_plan(foco_store::workspace::NewPlan {
                id: "remote-barrier",
                title: "Remote barrier",
                overview: "Sidecar actions share store semantics.",
                status: "ready",
                source_chat_id: None,
                phases: vec![foco_store::workspace::NewPlanPhase {
                    id: "remote-cancelled-phase",
                    title: "Cancelled phase",
                    summary: "Retry is required.",
                    steps: vec![foco_store::workspace::NewPlanStep {
                        id: "remote-cancelled-step",
                        title: "Cancelled step",
                        detail: "Do not skip.",
                        acceptance: vec!["blocked".to_string()],
                    }],
                }],
            })
            .expect("create plan");
        database
            .transition_plan("remote-barrier", "start")
            .expect("start plan");
        database
            .cancel_plan_phase_by_id(
                "remote-barrier",
                "remote-cancelled-phase",
                "remote user cancelled",
            )
            .expect("cancel phase");

        let error = database
            .transition_plan("remote-barrier", "resume")
            .expect_err("sidecar store transition must require Retry");
        assert!(error.to_string().contains("Retry"), "{error}");
    }

    #[test]
    fn plan_auto_run_candidate_stops_at_running_queue_boundary() {
        let plans = vec![
            plan("running", "running"),
            plan("completed", "completed"),
            plan("paused", "paused"),
            plan("ready", "ready"),
        ];

        assert_eq!(
            choose_plan_auto_run_candidate(&plans),
            PlanAutoRunSelection::Running {
                plan_id: "running".to_string(),
                phase_id: None,
            }
        );
    }

    #[test]
    fn plan_auto_run_candidate_waits_for_draft_and_failed_but_starts_ready() {
        assert_eq!(
            choose_plan_auto_run_candidate(&[plan("draft", "draft")]),
            PlanAutoRunSelection::WaitingForReady {
                plan_id: "draft".to_string(),
            }
        );
        assert_eq!(
            choose_plan_auto_run_candidate(&[plan("failed", "failed")]),
            PlanAutoRunSelection::WaitingForRetry {
                plan_id: "failed".to_string(),
                phase_id: None,
            }
        );
        let candidate = candidate(choose_plan_auto_run_candidate(&[plan("ready", "ready")]));
        assert_eq!(candidate.plan_id, "ready");
        assert_eq!(candidate.action, "start");
    }

    #[test]
    fn plan_auto_run_candidate_reports_running_and_ignores_terminal_plans() {
        assert_eq!(
            choose_plan_auto_run_candidate(&[plan("running", "running")]),
            PlanAutoRunSelection::Running {
                plan_id: "running".to_string(),
                phase_id: None,
            }
        );
        assert_eq!(
            choose_plan_auto_run_candidate(&[
                plan("implemented", "implemented"),
                plan("completed", "completed"),
                plan("cancelled", "cancelled"),
            ]),
            PlanAutoRunSelection::Idle
        );
    }

    #[test]
    fn plan_auto_run_candidate_stops_at_cancelled_phase_barrier() {
        let mut blocked = plan("blocked", "paused");
        blocked.phases = vec![
            phase("completed-phase", "completed", 0),
            phase("cancelled-phase", "cancelled", 1),
            phase("pending-phase", "pending", 2),
        ];
        let plans = vec![blocked, plan("later-ready", "ready")];

        assert_eq!(
            choose_plan_auto_run_candidate(&plans),
            PlanAutoRunSelection::BlockedByCancelledPhase {
                plan_id: "blocked".to_string(),
                phase_id: "cancelled-phase".to_string(),
            }
        );
    }

    #[test]
    fn plan_auto_run_candidate_respects_pause_during_active_phase() {
        let mut paused = plan("paused", "paused");
        paused.phases = vec![
            phase("completed-phase", "completed", 0),
            phase("running-phase", "running", 1),
        ];

        assert_eq!(
            choose_plan_auto_run_candidate(&[paused]),
            PlanAutoRunSelection::Paused {
                plan_id: "paused".to_string(),
                phase_id: Some("running-phase".to_string()),
            }
        );
    }
}
