use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use chrono::{SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;

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

pub struct MemoryDatabase {
    database_path: PathBuf,
    connection: Connection,
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
        })
    }

    pub fn open_or_create_global(
        foco_root_dir: impl AsRef<Path>,
    ) -> Result<Self, MemoryDatabaseError> {
        Self::open_or_create_global_at(global_memory_database_path(foco_root_dir))
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
        validate_global_scope(source.scope)?;
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

    pub fn insert_fact(&mut self, fact: NewMemoryFact<'_>) -> Result<(), MemoryDatabaseError> {
        validate_global_scope(fact.scope)?;
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

    pub fn insert_edge(&mut self, edge: NewMemoryEdge<'_>) -> Result<(), MemoryDatabaseError> {
        validate_edge(&edge)?;
        let now = now_timestamp();

        self.connection
            .execute(
                "INSERT INTO memory_edges
                    (id, source_fact_id, target_fact_id, relation, metadata_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    edge.id,
                    edge.source_fact_id,
                    edge.target_fact_id,
                    edge.relation.as_str(),
                    edge.metadata_json,
                    now,
                ],
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        Ok(())
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
                 ORDER BY bm25(memory_fts_index), f.pinned DESC, f.updated_at DESC
                 LIMIT ?2",
            )
            .map_err(|source| sqlite_error(&self.database_path, source))?;
        let rows = statement
            .query_map(params![query, limit], memory_fact_from_row)
            .map_err(|source| sqlite_error(&self.database_path, source))?;

        collect_rows(rows, &self.database_path)
    }
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

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemoryEdge<'a> {
    pub id: &'a str,
    pub source_fact_id: &'a str,
    pub target_fact_id: &'a str,
    pub relation: MemoryRelationKind,
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, PartialEq)]
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

fn validate_global_scope(scope: MemoryScope) -> Result<(), MemoryDatabaseError> {
    if scope != MemoryScope::Global {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!(
                "global memory database only accepts global scope, got '{}'",
                scope.as_str()
            ),
        });
    }

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
}
