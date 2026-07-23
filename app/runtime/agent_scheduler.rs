use std::{
    any::Any,
    collections::{HashMap, HashSet},
    future::Future,
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use foco_agent::{
    AgentAttemptId, AgentCollaborationTool, AgentExecutionWorkspaceMode, AgentInstanceStatus,
    AgentPermissions, AgentRole, AgentRunAssociations, AgentRunOutcome, AgentTaskId,
    AgentTaskStatus, AgentTaskTransition, ToolPromptInfo, build_available_tools_prompt,
    build_subagents_prompt_section, estimate_text_tokens,
};
use foco_providers::{NeutralChatMessage, NeutralChatRole, NeutralToolCall, NeutralToolDefinition};
use foco_store::{
    config::{AGENT_DEFINITION_SYSTEM_PROMPT_MAX_CHARS, AgentDefinitionSettings},
    workspace::{
        AgentAttemptRecord, AgentAttemptRecoveryDisposition, AgentContextEntryRecord,
        AgentInstanceRecord, AgentMessageRecord, AgentTaskDependencyRecord, AgentTaskRecord,
        AgentTaskStateUpdate, AgentTeamRecord, NewAgentContextEntry, NewAgentContextSnapshot,
        NewAgentEvent, PreStreamChatFailureClosure, PreStreamChatFailureClosureResult,
        WorkspaceDatabase,
    },
};
use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    sync::{Semaphore, mpsc, watch},
    task::{Id as TokioTaskId, JoinHandle, JoinSet},
    time,
};

use super::{
    ActiveAgentRunIdentity, ActiveChatRunRegistrationResult,
    spawn_code_graph_execution_root_initialization_if_needed,
};
use crate::git_backend::{
    agent_instance_worktree_path, agent_worktree_diff_id, git_diff_response,
    resolve_agent_worktree_path,
};
use crate::*;

// ponytail: fixed first-slice limits avoid new config surface; make them configurable when
// production workload data shows a different ceiling is needed.
pub(crate) const AGENT_MAX_QUEUED_TASKS_PER_TEAM: i64 = 64;
pub(crate) const AGENT_MAX_QUEUED_TASKS_PER_INSTANCE: i64 = 64;
pub(crate) const AGENT_MAX_QUEUED_TASKS_PER_CHAT: i64 = 64;
pub(crate) const AGENT_MAX_INSTANCES_PER_TEAM: i64 = 10;
pub(crate) const AGENT_MAX_CREATE_INSTANCES_PER_REQUEST: u32 = 16;
const AGENT_SCHEDULER_WAKE_CAPACITY: usize = 1;
const AGENT_SCHEDULER_SCAN_LIMIT: i64 = 64;
const AGENT_SCHEDULER_MIN_DEADLINE_DELAY_MS: u64 = 1_000;
const AGENT_SCHEDULER_ERROR_RETRY_SECS: i64 = 30;
const AGENT_ATTEMPT_LEASE_TIMEOUT: Duration = Duration::from_secs(30);
const AGENT_ATTEMPT_LEASE_HEARTBEAT: Duration = Duration::from_secs(5);
const AGENT_GLOBAL_MAX_CONCURRENT_RUNS: usize = 10;
const RESTART_INTERRUPTION_REASON: &str = "backend restarted while Agent attempt was active";
const AGENT_TEAM_PROTOCOL_VERSION: u32 = 2;
const AGENT_CONTEXT_SNAPSHOT_VERSION: u32 = 1;
const AGENT_CONTEXT_RECENT_MESSAGE_LIMIT: usize = 8;
const AGENT_CONTEXT_SUMMARY_ENTRY_LIMIT: usize = 16;
const AGENT_CONTEXT_SUMMARY_MAX_CHARS: usize = 320;
const AGENT_MAX_TASK_OUTCOME_BYTES: usize = 64 * 1024;
const AGENT_TASK_DB_SHORT_RETRY_ATTEMPTS: usize = 4;
const AGENT_TASK_DB_SHORT_RETRY_DELAY: Duration = Duration::from_millis(500);
#[cfg(not(test))]
const AGENT_LIFECYCLE_DB_RETRY_INITIAL_DELAY: Duration = Duration::from_millis(500);
#[cfg(test)]
const AGENT_LIFECYCLE_DB_RETRY_INITIAL_DELAY: Duration = Duration::from_millis(1);
#[cfg(not(test))]
const AGENT_LIFECYCLE_DB_RETRY_MAX_DELAY: Duration = Duration::from_secs(5);
#[cfg(test)]
const AGENT_LIFECYCLE_DB_RETRY_MAX_DELAY: Duration = Duration::from_millis(2);
const AGENT_LIFECYCLE_DB_RETRY_WARNING_INTERVAL: Duration = Duration::from_secs(30);
const AGENT_LIFECYCLE_DB_RETRY_ERROR_AFTER: Duration = Duration::from_secs(120);
const AGENT_LIFECYCLE_DB_RETRY_ERROR_INTERVAL: Duration = Duration::from_secs(120);
const AGENT_CURRENT_TASK_MESSAGE_PREVIEW_CHARS: usize = 4 * 1024;
const AGENT_WAIT_RESUME_INSTRUCTION: &str = "## Agent Wait Resume\n\nSource: Foco Agent wait resume\n\nThe following agent_wait_tasks tool result contains completed child task results. Continue the current parent task from this result, synthesize the child output as needed, and do not treat a child task's final text as the main chat reply by itself.";
// Bounded pre-stream ordinary-DB wait (~10–15s total). Not the infinite lifecycle retry.
#[cfg(not(test))]
const PRE_STREAM_DB_RETRY_BUDGET: Duration = Duration::from_secs(12);
#[cfg(test)]
const PRE_STREAM_DB_RETRY_BUDGET: Duration = Duration::from_millis(40);
#[cfg(not(test))]
const PRE_STREAM_DB_RETRY_INITIAL_DELAY: Duration = Duration::from_millis(200);
#[cfg(test)]
const PRE_STREAM_DB_RETRY_INITIAL_DELAY: Duration = Duration::from_millis(1);
#[cfg(not(test))]
const PRE_STREAM_DB_RETRY_MAX_DELAY: Duration = Duration::from_millis(1_500);
#[cfg(test)]
const PRE_STREAM_DB_RETRY_MAX_DELAY: Duration = Duration::from_millis(4);
const PRE_STREAM_FAILURE_CODE_WORKSPACE_DATABASE_BUSY: &str = "workspace_database_busy";
const PRE_STREAM_FAILURE_STAGE_PREPARE: &str = "pre_stream_prepare";
const PRE_STREAM_USER_MESSAGE_DATABASE_BUSY: &str =
    "Reply has not started: workspace database is busy. Please retry.";
const PRE_STREAM_USER_MESSAGE_GENERIC: &str =
    "Reply has not started: preparation failed. Please retry.";

#[derive(Clone)]
pub(crate) struct AgentScheduler {
    wake_tx: mpsc::Sender<()>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CoordinatorTaskInput {
    pub(crate) queued_user_message_id: String,
    #[serde(default)]
    pub(crate) visible_assistant_message_id: Option<String>,
    #[serde(default)]
    pub(crate) visible_assistant_sequence: Option<i64>,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) attachments: Vec<ChatAttachmentInput>,
    #[serde(default, skip_serializing)]
    pub(crate) skill_ids: Vec<String>,
    #[serde(default)]
    pub(crate) session_mode: Option<String>,
    #[serde(default)]
    pub(crate) latency_mode: foco_providers::LatencyMode,
    #[serde(default = "default_collaboration_tools_enabled")]
    pub(crate) collaboration_tools_enabled: bool,
    #[serde(default)]
    pub(crate) defer_until_workspace_idle: bool,
    #[serde(default)]
    pub(crate) delegated_input: Option<Value>,
    #[serde(default)]
    pub(crate) correlation_id: Option<String>,
}

struct AgentTaskModelSelection {
    model_id: String,
    thinking_level: Option<String>,
}

fn default_collaboration_tools_enabled() -> bool {
    true
}

impl AgentScheduler {
    pub(crate) fn new() -> (Self, mpsc::Receiver<()>) {
        let (wake_tx, wake_rx) = mpsc::channel(AGENT_SCHEDULER_WAKE_CAPACITY);
        (Self { wake_tx }, wake_rx)
    }

    pub(crate) fn wake(&self) -> Result<(), ApiError> {
        match self.wake_tx.try_send(()) {
            Ok(()) | Err(mpsc::error::TrySendError::Full(())) => Ok(()),
            Err(mpsc::error::TrySendError::Closed(())) => {
                Err(ApiError::internal("Agent scheduler is not running"))
            }
        }
    }

    pub(crate) fn spawn(&self, state: AppState, wake_rx: mpsc::Receiver<()>) -> JoinHandle<()> {
        tokio::spawn(run_agent_scheduler(state, wake_rx))
    }
}

async fn run_agent_scheduler(state: AppState, mut wake_rx: mpsc::Receiver<()>) {
    if let Err(error) = reconcile_agent_runtime(&state) {
        tracing::error!(error = %error.message, "Agent scheduler startup reconciliation failed");
    }

    let permits = Arc::new(Semaphore::new(AGENT_GLOBAL_MAX_CONCURRENT_RUNS));
    let mut runs = JoinSet::new();
    let mut run_identities = HashMap::new();
    let mut shutdown_rx = state.app_shutdown_rx.clone();
    let owner_incarnation = unique_id("agent-owner");
    let mut scan = true;
    let mut next_deadline_at: Option<DateTime<Utc>> = None;

    loop {
        if scan {
            scan = false;
            if let Err(error) = reconcile_agent_attempt_leases(&state) {
                tracing::error!(
                    error = %error.message,
                    "Agent scheduler lease reconciliation failed"
                );
            }
            match schedule_runnable_tasks(
                &state,
                &permits,
                &mut runs,
                &mut run_identities,
                &owner_incarnation,
            )
            .await
            {
                Ok(result) => next_deadline_at = result.next_deadline_at,
                Err(error) => {
                    next_deadline_at = Some(
                        Utc::now() + chrono::Duration::seconds(AGENT_SCHEDULER_ERROR_RETRY_SECS),
                    );
                    tracing::error!(error = %error.message, "Agent scheduler scan failed");
                }
            }
        }
        let deadline_sleep = time::sleep(agent_scheduler_deadline_delay(next_deadline_at.as_ref()));
        tokio::pin!(deadline_sleep);

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
            completed = runs.join_next_with_id(), if !runs.is_empty() => {
                if let Some(result) = completed {
                    handle_agent_run_join_result(&state, result, &mut run_identities).await;
                }
                scan = true;
            }
            _ = &mut deadline_sleep => {
                scan = true;
            }
        }
    }

    while let Some(result) = runs.join_next_with_id().await {
        handle_agent_run_join_result(&state, result, &mut run_identities).await;
    }
}

async fn handle_agent_run_join_result(
    state: &AppState,
    result: Result<(TokioTaskId, AgentCoordinatorRunCompletion), tokio::task::JoinError>,
    run_identities: &mut HashMap<TokioTaskId, AgentCoordinatorRunIdentity>,
) {
    match result {
        Ok((run_id, completion)) => {
            run_identities.remove(&run_id);
            if let AgentCoordinatorRunExit::Panicked(panic_message) = completion.exit {
                let reason = format!("Coordinator task panicked: {panic_message}");
                recover_abnormal_coordinator_exit(state, &completion.identity, &reason).await;
            }
        }
        Err(error) => {
            let run_id = error.id();
            let Some(identity) = run_identities.remove(&run_id) else {
                tracing::error!(error = %error, "Agent scheduler run exited without tracked identity");
                return;
            };
            let reason = if error.is_cancelled() {
                format!("Coordinator task future was cancelled: {error}")
            } else {
                format!("Coordinator task future exited abnormally: {error}")
            };
            recover_abnormal_coordinator_exit(state, &identity, &reason).await;
        }
    }
}

async fn recover_abnormal_coordinator_exit(
    state: &AppState,
    identity: &AgentCoordinatorRunIdentity,
    reason: &str,
) {
    tracing::error!(
        workspace_id = %identity.workspace.id,
        task_id = %identity.task_id,
        attempt_id = %identity.attempt_id,
        reason,
        "Coordinator task future exited before normal lifecycle closure"
    );
    match fail_claimed_task_durably(state, identity, reason).await {
        Ok(()) => {
            if let Err(wake_error) = state.agent_scheduler.wake() {
                tracing::warn!(
                    workspace_id = %identity.workspace.id,
                    task_id = %identity.task_id,
                    attempt_id = %identity.attempt_id,
                    error = %wake_error.message,
                    "failed to wake Agent scheduler after abnormal task recovery"
                );
            }
        }
        Err(persist_error) => {
            tracing::error!(
                workspace_id = %identity.workspace.id,
                task_id = %identity.task_id,
                attempt_id = %identity.attempt_id,
                original_error = reason,
                closure_error = %persist_error.message,
                "failed to recover abnormal Coordinator task exit"
            );
        }
    }
}

fn panic_payload_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "Coordinator task panicked with a non-string payload".to_string()
}

async fn capture_agent_coordinator_exit<F>(future: F) -> AgentCoordinatorRunExit
where
    F: Future<Output = ()>,
{
    match AssertUnwindSafe(future).catch_unwind().await {
        Ok(()) => AgentCoordinatorRunExit::Finished,
        Err(payload) => AgentCoordinatorRunExit::Panicked(panic_payload_message(payload)),
    }
}

pub(crate) fn reconcile_running_llm_request_audits_on_startup(state: &AppState) {
    let config = match config_snapshot(state) {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(
                error_category = "llm_audit_startup_reconciliation_failed",
                error = %error.message,
                "failed to load configuration for startup LLM audit reconciliation"
            );
            return;
        }
    };
    for workspace in config.local_workspaces() {
        let reconciliation = (|| -> Result<usize, ApiError> {
            let mut database = open_workspace_database_critical(&workspace.path)?;
            database
                .reconcile_running_llm_requests_on_startup()
                .map_err(ApiError::from_workspace_error)
        })();
        match reconciliation {
            Ok(reconciled_llm_requests) if reconciled_llm_requests > 0 => {
                tracing::warn!(
                    workspace_id = %workspace.id,
                    reconciled_llm_requests,
                    reason = "backend_restart_interrupted",
                    "reconciled running LLM request audits from a previous backend process"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::error!(
                    workspace_id = %workspace.id,
                    error_category = "llm_audit_startup_reconciliation_failed",
                    error = %error.message,
                    "failed to reconcile running LLM request audits at startup"
                );
            }
        }
    }
}

pub(crate) fn reconcile_agent_runtime(state: &AppState) -> Result<(), ApiError> {
    reconcile_running_llm_request_audits_on_startup(state);
    reconcile_agent_attempt_leases(state)
}

/// Reconcile only Agent lifecycle state after startup. Unlike the full startup
/// path, this is safe to repeat while coordinators and provider requests run.
fn reconcile_agent_attempt_leases(state: &AppState) -> Result<(), ApiError> {
    let config = config_snapshot(state)?;
    for workspace in config.local_workspaces() {
        let reconciliation = (|| -> Result<(), ApiError> {
            let mut database = open_workspace_database_critical(&workspace.path)?;
            for record in database
                .startup_agent_reconciliation()
                .map_err(ApiError::from_workspace_error)?
            {
                let expected_status = record.task.status;
                if expected_status != AgentTaskStatus::Running {
                    continue;
                }
                let recovery = database.agent_attempt_recovery_disposition(
                    &record.attempt,
                    AGENT_ATTEMPT_LEASE_TIMEOUT,
                );
                if recovery == AgentAttemptRecoveryDisposition::LeaseActive {
                    insert_agent_event(
                        &mut database,
                        &record.task.team_id,
                        "attempt_recovery_deferred",
                        Some(&record.task.owner_instance_id),
                        Some(&record.task.id),
                        Some(&record.attempt.id),
                        json!({ "reason": "lease_active" }),
                    )?;
                    continue;
                }
                if database
                    .suspend_running_agent_task_with_wait_dependencies(
                        &record.task.team_id,
                        &record.task.id,
                    )
                    .map_err(ApiError::from_workspace_error)?
                {
                    insert_agent_event(
                        &mut database,
                        &record.task.team_id,
                        "task_suspended",
                        Some(&record.task.owner_instance_id),
                        Some(&record.task.id),
                        Some(&record.attempt.id),
                        json!({ "reason": "startup_wait_dependency_recovery" }),
                    )?;
                    crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task_with_database(
                    &mut database,
                    &record.task.id,
                )?;
                    continue;
                }
                match close_restarted_coordinator_chat_run(
                    &mut database,
                    &record.task,
                    &record.attempt,
                )? {
                    RestartedCoordinatorChatClosure::Applied => {
                        crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task_with_database(
                            &mut database,
                            &record.task.id,
                        )?;
                        continue;
                    }
                    RestartedCoordinatorChatClosure::OwnerChanged => {
                        insert_agent_event(
                            &mut database,
                            &record.task.team_id,
                            "attempt_recovery_skipped",
                            Some(&record.task.owner_instance_id),
                            Some(&record.task.id),
                            Some(&record.attempt.id),
                            json!({ "reason": "owner_changed" }),
                        )?;
                        continue;
                    }
                    RestartedCoordinatorChatClosure::NotApplicable => {}
                }
                let updated = database
                    .update_agent_task_state_for_attempt_lease(
                        AgentTaskStateUpdate {
                            team_id: &record.task.team_id,
                            task_id: &record.task.id,
                            expected_status,
                            transition: AgentTaskTransition::Interrupt,
                            result_json: None,
                            error_json: Some(
                                r#"{"message":"backend restarted while Agent attempt was active"}"#,
                            ),
                            interruption_reason: Some(RESTART_INTERRUPTION_REASON),
                        },
                        &record.attempt.id,
                        record.attempt.owner_incarnation.as_deref(),
                        record.attempt.lease_renewed_at.as_deref(),
                    )
                    .map_err(ApiError::from_workspace_error)?;
                if !updated {
                    // The owner renewed after this recovery pass took its
                    // snapshot. It is still live, so this scheduler must not
                    // pause its instance or terminalize its Plan phase.
                    insert_agent_event(
                        &mut database,
                        &record.task.team_id,
                        "attempt_recovery_skipped",
                        Some(&record.task.owner_instance_id),
                        Some(&record.task.id),
                        Some(&record.attempt.id),
                        json!({ "reason": "owner_changed" }),
                    )?;
                    continue;
                }
                database
                    .transition_agent_instance_status(
                        &record.task.owner_instance_id,
                        AgentInstanceStatus::Paused,
                    )
                    .map_err(ApiError::from_workspace_error)?;
                insert_agent_event(
                    &mut database,
                    &record.task.team_id,
                    "attempt_interrupted",
                    Some(&record.task.owner_instance_id),
                    Some(&record.task.id),
                    Some(&record.attempt.id),
                    json!({
                        "reason": RESTART_INTERRUPTION_REASON,
                        "recovery": recovery_diagnostic_code(&recovery),
                    }),
                )?;
                database
                    .fail_plan_phase_run(&record.task.id, RESTART_INTERRUPTION_REASON)
                    .map_err(ApiError::from_workspace_error)?;
                crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task_with_database(
                &mut database,
                &record.task.id,
            )?;
            }
            database
                .fail_running_plan_phases_for_terminal_agent_tasks(RESTART_INTERRUPTION_REASON)
                .map_err(ApiError::from_workspace_error)?;
            database
                .fail_running_plan_phases_without_agent_runs(
                    "Plan phase start did not create an implementation chat or Agent task",
                )
                .map_err(ApiError::from_workspace_error)?;
            // Before terminal attempt reconciliation: reopen false `completed`
            // phases whose bound Agent task is still Queued/Running/Waiting so a
            // stale completed phase cannot force live attempts terminal.
            database
                .reconcile_prematurely_completed_plan_phases_with_active_tasks()
                .map_err(ApiError::from_workspace_error)?;
            database
                .reconcile_plan_phase_attempts_for_terminal_phases()
                .map_err(ApiError::from_workspace_error)?;
            database
                .discard_terminal_plan_phase_derived_effects(RESTART_INTERRUPTION_REASON)
                .map_err(ApiError::from_workspace_error)?;
            for instance in database
                .isolated_agent_instances()
                .map_err(ApiError::from_workspace_error)?
            {
                if instance.worktree_status.as_deref() == Some("deleted") {
                    continue;
                }
                let root_path = agent_instance_execution_root(&workspace.path, &instance);
                if root_path.exists() {
                    continue;
                }
                let updated = database
                    .update_agent_instance_worktree_status(&instance.id, "deleted")
                    .map_err(ApiError::from_workspace_error)?;
                insert_agent_event(
                    &mut database,
                    &instance.team_id,
                    "worktree_reconciled",
                    Some(&instance.id),
                    None,
                    None,
                    json!({
                        "reason": "isolated worktree path was not found during startup reconciliation",
                        "executionRootPath": root_path.display().to_string(),
                        "worktreeStatus": updated.worktree_status,
                    }),
                )?;
            }
            Ok(())
        })();
        if let Err(error) = reconciliation {
            tracing::error!(
                workspace_id = %workspace.id,
                error_category = "agent_startup_reconciliation_failed",
                error = %error.message,
                "failed to reconcile Agent runtime at startup for workspace"
            );
        }
    }
    if let Err(error) = crate::plan_runtime::reconcile_plan_derived_effects(state) {
        tracing::warn!(error = %error.message, "failed to reconcile integrated plan derived effects");
    }
    Ok(())
}

