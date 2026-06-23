use std::{path::Path, time::Duration};

use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use foco_agent::{AgentTaskId, AgentTaskStatus};
use foco_store::{
    config::{AgentDefinitionSettings, WorkspaceConfig},
    workspace::{
        AgentTaskRecord, NewScheduledTaskRun, ScheduledTaskDueRunClaim, ScheduledTaskRecord,
        ScheduledTaskRunRecord, ScheduledTaskRunUpdate, ScheduledTaskUpdate, WorkspaceDatabase,
    },
};
use serde_json::{Value, json};
use tokio::{sync::mpsc, task::JoinHandle, time};

use crate::{
    http::chat::{QueueChatMessageInput, QueuedChatMessageOrigin, queue_chat_message_internal},
    *,
};

use super::{
    service::{ScheduledTaskError, next_run_after},
    types::{
        ScheduleSpec, ScheduledAction, ScheduledConcurrencyPolicy, ScheduledSessionMode,
        ScheduledTaskMetadata,
    },
};

const STATUS_ENABLED: &str = "enabled";
const STATUS_COMPLETED: &str = "completed";
const STATUS_ARCHIVED: &str = "archived";
const RUN_STATUS_PENDING: &str = "pending";
const RUN_STATUS_QUEUED: &str = "queued";
const RUN_STATUS_RUNNING: &str = "running";
const RUN_STATUS_SUCCEEDED: &str = "succeeded";
const RUN_STATUS_FAILED: &str = "failed";
const RUN_STATUS_CANCELLED: &str = "cancelled";
const RUN_STATUS_SKIPPED: &str = "skipped";
const TRIGGER_REASON_SCHEDULED: &str = "scheduled";
const TRIGGER_REASON_MANUAL: &str = "manual";
const SCHEDULED_TASK_WAKE_CAPACITY: usize = 1;
const SCHEDULED_TASK_SCAN_LIMIT: usize = 64;
const SCHEDULED_TASK_MIN_SCAN_DELAY_MS: u64 = 1_000;
const SCHEDULED_TASK_IDLE_SCAN_INTERVAL_SECS: u64 = 300;
const SCHEDULED_TASK_RUN_RETENTION_DAYS: i64 = 90;

#[derive(Clone)]
pub(crate) struct ScheduledTaskScheduler {
    wake_tx: mpsc::Sender<()>,
}

impl ScheduledTaskScheduler {
    pub(crate) fn new() -> (Self, mpsc::Receiver<()>) {
        let (wake_tx, wake_rx) = mpsc::channel(SCHEDULED_TASK_WAKE_CAPACITY);
        (Self { wake_tx }, wake_rx)
    }

    pub(crate) fn wake(&self) -> Result<(), ApiError> {
        match self.wake_tx.try_send(()) {
            Ok(()) | Err(mpsc::error::TrySendError::Full(())) => Ok(()),
            Err(mpsc::error::TrySendError::Closed(())) => Err(ApiError::internal(
                "Scheduled task scheduler is not running",
            )),
        }
    }

    pub(crate) fn spawn(&self, state: AppState, wake_rx: mpsc::Receiver<()>) -> JoinHandle<()> {
        tokio::spawn(run_scheduled_task_scheduler(state, wake_rx))
    }
}

