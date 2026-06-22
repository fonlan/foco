use std::{
    collections::{HashMap, HashSet},
    fmt, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use chrono::{SecondsFormat, Utc};
use foco_agent::{
    AgentAttemptId, AgentAttemptStatus, AgentDomainError, AgentEntityKind, AgentInstanceId,
    AgentInstanceStatus, AgentMessageId, AgentTaskId, AgentTaskStatus, AgentTaskTransition,
    AgentTeamId, AgentTeamStatus, TeamWorkload,
};
use rusqlite::{
    Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params, params_from_iter,
    types::Value as SqlValue,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::config::WorkspaceConfig;
use crate::memory::WORKSPACE_MEMORY_SCHEMA_SQL;
#[path = "workspace_records.rs"]
mod workspace_records;
#[path = "workspace_schema.rs"]
mod workspace_schema;

pub use workspace_records::{
    AgentAttemptRecord, AgentContextEntryRecord, AgentContextSnapshotRecord, AgentEventRecord,
    AgentInstanceRecord, AgentMessageRecord, AgentReconciliationRecord, AgentTaskDependencyRecord,
    AgentTaskRecord, AgentTaskStateUpdate, AgentTeamRecord, ChatRecord, CodeChangeStats,
    CodeGraphContextRecord, CodeGraphReferenceRecord, CodeGraphRelatedFileRecord,
    CodeGraphSymbolRecord, CodeGraphSymbolRelationRecord, ContextCompressionSnapshotRecord,
    HookRunRecord, LlmRequestAuditFilters, LlmRequestAuditModelBreakdown,
    LlmRequestAuditProviderBreakdown, LlmRequestAuditRow, LlmRequestAuditSummaryRow,
    LlmRequestAuditTrendPoint, LlmRequestEventRecord, LlmRequestMetricsRecord, LlmRequestRecord,
    MessageRecord, NewAgentContextEntry, NewAgentContextSnapshot, NewAgentEvent, NewAgentInstance,
    NewAgentMessage, NewAgentTask, NewAgentTaskDependency, NewAgentTeam, NewCodeGraphEdge,
    NewCodeGraphFileIndex, NewCodeGraphImport, NewCodeGraphReference, NewCodeGraphSymbol,
    NewContextCompressionSnapshot, NewHookRun, NewLlmRequest, NewLlmRequestEvent, NewMessage,
    NewPromptContextInjection, NewRunEvent, NewScheduledTask, NewScheduledTaskRun,
    NewTerminalSession, NewToolCall, NewToolResult, PromptContextInjectionRecord, RunEventRecord,
    ScheduledTaskDueRunClaim, ScheduledTaskRecord, ScheduledTaskRunRecord, ScheduledTaskRunUpdate,
    ScheduledTaskUpdate, TerminalSessionRecord, TodoGraphFilter, TodoGraphRecord, TodoGraphTask,
    TodoGraphTaskPatch, ToolCallCountRecord, ToolCallWithResultRecord, ToolResultRecord,
    UpdateLlmRequestOutcome,
};
use workspace_schema::{
    MIGRATION_001, MIGRATION_002, MIGRATION_003, MIGRATION_004, MIGRATION_005, MIGRATION_006,
    MIGRATION_008, MIGRATION_009, MIGRATION_010, MIGRATION_011, MIGRATION_012, MIGRATION_013,
    MIGRATION_014, MIGRATION_015, Migration,
};

pub const WORKSPACE_FOCO_DIR: &str = ".foco";
pub const WORKSPACE_DATABASE_FILE: &str = "foco.sqlite";
pub const WORKSPACE_SCHEMA_VERSION: u32 = 15;
const QUEUED_CHAT_METADATA_KEY: &str = "queuedRun";
const QUEUED_MESSAGE_METADATA_KEY: &str = "queuedRun";
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
                "SELECT id, title, created_at, updated_at, archived_at, metadata_json
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
                    metadata_json: row.get(5)?,
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

    pub fn insert_message_if_absent(
        &mut self,
        message: NewMessage<'_>,
    ) -> Result<bool, WorkspaceDatabaseError> {
        let now = now_timestamp();
        let metadata_json = message.metadata_json.unwrap_or("{}");
        let inserted = self
            .connection
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
            .map_err(|source| self.sqlite_error(source))?;

        if inserted > 0 {
            self.connection
                .execute(
                    "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
                    params![now, message.chat_id],
                )
                .map_err(|source| self.sqlite_error(source))?;
        }

        Ok(inserted > 0)
    }

    pub fn mark_chat_queued_run_started(
        &mut self,
        chat_id: &str,
        user_message_id: &str,
        assistant_message_id: &str,
        assistant_sequence: i64,
    ) -> Result<(), WorkspaceDatabaseError> {
        let chat =
            self.chat(chat_id)?
                .ok_or_else(|| WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: format!("chat was not found: {chat_id}"),
                })?;
        let mut chat_metadata = parse_json_object(&chat.metadata_json, "chat metadata")?;
        if let Some(queued_run) = chat_metadata.get_mut(QUEUED_CHAT_METADATA_KEY) {
            let Some(queued_run_object) = queued_run.as_object_mut() else {
                return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: "chat metadata.queuedRun must be an object".to_string(),
                });
            };
            queued_run_object.insert("status".to_string(), Value::String("running".to_string()));
            queued_run_object.insert(
                "assistantMessageId".to_string(),
                Value::String(assistant_message_id.to_string()),
            );
            queued_run_object.insert(
                "assistantSequence".to_string(),
                Value::Number(assistant_sequence.into()),
            );
        }
        let chat_metadata_json = serde_json::to_string(&chat_metadata).map_err(|source| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("chat metadata is invalid JSON: {source}"),
            }
        })?;

        let message = self.message(user_message_id)?.ok_or_else(|| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("message was not found: {user_message_id}"),
            }
        })?;
        let mut message_metadata =
            parse_json_object(&message.metadata_json, "user message metadata")?;
        if let Some(queued_run) = message_metadata.get_mut(QUEUED_MESSAGE_METADATA_KEY) {
            let Some(queued_run_object) = queued_run.as_object_mut() else {
                return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: "message metadata.queuedRun must be an object".to_string(),
                });
            };
            queued_run_object.insert("status".to_string(), Value::String("running".to_string()));
            queued_run_object.insert(
                "assistantMessageId".to_string(),
                Value::String(assistant_message_id.to_string()),
            );
            queued_run_object.insert(
                "assistantSequence".to_string(),
                Value::Number(assistant_sequence.into()),
            );
        }
        let message_metadata_json = serde_json::to_string(&message_metadata).map_err(|source| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: format!("user message metadata is invalid JSON: {source}"),
            }
        })?;

        self.connection
            .execute(
                "UPDATE chats SET metadata_json = ?1 WHERE id = ?2",
                params![chat_metadata_json, chat_id],
            )
            .map_err(|source| self.sqlite_error(source))?;
        self.connection
            .execute(
                "UPDATE messages SET metadata_json = ?1 WHERE id = ?2 AND chat_id = ?3",
                params![message_metadata_json, user_message_id, chat_id],
            )
            .map_err(|source| self.sqlite_error(source))?;

        Ok(())
    }

    pub fn clear_chat_queued_run(
        &mut self,
        chat_id: &str,
        user_message_id: &str,
    ) -> Result<(), WorkspaceDatabaseError> {
        let chat =
            self.chat(chat_id)?
                .ok_or_else(|| WorkspaceDatabaseError::InvalidMessageMetadata {
                    message: format!("chat was not found: {chat_id}"),
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
            self.connection
                .execute(
                    "UPDATE chats SET metadata_json = ?1 WHERE id = ?2",
                    params![chat_metadata_json, chat_id],
                )
                .map_err(|source| self.sqlite_error(source))?;
        }

        let Some(message) = self.message(user_message_id)? else {
            return Ok(());
        };
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
            self.connection
                .execute(
                    "UPDATE messages SET metadata_json = ?1 WHERE id = ?2 AND chat_id = ?3",
                    params![message_metadata_json, user_message_id, chat_id],
                )
                .map_err(|source| self.sqlite_error(source))?;
        }

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
                   AND id NOT IN (
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
                   )
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
                       ('reasoning_delta', 'text_delta', 'tool_call', 'stream_reset')
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
        let request_body_json =
            redact_optional_audit_json(request.request_body_json, "request_body_json")?;
        let response_body_json =
            redact_optional_audit_json(request.response_body_json, "response_body_json")?;

        self.connection
            .execute(
                "INSERT INTO llm_requests
                    (
                        id, workspace_id, chat_id, agent_team_id, agent_instance_id,
                        agent_task_id, agent_attempt_id, provider_id, model_id, request_started_at,
                        first_token_at, completed_at, input_tokens, output_tokens,
                        cache_read_tokens, cache_write_tokens, cache_ratio,
                        first_token_latency_ms, total_latency_ms, status_code, final_state,
                        request_body_json, response_body_json
                    )
                 VALUES
                    (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
                params![
                    request.id,
                    request.workspace_id,
                    request.chat_id,
                    request.agent_team_id.map(AgentTeamId::as_str),
                    request.agent_instance_id.map(AgentInstanceId::as_str),
                    request.agent_task_id.map(AgentTaskId::as_str),
                    request.agent_attempt_id.map(AgentAttemptId::as_str),
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
                    id, workspace_id, chat_id, agent_team_id, agent_instance_id,
                    agent_task_id, agent_attempt_id, provider_id, model_id, request_started_at,
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
                        agent_team_id: optional_agent_id_from_row(row, 3)?,
                        agent_instance_id: optional_agent_id_from_row(row, 4)?,
                        agent_task_id: optional_agent_id_from_row(row, 5)?,
                        agent_attempt_id: optional_agent_id_from_row(row, 6)?,
                        provider_id: row.get(7)?,
                        model_id: row.get(8)?,
                        request_started_at: row.get(9)?,
                        first_token_at: row.get(10)?,
                        completed_at: row.get(11)?,
                        input_tokens: row.get(12)?,
                        output_tokens: row.get(13)?,
                        cache_read_tokens: row.get(14)?,
                        cache_write_tokens: row.get(15)?,
                        cache_ratio: row.get(16)?,
                        first_token_latency_ms: row.get(17)?,
                        total_latency_ms: row.get(18)?,
                        status_code: row.get(19)?,
                        final_state: row.get(20)?,
                        request_body_json: row.get(21)?,
                        response_body_json: row.get(22)?,
                    })
                },
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
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
                id, workspace_id, chat_id, provider_id, model_id, request_started_at,
                first_token_at, completed_at, input_tokens, output_tokens,
                cache_read_tokens, cache_write_tokens, cache_ratio,
                first_token_latency_ms, total_latency_ms, status_code, final_state
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
        query.push_str(" GROUP BY model_id");
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
        query.push_str(" GROUP BY provider_id");
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
        if let Some(status) = status {
            validate_scheduled_task_status(status)?;
            let mut statement = self
                .connection
                .prepare(
                    "SELECT id, title, description, schedule_json, action_json, status,
                            next_run_at, last_run_at, created_at, updated_at, metadata_json
                     FROM scheduled_tasks
                     WHERE status = ?1
                     ORDER BY
                        CASE WHEN next_run_at IS NULL THEN 1 ELSE 0 END,
                        next_run_at ASC,
                        updated_at DESC,
                        id ASC",
                )
                .map_err(|source| self.sqlite_error(source))?;
            let rows = statement
                .query_map(params![status], scheduled_task_from_row)
                .map_err(|source| self.sqlite_error(source))?;
            return collect_rows(rows, &self.database_path);
        }

        let mut statement = self
            .connection
            .prepare(
                "SELECT id, title, description, schedule_json, action_json, status,
                        next_run_at, last_run_at, created_at, updated_at, metadata_json
                 FROM scheduled_tasks
                 ORDER BY
                    CASE WHEN next_run_at IS NULL THEN 1 ELSE 0 END,
                    next_run_at ASC,
                    updated_at DESC,
                    id ASC",
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], scheduled_task_from_row)
            .map_err(|source| self.sqlite_error(source))?;
        collect_rows(rows, &self.database_path)
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
                     created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'coordinator', 'idle', 0, 0, 0, 'shared', ?6, ?6)",
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
                   AND json_extract(input_json, '$.queuedUserMessageId') = ?2",
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
            .prepare(
                "SELECT task.id, task.team_id, task.owner_instance_id,
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
                 LIMIT ?2",
            )
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
                 WHERE id = ?1 AND team_id = ?2 AND status = ?3",
                params![
                    update.task_id.as_str(),
                    update.team_id.as_str(),
                    update.expected_status.as_str()
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
                       AND status = ?6",
                    params![
                        update.task_id.as_str(),
                        update.team_id.as_str(),
                        attempt_target.as_str(),
                        attempt_completed_at,
                        update.interruption_reason,
                        source_attempt_status
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

const AGENT_TASK_SELECT_BY_ID: &str =
    "SELECT id, team_id, owner_instance_id, origin_instance_id, parent_task_id,
            sequence, status, input_json, result_json, error_json, created_at, updated_at,
            started_at, completed_at
     FROM agent_tasks WHERE id = ?1";

const AGENT_MESSAGE_SELECT_BY_ID: &str =
    "SELECT id, team_id, sender_instance_id, receiver_instance_id, related_task_id,
            reply_to_message_id, kind, content, sequence, created_at, consumed_at
     FROM agent_messages WHERE id = ?1";

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
    InvalidAgentRuntimeData {
        message: String,
    },
    InvalidCodeGraphInput {
        message: String,
    },
    InvalidMessageMetadata {
        message: String,
    },
    InvalidScheduledTaskData {
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
            Self::InvalidAgentRuntimeData { message } => {
                write!(formatter, "invalid Agent runtime data: {message}")
            }
            Self::InvalidCodeGraphInput { message } => {
                write!(formatter, "invalid code graph index data: {message}")
            }
            Self::InvalidMessageMetadata { message } => {
                write!(formatter, "invalid message metadata: {message}")
            }
            Self::InvalidScheduledTaskData { message } => {
                write!(formatter, "invalid scheduled task data: {message}")
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
            Self::AgentDomain { source } => Some(source),
            Self::AgentRuntimeJson { source, .. } => Some(source),
            Self::InvalidAuditJson { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::Sqlite { source, .. } => Some(source),
            Self::TodoGraphJson { source } => Some(source),
            Self::InvalidAgentRuntimeData { .. }
            | Self::InvalidAuditTokens { .. }
            | Self::InvalidCodeGraphInput { .. }
            | Self::InvalidMessageMetadata { .. }
            | Self::InvalidScheduledTaskData { .. }
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

fn append_llm_request_audit_where_clause(
    query: &mut String,
    query_params: &mut Vec<SqlValue>,
    filters: LlmRequestAuditFilters<'_>,
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
    if let Some(value) = filters.chat_id {
        push_condition("chat_id = ?", value);
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
}

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

fn validate_json_metadata(
    metadata_json: &str,
    context: &str,
) -> Result<(), WorkspaceDatabaseError> {
    let _ = parse_json_object(metadata_json, context)?;
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
