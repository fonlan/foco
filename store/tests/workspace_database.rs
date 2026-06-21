use std::{fs, thread, time::Duration};

use foco_agent::{
    AgentAttemptId, AgentDefinitionId, AgentDomainErrorCode, AgentExecutionWorkspaceMode,
    AgentInstanceId, AgentInstanceStatus, AgentMessageId, AgentMessageKind, AgentPermissions,
    AgentRole, AgentTaskId, AgentTaskStatus, AgentTaskTransition, AgentTaskWaitMode, AgentTeamId,
    AgentTeamStatus,
};
use foco_store::{
    config::{AgentDefinitionSettings, AgentModelOptions, WorkspaceConfig},
    memory::{
        MemoryDatabase, MemoryKind, MemoryScope, MemorySourceType, MemoryStatus, NewMemoryFact,
        NewMemorySource,
    },
    workspace::{
        AgentTaskStateUpdate, LlmRequestAuditFilters, LlmRequestRecord, NewAgentContextEntry,
        NewAgentContextSnapshot, NewAgentEvent, NewAgentInstance, NewAgentMessage, NewAgentTask,
        NewAgentTaskDependency, NewAgentTeam, NewCodeGraphEdge, NewCodeGraphFileIndex,
        NewCodeGraphImport, NewCodeGraphReference, NewCodeGraphSymbol,
        NewContextCompressionSnapshot, NewLlmRequest, NewLlmRequestEvent, NewMessage,
        NewPromptContextInjection, NewRunEvent, NewTerminalSession, NewToolCall, NewToolResult,
        TodoGraphFilter, TodoGraphTask, TodoGraphTaskPatch, UpdateLlmRequestOutcome,
        WORKSPACE_SCHEMA_VERSION, WorkspaceDatabase, WorkspaceDatabaseError,
        initialize_workspace_databases, workspace_database_path,
    },
};
use rusqlite::{Connection, params};
use serde_json::Value;

#[test]
fn creates_workspace_foco_database_and_runs_migrations() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");

    let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    assert!(workspace.path().join(".foco").is_dir());
    assert!(workspace_database_path(workspace.path()).is_file());
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );

    let connection = Connection::open(database.database_path()).expect("open database");
    for table in [
        "workspace_metadata",
        "chats",
        "messages",
        "run_events",
        "tool_calls",
        "tool_results",
        "terminal_sessions",
        "llm_requests",
        "llm_request_events",
        "context_compression_snapshots",
        "code_graph_files",
        "code_graph_symbols",
        "code_graph_edges",
        "code_graph_references",
        "code_graph_imports",
        "code_graph_fts_data",
        "code_graph_fts_index",
        "code_graph_file_hashes",
        "code_graph_parse_status",
        "todo_graphs",
        "hook_runs",
        "memory_sources",
        "memory_facts",
        "memory_fact_sources",
        "memory_edges",
        "memory_fts_data",
        "memory_fts_index",
        "memory_profiles",
        "memory_extraction_jobs",
        "prompt_context_injections",
        "agent_teams",
        "agent_instances",
        "agent_tasks",
        "agent_task_dependencies",
        "agent_messages",
        "agent_attempts",
        "agent_events",
        "agent_context_entries",
        "agent_context_snapshots",
    ] {
        assert!(
            table_exists(&connection, table),
            "{table} table should exist"
        );
    }
}

#[test]
fn workspace_connections_wait_for_concurrent_writer_lock() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let workspace_path = workspace.path().to_path_buf();
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_path).expect("workspace database");
    database
        .insert_chat("chat-1", "Lock test")
        .expect("chat insert");

    let locker = Connection::open(database.database_path()).expect("open locking connection");
    locker
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             BEGIN IMMEDIATE;",
        )
        .expect("hold writer lock");

    let writer = thread::spawn(move || {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_path).expect("writer database");
        database
            .insert_run_event(NewRunEvent {
                id: "event-1",
                chat_id: "chat-1",
                run_id: "run-1",
                sequence: 1,
                event_type: "textDelta",
                payload_json: r#"{"type":"textDelta","delta":"ok"}"#,
            })
            .expect("insert waits for lock");
    });

    thread::sleep(Duration::from_millis(100));
    assert!(!writer.is_finished(), "writer should wait for the lock");
    locker
        .execute_batch("COMMIT;")
        .expect("release writer lock");
    writer.join().expect("writer thread");

    let events = database
        .run_events_for_run("run-1")
        .expect("run events after lock release");
    assert_eq!(events.len(), 1);
}

#[test]
fn initializes_every_registered_workspace() {
    let first = tempfile::tempdir().expect("first workspace");
    let second = tempfile::tempdir().expect("second workspace");
    let workspaces = vec![
        WorkspaceConfig {
            id: "first".to_string(),
            name: "First".to_string(),
            path: first.path().to_path_buf(),
            pinned: false,
            terminal_shell: foco_store::config::DEFAULT_TERMINAL_SHELL.to_string(),
            common_commands: Vec::new(),
        },
        WorkspaceConfig {
            id: "second".to_string(),
            name: "Second".to_string(),
            path: second.path().to_path_buf(),
            pinned: false,
            terminal_shell: foco_store::config::DEFAULT_TERMINAL_SHELL.to_string(),
            common_commands: Vec::new(),
        },
    ];

    let initialized = initialize_workspace_databases(&workspaces).expect("initialize workspaces");

    assert_eq!(initialized.len(), 2);
    assert!(workspace_database_path(first.path()).is_file());
    assert!(workspace_database_path(second.path()).is_file());
}

#[test]
fn backs_up_existing_database_before_migration() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());

    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("old database");
    connection
        .execute_batch(
            "CREATE TABLE legacy_data (id INTEGER PRIMARY KEY);
             INSERT INTO legacy_data DEFAULT VALUES;
             PRAGMA user_version = 0;",
        )
        .expect("old schema");
    drop(connection);

    let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("migrated database");

    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );

    let backup_dir = workspace.path().join(".foco").join("backups");
    let backups = fs::read_dir(&backup_dir)
        .expect("backup directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("backup entries");
    assert_eq!(backups.len(), 1);
    assert!(backups[0].path().is_file());
}

#[test]
fn migrates_v7_task_graphs_table_to_todo_graphs() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let legacy_tasks = serde_json::to_string(&vec![todo_graph_task(
        "plan",
        "Plan work",
        "ready",
        vec![],
        vec!["Plan is clear"],
        "Legacy row",
        vec![],
    )])
    .expect("legacy graph json");
    let connection = Connection::open(&database_path).expect("old database");
    connection
        .execute_batch(
            "CREATE TABLE chats (
                id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
                title TEXT NOT NULL CHECK (length(title) > 0),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE task_graphs (
                chat_id TEXT PRIMARY KEY NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
                graph_json TEXT NOT NULL CHECK (length(graph_json) > 0),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX task_graphs_updated_at_idx ON task_graphs (updated_at);
            CREATE TABLE llm_requests (
                id TEXT PRIMARY KEY NOT NULL,
                chat_id TEXT REFERENCES chats(id) ON DELETE SET NULL,
                provider_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                request_started_at TEXT NOT NULL,
                final_state TEXT NOT NULL
            );
            INSERT INTO chats (id, title, created_at, updated_at)
                VALUES ('chat-1', 'Legacy todo graph', '2026-06-10T00:00:00Z', '2026-06-10T00:00:00Z');
            PRAGMA user_version = 7;",
        )
        .expect("old todo graph schema");
    connection
        .execute(
            "INSERT INTO task_graphs (chat_id, graph_json, created_at, updated_at)
             VALUES ('chat-1', ?1, '2026-06-10T00:00:00Z', '2026-06-10T00:00:00Z')",
            params![legacy_tasks],
        )
        .expect("legacy todo graph row");
    drop(connection);

    let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let connection = Connection::open(database.database_path()).expect("open migrated database");
    assert!(table_exists(&connection, "todo_graphs"));
    assert!(!table_exists(&connection, "task_graphs"));

    let graph = database
        .todo_graph("chat-1")
        .expect("read migrated todo graph")
        .expect("migrated todo graph");
    assert_eq!(graph.tasks[0].id, "plan");
}

#[test]
fn chat_memory_facts_cascade_when_chat_is_deleted() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "Memory chat")
        .expect("chat insert");

    {
        let connection = Connection::open(database.database_path()).expect("open database");
        connection
            .pragma_update(None, "foreign_keys", true)
            .expect("enable foreign keys");
        connection
            .execute_batch(
                "INSERT INTO memory_sources
                    (id, scope, chat_id, source_type, source_id, title, content, metadata_json, created_at, updated_at)
                 VALUES
                    ('source-1', 'chat', 'chat-1', 'manual_note', NULL, 'Note', 'Remember this session fact.', '{}', '2026-06-09T00:00:00Z', '2026-06-09T00:00:00Z');
                 INSERT INTO memory_facts
                    (id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest, metadata_json, created_at, updated_at)
                 VALUES
                    ('fact-1', 'chat', 'chat-1', 'active', 'user_note', 'Remember this session fact.', 1.0, 0, 1, '{}', '2026-06-09T00:00:00Z', '2026-06-09T00:00:00Z');
                 INSERT INTO memory_fact_sources (fact_id, source_id)
                 VALUES ('fact-1', 'source-1');
                 INSERT INTO memory_fts_data
                    (fact_id, scope, chat_id, status, kind, title, body, updated_at)
                 VALUES
                    ('fact-1', 'chat', 'chat-1', 'active', 'user_note', 'user_note', 'Remember this session fact.', '2026-06-09T00:00:00Z');",
            )
            .expect("memory rows");
        assert_eq!(table_count(&connection, "memory_facts"), 1);
        assert_eq!(table_count(&connection, "memory_fts_index"), 1);
    }

    assert!(database.delete_chat("chat-1").expect("chat delete"));

    let connection = Connection::open(database.database_path()).expect("open database");
    assert_eq!(table_count(&connection, "memory_facts"), 0);
    assert_eq!(table_count(&connection, "memory_fact_sources"), 0);
    assert_eq!(table_count(&connection, "memory_fts_data"), 0);
    assert_eq!(table_count(&connection, "memory_fts_index"), 0);
}

#[test]
fn chat_statistics_memory_sources_follow_message_and_tool_references() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "Statistics chat")
        .expect("chat insert");
    database
        .insert_chat("chat-2", "Other chat")
        .expect("second chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Read the file.",
            sequence: 0,
            metadata_json: Some("{}"),
        })
        .expect("assistant message insert");
    database
        .insert_message(NewMessage {
            id: "assistant-2",
            chat_id: "chat-2",
            role: "assistant",
            content: "Other chat.",
            sequence: 0,
            metadata_json: Some("{}"),
        })
        .expect("other assistant message insert");
    database
        .insert_tool_call(NewToolCall {
            id: "tool-call-1",
            chat_id: "chat-1",
            run_id: "run-1",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"README.md"}"#,
            status: "completed",
            started_at: "2026-06-10T00:00:00Z",
            completed_at: Some("2026-06-10T00:00:01Z"),
        })
        .expect("tool call insert");

    let tool_counts = database
        .tool_call_counts_for_chat("chat-1")
        .expect("tool count");
    assert_eq!(tool_counts.len(), 1);
    assert_eq!(tool_counts[0].tool_name, "read_file");
    assert_eq!(tool_counts[0].call_count, 1);
    drop(database);

    let mut memory = MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
        .expect("memory database");
    for (source_id, source_type, source_ref, content) in [
        (
            "source-message",
            MemorySourceType::AssistantMessage,
            "assistant-1",
            "Assistant evidence.",
        ),
        (
            "source-tool",
            MemorySourceType::ToolCall,
            "tool-call-1",
            "Tool evidence.",
        ),
        (
            "source-other",
            MemorySourceType::AssistantMessage,
            "assistant-2",
            "Other evidence.",
        ),
    ] {
        memory
            .insert_source(NewMemorySource {
                id: source_id,
                scope: MemoryScope::Workspace,
                chat_id: None,
                source_type,
                source_id: Some(source_ref),
                title: source_id,
                content,
                metadata_json: "{}",
            })
            .expect("memory source insert");
    }
    for (fact_id, source_id, fact) in [
        (
            "fact-message",
            "source-message",
            "Remember assistant evidence.",
        ),
        ("fact-tool", "source-tool", "Remember tool evidence."),
        ("fact-other", "source-other", "Remember other evidence."),
    ] {
        memory
            .insert_fact(NewMemoryFact {
                id: fact_id,
                scope: MemoryScope::Workspace,
                chat_id: None,
                status: MemoryStatus::Active,
                kind: MemoryKind::ProjectFact,
                fact,
                confidence: Some(1.0),
                pinned: false,
                source_ids: &[source_id],
                metadata_json: "{}",
            })
            .expect("memory fact insert");
    }

    let fact_ids = memory
        .facts_created_from_chat_sources("chat-1")
        .expect("chat source facts")
        .into_iter()
        .map(|fact| fact.id)
        .collect::<Vec<_>>();
    assert_eq!(fact_ids, vec!["fact-message", "fact-tool"]);
}

