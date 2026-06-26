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

pub(crate) const MIGRATION_010: &str = r#"
CREATE TABLE agent_teams (
    id TEXT PRIMARY KEY NOT NULL CHECK (id GLOB 'agent-team-*'),
    chat_id TEXT NOT NULL UNIQUE REFERENCES chats(id) ON DELETE CASCADE,
    coordinator_instance_id TEXT NOT NULL CHECK (coordinator_instance_id GLOB 'agent-instance-*'),
    status TEXT NOT NULL CHECK (status IN ('active', 'paused', 'draining', 'stopped', 'failed')),
    max_concurrent_runs INTEGER NOT NULL CHECK (max_concurrent_runs > 0),
    next_event_sequence INTEGER NOT NULL DEFAULT 0 CHECK (next_event_sequence >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (id, coordinator_instance_id)
        REFERENCES agent_instances(team_id, id)
        DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE agent_instances (
    id TEXT PRIMARY KEY NOT NULL CHECK (id GLOB 'agent-instance-*'),
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
    definition_id TEXT NOT NULL CHECK (definition_id GLOB 'agent-definition-*'),
    definition_revision INTEGER NOT NULL CHECK (definition_revision > 0),
    definition_snapshot_json TEXT NOT NULL CHECK (json_valid(definition_snapshot_json)),
    role TEXT NOT NULL CHECK (role IN ('coordinator', 'worker')),
    status TEXT NOT NULL CHECK (status IN ('idle', 'running', 'waiting', 'paused', 'draining', 'stopped', 'failed')),
    next_task_sequence INTEGER NOT NULL DEFAULT 0 CHECK (next_task_sequence >= 0),
    next_message_sequence INTEGER NOT NULL DEFAULT 0 CHECK (next_message_sequence >= 0),
    context_generation INTEGER NOT NULL DEFAULT 0 CHECK (context_generation >= 0),
    last_scheduled_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (team_id, id)
);

CREATE UNIQUE INDEX agent_instances_one_coordinator_idx
    ON agent_instances (team_id)
    WHERE role = 'coordinator';
CREATE INDEX agent_instances_team_status_idx
    ON agent_instances (team_id, status);

CREATE TABLE agent_tasks (
    id TEXT PRIMARY KEY NOT NULL CHECK (id GLOB 'agent-task-*'),
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
    owner_instance_id TEXT NOT NULL,
    origin_instance_id TEXT,
    parent_task_id TEXT,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'waiting', 'completed', 'failed', 'cancelled', 'interrupted')),
    input_json TEXT NOT NULL CHECK (json_valid(input_json)),
    result_json TEXT CHECK (result_json IS NULL OR json_valid(result_json)),
    error_json TEXT CHECK (error_json IS NULL OR json_valid(error_json)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    UNIQUE (team_id, id),
    UNIQUE (owner_instance_id, sequence),
    FOREIGN KEY (team_id, owner_instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE CASCADE,
    FOREIGN KEY (team_id, origin_instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (team_id, parent_task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX agent_tasks_one_active_per_instance_idx
    ON agent_tasks (owner_instance_id)
    WHERE status IN ('running', 'waiting');
CREATE INDEX agent_tasks_runnable_idx
    ON agent_tasks (team_id, status, owner_instance_id, sequence)
    WHERE status = 'queued';
CREATE INDEX agent_tasks_parent_idx
    ON agent_tasks (team_id, parent_task_id);

CREATE TABLE agent_task_dependencies (
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
    waiting_task_id TEXT NOT NULL,
    dependency_task_id TEXT NOT NULL,
    wait_mode TEXT NOT NULL CHECK (wait_mode IN ('all', 'any')),
    created_at TEXT NOT NULL,
    PRIMARY KEY (waiting_task_id, dependency_task_id),
    CHECK (waiting_task_id <> dependency_task_id),
    FOREIGN KEY (team_id, waiting_task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE CASCADE,
    FOREIGN KEY (team_id, dependency_task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE CASCADE
);

CREATE INDEX agent_task_dependencies_waiting_idx
    ON agent_task_dependencies (team_id, waiting_task_id);
CREATE INDEX agent_task_dependencies_dependency_idx
    ON agent_task_dependencies (team_id, dependency_task_id);

CREATE TABLE agent_messages (
    id TEXT PRIMARY KEY NOT NULL CHECK (id GLOB 'agent-message-*'),
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
    sender_instance_id TEXT,
    receiver_instance_id TEXT NOT NULL,
    related_task_id TEXT,
    reply_to_message_id TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('notification', 'reply')),
    content TEXT NOT NULL CHECK (length(content) > 0),
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    created_at TEXT NOT NULL,
    consumed_at TEXT,
    UNIQUE (team_id, id),
    UNIQUE (receiver_instance_id, sequence),
    FOREIGN KEY (team_id, sender_instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (team_id, receiver_instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE CASCADE,
    FOREIGN KEY (team_id, related_task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, reply_to_message_id)
        REFERENCES agent_messages(team_id, id) ON DELETE SET NULL
);

CREATE INDEX agent_messages_unread_idx
    ON agent_messages (receiver_instance_id, sequence)
    WHERE consumed_at IS NULL;
CREATE INDEX agent_messages_task_idx
    ON agent_messages (team_id, related_task_id);

CREATE TABLE agent_attempts (
    id TEXT PRIMARY KEY NOT NULL CHECK (id GLOB 'agent-attempt-*'),
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    status TEXT NOT NULL CHECK (status IN ('running', 'suspended', 'completed', 'failed', 'cancelled', 'interrupted')),
    started_at TEXT NOT NULL,
    completed_at TEXT,
    interruption_reason TEXT,
    UNIQUE (team_id, id),
    UNIQUE (task_id, sequence),
    FOREIGN KEY (team_id, task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX agent_attempts_one_active_per_task_idx
    ON agent_attempts (task_id)
    WHERE status IN ('running', 'suspended');
CREATE INDEX agent_attempts_reconciliation_idx
    ON agent_attempts (status, team_id, task_id)
    WHERE status IN ('running', 'suspended');

CREATE TABLE agent_events (
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    event_type TEXT NOT NULL CHECK (length(event_type) > 0),
    instance_id TEXT,
    task_id TEXT,
    attempt_id TEXT,
    message_id TEXT,
    payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
    created_at TEXT NOT NULL,
    PRIMARY KEY (team_id, sequence),
    FOREIGN KEY (team_id, instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, attempt_id)
        REFERENCES agent_attempts(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, message_id)
        REFERENCES agent_messages(team_id, id) ON DELETE SET NULL
);

CREATE INDEX agent_events_entity_idx
    ON agent_events (team_id, instance_id, task_id, sequence);

CREATE TABLE agent_context_entries (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
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
    FOREIGN KEY (team_id, instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE CASCADE,
    FOREIGN KEY (team_id, source_task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, source_message_id)
        REFERENCES agent_messages(team_id, id) ON DELETE SET NULL
);

CREATE INDEX agent_context_entries_owner_idx
    ON agent_context_entries (instance_id, generation, sequence);

CREATE TABLE agent_context_snapshots (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
    instance_id TEXT NOT NULL,
    generation INTEGER NOT NULL CHECK (generation >= 0),
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    entries_json TEXT NOT NULL CHECK (json_valid(entries_json)),
    token_count INTEGER CHECK (token_count IS NULL OR token_count >= 0),
    created_at TEXT NOT NULL,
    UNIQUE (team_id, id),
    UNIQUE (instance_id, generation, sequence),
    FOREIGN KEY (team_id, instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE CASCADE
);

CREATE INDEX agent_context_snapshots_owner_idx
    ON agent_context_snapshots (instance_id, generation, sequence);

ALTER TABLE llm_requests
    ADD COLUMN agent_team_id TEXT REFERENCES agent_teams(id) ON DELETE SET NULL;
ALTER TABLE llm_requests
    ADD COLUMN agent_instance_id TEXT REFERENCES agent_instances(id) ON DELETE SET NULL;
ALTER TABLE llm_requests
    ADD COLUMN agent_task_id TEXT REFERENCES agent_tasks(id) ON DELETE SET NULL;
ALTER TABLE llm_requests
    ADD COLUMN agent_attempt_id TEXT REFERENCES agent_attempts(id) ON DELETE SET NULL;

CREATE INDEX llm_requests_agent_team_idx ON llm_requests (agent_team_id, request_started_at);
CREATE INDEX llm_requests_agent_instance_idx ON llm_requests (agent_instance_id, request_started_at);
CREATE INDEX llm_requests_agent_task_idx ON llm_requests (agent_task_id, request_started_at);
CREATE INDEX llm_requests_agent_attempt_idx ON llm_requests (agent_attempt_id);
"#;

pub const MIGRATION_011: &str = r#"
PRAGMA legacy_alter_table = ON;

ALTER TABLE agent_messages RENAME TO agent_messages_old;

CREATE TABLE agent_messages (
    id TEXT PRIMARY KEY NOT NULL CHECK (id GLOB 'agent-message-*'),
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
    sender_instance_id TEXT,
    receiver_instance_id TEXT NOT NULL,
    related_task_id TEXT,
    reply_to_message_id TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('notification', 'reply')),
    content TEXT NOT NULL CHECK (length(content) > 0),
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    created_at TEXT NOT NULL,
    consumed_at TEXT,
    UNIQUE (team_id, id),
    UNIQUE (receiver_instance_id, sequence),
    FOREIGN KEY (team_id, sender_instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (team_id, receiver_instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE CASCADE,
    FOREIGN KEY (team_id, related_task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, reply_to_message_id)
        REFERENCES agent_messages(team_id, id) ON DELETE SET NULL
);

INSERT INTO agent_messages
    (id, team_id, sender_instance_id, receiver_instance_id, related_task_id,
     reply_to_message_id, kind, content, sequence, created_at, consumed_at)
SELECT
    id,
    team_id,
    sender_instance_id,
    receiver_instance_id,
    related_task_id,
    reply_to_message_id,
    CASE kind
        WHEN 'response' THEN 'reply'
        ELSE 'notification'
    END,
    content,
    sequence,
    created_at,
    consumed_at
FROM agent_messages_old;

DROP TABLE agent_messages_old;

CREATE INDEX agent_messages_unread_idx
    ON agent_messages (receiver_instance_id, sequence)
    WHERE consumed_at IS NULL;
CREATE INDEX agent_messages_task_idx
    ON agent_messages (team_id, related_task_id);

ALTER TABLE agent_events RENAME TO agent_events_old;

CREATE TABLE agent_events (
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    event_type TEXT NOT NULL CHECK (length(event_type) > 0),
    instance_id TEXT,
    task_id TEXT,
    attempt_id TEXT,
    message_id TEXT,
    payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
    created_at TEXT NOT NULL,
    PRIMARY KEY (team_id, sequence),
    FOREIGN KEY (team_id, instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, attempt_id)
        REFERENCES agent_attempts(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, message_id)
        REFERENCES agent_messages(team_id, id) ON DELETE SET NULL
);

INSERT INTO agent_events
    (team_id, sequence, event_type, instance_id, task_id, attempt_id,
     message_id, payload_json, created_at)
SELECT
    team_id, sequence, event_type, instance_id, task_id, attempt_id,
    message_id, payload_json, created_at
FROM agent_events_old;

DROP TABLE agent_events_old;

CREATE INDEX agent_events_entity_idx
    ON agent_events (team_id, instance_id, task_id, sequence);

ALTER TABLE agent_context_entries RENAME TO agent_context_entries_old;

CREATE TABLE agent_context_entries (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
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
    FOREIGN KEY (team_id, instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE CASCADE,
    FOREIGN KEY (team_id, source_task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, source_message_id)
        REFERENCES agent_messages(team_id, id) ON DELETE SET NULL
);

INSERT INTO agent_context_entries
    (id, team_id, instance_id, generation, sequence, role, content_json,
     source_task_id, source_message_id, created_at)
SELECT
    id, team_id, instance_id, generation, sequence, role, content_json,
    source_task_id, source_message_id, created_at
FROM agent_context_entries_old;

DROP TABLE agent_context_entries_old;

CREATE INDEX agent_context_entries_owner_idx
    ON agent_context_entries (instance_id, generation, sequence);

PRAGMA legacy_alter_table = OFF;
"#;

pub(crate) const MIGRATION_012: &str = r#"
ALTER TABLE agent_task_dependencies
    ADD COLUMN pending_tool_call_id TEXT CHECK (pending_tool_call_id IS NULL OR length(pending_tool_call_id) > 0);

ALTER TABLE agent_task_dependencies
    ADD COLUMN deadline_at TEXT;

DROP INDEX agent_attempts_one_active_per_task_idx;

CREATE UNIQUE INDEX agent_attempts_one_active_per_task_idx
    ON agent_attempts (task_id)
    WHERE status = 'running';
"#;

pub(crate) const MIGRATION_013: &str = r#"
ALTER TABLE agent_instances
    ADD COLUMN execution_workspace_mode TEXT NOT NULL DEFAULT 'shared' CHECK (execution_workspace_mode IN ('shared', 'isolated_worktree'));

ALTER TABLE agent_instances
    ADD COLUMN execution_root_path TEXT CHECK (execution_root_path IS NULL OR length(execution_root_path) > 0);

ALTER TABLE agent_instances
    ADD COLUMN worktree_base_revision TEXT CHECK (worktree_base_revision IS NULL OR length(worktree_base_revision) > 0);

ALTER TABLE agent_instances
    ADD COLUMN worktree_branch TEXT CHECK (worktree_branch IS NULL OR length(worktree_branch) > 0);

ALTER TABLE agent_instances
    ADD COLUMN worktree_status TEXT CHECK (worktree_status IS NULL OR worktree_status IN ('active', 'kept', 'archived', 'deleted'));

CREATE INDEX agent_instances_execution_workspace_idx
    ON agent_instances (team_id, execution_workspace_mode, worktree_status);
"#;

pub(crate) const MIGRATION_014: &str = r#"
ALTER TABLE agent_events RENAME TO agent_events_old;

CREATE TABLE agent_events (
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    event_type TEXT NOT NULL CHECK (length(event_type) > 0),
    instance_id TEXT,
    task_id TEXT,
    attempt_id TEXT,
    message_id TEXT,
    payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
    created_at TEXT NOT NULL,
    PRIMARY KEY (team_id, sequence),
    FOREIGN KEY (team_id, instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, attempt_id)
        REFERENCES agent_attempts(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, message_id)
        REFERENCES agent_messages(team_id, id) ON DELETE SET NULL
);

INSERT INTO agent_events
    (team_id, sequence, event_type, instance_id, task_id, attempt_id,
     message_id, payload_json, created_at)
SELECT
    team_id, sequence, event_type, instance_id, task_id, attempt_id,
    message_id, payload_json, created_at
FROM agent_events_old;

DROP TABLE agent_events_old;

CREATE INDEX agent_events_entity_idx
    ON agent_events (team_id, instance_id, task_id, sequence);

ALTER TABLE agent_context_entries RENAME TO agent_context_entries_old;

CREATE TABLE agent_context_entries (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    team_id TEXT NOT NULL REFERENCES agent_teams(id) ON DELETE CASCADE,
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
    FOREIGN KEY (team_id, instance_id)
        REFERENCES agent_instances(team_id, id) ON DELETE CASCADE,
    FOREIGN KEY (team_id, source_task_id)
        REFERENCES agent_tasks(team_id, id) ON DELETE SET NULL,
    FOREIGN KEY (team_id, source_message_id)
        REFERENCES agent_messages(team_id, id) ON DELETE SET NULL
);

INSERT INTO agent_context_entries
    (id, team_id, instance_id, generation, sequence, role, content_json,
     source_task_id, source_message_id, created_at)
SELECT
    id, team_id, instance_id, generation, sequence, role, content_json,
    source_task_id, source_message_id, created_at
FROM agent_context_entries_old;

DROP TABLE agent_context_entries_old;

CREATE INDEX agent_context_entries_owner_idx
    ON agent_context_entries (instance_id, generation, sequence);
"#;

pub(crate) const MIGRATION_015: &str = r#"
CREATE TABLE scheduled_tasks (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),

    title TEXT NOT NULL CHECK(length(title) > 0),
    description TEXT,

    schedule_json TEXT NOT NULL CHECK(length(schedule_json) > 0),
    action_json TEXT NOT NULL CHECK(length(action_json) > 0),

    status TEXT NOT NULL CHECK(status IN (
        'enabled',
        'paused',
        'completed',
        'archived'
    )),

    next_run_at TEXT,
    last_run_at TEXT,

    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX scheduled_tasks_status_next_run_idx
ON scheduled_tasks(status, next_run_at);

CREATE TABLE scheduled_task_runs (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),

    task_id TEXT NOT NULL
        REFERENCES scheduled_tasks(id) ON DELETE CASCADE,

    trigger_reason TEXT NOT NULL CHECK(trigger_reason IN (
        'scheduled',
        'manual',
        'retry',
        'misfire_catch_up'
    )),

    status TEXT NOT NULL CHECK(status IN (
        'pending',
        'queued',
        'running',
        'succeeded',
        'failed',
        'cancelled',
        'skipped'
    )),

    scheduled_at TEXT NOT NULL,
    queued_at TEXT,
    started_at TEXT,
    completed_at TEXT,

    chat_id TEXT REFERENCES chats(id) ON DELETE SET NULL,
    user_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    assistant_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,

    agent_team_id TEXT REFERENCES agent_teams(id) ON DELETE SET NULL,
    agent_task_id TEXT REFERENCES agent_tasks(id) ON DELETE SET NULL,
    agent_attempt_id TEXT REFERENCES agent_attempts(id) ON DELETE SET NULL,
    active_run_id TEXT,

    error_message TEXT,
    output_summary TEXT,

    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX scheduled_task_runs_task_scheduled_idx
ON scheduled_task_runs(task_id, scheduled_at DESC);

CREATE INDEX scheduled_task_runs_status_idx
ON scheduled_task_runs(status);

CREATE INDEX scheduled_task_runs_chat_idx
ON scheduled_task_runs(chat_id);

CREATE INDEX scheduled_task_runs_agent_task_idx
ON scheduled_task_runs(agent_task_id);
"#;

pub(crate) const MIGRATION_018: &str = r#"
CREATE TABLE workspace_specs (
    id TEXT PRIMARY KEY NOT NULL CHECK (id = 'default'),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    inject_enabled INTEGER NOT NULL CHECK (inject_enabled IN (0, 1)),
    content_markdown TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 0),
    generated_at TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE workspace_spec_jobs (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    trigger_type TEXT NOT NULL CHECK (trigger_type IN ('manual_initial', 'manual_refresh', 'chat_completed')),
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'skipped', 'failed')),
    chat_id TEXT REFERENCES chats(id) ON DELETE SET NULL,
    run_id TEXT CHECK (run_id IS NULL OR length(run_id) > 0),
    model_id TEXT CHECK (model_id IS NULL OR length(model_id) > 0),
    base_revision INTEGER CHECK (base_revision IS NULL OR base_revision >= 0),
    input_summary_json TEXT NOT NULL DEFAULT '{}',
    output_json TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);

CREATE TABLE chat_spec_snapshots (
    chat_id TEXT PRIMARY KEY NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    spec_revision INTEGER NOT NULL CHECK (spec_revision >= 0),
    content_markdown TEXT NOT NULL,
    created_at TEXT NOT NULL
);
"#;

pub(crate) const MIGRATION_019: &str = r#"
PRAGMA legacy_alter_table = ON;

ALTER TABLE memory_extraction_jobs RENAME TO memory_extraction_jobs_old;

CREATE TABLE memory_extraction_jobs (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope IN ('workspace', 'chat')),
    chat_id TEXT REFERENCES chats(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed', 'skipped')),
    model_id TEXT CHECK (model_id IS NULL OR length(model_id) > 0),
    input_json TEXT NOT NULL,
    output_json TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    CHECK ((scope = 'chat' AND chat_id IS NOT NULL) OR (scope = 'workspace' AND chat_id IS NULL))
);

INSERT INTO memory_extraction_jobs (
    id, scope, chat_id, status, model_id, input_json, output_json,
    error_message, created_at, started_at, completed_at
)
SELECT
    id, scope, chat_id, status, model_id, input_json, output_json,
    error_message, created_at, started_at, completed_at
FROM memory_extraction_jobs_old;

DROP TABLE memory_extraction_jobs_old;

CREATE INDEX memory_extraction_jobs_scope_status_idx ON memory_extraction_jobs (scope, status);
CREATE INDEX memory_extraction_jobs_chat_idx ON memory_extraction_jobs (chat_id);
CREATE INDEX memory_extraction_jobs_created_idx ON memory_extraction_jobs (created_at);
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