async fn run_scheduled_task_scheduler(state: AppState, mut wake_rx: mpsc::Receiver<()>) {
    let mut shutdown_rx = state.app_shutdown_rx.clone();
    let mut scan = true;
    let mut scan_delay = Duration::from_millis(SCHEDULED_TASK_MIN_SCAN_DELAY_MS);

    loop {
        if scan {
            scan = false;
            if let Err(error) = dispatch_due_scheduled_tasks(&state).await {
                tracing::error!(error = %error.message, "Scheduled task scheduler scan failed");
            }
            scan_delay = match next_scheduled_task_scan_delay(&state) {
                Ok(delay) => delay,
                Err(error) => {
                    tracing::error!(error = %error.message, "Scheduled task scheduler next scan failed");
                    Duration::from_secs(SCHEDULED_TASK_IDLE_SCAN_INTERVAL_SECS)
                }
            };
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

fn next_scheduled_task_scan_delay(state: &AppState) -> Result<Duration, ApiError> {
    let config = config_snapshot(state)?;
    let now = Utc::now();
    let mut next_run_at: Option<DateTime<Utc>> = None;

    for workspace in &config.workspaces {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        let Some(value) = database
            .next_enabled_scheduled_task_run_at()
            .map_err(ApiError::from_workspace_error)?
        else {
            continue;
        };
        let due_at = parse_utc_timestamp("scheduled task next_run_at", &value)?;
        match next_run_at.as_ref() {
            Some(current) if current <= &due_at => {}
            _ => next_run_at = Some(due_at),
        }
    }

    Ok(scheduled_task_scan_delay(now, next_run_at))
}

fn scheduled_task_scan_delay(now: DateTime<Utc>, next_run_at: Option<DateTime<Utc>>) -> Duration {
    let idle = Duration::from_secs(SCHEDULED_TASK_IDLE_SCAN_INTERVAL_SECS);
    let Some(next_run_at) = next_run_at else {
        return idle;
    };
    if next_run_at <= now {
        return Duration::from_millis(SCHEDULED_TASK_MIN_SCAN_DELAY_MS);
    }
    let millis_until_due = next_run_at.signed_duration_since(now).num_milliseconds();
    if millis_until_due <= 0 {
        return Duration::from_millis(SCHEDULED_TASK_MIN_SCAN_DELAY_MS);
    }
    // ponytail: cap long sleeps so a missed wake costs minutes, not a full schedule interval.
    idle.min(Duration::from_millis(millis_until_due as u64))
}

pub(crate) async fn dispatch_due_scheduled_tasks(state: &AppState) -> Result<(), ApiError> {
    let config = config_snapshot(state)?;
    reconcile_scheduled_task_runs_for_config(state, &config).await?;
    prune_old_scheduled_task_runs(&config)?;
    let now = Utc::now();
    let now_text = format_utc_timestamp(now);

    for workspace in &config.workspaces {
        for _ in 0..SCHEDULED_TASK_SCAN_LIMIT {
            let task = next_due_task(workspace, now)?;
            let Some(task) = task else {
                break;
            };
            let metadata = task_metadata(&task, &workspace.id)?;
            let (task_status, next_run_at) = next_task_state(&task.schedule_json, now)?;
            let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            let active_runs = database
                .active_scheduled_task_run_count(&task.id)
                .map_err(ApiError::from_workspace_error)?;
            let skip = metadata.concurrency_policy == ScheduledConcurrencyPolicy::SkipIfRunning
                && active_runs > 0;
            let (run_status, completed_at, error_message) = if skip {
                (
                    RUN_STATUS_SKIPPED,
                    Some(now_text.as_str()),
                    Some("skipped because a previous scheduled run is still active"),
                )
            } else {
                (RUN_STATUS_PENDING, None, None)
            };
            let run = database
                .claim_due_scheduled_task_run(ScheduledTaskDueRunClaim {
                    task_id: &task.id,
                    expected_next_run_at: task.next_run_at.as_deref().ok_or_else(|| {
                        ApiError::internal("due scheduled task is missing next_run_at")
                    })?,
                    run_id: &unique_id("scheduled-run"),
                    trigger_reason: TRIGGER_REASON_SCHEDULED,
                    run_status,
                    scheduled_at: task.next_run_at.as_deref().ok_or_else(|| {
                        ApiError::internal("due scheduled task is missing next_run_at")
                    })?,
                    completed_at,
                    error_message,
                    task_status: &task_status,
                    task_next_run_at: next_run_at.as_deref(),
                    task_last_run_at: &now_text,
                    metadata_json: None,
                })
                .map_err(ApiError::from_workspace_error)?;
            drop(database);

            let Some(run) = run else {
                continue;
            };
            if run.status == RUN_STATUS_PENDING {
                let _ = dispatch_scheduled_task_run(
                    state,
                    &config,
                    workspace,
                    task,
                    run,
                    TRIGGER_REASON_SCHEDULED,
                )
                .await?;
            }
        }
    }

    Ok(())
}

pub(crate) async fn reconcile_scheduled_task_runs(state: &AppState) -> Result<(), ApiError> {
    let config = config_snapshot(state)?;
    reconcile_scheduled_task_runs_for_config(state, &config).await
}

async fn reconcile_scheduled_task_runs_for_config(
    state: &AppState,
    config: &GlobalConfig,
) -> Result<(), ApiError> {
    for workspace in &config.workspaces {
        let runs = {
            let database = WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            database
                .active_scheduled_task_runs()
                .map_err(ApiError::from_workspace_error)?
        };

        for run in runs {
            if run.agent_task_id.is_some() {
                let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                    .map_err(ApiError::from_workspace_error)?;
                sync_scheduled_task_run(&mut database, run)?;
                continue;
            }

            if run.status == RUN_STATUS_PENDING {
                let task = {
                    let database = WorkspaceDatabase::open_or_create(&workspace.path)
                        .map_err(ApiError::from_workspace_error)?;
                    database
                        .scheduled_task(&run.task_id)
                        .map_err(ApiError::from_workspace_error)?
                };
                match task {
                    Some(task) => {
                        let trigger_reason = run.trigger_reason.clone();
                        dispatch_scheduled_task_run(
                            state,
                            config,
                            workspace,
                            task,
                            run,
                            &trigger_reason,
                        )
                        .await?;
                    }
                    None => {
                        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                            .map_err(ApiError::from_workspace_error)?;
                        mark_scheduled_run_failed_in_database(
                            &mut database,
                            run,
                            "scheduled task was not found during reconciliation",
                        )?;
                    }
                }
                continue;
            }

            let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            mark_scheduled_run_failed_in_database(
                &mut database,
                run,
                "scheduled run is missing linked Agent task",
            )?;
        }
    }

    Ok(())
}

fn prune_old_scheduled_task_runs(config: &GlobalConfig) -> Result<(), ApiError> {
    let cutoff = Utc::now()
        .checked_sub_signed(ChronoDuration::days(SCHEDULED_TASK_RUN_RETENTION_DAYS))
        .ok_or_else(|| ApiError::internal("scheduled run retention cutoff overflowed"))?;
    let cutoff = format_utc_timestamp(cutoff);
    for workspace in &config.workspaces {
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        database
            .delete_old_scheduled_task_runs(&cutoff)
            .map_err(ApiError::from_workspace_error)?;
    }
    Ok(())
}

pub(crate) async fn run_scheduled_task_now(
    state: &AppState,
    workspace_id: &str,
    task_id: &str,
) -> Result<ScheduledTaskRunRecord, ApiError> {
    let config = config_snapshot(state)?;
    let workspace = workspace_by_id(&config, workspace_id)?;
    let now = format_utc_timestamp(Utc::now());
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let task = database
        .scheduled_task(task_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("scheduled task was not found: {task_id}")))?;
    if task.status == STATUS_ARCHIVED {
        return Err(ApiError::bad_request(
            "archived scheduled tasks cannot be run manually",
        ));
    }

    let run = database
        .insert_scheduled_task_run(NewScheduledTaskRun {
            id: &unique_id("scheduled-run"),
            task_id: &task.id,
            trigger_reason: TRIGGER_REASON_MANUAL,
            status: RUN_STATUS_PENDING,
            scheduled_at: &now,
            queued_at: None,
            started_at: None,
            completed_at: None,
            chat_id: None,
            user_message_id: None,
            assistant_message_id: None,
            agent_team_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            active_run_id: None,
            error_message: None,
            output_summary: None,
            metadata_json: None,
        })
        .map_err(ApiError::from_workspace_error)?;
    database
        .update_scheduled_task(ScheduledTaskUpdate {
            id: &task.id,
            title: &task.title,
            description: task.description.as_deref(),
            schedule_json: &task.schedule_json,
            action_json: &task.action_json,
            status: &task.status,
            next_run_at: task.next_run_at.as_deref(),
            last_run_at: Some(&now),
            metadata_json: &task.metadata_json,
        })
        .map_err(ApiError::from_workspace_error)?;
    drop(database);

    dispatch_scheduled_task_run(state, &config, workspace, task, run, TRIGGER_REASON_MANUAL).await
}

pub(crate) fn cancel_scheduled_task_run(
    state: &AppState,
    workspace_id: &str,
    scheduled_run_id: &str,
) -> Result<ScheduledTaskRunRecord, ApiError> {
    let config = config_snapshot(state)?;
    let workspace = workspace_by_id(&config, workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let run = database
        .scheduled_task_run(scheduled_run_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "scheduled task run was not found: {scheduled_run_id}"
            ))
        })?;
    let run = sync_scheduled_task_run(&mut database, run)?;
    if scheduled_run_status_is_terminal(&run.status) {
        return Err(ApiError::bad_request(format!(
            "scheduled task run '{}' cannot be cancelled while {}",
            run.id, run.status
        )));
    }

    let Some(agent_task_id) = run.agent_task_id.clone() else {
        return mark_scheduled_run_cancelled(&mut database, run, "cancelled explicitly");
    };
    let task = database
        .agent_task(&agent_task_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!("Agent task '{agent_task_id}' was not found"))
        })?;

    match task.status {
        AgentTaskStatus::Queued => {
            if !database
                .cancel_queued_agent_task(
                    &task.team_id,
                    &task.id,
                    r#"{"message":"cancelled explicitly"}"#,
                )
                .map_err(ApiError::from_workspace_error)?
            {
                let run = sync_scheduled_task_run_with_task(&mut database, run, &task)?;
                return Err(ApiError::bad_request(format!(
                    "scheduled task run '{}' changed state before cancellation; current status is {}",
                    run.id, run.status
                )));
            }
            crate::insert_agent_event(
                &mut database,
                &task.team_id,
                "task_cancelled",
                Some(&task.owner_instance_id),
                Some(&task.id),
                None,
                json!({ "reason": "scheduled_run_cancel", "scheduledTaskRunId": run.id }),
            )?;
            state.agent_scheduler.wake()?;
            let task = database
                .agent_task(&agent_task_id)
                .map_err(ApiError::from_workspace_error)?
                .ok_or_else(|| {
                    ApiError::internal(format!("Agent task '{agent_task_id}' was not found"))
                })?;
            sync_scheduled_task_run_with_task(&mut database, run, &task)
        }
        AgentTaskStatus::Running => {
            let active_run_id = run
                .active_run_id
                .as_deref()
                .unwrap_or_else(|| task.id.as_str());
            state.active_chat_runs.cancel(workspace_id, active_run_id)?;
            crate::insert_agent_event(
                &mut database,
                &task.team_id,
                "task_cancel_requested",
                Some(&task.owner_instance_id),
                Some(&task.id),
                None,
                json!({ "scheduledTaskRunId": run.id }),
            )?;
            state.agent_scheduler.wake()?;
            sync_scheduled_task_run_with_task(&mut database, run, &task)
        }
        AgentTaskStatus::Waiting => Err(ApiError::bad_request(format!(
            "scheduled task run '{}' cannot be cancelled while Agent task '{}' is waiting",
            run.id, task.id
        ))),
        AgentTaskStatus::Completed
        | AgentTaskStatus::Failed
        | AgentTaskStatus::Cancelled
        | AgentTaskStatus::Interrupted => {
            let run = sync_scheduled_task_run_with_task(&mut database, run, &task)?;
            Err(ApiError::bad_request(format!(
                "scheduled task run '{}' cannot be cancelled while {}",
                run.id, run.status
            )))
        }
    }
}