#[test]
fn clears_completed_queued_run_metadata_from_chat_and_user_message() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .insert_chat_with_metadata(
            "chat-queued",
            "Queued chat",
            r#"{"queuedRun":{"status":"queued","userMessageId":"user-queued","modelId":"model","providerId":"provider","content":"hello"}}"#,
        )
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "user-queued",
            chat_id: "chat-queued",
            role: "user",
            content: "hello",
            sequence: 0,
            metadata_json: Some(
                r#"{"queuedRun":{"status":"queued","modelId":"model","providerId":"provider"}}"#,
            ),
        })
        .expect("message insert");

    database
        .mark_chat_queued_run_started("chat-queued", "user-queued", "assistant-queued", 1)
        .expect("queued run started");
    let running_chat_metadata: Value = serde_json::from_str(
        &database
            .chat("chat-queued")
            .expect("chat read")
            .expect("chat")
            .metadata_json,
    )
    .expect("chat metadata json");
    assert_eq!(running_chat_metadata["queuedRun"]["status"], "running");
    assert_eq!(
        running_chat_metadata["queuedRun"]["assistantMessageId"],
        "assistant-queued"
    );
    assert_eq!(running_chat_metadata["queuedRun"]["assistantSequence"], 1);

    database
        .clear_chat_queued_run("chat-queued", "user-queued")
        .expect("clear queued run");
    let chat_metadata: Value = serde_json::from_str(
        &database
            .chat("chat-queued")
            .expect("chat read")
            .expect("chat")
            .metadata_json,
    )
    .expect("chat metadata json");
    let message_metadata: Value = serde_json::from_str(
        &database
            .message("user-queued")
            .expect("message read")
            .expect("message")
            .metadata_json,
    )
    .expect("message metadata json");

    assert!(chat_metadata.get("queuedRun").is_none());
    assert!(message_metadata.get("queuedRun").is_none());
}

#[test]
fn repository_helpers_round_trip_todo_graphs() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "ToDo graph chat")
        .expect("chat insert");
    let graph = database
        .upsert_todo_graph(
            "chat-1",
            vec![todo_graph_task(
                "plan",
                "Plan work",
                "ready",
                vec![],
                vec!["Plan is clear"],
                "Find the smallest path.",
                vec![todo_graph_task(
                    "probe",
                    "Probe code",
                    "pending",
                    vec!["plan"],
                    vec!["Entrypoints identified"],
                    "",
                    vec![],
                )],
            )],
        )
        .expect("todo graph create");

    assert_eq!(graph.chat_id, "chat-1");
    assert_eq!(graph.tasks.len(), 1);
    assert_eq!(graph.tasks[0].created_at, graph.tasks[0].updated_at);
    assert_eq!(graph.tasks[0].subtasks[0].depends_on, vec!["plan"]);

    let updated = database
        .update_todo_graph_task(
            "chat-1",
            "probe",
            TodoGraphTaskPatch {
                status: Some("completed".to_string()),
                summary: Some("Found store, tools, app, and web entrypoints.".to_string()),
                ..TodoGraphTaskPatch::default()
            },
        )
        .expect("task patch");
    let updated_task = updated.updated_task.expect("updated task");
    assert_eq!(updated_task.id, "probe");
    assert_eq!(updated_task.status, "completed");
    assert_eq!(
        updated_task.summary,
        "Found store, tools, app, and web entrypoints."
    );

    let completed = database
        .filtered_todo_graph(
            "chat-1",
            TodoGraphFilter {
                status: Some("completed"),
                task_id: None,
                include_subtasks: false,
            },
        )
        .expect("filtered todo graph")
        .expect("todo graph");
    assert_eq!(completed.tasks.len(), 1);
    assert_eq!(completed.tasks[0].id, "probe");
    assert!(completed.tasks[0].subtasks.is_empty());
}

#[test]
fn repository_helpers_reject_invalid_todo_graph_dependencies() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "ToDo graph chat")
        .expect("chat insert");

    let missing = database
        .upsert_todo_graph(
            "chat-1",
            vec![todo_graph_task(
                "build",
                "Build feature",
                "pending",
                vec!["missing"],
                vec![],
                "",
                vec![],
            )],
        )
        .expect_err("missing dependency should fail")
        .to_string();
    assert!(missing.contains("depends on missing task"));

    let cycle = database
        .upsert_todo_graph(
            "chat-1",
            vec![
                todo_graph_task(
                    "first",
                    "First",
                    "pending",
                    vec!["second"],
                    vec![],
                    "",
                    vec![],
                ),
                todo_graph_task(
                    "second",
                    "Second",
                    "pending",
                    vec!["first"],
                    vec![],
                    "",
                    vec![],
                ),
            ],
        )
        .expect_err("cycle should fail")
        .to_string();
    assert!(cycle.contains("cycle"));
}

#[test]
fn repository_helpers_round_trip_core_records() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .set_workspace_metadata("active_chat", "chat-1")
        .expect("metadata write");
    assert_eq!(
        database
            .workspace_metadata("active_chat")
            .expect("metadata read"),
        Some("chat-1".to_string())
    );

    database
        .insert_chat("chat-1", "First chat")
        .expect("chat insert");
    database
        .insert_chat("chat-2", "Second chat")
        .expect("second chat insert");
    assert_eq!(
        database
            .chat("chat-1")
            .expect("chat read")
            .expect("chat")
            .title,
        "First chat"
    );
    let chats = database.chats().expect("chat list");
    assert_eq!(chats.len(), 2);
    assert_eq!(chats[0].title, "Second chat");
    assert_eq!(chats[1].title, "First chat");

    database
        .insert_message(NewMessage {
            id: "message-1",
            chat_id: "chat-1",
            role: "user",
            content: "Hello",
            sequence: 0,
            metadata_json: None,
        })
        .expect("message insert");
    let messages = database
        .messages_for_chat("chat-1")
        .expect("messages for chat");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Hello");

    database
        .upsert_message_content(NewMessage {
            id: "message-1",
            chat_id: "chat-1",
            role: "user",
            content: "Hello again",
            sequence: 0,
            metadata_json: Some(r#"{"draft":true}"#),
        })
        .expect("message upsert update");
    let messages = database
        .messages_for_chat("chat-1")
        .expect("messages for chat after upsert");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Hello again");
    assert_eq!(messages[0].metadata_json, r#"{"draft":true}"#);

    database
        .upsert_message_content(NewMessage {
            id: "message-2",
            chat_id: "chat-1",
            role: "assistant",
            content: "Streaming reply",
            sequence: 1,
            metadata_json: None,
        })
        .expect("message upsert insert");
    let messages = database
        .messages_for_chat("chat-1")
        .expect("messages for chat after inserted upsert");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content, "Streaming reply");
    database
        .update_message_metadata(
            "message-2",
            r#"{"parts":[{"type":"text","text":"Streaming reply"}]}"#,
        )
        .expect("message metadata update");
    let updated_message = database
        .message("message-2")
        .expect("updated message read")
        .expect("updated message");
    assert!(updated_message.metadata_json.contains("Streaming reply"));

    database
        .insert_run_event(NewRunEvent {
            id: "event-1",
            chat_id: "chat-1",
            run_id: "run-1",
            sequence: 0,
            event_type: "started",
            payload_json: "{}",
        })
        .expect("run event insert");
    let run_events = database
        .run_events_for_run("run-1")
        .expect("run events for run");
    assert_eq!(run_events.len(), 1);
    assert_eq!(run_events[0].event_type, "started");

    database
        .insert_llm_request(NewLlmRequest {
            id: "request-1",
            workspace_id: "workspace-1",
            chat_id: Some("chat-1"),
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            request_started_at: "2026-06-03T10:00:00.000Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: Some(3),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(200),
            final_state: "completed",
            request_body_json: Some(r#"{"input":"Hello"}"#),
            response_body_json: Some(r#"{"output":"Hi"}"#),
        })
        .expect("llm request insert");
    let request: LlmRequestRecord = database
        .llm_request("request-1")
        .expect("llm request read")
        .expect("llm request");
    assert_eq!(request.provider_id, "openai");
    assert_eq!(request.input_tokens, Some(3));
    assert_eq!(request.final_state, "completed");
    let metrics = database
        .llm_request_metrics_for_chat("chat-1")
        .expect("chat request metrics");
    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics[0].id, "request-1");
    assert_eq!(metrics[0].output_tokens, Some(5));

    database
        .insert_context_compression_snapshot(NewContextCompressionSnapshot {
            id: "snapshot-1",
            chat_id: "chat-1",
            run_id: "request-1",
            sequence: 0,
            summary: "Earlier conversation summary.",
            source_message_start_sequence: 0,
            source_message_end_sequence: 0,
            original_token_count: 120,
            summary_token_count: 8,
            metadata_json: Some(r#"{"reason":"test"}"#),
        })
        .expect("context compression snapshot insert");
    let snapshots = database
        .context_compression_snapshots_for_chat("chat-1")
        .expect("context compression snapshots");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].summary, "Earlier conversation summary.");
    assert_eq!(snapshots[0].original_token_count, 120);
    assert_eq!(snapshots[0].summary_token_count, 8);
}

#[test]
fn repository_helpers_delete_chat_cascades_chat_state_and_preserves_audit() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "Deleted chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "message-1",
            chat_id: "chat-1",
            role: "user",
            content: "Hello",
            sequence: 0,
            metadata_json: None,
        })
        .expect("message insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Tool calls completed.",
            sequence: 1,
            metadata_json: None,
        })
        .expect("assistant message insert");
    database
        .insert_run_event(NewRunEvent {
            id: "event-1",
            chat_id: "chat-1",
            run_id: "run-1",
            sequence: 0,
            event_type: "started",
            payload_json: "{}",
        })
        .expect("run event insert");
    database
        .insert_context_compression_snapshot(NewContextCompressionSnapshot {
            id: "snapshot-1",
            chat_id: "chat-1",
            run_id: "run-1",
            sequence: 0,
            summary: "Earlier conversation summary.",
            source_message_start_sequence: 0,
            source_message_end_sequence: 0,
            original_token_count: 120,
            summary_token_count: 8,
            metadata_json: None,
        })
        .expect("context compression snapshot insert");
    database
        .insert_tool_call(NewToolCall {
            id: "tool-call-1",
            chat_id: "chat-1",
            run_id: "run-1",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"README.md"}"#,
            status: "completed",
            started_at: "2026-06-03T10:00:00.000Z",
            completed_at: Some("2026-06-03T10:00:00.100Z"),
        })
        .expect("tool call insert");
    database
        .insert_tool_result(NewToolResult {
            id: "tool-result-1",
            tool_call_id: "tool-call-1",
            output_json: r#"{"content":"hello"}"#,
            is_error: false,
            created_at: "2026-06-03T10:00:00.100Z",
        })
        .expect("tool result insert");
    database
        .insert_llm_request(NewLlmRequest {
            id: "request-1",
            workspace_id: "workspace-1",
            chat_id: Some("chat-1"),
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            request_started_at: "2026-06-03T10:00:00.000Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: Some(3),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(200),
            final_state: "completed",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("llm request insert");

    assert!(database.delete_chat("chat-1").expect("chat delete"));
    assert_eq!(database.chat("chat-1").expect("chat read"), None);
    assert!(
        database
            .messages_for_chat("chat-1")
            .expect("messages for chat")
            .is_empty()
    );
    assert!(
        database
            .run_events_for_run("run-1")
            .expect("run events for run")
            .is_empty()
    );
    assert!(
        database
            .context_compression_snapshots_for_chat("chat-1")
            .expect("context compression snapshots")
            .is_empty()
    );
    assert!(
        database
            .tool_calls_for_message("assistant-1")
            .expect("tool calls for message")
            .is_empty()
    );
    let connection = Connection::open(database.database_path()).expect("open database");
    let remaining_tool_results: i64 = connection
        .query_row("SELECT COUNT(*) FROM tool_results", [], |row| row.get(0))
        .expect("tool result count");
    assert_eq!(remaining_tool_results, 0);
    let request = database
        .llm_request("request-1")
        .expect("llm request read")
        .expect("llm request");
    assert_eq!(request.chat_id, None);
    assert!(!database.delete_chat("chat-1").expect("second delete"));
}

#[test]
fn repository_helpers_persist_terminal_working_directory() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
    let first_directory = workspace.path().display().to_string();
    let second_directory = workspace.path().join("nested").display().to_string();

    database
        .upsert_terminal_session(NewTerminalSession {
            id: "terminal-1",
            name: "Workspace Terminal",
            working_directory: &first_directory,
            metadata_json: None,
        })
        .expect("terminal session insert");

    let session = database
        .latest_terminal_session()
        .expect("latest terminal session")
        .expect("terminal session");
    assert_eq!(session.id, "terminal-1");
    assert_eq!(session.working_directory, first_directory);
    assert_eq!(session.closed_at, None);

    database
        .update_terminal_working_directory("terminal-1", &second_directory)
        .expect("terminal cwd update");
    let session = database
        .latest_terminal_session()
        .expect("latest terminal session after cwd")
        .expect("terminal session after cwd");
    assert_eq!(session.working_directory, second_directory);

    database
        .close_terminal_session("terminal-1")
        .expect("terminal close");
    assert_eq!(
        database
            .latest_terminal_session()
            .expect("latest terminal after close"),
        None
    );
}