fn recovery_diagnostic_code(recovery: &AgentAttemptRecoveryDisposition) -> &'static str {
    match recovery {
        AgentAttemptRecoveryDisposition::VerifiedAbandonedLegacy => "verified_abandoned_legacy",
        AgentAttemptRecoveryDisposition::LeaseActive => "lease_active",
        AgentAttemptRecoveryDisposition::VerifiedAbandonedLeaseExpired => {
            "verified_abandoned_lease_expired"
        }
        AgentAttemptRecoveryDisposition::VerifiedAbandonedInvalidLease => {
            "verified_abandoned_invalid_lease"
        }
    }
}

/// Close a Coordinator-owned implementation chat during startup recovery.
///
/// The task, attempt, streaming assistant, and queuedRun must transition in
/// one transaction so a runner left over from the previous process cannot
/// continue writing after its Plan phase has become terminal.
enum RestartedCoordinatorChatClosure {
    Applied,
    OwnerChanged,
    NotApplicable,
}

fn close_restarted_coordinator_chat_run(
    database: &mut WorkspaceDatabase,
    task: &AgentTaskRecord,
    attempt: &AgentAttemptRecord,
) -> Result<RestartedCoordinatorChatClosure, ApiError> {
    let Ok(input) = serde_json::from_str::<CoordinatorTaskInput>(&task.input_json) else {
        return Ok(RestartedCoordinatorChatClosure::NotApplicable);
    };
    let (Some(assistant_message_id), Some(assistant_sequence)) = (
        input.visible_assistant_message_id.as_deref(),
        input.visible_assistant_sequence,
    ) else {
        return Ok(RestartedCoordinatorChatClosure::NotApplicable);
    };
    let Some(team) = database
        .agent_team(&task.team_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(RestartedCoordinatorChatClosure::NotApplicable);
    };
    let Some(instance) = database
        .agent_instance(&task.owner_instance_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(RestartedCoordinatorChatClosure::NotApplicable);
    };
    if instance.role != AgentRole::Coordinator {
        return Ok(RestartedCoordinatorChatClosure::NotApplicable);
    }

    let error_json = json!({
        "message": RESTART_INTERRUPTION_REASON,
        "code": "backend_restart_interrupted",
        "stage": "startup_reconciliation",
        "retryable": false,
    })
    .to_string();
    let assistant_metadata_json = json!({
        "streamingState": "failed",
        "runFailure": {
            "code": "backend_restart_interrupted",
            "stage": "startup_reconciliation",
            "retryable": false,
            "taskId": task.id.as_str(),
            "attemptId": attempt.id.as_str(),
            "message": RESTART_INTERRUPTION_REASON,
        },
        "parts": [StoredChatMessagePart::Error {
            text: RESTART_INTERRUPTION_REASON.to_string(),
        }],
        "partsVersion": STORED_CHAT_PARTS_VERSION,
        "partsSource": "startup_reconciliation",
    })
    .to_string();
    let result = database
        .close_pre_stream_chat_failure(PreStreamChatFailureClosure {
            task_id: &task.id,
            attempt_id: &attempt.id,
            chat_id: &team.chat_id,
            user_message_id: &input.queued_user_message_id,
            assistant_message_id,
            assistant_sequence,
            error_json: &error_json,
            assistant_content: RESTART_INTERRUPTION_REASON,
            assistant_metadata_json: &assistant_metadata_json,
            expected_attempt_owner_incarnation: attempt.owner_incarnation.as_deref(),
            expected_attempt_lease_renewed_at: attempt.lease_renewed_at.as_deref(),
            expected_queued_run_agent_task_id: None,
            expected_queued_run_id: None,
            materialize_assistant: true,
        })
        .map_err(ApiError::from_workspace_error)?;
    match result {
        PreStreamChatFailureClosureResult::Applied => Ok(RestartedCoordinatorChatClosure::Applied),
        PreStreamChatFailureClosureResult::Skipped { reason } => {
            tracing::info!(
                task_id = %task.id,
                attempt_id = %attempt.id,
                reason = %reason,
                "startup Coordinator chat closure skipped because the durable owner changed"
            );
            Ok(RestartedCoordinatorChatClosure::OwnerChanged)
        }
    }
}

#[derive(Clone)]
struct AgentCoordinatorRunIdentity {
    workspace: WorkspaceConfig,
    task_id: AgentTaskId,
    attempt_id: AgentAttemptId,
    owner_incarnation: String,
}

enum AgentCoordinatorRunExit {
    Finished,
    Panicked(String),
}

struct AgentCoordinatorRunCompletion {
    identity: AgentCoordinatorRunIdentity,
    exit: AgentCoordinatorRunExit,
}

#[derive(Default)]
struct AgentSchedulerScan {
    next_deadline_at: Option<DateTime<Utc>>,
}

async fn schedule_runnable_tasks(
    state: &AppState,
    permits: &Arc<Semaphore>,
    runs: &mut JoinSet<AgentCoordinatorRunCompletion>,
    run_identities: &mut HashMap<TokioTaskId, AgentCoordinatorRunIdentity>,
    owner_incarnation: &str,
) -> Result<AgentSchedulerScan, ApiError> {
    let config = config_snapshot(state)?;
    let mut scan = AgentSchedulerScan::default();
    'scan: for workspace in config.local_workspaces() {
        loop {
            let Ok(permit) = permits.clone().try_acquire_owned() else {
                break 'scan;
            };
            let database = open_workspace_database_critical(&workspace.path)?;
            let completed_plan_tasks = database
                .completed_running_plan_phase_agent_tasks()
                .map_err(ApiError::from_workspace_error)?;
            drop(database);
            for task_id in completed_plan_tasks {
                crate::plan_runtime::sync_plan_phase_for_agent_task(state, workspace, &task_id)
                    .await?;
            }
            let mut database = open_workspace_database_critical(&workspace.path)?;
            for recovered_task in database
                .recover_interrupted_agent_wait_tasks(
                    RESTART_INTERRUPTION_REASON,
                    AGENT_SCHEDULER_SCAN_LIMIT,
                )
                .map_err(ApiError::from_workspace_error)?
            {
                insert_agent_event(
                    &mut database,
                    &recovered_task.team_id,
                    "task_suspended",
                    Some(&recovered_task.owner_instance_id),
                    Some(&recovered_task.id),
                    None,
                    json!({ "reason": "interrupted_wait_dependency_recovery" }),
                )?;
            }
            for resumed_task in database
                .resume_satisfied_agent_tasks(AGENT_SCHEDULER_SCAN_LIMIT)
                .map_err(ApiError::from_workspace_error)?
            {
                insert_agent_event(
                    &mut database,
                    &resumed_task.team_id,
                    "task_resumed",
                    Some(&resumed_task.owner_instance_id),
                    Some(&resumed_task.id),
                    None,
                    json!({}),
                )?;
            }
            let Some(task) = database
                .runnable_agent_tasks(AGENT_SCHEDULER_SCAN_LIMIT)
                .map_err(ApiError::from_workspace_error)?
                .into_iter()
                .next()
            else {
                record_next_agent_deadline(&mut scan, &database)?;
                drop(permit);
                break;
            };
            let attempt_id = AgentAttemptId::new(unique_id("agent-attempt"))
                .map_err(|error| ApiError::internal(error.to_string()))?;
            let Some(claimed) = database
                .claim_runnable_agent_task_with_owner(
                    &task.team_id,
                    &task.id,
                    &attempt_id,
                    Some(owner_incarnation),
                )
                .map_err(ApiError::from_workspace_error)?
            else {
                drop(permit);
                continue;
            };
            drop(database);
            let identity = AgentCoordinatorRunIdentity {
                workspace: workspace.clone(),
                task_id: claimed.id.clone(),
                attempt_id: attempt_id.clone(),
                owner_incarnation: owner_incarnation.to_string(),
            };
            if let Err(error) =
                crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task(
                    &workspace.path,
                    &claimed.id,
                )
            {
                drop(permit);
                fail_claimed_task_after_scheduler_error(state, &identity, &error).await?;
                continue;
            }
            let mut database = open_workspace_database_critical(&workspace.path)?;
            if let Err(error) = insert_agent_event(
                &mut database,
                &claimed.team_id,
                "attempt_started",
                Some(&claimed.owner_instance_id),
                Some(&claimed.id),
                Some(&attempt_id),
                json!({}),
            ) {
                drop(database);
                drop(permit);
                fail_claimed_task_after_scheduler_error(state, &identity, &error).await?;
                continue;
            }
            if let Err(error) = insert_agent_event(
                &mut database,
                &claimed.team_id,
                "task_started",
                Some(&claimed.owner_instance_id),
                Some(&claimed.id),
                Some(&attempt_id),
                json!({
                    "queueWaitMs": timestamp_delta_ms(
                        Some(&claimed.created_at),
                        claimed.started_at.as_deref()
                    ),
                    "schedulerLatencyMs": timestamp_delta_ms(
                        Some(&task.updated_at),
                        claimed.started_at.as_deref()
                    ),
                }),
            ) {
                drop(database);
                drop(permit);
                fail_claimed_task_after_scheduler_error(state, &identity, &error).await?;
                continue;
            }
            drop(database);
            let run_state = state.clone();
            let run_identity = identity.clone();
            let abort_handle = runs.spawn(async move {
                let _permit = permit;
                let exit = capture_agent_coordinator_exit(run_coordinator_task(
                    run_state,
                    run_identity.workspace.clone(),
                    run_identity.task_id.clone(),
                    run_identity.attempt_id.clone(),
                    run_identity.owner_incarnation.clone(),
                ))
                .await;
                AgentCoordinatorRunCompletion {
                    identity: run_identity,
                    exit,
                }
            });
            run_identities.insert(abort_handle.id(), identity);
        }
    }
    Ok(scan)
}

async fn fail_claimed_task_after_scheduler_error(
    state: &AppState,
    identity: &AgentCoordinatorRunIdentity,
    original_error: &ApiError,
) -> Result<(), ApiError> {
    match fail_claimed_task_durably(state, identity, &original_error.message).await {
        Ok(()) => {
            if let Err(wake_error) = state.agent_scheduler.wake() {
                tracing::warn!(
                    workspace_id = %identity.workspace.id,
                    task_id = %identity.task_id,
                    attempt_id = %identity.attempt_id,
                    error = %wake_error.message,
                    "failed to wake Agent scheduler after failed task closure"
                );
            }
            Ok(())
        }
        Err(closure_error) => {
            tracing::error!(
                workspace_id = %identity.workspace.id,
                task_id = %identity.task_id,
                attempt_id = %identity.attempt_id,
                original_error = %original_error.message,
                closure_error = %closure_error.message,
                "failed to close claimed Agent task after scheduler error"
            );
            Err(ApiError::internal(format!(
                "Agent scheduler operation failed: {}; failed to persist task failure: {}",
                original_error.message, closure_error.message
            )))
        }
    }
}

fn record_next_agent_deadline(
    scan: &mut AgentSchedulerScan,
    database: &WorkspaceDatabase,
) -> Result<(), ApiError> {
    let Some(value) = database
        .next_waiting_agent_task_dependency_deadline()
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(());
    };
    let deadline = DateTime::parse_from_rfc3339(&value)
        .map_err(|source| {
            ApiError::internal(format!(
                "Agent task dependency deadline is invalid: {source}"
            ))
        })?
        .with_timezone(&Utc);
    match scan.next_deadline_at.as_ref() {
        Some(current) if current <= &deadline => {}
        _ => scan.next_deadline_at = Some(deadline),
    }
    Ok(())
}

fn agent_scheduler_deadline_delay(next_deadline_at: Option<&DateTime<Utc>>) -> Duration {
    let Some(next_deadline_at) = next_deadline_at else {
        // Lease expiry is a durable recovery deadline even when no task
        // dependency deadline exists. This re-check is what eventually closes
        // a coordinator that was live during startup but later proved absent.
        return AGENT_ATTEMPT_LEASE_TIMEOUT;
    };
    let now = Utc::now();
    if next_deadline_at <= &now {
        return Duration::from_millis(AGENT_SCHEDULER_MIN_DEADLINE_DELAY_MS);
    }
    let millis_until_deadline = next_deadline_at
        .signed_duration_since(now)
        .num_milliseconds();
    if millis_until_deadline <= 0 {
        return Duration::from_millis(AGENT_SCHEDULER_MIN_DEADLINE_DELAY_MS);
    }
    Duration::from_millis(millis_until_deadline as u64).min(AGENT_ATTEMPT_LEASE_TIMEOUT)
}

async fn run_coordinator_task(
    state: AppState,
    workspace: WorkspaceConfig,
    task_id: AgentTaskId,
    attempt_id: AgentAttemptId,
    owner_incarnation: String,
) {
    let identity = AgentCoordinatorRunIdentity {
        workspace: workspace.clone(),
        task_id: task_id.clone(),
        attempt_id: attempt_id.clone(),
        owner_incarnation: owner_incarnation.clone(),
    };
    let result = run_coordinator_task_with_lease_heartbeat(
        &state,
        &workspace,
        &task_id,
        &attempt_id,
        &owner_incarnation,
    )
    .await;
    if let Err(error) = result {
        tracing::error!(
            workspace_id = %workspace.id,
            task_id = %task_id,
            attempt_id = %attempt_id,
            error = %error.message,
            "Coordinator task failed"
        );
        if let Err(persist_error) =
            fail_claimed_task_durably(&state, &identity, &error.message).await
        {
            tracing::error!(
                workspace_id = %workspace.id,
                task_id = %task_id,
                attempt_id = %attempt_id,
                original_error = %error.message,
                closure_error = %persist_error.message,
                "failed to persist failed Coordinator task"
            );
            return;
        }
    }
    if let Err(wake_error) = state.agent_scheduler.wake() {
        tracing::warn!(
            workspace_id = %workspace.id,
            task_id = %task_id,
            attempt_id = %attempt_id,
            error = %wake_error.message,
            "failed to wake Agent scheduler after Coordinator task closure"
        );
    }
}

async fn run_coordinator_task_with_lease_heartbeat(
    state: &AppState,
    workspace: &WorkspaceConfig,
    task_id: &AgentTaskId,
    attempt_id: &AgentAttemptId,
    owner_incarnation: &str,
) -> Result<(), ApiError> {
    let run = run_coordinator_task_inner(state, workspace, task_id, attempt_id);
    tokio::pin!(run);
    let mut heartbeat = time::interval(AGENT_ATTEMPT_LEASE_HEARTBEAT);
    heartbeat.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    // Claiming the attempt writes its first lease renewal.
    heartbeat.tick().await;
    loop {
        tokio::select! {
            result = &mut run => return result,
            _ = heartbeat.tick() => {
                let renewal = (|| -> Result<bool, ApiError> {
                    let mut database = open_workspace_database_critical(&workspace.path)?;
                    database
                        .renew_agent_attempt_lease(task_id, attempt_id, owner_incarnation)
                        .map_err(ApiError::from_workspace_error)
                })();
                match renewal {
                    Ok(true) => {}
                    Ok(false) => {
                        return Err(ApiError::conflict(
                            "Agent attempt lease is no longer owned by this coordinator",
                        ));
                    }
                    Err(error) => {
                        // A failed renewal does not prove another owner took over. Keep the
                        // coordinator alive so a transient gate/SQLite failure cannot turn
                        // into an incorrect terminal transition; the next heartbeat retries.
                        tracing::warn!(
                            workspace_id = %workspace.id,
                            task_id = %task_id,
                            attempt_id = %attempt_id,
                            error = %error.message,
                            "failed to renew Agent attempt lease; retrying on next heartbeat"
                        );
                    }
                }
            }
        }
    }
}

