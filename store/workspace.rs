use std::{
    collections::{HashMap, HashSet},
    fmt, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, SystemTime},
};

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use foco_agent::{
    AgentAttemptId, AgentAttemptStatus, AgentDomainError, AgentEntityKind,
    AgentExecutionWorkspaceMode, AgentInstanceId, AgentInstanceStatus, AgentMessageId, AgentTaskId,
    AgentTaskStatus, AgentTaskTransition, AgentTeamId, AgentTeamStatus, TeamWorkload,
};
use rusqlite::{
    Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params, params_from_iter,
    types::Value as SqlValue,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::config::WorkspaceConfig;
use crate::memory::{
    MEMORY_DREAM_TRANSCRIPT_CHAT_KIND, MEMORY_FACT_ENABLED_MIGRATION_SQL,
    MEMORY_REFERENCES_SCHEMA_SQL, WORKSPACE_MEMORY_DREAM_SCHEMA_SQL, WORKSPACE_MEMORY_SCHEMA_SQL,
};
use crate::private_fs::{
    create_private_dir_all, prepare_private_file, restrict_private_file, restrict_sqlite_files,
};
#[path = "workspace_records.rs"]
mod workspace_records;
#[path = "workspace_schema.rs"]
mod workspace_schema;

pub use crate::workspace_gate::{
    OpenedMemoryDatabase, WORKSPACE_DATABASE_CRITICAL_GATE_TIMEOUT,
    WORKSPACE_DATABASE_ORDINARY_CAPACITY, WORKSPACE_DATABASE_ORDINARY_GATE_TIMEOUT,
    WORKSPACE_DATABASE_TOTAL_CAPACITY, WorkspaceDatabaseGateKind, WorkspaceDatabaseHandle,
    WorkspaceMemoryDatabaseHandle, open_workspace_database, open_workspace_database_critical,
    open_workspace_memory_database, open_workspace_memory_database_critical,
};
pub use workspace_records::{
    AgentAttemptRecord, AgentContextEntryRecord, AgentContextSnapshotRecord, AgentEventRecord,
    AgentInstanceRecord, AgentMessageRecord, AgentReconciliationRecord, AgentTaskDependencyRecord,
    AgentTaskRecord, AgentTaskStateUpdate, AgentTeamRecord, ChatPage, ChatPageCursor, ChatRecord,
    ChatSpecSnapshotRecord, CodeChangeStats, CodeGraphContextRecord, CodeGraphFileSummaryRecord,
    CodeGraphReferenceRecord, CodeGraphRelatedFileRecord, CodeGraphSymbolRecord,
    CodeGraphSymbolRelationRecord, ContextCompressionSnapshotRecord, HookRunRecord,
    LlmRequestAuditFilters, LlmRequestAuditModelBreakdown, LlmRequestAuditProviderBreakdown,
    LlmRequestAuditRequestKindBreakdown, LlmRequestAuditRow, LlmRequestAuditSummaryRow,
    LlmRequestAuditTrendPoint, LlmRequestEventRecord, LlmRequestMetricsRecord, LlmRequestRecord,
    LlmRequestUsageRecord, LlmRequestUsageRollupFilters, MessageMetadataMutation, MessageRecord,
    MessageRoleCountRecord, NewAgentContextEntry, NewAgentContextSnapshot, NewAgentEvent,
    NewAgentInstance, NewAgentMessage, NewAgentTask, NewAgentTaskDependency, NewAgentTeam,
    NewCodeGraphEdge, NewCodeGraphFileIndex, NewCodeGraphImport, NewCodeGraphReference,
    NewCodeGraphSymbol, NewContextCompressionSnapshot, NewHookRun, NewLlmRequest,
    NewLlmRequestEvent, NewMessage, NewPlan, NewPlanPhase, NewPlanPhaseDerivedEffects, NewPlanStep,
    NewPromptContextInjection, NewRunEvent, NewScheduledTask, NewScheduledTaskRun,
    NewTerminalSession, NewToolCall, NewToolResult, NewWorkspaceSpecJob,
    PlanAutoRunCandidateRecord, PlanAutoRunSelection, PlanAutoRunStateRecord, PlanListFilter,
    PlanListOrder, PlanListPage, PlanPatch, PlanPhaseAttemptRecord, PlanPhaseDerivedEffectsRecord,
    PlanPhaseRecord, PlanRecord, PlanStepPatch, PlanStepRecord, PlanWorktreeAuditRecord,
    PreStreamChatFailureClosure, PreStreamChatFailureClosureResult,
    PreStreamFailureMaterialization, PromptContextInjectionRecord, RewriteChatFromUserMessage,
    RewriteChatFromUserMessageResult, RunEventRecord, ScheduledTaskDueRunClaim,
    ScheduledTaskListFilter, ScheduledTaskRecord, ScheduledTaskRunRecord, ScheduledTaskRunUpdate,
    ScheduledTaskStatusCountRecord, ScheduledTaskUpdate, TerminalSessionRecord, TodoGraphFilter,
    TodoGraphRecord, TodoGraphTask, TodoGraphTaskPatch, ToolCallCountRecord,
    ToolCallWithResultRecord, ToolResultRecord, UpdateLlmRequestOutcome, WorkspaceSpecJobRecord,
    WorkspaceSpecRecord,
};
use workspace_schema::{
    MIGRATION_001, MIGRATION_002, MIGRATION_003, MIGRATION_004, MIGRATION_005, MIGRATION_006,
    MIGRATION_008, MIGRATION_009, MIGRATION_010, MIGRATION_011, MIGRATION_012, MIGRATION_013,
    MIGRATION_014, MIGRATION_015, MIGRATION_018, MIGRATION_019, MIGRATION_020, MIGRATION_021,
    MIGRATION_022, MIGRATION_022_BACKFILL, MIGRATION_023, MIGRATION_024, MIGRATION_025,
    MIGRATION_026, MIGRATION_027, MIGRATION_028, MIGRATION_029, MIGRATION_030, MIGRATION_032,
    MIGRATION_033, MIGRATION_034, MIGRATION_035, MIGRATION_036, MIGRATION_037, MIGRATION_038,
    MIGRATION_039, Migration,
};

pub const WORKSPACE_FOCO_DIR: &str = ".foco";
pub const WORKSPACE_DATABASE_FILE: &str = "foco.sqlite";
pub const WORKSPACE_BACKUP_RETAIN_COUNT: usize = 3;
pub const WORKSPACE_SCHEMA_VERSION: u32 = 39;
pub const WORKSPACE_SPEC_DEFAULT_ID: &str = "default";
pub const WORKSPACE_SPEC_MAX_MARKDOWN_BYTES: usize = 64 * 1024;
pub const WORKSPACE_SPEC_STALE_REVISION_SKIP_REASON: &str = "stale_revision";

/// Persisted `llm_requests.request_kind` for Workspace Spec manual/refresh generation.
pub const LLM_REQUEST_KIND_WORKSPACE_SPEC_GENERATION: &str = "workspace spec generation";
/// Persisted `llm_requests.request_kind` for chat-end automatic Spec updates.
pub const LLM_REQUEST_KIND_WORKSPACE_SPEC_UPDATE: &str = "workspace spec update";
/// Persisted `llm_requests.request_kind` for full-Markdown Spec generation compaction.
pub const LLM_REQUEST_KIND_WORKSPACE_SPEC_COMPACTION: &str = "workspace spec compaction";
/// Persisted `llm_requests.request_kind` for patch-only Spec update compaction.
pub const LLM_REQUEST_KIND_WORKSPACE_SPEC_UPDATE_COMPACTION: &str =
    "workspace spec update compaction";

pub const MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS: &[&str] = &[
    "chat title generation",
    "contextCompression",
    "memory extraction",
    "memory retrieval",
    "model availability test",
    "prompt hook",
    LLM_REQUEST_KIND_WORKSPACE_SPEC_COMPACTION,
    LLM_REQUEST_KIND_WORKSPACE_SPEC_GENERATION,
    LLM_REQUEST_KIND_WORKSPACE_SPEC_UPDATE,
    LLM_REQUEST_KIND_WORKSPACE_SPEC_UPDATE_COMPACTION,
];

/// Shared with query-plan regression tests so EXPLAIN stays tied to production SQL.
#[doc(hidden)]
pub const NEXT_ENABLED_SCHEDULED_TASK_SQL: &str = "SELECT next_run_at
                 FROM scheduled_tasks
                 WHERE status = 'enabled' AND next_run_at IS NOT NULL
                 ORDER BY next_run_at ASC
                 LIMIT 1";

/// Shared with query-plan regression tests so EXPLAIN stays tied to production SQL.
#[doc(hidden)]
pub const RUNNABLE_AGENT_TASKS_SQL: &str = "SELECT task.id, task.team_id, task.owner_instance_id,
                        task.origin_instance_id, task.parent_task_id, task.sequence,
                        task.status, task.input_json, task.result_json, task.error_json,
                        task.created_at, task.updated_at, task.started_at, task.completed_at
                 FROM agent_tasks AS task
                 JOIN agent_instances AS instance ON instance.id = task.owner_instance_id
                 JOIN agent_teams AS team ON team.id = task.team_id
                 WHERE task.status = 'queued' AND instance.status IN ('idle', 'draining')
                   AND team.status IN ('active', 'draining')
                   AND (
                        SELECT COUNT(*)
                        FROM agent_tasks AS running_task
                        WHERE running_task.team_id = task.team_id
                          AND running_task.status = 'running'
                   ) < team.max_concurrent_runs
                   AND NOT EXISTS (
                        SELECT 1 FROM agent_tasks AS earlier_task
                        WHERE earlier_task.owner_instance_id = task.owner_instance_id
                          AND earlier_task.sequence < task.sequence
                          AND earlier_task.status IN ('queued', 'running', 'waiting')
                   )
                   AND (
                        json_extract(task.input_json, '$.deferUntilWorkspaceIdle') IS NOT 1
                        OR NOT EXISTS (
                            SELECT 1 FROM agent_tasks AS earlier_workspace_task
                            WHERE earlier_workspace_task.rowid < task.rowid
                              AND earlier_workspace_task.status IN ('queued', 'running', 'waiting')
                              AND COALESCE(json_extract(earlier_workspace_task.input_json, '$.sessionMode'), '') <> 'plan'
                        )
                   )
                   AND (
                        NOT EXISTS (
                            SELECT 1 FROM agent_task_dependencies AS dependency
                            WHERE dependency.waiting_task_id = task.id
                        )
                        OR (
                            EXISTS (
                                SELECT 1 FROM agent_task_dependencies AS dependency
                                WHERE dependency.waiting_task_id = task.id
                                  AND dependency.wait_mode = 'all'
                            )
                            AND NOT EXISTS (
                                SELECT 1
                                FROM agent_task_dependencies AS dependency
                                JOIN agent_tasks AS required_task
                                  ON required_task.id = dependency.dependency_task_id
                                WHERE dependency.waiting_task_id = task.id
                                  AND required_task.status NOT IN ('completed', 'failed', 'cancelled', 'interrupted')
                            )
                        )
                        OR EXISTS (
                            SELECT 1
                            FROM agent_task_dependencies AS dependency
                            JOIN agent_tasks AS required_task
                              ON required_task.id = dependency.dependency_task_id
                            WHERE dependency.waiting_task_id = task.id
                              AND dependency.wait_mode = 'any'
                              AND required_task.status IN ('completed', 'failed', 'cancelled', 'interrupted')
                        )
                        OR EXISTS (
                            SELECT 1 FROM agent_task_dependencies AS dependency
                            WHERE dependency.waiting_task_id = task.id
                              AND dependency.deadline_at IS NOT NULL
                              AND dependency.deadline_at <= ?1
                        )
                   )
                 ORDER BY instance.last_scheduled_at IS NOT NULL,
                          instance.last_scheduled_at,
                          task.team_id,
                          task.owner_instance_id,
                          task.sequence
                 LIMIT ?2";
const QUEUED_CHAT_METADATA_KEY: &str = "queuedRun";
const QUEUED_MESSAGE_METADATA_KEY: &str = "queuedRun";
const PLAN_AUTO_RUN_DESIRED_ENABLED_KEY: &str = "plan_auto_run_desired_enabled";
const PLAN_AUTO_RUN_LEGACY_ENABLED_KEY: &str = "plan_auto_run_enabled";
const PLAN_AUTO_RUN_BLOCKED_REASON_KEY: &str = "plan_auto_run_blocked_reason";
const PLAN_AUTO_RUN_BLOCKED_PLAN_ID_KEY: &str = "plan_auto_run_blocked_plan_id";
const PLAN_AUTO_RUN_BLOCKED_PHASE_ID_KEY: &str = "plan_auto_run_blocked_phase_id";
const LLM_AUDIT_DETAIL_V1_PRUNED_KEY: &str = "llm_audit_detail_v1_pruned";
const LLM_AUDIT_STATUS_CODE_V1_REPAIRED_KEY: &str = "llm_audit_status_code_v1_repaired";
const SQLITE_PRAGMA_OPTIMIZE_LAST_AT_KEY: &str = "sqlite_pragma_optimize_last_at";
/// Minimum gap between successful `PRAGMA optimize` runs for the same database path.
pub const SQLITE_PRAGMA_OPTIMIZE_MIN_INTERVAL_SECS: u64 = 24 * 60 * 60;
const WORKSPACE_DATABASE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);
const WORKSPACE_MIGRATION_LOCK_SUFFIX: &str = ".migrate.lock";
const VISIBLE_MESSAGE_FILTER_SQL: &str = r#"
  AND messages.id NOT IN (
      SELECT private_message_id
      FROM (
          SELECT DISTINCT CAST(
              COALESCE(
                  json_extract(run_events.payload_json, '$.assistantMessageId'),
                  json_extract(run_events.payload_json, '$.assistant_message_id')
              ) AS TEXT
          ) AS private_message_id
          FROM run_events
          INNER JOIN agent_tasks
             ON agent_tasks.id = run_events.run_id
          INNER JOIN agent_teams
             ON agent_teams.id = agent_tasks.team_id
            AND agent_teams.chat_id = run_events.chat_id
          WHERE run_events.chat_id = ?1
            AND run_events.event_type = 'start'
            AND agent_tasks.owner_instance_id <> agent_teams.coordinator_instance_id
          UNION
          SELECT DISTINCT CAST(
              COALESCE(
                  json_extract(agent_tasks.input_json, '$.queuedUserMessageId'),
                  json_extract(agent_tasks.input_json, '$.queued_user_message_id')
              ) AS TEXT
          ) AS private_message_id
          FROM agent_tasks
          INNER JOIN agent_teams
             ON agent_teams.id = agent_tasks.team_id
          WHERE agent_teams.chat_id = ?1
            AND agent_tasks.owner_instance_id <> agent_teams.coordinator_instance_id
      )
      WHERE private_message_id IS NOT NULL
        AND private_message_id <> ''
  )"#;
const PLAN_SELECT_BASE_SQL: &str = "SELECT id, title, overview, status, sort_order,
       source_chat_id, active_phase_id, pause_requested_at, completed_at,
       completed_by_user_at, error_message, shared_merge_commit_id, created_at, updated_at
 FROM plans";
const PLAN_SELECT_SQL: &str = "SELECT id, title, overview, status, sort_order,
       source_chat_id, active_phase_id, pause_requested_at, completed_at,
       completed_by_user_at, error_message, shared_merge_commit_id, created_at, updated_at
 FROM plans
 WHERE id = ?1";

const LLM_REQUEST_ROLLUP_UNKNOWN_WORKSPACE: &str = "__foco_unknown_workspace__";
const LLM_REQUEST_ROLLUP_UNKNOWN_BUCKET: &str = "__foco_unknown_date__";
const LLM_REQUEST_ROLLUP_UNKNOWN_PROVIDER: &str = "__foco_unknown_provider__";
const LLM_REQUEST_ROLLUP_UNKNOWN_MODEL: &str = "__foco_unknown_model__";

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: MIGRATION_001,
    },
    Migration {
        version: 2,
        sql: MIGRATION_002,
    },
    Migration {
        version: 3,
        sql: MIGRATION_003,
    },
    Migration {
        version: 4,
        sql: MIGRATION_004,
    },
    Migration {
        version: 5,
        sql: MIGRATION_005,
    },
    Migration {
        version: 6,
        sql: MIGRATION_006,
    },
    Migration {
        version: 7,
        sql: WORKSPACE_MEMORY_SCHEMA_SQL,
    },
    Migration {
        version: 8,
        sql: MIGRATION_008,
    },
    Migration {
        version: 9,
        sql: MIGRATION_009,
    },
    Migration {
        version: 10,
        sql: MIGRATION_010,
    },
    Migration {
        version: 11,
        sql: MIGRATION_011,
    },
    Migration {
        version: 12,
        sql: MIGRATION_012,
    },
    Migration {
        version: 13,
        sql: MIGRATION_013,
    },
    Migration {
        version: 14,
        sql: MIGRATION_014,
    },
    Migration {
        version: 15,
        sql: MIGRATION_015,
    },
    Migration {
        version: 16,
        sql: WORKSPACE_MEMORY_DREAM_SCHEMA_SQL,
    },
    Migration {
        version: 17,
        sql: MEMORY_REFERENCES_SCHEMA_SQL,
    },
    Migration {
        version: 18,
        sql: MIGRATION_018,
    },
    Migration {
        version: 19,
        sql: MIGRATION_019,
    },
    Migration {
        version: 20,
        sql: MIGRATION_020,
    },
    Migration {
        version: 21,
        sql: MIGRATION_021,
    },
    Migration {
        version: 22,
        sql: MIGRATION_022,
    },
    Migration {
        version: 23,
        sql: MIGRATION_023,
    },
    Migration {
        version: 24,
        sql: MIGRATION_024,
    },
    Migration {
        version: 25,
        sql: MIGRATION_025,
    },
    Migration {
        version: 26,
        sql: MIGRATION_026,
    },
    Migration {
        version: 27,
        sql: MIGRATION_027,
    },
    Migration {
        version: 28,
        sql: MIGRATION_028,
    },
    Migration {
        version: 29,
        sql: MIGRATION_029,
    },
    Migration {
        version: 30,
        sql: MIGRATION_030,
    },
    Migration {
        version: 31,
        sql: MEMORY_FACT_ENABLED_MIGRATION_SQL,
    },
    Migration {
        version: 32,
        sql: MIGRATION_032,
    },
    Migration {
        version: 33,
        sql: MIGRATION_033,
    },
    Migration {
        version: 34,
        sql: MIGRATION_034,
    },
    Migration {
        version: 35,
        sql: MIGRATION_035,
    },
    Migration {
        version: 36,
        sql: MIGRATION_036,
    },
    Migration {
        version: 37,
        sql: MIGRATION_037,
    },
    Migration {
        version: 38,
        sql: MIGRATION_038,
    },
    Migration {
        version: 39,
        sql: MIGRATION_039,
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceDatabaseInfo {
    pub workspace_id: String,
    pub workspace_path: PathBuf,
    pub foco_dir: PathBuf,
    pub database_file: PathBuf,
}

pub fn initialize_workspace_databases(
    workspaces: &[WorkspaceConfig],
) -> Result<Vec<WorkspaceDatabaseInfo>, WorkspaceDatabaseError> {
    let mut initialized = Vec::with_capacity(workspaces.len());

    for workspace in workspaces {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)?;
        initialized.push(WorkspaceDatabaseInfo {
            workspace_id: workspace.id.clone(),
            workspace_path: workspace.path.clone(),
            foco_dir: workspace_foco_dir(&workspace.path),
            database_file: database.database_path().to_path_buf(),
        });
    }

    Ok(initialized)
}

pub fn workspace_foco_dir(workspace_path: impl AsRef<Path>) -> PathBuf {
    workspace_path.as_ref().join(WORKSPACE_FOCO_DIR)
}

pub fn workspace_database_path(workspace_path: impl AsRef<Path>) -> PathBuf {
    workspace_foco_dir(workspace_path).join(WORKSPACE_DATABASE_FILE)
}

pub fn prune_workspace_database_backups(
    workspace_path: &Path,
) -> Result<usize, WorkspaceDatabaseError> {
    let backup_dir = workspace_foco_dir(workspace_path).join("backups");
    if !backup_dir.exists() {
        return Ok(0);
    }

    let mut backups = Vec::new();
    for entry in fs::read_dir(&backup_dir).map_err(|source| WorkspaceDatabaseError::Io {
        path: backup_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| WorkspaceDatabaseError::Io {
            path: backup_dir.clone(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| WorkspaceDatabaseError::Io {
                path: path.clone(),
                source,
            })?;
        if !file_type.is_file() || !is_workspace_database_backup_file(&path) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        backups.push((workspace_backup_sort_key(&path, modified), path));
    }

    if backups.len() <= WORKSPACE_BACKUP_RETAIN_COUNT {
        return Ok(0);
    }

    backups.sort_by(|(left, _), (right, _)| right.cmp(left));
    let mut deleted = 0usize;
    // ponytail: fixed-count retention is enough for now; wire this constant into config if users need policy control.
    for (_, path) in backups.into_iter().skip(WORKSPACE_BACKUP_RETAIN_COUNT) {
        fs::remove_file(&path).map_err(|source| WorkspaceDatabaseError::Io {
            path: path.clone(),
            source,
        })?;
        deleted = deleted.saturating_add(1);
    }

    Ok(deleted)
}

fn is_workspace_database_backup_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    file_name.starts_with("foco-v") && file_name.ends_with(".sqlite")
}

fn workspace_backup_sort_key(path: &Path, modified: SystemTime) -> SystemTime {
    path.file_stem()
        .and_then(|value| value.to_str())
        .and_then(|stem| stem.rsplit_once('-').map(|(_, timestamp)| timestamp))
        .and_then(parse_workspace_backup_timestamp)
        .unwrap_or(modified)
}

fn parse_workspace_backup_timestamp(value: &str) -> Option<SystemTime> {
    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S%fZ")
        .ok()
        .map(|timestamp| timestamp.and_utc().into())
}

pub struct WorkspaceDatabase {
    database_path: PathBuf,
    connection: Connection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanPhaseAttemptTrigger {
    Initial,
    Retry,
    ModelOverrideRetry,
    MergeAuto,
}

impl PlanPhaseAttemptTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Retry => "retry",
            Self::ModelOverrideRetry => "model_override_retry",
            Self::MergeAuto => "merge_auto",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanPhaseAttemptStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl PlanPhaseAttemptStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

// ponytail: Phase 0 only codifies Project Spec behavior; storage, HTTP, prompt wiring, and UI land later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspaceSpecSettings {
    pub enabled: bool,
    pub inject_enabled: bool,
}

impl WorkspaceSpecSettings {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            inject_enabled: false,
        }
    }

    pub const fn enabled(inject_enabled: bool) -> Self {
        Self {
            enabled: true,
            inject_enabled,
        }
    }

    pub const fn allows_generation(self) -> bool {
        self.enabled
    }

    pub const fn allows_update(self) -> bool {
        self.enabled
    }

    pub const fn allows_injection(self) -> bool {
        self.enabled && self.inject_enabled
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceSpecPromptPlan {
    UseChatSnapshot,
    ReadWorkspaceSpecAndSaveSnapshot,
    SkipDisabled,
    SkipInjectionDisabled,
}

impl WorkspaceSpecPromptPlan {
    pub const fn for_chat(settings: WorkspaceSpecSettings, chat_snapshot_exists: bool) -> Self {
        if chat_snapshot_exists {
            return Self::UseChatSnapshot;
        }
        if !settings.enabled {
            return Self::SkipDisabled;
        }
        if !settings.inject_enabled {
            return Self::SkipInjectionDisabled;
        }

        Self::ReadWorkspaceSpecAndSaveSnapshot
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceSpecTriggerType {
    ManualInitial,
    ManualRefresh,
    ChatCompleted,
}

impl WorkspaceSpecTriggerType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ManualInitial => "manual_initial",
            Self::ManualRefresh => "manual_refresh",
            Self::ChatCompleted => "chat_completed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, WorkspaceDatabaseError> {
        match value {
            "manual_initial" => Ok(Self::ManualInitial),
            "manual_refresh" => Ok(Self::ManualRefresh),
            "chat_completed" => Ok(Self::ChatCompleted),
            _ => Err(WorkspaceDatabaseError::InvalidWorkspaceSpec {
                message: format!("unknown workspace spec trigger type: {value}"),
            }),
        }
    }

    pub const fn is_manual(self) -> bool {
        matches!(self, Self::ManualInitial | Self::ManualRefresh)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceSpecJobStatus {
    Queued,
    Running,
    Completed,
    Skipped,
    Failed,
}

impl WorkspaceSpecJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, WorkspaceDatabaseError> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "skipped" => Ok(Self::Skipped),
            "failed" => Ok(Self::Failed),
            _ => Err(WorkspaceDatabaseError::InvalidWorkspaceSpec {
                message: format!("unknown workspace spec job status: {value}"),
            }),
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Skipped | Self::Failed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceSpecJobEnqueueDecision {
    QueueNow,
    QueuePendingRefresh,
    AlreadyPendingRefresh,
    RejectAlreadyRunning,
}

impl WorkspaceSpecJobEnqueueDecision {
    pub const fn for_trigger(
        trigger: WorkspaceSpecTriggerType,
        running_job_exists: bool,
        pending_refresh_exists: bool,
    ) -> Self {
        if !running_job_exists {
            return Self::QueueNow;
        }
        if trigger.is_manual() {
            return Self::RejectAlreadyRunning;
        }
        if pending_refresh_exists {
            return Self::AlreadyPendingRefresh;
        }

        Self::QueuePendingRefresh
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceSpecWriteDecision {
    WriteFullReplacement,
    SkipStaleRevision { reason: &'static str },
}

impl WorkspaceSpecWriteDecision {
    pub const fn for_job_output(base_revision: u64, current_revision: u64) -> Self {
        if base_revision == current_revision {
            Self::WriteFullReplacement
        } else {
            Self::SkipStaleRevision {
                reason: WORKSPACE_SPEC_STALE_REVISION_SKIP_REASON,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceSpecOutputStrategy {
    FullReplacementMarkdown,
}

impl WorkspaceSpecOutputStrategy {
    pub const fn uses_patch_parser(self) -> bool {
        match self {
            Self::FullReplacementMarkdown => false,
        }
    }

    pub const fn allows_stale_merge(self) -> bool {
        match self {
            Self::FullReplacementMarkdown => false,
        }
    }

    pub fn validate_markdown_size(self, content: &str) -> Result<(), WorkspaceDatabaseError> {
        match self {
            Self::FullReplacementMarkdown => {
                if content.len() > WORKSPACE_SPEC_MAX_MARKDOWN_BYTES {
                    return Err(WorkspaceDatabaseError::InvalidWorkspaceSpec {
                        message: format!(
                            "workspace spec Markdown is {} bytes, limit is {}",
                            content.len(),
                            WORKSPACE_SPEC_MAX_MARKDOWN_BYTES
                        ),
                    });
                }
            }
        }

        Ok(())
    }
}

pub const WORKSPACE_SPEC_V1_OUTPUT_STRATEGY: WorkspaceSpecOutputStrategy =
    WorkspaceSpecOutputStrategy::FullReplacementMarkdown;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspaceDatabaseSpaceStats {
    pub page_size_bytes: u64,
    pub page_count: u64,
    pub freelist_count: u64,
}

impl WorkspaceDatabaseSpaceStats {
    pub fn file_bytes(self) -> u64 {
        self.page_size_bytes.saturating_mul(self.page_count)
    }

    pub fn free_bytes(self) -> u64 {
        self.page_size_bytes.saturating_mul(self.freelist_count)
    }
}

impl WorkspaceDatabase {
    /// Production default open: ordinary concurrency gate (capacity 2 of total 3).
    ///
    /// Prefer this (or [`Self::open_or_create_critical`]) outside tests. The
    /// returned handle must be dropped promptly; do not hold it across
    /// provider/network awaits or full Agent runs.
    #[track_caller]
    pub fn open_or_create(
        workspace_path: impl AsRef<Path>,
    ) -> Result<crate::workspace_gate::WorkspaceDatabaseHandle, WorkspaceDatabaseError> {
        crate::workspace_gate::open_workspace_database(workspace_path)
    }

    /// Critical open: total capacity only (reserves 1 slot for Agent lifecycle).
    #[track_caller]
    pub fn open_or_create_critical(
        workspace_path: impl AsRef<Path>,
    ) -> Result<crate::workspace_gate::WorkspaceDatabaseHandle, WorkspaceDatabaseError> {
        crate::workspace_gate::open_workspace_database_critical(workspace_path)
    }

    /// Ungated open for the gate implementation and controlled tests only.
    ///
    /// Production code must use [`Self::open_or_create`] or
    /// [`Self::open_or_create_critical`].
    pub fn open_or_create_ungated(
        workspace_path: impl AsRef<Path>,
    ) -> Result<Self, WorkspaceDatabaseError> {
        let workspace_path = workspace_path.as_ref();

        if !workspace_path.is_dir() {
            return Err(WorkspaceDatabaseError::WorkspaceNotDirectory {
                path: workspace_path.to_path_buf(),
            });
        }

        let foco_dir = workspace_foco_dir(workspace_path);
        create_directory(&foco_dir)?;

        let database_path = foco_dir.join(WORKSPACE_DATABASE_FILE);
        let database_existed = database_path.exists();
        let mut connection = open_connection(&database_path)?;
        run_migrations(&mut connection, &database_path, database_existed)?;
        enable_write_ahead_logging(&connection, &database_path)?;
        restrict_sqlite_files(&database_path).map_err(|source| WorkspaceDatabaseError::Io {
            path: database_path.clone(),
            source,
        })?;

        Ok(Self {
            database_path,
            connection,
        })
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn space_stats(&self) -> Result<WorkspaceDatabaseSpaceStats, WorkspaceDatabaseError> {
        let page_size_bytes = query_u64_pragma(&self.connection, &self.database_path, "page_size")?;
        let page_count = query_u64_pragma(&self.connection, &self.database_path, "page_count")?;
        let freelist_count =
            query_u64_pragma(&self.connection, &self.database_path, "freelist_count")?;

        Ok(WorkspaceDatabaseSpaceStats {
            page_size_bytes,
            page_count,
            freelist_count,
        })
    }

    pub fn vacuum(&mut self) -> Result<(), WorkspaceDatabaseError> {
        self.connection
            .execute_batch("VACUUM")
            .map_err(|source| self.sqlite_error(source))
    }

    /// Low-frequency `PRAGMA optimize` for query-planner statistics.
    ///
    /// Not for request hot paths. Throttled by durable `workspace_metadata` and a
    /// process-local minimum interval. Failures are returned to the caller so
    /// maintenance can log a warning without aborting the rest of the tick.
    pub fn maybe_run_pragma_optimize(
        &mut self,
        force: bool,
    ) -> Result<bool, WorkspaceDatabaseError> {
        maybe_run_sqlite_pragma_optimize(
            &mut self.connection,
            &self.database_path,
            SqlitePragmaOptimizeThrottle::WorkspaceMetadata,
            force,
        )
        .map_err(|source| self.sqlite_error(source))
    }

    /// Runs idempotent data repairs that may scan large audit tables.
    ///
    /// Call only from a low-frequency background maintenance task, never from
    /// request-time database opening.
    pub fn run_pending_one_time_maintenance(&mut self) -> Result<(), WorkspaceDatabaseError> {
        prune_non_v1_llm_audit_details_once(&mut self.connection, &self.database_path)?;
        repair_llm_request_status_codes_from_v1_once(&mut self.connection, &self.database_path)
    }

    pub fn schema_version(&self) -> Result<u32, WorkspaceDatabaseError> {
        schema_version(&self.connection, &self.database_path)
    }

    pub fn set_workspace_metadata(
        &mut self,
        key: &str,
        value: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        let updated_at = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO workspace_metadata (key, value, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    updated_at = excluded.updated_at",
                params![key, value, updated_at],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn workspace_metadata(&self, key: &str) -> Result<Option<String>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT value FROM workspace_metadata WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn plan_auto_run_state(&self) -> Result<PlanAutoRunStateRecord, WorkspaceDatabaseError> {
        let desired_enabled = self
            .workspace_metadata(PLAN_AUTO_RUN_DESIRED_ENABLED_KEY)?
            .or_else(|| {
                self.workspace_metadata(PLAN_AUTO_RUN_LEGACY_ENABLED_KEY)
                    .ok()
                    .flatten()
            })
            .as_deref()
            == Some("true");
        let selection = self.next_plan_auto_run_candidate()?;
        let selection_block = match &selection {
            PlanAutoRunSelection::WaitingForReady { plan_id } => (
                Some("waiting_for_ready".to_string()),
                Some(plan_id.clone()),
                None,
            ),
            PlanAutoRunSelection::WaitingForRetry { plan_id, phase_id } => (
                Some("waiting_for_retry".to_string()),
                Some(plan_id.clone()),
                phase_id.clone(),
            ),
            PlanAutoRunSelection::BlockedByCancelledPhase { plan_id, phase_id } => (
                Some("cancelled_phase".to_string()),
                Some(plan_id.clone()),
                Some(phase_id.clone()),
            ),
            PlanAutoRunSelection::Candidate(_)
            | PlanAutoRunSelection::Running { .. }
            | PlanAutoRunSelection::Idle => (None, None, None),
        };
        let persisted_block = (
            self.workspace_metadata(PLAN_AUTO_RUN_BLOCKED_REASON_KEY)?,
            self.workspace_metadata(PLAN_AUTO_RUN_BLOCKED_PLAN_ID_KEY)?,
            self.workspace_metadata(PLAN_AUTO_RUN_BLOCKED_PHASE_ID_KEY)?,
        );
        let (blocked_reason, blocked_plan_id, blocked_phase_id) = if selection_block.0.is_some() {
            selection_block
        } else {
            persisted_block
        };
        let enabled = desired_enabled && blocked_reason.is_none();
        let busy = enabled
            && (matches!(
                selection,
                PlanAutoRunSelection::Candidate(_) | PlanAutoRunSelection::Running { .. }
            ) || self.plan_auto_run_has_in_flight()?);
        Ok(PlanAutoRunStateRecord {
            enabled,
            desired_enabled,
            busy,
            blocked_reason,
            blocked_plan_id,
            blocked_phase_id,
        })
    }

    pub fn set_plan_auto_run_enabled(
        &mut self,
        enabled: bool,
    ) -> Result<PlanAutoRunStateRecord, WorkspaceDatabaseError> {
        self.set_workspace_metadata(
            PLAN_AUTO_RUN_DESIRED_ENABLED_KEY,
            if enabled { "true" } else { "false" },
        )?;
        // Keep the legacy key synchronized for older binaries during a rolling upgrade.
        self.set_workspace_metadata(
            PLAN_AUTO_RUN_LEGACY_ENABLED_KEY,
            if enabled { "true" } else { "false" },
        )?;
        self.plan_auto_run_state()
    }

    pub fn block_plan_auto_run(
        &mut self,
        reason: &str,
        plan_id: Option<&str>,
        phase_id: Option<&str>,
    ) -> Result<PlanAutoRunStateRecord, WorkspaceDatabaseError> {
        let reason = reason.trim();
        if reason.is_empty() {
            return self.clear_plan_auto_run_block();
        }
        self.set_workspace_metadata(PLAN_AUTO_RUN_BLOCKED_REASON_KEY, reason)?;
        self.set_optional_workspace_metadata(PLAN_AUTO_RUN_BLOCKED_PLAN_ID_KEY, plan_id)?;
        self.set_optional_workspace_metadata(PLAN_AUTO_RUN_BLOCKED_PHASE_ID_KEY, phase_id)?;
        self.plan_auto_run_state()
    }

    pub fn clear_plan_auto_run_block(
        &mut self,
    ) -> Result<PlanAutoRunStateRecord, WorkspaceDatabaseError> {
        self.delete_workspace_metadata(PLAN_AUTO_RUN_BLOCKED_REASON_KEY)?;
        self.delete_workspace_metadata(PLAN_AUTO_RUN_BLOCKED_PLAN_ID_KEY)?;
        self.delete_workspace_metadata(PLAN_AUTO_RUN_BLOCKED_PHASE_ID_KEY)?;
        self.plan_auto_run_state()
    }

    fn set_optional_workspace_metadata(
        &mut self,
        key: &str,
        value: Option<&str>,
    ) -> Result<(), WorkspaceDatabaseError> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            Some(value) => self.set_workspace_metadata(key, value),
            None => self.delete_workspace_metadata(key),
        }
    }

    fn delete_workspace_metadata(&mut self, key: &str) -> Result<(), WorkspaceDatabaseError> {
        self.connection
            .execute(
                "DELETE FROM workspace_metadata WHERE key = ?1",
                params![key],
            )
            .map_err(|source| self.sqlite_error(source))?;
        Ok(())
    }

    pub fn next_plan_auto_run_candidate(
        &self,
    ) -> Result<PlanAutoRunSelection, WorkspaceDatabaseError> {
        // The first non-terminal plan is the queue boundary. Draft, failed, cancelled-phase,
        // and running states must stop selection instead of being filtered out in favor of a
        // later ready plan.
        let plan = self
            .connection
            .query_row(
                "SELECT id, status FROM plans
                 WHERE status IN ('draft', 'ready', 'failed', 'paused', 'running')
                 ORDER BY sort_order ASC, created_at ASC, id ASC
                 LIMIT 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))?;
        let Some((plan_id, status)) = plan else {
            return Ok(PlanAutoRunSelection::Idle);
        };
        let phase = self
            .connection
            .query_row(
                "SELECT id, status
                 FROM plan_phases
                 WHERE plan_id = ?1 AND status <> 'completed'
                 ORDER BY sequence ASC
                 LIMIT 1",
                params![plan_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))?;
        let phase_id = phase.as_ref().map(|(phase_id, _)| phase_id.clone());
        let phase_status = phase.as_ref().map(|(_, status)| status.as_str());

        if phase_status == Some("cancelled") {
            return Ok(PlanAutoRunSelection::BlockedByCancelledPhase {
                plan_id,
                phase_id: phase_id.expect("cancelled phase has an id"),
            });
        }
        if status == "draft" {
            return Ok(PlanAutoRunSelection::WaitingForReady { plan_id });
        }
        if status == "failed" || phase_status == Some("failed") {
            return Ok(PlanAutoRunSelection::WaitingForRetry { plan_id, phase_id });
        }
        if status == "running" || matches!(phase_status, Some("running" | "queued")) {
            return Ok(PlanAutoRunSelection::Running { plan_id, phase_id });
        }

        let action = if status == "paused" {
            "resume"
        } else {
            "start"
        };
        Ok(PlanAutoRunSelection::Candidate(
            PlanAutoRunCandidateRecord {
                plan_id,
                action: action.to_string(),
            },
        ))
    }

    pub fn disable_plan_auto_run_if_idle(&mut self) -> Result<bool, WorkspaceDatabaseError> {
        if !matches!(
            self.next_plan_auto_run_candidate()?,
            PlanAutoRunSelection::Idle
        ) || self.plan_auto_run_has_in_flight()?
        {
            return Ok(false);
        }

        self.set_plan_auto_run_enabled(false)?;
        Ok(true)
    }

    pub fn plan_auto_run_has_in_flight(&self) -> Result<bool, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM plans
                    WHERE status = 'running'
                       OR EXISTS (
                           SELECT 1 FROM plan_phases
                           WHERE plan_phases.plan_id = plans.id
                             AND (
                                plan_phases.status = 'running'
                                OR plan_phases.status = 'queued'
                             )
                       )
                 )",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn workspace_spec(&self) -> Result<Option<WorkspaceSpecRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT enabled, inject_enabled, content_markdown, revision, generated_at, updated_at
                 FROM workspace_specs
                 WHERE id = ?1",
                params![WORKSPACE_SPEC_DEFAULT_ID],
                workspace_spec_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn upsert_workspace_spec_settings(
        &mut self,
        enabled: bool,
        inject_enabled: bool,
    ) -> Result<WorkspaceSpecRecord, WorkspaceDatabaseError> {
        let now = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO workspace_specs
                    (id, enabled, inject_enabled, content_markdown, revision, generated_at, updated_at)
                 VALUES (?1, ?2, ?3, '', 0, NULL, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                    enabled = excluded.enabled,
                    inject_enabled = excluded.inject_enabled,
                    updated_at = excluded.updated_at",
                params![
                    WORKSPACE_SPEC_DEFAULT_ID,
                    sql_bool(enabled),
                    sql_bool(inject_enabled),
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        self.workspace_spec()?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidWorkspaceSpec {
                message: "workspace spec row was not found after settings save".to_string(),
            })
    }

    pub fn update_workspace_spec_content(
        &mut self,
        expected_revision: u64,
        content_markdown: &str,
    ) -> Result<Option<WorkspaceSpecRecord>, WorkspaceDatabaseError> {
        self.update_workspace_spec_content_inner(expected_revision, content_markdown, false)
    }

    pub fn update_workspace_spec_generated_content(
        &mut self,
        expected_revision: u64,
        content_markdown: &str,
    ) -> Result<Option<WorkspaceSpecRecord>, WorkspaceDatabaseError> {
        self.update_workspace_spec_content_inner(expected_revision, content_markdown, true)
    }

    fn update_workspace_spec_content_inner(
        &mut self,
        expected_revision: u64,
        content_markdown: &str,
        generated: bool,
    ) -> Result<Option<WorkspaceSpecRecord>, WorkspaceDatabaseError> {
        WORKSPACE_SPEC_V1_OUTPUT_STRATEGY.validate_markdown_size(content_markdown)?;
        let expected_revision = workspace_spec_revision_to_i64(expected_revision, "revision")?;
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;

        let current_revision = transaction
            .query_row(
                "SELECT revision FROM workspace_specs WHERE id = ?1",
                params![WORKSPACE_SPEC_DEFAULT_ID],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?;

        match current_revision {
            Some(current_revision) if current_revision != expected_revision => {
                transaction
                    .commit()
                    .map_err(|source| sqlite_error(&database_path, source))?;
                return Ok(None);
            }
            Some(current_revision) => {
                let next_revision = current_revision.checked_add(1).ok_or_else(|| {
                    WorkspaceDatabaseError::InvalidWorkspaceSpec {
                        message: "workspace spec revision is too large".to_string(),
                    }
                })?;
                transaction
                    .execute(
                        "UPDATE workspace_specs
                         SET content_markdown = ?2,
                             revision = ?3,
                             generated_at = CASE WHEN ?6 THEN ?4 ELSE generated_at END,
                             updated_at = ?4
                         WHERE id = ?1 AND revision = ?5",
                        params![
                            WORKSPACE_SPEC_DEFAULT_ID,
                            content_markdown,
                            next_revision,
                            now,
                            current_revision,
                            sql_bool(generated)
                        ],
                    )
                    .map_err(|source| sqlite_error(&database_path, source))?;
            }
            None if expected_revision != 0 => {
                transaction
                    .commit()
                    .map_err(|source| sqlite_error(&database_path, source))?;
                return Ok(None);
            }
            None => {
                transaction
                    .execute(
                        "INSERT INTO workspace_specs
                            (id, enabled, inject_enabled, content_markdown, revision, generated_at, updated_at)
                         VALUES (?1, 0, 0, ?2, 1, ?4, ?3)",
                        params![
                            WORKSPACE_SPEC_DEFAULT_ID,
                            content_markdown,
                            now,
                            generated.then_some(now.as_str())
                        ],
                    )
                    .map_err(|source| sqlite_error(&database_path, source))?;
            }
        }

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        self.workspace_spec()
    }

    pub fn insert_workspace_spec_job(
        &mut self,
        job: NewWorkspaceSpecJob<'_>,
    ) -> Result<WorkspaceSpecJobRecord, WorkspaceDatabaseError> {
        self.insert_workspace_spec_job_inner(job, false)
    }

    pub fn insert_workspace_spec_job_if_absent(
        &mut self,
        job: NewWorkspaceSpecJob<'_>,
    ) -> Result<WorkspaceSpecJobRecord, WorkspaceDatabaseError> {
        self.insert_workspace_spec_job_inner(job, true)
    }

    fn insert_workspace_spec_job_inner(
        &mut self,
        job: NewWorkspaceSpecJob<'_>,
        ignore_existing: bool,
    ) -> Result<WorkspaceSpecJobRecord, WorkspaceDatabaseError> {
        WorkspaceSpecTriggerType::parse(job.trigger_type)?;
        let input_summary_json = job.input_summary_json.unwrap_or("{}");
        let input_summary_json =
            redact_workspace_spec_json_object(input_summary_json, "input_summary_json")?;
        let base_revision = job
            .base_revision
            .map(|revision| workspace_spec_revision_to_i64(revision, "base_revision"))
            .transpose()?;
        let now = now_timestamp();
        let insert = if ignore_existing {
            "INSERT OR IGNORE INTO workspace_spec_jobs
                (id, trigger_type, status, chat_id, run_id, model_id, base_revision,
                 input_summary_json, created_at)
             VALUES (?1, ?2, 'queued', ?3, ?4, ?5, ?6, ?7, ?8)"
        } else {
            "INSERT INTO workspace_spec_jobs
                (id, trigger_type, status, chat_id, run_id, model_id, base_revision,
                 input_summary_json, created_at)
             VALUES (?1, ?2, 'queued', ?3, ?4, ?5, ?6, ?7, ?8)"
        };

        self.connection
            .execute(
                insert,
                params![
                    job.id,
                    job.trigger_type,
                    job.chat_id,
                    job.run_id,
                    job.model_id,
                    base_revision,
                    input_summary_json,
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        self.workspace_spec_job(job.id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidWorkspaceSpec {
                message: format!("workspace spec job '{}' was not found after insert", job.id),
            }
        })
    }

    pub fn retry_failed_workspace_spec_job(
        &mut self,
        old_id: &str,
        new_id: &str,
        model_id: Option<&str>,
    ) -> Result<Option<WorkspaceSpecJobRecord>, WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let Some(old_job) = transaction
            .query_row(
                "SELECT id, trigger_type, status, chat_id, run_id, model_id, base_revision,
                        input_summary_json, output_json, error_message, created_at,
                        started_at, completed_at,
                        EXISTS(SELECT 1 FROM workspace_spec_jobs retry WHERE retry.retry_of_job_id = workspace_spec_jobs.id)
                 FROM workspace_spec_jobs
                 WHERE id = ?1",
                params![old_id],
                workspace_spec_job_from_row,
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?
        else {
            return Ok(None);
        };
        if old_job.status != WorkspaceSpecJobStatus::Failed.as_str() {
            return Ok(None);
        }
        let active_retry_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM workspace_spec_jobs
                    WHERE retry_of_job_id = ?1
                      AND status IN ('queued', 'running')
                 )",
                params![old_id],
                |row| row.get::<_, i64>(0).map(|value| value != 0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if active_retry_exists {
            return Err(WorkspaceDatabaseError::WorkspaceSpecRetryAlreadyQueued {
                job_id: old_id.to_string(),
            });
        }

        WorkspaceSpecTriggerType::parse(&old_job.trigger_type)?;
        let input_summary_json =
            redact_workspace_spec_json_object(&old_job.input_summary_json, "input_summary_json")?;
        let base_revision = old_job
            .base_revision
            .map(|revision| workspace_spec_revision_to_i64(revision, "base_revision"))
            .transpose()?;
        let now = now_timestamp();
        transaction
            .execute(
                "INSERT INTO workspace_spec_jobs
                    (id, trigger_type, status, retry_of_job_id, chat_id, run_id, model_id,
                     base_revision, input_summary_json, created_at)
                 VALUES (?1, ?2, 'queued', ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    new_id,
                    old_job.trigger_type,
                    old_id,
                    old_job.chat_id.as_deref(),
                    old_job.run_id.as_deref(),
                    model_id,
                    base_revision,
                    input_summary_json,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let new_job = transaction
            .query_row(
                "SELECT id, trigger_type, status, chat_id, run_id, model_id, base_revision,
                        input_summary_json, output_json, error_message, created_at,
                        started_at, completed_at,
                        EXISTS(SELECT 1 FROM workspace_spec_jobs retry WHERE retry.retry_of_job_id = workspace_spec_jobs.id)
                 FROM workspace_spec_jobs
                 WHERE id = ?1",
                params![new_id],
                workspace_spec_job_from_row,
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        Ok(Some(new_job))
    }

    pub fn workspace_spec_jobs(
        &self,
        limit: i64,
    ) -> Result<Vec<WorkspaceSpecJobRecord>, WorkspaceDatabaseError> {
        self.workspace_spec_jobs_filtered(limit, false)
    }

    pub fn workspace_spec_jobs_filtered(
        &self,
        limit: i64,
        retryable_only: bool,
    ) -> Result<Vec<WorkspaceSpecJobRecord>, WorkspaceDatabaseError> {
        if limit <= 0 {
            return Err(WorkspaceDatabaseError::InvalidWorkspaceSpec {
                message: "workspace spec job limit must be positive".to_string(),
            });
        }
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, trigger_type, status, chat_id, run_id, model_id, base_revision,
                        input_summary_json, output_json, error_message, created_at,
                        started_at, completed_at,
                        EXISTS(SELECT 1 FROM workspace_spec_jobs retry WHERE retry.retry_of_job_id = workspace_spec_jobs.id)
                 FROM workspace_spec_jobs
                 WHERE (?2 = 0
                        OR status IN (?3, ?4)
                        OR (status = ?5 AND NOT EXISTS(
                            SELECT 1 FROM workspace_spec_jobs retry
                            WHERE retry.retry_of_job_id = workspace_spec_jobs.id
                        )))
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?1",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(
                params![
                    limit,
                    retryable_only,
                    WorkspaceSpecJobStatus::Queued.as_str(),
                    WorkspaceSpecJobStatus::Running.as_str(),
                    WorkspaceSpecJobStatus::Failed.as_str(),
                ],
                workspace_spec_job_from_row,
            )
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }
    pub fn workspace_spec_job_count(&self) -> Result<i64, WorkspaceDatabaseError> {
        self.workspace_spec_job_count_filtered(false)
    }

    pub fn workspace_spec_job_count_filtered(
        &self,
        retryable_only: bool,
    ) -> Result<i64, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT COUNT(*)
                 FROM workspace_spec_jobs
                 WHERE (?1 = 0
                        OR status IN (?2, ?3)
                        OR (status = ?4 AND NOT EXISTS(
                            SELECT 1 FROM workspace_spec_jobs retry
                            WHERE retry.retry_of_job_id = workspace_spec_jobs.id
                        )))",
                params![
                    retryable_only,
                    WorkspaceSpecJobStatus::Queued.as_str(),
                    WorkspaceSpecJobStatus::Running.as_str(),
                    WorkspaceSpecJobStatus::Failed.as_str(),
                ],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn running_workspace_spec_job(
        &self,
    ) -> Result<Option<WorkspaceSpecJobRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, trigger_type, status, chat_id, run_id, model_id, base_revision,
                        input_summary_json, output_json, error_message, created_at,
                        started_at, completed_at,
                        EXISTS(SELECT 1 FROM workspace_spec_jobs retry WHERE retry.retry_of_job_id = workspace_spec_jobs.id)
                 FROM workspace_spec_jobs
                 WHERE status = ?1
                 ORDER BY created_at DESC, id DESC
                 LIMIT 1",
                params![WorkspaceSpecJobStatus::Running.as_str()],
                workspace_spec_job_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn claim_next_workspace_spec_job(
        &mut self,
    ) -> Result<Option<WorkspaceSpecJobRecord>, WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let running_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM workspace_spec_jobs WHERE status = ?1
                 )",
                params![WorkspaceSpecJobStatus::Running.as_str()],
                |row| row.get::<_, i64>(0).map(|value| value != 0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if running_exists {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(None);
        }

        let next_job_id = transaction
            .query_row(
                "SELECT id
                 FROM workspace_spec_jobs
                 WHERE status = ?1
                 ORDER BY created_at ASC, id ASC
                 LIMIT 1",
                params![WorkspaceSpecJobStatus::Queued.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?;
        let Some(next_job_id) = next_job_id else {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(None);
        };

        let now = now_timestamp();
        let updated = transaction
            .execute(
                "UPDATE workspace_spec_jobs
                 SET status = ?2,
                     started_at = ?3,
                     completed_at = NULL,
                     error_message = NULL
                 WHERE id = ?1 AND status = ?4",
                params![
                    next_job_id,
                    WorkspaceSpecJobStatus::Running.as_str(),
                    now,
                    WorkspaceSpecJobStatus::Queued.as_str()
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if updated != 1 {
            return Err(WorkspaceDatabaseError::InvalidWorkspaceSpec {
                message: "workspace spec queue claim lost its selected job".to_string(),
            });
        }
        let job = transaction
            .query_row(
                "SELECT id, trigger_type, status, chat_id, run_id, model_id, base_revision,
                        input_summary_json, output_json, error_message, created_at,
                        started_at, completed_at,
                        EXISTS(SELECT 1 FROM workspace_spec_jobs retry WHERE retry.retry_of_job_id = workspace_spec_jobs.id)
                 FROM workspace_spec_jobs
                 WHERE id = ?1",
                params![next_job_id],
                workspace_spec_job_from_row,
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        Ok(Some(job))
    }

    pub fn queued_workspace_spec_job(
        &self,
    ) -> Result<Option<WorkspaceSpecJobRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, trigger_type, status, chat_id, run_id, model_id, base_revision,
                        input_summary_json, output_json, error_message, created_at,
                        started_at, completed_at,
                        EXISTS(SELECT 1 FROM workspace_spec_jobs retry WHERE retry.retry_of_job_id = workspace_spec_jobs.id)
                 FROM workspace_spec_jobs
                 WHERE status = ?1
                 ORDER BY created_at ASC, id ASC
                 LIMIT 1",
                params![WorkspaceSpecJobStatus::Queued.as_str()],
                workspace_spec_job_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn queued_workspace_spec_update_job(
        &self,
    ) -> Result<Option<WorkspaceSpecJobRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, trigger_type, status, chat_id, run_id, model_id, base_revision,
                        input_summary_json, output_json, error_message, created_at,
                        started_at, completed_at,
                        EXISTS(SELECT 1 FROM workspace_spec_jobs retry WHERE retry.retry_of_job_id = workspace_spec_jobs.id)
                 FROM workspace_spec_jobs
                 WHERE status = ?1 AND trigger_type = ?2
                 ORDER BY created_at ASC, id ASC
                 LIMIT 1",
                params![
                    WorkspaceSpecJobStatus::Queued.as_str(),
                    WorkspaceSpecTriggerType::ChatCompleted.as_str()
                ],
                workspace_spec_job_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn update_workspace_spec_job_input_summary(
        &mut self,
        id: &str,
        input_summary_json: &str,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let input_summary_json =
            redact_workspace_spec_json_object(input_summary_json, "input_summary_json")?;
        self.connection
            .execute(
                "UPDATE workspace_spec_jobs
                 SET input_summary_json = ?2
                 WHERE id = ?1",
                params![id, input_summary_json],
            )
            .map(|updated| updated > 0)
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn update_workspace_spec_job_prepared_input(
        &mut self,
        id: &str,
        base_revision: u64,
        input_summary_json: &str,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let base_revision = workspace_spec_revision_to_i64(base_revision, "base_revision")?;
        let input_summary_json =
            redact_workspace_spec_json_object(input_summary_json, "input_summary_json")?;
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let updated = transaction
            .execute(
                "UPDATE workspace_spec_jobs
                 SET base_revision = ?2,
                     input_summary_json = ?3
                 WHERE id = ?1",
                params![id, base_revision, input_summary_json],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        Ok(updated > 0)
    }

    pub fn mark_workspace_spec_job_running(
        &mut self,
        id: &str,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let now = now_timestamp();
        let updated = self
            .connection
            .execute(
                "UPDATE workspace_spec_jobs
                 SET status = ?2,
                     started_at = ?3,
                     completed_at = NULL,
                     error_message = NULL
                 WHERE id = ?1
                   AND status = ?4
                   AND NOT EXISTS (
                       SELECT 1 FROM workspace_spec_jobs
                       WHERE status = ?2 AND id != ?1
                   )
                   AND NOT EXISTS (
                       SELECT 1 FROM workspace_spec_jobs
                       WHERE status = ?4
                         AND (created_at < (SELECT created_at FROM workspace_spec_jobs WHERE id = ?1)
                              OR (created_at = (SELECT created_at FROM workspace_spec_jobs WHERE id = ?1)
                                  AND id < ?1))
                   )",
                params![
                    id,
                    WorkspaceSpecJobStatus::Running.as_str(),
                    now,
                    WorkspaceSpecJobStatus::Queued.as_str()
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        Ok(updated == 1)
    }

    pub fn mark_workspace_spec_job_completed(
        &mut self,
        id: &str,
        output_json: Option<&str>,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let output_json = redact_optional_workspace_spec_json(output_json, "output_json")?;
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE workspace_spec_jobs
                 SET status = ?2,
                     output_json = ?3,
                     error_message = NULL,
                     completed_at = ?4
                 WHERE id = ?1",
                params![
                    id,
                    WorkspaceSpecJobStatus::Completed.as_str(),
                    output_json,
                    now
                ],
            )
            .map(|updated| updated > 0)
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn mark_workspace_spec_job_skipped(
        &mut self,
        id: &str,
        reason: &str,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE workspace_spec_jobs
                 SET status = ?2,
                     error_message = ?3,
                     completed_at = ?4
                 WHERE id = ?1",
                params![id, WorkspaceSpecJobStatus::Skipped.as_str(), reason, now],
            )
            .map(|updated| updated > 0)
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn mark_workspace_spec_job_failed(
        &mut self,
        id: &str,
        error_message: &str,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE workspace_spec_jobs
                 SET status = ?2,
                     error_message = ?3,
                     completed_at = ?4
                 WHERE id = ?1",
                params![
                    id,
                    WorkspaceSpecJobStatus::Failed.as_str(),
                    error_message,
                    now
                ],
            )
            .map(|updated| updated > 0)
            .map_err(|source| self.sqlite_error(source))
    }

    /// Deletes a workspace spec job only when it is currently `failed`.
    /// Status is enforced in the same DELETE WHERE clause so non-failed rows cannot be removed.
    pub fn delete_failed_workspace_spec_job(
        &mut self,
        id: &str,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let deleted = self
            .connection
            .execute(
                "DELETE FROM workspace_spec_jobs
                 WHERE id = ?1 AND status = ?2",
                params![id, WorkspaceSpecJobStatus::Failed.as_str()],
            )
            .map_err(|source| self.sqlite_error(source))?;
        Ok(deleted > 0)
    }

    pub fn chat_spec_snapshot(
        &self,
        chat_id: &str,
    ) -> Result<Option<ChatSpecSnapshotRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT chat_id, spec_revision, content_markdown, created_at
                 FROM chat_spec_snapshots
                 WHERE chat_id = ?1",
                params![chat_id],
                chat_spec_snapshot_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn insert_chat_spec_snapshot(
        &mut self,
        chat_id: &str,
        revision: u64,
        content_markdown: &str,
    ) -> Result<ChatSpecSnapshotRecord, WorkspaceDatabaseError> {
        WORKSPACE_SPEC_V1_OUTPUT_STRATEGY.validate_markdown_size(content_markdown)?;
        let revision = workspace_spec_revision_to_i64(revision, "spec_revision")?;
        let now = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO chat_spec_snapshots
                    (chat_id, spec_revision, content_markdown, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![chat_id, revision, content_markdown, now],
            )
            .map_err(|source| self.sqlite_error(source))?;

        self.chat_spec_snapshot(chat_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidWorkspaceSpec {
                message: format!("chat spec snapshot for '{chat_id}' was not found after insert"),
            }
        })
    }

    pub fn workspace_spec_job(
        &self,
        id: &str,
    ) -> Result<Option<WorkspaceSpecJobRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, trigger_type, status, chat_id, run_id, model_id, base_revision,
                        input_summary_json, output_json, error_message, created_at,
                        started_at, completed_at,
                        EXISTS(SELECT 1 FROM workspace_spec_jobs retry WHERE retry.retry_of_job_id = workspace_spec_jobs.id)
                 FROM workspace_spec_jobs
                 WHERE id = ?1",
                params![id],
                workspace_spec_job_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn create_plan(&mut self, plan: NewPlan<'_>) -> Result<PlanRecord, WorkspaceDatabaseError> {
        validate_plan_status(plan.status)?;
        let title = required_plan_text("title", plan.title)?;
        let overview = plan.overview.trim();
        let source_chat_id = plan
            .source_chat_id
            .map(str::trim)
            .filter(|id| !id.is_empty());
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        if let Some(source_chat_id) = source_chat_id {
            let chat_exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM chats WHERE id = ?1)",
                    params![source_chat_id],
                    |row| row.get(0),
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            if !chat_exists {
                return Err(WorkspaceDatabaseError::InvalidPlan {
                    message: format!("source chat was not found: {source_chat_id}"),
                });
            }
        }
        let plan_id = ensure_plan_entity_id_available(
            &transaction,
            &database_path,
            "SELECT EXISTS(SELECT 1 FROM plans WHERE id = ?1)",
            "plan",
            plan.id,
        )?;
        let sort_order = transaction
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM plans",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        transaction
            .execute(
                "INSERT INTO plans
                    (id, title, overview, status, sort_order, source_chat_id,
                     created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    plan_id,
                    title,
                    overview,
                    plan.status,
                    sort_order,
                    source_chat_id,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        for (phase_index, phase) in plan.phases.iter().enumerate() {
            let phase_id = ensure_plan_entity_id_available(
                &transaction,
                &database_path,
                "SELECT EXISTS(SELECT 1 FROM plan_phases WHERE id = ?1)",
                "plan phase",
                phase.id,
            )?;
            let sequence = i64::try_from(phase_index).map_err(|source| {
                WorkspaceDatabaseError::InvalidPlan {
                    message: format!("plan phase index overflowed: {source}"),
                }
            })?;
            transaction
                .execute(
                    "INSERT INTO plan_phases
                        (id, plan_id, sequence, title, summary, status, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?6)",
                    params![
                        phase_id,
                        plan_id,
                        sequence,
                        required_plan_text("phase.title", phase.title)?,
                        phase.summary.trim(),
                        now
                    ],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            for (step_index, step) in phase.steps.iter().enumerate() {
                let step_id = ensure_plan_entity_id_available(
                    &transaction,
                    &database_path,
                    "SELECT EXISTS(SELECT 1 FROM plan_steps WHERE id = ?1)",
                    "plan step",
                    step.id,
                )?;
                let step_sequence = i64::try_from(step_index).map_err(|source| {
                    WorkspaceDatabaseError::InvalidPlan {
                        message: format!("plan step index overflowed: {source}"),
                    }
                })?;
                let acceptance_json = plan_acceptance_json(&step.acceptance)?;
                transaction
                    .execute(
                        "INSERT INTO plan_steps
                            (id, plan_id, phase_id, sequence, title, detail, acceptance_json,
                             status, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, ?8)",
                        params![
                            step_id,
                            plan_id,
                            phase_id,
                            step_sequence,
                            required_plan_text("step.title", step.title)?,
                            step.detail.trim(),
                            acceptance_json,
                            now
                        ],
                    )
                    .map_err(|source| sqlite_error(&database_path, source))?;
            }
        }

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        self.refresh_plan_status_from_steps(plan_id.as_str())?;
        self.plan(plan_id.as_str())?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan '{}' was not found after insert", plan_id),
            })
    }

    pub fn plan(&self, id: &str) -> Result<Option<PlanRecord>, WorkspaceDatabaseError> {
        let Some(mut plan) = self
            .connection
            .query_row(PLAN_SELECT_SQL, params![id.trim()], plan_from_row)
            .optional()
            .map_err(|source| self.sqlite_error(source))?
        else {
            return Ok(None);
        };
        plan.phases = self.plan_phases_for_plan(&plan.id)?;
        Ok(Some(plan))
    }

    pub fn plans(
        &self,
        filter: PlanListFilter<'_>,
    ) -> Result<PlanListPage, WorkspaceDatabaseError> {
        validate_plan_view(filter.view)?;
        if let Some(status) = filter.status {
            validate_plan_status(status)?;
        }
        if filter.limit <= 0 || filter.offset < 0 {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: "plan pagination limit must be positive and offset must be non-negative"
                    .to_string(),
            });
        }
        let mut where_clause = String::from(" WHERE 1 = 1");
        let mut params = Vec::new();
        if filter.view == "active" {
            where_clause.push_str(" AND status <> ?");
            params.push(SqlValue::Text("completed".to_string()));
        }
        if let Some(status) = filter.status {
            where_clause.push_str(" AND status = ?");
            params.push(SqlValue::Text(status.to_string()));
        }

        let total_count = self
            .connection
            .query_row(
                &format!("SELECT COUNT(*) FROM plans{where_clause}"),
                params_from_iter(params.clone()),
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))?;

        let mut query = String::from(PLAN_SELECT_BASE_SQL);
        query.push_str(&where_clause);
        query.push_str(match filter.order {
            PlanListOrder::Manual => {
                " ORDER BY sort_order ASC, created_at ASC, id ASC LIMIT ? OFFSET ?"
            }
            PlanListOrder::NewestFirst => " ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?",
        });
        params.push(SqlValue::Integer(filter.limit));
        params.push(SqlValue::Integer(filter.offset));
        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(params), plan_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        let mut plans = collect_rows(rows, &self.database_path)?;
        for plan in &mut plans {
            plan.phases = self.plan_phases_for_plan(&plan.id)?;
        }

        Ok(PlanListPage { plans, total_count })
    }

    pub fn reorder_active_plans(
        &mut self,
        plan_ids: &[String],
    ) -> Result<(), WorkspaceDatabaseError> {
        let requested_plan_ids = plan_ids.iter().map(|id| id.trim()).collect::<Vec<_>>();
        if requested_plan_ids.iter().any(|id| id.is_empty()) {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: "plan order contains empty plan id".to_string(),
            });
        }
        let mut seen_plan_ids = HashSet::new();
        for plan_id in &requested_plan_ids {
            if !seen_plan_ids.insert(*plan_id) {
                return Err(WorkspaceDatabaseError::InvalidPlan {
                    message: format!("plan order contains duplicate id: {plan_id}"),
                });
            }
        }

        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let active_plans = {
            let mut statement = transaction
                .prepare(
                    "SELECT id, status, sort_order
                     FROM plans
                     WHERE status <> 'completed'
                     ORDER BY sort_order ASC, created_at ASC, id ASC",
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })
                .map_err(|source| sqlite_error(&database_path, source))?;
            collect_rows(rows, &database_path)?
        };
        let reorderable_plans = active_plans
            .iter()
            .filter(|(_, status, _)| is_reorderable_plan_status(status))
            .collect::<Vec<_>>();
        let reorderable_ids = reorderable_plans
            .iter()
            .map(|(id, _, _)| id.as_str())
            .collect::<Vec<_>>();
        let reorderable_id_set = reorderable_ids.iter().copied().collect::<HashSet<_>>();

        if requested_plan_ids.len() != reorderable_ids.len() {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan order must contain exactly {} reorderable active plan ids",
                    reorderable_ids.len()
                ),
            });
        }
        if let Some(plan_id) = requested_plan_ids
            .iter()
            .find(|plan_id| !reorderable_id_set.contains(**plan_id))
        {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan is not reorderable in the active queue: {plan_id}"),
            });
        }

        // ponytail: only the current active queue is reorderable; use a paged/cursor API if this list grows beyond the side panel queue.
        for (plan_id, (_, _, sort_order)) in requested_plan_ids.iter().zip(reorderable_plans.iter())
        {
            transaction
                .execute(
                    "UPDATE plans SET sort_order = ?2 WHERE id = ?1",
                    params![plan_id, sort_order],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))
    }

    pub fn plan_worktree_audit(
        &self,
    ) -> Result<Vec<PlanWorktreeAuditRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT plans.id, plans.status, phases.id, phases.status,
                        phases.implementation_chat_id, phases.agent_task_id,
                        tasks.status, instances.id, instances.execution_root_path,
                        instances.worktree_base_revision, instances.worktree_branch,
                        instances.worktree_status, plans.error_message,
                        phases.error_message, tasks.error_json, phases.commit_id
                 FROM plans
                 INNER JOIN plan_phases AS phases ON phases.plan_id = plans.id
                 INNER JOIN agent_teams AS teams ON teams.id = phases.agent_team_id
                 INNER JOIN agent_instances AS instances
                    ON instances.id = teams.coordinator_instance_id
                 LEFT JOIN agent_tasks AS tasks ON tasks.id = phases.agent_task_id
                 WHERE instances.execution_workspace_mode = 'isolated_worktree'
                   AND instances.execution_root_path IS NOT NULL
                   AND instances.worktree_status IN ('active', 'kept')
                   AND (
                        plans.shared_merge_commit_id IS NULL
                        OR (
                            plans.status IN ('implemented', 'completed', 'failed', 'cancelled')
                            AND phases.status IN ('completed', 'failed', 'cancelled')
                        )
                   )
                 ORDER BY plans.updated_at DESC, phases.sequence ASC, plans.id ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], |row| {
                let agent_instance_id: String = row.get(7)?;
                Ok(PlanWorktreeAuditRecord {
                    plan_id: row.get(0)?,
                    plan_status: row.get(1)?,
                    phase_id: row.get(2)?,
                    phase_status: row.get(3)?,
                    implementation_chat_id: row.get(4)?,
                    agent_task_id: row.get(5)?,
                    agent_task_status: row.get::<_, Option<String>>(6)?,
                    agent_instance_id: AgentInstanceId::new(agent_instance_id).map_err(
                        |error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                7,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        },
                    )?,
                    worktree_path: row.get(8)?,
                    base_revision: row.get(9)?,
                    branch: row.get(10)?,
                    worktree_status: row.get(11)?,
                    plan_error_message: row.get(12)?,
                    phase_error_message: row.get(13)?,
                    task_error_message: row.get(14)?,
                    commit_id: row.get(15)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn delete_plan(&mut self, id: &str) -> Result<bool, WorkspaceDatabaseError> {
        let changed = self
            .connection
            .execute("DELETE FROM plans WHERE id = ?1", params![id.trim()])
            .map_err(|source| self.sqlite_error(source))?;
        Ok(changed > 0)
    }

    pub fn update_plan(
        &mut self,
        plan_id: &str,
        patch: PlanPatch<'_>,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let current = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        if current.status == "completed" {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: "completed plans cannot be edited".to_string(),
            });
        }
        let title = match patch.title {
            Some(title) => required_plan_text("title", title)?,
            None => current.title,
        };
        let overview = patch
            .overview
            .map(str::trim)
            .unwrap_or(current.overview.as_str())
            .to_string();
        if let Some(status) = patch.status {
            validate_plan_status(status)?;
            if status == "completed" {
                return Err(WorkspaceDatabaseError::InvalidPlan {
                    message: "use mark_complete to complete a plan".to_string(),
                });
            }
            if status != current.status {
                return Err(WorkspaceDatabaseError::InvalidPlan {
                    message: format!(
                        "plan status cannot be changed from '{}' to '{}' with update_plan; use a plan action or state-machine reconciliation",
                        current.status, status
                    ),
                });
            }
        }
        let error_message = match patch.error_message {
            Some(Some(message)) => Some(message.trim().to_string()),
            Some(None) => None,
            None => current.error_message,
        };
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plans
                 SET title = ?2,
                     overview = ?3,
                     error_message = ?4,
                     updated_at = ?5
                 WHERE id = ?1",
                params![plan_id.trim(), title, overview, error_message, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found after update: {}", plan_id.trim()),
            })
    }

    pub fn mark_plan_invalid(
        &mut self,
        plan_id: &str,
        error_message: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        if matches!(
            plan.status.as_str(),
            "implemented" | "completed" | "cancelled"
        ) {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan '{}' cannot be marked invalid while {}",
                    plan.id, plan.status
                ),
            });
        }
        if plan.phases.iter().any(|phase| {
            phase
                .attempts
                .iter()
                .any(|attempt| matches!(attempt.status.as_str(), "queued" | "running"))
        }) {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan '{}' cannot be marked invalid while a phase attempt is active",
                    plan.id
                ),
            });
        }
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plans
                 SET status = 'failed',
                     active_phase_id = NULL,
                     error_message = ?2,
                     pause_requested_at = NULL,
                     updated_at = ?3
                 WHERE id = ?1",
                params![plan.id, error_message.trim(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan was not found after invalid reconciliation: {}",
                    plan_id.trim()
                ),
            })
    }

    pub fn transition_plan(
        &mut self,
        plan_id: &str,
        action: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        match action {
            "start" | "resume" => self.start_next_plan_phase(plan_id),
            "pause" => self.pause_plan(plan_id),
            "cancel" => self.cancel_plan(plan_id),
            "mark_complete" => self.mark_plan_complete(plan_id),
            _ => Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!("invalid plan action: {action}"),
            }),
        }
    }

    pub fn mark_plan_complete(
        &mut self,
        plan_id: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        if !matches!(plan.status.as_str(), "implemented" | "failed" | "cancelled") {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan '{}' cannot be marked complete while {}",
                    plan.id, plan.status
                ),
            });
        }
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plans
                 SET status = 'completed',
                     completed_at = COALESCE(completed_at, ?2),
                     completed_by_user_at = ?2,
                     updated_at = ?2
                 WHERE id = ?1",
                params![plan.id, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found after completion: {}", plan_id.trim()),
            })
    }

    pub fn record_plan_shared_merge_commit(
        &mut self,
        plan_id: &str,
        commit_id: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let commit_id = commit_id.trim();
        if commit_id.is_empty() {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: "shared merge commit id cannot be empty".to_string(),
            });
        }
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plans
                 SET shared_merge_commit_id = ?2,
                     error_message = NULL,
                     updated_at = ?3
                 WHERE id = ?1",
                params![plan_id.trim(), commit_id, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.clear_plan_auto_run_block()?;
        self.plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan was not found after shared merge update: {}",
                    plan_id.trim()
                ),
            })
    }

    pub fn update_plan_step(
        &mut self,
        plan_id: &str,
        step_id: &str,
        patch: PlanStepPatch<'_>,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        if plan.status == "completed" {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: "completed plan steps cannot be edited".to_string(),
            });
        }
        let current =
            self.plan_step(step_id)?
                .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                    message: format!("plan step was not found: {}", step_id.trim()),
                })?;
        if current.plan_id != plan.id {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan step '{}' does not belong to plan '{}'",
                    current.id, plan.id
                ),
            });
        }
        let title = match patch.title {
            Some(title) => required_plan_text("step.title", title)?,
            None => current.title,
        };
        let detail = patch
            .detail
            .map(str::trim)
            .unwrap_or(current.detail.as_str())
            .to_string();
        let acceptance = match patch.acceptance {
            Some(acceptance) => plan_acceptance_json(&acceptance)?,
            None => plan_acceptance_json(&current.acceptance)?,
        };
        let status = match patch.status {
            Some(status) => {
                validate_plan_step_status(status)?;
                status.to_string()
            }
            None => current.status,
        };
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_steps
                 SET title = ?3,
                     detail = ?4,
                     acceptance_json = ?5,
                     status = ?6,
                     checked_at = CASE WHEN ?6 = 'completed' THEN COALESCE(checked_at, ?7) ELSE NULL END,
                     updated_at = ?7
                 WHERE plan_id = ?1 AND id = ?2",
                params![plan.id, step_id.trim(), title, detail, acceptance, status, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.refresh_plan_status_from_steps(plan_id)
    }

    pub fn begin_plan_phase_attempt(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        trigger: PlanPhaseAttemptTrigger,
        provider_id: Option<&str>,
        model_id: Option<&str>,
        thinking_level: Option<&str>,
    ) -> Result<PlanPhaseAttemptRecord, WorkspaceDatabaseError> {
        let plan = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        if matches!(plan.status.as_str(), "completed" | "cancelled") {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan '{}' cannot retry while {}", plan.id, plan.status),
            });
        }
        let phase = plan
            .phases
            .iter()
            .find(|phase| phase.id == phase_id.trim())
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan phase '{}' does not belong to plan '{}'",
                    phase_id.trim(),
                    plan.id
                ),
            })?;
        self.ensure_plan_phase_predecessors_completed(&plan, phase)?;
        if (!matches!(phase.status.as_str(), "failed" | "cancelled")
            && phase.agent_task_id.is_some())
            || self.phase_has_active_attempt(&phase.id)?
        {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan phase '{}' already has an active attempt", phase.id),
            });
        }
        if matches!(
            trigger,
            PlanPhaseAttemptTrigger::Retry | PlanPhaseAttemptTrigger::ModelOverrideRetry
        ) && !matches!(phase.status.as_str(), "failed" | "cancelled")
            && !(phase.status == "running"
                && phase.attempts.iter().any(|attempt| {
                    matches!(
                        attempt.status.as_str(),
                        "failed" | "cancelled" | "interrupted"
                    )
                }))
        {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan phase '{}' is not retryable", phase.id),
            });
        }

        let sequence = self.next_plan_phase_attempt_sequence(&phase.id)?;
        let attempt_id = format!("plan-phase-attempt-{}-{sequence}", phase.id.trim());
        let now = now_timestamp();
        let provider_id = normalized_optional_text(provider_id);
        let model_id = normalized_optional_text(model_id);
        let thinking_level = normalized_optional_text(thinking_level);
        self.connection
            .execute(
                "INSERT INTO plan_phase_attempts (
                    id, plan_id, phase_id, sequence, trigger, status,
                    provider_id, model_id, thinking_level, created_at, updated_at
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6, ?7, ?8, ?9, ?9)",
                params![
                    attempt_id.as_str(),
                    plan.id.as_str(),
                    phase.id.as_str(),
                    sequence,
                    trigger.as_str(),
                    provider_id,
                    model_id,
                    thinking_level,
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        self.connection
            .execute(
                "UPDATE plan_steps
                 SET status = 'pending',
                     checked_at = NULL,
                     updated_at = ?3
                 WHERE plan_id = ?1
                   AND phase_id = ?2
                   AND EXISTS (
                       SELECT 1 FROM plan_phases
                       WHERE plan_id = ?1 AND id = ?2 AND status IN ('failed', 'cancelled')
                   )",
                params![plan.id.as_str(), phase.id.as_str(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plan_phases
                 SET status = 'running',
                     implementation_chat_id = NULL,
                     agent_team_id = NULL,
                     agent_task_id = NULL,
                     commit_id = NULL,
                     merge_attempt_count = CASE WHEN status IN ('failed', 'cancelled') THEN 0 ELSE merge_attempt_count END,
                     error_message = NULL,
                     started_at = CASE WHEN status IN ('failed', 'cancelled') THEN ?3 ELSE COALESCE(started_at, ?3) END,
                     completed_at = NULL,
                     updated_at = ?3
                 WHERE plan_id = ?1 AND id = ?2",
                params![plan.id.as_str(), phase.id.as_str(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plans
                 SET status = 'running',
                     active_phase_id = ?2,
                     pause_requested_at = NULL,
                     completed_at = NULL,
                     completed_by_user_at = NULL,
                     error_message = NULL,
                     updated_at = ?3
                 WHERE id = ?1",
                params![plan.id.as_str(), phase.id.as_str(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;

        self.clear_plan_auto_run_block()?;
        self.plan_phase_attempt(&attempt_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan phase attempt was not found after insert: {attempt_id}"),
            })
    }

    pub fn attach_plan_phase_attempt_run(
        &mut self,
        attempt_id: &str,
        implementation_chat_id: &str,
        agent_team_id: &AgentTeamId,
        agent_task_id: &AgentTaskId,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let attempt = self.plan_phase_attempt(attempt_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan phase attempt was not found: {}", attempt_id.trim()),
            }
        })?;
        if !matches!(attempt.status.as_str(), "queued" | "running") {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan phase attempt '{}' cannot attach while {}",
                    attempt.id, attempt.status
                ),
            });
        }
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phase_attempts
                 SET status = 'running',
                     implementation_chat_id = ?2,
                     agent_team_id = ?3,
                     agent_task_id = ?4,
                     started_at = COALESCE(started_at, ?5),
                     updated_at = ?5
                 WHERE id = ?1",
                params![
                    attempt.id.as_str(),
                    implementation_chat_id.trim(),
                    agent_team_id.as_str(),
                    agent_task_id.as_str(),
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.attach_plan_phase_run_fields(
            &attempt.plan_id,
            &attempt.phase_id,
            implementation_chat_id,
            agent_team_id,
            agent_task_id,
        )
    }

    pub fn plan_phase_attempts_for_phase(
        &self,
        phase_id: &str,
    ) -> Result<Vec<PlanPhaseAttemptRecord>, WorkspaceDatabaseError> {
        self.plan_phase_attempts_for_phase_inner(phase_id)
    }

    fn attach_plan_phase_run_fields(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        implementation_chat_id: &str,
        agent_team_id: &AgentTeamId,
        agent_task_id: &AgentTaskId,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phases
                 SET implementation_chat_id = ?3,
                     agent_team_id = ?4,
                     agent_task_id = ?5,
                     error_message = NULL,
                     updated_at = ?6
                 WHERE plan_id = ?1 AND id = ?2",
                params![
                    plan_id.trim(),
                    phase_id.trim(),
                    implementation_chat_id.trim(),
                    agent_team_id.as_str(),
                    agent_task_id.as_str(),
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found after phase attach: {}", plan_id.trim()),
            })
    }

    fn next_plan_phase_attempt_sequence(
        &self,
        phase_id: &str,
    ) -> Result<i64, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT COALESCE(MAX(sequence) + 1, 0)
                 FROM plan_phase_attempts
                 WHERE phase_id = ?1",
                params![phase_id.trim()],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))
    }

    fn phase_has_active_attempt(&self, phase_id: &str) -> Result<bool, WorkspaceDatabaseError> {
        let count: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*)
                 FROM plan_phase_attempts
                 WHERE phase_id = ?1 AND status IN ('queued', 'running')",
                params![phase_id.trim()],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))?;
        Ok(count > 0)
    }

    fn plan_phase_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<Option<PlanPhaseAttemptRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, plan_id, phase_id, sequence, trigger, status,
                        provider_id, model_id, thinking_level,
                        implementation_chat_id, agent_team_id, agent_task_id,
                        commit_id, error_message, started_at, completed_at,
                        created_at, updated_at
                 FROM plan_phase_attempts
                 WHERE id = ?1",
                params![attempt_id.trim()],
                plan_phase_attempt_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    fn plan_phase_attempts_for_phase_inner(
        &self,
        phase_id: &str,
    ) -> Result<Vec<PlanPhaseAttemptRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, plan_id, phase_id, sequence, trigger, status,
                        provider_id, model_id, thinking_level,
                        implementation_chat_id, agent_team_id, agent_task_id,
                        commit_id, error_message, started_at, completed_at,
                        created_at, updated_at
                 FROM plan_phase_attempts
                 WHERE phase_id = ?1
                 ORDER BY sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![phase_id.trim()], plan_phase_attempt_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    fn update_attempt_for_task(
        &mut self,
        agent_task_id: &AgentTaskId,
        status: PlanPhaseAttemptStatus,
        commit_id: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phase_attempts
                 SET status = ?2,
                     commit_id = COALESCE(?3, commit_id),
                     error_message = ?4,
                     completed_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN COALESCE(completed_at, ?5) ELSE completed_at END,
                     updated_at = ?5
                 WHERE agent_task_id = ?1",
                params![
                    agent_task_id.as_str(),
                    status.as_str(),
                    commit_id.map(str::trim).filter(|value| !value.is_empty()),
                    error_message.map(str::trim).filter(|value| !value.is_empty()),
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        Ok(())
    }

    fn update_latest_active_attempt_for_phase(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        status: PlanPhaseAttemptStatus,
        error_message: Option<&str>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phase_attempts
                 SET status = ?3,
                     error_message = ?4,
                     completed_at = CASE WHEN ?3 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN COALESCE(completed_at, ?5) ELSE completed_at END,
                     updated_at = ?5
                 WHERE id = (
                     SELECT id FROM plan_phase_attempts
                     WHERE plan_id = ?1 AND phase_id = ?2 AND status IN ('queued', 'running')
                     ORDER BY sequence DESC
                     LIMIT 1
                 )",
                params![
                    plan_id.trim(),
                    phase_id.trim(),
                    status.as_str(),
                    error_message.map(str::trim).filter(|value| !value.is_empty()),
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        Ok(())
    }

    fn agent_task_attempt_terminal_status(
        &self,
        agent_task_id: &AgentTaskId,
    ) -> Result<PlanPhaseAttemptStatus, WorkspaceDatabaseError> {
        let status = self
            .connection
            .query_row(
                "SELECT status FROM agent_tasks WHERE id = ?1",
                params![agent_task_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))?;
        Ok(match status.as_deref() {
            Some("cancelled") => PlanPhaseAttemptStatus::Cancelled,
            Some("interrupted") => PlanPhaseAttemptStatus::Interrupted,
            _ => PlanPhaseAttemptStatus::Failed,
        })
    }

    pub fn attach_plan_phase_run(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        implementation_chat_id: &str,
        agent_team_id: &AgentTeamId,
        agent_task_id: &AgentTaskId,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        let phase = plan
            .phases
            .iter()
            .find(|phase| phase.id == phase_id.trim())
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan phase '{}' does not belong to plan '{}'",
                    phase_id.trim(),
                    plan.id
                ),
            })?;
        if phase.status != "running" {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan phase '{}' is not running", phase.id),
            });
        }
        if phase.agent_task_id.is_some() {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan phase '{}' already has an Agent task", phase.id),
            });
        }
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phases
                 SET implementation_chat_id = ?3,
                     agent_team_id = ?4,
                     agent_task_id = ?5,
                     error_message = NULL,
                     updated_at = ?6
                 WHERE plan_id = ?1 AND id = ?2",
                params![
                    plan.id.as_str(),
                    phase.id.as_str(),
                    implementation_chat_id.trim(),
                    agent_team_id.as_str(),
                    agent_task_id.as_str(),
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found after phase attach: {}", plan_id.trim()),
            })
    }

    pub fn attach_plan_phase_merge_run(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        implementation_chat_id: &str,
        agent_team_id: &AgentTeamId,
        agent_task_id: &AgentTaskId,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        if matches!(plan.status.as_str(), "completed" | "cancelled") {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan '{}' cannot attach merge while {}",
                    plan.id, plan.status
                ),
            });
        }
        let phase = plan
            .phases
            .iter()
            .find(|phase| phase.id == phase_id.trim())
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan phase '{}' does not belong to plan '{}'",
                    phase_id.trim(),
                    plan.id
                ),
            })?;
        if phase.merge_attempt_count <= 0 {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan phase '{}' has no merge attempt", phase.id),
            });
        }
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phases
                 SET status = 'running',
                     implementation_chat_id = ?3,
                     agent_team_id = ?4,
                     agent_task_id = ?5,
                     commit_id = NULL,
                     completed_at = NULL,
                     updated_at = ?6
                 WHERE plan_id = ?1 AND id = ?2",
                params![
                    plan.id.as_str(),
                    phase.id.as_str(),
                    implementation_chat_id.trim(),
                    agent_team_id.as_str(),
                    agent_task_id.as_str(),
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plans
                 SET status = 'running',
                     active_phase_id = ?2,
                     completed_at = NULL,
                     completed_by_user_at = NULL,
                     updated_at = ?3
                 WHERE id = ?1",
                params![plan.id.as_str(), phase.id.as_str(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan was not found after merge phase attach: {}",
                    plan_id.trim()
                ),
            })
    }

    pub fn plan_phase_attempt_for_agent_task(
        &self,
        agent_task_id: &AgentTaskId,
    ) -> Result<Option<PlanPhaseAttemptRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, plan_id, phase_id, sequence, trigger, status,
                        provider_id, model_id, thinking_level,
                        implementation_chat_id, agent_team_id, agent_task_id,
                        commit_id, error_message, started_at, completed_at, created_at, updated_at
                 FROM plan_phase_attempts
                 WHERE agent_task_id = ?1",
                params![agent_task_id.as_str()],
                plan_phase_attempt_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn insert_plan_phase_derived_effects(
        &mut self,
        effects: NewPlanPhaseDerivedEffects<'_>,
    ) -> Result<PlanPhaseDerivedEffectsRecord, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "INSERT INTO plan_phase_derived_effects (
                    attempt_id, plan_id, phase_id, agent_task_id, chat_id, run_id,
                    user_message_id, assistant_message_id, status, context_json,
                    created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'awaiting_integration', ?9, ?10, ?10)
                 ON CONFLICT(attempt_id) DO NOTHING",
                params![
                    effects.attempt_id,
                    effects.plan_id,
                    effects.phase_id,
                    effects.agent_task_id.as_str(),
                    effects.chat_id,
                    effects.run_id,
                    effects.user_message_id,
                    effects.assistant_message_id,
                    effects.context_json,
                    now,
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan_phase_derived_effects(effects.attempt_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan phase derived effects were not found after insert: {}",
                    effects.attempt_id
                ),
            })
    }

    pub fn plan_phase_derived_effects(
        &self,
        attempt_id: &str,
    ) -> Result<Option<PlanPhaseDerivedEffectsRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT attempt_id, plan_id, phase_id, agent_task_id, chat_id, run_id,
                        user_message_id, assistant_message_id, status, context_json,
                        integration_confirmed_at, terminal_reason, released_at, discarded_at,
                        created_at, updated_at
                 FROM plan_phase_derived_effects
                 WHERE attempt_id = ?1",
                params![attempt_id.trim()],
                plan_phase_derived_effects_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn awaiting_plan_phase_derived_effects(
        &self,
    ) -> Result<Vec<PlanPhaseDerivedEffectsRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT attempt_id, plan_id, phase_id, agent_task_id, chat_id, run_id,
                        user_message_id, assistant_message_id, status, context_json,
                        integration_confirmed_at, terminal_reason, released_at, discarded_at,
                        created_at, updated_at
                 FROM plan_phase_derived_effects
                 WHERE status = 'awaiting_integration'
                 ORDER BY created_at ASC, attempt_id ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], plan_phase_derived_effects_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn releasable_plan_phase_derived_effects(
        &self,
    ) -> Result<Vec<PlanPhaseDerivedEffectsRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT attempt_id, plan_id, phase_id, agent_task_id, chat_id, run_id,
                        user_message_id, assistant_message_id, status, context_json,
                        integration_confirmed_at, terminal_reason, released_at, discarded_at,
                        created_at, updated_at
                 FROM plan_phase_derived_effects
                 WHERE status = 'awaiting_integration'
                   AND integration_confirmed_at IS NOT NULL
                 ORDER BY created_at ASC, attempt_id ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], plan_phase_derived_effects_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn confirm_plan_phase_derived_effects_integration(
        &mut self,
        attempt_id: &str,
    ) -> Result<Option<PlanPhaseDerivedEffectsRecord>, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phase_derived_effects
                 SET integration_confirmed_at = COALESCE(integration_confirmed_at, ?2),
                     updated_at = ?2
                 WHERE attempt_id = ?1 AND status = 'awaiting_integration'",
                params![attempt_id.trim(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan_phase_derived_effects(attempt_id)
    }

    pub fn confirm_latest_completed_plan_phase_derived_effects(
        &mut self,
        plan_id: &str,
        phase_id: &str,
    ) -> Result<Option<PlanPhaseDerivedEffectsRecord>, WorkspaceDatabaseError> {
        let attempt_id = self
            .connection
            .query_row(
                "SELECT effects.attempt_id
                 FROM plan_phase_derived_effects AS effects
                 JOIN plan_phase_attempts AS attempt ON attempt.id = effects.attempt_id
                 WHERE effects.plan_id = ?1 AND effects.phase_id = ?2
                   AND effects.status = 'awaiting_integration'
                   AND attempt.status = 'completed'
                 ORDER BY attempt.sequence DESC
                 LIMIT 1",
                params![plan_id.trim(), phase_id.trim()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))?;
        match attempt_id {
            Some(attempt_id) => self.confirm_plan_phase_derived_effects_integration(&attempt_id),
            None => Ok(None),
        }
    }

    pub fn discard_plan_phase_derived_effects(
        &mut self,
        attempt_id: &str,
        reason: &str,
    ) -> Result<Option<PlanPhaseDerivedEffectsRecord>, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phase_derived_effects
                 SET status = 'discarded',
                     terminal_reason = ?2,
                     discarded_at = COALESCE(discarded_at, ?3),
                     updated_at = ?3
                 WHERE attempt_id = ?1 AND status = 'awaiting_integration'",
                params![attempt_id.trim(), reason.trim(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan_phase_derived_effects(attempt_id)
    }

    pub fn mark_plan_phase_derived_effects_released(
        &mut self,
        attempt_id: &str,
    ) -> Result<Option<PlanPhaseDerivedEffectsRecord>, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phase_derived_effects
                 SET status = 'released',
                     terminal_reason = NULL,
                     released_at = COALESCE(released_at, ?2),
                     updated_at = ?2
                 WHERE attempt_id = ?1
                   AND status = 'awaiting_integration'
                   AND integration_confirmed_at IS NOT NULL",
                params![attempt_id.trim(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan_phase_derived_effects(attempt_id)
    }

    pub fn discard_plan_phase_derived_effects_for_phase(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        reason: &str,
    ) -> Result<usize, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phase_derived_effects
                 SET status = 'discarded',
                     terminal_reason = ?3,
                     discarded_at = COALESCE(discarded_at, ?4),
                     updated_at = ?4
                 WHERE plan_id = ?1 AND phase_id = ?2
                   AND status = 'awaiting_integration'
                   AND integration_confirmed_at IS NULL",
                params![plan_id.trim(), phase_id.trim(), reason.trim(), now],
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn discard_terminal_plan_phase_derived_effects(
        &mut self,
        default_reason: &str,
    ) -> Result<usize, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phase_derived_effects
                 SET status = 'discarded',
                     terminal_reason = COALESCE((
                         SELECT attempt.error_message
                         FROM plan_phase_attempts AS attempt
                         WHERE attempt.id = plan_phase_derived_effects.attempt_id
                     ), ?1),
                     discarded_at = COALESCE(discarded_at, ?2),
                     updated_at = ?2
                 WHERE status = 'awaiting_integration'
                   AND integration_confirmed_at IS NULL
                   AND EXISTS (
                       SELECT 1
                       FROM plan_phase_attempts AS attempt
                       WHERE attempt.id = plan_phase_derived_effects.attempt_id
                         AND attempt.status IN ('failed', 'cancelled', 'interrupted')
                   )",
                params![default_reason.trim(), now],
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn discard_superseded_plan_phase_derived_effects(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        current_attempt_id: &str,
        reason: &str,
    ) -> Result<usize, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phase_derived_effects
                 SET status = 'discarded',
                     terminal_reason = ?4,
                     discarded_at = COALESCE(discarded_at, ?5),
                     updated_at = ?5
                 WHERE plan_id = ?1 AND phase_id = ?2 AND attempt_id <> ?3
                   AND status = 'awaiting_integration'",
                params![
                    plan_id.trim(),
                    phase_id.trim(),
                    current_attempt_id.trim(),
                    reason.trim(),
                    now,
                ],
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn plan_phase_for_agent_task(
        &self,
        agent_task_id: &AgentTaskId,
    ) -> Result<Option<PlanPhaseRecord>, WorkspaceDatabaseError> {
        let Some(mut phase) = self
            .connection
            .query_row(
                "SELECT id, plan_id, sequence, title, summary, status,
                        implementation_chat_id, agent_team_id, agent_task_id,
                        commit_id, merge_attempt_count, error_message,
                        started_at, completed_at, created_at, updated_at
                 FROM plan_phases
                 WHERE agent_task_id = ?1",
                params![agent_task_id.as_str()],
                plan_phase_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))?
        else {
            return Ok(None);
        };
        phase.steps = self.plan_steps_for_phase(&phase.id)?;
        phase.attempts = self.plan_phase_attempts_for_phase_inner(&phase.id)?;
        Ok(Some(phase))
    }

    fn ensure_plan_phase_predecessors_completed(
        &self,
        plan: &PlanRecord,
        phase: &PlanPhaseRecord,
    ) -> Result<(), WorkspaceDatabaseError> {
        if let Some(predecessor) = plan.phases.iter().find(|candidate| {
            candidate.sequence < phase.sequence && candidate.status != "completed"
        }) {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan phase '{}' cannot start while earlier phase '{}' is {}",
                    phase.id, predecessor.id, predecessor.status
                ),
            });
        }
        Ok(())
    }

    fn plan_phase_for_plan(
        &self,
        plan_id: &str,
        phase_id: &str,
    ) -> Result<PlanPhaseRecord, WorkspaceDatabaseError> {
        let plan = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        plan.phases
            .into_iter()
            .find(|phase| phase.id == phase_id.trim())
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan phase '{}' does not belong to plan '{}'",
                    phase_id.trim(),
                    plan.id
                ),
            })
    }

    pub fn complete_plan_phase_run(
        &mut self,
        agent_task_id: &AgentTaskId,
        commit_id: Option<&str>,
    ) -> Result<Option<PlanRecord>, WorkspaceDatabaseError> {
        let Some(phase) = self.plan_phase_for_agent_task(agent_task_id)? else {
            return Ok(None);
        };
        self.update_attempt_for_task(
            agent_task_id,
            PlanPhaseAttemptStatus::Completed,
            commit_id,
            None,
        )?;
        self.complete_plan_phase_record(phase, commit_id).map(Some)
    }

    pub fn complete_plan_phase_by_id(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        commit_id: Option<&str>,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let phase = self.plan_phase_for_plan(plan_id, phase_id)?;
        self.update_latest_active_attempt_for_phase(
            plan_id,
            phase_id,
            PlanPhaseAttemptStatus::Completed,
            None,
        )?;
        self.complete_plan_phase_record(phase, commit_id)
    }

    fn complete_plan_phase_record(
        &mut self,
        phase: PlanPhaseRecord,
        commit_id: Option<&str>,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan =
            self.plan(&phase.plan_id)?
                .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                    message: format!("plan was not found: {}", phase.plan_id),
                })?;
        if matches!(plan.status.as_str(), "completed" | "cancelled") {
            return Ok(plan);
        }
        let commit_id = commit_id.map(str::trim).filter(|value| !value.is_empty());
        // ponytail: infer an out-of-band merge completion from a commit reported for an already blocked implemented plan; replace with an explicit outer-merge callback when Foco has one.
        let record_shared_merge_commit = plan.status == "implemented"
            && plan.shared_merge_commit_id.is_none()
            && plan.error_message.is_some()
            && commit_id.is_some();
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_steps
                 SET status = 'completed',
                     checked_at = COALESCE(checked_at, ?3),
                     updated_at = ?3
                 WHERE plan_id = ?1 AND phase_id = ?2",
                params![phase.plan_id.as_str(), phase.id.as_str(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plan_phases
                 SET status = 'completed',
                     commit_id = ?3,
                     error_message = NULL,
                     completed_at = COALESCE(completed_at, ?4),
                     updated_at = ?4
                 WHERE plan_id = ?1 AND id = ?2",
                params![phase.plan_id.as_str(), phase.id.as_str(), commit_id, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        let refreshed = self.refresh_plan_status_from_steps(&phase.plan_id)?;
        if refreshed.status == "ready" && refreshed.pause_requested_at.is_some() {
            self.connection
                .execute(
                    "UPDATE plans
                     SET status = 'paused',
                         active_phase_id = NULL,
                         updated_at = ?2
                     WHERE id = ?1",
                    params![phase.plan_id.as_str(), now],
                )
                .map_err(|source| self.sqlite_error(source))?;
            return self
                .plan(&phase.plan_id)
                .and_then(|plan| {
                    plan.ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                        message: format!("plan was not found after pause: {}", phase.plan_id),
                    })
                })
                .map(|plan| plan);
        }
        if record_shared_merge_commit {
            self.connection
                .execute(
                    "UPDATE plans
                     SET shared_merge_commit_id = ?2,
                         error_message = NULL,
                         updated_at = ?3
                     WHERE id = ?1",
                    params![phase.plan_id.as_str(), commit_id, now],
                )
                .map_err(|source| self.sqlite_error(source))?;
            self.clear_plan_auto_run_block()?;
            return self.plan(&phase.plan_id).and_then(|plan| {
                plan.ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                    message: format!(
                        "plan was not found after shared merge update: {}",
                        phase.plan_id
                    ),
                })
            });
        }
        Ok(refreshed)
    }

    pub fn fail_plan_phase_run(
        &mut self,
        agent_task_id: &AgentTaskId,
        error_message: &str,
    ) -> Result<Option<PlanRecord>, WorkspaceDatabaseError> {
        let Some(phase) = self.plan_phase_for_agent_task(agent_task_id)? else {
            return Ok(None);
        };
        let attempt_status = self.agent_task_attempt_terminal_status(agent_task_id)?;
        self.update_attempt_for_task(agent_task_id, attempt_status, None, Some(error_message))?;
        self.fail_plan_phase_record(phase, error_message).map(Some)
    }

    pub fn cancel_plan_phase_run(
        &mut self,
        agent_task_id: &AgentTaskId,
        error_message: &str,
    ) -> Result<Option<PlanRecord>, WorkspaceDatabaseError> {
        let Some(phase) = self.plan_phase_for_agent_task(agent_task_id)? else {
            return Ok(None);
        };
        self.cancel_plan_phase_record(phase, error_message)
            .map(Some)
    }

    pub fn completed_running_plan_phase_agent_tasks(
        &self,
    ) -> Result<Vec<AgentTaskId>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT DISTINCT task.id
                 FROM plan_phases AS phase
                 JOIN agent_tasks AS task ON task.id = phase.agent_task_id
                 WHERE phase.status = 'running'
                   AND task.status = 'completed'",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], |row| agent_id_from_row::<AgentTaskId>(row, 0))
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn fail_running_plan_phases_for_terminal_agent_tasks(
        &mut self,
        error_message: &str,
    ) -> Result<usize, WorkspaceDatabaseError> {
        let tasks = {
            let mut statement = self
                .connection
                .prepare(
                    "SELECT DISTINCT task.id, task.status, task.error_json
                     FROM plan_phases AS phase
                     JOIN agent_tasks AS task ON task.id = phase.agent_task_id
                     WHERE phase.status = 'running'
                       AND task.status IN ('failed', 'cancelled', 'interrupted')",
                )
                .map_err(|source| self.sqlite_error(source))?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        agent_id_from_row::<AgentTaskId>(row, 0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                })
                .map_err(|source| self.sqlite_error(source))?;
            collect_rows(rows, &self.database_path)?
        };
        let count = tasks.len();
        for (task_id, status, task_error_json) in tasks {
            if status == AgentTaskStatus::Cancelled.as_str() {
                let task_error_message = task_error_json
                    .as_deref()
                    .and_then(|error_json| serde_json::from_str::<Value>(error_json).ok())
                    .and_then(|value| {
                        value
                            .get("message")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    });
                self.cancel_plan_phase_run(
                    &task_id,
                    task_error_message.as_deref().unwrap_or(error_message),
                )?;
            } else {
                self.fail_plan_phase_run(&task_id, error_message)?;
            }
        }
        Ok(count)
    }

    pub fn fail_running_plan_phases_without_agent_runs(
        &mut self,
        error_message: &str,
    ) -> Result<usize, WorkspaceDatabaseError> {
        let phases = {
            let mut statement = self
                .connection
                .prepare(
                    "SELECT plan_id, id
                     FROM plan_phases
                     WHERE status = 'running'
                       AND implementation_chat_id IS NULL
                       AND agent_team_id IS NULL
                       AND agent_task_id IS NULL",
                )
                .map_err(|source| self.sqlite_error(source))?;
            let rows = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|source| self.sqlite_error(source))?;
            collect_rows(rows, &self.database_path)?
        };
        let count = phases.len();
        for (plan_id, phase_id) in phases {
            self.fail_plan_phase_by_id(&plan_id, &phase_id, error_message)?;
        }
        Ok(count)
    }

    pub fn reconcile_plan_phase_attempts_for_terminal_phases(
        &mut self,
    ) -> Result<usize, WorkspaceDatabaseError> {
        self.reconcile_plan_phase_attempts_for_terminal_phases_inner(None)
    }

    fn reconcile_plan_phase_attempts_for_terminal_phases_in_plan(
        &mut self,
        plan_id: &str,
    ) -> Result<usize, WorkspaceDatabaseError> {
        self.reconcile_plan_phase_attempts_for_terminal_phases_inner(Some(plan_id.trim()))
    }

    fn reconcile_plan_phase_attempts_for_terminal_phases_inner(
        &mut self,
        plan_id: Option<&str>,
    ) -> Result<usize, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phase_attempts
                 SET status = (
                         SELECT phase.status
                         FROM plan_phases AS phase
                         WHERE phase.id = plan_phase_attempts.phase_id
                           AND phase.plan_id = plan_phase_attempts.plan_id
                     ),
                     commit_id = CASE
                         WHEN (
                             SELECT phase.status
                             FROM plan_phases AS phase
                             WHERE phase.id = plan_phase_attempts.phase_id
                               AND phase.plan_id = plan_phase_attempts.plan_id
                         ) = 'completed'
                         THEN (
                             SELECT phase.commit_id
                             FROM plan_phases AS phase
                             WHERE phase.id = plan_phase_attempts.phase_id
                               AND phase.plan_id = plan_phase_attempts.plan_id
                         )
                         ELSE commit_id
                     END,
                     error_message = CASE
                         WHEN (
                             SELECT phase.status
                             FROM plan_phases AS phase
                             WHERE phase.id = plan_phase_attempts.phase_id
                               AND phase.plan_id = plan_phase_attempts.plan_id
                         ) = 'failed'
                         THEN (
                             SELECT phase.error_message
                             FROM plan_phases AS phase
                             WHERE phase.id = plan_phase_attempts.phase_id
                               AND phase.plan_id = plan_phase_attempts.plan_id
                         )
                         WHEN (
                             SELECT phase.status
                             FROM plan_phases AS phase
                             WHERE phase.id = plan_phase_attempts.phase_id
                               AND phase.plan_id = plan_phase_attempts.plan_id
                         ) IN ('completed', 'cancelled')
                         THEN NULL
                         ELSE error_message
                     END,
                     completed_at = COALESCE((
                         SELECT phase.completed_at
                         FROM plan_phases AS phase
                         WHERE phase.id = plan_phase_attempts.phase_id
                           AND phase.plan_id = plan_phase_attempts.plan_id
                     ), completed_at, ?1),
                     updated_at = ?1
                 WHERE status IN ('queued', 'running')
                   AND (?2 IS NULL OR plan_id = ?2)
                   AND EXISTS (
                       SELECT 1
                       FROM plan_phases AS phase
                       WHERE phase.id = plan_phase_attempts.phase_id
                         AND phase.plan_id = plan_phase_attempts.plan_id
                         AND phase.status IN ('completed', 'failed', 'cancelled')
                   )",
                params![now, plan_id],
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn fail_plan_phase_by_id(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        error_message: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let phase = self.plan_phase_for_plan(plan_id, phase_id)?;
        self.update_latest_active_attempt_for_phase(
            plan_id,
            phase_id,
            PlanPhaseAttemptStatus::Failed,
            Some(error_message),
        )?;
        self.fail_plan_phase_record(phase, error_message)
    }

    pub fn cancel_plan_phase_by_id(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        error_message: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let phase = self.plan_phase_for_plan(plan_id, phase_id)?;
        self.cancel_plan_phase_record(phase, error_message)
    }

    fn cancel_plan_phase_record(
        &mut self,
        phase: PlanPhaseRecord,
        error_message: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan =
            self.plan(&phase.plan_id)?
                .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                    message: format!("plan was not found: {}", phase.plan_id),
                })?;
        if matches!(plan.status.as_str(), "completed" | "cancelled") {
            return Ok(plan);
        }
        let now = now_timestamp();
        let error_message = match error_message.trim() {
            "" => "Plan phase run was cancelled",
            message => message,
        };
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "UPDATE plan_phase_attempts
                 SET status = 'cancelled',
                     error_message = ?3,
                     completed_at = COALESCE(completed_at, ?4),
                     updated_at = ?4
                 WHERE plan_id = ?1
                   AND phase_id = ?2
                   AND status IN ('queued', 'running')",
                params![
                    phase.plan_id.as_str(),
                    phase.id.as_str(),
                    error_message,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "UPDATE plan_steps
                 SET status = 'cancelled',
                     checked_at = NULL,
                     updated_at = ?3
                 WHERE plan_id = ?1
                   AND phase_id = ?2
                   AND status IN ('pending', 'running', 'failed')",
                params![phase.plan_id.as_str(), phase.id.as_str(), now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "UPDATE plan_phases
                 SET status = 'cancelled',
                     error_message = ?3,
                     completed_at = COALESCE(completed_at, ?4),
                     updated_at = ?4
                 WHERE plan_id = ?1 AND id = ?2",
                params![
                    phase.plan_id.as_str(),
                    phase.id.as_str(),
                    error_message,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "UPDATE plans
                 SET status = 'paused',
                     active_phase_id = NULL,
                     pause_requested_at = ?3,
                     error_message = ?2,
                     completed_at = NULL,
                     completed_by_user_at = NULL,
                     updated_at = ?3
                 WHERE id = ?1",
                params![phase.plan_id.as_str(), error_message, now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "INSERT INTO workspace_metadata (key, value, updated_at)
                 VALUES (?1, 'cancelled_phase', ?2)
                 ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    updated_at = excluded.updated_at",
                params![PLAN_AUTO_RUN_BLOCKED_REASON_KEY, now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "INSERT INTO workspace_metadata (key, value, updated_at)
                 VALUES (?1, ?2, ?4), (?3, ?5, ?4)
                 ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    updated_at = excluded.updated_at",
                params![
                    PLAN_AUTO_RUN_BLOCKED_PLAN_ID_KEY,
                    phase.plan_id.as_str(),
                    PLAN_AUTO_RUN_BLOCKED_PHASE_ID_KEY,
                    now,
                    phase.id.as_str()
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        self.plan(&phase.plan_id).and_then(|plan| {
            plan.ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan was not found after phase cancellation: {}",
                    phase.plan_id
                ),
            })
        })
    }

    fn fail_plan_phase_record(
        &mut self,
        phase: PlanPhaseRecord,
        error_message: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan =
            self.plan(&phase.plan_id)?
                .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                    message: format!("plan was not found: {}", phase.plan_id),
                })?;
        if matches!(plan.status.as_str(), "completed" | "cancelled") {
            return Ok(plan);
        }
        let now = now_timestamp();
        let error_message = error_message.trim();
        self.connection
            .execute(
                "UPDATE plan_steps
                 SET status = 'failed',
                     checked_at = NULL,
                     updated_at = ?3
                 WHERE plan_id = ?1 AND phase_id = ?2 AND status IN ('pending', 'running')",
                params![phase.plan_id.as_str(), phase.id.as_str(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plan_phases
                 SET status = 'failed',
                     error_message = ?3,
                     completed_at = COALESCE(completed_at, ?4),
                     updated_at = ?4
                 WHERE plan_id = ?1 AND id = ?2",
                params![
                    phase.plan_id.as_str(),
                    phase.id.as_str(),
                    error_message,
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plans
                 SET status = 'failed',
                     active_phase_id = NULL,
                     error_message = ?2,
                     completed_at = ?3,
                     completed_by_user_at = NULL,
                     updated_at = ?3
                 WHERE id = ?1",
                params![phase.plan_id.as_str(), error_message, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.block_plan_auto_run(
            "waiting_for_retry",
            Some(phase.plan_id.as_str()),
            Some(phase.id.as_str()),
        )?;
        self.plan(&phase.plan_id).and_then(|plan| {
            plan.ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found after phase failure: {}", phase.plan_id),
            })
        })
    }
    pub fn block_plan_phase_merge(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        error_message: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let phase = self.plan_phase_for_plan(plan_id, phase_id)?;
        let now = now_timestamp();
        let error_message = error_message.trim();
        self.connection
            .execute(
                "UPDATE plan_phases
                 SET status = 'completed',
                     error_message = ?3,
                     completed_at = COALESCE(completed_at, ?4),
                     updated_at = ?4
                 WHERE plan_id = ?1 AND id = ?2",
                params![
                    phase.plan_id.as_str(),
                    phase.id.as_str(),
                    error_message,
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plan_phase_attempts
                 SET status = 'failed',
                     error_message = ?3,
                     completed_at = COALESCE(completed_at, ?4),
                     updated_at = ?4
                 WHERE plan_id = ?1
                   AND phase_id = ?2
                   AND trigger = 'merge_auto'
                   AND status IN ('queued', 'running')",
                params![
                    phase.plan_id.as_str(),
                    phase.id.as_str(),
                    error_message,
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plans
                 SET status = 'implemented',
                     active_phase_id = NULL,
                     error_message = ?2,
                     completed_at = COALESCE(completed_at, ?3),
                     completed_by_user_at = NULL,
                     updated_at = ?3
                 WHERE id = ?1",
                params![phase.plan_id.as_str(), error_message, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.block_plan_auto_run(
            "merge_blocked",
            Some(phase.plan_id.as_str()),
            Some(phase.id.as_str()),
        )?;
        self.plan(&phase.plan_id).and_then(|plan| {
            plan.ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found after merge block: {}", phase.plan_id),
            })
        })
    }

    pub fn try_begin_plan_phase_merge_attempt(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        error_message: &str,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let phase = self.plan_phase_for_plan(plan_id, phase_id)?;
        let now = now_timestamp();
        let updated = self
            .connection
            .execute(
                "UPDATE plan_phases
                 SET merge_attempt_count = merge_attempt_count + 1,
                     error_message = ?3,
                     updated_at = ?4
                 WHERE plan_id = ?1 AND id = ?2 AND merge_attempt_count = 0",
                params![
                    phase.plan_id.as_str(),
                    phase.id.as_str(),
                    error_message.trim(),
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        Ok(updated == 1)
    }

    pub fn fail_plan_phase_start(
        &mut self,
        plan_id: &str,
        phase_id: &str,
        error_message: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        self.fail_plan_phase_by_id(plan_id, phase_id, error_message)
    }

    fn earliest_incomplete_plan_phase<'a>(
        &self,
        plan: &'a PlanRecord,
    ) -> Option<&'a PlanPhaseRecord> {
        // Execution order is a store invariant, not a scheduler convention: a
        // target phase may start only when every earlier phase is completed.
        plan.phases.iter().find(|phase| phase.status != "completed")
    }

    fn start_next_plan_phase(
        &mut self,
        plan_id: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        if matches!(plan.status.as_str(), "completed" | "cancelled") {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan '{}' cannot start while {}", plan.id, plan.status),
            });
        }
        let now = now_timestamp();
        let Some(next_phase) = self.earliest_incomplete_plan_phase(&plan) else {
            self.connection
                .execute(
                    "UPDATE plans
                     SET status = 'implemented',
                         completed_at = COALESCE(completed_at, ?2),
                         completed_by_user_at = NULL,
                         pause_requested_at = NULL,
                         active_phase_id = NULL,
                         updated_at = ?2
                     WHERE id = ?1",
                    params![plan.id, now],
                )
                .map_err(|source| self.sqlite_error(source))?;
            return self
                .plan(plan_id)?
                .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                    message: format!("plan was not found after start: {}", plan_id.trim()),
                });
        };
        if next_phase.status == "cancelled" {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!(
                    "plan phase '{}' was cancelled and must be retried explicitly with Retry before starting or resuming the plan",
                    next_phase.id
                ),
            });
        }
        if next_phase.status == "running" {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan phase '{}' is already running", next_phase.id),
            });
        }
        let next_phase_id = next_phase.id.clone();
        self.connection
            .execute(
                "UPDATE plan_steps
                 SET status = 'pending',
                     checked_at = NULL,
                     updated_at = ?3
                 WHERE plan_id = ?1
                   AND phase_id = ?2
                   AND EXISTS (
                       SELECT 1 FROM plan_phases
                       WHERE plan_id = ?1 AND id = ?2 AND status = 'failed'
                   )",
                params![plan.id.as_str(), next_phase_id.as_str(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plan_phases
                 SET status = 'running',
                     implementation_chat_id = CASE WHEN status = 'failed' THEN NULL ELSE implementation_chat_id END,
                     agent_team_id = CASE WHEN status = 'failed' THEN NULL ELSE agent_team_id END,
                     agent_task_id = CASE WHEN status = 'failed' THEN NULL ELSE agent_task_id END,
                     commit_id = CASE WHEN status = 'failed' THEN NULL ELSE commit_id END,
                     merge_attempt_count = CASE WHEN status = 'failed' THEN 0 ELSE merge_attempt_count END,
                     error_message = NULL,
                     started_at = CASE WHEN status = 'failed' THEN ?3 ELSE COALESCE(started_at, ?3) END,
                     completed_at = NULL,
                     updated_at = ?3
                 WHERE plan_id = ?1 AND id = ?2",
                params![plan.id, next_phase_id, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plans
                 SET status = 'running',
                     active_phase_id = ?2,
                     pause_requested_at = NULL,
                     completed_at = NULL,
                     completed_by_user_at = NULL,
                     updated_at = ?3
                 WHERE id = ?1",
                params![plan_id.trim(), next_phase_id, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found after start: {}", plan_id.trim()),
            })
    }

    fn pause_plan(&mut self, plan_id: &str) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        if !matches!(plan.status.as_str(), "running" | "ready") {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan '{}' cannot pause while {}", plan.id, plan.status),
            });
        }
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plans
                 SET status = 'paused',
                     pause_requested_at = ?2,
                     updated_at = ?2
                 WHERE id = ?1",
                params![plan.id, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found after pause: {}", plan_id.trim()),
            })
    }

    fn cancel_plan(&mut self, plan_id: &str) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        if plan.status == "completed" {
            return Err(WorkspaceDatabaseError::InvalidPlan {
                message: "completed plans cannot be cancelled".to_string(),
            });
        }
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE plan_phases
                 SET status = 'cancelled',
                     completed_at = COALESCE(completed_at, ?2),
                     updated_at = ?2
                 WHERE plan_id = ?1 AND status IN ('pending', 'running', 'failed')",
                params![plan.id, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plan_phase_attempts
                 SET status = 'cancelled',
                     completed_at = COALESCE(completed_at, ?2),
                     updated_at = ?2
                 WHERE plan_id = ?1 AND status IN ('queued', 'running')",
                params![plan_id.trim(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plan_steps
                 SET status = 'cancelled',
                     checked_at = NULL,
                     updated_at = ?2
                 WHERE plan_id = ?1 AND status IN ('pending', 'running', 'failed')",
                params![plan_id.trim(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE plans
                 SET status = 'cancelled',
                     active_phase_id = NULL,
                     pause_requested_at = NULL,
                     completed_at = ?2,
                     completed_by_user_at = NULL,
                     updated_at = ?2
                 WHERE id = ?1",
                params![plan_id.trim(), now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found after cancel: {}", plan_id.trim()),
            })
    }

    fn refresh_plan_status_from_steps(
        &mut self,
        plan_id: &str,
    ) -> Result<PlanRecord, WorkspaceDatabaseError> {
        let plan = self
            .plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found: {}", plan_id.trim()),
            })?;
        if matches!(plan.status.as_str(), "completed" | "cancelled") {
            self.reconcile_plan_phase_attempts_for_terminal_phases_in_plan(plan_id)?;
            return self
                .plan(plan_id)?
                .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                    message: format!("plan was not found after refresh: {}", plan_id.trim()),
                });
        }
        let now = now_timestamp();
        for phase in &plan.phases {
            let (total, completed, running, failed, cancelled): (i64, i64, i64, i64, i64) = self
                .connection
                .query_row(
                    "SELECT
                        COUNT(*),
                        COUNT(CASE WHEN status = 'completed' THEN 1 END),
                        COUNT(CASE WHEN status = 'running' THEN 1 END),
                        COUNT(CASE WHEN status = 'failed' THEN 1 END),
                        COUNT(CASE WHEN status = 'cancelled' THEN 1 END)
                     FROM plan_steps
                     WHERE phase_id = ?1",
                    params![phase.id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .map_err(|source| self.sqlite_error(source))?;
            let status = if phase.status == "cancelled" {
                "cancelled"
            } else if failed > 0 {
                "failed"
            } else if total > 0 && completed == total {
                "completed"
            } else if running > 0 || phase.status == "running" {
                "running"
            } else if total > 0 && cancelled == total {
                "cancelled"
            } else {
                "pending"
            };
            self.connection
                .execute(
                    "UPDATE plan_phases
                     SET status = ?2,
                         completed_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled') THEN COALESCE(completed_at, ?3) ELSE NULL END,
                         updated_at = ?3
                     WHERE id = ?1 AND status <> ?2",
                    params![phase.id, status, now],
                )
                .map_err(|source| self.sqlite_error(source))?;
        }
        let phases = self.plan_phases_for_plan(plan_id)?;
        let total_phases =
            i64::try_from(phases.len()).map_err(|source| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan phase count overflowed: {source}"),
            })?;
        let any_failed = phases.iter().any(|phase| phase.status == "failed");
        let any_cancelled = phases.iter().any(|phase| phase.status == "cancelled");
        let any_running = phases.iter().any(|phase| phase.status == "running");
        let all_completed =
            total_phases > 0 && phases.iter().all(|phase| phase.status == "completed");
        let current_status = self
            .connection
            .query_row(
                "SELECT status FROM plans WHERE id = ?1",
                params![plan_id.trim()],
                |row| row.get::<_, String>(0),
            )
            .map_err(|source| self.sqlite_error(source))?;
        let next_status = if any_cancelled {
            "paused"
        } else if any_failed {
            "failed"
        } else if all_completed {
            "implemented"
        } else if current_status == "paused" {
            "paused"
        } else if any_running {
            "running"
        } else if current_status == "draft" {
            "draft"
        } else {
            "ready"
        };
        let active_phase_id = phases
            .iter()
            .find(|phase| phase.status == "running")
            .map(|phase| phase.id.as_str());
        self.connection
            .execute(
                "UPDATE plans
                 SET status = ?2,
                     active_phase_id = ?3,
                     completed_at = CASE WHEN ?2 = 'implemented' THEN COALESCE(completed_at, ?4) ELSE NULL END,
                     completed_by_user_at = NULL,
                     updated_at = ?4
                 WHERE id = ?1",
                params![plan_id.trim(), next_status, active_phase_id, now],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.reconcile_plan_phase_attempts_for_terminal_phases_in_plan(plan_id)?;
        self.plan(plan_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidPlan {
                message: format!("plan was not found after refresh: {}", plan_id.trim()),
            })
    }

    fn plan_step(&self, id: &str) -> Result<Option<PlanStepRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, plan_id, phase_id, sequence, title, detail, acceptance_json,
                        status, checked_at, created_at, updated_at
                 FROM plan_steps
                 WHERE id = ?1",
                params![id.trim()],
                plan_step_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    fn plan_steps_for_phase(
        &self,
        phase_id: &str,
    ) -> Result<Vec<PlanStepRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, plan_id, phase_id, sequence, title, detail, acceptance_json,
                        status, checked_at, created_at, updated_at
                 FROM plan_steps
                 WHERE phase_id = ?1
                 ORDER BY sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![phase_id.trim()], plan_step_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    fn plan_phases_for_plan(
        &self,
        plan_id: &str,
    ) -> Result<Vec<PlanPhaseRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, plan_id, sequence, title, summary, status,
                        implementation_chat_id, agent_team_id, agent_task_id, commit_id,
                        merge_attempt_count, error_message, started_at, completed_at,
                        created_at, updated_at
                 FROM plan_phases
                 WHERE plan_id = ?1
                 ORDER BY sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![plan_id.trim()], plan_phase_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        let mut phases = collect_rows(rows, &self.database_path)?;
        for phase in &mut phases {
            phase.steps = self.plan_steps_for_phase(&phase.id)?;
            phase.attempts = self.plan_phase_attempts_for_phase_inner(&phase.id)?;
        }

        Ok(phases)
    }

    pub fn insert_chat(&mut self, id: &str, title: &str) -> Result<(), WorkspaceDatabaseError> {
        let now = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO chats (id, title, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, title, now, now],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn insert_chat_with_metadata(
        &mut self,
        id: &str,
        title: &str,
        metadata_json: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        validate_json_metadata(metadata_json, "chat metadata")?;
        let now = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO chats (id, title, created_at, updated_at, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, title, now, now, metadata_json],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn set_chat_queued_run(
        &mut self,
        chat_id: &str,
        queued_run_json: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        let chat =
            self.chat(chat_id)?
                .ok_or_else(|| WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: format!("chat was not found: {chat_id}"),
                })?;
        let mut chat_metadata = parse_json_object(&chat.metadata_json, "chat metadata")?;
        let queued_run = serde_json::from_str::<Value>(queued_run_json).map_err(|source| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("chat queued run is invalid JSON: {source}"),
            }
        })?;
        if !queued_run.is_object() {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "chat queued run must be an object".to_string(),
            });
        }
        chat_metadata.insert(QUEUED_CHAT_METADATA_KEY.to_string(), queued_run);
        let metadata_json = serde_json::to_string(&chat_metadata).map_err(|source| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("chat metadata is invalid JSON: {source}"),
            }
        })?;

        self.connection
            .execute(
                "UPDATE chats SET metadata_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![metadata_json, now_timestamp(), chat_id],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn update_chat_title_if_current(
        &mut self,
        id: &str,
        current_title: &str,
        title: &str,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let updated = self
            .connection
            .execute(
                "UPDATE chats SET title = ?1, updated_at = ?2 WHERE id = ?3 AND title = ?4",
                params![title, now_timestamp(), id, current_title],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(updated > 0)
    }

    pub fn chat(&self, id: &str) -> Result<Option<ChatRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, title, created_at, updated_at, archived_at, metadata_json
                 FROM chats
                 WHERE id = ?1",
                params![id],
                |row| {
                    Ok(ChatRecord {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                        archived_at: row.get(4)?,
                        metadata_json: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    /// Bounded primary-key existence check for a page of chat ids.
    /// Production list paths must not scan every chat in the workspace.
    pub fn existing_chat_ids(
        &self,
        chat_ids: &[String],
    ) -> Result<HashSet<String>, WorkspaceDatabaseError> {
        if chat_ids.is_empty() {
            return Ok(HashSet::new());
        }

        let placeholders = (1..=chat_ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("SELECT id FROM chats WHERE id IN ({placeholders})");
        let query_params = chat_ids
            .iter()
            .cloned()
            .map(SqlValue::Text)
            .collect::<Vec<_>>();
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                row.get::<_, String>(0)
            })
            .map_err(|source| self.sqlite_error(source))?;

        let mut existing = HashSet::with_capacity(chat_ids.len());
        for row in rows {
            existing.insert(row.map_err(|source| self.sqlite_error(source))?);
        }
        Ok(existing)
    }

    /// Bounded title lookup for chat ids (parameterized IN list, chunked).
    /// Missing chats are omitted so callers can map absent ids to `null`.
    ///
    /// Queries are split into fixed-size chunks so deep Spec job pages with many
    /// unique chat ids cannot exceed SQLite's variable limit.
    pub fn chat_titles_by_ids(
        &self,
        chat_ids: &[String],
    ) -> Result<HashMap<String, String>, WorkspaceDatabaseError> {
        if chat_ids.is_empty() {
            return Ok(HashMap::new());
        }

        const CHUNK_SIZE: usize = 500;
        let mut titles = HashMap::with_capacity(chat_ids.len());
        for chunk in chat_ids.chunks(CHUNK_SIZE) {
            let placeholders = (1..=chunk.len())
                .map(|index| format!("?{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!("SELECT id, title FROM chats WHERE id IN ({placeholders})");
            let query_params = chunk
                .iter()
                .cloned()
                .map(SqlValue::Text)
                .collect::<Vec<_>>();
            let mut statement = self
                .connection
                .prepare(&sql)
                .map_err(|source| self.sqlite_error(source))?;
            let rows = statement
                .query_map(params_from_iter(query_params), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|source| self.sqlite_error(source))?;

            for row in rows {
                let (id, title) = row.map_err(|source| self.sqlite_error(source))?;
                titles.insert(id, title);
            }
        }
        Ok(titles)
    }

    pub fn delete_chat(&mut self, id: &str) -> Result<bool, WorkspaceDatabaseError> {
        let deleted = self
            .connection
            .execute("DELETE FROM chats WHERE id = ?1", params![id])
            .map_err(|source| self.sqlite_error(source))?;

        Ok(deleted > 0)
    }

    pub fn chats(&self) -> Result<Vec<ChatRecord>, WorkspaceDatabaseError> {
        self.chats_matching_kind(None)
    }

    pub fn chat_count(&self) -> Result<usize, WorkspaceDatabaseError> {
        self.chat_count_matching_title(None)
    }

    pub fn chat_page(
        &self,
        limit: usize,
        cursor: Option<&ChatPageCursor>,
    ) -> Result<ChatPage, WorkspaceDatabaseError> {
        self.chat_page_matching_title(None, limit, cursor)
    }

    pub fn search_chats(
        &self,
        query: &str,
        limit: usize,
        cursor: Option<&ChatPageCursor>,
    ) -> Result<ChatPage, WorkspaceDatabaseError> {
        self.chat_page_matching_title(Some(query), limit, cursor)
    }

    pub fn dream_transcript_chats(&self) -> Result<Vec<ChatRecord>, WorkspaceDatabaseError> {
        self.chats_matching_kind(Some(MEMORY_DREAM_TRANSCRIPT_CHAT_KIND))
    }

    fn chat_page_matching_title(
        &self,
        title_query: Option<&str>,
        limit: usize,
        cursor: Option<&ChatPageCursor>,
    ) -> Result<ChatPage, WorkspaceDatabaseError> {
        let limit = limit.max(1);
        let total_count = self.chat_count_matching_title(title_query)?;
        let mut sql = String::from(
            "SELECT id, title, created_at, updated_at, archived_at, metadata_json
             FROM chats
             WHERE COALESCE(json_extract(metadata_json, '$.kind'), '') != 'memory_dream'",
        );
        let mut query_params = Vec::new();

        if let Some(query) = title_query {
            sql.push_str(" AND title LIKE ? ESCAPE '\\' COLLATE NOCASE");
            query_params.push(SqlValue::Text(like_contains_pattern(query)));
        }

        if let Some(cursor) = cursor {
            sql.push_str(
                " AND (
                    updated_at < ?
                    OR (updated_at = ? AND created_at < ?)
                    OR (updated_at = ? AND created_at = ? AND id < ?)
                 )",
            );
            query_params.push(SqlValue::Text(cursor.updated_at.clone()));
            query_params.push(SqlValue::Text(cursor.updated_at.clone()));
            query_params.push(SqlValue::Text(cursor.created_at.clone()));
            query_params.push(SqlValue::Text(cursor.updated_at.clone()));
            query_params.push(SqlValue::Text(cursor.created_at.clone()));
            query_params.push(SqlValue::Text(cursor.id.clone()));
        }

        sql.push_str(" ORDER BY updated_at DESC, created_at DESC, id DESC LIMIT ?");
        query_params.push(SqlValue::Integer((limit + 1) as i64));

        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), chat_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        let mut chats = collect_rows(rows, &self.database_path)?;
        let has_more = chats.len() > limit;
        if has_more {
            chats.truncate(limit);
        }
        let next_cursor = if has_more {
            chats.last().map(|chat| ChatPageCursor {
                updated_at: chat.updated_at.clone(),
                created_at: chat.created_at.clone(),
                id: chat.id.clone(),
            })
        } else {
            None
        };

        Ok(ChatPage {
            chats,
            total_count,
            has_more,
            next_cursor,
        })
    }

    fn chat_count_matching_title(
        &self,
        title_query: Option<&str>,
    ) -> Result<usize, WorkspaceDatabaseError> {
        let mut sql = String::from(
            "SELECT COUNT(*)
             FROM chats
             WHERE COALESCE(json_extract(metadata_json, '$.kind'), '') != 'memory_dream'",
        );
        let mut query_params = Vec::new();

        if let Some(query) = title_query {
            sql.push_str(" AND title LIKE ? ESCAPE '\\' COLLATE NOCASE");
            query_params.push(SqlValue::Text(like_contains_pattern(query)));
        }

        let count = self
            .connection
            .query_row(&sql, params_from_iter(query_params), |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|source| self.sqlite_error(source))?;

        usize::try_from(count).map_err(|_| WorkspaceDatabaseError::InvalidMessageMetadata {
            message: "chat count is too large".to_string(),
        })
    }

    pub fn code_change_stats_for_chats(
        &self,
        chat_ids: &[String],
    ) -> Result<HashMap<String, CodeChangeStats>, WorkspaceDatabaseError> {
        if chat_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders = (1..=chat_ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT chat_id, metadata_json
             FROM messages
             WHERE role = 'assistant'
               AND chat_id IN ({placeholders})",
        );
        let query_params = chat_ids
            .iter()
            .cloned()
            .map(SqlValue::Text)
            .collect::<Vec<_>>();
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|source| self.sqlite_error(source))?;

        self.code_change_stats_from_rows(rows)
    }

    fn chats_matching_kind(
        &self,
        kind: Option<&str>,
    ) -> Result<Vec<ChatRecord>, WorkspaceDatabaseError> {
        let (sql, params): (&str, Vec<SqlValue>) = match kind {
            Some(kind) => (
                "SELECT id, title, created_at, updated_at, archived_at, metadata_json
                 FROM chats
                 WHERE json_extract(metadata_json, '$.kind') = ?1
                 ORDER BY updated_at DESC, created_at DESC, id DESC",
                vec![SqlValue::Text(kind.to_string())],
            ),
            None => (
                "SELECT id, title, created_at, updated_at, archived_at, metadata_json
                 FROM chats
                 WHERE COALESCE(json_extract(metadata_json, '$.kind'), '') != 'memory_dream'
                 ORDER BY updated_at DESC, created_at DESC, id DESC",
                Vec::new(),
            ),
        };
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(params), chat_from_row)
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    fn code_change_stats_from_rows(
        &self,
        rows: rusqlite::MappedRows<
            '_,
            impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<(String, String)>,
        >,
    ) -> Result<HashMap<String, CodeChangeStats>, WorkspaceDatabaseError> {
        let mut stats_by_chat = HashMap::new();

        for row in rows {
            let (chat_id, metadata_json) = row.map_err(|source| self.sqlite_error(source))?;
            let metadata = serde_json::from_str::<Value>(&metadata_json).map_err(|source| {
                WorkspaceDatabaseError::InvalidAuditJson {
                    field: "message metadata_json",
                    source,
                }
            })?;
            let Some(stats_value) = metadata.get("codeChangeStats") else {
                continue;
            };
            let stats = CodeChangeStats::from_metadata(stats_value)?;
            if stats.additions == 0 && stats.deletions == 0 {
                continue;
            }
            let entry = stats_by_chat
                .entry(chat_id)
                .or_insert_with(CodeChangeStats::default);
            entry.additions += stats.additions;
            entry.deletions += stats.deletions;
        }

        Ok(stats_by_chat)
    }

    pub fn code_change_stats_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<CodeChangeStats, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT metadata_json
                 FROM messages
                 WHERE chat_id = ?1 AND role = 'assistant'",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| row.get::<_, String>(0))
            .map_err(|source| self.sqlite_error(source))?;
        let mut total = CodeChangeStats::default();

        for row in rows {
            let metadata_json = row.map_err(|source| self.sqlite_error(source))?;
            let metadata = serde_json::from_str::<Value>(&metadata_json).map_err(|source| {
                WorkspaceDatabaseError::InvalidAuditJson {
                    field: "message metadata_json",
                    source,
                }
            })?;
            let Some(stats_value) = metadata.get("codeChangeStats") else {
                continue;
            };
            let stats = CodeChangeStats::from_metadata(stats_value)?;
            total.additions += stats.additions;
            total.deletions += stats.deletions;
        }

        Ok(total)
    }

    pub fn has_user_message_since(&self, since: &str) -> Result<bool, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1
                     FROM messages
                     WHERE role = 'user' AND created_at >= ?1
                     LIMIT 1
                 )",
                params![since],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn insert_message(
        &mut self,
        message: NewMessage<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let now = now_timestamp();
        let metadata_json = message.metadata_json.unwrap_or("{}");
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;

        transaction
            .execute(
                "INSERT INTO messages
                    (id, chat_id, role, content, sequence, created_at, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    message.id,
                    message.chat_id,
                    message.role,
                    message.content,
                    message.sequence,
                    now,
                    metadata_json
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        transaction
            .execute(
                "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
                params![now, message.chat_id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(())
    }

    pub fn insert_message_if_absent(
        &mut self,
        message: NewMessage<'_>,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let now = now_timestamp();
        let metadata_json = message.metadata_json.unwrap_or("{}");
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO messages
                    (id, chat_id, role, content, sequence, created_at, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    message.id,
                    message.chat_id,
                    message.role,
                    message.content,
                    message.sequence,
                    now,
                    metadata_json
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        if inserted > 0 {
            transaction
                .execute(
                    "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
                    params![now, message.chat_id],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(inserted > 0)
    }

    pub fn mark_chat_queued_run_started(
        &mut self,
        chat_id: &str,
        user_message_id: &str,
        assistant_message_id: &str,
        assistant_sequence: i64,
    ) -> Result<(), WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let chat =
            chat_from_transaction(&transaction, &database_path, chat_id)?.ok_or_else(|| {
                WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: format!("chat was not found: {chat_id}"),
                }
            })?;
        let mut chat_metadata = parse_json_object(&chat.metadata_json, "chat metadata")?;
        // Rebuild when missing: list/message APIs may clear queuedRun before the Agent task
        // is visible (new chat insert has queuedRun before team/task exists).
        match chat_metadata.get_mut(QUEUED_CHAT_METADATA_KEY) {
            Some(queued_run) => {
                let Some(queued_run_object) = queued_run.as_object_mut() else {
                    return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                        message: "chat metadata.queuedRun must be an object".to_string(),
                    });
                };
                let existing_user_message_id = queued_run_object
                    .get("userMessageId")
                    .or_else(|| queued_run_object.get("user_message_id"))
                    .and_then(Value::as_str);
                if let Some(existing_user_message_id) = existing_user_message_id
                    && existing_user_message_id != user_message_id
                {
                    return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                        message: format!(
                            "chat metadata.queuedRun.userMessageId '{existing_user_message_id}' does not match '{user_message_id}'"
                        ),
                    });
                }
                queued_run_object
                    .insert("status".to_string(), Value::String("running".to_string()));
                queued_run_object.insert(
                    "userMessageId".to_string(),
                    Value::String(user_message_id.to_string()),
                );
                queued_run_object.insert(
                    "assistantMessageId".to_string(),
                    Value::String(assistant_message_id.to_string()),
                );
                queued_run_object.insert(
                    "assistantSequence".to_string(),
                    Value::Number(assistant_sequence.into()),
                );
            }
            None => {
                chat_metadata.insert(
                    QUEUED_CHAT_METADATA_KEY.to_string(),
                    json!({
                        "status": "running",
                        "userMessageId": user_message_id,
                        "assistantMessageId": assistant_message_id,
                        "assistantSequence": assistant_sequence,
                    }),
                );
            }
        }
        let chat_metadata_json = serde_json::to_string(&chat_metadata).map_err(|source| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("chat metadata is invalid JSON: {source}"),
            }
        })?;

        let message = message_from_transaction(&transaction, &database_path, user_message_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("message was not found: {user_message_id}"),
            })?;
        let mut message_metadata =
            parse_json_object(&message.metadata_json, "user message metadata")?;
        match message_metadata.get_mut(QUEUED_MESSAGE_METADATA_KEY) {
            Some(queued_run) => {
                let Some(queued_run_object) = queued_run.as_object_mut() else {
                    return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                        message: "message metadata.queuedRun must be an object".to_string(),
                    });
                };
                queued_run_object
                    .insert("status".to_string(), Value::String("running".to_string()));
                queued_run_object.insert(
                    "assistantMessageId".to_string(),
                    Value::String(assistant_message_id.to_string()),
                );
                queued_run_object.insert(
                    "assistantSequence".to_string(),
                    Value::Number(assistant_sequence.into()),
                );
            }
            None => {
                message_metadata.insert(
                    QUEUED_MESSAGE_METADATA_KEY.to_string(),
                    json!({
                        "status": "running",
                        "assistantMessageId": assistant_message_id,
                        "assistantSequence": assistant_sequence,
                    }),
                );
            }
        }
        let message_metadata_json = serde_json::to_string(&message_metadata).map_err(|source| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("user message metadata is invalid JSON: {source}"),
            }
        })?;

        transaction
            .execute(
                "UPDATE chats SET metadata_json = ?1 WHERE id = ?2",
                params![chat_metadata_json, chat_id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let updated_messages = transaction
            .execute(
                "UPDATE messages SET metadata_json = ?1 WHERE id = ?2 AND chat_id = ?3",
                params![message_metadata_json, user_message_id, chat_id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if updated_messages == 0 {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!(
                    "message '{user_message_id}' was not found in chat '{chat_id}' while marking queued run started"
                ),
            });
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(())
    }

    /// Atomic pre-stream coordinator failure: fail running task/attempt, write
    /// durable assistant Error bubble, clear matching queuedRun, and append
    /// task_failed event. Idempotent when run identity no longer matches.
    pub fn close_pre_stream_chat_failure(
        &mut self,
        closure: PreStreamChatFailureClosure<'_>,
    ) -> Result<PreStreamChatFailureClosureResult, WorkspaceDatabaseError> {
        validate_agent_json(closure.error_json, "error_json")?;
        validate_json_metadata(
            closure.assistant_metadata_json,
            "assistant message metadata",
        )?;

        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;

        let task = transaction
            .query_row(
                "SELECT id, team_id, owner_instance_id, status
                 FROM agent_tasks
                 WHERE id = ?1",
                params![closure.task_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?;
        let Some((_task_id, team_id, owner_instance_id, task_status)) = task else {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(PreStreamChatFailureClosureResult::Skipped {
                reason: "task not found".to_string(),
            });
        };
        if task_status != AgentTaskStatus::Running.as_str() {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(PreStreamChatFailureClosureResult::Skipped {
                reason: format!("task status is '{task_status}'"),
            });
        }

        let attempt_matches = transaction
            .query_row(
                "SELECT 1 FROM agent_attempts
                 WHERE id = ?1 AND task_id = ?2 AND team_id = ?3 AND status = 'running'",
                params![
                    closure.attempt_id.as_str(),
                    closure.task_id.as_str(),
                    team_id
                ],
                |_| Ok(1_i64),
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?
            .is_some();
        if !attempt_matches {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(PreStreamChatFailureClosureResult::Skipped {
                reason: "attempt is not the active running attempt".to_string(),
            });
        }

        // Fail task + attempt while still in the same Immediate transaction.
        let now = now_timestamp();
        let task_updated = transaction
            .execute(
                "UPDATE agent_tasks
                 SET status = ?4,
                     error_json = ?5,
                     completed_at = ?6,
                     updated_at = ?6
                 WHERE id = ?1 AND team_id = ?2 AND status = ?3",
                params![
                    closure.task_id.as_str(),
                    team_id,
                    AgentTaskStatus::Running.as_str(),
                    AgentTaskStatus::Failed.as_str(),
                    closure.error_json,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if task_updated != 1 {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(PreStreamChatFailureClosureResult::Skipped {
                reason: "task was no longer running".to_string(),
            });
        }
        let attempt_updated = transaction
            .execute(
                "UPDATE agent_attempts
                 SET status = 'failed', completed_at = ?3
                 WHERE id = ?1 AND task_id = ?2 AND status = 'running'",
                params![closure.attempt_id.as_str(), closure.task_id.as_str(), now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if attempt_updated != 1 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "task '{}' has no active attempt for pre-stream failure closure",
                    closure.task_id
                ),
            });
        }
        transaction
            .execute(
                "UPDATE agent_instances
                 SET status = CASE
                         WHEN status = 'draining' THEN 'draining'
                         ELSE 'idle'
                     END,
                     updated_at = ?3
                 WHERE id = ?1 AND team_id = ?2",
                params![owner_instance_id, team_id, now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        // Materialize durable assistant error only when identity still matches.
        if closure.materialize_assistant {
            let existing_assistant = message_from_transaction(
                &transaction,
                &database_path,
                closure.assistant_message_id,
            )?;
            let can_write_assistant = match existing_assistant {
                Some(existing) => {
                    if existing.chat_id != closure.chat_id
                        || existing.role != "assistant"
                        || existing.sequence != closure.assistant_sequence
                    {
                        false
                    } else {
                        // Do not overwrite a completed/successful assistant body.
                        let metadata = parse_json_object(
                            &existing.metadata_json,
                            "assistant message metadata",
                        )?;
                        let streaming_state = metadata
                            .get("streamingState")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        existing.content.trim().is_empty()
                            || streaming_state == "streaming"
                            || streaming_state == "failed"
                            || metadata.get("runFailure").is_some()
                    }
                }
                None => true,
            };

            if can_write_assistant {
                let changed = transaction
                    .execute(
                        "INSERT INTO messages
                            (id, chat_id, role, content, sequence, created_at, metadata_json)
                         VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6)
                         ON CONFLICT(id) DO UPDATE SET
                            content = excluded.content,
                            metadata_json = excluded.metadata_json
                         WHERE messages.chat_id = excluded.chat_id
                            AND messages.role = excluded.role
                            AND messages.sequence = excluded.sequence",
                        params![
                            closure.assistant_message_id,
                            closure.chat_id,
                            closure.assistant_content,
                            closure.assistant_sequence,
                            now,
                            closure.assistant_metadata_json
                        ],
                    )
                    .map_err(|source| sqlite_error(&database_path, source))?;
                if changed == 0 {
                    // Race: assistant already finalized by a newer identity.
                }
            }

            // Clear queuedRun only when it still matches this run identity.
            if let Some(chat) =
                chat_from_transaction(&transaction, &database_path, closure.chat_id)?
            {
                let mut chat_metadata = parse_json_object(&chat.metadata_json, "chat metadata")?;
                let should_clear_chat = chat_metadata
                    .get(QUEUED_CHAT_METADATA_KEY)
                    .and_then(Value::as_object)
                    .is_some_and(|queued_run| {
                        let user_ok = queued_run
                            .get("userMessageId")
                            .or_else(|| queued_run.get("user_message_id"))
                            .and_then(Value::as_str)
                            == Some(closure.user_message_id);
                        let assistant_ok = queued_run
                            .get("assistantMessageId")
                            .or_else(|| queued_run.get("assistant_message_id"))
                            .and_then(Value::as_str)
                            .map(|id| id == closure.assistant_message_id)
                            .unwrap_or(true);
                        let sequence_ok = queued_run
                            .get("assistantSequence")
                            .or_else(|| queued_run.get("assistant_sequence"))
                            .and_then(Value::as_i64)
                            .map(|seq| seq == closure.assistant_sequence)
                            .unwrap_or(true);
                        user_ok && assistant_ok && sequence_ok
                    });
                if should_clear_chat {
                    chat_metadata.remove(QUEUED_CHAT_METADATA_KEY);
                    let chat_metadata_json =
                        serde_json::to_string(&chat_metadata).map_err(|source| {
                            WorkspaceDatabaseError::InvalidMessageMetadata {
                                message: format!("chat metadata is invalid JSON: {source}"),
                            }
                        })?;
                    transaction
                        .execute(
                            "UPDATE chats SET metadata_json = ?1, updated_at = ?2 WHERE id = ?3",
                            params![chat_metadata_json, now, closure.chat_id],
                        )
                        .map_err(|source| sqlite_error(&database_path, source))?;
                } else {
                    transaction
                        .execute(
                            "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
                            params![now, closure.chat_id],
                        )
                        .map_err(|source| sqlite_error(&database_path, source))?;
                }
            }

            if let Some(message) =
                message_from_transaction(&transaction, &database_path, closure.user_message_id)?
            {
                if message.chat_id == closure.chat_id {
                    let mut message_metadata =
                        parse_json_object(&message.metadata_json, "user message metadata")?;
                    let should_clear_message = message_metadata
                        .get(QUEUED_MESSAGE_METADATA_KEY)
                        .and_then(Value::as_object)
                        .is_some_and(|queued_run| {
                            let assistant_ok = queued_run
                                .get("assistantMessageId")
                                .or_else(|| queued_run.get("assistant_message_id"))
                                .and_then(Value::as_str)
                                .map(|id| id == closure.assistant_message_id)
                                .unwrap_or(true);
                            let sequence_ok = queued_run
                                .get("assistantSequence")
                                .or_else(|| queued_run.get("assistant_sequence"))
                                .and_then(Value::as_i64)
                                .map(|seq| seq == closure.assistant_sequence)
                                .unwrap_or(true);
                            assistant_ok && sequence_ok
                        });
                    if should_clear_message
                        && message_metadata
                            .remove(QUEUED_MESSAGE_METADATA_KEY)
                            .is_some()
                    {
                        let message_metadata_json = serde_json::to_string(&message_metadata)
                            .map_err(|source| WorkspaceDatabaseError::InvalidMessageMetadata {
                                message: format!("user message metadata is invalid JSON: {source}"),
                            })?;
                        transaction
                            .execute(
                                "UPDATE messages SET metadata_json = ?1 WHERE id = ?2 AND chat_id = ?3",
                                params![
                                    message_metadata_json,
                                    closure.user_message_id,
                                    closure.chat_id
                                ],
                            )
                            .map_err(|source| sqlite_error(&database_path, source))?;
                    }
                }
            }
        }

        // task_failed event (same payload shape as fail_claimed_task).
        let payload_value = json!({
            "outcome": serde_json::from_str::<Value>(closure.error_json)
                .unwrap_or_else(|_| json!({ "message": closure.error_json })),
            "recoveryReason": "pre_stream_failure_closure",
        });
        let payload_json = redact_agent_json(&payload_value.to_string(), "payload_json")?;
        let event_sequence: i64 = transaction
            .query_row(
                "SELECT next_event_sequence FROM agent_teams WHERE id = ?1",
                params![team_id],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "UPDATE agent_teams
                 SET next_event_sequence = next_event_sequence + 1, updated_at = ?2
                 WHERE id = ?1",
                params![team_id, now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "INSERT INTO agent_events
                    (team_id, sequence, event_type, instance_id, task_id, attempt_id,
                     message_id, payload_json, created_at)
                 VALUES (?1, ?2, 'task_failed', ?3, ?4, ?5, NULL, ?6, ?7)",
                params![
                    team_id,
                    event_sequence,
                    owner_instance_id,
                    closure.task_id.as_str(),
                    closure.attempt_id.as_str(),
                    payload_json,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        // Plan phase / scheduled-task sync after commit (own transactions).
        let phase_error_message = serde_json::from_str::<Value>(closure.error_json)
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "pre-stream failure".to_string());
        let _ = self.fail_plan_phase_run(closure.task_id, &phase_error_message)?;

        Ok(PreStreamChatFailureClosureResult::Applied)
    }

    /// Bounded, idempotent healing for coordinator tasks that failed before any
    /// assistant message was written (legacy concurrency swallow or new
    /// `stage=pre_stream_prepare` / pre_active_run metadata).
    ///
    /// Scoped to a single chat via `agent_teams.chat_id`. Does not re-run the
    /// task, create attempts, or touch Memory/Spec/provider/tools.
    pub fn materialize_missing_pre_stream_failure_messages(
        &mut self,
        chat_id: &str,
    ) -> Result<Vec<PreStreamFailureMaterialization>, WorkspaceDatabaseError> {
        const PRE_STREAM_USER_MESSAGE_DATABASE_BUSY: &str =
            "Reply has not started: workspace database is busy. Please retry.";
        const PRE_STREAM_USER_MESSAGE_GENERIC: &str =
            "Reply has not started: preparation failed. Please retry.";
        const STORED_CHAT_PARTS_VERSION: i64 = 5;

        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;

        // Per-chat coordinator failed tasks with visible assistant identity.
        let mut candidates = transaction
            .prepare(
                "SELECT task.id,
                        task.error_json,
                        task.input_json,
                        task.completed_at,
                        task.updated_at
                 FROM agent_tasks AS task
                 INNER JOIN agent_teams AS team ON team.id = task.team_id
                 WHERE team.chat_id = ?1
                   AND task.status = 'failed'
                   AND task.owner_instance_id = team.coordinator_instance_id
                   AND COALESCE(
                         json_extract(task.input_json, '$.visibleAssistantMessageId'),
                         json_extract(task.input_json, '$.visible_assistant_message_id')
                       ) IS NOT NULL
                   AND COALESCE(
                         json_extract(task.input_json, '$.visibleAssistantMessageId'),
                         json_extract(task.input_json, '$.visible_assistant_message_id')
                       ) <> ''
                   AND COALESCE(
                         json_extract(task.input_json, '$.queuedUserMessageId'),
                         json_extract(task.input_json, '$.queued_user_message_id')
                       ) IS NOT NULL
                   AND COALESCE(
                         json_extract(task.input_json, '$.visibleAssistantSequence'),
                         json_extract(task.input_json, '$.visible_assistant_sequence')
                       ) IS NOT NULL
                 ORDER BY task.created_at ASC, task.id ASC",
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        let candidate_rows = candidates
            .query_map(params![chat_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|source| sqlite_error(&database_path, source))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| sqlite_error(&database_path, source))?;
        drop(candidates);

        let mut materialized = Vec::new();

        for (task_id, error_json, input_json, completed_at, updated_at) in candidate_rows {
            let input = match serde_json::from_str::<Value>(&input_json) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let Some(user_message_id) = input
                .get("queuedUserMessageId")
                .or_else(|| input.get("queued_user_message_id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let Some(assistant_message_id) = input
                .get("visibleAssistantMessageId")
                .or_else(|| input.get("visible_assistant_message_id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let Some(assistant_sequence) = input
                .get("visibleAssistantSequence")
                .or_else(|| input.get("visible_assistant_sequence"))
                .and_then(Value::as_i64)
            else {
                continue;
            };
            if assistant_sequence < 0 {
                continue;
            }

            // Skip when assistant already exists (idempotent + negative matrix).
            let assistant_exists = transaction
                .query_row(
                    "SELECT 1 FROM messages WHERE id = ?1",
                    params![assistant_message_id],
                    |_| Ok(1_i64),
                )
                .optional()
                .map_err(|source| sqlite_error(&database_path, source))?
                .is_some();
            if assistant_exists {
                continue;
            }

            // User must still exist in this chat (edit/truncate safety).
            let user_message =
                message_from_transaction(&transaction, &database_path, user_message_id)?;
            let Some(user_message) = user_message else {
                continue;
            };
            if user_message.chat_id != chat_id || user_message.role != "user" {
                continue;
            }

            // Sequence must still be free for this chat.
            let sequence_taken = transaction
                .query_row(
                    "SELECT 1 FROM messages WHERE chat_id = ?1 AND sequence = ?2",
                    params![chat_id, assistant_sequence],
                    |_| Ok(1_i64),
                )
                .optional()
                .map_err(|source| sqlite_error(&database_path, source))?
                .is_some();
            if sequence_taken {
                continue;
            }

            let error_value = error_json
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                .unwrap_or(Value::Null);

            let stage = error_value
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or("");
            let code = error_value
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("");
            let diagnostic = error_value
                .get("diagnostic")
                .and_then(Value::as_str)
                .or_else(|| error_value.get("message").and_then(Value::as_str))
                .unwrap_or("");
            let error_message_text = error_value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("");

            let is_structured_pre_stream = stage == "pre_stream_prepare"
                || stage == "pre_active_run"
                || code == "workspace_database_busy";
            let is_legacy_concurrency = !is_structured_pre_stream
                && (error_message_text.contains("workspace database concurrency limit reached")
                    || diagnostic.contains("workspace database concurrency limit reached"));

            if !is_structured_pre_stream && !is_legacy_concurrency {
                continue;
            }

            // Legacy: require no durable start/provider/tool evidence for this task.
            if is_legacy_concurrency {
                let has_run_start = transaction
                    .query_row(
                        "SELECT 1 FROM run_events
                         WHERE chat_id = ?1 AND run_id = ?2 AND event_type = 'start'
                         LIMIT 1",
                        params![chat_id, task_id],
                        |_| Ok(1_i64),
                    )
                    .optional()
                    .map_err(|source| sqlite_error(&database_path, source))?
                    .is_some();
                if has_run_start {
                    continue;
                }
                let has_llm = transaction
                    .query_row(
                        "SELECT 1 FROM llm_requests
                         WHERE agent_task_id = ?1
                         LIMIT 1",
                        params![task_id],
                        |_| Ok(1_i64),
                    )
                    .optional()
                    .map_err(|source| sqlite_error(&database_path, source))?
                    .is_some();
                if has_llm {
                    continue;
                }
                let has_tool = transaction
                    .query_row(
                        "SELECT 1 FROM tool_calls
                         WHERE chat_id = ?1 AND run_id = ?2
                         LIMIT 1",
                        params![chat_id, task_id],
                        |_| Ok(1_i64),
                    )
                    .optional()
                    .map_err(|source| sqlite_error(&database_path, source))?
                    .is_some();
                if has_tool {
                    continue;
                }
            }

            let retryable = error_value
                .get("retryable")
                .and_then(Value::as_bool)
                .unwrap_or(is_legacy_concurrency || code == "workspace_database_busy");
            let user_visible = if is_legacy_concurrency
                || code == "workspace_database_busy"
                || error_message_text.contains("workspace database is busy")
                || error_message_text.contains("workspace database concurrency limit reached")
            {
                PRE_STREAM_USER_MESSAGE_DATABASE_BUSY
            } else if !error_message_text.trim().is_empty()
                && !error_message_text.contains("workspace database concurrency limit reached")
                && error_message_text.len() < 280
            {
                error_message_text
            } else {
                PRE_STREAM_USER_MESSAGE_GENERIC
            };
            let resolved_code = if code.is_empty() {
                if is_legacy_concurrency {
                    "workspace_database_busy"
                } else {
                    "pre_stream_error"
                }
            } else {
                code
            };
            let resolved_stage = if stage.is_empty() {
                "pre_stream_prepare"
            } else {
                stage
            };

            let created_at = completed_at
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(updated_at.as_str());

            let run_failure = json!({
                "code": resolved_code,
                "stage": resolved_stage,
                "retryable": retryable,
                "taskId": task_id,
                "message": user_visible,
                "healedFromHistoricalTask": true,
            });
            let assistant_metadata = json!({
                "streamingState": "failed",
                "runFailure": run_failure,
                "parts": [{ "type": "error", "text": user_visible }],
                "partsVersion": STORED_CHAT_PARTS_VERSION,
                "partsSource": "pre_stream_failure_historical",
            });
            let assistant_metadata_json =
                serde_json::to_string(&assistant_metadata).map_err(|source| {
                    WorkspaceDatabaseError::InvalidMessageMetadata {
                        message: format!("assistant message metadata is invalid JSON: {source}"),
                    }
                })?;
            validate_json_metadata(&assistant_metadata_json, "assistant message metadata")?;

            // PK + (chat_id, sequence) unique: concurrent GET loads insert at most once.
            let inserted = transaction
                .execute(
                    "INSERT INTO messages
                        (id, chat_id, role, content, sequence, created_at, metadata_json)
                     VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6)
                     ON CONFLICT(id) DO NOTHING",
                    params![
                        assistant_message_id,
                        chat_id,
                        user_visible,
                        assistant_sequence,
                        created_at,
                        assistant_metadata_json
                    ],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;

            if inserted == 1 {
                // Keep chat updated_at so list ordering reflects the healed bubble.
                transaction
                    .execute(
                        "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
                        params![created_at, chat_id],
                    )
                    .map_err(|source| sqlite_error(&database_path, source))?;

                let task_id_parsed = AgentTaskId::new(task_id.clone()).map_err(|source| {
                    WorkspaceDatabaseError::InvalidAgentRuntimeData {
                        message: format!("invalid agent task id '{task_id}': {source}"),
                    }
                })?;
                materialized.push(PreStreamFailureMaterialization {
                    task_id: task_id_parsed,
                    assistant_message_id: assistant_message_id.to_string(),
                    assistant_sequence,
                });
            }
        }

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(materialized)
    }

    pub fn clear_chat_queued_run(
        &mut self,
        chat_id: &str,
        user_message_id: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let chat =
            chat_from_transaction(&transaction, &database_path, chat_id)?.ok_or_else(|| {
                WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: format!("chat was not found: {chat_id}"),
                }
            })?;
        let mut chat_metadata = parse_json_object(&chat.metadata_json, "chat metadata")?;
        let should_clear_chat = chat_metadata
            .get(QUEUED_CHAT_METADATA_KEY)
            .and_then(Value::as_object)
            .and_then(|queued_run| {
                queued_run
                    .get("userMessageId")
                    .or_else(|| queued_run.get("user_message_id"))
            })
            .and_then(Value::as_str)
            == Some(user_message_id);
        if should_clear_chat {
            chat_metadata.remove(QUEUED_CHAT_METADATA_KEY);
            let chat_metadata_json = serde_json::to_string(&chat_metadata).map_err(|source| {
                WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: format!("chat metadata is invalid JSON: {source}"),
                }
            })?;
            transaction
                .execute(
                    "UPDATE chats SET metadata_json = ?1 WHERE id = ?2",
                    params![chat_metadata_json, chat_id],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }

        if let Some(message) =
            message_from_transaction(&transaction, &database_path, user_message_id)?
        {
            let mut message_metadata =
                parse_json_object(&message.metadata_json, "user message metadata")?;
            if message_metadata
                .remove(QUEUED_MESSAGE_METADATA_KEY)
                .is_some()
            {
                let message_metadata_json =
                    serde_json::to_string(&message_metadata).map_err(|source| {
                        WorkspaceDatabaseError::InvalidMessageMetadata {
                            message: format!("user message metadata is invalid JSON: {source}"),
                        }
                    })?;
                transaction
                    .execute(
                        "UPDATE messages SET metadata_json = ?1 WHERE id = ?2 AND chat_id = ?3",
                        params![message_metadata_json, user_message_id, chat_id],
                    )
                    .map_err(|source| sqlite_error(&database_path, source))?;
            }
        }

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(())
    }

    pub fn message(&self, id: &str) -> Result<Option<MessageRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, chat_id, role, content, sequence, created_at, metadata_json
                 FROM messages
                 WHERE id = ?1",
                params![id],
                |row| {
                    Ok(MessageRecord {
                        id: row.get(0)?,
                        chat_id: row.get(1)?,
                        role: row.get(2)?,
                        content: row.get(3)?,
                        sequence: row.get(4)?,
                        created_at: row.get(5)?,
                        metadata_json: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn update_message_metadata(
        &mut self,
        message_id: &str,
        metadata_json: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        validate_json_metadata(metadata_json, "message metadata")?;
        let updated = self
            .connection
            .execute(
                "UPDATE messages SET metadata_json = ?1 WHERE id = ?2",
                params![metadata_json, message_id],
            )
            .map_err(|source| self.sqlite_error(source))?;
        if updated == 0 {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("message was not found: {message_id}"),
            });
        }

        Ok(())
    }

    /// Apply a typed metadata mutation inside one Immediate transaction.
    ///
    /// Reads the current `metadata_json` under the write lock, shallow-merges only the
    /// targeted field(s), and writes back. Concurrent mutations of unrelated keys do not
    /// clobber each other. Prefer this over read → `message()` → `update_message_metadata`.
    pub fn mutate_message_metadata(
        &mut self,
        message_id: &str,
        mutation: MessageMetadataMutation,
    ) -> Result<String, WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let message = message_from_transaction(&transaction, &database_path, message_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("message was not found: {message_id}"),
            })?;
        let mut metadata = parse_json_object(&message.metadata_json, "message metadata")?;
        apply_message_metadata_mutation(&mut metadata, mutation)?;
        let metadata_json = serde_json::to_string(&metadata).map_err(|source| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("message metadata is invalid JSON: {source}"),
            }
        })?;
        let updated = transaction
            .execute(
                "UPDATE messages SET metadata_json = ?1 WHERE id = ?2",
                params![metadata_json, message_id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if updated == 0 {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("message was not found: {message_id}"),
            });
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        Ok(metadata_json)
    }

    pub fn upsert_message_content(
        &mut self,
        message: NewMessage<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let now = now_timestamp();
        let metadata_json = message.metadata_json.unwrap_or("{}");
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;

        let changed = transaction
            .execute(
                "INSERT INTO messages
                    (id, chat_id, role, content, sequence, created_at, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                    content = excluded.content,
                    metadata_json = excluded.metadata_json
                 WHERE messages.chat_id = excluded.chat_id
                    AND messages.role = excluded.role
                    AND messages.sequence = excluded.sequence",
                params![
                    message.id,
                    message.chat_id,
                    message.role,
                    message.content,
                    message.sequence,
                    now,
                    metadata_json
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        if changed == 0 {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!(
                    "message '{}' already exists with a different chat, role, or sequence",
                    message.id
                ),
            });
        }

        transaction
            .execute(
                "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
                params![now, message.chat_id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(())
    }

    pub fn messages_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Vec<MessageRecord>, WorkspaceDatabaseError> {
        let sql = format!(
            "SELECT
                messages.id,
                messages.chat_id,
                messages.role,
                messages.content,
                messages.sequence,
                messages.created_at,
                messages.metadata_json
             FROM messages AS messages
             WHERE messages.chat_id = ?1
             {VISIBLE_MESSAGE_FILTER_SQL}
             ORDER BY messages.sequence ASC"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| {
                Ok(MessageRecord {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    sequence: row.get(4)?,
                    created_at: row.get(5)?,
                    metadata_json: row.get(6)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn messages_for_chat_page(
        &self,
        chat_id: &str,
        before_sequence: Option<i64>,
        limit: usize,
    ) -> Result<Vec<MessageRecord>, WorkspaceDatabaseError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let limit = i64::try_from(limit).map_err(|source| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("message page limit overflowed: {source}"),
            }
        })?;
        let sql = if before_sequence.is_some() {
            format!(
                "SELECT id, chat_id, role, content, sequence, created_at, metadata_json
                 FROM (
                     SELECT
                        messages.id,
                        messages.chat_id,
                        messages.role,
                        messages.content,
                        messages.sequence,
                        messages.created_at,
                        messages.metadata_json
                     FROM messages AS messages
                     WHERE messages.chat_id = ?1
                       AND messages.sequence < ?2
                     {VISIBLE_MESSAGE_FILTER_SQL}
                     ORDER BY messages.sequence DESC
                     LIMIT ?3
                 )
                 ORDER BY sequence ASC"
            )
        } else {
            format!(
                "SELECT id, chat_id, role, content, sequence, created_at, metadata_json
                 FROM (
                     SELECT
                        messages.id,
                        messages.chat_id,
                        messages.role,
                        messages.content,
                        messages.sequence,
                        messages.created_at,
                        messages.metadata_json
                     FROM messages AS messages
                     WHERE messages.chat_id = ?1
                     {VISIBLE_MESSAGE_FILTER_SQL}
                     ORDER BY messages.sequence DESC
                     LIMIT ?2
                 )
                 ORDER BY sequence ASC"
            )
        };
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| self.sqlite_error(source))?;
        let map_row = |row: &Row<'_>| {
            Ok(MessageRecord {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                sequence: row.get(4)?,
                created_at: row.get(5)?,
                metadata_json: row.get(6)?,
            })
        };
        let records = if let Some(before_sequence) = before_sequence {
            let rows = statement
                .query_map(params![chat_id, before_sequence, limit], map_row)
                .map_err(|source| self.sqlite_error(source))?;
            collect_rows(rows, &self.database_path)?
        } else {
            let rows = statement
                .query_map(params![chat_id, limit], map_row)
                .map_err(|source| self.sqlite_error(source))?;
            collect_rows(rows, &self.database_path)?
        };

        Ok(records)
    }

    pub fn message_role_counts_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Vec<MessageRoleCountRecord>, WorkspaceDatabaseError> {
        let sql = format!(
            "SELECT messages.role, COUNT(*)
             FROM messages AS messages
             WHERE messages.chat_id = ?1
             {VISIBLE_MESSAGE_FILTER_SQL}
             GROUP BY messages.role"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| {
                Ok(MessageRoleCountRecord {
                    role: row.get(0)?,
                    count: row.get(1)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn next_message_sequence_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<i64, WorkspaceDatabaseError> {
        let max_sequence = self
            .connection
            .query_row(
                "SELECT MAX(sequence) FROM messages WHERE chat_id = ?1",
                params![chat_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .map_err(|source| self.sqlite_error(source))?;

        match max_sequence {
            Some(sequence) => sequence.checked_add(1).ok_or_else(|| {
                WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: format!("message sequence overflowed for chat '{chat_id}'"),
                }
            }),
            None => Ok(0),
        }
    }

    pub fn rewrite_chat_from_user_message(
        &mut self,
        rewrite: RewriteChatFromUserMessage<'_>,
    ) -> Result<RewriteChatFromUserMessageResult, WorkspaceDatabaseError> {
        validate_json_metadata(rewrite.user_metadata_json, "user message metadata")?;
        validate_json_metadata(rewrite.chat_queued_run_json, "chat queued run")?;
        validate_json_metadata(
            rewrite.assistant_metadata_json,
            "assistant message metadata",
        )?;
        if let Some(input_json) = rewrite.coordinator_task_input_json {
            validate_agent_json(input_json, "input_json")?;
        }
        if rewrite.coordinator_task_id.is_some() != rewrite.coordinator_task_input_json.is_some() {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "Coordinator task id and input must be provided together".to_string(),
            });
        }

        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let now = now_timestamp();
        let chat = transaction
            .query_row(
                "SELECT id, title, created_at, updated_at, archived_at, metadata_json
                 FROM chats WHERE id = ?1",
                params![rewrite.chat_id],
                chat_from_row,
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("chat was not found: {}", rewrite.chat_id),
            })?;
        let chat_metadata = parse_json_object(&chat.metadata_json, "chat metadata")?;
        if chat.archived_at.is_some() {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "archived chats are read-only".to_string(),
            });
        }
        if chat_metadata.get("kind").and_then(Value::as_str)
            == Some(MEMORY_DREAM_TRANSCRIPT_CHAT_KIND)
        {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "memory Dream transcript chats are read-only".to_string(),
            });
        }
        if chat_metadata
            .get(QUEUED_CHAT_METADATA_KEY)
            .is_some_and(|queued_run| !queued_run.is_null())
        {
            return Err(WorkspaceDatabaseError::ChatRewriteConflict {
                message: "chat already has a queued or running run".to_string(),
            });
        }

        let visible_message_sql = format!(
            "SELECT id, chat_id, role, content, sequence, created_at, metadata_json
             FROM messages AS messages
             WHERE messages.id = ?2 AND messages.chat_id = ?1
             {VISIBLE_MESSAGE_FILTER_SQL}"
        );
        let user_message = transaction
            .query_row(
                &visible_message_sql,
                params![rewrite.chat_id, rewrite.user_message_id],
                message_from_row,
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!(
                    "visible message '{}' was not found in chat '{}'",
                    rewrite.user_message_id, rewrite.chat_id
                ),
            })?;
        if user_message.role != "user" {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "only visible user messages can be edited".to_string(),
            });
        }
        if rewrite
            .expected_content
            .is_some_and(|expected| expected != user_message.content)
        {
            return Err(WorkspaceDatabaseError::ChatRewriteConflict {
                message: "user message content changed before it could be edited".to_string(),
            });
        }
        let assistant_sequence = user_message.sequence.checked_add(1).ok_or_else(|| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "assistant message sequence overflowed".to_string(),
            }
        })?;

        let agent_team = transaction
            .query_row(
                "SELECT id, coordinator_instance_id
                 FROM agent_teams WHERE chat_id = ?1",
                params![rewrite.chat_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?;
        if rewrite.coordinator_task_id.is_some() && agent_team.is_none() {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "chat rewrite requested a Coordinator task without an Agent team"
                    .to_string(),
            });
        }
        let active_task_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*)
                 FROM agent_tasks AS task
                 JOIN agent_teams AS team ON team.id = task.team_id
                 WHERE team.chat_id = ?1 AND task.status IN ('running', 'waiting')",
                params![rewrite.chat_id],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if active_task_count > 0 {
            return Err(WorkspaceDatabaseError::ChatRewriteConflict {
                message: format!("chat has {active_task_count} running or waiting Agent task(s)"),
            });
        }

        let mut removed_messages_statement = transaction
            .prepare(
                "SELECT id, metadata_json
                 FROM messages
                 WHERE chat_id = ?1 AND sequence > ?2
                 ORDER BY sequence ASC",
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let removed_message_rows = removed_messages_statement
            .query_map(params![rewrite.chat_id, user_message.sequence], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|source| sqlite_error(&database_path, source))?;
        let mut removed_message_ids = Vec::new();
        let mut invalidated_run_ids = HashSet::new();
        for row in removed_message_rows {
            let (message_id, metadata_json) =
                row.map_err(|source| sqlite_error(&database_path, source))?;
            if let Ok(metadata) = serde_json::from_str::<Value>(&metadata_json)
                && let Some(request_ids) = metadata
                    .get("metrics")
                    .and_then(|metrics| metrics.get("llmRequestIds"))
                    .and_then(Value::as_array)
            {
                invalidated_run_ids.extend(
                    request_ids
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string),
                );
            }
            removed_message_ids.push(message_id);
        }
        drop(removed_messages_statement);

        let mut suffix_tasks_statement = transaction
            .prepare(
                "SELECT task.id, task.input_json
                 FROM agent_tasks AS task
                 JOIN agent_teams AS team ON team.id = task.team_id
                 WHERE team.chat_id = ?1",
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let suffix_task_rows = suffix_tasks_statement
            .query_map(params![rewrite.chat_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|source| sqlite_error(&database_path, source))?;
        let removed_message_id_set = removed_message_ids.iter().collect::<HashSet<_>>();
        let mut suffix_task_ids = HashSet::new();
        for row in suffix_task_rows {
            let (task_id, input_json) =
                row.map_err(|source| sqlite_error(&database_path, source))?;
            let input = serde_json::from_str::<Value>(&input_json).map_err(|source| {
                WorkspaceDatabaseError::AgentRuntimeJson {
                    field: "agent_tasks.input_json",
                    source,
                }
            })?;
            let queued_user_message_id = input
                .get("queuedUserMessageId")
                .or_else(|| input.get("queued_user_message_id"))
                .and_then(Value::as_str);
            let visible_assistant_message_id = input
                .get("visibleAssistantMessageId")
                .or_else(|| input.get("visible_assistant_message_id"))
                .and_then(Value::as_str);
            let visible_assistant_sequence = input
                .get("visibleAssistantSequence")
                .or_else(|| input.get("visible_assistant_sequence"))
                .and_then(Value::as_i64);
            if queued_user_message_id == Some(rewrite.user_message_id)
                || queued_user_message_id
                    .is_some_and(|id| removed_message_id_set.contains(&id.to_string()))
                || visible_assistant_message_id
                    .is_some_and(|id| removed_message_id_set.contains(&id.to_string()))
                || visible_assistant_sequence.is_some_and(|sequence| sequence >= assistant_sequence)
            {
                suffix_task_ids.insert(task_id);
            }
        }
        drop(suffix_tasks_statement);
        invalidated_run_ids.extend(suffix_task_ids.iter().cloned());

        let mut llm_request_statement = transaction
            .prepare(
                "SELECT DISTINCT request.id
                 FROM llm_requests AS request
                 LEFT JOIN llm_request_events AS event
                   ON event.llm_request_id = request.id
                  AND event.event_type = 'start'
                 WHERE request.chat_id = ?1
                   AND (
                       request.id IN (SELECT value FROM json_each(?2))
                       OR request.agent_task_id IN (SELECT value FROM json_each(?2))
                       OR CAST(COALESCE(
                            json_extract(event.normalized_event_json, '$.assistantMessageId'),
                            json_extract(event.normalized_event_json, '$.assistant_message_id')
                       ) AS TEXT) IN (SELECT value FROM json_each(?3))
                   )",
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let suffix_task_ids_json = serde_json::to_string(&suffix_task_ids).map_err(|source| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("failed to serialize suffix Agent task ids: {source}"),
            }
        })?;
        let removed_message_ids_json =
            serde_json::to_string(&removed_message_ids).map_err(|source| {
                WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: format!("failed to serialize removed message ids: {source}"),
                }
            })?;
        let llm_request_rows = llm_request_statement
            .query_map(
                params![
                    rewrite.chat_id,
                    suffix_task_ids_json,
                    removed_message_ids_json
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        for row in llm_request_rows {
            invalidated_run_ids.insert(row.map_err(|source| sqlite_error(&database_path, source))?);
        }
        drop(llm_request_statement);
        let mut invalidated_run_ids = invalidated_run_ids.into_iter().collect::<Vec<_>>();
        invalidated_run_ids.sort();
        let invalidated_run_ids_json =
            serde_json::to_string(&invalidated_run_ids).map_err(|source| {
                WorkspaceDatabaseError::InvalidAuditData {
                    message: format!("failed to serialize invalidated run ids: {source}"),
                }
            })?;

        let mut invalidated_requests_statement = transaction
            .prepare(
                "SELECT id, workspace_id, provider_id, model_id, request_started_at, final_state,
                        input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                        total_latency_ms
                 FROM llm_requests
                 WHERE invalidated_at IS NULL
                   AND id IN (SELECT value FROM json_each(?1))",
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let invalidated_request_rows = invalidated_requests_statement
            .query_map(params![invalidated_run_ids_json], |row| {
                Ok(LlmRequestRecord {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    chat_id: None,
                    request_kind: String::new(),
                    agent_team_id: None,
                    agent_instance_id: None,
                    agent_task_id: None,
                    agent_attempt_id: None,
                    provider_id: row.get(2)?,
                    model_id: row.get(3)?,
                    thinking_level: None,
                    request_started_at: row.get(4)?,
                    first_token_at: None,
                    completed_at: None,
                    input_tokens: row.get(6)?,
                    output_tokens: row.get(7)?,
                    cache_read_tokens: row.get(8)?,
                    cache_write_tokens: row.get(9)?,
                    reasoning_tokens: None,
                    cache_ratio: None,
                    first_token_latency_ms: None,
                    total_latency_ms: row.get(10)?,
                    status_code: None,
                    final_state: row.get(5)?,
                    request_body_json: None,
                    response_body_json: None,
                    invalidated_at: None,
                    invalidated_reason: None,
                })
            })
            .map_err(|source| sqlite_error(&database_path, source))?;
        let invalidated_requests = collect_rows(invalidated_request_rows, &database_path)?;
        drop(invalidated_requests_statement);
        for request in &invalidated_requests {
            apply_llm_request_usage_rollup_delta(
                &transaction,
                &database_path,
                llm_request_usage_rollup_delta(llm_request_record_rollup_source(request), -1),
            )?;
        }
        transaction
            .execute(
                "UPDATE llm_requests
                 SET invalidated_at = ?2, invalidated_reason = ?3
                 WHERE invalidated_at IS NULL
                   AND id IN (SELECT value FROM json_each(?1))",
                params![invalidated_run_ids_json, now, rewrite.invalidated_reason],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        let cancelled_agent_task_ids = if let Some((team_id, _)) = agent_team.as_ref() {
            let mut queued_suffix_task_ids_statement = transaction
                .prepare(
                    "SELECT id FROM agent_tasks
                     WHERE team_id = ?1 AND status = 'queued'
                       AND id IN (SELECT value FROM json_each(?2))",
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            let queued_suffix_task_rows = queued_suffix_task_ids_statement
                .query_map(params![team_id, suffix_task_ids_json], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|source| sqlite_error(&database_path, source))?;
            let mut ids = queued_suffix_task_rows
                .map(|row| row.map_err(|source| sqlite_error(&database_path, source)))
                .collect::<Result<Vec<_>, _>>()?;
            drop(queued_suffix_task_ids_statement);
            ids.sort();
            let queued_suffix_task_ids_json = serde_json::to_string(&ids).map_err(|source| {
                WorkspaceDatabaseError::InvalidAgentRuntimeData {
                    message: format!("failed to serialize queued suffix Agent task ids: {source}"),
                }
            })?;
            let error_json = serde_json::json!({
                "message": "cancelled because chat history was rewritten",
                "reason": rewrite.invalidated_reason,
            })
            .to_string();
            let cancelled_rows = transaction
                .execute(
                    "UPDATE agent_tasks
                     SET status = 'cancelled', error_json = ?3, completed_at = ?4, updated_at = ?4
                     WHERE team_id = ?1 AND status = 'queued'
                       AND id IN (SELECT value FROM json_each(?2))",
                    params![team_id, queued_suffix_task_ids_json, error_json, now],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            if cancelled_rows != ids.len() {
                return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                    message: "queued Agent tasks changed while chat history was rewritten"
                        .to_string(),
                });
            }
            ids.into_iter()
                .map(AgentTaskId::new)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|source| WorkspaceDatabaseError::AgentDomain { source })?
        } else {
            Vec::new()
        };

        let coordinator_context_generation = if let Some((team_id, coordinator_id)) =
            agent_team.as_ref()
        {
            let active_after_cancel: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM agent_tasks
                     WHERE team_id = ?1 AND status IN ('queued', 'running', 'waiting')",
                    params![team_id],
                    |row| row.get(0),
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            if active_after_cancel > 0 {
                return Err(WorkspaceDatabaseError::ChatRewriteConflict {
                    message: format!(
                        "chat has {active_after_cancel} active Agent task(s) that are not part of the rewritten suffix"
                    ),
                });
            }
            let generation: i64 = transaction
                .query_row(
                    "SELECT context_generation FROM agent_instances
                     WHERE id = ?1 AND team_id = ?2",
                    params![coordinator_id, team_id],
                    |row| row.get(0),
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            if generation == i64::MAX {
                return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                    message: "Coordinator context generation overflowed".to_string(),
                });
            }
            transaction
                .execute(
                    "UPDATE agent_instances
                     SET context_generation = context_generation + 1,
                         status = CASE WHEN status IN ('paused', 'failed') THEN 'idle' ELSE status END,
                         updated_at = ?3
                     WHERE id = ?1 AND team_id = ?2",
                    params![coordinator_id, team_id, now],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            Some(generation + 1)
        } else {
            None
        };

        transaction
            .execute(
                "DELETE FROM tool_calls
                 WHERE chat_id = ?1
                   AND (
                       message_id IN (SELECT value FROM json_each(?2))
                       OR run_id IN (SELECT value FROM json_each(?3))
                   )",
                params![
                    rewrite.chat_id,
                    removed_message_ids_json,
                    invalidated_run_ids_json
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "DELETE FROM run_events
                 WHERE chat_id = ?1
                   AND run_id IN (SELECT value FROM json_each(?2))",
                params![rewrite.chat_id, invalidated_run_ids_json],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "DELETE FROM prompt_context_injections
                 WHERE chat_id = ?1 AND kind = 'turn_memory' AND sequence >= ?2",
                params![rewrite.chat_id, user_message.sequence],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "DELETE FROM context_compression_snapshots
                 WHERE chat_id = ?1
                   AND (source_message_end_sequence >= ?2 OR sequence >= ?2)",
                params![rewrite.chat_id, user_message.sequence],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "DELETE FROM messages WHERE chat_id = ?1 AND sequence > ?2",
                params![rewrite.chat_id, user_message.sequence],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "UPDATE messages
                 SET content = ?3, metadata_json = ?4
                 WHERE id = ?1 AND chat_id = ?2 AND role = 'user'",
                params![
                    rewrite.user_message_id,
                    rewrite.chat_id,
                    rewrite.content,
                    rewrite.user_metadata_json
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "INSERT INTO messages
                    (id, chat_id, role, content, sequence, created_at, metadata_json)
                 VALUES (?1, ?2, 'assistant', '', ?3, ?4, ?5)",
                params![
                    rewrite.assistant_message_id,
                    rewrite.chat_id,
                    assistant_sequence,
                    now,
                    rewrite.assistant_metadata_json
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        let mut chat_metadata = chat_metadata;
        chat_metadata.insert(
            QUEUED_CHAT_METADATA_KEY.to_string(),
            serde_json::from_str(rewrite.chat_queued_run_json).map_err(|source| {
                WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: format!("chat queued run is invalid JSON: {source}"),
                }
            })?,
        );
        let chat_metadata_json = serde_json::to_string(&chat_metadata).map_err(|source| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("chat metadata is invalid JSON: {source}"),
            }
        })?;
        transaction
            .execute(
                "UPDATE chats SET metadata_json = ?2, updated_at = ?3 WHERE id = ?1",
                params![rewrite.chat_id, chat_metadata_json, now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        let mut skipped_workspace_spec_job_ids = Vec::new();
        let mut spec_jobs_statement = transaction
            .prepare(
                "SELECT id FROM workspace_spec_jobs
                 WHERE chat_id = ?1 AND status IN ('queued', 'running')
                   AND run_id IN (SELECT value FROM json_each(?2))",
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let spec_job_rows = spec_jobs_statement
            .query_map(params![rewrite.chat_id, invalidated_run_ids_json], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|source| sqlite_error(&database_path, source))?;
        for row in spec_job_rows {
            skipped_workspace_spec_job_ids
                .push(row.map_err(|source| sqlite_error(&database_path, source))?);
        }
        drop(spec_jobs_statement);
        transaction
            .execute(
                "UPDATE workspace_spec_jobs
                 SET status = 'skipped', error_message = ?3, completed_at = ?4
                 WHERE chat_id = ?1 AND status IN ('queued', 'running')
                   AND run_id IN (SELECT value FROM json_each(?2))",
                params![
                    rewrite.chat_id,
                    invalidated_run_ids_json,
                    rewrite.invalidated_reason,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        let mut skipped_memory_extraction_job_ids = Vec::new();
        let mut memory_jobs_statement = transaction
            .prepare(
                "SELECT id FROM memory_extraction_jobs
                 WHERE chat_id = ?1 AND status IN ('queued', 'running')
                   AND CAST(COALESCE(
                       json_extract(input_json, '$.runId'),
                       json_extract(input_json, '$.run_id')
                   ) AS TEXT) IN (SELECT value FROM json_each(?2))",
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let memory_job_rows = memory_jobs_statement
            .query_map(params![rewrite.chat_id, invalidated_run_ids_json], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|source| sqlite_error(&database_path, source))?;
        for row in memory_job_rows {
            skipped_memory_extraction_job_ids
                .push(row.map_err(|source| sqlite_error(&database_path, source))?);
        }
        drop(memory_jobs_statement);
        transaction
            .execute(
                "UPDATE memory_extraction_jobs
                 SET status = 'skipped', error_message = ?3, completed_at = ?4
                 WHERE chat_id = ?1 AND status IN ('queued', 'running')
                   AND CAST(COALESCE(
                       json_extract(input_json, '$.runId'),
                       json_extract(input_json, '$.run_id')
                   ) AS TEXT) IN (SELECT value FROM json_each(?2))",
                params![
                    rewrite.chat_id,
                    invalidated_run_ids_json,
                    rewrite.memory_invalidation_reason,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        let agent_task_id = match (
            agent_team.as_ref(),
            rewrite.coordinator_task_id,
            rewrite.coordinator_task_input_json,
        ) {
            (Some((team_id, coordinator_id)), Some(task_id), Some(input_json)) => {
                let task_sequence: i64 = transaction
                    .query_row(
                        "SELECT next_task_sequence FROM agent_instances
                         WHERE id = ?1 AND team_id = ?2",
                        params![coordinator_id, team_id],
                        |row| row.get(0),
                    )
                    .map_err(|source| sqlite_error(&database_path, source))?;
                transaction
                    .execute(
                        "UPDATE agent_instances
                         SET next_task_sequence = next_task_sequence + 1, updated_at = ?3
                         WHERE id = ?1 AND team_id = ?2",
                        params![coordinator_id, team_id, now],
                    )
                    .map_err(|source| sqlite_error(&database_path, source))?;
                transaction
                    .execute(
                        "INSERT INTO agent_tasks
                            (id, team_id, owner_instance_id, origin_instance_id, parent_task_id,
                             sequence, status, input_json, created_at, updated_at)
                         VALUES (?1, ?2, ?3, NULL, NULL, ?4, 'queued', ?5, ?6, ?6)",
                        params![
                            task_id.as_str(),
                            team_id,
                            coordinator_id,
                            task_sequence,
                            input_json,
                            now
                        ],
                    )
                    .map_err(|source| sqlite_error(&database_path, source))?;
                Some(task_id.clone())
            }
            (None, None, None) | (Some(_), None, None) => None,
            _ => {
                return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                    message: "invalid Coordinator task rewrite state".to_string(),
                });
            }
        };

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        let user_message = self.message(rewrite.user_message_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "rewritten user message was not found after commit".to_string(),
            }
        })?;
        let assistant_message = self.message(rewrite.assistant_message_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "new assistant message was not found after commit".to_string(),
            }
        })?;
        let agent_team_id = agent_team.and_then(|(team_id, _)| AgentTeamId::new(team_id).ok());

        Ok(RewriteChatFromUserMessageResult {
            user_message,
            assistant_message,
            removed_message_ids,
            invalidated_run_ids,
            cancelled_agent_task_ids,
            agent_team_id,
            agent_task_id,
            coordinator_context_generation,
            skipped_workspace_spec_job_ids,
            skipped_memory_extraction_job_ids,
        })
    }

    pub fn insert_run_event(
        &mut self,
        event: NewRunEvent<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let now = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO run_events
                    (id, chat_id, run_id, sequence, event_type, payload_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    event.id,
                    event.chat_id,
                    event.run_id,
                    event.sequence,
                    event.event_type,
                    event.payload_json,
                    now
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn run_events_for_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<RunEventRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, chat_id, run_id, sequence, event_type, payload_json, created_at
                 FROM run_events
                 WHERE run_id = ?1
                 ORDER BY sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![run_id], |row| {
                Ok(RunEventRecord {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    run_id: row.get(2)?,
                    sequence: row.get(3)?,
                    event_type: row.get(4)?,
                    payload_json: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn run_events_for_run_after(
        &self,
        run_id: &str,
        after_sequence: i64,
        limit: usize,
    ) -> Result<Vec<RunEventRecord>, WorkspaceDatabaseError> {
        let limit =
            i64::try_from(limit).map_err(|_| WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "run event query limit is too large".to_string(),
            })?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, chat_id, run_id, sequence, event_type, payload_json, created_at
                 FROM run_events
                 WHERE run_id = ?1 AND sequence > ?2
                 ORDER BY sequence ASC
                 LIMIT ?3",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![run_id, after_sequence, limit], |row| {
                Ok(RunEventRecord {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    run_id: row.get(2)?,
                    sequence: row.get(3)?,
                    event_type: row.get(4)?,
                    payload_json: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn next_run_event_sequence(&self, run_id: &str) -> Result<i64, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT COALESCE(MAX(sequence), -1) + 1
                 FROM run_events
                 WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn runtime_tool_state_compression_count_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<i64, WorkspaceDatabaseError> {
        let event_count = self
            .connection
            .query_row(
                "SELECT COUNT(*)
                 FROM run_events
                 WHERE chat_id = ?1
                   AND event_type = 'context_compression'
                   AND CAST(json_extract(payload_json, '$.kind') AS TEXT)
                       IN ('runtimeToolState', 'runtime_tool_state')",
                params![chat_id],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))?;
        if event_count > 0 {
            return Ok(event_count);
        }

        const RUNTIME_TOOL_STATE_MARKER: &str = "Runtime tool-state compression snapshot";
        self.connection
            .query_row(
                "SELECT COALESCE(SUM(snapshot_count), 0)
                 FROM (
                     SELECT MAX(
                         (
                             LENGTH(request_body_json)
                             - LENGTH(REPLACE(request_body_json, ?2, ''))
                         ) / ?3
                     ) AS snapshot_count
                     FROM llm_requests
                     WHERE chat_id = ?1
                       AND agent_task_id IS NOT NULL
                       AND request_body_json LIKE '%' || ?2 || '%'
                     GROUP BY agent_task_id
                 )",
                params![
                    chat_id,
                    RUNTIME_TOOL_STATE_MARKER,
                    RUNTIME_TOOL_STATE_MARKER.len() as i64
                ],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn run_ids_for_chat(&self, chat_id: &str) -> Result<Vec<String>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT DISTINCT run_id
                 FROM (
                     SELECT run_id FROM run_events WHERE chat_id = ?1
                     UNION
                     SELECT run_id FROM tool_calls WHERE chat_id = ?1
                 )
                 WHERE TRIM(run_id) <> ''
                 ORDER BY run_id ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| row.get(0))
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn history_run_events_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Vec<RunEventRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, chat_id, run_id, sequence, event_type, payload_json, created_at
                 FROM run_events
                 WHERE chat_id = ?1
                   AND event_type IN
                       ('reasoning_delta', 'text_delta', 'tool_call', 'stream_attempt_start', 'stream_reset', 'context_compression')
                 ORDER BY created_at ASC, run_id ASC, sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| {
                Ok(RunEventRecord {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    run_id: row.get(2)?,
                    sequence: row.get(3)?,
                    event_type: row.get(4)?,
                    payload_json: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn history_run_events_for_chat_messages(
        &self,
        chat_id: &str,
        message_ids: &[String],
    ) -> Result<Vec<RunEventRecord>, WorkspaceDatabaseError> {
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = (2..=message_ids.len() + 1)
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, chat_id, run_id, sequence, event_type, payload_json, created_at
             FROM run_events
             WHERE chat_id = ?1
               AND event_type IN
                   ('reasoning_delta', 'text_delta', 'tool_call', 'stream_attempt_start', 'stream_reset', 'context_compression', 'guidance_applied')
               AND (
                   CAST(
                       COALESCE(
                           json_extract(payload_json, '$.assistantMessageId'),
                           json_extract(payload_json, '$.assistant_message_id')
                       ) AS TEXT
                   ) IN ({placeholders})
                   OR CAST(
                       COALESCE(
                           json_extract(payload_json, '$.interruptedAssistantId'),
                           json_extract(payload_json, '$.interrupted_assistant_id')
                       ) AS TEXT
                   ) IN ({placeholders})
               )
             ORDER BY created_at ASC, run_id ASC, sequence ASC",
        );
        let mut parameters = Vec::with_capacity(message_ids.len() + 1);
        parameters.push(SqlValue::Text(chat_id.to_string()));
        parameters.extend(message_ids.iter().cloned().map(SqlValue::Text));
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(parameters), |row| {
                Ok(RunEventRecord {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    run_id: row.get(2)?,
                    sequence: row.get(3)?,
                    event_type: row.get(4)?,
                    payload_json: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn insert_tool_call(
        &mut self,
        tool_call: NewToolCall<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let input_json = redact_audit_json(tool_call.input_json, "tool_call.input_json")?;

        self.connection
            .execute(
                "INSERT INTO tool_calls
                    (
                        id, chat_id, run_id, message_id, tool_name,
                        input_json, status, started_at, completed_at
                    )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    tool_call.id,
                    tool_call.chat_id,
                    tool_call.run_id,
                    tool_call.message_id,
                    tool_call.tool_name,
                    input_json,
                    tool_call.status,
                    tool_call.started_at,
                    tool_call.completed_at
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn upsert_tool_call(
        &mut self,
        tool_call: NewToolCall<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let input_json = redact_audit_json(tool_call.input_json, "tool_call.input_json")?;
        let changed = self
            .connection
            .execute(
                "INSERT INTO tool_calls
                    (
                        id, chat_id, run_id, message_id, tool_name,
                        input_json, status, started_at, completed_at
                    )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                    chat_id = excluded.chat_id,
                    run_id = excluded.run_id,
                    message_id = excluded.message_id,
                    tool_name = excluded.tool_name,
                    input_json = excluded.input_json,
                    status = excluded.status,
                    started_at = excluded.started_at,
                    completed_at = excluded.completed_at
                 WHERE NOT EXISTS (
                    SELECT 1 FROM tool_results
                    WHERE tool_results.tool_call_id = tool_calls.id
                 )
                    OR (
                        tool_calls.chat_id = excluded.chat_id
                        AND tool_calls.run_id = excluded.run_id
                        AND tool_calls.tool_name = excluded.tool_name
                        AND tool_calls.input_json = excluded.input_json
                    )",
                params![
                    tool_call.id,
                    tool_call.chat_id,
                    tool_call.run_id,
                    tool_call.message_id,
                    tool_call.tool_name,
                    input_json,
                    tool_call.status,
                    tool_call.started_at,
                    tool_call.completed_at
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        if changed == 0 {
            return Err(WorkspaceDatabaseError::InvalidToolCall {
                message: format!(
                    "tool call '{}' already exists with a completed tool result and a different chat, run, name, or input",
                    tool_call.id
                ),
            });
        }

        Ok(())
    }

    pub fn insert_tool_result(
        &mut self,
        tool_result: NewToolResult<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let output_json = redact_audit_json(tool_result.output_json, "tool_result.output_json")?;

        self.connection
            .execute(
                "INSERT INTO tool_results
                    (id, tool_call_id, output_json, is_error, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    tool_result.id,
                    tool_result.tool_call_id,
                    output_json,
                    if tool_result.is_error { 1_i64 } else { 0_i64 },
                    tool_result.created_at
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn upsert_tool_result(
        &mut self,
        tool_result: NewToolResult<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let output_json = redact_audit_json(tool_result.output_json, "tool_result.output_json")?;
        let changed = self
            .connection
            .execute(
                "INSERT INTO tool_results
                    (id, tool_call_id, output_json, is_error, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                    output_json = excluded.output_json,
                    is_error = excluded.is_error,
                    created_at = excluded.created_at
                 WHERE tool_results.tool_call_id = excluded.tool_call_id",
                params![
                    tool_result.id,
                    tool_result.tool_call_id,
                    output_json,
                    if tool_result.is_error { 1_i64 } else { 0_i64 },
                    tool_result.created_at
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        if changed == 0 {
            return Err(WorkspaceDatabaseError::InvalidToolCall {
                message: format!(
                    "tool result '{}' already exists for a different tool call",
                    tool_result.id
                ),
            });
        }

        Ok(())
    }

    pub fn complete_tool_call(
        &mut self,
        tool_call_id: &str,
        status: &str,
        completed_at: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        let updated = self
            .connection
            .execute(
                "UPDATE tool_calls
                 SET status = ?2, completed_at = ?3
                 WHERE id = ?1",
                params![tool_call_id, status, completed_at],
            )
            .map_err(|source| self.sqlite_error(source))?;
        if updated == 0 {
            return Err(WorkspaceDatabaseError::MissingToolCall {
                id: tool_call_id.to_string(),
            });
        }

        Ok(())
    }

    pub fn complete_running_tool_calls_for_run(
        &mut self,
        run_id: &str,
        status: &str,
        completed_at: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        self.connection
            .execute(
                "UPDATE tool_calls
                 SET status = ?2, completed_at = ?3
                 WHERE run_id = ?1 AND status = 'running'",
                params![run_id, status, completed_at],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn delete_running_tool_calls_for_run(
        &mut self,
        run_id: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        self.connection
            .execute(
                "DELETE FROM tool_calls WHERE run_id = ?1 AND status = 'running'",
                params![run_id],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn delete_incomplete_tool_calls_for_run(
        &mut self,
        run_id: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        self.connection
            .execute(
                "DELETE FROM tool_calls
                 WHERE run_id = ?1
                    AND NOT EXISTS (
                        SELECT 1 FROM tool_results
                        WHERE tool_results.tool_call_id = tool_calls.id
                    )",
                params![run_id],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn tool_calls_for_message(
        &self,
        message_id: &str,
    ) -> Result<Vec<ToolCallWithResultRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    tool_calls.id,
                    tool_calls.chat_id,
                    tool_calls.run_id,
                    tool_calls.message_id,
                    tool_calls.tool_name,
                    tool_calls.input_json,
                    tool_calls.status,
                    tool_calls.started_at,
                    tool_calls.completed_at,
                    tool_results.id,
                    tool_results.output_json,
                    tool_results.is_error,
                    tool_results.created_at
                 FROM tool_calls
                 LEFT JOIN tool_results ON tool_results.tool_call_id = tool_calls.id
                 WHERE tool_calls.message_id = ?1
                 ORDER BY tool_calls.started_at ASC, tool_calls.id ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![message_id], |row| {
                Ok(ToolCallWithResultRecord {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    run_id: row.get(2)?,
                    message_id: row.get(3)?,
                    tool_name: row.get(4)?,
                    input_json: row.get(5)?,
                    status: row.get(6)?,
                    started_at: row.get(7)?,
                    completed_at: row.get(8)?,
                    result: match row.get::<_, Option<String>>(9)? {
                        Some(id) => Some(ToolResultRecord {
                            id,
                            tool_call_id: row.get(0)?,
                            output_json: row.get(10)?,
                            is_error: row.get::<_, i64>(11)? != 0,
                            created_at: row.get(12)?,
                        }),
                        None => None,
                    },
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn tool_calls_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Vec<ToolCallWithResultRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    tool_calls.id,
                    tool_calls.chat_id,
                    tool_calls.run_id,
                    tool_calls.message_id,
                    tool_calls.tool_name,
                    tool_calls.input_json,
                    tool_calls.status,
                    tool_calls.started_at,
                    tool_calls.completed_at,
                    tool_results.id,
                    tool_results.output_json,
                    tool_results.is_error,
                    tool_results.created_at
                 FROM tool_calls
                 LEFT JOIN tool_results ON tool_results.tool_call_id = tool_calls.id
                 WHERE tool_calls.chat_id = ?1
                 ORDER BY tool_calls.started_at ASC, tool_calls.id ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| {
                Ok(ToolCallWithResultRecord {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    run_id: row.get(2)?,
                    message_id: row.get(3)?,
                    tool_name: row.get(4)?,
                    input_json: row.get(5)?,
                    status: row.get(6)?,
                    started_at: row.get(7)?,
                    completed_at: row.get(8)?,
                    result: match row.get::<_, Option<String>>(9)? {
                        Some(id) => Some(ToolResultRecord {
                            id,
                            tool_call_id: row.get(0)?,
                            output_json: row.get(10)?,
                            is_error: row.get::<_, i64>(11)? != 0,
                            created_at: row.get(12)?,
                        }),
                        None => None,
                    },
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn tool_call_counts_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Vec<ToolCallCountRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT tool_name, COUNT(*)
                 FROM tool_calls
                 WHERE chat_id = ?1
                 GROUP BY tool_name
                 ORDER BY COUNT(*) DESC, tool_name ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| {
                Ok(ToolCallCountRecord {
                    tool_name: row.get(0)?,
                    call_count: row.get(1)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn insert_llm_request(
        &mut self,
        request: NewLlmRequest<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        validate_llm_request_tokens(&request)?;
        validate_llm_agent_references(&self.connection, &self.database_path, &request)?;

        let cache_ratio = calculate_cache_ratio(request.input_tokens, request.cache_read_tokens)?;
        let thinking_level = normalized_optional_text(request.thinking_level);
        let request_body_json =
            normalize_audit_detail_for_write(request.request_body_json, "request_body_json")?;
        let response_body_json =
            normalize_audit_detail_for_write(request.response_body_json, "response_body_json")?;
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;

        transaction
            .execute(
                "INSERT INTO llm_requests
                    (
                        id, workspace_id, chat_id, request_kind, agent_team_id, agent_instance_id,
                        agent_task_id, agent_attempt_id, provider_id, model_id, thinking_level,
                        request_started_at, first_token_at, completed_at, input_tokens, output_tokens,
                        cache_read_tokens, cache_write_tokens, reasoning_tokens, cache_ratio,
                        first_token_latency_ms, total_latency_ms, status_code, final_state,
                        request_body_json, response_body_json
                    )
                 VALUES
                    (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
                params![
                    request.id,
                    request.workspace_id,
                    request.chat_id,
                    request.request_kind,
                    request.agent_team_id.map(AgentTeamId::as_str),
                    request.agent_instance_id.map(AgentInstanceId::as_str),
                    request.agent_task_id.map(AgentTaskId::as_str),
                    request.agent_attempt_id.map(AgentAttemptId::as_str),
                    request.provider_id,
                    request.model_id,
                    thinking_level,
                    request.request_started_at,
                    request.first_token_at,
                    request.completed_at,
                    request.input_tokens,
                    request.output_tokens,
                    request.cache_read_tokens,
                    request.cache_write_tokens,
                    request.reasoning_tokens,
                    cache_ratio,
                    request.first_token_latency_ms,
                    request.total_latency_ms,
                    request.status_code,
                    request.final_state,
                    request_body_json,
                    response_body_json
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        apply_llm_request_usage_rollup_delta(
            &transaction,
            &database_path,
            llm_request_usage_rollup_delta(
                LlmRequestUsageRollupSource {
                    workspace_id: Some(request.workspace_id),
                    provider_id: request.provider_id,
                    model_id: request.model_id,
                    request_started_at: request.request_started_at,
                    final_state: request.final_state,
                    input_tokens: request.input_tokens,
                    output_tokens: request.output_tokens,
                    cache_read_tokens: request.cache_read_tokens,
                    cache_write_tokens: request.cache_write_tokens,
                    total_latency_ms: request.total_latency_ms,
                },
                1,
            ),
        )?;

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(())
    }

    pub fn update_llm_request_body(
        &mut self,
        id: &str,
        request_body_json: Option<&str>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let existing = select_llm_request_record(&transaction, id)
            .map_err(|source| sqlite_error(&database_path, source))?
            .ok_or_else(|| WorkspaceDatabaseError::MissingLlmRequest { id: id.to_string() })?;
        let request_body_json = merge_audit_detail_for_update(
            existing.request_body_json.as_deref(),
            request_body_json,
            "request_body_json",
        )?;
        let updated = transaction
            .execute(
                "UPDATE llm_requests SET request_body_json = ?2 WHERE id = ?1",
                params![id, request_body_json],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        if updated == 0 {
            return Err(WorkspaceDatabaseError::MissingLlmRequest { id: id.to_string() });
        }

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(())
    }

    pub fn update_llm_request_outcome(
        &mut self,
        id: &str,
        outcome: UpdateLlmRequestOutcome<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let cache_ratio = validate_llm_request_outcome(&outcome)?;
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        update_llm_request_outcome_in_transaction(
            &transaction,
            &database_path,
            id,
            &outcome,
            cache_ratio,
        )?;

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(())
    }

    pub fn update_llm_request_outcome_with_events(
        &mut self,
        id: &str,
        outcome: UpdateLlmRequestOutcome<'_>,
        events: &[NewLlmRequestEvent<'_>],
    ) -> Result<(), WorkspaceDatabaseError> {
        let cache_ratio = validate_llm_request_outcome(&outcome)?;
        let prepared_events = events
            .iter()
            .map(prepare_llm_request_event)
            .collect::<Result<Vec<_>, _>>()?;
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        update_llm_request_outcome_in_transaction(
            &transaction,
            &database_path,
            id,
            &outcome,
            cache_ratio,
        )?;
        for event in &prepared_events {
            insert_prepared_llm_request_event(&transaction, &database_path, event)?;
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))
    }

    pub fn rebuild_llm_request_usage_rollups(&mut self) -> Result<(), WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;

        transaction
            .execute("DELETE FROM llm_request_usage_rollups", [])
            .map_err(|source| sqlite_error(&database_path, source))?;
        insert_llm_request_usage_rollup_rebuild_rows(&transaction, &database_path, None)?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))
    }

    pub fn rebuild_llm_request_usage_rollups_for_workspace(
        &mut self,
        workspace_id: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let rollup_workspace_id = normalize_llm_request_rollup_dimension(
            Some(workspace_id),
            LLM_REQUEST_ROLLUP_UNKNOWN_WORKSPACE,
        );
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;

        transaction
            .execute(
                "DELETE FROM llm_request_usage_rollups WHERE workspace_id = ?1",
                params![rollup_workspace_id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        insert_llm_request_usage_rollup_rebuild_rows(
            &transaction,
            &database_path,
            Some(workspace_id),
        )?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))
    }

    pub fn llm_request(
        &self,
        id: &str,
    ) -> Result<Option<LlmRequestRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT
                    id, workspace_id, chat_id, request_kind, agent_team_id, agent_instance_id,
                    agent_task_id, agent_attempt_id, provider_id, model_id, thinking_level,
                    request_started_at, first_token_at, completed_at, input_tokens, output_tokens,
                    cache_read_tokens, cache_write_tokens, reasoning_tokens, cache_ratio,
                    first_token_latency_ms, total_latency_ms, status_code, final_state,
                    request_body_json, response_body_json, invalidated_at, invalidated_reason
                 FROM llm_requests
                 WHERE id = ?1",
                params![id],
                |row| {
                    Ok(LlmRequestRecord {
                        id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        chat_id: row.get(2)?,
                        request_kind: row.get(3)?,
                        agent_team_id: optional_agent_id_from_row(row, 4)?,
                        agent_instance_id: optional_agent_id_from_row(row, 5)?,
                        agent_task_id: optional_agent_id_from_row(row, 6)?,
                        agent_attempt_id: optional_agent_id_from_row(row, 7)?,
                        provider_id: row.get(8)?,
                        model_id: row.get(9)?,
                        thinking_level: row.get(10)?,
                        request_started_at: row.get(11)?,
                        first_token_at: row.get(12)?,
                        completed_at: row.get(13)?,
                        input_tokens: row.get(14)?,
                        output_tokens: row.get(15)?,
                        cache_read_tokens: row.get(16)?,
                        cache_write_tokens: row.get(17)?,
                        reasoning_tokens: row.get(18)?,
                        cache_ratio: row.get(19)?,
                        first_token_latency_ms: row.get(20)?,
                        total_latency_ms: row.get(21)?,
                        status_code: row.get(22)?,
                        final_state: row.get(23)?,
                        request_body_json: row.get(24)?,
                        response_body_json: row.get(25)?,
                        invalidated_at: row.get(26)?,
                        invalidated_reason: row.get(27)?,
                    })
                },
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn invalidate_llm_request(
        &mut self,
        id: &str,
        invalidated_reason: &str,
    ) -> Result<bool, WorkspaceDatabaseError> {
        if invalidated_reason.trim().is_empty() {
            return Err(WorkspaceDatabaseError::InvalidAuditData {
                message: "LLM request invalidation reason must not be empty".to_string(),
            });
        }
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let Some(request) = select_llm_request_record(&transaction, id)
            .map_err(|source| sqlite_error(&database_path, source))?
        else {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(false);
        };
        if request.invalidated_at.is_some() {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(false);
        }

        apply_llm_request_usage_rollup_delta(
            &transaction,
            &database_path,
            llm_request_usage_rollup_delta(llm_request_record_rollup_source(&request), -1),
        )?;
        transaction
            .execute(
                "UPDATE llm_requests
                 SET invalidated_at = ?2, invalidated_reason = ?3
                 WHERE id = ?1 AND invalidated_at IS NULL",
                params![id, now_timestamp(), invalidated_reason],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        Ok(true)
    }

    pub fn llm_request_metrics_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Vec<LlmRequestMetricsRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    id, provider_id, model_id, first_token_latency_ms,
                    total_latency_ms, output_tokens
                 FROM llm_requests
                 WHERE chat_id = ?1
                   AND invalidated_at IS NULL
                 ORDER BY request_started_at ASC, id ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| {
                Ok(LlmRequestMetricsRecord {
                    id: row.get(0)?,
                    provider_id: row.get(1)?,
                    model_id: row.get(2)?,
                    first_token_latency_ms: row.get(3)?,
                    total_latency_ms: row.get(4)?,
                    output_tokens: row.get(5)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn latest_completed_llm_usage_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Option<LlmRequestUsageRecord>, WorkspaceDatabaseError> {
        let mut query = String::from(
            "SELECT input_tokens, output_tokens, cache_read_tokens, cache_write_tokens
             FROM llm_requests
             WHERE chat_id = ?
               AND invalidated_at IS NULL
               AND final_state IN ('succeeded', 'completed')
               AND input_tokens IS NOT NULL
               AND output_tokens IS NOT NULL",
        );
        let mut query_params = vec![SqlValue::Text(chat_id.to_string())];
        append_llm_request_kind_exclusion_condition(
            &mut query,
            &mut query_params,
            MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS,
        );
        query.push_str(" ORDER BY request_started_at DESC, id DESC LIMIT 1");

        self.connection
            .query_row(&query, params_from_iter(query_params), |row| {
                Ok(LlmRequestUsageRecord {
                    input_tokens: row.get(0)?,
                    output_tokens: row.get(1)?,
                    cache_read_tokens: row.get(2)?,
                    cache_write_tokens: row.get(3)?,
                })
            })
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn insert_llm_request_event(
        &mut self,
        event: NewLlmRequestEvent<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let prepared = prepare_llm_request_event(&event)?;
        insert_prepared_llm_request_event(&self.connection, &self.database_path, &prepared)
    }

    pub fn llm_request_events(
        &self,
        llm_request_id: &str,
    ) -> Result<Vec<LlmRequestEventRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    id, llm_request_id, sequence, event_at, event_type,
                    raw_chunk_json, normalized_event_json
                 FROM llm_request_events
                 WHERE llm_request_id = ?1
                 ORDER BY sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![llm_request_id], |row| {
                Ok(LlmRequestEventRecord {
                    id: row.get(0)?,
                    llm_request_id: row.get(1)?,
                    sequence: row.get(2)?,
                    event_at: row.get(3)?,
                    event_type: row.get(4)?,
                    raw_chunk_json: row.get(5)?,
                    normalized_event_json: row.get(6)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn llm_request_event_next_sequence(
        &self,
        llm_request_id: &str,
    ) -> Result<usize, WorkspaceDatabaseError> {
        let next_sequence: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(sequence) + 1, 0)
                 FROM llm_request_events
                 WHERE llm_request_id = ?1",
                params![llm_request_id],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))?;

        usize::try_from(next_sequence).map_err(|_| WorkspaceDatabaseError::InvalidAuditData {
            message: format!("LLM request '{llm_request_id}' has an invalid next event sequence"),
        })
    }

    pub fn prune_llm_request_details_before(
        &mut self,
        cutoff_started_at: &str,
    ) -> Result<i64, WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;

        let deleted_events = transaction
            .execute(
                "DELETE FROM llm_request_events
                 WHERE event_type != 'start'
                   AND llm_request_id IN (
                        SELECT id FROM llm_requests WHERE request_started_at < ?1
                   )",
                params![cutoff_started_at],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let pruned_requests = transaction
            .execute(
                "UPDATE llm_requests
                 SET request_body_json = NULL,
                     response_body_json = NULL
                 WHERE request_started_at < ?1
                   AND (request_body_json IS NOT NULL OR response_body_json IS NOT NULL)",
                params![cutoff_started_at],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        i64::try_from(deleted_events.saturating_add(pruned_requests)).map_err(|_| {
            WorkspaceDatabaseError::InvalidAuditData {
                message: "pruned LLM request detail count exceeded i64".to_string(),
            }
        })
    }

    pub fn llm_request_events_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Vec<LlmRequestEventRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    llm_request_events.id,
                    llm_request_events.llm_request_id,
                    llm_request_events.sequence,
                    llm_request_events.event_at,
                    llm_request_events.event_type,
                    llm_request_events.raw_chunk_json,
                    llm_request_events.normalized_event_json
                 FROM llm_request_events
                 INNER JOIN llm_requests
                    ON llm_requests.id = llm_request_events.llm_request_id
                 WHERE llm_requests.chat_id = ?1
                   AND llm_requests.invalidated_at IS NULL
                 ORDER BY llm_requests.request_started_at ASC,
                    llm_request_events.sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| {
                Ok(LlmRequestEventRecord {
                    id: row.get(0)?,
                    llm_request_id: row.get(1)?,
                    sequence: row.get(2)?,
                    event_at: row.get(3)?,
                    event_type: row.get(4)?,
                    raw_chunk_json: row.get(5)?,
                    normalized_event_json: row.get(6)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn llm_request_start_events_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Vec<LlmRequestEventRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    llm_request_events.id,
                    llm_request_events.llm_request_id,
                    llm_request_events.sequence,
                    llm_request_events.event_at,
                    llm_request_events.event_type,
                    NULL,
                    llm_request_events.normalized_event_json
                 FROM llm_requests
                 INNER JOIN llm_request_events
                    ON llm_request_events.llm_request_id = llm_requests.id
                    AND llm_request_events.event_type = 'start'
                    AND llm_request_events.sequence = 0
                 WHERE llm_requests.chat_id = ?1
                   AND llm_requests.invalidated_at IS NULL
                 ORDER BY llm_requests.request_started_at ASC,
                    llm_request_events.sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| {
                Ok(LlmRequestEventRecord {
                    id: row.get(0)?,
                    llm_request_id: row.get(1)?,
                    sequence: row.get(2)?,
                    event_at: row.get(3)?,
                    event_type: row.get(4)?,
                    raw_chunk_json: row.get(5)?,
                    normalized_event_json: row.get(6)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn llm_request_audit_rows(
        &self,
        filters: LlmRequestAuditFilters<'_>,
    ) -> Result<Vec<LlmRequestAuditRow>, WorkspaceDatabaseError> {
        let limit = filters.limit.unwrap_or(200).max(1);
        let offset = filters.offset.unwrap_or(0).max(0);
        let mut query = String::from(
            "SELECT
                id, workspace_id, chat_id, request_kind, provider_id, model_id, thinking_level,
                request_started_at, first_token_at, completed_at, input_tokens, output_tokens,
                cache_read_tokens, cache_write_tokens, reasoning_tokens, cache_ratio,
                first_token_latency_ms, total_latency_ms, status_code, final_state,
                invalidated_at, invalidated_reason
             FROM llm_requests",
        );
        let mut query_params = Vec::new();
        append_llm_request_audit_where_clause(&mut query, &mut query_params, filters);
        query.push_str(" ORDER BY request_started_at DESC, id DESC LIMIT ? OFFSET ?");
        query_params.push(SqlValue::Integer(limit));
        query_params.push(SqlValue::Integer(offset));
        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok(LlmRequestAuditRow {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    chat_id: row.get(2)?,
                    request_kind: row.get(3)?,
                    provider_id: row.get(4)?,
                    model_id: row.get(5)?,
                    thinking_level: row.get(6)?,
                    request_started_at: row.get(7)?,
                    first_token_at: row.get(8)?,
                    completed_at: row.get(9)?,
                    input_tokens: row.get(10)?,
                    output_tokens: row.get(11)?,
                    cache_read_tokens: row.get(12)?,
                    cache_write_tokens: row.get(13)?,
                    reasoning_tokens: row.get(14)?,
                    cache_ratio: row.get(15)?,
                    first_token_latency_ms: row.get(16)?,
                    total_latency_ms: row.get(17)?,
                    status_code: row.get(18)?,
                    final_state: row.get(19)?,
                    invalidated_at: row.get(20)?,
                    invalidated_reason: row.get(21)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn llm_request_audit_count(
        &self,
        filters: LlmRequestAuditFilters<'_>,
    ) -> Result<i64, WorkspaceDatabaseError> {
        let mut query = String::from("SELECT COUNT(*) FROM llm_requests");
        let mut query_params = Vec::new();
        append_llm_request_audit_where_clause(&mut query, &mut query_params, filters);

        self.connection
            .query_row(&query, params_from_iter(query_params), |row| row.get(0))
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn llm_request_audit_summary(
        &self,
        filters: LlmRequestAuditFilters<'_>,
    ) -> Result<LlmRequestAuditSummaryRow, WorkspaceDatabaseError> {
        let mut query = String::from(
            "SELECT
                COUNT(*),
                COUNT(CASE WHEN final_state NOT IN ('succeeded', 'completed') THEN 1 END),
                COALESCE(SUM(COALESCE(input_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(output_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(cache_write_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)), 0),
                COUNT(total_latency_ms),
                COALESCE(SUM(COALESCE(total_latency_ms, 0)), 0)
             FROM llm_requests",
        );
        let mut query_params = Vec::new();
        append_llm_request_audit_where_clause(&mut query, &mut query_params, filters);

        self.connection
            .query_row(&query, params_from_iter(query_params), |row| {
                Ok(LlmRequestAuditSummaryRow {
                    total_requests: row.get(0)?,
                    failed_requests: row.get(1)?,
                    total_input_tokens: row.get(2)?,
                    total_output_tokens: row.get(3)?,
                    total_cache_read_tokens: row.get(4)?,
                    total_cache_write_tokens: row.get(5)?,
                    total_tokens: row.get(6)?,
                    latency_count: row.get(7)?,
                    latency_sum: row.get(8)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn llm_request_audit_trend_breakdown(
        &self,
        filters: LlmRequestAuditFilters<'_>,
    ) -> Result<Vec<LlmRequestAuditTrendPoint>, WorkspaceDatabaseError> {
        let mut query = String::from(
            "SELECT
                SUBSTR(request_started_at, 1, 10) AS bucket,
                COUNT(*),
                SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0))
             FROM llm_requests",
        );
        let mut query_params = Vec::new();
        append_llm_request_audit_where_clause(&mut query, &mut query_params, filters);
        query.push_str(" GROUP BY bucket ORDER BY bucket DESC");
        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok(LlmRequestAuditTrendPoint {
                    bucket: row.get(0)?,
                    request_count: row.get(1)?,
                    total_tokens: row.get(2)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn llm_request_audit_model_breakdown(
        &self,
        filters: LlmRequestAuditFilters<'_>,
    ) -> Result<Vec<LlmRequestAuditModelBreakdown>, WorkspaceDatabaseError> {
        let mut query = String::from(
            "SELECT
                model_id,
                COUNT(*),
                SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0))
             FROM llm_requests",
        );
        let mut query_params = Vec::new();
        append_llm_request_audit_where_clause(&mut query, &mut query_params, filters);
        query.push_str(" GROUP BY model_id ORDER BY model_id");
        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok(LlmRequestAuditModelBreakdown {
                    model_id: row.get(0)?,
                    request_count: row.get(1)?,
                    total_tokens: row.get(2)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn llm_request_audit_provider_breakdown(
        &self,
        filters: LlmRequestAuditFilters<'_>,
    ) -> Result<Vec<LlmRequestAuditProviderBreakdown>, WorkspaceDatabaseError> {
        let mut query = String::from(
            "SELECT
                provider_id,
                COUNT(*),
                COUNT(CASE WHEN final_state IN ('succeeded', 'completed') THEN 1 END),
                SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)),
                COUNT(total_latency_ms),
                SUM(COALESCE(total_latency_ms, 0))
             FROM llm_requests",
        );
        let mut query_params = Vec::new();
        append_llm_request_audit_where_clause(&mut query, &mut query_params, filters);
        query.push_str(" GROUP BY provider_id ORDER BY provider_id");
        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok(LlmRequestAuditProviderBreakdown {
                    provider_id: row.get(0)?,
                    request_count: row.get(1)?,
                    success_count: row.get(2)?,
                    total_tokens: row.get(3)?,
                    latency_count: row.get(4)?,
                    latency_sum: row.get(5)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn llm_request_audit_request_kind_breakdown(
        &self,
        filters: LlmRequestAuditFilters<'_>,
    ) -> Result<Vec<LlmRequestAuditRequestKindBreakdown>, WorkspaceDatabaseError> {
        let mut query = String::from(
            "SELECT
                request_kind,
                COUNT(*),
                COUNT(CASE WHEN final_state NOT IN ('succeeded', 'completed') THEN 1 END),
                COALESCE(SUM(COALESCE(input_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(output_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(cache_write_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(reasoning_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)), 0),
                COUNT(total_latency_ms),
                COALESCE(SUM(COALESCE(total_latency_ms, 0)), 0)
             FROM llm_requests",
        );
        let mut query_params = Vec::new();
        append_llm_request_audit_where_clause(&mut query, &mut query_params, filters);
        query.push_str(" GROUP BY request_kind ORDER BY request_kind");
        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok(LlmRequestAuditRequestKindBreakdown {
                    request_kind: row.get(0)?,
                    request_count: row.get(1)?,
                    failed_requests: row.get(2)?,
                    total_input_tokens: row.get(3)?,
                    total_output_tokens: row.get(4)?,
                    total_cache_read_tokens: row.get(5)?,
                    total_cache_write_tokens: row.get(6)?,
                    total_reasoning_tokens: row.get(7)?,
                    total_tokens: row.get(8)?,
                    latency_count: row.get(9)?,
                    latency_sum: row.get(10)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn llm_request_usage_rollup_summary(
        &self,
        filters: LlmRequestUsageRollupFilters<'_>,
    ) -> Result<LlmRequestAuditSummaryRow, WorkspaceDatabaseError> {
        let mut query = String::from(
            "SELECT
                COALESCE(SUM(request_count), 0),
                COALESCE(SUM(failed_count), 0),
                COALESCE(SUM(total_input_tokens), 0),
                COALESCE(SUM(total_output_tokens), 0),
                COALESCE(SUM(total_cache_read_tokens), 0),
                COALESCE(SUM(total_cache_write_tokens), 0),
                COALESCE(SUM(total_tokens), 0),
                COALESCE(SUM(latency_count), 0),
                COALESCE(SUM(latency_sum), 0)
             FROM llm_request_usage_rollups",
        );
        let mut query_params = Vec::new();
        append_llm_request_usage_rollup_where_clause(&mut query, &mut query_params, filters);

        self.connection
            .query_row(&query, params_from_iter(query_params), |row| {
                Ok(LlmRequestAuditSummaryRow {
                    total_requests: row.get(0)?,
                    failed_requests: row.get(1)?,
                    total_input_tokens: row.get(2)?,
                    total_output_tokens: row.get(3)?,
                    total_cache_read_tokens: row.get(4)?,
                    total_cache_write_tokens: row.get(5)?,
                    total_tokens: row.get(6)?,
                    latency_count: row.get(7)?,
                    latency_sum: row.get(8)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn llm_request_usage_rollup_trend_breakdown(
        &self,
        filters: LlmRequestUsageRollupFilters<'_>,
    ) -> Result<Vec<LlmRequestAuditTrendPoint>, WorkspaceDatabaseError> {
        let mut query = String::from(
            "SELECT
                bucket_date,
                COALESCE(SUM(request_count), 0),
                COALESCE(SUM(total_tokens), 0)
             FROM llm_request_usage_rollups",
        );
        let mut query_params = Vec::new();
        append_llm_request_usage_rollup_where_clause(&mut query, &mut query_params, filters);
        query.push_str(" GROUP BY bucket_date ORDER BY bucket_date DESC");
        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok(LlmRequestAuditTrendPoint {
                    bucket: row.get(0)?,
                    request_count: row.get(1)?,
                    total_tokens: row.get(2)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn llm_request_usage_rollup_model_breakdown(
        &self,
        filters: LlmRequestUsageRollupFilters<'_>,
    ) -> Result<Vec<LlmRequestAuditModelBreakdown>, WorkspaceDatabaseError> {
        let mut query = String::from(
            "SELECT
                model_id,
                COALESCE(SUM(request_count), 0),
                COALESCE(SUM(total_tokens), 0)
             FROM llm_request_usage_rollups",
        );
        let mut query_params = Vec::new();
        append_llm_request_usage_rollup_where_clause(&mut query, &mut query_params, filters);
        query.push_str(" GROUP BY model_id ORDER BY model_id");
        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok(LlmRequestAuditModelBreakdown {
                    model_id: row.get(0)?,
                    request_count: row.get(1)?,
                    total_tokens: row.get(2)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn llm_request_usage_rollup_provider_breakdown(
        &self,
        filters: LlmRequestUsageRollupFilters<'_>,
    ) -> Result<Vec<LlmRequestAuditProviderBreakdown>, WorkspaceDatabaseError> {
        let mut query = String::from(
            "SELECT
                provider_id,
                COALESCE(SUM(request_count), 0),
                COALESCE(SUM(success_count), 0),
                COALESCE(SUM(total_tokens), 0),
                COALESCE(SUM(latency_count), 0),
                COALESCE(SUM(latency_sum), 0)
             FROM llm_request_usage_rollups",
        );
        let mut query_params = Vec::new();
        append_llm_request_usage_rollup_where_clause(&mut query, &mut query_params, filters);
        query.push_str(" GROUP BY provider_id ORDER BY provider_id");
        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok(LlmRequestAuditProviderBreakdown {
                    provider_id: row.get(0)?,
                    request_count: row.get(1)?,
                    success_count: row.get(2)?,
                    total_tokens: row.get(3)?,
                    latency_count: row.get(4)?,
                    latency_sum: row.get(5)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }
    pub fn scheduled_task_usage_summary(
        &self,
        task_id: &str,
    ) -> Result<LlmRequestAuditSummaryRow, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT
                    COUNT(*),
                    COUNT(CASE WHEN final_state NOT IN ('succeeded', 'completed') THEN 1 END),
                    COALESCE(SUM(COALESCE(input_tokens, 0)), 0),
                    COALESCE(SUM(COALESCE(output_tokens, 0)), 0),
                    COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0),
                    COALESCE(SUM(COALESCE(cache_write_tokens, 0)), 0),
                    COALESCE(SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)), 0),
                    COUNT(total_latency_ms),
                    COALESCE(SUM(COALESCE(total_latency_ms, 0)), 0)
                 FROM llm_requests
                 WHERE agent_task_id IN (
                    SELECT DISTINCT agent_task_id
                    FROM scheduled_task_runs
                    WHERE task_id = ?1 AND agent_task_id IS NOT NULL
                 )",
                params![task_id],
                |row| {
                    Ok(LlmRequestAuditSummaryRow {
                        total_requests: row.get(0)?,
                        failed_requests: row.get(1)?,
                        total_input_tokens: row.get(2)?,
                        total_output_tokens: row.get(3)?,
                        total_cache_read_tokens: row.get(4)?,
                        total_cache_write_tokens: row.get(5)?,
                        total_tokens: row.get(6)?,
                        latency_count: row.get(7)?,
                        latency_sum: row.get(8)?,
                    })
                },
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn insert_context_compression_snapshot(
        &mut self,
        snapshot: NewContextCompressionSnapshot<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let metadata_json = snapshot.metadata_json.unwrap_or("{}");
        let created_at = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO context_compression_snapshots
                    (
                        id, chat_id, run_id, sequence, summary,
                        source_message_start_sequence, source_message_end_sequence,
                        original_token_count, summary_token_count, created_at, metadata_json
                    )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    snapshot.id,
                    snapshot.chat_id,
                    snapshot.run_id,
                    snapshot.sequence,
                    snapshot.summary,
                    snapshot.source_message_start_sequence,
                    snapshot.source_message_end_sequence,
                    snapshot.original_token_count,
                    snapshot.summary_token_count,
                    created_at,
                    metadata_json
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn context_compression_snapshots_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Vec<ContextCompressionSnapshotRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    id, chat_id, run_id, sequence, summary,
                    source_message_start_sequence, source_message_end_sequence,
                    original_token_count, summary_token_count, created_at, metadata_json
                 FROM context_compression_snapshots
                 WHERE chat_id = ?1
                 ORDER BY sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| {
                Ok(ContextCompressionSnapshotRecord {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    run_id: row.get(2)?,
                    sequence: row.get(3)?,
                    summary: row.get(4)?,
                    source_message_start_sequence: row.get(5)?,
                    source_message_end_sequence: row.get(6)?,
                    original_token_count: row.get(7)?,
                    summary_token_count: row.get(8)?,
                    created_at: row.get(9)?,
                    metadata_json: row.get(10)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn insert_prompt_context_injection(
        &mut self,
        injection: NewPromptContextInjection<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let created_at = now_timestamp();

        let query = match injection.kind {
            "stable" => {
                "INSERT INTO prompt_context_injections
                    (id, chat_id, kind, sequence, messages_json, memory_keys_json, memory_summaries_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(chat_id) WHERE kind = 'stable' DO UPDATE SET
                    messages_json = excluded.messages_json,
                    memory_keys_json = excluded.memory_keys_json,
                    memory_summaries_json = excluded.memory_summaries_json,
                    created_at = excluded.created_at"
            }
            "turn_memory" => {
                "INSERT INTO prompt_context_injections
                    (id, chat_id, kind, sequence, messages_json, memory_keys_json, memory_summaries_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(chat_id, sequence) WHERE kind = 'turn_memory' DO UPDATE SET
                    messages_json = excluded.messages_json,
                    memory_keys_json = excluded.memory_keys_json,
                    memory_summaries_json = excluded.memory_summaries_json,
                    created_at = excluded.created_at"
            }
            _ => {
                "INSERT INTO prompt_context_injections
                    (id, chat_id, kind, sequence, messages_json, memory_keys_json, memory_summaries_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            }
        };
        self.connection
            .execute(
                query,
                params![
                    injection.id,
                    injection.chat_id,
                    injection.kind,
                    injection.sequence,
                    injection.messages_json,
                    injection.memory_keys_json,
                    injection.memory_summaries_json,
                    created_at
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn prompt_context_injections_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Vec<PromptContextInjectionRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, chat_id, kind, sequence, messages_json, memory_keys_json, memory_summaries_json, created_at
                 FROM prompt_context_injections
                 WHERE chat_id = ?1
                 ORDER BY
                    CASE kind WHEN 'stable' THEN 0 ELSE 1 END,
                    sequence ASC,
                    created_at ASC,
                    id ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![chat_id], |row| {
                Ok(PromptContextInjectionRecord {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    kind: row.get(2)?,
                    sequence: row.get(3)?,
                    messages_json: row.get(4)?,
                    memory_keys_json: row.get(5)?,
                    memory_summaries_json: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn insert_scheduled_task(
        &mut self,
        task: NewScheduledTask<'_>,
    ) -> Result<ScheduledTaskRecord, WorkspaceDatabaseError> {
        validate_scheduled_task_status(task.status)?;
        validate_scheduled_task_json_object(task.schedule_json, "schedule_json")?;
        validate_scheduled_task_json_object(task.action_json, "action_json")?;
        let metadata_json = task.metadata_json.unwrap_or("{}");
        validate_scheduled_task_json_object(metadata_json, "metadata_json")?;
        let now = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO scheduled_tasks
                    (id, title, description, schedule_json, action_json, status,
                     next_run_at, created_at, updated_at, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9)",
                params![
                    task.id,
                    task.title,
                    task.description,
                    task.schedule_json,
                    task.action_json,
                    task.status,
                    task.next_run_at,
                    now,
                    metadata_json
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        self.scheduled_task(task.id)?
            .ok_or_else(|| WorkspaceDatabaseError::MissingScheduledTask {
                id: task.id.to_string(),
            })
    }

    pub fn update_scheduled_task(
        &mut self,
        task: ScheduledTaskUpdate<'_>,
    ) -> Result<ScheduledTaskRecord, WorkspaceDatabaseError> {
        validate_scheduled_task_status(task.status)?;
        validate_scheduled_task_json_object(task.schedule_json, "schedule_json")?;
        validate_scheduled_task_json_object(task.action_json, "action_json")?;
        validate_scheduled_task_json_object(task.metadata_json, "metadata_json")?;
        let now = now_timestamp();

        let updated = self
            .connection
            .execute(
                "UPDATE scheduled_tasks
                 SET title = ?2,
                     description = ?3,
                     schedule_json = ?4,
                     action_json = ?5,
                     status = ?6,
                     next_run_at = ?7,
                     last_run_at = ?8,
                     updated_at = ?9,
                     metadata_json = ?10
                 WHERE id = ?1",
                params![
                    task.id,
                    task.title,
                    task.description,
                    task.schedule_json,
                    task.action_json,
                    task.status,
                    task.next_run_at,
                    task.last_run_at,
                    now,
                    task.metadata_json
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        if updated == 0 {
            return Err(WorkspaceDatabaseError::MissingScheduledTask {
                id: task.id.to_string(),
            });
        }

        self.scheduled_task(task.id)?
            .ok_or_else(|| WorkspaceDatabaseError::MissingScheduledTask {
                id: task.id.to_string(),
            })
    }

    pub fn scheduled_task(
        &self,
        id: &str,
    ) -> Result<Option<ScheduledTaskRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, title, description, schedule_json, action_json, status,
                        next_run_at, last_run_at, created_at, updated_at, metadata_json
                 FROM scheduled_tasks
                 WHERE id = ?1",
                params![id],
                scheduled_task_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn scheduled_tasks(
        &self,
        status: Option<&str>,
    ) -> Result<Vec<ScheduledTaskRecord>, WorkspaceDatabaseError> {
        let total_count = self.scheduled_task_count(ScheduledTaskListFilter {
            status,
            search: None,
            limit: i64::MAX,
            offset: 0,
        })?;
        self.scheduled_tasks_page(ScheduledTaskListFilter {
            status,
            search: None,
            limit: total_count.max(1),
            offset: 0,
        })
    }

    pub fn scheduled_task_count(
        &self,
        filter: ScheduledTaskListFilter<'_>,
    ) -> Result<i64, WorkspaceDatabaseError> {
        validate_scheduled_task_list_filter(&filter)?;
        let (where_clause, query_params) = scheduled_task_filter_sql(filter.status, filter.search)?;
        self.connection
            .query_row(
                &format!("SELECT COUNT(*) FROM scheduled_tasks{where_clause}"),
                params_from_iter(query_params),
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn scheduled_task_status_counts(
        &self,
        search: Option<&str>,
    ) -> Result<Vec<ScheduledTaskStatusCountRecord>, WorkspaceDatabaseError> {
        let (where_clause, query_params) = scheduled_task_filter_sql(None, search)?;
        let mut statement = self
            .connection
            .prepare(&format!(
                "SELECT status, COUNT(*)
                 FROM scheduled_tasks{where_clause}
                 GROUP BY status"
            ))
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok(ScheduledTaskStatusCountRecord {
                    status: row.get(0)?,
                    count: row.get(1)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn scheduled_tasks_page(
        &self,
        filter: ScheduledTaskListFilter<'_>,
    ) -> Result<Vec<ScheduledTaskRecord>, WorkspaceDatabaseError> {
        validate_scheduled_task_list_filter(&filter)?;
        let (where_clause, mut query_params) =
            scheduled_task_filter_sql(filter.status, filter.search)?;
        let mut query = String::from(
            "SELECT id, title, description, schedule_json, action_json, status,
                    next_run_at, last_run_at, created_at, updated_at, metadata_json
             FROM scheduled_tasks",
        );
        query.push_str(&where_clause);
        query.push_str(
            " ORDER BY
                CASE WHEN next_run_at IS NULL THEN 1 ELSE 0 END,
                next_run_at ASC,
                updated_at DESC,
                id ASC
              LIMIT ? OFFSET ?",
        );
        query_params.push(SqlValue::Integer(filter.limit));
        query_params.push(SqlValue::Integer(filter.offset));
        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), scheduled_task_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn scheduled_task_usage_summaries(
        &self,
        task_ids: &[String],
    ) -> Result<HashMap<String, LlmRequestAuditSummaryRow>, WorkspaceDatabaseError> {
        if task_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders = (1..=task_ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT
                runs.task_id,
                COUNT(requests.id),
                COUNT(CASE WHEN requests.final_state NOT IN ('succeeded', 'completed') THEN 1 END),
                COALESCE(SUM(COALESCE(requests.input_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(requests.output_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(requests.cache_read_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(requests.cache_write_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(requests.input_tokens, 0) + COALESCE(requests.output_tokens, 0)), 0),
                COUNT(requests.total_latency_ms),
                COALESCE(SUM(COALESCE(requests.total_latency_ms, 0)), 0)
             FROM (
                SELECT DISTINCT task_id, agent_task_id
                FROM scheduled_task_runs
                WHERE task_id IN ({placeholders}) AND agent_task_id IS NOT NULL
             ) runs
             JOIN llm_requests requests ON requests.agent_task_id = runs.agent_task_id
             GROUP BY runs.task_id"
        );
        let query_params = task_ids
            .iter()
            .cloned()
            .map(SqlValue::Text)
            .collect::<Vec<_>>();
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    LlmRequestAuditSummaryRow {
                        total_requests: row.get(1)?,
                        failed_requests: row.get(2)?,
                        total_input_tokens: row.get(3)?,
                        total_output_tokens: row.get(4)?,
                        total_cache_read_tokens: row.get(5)?,
                        total_cache_write_tokens: row.get(6)?,
                        total_tokens: row.get(7)?,
                        latency_count: row.get(8)?,
                        latency_sum: row.get(9)?,
                    },
                ))
            })
            .map_err(|source| self.sqlite_error(source))?;
        let pairs = collect_rows(rows, &self.database_path)?;
        Ok(pairs.into_iter().collect())
    }

    pub fn scheduled_task_run_count(&self, task_id: &str) -> Result<i64, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM scheduled_task_runs WHERE task_id = ?1",
                params![task_id],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn scheduled_task_runs_for_task_page(
        &self,
        task_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ScheduledTaskRunRecord>, WorkspaceDatabaseError> {
        if limit <= 0 || offset < 0 {
            return Err(WorkspaceDatabaseError::InvalidScheduledTaskData {
                message: "scheduled task run pagination limit must be positive and offset must be non-negative"
                    .to_string(),
            });
        }
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, task_id, trigger_reason, status, scheduled_at, queued_at,
                        started_at, completed_at, chat_id, user_message_id,
                        assistant_message_id, agent_team_id, agent_task_id, agent_attempt_id,
                        active_run_id, error_message, output_summary, created_at, updated_at,
                        metadata_json
                 FROM scheduled_task_runs
                 WHERE task_id = ?1
                 ORDER BY scheduled_at DESC, created_at DESC, id DESC
                 LIMIT ?2 OFFSET ?3",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![task_id, limit, offset], scheduled_task_run_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn next_enabled_scheduled_task_run_at(
        &self,
    ) -> Result<Option<String>, WorkspaceDatabaseError> {
        self.connection
            .query_row(NEXT_ENABLED_SCHEDULED_TASK_SQL, [], |row| row.get(0))
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn active_scheduled_task_run_count(
        &self,
        task_id: &str,
    ) -> Result<i64, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT COUNT(*)
                 FROM scheduled_task_runs
                 WHERE task_id = ?1 AND status IN ('pending', 'queued', 'running')",
                params![task_id],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn active_scheduled_task_runs(
        &self,
    ) -> Result<Vec<ScheduledTaskRunRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, task_id, trigger_reason, status, scheduled_at, queued_at,
                        started_at, completed_at, chat_id, user_message_id,
                        assistant_message_id, agent_team_id, agent_task_id, agent_attempt_id,
                        active_run_id, error_message, output_summary, created_at, updated_at,
                        metadata_json
                 FROM scheduled_task_runs
                 WHERE status IN ('pending', 'queued', 'running')
                 ORDER BY scheduled_at ASC, created_at ASC, id ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], scheduled_task_run_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn claim_due_scheduled_task_run(
        &mut self,
        claim: ScheduledTaskDueRunClaim<'_>,
    ) -> Result<Option<ScheduledTaskRunRecord>, WorkspaceDatabaseError> {
        validate_scheduled_task_trigger_reason(claim.trigger_reason)?;
        validate_scheduled_task_run_status(claim.run_status)?;
        validate_scheduled_task_status(claim.task_status)?;
        let metadata_json = claim.metadata_json.unwrap_or("{}");
        validate_scheduled_task_json_object(metadata_json, "metadata_json")?;
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let due = transaction
            .query_row(
                "SELECT 1
                 FROM scheduled_tasks
                 WHERE id = ?1
                   AND status = 'enabled'
                   AND next_run_at = ?2
                   AND next_run_at <= ?3",
                params![
                    claim.task_id,
                    claim.expected_next_run_at,
                    claim.task_last_run_at
                ],
                |_| Ok(()),
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?;
        if due.is_none() {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(None);
        }

        let updated = transaction
            .execute(
                "UPDATE scheduled_tasks
                 SET status = ?2,
                     next_run_at = ?3,
                     last_run_at = ?4,
                     updated_at = ?4
                 WHERE id = ?1
                   AND status = 'enabled'
                   AND next_run_at = ?5",
                params![
                    claim.task_id,
                    claim.task_status,
                    claim.task_next_run_at,
                    claim.task_last_run_at,
                    claim.expected_next_run_at
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if updated != 1 {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(None);
        }

        transaction
            .execute(
                "INSERT INTO scheduled_task_runs
                    (
                        id, task_id, trigger_reason, status, scheduled_at, queued_at,
                        started_at, completed_at, chat_id, user_message_id,
                        assistant_message_id, agent_team_id, agent_task_id, agent_attempt_id,
                        active_run_id, error_message, output_summary, created_at, updated_at,
                        metadata_json
                    )
                 VALUES
                    (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, NULL, NULL,
                     NULL, NULL, NULL, NULL, NULL, ?7, NULL, ?8, ?8, ?9)",
                params![
                    claim.run_id,
                    claim.task_id,
                    claim.trigger_reason,
                    claim.run_status,
                    claim.scheduled_at,
                    claim.completed_at,
                    claim.error_message,
                    claim.task_last_run_at,
                    metadata_json
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        self.scheduled_task_run(claim.run_id)
            .map(|run| {
                run.expect("claimed scheduled task run should exist after transaction commit")
            })
            .map(Some)
    }

    pub fn delete_scheduled_task(&mut self, id: &str) -> Result<bool, WorkspaceDatabaseError> {
        let deleted = self
            .connection
            .execute("DELETE FROM scheduled_tasks WHERE id = ?1", params![id])
            .map_err(|source| self.sqlite_error(source))?;

        Ok(deleted > 0)
    }

    pub fn insert_scheduled_task_run(
        &mut self,
        run: NewScheduledTaskRun<'_>,
    ) -> Result<ScheduledTaskRunRecord, WorkspaceDatabaseError> {
        validate_scheduled_task_trigger_reason(run.trigger_reason)?;
        validate_scheduled_task_run_status(run.status)?;
        let metadata_json = run.metadata_json.unwrap_or("{}");
        validate_scheduled_task_json_object(metadata_json, "metadata_json")?;
        let now = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO scheduled_task_runs
                    (
                        id, task_id, trigger_reason, status, scheduled_at, queued_at,
                        started_at, completed_at, chat_id, user_message_id,
                        assistant_message_id, agent_team_id, agent_task_id, agent_attempt_id,
                        active_run_id, error_message, output_summary, created_at, updated_at,
                        metadata_json
                    )
                 VALUES
                    (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?18, ?19)",
                params![
                    run.id,
                    run.task_id,
                    run.trigger_reason,
                    run.status,
                    run.scheduled_at,
                    run.queued_at,
                    run.started_at,
                    run.completed_at,
                    run.chat_id,
                    run.user_message_id,
                    run.assistant_message_id,
                    run.agent_team_id.map(AgentTeamId::as_str),
                    run.agent_task_id.map(AgentTaskId::as_str),
                    run.agent_attempt_id.map(AgentAttemptId::as_str),
                    run.active_run_id,
                    run.error_message,
                    run.output_summary,
                    now,
                    metadata_json
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        self.scheduled_task_run(run.id)?.ok_or_else(|| {
            WorkspaceDatabaseError::MissingScheduledTaskRun {
                id: run.id.to_string(),
            }
        })
    }

    pub fn update_scheduled_task_run(
        &mut self,
        run: ScheduledTaskRunUpdate<'_>,
    ) -> Result<ScheduledTaskRunRecord, WorkspaceDatabaseError> {
        validate_scheduled_task_run_status(run.status)?;
        validate_scheduled_task_json_object(run.metadata_json, "metadata_json")?;
        let now = now_timestamp();

        let updated = self
            .connection
            .execute(
                "UPDATE scheduled_task_runs
                 SET status = ?2,
                     queued_at = ?3,
                     started_at = ?4,
                     completed_at = ?5,
                     chat_id = ?6,
                     user_message_id = ?7,
                     assistant_message_id = ?8,
                     agent_team_id = ?9,
                     agent_task_id = ?10,
                     agent_attempt_id = ?11,
                     active_run_id = ?12,
                     error_message = ?13,
                     output_summary = ?14,
                     updated_at = ?15,
                     metadata_json = ?16
                 WHERE id = ?1",
                params![
                    run.id,
                    run.status,
                    run.queued_at,
                    run.started_at,
                    run.completed_at,
                    run.chat_id,
                    run.user_message_id,
                    run.assistant_message_id,
                    run.agent_team_id.map(AgentTeamId::as_str),
                    run.agent_task_id.map(AgentTaskId::as_str),
                    run.agent_attempt_id.map(AgentAttemptId::as_str),
                    run.active_run_id,
                    run.error_message,
                    run.output_summary,
                    now,
                    run.metadata_json
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        if updated == 0 {
            return Err(WorkspaceDatabaseError::MissingScheduledTaskRun {
                id: run.id.to_string(),
            });
        }

        self.scheduled_task_run(run.id)?.ok_or_else(|| {
            WorkspaceDatabaseError::MissingScheduledTaskRun {
                id: run.id.to_string(),
            }
        })
    }

    pub fn scheduled_task_run(
        &self,
        id: &str,
    ) -> Result<Option<ScheduledTaskRunRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, task_id, trigger_reason, status, scheduled_at, queued_at,
                        started_at, completed_at, chat_id, user_message_id,
                        assistant_message_id, agent_team_id, agent_task_id, agent_attempt_id,
                        active_run_id, error_message, output_summary, created_at, updated_at,
                        metadata_json
                 FROM scheduled_task_runs
                 WHERE id = ?1",
                params![id],
                scheduled_task_run_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn scheduled_task_runs_for_task(
        &self,
        task_id: &str,
    ) -> Result<Vec<ScheduledTaskRunRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, task_id, trigger_reason, status, scheduled_at, queued_at,
                        started_at, completed_at, chat_id, user_message_id,
                        assistant_message_id, agent_team_id, agent_task_id, agent_attempt_id,
                        active_run_id, error_message, output_summary, created_at, updated_at,
                        metadata_json
                 FROM scheduled_task_runs
                 WHERE task_id = ?1
                 ORDER BY scheduled_at DESC, created_at DESC, id DESC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![task_id], scheduled_task_run_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn delete_old_scheduled_task_runs(
        &mut self,
        completed_before: &str,
    ) -> Result<usize, WorkspaceDatabaseError> {
        self.connection
            .execute(
                "DELETE FROM scheduled_task_runs
                 WHERE status IN ('succeeded', 'failed', 'cancelled', 'skipped')
                   AND completed_at IS NOT NULL
                   AND completed_at < ?1",
                params![completed_before],
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn scheduled_task_runs_for_agent_task(
        &self,
        agent_task_id: &AgentTaskId,
    ) -> Result<Vec<ScheduledTaskRunRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, task_id, trigger_reason, status, scheduled_at, queued_at,
                        started_at, completed_at, chat_id, user_message_id,
                        assistant_message_id, agent_team_id, agent_task_id, agent_attempt_id,
                        active_run_id, error_message, output_summary, created_at, updated_at,
                        metadata_json
                 FROM scheduled_task_runs
                 WHERE agent_task_id = ?1
                 ORDER BY scheduled_at DESC, created_at DESC, id DESC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![agent_task_id.as_str()], scheduled_task_run_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn create_agent_team(
        &mut self,
        team: NewAgentTeam<'_>,
    ) -> Result<(AgentTeamRecord, AgentInstanceRecord), WorkspaceDatabaseError> {
        if team.max_concurrent_runs <= 0 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "max_concurrent_runs must be greater than 0".to_string(),
            });
        }
        let snapshot_json =
            serde_json::to_string(team.coordinator_definition).map_err(|source| {
                WorkspaceDatabaseError::AgentRuntimeJson {
                    field: "definition_snapshot_json",
                    source,
                }
            })?;
        validate_agent_definition_snapshot(&snapshot_json)?;
        match team.coordinator_execution_workspace_mode {
            AgentExecutionWorkspaceMode::Shared => {
                if team.coordinator_execution_root_path.is_some()
                    || team.coordinator_worktree_base_revision.is_some()
                    || team.coordinator_worktree_branch.is_some()
                    || team.coordinator_worktree_status.is_some()
                {
                    return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                        message: "shared Coordinator must not include Agent worktree metadata"
                            .to_string(),
                    });
                }
            }
            AgentExecutionWorkspaceMode::IsolatedWorktree => {
                if team.coordinator_execution_root_path.is_none()
                    || team.coordinator_worktree_base_revision.is_none()
                    || team.coordinator_worktree_branch.is_none()
                    || team.coordinator_worktree_status.is_none()
                {
                    return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                        message: "isolated Coordinator requires Agent worktree metadata"
                            .to_string(),
                    });
                }
            }
        }

        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "INSERT INTO agent_teams
                    (id, chat_id, coordinator_instance_id, status, max_concurrent_runs,
                     next_event_sequence, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'active', ?4, 0, ?5, ?5)",
                params![
                    team.id.as_str(),
                    team.chat_id,
                    team.coordinator_instance_id.as_str(),
                    team.max_concurrent_runs,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "INSERT INTO agent_instances
                    (id, team_id, definition_id, definition_revision,
                     definition_snapshot_json, role, status, next_task_sequence,
                     next_message_sequence, context_generation, execution_workspace_mode,
                     execution_root_path, worktree_base_revision, worktree_branch,
                     worktree_status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'coordinator', 'idle', 0, 0, 0, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    team.coordinator_instance_id.as_str(),
                    team.id.as_str(),
                    team.coordinator_definition.id.as_str(),
                    i64::try_from(team.coordinator_definition.revision).map_err(|_| {
                        WorkspaceDatabaseError::InvalidAgentRuntimeData {
                            message: "agent definition revision exceeds SQLite integer range"
                                .to_string(),
                        }
                    })?,
                    snapshot_json,
                    team.coordinator_execution_workspace_mode.as_str(),
                    team.coordinator_execution_root_path,
                    team.coordinator_worktree_base_revision,
                    team.coordinator_worktree_branch,
                    team.coordinator_worktree_status,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        let team_record = self.agent_team(team.id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "created agent team was not found".to_string(),
            }
        })?;
        let instance_record = self
            .agent_instance(team.coordinator_instance_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "created coordinator instance was not found".to_string(),
            })?;
        Ok((team_record, instance_record))
    }

    pub fn create_agent_instances_with_limits(
        &mut self,
        instances: &[NewAgentInstance<'_>],
        max_team_instances: i64,
        max_definition_instances: i64,
    ) -> Result<Vec<AgentInstanceRecord>, WorkspaceDatabaseError> {
        if instances.is_empty() {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "at least one Agent instance is required".to_string(),
            });
        }
        if max_team_instances <= 0 || max_definition_instances <= 0 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "Agent instance limits must be greater than 0".to_string(),
            });
        }
        let first = &instances[0];
        if instances.iter().any(|instance| {
            instance.team_id != first.team_id
                || instance.definition.id != first.definition.id
                || instance.definition.revision != first.definition.revision
        }) {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "all Agent instances in one create request must share team, definition, and revision".to_string(),
            });
        }
        let count = i64::try_from(instances.len()).map_err(|_| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "Agent instance count exceeds SQLite integer range".to_string(),
            }
        })?;
        let snapshot_json = serde_json::to_string(first.definition).map_err(|source| {
            WorkspaceDatabaseError::AgentRuntimeJson {
                field: "definition_snapshot_json",
                source,
            }
        })?;
        validate_agent_definition_snapshot(&snapshot_json)?;
        let definition_revision = i64::try_from(first.definition.revision).map_err(|_| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "agent definition revision exceeds SQLite integer range".to_string(),
            }
        })?;

        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let team_status = transaction
            .query_row(
                "SELECT status FROM agent_teams WHERE id = ?1",
                params![first.team_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent team '{}' was not found", first.team_id),
            })?;
        if team_status != AgentTeamStatus::Active.as_str() {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent team '{}' does not accept new instances while {team_status}",
                    first.team_id
                ),
            });
        }
        let team_instances: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM agent_instances WHERE team_id = ?1",
                params![first.team_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let definition_instances: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM agent_instances
                 WHERE team_id = ?1 AND definition_id = ?2",
                params![first.team_id.as_str(), first.definition.id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if team_instances + count > max_team_instances {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent team '{}' would exceed instance limit {max_team_instances}",
                    first.team_id
                ),
            });
        }
        if definition_instances + count > max_definition_instances {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent definition '{}' would exceed team instance limit {max_definition_instances}",
                    first.definition.id
                ),
            });
        }

        for instance in instances {
            transaction
                .execute(
                    "INSERT INTO agent_instances
                        (id, team_id, definition_id, definition_revision,
                         definition_snapshot_json, role, status, next_task_sequence,
                         next_message_sequence, context_generation, execution_workspace_mode,
                         execution_root_path, worktree_base_revision, worktree_branch,
                         worktree_status, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'idle', 0, 0, 0, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                    params![
                        instance.id.as_str(),
                        instance.team_id.as_str(),
                        instance.definition.id.as_str(),
                        definition_revision,
                        snapshot_json,
                        instance.role.as_str(),
                        instance.execution_workspace_mode.as_str(),
                        instance.execution_root_path,
                        instance.worktree_base_revision,
                        instance.worktree_branch,
                        instance.worktree_status,
                        now
                    ],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        let mut created = Vec::with_capacity(instances.len());
        for instance in instances {
            created.push(self.agent_instance(instance.id)?.ok_or_else(|| {
                WorkspaceDatabaseError::InvalidAgentRuntimeData {
                    message: format!("created Agent instance '{}' was not found", instance.id),
                }
            })?);
        }
        Ok(created)
    }

    pub fn agent_team(
        &self,
        team_id: &AgentTeamId,
    ) -> Result<Option<AgentTeamRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, chat_id, coordinator_instance_id, status, max_concurrent_runs,
                        next_event_sequence, created_at, updated_at
                 FROM agent_teams WHERE id = ?1",
                params![team_id.as_str()],
                agent_team_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn agent_team_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Option<AgentTeamRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, chat_id, coordinator_instance_id, status, max_concurrent_runs,
                        next_event_sequence, created_at, updated_at
                 FROM agent_teams WHERE chat_id = ?1",
                params![chat_id],
                agent_team_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn agent_instance(
        &self,
        instance_id: &AgentInstanceId,
    ) -> Result<Option<AgentInstanceRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, team_id, definition_id, definition_revision,
                        definition_snapshot_json, role, status, next_task_sequence,
                        next_message_sequence, context_generation, last_scheduled_at,
                        execution_workspace_mode, execution_root_path, worktree_base_revision,
                        worktree_branch, worktree_status,
                        created_at, updated_at
                 FROM agent_instances WHERE id = ?1",
                params![instance_id.as_str()],
                agent_instance_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn agent_instances_for_team(
        &self,
        team_id: &AgentTeamId,
    ) -> Result<Vec<AgentInstanceRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, team_id, definition_id, definition_revision,
                        definition_snapshot_json, role, status, next_task_sequence,
                        next_message_sequence, context_generation, last_scheduled_at,
                        execution_workspace_mode, execution_root_path, worktree_base_revision,
                        worktree_branch, worktree_status,
                        created_at, updated_at
                 FROM agent_instances WHERE team_id = ?1
                 ORDER BY CASE role WHEN 'coordinator' THEN 0 ELSE 1 END, created_at, id",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![team_id.as_str()], agent_instance_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn isolated_agent_instances(
        &self,
    ) -> Result<Vec<AgentInstanceRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, team_id, definition_id, definition_revision,
                        definition_snapshot_json, role, status, next_task_sequence,
                        next_message_sequence, context_generation, last_scheduled_at,
                        execution_workspace_mode, execution_root_path, worktree_base_revision,
                        worktree_branch, worktree_status,
                        created_at, updated_at
                 FROM agent_instances
                 WHERE execution_workspace_mode = 'isolated_worktree'
                 ORDER BY team_id, created_at, id",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], agent_instance_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn agent_instances_for_definition(
        &self,
        team_id: &AgentTeamId,
        definition_id: &foco_agent::AgentDefinitionId,
    ) -> Result<Vec<AgentInstanceRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, team_id, definition_id, definition_revision,
                        definition_snapshot_json, role, status, next_task_sequence,
                        next_message_sequence, context_generation, last_scheduled_at,
                        execution_workspace_mode, execution_root_path, worktree_base_revision,
                        worktree_branch, worktree_status,
                        created_at, updated_at
                 FROM agent_instances
                 WHERE team_id = ?1 AND definition_id = ?2
                 ORDER BY last_scheduled_at IS NULL DESC, last_scheduled_at, created_at, id",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(
                params![team_id.as_str(), definition_id.as_str()],
                agent_instance_from_row,
            )
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn route_agent_instance_for_definition(
        &self,
        team_id: &AgentTeamId,
        definition_id: &foco_agent::AgentDefinitionId,
    ) -> Result<Option<AgentInstanceRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT instance.id, instance.team_id, instance.definition_id,
                        instance.definition_revision, instance.definition_snapshot_json,
                        instance.role, instance.status, instance.next_task_sequence,
                        instance.next_message_sequence, instance.context_generation,
                        instance.last_scheduled_at, instance.execution_workspace_mode,
                        instance.execution_root_path, instance.worktree_base_revision,
                        instance.worktree_branch, instance.worktree_status,
                        instance.created_at, instance.updated_at
                 FROM agent_instances AS instance
                 LEFT JOIN agent_tasks AS task
                   ON task.owner_instance_id = instance.id
                  AND task.status IN ('queued', 'running', 'waiting')
                 WHERE instance.team_id = ?1
                   AND instance.definition_id = ?2
                   AND instance.status IN ('idle', 'running')
                 GROUP BY instance.id
                 ORDER BY COUNT(task.id), instance.last_scheduled_at IS NOT NULL,
                          instance.last_scheduled_at, instance.created_at, instance.id
                 LIMIT 1",
                params![team_id.as_str(), definition_id.as_str()],
                agent_instance_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn agent_team_workload(
        &self,
        team_id: &AgentTeamId,
    ) -> Result<TeamWorkload, WorkspaceDatabaseError> {
        let (queued, running, waiting) = self
            .connection
            .query_row(
                "SELECT
                    SUM(CASE WHEN status = 'queued' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 'running' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 'waiting' THEN 1 ELSE 0 END)
                 FROM agent_tasks WHERE team_id = ?1",
                params![team_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                        row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                        row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    ))
                },
            )
            .map_err(|source| self.sqlite_error(source))?;
        Ok(TeamWorkload {
            queued_tasks: u32::try_from(queued).map_err(|_| {
                WorkspaceDatabaseError::InvalidAgentRuntimeData {
                    message: "queued Agent task count exceeds u32".to_string(),
                }
            })?,
            running_tasks: u32::try_from(running).map_err(|_| {
                WorkspaceDatabaseError::InvalidAgentRuntimeData {
                    message: "running Agent task count exceeds u32".to_string(),
                }
            })?,
            waiting_tasks: u32::try_from(waiting).map_err(|_| {
                WorkspaceDatabaseError::InvalidAgentRuntimeData {
                    message: "waiting Agent task count exceeds u32".to_string(),
                }
            })?,
        })
    }

    pub fn transition_agent_team_status(
        &mut self,
        team_id: &AgentTeamId,
        target: AgentTeamStatus,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let current = self.agent_team(team_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent team '{team_id}' was not found"),
            }
        })?;
        current
            .status
            .transition_to(target)
            .map_err(|source| WorkspaceDatabaseError::AgentDomain { source })?;
        if target == AgentTeamStatus::Stopped {
            self.agent_team_workload(team_id)?
                .validate_deactivation()
                .map_err(|source| WorkspaceDatabaseError::AgentDomain { source })?;
        }

        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let updated = transaction
            .execute(
                "UPDATE agent_teams SET status = ?2, updated_at = ?3
                 WHERE id = ?1 AND status = ?4
                   AND (NOT ?5 OR NOT EXISTS (
                        SELECT 1 FROM agent_tasks
                        WHERE team_id = ?1 AND status IN ('queued', 'running', 'waiting')
                   ))",
                params![
                    team_id.as_str(),
                    target.as_str(),
                    now,
                    current.status.as_str(),
                    target == AgentTeamStatus::Stopped
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if updated != 1 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent team '{team_id}' changed state or workload during transition"
                ),
            });
        }
        if updated == 1 {
            match target {
                AgentTeamStatus::Active => {
                    transaction
                        .execute(
                            "UPDATE agent_instances SET status = 'idle', updated_at = ?2
                             WHERE team_id = ?1 AND status IN ('paused', 'failed')",
                            params![team_id.as_str(), now],
                        )
                        .map_err(|source| sqlite_error(&database_path, source))?;
                }
                AgentTeamStatus::Paused => {
                    transaction
                        .execute(
                            "UPDATE agent_instances SET status = 'paused', updated_at = ?2
                             WHERE team_id = ?1 AND status = 'idle'",
                            params![team_id.as_str(), now],
                        )
                        .map_err(|source| sqlite_error(&database_path, source))?;
                }
                AgentTeamStatus::Draining => {
                    transaction
                        .execute(
                            "UPDATE agent_instances SET status = 'draining', updated_at = ?2
                             WHERE team_id = ?1 AND status IN ('idle', 'paused')",
                            params![team_id.as_str(), now],
                        )
                        .map_err(|source| sqlite_error(&database_path, source))?;
                }
                AgentTeamStatus::Stopped => {
                    transaction
                        .execute(
                            "UPDATE agent_instances SET status = 'stopped', updated_at = ?2
                             WHERE team_id = ?1",
                            params![team_id.as_str(), now],
                        )
                        .map_err(|source| sqlite_error(&database_path, source))?;
                }
                AgentTeamStatus::Failed => {
                    transaction
                        .execute(
                            "UPDATE agent_instances SET status = 'failed', updated_at = ?2
                             WHERE team_id = ?1 AND status <> 'stopped'",
                            params![team_id.as_str(), now],
                        )
                        .map_err(|source| sqlite_error(&database_path, source))?;
                }
            }
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        Ok(updated == 1)
    }

    pub fn transition_agent_instance_status(
        &mut self,
        instance_id: &AgentInstanceId,
        target: AgentInstanceStatus,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let current = self.agent_instance(instance_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent instance '{instance_id}' was not found"),
            }
        })?;
        current
            .status
            .transition_to(target)
            .map_err(|source| WorkspaceDatabaseError::AgentDomain { source })?;
        if matches!(
            target,
            AgentInstanceStatus::Paused
                | AgentInstanceStatus::Draining
                | AgentInstanceStatus::Stopped
        ) {
            let blocking_statuses = if matches!(
                target,
                AgentInstanceStatus::Paused | AgentInstanceStatus::Draining
            ) {
                "'running', 'waiting'"
            } else {
                "'queued', 'running', 'waiting'"
            };
            let active_tasks: i64 = self
                .connection
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM agent_tasks
                         WHERE owner_instance_id = ?1 AND status IN ({blocking_statuses})"
                    ),
                    params![instance_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(|source| self.sqlite_error(source))?;
            if active_tasks > 0 {
                return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                    message: format!(
                        "Agent instance '{instance_id}' has {active_tasks} active or queued task(s)"
                    ),
                });
            }
        }
        let requires_empty_queue = target == AgentInstanceStatus::Stopped;
        let requires_no_running = matches!(
            target,
            AgentInstanceStatus::Paused | AgentInstanceStatus::Draining
        );
        let updated = self
            .connection
            .execute(
                "UPDATE agent_instances SET status = ?2, updated_at = ?3
                 WHERE id = ?1 AND status = ?4
                   AND (NOT ?5 OR NOT EXISTS (
                        SELECT 1 FROM agent_tasks
                        WHERE owner_instance_id = ?1
                          AND status IN ('queued', 'running', 'waiting')
                   ))
                   AND (NOT ?6 OR NOT EXISTS (
                        SELECT 1 FROM agent_tasks
                        WHERE owner_instance_id = ?1
                          AND status IN ('running', 'waiting')
                   ))",
                params![
                    instance_id.as_str(),
                    target.as_str(),
                    now_timestamp(),
                    current.status.as_str(),
                    requires_empty_queue,
                    requires_no_running
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        if updated != 1 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent instance '{instance_id}' changed state or workload during transition"
                ),
            });
        }
        Ok(updated == 1)
    }

    pub fn reset_agent_instance_context(
        &mut self,
        instance_id: &AgentInstanceId,
    ) -> Result<AgentInstanceRecord, WorkspaceDatabaseError> {
        let instance = self.agent_instance(instance_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent instance '{instance_id}' was not found"),
            }
        })?;
        if instance.context_generation == i64::MAX {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent instance '{instance_id}' context generation overflowed"),
            });
        }
        let active_tasks: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM agent_tasks
                 WHERE owner_instance_id = ?1 AND status IN ('queued', 'running', 'waiting')",
                params![instance_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))?;
        if active_tasks > 0 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent instance '{instance_id}' has {active_tasks} active or queued task(s)"
                ),
            });
        }
        let updated = self
            .connection
            .execute(
                "UPDATE agent_instances
                 SET context_generation = context_generation + 1, updated_at = ?3
                 WHERE id = ?1 AND context_generation = ?2
                   AND NOT EXISTS (
                        SELECT 1 FROM agent_tasks
                        WHERE owner_instance_id = ?1
                          AND status IN ('queued', 'running', 'waiting')
                   )",
                params![
                    instance_id.as_str(),
                    instance.context_generation,
                    now_timestamp()
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        if updated != 1 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent instance '{instance_id}' changed workload during context reset"
                ),
            });
        }
        self.agent_instance(instance_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent instance '{instance_id}' was not found after reset"),
            }
        })
    }

    pub fn update_agent_instance_worktree_status(
        &mut self,
        instance_id: &AgentInstanceId,
        status: &str,
    ) -> Result<AgentInstanceRecord, WorkspaceDatabaseError> {
        if !matches!(status, "active" | "kept" | "archived" | "deleted") {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("invalid Agent worktree status '{status}'"),
            });
        }
        let instance = self.agent_instance(instance_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent instance '{instance_id}' was not found"),
            }
        })?;
        if instance.execution_root_path.is_none() {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent instance '{instance_id}' does not use an isolated worktree"
                ),
            });
        }
        let updated = self
            .connection
            .execute(
                "UPDATE agent_instances
                 SET worktree_status = ?2, updated_at = ?3
                 WHERE id = ?1 AND execution_workspace_mode = 'isolated_worktree'",
                params![instance_id.as_str(), status, now_timestamp()],
            )
            .map_err(|source| self.sqlite_error(source))?;
        if updated != 1 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent instance '{instance_id}' worktree status was not updated"),
            });
        }
        self.agent_instance(instance_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent instance '{instance_id}' was not found after worktree update"
                ),
            }
        })
    }

    pub fn switch_agent_instance_to_shared_workspace(
        &mut self,
        instance_id: &AgentInstanceId,
    ) -> Result<AgentInstanceRecord, WorkspaceDatabaseError> {
        let instance = self.agent_instance(instance_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent instance '{instance_id}' was not found"),
            }
        })?;
        if instance.execution_workspace_mode == AgentExecutionWorkspaceMode::Shared {
            return Ok(instance);
        }
        let updated = self
            .connection
            .execute(
                "UPDATE agent_instances
                 SET execution_workspace_mode = 'shared',
                     execution_root_path = NULL,
                     worktree_base_revision = NULL,
                     worktree_branch = NULL,
                     worktree_status = NULL,
                     updated_at = ?2
                 WHERE id = ?1 AND execution_workspace_mode = 'isolated_worktree'",
                params![instance_id.as_str(), now_timestamp()],
            )
            .map_err(|source| self.sqlite_error(source))?;
        if updated != 1 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent instance '{instance_id}' workspace mode was not updated"),
            });
        }
        self.agent_instance(instance_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent instance '{instance_id}' was not found after mode update"),
            }
        })
    }

    pub fn delete_agent_instance(
        &mut self,
        instance_id: &AgentInstanceId,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let instance = self.agent_instance(instance_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent instance '{instance_id}' was not found"),
            }
        })?;
        let team = self.agent_team(&instance.team_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("Agent team '{}' was not found", instance.team_id),
            }
        })?;
        if team.coordinator_instance_id == *instance_id {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "the Coordinator instance cannot be deleted while its team exists"
                    .to_string(),
            });
        }
        let active_tasks: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM agent_tasks
                 WHERE owner_instance_id = ?1 AND status IN ('queued', 'running', 'waiting')",
                params![instance_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))?;
        if active_tasks > 0 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent instance '{instance_id}' has {active_tasks} active or queued task(s)"
                ),
            });
        }
        let deleted = self
            .connection
            .execute(
                "DELETE FROM agent_instances
                 WHERE id = ?1
                   AND NOT EXISTS (
                        SELECT 1 FROM agent_tasks
                        WHERE owner_instance_id = ?1
                          AND status IN ('queued', 'running', 'waiting')
                   )",
                params![instance_id.as_str()],
            )
            .map_err(|source| self.sqlite_error(source))?;
        if deleted != 1 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent instance '{instance_id}' changed state or workload during deletion"
                ),
            });
        }
        Ok(true)
    }

    pub fn enqueue_agent_task(
        &mut self,
        task: NewAgentTask<'_>,
    ) -> Result<AgentTaskRecord, WorkspaceDatabaseError> {
        self.enqueue_agent_task_with_limits(task, i64::MAX, i64::MAX, i64::MAX)
    }

    pub fn enqueue_agent_task_with_limits(
        &mut self,
        task: NewAgentTask<'_>,
        max_team_queued: i64,
        max_instance_queued: i64,
        max_chat_queued: i64,
    ) -> Result<AgentTaskRecord, WorkspaceDatabaseError> {
        if max_team_queued <= 0 || max_instance_queued <= 0 || max_chat_queued <= 0 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "Agent queued task limits must be greater than 0".to_string(),
            });
        }
        validate_agent_json(task.input_json, "input_json")?;
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        ensure_agent_entity_team(
            &transaction,
            "agent_instances",
            task.owner_instance_id.as_str(),
            task.team_id,
            AgentEntityKind::Instance,
            &database_path,
        )?;
        if let Some(origin_instance_id) = task.origin_instance_id {
            ensure_agent_entity_team(
                &transaction,
                "agent_instances",
                origin_instance_id.as_str(),
                task.team_id,
                AgentEntityKind::Instance,
                &database_path,
            )?;
        }
        if let Some(parent_task_id) = task.parent_task_id {
            ensure_agent_entity_team(
                &transaction,
                "agent_tasks",
                parent_task_id.as_str(),
                task.team_id,
                AgentEntityKind::Task,
                &database_path,
            )?;
        }

        let (team_status, instance_status) = transaction
            .query_row(
                "SELECT team.status, instance.status
                 FROM agent_teams AS team
                 JOIN agent_instances AS instance ON instance.team_id = team.id
                 WHERE team.id = ?1 AND instance.id = ?2",
                params![task.team_id.as_str(), task.owner_instance_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if team_status != AgentTeamStatus::Active.as_str() {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent team '{}' does not accept new tasks while {}",
                    task.team_id, team_status
                ),
            });
        }
        if !matches!(instance_status.as_str(), "idle" | "running") {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent instance '{}' does not accept new tasks while {}",
                    task.owner_instance_id, instance_status
                ),
            });
        }
        let team_queued: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM agent_tasks WHERE team_id = ?1 AND status = 'queued'",
                params![task.team_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let instance_queued: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM agent_tasks
                 WHERE owner_instance_id = ?1 AND status = 'queued'",
                params![task.owner_instance_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if team_queued >= max_team_queued || team_queued >= max_chat_queued {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent task queue is full for team/chat '{}' ({} queued)",
                    task.team_id, team_queued
                ),
            });
        }
        if instance_queued >= max_instance_queued {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent task queue is full for instance '{}' ({} queued)",
                    task.owner_instance_id, instance_queued
                ),
            });
        }

        let sequence: i64 = transaction
            .query_row(
                "SELECT next_task_sequence FROM agent_instances
                 WHERE id = ?1 AND team_id = ?2",
                params![task.owner_instance_id.as_str(), task.team_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "UPDATE agent_instances
                 SET next_task_sequence = next_task_sequence + 1, updated_at = ?3
                 WHERE id = ?1 AND team_id = ?2",
                params![task.owner_instance_id.as_str(), task.team_id.as_str(), now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "INSERT INTO agent_tasks
                    (id, team_id, owner_instance_id, origin_instance_id, parent_task_id,
                     sequence, status, input_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'queued', ?7, ?8, ?8)",
                params![
                    task.id.as_str(),
                    task.team_id.as_str(),
                    task.owner_instance_id.as_str(),
                    task.origin_instance_id.map(AgentInstanceId::as_str),
                    task.parent_task_id.map(AgentTaskId::as_str),
                    sequence,
                    task.input_json,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        self.agent_task(task.id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "created agent task was not found".to_string(),
            })
    }

    pub fn agent_task(
        &self,
        task_id: &AgentTaskId,
    ) -> Result<Option<AgentTaskRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                AGENT_TASK_SELECT_BY_ID,
                params![task_id.as_str()],
                agent_task_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn agent_tasks_for_team(
        &self,
        team_id: &AgentTeamId,
    ) -> Result<Vec<AgentTaskRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, team_id, owner_instance_id, origin_instance_id, parent_task_id,
                        sequence, status, input_json, result_json, error_json, created_at,
                        updated_at, started_at, completed_at
                 FROM agent_tasks WHERE team_id = ?1
                 ORDER BY owner_instance_id, sequence",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![team_id.as_str()], agent_task_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn agent_tasks_for_parent(
        &self,
        team_id: &AgentTeamId,
        parent_task_id: &AgentTaskId,
    ) -> Result<Vec<AgentTaskRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, team_id, owner_instance_id, origin_instance_id, parent_task_id,
                        sequence, status, input_json, result_json, error_json, created_at,
                        updated_at, started_at, completed_at
                 FROM agent_tasks
                 WHERE team_id = ?1 AND parent_task_id = ?2
                 ORDER BY created_at, id",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(
                params![team_id.as_str(), parent_task_id.as_str()],
                agent_task_from_row,
            )
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn agent_task_for_queued_user_message(
        &self,
        team_id: &AgentTeamId,
        user_message_id: &str,
    ) -> Result<Option<AgentTaskRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, team_id, owner_instance_id, origin_instance_id, parent_task_id,
                        sequence, status, input_json, result_json, error_json, created_at,
                        updated_at, started_at, completed_at
                 FROM agent_tasks
                 WHERE team_id = ?1
                   AND json_extract(input_json, '$.queuedUserMessageId') = ?2
                 ORDER BY sequence DESC
                 LIMIT 1",
                params![team_id.as_str(), user_message_id],
                agent_task_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn agent_task_for_team(
        &self,
        team_id: &AgentTeamId,
        task_id: &AgentTaskId,
    ) -> Result<Option<AgentTaskRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, team_id, owner_instance_id, origin_instance_id, parent_task_id,
                        sequence, status, input_json, result_json, error_json, created_at,
                        updated_at, started_at, completed_at
                 FROM agent_tasks WHERE team_id = ?1 AND id = ?2",
                params![team_id.as_str(), task_id.as_str()],
                agent_task_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn cancel_queued_agent_task(
        &mut self,
        team_id: &AgentTeamId,
        task_id: &AgentTaskId,
        error_json: &str,
    ) -> Result<bool, WorkspaceDatabaseError> {
        validate_agent_json(error_json, "error_json")?;
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE agent_tasks
                 SET status = 'cancelled', error_json = ?3, completed_at = ?4, updated_at = ?4
                 WHERE team_id = ?1 AND id = ?2 AND status = 'queued'",
                params![team_id.as_str(), task_id.as_str(), error_json, now],
            )
            .map(|updated| updated == 1)
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn transfer_queued_agent_task_with_limits(
        &mut self,
        team_id: &AgentTeamId,
        task_id: &AgentTaskId,
        target_instance_id: &AgentInstanceId,
        max_team_queued: i64,
        max_instance_queued: i64,
        max_chat_queued: i64,
    ) -> Result<Option<AgentTaskRecord>, WorkspaceDatabaseError> {
        if max_team_queued <= 0 || max_instance_queued <= 0 || max_chat_queued <= 0 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "Agent queued task limits must be greater than 0".to_string(),
            });
        }
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let task_state = transaction
            .query_row(
                "SELECT task.owner_instance_id, task.status, team.status, target.status
                 FROM agent_tasks AS task
                 JOIN agent_teams AS team ON team.id = task.team_id
                 JOIN agent_instances AS target ON target.team_id = task.team_id
                 WHERE task.team_id = ?1 AND task.id = ?2 AND target.id = ?3",
                params![
                    team_id.as_str(),
                    task_id.as_str(),
                    target_instance_id.as_str()
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?;
        let Some((owner_instance_id, task_status, team_status, target_status)) = task_state else {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(None);
        };
        if task_status != AgentTaskStatus::Queued.as_str() {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent task '{task_id}' cannot be transferred while {task_status}"
                ),
            });
        }
        if !matches!(team_status.as_str(), "active" | "draining") {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent team '{team_id}' does not accept transfers while {team_status}"
                ),
            });
        }
        if !matches!(target_status.as_str(), "idle" | "running") {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent instance '{target_instance_id}' does not accept transferred tasks while {target_status}"
                ),
            });
        }
        if owner_instance_id == target_instance_id.as_str() {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return self.agent_task(task_id);
        }
        let target_queued: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM agent_tasks
                 WHERE owner_instance_id = ?1 AND status = 'queued'",
                params![target_instance_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if target_queued >= max_instance_queued {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "Agent task queue is full for instance '{target_instance_id}' ({target_queued} queued)"
                ),
            });
        }
        let sequence: i64 = transaction
            .query_row(
                "SELECT next_task_sequence FROM agent_instances
                 WHERE id = ?1 AND team_id = ?2",
                params![target_instance_id.as_str(), team_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "UPDATE agent_instances
                 SET next_task_sequence = next_task_sequence + 1, updated_at = ?3
                 WHERE id = ?1 AND team_id = ?2",
                params![target_instance_id.as_str(), team_id.as_str(), now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let updated = transaction
            .execute(
                "UPDATE agent_tasks
                 SET owner_instance_id = ?3, sequence = ?4, updated_at = ?5
                 WHERE team_id = ?1 AND id = ?2 AND status = 'queued'",
                params![
                    team_id.as_str(),
                    task_id.as_str(),
                    target_instance_id.as_str(),
                    sequence,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        if updated == 0 {
            return Ok(None);
        }
        self.agent_task(task_id)
    }

    pub fn resume_satisfied_agent_tasks(
        &mut self,
        limit: i64,
    ) -> Result<Vec<AgentTaskRecord>, WorkspaceDatabaseError> {
        if limit <= 0 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "waiting Agent task resume limit must be greater than 0".to_string(),
            });
        }
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let task_ids = {
            let mut statement = transaction
                .prepare(
                    "SELECT task.id, task.team_id, task.owner_instance_id
                     FROM agent_tasks AS task
                     JOIN agent_instances AS instance ON instance.id = task.owner_instance_id
                     JOIN agent_teams AS team ON team.id = task.team_id
                     WHERE task.status = 'waiting'
                       AND instance.status IN ('waiting', 'draining')
                       AND team.status IN ('active', 'draining')
                       AND EXISTS (
                            SELECT 1 FROM agent_task_dependencies AS dependency
                            WHERE dependency.waiting_task_id = task.id
                       )
                       AND (
                            EXISTS (
                                SELECT 1 FROM agent_task_dependencies AS dependency
                                WHERE dependency.waiting_task_id = task.id
                                  AND dependency.deadline_at IS NOT NULL
                                  AND dependency.deadline_at <= ?1
                            )
                            OR (
                                EXISTS (
                                    SELECT 1 FROM agent_task_dependencies AS dependency
                                    WHERE dependency.waiting_task_id = task.id
                                      AND dependency.wait_mode = 'all'
                                )
                                AND NOT EXISTS (
                                    SELECT 1
                                    FROM agent_task_dependencies AS dependency
                                    JOIN agent_tasks AS required_task
                                      ON required_task.id = dependency.dependency_task_id
                                    WHERE dependency.waiting_task_id = task.id
                                      AND required_task.status NOT IN ('completed', 'failed', 'cancelled', 'interrupted')
                                )
                            )
                            OR EXISTS (
                                SELECT 1
                                FROM agent_task_dependencies AS dependency
                                JOIN agent_tasks AS required_task
                                  ON required_task.id = dependency.dependency_task_id
                                WHERE dependency.waiting_task_id = task.id
                                  AND dependency.wait_mode = 'any'
                                  AND required_task.status IN ('completed', 'failed', 'cancelled', 'interrupted')
                            )
                       )
                     ORDER BY task.created_at, task.team_id, task.owner_instance_id, task.sequence
                     LIMIT ?2",
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            let rows = statement
                .query_map(params![now.as_str(), limit], |row| {
                    Ok((
                        agent_id_from_row::<AgentTaskId>(row, 0)?,
                        agent_id_from_row::<AgentTeamId>(row, 1)?,
                        agent_id_from_row::<AgentInstanceId>(row, 2)?,
                    ))
                })
                .map_err(|source| sqlite_error(&database_path, source))?;
            collect_rows(rows, &database_path)?
        };

        for (task_id, team_id, owner_instance_id) in &task_ids {
            transaction
                .execute(
                    "UPDATE agent_tasks
                     SET status = 'queued', updated_at = ?3
                     WHERE id = ?1 AND team_id = ?2 AND status = 'waiting'",
                    params![task_id.as_str(), team_id.as_str(), now.as_str()],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            transaction
                .execute(
                    "UPDATE agent_instances
                     SET status = CASE WHEN status = 'draining' THEN 'draining' ELSE 'idle' END,
                         updated_at = ?3
                     WHERE id = ?1 AND team_id = ?2 AND status IN ('waiting', 'draining')",
                    params![owner_instance_id.as_str(), team_id.as_str(), now.as_str()],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        let mut tasks = Vec::with_capacity(task_ids.len());
        for (task_id, _, _) in task_ids {
            if let Some(task) = self.agent_task(&task_id)? {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    pub fn runnable_agent_tasks(
        &self,
        limit: i64,
    ) -> Result<Vec<AgentTaskRecord>, WorkspaceDatabaseError> {
        if limit <= 0 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "runnable Agent task query limit must be greater than 0".to_string(),
            });
        }
        let now = now_timestamp();
        let mut statement = self
            .connection
            .prepare(RUNNABLE_AGENT_TASKS_SQL)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![now, limit], agent_task_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn claim_runnable_agent_task(
        &mut self,
        team_id: &AgentTeamId,
        task_id: &AgentTaskId,
        attempt_id: &AgentAttemptId,
    ) -> Result<Option<AgentTaskRecord>, WorkspaceDatabaseError> {
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let runnable = transaction
            .query_row(
                "SELECT task.owner_instance_id
                 FROM agent_tasks AS task
                 JOIN agent_instances AS instance ON instance.id = task.owner_instance_id
                 JOIN agent_teams AS team ON team.id = task.team_id
                 WHERE task.id = ?1 AND task.team_id = ?2 AND task.status = 'queued'
                   AND instance.status IN ('idle', 'draining')
                   AND team.status IN ('active', 'draining')
                   AND (
                        SELECT COUNT(*)
                        FROM agent_tasks AS running_task
                        WHERE running_task.team_id = task.team_id
                          AND running_task.status = 'running'
                   ) < team.max_concurrent_runs
                   AND NOT EXISTS (
                        SELECT 1 FROM agent_tasks AS earlier_task
                        WHERE earlier_task.owner_instance_id = task.owner_instance_id
                          AND earlier_task.sequence < task.sequence
                          AND earlier_task.status IN ('queued', 'running', 'waiting')
                   )
                   AND (
                        json_extract(task.input_json, '$.deferUntilWorkspaceIdle') IS NOT 1
                        OR NOT EXISTS (
                            SELECT 1 FROM agent_tasks AS earlier_workspace_task
                            WHERE earlier_workspace_task.rowid < task.rowid
                              AND earlier_workspace_task.status IN ('queued', 'running', 'waiting')
                              AND COALESCE(json_extract(earlier_workspace_task.input_json, '$.sessionMode'), '') <> 'plan'
                        )
                   )
                   AND (
                        NOT EXISTS (
                            SELECT 1 FROM agent_task_dependencies AS dependency
                            WHERE dependency.waiting_task_id = task.id
                        )
                        OR (
                            EXISTS (
                                SELECT 1 FROM agent_task_dependencies AS dependency
                                WHERE dependency.waiting_task_id = task.id
                                  AND dependency.wait_mode = 'all'
                            )
                            AND NOT EXISTS (
                                SELECT 1
                                FROM agent_task_dependencies AS dependency
                                JOIN agent_tasks AS required_task
                                  ON required_task.id = dependency.dependency_task_id
                                WHERE dependency.waiting_task_id = task.id
                                  AND required_task.status NOT IN ('completed', 'failed', 'cancelled', 'interrupted')
                            )
                        )
                        OR EXISTS (
                            SELECT 1
                            FROM agent_task_dependencies AS dependency
                            JOIN agent_tasks AS required_task
                              ON required_task.id = dependency.dependency_task_id
                            WHERE dependency.waiting_task_id = task.id
                              AND dependency.wait_mode = 'any'
                              AND required_task.status IN ('completed', 'failed', 'cancelled', 'interrupted')
                        )
                        OR EXISTS (
                            SELECT 1 FROM agent_task_dependencies AS dependency
                            WHERE dependency.waiting_task_id = task.id
                              AND dependency.deadline_at IS NOT NULL
                              AND dependency.deadline_at <= ?3
                        )
                   )",
                params![task_id.as_str(), team_id.as_str(), now],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?;
        let Some(owner_instance_id) = runnable else {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(None);
        };
        let attempt_sequence: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(sequence), -1) + 1 FROM agent_attempts WHERE task_id = ?1",
                params![task_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let updated = transaction
            .execute(
                "UPDATE agent_tasks
                 SET status = 'running', started_at = COALESCE(started_at, ?3),
                     completed_at = NULL, updated_at = ?3
                 WHERE id = ?1 AND team_id = ?2 AND status = 'queued'",
                params![task_id.as_str(), team_id.as_str(), now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if updated != 1 {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(None);
        }
        transaction
            .execute(
                "INSERT INTO agent_attempts
                    (id, team_id, task_id, sequence, status, started_at)
                 VALUES (?1, ?2, ?3, ?4, 'running', ?5)",
                params![
                    attempt_id.as_str(),
                    team_id.as_str(),
                    task_id.as_str(),
                    attempt_sequence,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let instance_updated = transaction
            .execute(
                "UPDATE agent_instances
                 SET status = CASE WHEN status = 'draining' THEN 'draining' ELSE 'running' END,
                     last_scheduled_at = ?3, updated_at = ?3
                 WHERE id = ?1 AND team_id = ?2 AND status IN ('idle', 'draining')",
                params![owner_instance_id, team_id.as_str(), now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if instance_updated != 1 {
            return Err(WorkspaceDatabaseError::AgentDomain {
                source: AgentDomainError::queue_conflict(
                    AgentInstanceId::new(owner_instance_id)
                        .map_err(|source| WorkspaceDatabaseError::AgentDomain { source })?,
                ),
            });
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        self.agent_task(task_id)
    }

    pub fn update_agent_task_state(
        &mut self,
        update: AgentTaskStateUpdate<'_>,
    ) -> Result<bool, WorkspaceDatabaseError> {
        self.update_agent_task_state_inner(update, None)
    }

    pub fn update_agent_task_state_for_attempt(
        &mut self,
        update: AgentTaskStateUpdate<'_>,
        expected_attempt_id: &AgentAttemptId,
    ) -> Result<bool, WorkspaceDatabaseError> {
        self.update_agent_task_state_inner(update, Some(expected_attempt_id))
    }

    fn update_agent_task_state_inner(
        &mut self,
        update: AgentTaskStateUpdate<'_>,
        expected_attempt_id: Option<&AgentAttemptId>,
    ) -> Result<bool, WorkspaceDatabaseError> {
        if update.transition == AgentTaskTransition::Start {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "queued tasks must be started through claim_runnable_agent_task"
                    .to_string(),
            });
        }
        let target_status = update
            .expected_status
            .apply(update.transition)
            .map_err(|source| WorkspaceDatabaseError::AgentDomain { source })?;
        if let Some(result_json) = update.result_json {
            validate_agent_json(result_json, "result_json")?;
        }
        if let Some(error_json) = update.error_json {
            validate_agent_json(error_json, "error_json")?;
        }

        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let owner_instance_id = transaction
            .query_row(
                "SELECT owner_instance_id FROM agent_tasks
                 WHERE id = ?1 AND team_id = ?2 AND status = ?3
                   AND (
                       ?4 IS NULL
                       OR EXISTS (
                           SELECT 1 FROM agent_attempts
                           WHERE id = ?4 AND task_id = ?1 AND team_id = ?2
                             AND status = CASE WHEN ?3 = 'waiting' THEN 'suspended' ELSE 'running' END
                       )
                   )",
                params![
                    update.task_id.as_str(),
                    update.team_id.as_str(),
                    update.expected_status.as_str(),
                    expected_attempt_id.map(AgentAttemptId::as_str)
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?;
        let Some(owner_instance_id) = owner_instance_id else {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(false);
        };
        let completed_at = target_status.is_terminal().then_some(now.as_str());
        let updated = transaction
            .execute(
                "UPDATE agent_tasks
                 SET status = ?4,
                     result_json = CASE WHEN ?8 THEN result_json ELSE ?5 END,
                     error_json = CASE WHEN ?8 THEN error_json ELSE ?6 END,
                     started_at = CASE WHEN ?8 THEN NULL ELSE started_at END,
                     completed_at = CASE WHEN ?8 THEN completed_at ELSE ?7 END,
                     updated_at = ?9
                 WHERE id = ?1 AND team_id = ?2 AND status = ?3",
                params![
                    update.task_id.as_str(),
                    update.team_id.as_str(),
                    update.expected_status.as_str(),
                    target_status.as_str(),
                    update.result_json,
                    update.error_json,
                    completed_at,
                    update.transition == AgentTaskTransition::Retry,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if updated != 1 {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(false);
        }

        let attempt_target = match update.transition {
            AgentTaskTransition::Wait => Some(AgentAttemptStatus::Suspended),
            AgentTaskTransition::Resume => Some(AgentAttemptStatus::Running),
            AgentTaskTransition::Complete => Some(AgentAttemptStatus::Completed),
            AgentTaskTransition::Fail => Some(AgentAttemptStatus::Failed),
            AgentTaskTransition::Cancel
                if matches!(
                    update.expected_status,
                    AgentTaskStatus::Running | AgentTaskStatus::Waiting
                ) =>
            {
                Some(AgentAttemptStatus::Cancelled)
            }
            AgentTaskTransition::Interrupt => Some(AgentAttemptStatus::Interrupted),
            AgentTaskTransition::Start
            | AgentTaskTransition::Cancel
            | AgentTaskTransition::Retry => None,
        };
        if let Some(attempt_target) = attempt_target {
            let attempt_completed_at = attempt_target.is_terminal().then_some(now.as_str());
            let source_attempt_status = match update.expected_status {
                AgentTaskStatus::Waiting => "suspended",
                _ => "running",
            };
            let attempt_updated = transaction
                .execute(
                    "UPDATE agent_attempts
                     SET status = ?3, completed_at = ?4, interruption_reason = ?5
                     WHERE task_id = ?1 AND team_id = ?2
                       AND status = ?6
                       AND (?7 IS NULL OR id = ?7)",
                    params![
                        update.task_id.as_str(),
                        update.team_id.as_str(),
                        attempt_target.as_str(),
                        attempt_completed_at,
                        update.interruption_reason,
                        source_attempt_status,
                        expected_attempt_id.map(AgentAttemptId::as_str)
                    ],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            if attempt_updated != 1 {
                return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                    message: format!(
                        "task '{}' has no active attempt for transition {:?}",
                        update.task_id, update.transition
                    ),
                });
            }
        }

        if matches!(
            update.transition,
            AgentTaskTransition::Cancel | AgentTaskTransition::Retry
        ) {
            transaction
                .execute(
                    "DELETE FROM agent_task_dependencies WHERE waiting_task_id = ?1",
                    params![update.task_id.as_str()],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }

        let instance_status = match (update.expected_status, target_status) {
            (AgentTaskStatus::Queued, _) => None,
            (_, AgentTaskStatus::Running) => Some(AgentInstanceStatus::Running),
            (_, AgentTaskStatus::Waiting) => Some(AgentInstanceStatus::Waiting),
            (_, status) if status.is_terminal() => Some(AgentInstanceStatus::Idle),
            _ => None,
        };
        if let Some(instance_status) = instance_status {
            transaction
                .execute(
                    "UPDATE agent_instances
                 SET status = CASE
                         WHEN status = 'draining' AND ?3 = 'idle' THEN 'draining'
                         ELSE ?3
                     END,
                     updated_at = ?4
                 WHERE id = ?1 AND team_id = ?2",
                    params![
                        owner_instance_id,
                        update.team_id.as_str(),
                        instance_status.as_str(),
                        now
                    ],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        Ok(true)
    }

    pub fn agent_attempts_for_task(
        &self,
        task_id: &AgentTaskId,
    ) -> Result<Vec<AgentAttemptRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, team_id, task_id, sequence, status, started_at,
                        completed_at, interruption_reason
                 FROM agent_attempts WHERE task_id = ?1 ORDER BY sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![task_id.as_str()], |row| {
                Ok(AgentAttemptRecord {
                    id: agent_id_from_row(row, 0)?,
                    team_id: agent_id_from_row(row, 1)?,
                    task_id: agent_id_from_row(row, 2)?,
                    sequence: row.get(3)?,
                    status: agent_enum_from_row(row, 4)?,
                    started_at: row.get(5)?,
                    completed_at: row.get(6)?,
                    interruption_reason: row.get(7)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn insert_agent_message(
        &mut self,
        message: NewAgentMessage<'_>,
    ) -> Result<AgentMessageRecord, WorkspaceDatabaseError> {
        if message.content.trim().is_empty() {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "agent message content must not be empty".to_string(),
            });
        }
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        ensure_agent_entity_team(
            &transaction,
            "agent_instances",
            message.receiver_instance_id.as_str(),
            message.team_id,
            AgentEntityKind::Instance,
            &database_path,
        )?;
        if let Some(sender_instance_id) = message.sender_instance_id {
            ensure_agent_entity_team(
                &transaction,
                "agent_instances",
                sender_instance_id.as_str(),
                message.team_id,
                AgentEntityKind::Instance,
                &database_path,
            )?;
        }
        if let Some(related_task_id) = message.related_task_id {
            ensure_agent_entity_team(
                &transaction,
                "agent_tasks",
                related_task_id.as_str(),
                message.team_id,
                AgentEntityKind::Task,
                &database_path,
            )?;
        }
        if let Some(reply_to_message_id) = message.reply_to_message_id {
            ensure_agent_entity_team(
                &transaction,
                "agent_messages",
                reply_to_message_id.as_str(),
                message.team_id,
                AgentEntityKind::Message,
                &database_path,
            )?;
        }
        let sequence: i64 = transaction
            .query_row(
                "SELECT next_message_sequence FROM agent_instances
                 WHERE id = ?1 AND team_id = ?2",
                params![
                    message.receiver_instance_id.as_str(),
                    message.team_id.as_str()
                ],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "UPDATE agent_instances
                 SET next_message_sequence = next_message_sequence + 1, updated_at = ?3
                 WHERE id = ?1 AND team_id = ?2",
                params![
                    message.receiver_instance_id.as_str(),
                    message.team_id.as_str(),
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let content = redact_agent_text(message.content);
        transaction
            .execute(
                "INSERT INTO agent_messages
                    (id, team_id, sender_instance_id, receiver_instance_id, related_task_id,
                     reply_to_message_id, kind, content, sequence, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    message.id.as_str(),
                    message.team_id.as_str(),
                    message.sender_instance_id.map(AgentInstanceId::as_str),
                    message.receiver_instance_id.as_str(),
                    message.related_task_id.map(AgentTaskId::as_str),
                    message.reply_to_message_id.map(AgentMessageId::as_str),
                    message.kind.as_str(),
                    content,
                    sequence,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        self.agent_message(message.id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "created agent message was not found".to_string(),
            }
        })
    }

    pub fn agent_message(
        &self,
        message_id: &AgentMessageId,
    ) -> Result<Option<AgentMessageRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                AGENT_MESSAGE_SELECT_BY_ID,
                params![message_id.as_str()],
                agent_message_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn agent_messages_after(
        &self,
        receiver_instance_id: &AgentInstanceId,
        sequence: i64,
    ) -> Result<Vec<AgentMessageRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, team_id, sender_instance_id, receiver_instance_id,
                        related_task_id, reply_to_message_id, kind, content, sequence,
                        created_at, consumed_at
                 FROM agent_messages
                 WHERE receiver_instance_id = ?1 AND sequence > ?2
                 ORDER BY sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(
                params![receiver_instance_id.as_str(), sequence],
                agent_message_from_row,
            )
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn mark_agent_message_consumed(
        &mut self,
        message_id: &AgentMessageId,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE agent_messages SET consumed_at = ?2
                 WHERE id = ?1 AND consumed_at IS NULL",
                params![message_id.as_str(), now],
            )
            .map(|updated| updated == 1)
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn insert_agent_task_dependency(
        &mut self,
        dependency: NewAgentTaskDependency<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        ensure_agent_entity_team(
            &transaction,
            "agent_tasks",
            dependency.waiting_task_id.as_str(),
            dependency.team_id,
            AgentEntityKind::Task,
            &database_path,
        )?;
        ensure_agent_entity_team(
            &transaction,
            "agent_tasks",
            dependency.dependency_task_id.as_str(),
            dependency.team_id,
            AgentEntityKind::Task,
            &database_path,
        )?;
        if dependency.waiting_task_id == dependency.dependency_task_id
            || agent_dependency_path_exists(
                &transaction,
                dependency.team_id,
                dependency.dependency_task_id,
                dependency.waiting_task_id,
                &database_path,
            )?
        {
            return Err(WorkspaceDatabaseError::AgentDomain {
                source: AgentDomainError::dependency_cycle(dependency.waiting_task_id.clone()),
            });
        }
        let existing_mode = transaction
            .query_row(
                "SELECT wait_mode FROM agent_task_dependencies
                 WHERE team_id = ?1 AND waiting_task_id = ?2 LIMIT 1",
                params![
                    dependency.team_id.as_str(),
                    dependency.waiting_task_id.as_str()
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?;
        if existing_mode
            .as_deref()
            .is_some_and(|mode| mode != dependency.wait_mode.as_str())
        {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "all dependencies for a waiting task must use the same wait mode"
                    .to_string(),
            });
        }
        transaction
            .execute(
                "INSERT INTO agent_task_dependencies
                    (team_id, waiting_task_id, dependency_task_id, wait_mode,
                     pending_tool_call_id, deadline_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    dependency.team_id.as_str(),
                    dependency.waiting_task_id.as_str(),
                    dependency.dependency_task_id.as_str(),
                    dependency.wait_mode.as_str(),
                    dependency.pending_tool_call_id,
                    dependency.deadline_at,
                    now_timestamp()
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))
    }

    pub fn agent_task_dependencies(
        &self,
        waiting_task_id: &AgentTaskId,
    ) -> Result<Vec<AgentTaskDependencyRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT team_id, waiting_task_id, dependency_task_id, wait_mode,
                        pending_tool_call_id, deadline_at, created_at
                 FROM agent_task_dependencies
                 WHERE waiting_task_id = ?1
                 ORDER BY dependency_task_id ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![waiting_task_id.as_str()], |row| {
                Ok(AgentTaskDependencyRecord {
                    team_id: agent_id_from_row(row, 0)?,
                    waiting_task_id: agent_id_from_row(row, 1)?,
                    dependency_task_id: agent_id_from_row(row, 2)?,
                    wait_mode: agent_enum_from_row(row, 3)?,
                    pending_tool_call_id: row.get(4)?,
                    deadline_at: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn agent_task_dependencies_satisfied(
        &self,
        waiting_task_id: &AgentTaskId,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let now = now_timestamp();
        let (total, ready, expired, wait_mode): (i64, i64, i64, Option<String>) = self
            .connection
            .query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(CASE WHEN task.status IN ('completed', 'failed', 'cancelled', 'interrupted') THEN 1 ELSE 0 END), 0),
                        COALESCE(MAX(CASE WHEN dependency.deadline_at IS NOT NULL AND dependency.deadline_at <= ?2 THEN 1 ELSE 0 END), 0),
                        MIN(dependency.wait_mode)
                 FROM agent_task_dependencies AS dependency
                 JOIN agent_tasks AS task ON task.id = dependency.dependency_task_id
                 WHERE dependency.waiting_task_id = ?1",
                params![waiting_task_id.as_str(), now],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|source| self.sqlite_error(source))?;
        if total == 0 {
            return Ok(true);
        }
        if expired > 0 {
            return Ok(true);
        }
        Ok(match wait_mode.as_deref() {
            Some("all") => ready == total,
            Some("any") => ready > 0,
            _ => false,
        })
    }

    pub fn next_waiting_agent_task_dependency_deadline(
        &self,
    ) -> Result<Option<String>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT MIN(dependency.deadline_at)
                 FROM agent_task_dependencies AS dependency
                 JOIN agent_tasks AS task
                   ON task.team_id = dependency.team_id
                  AND task.id = dependency.waiting_task_id
                 JOIN agent_instances AS instance
                   ON instance.team_id = task.team_id
                  AND instance.id = task.owner_instance_id
                 JOIN agent_teams AS team ON team.id = task.team_id
                 WHERE task.status = 'waiting'
                   AND dependency.deadline_at IS NOT NULL
                   AND instance.status IN ('waiting', 'draining')
                   AND team.status IN ('active', 'draining')",
                [],
                |row| row.get(0),
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn delete_agent_task_dependencies(
        &mut self,
        waiting_task_id: &AgentTaskId,
    ) -> Result<usize, WorkspaceDatabaseError> {
        self.connection
            .execute(
                "DELETE FROM agent_task_dependencies WHERE waiting_task_id = ?1",
                params![waiting_task_id.as_str()],
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn suspend_running_agent_task_with_wait_dependencies(
        &mut self,
        team_id: &AgentTeamId,
        task_id: &AgentTaskId,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let owner_instance_id = transaction
            .query_row(
                "SELECT task.owner_instance_id
                 FROM agent_tasks AS task
                 WHERE task.id = ?1
                   AND task.team_id = ?2
                   AND task.status = 'running'
                   AND EXISTS (
                        SELECT 1 FROM agent_task_dependencies AS dependency
                        WHERE dependency.team_id = task.team_id
                          AND dependency.waiting_task_id = task.id
                   )",
                params![task_id.as_str(), team_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| sqlite_error(&database_path, source))?;
        let Some(owner_instance_id) = owner_instance_id else {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(false);
        };
        let updated = transaction
            .execute(
                "UPDATE agent_tasks
                 SET status = 'waiting', updated_at = ?3
                 WHERE id = ?1 AND team_id = ?2 AND status = 'running'",
                params![task_id.as_str(), team_id.as_str(), now.as_str()],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if updated != 1 {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(false);
        }
        let attempt_updated = transaction
            .execute(
                "UPDATE agent_attempts
                 SET status = 'suspended'
                 WHERE task_id = ?1 AND team_id = ?2 AND status = 'running'",
                params![task_id.as_str(), team_id.as_str()],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        if attempt_updated != 1 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!(
                    "task '{task_id}' has no running attempt to suspend during reconciliation"
                ),
            });
        }
        transaction
            .execute(
                "UPDATE agent_instances
                 SET status = CASE WHEN status = 'draining' THEN 'draining' ELSE 'waiting' END,
                     updated_at = ?3
                 WHERE id = ?1 AND team_id = ?2 AND status IN ('running', 'draining')",
                params![owner_instance_id, team_id.as_str(), now.as_str()],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        Ok(true)
    }

    pub fn recover_interrupted_agent_wait_tasks(
        &mut self,
        interruption_reason: &str,
        limit: i64,
    ) -> Result<Vec<AgentTaskRecord>, WorkspaceDatabaseError> {
        if limit <= 0 {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "interrupted Agent wait recovery limit must be greater than 0".to_string(),
            });
        }
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let task_ids = {
            let mut statement = transaction
                .prepare(
                    "SELECT DISTINCT task.id, task.team_id, task.owner_instance_id
                     FROM agent_tasks AS task
                     JOIN agent_attempts AS attempt
                       ON attempt.team_id = task.team_id
                      AND attempt.task_id = task.id
                     JOIN agent_instances AS instance
                       ON instance.team_id = task.team_id
                      AND instance.id = task.owner_instance_id
                     JOIN agent_teams AS team ON team.id = task.team_id
                     WHERE task.status = 'interrupted'
                       AND attempt.status = 'interrupted'
                       AND attempt.interruption_reason = ?1
                       AND instance.status IN ('paused', 'idle', 'waiting', 'draining')
                       AND team.status IN ('active', 'draining')
                       AND EXISTS (
                            SELECT 1 FROM agent_task_dependencies AS dependency
                            WHERE dependency.team_id = task.team_id
                              AND dependency.waiting_task_id = task.id
                       )
                       AND NOT EXISTS (
                            SELECT 1 FROM agent_tasks AS active_task
                            WHERE active_task.owner_instance_id = task.owner_instance_id
                              AND active_task.id <> task.id
                              AND active_task.status IN ('running', 'waiting')
                       )
                       AND NOT EXISTS (
                            SELECT 1
                            FROM agent_tasks AS earlier_task
                            JOIN agent_attempts AS earlier_attempt
                              ON earlier_attempt.team_id = earlier_task.team_id
                             AND earlier_attempt.task_id = earlier_task.id
                            WHERE earlier_task.owner_instance_id = task.owner_instance_id
                              AND earlier_task.status = 'interrupted'
                              AND earlier_attempt.status = 'interrupted'
                              AND earlier_attempt.interruption_reason = ?1
                              AND earlier_task.sequence < task.sequence
                              AND EXISTS (
                                    SELECT 1 FROM agent_task_dependencies AS dependency
                                    WHERE dependency.team_id = earlier_task.team_id
                                      AND dependency.waiting_task_id = earlier_task.id
                              )
                       )
                     ORDER BY task.updated_at, task.team_id, task.owner_instance_id, task.sequence
                     LIMIT ?2",
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            let rows = statement
                .query_map(params![interruption_reason, limit], |row| {
                    Ok((
                        agent_id_from_row::<AgentTaskId>(row, 0)?,
                        agent_id_from_row::<AgentTeamId>(row, 1)?,
                        agent_id_from_row::<AgentInstanceId>(row, 2)?,
                    ))
                })
                .map_err(|source| sqlite_error(&database_path, source))?;
            collect_rows(rows, &database_path)?
        };

        for (task_id, team_id, owner_instance_id) in &task_ids {
            transaction
                .execute(
                    "UPDATE agent_tasks
                     SET status = 'waiting', error_json = NULL, completed_at = NULL, updated_at = ?3
                     WHERE id = ?1 AND team_id = ?2 AND status = 'interrupted'",
                    params![task_id.as_str(), team_id.as_str(), now.as_str()],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            transaction
                .execute(
                    "UPDATE agent_attempts
                     SET status = 'suspended', completed_at = NULL, interruption_reason = NULL
                     WHERE task_id = ?1 AND team_id = ?2
                       AND status = 'interrupted'
                       AND interruption_reason = ?3",
                    params![task_id.as_str(), team_id.as_str(), interruption_reason],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            transaction
                .execute(
                    "UPDATE agent_instances
                     SET status = CASE WHEN status = 'draining' THEN 'draining' ELSE 'waiting' END,
                         updated_at = ?3
                     WHERE id = ?1 AND team_id = ?2
                       AND status IN ('paused', 'idle', 'waiting', 'draining')",
                    params![owner_instance_id.as_str(), team_id.as_str(), now.as_str()],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        let mut tasks = Vec::with_capacity(task_ids.len());
        for (task_id, _, _) in task_ids {
            if let Some(task) = self.agent_task(&task_id)? {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    pub fn append_agent_event(
        &mut self,
        event: NewAgentEvent<'_>,
    ) -> Result<AgentEventRecord, WorkspaceDatabaseError> {
        if event.event_type.trim().is_empty() {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "agent event type must not be empty".to_string(),
            });
        }
        let payload_json = redact_agent_json(event.payload_json, "payload_json")?;
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let sequence: i64 = transaction
            .query_row(
                "SELECT next_event_sequence FROM agent_teams WHERE id = ?1",
                params![event.team_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "UPDATE agent_teams
                 SET next_event_sequence = next_event_sequence + 1, updated_at = ?2
                 WHERE id = ?1",
                params![event.team_id.as_str(), now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "INSERT INTO agent_events
                    (team_id, sequence, event_type, instance_id, task_id, attempt_id,
                     message_id, payload_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    event.team_id.as_str(),
                    sequence,
                    event.event_type,
                    event.instance_id.map(AgentInstanceId::as_str),
                    event.task_id.map(AgentTaskId::as_str),
                    event.attempt_id.map(AgentAttemptId::as_str),
                    event.message_id.map(AgentMessageId::as_str),
                    payload_json,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        Ok(AgentEventRecord {
            team_id: event.team_id.clone(),
            sequence,
            event_type: event.event_type.to_string(),
            instance_id: event.instance_id.cloned(),
            task_id: event.task_id.cloned(),
            attempt_id: event.attempt_id.cloned(),
            message_id: event.message_id.cloned(),
            payload_json,
            created_at: now,
        })
    }

    pub fn agent_events_after(
        &self,
        team_id: &AgentTeamId,
        sequence: i64,
    ) -> Result<Vec<AgentEventRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT team_id, sequence, event_type, instance_id, task_id, attempt_id,
                        message_id, payload_json, created_at
                 FROM agent_events
                 WHERE team_id = ?1 AND sequence > ?2
                 ORDER BY sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![team_id.as_str(), sequence], |row| {
                Ok(AgentEventRecord {
                    team_id: agent_id_from_row(row, 0)?,
                    sequence: row.get(1)?,
                    event_type: row.get(2)?,
                    instance_id: optional_agent_id_from_row(row, 3)?,
                    task_id: optional_agent_id_from_row(row, 4)?,
                    attempt_id: optional_agent_id_from_row(row, 5)?,
                    message_id: optional_agent_id_from_row(row, 6)?,
                    payload_json: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn insert_agent_context_entry(
        &mut self,
        entry: NewAgentContextEntry<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        validate_agent_json(entry.content_json, "content_json")?;
        self.connection
            .execute(
                "INSERT INTO agent_context_entries
                    (id, team_id, instance_id, generation, sequence, role, content_json,
                     source_task_id, source_message_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    entry.id,
                    entry.team_id.as_str(),
                    entry.instance_id.as_str(),
                    entry.generation,
                    entry.sequence,
                    entry.role,
                    entry.content_json,
                    entry.source_task_id.map(AgentTaskId::as_str),
                    entry.source_message_id.map(AgentMessageId::as_str),
                    now_timestamp()
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        Ok(())
    }

    pub fn agent_context_entries(
        &self,
        instance_id: &AgentInstanceId,
        generation: i64,
        after_sequence: i64,
    ) -> Result<Vec<AgentContextEntryRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, team_id, instance_id, generation, sequence, role, content_json,
                        source_task_id, source_message_id, created_at
                 FROM agent_context_entries
                 WHERE instance_id = ?1 AND generation = ?2 AND sequence > ?3
                 ORDER BY sequence ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(
                params![instance_id.as_str(), generation, after_sequence],
                |row| {
                    Ok(AgentContextEntryRecord {
                        id: row.get(0)?,
                        team_id: agent_id_from_row(row, 1)?,
                        instance_id: agent_id_from_row(row, 2)?,
                        generation: row.get(3)?,
                        sequence: row.get(4)?,
                        role: row.get(5)?,
                        content_json: row.get(6)?,
                        source_task_id: optional_agent_id_from_row(row, 7)?,
                        source_message_id: optional_agent_id_from_row(row, 8)?,
                        created_at: row.get(9)?,
                    })
                },
            )
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn insert_agent_context_snapshot(
        &mut self,
        snapshot: NewAgentContextSnapshot<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        validate_agent_json(snapshot.entries_json, "entries_json")?;
        if snapshot
            .token_count
            .is_some_and(|token_count| token_count < 0)
        {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "agent context snapshot token_count must not be negative".to_string(),
            });
        }
        self.connection
            .execute(
                "INSERT INTO agent_context_snapshots
                    (id, team_id, instance_id, generation, sequence, entries_json,
                     token_count, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    snapshot.id,
                    snapshot.team_id.as_str(),
                    snapshot.instance_id.as_str(),
                    snapshot.generation,
                    snapshot.sequence,
                    snapshot.entries_json,
                    snapshot.token_count,
                    now_timestamp()
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        Ok(())
    }

    pub fn latest_agent_context_snapshot(
        &self,
        instance_id: &AgentInstanceId,
        generation: i64,
    ) -> Result<Option<AgentContextSnapshotRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, team_id, instance_id, generation, sequence, entries_json,
                        token_count, created_at
                 FROM agent_context_snapshots
                 WHERE instance_id = ?1 AND generation = ?2
                 ORDER BY sequence DESC LIMIT 1",
                params![instance_id.as_str(), generation],
                |row| {
                    Ok(AgentContextSnapshotRecord {
                        id: row.get(0)?,
                        team_id: agent_id_from_row(row, 1)?,
                        instance_id: agent_id_from_row(row, 2)?,
                        generation: row.get(3)?,
                        sequence: row.get(4)?,
                        entries_json: row.get(5)?,
                        token_count: row.get(6)?,
                        created_at: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn startup_agent_reconciliation(
        &self,
    ) -> Result<Vec<AgentReconciliationRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    attempt.id, attempt.team_id, attempt.task_id, attempt.sequence,
                    attempt.status, attempt.started_at, attempt.completed_at,
                    attempt.interruption_reason,
                    task.id, task.team_id, task.owner_instance_id, task.origin_instance_id,
                    task.parent_task_id, task.sequence, task.status, task.input_json,
                    task.result_json, task.error_json, task.created_at, task.updated_at,
                    task.started_at, task.completed_at
                 FROM agent_attempts AS attempt
                 JOIN agent_tasks AS task ON task.id = attempt.task_id
                 WHERE attempt.status IN ('running', 'suspended')
                    OR task.status IN ('running', 'waiting')
                 ORDER BY attempt.team_id, task.owner_instance_id, task.sequence",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], |row| {
                Ok(AgentReconciliationRecord {
                    attempt: AgentAttemptRecord {
                        id: agent_id_from_row(row, 0)?,
                        team_id: agent_id_from_row(row, 1)?,
                        task_id: agent_id_from_row(row, 2)?,
                        sequence: row.get(3)?,
                        status: agent_enum_from_row(row, 4)?,
                        started_at: row.get(5)?,
                        completed_at: row.get(6)?,
                        interruption_reason: row.get(7)?,
                    },
                    task: AgentTaskRecord {
                        id: agent_id_from_row(row, 8)?,
                        team_id: agent_id_from_row(row, 9)?,
                        owner_instance_id: agent_id_from_row(row, 10)?,
                        origin_instance_id: optional_agent_id_from_row(row, 11)?,
                        parent_task_id: optional_agent_id_from_row(row, 12)?,
                        sequence: row.get(13)?,
                        status: agent_enum_from_row(row, 14)?,
                        input_json: row.get(15)?,
                        result_json: row.get(16)?,
                        error_json: row.get(17)?,
                        created_at: row.get(18)?,
                        updated_at: row.get(19)?,
                        started_at: row.get(20)?,
                        completed_at: row.get(21)?,
                    },
                })
            })
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
    }

    pub fn code_graph_file_hash(
        &self,
        path: &str,
    ) -> Result<Option<String>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT code_graph_file_hashes.content_hash
                 FROM code_graph_file_hashes
                 JOIN code_graph_files
                    ON code_graph_files.id = code_graph_file_hashes.file_id
                 WHERE code_graph_files.path = ?1",
                params![path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    /// Load all indexed code-graph path → content-hash pairs for short permit holds.
    pub fn code_graph_file_hashes(
        &self,
    ) -> Result<HashMap<String, String>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT code_graph_files.path, code_graph_file_hashes.content_hash
                 FROM code_graph_file_hashes
                 JOIN code_graph_files
                    ON code_graph_files.id = code_graph_file_hashes.file_id",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|source| self.sqlite_error(source))?;
        let mut hashes = HashMap::new();
        for row in rows {
            let (path, hash) = row.map_err(|source| self.sqlite_error(source))?;
            hashes.insert(path, hash);
        }
        Ok(hashes)
    }

    pub fn replace_code_graph_file_index(
        &mut self,
        index: NewCodeGraphFileIndex<'_>,
    ) -> Result<i64, WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction =
            self.connection
                .transaction()
                .map_err(|source| WorkspaceDatabaseError::Sqlite {
                    path: database_path.clone(),
                    source,
                })?;
        let now = now_timestamp();

        transaction
            .execute(
                "INSERT INTO code_graph_files
                    (path, language, size_bytes, modified_at, discovered_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(path) DO UPDATE SET
                    language = excluded.language,
                    size_bytes = excluded.size_bytes,
                    modified_at = excluded.modified_at",
                params![
                    index.path,
                    index.language,
                    index.size_bytes,
                    index.modified_at,
                    now
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let file_id = code_graph_file_id(&transaction, &database_path, index.path)?;

        clear_code_graph_file_index(&transaction, &database_path, file_id, index.path)?;
        transaction
            .execute(
                "INSERT INTO code_graph_file_hashes (file_id, content_hash, hashed_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(file_id) DO UPDATE SET
                    content_hash = excluded.content_hash,
                    hashed_at = excluded.hashed_at",
                params![file_id, index.content_hash, now],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "INSERT INTO code_graph_parse_status
                    (file_id, status, parsed_at, error_message)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(file_id) DO UPDATE SET
                    status = excluded.status,
                    parsed_at = excluded.parsed_at,
                    error_message = excluded.error_message",
                params![file_id, index.parse_status, now, index.parse_error_message],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        let mut symbol_ids = Vec::with_capacity(index.symbols.len());
        {
            let mut insert_symbol = transaction
                .prepare(
                    "INSERT INTO code_graph_symbols
                        (
                            file_id, name, kind, start_line, start_column,
                            end_line, end_column, signature, documentation
                        )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            let mut insert_fts_data = transaction
                .prepare(
                    "INSERT INTO code_graph_fts_data
                        (entity_kind, entity_id, title, body, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            let mut insert_fts_index = transaction
                .prepare(
                    "INSERT INTO code_graph_fts_index (entity_kind, entity_id, title, body)
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .map_err(|source| sqlite_error(&database_path, source))?;

            for symbol in index.symbols {
                insert_symbol
                    .execute(params![
                        file_id,
                        symbol.name,
                        symbol.kind,
                        symbol.start_line,
                        symbol.start_column,
                        symbol.end_line,
                        symbol.end_column,
                        symbol.signature,
                        symbol.documentation
                    ])
                    .map_err(|source| sqlite_error(&database_path, source))?;
                let symbol_id = transaction.last_insert_rowid();
                let symbol_entity_id = symbol_id.to_string();
                symbol_ids.push(symbol_id);
                insert_code_graph_fts_entry(
                    &mut insert_fts_data,
                    &mut insert_fts_index,
                    &database_path,
                    "symbol",
                    &symbol_entity_id,
                    symbol.name,
                    symbol
                        .documentation
                        .or(symbol.signature)
                        .unwrap_or(symbol.name),
                    &now,
                )?;
            }

            insert_code_graph_fts_entry(
                &mut insert_fts_data,
                &mut insert_fts_index,
                &database_path,
                "file",
                index.path,
                index.path,
                index.fts_body,
                &now,
            )?;
        }

        {
            let mut insert_import = transaction
                .prepare(
                    "INSERT INTO code_graph_imports
                        (
                            file_id, module, imported_symbol, alias,
                            start_line, start_column
                        )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            for import in index.imports {
                insert_import
                    .execute(params![
                        file_id,
                        import.module,
                        import.imported_symbol,
                        import.alias,
                        import.start_line,
                        import.start_column
                    ])
                    .map_err(|source| sqlite_error(&database_path, source))?;
            }
        }

        {
            let mut insert_reference = transaction
                .prepare(
                    "INSERT INTO code_graph_references
                        (
                            file_id, symbol_id, name, start_line, start_column,
                            end_line, end_column
                        )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            for reference in index.references {
                let symbol_id = match reference.symbol_index {
                    Some(symbol_index) => Some(*symbol_ids.get(symbol_index).ok_or_else(|| {
                        WorkspaceDatabaseError::InvalidCodeGraphInput {
                            message: format!(
                                "reference points to missing symbol index {symbol_index}"
                            ),
                        }
                    })?),
                    None => None,
                };
                insert_reference
                    .execute(params![
                        file_id,
                        symbol_id,
                        reference.name,
                        reference.start_line,
                        reference.start_column,
                        reference.end_line,
                        reference.end_column
                    ])
                    .map_err(|source| sqlite_error(&database_path, source))?;
            }
        }

        {
            let mut insert_edge = transaction
                .prepare(
                    "INSERT INTO code_graph_edges
                        (
                            source_symbol_id, target_symbol_id,
                            edge_kind, metadata_json
                        )
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            for edge in index.edges {
                let source_symbol_id =
                    *symbol_ids.get(edge.source_symbol_index).ok_or_else(|| {
                        WorkspaceDatabaseError::InvalidCodeGraphInput {
                            message: format!(
                                "edge source points to missing symbol index {}",
                                edge.source_symbol_index
                            ),
                        }
                    })?;
                let target_symbol_id =
                    *symbol_ids.get(edge.target_symbol_index).ok_or_else(|| {
                        WorkspaceDatabaseError::InvalidCodeGraphInput {
                            message: format!(
                                "edge target points to missing symbol index {}",
                                edge.target_symbol_index
                            ),
                        }
                    })?;
                insert_edge
                    .execute(params![
                        source_symbol_id,
                        target_symbol_id,
                        edge.edge_kind,
                        edge.metadata_json.unwrap_or("{}")
                    ])
                    .map_err(|source| sqlite_error(&database_path, source))?;
            }
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(file_id)
    }

    pub fn delete_code_graph_file(&mut self, path: &str) -> Result<bool, WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| WorkspaceDatabaseError::Sqlite {
                path: database_path.clone(),
                source,
            })?;
        let Some(file_id) = optional_code_graph_file_id(&transaction, &database_path, path)? else {
            transaction
                .commit()
                .map_err(|source| sqlite_error(&database_path, source))?;
            return Ok(false);
        };

        clear_code_graph_file_index(&transaction, &database_path, file_id, path)?;
        transaction
            .execute(
                "DELETE FROM code_graph_parse_status WHERE file_id = ?1",
                params![file_id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "DELETE FROM code_graph_file_hashes WHERE file_id = ?1",
                params![file_id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .execute(
                "DELETE FROM code_graph_files WHERE id = ?1",
                params![file_id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(true)
    }

    pub fn remove_stale_code_graph_files(
        &mut self,
        live_paths: &[String],
    ) -> Result<Vec<String>, WorkspaceDatabaseError> {
        let live_paths = live_paths
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let indexed_paths = {
            let mut statement = self
                .connection
                .prepare("SELECT path FROM code_graph_files ORDER BY path ASC")
                .map_err(|source| self.sqlite_error(source))?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|source| self.sqlite_error(source))?;

            collect_rows(rows, &self.database_path)?
        };
        let stale_paths = indexed_paths
            .into_iter()
            .filter(|path| !live_paths.contains(path.as_str()))
            .collect::<Vec<_>>();

        for path in &stale_paths {
            self.delete_code_graph_file(path)?;
        }

        Ok(stale_paths)
    }

    pub fn code_graph_context(&self) -> Result<CodeGraphContextRecord, WorkspaceDatabaseError> {
        let indexed_files = self
            .connection
            .query_row("SELECT COUNT(*) FROM code_graph_files", [], |row| {
                row.get(0)
            })
            .map_err(|source| self.sqlite_error(source))?;
        let symbols = self
            .connection
            .query_row("SELECT COUNT(*) FROM code_graph_symbols", [], |row| {
                row.get(0)
            })
            .map_err(|source| self.sqlite_error(source))?;
        let references = self
            .connection
            .query_row("SELECT COUNT(*) FROM code_graph_references", [], |row| {
                row.get(0)
            })
            .map_err(|source| self.sqlite_error(source))?;
        let edges = self
            .connection
            .query_row("SELECT COUNT(*) FROM code_graph_edges", [], |row| {
                row.get(0)
            })
            .map_err(|source| self.sqlite_error(source))?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT language
                 FROM code_graph_files
                 WHERE language IS NOT NULL
                 GROUP BY language
                 ORDER BY language ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| self.sqlite_error(source))?;

        Ok(CodeGraphContextRecord {
            indexed_files,
            symbols,
            references,
            edges,
            languages: collect_rows(rows, &self.database_path)?,
        })
    }

    pub fn code_graph_file_summaries(
        &self,
        limit: i64,
    ) -> Result<Vec<CodeGraphFileSummaryRecord>, WorkspaceDatabaseError> {
        if limit <= 0 {
            return Err(WorkspaceDatabaseError::InvalidCodeGraphInput {
                message: "code graph file summary limit must be positive".to_string(),
            });
        }
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    file.path,
                    file.language,
                    COUNT(DISTINCT symbol.id) AS symbol_count,
                    COUNT(DISTINCT imp.id) AS import_count,
                    COALESCE(GROUP_CONCAT(DISTINCT imp.module), '') AS import_modules
                 FROM code_graph_files file
                 LEFT JOIN code_graph_symbols symbol ON symbol.file_id = file.id
                 LEFT JOIN code_graph_imports imp ON imp.file_id = file.id
                 GROUP BY file.id, file.path, file.language
                 ORDER BY symbol_count DESC, import_count DESC, file.path ASC
                 LIMIT ?1",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![limit], code_graph_file_summary_from_row)
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn find_code_graph_symbols(
        &self,
        query: &str,
        kind: Option<&str>,
        path: Option<&str>,
        limit: i64,
    ) -> Result<Vec<CodeGraphSymbolRecord>, WorkspaceDatabaseError> {
        let query_like = format!("%{}%", query.trim().to_ascii_lowercase());
        let kind = kind.map(str::trim).filter(|value| !value.is_empty());
        let path = path.map(str::trim).filter(|value| !value.is_empty());
        let path_prefix = path.map(|value| format!("{value}/%"));
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    s.id, f.path, f.language, s.name, s.kind,
                    s.start_line, s.start_column, s.end_line, s.end_column,
                    s.signature, s.documentation
                 FROM code_graph_symbols s
                 JOIN code_graph_files f ON f.id = s.file_id
                 WHERE
                    (
                        lower(s.name) LIKE ?1
                        OR lower(COALESCE(s.signature, '')) LIKE ?1
                        OR lower(COALESCE(s.documentation, '')) LIKE ?1
                    )
                    AND (?2 IS NULL OR s.kind = ?2)
                    AND (?3 IS NULL OR f.path = ?3 OR f.path LIKE ?4)
                 ORDER BY
                    CASE WHEN lower(s.name) = lower(?5) THEN 0 ELSE 1 END,
                    f.path ASC,
                    s.start_line ASC,
                    s.name ASC
                 LIMIT ?6",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(
                params![query_like, kind, path, path_prefix, query.trim(), limit],
                code_graph_symbol_from_row,
            )
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn code_graph_symbol(
        &self,
        symbol_id: i64,
    ) -> Result<Option<CodeGraphSymbolRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT
                    s.id, f.path, f.language, s.name, s.kind,
                    s.start_line, s.start_column, s.end_line, s.end_column,
                    s.signature, s.documentation
                 FROM code_graph_symbols s
                 JOIN code_graph_files f ON f.id = s.file_id
                 WHERE s.id = ?1",
                params![symbol_id],
                code_graph_symbol_from_row,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn code_graph_callers(
        &self,
        symbol_id: i64,
        limit: i64,
    ) -> Result<Vec<CodeGraphSymbolRelationRecord>, WorkspaceDatabaseError> {
        self.code_graph_symbol_relations(
            "WHERE edge.target_symbol_id = ?1",
            params![symbol_id, limit],
        )
    }

    pub fn code_graph_callees(
        &self,
        symbol_id: i64,
        limit: i64,
    ) -> Result<Vec<CodeGraphSymbolRelationRecord>, WorkspaceDatabaseError> {
        self.code_graph_symbol_relations(
            "WHERE edge.source_symbol_id = ?1",
            params![symbol_id, limit],
        )
    }

    pub fn code_graph_references(
        &self,
        symbol_id: i64,
        limit: i64,
    ) -> Result<Vec<CodeGraphReferenceRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    reference.id, file.path, file.language, reference.name,
                    reference.start_line, reference.start_column,
                    reference.end_line, reference.end_column,
                    symbol.id, symbol_file.path, symbol_file.language,
                    symbol.name, symbol.kind, symbol.start_line, symbol.start_column,
                    symbol.end_line, symbol.end_column, symbol.signature,
                    symbol.documentation
                 FROM code_graph_references reference
                 JOIN code_graph_files file ON file.id = reference.file_id
                 LEFT JOIN code_graph_symbols symbol ON symbol.id = reference.symbol_id
                 LEFT JOIN code_graph_files symbol_file ON symbol_file.id = symbol.file_id
                 WHERE reference.symbol_id = ?1
                 ORDER BY file.path ASC, reference.start_line ASC, reference.start_column ASC
                 LIMIT ?2",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![symbol_id, limit], code_graph_reference_from_row)
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn code_graph_related_files(
        &self,
        path: &str,
        limit: i64,
    ) -> Result<Vec<CodeGraphRelatedFileRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "WITH related AS (
                    SELECT target_file.path AS path, target_file.language AS language,
                           'callee' AS relation, COUNT(*) AS score
                    FROM code_graph_edges edge
                    JOIN code_graph_symbols source_symbol
                        ON source_symbol.id = edge.source_symbol_id
                    JOIN code_graph_files source_file
                        ON source_file.id = source_symbol.file_id
                    JOIN code_graph_symbols target_symbol
                        ON target_symbol.id = edge.target_symbol_id
                    JOIN code_graph_files target_file
                        ON target_file.id = target_symbol.file_id
                    WHERE source_file.path = ?1 AND target_file.path <> ?1
                    GROUP BY target_file.path, target_file.language

                    UNION ALL

                    SELECT source_file.path AS path, source_file.language AS language,
                           'caller' AS relation, COUNT(*) AS score
                    FROM code_graph_edges edge
                    JOIN code_graph_symbols source_symbol
                        ON source_symbol.id = edge.source_symbol_id
                    JOIN code_graph_files source_file
                        ON source_file.id = source_symbol.file_id
                    JOIN code_graph_symbols target_symbol
                        ON target_symbol.id = edge.target_symbol_id
                    JOIN code_graph_files target_file
                        ON target_file.id = target_symbol.file_id
                    WHERE target_file.path = ?1 AND source_file.path <> ?1
                    GROUP BY source_file.path, source_file.language

                    UNION ALL

                    SELECT other_file.path AS path, other_file.language AS language,
                           'shared_import' AS relation, COUNT(*) AS score
                    FROM code_graph_imports import
                    JOIN code_graph_files file ON file.id = import.file_id
                    JOIN code_graph_imports other_import
                        ON other_import.module = import.module
                    JOIN code_graph_files other_file
                        ON other_file.id = other_import.file_id
                    WHERE file.path = ?1 AND other_file.path <> ?1
                    GROUP BY other_file.path, other_file.language
                 )
                 SELECT path, language, relation, score
                 FROM related
                 ORDER BY score DESC, path ASC, relation ASC
                 LIMIT ?2",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![path, limit], |row| {
                Ok(CodeGraphRelatedFileRecord {
                    path: row.get(0)?,
                    language: row.get(1)?,
                    relation: row.get(2)?,
                    score: row.get(3)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn upsert_todo_graph(
        &mut self,
        chat_id: &str,
        tasks: Vec<TodoGraphTask>,
    ) -> Result<TodoGraphRecord, WorkspaceDatabaseError> {
        if self.chat(chat_id)?.is_none() {
            return Err(WorkspaceDatabaseError::InvalidTodoGraph {
                message: format!("chat was not found: {chat_id}"),
            });
        }

        let now = now_timestamp();
        let tasks = normalize_new_todo_graph_tasks(tasks, &now)?;
        let graph_json = serde_json::to_string(&tasks)
            .map_err(|source| WorkspaceDatabaseError::TodoGraphJson { source })?;

        self.connection
            .execute(
                "INSERT INTO todo_graphs
                    (chat_id, graph_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)
                 ON CONFLICT(chat_id) DO UPDATE SET
                    graph_json = excluded.graph_json,
                    updated_at = excluded.updated_at",
                params![chat_id, graph_json, now],
            )
            .map_err(|source| self.sqlite_error(source))?;

        self.todo_graph(chat_id)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidTodoGraph {
                message: format!("todo graph was not saved for chat: {chat_id}"),
            })
    }

    pub fn todo_graph(
        &self,
        chat_id: &str,
    ) -> Result<Option<TodoGraphRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT chat_id, graph_json, created_at, updated_at
                 FROM todo_graphs
                 WHERE chat_id = ?1",
                params![chat_id],
                |row| {
                    let graph_json: String = row.get(1)?;
                    let tasks = serde_json::from_str(&graph_json).map_err(|source| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(source),
                        )
                    })?;

                    Ok(TodoGraphRecord {
                        chat_id: row.get(0)?,
                        tasks,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                        updated_task: None,
                    })
                },
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn filtered_todo_graph(
        &self,
        chat_id: &str,
        filter: TodoGraphFilter<'_>,
    ) -> Result<Option<TodoGraphRecord>, WorkspaceDatabaseError> {
        let Some(mut graph) = self.todo_graph(chat_id)? else {
            return Ok(None);
        };

        graph.tasks = filter_todo_graph_tasks(graph.tasks, filter)?;

        Ok(Some(graph))
    }

    pub fn update_todo_graph_task(
        &mut self,
        chat_id: &str,
        task_id: &str,
        patch: TodoGraphTaskPatch,
    ) -> Result<TodoGraphRecord, WorkspaceDatabaseError> {
        let mut record =
            self.todo_graph(chat_id)?
                .ok_or_else(|| WorkspaceDatabaseError::MissingTodoGraph {
                    chat_id: chat_id.to_string(),
                })?;
        if task_id.trim().is_empty() {
            return Err(WorkspaceDatabaseError::InvalidTodoGraph {
                message: "task id must not be empty".to_string(),
            });
        }
        let now = now_timestamp();
        let updated_task = update_task_by_id(&mut record.tasks, task_id.trim(), &patch, &now)?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidTodoGraph {
                message: format!("task was not found: {}", task_id.trim()),
            })?;
        validate_todo_graph_tasks(&record.tasks)?;

        let graph_json = serde_json::to_string(&record.tasks)
            .map_err(|source| WorkspaceDatabaseError::TodoGraphJson { source })?;
        self.connection
            .execute(
                "UPDATE todo_graphs
                 SET graph_json = ?2, updated_at = ?3
                 WHERE chat_id = ?1",
                params![chat_id, graph_json, now],
            )
            .map_err(|source| self.sqlite_error(source))?;

        record.updated_at = now;
        record.updated_task = Some(updated_task);

        Ok(record)
    }

    fn code_graph_symbol_relations<P>(
        &self,
        where_clause: &str,
        params: P,
    ) -> Result<Vec<CodeGraphSymbolRelationRecord>, WorkspaceDatabaseError>
    where
        P: rusqlite::Params,
    {
        let sql = format!(
            "SELECT
                edge.id, edge.edge_kind, edge.metadata_json,
                source.id, source_file.path, source_file.language,
                source.name, source.kind, source.start_line, source.start_column,
                source.end_line, source.end_column, source.signature, source.documentation,
                target.id, target_file.path, target_file.language,
                target.name, target.kind, target.start_line, target.start_column,
                target.end_line, target.end_column, target.signature, target.documentation
             FROM code_graph_edges edge
             JOIN code_graph_symbols source ON source.id = edge.source_symbol_id
             JOIN code_graph_files source_file ON source_file.id = source.file_id
             JOIN code_graph_symbols target ON target.id = edge.target_symbol_id
             JOIN code_graph_files target_file ON target_file.id = target.file_id
             {where_clause}
             ORDER BY source_file.path ASC, source.start_line ASC,
                      target_file.path ASC, target.start_line ASC
             LIMIT ?2"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params, code_graph_relation_from_row)
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn upsert_terminal_session(
        &mut self,
        session: NewTerminalSession<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let now = now_timestamp();
        let metadata_json = session.metadata_json.unwrap_or("{}");

        self.connection
            .execute(
                "INSERT INTO terminal_sessions
                    (id, name, working_directory, created_at, updated_at, closed_at, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?4, NULL, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    working_directory = excluded.working_directory,
                    updated_at = excluded.updated_at,
                    closed_at = NULL,
                    metadata_json = excluded.metadata_json",
                params![
                    session.id,
                    session.name,
                    session.working_directory,
                    now,
                    metadata_json
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn latest_terminal_session(
        &self,
    ) -> Result<Option<TerminalSessionRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, name, working_directory, created_at, updated_at, closed_at, metadata_json
                 FROM terminal_sessions
                 WHERE closed_at IS NULL
                 ORDER BY updated_at DESC, created_at DESC, id DESC
                 LIMIT 1",
                [],
                |row| {
                    Ok(TerminalSessionRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        working_directory: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        closed_at: row.get(5)?,
                        metadata_json: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn latest_terminal_working_directory(
        &self,
    ) -> Result<Option<String>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT working_directory
                 FROM terminal_sessions
                 ORDER BY updated_at DESC, created_at DESC, id DESC
                 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn terminal_session(
        &self,
        id: &str,
    ) -> Result<Option<TerminalSessionRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, name, working_directory, created_at, updated_at, closed_at, metadata_json
                 FROM terminal_sessions
                 WHERE id = ?1",
                params![id],
                |row| {
                    Ok(TerminalSessionRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        working_directory: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        closed_at: row.get(5)?,
                        metadata_json: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn update_terminal_working_directory(
        &mut self,
        id: &str,
        working_directory: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        let updated = self
            .connection
            .execute(
                "UPDATE terminal_sessions
                 SET working_directory = ?2, updated_at = ?3
                 WHERE id = ?1",
                params![id, working_directory, now_timestamp()],
            )
            .map_err(|source| self.sqlite_error(source))?;

        if updated == 0 {
            return Err(WorkspaceDatabaseError::MissingTerminalSession { id: id.to_string() });
        }

        Ok(())
    }

    pub fn close_terminal_session(&mut self, id: &str) -> Result<(), WorkspaceDatabaseError> {
        let now = now_timestamp();
        let updated = self
            .connection
            .execute(
                "UPDATE terminal_sessions
                 SET updated_at = ?2, closed_at = ?2
                 WHERE id = ?1",
                params![id, now],
            )
            .map_err(|source| self.sqlite_error(source))?;

        if updated == 0 {
            return Err(WorkspaceDatabaseError::MissingTerminalSession { id: id.to_string() });
        }

        Ok(())
    }

    pub fn insert_hook_run(
        &mut self,
        hook_run: NewHookRun<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let input_json = redact_audit_json(hook_run.input_json, "hook_runs.input_json")?;
        let output_json =
            redact_optional_audit_json(hook_run.output_json, "hook_runs.output_json")?;

        self.connection
            .execute(
                "INSERT INTO hook_runs
                    (
                        id, workspace_id, chat_id, run_id, tool_call_id,
                        event, hook_source, handler_type, input_json, output_json,
                        status, exit_code, stdout_preview, stderr_preview,
                        started_at, completed_at
                    )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    hook_run.id,
                    hook_run.workspace_id,
                    hook_run.chat_id,
                    hook_run.run_id,
                    hook_run.tool_call_id,
                    hook_run.event,
                    hook_run.hook_source,
                    hook_run.handler_type,
                    input_json,
                    output_json,
                    hook_run.status,
                    hook_run.exit_code,
                    hook_run.stdout_preview,
                    hook_run.stderr_preview,
                    hook_run.started_at,
                    hook_run.completed_at
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn hook_runs(&self, limit: i64) -> Result<Vec<HookRunRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, workspace_id, chat_id, run_id, tool_call_id,
                        event, hook_source, handler_type, input_json, output_json,
                        status, exit_code, stdout_preview, stderr_preview,
                        started_at, completed_at
                 FROM hook_runs
                 ORDER BY started_at DESC, id DESC
                 LIMIT ?1",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(params![limit], |row| {
                Ok(HookRunRecord {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    chat_id: row.get(2)?,
                    run_id: row.get(3)?,
                    tool_call_id: row.get(4)?,
                    event: row.get(5)?,
                    hook_source: row.get(6)?,
                    handler_type: row.get(7)?,
                    input_json: row.get(8)?,
                    output_json: row.get(9)?,
                    status: row.get(10)?,
                    exit_code: row.get(11)?,
                    stdout_preview: row.get(12)?,
                    stderr_preview: row.get(13)?,
                    started_at: row.get(14)?,
                    completed_at: row.get(15)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn hook_run(&self, id: &str) -> Result<Option<HookRunRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, workspace_id, chat_id, run_id, tool_call_id,
                        event, hook_source, handler_type, input_json, output_json,
                        status, exit_code, stdout_preview, stderr_preview,
                        started_at, completed_at
                 FROM hook_runs
                 WHERE id = ?1",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let mut rows = statement
            .query_map(params![id], |row| {
                Ok(HookRunRecord {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    chat_id: row.get(2)?,
                    run_id: row.get(3)?,
                    tool_call_id: row.get(4)?,
                    event: row.get(5)?,
                    hook_source: row.get(6)?,
                    handler_type: row.get(7)?,
                    input_json: row.get(8)?,
                    output_json: row.get(9)?,
                    status: row.get(10)?,
                    exit_code: row.get(11)?,
                    stdout_preview: row.get(12)?,
                    stderr_preview: row.get(13)?,
                    started_at: row.get(14)?,
                    completed_at: row.get(15)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        match rows.next() {
            Some(row) => Ok(Some(row.map_err(|source| self.sqlite_error(source))?)),
            None => Ok(None),
        }
    }

    fn sqlite_error(&self, source: rusqlite::Error) -> WorkspaceDatabaseError {
        WorkspaceDatabaseError::Sqlite {
            path: self.database_path.clone(),
            source,
        }
    }
}

const AGENT_TASK_SELECT_BY_ID: &str =
    "SELECT id, team_id, owner_instance_id, origin_instance_id, parent_task_id,
            sequence, status, input_json, result_json, error_json, created_at, updated_at,
            started_at, completed_at
     FROM agent_tasks WHERE id = ?1";

const AGENT_MESSAGE_SELECT_BY_ID: &str =
    "SELECT id, team_id, sender_instance_id, receiver_instance_id, related_task_id,
            reply_to_message_id, kind, content, sequence, created_at, consumed_at
     FROM agent_messages WHERE id = ?1";

fn sql_bool(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn workspace_spec_revision_to_i64(
    revision: u64,
    field: &str,
) -> Result<i64, WorkspaceDatabaseError> {
    i64::try_from(revision).map_err(|_| WorkspaceDatabaseError::InvalidWorkspaceSpec {
        message: format!("{field} is too large"),
    })
}

fn u64_from_row(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|source| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(source),
        )
    })
}

fn optional_u64_from_row(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| {
            u64::try_from(value).map_err(|source| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Integer,
                    Box::new(source),
                )
            })
        })
        .transpose()
}

fn workspace_spec_from_row(row: &Row<'_>) -> rusqlite::Result<WorkspaceSpecRecord> {
    Ok(WorkspaceSpecRecord {
        enabled: row.get::<_, i64>(0)? != 0,
        inject_enabled: row.get::<_, i64>(1)? != 0,
        content_markdown: row.get(2)?,
        revision: u64_from_row(row, 3)?,
        generated_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn workspace_spec_job_from_row(row: &Row<'_>) -> rusqlite::Result<WorkspaceSpecJobRecord> {
    Ok(WorkspaceSpecJobRecord {
        id: row.get(0)?,
        trigger_type: row.get(1)?,
        status: row.get(2)?,
        chat_id: row.get(3)?,
        run_id: row.get(4)?,
        model_id: row.get(5)?,
        base_revision: optional_u64_from_row(row, 6)?,
        input_summary_json: row.get(7)?,
        output_json: row.get(8)?,
        error_message: row.get(9)?,
        created_at: row.get(10)?,
        started_at: row.get(11)?,
        completed_at: row.get(12)?,
        has_retry: row.get::<_, i64>(13)? != 0,
    })
}

fn chat_spec_snapshot_from_row(row: &Row<'_>) -> rusqlite::Result<ChatSpecSnapshotRecord> {
    Ok(ChatSpecSnapshotRecord {
        chat_id: row.get(0)?,
        spec_revision: u64_from_row(row, 1)?,
        content_markdown: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn scheduled_task_from_row(row: &Row<'_>) -> rusqlite::Result<ScheduledTaskRecord> {
    Ok(ScheduledTaskRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        schedule_json: row.get(3)?,
        action_json: row.get(4)?,
        status: row.get(5)?,
        next_run_at: row.get(6)?,
        last_run_at: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        metadata_json: row.get(10)?,
    })
}

fn scheduled_task_run_from_row(row: &Row<'_>) -> rusqlite::Result<ScheduledTaskRunRecord> {
    Ok(ScheduledTaskRunRecord {
        id: row.get(0)?,
        task_id: row.get(1)?,
        trigger_reason: row.get(2)?,
        status: row.get(3)?,
        scheduled_at: row.get(4)?,
        queued_at: row.get(5)?,
        started_at: row.get(6)?,
        completed_at: row.get(7)?,
        chat_id: row.get(8)?,
        user_message_id: row.get(9)?,
        assistant_message_id: row.get(10)?,
        agent_team_id: optional_agent_id_from_row(row, 11)?,
        agent_task_id: optional_agent_id_from_row(row, 12)?,
        agent_attempt_id: optional_agent_id_from_row(row, 13)?,
        active_run_id: row.get(14)?,
        error_message: row.get(15)?,
        output_summary: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
        metadata_json: row.get(19)?,
    })
}

fn agent_team_from_row(row: &Row<'_>) -> rusqlite::Result<AgentTeamRecord> {
    Ok(AgentTeamRecord {
        id: agent_id_from_row(row, 0)?,
        chat_id: row.get(1)?,
        coordinator_instance_id: agent_id_from_row(row, 2)?,
        status: agent_enum_from_row(row, 3)?,
        max_concurrent_runs: row.get(4)?,
        next_event_sequence: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn agent_instance_from_row(row: &Row<'_>) -> rusqlite::Result<AgentInstanceRecord> {
    let revision: i64 = row.get(3)?;
    let snapshot_json: String = row.get(4)?;
    Ok(AgentInstanceRecord {
        id: agent_id_from_row(row, 0)?,
        team_id: agent_id_from_row(row, 1)?,
        definition_id: agent_id_from_row(row, 2)?,
        definition_revision: u64::try_from(revision).map_err(|source| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Integer,
                Box::new(source),
            )
        })?,
        definition_snapshot: serde_json::from_str(&snapshot_json).map_err(|source| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(source),
            )
        })?,
        role: agent_enum_from_row(row, 5)?,
        status: agent_enum_from_row(row, 6)?,
        next_task_sequence: row.get(7)?,
        next_message_sequence: row.get(8)?,
        context_generation: row.get(9)?,
        last_scheduled_at: row.get(10)?,
        execution_workspace_mode: agent_enum_from_row(row, 11)?,
        execution_root_path: row.get(12)?,
        worktree_base_revision: row.get(13)?,
        worktree_branch: row.get(14)?,
        worktree_status: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

fn agent_task_from_row(row: &Row<'_>) -> rusqlite::Result<AgentTaskRecord> {
    Ok(AgentTaskRecord {
        id: agent_id_from_row(row, 0)?,
        team_id: agent_id_from_row(row, 1)?,
        owner_instance_id: agent_id_from_row(row, 2)?,
        origin_instance_id: optional_agent_id_from_row(row, 3)?,
        parent_task_id: optional_agent_id_from_row(row, 4)?,
        sequence: row.get(5)?,
        status: agent_enum_from_row(row, 6)?,
        input_json: row.get(7)?,
        result_json: row.get(8)?,
        error_json: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        started_at: row.get(12)?,
        completed_at: row.get(13)?,
    })
}

fn agent_message_from_row(row: &Row<'_>) -> rusqlite::Result<AgentMessageRecord> {
    Ok(AgentMessageRecord {
        id: agent_id_from_row(row, 0)?,
        team_id: agent_id_from_row(row, 1)?,
        sender_instance_id: optional_agent_id_from_row(row, 2)?,
        receiver_instance_id: agent_id_from_row(row, 3)?,
        related_task_id: optional_agent_id_from_row(row, 4)?,
        reply_to_message_id: optional_agent_id_from_row(row, 5)?,
        kind: agent_enum_from_row(row, 6)?,
        content: row.get(7)?,
        sequence: row.get(8)?,
        created_at: row.get(9)?,
        consumed_at: row.get(10)?,
    })
}

fn agent_id_from_row<T>(row: &Row<'_>, index: usize) -> rusqlite::Result<T>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let value: String = row.get(index)?;
    value.parse().map_err(|source| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(source),
        )
    })
}

fn optional_agent_id_from_row<T>(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<T>>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    row.get::<_, Option<String>>(index)?
        .map(|value| {
            value.parse().map_err(|source| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Text,
                    Box::new(source),
                )
            })
        })
        .transpose()
}

fn agent_enum_from_row<T>(row: &Row<'_>, index: usize) -> rusqlite::Result<T>
where
    T: DeserializeOwned,
{
    let value: String = row.get(index)?;
    serde_json::from_value(Value::String(value)).map_err(|source| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(source),
        )
    })
}

fn ensure_agent_entity_team(
    transaction: &Transaction<'_>,
    table: &str,
    entity_id: &str,
    expected_team_id: &AgentTeamId,
    entity_kind: AgentEntityKind,
    database_path: &Path,
) -> Result<(), WorkspaceDatabaseError> {
    let sql = match table {
        "agent_instances" => "SELECT team_id FROM agent_instances WHERE id = ?1",
        "agent_tasks" => "SELECT team_id FROM agent_tasks WHERE id = ?1",
        "agent_messages" => "SELECT team_id FROM agent_messages WHERE id = ?1",
        _ => {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("unsupported agent entity table '{table}'"),
            });
        }
    };
    let actual_team_id = transaction
        .query_row(sql, params![entity_id], |row| row.get::<_, String>(0))
        .optional()
        .map_err(|source| sqlite_error(database_path, source))?
        .ok_or_else(|| WorkspaceDatabaseError::InvalidAgentRuntimeData {
            message: format!("{entity_kind} '{entity_id}' was not found"),
        })?;
    if actual_team_id != expected_team_id.as_str() {
        return Err(WorkspaceDatabaseError::AgentDomain {
            source: AgentDomainError::cross_team_reference(entity_kind, entity_id),
        });
    }
    Ok(())
}

fn agent_dependency_path_exists(
    transaction: &Transaction<'_>,
    team_id: &AgentTeamId,
    start_task_id: &AgentTaskId,
    target_task_id: &AgentTaskId,
    database_path: &Path,
) -> Result<bool, WorkspaceDatabaseError> {
    transaction
        .query_row(
            "WITH RECURSIVE dependency_path(task_id) AS (
                SELECT ?2
                UNION
                SELECT dependency.dependency_task_id
                FROM agent_task_dependencies AS dependency
                JOIN dependency_path AS path
                  ON dependency.waiting_task_id = path.task_id
                WHERE dependency.team_id = ?1
             )
             SELECT EXISTS(SELECT 1 FROM dependency_path WHERE task_id = ?3)",
            params![
                team_id.as_str(),
                start_task_id.as_str(),
                target_task_id.as_str()
            ],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|source| sqlite_error(database_path, source))
}

fn validate_agent_json(value: &str, field: &'static str) -> Result<(), WorkspaceDatabaseError> {
    serde_json::from_str::<Value>(value)
        .map(|_| ())
        .map_err(|source| WorkspaceDatabaseError::AgentRuntimeJson { field, source })
}

fn redact_agent_json(value: &str, field: &'static str) -> Result<String, WorkspaceDatabaseError> {
    let mut parsed = serde_json::from_str::<Value>(value)
        .map_err(|source| WorkspaceDatabaseError::AgentRuntimeJson { field, source })?;
    redact_json_value(&mut parsed);
    serde_json::to_string(&parsed)
        .map_err(|source| WorkspaceDatabaseError::AgentRuntimeJson { field, source })
}

fn redact_agent_text(value: &str) -> String {
    const SENSITIVE_KEYS: &[&str] = &[
        "authorization",
        "api_key",
        "apikey",
        "api-key",
        "cookie",
        "password",
        "token",
        "secret",
    ];

    value
        .lines()
        .map(|line| redact_agent_text_line(line, SENSITIVE_KEYS))
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_agent_text_line(line: &str, sensitive_keys: &[&str]) -> String {
    let trimmed = line.trim_start();
    let indentation_len = line.len() - trimmed.len();
    let lower = trimmed.to_ascii_lowercase();

    for key in sensitive_keys {
        for marker in [format!("{key}="), format!("{key}:"), format!("\"{key}\":")] {
            if lower.starts_with(&marker) {
                return format!("{}[REDACTED]", &line[..indentation_len + marker.len()]);
            }
        }
    }

    line.split_whitespace()
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            if sensitive_keys.iter().any(|key| {
                lower.starts_with(&format!("{key}="))
                    || lower.starts_with(&format!("{key}:"))
                    || lower.starts_with(&format!("\"{key}\":"))
            }) {
                let separator = part.find(['=', ':']).expect("matched sensitive separator");
                format!("{}[REDACTED]", &part[..=separator])
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn validate_agent_definition_snapshot(value: &str) -> Result<(), WorkspaceDatabaseError> {
    let parsed = serde_json::from_str::<Value>(value).map_err(|source| {
        WorkspaceDatabaseError::AgentRuntimeJson {
            field: "definition_snapshot_json",
            source,
        }
    })?;
    if json_contains_secret_key(&parsed) {
        return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
            message: "agent definition snapshot contains a sensitive field".to_string(),
        });
    }
    Ok(())
}

fn redact_workspace_spec_json_object(
    value: &str,
    field: &str,
) -> Result<String, WorkspaceDatabaseError> {
    let mut parsed = parse_workspace_spec_json(value, field)?;
    if !parsed.is_object() {
        return Err(WorkspaceDatabaseError::InvalidWorkspaceSpec {
            message: format!("{field} must be a JSON object"),
        });
    }
    redact_json_value(&mut parsed);
    serde_json::to_string(&parsed).map_err(|source| WorkspaceDatabaseError::InvalidWorkspaceSpec {
        message: format!("{field} could not be serialized after redaction: {source}"),
    })
}

fn redact_optional_workspace_spec_json(
    value: Option<&str>,
    field: &str,
) -> Result<Option<String>, WorkspaceDatabaseError> {
    value
        .map(|value| redact_workspace_spec_json(value, field))
        .transpose()
}

fn redact_workspace_spec_json(value: &str, field: &str) -> Result<String, WorkspaceDatabaseError> {
    let mut parsed = parse_workspace_spec_json(value, field)?;
    redact_json_value(&mut parsed);
    serde_json::to_string(&parsed).map_err(|source| WorkspaceDatabaseError::InvalidWorkspaceSpec {
        message: format!("{field} could not be serialized after redaction: {source}"),
    })
}

fn parse_workspace_spec_json(value: &str, field: &str) -> Result<Value, WorkspaceDatabaseError> {
    serde_json::from_str::<Value>(value).map_err(|source| {
        WorkspaceDatabaseError::InvalidWorkspaceSpec {
            message: format!("{field} must be valid JSON: {source}"),
        }
    })
}

fn validate_scheduled_task_status(status: &str) -> Result<(), WorkspaceDatabaseError> {
    validate_scheduled_task_value(
        "status",
        status,
        &["enabled", "paused", "completed", "archived"],
    )
}

fn validate_scheduled_task_run_status(status: &str) -> Result<(), WorkspaceDatabaseError> {
    validate_scheduled_task_value(
        "run status",
        status,
        &[
            "pending",
            "queued",
            "running",
            "succeeded",
            "failed",
            "cancelled",
            "skipped",
        ],
    )
}

fn validate_scheduled_task_trigger_reason(
    trigger_reason: &str,
) -> Result<(), WorkspaceDatabaseError> {
    validate_scheduled_task_value(
        "trigger reason",
        trigger_reason,
        &["scheduled", "manual", "retry", "misfire_catch_up"],
    )
}

fn validate_scheduled_task_list_filter(
    filter: &ScheduledTaskListFilter<'_>,
) -> Result<(), WorkspaceDatabaseError> {
    if let Some(status) = filter.status {
        validate_scheduled_task_status(status)?;
    }
    if filter.limit <= 0 || filter.offset < 0 {
        return Err(WorkspaceDatabaseError::InvalidScheduledTaskData {
            message:
                "scheduled task pagination limit must be positive and offset must be non-negative"
                    .to_string(),
        });
    }
    Ok(())
}

fn scheduled_task_filter_sql(
    status: Option<&str>,
    search: Option<&str>,
) -> Result<(String, Vec<SqlValue>), WorkspaceDatabaseError> {
    if let Some(status) = status {
        validate_scheduled_task_status(status)?;
    }
    let mut where_clause = String::from(" WHERE 1 = 1");
    let mut query_params = Vec::new();
    if let Some(status) = status {
        where_clause.push_str(" AND status = ?");
        query_params.push(SqlValue::Text(status.to_string()));
    }
    if let Some(search) = search.map(str::trim).filter(|search| !search.is_empty()) {
        let pattern = like_contains_pattern(search);
        where_clause.push_str(
            " AND (
                id LIKE ? ESCAPE '\\' COLLATE NOCASE
                OR title LIKE ? ESCAPE '\\' COLLATE NOCASE
                OR COALESCE(description, '') LIKE ? ESCAPE '\\' COLLATE NOCASE
                OR action_json LIKE ? ESCAPE '\\' COLLATE NOCASE
             )",
        );
        for _ in 0..4 {
            query_params.push(SqlValue::Text(pattern.clone()));
        }
    }
    Ok((where_clause, query_params))
}

fn validate_scheduled_task_value(
    field: &str,
    value: &str,
    allowed: &[&str],
) -> Result<(), WorkspaceDatabaseError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(WorkspaceDatabaseError::InvalidScheduledTaskData {
            message: format!("{field} must be one of: {}", allowed.join(", ")),
        })
    }
}

fn validate_scheduled_task_json_object(
    value: &str,
    field: &str,
) -> Result<(), WorkspaceDatabaseError> {
    let parsed = serde_json::from_str::<Value>(value).map_err(|source| {
        WorkspaceDatabaseError::InvalidScheduledTaskData {
            message: format!("{field} must be valid JSON: {source}"),
        }
    })?;
    if parsed.is_object() {
        Ok(())
    } else {
        Err(WorkspaceDatabaseError::InvalidScheduledTaskData {
            message: format!("{field} must be a JSON object"),
        })
    }
}

fn validate_llm_agent_references(
    connection: &Connection,
    database_path: &Path,
    request: &NewLlmRequest<'_>,
) -> Result<(), WorkspaceDatabaseError> {
    let has_agent_reference = request.agent_team_id.is_some()
        || request.agent_instance_id.is_some()
        || request.agent_task_id.is_some()
        || request.agent_attempt_id.is_some();
    if !has_agent_reference {
        return Ok(());
    }
    let team_id =
        request
            .agent_team_id
            .ok_or_else(|| WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message:
                    "agent_team_id is required when an LLM request has Agent runtime references"
                        .to_string(),
            })?;
    let team_chat_id = connection
        .query_row(
            "SELECT chat_id FROM agent_teams WHERE id = ?1",
            params![team_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|source| sqlite_error(database_path, source))?
        .ok_or_else(|| WorkspaceDatabaseError::InvalidAgentRuntimeData {
            message: format!("agent team '{team_id}' was not found"),
        })?;
    if request
        .chat_id
        .is_some_and(|chat_id| chat_id != team_chat_id)
    {
        return Err(WorkspaceDatabaseError::AgentDomain {
            source: AgentDomainError::cross_team_reference(
                AgentEntityKind::Team,
                team_id.to_string(),
            ),
        });
    }

    for (table, id, kind) in [
        (
            "agent_instances",
            request.agent_instance_id.map(AgentInstanceId::as_str),
            AgentEntityKind::Instance,
        ),
        (
            "agent_tasks",
            request.agent_task_id.map(AgentTaskId::as_str),
            AgentEntityKind::Task,
        ),
        (
            "agent_attempts",
            request.agent_attempt_id.map(AgentAttemptId::as_str),
            AgentEntityKind::Attempt,
        ),
    ] {
        let Some(id) = id else { continue };
        let sql = match table {
            "agent_instances" => "SELECT team_id FROM agent_instances WHERE id = ?1",
            "agent_tasks" => "SELECT team_id FROM agent_tasks WHERE id = ?1",
            "agent_attempts" => "SELECT team_id FROM agent_attempts WHERE id = ?1",
            _ => unreachable!(),
        };
        let actual_team_id = connection
            .query_row(sql, params![id], |row| row.get::<_, String>(0))
            .optional()
            .map_err(|source| sqlite_error(database_path, source))?
            .ok_or_else(|| WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: format!("{kind} '{id}' was not found"),
            })?;
        if actual_team_id != team_id.as_str() {
            return Err(WorkspaceDatabaseError::AgentDomain {
                source: AgentDomainError::cross_team_reference(kind, id),
            });
        }
    }
    if let (Some(instance_id), Some(task_id)) = (request.agent_instance_id, request.agent_task_id) {
        let owner_instance_id: String = connection
            .query_row(
                "SELECT owner_instance_id FROM agent_tasks WHERE id = ?1",
                params![task_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(database_path, source))?;
        if owner_instance_id != instance_id.as_str() {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "LLM request Agent task does not belong to the referenced instance"
                    .to_string(),
            });
        }
    }
    if let (Some(task_id), Some(attempt_id)) = (request.agent_task_id, request.agent_attempt_id) {
        let attempt_task_id: String = connection
            .query_row(
                "SELECT task_id FROM agent_attempts WHERE id = ?1",
                params![attempt_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(database_path, source))?;
        if attempt_task_id != task_id.as_str() {
            return Err(WorkspaceDatabaseError::InvalidAgentRuntimeData {
                message: "LLM request Agent attempt does not belong to the referenced task"
                    .to_string(),
            });
        }
    }
    Ok(())
}

fn json_contains_secret_key(value: &Value) -> bool {
    match value {
        Value::Object(object) => object
            .iter()
            .any(|(key, value)| is_secret_audit_key(key) || json_contains_secret_key(value)),
        Value::Array(items) => items.iter().any(json_contains_secret_key),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

#[derive(Debug)]
pub enum WorkspaceDatabaseError {
    AgentDomain {
        source: AgentDomainError,
    },
    AgentRuntimeJson {
        field: &'static str,
        source: serde_json::Error,
    },
    ConcurrencyLimit {
        message: String,
    },
    InvalidAgentRuntimeData {
        message: String,
    },
    InvalidCodeGraphInput {
        message: String,
    },
    InvalidMessageMetadata {
        message: String,
    },
    ChatRewriteConflict {
        message: String,
    },
    InvalidPlan {
        message: String,
    },
    InvalidScheduledTaskData {
        message: String,
    },
    InvalidWorkspaceSpec {
        message: String,
    },
    InvalidToolCall {
        message: String,
    },
    InvalidTodoGraph {
        message: String,
    },
    InvalidAuditJson {
        field: &'static str,
        source: serde_json::Error,
    },
    InvalidAuditData {
        message: String,
    },
    InvalidAuditTokens {
        message: String,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
    MissingDatabaseParent {
        path: PathBuf,
    },
    MissingTodoGraph {
        chat_id: String,
    },
    MissingTerminalSession {
        id: String,
    },
    MissingToolCall {
        id: String,
    },
    MissingLlmRequest {
        id: String,
    },
    MissingScheduledTask {
        id: String,
    },
    MissingScheduledTaskRun {
        id: String,
    },
    NonUtf8Path {
        path: PathBuf,
    },
    Sqlite {
        path: PathBuf,
        source: rusqlite::Error,
    },
    TodoGraphJson {
        source: serde_json::Error,
    },
    UnsupportedSchemaVersion {
        path: PathBuf,
        found: u32,
        latest: u32,
    },
    WorkspaceSpecRetryAlreadyQueued {
        job_id: String,
    },
    WorkspaceNotDirectory {
        path: PathBuf,
    },
}

impl fmt::Display for WorkspaceDatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AgentDomain { source } => write!(formatter, "agent domain error: {source}"),
            Self::AgentRuntimeJson { field, source } => {
                write!(formatter, "invalid Agent runtime JSON in {field}: {source}")
            }
            Self::ConcurrencyLimit { message } => write!(formatter, "{message}"),
            Self::InvalidAgentRuntimeData { message } => {
                write!(formatter, "invalid Agent runtime data: {message}")
            }
            Self::InvalidCodeGraphInput { message } => {
                write!(formatter, "invalid code graph index data: {message}")
            }
            Self::InvalidMessageMetadata { message } => {
                write!(formatter, "invalid message metadata: {message}")
            }
            Self::ChatRewriteConflict { message } => {
                write!(formatter, "chat rewrite conflict: {message}")
            }
            Self::InvalidPlan { message } => {
                write!(formatter, "invalid plan: {message}")
            }
            Self::InvalidScheduledTaskData { message } => {
                write!(formatter, "invalid scheduled task data: {message}")
            }
            Self::InvalidWorkspaceSpec { message } => {
                write!(formatter, "invalid workspace spec: {message}")
            }
            Self::InvalidToolCall { message } => {
                write!(formatter, "invalid tool call data: {message}")
            }
            Self::InvalidTodoGraph { message } => {
                write!(formatter, "invalid todo graph: {message}")
            }
            Self::InvalidAuditJson { field, source } => {
                write!(formatter, "invalid LLM audit JSON in {field}: {source}")
            }
            Self::InvalidAuditData { message } => {
                write!(formatter, "invalid LLM audit data: {message}")
            }
            Self::InvalidAuditTokens { message } => {
                write!(formatter, "invalid LLM audit token usage: {message}")
            }
            Self::Io { path, source } => write!(formatter, "{}: {}", path.display(), source),
            Self::MissingDatabaseParent { path } => write!(
                formatter,
                "workspace database path has no parent directory: {}",
                path.display()
            ),
            Self::MissingTodoGraph { chat_id } => {
                write!(formatter, "todo graph was not found for chat: {chat_id}")
            }
            Self::MissingTerminalSession { id } => {
                write!(formatter, "terminal session was not found: {id}")
            }
            Self::MissingToolCall { id } => {
                write!(formatter, "tool call was not found: {id}")
            }
            Self::MissingLlmRequest { id } => {
                write!(formatter, "LLM request audit row was not found: {id}")
            }
            Self::MissingScheduledTask { id } => {
                write!(formatter, "scheduled task was not found: {id}")
            }
            Self::MissingScheduledTaskRun { id } => {
                write!(formatter, "scheduled task run was not found: {id}")
            }
            Self::NonUtf8Path { path } => {
                write!(formatter, "path must be valid UTF-8: {}", path.display())
            }
            Self::Sqlite { path, source } => {
                write!(formatter, "{} SQLite error: {}", path.display(), source)?;
                if let Some(error) = source.sqlite_error() {
                    write!(
                        formatter,
                        " (code={:?}, extended_code={}, extended_message={})",
                        error.code, error.extended_code, error
                    )?;
                }
                Ok(())
            }
            Self::TodoGraphJson { source } => {
                write!(formatter, "invalid todo graph JSON: {source}")
            }
            Self::UnsupportedSchemaVersion {
                path,
                found,
                latest,
            } => write!(
                formatter,
                "{} has unsupported workspace database schema version {}; latest supported version is {}",
                path.display(),
                found,
                latest
            ),
            Self::WorkspaceSpecRetryAlreadyQueued { job_id } => write!(
                formatter,
                "workspace spec job '{job_id}' already has a queued or running retry"
            ),
            Self::WorkspaceNotDirectory { path } => write!(
                formatter,
                "workspace path does not exist or is not a directory: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for WorkspaceDatabaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AgentDomain { source } => Some(source),
            Self::AgentRuntimeJson { source, .. } => Some(source),
            Self::InvalidAuditJson { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::Sqlite { source, .. } => Some(source),
            Self::TodoGraphJson { source } => Some(source),
            Self::InvalidAgentRuntimeData { .. }
            | Self::ConcurrencyLimit { .. }
            | Self::InvalidAuditData { .. }
            | Self::InvalidAuditTokens { .. }
            | Self::InvalidCodeGraphInput { .. }
            | Self::InvalidMessageMetadata { .. }
            | Self::ChatRewriteConflict { .. }
            | Self::InvalidPlan { .. }
            | Self::InvalidScheduledTaskData { .. }
            | Self::InvalidWorkspaceSpec { .. }
            | Self::InvalidToolCall { .. }
            | Self::InvalidTodoGraph { .. }
            | Self::MissingDatabaseParent { .. }
            | Self::MissingLlmRequest { .. }
            | Self::MissingScheduledTask { .. }
            | Self::MissingScheduledTaskRun { .. }
            | Self::MissingTodoGraph { .. }
            | Self::MissingTerminalSession { .. }
            | Self::MissingToolCall { .. }
            | Self::NonUtf8Path { .. }
            | Self::UnsupportedSchemaVersion { .. }
            | Self::WorkspaceSpecRetryAlreadyQueued { .. }
            | Self::WorkspaceNotDirectory { .. } => None,
        }
    }
}

fn message_from_row(row: &Row<'_>) -> rusqlite::Result<MessageRecord> {
    Ok(MessageRecord {
        id: row.get(0)?,
        chat_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        sequence: row.get(4)?,
        created_at: row.get(5)?,
        metadata_json: row.get(6)?,
    })
}

fn plan_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlanRecord> {
    Ok(PlanRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        overview: row.get(2)?,
        status: row.get(3)?,
        sort_order: row.get(4)?,
        source_chat_id: row.get(5)?,
        active_phase_id: row.get(6)?,
        pause_requested_at: row.get(7)?,
        completed_at: row.get(8)?,
        completed_by_user_at: row.get(9)?,
        error_message: row.get(10)?,
        shared_merge_commit_id: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        phases: Vec::new(),
    })
}

fn plan_phase_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlanPhaseRecord> {
    Ok(PlanPhaseRecord {
        id: row.get(0)?,
        plan_id: row.get(1)?,
        sequence: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        status: row.get(5)?,
        implementation_chat_id: row.get(6)?,
        agent_team_id: row.get(7)?,
        agent_task_id: row.get(8)?,
        commit_id: row.get(9)?,
        merge_attempt_count: row.get(10)?,
        error_message: row.get(11)?,
        started_at: row.get(12)?,
        completed_at: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        steps: Vec::new(),
        attempts: Vec::new(),
    })
}

fn plan_phase_attempt_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<PlanPhaseAttemptRecord> {
    Ok(PlanPhaseAttemptRecord {
        id: row.get(0)?,
        plan_id: row.get(1)?,
        phase_id: row.get(2)?,
        sequence: row.get(3)?,
        trigger: row.get(4)?,
        status: row.get(5)?,
        provider_id: row.get(6)?,
        model_id: row.get(7)?,
        thinking_level: row.get(8)?,
        implementation_chat_id: row.get(9)?,
        agent_team_id: row.get(10)?,
        agent_task_id: row.get(11)?,
        commit_id: row.get(12)?,
        error_message: row.get(13)?,
        started_at: row.get(14)?,
        completed_at: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

fn plan_phase_derived_effects_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<PlanPhaseDerivedEffectsRecord> {
    Ok(PlanPhaseDerivedEffectsRecord {
        attempt_id: row.get(0)?,
        plan_id: row.get(1)?,
        phase_id: row.get(2)?,
        agent_task_id: AgentTaskId::new(row.get::<_, String>(3)?).map_err(|source| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(source),
            )
        })?,
        chat_id: row.get(4)?,
        run_id: row.get(5)?,
        user_message_id: row.get(6)?,
        assistant_message_id: row.get(7)?,
        status: row.get(8)?,
        context_json: row.get(9)?,
        integration_confirmed_at: row.get(10)?,
        terminal_reason: row.get(11)?,
        released_at: row.get(12)?,
        discarded_at: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn plan_step_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlanStepRecord> {
    let acceptance_json: String = row.get(6)?;
    let acceptance = serde_json::from_str::<Vec<String>>(&acceptance_json).map_err(|source| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(source))
    })?;

    Ok(PlanStepRecord {
        id: row.get(0)?,
        plan_id: row.get(1)?,
        phase_id: row.get(2)?,
        sequence: row.get(3)?,
        title: row.get(4)?,
        detail: row.get(5)?,
        acceptance,
        status: row.get(7)?,
        checked_at: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn normalized_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn validate_plan_view(view: &str) -> Result<(), WorkspaceDatabaseError> {
    if matches!(view, "active" | "all") {
        Ok(())
    } else {
        Err(WorkspaceDatabaseError::InvalidPlan {
            message: format!("unknown plan view: {view}"),
        })
    }
}

fn validate_plan_status(status: &str) -> Result<(), WorkspaceDatabaseError> {
    if matches!(
        status,
        "draft"
            | "ready"
            | "running"
            | "paused"
            | "implemented"
            | "completed"
            | "failed"
            | "cancelled"
    ) {
        Ok(())
    } else {
        Err(WorkspaceDatabaseError::InvalidPlan {
            message: format!("unknown plan status: {status}"),
        })
    }
}

fn is_reorderable_plan_status(status: &str) -> bool {
    matches!(status, "draft" | "ready" | "paused" | "failed")
}

fn validate_plan_step_status(status: &str) -> Result<(), WorkspaceDatabaseError> {
    if matches!(
        status,
        "pending" | "running" | "completed" | "failed" | "cancelled"
    ) {
        Ok(())
    } else {
        Err(WorkspaceDatabaseError::InvalidPlan {
            message: format!("unknown plan step status: {status}"),
        })
    }
}

fn required_plan_text(field: &str, value: &str) -> Result<String, WorkspaceDatabaseError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(WorkspaceDatabaseError::InvalidPlan {
            message: format!("plan {field} must not be empty"),
        });
    }

    Ok(value.to_string())
}

fn ensure_plan_entity_id_available(
    transaction: &Transaction<'_>,
    database_path: &Path,
    exists_sql: &str,
    entity: &str,
    id: &str,
) -> Result<String, WorkspaceDatabaseError> {
    let id = required_plan_text(&format!("{entity}.id"), id)?;
    let exists: bool = transaction
        .query_row(exists_sql, params![id.as_str()], |row| row.get(0))
        .map_err(|source| sqlite_error(database_path, source))?;
    if exists {
        return Err(WorkspaceDatabaseError::InvalidPlan {
            message: format!("{entity} id already exists: {id}"),
        });
    }

    Ok(id)
}

fn plan_acceptance_json(values: &[String]) -> Result<String, WorkspaceDatabaseError> {
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        let value = value.trim();
        if !value.is_empty() {
            normalized.push(value.to_string());
        }
    }
    serde_json::to_string(&normalized).map_err(|source| WorkspaceDatabaseError::InvalidPlan {
        message: format!("plan acceptance is invalid JSON: {source}"),
    })
}

fn code_graph_symbol_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CodeGraphSymbolRecord> {
    code_graph_symbol_from_row_offset(row, 0)
}

fn code_graph_file_summary_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CodeGraphFileSummaryRecord> {
    let modules: String = row.get(4)?;
    Ok(CodeGraphFileSummaryRecord {
        path: row.get(0)?,
        language: row.get(1)?,
        symbol_count: row.get(2)?,
        import_count: row.get(3)?,
        import_modules: modules
            .split(',')
            .map(str::trim)
            .filter(|module| !module.is_empty())
            .map(str::to_string)
            .collect(),
    })
}

fn code_graph_symbol_from_row_offset(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<CodeGraphSymbolRecord> {
    Ok(CodeGraphSymbolRecord {
        id: row.get(offset)?,
        path: row.get(offset + 1)?,
        language: row.get(offset + 2)?,
        name: row.get(offset + 3)?,
        kind: row.get(offset + 4)?,
        start_line: row.get(offset + 5)?,
        start_column: row.get(offset + 6)?,
        end_line: row.get(offset + 7)?,
        end_column: row.get(offset + 8)?,
        signature: row.get(offset + 9)?,
        documentation: row.get(offset + 10)?,
    })
}

fn optional_code_graph_symbol_from_row_offset(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<Option<CodeGraphSymbolRecord>> {
    let id = row.get::<_, Option<i64>>(offset)?;

    if id.is_none() {
        return Ok(None);
    }

    Ok(Some(code_graph_symbol_from_row_offset(row, offset)?))
}

fn code_graph_relation_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CodeGraphSymbolRelationRecord> {
    Ok(CodeGraphSymbolRelationRecord {
        edge_id: row.get(0)?,
        edge_kind: row.get(1)?,
        metadata_json: row.get(2)?,
        source: code_graph_symbol_from_row_offset(row, 3)?,
        target: code_graph_symbol_from_row_offset(row, 14)?,
    })
}

fn code_graph_reference_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CodeGraphReferenceRecord> {
    Ok(CodeGraphReferenceRecord {
        id: row.get(0)?,
        path: row.get(1)?,
        language: row.get(2)?,
        name: row.get(3)?,
        start_line: row.get(4)?,
        start_column: row.get(5)?,
        end_line: row.get(6)?,
        end_column: row.get(7)?,
        symbol: optional_code_graph_symbol_from_row_offset(row, 8)?,
    })
}

fn code_graph_file_id(
    transaction: &Transaction<'_>,
    database_path: &Path,
    path: &str,
) -> Result<i64, WorkspaceDatabaseError> {
    transaction
        .query_row(
            "SELECT id FROM code_graph_files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )
        .map_err(|source| sqlite_error(database_path, source))
}

fn optional_code_graph_file_id(
    transaction: &Transaction<'_>,
    database_path: &Path,
    path: &str,
) -> Result<Option<i64>, WorkspaceDatabaseError> {
    transaction
        .query_row(
            "SELECT id FROM code_graph_files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| sqlite_error(database_path, source))
}

fn clear_code_graph_file_index(
    transaction: &Transaction<'_>,
    database_path: &Path,
    file_id: i64,
    path: &str,
) -> Result<(), WorkspaceDatabaseError> {
    delete_code_graph_file_fts_entries(transaction, database_path, file_id, path)?;
    transaction
        .execute(
            "DELETE FROM code_graph_references WHERE file_id = ?1",
            params![file_id],
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    transaction
        .execute(
            "DELETE FROM code_graph_imports WHERE file_id = ?1",
            params![file_id],
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    transaction
        .execute(
            "DELETE FROM code_graph_symbols WHERE file_id = ?1",
            params![file_id],
        )
        .map_err(|source| sqlite_error(database_path, source))?;

    Ok(())
}

fn insert_code_graph_fts_entry(
    insert_fts_data: &mut rusqlite::Statement<'_>,
    insert_fts_index: &mut rusqlite::Statement<'_>,
    database_path: &Path,
    entity_kind: &str,
    entity_id: &str,
    title: &str,
    body: &str,
    updated_at: &str,
) -> Result<(), WorkspaceDatabaseError> {
    insert_fts_data
        .execute(params![entity_kind, entity_id, title, body, updated_at])
        .map_err(|source| sqlite_error(database_path, source))?;
    insert_fts_index
        .execute(params![entity_kind, entity_id, title, body])
        .map_err(|source| sqlite_error(database_path, source))?;

    Ok(())
}

fn delete_code_graph_file_fts_entries(
    transaction: &Transaction<'_>,
    database_path: &Path,
    file_id: i64,
    path: &str,
) -> Result<(), WorkspaceDatabaseError> {
    transaction
        .execute(
            "DELETE FROM code_graph_fts_index
             WHERE
                (entity_kind = 'file' AND entity_id = ?1)
                OR (
                    entity_kind = 'symbol'
                    AND entity_id IN (
                        SELECT CAST(id AS TEXT)
                        FROM code_graph_symbols
                        WHERE file_id = ?2
                    )
                )",
            params![path, file_id],
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    transaction
        .execute(
            "DELETE FROM code_graph_fts_data
             WHERE
                (entity_kind = 'file' AND entity_id = ?1)
                OR (
                    entity_kind = 'symbol'
                    AND entity_id IN (
                        SELECT CAST(id AS TEXT)
                        FROM code_graph_symbols
                        WHERE file_id = ?2
                    )
                )",
            params![path, file_id],
        )
        .map_err(|source| sqlite_error(database_path, source))?;

    Ok(())
}

fn normalize_new_todo_graph_tasks(
    tasks: Vec<TodoGraphTask>,
    now: &str,
) -> Result<Vec<TodoGraphTask>, WorkspaceDatabaseError> {
    let mut normalized = Vec::with_capacity(tasks.len());

    for task in tasks {
        normalized.push(normalize_todo_graph_task(task, now)?);
    }

    validate_todo_graph_tasks(&normalized)?;

    Ok(normalized)
}

fn normalize_todo_graph_task(
    mut task: TodoGraphTask,
    now: &str,
) -> Result<TodoGraphTask, WorkspaceDatabaseError> {
    task.id = required_todo_graph_text("id", task.id)?;
    task.title = required_todo_graph_text("title", task.title)?;
    task.status = normalize_task_status(task.status)?;
    task.depends_on = normalize_todo_graph_text_array("dependsOn", task.depends_on)?;
    task.acceptance = normalize_todo_graph_text_array("acceptance", task.acceptance)?;
    task.summary = task.summary.trim().to_string();
    task.created_at = now.to_string();
    task.updated_at = now.to_string();
    task.subtasks = normalize_new_todo_graph_tasks_without_validation(task.subtasks, now)?;

    Ok(task)
}

fn normalize_new_todo_graph_tasks_without_validation(
    tasks: Vec<TodoGraphTask>,
    now: &str,
) -> Result<Vec<TodoGraphTask>, WorkspaceDatabaseError> {
    let mut normalized = Vec::with_capacity(tasks.len());

    for task in tasks {
        normalized.push(normalize_todo_graph_task(task, now)?);
    }

    Ok(normalized)
}

struct LlmRequestUsageRollupSource<'a> {
    workspace_id: Option<&'a str>,
    provider_id: &'a str,
    model_id: &'a str,
    request_started_at: &'a str,
    final_state: &'a str,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
    total_latency_ms: Option<i64>,
}

struct LlmRequestUsageRollupDelta {
    workspace_id: String,
    bucket_date: String,
    provider_id: String,
    model_id: String,
    final_state: String,
    request_count: i64,
    success_count: i64,
    failed_count: i64,
    total_input_tokens: i64,
    total_output_tokens: i64,
    total_cache_read_tokens: i64,
    total_cache_write_tokens: i64,
    total_tokens: i64,
    latency_count: i64,
    latency_sum: i64,
}

fn select_llm_request_record(
    transaction: &Transaction<'_>,
    id: &str,
) -> rusqlite::Result<Option<LlmRequestRecord>> {
    transaction
        .query_row(
            "SELECT
                id, workspace_id, chat_id, request_kind, agent_team_id, agent_instance_id,
                agent_task_id, agent_attempt_id, provider_id, model_id, thinking_level,
                request_started_at, first_token_at, completed_at, input_tokens, output_tokens,
                cache_read_tokens, cache_write_tokens, reasoning_tokens, cache_ratio,
                first_token_latency_ms, total_latency_ms, status_code, final_state,
                request_body_json, response_body_json, invalidated_at, invalidated_reason
             FROM llm_requests
             WHERE id = ?1",
            params![id],
            llm_request_record_from_row,
        )
        .optional()
}

fn llm_request_record_from_row(row: &Row<'_>) -> rusqlite::Result<LlmRequestRecord> {
    Ok(LlmRequestRecord {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        chat_id: row.get(2)?,
        request_kind: row.get(3)?,
        agent_team_id: optional_agent_id_from_row(row, 4)?,
        agent_instance_id: optional_agent_id_from_row(row, 5)?,
        agent_task_id: optional_agent_id_from_row(row, 6)?,
        agent_attempt_id: optional_agent_id_from_row(row, 7)?,
        provider_id: row.get(8)?,
        model_id: row.get(9)?,
        thinking_level: row.get(10)?,
        request_started_at: row.get(11)?,
        first_token_at: row.get(12)?,
        completed_at: row.get(13)?,
        input_tokens: row.get(14)?,
        output_tokens: row.get(15)?,
        cache_read_tokens: row.get(16)?,
        cache_write_tokens: row.get(17)?,
        reasoning_tokens: row.get(18)?,
        cache_ratio: row.get(19)?,
        first_token_latency_ms: row.get(20)?,
        total_latency_ms: row.get(21)?,
        status_code: row.get(22)?,
        final_state: row.get(23)?,
        request_body_json: row.get(24)?,
        response_body_json: row.get(25)?,
        invalidated_at: row.get(26)?,
        invalidated_reason: row.get(27)?,
    })
}

fn llm_request_record_rollup_source(request: &LlmRequestRecord) -> LlmRequestUsageRollupSource<'_> {
    LlmRequestUsageRollupSource {
        workspace_id: request.workspace_id.as_deref(),
        provider_id: request.provider_id.as_str(),
        model_id: request.model_id.as_str(),
        request_started_at: request.request_started_at.as_str(),
        final_state: request.final_state.as_str(),
        input_tokens: request.input_tokens,
        output_tokens: request.output_tokens,
        cache_read_tokens: request.cache_read_tokens,
        cache_write_tokens: request.cache_write_tokens,
        total_latency_ms: request.total_latency_ms,
    }
}

fn llm_request_usage_rollup_delta(
    source: LlmRequestUsageRollupSource<'_>,
    sign: i64,
) -> Option<LlmRequestUsageRollupDelta> {
    if source.final_state == "running" {
        return None;
    }

    let input_tokens = source.input_tokens.unwrap_or(0);
    let output_tokens = source.output_tokens.unwrap_or(0);
    let latency_sum = source.total_latency_ms.unwrap_or(0);
    Some(LlmRequestUsageRollupDelta {
        workspace_id: normalize_llm_request_rollup_dimension(
            source.workspace_id,
            LLM_REQUEST_ROLLUP_UNKNOWN_WORKSPACE,
        ),
        bucket_date: normalized_llm_request_rollup_bucket(source.request_started_at),
        provider_id: normalize_llm_request_rollup_dimension(
            Some(source.provider_id),
            LLM_REQUEST_ROLLUP_UNKNOWN_PROVIDER,
        ),
        model_id: normalize_llm_request_rollup_dimension(
            Some(source.model_id),
            LLM_REQUEST_ROLLUP_UNKNOWN_MODEL,
        ),
        final_state: source.final_state.to_string(),
        request_count: sign,
        success_count: if matches!(source.final_state, "succeeded" | "completed") {
            sign
        } else {
            0
        },
        failed_count: if matches!(source.final_state, "succeeded" | "completed") {
            0
        } else {
            sign
        },
        total_input_tokens: sign * input_tokens,
        total_output_tokens: sign * output_tokens,
        total_cache_read_tokens: sign * source.cache_read_tokens.unwrap_or(0),
        total_cache_write_tokens: sign * source.cache_write_tokens.unwrap_or(0),
        total_tokens: sign * (input_tokens + output_tokens),
        latency_count: if source.total_latency_ms.is_some() {
            sign
        } else {
            0
        },
        latency_sum: sign * latency_sum,
    })
}

fn apply_llm_request_usage_rollup_delta(
    transaction: &Transaction<'_>,
    database_path: &Path,
    delta: Option<LlmRequestUsageRollupDelta>,
) -> Result<(), WorkspaceDatabaseError> {
    let Some(delta) = delta else {
        return Ok(());
    };

    if delta.request_count < 0 {
        transaction
            .execute(
                "UPDATE llm_request_usage_rollups
                 SET request_count = request_count + ?6,
                     success_count = success_count + ?7,
                     failed_count = failed_count + ?8,
                     total_input_tokens = total_input_tokens + ?9,
                     total_output_tokens = total_output_tokens + ?10,
                     total_cache_read_tokens = total_cache_read_tokens + ?11,
                     total_cache_write_tokens = total_cache_write_tokens + ?12,
                     total_tokens = total_tokens + ?13,
                     latency_count = latency_count + ?14,
                     latency_sum = latency_sum + ?15
                 WHERE workspace_id = ?1
                   AND bucket_date = ?2
                   AND provider_id = ?3
                   AND model_id = ?4
                   AND final_state = ?5",
                params![
                    delta.workspace_id.as_str(),
                    delta.bucket_date.as_str(),
                    delta.provider_id.as_str(),
                    delta.model_id.as_str(),
                    delta.final_state.as_str(),
                    delta.request_count,
                    delta.success_count,
                    delta.failed_count,
                    delta.total_input_tokens,
                    delta.total_output_tokens,
                    delta.total_cache_read_tokens,
                    delta.total_cache_write_tokens,
                    delta.total_tokens,
                    delta.latency_count,
                    delta.latency_sum,
                ],
            )
            .map_err(|source| sqlite_error(database_path, source))?;
        transaction
            .execute(
                "DELETE FROM llm_request_usage_rollups
                 WHERE workspace_id = ?1
                   AND bucket_date = ?2
                   AND provider_id = ?3
                   AND model_id = ?4
                   AND final_state = ?5
                   AND request_count = 0",
                params![
                    delta.workspace_id.as_str(),
                    delta.bucket_date.as_str(),
                    delta.provider_id.as_str(),
                    delta.model_id.as_str(),
                    delta.final_state.as_str(),
                ],
            )
            .map_err(|source| sqlite_error(database_path, source))?;
        return Ok(());
    }

    transaction
        .execute(
            "INSERT INTO llm_request_usage_rollups (
                workspace_id, bucket_date, provider_id, model_id, final_state,
                request_count, success_count, failed_count,
                total_input_tokens, total_output_tokens,
                total_cache_read_tokens, total_cache_write_tokens,
                total_tokens, latency_count, latency_sum
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(workspace_id, bucket_date, provider_id, model_id, final_state)
             DO UPDATE SET
                request_count = request_count + excluded.request_count,
                success_count = success_count + excluded.success_count,
                failed_count = failed_count + excluded.failed_count,
                total_input_tokens = total_input_tokens + excluded.total_input_tokens,
                total_output_tokens = total_output_tokens + excluded.total_output_tokens,
                total_cache_read_tokens = total_cache_read_tokens + excluded.total_cache_read_tokens,
                total_cache_write_tokens = total_cache_write_tokens + excluded.total_cache_write_tokens,
                total_tokens = total_tokens + excluded.total_tokens,
                latency_count = latency_count + excluded.latency_count,
                latency_sum = latency_sum + excluded.latency_sum",
            params![
                delta.workspace_id.as_str(),
                delta.bucket_date.as_str(),
                delta.provider_id.as_str(),
                delta.model_id.as_str(),
                delta.final_state.as_str(),
                delta.request_count,
                delta.success_count,
                delta.failed_count,
                delta.total_input_tokens,
                delta.total_output_tokens,
                delta.total_cache_read_tokens,
                delta.total_cache_write_tokens,
                delta.total_tokens,
                delta.latency_count,
                delta.latency_sum,
            ],
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    Ok(())
}

fn insert_llm_request_usage_rollup_rebuild_rows(
    transaction: &Transaction<'_>,
    database_path: &Path,
    workspace_id: Option<&str>,
) -> Result<(), WorkspaceDatabaseError> {
    let mut query = String::from(
        "INSERT INTO llm_request_usage_rollups (
            workspace_id, bucket_date, provider_id, model_id, final_state,
            request_count, success_count, failed_count,
            total_input_tokens, total_output_tokens,
            total_cache_read_tokens, total_cache_write_tokens,
            total_tokens, latency_count, latency_sum
         )
         SELECT
            COALESCE(NULLIF(workspace_id, ''), '__foco_unknown_workspace__'),
            COALESCE(NULLIF(SUBSTR(request_started_at, 1, 10), ''), '__foco_unknown_date__'),
            COALESCE(NULLIF(provider_id, ''), '__foco_unknown_provider__'),
            COALESCE(NULLIF(model_id, ''), '__foco_unknown_model__'),
            final_state,
            COUNT(*),
            COUNT(CASE WHEN final_state IN ('succeeded', 'completed') THEN 1 END),
            COUNT(CASE WHEN final_state NOT IN ('succeeded', 'completed') THEN 1 END),
            COALESCE(SUM(COALESCE(input_tokens, 0)), 0),
            COALESCE(SUM(COALESCE(output_tokens, 0)), 0),
            COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0),
            COALESCE(SUM(COALESCE(cache_write_tokens, 0)), 0),
            COALESCE(SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)), 0),
            COUNT(total_latency_ms),
            COALESCE(SUM(COALESCE(total_latency_ms, 0)), 0)
         FROM llm_requests
         WHERE final_state != 'running'
           AND invalidated_at IS NULL",
    );
    let mut query_params = Vec::new();
    if let Some(workspace_id) = workspace_id {
        query.push_str(" AND workspace_id = ?");
        query_params.push(SqlValue::Text(workspace_id.to_string()));
    }
    query.push_str(" GROUP BY 1, 2, 3, 4, 5");

    transaction
        .execute(&query, params_from_iter(query_params))
        .map_err(|source| sqlite_error(database_path, source))?;
    Ok(())
}

fn normalize_llm_request_rollup_dimension(value: Option<&str>, unknown: &str) -> String {
    value
        .and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .unwrap_or(unknown)
        .to_string()
}

fn normalized_llm_request_rollup_bucket(request_started_at: &str) -> String {
    let bucket = request_started_at
        .get(..10)
        .unwrap_or(request_started_at)
        .trim();
    if bucket.is_empty() {
        LLM_REQUEST_ROLLUP_UNKNOWN_BUCKET.to_string()
    } else {
        bucket.to_string()
    }
}

fn required_todo_graph_text(field: &str, value: String) -> Result<String, WorkspaceDatabaseError> {
    let value = value.trim().to_string();

    if value.is_empty() {
        return Err(WorkspaceDatabaseError::InvalidTodoGraph {
            message: format!("{field} must not be empty"),
        });
    }

    Ok(value)
}

fn normalize_todo_graph_text_array(
    field: &str,
    values: Vec<String>,
) -> Result<Vec<String>, WorkspaceDatabaseError> {
    let mut normalized = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();

    for value in values {
        let value = required_todo_graph_text(field, value)?;

        if seen.insert(value.clone()) {
            normalized.push(value);
        }
    }

    Ok(normalized)
}

fn normalize_task_status(status: String) -> Result<String, WorkspaceDatabaseError> {
    let status = status.trim().to_string();

    if is_todo_graph_status(&status) {
        Ok(status)
    } else {
        Err(WorkspaceDatabaseError::InvalidTodoGraph {
            message: format!("status must be one of: {}", TODO_GRAPH_STATUSES.join(", ")),
        })
    }
}

fn validate_todo_graph_tasks(tasks: &[TodoGraphTask]) -> Result<(), WorkspaceDatabaseError> {
    let mut task_ids = HashSet::new();
    let mut dependencies = HashMap::new();

    collect_todo_graph_ids(tasks, &mut task_ids, &mut dependencies)?;

    for (task_id, depends_on) in &dependencies {
        for dependency_id in depends_on {
            if dependency_id == task_id {
                return Err(WorkspaceDatabaseError::InvalidTodoGraph {
                    message: format!("task '{task_id}' cannot depend on itself"),
                });
            }

            if !task_ids.contains(dependency_id) {
                return Err(WorkspaceDatabaseError::InvalidTodoGraph {
                    message: format!("task '{task_id}' depends on missing task '{dependency_id}'"),
                });
            }
        }
    }

    validate_todo_graph_dependency_cycles(&dependencies)
}

fn collect_todo_graph_ids(
    tasks: &[TodoGraphTask],
    task_ids: &mut HashSet<String>,
    dependencies: &mut HashMap<String, Vec<String>>,
) -> Result<(), WorkspaceDatabaseError> {
    for task in tasks {
        if !task_ids.insert(task.id.clone()) {
            return Err(WorkspaceDatabaseError::InvalidTodoGraph {
                message: format!("duplicate task id: {}", task.id),
            });
        }
        if !is_todo_graph_status(&task.status) {
            return Err(WorkspaceDatabaseError::InvalidTodoGraph {
                message: format!("task '{}' has invalid status '{}'", task.id, task.status),
            });
        }
        dependencies.insert(task.id.clone(), task.depends_on.clone());
        collect_todo_graph_ids(&task.subtasks, task_ids, dependencies)?;
    }

    Ok(())
}

fn validate_todo_graph_dependency_cycles(
    dependencies: &HashMap<String, Vec<String>>,
) -> Result<(), WorkspaceDatabaseError> {
    let mut states = HashMap::new();

    for task_id in dependencies.keys() {
        visit_task_dependency(task_id, dependencies, &mut states)?;
    }

    Ok(())
}

fn visit_task_dependency(
    task_id: &str,
    dependencies: &HashMap<String, Vec<String>>,
    states: &mut HashMap<String, u8>,
) -> Result<(), WorkspaceDatabaseError> {
    match states.get(task_id).copied() {
        Some(1) => {
            return Err(WorkspaceDatabaseError::InvalidTodoGraph {
                message: format!("todo graph dependencies contain a cycle at '{task_id}'"),
            });
        }
        Some(2) => return Ok(()),
        _ => {}
    }

    states.insert(task_id.to_string(), 1);
    if let Some(depends_on) = dependencies.get(task_id) {
        for dependency_id in depends_on {
            visit_task_dependency(dependency_id, dependencies, states)?;
        }
    }
    states.insert(task_id.to_string(), 2);

    Ok(())
}

fn update_task_by_id(
    tasks: &mut [TodoGraphTask],
    task_id: &str,
    patch: &TodoGraphTaskPatch,
    now: &str,
) -> Result<Option<TodoGraphTask>, WorkspaceDatabaseError> {
    for task in tasks {
        if task.id == task_id {
            apply_task_patch(task, patch, now)?;
            return Ok(Some(task.clone()));
        }

        if let Some(updated) = update_task_by_id(&mut task.subtasks, task_id, patch, now)? {
            return Ok(Some(updated));
        }
    }

    Ok(None)
}

fn apply_task_patch(
    task: &mut TodoGraphTask,
    patch: &TodoGraphTaskPatch,
    now: &str,
) -> Result<(), WorkspaceDatabaseError> {
    if patch.title.is_none()
        && patch.status.is_none()
        && patch.depends_on.is_none()
        && patch.acceptance.is_none()
        && patch.summary.is_none()
        && patch.subtasks.is_none()
    {
        return Err(WorkspaceDatabaseError::InvalidTodoGraph {
            message: "task patch must update at least one field".to_string(),
        });
    }

    if let Some(title) = &patch.title {
        task.title = required_todo_graph_text("title", title.clone())?;
    }
    if let Some(status) = &patch.status {
        task.status = normalize_task_status(status.clone())?;
    }
    if let Some(depends_on) = &patch.depends_on {
        task.depends_on = normalize_todo_graph_text_array("dependsOn", depends_on.clone())?;
    }
    if let Some(acceptance) = &patch.acceptance {
        task.acceptance = normalize_todo_graph_text_array("acceptance", acceptance.clone())?;
    }
    if let Some(summary) = &patch.summary {
        task.summary = summary.trim().to_string();
    }
    if let Some(subtasks) = &patch.subtasks {
        task.subtasks = normalize_new_todo_graph_tasks_without_validation(subtasks.clone(), now)?;
    }

    task.updated_at = now.to_string();

    Ok(())
}

fn filter_todo_graph_tasks(
    tasks: Vec<TodoGraphTask>,
    filter: TodoGraphFilter<'_>,
) -> Result<Vec<TodoGraphTask>, WorkspaceDatabaseError> {
    if let Some(status) = filter.status {
        if !is_todo_graph_status(status) {
            return Err(WorkspaceDatabaseError::InvalidTodoGraph {
                message: format!("status must be one of: {}", TODO_GRAPH_STATUSES.join(", ")),
            });
        }
    }

    if filter.status.is_none() && filter.task_id.is_none() {
        return Ok(tasks);
    }

    let mut matches = Vec::new();
    collect_matching_todo_graph_tasks(&tasks, filter, &mut matches);

    Ok(matches)
}

fn collect_matching_todo_graph_tasks(
    tasks: &[TodoGraphTask],
    filter: TodoGraphFilter<'_>,
    matches: &mut Vec<TodoGraphTask>,
) {
    for task in tasks {
        let status_matches = filter.status.is_none_or(|status| task.status == status);
        let id_matches = filter.task_id.is_none_or(|task_id| task.id == task_id);

        if status_matches && id_matches {
            matches.push(if filter.include_subtasks {
                task.clone()
            } else {
                task_without_subtasks(task)
            });
        }

        collect_matching_todo_graph_tasks(&task.subtasks, filter, matches);
    }
}

fn task_without_subtasks(task: &TodoGraphTask) -> TodoGraphTask {
    TodoGraphTask {
        subtasks: Vec::new(),
        ..task.clone()
    }
}

fn is_todo_graph_status(status: &str) -> bool {
    TODO_GRAPH_STATUSES.contains(&status)
}

const TODO_GRAPH_STATUSES: &[&str] = &[
    "pending",
    "ready",
    "running",
    "blocked",
    "completed",
    "failed",
    "cancelled",
];

fn append_llm_request_kind_exclusion_condition(
    query: &mut String,
    query_params: &mut Vec<SqlValue>,
    request_kinds: &[&str],
) {
    if request_kinds.is_empty() {
        return;
    }
    let placeholders = vec!["?"; request_kinds.len()].join(", ");
    query.push_str(&format!(" AND request_kind NOT IN ({placeholders})"));
    query_params.extend(
        request_kinds
            .iter()
            .map(|value| SqlValue::Text((*value).to_string())),
    );
}

fn append_llm_request_audit_where_clause(
    query: &mut String,
    query_params: &mut Vec<SqlValue>,
    filters: LlmRequestAuditFilters<'_>,
) {
    fn append_condition(query: &mut String, has_where: &mut bool, condition: &str) {
        query.push_str(if *has_where { " AND " } else { " WHERE " });
        query.push_str(condition);
        *has_where = true;
    }

    fn append_request_kind_exclusion(
        query: &mut String,
        query_params: &mut Vec<SqlValue>,
        has_where: &mut bool,
        request_kinds: &[&str],
    ) {
        if request_kinds.is_empty() {
            return;
        }
        let placeholders = vec!["?"; request_kinds.len()].join(", ");
        append_condition(
            query,
            has_where,
            &format!("request_kind NOT IN ({placeholders})"),
        );
        query_params.extend(
            request_kinds
                .iter()
                .map(|value| SqlValue::Text((*value).to_string())),
        );
    }

    let mut has_where = false;
    if !filters.request_ids.is_empty() {
        let placeholders = vec!["?"; filters.request_ids.len()].join(", ");
        append_condition(query, &mut has_where, &format!("id IN ({placeholders})"));
        query_params.extend(
            filters
                .request_ids
                .iter()
                .map(|value| SqlValue::Text(value.to_string())),
        );
    }
    let mut push_condition = |condition: &str, value: &str| {
        append_condition(query, &mut has_where, condition);
        query_params.push(SqlValue::Text(value.to_string()));
    };

    if let Some(value) = filters.workspace_id {
        push_condition("workspace_id = ?", value);
    }
    if let Some(value) = filters.chat_id {
        push_condition("chat_id = ?", value);
    }
    if let Some(value) = filters.request_kind {
        push_condition("request_kind = ?", value);
    }
    if let Some(value) = filters.provider_id {
        push_condition("provider_id = ?", value);
    }
    if let Some(value) = filters.model_id {
        push_condition("model_id = ?", value);
    }
    if let Some(value) = filters.final_state {
        push_condition("final_state = ?", value);
    }
    if let Some(value) = filters.started_after {
        push_condition("request_started_at >= ?", value);
    }
    if let Some(value) = filters.started_before {
        push_condition("request_started_at <= ?", value);
    }
    if filters.valid_only {
        append_condition(query, &mut has_where, "invalidated_at IS NULL");
    }
    append_request_kind_exclusion(
        query,
        query_params,
        &mut has_where,
        filters.exclude_request_kinds,
    );
}

/// Builds production LLM audit SELECT SQL for EXPLAIN QUERY PLAN regression tests.
#[doc(hidden)]
pub fn llm_request_audit_rows_sql_for_tests(filters: LlmRequestAuditFilters<'_>) -> String {
    let mut query = String::from(
        "SELECT
                id, workspace_id, chat_id, request_kind, provider_id, model_id, thinking_level,
                request_started_at, first_token_at, completed_at, input_tokens, output_tokens,
                cache_read_tokens, cache_write_tokens, reasoning_tokens, cache_ratio,
                first_token_latency_ms, total_latency_ms, status_code, final_state,
                invalidated_at, invalidated_reason
             FROM llm_requests",
    );
    let mut query_params = Vec::new();
    append_llm_request_audit_where_clause(&mut query, &mut query_params, filters);
    query.push_str(" ORDER BY request_started_at DESC, id DESC LIMIT ? OFFSET ?");
    let _ = query_params;
    query
}

/// Builds production LLM audit COUNT SQL for EXPLAIN QUERY PLAN regression tests.
#[doc(hidden)]
pub fn llm_request_audit_count_sql_for_tests(filters: LlmRequestAuditFilters<'_>) -> String {
    let mut query = String::from("SELECT COUNT(*) FROM llm_requests");
    let mut query_params = Vec::new();
    append_llm_request_audit_where_clause(&mut query, &mut query_params, filters);
    let _ = query_params;
    query
}

/// Builds production LLM audit summary SQL for EXPLAIN QUERY PLAN regression tests.
#[doc(hidden)]
pub fn llm_request_audit_summary_sql_for_tests(filters: LlmRequestAuditFilters<'_>) -> String {
    let mut query = String::from(
        "SELECT
                COUNT(*),
                COUNT(CASE WHEN final_state NOT IN ('succeeded', 'completed') THEN 1 END),
                COALESCE(SUM(COALESCE(input_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(output_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(cache_write_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)), 0),
                COUNT(total_latency_ms),
                COALESCE(SUM(COALESCE(total_latency_ms, 0)), 0)
             FROM llm_requests",
    );
    let mut query_params = Vec::new();
    append_llm_request_audit_where_clause(&mut query, &mut query_params, filters);
    let _ = query_params;
    query
}

/// Builds production requestKind breakdown SQL for EXPLAIN QUERY PLAN regression tests.
#[doc(hidden)]
pub fn llm_request_audit_request_kind_breakdown_sql_for_tests(
    filters: LlmRequestAuditFilters<'_>,
) -> String {
    let mut query = String::from(
        "SELECT
                request_kind,
                COUNT(*),
                COUNT(CASE WHEN final_state NOT IN ('succeeded', 'completed') THEN 1 END),
                COALESCE(SUM(COALESCE(input_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(output_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(cache_write_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(reasoning_tokens, 0)), 0),
                COALESCE(SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)), 0),
                COUNT(total_latency_ms),
                COALESCE(SUM(COALESCE(total_latency_ms, 0)), 0)
             FROM llm_requests",
    );
    let mut query_params = Vec::new();
    append_llm_request_audit_where_clause(&mut query, &mut query_params, filters);
    query.push_str(" GROUP BY request_kind ORDER BY request_kind");
    let _ = query_params;
    query
}

/// Builds production scheduled task list SQL for EXPLAIN QUERY PLAN regression tests.
#[doc(hidden)]
pub fn scheduled_tasks_page_sql_for_tests(
    status: Option<&str>,
    search: Option<&str>,
) -> Result<String, WorkspaceDatabaseError> {
    let (where_clause, _) = scheduled_task_filter_sql(status, search)?;
    Ok(format!(
        "SELECT id, title, description, schedule_json, action_json, status,
                    next_run_at, last_run_at, created_at, updated_at, metadata_json
             FROM scheduled_tasks{where_clause}
             ORDER BY
                CASE WHEN next_run_at IS NULL THEN 1 ELSE 0 END,
                next_run_at ASC,
                updated_at DESC,
                id ASC
              LIMIT ? OFFSET ?"
    ))
}

/// Builds production scheduled task count SQL for EXPLAIN QUERY PLAN regression tests.
#[doc(hidden)]
pub fn scheduled_task_count_sql_for_tests(
    status: Option<&str>,
    search: Option<&str>,
) -> Result<String, WorkspaceDatabaseError> {
    let (where_clause, _) = scheduled_task_filter_sql(status, search)?;
    Ok(format!(
        "SELECT COUNT(*) FROM scheduled_tasks{where_clause}"
    ))
}

fn append_llm_request_usage_rollup_where_clause(
    query: &mut String,
    query_params: &mut Vec<SqlValue>,
    filters: LlmRequestUsageRollupFilters<'_>,
) {
    let mut has_where = false;
    let mut push_condition = |condition: &str, value: &str| {
        query.push_str(if has_where { " AND " } else { " WHERE " });
        query.push_str(condition);
        query_params.push(SqlValue::Text(value.to_string()));
        has_where = true;
    };

    if let Some(value) = filters.workspace_id {
        push_condition("workspace_id = ?", value);
    }
    if let Some(value) = filters.provider_id {
        push_condition("provider_id = ?", value);
    }
    if let Some(value) = filters.model_id {
        push_condition("model_id = ?", value);
    }
    if let Some(value) = filters.final_state {
        push_condition("final_state = ?", value);
    }
    if let Some(value) = filters.bucket_after {
        push_condition("bucket_date >= ?", value);
    }
    if let Some(value) = filters.bucket_before {
        push_condition("bucket_date <= ?", value);
    }
}
fn sqlite_error(database_path: &Path, source: rusqlite::Error) -> WorkspaceDatabaseError {
    WorkspaceDatabaseError::Sqlite {
        path: database_path.to_path_buf(),
        source,
    }
}

fn query_u64_pragma(
    connection: &Connection,
    database_path: &Path,
    pragma: &str,
) -> Result<u64, WorkspaceDatabaseError> {
    let value: i64 = connection
        .query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))
        .map_err(|source| sqlite_error(database_path, source))?;
    u64::try_from(value).map_err(|_| WorkspaceDatabaseError::InvalidAuditData {
        message: format!("workspace database PRAGMA {pragma} returned a negative value"),
    })
}

fn open_connection(database_path: &Path) -> Result<Connection, WorkspaceDatabaseError> {
    prepare_private_file(database_path).map_err(|source| WorkspaceDatabaseError::Io {
        path: database_path.to_path_buf(),
        source,
    })?;
    let connection =
        Connection::open(database_path).map_err(|source| WorkspaceDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;

    connection
        .busy_timeout(WORKSPACE_DATABASE_BUSY_TIMEOUT)
        .map_err(|source| WorkspaceDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|source| sqlite_error(database_path, source))?;

    Ok(connection)
}

fn enable_write_ahead_logging(
    connection: &Connection,
    database_path: &Path,
) -> Result<(), WorkspaceDatabaseError> {
    let journal_mode: String = connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(|source| sqlite_error(database_path, source))?;
    if journal_mode.eq_ignore_ascii_case("wal") {
        return Ok(());
    }

    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|source| sqlite_error(database_path, source))
}

fn run_migrations(
    connection: &mut Connection,
    database_path: &Path,
    database_existed: bool,
) -> Result<(), WorkspaceDatabaseError> {
    let current_version = schema_version(connection, database_path)?;

    if current_version > WORKSPACE_SCHEMA_VERSION {
        return Err(WorkspaceDatabaseError::UnsupportedSchemaVersion {
            path: database_path.to_path_buf(),
            found: current_version,
            latest: WORKSPACE_SCHEMA_VERSION,
        });
    }

    if current_version == WORKSPACE_SCHEMA_VERSION {
        return Ok(());
    }

    // VACUUM INTO cannot run inside a transaction, so backup + Immediate migration must be
    // owned by a process-wide lock keyed by the database file path.
    let _migration_lock = acquire_workspace_migration_lock(database_path)?;
    let current_version = schema_version(connection, database_path)?;

    if current_version > WORKSPACE_SCHEMA_VERSION {
        return Err(WorkspaceDatabaseError::UnsupportedSchemaVersion {
            path: database_path.to_path_buf(),
            found: current_version,
            latest: WORKSPACE_SCHEMA_VERSION,
        });
    }

    if current_version == WORKSPACE_SCHEMA_VERSION {
        return Ok(());
    }

    if database_existed && has_user_schema(connection, database_path)? {
        create_migration_backup(connection, database_path, current_version)?;
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|source| WorkspaceDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;
    let current_version = schema_version(&transaction, database_path)?;

    if current_version > WORKSPACE_SCHEMA_VERSION {
        return Err(WorkspaceDatabaseError::UnsupportedSchemaVersion {
            path: database_path.to_path_buf(),
            found: current_version,
            latest: WORKSPACE_SCHEMA_VERSION,
        });
    }

    if current_version == WORKSPACE_SCHEMA_VERSION {
        transaction
            .commit()
            .map_err(|source| WorkspaceDatabaseError::Sqlite {
                path: database_path.to_path_buf(),
                source,
            })?;
        return Ok(());
    }

    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current_version)
    {
        let skip_migration = match migration.version {
            27 => {
                !table_exists(&transaction, database_path, "llm_requests")?
                    || table_has_column(
                        &transaction,
                        database_path,
                        "llm_requests",
                        "reasoning_tokens",
                    )?
            }
            28 => {
                !table_exists(&transaction, database_path, "llm_requests")?
                    || table_has_column(
                        &transaction,
                        database_path,
                        "llm_requests",
                        "request_kind",
                    )?
            }
            29 => {
                !table_exists(&transaction, database_path, "workspace_spec_jobs")?
                    || table_has_column(
                        &transaction,
                        database_path,
                        "workspace_spec_jobs",
                        "chat_id",
                    )?
            }
            30 => {
                !table_exists(&transaction, database_path, "prompt_context_injections")?
                    || table_has_column(
                        &transaction,
                        database_path,
                        "prompt_context_injections",
                        "memory_summaries_json",
                    )?
            }
            31 => {
                !table_exists(&transaction, database_path, "memory_facts")?
                    || table_has_column(&transaction, database_path, "memory_facts", "enabled")?
            }
            32 => {
                !table_exists(&transaction, database_path, "llm_requests")?
                    || table_has_column(
                        &transaction,
                        database_path,
                        "llm_requests",
                        "invalidated_at",
                    )?
            }
            33 => table_exists(&transaction, database_path, "plan_phase_derived_effects")?,
            34 => {
                !table_exists(&transaction, database_path, "plan_phase_derived_effects")?
                    || table_has_columns(
                        &transaction,
                        database_path,
                        "plan_phase_derived_effects",
                        &["integration_confirmed_at", "terminal_reason"],
                    )?
            }
            35 => !table_exists(&transaction, database_path, "workspace_metadata")?,
            36 => {
                !table_exists(&transaction, database_path, "llm_requests")?
                    || table_has_column(
                        &transaction,
                        database_path,
                        "llm_requests",
                        "thinking_level",
                    )?
            }
            39 => {
                !table_exists(&transaction, database_path, "workspace_specs")?
                    || !table_exists(&transaction, database_path, "chat_spec_snapshots")?
            }
            _ => false,
        };
        if skip_migration {
            transaction
                .pragma_update(None, "user_version", migration.version)
                .map_err(|source| WorkspaceDatabaseError::Sqlite {
                    path: database_path.to_path_buf(),
                    source,
                })?;
            continue;
        }

        transaction.execute_batch(migration.sql).map_err(|source| {
            WorkspaceDatabaseError::Sqlite {
                path: database_path.to_path_buf(),
                source,
            }
        })?;
        if migration.version == 22
            && table_has_columns(
                &transaction,
                database_path,
                "llm_requests",
                &[
                    "workspace_id",
                    "request_started_at",
                    "provider_id",
                    "model_id",
                    "final_state",
                    "input_tokens",
                    "output_tokens",
                    "cache_read_tokens",
                    "cache_write_tokens",
                    "total_latency_ms",
                ],
            )?
        {
            transaction
                .execute_batch(MIGRATION_022_BACKFILL)
                .map_err(|source| WorkspaceDatabaseError::Sqlite {
                    path: database_path.to_path_buf(),
                    source,
                })?;
        }
        transaction
            .pragma_update(None, "user_version", migration.version)
            .map_err(|source| WorkspaceDatabaseError::Sqlite {
                path: database_path.to_path_buf(),
                source,
            })?;
    }

    transaction
        .commit()
        .map_err(|source| WorkspaceDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;

    Ok(())
}

struct WorkspaceMigrationLock {
    _file: fs::File,
}

fn acquire_workspace_migration_lock(
    database_path: &Path,
) -> Result<WorkspaceMigrationLock, WorkspaceDatabaseError> {
    let lock_path = workspace_migration_lock_path(database_path);
    if let Some(parent) = lock_path.parent() {
        create_directory(parent)?;
    }
    prepare_private_file(&lock_path).map_err(|source| WorkspaceDatabaseError::Io {
        path: lock_path.clone(),
        source,
    })?;

    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|source| WorkspaceDatabaseError::Io {
            path: lock_path.clone(),
            source,
        })?;
    lock_file_exclusive(&file).map_err(|source| WorkspaceDatabaseError::Io {
        path: lock_path,
        source,
    })?;

    Ok(WorkspaceMigrationLock { _file: file })
}

fn workspace_migration_lock_path(database_path: &Path) -> PathBuf {
    let resolved = database_path
        .canonicalize()
        .unwrap_or_else(|_| database_path.to_path_buf());
    let file_name = resolved
        .file_name()
        .map(|name| {
            let mut lock_name = name.to_os_string();
            lock_name.push(WORKSPACE_MIGRATION_LOCK_SUFFIX);
            lock_name
        })
        .unwrap_or_else(|| format!("foco.sqlite{WORKSPACE_MIGRATION_LOCK_SUFFIX}").into());
    resolved.with_file_name(file_name)
}

#[cfg(unix)]
fn lock_file_exclusive(file: &fs::File) -> io::Result<()> {
    // LOCK_EX = 2
    let result = libc_flock(file.as_raw_fd(), 2);
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int;
}

#[cfg(unix)]
fn libc_flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int {
    unsafe { flock(fd, operation) }
}

#[cfg(windows)]
fn lock_file_exclusive(file: &fs::File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use std::ptr;

    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        h_event: *mut std::ffi::c_void,
    }

    unsafe extern "system" {
        fn LockFileEx(
            h_file: *mut std::ffi::c_void,
            dw_flags: u32,
            dw_reserved: u32,
            n_number_of_bytes_to_lock_low: u32,
            n_number_of_bytes_to_lock_high: u32,
            lp_overlapped: *mut Overlapped,
        ) -> i32;
    }

    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x0000_0002;
    let mut overlapped = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: 0,
        offset_high: 0,
        h_event: ptr::null_mut(),
    };
    let ok = unsafe {
        LockFileEx(
            file.as_raw_handle(),
            LOCKFILE_EXCLUSIVE_LOCK,
            0,
            1,
            0,
            &mut overlapped,
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(unix, windows)))]
fn lock_file_exclusive(_file: &fs::File) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "workspace database migration lock is not supported on this platform",
    ))
}

fn table_exists(
    connection: &Connection,
    database_path: &Path,
    table: &str,
) -> Result<bool, WorkspaceDatabaseError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    Ok(count > 0)
}

fn table_has_columns(
    connection: &Connection,
    database_path: &Path,
    table: &str,
    required_columns: &[&str],
) -> Result<bool, WorkspaceDatabaseError> {
    if !table_exists(connection, database_path, table)? {
        return Ok(false);
    }

    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|source| sqlite_error(database_path, source))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|source| sqlite_error(database_path, source))?;
    let columns = rows
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|source| sqlite_error(database_path, source))?;

    Ok(required_columns
        .iter()
        .all(|column| columns.contains(*column)))
}

fn table_has_column(
    connection: &Connection,
    database_path: &Path,
    table: &str,
    column: &str,
) -> Result<bool, WorkspaceDatabaseError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|source| sqlite_error(database_path, source))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|source| sqlite_error(database_path, source))?;

    for row in rows {
        if row.map_err(|source| sqlite_error(database_path, source))? == column {
            return Ok(true);
        }
    }

    Ok(false)
}
fn schema_version(
    connection: &Connection,
    database_path: &Path,
) -> Result<u32, WorkspaceDatabaseError> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|source| WorkspaceDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })
}

fn has_user_schema(
    connection: &Connection,
    database_path: &Path,
) -> Result<bool, WorkspaceDatabaseError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*)
             FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%'
               AND type IN ('table', 'index', 'trigger', 'view')",
            [],
            |row| row.get(0),
        )
        .map_err(|source| WorkspaceDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;

    Ok(count > 0)
}

fn create_migration_backup(
    connection: &Connection,
    database_path: &Path,
    current_version: u32,
) -> Result<(), WorkspaceDatabaseError> {
    let parent =
        database_path
            .parent()
            .ok_or_else(|| WorkspaceDatabaseError::MissingDatabaseParent {
                path: database_path.to_path_buf(),
            })?;
    let backup_dir = parent.join("backups");

    create_directory(&backup_dir)?;

    let timestamp = Utc::now().format("%Y%m%dT%H%M%S%fZ");
    let backup_path = backup_dir.join(format!("foco-v{current_version}-{timestamp}.sqlite"));
    let backup_path_text =
        backup_path
            .to_str()
            .ok_or_else(|| WorkspaceDatabaseError::NonUtf8Path {
                path: backup_path.clone(),
            })?;

    connection
        .execute("VACUUM main INTO ?1", params![backup_path_text])
        .map_err(|source| WorkspaceDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;
    restrict_private_file(&backup_path).map_err(|source| WorkspaceDatabaseError::Io {
        path: backup_path.clone(),
        source,
    })?;

    if let Some(workspace_path) = parent.parent()
        && let Err(error) = prune_workspace_database_backups(workspace_path)
    {
        tracing::warn!(
            workspace_path = %workspace_path.display(),
            backup_dir = %backup_dir.display(),
            error = %error,
            "workspace database backup pruning skipped"
        );
    }

    Ok(())
}

fn validate_json_metadata(
    metadata_json: &str,
    context: &str,
) -> Result<(), WorkspaceDatabaseError> {
    let _ = parse_json_object(metadata_json, context)?;
    Ok(())
}

fn apply_message_metadata_mutation(
    metadata: &mut serde_json::Map<String, Value>,
    mutation: MessageMetadataMutation,
) -> Result<(), WorkspaceDatabaseError> {
    match mutation {
        MessageMetadataMutation::MergeFields { fields } => {
            for (key, value) in fields {
                metadata.insert(key, value);
            }
        }
        MessageMetadataMutation::SetParts {
            parts,
            parts_version,
            parts_source,
        } => {
            if !parts.is_array() {
                return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: "message metadata.parts must be a JSON array".to_string(),
                });
            }
            if parts_source.trim().is_empty() {
                return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: "message metadata.partsSource must not be empty".to_string(),
                });
            }
            metadata.insert("parts".to_string(), parts);
            metadata.insert(
                "partsVersion".to_string(),
                Value::Number(parts_version.into()),
            );
            metadata.insert("partsSource".to_string(), Value::String(parts_source));
        }
        MessageMetadataMutation::UpsertSpecUpdate { summary } => {
            let Some(summary_object) = summary.as_object() else {
                return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: "message metadata.specUpdates entry must be a JSON object".to_string(),
                });
            };
            let Some(summary_id) = summary_object.get("id").and_then(Value::as_str) else {
                return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: "message metadata.specUpdates entry must include id".to_string(),
                });
            };
            if summary_id.trim().is_empty() {
                return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: "message metadata.specUpdates entry id must not be empty".to_string(),
                });
            }
            let mut updates = match metadata.get("specUpdates") {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::Array(items)) => items.clone(),
                Some(_) => {
                    return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                        message: "message metadata.specUpdates must be a JSON array".to_string(),
                    });
                }
            };
            if let Some(existing) = updates.iter_mut().find(|item| {
                item.get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id == summary_id)
            }) {
                *existing = summary;
            } else {
                updates.push(summary);
            }
            metadata.insert("specUpdates".to_string(), Value::Array(updates));
        }
        MessageMetadataMutation::RemoveKey { key } => {
            if key.trim().is_empty() {
                return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: "message metadata key must not be empty".to_string(),
                });
            }
            metadata.remove(&key);
        }
        MessageMetadataMutation::MergeNestedObjectFields { key, fields } => {
            if key.trim().is_empty() {
                return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: "message metadata key must not be empty".to_string(),
                });
            }
            match metadata.get_mut(&key) {
                None | Some(Value::Null) => {}
                Some(Value::Object(nested)) => {
                    for (field, value) in fields {
                        nested.insert(field, value);
                    }
                }
                Some(_) => {
                    return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                        message: format!("message metadata.{key} must be a JSON object"),
                    });
                }
            }
        }
    }
    Ok(())
}

fn parse_json_object(
    metadata_json: &str,
    context: &str,
) -> Result<serde_json::Map<String, Value>, WorkspaceDatabaseError> {
    let value = serde_json::from_str::<Value>(metadata_json).map_err(|source| {
        WorkspaceDatabaseError::InvalidMessageMetadata {
            message: format!("{context} is invalid JSON: {source}"),
        }
    })?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| WorkspaceDatabaseError::InvalidMessageMetadata {
            message: format!("{context} must be a JSON object"),
        })
}

fn chat_from_transaction(
    transaction: &Transaction<'_>,
    database_path: &Path,
    id: &str,
) -> Result<Option<ChatRecord>, WorkspaceDatabaseError> {
    transaction
        .query_row(
            "SELECT id, title, created_at, updated_at, archived_at, metadata_json
             FROM chats
             WHERE id = ?1",
            params![id],
            chat_from_row,
        )
        .optional()
        .map_err(|source| sqlite_error(database_path, source))
}

fn message_from_transaction(
    transaction: &Transaction<'_>,
    database_path: &Path,
    id: &str,
) -> Result<Option<MessageRecord>, WorkspaceDatabaseError> {
    transaction
        .query_row(
            "SELECT id, chat_id, role, content, sequence, created_at, metadata_json
             FROM messages
             WHERE id = ?1",
            params![id],
            |row| {
                Ok(MessageRecord {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    sequence: row.get(4)?,
                    created_at: row.get(5)?,
                    metadata_json: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(|source| sqlite_error(database_path, source))
}

fn chat_from_row(row: &Row<'_>) -> rusqlite::Result<ChatRecord> {
    Ok(ChatRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
        archived_at: row.get(4)?,
        metadata_json: row.get(5)?,
    })
}

fn like_contains_pattern(query: &str) -> String {
    let mut pattern = String::from("%");
    for character in query.chars() {
        match character {
            '%' | '_' | '\\' => {
                pattern.push('\\');
                pattern.push(character);
            }
            _ => pattern.push(character),
        }
    }
    pattern.push('%');
    pattern
}

fn create_directory(path: &Path) -> Result<(), WorkspaceDatabaseError> {
    create_private_dir_all(path).map_err(|source| WorkspaceDatabaseError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
    database_path: &Path,
) -> Result<Vec<T>, WorkspaceDatabaseError> {
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|source| WorkspaceDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })
}

fn now_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SqlitePragmaOptimizeThrottle {
    /// Persist last success under `workspace_metadata` and process-local map.
    WorkspaceMetadata,
    /// Process-local map only (Global Memory has no workspace_metadata table).
    ProcessLocalOnly,
}

pub(crate) fn maybe_run_sqlite_pragma_optimize(
    connection: &mut Connection,
    database_path: &Path,
    throttle: SqlitePragmaOptimizeThrottle,
    force: bool,
) -> Result<bool, rusqlite::Error> {
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    static LAST_PROCESS_RUN: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    let path_key = database_path.to_string_lossy().into_owned();
    let now_instant = Instant::now();
    let min_interval = Duration::from_secs(SQLITE_PRAGMA_OPTIMIZE_MIN_INTERVAL_SECS);

    if !force {
        let map = LAST_PROCESS_RUN.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(guard) = map.lock() {
            if let Some(last) = guard.get(&path_key) {
                if now_instant.duration_since(*last) < min_interval {
                    return Ok(false);
                }
            }
        }

        if matches!(throttle, SqlitePragmaOptimizeThrottle::WorkspaceMetadata) {
            let last_at: Option<String> = connection
                .query_row(
                    "SELECT value FROM workspace_metadata WHERE key = ?1",
                    params![SQLITE_PRAGMA_OPTIMIZE_LAST_AT_KEY],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(last_at) = last_at {
                if let Ok(last) = DateTime::parse_from_rfc3339(&last_at) {
                    let elapsed = Utc::now().signed_duration_since(last.with_timezone(&Utc));
                    if elapsed
                        < chrono::Duration::seconds(
                            i64::try_from(SQLITE_PRAGMA_OPTIMIZE_MIN_INTERVAL_SECS)
                                .unwrap_or(i64::MAX),
                        )
                    {
                        return Ok(false);
                    }
                }
            }
        }
    }

    connection.execute_batch("PRAGMA optimize")?;

    if matches!(throttle, SqlitePragmaOptimizeThrottle::WorkspaceMetadata) {
        let updated_at = now_timestamp();
        connection.execute(
            "INSERT INTO workspace_metadata (key, value, updated_at)
             VALUES (?1, ?2, ?2)
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            params![SQLITE_PRAGMA_OPTIMIZE_LAST_AT_KEY, updated_at],
        )?;
    }

    if let Ok(mut guard) = LAST_PROCESS_RUN
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        guard.insert(path_key, Instant::now());
    }

    Ok(true)
}

fn validate_llm_request_tokens(request: &NewLlmRequest<'_>) -> Result<(), WorkspaceDatabaseError> {
    if request.request_kind.trim().is_empty() {
        return Err(WorkspaceDatabaseError::InvalidAuditData {
            message: "request_kind must be non-empty".to_string(),
        });
    }

    validate_llm_token_values(
        request.input_tokens,
        request.output_tokens,
        request.cache_read_tokens,
        request.cache_write_tokens,
        request.reasoning_tokens,
    )
}

fn validate_llm_token_values(
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
    reasoning_tokens: Option<i64>,
) -> Result<(), WorkspaceDatabaseError> {
    for (name, value) in [
        ("input_tokens", input_tokens),
        ("output_tokens", output_tokens),
        ("cache_read_tokens", cache_read_tokens),
        ("cache_write_tokens", cache_write_tokens),
        ("reasoning_tokens", reasoning_tokens),
    ] {
        if let Some(value) = value
            && value < 0
        {
            return Err(WorkspaceDatabaseError::InvalidAuditTokens {
                message: format!("{name} must be non-negative, got {value}"),
            });
        }
    }

    if let (Some(input_tokens), Some(cache_read_tokens)) = (input_tokens, cache_read_tokens) {
        if input_tokens == 0 && cache_read_tokens > 0 {
            return Err(WorkspaceDatabaseError::InvalidAuditTokens {
                message: "cache_read_tokens cannot be positive when input_tokens is zero"
                    .to_string(),
            });
        }

        if cache_read_tokens > input_tokens {
            return Err(WorkspaceDatabaseError::InvalidAuditTokens {
                message: format!(
                    "cache_read_tokens ({cache_read_tokens}) cannot exceed input_tokens ({input_tokens})"
                ),
            });
        }
    }

    Ok(())
}

fn calculate_cache_ratio(
    input_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
) -> Result<Option<f64>, WorkspaceDatabaseError> {
    match (input_tokens, cache_read_tokens) {
        (Some(input_tokens), Some(cache_read_tokens)) if input_tokens > 0 => {
            if cache_read_tokens > input_tokens {
                return Err(WorkspaceDatabaseError::InvalidAuditTokens {
                    message: format!(
                        "cache_read_tokens ({cache_read_tokens}) cannot exceed input_tokens ({input_tokens})"
                    ),
                });
            }

            Ok(Some(cache_read_tokens as f64 / input_tokens as f64))
        }
        (Some(_), Some(_)) => Ok(None),
        _ => Ok(None),
    }
}

fn redact_optional_audit_json(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, WorkspaceDatabaseError> {
    value.map(|json| redact_audit_json(json, field)).transpose()
}

fn normalize_audit_detail_for_write(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, WorkspaceDatabaseError> {
    match value {
        None => Ok(None),
        Some(json) => {
            validate_audit_detail_format(json, field)?;
            Ok(Some(redact_audit_json(json, field)?))
        }
    }
}

struct PreparedLlmRequestEvent<'a> {
    id: &'a str,
    llm_request_id: &'a str,
    sequence: i64,
    event_at: &'a str,
    event_type: &'a str,
    raw_chunk_json: Option<String>,
    normalized_event_json: String,
}

fn prepare_llm_request_event<'a>(
    event: &NewLlmRequestEvent<'a>,
) -> Result<PreparedLlmRequestEvent<'a>, WorkspaceDatabaseError> {
    Ok(PreparedLlmRequestEvent {
        id: event.id,
        llm_request_id: event.llm_request_id,
        sequence: event.sequence,
        event_at: event.event_at,
        event_type: event.event_type,
        raw_chunk_json: redact_optional_audit_json(event.raw_chunk_json, "raw_chunk_json")?,
        normalized_event_json: redact_audit_json(
            event.normalized_event_json,
            "normalized_event_json",
        )?,
    })
}

fn insert_prepared_llm_request_event(
    connection: &Connection,
    database_path: &Path,
    event: &PreparedLlmRequestEvent<'_>,
) -> Result<(), WorkspaceDatabaseError> {
    connection
        .execute(
            "INSERT INTO llm_request_events
                (
                    id, llm_request_id, sequence, event_at, event_type,
                    raw_chunk_json, normalized_event_json
                )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(llm_request_id, sequence) DO NOTHING",
            params![
                event.id,
                event.llm_request_id,
                event.sequence,
                event.event_at,
                event.event_type,
                event.raw_chunk_json.as_deref(),
                event.normalized_event_json.as_str()
            ],
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    Ok(())
}

fn validate_llm_request_outcome(
    outcome: &UpdateLlmRequestOutcome<'_>,
) -> Result<Option<f64>, WorkspaceDatabaseError> {
    validate_llm_token_values(
        outcome.input_tokens,
        outcome.output_tokens,
        outcome.cache_read_tokens,
        outcome.cache_write_tokens,
        outcome.reasoning_tokens,
    )?;
    calculate_cache_ratio(outcome.input_tokens, outcome.cache_read_tokens)
}

fn update_llm_request_outcome_in_transaction(
    transaction: &Transaction<'_>,
    database_path: &Path,
    id: &str,
    outcome: &UpdateLlmRequestOutcome<'_>,
    cache_ratio: Option<f64>,
) -> Result<(), WorkspaceDatabaseError> {
    let old_request = select_llm_request_record(transaction, id)
        .map_err(|source| sqlite_error(database_path, source))?
        .ok_or_else(|| WorkspaceDatabaseError::MissingLlmRequest { id: id.to_string() })?;
    let response_body_json = merge_audit_detail_for_update(
        old_request.response_body_json.as_deref(),
        outcome.response_body_json,
        "response_body_json",
    )?;

    let updated = transaction
        .execute(
            "UPDATE llm_requests
             SET first_token_at = ?2,
                 completed_at = ?3,
                 input_tokens = ?4,
                 output_tokens = ?5,
                 cache_read_tokens = ?6,
                 cache_write_tokens = ?7,
                 reasoning_tokens = ?8,
                 cache_ratio = ?9,
                 first_token_latency_ms = ?10,
                 total_latency_ms = ?11,
                 status_code = ?12,
                 final_state = ?13,
                 response_body_json = ?14
             WHERE id = ?1",
            params![
                id,
                outcome.first_token_at,
                outcome.completed_at,
                outcome.input_tokens,
                outcome.output_tokens,
                outcome.cache_read_tokens,
                outcome.cache_write_tokens,
                outcome.reasoning_tokens,
                cache_ratio,
                outcome.first_token_latency_ms,
                outcome.total_latency_ms,
                outcome.status_code,
                outcome.final_state,
                response_body_json
            ],
        )
        .map_err(|source| sqlite_error(database_path, source))?;

    if updated == 0 {
        return Err(WorkspaceDatabaseError::MissingLlmRequest { id: id.to_string() });
    }

    if old_request.invalidated_at.is_none() {
        apply_llm_request_usage_rollup_delta(
            transaction,
            database_path,
            llm_request_usage_rollup_delta(llm_request_record_rollup_source(&old_request), -1),
        )?;
        apply_llm_request_usage_rollup_delta(
            transaction,
            database_path,
            llm_request_usage_rollup_delta(
                LlmRequestUsageRollupSource {
                    workspace_id: old_request.workspace_id.as_deref(),
                    provider_id: old_request.provider_id.as_str(),
                    model_id: old_request.model_id.as_str(),
                    request_started_at: old_request.request_started_at.as_str(),
                    final_state: outcome.final_state,
                    input_tokens: outcome.input_tokens,
                    output_tokens: outcome.output_tokens,
                    cache_read_tokens: outcome.cache_read_tokens,
                    cache_write_tokens: outcome.cache_write_tokens,
                    total_latency_ms: outcome.total_latency_ms,
                },
                1,
            ),
        )?;
    }

    Ok(())
}

/// Format-aware CAS for audit detail columns.
///
/// - Real v1 may replace NULL / non-v1 legacy values.
/// - Existing valid v1 is preserved (first capture wins).
/// - Incoming non-v1 JSON is rejected (callers must not write legacy dumps).
/// - Incoming NULL does not clear existing valid v1; it clears non-v1 leftovers.
fn merge_audit_detail_for_update(
    existing: Option<&str>,
    incoming: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, WorkspaceDatabaseError> {
    let existing_valid = existing
        .filter(|value| is_valid_audit_detail_format(value, field))
        .map(|value| redact_audit_json(value, field))
        .transpose()?;

    if let Some(existing_valid) = existing_valid {
        return Ok(Some(existing_valid));
    }

    match incoming {
        None => Ok(None),
        Some(json) => {
            validate_audit_detail_format(json, field)?;
            Ok(Some(redact_audit_json(json, field)?))
        }
    }
}

fn prune_non_v1_llm_audit_details_once(
    connection: &mut Connection,
    database_path: &Path,
) -> Result<(), WorkspaceDatabaseError> {
    if !table_exists(connection, database_path, "workspace_metadata")?
        || !table_has_columns(
            connection,
            database_path,
            "llm_requests",
            &["request_body_json", "response_body_json"],
        )?
    {
        return Ok(());
    }

    let already_pruned: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspace_metadata WHERE key = ?1)",
            params![LLM_AUDIT_DETAIL_V1_PRUNED_KEY],
            |row| row.get(0),
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    if already_pruned {
        return Ok(());
    }

    // PERF: The audit columns can contain gigabytes of provider payloads. Serialize this
    // one-time cleanup and persist completion so normal database opens remain read-only.
    connection
        .busy_timeout(Duration::from_millis(0))
        .map_err(|source| sqlite_error(database_path, source))?;
    let prune_result = (|| -> Result<(), rusqlite::Error> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let already_pruned: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM workspace_metadata WHERE key = ?1)",
            params![LLM_AUDIT_DETAIL_V1_PRUNED_KEY],
            |row| row.get(0),
        )?;
        if !already_pruned {
            transaction.execute_batch(
                r#"
            UPDATE llm_requests
            SET request_body_json = NULL
            WHERE request_body_json IS NOT NULL
              AND (
                json_valid(request_body_json) = 0
                OR COALESCE(json_extract(request_body_json, '$.format'), '') <> 'provider_request_v1'
                OR COALESCE(json_extract(request_body_json, '$.version'), 0) <> 1
              );

            UPDATE llm_requests
            SET response_body_json = NULL
            WHERE response_body_json IS NOT NULL
              AND (
                json_valid(response_body_json) = 0
                OR COALESCE(json_extract(response_body_json, '$.format'), '') <> 'provider_final_response_v1'
                OR COALESCE(json_extract(response_body_json, '$.version'), 0) <> 1
              );
            "#,
            )?;
            transaction.execute(
                "INSERT INTO workspace_metadata (key, value, updated_at) VALUES (?1, 'true', ?2)",
                params![LLM_AUDIT_DETAIL_V1_PRUNED_KEY, now_timestamp()],
            )?;
        }
        transaction.commit()
    })();
    connection
        .busy_timeout(WORKSPACE_DATABASE_BUSY_TIMEOUT)
        .map_err(|source| sqlite_error(database_path, source))?;

    match prune_result {
        Ok(()) => Ok(()),
        Err(source) if is_sqlite_busy_error(&source) => Ok(()),
        Err(source) => Err(sqlite_error(database_path, source)),
    }
}

/// One-shot, idempotent backfill of structured `status_code` from retained
/// `provider_final_response_v1` wire dumps. Does not invent status from final_state.
fn repair_llm_request_status_codes_from_v1_once(
    connection: &mut Connection,
    database_path: &Path,
) -> Result<(), WorkspaceDatabaseError> {
    if !table_exists(connection, database_path, "workspace_metadata")?
        || !table_has_columns(
            connection,
            database_path,
            "llm_requests",
            &["status_code", "response_body_json", "final_state"],
        )?
    {
        return Ok(());
    }

    let already_repaired: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspace_metadata WHERE key = ?1)",
            params![LLM_AUDIT_STATUS_CODE_V1_REPAIRED_KEY],
            |row| row.get(0),
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    if already_repaired {
        return Ok(());
    }

    connection
        .busy_timeout(Duration::from_millis(0))
        .map_err(|source| sqlite_error(database_path, source))?;
    let repair_result = (|| -> Result<(), rusqlite::Error> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let already_repaired: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM workspace_metadata WHERE key = ?1)",
            params![LLM_AUDIT_STATUS_CODE_V1_REPAIRED_KEY],
            |row| row.get(0),
        )?;
        if !already_repaired {
            // Prefer http.status; only use failed-envelope statusCode when http is absent.
            // Strict 100–599; never invent 200 from final_state=succeeded.
            transaction.execute_batch(
                r#"
            UPDATE llm_requests
            SET status_code = CAST(
              CASE
                WHEN json_type(json_extract(response_body_json, '$.http.status')) IN ('integer', 'real')
                  AND CAST(json_extract(response_body_json, '$.http.status') AS INTEGER) BETWEEN 100 AND 599
                THEN CAST(json_extract(response_body_json, '$.http.status') AS INTEGER)
                WHEN json_extract(response_body_json, '$.http') IS NULL
                  AND json_type(json_extract(response_body_json, '$.statusCode')) IN ('integer', 'real')
                  AND CAST(json_extract(response_body_json, '$.statusCode') AS INTEGER) BETWEEN 100 AND 599
                THEN CAST(json_extract(response_body_json, '$.statusCode') AS INTEGER)
                ELSE NULL
              END AS INTEGER
            )
            WHERE status_code IS NULL
              AND final_state IS NOT NULL
              AND final_state <> 'running'
              AND response_body_json IS NOT NULL
              AND json_valid(response_body_json) = 1
              AND COALESCE(json_extract(response_body_json, '$.format'), '') = 'provider_final_response_v1'
              AND COALESCE(json_extract(response_body_json, '$.version'), 0) = 1
              AND (
                (
                  json_type(json_extract(response_body_json, '$.http.status')) IN ('integer', 'real')
                  AND CAST(json_extract(response_body_json, '$.http.status') AS INTEGER) BETWEEN 100 AND 599
                )
                OR (
                  json_extract(response_body_json, '$.http') IS NULL
                  AND json_type(json_extract(response_body_json, '$.statusCode')) IN ('integer', 'real')
                  AND CAST(json_extract(response_body_json, '$.statusCode') AS INTEGER) BETWEEN 100 AND 599
                )
              );
            "#,
            )?;
            transaction.execute(
                "INSERT INTO workspace_metadata (key, value, updated_at) VALUES (?1, 'true', ?2)",
                params![LLM_AUDIT_STATUS_CODE_V1_REPAIRED_KEY, now_timestamp()],
            )?;
        }
        transaction.commit()
    })();
    connection
        .busy_timeout(WORKSPACE_DATABASE_BUSY_TIMEOUT)
        .map_err(|source| sqlite_error(database_path, source))?;

    match repair_result {
        Ok(()) => Ok(()),
        Err(source) if is_sqlite_busy_error(&source) => Ok(()),
        Err(source) => Err(sqlite_error(database_path, source)),
    }
}

fn is_sqlite_busy_error(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::DatabaseBusy
                || code.code == rusqlite::ErrorCode::DatabaseLocked
    )
}

fn is_valid_audit_detail_format(value: &str, field: &'static str) -> bool {
    validate_audit_detail_format(value, field).is_ok()
}

fn validate_audit_detail_format(
    value: &str,
    field: &'static str,
) -> Result<(), WorkspaceDatabaseError> {
    let parsed: Value = serde_json::from_str(value)
        .map_err(|source| WorkspaceDatabaseError::InvalidAuditJson { field, source })?;
    let format = parsed.get("format").and_then(Value::as_str);
    let version = parsed.get("version").and_then(Value::as_u64);
    let ok = match field {
        "request_body_json" => format == Some("provider_request_v1") && version == Some(1),
        "response_body_json" => format == Some("provider_final_response_v1") && version == Some(1),
        _ => false,
    };
    if !ok {
        return Err(WorkspaceDatabaseError::InvalidAuditData {
            message: format!(
                "{field} must be a versioned provider dump (got format={format:?}, version={version:?})"
            ),
        });
    }
    Ok(())
}

fn redact_audit_json(value: &str, field: &'static str) -> Result<String, WorkspaceDatabaseError> {
    let mut parsed: Value = serde_json::from_str(value)
        .map_err(|source| WorkspaceDatabaseError::InvalidAuditJson { field, source })?;

    let format = parsed.get("format").and_then(Value::as_str);
    let version = parsed.get("version").and_then(Value::as_u64);
    match (field, format, version) {
        ("request_body_json", Some("provider_request_v1"), Some(1)) => {
            redact_provider_request_envelope(&mut parsed)
        }
        ("response_body_json", Some("provider_final_response_v1"), Some(1)) => {
            redact_provider_response_envelope(&mut parsed)
        }
        ("request_body_json" | "response_body_json", _, _) => {
            return Err(WorkspaceDatabaseError::InvalidAuditData {
                message: format!(
                    "{field} must be a versioned provider dump (got format={format:?}, version={version:?})"
                ),
            });
        }
        // Event/raw chunk audit JSON still uses recursive secret-key redaction.
        _ => redact_json_value(&mut parsed),
    }

    serde_json::to_string(&parsed)
        .map_err(|source| WorkspaceDatabaseError::InvalidAuditJson { field, source })
}

fn redact_provider_request_envelope(value: &mut Value) {
    let headers = value
        .as_object_mut()
        .and_then(|object| object.remove("headers"));
    redact_json_value(value);
    if let (Some(object), Some(mut headers)) = (value.as_object_mut(), headers) {
        mask_provider_authorization_header(&mut headers);
        object.insert("headers".to_string(), headers);
    }
}

fn redact_provider_response_envelope(value: &mut Value) {
    let headers = value
        .as_object_mut()
        .and_then(|object| object.get_mut("http"))
        .and_then(Value::as_object_mut)
        .and_then(|http| http.remove("headers"));
    redact_json_value(value);
    if let (Some(http), Some(mut headers)) = (
        value
            .as_object_mut()
            .and_then(|object| object.get_mut("http"))
            .and_then(Value::as_object_mut),
        headers,
    ) {
        mask_provider_authorization_header(&mut headers);
        http.insert("headers".to_string(), headers);
    }
}

fn mask_provider_authorization_header(headers: &mut Value) {
    let Some(headers) = headers.as_object_mut() else {
        return;
    };
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("authorization") {
            mask_provider_header_value(value);
        }
    }
}

fn mask_provider_header_value(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                *value = Value::String("********".to_string());
            }
        }
        _ => *value = Value::String("********".to_string()),
    }
}

fn redact_json_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if is_secret_audit_key(key) {
                    *value = Value::String("[REDACTED]".to_string());
                } else {
                    redact_json_value(value);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_json_value(item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn is_secret_audit_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| *character != '-' && *character != '_')
        .flat_map(char::to_lowercase)
        .collect::<String>();

    normalized == "authorization"
        || normalized.contains("apikey")
        || normalized.contains("cookie")
        || normalized.contains("password")
}
