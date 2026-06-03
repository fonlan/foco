use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use chrono::{SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::config::WorkspaceConfig;

pub const WORKSPACE_FOCO_DIR: &str = ".foco";
pub const WORKSPACE_DATABASE_FILE: &str = "foco.sqlite";
pub const WORKSPACE_SCHEMA_VERSION: u32 = 2;

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: MIGRATION_001,
    },
    Migration {
        version: 2,
        sql: MIGRATION_002,
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

pub struct WorkspaceDatabase {
    database_path: PathBuf,
    connection: Connection,
}

impl WorkspaceDatabase {
    pub fn open_or_create(
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

        Ok(Self {
            database_path,
            connection,
        })
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
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

    pub fn chat(&self, id: &str) -> Result<Option<ChatRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT id, title, created_at, updated_at, archived_at
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
                    })
                },
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn chats(&self) -> Result<Vec<ChatRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, title, created_at, updated_at, archived_at
                 FROM chats
                 ORDER BY updated_at DESC, created_at DESC, id DESC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], |row| {
                Ok(ChatRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    archived_at: row.get(4)?,
                })
            })
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn insert_message(
        &mut self,
        message: NewMessage<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let now = now_timestamp();
        let metadata_json = message.metadata_json.unwrap_or("{}");

        self.connection
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
            .map_err(|source| self.sqlite_error(source))?;

        self.connection
            .execute(
                "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
                params![now, message.chat_id],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn messages_for_chat(
        &self,
        chat_id: &str,
    ) -> Result<Vec<MessageRecord>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, chat_id, role, content, sequence, created_at, metadata_json
                 FROM messages
                 WHERE chat_id = ?1
                 ORDER BY sequence ASC",
            )
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

    pub fn insert_llm_request(
        &mut self,
        request: NewLlmRequest<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        validate_llm_request_tokens(&request)?;

        let cache_ratio = calculate_cache_ratio(request.input_tokens, request.cache_read_tokens)?;
        let request_body_json =
            redact_optional_audit_json(request.request_body_json, "request_body_json")?;
        let response_body_json =
            redact_optional_audit_json(request.response_body_json, "response_body_json")?;

        self.connection
            .execute(
                "INSERT INTO llm_requests
                    (
                        id, workspace_id, chat_id, provider_id, model_id, request_started_at,
                        first_token_at, completed_at, input_tokens, output_tokens,
                        cache_read_tokens, cache_write_tokens, cache_ratio,
                        first_token_latency_ms, total_latency_ms, status_code, final_state,
                        request_body_json, response_body_json
                    )
                 VALUES
                    (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    request.id,
                    request.workspace_id,
                    request.chat_id,
                    request.provider_id,
                    request.model_id,
                    request.request_started_at,
                    request.first_token_at,
                    request.completed_at,
                    request.input_tokens,
                    request.output_tokens,
                    request.cache_read_tokens,
                    request.cache_write_tokens,
                    cache_ratio,
                    request.first_token_latency_ms,
                    request.total_latency_ms,
                    request.status_code,
                    request.final_state,
                    request_body_json,
                    response_body_json
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn llm_request(
        &self,
        id: &str,
    ) -> Result<Option<LlmRequestRecord>, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT
                    id, workspace_id, chat_id, provider_id, model_id, request_started_at,
                    first_token_at, completed_at, input_tokens, output_tokens,
                    cache_read_tokens, cache_write_tokens, cache_ratio,
                    first_token_latency_ms, total_latency_ms, status_code, final_state,
                    request_body_json, response_body_json
                 FROM llm_requests
                 WHERE id = ?1",
                params![id],
                |row| {
                    Ok(LlmRequestRecord {
                        id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        chat_id: row.get(2)?,
                        provider_id: row.get(3)?,
                        model_id: row.get(4)?,
                        request_started_at: row.get(5)?,
                        first_token_at: row.get(6)?,
                        completed_at: row.get(7)?,
                        input_tokens: row.get(8)?,
                        output_tokens: row.get(9)?,
                        cache_read_tokens: row.get(10)?,
                        cache_write_tokens: row.get(11)?,
                        cache_ratio: row.get(12)?,
                        first_token_latency_ms: row.get(13)?,
                        total_latency_ms: row.get(14)?,
                        status_code: row.get(15)?,
                        final_state: row.get(16)?,
                        request_body_json: row.get(17)?,
                        response_body_json: row.get(18)?,
                    })
                },
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn insert_llm_request_event(
        &mut self,
        event: NewLlmRequestEvent<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let raw_chunk_json = redact_optional_audit_json(event.raw_chunk_json, "raw_chunk_json")?;
        let normalized_event_json =
            redact_audit_json(event.normalized_event_json, "normalized_event_json")?;

        self.connection
            .execute(
                "INSERT INTO llm_request_events
                    (
                        id, llm_request_id, sequence, event_at, event_type,
                        raw_chunk_json, normalized_event_json
                    )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    event.id,
                    event.llm_request_id,
                    event.sequence,
                    event.event_at,
                    event.event_type,
                    raw_chunk_json,
                    normalized_event_json
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
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

    fn sqlite_error(&self, source: rusqlite::Error) -> WorkspaceDatabaseError {
        WorkspaceDatabaseError::Sqlite {
            path: self.database_path.clone(),
            source,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatRecord {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
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
pub struct NewLlmRequest<'a> {
    pub id: &'a str,
    pub workspace_id: &'a str,
    pub chat_id: Option<&'a str>,
    pub provider_id: &'a str,
    pub model_id: &'a str,
    pub request_started_at: &'a str,
    pub first_token_at: Option<&'a str>,
    pub completed_at: Option<&'a str>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub status_code: Option<i64>,
    pub final_state: &'a str,
    pub request_body_json: Option<&'a str>,
    pub response_body_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LlmRequestRecord {
    pub id: String,
    pub workspace_id: Option<String>,
    pub chat_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub request_started_at: String,
    pub first_token_at: Option<String>,
    pub completed_at: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub cache_ratio: Option<f64>,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub status_code: Option<i64>,
    pub final_state: String,
    pub request_body_json: Option<String>,
    pub response_body_json: Option<String>,
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

#[derive(Debug)]
pub enum WorkspaceDatabaseError {
    InvalidAuditJson {
        field: &'static str,
        source: serde_json::Error,
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
    NonUtf8Path {
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
    WorkspaceNotDirectory {
        path: PathBuf,
    },
}

impl fmt::Display for WorkspaceDatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAuditJson { field, source } => {
                write!(formatter, "invalid LLM audit JSON in {field}: {source}")
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
            Self::NonUtf8Path { path } => {
                write!(formatter, "path must be valid UTF-8: {}", path.display())
            }
            Self::Sqlite { path, source } => {
                write!(formatter, "{} SQLite error: {}", path.display(), source)
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
            Self::InvalidAuditJson { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::Sqlite { source, .. } => Some(source),
            Self::InvalidAuditTokens { .. }
            | Self::MissingDatabaseParent { .. }
            | Self::NonUtf8Path { .. }
            | Self::UnsupportedSchemaVersion { .. }
            | Self::WorkspaceNotDirectory { .. } => None,
        }
    }
}

struct Migration {
    version: u32,
    sql: &'static str,
}

fn open_connection(database_path: &Path) -> Result<Connection, WorkspaceDatabaseError> {
    let connection =
        Connection::open(database_path).map_err(|source| WorkspaceDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;

    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|source| WorkspaceDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|source| WorkspaceDatabaseError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;

    Ok(connection)
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

    if database_existed && has_user_schema(connection, database_path)? {
        create_migration_backup(connection, database_path, current_version)?;
    }

    let transaction =
        connection
            .transaction()
            .map_err(|source| WorkspaceDatabaseError::Sqlite {
                path: database_path.to_path_buf(),
                source,
            })?;

    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current_version)
    {
        transaction.execute_batch(migration.sql).map_err(|source| {
            WorkspaceDatabaseError::Sqlite {
                path: database_path.to_path_buf(),
                source,
            }
        })?;
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

    Ok(())
}

fn create_directory(path: &Path) -> Result<(), WorkspaceDatabaseError> {
    fs::create_dir_all(path).map_err(|source| WorkspaceDatabaseError::Io {
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

fn validate_llm_request_tokens(request: &NewLlmRequest<'_>) -> Result<(), WorkspaceDatabaseError> {
    for (name, value) in [
        ("input_tokens", request.input_tokens),
        ("output_tokens", request.output_tokens),
        ("cache_read_tokens", request.cache_read_tokens),
        ("cache_write_tokens", request.cache_write_tokens),
    ] {
        if let Some(value) = value
            && value < 0
        {
            return Err(WorkspaceDatabaseError::InvalidAuditTokens {
                message: format!("{name} must be non-negative, got {value}"),
            });
        }
    }

    if let (Some(input_tokens), Some(cache_read_tokens)) =
        (request.input_tokens, request.cache_read_tokens)
    {
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

fn redact_audit_json(value: &str, field: &'static str) -> Result<String, WorkspaceDatabaseError> {
    let mut parsed: Value = serde_json::from_str(value)
        .map_err(|source| WorkspaceDatabaseError::InvalidAuditJson { field, source })?;

    redact_json_value(&mut parsed);

    serde_json::to_string(&parsed)
        .map_err(|source| WorkspaceDatabaseError::InvalidAuditJson { field, source })
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

    normalized == "authorization" || normalized.contains("apikey")
}

const MIGRATION_001: &str = r#"
CREATE TABLE workspace_metadata (
    key TEXT PRIMARY KEY NOT NULL CHECK (length(key) > 0),
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE chats (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    title TEXT NOT NULL CHECK (length(title) > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    archived_at TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
    content TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    created_at TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    UNIQUE (chat_id, sequence)
);

CREATE INDEX messages_chat_sequence_idx ON messages (chat_id, sequence);

CREATE TABLE run_events (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    run_id TEXT NOT NULL CHECK (length(run_id) > 0),
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    event_type TEXT NOT NULL CHECK (length(event_type) > 0),
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE (run_id, sequence)
);

CREATE INDEX run_events_chat_idx ON run_events (chat_id);
CREATE INDEX run_events_run_sequence_idx ON run_events (run_id, sequence);

CREATE TABLE tool_calls (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    run_id TEXT NOT NULL CHECK (length(run_id) > 0),
    message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    tool_name TEXT NOT NULL CHECK (length(tool_name) > 0),
    input_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK (length(status) > 0),
    started_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE INDEX tool_calls_run_idx ON tool_calls (run_id);
CREATE INDEX tool_calls_message_idx ON tool_calls (message_id);

CREATE TABLE tool_results (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    tool_call_id TEXT NOT NULL REFERENCES tool_calls(id) ON DELETE CASCADE,
    output_json TEXT NOT NULL,
    is_error INTEGER NOT NULL DEFAULT 0 CHECK (is_error IN (0, 1)),
    created_at TEXT NOT NULL
);

CREATE INDEX tool_results_tool_call_idx ON tool_results (tool_call_id);

CREATE TABLE terminal_sessions (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    name TEXT NOT NULL CHECK (length(name) > 0),
    working_directory TEXT NOT NULL CHECK (length(working_directory) > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    closed_at TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE llm_requests (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    chat_id TEXT REFERENCES chats(id) ON DELETE SET NULL,
    provider_id TEXT NOT NULL CHECK (length(provider_id) > 0),
    model_id TEXT NOT NULL CHECK (length(model_id) > 0),
    request_started_at TEXT NOT NULL,
    first_token_at TEXT,
    completed_at TEXT,
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    cache_read_tokens INTEGER CHECK (cache_read_tokens IS NULL OR cache_read_tokens >= 0),
    cache_write_tokens INTEGER CHECK (cache_write_tokens IS NULL OR cache_write_tokens >= 0),
    first_token_latency_ms INTEGER CHECK (first_token_latency_ms IS NULL OR first_token_latency_ms >= 0),
    total_latency_ms INTEGER CHECK (total_latency_ms IS NULL OR total_latency_ms >= 0),
    status_code INTEGER,
    final_state TEXT NOT NULL CHECK (length(final_state) > 0),
    request_body_json TEXT,
    response_body_json TEXT
);

CREATE INDEX llm_requests_chat_idx ON llm_requests (chat_id);
CREATE INDEX llm_requests_provider_model_idx ON llm_requests (provider_id, model_id);
CREATE INDEX llm_requests_started_at_idx ON llm_requests (request_started_at);

CREATE TABLE code_graph_files (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE CHECK (length(path) > 0),
    language TEXT,
    size_bytes INTEGER CHECK (size_bytes IS NULL OR size_bytes >= 0),
    modified_at TEXT,
    discovered_at TEXT NOT NULL
);

CREATE TABLE code_graph_file_hashes (
    file_id INTEGER PRIMARY KEY REFERENCES code_graph_files(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL CHECK (length(content_hash) > 0),
    hashed_at TEXT NOT NULL
);

CREATE TABLE code_graph_parse_status (
    file_id INTEGER PRIMARY KEY REFERENCES code_graph_files(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (length(status) > 0),
    parsed_at TEXT,
    error_message TEXT
);

CREATE TABLE code_graph_symbols (
    id INTEGER PRIMARY KEY,
    file_id INTEGER NOT NULL REFERENCES code_graph_files(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(name) > 0),
    kind TEXT NOT NULL CHECK (length(kind) > 0),
    start_line INTEGER CHECK (start_line IS NULL OR start_line >= 0),
    start_column INTEGER CHECK (start_column IS NULL OR start_column >= 0),
    end_line INTEGER CHECK (end_line IS NULL OR end_line >= 0),
    end_column INTEGER CHECK (end_column IS NULL OR end_column >= 0),
    signature TEXT,
    documentation TEXT,
    UNIQUE (file_id, name, kind, start_line, start_column)
);

CREATE INDEX code_graph_symbols_file_idx ON code_graph_symbols (file_id);
CREATE INDEX code_graph_symbols_name_idx ON code_graph_symbols (name);

CREATE TABLE code_graph_edges (
    id INTEGER PRIMARY KEY,
    source_symbol_id INTEGER REFERENCES code_graph_symbols(id) ON DELETE CASCADE,
    target_symbol_id INTEGER REFERENCES code_graph_symbols(id) ON DELETE CASCADE,
    edge_kind TEXT NOT NULL CHECK (length(edge_kind) > 0),
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX code_graph_edges_source_idx ON code_graph_edges (source_symbol_id);
CREATE INDEX code_graph_edges_target_idx ON code_graph_edges (target_symbol_id);

CREATE TABLE code_graph_references (
    id INTEGER PRIMARY KEY,
    file_id INTEGER NOT NULL REFERENCES code_graph_files(id) ON DELETE CASCADE,
    symbol_id INTEGER REFERENCES code_graph_symbols(id) ON DELETE SET NULL,
    name TEXT NOT NULL CHECK (length(name) > 0),
    start_line INTEGER CHECK (start_line IS NULL OR start_line >= 0),
    start_column INTEGER CHECK (start_column IS NULL OR start_column >= 0),
    end_line INTEGER CHECK (end_line IS NULL OR end_line >= 0),
    end_column INTEGER CHECK (end_column IS NULL OR end_column >= 0)
);

CREATE INDEX code_graph_references_file_idx ON code_graph_references (file_id);
CREATE INDEX code_graph_references_symbol_idx ON code_graph_references (symbol_id);

CREATE TABLE code_graph_imports (
    id INTEGER PRIMARY KEY,
    file_id INTEGER NOT NULL REFERENCES code_graph_files(id) ON DELETE CASCADE,
    module TEXT NOT NULL CHECK (length(module) > 0),
    imported_symbol TEXT,
    alias TEXT,
    start_line INTEGER CHECK (start_line IS NULL OR start_line >= 0),
    start_column INTEGER CHECK (start_column IS NULL OR start_column >= 0)
);

CREATE INDEX code_graph_imports_file_idx ON code_graph_imports (file_id);
CREATE INDEX code_graph_imports_module_idx ON code_graph_imports (module);

CREATE TABLE code_graph_fts_data (
    id INTEGER PRIMARY KEY,
    entity_kind TEXT NOT NULL CHECK (length(entity_kind) > 0),
    entity_id TEXT NOT NULL CHECK (length(entity_id) > 0),
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (entity_kind, entity_id)
);
"#;

const MIGRATION_002: &str = r#"
ALTER TABLE llm_requests
    ADD COLUMN workspace_id TEXT CHECK (workspace_id IS NULL OR length(workspace_id) > 0);

ALTER TABLE llm_requests
    ADD COLUMN cache_ratio REAL CHECK (cache_ratio IS NULL OR (cache_ratio >= 0.0 AND cache_ratio <= 1.0));

CREATE INDEX llm_requests_workspace_started_at_idx ON llm_requests (workspace_id, request_started_at);

CREATE TABLE llm_request_events (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    llm_request_id TEXT NOT NULL REFERENCES llm_requests(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    event_at TEXT NOT NULL,
    event_type TEXT NOT NULL CHECK (length(event_type) > 0),
    raw_chunk_json TEXT,
    normalized_event_json TEXT NOT NULL,
    UNIQUE (llm_request_id, sequence)
);

CREATE INDEX llm_request_events_request_sequence_idx ON llm_request_events (llm_request_id, sequence);
CREATE INDEX llm_request_events_type_idx ON llm_request_events (event_type);
"#;
