use std::fs;

use foco_store::{
    config::WorkspaceConfig,
    workspace::{
        LlmRequestRecord, NewContextCompressionSnapshot, NewLlmRequest, NewLlmRequestEvent,
        NewMessage, NewRunEvent, NewTerminalSession, NewToolCall, NewToolResult,
        WORKSPACE_SCHEMA_VERSION, WorkspaceDatabase, initialize_workspace_databases,
        workspace_database_path,
    },
};
use rusqlite::Connection;
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
        "code_graph_file_hashes",
        "code_graph_parse_status",
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
        },
        WorkspaceConfig {
            id: "second".to_string(),
            name: "Second".to_string(),
            path: second.path().to_path_buf(),
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
}

fn assert_json_eq(actual: &str, expected: &str) {
    let actual: Value = serde_json::from_str(actual).expect("actual json");
    let expected: Value = serde_json::from_str(expected).expect("expected json");

    assert_eq!(actual, expected);
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