#[test]
fn repository_helpers_round_trip_tool_calls_and_results() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "Tool chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Tool calls completed.",
            sequence: 0,
            metadata_json: None,
        })
        .expect("assistant message insert");
    database
        .upsert_tool_call(NewToolCall {
            id: "tool-call-1",
            chat_id: "chat-1",
            run_id: "run-1",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"README.md","apiKey":"secret-value"}"#,
            status: "running",
            started_at: "2026-06-03T10:00:00.000Z",
            completed_at: None,
        })
        .expect("running tool call upsert");
    database
        .upsert_tool_call(NewToolCall {
            id: "tool-call-1",
            chat_id: "chat-1",
            run_id: "run-1",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"README.md","apiKey":"secret-value"}"#,
            status: "completed",
            started_at: "2026-06-03T10:00:00.000Z",
            completed_at: Some("2026-06-03T10:00:00.100Z"),
        })
        .expect("completed tool call upsert");
    database
        .upsert_tool_result(NewToolResult {
            id: "tool-result-1",
            tool_call_id: "tool-call-1",
            output_json: r#"{"content":"hello","authorization":"Bearer secret"}"#,
            is_error: false,
            created_at: "2026-06-03T10:00:00.100Z",
        })
        .expect("tool result upsert");
    database
        .upsert_tool_call(NewToolCall {
            id: "tool-call-incomplete",
            chat_id: "chat-1",
            run_id: "run-1",
            message_id: Some("assistant-1"),
            tool_name: "run_command",
            input_json: r#"{"command":"git status"}"#,
            status: "completed",
            started_at: "2026-06-03T10:00:00.200Z",
            completed_at: Some("2026-06-03T10:00:00.300Z"),
        })
        .expect("incomplete tool call upsert");
    database
        .delete_incomplete_tool_calls_for_run("run-1")
        .expect("delete incomplete tool calls");

    let records = database
        .tool_calls_for_message("assistant-1")
        .expect("tool calls for message");
    let chat_records = database
        .tool_calls_for_chat("chat-1")
        .expect("tool calls for chat");

    assert_eq!(records.len(), 1);
    assert_eq!(chat_records, records);
    assert_eq!(records[0].id, "tool-call-1");
    assert_eq!(records[0].tool_name, "read_file");
    assert_eq!(records[0].status, "completed");
    assert_eq!(records[0].message_id.as_deref(), Some("assistant-1"));
    let input: Value = serde_json::from_str(&records[0].input_json).expect("input json");
    assert_eq!(input["path"], "README.md");
    assert_eq!(input["apiKey"], "[REDACTED]");

    let result = records[0].result.as_ref().expect("tool result");
    assert!(!result.is_error);
    let output: Value = serde_json::from_str(&result.output_json).expect("output json");
    assert_eq!(output["content"], "hello");
    assert_eq!(output["authorization"], "[REDACTED]");
}

#[test]
fn upsert_tool_call_overwrites_incomplete_stub_with_different_run_or_input() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "Tool chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Tool calls.",
            sequence: 0,
            metadata_json: None,
        })
        .expect("assistant message insert");

    // A prior run persisted a tool call stub that was cancelled before its
    // result arrived, leaving an incomplete row under the old run id.
    database
        .upsert_tool_call(NewToolCall {
            id: "call-stub",
            chat_id: "chat-1",
            run_id: "run-old",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"OLD.md"}"#,
            status: "cancelled",
            started_at: "2026-06-18T14:10:00.000Z",
            completed_at: Some("2026-06-18T14:10:05.000Z"),
        })
        .expect("cancelled stub upsert");

    // The new run reuses the same provider call id with a different run and
    // different input. Because the stub has no tool result, it must be
    // overwritten rather than rejected.
    database
        .upsert_tool_call(NewToolCall {
            id: "call-stub",
            chat_id: "chat-1",
            run_id: "run-new",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"NEW.md"}"#,
            status: "running",
            started_at: "2026-06-18T14:17:00.000Z",
            completed_at: None,
        })
        .expect("overwrite incomplete stub");

    let records = database
        .tool_calls_for_chat("chat-1")
        .expect("tool calls for chat");
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.run_id, "run-new");
    assert_eq!(record.status, "running");
    let input: Value = serde_json::from_str(&record.input_json).expect("input json");
    assert_eq!(input["path"], "NEW.md");
    assert!(record.result.is_none());
}

#[test]
fn upsert_tool_call_rejects_overwrite_of_call_with_completed_result() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "Tool chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Tool calls.",
            sequence: 0,
            metadata_json: None,
        })
        .expect("assistant message insert");

    // A genuinely completed tool call (has a tool result) is audit history and
    // must not be clobbered by a later attempt with a different run or input.
    database
        .upsert_tool_call(NewToolCall {
            id: "call-done",
            chat_id: "chat-1",
            run_id: "run-old",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"README.md"}"#,
            status: "completed",
            started_at: "2026-06-18T14:10:00.000Z",
            completed_at: Some("2026-06-18T14:10:01.000Z"),
        })
        .expect("completed tool call upsert");
    database
        .upsert_tool_result(NewToolResult {
            id: "call-done-result",
            tool_call_id: "call-done",
            output_json: r#"{"content":"hello"}"#,
            is_error: false,
            created_at: "2026-06-18T14:10:01.000Z",
        })
        .expect("tool result upsert");

    let error = database
        .upsert_tool_call(NewToolCall {
            id: "call-done",
            chat_id: "chat-1",
            run_id: "run-new",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"DIFFERENT.md"}"#,
            status: "running",
            started_at: "2026-06-18T14:17:00.000Z",
            completed_at: None,
        })
        .expect_err("overwrite of completed tool call must be rejected");
    assert!(
        matches!(error, WorkspaceDatabaseError::InvalidToolCall { .. }),
        "expected InvalidToolCall, got {error:?}"
    );

    let records = database
        .tool_calls_for_chat("chat-1")
        .expect("tool calls for chat");
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.run_id, "run-old");
    assert_eq!(record.status, "completed");
    let input: Value = serde_json::from_str(&record.input_json).expect("input json");
    assert_eq!(input["path"], "README.md");
    assert!(record.result.is_some());
}

#[test]
fn upsert_tool_call_promotes_status_for_completed_call_with_matching_identity() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "Tool chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Tool calls.",
            sequence: 0,
            metadata_json: None,
        })
        .expect("assistant message insert");

    // The streaming path writes the call as running under the chat run id.
    database
        .upsert_tool_call(NewToolCall {
            id: "call-promote",
            chat_id: "chat-1",
            run_id: "run-1",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"README.md"}"#,
            status: "running",
            started_at: "2026-06-18T14:10:00.000Z",
            completed_at: None,
        })
        .expect("running tool call upsert");
    database
        .upsert_tool_result(NewToolResult {
            id: "call-promote-result",
            tool_call_id: "call-promote",
            output_json: r#"{"content":"hello"}"#,
            is_error: false,
            created_at: "2026-06-18T14:10:01.000Z",
        })
        .expect("tool result upsert");
    // The finalize path re-upserts the same call (same chat, run, name, input)
    // to promote its status to completed even though a result now exists.
    database
        .upsert_tool_call(NewToolCall {
            id: "call-promote",
            chat_id: "chat-1",
            run_id: "run-1",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"README.md"}"#,
            status: "completed",
            started_at: "2026-06-18T14:10:00.000Z",
            completed_at: Some("2026-06-18T14:10:01.000Z"),
        })
        .expect("identity-matched status promotion");

    let records = database
        .tool_calls_for_chat("chat-1")
        .expect("tool calls for chat");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].status, "completed");
    assert!(records[0].result.is_some());
}

#[test]
fn code_graph_query_helpers_return_compact_relationships() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
    let lib_symbols = [
        NewCodeGraphSymbol {
            name: "public_api",
            kind: "function",
            start_line: Some(1),
            start_column: Some(1),
            end_line: Some(5),
            end_column: Some(1),
            signature: Some("fn public_api()"),
            documentation: None,
        },
        NewCodeGraphSymbol {
            name: "helper",
            kind: "function",
            start_line: Some(7),
            start_column: Some(1),
            end_line: Some(9),
            end_column: Some(1),
            signature: Some("fn helper()"),
            documentation: None,
        },
    ];
    let lib_imports = [NewCodeGraphImport {
        module: "crate::shared",
        imported_symbol: None,
        alias: None,
        start_line: Some(0),
        start_column: Some(0),
    }];
    let lib_references = [NewCodeGraphReference {
        name: "helper",
        symbol_index: Some(1),
        start_line: Some(3),
        start_column: Some(5),
        end_line: Some(3),
        end_column: Some(11),
    }];
    let lib_edges = [NewCodeGraphEdge {
        source_symbol_index: 0,
        target_symbol_index: 1,
        edge_kind: "references",
        metadata_json: None,
    }];
    database
        .replace_code_graph_file_index(NewCodeGraphFileIndex {
            path: "lib.rs",
            language: Some("rust"),
            size_bytes: Some(64),
            modified_at: Some("2026-06-04T00:00:00.000Z"),
            content_hash: "lib-hash",
            parse_status: "parsed",
            parse_error_message: None,
            symbols: &lib_symbols,
            imports: &lib_imports,
            references: &lib_references,
            edges: &lib_edges,
            fts_body: "fn public_api() { helper(); } fn helper() {}",
        })
        .expect("lib graph index");
    let caller_symbols = [NewCodeGraphSymbol {
        name: "caller_entry",
        kind: "function",
        start_line: Some(1),
        start_column: Some(1),
        end_line: Some(3),
        end_column: Some(1),
        signature: Some("fn caller_entry()"),
        documentation: None,
    }];
    let caller_imports = [NewCodeGraphImport {
        module: "crate::shared",
        imported_symbol: None,
        alias: None,
        start_line: Some(0),
        start_column: Some(0),
    }];
    database
        .replace_code_graph_file_index(NewCodeGraphFileIndex {
            path: "caller.rs",
            language: Some("rust"),
            size_bytes: Some(32),
            modified_at: Some("2026-06-04T00:00:00.000Z"),
            content_hash: "caller-hash",
            parse_status: "parsed",
            parse_error_message: None,
            symbols: &caller_symbols,
            imports: &caller_imports,
            references: &[],
            edges: &[],
            fts_body: "fn caller_entry() {}",
        })
        .expect("caller graph index");

    let context = database.code_graph_context().expect("graph context");
    assert_eq!(context.indexed_files, 2);
    assert_eq!(context.symbols, 3);
    assert_eq!(context.languages, vec!["rust"]);

    let symbols = database
        .find_code_graph_symbols("helper", Some("function"), None, 10)
        .expect("find symbols");
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].path, "lib.rs");
    let helper_id = symbols[0].id;

    let public_api = database
        .find_code_graph_symbols("public_api", None, Some("lib.rs"), 10)
        .expect("find public_api")
        .pop()
        .expect("public_api symbol");
    let callees = database
        .code_graph_callees(public_api.id, 10)
        .expect("callees");
    assert_eq!(callees.len(), 1);
    assert_eq!(callees[0].target.name, "helper");

    let callers = database.code_graph_callers(helper_id, 10).expect("callers");
    assert_eq!(callers.len(), 1);
    assert_eq!(callers[0].source.name, "public_api");

    let references = database
        .code_graph_references(helper_id, 10)
        .expect("references");
    assert_eq!(references.len(), 1);
    assert_eq!(references[0].path, "lib.rs");
    assert_eq!(
        references[0].symbol.as_ref().expect("target symbol").name,
        "helper"
    );

    let related_files = database
        .code_graph_related_files("lib.rs", 10)
        .expect("related files");
    assert_eq!(related_files.len(), 1);
    assert_eq!(related_files[0].path, "caller.rs");
    assert_eq!(related_files[0].relation, "shared_import");
}

#[test]
fn replacing_code_graph_file_index_clears_old_fts_entries() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
    let old_symbols = [
        NewCodeGraphSymbol {
            name: "kept_helper",
            kind: "function",
            start_line: Some(1),
            start_column: Some(1),
            end_line: Some(3),
            end_column: Some(1),
            signature: Some("fn kept_helper()"),
            documentation: None,
        },
        NewCodeGraphSymbol {
            name: "removed_helper",
            kind: "function",
            start_line: Some(5),
            start_column: Some(1),
            end_line: Some(7),
            end_column: Some(1),
            signature: Some("fn removed_helper()"),
            documentation: None,
        },
    ];
    database
        .replace_code_graph_file_index(NewCodeGraphFileIndex {
            path: "lib.rs",
            language: Some("rust"),
            size_bytes: Some(64),
            modified_at: Some("2026-06-04T00:00:00.000Z"),
            content_hash: "old-hash",
            parse_status: "parsed",
            parse_error_message: None,
            symbols: &old_symbols,
            imports: &[],
            references: &[],
            edges: &[],
            fts_body: "fn kept_helper() {} fn removed_helper() {}",
        })
        .expect("old graph index");

    let new_symbols = [NewCodeGraphSymbol {
        name: "kept_helper",
        kind: "function",
        start_line: Some(1),
        start_column: Some(1),
        end_line: Some(3),
        end_column: Some(1),
        signature: Some("fn kept_helper()"),
        documentation: None,
    }];
    database
        .replace_code_graph_file_index(NewCodeGraphFileIndex {
            path: "lib.rs",
            language: Some("rust"),
            size_bytes: Some(32),
            modified_at: Some("2026-06-04T00:01:00.000Z"),
            content_hash: "new-hash",
            parse_status: "parsed",
            parse_error_message: None,
            symbols: &new_symbols,
            imports: &[],
            references: &[],
            edges: &[],
            fts_body: "fn kept_helper() {}",
        })
        .expect("new graph index");

    let removed_symbols = database
        .find_code_graph_symbols("removed_helper", None, None, 10)
        .expect("removed symbol lookup");
    assert!(removed_symbols.is_empty());

    let connection = Connection::open(database.database_path()).expect("open database");
    let removed_fts_data_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*)
             FROM code_graph_fts_data
             WHERE entity_kind = 'symbol' AND title = ?1",
            params!["removed_helper"],
            |row| row.get(0),
        )
        .expect("removed fts data count");
    let removed_fts_index_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*)
             FROM code_graph_fts_index
             WHERE entity_kind = 'symbol' AND title = ?1",
            params!["removed_helper"],
            |row| row.get(0),
        )
        .expect("removed fts index count");
    assert_eq!(removed_fts_data_rows, 0);
    assert_eq!(removed_fts_index_rows, 0);
}

