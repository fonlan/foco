use std::{
    collections::HashSet,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use chrono::{SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

pub const GLOBAL_MEMORY_DATABASE_FILE: &str = "memory.sqlite";
pub const GLOBAL_MEMORY_SCHEMA_VERSION: u32 = 1;

const GLOBAL_MEMORY_MIGRATIONS: &[MemoryMigration] = &[MemoryMigration {
    version: 1,
    sql: GLOBAL_MEMORY_SCHEMA_SQL,
}];

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryExtractionJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl MemoryExtractionJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryDatabaseError> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
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

    pub fn open_workspace_at(database_path: impl AsRef<Path>) -> Result<Self, MemoryDatabaseError> {
        let database_path = database_path.as_ref().to_path_buf();
        let connection = open_connection(&database_path)?;
        ensure_memory_schema_exists(&connection, &database_path)?;

        Ok(Self {
            database_path,
            connection,
            kind: MemoryDatabaseKind::Workspace,
        })
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn schema_version(&self) -> Result<u32, MemoryDatabaseError> {
        schema_version(&self.connection, &self.database_path)
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
        let linked_count: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM memory_fact_sources WHERE source_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        if linked_count > 0 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: format!("memory source '{id}' is still linked to {linked_count} fact(s)"),
            });
        }

        let deleted = self
            .connection
            .execute("DELETE FROM memory_sources WHERE id = ?1", params![id])
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(deleted > 0)
    }

    pub fn insert_fact(&mut self, fact: NewMemoryFact<'_>) -> Result<(), MemoryDatabaseError> {
        self.validate_scope(fact.scope)?;
        validate_fact(&fact)?;

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
            .transaction()
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
        let fact = self
            .fact(fact_id)?
            .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                message: format!("memory fact was not found: {fact_id}"),
            })?;
        let source_count = self.source_count_for_fact(fact_id)?;

        if fact.kind != MemoryKind::UserNote.as_str() && source_count <= 1 {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "non-user_note facts must keep at least one source".to_string(),
            });
        }

        let deleted = self
            .connection
            .execute(
                "DELETE FROM memory_fact_sources WHERE fact_id = ?1 AND source_id = ?2",
                params![fact_id, source_id],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(deleted > 0)
    }

    pub fn insert_edge(&mut self, edge: NewMemoryEdge<'_>) -> Result<(), MemoryDatabaseError> {
        validate_edge(&edge)?;
        if edge.relation == MemoryRelationKind::Updates
            && update_relation_would_cycle(
                &self.connection,
                &self.database_path,
                edge.source_fact_id,
                edge.target_fact_id,
            )?
        {
            return Err(MemoryDatabaseError::InvalidMemoryInput {
                message: "updates relation would create a cycle".to_string(),
            });
        }
        let now = now_timestamp();
        let database_path = self.database_path.clone();
        let transaction = self
            .connection
            .transaction()
            .map_err(|source| sqlite_error(&database_path, source))?;
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
        let mut source_links = Vec::new();
        for fact in &facts {
            let mut source_ids = self
                .sources_for_fact(&fact.id)?
                .into_iter()
                .map(|source| source.id)
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
            .transaction()
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
        self.validate_scope(job.scope)?;
        validate_extraction_job(&job)?;
        let now = now_timestamp();
        let input_json = redact_memory_json(job.input_json, "memory_extraction_jobs.input_json")?;
        let output_json =
            redact_optional_memory_json(job.output_json, "memory_extraction_jobs.output_json")?;

        self.connection
            .execute(
                "INSERT INTO memory_extraction_jobs
                    (id, scope, chat_id, status, model_id, input_json, output_json, error_message, created_at, started_at, completed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL)",
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
                     started_at = COALESCE(started_at, ?2),
                     completed_at = NULL,
                     error_message = NULL
                 WHERE id = ?1",
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
                 WHERE id = ?1",
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
                 WHERE id = ?1",
                params![id, output_json, error_message, now],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(changed > 0)
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

    pub fn fact(&self, id: &str) -> Result<Option<MemoryFactRecord>, MemoryDatabaseError> {
        require_non_empty("id", id)?;
        self.connection
            .query_row(
                "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest,
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
                "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest,
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
        let mut statement = self
            .connection
            .prepare(
                "SELECT s.id, s.scope, s.chat_id, s.source_type, s.source_id, s.title, s.content,
                        s.metadata_json, s.created_at, s.updated_at
                 FROM memory_sources s
                 JOIN memory_fact_sources fs ON fs.source_id = s.id
                 WHERE fs.fact_id = ?1
                 ORDER BY s.created_at ASC, s.id ASC",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(params![fact_id], memory_source_from_row)
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
                        f.pinned, f.is_latest, f.expires_at, f.metadata_json, f.created_at, f.updated_at
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
        limit: u32,
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
        let sql = format!(
            "SELECT f.id, f.scope, f.chat_id, f.status, f.kind, f.fact, f.confidence,
                    f.pinned, f.is_latest, f.expires_at, f.metadata_json, f.created_at, f.updated_at
             FROM memory_fts_index
             JOIN memory_facts f ON f.id = memory_fts_index.fact_id
             WHERE memory_fts_index MATCH ?1
               AND ({filter_sql})
               AND f.status = 'active'
               AND f.is_latest = 1
             ORDER BY
               CASE WHEN f.scope = 'chat' THEN 0 WHEN f.scope = 'workspace' THEN 1 ELSE 2 END,
               bm25(memory_fts_index),
               f.pinned DESC,
               f.updated_at DESC,
               COALESCE(f.confidence, -1.0) DESC,
               f.is_latest DESC,
               f.id ASC
             LIMIT ?3"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(params![query, chat_param, limit], memory_fact_from_row)
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn related_active_facts(
        &self,
        seed_fact_ids: &[String],
        max_depth: u32,
        limit: u32,
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
                    if let Some(fact) = self.active_latest_fact(&neighbor_id)? {
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
        self.list_facts_for_scope(chat_id, MemoryStatus::Active, limit)
    }

    pub fn list_facts_for_scope(
        &self,
        chat_id: Option<&str>,
        status: MemoryStatus,
        limit: u32,
    ) -> Result<Vec<MemoryFactRecord>, MemoryDatabaseError> {
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
            "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest,
                    expires_at, metadata_json, created_at, updated_at
             FROM memory_facts
             WHERE ({filter_sql})
               AND status = ?3
               AND is_latest = 1
             ORDER BY
               CASE WHEN scope = 'chat' THEN 0 WHEN scope = 'workspace' THEN 1 ELSE 2 END,
               pinned DESC,
               updated_at DESC
             LIMIT ?2"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(
                params![chat_param, limit, status.as_str()],
                memory_fact_from_row,
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
                "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest,
                        expires_at, metadata_json, created_at, updated_at
                 FROM memory_facts
                 WHERE scope = ?1
                   AND ((?2 IS NULL AND chat_id IS NULL) OR chat_id = ?2)
                   AND status = 'active'
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
            kind: memory_kind_from_str(&fact.kind)?,
            fact: &fact.fact,
            confidence: fact.confidence,
            pinned: fact.pinned,
            source_ids: &promoted_source_refs,
            metadata_json: &fact.metadata_json,
        })?;

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
            kind: memory_kind_from_str(&fact.kind)?,
            fact: &fact.fact,
            confidence: fact.confidence,
            pinned: fact.pinned,
            source_ids: &promoted_source_refs,
            metadata_json: &fact.metadata_json,
        })?;

        target
            .fact(promoted_fact_id)?
            .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
                message: format!("promoted memory fact was not found: {promoted_fact_id}"),
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemoryDatabaseKind {
    Global,
    Workspace,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemorySource<'a> {
    pub id: &'a str,
    pub scope: MemoryScope,
    pub chat_id: Option<&'a str>,
    pub source_type: MemorySourceType,
    pub source_id: Option<&'a str>,
    pub title: &'a str,
    pub content: &'a str,
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpdateMemorySource<'a> {
    pub id: &'a str,
    pub title: Option<&'a str>,
    pub content: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemoryFact<'a> {
    pub id: &'a str,
    pub scope: MemoryScope,
    pub chat_id: Option<&'a str>,
    pub status: MemoryStatus,
    pub kind: MemoryKind,
    pub fact: &'a str,
    pub confidence: Option<f64>,
    pub pinned: bool,
    pub source_ids: &'a [&'a str],
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpdateMemoryFact<'a> {
    pub id: &'a str,
    pub scope: Option<MemoryScope>,
    pub chat_id: Option<&'a str>,
    pub status: Option<MemoryStatus>,
    pub kind: Option<MemoryKind>,
    pub fact: Option<&'a str>,
    pub confidence: Option<f64>,
    pub pinned: Option<bool>,
    pub is_latest: Option<bool>,
    pub expires_at: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemoryEdge<'a> {
    pub id: &'a str,
    pub source_fact_id: &'a str,
    pub target_fact_id: &'a str,
    pub relation: MemoryRelationKind,
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemoryProfile<'a> {
    pub id: &'a str,
    pub scope: MemoryScope,
    pub chat_id: Option<&'a str>,
    pub profile_text: &'a str,
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemoryExtractionJob<'a> {
    pub id: &'a str,
    pub scope: MemoryScope,
    pub chat_id: Option<&'a str>,
    pub status: MemoryExtractionJobStatus,
    pub model_id: Option<&'a str>,
    pub input_json: &'a str,
    pub output_json: Option<&'a str>,
    pub error_message: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemorySourceRecord {
    pub id: String,
    pub scope: String,
    pub chat_id: Option<String>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub title: String,
    pub content: String,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemoryFactRecord {
    pub id: String,
    pub scope: String,
    pub chat_id: Option<String>,
    pub status: String,
    pub kind: String,
    pub fact: String,
    pub confidence: Option<f64>,
    pub pinned: bool,
    pub is_latest: bool,
    pub expires_at: Option<String>,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemoryProfileRecord {
    pub id: String,
    pub scope: String,
    pub chat_id: Option<String>,
    pub profile_text: String,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemoryExtractionJobRecord {
    pub id: String,
    pub scope: String,
    pub chat_id: Option<String>,
    pub status: String,
    pub model_id: Option<String>,
    pub input_json: String,
    pub output_json: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug)]
pub enum MemoryDatabaseError {
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
            Self::InvalidMemoryInput { .. }
            | Self::MissingDatabaseParent { .. }
            | Self::UnsupportedSchemaVersion { .. } => None,
        }
    }
}

pub fn global_memory_database_path(foco_root_dir: impl AsRef<Path>) -> PathBuf {
    foco_root_dir.as_ref().join(GLOBAL_MEMORY_DATABASE_FILE)
}

fn open_connection(database_path: &Path) -> Result<Connection, MemoryDatabaseError> {
    let connection =
        Connection::open(database_path).map_err(|source| MemoryDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;

    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|source| MemoryDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|source| MemoryDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;

    Ok(connection)
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

    let transaction = connection
        .transaction()
        .map_err(|source| sqlite_error(database_path, source))?;

    for migration in GLOBAL_MEMORY_MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current_version)
    {
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

fn require_non_empty(field: &str, value: &str) -> Result<(), MemoryDatabaseError> {
    if value.trim().is_empty() {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("{field} must not be empty"),
        });
    }

    Ok(())
}

fn validate_json(field: &'static str, value: &str) -> Result<(), MemoryDatabaseError> {
    serde_json::from_str::<Value>(value)
        .map(|_| ())
        .map_err(|source| MemoryDatabaseError::InvalidMemoryJson { field, source })
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
        is_latest: row.get::<_, i64>(8)? != 0,
        expires_at: row.get(9)?,
        metadata_json: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
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

fn fact_by_id(
    transaction: &Transaction<'_>,
    database_path: &Path,
    id: &str,
) -> Result<MemoryFactRecord, MemoryDatabaseError> {
    transaction
        .query_row(
            "SELECT id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest,
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

fn promoted_source_id(promoted_fact_id: &str, index: usize) -> String {
    format!("{promoted_fact_id}:source:{index}")
}

fn promoted_source_ids(promoted_fact_id: &str, count: usize) -> Vec<String> {
    (0..count)
        .map(|index| promoted_source_id(promoted_fact_id, index))
        .collect()
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
    database_path: &Path,
) -> Result<Vec<T>, MemoryDatabaseError> {
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|source| sqlite_error(database_path, source))
}

fn create_directory(path: &Path) -> Result<(), MemoryDatabaseError> {
    fs::create_dir_all(path).map_err(|source| MemoryDatabaseError::Io {
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

fn now_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn bool_to_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

struct MemoryMigration {
    version: u32,
    sql: &'static str,
}

pub const WORKSPACE_MEMORY_SCHEMA_SQL: &str = r#"
CREATE TABLE memory_sources (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope IN ('workspace', 'chat')),
    chat_id TEXT CHECK (chat_id IS NULL OR length(chat_id) > 0),
    source_type TEXT NOT NULL CHECK (source_type IN ('chat_message', 'assistant_message', 'tool_call', 'tool_result', 'context_snapshot', 'manual_note', 'imported_document')),
    source_id TEXT CHECK (source_id IS NULL OR length(source_id) > 0),
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL CHECK (length(content) > 0),
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (scope != 'chat' OR chat_id IS NOT NULL)
);

CREATE INDEX memory_sources_scope_idx ON memory_sources (scope);
CREATE INDEX memory_sources_chat_idx ON memory_sources (chat_id);
CREATE INDEX memory_sources_type_ref_idx ON memory_sources (source_type, source_id);
CREATE INDEX memory_sources_updated_idx ON memory_sources (updated_at);

CREATE TABLE memory_facts (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope IN ('workspace', 'chat')),
    chat_id TEXT REFERENCES chats(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'superseded', 'expired', 'rejected')),
    kind TEXT NOT NULL CHECK (kind IN ('preference', 'project_fact', 'project_decision', 'procedure', 'constraint', 'episode', 'user_note')),
    fact TEXT NOT NULL CHECK (length(fact) > 0),
    confidence REAL CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
    pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    is_latest INTEGER NOT NULL DEFAULT 1 CHECK (is_latest IN (0, 1)),
    expires_at TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK ((scope = 'chat' AND chat_id IS NOT NULL) OR (scope = 'workspace' AND chat_id IS NULL))
);

CREATE INDEX memory_facts_scope_status_idx ON memory_facts (scope, status);
CREATE INDEX memory_facts_chat_status_idx ON memory_facts (chat_id, status);
CREATE INDEX memory_facts_status_updated_idx ON memory_facts (status, updated_at);
CREATE INDEX memory_facts_kind_idx ON memory_facts (kind);
CREATE INDEX memory_facts_latest_idx ON memory_facts (is_latest);

CREATE TABLE memory_fact_sources (
    fact_id TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES memory_sources(id) ON DELETE CASCADE,
    PRIMARY KEY (fact_id, source_id)
);

CREATE INDEX memory_fact_sources_source_idx ON memory_fact_sources (source_id);

CREATE TABLE memory_edges (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    source_fact_id TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    target_fact_id TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    relation TEXT NOT NULL CHECK (relation IN ('updates', 'extends', 'derives')),
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    CHECK (source_fact_id <> target_fact_id)
);

CREATE INDEX memory_edges_source_relation_idx ON memory_edges (source_fact_id, relation);
CREATE INDEX memory_edges_target_relation_idx ON memory_edges (target_fact_id, relation);

CREATE TABLE memory_fts_data (
    id INTEGER PRIMARY KEY,
    fact_id TEXT NOT NULL UNIQUE REFERENCES memory_facts(id) ON DELETE CASCADE,
    scope TEXT NOT NULL,
    chat_id TEXT,
    status TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE VIRTUAL TABLE memory_fts_index USING fts5(
    fact_id UNINDEXED,
    scope UNINDEXED,
    chat_id UNINDEXED,
    title,
    body
);

CREATE TRIGGER memory_fts_data_after_insert
AFTER INSERT ON memory_fts_data
BEGIN
    INSERT INTO memory_fts_index(rowid, fact_id, scope, chat_id, title, body)
    VALUES (new.id, new.fact_id, new.scope, new.chat_id, new.title, new.body);
END;

CREATE TRIGGER memory_fts_data_after_update
AFTER UPDATE ON memory_fts_data
BEGIN
    DELETE FROM memory_fts_index WHERE rowid = old.id;
    INSERT INTO memory_fts_index(rowid, fact_id, scope, chat_id, title, body)
    VALUES (new.id, new.fact_id, new.scope, new.chat_id, new.title, new.body);
END;

CREATE TRIGGER memory_fts_data_after_delete
AFTER DELETE ON memory_fts_data
BEGIN
    DELETE FROM memory_fts_index WHERE rowid = old.id;
END;

CREATE TABLE memory_profiles (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope IN ('workspace', 'chat')),
    chat_id TEXT REFERENCES chats(id) ON DELETE CASCADE,
    profile_text TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK ((scope = 'chat' AND chat_id IS NOT NULL) OR (scope = 'workspace' AND chat_id IS NULL))
);

CREATE INDEX memory_profiles_scope_idx ON memory_profiles (scope);
CREATE INDEX memory_profiles_chat_idx ON memory_profiles (chat_id);

CREATE TABLE memory_extraction_jobs (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope IN ('workspace', 'chat')),
    chat_id TEXT REFERENCES chats(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    model_id TEXT CHECK (model_id IS NULL OR length(model_id) > 0),
    input_json TEXT NOT NULL,
    output_json TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    CHECK ((scope = 'chat' AND chat_id IS NOT NULL) OR (scope = 'workspace' AND chat_id IS NULL))
);

CREATE INDEX memory_extraction_jobs_scope_status_idx ON memory_extraction_jobs (scope, status);
CREATE INDEX memory_extraction_jobs_chat_idx ON memory_extraction_jobs (chat_id);
CREATE INDEX memory_extraction_jobs_created_idx ON memory_extraction_jobs (created_at);
"#;

pub const GLOBAL_MEMORY_SCHEMA_SQL: &str = r#"
CREATE TABLE memory_sources (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope = 'global'),
    chat_id TEXT CHECK (chat_id IS NULL),
    source_type TEXT NOT NULL CHECK (source_type IN ('chat_message', 'assistant_message', 'tool_call', 'tool_result', 'context_snapshot', 'manual_note', 'imported_document')),
    source_id TEXT CHECK (source_id IS NULL OR length(source_id) > 0),
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL CHECK (length(content) > 0),
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX memory_sources_scope_idx ON memory_sources (scope);
CREATE INDEX memory_sources_type_ref_idx ON memory_sources (source_type, source_id);
CREATE INDEX memory_sources_updated_idx ON memory_sources (updated_at);

CREATE TABLE memory_facts (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope = 'global'),
    chat_id TEXT CHECK (chat_id IS NULL),
    status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'superseded', 'expired', 'rejected')),
    kind TEXT NOT NULL CHECK (kind IN ('preference', 'project_fact', 'project_decision', 'procedure', 'constraint', 'episode', 'user_note')),
    fact TEXT NOT NULL CHECK (length(fact) > 0),
    confidence REAL CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
    pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    is_latest INTEGER NOT NULL DEFAULT 1 CHECK (is_latest IN (0, 1)),
    expires_at TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX memory_facts_scope_status_idx ON memory_facts (scope, status);
CREATE INDEX memory_facts_status_updated_idx ON memory_facts (status, updated_at);
CREATE INDEX memory_facts_kind_idx ON memory_facts (kind);
CREATE INDEX memory_facts_latest_idx ON memory_facts (is_latest);

CREATE TABLE memory_fact_sources (
    fact_id TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES memory_sources(id) ON DELETE CASCADE,
    PRIMARY KEY (fact_id, source_id)
);

CREATE INDEX memory_fact_sources_source_idx ON memory_fact_sources (source_id);

CREATE TABLE memory_edges (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    source_fact_id TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    target_fact_id TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    relation TEXT NOT NULL CHECK (relation IN ('updates', 'extends', 'derives')),
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    CHECK (source_fact_id <> target_fact_id)
);

CREATE INDEX memory_edges_source_relation_idx ON memory_edges (source_fact_id, relation);
CREATE INDEX memory_edges_target_relation_idx ON memory_edges (target_fact_id, relation);

CREATE TABLE memory_fts_data (
    id INTEGER PRIMARY KEY,
    fact_id TEXT NOT NULL UNIQUE REFERENCES memory_facts(id) ON DELETE CASCADE,
    scope TEXT NOT NULL,
    chat_id TEXT,
    status TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE VIRTUAL TABLE memory_fts_index USING fts5(
    fact_id UNINDEXED,
    scope UNINDEXED,
    chat_id UNINDEXED,
    title,
    body
);

CREATE TRIGGER memory_fts_data_after_insert
AFTER INSERT ON memory_fts_data
BEGIN
    INSERT INTO memory_fts_index(rowid, fact_id, scope, chat_id, title, body)
    VALUES (new.id, new.fact_id, new.scope, new.chat_id, new.title, new.body);
END;

CREATE TRIGGER memory_fts_data_after_update
AFTER UPDATE ON memory_fts_data
BEGIN
    DELETE FROM memory_fts_index WHERE rowid = old.id;
    INSERT INTO memory_fts_index(rowid, fact_id, scope, chat_id, title, body)
    VALUES (new.id, new.fact_id, new.scope, new.chat_id, new.title, new.body);
END;

CREATE TRIGGER memory_fts_data_after_delete
AFTER DELETE ON memory_fts_data
BEGIN
    DELETE FROM memory_fts_index WHERE rowid = old.id;
END;

CREATE TABLE memory_profiles (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope = 'global'),
    chat_id TEXT CHECK (chat_id IS NULL),
    profile_text TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX memory_profiles_scope_idx ON memory_profiles (scope);

CREATE TABLE memory_extraction_jobs (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope = 'global'),
    chat_id TEXT CHECK (chat_id IS NULL),
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    model_id TEXT CHECK (model_id IS NULL OR length(model_id) > 0),
    input_json TEXT NOT NULL,
    output_json TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX memory_extraction_jobs_scope_status_idx ON memory_extraction_jobs (scope, status);
CREATE INDEX memory_extraction_jobs_created_idx ON memory_extraction_jobs (created_at);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{WorkspaceDatabase, workspace_database_path};

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
                WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
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
                WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
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
                .complete_extraction_job("job-1", r#"{"apiKey":"sk-secret","facts":[]}"#)
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
    fn workspace_memory_api_promotes_and_preserves_workspace_facts() {
        let workspace = tempfile::tempdir().expect("workspace");
        {
            let mut workspace_database =
                WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
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
            .search_active_facts_for_scope("workspace", Some("chat-1"), 10)
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
                WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
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
                WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
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
                WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
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
                WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
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
}