// ponytail: read-time sync avoids a second scheduled-task event bus; add push updates when
// the UI needs live run rows without polling/refetching.
pub(crate) fn sync_scheduled_task_run(
    database: &mut WorkspaceDatabase,
    run: ScheduledTaskRunRecord,
) -> Result<ScheduledTaskRunRecord, ApiError> {
    let Some(agent_task_id) = run.agent_task_id.clone() else {
        return Ok(run);
    };
    let Some(task) = database
        .agent_task(&agent_task_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        if scheduled_run_status_is_terminal(&run.status) {
            return Ok(run);
        }
        return mark_scheduled_run_failed_in_database(
            database,
            run,
            "linked Agent task was not found",
        );
    };
    sync_scheduled_task_run_with_task(database, run, &task)
}

pub(crate) fn sync_scheduled_task_runs_for_agent_task(
    workspace_path: &Path,
    agent_task_id: &AgentTaskId,
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let runs = database
        .scheduled_task_runs_for_agent_task(agent_task_id)
        .map_err(ApiError::from_workspace_error)?;
    for run in runs {
        sync_scheduled_task_run(&mut database, run)?;
    }
    Ok(())
}

fn next_due_task(
    workspace: &WorkspaceConfig,
    now: DateTime<Utc>,
) -> Result<Option<ScheduledTaskRecord>, ApiError> {
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    for task in database
        .scheduled_tasks(Some(STATUS_ENABLED))
        .map_err(ApiError::from_workspace_error)?
    {
        let Some(next_run_at) = task.next_run_at.as_deref() else {
            continue;
        };
        let next_run_at = parse_utc_timestamp("scheduled task next_run_at", next_run_at)?;
        if next_run_at <= now {
            return Ok(Some(task));
        }
    }
    Ok(None)
}