#[test]
fn audits_mocked_llm_request_response_and_stream_events() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "Audit chat")
        .expect("chat insert");
    database
        .insert_llm_request(NewLlmRequest {
            id: "request-1",
            workspace_id: "workspace-1",
            chat_id: Some("chat-1"),
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai-responses",
            model_id: "gpt-audit",
            request_started_at: "2026-06-03T10:00:00.000Z",
            first_token_at: Some("2026-06-03T10:00:00.250Z"),
            completed_at: Some("2026-06-03T10:00:01.500Z"),
            input_tokens: Some(100),
            output_tokens: Some(25),
            cache_read_tokens: Some(40),
            cache_write_tokens: Some(10),
            first_token_latency_ms: Some(250),
            total_latency_ms: Some(1500),
            status_code: Some(200),
            final_state: "completed",
            request_body_json: Some(
                r#"{
                    "headers": {
                        "Authorization": "Bearer secret-token",
                        "OpenAI-Api-Key": "request-key"
                    },
                    "body": {
                        "model": "gpt-audit",
                        "input": "Hello"
                    }
                }"#,
            ),
            response_body_json: Some(
                r#"{
                    "status": 200,
                    "headers": {
                        "x-api-key": "response-key"
                    },
                    "body": {
                        "output": "Hi"
                    }
                }"#,
            ),
        })
        .expect("llm request insert");

    database
        .insert_llm_request_event(NewLlmRequestEvent {
            id: "event-1",
            llm_request_id: "request-1",
            sequence: 0,
            event_at: "2026-06-03T10:00:00.250Z",
            event_type: "text_delta",
            raw_chunk_json: Some(
                r#"{
                    "headers": {
                        "authorization": "Bearer streamed-secret"
                    },
                    "delta": "H"
                }"#,
            ),
            normalized_event_json: r#"{"type":"text_delta","text":"H"}"#,
        })
        .expect("llm event insert");
    database
        .insert_llm_request_event(NewLlmRequestEvent {
            id: "event-2",
            llm_request_id: "request-1",
            sequence: 1,
            event_at: "2026-06-03T10:00:01.500Z",
            event_type: "usage",
            raw_chunk_json: None,
            normalized_event_json: r#"{"type":"usage","input":100,"output":25}"#,
        })
        .expect("second llm event insert");

    let request: LlmRequestRecord = database
        .llm_request("request-1")
        .expect("llm request read")
        .expect("llm request");
    assert_eq!(request.workspace_id, Some("workspace-1".to_string()));
    assert_eq!(request.chat_id, Some("chat-1".to_string()));
    assert_eq!(request.provider_id, "openai-responses");
    assert_eq!(request.model_id, "gpt-audit");
    assert_eq!(request.request_started_at, "2026-06-03T10:00:00.000Z");
    assert_eq!(request.first_token_latency_ms, Some(250));
    assert_eq!(request.total_latency_ms, Some(1500));
    assert_eq!(request.status_code, Some(200));
    assert_eq!(request.final_state, "completed");
    assert_eq!(request.cache_ratio, Some(0.4));

    let request_body = request
        .request_body_json
        .as_deref()
        .expect("request body json");
    assert!(request_body.contains(r#""Authorization":"[REDACTED]""#));
    assert!(request_body.contains(r#""OpenAI-Api-Key":"[REDACTED]""#));
    assert!(!request_body.contains("secret-token"));
    assert!(!request_body.contains("request-key"));

    let response_body = request
        .response_body_json
        .as_deref()
        .expect("response body json");
    assert!(response_body.contains(r#""x-api-key":"[REDACTED]""#));
    assert!(!response_body.contains("response-key"));

    let events = database
        .llm_request_events("request-1")
        .expect("llm request events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "text_delta");
    assert_json_eq(
        &events[0].normalized_event_json,
        r#"{"type":"text_delta","text":"H"}"#,
    );
    let raw_chunk = events[0].raw_chunk_json.as_deref().expect("raw chunk json");
    assert!(raw_chunk.contains(r#""authorization":"[REDACTED]""#));
    assert!(!raw_chunk.contains("streamed-secret"));
    assert_eq!(events[1].event_type, "usage");

    database
        .insert_llm_request(NewLlmRequest {
            id: "request-2",
            workspace_id: "workspace-1",
            chat_id: None,
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai-chat",
            model_id: "gpt-other",
            request_started_at: "2026-06-03T11:00:00.000Z",
            first_token_at: None,
            completed_at: Some("2026-06-03T11:00:00.250Z"),
            input_tokens: Some(8),
            output_tokens: Some(2),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            first_token_latency_ms: None,
            total_latency_ms: Some(250),
            status_code: None,
            final_state: "failed",
            request_body_json: Some(r#"{"model":"gpt-other"}"#),
            response_body_json: Some(r#"{"error":"boom"}"#),
        })
        .expect("second llm request insert");
    database
        .update_llm_request_outcome(
            "request-2",
            UpdateLlmRequestOutcome {
                first_token_at: Some("2026-06-03T11:00:00.050Z"),
                completed_at: Some("2026-06-03T11:00:00.300Z"),
                input_tokens: Some(10),
                output_tokens: Some(4),
                cache_read_tokens: Some(2),
                cache_write_tokens: Some(1),
                first_token_latency_ms: Some(50),
                total_latency_ms: Some(300),
                status_code: Some(200),
                final_state: "succeeded",
                response_body_json: Some(r#"{"ok":true,"apiKey":"secret"}"#),
            },
        )
        .expect("update llm request outcome");
    let updated_request = database
        .llm_request("request-2")
        .expect("updated request read")
        .expect("updated request");
    assert_eq!(updated_request.final_state, "succeeded");
    assert_eq!(updated_request.status_code, Some(200));
    assert_eq!(updated_request.cache_ratio, Some(0.2));
    assert!(
        updated_request
            .response_body_json
            .as_deref()
            .expect("updated response body")
            .contains(r#""apiKey":"[REDACTED]""#)
    );

    let all_rows = database
        .llm_request_audit_rows(LlmRequestAuditFilters::default())
        .expect("audit rows");
    assert_eq!(all_rows.len(), 2);
    assert_eq!(all_rows[0].id, "request-2");
    assert_eq!(all_rows[1].id, "request-1");
    assert_eq!(
        database
            .llm_request_audit_count(LlmRequestAuditFilters::default())
            .expect("audit count"),
        2
    );
    let empty_summary = database
        .llm_request_audit_summary(LlmRequestAuditFilters {
            final_state: Some("missing"),
            ..LlmRequestAuditFilters::default()
        })
        .expect("empty audit summary");
    assert_eq!(empty_summary.total_requests, 0);
    assert_eq!(empty_summary.total_tokens, 0);

    let second_page_rows = database
        .llm_request_audit_rows(LlmRequestAuditFilters {
            limit: Some(1),
            offset: Some(1),
            ..LlmRequestAuditFilters::default()
        })
        .expect("second page audit rows");
    assert_eq!(second_page_rows.len(), 1);
    assert_eq!(second_page_rows[0].id, "request-1");

    let filtered_rows = database
        .llm_request_audit_rows(LlmRequestAuditFilters {
            workspace_id: Some("workspace-1"),
            chat_id: Some("chat-1"),
            provider_id: Some("openai-responses"),
            model_id: Some("gpt-audit"),
            final_state: Some("completed"),
            started_after: Some("2026-06-03T09:00:00.000Z"),
            started_before: Some("2026-06-03T10:30:00.000Z"),
            limit: Some(1),
            offset: None,
        })
        .expect("filtered audit rows");
    assert_eq!(filtered_rows.len(), 1);
    assert_eq!(filtered_rows[0].id, "request-1");
    assert_eq!(filtered_rows[0].cache_ratio, Some(0.4));
    assert_eq!(
        database
            .llm_request_audit_count(LlmRequestAuditFilters {
                workspace_id: Some("workspace-1"),
                chat_id: Some("chat-1"),
                provider_id: Some("openai-responses"),
                model_id: Some("gpt-audit"),
                final_state: Some("completed"),
                started_after: Some("2026-06-03T09:00:00.000Z"),
                started_before: Some("2026-06-03T10:30:00.000Z"),
                limit: None,
                offset: None,
            })
            .expect("filtered audit count"),
        1
    );
}

#[test]
fn stores_prompt_context_injections_for_chat_replay() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    database
        .insert_chat("chat-1", "Prompt cache chat")
        .expect("chat insert");

    database
        .insert_prompt_context_injection(NewPromptContextInjection {
            id: "stable-1",
            chat_id: "chat-1",
            kind: "stable",
            sequence: None,
            messages_json: r#"[{"role":"system","content":"Stable memory"}]"#,
            memory_keys_json: r#"["workspace:fact-1"]"#,
        })
        .expect("stable injection");
    database
        .insert_prompt_context_injection(NewPromptContextInjection {
            id: "turn-1",
            chat_id: "chat-1",
            kind: "turn_memory",
            sequence: Some(0),
            messages_json: r#"[{"role":"user","content":"Turn memory"}]"#,
            memory_keys_json: r#"["chat:fact-2"]"#,
        })
        .expect("turn injection");

    let injections = database
        .prompt_context_injections_for_chat("chat-1")
        .expect("injections");

    assert_eq!(injections.len(), 2);
    assert_eq!(injections[0].kind, "stable");
    assert_eq!(injections[0].sequence, None);
    assert_eq!(injections[1].kind, "turn_memory");
    assert_eq!(injections[1].sequence, Some(0));
    assert_eq!(injections[1].memory_keys_json, r#"["chat:fact-2"]"#);
}

#[test]
fn migrates_v9_without_creating_teams_for_existing_chats() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("v9 database");
    connection
        .execute_batch(
            "CREATE TABLE chats (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE TABLE llm_requests (
                id TEXT PRIMARY KEY NOT NULL,
                chat_id TEXT REFERENCES chats(id) ON DELETE SET NULL,
                provider_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                request_started_at TEXT NOT NULL,
                final_state TEXT NOT NULL
             );
             INSERT INTO chats (id, title, created_at, updated_at)
                VALUES ('chat-existing', 'Existing', '2026-06-19T00:00:00Z', '2026-06-19T00:00:00Z');
             PRAGMA user_version = 9;",
        )
        .expect("v9 schema");
    for index in 0..500 {
        connection
            .execute(
                "INSERT INTO chats (id, title, created_at, updated_at)
                 VALUES (?1, ?2, '2026-06-19T00:00:00Z', '2026-06-19T00:00:00Z')",
                params![format!("chat-bulk-{index}"), format!("Bulk {index}")],
            )
            .expect("bulk chat insert");
    }
    drop(connection);

    let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let connection = Connection::open(database.database_path()).expect("open migrated database");
    assert_eq!(table_count(&connection, "agent_teams"), 0);
    assert_eq!(table_count(&connection, "chats"), 501);
    assert_no_agent_messages_old_references(&connection);
    let backups = fs::read_dir(workspace.path().join(".foco").join("backups"))
        .expect("backup directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("backup entries");
    assert_eq!(backups.len(), 1);
}

#[test]
fn migrates_v13_agent_message_foreign_keys_to_current_table() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("v13 database");
    connection
        .execute_batch(
            r#"CREATE TABLE agent_messages (
                id TEXT PRIMARY KEY NOT NULL CHECK (id GLOB 'agent-message-*'),
                team_id TEXT NOT NULL,
                UNIQUE (team_id, id)
             );
             CREATE TABLE agent_teams (
                id TEXT PRIMARY KEY NOT NULL
             );
             CREATE TABLE agent_instances (
                id TEXT PRIMARY KEY NOT NULL,
                team_id TEXT NOT NULL,
                UNIQUE (team_id, id)
             );
             CREATE TABLE agent_tasks (
                id TEXT PRIMARY KEY NOT NULL,
                team_id TEXT NOT NULL,
                UNIQUE (team_id, id)
             );
             CREATE TABLE agent_attempts (
                id TEXT PRIMARY KEY NOT NULL,
                team_id TEXT NOT NULL,
                UNIQUE (team_id, id)
             );
             CREATE TABLE agent_events (
                team_id TEXT NOT NULL,
                sequence INTEGER NOT NULL CHECK (sequence >= 0),
                event_type TEXT NOT NULL CHECK (length(event_type) > 0),
                instance_id TEXT,
                task_id TEXT,
                attempt_id TEXT,
                message_id TEXT,
                payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
                created_at TEXT NOT NULL,
                PRIMARY KEY (team_id, sequence),
                FOREIGN KEY (team_id, message_id)
                    REFERENCES "agent_messages_old"(team_id, id) ON DELETE SET NULL
             );
             CREATE INDEX agent_events_entity_idx
                ON agent_events (team_id, instance_id, task_id, sequence);
             CREATE TABLE agent_context_entries (
                id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
                team_id TEXT NOT NULL,
                instance_id TEXT NOT NULL,
                generation INTEGER NOT NULL CHECK (generation >= 0),
                sequence INTEGER NOT NULL CHECK (sequence >= 0),
                role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
                content_json TEXT NOT NULL CHECK (json_valid(content_json)),
                source_task_id TEXT,
                source_message_id TEXT,
                created_at TEXT NOT NULL,
                UNIQUE (team_id, id),
                UNIQUE (instance_id, generation, sequence),
                FOREIGN KEY (team_id, source_message_id)
                    REFERENCES "agent_messages_old"(team_id, id) ON DELETE SET NULL
             );
             CREATE INDEX agent_context_entries_owner_idx
                ON agent_context_entries (instance_id, generation, sequence);
             PRAGMA user_version = 13;"#,
        )
        .expect("v13 stale agent schema");
    drop(connection);

    let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let connection = Connection::open(database.database_path()).expect("open migrated database");
    assert_no_agent_messages_old_references(&connection);
}

#[test]
fn failed_agent_schema_migration_rolls_back_and_preserves_backup() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("v9 database");
    connection
        .execute_batch(
            "CREATE TABLE chats (id TEXT PRIMARY KEY NOT NULL);
             CREATE TABLE llm_requests (
                id TEXT PRIMARY KEY NOT NULL,
                request_started_at TEXT NOT NULL
             );
             CREATE TABLE agent_teams (sentinel TEXT NOT NULL);
             INSERT INTO agent_teams (sentinel) VALUES ('preserve-me');
             PRAGMA user_version = 9;",
        )
        .expect("conflicting v9 schema");
    drop(connection);

    assert!(
        WorkspaceDatabase::open_or_create(workspace.path()).is_err(),
        "migration must fail"
    );
    let connection = Connection::open(&database_path).expect("preserved database");
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .expect("schema version"),
        9
    );
    assert_eq!(
        connection
            .query_row("SELECT sentinel FROM agent_teams", [], |row| {
                row.get::<_, String>(0)
            })
            .expect("sentinel row"),
        "preserve-me"
    );
    assert!(!table_exists(&connection, "agent_instances"));
    let backups = fs::read_dir(workspace.path().join(".foco").join("backups"))
        .expect("backup directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("backup entries");
    assert_eq!(backups.len(), 1);
}

#[test]
fn agent_task_enqueue_sequences_are_unique_and_strictly_increasing() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let workspace_path = workspace.path().to_path_buf();
    let mut database = WorkspaceDatabase::open_or_create(&workspace_path).expect("database");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-agent-sequence", "seq");

    let workers = (0..8)
        .map(|index| {
            let workspace_path = workspace_path.clone();
            let team_id = team_id.clone();
            let instance_id = instance_id.clone();
            thread::spawn(move || {
                let mut database =
                    WorkspaceDatabase::open_or_create(workspace_path).expect("worker database");
                let task_id =
                    AgentTaskId::new(format!("agent-task-sequence-{index}")).expect("task id");
                database
                    .enqueue_agent_task(NewAgentTask {
                        id: &task_id,
                        team_id: &team_id,
                        owner_instance_id: &instance_id,
                        origin_instance_id: None,
                        parent_task_id: None,
                        input_json: "{}",
                    })
                    .expect("enqueue")
                    .sequence
            })
        })
        .collect::<Vec<_>>();
    let mut sequences = workers
        .into_iter()
        .map(|worker| worker.join().expect("worker"))
        .collect::<Vec<_>>();
    sequences.sort_unstable();
    assert_eq!(sequences, (0..8).collect::<Vec<_>>());
}

#[test]
fn two_schedulers_cannot_claim_the_same_agent_task() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let workspace_path = workspace.path().to_path_buf();
    let mut database = WorkspaceDatabase::open_or_create(&workspace_path).expect("database");
    let (team_id, instance_id) = create_test_agent_team(&mut database, "chat-agent-claim", "claim");
    let task_id = AgentTaskId::new("agent-task-claim").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue");

    let schedulers = (0..2)
        .map(|index| {
            let workspace_path = workspace_path.clone();
            let team_id = team_id.clone();
            let task_id = task_id.clone();
            thread::spawn(move || {
                let mut database =
                    WorkspaceDatabase::open_or_create(workspace_path).expect("scheduler database");
                let attempt_id = AgentAttemptId::new(format!("agent-attempt-claim-{index}"))
                    .expect("attempt id");
                database
                    .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
                    .expect("claim")
                    .is_some()
            })
        })
        .collect::<Vec<_>>();
    let claims = schedulers
        .into_iter()
        .map(|scheduler| scheduler.join().expect("scheduler"))
        .filter(|claimed| *claimed)
        .count();
    assert_eq!(claims, 1);
    assert_eq!(
        database
            .startup_agent_reconciliation()
            .expect("reconcile")
            .len(),
        1
    );
}

#[test]
fn messages_for_chat_filters_worker_agent_assistant_messages() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-agent-message-filter", "message-filter");
    let worker_id = create_test_agent_worker(&database, &team_id, "message-filter-worker");
    let worker_task_id =
        AgentTaskId::new("agent-task-message-filter-worker").expect("worker task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &worker_task_id,
            team_id: &team_id,
            owner_instance_id: &worker_id,
            origin_instance_id: Some(&coordinator_id),
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("worker task enqueue");
    database
        .insert_message(NewMessage {
            id: "user-main",
            chat_id: "chat-agent-message-filter",
            role: "user",
            content: "Main request",
            sequence: 0,
            metadata_json: None,
        })
        .expect("user message insert");
    database
        .insert_message(NewMessage {
            id: "assistant-main",
            chat_id: "chat-agent-message-filter",
            role: "assistant",
            content: "Main answer",
            sequence: 1,
            metadata_json: None,
        })
        .expect("main assistant message insert");
    database
        .insert_message(NewMessage {
            id: "assistant-worker",
            chat_id: "chat-agent-message-filter",
            role: "assistant",
            content: "Worker-only answer",
            sequence: 2,
            metadata_json: None,
        })
        .expect("worker assistant message insert");
    database
        .insert_run_event(NewRunEvent {
            id: "worker-run-start",
            chat_id: "chat-agent-message-filter",
            run_id: worker_task_id.as_str(),
            sequence: 0,
            event_type: "start",
            payload_json: r#"{"assistantMessageId":"assistant-worker"}"#,
        })
        .expect("worker start event insert");

    let message_ids = database
        .messages_for_chat("chat-agent-message-filter")
        .expect("messages for chat")
        .into_iter()
        .map(|message| message.id)
        .collect::<Vec<_>>();

    assert_eq!(message_ids, vec!["user-main", "assistant-main"]);
}

#[test]
fn agent_queue_limits_and_team_lifecycle_are_enforced() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-agent-lifecycle", "lifecycle");
    let first_task = AgentTaskId::new("agent-task-lifecycle-first").expect("task id");
    database
        .enqueue_agent_task_with_limits(
            NewAgentTask {
                id: &first_task,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: r#"{"queuedUserMessageId":"message-first"}"#,
            },
            1,
            1,
            1,
        )
        .expect("first enqueue");
    let second_task = AgentTaskId::new("agent-task-lifecycle-second").expect("task id");
    let full_error = database
        .enqueue_agent_task_with_limits(
            NewAgentTask {
                id: &second_task,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: r#"{"queuedUserMessageId":"message-second"}"#,
            },
            1,
            1,
            1,
        )
        .expect_err("queue must reject overflow");
    assert!(full_error.to_string().contains("queue is full"));
    assert!(
        database
            .transition_agent_team_status(&team_id, AgentTeamStatus::Stopped)
            .is_err(),
        "a team with queued work must not stop"
    );
    database
        .transition_agent_team_status(&team_id, AgentTeamStatus::Paused)
        .expect("pause team");
    assert_eq!(
        database
            .agent_instance(&instance_id)
            .expect("instance")
            .expect("instance")
            .status,
        AgentInstanceStatus::Paused
    );
    database
        .transition_agent_team_status(&team_id, AgentTeamStatus::Active)
        .expect("resume team");
    database
        .transition_agent_instance_status(&instance_id, AgentInstanceStatus::Draining)
        .expect("drain queued instance");
    assert_eq!(
        database.runnable_agent_tasks(10).expect("draining queue")[0].id,
        first_task
    );
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &first_task,
            expected_status: AgentTaskStatus::Queued,
            transition: AgentTaskTransition::Cancel,
            result_json: None,
            error_json: Some(r#"{"message":"cancelled"}"#),
            interruption_reason: None,
        })
        .expect("cancel queued task");
    database
        .transition_agent_team_status(&team_id, AgentTeamStatus::Stopped)
        .expect("stop idle team");
}

#[test]
fn agent_instance_context_reset_creates_new_generation_without_deleting_history() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-agent-context-reset", "context-reset");

    database
        .insert_agent_context_entry(NewAgentContextEntry {
            id: "agent-context-entry-reset-old",
            team_id: &team_id,
            instance_id: &instance_id,
            generation: 0,
            sequence: 0,
            role: "assistant",
            content_json: r#"{"summary":"old context"}"#,
            source_task_id: None,
            source_message_id: None,
        })
        .expect("insert old context entry");

    let reset_instance = database
        .reset_agent_instance_context(&instance_id)
        .expect("reset instance context");
    assert_eq!(reset_instance.context_generation, 1);
    assert_eq!(
        database
            .agent_context_entries(&instance_id, 0, -1)
            .expect("old context entries")
            .len(),
        1
    );
    assert!(
        database
            .agent_context_entries(&instance_id, 1, -1)
            .expect("new context entries")
            .is_empty()
    );

    let task_id = AgentTaskId::new("agent-task-context-reset-blocker").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue blocker task");
    assert!(
        database.reset_agent_instance_context(&instance_id).is_err(),
        "context reset must reject instances with queued work"
    );
}

#[test]
fn interrupted_queue_head_requires_explicit_retry_and_keeps_fifo() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, instance_id) = create_test_agent_team(&mut database, "chat-agent-retry", "retry");
    let first_task = AgentTaskId::new("agent-task-retry-first").expect("task id");
    let second_task = AgentTaskId::new("agent-task-retry-second").expect("task id");
    for task_id in [&first_task, &second_task] {
        database
            .enqueue_agent_task(NewAgentTask {
                id: task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue");
    }
    let attempt_id = AgentAttemptId::new("agent-attempt-retry-first").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &first_task, &attempt_id)
        .expect("claim")
        .expect("claimed");
    assert!(
        database
            .runnable_agent_tasks(10)
            .expect("runnable behind active queue head")
            .is_empty(),
        "a second Coordinator task must not run beside the active queue head"
    );
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &first_task,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Interrupt,
            result_json: None,
            error_json: Some(r#"{"message":"restart"}"#),
            interruption_reason: Some("restart"),
        })
        .expect("interrupt");
    database
        .transition_agent_instance_status(&instance_id, AgentInstanceStatus::Paused)
        .expect("pause after interruption");
    assert!(
        database
            .runnable_agent_tasks(10)
            .expect("runnable while paused")
            .is_empty()
    );
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &first_task,
            expected_status: AgentTaskStatus::Interrupted,
            transition: AgentTaskTransition::Retry,
            result_json: None,
            error_json: None,
            interruption_reason: None,
        })
        .expect("retry");
    database
        .transition_agent_instance_status(&instance_id, AgentInstanceStatus::Idle)
        .expect("resume instance");
    let runnable = database.runnable_agent_tasks(10).expect("runnable");
    assert_eq!(runnable.len(), 1);
    assert_eq!(runnable[0].id, first_task);
    let retry_attempt = AgentAttemptId::new("agent-attempt-retry-second").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &first_task, &retry_attempt)
        .expect("retry claim")
        .expect("retry claimed");
    assert_eq!(
        database
            .agent_attempts_for_task(&first_task)
            .expect("attempts")
            .len(),
        2
    );
}