async fn run_coordinator_task_inner(
    state: &AppState,
    workspace: &WorkspaceConfig,
    task_id: &AgentTaskId,
    attempt_id: &AgentAttemptId,
) -> Result<(), ApiError> {
    let database = open_workspace_database_critical(&workspace.path)?;
    let task = database
        .agent_task(task_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::internal(format!("Agent task '{task_id}' was not found")))?;
    let team = database
        .agent_team(&task.team_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::internal(format!("Agent team '{}' was not found", task.team_id))
        })?;
    let instance = database
        .agent_instance(&task.owner_instance_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::internal(format!(
                "Agent instance '{}' was not found",
                task.owner_instance_id
            ))
        })?;
    let task_input =
        serde_json::from_str::<CoordinatorTaskInput>(&task.input_json).map_err(|source| {
            ApiError::internal(format!("invalid Coordinator task input: {source}"))
        })?;
    let session_upload_paths = task_input
        .attachments
        .iter()
        .filter_map(|attachment| attachment.path.clone())
        .collect::<Vec<_>>();
    let model_selection = agent_task_model_selection(&database, &team, &instance, &task_input)?;
    drop(database);

    let config = config_snapshot(state)?;
    validate_agent_snapshot_for_workspace(&config, workspace, &instance.definition_snapshot)?;
    let agent_primary_chat_output = instance.role == AgentRole::Coordinator;
    let mut chat_context = prepare_chat_context_for_output(
        state,
        &config,
        &workspace.id,
        ChatStreamRequest {
            chat_id: Some(team.chat_id.clone()),
            queued_user_message_id: Some(task_input.queued_user_message_id.clone()),
            run_id_override: Some(task.id.to_string()),
            visible_assistant_message_id: task_input.visible_assistant_message_id.clone(),
            visible_assistant_sequence: task_input.visible_assistant_sequence,
            model_id: model_selection.model_id,
            provider_id: None,
            thinking_level: model_selection.thinking_level,
            latency_mode: task_input.latency_mode,
            skill_ids: Some(task_input.skill_ids.clone()),
            session_mode: task_input.session_mode.clone(),
            message: task_input.message.clone(),
            attachments: task_input.attachments.clone(),
        },
        agent_primary_chat_output,
    )
    .await
    .map_err(|error| {
        if is_workspace_database_concurrency_error(&error) {
            pre_stream_workspace_database_busy_error(&error.message)
        } else {
            error
        }
    })?;
    chat_context.tool_workspace_path = match instance.execution_workspace_mode {
        AgentExecutionWorkspaceMode::Shared => instance
            .execution_root_path
            .as_deref()
            .map(|root_path| resolve_agent_worktree_path(&workspace.path, root_path))
            .unwrap_or_else(|| workspace.path.clone()),
        AgentExecutionWorkspaceMode::IsolatedWorktree => {
            agent_instance_execution_root(&workspace.path, &instance)
        }
    };
    // Prewarm the execution-root graph as soon as the tool root is known so the
    // first Graph tool call does not wait on a cold full index. Shared main-root
    // sessions already claim via prepare_prompt_context; this is a no-op when
    // the path is already Initializing/Ready.
    spawn_code_graph_execution_root_initialization_if_needed(
        state.code_graph_indexes.clone(),
        chat_context.tool_workspace_path.clone(),
        format!("agent-task:{}", task.id),
    );
    let allowed_tools = instance
        .definition_snapshot
        .allowed_tools
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    retain_agent_snapshot_tools(&mut chat_context.provider_request.tools, &allowed_tools);
    let removed_tool_routing = sync_agent_tool_routing_prompt(
        &mut chat_context.provider_request.messages,
        &mut chat_context.message_source_sequences,
        &mut chat_context.message_context_sources,
        &chat_context.provider_request.tools,
    );
    if removed_tool_routing {
        chat_context.active_tool_start_index =
            chat_context.active_tool_start_index.saturating_sub(1);
    }
    let collaboration_permissions = if task_input.collaboration_tools_enabled {
        instance.definition_snapshot.permissions.clone()
    } else {
        AgentPermissions::default()
    };
    if task_input.collaboration_tools_enabled {
        append_agent_collaboration_tools(&mut chat_context, &collaboration_permissions);
    }
    if let Some(max_output_tokens) = instance.definition_snapshot.model_options.max_output_tokens {
        chat_context.provider_request.max_output_tokens = Some(max_output_tokens);
    }
    chat_context.agent_associations = AgentRunAssociations {
        team_id: Some(task.team_id.clone()),
        instance_id: Some(task.owner_instance_id.clone()),
        task_id: Some(task.id.clone()),
        attempt_id: Some(attempt_id.clone()),
    };
    let (guidance_tx, guidance_rx) = mpsc::unbounded_channel();
    let mut database = open_workspace_database_critical(&workspace.path)?;
    if chat_context.agent_primary_chat_output {
        let queued_user_message_id =
            chat_context
                .queued_user_message_id
                .as_deref()
                .ok_or_else(|| {
                    ApiError::internal(
                        "Coordinator primary chat run is missing its queued user message",
                    )
                })?;
        database
            .claim_agent_chat_queued_run(
                &chat_context.chat_id,
                queued_user_message_id,
                &chat_context.assistant_message_id,
                chat_context.assistant_sequence,
                task.id.as_str(),
                &chat_context.llm_request_id,
            )
            .map_err(ApiError::from_workspace_error)?;
    }
    let next_run_event_sequence = database
        .next_run_event_sequence(task.id.as_str())
        .map_err(ApiError::from_workspace_error)?;
    drop(database);
    // Register before snapshotting unread Agent messages. Messages sent after the snapshot are
    // buffered as live guidance instead of being stranded until a later attempt.
    let registration = state
        .active_chat_runs
        .register_agent_with_queued_user_message(
            task.id.to_string(),
            workspace.id.clone(),
            team.chat_id.clone(),
            chat_context.assistant_message_id.clone(),
            chat_context.assistant_sequence,
            chat_context.queued_user_message_id.clone(),
            chat_context.memories_used.clone(),
            chat_context.agent_primary_chat_output,
            ActiveAgentRunIdentity {
                team_id: task.team_id.clone(),
                instance_id: task.owner_instance_id.clone(),
                task_id: task.id.clone(),
                _attempt_id: attempt_id.clone(),
            },
            next_run_event_sequence,
            guidance_tx,
        )?;
    let registration = match registration {
        ActiveChatRunRegistrationResult::Registered(registration) => registration,
        ActiveChatRunRegistrationResult::Existing => {
            tracing::debug!(
                workspace_id = %workspace.id,
                task_id = %task.id,
                "Coordinator task replay is already owned by an active chat run"
            );
            return Ok(());
        }
    };
    let (agent_unread_messages, consumed_agent_message_ids) = apply_agent_prompt_layers(
        &workspace.path,
        &mut chat_context,
        &team,
        &instance,
        &task,
        attempt_id,
        &allowed_tools,
        task_input.collaboration_tools_enabled,
        &collaboration_permissions,
        &config.agent_definitions,
    )?;
    chat_context.agent_unread_messages = agent_unread_messages;
    if chat_context.pending_memory_retrieval.is_none() {
        chat_context.provider_request.prompt_cache_key = Some(prompt_cache_key(
            &chat_context.workspace_id,
            &chat_context.chat_id,
            &chat_context.provider_id,
            &chat_context.model_id,
            &chat_context.provider_request,
            &chat_context.message_source_sequences,
            &chat_context.message_context_sources,
        )?);
        chat_context.provider_request.prompt_cache_retention =
            Some(PROMPT_CACHE_RETENTION_24H.to_string());
    }
    retry_agent_runtime_database_operation("consume Agent messages", || {
        consume_agent_messages(&workspace.path, &consumed_agent_message_ids)
    })
    .await?;
    let database = open_workspace_database_critical(&workspace.path)?;
    chat_context.plan_phase_provenance = database
        .plan_phase_attempt_for_agent_task(&task.id)
        .map_err(ApiError::from_workspace_error)?
        .map(|plan_attempt| PlanPhaseRunProvenance {
            plan_id: plan_attempt.plan_id,
            phase_id: plan_attempt.phase_id,
            attempt_id: plan_attempt.id,
            agent_task_id: task.id.clone(),
            integration_status: PlanPhaseIntegrationStatus::AwaitingIntegration,
        });
    // Re-resolve after plan bind so OpenAIResp session-id becomes plan_id when applicable.
    // Reuse the critical DB still held below; do not nest another workspace open.
    chat_context.refresh_provider_session_thread_mapping_with_database(&database)?;
    drop(database);
    chat_context.agent_definition_snapshot = Some(
        serde_json::to_value(&instance.definition_snapshot).map_err(|source| {
            ApiError::internal(format!(
                "failed to serialize Agent definition snapshot: {source}"
            ))
        })?,
    );
    let input = agent_task_input_prompt_value(&task)?;
    chat_context.agent_task_input = Some(input);
    chat_context.agent_allowed_tools = Some(allowed_tools);
    chat_context.agent_tool_context = Some(AgentToolContext {
        workspace_id: workspace.id.clone(),
        workspace_path: workspace.path.clone(),
        associations: chat_context.agent_associations.clone(),
        collaboration_tools_enabled: task_input.collaboration_tools_enabled,
        permissions: collaboration_permissions,
        agent_definitions: config.agent_definitions.clone(),
        scheduler: state.agent_scheduler.clone(),
        active_chat_runs: state.active_chat_runs.clone(),
    });
    chat_context.session_upload_paths = Some(session_upload_paths);

    let outcome = run_chat_context_in_background(chat_context, registration, guidance_rx).await;
    let lifecycle_context =
        AgentLifecycleOperationContext::from_identity(workspace, task_id, attempt_id);
    retry_agent_lifecycle_database_operation(
        "persist Agent task context",
        &lifecycle_context,
        state.app_shutdown_rx.clone(),
        || persist_agent_task_context(&workspace.path, &task, &instance, attempt_id, &outcome),
    )
    .await?;
    retry_agent_lifecycle_database_operation(
        "finish Agent task",
        &lifecycle_context,
        state.app_shutdown_rx.clone(),
        || finish_claimed_task(&workspace.path, &task, attempt_id, outcome.clone()),
    )
    .await?;
    crate::plan_runtime::sync_plan_phase_for_agent_task(state, workspace, &task.id).await
}

struct AgentLifecycleOperationContext<'a> {
    workspace_id: &'a str,
    workspace_path: &'a Path,
    task_id: &'a AgentTaskId,
    attempt_id: Option<&'a AgentAttemptId>,
}

impl<'a> AgentLifecycleOperationContext<'a> {
    fn from_identity(
        workspace: &'a WorkspaceConfig,
        task_id: &'a AgentTaskId,
        attempt_id: &'a AgentAttemptId,
    ) -> Self {
        Self {
            workspace_id: &workspace.id,
            workspace_path: &workspace.path,
            task_id,
            attempt_id: Some(attempt_id),
        }
    }
}

async fn retry_agent_runtime_database_operation<T, F>(
    operation_name: &'static str,
    mut operation: F,
) -> Result<T, ApiError>
where
    F: FnMut() -> Result<T, ApiError>,
{
    for attempt in 1..=AGENT_TASK_DB_SHORT_RETRY_ATTEMPTS {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error)
                if attempt < AGENT_TASK_DB_SHORT_RETRY_ATTEMPTS
                    && is_workspace_database_concurrency_error(&error) =>
            {
                tracing::warn!(
                    operation = operation_name,
                    attempt,
                    error = %error.message,
                    "Agent runtime database operation hit concurrency limit; short retry"
                );
                time::sleep(AGENT_TASK_DB_SHORT_RETRY_DELAY).await;
            }
            Err(error) => return Err(error),
        }
    }

    unreachable!("retry loop always returns")
}

async fn retry_agent_lifecycle_database_operation<T, F>(
    operation_name: &'static str,
    context: &AgentLifecycleOperationContext<'_>,
    mut shutdown_rx: watch::Receiver<bool>,
    mut operation: F,
) -> Result<T, ApiError>
where
    F: FnMut() -> Result<T, ApiError>,
{
    let started_at = Instant::now();
    let mut attempt = 0_u64;
    let mut retry_delay = AGENT_LIFECYCLE_DB_RETRY_INITIAL_DELAY;
    let mut next_warning_at = Duration::ZERO;
    let mut next_error_at = AGENT_LIFECYCLE_DB_RETRY_ERROR_AFTER;

    loop {
        attempt = attempt.saturating_add(1);
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if is_workspace_database_concurrency_error(&error) => {
                let elapsed = started_at.elapsed();
                if elapsed >= next_error_at {
                    tracing::error!(
                        workspace_id = context.workspace_id,
                        workspace_path = %context.workspace_path.display(),
                        task_id = %context.task_id,
                        attempt_id = context.attempt_id.map(ToString::to_string),
                        operation = operation_name,
                        retry_attempt = attempt,
                        elapsed_ms = elapsed.as_millis(),
                        error = %error.message,
                        "Agent lifecycle database operation remains blocked; runtime health degraded"
                    );
                    next_error_at = elapsed.saturating_add(AGENT_LIFECYCLE_DB_RETRY_ERROR_INTERVAL);
                } else if elapsed >= next_warning_at {
                    tracing::warn!(
                        workspace_id = context.workspace_id,
                        workspace_path = %context.workspace_path.display(),
                        task_id = %context.task_id,
                        attempt_id = context.attempt_id.map(ToString::to_string),
                        operation = operation_name,
                        retry_attempt = attempt,
                        elapsed_ms = elapsed.as_millis(),
                        retry_delay_ms = retry_delay.as_millis(),
                        error = %error.message,
                        "Agent lifecycle database operation hit concurrency limit; retrying durably"
                    );
                    next_warning_at =
                        elapsed.saturating_add(AGENT_LIFECYCLE_DB_RETRY_WARNING_INTERVAL);
                }

                if *shutdown_rx.borrow() {
                    return Err(ApiError::internal(format!(
                        "{SHUTDOWN_MESSAGE} while waiting to {operation_name}"
                    )));
                }
                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            return Err(ApiError::internal(format!(
                                "{SHUTDOWN_MESSAGE} while waiting to {operation_name}"
                            )));
                        }
                    }
                    _ = time::sleep(retry_delay) => {}
                }
                retry_delay = retry_delay
                    .saturating_mul(2)
                    .min(AGENT_LIFECYCLE_DB_RETRY_MAX_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
pub(crate) async fn agent_lifecycle_retry_until_shutdown_for_test(
    workspace_path: &Path,
    task_id: &AgentTaskId,
    attempt_id: &AgentAttemptId,
    shutdown_rx: watch::Receiver<bool>,
    started_tx: tokio::sync::oneshot::Sender<()>,
) -> Result<(), ApiError> {
    let workspace_id = workspace_path.display().to_string();
    let context = AgentLifecycleOperationContext {
        workspace_id: &workspace_id,
        workspace_path,
        task_id,
        attempt_id: Some(attempt_id),
    };
    let mut started_tx = Some(started_tx);
    retry_agent_lifecycle_database_operation("test shutdown handoff", &context, shutdown_rx, || {
        if let Some(started_tx) = started_tx.take() {
            let _ = started_tx.send(());
        }
        Err(ApiError::internal(
            "workspace database concurrency limit reached: synthetic sustained pressure",
        ))
    })
    .await
}

#[cfg(test)]
pub(crate) async fn recover_panicked_coordinator_for_test(
    state: &AppState,
    workspace: WorkspaceConfig,
    task_id: AgentTaskId,
    attempt_id: AgentAttemptId,
) -> Result<(), ApiError> {
    let exit = capture_agent_coordinator_exit(async {
        panic!("synthetic Coordinator panic after claim");
    })
    .await;
    let AgentCoordinatorRunExit::Panicked(message) = exit else {
        return Err(ApiError::internal(
            "synthetic Coordinator panic completed normally",
        ));
    };
    let identity = AgentCoordinatorRunIdentity {
        workspace,
        task_id,
        attempt_id,
        owner_incarnation: "agent-owner-test-panic-recovery".to_string(),
    };
    let reason = format!("Coordinator task panicked: {message}");
    recover_abnormal_coordinator_exit(state, &identity, &reason).await;
    let database = open_workspace_database_critical(&identity.workspace.path)?;
    let task = database
        .agent_task(&identity.task_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::internal("panic recovery task was not found"))?;
    if task.status != AgentTaskStatus::Failed {
        return Err(ApiError::internal(format!(
            "panic recovery left Agent task '{}' in state '{}'",
            task.id,
            task.status.as_str()
        )));
    }
    Ok(())
}

async fn fail_claimed_task_durably(
    state: &AppState,
    identity: &AgentCoordinatorRunIdentity,
    message: &str,
) -> Result<(), ApiError> {
    let context = AgentLifecycleOperationContext::from_identity(
        &identity.workspace,
        &identity.task_id,
        &identity.attempt_id,
    );
    retry_agent_lifecycle_database_operation(
        "fail claimed Agent task",
        &context,
        state.app_shutdown_rx.clone(),
        || {
            fail_claimed_task_with_pre_stream_closure(
                &identity.workspace.path,
                &identity.task_id,
                Some(&identity.attempt_id),
                message,
            )
        },
    )
    .await
}

pub(crate) async fn open_workspace_database_ordinary_with_pre_stream_retry(
    workspace_path: &Path,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<foco_store::workspace::WorkspaceDatabaseHandle, ApiError> {
    let started_at = Instant::now();
    let mut attempt = 0_u64;
    let mut retry_delay = PRE_STREAM_DB_RETRY_INITIAL_DELAY;

    loop {
        attempt = attempt.saturating_add(1);
        match open_workspace_database(workspace_path) {
            Ok(database) => return Ok(database),
            Err(api_error) => {
                if !is_workspace_database_concurrency_error(&api_error) {
                    return Err(api_error);
                }
                if started_at.elapsed() >= PRE_STREAM_DB_RETRY_BUDGET {
                    return Err(pre_stream_workspace_database_busy_error(&api_error.message));
                }
                if *shutdown_rx.borrow() {
                    return Err(ApiError::internal(
                        "application is shutting down while waiting for workspace database",
                    ));
                }
                tracing::warn!(
                    workspace = %workspace_path.display(),
                    attempt,
                    error = %api_error.message,
                    "pre-stream ordinary workspace database open hit concurrency limit; bounded retry"
                );
                let jitter_ms = (attempt.saturating_mul(17)) % 50;
                let sleep_for = retry_delay.saturating_add(Duration::from_millis(jitter_ms));
                tokio::select! {
                    _ = time::sleep(sleep_for) => {}
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            return Err(ApiError::internal(
                                "application is shutting down while waiting for workspace database",
                            ));
                        }
                    }
                }
                retry_delay = retry_delay
                    .saturating_mul(2)
                    .min(PRE_STREAM_DB_RETRY_MAX_DELAY);
            }
        }
    }
}