async fn dispatch_scheduled_task_run(
    state: &AppState,
    config: &GlobalConfig,
    workspace: &WorkspaceConfig,
    task: ScheduledTaskRecord,
    run: ScheduledTaskRunRecord,
    trigger_reason: &str,
) -> Result<ScheduledTaskRunRecord, ApiError> {
    let input = scheduled_queue_input(config, &task, &run, trigger_reason)?;
    let queued = match queue_chat_message_internal(state, &workspace.id, input).await {
        Ok(queued) => queued,
        Err(error) => return mark_scheduled_run_failed(workspace, run, &error.message),
    };
    let queued_at = format_utc_timestamp(Utc::now());
    let active_run_id = queued.agent_task_id.as_ref().map(ToString::to_string);
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let run = database
        .update_scheduled_task_run(ScheduledTaskRunUpdate {
            id: &run.id,
            status: RUN_STATUS_QUEUED,
            queued_at: Some(&queued_at),
            started_at: run.started_at.as_deref(),
            completed_at: None,
            chat_id: Some(&queued.chat_id),
            user_message_id: Some(&queued.user_message_id),
            assistant_message_id: run.assistant_message_id.as_deref(),
            agent_team_id: queued.agent_team_id.as_ref(),
            agent_task_id: queued.agent_task_id.as_ref(),
            agent_attempt_id: run.agent_attempt_id.as_ref(),
            active_run_id: active_run_id.as_deref(),
            error_message: None,
            output_summary: run.output_summary.as_deref(),
            metadata_json: &run.metadata_json,
        })
        .map_err(ApiError::from_workspace_error)?;
    state.agent_scheduler.wake()?;
    Ok(run)
}