#[test]
fn agent_task_state_updates_are_conditional_and_attempts_are_durable() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, instance_id) = create_test_agent_team(&mut database, "chat-agent-state", "state");
    let task_id = AgentTaskId::new("agent-task-state").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue");
    let first_attempt = AgentAttemptId::new("agent-attempt-state-first").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &first_attempt)
        .expect("claim")
        .expect("claimed task");
    assert!(
        !database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &task_id,
                expected_status: AgentTaskStatus::Queued,
                transition: AgentTaskTransition::Cancel,
                result_json: None,
                error_json: None,
                interruption_reason: None,
            })
            .expect("stale conditional update")
    );
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &task_id,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Wait,
                result_json: None,
                error_json: None,
                interruption_reason: None,
            })
            .expect("wait")
    );
    assert_eq!(
        database
            .agent_task(&task_id)
            .expect("task")
            .expect("task")
            .status,
        AgentTaskStatus::Waiting
    );
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &task_id,
                expected_status: AgentTaskStatus::Waiting,
                transition: AgentTaskTransition::Resume,
                result_json: None,
                error_json: None,
                interruption_reason: None,
            })
            .expect("resume")
    );
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &task_id,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Complete,
                result_json: Some(r#"{"ok":true}"#),
                error_json: None,
                interruption_reason: None,
            })
            .expect("complete")
    );
    assert_eq!(
        database
            .agent_attempts_for_task(&task_id)
            .expect("attempts")[0]
            .status,
        foco_agent::AgentAttemptStatus::Completed
    );
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &task_id,
                expected_status: AgentTaskStatus::Completed,
                transition: AgentTaskTransition::Retry,
                result_json: None,
                error_json: None,
                interruption_reason: None,
            })
            .is_err(),
        "completed tasks are not retryable by the frozen state machine"
    );
    assert!(
        database
            .startup_agent_reconciliation()
            .expect("reconcile")
            .is_empty()
    );
}