fn pre_stream_workspace_database_busy_error(diagnostic: &str) -> ApiError {
    ApiError::internal(format!(
        "pre_stream_failure code={PRE_STREAM_FAILURE_CODE_WORKSPACE_DATABASE_BUSY} stage={PRE_STREAM_FAILURE_STAGE_PREPARE} retryable=true message={PRE_STREAM_USER_MESSAGE_DATABASE_BUSY} detail={diagnostic}"
    ))
}

fn parse_pre_stream_failure(message: &str) -> PreStreamFailureInfo {
    if message.contains(PRE_STREAM_FAILURE_CODE_WORKSPACE_DATABASE_BUSY)
        || is_workspace_database_concurrency_error_message(message)
    {
        return PreStreamFailureInfo {
            code: PRE_STREAM_FAILURE_CODE_WORKSPACE_DATABASE_BUSY.to_string(),
            stage: PRE_STREAM_FAILURE_STAGE_PREPARE.to_string(),
            retryable: true,
            user_message: PRE_STREAM_USER_MESSAGE_DATABASE_BUSY.to_string(),
            diagnostic: message.to_string(),
        };
    }
    if let Some(rest) = message.strip_prefix("pre_stream_failure ") {
        let mut code = "pre_stream_error".to_string();
        let mut stage = PRE_STREAM_FAILURE_STAGE_PREPARE.to_string();
        let mut retryable = false;
        for token in rest.split_whitespace() {
            if let Some(value) = token.strip_prefix("code=") {
                code = value.to_string();
            } else if let Some(value) = token.strip_prefix("stage=") {
                stage = value.to_string();
            } else if let Some(value) = token.strip_prefix("retryable=") {
                retryable = value == "true";
            }
        }
        let (user_message, diagnostic) =
            if let Some((_, after_message)) = rest.split_once(" message=") {
                if let Some((user, detail)) = after_message.split_once(" detail=") {
                    (user.to_string(), detail.to_string())
                } else {
                    (after_message.to_string(), message.to_string())
                }
            } else {
                (
                    PRE_STREAM_USER_MESSAGE_GENERIC.to_string(),
                    message.to_string(),
                )
            };
        return PreStreamFailureInfo {
            code,
            stage,
            retryable,
            user_message,
            diagnostic,
        };
    }
    PreStreamFailureInfo {
        code: "pre_stream_error".to_string(),
        stage: PRE_STREAM_FAILURE_STAGE_PREPARE.to_string(),
        retryable: false,
        user_message: PRE_STREAM_USER_MESSAGE_GENERIC.to_string(),
        diagnostic: message.to_string(),
    }
}

#[derive(Clone, Debug)]
struct PreStreamFailureInfo {
    code: String,
    stage: String,
    retryable: bool,
    user_message: String,
    diagnostic: String,
}

pub(crate) fn pre_stream_failure_user_message(message: &str) -> String {
    parse_pre_stream_failure(message).user_message
}

fn is_workspace_database_concurrency_error_message(message: &str) -> bool {
    message.contains("workspace database concurrency limit reached")
}

