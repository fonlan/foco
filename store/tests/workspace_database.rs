use std::fs;

use foco_store::{
    config::WorkspaceConfig,
    workspace::{
        LlmRequestAuditFilters, LlmRequestRecord, NewCodeGraphEdge, NewCodeGraphFileIndex,
        NewCodeGraphImport, NewCodeGraphReference, NewCodeGraphSymbol,
        NewContextCompressionSnapshot, NewLlmRequest, NewLlmRequestEvent, NewMessage, NewRunEvent,
        NewTerminalSession, NewToolCall, NewToolResult, TodoGraphFilter, TodoGraphTask,
        TodoGraphTaskPatch, UpdateLlmRequestOutcome, WORKSPACE_SCHEMA_VERSION, WorkspaceDatabase,
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
    ] {
        assert!(
            table_exists(&connection, table),
            "{table} table should exist"
        );
    }
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
        },
        WorkspaceConfig {
            id: "second".to_string(),
            name: "Second".to_string(),
            path: second.path().to_path_buf(),
            pinned: false,
            terminal_shell: foco_store::config::DEFAULT_TERMINAL_SHELL.to_string(),
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
        .insert_tool_call(NewToolCall {
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
        .expect("tool call insert");
    database
        .insert_tool_result(NewToolResult {
            id: "tool-result-1",
            tool_call_id: "tool-call-1",
            output_json: r#"{"content":"hello","authorization":"Bearer secret"}"#,
            is_error: false,
            created_at: "2026-06-03T10:00:00.100Z",
        })
        .expect("tool result insert");

    let records = database
        .tool_calls_for_message("assistant-1")
        .expect("tool calls for message");

    assert_eq!(records.len(), 1);
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