fn scheduled_queue_input(
    config: &GlobalConfig,
    task: &ScheduledTaskRecord,
    run: &ScheduledTaskRunRecord,
    trigger_reason: &str,
) -> Result<QueueChatMessageInput, ApiError> {
    let action = serde_json::from_str::<ScheduledAction>(&task.action_json).map_err(|source| {
        ApiError::bad_request(format!(
            "scheduled task '{}' has invalid action JSON: {source}",
            task.id
        ))
    })?;
    match action {
        ScheduledAction::AgentPrompt {
            prompt,
            session_mode,
            agent_definition_id,
            model_id,
            provider_id,
            thinking_level,
            skill_ids,
            collaboration_tools_enabled,
        } => {
            let definition = agent_definition_id
                .as_deref()
                .map(|id| scheduled_agent_definition(config, id))
                .transpose()?;
            let prompt = prompt.trim().to_string();
            if prompt.is_empty() {
                return Err(ApiError::bad_request("scheduled prompt must not be empty"));
            }
            let chat_id = match session_mode {
                ScheduledSessionMode::CreateNewChat => None,
                ScheduledSessionMode::ReuseChat { chat_id } => {
                    let chat_id = chat_id.trim().to_string();
                    if chat_id.is_empty() {
                        return Err(ApiError::bad_request(
                            "scheduled session chat id must not be empty",
                        ));
                    }
                    Some(chat_id)
                }
            };
            let model_id = model_id
                .or_else(|| {
                    definition
                        .as_ref()
                        .map(|definition| definition.model_id.clone())
                })
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "scheduled agent prompt must specify modelId or agentDefinitionId",
                    )
                })?;
            let provider_id = provider_id.or_else(|| {
                definition
                    .as_ref()
                    .map(|definition| definition.provider_id.clone())
            });
            let thinking_level = thinking_level.or_else(|| {
                definition
                    .as_ref()
                    .and_then(|definition| definition.model_options.thinking_level.clone())
            });

            Ok(QueueChatMessageInput {
                chat_id,
                model_id,
                provider_id,
                thinking_level,
                skill_ids: Some(skill_ids),
                message: prompt,
                team_mode_enabled: collaboration_tools_enabled,
                attachments: Vec::new(),
                agent_definition_id,
                origin: QueuedChatMessageOrigin::ScheduledTask {
                    task_id: task.id.clone(),
                    run_id: run.id.clone(),
                    trigger_reason: trigger_reason.to_string(),
                },
            })
        }
    }
}

