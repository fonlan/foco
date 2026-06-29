use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use foco_agent::{
    AgentExecutionWorkspaceMode, AgentInstanceStatus, AgentTaskId, AgentTaskStatus, AgentTeamId,
};
use foco_store::{
    config::{
        GlobalConfig, ModelSettings, PLAN_MERGE_AUTOMATION_DIRECT_AUTO,
        PLAN_MERGE_AUTOMATION_ISOLATED_AUTO_ONCE, WorkspaceConfig,
    },
    workspace::{
        AgentInstanceRecord, AgentTaskRecord, PlanPhaseRecord, PlanRecord, WorkspaceDatabase,
    },
};
use serde_json::Value;

use crate::{
    git_backend::{
        AgentWorktreeInfo, agent_worktree_committed_diff, commit_staged_changes,
        delete_agent_worktree, fast_forward_shared_workspace_to_agent_worktree, git_diff_response,
        merge_agent_worktree, stage_git_file,
    },
    http::chat::{QueueChatMessageInput, QueuedChatMessageOrigin, queue_chat_message_internal},
    *,
};

const PLAN_MERGE_CORRELATION_PREFIX: &str = "plan_merge:";
const PLAN_MERGE_DIFF_MAX_CHARS: usize = 60_000;

struct PlanRunnerModelSelection {
    model_id: String,
    provider_id: String,
    thinking_level: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlanMergeTarget {
    plan_id: String,
    phase_id: String,
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
        let phase = database
            .plan_phase_for_agent_task(task_id)
            .map_err(ApiError::from_workspace_error)?;
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

    if let Some(target) = plan_merge_target_for_task(&task)? {
        sync_plan_merge_task(state, workspace, &target, &task, &instance).await?;
        return Ok(());
    }

    let Some(phase) = phase else {
        return Ok(());
    };

    match task.status {
        AgentTaskStatus::Completed => {
            let commit_id = match commit_plan_phase_to_worktree(&phase, &instance) {
                Ok(commit_id) => commit_id,
                Err(error) => {
                    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                        .map_err(ApiError::from_workspace_error)?;
                    database
                        .fail_plan_phase_by_id(&phase.plan_id, &phase.id, &error.message)
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

async fn sync_plan_merge_task(
    state: &AppState,
    workspace: &WorkspaceConfig,
    target: &PlanMergeTarget,
    task: &AgentTaskRecord,
    instance: &AgentInstanceRecord,
) -> Result<(), ApiError> {
    match task.status {
        AgentTaskStatus::Completed => {
            let phase = {
                let database = WorkspaceDatabase::open_or_create(&workspace.path)
                    .map_err(ApiError::from_workspace_error)?;
                database
                    .plan(&target.plan_id)
                    .map_err(ApiError::from_workspace_error)?
                    .and_then(|plan| {
                        plan.phases
                            .into_iter()
                            .find(|phase| phase.id == target.phase_id)
                    })
                    .ok_or_else(|| {
                        ApiError::internal(format!(
                            "plan merge target '{}:{}' was not found",
                            target.plan_id, target.phase_id
                        ))
                    })?
            };
            let commit_id = match instance.execution_workspace_mode {
                AgentExecutionWorkspaceMode::IsolatedWorktree => {
                    merge_and_commit_plan_phase(workspace, &phase, instance)
                }
                AgentExecutionWorkspaceMode::Shared => commit_direct_plan_merge(workspace, &phase),
            };
            let commit_id = match commit_id {
                Ok(commit_id) => commit_id,
                Err(error) => {
                    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                        .map_err(ApiError::from_workspace_error)?;
                    database
                        .fail_plan_phase_by_id(&target.plan_id, &target.phase_id, &error.message)
                        .map_err(ApiError::from_workspace_error)?;
                    return Ok(());
                }
            };
            let plan = {
                let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                    .map_err(ApiError::from_workspace_error)?;
                database
                    .complete_plan_phase_by_id(
                        &target.plan_id,
                        &target.phase_id,
                        commit_id.as_deref(),
                    )
                    .map_err(ApiError::from_workspace_error)?
            };
            if instance.execution_workspace_mode == AgentExecutionWorkspaceMode::IsolatedWorktree {
                delete_instance_worktree(workspace, instance, true)?;
            }
            delete_plan_worktrees(workspace, &plan, true)?;
            continue_plan_if_ready(state, workspace, plan).await?;
        }
        AgentTaskStatus::Failed | AgentTaskStatus::Cancelled | AgentTaskStatus::Interrupted => {
            let message = agent_task_error_message(task);
            let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            database
                .fail_plan_phase_by_id(&target.plan_id, &target.phase_id, &message)
                .map_err(ApiError::from_workspace_error)?;
        }
        AgentTaskStatus::Queued | AgentTaskStatus::Running | AgentTaskStatus::Waiting => {}
    }
    Ok(())
}

async fn dispatch_plan_merge(
    state: &AppState,
    workspace: &WorkspaceConfig,
    plan: &PlanRecord,
    phase: &PlanPhaseRecord,
    source_instance: &AgentInstanceRecord,
    merge_error: &ApiError,
) -> Result<bool, ApiError> {
    let source_diff = match plan_phase_source_diff(workspace, source_instance) {
        Ok(source_diff) => source_diff,
        Err(_) => return Ok(false),
    };
    let config = config_snapshot(state)?;
    let merge_mode = config.plan.merge_automation_mode.as_str();
    let execution_mode = match merge_mode {
        PLAN_MERGE_AUTOMATION_DIRECT_AUTO => AgentExecutionWorkspaceMode::Shared,
        PLAN_MERGE_AUTOMATION_ISOLATED_AUTO_ONCE => AgentExecutionWorkspaceMode::IsolatedWorktree,
        _ => {
            return Err(ApiError::bad_request(format!(
                "unsupported plan merge automation mode: {merge_mode}"
            )));
        }
    };
    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        if !database
            .try_begin_plan_phase_merge_attempt(&plan.id, &phase.id, &merge_error.message)
            .map_err(ApiError::from_workspace_error)?
        {
            return Ok(false);
        }
    }
    let selection = plan_runner_model_selection(&config)?;
    let queued = queue_chat_message_internal(
        state,
        &workspace.id,
        QueueChatMessageInput {
            chat_id: None,
            chat_title_override: None,
            model_id: selection.model_id,
            provider_id: Some(selection.provider_id),
            thinking_level: selection.thinking_level,
            skill_ids: None,
            session_mode: None,
            message: plan_merge_prompt(plan, phase, merge_mode, &merge_error.message, &source_diff),
            team_mode_enabled: false,
            defer_start: true,
            attachments: Vec::new(),
            agent_definition_id: None,
            coordinator_execution_workspace_mode: execution_mode,
            coordinator_worktree: None,
            correlation_id: Some(plan_merge_correlation_id(&plan.id, &phase.id)?),
            origin: QueuedChatMessageOrigin::PlanMerge {
                plan_id: plan.id.clone(),
                phase_id: phase.id.clone(),
            },
        },
    )
    .await;
    let queued = match queued {
        Ok(queued) => queued,
        Err(error) => {
            let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            database
                .fail_plan_phase_by_id(&plan.id, &phase.id, &error.message)
                .map_err(ApiError::from_workspace_error)?;
            return Err(error);
        }
    };
    let (team_id, task_id) = match (queued.agent_team_id.as_ref(), queued.agent_task_id.as_ref()) {
        (Some(team_id), Some(task_id)) => (team_id, task_id),
        (None, _) => {
            let error = ApiError::internal("plan merge queue did not create an Agent team");
            let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            database
                .fail_plan_phase_by_id(&plan.id, &phase.id, &error.message)
                .map_err(ApiError::from_workspace_error)?;
            return Err(error);
        }
        (_, None) => {
            let error = ApiError::internal("plan merge queue did not create an Agent task");
            let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                .map_err(ApiError::from_workspace_error)?;
            database
                .fail_plan_phase_by_id(&plan.id, &phase.id, &error.message)
                .map_err(ApiError::from_workspace_error)?;
            return Err(error);
        }
    };
    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        database
            .attach_plan_phase_merge_run(&plan.id, &phase.id, &queued.chat_id, team_id, task_id)
            .map_err(ApiError::from_workspace_error)?;
    }
    if source_instance.execution_workspace_mode == AgentExecutionWorkspaceMode::IsolatedWorktree
        && source_instance.worktree_status.as_deref() == Some("active")
    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        database
            .update_agent_instance_worktree_status(&source_instance.id, "kept")
            .map_err(ApiError::from_workspace_error)?;
    }
    state.agent_scheduler.wake()?;
    Ok(true)
}

async fn continue_plan_if_ready(
    state: &AppState,
    workspace: &WorkspaceConfig,
    plan: PlanRecord,
) -> Result<(), ApiError> {
    match plan.status.as_str() {
        "ready" => {
            let _ = transition_plan_action(state, &workspace.id, &plan.id, "resume").await?;
        }
        "implemented" => {
            finalize_plan_worktree(state, workspace, &plan).await?;
        }
        _ => {}
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
    let workspace = workspace_by_id(&config, workspace_id)?;
    let coordinator_worktree = {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        plan_worktree_info(&database, &plan)?
    };
    let queued = match queue_chat_message_internal(
        state,
        workspace_id,
        QueueChatMessageInput {
            chat_id: None,
            chat_title_override: Some(plan_phase_chat_title(&plan.title, &phase.title)),
            model_id: selection.model_id,
            provider_id: Some(selection.provider_id),
            thinking_level: selection.thinking_level,
            skill_ids: None,
            session_mode: None,
            message: plan_phase_prompt(&plan, phase),
            team_mode_enabled: true,
            defer_start: true,
            attachments: Vec::new(),
            agent_definition_id: None,
            coordinator_execution_workspace_mode: AgentExecutionWorkspaceMode::IsolatedWorktree,
            coordinator_worktree,
            correlation_id: None,
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
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let plan = database
        .attach_plan_phase_run(&plan.id, &phase.id, &queued.chat_id, team_id, task_id)
        .map_err(ApiError::from_workspace_error)?;
    state.agent_scheduler.wake()?;
    Ok(plan)
}

async fn finalize_plan_worktree(
    state: &AppState,
    workspace: &WorkspaceConfig,
    plan: &PlanRecord,
) -> Result<(), ApiError> {
    let (phase, instance) = {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        let Some(source) = plan_worktree_source(&database, plan)? else {
            return Ok(());
        };
        source
    };
    let root_path = instance.execution_root_path.as_deref().ok_or_else(|| {
        ApiError::internal(format!(
            "plan '{}' worktree Coordinator is missing execution root",
            plan.id
        ))
    })?;
    let base_revision = instance.worktree_base_revision.as_deref().ok_or_else(|| {
        ApiError::internal(format!(
            "plan '{}' worktree Coordinator is missing base revision",
            plan.id
        ))
    })?;
    match fast_forward_shared_workspace_to_agent_worktree(
        &workspace.path,
        Path::new(root_path),
        base_revision,
    ) {
        Ok(_) => delete_plan_worktrees(workspace, plan, true),
        Err(error) => {
            if dispatch_plan_merge(state, workspace, plan, &phase, &instance, &error).await? {
                Ok(())
            } else {
                let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
                    .map_err(ApiError::from_workspace_error)?;
                database
                    .fail_plan_phase_by_id(&phase.plan_id, &phase.id, &error.message)
                    .map_err(ApiError::from_workspace_error)?;
                Ok(())
            }
        }
    }
}

fn commit_plan_phase_to_worktree(
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
    commit_workspace_changes(
        Path::new(root_path),
        format!("plan: implement {}", phase.title.trim()),
    )
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

fn commit_workspace_changes(
    workspace_path: &Path,
    message: String,
) -> Result<Option<String>, ApiError> {
    let diff = git_diff_response(workspace_path, None)?;
    let changed_paths = diff
        .files
        .iter()
        .chain(diff.staged_files.iter())
        .map(|file| file.path.trim())
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if changed_paths.is_empty() {
        return Ok(None);
    }
    for path in &changed_paths {
        stage_git_file(workspace_path, path)?;
    }
    let staged = git_diff_response(workspace_path, None)?;
    if staged.staged_files.is_empty() {
        return Ok(None);
    }
    commit_staged_changes(workspace_path, message).map(Some)
}

fn plan_worktree_info(
    database: &WorkspaceDatabase,
    plan: &PlanRecord,
) -> Result<Option<AgentWorktreeInfo>, ApiError> {
    let Some((_, instance)) = plan_worktree_source(database, plan)? else {
        return Ok(None);
    };
    let root_path = instance.execution_root_path.as_deref().ok_or_else(|| {
        ApiError::internal(format!(
            "plan '{}' worktree Coordinator is missing execution root",
            plan.id
        ))
    })?;
    let base_revision = instance.worktree_base_revision.as_deref().ok_or_else(|| {
        ApiError::internal(format!(
            "plan '{}' worktree Coordinator is missing base revision",
            plan.id
        ))
    })?;
    let branch = instance.worktree_branch.as_deref().ok_or_else(|| {
        ApiError::internal(format!(
            "plan '{}' worktree Coordinator is missing branch",
            plan.id
        ))
    })?;
    Ok(Some(AgentWorktreeInfo {
        root_path: PathBuf::from(root_path),
        base_revision: base_revision.to_string(),
        branch: branch.to_string(),
    }))
}

fn plan_worktree_source(
    database: &WorkspaceDatabase,
    plan: &PlanRecord,
) -> Result<Option<(PlanPhaseRecord, AgentInstanceRecord)>, ApiError> {
    for phase in plan.phases.iter().rev() {
        let Some(instance) = plan_phase_coordinator_instance(database, phase)? else {
            continue;
        };
        if instance.execution_workspace_mode == AgentExecutionWorkspaceMode::IsolatedWorktree
            && instance.worktree_status.as_deref() != Some("deleted")
        {
            return Ok(Some((phase.clone(), instance)));
        }
    }
    Ok(None)
}

fn plan_worktree_instances(
    database: &WorkspaceDatabase,
    plan: &PlanRecord,
) -> Result<Vec<AgentInstanceRecord>, ApiError> {
    let mut seen = BTreeSet::new();
    let mut instances = Vec::new();
    for phase in &plan.phases {
        let Some(instance) = plan_phase_coordinator_instance(database, phase)? else {
            continue;
        };
        if instance.execution_workspace_mode == AgentExecutionWorkspaceMode::IsolatedWorktree
            && seen.insert(instance.id.to_string())
        {
            instances.push(instance);
        }
    }
    Ok(instances)
}

fn plan_phase_coordinator_instance(
    database: &WorkspaceDatabase,
    phase: &PlanPhaseRecord,
) -> Result<Option<AgentInstanceRecord>, ApiError> {
    let Some(team_id) = phase.agent_team_id.as_deref() else {
        return Ok(None);
    };
    let team_id = AgentTeamId::new(team_id.to_string())
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let Some(team) = database
        .agent_team(&team_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Ok(None);
    };
    database
        .agent_instance(&team.coordinator_instance_id)
        .map_err(ApiError::from_workspace_error)
}

fn delete_plan_worktrees(
    workspace: &WorkspaceConfig,
    plan: &PlanRecord,
    allow_changes: bool,
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let instances = plan_worktree_instances(&database, plan)?;
    let mut deleted_roots = BTreeSet::new();
    for instance in instances {
        if let Some(root_path) = instance.execution_root_path.as_deref() {
            if deleted_roots.insert(root_path.to_string()) {
                delete_agent_worktree(&workspace.path, Path::new(root_path), allow_changes)?;
            }
        }
        database
            .switch_agent_instance_to_shared_workspace(&instance.id)
            .map_err(ApiError::from_workspace_error)?;
    }
    Ok(())
}

fn delete_instance_worktree(
    workspace: &WorkspaceConfig,
    instance: &AgentInstanceRecord,
    allow_changes: bool,
) -> Result<(), ApiError> {
    let Some(root_path) = instance.execution_root_path.as_deref() else {
        return Ok(());
    };
    delete_agent_worktree(&workspace.path, Path::new(root_path), allow_changes)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    database
        .switch_agent_instance_to_shared_workspace(&instance.id)
        .map_err(ApiError::from_workspace_error)?;
    Ok(())
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

fn plan_phase_chat_title(plan_title: &str, phase_title: &str) -> String {
    format!("{plan_title} - {phase_title}")
}

fn plan_phase_prompt(plan: &PlanRecord, phase: &PlanPhaseRecord) -> String {
    let mut message = format!(
        "Implement this plan phase in the plan's isolated worktree. Do not create a git commit; Foco will commit this phase in the worktree after the phase completes, and later phases will continue from that commit. Foco merges the worktree back to the shared workspace only after all phases complete.\n\nPlan: {}\n\nOverview:\n{}\n\nPhase {}: {}\n\n{}",
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

fn plan_merge_correlation_id(plan_id: &str, phase_id: &str) -> Result<String, ApiError> {
    let plan_id = plan_id.trim();
    let phase_id = phase_id.trim();
    if plan_id.is_empty() || phase_id.is_empty() {
        return Err(ApiError::internal(
            "plan merge correlation requires non-empty plan and phase ids",
        ));
    }
    let target = serde_json::to_string(&(plan_id, phase_id)).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize plan merge correlation id: {source}"
        ))
    })?;
    Ok(format!("{PLAN_MERGE_CORRELATION_PREFIX}{target}"))
}

fn plan_merge_target_for_task(task: &AgentTaskRecord) -> Result<Option<PlanMergeTarget>, ApiError> {
    let value = serde_json::from_str::<Value>(&task.input_json).map_err(|source| {
        ApiError::internal(format!("failed to parse Agent task input: {source}"))
    })?;
    let Some(correlation_id) = value
        .get("correlationId")
        .or_else(|| value.get("correlation_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let Some(target) = correlation_id.strip_prefix(PLAN_MERGE_CORRELATION_PREFIX) else {
        return Ok(None);
    };
    if target.starts_with('[') {
        let (plan_id, phase_id) =
            serde_json::from_str::<(String, String)>(target).map_err(|source| {
                ApiError::internal(format!(
                    "invalid plan merge correlation id '{correlation_id}': {source}"
                ))
            })?;
        if plan_id.trim().is_empty() || phase_id.trim().is_empty() {
            return Err(ApiError::internal(format!(
                "invalid plan merge correlation id '{correlation_id}'"
            )));
        }
        return Ok(Some(PlanMergeTarget {
            plan_id: plan_id.trim().to_string(),
            phase_id: phase_id.trim().to_string(),
        }));
    }
    let Some((plan_id, phase_id)) = target.split_once(':') else {
        return Err(ApiError::internal(format!(
            "invalid plan merge correlation id '{correlation_id}'"
        )));
    };
    if phase_id.contains(':') || plan_id.trim().is_empty() || phase_id.trim().is_empty() {
        return Err(ApiError::internal(format!(
            "invalid plan merge correlation id '{correlation_id}'"
        )));
    }
    Ok(Some(PlanMergeTarget {
        plan_id: plan_id.to_string(),
        phase_id: phase_id.to_string(),
    }))
}

fn plan_phase_source_diff(
    workspace: &WorkspaceConfig,
    instance: &AgentInstanceRecord,
) -> Result<String, ApiError> {
    let root_path = instance.execution_root_path.as_deref().ok_or_else(|| {
        ApiError::internal(format!(
            "Agent instance '{}' is missing execution root",
            instance.id
        ))
    })?;
    let diff = git_diff_response(Path::new(root_path), None)?;
    let committed_diff = instance
        .worktree_base_revision
        .as_deref()
        .map(|base_revision| {
            agent_worktree_committed_diff(&workspace.path, Path::new(root_path), base_revision)
        })
        .transpose()?
        .unwrap_or_default();
    let source = format!(
        "Committed diff from plan worktree base to HEAD:\n{}\n\nGit status:\n{}\n\nUnstaged diff:\n{}\n\nStaged diff:\n{}",
        committed_diff.trim_end(),
        diff.status.trim_end(),
        diff.diff.trim_end(),
        diff.staged_diff.trim_end()
    );
    Ok(truncate_for_prompt(&source, PLAN_MERGE_DIFF_MAX_CHARS))
}

fn plan_merge_prompt(
    plan: &PlanRecord,
    phase: &PlanPhaseRecord,
    merge_mode: &str,
    error_message: &str,
    source_diff: &str,
) -> String {
    let workspace_instruction = if merge_mode == PLAN_MERGE_AUTOMATION_DIRECT_AUTO {
        "You are running in the shared workspace. Apply the needed merge resolution directly in this workspace. Do not create a git commit; Foco will stage and commit after this task completes."
    } else {
        "You are running in a fresh isolated worktree based on the current shared workspace. Recreate the intended phase changes from the source diff. Do not create a git commit; Foco will merge and commit after this task completes."
    };
    let mut message = format!(
        "Resolve this failed automated plan phase merge.\n\n{workspace_instruction}\n\nPlan: {}\n\nOverview:\n{}\n\nPhase {}: {}\n\n{}\n\nMerge failure:\n{}\n\nSource worktree diff:\n{}",
        plan.title,
        plan.overview,
        phase.sequence + 1,
        phase.title,
        phase.summary,
        error_message.trim(),
        source_diff
    );
    if !phase.steps.is_empty() {
        message.push_str("\n\nPhase steps:");
        for (index, step) in phase.steps.iter().enumerate() {
            message.push_str(&format!(
                "\n{}. {}\nDetail: {}",
                index + 1,
                step.title,
                step.detail
            ));
        }
    }
    message.push_str("\n\nRun the smallest relevant checks and finish with a concise summary.");
    message
}

fn commit_direct_plan_merge(
    workspace: &WorkspaceConfig,
    phase: &PlanPhaseRecord,
) -> Result<Option<String>, ApiError> {
    commit_workspace_changes(
        &workspace.path,
        format!("plan: resolve merge for {}", phase.title.trim()),
    )
}

fn truncate_for_prompt(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n\n[truncated to {max_bytes} bytes for the merge prompt]",
        &value[..end]
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_phase_chat_title_uses_plan_and_phase_titles() {
        assert_eq!(
            plan_phase_chat_title("Build plan runner UI", "Wire start action"),
            "Build plan runner UI - Wire start action"
        );
    }

    fn task_with_input(input_json: &str) -> AgentTaskRecord {
        AgentTaskRecord {
            id: AgentTaskId::new("agent-task-plan-merge-test").expect("task id"),
            team_id: foco_agent::AgentTeamId::new("agent-team-plan-merge-test").expect("team id"),
            owner_instance_id: foco_agent::AgentInstanceId::new("agent-instance-plan-merge-test")
                .expect("instance id"),
            origin_instance_id: None,
            parent_task_id: None,
            sequence: 1,
            status: AgentTaskStatus::Completed,
            input_json: input_json.to_string(),
            result_json: None,
            error_json: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            started_at: None,
            completed_at: None,
        }
    }

    #[test]
    fn plan_merge_correlation_round_trips_target() {
        let correlation_id =
            plan_merge_correlation_id("plan:merge", "phase:one").expect("correlation id");
        let input_json = serde_json::json!({ "correlationId": correlation_id }).to_string();
        let task = task_with_input(&input_json);
        let target = plan_merge_target_for_task(&task)
            .expect("parse target")
            .expect("target");

        assert_eq!(
            target,
            PlanMergeTarget {
                plan_id: "plan:merge".to_string(),
                phase_id: "phase:one".to_string(),
            }
        );
    }

    #[test]
    fn plan_merge_target_ignores_non_merge_correlation() {
        let task = task_with_input(r#"{"correlationId":"delegated-task"}"#);

        assert_eq!(
            plan_merge_target_for_task(&task).expect("parse target"),
            None
        );
    }
}
