use std::{
    collections::{HashMap, HashSet},
    fmt, fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

use chrono::{SecondsFormat, Utc};
use rusqlite::{
    Connection, OptionalExtension, Transaction, TransactionBehavior, params, params_from_iter,
};
use serde_json::Value;
use serde_json::json;

#[path = "memory_records.rs"]
mod memory_records;
#[path = "memory_schema.rs"]
mod memory_schema;

use crate::private_fs::{create_private_dir_all, prepare_private_file, restrict_sqlite_files};
use memory_records::MemoryDatabaseKind;
pub use memory_records::{
    MemoryDreamChangeRecord, MemoryDreamJobRecord, MemoryDreamJobTransitionOutcome,
    MemoryEdgeRecord, MemoryExtractionJobRecord, MemoryFactCopyOutcome, MemoryFactRecord,
    MemoryProfileRecord, MemoryReferenceRecord, MemorySourceRecord, NewMemoryDreamChange,
    NewMemoryDreamJob, NewMemoryEdge, NewMemoryExtractionJob, NewMemoryFact, NewMemoryProfile,
    NewMemoryReference, NewMemorySource, StartMemoryDreamJobOutcome, UpdateMemoryDreamChange,
    UpdateMemoryDreamJob, UpdateMemoryFact, UpdateMemorySource,
};
use memory_schema::MemoryMigration;
pub use memory_schema::{
    GLOBAL_MEMORY_DREAM_ACTIVE_SINGLEFLIGHT_MIGRATION_SQL, GLOBAL_MEMORY_DREAM_SCHEMA_SQL,
    GLOBAL_MEMORY_EXTRACTION_SKIPPED_STATUS_MIGRATION_SQL, GLOBAL_MEMORY_SCHEMA_SQL,
    MEMORY_FACT_ENABLED_MIGRATION_SQL, MEMORY_REFERENCES_SCHEMA_SQL,
    WORKSPACE_MEMORY_DREAM_SCHEMA_SQL, WORKSPACE_MEMORY_SCHEMA_SQL,
};

pub const GLOBAL_MEMORY_DATABASE_FILE: &str = "memory.sqlite";
pub const GLOBAL_MEMORY_SCHEMA_VERSION: u32 = 6;
const GLOBAL_MEMORY_DATABASE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);
const GLOBAL_MEMORY_MIGRATION_LOCK_SUFFIX: &str = ".migrate.lock";

const GLOBAL_MEMORY_MIGRATIONS: &[MemoryMigration] = &[
    MemoryMigration {
        version: 1,
        sql: GLOBAL_MEMORY_SCHEMA_SQL,
    },
    MemoryMigration {
        version: 2,
        sql: GLOBAL_MEMORY_DREAM_SCHEMA_SQL,
    },
    MemoryMigration {
        version: 3,
        sql: MEMORY_REFERENCES_SCHEMA_SQL,
    },
    MemoryMigration {
        version: 4,
        sql: GLOBAL_MEMORY_EXTRACTION_SKIPPED_STATUS_MIGRATION_SQL,
    },
    MemoryMigration {
        version: 5,
        sql: MEMORY_FACT_ENABLED_MIGRATION_SQL,
    },
    MemoryMigration {
        version: 6,
        sql: GLOBAL_MEMORY_DREAM_ACTIVE_SINGLEFLIGHT_MIGRATION_SQL,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryScope {
    Global,
    Workspace,
    Chat,
}

impl MemoryScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Workspace => "workspace",
            Self::Chat => "chat",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        match value {
            "global" => Ok(Self::Global),
            "workspace" => Ok(Self::Workspace),
            "chat" => Ok(Self::Chat),
            _ => Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!("unknown memory scope: {value}"),
            }),
        }
    }
}

// ponytail: Phase 0 is only the Dream contract; schema, API, and UI wire it in later phases.
pub const MEMORY_DREAM_HARD_DELETE_ALLOWED: bool = false;
pub const MEMORY_DREAM_TRANSCRIPT_CHAT_KIND: &str = "memory_dream";
pub const MEMORY_DREAM_TRANSCRIPT_VISIBLE_IN_NORMAL_CHAT_LIST: bool = false;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryDreamScope {
    Global,
    Workspace,
}

impl MemoryDreamScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Workspace => "workspace",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        match value {
            "global" => Ok(Self::Global),
            "workspace" => Ok(Self::Workspace),
            _ => Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!("unknown memory Dream scope: {value}"),
            }),
        }
    }

    pub fn allows_candidate_fact_scope(self, scope: MemoryScope) -> bool {
        match self {
            Self::Global => scope == MemoryScope::Global,
            Self::Workspace => matches!(scope, MemoryScope::Workspace | MemoryScope::Chat),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryDreamTriggerType {
    Manual,
    AutoInterval,
    AutoThreshold,
}

impl MemoryDreamTriggerType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::AutoInterval => "auto_interval",
            Self::AutoThreshold => "auto_threshold",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        match value {
            "manual" => Ok(Self::Manual),
            "auto_interval" => Ok(Self::AutoInterval),
            "auto_threshold" => Ok(Self::AutoThreshold),
            _ => Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!("unknown memory Dream trigger type: {value}"),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryDreamRunMode {
    DeterministicOnly,
    Llm,
}

impl MemoryDreamRunMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeterministicOnly => "deterministic_only",
            Self::Llm => "llm",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        match value {
            "deterministic_only" => Ok(Self::DeterministicOnly),
            "llm" => Ok(Self::Llm),
            _ => Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!("unknown memory Dream run mode: {value}"),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryDreamJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Skipped,
}

impl MemoryDreamJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "skipped" => Ok(Self::Skipped),
            _ => Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!("unknown memory Dream job status: {value}"),
            }),
        }
    }

    fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Skipped
        )
    }

    fn starts_run(self) -> bool {
        self == Self::Running || self.is_terminal()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryDreamChangeStatus {
    Proposed,
    Applied,
    Skipped,
    Failed,
}

impl MemoryDreamChangeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Applied => "applied",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        match value {
            "proposed" => Ok(Self::Proposed),
            "applied" => Ok(Self::Applied),
            "skipped" => Ok(Self::Skipped),
            "failed" => Ok(Self::Failed),
            _ => Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!("unknown memory Dream change status: {value}"),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryDreamSafetyPolicy {
    pub max_facts_per_run: usize,
    pub max_changes_per_run: usize,
}

impl MemoryDreamSafetyPolicy {
    pub fn new(
        max_facts_per_run: usize,
        max_changes_per_run: usize,
    ) -> Result<Self, MemoryDatabaseError> {
        if max_facts_per_run == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "memory Dream max facts per run must be greater than 0".to_string(),
            });
        }
        if max_changes_per_run == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "memory Dream max changes per run must be greater than 0".to_string(),
            });
        }

        Ok(Self {
            max_facts_per_run,
            max_changes_per_run,
        })
    }

    pub fn validate_batch_size(
        &self,
        fact_count: usize,
        change_count: usize,
    ) -> Result<(), MemoryDatabaseError> {
        if fact_count > self.max_facts_per_run {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!(
                    "memory Dream considered {fact_count} facts, limit is {}",
                    self.max_facts_per_run
                ),
            });
        }
        if change_count > self.max_changes_per_run {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!(
                    "memory Dream proposed {change_count} changes, limit is {}",
                    self.max_changes_per_run
                ),
            });
        }

        Ok(())
    }

    pub fn validate_updated_at(
        &self,
        expected_updated_at: &str,
        actual_updated_at: &str,
    ) -> Result<(), MemoryDatabaseError> {
        if expected_updated_at != actual_updated_at {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "memory Dream target changed before apply".to_string(),
            });
        }

        Ok(())
    }

    pub fn allows_automatic_global_promotion(
        &self,
        has_explicit_cross_project_user_preference: bool,
    ) -> bool {
        has_explicit_cross_project_user_preference
    }

    pub fn allows_direct_expiration(
        &self,
        kind: MemoryKind,
        pinned: bool,
        operation_is_deterministic: bool,
        has_explicit_evidence: bool,
    ) -> bool {
        if !pinned && kind != MemoryKind::UserNote {
            return true;
        }

        operation_is_deterministic && has_explicit_evidence
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryStatus {
    Pending,
    Active,
    Superseded,
    Expired,
    Rejected,
}

impl MemoryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Superseded => "superseded",
            Self::Expired => "expired",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        memory_status_from_str(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryKind {
    Preference,
    ProjectFact,
    ProjectDecision,
    Procedure,
    Constraint,
    Episode,
    UserNote,
}

impl MemoryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preference => "preference",
            Self::ProjectFact => "project_fact",
            Self::ProjectDecision => "project_decision",
            Self::Procedure => "procedure",
            Self::Constraint => "constraint",
            Self::Episode => "episode",
            Self::UserNote => "user_note",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        memory_kind_from_str(value)
    }
}

/// Unified kind/scope compatibility policy.
///
/// `project_fact` and `project_decision` describe a specific workspace and must
/// never be newly created, extracted, or promoted into `MemoryScope::Global`.
/// Historical global project-class rows stay readable (and editable in place)
/// so users can migrate them; this policy only guards write paths that would
/// create or change a fact into an illegal combination.
pub fn memory_scope_allows_kind(scope: MemoryScope, kind: MemoryKind) -> bool {
    !matches!(
        (scope, kind),
        (
            MemoryScope::Global,
            MemoryKind::ProjectFact | MemoryKind::ProjectDecision
        )
    )
}

pub fn ensure_memory_kind_scope_allowed(
    scope: MemoryScope,
    kind: MemoryKind,
) -> Result<(), MemoryDatabaseError> {
    if memory_scope_allows_kind(scope, kind) {
        Ok(())
    } else {
        Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!(
                "{} memory kind is not allowed in global scope; use workspace scope",
                kind.as_str()
            ),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemorySourceType {
    ChatMessage,
    AssistantMessage,
    ToolCall,
    ToolResult,
    ContextSnapshot,
    ManualNote,
    ImportedDocument,
}

impl MemorySourceType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChatMessage => "chat_message",
            Self::AssistantMessage => "assistant_message",
            Self::ToolCall => "tool_call",
            Self::ToolResult => "tool_result",
            Self::ContextSnapshot => "context_snapshot",
            Self::ManualNote => "manual_note",
            Self::ImportedDocument => "imported_document",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        memory_source_type_from_str(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryRelationKind {
    Updates,
    Extends,
    Derives,
}

impl MemoryRelationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Updates => "updates",
            Self::Extends => "extends",
            Self::Derives => "derives",
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum MemoryReferenceType {
    FilePath,
    Symbol,
    Command,
    Url,
    WorkspaceId,
}

impl MemoryReferenceType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FilePath => "file_path",
            Self::Symbol => "symbol",
            Self::Command => "command",
            Self::Url => "url",
            Self::WorkspaceId => "workspace_id",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        memory_reference_type_from_str(value)
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum MemoryReferenceStatus {
    Valid,
    Invalid,
    Ambiguous,
    Skipped,
}

impl MemoryReferenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Invalid => "invalid",
            Self::Ambiguous => "ambiguous",
            Self::Skipped => "skipped",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        memory_reference_status_from_str(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryExtractionJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl MemoryExtractionJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "skipped" => Ok(Self::Skipped),
            _ => Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!("unknown memory extraction job status: {value}"),
            }),
        }
    }
}

pub struct MemoryDatabase {
    database_path: PathBuf,
    connection: Connection,
    kind: MemoryDatabaseKind,
}

impl MemoryDatabase {
    pub fn open_or_create_global_at(
        database_path: impl AsRef<Path>,
    ) -> Result<Self, MemoryDatabaseError> {
        let database_path = database_path.as_ref().to_path_buf();
        let parent =
            database_path
                .parent()
                .ok_or_else(|| MemoryDatabaseError::MissingDatabaseParent {
                    path: database_path.clone(),
                })?;
        create_directory(parent)?;

        let mut connection = open_connection(&database_path)?;
        run_global_migrations(&mut connection, &database_path)?;
        enable_write_ahead_logging(&connection, &database_path)?;
        restrict_sqlite_files(&database_path).map_err(|source| MemoryDatabaseError::Io {
            path: database_path.clone(),
            source,
        })?;

        Ok(Self {
            database_path,
            connection,
            kind: MemoryDatabaseKind::Global,
        })
    }

    pub fn open_or_create_global(
        foco_root_dir: impl AsRef<Path>,
    ) -> Result<Self, MemoryDatabaseError> {
        Self::open_or_create_global_at(global_memory_database_path(foco_root_dir))
    }

    /// Open workspace Memory under the process-local ordinary gate.
    ///
    /// Production code must use this (or [`Self::open_or_create_workspace_critical`]).
    /// The returned handle shares the same ordinary/critical ledger as
    /// [`crate::workspace::WorkspaceDatabase::open_or_create`].
    #[track_caller]
    pub fn open_or_create_workspace(
        workspace_path: impl AsRef<Path>,
    ) -> Result<crate::workspace_gate::WorkspaceMemoryDatabaseHandle, MemoryDatabaseError> {
        crate::workspace_gate::open_workspace_memory_database(workspace_path)
    }

    /// Critical open for workspace Memory (total capacity only).
    #[track_caller]
    pub fn open_or_create_workspace_critical(
        workspace_path: impl AsRef<Path>,
    ) -> Result<crate::workspace_gate::WorkspaceMemoryDatabaseHandle, MemoryDatabaseError> {
        crate::workspace_gate::open_workspace_memory_database_critical(workspace_path)
    }

    /// Ungated workspace Memory open for the gate implementation and controlled tests only.
    ///
    /// Production code must use [`Self::open_or_create_workspace`] or
    /// [`Self::open_or_create_workspace_critical`]. Prefer the workspace root path
    /// APIs so the shared gate key matches [`crate::workspace::WorkspaceDatabase`].
    pub fn open_workspace_at_ungated(
        database_path: impl AsRef<Path>,
    ) -> Result<Self, MemoryDatabaseError> {
        let database_path = database_path.as_ref().to_path_buf();
        let connection = open_connection(&database_path)?;
        ensure_memory_schema_exists(&connection, &database_path)?;
        restrict_sqlite_files(&database_path).map_err(|source| MemoryDatabaseError::Io {
            path: database_path.clone(),
            source,
        })?;

        Ok(Self {
            database_path,
            connection,
            kind: MemoryDatabaseKind::Workspace,
        })
    }