fn fail_claimed_task_with_pre_stream_closure(
    workspace_path: &Path,
    task_id: &AgentTaskId,
    expected_attempt_id: Option<&AgentAttemptId>,
    message: &str,
) -> Result<(), ApiError> {
    let mut database = open_workspace_database_critical(workspace_path)?;
    let Some(task) = database
        .agent_task(task_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(());
    };
    if task.status != AgentTaskStatus::Running {
        return Ok(());
    }

    let failure = parse_pre_stream_failure(message);
    let mut error = json!({
        "message": failure.user_message,
        "code": failure.code,
        "stage": failure.stage,
        "retryable": failure.retryable,
        "diagnostic": failure.diagnostic,
    });
    let mut error_json = error.to_string();
    if error_json.len() > AGENT_MAX_TASK_OUTCOME_BYTES {
        error = json!({
            "message": format!(
                "Agent task error_json exceeds {AGENT_MAX_TASK_OUTCOME_BYTES} bytes"
            )
        });
        error_json = error.to_string();
    }

    // Prefer atomic coordinator pre-stream closure when we have attempt + visible assistant ids.
    if let Some(attempt_id) = expected_attempt_id {
        if let Ok(task_input) = serde_json::from_str::<CoordinatorTaskInput>(&task.input_json) {
            if let Some(assistant_id) = task_input.visible_assistant_message_id.as_deref() {
                if let Some(assistant_sequence) = task_input.visible_assistant_sequence {
                    let team = database
                        .agent_team(&task.team_id)
                        .map_err(ApiError::from_workspace_error)?;
                    let instance = database
                        .agent_instance(&task.owner_instance_id)
                        .map_err(ApiError::from_workspace_error)?;
                    let materialize_assistant = instance
                        .as_ref()
                        .is_some_and(|instance| instance.role == AgentRole::Coordinator);
                    if let Some(team) = team {
                        let run_failure = json!({
                            "code": failure.code,
                            "stage": failure.stage,
                            "retryable": failure.retryable,
                            "taskId": task.id.as_str(),
                            "attemptId": attempt_id.as_str(),
                            "message": failure.user_message,
                        });
                        let parts = vec![StoredChatMessagePart::Error {
                            text: failure.user_message.clone(),
                        }];
                        let assistant_metadata = json!({
                            "streamingState": "failed",
                            "runFailure": run_failure,
                            "parts": parts,
                            "partsVersion": STORED_CHAT_PARTS_VERSION,
                            "partsSource": "pre_stream_failure",
                        })
                        .to_string();
                        let result = database
                            .close_pre_stream_chat_failure(PreStreamChatFailureClosure {
                                task_id: &task.id,
                                attempt_id,
                                chat_id: &team.chat_id,
                                user_message_id: &task_input.queued_user_message_id,
                                assistant_message_id: assistant_id,
                                assistant_sequence,
                                error_json: &error_json,
                                assistant_content: &failure.user_message,
                                assistant_metadata_json: &assistant_metadata,
                                expected_attempt_owner_incarnation: None,
                                expected_attempt_lease_renewed_at: None,
                                expected_queued_run_agent_task_id: None,
                                expected_queued_run_id: None,
                                materialize_assistant,
                            })
                            .map_err(ApiError::from_workspace_error)?;
                        match result {
                            PreStreamChatFailureClosureResult::Applied => {
                                crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task_with_database(
                                    &mut database,
                                    &task.id,
                                )?;
                                return Ok(());
                            }
                            PreStreamChatFailureClosureResult::Skipped { reason } => {
                                tracing::info!(
                                    task_id = %task.id,
                                    attempt_id = %attempt_id,
                                    reason = %reason,
                                    "pre-stream failure closure skipped; falling back to task-only fail"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Generic path: task/event only (workers, missing assistant identity, or skipped race).
    let update = AgentTaskStateUpdate {
        team_id: &task.team_id,
        task_id: &task.id,
        expected_status: AgentTaskStatus::Running,
        transition: AgentTaskTransition::Fail,
        result_json: None,
        error_json: Some(&error_json),
        interruption_reason: None,
    };
    let updated = match expected_attempt_id {
        Some(attempt_id) => database.update_agent_task_state_for_attempt(update, attempt_id),
        None => database.update_agent_task_state(update),
    }
    .map_err(ApiError::from_workspace_error)?;
    if !updated {
        return Ok(());
    }
    insert_agent_event(
        &mut database,
        &task.team_id,
        "task_failed",
        Some(&task.owner_instance_id),
        Some(&task.id),
        expected_attempt_id,
        json!({
            "outcome": error,
            "recoveryReason": "coordinator_lifecycle_closure",
        }),
    )?;
    database
        .fail_plan_phase_run(&task.id, &failure.user_message)
        .map_err(ApiError::from_workspace_error)?;
    crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task_with_database(
        &mut database,
        &task.id,
    )?;
    Ok(())
}

pub(crate) async fn fail_claimed_task_with_retry(
    workspace_path: &Path,
    task_id: &AgentTaskId,
    message: &str,
) -> Result<(), ApiError> {
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let workspace_id = workspace_path.display().to_string();
    let context = AgentLifecycleOperationContext {
        workspace_id: &workspace_id,
        workspace_path,
        task_id,
        attempt_id: None,
    };
    retry_agent_lifecycle_database_operation(
        "fail claimed Agent task",
        &context,
        shutdown_rx,
        || fail_claimed_task(workspace_path, task_id, None, message),
    )
    .await
}

fn is_workspace_database_concurrency_error(error: &ApiError) -> bool {
    is_workspace_database_concurrency_error_message(&error.message)
}

fn agent_task_model_selection(
    database: &WorkspaceDatabase,
    team: &AgentTeamRecord,
    instance: &AgentInstanceRecord,
    task_input: &CoordinatorTaskInput,
) -> Result<AgentTaskModelSelection, ApiError> {
    let queued_run = match database
        .message(&task_input.queued_user_message_id)
        .map_err(ApiError::from_workspace_error)?
    {
        Some(message) if message.chat_id == team.chat_id => {
            queued_run_summary_from_message_metadata(&message.metadata_json)?
        }
        Some(message) => {
            return Err(ApiError::internal(format!(
                "Queued user message '{}' belongs to chat '{}' instead of Agent team chat '{}'",
                task_input.queued_user_message_id, message.chat_id, team.chat_id
            )));
        }
        None => None,
    };

    Ok(match queued_run {
        // Legacy queuedRun.providerId is deliberately not carried into the request. The current
        // model route is resolved by prepare_prompt_context immediately before execution.
        Some(queued_run) => AgentTaskModelSelection {
            model_id: queued_run.model_id,
            thinking_level: queued_run.thinking_level,
        },
        None => AgentTaskModelSelection {
            model_id: instance.definition_snapshot.model_id.clone(),
            thinking_level: instance
                .definition_snapshot
                .model_options
                .thinking_level
                .clone(),
        },
    })
}

fn apply_agent_prompt_layers(
    workspace_path: &Path,
    chat_context: &mut PreparedChatContext,
    team: &AgentTeamRecord,
    instance: &AgentInstanceRecord,
    task: &AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    allowed_tools: &HashSet<String>,
    collaboration_tools_enabled: bool,
    collaboration_permissions: &AgentPermissions,
    agent_definitions: &[AgentDefinitionSettings],
) -> Result<(Vec<Value>, Vec<foco_agent::AgentMessageId>), ApiError> {
    validate_agent_definition_system_prompt(instance)?;

    let database = open_workspace_database_critical(workspace_path)?;
    let context_snapshot = database
        .latest_agent_context_snapshot(&instance.id, instance.context_generation)
        .map_err(ApiError::from_workspace_error)?;
    let after_context_sequence = context_snapshot
        .as_ref()
        .map(|snapshot| snapshot.sequence)
        .unwrap_or(-1);
    let context_entries = database
        .agent_context_entries(
            &instance.id,
            instance.context_generation,
            after_context_sequence,
        )
        .map_err(ApiError::from_workspace_error)?;
    let unread_messages = database
        .agent_messages_after(&instance.id, -1)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .filter(|message| message.consumed_at.is_none())
        .collect::<Vec<_>>();
    let team_instances = database
        .agent_instances_for_team(&team.id)
        .map_err(ApiError::from_workspace_error)?;
    let wait_dependencies = database
        .agent_task_dependencies(&task.id)
        .map_err(ApiError::from_workspace_error)?;
    let wait_dependency_tasks = wait_dependencies
        .iter()
        .map(|dependency| {
            database
                .agent_task_for_team(&dependency.team_id, &dependency.dependency_task_id)
                .map_err(ApiError::from_workspace_error)?
                .ok_or_else(|| {
                    ApiError::internal(format!(
                        "Agent dependency task '{}' was not found",
                        dependency.dependency_task_id
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    drop(database);

    let agent_prompt_role = agent_prompt_role(collaboration_tools_enabled);
    let definition_index = agent_definition_insert_index(chat_context);
    insert_agent_prompt_message(
        chat_context,
        definition_index,
        neutral_agent_message(
            agent_prompt_role.clone(),
            instance
                .definition_snapshot
                .system_prompt
                .trim()
                .to_string(),
        ),
        None,
        PromptContextSource::AgentDefinition,
    );

    let protocol_index = agent_team_protocol_insert_index(chat_context);
    insert_agent_prompt_message(
        chat_context,
        protocol_index,
        neutral_agent_message(
            agent_prompt_role,
            agent_team_protocol_prompt(
                team,
                instance,
                task,
                attempt_id,
                allowed_tools,
                collaboration_tools_enabled,
                collaboration_permissions,
                agent_definitions,
                &team_instances,
            )?,
        ),
        None,
        PromptContextSource::AgentTeamProtocol,
    );

    if let Some(private_context) =
        agent_private_context_prompt(context_snapshot.as_ref(), &context_entries)?
    {
        let index = chat_context.active_tool_start_index;
        insert_agent_prompt_message(
            chat_context,
            index,
            neutral_agent_message(NeutralChatRole::System, private_context),
            None,
            PromptContextSource::AgentPrivateContext,
        );
    }

    let current_task =
        agent_current_task_prompt(task, attempt_id, &wait_dependencies, &wait_dependency_tasks)?;
    let index = agent_current_task_insert_index(chat_context);
    insert_agent_prompt_message(
        chat_context,
        index,
        neutral_agent_message(NeutralChatRole::User, current_task),
        Some(task.sequence),
        PromptContextSource::AgentCurrentTask {
            sequence: task.sequence,
        },
    );

    for message in agent_wait_resume_messages(&wait_dependencies, &wait_dependency_tasks)? {
        let index = agent_current_task_insert_index(chat_context);
        insert_agent_prompt_message(
            chat_context,
            index,
            message,
            Some(task.sequence),
            PromptContextSource::AgentCurrentTask {
                sequence: task.sequence,
            },
        );
    }

    let mut run_unread_messages = Vec::with_capacity(unread_messages.len());
    let mut consumed_message_ids = Vec::with_capacity(unread_messages.len());
    for message in unread_messages {
        let payload = agent_message_payload(&message);
        let payload_json = serde_json::to_string_pretty(&payload).map_err(|source| {
            ApiError::internal(format!(
                "failed to serialize Agent message prompt: {source}"
            ))
        })?;
        let prompt = markdown_json_section("Agent Unread Message", &payload_json);
        let index = chat_context.active_tool_start_index;
        insert_agent_prompt_message(
            chat_context,
            index,
            neutral_agent_message(NeutralChatRole::User, prompt),
            None,
            PromptContextSource::AgentUnreadMessage,
        );
        consumed_message_ids.push(message.id.clone());
        run_unread_messages.push(payload);
    }

    Ok((run_unread_messages, consumed_message_ids))
}

fn validate_agent_definition_system_prompt(instance: &AgentInstanceRecord) -> Result<(), ApiError> {
    let system_prompt = instance.definition_snapshot.system_prompt.trim();
    if system_prompt.is_empty() {
        return Err(ApiError::bad_request(format!(
            "Agent definition snapshot '{}' has an empty system prompt",
            instance.definition_id
        )));
    }
    if system_prompt.chars().count() > AGENT_DEFINITION_SYSTEM_PROMPT_MAX_CHARS {
        return Err(ApiError::bad_request(format!(
            "Agent definition snapshot '{}' system prompt exceeds {AGENT_DEFINITION_SYSTEM_PROMPT_MAX_CHARS} characters",
            instance.definition_id
        )));
    }
    Ok(())
}

fn agent_prompt_role(collaboration_tools_enabled: bool) -> NeutralChatRole {
    if collaboration_tools_enabled {
        NeutralChatRole::Developer
    } else {
        NeutralChatRole::System
    }
}

fn agent_definition_insert_index(chat_context: &PreparedChatContext) -> usize {
    chat_context
        .message_context_sources
        .iter()
        .position(|source| !matches!(source, PromptContextSource::ReservedPrompt))
        .unwrap_or(chat_context.active_tool_start_index)
}

fn agent_team_protocol_insert_index(chat_context: &PreparedChatContext) -> usize {
    agent_team_protocol_insert_index_for_sources(
        &chat_context.message_context_sources,
        chat_context.active_tool_start_index,
    )
}

fn agent_team_protocol_insert_index_for_sources(
    message_context_sources: &[PromptContextSource],
    fallback_index: usize,
) -> usize {
    message_context_sources
        .iter()
        .position(|source| {
            !matches!(
                source,
                PromptContextSource::ReservedPrompt | PromptContextSource::AgentDefinition
            )
        })
        .unwrap_or(fallback_index)
}

fn agent_current_task_insert_index(chat_context: &PreparedChatContext) -> usize {
    agent_current_task_insert_index_for_sources(
        &chat_context.message_context_sources,
        chat_context.active_tool_start_index,
    )
}

fn agent_current_task_insert_index_for_sources(
    message_context_sources: &[PromptContextSource],
    fallback_index: usize,
) -> usize {
    message_context_sources
        .iter()
        .position(|source| matches!(source, PromptContextSource::CurrentUser { .. }))
        .unwrap_or(fallback_index)
}

fn insert_agent_prompt_message(
    chat_context: &mut PreparedChatContext,
    index: usize,
    message: NeutralChatMessage,
    source_sequence: Option<i64>,
    source: PromptContextSource,
) {
    chat_context
        .provider_request
        .messages
        .insert(index, message);
    chat_context
        .message_source_sequences
        .insert(index, source_sequence);
    chat_context.message_context_sources.insert(index, source);
    if index <= chat_context.active_tool_start_index {
        chat_context.active_tool_start_index += 1;
    }
    if let Some(pending) = &mut chat_context.pending_memory_retrieval {
        if index <= pending.stable_insert_index {
            pending.stable_insert_index += 1;
        }
        if index <= pending.turn_insert_index {
            pending.turn_insert_index += 1;
        }
    }
}

fn neutral_agent_message(role: NeutralChatRole, content: String) -> NeutralChatMessage {
    NeutralChatMessage {
        role,
        content,
        attachments: Vec::new(),
        reasoning: None,
        tool_calls: Vec::new(),
        tool_call_id: None,
        tool_name: None,
    }
}

pub(crate) fn retain_agent_snapshot_tools(
    tools: &mut Vec<foco_providers::NeutralToolDefinition>,
    allowed_tools: &HashSet<String>,
) {
    tools.retain(|tool| allowed_tools.contains(&tool.name));
}

fn sync_agent_tool_routing_prompt(
    messages: &mut Vec<NeutralChatMessage>,
    message_source_sequences: &mut Vec<Option<i64>>,
    message_context_sources: &mut Vec<PromptContextSource>,
    tools: &[NeutralToolDefinition],
) -> bool {
    let tool_infos = tools
        .iter()
        .map(|tool| ToolPromptInfo {
            name: tool.name.clone(),
        })
        .collect::<Vec<_>>();
    let routing_prompt = build_available_tools_prompt(&tool_infos);
    let routing_index = messages.iter().position(|message| {
        message.role == NeutralChatRole::System && message.content.starts_with("## Tool Routing")
    });

    match (routing_index, routing_prompt) {
        (Some(index), Some(prompt)) => {
            messages[index].content = prompt;
            false
        }
        (Some(index), None) => {
            messages.remove(index);
            message_source_sequences.remove(index);
            message_context_sources.remove(index);
            true
        }
        (None, _) => false,
    }
}

fn append_agent_collaboration_tools(
    chat_context: &mut PreparedChatContext,
    permissions: &AgentPermissions,
) {
    for definition in foco_tools::agent_tool_definitions() {
        let include = match definition.name {
            foco_tools::AGENT_LIST_TOOL
            | foco_tools::AGENT_GET_TASK_TOOL
            | foco_tools::AGENT_SEND_MESSAGE_TOOL => true,
            foco_tools::AGENT_DELEGATE_TASK_TOOL
            | foco_tools::AGENT_CANCEL_TASK_TOOL
            | foco_tools::AGENT_WAIT_TASKS_TOOL
            | foco_tools::AGENT_TRANSFER_TASK_TOOL => {
                permissions.collaboration_tool_allowed(AgentCollaborationTool::DelegateTask)
            }
            foco_tools::AGENT_CREATE_INSTANCES_TOOL => {
                permissions.collaboration_tool_allowed(AgentCollaborationTool::CreateInstance)
            }
            _ => false,
        };
        if include
            && !chat_context
                .provider_request
                .tools
                .iter()
                .any(|tool| tool.name == definition.name)
        {
            chat_context
                .provider_request
                .tools
                .push(neutral_tool_definition(definition));
        }
    }
}

fn agent_team_protocol_prompt(
    team: &AgentTeamRecord,
    instance: &AgentInstanceRecord,
    task: &AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    allowed_tools: &HashSet<String>,
    collaboration_tools_enabled: bool,
    collaboration_permissions: &AgentPermissions,
    agent_definitions: &[AgentDefinitionSettings],
    team_instances: &[AgentInstanceRecord],
) -> Result<String, ApiError> {
    let mut tools = allowed_tools.iter().cloned().collect::<Vec<_>>();
    tools.sort();
    let creatable_agent_definitions = creatable_agent_definitions_prompt(
        team,
        collaboration_permissions,
        agent_definitions,
        team_instances,
    )?;
    let protocol = json!({
        "version": AGENT_TEAM_PROTOCOL_VERSION,
        "teamId": team.id.to_string(),
        "chatId": team.chat_id,
        "instanceId": instance.id.to_string(),
        "definitionId": instance.definition_id.to_string(),
        "definitionRevision": instance.definition_revision,
        "role": instance.role.as_str(),
        "taskId": task.id.to_string(),
        "attemptId": attempt_id.to_string(),
        "contextGeneration": instance.context_generation,
        "executionWorkspace": {
            "mode": instance.execution_workspace_mode.as_str(),
            "rootPath": instance.execution_root_path,
            "baseRevision": instance.worktree_base_revision,
            "branch": instance.worktree_branch,
            "status": instance.worktree_status,
        },
        "permissions": collaboration_permissions,
        "creatableAgentDefinitions": creatable_agent_definitions,
        "allowedRuntimeTools": tools,
        "runtimeLimits": {
            "maxQueuedTasksPerTeam": AGENT_MAX_QUEUED_TASKS_PER_TEAM,
            "maxQueuedTasksPerInstance": AGENT_MAX_QUEUED_TASKS_PER_INSTANCE,
            "maxQueuedTasksPerChat": AGENT_MAX_QUEUED_TASKS_PER_CHAT,
            "maxInstancesPerTeam": AGENT_MAX_INSTANCES_PER_TEAM,
            "maxCreateInstancesPerRequest": AGENT_MAX_CREATE_INSTANCES_PER_REQUEST,
            "maxAgentToolRounds": MAX_AGENT_TOOL_ROUNDS,
        },
        "outputPolicy": {
            "coordinatorWritesMainChat": true,
            "workerWritesMainChat": false,
            "workerAutomaticMemoryExtraction": false,
        },
    });
    let protocol_json = serde_json::to_string_pretty(&protocol).map_err(|source| {
        ApiError::internal(format!("failed to serialize Agent team protocol: {source}"))
    })?;
    let protocol_section = markdown_json_section("Agent Team Protocol", &protocol_json);
    if collaboration_tools_enabled {
        Ok(format!(
            "{}\n{}",
            build_subagents_prompt_section(),
            protocol_section
        ))
    } else {
        Ok(protocol_section)
    }
}

fn creatable_agent_definitions_prompt(
    team: &AgentTeamRecord,
    permissions: &AgentPermissions,
    agent_definitions: &[AgentDefinitionSettings],
    team_instances: &[AgentInstanceRecord],
) -> Result<Vec<Value>, ApiError> {
    if !permissions.can_create_instances {
        return Ok(Vec::new());
    }

    let max_instances_per_team = u32::try_from(AGENT_MAX_INSTANCES_PER_TEAM)
        .map_err(|_| ApiError::internal("Agent max instances per team exceeds u32"))?;
    let current_team_instances = u32::try_from(
        team_instances
            .iter()
            .filter(|instance| instance.team_id == team.id)
            .count(),
    )
    .map_err(|_| ApiError::internal("Agent team instance count exceeds u32"))?;
    let remaining_team_slots = max_instances_per_team.saturating_sub(current_team_instances);
    let mut definitions = Vec::with_capacity(permissions.allowed_agent_definition_ids.len());

    for allowed_id in &permissions.allowed_agent_definition_ids {
        let Some(definition) =
            creatable_agent_definition(allowed_id, agent_definitions, team_instances)
        else {
            // ponytail: stale create permissions should shrink advertised options, not fail
            // the current task; config validation still rejects bad newly-saved definitions.
            continue;
        };
        let current_definition_instances = u32::try_from(
            team_instances
                .iter()
                .filter(|instance| {
                    instance.team_id == team.id && instance.definition_id == *allowed_id
                })
                .count(),
        )
        .map_err(|_| ApiError::internal("Agent definition instance count exceeds u32"))?;
        let remaining_definition_slots = definition
            .max_instances
            .saturating_sub(current_definition_instances);
        let max_create_count = AGENT_MAX_CREATE_INSTANCES_PER_REQUEST
            .min(remaining_team_slots)
            .min(remaining_definition_slots);
        let count_schema = if max_create_count == 0 {
            Value::Null
        } else {
            json!({
                "minimum": 1,
                "maximum": max_create_count,
            })
        };
        let allowed_execution_workspace_modes = definition
            .allowed_execution_workspace_modes
            .iter()
            .map(|mode| mode.as_str())
            .collect::<Vec<_>>();

        definitions.push(json!({
            "definitionId": definition.id.to_string(),
            "revision": definition.revision,
            "name": definition.name,
            "description": definition.description,
            "maxInstances": definition.max_instances,
            "currentTeamInstances": current_team_instances,
            "remainingTeamSlots": remaining_team_slots,
            "currentTeamDefinitionInstances": current_definition_instances,
            "remainingTeamDefinitionSlots": remaining_definition_slots,
            "maxCreateCount": max_create_count,
            "canCreateMore": max_create_count > 0,
            "allowedExecutionWorkspaceModes": allowed_execution_workspace_modes.clone(),
            "agentCreateInstancesSchema": {
                "tool": "agent_create_instances",
                "definitionId": { "const": definition.id.to_string() },
                "count": count_schema,
                "executionWorkspaceMode": { "enum": allowed_execution_workspace_modes },
                "timeoutMs": { "const": null },
            },
        }));
    }

    Ok(definitions)
}

fn creatable_agent_definition<'a>(
    allowed_id: &foco_agent::AgentDefinitionId,
    agent_definitions: &'a [AgentDefinitionSettings],
    team_instances: &'a [AgentInstanceRecord],
) -> Option<&'a AgentDefinitionSettings> {
    agent_definitions
        .iter()
        .find(|definition| definition.id == *allowed_id)
        .or_else(|| {
            team_instances
                .iter()
                .find(|instance| instance.definition_id == *allowed_id)
                .map(|instance| &instance.definition_snapshot)
        })
}

fn agent_private_context_prompt(
    snapshot: Option<&foco_store::workspace::AgentContextSnapshotRecord>,
    entries: &[AgentContextEntryRecord],
) -> Result<Option<String>, ApiError> {
    if snapshot.is_none() && entries.is_empty() {
        return Ok(None);
    }
    let recent_entries = entries
        .iter()
        .rev()
        .take(AGENT_CONTEXT_RECENT_MESSAGE_LIMIT)
        .map(agent_context_entry_prompt_value)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let snapshot_value = snapshot
        .map(|record| {
            serde_json::from_str::<Value>(&record.entries_json).map_err(|source| {
                ApiError::internal(format!("failed to parse Agent context snapshot: {source}"))
            })
        })
        .transpose()?;
    let context = json!({
        "snapshot": snapshot_value,
        "recentEntries": recent_entries,
    });
    let context_json = serde_json::to_string_pretty(&context).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize Agent private context: {source}"
        ))
    })?;
    Ok(Some(markdown_json_section(
        "Agent Private Context",
        &context_json,
    )))
}

fn agent_context_entry_prompt_value(entry: &AgentContextEntryRecord) -> Result<Value, ApiError> {
    let content = serde_json::from_str::<Value>(&entry.content_json).map_err(|source| {
        ApiError::internal(format!("failed to parse Agent context entry: {source}"))
    })?;
    Ok(json!({
        "id": entry.id,
        "sequence": entry.sequence,
        "role": entry.role,
        "sourceTaskId": entry.source_task_id.as_ref().map(ToString::to_string),
        "sourceMessageId": entry.source_message_id.as_ref().map(ToString::to_string),
        "createdAt": entry.created_at,
        "content": content,
    }))
}

fn agent_task_input_prompt_value(task: &AgentTaskRecord) -> Result<Value, ApiError> {
    let mut input = serde_json::from_str::<Value>(&task.input_json).map_err(|source| {
        ApiError::internal(format!("failed to parse Agent task input: {source}"))
    })?;
    if let Some(object) = input.as_object_mut() {
        object.remove("skillIds");
        object.remove("skill_ids");
        truncate_agent_task_input_message(object);
    }
    Ok(input)
}

fn truncate_agent_task_input_message(object: &mut serde_json::Map<String, Value>) {
    let Some(message) = object.get("message").and_then(Value::as_str) else {
        return;
    };
    let total_chars = message.chars().count();
    if total_chars <= AGENT_CURRENT_TASK_MESSAGE_PREVIEW_CHARS {
        return;
    }

    let preview = message
        .chars()
        .take(AGENT_CURRENT_TASK_MESSAGE_PREVIEW_CHARS)
        .collect::<String>();
    object.insert("messagePreview".to_string(), Value::String(preview));
    object.insert(
        "messageOmitted".to_string(),
        json!({
            "reason": "message is already present as the current user message; agent_current_task keeps only a preview to avoid duplicating large prompt content",
            "originalChars": total_chars,
            "previewChars": AGENT_CURRENT_TASK_MESSAGE_PREVIEW_CHARS,
        }),
    );
    object.remove("message");
}

fn agent_current_task_prompt(
    task: &AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    wait_dependencies: &[AgentTaskDependencyRecord],
    wait_dependency_tasks: &[AgentTaskRecord],
) -> Result<String, ApiError> {
    let input = agent_task_input_prompt_value(task)?;
    let mut current_task = json!({
        "taskId": task.id.to_string(),
        "teamId": task.team_id.to_string(),
        "ownerInstanceId": task.owner_instance_id.to_string(),
        "originInstanceId": task.origin_instance_id.as_ref().map(ToString::to_string),
        "parentTaskId": task.parent_task_id.as_ref().map(ToString::to_string),
        "attemptId": attempt_id.to_string(),
        "sequence": task.sequence,
        "status": task.status.as_str(),
        "input": input,
    });
    if task.result_json.is_some() || task.error_json.is_some() {
        current_task["previousAttempt"] = agent_previous_attempt_payload(task)?;
    }
    if !wait_dependencies.is_empty() {
        current_task["resume"] =
            agent_wait_resume_payload(wait_dependencies, wait_dependency_tasks)?;
    }
    let current_task_json = serde_json::to_string_pretty(&current_task).map_err(|source| {
        ApiError::internal(format!("failed to serialize Agent current task: {source}"))
    })?;
    Ok(markdown_json_section(
        "Agent Current Task",
        &current_task_json,
    ))
}

fn agent_previous_attempt_payload(task: &AgentTaskRecord) -> Result<Value, ApiError> {
    let result = task
        .result_json
        .as_deref()
        .map(|value| {
            serde_json::from_str::<Value>(value).map_err(|source| {
                ApiError::internal(format!(
                    "failed to parse Agent task previous result: {source}"
                ))
            })
        })
        .transpose()?;
    let error = task
        .error_json
        .as_deref()
        .map(|value| {
            serde_json::from_str::<Value>(value).map_err(|source| {
                ApiError::internal(format!(
                    "failed to parse Agent task previous error: {source}"
                ))
            })
        })
        .transpose()?;
    Ok(json!({
        "result": result,
        "error": error,
        "completedAt": task.completed_at,
    }))
}

fn agent_wait_resume_payload(
    dependencies: &[AgentTaskDependencyRecord],
    dependency_tasks: &[AgentTaskRecord],
) -> Result<Value, ApiError> {
    let pending_tool_call_id = dependencies
        .iter()
        .find_map(|dependency| dependency.pending_tool_call_id.clone());
    Ok(json!({
        "kind": "agent_wait_tasks",
        "pendingToolCallId": pending_tool_call_id,
        "toolResult": agent_wait_resume_tool_result(dependencies, dependency_tasks)?,
    }))
}

pub(crate) fn agent_wait_resume_messages(
    dependencies: &[AgentTaskDependencyRecord],
    dependency_tasks: &[AgentTaskRecord],
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    if dependencies.is_empty() {
        return Ok(Vec::new());
    }
    let pending_tool_call_id = dependencies
        .iter()
        .find_map(|dependency| dependency.pending_tool_call_id.clone())
        .ok_or_else(|| {
            ApiError::internal("Agent wait dependency is missing pending tool call id")
        })?;
    let mode = dependencies
        .first()
        .map(|dependency| dependency.wait_mode.as_str())
        .ok_or_else(|| ApiError::internal("Agent wait dependency list is empty"))?;
    let task_ids = dependencies
        .iter()
        .map(|dependency| dependency.dependency_task_id.to_string())
        .collect::<Vec<_>>();
    let tool_result = agent_wait_resume_tool_result(dependencies, dependency_tasks)?;
    let tool_result_content = serde_json::to_string(&tool_result).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize Agent wait tool result: {source}"
        ))
    })?;
    Ok(vec![
        neutral_agent_message(
            NeutralChatRole::System,
            AGENT_WAIT_RESUME_INSTRUCTION.to_string(),
        ),
        NeutralChatMessage {
            role: NeutralChatRole::Assistant,
            content: String::new(),
            attachments: Vec::new(),
            reasoning: None,
            tool_calls: vec![NeutralToolCall {
                call_id: pending_tool_call_id.clone(),
                name: foco_tools::AGENT_WAIT_TASKS_TOOL.to_string(),
                arguments: json!({
                    "taskIds": task_ids,
                    "mode": mode,
                    "deadlineMs": null,
                    "timeoutMs": null,
                }),
                thought_signatures: None,
            }],
            tool_call_id: None,
            tool_name: None,
        },
        NeutralChatMessage {
            role: NeutralChatRole::Tool,
            content: tool_result_content,
            attachments: Vec::new(),
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: Some(pending_tool_call_id),
            tool_name: Some(foco_tools::AGENT_WAIT_TASKS_TOOL.to_string()),
        },
    ])
}

fn agent_wait_resume_tool_result(
    dependencies: &[AgentTaskDependencyRecord],
    dependency_tasks: &[AgentTaskRecord],
) -> Result<Value, ApiError> {
    let deadline_at = dependencies
        .iter()
        .find_map(|dependency| dependency.deadline_at.clone());
    let dependency_values = dependencies
        .iter()
        .map(|dependency| {
            let task = dependency_tasks
                .iter()
                .find(|task| task.id == dependency.dependency_task_id)
                .ok_or_else(|| {
                    ApiError::internal(format!(
                        "Agent dependency task '{}' was not found",
                        dependency.dependency_task_id
                    ))
                })?;
            Ok(json!({
                "taskId": task.id.to_string(),
                "status": task.status.as_str(),
                "result": agent_optional_json(task.result_json.as_deref(), "Agent dependency task result")?,
                "error": agent_optional_json(task.error_json.as_deref(), "Agent dependency task error")?,
                "completedAt": task.completed_at,
            }))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(json!({
        "waiting": false,
        "mode": dependencies.first().map(|dependency| dependency.wait_mode.as_str()),
        "deadlineAt": deadline_at,
        "dependencies": dependency_values,
    }))
}

fn agent_optional_json(
    value: Option<&str>,
    label: &'static str,
) -> Result<Option<Value>, ApiError> {
    value
        .map(|value| {
            serde_json::from_str::<Value>(value)
                .map_err(|source| ApiError::internal(format!("failed to parse {label}: {source}")))
        })
        .transpose()
}

fn agent_message_payload(message: &AgentMessageRecord) -> Value {
    json!({
        "messageId": message.id.to_string(),
        "teamId": message.team_id.to_string(),
        "senderInstanceId": message.sender_instance_id.as_ref().map(ToString::to_string),
        "receiverInstanceId": message.receiver_instance_id.to_string(),
        "relatedTaskId": message.related_task_id.as_ref().map(ToString::to_string),
        "replyToMessageId": message.reply_to_message_id.as_ref().map(ToString::to_string),
        "kind": message.kind.as_str(),
        "content": message.content,
        "sequence": message.sequence,
        "createdAt": message.created_at,
    })
}

fn consume_agent_messages(
    workspace_path: &Path,
    message_ids: &[foco_agent::AgentMessageId],
) -> Result<(), ApiError> {
    if message_ids.is_empty() {
        return Ok(());
    }
    let mut database = open_workspace_database_critical(workspace_path)?;
    for message_id in message_ids {
        let message = database
            .agent_message(message_id)
            .map_err(ApiError::from_workspace_error)?
            .ok_or_else(|| {
                ApiError::internal(format!("Agent message '{message_id}' was not found"))
            })?;
        let consumed = database
            .mark_agent_message_consumed(message_id)
            .map_err(ApiError::from_workspace_error)?;
        if consumed {
            database
                .append_agent_event(NewAgentEvent {
                    team_id: &message.team_id,
                    event_type: "message_consumed",
                    instance_id: Some(&message.receiver_instance_id),
                    task_id: message.related_task_id.as_ref(),
                    attempt_id: None,
                    message_id: Some(&message.id),
                    payload_json: &json!({
                        "senderInstanceId": message.sender_instance_id.as_ref().map(ToString::to_string),
                        "receiverInstanceId": message.receiver_instance_id.to_string(),
                        "kind": message.kind.as_str(),
                    })
                    .to_string(),
                })
                .map_err(ApiError::from_workspace_error)?;
        }
    }
    Ok(())
}

fn persist_agent_task_context(
    workspace_path: &Path,
    task: &AgentTaskRecord,
    instance: &AgentInstanceRecord,
    attempt_id: &AgentAttemptId,
    outcome: &AgentRunOutcome,
) -> Result<(), ApiError> {
    let mut database = open_workspace_database_critical(workspace_path)?;
    let latest_snapshot = database
        .latest_agent_context_snapshot(&instance.id, instance.context_generation)
        .map_err(ApiError::from_workspace_error)?;
    let after_context_sequence = latest_snapshot
        .as_ref()
        .map(|snapshot| snapshot.sequence)
        .unwrap_or(-1);
    let context_entries = database
        .agent_context_entries(
            &instance.id,
            instance.context_generation,
            after_context_sequence,
        )
        .map_err(ApiError::from_workspace_error)?;
    let previous_sequence = context_entries
        .iter()
        .map(|entry| entry.sequence)
        .chain(latest_snapshot.as_ref().map(|snapshot| snapshot.sequence))
        .max()
        .unwrap_or(-1);
    let sequence = previous_sequence
        .checked_add(1)
        .ok_or_else(|| ApiError::internal("Agent private context sequence overflowed"))?;
    let content = agent_task_context_content(task, attempt_id, outcome);
    let content_json = content.to_string();
    let entry_id = unique_id("agent-context-entry");
    let role = agent_task_context_role(outcome);
    database
        .insert_agent_context_entry(NewAgentContextEntry {
            id: &entry_id,
            team_id: &task.team_id,
            instance_id: &instance.id,
            generation: instance.context_generation,
            sequence,
            role,
            content_json: &content_json,
            source_task_id: Some(&task.id),
            source_message_id: None,
        })
        .map_err(ApiError::from_workspace_error)?;

    let snapshot_entries = agent_context_snapshot_entries(&context_entries, sequence, &content)?;
    let snapshot_value = json!({
        "version": AGENT_CONTEXT_SNAPSHOT_VERSION,
        "teamProtocolVersion": AGENT_TEAM_PROTOCOL_VERSION,
        "buildVersion": "phase5",
        "teamId": task.team_id.to_string(),
        "instanceId": instance.id.to_string(),
        "generation": instance.context_generation,
        "taskId": task.id.to_string(),
        "attemptId": attempt_id.to_string(),
        "latestSequence": sequence,
        "previousSnapshotId": latest_snapshot.as_ref().map(|snapshot| snapshot.id.clone()),
        "entries": snapshot_entries,
    });
    let snapshot_json = snapshot_value.to_string();
    let token_count = i64::try_from(estimate_text_tokens(&snapshot_json)).map_err(|_| {
        ApiError::internal("Agent context snapshot token count exceeds SQLite integer range")
    })?;
    let snapshot_id = unique_id("agent-context-snapshot");
    database
        .insert_agent_context_snapshot(NewAgentContextSnapshot {
            id: &snapshot_id,
            team_id: &task.team_id,
            instance_id: &instance.id,
            generation: instance.context_generation,
            sequence,
            entries_json: &snapshot_json,
            token_count: Some(token_count),
        })
        .map_err(ApiError::from_workspace_error)?;
    Ok(())
}

fn agent_task_context_content(
    task: &AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    outcome: &AgentRunOutcome,
) -> Value {
    match outcome {
        AgentRunOutcome::Completed {
            text,
            reasoning,
            usage,
        } => json!({
            "status": "completed",
            "taskId": task.id.to_string(),
            "attemptId": attempt_id.to_string(),
            "summary": truncate_agent_context_text(text),
            "reasoningSummary": reasoning.as_ref().map(|value| truncate_agent_context_text(value)),
            "usage": usage,
        }),
        AgentRunOutcome::Failed { message, retryable } => json!({
            "status": "failed",
            "taskId": task.id.to_string(),
            "attemptId": attempt_id.to_string(),
            "message": truncate_agent_context_text(message),
            "retryable": retryable,
        }),
        AgentRunOutcome::Cancelled { message } => json!({
            "status": "cancelled",
            "taskId": task.id.to_string(),
            "attemptId": attempt_id.to_string(),
            "message": truncate_agent_context_text(message),
        }),
        AgentRunOutcome::Suspended { control } => json!({
            "status": "suspended",
            "taskId": task.id.to_string(),
            "attemptId": attempt_id.to_string(),
            "control": control,
        }),
    }
}

fn agent_task_context_role(outcome: &AgentRunOutcome) -> &'static str {
    match outcome {
        AgentRunOutcome::Completed { .. } | AgentRunOutcome::Suspended { .. } => "assistant",
        AgentRunOutcome::Failed { .. } | AgentRunOutcome::Cancelled { .. } => "system",
    }
}

fn agent_context_snapshot_entries(
    existing_entries: &[AgentContextEntryRecord],
    new_sequence: i64,
    new_content: &Value,
) -> Result<Vec<Value>, ApiError> {
    let keep_existing = AGENT_CONTEXT_SUMMARY_ENTRY_LIMIT.saturating_sub(1);
    let mut entries = existing_entries
        .iter()
        .rev()
        .take(keep_existing)
        .map(agent_context_snapshot_entry_value)
        .collect::<Result<Vec<_>, _>>()?;
    entries.reverse();
    entries.push(json!({
        "sequence": new_sequence,
        "content": new_content,
    }));
    Ok(entries)
}

fn agent_context_snapshot_entry_value(entry: &AgentContextEntryRecord) -> Result<Value, ApiError> {
    let content = serde_json::from_str::<Value>(&entry.content_json).map_err(|source| {
        ApiError::internal(format!("failed to parse Agent context entry: {source}"))
    })?;
    Ok(json!({
        "sequence": entry.sequence,
        "role": entry.role,
        "sourceTaskId": entry.source_task_id.as_ref().map(ToString::to_string),
        "sourceMessageId": entry.source_message_id.as_ref().map(ToString::to_string),
        "content": content,
    }))
}

fn truncate_agent_context_text(text: &str) -> String {
    if text.chars().count() <= AGENT_CONTEXT_SUMMARY_MAX_CHARS {
        return text.to_string();
    }
    text.chars()
        .take(AGENT_CONTEXT_SUMMARY_MAX_CHARS)
        .collect::<String>()
}

fn timestamp_delta_ms(start: Option<&str>, end: Option<&str>) -> Option<i64> {
    let start = chrono::DateTime::parse_from_rfc3339(start?).ok()?;
    let end = chrono::DateTime::parse_from_rfc3339(end?).ok()?;
    Some((end - start).num_milliseconds())
}

fn finish_claimed_task(
    workspace_path: &Path,
    task: &foco_store::workspace::AgentTaskRecord,
    attempt_id: &AgentAttemptId,
    outcome: AgentRunOutcome,
) -> Result<(), ApiError> {
    let instance = open_workspace_database_critical(workspace_path)?
        .agent_instance(&task.owner_instance_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::internal(format!(
                "Agent instance '{}' was not found",
                task.owner_instance_id
            ))
        })?;
    let (transition, result, error, event_type) = match outcome {
        AgentRunOutcome::Completed {
            text,
            reasoning,
            usage,
        } => {
            let mut result = json!({ "text": text, "reasoning": reasoning, "usage": usage });
            if let Some(worktree) = agent_task_worktree_result(workspace_path, &instance)? {
                result["worktree"] = worktree;
            }
            (
                AgentTaskTransition::Complete,
                Some(result),
                None,
                "task_completed",
            )
        }
        AgentRunOutcome::Failed { message, retryable } => (
            AgentTaskTransition::Fail,
            None,
            Some(json!({ "message": message, "retryable": retryable })),
            "task_failed",
        ),
        AgentRunOutcome::Cancelled { message } => (
            AgentTaskTransition::Cancel,
            None,
            Some(json!({ "message": message })),
            "task_cancelled",
        ),
        AgentRunOutcome::Suspended { control } => (
            AgentTaskTransition::Wait,
            Some(json!({ "control": control })),
            None,
            "task_suspended",
        ),
    };
    let result_json = result
        .as_ref()
        .map(|value| agent_task_outcome_json(value, "result_json"))
        .transpose()?;
    let error_json = error
        .as_ref()
        .map(|value| agent_task_outcome_json(value, "error_json"))
        .transpose()?;
    let mut database = open_workspace_database_critical(workspace_path)?;
    let updated = database
        .update_agent_task_state_for_attempt(
            AgentTaskStateUpdate {
                team_id: &task.team_id,
                task_id: &task.id,
                expected_status: AgentTaskStatus::Running,
                transition,
                result_json: result_json.as_deref(),
                error_json: error_json.as_deref(),
                interruption_reason: None,
            },
            attempt_id,
        )
        .map_err(ApiError::from_workspace_error)?;
    if !updated {
        return Err(ApiError::internal(format!(
            "Agent task '{}' changed state before its outcome was persisted",
            task.id
        )));
    }
    let completed_task = database
        .agent_task(&task.id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::internal(format!("Agent task '{}' was not found", task.id)))?;
    let payload = result.or(error).unwrap_or_else(|| json!({}));
    let payload = json!({
        "outcome": payload,
        "originInstanceId": task.origin_instance_id.as_ref().map(ToString::to_string),
        "parentTaskId": task.parent_task_id.as_ref().map(ToString::to_string),
        "runTimeMs": completed_task
            .completed_at
            .as_ref()
            .and_then(|completed_at| timestamp_delta_ms(task.started_at.as_deref(), Some(completed_at))),
    });
    insert_agent_event(
        &mut database,
        &task.team_id,
        event_type,
        Some(&task.owner_instance_id),
        Some(&task.id),
        Some(attempt_id),
        payload,
    )?;
    crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task_with_database(
        &mut database,
        &task.id,
    )?;
    Ok(())
}

fn agent_instance_execution_root(workspace_path: &Path, instance: &AgentInstanceRecord) -> PathBuf {
    instance
        .execution_root_path
        .as_deref()
        .map(|root_path| resolve_agent_worktree_path(workspace_path, root_path))
        .unwrap_or_else(|| agent_instance_worktree_path(workspace_path, &instance.id))
}

fn agent_task_worktree_result(
    workspace_path: &Path,
    instance: &AgentInstanceRecord,
) -> Result<Option<Value>, ApiError> {
    if instance.execution_workspace_mode != AgentExecutionWorkspaceMode::IsolatedWorktree {
        return Ok(None);
    }
    let root_path = agent_instance_execution_root(workspace_path, instance);
    let diff = git_diff_response(&root_path, None)?;
    Ok(Some(json!({
        "mode": instance.execution_workspace_mode.as_str(),
        "rootPath": root_path.display().to_string(),
        "baseRevision": instance.worktree_base_revision,
        "branch": instance.worktree_branch,
        "status": instance.worktree_status,
        "diffId": agent_worktree_diff_id(&diff),
        "changedPaths": diff
            .files
            .iter()
            .chain(diff.staged_files.iter())
            .map(|file| file.path.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    })))
}

fn fail_claimed_task(
    workspace_path: &Path,
    task_id: &AgentTaskId,
    expected_attempt_id: Option<&AgentAttemptId>,
    message: &str,
) -> Result<(), ApiError> {
    let mut database = open_workspace_database_critical(workspace_path)?;
    let Some(task) = database
        .agent_task(task_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(());
    };
    if task.status != AgentTaskStatus::Running {
        return Ok(());
    }
    let mut error = json!({ "message": message });
    let mut error_json = error.to_string();
    if error_json.len() > AGENT_MAX_TASK_OUTCOME_BYTES {
        error = json!({
            "message": format!(
                "Agent task error_json exceeds {AGENT_MAX_TASK_OUTCOME_BYTES} bytes"
            )
        });
        error_json = error.to_string();
    }
    let update = AgentTaskStateUpdate {
        team_id: &task.team_id,
        task_id: &task.id,
        expected_status: AgentTaskStatus::Running,
        transition: AgentTaskTransition::Fail,
        result_json: None,
        error_json: Some(&error_json),
        interruption_reason: None,
    };
    let updated = match expected_attempt_id {
        Some(attempt_id) => database.update_agent_task_state_for_attempt(update, attempt_id),
        None => database.update_agent_task_state(update),
    }
    .map_err(ApiError::from_workspace_error)?;
    if !updated {
        return Ok(());
    }
    insert_agent_event(
        &mut database,
        &task.team_id,
        "task_failed",
        Some(&task.owner_instance_id),
        Some(&task.id),
        expected_attempt_id,
        json!({
            "outcome": error,
            "recoveryReason": "coordinator_lifecycle_closure",
        }),
    )?;
    database
        .fail_plan_phase_run(&task.id, message)
        .map_err(ApiError::from_workspace_error)?;
    crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task_with_database(
        &mut database,
        &task.id,
    )?;
    Ok(())
}

fn agent_task_outcome_json(value: &Value, field: &'static str) -> Result<String, ApiError> {
    let json = value.to_string();
    if json.len() <= AGENT_MAX_TASK_OUTCOME_BYTES {
        return Ok(json);
    }
    if field == "result_json" {
        return compact_agent_task_result_json(value, json.len());
    }
    Err(ApiError::internal(format!(
        "Agent task {field} exceeds {AGENT_MAX_TASK_OUTCOME_BYTES} bytes"
    )))
}

fn compact_agent_task_result_json(
    value: &Value,
    original_bytes: usize,
) -> Result<String, ApiError> {
    let mut compacted = value.clone();
    let Some(object) = compacted.as_object_mut() else {
        return Err(ApiError::internal(format!(
            "Agent task result_json exceeds {AGENT_MAX_TASK_OUTCOME_BYTES} bytes"
        )));
    };
    let mut truncated_fields = Vec::new();

    if object
        .get("reasoning")
        .and_then(Value::as_str)
        .is_some_and(|reasoning| !reasoning.is_empty())
    {
        object.insert("reasoning".to_string(), Value::String(String::new()));
        truncated_fields.push(Value::String("reasoning".to_string()));
    }
    mark_agent_task_result_truncated(object, original_bytes, &truncated_fields);
    if let Some(json) = agent_task_json_if_within_limit(&compacted) {
        return Ok(json);
    }

    if truncate_agent_task_string_field_to_fit(&mut compacted, "text", &mut truncated_fields) {
        if let Some(object) = compacted.as_object_mut() {
            mark_agent_task_result_truncated(object, original_bytes, &truncated_fields);
        }
        if let Some(json) = agent_task_json_if_within_limit(&compacted) {
            return Ok(json);
        }
    }

    compact_agent_task_worktree(&mut compacted, &mut truncated_fields);
    if let Some(object) = compacted.as_object_mut() {
        mark_agent_task_result_truncated(object, original_bytes, &truncated_fields);
    }
    if let Some(json) = agent_task_json_if_within_limit(&compacted) {
        return Ok(json);
    }

    let fallback = json!({
        "text": "",
        "reasoning": "",
        "usage": value.get("usage").cloned().unwrap_or(Value::Null),
        "worktree": compacted.get("worktree").cloned().unwrap_or(Value::Null),
        "truncated": true,
        "truncatedFields": ["reasoning", "text", "worktree"],
        "originalBytes": original_bytes,
    });
    agent_task_json_if_within_limit(&fallback).ok_or_else(|| {
        ApiError::internal(format!(
            "Agent task result_json exceeds {AGENT_MAX_TASK_OUTCOME_BYTES} bytes after compaction"
        ))
    })
}

fn mark_agent_task_result_truncated(
    object: &mut serde_json::Map<String, Value>,
    original_bytes: usize,
    truncated_fields: &[Value],
) {
    object.insert("truncated".to_string(), Value::Bool(true));
    object.insert("originalBytes".to_string(), json!(original_bytes));
    object.insert(
        "truncatedFields".to_string(),
        Value::Array(truncated_fields.to_vec()),
    );
}

fn agent_task_json_if_within_limit(value: &Value) -> Option<String> {
    let json = value.to_string();
    (json.len() <= AGENT_MAX_TASK_OUTCOME_BYTES).then_some(json)
}

fn truncate_agent_task_string_field_to_fit(
    value: &mut Value,
    field: &'static str,
    truncated_fields: &mut Vec<Value>,
) -> bool {
    let Some(original) = value.get(field).and_then(Value::as_str).map(str::to_string) else {
        return false;
    };
    if original.is_empty() {
        return false;
    }

    truncated_fields.push(Value::String(field.to_string()));
    if let Some(fields) = value
        .get_mut("truncatedFields")
        .and_then(Value::as_array_mut)
    {
        fields.push(Value::String(field.to_string()));
    }

    let mut low = 0usize;
    let mut high = original.len();
    let mut best = String::new();
    while low <= high {
        let mid = low + (high - low) / 2;
        let candidate = truncated_agent_task_text(&original, mid);
        if let Some(object) = value.as_object_mut() {
            object.insert(field.to_string(), Value::String(candidate.clone()));
        }
        if agent_task_json_if_within_limit(value).is_some() {
            best = candidate;
            low = mid.saturating_add(1);
        } else if mid == 0 {
            break;
        } else {
            high = mid - 1;
        }
    }
    if let Some(object) = value.as_object_mut() {
        object.insert(field.to_string(), Value::String(best));
    }
    true
}

fn truncated_agent_task_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n\n[truncated to fit agent task result_json]",
        &value[..end]
    )
}

fn compact_agent_task_worktree(value: &mut Value, truncated_fields: &mut Vec<Value>) {
    let Some(worktree) = value.get_mut("worktree").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(changed_paths) = worktree.get("changedPaths").and_then(Value::as_array) else {
        return;
    };
    let original_count = changed_paths.len();
    if original_count <= 32 {
        return;
    }
    let kept = changed_paths.iter().take(32).cloned().collect::<Vec<_>>();
    worktree.insert("changedPaths".to_string(), Value::Array(kept));
    worktree.insert("changedPathsTruncated".to_string(), Value::Bool(true));
    worktree.insert(
        "originalChangedPathCount".to_string(),
        json!(original_count),
    );
    truncated_fields.push(Value::String("worktree.changedPaths".to_string()));
}

pub(crate) fn validate_agent_snapshot_for_workspace(
    config: &GlobalConfig,
    workspace: &WorkspaceConfig,
    definition: &AgentDefinitionSettings,
) -> Result<(), ApiError> {
    if !workspace.path.is_absolute() || !workspace.path.is_dir() {
        return Err(ApiError::bad_request(format!(
            "Agent workspace is no longer a valid directory: {}",
            workspace.path.display()
        )));
    }
    let (model, _) = config
        .resolve_active_model_provider(&definition.model_id)
        .map_err(|error| {
            ApiError::bad_request(format!(
                "Agent definition snapshot model route is unavailable: {error}"
            ))
        })?;
    let limits = model.limits.as_ref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "Agent definition snapshot model '{}' is missing limits",
            definition.model_id
        ))
    })?;
    if definition
        .model_options
        .max_output_tokens
        .is_some_and(|value| u64::from(value) > limits.max_output_tokens)
    {
        return Err(ApiError::bad_request(format!(
            "Agent definition snapshot max output tokens exceed model '{}' limits",
            definition.model_id
        )));
    }
    Ok(())
}

