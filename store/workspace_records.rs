use serde::{Deserialize, Serialize};
use serde_json::Value;

use foco_agent::{
    AgentAttemptId, AgentAttemptStatus, AgentDefinitionId, AgentExecutionWorkspaceMode,
    AgentInstanceId, AgentInstanceStatus, AgentMessageId, AgentMessageKind, AgentRole, AgentTaskId,
    AgentTaskStatus, AgentTaskTransition, AgentTaskWaitMode, AgentTeamId, AgentTeamStatus,
};

use crate::{config::AgentDefinitionSettings, workspace::WorkspaceDatabaseError};

#[derive(Clone, Debug)]
pub struct NewAgentTeam<'a> {
    pub id: &'a AgentTeamId,
    pub chat_id: &'a str,
    pub coordinator_instance_id: &'a AgentInstanceId,
    pub coordinator_definition: &'a AgentDefinitionSettings,
    pub coordinator_execution_workspace_mode: AgentExecutionWorkspaceMode,
    pub coordinator_execution_root_path: Option<&'a str>,
    pub coordinator_worktree_base_revision: Option<&'a str>,
    pub coordinator_worktree_branch: Option<&'a str>,
    pub coordinator_worktree_status: Option<&'a str>,
    pub max_concurrent_runs: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentTeamRecord {
    pub id: AgentTeamId,
    pub chat_id: String,
    pub coordinator_instance_id: AgentInstanceId,
    pub status: AgentTeamStatus,
    pub max_concurrent_runs: i64,
    pub next_event_sequence: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct NewAgentInstance<'a> {
    pub id: &'a AgentInstanceId,
    pub team_id: &'a AgentTeamId,
    pub definition: &'a AgentDefinitionSettings,
    pub role: AgentRole,
    pub execution_workspace_mode: AgentExecutionWorkspaceMode,
    pub execution_root_path: Option<&'a str>,
    pub worktree_base_revision: Option<&'a str>,
    pub worktree_branch: Option<&'a str>,
    pub worktree_status: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentInstanceRecord {
    pub id: AgentInstanceId,
    pub team_id: AgentTeamId,
    pub definition_id: AgentDefinitionId,
    pub definition_revision: u64,
    pub definition_snapshot: AgentDefinitionSettings,
    pub role: AgentRole,
    pub status: AgentInstanceStatus,
    pub next_task_sequence: i64,
    pub next_message_sequence: i64,
    pub context_generation: i64,
    pub last_scheduled_at: Option<String>,
    pub execution_workspace_mode: AgentExecutionWorkspaceMode,
    pub execution_root_path: Option<String>,
    pub worktree_base_revision: Option<String>,
    pub worktree_branch: Option<String>,
    pub worktree_status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanAutoRunStateRecord {
    /// Backward-compatible effective state: desired and not runtime-blocked.
    pub enabled: bool,
    pub desired_enabled: bool,
    pub busy: bool,
    pub blocked_reason: Option<String>,
    pub blocked_plan_id: Option<String>,
    pub blocked_phase_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanAutoRunCandidateRecord {
    pub plan_id: String,
    pub action: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanAutoRunSelection {
    Candidate(PlanAutoRunCandidateRecord),
    WaitingForReady {
        plan_id: String,
    },
    WaitingForRetry {
        plan_id: String,
        phase_id: Option<String>,
    },
    BlockedByCancelledPhase {
        plan_id: String,
        phase_id: String,
    },
    /// A user pause is a scheduling gate, including while the active Phase keeps running.
    Paused {
        plan_id: String,
        phase_id: Option<String>,
    },
    Running {
        plan_id: String,
        phase_id: Option<String>,
    },
    Idle,
}

#[derive(Clone, Debug)]
pub struct NewAgentTask<'a> {
    pub id: &'a AgentTaskId,
    pub team_id: &'a AgentTeamId,
    pub owner_instance_id: &'a AgentInstanceId,
    pub origin_instance_id: Option<&'a AgentInstanceId>,
    pub parent_task_id: Option<&'a AgentTaskId>,
    pub input_json: &'a str,
}

#[derive(Clone, Debug)]
pub struct NewChatSpecSnapshot<'a> {
    pub revision: u64,
    pub content_markdown: &'a str,
}

/// The durable state that makes a primary chat run observable to readers.
///
/// A coordinator task is deliberately part of the same insertion: readers may
/// observe `queuedRun` only after the user message and its owning task exist.
#[derive(Clone, Debug)]
pub struct QueueCoordinatorChatMessage<'a> {
    pub chat_id: &'a str,
    pub new_chat_title: Option<&'a str>,
    pub new_chat_metadata_json: Option<&'a str>,
    pub user_message: NewMessage<'a>,
    pub chat_queued_run_json: &'a str,
    pub chat_spec_snapshot: Option<NewChatSpecSnapshot<'a>>,
    pub prompt_context_injections: Vec<NewPromptContextInjection<'a>>,
    pub new_team: Option<NewAgentTeam<'a>>,
    pub task: NewAgentTask<'a>,
    pub max_team_queued: i64,
    pub max_instance_queued: i64,
    pub max_chat_queued: i64,
    pub task_queued_payload_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentTaskRecord {
    pub id: AgentTaskId,
    pub team_id: AgentTeamId,
    pub owner_instance_id: AgentInstanceId,
    pub origin_instance_id: Option<AgentInstanceId>,
    pub parent_task_id: Option<AgentTaskId>,
    pub sequence: i64,
    pub status: AgentTaskStatus,
    pub input_json: String,
    pub result_json: Option<String>,
    pub error_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// The durable and runtime-facing effects of cancelling an Agent task subtree.
///
/// Running tasks remain owned by their active runs so their normal cancellation
/// path can close them without replacing a more specific terminal cause.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentTaskSubtreeCancellation {
    /// Running tasks whose active-run cancellation token must be notified.
    pub running_task_ids: Vec<AgentTaskId>,
    /// Queued or waiting tasks transitioned to `cancelled` by this transaction.
    pub cancelled_tasks: Vec<AgentTaskRecord>,
}

#[derive(Clone, Debug)]
pub struct AgentTaskStateUpdate<'a> {
    pub team_id: &'a AgentTeamId,
    pub task_id: &'a AgentTaskId,
    pub expected_status: AgentTaskStatus,
    pub transition: AgentTaskTransition,
    pub result_json: Option<&'a str>,
    pub error_json: Option<&'a str>,
    pub interruption_reason: Option<&'a str>,
}

/// Atomic pre-stream coordinator failure: task fail + optional assistant error bubble + queuedRun clear.
#[derive(Clone, Debug)]
pub struct PreStreamChatFailureClosure<'a> {
    pub task_id: &'a AgentTaskId,
    pub attempt_id: &'a AgentAttemptId,
    pub chat_id: &'a str,
    pub user_message_id: &'a str,
    pub assistant_message_id: &'a str,
    pub assistant_sequence: i64,
    pub error_json: &'a str,
    /// Recovery closures use this to preserve the durable recovery diagnostics
    /// on their `attempt_interrupted` event. Ordinary pre-stream failures leave
    /// it unset and retain their existing `task_failed` event shape.
    pub interruption_event_payload_json: Option<&'a str>,
    pub interruption_reason: Option<&'a str>,
    pub assistant_content: &'a str,
    pub assistant_metadata_json: &'a str,
    /// Optional CAS snapshot for recovery paths. When provided, the active
    /// attempt must still have this exact owner/lease pair before it can close.
    /// Normal pre-stream failures leave these unset.
    pub expected_attempt_owner_incarnation: Option<&'a str>,
    pub expected_attempt_lease_renewed_at: Option<&'a str>,
    /// Optional durable queuedRun owner for remote Plan recovery. When set,
    /// the queued user message must still belong to this exact task/run pair
    /// before any lifecycle state is terminalized.
    pub expected_queued_run_agent_task_id: Option<&'a AgentTaskId>,
    pub expected_queued_run_id: Option<&'a str>,
    /// When false (worker/subagent), only task/event close; no main-chat assistant row.
    pub materialize_assistant: bool,
}