    /// Deprecated alias: use [`Self::open_workspace_at_ungated`] in tests/migrations only.
    ///
    /// Production callers must switch to [`Self::open_or_create_workspace`].
    #[deprecated(
        note = "use MemoryDatabase::open_or_create_workspace(workspace_path) so the shared gate is applied"
    )]
    pub fn open_workspace_at(database_path: impl AsRef<Path>) -> Result<Self, MemoryDatabaseError> {
        Self::open_workspace_at_ungated(database_path)
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn schema_version(&self) -> Result<u32, MemoryDatabaseError> {
        schema_version(&self.connection, &self.database_path)
    }

    /// Low-frequency `PRAGMA optimize` for query-planner statistics.
    ///
    /// Global Memory uses process-local throttling only (no durable metadata table).
    /// Workspace Memory reuses the shared workspace gate path; prefer calling
    /// [`crate::workspace::WorkspaceDatabase::maybe_run_pragma_optimize`] on the
    /// workspace DB for durable throttle. Failures must not abort Dream/terminal work.
    pub fn maybe_run_pragma_optimize(&mut self, force: bool) -> Result<bool, MemoryDatabaseError> {
        let throttle = match self.kind {
            MemoryDatabaseKind::Global => {
                crate::workspace::SqlitePragmaOptimizeThrottle::ProcessLocalOnly
            }
            MemoryDatabaseKind::Workspace => {
                // Workspace Memory shares the file with WorkspaceDatabase but has no
                // independent metadata table; process-local throttle still avoids hot-path spam.
                crate::workspace::SqlitePragmaOptimizeThrottle::ProcessLocalOnly
            }
        };
        crate::workspace::maybe_run_sqlite_pragma_optimize(
            &mut self.connection,
            &self.database_path,
            throttle,
            force,
        )
        .map_err(|source| sqlite_error(&self.database_path, source))
    }

    pub fn insert_source(
        &mut self,
        source: NewMemorySource<'_>,
    ) -> Result<(), MemoryDatabaseError> {
        self.validate_scope(source.scope)?;
        validate_source(&source)?;
        let now = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO memory_sources
                    (id, scope, chat_id, source_type, source_id, title, content, metadata_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    source.id,
                    source.scope.as_str(),
                    source.chat_id,
                    source.source_type.as_str(),
                    source.source_id,
                    source.title,
                    source.content,
                    source.metadata_json,
                    now,
                    now,
                ],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(())
    }

    pub fn update_source(
        &mut self,
        source: UpdateMemorySource<'_>,
    ) -> Result<bool, MemoryDatabaseError> {
        validate_source_update(&source)?;
        let now = now_timestamp();
        let updated = self
            .connection
            .execute(
                "UPDATE memory_sources
                 SET title = COALESCE(?2, title),
                     content = COALESCE(?3, content),
                     metadata_json = COALESCE(?4, metadata_json),
                     updated_at = ?5
                 WHERE id = ?1",
                params![
                    source.id,
                    source.title,
                    source.content,
                    source.metadata_json,
                    now,
                ],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(updated > 0)
    }

    pub fn delete_source(&mut self, id: &str) -> Result<bool, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let linked_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM memory_fact_sources WHERE source_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        if linked_count > 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!("memory source '{id}' is still linked to {linked_count} fact(s)"),
            });
        }

        let deleted = transaction
            .execute("DELETE FROM memory_sources WHERE id = ?1", params![id])
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(deleted > 0)
    }

    pub fn insert_fact(&mut self, fact: NewMemoryFact<'_>) -> Result<(), MemoryDatabaseError> {
        self.validate_scope(fact.scope)?;
        validate_fact(&fact)?;
        ensure_memory_kind_scope_allowed(fact.scope, fact.kind)?;

        let database_path = self.database_path.clone();
        let now = now_timestamp();
        let transaction = self
            .connection
            .transaction()
            .map_err(|source| sqlite_error(&database_path, source))?;

        transaction
            .execute(
                "INSERT INTO memory_facts
                    (id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest, expires_at, metadata_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, NULL, ?9, ?10, ?11)",
                params![
                    fact.id,
                    fact.scope.as_str(),
                    fact.chat_id,
                    fact.status.as_str(),
                    fact.kind.as_str(),
                    fact.fact,
                    fact.confidence,
                    bool_to_i64(fact.pinned),
                    fact.metadata_json,
                    now,
                    now,
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        for source_id in fact.source_ids {
            require_non_empty("source_id", source_id)?;
            transaction
                .execute(
                    "INSERT INTO memory_fact_sources (fact_id, source_id)
                     VALUES (?1, ?2)",
                    params![fact.id, source_id],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }

        upsert_fact_fts_data(&transaction, &database_path, &fact, &now)?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(())
    }

    pub fn update_fact(&mut self, fact: UpdateMemoryFact<'_>) -> Result<bool, MemoryDatabaseError> {
        validate_fact_update(&fact)?;
        if let Some(scope) = fact.scope {
            self.validate_scope(scope)?;
        }
        // Enforce the unified kind/scope policy whenever either scope or kind is
        // changing, using the resulting combination. Historical global
        // project-class rows remain readable and can still be updated without
        // changing kind/scope (e.g. fact text or status).
        if fact.scope.is_some() || fact.kind.is_some() {
            if let Some(current) = self.fact(fact.id)? {
                let current_scope = MemoryScope::parse(&current.scope)?;
                let current_kind = MemoryKind::parse(&current.kind)?;
                let result_scope = fact.scope.unwrap_or(current_scope);
                let result_kind = fact.kind.unwrap_or(current_kind);
                ensure_memory_kind_scope_allowed(result_scope, result_kind)?;
            }
        }

        let database_path = self.database_path.clone();
        let now = now_timestamp();
        let transaction = self
            .connection
            .transaction()
            .map_err(|source| sqlite_error(&database_path, source))?;

        let updated = transaction
            .execute(
                "UPDATE memory_facts
                 SET scope = COALESCE(?2, scope),
                     chat_id = COALESCE(?3, chat_id),
                     status = COALESCE(?4, status),
                     kind = COALESCE(?5, kind),
                     fact = COALESCE(?6, fact),
                     confidence = COALESCE(?7, confidence),
                     pinned = COALESCE(?8, pinned),
                     is_latest = COALESCE(?9, is_latest),
                     expires_at = COALESCE(?10, expires_at),
                     metadata_json = COALESCE(?11, metadata_json),
                     updated_at = ?12
                 WHERE id = ?1",
                params![
                    fact.id,
                    fact.scope.map(MemoryScope::as_str),
                    fact.chat_id,
                    fact.status.map(MemoryStatus::as_str),
                    fact.kind.map(MemoryKind::as_str),
                    fact.fact,
                    fact.confidence,
                    fact.pinned.map(bool_to_i64),
                    fact.is_latest.map(bool_to_i64),
                    fact.expires_at,
                    fact.metadata_json,
                    now,
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        if updated > 0 {
            let updated_fact = fact_by_id(&transaction, &database_path, fact.id)?;
            upsert_fact_record_fts_data(&transaction, &database_path, &updated_fact)?;
            if fact.status == Some(MemoryStatus::Active) {
                apply_update_relation_effects(&transaction, &database_path, fact.id, &now)?;
            }
        }

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(updated > 0)
    }

    pub fn set_fact_status(
        &mut self,
        id: &str,
        status: MemoryStatus,
    ) -> Result<bool, MemoryDatabaseError> {
        self.update_fact(UpdateMemoryFact {
            id,
            status: Some(status),
            ..UpdateMemoryFact::default()
        })
    }

    pub fn set_fact_enabled(
        &mut self,
        id: &str,
        enabled: bool,
    ) -> Result<MemoryFactRecord, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        let current = self
            .fact(id)?
            .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                message: format!("memory fact was not found: {id}"),
            })?;
        if current.enabled == enabled {
            return Ok(current);
        }

        let now = now_timestamp();
        self.connection
            .execute(
                "UPDATE memory_facts SET enabled = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, bool_to_i64(enabled), now],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        self.fact(id)?
            .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                message: format!("memory fact was not found after update: {id}"),
            })
    }

    pub fn delete_fact(&mut self, id: &str) -> Result<bool, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction()
            .map_err(|source| sqlite_error(&database_path, source))?;

        transaction
            .execute(
                "DELETE FROM memory_fts_data WHERE fact_id = ?1",
                params![id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let deleted = transaction
            .execute("DELETE FROM memory_facts WHERE id = ?1", params![id])
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(deleted > 0)
    }

    pub fn hard_delete_fact(&mut self, id: &str) -> Result<bool, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let source_ids = source_ids_for_fact(&transaction, &database_path, id)?;

        transaction
            .execute(
                "DELETE FROM memory_fts_data WHERE fact_id = ?1",
                params![id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        let deleted = transaction
            .execute("DELETE FROM memory_facts WHERE id = ?1", params![id])
            .map_err(|source| sqlite_error(&database_path, source))?;
        if deleted > 0 {
            delete_unlinked_sources(&transaction, &database_path, &source_ids)?;
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(deleted > 0)
    }

    pub fn link_fact_source(
        &mut self,
        fact_id: &str,
        source_id: &str,
    ) -> Result<(), MemoryDatabaseError> {
        require_non_empty("fact_id", fact_id)?;
        require_non_empty("source_id", source_id)?;
        self.connection
            .execute(
                "INSERT OR IGNORE INTO memory_fact_sources (fact_id, source_id)
                 VALUES (?1, ?2)",
                params![fact_id, source_id],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(())
    }

    pub fn unlink_fact_source(
        &mut self,
        fact_id: &str,
        source_id: &str,
    ) -> Result<bool, MemoryDatabaseError> {
        require_non_empty("fact_id", fact_id)?;
        require_non_empty("source_id", source_id)?;
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let fact = fact_by_id(&transaction, &database_path, fact_id).map_err(|error| {
            if matches!(
                &error,
                MemoryDatabaseError::Sqlite {
                    source: rusqlite::Error::QueryReturnedNoRows,
                    ..
                }
            ) {
                MemoryDatabaseError::InvalidMemoryInput {
                    message: format!("memory fact was not found: {fact_id}"),
                }
            } else {
                error
            }
        })?;
        let source_count = source_count_for_fact(&transaction, &database_path, fact_id)?;

        if fact.kind != MemoryKind::UserNote.as_str() && source_count <= 1 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "non-user_note facts must keep at least one source".to_string(),
            });
        }

        let deleted = transaction
            .execute(
                "DELETE FROM memory_fact_sources WHERE fact_id = ?1 AND source_id = ?2",
                params![fact_id, source_id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(deleted > 0)
    }

    pub fn insert_edge(&mut self, edge: NewMemoryEdge<'_>) -> Result<(), MemoryDatabaseError> {
        validate_edge(&edge)?;
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        if edge.relation == MemoryRelationKind::Updates
            && update_relation_would_cycle(
                &transaction,
                &database_path,
                edge.source_fact_id,
                edge.target_fact_id,
            )?
        {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "updates relation would create a cycle".to_string(),
            });
        }
        let metadata_json = if edge.relation == MemoryRelationKind::Derives {
            Some(derives_edge_metadata(
                &transaction,
                &database_path,
                edge.source_fact_id,
                edge.target_fact_id,
                edge.metadata_json,
            )?)
        } else {
            None
        };
        let edge_metadata_json = metadata_json.as_deref().unwrap_or(edge.metadata_json);

        transaction
            .execute(
                "INSERT INTO memory_edges
                    (id, source_fact_id, target_fact_id, relation, metadata_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    edge.id,
                    edge.source_fact_id,
                    edge.target_fact_id,
                    edge.relation.as_str(),
                    edge_metadata_json,
                    now,
                ],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        if edge.relation == MemoryRelationKind::Updates {
            inherit_update_relation_enabled_state(
                &transaction,
                &database_path,
                edge.source_fact_id,
                edge.target_fact_id,
                &now,
            )?;
            apply_update_relation_effects(&transaction, &database_path, edge.source_fact_id, &now)?;
        }

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(())
    }

    pub fn refresh_profile_from_active_facts(
        &mut self,
        scope: MemoryScope,
        chat_id: Option<&str>,
        limit: u32,
    ) -> Result<Option<MemoryProfileRecord>, MemoryDatabaseError> {
        self.validate_scope(scope)?;
        validate_scope_chat_id(scope, chat_id)?;
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let facts = self.latest_active_facts_for_exact_scope(scope, chat_id, limit)?;
        let profile_id = profile_id_for_scope(scope, chat_id);
        if facts.is_empty() {
            self.connection
                .execute(
                    "DELETE FROM memory_profiles WHERE id = ?1",
                    params![profile_id],
                )
                .map_err(|source| sqlite_error(&self.database_path, source))?;
            return Ok(None);
        }

        let source_fact_ids = facts
            .iter()
            .map(|fact| fact.id.as_str())
            .collect::<Vec<_>>();
        let sources_by_fact = self.sources_for_facts(&source_fact_ids)?;
        let mut source_links = Vec::with_capacity(facts.len());
        for fact in &facts {
            let mut source_ids = sources_by_fact
                .get(&fact.id)
                .into_iter()
                .flatten()
                .map(|source| source.id.clone())
                .collect::<Vec<_>>();
            source_ids.sort();
            source_links.push(json!({
                "factId": &fact.id,
                "sourceIds": source_ids,
            }));
        }
        let profile_text = facts
            .iter()
            .map(memory_profile_fact_line)
            .collect::<Vec<_>>()
            .join("\n");
        let metadata_json = serde_json::to_string(&json!({
            "sourceFactIds": source_fact_ids,
            "sourceLinks": source_links,
            "sourceFactCount": facts.len(),
            "algorithm": "active-latest-facts-v1",
        }))
        .map_err(|source| MemoryDatabaseError::InvalidMemoryJson {
            field: "metadata_json",
            source,
        })?;

        self.upsert_profile(NewMemoryProfile {
            id: &profile_id,
            scope,
            chat_id,
            profile_text: &profile_text,
            metadata_json: &metadata_json,
        })?;
        self.profile(&profile_id)
    }

    pub fn expire_due_facts(&mut self, now: &str) -> Result<u64, MemoryDatabaseError> {
        require_non_empty("now", now)?;
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;
        let fact_ids = due_unexpired_fact_ids(&transaction, &database_path, now)?;

        for fact_id in &fact_ids {
            transaction
                .execute(
                    "UPDATE memory_facts
                     SET status = 'expired',
                         updated_at = ?2
                     WHERE id = ?1",
                    params![fact_id, now],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            let updated_fact = fact_by_id(&transaction, &database_path, fact_id)?;
            upsert_fact_record_fts_data(&transaction, &database_path, &updated_fact)?;
        }
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(fact_ids.len() as u64)
    }

    pub fn upsert_profile(
        &mut self,
        profile: NewMemoryProfile<'_>,
    ) -> Result<(), MemoryDatabaseError> {
        self.validate_scope(profile.scope)?;
        validate_profile(&profile)?;
        let now = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO memory_profiles
                    (id, scope, chat_id, profile_text, metadata_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                    profile_text = excluded.profile_text,
                    metadata_json = excluded.metadata_json,
                    updated_at = excluded.updated_at",
                params![
                    profile.id,
                    profile.scope.as_str(),
                    profile.chat_id,
                    profile.profile_text,
                    profile.metadata_json,
                    now,
                    now,
                ],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(())
    }

    pub fn profile(&self, id: &str) -> Result<Option<MemoryProfileRecord>, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        self.connection
            .query_row(
                "SELECT id, scope, chat_id, profile_text, metadata_json, created_at, updated_at
                 FROM memory_profiles
                 WHERE id = ?1",
                params![id],
                memory_profile_from_row,
            )
            .optional()
            .map_err(|source| sqlite_error(&self.database_path, source))
    }

    pub fn profiles_for_scope(
        &self,
        chat_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<MemoryProfileRecord>, MemoryDatabaseError> {
        if let Some(chat_id) = chat_id {
            require_non_empty("chat_id", chat_id)?;
        }
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let (filter_sql, chat_param) = match self.kind {
            MemoryDatabaseKind::Global => ("scope = 'global'", None),
            MemoryDatabaseKind::Workspace if chat_id.is_some() => (
                "(scope = 'chat' AND chat_id = ?1) OR scope = 'workspace'",
                chat_id,
            ),
            MemoryDatabaseKind::Workspace => ("scope = 'workspace'", None),
        };
        let sql = format!(
            "SELECT id, scope, chat_id, profile_text, metadata_json, created_at, updated_at
             FROM memory_profiles
             WHERE ({filter_sql})
             ORDER BY
               CASE WHEN scope = 'chat' THEN 0 WHEN scope = 'workspace' THEN 1 ELSE 2 END,
               updated_at DESC
             LIMIT ?2"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(params![chat_param, limit], memory_profile_from_row)
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn insert_extraction_job(
        &mut self,
        job: NewMemoryExtractionJob<'_>,
    ) -> Result<(), MemoryDatabaseError> {
        self.insert_extraction_job_inner(job, false)
    }

    pub fn insert_extraction_job_if_absent(
        &mut self,
        job: NewMemoryExtractionJob<'_>,
    ) -> Result<(), MemoryDatabaseError> {
        self.insert_extraction_job_inner(job, true)
    }

    fn insert_extraction_job_inner(
        &mut self,
        job: NewMemoryExtractionJob<'_>,
        ignore_existing: bool,
    ) -> Result<(), MemoryDatabaseError> {
        self.validate_scope(job.scope)?;
        validate_extraction_job(&job)?;
        let now = now_timestamp();
        let input_json = redact_memory_json(job.input_json, "memory_extraction_jobs.input_json")?;
        let output_json =
            redact_optional_memory_json(job.output_json, "memory_extraction_jobs.output_json")?;
        let insert = if ignore_existing {
            "INSERT OR IGNORE INTO memory_extraction_jobs
                (id, scope, chat_id, status, model_id, input_json, output_json, error_message, created_at, started_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL)"
        } else {
            "INSERT INTO memory_extraction_jobs
                (id, scope, chat_id, status, model_id, input_json, output_json, error_message, created_at, started_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL)"
        };

        self.connection
            .execute(
                insert,
                params![
                    job.id,
                    job.scope.as_str(),
                    job.chat_id,
                    job.status.as_str(),
                    job.model_id,
                    input_json,
                    output_json,
                    job.error_message,
                    now,
                ],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(())
    }

    pub fn mark_extraction_job_running(&mut self, id: &str) -> Result<bool, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        let now = now_timestamp();
        let changed = self
            .connection
            .execute(
                "UPDATE memory_extraction_jobs
                 SET status = 'running',
                     output_json = NULL,
                     started_at = COALESCE(started_at, ?2),
                     completed_at = NULL,
                     error_message = NULL
                 WHERE id = ?1 AND status = 'queued'",
                params![id, now],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(changed > 0)
    }

    pub fn complete_extraction_job(
        &mut self,
        id: &str,
        output_json: &str,
    ) -> Result<bool, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        let output_json = redact_memory_json(output_json, "memory_extraction_jobs.output_json")?;
        let now = now_timestamp();
        let changed = self
            .connection
            .execute(
                "UPDATE memory_extraction_jobs
                 SET status = 'completed',
                     output_json = ?2,
                     error_message = NULL,
                     started_at = COALESCE(started_at, ?3),
                     completed_at = ?3
                 WHERE id = ?1 AND status = 'running'",
                params![id, output_json, now],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(changed > 0)
    }

    pub fn fail_extraction_job(
        &mut self,
        id: &str,
        error_message: &str,
        output_json: Option<&str>,
    ) -> Result<bool, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        require_non_empty("error_message", error_message)?;
        let output_json =
            redact_optional_memory_json(output_json, "memory_extraction_jobs.output_json")?;
        let now = now_timestamp();
        let changed = self
            .connection
            .execute(
                "UPDATE memory_extraction_jobs
                 SET status = 'failed',
                     output_json = ?2,
                     error_message = ?3,
                     started_at = COALESCE(started_at, ?4),
                     completed_at = ?4
                 WHERE id = ?1 AND status = 'running'",
                params![id, output_json, error_message, now],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(changed > 0)
    }

    pub fn retry_failed_extraction_job(
        &mut self,
        id: &str,
        model_id: &str,
    ) -> Result<bool, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        require_non_empty("model_id", model_id)?;
        let now = now_timestamp();
        let changed = self
            .connection
            .execute(
                "UPDATE memory_extraction_jobs
                 SET status = 'running',
                     model_id = ?3,
                     output_json = NULL,
                     error_message = NULL,
                     started_at = ?2,
                     completed_at = NULL
                 WHERE id = ?1 AND status = 'failed'",
                params![id, now, model_id],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(changed > 0)
    }

    pub fn skip_failed_extraction_job(&mut self, id: &str) -> Result<bool, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        let now = now_timestamp();
        let changed = self
            .connection
            .execute(
                "UPDATE memory_extraction_jobs
                 SET status = 'skipped',
                     completed_at = ?2
                 WHERE id = ?1 AND status = 'failed'",
                params![id, now],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(changed > 0)
    }

    pub fn extraction_job(
        &self,
        id: &str,
    ) -> Result<Option<MemoryExtractionJobRecord>, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        self.connection
            .query_row(
                "SELECT id, scope, chat_id, status, model_id, input_json, output_json,
                        error_message, created_at, started_at, completed_at
                 FROM memory_extraction_jobs
                 WHERE id = ?1",
                params![id],
                memory_extraction_job_from_row,
            )
            .optional()
            .map_err(|source| sqlite_error(&self.database_path, source))
    }

    pub fn extraction_jobs_for_scope(
        &self,
        chat_id: Option<&str>,
        status: Option<MemoryExtractionJobStatus>,
        limit: u32,
    ) -> Result<Vec<MemoryExtractionJobRecord>, MemoryDatabaseError> {
        if let Some(chat_id) = chat_id {
            require_non_empty("chat_id", chat_id)?;
        }
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let (filter_sql, chat_param) = match self.kind {
            MemoryDatabaseKind::Global => ("scope = 'global'", None),
            MemoryDatabaseKind::Workspace if chat_id.is_some() => (
                "(scope = 'chat' AND chat_id = ?1) OR scope = 'workspace'",
                chat_id,
            ),
            MemoryDatabaseKind::Workspace => ("scope = 'workspace'", None),
        };
        let sql = format!(
            "SELECT id, scope, chat_id, status, model_id, input_json, output_json,
                    error_message, created_at, started_at, completed_at
             FROM memory_extraction_jobs
             WHERE ({filter_sql})
               AND (?2 IS NULL OR status = ?2)
             ORDER BY created_at DESC, id ASC
             LIMIT ?3"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(
                params![
                    chat_param,
                    status.map(MemoryExtractionJobStatus::as_str),
                    limit
                ],
                memory_extraction_job_from_row,
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn extraction_jobs(
        &self,
        status: Option<MemoryExtractionJobStatus>,
        limit: u32,
    ) -> Result<Vec<MemoryExtractionJobRecord>, MemoryDatabaseError> {
        if self.kind == MemoryDatabaseKind::Global {
            return Ok(Vec::new());
        }
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let mut statement = self
            .connection
            .prepare(
                "SELECT id, scope, chat_id, status, model_id, input_json, output_json,
                        error_message, created_at, started_at, completed_at
                 FROM memory_extraction_jobs
                 WHERE (?1 IS NULL OR status = ?1)
                 ORDER BY created_at DESC, id ASC
                 LIMIT ?2",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(
                params![status.map(MemoryExtractionJobStatus::as_str), limit],
                memory_extraction_job_from_row,
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn insert_dream_job(
        &mut self,
        job: NewMemoryDreamJob<'_>,
    ) -> Result<(), MemoryDatabaseError> {
        match self.start_dream_job(job)? {
            StartMemoryDreamJobOutcome::Started => Ok(()),
            StartMemoryDreamJobOutcome::AlreadyActive => Err(MemoryDatabaseError::AlreadyActive {
                message: "memory Dream is already active".to_string(),
            }),
        }
    }

    /// Insert a Dream job under the partial UNIQUE active-job constraint.
    ///
    /// Always inserts as `queued` first (or as terminal for non-live starts), then claims
    /// to `running` in the same Immediate transaction when `job.status == Running`.
    /// Concurrent starters lose the UNIQUE race and receive `AlreadyActive` without a
    /// raw SQLite constraint error.
    pub fn start_dream_job(
        &mut self,
        job: NewMemoryDreamJob<'_>,
    ) -> Result<StartMemoryDreamJobOutcome, MemoryDatabaseError> {
        self.validate_dream_scope(job.scope, job.workspace_id)?;
        validate_dream_job(&job)?;
        let insert_status = match job.status {
            MemoryDreamJobStatus::Running => MemoryDreamJobStatus::Queued,
            other => other,
        };
        let now = now_timestamp();
        let input_summary_json = redact_memory_json(
            job.input_summary_json,
            "memory_dream_jobs.input_summary_json",
        )?;
        let output_summary_json = redact_optional_memory_json(
            job.output_summary_json,
            "memory_dream_jobs.output_summary_json",
        )?;
        let started_at = insert_status.starts_run().then_some(now.as_str());
        let completed_at = insert_status.is_terminal().then_some(now.as_str());
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| sqlite_error(&database_path, source))?;

        let insert_result = transaction.execute(
            "INSERT INTO memory_dream_jobs
                (id, scope, workspace_id, trigger_type, mode, status, model_id,
                 input_summary_json, output_summary_json, transcript_chat_id, error_message,
                 created_at, started_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                job.id,
                job.scope.as_str(),
                job.workspace_id,
                job.trigger_type.as_str(),
                job.mode.as_str(),
                insert_status.as_str(),
                job.model_id,
                input_summary_json,
                output_summary_json,
                job.transcript_chat_id,
                job.error_message,
                now,
                started_at,
                completed_at,
            ],
        );
        match insert_result {
            Ok(_) => {}
            Err(source) if is_active_dream_singleflight_conflict(&source) => {
                return Ok(StartMemoryDreamJobOutcome::AlreadyActive);
            }
            Err(source) => return Err(sqlite_error(&database_path, source)),
        }

        if job.status == MemoryDreamJobStatus::Running {
            let claimed = transaction
                .execute(
                    "UPDATE memory_dream_jobs
                     SET status = 'running',
                         started_at = COALESCE(started_at, ?2),
                         completed_at = NULL,
                         error_message = NULL
                     WHERE id = ?1 AND status = 'queued'",
                    params![job.id, now],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            if claimed == 0 {
                return Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: format!("memory Dream job was not claimable: {}", job.id),
                });
            }
        }

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;
        Ok(StartMemoryDreamJobOutcome::Started)
    }

    /// Claim a queued Dream job to running exactly once.
    pub fn claim_dream_job_running(
        &mut self,
        id: &str,
    ) -> Result<MemoryDreamJobTransitionOutcome, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        let now = now_timestamp();
        let changed = self
            .connection
            .execute(
                "UPDATE memory_dream_jobs
                 SET status = 'running',
                     started_at = COALESCE(started_at, ?2),
                     completed_at = NULL,
                     error_message = NULL
                 WHERE id = ?1 AND status = 'queued'",
                params![id, now],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        Ok(if changed > 0 {
            MemoryDreamJobTransitionOutcome::Applied
        } else {
            MemoryDreamJobTransitionOutcome::NotApplied
        })
    }

    pub fn update_dream_job_status(
        &mut self,
        update: UpdateMemoryDreamJob<'_>,
    ) -> Result<bool, MemoryDatabaseError> {
        Ok(self.finish_dream_job(update)? == MemoryDreamJobTransitionOutcome::Applied)
    }

    /// Apply a Dream job status transition with strict pre-state rules.
    ///
    /// - claim (`queued` → `running`) only via [`Self::claim_dream_job_running`] / [`Self::start_dream_job`]
    /// - `Running` updates attach fields while already `running` (never claim from `queued`)
    /// - terminal statuses only from `running`
    /// - terminal rows are never overwritten
    pub fn finish_dream_job(
        &mut self,
        update: UpdateMemoryDreamJob<'_>,
    ) -> Result<MemoryDreamJobTransitionOutcome, MemoryDatabaseError> {
        validate_dream_job_update(&update)?;
        let now = now_timestamp();
        let output_summary_json = redact_optional_memory_json(
            update.output_summary_json,
            "memory_dream_jobs.output_summary_json",
        )?;
        let error_message = match update.status {
            MemoryDreamJobStatus::Failed
            | MemoryDreamJobStatus::Cancelled
            | MemoryDreamJobStatus::Skipped => update.error_message,
            MemoryDreamJobStatus::Queued
            | MemoryDreamJobStatus::Running
            | MemoryDreamJobStatus::Completed => None,
        };

        let changed = match update.status {
            MemoryDreamJobStatus::Queued => {
                return Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: "memory Dream job cannot transition back to queued".to_string(),
                });
            }
            MemoryDreamJobStatus::Running => {
                // Attach transcript / refresh running fields only; never claim from queued.
                self.connection
                    .execute(
                        "UPDATE memory_dream_jobs
                         SET status = 'running',
                             output_summary_json = COALESCE(?2, output_summary_json),
                             transcript_chat_id = COALESCE(?3, transcript_chat_id),
                             error_message = NULL,
                             started_at = COALESCE(started_at, ?4),
                             completed_at = NULL
                         WHERE id = ?1
                           AND status = 'running'",
                        params![
                            update.id,
                            output_summary_json,
                            update.transcript_chat_id,
                            now,
                        ],
                    )
                    .map_err(|source| sqlite_error(&self.database_path, source))?
            }
            MemoryDreamJobStatus::Completed
            | MemoryDreamJobStatus::Failed
            | MemoryDreamJobStatus::Cancelled
            | MemoryDreamJobStatus::Skipped => self
                .connection
                .execute(
                    "UPDATE memory_dream_jobs
                     SET status = ?2,
                         output_summary_json = COALESCE(?3, output_summary_json),
                         transcript_chat_id = COALESCE(?4, transcript_chat_id),
                         error_message = ?5,
                         started_at = COALESCE(started_at, ?6),
                         completed_at = ?6
                     WHERE id = ?1
                       AND status = 'running'",
                    params![
                        update.id,
                        update.status.as_str(),
                        output_summary_json,
                        update.transcript_chat_id,
                        error_message,
                        now,
                    ],
                )
                .map_err(|source| sqlite_error(&self.database_path, source))?,
        };

        Ok(if changed > 0 {
            MemoryDreamJobTransitionOutcome::Applied
        } else {
            MemoryDreamJobTransitionOutcome::NotApplied
        })
    }

    /// Startup reconcile: mark a leftover active job failed if still `queued` or `running`.
    ///
    /// This is intentionally broader than terminal finish (running-only) so interrupted
    /// queued rows can be closed. Callers must only use it for stale/interrupted recovery
    /// after excluding live in-process Dream runs.
    pub fn fail_interrupted_dream_job(
        &mut self,
        id: &str,
        error_message: &str,
    ) -> Result<MemoryDreamJobTransitionOutcome, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        require_non_empty("error_message", error_message)?;
        let now = now_timestamp();
        let changed = self
            .connection
            .execute(
                "UPDATE memory_dream_jobs
                 SET status = 'failed',
                     error_message = ?2,
                     started_at = COALESCE(started_at, ?3),
                     completed_at = ?3
                 WHERE id = ?1
                   AND status IN ('queued', 'running')",
                params![id, error_message, now],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        Ok(if changed > 0 {
            MemoryDreamJobTransitionOutcome::Applied
        } else {
            MemoryDreamJobTransitionOutcome::NotApplied
        })
    }

    pub fn insert_dream_change(
        &mut self,
        change: NewMemoryDreamChange<'_>,
    ) -> Result<(), MemoryDatabaseError> {
        validate_dream_change(&change)?;
        let now = now_timestamp();
        let target_fact_ids_json = redact_memory_json(
            change.target_fact_ids_json,
            "memory_dream_changes.target_fact_ids_json",
        )?;
        let before_json =
            redact_optional_memory_json(change.before_json, "memory_dream_changes.before_json")?;
        let after_json =
            redact_optional_memory_json(change.after_json, "memory_dream_changes.after_json")?;
        let evidence_json =
            redact_memory_json(change.evidence_json, "memory_dream_changes.evidence_json")?;
        let applied_at =
            (change.status == MemoryDreamChangeStatus::Applied).then_some(now.as_str());

        self.connection
            .execute(
                "INSERT INTO memory_dream_changes
                    (id, job_id, operation, target_fact_ids_json, new_fact_id, before_json,
                     after_json, reason, confidence, risk_level, status, evidence_json,
                     error_message, created_at, applied_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    change.id,
                    change.job_id,
                    change.operation,
                    target_fact_ids_json,
                    change.new_fact_id,
                    before_json,
                    after_json,
                    change.reason,
                    change.confidence,
                    change.risk_level,
                    change.status.as_str(),
                    evidence_json,
                    change.error_message,
                    now,
                    applied_at,
                ],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(())
    }

    pub fn update_dream_change_status(
        &mut self,
        update: UpdateMemoryDreamChange<'_>,
    ) -> Result<bool, MemoryDatabaseError> {
        validate_dream_change_update(&update)?;
        let now = now_timestamp();
        let after_json =
            redact_optional_memory_json(update.after_json, "memory_dream_changes.after_json")?;
        let applied_at =
            (update.status == MemoryDreamChangeStatus::Applied).then_some(now.as_str());
        let error_message = match update.status {
            MemoryDreamChangeStatus::Failed | MemoryDreamChangeStatus::Skipped => {
                update.error_message
            }
            MemoryDreamChangeStatus::Proposed | MemoryDreamChangeStatus::Applied => None,
        };

        let changed = self
            .connection
            .execute(
                "UPDATE memory_dream_changes
                 SET status = ?2,
                     after_json = COALESCE(?3, after_json),
                     error_message = ?4,
                     applied_at = CASE
                        WHEN ?5 IS NULL THEN applied_at
                        ELSE COALESCE(applied_at, ?5)
                     END
                 WHERE id = ?1
                   AND status = 'proposed'
                   AND ?2 IN ('applied', 'skipped', 'failed')",
                params![
                    update.id,
                    update.status.as_str(),
                    after_json,
                    error_message,
                    applied_at,
                ],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(changed > 0)
    }

    pub fn dream_jobs_for_scope(
        &self,
        scope: MemoryDreamScope,
        workspace_id: Option<&str>,
        status: Option<MemoryDreamJobStatus>,
        limit: u32,
    ) -> Result<Vec<MemoryDreamJobRecord>, MemoryDatabaseError> {
        self.dream_jobs_for_scope_page(scope, workspace_id, status, limit, 0)
    }

    pub fn count_dream_jobs_for_scope(
        &self,
        scope: MemoryDreamScope,
        workspace_id: Option<&str>,
        status: Option<MemoryDreamJobStatus>,
    ) -> Result<u32, MemoryDatabaseError> {
        self.validate_dream_scope(scope, workspace_id)?;

        let count: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*)
                 FROM memory_dream_jobs
                 WHERE scope = ?1
                   AND (?2 IS NULL OR workspace_id = ?2)
                   AND (?3 IS NULL OR status = ?3)",
                params![
                    scope.as_str(),
                    workspace_id,
                    status.map(MemoryDreamJobStatus::as_str),
                ],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        u32::try_from(count).map_err(|_| MemoryDatabaseError::InvalidMemoryInput {
            message: format!("memory Dream job count exceeds u32: {count}"),
        })
    }

    pub fn dream_jobs_for_scope_page(
        &self,
        scope: MemoryDreamScope,
        workspace_id: Option<&str>,
        status: Option<MemoryDreamJobStatus>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MemoryDreamJobRecord>, MemoryDatabaseError> {
        self.validate_dream_scope(scope, workspace_id)?;
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let mut statement = self
            .connection
            .prepare(
                "SELECT id, scope, workspace_id, trigger_type, mode, status, model_id,
                        input_summary_json, output_summary_json, transcript_chat_id,
                        error_message, created_at, started_at, completed_at
                 FROM memory_dream_jobs
                 WHERE scope = ?1
                   AND (?2 IS NULL OR workspace_id = ?2)
                   AND (?3 IS NULL OR status = ?3)
                 ORDER BY created_at DESC, id ASC
                 LIMIT ?4 OFFSET ?5",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(
                params![
                    scope.as_str(),
                    workspace_id,
                    status.map(MemoryDreamJobStatus::as_str),
                    limit,
                    offset,
                ],
                memory_dream_job_from_row,
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn dream_job(
        &self,
        job_id: &str,
    ) -> Result<Option<MemoryDreamJobRecord>, MemoryDatabaseError> {
        require_non_empty("job_id", job_id)?;

        self.connection
            .query_row(
                "SELECT id, scope, workspace_id, trigger_type, mode, status, model_id,
                        input_summary_json, output_summary_json, transcript_chat_id,
                        error_message, created_at, started_at, completed_at
                 FROM memory_dream_jobs
                 WHERE id = ?1",
                params![job_id],
                memory_dream_job_from_row,
            )
            .optional()
            .map_err(|source| sqlite_error(&self.database_path, source))
    }

    pub fn dream_changes_for_job(
        &self,
        job_id: &str,
        status: Option<MemoryDreamChangeStatus>,
        limit: u32,
    ) -> Result<Vec<MemoryDreamChangeRecord>, MemoryDatabaseError> {
        require_non_empty("job_id", job_id)?;
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let mut statement = self
            .connection
            .prepare(
                "SELECT id, job_id, operation, target_fact_ids_json, new_fact_id, before_json,
                        after_json, reason, confidence, risk_level, status, evidence_json,
                        error_message, created_at, applied_at
                 FROM memory_dream_changes
                 WHERE job_id = ?1
                   AND (?2 IS NULL OR status = ?2)
                 ORDER BY created_at ASC, id ASC
                 LIMIT ?3",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(
                params![job_id, status.map(MemoryDreamChangeStatus::as_str), limit],
                memory_dream_change_from_row,
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn latest_successful_dream_time(
        &self,
        scope: MemoryDreamScope,
        workspace_id: Option<&str>,
    ) -> Result<Option<String>, MemoryDatabaseError> {
        self.validate_dream_scope(scope, workspace_id)?;

        self.connection
            .query_row(
                "SELECT completed_at
                 FROM memory_dream_jobs
                 WHERE scope = ?1
                   AND (?2 IS NULL OR workspace_id = ?2)
                   AND status = 'completed'
                   AND completed_at IS NOT NULL
                 ORDER BY completed_at DESC, id ASC
                 LIMIT 1",
                params![scope.as_str(), workspace_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| sqlite_error(&self.database_path, source))
    }

    pub fn dream_candidate_facts(
        &self,
        scope: MemoryDreamScope,
        workspace_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.validate_dream_scope(scope, workspace_id)?;
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let scope_filter = match scope {
            MemoryDreamScope::Global => "f.scope = 'global'",
            MemoryDreamScope::Workspace => "f.scope IN ('workspace', 'chat')",
        };
        let now = now_timestamp();
        let sql = format!(
            "WITH source_counts AS (
                 SELECT fact_id, COUNT(*) AS source_count
                 FROM memory_fact_sources
                 GROUP BY fact_id
             ), reference_counts AS (
                 SELECT fact_id,
                        SUM(CASE WHEN status = 'invalid' THEN 1 ELSE 0 END) AS invalid_count,
                        SUM(CASE WHEN status = 'ambiguous' THEN 1 ELSE 0 END) AS ambiguous_count,
                        SUM(CASE WHEN status = 'skipped' THEN 1 ELSE 0 END) AS skipped_count
                 FROM memory_references
                 GROUP BY fact_id
             )
             SELECT f.id, f.scope, f.chat_id, f.status, f.kind, f.fact, f.confidence,
                    f.pinned, f.enabled, f.is_latest, f.expires_at, f.metadata_json, f.created_at,
                    f.updated_at
             FROM memory_facts f
             LEFT JOIN source_counts sc ON sc.fact_id = f.id
             LEFT JOIN reference_counts rc ON rc.fact_id = f.id
             WHERE ({scope_filter})
               AND f.status IN ('active', 'pending')
               AND f.enabled = 1
             ORDER BY
               CASE
                 WHEN f.expires_at IS NOT NULL AND f.expires_at <= ?1 THEN 0
                 WHEN f.status = 'pending'
                      AND COALESCE(f.confidence, 0) >= 0.85
                      AND COALESCE(sc.source_count, 0) > 0
                      AND COALESCE(rc.invalid_count, 0) = 0
                      AND COALESCE(rc.ambiguous_count, 0) = 0
                      AND COALESCE(rc.skipped_count, 0) = 0 THEN 1
                 WHEN COALESCE(rc.invalid_count, 0) > 0
                      OR COALESCE(rc.ambiguous_count, 0) > 0
                      OR COALESCE(rc.skipped_count, 0) > 0 THEN 2
                 WHEN f.expires_at IS NOT NULL THEN 3
                 WHEN f.status = 'pending' THEN 4
                 ELSE 5
               END,
               f.pinned DESC,
               COALESCE(sc.source_count, 0) DESC,
               COALESCE(f.confidence, -1) DESC,
               f.updated_at DESC,
               f.id ASC
             LIMIT ?2"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(params![now, limit], memory_fact_from_row)
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn dream_updated_fact_count_since(
        &self,
        scope: MemoryDreamScope,
        workspace_id: Option<&str>,
        since: Option<&str>,
    ) -> Result<u32, MemoryDatabaseError> {
        self.validate_dream_scope(scope, workspace_id)?;
        if let Some(since) = since {
            require_non_empty("since", since)?;
        }

        let scope_filter = match scope {
            MemoryDreamScope::Global => "scope = 'global'",
            MemoryDreamScope::Workspace => "scope IN ('workspace', 'chat')",
        };
        let sql = format!(
            "SELECT COUNT(*)
             FROM memory_facts
             WHERE ({scope_filter})
               AND status IN ('active', 'pending')
               AND enabled = 1
               AND (?1 IS NULL OR updated_at > ?1 OR created_at > ?1)"
        );
        let count: i64 = self
            .connection
            .query_row(&sql, params![since], |row| row.get(0))
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        u32::try_from(count).map_err(|_| MemoryDatabaseError::InvalidMemoryInput {
            message: format!("memory Dream updated fact count exceeds u32: {count}"),
        })
    }

    pub fn update_chain_target_facts(
        &self,
        source_fact_id: &str,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        require_non_empty("source_fact_id", source_fact_id)?;
        let mut statement = self
            .connection
            .prepare(
                "WITH RECURSIVE update_chain(fact_id) AS (
                    SELECT target_fact_id
                    FROM memory_edges
                    WHERE source_fact_id = ?1 AND relation = 'updates'
                    UNION
                    SELECT e.target_fact_id
                    FROM memory_edges e
                    JOIN update_chain c ON e.source_fact_id = c.fact_id
                    WHERE e.relation = 'updates'
                 )
                 SELECT f.id, f.scope, f.chat_id, f.status, f.kind, f.fact, f.confidence,
                        f.pinned, f.enabled, f.is_latest, f.expires_at, f.metadata_json, f.created_at,
                        f.updated_at
                 FROM memory_facts f
                 JOIN update_chain c ON c.fact_id = f.id
                 WHERE f.enabled = 1
                 ORDER BY f.created_at DESC, f.id ASC",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(params![source_fact_id], memory_fact_from_row)
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn edges_for_fact_ids(
        &self,
        fact_ids: &[String],
        limit: u32,
    ) -> Result<Vec<MemoryEdgeRecord>, MemoryDatabaseError> {
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let mut edges = Vec::new();
        let mut seen = HashSet::new();
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, source_fact_id, target_fact_id, relation, metadata_json, created_at
                 FROM memory_edges
                 WHERE source_fact_id = ?1 OR target_fact_id = ?2
                 ORDER BY created_at DESC, id ASC
                 LIMIT ?3",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        for fact_id in fact_ids {
            require_non_empty("fact_id", fact_id)?;
            let remaining = limit as usize - edges.len();
            if remaining == 0 {
                break;
            }
            let rows = statement
                .query_map(
                    params![fact_id, fact_id, remaining as u32],
                    memory_edge_from_row,
                )
                .map_err(|source| sqlite_error(&self.database_path, source))?;
            for edge in collect_rows(rows, &self.database_path)? {
                if seen.insert(edge.id.clone()) {
                    edges.push(edge);
                }
            }
        }

        Ok(edges)
    }

    pub fn replace_fact_references(
        &mut self,
        fact_id: &str,
        references: &[NewMemoryReference<'_>],
    ) -> Result<(), MemoryDatabaseError> {
        require_non_empty("fact_id", fact_id)?;
        for reference in references {
            validate_reference(reference)?;
            if reference.fact_id != fact_id {
                return Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: format!(
                        "memory reference '{}' belongs to fact '{}', expected '{}'",
                        reference.id, reference.fact_id, fact_id
                    ),
                });
            }
        }

        let database_path = self.database_path.clone();
        let now = now_timestamp();
        let transaction = self
            .connection
            .transaction()
            .map_err(|source| sqlite_error(&database_path, source))?;

        transaction
            .execute(
                "DELETE FROM memory_references WHERE fact_id = ?1",
                params![fact_id],
            )
            .map_err(|source| sqlite_error(&database_path, source))?;

        for reference in references {
            transaction
                .execute(
                    "INSERT INTO memory_references
                        (id, fact_id, reference_type, value, normalized_value, status,
                         metadata_json, checked_at, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        reference.id,
                        reference.fact_id,
                        reference.reference_type.as_str(),
                        reference.value,
                        reference.normalized_value,
                        reference.status.as_str(),
                        reference.metadata_json,
                        reference.checked_at,
                        now,
                        now,
                    ],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }

        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(())
    }

    pub fn references_for_fact_ids(
        &self,
        fact_ids: &[String],
        limit: u32,
    ) -> Result<Vec<MemoryReferenceRecord>, MemoryDatabaseError> {
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let mut references = Vec::new();
        let mut seen = HashSet::new();
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, fact_id, reference_type, value, normalized_value, status,
                        metadata_json, checked_at, created_at, updated_at
                 FROM memory_references
                 WHERE fact_id = ?1
                 ORDER BY reference_type ASC, normalized_value ASC, id ASC
                 LIMIT ?2",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        for fact_id in fact_ids {
            require_non_empty("fact_id", fact_id)?;
            let remaining = limit as usize - references.len();
            if remaining == 0 {
                break;
            }
            let rows = statement
                .query_map(
                    params![fact_id, remaining as u32],
                    memory_reference_from_row,
                )
                .map_err(|source| sqlite_error(&self.database_path, source))?;
            for reference in collect_rows(rows, &self.database_path)? {
                if seen.insert(reference.id.clone()) {
                    references.push(reference);
                }
            }
        }

        Ok(references)
    }

    pub fn fact(&self, id: &str) -> Result<Option<MemoryFactRecord>, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        self.connection
            .query_row(
                "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, enabled, is_latest,
                        expires_at, metadata_json, created_at, updated_at
                 FROM memory_facts
                 WHERE id = ?1",
                params![id],
                memory_fact_from_row,
            )
            .optional()
            .map_err(|source| sqlite_error(&self.database_path, source))
    }

    fn active_latest_fact(
        &self,
        id: &str,
    ) -> Result<Option<MemoryFactRecord>, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        self.connection
            .query_row(
                "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, enabled, is_latest,
                        expires_at, metadata_json, created_at, updated_at
                 FROM memory_facts
                 WHERE id = ?1
                   AND status = 'active'
                   AND is_latest = 1",
                params![id],
                memory_fact_from_row,
            )
            .optional()
            .map_err(|source| sqlite_error(&self.database_path, source))
    }

    fn enabled_active_latest_fact(
        &self,
        id: &str,
    ) -> Result<Option<MemoryFactRecord>, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        self.connection
            .query_row(
                "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, enabled, is_latest,
                        expires_at, metadata_json, created_at, updated_at
                 FROM memory_facts
                 WHERE id = ?1
                   AND status = 'active'
                   AND enabled = 1
                   AND is_latest = 1",
                params![id],
                memory_fact_from_row,
            )
            .optional()
            .map_err(|source| sqlite_error(&self.database_path, source))
    }

    pub fn source(&self, id: &str) -> Result<Option<MemorySourceRecord>, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        self.connection
            .query_row(
                "SELECT id, scope, chat_id, source_type, source_id, title, content,
                        metadata_json, created_at, updated_at
                 FROM memory_sources
                 WHERE id = ?1",
                params![id],
                memory_source_from_row,
            )
            .optional()
            .map_err(|source| sqlite_error(&self.database_path, source))
    }

    pub fn sources_for_fact(
        &self,
        fact_id: &str,
    ) -> Result<Vec<MemorySourceRecord>, MemoryDatabaseError> {
        require_non_empty("fact_id", fact_id)?;
        let mut by_fact = self.sources_for_facts(&[fact_id])?;
        Ok(by_fact.remove(fact_id).unwrap_or_default())
    }

    pub fn sources_for_facts(
        &self,
        fact_ids: &[&str],
    ) -> Result<HashMap<String, Vec<MemorySourceRecord>>, MemoryDatabaseError> {
        if fact_ids.is_empty() {
            return Ok(HashMap::new());
        }
        for fact_id in fact_ids {
            require_non_empty("fact_id", fact_id)?;
        }

        let placeholders = (1..=fact_ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT fs.fact_id, s.id, s.scope, s.chat_id, s.source_type, s.source_id, s.title, s.content,
                    s.metadata_json, s.created_at, s.updated_at
             FROM memory_sources s
             JOIN memory_fact_sources fs ON fs.source_id = s.id
             WHERE fs.fact_id IN ({placeholders})
             ORDER BY fs.fact_id ASC, s.created_at ASC, s.id ASC"
        );
        let query_params = fact_ids.iter().map(|fact_id| *fact_id).collect::<Vec<_>>();
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(params_from_iter(query_params), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    MemorySourceRecord {
                        id: row.get(1)?,
                        scope: row.get(2)?,
                        chat_id: row.get(3)?,
                        source_type: row.get(4)?,
                        source_id: row.get(5)?,
                        title: row.get(6)?,
                        content: row.get(7)?,
                        metadata_json: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    },
                ))
            })
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        let mut sources_by_fact = HashMap::new();
        for row in rows {
            let (fact_id, source) =
                row.map_err(|source| sqlite_error(&self.database_path, source))?;
            sources_by_fact
                .entry(fact_id)
                .or_insert_with(Vec::new)
                .push(source);
        }
        for fact_id in fact_ids {
            sources_by_fact.entry((*fact_id).to_string()).or_default();
        }
        Ok(sources_by_fact)
    }

    pub fn facts_created_from_chat_sources(
        &self,
        chat_id: &str,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        require_non_empty("chat_id", chat_id)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT DISTINCT f.id, f.scope, f.chat_id, f.status, f.kind, f.fact, f.confidence,
                        f.pinned, f.enabled, f.is_latest, f.expires_at, f.metadata_json, f.created_at,
                        f.updated_at
                 FROM memory_facts f
                 JOIN memory_fact_sources fs ON fs.fact_id = f.id
                 JOIN memory_sources s ON s.id = fs.source_id
                 WHERE s.chat_id = ?1
                    OR f.chat_id = ?1
                    OR (
                        s.source_type IN ('chat_message', 'assistant_message')
                        AND s.source_id IN (SELECT id FROM messages WHERE chat_id = ?1)
                    )
                    OR (
                        s.source_type = 'tool_call'
                        AND s.source_id IN (SELECT id FROM tool_calls WHERE chat_id = ?1)
                    )
                    OR (
                        s.source_type = 'tool_result'
                        AND s.source_id IN (
                            SELECT tool_results.id
                            FROM tool_results
                            JOIN tool_calls ON tool_calls.id = tool_results.tool_call_id
                            WHERE tool_calls.chat_id = ?1
                        )
                    )
                 ORDER BY f.created_at ASC, f.id ASC",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(params![chat_id], memory_fact_from_row)
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn facts_created_from_source_run_ids(
        &self,
        run_ids: &HashSet<String>,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        if run_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut statement = self
            .connection
            .prepare(
                "SELECT f.id, f.scope, f.chat_id, f.status, f.kind, f.fact, f.confidence,
                        f.pinned, f.enabled, f.is_latest, f.expires_at, f.metadata_json, f.created_at,
                        f.updated_at, s.metadata_json
                 FROM memory_facts f
                 JOIN memory_fact_sources fs ON fs.fact_id = f.id
                 JOIN memory_sources s ON s.id = fs.source_id
                 ORDER BY f.created_at ASC, f.id ASC",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map([], |row| {
                Ok((memory_fact_from_row(row)?, row.get::<_, String>(14)?))
            })
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let mut seen = HashSet::new();
        let mut facts = Vec::new();

        for row in rows {
            let (fact, metadata_json) =
                row.map_err(|source| sqlite_error(&self.database_path, source))?;
            let metadata = serde_json::from_str::<Value>(&metadata_json).map_err(|source| {
                MemoryDatabaseError::InvalidMemoryJson {
                    field: "memory source metadata_json",
                    source,
                }
            })?;
            let Some(run_id) = metadata.get("runId").and_then(Value::as_str) else {
                continue;
            };
            if run_ids.contains(run_id) && seen.insert(fact.id.clone()) {
                facts.push(fact);
            }
        }

        Ok(facts)
    }

    pub fn facts_for_source_reference(
        &self,
        source_type: MemorySourceType,
        source_id: &str,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        require_non_empty("source_id", source_id)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT f.id, f.scope, f.chat_id, f.status, f.kind, f.fact, f.confidence,
                        f.pinned, f.enabled, f.is_latest, f.expires_at, f.metadata_json, f.created_at,
                        f.updated_at
                 FROM memory_facts f
                 JOIN memory_fact_sources fs ON fs.fact_id = f.id
                 JOIN memory_sources s ON s.id = fs.source_id
                 WHERE s.source_type = ?1
                   AND s.source_id = ?2
                 ORDER BY f.created_at ASC, f.id ASC",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(
                params![source_type.as_str(), source_id],
                memory_fact_from_row,
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn facts_for_source_references(
        &self,
        source_type: MemorySourceType,
        source_ids: &[String],
    ) -> Result<Vec<(String, MemoryFactRecord)>, MemoryDatabaseError> {
        if source_ids.is_empty() {
            return Ok(Vec::new());
        }
        for source_id in source_ids {
            require_non_empty("source_id", source_id)?;
        }

        let placeholders = (2..=source_ids.len() + 1)
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT f.id, f.scope, f.chat_id, f.status, f.kind, f.fact, f.confidence,
                    f.pinned, f.enabled, f.is_latest, f.expires_at, f.metadata_json, f.created_at,
                    f.updated_at, s.source_id
             FROM memory_facts f
             JOIN memory_fact_sources fs ON fs.fact_id = f.id
             JOIN memory_sources s ON s.id = fs.source_id
             WHERE s.source_type = ?1
               AND s.source_id IN ({placeholders})
             ORDER BY f.created_at ASC, f.id ASC",
        );
        let mut parameters = Vec::with_capacity(source_ids.len() + 1);
        parameters.push(source_type.as_str());
        parameters.extend(source_ids.iter().map(String::as_str));
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(params_from_iter(parameters), |row| {
                Ok((row.get(14)?, memory_fact_from_row(row)?))
            })
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn source_count_for_fact(&self, fact_id: &str) -> Result<i64, MemoryDatabaseError> {
        require_non_empty("fact_id", fact_id)?;
        source_count_for_fact(&self.connection, &self.database_path, fact_id)
    }

    pub fn search_active_facts(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        require_non_empty("query", query)?;
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let mut statement = self
            .connection
            .prepare(
                "SELECT f.id, f.scope, f.chat_id, f.status, f.kind, f.fact, f.confidence,
                        f.pinned, f.enabled, f.is_latest, f.expires_at, f.metadata_json, f.created_at, f.updated_at
                 FROM memory_fts_index
                 JOIN memory_facts f ON f.id = memory_fts_index.fact_id
                 WHERE memory_fts_index MATCH ?1
                   AND f.status = 'active'
                   AND f.is_latest = 1
                 ORDER BY bm25(memory_fts_index),
                          f.pinned DESC,
                          f.updated_at DESC,
                          COALESCE(f.confidence, -1.0) DESC,
                          f.is_latest DESC,
                          f.id ASC
                 LIMIT ?2",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(params![query, limit], memory_fact_from_row)
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn search_active_facts_for_scope(
        &self,
        query: &str,
        chat_id: Option<&str>,
        kind: Option<MemoryKind>,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.search_active_facts_for_scope_page(query, chat_id, kind, limit, 0)
    }

    pub fn search_enabled_active_facts_for_scope(
        &self,
        query: &str,
        chat_id: Option<&str>,
        kind: Option<MemoryKind>,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.search_active_facts_for_scope_page_filtered(query, chat_id, kind, limit, 0, true)
    }

    pub fn search_active_facts_for_scope_page(
        &self,
        query: &str,
        chat_id: Option<&str>,
        kind: Option<MemoryKind>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.search_active_facts_for_scope_page_filtered(query, chat_id, kind, limit, offset, false)
    }

    fn search_active_facts_for_scope_page_filtered(
        &self,
        query: &str,
        chat_id: Option<&str>,
        kind: Option<MemoryKind>,
        limit: u32,
        offset: u32,
        enabled_only: bool,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        require_non_empty("query", query)?;
        if let Some(chat_id) = chat_id {
            require_non_empty("chat_id", chat_id)?;
        }
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let (filter_sql, chat_param) = match self.kind {
            MemoryDatabaseKind::Global => ("f.scope = 'global'", None),
            MemoryDatabaseKind::Workspace if chat_id.is_some() => (
                "(f.scope = 'chat' AND f.chat_id = ?2) OR f.scope = 'workspace'",
                chat_id,
            ),
            MemoryDatabaseKind::Workspace => ("f.scope = 'workspace'", None),
        };
        let enabled_filter_sql = if enabled_only {
            "AND f.enabled = 1"
        } else {
            ""
        };
        let sql = format!(
            "SELECT f.id, f.scope, f.chat_id, f.status, f.kind, f.fact, f.confidence,
                    f.pinned, f.enabled, f.is_latest, f.expires_at, f.metadata_json, f.created_at, f.updated_at
             FROM memory_fts_index
             JOIN memory_facts f ON f.id = memory_fts_index.fact_id
             WHERE memory_fts_index MATCH ?1
               AND ({filter_sql})
               AND f.status = 'active'
               {enabled_filter_sql}
               AND (?4 IS NULL OR f.kind = ?4)
               AND f.is_latest = 1
             ORDER BY
               CASE WHEN f.scope = 'chat' THEN 0 WHEN f.scope = 'workspace' THEN 1 ELSE 2 END,
               bm25(memory_fts_index),
               f.pinned DESC,
               f.updated_at DESC,
               COALESCE(f.confidence, -1.0) DESC,
               f.is_latest DESC,
               f.id ASC
             LIMIT ?3 OFFSET ?5"
        );
        let kind_param = kind
            .map(|kind| kind.as_str().to_string())
            .map(rusqlite::types::Value::Text)
            .unwrap_or(rusqlite::types::Value::Null);
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(
                params![query, chat_param, limit, kind_param, offset],
                memory_fact_from_row,
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn count_search_active_facts_for_scope(
        &self,
        query: &str,
        chat_id: Option<&str>,
        kind: Option<MemoryKind>,
    ) -> Result<u32, MemoryDatabaseError> {
        require_non_empty("query", query)?;
        if let Some(chat_id) = chat_id {
            require_non_empty("chat_id", chat_id)?;
        }

        let (filter_sql, chat_param) = match self.kind {
            MemoryDatabaseKind::Global => ("f.scope = 'global'", None),
            MemoryDatabaseKind::Workspace if chat_id.is_some() => (
                "(f.scope = 'chat' AND f.chat_id = ?2) OR f.scope = 'workspace'",
                chat_id,
            ),
            MemoryDatabaseKind::Workspace => ("f.scope = 'workspace'", None),
        };
        let sql = format!(
            "SELECT COUNT(*)
             FROM memory_fts_index
             JOIN memory_facts f ON f.id = memory_fts_index.fact_id
             WHERE memory_fts_index MATCH ?1
               AND ({filter_sql})
               AND f.status = 'active'
               AND (?3 IS NULL OR f.kind = ?3)
               AND f.is_latest = 1"
        );
        let kind_param = kind
            .map(|kind| kind.as_str().to_string())
            .map(rusqlite::types::Value::Text)
            .unwrap_or(rusqlite::types::Value::Null);
        let count = self
            .connection
            .query_row(&sql, params![query, chat_param, kind_param], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(count as u32)
    }

    pub fn find_active_facts_containing_any_for_scope(
        &self,
        terms: &[String],
        chat_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.find_active_facts_containing_any_for_scope_filtered(terms, chat_id, limit, false)
    }

    pub fn find_enabled_active_facts_containing_any_for_scope(
        &self,
        terms: &[String],
        chat_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.find_active_facts_containing_any_for_scope_filtered(terms, chat_id, limit, true)
    }

    fn find_active_facts_containing_any_for_scope_filtered(
        &self,
        terms: &[String],
        chat_id: Option<&str>,
        limit: u32,
        enabled_only: bool,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        if let Some(chat_id) = chat_id {
            require_non_empty("chat_id", chat_id)?;
        }
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let mut like_terms = Vec::new();
        for term in terms {
            require_non_empty("term", term)?;
            like_terms.push(format!("%{}%", escaped_memory_like_term(term)));
        }
        let like_filter_sql = (0..like_terms.len())
            .map(|index| format!("lower(fact) LIKE ?{} ESCAPE '\\'", index + 4))
            .collect::<Vec<_>>()
            .join(" OR ");
        let (filter_sql, chat_param) = match self.kind {
            MemoryDatabaseKind::Global => ("scope = 'global'", None),
            MemoryDatabaseKind::Workspace if chat_id.is_some() => (
                "(scope = 'chat' AND chat_id = ?1) OR scope = 'workspace'",
                chat_id,
            ),
            MemoryDatabaseKind::Workspace => ("scope = 'workspace'", None),
        };
        let enabled_filter_sql = if enabled_only { "AND enabled = 1" } else { "" };
        let sql = format!(
            "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, enabled, is_latest,
                    expires_at, metadata_json, created_at, updated_at
             FROM memory_facts
             WHERE ({filter_sql})
               AND status = ?3
               {enabled_filter_sql}
               AND is_latest = 1
               AND ({like_filter_sql})
             ORDER BY
               CASE WHEN scope = 'chat' THEN 0 WHEN scope = 'workspace' THEN 1 ELSE 2 END,
               pinned DESC,
               updated_at DESC,
               COALESCE(confidence, -1.0) DESC,
               id ASC
             LIMIT ?2"
        );
        let chat_value = chat_param
            .map(|value| rusqlite::types::Value::Text(value.to_string()))
            .unwrap_or(rusqlite::types::Value::Null);
        let mut params = vec![
            chat_value,
            rusqlite::types::Value::Integer(i64::from(limit)),
            rusqlite::types::Value::Text(MemoryStatus::Active.as_str().to_string()),
        ];
        params.extend(like_terms.into_iter().map(rusqlite::types::Value::Text));
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(params_from_iter(params), memory_fact_from_row)
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn related_enabled_active_facts(
        &self,
        seed_fact_ids: &[String],
        max_depth: u32,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.related_active_facts_filtered(seed_fact_ids, max_depth, limit, true)
    }

    pub fn related_active_facts(
        &self,
        seed_fact_ids: &[String],
        max_depth: u32,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.related_active_facts_filtered(seed_fact_ids, max_depth, limit, false)
    }

    fn related_active_facts_filtered(
        &self,
        seed_fact_ids: &[String],
        max_depth: u32,
        limit: u32,
        enabled_only: bool,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }
        if max_depth == 0 || seed_fact_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut seen = HashSet::new();
        let mut frontier = Vec::new();
        for fact_id in seed_fact_ids {
            require_non_empty("seed_fact_id", fact_id)?;
            if seen.insert(fact_id.clone()) {
                frontier.push(fact_id.clone());
            }
        }

        let mut related = Vec::new();
        for _ in 0..max_depth {
            if frontier.is_empty() || related.len() >= limit as usize {
                break;
            }

            let mut next_frontier = Vec::new();
            for fact_id in frontier {
                let neighbor_ids =
                    related_fact_ids(&self.connection, &self.database_path, &fact_id)?;
                for neighbor_id in neighbor_ids {
                    if !seen.insert(neighbor_id.clone()) {
                        continue;
                    }
                    next_frontier.push(neighbor_id.clone());
                    let fact = if enabled_only {
                        self.enabled_active_latest_fact(&neighbor_id)?
                    } else {
                        self.active_latest_fact(&neighbor_id)?
                    };
                    if let Some(fact) = fact {
                        related.push(fact);
                        if related.len() >= limit as usize {
                            break;
                        }
                    }
                }
                if related.len() >= limit as usize {
                    break;
                }
            }
            frontier = next_frontier;
        }

        Ok(related)
    }

    pub fn list_active_facts_for_scope(
        &self,
        chat_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.list_facts_for_scope(chat_id, MemoryStatus::Active, None, None, limit)
    }

    pub fn list_enabled_active_facts_for_scope(
        &self,
        chat_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.list_facts_for_scope_page_filtered(
            chat_id,
            MemoryStatus::Active,
            None,
            None,
            limit,
            0,
            true,
        )
    }

    pub fn list_facts_for_scope(
        &self,
        chat_id: Option<&str>,
        status: MemoryStatus,
        kind: Option<MemoryKind>,
        query: Option<&str>,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.list_facts_for_scope_page(chat_id, status, kind, query, limit, 0)
    }

    pub fn list_facts_for_scope_page(
        &self,
        chat_id: Option<&str>,
        status: MemoryStatus,
        kind: Option<MemoryKind>,
        query: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        self.list_facts_for_scope_page_filtered(chat_id, status, kind, query, limit, offset, false)
    }

    fn list_facts_for_scope_page_filtered(
        &self,
        chat_id: Option<&str>,
        status: MemoryStatus,
        kind: Option<MemoryKind>,
        query: Option<&str>,
        limit: u32,
        offset: u32,
        enabled_only: bool,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        if let Some(chat_id) = chat_id {
            require_non_empty("chat_id", chat_id)?;
        }
        if let Some(query) = query {
            require_non_empty("query", query)?;
        }
        if limit == 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "limit must be greater than 0".to_string(),
            });
        }

        let (filter_sql, chat_param) = memory_facts_scope_filter_sql(self.kind, chat_id);
        let sql = memory_facts_list_page_sql(filter_sql, enabled_only);
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(
                params![
                    chat_param,
                    limit,
                    status.as_str(),
                    kind.map(MemoryKind::as_str),
                    query.map(|query| format!(
                        "%{}%",
                        escaped_memory_like_term(&query.to_lowercase())
                    )),
                    offset,
                ],
                memory_fact_from_row,
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn count_facts_for_scope(
        &self,
        chat_id: Option<&str>,
        status: MemoryStatus,
        kind: Option<MemoryKind>,
        query: Option<&str>,
    ) -> Result<u32, MemoryDatabaseError> {
        if let Some(chat_id) = chat_id {
            require_non_empty("chat_id", chat_id)?;
        }
        if let Some(query) = query {
            require_non_empty("query", query)?;
        }

        let (filter_sql, chat_param) = memory_facts_scope_filter_sql(self.kind, chat_id);
        let sql = memory_facts_count_sql(filter_sql);
        let count = self
            .connection
            .query_row(
                &sql,
                params![
                    chat_param,
                    status.as_str(),
                    kind.map(MemoryKind::as_str),
                    query.map(|query| format!(
                        "%{}%",
                        escaped_memory_like_term(&query.to_lowercase())
                    )),
                ],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(count as u32)
    }

    pub fn list_fact_ids_for_exact_scope(
        &self,
        scope: MemoryScope,
        chat_id: Option<&str>,
        status: MemoryStatus,
        kind: Option<MemoryKind>,
        query: Option<&str>,
    ) -> Result<Vec<String>, MemoryDatabaseError> {
        self.validate_scope(scope)?;
        validate_scope_chat_id(scope, chat_id)?;
        if let Some(query) = query {
            require_non_empty("query", query)?;
        }

        if status == MemoryStatus::Active && query.is_some() {
            return self.search_active_fact_ids_for_exact_scope(
                scope,
                chat_id,
                kind,
                query.unwrap(),
            );
        }

        let sql = "SELECT id
             FROM memory_facts
             WHERE scope = ?1
               AND ((?2 IS NULL AND chat_id IS NULL) OR chat_id = ?2)
               AND status = ?3
               AND (?4 IS NULL OR kind = ?4)
               AND (?5 IS NULL OR lower(fact) LIKE ?5 ESCAPE '\\')
               AND is_latest = 1
             ORDER BY pinned DESC, updated_at DESC, id ASC";
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(
                params![
                    scope.as_str(),
                    chat_id,
                    status.as_str(),
                    kind.map(MemoryKind::as_str),
                    query.map(|query| format!(
                        "%{}%",
                        escaped_memory_like_term(&query.to_lowercase())
                    )),
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    fn search_active_fact_ids_for_exact_scope(
        &self,
        scope: MemoryScope,
        chat_id: Option<&str>,
        kind: Option<MemoryKind>,
        query: &str,
    ) -> Result<Vec<String>, MemoryDatabaseError> {
        let sql = "SELECT f.id
             FROM memory_fts_index
             JOIN memory_facts f ON f.id = memory_fts_index.fact_id
             WHERE memory_fts_index MATCH ?1
               AND f.scope = ?2
               AND ((?3 IS NULL AND f.chat_id IS NULL) OR f.chat_id = ?3)
               AND f.status = 'active'
               AND (?4 IS NULL OR f.kind = ?4)
               AND f.is_latest = 1
             ORDER BY bm25(memory_fts_index), f.pinned DESC, f.updated_at DESC, f.id ASC";
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(
                params![query, scope.as_str(), chat_id, kind.map(MemoryKind::as_str)],
                |row| row.get::<_, String>(0),
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    fn latest_active_facts_for_exact_scope(
        &self,
        scope: MemoryScope,
        chat_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, enabled, is_latest,
                        expires_at, metadata_json, created_at, updated_at
                 FROM memory_facts
                 WHERE scope = ?1
                   AND ((?2 IS NULL AND chat_id IS NULL) OR chat_id = ?2)
                   AND status = 'active'
                   AND enabled = 1
                   AND is_latest = 1
                 ORDER BY pinned DESC, kind ASC, lower(fact) ASC, id ASC
                 LIMIT ?3",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(
                params![scope.as_str(), chat_id, limit],
                memory_fact_from_row,
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn promote_fact(
        &mut self,
        source_fact_id: &str,
        promoted_fact_id: &str,
        target_scope: MemoryScope,
        target_chat_id: Option<&str>,
    ) -> Result<MemoryFactRecord, MemoryDatabaseError> {
        self.validate_scope(target_scope)?;
        let fact =
            self.fact(source_fact_id)?
                .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                    message: format!("memory fact was not found: {source_fact_id}"),
                })?;
        let source_kind = memory_kind_from_str(&fact.kind)?;
        ensure_memory_kind_scope_allowed(target_scope, source_kind)?;
        let sources = self.sources_for_fact(source_fact_id)?;

        for (index, source) in sources.iter().enumerate() {
            self.insert_source(NewMemorySource {
                id: &promoted_source_id(promoted_fact_id, index),
                scope: target_scope,
                chat_id: target_chat_id,
                source_type: memory_source_type_from_str(&source.source_type)?,
                source_id: source.source_id.as_deref(),
                title: &source.title,
                content: &source.content,
                metadata_json: &source.metadata_json,
            })?;
        }

        let promoted_source_ids = promoted_source_ids(promoted_fact_id, sources.len());
        let promoted_source_refs = promoted_source_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        self.insert_fact(NewMemoryFact {
            id: promoted_fact_id,
            scope: target_scope,
            chat_id: target_chat_id,
            status: memory_status_from_str(&fact.status)?,
            kind: source_kind,
            fact: &fact.fact,
            confidence: fact.confidence,
            pinned: fact.pinned,
            source_ids: &promoted_source_refs,
            metadata_json: &fact.metadata_json,
        })?;
        if !fact.enabled {
            self.set_fact_enabled(promoted_fact_id, false)?;
        }

        self.fact(promoted_fact_id)?
            .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                message: format!("promoted memory fact was not found: {promoted_fact_id}"),
            })
    }

    pub fn promote_fact_to_database(
        &self,
        source_fact_id: &str,
        target: &mut MemoryDatabase,
        promoted_fact_id: &str,
        target_scope: MemoryScope,
        target_chat_id: Option<&str>,
    ) -> Result<MemoryFactRecord, MemoryDatabaseError> {
        target.validate_scope(target_scope)?;
        let fact =
            self.fact(source_fact_id)?
                .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                    message: format!("memory fact was not found: {source_fact_id}"),
                })?;
        let source_kind = memory_kind_from_str(&fact.kind)?;
        ensure_memory_kind_scope_allowed(target_scope, source_kind)?;
        let sources = self.sources_for_fact(source_fact_id)?;

        for (index, source) in sources.iter().enumerate() {
            target.insert_source(NewMemorySource {
                id: &promoted_source_id(promoted_fact_id, index),
                scope: target_scope,
                chat_id: target_chat_id,
                source_type: memory_source_type_from_str(&source.source_type)?,
                source_id: source.source_id.as_deref(),
                title: &source.title,
                content: &source.content,
                metadata_json: &source.metadata_json,
            })?;
        }

        let promoted_source_ids = promoted_source_ids(promoted_fact_id, sources.len());
        let promoted_source_refs = promoted_source_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        target.insert_fact(NewMemoryFact {
            id: promoted_fact_id,
            scope: target_scope,
            chat_id: target_chat_id,
            status: memory_status_from_str(&fact.status)?,
            kind: source_kind,
            fact: &fact.fact,
            confidence: fact.confidence,
            pinned: fact.pinned,
            source_ids: &promoted_source_refs,
            metadata_json: &fact.metadata_json,
        })?;
        if !fact.enabled {
            target.set_fact_enabled(promoted_fact_id, false)?;
        }

        target
            .fact(promoted_fact_id)?
            .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                message: format!("promoted memory fact was not found: {promoted_fact_id}"),
            })
    }

    /// Copy a source fact (and its sources) into `target` under
    /// `target_fact_id`, skipping the write when the target already contains
    /// that fact id. The target fact id must match the source kind and fact
    /// text; anything else is an id collision and fails without touching the
    /// source. This gives moves a retryable boundary across two SQLite files:
    /// re-running the copy after a partial failure never creates a second
    /// target copy.
    pub fn copy_fact_to_database_idempotent(
        &self,
        target: &mut MemoryDatabase,
        target_fact_id: &str,
        target_scope: MemoryScope,
        target_chat_id: Option<&str>,
    ) -> Result<MemoryFactCopyOutcome, MemoryDatabaseError> {
        require_non_empty("target_fact_id", target_fact_id)?;
        let fact =
            self.fact(target_fact_id)?
                .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                    message: format!("memory fact was not found: {target_fact_id}"),
                })?;
        let sources = self.sources_for_fact(target_fact_id)?;
        target.write_fact_copy_idempotent(
            target_fact_id,
            target_scope,
            target_chat_id,
            &fact,
            &sources,
        )
    }

    /// Write a serialized fact (and its sources) into `target` under
    /// `target_fact_id`, skipping the write when the target already contains
    /// that fact id. The target fact id must match the source kind and fact
    /// text; anything else is an id collision and fails without touching the
    /// source. Used by the local move handler and by the remote sidecar, which
    /// receives the fact payload from the main process instead of a second
    /// SQLite handle; both paths share the same idempotency boundary.
    pub fn write_fact_copy_idempotent(
        &mut self,
        target_fact_id: &str,
        target_scope: MemoryScope,
        target_chat_id: Option<&str>,
        fact: &MemoryFactRecord,
        sources: &[MemorySourceRecord],
    ) -> Result<MemoryFactCopyOutcome, MemoryDatabaseError> {
        require_non_empty("target_fact_id", target_fact_id)?;
        self.validate_scope(target_scope)?;
        let source_kind = memory_kind_from_str(&fact.kind)?;
        ensure_memory_kind_scope_allowed(target_scope, source_kind)?;

        if let Some(existing) = self.fact(target_fact_id)? {
            let existing_kind = memory_kind_from_str(&existing.kind)?;
            if existing_kind != source_kind || existing.fact != fact.fact {
                return Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: format!(
                        "target memory fact id '{target_fact_id}' already exists with different content"
                    ),
                });
            }
            // A previous attempt may have written the fact but failed before
            // preserving a disabled state; reconcile it so a retry ends in the
            // same terminal state as a fresh move.
            if existing.enabled != fact.enabled {
                self.set_fact_enabled(target_fact_id, fact.enabled)?;
            }
            let target_fact = self.fact(target_fact_id)?.ok_or_else(|| {
                MemoryDatabaseError::InvalidMemoryInput {
                    message: format!("copied memory fact was not found: {target_fact_id}"),
                }
            })?;
            return Ok(MemoryFactCopyOutcome {
                target_fact,
                target_pre_existed: true,
            });
        }

        for (index, source) in sources.iter().enumerate() {
            let source_id = promoted_source_id(target_fact_id, index);
            // Sources are written before the fact row, so a crash in between
            // leaves orphaned source rows. Re-running the copy must not collide
            // with those rows: matching rows are reused, mismatching rows are a
            // real id collision and fail without touching the source.
            if let Some(existing_source) = self.source(&source_id)? {
                if !copied_source_matches(&existing_source, source) {
                    return Err(MemoryDatabaseError::InvalidMemoryInput {
                        message: format!(
                            "target memory source id '{source_id}' already exists with different content"
                        ),
                    });
                }
                continue;
            }
            self.insert_source(NewMemorySource {
                id: &source_id,
                scope: target_scope,
                chat_id: target_chat_id,
                source_type: memory_source_type_from_str(&source.source_type)?,
                source_id: source.source_id.as_deref(),
                title: &source.title,
                content: &source.content,
                metadata_json: &source.metadata_json,
            })?;
        }

        let copied_source_ids = promoted_source_ids(target_fact_id, sources.len());
        let copied_source_refs = copied_source_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        self.insert_fact(NewMemoryFact {
            id: target_fact_id,
            scope: target_scope,
            chat_id: target_chat_id,
            status: memory_status_from_str(&fact.status)?,
            kind: source_kind,
            fact: &fact.fact,
            confidence: fact.confidence,
            pinned: fact.pinned,
            source_ids: &copied_source_refs,
            metadata_json: &fact.metadata_json,
        })?;
        if !fact.enabled {
            self.set_fact_enabled(target_fact_id, false)?;
        }

        let target_fact =
            self.fact(target_fact_id)?
                .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                    message: format!("copied memory fact was not found: {target_fact_id}"),
                })?;
        Ok(MemoryFactCopyOutcome {
            target_fact,
            target_pre_existed: false,
        })
    }

    /// Finalize a move on the source side: mark the source fact superseded and
    /// record where it moved. Idempotent: re-running after a partial failure
    /// (or after a lost response) leaves the source in the same terminal state.
    pub fn mark_fact_moved(
        &mut self,
        source_fact_id: &str,
        moved_to_workspace_id: &str,
        target_fact_id: &str,
    ) -> Result<MemoryFactRecord, MemoryDatabaseError> {
        require_non_empty("source_fact_id", source_fact_id)?;
        require_non_empty("moved_to_workspace_id", moved_to_workspace_id)?;
        require_non_empty("target_fact_id", target_fact_id)?;
        let current =
            self.fact(source_fact_id)?
                .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                    message: format!("memory fact was not found: {source_fact_id}"),
                })?;
        let mut metadata = serde_json::from_str::<Value>(&current.metadata_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        if let Some(object) = metadata.as_object_mut() {
            object.insert(
                "movedToWorkspace".to_string(),
                Value::String(moved_to_workspace_id.to_string()),
            );
            object.insert(
                "movedTargetFactId".to_string(),
                Value::String(target_fact_id.to_string()),
            );
            object.insert("movedAt".to_string(), Value::String(now_timestamp()));
        }
        let metadata_json = serde_json::to_string(&metadata).map_err(|source| {
            MemoryDatabaseError::InvalidMemoryInput {
                message: format!("failed to serialize moved memory metadata: {source}"),
            }
        })?;
        self.update_fact(UpdateMemoryFact {
            id: source_fact_id,
            status: Some(MemoryStatus::Superseded),
            metadata_json: Some(&metadata_json),
            ..UpdateMemoryFact::default()
        })?;
        self.fact(source_fact_id)?
            .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                message: format!("memory fact was not found after move: {source_fact_id}"),
            })
    }

    fn validate_scope(&self, scope: MemoryScope) -> Result<(), MemoryDatabaseError> {
        match (self.kind, scope) {
            (MemoryDatabaseKind::Global, MemoryScope::Global)
            | (MemoryDatabaseKind::Workspace, MemoryScope::Workspace | MemoryScope::Chat) => Ok(()),
            (MemoryDatabaseKind::Global, MemoryScope::Workspace | MemoryScope::Chat) => {
                Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: format!(
                        "global memory database only accepts global scope, got '{}'",
                        scope.as_str()
                    ),
                })
            }
            (MemoryDatabaseKind::Workspace, MemoryScope::Global) => {
                Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: "workspace memory database does not accept global scope".to_string(),
                })
            }
        }
    }

    fn validate_dream_scope(
        &self,
        scope: MemoryDreamScope,
        workspace_id: Option<&str>,
    ) -> Result<(), MemoryDatabaseError> {
        if let Some(workspace_id) = workspace_id {
            require_non_empty("workspace_id", workspace_id)?;
        }

        match (self.kind, scope, workspace_id) {
            (MemoryDatabaseKind::Global, MemoryDreamScope::Global, None)
            | (MemoryDatabaseKind::Workspace, MemoryDreamScope::Workspace, _) => Ok(()),
            (MemoryDatabaseKind::Global, MemoryDreamScope::Global, Some(_)) => {
                Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: "global memory Dream must not include workspace_id".to_string(),
                })
            }
            (MemoryDatabaseKind::Global, MemoryDreamScope::Workspace, _) => {
                Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: "global memory database does not accept workspace Dream scope"
                        .to_string(),
                })
            }
            (MemoryDatabaseKind::Workspace, MemoryDreamScope::Global, _) => {
                Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: "workspace memory database does not accept global Dream scope"
                        .to_string(),
                })
            }
        }
    }
}