#[test]
fn agent_store_rejects_cross_team_references_and_dependency_cycles() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (first_team, first_instance) =
        create_test_agent_team(&mut database, "chat-agent-first", "first");
    let (second_team, second_instance) =
        create_test_agent_team(&mut database, "chat-agent-second", "second");
    let first_task = AgentTaskId::new("agent-task-first").expect("first task");
    let second_task = AgentTaskId::new("agent-task-second").expect("second task");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &first_task,
            team_id: &first_team,
            owner_instance_id: &first_instance,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("first task enqueue");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &second_task,
            team_id: &second_team,
            owner_instance_id: &second_instance,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("second task enqueue");

    let cross_parent_error = database
        .enqueue_agent_task(NewAgentTask {
            id: &AgentTaskId::new("agent-task-cross-parent").expect("cross-parent task id"),
            team_id: &first_team,
            owner_instance_id: &first_instance,
            origin_instance_id: None,
            parent_task_id: Some(&second_task),
            input_json: "{}",
        })
        .expect_err("cross-team parent must fail");
    assert!(matches!(
        cross_parent_error,
        WorkspaceDatabaseError::AgentDomain { ref source }
            if source.code() == AgentDomainErrorCode::CrossTeamReference
    ));
    let cross_dependency_error = database
        .insert_agent_task_dependency(NewAgentTaskDependency {
            team_id: &first_team,
            waiting_task_id: &first_task,
            dependency_task_id: &second_task,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: None,
            deadline_at: None,
        })
        .expect_err("cross-team dependency must fail");
    assert!(matches!(
        cross_dependency_error,
        WorkspaceDatabaseError::AgentDomain { ref source }
            if source.code() == AgentDomainErrorCode::CrossTeamReference
    ));

    let cross_team_error = database
        .insert_agent_message(NewAgentMessage {
            id: &AgentMessageId::new("agent-message-cross-team").expect("message id"),
            team_id: &first_team,
            sender_instance_id: Some(&first_instance),
            receiver_instance_id: &second_instance,
            related_task_id: None,
            reply_to_message_id: None,
            kind: AgentMessageKind::Notification,
            content: "cross-team",
        })
        .expect_err("cross-team receiver must fail");
    assert!(matches!(
        cross_team_error,
        WorkspaceDatabaseError::AgentDomain { ref source }
            if source.code() == AgentDomainErrorCode::CrossTeamReference
    ));

    let third_task = AgentTaskId::new("agent-task-third").expect("third task");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &third_task,
            team_id: &first_team,
            owner_instance_id: &first_instance,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("third task enqueue");
    database
        .insert_agent_task_dependency(NewAgentTaskDependency {
            team_id: &first_team,
            waiting_task_id: &first_task,
            dependency_task_id: &third_task,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: None,
            deadline_at: None,
        })
        .expect("first dependency");
    let cycle_error = database
        .insert_agent_task_dependency(NewAgentTaskDependency {
            team_id: &first_team,
            waiting_task_id: &third_task,
            dependency_task_id: &first_task,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: None,
            deadline_at: None,
        })
        .expect_err("dependency cycle must fail");
    assert!(matches!(
        cycle_error,
        WorkspaceDatabaseError::AgentDomain { ref source }
            if source.code() == AgentDomainErrorCode::DependencyCycle
    ));
    assert!(
        !database
            .agent_task_dependencies_satisfied(&first_task)
            .expect("dependency state")
    );
}

#[test]
fn phase8_creates_multiple_agent_instances_atomically() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, _) =
        create_test_agent_team(&mut database, "chat-agent-phase8-create", "phase8-create");
    let definition = phase8_agent_definition("phase8-create-worker", 7, 4);
    let first_id = AgentInstanceId::new("agent-instance-phase8-create-a").expect("instance id");
    let second_id = AgentInstanceId::new("agent-instance-phase8-create-b").expect("instance id");
    let instances = [
        NewAgentInstance {
            id: &first_id,
            team_id: &team_id,
            definition: &definition,
            role: AgentRole::Worker,
            execution_workspace_mode: foco_agent::AgentExecutionWorkspaceMode::Shared,
            execution_root_path: None,
            worktree_base_revision: None,
            worktree_branch: None,
            worktree_status: None,
        },
        NewAgentInstance {
            id: &second_id,
            team_id: &team_id,
            definition: &definition,
            role: AgentRole::Worker,
            execution_workspace_mode: foco_agent::AgentExecutionWorkspaceMode::Shared,
            execution_root_path: None,
            worktree_base_revision: None,
            worktree_branch: None,
            worktree_status: None,
        },
    ];

    let created = database
        .create_agent_instances_with_limits(&instances, 3, 2)
        .expect("create workers");

    assert_eq!(created.len(), 2);
    assert_eq!(created[0].definition_id, definition.id);
    assert_eq!(created[0].definition_revision, definition.revision);
    assert_eq!(
        created[1].definition_snapshot,
        created[0].definition_snapshot
    );
    assert_eq!(created[0].context_generation, 0);
    assert_eq!(created[1].next_task_sequence, 0);
    assert_eq!(created[0].status, AgentInstanceStatus::Idle);
    assert_eq!(created[1].role, AgentRole::Worker);

    let rejected_first =
        AgentInstanceId::new("agent-instance-phase8-create-c").expect("instance id");
    let rejected_second =
        AgentInstanceId::new("agent-instance-phase8-create-d").expect("instance id");
    let rejected = [
        NewAgentInstance {
            id: &rejected_first,
            team_id: &team_id,
            definition: &definition,
            role: AgentRole::Worker,
            execution_workspace_mode: foco_agent::AgentExecutionWorkspaceMode::Shared,
            execution_root_path: None,
            worktree_base_revision: None,
            worktree_branch: None,
            worktree_status: None,
        },
        NewAgentInstance {
            id: &rejected_second,
            team_id: &team_id,
            definition: &definition,
            role: AgentRole::Worker,
            execution_workspace_mode: foco_agent::AgentExecutionWorkspaceMode::Shared,
            execution_root_path: None,
            worktree_base_revision: None,
            worktree_branch: None,
            worktree_status: None,
        },
    ];
    database
        .create_agent_instances_with_limits(&rejected, 4, 3)
        .expect_err("limit failure must abort the whole create request");
    assert!(
        database
            .agent_instance(&rejected_first)
            .expect("rejected first lookup")
            .is_none()
    );
    assert!(
        database
            .agent_instance(&rejected_second)
            .expect("rejected second lookup")
            .is_none()
    );
}

#[test]
fn phase12_persists_isolated_agent_instance_worktree_metadata() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, _) = create_test_agent_team(
        &mut database,
        "chat-agent-phase12-worktree",
        "phase12-worktree",
    );
    let definition = phase8_agent_definition("phase12-worktree-worker", 1, 2);
    let instance_id =
        AgentInstanceId::new("agent-instance-phase12-worktree-worker").expect("instance id");
    let root_path = workspace
        .path()
        .join(".foco")
        .join("agent-worktrees")
        .join(instance_id.as_str())
        .display()
        .to_string();

    let created = database
        .create_agent_instances_with_limits(
            &[NewAgentInstance {
                id: &instance_id,
                team_id: &team_id,
                definition: &definition,
                role: AgentRole::Worker,
                execution_workspace_mode: AgentExecutionWorkspaceMode::IsolatedWorktree,
                execution_root_path: Some(&root_path),
                worktree_base_revision: Some("0123456789abcdef0123456789abcdef01234567"),
                worktree_branch: Some(
                    "foco/agent-worktrees/agent-instance-phase12-worktree-worker",
                ),
                worktree_status: Some("active"),
            }],
            2,
            2,
        )
        .expect("create isolated worker");

    assert_eq!(created.len(), 1);
    assert_eq!(
        created[0].execution_workspace_mode,
        AgentExecutionWorkspaceMode::IsolatedWorktree
    );
    assert_eq!(
        created[0].execution_root_path.as_deref(),
        Some(root_path.as_str())
    );
    assert_eq!(created[0].worktree_status.as_deref(), Some("active"));

    let updated = database
        .update_agent_instance_worktree_status(&instance_id, "archived")
        .expect("archive worktree");
    assert_eq!(updated.worktree_status.as_deref(), Some("archived"));
}

#[test]
fn phase12_rejects_worktree_status_update_for_shared_instance() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, _) =
        create_test_agent_team(&mut database, "chat-agent-phase12-shared", "phase12-shared");
    let definition = phase8_agent_definition("phase12-shared-worker", 1, 2);
    let instance_id =
        AgentInstanceId::new("agent-instance-phase12-shared-worker").expect("instance id");

    database
        .create_agent_instances_with_limits(
            &[NewAgentInstance {
                id: &instance_id,
                team_id: &team_id,
                definition: &definition,
                role: AgentRole::Worker,
                execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
                execution_root_path: None,
                worktree_base_revision: None,
                worktree_branch: None,
                worktree_status: None,
            }],
            2,
            2,
        )
        .expect("create shared worker");

    let error = database
        .update_agent_instance_worktree_status(&instance_id, "archived")
        .expect_err("shared instance must reject worktree status updates");
    assert!(matches!(
        error,
        WorkspaceDatabaseError::InvalidAgentRuntimeData { ref message }
            if message.contains("does not use an isolated worktree")
    ));
}

#[test]
fn phase8_routes_definition_by_least_pending_and_filters_unavailable_instances() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, _) =
        create_test_agent_team(&mut database, "chat-agent-phase8-route", "phase8-route");
    let definition = phase8_agent_definition("phase8-route-worker", 1, 4);
    let first_id = AgentInstanceId::new("agent-instance-phase8-route-a").expect("instance id");
    let second_id = AgentInstanceId::new("agent-instance-phase8-route-b").expect("instance id");
    let instances = [
        NewAgentInstance {
            id: &first_id,
            team_id: &team_id,
            definition: &definition,
            role: AgentRole::Worker,
            execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
            execution_root_path: None,
            worktree_base_revision: None,
            worktree_branch: None,
            worktree_status: None,
        },
        NewAgentInstance {
            id: &second_id,
            team_id: &team_id,
            definition: &definition,
            role: AgentRole::Worker,
            execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
            execution_root_path: None,
            worktree_base_revision: None,
            worktree_branch: None,
            worktree_status: None,
        },
    ];
    database
        .create_agent_instances_with_limits(&instances, 3, 2)
        .expect("create workers");

    assert_eq!(
        database
            .route_agent_instance_for_definition(&team_id, &definition.id)
            .expect("initial route")
            .expect("initial instance")
            .id,
        first_id
    );

    let task_id = AgentTaskId::new("agent-task-phase8-route-first").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &first_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue first task");
    assert_eq!(
        database
            .route_agent_instance_for_definition(&team_id, &definition.id)
            .expect("least pending route")
            .expect("least pending instance")
            .id,
        second_id
    );

    database
        .transition_agent_instance_status(&second_id, AgentInstanceStatus::Paused)
        .expect("pause second");
    assert_eq!(
        database
            .route_agent_instance_for_definition(&team_id, &definition.id)
            .expect("paused filtered route")
            .expect("first is only routable instance")
            .id,
        first_id
    );

    let attempt_id = AgentAttemptId::new("agent-attempt-phase8-route-first").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
        .expect("claim first task")
        .expect("first task claimed");
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &task_id,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Wait,
                result_json: None,
                error_json: None,
                interruption_reason: None,
            })
            .expect("wait first task")
    );
    assert!(
        database
            .route_agent_instance_for_definition(&team_id, &definition.id)
            .expect("waiting filtered route")
            .is_none(),
        "waiting and paused instances must not accept new definition routes"
    );
}

#[test]
fn phase8_runnable_tasks_are_fair_and_keep_instance_fifo() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, _) =
        create_test_agent_team(&mut database, "chat-agent-phase8-fair", "phase8-fair");
    let definition = phase8_agent_definition("phase8-fair-worker", 1, 4);
    let first_id = AgentInstanceId::new("agent-instance-phase8-fair-a").expect("instance id");
    let second_id = AgentInstanceId::new("agent-instance-phase8-fair-b").expect("instance id");
    let instances = [
        NewAgentInstance {
            id: &first_id,
            team_id: &team_id,
            definition: &definition,
            role: AgentRole::Worker,
            execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
            execution_root_path: None,
            worktree_base_revision: None,
            worktree_branch: None,
            worktree_status: None,
        },
        NewAgentInstance {
            id: &second_id,
            team_id: &team_id,
            definition: &definition,
            role: AgentRole::Worker,
            execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
            execution_root_path: None,
            worktree_base_revision: None,
            worktree_branch: None,
            worktree_status: None,
        },
    ];
    database
        .create_agent_instances_with_limits(&instances, 3, 2)
        .expect("create workers");
    let first_task = AgentTaskId::new("agent-task-phase8-fair-a1").expect("task id");
    let first_followup = AgentTaskId::new("agent-task-phase8-fair-a2").expect("task id");
    let second_task = AgentTaskId::new("agent-task-phase8-fair-b1").expect("task id");
    for (task_id, instance_id) in [
        (&first_task, &first_id),
        (&first_followup, &first_id),
        (&second_task, &second_id),
    ] {
        database
            .enqueue_agent_task(NewAgentTask {
                id: task_id,
                team_id: &team_id,
                owner_instance_id: instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue task");
    }

    let runnable = database.runnable_agent_tasks(10).expect("initial runnable");
    assert_eq!(
        runnable.iter().map(|task| &task.id).collect::<Vec<_>>(),
        vec![&first_task, &second_task]
    );

    let attempt_id = AgentAttemptId::new("agent-attempt-phase8-fair-a1").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &first_task, &attempt_id)
        .expect("claim first")
        .expect("first claimed");
    let runnable = database.runnable_agent_tasks(10).expect("running runnable");
    assert_eq!(
        runnable.iter().map(|task| &task.id).collect::<Vec<_>>(),
        vec![&second_task],
        "one running task blocks the same instance's later queued task"
    );

    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &first_task,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Complete,
                result_json: Some(r#"{"ok":true}"#),
                error_json: None,
                interruption_reason: None,
            })
            .expect("complete first")
    );
    let runnable = database.runnable_agent_tasks(10).expect("fair runnable");
    assert_eq!(
        runnable.iter().map(|task| &task.id).collect::<Vec<_>>(),
        vec![&second_task, &first_followup],
        "an instance that has not run yet is scheduled before a long local queue"
    );
}

