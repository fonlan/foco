use std::path::Path;

use foco_agent::{AgentExecutionWorkspaceMode, AgentInstanceStatus, AgentTaskId, AgentTaskStatus};
use foco_store::{
    config::{GlobalConfig, ModelSettings, WorkspaceConfig},
    workspace::{
        AgentInstanceRecord, AgentTaskRecord, PlanPhaseRecord, PlanRecord, WorkspaceDatabase,
    },
};
use serde_json::Value;

use crate::{
    git_backend::{commit_staged_changes, merge_agent_worktree, stage_git_file},
    http::chat::{QueueChatMessageInput, QueuedChatMessageOrigin, queue_chat_message_internal},
    *,
};

struct PlanRunnerModelSelection {
    model_id: String,
    provider_id: String,
    thinking_level: Option<String>,
}

pub(crate) async fn transition_plan_action(
    state: &AppState,
    workspace_id: &str,
    plan_id: &str,
    action: &str,
) -> Result<PlanRecord, ApiError> {
    let action = action.trim();
    if !matches!(action, "start" | "resume") {
        let config = config_snapshot(state)?;
        let workspace = workspace_by_id(&config, workspace_id)?;
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        return database
            .transition_plan(plan_id, action)
            .map_err(ApiError::from_workspace_error);
    }

    let config = config_snapshot(state)?;
    let _selection = plan_runner_model_selection(&config)?;
    let workspace = workspace_by_id(&config, workspace_id)?.clone();
    let plan = {
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        database
            .transition_plan(plan_id, action)
            .map_err(ApiError::from_workspace_error)?
    };
    dispatch_plan_phase(state, &workspace.id, plan).await
}

pub(crate) async fn sync_plan_phase_for_agent_task(
    state: &AppState,
    workspace: &WorkspaceConfig,
    task_id: &AgentTaskId,
) -> Result<(), ApiError> {
    let (phase, task, instance) = {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        let Some(phase) = database
            .plan_phase_for_agent_task(task_id)
            .map_err(ApiError::from_workspace_error)?
        else {
            return Ok(());
        };
        let task = database
            .agent_task(task_id)
            .map_err(ApiError::from_workspace_error)?
            .ok_or_else(|| ApiError::internal(format!("Agent task '{task_id}' was not found")))?;
        let instance = database
            .agent_instance(&task.owner_instance_id)
            .map_err(ApiError::from_workspace_error)?
            .ok_or_else(|| {
                ApiError::internal(format!(
                    "Agent instance '{}' was not found",
                    task.owner_instance_id
                ))
            })?;
        (phase, task, instance)
    };

    match task.status {
        AgentTaskStatus::Completed => {
            let commit_id = match merge_and_commit_plan_phase(workspace, &phase, &instance) {
                Ok(commit_id) => commit_id,
                Err(error) => {
                    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                        .map_err(ApiError::from_workspace_error)?;
                    database
                        .fail_plan_phase_run(task_id, &error.message)
                        .map_err(ApiError::from_workspace_error)?;
                    return Ok(());
                }
            };
            let plan = {
                let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                    .map_err(ApiError::from_workspace_error)?;
                database
                    .complete_plan_phase_run(task_id, commit_id.as_deref())
                    .map_err(ApiError::from_workspace_error)?
            };
            if let Some(plan) = plan {
                continue_plan_if_ready(state, workspace, plan).await?;
            }
        }
        AgentTaskStatus::Failed | AgentTaskStatus::Cancelled | AgentTaskStatus::Interrupted => {
            let message = agent_task_error_message(&task);
            let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            database
                .fail_plan_phase_run(task_id, &message)
                .map_err(ApiError::from_workspace_error)?;
        }
        AgentTaskStatus::Queued | AgentTaskStatus::Running | AgentTaskStatus::Waiting => {}
    }

    Ok(())
}

async fn continue_plan_if_ready(
    state: &AppState,
    workspace: &WorkspaceConfig,
    plan: PlanRecord,
) -> Result<(), ApiError> {
    if plan.status == "ready" {
        let _ = transition_plan_action(state, &workspace.id, &plan.id, "resume").await?;
    }
    Ok(())
}