/// Typed assistant/message metadata mutations applied inside one Immediate transaction.
///
/// Unrelated top-level keys are preserved across concurrent mutations; callers must not
/// read-modify-write whole metadata columns via bare `update_message_metadata`.
#[derive(Clone, Debug)]
pub enum MessageMetadataMutation {
    /// Shallow-merge top-level object fields into existing metadata.
    MergeFields {
        fields: serde_json::Map<String, Value>,
    },
    /// Merge top-level fields and fields of an existing nested object in one transaction.
    /// The nested update is a no-op when `key` is missing or null; it errors when present but not an object.
    MergeFieldsAndNestedObjectFields {
        fields: serde_json::Map<String, Value>,
        key: String,
        nested_fields: serde_json::Map<String, Value>,
    },
    /// Replace `parts` / `partsVersion` / `partsSource` without touching other keys.
    SetParts {
        parts: Value,
        parts_version: i64,
        parts_source: String,
    },
    /// Upsert one object into `specUpdates[]` by `id` (replace matching id or append).
    UpsertSpecUpdate { summary: Value },
    /// Remove a single top-level key when present (idempotent if missing).
    RemoveKey { key: String },
    /// Merge fields into an existing nested object under `key`.
    /// No-op when the key is missing or null; errors when present but not an object.
    MergeNestedObjectFields {
        key: String,
        fields: serde_json::Map<String, Value>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreStreamChatFailureClosureResult {
    Applied,
    Skipped { reason: String },
}

/// One historical pre-stream failure healed into a durable assistant Error bubble.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreStreamFailureMaterialization {
    pub task_id: AgentTaskId,
    pub assistant_message_id: String,
    pub assistant_sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentAttemptRecord {
    pub id: AgentAttemptId,
    pub team_id: AgentTeamId,
    pub task_id: AgentTaskId,
    pub sequence: i64,
    pub status: AgentAttemptStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub interruption_reason: Option<String>,
    /// Scheduler process incarnation currently allowed to renew this attempt.
    /// Legacy attempts have no owner and are conservatively treated as abandoned.
    pub owner_incarnation: Option<String>,
    /// Last durable liveness renewal for `owner_incarnation`.
    pub lease_renewed_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentAttemptRecoveryDisposition {
    /// No owner/lease was written by an older build, so no live coordinator can
    /// be verified and normal startup interruption remains safe.
    VerifiedAbandonedLegacy,
    /// A coordinator has recently renewed its durable lease.
    LeaseActive,
    /// The owner was known, but its durable renewal expired.
    VerifiedAbandonedLeaseExpired,
    /// Malformed owner/lease state is never treated as live.
    VerifiedAbandonedInvalidLease,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentReconciliationRecord {
    pub attempt: AgentAttemptRecord,
    pub task: AgentTaskRecord,
}

#[derive(Clone, Debug)]
pub struct NewAgentTaskDependency<'a> {
    pub team_id: &'a AgentTeamId,
    pub waiting_task_id: &'a AgentTaskId,
    pub dependency_task_id: &'a AgentTaskId,
    pub wait_mode: AgentTaskWaitMode,
    pub pending_tool_call_id: Option<&'a str>,
    pub deadline_at: Option<&'a str>,
}

/// Atomic registration of a full `agent_wait_tasks` dependency set for one wait round.
///
/// `pending_tool_call_id` identifies the wait round. Same-round retries are idempotent
/// (and may repair a partial legacy multi-row write). A different round may replace the
/// previous set only when every prior dependency task is terminal.
#[derive(Clone, Debug)]
pub struct RegisterAgentTaskWaitDependencies<'a> {
    pub team_id: &'a AgentTeamId,
    pub waiting_task_id: &'a AgentTaskId,
    pub dependency_task_ids: &'a [AgentTaskId],
    pub wait_mode: AgentTaskWaitMode,
    pub pending_tool_call_id: Option<&'a str>,
    pub deadline_at: Option<&'a str>,
    /// Optional instance stamped on the `task_waiting_requested` event.
    pub event_instance_id: Option<&'a AgentInstanceId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentTaskWaitRegistrationOutcome {
    /// First durable registration for this wait round.
    Created,
    /// Exact same wait round already present; no dependency rows changed.
    Replayed,
    /// Same wait round metadata; missing dependency rows were inserted (legacy partial write).
    Repaired,
    /// Prior wait round was fully terminal and replaced by this registration.
    Replaced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentTaskDependencyRecord {
    pub team_id: AgentTeamId,
    pub waiting_task_id: AgentTaskId,
    pub dependency_task_id: AgentTaskId,
    pub wait_mode: AgentTaskWaitMode,
    pub pending_tool_call_id: Option<String>,
    pub deadline_at: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct NewAgentMessage<'a> {
    pub id: &'a AgentMessageId,
    pub team_id: &'a AgentTeamId,
    pub sender_instance_id: Option<&'a AgentInstanceId>,
    pub receiver_instance_id: &'a AgentInstanceId,
    pub related_task_id: Option<&'a AgentTaskId>,
    pub reply_to_message_id: Option<&'a AgentMessageId>,
    pub kind: AgentMessageKind,
    pub content: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentMessageRecord {
    pub id: AgentMessageId,
    pub team_id: AgentTeamId,
    pub sender_instance_id: Option<AgentInstanceId>,
    pub receiver_instance_id: AgentInstanceId,
    pub related_task_id: Option<AgentTaskId>,
    pub reply_to_message_id: Option<AgentMessageId>,
    pub kind: AgentMessageKind,
    pub content: String,
    pub sequence: i64,
    pub created_at: String,
    pub consumed_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewAgentEvent<'a> {
    pub team_id: &'a AgentTeamId,
    pub event_type: &'a str,
    pub instance_id: Option<&'a AgentInstanceId>,
    pub task_id: Option<&'a AgentTaskId>,
    pub attempt_id: Option<&'a AgentAttemptId>,
    pub message_id: Option<&'a AgentMessageId>,
    pub payload_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentEventRecord {
    pub team_id: AgentTeamId,
    pub sequence: i64,
    pub event_type: String,
    pub instance_id: Option<AgentInstanceId>,
    pub task_id: Option<AgentTaskId>,
    pub attempt_id: Option<AgentAttemptId>,
    pub message_id: Option<AgentMessageId>,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct NewAgentContextEntry<'a> {
    pub id: &'a str,
    pub team_id: &'a AgentTeamId,
    pub instance_id: &'a AgentInstanceId,
    pub generation: i64,
    pub sequence: i64,
    pub role: &'a str,
    pub content_json: &'a str,
    pub source_task_id: Option<&'a AgentTaskId>,
    pub source_message_id: Option<&'a AgentMessageId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentContextEntryRecord {
    pub id: String,
    pub team_id: AgentTeamId,
    pub instance_id: AgentInstanceId,
    pub generation: i64,
    pub sequence: i64,
    pub role: String,
    pub content_json: String,
    pub source_task_id: Option<AgentTaskId>,
    pub source_message_id: Option<AgentMessageId>,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct NewAgentContextSnapshot<'a> {
    pub id: &'a str,
    pub team_id: &'a AgentTeamId,
    pub instance_id: &'a AgentInstanceId,
    pub generation: i64,
    pub sequence: i64,
    pub entries_json: &'a str,
    pub token_count: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentContextSnapshotRecord {
    pub id: String,
    pub team_id: AgentTeamId,
    pub instance_id: AgentInstanceId,
    pub generation: i64,
    pub sequence: i64,
    pub entries_json: String,
    pub token_count: Option<i64>,
    pub created_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduledTaskListFilter<'a> {
    pub status: Option<&'a str>,
    pub search: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledTaskStatusCountRecord {
    pub status: String,
    pub count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewScheduledTask<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub schedule_json: &'a str,
    pub action_json: &'a str,
    pub status: &'a str,
    pub next_run_at: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledTaskUpdate<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub schedule_json: &'a str,
    pub action_json: &'a str,
    pub status: &'a str,
    pub next_run_at: Option<&'a str>,
    pub last_run_at: Option<&'a str>,
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledTaskRecord {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub schedule_json: String,
    pub action_json: String,
    pub status: String,
    pub next_run_at: Option<String>,
    pub last_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledTaskDueRunClaim<'a> {
    pub task_id: &'a str,
    pub expected_next_run_at: &'a str,
    pub run_id: &'a str,
    pub trigger_reason: &'a str,
    pub run_status: &'a str,
    pub scheduled_at: &'a str,
    pub completed_at: Option<&'a str>,
    pub error_message: Option<&'a str>,
    pub task_status: &'a str,
    pub task_next_run_at: Option<&'a str>,
    pub task_last_run_at: &'a str,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewScheduledTaskRun<'a> {
    pub id: &'a str,
    pub task_id: &'a str,
    pub trigger_reason: &'a str,
    pub status: &'a str,
    pub scheduled_at: &'a str,
    pub queued_at: Option<&'a str>,
    pub started_at: Option<&'a str>,
    pub completed_at: Option<&'a str>,
    pub chat_id: Option<&'a str>,
    pub user_message_id: Option<&'a str>,
    pub assistant_message_id: Option<&'a str>,
    pub agent_team_id: Option<&'a AgentTeamId>,
    pub agent_task_id: Option<&'a AgentTaskId>,
    pub agent_attempt_id: Option<&'a AgentAttemptId>,
    pub active_run_id: Option<&'a str>,
    pub error_message: Option<&'a str>,
    pub output_summary: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledTaskRunUpdate<'a> {
    pub id: &'a str,
    pub status: &'a str,
    pub queued_at: Option<&'a str>,
    pub started_at: Option<&'a str>,
    pub completed_at: Option<&'a str>,
    pub chat_id: Option<&'a str>,
    pub user_message_id: Option<&'a str>,
    pub assistant_message_id: Option<&'a str>,
    pub agent_team_id: Option<&'a AgentTeamId>,
    pub agent_task_id: Option<&'a AgentTaskId>,
    pub agent_attempt_id: Option<&'a AgentAttemptId>,
    pub active_run_id: Option<&'a str>,
    pub error_message: Option<&'a str>,
    pub output_summary: Option<&'a str>,
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledTaskRunRecord {
    pub id: String,
    pub task_id: String,
    pub trigger_reason: String,
    pub status: String,
    pub scheduled_at: String,
    pub queued_at: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub chat_id: Option<String>,
    pub user_message_id: Option<String>,
    pub assistant_message_id: Option<String>,
    pub agent_team_id: Option<AgentTeamId>,
    pub agent_task_id: Option<AgentTaskId>,
    pub agent_attempt_id: Option<AgentAttemptId>,
    pub active_run_id: Option<String>,
    pub error_message: Option<String>,
    pub output_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceSpecRecord {
    pub enabled: bool,
    pub inject_enabled: bool,
    pub content_markdown: String,
    pub revision: u64,
    pub generated_at: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewWorkspaceSpecJob<'a> {
    pub id: &'a str,
    pub trigger_type: &'a str,
    pub chat_id: Option<&'a str>,
    pub run_id: Option<&'a str>,
    pub model_id: Option<&'a str>,
    pub base_revision: Option<u64>,
    pub input_summary_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceSpecJobRecord {
    pub id: String,
    pub trigger_type: String,
    pub status: String,
    pub chat_id: Option<String>,
    pub run_id: Option<String>,
    pub model_id: Option<String>,
    pub base_revision: Option<u64>,
    pub input_summary_json: String,
    pub output_json: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    /// Last lease renewal for stale recovery. Independent of `started_at`.
    /// NULL on pre-migration or never-running rows; fall back to started_at/created_at.
    pub lease_renewed_at: Option<String>,
    pub has_retry: bool,
}

impl WorkspaceSpecJobRecord {
    /// Liveness timestamp for stale recovery: lease → started_at → created_at.
    pub fn lease_or_started_or_created_at(&self) -> &str {
        self.lease_renewed_at
            .as_deref()
            .or(self.started_at.as_deref())
            .unwrap_or(&self.created_at)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatSpecSnapshotRecord {
    pub chat_id: String,
    pub spec_revision: u64,
    pub content_markdown: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPlan<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub overview: &'a str,
    pub status: &'a str,
    pub source_chat_id: Option<&'a str>,
    pub phases: Vec<NewPlanPhase<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPlanPhase<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub summary: &'a str,
    pub steps: Vec<NewPlanStep<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPlanStep<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub detail: &'a str,
    pub acceptance: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStepPatch<'a> {
    pub title: Option<&'a str>,
    pub detail: Option<&'a str>,
    pub acceptance: Option<Vec<String>>,
    pub status: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanPatch<'a> {
    pub title: Option<&'a str>,
    pub overview: Option<&'a str>,
    pub status: Option<&'a str>,
    pub error_message: Option<Option<&'a str>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlanListOrder {
    #[default]
    Manual,
    NewestFirst,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanListFilter<'a> {
    pub view: &'a str,
    pub status: Option<&'a str>,
    pub order: PlanListOrder,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanListPage {
    pub plans: Vec<PlanRecord>,
    pub total_count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanWorktreeAuditRecord {
    pub plan_id: String,
    pub plan_status: String,
    pub phase_id: String,
    pub phase_status: String,
    pub implementation_chat_id: Option<String>,
    pub agent_task_id: Option<String>,
    pub agent_task_status: Option<String>,
    pub agent_instance_id: AgentInstanceId,
    pub worktree_path: String,
    pub base_revision: Option<String>,
    pub branch: Option<String>,
    pub worktree_status: Option<String>,
    pub plan_error_message: Option<String>,
    pub phase_error_message: Option<String>,
    pub task_error_message: Option<String>,
    pub commit_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRecord {
    pub id: String,
    pub title: String,
    pub overview: String,
    pub status: String,
    pub sort_order: i64,
    pub source_chat_id: Option<String>,
    pub active_phase_id: Option<String>,
    pub pause_requested_at: Option<String>,
    pub completed_at: Option<String>,
    pub completed_by_user_at: Option<String>,
    pub error_message: Option<String>,
    pub shared_merge_commit_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub phases: Vec<PlanPhaseRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanPhaseRecord {
    pub id: String,
    pub plan_id: String,
    pub sequence: i64,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub implementation_chat_id: Option<String>,
    pub agent_team_id: Option<String>,
    pub agent_task_id: Option<String>,
    pub commit_id: Option<String>,
    pub merge_attempt_count: i64,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub steps: Vec<PlanStepRecord>,
    pub attempts: Vec<PlanPhaseAttemptRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanPhaseAttemptRecord {
    pub id: String,
    pub plan_id: String,
    pub phase_id: String,
    pub sequence: i64,
    pub trigger: String,
    pub status: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub thinking_level: Option<String>,
    pub implementation_chat_id: Option<String>,
    pub agent_team_id: Option<String>,
    pub agent_task_id: Option<String>,
    pub commit_id: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Runtime incarnation that owns the unbound begin→attach dispatch window.
    /// Internal recovery metadata only; omitted from PlanRecord JSON so the UI
    /// protocol stays unchanged. NULL means legacy/previous-runtime ownership.
    #[serde(skip)]
    pub dispatch_owner_incarnation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPlanPhaseDerivedEffects<'a> {
    pub attempt_id: &'a str,
    pub plan_id: &'a str,
    pub phase_id: &'a str,
    pub agent_task_id: &'a AgentTaskId,
    pub chat_id: &'a str,
    pub run_id: &'a str,
    pub user_message_id: &'a str,
    pub assistant_message_id: &'a str,
    pub context_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanPhaseDerivedEffectsRecord {
    pub attempt_id: String,
    pub plan_id: String,
    pub phase_id: String,
    pub agent_task_id: AgentTaskId,
    pub chat_id: String,
    pub run_id: String,
    pub user_message_id: String,
    pub assistant_message_id: String,
    pub status: String,
    pub context_json: String,
    pub integration_confirmed_at: Option<String>,
    pub terminal_reason: Option<String>,
    pub released_at: Option<String>,
    pub discarded_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStepRecord {
    pub id: String,
    pub plan_id: String,
    pub phase_id: String,
    pub sequence: i64,
    pub title: String,
    pub detail: String,
    pub acceptance: Vec<String>,
    pub status: String,
    pub checked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatRecord {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
    pub metadata_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatPageCursor {
    pub updated_at: String,
    pub created_at: String,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatPage {
    pub chats: Vec<ChatRecord>,
    pub total_count: usize,
    pub has_more: bool,
    pub next_cursor: Option<ChatPageCursor>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeChangeStats {
    pub additions: usize,
    pub deletions: usize,
}

impl CodeChangeStats {
    pub(crate) fn from_metadata(value: &Value) -> Result<Self, WorkspaceDatabaseError> {
        let Some(additions) = value.get("additions").and_then(Value::as_u64) else {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "message metadata.codeChangeStats.additions must be an unsigned integer"
                    .to_string(),
            });
        };
        let Some(deletions) = value.get("deletions").and_then(Value::as_u64) else {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "message metadata.codeChangeStats.deletions must be an unsigned integer"
                    .to_string(),
            });
        };

        let additions = usize::try_from(additions).map_err(|_| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "message metadata.codeChangeStats.additions is too large".to_string(),
            }
        })?;
        let deletions = usize::try_from(deletions).map_err(|_| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "message metadata.codeChangeStats.deletions is too large".to_string(),
            }
        })?;

        Ok(Self {
            additions,
            deletions,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewMessage<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub role: &'a str,
    pub content: &'a str,
    pub sequence: i64,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageRecord {
    pub id: String,
    pub chat_id: String,
    pub role: String,
    pub content: String,
    pub sequence: i64,
    pub created_at: String,
    pub metadata_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageRoleCountRecord {
    pub role: String,
    pub count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteChatFromUserMessage<'a> {
    pub chat_id: &'a str,
    pub user_message_id: &'a str,
    pub expected_content: Option<&'a str>,
    pub content: &'a str,
    pub user_metadata_json: &'a str,
    pub chat_queued_run_json: &'a str,
    pub assistant_message_id: &'a str,
    pub assistant_metadata_json: &'a str,
    pub coordinator_task_id: Option<&'a AgentTaskId>,
    pub coordinator_task_input_json: Option<&'a str>,
    pub invalidated_reason: &'a str,
    pub memory_invalidation_reason: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteChatFromUserMessageResult {
    pub user_message: MessageRecord,
    pub assistant_message: MessageRecord,
    pub removed_message_ids: Vec<String>,
    pub invalidated_run_ids: Vec<String>,
    pub cancelled_agent_task_ids: Vec<AgentTaskId>,
    pub agent_team_id: Option<AgentTeamId>,
    pub agent_task_id: Option<AgentTaskId>,
    pub coordinator_context_generation: Option<i64>,
    pub skipped_workspace_spec_job_ids: Vec<String>,
    pub skipped_memory_extraction_job_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRunEvent<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub run_id: &'a str,
    pub sequence: i64,
    pub event_type: &'a str,
    pub payload_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunEventRecord {
    pub id: String,
    pub chat_id: String,
    pub run_id: String,
    pub sequence: i64,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewToolCall<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub run_id: &'a str,
    pub message_id: Option<&'a str>,
    pub tool_name: &'a str,
    pub input_json: &'a str,
    pub status: &'a str,
    pub started_at: &'a str,
    pub completed_at: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewToolResult<'a> {
    pub id: &'a str,
    pub tool_call_id: &'a str,
    pub output_json: &'a str,
    pub is_error: bool,
    pub created_at: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCallWithResultRecord {
    pub id: String,
    pub chat_id: String,
    pub run_id: String,
    pub message_id: Option<String>,
    pub tool_name: String,
    pub input_json: String,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub result: Option<ToolResultRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolResultRecord {
    pub id: String,
    pub tool_call_id: String,
    pub output_json: String,
    pub is_error: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCallCountRecord {
    pub tool_name: String,
    pub call_count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewLlmRequest<'a> {
    pub id: &'a str,
    pub workspace_id: &'a str,
    pub chat_id: Option<&'a str>,
    // ponytail: controlled string for now; upgrade to an enum/table if kind-level analytics need it.
    pub request_kind: &'a str,
    pub agent_team_id: Option<&'a AgentTeamId>,
    pub agent_instance_id: Option<&'a AgentInstanceId>,
    pub agent_task_id: Option<&'a AgentTaskId>,
    pub agent_attempt_id: Option<&'a AgentAttemptId>,
    pub provider_id: &'a str,
    pub model_id: &'a str,
    pub thinking_level: Option<&'a str>,
    pub request_started_at: &'a str,
    pub first_token_at: Option<&'a str>,
    pub completed_at: Option<&'a str>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub status_code: Option<i64>,
    pub final_state: &'a str,
    pub request_body_json: Option<&'a str>,
    pub response_body_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateLlmRequestOutcome<'a> {
    pub first_token_at: Option<&'a str>,
    pub completed_at: Option<&'a str>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub status_code: Option<i64>,
    pub final_state: &'a str,
    pub response_body_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LlmRequestRecord {
    pub id: String,
    pub workspace_id: Option<String>,
    pub chat_id: Option<String>,
    pub request_kind: String,
    pub agent_team_id: Option<AgentTeamId>,
    pub agent_instance_id: Option<AgentInstanceId>,
    pub agent_task_id: Option<AgentTaskId>,
    pub agent_attempt_id: Option<AgentAttemptId>,
    pub provider_id: String,
    pub model_id: String,
    pub thinking_level: Option<String>,
    pub request_started_at: String,
    pub first_token_at: Option<String>,
    pub completed_at: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub cache_ratio: Option<f64>,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub status_code: Option<i64>,
    pub final_state: String,
    /// Stable structured single-tool outcome code (nullable for historical rows).
    /// See `STRUCTURED_LLM_OUTCOME_*` constants.
    pub structured_outcome: Option<String>,
    /// How structured arguments were obtained when applicable.
    /// See `STRUCTURED_LLM_RECOVERY_*` constants.
    pub recovery_source: Option<String>,
    /// 1-based attempt within the audited provider request loop.
    pub attempt_index: Option<i64>,
    /// Durable id linking all stream attempts of one audited structured call (job).
    pub structured_call_id: Option<String>,
    pub request_body_json: Option<String>,
    pub response_body_json: Option<String>,
    pub invalidated_at: Option<String>,
    pub invalidated_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmRequestMetricsRecord {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub output_tokens: Option<i64>,
}

/// LLM metrics joined to the assistant message id from the request `start` event.
/// Used by chat message page assembly so metrics load only for the current page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmRequestMetricsForAssistantRecord {
    pub assistant_message_id: String,
    pub metrics: LlmRequestMetricsRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmRequestUsageRecord {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LlmRequestAuditFilters<'a> {
    pub request_ids: &'a [String],
    pub workspace_id: Option<&'a str>,
    pub chat_id: Option<&'a str>,
    pub request_kind: Option<&'a str>,
    pub exclude_request_kinds: &'a [&'a str],
    pub provider_id: Option<&'a str>,
    pub model_id: Option<&'a str>,
    pub final_state: Option<&'a str>,
    pub started_after: Option<&'a str>,
    pub started_before: Option<&'a str>,
    pub valid_only: bool,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LlmRequestUsageRollupFilters<'a> {
    pub workspace_id: Option<&'a str>,
    pub provider_id: Option<&'a str>,
    pub model_id: Option<&'a str>,
    pub final_state: Option<&'a str>,
    pub bucket_after: Option<&'a str>,
    pub bucket_before: Option<&'a str>,
}

/// Wire-derived LLM transport for a single `llm_requests` audit row.
///
/// Derived only from versioned `request_body_json` (never from current Provider config).
/// `http` = ordinary `provider_request_v1`; `websocket` = `provider_websocket_request_v1`
/// or compatible `provider_request_v1` with `method=WEBSOCKET`; anything else is `unknown`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LlmRequestTransport {
    Http,
    Websocket,
    #[default]
    Unknown,
}

impl LlmRequestTransport {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Websocket => "websocket",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "http" => Self::Http,
            "websocket" => Self::Websocket,
            _ => Self::Unknown,
        }
    }

    /// Classify transport from a stored versioned request dump (same rules as SQL CASE).
    pub fn from_request_body_json(raw: Option<&str>) -> Self {
        let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
            return Self::Unknown;
        };
        let Ok(parsed) = serde_json::from_str::<Value>(raw) else {
            return Self::Unknown;
        };
        let format = parsed.get("format").and_then(Value::as_str);
        let version = parsed.get("version").and_then(Value::as_u64);
        match (format, version) {
            (Some("provider_websocket_request_v1"), Some(1)) => Self::Websocket,
            (Some("provider_request_v1"), Some(1)) => {
                let method = parsed.get("method").and_then(Value::as_str).unwrap_or("");
                if method.eq_ignore_ascii_case("WEBSOCKET") {
                    Self::Websocket
                } else {
                    Self::Http
                }
            }
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LlmRequestAuditRow {
    pub id: String,
    pub workspace_id: Option<String>,
    pub chat_id: Option<String>,
    pub request_kind: String,
    pub provider_id: String,
    pub model_id: String,
    pub thinking_level: Option<String>,
    pub request_started_at: String,
    pub first_token_at: Option<String>,
    pub completed_at: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub cache_ratio: Option<f64>,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub status_code: Option<i64>,
    pub final_state: String,
    pub invalidated_at: Option<String>,
    pub invalidated_reason: Option<String>,
    /// Derived from `request_body_json` wire; never from live Provider settings.
    pub transport: LlmRequestTransport,
}

#[derive(Clone, Debug, Default)]
pub struct LlmRequestAuditSummaryRow {
    pub total_requests: i64,
    pub failed_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub total_cache_write_tokens: i64,
    pub total_tokens: i64,
    pub latency_count: i64,
    pub latency_sum: i64,
}

#[derive(Clone, Debug)]
pub struct LlmRequestAuditTrendPoint {
    pub bucket: String,
    pub request_count: i64,
    pub total_tokens: i64,
}

#[derive(Clone, Debug)]
pub struct LlmRequestAuditModelBreakdown {
    pub model_id: String,
    pub request_count: i64,
    pub total_tokens: i64,
}

#[derive(Clone, Debug)]
pub struct LlmRequestAuditProviderBreakdown {
    pub provider_id: String,
    pub request_count: i64,
    pub success_count: i64,
    pub total_tokens: i64,
    pub latency_count: i64,
    pub latency_sum: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LlmRequestAuditRequestKindBreakdown {
    pub request_kind: String,
    pub request_count: i64,
    pub failed_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub total_cache_write_tokens: i64,
    pub total_reasoning_tokens: i64,
    pub total_tokens: i64,
    pub latency_count: i64,
    pub latency_sum: i64,
}

/// Stable structured single-tool LLM outcome codes (no sensitive body text).
///
/// Used for memory retrieval/extraction and workspace-spec update baseline metrics.
pub const STRUCTURED_LLM_OUTCOME_SUCCEEDED: &str = "succeeded";
pub const STRUCTURED_LLM_OUTCOME_TEXT_JSON_RECOVERED: &str = "text_json_recovered";
pub const STRUCTURED_LLM_OUTCOME_PROVIDER_TIMEOUT: &str = "provider_timeout";
pub const STRUCTURED_LLM_OUTCOME_PROVIDER_ERROR: &str = "provider_error";
pub const STRUCTURED_LLM_OUTCOME_MISSING_TOOL: &str = "missing_tool";
pub const STRUCTURED_LLM_OUTCOME_WRONG_TOOL: &str = "wrong_tool";
pub const STRUCTURED_LLM_OUTCOME_SCHEMA_INVALID: &str = "schema_invalid";
pub const STRUCTURED_LLM_OUTCOME_SEMANTIC_INVALID: &str = "semantic_invalid";
pub const STRUCTURED_LLM_OUTCOME_OTHER: &str = "other";

/// Recovery path when structured arguments were obtained.
pub const STRUCTURED_LLM_RECOVERY_TOOL_CALL: &str = "tool_call";
pub const STRUCTURED_LLM_RECOVERY_TEXT_JSON: &str = "text_json";
pub const STRUCTURED_LLM_RECOVERY_CORRECTION_RETRY: &str = "correction_retry";
pub const STRUCTURED_LLM_RECOVERY_NONE: &str = "none";

/// Primary request kinds tracked for structured single-tool reliability baseline.
pub const STRUCTURED_LLM_BASELINE_REQUEST_KINDS: &[&str] = &[
    "memory retrieval",
    "memory extraction",
    "workspace spec update",
];

/// Classification payload written to `llm_requests` (never includes model/prompt body).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructuredLlmRequestClassification<'a> {
    pub structured_outcome: &'a str,
    pub recovery_source: &'a str,
    pub attempt_index: i64,
    /// Shared across attempts of one audited call. `None` preserves an existing id on update.
    pub structured_call_id: Option<&'a str>,
}

/// Filters for structured outcome breakdown queries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StructuredLlmOutcomeFilters<'a> {
    pub request_kinds: &'a [&'a str],
    pub provider_id: Option<&'a str>,
    pub model_id: Option<&'a str>,
    pub started_after: Option<&'a str>,
    pub started_before: Option<&'a str>,
    pub valid_only: bool,
}

/// One bucket for structured outcome analytics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuredLlmOutcomeBreakdownRow {
    pub request_kind: String,
    pub provider_id: String,
    pub model_id: String,
    pub transport: LlmRequestTransport,
    pub attempt_index: i64,
    pub structured_outcome: String,
    pub recovery_source: String,
    pub request_count: i64,
    pub success_count: i64,
    pub failed_count: i64,
}

/// Per-kind first-attempt vs **job-level** terminal success summary for structured requests.
///
/// - `first_attempt_requests` ≈ number of audited jobs (rows with `attempt_index = 1`).
/// - `terminal_successes` counts structured successes (`succeeded` / `text_json_recovered`) on any
///   attempt of a job; production writes at most one success row per job.
/// - `terminal_success_rate` = `terminal_successes / first_attempt_requests` (not `/ total_requests`).
/// - Average attempts per job ≈ `total_requests / first_attempt_requests`.
/// - `job_terminal_failures` = `first_attempt_requests - terminal_successes` (exact under the
///   production invariant of one `attempt_index=1` row and ≤1 structured success per job).
/// - `job_terminal_failure_rate` = `job_terminal_failures / first_attempt_requests`.
/// - `first_attempt_provider_failures` = first-attempt rows with `provider_timeout` /
///   `provider_error` outcomes (exact first-attempt slice).
/// - `first_attempt_protocol_failures` = first-attempt rows with protocol-class outcomes
///   (`missing_tool` / `wrong_tool` / `schema_invalid`) (exact first-attempt slice).
///
/// **Job linking:** production audited paths write `structured_call_id` on every attempt. Fixed
/// observation windows attribute a job to the window by the **first attempt's**
/// `request_started_at`, then include **all** later attempts of that call id (even outside the
/// window) when deciding terminal success. Without call ids, multi-attempt jobs cannot be joined
/// across a window boundary; see rollout docs.
///
/// Summaries **must not** subtract first-attempt provider counts from job terminal failures to
/// invent a "protocol terminal failure rate". That cross-job aggregate is wrong both when a job
/// recovers after a first-attempt provider error and when a job starts as protocol then ends as
/// provider. Until call-level terminal outcomes are labeled by class, use
/// `job_terminal_failure_rate` as a conservative upper bound on any terminal-failure slice, and
/// use the first-attempt protocol/provider counts only as diagnostic first-attempt signals.
#[derive(Clone, Debug, PartialEq)]
pub struct StructuredLlmOutcomeKindSummary {
    pub request_kind: String,
    pub total_requests: i64,
    pub first_attempt_requests: i64,
    pub first_attempt_successes: i64,
    pub terminal_successes: i64,
    pub extra_request_count: i64,
    pub first_attempt_success_rate: f64,
    pub terminal_success_rate: f64,
    pub job_terminal_failures: i64,
    pub job_terminal_failure_rate: f64,
    pub first_attempt_provider_failures: i64,
    pub first_attempt_protocol_failures: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewLlmRequestEvent<'a> {
    pub id: &'a str,
    pub llm_request_id: &'a str,
    pub sequence: i64,
    pub event_at: &'a str,
    pub event_type: &'a str,
    pub raw_chunk_json: Option<&'a str>,
    pub normalized_event_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmRequestEventRecord {
    pub id: String,
    pub llm_request_id: String,
    pub sequence: i64,
    pub event_at: String,
    pub event_type: String,
    pub raw_chunk_json: Option<String>,
    pub normalized_event_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewContextCompressionSnapshot<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub run_id: &'a str,
    pub sequence: i64,
    pub summary: &'a str,
    pub source_message_start_sequence: i64,
    pub source_message_end_sequence: i64,
    pub original_token_count: i64,
    pub summary_token_count: i64,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewContextCompressionSnapshotUnsequenced<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub run_id: &'a str,
    pub summary: &'a str,
    pub source_message_start_sequence: i64,
    pub source_message_end_sequence: i64,
    pub original_token_count: i64,
    pub summary_token_count: i64,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextCompressionSnapshotRecord {
    pub id: String,
    pub chat_id: String,
    pub run_id: String,
    pub sequence: i64,
    pub summary: String,
    pub source_message_start_sequence: i64,
    pub source_message_end_sequence: i64,
    pub original_token_count: i64,
    pub summary_token_count: i64,
    pub created_at: String,
    pub metadata_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPromptContextInjection<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub kind: &'a str,
    pub sequence: Option<i64>,
    pub messages_json: &'a str,
    pub memory_keys_json: &'a str,
    pub memory_summaries_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptContextInjectionRecord {
    pub id: String,
    pub chat_id: String,
    pub kind: String,
    pub sequence: Option<i64>,
    pub messages_json: String,
    pub memory_keys_json: String,
    pub memory_summaries_json: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTerminalSession<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub working_directory: &'a str,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSessionRecord {
    pub id: String,
    pub name: String,
    pub working_directory: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub metadata_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewHookRun<'a> {
    pub id: &'a str,
    pub workspace_id: &'a str,
    pub chat_id: Option<&'a str>,
    pub run_id: Option<&'a str>,
    pub tool_call_id: Option<&'a str>,
    pub event: &'a str,
    pub hook_source: &'a str,
    pub handler_type: &'a str,
    pub input_json: &'a str,
    pub output_json: Option<&'a str>,
    pub status: &'a str,
    pub exit_code: Option<i64>,
    pub stdout_preview: Option<&'a str>,
    pub stderr_preview: Option<&'a str>,
    pub started_at: &'a str,
    pub completed_at: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookRunRecord {
    pub id: String,
    pub workspace_id: String,
    pub chat_id: Option<String>,
    pub run_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub event: String,
    pub hook_source: String,
    pub handler_type: String,
    pub input_json: String,
    pub output_json: Option<String>,
    pub status: String,
    pub exit_code: Option<i64>,
    pub stdout_preview: Option<String>,
    pub stderr_preview: Option<String>,
    pub started_at: String,
    pub completed_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoGraphTask {
    pub id: String,
    pub title: String,
    pub status: String,
    pub depends_on: Vec<String>,
    pub acceptance: Vec<String>,
    pub summary: String,
    pub created_at: String,
    pub updated_at: String,
    pub subtasks: Vec<TodoGraphTask>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoGraphRecord {
    pub chat_id: String,
    pub tasks: Vec<TodoGraphTask>,
    pub created_at: String,
    pub updated_at: String,
    pub updated_task: Option<TodoGraphTask>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoGraphTaskPatch {
    pub title: Option<String>,
    pub status: Option<String>,
    pub depends_on: Option<Vec<String>>,
    pub acceptance: Option<Vec<String>>,
    pub summary: Option<String>,
    pub subtasks: Option<Vec<TodoGraphTask>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TodoGraphFilter<'a> {
    pub status: Option<&'a str>,
    pub task_id: Option<&'a str>,
    pub include_subtasks: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCodeGraphFileIndex<'a> {
    pub path: &'a str,
    pub language: Option<&'a str>,
    pub size_bytes: Option<i64>,
    pub modified_at: Option<&'a str>,
    pub content_hash: &'a str,
    pub parse_status: &'a str,
    pub parse_error_message: Option<&'a str>,
    pub symbols: &'a [NewCodeGraphSymbol<'a>],
    pub imports: &'a [NewCodeGraphImport<'a>],
    pub references: &'a [NewCodeGraphReference<'a>],
    pub edges: &'a [NewCodeGraphEdge<'a>],
    pub fts_body: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCodeGraphSymbol<'a> {
    pub name: &'a str,
    pub qualified_name: &'a str,
    pub kind: &'a str,
    pub visibility: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
    pub signature: Option<&'a str>,
    pub documentation: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCodeGraphImport<'a> {
    pub module: &'a str,
    pub imported_symbol: Option<&'a str>,
    pub alias: Option<&'a str>,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCodeGraphReference<'a> {
    pub name: &'a str,
    pub symbol_index: Option<usize>,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCodeGraphEdge<'a> {
    pub source_symbol_index: usize,
    pub target_symbol_index: usize,
    pub edge_kind: &'a str,
    pub metadata_json: Option<&'a str>,
}

/// A resolver-owned import relation. File and symbol ids refer to the durable
/// code graph rows captured by a resolver snapshot, not extractor-local keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCodeGraphImportResolution<'a> {
    pub import_id: i64,
    pub resolution: &'a str,
    pub target_file_id: Option<i64>,
    pub target_symbol_id: Option<i64>,
    pub candidates: &'a [NewCodeGraphImportResolutionCandidate],
    pub candidates_json: &'a str,
    pub metadata_json: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewCodeGraphImportResolutionCandidate {
    pub target_file_id: i64,
    pub target_symbol_id: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewCodeGraphResolvedCall<'a> {
    pub source_symbol_id: i64,
    pub target_symbol_id: i64,
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphContextRecord {
    pub indexed_files: i64,
    pub symbols: i64,
    pub references: i64,
    pub edges: i64,
    pub exact_import_resolutions: i64,
    pub candidate_import_resolutions: i64,
    pub unresolved_import_resolutions: i64,
    pub external_import_resolutions: i64,
    pub languages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphFileSummaryRecord {
    pub path: String,
    pub language: Option<String>,
    pub symbol_count: i64,
    pub import_count: i64,
    pub import_modules: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphSymbolRecord {
    pub id: i64,
    pub path: String,
    pub language: Option<String>,
    pub name: String,
    pub qualified_name: Option<String>,
    pub kind: String,
    pub visibility: Option<String>,
    pub metadata_json: String,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
    pub signature: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphSymbolRelationRecord {
    pub edge_id: i64,
    pub edge_kind: String,
    pub metadata_json: String,
    pub source: CodeGraphSymbolRecord,
    pub target: CodeGraphSymbolRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphReferenceRecord {
    pub id: i64,
    pub path: String,
    pub language: Option<String>,
    pub name: String,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
    pub symbol: Option<CodeGraphSymbolRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphRelatedFileRecord {
    pub path: String,
    pub language: Option<String>,
    pub relation: String,
    pub score: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphResolverSnapshot {
    pub files: Vec<CodeGraphResolverFileRecord>,
    pub imports: Vec<CodeGraphResolverImportRecord>,
    pub symbols: Vec<CodeGraphResolverSymbolRecord>,
    pub references: Vec<CodeGraphResolverReferenceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphResolverFileRecord {
    pub id: i64,
    pub path: String,
    pub language: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphResolverImportRecord {
    pub id: i64,
    pub file_id: i64,
    pub path: String,
    pub language: Option<String>,
    pub module: String,
    pub imported_symbol: Option<String>,
    pub alias: Option<String>,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphResolverSymbolRecord {
    pub id: i64,
    pub file_id: i64,
    pub path: String,
    pub name: String,
    pub qualified_name: Option<String>,
    pub kind: String,
    pub visibility: Option<String>,
    pub metadata_json: String,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphResolverReferenceRecord {
    pub id: i64,
    pub file_id: i64,
    pub name: String,
    /// A locally resolved declaration means an import binding must not claim this reference.
    pub symbol_id: Option<i64>,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphImportRecord {
    pub id: i64,
    pub path: String,
    pub language: Option<String>,
    pub module: String,
    pub imported_symbol: Option<String>,
    pub alias: Option<String>,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
    pub resolution: String,
    pub target_path: Option<String>,
    pub target_symbol: Option<CodeGraphSymbolRecord>,
    pub candidates_json: String,
    pub metadata_json: String,
}
