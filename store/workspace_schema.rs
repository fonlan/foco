pub(crate) struct Migration {
    pub(crate) version: u32,
    pub(crate) sql: &'static str,
}

pub(crate) const MIGRATION_001: &str = r#"
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

pub(crate) const MIGRATION_002: &str = r#"
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

pub(crate) const MIGRATION_003: &str = r#"
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

pub(crate) const MIGRATION_004: &str = r#"
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

pub(crate) const MIGRATION_005: &str = r#"
CREATE TABLE task_graphs (
    chat_id TEXT PRIMARY KEY NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    graph_json TEXT NOT NULL CHECK (length(graph_json) > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX task_graphs_updated_at_idx ON task_graphs (updated_at);
"#;

pub(crate) const MIGRATION_006: &str = r#"
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

pub(crate) const MIGRATION_008: &str = r#"
DROP INDEX task_graphs_updated_at_idx;
ALTER TABLE task_graphs RENAME TO todo_graphs;
CREATE INDEX todo_graphs_updated_at_idx ON todo_graphs (updated_at);
"#;

pub(crate) const MIGRATION_009: &str = r#"
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
    use crate::workspace::{NewHookRun, WorkspaceDatabase};

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