#[derive(Debug)]
pub enum MemoryDatabaseError {
    ConcurrencyLimit {
        message: String,
    },
    /// Another active Dream job already exists for this database/scope singleflight.
    AlreadyActive {
        message: String,
    },
    InvalidMemoryInput {
        message: String,
    },
    InvalidMemoryJson {
        field: &'static str,
        source: serde_json::Error,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
    MissingDatabaseParent {
        path: PathBuf,
    },
    Sqlite {
        path: PathBuf,
        source: rusqlite::Error,
    },
    UnsupportedSchemaVersion {
        path: PathBuf,
        found: u32,
        latest: u32,
    },
}

impl fmt::Display for MemoryDatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConcurrencyLimit { message } => write!(formatter, "{message}"),
            Self::AlreadyActive { message } => write!(formatter, "{message}"),
            Self::InvalidMemoryInput { message } => {
                write!(formatter, "invalid memory data: {message}")
            }
            Self::InvalidMemoryJson { field, source } => {
                write!(formatter, "invalid memory JSON in {field}: {source}")
            }
            Self::Io { path, source } => write!(formatter, "{}: {}", path.display(), source),
            Self::MissingDatabaseParent { path } => write!(
                formatter,
                "memory database path has no parent directory: {}",
                path.display()
            ),
            Self::Sqlite { path, source } => {
                write!(formatter, "{} SQLite error: {}", path.display(), source)
            }
            Self::UnsupportedSchemaVersion {
                path,
                found,
                latest,
            } => write!(
                formatter,
                "{} has unsupported memory database schema version {}; latest supported version is {}",
                path.display(),
                found,
                latest
            ),
        }
    }
}