fn scheduled_agent_definition(
    config: &GlobalConfig,
    id: &str,
) -> Result<AgentDefinitionSettings, ApiError> {
    let id = foco_agent::AgentDefinitionId::new(id.to_string())
        .map_err(|source| ApiError::bad_request(source.to_string()))?;
    config
        .agent_definitions
        .iter()
        .find(|definition| definition.id == id)
        .cloned()
        .ok_or_else(|| ApiError::bad_request(format!("AgentDefinition '{id}' was not found")))
}

fn mark_scheduled_run_failed(
    workspace: &WorkspaceConfig,
    run: ScheduledTaskRunRecord,
    message: &str,
) -> Result<ScheduledTaskRunRecord, ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    mark_scheduled_run_failed_in_database(&mut database, run, message)
}

fn mark_scheduled_run_failed_in_database(
    database: &mut WorkspaceDatabase,
    run: ScheduledTaskRunRecord,
    message: &str,
) -> Result<ScheduledTaskRunRecord, ApiError> {
    let completed_at = format_utc_timestamp(Utc::now());
    database
        .update_scheduled_task_run(ScheduledTaskRunUpdate {
            id: &run.id,
            status: RUN_STATUS_FAILED,
            queued_at: run.queued_at.as_deref(),
            started_at: run.started_at.as_deref(),
            completed_at: Some(&completed_at),
            chat_id: run.chat_id.as_deref(),
            user_message_id: run.user_message_id.as_deref(),
            assistant_message_id: run.assistant_message_id.as_deref(),
            agent_team_id: run.agent_team_id.as_ref(),
            agent_task_id: run.agent_task_id.as_ref(),
            agent_attempt_id: run.agent_attempt_id.as_ref(),
            active_run_id: run.active_run_id.as_deref(),
            error_message: Some(message),
            output_summary: run.output_summary.as_deref(),
            metadata_json: &run.metadata_json,
        })
        .map_err(ApiError::from_workspace_error)
}