#[test]
fn phase7_waiting_tasks_resume_after_dependency_finishes() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-agent-phase7-resume", "phase7-resume");
    let worker_id = create_test_agent_worker(&database, &team_id, "phase7-resume-worker");

    let waiting_task_id = AgentTaskId::new("agent-task-phase7-waiting").expect("task id");
    let dependency_task_id = AgentTaskId::new("agent-task-phase7-dependency").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &waiting_task_id,
            team_id: &team_id,
            owner_instance_id: &coordinator_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: r#"{"goal":"wait"}"#,
        })
        .expect("waiting task enqueue");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &dependency_task_id,
            team_id: &team_id,
            owner_instance_id: &worker_id,
            origin_instance_id: Some(&coordinator_id),
            parent_task_id: Some(&waiting_task_id),
            input_json: r#"{"goal":"dependency"}"#,
        })
        .expect("dependency task enqueue");

    let first_attempt_id =
        AgentAttemptId::new("agent-attempt-phase7-waiting-first").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &waiting_task_id, &first_attempt_id)
        .expect("claim waiting task")
        .expect("waiting task claimed");
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &waiting_task_id,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Wait,
                result_json: None,
                error_json: None,
                interruption_reason: None,
            })
            .expect("suspend waiting task")
    );
    database
        .insert_agent_task_dependency(NewAgentTaskDependency {
            team_id: &team_id,
            waiting_task_id: &waiting_task_id,
            dependency_task_id: &dependency_task_id,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("tool-call-phase7-wait"),
            deadline_at: None,
        })
        .expect("wait dependency insert");
    let dependency = database
        .agent_task_dependencies(&waiting_task_id)
        .expect("dependencies")
        .pop()
        .expect("dependency");
    assert_eq!(
        dependency.pending_tool_call_id.as_deref(),
        Some("tool-call-phase7-wait")
    );
    assert!(
        database
            .resume_satisfied_agent_tasks(10)
            .expect("resume before dependency completes")
            .is_empty()
    );

    let dependency_attempt_id =
        AgentAttemptId::new("agent-attempt-phase7-dependency").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &dependency_task_id, &dependency_attempt_id)
        .expect("claim dependency")
        .expect("dependency claimed");
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &dependency_task_id,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Complete,
                result_json: Some(r#"{"ok":true}"#),
                error_json: None,
                interruption_reason: None,
            })
            .expect("complete dependency")
    );

    let resumed = database
        .resume_satisfied_agent_tasks(10)
        .expect("resume satisfied task");
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].id, waiting_task_id);
    assert_eq!(resumed[0].status, AgentTaskStatus::Queued);
    assert_eq!(
        database
            .agent_instance(&coordinator_id)
            .expect("coordinator")
            .expect("coordinator")
            .status,
        AgentInstanceStatus::Idle
    );

    let second_attempt_id =
        AgentAttemptId::new("agent-attempt-phase7-waiting-second").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &waiting_task_id, &second_attempt_id)
        .expect("claim resumed task")
        .expect("resumed task claimed");
    let attempts = database
        .agent_attempts_for_task(&waiting_task_id)
        .expect("attempts");
    assert_eq!(attempts.len(), 2);
    assert_eq!(
        attempts[0].status,
        foco_agent::AgentAttemptStatus::Suspended
    );
    assert_eq!(attempts[1].status, foco_agent::AgentAttemptStatus::Running);
}

#[test]
fn phase7_waiting_tasks_resume_after_deadline() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, coordinator_id) = create_test_agent_team(
        &mut database,
        "chat-agent-phase7-deadline",
        "phase7-deadline",
    );
    let worker_id = create_test_agent_worker(&database, &team_id, "phase7-deadline-worker");
    let waiting_task_id = AgentTaskId::new("agent-task-phase7-deadline-waiting").expect("task id");
    let dependency_task_id = AgentTaskId::new("agent-task-phase7-deadline-dep").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &waiting_task_id,
            team_id: &team_id,
            owner_instance_id: &coordinator_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("waiting task enqueue");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &dependency_task_id,
            team_id: &team_id,
            owner_instance_id: &worker_id,
            origin_instance_id: Some(&coordinator_id),
            parent_task_id: Some(&waiting_task_id),
            input_json: "{}",
        })
        .expect("dependency task enqueue");
    database
        .claim_runnable_agent_task(
            &team_id,
            &waiting_task_id,
            &AgentAttemptId::new("agent-attempt-phase7-deadline").expect("attempt id"),
        )
        .expect("claim waiting task")
        .expect("waiting task claimed");
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &waiting_task_id,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Wait,
                result_json: None,
                error_json: None,
                interruption_reason: None,
            })
            .expect("suspend waiting task")
    );
    database
        .insert_agent_task_dependency(NewAgentTaskDependency {
            team_id: &team_id,
            waiting_task_id: &waiting_task_id,
            dependency_task_id: &dependency_task_id,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("tool-call-phase7-deadline"),
            deadline_at: Some("2000-01-01T00:00:00.000Z"),
        })
        .expect("deadline dependency insert");

    assert!(
        database
            .agent_task_dependencies_satisfied(&waiting_task_id)
            .expect("deadline dependency state")
    );
    let resumed = database
        .resume_satisfied_agent_tasks(10)
        .expect("deadline resume");
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].id, waiting_task_id);
    assert_eq!(resumed[0].status, AgentTaskStatus::Queued);
}

#[test]
fn phase7_agent_task_transfer_accepts_only_queued_tasks() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, coordinator_id) = create_test_agent_team(
        &mut database,
        "chat-agent-phase7-transfer",
        "phase7-transfer",
    );
    let worker_id = create_test_agent_worker(&database, &team_id, "phase7-transfer-worker");
    let task_id = AgentTaskId::new("agent-task-phase7-transfer").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &coordinator_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("task enqueue");

    let transferred = database
        .transfer_queued_agent_task_with_limits(&team_id, &task_id, &worker_id, 64, 64, 64)
        .expect("transfer queued task")
        .expect("transferred task");
    assert_eq!(transferred.owner_instance_id, worker_id);
    assert_eq!(transferred.status, AgentTaskStatus::Queued);
    assert_eq!(transferred.sequence, 0);

    database
        .claim_runnable_agent_task(
            &team_id,
            &task_id,
            &AgentAttemptId::new("agent-attempt-phase7-transfer").expect("attempt id"),
        )
        .expect("claim transferred task")
        .expect("transferred task claimed");
    assert!(
        database
            .transfer_queued_agent_task_with_limits(&team_id, &task_id, &coordinator_id, 64, 64, 64)
            .is_err(),
        "running tasks cannot be transferred"
    );
}

#[test]
fn phase7_waiting_cancel_clears_dependencies_and_retry_preserves_previous_error() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, coordinator_id) = create_test_agent_team(
        &mut database,
        "chat-agent-phase7-cancel-retry",
        "phase7-cancel-retry",
    );
    let worker_id = create_test_agent_worker(&database, &team_id, "phase7-cancel-retry-worker");
    let waiting_task_id = AgentTaskId::new("agent-task-phase7-cancel-waiting").expect("task id");
    let dependency_task_id = AgentTaskId::new("agent-task-phase7-cancel-dep").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &waiting_task_id,
            team_id: &team_id,
            owner_instance_id: &coordinator_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("waiting task enqueue");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &dependency_task_id,
            team_id: &team_id,
            owner_instance_id: &worker_id,
            origin_instance_id: Some(&coordinator_id),
            parent_task_id: Some(&waiting_task_id),
            input_json: "{}",
        })
        .expect("dependency task enqueue");
    database
        .claim_runnable_agent_task(
            &team_id,
            &waiting_task_id,
            &AgentAttemptId::new("agent-attempt-phase7-cancel-first").expect("attempt id"),
        )
        .expect("claim waiting task")
        .expect("waiting task claimed");
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &waiting_task_id,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Wait,
                result_json: None,
                error_json: None,
                interruption_reason: None,
            })
            .expect("suspend waiting task")
    );
    database
        .insert_agent_task_dependency(NewAgentTaskDependency {
            team_id: &team_id,
            waiting_task_id: &waiting_task_id,
            dependency_task_id: &dependency_task_id,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("tool-call-phase7-cancel"),
            deadline_at: None,
        })
        .expect("wait dependency insert");

    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &waiting_task_id,
                expected_status: AgentTaskStatus::Waiting,
                transition: AgentTaskTransition::Cancel,
                result_json: None,
                error_json: Some(r#"{"message":"cancelled explicitly"}"#),
                interruption_reason: None,
            })
            .expect("cancel waiting task")
    );
    assert!(
        database
            .agent_task_dependencies(&waiting_task_id)
            .expect("dependencies")
            .is_empty()
    );
    let cancelled = database
        .agent_task(&waiting_task_id)
        .expect("cancelled task")
        .expect("cancelled task");
    assert_eq!(cancelled.status, AgentTaskStatus::Cancelled);
    assert_json_eq(
        cancelled.error_json.as_deref().expect("cancel error"),
        r#"{"message":"cancelled explicitly"}"#,
    );

    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &waiting_task_id,
                expected_status: AgentTaskStatus::Cancelled,
                transition: AgentTaskTransition::Retry,
                result_json: None,
                error_json: None,
                interruption_reason: None,
            })
            .expect("retry cancelled task")
    );
    let retried = database
        .agent_task(&waiting_task_id)
        .expect("retried task")
        .expect("retried task");
    assert_eq!(retried.status, AgentTaskStatus::Queued);
    assert_eq!(retried.started_at, None);
    assert!(retried.completed_at.is_some());
    assert_json_eq(
        retried.error_json.as_deref().expect("previous error"),
        r#"{"message":"cancelled explicitly"}"#,
    );

    database
        .claim_runnable_agent_task(
            &team_id,
            &waiting_task_id,
            &AgentAttemptId::new("agent-attempt-phase7-cancel-second").expect("attempt id"),
        )
        .expect("claim retried task")
        .expect("retried task claimed");
    assert_eq!(
        database
            .agent_attempts_for_task(&waiting_task_id)
            .expect("attempts")
            .len(),
        2
    );
}

#[test]
fn phase6_agent_messages_are_ordered_redacted_and_explicitly_consumed() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-agent-messages", "messages");

    let first_message_id = AgentMessageId::new("agent-message-phase6-first").expect("message id");
    let first_message = database
        .insert_agent_message(NewAgentMessage {
            id: &first_message_id,
            team_id: &team_id,
            sender_instance_id: Some(&instance_id),
            receiver_instance_id: &instance_id,
            related_task_id: None,
            reply_to_message_id: None,
            kind: AgentMessageKind::Notification,
            content: "Authorization: Bearer secret\nstatus ok password=hunter2 token:abc",
        })
        .expect("first message");
    assert_eq!(first_message.sequence, 0);
    assert_eq!(first_message.consumed_at, None);
    assert!(first_message.content.contains("[REDACTED]"));
    assert!(!first_message.content.contains("Bearer secret"));
    assert!(!first_message.content.contains("hunter2"));
    assert!(!first_message.content.contains("abc"));

    let second_message_id = AgentMessageId::new("agent-message-phase6-second").expect("message id");
    let second_message = database
        .insert_agent_message(NewAgentMessage {
            id: &second_message_id,
            team_id: &team_id,
            sender_instance_id: Some(&instance_id),
            receiver_instance_id: &instance_id,
            related_task_id: None,
            reply_to_message_id: Some(&first_message_id),
            kind: AgentMessageKind::Reply,
            content: "plain reply",
        })
        .expect("second message");
    assert_eq!(second_message.sequence, 1);

    let messages = database
        .agent_messages_after(&instance_id, -1)
        .expect("messages after");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].id.as_str(), first_message_id.as_str());
    assert_eq!(messages[1].id.as_str(), second_message_id.as_str());
    assert_eq!(messages[0].consumed_at, None);
    assert_eq!(messages[1].consumed_at, None);

    assert!(
        database
            .mark_agent_message_consumed(&first_message_id)
            .expect("consume first message")
    );
    assert!(
        !database
            .mark_agent_message_consumed(&first_message_id)
            .expect("consume first message twice")
    );
    assert!(
        database
            .agent_message(&first_message_id)
            .expect("first message read")
            .expect("first message")
            .consumed_at
            .is_some()
    );
    assert_eq!(
        database
            .agent_message(&second_message_id)
            .expect("second message read")
            .expect("second message")
            .consumed_at,
        None
    );
}