impl std::error::Error for MemoryDatabaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidMemoryJson { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::Sqlite { source, .. } => Some(source),
            Self::ConcurrencyLimit { .. }
            | Self::AlreadyActive { .. }
            | Self::InvalidMemoryInput { .. }
            | Self::MissingDatabaseParent { .. }
            | Self::UnsupportedSchemaVersion { .. } => None,
        }
    }
}

pub fn global_memory_database_path(foco_root_dir: impl AsRef<Path>) -> PathBuf {
    foco_root_dir.as_ref().join(GLOBAL_MEMORY_DATABASE_FILE)
}

fn open_connection(database_path: &Path) -> Result<Connection, MemoryDatabaseError> {
    prepare_private_file(database_path).map_err(|source| MemoryDatabaseError::Io {
        path: database_path.to_path_buf(),
        source,
    })?;
    let connection =
        Connection::open(database_path).map_err(|source| MemoryDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;

    connection
        .busy_timeout(GLOBAL_MEMORY_DATABASE_BUSY_TIMEOUT)
        .map_err(|source| MemoryDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|source| MemoryDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;

    Ok(connection)
}

fn enable_write_ahead_logging(
    connection: &Connection,
    database_path: &Path,
) -> Result<(), MemoryDatabaseError> {
    let journal_mode: String = connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(|source| MemoryDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;
    if journal_mode.eq_ignore_ascii_case("wal") {
        return Ok(());
    }

    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|source| MemoryDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })
}

fn run_global_migrations(
    connection: &mut Connection,
    database_path: &Path,
) -> Result<(), MemoryDatabaseError> {
    let current_version = schema_version(connection, database_path)?;

    if current_version > GLOBAL_MEMORY_SCHEMA_VERSION {
        return Err(MemoryDatabaseError::UnsupportedSchemaVersion {
            path: database_path.to_path_buf(),
            found: current_version,
            latest: GLOBAL_MEMORY_SCHEMA_VERSION,
        });
    }

    if current_version == GLOBAL_MEMORY_SCHEMA_VERSION {
        return Ok(());
    }

    let _migration_lock = acquire_global_memory_migration_lock(database_path)?;
    let current_version = schema_version(connection, database_path)?;

    if current_version > GLOBAL_MEMORY_SCHEMA_VERSION {
        return Err(MemoryDatabaseError::UnsupportedSchemaVersion {
            path: database_path.to_path_buf(),
            found: current_version,
            latest: GLOBAL_MEMORY_SCHEMA_VERSION,
        });
    }

    if current_version == GLOBAL_MEMORY_SCHEMA_VERSION {
        return Ok(());
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|source| sqlite_error(database_path, source))?;
    let current_version = schema_version(&transaction, database_path)?;

    if current_version > GLOBAL_MEMORY_SCHEMA_VERSION {
        return Err(MemoryDatabaseError::UnsupportedSchemaVersion {
            path: database_path.to_path_buf(),
            found: current_version,
            latest: GLOBAL_MEMORY_SCHEMA_VERSION,
        });
    }

    if current_version == GLOBAL_MEMORY_SCHEMA_VERSION {
        transaction
            .commit()
            .map_err(|source| sqlite_error(database_path, source))?;
        return Ok(());
    }

    for migration in GLOBAL_MEMORY_MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current_version)
    {
        if migration.version == 5
            && table_has_column(&transaction, database_path, "memory_facts", "enabled")?
        {
            transaction
                .pragma_update(None, "user_version", migration.version)
                .map_err(|source| sqlite_error(database_path, source))?;
            continue;
        }
        transaction
            .execute_batch(migration.sql)
            .map_err(|source| sqlite_error(database_path, source))?;
        transaction
            .pragma_update(None, "user_version", migration.version)
            .map_err(|source| sqlite_error(database_path, source))?;
    }

    transaction
        .commit()
        .map_err(|source| sqlite_error(database_path, source))?;

    Ok(())
}

struct GlobalMemoryMigrationLock {
    _file: fs::File,
}

fn acquire_global_memory_migration_lock(
    database_path: &Path,
) -> Result<GlobalMemoryMigrationLock, MemoryDatabaseError> {
    let lock_path = global_memory_migration_lock_path(database_path);
    if let Some(parent) = lock_path.parent() {
        create_directory(parent)?;
    }
    prepare_private_file(&lock_path).map_err(|source| MemoryDatabaseError::Io {
        path: lock_path.clone(),
        source,
    })?;

    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|source| MemoryDatabaseError::Io {
            path: lock_path.clone(),
            source,
        })?;
    lock_file_exclusive(&file).map_err(|source| MemoryDatabaseError::Io {
        path: lock_path,
        source,
    })?;

    Ok(GlobalMemoryMigrationLock { _file: file })
}

fn global_memory_migration_lock_path(database_path: &Path) -> PathBuf {
    let resolved = database_path
        .canonicalize()
        .unwrap_or_else(|_| database_path.to_path_buf());
    let file_name = resolved
        .file_name()
        .map(|name| {
            let mut lock_name = name.to_os_string();
            lock_name.push(GLOBAL_MEMORY_MIGRATION_LOCK_SUFFIX);
            lock_name
        })
        .unwrap_or_else(|| format!("memory.sqlite{GLOBAL_MEMORY_MIGRATION_LOCK_SUFFIX}").into());
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
        "global memory database migration lock is not supported on this platform",
    ))
}

fn table_has_column(
    connection: &Connection,
    database_path: &Path,
    table_name: &str,
    column_name: &str,
) -> Result<bool, MemoryDatabaseError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table_name})"))
        .map_err(|source| sqlite_error(database_path, source))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|source| sqlite_error(database_path, source))?;
    for row in rows {
        if row.map_err(|source| sqlite_error(database_path, source))? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn schema_version(
    connection: &Connection,
    database_path: &Path,
) -> Result<u32, MemoryDatabaseError> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|source| sqlite_error(database_path, source))
}

fn upsert_fact_fts_data(
    transaction: &Transaction<'_>,
    database_path: &Path,
    fact: &NewMemoryFact<'_>,
    updated_at: &str,
) -> Result<(), MemoryDatabaseError> {
    transaction
        .execute(
            "INSERT INTO memory_fts_data
                (fact_id, scope, chat_id, status, kind, title, body, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(fact_id) DO UPDATE SET
                scope = excluded.scope,
                chat_id = excluded.chat_id,
                status = excluded.status,
                kind = excluded.kind,
                title = excluded.title,
                body = excluded.body,
                updated_at = excluded.updated_at",
            params![
                fact.id,
                fact.scope.as_str(),
                fact.chat_id,
                fact.status.as_str(),
                fact.kind.as_str(),
                fact.kind.as_str(),
                fact.fact,
                updated_at,
            ],
        )
        .map_err(|source| sqlite_error(database_path, source))?;

    Ok(())
}

fn upsert_fact_record_fts_data(
    transaction: &Transaction<'_>,
    database_path: &Path,
    fact: &MemoryFactRecord,
) -> Result<(), MemoryDatabaseError> {
    transaction
        .execute(
            "INSERT INTO memory_fts_data
                (fact_id, scope, chat_id, status, kind, title, body, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(fact_id) DO UPDATE SET
                scope = excluded.scope,
                chat_id = excluded.chat_id,
                status = excluded.status,
                kind = excluded.kind,
                title = excluded.title,
                body = excluded.body,
                updated_at = excluded.updated_at",
            params![
                fact.id,
                fact.scope,
                fact.chat_id,
                fact.status,
                fact.kind,
                fact.kind,
                fact.fact,
                fact.updated_at,
            ],
        )
        .map_err(|source| sqlite_error(database_path, source))?;

    Ok(())
}

fn validate_source(source: &NewMemorySource<'_>) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", source.id)?;
    validate_scope_chat_id(source.scope, source.chat_id)?;
    if let Some(source_id) = source.source_id {
        require_non_empty("source_id", source_id)?;
    }
    if source.content.trim().is_empty() {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "content must not be empty".to_string(),
        });
    }
    validate_json("metadata_json", source.metadata_json)
}

fn validate_source_update(source: &UpdateMemorySource<'_>) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", source.id)?;
    if source.title.is_none() && source.content.is_none() && source.metadata_json.is_none() {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "source update must change at least one field".to_string(),
        });
    }
    if let Some(content) = source.content
        && content.trim().is_empty()
    {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "content must not be empty".to_string(),
        });
    }
    if let Some(metadata_json) = source.metadata_json {
        validate_json("metadata_json", metadata_json)?;
    }

    Ok(())
}

fn validate_fact(fact: &NewMemoryFact<'_>) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", fact.id)?;
    validate_scope_chat_id(fact.scope, fact.chat_id)?;
    if fact.fact.trim().is_empty() {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "fact must not be empty".to_string(),
        });
    }
    if let Some(confidence) = fact.confidence
        && !(0.0..=1.0).contains(&confidence)
    {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("confidence must be between 0 and 1, got {confidence}"),
        });
    }
    if fact.kind != MemoryKind::UserNote && fact.source_ids.is_empty() {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "non-user_note facts must reference at least one source".to_string(),
        });
    }
    for source_id in fact.source_ids {
        require_non_empty("source_id", source_id)?;
    }
    validate_json("metadata_json", fact.metadata_json)
}

fn validate_fact_update(fact: &UpdateMemoryFact<'_>) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", fact.id)?;
    if fact.scope.is_none()
        && fact.chat_id.is_none()
        && fact.status.is_none()
        && fact.kind.is_none()
        && fact.fact.is_none()
        && fact.confidence.is_none()
        && fact.pinned.is_none()
        && fact.is_latest.is_none()
        && fact.expires_at.is_none()
        && fact.metadata_json.is_none()
    {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "fact update must change at least one field".to_string(),
        });
    }
    if let Some(scope) = fact.scope {
        validate_scope_chat_id(scope, fact.chat_id)?;
    }
    if let Some(text) = fact.fact
        && text.trim().is_empty()
    {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "fact must not be empty".to_string(),
        });
    }
    if let Some(confidence) = fact.confidence
        && !(0.0..=1.0).contains(&confidence)
    {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("confidence must be between 0 and 1, got {confidence}"),
        });
    }
    if let Some(metadata_json) = fact.metadata_json {
        validate_json("metadata_json", metadata_json)?;
    }

    Ok(())
}

fn validate_edge(edge: &NewMemoryEdge<'_>) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", edge.id)?;
    require_non_empty("source_fact_id", edge.source_fact_id)?;
    require_non_empty("target_fact_id", edge.target_fact_id)?;
    if edge.source_fact_id == edge.target_fact_id {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "memory edge cannot target the same fact".to_string(),
        });
    }
    validate_json("metadata_json", edge.metadata_json)
}

fn validate_profile(profile: &NewMemoryProfile<'_>) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", profile.id)?;
    validate_scope_chat_id(profile.scope, profile.chat_id)?;
    validate_json("metadata_json", profile.metadata_json)
}

fn validate_extraction_job(job: &NewMemoryExtractionJob<'_>) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", job.id)?;
    validate_scope_chat_id(job.scope, job.chat_id)?;
    validate_json("input_json", job.input_json)?;
    if let Some(model_id) = job.model_id {
        require_non_empty("model_id", model_id)?;
    }
    if let Some(output_json) = job.output_json {
        validate_json("output_json", output_json)?;
    }
    if let Some(error_message) = job.error_message {
        require_non_empty("error_message", error_message)?;
    }
    if job.status == MemoryExtractionJobStatus::Queued
        && (job.output_json.is_some() || job.error_message.is_some())
    {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "queued memory extraction job must not include output or error".to_string(),
        });
    }

    Ok(())
}

fn validate_dream_job(job: &NewMemoryDreamJob<'_>) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", job.id)?;
    if let Some(workspace_id) = job.workspace_id {
        require_non_empty("workspace_id", workspace_id)?;
    }
    if let Some(model_id) = job.model_id {
        require_non_empty("model_id", model_id)?;
    }
    if let Some(transcript_chat_id) = job.transcript_chat_id {
        require_non_empty("transcript_chat_id", transcript_chat_id)?;
    }
    validate_json("input_summary_json", job.input_summary_json)?;
    if let Some(output_summary_json) = job.output_summary_json {
        validate_json("output_summary_json", output_summary_json)?;
    }
    validate_dream_job_status_payload(job.status, job.output_summary_json, job.error_message)
}

fn validate_dream_job_update(update: &UpdateMemoryDreamJob<'_>) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", update.id)?;
    if let Some(transcript_chat_id) = update.transcript_chat_id {
        require_non_empty("transcript_chat_id", transcript_chat_id)?;
    }
    if let Some(output_summary_json) = update.output_summary_json {
        validate_json("output_summary_json", output_summary_json)?;
    }
    validate_dream_job_status_payload(
        update.status,
        update.output_summary_json,
        update.error_message,
    )
}

fn validate_dream_job_status_payload(
    status: MemoryDreamJobStatus,
    output_summary_json: Option<&str>,
    error_message: Option<&str>,
) -> Result<(), MemoryDatabaseError> {
    if let Some(error_message) = error_message {
        require_non_empty("error_message", error_message)?;
    }

    match status {
        MemoryDreamJobStatus::Queued | MemoryDreamJobStatus::Running => {
            if output_summary_json.is_some() || error_message.is_some() {
                return Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: format!(
                        "{} memory Dream job must not include output or error",
                        status.as_str()
                    ),
                });
            }
        }
        MemoryDreamJobStatus::Completed => {
            if error_message.is_some() {
                return Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: "completed memory Dream job must not include error".to_string(),
                });
            }
        }
        MemoryDreamJobStatus::Failed => {
            if error_message.is_none() {
                return Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: "failed memory Dream job requires error_message".to_string(),
                });
            }
        }
        MemoryDreamJobStatus::Cancelled | MemoryDreamJobStatus::Skipped => {}
    }

    Ok(())
}

fn validate_dream_change(change: &NewMemoryDreamChange<'_>) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", change.id)?;
    require_non_empty("job_id", change.job_id)?;
    require_non_empty("operation", change.operation)?;
    validate_json_array("target_fact_ids_json", change.target_fact_ids_json)?;
    if let Some(new_fact_id) = change.new_fact_id {
        require_non_empty("new_fact_id", new_fact_id)?;
    }
    if let Some(before_json) = change.before_json {
        validate_json("before_json", before_json)?;
    }
    if let Some(after_json) = change.after_json {
        validate_json("after_json", after_json)?;
    }
    require_non_empty("reason", change.reason)?;
    if let Some(confidence) = change.confidence
        && !(0.0..=1.0).contains(&confidence)
    {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("confidence must be between 0 and 1, got {confidence}"),
        });
    }
    validate_dream_risk_level(change.risk_level)?;
    validate_json_array("evidence_json", change.evidence_json)?;
    validate_dream_change_status_payload(change.status, change.error_message)
}

fn validate_dream_change_update(
    update: &UpdateMemoryDreamChange<'_>,
) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", update.id)?;
    if let Some(after_json) = update.after_json {
        validate_json("after_json", after_json)?;
    }
    validate_dream_change_status_payload(update.status, update.error_message)
}

fn validate_reference(reference: &NewMemoryReference<'_>) -> Result<(), MemoryDatabaseError> {
    require_non_empty("id", reference.id)?;
    require_non_empty("fact_id", reference.fact_id)?;
    require_non_empty("value", reference.value)?;
    require_non_empty("normalized_value", reference.normalized_value)?;
    validate_json("metadata_json", reference.metadata_json)
}

fn validate_dream_change_status_payload(
    status: MemoryDreamChangeStatus,
    error_message: Option<&str>,
) -> Result<(), MemoryDatabaseError> {
    if let Some(error_message) = error_message {
        require_non_empty("error_message", error_message)?;
    }

    match status {
        MemoryDreamChangeStatus::Failed => {
            if error_message.is_none() {
                return Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: "failed memory Dream change requires error_message".to_string(),
                });
            }
        }
        MemoryDreamChangeStatus::Proposed | MemoryDreamChangeStatus::Applied => {
            if error_message.is_some() {
                return Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: format!(
                        "{} memory Dream change must not include error",
                        status.as_str()
                    ),
                });
            }
        }
        MemoryDreamChangeStatus::Skipped => {}
    }

    Ok(())
}

fn validate_dream_risk_level(value: &str) -> Result<(), MemoryDatabaseError> {
    match value {
        "low" | "medium" | "high" => Ok(()),
        _ => Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("unknown memory Dream risk level: {value}"),
        }),
    }
}

fn validate_scope_chat_id(
    scope: MemoryScope,
    chat_id: Option<&str>,
) -> Result<(), MemoryDatabaseError> {
    match (scope, chat_id) {
        (MemoryScope::Chat, Some(chat_id)) => require_non_empty("chat_id", chat_id),
        (MemoryScope::Chat, None) => Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "chat memory requires chat_id".to_string(),
        }),
        (MemoryScope::Global | MemoryScope::Workspace, Some(_)) => {
            Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!("{} memory must not include chat_id", scope.as_str()),
            })
        }
        (MemoryScope::Global | MemoryScope::Workspace, None) => Ok(()),
    }
}

/// Shared scope predicate for list/count so EXPLAIN tests stay production-homologous.
fn memory_facts_scope_filter_sql(
    kind: MemoryDatabaseKind,
    chat_id: Option<&str>,
) -> (&'static str, Option<&str>) {
    match kind {
        MemoryDatabaseKind::Global => ("scope = 'global'", None),
        MemoryDatabaseKind::Workspace if chat_id.is_some() => (
            "(scope = 'chat' AND chat_id = ?1) OR scope = 'workspace'",
            chat_id,
        ),
        MemoryDatabaseKind::Workspace => ("scope = 'workspace'", None),
    }
}

/// Shared list SQL (parameterized) for production + query-plan regression tests.
fn memory_facts_list_page_sql(filter_sql: &str, enabled_only: bool) -> String {
    let enabled_filter_sql = if enabled_only { "AND enabled = 1" } else { "" };
    format!(
        "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, enabled, is_latest,
                expires_at, metadata_json, created_at, updated_at
         FROM memory_facts
         WHERE ({filter_sql})
           AND status = ?3
           {enabled_filter_sql}
           AND (?4 IS NULL OR kind = ?4)
           AND (?5 IS NULL OR lower(fact) LIKE ?5 ESCAPE '\\')
           AND is_latest = 1
         ORDER BY
           CASE WHEN scope = 'chat' THEN 0 WHEN scope = 'workspace' THEN 1 ELSE 2 END,
           pinned DESC,
           updated_at DESC
         LIMIT ?2 OFFSET ?6"
    )
}

/// Shared count SQL (parameterized) for production + query-plan regression tests.
fn memory_facts_count_sql(filter_sql: &str) -> String {
    format!(
        "SELECT COUNT(*)
         FROM memory_facts
         WHERE ({filter_sql})
           AND status = ?2
           AND (?3 IS NULL OR kind = ?3)
           AND (?4 IS NULL OR lower(fact) LIKE ?4 ESCAPE '\\')
           AND is_latest = 1"
    )
}

fn require_non_empty(field: &str, value: &str) -> Result<(), MemoryDatabaseError> {
    if value.trim().is_empty() {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("{field} must not be empty"),
        });
    }

    Ok(())
}