fn mark_scheduled_run_cancelled(
    database: &mut WorkspaceDatabase,
    run: ScheduledTaskRunRecord,
    message: &str,
) -> Result<ScheduledTaskRunRecord, ApiError> {
    let completed_at = format_utc_timestamp(Utc::now());
    database
        .update_scheduled_task_run(ScheduledTaskRunUpdate {
            id: &run.id,
            status: RUN_STATUS_CANCELLED,
            queued_at: run.queued_at.as_deref(),
            started_at: run.started_at.as_deref(),
            completed_at: Some(&completed_at),
            chat_id: run.chat_id.as_deref(),
            user_message_id: run.user_message_id.as_deref(),
            assistant_message_id: run.assistant_message_id.as_deref(),
            agent_team_id: run.agent_team_id.as_ref(),
            agent_task_id: run.agent_task_id.as_ref(),
            agent_attempt_id: run.agent_attempt_id.as_ref(),
            active_run_id: run.active_run_id.as_deref(),
            error_message: Some(message),
            output_summary: run.output_summary.as_deref(),
            metadata_json: &run.metadata_json,
        })
        .map_err(ApiError::from_workspace_error)
}

fn sync_scheduled_task_run_with_task(
    database: &mut WorkspaceDatabase,
    run: ScheduledTaskRunRecord,
    task: &AgentTaskRecord,
) -> Result<ScheduledTaskRunRecord, ApiError> {
    let status = scheduled_run_status_for_agent_task(task.status);
    let started_at = match task.status {
        AgentTaskStatus::Queued => run.started_at.clone(),
        AgentTaskStatus::Running
        | AgentTaskStatus::Waiting
        | AgentTaskStatus::Completed
        | AgentTaskStatus::Failed
        | AgentTaskStatus::Cancelled
        | AgentTaskStatus::Interrupted => task.started_at.clone().or(run.started_at.clone()),
    };
    let completed_at = if task.status.is_terminal() {
        task.completed_at.clone().or(run.completed_at.clone())
    } else {
        None
    };
    let error_message = match task.status {
        AgentTaskStatus::Failed | AgentTaskStatus::Cancelled | AgentTaskStatus::Interrupted => {
            agent_task_error_message(task)
        }
        AgentTaskStatus::Queued
        | AgentTaskStatus::Running
        | AgentTaskStatus::Waiting
        | AgentTaskStatus::Completed => None,
    };
    let agent_attempt_id = database
        .agent_attempts_for_task(&task.id)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .last()
        .map(|attempt| attempt.id)
        .or_else(|| run.agent_attempt_id.clone());
    let active_run_id = run
        .active_run_id
        .clone()
        .or_else(|| Some(task.id.to_string()));

    if run.status == status
        && run.started_at == started_at
        && run.completed_at == completed_at
        && run.error_message == error_message
        && run.agent_attempt_id == agent_attempt_id
        && run.active_run_id == active_run_id
    {
        return Ok(run);
    }

    database
        .update_scheduled_task_run(ScheduledTaskRunUpdate {
            id: &run.id,
            status,
            queued_at: run.queued_at.as_deref(),
            started_at: started_at.as_deref(),
            completed_at: completed_at.as_deref(),
            chat_id: run.chat_id.as_deref(),
            user_message_id: run.user_message_id.as_deref(),
            assistant_message_id: run.assistant_message_id.as_deref(),
            agent_team_id: run.agent_team_id.as_ref(),
            agent_task_id: run.agent_task_id.as_ref(),
            agent_attempt_id: agent_attempt_id.as_ref(),
            active_run_id: active_run_id.as_deref(),
            error_message: error_message.as_deref(),
            output_summary: run.output_summary.as_deref(),
            metadata_json: &run.metadata_json,
        })
        .map_err(ApiError::from_workspace_error)
}