pub(crate) fn insert_agent_event(
    database: &mut WorkspaceDatabase,
    team_id: &foco_agent::AgentTeamId,
    event_type: &str,
    instance_id: Option<&foco_agent::AgentInstanceId>,
    task_id: Option<&AgentTaskId>,
    attempt_id: Option<&AgentAttemptId>,
    payload: Value,
) -> Result<(), ApiError> {
    let payload_json = payload.to_string();
    database
        .append_agent_event(NewAgentEvent {
            team_id,
            event_type,
            instance_id,
            task_id,
            attempt_id,
            message_id: None,
            payload_json: &payload_json,
        })
        .map_err(ApiError::from_workspace_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn wake_signal_is_coalesced_without_blocking() {
        let (scheduler, mut receiver) = AgentScheduler::new();
        scheduler.wake().expect("first wake");
        scheduler.wake().expect("coalesced wake");
        assert_eq!(receiver.recv().await, Some(()));
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn global_run_permits_enforce_the_process_limit() {
        let permits = Arc::new(Semaphore::new(AGENT_GLOBAL_MAX_CONCURRENT_RUNS));
        let held = (0..AGENT_GLOBAL_MAX_CONCURRENT_RUNS)
            .map(|_| permits.clone().try_acquire_owned().expect("run permit"))
            .collect::<Vec<_>>();
        assert!(permits.clone().try_acquire_owned().is_err());
        drop(held);
        assert!(permits.try_acquire_owned().is_ok());
    }

    #[tokio::test]
    async fn scheduler_scan_skips_remote_workspace_without_local_path() {
        let workspace = tempfile::tempdir().expect("workspace");
        let profile = tempfile::tempdir().expect("profile");
        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config.workspaces.insert(
            0,
            foco_store::config::WorkspaceConfig {
                id: "remote".to_string(),
                name: "Remote".to_string(),
                path: PathBuf::new(),
                location: foco_store::config::WorkspaceLocation::Ssh {
                    server_id: "server".to_string(),
                    remote_path: "/srv/project".to_string(),
                },
                pinned: false,
                terminal_shell: "bash".to_string(),
                common_commands: Vec::new(),
            },
        );
        let state = crate::tests::test_app_state(config, profile.path().to_path_buf());
        let permits = Arc::new(Semaphore::new(AGENT_GLOBAL_MAX_CONCURRENT_RUNS));
        let mut runs = JoinSet::new();
        let mut run_identities = HashMap::new();

        schedule_runnable_tasks(
            &state,
            &permits,
            &mut runs,
            &mut run_identities,
            "agent-owner-test-scheduler",
        )
        .await
        .expect("remote workspace should not abort the local scheduler scan");

        assert!(runs.is_empty());
    }

    #[tokio::test]
    async fn coordinator_exit_capture_turns_panic_into_recoverable_result() {
        let exit = capture_agent_coordinator_exit(async {
            panic!("synthetic Coordinator panic");
        })
        .await;

        match exit {
            AgentCoordinatorRunExit::Panicked(message) => {
                assert_eq!(message, "synthetic Coordinator panic");
            }
            AgentCoordinatorRunExit::Finished => panic!("panic must not look like normal finish"),
        }
    }

    #[tokio::test]
    async fn agent_lifecycle_database_retry_outlives_short_retry_budget() {
        let workspace = tempfile::tempdir().expect("workspace");
        let task_id = AgentTaskId::new("agent-task-durable-retry").expect("task id");
        let attempt_id = AgentAttemptId::new("agent-attempt-durable-retry").expect("attempt id");
        let context = AgentLifecycleOperationContext {
            workspace_id: "workspace-durable-retry",
            workspace_path: workspace.path(),
            task_id: &task_id,
            attempt_id: Some(&attempt_id),
        };
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let mut attempts = 0;

        let value = retry_agent_lifecycle_database_operation(
            "test durable retry",
            &context,
            shutdown_rx,
            || {
                attempts += 1;
                if attempts <= AGENT_TASK_DB_SHORT_RETRY_ATTEMPTS + 2 {
                    return Err(ApiError::internal(
                        "workspace database concurrency limit reached: synthetic pressure",
                    ));
                }
                Ok("persisted")
            },
        )
        .await
        .expect("durable retry succeeds after the short retry budget");

        assert_eq!(value, "persisted");
        assert_eq!(attempts, AGENT_TASK_DB_SHORT_RETRY_ATTEMPTS + 3);
    }

    #[tokio::test]
    async fn agent_lifecycle_database_retry_stops_for_shutdown() {
        let workspace = tempfile::tempdir().expect("workspace");
        let task_id = AgentTaskId::new("agent-task-retry-shutdown").expect("task id");
        let attempt_id = AgentAttemptId::new("agent-attempt-retry-shutdown").expect("attempt id");
        let context = AgentLifecycleOperationContext {
            workspace_id: "workspace-retry-shutdown",
            workspace_path: workspace.path(),
            task_id: &task_id,
            attempt_id: Some(&attempt_id),
        };
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        shutdown_tx.send(true).expect("request shutdown");

        let error = retry_agent_lifecycle_database_operation::<(), _>(
            "test shutdown-aware retry",
            &context,
            shutdown_rx,
            || {
                Err(ApiError::internal(
                    "workspace database concurrency limit reached: synthetic pressure",
                ))
            },
        )
        .await
        .expect_err("shutdown stops durable retry");

        assert!(error.message.contains(SHUTDOWN_MESSAGE));
    }

    #[test]
    fn agent_scheduler_deadline_delay_has_idle_and_past_deadline_bounds() {
        let past = Utc::now() - chrono::Duration::seconds(1);

        assert_eq!(
            agent_scheduler_deadline_delay(None),
            Duration::from_secs(86_400)
        );
        assert_eq!(
            agent_scheduler_deadline_delay(Some(&past)),
            Duration::from_millis(AGENT_SCHEDULER_MIN_DEADLINE_DELAY_MS)
        );
    }

    #[test]
    fn agent_task_outcome_json_compacts_large_result_reasoning_first() {
        let text = "phase completed";
        let oversized = json!({
            "text": text,
            "reasoning": "r".repeat(AGENT_MAX_TASK_OUTCOME_BYTES),
            "usage": { "totalTokens": 1 },
        });

        let compacted =
            agent_task_outcome_json(&oversized, "result_json").expect("compacted result");
        assert!(compacted.len() <= AGENT_MAX_TASK_OUTCOME_BYTES);
        let value = serde_json::from_str::<Value>(&compacted).expect("valid json");
        assert_eq!(value["text"], json!(text));
        assert_eq!(value["reasoning"], json!(""));
        assert_eq!(value["truncated"], json!(true));
        assert_eq!(value["truncatedFields"], json!(["reasoning"]));
    }

    #[test]
    fn agent_task_outcome_json_truncates_text_if_reasoning_is_not_enough() {
        let oversized = json!({
            "text": "x".repeat(AGENT_MAX_TASK_OUTCOME_BYTES),
            "reasoning": "r".repeat(AGENT_MAX_TASK_OUTCOME_BYTES),
            "usage": { "totalTokens": 1 },
        });

        let compacted =
            agent_task_outcome_json(&oversized, "result_json").expect("compacted result");
        assert!(compacted.len() <= AGENT_MAX_TASK_OUTCOME_BYTES);
        let value = serde_json::from_str::<Value>(&compacted).expect("valid json");
        assert_eq!(value["reasoning"], json!(""));
        assert_eq!(value["truncated"], json!(true));
        assert_eq!(value["truncatedFields"], json!(["reasoning", "text"]));
        assert!(
            value["text"]
                .as_str()
                .expect("text")
                .contains("[truncated to fit agent task result_json]")
        );
    }

    #[test]
    fn agent_task_outcome_json_still_rejects_oversized_error_payload() {
        let oversized = json!({ "message": "x".repeat(AGENT_MAX_TASK_OUTCOME_BYTES) });
        assert!(agent_task_outcome_json(&oversized, "error_json").is_err());
    }

    #[test]
    fn fail_claimed_task_fails_linked_plan_phase() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        database
            .create_plan(foco_store::workspace::NewPlan {
                id: "plan-fail-claimed-task",
                title: "Fail claimed task",
                overview: "A scheduler persistence error must not leave a Plan phase running.",
                status: "ready",
                source_chat_id: None,
                phases: vec![foco_store::workspace::NewPlanPhase {
                    id: "plan-fail-claimed-task-phase",
                    title: "Phase one",
                    summary: "Attached task fails outside the normal outcome path.",
                    steps: vec![foco_store::workspace::NewPlanStep {
                        id: "plan-fail-claimed-task-step",
                        title: "Do work",
                        detail: "Fail cleanly.",
                        acceptance: vec!["phase is failed".to_string()],
                    }],
                }],
            })
            .expect("create plan");
        database
            .transition_plan("plan-fail-claimed-task", "start")
            .expect("start plan");
        database
            .insert_chat("chat-fail-claimed-task", "Fail claimed task")
            .expect("insert chat");
        let team_id =
            foco_agent::AgentTeamId::new("agent-team-fail-claimed-task").expect("team id");
        let instance_id = foco_agent::AgentInstanceId::new("agent-instance-fail-claimed-task")
            .expect("instance id");
        let definition = foco_store::config::AgentDefinitionSettings {
            id: foco_agent::AgentDefinitionId::new("agent-definition-fail-claimed-task")
                .expect("definition id"),
            revision: 1,
            name: "Fail claimed task".to_string(),
            description: String::new(),
            provider_id: "provider-test".to_string(),
            model_id: "model-test".to_string(),
            model_options: foco_store::config::AgentModelOptions::default(),
            system_prompt: "Be precise.".to_string(),
            allowed_tools: Vec::new(),
            max_instances: 1,
            allowed_execution_workspace_modes: AgentExecutionWorkspaceMode::all(),
            permissions: AgentPermissions::default(),
        };
        database
            .create_agent_team(foco_store::workspace::NewAgentTeam {
                id: &team_id,
                chat_id: "chat-fail-claimed-task",
                coordinator_instance_id: &instance_id,
                coordinator_definition: &definition,
                coordinator_execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
                coordinator_execution_root_path: None,
                coordinator_worktree_base_revision: None,
                coordinator_worktree_branch: None,
                coordinator_worktree_status: None,
                max_concurrent_runs: 1,
            })
            .expect("create team");
        let task_id = AgentTaskId::new("agent-task-fail-claimed-task").expect("task id");
        database
            .enqueue_agent_task(foco_store::workspace::NewAgentTask {
                id: &task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue task");
        let attempt = database
            .begin_plan_phase_attempt(
                "plan-fail-claimed-task",
                "plan-fail-claimed-task-phase",
                foco_store::workspace::PlanPhaseAttemptTrigger::Initial,
                Some("provider-test"),
                Some("model-test"),
                None,
            )
            .expect("begin plan attempt");
        database
            .attach_plan_phase_attempt_run(
                &attempt.id,
                "chat-fail-claimed-task",
                &team_id,
                &task_id,
            )
            .expect("attach task");
        database
            .claim_runnable_agent_task(
                &team_id,
                &task_id,
                &AgentAttemptId::new("agent-attempt-fail-claimed-task").expect("attempt id"),
            )
            .expect("claim task")
            .expect("claimed");
        drop(database);

        fail_claimed_task(
            workspace.path(),
            &task_id,
            None,
            "Agent task result_json exceeds 65536 bytes",
        )
        .expect("fail claimed task");

        let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        let plan = database
            .plan("plan-fail-claimed-task")
            .expect("plan")
            .expect("plan");
        assert_eq!(plan.status, "failed");
        assert_eq!(plan.phases[0].status, "failed");
        assert_eq!(plan.phases[0].steps[0].status, "failed");
        assert_eq!(plan.phases[0].attempts[0].status, "failed");
        let failure_event = database
            .agent_events_after(&team_id, -1)
            .expect("events")
            .into_iter()
            .find(|event| event.event_type == "task_failed")
            .expect("task failed event");
        assert_eq!(failure_event.task_id.as_ref(), Some(&task_id));
        assert!(
            failure_event
                .payload_json
                .contains("coordinator_lifecycle_closure")
        );
    }

    #[test]
    fn agent_task_input_prompt_value_removes_skill_ids() {
        let now = "2026-01-01T00:00:00Z".to_string();
        let task = AgentTaskRecord {
            id: AgentTaskId::new("agent-task-skill-ids").expect("task id"),
            team_id: foco_agent::AgentTeamId::new("agent-team-skill-ids").expect("team id"),
            owner_instance_id: foco_agent::AgentInstanceId::new("agent-instance-skill-ids")
                .expect("instance id"),
            origin_instance_id: None,
            parent_task_id: None,
            sequence: 0,
            status: AgentTaskStatus::Running,
            input_json: r#"{"message":"work","skillIds":[],"skill_ids":["legacy"]}"#.to_string(),
            result_json: None,
            error_json: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            started_at: Some(now),
            completed_at: None,
        };

        let input = agent_task_input_prompt_value(&task).expect("prompt input");

        assert_eq!(input["message"], json!("work"));
        assert!(input.get("skillIds").is_none());
        assert!(input.get("skill_ids").is_none());
    }

    #[test]
    fn agent_task_input_prompt_value_truncates_large_message_copy() {
        let now = "2026-01-01T00:00:00Z".to_string();
        let large_message = "好".repeat(AGENT_CURRENT_TASK_MESSAGE_PREVIEW_CHARS + 8);
        let task = AgentTaskRecord {
            id: AgentTaskId::new("agent-task-large-message").expect("task id"),
            team_id: foco_agent::AgentTeamId::new("agent-team-large-message").expect("team id"),
            owner_instance_id: foco_agent::AgentInstanceId::new("agent-instance-large-message")
                .expect("instance id"),
            origin_instance_id: None,
            parent_task_id: None,
            sequence: 0,
            status: AgentTaskStatus::Running,
            input_json: json!({ "message": large_message, "queuedUserMessageId": "msg-1" })
                .to_string(),
            result_json: None,
            error_json: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            started_at: Some(now),
            completed_at: None,
        };

        let input = agent_task_input_prompt_value(&task).expect("prompt input");

        assert!(input.get("message").is_none());
        assert_eq!(
            input["messagePreview"]
                .as_str()
                .expect("message preview")
                .chars()
                .count(),
            AGENT_CURRENT_TASK_MESSAGE_PREVIEW_CHARS
        );
        assert_eq!(
            input["messageOmitted"]["originalChars"],
            json!(AGENT_CURRENT_TASK_MESSAGE_PREVIEW_CHARS + 8)
        );
        assert_eq!(input["queuedUserMessageId"], json!("msg-1"));
    }

    #[test]
    fn agent_definition_and_protocol_role_uses_developer_only_for_collaboration_tools() {
        let collaboration_role = agent_prompt_role(true);
        let definition_message =
            neutral_agent_message(collaboration_role.clone(), "definition".to_string());
        let protocol_message = neutral_agent_message(collaboration_role, "protocol".to_string());

        assert_eq!(definition_message.role, NeutralChatRole::Developer);
        assert_eq!(protocol_message.role, NeutralChatRole::Developer);
        assert_eq!(agent_prompt_role(false), NeutralChatRole::System);
    }

    #[test]
    fn agent_current_task_inserts_before_current_user_and_preserves_order() {
        let mut sources = vec![
            PromptContextSource::ReservedPrompt,
            PromptContextSource::AgentDefinition,
            PromptContextSource::CurrentUser { sequence: 7 },
        ];
        let first_index = agent_current_task_insert_index_for_sources(&sources, sources.len());
        sources.insert(
            first_index,
            PromptContextSource::AgentCurrentTask { sequence: 0 },
        );
        let second_index = agent_current_task_insert_index_for_sources(&sources, sources.len());
        sources.insert(
            second_index,
            PromptContextSource::AgentCurrentTask { sequence: 0 },
        );

        assert_eq!(first_index, 2);
        assert_eq!(second_index, 3);
        assert!(matches!(
            sources.last(),
            Some(PromptContextSource::CurrentUser { sequence: 7 })
        ));
    }

    #[test]
    fn first_turn_agent_sources_keep_current_user_last() {
        let mut sources = vec![
            PromptContextSource::ReservedPrompt,
            PromptContextSource::CurrentUser { sequence: 7 },
        ];
        let definition_index = sources
            .iter()
            .position(|source| !matches!(source, PromptContextSource::ReservedPrompt))
            .unwrap_or(sources.len());
        sources.insert(definition_index, PromptContextSource::AgentDefinition);
        let protocol_index = agent_team_protocol_insert_index_for_sources(&sources, sources.len());
        sources.insert(protocol_index, PromptContextSource::AgentTeamProtocol);
        let current_task_index =
            agent_current_task_insert_index_for_sources(&sources, sources.len());
        sources.insert(
            current_task_index,
            PromptContextSource::AgentCurrentTask { sequence: 0 },
        );

        assert_eq!(
            sources,
            vec![
                PromptContextSource::ReservedPrompt,
                PromptContextSource::AgentDefinition,
                PromptContextSource::AgentTeamProtocol,
                PromptContextSource::AgentCurrentTask { sequence: 0 },
                PromptContextSource::CurrentUser { sequence: 7 },
            ]
        );
    }

    #[test]
    fn agent_current_task_insert_index_falls_back_without_current_user() {
        let sources = vec![
            PromptContextSource::ReservedPrompt,
            PromptContextSource::AgentDefinition,
            PromptContextSource::StableInjection,
        ];

        assert_eq!(agent_current_task_insert_index_for_sources(&sources, 9), 9);
    }

    #[test]
    fn team_protocol_omits_subagents_when_collaboration_tools_are_disabled() {
        let definition = test_agent_definition("solo", 1);
        let team_id = foco_agent::AgentTeamId::new("agent-team-solo").expect("team id");
        let instance_id =
            foco_agent::AgentInstanceId::new("agent-instance-solo").expect("instance id");
        let task_id = AgentTaskId::new("agent-task-solo").expect("task id");
        let attempt_id = AgentAttemptId::new("agent-attempt-solo").expect("attempt id");
        let now = "2026-01-01T00:00:00Z".to_string();
        let team = AgentTeamRecord {
            id: team_id.clone(),
            chat_id: "chat-solo".to_string(),
            coordinator_instance_id: instance_id.clone(),
            status: foco_agent::AgentTeamStatus::Active,
            max_concurrent_runs: 1,
            next_event_sequence: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let instance = test_agent_instance(
            &team_id,
            &instance_id,
            definition.clone(),
            AgentRole::Coordinator,
            &now,
        );
        let task = AgentTaskRecord {
            id: task_id,
            team_id: team_id.clone(),
            owner_instance_id: instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            sequence: 0,
            status: AgentTaskStatus::Running,
            input_json: "{}".to_string(),
            result_json: None,
            error_json: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            started_at: Some(now),
            completed_at: None,
        };

        let prompt = agent_team_protocol_prompt(
            &team,
            &instance,
            &task,
            &attempt_id,
            &HashSet::new(),
            false,
            &AgentPermissions::default(),
            &[definition],
            &[instance.clone()],
        )
        .expect("protocol prompt");

        assert!(!prompt.contains("## Subagents"));
        assert!(prompt.starts_with("## Agent Team Protocol"));
    }

    fn agent_team_protocol_json_from_prompt(prompt: &str) -> Value {
        assert!(prompt.contains("## Subagents"));
        let protocol_prompt = prompt
            .split_once("## Agent Team Protocol")
            .map(|(_, rest)| format!("## Agent Team Protocol{rest}"))
            .expect("protocol section");
        assert!(protocol_prompt.starts_with("## Agent Team Protocol\n\n```json\n"));
        assert!(protocol_prompt.ends_with("\n```"));
        let json_text = protocol_prompt
            .strip_prefix("## Agent Team Protocol\n\n```json\n")
            .expect("protocol prefix")
            .strip_suffix("\n```")
            .expect("protocol suffix");
        serde_json::from_str(json_text).expect("protocol json")
    }

    #[test]
    fn agent_team_protocol_inserts_before_stable_context() {
        let sources = vec![
            PromptContextSource::ReservedPrompt,
            PromptContextSource::AgentDefinition,
            PromptContextSource::StableInjection,
            PromptContextSource::ProjectSpec,
            PromptContextSource::CurrentUser { sequence: 7 },
        ];

        assert_eq!(agent_team_protocol_insert_index_for_sources(&sources, 5), 2);
    }

    #[test]
    fn team_protocol_expands_creatable_definition_schema() {
        let coordinator_definition = test_agent_definition("coordinator", 1);
        let mut worker_definition = test_agent_definition("worker", 3);
        worker_definition.allowed_execution_workspace_modes =
            vec![AgentExecutionWorkspaceMode::Shared];
        let team_id = foco_agent::AgentTeamId::new("agent-team-protocol").expect("team id");
        let coordinator_id =
            foco_agent::AgentInstanceId::new("agent-instance-coordinator").expect("instance id");
        let worker_id =
            foco_agent::AgentInstanceId::new("agent-instance-worker").expect("instance id");
        let task_id = AgentTaskId::new("agent-task-protocol").expect("task id");
        let attempt_id = AgentAttemptId::new("agent-attempt-protocol").expect("attempt id");
        let now = "2026-01-01T00:00:00Z".to_string();
        let team = AgentTeamRecord {
            id: team_id.clone(),
            chat_id: "chat-protocol".to_string(),
            coordinator_instance_id: coordinator_id.clone(),
            status: foco_agent::AgentTeamStatus::Active,
            max_concurrent_runs: 1,
            next_event_sequence: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let coordinator = test_agent_instance(
            &team_id,
            &coordinator_id,
            coordinator_definition.clone(),
            AgentRole::Coordinator,
            &now,
        );
        let worker = test_agent_instance(
            &team_id,
            &worker_id,
            worker_definition.clone(),
            AgentRole::Worker,
            &now,
        );
        let task = AgentTaskRecord {
            id: task_id,
            team_id: team_id.clone(),
            owner_instance_id: coordinator_id,
            origin_instance_id: None,
            parent_task_id: None,
            sequence: 0,
            status: AgentTaskStatus::Running,
            input_json: "{}".to_string(),
            result_json: None,
            error_json: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            started_at: Some(now.clone()),
            completed_at: None,
        };
        let permissions = AgentPermissions {
            can_create_instances: true,
            can_delegate: false,
            allowed_agent_definition_ids: vec![worker_definition.id.clone()],
        };

        let prompt = agent_team_protocol_prompt(
            &team,
            &coordinator,
            &task,
            &attempt_id,
            &HashSet::new(),
            true,
            &permissions,
            &[coordinator_definition, worker_definition],
            &[coordinator.clone(), worker],
        )
        .expect("protocol prompt");
        assert!(prompt.contains("## Subagents"));
        let protocol = agent_team_protocol_json_from_prompt(&prompt);
        let creatable = protocol["creatableAgentDefinitions"]
            .as_array()
            .expect("creatable definitions");

        assert_eq!(protocol["version"], json!(2));
        assert_eq!(creatable.len(), 1);
        assert_eq!(
            creatable[0]["definitionId"],
            json!("agent-definition-worker")
        );
        assert_eq!(creatable[0]["maxInstances"], json!(3));
        assert_eq!(creatable[0]["currentTeamInstances"], json!(2));
        assert_eq!(creatable[0]["currentTeamDefinitionInstances"], json!(1));
        assert_eq!(creatable[0]["remainingTeamDefinitionSlots"], json!(2));
        assert_eq!(creatable[0]["maxCreateCount"], json!(2));
        assert_eq!(creatable[0]["canCreateMore"], json!(true));
        assert_eq!(
            creatable[0]["agentCreateInstancesSchema"]["count"]["maximum"],
            json!(2)
        );
        assert_eq!(
            creatable[0]["allowedExecutionWorkspaceModes"],
            json!(["shared"])
        );
        assert_eq!(
            creatable[0]["agentCreateInstancesSchema"]["executionWorkspaceMode"]["enum"],
            json!(["shared"])
        );
        assert!(creatable[0]["agentCreateInstancesSchema"]["maxInstancesPerTeam"].is_null());
        assert!(creatable[0]["agentCreateInstancesSchema"]["maxInstancesForDefinition"].is_null());
    }

    #[test]
    fn team_protocol_uses_instance_snapshot_for_stale_creatable_definition() {
        let coordinator_definition = test_agent_definition("stale-coordinator", 1);
        let worker_definition = test_agent_definition("stale-worker", 3);
        let missing_id = foco_agent::AgentDefinitionId::new("agent-definition-stale-missing")
            .expect("missing definition id");
        let team_id = foco_agent::AgentTeamId::new("agent-team-stale-creatable").expect("team id");
        let coordinator_id = foco_agent::AgentInstanceId::new("agent-instance-stale-coordinator")
            .expect("coordinator id");
        let worker_id =
            foco_agent::AgentInstanceId::new("agent-instance-stale-worker").expect("worker id");
        let task_id = AgentTaskId::new("agent-task-stale-creatable").expect("task id");
        let attempt_id = AgentAttemptId::new("agent-attempt-stale-creatable").expect("attempt id");
        let now = "2026-01-01T00:00:00Z".to_string();
        let team = AgentTeamRecord {
            id: team_id.clone(),
            chat_id: "chat-stale-creatable".to_string(),
            coordinator_instance_id: coordinator_id.clone(),
            status: foco_agent::AgentTeamStatus::Active,
            max_concurrent_runs: 1,
            next_event_sequence: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let coordinator = test_agent_instance(
            &team_id,
            &coordinator_id,
            coordinator_definition.clone(),
            AgentRole::Coordinator,
            &now,
        );
        let worker = test_agent_instance(
            &team_id,
            &worker_id,
            worker_definition.clone(),
            AgentRole::Worker,
            &now,
        );
        let task = AgentTaskRecord {
            id: task_id,
            team_id: team_id.clone(),
            owner_instance_id: coordinator_id,
            origin_instance_id: None,
            parent_task_id: None,
            sequence: 0,
            status: AgentTaskStatus::Running,
            input_json: "{}".to_string(),
            result_json: None,
            error_json: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            started_at: Some(now.clone()),
            completed_at: None,
        };
        let permissions = AgentPermissions {
            can_create_instances: true,
            can_delegate: false,
            allowed_agent_definition_ids: vec![worker_definition.id.clone(), missing_id],
        };

        let prompt = agent_team_protocol_prompt(
            &team,
            &coordinator,
            &task,
            &attempt_id,
            &HashSet::new(),
            true,
            &permissions,
            &[coordinator_definition],
            &[coordinator.clone(), worker],
        )
        .expect("protocol prompt");
        let protocol = agent_team_protocol_json_from_prompt(&prompt);
        let creatable = protocol["creatableAgentDefinitions"]
            .as_array()
            .expect("creatable definitions");

        assert_eq!(creatable.len(), 1);
        assert_eq!(
            creatable[0]["definitionId"],
            json!("agent-definition-stale-worker")
        );
        assert_eq!(creatable[0]["maxInstances"], json!(3));
    }

    #[test]
    fn wait_resume_messages_include_parent_resume_instruction() {
        let team_id = foco_agent::AgentTeamId::new("agent-team-wait-resume").expect("team id");
        let waiting_task_id = AgentTaskId::new("agent-task-waiting").expect("waiting task id");
        let dependency_task_id =
            AgentTaskId::new("agent-task-dependency").expect("dependency task id");
        let worker_id =
            foco_agent::AgentInstanceId::new("agent-instance-worker").expect("worker id");
        let now = "2026-01-01T00:00:00Z".to_string();
        let dependencies = vec![foco_store::workspace::AgentTaskDependencyRecord {
            team_id: team_id.clone(),
            waiting_task_id: waiting_task_id.clone(),
            dependency_task_id: dependency_task_id.clone(),
            wait_mode: foco_agent::AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait".to_string()),
            deadline_at: None,
            created_at: now.clone(),
        }];
        let dependency_tasks = vec![AgentTaskRecord {
            id: dependency_task_id,
            team_id,
            owner_instance_id: worker_id,
            origin_instance_id: None,
            parent_task_id: Some(waiting_task_id),
            sequence: 0,
            status: AgentTaskStatus::Completed,
            input_json: "{}".to_string(),
            result_json: Some(r#"{"text":"worker result"}"#.to_string()),
            error_json: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            started_at: Some(now.clone()),
            completed_at: Some(now),
        }];

        let messages = agent_wait_resume_messages(&dependencies, &dependency_tasks)
            .expect("wait resume messages");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, NeutralChatRole::System);
        assert_eq!(messages[0].content, AGENT_WAIT_RESUME_INSTRUCTION);
        assert_eq!(messages[1].role, NeutralChatRole::Assistant);
        assert_eq!(messages[1].tool_calls.len(), 1);
        assert_eq!(messages[1].tool_calls[0].call_id, "call-wait");
        assert_eq!(messages[2].role, NeutralChatRole::Tool);
        assert_eq!(messages[2].tool_call_id.as_deref(), Some("call-wait"));
        assert!(messages[2].content.contains("worker result"));
    }

    #[test]
    fn agent_task_model_selection_uses_queued_model_without_pinning_legacy_provider() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut database = open_workspace_database_critical(workspace.path()).expect("database");
        let team_id = foco_agent::AgentTeamId::new("agent-team-model-selection").expect("team id");
        let coordinator_id = foco_agent::AgentInstanceId::new("agent-instance-model-selection")
            .expect("instance id");
        let now = "2026-01-01T00:00:00Z".to_string();
        let team = AgentTeamRecord {
            id: team_id.clone(),
            chat_id: "chat-model-selection".to_string(),
            coordinator_instance_id: coordinator_id.clone(),
            status: foco_agent::AgentTeamStatus::Active,
            max_concurrent_runs: 1,
            next_event_sequence: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let mut definition = test_agent_definition("coordinator", 1);
        definition.model_id = "snapshot-model".to_string();
        definition.provider_id = "snapshot-provider".to_string();
        definition.model_options.thinking_level = Some("snapshot-thinking".to_string());
        let instance = test_agent_instance(
            &team_id,
            &coordinator_id,
            definition,
            AgentRole::Coordinator,
            &now,
        );
        let task_input = CoordinatorTaskInput {
            queued_user_message_id: "user-model-selection".to_string(),
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            message: "Use override".to_string(),
            attachments: Vec::new(),
            skill_ids: Vec::new(),
            session_mode: None,
            latency_mode: foco_providers::LatencyMode::Standard,
            collaboration_tools_enabled: false,
            defer_until_workspace_idle: false,
            delegated_input: None,
            correlation_id: None,
        };

        database
            .insert_chat(&team.chat_id, "Model selection")
            .expect("chat insert");
        database
            .insert_message(foco_store::workspace::NewMessage {
                id: &task_input.queued_user_message_id,
                chat_id: &team.chat_id,
                role: "user",
                content: "Use override",
                sequence: 0,
                metadata_json: Some(
                    r#"{"queuedRun":{"status":"queued","modelId":"queued-model","providerId":"queued-provider","thinkingLevel":"queued-thinking","skillIds":[]}}"#,
                ),
            })
            .expect("message insert");

        let selection = agent_task_model_selection(&database, &team, &instance, &task_input)
            .expect("selection");

        assert_eq!(selection.model_id, "queued-model");
        assert_eq!(selection.thinking_level.as_deref(), Some("queued-thinking"));
    }

    fn test_agent_definition(suffix: &str, max_instances: u32) -> AgentDefinitionSettings {
        AgentDefinitionSettings {
            id: foco_agent::AgentDefinitionId::new(format!("agent-definition-{suffix}"))
                .expect("definition id"),
            revision: 1,
            name: suffix.to_string(),
            description: format!("{suffix} definition"),
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            model_options: foco_store::config::AgentModelOptions::default(),
            system_prompt: "Do the task.".to_string(),
            allowed_tools: Vec::new(),
            max_instances,
            allowed_execution_workspace_modes: AgentExecutionWorkspaceMode::all(),
            permissions: AgentPermissions::default(),
        }
    }

    fn test_agent_instance(
        team_id: &foco_agent::AgentTeamId,
        instance_id: &foco_agent::AgentInstanceId,
        definition: AgentDefinitionSettings,
        role: AgentRole,
        now: &str,
    ) -> AgentInstanceRecord {
        AgentInstanceRecord {
            id: instance_id.clone(),
            team_id: team_id.clone(),
            definition_id: definition.id.clone(),
            definition_revision: definition.revision,
            definition_snapshot: definition,
            role,
            status: AgentInstanceStatus::Idle,
            next_task_sequence: 0,
            next_message_sequence: 0,
            context_generation: 0,
            last_scheduled_at: None,
            execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
            execution_root_path: None,
            worktree_base_revision: None,
            worktree_branch: None,
            worktree_status: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        }
    }
}