fn escaped_memory_like_term(term: &str) -> String {
    let mut escaped = String::new();
    for character in term.chars() {
        match character {
            '\\' | '%' | '_' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

fn validate_json(field: &'static str, value: &str) -> Result<(), MemoryDatabaseError> {
    serde_json::from_str::<Value>(value)
        .map(|_| ())
        .map_err(|source| MemoryDatabaseError::InvalidMemoryJson { field, source })
}

fn validate_json_array(field: &'static str, value: &str) -> Result<(), MemoryDatabaseError> {
    let parsed: Value = serde_json::from_str(value)
        .map_err(|source| MemoryDatabaseError::InvalidMemoryJson { field, source })?;
    if !parsed.is_array() {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("{field} must be a JSON array"),
        });
    }

    Ok(())
}

fn redact_optional_memory_json(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, MemoryDatabaseError> {
    value
        .map(|json| redact_memory_json(json, field))
        .transpose()
}

fn redact_memory_json(value: &str, field: &'static str) -> Result<String, MemoryDatabaseError> {
    let mut parsed: Value = serde_json::from_str(value)
        .map_err(|source| MemoryDatabaseError::InvalidMemoryJson { field, source })?;

    redact_memory_json_value(&mut parsed);

    serde_json::to_string(&parsed)
        .map_err(|source| MemoryDatabaseError::InvalidMemoryJson { field, source })
}

fn redact_memory_json_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if is_secret_memory_key(key) {
                    *value = Value::String("[REDACTED]".to_string());
                } else {
                    redact_memory_json_value(value);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_memory_json_value(item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn is_secret_memory_key(key: &str) -> bool {
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

fn memory_fact_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryFactRecord> {
    Ok(MemoryFactRecord {
        id: row.get(0)?,
        scope: row.get(1)?,
        chat_id: row.get(2)?,
        status: row.get(3)?,
        kind: row.get(4)?,
        fact: row.get(5)?,
        confidence: row.get(6)?,
        pinned: row.get::<_, i64>(7)? != 0,
        enabled: row.get::<_, i64>(8)? != 0,
        is_latest: row.get::<_, i64>(9)? != 0,
        expires_at: row.get(10)?,
        metadata_json: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn memory_source_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemorySourceRecord> {
    Ok(MemorySourceRecord {
        id: row.get(0)?,
        scope: row.get(1)?,
        chat_id: row.get(2)?,
        source_type: row.get(3)?,
        source_id: row.get(4)?,
        title: row.get(5)?,
        content: row.get(6)?,
        metadata_json: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn memory_edge_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryEdgeRecord> {
    Ok(MemoryEdgeRecord {
        id: row.get(0)?,
        source_fact_id: row.get(1)?,
        target_fact_id: row.get(2)?,
        relation: row.get(3)?,
        metadata_json: row.get(4)?,
        created_at: row.get(5)?,
    })
}

fn memory_reference_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryReferenceRecord> {
    Ok(MemoryReferenceRecord {
        id: row.get(0)?,
        fact_id: row.get(1)?,
        reference_type: row.get(2)?,
        value: row.get(3)?,
        normalized_value: row.get(4)?,
        status: row.get(5)?,
        metadata_json: row.get(6)?,
        checked_at: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn memory_profile_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryProfileRecord> {
    Ok(MemoryProfileRecord {
        id: row.get(0)?,
        scope: row.get(1)?,
        chat_id: row.get(2)?,
        profile_text: row.get(3)?,
        metadata_json: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn memory_extraction_job_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<MemoryExtractionJobRecord> {
    Ok(MemoryExtractionJobRecord {
        id: row.get(0)?,
        scope: row.get(1)?,
        chat_id: row.get(2)?,
        status: row.get(3)?,
        model_id: row.get(4)?,
        input_json: row.get(5)?,
        output_json: row.get(6)?,
        error_message: row.get(7)?,
        created_at: row.get(8)?,
        started_at: row.get(9)?,
        completed_at: row.get(10)?,
    })
}

fn memory_dream_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryDreamJobRecord> {
    Ok(MemoryDreamJobRecord {
        id: row.get(0)?,
        scope: row.get(1)?,
        workspace_id: row.get(2)?,
        trigger_type: row.get(3)?,
        mode: row.get(4)?,
        status: row.get(5)?,
        model_id: row.get(6)?,
        input_summary_json: row.get(7)?,
        output_summary_json: row.get(8)?,
        transcript_chat_id: row.get(9)?,
        error_message: row.get(10)?,
        created_at: row.get(11)?,
        started_at: row.get(12)?,
        completed_at: row.get(13)?,
    })
}

fn memory_dream_change_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<MemoryDreamChangeRecord> {
    Ok(MemoryDreamChangeRecord {
        id: row.get(0)?,
        job_id: row.get(1)?,
        operation: row.get(2)?,
        target_fact_ids_json: row.get(3)?,
        new_fact_id: row.get(4)?,
        before_json: row.get(5)?,
        after_json: row.get(6)?,
        reason: row.get(7)?,
        confidence: row.get(8)?,
        risk_level: row.get(9)?,
        status: row.get(10)?,
        evidence_json: row.get(11)?,
        error_message: row.get(12)?,
        created_at: row.get(13)?,
        applied_at: row.get(14)?,
    })
}

fn fact_by_id(
    transaction: &Transaction<'_>,
    database_path: &Path,
    id: &str,
) -> Result<MemoryFactRecord, MemoryDatabaseError> {
    transaction
        .query_row(
            "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, enabled, is_latest,
                    expires_at, metadata_json, created_at, updated_at
             FROM memory_facts
             WHERE id = ?1",
            params![id],
            memory_fact_from_row,
        )
        .map_err(|source| sqlite_error(database_path, source))
}

fn source_ids_for_fact(
    transaction: &Transaction<'_>,
    database_path: &Path,
    fact_id: &str,
) -> Result<Vec<String>, MemoryDatabaseError> {
    let mut statement = transaction
        .prepare(
            "SELECT source_id
             FROM memory_fact_sources
             WHERE fact_id = ?1
             ORDER BY source_id ASC",
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    let rows = statement
        .query_map(params![fact_id], |row| row.get(0))
        .map_err(|source| sqlite_error(database_path, source))?;

    collect_rows(rows, database_path)
}

fn delete_unlinked_sources(
    transaction: &Transaction<'_>,
    database_path: &Path,
    source_ids: &[String],
) -> Result<(), MemoryDatabaseError> {
    for source_id in source_ids {
        transaction
            .execute(
                "DELETE FROM memory_sources
                 WHERE id = ?1
                   AND NOT EXISTS (
                       SELECT 1
                       FROM memory_fact_sources
                       WHERE source_id = ?1
                   )",
                params![source_id],
            )
            .map_err(|source| sqlite_error(database_path, source))?;
    }

    Ok(())
}

fn source_count_for_fact(
    connection: &Connection,
    database_path: &Path,
    fact_id: &str,
) -> Result<i64, MemoryDatabaseError> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM memory_fact_sources WHERE fact_id = ?1",
            params![fact_id],
            |row| row.get(0),
        )
        .map_err(|source| sqlite_error(database_path, source))
}

fn related_fact_ids(
    connection: &Connection,
    database_path: &Path,
    fact_id: &str,
) -> Result<Vec<String>, MemoryDatabaseError> {
    let mut statement = connection
        .prepare(
            "SELECT CASE
                        WHEN source_fact_id = ?1 THEN target_fact_id
                        ELSE source_fact_id
                    END AS related_fact_id
             FROM memory_edges
             WHERE source_fact_id = ?1 OR target_fact_id = ?1
             ORDER BY relation ASC, related_fact_id ASC",
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    let rows = statement
        .query_map(params![fact_id], |row| row.get(0))
        .map_err(|source| sqlite_error(database_path, source))?;

    collect_rows(rows, database_path)
}

fn derives_edge_metadata(
    transaction: &Transaction<'_>,
    database_path: &Path,
    source_fact_id: &str,
    target_fact_id: &str,
    metadata_json: &str,
) -> Result<String, MemoryDatabaseError> {
    require_fact_exists(transaction, database_path, source_fact_id)?;
    require_fact_exists(transaction, database_path, target_fact_id)?;
    let source_source_ids = source_ids_for_fact(transaction, database_path, source_fact_id)?;
    let target_source_ids = source_ids_for_fact(transaction, database_path, target_fact_id)?;

    if source_source_ids.is_empty() && target_source_ids.is_empty() {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "derives relation requires source or target evidence".to_string(),
        });
    }

    let parsed: Value = serde_json::from_str(metadata_json).map_err(|source| {
        MemoryDatabaseError::InvalidMemoryJson {
            field: "metadata_json",
            source,
        }
    })?;
    let mut metadata = match parsed {
        Value::Object(object) => object,
        other => {
            let mut object = serde_json::Map::new();
            object.insert("metadata".to_string(), other);
            object
        }
    };
    metadata.insert(
        "sourceFactId".to_string(),
        Value::String(source_fact_id.to_string()),
    );
    metadata.insert(
        "targetFactId".to_string(),
        Value::String(target_fact_id.to_string()),
    );
    metadata.insert("sourceSourceIds".to_string(), json!(source_source_ids));
    metadata.insert("targetSourceIds".to_string(), json!(target_source_ids));

    serde_json::to_string(&Value::Object(metadata)).map_err(|source| {
        MemoryDatabaseError::InvalidMemoryJson {
            field: "metadata_json",
            source,
        }
    })
}

fn require_fact_exists(
    transaction: &Transaction<'_>,
    database_path: &Path,
    fact_id: &str,
) -> Result<(), MemoryDatabaseError> {
    let exists: Option<i64> = transaction
        .query_row(
            "SELECT 1 FROM memory_facts WHERE id = ?1",
            params![fact_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| sqlite_error(database_path, source))?;

    if exists.is_none() {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("memory fact was not found: {fact_id}"),
        });
    }

    Ok(())
}

fn due_unexpired_fact_ids(
    transaction: &Transaction<'_>,
    database_path: &Path,
    now: &str,
) -> Result<Vec<String>, MemoryDatabaseError> {
    let mut statement = transaction
        .prepare(
            "SELECT id
             FROM memory_facts
             WHERE status IN ('active', 'pending')
               AND expires_at IS NOT NULL
               AND expires_at <= ?1
             ORDER BY id ASC",
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    let rows = statement
        .query_map(params![now], |row| row.get(0))
        .map_err(|source| sqlite_error(database_path, source))?;

    collect_rows(rows, database_path)
}

fn profile_id_for_scope(scope: MemoryScope, chat_id: Option<&str>) -> String {
    match scope {
        MemoryScope::Global => "memory-profile:global".to_string(),
        MemoryScope::Workspace => "memory-profile:workspace".to_string(),
        MemoryScope::Chat => format!(
            "memory-profile:chat:{}",
            chat_id.expect("chat profile id requires chat id")
        ),
    }
}

fn memory_profile_fact_line(fact: &MemoryFactRecord) -> String {
    let pinned = if fact.pinned { " pinned" } else { "" };
    format!(
        "- {}{}: {}",
        fact.kind,
        pinned,
        fact.fact.split_whitespace().collect::<Vec<_>>().join(" ")
    )
}

fn update_relation_would_cycle(
    connection: &Connection,
    database_path: &Path,
    source_fact_id: &str,
    target_fact_id: &str,
) -> Result<bool, MemoryDatabaseError> {
    let found: Option<i64> = connection
        .query_row(
            "WITH RECURSIVE update_chain(fact_id) AS (
                SELECT target_fact_id
                FROM memory_edges
                WHERE source_fact_id = ?1 AND relation = 'updates'
                UNION
                SELECT e.target_fact_id
                FROM memory_edges e
                JOIN update_chain c ON e.source_fact_id = c.fact_id
                WHERE e.relation = 'updates'
             )
             SELECT 1
             FROM update_chain
             WHERE fact_id = ?2
             LIMIT 1",
            params![target_fact_id, source_fact_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| sqlite_error(database_path, source))?;

    Ok(found.is_some())
}

fn inherit_update_relation_enabled_state(
    transaction: &Transaction<'_>,
    database_path: &Path,
    source_fact_id: &str,
    target_fact_id: &str,
    now: &str,
) -> Result<(), MemoryDatabaseError> {
    let target_enabled: i64 = transaction
        .query_row(
            "SELECT enabled FROM memory_facts WHERE id = ?1",
            params![target_fact_id],
            |row| row.get(0),
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    transaction
        .execute(
            "UPDATE memory_facts
             SET enabled = ?2,
                 updated_at = ?3
             WHERE id = ?1",
            params![source_fact_id, target_enabled, now],
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    Ok(())
}

fn apply_update_relation_effects(
    transaction: &Transaction<'_>,
    database_path: &Path,
    source_fact_id: &str,
    now: &str,
) -> Result<(), MemoryDatabaseError> {
    let source_status: String = transaction
        .query_row(
            "SELECT status FROM memory_facts WHERE id = ?1",
            params![source_fact_id],
            |row| row.get(0),
        )
        .map_err(|source| sqlite_error(database_path, source))?;

    if source_status != MemoryStatus::Active.as_str() {
        return Ok(());
    }

    let target_ids = update_relation_target_chain(transaction, database_path, source_fact_id)?;
    for target_id in target_ids {
        transaction
            .execute(
                "UPDATE memory_facts
                 SET is_latest = 0,
                     status = CASE
                         WHEN status IN ('active', 'pending') THEN 'superseded'
                         ELSE status
                     END,
                     updated_at = ?2
                 WHERE id = ?1",
                params![target_id, now],
            )
            .map_err(|source| sqlite_error(database_path, source))?;
        let updated_fact = fact_by_id(transaction, database_path, &target_id)?;
        upsert_fact_record_fts_data(transaction, database_path, &updated_fact)?;
    }

    Ok(())
}

fn update_relation_target_chain(
    transaction: &Transaction<'_>,
    database_path: &Path,
    source_fact_id: &str,
) -> Result<Vec<String>, MemoryDatabaseError> {
    let mut statement = transaction
        .prepare(
            "WITH RECURSIVE update_chain(fact_id) AS (
                SELECT target_fact_id
                FROM memory_edges
                WHERE source_fact_id = ?1 AND relation = 'updates'
                UNION
                SELECT e.target_fact_id
                FROM memory_edges e
                JOIN update_chain c ON e.source_fact_id = c.fact_id
                WHERE e.relation = 'updates'
             )
             SELECT fact_id
             FROM update_chain
             ORDER BY fact_id ASC",
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    let rows = statement
        .query_map(params![source_fact_id], |row| row.get(0))
        .map_err(|source| sqlite_error(database_path, source))?;

    collect_rows(rows, database_path)
}

fn ensure_memory_schema_exists(
    connection: &Connection,
    database_path: &Path,
) -> Result<(), MemoryDatabaseError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS (
                SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'memory_facts'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|source| sqlite_error(database_path, source))?;

    if !exists {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!(
                "{} does not contain the memory schema; run workspace migrations first",
                database_path.display()
            ),
        });
    }

    Ok(())
}

fn memory_status_from_str(value: &str) -> Result<MemoryStatus, MemoryDatabaseError> {
    match value {
        "pending" => Ok(MemoryStatus::Pending),
        "active" => Ok(MemoryStatus::Active),
        "superseded" => Ok(MemoryStatus::Superseded),
        "expired" => Ok(MemoryStatus::Expired),
        "rejected" => Ok(MemoryStatus::Rejected),
        _ => Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("unknown memory status: {value}"),
        }),
    }
}

fn memory_kind_from_str(value: &str) -> Result<MemoryKind, MemoryDatabaseError> {
    match value {
        "preference" => Ok(MemoryKind::Preference),
        "project_fact" => Ok(MemoryKind::ProjectFact),
        "project_decision" => Ok(MemoryKind::ProjectDecision),
        "procedure" => Ok(MemoryKind::Procedure),
        "constraint" => Ok(MemoryKind::Constraint),
        "episode" => Ok(MemoryKind::Episode),
        "user_note" => Ok(MemoryKind::UserNote),
        _ => Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("unknown memory kind: {value}"),
        }),
    }
}

fn memory_source_type_from_str(value: &str) -> Result<MemorySourceType, MemoryDatabaseError> {
    match value {
        "chat_message" => Ok(MemorySourceType::ChatMessage),
        "assistant_message" => Ok(MemorySourceType::AssistantMessage),
        "tool_call" => Ok(MemorySourceType::ToolCall),
        "tool_result" => Ok(MemorySourceType::ToolResult),
        "context_snapshot" => Ok(MemorySourceType::ContextSnapshot),
        "manual_note" => Ok(MemorySourceType::ManualNote),
        "imported_document" => Ok(MemorySourceType::ImportedDocument),
        _ => Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("unknown memory source type: {value}"),
        }),
    }
}

fn memory_reference_type_from_str(value: &str) -> Result<MemoryReferenceType, MemoryDatabaseError> {
    match value {
        "file_path" => Ok(MemoryReferenceType::FilePath),
        "symbol" => Ok(MemoryReferenceType::Symbol),
        "command" => Ok(MemoryReferenceType::Command),
        "url" => Ok(MemoryReferenceType::Url),
        "workspace_id" => Ok(MemoryReferenceType::WorkspaceId),
        _ => Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("unknown memory reference type: {value}"),
        }),
    }
}

fn memory_reference_status_from_str(
    value: &str,
) -> Result<MemoryReferenceStatus, MemoryDatabaseError> {
    match value {
        "valid" => Ok(MemoryReferenceStatus::Valid),
        "invalid" => Ok(MemoryReferenceStatus::Invalid),
        "ambiguous" => Ok(MemoryReferenceStatus::Ambiguous),
        "skipped" => Ok(MemoryReferenceStatus::Skipped),
        _ => Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("unknown memory reference status: {value}"),
        }),
    }
}

fn promoted_source_id(promoted_fact_id: &str, index: usize) -> String {
    format!("{promoted_fact_id}:source:{index}")
}

fn promoted_source_ids(promoted_fact_id: &str, count: usize) -> Vec<String> {
    (0..count)
        .map(|index| promoted_source_id(promoted_fact_id, index))
        .collect()
}

/// Whether an existing target source row can be reused by an idempotent copy
/// retry. The copied source fields are deterministic from the source fact, so
/// a matching row must have come from the same (partial) copy attempt.
fn copied_source_matches(existing: &MemorySourceRecord, source: &MemorySourceRecord) -> bool {
    existing.source_type == source.source_type
        && existing.source_id == source.source_id
        && existing.title == source.title
        && existing.content == source.content
        && existing.metadata_json == source.metadata_json
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
    database_path: &Path,
) -> Result<Vec<T>, MemoryDatabaseError> {
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|source| sqlite_error(database_path, source))
}

fn create_directory(path: &Path) -> Result<(), MemoryDatabaseError> {
    create_private_dir_all(path).map_err(|source| MemoryDatabaseError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sqlite_error(database_path: &Path, source: rusqlite::Error) -> MemoryDatabaseError {
    MemoryDatabaseError::Sqlite {
        path: database_path.to_path_buf(),
        source,
    }
}

fn is_sqlite_unique_constraint_error(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation
                || matches!(
                    code.extended_code,
                    rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE | rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
                )
    ) || error
        .to_string()
        .to_ascii_lowercase()
        .contains("unique constraint failed")
}

/// Only the active-job partial UNIQUE counts as already-active singleflight.
/// PK / other UNIQUE conflicts must surface as Sqlite errors.
fn is_active_dream_singleflight_conflict(error: &rusqlite::Error) -> bool {
    if !is_sqlite_unique_constraint_error(error) {
        return false;
    }
    let message = error.to_string().to_ascii_lowercase();
    // SQLite may report the index name or the constrained column expression.
    message.contains("memory_dream_jobs_active_singleflight_idx")
        || message.contains("memory_dream_jobs.scope")
}

fn now_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn bool_to_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{WorkspaceDatabase, workspace_database_path};
    use rusqlite::{Connection, params};

    #[test]
    fn kind_scope_policy_rejects_global_project_memories() {
        for kind in [MemoryKind::ProjectFact, MemoryKind::ProjectDecision] {
            assert!(!memory_scope_allows_kind(MemoryScope::Global, kind));
            assert!(matches!(
                ensure_memory_kind_scope_allowed(MemoryScope::Global, kind),
                Err(MemoryDatabaseError::InvalidMemoryInput { .. })
            ));
        }
        for kind in [
            MemoryKind::Preference,
            MemoryKind::Procedure,
            MemoryKind::Constraint,
            MemoryKind::Episode,
            MemoryKind::UserNote,
        ] {
            assert!(memory_scope_allows_kind(MemoryScope::Global, kind));
            assert!(ensure_memory_kind_scope_allowed(MemoryScope::Global, kind).is_ok());
        }
        for kind in [
            MemoryKind::Preference,
            MemoryKind::ProjectFact,
            MemoryKind::ProjectDecision,
            MemoryKind::Procedure,
            MemoryKind::Constraint,
            MemoryKind::Episode,
            MemoryKind::UserNote,
        ] {
            assert!(memory_scope_allows_kind(MemoryScope::Workspace, kind));
            assert!(memory_scope_allows_kind(MemoryScope::Chat, kind));
            assert!(ensure_memory_kind_scope_allowed(MemoryScope::Workspace, kind).is_ok());
            assert!(ensure_memory_kind_scope_allowed(MemoryScope::Chat, kind).is_ok());
        }
    }

    #[test]
    fn insert_fact_rejects_global_project_memories_but_keeps_historical_readable() {
        let profile = tempfile::tempdir().expect("profile");
        let mut database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");

        database
            .insert_source(NewMemorySource {
                id: "source-project-global",
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Test source",
                content: "source content",
                metadata_json: "{}",
            })
            .expect("source insert");
        let error = database
            .insert_fact(NewMemoryFact {
                id: "fact-project-global",
                scope: MemoryScope::Global,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::ProjectFact,
                fact: "Project fact must not be global.",
                confidence: None,
                pinned: false,
                source_ids: &["source-project-global"],
                metadata_json: "{}",
            })
            .expect_err("global project fact insert must fail");
        assert!(matches!(
            error,
            MemoryDatabaseError::InvalidMemoryInput { .. }
        ));

        // Global preference and workspace project-class rows remain writable.
        database
            .insert_fact(NewMemoryFact {
                id: "fact-preference-global",
                scope: MemoryScope::Global,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::Preference,
                fact: "User-wide preference.",
                confidence: None,
                pinned: false,
                source_ids: &["source-project-global"],
                metadata_json: "{}",
            })
            .expect("global preference insert");

        let workspace_dir = profile.path().join("workspace");
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "kind scope policy")
                .expect("chat insert");
        }
        let mut workspace =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
                .expect("workspace memory database");
        workspace
            .insert_source(NewMemorySource {
                id: "source-project-workspace",
                scope: MemoryScope::Workspace,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Test source",
                content: "source content",
                metadata_json: "{}",
            })
            .expect("workspace source insert");
        workspace
            .insert_fact(NewMemoryFact {
                id: "fact-project-workspace",
                scope: MemoryScope::Workspace,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::ProjectDecision,
                fact: "Project decision belongs to a workspace.",
                confidence: None,
                pinned: false,
                source_ids: &["source-project-workspace"],
                metadata_json: "{}",
            })
            .expect("workspace project decision insert");

        // A historical global project-class row (written before the policy)
        // remains readable and editable in place without changing kind/scope.
        drop(database);
        let connection = Connection::open(global_memory_database_path(profile.path()))
            .expect("open global memory database");
        connection
            .execute_batch(
                "INSERT INTO memory_facts
                    (id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest,
                     expires_at, metadata_json, created_at, updated_at)
                 VALUES
                    ('fact-historical-project', 'global', NULL, 'active', 'project_fact',
                     'Historical global project fact.', 0.8, 0, 1, NULL, '{}',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');",
            )
            .expect("insert historical global project fact");
        drop(connection);
        let mut database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("reopen global memory");
        let historical = database
            .fact("fact-historical-project")
            .expect("historical fact query")
            .expect("historical fact");
        assert_eq!(historical.kind, "project_fact");
        assert_eq!(historical.scope, "global");
        database
            .update_fact(UpdateMemoryFact {
                id: "fact-historical-project",
                fact: Some("Historical global project fact (edited in place)."),
                ..UpdateMemoryFact::default()
            })
            .expect("historical global project fact stays editable without kind/scope change");
        let error = database
            .update_fact(UpdateMemoryFact {
                id: "fact-historical-project",
                kind: Some(MemoryKind::ProjectDecision),
                ..UpdateMemoryFact::default()
            })
            .expect_err("changing a global fact kind to project must fail");
        assert!(matches!(
            error,
            MemoryDatabaseError::InvalidMemoryInput { .. }
        ));
        let promoted_error = database
            .promote_fact(
                "fact-historical-project",
                "fact-promoted",
                MemoryScope::Global,
                None,
            )
            .expect_err("promoting a global project fact within global scope must fail");
        assert!(matches!(
            promoted_error,
            MemoryDatabaseError::InvalidMemoryInput { .. }
        ));
    }

    #[test]
    fn promote_rejects_workspace_project_memories_to_global() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace_dir = profile.path().join("workspace");
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "promote policy")
                .expect("chat insert");
        }
        let mut workspace =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
                .expect("workspace memory database");
        workspace
            .insert_source(NewMemorySource {
                id: "source-project",
                scope: MemoryScope::Workspace,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Test source",
                content: "source content",
                metadata_json: "{}",
            })
            .expect("workspace source insert");
        workspace
            .insert_fact(NewMemoryFact {
                id: "fact-project",
                scope: MemoryScope::Workspace,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::ProjectFact,
                fact: "Workspace project fact.",
                confidence: None,
                pinned: false,
                source_ids: &["source-project"],
                metadata_json: "{}",
            })
            .expect("workspace project fact insert");

        let mut global =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        let error = workspace
            .promote_fact_to_database(
                "fact-project",
                &mut global,
                "fact-promoted-global",
                MemoryScope::Global,
                None,
            )
            .expect_err("promoting a workspace project fact to global must fail");
        assert!(matches!(
            error,
            MemoryDatabaseError::InvalidMemoryInput { .. }
        ));
        assert!(
            global
                .fact("fact-promoted-global")
                .expect("query")
                .is_none()
        );

        // Moving a workspace project fact to a workspace target stays allowed.
        let mut target_workspace =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
                .expect("target workspace memory database");
        target_workspace
            .promote_fact(
                "fact-project",
                "fact-project-promoted",
                MemoryScope::Workspace,
                None,
            )
            .expect("workspace to workspace promote stays allowed");
        assert!(
            target_workspace
                .fact("fact-project-promoted")
                .expect("query")
                .is_some()
        );
    }

    #[test]
    fn move_copy_is_idempotent_and_mark_fact_moved_supersedes_source() {
        let profile = tempfile::tempdir().expect("profile");
        let mut global =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        global
            .insert_source(NewMemorySource {
                id: "source-move",
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Move source",
                content: "Historical global project fact content",
                metadata_json: "{}",
            })
            .expect("source insert");
        // Historical global project-class row (predates the kind/scope policy).
        drop(global);
        let connection = Connection::open(global_memory_database_path(profile.path()))
            .expect("open global memory database");
        connection
            .execute_batch(
                "INSERT INTO memory_facts
                    (id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest,
                     expires_at, metadata_json, created_at, updated_at)
                 VALUES
                    ('fact-move', 'global', NULL, 'active', 'project_fact',
                     'Historical global project fact.', 0.8, 1, 1, NULL,
                     '{\"origin\":\"legacy\"}',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                 INSERT INTO memory_fact_sources (fact_id, source_id)
                 VALUES ('fact-move', 'source-move');",
            )
            .expect("insert historical global project fact");
        drop(connection);

        let workspace_dir = profile.path().join("workspace");
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "move target")
                .expect("chat insert");
        }
        let mut global =
            MemoryDatabase::open_or_create_global(profile.path()).expect("reopen global memory");
        let mut target = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("target workspace memory database");

        let outcome = global
            .copy_fact_to_database_idempotent(
                &mut target,
                "fact-move",
                MemoryScope::Workspace,
                None,
            )
            .expect("copy to workspace");
        assert!(!outcome.target_pre_existed);
        assert_eq!(outcome.target_fact.id, "fact-move");
        assert_eq!(outcome.target_fact.scope, "workspace");
        assert_eq!(outcome.target_fact.kind, "project_fact");
        assert_eq!(outcome.target_fact.status, "active");
        assert_eq!(outcome.target_fact.confidence, Some(0.8));
        assert!(outcome.target_fact.pinned);
        assert!(outcome.target_fact.enabled);
        assert_eq!(
            serde_json::from_str::<Value>(&outcome.target_fact.metadata_json)
                .expect("metadata json")["origin"],
            "legacy"
        );
        let target_sources = target
            .sources_for_fact("fact-move")
            .expect("target sources");
        assert_eq!(target_sources.len(), 1);
        assert_eq!(
            target_sources[0].id, "fact-move:source:0",
            "sources are copied with ids namespaced under the target fact"
        );
        assert_eq!(
            target_sources[0].content,
            "Historical global project fact content"
        );

        // Source is still active after the copy; only mark_fact_moved finalizes.
        let source_before = global.fact("fact-move").expect("query").expect("source");
        assert_eq!(source_before.status, "active");

        // Idempotent retry never creates a second target copy.
        let retry = global
            .copy_fact_to_database_idempotent(
                &mut target,
                "fact-move",
                MemoryScope::Workspace,
                None,
            )
            .expect("idempotent copy retry");
        assert!(retry.target_pre_existed);
        assert_eq!(
            target
                .list_facts_for_scope_page(None, MemoryStatus::Active, None, None, 10, 0)
                .expect("target active list")
                .len(),
            1
        );

        let moved = global
            .mark_fact_moved("fact-move", "workspace-1", "fact-move")
            .expect("mark moved");
        assert_eq!(moved.status, "superseded");
        let metadata: Value =
            serde_json::from_str(&moved.metadata_json).expect("moved metadata json");
        assert_eq!(metadata["movedToWorkspace"], "workspace-1");
        assert_eq!(metadata["movedTargetFactId"], "fact-move");
        assert!(metadata.get("movedAt").is_some());
        assert_eq!(
            metadata["origin"], "legacy",
            "existing metadata is preserved"
        );

        // The source no longer appears as active global memory.
        assert!(global.fact("fact-move").expect("query").is_some());
        assert_eq!(
            global
                .list_facts_for_scope_page(None, MemoryStatus::Active, None, None, 10, 0)
                .expect("global active list")
                .len(),
            0
        );
        // mark_fact_moved is itself idempotent.
        let moved_again = global
            .mark_fact_moved("fact-move", "workspace-1", "fact-move")
            .expect("idempotent mark moved");
        assert_eq!(moved_again.status, "superseded");
    }

    #[test]
    fn copy_fact_to_database_idempotent_rejects_colliding_target() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace_dir = profile.path().join("workspace");
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "move collision")
                .expect("chat insert");
        }
        let mut global =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        global
            .insert_source(NewMemorySource {
                id: "source-collision",
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Collision source",
                content: "Global preference",
                metadata_json: "{}",
            })
            .expect("source insert");
        global
            .insert_fact(NewMemoryFact {
                id: "fact-collision",
                scope: MemoryScope::Global,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::Preference,
                fact: "Global preference",
                confidence: None,
                pinned: false,
                source_ids: &["source-collision"],
                metadata_json: "{}",
            })
            .expect("fact insert");
        let mut target = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("target workspace memory database");
        target
            .insert_source(NewMemorySource {
                id: "source-target-collision",
                scope: MemoryScope::Workspace,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Existing target source",
                content: "Different target content",
                metadata_json: "{}",
            })
            .expect("target source insert");
        target
            .insert_fact(NewMemoryFact {
                id: "fact-collision",
                scope: MemoryScope::Workspace,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::Preference,
                fact: "Different target content",
                confidence: None,
                pinned: false,
                source_ids: &["source-target-collision"],
                metadata_json: "{}",
            })
            .expect("target fact insert");

        let error = global
            .copy_fact_to_database_idempotent(
                &mut target,
                "fact-collision",
                MemoryScope::Workspace,
                None,
            )
            .expect_err("colliding target id must fail");
        assert!(
            error
                .to_string()
                .contains("already exists with different content")
        );
        // Source stays untouched.
        assert_eq!(
            global
                .fact("fact-collision")
                .expect("query")
                .expect("source")
                .status,
            "active"
        );
    }

    #[test]
    fn copy_target_write_failure_keeps_source_active() {
        let profile = tempfile::tempdir().expect("profile");
        let mut global =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        global
            .insert_source(NewMemorySource {
                id: "source-target-failure",
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Source",
                content: "Historical project fact",
                metadata_json: "{}",
            })
            .expect("source insert");
        drop(global);
        let connection = Connection::open(global_memory_database_path(profile.path()))
            .expect("open global memory database");
        connection
            .execute_batch(
                "INSERT INTO memory_facts
                    (id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest,
                     expires_at, metadata_json, created_at, updated_at)
                 VALUES
                    ('fact-target-failure', 'global', NULL, 'active', 'project_decision',
                     'Historical global project decision.', NULL, 0, 1, NULL, '{}',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                 INSERT INTO memory_fact_sources (fact_id, source_id)
                 VALUES ('fact-target-failure', 'source-target-failure');",
            )
            .expect("insert historical global project decision");
        drop(connection);

        let workspace_dir = profile.path().join("workspace");
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "move target failure")
                .expect("chat insert");
        }
        let global =
            MemoryDatabase::open_or_create_global(profile.path()).expect("reopen global memory");
        let mut target = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("target workspace memory database");
        // A pre-existing source with the id the copy would use makes the target
        // write fail before any fact row is inserted.
        target
            .insert_source(NewMemorySource {
                id: "fact-target-failure:source:0",
                scope: MemoryScope::Workspace,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Occupied source id",
                content: "Occupied",
                metadata_json: "{}",
            })
            .expect("pre-insert colliding target source");

        global
            .copy_fact_to_database_idempotent(
                &mut target,
                "fact-target-failure",
                MemoryScope::Workspace,
                None,
            )
            .expect_err("target write must fail");

        let source = global
            .fact("fact-target-failure")
            .expect("query")
            .expect("source");
        assert_eq!(
            source.status, "active",
            "failed target write must not touch source"
        );
        assert!(target.fact("fact-target-failure").expect("query").is_none());
    }

    #[test]
    fn write_fact_copy_idempotent_payload_path_is_idempotent_and_rejects_collisions() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace_dir = profile.path().join("workspace");
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "move payload")
                .expect("chat insert");
        }
        let mut target = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("target workspace memory database");

        // Serialized payload as the sidecar receives it from the main process.
        let fact = MemoryFactRecord {
            id: "fact-payload".to_string(),
            scope: "global".to_string(),
            chat_id: None,
            status: "active".to_string(),
            kind: "project_fact".to_string(),
            fact: "Historical global project fact (payload).".to_string(),
            confidence: Some(0.7),
            pinned: true,
            enabled: false,
            is_latest: true,
            expires_at: None,
            metadata_json: "{\"origin\":\"legacy\"}".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let sources = vec![MemorySourceRecord {
            id: "source-payload".to_string(),
            scope: "global".to_string(),
            chat_id: None,
            source_type: "manual_note".to_string(),
            source_id: None,
            title: "Payload source".to_string(),
            content: "Historical source content".to_string(),
            metadata_json: "{}".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }];

        let outcome = target
            .write_fact_copy_idempotent(
                "fact-payload",
                MemoryScope::Workspace,
                None,
                &fact,
                &sources,
            )
            .expect("payload write");
        assert!(!outcome.target_pre_existed);
        assert_eq!(outcome.target_fact.kind, "project_fact");
        assert_eq!(outcome.target_fact.scope, "workspace");
        assert_eq!(outcome.target_fact.status, "active");
        assert_eq!(outcome.target_fact.confidence, Some(0.7));
        assert!(outcome.target_fact.pinned);
        assert!(!outcome.target_fact.enabled, "disabled flag is preserved");
        assert_eq!(
            serde_json::from_str::<Value>(&outcome.target_fact.metadata_json)
                .expect("metadata json")["origin"],
            "legacy"
        );
        assert_eq!(
            target
                .sources_for_fact("fact-payload")
                .expect("target sources")[0]
                .content,
            "Historical source content"
        );

        // A retry after a lost response is idempotent and never duplicates.
        let retry = target
            .write_fact_copy_idempotent(
                "fact-payload",
                MemoryScope::Workspace,
                None,
                &fact,
                &sources,
            )
            .expect("idempotent payload retry");
        assert!(retry.target_pre_existed);
        assert_eq!(
            target
                .list_facts_for_scope_page(None, MemoryStatus::Active, None, None, 10, 0)
                .expect("target active list")
                .len(),
            1
        );

        // A colliding id with different content fails without overwriting.
        let mut changed = fact.clone();
        changed.fact = "Different payload content".to_string();
        let error = target
            .write_fact_copy_idempotent(
                "fact-payload",
                MemoryScope::Workspace,
                None,
                &changed,
                &sources,
            )
            .expect_err("colliding payload must fail");
        assert!(
            error
                .to_string()
                .contains("already exists with different content")
        );
        assert_eq!(
            target
                .fact("fact-payload")
                .expect("query")
                .expect("target fact")
                .fact,
            "Historical global project fact (payload)."
        );
    }

    #[test]
    fn write_fact_copy_idempotent_recovers_partially_written_sources() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace_dir = profile.path().join("workspace");
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "move partial recovery")
                .expect("chat insert");
        }
        let mut target = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("target workspace memory database");
        let fact = MemoryFactRecord {
            id: "fact-partial".to_string(),
            scope: "global".to_string(),
            chat_id: None,
            status: "active".to_string(),
            kind: "project_fact".to_string(),
            fact: "Partially copied project fact.".to_string(),
            confidence: Some(0.6),
            pinned: false,
            enabled: false,
            is_latest: true,
            expires_at: None,
            metadata_json: "{\"origin\":\"legacy\"}".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let sources = vec![MemorySourceRecord {
            id: "source-partial".to_string(),
            scope: "global".to_string(),
            chat_id: None,
            source_type: "manual_note".to_string(),
            source_id: None,
            title: "Partial source".to_string(),
            content: "Partial source content".to_string(),
            metadata_json: "{}".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }];
        // Orphaned source rows from a failed first attempt that crashed after
        // writing sources but before writing the fact row.
        target
            .insert_source(NewMemorySource {
                id: "fact-partial:source:0",
                scope: MemoryScope::Workspace,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Partial source",
                content: "Partial source content",
                metadata_json: "{}",
            })
            .expect("orphan source insert");

        let outcome = target
            .write_fact_copy_idempotent(
                "fact-partial",
                MemoryScope::Workspace,
                None,
                &fact,
                &sources,
            )
            .expect("recovered copy");
        assert!(!outcome.target_pre_existed);
        assert!(!outcome.target_fact.enabled, "disabled state is preserved");
        assert_eq!(
            target
                .list_facts_for_scope_page(None, MemoryStatus::Active, None, None, 10, 0)
                .expect("target active list")
                .len(),
            1
        );
        assert_eq!(
            target
                .sources_for_fact("fact-partial")
                .expect("target sources")
                .len(),
            1,
            "orphan source rows are reused, not duplicated"
        );

        // A mismatching orphan source is a real collision and still fails
        // without overwriting the existing row.
        let mut mismatched = fact.clone();
        mismatched.id = "fact-partial-mismatch".to_string();
        mismatched.fact = "Mismatched copy.".to_string();
        target
            .insert_source(NewMemorySource {
                id: "fact-partial-mismatch:source:0",
                scope: MemoryScope::Workspace,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Occupied",
                content: "Different content",
                metadata_json: "{}",
            })
            .expect("mismatch source insert");
        let error = target
            .write_fact_copy_idempotent(
                "fact-partial-mismatch",
                MemoryScope::Workspace,
                None,
                &mismatched,
                &sources,
            )
            .expect_err("mismatched source id must fail");
        assert!(
            error
                .to_string()
                .contains("already exists with different content")
        );
        assert!(
            target
                .fact("fact-partial-mismatch")
                .expect("query")
                .is_none()
        );
    }

    #[test]
    fn mark_fact_moved_failure_after_target_copy_keeps_target_intact() {
        let profile = tempfile::tempdir().expect("profile");
        let mut global =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        global
            .insert_source(NewMemorySource {
                id: "source-cleanup-failure",
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Source",
                content: "Historical global project fact",
                metadata_json: "{}",
            })
            .expect("source insert");
        drop(global);
        let connection = Connection::open(global_memory_database_path(profile.path()))
            .expect("open global memory database");
        connection
            .execute_batch(
                "INSERT INTO memory_facts
                    (id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest,
                     expires_at, metadata_json, created_at, updated_at)
                 VALUES
                    ('fact-cleanup-failure', 'global', NULL, 'active', 'project_fact',
                     'Historical global project fact.', 0.8, 1, 1, NULL, '{}',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                 INSERT INTO memory_fact_sources (fact_id, source_id)
                 VALUES ('fact-cleanup-failure', 'source-cleanup-failure');",
            )
            .expect("insert historical global project fact");
        drop(connection);

        let workspace_dir = profile.path().join("workspace");
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "move cleanup failure")
                .expect("chat insert");
        }
        let mut global =
            MemoryDatabase::open_or_create_global(profile.path()).expect("reopen global memory");
        let mut target = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("target workspace memory database");

        // Target write succeeds first (the safe order).
        global
            .copy_fact_to_database_idempotent(
                &mut target,
                "fact-cleanup-failure",
                MemoryScope::Workspace,
                None,
            )
            .expect("copy to workspace");

        // Source cleanup fails: a concurrent forget hard-deletes the source
        // between the copy and the mark. The error surfaces the partial state
        // and the target copy remains fully intact.
        global
            .hard_delete_fact("fact-cleanup-failure")
            .expect("concurrent forget");
        let error = global
            .mark_fact_moved(
                "fact-cleanup-failure",
                "workspace-1",
                "fact-cleanup-failure",
            )
            .expect_err("missing source must fail the cleanup step");
        assert!(error.to_string().contains("was not found"));

        let target_fact = target
            .fact("fact-cleanup-failure")
            .expect("query")
            .expect("target fact survives failed cleanup");
        assert_eq!(target_fact.status, "active");
        assert_eq!(target_fact.kind, "project_fact");
        assert_eq!(
            target
                .list_facts_for_scope_page(None, MemoryStatus::Active, None, None, 10, 0)
                .expect("target active list")
                .len(),
            1,
            "no duplicate target copy"
        );
    }

    #[test]
    fn global_database_creates_memory_schema() {
        let profile = tempfile::tempdir().expect("profile");
        let database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");

        assert!(database.database_path().is_file());
        assert_eq!(
            database.schema_version().expect("schema version"),
            GLOBAL_MEMORY_SCHEMA_VERSION
        );
        let connection = Connection::open(database.database_path()).expect("open database");
        assert!(memory_table_exists(&connection, "memory_dream_jobs"));
        assert!(memory_table_exists(&connection, "memory_dream_changes"));
        assert!(memory_table_exists(&connection, "memory_references"));
        assert_eq!(
            memory_column_definition(&connection, "memory_facts", "enabled"),
            Some(("INTEGER".to_string(), true, Some("1".to_string())))
        );
    }

    #[test]
    fn global_database_migrates_v1_dream_schema() {
        let profile = tempfile::tempdir().expect("profile");
        let database_path = global_memory_database_path(profile.path());
        let connection = Connection::open(&database_path).expect("v1 memory database");
        connection
            .execute_batch(&format!(
                "{GLOBAL_MEMORY_SCHEMA_SQL}
                 PRAGMA user_version = 1;"
            ))
            .expect("v1 memory schema");
        drop(connection);

        let database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("migrated database");
        assert_eq!(
            database.schema_version().expect("schema version"),
            GLOBAL_MEMORY_SCHEMA_VERSION
        );
        let connection = Connection::open(database.database_path()).expect("open database");
        assert!(memory_table_exists(&connection, "memory_dream_jobs"));
        assert!(memory_table_exists(&connection, "memory_dream_changes"));
        assert!(memory_table_exists(&connection, "memory_references"));
    }

    #[test]
    fn global_database_migrates_v2_memory_references_schema() {
        let profile = tempfile::tempdir().expect("profile");
        let database_path = global_memory_database_path(profile.path());
        let connection = Connection::open(&database_path).expect("v2 memory database");
        connection
            .execute_batch(&format!(
                "{GLOBAL_MEMORY_SCHEMA_SQL}
                 {GLOBAL_MEMORY_DREAM_SCHEMA_SQL}
                 PRAGMA user_version = 2;"
            ))
            .expect("v2 memory schema");
        drop(connection);

        let database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("migrated database");
        assert_eq!(
            database.schema_version().expect("schema version"),
            GLOBAL_MEMORY_SCHEMA_VERSION
        );
        let connection = Connection::open(database.database_path()).expect("open database");
        assert!(memory_table_exists(&connection, "memory_references"));
    }

    #[test]
    fn global_database_migrates_existing_facts_as_enabled() {
        let profile = tempfile::tempdir().expect("profile");
        let database_path = global_memory_database_path(profile.path());
        let connection = Connection::open(&database_path).expect("v4 memory database");
        connection
            .execute_batch(&format!(
                "{GLOBAL_MEMORY_SCHEMA_SQL}
                 {GLOBAL_MEMORY_DREAM_SCHEMA_SQL}
                 {MEMORY_REFERENCES_SCHEMA_SQL}
                 {GLOBAL_MEMORY_EXTRACTION_SKIPPED_STATUS_MIGRATION_SQL}
                 INSERT INTO memory_facts
                    (id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest,
                     expires_at, metadata_json, created_at, updated_at)
                 VALUES
                    ('fact-old', 'global', NULL, 'active', 'preference', 'Use Rust.', 0.9,
                     0, 1, NULL, '{{}}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                 PRAGMA user_version = 4;"
            ))
            .expect("v4 memory schema");
        drop(connection);

        let database = MemoryDatabase::open_or_create_global(profile.path()).expect("migration");
        let fact = database
            .fact("fact-old")
            .expect("fact query")
            .expect("fact");
        assert!(fact.enabled);
        drop(database);

        let reopened = MemoryDatabase::open_or_create_global(profile.path()).expect("reopen");
        assert!(
            reopened
                .fact("fact-old")
                .expect("fact query")
                .expect("fact")
                .enabled
        );
    }

    #[test]
    fn memory_dream_phase0_contract_excludes_chat_runs() {
        assert_eq!(
            MemoryDreamScope::parse("global").unwrap().as_str(),
            "global"
        );
        assert_eq!(
            MemoryDreamScope::parse("workspace").unwrap().as_str(),
            "workspace"
        );
        assert!(MemoryDreamScope::parse("chat").is_err());
        assert_eq!(
            MemoryDreamTriggerType::parse("auto_threshold")
                .unwrap()
                .as_str(),
            "auto_threshold"
        );
        assert_eq!(
            MemoryDreamRunMode::parse("deterministic_only")
                .unwrap()
                .as_str(),
            "deterministic_only"
        );

        assert!(MemoryDreamScope::Global.allows_candidate_fact_scope(MemoryScope::Global));
        assert!(!MemoryDreamScope::Global.allows_candidate_fact_scope(MemoryScope::Workspace));
        assert!(MemoryDreamScope::Workspace.allows_candidate_fact_scope(MemoryScope::Workspace));
        assert!(MemoryDreamScope::Workspace.allows_candidate_fact_scope(MemoryScope::Chat));
        assert_eq!(MEMORY_DREAM_TRANSCRIPT_CHAT_KIND, "memory_dream");
        assert!(!MEMORY_DREAM_TRANSCRIPT_VISIBLE_IN_NORMAL_CHAT_LIST);
    }

    #[test]
    fn memory_dream_phase0_safety_policy_enforces_invariants() {
        let policy = MemoryDreamSafetyPolicy::new(2, 1).expect("valid policy");

        assert!(!MEMORY_DREAM_HARD_DELETE_ALLOWED);
        assert!(MemoryDreamSafetyPolicy::new(0, 1).is_err());
        assert!(policy.validate_batch_size(2, 1).is_ok());
        assert!(policy.validate_batch_size(3, 1).is_err());
        assert!(policy.validate_batch_size(2, 2).is_err());
        assert!(
            policy
                .validate_updated_at("2026-06-23T00:00:00Z", "2026-06-23T00:00:00Z")
                .is_ok()
        );
        assert!(policy.validate_updated_at("old", "new").is_err());

        assert!(!policy.allows_automatic_global_promotion(false));
        assert!(policy.allows_automatic_global_promotion(true));
        assert!(policy.allows_direct_expiration(MemoryKind::ProjectFact, false, false, false));
        assert!(!policy.allows_direct_expiration(MemoryKind::ProjectFact, true, true, false));
        assert!(!policy.allows_direct_expiration(MemoryKind::UserNote, false, false, true));
        assert!(policy.allows_direct_expiration(MemoryKind::UserNote, false, true, true));
    }

    #[test]
    fn memory_dream_candidates_use_maintenance_buckets() {
        let profile = tempfile::tempdir().expect("profile");
        let mut database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");

        for (id, status, confidence) in [
            ("fact-active", MemoryStatus::Active, Some(0.5)),
            ("fact-pending", MemoryStatus::Pending, Some(0.4)),
            ("fact-future-expiry", MemoryStatus::Active, Some(0.7)),
            ("fact-reference-issue", MemoryStatus::Active, Some(0.6)),
            ("fact-high-pending", MemoryStatus::Pending, Some(0.9)),
            ("fact-due-expiry", MemoryStatus::Active, Some(0.8)),
        ] {
            let source_id = format!("{id}-source");
            database
                .insert_source(NewMemorySource {
                    id: &source_id,
                    scope: MemoryScope::Global,
                    chat_id: None,
                    source_type: MemorySourceType::ManualNote,
                    source_id: None,
                    title: "Candidate source",
                    content: id,
                    metadata_json: "{}",
                })
                .expect("source insert");
            database
                .insert_fact(NewMemoryFact {
                    id,
                    scope: MemoryScope::Global,
                    chat_id: None,
                    status,
                    kind: MemoryKind::Preference,
                    fact: id,
                    confidence,
                    pinned: false,
                    source_ids: &[source_id.as_str()],
                    metadata_json: "{}",
                })
                .expect("fact insert");
        }
        database
            .update_fact(UpdateMemoryFact {
                id: "fact-due-expiry",
                expires_at: Some("2000-01-01T00:00:00.000Z"),
                ..UpdateMemoryFact::default()
            })
            .expect("due expiry");
        database
            .update_fact(UpdateMemoryFact {
                id: "fact-future-expiry",
                expires_at: Some("2999-01-01T00:00:00.000Z"),
                ..UpdateMemoryFact::default()
            })
            .expect("future expiry");
        database
            .replace_fact_references(
                "fact-reference-issue",
                &[NewMemoryReference {
                    id: "reference-invalid",
                    fact_id: "fact-reference-issue",
                    reference_type: MemoryReferenceType::FilePath,
                    value: "missing.rs",
                    normalized_value: "missing.rs",
                    status: MemoryReferenceStatus::Invalid,
                    metadata_json: r#"{"reason":"notFound"}"#,
                    checked_at: Some("2026-06-23T00:00:00Z"),
                }],
            )
            .expect("reference issue");

        let candidates = database
            .dream_candidate_facts(MemoryDreamScope::Global, None, 10)
            .expect("candidate facts");
        let ids = candidates
            .iter()
            .map(|fact| fact.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                "fact-due-expiry",
                "fact-high-pending",
                "fact-reference-issue",
                "fact-future-expiry",
                "fact-pending",
                "fact-active",
            ]
        );
    }

    #[test]
    fn memory_dream_jobs_and_changes_round_trip() {
        let profile = tempfile::tempdir().expect("profile");
        let mut database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");

        database
            .insert_dream_job(NewMemoryDreamJob {
                id: "dream-job-1",
                scope: MemoryDreamScope::Global,
                workspace_id: None,
                trigger_type: MemoryDreamTriggerType::Manual,
                mode: MemoryDreamRunMode::DeterministicOnly,
                status: MemoryDreamJobStatus::Queued,
                model_id: Some("model-1"),
                input_summary_json: r#"{"candidateFacts":1}"#,
                output_summary_json: None,
                transcript_chat_id: None,
                error_message: None,
            })
            .expect("dream job insert");
        assert_eq!(
            database
                .claim_dream_job_running("dream-job-1")
                .expect("claim running"),
            MemoryDreamJobTransitionOutcome::Applied
        );
        assert!(
            database
                .update_dream_job_status(UpdateMemoryDreamJob {
                    id: "dream-job-1",
                    status: MemoryDreamJobStatus::Running,
                    output_summary_json: None,
                    transcript_chat_id: Some("transcript-chat-1"),
                    error_message: None,
                })
                .expect("attach transcript")
        );
        assert!(
            database
                .update_dream_job_status(UpdateMemoryDreamJob {
                    id: "dream-job-1",
                    status: MemoryDreamJobStatus::Completed,
                    output_summary_json: Some(
                        r#"{"authorization":"Bearer secret","changesApplied":1}"#
                    ),
                    transcript_chat_id: None,
                    error_message: None,
                })
                .expect("mark completed")
        );

        database
            .insert_dream_change(NewMemoryDreamChange {
                id: "dream-change-1",
                job_id: "dream-job-1",
                operation: "expire",
                target_fact_ids_json: r#"["fact-1"]"#,
                new_fact_id: None,
                before_json: Some(r#"{"id":"fact-1","api_key":"sk-secret","status":"active"}"#),
                after_json: None,
                reason: "Fact expired.",
                confidence: Some(1.0),
                risk_level: "low",
                status: MemoryDreamChangeStatus::Proposed,
                evidence_json: r#"[{"sourceType":"memory_fact","sourceId":"fact-1"}]"#,
                error_message: None,
            })
            .expect("dream change insert");
        assert!(
            database
                .update_dream_change_status(UpdateMemoryDreamChange {
                    id: "dream-change-1",
                    status: MemoryDreamChangeStatus::Applied,
                    after_json: Some(r#"{"id":"fact-1","status":"expired"}"#),
                    error_message: None,
                })
                .expect("mark change applied")
        );

        let jobs = database
            .dream_jobs_for_scope(
                MemoryDreamScope::Global,
                None,
                Some(MemoryDreamJobStatus::Completed),
                10,
            )
            .expect("completed dream jobs");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, "completed");
        assert_eq!(
            jobs[0].transcript_chat_id.as_deref(),
            Some("transcript-chat-1")
        );
        assert!(jobs[0].started_at.is_some());
        assert!(jobs[0].completed_at.is_some());
        assert_eq!(
            serde_json::from_str::<Value>(jobs[0].output_summary_json.as_deref().unwrap())
                .expect("output json")["authorization"],
            "[REDACTED]"
        );
        assert_eq!(
            database
                .latest_successful_dream_time(MemoryDreamScope::Global, None)
                .expect("latest successful dream"),
            jobs[0].completed_at
        );

        let changes = database
            .dream_changes_for_job("dream-job-1", Some(MemoryDreamChangeStatus::Applied), 10)
            .expect("applied dream changes");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].status, "applied");
        assert!(changes[0].applied_at.is_some());
        assert!(
            changes[0]
                .after_json
                .as_deref()
                .unwrap()
                .contains("expired")
        );
        assert_eq!(
            serde_json::from_str::<Value>(changes[0].before_json.as_deref().unwrap())
                .expect("before json")["api_key"],
            "[REDACTED]"
        );
    }

    #[test]
    fn memory_dream_start_is_singleflight_and_terminal_is_strict() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let profile = tempfile::tempdir().expect("profile");
        let path = profile.path().to_path_buf();
        {
            let mut database =
                MemoryDatabase::open_or_create_global(&path).expect("global memory database");
            assert_eq!(
                database
                    .start_dream_job(NewMemoryDreamJob {
                        id: "dream-a",
                        scope: MemoryDreamScope::Global,
                        workspace_id: None,
                        trigger_type: MemoryDreamTriggerType::Manual,
                        mode: MemoryDreamRunMode::DeterministicOnly,
                        status: MemoryDreamJobStatus::Running,
                        model_id: None,
                        input_summary_json: "{}",
                        output_summary_json: None,
                        transcript_chat_id: None,
                        error_message: None,
                    })
                    .expect("start a"),
                StartMemoryDreamJobOutcome::Started
            );
            assert_eq!(
                database
                    .start_dream_job(NewMemoryDreamJob {
                        id: "dream-b",
                        scope: MemoryDreamScope::Global,
                        workspace_id: None,
                        trigger_type: MemoryDreamTriggerType::Manual,
                        mode: MemoryDreamRunMode::DeterministicOnly,
                        status: MemoryDreamJobStatus::Running,
                        model_id: None,
                        input_summary_json: "{}",
                        output_summary_json: None,
                        transcript_chat_id: None,
                        error_message: None,
                    })
                    .expect("start b"),
                StartMemoryDreamJobOutcome::AlreadyActive
            );
            assert!(
                database
                    .finish_dream_job(UpdateMemoryDreamJob {
                        id: "dream-a",
                        status: MemoryDreamJobStatus::Completed,
                        output_summary_json: Some(r#"{"ok":true}"#),
                        transcript_chat_id: None,
                        error_message: None,
                    })
                    .expect("complete")
                    == MemoryDreamJobTransitionOutcome::Applied
            );
            // Terminal cannot be overwritten by a late failure path.
            assert!(
                database
                    .finish_dream_job(UpdateMemoryDreamJob {
                        id: "dream-a",
                        status: MemoryDreamJobStatus::Failed,
                        output_summary_json: None,
                        transcript_chat_id: None,
                        error_message: Some("late"),
                    })
                    .expect("late fail")
                    == MemoryDreamJobTransitionOutcome::NotApplied
            );
        }

        // Concurrent starters: only one succeeds under the partial unique index.
        let barrier = Arc::new(Barrier::new(2));
        let path_a = path.clone();
        let barrier_a = Arc::clone(&barrier);
        let thread_a = thread::spawn(move || {
            let mut database = MemoryDatabase::open_or_create_global(&path_a).expect("conn a");
            barrier_a.wait();
            database
                .start_dream_job(NewMemoryDreamJob {
                    id: "concurrent-a",
                    scope: MemoryDreamScope::Global,
                    workspace_id: None,
                    trigger_type: MemoryDreamTriggerType::Manual,
                    mode: MemoryDreamRunMode::DeterministicOnly,
                    status: MemoryDreamJobStatus::Running,
                    model_id: None,
                    input_summary_json: "{}",
                    output_summary_json: None,
                    transcript_chat_id: None,
                    error_message: None,
                })
                .expect("start concurrent a")
        });
        let path_b = path.clone();
        let barrier_b = Arc::clone(&barrier);
        let thread_b = thread::spawn(move || {
            let mut database = MemoryDatabase::open_or_create_global(&path_b).expect("conn b");
            barrier_b.wait();
            database
                .start_dream_job(NewMemoryDreamJob {
                    id: "concurrent-b",
                    scope: MemoryDreamScope::Global,
                    workspace_id: None,
                    trigger_type: MemoryDreamTriggerType::Manual,
                    mode: MemoryDreamRunMode::DeterministicOnly,
                    status: MemoryDreamJobStatus::Running,
                    model_id: None,
                    input_summary_json: "{}",
                    output_summary_json: None,
                    transcript_chat_id: None,
                    error_message: None,
                })
                .expect("start concurrent b")
        });
        let outcome_a = thread_a.join().expect("join a");
        let outcome_b = thread_b.join().expect("join b");
        let started = matches!(outcome_a, StartMemoryDreamJobOutcome::Started) as u8
            + matches!(outcome_b, StartMemoryDreamJobOutcome::Started) as u8;
        let blocked = matches!(outcome_a, StartMemoryDreamJobOutcome::AlreadyActive) as u8
            + matches!(outcome_b, StartMemoryDreamJobOutcome::AlreadyActive) as u8;
        assert_eq!(started, 1);
        assert_eq!(blocked, 1);
    }

    #[test]
    fn finish_dream_job_running_does_not_claim_from_queued() {
        let profile = tempfile::tempdir().expect("profile");
        let mut database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        assert_eq!(
            database
                .start_dream_job(NewMemoryDreamJob {
                    id: "queued-only",
                    scope: MemoryDreamScope::Global,
                    workspace_id: None,
                    trigger_type: MemoryDreamTriggerType::Manual,
                    mode: MemoryDreamRunMode::DeterministicOnly,
                    status: MemoryDreamJobStatus::Queued,
                    model_id: None,
                    input_summary_json: "{}",
                    output_summary_json: None,
                    transcript_chat_id: None,
                    error_message: None,
                })
                .expect("insert queued"),
            StartMemoryDreamJobOutcome::Started
        );

        // finish_dream_job(Running) must not claim; only claim_dream_job_running may.
        assert_eq!(
            database
                .finish_dream_job(UpdateMemoryDreamJob {
                    id: "queued-only",
                    status: MemoryDreamJobStatus::Running,
                    output_summary_json: None,
                    transcript_chat_id: Some("chat-1"),
                    error_message: None,
                })
                .expect("running attach"),
            MemoryDreamJobTransitionOutcome::NotApplied
        );
        let job = database
            .dream_job("queued-only")
            .expect("load")
            .expect("exists");
        assert_eq!(job.status, MemoryDreamJobStatus::Queued.as_str());
        assert!(job.transcript_chat_id.is_none());

        assert_eq!(
            database
                .claim_dream_job_running("queued-only")
                .expect("claim"),
            MemoryDreamJobTransitionOutcome::Applied
        );
        assert_eq!(
            database
                .finish_dream_job(UpdateMemoryDreamJob {
                    id: "queued-only",
                    status: MemoryDreamJobStatus::Running,
                    output_summary_json: None,
                    transcript_chat_id: Some("chat-1"),
                    error_message: None,
                })
                .expect("attach after claim"),
            MemoryDreamJobTransitionOutcome::Applied
        );
        let job = database
            .dream_job("queued-only")
            .expect("load")
            .expect("exists");
        assert_eq!(job.status, MemoryDreamJobStatus::Running.as_str());
        assert_eq!(job.transcript_chat_id.as_deref(), Some("chat-1"));
    }

    #[test]
    fn memory_dream_duplicate_id_is_sqlite_not_already_active() {
        let profile = tempfile::tempdir().expect("profile");
        let mut database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        database
            .start_dream_job(NewMemoryDreamJob {
                id: "same-id",
                scope: MemoryDreamScope::Global,
                workspace_id: None,
                trigger_type: MemoryDreamTriggerType::Manual,
                mode: MemoryDreamRunMode::DeterministicOnly,
                status: MemoryDreamJobStatus::Completed,
                model_id: None,
                input_summary_json: "{}",
                output_summary_json: Some(r#"{"ok":true}"#),
                transcript_chat_id: None,
                error_message: None,
            })
            .expect("first insert");
        let err = database
            .start_dream_job(NewMemoryDreamJob {
                id: "same-id",
                scope: MemoryDreamScope::Global,
                workspace_id: None,
                trigger_type: MemoryDreamTriggerType::Manual,
                mode: MemoryDreamRunMode::DeterministicOnly,
                status: MemoryDreamJobStatus::Completed,
                model_id: None,
                input_summary_json: "{}",
                output_summary_json: Some(r#"{"ok":true}"#),
                transcript_chat_id: None,
                error_message: None,
            })
            .expect_err("duplicate primary key");
        assert!(
            matches!(err, MemoryDatabaseError::Sqlite { .. }),
            "expected Sqlite error for PK conflict, got {err:?}"
        );
    }

    #[test]
    fn global_memory_migration_6_collapses_multi_active_dreams() {
        let profile = tempfile::tempdir().expect("profile");
        let database_path = global_memory_database_path(profile.path());
        {
            let connection = Connection::open(&database_path).expect("open raw");
            connection
                .execute_batch(&format!(
                    "{GLOBAL_MEMORY_SCHEMA_SQL}
                     {GLOBAL_MEMORY_DREAM_SCHEMA_SQL}
                     {MEMORY_REFERENCES_SCHEMA_SQL}
                     {GLOBAL_MEMORY_EXTRACTION_SKIPPED_STATUS_MIGRATION_SQL}
                     PRAGMA user_version = 5;
                     INSERT INTO memory_dream_jobs
                         (id, scope, workspace_id, trigger_type, mode, status, input_summary_json, created_at)
                     VALUES
                         ('old-queued', 'global', NULL, 'manual', 'deterministic_only', 'queued', '{{}}', '2026-07-01T00:00:00Z'),
                         ('keep-running', 'global', NULL, 'manual', 'deterministic_only', 'running', '{{}}', '2026-07-02T00:00:00Z');"
                ))
                .expect("seed global v5 multi-active");
        }

        let database = MemoryDatabase::open_or_create_global(profile.path()).expect("migrate to 6");
        assert_eq!(
            database.schema_version().expect("schema version"),
            GLOBAL_MEMORY_SCHEMA_VERSION
        );
        let connection = Connection::open(database.database_path()).expect("open database");
        let active: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM memory_dream_jobs WHERE status IN ('queued', 'running')",
                [],
                |row| row.get(0),
            )
            .expect("active count");
        assert_eq!(active, 1);
        let kept: String = connection
            .query_row(
                "SELECT id FROM memory_dream_jobs WHERE status IN ('queued', 'running')",
                [],
                |row| row.get(0),
            )
            .expect("kept id");
        assert_eq!(kept, "keep-running");
        let failed_error: String = connection
            .query_row(
                "SELECT error_message FROM memory_dream_jobs WHERE id = 'old-queued'",
                [],
                |row| row.get(0),
            )
            .expect("collapsed error");
        assert!(failed_error.contains("collapsed during schema migration 6"));
        let singleflight: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'memory_dream_jobs_active_singleflight_idx'",
                [],
                |row| row.get(0),
            )
            .expect("singleflight index");
        assert_eq!(singleflight, 1);
    }

    #[test]
    fn workspace_memory_dream_start_is_singleflight_across_connections() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let workspace = tempfile::tempdir().expect("workspace");
        let path = workspace.path().to_path_buf();
        {
            let mut database =
                MemoryDatabase::open_or_create_workspace(&path).expect("workspace memory");
            assert_eq!(
                database
                    .start_dream_job(NewMemoryDreamJob {
                        id: "ws-dream-a",
                        scope: MemoryDreamScope::Workspace,
                        workspace_id: Some("ws-1"),
                        trigger_type: MemoryDreamTriggerType::Manual,
                        mode: MemoryDreamRunMode::DeterministicOnly,
                        status: MemoryDreamJobStatus::Running,
                        model_id: None,
                        input_summary_json: "{}",
                        output_summary_json: None,
                        transcript_chat_id: None,
                        error_message: None,
                    })
                    .expect("start a"),
                StartMemoryDreamJobOutcome::Started
            );
            assert_eq!(
                database
                    .start_dream_job(NewMemoryDreamJob {
                        id: "ws-dream-b",
                        scope: MemoryDreamScope::Workspace,
                        workspace_id: Some("ws-1"),
                        trigger_type: MemoryDreamTriggerType::Manual,
                        mode: MemoryDreamRunMode::DeterministicOnly,
                        status: MemoryDreamJobStatus::Running,
                        model_id: None,
                        input_summary_json: "{}",
                        output_summary_json: None,
                        transcript_chat_id: None,
                        error_message: None,
                    })
                    .expect("start b"),
                StartMemoryDreamJobOutcome::AlreadyActive
            );
            assert_eq!(
                database
                    .finish_dream_job(UpdateMemoryDreamJob {
                        id: "ws-dream-a",
                        status: MemoryDreamJobStatus::Completed,
                        output_summary_json: Some(r#"{"ok":true}"#),
                        transcript_chat_id: None,
                        error_message: None,
                    })
                    .expect("complete"),
                MemoryDreamJobTransitionOutcome::Applied
            );
            assert_eq!(
                database
                    .finish_dream_job(UpdateMemoryDreamJob {
                        id: "ws-dream-a",
                        status: MemoryDreamJobStatus::Failed,
                        output_summary_json: None,
                        transcript_chat_id: None,
                        error_message: Some("late"),
                    })
                    .expect("late fail"),
                MemoryDreamJobTransitionOutcome::NotApplied
            );
        }

        let barrier = Arc::new(Barrier::new(2));
        let path_a = path.clone();
        let barrier_a = Arc::clone(&barrier);
        let thread_a = thread::spawn(move || {
            let mut database = MemoryDatabase::open_or_create_workspace(&path_a).expect("conn a");
            barrier_a.wait();
            database
                .start_dream_job(NewMemoryDreamJob {
                    id: "ws-concurrent-a",
                    scope: MemoryDreamScope::Workspace,
                    workspace_id: Some("ws-1"),
                    trigger_type: MemoryDreamTriggerType::Manual,
                    mode: MemoryDreamRunMode::DeterministicOnly,
                    status: MemoryDreamJobStatus::Running,
                    model_id: None,
                    input_summary_json: "{}",
                    output_summary_json: None,
                    transcript_chat_id: None,
                    error_message: None,
                })
                .expect("start concurrent a")
        });
        let path_b = path.clone();
        let barrier_b = Arc::clone(&barrier);
        let thread_b = thread::spawn(move || {
            let mut database = MemoryDatabase::open_or_create_workspace(&path_b).expect("conn b");
            barrier_b.wait();
            database
                .start_dream_job(NewMemoryDreamJob {
                    id: "ws-concurrent-b",
                    scope: MemoryDreamScope::Workspace,
                    workspace_id: Some("ws-1"),
                    trigger_type: MemoryDreamTriggerType::Manual,
                    mode: MemoryDreamRunMode::DeterministicOnly,
                    status: MemoryDreamJobStatus::Running,
                    model_id: None,
                    input_summary_json: "{}",
                    output_summary_json: None,
                    transcript_chat_id: None,
                    error_message: None,
                })
                .expect("start concurrent b")
        });
        let outcome_a = thread_a.join().expect("join a");
        let outcome_b = thread_b.join().expect("join b");
        let started = matches!(outcome_a, StartMemoryDreamJobOutcome::Started) as u8
            + matches!(outcome_b, StartMemoryDreamJobOutcome::Started) as u8;
        let blocked = matches!(outcome_a, StartMemoryDreamJobOutcome::AlreadyActive) as u8
            + matches!(outcome_b, StartMemoryDreamJobOutcome::AlreadyActive) as u8;
        assert_eq!(started, 1);
        assert_eq!(blocked, 1);
    }

    #[test]
    fn memory_references_replace_and_list_by_fact() {
        let profile = tempfile::tempdir().expect("profile");
        let mut database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        database
            .insert_fact(NewMemoryFact {
                id: "fact-1",
                scope: MemoryScope::Global,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::UserNote,
                fact: "Use app/main.rs for startup behavior.",
                confidence: Some(0.9),
                pinned: false,
                source_ids: &[],
                metadata_json: "{}",
            })
            .expect("fact insert");

        database
            .replace_fact_references(
                "fact-1",
                &[NewMemoryReference {
                    id: "reference-1",
                    fact_id: "fact-1",
                    reference_type: MemoryReferenceType::FilePath,
                    value: "app/main.rs",
                    normalized_value: "app/main.rs",
                    status: MemoryReferenceStatus::Valid,
                    metadata_json: r#"{"path":"app/main.rs"}"#,
                    checked_at: Some("2026-06-23T00:00:00Z"),
                }],
            )
            .expect("replace references");

        let references = database
            .references_for_fact_ids(&["fact-1".to_string()], 10)
            .expect("references");
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].reference_type, "file_path");
        assert_eq!(references[0].status, "valid");

        database
            .replace_fact_references("fact-1", &[])
            .expect("clear references");
        assert!(
            database
                .references_for_fact_ids(&["fact-1".to_string()], 10)
                .expect("references")
                .is_empty()
        );
    }

    #[test]
    fn global_database_round_trips_active_fact_search() {
        let profile = tempfile::tempdir().expect("profile");
        let mut database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");

        database
            .insert_source(NewMemorySource {
                id: "source-1",
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Manual note",
                content: "Prefer concise implementation notes.",
                metadata_json: "{}",
            })
            .expect("source insert");
        database
            .insert_fact(NewMemoryFact {
                id: "fact-1",
                scope: MemoryScope::Global,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::Preference,
                fact: "Prefer concise implementation notes.",
                confidence: Some(1.0),
                pinned: true,
                source_ids: &["source-1"],
                metadata_json: "{}",
            })
            .expect("fact insert");

        let results = database
            .search_active_facts("concise", 5)
            .expect("memory search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "fact-1");
        assert!(results[0].pinned);
    }

    #[test]
    fn global_database_filters_list_by_kind_and_query() {
        let profile = tempfile::tempdir().expect("profile");
        let mut database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");

        for (id, kind, status, fact) in [
            (
                "preference",
                MemoryKind::Preference,
                MemoryStatus::Active,
                "Prefer compact memory lists.",
            ),
            (
                "decision",
                MemoryKind::ProjectDecision,
                MemoryStatus::Active,
                "Memory list uses a dialog for edits.",
            ),
            (
                "pending",
                MemoryKind::Preference,
                MemoryStatus::Pending,
                "Pending memory also supports keyword filtering.",
            ),
        ] {
            let source_id = format!("source-{id}");
            database
                .insert_source(NewMemorySource {
                    id: &source_id,
                    scope: MemoryScope::Global,
                    chat_id: None,
                    source_type: MemorySourceType::ManualNote,
                    source_id: None,
                    title: "Manual note",
                    content: fact,
                    metadata_json: "{}",
                })
                .expect("source insert");
            database
                .insert_fact(NewMemoryFact {
                    id,
                    scope: MemoryScope::Global,
                    chat_id: None,
                    status,
                    kind,
                    fact,
                    confidence: None,
                    pinned: false,
                    source_ids: &[source_id.as_str()],
                    metadata_json: "{}",
                })
                .expect("fact insert");
        }

        let active_preferences = database
            .list_facts_for_scope(
                None,
                MemoryStatus::Active,
                Some(MemoryKind::Preference),
                None,
                10,
            )
            .expect("kind filtered active facts");
        assert_eq!(active_preferences.len(), 1);
        assert_eq!(active_preferences[0].id, "preference");

        let pending_keyword = database
            .list_facts_for_scope(None, MemoryStatus::Pending, None, Some("keyword"), 10)
            .expect("query filtered pending facts");
        assert_eq!(pending_keyword.len(), 1);
        assert_eq!(pending_keyword[0].id, "pending");

        let active_total = database
            .count_facts_for_scope(None, MemoryStatus::Active, None, None)
            .expect("count active facts");
        assert_eq!(active_total, 2);

        let second_active_page = database
            .list_facts_for_scope_page(None, MemoryStatus::Active, None, None, 1, 1)
            .expect("second active page");
        assert_eq!(second_active_page.len(), 1);
    }

    #[test]
    fn workspace_database_lists_exact_scope_fact_ids() {
        let workspace = tempfile::tempdir().expect("workspace");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create_ungated(workspace.path())
                    .expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "Memory chat")
                .expect("chat insert");
        }
        let mut database =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory database");

        for (id, scope, chat_id, fact) in [
            (
                "workspace",
                MemoryScope::Workspace,
                None,
                "Workspace memory visible in chat context.",
            ),
            (
                "chat",
                MemoryScope::Chat,
                Some("chat-1"),
                "Chat memory clear target.",
            ),
        ] {
            let source_id = format!("source-{id}");
            database
                .insert_source(NewMemorySource {
                    id: &source_id,
                    scope,
                    chat_id,
                    source_type: MemorySourceType::ManualNote,
                    source_id: None,
                    title: "Manual note",
                    content: fact,
                    metadata_json: "{}",
                })
                .expect("source insert");
            database
                .insert_fact(NewMemoryFact {
                    id,
                    scope,
                    chat_id,
                    status: MemoryStatus::Active,
                    kind: MemoryKind::ProjectFact,
                    fact,
                    confidence: None,
                    pinned: false,
                    source_ids: &[source_id.as_str()],
                    metadata_json: "{}",
                })
                .expect("fact insert");
        }

        let chat_visible = database
            .list_facts_for_scope(Some("chat-1"), MemoryStatus::Active, None, None, 10)
            .expect("chat visible facts");
        assert_eq!(chat_visible.len(), 2);

        let exact_chat_ids = database
            .list_fact_ids_for_exact_scope(
                MemoryScope::Chat,
                Some("chat-1"),
                MemoryStatus::Active,
                None,
                None,
            )
            .expect("exact chat fact ids");
        assert_eq!(exact_chat_ids, vec!["chat".to_string()]);

        let exact_workspace_ids = database
            .list_fact_ids_for_exact_scope(
                MemoryScope::Workspace,
                None,
                MemoryStatus::Active,
                None,
                None,
            )
            .expect("exact workspace fact ids");
        assert_eq!(exact_workspace_ids, vec!["workspace".to_string()]);
    }

    #[test]
    fn non_user_note_facts_require_source_evidence() {
        let profile = tempfile::tempdir().expect("profile");
        let mut database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");

        let error = database
            .insert_fact(NewMemoryFact {
                id: "fact-1",
                scope: MemoryScope::Global,
                chat_id: None,
                status: MemoryStatus::Pending,
                kind: MemoryKind::ProjectFact,
                fact: "Foco stores global memories in memory.sqlite.",
                confidence: Some(0.8),
                pinned: false,
                source_ids: &[],
                metadata_json: "{}",
            })
            .expect_err("missing source should fail");

        assert!(error.to_string().contains("at least one source"));
    }

    #[test]
    fn workspace_extraction_jobs_round_trip_queued_status() {
        let workspace = tempfile::tempdir().expect("workspace");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create_ungated(workspace.path())
                    .expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "Extraction chat")
                .expect("chat insert");
        }

        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory database");
        memory
            .insert_extraction_job(NewMemoryExtractionJob {
                id: "job-1",
                scope: MemoryScope::Chat,
                chat_id: Some("chat-1"),
                status: MemoryExtractionJobStatus::Queued,
                model_id: Some("model-1"),
                input_json: r#"{"trigger":"chat_completed"}"#,
                output_json: None,
                error_message: None,
            })
            .expect("job insert");

        let jobs = memory
            .extraction_jobs_for_scope(Some("chat-1"), Some(MemoryExtractionJobStatus::Queued), 10)
            .expect("queued jobs");

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "job-1");
        assert_eq!(jobs[0].status, "queued");
        assert_eq!(jobs[0].model_id.as_deref(), Some("model-1"));
    }

    #[test]
    fn workspace_extraction_jobs_update_status_and_redact_json() {
        let workspace = tempfile::tempdir().expect("workspace");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create_ungated(workspace.path())
                    .expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "Extraction chat")
                .expect("chat insert");
        }

        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory database");
        memory
            .insert_extraction_job(NewMemoryExtractionJob {
                id: "job-1",
                scope: MemoryScope::Chat,
                chat_id: Some("chat-1"),
                status: MemoryExtractionJobStatus::Queued,
                model_id: Some("model-1"),
                input_json: r#"{"headers":{"authorization":"Bearer sk-secret"},"safe":"ok"}"#,
                output_json: None,
                error_message: None,
            })
            .expect("job insert");

        assert!(
            memory
                .mark_extraction_job_running("job-1")
                .expect("mark running")
        );
        assert!(
            memory
                .fail_extraction_job(
                    "job-1",
                    "provider failed",
                    Some(r#"{"password":"secret","facts":[]}"#)
                )
                .expect("mark failed")
        );
        let failed = memory
            .extraction_jobs_for_scope(Some("chat-1"), Some(MemoryExtractionJobStatus::Failed), 10)
            .expect("failed jobs");

        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].status, "failed");
        assert_eq!(failed[0].error_message.as_deref(), Some("provider failed"));
        assert!(failed[0].started_at.is_some());
        assert!(failed[0].completed_at.is_some());
        assert!(!failed[0].input_json.contains("sk-secret"));
        assert_eq!(
            serde_json::from_str::<Value>(&failed[0].input_json).expect("input json")["headers"]["authorization"],
            "[REDACTED]"
        );
        assert!(!failed[0].output_json.as_deref().unwrap().contains("secret"));
        assert_eq!(
            serde_json::from_str::<Value>(failed[0].output_json.as_deref().unwrap())
                .expect("output json")["password"],
            "[REDACTED]"
        );
        let all_failed = memory
            .extraction_jobs(Some(MemoryExtractionJobStatus::Failed), 10)
            .expect("all failed jobs");
        assert_eq!(all_failed.len(), 1);
        assert_eq!(all_failed[0].id, "job-1");

        assert!(
            memory
                .skip_failed_extraction_job("job-1")
                .expect("mark skipped")
        );
        let skipped = memory
            .extraction_job("job-1")
            .expect("skipped job")
            .expect("job exists");
        assert_eq!(skipped.status, "skipped");
        let failed_after_skip = memory
            .extraction_jobs(Some(MemoryExtractionJobStatus::Failed), 10)
            .expect("failed jobs after skip");
        assert!(failed_after_skip.is_empty());

        assert!(
            memory
                .retry_failed_extraction_job("job-1", "model-2")
                .expect("retry skipped job ignored")
                == false
        );

        memory
            .insert_extraction_job(NewMemoryExtractionJob {
                id: "job-2",
                scope: MemoryScope::Chat,
                chat_id: Some("chat-1"),
                status: MemoryExtractionJobStatus::Failed,
                model_id: Some("model-1"),
                input_json: r#"{"safe":"ok"}"#,
                output_json: None,
                error_message: Some("provider failed"),
            })
            .expect("second failed job insert");
        assert!(
            memory
                .retry_failed_extraction_job("job-2", "model-2")
                .expect("retry failed job")
        );
        let retried = memory
            .extraction_job("job-2")
            .expect("retried job")
            .expect("job exists");
        assert_eq!(retried.status, "running");
        assert_eq!(retried.model_id.as_deref(), Some("model-2"));

        assert!(
            memory
                .complete_extraction_job("job-1", r#"{"apiKey":"sk-secret","facts":[]}"#)
                .expect("complete skipped job is rejected")
                == false
        );
        assert!(
            memory
                .complete_extraction_job("job-2", r#"{"apiKey":"sk-secret","facts":[]}"#)
                .expect("mark completed")
        );
        let completed = memory
            .extraction_jobs_for_scope(
                Some("chat-1"),
                Some(MemoryExtractionJobStatus::Completed),
                10,
            )
            .expect("completed jobs");

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "job-2");
        assert!(completed[0].error_message.is_none());
        assert!(
            !completed[0]
                .output_json
                .as_deref()
                .unwrap()
                .contains("sk-secret")
        );
    }

    #[test]
    fn concurrent_extraction_claim_only_succeeds_once() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let workspace = tempfile::tempdir().expect("workspace");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create_ungated(workspace.path())
                    .expect("workspace database");
            workspace_database
                .insert_chat("chat-claim", "Extraction claim chat")
                .expect("chat insert");
        }
        {
            let mut memory =
                MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                    .expect("workspace memory database");
            memory
                .insert_extraction_job(NewMemoryExtractionJob {
                    id: "job-claim",
                    scope: MemoryScope::Chat,
                    chat_id: Some("chat-claim"),
                    status: MemoryExtractionJobStatus::Queued,
                    model_id: Some("model-1"),
                    input_json: r#"{"trigger":"chat_completed"}"#,
                    output_json: None,
                    error_message: None,
                })
                .expect("job insert");
        }

        const THREAD_COUNT: usize = 8;
        let workspace_path = Arc::new(workspace.path().to_path_buf());
        let barrier = Arc::new(Barrier::new(THREAD_COUNT));
        let threads = (0..THREAD_COUNT)
            .map(|_| {
                let workspace_path = Arc::clone(&workspace_path);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    let mut memory = MemoryDatabase::open_workspace_at(workspace_database_path(
                        workspace_path.as_path(),
                    ))
                    .expect("workspace memory database");
                    memory
                        .mark_extraction_job_running("job-claim")
                        .expect("claim attempt")
                })
            })
            .collect::<Vec<_>>();

        let claimed = threads
            .into_iter()
            .map(|thread| thread.join().expect("claim thread"))
            .filter(|claimed| *claimed)
            .count();
        assert_eq!(claimed, 1);

        let memory = MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
            .expect("workspace memory database");
        let job = memory
            .extraction_job("job-claim")
            .expect("job")
            .expect("job exists");
        assert_eq!(job.status, "running");
    }

    #[test]
    fn concurrent_unlink_keeps_non_user_note_source_invariant() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let profile = tempfile::tempdir().expect("profile");
        {
            let mut database =
                MemoryDatabase::open_or_create_global(profile.path()).expect("global memory");
            database
                .insert_source(NewMemorySource {
                    id: "source-a",
                    scope: MemoryScope::Global,
                    chat_id: None,
                    source_type: MemorySourceType::ManualNote,
                    source_id: None,
                    title: "A",
                    content: "source a",
                    metadata_json: "{}",
                })
                .expect("source a");
            database
                .insert_source(NewMemorySource {
                    id: "source-b",
                    scope: MemoryScope::Global,
                    chat_id: None,
                    source_type: MemorySourceType::ManualNote,
                    source_id: None,
                    title: "B",
                    content: "source b",
                    metadata_json: "{}",
                })
                .expect("source b");
            database
                .insert_fact(NewMemoryFact {
                    id: "fact-dual",
                    scope: MemoryScope::Global,
                    chat_id: None,
                    status: MemoryStatus::Active,
                    kind: MemoryKind::ProjectFact,
                    fact: "dual source fact",
                    confidence: Some(0.9),
                    pinned: false,
                    source_ids: &["source-a", "source-b"],
                    metadata_json: "{}",
                })
                .expect("fact");
        }

        let profile_path = Arc::new(profile.path().to_path_buf());
        let barrier = Arc::new(Barrier::new(2));
        let left = {
            let profile_path = Arc::clone(&profile_path);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let mut database =
                    MemoryDatabase::open_or_create_global(profile_path.as_path()).expect("db");
                database.unlink_fact_source("fact-dual", "source-a")
            })
        };
        let right = {
            let profile_path = Arc::clone(&profile_path);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let mut database =
                    MemoryDatabase::open_or_create_global(profile_path.as_path()).expect("db");
                database.unlink_fact_source("fact-dual", "source-b")
            })
        };

        let left_result = left.join().expect("left unlink thread");
        let right_result = right.join().expect("right unlink thread");
        let successes = [&left_result, &right_result]
            .iter()
            .filter(|result| matches!(result, Ok(true)))
            .count();
        let rejections = [&left_result, &right_result]
            .iter()
            .filter(|result| {
                matches!(
                    result,
                    Err(MemoryDatabaseError::InvalidMemoryInput { message })
                        if message.contains("at least one source")
                )
            })
            .count();
        assert_eq!(successes, 1);
        assert_eq!(rejections, 1);

        let database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory");
        assert_eq!(
            database
                .source_count_for_fact("fact-dual")
                .expect("source count"),
            1
        );
    }

    #[test]
    fn concurrent_opposite_updates_edges_reject_cycle() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let profile = tempfile::tempdir().expect("profile");
        {
            let mut database =
                MemoryDatabase::open_or_create_global(profile.path()).expect("global memory");
            for (source_id, content) in [("source-x", "x"), ("source-y", "y")] {
                database
                    .insert_source(NewMemorySource {
                        id: source_id,
                        scope: MemoryScope::Global,
                        chat_id: None,
                        source_type: MemorySourceType::ManualNote,
                        source_id: None,
                        title: source_id,
                        content,
                        metadata_json: "{}",
                    })
                    .expect("source");
            }
            database
                .insert_fact(NewMemoryFact {
                    id: "fact-x",
                    scope: MemoryScope::Global,
                    chat_id: None,
                    status: MemoryStatus::Active,
                    kind: MemoryKind::ProjectFact,
                    fact: "fact x",
                    confidence: Some(0.8),
                    pinned: false,
                    source_ids: &["source-x"],
                    metadata_json: "{}",
                })
                .expect("fact x");
            database
                .insert_fact(NewMemoryFact {
                    id: "fact-y",
                    scope: MemoryScope::Global,
                    chat_id: None,
                    status: MemoryStatus::Active,
                    kind: MemoryKind::ProjectFact,
                    fact: "fact y",
                    confidence: Some(0.8),
                    pinned: false,
                    source_ids: &["source-y"],
                    metadata_json: "{}",
                })
                .expect("fact y");
        }

        let profile_path = Arc::new(profile.path().to_path_buf());
        let barrier = Arc::new(Barrier::new(2));
        let xy = {
            let profile_path = Arc::clone(&profile_path);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let mut database =
                    MemoryDatabase::open_or_create_global(profile_path.as_path()).expect("db");
                database.insert_edge(NewMemoryEdge {
                    id: "edge-x-y",
                    source_fact_id: "fact-x",
                    target_fact_id: "fact-y",
                    relation: MemoryRelationKind::Updates,
                    metadata_json: "{}",
                })
            })
        };
        let yx = {
            let profile_path = Arc::clone(&profile_path);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let mut database =
                    MemoryDatabase::open_or_create_global(profile_path.as_path()).expect("db");
                database.insert_edge(NewMemoryEdge {
                    id: "edge-y-x",
                    source_fact_id: "fact-y",
                    target_fact_id: "fact-x",
                    relation: MemoryRelationKind::Updates,
                    metadata_json: "{}",
                })
            })
        };

        let xy_result = xy.join().expect("xy edge thread");
        let yx_result = yx.join().expect("yx edge thread");
        let successes = [&xy_result, &yx_result]
            .iter()
            .filter(|result| result.is_ok())
            .count();
        let cycle_rejections = [&xy_result, &yx_result]
            .iter()
            .filter(|result| {
                matches!(
                    result,
                    Err(MemoryDatabaseError::InvalidMemoryInput { message })
                        if message.contains("cycle")
                )
            })
            .count();
        assert_eq!(successes, 1);
        assert_eq!(cycle_rejections, 1);
    }

    #[test]
    fn dream_job_status_rejects_terminal_overwrite() {
        let profile = tempfile::tempdir().expect("profile");
        let mut database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory");
        database
            .insert_dream_job(NewMemoryDreamJob {
                id: "dream-job-1",
                scope: MemoryDreamScope::Global,
                workspace_id: None,
                trigger_type: MemoryDreamTriggerType::Manual,
                mode: MemoryDreamRunMode::DeterministicOnly,
                status: MemoryDreamJobStatus::Queued,
                model_id: None,
                input_summary_json: "{}",
                output_summary_json: None,
                transcript_chat_id: None,
                error_message: None,
            })
            .expect("dream job insert");
        assert_eq!(
            database
                .claim_dream_job_running("dream-job-1")
                .expect("claim running"),
            MemoryDreamJobTransitionOutcome::Applied
        );
        assert!(
            database
                .update_dream_job_status(UpdateMemoryDreamJob {
                    id: "dream-job-1",
                    status: MemoryDreamJobStatus::Completed,
                    output_summary_json: Some(r#"{"ok":true}"#),
                    transcript_chat_id: None,
                    error_message: None,
                })
                .expect("completed")
        );
        assert!(
            database
                .update_dream_job_status(UpdateMemoryDreamJob {
                    id: "dream-job-1",
                    status: MemoryDreamJobStatus::Failed,
                    output_summary_json: None,
                    transcript_chat_id: None,
                    error_message: Some("stale runner"),
                })
                .expect("stale fail rejected")
                == false
        );
        let job = database
            .dream_job("dream-job-1")
            .expect("job")
            .expect("exists");
        assert_eq!(job.status, "completed");
    }

    #[test]
    fn workspace_memory_api_promotes_and_preserves_workspace_facts() {
        let workspace = tempfile::tempdir().expect("workspace");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create_ungated(workspace.path())
                    .expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "Memory chat")
                .expect("chat insert");
        }

        let database_path = workspace_database_path(workspace.path());
        let mut memory =
            MemoryDatabase::open_workspace_at(&database_path).expect("workspace memory database");
        memory
            .insert_source(NewMemorySource {
                id: "source-1",
                scope: MemoryScope::Chat,
                chat_id: Some("chat-1"),
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Original note",
                content: "Use workspace memory API for scoped facts.",
                metadata_json: "{}",
            })
            .expect("source insert");
        memory
            .insert_source(NewMemorySource {
                id: "source-2",
                scope: MemoryScope::Chat,
                chat_id: Some("chat-1"),
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Extra note",
                content: "Extra source can be removed after unlink.",
                metadata_json: "{}",
            })
            .expect("second source insert");
        memory
            .insert_fact(NewMemoryFact {
                id: "fact-1",
                scope: MemoryScope::Chat,
                chat_id: Some("chat-1"),
                status: MemoryStatus::Pending,
                kind: MemoryKind::ProjectFact,
                fact: "Use memory API for scoped facts.",
                confidence: Some(0.9),
                pinned: false,
                source_ids: &["source-1"],
                metadata_json: "{}",
            })
            .expect("fact insert");

        memory
            .link_fact_source("fact-1", "source-2")
            .expect("link source");
        assert!(
            memory
                .unlink_fact_source("fact-1", "source-2")
                .expect("unlink source")
        );
        assert!(
            memory
                .delete_source("source-2")
                .expect("delete unlinked source")
        );
        assert!(
            memory
                .delete_source("source-1")
                .expect_err("linked source delete should fail")
                .to_string()
                .contains("still linked")
        );

        assert!(
            memory
                .update_source(UpdateMemorySource {
                    id: "source-1",
                    title: Some("Updated note"),
                    ..UpdateMemorySource::default()
                })
                .expect("source update")
        );
        assert!(
            memory
                .update_fact(UpdateMemoryFact {
                    id: "fact-1",
                    status: Some(MemoryStatus::Active),
                    fact: Some("Use the workspace memory API for scoped facts."),
                    pinned: Some(true),
                    ..UpdateMemoryFact::default()
                })
                .expect("fact update")
        );

        let chat_results = memory
            .search_active_facts_for_scope("workspace", Some("chat-1"), None, 10)
            .expect("chat scoped search");
        assert_eq!(chat_results[0].id, "fact-1");

        memory
            .upsert_profile(NewMemoryProfile {
                id: "chat-profile",
                scope: MemoryScope::Chat,
                chat_id: Some("chat-1"),
                profile_text: "Chat prefers scoped memory facts.",
                metadata_json: "{}",
            })
            .expect("profile upsert");
        assert_eq!(
            memory
                .profile("chat-profile")
                .expect("profile")
                .expect("profile row")
                .profile_text,
            "Chat prefers scoped memory facts."
        );

        let promoted = memory
            .promote_fact("fact-1", "fact-workspace", MemoryScope::Workspace, None)
            .expect("chat to workspace promotion");
        assert_eq!(promoted.scope, "workspace");

        memory
            .insert_edge(NewMemoryEdge {
                id: "edge-1",
                source_fact_id: "fact-workspace",
                target_fact_id: "fact-1",
                relation: MemoryRelationKind::Updates,
                metadata_json: "{}",
            })
            .expect("updates edge");
        assert!(
            memory
                .insert_edge(NewMemoryEdge {
                    id: "edge-2",
                    source_fact_id: "fact-1",
                    target_fact_id: "fact-workspace",
                    relation: MemoryRelationKind::Updates,
                    metadata_json: "{}",
                })
                .expect_err("updates cycle should fail")
                .to_string()
                .contains("cycle")
        );

        let profile = tempfile::tempdir().expect("profile");
        let mut global =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        memory
            .promote_fact_to_database(
                "fact-workspace",
                &mut global,
                "fact-global",
                MemoryScope::Global,
                None,
            )
            .expect("workspace to global promotion");
        assert_eq!(
            global
                .search_active_facts("workspace", 10)
                .expect("global search")[0]
                .id,
            "fact-global"
        );

        drop(memory);
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create_ungated(workspace.path())
                    .expect("workspace database");
            assert!(
                workspace_database
                    .delete_chat("chat-1")
                    .expect("chat delete")
            );
        }

        let memory =
            MemoryDatabase::open_workspace_at(&database_path).expect("workspace memory database");
        assert!(memory.fact("fact-1").expect("chat fact lookup").is_none());
        assert_eq!(
            memory
                .fact("fact-workspace")
                .expect("workspace fact lookup")
                .expect("workspace fact")
                .scope,
            "workspace"
        );
    }

    #[test]
    fn updates_relation_supersedes_active_update_chain() {
        let workspace = tempfile::tempdir().expect("workspace");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create_ungated(workspace.path())
                    .expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "Memory updates")
                .expect("chat insert");
        }

        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory database");
        memory
            .insert_source(NewMemorySource {
                id: "source-1",
                scope: MemoryScope::Chat,
                chat_id: Some("chat-1"),
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Manual note",
                content: "Update chain source.",
                metadata_json: "{}",
            })
            .expect("source insert");
        for (id, status, fact) in [
            ("fact-old", MemoryStatus::Active, "Old memory fact."),
            ("fact-mid", MemoryStatus::Active, "Middle memory fact."),
            ("fact-new", MemoryStatus::Pending, "New memory fact."),
        ] {
            memory
                .insert_fact(NewMemoryFact {
                    id,
                    scope: MemoryScope::Chat,
                    chat_id: Some("chat-1"),
                    status,
                    kind: MemoryKind::ProjectFact,
                    fact,
                    confidence: Some(0.9),
                    pinned: false,
                    source_ids: &["source-1"],
                    metadata_json: "{}",
                })
                .expect("fact insert");
        }
        memory
            .insert_edge(NewMemoryEdge {
                id: "edge-mid-old",
                source_fact_id: "fact-mid",
                target_fact_id: "fact-old",
                relation: MemoryRelationKind::Updates,
                metadata_json: "{}",
            })
            .expect("mid updates old");
        memory
            .insert_edge(NewMemoryEdge {
                id: "edge-new-mid",
                source_fact_id: "fact-new",
                target_fact_id: "fact-mid",
                relation: MemoryRelationKind::Updates,
                metadata_json: "{}",
            })
            .expect("new updates mid");

        assert_eq!(
            memory
                .fact("fact-old")
                .expect("old lookup")
                .expect("old fact")
                .status,
            "superseded"
        );
        assert!(
            memory
                .fact("fact-mid")
                .expect("mid lookup")
                .expect("mid fact")
                .is_latest
        );

        assert!(
            memory
                .set_fact_status("fact-new", MemoryStatus::Active)
                .expect("approve new fact")
        );
        let old = memory
            .fact("fact-old")
            .expect("old lookup")
            .expect("old fact");
        let mid = memory
            .fact("fact-mid")
            .expect("mid lookup")
            .expect("mid fact");
        let new = memory
            .fact("fact-new")
            .expect("new lookup")
            .expect("new fact");

        assert_eq!(old.status, "superseded");
        assert!(!old.is_latest);
        assert_eq!(mid.status, "superseded");
        assert!(!mid.is_latest);
        assert_eq!(new.status, "active");
        assert!(new.is_latest);
    }

    #[test]
    fn non_updates_relations_do_not_supersede_targets_and_self_edges_fail() {
        let workspace = tempfile::tempdir().expect("workspace");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create_ungated(workspace.path())
                    .expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "Memory relations")
                .expect("chat insert");
        }

        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory database");
        memory
            .insert_source(NewMemorySource {
                id: "source-1",
                scope: MemoryScope::Chat,
                chat_id: Some("chat-1"),
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Manual note",
                content: "Relation source.",
                metadata_json: "{}",
            })
            .expect("source insert");
        for id in ["fact-a", "fact-b"] {
            memory
                .insert_fact(NewMemoryFact {
                    id,
                    scope: MemoryScope::Chat,
                    chat_id: Some("chat-1"),
                    status: MemoryStatus::Active,
                    kind: MemoryKind::ProjectFact,
                    fact: "Relation fact.",
                    confidence: Some(0.9),
                    pinned: false,
                    source_ids: &["source-1"],
                    metadata_json: "{}",
                })
                .expect("fact insert");
        }
        memory
            .insert_edge(NewMemoryEdge {
                id: "edge-extends",
                source_fact_id: "fact-b",
                target_fact_id: "fact-a",
                relation: MemoryRelationKind::Extends,
                metadata_json: "{}",
            })
            .expect("extends edge");
        memory
            .insert_edge(NewMemoryEdge {
                id: "edge-derives",
                source_fact_id: "fact-b",
                target_fact_id: "fact-a",
                relation: MemoryRelationKind::Derives,
                metadata_json: r#"{"reason":"inferred from the target fact"}"#,
            })
            .expect("derives edge");

        let target = memory
            .fact("fact-a")
            .expect("target lookup")
            .expect("target fact");
        assert_eq!(target.status, "active");
        assert!(target.is_latest);
        let derives_metadata: String = memory
            .connection
            .query_row(
                "SELECT metadata_json FROM memory_edges WHERE id = 'edge-derives'",
                [],
                |row| row.get(0),
            )
            .expect("derives metadata");
        let derives_metadata =
            serde_json::from_str::<Value>(&derives_metadata).expect("derives metadata json");
        assert_eq!(derives_metadata["sourceFactId"], "fact-b");
        assert_eq!(derives_metadata["targetFactId"], "fact-a");
        assert_eq!(derives_metadata["sourceSourceIds"], json!(["source-1"]));
        assert_eq!(derives_metadata["targetSourceIds"], json!(["source-1"]));
        assert!(
            memory
                .insert_edge(NewMemoryEdge {
                    id: "edge-self",
                    source_fact_id: "fact-a",
                    target_fact_id: "fact-a",
                    relation: MemoryRelationKind::Derives,
                    metadata_json: "{}",
                })
                .expect_err("self edge should fail")
                .to_string()
                .contains("cannot target the same fact")
        );
    }

    #[test]
    fn profile_refresh_uses_active_latest_facts_in_deterministic_source_linked_order() {
        let workspace = tempfile::tempdir().expect("workspace");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create_ungated(workspace.path())
                    .expect("workspace database");
            workspace_database
                .insert_chat("chat-1", "Memory profile")
                .expect("chat insert");
        }

        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory database");
        for (id, content) in [
            ("source-a", "Pinned preference source."),
            ("source-z", "Project fact source."),
            ("source-pending", "Pending fact source."),
            ("source-old", "Superseded fact source."),
        ] {
            memory
                .insert_source(NewMemorySource {
                    id,
                    scope: MemoryScope::Chat,
                    chat_id: Some("chat-1"),
                    source_type: MemorySourceType::ManualNote,
                    source_id: None,
                    title: "Manual note",
                    content,
                    metadata_json: "{}",
                })
                .expect("source insert");
        }
        for (id, status, kind, fact, pinned, source_id) in [
            (
                "fact-z",
                MemoryStatus::Active,
                MemoryKind::ProjectFact,
                "Workspace uses a local memory graph.",
                false,
                "source-z",
            ),
            (
                "fact-a",
                MemoryStatus::Active,
                MemoryKind::Preference,
                "Prefer concise memory summaries.",
                true,
                "source-a",
            ),
            (
                "fact-pending",
                MemoryStatus::Pending,
                MemoryKind::ProjectFact,
                "Pending facts stay out of profile summaries.",
                false,
                "source-pending",
            ),
            (
                "fact-old",
                MemoryStatus::Superseded,
                MemoryKind::ProjectFact,
                "Superseded facts stay out of profile summaries.",
                false,
                "source-old",
            ),
        ] {
            memory
                .insert_fact(NewMemoryFact {
                    id,
                    scope: MemoryScope::Chat,
                    chat_id: Some("chat-1"),
                    status,
                    kind,
                    fact,
                    confidence: Some(0.9),
                    pinned,
                    source_ids: &[source_id],
                    metadata_json: "{}",
                })
                .expect("fact insert");
        }

        let profile = memory
            .refresh_profile_from_active_facts(MemoryScope::Chat, Some("chat-1"), 10)
            .expect("profile refresh")
            .expect("profile row");
        let refreshed_again = memory
            .refresh_profile_from_active_facts(MemoryScope::Chat, Some("chat-1"), 10)
            .expect("second profile refresh")
            .expect("profile row");

        assert_eq!(profile.id, "memory-profile:chat:chat-1");
        assert_eq!(profile.profile_text, refreshed_again.profile_text);
        assert_eq!(
            profile.profile_text,
            "- preference pinned: Prefer concise memory summaries.\n- project_fact: Workspace uses a local memory graph."
        );
        assert!(!profile.profile_text.contains("Pending facts"));
        assert!(!profile.profile_text.contains("Superseded facts"));
        let metadata =
            serde_json::from_str::<Value>(&profile.metadata_json).expect("profile metadata json");
        assert_eq!(metadata["sourceFactIds"], json!(["fact-a", "fact-z"]));
        assert_eq!(
            metadata["sourceLinks"],
            json!([
                {"factId":"fact-a","sourceIds":["source-a"]},
                {"factId":"fact-z","sourceIds":["source-z"]}
            ])
        );
        assert_eq!(metadata["algorithm"], "active-latest-facts-v1");
    }

    #[test]
    fn expired_facts_leave_active_search_and_hard_delete_removes_orphaned_graph_rows() {
        let profile = tempfile::tempdir().expect("profile");
        let mut memory =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");

        for (id, content) in [
            ("source-expired", "Expired memory source."),
            ("source-delete", "Forget-only source."),
            ("source-shared", "Shared source."),
        ] {
            memory
                .insert_source(NewMemorySource {
                    id,
                    scope: MemoryScope::Global,
                    chat_id: None,
                    source_type: MemorySourceType::ManualNote,
                    source_id: None,
                    title: "Manual note",
                    content,
                    metadata_json: "{}",
                })
                .expect("source insert");
        }
        for (id, fact, source_ids) in [
            (
                "fact-expired",
                "This stale memory should expire.",
                vec!["source-expired"],
            ),
            (
                "fact-delete",
                "This forget memory should be hard deleted.",
                vec!["source-delete", "source-shared"],
            ),
            (
                "fact-keep",
                "This retained memory keeps the shared source.",
                vec!["source-shared"],
            ),
            (
                "fact-pending-expired",
                "This pending stale memory should expire.",
                vec!["source-expired"],
            ),
        ] {
            memory
                .insert_fact(NewMemoryFact {
                    id,
                    scope: MemoryScope::Global,
                    chat_id: None,
                    status: if id == "fact-pending-expired" {
                        MemoryStatus::Pending
                    } else {
                        MemoryStatus::Active
                    },
                    kind: MemoryKind::ProjectFact,
                    fact,
                    confidence: Some(0.9),
                    pinned: false,
                    source_ids: &source_ids,
                    metadata_json: "{}",
                })
                .expect("fact insert");
        }
        memory
            .update_fact(UpdateMemoryFact {
                id: "fact-expired",
                expires_at: Some("2020-01-01T00:00:00.000Z"),
                ..UpdateMemoryFact::default()
            })
            .expect("set expiration");
        memory
            .update_fact(UpdateMemoryFact {
                id: "fact-pending-expired",
                expires_at: Some("2020-01-01T00:00:00.000Z"),
                ..UpdateMemoryFact::default()
            })
            .expect("set pending expiration");
        memory
            .insert_edge(NewMemoryEdge {
                id: "edge-delete",
                source_fact_id: "fact-keep",
                target_fact_id: "fact-delete",
                relation: MemoryRelationKind::Extends,
                metadata_json: "{}",
            })
            .expect("edge insert");

        assert_eq!(
            memory
                .expire_due_facts("2026-06-09T00:00:00.000Z")
                .expect("expire due facts"),
            2
        );
        assert_eq!(
            memory
                .fact("fact-expired")
                .expect("expired lookup")
                .expect("expired fact")
                .status,
            "expired"
        );
        assert_eq!(
            memory
                .fact("fact-pending-expired")
                .expect("pending expired lookup")
                .expect("pending expired fact")
                .status,
            "expired"
        );
        assert!(
            memory
                .search_active_facts("stale", 10)
                .expect("active search")
                .is_empty()
        );

        assert!(
            memory
                .hard_delete_fact("fact-delete")
                .expect("hard delete fact")
        );
        assert!(
            memory
                .fact("fact-delete")
                .expect("deleted lookup")
                .is_none()
        );
        assert!(
            memory
                .source("source-delete")
                .expect("orphan source")
                .is_none()
        );
        assert!(
            memory
                .source("source-shared")
                .expect("shared source")
                .is_some()
        );
        assert!(
            memory
                .search_active_facts("forget", 10)
                .expect("deleted fts search")
                .is_empty()
        );
        let edge_count: i64 = memory
            .connection
            .query_row(
                "SELECT COUNT(*)
                 FROM memory_edges
                 WHERE source_fact_id = 'fact-delete' OR target_fact_id = 'fact-delete'",
                [],
                |row| row.get(0),
            )
            .expect("edge count");
        assert_eq!(edge_count, 0);
    }

    #[test]
    fn related_active_facts_expands_edges_without_returning_inactive_targets() {
        let profile = tempfile::tempdir().expect("profile");
        let mut memory =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");

        memory
            .insert_source(NewMemorySource {
                id: "source-1",
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Manual note",
                content: "Related memory source.",
                metadata_json: "{}",
            })
            .expect("source insert");
        for (id, status, fact) in [
            ("fact-seed", MemoryStatus::Active, "Seed memory fact."),
            ("fact-related", MemoryStatus::Active, "Related memory fact."),
            (
                "fact-superseded",
                MemoryStatus::Superseded,
                "Superseded related fact.",
            ),
        ] {
            memory
                .insert_fact(NewMemoryFact {
                    id,
                    scope: MemoryScope::Global,
                    chat_id: None,
                    status,
                    kind: MemoryKind::ProjectFact,
                    fact,
                    confidence: Some(0.8),
                    pinned: false,
                    source_ids: &["source-1"],
                    metadata_json: "{}",
                })
                .expect("fact insert");
        }
        for (edge_id, target_id) in [
            ("edge-related", "fact-related"),
            ("edge-superseded", "fact-superseded"),
        ] {
            memory
                .insert_edge(NewMemoryEdge {
                    id: edge_id,
                    source_fact_id: "fact-seed",
                    target_fact_id: target_id,
                    relation: MemoryRelationKind::Extends,
                    metadata_json: "{}",
                })
                .expect("edge insert");
        }

        let related = memory
            .related_active_facts(&["fact-seed".to_string()], 1, 10)
            .expect("related facts");

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].id, "fact-related");
    }

    #[test]
    fn fact_enabled_toggle_is_persistent_and_runtime_queries_exclude_disabled() {
        let profile = tempfile::tempdir().expect("profile");
        let mut memory =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        memory
            .insert_source(NewMemorySource {
                id: "source-enabled",
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Test",
                content: "Runtime enabled fact",
                metadata_json: "{}",
            })
            .expect("source insert");
        memory
            .insert_fact(NewMemoryFact {
                id: "fact-enabled",
                scope: MemoryScope::Global,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::Preference,
                fact: "Runtime enabled fact",
                confidence: Some(0.9),
                pinned: true,
                source_ids: &["source-enabled"],
                metadata_json: "{}",
            })
            .expect("fact insert");

        memory
            .insert_fact(NewMemoryFact {
                id: "fact-related-enabled",
                scope: MemoryScope::Global,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::Preference,
                fact: "Related enabled fact",
                confidence: Some(0.8),
                pinned: false,
                source_ids: &["source-enabled"],
                metadata_json: "{}",
            })
            .expect("related enabled fact insert");
        memory
            .insert_edge(NewMemoryEdge {
                id: "edge-related-enabled",
                source_fact_id: "fact-enabled",
                target_fact_id: "fact-related-enabled",
                relation: MemoryRelationKind::Extends,
                metadata_json: "{}",
            })
            .expect("related edge insert");
        memory
            .set_fact_enabled("fact-related-enabled", false)
            .expect("disable related fact");

        assert!(memory.fact("fact-enabled").unwrap().unwrap().enabled);
        assert!(
            !memory
                .set_fact_enabled("fact-enabled", false)
                .unwrap()
                .enabled
        );
        assert!(
            !memory
                .set_fact_enabled("fact-enabled", false)
                .unwrap()
                .enabled
        );
        assert_eq!(
            memory
                .list_facts_for_scope(None, MemoryStatus::Active, None, None, 10)
                .unwrap()
                .len(),
            2
        );
        assert!(
            memory
                .list_enabled_active_facts_for_scope(None, 10)
                .unwrap()
                .is_empty()
        );
        assert!(
            memory
                .search_enabled_active_facts_for_scope("runtime", None, None, 10)
                .unwrap()
                .is_empty()
        );
        assert!(
            memory
                .related_enabled_active_facts(&["fact-enabled".to_string()], 1, 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            memory
                .related_active_facts(&["fact-enabled".to_string()], 1, 10)
                .unwrap()
                .len(),
            1
        );
        assert!(
            memory
                .refresh_profile_from_active_facts(MemoryScope::Global, None, 10)
                .unwrap()
                .is_none()
        );
        assert!(
            memory
                .set_fact_enabled("fact-enabled", true)
                .unwrap()
                .enabled
        );
        assert_eq!(
            memory
                .list_enabled_active_facts_for_scope(None, 10)
                .unwrap()
                .len(),
            1
        );
        assert!(
            memory
                .set_fact_enabled("missing-fact", false)
                .unwrap_err()
                .to_string()
                .contains("was not found")
        );

        drop(memory);
        let reopened = MemoryDatabase::open_or_create_global(profile.path()).expect("reopen");
        assert!(reopened.fact("fact-enabled").unwrap().unwrap().enabled);
    }

    #[test]
    fn promote_fact_inherits_disabled_state() {
        let profile = tempfile::tempdir().expect("profile");
        let mut memory =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        memory
            .insert_source(NewMemorySource {
                id: "source-promote-disabled",
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Test",
                content: "Disabled source fact",
                metadata_json: "{}",
            })
            .expect("source insert");
        memory
            .insert_fact(NewMemoryFact {
                id: "fact-promote-disabled",
                scope: MemoryScope::Global,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::Preference,
                fact: "Disabled source fact",
                confidence: Some(0.9),
                pinned: false,
                source_ids: &["source-promote-disabled"],
                metadata_json: "{}",
            })
            .expect("fact insert");
        memory
            .set_fact_enabled("fact-promote-disabled", false)
            .expect("disable source fact");

        let promoted = memory
            .promote_fact(
                "fact-promote-disabled",
                "fact-promoted-disabled",
                MemoryScope::Global,
                None,
            )
            .expect("promote fact");
        assert!(!promoted.enabled);
    }

    #[test]
    fn updates_relation_inherits_disabled_state_from_previous_fact() {
        let profile = tempfile::tempdir().expect("profile");
        let mut memory =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        memory
            .insert_source(NewMemorySource {
                id: "source-update-disabled",
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Test",
                content: "Disabled update chain",
                metadata_json: "{}",
            })
            .expect("source insert");
        for (id, fact) in [
            ("fact-update-old", "Old disabled fact"),
            ("fact-update-new", "New replacement fact"),
        ] {
            memory
                .insert_fact(NewMemoryFact {
                    id,
                    scope: MemoryScope::Global,
                    chat_id: None,
                    status: MemoryStatus::Active,
                    kind: MemoryKind::ProjectFact,
                    fact,
                    confidence: Some(0.9),
                    pinned: false,
                    source_ids: &["source-update-disabled"],
                    metadata_json: "{}",
                })
                .expect("fact insert");
        }
        memory
            .set_fact_enabled("fact-update-old", false)
            .expect("disable previous fact");
        memory
            .insert_edge(NewMemoryEdge {
                id: "edge-update-disabled",
                source_fact_id: "fact-update-new",
                target_fact_id: "fact-update-old",
                relation: MemoryRelationKind::Updates,
                metadata_json: "{}",
            })
            .expect("updates edge");

        let old = memory.fact("fact-update-old").unwrap().unwrap();
        let new = memory.fact("fact-update-new").unwrap().unwrap();
        assert!(!old.enabled);
        assert_eq!(old.status, MemoryStatus::Superseded.as_str());
        assert!(!new.enabled);
        assert!(new.is_latest);
    }

    #[test]
    fn memory_list_and_count_query_plans_use_scope_status_indexes() {
        // Global high-volume list/count path (no search term).
        let profile = tempfile::tempdir().expect("profile");
        let database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("global memory database");
        drop(database);

        let connection =
            Connection::open(global_memory_database_path(profile.path())).expect("open global");
        connection.execute_batch("BEGIN;").expect("begin");
        {
            let mut insert = connection
                .prepare(
                    "INSERT INTO memory_facts (
                        id, scope, chat_id, status, kind, fact, confidence, pinned, enabled,
                        is_latest, expires_at, metadata_json, created_at, updated_at
                     ) VALUES (?1, 'global', NULL, ?2, ?3, ?4, 0.9, 0, 1, 1, NULL, '{}', ?5, ?5)",
                )
                .expect("prepare fact insert");
            for index in 0..12_000 {
                let status = match index % 11 {
                    0 => "pending",
                    1 => "superseded",
                    2 => "expired",
                    _ => "active",
                };
                let kind = match index % 3 {
                    0 => "preference",
                    1 => "project_fact",
                    _ => "procedure",
                };
                let created_at = format!("2026-06-01T{:02}:00:00.000Z", index % 24);
                insert
                    .execute(params![
                        format!("fact-global-{index}"),
                        status,
                        kind,
                        format!("Global fact body {index}"),
                        created_at,
                    ])
                    .expect("fact insert");
            }
        }
        connection.execute_batch("COMMIT;").expect("commit");
        drop(connection);

        let database =
            MemoryDatabase::open_or_create_global(profile.path()).expect("reopen global");
        let page = database
            .list_facts_for_scope_page(None, MemoryStatus::Active, None, None, 50, 0)
            .expect("global page");
        assert_eq!(page.len(), 50);
        let count = database
            .count_facts_for_scope(None, MemoryStatus::Active, None, None)
            .expect("global count");
        assert!(count as usize >= page.len());

        let connection = Connection::open(database.database_path()).expect("open global");
        // Production-homologous Global list SQL with representative binds (no search term).
        let list_sql = explain_sql_with_numbered_binds(
            &memory_facts_list_page_sql("scope = 'global'", false),
            &[(2, "50"), (3, "active"), (4, "NULL"), (5, "NULL"), (6, "0")],
        );
        let list_plan = explain_query_plan(&connection, &list_sql);
        assert!(
            plan_uses_index(&list_plan, "memory_facts_scope_status_idx")
                || plan_uses_index(&list_plan, "memory_facts_status_updated_idx")
                || plan_uses_index(&list_plan, "memory_facts_latest_idx"),
            "global list without query should use existing memory_facts indexes, plan:\n{list_plan}"
        );
        assert!(
            !plan_has_unconstrained_scan_on(&list_plan, "memory_facts"),
            "global list should not unconstrained-scan memory_facts, plan:\n{list_plan}"
        );

        let count_sql = explain_sql_with_numbered_binds(
            &memory_facts_count_sql("scope = 'global'"),
            &[(2, "active"), (3, "NULL"), (4, "NULL")],
        );
        let count_plan = explain_query_plan(&connection, &count_sql);
        assert!(
            plan_uses_index(&count_plan, "memory_facts_scope_status_idx")
                || plan_uses_index(&count_plan, "memory_facts_status_updated_idx")
                || plan_uses_index(&count_plan, "memory_facts_latest_idx"),
            "global count without query should use existing memory_facts indexes, plan:\n{count_plan}"
        );
        assert!(
            !plan_has_unconstrained_scan_on(&count_plan, "memory_facts"),
            "global count should not unconstrained-scan memory_facts, plan:\n{count_plan}"
        );

        // User-triggered substring search is an accepted interactive scan exception
        // (leading-wildcard LIKE); do not add ordinary B-tree indexes for '%query%'.
        let search_sql = explain_sql_with_numbered_binds(
            &memory_facts_list_page_sql("scope = 'global'", false),
            &[
                (2, "20"),
                (3, "active"),
                (4, "NULL"),
                (5, "%fact body 1%"),
                (6, "0"),
            ],
        );
        let search_plan = explain_query_plan(&connection, &search_sql);
        assert!(
            !search_plan.is_empty(),
            "fact substring search plan should exist for interactive exception documentation"
        );
        let _accepted_fact_substring_scan =
            plan_has_unconstrained_scan_on(&search_plan, "memory_facts");

        // Workspace scope path with chat OR workspace filter.
        let workspace = tempfile::tempdir().expect("workspace");
        let _ws =
            WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace db");
        let workspace_memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory");
        drop(workspace_memory);

        let connection =
            Connection::open(workspace_database_path(workspace.path())).expect("open workspace");
        connection
            .execute(
                "INSERT INTO chats (id, title, created_at, updated_at, archived_at, metadata_json)
                 VALUES ('chat-mem', 'Mem', '2026-06-01T00:00:00.000Z', '2026-06-01T00:00:00.000Z', NULL, '{}')",
                [],
            )
            .expect("chat insert");
        connection.execute_batch("BEGIN;").expect("begin ws");
        {
            let mut insert = connection
                .prepare(
                    "INSERT INTO memory_facts (
                        id, scope, chat_id, status, kind, fact, confidence, pinned, enabled,
                        is_latest, expires_at, metadata_json, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 'active', 'project_fact', ?4, 0.9, 0, 1, 1, NULL, '{}', ?5, ?5)",
                )
                .expect("prepare ws fact insert");
            for index in 0..10_000 {
                let (scope, chat_id): (&str, Option<&str>) = if index % 4 == 0 {
                    ("chat", Some("chat-mem"))
                } else {
                    ("workspace", None)
                };
                insert
                    .execute(params![
                        format!("fact-ws-{index}"),
                        scope,
                        chat_id,
                        format!("Workspace fact {index}"),
                        format!("2026-06-02T{:02}:00:00.000Z", index % 24),
                    ])
                    .expect("ws fact insert");
            }
        }
        connection.execute_batch("COMMIT;").expect("commit ws");
        drop(connection);

        let workspace_memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("reopen workspace memory");
        let ws_page = workspace_memory
            .list_facts_for_scope_page(Some("chat-mem"), MemoryStatus::Active, None, None, 30, 0)
            .expect("workspace+chat page");
        assert_eq!(ws_page.len(), 30);

        let connection =
            Connection::open(workspace_database_path(workspace.path())).expect("open ws plan");
        let (ws_filter, _) =
            memory_facts_scope_filter_sql(MemoryDatabaseKind::Workspace, Some("chat-mem"));
        let ws_list_sql = explain_sql_with_numbered_binds(
            &memory_facts_list_page_sql(ws_filter, false),
            &[
                (1, "chat-mem"),
                (2, "30"),
                (3, "active"),
                (4, "NULL"),
                (5, "NULL"),
                (6, "0"),
            ],
        );
        let ws_plan = explain_query_plan(&connection, &ws_list_sql);
        assert!(
            plan_uses_index(&ws_plan, "memory_facts_scope_status_idx")
                || plan_uses_index(&ws_plan, "memory_facts_chat_status_idx")
                || plan_uses_index(&ws_plan, "memory_facts_status_updated_idx")
                || plan_uses_index(&ws_plan, "memory_facts_latest_idx"),
            "workspace/chat list should use existing named indexes, plan:\n{ws_plan}"
        );
        assert!(
            !plan_has_unconstrained_scan_on(&ws_plan, "memory_facts"),
            "workspace/chat list should not unconstrained-scan memory_facts, plan:\n{ws_plan}"
        );
        // Decision: no new memory_facts composite index; existing scope/status/latest indexes suffice.
    }

    #[test]
    fn sources_for_facts_batches_and_profile_refresh_avoids_n_plus_one() {
        let workspace = tempfile::tempdir().expect("workspace");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace db");
            workspace_database
                .insert_chat("chat-batch", "Batch memory")
                .expect("chat insert");
        }
        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory");

        for index in 0..40 {
            let source_id = format!("source-batch-{index}");
            let fact_id = format!("fact-batch-{index}");
            let source_content = format!("Source content {index}");
            let fact_text = format!("Batch fact {index}");
            memory
                .insert_source(NewMemorySource {
                    id: &source_id,
                    scope: MemoryScope::Chat,
                    chat_id: Some("chat-batch"),
                    source_type: MemorySourceType::ManualNote,
                    source_id: None,
                    title: "Batch source",
                    content: &source_content,
                    metadata_json: "{}",
                })
                .expect("source insert");
            memory
                .insert_fact(NewMemoryFact {
                    id: &fact_id,
                    scope: MemoryScope::Chat,
                    chat_id: Some("chat-batch"),
                    status: MemoryStatus::Active,
                    kind: MemoryKind::ProjectFact,
                    fact: &fact_text,
                    confidence: Some(0.8),
                    pinned: index % 5 == 0,
                    source_ids: &[source_id.as_str()],
                    metadata_json: "{}",
                })
                .expect("fact insert");
        }

        let fact_ids = (0..40)
            .map(|index| format!("fact-batch-{index}"))
            .collect::<Vec<_>>();
        let fact_id_refs = fact_ids.iter().map(String::as_str).collect::<Vec<_>>();
        let sources_by_fact = memory
            .sources_for_facts(&fact_id_refs)
            .expect("batch sources");
        assert_eq!(sources_by_fact.len(), 40);
        for index in 0..40 {
            let sources = sources_by_fact
                .get(&format!("fact-batch-{index}"))
                .expect("fact sources");
            assert_eq!(sources.len(), 1);
            assert_eq!(sources[0].id, format!("source-batch-{index}"));
        }

        let profile = memory
            .refresh_profile_from_active_facts(MemoryScope::Chat, Some("chat-batch"), 40)
            .expect("profile refresh")
            .expect("profile row");
        let metadata: serde_json::Value =
            serde_json::from_str(&profile.metadata_json).expect("profile metadata");
        assert_eq!(metadata["sourceFactCount"], 40);
        assert_eq!(metadata["sourceLinks"].as_array().expect("links").len(), 40);
        // Profile stores sourceLinks for all facts from one sources_for_facts batch, not N+1.
        for link in metadata["sourceLinks"].as_array().expect("links") {
            assert_eq!(link["sourceIds"].as_array().expect("ids").len(), 1);
        }

        let connection = Connection::open(memory.database_path()).expect("open for explain");
        // Homologous to sources_for_facts JOIN + IN list (PK/autoindex on fact_sources).
        let sources_sql = "SELECT fs.fact_id, s.id
             FROM memory_sources s
             JOIN memory_fact_sources fs ON fs.source_id = s.id
             WHERE fs.fact_id IN ('fact-batch-0','fact-batch-1','fact-batch-2')
             ORDER BY fs.fact_id ASC, s.created_at ASC, s.id ASC";
        let sources_plan = explain_query_plan(&connection, sources_sql);
        assert!(
            plan_uses_index(&sources_plan, "sqlite_autoindex_memory_fact_sources_1")
                || plan_uses_index(&sources_plan, "memory_fact_sources_source_idx")
                || plan_uses_index(&sources_plan, "sqlite_autoindex_memory_sources_1"),
            "sources_for_facts batch join should use fact_sources/source keys, plan:\n{sources_plan}"
        );
        assert!(
            !plan_has_unconstrained_scan_on(&sources_plan, "memory_fact_sources"),
            "sources batch should not unconstrained-scan memory_fact_sources, plan:\n{sources_plan}"
        );
    }

    fn explain_query_plan(connection: &Connection, sql: &str) -> String {
        let mut statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .expect("prepare explain");
        let rows = statement
            .query_map([], |row| {
                Ok(format!(
                    "{}|{}|{}|{}",
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?
                ))
            })
            .expect("explain rows");
        rows.map(|row| row.expect("explain row"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Replace `?N` placeholders by number (desc) so EXPLAIN mirrors production bind semantics.
    fn explain_sql_with_numbered_binds(sql: &str, binds: &[(u32, &str)]) -> String {
        let mut ordered = binds.to_vec();
        ordered.sort_by(|left, right| right.0.cmp(&left.0));
        let mut result = sql.to_string();
        for (index, value) in ordered {
            let needle = format!("?{index}");
            let replacement =
                if value == "NULL" || value.chars().all(|ch| ch.is_ascii_digit() || ch == '-') {
                    value.to_string()
                } else {
                    format!("'{}'", value.replace('\'', "''"))
                };
            result = result.replace(&needle, &replacement);
        }
        result
    }

    fn plan_uses_index(plan: &str, index_name: &str) -> bool {
        for line in plan.lines() {
            let detail = line
                .rsplit_once('|')
                .map(|(_, detail)| detail.trim())
                .unwrap_or(line.trim());
            if detail.contains(index_name)
                && (detail.contains("USING INDEX")
                    || detail.contains("USING COVERING INDEX")
                    || detail.contains("SEARCH"))
            {
                return true;
            }
        }
        false
    }

    fn plan_has_unconstrained_scan_on(plan: &str, table: &str) -> bool {
        for line in plan.lines() {
            let detail = line
                .rsplit_once('|')
                .map(|(_, detail)| detail.trim())
                .unwrap_or(line.trim());
            if !detail.starts_with("SCAN ") {
                continue;
            }
            if detail.contains("USING INDEX ")
                || detail.contains("USING COVERING INDEX ")
                || detail.contains("USING INTEGER PRIMARY KEY")
                || detail.contains("USING ROWID")
                || detail.contains("CONSTANT ROW")
                || detail.contains("SUBQUERY")
                || detail.contains("AUTOMATIC")
            {
                continue;
            }
            let after_scan = detail.trim_start_matches("SCAN ").trim_start();
            if after_scan == table
                || after_scan.starts_with(&format!("{table} "))
                || after_scan.starts_with(&format!("{table}\t"))
            {
                return true;
            }
        }
        false
    }

    fn memory_column_definition(
        connection: &Connection,
        table: &str,
        column: &str,
    ) -> Option<(String, bool, Option<String>)> {
        let mut statement = connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table info statement");
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)? != 0,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .expect("table info rows");
        rows.filter_map(Result::ok)
            .find(|(name, _, _, _)| name == column)
            .map(|(_, data_type, not_null, default_value)| (data_type, not_null, default_value))
    }

    fn memory_table_exists(connection: &Connection, table: &str) -> bool {
        connection
            .query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1
                 )",
                [table],
                |row| row.get::<_, bool>(0),
            )
            .expect("table exists query")
    }
}