fn scheduled_run_status_for_agent_task(status: AgentTaskStatus) -> &'static str {
    match status {
        AgentTaskStatus::Queued => RUN_STATUS_QUEUED,
        AgentTaskStatus::Running | AgentTaskStatus::Waiting => RUN_STATUS_RUNNING,
        AgentTaskStatus::Completed => RUN_STATUS_SUCCEEDED,
        AgentTaskStatus::Failed | AgentTaskStatus::Interrupted => RUN_STATUS_FAILED,
        AgentTaskStatus::Cancelled => RUN_STATUS_CANCELLED,
    }
}

fn scheduled_run_status_is_terminal(status: &str) -> bool {
    matches!(
        status,
        RUN_STATUS_SUCCEEDED | RUN_STATUS_FAILED | RUN_STATUS_CANCELLED | RUN_STATUS_SKIPPED
    )
}

fn agent_task_error_message(task: &AgentTaskRecord) -> Option<String> {
    let error_json = task.error_json.as_deref()?;
    serde_json::from_str::<Value>(error_json)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .or_else(|| Some(error_json.to_string()))
}

fn task_metadata(
    task: &ScheduledTaskRecord,
    workspace_id: &str,
) -> Result<ScheduledTaskMetadata, ApiError> {
    let mut value = serde_json::from_str::<Value>(&task.metadata_json).map_err(|source| {
        ApiError::bad_request(format!(
            "scheduled task '{}' has invalid metadata JSON: {source}",
            task.id
        ))
    })?;
    let Value::Object(ref mut metadata) = value else {
        return Err(ApiError::bad_request(format!(
            "scheduled task '{}' metadata must be a JSON object",
            task.id
        )));
    };
    metadata
        .entry("workspaceId".to_string())
        .or_insert_with(|| Value::String(workspace_id.to_string()));
    serde_json::from_value(value).map_err(|source| {
        ApiError::bad_request(format!(
            "scheduled task '{}' has invalid metadata JSON: {source}",
            task.id
        ))
    })
}

fn next_task_state(
    schedule_json: &str,
    now: DateTime<Utc>,
) -> Result<(String, Option<String>), ApiError> {
    let schedule = serde_json::from_str::<ScheduleSpec>(schedule_json).map_err(|source| {
        ApiError::bad_request(format!(
            "scheduled task has invalid schedule JSON: {source}"
        ))
    })?;
    let next = next_run_after(&schedule, now)
        .map_err(scheduled_task_error)?
        .map(format_utc_timestamp);
    let status = if next.is_some() {
        STATUS_ENABLED
    } else {
        STATUS_COMPLETED
    };
    Ok((status.to_string(), next))
}

fn scheduled_task_error(error: ScheduledTaskError) -> ApiError {
    ApiError::bad_request(error.to_string())
}

fn parse_utc_timestamp(field: &str, value: &str) -> Result<DateTime<Utc>, ApiError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| ApiError::bad_request(format!("{field} must be an RFC 3339 timestamp")))
}

fn format_utc_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduled_task_scan_delay_caps_idle_and_uses_due_time() {
        let now = DateTime::parse_from_rfc3339("2026-06-23T10:00:00Z")
            .expect("now")
            .with_timezone(&Utc);
        let soon = now + ChronoDuration::seconds(30);
        let later = now + ChronoDuration::minutes(10);

        assert_eq!(
            scheduled_task_scan_delay(now, None),
            Duration::from_secs(SCHEDULED_TASK_IDLE_SCAN_INTERVAL_SECS)
        );
        assert_eq!(
            scheduled_task_scan_delay(now, Some(soon)),
            Duration::from_secs(30)
        );
        assert_eq!(
            scheduled_task_scan_delay(now, Some(later)),
            Duration::from_secs(SCHEDULED_TASK_IDLE_SCAN_INTERVAL_SECS)
        );
        assert_eq!(
            scheduled_task_scan_delay(now, Some(now - ChronoDuration::seconds(1))),
            Duration::from_millis(SCHEDULED_TASK_MIN_SCAN_DELAY_MS)
        );
    }
}
