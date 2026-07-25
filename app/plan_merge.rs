use std::path::Path;

use foco_store::{
    config::{PLAN_MERGE_AUTOMATION_DIRECT_AUTO, PLAN_MERGE_AUTOMATION_ISOLATED_AUTO_ONCE},
    workspace::{PlanPhaseRecord, PlanRecord},
};

use crate::{
    ApiError,
    git_backend::{
        AGENT_WORKTREE_SHARED_DIRTY_MESSAGE, AGENT_WORKTREE_SHARED_HEAD_MISMATCH_MESSAGE,
        agent_worktree_committed_diff, git_diff_response,
    },
};

pub(crate) const PLAN_MERGE_DIFF_MAX_CHARS: usize = 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlanMergeFailureKind {
    SharedWorkspaceDirty,
    SharedHeadMismatch,
    Other,
}

pub(crate) fn classify_plan_merge_failure(error: &ApiError) -> PlanMergeFailureKind {
    if error.message.contains(AGENT_WORKTREE_SHARED_DIRTY_MESSAGE) {
        PlanMergeFailureKind::SharedWorkspaceDirty
    } else if error
        .message
        .contains(AGENT_WORKTREE_SHARED_HEAD_MISMATCH_MESSAGE)
    {
        PlanMergeFailureKind::SharedHeadMismatch
    } else {
        PlanMergeFailureKind::Other
    }
}

pub(crate) fn plan_phase_source_diff(
    workspace_path: &Path,
    source_worktree_path: &Path,
    base_revision: &str,
) -> Result<String, ApiError> {
    let diff = git_diff_response(source_worktree_path, None)?;
    let committed_diff =
        agent_worktree_committed_diff(workspace_path, source_worktree_path, base_revision)?;
    let source = format!(
        "Committed diff from plan worktree base to HEAD:\n{}\n\nGit status:\n{}\n\nUnstaged diff:\n{}\n\nStaged diff:\n{}",
        committed_diff.trim_end(),
        diff.status.trim_end(),
        diff.diff.trim_end(),
        diff.staged_diff.trim_end()
    );
    Ok(truncate_for_prompt(&source, PLAN_MERGE_DIFF_MAX_CHARS))
}

pub(crate) fn plan_merge_prompt(
    plan: &PlanRecord,
    phase: &PlanPhaseRecord,
    merge_mode: &str,
    error_message: &str,
    source_diff: &str,
) -> String {
    let workspace_instruction = if merge_mode == PLAN_MERGE_AUTOMATION_DIRECT_AUTO {
        "You are running in the shared workspace. Apply the needed merge resolution directly in this workspace. Do not create a git commit; Foco will stage and commit after this task completes."
    } else {
        debug_assert_eq!(merge_mode, PLAN_MERGE_AUTOMATION_ISOLATED_AUTO_ONCE);
        "You are running in a fresh isolated worktree based on the current shared workspace. Recreate the intended phase changes from the source diff. Do not create a git commit; Foco will merge and commit after this task completes."
    };
    let mut message = format!(
        "Resolve this failed automated plan phase merge.\n\n{workspace_instruction}\n\nPlan: {}\n\nOverview:\n{}\n\nPhase {}: {}\n\n{}\n\nMerge failure:\n{}\n\nSource worktree diff:\n```diff\n{}\n```",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_plan_merge_failure_keeps_head_mismatch_distinct_from_dirty_workspace() {
        let mismatch = ApiError::bad_request(format!(
            "shared workspace HEAD 'new' {AGENT_WORKTREE_SHARED_HEAD_MISMATCH_MESSAGE} 'old'"
        ));
        let dirty = ApiError::bad_request(AGENT_WORKTREE_SHARED_DIRTY_MESSAGE);

        assert_eq!(
            classify_plan_merge_failure(&mismatch),
            PlanMergeFailureKind::SharedHeadMismatch
        );
        assert_eq!(
            classify_plan_merge_failure(&dirty),
            PlanMergeFailureKind::SharedWorkspaceDirty
        );
    }
}
/// Controls chat lifecycle side effects for durable Plan merge sessions.
///
/// A merge Coordinator has no user-authored implementation result of its own:
/// its successful output only integrates an earlier Plan phase. Keeping this
/// policy distinct prevents merge prompts from being treated as a second source
/// for memory and Workspace Spec derivation while preserving the implementation
/// phase's separately persisted effects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PlanMergeChatLifecycle {
    #[default]
    Standard,
    PlanMerge,
}

impl PlanMergeChatLifecycle {
    pub(crate) fn allows_memory_retrieval(self) -> bool {
        matches!(self, Self::Standard)
    }

    pub(crate) fn allows_derived_effects(self) -> bool {
        matches!(self, Self::Standard)
    }

    pub(crate) fn from_local_correlation(correlation_id: Option<&str>) -> Self {
        correlation_id
            .is_some_and(|value| value.trim_start().starts_with("plan_merge:"))
            .then_some(Self::PlanMerge)
            .unwrap_or(Self::Standard)
    }
}
#[cfg(test)]
mod lifecycle_tests {
    use super::PlanMergeChatLifecycle;

    #[test]
    fn local_plan_merge_correlation_disables_chat_memory_and_derived_effects() {
        let lifecycle =
            PlanMergeChatLifecycle::from_local_correlation(Some("plan_merge:plan-1:phase-1"));

        assert!(!lifecycle.allows_memory_retrieval());
        assert!(!lifecycle.allows_derived_effects());
    }

    #[test]
    fn ordinary_and_non_merge_correlations_keep_chat_lifecycle_effects() {
        for correlation in [None, Some("plan_phase:plan-1:phase-1"), Some("user-action")] {
            let lifecycle = PlanMergeChatLifecycle::from_local_correlation(correlation);

            assert!(lifecycle.allows_memory_retrieval());
            assert!(lifecycle.allows_derived_effects());
        }
    }
}