async fn dispatch_plan_phase(
    state: &AppState,
    workspace_id: &str,
    plan: PlanRecord,
) -> Result<PlanRecord, ApiError> {
    if plan.status == "implemented" || plan.active_phase_id.is_none() {
        return Ok(plan);
    }
    let phase_id = plan
        .active_phase_id
        .as_deref()
        .ok_or_else(|| ApiError::internal(format!("plan '{}' has no active phase", plan.id)))?;
    let phase = plan
        .phases
        .iter()
        .find(|phase| phase.id == phase_id)
        .ok_or_else(|| {
            ApiError::internal(format!(
                "plan '{}' active phase '{}' was not found",
                plan.id, phase_id
            ))
        })?;
    if phase.agent_task_id.is_some() {
        return Ok(plan);
    }

    let config = config_snapshot(state)?;
    let selection = plan_runner_model_selection(&config)?;
    let queued = match queue_chat_message_internal(
        state,
        workspace_id,
        QueueChatMessageInput {
            chat_id: None,
            model_id: selection.model_id,
            provider_id: Some(selection.provider_id),
            thinking_level: selection.thinking_level,
            skill_ids: None,
            session_mode: None,
            message: plan_phase_prompt(&plan, phase),
            team_mode_enabled: false,
            defer_start: true,
            attachments: Vec::new(),
            agent_definition_id: None,
            coordinator_execution_workspace_mode: AgentExecutionWorkspaceMode::IsolatedWorktree,
            origin: QueuedChatMessageOrigin::PlanPhase {
                plan_id: plan.id.clone(),
                phase_id: phase.id.clone(),
            },
        },
    )
    .await
    {
        Ok(queued) => queued,
        Err(error) => {
            let workspace = workspace_by_id(&config, workspace_id)?;
            let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            database
                .fail_plan_phase_start(&plan.id, &phase.id, &error.message)
                .map_err(ApiError::from_workspace_error)?;
            return Err(error);
        }
    };

    let team_id = queued
        .agent_team_id
        .as_ref()
        .ok_or_else(|| ApiError::internal("plan phase queue did not create an Agent team"))?;
    let task_id = queued
        .agent_task_id
        .as_ref()
        .ok_or_else(|| ApiError::internal("plan phase queue did not create an Agent task"))?;
    let workspace = workspace_by_id(&config, workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let plan = database
        .attach_plan_phase_run(&plan.id, &phase.id, &queued.chat_id, team_id, task_id)
        .map_err(ApiError::from_workspace_error)?;
    state.agent_scheduler.wake()?;
    Ok(plan)
}

fn merge_and_commit_plan_phase(
    workspace: &WorkspaceConfig,
    phase: &PlanPhaseRecord,
    instance: &AgentInstanceRecord,
) -> Result<Option<String>, ApiError> {
    if instance.execution_workspace_mode != AgentExecutionWorkspaceMode::IsolatedWorktree {
        return Err(ApiError::internal(format!(
            "plan phase '{}' did not run in an isolated worktree",
            phase.id
        )));
    }
    if instance.status != AgentInstanceStatus::Idle {
        return Err(ApiError::internal(format!(
            "plan phase '{}' Coordinator is not idle after task completion",
            phase.id
        )));
    }
    let root_path = instance.execution_root_path.as_deref().ok_or_else(|| {
        ApiError::internal(format!(
            "plan phase '{}' Coordinator is missing execution root",
            phase.id
        ))
    })?;
    let base_revision = instance.worktree_base_revision.as_deref().ok_or_else(|| {
        ApiError::internal(format!(
            "plan phase '{}' Coordinator is missing worktree base revision",
            phase.id
        ))
    })?;
    let merge = merge_agent_worktree(&workspace.path, Path::new(root_path), base_revision)?;
    if merge.changed_paths.is_empty() {
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        database
            .update_agent_instance_worktree_status(&instance.id, "kept")
            .map_err(ApiError::from_workspace_error)?;
        return Ok(None);
    }
    for path in &merge.changed_paths {
        stage_git_file(&workspace.path, path)?;
    }
    let commit_id = commit_staged_changes(
        &workspace.path,
        format!("plan: implement {}", phase.title.trim()),
    )?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    database
        .update_agent_instance_worktree_status(&instance.id, "kept")
        .map_err(ApiError::from_workspace_error)?;
    Ok(Some(commit_id))
}

fn plan_runner_model_selection(
    config: &GlobalConfig,
) -> Result<PlanRunnerModelSelection, ApiError> {
    for model in &config.models {
        if !model.enabled || !model_outputs_text(model) {
            continue;
        }
        let Some(provider_id) = model.active_provider_id.as_deref() else {
            continue;
        };
        let Some(provider) = config
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
        else {
            continue;
        };
        if provider.enabled {
            return Ok(PlanRunnerModelSelection {
                model_id: model.id.clone(),
                provider_id: provider_id.to_string(),
                thinking_level: model.thinking_level.clone(),
            });
        }
    }
    Err(ApiError::bad_request(
        "plan runner requires an enabled text-output model with an enabled active provider",
    ))
}

fn model_outputs_text(model: &ModelSettings) -> bool {
    model.output_modalities.is_empty()
        || model
            .output_modalities
            .iter()
            .any(|modality| modality == "text")
}

fn plan_phase_prompt(plan: &PlanRecord, phase: &PlanPhaseRecord) -> String {
    let mut message = format!(
        "Implement this plan phase in the isolated worktree. Do not create a git commit; Foco will merge and commit after the phase completes.\n\nPlan: {}\n\nOverview:\n{}\n\nPhase {}: {}\n\n{}",
        plan.title,
        plan.overview,
        phase.sequence + 1,
        phase.title,
        phase.summary
    );
    if !phase.steps.is_empty() {
        message.push_str("\n\nSteps:");
        for (index, step) in phase.steps.iter().enumerate() {
            message.push_str(&format!(
                "\n{}. {}\nDetail: {}",
                index + 1,
                step.title,
                step.detail
            ));
            if !step.acceptance.is_empty() {
                message.push_str("\nAcceptance:");
                for item in &step.acceptance {
                    message.push_str(&format!("\n- {item}"));
                }
            }
        }
    }
    message.push_str("\n\nWhen the phase is implemented, run the smallest relevant checks and finish with a concise summary.");
    message
}

fn agent_task_error_message(task: &AgentTaskRecord) -> String {
    task.error_json
        .as_deref()
        .and_then(|error_json| serde_json::from_str::<Value>(error_json).ok())
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| format!("Agent task finished with status '{}'", task.status.as_str()))
}