#[test]
fn phase6_agent_child_tasks_are_team_scoped_and_queued_only_cancellable() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-agent-phase6-tasks", "phase6-tasks");
    let (other_team_id, other_instance_id) =
        create_test_agent_team(&mut database, "chat-agent-phase6-other", "phase6-other");

    let parent_task_id = AgentTaskId::new("agent-task-phase6-parent").expect("parent task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &parent_task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: r#"{"goal":"parent"}"#,
        })
        .expect("parent enqueue");
    let child_task_id = AgentTaskId::new("agent-task-phase6-child").expect("child task id");
    let child_task = database
        .enqueue_agent_task(NewAgentTask {
            id: &child_task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: Some(&instance_id),
            parent_task_id: Some(&parent_task_id),
            input_json: r#"{"correlationId":"phase6-correlation","delegatedInput":{"goal":"child"}}"#,
        })
        .expect("child enqueue");
    assert_eq!(child_task.origin_instance_id.as_ref(), Some(&instance_id));
    assert_eq!(child_task.parent_task_id.as_ref(), Some(&parent_task_id));

    let child_tasks = database
        .agent_tasks_for_parent(&team_id, &parent_task_id)
        .expect("child tasks");
    assert_eq!(child_tasks.len(), 1);
    assert_eq!(child_tasks[0].id.as_str(), child_task_id.as_str());
    assert!(
        database
            .agent_task_for_team(&team_id, &child_task_id)
            .expect("own team task")
            .is_some()
    );
    assert!(
        database
            .agent_task_for_team(&other_team_id, &child_task_id)
            .expect("cross team task")
            .is_none()
    );

    assert!(
        database
            .cancel_queued_agent_task(&team_id, &child_task_id, r#"{"code":"cancelled_by_agent"}"#,)
            .expect("cancel queued child")
    );
    let cancelled_child = database
        .agent_task(&child_task_id)
        .expect("cancelled child read")
        .expect("cancelled child");
    assert_eq!(cancelled_child.status, AgentTaskStatus::Cancelled);
    assert_json_eq(
        cancelled_child.error_json.as_deref().expect("cancel error"),
        r#"{"code":"cancelled_by_agent"}"#,
    );

    let running_task_id = AgentTaskId::new("agent-task-phase6-running").expect("running task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &running_task_id,
            team_id: &other_team_id,
            owner_instance_id: &other_instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: r#"{"goal":"running"}"#,
        })
        .expect("running enqueue");
    let attempt_id = AgentAttemptId::new("agent-attempt-phase6-running").expect("attempt id");
    database
        .claim_runnable_agent_task(&other_team_id, &running_task_id, &attempt_id)
        .expect("claim running task")
        .expect("running task");
    assert!(
        !database
            .cancel_queued_agent_task(
                &other_team_id,
                &running_task_id,
                r#"{"code":"cancelled_by_agent"}"#,
            )
            .expect("cancel running task")
    );
}

#[test]
fn phase6_agent_definition_lookup_returns_existing_instances_only() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-agent-definition-lookup",
        "definition-lookup",
    );
    let instance = database
        .agent_instance(&instance_id)
        .expect("instance read")
        .expect("instance");

    let matches = database
        .agent_instances_for_definition(&team_id, &instance.definition_id)
        .expect("instances for definition");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id.as_str(), instance_id.as_str());

    let missing_definition_id =
        AgentDefinitionId::new("agent-definition-phase6-missing").expect("definition id");
    assert!(
        database
            .agent_instances_for_definition(&team_id, &missing_definition_id)
            .expect("missing instances for definition")
            .is_empty()
    );
}

#[test]
fn agent_runtime_state_round_trips_and_chat_delete_preserves_llm_audit() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-agent-runtime", "runtime");
    let task_id = AgentTaskId::new("agent-task-runtime").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: r#"{"goal":"verify persistence"}"#,
        })
        .expect("enqueue");
    assert_eq!(
        database
            .agent_team_for_chat("chat-agent-runtime")
            .expect("team for chat")
            .expect("team")
            .id,
        team_id
    );
    assert_eq!(
        database
            .agent_instances_for_team(&team_id)
            .expect("instances")
            .len(),
        1
    );
    assert_eq!(
        database
            .agent_tasks_for_team(&team_id)
            .expect("tasks")
            .len(),
        1
    );
    assert_eq!(
        database.runnable_agent_tasks(10).expect("runnable").len(),
        1
    );
    let attempt_id = AgentAttemptId::new("agent-attempt-runtime").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
        .expect("claim")
        .expect("runnable task");
    assert_eq!(
        database
            .startup_agent_reconciliation()
            .expect("reconcile")
            .len(),
        1
    );

    database
        .insert_agent_context_entry(NewAgentContextEntry {
            id: "context-entry-1",
            team_id: &team_id,
            instance_id: &instance_id,
            generation: 0,
            sequence: 0,
            role: "assistant",
            content_json: r#"{"text":"private"}"#,
            source_task_id: Some(&task_id),
            source_message_id: None,
        })
        .expect("context entry");
    database
        .insert_agent_context_snapshot(NewAgentContextSnapshot {
            id: "context-snapshot-1",
            team_id: &team_id,
            instance_id: &instance_id,
            generation: 0,
            sequence: 0,
            entries_json: r#"[{"text":"private"}]"#,
            token_count: Some(2),
        })
        .expect("context snapshot");
    assert_eq!(
        database
            .agent_context_entries(&instance_id, 0, -1)
            .expect("context entries")
            .len(),
        1
    );
    assert!(
        database
            .latest_agent_context_snapshot(&instance_id, 0)
            .expect("latest snapshot")
            .is_some()
    );

    let message_id = AgentMessageId::new("agent-message-runtime").expect("message id");
    let message = database
        .insert_agent_message(NewAgentMessage {
            id: &message_id,
            team_id: &team_id,
            sender_instance_id: Some(&instance_id),
            receiver_instance_id: &instance_id,
            related_task_id: Some(&task_id),
            reply_to_message_id: None,
            kind: AgentMessageKind::Notification,
            content: "persisted message",
        })
        .expect("message");
    assert_eq!(message.sequence, 0);
    assert!(
        database
            .mark_agent_message_consumed(&message_id)
            .expect("consume message")
    );

    let event = database
        .append_agent_event(NewAgentEvent {
            team_id: &team_id,
            event_type: "task_started",
            instance_id: Some(&instance_id),
            task_id: Some(&task_id),
            attempt_id: Some(&attempt_id),
            message_id: Some(&message_id),
            payload_json: r#"{"authorization":"Bearer secret","safe":true}"#,
        })
        .expect("event");
    assert!(event.payload_json.contains("[REDACTED]"));

    database
        .insert_llm_request(NewLlmRequest {
            id: "request-agent-runtime",
            workspace_id: "workspace-1",
            chat_id: Some("chat-agent-runtime"),
            agent_team_id: Some(&team_id),
            agent_instance_id: Some(&instance_id),
            agent_task_id: Some(&task_id),
            agent_attempt_id: Some(&attempt_id),
            provider_id: "openai",
            model_id: "gpt-test",
            request_started_at: "2026-06-19T00:00:00Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: None,
            final_state: "running",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("llm request");

    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &task_id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Interrupt,
            result_json: None,
            error_json: Some(r#"{"code":"backend_restart"}"#),
            interruption_reason: Some("backend_restart"),
        })
        .expect("interrupt task");
    assert!(
        database
            .startup_agent_reconciliation()
            .expect("reconcile after interrupt")
            .is_empty()
    );

    assert!(
        database
            .delete_chat("chat-agent-runtime")
            .expect("delete chat")
    );
    let connection = Connection::open(database.database_path()).expect("database connection");
    for table in [
        "agent_teams",
        "agent_instances",
        "agent_tasks",
        "agent_messages",
        "agent_attempts",
        "agent_events",
        "agent_context_entries",
        "agent_context_snapshots",
    ] {
        assert_eq!(table_count(&connection, table), 0, "{table} should cascade");
    }
    let request = database
        .llm_request("request-agent-runtime")
        .expect("llm request read")
        .expect("llm request preserved");
    assert_eq!(request.chat_id, None);
    assert_eq!(request.agent_team_id, None);
    assert_eq!(request.agent_instance_id, None);
    assert_eq!(request.agent_task_id, None);
    assert_eq!(request.agent_attempt_id, None);
}

fn create_test_agent_team(
    database: &mut WorkspaceDatabase,
    chat_id: &str,
    suffix: &str,
) -> (AgentTeamId, AgentInstanceId) {
    database
        .insert_chat(chat_id, &format!("Agent team {suffix}"))
        .expect("chat insert");
    let team_id = AgentTeamId::new(format!("agent-team-{suffix}")).expect("team id");
    let instance_id =
        AgentInstanceId::new(format!("agent-instance-{suffix}")).expect("instance id");
    let definition = AgentDefinitionSettings {
        id: AgentDefinitionId::new(format!("agent-definition-{suffix}")).expect("definition id"),
        revision: 1,
        name: format!("Agent {suffix}"),
        description: String::new(),
        provider_id: "provider-test".to_string(),
        model_id: "model-test".to_string(),
        model_options: AgentModelOptions::default(),
        system_prompt: "Be precise.".to_string(),
        allowed_tools: vec!["read_file".to_string()],
        max_instances: 1,
        permissions: AgentPermissions::default(),
    };
    database
        .create_agent_team(NewAgentTeam {
            id: &team_id,
            chat_id,
            coordinator_instance_id: &instance_id,
            coordinator_definition: &definition,
            max_concurrent_runs: 1,
        })
        .expect("agent team create");
    (team_id, instance_id)
}

fn phase8_agent_definition(
    suffix: &str,
    revision: u64,
    max_instances: u32,
) -> AgentDefinitionSettings {
    AgentDefinitionSettings {
        id: AgentDefinitionId::new(format!("agent-definition-{suffix}")).expect("definition id"),
        revision,
        name: format!("Agent {suffix}"),
        description: String::new(),
        provider_id: "provider-test".to_string(),
        model_id: "model-test".to_string(),
        model_options: AgentModelOptions::default(),
        system_prompt: "Be precise.".to_string(),
        allowed_tools: vec!["read_file".to_string()],
        max_instances,
        permissions: AgentPermissions::default(),
    }
}

fn create_test_agent_worker(
    database: &WorkspaceDatabase,
    team_id: &AgentTeamId,
    suffix: &str,
) -> AgentInstanceId {
    let coordinator = database
        .agent_instances_for_team(team_id)
        .expect("instances")
        .into_iter()
        .find(|instance| instance.role.as_str() == "coordinator")
        .expect("coordinator instance");
    let instance_id =
        AgentInstanceId::new(format!("agent-instance-{suffix}")).expect("instance id");
    let definition_snapshot_json =
        serde_json::to_string(&coordinator.definition_snapshot).expect("definition snapshot json");
    let connection = Connection::open(database.database_path()).expect("database connection");
    connection
        .execute(
            "INSERT INTO agent_instances
                (id, team_id, definition_id, definition_revision, definition_snapshot_json,
                 role, status, next_task_sequence, next_message_sequence, context_generation,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'worker', ?6, 0, 0, 0,
                     '2026-06-19T00:00:00.000Z', '2026-06-19T00:00:00.000Z')",
            params![
                instance_id.as_str(),
                team_id.as_str(),
                coordinator.definition_id.as_str(),
                coordinator.definition_revision as i64,
                definition_snapshot_json,
                AgentInstanceStatus::Idle.as_str(),
            ],
        )
        .expect("worker instance insert");
    instance_id
}

fn assert_json_eq(actual: &str, expected: &str) {
    let actual: Value = serde_json::from_str(actual).expect("actual json");
    let expected: Value = serde_json::from_str(expected).expect("expected json");

    assert_eq!(actual, expected);
}

fn todo_graph_task(
    id: &str,
    title: &str,
    status: &str,
    depends_on: Vec<&str>,
    acceptance: Vec<&str>,
    summary: &str,
    subtasks: Vec<TodoGraphTask>,
) -> TodoGraphTask {
    TodoGraphTask {
        id: id.to_string(),
        title: title.to_string(),
        status: status.to_string(),
        depends_on: depends_on
            .into_iter()
            .map(std::string::ToString::to_string)
            .collect(),
        acceptance: acceptance
            .into_iter()
            .map(std::string::ToString::to_string)
            .collect(),
        summary: summary.to_string(),
        created_at: String::new(),
        updated_at: String::new(),
        subtasks,
    }
}

fn table_exists(connection: &Connection, table: &str) -> bool {
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

fn table_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("table count query")
}

fn assert_no_agent_messages_old_references(connection: &Connection) {
    let stale_schema_count: i64 = connection
        .query_row(
            "SELECT COUNT(*)
             FROM sqlite_schema
             WHERE sql LIKE '%agent_messages_old%'",
            [],
            |row| row.get(0),
        )
        .expect("stale schema query");
    assert_eq!(stale_schema_count, 0);

    for table in ["agent_events", "agent_context_entries"] {
        let mut statement = connection
            .prepare(&format!("PRAGMA foreign_key_list({table})"))
            .expect("foreign key list statement");
        let referenced_tables = statement
            .query_map([], |row| row.get::<_, String>(2))
            .expect("foreign key list rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("foreign key list collect");
        assert!(
            !referenced_tables
                .iter()
                .any(|referenced_table| referenced_table == "agent_messages_old"),
            "{table} must not reference agent_messages_old"
        );
    }
}
