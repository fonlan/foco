use std::{
    collections::{HashMap, HashSet},
    fmt, fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::WorkspaceConfig;
use crate::memory::WORKSPACE_MEMORY_SCHEMA_SQL;

pub const WORKSPACE_FOCO_DIR: &str = ".foco";
pub const WORKSPACE_DATABASE_FILE: &str = "foco.sqlite";
pub const WORKSPACE_SCHEMA_VERSION: u32 = 9;
const WORKSPACE_DATABASE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

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

    pub fn delete_chat(&mut self, id: &str) -> Result<bool, WorkspaceDatabaseError> {
        let deleted = self
            .connection
            .execute("DELETE FROM chats WHERE id = ?1", params![id])
            .map_err(|source| self.sqlite_error(source))?;

        Ok(deleted > 0)
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

    pub fn chat_code_change_stats(
        &self,
    ) -> Result<HashMap<String, CodeChangeStats>, WorkspaceDatabaseError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT chat_id, metadata_json
                 FROM messages
                 WHERE role = 'assistant'",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|source| self.sqlite_error(source))?;
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

    pub fn upsert_message_content(
        &mut self,
        message: NewMessage<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        let now = now_timestamp();
        let metadata_json = message.metadata_json.unwrap_or("{}");

        let changed = self
            .connection
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
            .map_err(|source| self.sqlite_error(source))?;

        if changed == 0 {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!(
                    "message '{}' already exists with a different chat, role, or sequence",
                    message.id
                ),
            });
        }

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

    pub fn update_llm_request_outcome(
        &mut self,
        id: &str,
        outcome: UpdateLlmRequestOutcome<'_>,
    ) -> Result<(), WorkspaceDatabaseError> {
        validate_llm_token_values(
            outcome.input_tokens,
            outcome.output_tokens,
            outcome.cache_read_tokens,
            outcome.cache_write_tokens,
        )?;

        let cache_ratio = calculate_cache_ratio(outcome.input_tokens, outcome.cache_read_tokens)?;
        let response_body_json =
            redact_optional_audit_json(outcome.response_body_json, "response_body_json")?;

        let updated = self
            .connection
            .execute(
                "UPDATE llm_requests
                 SET first_token_at = ?2,
                     completed_at = ?3,
                     input_tokens = ?4,
                     output_tokens = ?5,
                     cache_read_tokens = ?6,
                     cache_write_tokens = ?7,
                     cache_ratio = ?8,
                     first_token_latency_ms = ?9,
                     total_latency_ms = ?10,
                     status_code = ?11,
                     final_state = ?12,
                     response_body_json = ?13
                 WHERE id = ?1",
                params![
                    id,
                    outcome.first_token_at,
                    outcome.completed_at,
                    outcome.input_tokens,
                    outcome.output_tokens,
                    outcome.cache_read_tokens,
                    outcome.cache_write_tokens,
                    cache_ratio,
                    outcome.first_token_latency_ms,
                    outcome.total_latency_ms,
                    outcome.status_code,
                    outcome.final_state,
                    response_body_json
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;

        if updated == 0 {
            return Err(WorkspaceDatabaseError::MissingLlmRequest { id: id.to_string() });
        }

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
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    id, workspace_id, chat_id, provider_id, model_id, request_started_at,
                    first_token_at, completed_at, input_tokens, output_tokens,
                    cache_read_tokens, cache_write_tokens, cache_ratio,
                    first_token_latency_ms, total_latency_ms, status_code, final_state
                 FROM llm_requests
                 WHERE (?1 IS NULL OR workspace_id = ?1)
                   AND (?2 IS NULL OR chat_id = ?2)
                   AND (?3 IS NULL OR provider_id = ?3)
                   AND (?4 IS NULL OR model_id = ?4)
                   AND (?5 IS NULL OR final_state = ?5)
                   AND (?6 IS NULL OR request_started_at >= ?6)
                   AND (?7 IS NULL OR request_started_at <= ?7)
                 ORDER BY request_started_at DESC, id DESC
                 LIMIT ?8 OFFSET ?9",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(
                params![
                    filters.workspace_id,
                    filters.chat_id,
                    filters.provider_id,
                    filters.model_id,
                    filters.final_state,
                    filters.started_after,
                    filters.started_before,
                    limit,
                    offset
                ],
                |row| {
                    Ok(LlmRequestAuditRow {
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
                    })
                },
            )
            .map_err(|source| self.sqlite_error(source))?;

        collect_rows(rows, &self.database_path)
    }

    pub fn llm_request_audit_count(
        &self,
        filters: LlmRequestAuditFilters<'_>,
    ) -> Result<i64, WorkspaceDatabaseError> {
        self.connection
            .query_row(
                "SELECT COUNT(*)
                 FROM llm_requests
                 WHERE (?1 IS NULL OR workspace_id = ?1)
                   AND (?2 IS NULL OR chat_id = ?2)
                   AND (?3 IS NULL OR provider_id = ?3)
                   AND (?4 IS NULL OR model_id = ?4)
                   AND (?5 IS NULL OR final_state = ?5)
                   AND (?6 IS NULL OR request_started_at >= ?6)
                   AND (?7 IS NULL OR request_started_at <= ?7)",
                params![
                    filters.workspace_id,
                    filters.chat_id,
                    filters.provider_id,
                    filters.model_id,
                    filters.final_state,
                    filters.started_after,
                    filters.started_before,
                ],
                |row| row.get(0),
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

        self.connection
            .execute(
                "INSERT INTO prompt_context_injections
                    (id, chat_id, kind, sequence, messages_json, memory_keys_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    injection.id,
                    injection.chat_id,
                    injection.kind,
                    injection.sequence,
                    injection.messages_json,
                    injection.memory_keys_json,
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
                "SELECT id, chat_id, kind, sequence, messages_json, memory_keys_json, created_at
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
                    created_at: row.get(6)?,
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
        for symbol in index.symbols {
            transaction
                .execute(
                    "INSERT INTO code_graph_symbols
                        (
                            file_id, name, kind, start_line, start_column,
                            end_line, end_column, signature, documentation
                        )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        file_id,
                        symbol.name,
                        symbol.kind,
                        symbol.start_line,
                        symbol.start_column,
                        symbol.end_line,
                        symbol.end_column,
                        symbol.signature,
                        symbol.documentation
                    ],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
            let symbol_id = transaction.last_insert_rowid();
            symbol_ids.push(symbol_id);
            upsert_code_graph_fts_entry(
                &transaction,
                &database_path,
                "symbol",
                &symbol_id.to_string(),
                symbol.name,
                symbol
                    .documentation
                    .or(symbol.signature)
                    .unwrap_or(symbol.name),
                &now,
            )?;
        }

        for import in index.imports {
            transaction
                .execute(
                    "INSERT INTO code_graph_imports
                        (
                            file_id, module, imported_symbol, alias,
                            start_line, start_column
                        )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        file_id,
                        import.module,
                        import.imported_symbol,
                        import.alias,
                        import.start_line,
                        import.start_column
                    ],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }

        for reference in index.references {
            let symbol_id = match reference.symbol_index {
                Some(symbol_index) => Some(*symbol_ids.get(symbol_index).ok_or_else(|| {
                    WorkspaceDatabaseError::InvalidCodeGraphInput {
                        message: format!("reference points to missing symbol index {symbol_index}"),
                    }
                })?),
                None => None,
            };
            transaction
                .execute(
                    "INSERT INTO code_graph_references
                        (
                            file_id, symbol_id, name, start_line, start_column,
                            end_line, end_column
                        )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        file_id,
                        symbol_id,
                        reference.name,
                        reference.start_line,
                        reference.start_column,
                        reference.end_line,
                        reference.end_column
                    ],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }

        for edge in index.edges {
            let source_symbol_id = *symbol_ids.get(edge.source_symbol_index).ok_or_else(|| {
                WorkspaceDatabaseError::InvalidCodeGraphInput {
                    message: format!(
                        "edge source points to missing symbol index {}",
                        edge.source_symbol_index
                    ),
                }
            })?;
            let target_symbol_id = *symbol_ids.get(edge.target_symbol_index).ok_or_else(|| {
                WorkspaceDatabaseError::InvalidCodeGraphInput {
                    message: format!(
                        "edge target points to missing symbol index {}",
                        edge.target_symbol_index
                    ),
                }
            })?;
            transaction
                .execute(
                    "INSERT INTO code_graph_edges
                        (
                            source_symbol_id, target_symbol_id,
                            edge_kind, metadata_json
                        )
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        source_symbol_id,
                        target_symbol_id,
                        edge.edge_kind,
                        edge.metadata_json.unwrap_or("{}")
                    ],
                )
                .map_err(|source| sqlite_error(&database_path, source))?;
        }

        upsert_code_graph_fts_entry(
            &transaction,
            &database_path,
            "file",
            index.path,
            index.path,
            index.fts_body,
            &now,
        )?;
        transaction
            .commit()
            .map_err(|source| sqlite_error(&database_path, source))?;

        Ok(file_id)
    }

    pub fn delete_code_graph_file(&mut self, path: &str) -> Result<bool, WorkspaceDatabaseError> {
        let database_path = self.database_path.clone();
        let transaction =
            self.connection
                .transaction()
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatRecord {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeChangeStats {
    pub additions: usize,
    pub deletions: usize,
}

impl CodeChangeStats {
    fn from_metadata(value: &Value) -> Result<Self, WorkspaceDatabaseError> {
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

        let additions =
            usize::try_from(additions).map_err(|_| WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "message metadata.codeChangeStats.additions is too large".to_string(),
            })?;
        let deletions =
            usize::try_from(deletions).map_err(|_| WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "message metadata.codeChangeStats.deletions is too large".to_string(),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateLlmRequestOutcome<'a> {
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LlmRequestAuditFilters<'a> {
    pub workspace_id: Option<&'a str>,
    pub chat_id: Option<&'a str>,
    pub provider_id: Option<&'a str>,
    pub model_id: Option<&'a str>,
    pub final_state: Option<&'a str>,
    pub started_after: Option<&'a str>,
    pub started_before: Option<&'a str>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LlmRequestAuditRow {
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptContextInjectionRecord {
    pub id: String,
    pub chat_id: String,
    pub kind: String,
    pub sequence: Option<i64>,
    pub messages_json: String,
    pub memory_keys_json: String,
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
    pub kind: &'a str,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphContextRecord {
    pub indexed_files: i64,
    pub symbols: i64,
    pub references: i64,
    pub edges: i64,
    pub languages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphSymbolRecord {
    pub id: i64,
    pub path: String,
    pub language: Option<String>,
    pub name: String,
    pub kind: String,
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

#[derive(Debug)]
pub enum WorkspaceDatabaseError {
    InvalidCodeGraphInput {
        message: String,
    },
    InvalidMessageMetadata {
        message: String,
    },
    InvalidTodoGraph {
        message: String,
    },
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
    MissingTodoGraph {
        chat_id: String,
    },
    MissingTerminalSession {
        id: String,
    },
    MissingLlmRequest {
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
    WorkspaceNotDirectory {
        path: PathBuf,
    },
}

impl fmt::Display for WorkspaceDatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCodeGraphInput { message } => {
                write!(formatter, "invalid code graph index data: {message}")
            }
            Self::InvalidMessageMetadata { message } => {
                write!(formatter, "invalid message metadata: {message}")
            }
            Self::InvalidTodoGraph { message } => {
                write!(formatter, "invalid todo graph: {message}")
            }
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
            Self::MissingTodoGraph { chat_id } => {
                write!(formatter, "todo graph was not found for chat: {chat_id}")
            }
            Self::MissingTerminalSession { id } => {
                write!(formatter, "terminal session was not found: {id}")
            }
            Self::MissingLlmRequest { id } => {
                write!(formatter, "LLM request audit row was not found: {id}")
            }
            Self::NonUtf8Path { path } => {
                write!(formatter, "path must be valid UTF-8: {}", path.display())
            }
            Self::Sqlite { path, source } => {
                write!(formatter, "{} SQLite error: {}", path.display(), source)
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
            Self::TodoGraphJson { source } => Some(source),
            Self::InvalidAuditTokens { .. }
            | Self::InvalidCodeGraphInput { .. }
            | Self::InvalidMessageMetadata { .. }
            | Self::InvalidTodoGraph { .. }
            | Self::MissingDatabaseParent { .. }
            | Self::MissingLlmRequest { .. }
            | Self::MissingTodoGraph { .. }
            | Self::MissingTerminalSession { .. }
            | Self::NonUtf8Path { .. }
            | Self::UnsupportedSchemaVersion { .. }
            | Self::WorkspaceNotDirectory { .. } => None,
        }
    }
}

fn code_graph_symbol_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CodeGraphSymbolRecord> {
    code_graph_symbol_from_row_offset(row, 0)
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

struct Migration {
    version: u32,
    sql: &'static str,
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
    let symbol_ids = {
        let mut statement = transaction
            .prepare("SELECT id FROM code_graph_symbols WHERE file_id = ?1")
            .map_err(|source| sqlite_error(database_path, source))?;
        let rows = statement
            .query_map(params![file_id], |row| row.get::<_, i64>(0))
            .map_err(|source| sqlite_error(database_path, source))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|source| sqlite_error(database_path, source))?
    };

    for symbol_id in symbol_ids {
        delete_code_graph_fts_entry(transaction, database_path, "symbol", &symbol_id.to_string())?;
    }
    delete_code_graph_fts_entry(transaction, database_path, "file", path)?;
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

fn upsert_code_graph_fts_entry(
    transaction: &Transaction<'_>,
    database_path: &Path,
    entity_kind: &str,
    entity_id: &str,
    title: &str,
    body: &str,
    updated_at: &str,
) -> Result<(), WorkspaceDatabaseError> {
    delete_code_graph_fts_entry(transaction, database_path, entity_kind, entity_id)?;
    transaction
        .execute(
            "INSERT INTO code_graph_fts_data
                (entity_kind, entity_id, title, body, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(entity_kind, entity_id) DO UPDATE SET
                title = excluded.title,
                body = excluded.body,
                updated_at = excluded.updated_at",
            params![entity_kind, entity_id, title, body, updated_at],
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    transaction
        .execute(
            "INSERT INTO code_graph_fts_index (entity_kind, entity_id, title, body)
             VALUES (?1, ?2, ?3, ?4)",
            params![entity_kind, entity_id, title, body],
        )
        .map_err(|source| sqlite_error(database_path, source))?;

    Ok(())
}

fn delete_code_graph_fts_entry(
    transaction: &Transaction<'_>,
    database_path: &Path,
    entity_kind: &str,
    entity_id: &str,
) -> Result<(), WorkspaceDatabaseError> {
    transaction
        .execute(
            "DELETE FROM code_graph_fts_index
             WHERE entity_kind = ?1 AND entity_id = ?2",
            params![entity_kind, entity_id],
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    transaction
        .execute(
            "DELETE FROM code_graph_fts_data
             WHERE entity_kind = ?1 AND entity_id = ?2",
            params![entity_kind, entity_id],
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

fn sqlite_error(database_path: &Path, source: rusqlite::Error) -> WorkspaceDatabaseError {
    WorkspaceDatabaseError::Sqlite {
        path: database_path.to_path_buf(),
        source,
    }
}

fn open_connection(database_path: &Path) -> Result<Connection, WorkspaceDatabaseError> {
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
    validate_llm_token_values(
        request.input_tokens,
        request.output_tokens,
        request.cache_read_tokens,
        request.cache_write_tokens,
    )
}

fn validate_llm_token_values(
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
) -> Result<(), WorkspaceDatabaseError> {
    for (name, value) in [
        ("input_tokens", input_tokens),
        ("output_tokens", output_tokens),
        ("cache_read_tokens", cache_read_tokens),
        ("cache_write_tokens", cache_write_tokens),
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

    normalized == "authorization"
        || normalized.contains("apikey")
        || normalized.contains("cookie")
        || normalized.contains("password")
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

const MIGRATION_003: &str = r#"
CREATE TABLE context_compression_snapshots (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    run_id TEXT NOT NULL CHECK (length(run_id) > 0),
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    summary TEXT NOT NULL CHECK (length(summary) > 0),
    source_message_start_sequence INTEGER NOT NULL CHECK (source_message_start_sequence >= 0),
    source_message_end_sequence INTEGER NOT NULL CHECK (source_message_end_sequence >= source_message_start_sequence),
    original_token_count INTEGER NOT NULL CHECK (original_token_count > 0),
    summary_token_count INTEGER NOT NULL CHECK (summary_token_count > 0),
    created_at TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    UNIQUE (chat_id, sequence)
);

CREATE INDEX context_compression_snapshots_chat_sequence_idx
    ON context_compression_snapshots (chat_id, sequence);
CREATE INDEX context_compression_snapshots_run_idx
    ON context_compression_snapshots (run_id);
"#;

const MIGRATION_004: &str = r#"
CREATE VIRTUAL TABLE code_graph_fts_index USING fts5(
    entity_kind UNINDEXED,
    entity_id UNINDEXED,
    title,
    body
);

INSERT INTO code_graph_fts_index (entity_kind, entity_id, title, body)
    SELECT entity_kind, entity_id, title, body
    FROM code_graph_fts_data;
"#;

const MIGRATION_005: &str = r#"
CREATE TABLE task_graphs (
    chat_id TEXT PRIMARY KEY NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    graph_json TEXT NOT NULL CHECK (length(graph_json) > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX task_graphs_updated_at_idx ON task_graphs (updated_at);
"#;

const MIGRATION_006: &str = r#"
CREATE TABLE hook_runs (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL CHECK (length(workspace_id) > 0),
    chat_id TEXT REFERENCES chats(id) ON DELETE SET NULL,
    run_id TEXT CHECK (run_id IS NULL OR length(run_id) > 0),
    tool_call_id TEXT REFERENCES tool_calls(id) ON DELETE SET NULL,
    event TEXT NOT NULL CHECK (length(event) > 0),
    hook_source TEXT NOT NULL CHECK (length(hook_source) > 0),
    handler_type TEXT NOT NULL CHECK (length(handler_type) > 0),
    input_json TEXT NOT NULL,
    output_json TEXT,
    status TEXT NOT NULL CHECK (length(status) > 0),
    exit_code INTEGER,
    stdout_preview TEXT,
    stderr_preview TEXT,
    started_at TEXT NOT NULL,
    completed_at TEXT NOT NULL
);

CREATE INDEX hook_runs_workspace_started_idx ON hook_runs (workspace_id, started_at);
CREATE INDEX hook_runs_chat_idx ON hook_runs (chat_id);
CREATE INDEX hook_runs_run_idx ON hook_runs (run_id);
CREATE INDEX hook_runs_event_idx ON hook_runs (event);
"#;

const MIGRATION_008: &str = r#"
DROP INDEX task_graphs_updated_at_idx;
ALTER TABLE task_graphs RENAME TO todo_graphs;
CREATE INDEX todo_graphs_updated_at_idx ON todo_graphs (updated_at);
"#;

const MIGRATION_009: &str = r#"
CREATE TABLE prompt_context_injections (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('stable', 'turn_memory')),
    sequence INTEGER CHECK (sequence IS NULL OR sequence >= 0),
    messages_json TEXT NOT NULL CHECK (length(messages_json) > 0),
    memory_keys_json TEXT NOT NULL CHECK (length(memory_keys_json) > 0),
    created_at TEXT NOT NULL,
    CHECK ((kind = 'stable' AND sequence IS NULL) OR (kind = 'turn_memory' AND sequence IS NOT NULL))
);

CREATE UNIQUE INDEX prompt_context_injections_stable_chat_idx
    ON prompt_context_injections (chat_id)
    WHERE kind = 'stable';
CREATE UNIQUE INDEX prompt_context_injections_turn_chat_sequence_idx
    ON prompt_context_injections (chat_id, sequence)
    WHERE kind = 'turn_memory';
CREATE INDEX prompt_context_injections_chat_kind_sequence_idx
    ON prompt_context_injections (chat_id, kind, sequence);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_runs_redact_secret_input_and_output_json() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

        database
            .insert_hook_run(NewHookRun {
                id: "hook-1",
                workspace_id: "workspace-1",
                chat_id: None,
                run_id: Some("run-1"),
                tool_call_id: None,
                event: "PreToolUse",
                hook_source: "global",
                handler_type: "command",
                input_json: r#"{"headers":{"authorization":"Bearer sk-secret"},"cookie":"abc","safe":"ok"}"#,
                output_json: Some(r#"{"password":"secret","nested":{"api_key":"sk-test"},"ok":true}"#),
                status: "succeeded",
                exit_code: Some(0),
                stdout_preview: Some("safe output"),
                stderr_preview: None,
                started_at: "2026-06-08T10:00:00Z",
                completed_at: "2026-06-08T10:00:01Z",
            })
            .expect("hook run insert");

        let run = database
            .hook_runs(10)
            .expect("hook runs")
            .into_iter()
            .next()
            .expect("inserted hook run");

        assert!(run.input_json.contains("[REDACTED]"));
        assert!(
            run.output_json
                .as_deref()
                .unwrap_or_default()
                .contains("[REDACTED]")
        );
        assert!(!run.input_json.contains("sk-secret"));
        assert!(!run.input_json.contains("abc"));
        assert!(
            !run.output_json
                .as_deref()
                .unwrap_or_default()
                .contains("sk-test")
        );
        assert!(run.input_json.contains("\"safe\":\"ok\""));
    }
}
