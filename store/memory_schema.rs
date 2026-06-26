pub(crate) struct MemoryMigration {
    pub(crate) version: u32,
    pub(crate) sql: &'static str,
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

CREATE INDEX memory_extraction_jobs_scope_status_idx ON memory_extraction_jobs (scope, status);
CREATE INDEX memory_extraction_jobs_chat_idx ON memory_extraction_jobs (chat_id);
CREATE INDEX memory_extraction_jobs_created_idx ON memory_extraction_jobs (created_at);
"#;

pub const WORKSPACE_MEMORY_DREAM_SCHEMA_SQL: &str = r#"
CREATE TABLE memory_dream_jobs (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope = 'workspace'),
    workspace_id TEXT CHECK (workspace_id IS NULL OR length(workspace_id) > 0),
    trigger_type TEXT NOT NULL CHECK (trigger_type IN ('manual', 'auto_interval', 'auto_threshold')),
    mode TEXT NOT NULL CHECK (mode IN ('deterministic_only', 'llm')),
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled', 'skipped')),
    model_id TEXT CHECK (model_id IS NULL OR length(model_id) > 0),
    input_summary_json TEXT NOT NULL DEFAULT '{}',
    output_summary_json TEXT,
    transcript_chat_id TEXT CHECK (transcript_chat_id IS NULL OR length(transcript_chat_id) > 0),
    error_message TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX memory_dream_jobs_scope_status_created_idx
    ON memory_dream_jobs (scope, status, created_at);
CREATE INDEX memory_dream_jobs_workspace_idx ON memory_dream_jobs (workspace_id);

CREATE TABLE memory_dream_changes (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    job_id TEXT NOT NULL REFERENCES memory_dream_jobs(id) ON DELETE CASCADE,
    operation TEXT NOT NULL CHECK (length(operation) > 0),
    target_fact_ids_json TEXT NOT NULL DEFAULT '[]',
    new_fact_id TEXT CHECK (new_fact_id IS NULL OR length(new_fact_id) > 0),
    before_json TEXT,
    after_json TEXT,
    reason TEXT NOT NULL CHECK (length(reason) > 0),
    confidence REAL CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
    risk_level TEXT NOT NULL CHECK (risk_level IN ('low', 'medium', 'high')),
    status TEXT NOT NULL CHECK (status IN ('proposed', 'applied', 'skipped', 'failed')),
    evidence_json TEXT NOT NULL DEFAULT '[]',
    error_message TEXT,
    created_at TEXT NOT NULL,
    applied_at TEXT
);

CREATE INDEX memory_dream_changes_job_status_idx
    ON memory_dream_changes (job_id, status);
CREATE INDEX memory_dream_changes_target_fact_ids_idx
    ON memory_dream_changes (target_fact_ids_json);
CREATE INDEX memory_dream_changes_new_fact_idx ON memory_dream_changes (new_fact_id);
"#;

pub const MEMORY_REFERENCES_SCHEMA_SQL: &str = r#"
CREATE TABLE memory_references (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    fact_id TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    reference_type TEXT NOT NULL CHECK (reference_type IN ('file_path', 'symbol', 'command', 'url', 'workspace_id')),
    value TEXT NOT NULL CHECK (length(value) > 0),
    normalized_value TEXT NOT NULL CHECK (length(normalized_value) > 0),
    status TEXT NOT NULL CHECK (status IN ('valid', 'invalid', 'ambiguous', 'skipped')),
    metadata_json TEXT NOT NULL DEFAULT '{}',
    checked_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (fact_id, reference_type, normalized_value)
);

CREATE INDEX memory_references_fact_idx ON memory_references (fact_id);
CREATE INDEX memory_references_type_status_idx ON memory_references (reference_type, status);
CREATE INDEX memory_references_normalized_idx ON memory_references (normalized_value);
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
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed', 'skipped')),
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

pub const GLOBAL_MEMORY_EXTRACTION_SKIPPED_STATUS_MIGRATION_SQL: &str = r#"
PRAGMA legacy_alter_table = ON;

ALTER TABLE memory_extraction_jobs RENAME TO memory_extraction_jobs_old;

CREATE TABLE memory_extraction_jobs (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope = 'global'),
    chat_id TEXT CHECK (chat_id IS NULL),
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed', 'skipped')),
    model_id TEXT CHECK (model_id IS NULL OR length(model_id) > 0),
    input_json TEXT NOT NULL,
    output_json TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
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
CREATE INDEX memory_extraction_jobs_created_idx ON memory_extraction_jobs (created_at);
"#;

pub const GLOBAL_MEMORY_DREAM_SCHEMA_SQL: &str = r#"
CREATE TABLE memory_dream_jobs (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    scope TEXT NOT NULL CHECK (scope = 'global'),
    workspace_id TEXT CHECK (workspace_id IS NULL),
    trigger_type TEXT NOT NULL CHECK (trigger_type IN ('manual', 'auto_interval', 'auto_threshold')),
    mode TEXT NOT NULL CHECK (mode IN ('deterministic_only', 'llm')),
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled', 'skipped')),
    model_id TEXT CHECK (model_id IS NULL OR length(model_id) > 0),
    input_summary_json TEXT NOT NULL DEFAULT '{}',
    output_summary_json TEXT,
    transcript_chat_id TEXT CHECK (transcript_chat_id IS NULL OR length(transcript_chat_id) > 0),
    error_message TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX memory_dream_jobs_scope_status_created_idx
    ON memory_dream_jobs (scope, status, created_at);

CREATE TABLE memory_dream_changes (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    job_id TEXT NOT NULL REFERENCES memory_dream_jobs(id) ON DELETE CASCADE,
    operation TEXT NOT NULL CHECK (length(operation) > 0),
    target_fact_ids_json TEXT NOT NULL DEFAULT '[]',
    new_fact_id TEXT CHECK (new_fact_id IS NULL OR length(new_fact_id) > 0),
    before_json TEXT,
    after_json TEXT,
    reason TEXT NOT NULL CHECK (length(reason) > 0),
    confidence REAL CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
    risk_level TEXT NOT NULL CHECK (risk_level IN ('low', 'medium', 'high')),
    status TEXT NOT NULL CHECK (status IN ('proposed', 'applied', 'skipped', 'failed')),
    evidence_json TEXT NOT NULL DEFAULT '[]',
    error_message TEXT,
    created_at TEXT NOT NULL,
    applied_at TEXT
);

CREATE INDEX memory_dream_changes_job_status_idx
    ON memory_dream_changes (job_id, status);
CREATE INDEX memory_dream_changes_target_fact_ids_idx
    ON memory_dream_changes (target_fact_ids_json);
CREATE INDEX memory_dream_changes_new_fact_idx ON memory_dream_changes (new_fact_id);
"#;
