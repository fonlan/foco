use std::{
    fs,
    sync::{Arc, Barrier, mpsc},
    thread,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::{Path, PathBuf};

use foco_agent::{
    AgentAttemptId, AgentDefinitionId, AgentDomainErrorCode, AgentExecutionWorkspaceMode,
    AgentInstanceId, AgentInstanceStatus, AgentMessageId, AgentMessageKind, AgentPermissions,
    AgentRole, AgentTaskId, AgentTaskStatus, AgentTaskTransition, AgentTaskWaitMode, AgentTeamId,
    AgentTeamStatus,
};
use foco_store::{
    config::{AgentDefinitionSettings, AgentModelOptions, WorkspaceConfig},
    memory::{
        MEMORY_DREAM_TRANSCRIPT_CHAT_KIND, MEMORY_REFERENCES_SCHEMA_SQL, MemoryDatabase,
        MemoryKind, MemoryScope, MemorySourceType, MemoryStatus, NewMemoryFact, NewMemorySource,
        WORKSPACE_MEMORY_DREAM_SCHEMA_SQL, WORKSPACE_MEMORY_SCHEMA_SQL,
    },
    workspace::{
        AgentTaskStateUpdate, AgentTaskWaitRegistrationOutcome, LlmRequestAuditFilters,
        LlmRequestRecord, LlmRequestTransport, LlmRequestUsageRollupFilters,
        MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS, MessageMetadataMutation,
        NEXT_ENABLED_SCHEDULED_TASK_SQL, NewAgentContextEntry, NewAgentContextSnapshot,
        NewAgentEvent, NewAgentInstance, NewAgentMessage, NewAgentTask, NewAgentTaskDependency,
        NewAgentTeam, NewCodeGraphEdge, NewCodeGraphFileIndex, NewCodeGraphImport,
        NewCodeGraphReference, NewCodeGraphSymbol, NewContextCompressionSnapshot, NewLlmRequest,
        NewLlmRequestEvent, NewMessage, NewPlan, NewPlanPhase, NewPlanPhaseDerivedEffects,
        NewPlanStep, NewPromptContextInjection, NewRunEvent, NewScheduledTask, NewScheduledTaskRun,
        NewTerminalSession, NewToolCall, NewToolResult, NewWorkspaceSpecJob, PlanListFilter,
        PlanListOrder, PlanPatch, PlanPhaseAttemptTrigger, PlanStepPatch,
        PreStreamChatFailureClosure, PreStreamChatFailureClosureResult, RUNNABLE_AGENT_TASKS_SQL,
        RegisterAgentTaskWaitDependencies, RemotePreStreamFailureClosureOutcome,
        RemoteQueuedRunClaimOutcome, RemoteQueuedRunClearOutcome, RewriteChatFromUserMessage,
        ScheduledTaskDueRunClaim, ScheduledTaskListFilter, ScheduledTaskRunUpdate,
        ScheduledTaskUpdate, TodoGraphFilter, TodoGraphTask, TodoGraphTaskPatch,
        UpdateLlmRequestOutcome, WORKSPACE_SCHEMA_VERSION, WORKSPACE_SPEC_MAX_MARKDOWN_BYTES,
        WORKSPACE_SPEC_STALE_REVISION_SKIP_REASON, WORKSPACE_SPEC_V1_OUTPUT_STRATEGY,
        WorkspaceDatabase, WorkspaceDatabaseError, WorkspaceSpecJobEnqueueDecision,
        WorkspaceSpecJobStatus, WorkspaceSpecOutputStrategy, WorkspaceSpecPromptPlan,
        WorkspaceSpecSettings, WorkspaceSpecTriggerType, WorkspaceSpecWriteDecision,
        initialize_workspace_databases, llm_request_audit_count_sql_for_tests,
        llm_request_audit_request_kind_breakdown_sql_for_tests,
        llm_request_audit_rows_sql_for_tests, llm_request_audit_summary_sql_for_tests,
        prune_workspace_database_backups, scheduled_task_count_sql_for_tests,
        scheduled_tasks_page_sql_for_tests, workspace_database_path,
    },
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

#[cfg(unix)]
fn sqlite_sidecar_path(database_path: &Path, suffix: &str) -> PathBuf {
    let mut path = database_path.as_os_str().to_os_string();
    path.push(suffix);
    PathBuf::from(path)
}

#[test]
fn creates_workspace_foco_database_and_runs_migrations() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    assert!(workspace.path().join(".foco").is_dir());
    assert!(workspace_database_path(workspace.path()).is_file());
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );

    let connection = Connection::open(database.database_path()).expect("open database");
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("journal mode");
    assert_eq!(journal_mode, "wal");
    connection
        .execute(
            "INSERT INTO workspace_metadata (key, value, updated_at)
             VALUES ('permission_probe', 'true', '2026-07-16T00:00:00Z')",
            [],
        )
        .expect("create WAL sidecars");

    for table in [
        "workspace_metadata",
        "chats",
        "messages",
        "run_events",
        "tool_calls",
        "tool_results",
        "terminal_sessions",
        "llm_requests",
        "llm_request_usage_rollups",
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
        "memory_dream_jobs",
        "memory_dream_changes",
        "memory_references",
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
        "scheduled_tasks",
        "scheduled_task_runs",
        "workspace_specs",
        "workspace_spec_jobs",
        "chat_spec_snapshots",
        "plans",
        "plan_phases",
        "plan_steps",
        "plan_phase_attempts",
    ] {
        assert!(
            table_exists(&connection, table),
            "{table} table should exist"
        );
    }
    assert!(column_exists(&connection, "memory_facts", "enabled"));
    let (enabled_type, enabled_not_null, enabled_default): (String, i64, Option<String>) =
        connection
            .query_row(
                "SELECT type, \"notnull\", dflt_value
             FROM pragma_table_info('memory_facts')
             WHERE name = 'enabled'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("memory enabled column definition");
    assert_eq!(enabled_type, "INTEGER");
    assert_eq!(enabled_not_null, 1);
    assert_eq!(enabled_default.as_deref(), Some("1"));

    #[cfg(unix)]
    {
        assert_eq!(
            fs::metadata(workspace.path().join(".foco"))
                .expect("workspace private directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(database.database_path())
                .expect("workspace database metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        for suffix in ["-wal", "-shm"] {
            let sidecar = sqlite_sidecar_path(database.database_path(), suffix);
            assert!(sidecar.is_file(), "{} should exist", sidecar.display());
            assert_eq!(
                fs::metadata(&sidecar)
                    .expect("workspace SQLite sidecar metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}

#[test]
fn concurrent_first_open_serializes_workspace_migrations() {
    const THREAD_COUNT: usize = 8;

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let workspace_path = Arc::new(workspace.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(THREAD_COUNT));
    let threads = (0..THREAD_COUNT)
        .map(|_| {
            let workspace_path = Arc::clone(&workspace_path);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                WorkspaceDatabase::open_or_create_ungated(workspace_path.as_path())
                    .and_then(|database| database.schema_version())
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        let schema_version = thread
            .join()
            .expect("workspace database open thread")
            .expect("concurrent workspace database open");
        assert_eq!(schema_version, WORKSPACE_SCHEMA_VERSION);
    }

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
}

#[test]
fn concurrent_old_workspace_open_serializes_migration_backup() {
    const THREAD_COUNT: usize = 8;

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    {
        let connection = Connection::open(&database_path).expect("old database");
        connection
            .execute_batch(
                "CREATE TABLE legacy_data (id INTEGER PRIMARY KEY);
                 INSERT INTO legacy_data DEFAULT VALUES;
                 PRAGMA user_version = 0;",
            )
            .expect("old schema");
    }

    let workspace_path = Arc::new(workspace.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(THREAD_COUNT));
    let threads = (0..THREAD_COUNT)
        .map(|_| {
            let workspace_path = Arc::clone(&workspace_path);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                WorkspaceDatabase::open_or_create_ungated(workspace_path.as_path())
                    .and_then(|database| database.schema_version())
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        let schema_version = thread
            .join()
            .expect("workspace database open thread")
            .expect("concurrent old workspace database open");
        assert_eq!(schema_version, WORKSPACE_SCHEMA_VERSION);
    }

    let backup_dir = workspace.path().join(".foco").join("backups");
    let backups = fs::read_dir(&backup_dir)
        .expect("backup directory")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .collect::<Vec<_>>();
    assert_eq!(
        backups.len(),
        1,
        "migration backup must be created exactly once under concurrent open"
    );

    #[cfg(unix)]
    {
        assert_eq!(
            fs::metadata(&backup_dir)
                .expect("backup directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            backups[0]
                .metadata()
                .expect("backup metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn concurrent_global_memory_open_serializes_migrations() {
    const THREAD_COUNT: usize = 8;

    let root = tempfile::tempdir().expect("global memory root");
    let database_path = root.path().join("memory.sqlite");
    {
        let connection = Connection::open(&database_path).expect("old global memory database");
        connection
            .execute_batch(
                "CREATE TABLE legacy_memory (id INTEGER PRIMARY KEY);
                 INSERT INTO legacy_memory DEFAULT VALUES;
                 PRAGMA user_version = 0;",
            )
            .expect("old global memory schema");
    }

    let database_path = Arc::new(database_path);
    let barrier = Arc::new(Barrier::new(THREAD_COUNT));
    let threads = (0..THREAD_COUNT)
        .map(|_| {
            let database_path = Arc::clone(&database_path);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                MemoryDatabase::open_or_create_global_at(database_path.as_path())
                    .and_then(|database| database.schema_version())
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        let schema_version = thread
            .join()
            .expect("global memory open thread")
            .expect("concurrent global memory open");
        assert_eq!(
            schema_version,
            foco_store::memory::GLOBAL_MEMORY_SCHEMA_VERSION
        );
    }

    let database =
        MemoryDatabase::open_or_create_global_at(database_path.as_path()).expect("global memory");
    assert_eq!(
        database.schema_version().expect("schema version"),
        foco_store::memory::GLOBAL_MEMORY_SCHEMA_VERSION
    );
    let connection = Connection::open(database_path.as_path()).expect("open global memory");
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("journal mode");
    assert_eq!(journal_mode, "wal");
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS permission_probe (id INTEGER PRIMARY KEY);
             INSERT INTO permission_probe DEFAULT VALUES;",
        )
        .expect("create global memory WAL sidecars");

    #[cfg(unix)]
    {
        assert_eq!(
            fs::metadata(root.path())
                .expect("global memory directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(database_path.as_path())
                .expect("global memory database metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        for suffix in ["-wal", "-shm"] {
            let sidecar = sqlite_sidecar_path(database_path.as_path(), suffix);
            assert!(sidecar.is_file(), "{} should exist", sidecar.display());
            assert_eq!(
                fs::metadata(&sidecar)
                    .expect("global memory SQLite sidecar metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}

#[test]
fn mark_and_clear_chat_queued_run_are_atomic_with_unrelated_metadata() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_chat_with_metadata(
            "chat-queued-atomic",
            "Queued chat",
            r#"{"queuedRun":{"status":"queued","userMessageId":"user-queued-atomic","modelId":"model","providerId":"provider","content":"hello"},"planOrigin":{"planId":"plan-1","phaseId":"phase-1"},"skillIds":["skill-a"]}"#,
        )
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "user-queued-atomic",
            chat_id: "chat-queued-atomic",
            role: "user",
            content: "hello",
            sequence: 0,
            metadata_json: Some(
                r#"{"queuedRun":{"status":"queued","modelId":"model","providerId":"provider"},"skillIds":["skill-a"]}"#,
            ),
        })
        .expect("message insert");

    database
        .mark_chat_queued_run_started(
            "chat-queued-atomic",
            "user-queued-atomic",
            "assistant-queued-atomic",
            1,
        )
        .expect("queued run started");

    let chat_metadata: Value = serde_json::from_str(
        &database
            .chat("chat-queued-atomic")
            .expect("chat read")
            .expect("chat")
            .metadata_json,
    )
    .expect("chat metadata json");
    assert_eq!(chat_metadata["queuedRun"]["status"], "running");
    assert_eq!(chat_metadata["planOrigin"]["planId"], "plan-1");
    assert_eq!(chat_metadata["skillIds"][0], "skill-a");

    database
        .clear_chat_queued_run("chat-queued-atomic", "user-other")
        .expect("clear other queued run is no-op for chat");
    let chat_metadata_after_mismatch: Value = serde_json::from_str(
        &database
            .chat("chat-queued-atomic")
            .expect("chat read")
            .expect("chat")
            .metadata_json,
    )
    .expect("chat metadata json");
    assert_eq!(
        chat_metadata_after_mismatch["queuedRun"]["status"],
        "running"
    );

    database
        .clear_chat_queued_run("chat-queued-atomic", "user-queued-atomic")
        .expect("clear matching queued run");
    let chat_metadata_cleared: Value = serde_json::from_str(
        &database
            .chat("chat-queued-atomic")
            .expect("chat read")
            .expect("chat")
            .metadata_json,
    )
    .expect("chat metadata json");
    assert!(chat_metadata_cleared.get("queuedRun").is_none());
    assert_eq!(chat_metadata_cleared["planOrigin"]["planId"], "plan-1");
    assert_eq!(chat_metadata_cleared["skillIds"][0], "skill-a");
}

#[test]
fn concurrent_mark_and_clear_queued_run_preserve_chat_message_identity() {
    const THREAD_COUNT: usize = 6;

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat_with_metadata(
            "chat-race",
            "Race chat",
            r#"{"queuedRun":{"status":"queued","userMessageId":"user-race","modelId":"model","providerId":"provider"},"keep":"yes"}"#,
        )
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "user-race",
            chat_id: "chat-race",
            role: "user",
            content: "hello",
            sequence: 0,
            metadata_json: Some(
                r#"{"queuedRun":{"status":"queued","modelId":"model","providerId":"provider"},"keep":"yes"}"#,
            ),
        })
        .expect("message insert");
    drop(database);

    let workspace_path = Arc::new(workspace.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(THREAD_COUNT));
    let threads = (0..THREAD_COUNT)
        .map(|index| {
            let workspace_path = Arc::clone(&workspace_path);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let mut database =
                    WorkspaceDatabase::open_or_create_ungated(workspace_path.as_path())
                        .expect("workspace database");
                if index % 2 == 0 {
                    database
                        .mark_chat_queued_run_started("chat-race", "user-race", "assistant-race", 1)
                        .expect("mark started");
                } else {
                    database
                        .clear_chat_queued_run("chat-race", "user-race")
                        .expect("clear queued run");
                }
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        thread.join().expect("queued run race thread");
    }

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    let chat = database.chat("chat-race").expect("chat").expect("chat row");
    let message = database
        .message("user-race")
        .expect("message")
        .expect("message row");
    let chat_metadata: Value = serde_json::from_str(&chat.metadata_json).expect("chat metadata");
    let message_metadata: Value =
        serde_json::from_str(&message.metadata_json).expect("message metadata");
    assert_eq!(chat_metadata["keep"], "yes");
    assert_eq!(message_metadata["keep"], "yes");
    let chat_has_queued = chat_metadata.get("queuedRun").is_some();
    let message_has_queued = message_metadata.get("queuedRun").is_some();
    if chat_has_queued {
        assert_eq!(chat_metadata["queuedRun"]["userMessageId"], "user-race");
        assert!(
            message_has_queued,
            "chat and message queuedRun identity must stay aligned when present"
        );
    }
}

#[test]
fn remote_queued_run_claim_replays_for_owner_and_rejects_other_run() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-remote-claim", "Remote claim")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "user-remote-claim",
            chat_id: "chat-remote-claim",
            role: "user",
            content: "hello",
            sequence: 0,
            metadata_json: Some(
                &json!({
                    "queuedRun": {
                        "status": "queued",
                        "userMessageId": "user-remote-claim",
                        "assistantMessageId": "assistant-remote-claim",
                    }
                })
                .to_string(),
            ),
        })
        .expect("user insert");
    database
        .insert_message(NewMessage {
            id: "assistant-remote-claim",
            chat_id: "chat-remote-claim",
            role: "assistant",
            content: "",
            sequence: 1,
            metadata_json: Some(r#"{"streamingState":"streaming"}"#),
        })
        .expect("assistant insert");

    assert_eq!(
        database
            .claim_remote_queued_run(
                "chat-remote-claim",
                "user-remote-claim",
                "assistant-remote-claim",
                "remote-run-owner",
            )
            .expect("first claim"),
        RemoteQueuedRunClaimOutcome::Claimed
    );
    assert_eq!(
        database
            .claim_remote_queued_run(
                "chat-remote-claim",
                "user-remote-claim",
                "assistant-remote-claim",
                "remote-run-owner",
            )
            .expect("owner replay"),
        RemoteQueuedRunClaimOutcome::AlreadyOwned
    );
    let error = database
        .claim_remote_queued_run(
            "chat-remote-claim",
            "user-remote-claim",
            "assistant-remote-claim",
            "remote-run-other",
        )
        .expect_err("another run must not claim the durable identity");
    assert!(
        error.to_string().contains("already owned by another run"),
        "unexpected claim error: {error}"
    );
    assert_eq!(
        database
            .clear_remote_queued_run_if_owned(
                "chat-remote-claim",
                "user-remote-claim",
                "assistant-remote-claim",
                "remote-run-other",
            )
            .expect("late other-owner clear"),
        RemoteQueuedRunClearOutcome::NotOwned
    );
    assert_eq!(
        database
            .clear_remote_queued_run_if_owned(
                "chat-remote-claim",
                "user-remote-claim",
                "assistant-remote-claim",
                "remote-run-owner",
            )
            .expect("owner clear"),
        RemoteQueuedRunClearOutcome::Cleared
    );
}

#[test]
fn remote_pre_stream_failure_persists_assistant_and_clears_owned_queue_atomically() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-remote-pre-stream", "Remote pre-stream")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "user-remote-pre-stream",
            chat_id: "chat-remote-pre-stream",
            role: "user",
            content: "hello",
            sequence: 0,
            metadata_json: Some(
                &json!({
                    "keep": "metadata",
                    "queuedRun": {
                        "status": "queued",
                        "userMessageId": "user-remote-pre-stream",
                        "assistantMessageId": "assistant-remote-pre-stream",
                    }
                })
                .to_string(),
            ),
        })
        .expect("user insert");
    let assistant_metadata = json!({
        "streamingState": "failed",
        "parts": [{ "type": "error", "text": "database busy" }],
        "partsSource": "pre_stream_failure",
    })
    .to_string();
    assert_eq!(
        database
            .close_remote_pre_stream_failure_if_owned(
                "chat-remote-pre-stream",
                "user-remote-pre-stream",
                "assistant-remote-pre-stream",
                "remote-run-owner",
                "Reply has not started: workspace database is busy. Please retry.",
                &assistant_metadata,
            )
            .expect("close owned pre-stream failure"),
        RemotePreStreamFailureClosureOutcome::Applied
    );

    let user = database
        .message("user-remote-pre-stream")
        .expect("user lookup")
        .expect("user message");
    let user_metadata: Value = serde_json::from_str(&user.metadata_json).expect("user metadata");
    assert_eq!(user_metadata["keep"], "metadata");
    assert!(user_metadata.get("queuedRun").is_none());
    let assistant = database
        .message("assistant-remote-pre-stream")
        .expect("assistant lookup")
        .expect("materialized assistant");
    assert_eq!(
        assistant.content,
        "Reply has not started: workspace database is busy. Please retry."
    );
    assert_eq!(assistant.sequence, 1);
    let persisted_metadata: Value =
        serde_json::from_str(&assistant.metadata_json).expect("assistant metadata");
    assert_eq!(persisted_metadata["streamingState"], "failed");
}

#[test]
fn stale_remote_owner_cannot_clear_a_later_turn_queued_run() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-remote-turns", "Remote turns")
        .expect("chat insert");

    for (user_id, assistant_id, sequence) in [
        ("user-remote-turn-1", "assistant-remote-turn-1", 0),
        ("user-remote-turn-2", "assistant-remote-turn-2", 2),
    ] {
        database
            .insert_message(NewMessage {
                id: user_id,
                chat_id: "chat-remote-turns",
                role: "user",
                content: "continue",
                sequence,
                metadata_json: Some(
                    &json!({
                        "queuedRun": {
                            "status": "queued",
                            "userMessageId": user_id,
                            "assistantMessageId": assistant_id,
                        }
                    })
                    .to_string(),
                ),
            })
            .expect("user insert");
        database
            .insert_message(NewMessage {
                id: assistant_id,
                chat_id: "chat-remote-turns",
                role: "assistant",
                content: "",
                sequence: sequence + 1,
                metadata_json: Some(r#"{"streamingState":"streaming"}"#),
            })
            .expect("assistant insert");
    }

    assert_eq!(
        database
            .claim_remote_queued_run(
                "chat-remote-turns",
                "user-remote-turn-1",
                "assistant-remote-turn-1",
                "remote-run-1",
            )
            .expect("claim first turn"),
        RemoteQueuedRunClaimOutcome::Claimed
    );
    assert_eq!(
        database
            .clear_remote_queued_run_if_owned(
                "chat-remote-turns",
                "user-remote-turn-1",
                "assistant-remote-turn-1",
                "remote-run-1",
            )
            .expect("finish first turn"),
        RemoteQueuedRunClearOutcome::Cleared
    );
    assert_eq!(
        database
            .claim_remote_queued_run(
                "chat-remote-turns",
                "user-remote-turn-2",
                "assistant-remote-turn-2",
                "remote-run-2",
            )
            .expect("claim second turn"),
        RemoteQueuedRunClaimOutcome::Claimed
    );

    assert_eq!(
        database
            .clear_remote_queued_run_if_owned(
                "chat-remote-turns",
                "user-remote-turn-1",
                "assistant-remote-turn-1",
                "remote-run-1",
            )
            .expect("late first-run cleanup"),
        RemoteQueuedRunClearOutcome::NotOwned
    );
    let second_user = database
        .message("user-remote-turn-2")
        .expect("second user lookup")
        .expect("second user message");
    let metadata: Value =
        serde_json::from_str(&second_user.metadata_json).expect("second metadata");
    assert_eq!(
        metadata["queuedRun"],
        json!({
            "status": "running",
            "userMessageId": "user-remote-turn-2",
            "assistantMessageId": "assistant-remote-turn-2",
            "runId": "remote-run-2",
        })
    );
}

#[test]
fn concurrent_remote_queued_run_claims_choose_one_owner() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-remote-claim-race", "Remote claim race")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "user-remote-claim-race",
            chat_id: "chat-remote-claim-race",
            role: "user",
            content: "hello",
            sequence: 0,
            metadata_json: Some(
                &json!({
                    "queuedRun": {
                        "status": "queued",
                        "userMessageId": "user-remote-claim-race",
                        "assistantMessageId": "assistant-remote-claim-race",
                    }
                })
                .to_string(),
            ),
        })
        .expect("user insert");
    database
        .insert_message(NewMessage {
            id: "assistant-remote-claim-race",
            chat_id: "chat-remote-claim-race",
            role: "assistant",
            content: "",
            sequence: 1,
            metadata_json: Some(r#"{"streamingState":"streaming"}"#),
        })
        .expect("assistant insert");
    drop(database);

    let workspace_path = Arc::new(workspace.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(2));
    let (result_tx, result_rx) = mpsc::channel();
    let threads = ["remote-run-a", "remote-run-b"]
        .into_iter()
        .map(|run_id| {
            let workspace_path = Arc::clone(&workspace_path);
            let barrier = Arc::clone(&barrier);
            let result_tx = result_tx.clone();
            thread::spawn(move || {
                barrier.wait();
                let mut database =
                    WorkspaceDatabase::open_or_create_ungated(workspace_path.as_path())
                        .expect("workspace database");
                let outcome = database.claim_remote_queued_run(
                    "chat-remote-claim-race",
                    "user-remote-claim-race",
                    "assistant-remote-claim-race",
                    run_id,
                );
                result_tx
                    .send((run_id, outcome))
                    .expect("send claim outcome");
            })
        })
        .collect::<Vec<_>>();
    drop(result_tx);
    for thread in threads {
        thread.join().expect("claim thread");
    }

    let outcomes = result_rx.into_iter().collect::<Vec<_>>();
    assert_eq!(
        outcomes.iter().filter(|(_, result)| result.is_ok()).count(),
        1,
        "exactly one concurrent owner must claim the queuedRun: {outcomes:?}"
    );
    let (owner_run_id, claim_outcome) = outcomes
        .iter()
        .find_map(|(run_id, result)| result.as_ref().ok().map(|outcome| (*run_id, *outcome)))
        .expect("winning claim");
    assert_eq!(claim_outcome, RemoteQueuedRunClaimOutcome::Claimed);

    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    assert_eq!(
        database
            .claim_remote_queued_run(
                "chat-remote-claim-race",
                "user-remote-claim-race",
                "assistant-remote-claim-race",
                owner_run_id,
            )
            .expect("winner replay"),
        RemoteQueuedRunClaimOutcome::AlreadyOwned
    );
}

#[test]
fn workspace_spec_phase0_contract_defines_lifecycle_and_prompt_snapshot() {
    let disabled = WorkspaceSpecSettings::disabled();
    assert!(!disabled.allows_generation());
    assert!(!disabled.allows_update());
    assert!(!disabled.allows_injection());

    let enabled_without_injection = WorkspaceSpecSettings::enabled(false);
    assert!(enabled_without_injection.allows_generation());
    assert!(enabled_without_injection.allows_update());
    assert!(!enabled_without_injection.allows_injection());

    let enabled_with_injection = WorkspaceSpecSettings::enabled(true);
    assert!(enabled_with_injection.allows_injection());
    assert_eq!(
        WorkspaceSpecPromptPlan::for_chat(enabled_with_injection, false),
        WorkspaceSpecPromptPlan::ReadWorkspaceSpecAndSaveSnapshot
    );
    assert_eq!(
        WorkspaceSpecPromptPlan::for_chat(enabled_without_injection, false),
        WorkspaceSpecPromptPlan::SkipInjectionDisabled
    );
    assert_eq!(
        WorkspaceSpecPromptPlan::for_chat(disabled, false),
        WorkspaceSpecPromptPlan::SkipDisabled
    );
    assert_eq!(
        WorkspaceSpecPromptPlan::for_chat(disabled, true),
        WorkspaceSpecPromptPlan::UseChatSnapshot
    );

    assert_eq!(
        WorkspaceSpecTriggerType::parse("manual_initial")
            .unwrap()
            .as_str(),
        "manual_initial"
    );
    assert_eq!(
        WorkspaceSpecTriggerType::parse("chat_completed")
            .unwrap()
            .as_str(),
        "chat_completed"
    );
    assert!(WorkspaceSpecTriggerType::ManualRefresh.is_manual());
    assert!(WorkspaceSpecTriggerType::parse("manual_cancel").is_err());
}

#[test]
fn workspace_spec_phase0_contract_defines_jobs_stale_writes_and_v1_output() {
    assert_eq!(
        WorkspaceSpecJobStatus::parse("queued").unwrap().as_str(),
        "queued"
    );
    assert!(WorkspaceSpecJobStatus::Completed.is_terminal());
    assert!(!WorkspaceSpecJobStatus::Running.is_terminal());

    assert_eq!(
        WorkspaceSpecJobEnqueueDecision::for_trigger(
            WorkspaceSpecTriggerType::ManualInitial,
            false,
            false,
        ),
        WorkspaceSpecJobEnqueueDecision::QueueNow
    );
    assert_eq!(
        WorkspaceSpecJobEnqueueDecision::for_trigger(
            WorkspaceSpecTriggerType::ManualRefresh,
            true,
            false,
        ),
        WorkspaceSpecJobEnqueueDecision::RejectAlreadyRunning
    );
    assert_eq!(
        WorkspaceSpecJobEnqueueDecision::for_trigger(
            WorkspaceSpecTriggerType::ChatCompleted,
            true,
            false,
        ),
        WorkspaceSpecJobEnqueueDecision::QueuePendingRefresh
    );
    assert_eq!(
        WorkspaceSpecJobEnqueueDecision::for_trigger(
            WorkspaceSpecTriggerType::ChatCompleted,
            true,
            true,
        ),
        WorkspaceSpecJobEnqueueDecision::AlreadyPendingRefresh
    );

    assert_eq!(
        WorkspaceSpecWriteDecision::for_job_output(7, 7),
        WorkspaceSpecWriteDecision::WriteFullReplacement
    );
    assert_eq!(
        WorkspaceSpecWriteDecision::for_job_output(7, 8),
        WorkspaceSpecWriteDecision::SkipStaleRevision {
            reason: WORKSPACE_SPEC_STALE_REVISION_SKIP_REASON,
        }
    );

    assert_eq!(
        WORKSPACE_SPEC_V1_OUTPUT_STRATEGY,
        WorkspaceSpecOutputStrategy::FullReplacementMarkdown
    );
    assert!(!WORKSPACE_SPEC_V1_OUTPUT_STRATEGY.uses_patch_parser());
    assert!(!WORKSPACE_SPEC_V1_OUTPUT_STRATEGY.allows_stale_merge());
    assert!(
        WORKSPACE_SPEC_V1_OUTPUT_STRATEGY
            .validate_markdown_size("# Project Spec\n")
            .is_ok()
    );
    assert!(
        WORKSPACE_SPEC_V1_OUTPUT_STRATEGY
            .validate_markdown_size(&"x".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 1))
            .is_err()
    );
}

#[test]
fn workspace_spec_content_update_rejects_stale_revision() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    assert!(database.workspace_spec().expect("initial spec").is_none());

    let settings = database
        .upsert_workspace_spec_settings(true, true)
        .expect("settings save");
    assert!(settings.enabled);
    assert!(settings.inject_enabled);
    assert_eq!(settings.revision, 0);
    assert_eq!(settings.content_markdown, "");

    let saved = database
        .update_workspace_spec_content(0, "# Project Spec\n\nFirst")
        .expect("first save")
        .expect("saved spec");
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.content_markdown, "# Project Spec\n\nFirst");

    let stale = database
        .update_workspace_spec_content(0, "# Project Spec\n\nStale")
        .expect("stale save");
    assert!(stale.is_none());

    let current = database
        .workspace_spec()
        .expect("current spec")
        .expect("current spec row");
    assert_eq!(current.revision, 1);
    assert_eq!(current.content_markdown, "# Project Spec\n\nFirst");

    let updated = database
        .update_workspace_spec_content(1, "# Project Spec\n\nSecond")
        .expect("second save")
        .expect("updated spec");
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.content_markdown, "# Project Spec\n\nSecond");
}

#[test]
fn workspace_spec_trigger_rejects_oversized_direct_sql_update_by_utf8_bytes() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .upsert_workspace_spec_settings(true, true)
        .expect("settings save");
    database
        .update_workspace_spec_content(0, "# Project Spec\n\nValid")
        .expect("spec save")
        .expect("saved spec");
    let database_path = database.database_path().to_path_buf();
    drop(database);

    let oversized = "界".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES / 3 + 1);
    let connection = Connection::open(database_path).expect("raw connection");
    let error = connection
        .execute(
            "UPDATE workspace_specs SET content_markdown = ?1 WHERE id = 'default'",
            params![oversized],
        )
        .expect_err("direct oversized update must fail");

    assert!(
        error
            .to_string()
            .contains("workspace spec Markdown exceeds 65536 bytes")
    );
}

#[test]
fn chat_spec_snapshot_trigger_rejects_oversized_direct_sql_update() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database.insert_chat("chat-1", "Chat").expect("chat insert");
    database
        .insert_chat_spec_snapshot("chat-1", 1, "# Project Spec\n\nValid")
        .expect("snapshot insert");
    let database_path = database.database_path().to_path_buf();
    drop(database);

    let connection = Connection::open(database_path).expect("raw connection");
    let error = connection
        .execute(
            "UPDATE chat_spec_snapshots SET content_markdown = ?1 WHERE chat_id = 'chat-1'",
            params!["x".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 1)],
        )
        .expect_err("direct oversized snapshot update must fail");

    assert!(
        error
            .to_string()
            .contains("chat spec snapshot Markdown exceeds 65536 bytes")
    );
}

#[test]
fn migration_039_preserves_existing_oversized_spec_and_blocks_future_oversized_writes() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .upsert_workspace_spec_settings(true, true)
        .expect("settings save");
    database
        .update_workspace_spec_content(0, "# Project Spec\n\nValid")
        .expect("spec save")
        .expect("saved spec");
    let database_path = database.database_path().to_path_buf();
    drop(database);

    let oversized = "x".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 1);
    {
        let connection = Connection::open(&database_path).expect("raw v38 connection");
        connection
            .execute_batch(
                r#"
                DROP TRIGGER workspace_specs_markdown_bytes_insert;
                DROP TRIGGER workspace_specs_markdown_bytes_update;
                DROP TRIGGER chat_spec_snapshots_markdown_bytes_insert;
                DROP TRIGGER chat_spec_snapshots_markdown_bytes_update;
                PRAGMA user_version = 38;
                "#,
            )
            .expect("simulate v38 schema");
        connection
            .execute(
                "UPDATE workspace_specs SET content_markdown = ?1 WHERE id = 'default'",
                params![oversized],
            )
            .expect("seed legacy oversized spec");
    }

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrate to 39");
    let current = database
        .workspace_spec()
        .expect("current spec")
        .expect("spec row");
    assert_eq!(
        current.content_markdown.len(),
        WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 1
    );
    drop(database);

    let connection = Connection::open(database_path).expect("post-migration connection");
    let error = connection
        .execute(
            "UPDATE workspace_specs SET content_markdown = ?1 WHERE id = 'default'",
            params!["y".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 1)],
        )
        .expect_err("restored trigger must reject oversized update");
    assert!(
        error
            .to_string()
            .contains("workspace spec Markdown exceeds 65536 bytes")
    );
}

#[test]
fn delete_plan_removes_plan_graph_and_reports_missing() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-delete",
            title: "Delete plan",
            overview: "Remove the full plan graph.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-phase-delete",
                title: "Phase one",
                summary: "Delete cascades to this phase.",
                steps: vec![NewPlanStep {
                    id: "plan-step-delete",
                    title: "Step one",
                    detail: "Delete cascades to this step.",
                    acceptance: vec!["row is removed".to_string()],
                }],
            }],
        })
        .expect("create plan");

    assert!(database.delete_plan(" plan-delete ").expect("delete plan"));
    assert!(
        !database
            .delete_plan("plan-delete")
            .expect("delete missing plan")
    );

    let connection = Connection::open(database.database_path()).expect("open database");
    for table in ["plans", "plan_phases", "plan_steps"] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count rows");
        assert_eq!(count, 0, "{table} should be empty after deleting the plan");
    }
}

#[test]
fn plan_completed_steps_remain_active_until_user_marks_complete() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    let created = database
        .create_plan(NewPlan {
            id: "plan-history-active",
            title: "Plan history active",
            overview: "Keep implemented plans visible until the user archives them.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-phase-history-active",
                title: "Phase one",
                summary: "Implement the phase.",
                steps: vec![NewPlanStep {
                    id: "plan-step-history-active",
                    title: "Finish step",
                    detail: "Complete the implementation step.",
                    acceptance: vec!["step is checked".to_string()],
                }],
            }],
        })
        .expect("create plan");
    assert_eq!(created.status, "ready");

    let active = database
        .plans(PlanListFilter {
            view: "active",
            status: None,
            order: PlanListOrder::Manual,
            limit: 20,
            offset: 0,
        })
        .expect("active plans");
    assert_eq!(active.total_count, 1);

    let implemented = database
        .update_plan_step(
            "plan-history-active",
            "plan-step-history-active",
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some("completed"),
            },
        )
        .expect("complete plan step");
    assert_eq!(implemented.status, "implemented");
    assert!(implemented.completed_at.is_some());
    assert!(implemented.completed_by_user_at.is_none());

    let active = database
        .plans(PlanListFilter {
            view: "active",
            status: None,
            order: PlanListOrder::Manual,
            limit: 20,
            offset: 0,
        })
        .expect("implemented active plans");
    assert_eq!(active.total_count, 1);
    assert_eq!(active.plans[0].status, "implemented");

    let completed = database
        .transition_plan("plan-history-active", "mark_complete")
        .expect("mark complete");
    assert_eq!(completed.status, "completed");
    assert!(completed.completed_by_user_at.is_some());

    let active = database
        .plans(PlanListFilter {
            view: "active",
            status: None,
            order: PlanListOrder::Manual,
            limit: 20,
            offset: 0,
        })
        .expect("active plans after archive");
    assert_eq!(active.total_count, 0);
    assert!(active.plans.is_empty());

    let all_completed = database
        .plans(PlanListFilter {
            view: "all",
            status: Some("completed"),
            order: PlanListOrder::Manual,
            limit: 20,
            offset: 0,
        })
        .expect("completed history plans");
    assert_eq!(all_completed.total_count, 1);
    assert_eq!(all_completed.plans[0].status, "completed");
}

#[test]
fn plans_newest_first_orders_before_pagination() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    for plan_id in [
        "plan-oldest",
        "plan-middle-a",
        "plan-middle-b",
        "plan-newest",
    ] {
        create_minimal_plan(&mut database, plan_id, "ready");
    }
    let connection = Connection::open(database.database_path()).expect("open database");
    for (plan_id, created_at) in [
        ("plan-oldest", "2026-01-01T00:00:00.000Z"),
        ("plan-middle-a", "2026-01-02T00:00:00.000Z"),
        ("plan-middle-b", "2026-01-02T00:00:00.000Z"),
        ("plan-newest", "2026-01-03T00:00:00.000Z"),
    ] {
        connection
            .execute(
                "UPDATE plans SET created_at = ?1 WHERE id = ?2",
                params![created_at, plan_id],
            )
            .expect("set created_at");
    }

    let first_page = database
        .plans(PlanListFilter {
            view: "active",
            status: None,
            order: PlanListOrder::NewestFirst,
            limit: 2,
            offset: 0,
        })
        .expect("first newest-first page");
    let second_page = database
        .plans(PlanListFilter {
            view: "active",
            status: None,
            order: PlanListOrder::NewestFirst,
            limit: 2,
            offset: 2,
        })
        .expect("second newest-first page");

    assert_eq!(first_page.total_count, 4);
    assert_eq!(
        first_page
            .plans
            .iter()
            .map(|plan| plan.id.as_str())
            .collect::<Vec<_>>(),
        vec!["plan-newest", "plan-middle-b"]
    );
    assert_eq!(
        second_page
            .plans
            .iter()
            .map(|plan| plan.id.as_str())
            .collect::<Vec<_>>(),
        vec!["plan-middle-a", "plan-oldest"]
    );
}

#[test]
fn reorder_active_plans_updates_only_reorderable_slots() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    create_minimal_plan(&mut database, "plan-ready-1", "ready");
    create_minimal_plan(&mut database, "plan-running", "ready");
    create_minimal_plan(&mut database, "plan-ready-2", "ready");
    create_minimal_plan(&mut database, "plan-implemented", "ready");
    database
        .transition_plan("plan-running", "start")
        .expect("start running plan");
    database
        .update_plan_step(
            "plan-implemented",
            "plan-implemented-step",
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some("completed"),
            },
        )
        .expect("implement plan");

    database
        .reorder_active_plans(&["plan-ready-2".to_string(), "plan-ready-1".to_string()])
        .expect("reorder active plans");

    let active = database
        .plans(PlanListFilter {
            view: "active",
            status: None,
            order: PlanListOrder::Manual,
            limit: 20,
            offset: 0,
        })
        .expect("active plans");
    let plan_order = active
        .plans
        .iter()
        .map(|plan| plan.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        plan_order,
        vec![
            "plan-ready-2",
            "plan-running",
            "plan-ready-1",
            "plan-implemented"
        ]
    );

    let duplicate_error = database
        .reorder_active_plans(&["plan-ready-2".to_string(), "plan-ready-2".to_string()])
        .expect_err("duplicate id should fail");
    assert!(duplicate_error.to_string().contains("duplicate id"));

    let missing_error = database
        .reorder_active_plans(&["plan-ready-2".to_string()])
        .expect_err("missing id should fail");
    assert!(
        missing_error
            .to_string()
            .contains("exactly 2 reorderable active plan ids")
    );

    let running_error = database
        .reorder_active_plans(&["plan-running".to_string(), "plan-ready-1".to_string()])
        .expect_err("running plan should fail");
    assert!(running_error.to_string().contains("not reorderable"));
}

fn create_minimal_plan(database: &mut WorkspaceDatabase, id: &str, status: &str) {
    let phase_id = format!("{id}-phase");
    let step_id = format!("{id}-step");
    database
        .create_plan(NewPlan {
            id,
            title: id,
            overview: "Minimal plan for ordering tests.",
            status,
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: phase_id.as_str(),
                title: "Phase one",
                summary: "Single phase.",
                steps: vec![NewPlanStep {
                    id: step_id.as_str(),
                    title: "Step one",
                    detail: "Single step.",
                    acceptance: vec!["done".to_string()],
                }],
            }],
        })
        .expect("create minimal plan");
}

#[test]
fn update_plan_cannot_bypass_execution_state_machine() {
    for phase_status in ["pending", "running", "failed", "cancelled"] {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database = WorkspaceDatabase::open_or_create_ungated(workspace.path())
            .expect("workspace database");
        let plan_id = format!("plan-update-guard-{phase_status}");
        create_minimal_plan(&mut database, &plan_id, "ready");
        let connection = Connection::open(database.database_path()).expect("open fixture database");
        connection
            .execute(
                "UPDATE plan_phases SET status = ?2 WHERE plan_id = ?1",
                params![plan_id, phase_status],
            )
            .expect("set phase fixture status");

        let error = database
            .update_plan(
                &plan_id,
                PlanPatch {
                    title: None,
                    overview: None,
                    status: Some("implemented"),
                    error_message: None,
                },
            )
            .expect_err("generic update must not set implemented");
        assert!(
            error.to_string().contains("cannot be changed"),
            "unexpected error for {phase_status}: {error}"
        );
        assert_eq!(
            database
                .plan(&plan_id)
                .expect("plan lookup")
                .expect("plan")
                .status,
            "ready"
        );
    }
}

#[test]
fn update_plan_only_edits_metadata_and_normal_state_machine_still_completes() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    create_minimal_plan(&mut database, "plan-update-metadata", "ready");

    let updated = database
        .update_plan(
            "plan-update-metadata",
            PlanPatch {
                title: Some("Updated title"),
                overview: Some("Updated overview"),
                status: Some("ready"),
                error_message: Some(Some("visible note")),
            },
        )
        .expect("metadata update");
    assert_eq!(updated.status, "ready");
    assert_eq!(updated.title, "Updated title");
    assert_eq!(updated.error_message.as_deref(), Some("visible note"));

    database
        .transition_plan("plan-update-metadata", "start")
        .expect("start plan");
    let implemented = database
        .update_plan_step(
            "plan-update-metadata",
            "plan-update-metadata-step",
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some("completed"),
            },
        )
        .expect("complete through state machine");
    assert_eq!(implemented.status, "implemented");
    let completed = database
        .transition_plan("plan-update-metadata", "mark_complete")
        .expect("mark complete");
    assert_eq!(completed.status, "completed");
}

#[test]
fn mark_plan_invalid_is_narrow_and_rejects_active_or_terminal_plans() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    create_minimal_plan(&mut database, "plan-invalid-reconcile", "ready");
    let failed = database
        .mark_plan_invalid("plan-invalid-reconcile", "invalid scheduler candidate")
        .expect("reconcile invalid plan");
    assert_eq!(failed.status, "failed");
    assert_eq!(
        failed.error_message.as_deref(),
        Some("invalid scheduler candidate")
    );

    create_minimal_plan(&mut database, "plan-invalid-active", "ready");
    database
        .transition_plan("plan-invalid-active", "start")
        .expect("start active plan");
    database
        .begin_plan_phase_attempt(
            "plan-invalid-active",
            "plan-invalid-active-phase",
            PlanPhaseAttemptTrigger::Initial,
            None,
            None,
            None,
        )
        .expect("begin active attempt");
    let active_error = database
        .mark_plan_invalid("plan-invalid-active", "must not bypass active attempt")
        .expect_err("active attempt must block invalid reconciliation");
    assert!(active_error.to_string().contains("attempt is active"));

    let connection = Connection::open(database.database_path()).expect("open fixture database");
    connection
        .execute(
            "UPDATE plans SET status = 'implemented' WHERE id = 'plan-invalid-reconcile'",
            [],
        )
        .expect("create historical terminal fixture");
    let terminal_error = database
        .mark_plan_invalid("plan-invalid-reconcile", "must not rewrite terminal plan")
        .expect_err("terminal plan must be preserved");
    assert!(terminal_error.to_string().contains("while implemented"));
}

#[test]
fn create_plan_reports_duplicate_step_id_before_sqlite_constraint() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-duplicate-step-source",
            title: "Source plan",
            overview: "Existing plan with a step id.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-duplicate-step-source-phase",
                title: "Phase one",
                summary: "Existing phase.",
                steps: vec![NewPlanStep {
                    id: "plan-step-duplicate",
                    title: "Existing step",
                    detail: "Create the existing step.",
                    acceptance: vec!["existing step is stored".to_string()],
                }],
            }],
        })
        .expect("create source plan");

    let error = database
        .create_plan(NewPlan {
            id: "plan-duplicate-step-new",
            title: "New plan",
            overview: "Attempts to reuse the step id.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-duplicate-step-new-phase",
                title: "Phase one",
                summary: "New phase.",
                steps: vec![NewPlanStep {
                    id: "plan-step-duplicate",
                    title: "New step",
                    detail: "Reuse the existing step id.",
                    acceptance: vec!["new step is rejected".to_string()],
                }],
            }],
        })
        .expect_err("duplicate step id rejected");

    assert!(matches!(
        error,
        WorkspaceDatabaseError::InvalidPlan { ref message }
            if message == "plan step id already exists: plan-step-duplicate"
    ));
    assert!(
        database
            .plan("plan-duplicate-step-new")
            .expect("plan lookup")
            .is_none()
    );
}

#[test]
fn create_plan_reports_duplicate_step_id_within_same_request() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    let error = database
        .create_plan(NewPlan {
            id: "plan-duplicate-step-same-request",
            title: "Duplicate step request",
            overview: "Attempts to reuse a step id in one plan.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-duplicate-step-same-request-phase",
                title: "Phase one",
                summary: "Duplicate step phase.",
                steps: vec![
                    NewPlanStep {
                        id: "plan-step-duplicate-in-request",
                        title: "First step",
                        detail: "Create the first step.",
                        acceptance: vec!["first step is seen".to_string()],
                    },
                    NewPlanStep {
                        id: "plan-step-duplicate-in-request",
                        title: "Second step",
                        detail: "Reuse the first step id.",
                        acceptance: vec!["second step is rejected".to_string()],
                    },
                ],
            }],
        })
        .expect_err("duplicate step id rejected");

    assert!(matches!(
        error,
        WorkspaceDatabaseError::InvalidPlan { ref message }
            if message == "plan step id already exists: plan-step-duplicate-in-request"
    ));
    assert!(
        database
            .plan("plan-duplicate-step-same-request")
            .expect("plan lookup")
            .is_none()
    );
}

fn create_auto_run_test_plan(database: &mut WorkspaceDatabase, id: &str, status: &str) {
    database
        .create_plan(NewPlan {
            id,
            title: id,
            overview: "auto run plan",
            status,
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: &format!("{id}-phase"),
                title: "Phase",
                summary: "Run it",
                steps: vec![NewPlanStep {
                    id: &format!("{id}-step"),
                    title: "Step",
                    detail: "Do it",
                    acceptance: vec!["done".to_string()],
                }],
            }],
        })
        .expect("create auto-run test plan");
}

#[test]
fn plan_auto_run_paused_plan_is_a_scheduling_gate() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    let mut database = database;

    create_auto_run_test_plan(&mut database, "paused", "paused");
    create_auto_run_test_plan(&mut database, "ready", "ready");

    assert_eq!(
        database
            .next_plan_auto_run_candidate()
            .expect("candidate query"),
        foco_store::workspace::PlanAutoRunSelection::Paused {
            plan_id: "paused".to_string(),
            phase_id: Some("paused-phase".to_string()),
        }
    );
}

#[test]
fn plan_auto_run_distinguishes_start_candidates_from_user_paused_plans() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    let cases = [
        (
            "draft",
            foco_store::workspace::PlanAutoRunSelection::WaitingForReady {
                plan_id: "candidate-0".to_string(),
            },
        ),
        (
            "ready",
            foco_store::workspace::PlanAutoRunSelection::Candidate(
                foco_store::workspace::PlanAutoRunCandidateRecord {
                    plan_id: "candidate-1".to_string(),
                    action: "start".to_string(),
                },
            ),
        ),
        (
            "failed",
            foco_store::workspace::PlanAutoRunSelection::Candidate(
                foco_store::workspace::PlanAutoRunCandidateRecord {
                    plan_id: "candidate-2".to_string(),
                    action: "start".to_string(),
                },
            ),
        ),
        (
            "paused",
            foco_store::workspace::PlanAutoRunSelection::Paused {
                plan_id: "candidate-3".to_string(),
                phase_id: Some("candidate-3-phase".to_string()),
            },
        ),
    ];
    for (index, (status, expected)) in cases.into_iter().enumerate() {
        let id = format!("candidate-{index}");
        create_auto_run_test_plan(&mut database, &id, status);
        assert_eq!(
            database
                .next_plan_auto_run_candidate()
                .expect("candidate selection"),
            expected
        );
        database.delete_plan(&id).expect("delete candidate plan");
    }
}

#[test]
fn plan_auto_run_cancelled_phase_blocks_later_plan_and_is_not_busy() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    create_auto_run_test_plan(&mut database, "blocked", "ready");
    create_auto_run_test_plan(&mut database, "later", "ready");
    database
        .transition_plan("blocked", "start")
        .expect("start blocked plan");
    database
        .cancel_plan_phase_by_id("blocked", "blocked-phase", "user cancelled phase")
        .expect("cancel blocked phase");
    database
        .set_plan_auto_run_enabled(true)
        .expect("re-enable auto-run");

    assert_eq!(
        database
            .next_plan_auto_run_candidate()
            .expect("candidate selection"),
        foco_store::workspace::PlanAutoRunSelection::BlockedByCancelledPhase {
            plan_id: "blocked".to_string(),
            phase_id: "blocked-phase".to_string(),
        }
    );
    let state = database.plan_auto_run_state().expect("auto-run state");
    assert!(state.desired_enabled);
    assert!(!state.enabled);
    assert!(!state.busy);
    assert_eq!(state.blocked_reason.as_deref(), Some("cancelled_phase"));
    assert_eq!(state.blocked_plan_id.as_deref(), Some("blocked"));
    assert_eq!(state.blocked_phase_id.as_deref(), Some("blocked-phase"));
    assert!(!database.disable_plan_auto_run_if_idle().expect("not idle"));
}

#[test]
fn plan_auto_run_idle_disables_desired_preference() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    create_auto_run_test_plan(&mut database, "completed", "completed");
    database
        .set_plan_auto_run_enabled(true)
        .expect("enable auto-run");

    assert!(
        database
            .disable_plan_auto_run_if_idle()
            .expect("disable idle auto-run")
    );
    let state = database.plan_auto_run_state().expect("state");
    assert!(!state.desired_enabled);
    assert!(!state.enabled);
    assert!(!state.busy);
}

#[test]
fn plan_auto_run_marks_running_plan_busy_without_candidate() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    create_auto_run_test_plan(&mut database, "running-plan", "ready");
    database
        .transition_plan("running-plan", "start")
        .expect("start plan");
    database
        .set_plan_auto_run_enabled(true)
        .expect("enable auto-run");

    assert!(database.plan_auto_run_has_in_flight().expect("in flight"));
    assert_eq!(
        database
            .next_plan_auto_run_candidate()
            .expect("candidate query"),
        foco_store::workspace::PlanAutoRunSelection::Running {
            plan_id: "running-plan".to_string(),
            phase_id: Some("running-plan-phase".to_string()),
        }
    );
    assert!(database.plan_auto_run_state().expect("state").busy);
}

#[test]
fn plan_auto_run_legacy_enabled_metadata_migrates_to_desired_preference() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    {
        let mut database = WorkspaceDatabase::open_or_create_ungated(workspace.path())
            .expect("workspace database");
        database
            .set_workspace_metadata("plan_auto_run_enabled", "true")
            .expect("set legacy preference");
    }
    let database_path = workspace_database_path(workspace.path());
    let connection = Connection::open(&database_path).expect("legacy database connection");
    connection
        .execute(
            "DELETE FROM workspace_metadata WHERE key = 'plan_auto_run_desired_enabled'",
            [],
        )
        .expect("remove desired metadata before migration");
    connection
        .pragma_update(None, "user_version", 34)
        .expect("rewind schema version");
    drop(connection);

    let database = WorkspaceDatabase::open_or_create_ungated(workspace.path())
        .expect("migrated workspace database");
    let state = database.plan_auto_run_state().expect("migrated state");
    assert!(state.desired_enabled);
}

#[test]
fn plan_auto_run_desired_and_block_survive_reopen_and_retry_clears_block() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    {
        let mut database = WorkspaceDatabase::open_or_create_ungated(workspace.path())
            .expect("workspace database");
        create_auto_run_test_plan(&mut database, "blocked", "ready");
        database
            .set_plan_auto_run_enabled(true)
            .expect("enable auto-run");
        database
            .transition_plan("blocked", "start")
            .expect("start blocked plan");
        database
            .cancel_plan_phase_by_id("blocked", "blocked-phase", "user cancelled phase")
            .expect("cancel phase");
        let state = database.plan_auto_run_state().expect("blocked state");
        assert!(state.desired_enabled);
        assert!(!state.enabled);
        assert_eq!(state.blocked_reason.as_deref(), Some("cancelled_phase"));
    }

    let mut reopened =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopened database");
    let state = reopened.plan_auto_run_state().expect("reopened state");
    assert!(state.desired_enabled);
    assert_eq!(state.blocked_reason.as_deref(), Some("cancelled_phase"));

    reopened
        .begin_plan_phase_attempt(
            "blocked",
            "blocked-phase",
            foco_store::workspace::PlanPhaseAttemptTrigger::Retry,
            None,
            None,
            None,
        )
        .expect("begin retry");
    let state = reopened.plan_auto_run_state().expect("retry state");
    assert!(state.desired_enabled);
    assert!(state.enabled);
    assert!(state.busy);
    assert!(state.blocked_reason.is_none());
}

#[test]
fn disabling_auto_run_while_blocked_prevents_retry_resume() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    create_auto_run_test_plan(&mut database, "blocked", "ready");
    database
        .set_plan_auto_run_enabled(true)
        .expect("enable auto-run");
    database
        .transition_plan("blocked", "start")
        .expect("start blocked plan");
    database
        .cancel_plan_phase_by_id("blocked", "blocked-phase", "user cancelled phase")
        .expect("cancel phase");
    database
        .set_plan_auto_run_enabled(false)
        .expect("disable preference");
    database
        .begin_plan_phase_attempt(
            "blocked",
            "blocked-phase",
            foco_store::workspace::PlanPhaseAttemptTrigger::Retry,
            None,
            None,
            None,
        )
        .expect("begin retry");
    let state = database.plan_auto_run_state().expect("retry state");
    assert!(!state.desired_enabled);
    assert!(!state.enabled);
    assert!(!state.busy);
    assert!(state.blocked_reason.is_none());
}

#[test]
fn plan_phase_run_completion_advances_until_pause() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-runner",
            title: "Plan runner",
            overview: "Run phases through Agent tasks.",
            status: "ready",
            source_chat_id: None,
            phases: vec![
                NewPlanPhase {
                    id: "plan-runner-phase-1",
                    title: "Phase one",
                    summary: "First implementation phase.",
                    steps: vec![NewPlanStep {
                        id: "plan-runner-step-1",
                        title: "Do first",
                        detail: "Complete first work.",
                        acceptance: vec!["first done".to_string()],
                    }],
                },
                NewPlanPhase {
                    id: "plan-runner-phase-2",
                    title: "Phase two",
                    summary: "Second implementation phase.",
                    steps: vec![NewPlanStep {
                        id: "plan-runner-step-2",
                        title: "Do second",
                        detail: "Complete second work.",
                        acceptance: vec!["second done".to_string()],
                    }],
                },
                NewPlanPhase {
                    id: "plan-runner-phase-3",
                    title: "Phase three",
                    summary: "Third implementation phase.",
                    steps: vec![NewPlanStep {
                        id: "plan-runner-step-3",
                        title: "Do third",
                        detail: "Complete third work.",
                        acceptance: vec!["third done".to_string()],
                    }],
                },
            ],
        })
        .expect("create plan");

    let first_running = database
        .transition_plan("plan-runner", "start")
        .expect("start first phase");
    assert_eq!(first_running.status, "running");
    assert_eq!(
        first_running.active_phase_id.as_deref(),
        Some("plan-runner-phase-1")
    );

    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-plan-runner-1", "plan-runner-1");
    let first_task_id = AgentTaskId::new("agent-task-plan-runner-1").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &first_task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue first task");
    let attached = database
        .attach_plan_phase_run(
            "plan-runner",
            "plan-runner-phase-1",
            "chat-plan-runner-1",
            &team_id,
            &first_task_id,
        )
        .expect("attach first phase");
    assert_eq!(
        attached.phases[0].agent_task_id.as_deref(),
        Some("agent-task-plan-runner-1")
    );
    complete_test_agent_task(
        &mut database,
        &team_id,
        &first_task_id,
        "agent-attempt-plan-runner-1",
    );

    let after_first = database
        .complete_plan_phase_run(&first_task_id, Some("commit-one"))
        .expect("complete first phase")
        .expect("plan after first phase");
    assert_eq!(after_first.status, "ready");
    assert_eq!(after_first.phases[0].status, "completed");
    assert_eq!(
        after_first.phases[0].commit_id.as_deref(),
        Some("commit-one")
    );
    assert_eq!(after_first.phases[0].steps[0].status, "completed");
    assert_eq!(after_first.phases[1].status, "pending");

    let second_running = database
        .transition_plan("plan-runner", "resume")
        .expect("start second phase");
    assert_eq!(
        second_running.active_phase_id.as_deref(),
        Some("plan-runner-phase-2")
    );
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-plan-runner-2", "plan-runner-2");
    let second_task_id = AgentTaskId::new("agent-task-plan-runner-2").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &second_task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue second task");
    database
        .attach_plan_phase_run(
            "plan-runner",
            "plan-runner-phase-2",
            "chat-plan-runner-2",
            &team_id,
            &second_task_id,
        )
        .expect("attach second phase");
    complete_test_agent_task(
        &mut database,
        &team_id,
        &second_task_id,
        "agent-attempt-plan-runner-2",
    );
    let paused = database
        .transition_plan("plan-runner", "pause")
        .expect("pause plan");
    assert_eq!(paused.status, "paused");

    let after_second = database
        .complete_plan_phase_run(&second_task_id, Some("commit-two"))
        .expect("complete second phase")
        .expect("plan after second phase");
    assert_eq!(after_second.status, "paused");
    assert!(after_second.active_phase_id.is_none());
    assert_eq!(after_second.phases[1].status, "completed");
    assert_eq!(after_second.phases[2].status, "pending");
}

#[test]
fn resume_after_pause_keeps_running_phase_and_execution_identity() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-pause-resume-active",
            title: "Pause resume active phase",
            overview: "Resume must unpause without re-starting a running phase.",
            status: "ready",
            source_chat_id: None,
            phases: vec![
                NewPlanPhase {
                    id: "plan-pause-resume-active-phase-1",
                    title: "Phase one",
                    summary: "Keep executing after pause.",
                    steps: vec![NewPlanStep {
                        id: "plan-pause-resume-active-step-1",
                        title: "Do work",
                        detail: "Stay bound to the original task.",
                        acceptance: vec!["still running".to_string()],
                    }],
                },
                NewPlanPhase {
                    id: "plan-pause-resume-active-phase-2",
                    title: "Phase two",
                    summary: "Must not start on resume of phase one.",
                    steps: vec![NewPlanStep {
                        id: "plan-pause-resume-active-step-2",
                        title: "Later work",
                        detail: "Remain pending.",
                        acceptance: vec!["not started".to_string()],
                    }],
                },
            ],
        })
        .expect("create plan");

    database
        .transition_plan("plan-pause-resume-active", "start")
        .expect("start phase");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-pause-resume-active",
            "plan-pause-resume-active-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider-test"),
            Some("model-test"),
            None,
        )
        .expect("begin attempt");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-pause-resume-active",
        "pause-resume-active",
    );
    let task_id = AgentTaskId::new("agent-task-pause-resume-active").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    let attached = database
        .attach_plan_phase_attempt_run(&attempt.id, "chat-pause-resume-active", &team_id, &task_id)
        .expect("attach run");
    assert_eq!(attached.status, "running");
    assert_eq!(
        attached.active_phase_id.as_deref(),
        Some("plan-pause-resume-active-phase-1")
    );
    let phase_before = &attached.phases[0];
    assert_eq!(phase_before.status, "running");
    assert_eq!(
        phase_before.agent_task_id.as_deref(),
        Some("agent-task-pause-resume-active")
    );
    assert_eq!(
        phase_before.implementation_chat_id.as_deref(),
        Some("chat-pause-resume-active")
    );
    assert_eq!(phase_before.attempts.len(), 1);
    assert_eq!(phase_before.attempts[0].id, attempt.id);
    assert_eq!(phase_before.attempts[0].status, "running");

    let paused = database
        .transition_plan("plan-pause-resume-active", "pause")
        .expect("pause plan");
    assert_eq!(paused.status, "paused");
    assert_eq!(
        paused.active_phase_id.as_deref(),
        Some("plan-pause-resume-active-phase-1")
    );
    assert_eq!(paused.phases[0].status, "running");
    assert_eq!(
        paused.phases[0].agent_task_id.as_deref(),
        Some("agent-task-pause-resume-active")
    );
    assert_eq!(paused.phases[0].attempts[0].id, attempt.id);
    assert_eq!(paused.phases[0].attempts[0].status, "running");
    assert_eq!(paused.phases[1].status, "pending");

    let resumed = database
        .transition_plan("plan-pause-resume-active", "resume")
        .expect("resume must unpause active phase without restart");
    assert_eq!(resumed.status, "running");
    assert_eq!(
        resumed.active_phase_id.as_deref(),
        Some("plan-pause-resume-active-phase-1")
    );
    assert!(resumed.pause_requested_at.is_none());
    assert_eq!(resumed.phases[0].status, "running");
    assert_eq!(
        resumed.phases[0].agent_task_id.as_deref(),
        Some("agent-task-pause-resume-active")
    );
    assert_eq!(
        resumed.phases[0].implementation_chat_id.as_deref(),
        Some("chat-pause-resume-active")
    );
    assert_eq!(
        resumed.phases[0].agent_team_id.as_deref(),
        Some(team_id.as_str())
    );
    assert_eq!(resumed.phases[0].attempts.len(), 1);
    assert_eq!(resumed.phases[0].attempts[0].id, attempt.id);
    assert_eq!(resumed.phases[0].attempts[0].status, "running");
    assert_eq!(
        resumed.phases[0].attempts[0].agent_task_id.as_deref(),
        Some("agent-task-pause-resume-active")
    );
    assert_eq!(resumed.phases[1].status, "pending");
    assert!(resumed.phases[1].agent_task_id.is_none());
}

#[test]
fn resume_after_pause_keeps_waiting_agent_task_without_restart() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-pause-resume-waiting",
            title: "Pause resume waiting task",
            overview: "Waiting agent task still counts as active execution.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-pause-resume-waiting-phase-1",
                title: "Phase one",
                summary: "Coordinator waiting on subagent.",
                steps: vec![NewPlanStep {
                    id: "plan-pause-resume-waiting-step-1",
                    title: "Wait",
                    detail: "Keep task binding.",
                    acceptance: vec!["waiting preserved".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-pause-resume-waiting", "start")
        .expect("start");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-pause-resume-waiting",
            "plan-pause-resume-waiting-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider-test"),
            Some("model-test"),
            None,
        )
        .expect("begin attempt");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-pause-resume-waiting",
        "pause-resume-waiting",
    );
    let task_id = AgentTaskId::new("agent-task-pause-resume-waiting").expect("task id");
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
    database
        .attach_plan_phase_attempt_run(&attempt.id, "chat-pause-resume-waiting", &team_id, &task_id)
        .expect("attach");
    let worker_id =
        create_test_agent_worker(&mut database, &team_id, "pause-resume-waiting-worker");
    let child_task = AgentTaskId::new("agent-task-pause-resume-waiting-child").expect("child");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &child_task,
            team_id: &team_id,
            owner_instance_id: &worker_id,
            origin_instance_id: Some(&instance_id),
            parent_task_id: Some(&task_id),
            input_json: "{}",
        })
        .expect("enqueue child");
    let parent_attempt =
        AgentAttemptId::new("agent-attempt-pause-resume-waiting-parent").expect("attempt");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &parent_attempt)
        .expect("claim parent")
        .expect("claimed parent");
    database
        .insert_agent_task_dependency(NewAgentTaskDependency {
            team_id: &team_id,
            waiting_task_id: &task_id,
            dependency_task_id: &child_task,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-pause-resume-waiting"),
            deadline_at: None,
        })
        .expect("dependency");
    assert!(
        database
            .suspend_running_agent_task_with_wait_dependencies(&team_id, &task_id)
            .expect("suspend to waiting")
    );

    database
        .transition_plan("plan-pause-resume-waiting", "pause")
        .expect("pause");
    let resumed = database
        .transition_plan("plan-pause-resume-waiting", "resume")
        .expect("resume waiting phase");
    assert_eq!(resumed.status, "running");
    assert_eq!(
        resumed.active_phase_id.as_deref(),
        Some("plan-pause-resume-waiting-phase-1")
    );
    assert_eq!(
        resumed.phases[0].agent_task_id.as_deref(),
        Some("agent-task-pause-resume-waiting")
    );
    assert_eq!(resumed.phases[0].attempts[0].id, attempt.id);
    let task = database
        .agent_task(&task_id)
        .expect("task")
        .expect("task row");
    assert_eq!(task.status, AgentTaskStatus::Waiting);
}

#[test]
fn resume_after_pause_keeps_queued_attempt_without_agent_task() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-pause-resume-queued-attempt",
            title: "Pause resume queued attempt",
            overview: "Active attempt without attached task is still resumable.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-pause-resume-queued-attempt-phase-1",
                title: "Phase one",
                summary: "Attempt queued before dispatch attaches.",
                steps: vec![NewPlanStep {
                    id: "plan-pause-resume-queued-attempt-step-1",
                    title: "Dispatch",
                    detail: "Keep attempt id.",
                    acceptance: vec!["attempt preserved".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-pause-resume-queued-attempt", "start")
        .expect("start");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-pause-resume-queued-attempt",
            "plan-pause-resume-queued-attempt-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider-test"),
            Some("model-test"),
            None,
        )
        .expect("begin attempt");
    assert_eq!(attempt.status, "queued");
    assert!(attempt.agent_task_id.is_none());

    database
        .transition_plan("plan-pause-resume-queued-attempt", "pause")
        .expect("pause");
    let resumed = database
        .transition_plan("plan-pause-resume-queued-attempt", "resume")
        .expect("resume queued attempt phase");
    assert_eq!(resumed.status, "running");
    assert_eq!(
        resumed.active_phase_id.as_deref(),
        Some("plan-pause-resume-queued-attempt-phase-1")
    );
    assert_eq!(resumed.phases[0].attempts.len(), 1);
    assert_eq!(resumed.phases[0].attempts[0].id, attempt.id);
    assert_eq!(resumed.phases[0].attempts[0].status, "queued");
    assert!(resumed.phases[0].agent_task_id.is_none());
}

#[test]
fn resume_after_pause_preserves_running_phase_before_execution_identity_is_attached() {
    // Store marks the phase running before runtime attaches chat/team/task identity.
    // Resume must only lift the plan-level scheduling gate during that interval.
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-pause-resume-surface",
            title: "Surface running pause resume",
            overview: "Fake start leaves no agent identity.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-pause-resume-surface-phase-1",
                title: "Phase one",
                summary: "Only Store transition_plan start.",
                steps: vec![NewPlanStep {
                    id: "plan-pause-resume-surface-step-1",
                    title: "Start",
                    detail: "No dispatch.",
                    acceptance: vec!["no identity".to_string()],
                }],
            }],
        })
        .expect("create plan");

    let started = database
        .transition_plan("plan-pause-resume-surface", "start")
        .expect("store-only start");
    assert_eq!(started.status, "running");
    assert_eq!(
        started.active_phase_id.as_deref(),
        Some("plan-pause-resume-surface-phase-1")
    );
    assert_eq!(started.phases[0].status, "running");
    assert!(started.phases[0].agent_task_id.is_none());
    assert!(started.phases[0].implementation_chat_id.is_none());
    assert!(started.phases[0].agent_team_id.is_none());
    assert!(started.phases[0].attempts.is_empty());

    database
        .transition_plan("plan-pause-resume-surface", "pause")
        .expect("pause");
    let resumed = database
        .transition_plan("plan-pause-resume-surface", "resume")
        .expect("resume keeps running phase for runtime dispatch");
    assert_eq!(resumed.status, "running");
    assert_eq!(
        resumed.active_phase_id.as_deref(),
        Some("plan-pause-resume-surface-phase-1")
    );
    assert!(resumed.pause_requested_at.is_none());
    assert_eq!(resumed.phases[0].status, "running");
    assert!(resumed.phases[0].agent_task_id.is_none());
    assert!(resumed.phases[0].implementation_chat_id.is_none());
    assert!(resumed.phases[0].agent_team_id.is_none());
    assert!(resumed.phases[0].attempts.is_empty());
}

#[test]
fn store_only_start_preserves_identity_free_running_phase_across_resume() {
    // Store transitions deliberately do not create chat/team/task/attempt identity.
    // Runtime attaches that identity after its own dispatch step, so resume must keep
    // the running phase intact rather than restarting it.
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-store-only-start",
            title: "Store only start",
            overview: "Sidecar-style transition_plan start.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-store-only-start-phase-1",
                title: "Phase one",
                summary: "Needs runtime dispatch after Store start.",
                steps: vec![NewPlanStep {
                    id: "plan-store-only-start-step-1",
                    title: "Implement",
                    detail: "Requires agent task.",
                    acceptance: vec!["has identity after real start".to_string()],
                }],
            }],
        })
        .expect("create plan");

    let plan = database
        .transition_plan("plan-store-only-start", "start")
        .expect("start");
    assert_eq!(plan.status, "running");
    assert_eq!(plan.phases[0].status, "running");
    assert!(
        plan.phases[0].agent_task_id.is_none(),
        "Store start must not invent agent_task_id"
    );
    assert!(
        plan.phases[0].implementation_chat_id.is_none(),
        "Store start must not invent implementation_chat_id"
    );
    assert!(
        plan.phases[0].agent_team_id.is_none(),
        "Store start must not invent agent_team_id"
    );
    assert!(
        plan.phases[0].attempts.is_empty(),
        "Store start must not create plan_phase_attempts"
    );

    let agent_tasks: i64 = {
        let connection = Connection::open(database.database_path()).expect("open database");
        connection
            .query_row("SELECT COUNT(*) FROM agent_tasks", [], |row| row.get(0))
            .expect("count agent_tasks")
    };
    let agent_teams: i64 = {
        let connection = Connection::open(database.database_path()).expect("open database");
        connection
            .query_row("SELECT COUNT(*) FROM agent_teams", [], |row| row.get(0))
            .expect("count agent_teams")
    };
    let attempts: i64 = {
        let connection = Connection::open(database.database_path()).expect("open database");
        connection
            .query_row("SELECT COUNT(*) FROM plan_phase_attempts", [], |row| {
                row.get(0)
            })
            .expect("count attempts")
    };
    assert_eq!(agent_tasks, 0);
    assert_eq!(agent_teams, 0);
    assert_eq!(attempts, 0);

    database
        .transition_plan("plan-store-only-start", "pause")
        .expect("pause surface-only plan");
    let resumed = database
        .transition_plan("plan-store-only-start", "resume")
        .expect("resume keeps the identity-free running phase");
    assert_eq!(resumed.status, "running");
    assert_eq!(
        resumed.active_phase_id.as_deref(),
        Some("plan-store-only-start-phase-1")
    );
    assert!(resumed.pause_requested_at.is_none());
    assert_eq!(resumed.phases[0].status, "running");
    assert!(resumed.phases[0].agent_task_id.is_none());
    assert!(resumed.phases[0].implementation_chat_id.is_none());
    assert!(resumed.phases[0].agent_team_id.is_none());
    assert!(resumed.phases[0].attempts.is_empty());
}

#[test]
fn plan_start_pause_resume_state_contract_matrix() {
    // Documents expected Store outcomes for start/pause/resume across the three
    // execution postures used by Project Spec (no active execution / active attempt /
    // active agent task). Runtime dispatch is out of scope for Store.
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    // --- No active execution: start marks earliest incomplete running ---
    database
        .create_plan(NewPlan {
            id: "plan-contract-idle",
            title: "Idle start",
            overview: "No active execution.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-contract-idle-phase-1",
                title: "P1",
                summary: "s",
                steps: vec![NewPlanStep {
                    id: "plan-contract-idle-step-1",
                    title: "s",
                    detail: "d",
                    acceptance: vec!["a".to_string()],
                }],
            }],
        })
        .expect("create idle");
    let started = database
        .transition_plan("plan-contract-idle", "start")
        .expect("start idle");
    assert_eq!(started.status, "running");
    assert_eq!(started.phases[0].status, "running");
    let start_again = database
        .transition_plan("plan-contract-idle", "start")
        .expect_err("start while phase running is InvalidPlan");
    assert!(matches!(
        start_again,
        WorkspaceDatabaseError::InvalidPlan { .. }
    ));
    assert!(start_again.to_string().contains("is already running"));
    let paused_idle = database
        .transition_plan("plan-contract-idle", "pause")
        .expect("pause surface running");
    assert_eq!(paused_idle.status, "paused");
    assert_eq!(paused_idle.phases[0].status, "running");
    let resumed_idle = database
        .transition_plan("plan-contract-idle", "resume")
        .expect("resume keeps the running phase for runtime dispatch");
    assert_eq!(resumed_idle.status, "running");
    assert_eq!(resumed_idle.phases[0].status, "running");
    assert_eq!(
        resumed_idle.active_phase_id.as_deref(),
        Some("plan-contract-idle-phase-1")
    );

    // --- Active queued attempt ---
    database
        .create_plan(NewPlan {
            id: "plan-contract-attempt",
            title: "Attempt active",
            overview: "Queued attempt.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-contract-attempt-phase-1",
                title: "P1",
                summary: "s",
                steps: vec![NewPlanStep {
                    id: "plan-contract-attempt-step-1",
                    title: "s",
                    detail: "d",
                    acceptance: vec!["a".to_string()],
                }],
            }],
        })
        .expect("create attempt plan");
    database
        .transition_plan("plan-contract-attempt", "start")
        .expect("start");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-contract-attempt",
            "plan-contract-attempt-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider-test"),
            Some("model-test"),
            None,
        )
        .expect("attempt");
    database
        .transition_plan("plan-contract-attempt", "pause")
        .expect("pause attempt plan");
    let resume_attempt = database
        .transition_plan("plan-contract-attempt", "resume")
        .expect("resume attempt plan");
    assert_eq!(resume_attempt.status, "running");
    assert_eq!(resume_attempt.phases[0].attempts[0].id, attempt.id);
    assert_eq!(resume_attempt.phases[0].attempts[0].status, "queued");
    let start_while_attempt = database
        .transition_plan("plan-contract-attempt", "start")
        .expect_err("start while attempt active");
    assert!(matches!(
        start_while_attempt,
        WorkspaceDatabaseError::InvalidPlan { .. }
    ));

    // --- Active agent task (running) ---
    database
        .create_plan(NewPlan {
            id: "plan-contract-task",
            title: "Task active",
            overview: "Running agent task.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-contract-task-phase-1",
                title: "P1",
                summary: "s",
                steps: vec![NewPlanStep {
                    id: "plan-contract-task-step-1",
                    title: "s",
                    detail: "d",
                    acceptance: vec!["a".to_string()],
                }],
            }],
        })
        .expect("create task plan");
    database
        .transition_plan("plan-contract-task", "start")
        .expect("start");
    let task_attempt = database
        .begin_plan_phase_attempt(
            "plan-contract-task",
            "plan-contract-task-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider-test"),
            Some("model-test"),
            None,
        )
        .expect("task attempt");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-contract-task", "contract-task");
    let task_id = AgentTaskId::new("agent-task-contract-task").expect("task id");
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
    database
        .attach_plan_phase_attempt_run(&task_attempt.id, "chat-contract-task", &team_id, &task_id)
        .expect("attach");
    database
        .transition_plan("plan-contract-task", "pause")
        .expect("pause task plan");
    let resume_task = database
        .transition_plan("plan-contract-task", "resume")
        .expect("resume task plan");
    assert_eq!(resume_task.status, "running");
    assert_eq!(
        resume_task.phases[0].agent_task_id.as_deref(),
        Some("agent-task-contract-task")
    );
    assert_eq!(resume_task.phases[0].attempts[0].id, task_attempt.id);

    // Illegal terminal resume remains structured InvalidPlan.
    database
        .create_plan(NewPlan {
            id: "plan-contract-cancelled",
            title: "Cancelled",
            overview: "Cannot resume.",
            status: "cancelled",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-contract-cancelled-phase-1",
                title: "P1",
                summary: "s",
                steps: vec![NewPlanStep {
                    id: "plan-contract-cancelled-step-1",
                    title: "s",
                    detail: "d",
                    acceptance: vec!["a".to_string()],
                }],
            }],
        })
        .expect("create cancelled");
    let resume_cancelled = database
        .transition_plan("plan-contract-cancelled", "resume")
        .expect_err("cancelled plan cannot resume");
    assert!(matches!(
        resume_cancelled,
        WorkspaceDatabaseError::InvalidPlan { .. }
    ));
}

#[test]
fn resume_from_paused_plan_without_active_phase_starts_earliest_incomplete_phase() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-resume-no-active-phase",
            title: "Resume without active phase",
            overview: "Resume starts only when no phase remains active.",
            status: "ready",
            source_chat_id: None,
            phases: vec![
                NewPlanPhase {
                    id: "plan-resume-no-active-phase-1",
                    title: "First",
                    summary: "Must start first.",
                    steps: vec![NewPlanStep {
                        id: "plan-resume-no-active-phase-step-1",
                        title: "First step",
                        detail: "Start on resume.",
                        acceptance: vec!["first phase starts".to_string()],
                    }],
                },
                NewPlanPhase {
                    id: "plan-resume-no-active-phase-2",
                    title: "Second",
                    summary: "Must stay pending.",
                    steps: vec![NewPlanStep {
                        id: "plan-resume-no-active-phase-step-2",
                        title: "Second step",
                        detail: "Do not skip ahead.",
                        acceptance: vec!["second phase remains pending".to_string()],
                    }],
                },
            ],
        })
        .expect("create plan");

    let paused = database
        .transition_plan("plan-resume-no-active-phase", "pause")
        .expect("pause ready plan");
    assert_eq!(paused.status, "paused");
    assert!(paused.active_phase_id.is_none());
    assert_eq!(paused.phases[0].status, "pending");

    let resumed = database
        .transition_plan("plan-resume-no-active-phase", "resume")
        .expect("resume starts earliest incomplete phase when no phase is active");
    assert_eq!(resumed.status, "running");
    assert_eq!(
        resumed.active_phase_id.as_deref(),
        Some("plan-resume-no-active-phase-1")
    );
    assert_eq!(resumed.phases[0].status, "running");
    assert_eq!(resumed.phases[1].status, "pending");
}

#[test]
fn phase_commit_does_not_mark_plan_shared_merged() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-shared-merge-marker",
            title: "Shared merge marker",
            overview: "Track shared merge separately from phase commits.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-shared-merge-marker-phase-1",
                title: "Phase one",
                summary: "Produces an isolated commit.",
                steps: vec![NewPlanStep {
                    id: "plan-shared-merge-marker-step-1",
                    title: "Do work",
                    detail: "Complete the phase.",
                    acceptance: vec!["phase committed".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-shared-merge-marker", "start")
        .expect("start phase");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-plan-shared-merge-marker",
        "plan-shared-merge-marker",
    );
    let task_id = AgentTaskId::new("agent-task-plan-shared-merge-marker").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_run(
            "plan-shared-merge-marker",
            "plan-shared-merge-marker-phase-1",
            "chat-plan-shared-merge-marker",
            &team_id,
            &task_id,
        )
        .expect("attach phase");
    complete_test_agent_task(
        &mut database,
        &team_id,
        &task_id,
        "agent-attempt-plan-shared-merge-marker",
    );

    let completed = database
        .complete_plan_phase_run(&task_id, Some("phase-commit"))
        .expect("complete phase")
        .expect("completed plan");
    assert_eq!(completed.status, "implemented");
    assert_eq!(
        completed.phases[0].commit_id.as_deref(),
        Some("phase-commit")
    );
    assert!(completed.shared_merge_commit_id.is_none());

    let merged = database
        .record_plan_shared_merge_commit("plan-shared-merge-marker", "shared-commit")
        .expect("record shared merge");
    assert_eq!(
        merged.shared_merge_commit_id.as_deref(),
        Some("shared-commit")
    );
}

#[test]
fn blocked_merge_completion_records_shared_commit_and_clears_errors() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-blocked-merge-complete",
            title: "Blocked merge complete",
            overview: "A later merge commit should clear the blocked state.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-blocked-merge-complete-phase-1",
                title: "Phase one",
                summary: "Produces an isolated commit.",
                steps: vec![NewPlanStep {
                    id: "plan-blocked-merge-complete-step-1",
                    title: "Do work",
                    detail: "Complete the phase.",
                    acceptance: vec!["phase committed".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-blocked-merge-complete", "start")
        .expect("start phase");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-plan-blocked-merge-complete",
        "plan-blocked-merge-complete",
    );
    let task_id = AgentTaskId::new("agent-task-plan-blocked-merge-complete").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_run(
            "plan-blocked-merge-complete",
            "plan-blocked-merge-complete-phase-1",
            "chat-plan-blocked-merge-complete",
            &team_id,
            &task_id,
        )
        .expect("attach phase");
    complete_test_agent_task(
        &mut database,
        &team_id,
        &task_id,
        "agent-attempt-plan-blocked-merge-complete",
    );

    database
        .set_plan_auto_run_enabled(true)
        .expect("enable auto-run preference");
    let completed = database
        .complete_plan_phase_run(&task_id, Some("phase-worktree-commit"))
        .expect("complete phase")
        .expect("completed plan");
    assert_eq!(completed.status, "implemented");
    assert!(completed.shared_merge_commit_id.is_none());
    let blocked = database
        .block_plan_phase_merge(
            "plan-blocked-merge-complete",
            "plan-blocked-merge-complete-phase-1",
            "cannot merge Agent worktree while shared workspace has uncommitted changes",
        )
        .expect("block merge");
    assert!(blocked.error_message.is_some());
    assert!(blocked.phases[0].error_message.is_some());
    let blocked_state = database
        .plan_auto_run_state()
        .expect("blocked auto-run state");
    assert!(blocked_state.desired_enabled);
    assert!(!blocked_state.enabled);
    assert_eq!(
        blocked_state.blocked_reason.as_deref(),
        Some("merge_blocked")
    );

    let merged = database
        .complete_plan_phase_by_id(
            "plan-blocked-merge-complete",
            "plan-blocked-merge-complete-phase-1",
            Some("shared-merge-commit"),
        )
        .expect("complete merge phase");

    assert_eq!(merged.status, "implemented");
    assert_eq!(
        merged.shared_merge_commit_id.as_deref(),
        Some("shared-merge-commit")
    );
    assert!(merged.error_message.is_none());
    assert_eq!(
        merged.phases[0].commit_id.as_deref(),
        Some("shared-merge-commit")
    );
    assert!(merged.phases[0].error_message.is_none());
    let resumed_state = database
        .plan_auto_run_state()
        .expect("resumed auto-run state");
    assert!(resumed_state.desired_enabled);
    assert!(resumed_state.enabled);
    assert!(resumed_state.blocked_reason.is_none());
}

#[test]
fn running_plan_phase_without_agent_run_reconciliation_marks_failed() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-running-without-agent-run",
            title: "Running without Agent run",
            overview: "A failed dispatch must not leave the phase stuck running.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-running-without-agent-run-phase-1",
                title: "Phase one",
                summary: "Dispatch failed before a chat was created.",
                steps: vec![NewPlanStep {
                    id: "plan-running-without-agent-run-step-1",
                    title: "Do work",
                    detail: "Create an Agent run.",
                    acceptance: vec!["phase is not stuck".to_string()],
                }],
            }],
        })
        .expect("create plan");

    let running = database
        .transition_plan("plan-running-without-agent-run", "start")
        .expect("start plan");
    assert_eq!(running.status, "running");
    assert_eq!(running.phases[0].status, "running");
    assert!(running.phases[0].implementation_chat_id.is_none());
    assert!(running.phases[0].agent_team_id.is_none());
    assert!(running.phases[0].agent_task_id.is_none());

    database
        .set_plan_auto_run_enabled(true)
        .expect("enable auto-run before dispatch repair");

    let repaired = database
        .fail_running_plan_phases_without_agent_runs(
            "Plan phase start did not create an implementation chat or Agent task",
        )
        .expect("repair running phase without Agent run");
    assert_eq!(repaired, 1);
    assert!(
        !database
            .plan_auto_run_state()
            .expect("auto-run state")
            .enabled
    );

    let failed = database
        .plan("plan-running-without-agent-run")
        .expect("failed plan")
        .expect("failed plan");
    assert_eq!(failed.status, "failed");
    assert!(failed.active_phase_id.is_none());
    assert_eq!(failed.phases[0].status, "failed");
    assert_eq!(failed.phases[0].steps[0].status, "failed");
    assert_eq!(
        failed.phases[0].error_message.as_deref(),
        Some("Plan phase start did not create an implementation chat or Agent task"),
    );
}

#[test]
fn plan_phase_derived_effects_are_idempotent_and_survive_reopen() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let attempt_id;
    {
        let mut database = WorkspaceDatabase::open_or_create_ungated(workspace.path())
            .expect("workspace database");
        database
            .create_plan(NewPlan {
                id: "plan-derived-effects",
                title: "Derived effects",
                overview: "Wait for integration.",
                status: "ready",
                source_chat_id: None,
                phases: vec![NewPlanPhase {
                    id: "plan-derived-effects-phase",
                    title: "Phase",
                    summary: "Implement.",
                    steps: vec![NewPlanStep {
                        id: "plan-derived-effects-step",
                        title: "Work",
                        detail: "Do it.",
                        acceptance: vec!["done".to_string()],
                    }],
                }],
            })
            .expect("create plan");
        database
            .transition_plan("plan-derived-effects", "start")
            .expect("start plan");
        let (team_id, instance_id) =
            create_test_agent_team(&mut database, "chat-derived-effects", "derived-effects");
        database
            .upsert_message_content(NewMessage {
                id: "user-derived-effects",
                chat_id: "chat-derived-effects",
                role: "user",
                content: "Implement",
                sequence: 0,
                metadata_json: None,
            })
            .expect("user message");
        database
            .upsert_message_content(NewMessage {
                id: "assistant-derived-effects",
                chat_id: "chat-derived-effects",
                role: "assistant",
                content: "Done",
                sequence: 1,
                metadata_json: None,
            })
            .expect("assistant message");
        let task_id = AgentTaskId::new("agent-task-derived-effects").expect("task id");
        database
            .enqueue_agent_task(NewAgentTask {
                id: &task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue task");
        let attempt = database
            .begin_plan_phase_attempt(
                "plan-derived-effects",
                "plan-derived-effects-phase",
                PlanPhaseAttemptTrigger::Initial,
                Some("provider"),
                Some("model"),
                None,
            )
            .expect("begin attempt");
        attempt_id = attempt.id.clone();
        database
            .attach_plan_phase_attempt_run(&attempt.id, "chat-derived-effects", &team_id, &task_id)
            .expect("attach run");
        let input = NewPlanPhaseDerivedEffects {
            attempt_id: &attempt.id,
            plan_id: &attempt.plan_id,
            phase_id: &attempt.phase_id,
            agent_task_id: &task_id,
            chat_id: "chat-derived-effects",
            run_id: task_id.as_str(),
            user_message_id: "user-derived-effects",
            assistant_message_id: "assistant-derived-effects",
            context_json: r#"{"runId":"agent-task-derived-effects"}"#,
        };
        let first = database
            .insert_plan_phase_derived_effects(input.clone())
            .expect("insert effects");
        let duplicate = database
            .insert_plan_phase_derived_effects(input)
            .expect("duplicate effects");
        assert_eq!(first, duplicate);
        assert_eq!(first.status, "awaiting_integration");
        assert!(first.integration_confirmed_at.is_none());
        let confirmed = database
            .confirm_plan_phase_derived_effects_integration(&attempt.id)
            .expect("confirm integration")
            .expect("confirmed effects");
        assert!(confirmed.integration_confirmed_at.is_some());
        assert_eq!(
            database
                .releasable_plan_phase_derived_effects()
                .expect("releasable effects")
                .len(),
            1
        );
    }

    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopen database");
    let pending = database
        .awaiting_plan_phase_derived_effects()
        .expect("pending effects");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].attempt_id, attempt_id);
    assert!(pending[0].integration_confirmed_at.is_some());
    let released = database
        .mark_plan_phase_derived_effects_released(&attempt_id)
        .expect("mark released")
        .expect("released effects");
    assert_eq!(released.status, "released");
    assert!(released.released_at.is_some());
    assert!(
        database
            .releasable_plan_phase_derived_effects()
            .expect("no releasable effects")
            .is_empty()
    );
}

#[test]
fn terminal_plan_phase_attempt_discards_unconfirmed_derived_effects() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-terminal-effects",
            title: "Terminal effects",
            overview: "Discard failed work.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-terminal-effects-phase",
                title: "Phase",
                summary: "Fail safely.",
                steps: vec![NewPlanStep {
                    id: "plan-terminal-effects-step",
                    title: "Work",
                    detail: "Do work.",
                    acceptance: vec!["done".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-terminal-effects", "start")
        .expect("start plan");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-terminal-effects",
            "plan-terminal-effects-phase",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider"),
            Some("model"),
            None,
        )
        .expect("begin attempt");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-terminal-effects", "terminal-effects");
    database
        .upsert_message_content(NewMessage {
            id: "user-terminal-effects",
            chat_id: "chat-terminal-effects",
            role: "user",
            content: "Implement",
            sequence: 0,
            metadata_json: None,
        })
        .expect("user message");
    database
        .upsert_message_content(NewMessage {
            id: "assistant-terminal-effects",
            chat_id: "chat-terminal-effects",
            role: "assistant",
            content: "Failed",
            sequence: 1,
            metadata_json: None,
        })
        .expect("assistant message");
    let task_id = AgentTaskId::new("agent-task-terminal-effects").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_attempt_run(&attempt.id, "chat-terminal-effects", &team_id, &task_id)
        .expect("attach attempt");
    database
        .insert_plan_phase_derived_effects(NewPlanPhaseDerivedEffects {
            attempt_id: &attempt.id,
            plan_id: &attempt.plan_id,
            phase_id: &attempt.phase_id,
            agent_task_id: &task_id,
            chat_id: "chat-terminal-effects",
            run_id: task_id.as_str(),
            user_message_id: "user-terminal-effects",
            assistant_message_id: "assistant-terminal-effects",
            context_json: "{}",
        })
        .expect("insert effects");
    database
        .fail_plan_phase_run(&task_id, "attempt failed")
        .expect("fail attempt");
    assert_eq!(
        database
            .discard_terminal_plan_phase_derived_effects("terminal attempt")
            .expect("discard terminal effects"),
        1
    );
    assert_eq!(
        database
            .discard_terminal_plan_phase_derived_effects("terminal attempt")
            .expect("discard terminal effects again"),
        0
    );
    let effects = database
        .plan_phase_derived_effects(&attempt.id)
        .expect("effects")
        .expect("effects record");
    assert_eq!(effects.status, "discarded");
    assert_eq!(effects.terminal_reason.as_deref(), Some("attempt failed"));
    assert!(effects.discarded_at.is_some());
}

#[test]
fn starting_failed_plan_phase_clears_previous_agent_run() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-restart-failed-phase",
            title: "Restart failed phase",
            overview: "Retry should create a fresh phase run.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-restart-failed-phase-1",
                title: "Phase one",
                summary: "Fails after an Agent run.",
                steps: vec![NewPlanStep {
                    id: "plan-restart-failed-step-1",
                    title: "Do work",
                    detail: "Complete the change.",
                    acceptance: vec!["fresh run".to_string()],
                }],
            }],
        })
        .expect("create plan");

    database
        .transition_plan("plan-restart-failed-phase", "start")
        .expect("start phase");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-plan-restart", "plan-restart");
    let task_id = AgentTaskId::new("agent-task-plan-restart").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    let first_attempt = database
        .begin_plan_phase_attempt(
            "plan-restart-failed-phase",
            "plan-restart-failed-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider-a"),
            Some("model-a"),
            None,
        )
        .expect("begin first attempt");
    database
        .attach_plan_phase_attempt_run(&first_attempt.id, "chat-plan-restart", &team_id, &task_id)
        .expect("attach phase attempt");

    database
        .set_plan_auto_run_enabled(true)
        .expect("enable auto-run before phase failure");

    let failed = database
        .fail_plan_phase_run(&task_id, "provider failed")
        .expect("fail phase")
        .expect("failed plan");
    assert!(
        !database
            .plan_auto_run_state()
            .expect("auto-run state")
            .enabled
    );
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.phases[0].status, "failed");
    assert_eq!(
        failed.phases[0].agent_task_id.as_deref(),
        Some("agent-task-plan-restart")
    );
    assert_eq!(failed.phases[0].steps[0].status, "failed");
    assert!(
        database
            .try_begin_plan_phase_merge_attempt(
                "plan-restart-failed-phase",
                "plan-restart-failed-phase-1",
                "merge failed",
            )
            .expect("record merge attempt")
    );

    let restarted = database
        .transition_plan("plan-restart-failed-phase", "start")
        .expect("restart failed phase");
    let phase = &restarted.phases[0];
    assert_eq!(restarted.status, "running");
    assert_eq!(
        restarted.active_phase_id.as_deref(),
        Some("plan-restart-failed-phase-1")
    );
    assert_eq!(phase.status, "running");
    assert!(phase.implementation_chat_id.is_none());
    assert!(phase.agent_team_id.is_none());
    assert!(phase.agent_task_id.is_none());
    assert!(phase.commit_id.is_none());
    assert_eq!(phase.merge_attempt_count, 0);
    assert!(phase.error_message.is_none());
    assert!(phase.completed_at.is_none());
    assert_eq!(phase.steps[0].status, "pending");
    assert!(phase.steps[0].checked_at.is_none());

    let retry_attempt = database
        .begin_plan_phase_attempt(
            "plan-restart-failed-phase",
            "plan-restart-failed-phase-1",
            PlanPhaseAttemptTrigger::Retry,
            Some("provider-a"),
            Some("model-a"),
            None,
        )
        .expect("begin retry for restarted failed phase");
    assert_eq!(retry_attempt.trigger, "retry");
}

#[test]
fn plan_phase_attempt_history_survives_retry_and_second_failure() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-attempt-history",
            title: "Attempt history",
            overview: "Keep failed attempts.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-attempt-history-phase-1",
                title: "Phase one",
                summary: "Retry me.",
                steps: vec![NewPlanStep {
                    id: "plan-attempt-history-step-1",
                    title: "Do work",
                    detail: "Complete change.",
                    acceptance: vec!["done".to_string()],
                }],
            }],
        })
        .expect("create plan");

    database
        .transition_plan("plan-attempt-history", "start")
        .expect("start phase");
    let first_attempt = database
        .begin_plan_phase_attempt(
            "plan-attempt-history",
            "plan-attempt-history-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider-a"),
            Some("model-a"),
            Some("low"),
        )
        .expect("begin first attempt");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-plan-attempt-history-1",
        "plan-attempt-history-1",
    );
    let first_task_id = AgentTaskId::new("agent-task-plan-attempt-history-1").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &first_task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue first task");
    database
        .attach_plan_phase_attempt_run(
            &first_attempt.id,
            "chat-plan-attempt-history-1",
            &team_id,
            &first_task_id,
        )
        .expect("attach first attempt");
    database
        .fail_plan_phase_run(&first_task_id, "provider failed")
        .expect("fail first attempt");

    assert!(
        database
            .begin_plan_phase_attempt(
                "plan-attempt-history",
                "plan-attempt-history-phase-1",
                PlanPhaseAttemptTrigger::Retry,
                Some("provider-a"),
                Some("model-a"),
                Some("low"),
            )
            .is_ok(),
        "failed phase can retry"
    );
    assert!(
        database
            .begin_plan_phase_attempt(
                "plan-attempt-history",
                "plan-attempt-history-phase-1",
                PlanPhaseAttemptTrigger::Retry,
                Some("provider-a"),
                Some("model-a"),
                Some("low"),
            )
            .is_err(),
        "active retry is protected from duplicate dispatch"
    );
    let retry_attempt = database
        .plan_phase_attempts_for_phase("plan-attempt-history-phase-1")
        .expect("attempts")
        .into_iter()
        .find(|attempt| attempt.sequence == 1)
        .expect("retry attempt");
    assert_eq!(retry_attempt.trigger, "retry");
    assert_eq!(retry_attempt.provider_id.as_deref(), Some("provider-a"));
    assert_eq!(retry_attempt.model_id.as_deref(), Some("model-a"));
    assert_eq!(retry_attempt.thinking_level.as_deref(), Some("low"));

    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-plan-attempt-history-2",
        "plan-attempt-history-2",
    );
    let second_task_id = AgentTaskId::new("agent-task-plan-attempt-history-2").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &second_task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue second task");
    database
        .attach_plan_phase_attempt_run(
            &retry_attempt.id,
            "chat-plan-attempt-history-2",
            &team_id,
            &second_task_id,
        )
        .expect("attach retry attempt");
    database
        .fail_plan_phase_run(&second_task_id, "still failed")
        .expect("fail retry attempt");

    let attempts = database
        .plan_phase_attempts_for_phase("plan-attempt-history-phase-1")
        .expect("attempts");
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].status, "failed");
    assert_eq!(
        attempts[0].error_message.as_deref(),
        Some("provider failed")
    );
    assert_eq!(attempts[1].status, "failed");
    assert_eq!(attempts[1].error_message.as_deref(), Some("still failed"));
    assert!(
        database
            .begin_plan_phase_attempt(
                "plan-attempt-history",
                "plan-attempt-history-phase-1",
                PlanPhaseAttemptTrigger::ModelOverrideRetry,
                Some("provider-b"),
                Some("model-b"),
                Some("high"),
            )
            .is_ok(),
        "failed retry can be retried again with override config"
    );
}

#[test]
fn cancelled_plan_phase_run_marks_phase_cancelled_and_retryable() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-cancelled-phase-run",
            title: "Cancelled phase run",
            overview: "A user-cancelled Agent task should leave a retryable phase.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-cancelled-phase-run-1",
                title: "Phase one",
                summary: "Gets cancelled by the user.",
                steps: vec![NewPlanStep {
                    id: "plan-cancelled-phase-run-step-1",
                    title: "Do work",
                    detail: "Complete change.",
                    acceptance: vec!["retryable cancellation".to_string()],
                }],
            }],
        })
        .expect("create plan");

    database
        .transition_plan("plan-cancelled-phase-run", "start")
        .expect("start phase");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-cancelled-phase-run",
            "plan-cancelled-phase-run-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider-a"),
            Some("model-a"),
            None,
        )
        .expect("begin attempt");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-plan-cancelled-phase",
        "plan-cancelled-phase",
    );
    let task_id = AgentTaskId::new("agent-task-plan-cancelled-phase").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_attempt_run(&attempt.id, "chat-plan-cancelled-phase", &team_id, &task_id)
        .expect("attach attempt");
    database
        .claim_runnable_agent_task(
            &team_id,
            &task_id,
            &AgentAttemptId::new("agent-attempt-plan-cancelled-phase").expect("attempt id"),
        )
        .expect("claim task")
        .expect("claimed task");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &task_id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Cancel,
            result_json: None,
            error_json: Some(r#"{"message":"user cancelled the run"}"#),
            interruption_reason: None,
        })
        .expect("cancel task");

    database
        .set_plan_auto_run_enabled(true)
        .expect("enable auto-run before cancellation");

    let cancelled = database
        .cancel_plan_phase_run(&task_id, "user cancelled the run")
        .expect("cancel phase run")
        .expect("cancelled plan");

    assert!(
        !database
            .plan_auto_run_state()
            .expect("auto-run state")
            .enabled
    );
    assert_eq!(cancelled.status, "paused");
    assert!(cancelled.active_phase_id.is_none());
    assert!(cancelled.pause_requested_at.is_some());
    assert_eq!(
        cancelled.error_message.as_deref(),
        Some("user cancelled the run")
    );
    assert_eq!(cancelled.phases[0].status, "cancelled");
    assert_eq!(
        cancelled.phases[0].error_message.as_deref(),
        Some("user cancelled the run")
    );
    assert_eq!(cancelled.phases[0].steps[0].status, "cancelled");

    let attempts = database
        .plan_phase_attempts_for_phase("plan-cancelled-phase-run-1")
        .expect("attempts");
    assert_eq!(attempts[0].status, "cancelled");
    assert_eq!(
        attempts[0].error_message.as_deref(),
        Some("user cancelled the run")
    );
    assert!(
        database
            .begin_plan_phase_attempt(
                "plan-cancelled-phase-run",
                "plan-cancelled-phase-run-1",
                PlanPhaseAttemptTrigger::Retry,
                Some("provider-a"),
                Some("model-a"),
                None,
            )
            .is_ok(),
        "cancelled phase can retry"
    );
}

#[test]
fn cancelled_earliest_phase_blocks_resume_without_state_changes() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-cancelled-resume-barrier",
            title: "Cancelled resume barrier",
            overview: "Resume must not skip a cancelled phase.",
            status: "ready",
            source_chat_id: None,
            phases: vec![
                NewPlanPhase {
                    id: "plan-cancelled-resume-barrier-phase-1",
                    title: "Phase one",
                    summary: "Cancel me.",
                    steps: vec![NewPlanStep {
                        id: "plan-cancelled-resume-barrier-step-1",
                        title: "Do phase one",
                        detail: "Cancel before completion.",
                        acceptance: vec!["cancelled".to_string()],
                    }],
                },
                NewPlanPhase {
                    id: "plan-cancelled-resume-barrier-phase-2",
                    title: "Phase two",
                    summary: "Must remain pending.",
                    steps: vec![NewPlanStep {
                        id: "plan-cancelled-resume-barrier-step-2",
                        title: "Do phase two",
                        detail: "Wait for phase one retry.",
                        acceptance: vec!["not started".to_string()],
                    }],
                },
            ],
        })
        .expect("create plan");
    database
        .transition_plan("plan-cancelled-resume-barrier", "start")
        .expect("start first phase");
    database
        .cancel_plan_phase_by_id(
            "plan-cancelled-resume-barrier",
            "plan-cancelled-resume-barrier-phase-1",
            "user cancelled phase one",
        )
        .expect("cancel first phase");

    let error = database
        .transition_plan("plan-cancelled-resume-barrier", "resume")
        .expect_err("cancelled phase must block resume");
    assert!(matches!(error, WorkspaceDatabaseError::InvalidPlan { .. }));
    let plan = database
        .plan("plan-cancelled-resume-barrier")
        .expect("plan")
        .expect("plan");
    assert_eq!(plan.status, "paused");
    assert!(plan.active_phase_id.is_none());
    assert_eq!(plan.phases[0].status, "cancelled");
    assert_eq!(plan.phases[1].status, "pending");

    let refreshed = database
        .update_plan_step(
            "plan-cancelled-resume-barrier",
            "plan-cancelled-resume-barrier-step-2",
            PlanStepPatch {
                title: None,
                detail: Some("Refresh without clearing the cancellation barrier."),
                acceptance: None,
                status: None,
            },
        )
        .expect("refresh plan through step update");
    assert_eq!(refreshed.status, "paused");
    assert_eq!(refreshed.phases[0].status, "cancelled");
    assert_eq!(refreshed.phases[1].status, "pending");

    drop(database);
    let mut reopened =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopen database");
    let error = reopened
        .transition_plan("plan-cancelled-resume-barrier", "resume")
        .expect_err("persisted cancelled phase must block resume after restart");
    assert!(matches!(error, WorkspaceDatabaseError::InvalidPlan { .. }));
    let reopened_plan = reopened
        .plan("plan-cancelled-resume-barrier")
        .expect("reopened plan")
        .expect("reopened plan");
    assert_eq!(reopened_plan.status, "paused");
    assert_eq!(reopened_plan.phases[0].status, "cancelled");
    assert_eq!(reopened_plan.phases[1].status, "pending");
}

#[test]
fn retry_rejects_phase_with_incomplete_predecessor_without_creating_attempt() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-retry-order-barrier",
            title: "Retry order barrier",
            overview: "Later retries require completed predecessors.",
            status: "ready",
            source_chat_id: None,
            phases: vec![
                NewPlanPhase {
                    id: "plan-retry-order-barrier-phase-1",
                    title: "Phase one",
                    summary: "Still pending.",
                    steps: vec![NewPlanStep {
                        id: "plan-retry-order-barrier-step-1",
                        title: "Do phase one",
                        detail: "Must finish first.",
                        acceptance: vec!["completed".to_string()],
                    }],
                },
                NewPlanPhase {
                    id: "plan-retry-order-barrier-phase-2",
                    title: "Phase two",
                    summary: "Seeded as failed history.",
                    steps: vec![NewPlanStep {
                        id: "plan-retry-order-barrier-step-2",
                        title: "Do phase two",
                        detail: "Cannot retry yet.",
                        acceptance: vec!["blocked".to_string()],
                    }],
                },
            ],
        })
        .expect("create plan");
    let connection = Connection::open(database.database_path()).expect("open database");
    connection
        .execute(
            "UPDATE plan_phases SET status = 'failed', error_message = 'old failure' WHERE id = ?1",
            params!["plan-retry-order-barrier-phase-2"],
        )
        .expect("seed failed later phase");
    drop(connection);

    let error = database
        .begin_plan_phase_attempt(
            "plan-retry-order-barrier",
            "plan-retry-order-barrier-phase-2",
            PlanPhaseAttemptTrigger::Retry,
            Some("provider"),
            Some("model"),
            None,
        )
        .expect_err("later phase retry must be rejected");
    assert!(matches!(error, WorkspaceDatabaseError::InvalidPlan { .. }));
    assert!(
        database
            .plan_phase_attempts_for_phase("plan-retry-order-barrier-phase-2")
            .expect("attempts")
            .is_empty()
    );
}

#[test]
fn retry_allows_earliest_cancelled_phase_with_completed_later_history() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-retry-earliest-cancelled",
            title: "Retry earliest cancelled phase",
            overview: "Later completed history does not block the earliest phase retry.",
            status: "ready",
            source_chat_id: None,
            phases: vec![
                NewPlanPhase {
                    id: "plan-retry-earliest-cancelled-phase-1",
                    title: "Phase one",
                    summary: "Cancelled earlier phase.",
                    steps: vec![NewPlanStep {
                        id: "plan-retry-earliest-cancelled-step-1",
                        title: "Retry phase one",
                        detail: "Run again.",
                        acceptance: vec!["retried".to_string()],
                    }],
                },
                NewPlanPhase {
                    id: "plan-retry-earliest-cancelled-phase-2",
                    title: "Phase two",
                    summary: "Already completed in abnormal history.",
                    steps: vec![NewPlanStep {
                        id: "plan-retry-earliest-cancelled-step-2",
                        title: "Completed phase two",
                        detail: "Do not rerun.",
                        acceptance: vec!["preserved".to_string()],
                    }],
                },
            ],
        })
        .expect("create plan");
    let connection = Connection::open(database.database_path()).expect("open database");
    connection
        .execute_batch(
            "UPDATE plan_phases
             SET status = 'cancelled', error_message = 'old cancellation'
             WHERE id = 'plan-retry-earliest-cancelled-phase-1';
             UPDATE plan_steps
             SET status = 'cancelled'
             WHERE phase_id = 'plan-retry-earliest-cancelled-phase-1';
             UPDATE plan_phases
             SET status = 'completed', completed_at = '2026-07-10T00:00:00.000Z'
             WHERE id = 'plan-retry-earliest-cancelled-phase-2';
             UPDATE plan_steps
             SET status = 'completed', checked_at = '2026-07-10T00:00:00.000Z'
             WHERE phase_id = 'plan-retry-earliest-cancelled-phase-2';
             UPDATE plans
             SET status = 'paused', pause_requested_at = '2026-07-10T00:00:00.000Z'
             WHERE id = 'plan-retry-earliest-cancelled';",
        )
        .expect("seed abnormal history");
    drop(connection);

    let attempt = database
        .begin_plan_phase_attempt(
            "plan-retry-earliest-cancelled",
            "plan-retry-earliest-cancelled-phase-1",
            PlanPhaseAttemptTrigger::Retry,
            Some("provider-b"),
            Some("model-b"),
            Some("high"),
        )
        .expect("retry earliest cancelled phase");
    assert_eq!(attempt.sequence, 0);
    assert_eq!(attempt.trigger, "retry");
    assert_eq!(attempt.provider_id.as_deref(), Some("provider-b"));
    let plan = database
        .plan("plan-retry-earliest-cancelled")
        .expect("plan")
        .expect("plan");
    assert_eq!(plan.phases[0].status, "running");
    assert_eq!(plan.phases[1].status, "completed");

    let still_running = database
        .update_plan_step(
            "plan-retry-earliest-cancelled",
            "plan-retry-earliest-cancelled-step-1",
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some("completed"),
            },
        )
        .expect("complete retried earliest phase");
    assert_eq!(still_running.status, "running");
    assert_eq!(still_running.phases[0].status, "running");

    let completed_retry = database
        .complete_plan_phase_by_id(
            "plan-retry-earliest-cancelled",
            "plan-retry-earliest-cancelled-phase-1",
            None,
        )
        .expect("complete retry lifecycle");
    assert_eq!(completed_retry.status, "implemented");
    assert!(completed_retry.active_phase_id.is_none());
    assert_eq!(completed_retry.phases[0].status, "completed");
    assert_eq!(completed_retry.phases[1].status, "completed");

    let after_resume = database
        .transition_plan("plan-retry-earliest-cancelled", "resume")
        .expect("all-completed abnormal history stays implemented");
    assert_eq!(after_resume.status, "implemented");
    assert!(after_resume.active_phase_id.is_none());
    assert_eq!(after_resume.phases[1].status, "completed");
}

#[test]
fn completed_running_plan_phase_agent_tasks_finds_stale_completed_tasks() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-stale-completed-task",
            title: "Plan stale completed task",
            overview: "A completed Agent task should be resynced if the phase is still running.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-stale-completed-task-phase",
                title: "Phase one",
                summary: "Leaves a stale running phase.",
                steps: vec![NewPlanStep {
                    id: "plan-stale-completed-task-step",
                    title: "Do work",
                    detail: "Complete the change.",
                    acceptance: vec!["task discovered".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-stale-completed-task", "start")
        .expect("start plan");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-stale-completed-task",
        "stale-completed-task",
    );
    let task_id = AgentTaskId::new("agent-task-stale-completed-task").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_run(
            "plan-stale-completed-task",
            "plan-stale-completed-task-phase",
            "chat-stale-completed-task",
            &team_id,
            &task_id,
        )
        .expect("attach phase task");
    let attempt_id = AgentAttemptId::new("agent-attempt-stale-completed-task").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
        .expect("claim task");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &task_id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Complete,
            result_json: Some(r#"{"text":"done"}"#),
            error_json: None,
            interruption_reason: None,
        })
        .expect("complete task without syncing phase");

    assert_eq!(
        database
            .completed_running_plan_phase_agent_tasks()
            .expect("stale completed tasks"),
        vec![task_id]
    );
}

#[test]
fn terminal_agent_task_reconciliation_finishes_stale_running_plan_phase() {
    for (suffix, transition, expected_phase_status, expected_attempt_status) in [
        ("failed", AgentTaskTransition::Fail, "failed", "failed"),
        (
            "cancelled",
            AgentTaskTransition::Cancel,
            "cancelled",
            "cancelled",
        ),
        (
            "interrupted",
            AgentTaskTransition::Interrupt,
            "failed",
            "interrupted",
        ),
    ] {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database = WorkspaceDatabase::open_or_create_ungated(workspace.path())
            .expect("workspace database");
        let plan_id = format!("plan-stale-{suffix}-phase");
        let phase_id = format!("{plan_id}-1");
        let step_id = format!("plan-stale-{suffix}-step-1");

        database
            .create_plan(NewPlan {
                id: &plan_id,
                title: "Stale terminal phase",
                overview: "Startup reconciliation should repair a stale running phase.",
                status: "ready",
                source_chat_id: None,
                phases: vec![NewPlanPhase {
                    id: &phase_id,
                    title: "Phase one",
                    summary: "The Agent task ended before phase sync.",
                    steps: vec![NewPlanStep {
                        id: &step_id,
                        title: "Do work",
                        detail: "Complete the change.",
                        acceptance: vec!["phase failed".to_string()],
                    }],
                }],
            })
            .expect("create plan");
        database
            .transition_plan(&plan_id, "start")
            .expect("start phase");
        let (team_id, instance_id) = create_test_agent_team(
            &mut database,
            &format!("chat-stale-{suffix}"),
            &format!("stale-{suffix}"),
        );
        let task_id = AgentTaskId::new(format!("agent-task-stale-{suffix}")).expect("task id");
        let attempt_id =
            AgentAttemptId::new(format!("agent-attempt-stale-{suffix}")).expect("attempt id");
        database
            .enqueue_agent_task(NewAgentTask {
                id: &task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue task");
        database
            .begin_plan_phase_attempt(
                &plan_id,
                &phase_id,
                PlanPhaseAttemptTrigger::Initial,
                Some("provider"),
                Some("model"),
                None,
            )
            .expect("begin phase attempt");
        database
            .attach_plan_phase_attempt_run(
                &format!("plan-phase-attempt-{phase_id}-0"),
                &format!("chat-stale-{suffix}"),
                &team_id,
                &task_id,
            )
            .expect("attach phase attempt");
        database
            .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
            .expect("claim")
            .expect("claimed");
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &task_id,
                expected_status: AgentTaskStatus::Running,
                transition,
                result_json: None,
                error_json: Some(r#"{"message":"task reached terminal state before phase sync"}"#),
                interruption_reason: if matches!(transition, AgentTaskTransition::Interrupt) {
                    Some("backend restarted")
                } else {
                    None
                },
            })
            .expect("finish task");
        let stale = database
            .plan(&plan_id)
            .expect("stale plan")
            .expect("stale plan");
        assert_eq!(stale.status, "running");
        assert_eq!(stale.phases[0].status, "running");

        let repaired = database
            .fail_running_plan_phases_for_terminal_agent_tasks(
                "task reached terminal state before phase sync",
            )
            .expect("repair stale phase");
        assert_eq!(repaired, 1);
        let repaired_plan = database
            .plan(&plan_id)
            .expect("repaired plan")
            .expect("repaired plan");
        let expected_plan_status = if suffix == "cancelled" {
            "paused"
        } else {
            "failed"
        };
        assert_eq!(repaired_plan.status, expected_plan_status);
        assert_eq!(repaired_plan.phases[0].status, expected_phase_status);
        assert_eq!(
            repaired_plan.phases[0].steps[0].status,
            expected_phase_status
        );
        assert!(repaired_plan.active_phase_id.is_none());
        let attempts = database
            .plan_phase_attempts_for_phase(&phase_id)
            .expect("phase attempts");
        assert_eq!(attempts[0].status, expected_attempt_status);
    }
}

#[test]
fn plan_phase_attempt_stays_running_when_step_completion_has_active_execution() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-attempt-step-active",
            title: "Attempt step active",
            overview: "Step completion must not finish an active execution.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-attempt-step-active-phase-1",
                title: "Phase one",
                summary: "Still executing.",
                steps: vec![NewPlanStep {
                    id: "plan-attempt-step-active-step-1",
                    title: "Do work",
                    detail: "Mark complete while agent runs.",
                    acceptance: vec!["step completed".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-attempt-step-active", "start")
        .expect("start plan");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-attempt-step-active",
            "plan-attempt-step-active-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider"),
            Some("model"),
            None,
        )
        .expect("begin attempt");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-attempt-step-active",
        "attempt-step-active",
    );
    let task_id = AgentTaskId::new("agent-task-attempt-step-active").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_attempt_run(&attempt.id, "chat-attempt-step-active", &team_id, &task_id)
        .expect("attach attempt");

    let updated = database
        .update_plan_step(
            "plan-attempt-step-active",
            "plan-attempt-step-active-step-1",
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some("completed"),
            },
        )
        .expect("complete step");

    assert_eq!(updated.status, "running");
    assert_eq!(
        updated.active_phase_id.as_deref(),
        Some("plan-attempt-step-active-phase-1")
    );
    assert_eq!(updated.phases[0].status, "running");
    assert_eq!(updated.phases[0].steps[0].status, "completed");
    assert!(updated.phases[0].steps[0].checked_at.is_some());
    assert_eq!(updated.phases[0].attempts[0].status, "running");
    assert!(updated.phases[0].attempts[0].completed_at.is_none());
}

#[test]
fn plan_step_completion_keeps_phase_running_for_queued_running_and_waiting_tasks() {
    for (suffix, make_waiting) in [("queued", false), ("running", false), ("waiting", true)] {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
        let plan_id = format!("plan-step-active-{suffix}");
        let phase_id = format!("{plan_id}-phase-1");
        let step_id = format!("{plan_id}-step-1");
        database
            .create_plan(NewPlan {
                id: &plan_id,
                title: "Step active task",
                overview: "All steps completed while agent task is non-terminal.",
                status: "ready",
                source_chat_id: None,
                phases: vec![NewPlanPhase {
                    id: &phase_id,
                    title: "Phase one",
                    summary: "Bound agent task still active.",
                    steps: vec![NewPlanStep {
                        id: &step_id,
                        title: "Do work",
                        detail: "Complete all steps early.",
                        acceptance: vec!["step completed".to_string()],
                    }],
                }],
            })
            .expect("create plan");
        database
            .transition_plan(&plan_id, "start")
            .expect("start plan");
        let attempt = database
            .begin_plan_phase_attempt(
                &plan_id,
                &phase_id,
                PlanPhaseAttemptTrigger::Initial,
                Some("provider"),
                Some("model"),
                None,
            )
            .expect("begin attempt");
        let (team_id, instance_id) = create_test_agent_team(
            &mut database,
            &format!("chat-step-active-{suffix}"),
            &format!("step-active-{suffix}"),
        );
        let task_id =
            AgentTaskId::new(format!("agent-task-step-active-{suffix}")).expect("task id");
        database
            .enqueue_agent_task(NewAgentTask {
                id: &task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue task");
        database
            .attach_plan_phase_attempt_run(
                &attempt.id,
                &format!("chat-step-active-{suffix}"),
                &team_id,
                &task_id,
            )
            .expect("attach attempt");
        if suffix != "queued" {
            let attempt_id = AgentAttemptId::new(format!("agent-attempt-step-active-{suffix}"))
                .expect("attempt");
            database
                .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
                .expect("claim")
                .expect("claimed");
            if make_waiting {
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
                    .expect("wait task");
            }
        }

        let updated = database
            .update_plan_step(
                &plan_id,
                &step_id,
                PlanStepPatch {
                    title: None,
                    detail: None,
                    acceptance: None,
                    status: Some("completed"),
                },
            )
            .expect("complete step");

        assert_eq!(updated.status, "running", "plan status for {suffix}");
        assert_eq!(
            updated.active_phase_id.as_deref(),
            Some(phase_id.as_str()),
            "active_phase_id for {suffix}"
        );
        assert_eq!(updated.phases[0].status, "running", "phase for {suffix}");
        assert_eq!(
            updated.phases[0].steps[0].status, "completed",
            "step for {suffix}"
        );
        assert!(updated.phases[0].steps[0].checked_at.is_some());
        assert_eq!(
            updated.phases[0].attempts[0].status, "running",
            "attempt for {suffix}"
        );
        assert!(
            updated.phases[0].attempts[0].completed_at.is_none(),
            "attempt completed_at for {suffix}"
        );

        if make_waiting {
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
                .expect("resume task");
            let after_resume = database.plan(&plan_id).expect("plan").expect("plan");
            assert_eq!(after_resume.status, "running");
            assert_eq!(after_resume.phases[0].status, "running");
            assert_eq!(after_resume.phases[0].attempts[0].status, "running");
        }
    }
}

#[test]
fn plan_phase_completes_only_after_bound_task_completion_entry() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-complete-after-task",
            title: "Complete after task",
            overview: "Lifecycle complete finishes steps, phase, and attempt together.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-complete-after-task-phase-1",
                title: "Phase one",
                summary: "Needs complete_plan_phase_run.",
                steps: vec![NewPlanStep {
                    id: "plan-complete-after-task-step-1",
                    title: "Do work",
                    detail: "May be checked early.",
                    acceptance: vec!["task completed".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-complete-after-task", "start")
        .expect("start plan");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-complete-after-task",
            "plan-complete-after-task-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider"),
            Some("model"),
            None,
        )
        .expect("begin attempt");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-complete-after-task",
        "complete-after-task",
    );
    let task_id = AgentTaskId::new("agent-task-complete-after-task").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_attempt_run(&attempt.id, "chat-complete-after-task", &team_id, &task_id)
        .expect("attach attempt");
    let agent_attempt_id =
        AgentAttemptId::new("agent-attempt-complete-after-task").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &agent_attempt_id)
        .expect("claim")
        .expect("claimed");
    database
        .update_plan_step(
            "plan-complete-after-task",
            "plan-complete-after-task-step-1",
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some("completed"),
            },
        )
        .expect("complete step early");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &task_id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Complete,
            result_json: Some(r#"{"text":"done"}"#),
            error_json: None,
            interruption_reason: None,
        })
        .expect("complete agent task");

    let completed = database
        .complete_plan_phase_run(&task_id, Some("deadbeef"))
        .expect("complete phase run")
        .expect("phase plan");

    assert_eq!(completed.status, "implemented");
    assert!(completed.active_phase_id.is_none());
    assert_eq!(completed.phases[0].status, "completed");
    assert_eq!(completed.phases[0].steps[0].status, "completed");
    assert!(completed.phases[0].steps[0].checked_at.is_some());
    assert_eq!(completed.phases[0].attempts[0].status, "completed");
    assert!(completed.phases[0].attempts[0].completed_at.is_some());
    assert_eq!(
        completed.phases[0].attempts[0].commit_id.as_deref(),
        Some("deadbeef")
    );
}

#[test]
fn manual_plan_without_active_execution_implements_when_all_steps_complete() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-manual-step-complete",
            title: "Manual step complete",
            overview: "No bound attempt or task: steps still drive implemented.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-manual-step-complete-phase-1",
                title: "Phase one",
                summary: "Manual checkbox plan.",
                steps: vec![NewPlanStep {
                    id: "plan-manual-step-complete-step-1",
                    title: "Do work",
                    detail: "Finish manually.",
                    acceptance: vec!["done".to_string()],
                }],
            }],
        })
        .expect("create plan");

    let updated = database
        .update_plan_step(
            "plan-manual-step-complete",
            "plan-manual-step-complete-step-1",
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some("completed"),
            },
        )
        .expect("complete step");

    assert_eq!(updated.status, "implemented");
    assert!(updated.active_phase_id.is_none());
    assert_eq!(updated.phases[0].status, "completed");
    assert_eq!(updated.phases[0].steps[0].status, "completed");
    assert!(updated.phases[0].steps[0].checked_at.is_some());
    assert!(updated.phases[0].attempts.is_empty());
}

#[test]
fn manual_plan_recovers_from_failed_step_when_all_steps_completed() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-manual-recover-failed-step",
            title: "Manual recover failed step",
            overview: "Hand-managed failed step must not sticky-block implemented.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-manual-recover-failed-step-phase-1",
                title: "Phase one",
                summary: "Manual checkbox plan.",
                steps: vec![NewPlanStep {
                    id: "plan-manual-recover-failed-step-step-1",
                    title: "Do work",
                    detail: "Fail first, then complete.",
                    acceptance: vec!["done".to_string()],
                }],
            }],
        })
        .expect("create plan");

    let failed = database
        .update_plan_step(
            "plan-manual-recover-failed-step",
            "plan-manual-recover-failed-step-step-1",
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some("failed"),
            },
        )
        .expect("fail step");
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.phases[0].status, "failed");
    assert_eq!(failed.phases[0].steps[0].status, "failed");
    assert!(failed.phases[0].error_message.is_none());
    assert!(failed.phases[0].attempts.is_empty());

    let recovered = database
        .update_plan_step(
            "plan-manual-recover-failed-step",
            "plan-manual-recover-failed-step-step-1",
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some("completed"),
            },
        )
        .expect("complete step after fail");

    assert_eq!(recovered.status, "implemented");
    assert!(recovered.active_phase_id.is_none());
    assert_eq!(recovered.phases[0].status, "completed");
    assert_eq!(recovered.phases[0].steps[0].status, "completed");
    assert!(recovered.phases[0].steps[0].checked_at.is_some());
    assert!(recovered.phases[0].attempts.is_empty());
}

#[test]
fn plan_phase_follows_task_terminal_status_even_when_all_steps_completed() {
    for (
        suffix,
        transition,
        expected_phase_status,
        expected_attempt_status,
        expected_plan_status,
    ) in [
        (
            "failed",
            AgentTaskTransition::Fail,
            "failed",
            "failed",
            "failed",
        ),
        (
            "cancelled",
            AgentTaskTransition::Cancel,
            "cancelled",
            "cancelled",
            "paused",
        ),
        (
            "interrupted",
            AgentTaskTransition::Interrupt,
            "failed",
            "interrupted",
            "failed",
        ),
    ] {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
        let plan_id = format!("plan-steps-done-then-{suffix}");
        let phase_id = format!("{plan_id}-phase-1");
        let step_id = format!("{plan_id}-step-1");
        database
            .create_plan(NewPlan {
                id: &plan_id,
                title: "Steps done then terminal",
                overview: "Task outcome wins over completed steps.",
                status: "ready",
                source_chat_id: None,
                phases: vec![NewPlanPhase {
                    id: &phase_id,
                    title: "Phase one",
                    summary: "Steps already completed.",
                    steps: vec![NewPlanStep {
                        id: &step_id,
                        title: "Do work",
                        detail: "Checked before task ends.",
                        acceptance: vec!["steps done".to_string()],
                    }],
                }],
            })
            .expect("create plan");
        database
            .transition_plan(&plan_id, "start")
            .expect("start plan");
        let attempt = database
            .begin_plan_phase_attempt(
                &plan_id,
                &phase_id,
                PlanPhaseAttemptTrigger::Initial,
                Some("provider"),
                Some("model"),
                None,
            )
            .expect("begin attempt");
        let (team_id, instance_id) = create_test_agent_team(
            &mut database,
            &format!("chat-steps-done-{suffix}"),
            &format!("steps-done-{suffix}"),
        );
        let task_id = AgentTaskId::new(format!("agent-task-steps-done-{suffix}")).expect("task id");
        database
            .enqueue_agent_task(NewAgentTask {
                id: &task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue task");
        database
            .attach_plan_phase_attempt_run(
                &attempt.id,
                &format!("chat-steps-done-{suffix}"),
                &team_id,
                &task_id,
            )
            .expect("attach attempt");
        let agent_attempt_id =
            AgentAttemptId::new(format!("agent-attempt-steps-done-{suffix}")).expect("attempt");
        database
            .claim_runnable_agent_task(&team_id, &task_id, &agent_attempt_id)
            .expect("claim")
            .expect("claimed");
        database
            .update_plan_step(
                &plan_id,
                &step_id,
                PlanStepPatch {
                    title: None,
                    detail: None,
                    acceptance: None,
                    status: Some("completed"),
                },
            )
            .expect("complete all steps");
        let still_running = database.plan(&plan_id).expect("plan").expect("plan");
        assert_eq!(still_running.status, "running");
        assert_eq!(still_running.phases[0].status, "running");
        assert_eq!(still_running.phases[0].steps[0].status, "completed");

        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &task_id,
                expected_status: AgentTaskStatus::Running,
                transition,
                result_json: None,
                error_json: Some(r#"{"message":"task ended after steps completed"}"#),
                interruption_reason: if matches!(transition, AgentTaskTransition::Interrupt) {
                    Some("backend restarted")
                } else {
                    None
                },
            })
            .expect("finish task");

        let closed = if matches!(transition, AgentTaskTransition::Cancel) {
            database
                .cancel_plan_phase_run(&task_id, "task ended after steps completed")
                .expect("cancel phase")
                .expect("plan")
        } else {
            database
                .fail_plan_phase_run(&task_id, "task ended after steps completed")
                .expect("fail phase")
                .expect("plan")
        };

        assert_eq!(closed.status, expected_plan_status, "plan for {suffix}");
        assert!(
            closed.active_phase_id.is_none(),
            "active phase for {suffix}"
        );
        assert_eq!(
            closed.phases[0].status, expected_phase_status,
            "phase for {suffix}"
        );
        assert_eq!(
            closed.phases[0].attempts[0].status, expected_attempt_status,
            "attempt for {suffix}"
        );
        assert!(
            closed.phases[0].attempts[0].completed_at.is_some(),
            "attempt completed_at for {suffix}"
        );
    }
}

#[test]
fn plan_phase_failed_status_stays_failed_after_step_edit_when_all_steps_completed() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-failed-sticky-step-edit",
            title: "Failed sticky step edit",
            overview: "Task-failed phase must not revive on later step edits.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-failed-sticky-step-edit-phase-1",
                title: "Phase one",
                summary: "Fails after steps complete.",
                steps: vec![NewPlanStep {
                    id: "plan-failed-sticky-step-edit-step-1",
                    title: "Do work",
                    detail: "Checked before task fails.",
                    acceptance: vec!["steps done".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-failed-sticky-step-edit", "start")
        .expect("start plan");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-failed-sticky-step-edit",
            "plan-failed-sticky-step-edit-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider"),
            Some("model"),
            None,
        )
        .expect("begin attempt");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-failed-sticky-step-edit",
        "failed-sticky-step-edit",
    );
    let task_id = AgentTaskId::new("agent-task-failed-sticky-step-edit").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_attempt_run(
            &attempt.id,
            "chat-failed-sticky-step-edit",
            &team_id,
            &task_id,
        )
        .expect("attach attempt");
    let agent_attempt_id =
        AgentAttemptId::new("agent-attempt-failed-sticky-step-edit").expect("attempt");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &agent_attempt_id)
        .expect("claim")
        .expect("claimed");
    database
        .update_plan_step(
            "plan-failed-sticky-step-edit",
            "plan-failed-sticky-step-edit-step-1",
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some("completed"),
            },
        )
        .expect("complete all steps");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &task_id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Fail,
            result_json: None,
            error_json: Some(r#"{"message":"task failed after steps completed"}"#),
            interruption_reason: None,
        })
        .expect("fail task");
    let failed = database
        .fail_plan_phase_run(&task_id, "task failed after steps completed")
        .expect("fail phase")
        .expect("plan");
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.phases[0].status, "failed");
    assert_eq!(failed.phases[0].steps[0].status, "completed");
    assert_eq!(failed.phases[0].attempts[0].status, "failed");

    let after_edit = database
        .update_plan_step(
            "plan-failed-sticky-step-edit",
            "plan-failed-sticky-step-edit-step-1",
            PlanStepPatch {
                title: Some("Do work (edited)"),
                detail: Some("Still completed; title-only refresh."),
                acceptance: None,
                status: None,
            },
        )
        .expect("edit step without status change");

    assert_eq!(after_edit.status, "failed");
    assert!(after_edit.active_phase_id.is_none());
    assert_eq!(after_edit.phases[0].status, "failed");
    assert_eq!(after_edit.phases[0].steps[0].status, "completed");
    assert!(after_edit.phases[0].steps[0].checked_at.is_some());
    assert_eq!(after_edit.phases[0].attempts[0].status, "failed");
    assert!(after_edit.phases[0].attempts[0].completed_at.is_some());
}

#[test]
fn plan_step_completion_keeps_phase_running_for_merge_task_without_active_attempt() {
    for (suffix, make_waiting) in [("queued", false), ("running", false), ("waiting", true)] {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
        let plan_id = format!("plan-merge-task-gate-{suffix}");
        let phase_id = format!("{plan_id}-phase-1");
        let step_id = format!("{phase_id}-step");
        let merge_task_id = attach_test_plan_merge_run(
            &mut database,
            &plan_id,
            &phase_id,
            &format!("merge-task-gate-{suffix}"),
        );
        let plan_before = database.plan(&plan_id).expect("plan").expect("plan");
        assert!(
            plan_before.phases[0].attempts.is_empty()
                || plan_before.phases[0]
                    .attempts
                    .iter()
                    .all(|attempt| !matches!(attempt.status.as_str(), "queued" | "running")),
            "merge gate must not rely on an active plan_phase_attempt for {suffix}"
        );
        assert_eq!(
            plan_before.phases[0].agent_task_id.as_deref(),
            Some(merge_task_id.as_str())
        );
        assert_eq!(plan_before.phases[0].steps[0].status, "completed");

        let team_id = AgentTeamId::new(format!("agent-team-merge-task-gate-{suffix}-merge"))
            .expect("team id");
        if suffix != "queued" {
            let attempt_id = AgentAttemptId::new(format!("agent-attempt-merge-task-gate-{suffix}"))
                .expect("attempt");
            database
                .claim_runnable_agent_task(&team_id, &merge_task_id, &attempt_id)
                .expect("claim")
                .expect("claimed");
            if make_waiting {
                database
                    .update_agent_task_state(AgentTaskStateUpdate {
                        team_id: &team_id,
                        task_id: &merge_task_id,
                        expected_status: AgentTaskStatus::Running,
                        transition: AgentTaskTransition::Wait,
                        result_json: None,
                        error_json: None,
                        interruption_reason: None,
                    })
                    .expect("wait merge task");
            }
        }

        let updated = database
            .update_plan_step(
                &plan_id,
                &step_id,
                PlanStepPatch {
                    title: Some("Do work (still complete)"),
                    detail: None,
                    acceptance: None,
                    status: Some("completed"),
                },
            )
            .expect("refresh steps while merge task active");

        assert_eq!(updated.status, "running", "plan status for {suffix}");
        assert_eq!(
            updated.active_phase_id.as_deref(),
            Some(phase_id.as_str()),
            "active_phase_id for {suffix}"
        );
        assert_eq!(updated.phases[0].status, "running", "phase for {suffix}");
        assert_eq!(
            updated.phases[0].steps[0].status, "completed",
            "step for {suffix}"
        );
        assert!(updated.phases[0].steps[0].checked_at.is_some());

        if make_waiting {
            database
                .update_agent_task_state(AgentTaskStateUpdate {
                    team_id: &team_id,
                    task_id: &merge_task_id,
                    expected_status: AgentTaskStatus::Waiting,
                    transition: AgentTaskTransition::Resume,
                    result_json: None,
                    error_json: None,
                    interruption_reason: None,
                })
                .expect("resume merge task");
            let after_resume = database.plan(&plan_id).expect("plan").expect("plan");
            assert_eq!(after_resume.status, "running");
            assert_eq!(after_resume.phases[0].status, "running");

            database
                .update_agent_task_state(AgentTaskStateUpdate {
                    team_id: &team_id,
                    task_id: &merge_task_id,
                    expected_status: AgentTaskStatus::Running,
                    transition: AgentTaskTransition::Complete,
                    result_json: Some(r#"{"text":"merged"}"#),
                    error_json: None,
                    interruption_reason: None,
                })
                .expect("complete merge task");
            let completed = database
                .complete_plan_phase_run(&merge_task_id, Some("shared-merge-commit"))
                .expect("complete merge phase")
                .expect("plan");
            assert_eq!(completed.status, "implemented");
            assert!(completed.active_phase_id.is_none());
            assert_eq!(completed.phases[0].status, "completed");
            assert_eq!(
                completed.phases[0].commit_id.as_deref(),
                Some("shared-merge-commit")
            );
        }
    }
}

#[test]
fn plan_step_edit_keeps_phase_running_while_bound_task_is_terminal_before_lifecycle_sync() {
    for (suffix, transition) in [
        ("completed", AgentTaskTransition::Complete),
        ("failed", AgentTaskTransition::Fail),
        ("cancelled", AgentTaskTransition::Cancel),
        ("interrupted", AgentTaskTransition::Interrupt),
    ] {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
        let plan_id = format!("plan-terminal-before-sync-{suffix}");
        let phase_id = format!("{plan_id}-phase-1");
        let step_id = format!("{phase_id}-step");
        let merge_task_id = attach_test_plan_merge_run(
            &mut database,
            &plan_id,
            &phase_id,
            &format!("terminal-before-sync-{suffix}"),
        );
        let plan_before = database.plan(&plan_id).expect("plan").expect("plan");
        assert!(
            plan_before.phases[0].attempts.is_empty()
                || plan_before.phases[0]
                    .attempts
                    .iter()
                    .all(|attempt| !matches!(attempt.status.as_str(), "queued" | "running")),
            "fixture must rely on bound task, not an active plan_phase_attempt, for {suffix}"
        );
        assert_eq!(plan_before.phases[0].status, "running");
        assert_eq!(plan_before.phases[0].steps[0].status, "completed");

        let team_id = AgentTeamId::new(format!("agent-team-terminal-before-sync-{suffix}-merge"))
            .expect("team id");
        let attempt_id =
            AgentAttemptId::new(format!("agent-attempt-terminal-before-sync-{suffix}"))
                .expect("attempt");
        database
            .claim_runnable_agent_task(&team_id, &merge_task_id, &attempt_id)
            .expect("claim")
            .expect("claimed");
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &merge_task_id,
                expected_status: AgentTaskStatus::Running,
                transition,
                result_json: matches!(transition, AgentTaskTransition::Complete)
                    .then_some(r#"{"text":"merged"}"#),
                error_json: (!matches!(transition, AgentTaskTransition::Complete))
                    .then_some(r#"{"message":"merge ended before phase sync"}"#),
                interruption_reason: matches!(transition, AgentTaskTransition::Interrupt)
                    .then_some("backend restarted"),
            })
            .expect("finish merge task without phase lifecycle sync");

        let updated = database
            .update_plan_step(
                &plan_id,
                &step_id,
                PlanStepPatch {
                    title: Some("Do work (still complete)"),
                    detail: None,
                    acceptance: None,
                    status: Some("completed"),
                },
            )
            .expect("step edit before lifecycle sync");

        assert_eq!(updated.status, "running", "plan status for {suffix}");
        assert_eq!(
            updated.active_phase_id.as_deref(),
            Some(phase_id.as_str()),
            "active_phase_id for {suffix}"
        );
        assert_eq!(updated.phases[0].status, "running", "phase for {suffix}");
        assert_eq!(
            updated.phases[0].steps[0].status, "completed",
            "step for {suffix}"
        );
        assert!(updated.phases[0].steps[0].checked_at.is_some());

        let closed = match transition {
            AgentTaskTransition::Complete => database
                .complete_plan_phase_run(&merge_task_id, Some("shared-merge-commit"))
                .expect("complete phase after window")
                .expect("plan"),
            AgentTaskTransition::Cancel => database
                .cancel_plan_phase_run(&merge_task_id, "merge ended before phase sync")
                .expect("cancel phase after window")
                .expect("plan"),
            AgentTaskTransition::Fail | AgentTaskTransition::Interrupt => database
                .fail_plan_phase_run(&merge_task_id, "merge ended before phase sync")
                .expect("fail phase after window")
                .expect("plan"),
            _ => unreachable!("unexpected transition for {suffix}"),
        };

        match transition {
            AgentTaskTransition::Complete => {
                assert_eq!(closed.status, "implemented", "final plan for {suffix}");
                assert_eq!(
                    closed.phases[0].status, "completed",
                    "final phase for {suffix}"
                );
            }
            AgentTaskTransition::Cancel => {
                assert_eq!(closed.status, "paused", "final plan for {suffix}");
                assert_eq!(
                    closed.phases[0].status, "cancelled",
                    "final phase for {suffix}"
                );
            }
            AgentTaskTransition::Fail | AgentTaskTransition::Interrupt => {
                assert_eq!(closed.status, "failed", "final plan for {suffix}");
                assert_eq!(
                    closed.phases[0].status, "failed",
                    "final phase for {suffix}"
                );
            }
            _ => unreachable!("unexpected transition for {suffix}"),
        }
        assert!(
            closed.active_phase_id.is_none(),
            "active phase for {suffix}"
        );
    }
}

#[test]
fn plan_phase_attempt_reconciliation_copies_terminal_phase_state() {
    for (suffix, terminal_status, commit_id, error_message) in [
        ("completed", "completed", Some("commit-completed"), None),
        ("failed", "failed", None, Some("phase failed")),
        ("cancelled", "cancelled", None, None),
    ] {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
        let plan_id = format!("plan-attempt-reconcile-{suffix}");
        let phase_id = format!("{plan_id}-phase-1");
        let step_id = format!("{plan_id}-step-1");
        database
            .create_plan(NewPlan {
                id: &plan_id,
                title: "Attempt reconcile",
                overview: "Repair active attempts for terminal phases.",
                status: "ready",
                source_chat_id: None,
                phases: vec![NewPlanPhase {
                    id: &phase_id,
                    title: "Phase one",
                    summary: "Already terminal.",
                    steps: vec![NewPlanStep {
                        id: &step_id,
                        title: "Do work",
                        detail: "Reach terminal state.",
                        acceptance: vec!["attempt reconciled".to_string()],
                    }],
                }],
            })
            .expect("create plan");
        database
            .transition_plan(&plan_id, "start")
            .expect("start plan");
        let attempt = database
            .begin_plan_phase_attempt(
                &plan_id,
                &phase_id,
                PlanPhaseAttemptTrigger::Initial,
                Some("provider"),
                Some("model"),
                None,
            )
            .expect("begin attempt");
        let (team_id, instance_id) = create_test_agent_team(
            &mut database,
            &format!("chat-attempt-reconcile-{suffix}"),
            &format!("attempt-reconcile-{suffix}"),
        );
        let task_id =
            AgentTaskId::new(format!("agent-task-attempt-reconcile-{suffix}")).expect("task id");
        database
            .enqueue_agent_task(NewAgentTask {
                id: &task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue task");
        database
            .attach_plan_phase_attempt_run(
                &attempt.id,
                &format!("chat-attempt-reconcile-{suffix}"),
                &team_id,
                &task_id,
            )
            .expect("attach attempt");
        let connection = Connection::open(database.database_path()).expect("open database");
        connection
            .execute(
                "UPDATE plan_phases
                 SET status = ?2,
                     commit_id = ?3,
                     error_message = ?4,
                     completed_at = '2026-07-02T00:00:00.000Z',
                     updated_at = '2026-07-02T00:00:00.000Z'
                 WHERE id = ?1",
                params![phase_id, terminal_status, commit_id, error_message],
            )
            .expect("make phase terminal");
        connection
            .execute(
                "INSERT INTO plan_phase_attempts (
                    id, plan_id, phase_id, sequence, trigger, status, error_message, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 1, 'initial', 'failed', 'old terminal',
                    '2026-07-01T00:00:00.000Z', '2026-07-01T00:00:00.000Z')",
                params![
                    format!("plan-phase-attempt-{phase_id}-terminal"),
                    plan_id,
                    phase_id
                ],
            )
            .expect("insert terminal attempt history");
        drop(connection);

        let repaired = database
            .reconcile_plan_phase_attempts_for_terminal_phases()
            .expect("reconcile attempts");
        assert_eq!(repaired, 1);
        let attempts = database
            .plan_phase_attempts_for_phase(&phase_id)
            .expect("phase attempts");
        assert_eq!(attempts[0].status, terminal_status);
        assert_eq!(attempts[0].commit_id.as_deref(), commit_id);
        assert_eq!(attempts[0].error_message.as_deref(), error_message);
        assert_eq!(
            attempts[0].completed_at.as_deref(),
            Some("2026-07-02T00:00:00.000Z")
        );
        assert_eq!(attempts[1].status, "failed");
        assert_eq!(attempts[1].error_message.as_deref(), Some("old terminal"));
    }
}

#[test]
fn reconcile_prematurely_completed_plan_phases_reopens_active_task_phase() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-premature-reopen",
            title: "Premature reopen",
            overview: "Startup repair reopens false completed phase.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-premature-reopen-phase-1",
                title: "Phase one",
                summary: "Still executing.",
                steps: vec![NewPlanStep {
                    id: "plan-premature-reopen-step-1",
                    title: "Do work",
                    detail: "Completed early via bug.",
                    acceptance: vec!["step completed".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-premature-reopen", "start")
        .expect("start plan");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-premature-reopen",
            "plan-premature-reopen-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider"),
            Some("model"),
            None,
        )
        .expect("begin attempt");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-premature-reopen", "premature-reopen");
    let task_id = AgentTaskId::new("agent-task-premature-reopen").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_attempt_run(&attempt.id, "chat-premature-reopen", &team_id, &task_id)
        .expect("attach attempt");
    let agent_attempt_id =
        AgentAttemptId::new("agent-attempt-premature-reopen").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &agent_attempt_id)
        .expect("claim")
        .expect("claimed");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &task_id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Wait,
            result_json: Some(r#"{"control":{"kind":"agent_wait_tasks"}}"#),
            error_json: None,
            interruption_reason: None,
        })
        .expect("wait task");
    database
        .update_plan_step(
            "plan-premature-reopen",
            "plan-premature-reopen-step-1",
            PlanStepPatch {
                title: None,
                detail: None,
                acceptance: None,
                status: Some("completed"),
            },
        )
        .expect("complete step");

    // Simulate the pre-fix row shape: phase/attempt completed without commit
    // while the bound task is still waiting (step remains completed).
    let connection = Connection::open(database.database_path()).expect("open database");
    connection
        .execute(
            "UPDATE plan_phases
             SET status = 'completed',
                 commit_id = NULL,
                 error_message = NULL,
                 completed_at = '2026-07-16T00:00:00.000Z',
                 updated_at = '2026-07-16T00:00:00.000Z'
             WHERE id = 'plan-premature-reopen-phase-1'",
            [],
        )
        .expect("force phase completed");
    connection
        .execute(
            "UPDATE plan_phase_attempts
             SET status = 'completed',
                 commit_id = NULL,
                 error_message = NULL,
                 completed_at = '2026-07-16T00:00:00.000Z',
                 updated_at = '2026-07-16T00:00:00.000Z'
             WHERE id = ?1",
            params![attempt.id],
        )
        .expect("force attempt completed");
    connection
        .execute(
            "UPDATE plans
             SET status = 'implemented',
                 active_phase_id = NULL,
                 completed_at = '2026-07-16T00:00:00.000Z',
                 updated_at = '2026-07-16T00:00:00.000Z'
             WHERE id = 'plan-premature-reopen'",
            [],
        )
        .expect("force plan implemented");
    drop(connection);

    let repaired = database
        .reconcile_prematurely_completed_plan_phases_with_active_tasks()
        .expect("reconcile premature");
    assert_eq!(repaired, 1);

    let plan = database
        .plan("plan-premature-reopen")
        .expect("plan")
        .expect("plan");
    assert_eq!(plan.status, "running");
    assert_eq!(
        plan.active_phase_id.as_deref(),
        Some("plan-premature-reopen-phase-1")
    );
    assert_eq!(plan.phases[0].status, "running");
    assert!(plan.phases[0].completed_at.is_none());
    assert_eq!(plan.phases[0].steps[0].status, "completed");
    assert!(plan.phases[0].steps[0].checked_at.is_some());
    assert_eq!(plan.phases[0].attempts[0].status, "running");
    assert!(plan.phases[0].attempts[0].completed_at.is_none());
    assert_eq!(
        database
            .agent_task(&task_id)
            .expect("task")
            .expect("task")
            .status,
        AgentTaskStatus::Waiting
    );
}

#[test]
fn reconcile_prematurely_completed_plan_phases_skips_when_later_phase_active() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-premature-skip-later",
            title: "Premature skip later",
            overview: "Do not reopen when a later phase already progressed.",
            status: "ready",
            source_chat_id: None,
            phases: vec![
                NewPlanPhase {
                    id: "plan-premature-skip-later-phase-1",
                    title: "Phase one",
                    summary: "False completed.",
                    steps: vec![NewPlanStep {
                        id: "plan-premature-skip-later-step-1",
                        title: "Do work",
                        detail: "Completed early.",
                        acceptance: vec!["done".to_string()],
                    }],
                },
                NewPlanPhase {
                    id: "plan-premature-skip-later-phase-2",
                    title: "Phase two",
                    summary: "Already running.",
                    steps: vec![NewPlanStep {
                        id: "plan-premature-skip-later-step-2",
                        title: "Next",
                        detail: "Later activity.",
                        acceptance: vec!["running".to_string()],
                    }],
                },
            ],
        })
        .expect("create plan");
    database
        .transition_plan("plan-premature-skip-later", "start")
        .expect("start plan");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-premature-skip-later",
            "plan-premature-skip-later-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider"),
            Some("model"),
            None,
        )
        .expect("begin attempt");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-premature-skip-later",
        "premature-skip-later",
    );
    let task_id = AgentTaskId::new("agent-task-premature-skip-later").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_attempt_run(&attempt.id, "chat-premature-skip-later", &team_id, &task_id)
        .expect("attach attempt");
    database
        .claim_runnable_agent_task(
            &team_id,
            &task_id,
            &AgentAttemptId::new("agent-attempt-premature-skip-later").expect("attempt"),
        )
        .expect("claim")
        .expect("claimed");

    let connection = Connection::open(database.database_path()).expect("open database");
    connection
        .execute(
            "UPDATE plan_phases
             SET status = 'completed',
                 commit_id = NULL,
                 error_message = NULL,
                 completed_at = '2026-07-16T00:00:00.000Z'
             WHERE id = 'plan-premature-skip-later-phase-1'",
            [],
        )
        .expect("force phase 1 completed");
    connection
        .execute(
            "UPDATE plan_phase_attempts
             SET status = 'completed',
                 completed_at = '2026-07-16T00:00:00.000Z'
             WHERE id = ?1",
            params![attempt.id],
        )
        .expect("force attempt completed");
    connection
        .execute(
            "UPDATE plan_phases
             SET status = 'running',
                 started_at = COALESCE(started_at, '2026-07-16T00:00:01.000Z'),
                 completed_at = NULL
             WHERE id = 'plan-premature-skip-later-phase-2'",
            [],
        )
        .expect("mark later phase running");
    connection
        .execute(
            "UPDATE plans
             SET status = 'running',
                 active_phase_id = 'plan-premature-skip-later-phase-2'
             WHERE id = 'plan-premature-skip-later'",
            [],
        )
        .expect("point plan at phase 2");
    drop(connection);

    let repaired = database
        .reconcile_prematurely_completed_plan_phases_with_active_tasks()
        .expect("reconcile");
    assert_eq!(repaired, 0);

    let plan = database
        .plan("plan-premature-skip-later")
        .expect("plan")
        .expect("plan");
    assert_eq!(plan.phases[0].status, "completed");
    assert_eq!(plan.phases[0].attempts[0].status, "completed");
    assert_eq!(plan.phases[1].status, "running");
    assert_eq!(
        plan.active_phase_id.as_deref(),
        Some("plan-premature-skip-later-phase-2")
    );
}

#[test]
fn reconcile_prematurely_completed_plan_phases_skips_real_commit_and_terminal_task() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-premature-skip-real",
            title: "Premature skip real",
            overview: "Real commit or terminal task must not reopen.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-premature-skip-real-phase-1",
                title: "Phase one",
                summary: "Legitimately completed.",
                steps: vec![NewPlanStep {
                    id: "plan-premature-skip-real-step-1",
                    title: "Do work",
                    detail: "Finished.",
                    acceptance: vec!["done".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-premature-skip-real", "start")
        .expect("start plan");
    let attempt = database
        .begin_plan_phase_attempt(
            "plan-premature-skip-real",
            "plan-premature-skip-real-phase-1",
            PlanPhaseAttemptTrigger::Initial,
            Some("provider"),
            Some("model"),
            None,
        )
        .expect("begin attempt");
    let (team_id, instance_id) = create_test_agent_team(
        &mut database,
        "chat-premature-skip-real",
        "premature-skip-real",
    );
    let task_id = AgentTaskId::new("agent-task-premature-skip-real").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_attempt_run(&attempt.id, "chat-premature-skip-real", &team_id, &task_id)
        .expect("attach attempt");
    let agent_attempt_id =
        AgentAttemptId::new("agent-attempt-premature-skip-real").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &agent_attempt_id)
        .expect("claim")
        .expect("claimed");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &task_id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Complete,
            result_json: Some(r#"{"text":"done"}"#),
            error_json: None,
            interruption_reason: None,
        })
        .expect("complete task");
    database
        .complete_plan_phase_run(&task_id, Some("deadbeef"))
        .expect("complete phase")
        .expect("plan");

    let repaired = database
        .reconcile_prematurely_completed_plan_phases_with_active_tasks()
        .expect("reconcile");
    assert_eq!(repaired, 0);
    let plan = database
        .plan("plan-premature-skip-real")
        .expect("plan")
        .expect("plan");
    assert_eq!(plan.status, "implemented");
    assert_eq!(plan.phases[0].status, "completed");
    assert_eq!(plan.phases[0].commit_id.as_deref(), Some("deadbeef"));
    assert_eq!(plan.phases[0].attempts[0].status, "completed");
}

#[test]
fn plan_phase_attempt_migration_024_reconciles_terminal_phases() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .create_plan(NewPlan {
            id: "plan-attempt-migration-024",
            title: "Attempt migration",
            overview: "Migration repairs stale active attempts.",
            status: "ready",
            source_chat_id: None,
            phases: vec![
                NewPlanPhase {
                    id: "plan-attempt-migration-024-completed",
                    title: "Completed phase",
                    summary: "Has commit.",
                    steps: vec![NewPlanStep {
                        id: "plan-attempt-migration-024-completed-step",
                        title: "Do completed work",
                        detail: "Done.",
                        acceptance: vec!["completed".to_string()],
                    }],
                },
                NewPlanPhase {
                    id: "plan-attempt-migration-024-failed",
                    title: "Failed phase",
                    summary: "Has error.",
                    steps: vec![NewPlanStep {
                        id: "plan-attempt-migration-024-failed-step",
                        title: "Do failed work",
                        detail: "Fail.",
                        acceptance: vec!["failed".to_string()],
                    }],
                },
                NewPlanPhase {
                    id: "plan-attempt-migration-024-cancelled",
                    title: "Cancelled phase",
                    summary: "Was cancelled.",
                    steps: vec![NewPlanStep {
                        id: "plan-attempt-migration-024-cancelled-step",
                        title: "Do cancelled work",
                        detail: "Cancel.",
                        acceptance: vec!["cancelled".to_string()],
                    }],
                },
            ],
        })
        .expect("create plan");
    let database_path = database.database_path().to_path_buf();
    drop(database);

    let connection = Connection::open(&database_path).expect("open database");
    for (sequence, phase_id) in [
        "plan-attempt-migration-024-completed",
        "plan-attempt-migration-024-failed",
        "plan-attempt-migration-024-cancelled",
    ]
    .into_iter()
    .enumerate()
    {
        connection
            .execute(
                "INSERT INTO plan_phase_attempts (
                    id, plan_id, phase_id, sequence, trigger, status,
                    provider_id, model_id, created_at, updated_at
                 ) VALUES (?1, 'plan-attempt-migration-024', ?2, 0, 'initial', 'queued',
                    'provider', 'model', '2026-07-01T00:00:00.000Z', '2026-07-01T00:00:00.000Z')",
                params![
                    format!("plan-phase-attempt-migration-024-seed-{sequence}"),
                    phase_id
                ],
            )
            .expect("seed active attempt history");
    }
    connection
        .execute_batch(
            "DROP INDEX IF EXISTS workspace_spec_jobs_active_retry_idx;
             ALTER TABLE workspace_spec_jobs DROP COLUMN retry_of_job_id;
             ALTER TABLE workspace_spec_jobs DROP COLUMN lease_renewed_at;
             UPDATE plan_phases
             SET status = 'completed', commit_id = 'commit-from-phase', completed_at = '2026-07-02T00:00:00.000Z'
             WHERE id = 'plan-attempt-migration-024-completed';
             UPDATE plan_phases
             SET status = 'failed', error_message = 'phase failed', completed_at = '2026-07-02T00:00:01.000Z'
             WHERE id = 'plan-attempt-migration-024-failed';
             UPDATE plan_phases
             SET status = 'cancelled', completed_at = '2026-07-02T00:00:02.000Z'
             WHERE id = 'plan-attempt-migration-024-cancelled';
             PRAGMA user_version = 23;",
        )
        .expect("seed stale v23 data");
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let completed = database
        .plan_phase_attempts_for_phase("plan-attempt-migration-024-completed")
        .expect("completed attempts");
    assert_eq!(completed[0].status, "completed");
    assert_eq!(completed[0].commit_id.as_deref(), Some("commit-from-phase"));
    assert_eq!(
        completed[0].completed_at.as_deref(),
        Some("2026-07-02T00:00:00.000Z")
    );
    let failed = database
        .plan_phase_attempts_for_phase("plan-attempt-migration-024-failed")
        .expect("failed attempts");
    assert_eq!(failed[0].status, "failed");
    assert_eq!(failed[0].error_message.as_deref(), Some("phase failed"));
    let cancelled = database
        .plan_phase_attempts_for_phase("plan-attempt-migration-024-cancelled")
        .expect("cancelled attempts");
    assert_eq!(cancelled[0].status, "cancelled");
}

#[test]
fn plan_phase_merge_attempt_can_begin_once() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-merge-once",
            title: "Plan merge once",
            overview: "Only one automated merge retry is allowed.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-merge-once-phase",
                title: "Phase one",
                summary: "Needs merge automation.",
                steps: vec![NewPlanStep {
                    id: "plan-merge-once-step",
                    title: "Do work",
                    detail: "Complete the change.",
                    acceptance: vec!["merge retry recorded".to_string()],
                }],
            }],
        })
        .expect("create plan");

    assert!(
        database
            .try_begin_plan_phase_merge_attempt(
                "plan-merge-once",
                "plan-merge-once-phase",
                "first merge failure",
            )
            .expect("first merge attempt")
    );
    assert!(
        !database
            .try_begin_plan_phase_merge_attempt(
                "plan-merge-once",
                "plan-merge-once-phase",
                "second merge failure",
            )
            .expect("second merge attempt")
    );

    let plan = database
        .plan("plan-merge-once")
        .expect("plan lookup")
        .expect("plan");
    assert_eq!(plan.phases[0].merge_attempt_count, 1);
    let attempt = plan.phases[0]
        .attempts
        .iter()
        .find(|attempt| attempt.trigger == "merge_auto")
        .expect("durable merge attempt");
    assert_eq!(attempt.status, "queued");
    assert!(attempt.implementation_chat_id.is_none());
    assert!(attempt.agent_team_id.is_none());
    assert!(attempt.agent_task_id.is_none());
    assert_eq!(
        plan.phases[0].error_message.as_deref(),
        Some("first merge failure")
    );
}

#[test]
fn plan_phase_merge_run_keeps_plan_running_until_merge_task_finishes() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-merge-running",
            title: "Plan merge running",
            overview: "A failed fast-forward should keep the plan in flight.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-merge-running-phase",
                title: "Phase one",
                summary: "Needs merge automation.",
                steps: vec![NewPlanStep {
                    id: "plan-merge-running-step",
                    title: "Do work",
                    detail: "Complete the change.",
                    acceptance: vec!["merge retry attached".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-merge-running", "start")
        .expect("start plan");
    let (phase_team_id, phase_instance_id) = create_test_agent_team(
        &mut database,
        "chat-merge-running-phase",
        "merge-running-phase",
    );
    let phase_task_id = AgentTaskId::new("agent-task-merge-running-phase").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &phase_task_id,
            team_id: &phase_team_id,
            owner_instance_id: &phase_instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue phase task");
    database
        .attach_plan_phase_run(
            "plan-merge-running",
            "plan-merge-running-phase",
            "chat-merge-running-phase",
            &phase_team_id,
            &phase_task_id,
        )
        .expect("attach phase task");
    complete_test_agent_task(
        &mut database,
        &phase_team_id,
        &phase_task_id,
        "agent-attempt-merge-running-phase",
    );
    let implemented = database
        .complete_plan_phase_run(&phase_task_id, Some("worktree-commit"))
        .expect("complete phase")
        .expect("plan");
    assert_eq!(implemented.status, "implemented");
    assert_eq!(implemented.phases[0].status, "completed");

    assert!(
        database
            .try_begin_plan_phase_merge_attempt(
                "plan-merge-running",
                "plan-merge-running-phase",
                "shared workspace HEAD changed",
            )
            .expect("record merge attempt")
    );
    let (merge_team_id, merge_instance_id) = create_test_agent_team(
        &mut database,
        "chat-merge-running-merge",
        "merge-running-merge",
    );
    let merge_task_id = AgentTaskId::new("agent-task-merge-running-merge").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &merge_task_id,
            team_id: &merge_team_id,
            owner_instance_id: &merge_instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue merge task");
    let running = database
        .attach_plan_phase_merge_run(
            "plan-merge-running",
            "plan-merge-running-phase",
            "chat-merge-running-merge",
            &merge_team_id,
            &merge_task_id,
        )
        .expect("attach merge task");
    assert_eq!(running.status, "running");
    assert_eq!(
        running.active_phase_id.as_deref(),
        Some("plan-merge-running-phase")
    );
    assert_eq!(running.phases[0].status, "running");
    assert_eq!(
        running.phases[0].implementation_chat_id.as_deref(),
        Some("chat-merge-running-merge")
    );
    assert_eq!(
        running.phases[0].agent_task_id.as_deref(),
        Some("agent-task-merge-running-merge")
    );
    assert!(running.phases[0].commit_id.is_none());
    complete_test_agent_task(
        &mut database,
        &merge_team_id,
        &merge_task_id,
        "agent-attempt-merge-running-merge",
    );

    let completed = database
        .complete_plan_phase_by_id(
            "plan-merge-running",
            "plan-merge-running-phase",
            Some("shared-merge-commit"),
        )
        .expect("complete merge phase");
    assert_eq!(completed.status, "implemented");
    assert!(completed.active_phase_id.is_none());
    assert_eq!(completed.phases[0].status, "completed");
    assert_eq!(
        completed.phases[0].commit_id.as_deref(),
        Some("shared-merge-commit")
    );
    assert!(completed.phases[0].error_message.is_none());
    assert!(completed.shared_merge_commit_id.is_none());

    let merged = database
        .record_plan_shared_merge_commit("plan-merge-running", "shared-head-commit")
        .expect("record shared merge commit");
    assert_eq!(merged.status, "implemented");
    assert_eq!(
        merged.shared_merge_commit_id.as_deref(),
        Some("shared-head-commit")
    );
}

#[test]
fn plan_phase_merge_run_failure_marks_plan_failed_without_shared_merge_commit() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    let merge_task_id = attach_test_plan_merge_run(
        &mut database,
        "plan-merge-failed",
        "plan-merge-failed-phase",
        "merge-failed",
    );
    let failed = database
        .fail_plan_phase_run(&merge_task_id, "merge task failed")
        .expect("fail merge task")
        .expect("plan");

    assert_eq!(failed.status, "failed");
    assert!(failed.active_phase_id.is_none());
    assert_eq!(failed.error_message.as_deref(), Some("merge task failed"));
    assert!(failed.shared_merge_commit_id.is_none());
    assert_eq!(failed.phases[0].status, "failed");
    assert_eq!(
        failed.phases[0].error_message.as_deref(),
        Some("merge task failed")
    );
    assert!(failed.phases[0].commit_id.is_none());
}

#[test]
fn plan_phase_merge_run_cancel_or_interrupt_marks_plan_failed_without_shared_merge_commit() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    let merge_task_id = attach_test_plan_merge_run(
        &mut database,
        "plan-merge-cancelled",
        "plan-merge-cancelled-phase",
        "merge-cancelled",
    );
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &AgentTeamId::new("agent-team-merge-cancelled-merge").expect("merge team id"),
            task_id: &merge_task_id,
            expected_status: AgentTaskStatus::Queued,
            transition: AgentTaskTransition::Cancel,
            result_json: None,
            error_json: Some(r#"{"message":"merge task cancelled"}"#),
            interruption_reason: None,
        })
        .expect("cancel merge task");
    let cancelled = database
        .fail_plan_phase_run(&merge_task_id, "merge task cancelled")
        .expect("fail cancelled merge task")
        .expect("plan");
    assert_eq!(cancelled.status, "failed");
    assert!(cancelled.shared_merge_commit_id.is_none());
    assert_eq!(cancelled.phases[0].status, "failed");
    assert_eq!(
        cancelled.phases[0].error_message.as_deref(),
        Some("merge task cancelled")
    );

    let merge_task_id = attach_test_plan_merge_run(
        &mut database,
        "plan-merge-interrupted",
        "plan-merge-interrupted-phase",
        "merge-interrupted",
    );
    let merge_team_id = AgentTeamId::new("agent-team-merge-interrupted-merge").expect("team id");
    database
        .claim_runnable_agent_task(
            &merge_team_id,
            &merge_task_id,
            &AgentAttemptId::new("agent-attempt-merge-interrupted").expect("attempt id"),
        )
        .expect("claim merge task")
        .expect("claimed merge task");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &merge_team_id,
            task_id: &merge_task_id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Interrupt,
            result_json: None,
            error_json: Some(r#"{"message":"merge task interrupted"}"#),
            interruption_reason: Some("backend stopped"),
        })
        .expect("interrupt merge task");
    let interrupted = database
        .fail_plan_phase_run(&merge_task_id, "merge task interrupted")
        .expect("fail interrupted merge task")
        .expect("plan");
    assert_eq!(interrupted.status, "failed");
    assert!(interrupted.shared_merge_commit_id.is_none());
    assert_eq!(interrupted.phases[0].status, "failed");
    assert_eq!(
        interrupted.phases[0].error_message.as_deref(),
        Some("merge task interrupted")
    );
}

#[test]
fn fast_forward_failure_without_merge_dispatch_marks_failed_and_archive_preserves_error() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .create_plan(NewPlan {
            id: "plan-archive-failed-merge",
            title: "Archive failed merge",
            overview: "Completed means user archive, not shared merge.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "plan-archive-failed-merge-phase",
                title: "Phase one",
                summary: "Fails during merge.",
                steps: vec![NewPlanStep {
                    id: "plan-archive-failed-merge-step",
                    title: "Do work",
                    detail: "Complete the change.",
                    acceptance: vec!["failure remains visible".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan("plan-archive-failed-merge", "start")
        .expect("start plan");
    let failed = database
        .fail_plan_phase_by_id(
            "plan-archive-failed-merge",
            "plan-archive-failed-merge-phase",
            "fast-forward failed and merge task was not dispatched",
        )
        .expect("fail phase");
    assert_eq!(failed.status, "failed");
    assert!(failed.shared_merge_commit_id.is_none());

    let archived = database
        .transition_plan("plan-archive-failed-merge", "mark_complete")
        .expect("archive failed plan");
    assert_eq!(archived.status, "completed");
    assert!(archived.completed_by_user_at.is_some());
    assert_eq!(
        archived.error_message.as_deref(),
        Some("fast-forward failed and merge task was not dispatched")
    );
    assert_eq!(
        archived.phases[0].error_message.as_deref(),
        Some("fast-forward failed and merge task was not dispatched")
    );
    assert!(archived.shared_merge_commit_id.is_none());
}

#[test]
fn workspace_spec_jobs_redact_audit_json_fields() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    let job = database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-1",
            trigger_type: "manual_initial",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(0),
            input_summary_json: Some(
                r#"{"headers":{"authorization":"Bearer sk-test"},"safe":"ok","nested":{"api_key":"secret"}}"#,
            ),
        })
        .expect("spec job insert");
    let input: Value = serde_json::from_str(&job.input_summary_json).expect("input json");
    assert_eq!(input["headers"]["authorization"], "[REDACTED]");
    assert_eq!(input["nested"]["api_key"], "[REDACTED]");
    assert_eq!(input["safe"], "ok");

    database
        .update_workspace_spec_job_input_summary(
            "spec-job-1",
            r#"{"cookie":"session=secret","sourceFiles":[{"content":"password in source text stays as evidence"}]}"#,
        )
        .expect("update input");
    database
        .update_workspace_spec_job_prepared_input(
            "spec-job-1",
            7,
            r#"{"headers":{"authorization":"Bearer newer"},"sourceFiles":[{"content":"password in source text stays as evidence"}]}"#,
        )
        .expect("update prepared input");
    database
        .mark_workspace_spec_job_completed(
            "spec-job-1",
            Some(r#"{"response":{"password":"secret"},"contentBytes":12}"#),
        )
        .expect("complete job");
    let job = database
        .workspace_spec_job("spec-job-1")
        .expect("job lookup")
        .expect("spec job");
    assert_eq!(job.base_revision, Some(7));
    let input: Value = serde_json::from_str(&job.input_summary_json).expect("updated input json");
    assert_eq!(input["headers"]["authorization"], "[REDACTED]");
    assert_eq!(
        input["sourceFiles"][0]["content"],
        "password in source text stays as evidence"
    );
    let output: Value =
        serde_json::from_str(job.output_json.as_deref().expect("output json")).expect("output");
    assert_eq!(output["response"]["password"], "[REDACTED]");
    assert_eq!(output["contentBytes"], 12);
}

#[test]
fn workspace_spec_job_claim_is_fifo_and_single_owner() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    {
        let mut database = WorkspaceDatabase::open_or_create_ungated(workspace.path())
            .expect("workspace database");
        for id in ["spec-job-a", "spec-job-b", "spec-job-c"] {
            database
                .insert_workspace_spec_job(NewWorkspaceSpecJob {
                    id,
                    trigger_type: "manual_refresh",
                    chat_id: None,
                    run_id: None,
                    model_id: Some("model-1"),
                    base_revision: Some(1),
                    input_summary_json: None,
                })
                .expect("spec job insert");
        }
    }

    let workspace_path = Arc::new(workspace.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let workspace_path = workspace_path.clone();
        let barrier = barrier.clone();
        workers.push(thread::spawn(move || {
            let mut database = WorkspaceDatabase::open_or_create_ungated(workspace_path.as_path())
                .expect("worker db");
            barrier.wait();
            database
                .claim_next_workspace_spec_job()
                .expect("claim spec job")
                .map(|job| job.id)
        }));
    }
    barrier.wait();
    let claims = workers
        .into_iter()
        .map(|worker| worker.join().expect("claim worker"))
        .collect::<Vec<_>>();
    assert_eq!(claims.iter().filter(|claim| claim.is_some()).count(), 1);
    assert!(
        claims
            .iter()
            .any(|claim| claim.as_deref() == Some("spec-job-a"))
    );

    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    assert!(
        database
            .claim_next_workspace_spec_job()
            .expect("second claim")
            .is_none()
    );
    database
        .mark_workspace_spec_job_failed("spec-job-a", "expected failure")
        .expect("fail first job");
    assert_eq!(
        database
            .claim_next_workspace_spec_job()
            .expect("claim second job")
            .expect("second job")
            .id,
        "spec-job-b"
    );
    database
        .mark_workspace_spec_job_completed("spec-job-b", None)
        .expect("complete second job");
    drop(database);

    let mut reopened = WorkspaceDatabase::open_or_create_ungated(workspace.path())
        .expect("reopen workspace database");
    assert_eq!(
        reopened
            .claim_next_workspace_spec_job()
            .expect("claim after reopen")
            .expect("third job")
            .id,
        "spec-job-c"
    );
}

#[test]
fn workspace_spec_concurrent_claim_with_live_running_job_claims_none() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    {
        let mut database = WorkspaceDatabase::open_or_create_ungated(workspace.path())
            .expect("workspace database");
        for id in ["spec-job-live", "spec-job-queued"] {
            database
                .insert_workspace_spec_job(NewWorkspaceSpecJob {
                    id,
                    trigger_type: "manual_refresh",
                    chat_id: None,
                    run_id: None,
                    model_id: Some("model-1"),
                    base_revision: Some(1),
                    input_summary_json: None,
                })
                .expect("spec job insert");
        }
        database
            .mark_workspace_spec_job_running("spec-job-live")
            .expect("mark live running");
        let database_path = database.database_path().to_path_buf();
        drop(database);
        // Long-running but recently renewed lease: claim must still see a single owner.
        let connection = Connection::open(&database_path).expect("open db");
        connection
            .execute(
                "UPDATE workspace_spec_jobs
                 SET started_at = '2026-06-30T11:00:00Z',
                     lease_renewed_at = '2026-06-30T11:55:00Z'
                 WHERE id = 'spec-job-live'",
                [],
            )
            .expect("seed long-running live lease");
    }

    let workspace_path = Arc::new(workspace.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let workspace_path = workspace_path.clone();
        let barrier = barrier.clone();
        workers.push(thread::spawn(move || {
            let mut database = WorkspaceDatabase::open_or_create_ungated(workspace_path.as_path())
                .expect("worker db");
            barrier.wait();
            database
                .claim_next_workspace_spec_job()
                .expect("claim under live running")
                .map(|job| job.id)
        }));
    }
    barrier.wait();
    let claims = workers
        .into_iter()
        .map(|worker| worker.join().expect("claim worker"))
        .collect::<Vec<_>>();
    assert!(
        claims.iter().all(|claim| claim.is_none()),
        "live running job must block concurrent claim of queued jobs: {claims:?}"
    );

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    let live = database
        .workspace_spec_job("spec-job-live")
        .expect("live lookup")
        .expect("live job");
    assert_eq!(live.status, "running");
    assert_eq!(
        live.lease_renewed_at.as_deref(),
        Some("2026-06-30T11:55:00Z")
    );
    let queued = database
        .workspace_spec_job("spec-job-queued")
        .expect("queued lookup")
        .expect("queued job");
    assert_eq!(queued.status, "queued");
    assert_eq!(
        database
            .running_workspace_spec_job()
            .expect("running lookup")
            .expect("exactly one running")
            .id,
        "spec-job-live"
    );
}

#[test]
fn workspace_spec_job_has_retry_reflects_retry_children() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-failed",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(1),
            input_summary_json: None,
        })
        .expect("spec job insert");
    database
        .mark_workspace_spec_job_failed("spec-job-failed", "model timed out")
        .expect("fail job");
    assert!(
        !database
            .workspace_spec_job("spec-job-failed")
            .expect("source lookup")
            .expect("source job")
            .has_retry
    );

    let retry = database
        .retry_failed_workspace_spec_job("spec-job-failed", "spec-job-retry", Some("model-2"))
        .expect("retry job")
        .expect("created retry");
    assert!(!retry.has_retry);
    assert!(
        database
            .workspace_spec_job("spec-job-failed")
            .expect("source lookup")
            .expect("source job")
            .has_retry
    );
}

#[test]
fn chat_titles_by_ids_returns_existing_titles_only() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-one", "Alpha chat")
        .expect("insert chat one");
    database
        .insert_chat("chat-two", "Beta chat")
        .expect("insert chat two");

    let titles = database
        .chat_titles_by_ids(&[
            "chat-one".to_string(),
            "missing-chat".to_string(),
            "chat-two".to_string(),
        ])
        .expect("chat titles");
    assert_eq!(titles.len(), 2);
    assert_eq!(
        titles.get("chat-one").map(String::as_str),
        Some("Alpha chat")
    );
    assert_eq!(
        titles.get("chat-two").map(String::as_str),
        Some("Beta chat")
    );
    assert!(!titles.contains_key("missing-chat"));

    let empty = database.chat_titles_by_ids(&[]).expect("empty chat titles");
    assert!(empty.is_empty());
}

#[test]
fn chat_titles_by_ids_chunks_large_id_lists() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    // Cross the 500-id chunk boundary and include gaps / missing ids.
    let total = 1_050;
    let mut requested = Vec::with_capacity(total + 2);
    for index in 0..total {
        let id = format!("chat-chunk-{index}");
        if index % 3 != 0 {
            database
                .insert_chat(&id, &format!("Title {index}"))
                .expect("insert chat");
        }
        requested.push(id);
    }
    requested.push("missing-after-chunks".to_string());
    requested.push(requested[1].clone()); // duplicate within the list

    let titles = database
        .chat_titles_by_ids(&requested)
        .expect("chunked chat titles");

    let expected_present = (0..total).filter(|index| index % 3 != 0).count();
    assert_eq!(titles.len(), expected_present);
    assert_eq!(
        titles.get("chat-chunk-1").map(String::as_str),
        Some("Title 1")
    );
    assert_eq!(
        titles.get("chat-chunk-1049").map(String::as_str),
        Some("Title 1049")
    );
    assert!(!titles.contains_key("chat-chunk-0"));
    assert!(!titles.contains_key("missing-after-chunks"));
}

#[test]
fn delete_failed_workspace_spec_job_only_removes_failed_rows() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    // Mark running first while it is the only queued job (FIFO claim constraint).
    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-running",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(1),
            input_summary_json: None,
        })
        .expect("insert running job");
    assert!(
        database
            .mark_workspace_spec_job_running("spec-job-running")
            .expect("mark running")
    );

    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-queued",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(1),
            input_summary_json: None,
        })
        .expect("insert queued job");

    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-completed",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(1),
            input_summary_json: None,
        })
        .expect("insert completed job");
    database
        .mark_workspace_spec_job_completed("spec-job-completed", None)
        .expect("mark completed");

    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-skipped",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(1),
            input_summary_json: None,
        })
        .expect("insert skipped job");
    database
        .mark_workspace_spec_job_skipped("spec-job-skipped", "not needed")
        .expect("mark skipped");

    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-failed",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(1),
            input_summary_json: None,
        })
        .expect("insert failed job");
    database
        .mark_workspace_spec_job_failed("spec-job-failed", "provider failed")
        .expect("mark failed");

    assert!(
        database
            .delete_failed_workspace_spec_job("spec-job-failed")
            .expect("delete failed job")
    );
    assert!(
        database
            .workspace_spec_job("spec-job-failed")
            .expect("failed lookup")
            .is_none()
    );

    for id in [
        "spec-job-queued",
        "spec-job-running",
        "spec-job-completed",
        "spec-job-skipped",
    ] {
        assert!(
            !database
                .delete_failed_workspace_spec_job(id)
                .expect("reject non-failed delete"),
            "expected reject for {id}"
        );
        assert!(
            database.workspace_spec_job(id).expect("lookup").is_some(),
            "expected row retained for {id}"
        );
    }

    assert!(
        !database
            .delete_failed_workspace_spec_job("missing-spec-job")
            .expect("missing id")
    );
    assert_eq!(database.workspace_spec_job_count().expect("job count"), 4);
}

#[test]
fn delete_failed_workspace_spec_job_nulls_retry_of_parent() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-parent-failed",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(1),
            input_summary_json: None,
        })
        .expect("insert parent");
    database
        .mark_workspace_spec_job_failed("spec-job-parent-failed", "failed")
        .expect("mark parent failed");
    let retry = database
        .retry_failed_workspace_spec_job(
            "spec-job-parent-failed",
            "spec-job-retry-child",
            Some("model-2"),
        )
        .expect("retry")
        .expect("retry job");
    assert_eq!(retry.id, "spec-job-retry-child");

    let connection = Connection::open(database.database_path()).expect("open db");
    let retry_of_before: Option<String> = connection
        .query_row(
            "SELECT retry_of_job_id FROM workspace_spec_jobs WHERE id = ?1",
            params!["spec-job-retry-child"],
            |row| row.get(0),
        )
        .expect("retry_of before");
    assert_eq!(retry_of_before.as_deref(), Some("spec-job-parent-failed"));
    drop(connection);

    assert!(
        database
            .delete_failed_workspace_spec_job("spec-job-parent-failed")
            .expect("delete parent")
    );
    assert!(
        database
            .workspace_spec_job("spec-job-parent-failed")
            .expect("parent lookup")
            .is_none()
    );
    let retry_after = database
        .workspace_spec_job("spec-job-retry-child")
        .expect("retry lookup")
        .expect("retry retained");
    assert_eq!(retry_after.status, "queued");

    let connection = Connection::open(database.database_path()).expect("reopen db");
    let retry_of_after: Option<String> = connection
        .query_row(
            "SELECT retry_of_job_id FROM workspace_spec_jobs WHERE id = ?1",
            params!["spec-job-retry-child"],
            |row| row.get(0),
        )
        .expect("retry_of after");
    assert!(retry_of_after.is_none());
}

#[test]
fn workspace_spec_job_lease_renewed_at_initialized_on_claim_and_mark_running() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-claim",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(0),
            input_summary_json: None,
        })
        .expect("insert claim job");
    let claimed = database
        .claim_next_workspace_spec_job()
        .expect("claim")
        .expect("claimed job");
    assert_eq!(claimed.status, "running");
    assert!(claimed.started_at.is_some());
    assert_eq!(
        claimed.lease_renewed_at.as_deref(),
        claimed.started_at.as_deref(),
        "claim must seed lease without rewriting started_at later"
    );
    assert_eq!(
        claimed.lease_or_started_or_created_at(),
        claimed.lease_renewed_at.as_deref().expect("lease set")
    );

    database
        .mark_workspace_spec_job_completed("spec-job-claim", None)
        .expect("complete claimed");

    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-mark",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(0),
            input_summary_json: None,
        })
        .expect("insert mark job");
    assert!(
        database
            .mark_workspace_spec_job_running("spec-job-mark")
            .expect("mark running")
    );
    let marked = database
        .workspace_spec_job("spec-job-mark")
        .expect("lookup")
        .expect("marked job");
    assert_eq!(marked.status, "running");
    assert!(marked.started_at.is_some());
    assert_eq!(
        marked.lease_renewed_at.as_deref(),
        marked.started_at.as_deref()
    );

    let connection = Connection::open(database.database_path()).expect("open db");
    assert!(column_exists(
        &connection,
        "workspace_spec_jobs",
        "lease_renewed_at"
    ));
}

#[test]
fn touch_workspace_spec_job_lease_only_updates_running_rows() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-running-lease",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(0),
            input_summary_json: None,
        })
        .expect("insert running");
    assert!(
        database
            .mark_workspace_spec_job_running("spec-job-running-lease")
            .expect("mark running")
    );
    let before = database
        .workspace_spec_job("spec-job-running-lease")
        .expect("lookup before")
        .expect("running job");
    let started_at = before.started_at.clone().expect("started_at");
    let lease_before = before.lease_renewed_at.clone().expect("lease before");

    // Ensure touch advances lease without rewriting started_at.
    std::thread::sleep(Duration::from_millis(5));
    assert!(
        database
            .touch_workspace_spec_job_lease("spec-job-running-lease")
            .expect("touch running")
    );
    let after = database
        .workspace_spec_job("spec-job-running-lease")
        .expect("lookup after")
        .expect("still running");
    assert_eq!(after.started_at.as_deref(), Some(started_at.as_str()));
    let lease_after = after.lease_renewed_at.expect("lease after touch");
    assert_ne!(lease_after, lease_before);
    assert!(lease_after >= lease_before);

    database
        .mark_workspace_spec_job_completed("spec-job-running-lease", None)
        .expect("complete");
    let completed = database
        .workspace_spec_job("spec-job-running-lease")
        .expect("completed lookup")
        .expect("completed job");
    let lease_at_complete = completed.lease_renewed_at.clone();

    assert!(
        !database
            .touch_workspace_spec_job_lease("spec-job-running-lease")
            .expect("touch completed returns false")
    );
    let still_completed = database
        .workspace_spec_job("spec-job-running-lease")
        .expect("still completed lookup")
        .expect("completed job");
    assert_eq!(still_completed.status, "completed");
    assert_eq!(still_completed.lease_renewed_at, lease_at_complete);
    assert_eq!(
        still_completed.started_at.as_deref(),
        Some(started_at.as_str())
    );

    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-queued-lease",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(0),
            input_summary_json: None,
        })
        .expect("insert queued");
    assert!(
        !database
            .touch_workspace_spec_job_lease("spec-job-queued-lease")
            .expect("touch queued returns false")
    );
    let queued = database
        .workspace_spec_job("spec-job-queued-lease")
        .expect("queued lookup")
        .expect("queued job");
    assert_eq!(queued.status, "queued");
    assert!(queued.lease_renewed_at.is_none());

    assert!(
        !database
            .touch_workspace_spec_job_lease("missing-spec-job")
            .expect("touch missing returns false")
    );
}

#[test]
fn fail_stale_running_workspace_spec_job_is_atomic_and_lease_aware() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-stale-lease",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(0),
            input_summary_json: None,
        })
        .expect("insert job");
    assert!(
        database
            .mark_workspace_spec_job_running("spec-job-stale-lease")
            .expect("mark running")
    );
    let database_path = database.database_path().to_path_buf();
    drop(database);

    let connection = Connection::open(&database_path).expect("open db");
    connection
        .execute(
            "UPDATE workspace_spec_jobs
             SET started_at = '2026-06-30T11:00:00Z',
                 lease_renewed_at = '2026-06-30T11:29:00Z'
             WHERE id = 'spec-job-stale-lease'",
            [],
        )
        .expect("seed stale lease");
    drop(connection);

    let now = chrono::DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
        .expect("now")
        .with_timezone(&chrono::Utc);
    // 31 minutes without heartbeat is stale (> 30 minutes).
    let stale_after_ms = 30 * 60 * 1000;

    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    assert!(
        database
            .fail_stale_running_workspace_spec_job(
                "spec-job-stale-lease",
                now,
                stale_after_ms,
                "stale lease recovered"
            )
            .expect("fail stale")
    );
    let failed = database
        .workspace_spec_job("spec-job-stale-lease")
        .expect("lookup failed")
        .expect("job");
    assert_eq!(failed.status, "failed");
    assert_eq!(
        failed.error_message.as_deref(),
        Some("stale lease recovered")
    );

    // Re-seed a long-running job that receives a heartbeat after a stale snapshot.
    database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-renewed-before-fail",
            trigger_type: "manual_refresh",
            chat_id: None,
            run_id: None,
            model_id: Some("model-1"),
            base_revision: Some(0),
            input_summary_json: None,
        })
        .expect("insert renewed job");
    assert!(
        database
            .mark_workspace_spec_job_running("spec-job-renewed-before-fail")
            .expect("mark running")
    );
    drop(database);

    let connection = Connection::open(&database_path).expect("open db");
    connection
        .execute(
            "UPDATE workspace_spec_jobs
             SET started_at = '2026-06-30T11:00:00Z',
                 lease_renewed_at = '2026-06-30T11:29:00Z'
             WHERE id = 'spec-job-renewed-before-fail'",
            [],
        )
        .expect("seed stale then renew");
    drop(connection);

    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    // Heartbeat renews lease under a separate IMMEDIATE transaction before fail.
    assert!(
        database
            .touch_workspace_spec_job_lease("spec-job-renewed-before-fail")
            .expect("touch after stale snapshot")
    );
    assert!(
        !database
            .fail_stale_running_workspace_spec_job(
                "spec-job-renewed-before-fail",
                now,
                stale_after_ms,
                "must not kill after heartbeat"
            )
            .expect("fail after renew")
    );
    let live = database
        .workspace_spec_job("spec-job-renewed-before-fail")
        .expect("lookup live")
        .expect("job");
    assert_eq!(live.status, "running");
    assert_ne!(
        live.lease_renewed_at.as_deref(),
        Some("2026-06-30T11:29:00Z")
    );
    assert_eq!(live.started_at.as_deref(), Some("2026-06-30T11:00:00Z"));

    // Exact 30-minute boundary is not stale.
    drop(database);
    let connection = Connection::open(&database_path).expect("open db");
    connection
        .execute(
            "UPDATE workspace_spec_jobs
             SET status = 'running',
                 error_message = NULL,
                 completed_at = NULL,
                 started_at = '2026-06-30T11:00:00Z',
                 lease_renewed_at = '2026-06-30T11:30:00Z'
             WHERE id = 'spec-job-renewed-before-fail'",
            [],
        )
        .expect("seed exact boundary");
    drop(connection);
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    assert!(
        !database
            .fail_stale_running_workspace_spec_job(
                "spec-job-renewed-before-fail",
                now,
                stale_after_ms,
                "exact boundary"
            )
            .expect("exact boundary not stale")
    );
    assert_eq!(
        database
            .workspace_spec_job("spec-job-renewed-before-fail")
            .expect("lookup")
            .expect("job")
            .status,
        "running"
    );

    // Terminal jobs cannot be failed as stale.
    database
        .mark_workspace_spec_job_completed("spec-job-renewed-before-fail", None)
        .expect("complete");
    assert!(
        !database
            .fail_stale_running_workspace_spec_job(
                "spec-job-renewed-before-fail",
                now,
                stale_after_ms,
                "terminal"
            )
            .expect("fail completed returns false")
    );
}

#[test]
fn fail_stale_running_workspace_spec_job_races_with_touch_safely() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    {
        let mut database = WorkspaceDatabase::open_or_create_ungated(workspace.path())
            .expect("workspace database");
        database
            .insert_workspace_spec_job(NewWorkspaceSpecJob {
                id: "spec-job-race",
                trigger_type: "manual_refresh",
                chat_id: None,
                run_id: None,
                model_id: Some("model-1"),
                base_revision: Some(0),
                input_summary_json: None,
            })
            .expect("insert");
        database
            .mark_workspace_spec_job_running("spec-job-race")
            .expect("running");
        let database_path = database.database_path().to_path_buf();
        drop(database);
        let connection = Connection::open(&database_path).expect("open db");
        connection
            .execute(
                "UPDATE workspace_spec_jobs
                 SET started_at = '2026-06-30T11:00:00Z',
                     lease_renewed_at = '2026-06-30T11:29:00Z'
                 WHERE id = 'spec-job-race'",
                [],
            )
            .expect("seed stale");
    }

    let now = chrono::DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
        .expect("now")
        .with_timezone(&chrono::Utc);
    let stale_after_ms = 30 * 60 * 1000i64;
    let workspace_path = Arc::new(workspace.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(3));

    let touch_path = workspace_path.clone();
    let touch_barrier = barrier.clone();
    let touch_worker = thread::spawn(move || {
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(touch_path.as_path()).expect("touch db");
        touch_barrier.wait();
        database
            .touch_workspace_spec_job_lease("spec-job-race")
            .expect("touch")
    });

    let fail_path = workspace_path.clone();
    let fail_barrier = barrier.clone();
    let fail_worker = thread::spawn(move || {
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(fail_path.as_path()).expect("fail db");
        fail_barrier.wait();
        database
            .fail_stale_running_workspace_spec_job(
                "spec-job-race",
                now,
                stale_after_ms,
                "race recovery",
            )
            .expect("fail stale")
    });

    barrier.wait();
    let touched = touch_worker.join().expect("touch join");
    let failed = fail_worker.join().expect("fail join");

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    let job = database
        .workspace_spec_job("spec-job-race")
        .expect("lookup")
        .expect("job");
    match (touched, failed, job.status.as_str()) {
        // Heartbeat won: job stays running with a renewed lease.
        (true, false, "running") => {
            assert_ne!(
                job.lease_renewed_at.as_deref(),
                Some("2026-06-30T11:29:00Z"),
                "renewed lease must advance past the stale snapshot"
            );
        }
        // Recovery won before heartbeat: job is failed and touch saw non-running.
        (false, true, "failed") => {
            assert_eq!(job.error_message.as_deref(), Some("race recovery"));
        }
        other => panic!(
            "unexpected race outcome: touched={} failed={} status={} other={other:?}",
            other.0, other.1, other.2
        ),
    }
    assert_eq!(job.started_at.as_deref(), Some("2026-06-30T11:00:00Z"));
}

#[test]
fn workspace_spec_job_lease_migration_leaves_legacy_null_for_fallback() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("parent")).expect("foco dir");

    {
        let connection = Connection::open(&database_path).expect("create legacy db");
        connection
            .execute_batch(
                r#"
            PRAGMA journal_mode = WAL;
            CREATE TABLE workspace_metadata (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE chats (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived_at TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}'
            );
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
                id TEXT PRIMARY KEY NOT NULL,
                trigger_type TEXT NOT NULL,
                status TEXT NOT NULL,
                run_id TEXT,
                model_id TEXT,
                base_revision INTEGER,
                input_summary_json TEXT NOT NULL DEFAULT '{}',
                output_json TEXT,
                error_message TEXT,
                created_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                retry_of_job_id TEXT,
                chat_id TEXT
            );
            CREATE TABLE chat_spec_snapshots (
                chat_id TEXT PRIMARY KEY NOT NULL,
                spec_revision INTEGER NOT NULL,
                content_markdown TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            INSERT INTO workspace_spec_jobs (
                id, trigger_type, status, input_summary_json, created_at, started_at
            ) VALUES (
                'legacy-running',
                'manual_refresh',
                'running',
                '{}',
                '2026-06-30T10:00:00Z',
                '2026-06-30T10:05:00Z'
            );
            INSERT INTO workspace_spec_jobs (
                id, trigger_type, status, input_summary_json, created_at
            ) VALUES (
                'legacy-queued',
                'manual_refresh',
                'queued',
                '{}',
                '2026-06-30T10:10:00Z'
            );
            PRAGMA user_version = 39;
            "#,
            )
            .expect("seed legacy schema 39");
    }

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrate to 40");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );

    let running = database
        .workspace_spec_job("legacy-running")
        .expect("running lookup")
        .expect("running job");
    assert_eq!(running.status, "running");
    assert!(running.lease_renewed_at.is_none());
    assert_eq!(running.started_at.as_deref(), Some("2026-06-30T10:05:00Z"));
    assert_eq!(
        running.lease_or_started_or_created_at(),
        "2026-06-30T10:05:00Z"
    );

    let queued = database
        .workspace_spec_job("legacy-queued")
        .expect("queued lookup")
        .expect("queued job");
    assert_eq!(queued.status, "queued");
    assert!(queued.lease_renewed_at.is_none());
    assert_eq!(
        queued.lease_or_started_or_created_at(),
        "2026-06-30T10:10:00Z"
    );

    let connection = Connection::open(database.database_path()).expect("open db");
    assert!(column_exists(
        &connection,
        "workspace_spec_jobs",
        "lease_renewed_at"
    ));
}

#[test]
fn delete_chat_cascades_spec_snapshot_but_preserves_workspace_spec() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "Spec snapshot chat")
        .expect("chat insert");
    database
        .upsert_workspace_spec_settings(true, true)
        .expect("spec settings");
    database
        .update_workspace_spec_content(0, "# Project Spec\n\nWorkspace spec survives.")
        .expect("workspace spec")
        .expect("workspace spec saved");
    database
        .insert_chat_spec_snapshot("chat-1", 1, "# Project Spec\n\nChat snapshot")
        .expect("snapshot insert");

    assert!(database.delete_chat("chat-1").expect("chat delete"));
    assert!(
        database
            .chat_spec_snapshot("chat-1")
            .expect("snapshot lookup")
            .is_none()
    );
    let spec = database
        .workspace_spec()
        .expect("workspace spec lookup")
        .expect("workspace spec");
    assert_eq!(spec.revision, 1);
    assert!(spec.content_markdown.contains("Workspace spec survives"));
}

#[test]
fn workspace_connections_wait_for_concurrent_writer_lock() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let workspace_path = workspace.path().to_path_buf();
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(&workspace_path).expect("workspace database");
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

    let (started_tx, started_rx) = mpsc::channel();
    let writer = thread::spawn(move || {
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(&workspace_path).expect("writer database");
        started_tx.send(()).expect("writer start signal");
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

    started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("writer should reach locked insert");
    thread::yield_now();
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
fn run_events_for_run_after_returns_only_later_sequences() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Incremental run events")
        .expect("chat insert");
    for sequence in 0..5 {
        database
            .insert_run_event(NewRunEvent {
                id: &format!("event-{sequence}"),
                chat_id: "chat-1",
                run_id: "run-1",
                sequence,
                event_type: "text_delta",
                payload_json: &format!(r#"{{"sequence":{sequence}}}"#),
            })
            .expect("run event insert");
    }

    let sequences = database
        .run_events_for_run_after("run-1", 2, 10)
        .expect("incremental events")
        .into_iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>();

    assert_eq!(sequences, vec![3, 4]);
}

#[test]
fn run_events_for_run_after_respects_batch_limit() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Bounded run events")
        .expect("chat insert");
    for sequence in 0..3 {
        database
            .insert_run_event(NewRunEvent {
                id: &format!("event-{sequence}"),
                chat_id: "chat-1",
                run_id: "run-1",
                sequence,
                event_type: "text_delta",
                payload_json: &format!(r#"{{"sequence":{sequence}}}"#),
            })
            .expect("run event insert");
    }

    let events = database
        .run_events_for_run_after("run-1", -1, 2)
        .expect("bounded incremental events");

    assert_eq!(events.len(), 2);
}

#[test]
fn counts_runtime_tool_state_compression_events_for_chat() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Compression events")
        .expect("chat insert");
    database
        .insert_chat("chat-2", "Other chat")
        .expect("other chat insert");

    for (sequence, (id, chat_id, kind)) in [
        ("event-1", "chat-1", "runtimeToolState"),
        ("event-2", "chat-1", "rule"),
        ("event-3", "chat-2", "runtimeToolState"),
    ]
    .into_iter()
    .enumerate()
    {
        database
            .insert_run_event(NewRunEvent {
                id,
                chat_id,
                run_id: "run-1",
                sequence: sequence as i64,
                event_type: "context_compression",
                payload_json: &format!(r#"{{"type":"contextCompression","kind":"{kind}"}}"#),
            })
            .expect("run event insert");
    }

    assert_eq!(
        database
            .runtime_tool_state_compression_count_for_chat("chat-1")
            .expect("runtime compression count"),
        1
    );
}

#[test]
fn infers_runtime_tool_state_compression_from_saved_request_bodies() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-1", "legacy-compression");
    let task_1 = AgentTaskId::new("agent-task-1").expect("task id");
    let task_2 = AgentTaskId::new("agent-task-2").expect("task id");
    for task_id in [&task_1, &task_2] {
        database
            .enqueue_agent_task(NewAgentTask {
                id: task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("agent task enqueue");
    }

    for (id, task_id, body) in [
        (
            "request-1",
            &task_1,
            r#"{"format":"provider_request_v1","version":1,"method":"POST","url":"https://example.test","headers":{},"body":"Runtime tool-state compression snapshot Runtime tool-state compression snapshot"}"#,
        ),
        (
            "request-2",
            &task_1,
            r#"{"format":"provider_request_v1","version":1,"method":"POST","url":"https://example.test","headers":{},"body":"Runtime tool-state compression snapshot"}"#,
        ),
        (
            "request-3",
            &task_2,
            r#"{"format":"provider_request_v1","version":1,"method":"POST","url":"https://example.test","headers":{},"body":"Runtime tool-state compression snapshot"}"#,
        ),
    ] {
        database
            .insert_llm_request(NewLlmRequest {
                id,
                workspace_id: "workspace-1",
                chat_id: Some("chat-1"),
                request_kind: "chat completion",
                agent_team_id: Some(&team_id),
                agent_instance_id: None,
                agent_task_id: Some(task_id),
                agent_attempt_id: None,
                provider_id: "openai",
                model_id: "gpt-test",
                thinking_level: None,
                request_started_at: "2026-06-27T00:00:00Z",
                first_token_at: None,
                completed_at: None,
                input_tokens: None,
                output_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
                first_token_latency_ms: None,
                total_latency_ms: None,
                status_code: None,
                final_state: "succeeded",
                request_body_json: Some(body),
                response_body_json: None,
            })
            .expect("llm request insert");
    }

    assert_eq!(
        database
            .runtime_tool_state_compression_count_for_chat("chat-1")
            .expect("runtime compression count"),
        3
    );
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
            location: foco_store::config::WorkspaceLocation::Local,
            pinned: false,
            terminal_shell: foco_store::config::DEFAULT_TERMINAL_SHELL.to_string(),
            common_commands: Vec::new(),
        },
        WorkspaceConfig {
            id: "second".to_string(),
            name: "Second".to_string(),
            path: second.path().to_path_buf(),
            location: foco_store::config::WorkspaceLocation::Local,
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

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");

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
fn prunes_old_workspace_database_backups() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let backup_dir = workspace.path().join(".foco").join("backups");
    fs::create_dir_all(&backup_dir).expect("backup directory");

    for timestamp in [
        "20260101T000000000000000Z",
        "20260102T000000000000000Z",
        "20260103T000000000000000Z",
        "20260104T000000000000000Z",
        "20260105T000000000000000Z",
    ] {
        fs::write(
            backup_dir.join(format!("foco-v1-{timestamp}.sqlite")),
            b"backup",
        )
        .expect("backup file");
    }
    fs::write(backup_dir.join("notes.txt"), b"keep").expect("non-backup file");

    let deleted = prune_workspace_database_backups(workspace.path()).expect("prune backups");

    assert_eq!(deleted, 2);
    let mut remaining = fs::read_dir(&backup_dir)
        .expect("backup entries")
        .map(|entry| {
            entry
                .expect("backup entry")
                .file_name()
                .into_string()
                .expect("utf8 filename")
        })
        .collect::<Vec<_>>();
    remaining.sort();
    assert_eq!(
        remaining,
        vec![
            "foco-v1-20260103T000000000000000Z.sqlite".to_string(),
            "foco-v1-20260104T000000000000000Z.sqlite".to_string(),
            "foco-v1-20260105T000000000000000Z.sqlite".to_string(),
            "notes.txt".to_string(),
        ]
    );
}

#[test]
fn migrates_v17_workspace_spec_tables_and_creates_backup() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());

    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("v17 database");
    connection
        .execute_batch(
            "CREATE TABLE chats (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived_at TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}'
             );
             PRAGMA user_version = 17;",
        )
        .expect("v17 schema");
    add_workspace_memory_tables(&connection);
    add_workspace_memory_dream_tables(&connection);
    add_memory_reference_tables(&connection);
    add_workspace_agent_plan_reference_tables(&connection);
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );

    let connection = Connection::open(database.database_path()).expect("open migrated database");
    assert!(table_exists(&connection, "workspace_specs"));
    assert!(table_exists(&connection, "workspace_spec_jobs"));
    assert!(table_exists(&connection, "chat_spec_snapshots"));
    let backups = fs::read_dir(workspace.path().join(".foco").join("backups"))
        .expect("backup directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("backup entries");
    assert_eq!(backups.len(), 1);
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
    add_workspace_memory_tables(&connection);
    connection
        .execute(
            "INSERT INTO task_graphs (chat_id, graph_json, created_at, updated_at)
             VALUES ('chat-1', ?1, '2026-06-10T00:00:00Z', '2026-06-10T00:00:00Z')",
            params![legacy_tasks],
        )
        .expect("legacy todo graph row");
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
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
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

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
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

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
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

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
fn mark_chat_queued_run_started_rebuilds_missing_queued_run_metadata() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_chat_with_metadata(
            "chat-missing-queued",
            "Missing queued run",
            r#"{"source":"plan_phase","planId":"plan-1","phaseId":"phase-1"}"#,
        )
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "user-missing-queued",
            chat_id: "chat-missing-queued",
            role: "user",
            content: "implement phase",
            sequence: 0,
            metadata_json: Some(
                r#"{"source":"plan_phase","modelId":"model","providerId":"provider"}"#,
            ),
        })
        .expect("message insert");

    database
        .mark_chat_queued_run_started(
            "chat-missing-queued",
            "user-missing-queued",
            "assistant-missing-queued",
            1,
        )
        .expect("rebuild missing queued run");

    let chat_metadata: Value = serde_json::from_str(
        &database
            .chat("chat-missing-queued")
            .expect("chat read")
            .expect("chat")
            .metadata_json,
    )
    .expect("chat metadata json");
    let message_metadata: Value = serde_json::from_str(
        &database
            .message("user-missing-queued")
            .expect("message read")
            .expect("message")
            .metadata_json,
    )
    .expect("message metadata json");

    assert_eq!(chat_metadata["source"], "plan_phase");
    assert_eq!(chat_metadata["queuedRun"]["status"], "running");
    assert_eq!(
        chat_metadata["queuedRun"]["userMessageId"],
        "user-missing-queued"
    );
    assert_eq!(
        chat_metadata["queuedRun"]["assistantMessageId"],
        "assistant-missing-queued"
    );
    assert_eq!(chat_metadata["queuedRun"]["assistantSequence"], 1);
    assert_eq!(message_metadata["source"], "plan_phase");
    assert_eq!(message_metadata["queuedRun"]["status"], "running");
    assert_eq!(
        message_metadata["queuedRun"]["assistantMessageId"],
        "assistant-missing-queued"
    );
    assert_eq!(message_metadata["queuedRun"]["assistantSequence"], 1);
}

#[test]
fn set_chat_queued_run_adds_run_to_existing_chat_metadata() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat_with_metadata("chat-existing", "Existing chat", r#"{"kind":"normal"}"#)
        .expect("chat insert");

    database
        .set_chat_queued_run(
            "chat-existing",
            r#"{"status":"queued","userMessageId":"user-2","assistantMessageId":"assistant-2","assistantSequence":3,"modelId":"model"}"#,
        )
        .expect("set queued run");

    let metadata: Value = serde_json::from_str(
        &database
            .chat("chat-existing")
            .expect("chat read")
            .expect("chat")
            .metadata_json,
    )
    .expect("chat metadata json");
    assert_eq!(metadata["kind"], "normal");
    assert_eq!(metadata["queuedRun"]["userMessageId"], "user-2");
    assert_eq!(metadata["queuedRun"]["assistantMessageId"], "assistant-2");
}

#[test]
fn repository_helpers_round_trip_todo_graphs() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

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
fn scheduled_task_records_round_trip_and_list_runs() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    let task = database
        .insert_scheduled_task(NewScheduledTask {
            id: "scheduled-task-1",
            title: "Daily workspace summary",
            description: Some("Summarize the current workspace."),
            schedule_json: r#"{"type":"one_shot_at","run_at":"2026-06-22T10:00:00Z"}"#,
            action_json: r#"{"type":"agent_prompt","prompt":"Summarize changes"}"#,
            status: "enabled",
            next_run_at: Some("2026-06-22T10:00:00Z"),
            metadata_json: Some(
                r#"{"workspaceId":"workspace-1","concurrencyPolicy":"skip_if_running"}"#,
            ),
        })
        .expect("scheduled task insert");
    assert_eq!(task.id, "scheduled-task-1");
    assert_eq!(task.status, "enabled");
    assert_eq!(task.last_run_at, None);

    let paused = database
        .update_scheduled_task(ScheduledTaskUpdate {
            id: "scheduled-task-1",
            title: "Daily workspace summary",
            description: task.description.as_deref(),
            schedule_json: &task.schedule_json,
            action_json: &task.action_json,
            status: "paused",
            next_run_at: None,
            last_run_at: Some("2026-06-22T10:00:00Z"),
            metadata_json: &task.metadata_json,
        })
        .expect("scheduled task pause");
    assert_eq!(paused.status, "paused");
    assert_eq!(paused.next_run_at, None);

    let paused_tasks = database
        .scheduled_tasks(Some("paused"))
        .expect("paused scheduled tasks");
    assert_eq!(paused_tasks.len(), 1);
    assert_eq!(paused_tasks[0].id, "scheduled-task-1");

    let resumed = database
        .update_scheduled_task(ScheduledTaskUpdate {
            id: "scheduled-task-1",
            title: &paused.title,
            description: paused.description.as_deref(),
            schedule_json: &paused.schedule_json,
            action_json: &paused.action_json,
            status: "enabled",
            next_run_at: Some("2026-06-23T10:00:00Z"),
            last_run_at: paused.last_run_at.as_deref(),
            metadata_json: &paused.metadata_json,
        })
        .expect("scheduled task resume");
    assert_eq!(resumed.status, "enabled");
    assert_eq!(resumed.next_run_at.as_deref(), Some("2026-06-23T10:00:00Z"));
    assert_eq!(
        database
            .next_enabled_scheduled_task_run_at()
            .expect("next scheduled run"),
        Some("2026-06-23T10:00:00Z".to_string())
    );
    assert_eq!(
        database
            .scheduled_tasks(None)
            .expect("all scheduled tasks")
            .len(),
        1
    );

    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-scheduled-run", "scheduled-run");
    database
        .insert_message(NewMessage {
            id: "message-scheduled-user",
            chat_id: "chat-scheduled-run",
            role: "user",
            content: "Summarize changes",
            sequence: 0,
            metadata_json: Some("{}"),
        })
        .expect("scheduled user message insert");
    database
        .insert_message(NewMessage {
            id: "message-scheduled-assistant",
            chat_id: "chat-scheduled-run",
            role: "assistant",
            content: "",
            sequence: 1,
            metadata_json: Some("{}"),
        })
        .expect("scheduled assistant message insert");

    let agent_task_id = AgentTaskId::new("agent-task-scheduled-run").expect("agent task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &agent_task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: r#"{"goal":"Summarize changes"}"#,
        })
        .expect("agent task enqueue");
    let attempt_id = AgentAttemptId::new("agent-attempt-scheduled-run").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &agent_task_id, &attempt_id)
        .expect("agent task claim")
        .expect("claimed agent task");

    let run = database
        .insert_scheduled_task_run(NewScheduledTaskRun {
            id: "scheduled-run-1",
            task_id: "scheduled-task-1",
            trigger_reason: "manual",
            status: "queued",
            scheduled_at: "2026-06-22T10:00:00Z",
            queued_at: Some("2026-06-22T10:00:01Z"),
            started_at: None,
            completed_at: None,
            chat_id: Some("chat-scheduled-run"),
            user_message_id: Some("message-scheduled-user"),
            assistant_message_id: Some("message-scheduled-assistant"),
            agent_team_id: Some(&team_id),
            agent_task_id: Some(&agent_task_id),
            agent_attempt_id: None,
            active_run_id: Some("agent-task-scheduled-run"),
            error_message: None,
            output_summary: None,
            metadata_json: Some(r#"{"triggeredBy":"test"}"#),
        })
        .expect("scheduled task run insert");
    assert_eq!(run.status, "queued");
    assert_eq!(run.chat_id.as_deref(), Some("chat-scheduled-run"));
    assert_eq!(run.agent_task_id.as_ref(), Some(&agent_task_id));

    let completed = database
        .update_scheduled_task_run(ScheduledTaskRunUpdate {
            id: "scheduled-run-1",
            status: "succeeded",
            queued_at: run.queued_at.as_deref(),
            started_at: Some("2026-06-22T10:00:02Z"),
            completed_at: Some("2026-06-22T10:00:30Z"),
            chat_id: run.chat_id.as_deref(),
            user_message_id: run.user_message_id.as_deref(),
            assistant_message_id: run.assistant_message_id.as_deref(),
            agent_team_id: run.agent_team_id.as_ref(),
            agent_task_id: run.agent_task_id.as_ref(),
            agent_attempt_id: Some(&attempt_id),
            active_run_id: run.active_run_id.as_deref(),
            error_message: None,
            output_summary: Some("Workspace summarized."),
            metadata_json: &run.metadata_json,
        })
        .expect("scheduled task run update");
    assert_eq!(completed.status, "succeeded");
    assert_eq!(completed.agent_attempt_id.as_ref(), Some(&attempt_id));
    assert_eq!(
        completed.output_summary.as_deref(),
        Some("Workspace summarized.")
    );

    database
        .insert_scheduled_task_run(NewScheduledTaskRun {
            id: "scheduled-run-2",
            task_id: "scheduled-task-1",
            trigger_reason: "scheduled",
            status: "failed",
            scheduled_at: "2026-06-23T10:00:00Z",
            queued_at: None,
            started_at: None,
            completed_at: Some("2026-06-23T10:00:01Z"),
            chat_id: None,
            user_message_id: None,
            assistant_message_id: None,
            agent_team_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            active_run_id: None,
            error_message: Some("dispatch failed"),
            output_summary: None,
            metadata_json: None,
        })
        .expect("second scheduled task run insert");

    let runs = database
        .scheduled_task_runs_for_task("scheduled-task-1")
        .expect("scheduled task runs");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].id, "scheduled-run-2");
    assert_eq!(runs[1].id, "scheduled-run-1");
    let agent_runs = database
        .scheduled_task_runs_for_agent_task(&agent_task_id)
        .expect("scheduled task runs for agent task");
    assert_eq!(agent_runs.len(), 1);
    assert_eq!(agent_runs[0].id, "scheduled-run-1");

    database
        .insert_llm_request(NewLlmRequest {
            id: "request-scheduled-1",
            workspace_id: "workspace-1",
            chat_id: Some("chat-scheduled-run"),
            request_kind: "chat completion",
            agent_team_id: Some(&team_id),
            agent_instance_id: Some(&instance_id),
            agent_task_id: Some(&agent_task_id),
            agent_attempt_id: Some(&attempt_id),
            provider_id: "openai-responses",
            model_id: "gpt-scheduled",
            thinking_level: None,
            request_started_at: "2026-06-22T10:00:02Z",
            first_token_at: Some("2026-06-22T10:00:03Z"),
            completed_at: Some("2026-06-22T10:00:04Z"),
            input_tokens: Some(100),
            output_tokens: Some(20),
            cache_read_tokens: Some(5),
            cache_write_tokens: Some(7),
            reasoning_tokens: None,
            first_token_latency_ms: Some(1000),
            total_latency_ms: Some(2000),
            status_code: Some(200),
            final_state: "succeeded",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("scheduled llm request insert");
    database
        .insert_llm_request(NewLlmRequest {
            id: "request-scheduled-2",
            workspace_id: "workspace-1",
            chat_id: Some("chat-scheduled-run"),
            request_kind: "chat completion",
            agent_team_id: Some(&team_id),
            agent_instance_id: Some(&instance_id),
            agent_task_id: Some(&agent_task_id),
            agent_attempt_id: Some(&attempt_id),
            provider_id: "openai-responses",
            model_id: "gpt-scheduled",
            thinking_level: None,
            request_started_at: "2026-06-22T10:00:05Z",
            first_token_at: None,
            completed_at: Some("2026-06-22T10:00:06Z"),
            input_tokens: Some(10),
            output_tokens: Some(0),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(500),
            final_state: "failed",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("failed scheduled llm request insert");
    database
        .insert_llm_request(NewLlmRequest {
            id: "request-unrelated",
            workspace_id: "workspace-1",
            chat_id: Some("chat-scheduled-run"),
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai-responses",
            model_id: "gpt-scheduled",
            thinking_level: None,
            request_started_at: "2026-06-22T10:00:07Z",
            first_token_at: None,
            completed_at: Some("2026-06-22T10:00:08Z"),
            input_tokens: Some(999),
            output_tokens: Some(999),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: Some(999),
            status_code: Some(200),
            final_state: "succeeded",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("unrelated llm request insert");

    let usage = database
        .scheduled_task_usage_summary("scheduled-task-1")
        .expect("scheduled task usage summary");
    assert_eq!(usage.total_requests, 2);
    assert_eq!(usage.failed_requests, 1);
    assert_eq!(usage.total_input_tokens, 110);
    assert_eq!(usage.total_output_tokens, 20);
    assert_eq!(usage.total_cache_read_tokens, 5);
    assert_eq!(usage.total_cache_write_tokens, 7);
    assert_eq!(usage.total_tokens, 130);
    assert_eq!(usage.latency_count, 1);
    assert_eq!(usage.latency_sum, 2000);
    let usage_by_task = database
        .scheduled_task_usage_summaries(&[
            "scheduled-task-1".to_string(),
            "missing-task".to_string(),
        ])
        .expect("scheduled task usage summaries");
    assert_eq!(usage_by_task["scheduled-task-1"].total_requests, 2);
    assert!(!usage_by_task.contains_key("missing-task"));

    let page = database
        .scheduled_tasks_page(ScheduledTaskListFilter {
            status: Some("enabled"),
            search: Some("summarize"),
            limit: 1,
            offset: 0,
        })
        .expect("scheduled task page");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].id, "scheduled-task-1");
    assert_eq!(
        database
            .scheduled_task_count(ScheduledTaskListFilter {
                status: Some("enabled"),
                search: Some("summarize"),
                limit: 1,
                offset: 0,
            })
            .expect("scheduled task count"),
        1
    );
    let status_counts = database
        .scheduled_task_status_counts(Some("summarize"))
        .expect("scheduled task status counts");
    assert_eq!(status_counts.len(), 1);
    assert_eq!(status_counts[0].status, "enabled");
    assert_eq!(status_counts[0].count, 1);
    assert_eq!(
        database
            .scheduled_task_run_count("scheduled-task-1")
            .expect("scheduled task run count"),
        2
    );
    let run_page = database
        .scheduled_task_runs_for_task_page("scheduled-task-1", 1, 1)
        .expect("scheduled task run page");
    assert_eq!(run_page.len(), 1);
    assert_eq!(run_page[0].id, "scheduled-run-1");
    assert!(
        database
            .delete_scheduled_task("scheduled-task-1")
            .expect("scheduled task delete")
    );
    assert!(
        database
            .scheduled_task("scheduled-task-1")
            .expect("deleted scheduled task lookup")
            .is_none()
    );
    assert!(
        database
            .scheduled_task_runs_for_task("scheduled-task-1")
            .expect("deleted scheduled task runs")
            .is_empty()
    );
}

#[test]
fn claims_due_scheduled_task_run_once_and_updates_task_state() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_scheduled_task(NewScheduledTask {
            id: "scheduled-task-due",
            title: "Due task",
            description: None,
            schedule_json: r#"{"type":"one_shot_at","run_at":"2026-06-22T10:00:00Z"}"#,
            action_json: r#"{"type":"agent_prompt","prompt":"Run"}"#,
            status: "enabled",
            next_run_at: Some("2026-06-22T10:00:00Z"),
            metadata_json: Some("{}"),
        })
        .expect("scheduled task insert");

    let run = database
        .claim_due_scheduled_task_run(ScheduledTaskDueRunClaim {
            task_id: "scheduled-task-due",
            expected_next_run_at: "2026-06-22T10:00:00Z",
            run_id: "scheduled-run-due",
            trigger_reason: "scheduled",
            run_status: "pending",
            scheduled_at: "2026-06-22T10:00:00Z",
            completed_at: None,
            error_message: None,
            task_status: "completed",
            task_next_run_at: None,
            task_last_run_at: "2026-06-22T10:00:01Z",
            metadata_json: None,
        })
        .expect("claim due scheduled task")
        .expect("due task claimed");
    assert_eq!(run.status, "pending");

    let task = database
        .scheduled_task("scheduled-task-due")
        .expect("scheduled task lookup")
        .expect("scheduled task");
    assert_eq!(task.status, "completed");
    assert_eq!(task.next_run_at, None);
    assert_eq!(task.last_run_at.as_deref(), Some("2026-06-22T10:00:01Z"));

    assert!(
        database
            .claim_due_scheduled_task_run(ScheduledTaskDueRunClaim {
                task_id: "scheduled-task-due",
                expected_next_run_at: "2026-06-22T10:00:00Z",
                run_id: "scheduled-run-duplicate",
                trigger_reason: "scheduled",
                run_status: "pending",
                scheduled_at: "2026-06-22T10:00:00Z",
                completed_at: None,
                error_message: None,
                task_status: "completed",
                task_next_run_at: None,
                task_last_run_at: "2026-06-22T10:00:02Z",
                metadata_json: None,
            })
            .expect("duplicate claim")
            .is_none()
    );
}

#[test]
fn scheduled_task_active_runs_and_retention_policy() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_scheduled_task(NewScheduledTask {
            id: "scheduled-task-retention",
            title: "Retention task",
            description: None,
            schedule_json: r#"{"type":"interval","every_seconds":60}"#,
            action_json: r#"{"type":"agent_prompt","prompt":"Run"}"#,
            status: "enabled",
            next_run_at: Some("2026-06-22T10:00:00Z"),
            metadata_json: Some("{}"),
        })
        .expect("scheduled task insert");

    for (id, status, completed_at) in [
        (
            "scheduled-run-old",
            "succeeded",
            Some("2026-01-01T00:00:00Z"),
        ),
        (
            "scheduled-run-recent",
            "failed",
            Some("2026-06-22T10:00:00Z"),
        ),
        ("scheduled-run-pending", "pending", None),
        ("scheduled-run-queued", "queued", None),
    ] {
        database
            .insert_scheduled_task_run(NewScheduledTaskRun {
                id,
                task_id: "scheduled-task-retention",
                trigger_reason: "scheduled",
                status,
                scheduled_at: "2026-06-22T10:00:00Z",
                queued_at: None,
                started_at: None,
                completed_at,
                chat_id: None,
                user_message_id: None,
                assistant_message_id: None,
                agent_team_id: None,
                agent_task_id: None,
                agent_attempt_id: None,
                active_run_id: None,
                error_message: None,
                output_summary: None,
                metadata_json: None,
            })
            .expect("scheduled run insert");
    }

    let active_ids = database
        .active_scheduled_task_runs()
        .expect("active scheduled runs")
        .into_iter()
        .map(|run| run.id)
        .collect::<Vec<_>>();
    assert_eq!(
        active_ids,
        vec!["scheduled-run-pending", "scheduled-run-queued"]
    );

    assert_eq!(
        database
            .delete_old_scheduled_task_runs("2026-06-01T00:00:00Z")
            .expect("delete old scheduled runs"),
        1
    );

    assert!(
        database
            .scheduled_task_run("scheduled-run-old")
            .expect("old run lookup")
            .is_none()
    );
    assert!(
        database
            .scheduled_task_run("scheduled-run-recent")
            .expect("recent run lookup")
            .is_some()
    );
    assert!(
        database
            .scheduled_task_run("scheduled-run-pending")
            .expect("pending run lookup")
            .is_some()
    );
}

#[test]
fn repository_helpers_reject_invalid_todo_graph_dependencies() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

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
fn chat_page_uses_cursor_search_and_scoped_code_change_stats() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    for (id, title, created_at) in [
        ("chat-1", "Alpha setup", "2026-07-03T10:00:00.000Z"),
        ("chat-2", "beta Plan", "2026-07-03T11:00:00.000Z"),
        ("chat-3", "ALPHA followup", "2026-07-03T12:00:00.000Z"),
        ("chat-4", "Gamma", "2026-07-03T13:00:00.000Z"),
    ] {
        database.insert_chat(id, title).expect("chat insert");
        Connection::open(database.database_path())
            .expect("open database")
            .execute(
                "UPDATE chats SET created_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![id, created_at],
            )
            .expect("timestamp update");
    }

    database
        .insert_chat_with_metadata(
            "chat-dream",
            "Alpha dream",
            &format!(r#"{{"kind":"{MEMORY_DREAM_TRANSCRIPT_CHAT_KIND}"}}"#),
        )
        .expect("dream insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-3",
            role: "assistant",
            content: "Done",
            sequence: 0,
            metadata_json: Some(r#"{"codeChangeStats":{"additions":4,"deletions":1}}"#),
        })
        .expect("message insert");
    database
        .insert_message(NewMessage {
            id: "assistant-2",
            chat_id: "chat-1",
            role: "assistant",
            content: "Done",
            sequence: 0,
            metadata_json: Some(r#"{"codeChangeStats":{"additions":2,"deletions":3}}"#),
        })
        .expect("message insert");

    for (id, created_at) in [
        ("chat-1", "2026-07-03T10:00:00.000Z"),
        ("chat-2", "2026-07-03T11:00:00.000Z"),
        ("chat-3", "2026-07-03T12:00:00.000Z"),
        ("chat-4", "2026-07-03T13:00:00.000Z"),
    ] {
        Connection::open(database.database_path())
            .expect("open database")
            .execute(
                "UPDATE chats SET created_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![id, created_at],
            )
            .expect("timestamp update");
    }

    let first_page = database.chat_page(2, None).expect("first page");
    assert_eq!(first_page.total_count, 4);
    assert_eq!(
        first_page
            .chats
            .iter()
            .map(|chat| chat.id.as_str())
            .collect::<Vec<_>>(),
        vec!["chat-4", "chat-3"]
    );
    assert!(first_page.has_more);

    let second_page = database
        .chat_page(2, first_page.next_cursor.as_ref())
        .expect("second page");
    assert_eq!(
        second_page
            .chats
            .iter()
            .map(|chat| chat.id.as_str())
            .collect::<Vec<_>>(),
        vec!["chat-2", "chat-1"]
    );
    assert!(!second_page.has_more);

    let search_page = database
        .search_chats("alpha", 10, None)
        .expect("search page");
    assert_eq!(search_page.total_count, 2);
    assert_eq!(
        search_page
            .chats
            .iter()
            .map(|chat| chat.id.as_str())
            .collect::<Vec<_>>(),
        vec!["chat-3", "chat-1"]
    );

    let stats = database
        .code_change_stats_for_chats(&["chat-3".to_string()])
        .expect("stats");
    assert_eq!(stats.len(), 1);
    assert_eq!(stats["chat-3"].additions, 4);
    assert_eq!(stats["chat-3"].deletions, 1);

    let existing = database
        .existing_chat_ids(&[
            "chat-3".to_string(),
            "missing-chat".to_string(),
            "chat-1".to_string(),
        ])
        .expect("existing chat ids");
    assert!(existing.contains("chat-1"));
    assert!(existing.contains("chat-3"));
    assert!(!existing.contains("missing-chat"));
    assert_eq!(existing.len(), 2);
}

#[test]
fn repository_helpers_round_trip_core_records() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
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
    database
        .insert_chat_with_metadata(
            "chat-dream",
            "Memory Dream",
            &format!(r#"{{"kind":"{MEMORY_DREAM_TRANSCRIPT_CHAT_KIND}"}}"#),
        )
        .expect("dream chat insert");
    assert_eq!(
        database
            .chat("chat-1")
            .expect("chat read")
            .expect("chat")
            .title,
        "First chat"
    );
    assert!(
        database
            .update_chat_title_if_current("chat-1", "First chat", "Generated title")
            .expect("chat title update")
    );
    assert!(
        !database
            .update_chat_title_if_current("chat-1", "First chat", "Stale title")
            .expect("stale chat title update")
    );
    assert_eq!(
        database
            .chat("chat-1")
            .expect("updated chat read")
            .expect("updated chat")
            .title,
        "Generated title"
    );
    Connection::open(database.database_path())
        .expect("open database for deterministic chat ordering")
        .execute(
            "UPDATE chats
             SET created_at = CASE id
                     WHEN 'chat-1' THEN '2026-06-01T00:00:00.000Z'
                     ELSE '2026-06-02T00:00:00.000Z'
                 END,
                 updated_at = CASE id
                     WHEN 'chat-1' THEN '2026-06-03T00:00:00.000Z'
                     ELSE '2026-06-02T00:00:00.000Z'
                 END
             WHERE id IN ('chat-1', 'chat-2')",
            [],
        )
        .expect("set deterministic chat ordering");
    let chats = database.chats().expect("chat list");
    assert_eq!(chats.len(), 2);
    assert_eq!(chats[0].title, "Generated title");
    assert_eq!(chats[1].title, "Second chat");
    let dream_chats = database
        .dream_transcript_chats()
        .expect("dream transcript chat list");
    assert_eq!(dream_chats.len(), 1);
    assert_eq!(dream_chats[0].id, "chat-dream");

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
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-06-03T10:00:00.000Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: Some(3),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(200),
            final_state: "completed",
            request_body_json: None,
            response_body_json: None,
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
        .update_llm_request_body(
            "request-1",
            Some(
                r#"{"format":"provider_request_v1","version":1,"method":"POST","url":"https://example.test","headers":{"authorization":"Bearer secret"},"body":"actual"}"#,
            ),
        )
        .expect("llm request body update");
    let request = database
        .llm_request("request-1")
        .expect("updated llm request read")
        .expect("updated llm request");
    let request_body: serde_json::Value = serde_json::from_str(
        request
            .request_body_json
            .as_deref()
            .expect("updated request body"),
    )
    .expect("updated request body json");
    assert_eq!(request_body["format"], "provider_request_v1");
    assert_eq!(request_body["headers"]["authorization"], "********");
    assert_eq!(request_body["body"], "actual");
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
fn rejects_non_v1_audit_details_and_prunes_legacy_during_explicit_maintenance() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Audit detail invariants")
        .expect("chat insert");

    let insert_request = |database: &mut WorkspaceDatabase, id: &str| {
        database
            .insert_llm_request(NewLlmRequest {
                id,
                workspace_id: "workspace-1",
                chat_id: Some("chat-1"),
                request_kind: "chat completion",
                agent_team_id: None,
                agent_instance_id: None,
                agent_task_id: None,
                agent_attempt_id: None,
                provider_id: "openai",
                model_id: "gpt-test",
                thinking_level: None,
                request_started_at: "2026-07-13T00:00:00Z",
                first_token_at: None,
                completed_at: Some("2026-07-13T00:00:01Z"),
                input_tokens: Some(1),
                output_tokens: Some(1),
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
                first_token_latency_ms: None,
                total_latency_ms: Some(1000),
                status_code: Some(200),
                final_state: "succeeded",
                request_body_json: None,
                response_body_json: None,
            })
            .expect("request insert");
    };
    insert_request(&mut database, "request-1");
    insert_request(&mut database, "request-empty-object");
    insert_request(&mut database, "request-neutral");
    insert_request(&mut database, "request-normalized");
    insert_request(&mut database, "request-error");
    insert_request(&mut database, "request-cancelled");
    insert_request(&mut database, "request-legacy-text");
    insert_request(&mut database, "request-valid-v1");
    insert_request(&mut database, "request-valid-websocket-v1");

    let reject = database.update_llm_request_body("request-1", Some(r#"{"text":"legacy"}"#));
    assert!(reject.is_err(), "non-v1 request body must be rejected");

    // Bypass store validators to simulate an upgrade from a database without the cleanup marker.
    {
        let database_path = database.database_path().to_path_buf();
        let connection = rusqlite::Connection::open(&database_path).expect("open raw sqlite");
        let plant = |id: &str, request: &str, response: &str| {
            connection
                .execute(
                    "UPDATE llm_requests SET request_body_json = ?1, response_body_json = ?2 WHERE id = ?3",
                    rusqlite::params![request, response, id],
                )
                .expect("plant legacy detail");
        };
        plant(
            "request-1",
            r#"{"messages":[{"role":"user","content":"hi"}]}"#,
            r#"{"text":"normalized","reasoning":null}"#,
        );
        plant(
            "request-empty-object",
            "{}",
            r#"{"requestKind":"chat completion"}"#,
        );
        plant(
            "request-neutral",
            r#"{"modelId":"m","messages":[],"tools":[]}"#,
            r#"{"providerCompletions":[]}"#,
        );
        plant(
            "request-normalized",
            r#"{"format":"legacy_text_v1","version":1,"text":"nope"}"#,
            r#"{"text":"done","reasoning":null,"providerCompletions":[]}"#,
        );
        plant(
            "request-error",
            r#"{"error":"request failed"}"#,
            r#"{"error":"stream failed"}"#,
        );
        plant(
            "request-cancelled",
            r#"{"cancelled":true}"#,
            r#"{"cancelled":"chat run cancelled"}"#,
        );
        plant(
            "request-legacy-text",
            r#"{"format":"legacy_text_v1","body":"x"}"#,
            r#"{"format":"legacy_text_v1","body":"y"}"#,
        );
        plant(
            "request-valid-v1",
            r#"{"format":"provider_request_v1","version":1,"method":"POST","url":"https://example.test","headers":{"authorization":["********"]},"body":null}"#,
            r#"{"format":"provider_final_response_v1","version":1,"state":"succeeded","partial":false,"text":"ok","reasoning":null,"toolCalls":[],"usage":null,"stopReason":null,"responseId":null,"error":null,"http":{"status":200,"version":"HTTP/1.1","headers":{"authorization":["********"],"x-multi":["a","b"]}}}"#,
        );
        plant(
            "request-valid-websocket-v1",
            r#"{"format":"provider_websocket_request_v1","version":1,"url":"wss://example.test/v1/responses","headers":{"authorization":["********"]},"createFrame":"{\"type\":\"response.create\"}","createFrameEncoding":"utf8","frameSent":true,"connectionReused":false,"handshake":{"status":101,"version":"HTTP/1.1","headers":{"upgrade":["websocket"]}}}"#,
            r#"{"format":"provider_final_response_v1","version":1,"state":"succeeded","partial":false,"text":"ok","reasoning":null,"toolCalls":[],"usage":null,"stopReason":null,"responseId":"resp_ws","error":null,"http":{"status":101,"version":"HTTP/1.1","headers":{}}}"#,
        );
        connection
            .execute(
                "DELETE FROM workspace_metadata WHERE key = 'llm_audit_detail_v1_pruned'",
                [],
            )
            .expect("remove cleanup marker");
    }
    drop(database);

    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopen database");
    assert!(
        database
            .llm_request("request-1")
            .expect("request read before maintenance")
            .expect("request before maintenance")
            .request_body_json
            .is_some(),
        "ordinary reopen must not scan and prune audit detail tables"
    );
    database
        .run_pending_one_time_maintenance()
        .expect("run explicit one-time maintenance");
    for id in [
        "request-1",
        "request-empty-object",
        "request-neutral",
        "request-normalized",
        "request-error",
        "request-cancelled",
        "request-legacy-text",
    ] {
        let request = database
            .llm_request(id)
            .expect("request read")
            .expect("request");
        assert_eq!(request.request_body_json, None, "{id} request pruned");
        assert_eq!(request.response_body_json, None, "{id} response pruned");
    }

    let valid = database
        .llm_request("request-valid-v1")
        .expect("valid read")
        .expect("valid request");
    let valid_request: serde_json::Value =
        serde_json::from_str(valid.request_body_json.as_deref().expect("kept request"))
            .expect("parse kept request");
    assert_eq!(valid_request["format"], "provider_request_v1");
    assert_eq!(valid_request["headers"]["authorization"][0], "********");
    let valid_response: serde_json::Value =
        serde_json::from_str(valid.response_body_json.as_deref().expect("kept response"))
            .expect("parse kept response");
    assert_eq!(valid_response["format"], "provider_final_response_v1");
    assert_eq!(
        valid_response["http"]["headers"]["authorization"][0],
        "********"
    );
    assert_eq!(
        valid_response["http"]["headers"]["x-multi"],
        json!(["a", "b"])
    );
    let valid_ws = database
        .llm_request("request-valid-websocket-v1")
        .expect("valid websocket read")
        .expect("valid websocket request");
    let valid_ws_request: serde_json::Value = serde_json::from_str(
        valid_ws
            .request_body_json
            .as_deref()
            .expect("kept ws request"),
    )
    .expect("parse kept ws request");
    assert_eq!(valid_ws_request["format"], "provider_websocket_request_v1");
    assert_eq!(valid_ws_request["connectionReused"], false);
    assert_eq!(valid_ws_request["url"], "wss://example.test/v1/responses");
    assert_eq!(valid_ws_request["headers"]["authorization"][0], "********");
    let valid_ws_response: serde_json::Value = serde_json::from_str(
        valid_ws
            .response_body_json
            .as_deref()
            .expect("kept ws response"),
    )
    .expect("parse kept ws response");
    assert_eq!(valid_ws_response["format"], "provider_final_response_v1");
    assert_eq!(valid_ws_response["http"]["status"], 101);
    let cleanup_marker: String = Connection::open(database.database_path())
        .expect("open marker database")
        .query_row(
            "SELECT value FROM workspace_metadata WHERE key = 'llm_audit_detail_v1_pruned'",
            [],
            |row| row.get(0),
        )
        .expect("cleanup marker");
    assert_eq!(cleanup_marker, "true");

    // Valid v1 is retained; later NULL/non-v1 cannot overwrite first capture.
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopen mut");
    database
        .update_llm_request_body(
            "request-1",
            Some(
                r#"{"format":"provider_request_v1","version":1,"method":"POST","url":"https://example.test","headers":{},"body":null}"#,
            ),
        )
        .expect("v1 request body");
    database
        .update_llm_request_outcome(
            "request-1",
            UpdateLlmRequestOutcome {
                first_token_at: None,
                completed_at: Some("2026-07-13T00:00:01Z"),
                input_tokens: Some(1),
                output_tokens: Some(1),
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
                first_token_latency_ms: None,
                total_latency_ms: Some(1000),
                status_code: Some(200),
                final_state: "succeeded",
                response_body_json: Some(
                    r#"{"format":"provider_final_response_v1","version":1,"state":"succeeded","partial":false,"text":"ok","reasoning":null,"toolCalls":[],"usage":null,"stopReason":null,"responseId":null,"error":null,"http":null}"#,
                ),
            },
        )
        .expect("v1 response body");
    database
        .update_llm_request_body("request-1", None)
        .expect("null must not clear valid v1 request");
    database
        .update_llm_request_outcome(
            "request-1",
            UpdateLlmRequestOutcome {
                first_token_at: None,
                completed_at: Some("2026-07-13T00:00:01Z"),
                input_tokens: Some(1),
                output_tokens: Some(1),
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
                first_token_latency_ms: None,
                total_latency_ms: Some(1000),
                status_code: Some(200),
                final_state: "succeeded",
                response_body_json: None,
            },
        )
        .expect("null must not clear valid v1 response");
    let request = database
        .llm_request("request-1")
        .expect("request read")
        .expect("request");
    assert!(
        request
            .request_body_json
            .as_deref()
            .is_some_and(|value| value.contains("provider_request_v1"))
    );
    assert!(
        request
            .response_body_json
            .as_deref()
            .is_some_and(|value| value.contains("provider_final_response_v1"))
    );
}

#[test]
fn repairs_null_status_code_from_valid_v1_response_wire_during_explicit_maintenance() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Status code repair")
        .expect("chat insert");

    let insert = |database: &mut WorkspaceDatabase, id: &str, final_state: &str| {
        database
            .insert_llm_request(NewLlmRequest {
                id,
                workspace_id: "workspace-1",
                chat_id: Some("chat-1"),
                request_kind: "chat completion",
                agent_team_id: None,
                agent_instance_id: None,
                agent_task_id: None,
                agent_attempt_id: None,
                provider_id: "openai",
                model_id: "gpt-test",
                thinking_level: None,
                request_started_at: "2026-07-14T00:00:00Z",
                first_token_at: None,
                completed_at: if final_state == "running" {
                    None
                } else {
                    Some("2026-07-14T00:00:01Z")
                },
                input_tokens: Some(1),
                output_tokens: Some(1),
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
                first_token_latency_ms: None,
                total_latency_ms: if final_state == "running" {
                    None
                } else {
                    Some(1000)
                },
                status_code: None,
                final_state,
                request_body_json: None,
                response_body_json: None,
            })
            .expect("request insert");
    };

    insert(&mut database, "with-http-status", "succeeded");
    insert(&mut database, "failed-status-code-only", "failed");
    insert(&mut database, "no-head-succeeded", "succeeded");
    insert(&mut database, "running-with-http", "running");
    insert(&mut database, "invalid-status-range", "failed");
    insert(&mut database, "non-v1-response", "succeeded");
    insert(&mut database, "already-has-status", "succeeded");
    insert(&mut database, "cleaned-details", "succeeded");

    {
        let database_path = database.database_path().to_path_buf();
        let connection = rusqlite::Connection::open(&database_path).expect("open raw sqlite");
        let plant = |id: &str, status_code: Option<i64>, response: Option<&str>| {
            connection
                .execute(
                    "UPDATE llm_requests SET status_code = ?1, response_body_json = ?2 WHERE id = ?3",
                    rusqlite::params![status_code, response, id],
                )
                .expect("plant audit row");
        };
        plant(
            "with-http-status",
            None,
            Some(
                r#"{"format":"provider_final_response_v1","version":1,"state":"succeeded","text":"ok","reasoning":null,"toolCalls":[],"usage":null,"stopReason":null,"responseId":null,"http":{"status":200,"version":"HTTP/1.1","headers":{}}}"#,
            ),
        );
        plant(
            "failed-status-code-only",
            None,
            Some(
                r#"{"format":"provider_final_response_v1","version":1,"state":"failed","partial":false,"error":"upstream","statusCode":502,"http":null}"#,
            ),
        );
        plant(
            "no-head-succeeded",
            None,
            Some(
                r#"{"format":"provider_final_response_v1","version":1,"state":"succeeded","text":"ok","reasoning":null,"toolCalls":[],"usage":null,"stopReason":null,"responseId":null,"http":null}"#,
            ),
        );
        plant(
            "running-with-http",
            None,
            Some(
                r#"{"format":"provider_final_response_v1","version":1,"state":"succeeded","text":"ok","reasoning":null,"toolCalls":[],"usage":null,"stopReason":null,"responseId":null,"http":{"status":200,"version":"HTTP/1.1","headers":{}}}"#,
            ),
        );
        plant(
            "invalid-status-range",
            None,
            Some(
                r#"{"format":"provider_final_response_v1","version":1,"state":"failed","partial":false,"error":"bad","statusCode":999,"http":null}"#,
            ),
        );
        plant(
            "non-v1-response",
            None,
            Some(r#"{"text":"normalized","statusCode":200}"#),
        );
        plant(
            "already-has-status",
            Some(418),
            Some(
                r#"{"format":"provider_final_response_v1","version":1,"state":"succeeded","text":"ok","reasoning":null,"toolCalls":[],"usage":null,"stopReason":null,"responseId":null,"http":{"status":200,"version":"HTTP/1.1","headers":{}}}"#,
            ),
        );
        // Detail already cleaned / never captured: keep NULL status_code (no forged 200).
        plant("cleaned-details", None, None);
        connection
            .execute(
                "DELETE FROM workspace_metadata WHERE key = 'llm_audit_status_code_v1_repaired'",
                [],
            )
            .expect("remove repair marker");
    }
    drop(database);

    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopen for repair");
    assert_eq!(
        database
            .llm_request("with-http-status")
            .expect("read before maintenance")
            .expect("row before maintenance")
            .status_code,
        None,
        "ordinary reopen must not scan response payloads"
    );
    database
        .run_pending_one_time_maintenance()
        .expect("run explicit one-time maintenance");
    assert_eq!(
        database
            .llm_request("with-http-status")
            .expect("read")
            .expect("row")
            .status_code,
        Some(200)
    );
    assert_eq!(
        database
            .llm_request("failed-status-code-only")
            .expect("read")
            .expect("row")
            .status_code,
        Some(502)
    );
    assert_eq!(
        database
            .llm_request("no-head-succeeded")
            .expect("read")
            .expect("row")
            .status_code,
        None,
        "must not invent 200 from final_state"
    );
    assert_eq!(
        database
            .llm_request("running-with-http")
            .expect("read")
            .expect("row")
            .status_code,
        None,
        "running rows stay untouched"
    );
    assert_eq!(
        database
            .llm_request("invalid-status-range")
            .expect("read")
            .expect("row")
            .status_code,
        None
    );
    assert_eq!(
        database
            .llm_request("non-v1-response")
            .expect("read")
            .expect("row")
            .status_code,
        None
    );
    assert_eq!(
        database
            .llm_request("already-has-status")
            .expect("read")
            .expect("row")
            .status_code,
        Some(418),
        "existing status_code is not overwritten"
    );
    assert_eq!(
        database
            .llm_request("cleaned-details")
            .expect("read")
            .expect("row")
            .status_code,
        None,
        "cleaned/missing detail must stay n/a"
    );

    let repair_marker: String = rusqlite::Connection::open(database.database_path())
        .expect("open marker database")
        .query_row(
            "SELECT value FROM workspace_metadata WHERE key = 'llm_audit_status_code_v1_repaired'",
            [],
            |row| row.get(0),
        )
        .expect("repair marker");
    assert_eq!(repair_marker, "true");

    // Marker makes a second explicit maintenance run a no-op.
    let status_before = database
        .llm_request("with-http-status")
        .expect("read")
        .expect("row")
        .status_code;
    drop(database);
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("second reopen");
    database
        .run_pending_one_time_maintenance()
        .expect("second explicit maintenance run");
    assert_eq!(
        database
            .llm_request("with-http-status")
            .expect("read")
            .expect("row")
            .status_code,
        status_before
    );
    let marker_count: i64 = rusqlite::Connection::open(database.database_path())
        .expect("open marker database")
        .query_row(
            "SELECT COUNT(*) FROM workspace_metadata WHERE key = 'llm_audit_status_code_v1_repaired'",
            [],
            |row| row.get(0),
        )
        .expect("marker count");
    assert_eq!(marker_count, 1);
}

#[test]
fn reopening_workspace_does_not_run_pending_audit_maintenance() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    let database_path = database.database_path().to_path_buf();
    drop(database);

    let mut writer = Connection::open(&database_path).expect("open writer");
    let transaction = writer.transaction().expect("writer transaction");
    transaction
        .execute(
            "INSERT INTO chats (id, title, created_at, updated_at) VALUES ('chat-1', 'Lock holder', '2026-07-14T00:00:00Z', '2026-07-14T00:00:00Z')",
            [],
        )
        .expect("hold write transaction");

    let reopened = WorkspaceDatabase::open_or_create_ungated(workspace.path())
        .expect("reopen while another connection holds a write transaction");
    assert_eq!(
        reopened
            .workspace_metadata("llm_audit_detail_v1_pruned")
            .expect("cleanup marker lookup"),
        None
    );
    assert_eq!(
        reopened
            .workspace_metadata("llm_audit_status_code_v1_repaired")
            .expect("repair marker lookup"),
        None
    );
    transaction.rollback().expect("rollback writer transaction");
}

#[test]
fn repository_helpers_delete_chat_cascades_chat_state_and_preserves_audit() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

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
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-06-03T10:00:00.000Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: Some(3),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            reasoning_tokens: None,
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
fn messages_for_chat_page_and_role_counts_are_ordered() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Paged chat")
        .expect("chat insert");

    for (sequence, role) in [
        (0, "user"),
        (1, "assistant"),
        (2, "user"),
        (3, "assistant"),
        (4, "tool"),
    ] {
        database
            .insert_message(NewMessage {
                id: &format!("message-{sequence}"),
                chat_id: "chat-1",
                role,
                content: &format!("message {sequence}"),
                sequence,
                metadata_json: None,
            })
            .expect("message insert");
    }

    let recent = database
        .messages_for_chat_page("chat-1", None, 2)
        .expect("recent page");
    assert_eq!(
        recent
            .iter()
            .map(|message| message.sequence)
            .collect::<Vec<_>>(),
        vec![3, 4]
    );
    let previous = database
        .messages_for_chat_page("chat-1", Some(3), 2)
        .expect("previous page");
    assert_eq!(
        previous
            .iter()
            .map(|message| message.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );

    let counts = database
        .message_role_counts_for_chat("chat-1")
        .expect("role counts");
    let count_for = |role: &str| {
        counts
            .iter()
            .find(|record| record.role == role)
            .map(|record| record.count)
            .unwrap_or_default()
    };
    assert_eq!(counts.iter().map(|record| record.count).sum::<i64>(), 5);
    assert_eq!(count_for("user"), 2);
    assert_eq!(count_for("assistant"), 2);
    assert_eq!(count_for("tool"), 1);
}

#[test]
fn tool_calls_for_message_ids_scopes_to_page_and_short_circuits_empty() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Scoped tools")
        .expect("chat insert");
    for sequence in 0..6 {
        let role = if sequence % 2 == 0 {
            "user"
        } else {
            "assistant"
        };
        database
            .insert_message(NewMessage {
                id: &format!("message-{sequence}"),
                chat_id: "chat-1",
                role,
                content: &format!("message {sequence}"),
                sequence,
                metadata_json: None,
            })
            .expect("message insert");
    }
    for (call_index, message_id) in [
        ("message-1", "message-1"),
        ("message-3", "message-3"),
        ("message-5", "message-5"),
    ] {
        database
            .insert_tool_call(NewToolCall {
                id: &format!("tool-{call_index}"),
                chat_id: "chat-1",
                run_id: "run-1",
                message_id: Some(message_id),
                tool_name: "read_file",
                input_json: r#"{"path":"a.rs"}"#,
                status: "completed",
                started_at: "2026-07-01T00:00:00.000Z",
                completed_at: Some("2026-07-01T00:00:01.000Z"),
            })
            .expect("tool call insert");
        database
            .insert_tool_result(NewToolResult {
                id: &format!("result-{call_index}"),
                tool_call_id: &format!("tool-{call_index}"),
                output_json: r#"{"ok":true}"#,
                is_error: false,
                created_at: "2026-07-01T00:00:01.000Z",
            })
            .expect("tool result insert");
    }

    let empty = database
        .tool_calls_for_message_ids(&[])
        .expect("empty tool calls");
    assert!(empty.is_empty());

    let page = database
        .tool_calls_for_message_ids(&["message-3".to_string(), "message-5".to_string()])
        .expect("page tool calls");
    assert_eq!(page.len(), 2);
    assert!(
        page.iter()
            .all(|call| call.message_id.as_deref() == Some("message-3")
                || call.message_id.as_deref() == Some("message-5"))
    );
    assert!(page.iter().all(|call| call.result.is_some()));
}

#[test]
fn llm_request_metrics_for_assistant_message_ids_scopes_to_page() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Scoped metrics")
        .expect("chat insert");
    for sequence in [0_i64, 1, 2, 3] {
        let role = if sequence % 2 == 0 {
            "user"
        } else {
            "assistant"
        };
        database
            .insert_message(NewMessage {
                id: &format!("message-{sequence}"),
                chat_id: "chat-1",
                role,
                content: &format!("message {sequence}"),
                sequence,
                metadata_json: None,
            })
            .expect("message insert");
    }
    for (request_id, assistant_id, started_at) in [
        ("req-1", "message-1", "2026-07-01T00:00:00.000Z"),
        ("req-2", "message-3", "2026-07-01T00:01:00.000Z"),
        ("req-3", "message-3", "2026-07-01T00:01:30.000Z"),
    ] {
        database
            .insert_llm_request(NewLlmRequest {
                id: request_id,
                workspace_id: "workspace-1",
                chat_id: Some("chat-1"),
                request_kind: "chat completion",
                agent_team_id: None,
                agent_instance_id: None,
                agent_task_id: None,
                agent_attempt_id: None,
                provider_id: "openai",
                model_id: "gpt-test",
                thinking_level: None,
                request_started_at: started_at,
                first_token_at: Some(started_at),
                completed_at: Some(started_at),
                input_tokens: Some(10),
                output_tokens: Some(5),
                cache_read_tokens: Some(0),
                cache_write_tokens: Some(0),
                reasoning_tokens: None,
                first_token_latency_ms: Some(10),
                total_latency_ms: Some(100),
                status_code: Some(200),
                final_state: "succeeded",
                request_body_json: None,
                response_body_json: None,
            })
            .expect("llm request insert");
        database
            .insert_llm_request_event(NewLlmRequestEvent {
                id: &format!("{request_id}-start"),
                llm_request_id: request_id,
                sequence: 0,
                event_at: started_at,
                event_type: "start",
                raw_chunk_json: None,
                normalized_event_json: &format!(
                    r#"{{"type":"start","assistantMessageId":"{assistant_id}"}}"#
                ),
            })
            .expect("start event insert");
    }

    let empty = database
        .llm_request_metrics_for_assistant_message_ids("chat-1", &[])
        .expect("empty metrics");
    assert!(empty.is_empty());

    let page = database
        .llm_request_metrics_for_assistant_message_ids("chat-1", &["message-3".to_string()])
        .expect("page metrics");
    assert_eq!(page.len(), 2);
    assert!(
        page.iter()
            .all(|row| row.assistant_message_id == "message-3")
    );
    assert_eq!(
        page.iter()
            .map(|row| row.metrics.id.as_str())
            .collect::<Vec<_>>(),
        vec!["req-2", "req-3"]
    );
}

#[test]
fn prompt_context_injections_for_message_page_scopes_turn_memory_and_stable() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Paged injections")
        .expect("chat insert");
    database
        .insert_prompt_context_injection(NewPromptContextInjection {
            id: "stable-1",
            chat_id: "chat-1",
            kind: "stable",
            sequence: None,
            messages_json: r#"[{"role":"system","content":"stable"}]"#,
            memory_keys_json: r#"["stable-key"]"#,
            memory_summaries_json: r#"[{"id":"stable-key","content":"stable memory"}]"#,
        })
        .expect("stable injection");
    for (id, sequence) in [
        ("turn-0", 0_i64),
        ("turn-2", 2),
        ("turn-4", 4),
        ("turn-6", 6),
    ] {
        database
            .insert_prompt_context_injection(NewPromptContextInjection {
                id,
                chat_id: "chat-1",
                kind: "turn_memory",
                sequence: Some(sequence),
                messages_json: r#"[{"role":"system","content":"turn"}]"#,
                memory_keys_json: &format!(r#"["{id}"]"#),
                memory_summaries_json: &format!(r#"[{{"id":"{id}","content":"memory for {id}"}}]"#),
            })
            .expect("turn injection");
    }

    // Page assistants sequences 5..7 → turn_memory user sequences 4..6.
    let with_stable = database
        .prompt_context_injections_for_message_page("chat-1", Some(5), Some(7), true)
        .expect("page with stable");
    assert!(
        with_stable
            .iter()
            .any(|row| row.kind == "stable" && row.id == "stable-1")
    );
    let turn_ids = with_stable
        .iter()
        .filter(|row| row.kind == "turn_memory")
        .map(|row| row.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(turn_ids, vec!["turn-4", "turn-6"]);

    let without_stable = database
        .prompt_context_injections_for_message_page("chat-1", Some(5), Some(7), false)
        .expect("page without stable");
    assert!(without_stable.iter().all(|row| row.kind != "stable"));
    assert_eq!(
        without_stable
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["turn-4", "turn-6"]
    );

    let no_assistants = database
        .prompt_context_injections_for_message_page("chat-1", None, None, false)
        .expect("no assistants no stable");
    assert!(no_assistants.is_empty());
}

#[test]
fn large_chat_page_association_loads_only_current_page_ids() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Large chat")
        .expect("chat insert");

    const MESSAGE_COUNT: i64 = 4000;
    const PAGE_LIMIT: usize = 60;
    for sequence in 0..MESSAGE_COUNT {
        let role = if sequence % 2 == 0 {
            "user"
        } else {
            "assistant"
        };
        database
            .insert_message(NewMessage {
                id: &format!("message-{sequence}"),
                chat_id: "chat-1",
                role,
                content: &format!("message {sequence}"),
                sequence,
                metadata_json: None,
            })
            .expect("message insert");
        if role == "assistant" {
            let message_id = format!("message-{sequence}");
            let request_id = format!("req-{sequence}");
            let tool_id = format!("tool-{sequence}");
            let result_id = format!("result-{sequence}");
            let minute = (sequence / 60) % 60;
            let second = sequence % 60;
            let started_at = format!("2026-07-01T12:{minute:02}:{second:02}.000Z");
            database
                .insert_llm_request(NewLlmRequest {
                    id: &request_id,
                    workspace_id: "workspace-1",
                    chat_id: Some("chat-1"),
                    request_kind: "chat completion",
                    agent_team_id: None,
                    agent_instance_id: None,
                    agent_task_id: None,
                    agent_attempt_id: None,
                    provider_id: "openai",
                    model_id: "gpt-test",
                    thinking_level: None,
                    request_started_at: &started_at,
                    first_token_at: Some(&started_at),
                    completed_at: Some(&started_at),
                    input_tokens: Some(10),
                    output_tokens: Some(5),
                    cache_read_tokens: Some(0),
                    cache_write_tokens: Some(0),
                    reasoning_tokens: None,
                    first_token_latency_ms: Some(10),
                    total_latency_ms: Some(100),
                    status_code: Some(200),
                    final_state: "succeeded",
                    request_body_json: None,
                    response_body_json: None,
                })
                .expect("llm request insert");
            database
                .insert_llm_request_event(NewLlmRequestEvent {
                    id: &format!("{request_id}-start"),
                    llm_request_id: &request_id,
                    sequence: 0,
                    event_at: &started_at,
                    event_type: "start",
                    raw_chunk_json: None,
                    normalized_event_json: &format!(
                        r#"{{"type":"start","assistantMessageId":"{message_id}"}}"#
                    ),
                })
                .expect("start event insert");
            database
                .insert_tool_call(NewToolCall {
                    id: &tool_id,
                    chat_id: "chat-1",
                    run_id: "run-1",
                    message_id: Some(message_id.as_str()),
                    tool_name: "read_file",
                    input_json: r#"{"path":"a.rs"}"#,
                    status: "completed",
                    started_at: &started_at,
                    completed_at: Some(&started_at),
                })
                .expect("tool call insert");
            database
                .insert_tool_result(NewToolResult {
                    id: &result_id,
                    tool_call_id: &tool_id,
                    output_json: r#"{"ok":true}"#,
                    is_error: false,
                    created_at: &started_at,
                })
                .expect("tool result insert");
        }
    }

    let whole_chat_tools = database
        .tool_calls_for_chat("chat-1")
        .expect("whole chat tools");
    assert_eq!(whole_chat_tools.len(), (MESSAGE_COUNT / 2) as usize);

    let latest = database
        .messages_for_chat_page("chat-1", None, PAGE_LIMIT)
        .expect("latest page");
    assert_eq!(latest.len(), PAGE_LIMIT);
    assert_eq!(
        latest.first().map(|m| m.sequence),
        Some(MESSAGE_COUNT - PAGE_LIMIT as i64)
    );
    assert_eq!(latest.last().map(|m| m.sequence), Some(MESSAGE_COUNT - 1));

    let page_ids = latest
        .iter()
        .map(|message| message.id.clone())
        .collect::<Vec<_>>();
    let assistant_ids = latest
        .iter()
        .filter(|message| message.role == "assistant")
        .map(|message| message.id.clone())
        .collect::<Vec<_>>();
    let page_tools = database
        .tool_calls_for_message_ids(&page_ids)
        .expect("page tools");
    assert_eq!(page_tools.len(), assistant_ids.len());
    assert!(page_tools.iter().all(|call| {
        call.message_id
            .as_ref()
            .is_some_and(|id| page_ids.contains(id))
    }));

    let page_metrics = database
        .llm_request_metrics_for_assistant_message_ids("chat-1", &assistant_ids)
        .expect("page metrics");
    assert_eq!(page_metrics.len(), assistant_ids.len());
    assert!(
        page_metrics
            .iter()
            .all(|row| assistant_ids.contains(&row.assistant_message_id))
    );

    let next_before = latest
        .first()
        .map(|message| message.sequence)
        .expect("cursor");
    let previous = database
        .messages_for_chat_page("chat-1", Some(next_before), PAGE_LIMIT)
        .expect("previous page");
    assert_eq!(previous.len(), PAGE_LIMIT);
    let previous_ids = previous
        .iter()
        .map(|message| message.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let latest_ids = latest
        .iter()
        .map(|message| message.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert!(previous_ids.is_disjoint(&latest_ids));
    assert_eq!(previous.last().map(|m| m.sequence), Some(next_before - 1));

    // EXPLAIN: page-scoped metrics should use chat-valid index path, not full table scan.
    let connection = Connection::open(database.database_path()).expect("open database");
    let metrics_plan = explain_query_plan(
        &connection,
        "SELECT
            CAST(
                COALESCE(
                    json_extract(llm_request_events.normalized_event_json, '$.assistantMessageId'),
                    json_extract(llm_request_events.normalized_event_json, '$.assistant_message_id')
                ) AS TEXT
            ) AS assistant_message_id,
            llm_requests.id
         FROM llm_requests
         INNER JOIN llm_request_events
            ON llm_request_events.llm_request_id = llm_requests.id
            AND llm_request_events.event_type = 'start'
            AND llm_request_events.sequence = 0
         WHERE llm_requests.chat_id = 'chat-1'
           AND llm_requests.invalidated_at IS NULL
           AND CAST(
                COALESCE(
                    json_extract(llm_request_events.normalized_event_json, '$.assistantMessageId'),
                    json_extract(llm_request_events.normalized_event_json, '$.assistant_message_id')
                ) AS TEXT
           ) IN ('message-3999', 'message-3997')
         ORDER BY llm_requests.request_started_at ASC, llm_requests.id ASC",
    );
    assert!(
        plan_uses_index(&metrics_plan, "llm_requests_chat_valid_idx")
            || plan_uses_index(&metrics_plan, "llm_requests_chat_idx")
            || metrics_plan.contains("SEARCH llm_requests")
            || metrics_plan.contains("USING INDEX"),
        "page metrics should use indexed llm_requests path, plan:\n{metrics_plan}"
    );
    assert_no_unconstrained_table_scan(&metrics_plan, "llm_requests");

    let tools_plan = explain_query_plan(
        &connection,
        "SELECT tool_calls.id
         FROM tool_calls
         LEFT JOIN tool_results ON tool_results.tool_call_id = tool_calls.id
         WHERE tool_calls.message_id IN ('message-3999', 'message-3997')
         ORDER BY tool_calls.started_at ASC, tool_calls.id ASC",
    );
    assert!(
        plan_uses_index(&tools_plan, "tool_calls_message_idx")
            || tools_plan.contains("SEARCH tool_calls")
            || tools_plan.contains("USING INDEX"),
        "page tool calls should use message_id index, plan:\n{tools_plan}"
    );
}

#[test]
fn repository_helpers_persist_terminal_working_directory() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
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
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

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
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

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
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

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
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

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
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
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
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
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
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_chat("chat-1", "Audit chat")
        .expect("chat insert");
    database
        .insert_llm_request(NewLlmRequest {
            id: "request-1",
            workspace_id: "workspace-1",
            chat_id: Some("chat-1"),
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai-responses",
            model_id: "gpt-audit",
            thinking_level: None,
            request_started_at: "2026-06-03T10:00:00.000Z",
            first_token_at: Some("2026-06-03T10:00:00.250Z"),
            completed_at: Some("2026-06-03T10:00:01.500Z"),
            input_tokens: Some(100),
            output_tokens: Some(25),
            cache_read_tokens: Some(40),
            cache_write_tokens: Some(10),
            reasoning_tokens: Some(3),
            first_token_latency_ms: Some(250),
            total_latency_ms: Some(1500),
            status_code: Some(200),
            final_state: "completed",
            request_body_json: Some(
                r#"{
                    "format": "provider_request_v1",
                    "version": 1,
                    "method": "POST",
                    "url": "https://example.test/v1/chat",
                    "headers": {
                        "Authorization": "Bearer secret-token",
                        "OpenAI-Api-Key": "request-key"
                    },
                    "body": {
                        "model": "gpt-audit",
                        "input": "Hello",
                        "apiKey": "body-secret"
                    }
                }"#,
            ),
            response_body_json: Some(
                r#"{
                    "format": "provider_final_response_v1",
                    "version": 1,
                    "state": "succeeded",
                    "partial": false,
                    "text": "Hi",
                    "reasoning": null,
                    "toolCalls": [],
                    "usage": null,
                    "stopReason": null,
                    "responseId": null,
                    "error": null,
                    "http": {
                        "status": 200,
                        "version": "HTTP/1.1",
                        "headers": {
                            "authorization": "Bearer response-secret",
                            "x-api-key": "response-key"
                        }
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
                    "format": "provider_request_v1",
                    "version": 1,
                    "headers": {
                        "authorization": "Bearer streamed-secret",
                        "x-api-key": "streamed-api-key"
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
    assert_eq!(request.request_kind, "chat completion");
    assert_eq!(request.provider_id, "openai-responses");
    assert_eq!(request.model_id, "gpt-audit");
    assert_eq!(request.request_started_at, "2026-06-03T10:00:00.000Z");
    assert_eq!(request.first_token_latency_ms, Some(250));
    assert_eq!(request.total_latency_ms, Some(1500));
    assert_eq!(request.status_code, Some(200));
    assert_eq!(request.final_state, "completed");
    assert_eq!(request.reasoning_tokens, Some(3));
    assert_eq!(request.cache_ratio, Some(0.4));

    let request_body = request
        .request_body_json
        .as_deref()
        .expect("request body json");
    assert!(request_body.contains(r#""Authorization":"********""#));
    assert!(request_body.contains(r#""apiKey":"[REDACTED]""#));
    assert!(!request_body.contains("secret-token"));
    assert!(!request_body.contains("body-secret"));

    let response_body = request
        .response_body_json
        .as_deref()
        .expect("response body json");
    assert!(response_body.contains(r#""authorization":"********""#));
    assert!(!response_body.contains("response-secret"));
    // Non-Authorization headers on the HTTP head keep their values for v1 dumps.
    assert!(response_body.contains("response-key"));

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
    assert!(raw_chunk.contains(r#""x-api-key":"[REDACTED]""#));
    assert!(!raw_chunk.contains("streamed-secret"));
    assert!(!raw_chunk.contains("streamed-api-key"));
    assert_eq!(events[1].event_type, "usage");

    database
        .insert_llm_request(NewLlmRequest {
            id: "request-2",
            workspace_id: "workspace-1",
            chat_id: None,
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai-chat",
            model_id: "gpt-other",
            thinking_level: None,
            request_started_at: "2026-06-03T11:00:00.000Z",
            first_token_at: None,
            completed_at: Some("2026-06-03T11:00:00.250Z"),
            input_tokens: Some(8),
            output_tokens: Some(2),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: Some(250),
            status_code: None,
            final_state: "failed",
            request_body_json: None,
            response_body_json: None,
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
                reasoning_tokens: Some(6),
                first_token_latency_ms: Some(50),
                total_latency_ms: Some(300),
                status_code: Some(200),
                final_state: "succeeded",
                response_body_json: Some(r#"{"format":"provider_final_response_v1","version":1,"state":"failed","partial":false,"text":null,"reasoning":null,"toolCalls":[],"usage":null,"stopReason":null,"responseId":null,"error":{"message":"boom","apiKey":"secret"},"http":null}"#),
            },
        )
        .expect("update llm request outcome");
    let updated_request = database
        .llm_request("request-2")
        .expect("updated request read")
        .expect("updated request");
    assert_eq!(updated_request.final_state, "succeeded");
    assert_eq!(updated_request.status_code, Some(200));
    assert_eq!(updated_request.reasoning_tokens, Some(6));
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
    assert_eq!(all_rows[0].reasoning_tokens, Some(6));
    assert_eq!(all_rows[1].id, "request-1");
    assert_eq!(
        database
            .llm_request_audit_count(LlmRequestAuditFilters::default())
            .expect("audit count"),
        2
    );
    let request_ids = vec!["request-1".to_string()];
    let request_id_rows = database
        .llm_request_audit_rows(LlmRequestAuditFilters {
            request_ids: &request_ids,
            ..LlmRequestAuditFilters::default()
        })
        .expect("request id audit rows");
    assert_eq!(request_id_rows.len(), 1);
    assert_eq!(request_id_rows[0].id, "request-1");
    assert_eq!(
        database
            .llm_request_audit_count(LlmRequestAuditFilters {
                request_ids: &request_ids,
                ..LlmRequestAuditFilters::default()
            })
            .expect("request id audit count"),
        1
    );

    let request_ids = vec!["request-1".to_string(), "request-2".to_string()];
    let request_id_rows = database
        .llm_request_audit_rows(LlmRequestAuditFilters {
            request_ids: &request_ids,
            ..LlmRequestAuditFilters::default()
        })
        .expect("request ids audit rows");
    assert_eq!(request_id_rows.len(), 2);
    assert_eq!(request_id_rows[0].id, "request-2");
    assert_eq!(request_id_rows[1].id, "request-1");
    assert_eq!(
        database
            .llm_request_audit_count(LlmRequestAuditFilters {
                request_ids: &request_ids,
                ..LlmRequestAuditFilters::default()
            })
            .expect("request ids audit count"),
        2
    );
    let request_ids_summary = database
        .llm_request_audit_summary(LlmRequestAuditFilters {
            request_ids: &request_ids,
            ..LlmRequestAuditFilters::default()
        })
        .expect("request ids audit summary");
    assert_eq!(request_ids_summary.total_requests, 2);
    assert_eq!(request_ids_summary.total_tokens, 139);
    let request_ids_trend = database
        .llm_request_audit_trend_breakdown(LlmRequestAuditFilters {
            request_ids: &request_ids,
            ..LlmRequestAuditFilters::default()
        })
        .expect("request ids audit trend");
    assert_eq!(request_ids_trend.len(), 1);
    assert_eq!(request_ids_trend[0].request_count, 2);
    let request_ids_models = database
        .llm_request_audit_model_breakdown(LlmRequestAuditFilters {
            request_ids: &request_ids,
            ..LlmRequestAuditFilters::default()
        })
        .expect("request ids audit model breakdown");
    assert_eq!(request_ids_models.len(), 2);
    let request_ids_providers = database
        .llm_request_audit_provider_breakdown(LlmRequestAuditFilters {
            request_ids: &request_ids,
            ..LlmRequestAuditFilters::default()
        })
        .expect("request ids audit provider breakdown");
    assert_eq!(request_ids_providers.len(), 2);
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
            request_ids: &request_ids,
            workspace_id: Some("workspace-1"),
            chat_id: Some("chat-1"),
            request_kind: Some("chat completion"),
            exclude_request_kinds: &[],
            provider_id: Some("openai-responses"),
            model_id: Some("gpt-audit"),
            final_state: Some("completed"),
            started_after: Some("2026-06-03T09:00:00.000Z"),
            started_before: Some("2026-06-03T10:30:00.000Z"),
            valid_only: false,
            limit: Some(1),
            offset: None,
        })
        .expect("filtered audit rows");
    assert_eq!(filtered_rows.len(), 1);
    assert_eq!(filtered_rows[0].id, "request-1");
    assert_eq!(filtered_rows[0].reasoning_tokens, Some(3));
    assert_eq!(filtered_rows[0].cache_ratio, Some(0.4));
    assert_eq!(filtered_rows[0].transport, LlmRequestTransport::Http);
    assert_eq!(all_rows[0].transport, LlmRequestTransport::Unknown);
    assert_eq!(all_rows[1].transport, LlmRequestTransport::Http);
    assert_eq!(
        database
            .llm_request_audit_count(LlmRequestAuditFilters {
                request_ids: &request_ids,
                workspace_id: Some("workspace-1"),
                chat_id: Some("chat-1"),
                request_kind: Some("chat completion"),
                exclude_request_kinds: &[],
                provider_id: Some("openai-responses"),
                model_id: Some("gpt-audit"),
                final_state: Some("completed"),
                started_after: Some("2026-06-03T09:00:00.000Z"),
                started_before: Some("2026-06-03T10:30:00.000Z"),
                valid_only: false,
                limit: None,
                offset: None,
            })
            .expect("filtered audit count"),
        1
    );
}

#[test]
fn llm_request_audit_rows_derive_transport_from_request_body_wire() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    // Insertable wire (write whitelist) + expected transport.
    let insertable = [
        (
            "transport-http",
            Some(
                r#"{"format":"provider_request_v1","version":1,"method":"POST","url":"https://example.test","headers":{},"body":null}"#,
            ),
            LlmRequestTransport::Http,
        ),
        (
            "transport-ws-format",
            Some(
                r#"{"format":"provider_websocket_request_v1","version":1,"url":"wss://example.test/v1/responses","headers":{},"createFrame":"{}","frameSent":true,"connectionReused":false}"#,
            ),
            LlmRequestTransport::Websocket,
        ),
        (
            "transport-ws-method",
            Some(
                r#"{"format":"provider_request_v1","version":1,"method":"WEBSOCKET","url":"wss://example.test","headers":{},"body":null}"#,
            ),
            LlmRequestTransport::Websocket,
        ),
        (
            "transport-ws-method-case",
            Some(
                r#"{"format":"provider_request_v1","version":1,"method":"websocket","url":"wss://example.test","headers":{},"body":null}"#,
            ),
            LlmRequestTransport::Websocket,
        ),
        ("transport-null", None, LlmRequestTransport::Unknown),
    ];

    // Historical / non-whitelist bodies planted after insert (NULL detail first).
    let planted = [
        ("transport-empty", "   ", LlmRequestTransport::Unknown),
        (
            "transport-invalid-json",
            "{not-json",
            LlmRequestTransport::Unknown,
        ),
        (
            "transport-unsupported",
            r#"{"format":"legacy_text_v1","version":1}"#,
            LlmRequestTransport::Unknown,
        ),
        (
            "transport-wrong-version",
            r#"{"format":"provider_request_v1","version":2,"method":"POST","url":"https://example.test","headers":{},"body":null}"#,
            LlmRequestTransport::Unknown,
        ),
        // SQLite coerces JSON true/1.0 to numeric 1; list+detail must still be unknown.
        (
            "transport-version-bool",
            r#"{"format":"provider_request_v1","version":true,"method":"POST","url":"https://example.test","headers":{},"body":null}"#,
            LlmRequestTransport::Unknown,
        ),
        (
            "transport-version-real",
            r#"{"format":"provider_request_v1","version":1.0,"method":"POST","url":"https://example.test","headers":{},"body":null}"#,
            LlmRequestTransport::Unknown,
        ),
        (
            "transport-version-exp",
            r#"{"format":"provider_websocket_request_v1","version":1e0,"url":"wss://example.test","headers":{},"createFrame":"{}","frameSent":true,"connectionReused":false}"#,
            LlmRequestTransport::Unknown,
        ),
        (
            "transport-version-string",
            r#"{"format":"provider_request_v1","version":"1","method":"POST","url":"https://example.test","headers":{},"body":null}"#,
            LlmRequestTransport::Unknown,
        ),
    ];

    let mut index = 0usize;
    for (id, body, expected) in &insertable {
        database
            .insert_llm_request(NewLlmRequest {
                id,
                workspace_id: "workspace-1",
                chat_id: None,
                request_kind: "chat completion",
                agent_team_id: None,
                agent_instance_id: None,
                agent_task_id: None,
                agent_attempt_id: None,
                // Provider kind must not influence transport classification.
                provider_id: "openai-responses-websocket",
                model_id: "gpt-test",
                thinking_level: None,
                request_started_at: &format!("2026-07-19T10:00:{index:02}.000Z"),
                first_token_at: None,
                completed_at: Some(&format!("2026-07-19T10:00:{index:02}.100Z")),
                input_tokens: Some(1),
                output_tokens: Some(1),
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
                first_token_latency_ms: None,
                total_latency_ms: Some(100),
                status_code: Some(200),
                final_state: "succeeded",
                request_body_json: *body,
                response_body_json: None,
            })
            .unwrap_or_else(|error| panic!("insert {id}: {error}"));
        assert_eq!(
            LlmRequestTransport::from_request_body_json(*body),
            *expected,
            "rust helper for {id}"
        );
        index += 1;
    }

    for (id, body, expected) in &planted {
        database
            .insert_llm_request(NewLlmRequest {
                id,
                workspace_id: "workspace-1",
                chat_id: None,
                request_kind: "chat completion",
                agent_team_id: None,
                agent_instance_id: None,
                agent_task_id: None,
                agent_attempt_id: None,
                provider_id: "openai-responses-websocket",
                model_id: "gpt-test",
                thinking_level: None,
                request_started_at: &format!("2026-07-19T10:00:{index:02}.000Z"),
                first_token_at: None,
                completed_at: Some(&format!("2026-07-19T10:00:{index:02}.100Z")),
                input_tokens: Some(1),
                output_tokens: Some(1),
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
                first_token_latency_ms: None,
                total_latency_ms: Some(100),
                status_code: Some(200),
                final_state: "succeeded",
                request_body_json: None,
                response_body_json: None,
            })
            .unwrap_or_else(|error| panic!("insert shell {id}: {error}"));
        assert_eq!(
            LlmRequestTransport::from_request_body_json(Some(body)),
            *expected,
            "rust helper for planted {id}"
        );
        index += 1;
    }

    {
        let connection = Connection::open(database.database_path()).expect("open db");
        for (id, body, _) in &planted {
            connection
                .execute(
                    "UPDATE llm_requests SET request_body_json = ?2 WHERE id = ?1",
                    params![id, body],
                )
                .unwrap_or_else(|error| panic!("plant {id}: {error}"));
        }
    }

    let rows = database
        .llm_request_audit_rows(LlmRequestAuditFilters {
            limit: Some(100),
            offset: Some(0),
            ..LlmRequestAuditFilters::default()
        })
        .expect("audit rows");
    let by_id: std::collections::HashMap<_, _> =
        rows.into_iter().map(|row| (row.id.clone(), row)).collect();

    for (id, _body, expected) in &insertable {
        let row = by_id.get(*id).unwrap_or_else(|| panic!("missing row {id}"));
        assert_eq!(row.transport, *expected, "sql derived transport for {id}");
        assert_eq!(row.provider_id, "openai-responses-websocket");
    }
    for (id, _body, expected) in &planted {
        let row = by_id.get(*id).unwrap_or_else(|| panic!("missing row {id}"));
        assert_eq!(row.transport, *expected, "sql derived transport for {id}");
    }

    let rows_sql = llm_request_audit_rows_sql_for_tests(LlmRequestAuditFilters::default());
    assert!(
        rows_sql.contains("provider_websocket_request_v1"),
        "list SQL should classify websocket wire format"
    );
    assert!(
        rows_sql.contains("AS transport"),
        "list SQL should expose derived transport column"
    );
    assert!(
        rows_sql.contains("json_type(request_body_json, '$.version') = 'integer'"),
        "list SQL must require integer JSON version so true/1.0 stay unknown"
    );
    // Derived CASE may reference request_body_json; the SELECT list must not project it.
    assert!(
        !rows_sql.contains("request_body_json AS") && !rows_sql.contains(", request_body_json"),
        "list SQL must not project request_body_json into the result set"
    );
}

#[test]
fn llm_request_event_retries_are_idempotent_by_request_sequence() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .insert_llm_request(NewLlmRequest {
            id: "request-1",
            workspace_id: "workspace-1",
            chat_id: None,
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-07-16T00:00:00Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: None,
            final_state: "running",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("request insert");

    for id in ["event-first", "event-retry"] {
        database
            .insert_llm_request_event(NewLlmRequestEvent {
                id,
                llm_request_id: "request-1",
                sequence: 0,
                event_at: "2026-07-16T00:00:01Z",
                event_type: "text_delta",
                raw_chunk_json: None,
                normalized_event_json: r#"{"type":"text_delta","text":"hello"}"#,
            })
            .expect("idempotent event insert");
    }

    let events = database
        .llm_request_events("request-1")
        .expect("request events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, "event-first");
}

#[test]
fn concurrent_llm_request_event_retries_share_one_sequence() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .insert_llm_request(NewLlmRequest {
            id: "request-1",
            workspace_id: "workspace-1",
            chat_id: None,
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-07-16T00:00:00Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: None,
            final_state: "running",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("request insert");
    drop(database);

    let workspace_path = Arc::new(workspace.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(2));
    let threads = (0..2)
        .map(|index| {
            let workspace_path = Arc::clone(&workspace_path);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut database =
                    WorkspaceDatabase::open_or_create_ungated(workspace_path.as_path())?;
                barrier.wait();
                let id = format!("event-{index}");
                database.insert_llm_request_event(NewLlmRequestEvent {
                    id: &id,
                    llm_request_id: "request-1",
                    sequence: 0,
                    event_at: "2026-07-16T00:00:01Z",
                    event_type: "text_delta",
                    raw_chunk_json: None,
                    normalized_event_json: r#"{"type":"text_delta","text":"hello"}"#,
                })
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        thread
            .join()
            .expect("event writer thread")
            .expect("concurrent event insert");
    }

    let database = WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    assert_eq!(
        database
            .llm_request_events("request-1")
            .expect("request events")
            .len(),
        1
    );
}

#[test]
fn llm_request_outcome_and_events_roll_back_together() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .insert_llm_request(NewLlmRequest {
            id: "request-1",
            workspace_id: "workspace-1",
            chat_id: None,
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-07-16T00:00:00Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: None,
            final_state: "running",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("request insert");
    database
        .insert_llm_request_event(NewLlmRequestEvent {
            id: "shared-event-id",
            llm_request_id: "request-1",
            sequence: 9,
            event_at: "2026-07-16T00:00:01Z",
            event_type: "existing",
            raw_chunk_json: None,
            normalized_event_json: r#"{"type":"existing"}"#,
        })
        .expect("existing event insert");

    let result = database.update_llm_request_outcome_with_events(
        "request-1",
        UpdateLlmRequestOutcome {
            first_token_at: Some("2026-07-16T00:00:00.100Z"),
            completed_at: Some("2026-07-16T00:00:02Z"),
            input_tokens: Some(10),
            output_tokens: Some(5),
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: Some(100),
            total_latency_ms: Some(2000),
            status_code: Some(200),
            final_state: "succeeded",
            response_body_json: None,
        },
        &[NewLlmRequestEvent {
            id: "shared-event-id",
            llm_request_id: "request-1",
            sequence: 0,
            event_at: "2026-07-16T00:00:02Z",
            event_type: "finish",
            raw_chunk_json: None,
            normalized_event_json: r#"{"type":"finish"}"#,
        }],
    );
    assert!(result.is_err(), "event failure must abort the transaction");

    let request = database
        .llm_request("request-1")
        .expect("request lookup")
        .expect("request row");
    assert_eq!(request.final_state, "running");
    assert_eq!(request.completed_at, None);
}

#[test]
fn versioned_provider_http_headers_only_mask_authorization() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_llm_request(NewLlmRequest {
            id: "provider-http-headers",
            workspace_id: "workspace-1",
            chat_id: None,
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai-chat",
            model_id: "gpt-audit",
            thinking_level: None,
            request_started_at: "2026-07-12T08:00:00.000Z",
            first_token_at: None,
            completed_at: Some("2026-07-12T08:00:01.000Z"),
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: Some(1000),
            status_code: Some(200),
            final_state: "succeeded",
            request_body_json: Some(
                r#"{
                    "format":"provider_request_v1",
                    "version":1,
                    "method":"POST",
                    "url":"https://example.test/v1/chat",
                    "headers":{
                        "Authorization":["Bearer request-secret"],
                        "x-api-key":["request-api-key"],
                        "cookie":["session=request-cookie"],
                        "x-provider-signature":["request-signature"]
                    },
                    "body":"{\"apiKey\":\"body-secret\",\"prompt\":\"keep\"}",
                    "metadata":{"password":"metadata-secret"}
                }"#,
            ),
            response_body_json: Some(
                r#"{
                    "format":"provider_final_response_v1",
                    "version":1,
                    "state":"succeeded",
                    "http":{
                        "status":200,
                        "version":"HTTP/1.1",
                        "headers":{
                            "authorization":["Bearer response-secret"],
                            "set-cookie":["session=response-cookie"],
                            "x-api-key":["response-api-key"],
                            "x-provider-signature":["response-signature"]
                        }
                    },
                    "text":"ok",
                    "reasoning":null,
                    "toolCalls":[],
                    "usage":null,
                    "stopReason":"stop",
                    "responseId":null,
                    "metadata":{"apiKey":"response-metadata-secret"}
                }"#,
            ),
        })
        .expect("versioned provider request insert");

    let request = database
        .llm_request("provider-http-headers")
        .expect("provider request read")
        .expect("provider request");
    let request_body: Value = serde_json::from_str(
        request
            .request_body_json
            .as_deref()
            .expect("provider request body"),
    )
    .expect("provider request JSON");
    assert_eq!(request_body["headers"]["Authorization"][0], "********");
    assert_eq!(request_body["headers"]["x-api-key"][0], "request-api-key");
    assert_eq!(
        request_body["headers"]["cookie"][0],
        "session=request-cookie"
    );
    assert_eq!(
        request_body["headers"]["x-provider-signature"][0],
        "request-signature"
    );
    assert_eq!(request_body["metadata"]["password"], "[REDACTED]");

    let response_body: Value = serde_json::from_str(
        request
            .response_body_json
            .as_deref()
            .expect("provider response body"),
    )
    .expect("provider response JSON");
    assert_eq!(
        response_body["http"]["headers"]["authorization"][0],
        "********"
    );
    assert_eq!(
        response_body["http"]["headers"]["set-cookie"][0],
        "session=response-cookie"
    );
    assert_eq!(
        response_body["http"]["headers"]["x-api-key"][0],
        "response-api-key"
    );
    assert_eq!(
        response_body["http"]["headers"]["x-provider-signature"][0],
        "response-signature"
    );
    assert_eq!(response_body["metadata"]["apiKey"], "[REDACTED]");
}

#[test]
fn main_chat_llm_audit_filter_excludes_internal_requests_bound_to_chat() {
    fn request<'a>(
        id: &'a str,
        request_kind: &'a str,
        input_tokens: i64,
        output_tokens: i64,
        request_started_at: &'a str,
        response_body_json: Option<&'a str>,
    ) -> NewLlmRequest<'a> {
        NewLlmRequest {
            id,
            workspace_id: "workspace-1",
            chat_id: Some("chat-1"),
            request_kind,
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "provider",
            model_id: "model",
            thinking_level: None,
            request_started_at,
            first_token_at: None,
            completed_at: Some(request_started_at),
            input_tokens: Some(input_tokens),
            output_tokens: Some(output_tokens),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: Some(100),
            status_code: Some(200),
            final_state: "succeeded",
            request_body_json: None,
            response_body_json,
        }
    }

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Audit filter chat")
        .expect("chat insert");
    let spec_job = database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: "spec-job-1",
            trigger_type: WorkspaceSpecTriggerType::ChatCompleted.as_str(),
            chat_id: Some("chat-1"),
            run_id: Some("run-1"),
            model_id: Some("model"),
            base_revision: Some(1),
            input_summary_json: Some("{}"),
        })
        .expect("spec job insert");
    assert_eq!(spec_job.chat_id.as_deref(), Some("chat-1"));

    for request in [
        request(
            "internal-spec-update-compaction",
            "workspace spec update compaction",
            900,
            90,
            "2026-07-06T10:00:07Z",
            None,
        ),
        request(
            "internal-spec-compaction",
            "workspace spec compaction",
            800,
            80,
            "2026-07-06T10:00:06Z",
            None,
        ),
        request(
            "internal-spec-update",
            "workspace spec update",
            700,
            70,
            "2026-07-06T10:00:05Z",
            None,
        ),
        request(
            "internal-spec-generation",
            "workspace spec generation",
            650,
            65,
            "2026-07-06T10:00:04.500Z",
            None,
        ),
        request(
            "legacy-memory-retrieval",
            "memory retrieval",
            600,
            60,
            "2026-07-06T10:00:04Z",
            None,
        ),
        request(
            "internal-memory-extraction",
            "memory extraction",
            500,
            50,
            "2026-07-06T10:00:03Z",
            None,
        ),
        request(
            "internal-prompt-hook",
            "prompt hook",
            450,
            45,
            "2026-07-06T10:00:02.500Z",
            None,
        ),
        request(
            "main-chat-request",
            "chat completion",
            10,
            5,
            "2026-07-06T10:00:02Z",
            None,
        ),
    ] {
        database
            .insert_llm_request(request)
            .expect("llm request insert");
    }

    let all_chat_rows = database
        .llm_request_audit_rows(LlmRequestAuditFilters {
            chat_id: Some("chat-1"),
            ..LlmRequestAuditFilters::default()
        })
        .expect("all chat audit rows");
    assert_eq!(all_chat_rows.len(), 8);

    let main_chat_filters = LlmRequestAuditFilters {
        chat_id: Some("chat-1"),
        exclude_request_kinds: MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS,
        ..LlmRequestAuditFilters::default()
    };
    let filtered_rows = database
        .llm_request_audit_rows(main_chat_filters)
        .expect("main chat audit rows");
    assert_eq!(filtered_rows.len(), 1);
    assert_eq!(filtered_rows[0].id, "main-chat-request");
    assert_eq!(
        database
            .llm_request_audit_count(main_chat_filters)
            .expect("main chat audit count"),
        1
    );

    let valid_main_chat_filters = LlmRequestAuditFilters {
        valid_only: true,
        ..main_chat_filters
    };
    assert_eq!(
        database
            .llm_request_audit_count(valid_main_chat_filters)
            .expect("valid main chat audit count"),
        1
    );

    Connection::open(database.database_path())
        .expect("open audit database")
        .execute(
            "UPDATE llm_requests
             SET invalidated_at = '2026-07-06T10:01:00Z',
                 invalidated_reason = 'chat message edited'
             WHERE id = 'main-chat-request'",
            [],
        )
        .expect("invalidate main chat request");
    let audit_rows_after_invalidation = database
        .llm_request_audit_rows(main_chat_filters)
        .expect("audit rows after invalidation");
    assert_eq!(audit_rows_after_invalidation.len(), 1);
    assert_eq!(
        audit_rows_after_invalidation[0]
            .invalidated_reason
            .as_deref(),
        Some("chat message edited")
    );
    assert_eq!(
        database
            .llm_request_audit_count(valid_main_chat_filters)
            .expect("valid main chat audit count after invalidation"),
        0
    );
    let invalidated_breakdown = database
        .llm_request_audit_request_kind_breakdown(main_chat_filters)
        .expect("invalidated request kind breakdown");
    assert_eq!(invalidated_breakdown.len(), 1);
    assert_eq!(invalidated_breakdown[0].request_kind, "chat completion");
    assert!(
        database
            .llm_request_audit_request_kind_breakdown(valid_main_chat_filters)
            .expect("valid request kind breakdown after invalidation")
            .is_empty()
    );

    let summary = database
        .llm_request_audit_summary(main_chat_filters)
        .expect("main chat audit summary");
    assert_eq!(summary.total_requests, 1);
    assert_eq!(summary.total_input_tokens, 10);
    assert_eq!(summary.total_output_tokens, 5);
    assert_eq!(summary.total_tokens, 15);

    let request_kind_rows = database
        .llm_request_audit_rows(LlmRequestAuditFilters {
            chat_id: Some("chat-1"),
            request_kind: Some("chat completion"),
            exclude_request_kinds: MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS,
            ..LlmRequestAuditFilters::default()
        })
        .expect("request kind audit rows");
    assert_eq!(request_kind_rows.len(), 1);
    assert_eq!(request_kind_rows[0].id, "main-chat-request");

    for request_kind in [
        "workspace spec generation",
        "workspace spec update",
        "workspace spec compaction",
        "workspace spec update compaction",
    ] {
        assert!(
            MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS.contains(&request_kind),
            "{request_kind} must stay in MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS"
        );
        let rows = database
            .llm_request_audit_rows(LlmRequestAuditFilters {
                chat_id: Some("chat-1"),
                request_kind: Some(request_kind),
                exclude_request_kinds: &[],
                ..LlmRequestAuditFilters::default()
            })
            .expect("explicit workspace spec kind rows");
        assert_eq!(
            rows.len(),
            1,
            "explicit requestKind={request_kind} must still find the audit row"
        );
        assert_eq!(rows[0].request_kind, request_kind);
        assert!(
            database
                .llm_request_audit_rows(LlmRequestAuditFilters {
                    chat_id: Some("chat-1"),
                    request_kind: Some(request_kind),
                    exclude_request_kinds: MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS,
                    ..LlmRequestAuditFilters::default()
                })
                .expect("excluded workspace spec kind rows")
                .is_empty(),
            "default main-chat exclude must hide {request_kind}"
        );
    }
}

#[test]
fn llm_request_audit_request_kind_breakdown_uses_fact_filters_and_sums_usage() {
    fn request<'a>(
        id: &'a str,
        request_kind: &'a str,
        provider_id: &'a str,
        final_state: &'a str,
        request_started_at: &'a str,
        input_tokens: i64,
        output_tokens: i64,
        cache_read_tokens: i64,
        cache_write_tokens: i64,
        reasoning_tokens: i64,
        total_latency_ms: Option<i64>,
    ) -> NewLlmRequest<'a> {
        NewLlmRequest {
            id,
            workspace_id: "workspace-1",
            chat_id: Some("chat-1"),
            request_kind,
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id,
            model_id: "model-1",
            thinking_level: None,
            request_started_at,
            first_token_at: None,
            completed_at: (final_state != "running").then_some(request_started_at),
            input_tokens: Some(input_tokens),
            output_tokens: Some(output_tokens),
            cache_read_tokens: Some(cache_read_tokens),
            cache_write_tokens: Some(cache_write_tokens),
            reasoning_tokens: Some(reasoning_tokens),
            first_token_latency_ms: None,
            total_latency_ms,
            status_code: None,
            final_state,
            request_body_json: None,
            response_body_json: None,
        }
    }

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-1", "Request kind breakdown")
        .expect("chat insert");
    for request in [
        request(
            "chat-success",
            "chat completion",
            "provider-1",
            "succeeded",
            "2026-07-10T10:00:00Z",
            10,
            5,
            2,
            1,
            3,
            Some(100),
        ),
        request(
            "compression-failed",
            "contextCompression",
            "provider-1",
            "failed",
            "2026-07-10T10:01:00Z",
            20,
            4,
            6,
            2,
            8,
            Some(300),
        ),
        request(
            "other-running",
            "prompt hook",
            "provider-2",
            "running",
            "2026-07-10T10:02:00Z",
            7,
            1,
            0,
            0,
            2,
            None,
        ),
    ] {
        database
            .insert_llm_request(request)
            .expect("llm request insert");
    }

    let breakdown = database
        .llm_request_audit_request_kind_breakdown(LlmRequestAuditFilters {
            chat_id: Some("chat-1"),
            provider_id: Some("provider-1"),
            started_after: Some("2026-07-10T10:00:30Z"),
            ..LlmRequestAuditFilters::default()
        })
        .expect("request kind breakdown");
    assert_eq!(breakdown.len(), 1);
    let compression = &breakdown[0];
    assert_eq!(compression.request_kind, "contextCompression");
    assert_eq!(compression.request_count, 1);
    assert_eq!(compression.failed_requests, 1);
    assert_eq!(compression.total_input_tokens, 20);
    assert_eq!(compression.total_output_tokens, 4);
    assert_eq!(compression.total_cache_read_tokens, 6);
    assert_eq!(compression.total_cache_write_tokens, 2);
    assert_eq!(compression.total_reasoning_tokens, 8);
    assert_eq!(compression.total_tokens, 24);
    assert_eq!(compression.latency_count, 1);
    assert_eq!(compression.latency_sum, 300);

    let running = database
        .llm_request_audit_request_kind_breakdown(LlmRequestAuditFilters {
            final_state: Some("running"),
            ..LlmRequestAuditFilters::default()
        })
        .expect("running request kind breakdown");
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].request_kind, "prompt hook");
    assert_eq!(running[0].failed_requests, 1);
    assert_eq!(running[0].latency_count, 0);
}

#[test]
fn migrates_v27_llm_request_kind_and_spec_job_chat_id_defaults() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("v27 database");
    connection
        .execute_batch(
            r#"
            CREATE TABLE chats (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived_at TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}'
            );
            INSERT INTO chats (id, title, created_at, updated_at, metadata_json)
            VALUES ('chat-1', 'Old audit chat', '2026-06-03T09:00:00Z', '2026-06-03T09:00:00Z', '{}');

            CREATE TABLE llm_requests (
                id TEXT PRIMARY KEY NOT NULL,
                chat_id TEXT REFERENCES chats(id) ON DELETE SET NULL,
                provider_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                request_started_at TEXT NOT NULL,
                first_token_at TEXT,
                completed_at TEXT,
                input_tokens INTEGER,
                output_tokens INTEGER,
                cache_read_tokens INTEGER,
                cache_write_tokens INTEGER,
                reasoning_tokens INTEGER,
                first_token_latency_ms INTEGER,
                total_latency_ms INTEGER,
                status_code INTEGER,
                final_state TEXT NOT NULL,
                request_body_json TEXT,
                response_body_json TEXT,
                workspace_id TEXT,
                cache_ratio REAL,
                agent_team_id TEXT,
                agent_instance_id TEXT,
                agent_task_id TEXT,
                agent_attempt_id TEXT
            );
            INSERT INTO llm_requests (
                id, workspace_id, chat_id, provider_id, model_id, request_started_at,
                completed_at, input_tokens, output_tokens, final_state
            ) VALUES (
                'old-request', 'workspace-1', 'chat-1', 'openai-responses', 'gpt-old',
                '2026-06-03T10:00:00Z', '2026-06-03T10:00:01Z', 10, 5, 'completed'
            );

            CREATE TABLE workspace_spec_jobs (
                id TEXT PRIMARY KEY NOT NULL,
                trigger_type TEXT NOT NULL,
                status TEXT NOT NULL,
                run_id TEXT,
                model_id TEXT,
                base_revision INTEGER,
                input_summary_json TEXT NOT NULL DEFAULT '{}',
                output_json TEXT,
                error_message TEXT,
                created_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                retry_of_job_id TEXT REFERENCES workspace_spec_jobs(id) ON DELETE SET NULL
            );
            INSERT INTO workspace_spec_jobs (
                id, trigger_type, status, run_id, model_id, base_revision, input_summary_json, created_at
            ) VALUES (
                'old-spec-job', 'manual_refresh', 'queued', NULL, 'gpt-old', 1, '{}',
                '2026-06-03T10:00:00Z'
            );
            PRAGMA user_version = 27;
            "#,
        )
        .expect("v27 audit/spec schema");
    ensure_messages_table_for_migration_fixture(&connection);
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let request = database
        .llm_request("old-request")
        .expect("old request lookup")
        .expect("old request");
    assert_eq!(request.request_kind, "unknown");
    let rows = database
        .llm_request_audit_rows(LlmRequestAuditFilters {
            request_kind: Some("unknown"),
            ..LlmRequestAuditFilters::default()
        })
        .expect("audit rows by request kind");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "old-request");
    assert_eq!(rows[0].request_kind, "unknown");
    let job = database
        .workspace_spec_job("old-spec-job")
        .expect("old spec job lookup")
        .expect("old spec job");
    assert_eq!(job.chat_id, None);
}

#[test]
fn migrates_v21_llm_requests_into_usage_rollups() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    let database_path = database.database_path().to_path_buf();
    drop(database);
    let connection = Connection::open(&database_path).expect("database rollback to v21");
    connection
        .execute_batch(
            r#"DROP TABLE llm_request_usage_rollups;
             DROP TABLE plan_phase_attempts;
             DROP INDEX workspace_spec_jobs_active_retry_idx;
             ALTER TABLE workspace_spec_jobs DROP COLUMN retry_of_job_id;
             ALTER TABLE plans DROP COLUMN shared_merge_commit_id;
             INSERT INTO llm_requests (
                id, workspace_id, provider_id, model_id, request_started_at,
                input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                total_latency_ms, final_state
             ) VALUES
                ('request-1', 'workspace-1', 'openai', 'gpt-old',
                 '2026-06-03T10:00:00Z', 10, 4, 2, 1, 120, 'completed'),
                ('request-2', 'workspace-1', 'openai', 'gpt-old',
                 '2026-06-03T11:00:00Z', 5, 3, NULL, NULL, NULL, 'failed'),
                ('request-running', 'workspace-1', 'openai', 'gpt-old',
                 '2026-06-03T12:00:00Z', 100, 100, NULL, NULL, NULL, 'running');
             PRAGMA user_version = 21;"#,
        )
        .expect("v21 llm request fixture");
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let connection = Connection::open(database.database_path()).expect("open migrated database");
    let rollups = connection
        .prepare(
            "SELECT final_state, request_count, success_count, failed_count,
                    total_input_tokens, total_output_tokens,
                    total_cache_read_tokens, total_cache_write_tokens,
                    total_tokens, latency_count, latency_sum
             FROM llm_request_usage_rollups
             ORDER BY final_state",
        )
        .expect("rollup query")
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
            ))
        })
        .expect("rollup rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect rollups");

    assert_eq!(
        rollups,
        vec![
            ("completed".to_string(), 1, 1, 0, 10, 4, 2, 1, 14, 1, 120),
            ("failed".to_string(), 1, 0, 1, 5, 3, 0, 0, 8, 0, 0),
        ]
    );
}

#[test]
fn llm_request_usage_rollup_tracks_delta_and_matches_direct_group_by_after_rebuild() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_llm_request(NewLlmRequest {
            id: "rollup-request-1",
            workspace_id: "workspace-1",
            chat_id: None,
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-rollup",
            thinking_level: None,
            request_started_at: "2026-06-03T10:00:00.000Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: None,
            final_state: "running",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("running llm request insert");

    let connection = Connection::open(database.database_path()).expect("open database");
    let rollup_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM llm_request_usage_rollups",
            [],
            |row| row.get(0),
        )
        .expect("rollup count");
    assert_eq!(rollup_count, 0);

    database
        .update_llm_request_outcome(
            "rollup-request-1",
            UpdateLlmRequestOutcome {
                first_token_at: Some("2026-06-03T10:00:00.050Z"),
                completed_at: Some("2026-06-03T10:00:00.120Z"),
                input_tokens: Some(10),
                output_tokens: Some(4),
                cache_read_tokens: Some(2),
                cache_write_tokens: Some(1),
                reasoning_tokens: None,
                first_token_latency_ms: Some(50),
                total_latency_ms: Some(120),
                status_code: Some(200),
                final_state: "succeeded",
                response_body_json: None,
            },
        )
        .expect("final llm request update");

    database
        .update_llm_request_outcome(
            "rollup-request-1",
            UpdateLlmRequestOutcome {
                first_token_at: Some("2026-06-03T10:00:00.050Z"),
                completed_at: Some("2026-06-03T10:00:00.150Z"),
                input_tokens: Some(15),
                output_tokens: Some(5),
                cache_read_tokens: Some(3),
                cache_write_tokens: Some(2),
                reasoning_tokens: None,
                first_token_latency_ms: Some(50),
                total_latency_ms: Some(150),
                status_code: Some(200),
                final_state: "succeeded",
                response_body_json: None,
            },
        )
        .expect("final llm request token correction");

    database
        .insert_llm_request(NewLlmRequest {
            id: "rollup-request-2",
            workspace_id: "workspace-1",
            chat_id: None,
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "anthropic",
            model_id: "claude-rollup",
            thinking_level: None,
            request_started_at: "2026-06-04T10:00:00.000Z",
            first_token_at: None,
            completed_at: Some("2026-06-04T10:00:00.200Z"),
            input_tokens: Some(3),
            output_tokens: Some(7),
            cache_read_tokens: None,
            cache_write_tokens: Some(5),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: Some(200),
            status_code: Some(500),
            final_state: "failed",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("failed llm request insert");

    database
        .insert_llm_request(NewLlmRequest {
            id: "rollup-request-running",
            workspace_id: "workspace-1",
            chat_id: None,
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-rollup",
            thinking_level: None,
            request_started_at: "2026-06-05T10:00:00.000Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: Some(100),
            output_tokens: Some(100),
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: None,
            final_state: "running",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("second running llm request insert");

    let corrected_rollup: (i64, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT request_count, success_count, failed_count,
                    total_tokens, latency_count, latency_sum
             FROM llm_request_usage_rollups
             WHERE workspace_id = 'workspace-1'
               AND bucket_date = '2026-06-03'
               AND provider_id = 'openai'
               AND model_id = 'gpt-rollup'
               AND final_state = 'succeeded'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("corrected rollup row");
    assert_eq!(corrected_rollup, (1, 1, 0, 20, 1, 150));

    database
        .rebuild_llm_request_usage_rollups()
        .expect("rollup rebuild");

    let rollup_filters = LlmRequestUsageRollupFilters {
        workspace_id: Some("workspace-1"),
        ..LlmRequestUsageRollupFilters::default()
    };
    let rollup_summary = database
        .llm_request_usage_rollup_summary(rollup_filters)
        .expect("rollup summary");
    let direct_summary: (i64, i64, i64, i64, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT COUNT(*),
                    COUNT(CASE WHEN final_state NOT IN ('succeeded', 'completed') THEN 1 END),
                    COALESCE(SUM(COALESCE(input_tokens, 0)), 0),
                    COALESCE(SUM(COALESCE(output_tokens, 0)), 0),
                    COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0),
                    COALESCE(SUM(COALESCE(cache_write_tokens, 0)), 0),
                    COALESCE(SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)), 0),
                    COUNT(total_latency_ms),
                    COALESCE(SUM(COALESCE(total_latency_ms, 0)), 0)
             FROM llm_requests
             WHERE workspace_id = 'workspace-1' AND final_state != 'running'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .expect("direct summary");
    assert_eq!(
        (
            rollup_summary.total_requests,
            rollup_summary.failed_requests,
            rollup_summary.total_input_tokens,
            rollup_summary.total_output_tokens,
            rollup_summary.total_cache_read_tokens,
            rollup_summary.total_cache_write_tokens,
            rollup_summary.total_tokens,
            rollup_summary.latency_count,
            rollup_summary.latency_sum,
        ),
        direct_summary
    );

    let rollup_trend: Vec<_> = database
        .llm_request_usage_rollup_trend_breakdown(rollup_filters)
        .expect("rollup trend")
        .into_iter()
        .map(|row| (row.bucket, row.request_count, row.total_tokens))
        .collect();
    let mut statement = connection
        .prepare(
            "SELECT SUBSTR(request_started_at, 1, 10), COUNT(*),
                    SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0))
             FROM llm_requests
             WHERE workspace_id = 'workspace-1' AND final_state != 'running'
             GROUP BY 1 ORDER BY 1 DESC",
        )
        .expect("direct trend statement");
    let direct_trend: Vec<(String, i64, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("direct trend query")
        .collect::<Result<Vec<_>, _>>()
        .expect("direct trend rows");
    assert_eq!(rollup_trend, direct_trend);

    let rollup_providers: Vec<_> = database
        .llm_request_usage_rollup_provider_breakdown(rollup_filters)
        .expect("rollup provider breakdown")
        .into_iter()
        .map(|row| {
            (
                row.provider_id,
                row.request_count,
                row.success_count,
                row.total_tokens,
                row.latency_count,
                row.latency_sum,
            )
        })
        .collect();
    let mut statement = connection
        .prepare(
            "SELECT provider_id,
                    COUNT(*),
                    COUNT(CASE WHEN final_state IN ('succeeded', 'completed') THEN 1 END),
                    SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)),
                    COUNT(total_latency_ms),
                    SUM(COALESCE(total_latency_ms, 0))
             FROM llm_requests
             WHERE workspace_id = 'workspace-1' AND final_state != 'running'
             GROUP BY provider_id ORDER BY provider_id",
        )
        .expect("direct provider statement");
    let direct_providers: Vec<(String, i64, i64, i64, i64, i64)> = statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .expect("direct provider query")
        .collect::<Result<Vec<_>, _>>()
        .expect("direct provider rows");
    assert_eq!(rollup_providers, direct_providers);
}

#[test]
fn prunes_llm_request_details_without_deleting_statistics() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    database
        .insert_llm_request(NewLlmRequest {
            id: "old-request",
            workspace_id: "workspace-1",
            chat_id: None,
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-audit",
            thinking_level: None,
            request_started_at: "2026-06-01T00:00:00.000Z",
            first_token_at: Some("2026-06-01T00:00:00.100Z"),
            completed_at: Some("2026-06-01T00:00:01.000Z"),
            input_tokens: Some(10),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            reasoning_tokens: None,
            first_token_latency_ms: Some(100),
            total_latency_ms: Some(1000),
            status_code: Some(200),
            final_state: "succeeded",
            request_body_json: Some(
                r#"{"format":"provider_request_v1","version":1,"method":"POST","url":"https://example.test","headers":{},"body":"old"}"#,
            ),
            response_body_json: Some(
                r#"{"format":"provider_final_response_v1","version":1,"state":"succeeded","partial":false,"text":"old","reasoning":null,"toolCalls":[],"usage":null,"stopReason":null,"responseId":null,"error":null,"http":null}"#,
            ),
        })
        .expect("old request insert");
    database
        .insert_llm_request_event(NewLlmRequestEvent {
            id: "old-request-event-0",
            llm_request_id: "old-request",
            sequence: 0,
            event_at: "2026-06-01T00:00:00.000Z",
            event_type: "start",
            raw_chunk_json: None,
            normalized_event_json: r#"{"type":"start","assistantMessageId":"message-1"}"#,
        })
        .expect("old start event insert");
    database
        .insert_llm_request_event(NewLlmRequestEvent {
            id: "old-request-event-1",
            llm_request_id: "old-request",
            sequence: 1,
            event_at: "2026-06-01T00:00:00.500Z",
            event_type: "tool_call",
            raw_chunk_json: None,
            normalized_event_json: r#"{"type":"toolCall","toolCall":{"callId":"call-1"}}"#,
        })
        .expect("old detail event insert");
    database
        .insert_llm_request(NewLlmRequest {
            id: "new-request",
            workspace_id: "workspace-1",
            chat_id: None,
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-audit",
            thinking_level: None,
            request_started_at: "2026-06-05T00:00:00.000Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: Some(7),
            output_tokens: Some(3),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: None,
            final_state: "running",
            request_body_json: Some(
                r#"{"format":"provider_request_v1","version":1,"method":"POST","url":"https://example.test","headers":{},"body":"keep"}"#,
            ),
            response_body_json: None,
        })
        .expect("new request insert");

    let pruned = database
        .prune_llm_request_details_before("2026-06-03T00:00:00.000Z")
        .expect("prune request details");
    assert_eq!(pruned, 2);

    let old_request = database
        .llm_request("old-request")
        .expect("old request read")
        .expect("old request");
    assert_eq!(old_request.input_tokens, Some(10));
    assert_eq!(old_request.output_tokens, Some(5));
    assert_eq!(old_request.request_body_json, None);
    assert_eq!(old_request.response_body_json, None);
    let old_events = database
        .llm_request_events("old-request")
        .expect("old events read");
    assert_eq!(old_events.len(), 1);
    assert_eq!(old_events[0].event_type, "start");

    let new_request = database
        .llm_request("new-request")
        .expect("new request read")
        .expect("new request");
    let kept_body: serde_json::Value = serde_json::from_str(
        new_request
            .request_body_json
            .as_deref()
            .expect("kept request body"),
    )
    .expect("kept body json");
    assert_eq!(kept_body["format"], "provider_request_v1");
    assert_eq!(kept_body["body"], "keep");
}

#[test]
fn vacuum_reclaims_workspace_database_freelist_pages() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    let large_input = "x".repeat(1024 * 1024);
    let large_body = format!(
        r#"{{"format":"provider_request_v1","version":1,"method":"POST","url":"https://example.test","headers":{{}},"body":"{large_input}"}}"#
    );

    database
        .insert_llm_request(NewLlmRequest {
            id: "large-old-request",
            workspace_id: "workspace-1",
            chat_id: None,
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-audit",
            thinking_level: None,
            request_started_at: "2026-06-01T00:00:00.000Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: Some(10),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: None,
            final_state: "running",
            request_body_json: Some(&large_body),
            response_body_json: None,
        })
        .expect("large old request insert");

    let before_prune = database.space_stats().expect("stats before prune");
    database
        .prune_llm_request_details_before("2026-06-03T00:00:00.000Z")
        .expect("prune large old request");
    let after_prune = database.space_stats().expect("stats after prune");
    assert!(after_prune.free_bytes() > before_prune.free_bytes());

    database.vacuum().expect("vacuum workspace database");
    let after_vacuum = database.space_stats().expect("stats after vacuum");
    assert!(after_vacuum.file_bytes() < after_prune.file_bytes());
    assert!(after_vacuum.free_bytes() < after_prune.free_bytes());
}

#[test]
fn stores_prompt_context_injections_for_chat_replay() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
            memory_summaries_json: r#"[{"id":"fact-1"}]"#,
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
            memory_summaries_json: r#"[{"id":"fact-2"}]"#,
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
    assert_eq!(injections[1].memory_summaries_json, r#"[{"id":"fact-2"}]"#);
}

#[test]
fn prompt_context_injections_upsert_their_logical_replay_slots() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .insert_chat("chat-1", "Prompt cache chat")
        .expect("chat insert");

    for (id, kind, sequence, messages_json) in [
        (
            "stable-first",
            "stable",
            None,
            r#"[{"role":"system","content":"old stable"}]"#,
        ),
        (
            "stable-retry",
            "stable",
            None,
            r#"[{"role":"system","content":"new stable"}]"#,
        ),
        (
            "turn-first",
            "turn_memory",
            Some(3),
            r#"[{"role":"user","content":"old turn"}]"#,
        ),
        (
            "turn-retry",
            "turn_memory",
            Some(3),
            r#"[{"role":"user","content":"new turn"}]"#,
        ),
    ] {
        database
            .insert_prompt_context_injection(NewPromptContextInjection {
                id,
                chat_id: "chat-1",
                kind,
                sequence,
                messages_json,
                memory_keys_json: r#"["workspace:fact-1"]"#,
                memory_summaries_json: r#"[{"id":"fact-1"}]"#,
            })
            .expect("prompt context injection upsert");
    }

    let injections = database
        .prompt_context_injections_for_chat("chat-1")
        .expect("injections");
    assert_eq!(injections.len(), 2);
    assert_eq!(injections[0].id, "stable-first");
    assert!(injections[0].messages_json.contains("new stable"));
    assert_eq!(injections[1].id, "turn-first");
    assert!(injections[1].messages_json.contains("new turn"));
}

#[test]
fn concurrent_prompt_context_retries_share_one_logical_slot() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .insert_chat("chat-1", "Concurrent prompt cache")
        .expect("chat insert");
    drop(database);

    let workspace_path = Arc::new(workspace.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(2));
    let threads = (0..2)
        .map(|index| {
            let workspace_path = Arc::clone(&workspace_path);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut database =
                    WorkspaceDatabase::open_or_create_ungated(workspace_path.as_path())?;
                barrier.wait();
                let id = format!("stable-{index}");
                let messages_json =
                    format!(r#"[{{"role":"system","content":"stable payload {index}"}}]"#);
                database.insert_prompt_context_injection(NewPromptContextInjection {
                    id: &id,
                    chat_id: "chat-1",
                    kind: "stable",
                    sequence: None,
                    messages_json: &messages_json,
                    memory_keys_json: "[]",
                    memory_summaries_json: "[]",
                })
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        thread
            .join()
            .expect("prompt context writer thread")
            .expect("concurrent prompt context upsert");
    }

    let database = WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let injections = database
        .prompt_context_injections_for_chat("chat-1")
        .expect("injections");
    assert_eq!(injections.len(), 1);
}

#[test]
fn prompt_context_injection_memory_summaries_round_trip_structured_json() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .insert_chat("chat-1", "Memory summary chat")
        .expect("chat insert");

    let memory_summaries_json = r#"[{"id":"fact-1","scope":"workspace","chatId":null,"kind":"project_fact","fact":"Structured summary","pinned":false,"source":"direct"}]"#;
    database
        .insert_prompt_context_injection(NewPromptContextInjection {
            id: "turn-1",
            chat_id: "chat-1",
            kind: "turn_memory",
            sequence: Some(0),
            messages_json: r#"[{"role":"user","content":"Memory context"}]"#,
            memory_keys_json: r#"["workspace:fact-1"]"#,
            memory_summaries_json,
        })
        .expect("prompt context injection insert");

    let injections = database
        .prompt_context_injections_for_chat("chat-1")
        .expect("injections");
    assert_eq!(injections.len(), 1);
    assert_eq!(injections[0].memory_summaries_json, memory_summaries_json);

    let parsed: Value = serde_json::from_str(&injections[0].memory_summaries_json)
        .expect("memory summaries json should parse");
    assert_eq!(parsed[0]["id"], "fact-1");
    assert_eq!(parsed[0]["source"], "direct");
}

#[test]
fn migrates_prompt_context_injection_memory_summaries_default() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("v29 database");
    connection
        .execute_batch(
            r#"CREATE TABLE chats (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
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
             INSERT INTO chats (id, title, created_at, updated_at)
                VALUES ('chat-1', 'Existing', '2026-07-09T00:00:00Z', '2026-07-09T00:00:00Z');
             INSERT INTO prompt_context_injections
                (id, chat_id, kind, sequence, messages_json, memory_keys_json, created_at)
                VALUES ('inj-1', 'chat-1', 'turn_memory', 0, '[{"role":"user","content":"Memory"}]', '["workspace:fact-1"]', '2026-07-09T00:00:00Z');
             PRAGMA user_version = 29;"#,
        )
        .expect("v29 prompt context schema");
    ensure_messages_table_for_migration_fixture(&connection);
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let connection = Connection::open(database.database_path()).expect("open migrated database");
    assert!(column_exists(
        &connection,
        "prompt_context_injections",
        "memory_summaries_json"
    ));
    let memory_summaries_json: String = connection
        .query_row(
            "SELECT memory_summaries_json FROM prompt_context_injections WHERE id = 'inj-1'",
            [],
            |row| row.get(0),
        )
        .expect("memory summaries default");
    assert_eq!(memory_summaries_json, "[]");
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
             INSERT INTO chats (id, title, created_at, updated_at)
                VALUES ('chat-existing', 'Existing', '2026-06-19T00:00:00Z', '2026-06-19T00:00:00Z');
             PRAGMA user_version = 9;",
        )
        .expect("v9 schema");
    // ponytail: three chats cover the no-backfill invariant without turning this migration test into a bulk benchmark.
    for index in 0..2 {
        connection
            .execute(
                "INSERT INTO chats (id, title, created_at, updated_at)
                 VALUES (?1, ?2, '2026-06-19T00:00:00Z', '2026-06-19T00:00:00Z')",
                params![format!("chat-bulk-{index}"), format!("Bulk {index}")],
            )
            .expect("bulk chat insert");
    }
    ensure_messages_table_for_migration_fixture(&connection);
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let connection = Connection::open(database.database_path()).expect("open migrated database");
    assert_eq!(table_count(&connection, "agent_teams"), 0);
    assert_eq!(table_count(&connection, "chats"), 3);
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
    add_workspace_chats_table(&connection);
    add_workspace_memory_tables(&connection);
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let connection = Connection::open(database.database_path()).expect("open migrated database");
    assert_no_agent_messages_old_references(&connection);
}

#[test]
fn migrates_v14_scheduled_task_tables_without_losing_existing_data() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("v14 database");
    connection
        .execute_batch(
            r#"CREATE TABLE chats (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                metadata_json TEXT NOT NULL DEFAULT '{}'
             );
             CREATE TABLE messages (
                id TEXT PRIMARY KEY NOT NULL,
                chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                metadata_json TEXT NOT NULL DEFAULT '{}'
             );
             CREATE TABLE agent_teams (
                id TEXT PRIMARY KEY NOT NULL,
                chat_id TEXT NOT NULL
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
                task_id TEXT NOT NULL,
                UNIQUE (team_id, id)
             );
             CREATE TABLE llm_requests (
                id TEXT PRIMARY KEY NOT NULL,
                chat_id TEXT REFERENCES chats(id) ON DELETE SET NULL,
                agent_team_id TEXT REFERENCES agent_teams(id) ON DELETE SET NULL,
                agent_task_id TEXT REFERENCES agent_tasks(id) ON DELETE SET NULL,
                agent_attempt_id TEXT REFERENCES agent_attempts(id) ON DELETE SET NULL,
                provider_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                request_started_at TEXT NOT NULL,
                final_state TEXT NOT NULL
             );
             INSERT INTO chats (id, title, created_at, updated_at)
                VALUES ('chat-existing', 'Existing', '2026-06-22T00:00:00Z', '2026-06-22T00:00:00Z');
             INSERT INTO messages (id, chat_id, role, content, sequence, created_at)
                VALUES ('message-existing', 'chat-existing', 'user', 'keep me', 0, '2026-06-22T00:00:00Z');
             INSERT INTO agent_teams (id, chat_id)
                VALUES ('agent-team-existing', 'chat-existing');
             INSERT INTO agent_instances (id, team_id)
                VALUES ('agent-instance-existing', 'agent-team-existing');
             INSERT INTO agent_tasks (id, team_id)
                VALUES ('agent-task-existing', 'agent-team-existing');
             INSERT INTO agent_attempts (id, team_id, task_id)
                VALUES ('agent-attempt-existing', 'agent-team-existing', 'agent-task-existing');
             INSERT INTO llm_requests
                (id, chat_id, agent_team_id, agent_task_id, agent_attempt_id,
                 provider_id, model_id, request_started_at, final_state)
                VALUES
                ('request-existing', 'chat-existing', 'agent-team-existing',
                 'agent-task-existing', 'agent-attempt-existing',
                 'provider-test', 'model-test', '2026-06-22T00:00:00Z', 'completed');
             PRAGMA user_version = 14;"#,
        )
        .expect("v14 schema");
    add_workspace_memory_tables(&connection);
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );

    let connection = Connection::open(database.database_path()).expect("open migrated database");
    assert!(table_exists(&connection, "scheduled_tasks"));
    assert!(table_exists(&connection, "scheduled_task_runs"));
    assert_eq!(table_count(&connection, "chats"), 1);
    assert_eq!(table_count(&connection, "messages"), 1);
    assert_eq!(table_count(&connection, "agent_teams"), 1);
    assert_eq!(table_count(&connection, "agent_tasks"), 1);
    assert_eq!(table_count(&connection, "agent_attempts"), 1);
    assert_eq!(table_count(&connection, "llm_requests"), 1);
}

#[test]
fn migrates_v15_memory_dream_tables() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("v15 database");
    connection
        .execute_batch(
            r#"CREATE TABLE workspace_metadata (
                key TEXT PRIMARY KEY NOT NULL CHECK (length(key) > 0),
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             INSERT INTO workspace_metadata (key, value, updated_at)
                VALUES ('sentinel', 'keep', '2026-06-23T00:00:00Z');
             PRAGMA user_version = 15;"#,
        )
        .expect("v15 schema");
    add_workspace_chats_table(&connection);
    add_workspace_memory_tables(&connection);
    add_workspace_agent_plan_reference_tables(&connection);
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );

    let connection = Connection::open(database.database_path()).expect("open migrated database");
    assert!(table_exists(&connection, "memory_dream_jobs"));
    assert!(table_exists(&connection, "memory_dream_changes"));
    assert!(table_exists(&connection, "memory_references"));
    assert_eq!(
        connection
            .query_row(
                "SELECT value FROM workspace_metadata WHERE key = 'sentinel'",
                [],
                |row| row.get::<_, String>(0)
            )
            .expect("sentinel metadata"),
        "keep"
    );
}

#[test]
fn migrates_v16_memory_references_table() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("v16 database");
    connection
        .execute_batch(&format!(
            "{WORKSPACE_MEMORY_SCHEMA_SQL}
             {WORKSPACE_MEMORY_DREAM_SCHEMA_SQL}
             PRAGMA user_version = 16;"
        ))
        .expect("v16 schema");
    add_workspace_chats_table(&connection);
    add_workspace_agent_plan_reference_tables(&connection);
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );

    let connection = Connection::open(database.database_path()).expect("open migrated database");
    assert!(table_exists(&connection, "memory_references"));
}

#[test]
fn migrates_v18_memory_extraction_jobs_allow_skipped_status() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("v18 database");
    connection
        .execute_batch(&format!(
            r#"CREATE TABLE chats (
                id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
                title TEXT NOT NULL CHECK (length(title) > 0),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived_at TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{{}}'
             );
             {WORKSPACE_MEMORY_SCHEMA_SQL}
             {WORKSPACE_MEMORY_DREAM_SCHEMA_SQL}
             CREATE TABLE memory_references (
                id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
                fact_id TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
                reference_type TEXT NOT NULL CHECK (reference_type IN ('file_path', 'url', 'symbol', 'command', 'ticket', 'external_id')),
                value TEXT NOT NULL CHECK (length(value) > 0),
                normalized_value TEXT NOT NULL CHECK (length(normalized_value) > 0),
                status TEXT NOT NULL CHECK (status IN ('valid', 'invalid', 'ambiguous', 'skipped')),
                metadata_json TEXT NOT NULL DEFAULT '{{}}',
                checked_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE (fact_id, reference_type, normalized_value)
             );
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
                input_summary_json TEXT NOT NULL DEFAULT '{{}}',
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
             INSERT INTO chats (id, title, created_at, updated_at, metadata_json)
             VALUES ('chat-1', 'Chat', '2026-06-26T00:00:00.000Z', '2026-06-26T00:00:00.000Z', '{{}}');
             INSERT INTO memory_extraction_jobs (
                id, scope, chat_id, status, model_id, input_json, output_json,
                error_message, created_at, started_at, completed_at
             ) VALUES (
                'job-1', 'chat', 'chat-1', 'failed', 'model-1', '{{"safe":"ok"}}', NULL,
                'provider failed', '2026-06-26T00:00:00.000Z', NULL, '2026-06-26T00:00:01.000Z'
             );
             PRAGMA user_version = 18;"#
        ))
        .expect("v18 schema");
    add_workspace_agent_plan_reference_tables(&connection);
    ensure_messages_table_for_migration_fixture(&connection);
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    drop(database);

    let mut memory_database = MemoryDatabase::open_workspace_at(&database_path).expect("memory db");
    assert!(
        memory_database
            .skip_failed_extraction_job("job-1")
            .expect("skip failed extraction")
    );
    assert_eq!(
        memory_database
            .extraction_job("job-1")
            .expect("job lookup")
            .expect("job exists")
            .status,
        "skipped"
    );
}

#[test]
fn migrates_v30_memory_facts_enabled_column_with_existing_rows() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    fs::create_dir_all(database_path.parent().expect("database parent")).expect("database parent");
    let connection = Connection::open(&database_path).expect("v30 database");
    connection
        .execute_batch(&format!(
            r#"CREATE TABLE chats (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT '2026-01-01T00:00:00Z',
                updated_at TEXT NOT NULL DEFAULT '2026-01-01T00:00:00Z',
                archived_at TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{{}}'
             );
             {WORKSPACE_MEMORY_SCHEMA_SQL}
             ALTER TABLE memory_facts RENAME TO memory_facts_enabled_fixture;
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
                metadata_json TEXT NOT NULL DEFAULT '{{}}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CHECK ((scope = 'chat' AND chat_id IS NOT NULL) OR (scope = 'workspace' AND chat_id IS NULL))
             );
             DROP TABLE memory_facts_enabled_fixture;
             INSERT INTO memory_facts
                (id, scope, chat_id, status, kind, fact, confidence, pinned, is_latest,
                 expires_at, metadata_json, created_at, updated_at)
             VALUES
                ('fact-old', 'workspace', NULL, 'active', 'project_fact', 'Old fact', 0.9,
                 0, 1, NULL, '{{}}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
             PRAGMA user_version = 30;"#
        ))
        .expect("v30 schema");
    ensure_messages_table_for_migration_fixture(&connection);
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrated database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let memory = MemoryDatabase::open_workspace_at(&database_path).expect("memory database");
    assert!(memory.fact("fact-old").unwrap().unwrap().enabled);
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
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).is_err(),
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(&workspace_path).expect("database");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-agent-sequence", "seq");

    let workers = (0..8)
        .map(|index| {
            let workspace_path = workspace_path.clone();
            let team_id = team_id.clone();
            let instance_id = instance_id.clone();
            thread::spawn(move || {
                let mut database = WorkspaceDatabase::open_or_create_ungated(workspace_path)
                    .expect("worker database");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(&workspace_path).expect("database");
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
                let mut database = WorkspaceDatabase::open_or_create_ungated(workspace_path)
                    .expect("scheduler database");
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
fn deferred_workspace_agent_task_waits_for_earlier_active_task() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (first_team_id, first_instance_id) =
        create_test_agent_team(&mut database, "chat-agent-defer-first", "defer-first");
    let (deferred_team_id, deferred_instance_id) =
        create_test_agent_team(&mut database, "chat-agent-defer-second", "defer-second");
    let first_task = AgentTaskId::new("agent-task-defer-first").expect("task id");
    let deferred_task = AgentTaskId::new("agent-task-defer-second").expect("task id");

    database
        .enqueue_agent_task(NewAgentTask {
            id: &first_task,
            team_id: &first_team_id,
            owner_instance_id: &first_instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("first enqueue");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &deferred_task,
            team_id: &deferred_team_id,
            owner_instance_id: &deferred_instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: r#"{"deferUntilWorkspaceIdle":true}"#,
        })
        .expect("deferred enqueue");

    database
        .claim_runnable_agent_task(
            &first_team_id,
            &first_task,
            &AgentAttemptId::new("agent-attempt-defer-first").expect("attempt id"),
        )
        .expect("claim first task")
        .expect("first task claimed");
    assert!(
        database
            .claim_runnable_agent_task(
                &deferred_team_id,
                &deferred_task,
                &AgentAttemptId::new("agent-attempt-defer-second-early").expect("attempt id"),
            )
            .expect("early deferred claim")
            .is_none(),
        "deferred task must wait while an earlier workspace task is active"
    );

    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &first_team_id,
            task_id: &first_task,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Complete,
            result_json: Some(r#"{"ok":true}"#),
            error_json: None,
            interruption_reason: None,
        })
        .expect("complete first task");
    database
        .claim_runnable_agent_task(
            &deferred_team_id,
            &deferred_task,
            &AgentAttemptId::new("agent-attempt-defer-second").expect("attempt id"),
        )
        .expect("claim deferred task")
        .expect("deferred task claimed");
}

#[test]
fn deferred_workspace_agent_task_ignores_earlier_plan_session_task() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (plan_team_id, plan_instance_id) =
        create_test_agent_team(&mut database, "chat-agent-defer-plan", "defer-plan");
    let (deferred_team_id, deferred_instance_id) = create_test_agent_team(
        &mut database,
        "chat-agent-defer-after-plan",
        "defer-after-plan",
    );
    let plan_task = AgentTaskId::new("agent-task-defer-plan").expect("task id");
    let deferred_task = AgentTaskId::new("agent-task-defer-after-plan").expect("task id");

    database
        .enqueue_agent_task(NewAgentTask {
            id: &plan_task,
            team_id: &plan_team_id,
            owner_instance_id: &plan_instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: r#"{"sessionMode":"plan"}"#,
        })
        .expect("plan enqueue");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &deferred_task,
            team_id: &deferred_team_id,
            owner_instance_id: &deferred_instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: r#"{"deferUntilWorkspaceIdle":true}"#,
        })
        .expect("deferred enqueue");

    database
        .claim_runnable_agent_task(
            &plan_team_id,
            &plan_task,
            &AgentAttemptId::new("agent-attempt-defer-plan").expect("attempt id"),
        )
        .expect("claim plan task")
        .expect("plan task claimed");

    let runnable = database.runnable_agent_tasks(10).expect("runnable");
    assert!(
        runnable.iter().any(|task| task.id == deferred_task),
        "deferred task should be runnable while an earlier Plan Mode task is active"
    );
    database
        .claim_runnable_agent_task(
            &deferred_team_id,
            &deferred_task,
            &AgentAttemptId::new("agent-attempt-defer-after-plan").expect("attempt id"),
        )
        .expect("claim deferred task")
        .expect("deferred task claimed");
}

#[test]
fn agent_team_max_concurrent_runs_blocks_second_instance_claim() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-agent-team-limit", "team-limit");
    let worker_id = create_test_agent_worker(&database, &team_id, "team-limit-worker");
    let first_task = AgentTaskId::new("agent-task-team-limit-first").expect("task id");
    let second_task = AgentTaskId::new("agent-task-team-limit-second").expect("task id");

    database
        .enqueue_agent_task(NewAgentTask {
            id: &first_task,
            team_id: &team_id,
            owner_instance_id: &coordinator_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("first enqueue");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &second_task,
            team_id: &team_id,
            owner_instance_id: &worker_id,
            origin_instance_id: Some(&coordinator_id),
            parent_task_id: Some(&first_task),
            input_json: "{}",
        })
        .expect("second enqueue");

    database
        .claim_runnable_agent_task(
            &team_id,
            &first_task,
            &AgentAttemptId::new("agent-attempt-team-limit-first").expect("attempt id"),
        )
        .expect("claim first task")
        .expect("first task claimed");
    assert!(
        database
            .runnable_agent_tasks(10)
            .expect("runnable while team is saturated")
            .is_empty()
    );
    assert!(
        database
            .claim_runnable_agent_task(
                &team_id,
                &second_task,
                &AgentAttemptId::new("agent-attempt-team-limit-blocked").expect("attempt id"),
            )
            .expect("claim blocked second task")
            .is_none(),
        "team max_concurrent_runs=1 must block another running task"
    );

    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &first_task,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Complete,
            result_json: Some(r#"{"text":"done"}"#),
            error_json: None,
            interruption_reason: None,
        })
        .expect("complete first task");
    assert_eq!(
        database.runnable_agent_tasks(10).expect("runnable")[0].id,
        second_task
    );
    database
        .claim_runnable_agent_task(
            &team_id,
            &second_task,
            &AgentAttemptId::new("agent-attempt-team-limit-second").expect("attempt id"),
        )
        .expect("claim second task")
        .expect("second task claimed");
}

#[test]
fn messages_for_chat_filters_worker_agent_assistant_messages() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
            input_json: r#"{"queuedUserMessageId":"user-worker"}"#,
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
            id: "user-worker",
            chat_id: "chat-agent-message-filter",
            role: "user",
            content: "Worker-only prompt",
            sequence: 2,
            metadata_json: None,
        })
        .expect("worker user message insert");
    database
        .insert_message(NewMessage {
            id: "assistant-worker",
            chat_id: "chat-agent-message-filter",
            role: "assistant",
            content: "Worker-only answer",
            sequence: 3,
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
    assert_eq!(
        database
            .next_message_sequence_for_chat("chat-agent-message-filter")
            .expect("next message sequence"),
        4
    );
}

#[test]
fn agent_queue_limits_and_team_lifecycle_are_enforced() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
fn close_pre_stream_chat_failure_writes_assistant_error_and_clears_queued_run() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let chat_id = "chat-prestream-fail";
    let (team_id, instance_id) = create_test_agent_team(&mut database, chat_id, "prestream-fail");
    let user_id = "user-prestream-fail";
    let assistant_id = "assistant-prestream-fail";
    database
        .insert_message(NewMessage {
            id: user_id,
            chat_id,
            role: "user",
            content: "hello",
            sequence: 0,
            metadata_json: Some(
                r#"{"queuedRun":{"status":"running","assistantMessageId":"assistant-prestream-fail","assistantSequence":1,"modelId":"model-test"}}"#,
            ),
        })
        .expect("user insert");
    database
        .set_chat_queued_run(
            chat_id,
            &json!({
                "status": "running",
                "userMessageId": user_id,
                "assistantMessageId": assistant_id,
                "assistantSequence": 1,
                "modelId": "model-test",
            })
            .to_string(),
        )
        .expect("set chat queued run");

    let task_id = AgentTaskId::new("agent-task-prestream-fail").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: &json!({
                "queuedUserMessageId": user_id,
                "visibleAssistantMessageId": assistant_id,
                "visibleAssistantSequence": 1,
                "message": "hello",
            })
            .to_string(),
        })
        .expect("enqueue");
    let attempt_id = AgentAttemptId::new("agent-attempt-prestream-fail").expect("attempt");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
        .expect("claim")
        .expect("claimed");

    let error_json = json!({
        "message": "Reply has not started: workspace database is busy. Please retry.",
        "code": "workspace_database_busy",
        "stage": "pre_stream_prepare",
        "retryable": true,
    })
    .to_string();
    let assistant_metadata = json!({
        "streamingState": "failed",
        "runFailure": {
            "code": "workspace_database_busy",
            "stage": "pre_stream_prepare",
            "retryable": true,
            "taskId": task_id.as_str(),
            "attemptId": attempt_id.as_str(),
            "message": "Reply has not started: workspace database is busy. Please retry.",
        },
        "parts": [{ "type": "error", "text": "Reply has not started: workspace database is busy. Please retry." }],
        "partsVersion": 5,
        "partsSource": "pre_stream_failure",
    })
    .to_string();

    let result = database
        .close_pre_stream_chat_failure(PreStreamChatFailureClosure {
            task_id: &task_id,
            attempt_id: &attempt_id,
            chat_id,
            user_message_id: user_id,
            assistant_message_id: assistant_id,
            assistant_sequence: 1,
            error_json: &error_json,
            assistant_content: "Reply has not started: workspace database is busy. Please retry.",
            assistant_metadata_json: &assistant_metadata,
            materialize_assistant: true,
        })
        .expect("closure");
    assert_eq!(result, PreStreamChatFailureClosureResult::Applied);

    let task = database.agent_task(&task_id).expect("task").expect("task");
    assert_eq!(task.status, AgentTaskStatus::Failed);

    let assistant = database
        .message(assistant_id)
        .expect("assistant")
        .expect("assistant");
    assert_eq!(
        assistant.content,
        "Reply has not started: workspace database is busy. Please retry."
    );
    let metadata: Value =
        serde_json::from_str(&assistant.metadata_json).expect("assistant metadata");
    assert_eq!(metadata["streamingState"], "failed");
    assert_eq!(metadata["runFailure"]["code"], "workspace_database_busy");
    assert_eq!(metadata["runFailure"]["retryable"], true);
    assert_eq!(metadata["parts"][0]["type"], "error");

    let chat = database.chat(chat_id).expect("chat").expect("chat");
    let chat_metadata: Value = serde_json::from_str(&chat.metadata_json).expect("chat metadata");
    assert!(chat_metadata.get("queuedRun").is_none());

    let user = database.message(user_id).expect("user").expect("user");
    let user_metadata: Value = serde_json::from_str(&user.metadata_json).expect("user metadata");
    assert!(user_metadata.get("queuedRun").is_none());

    let events = database.agent_events_after(&team_id, -1).expect("events");
    assert!(
        events.iter().any(|event| event.event_type == "task_failed"
            && event.attempt_id.as_ref() == Some(&attempt_id)),
        "task_failed event for attempt"
    );
}

#[test]
fn close_pre_stream_chat_failure_skips_when_queued_run_replaced() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let chat_id = "chat-prestream-skip";
    let (team_id, instance_id) = create_test_agent_team(&mut database, chat_id, "prestream-skip");
    let user_id = "user-prestream-skip";
    let assistant_id = "assistant-prestream-skip";
    database
        .insert_message(NewMessage {
            id: user_id,
            chat_id,
            role: "user",
            content: "hello",
            sequence: 0,
            metadata_json: Some(
                r#"{"queuedRun":{"status":"running","assistantMessageId":"assistant-new","assistantSequence":3,"modelId":"model-test"}}"#,
            ),
        })
        .expect("user insert");
    // Newer queued run identity (edit re-run).
    database
        .set_chat_queued_run(
            chat_id,
            &json!({
                "status": "running",
                "userMessageId": "user-new",
                "assistantMessageId": "assistant-new",
                "assistantSequence": 3,
                "modelId": "model-test",
            })
            .to_string(),
        )
        .expect("set chat queued run");

    let task_id = AgentTaskId::new("agent-task-prestream-skip").expect("task id");
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
    let attempt_id = AgentAttemptId::new("agent-attempt-prestream-skip").expect("attempt");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
        .expect("claim")
        .expect("claimed");

    // Complete the task first so attempt identity no longer matches running.
    database
        .update_agent_task_state_for_attempt(
            AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &task_id,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Complete,
                result_json: Some(r#"{"ok":true}"#),
                error_json: None,
                interruption_reason: None,
            },
            &attempt_id,
        )
        .expect("complete");

    let result = database
        .close_pre_stream_chat_failure(PreStreamChatFailureClosure {
            task_id: &task_id,
            attempt_id: &attempt_id,
            chat_id,
            user_message_id: user_id,
            assistant_message_id: assistant_id,
            assistant_sequence: 1,
            error_json: r#"{"message":"stale"}"#,
            assistant_content: "stale",
            assistant_metadata_json: r#"{"streamingState":"failed"}"#,
            materialize_assistant: true,
        })
        .expect("closure");
    assert!(matches!(
        result,
        PreStreamChatFailureClosureResult::Skipped { .. }
    ));
    assert!(
        database
            .message(assistant_id)
            .expect("assistant lookup")
            .is_none(),
        "must not insert assistant error when attempt already finished"
    );
    let chat = database.chat(chat_id).expect("chat").expect("chat");
    let chat_metadata: Value = serde_json::from_str(&chat.metadata_json).expect("chat metadata");
    assert_eq!(
        chat_metadata["queuedRun"]["assistantMessageId"], "assistant-new",
        "must not clear replaced queuedRun"
    );
}

#[test]
fn materialize_missing_pre_stream_failure_heals_legacy_concurrency_once() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let chat_id = "chat-legacy-heal";
    let (team_id, instance_id) = create_test_agent_team(&mut database, chat_id, "legacy-heal");
    let user_id = "user-legacy-heal";
    let assistant_id = "assistant-legacy-heal";
    database
        .insert_message(NewMessage {
            id: user_id,
            chat_id,
            role: "user",
            content: "hello",
            sequence: 0,
            metadata_json: Some(
                r#"{"runConfig":{"modelId":"model-test","providerId":"provider-test"}}"#,
            ),
        })
        .expect("user insert");

    let task_id = AgentTaskId::new("agent-task-legacy-heal").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: &json!({
                "queuedUserMessageId": user_id,
                "visibleAssistantMessageId": assistant_id,
                "visibleAssistantSequence": 1,
                "message": "hello",
            })
            .to_string(),
        })
        .expect("enqueue");
    let attempt_id = AgentAttemptId::new("agent-attempt-legacy-heal").expect("attempt");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
        .expect("claim")
        .expect("claimed");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &task_id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Fail,
            result_json: None,
            error_json: Some(
                r#"{"message":"workspace database concurrency limit reached: waiting for ordinary permit timed out after 5s (gate=ordinary, holders=2/2, waiting=3)"}"#,
            ),
            interruption_reason: None,
        })
        .expect("fail task");

    let first = database
        .materialize_missing_pre_stream_failure_messages(chat_id)
        .expect("heal");
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].assistant_message_id, assistant_id);
    assert_eq!(first[0].assistant_sequence, 1);

    let assistant = database
        .message(assistant_id)
        .expect("assistant")
        .expect("assistant");
    assert_eq!(assistant.sequence, 1);
    assert_eq!(assistant.role, "assistant");
    assert!(
        assistant.content.contains("workspace database is busy")
            || assistant.content.contains("Reply has not started")
    );
    let metadata: Value =
        serde_json::from_str(&assistant.metadata_json).expect("assistant metadata");
    assert_eq!(metadata["streamingState"], "failed");
    assert_eq!(metadata["runFailure"]["code"], "workspace_database_busy");
    assert_eq!(metadata["runFailure"]["retryable"], true);
    assert_eq!(metadata["runFailure"]["healedFromHistoricalTask"], true);
    assert_eq!(metadata["partsSource"], "pre_stream_failure_historical");
    assert_eq!(metadata["parts"][0]["type"], "error");

    let second = database
        .materialize_missing_pre_stream_failure_messages(chat_id)
        .expect("second heal");
    assert!(second.is_empty(), "second load must not duplicate");
    let messages = database.messages_for_chat(chat_id).expect("messages");
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.id == assistant_id)
            .count(),
        1
    );

    // Failed task stays terminal (not requeued).
    let task = database.agent_task(&task_id).expect("task").expect("task");
    assert_eq!(task.status, AgentTaskStatus::Failed);
}

#[test]
fn materialize_missing_pre_stream_failure_heals_structured_stage() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let chat_id = "chat-structured-heal";
    let (team_id, instance_id) = create_test_agent_team(&mut database, chat_id, "structured-heal");
    let user_id = "user-structured-heal";
    let assistant_id = "assistant-structured-heal";
    database
        .insert_message(NewMessage {
            id: user_id,
            chat_id,
            role: "user",
            content: "hello",
            sequence: 0,
            metadata_json: None,
        })
        .expect("user insert");

    let task_id = AgentTaskId::new("agent-task-structured-heal").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: &json!({
                "queuedUserMessageId": user_id,
                "visibleAssistantMessageId": assistant_id,
                "visibleAssistantSequence": 1,
                "message": "hello",
            })
            .to_string(),
        })
        .expect("enqueue");
    let attempt_id = AgentAttemptId::new("agent-attempt-structured-heal").expect("attempt");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
        .expect("claim")
        .expect("claimed");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &task_id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Fail,
            result_json: None,
            error_json: Some(
                &json!({
                    "message": "Reply has not started: workspace database is busy. Please retry.",
                    "code": "workspace_database_busy",
                    "stage": "pre_stream_prepare",
                    "retryable": true,
                })
                .to_string(),
            ),
            interruption_reason: None,
        })
        .expect("fail task");

    let healed = database
        .materialize_missing_pre_stream_failure_messages(chat_id)
        .expect("heal");
    assert_eq!(healed.len(), 1);
    let metadata: Value = serde_json::from_str(
        &database
            .message(assistant_id)
            .expect("assistant")
            .expect("assistant")
            .metadata_json,
    )
    .expect("metadata");
    assert_eq!(metadata["runFailure"]["stage"], "pre_stream_prepare");
}

#[test]
fn materialize_missing_pre_stream_failure_negative_matrix() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let chat_id = "chat-neg-matrix";
    let (team_id, coordinator_id) = create_test_agent_team(&mut database, chat_id, "neg-matrix");

    // 1) Already has assistant → skip
    database
        .insert_message(NewMessage {
            id: "user-has-assistant",
            chat_id,
            role: "user",
            content: "u1",
            sequence: 0,
            metadata_json: None,
        })
        .expect("user");
    database
        .insert_message(NewMessage {
            id: "assistant-has-assistant",
            chat_id,
            role: "assistant",
            content: "already there",
            sequence: 1,
            metadata_json: Some(r#"{"streamingState":"complete"}"#),
        })
        .expect("assistant");
    let task_has_assistant = AgentTaskId::new("agent-task-neg-has-assistant").expect("task id");
    seed_failed_coordinator_task(
        &mut database,
        &team_id,
        &coordinator_id,
        &task_has_assistant,
        "user-has-assistant",
        "assistant-has-assistant",
        1,
        r#"{"message":"workspace database concurrency limit reached: gate=ordinary"}"#,
    );

    // 2) User deleted → skip
    let task_missing_user = AgentTaskId::new("agent-task-neg-missing-user").expect("task id");
    seed_failed_coordinator_task(
        &mut database,
        &team_id,
        &coordinator_id,
        &task_missing_user,
        "user-missing",
        "assistant-missing-user",
        3,
        r#"{"message":"workspace database concurrency limit reached: gate=ordinary"}"#,
    );

    // 3) Worker task → skip
    let worker_id = AgentInstanceId::new("agent-instance-neg-worker").expect("worker");
    let worker_definition = phase8_agent_definition("neg-worker", 1, 1);
    database
        .create_agent_instances_with_limits(
            &[NewAgentInstance {
                id: &worker_id,
                team_id: &team_id,
                definition: &worker_definition,
                role: AgentRole::Worker,
                execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
                execution_root_path: None,
                worktree_base_revision: None,
                worktree_branch: None,
                worktree_status: None,
            }],
            4,
            4,
        )
        .expect("worker");
    let task_worker = AgentTaskId::new("agent-task-neg-worker").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_worker,
            team_id: &team_id,
            owner_instance_id: &worker_id,
            origin_instance_id: Some(&coordinator_id),
            parent_task_id: None,
            input_json: &json!({
                "queuedUserMessageId": "user-has-assistant",
                "visibleAssistantMessageId": "assistant-worker",
                "visibleAssistantSequence": 5,
                "message": "worker",
            })
            .to_string(),
        })
        .expect("enqueue worker");
    let attempt_worker = AgentAttemptId::new("agent-attempt-neg-worker").expect("attempt");
    database
        .claim_runnable_agent_task(&team_id, &task_worker, &attempt_worker)
        .expect("claim")
        .expect("claimed");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &task_worker,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Fail,
            result_json: None,
            error_json: Some(
                r#"{"message":"workspace database concurrency limit reached: gate=ordinary"}"#,
            ),
            interruption_reason: None,
        })
        .expect("fail worker");

    // 4) Ordinary provider failure (not pre-stream whitelist) → skip
    database
        .insert_message(NewMessage {
            id: "user-provider-fail",
            chat_id,
            role: "user",
            content: "u2",
            sequence: 2,
            metadata_json: None,
        })
        .expect("user");
    let task_provider = AgentTaskId::new("agent-task-neg-provider").expect("task id");
    seed_failed_coordinator_task(
        &mut database,
        &team_id,
        &coordinator_id,
        &task_provider,
        "user-provider-fail",
        "assistant-provider-fail",
        4,
        r#"{"message":"provider returned 500","code":"provider_error","stage":"stream"}"#,
    );

    // 5) Legacy concurrency but has start/tool/provider evidence → skip
    database
        .insert_message(NewMessage {
            id: "user-with-evidence",
            chat_id,
            role: "user",
            content: "u3",
            sequence: 6,
            metadata_json: None,
        })
        .expect("user");
    let task_evidence = AgentTaskId::new("agent-task-neg-evidence").expect("task id");
    seed_failed_coordinator_task(
        &mut database,
        &team_id,
        &coordinator_id,
        &task_evidence,
        "user-with-evidence",
        "assistant-with-evidence",
        7,
        r#"{"message":"workspace database concurrency limit reached: gate=ordinary"}"#,
    );
    database
        .insert_run_event(NewRunEvent {
            id: "event-evidence-start",
            chat_id,
            run_id: task_evidence.as_str(),
            sequence: 0,
            event_type: "start",
            payload_json: r#"{"assistantMessageId":"assistant-with-evidence"}"#,
        })
        .expect("start event");

    let healed = database
        .materialize_missing_pre_stream_failure_messages(chat_id)
        .expect("heal");
    assert!(
        healed.is_empty(),
        "negative matrix must not materialize: {healed:?}"
    );
    assert!(
        database
            .message("assistant-worker")
            .expect("lookup")
            .is_none()
    );
    assert!(
        database
            .message("assistant-provider-fail")
            .expect("lookup")
            .is_none()
    );
    assert!(
        database
            .message("assistant-with-evidence")
            .expect("lookup")
            .is_none()
    );
    assert!(
        database
            .message("assistant-missing-user")
            .expect("lookup")
            .is_none()
    );
}

#[test]
fn materialize_missing_pre_stream_failure_is_concurrent_idempotent() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let workspace_path = workspace.path().to_path_buf();
    {
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(&workspace_path).expect("database");
        let chat_id = "chat-concurrent-heal";
        let (team_id, instance_id) =
            create_test_agent_team(&mut database, chat_id, "concurrent-heal");
        database
            .insert_message(NewMessage {
                id: "user-concurrent-heal",
                chat_id,
                role: "user",
                content: "hello",
                sequence: 0,
                metadata_json: None,
            })
            .expect("user");
        let task_id = AgentTaskId::new("agent-task-concurrent-heal").expect("task id");
        seed_failed_coordinator_task(
            &mut database,
            &team_id,
            &instance_id,
            &task_id,
            "user-concurrent-heal",
            "assistant-concurrent-heal",
            1,
            r#"{"message":"workspace database concurrency limit reached: gate=ordinary"}"#,
        );
    }

    let barrier = Arc::new(Barrier::new(4));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let path = workspace_path.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let mut database = WorkspaceDatabase::open_or_create_ungated(&path).expect("database");
            barrier.wait();
            database
                .materialize_missing_pre_stream_failure_messages("chat-concurrent-heal")
                .expect("heal")
        }));
    }
    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("join"))
        .collect::<Vec<_>>();
    let applied: usize = results.iter().map(Vec::len).sum();
    assert_eq!(applied, 1, "exactly one concurrent insert should apply");

    let database = WorkspaceDatabase::open_or_create_ungated(&workspace_path).expect("database");
    let messages = database
        .messages_for_chat("chat-concurrent-heal")
        .expect("messages");
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.id == "assistant-concurrent-heal")
            .count(),
        1
    );
    let assistant = messages
        .iter()
        .find(|message| message.id == "assistant-concurrent-heal")
        .expect("assistant");
    assert_eq!(assistant.sequence, 1);
}

#[test]
fn materialize_missing_terminal_failure_repairs_only_verified_failed_runs() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let chat_id = "chat-terminal-heal";
    let (team_id, coordinator_id) = create_test_agent_team(&mut database, chat_id, "terminal-heal");

    for (id, content, sequence) in [
        ("user-terminal-existing", "existing", 0),
        ("user-terminal-missing", "missing", 2),
        ("user-terminal-success", "success", 4),
        ("user-terminal-cancel", "cancel", 6),
        ("user-terminal-conflict", "conflict", 8),
    ] {
        database
            .insert_message(NewMessage {
                id,
                chat_id,
                role: "user",
                content,
                sequence,
                metadata_json: None,
            })
            .expect("user");
    }
    database
        .insert_message(NewMessage {
            id: "assistant-terminal-existing",
            chat_id,
            role: "assistant",
            content: "partial answer",
            sequence: 1,
            metadata_json: Some(
                r#"{"streamingState":"streaming","parts":[{"type":"text","text":"partial answer"}]}"#,
            ),
        })
        .expect("partial assistant");
    database
        .insert_message(NewMessage {
            id: "assistant-terminal-success",
            chat_id,
            role: "assistant",
            content: "complete answer",
            sequence: 5,
            metadata_json: Some(r#"{"streamingState":"complete","parts":[]}"#),
        })
        .expect("complete assistant");

    database
        .insert_message(NewMessage {
            id: "assistant-terminal-conflict",
            chat_id,
            role: "assistant",
            content: "edited replacement",
            sequence: 10,
            metadata_json: Some(r#"{"streamingState":"streaming","parts":[]}"#),
        })
        .expect("conflicting assistant");

    let candidates = [
        (
            "agent-task-terminal-existing",
            "user-terminal-existing",
            "assistant-terminal-existing",
            1,
            "run-terminal-existing",
            "remote provider failed",
        ),
        (
            "agent-task-terminal-missing",
            "user-terminal-missing",
            "assistant-terminal-missing",
            3,
            "run-terminal-missing",
            "tool broker failed",
        ),
        (
            "agent-task-terminal-success",
            "user-terminal-success",
            "assistant-terminal-success",
            5,
            "run-terminal-success",
            "late failure must not overwrite success",
        ),
        (
            "agent-task-terminal-cancel",
            "user-terminal-cancel",
            "assistant-terminal-cancel",
            7,
            "run-terminal-cancel",
            "agent run cancelled by user",
        ),
        (
            "agent-task-terminal-conflict",
            "user-terminal-conflict",
            "assistant-terminal-conflict",
            9,
            "run-terminal-conflict",
            "stale run must not overwrite edited replacement",
        ),
    ];
    for (task_id, user_id, assistant_id, sequence, run_id, error_message) in candidates {
        let task_id = AgentTaskId::new(task_id).expect("task id");
        seed_failed_coordinator_task(
            &mut database,
            &team_id,
            &coordinator_id,
            &task_id,
            user_id,
            assistant_id,
            sequence,
            r#"{"message":"terminal task failure"}"#,
        );
        database
            .insert_run_event(NewRunEvent {
                id: &format!("event-{run_id}-start"),
                chat_id,
                run_id,
                sequence: 0,
                event_type: "start",
                payload_json: &json!({ "assistantMessageId": assistant_id }).to_string(),
            })
            .expect("start event");
        database
            .insert_run_event(NewRunEvent {
                id: &format!("event-{run_id}-error"),
                chat_id,
                run_id,
                sequence: 1,
                event_type: "error",
                payload_json: &json!({ "message": error_message }).to_string(),
            })
            .expect("error event");
    }

    assert_eq!(
        database
            .materialize_missing_terminal_failure_messages(chat_id)
            .expect("terminal repair"),
        2
    );
    assert_eq!(
        database
            .materialize_missing_terminal_failure_messages(chat_id)
            .expect("second terminal repair"),
        0
    );

    let existing = database
        .message("assistant-terminal-existing")
        .expect("existing assistant")
        .expect("existing assistant");
    assert_eq!(existing.content, "partial answer");
    let existing_metadata: Value =
        serde_json::from_str(&existing.metadata_json).expect("existing metadata");
    assert_eq!(existing_metadata["streamingState"], "failed");
    assert_eq!(
        existing_metadata["runFailure"]["message"],
        "remote provider failed"
    );
    assert_eq!(existing_metadata["parts"][0]["text"], "partial answer");
    assert_eq!(existing_metadata["parts"][1]["type"], "error");

    let missing = database
        .message("assistant-terminal-missing")
        .expect("missing assistant")
        .expect("healed assistant");
    assert_eq!(missing.content, "tool broker failed");
    let missing_metadata: Value =
        serde_json::from_str(&missing.metadata_json).expect("missing metadata");
    assert_eq!(missing_metadata["streamingState"], "failed");
    assert_eq!(missing_metadata["parts"][0]["type"], "error");

    let success = database
        .message("assistant-terminal-success")
        .expect("success assistant")
        .expect("success assistant");
    assert_eq!(success.content, "complete answer");
    let conflict = database
        .message("assistant-terminal-conflict")
        .expect("conflicting assistant")
        .expect("conflicting assistant");
    assert_eq!(conflict.content, "edited replacement");
    assert_eq!(conflict.sequence, 10);
    assert!(
        database
            .message("assistant-terminal-cancel")
            .expect("cancel assistant")
            .is_none(),
        "cancelled terminal evidence must not become an error message"
    );
}

#[test]
fn materialize_missing_terminal_failure_is_concurrent_idempotent() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let workspace_path = workspace.path().to_path_buf();
    {
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(&workspace_path).expect("database");
        let chat_id = "chat-terminal-concurrent";
        let (team_id, coordinator_id) =
            create_test_agent_team(&mut database, chat_id, "terminal-concurrent");
        database
            .insert_message(NewMessage {
                id: "user-terminal-concurrent",
                chat_id,
                role: "user",
                content: "hello",
                sequence: 0,
                metadata_json: None,
            })
            .expect("user");
        let task_id = AgentTaskId::new("agent-task-terminal-concurrent").expect("task id");
        seed_failed_coordinator_task(
            &mut database,
            &team_id,
            &coordinator_id,
            &task_id,
            "user-terminal-concurrent",
            "assistant-terminal-concurrent",
            1,
            r#"{"message":"provider stream failed"}"#,
        );
        database
            .insert_run_event(NewRunEvent {
                id: "event-terminal-concurrent-start",
                chat_id,
                run_id: "run-terminal-concurrent",
                sequence: 0,
                event_type: "start",
                payload_json: r#"{"assistantMessageId":"assistant-terminal-concurrent"}"#,
            })
            .expect("start event");
        database
            .insert_run_event(NewRunEvent {
                id: "event-terminal-concurrent-error",
                chat_id,
                run_id: "run-terminal-concurrent",
                sequence: 1,
                event_type: "error",
                payload_json: r#"{"message":"provider stream failed"}"#,
            })
            .expect("error event");
    }

    let barrier = Arc::new(Barrier::new(4));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let path = workspace_path.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let mut database = WorkspaceDatabase::open_or_create_ungated(&path).expect("database");
            barrier.wait();
            database
                .materialize_missing_terminal_failure_messages("chat-terminal-concurrent")
                .expect("terminal repair")
        }));
    }
    let applied: usize = handles
        .into_iter()
        .map(|handle| handle.join().expect("join"))
        .sum();
    assert_eq!(
        applied, 1,
        "exactly one concurrent terminal repair should apply"
    );

    let database = WorkspaceDatabase::open_or_create_ungated(&workspace_path).expect("database");
    let assistant = database
        .message("assistant-terminal-concurrent")
        .expect("assistant lookup")
        .expect("assistant materialized");
    let metadata: Value = serde_json::from_str(&assistant.metadata_json).expect("metadata");
    assert_eq!(metadata["streamingState"], "failed");
    assert_eq!(metadata["parts"][0]["text"], "provider stream failed");
}

fn seed_failed_coordinator_task(
    database: &mut WorkspaceDatabase,
    team_id: &AgentTeamId,
    owner_instance_id: &AgentInstanceId,
    task_id: &AgentTaskId,
    user_message_id: &str,
    assistant_message_id: &str,
    assistant_sequence: i64,
    error_json: &str,
) {
    database
        .enqueue_agent_task(NewAgentTask {
            id: task_id,
            team_id,
            owner_instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: &json!({
                "queuedUserMessageId": user_message_id,
                "visibleAssistantMessageId": assistant_message_id,
                "visibleAssistantSequence": assistant_sequence,
                "message": "hello",
            })
            .to_string(),
        })
        .expect("enqueue");
    let attempt_id =
        AgentAttemptId::new(format!("agent-attempt-{}", task_id.as_str())).expect("attempt");
    database
        .claim_runnable_agent_task(team_id, task_id, &attempt_id)
        .expect("claim")
        .expect("claimed");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id,
            task_id,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Fail,
            result_json: None,
            error_json: Some(error_json),
            interruption_reason: None,
        })
        .expect("fail");
}

#[test]
fn interrupted_queue_head_requires_explicit_retry_and_keeps_fifo() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
fn agent_task_state_update_for_attempt_rejects_replaced_attempt() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-agent-attempt-guard", "attempt-guard");
    let task_id = AgentTaskId::new("agent-task-attempt-guard").expect("task id");
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
    let first_attempt = AgentAttemptId::new("agent-attempt-guard-first").expect("first attempt id");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &first_attempt)
        .expect("claim first")
        .expect("first claimed");
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &task_id,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Fail,
                result_json: None,
                error_json: Some(r#"{"message":"first failed"}"#),
                interruption_reason: None,
            })
            .expect("fail first")
    );
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &task_id,
                expected_status: AgentTaskStatus::Failed,
                transition: AgentTaskTransition::Retry,
                result_json: None,
                error_json: None,
                interruption_reason: None,
            })
            .expect("retry first")
    );
    let second_attempt =
        AgentAttemptId::new("agent-attempt-guard-second").expect("second attempt id");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &second_attempt)
        .expect("claim second")
        .expect("second claimed");

    assert!(
        !database
            .update_agent_task_state_for_attempt(
                AgentTaskStateUpdate {
                    team_id: &team_id,
                    task_id: &task_id,
                    expected_status: AgentTaskStatus::Running,
                    transition: AgentTaskTransition::Fail,
                    result_json: None,
                    error_json: Some(r#"{"message":"stale recovery"}"#),
                    interruption_reason: None,
                },
                &first_attempt,
            )
            .expect("stale attempt update")
    );
    assert_eq!(
        database
            .agent_task(&task_id)
            .expect("task")
            .expect("task")
            .status,
        AgentTaskStatus::Running
    );
    let attempts = database
        .agent_attempts_for_task(&task_id)
        .expect("attempts");
    assert_eq!(attempts[0].status, foco_agent::AgentAttemptStatus::Failed);
    assert_eq!(attempts[1].id, second_attempt);
    assert_eq!(attempts[1].status, foco_agent::AgentAttemptStatus::Running);
}

#[test]
fn running_agent_task_with_wait_dependencies_recovers_as_waiting() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-agent-wait-recovery", "wait-recovery");
    let worker_id = AgentInstanceId::new("agent-instance-wait-recovery-worker").expect("worker id");
    let worker_definition = phase8_agent_definition("wait-recovery-worker", 1, 1);
    database
        .create_agent_instances_with_limits(
            &[NewAgentInstance {
                id: &worker_id,
                team_id: &team_id,
                definition: &worker_definition,
                role: AgentRole::Worker,
                execution_workspace_mode: foco_agent::AgentExecutionWorkspaceMode::Shared,
                execution_root_path: None,
                worktree_base_revision: None,
                worktree_branch: None,
                worktree_status: None,
            }],
            2,
            1,
        )
        .expect("create worker");
    let parent_task = AgentTaskId::new("agent-task-wait-recovery-parent").expect("parent task");
    let child_task = AgentTaskId::new("agent-task-wait-recovery-child").expect("child task");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &parent_task,
            team_id: &team_id,
            owner_instance_id: &coordinator_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue parent");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &child_task,
            team_id: &team_id,
            owner_instance_id: &worker_id,
            origin_instance_id: Some(&coordinator_id),
            parent_task_id: Some(&parent_task),
            input_json: "{}",
        })
        .expect("enqueue child");
    let parent_attempt =
        AgentAttemptId::new("agent-attempt-wait-recovery-parent").expect("parent attempt");
    database
        .claim_runnable_agent_task(&team_id, &parent_task, &parent_attempt)
        .expect("claim parent")
        .expect("claimed parent");
    database
        .insert_agent_task_dependency(NewAgentTaskDependency {
            team_id: &team_id,
            waiting_task_id: &parent_task,
            dependency_task_id: &child_task,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-recovery"),
            deadline_at: None,
        })
        .expect("insert dependency");

    assert!(
        database
            .suspend_running_agent_task_with_wait_dependencies(&team_id, &parent_task)
            .expect("recover wait dependency")
    );
    assert_eq!(
        database
            .agent_task(&parent_task)
            .expect("parent task")
            .expect("parent task")
            .status,
        AgentTaskStatus::Waiting
    );
    assert_eq!(
        database
            .agent_attempts_for_task(&parent_task)
            .expect("parent attempts")[0]
            .status,
        foco_agent::AgentAttemptStatus::Suspended
    );
    assert_eq!(
        database
            .agent_instance(&coordinator_id)
            .expect("coordinator")
            .expect("coordinator")
            .status,
        AgentInstanceStatus::Waiting
    );
    assert!(
        database
            .resume_satisfied_agent_tasks(10)
            .expect("resume before child done")
            .is_empty()
    );

    let child_attempt =
        AgentAttemptId::new("agent-attempt-wait-recovery-child").expect("child attempt");
    database
        .claim_runnable_agent_task(&team_id, &child_task, &child_attempt)
        .expect("claim child")
        .expect("claimed child");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &child_task,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Complete,
            result_json: Some(r#"{"text":"verified"}"#),
            error_json: None,
            interruption_reason: None,
        })
        .expect("complete child");

    let resumed = database
        .resume_satisfied_agent_tasks(10)
        .expect("resume after child done");
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].id, parent_task);
    assert_eq!(resumed[0].status, AgentTaskStatus::Queued);
    assert_eq!(
        database
            .agent_instance(&coordinator_id)
            .expect("coordinator")
            .expect("coordinator")
            .status,
        AgentInstanceStatus::Idle
    );
}

#[test]
fn interrupted_agent_wait_task_recovers_when_dependency_finishes() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) = create_test_agent_team(
        &mut database,
        "chat-agent-interrupted-wait-recovery",
        "interrupted-wait-recovery",
    );
    let worker_id =
        AgentInstanceId::new("agent-instance-interrupted-wait-recovery-worker").expect("worker id");
    let worker_definition = phase8_agent_definition("interrupted-wait-recovery-worker", 1, 1);
    database
        .create_agent_instances_with_limits(
            &[NewAgentInstance {
                id: &worker_id,
                team_id: &team_id,
                definition: &worker_definition,
                role: AgentRole::Worker,
                execution_workspace_mode: foco_agent::AgentExecutionWorkspaceMode::Shared,
                execution_root_path: None,
                worktree_base_revision: None,
                worktree_branch: None,
                worktree_status: None,
            }],
            2,
            1,
        )
        .expect("create worker");
    let parent_task =
        AgentTaskId::new("agent-task-interrupted-wait-recovery-parent").expect("parent task");
    let child_task =
        AgentTaskId::new("agent-task-interrupted-wait-recovery-child").expect("child task");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &parent_task,
            team_id: &team_id,
            owner_instance_id: &coordinator_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue parent");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &child_task,
            team_id: &team_id,
            owner_instance_id: &worker_id,
            origin_instance_id: Some(&coordinator_id),
            parent_task_id: Some(&parent_task),
            input_json: "{}",
        })
        .expect("enqueue child");
    let parent_attempt = AgentAttemptId::new("agent-attempt-interrupted-wait-recovery-parent")
        .expect("parent attempt");
    database
        .claim_runnable_agent_task(&team_id, &parent_task, &parent_attempt)
        .expect("claim parent")
        .expect("claimed parent");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &parent_task,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Interrupt,
            result_json: None,
            error_json: Some(r#"{"message":"backend restarted while Agent attempt was active"}"#),
            interruption_reason: Some("backend restarted while Agent attempt was active"),
        })
        .expect("interrupt parent");
    database
        .transition_agent_instance_status(&coordinator_id, AgentInstanceStatus::Paused)
        .expect("pause coordinator");
    database
        .insert_agent_task_dependency(NewAgentTaskDependency {
            team_id: &team_id,
            waiting_task_id: &parent_task,
            dependency_task_id: &child_task,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-interrupted-wait-recovery"),
            deadline_at: None,
        })
        .expect("insert dependency");

    let child_attempt = AgentAttemptId::new("agent-attempt-interrupted-wait-recovery-child")
        .expect("child attempt");
    database
        .claim_runnable_agent_task(&team_id, &child_task, &child_attempt)
        .expect("claim child")
        .expect("claimed child");
    database
        .update_agent_task_state(AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &child_task,
            expected_status: AgentTaskStatus::Running,
            transition: AgentTaskTransition::Complete,
            result_json: Some(r#"{"text":"verified"}"#),
            error_json: None,
            interruption_reason: None,
        })
        .expect("complete child");

    let recovered = database
        .recover_interrupted_agent_wait_tasks(
            "backend restarted while Agent attempt was active",
            10,
        )
        .expect("recover interrupted wait");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].id, parent_task);
    assert_eq!(recovered[0].status, AgentTaskStatus::Waiting);
    assert_eq!(
        database
            .agent_attempts_for_task(&parent_task)
            .expect("parent attempts")[0]
            .status,
        foco_agent::AgentAttemptStatus::Suspended
    );
    let resumed = database
        .resume_satisfied_agent_tasks(10)
        .expect("resume recovered wait");
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].id, parent_task);
    assert_eq!(resumed[0].status, AgentTaskStatus::Queued);
    assert_eq!(
        database
            .agent_instance(&coordinator_id)
            .expect("coordinator")
            .expect("coordinator")
            .status,
        AgentInstanceStatus::Idle
    );
}

#[test]
fn interrupted_agent_wait_recovery_keeps_one_active_task_per_owner() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) = create_test_agent_team(
        &mut database,
        "chat-agent-interrupted-wait-fifo",
        "interrupted-wait-fifo",
    );
    let worker_id = create_test_agent_worker(&database, &team_id, "interrupted-wait-fifo-worker");
    let first_parent =
        AgentTaskId::new("agent-task-interrupted-wait-fifo-parent-1").expect("parent 1");
    let second_parent =
        AgentTaskId::new("agent-task-interrupted-wait-fifo-parent-2").expect("parent 2");
    let first_child =
        AgentTaskId::new("agent-task-interrupted-wait-fifo-child-1").expect("child 1");
    let second_child =
        AgentTaskId::new("agent-task-interrupted-wait-fifo-child-2").expect("child 2");

    for (task_id, owner_instance_id, origin_instance_id, parent_task_id) in [
        (&first_parent, &coordinator_id, None, None),
        (
            &first_child,
            &worker_id,
            Some(&coordinator_id),
            Some(&first_parent),
        ),
        (&second_parent, &coordinator_id, None, None),
        (
            &second_child,
            &worker_id,
            Some(&coordinator_id),
            Some(&second_parent),
        ),
    ] {
        database
            .enqueue_agent_task(NewAgentTask {
                id: task_id,
                team_id: &team_id,
                owner_instance_id,
                origin_instance_id,
                parent_task_id,
                input_json: "{}",
            })
            .expect("enqueue task");
    }

    for (task_id, attempt_id) in [
        (
            &first_parent,
            "agent-attempt-interrupted-wait-fifo-parent-1",
        ),
        (
            &second_parent,
            "agent-attempt-interrupted-wait-fifo-parent-2",
        ),
    ] {
        database
            .claim_runnable_agent_task(
                &team_id,
                task_id,
                &AgentAttemptId::new(attempt_id).expect("attempt id"),
            )
            .expect("claim parent")
            .expect("claimed parent");
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Interrupt,
                result_json: None,
                error_json: Some(
                    r#"{"message":"backend restarted while Agent attempt was active"}"#,
                ),
                interruption_reason: Some("backend restarted while Agent attempt was active"),
            })
            .expect("interrupt parent");
    }
    database
        .transition_agent_instance_status(&coordinator_id, AgentInstanceStatus::Paused)
        .expect("pause coordinator");

    for (parent_task, child_task) in [
        (&first_parent, &first_child),
        (&second_parent, &second_child),
    ] {
        database
            .insert_agent_task_dependency(NewAgentTaskDependency {
                team_id: &team_id,
                waiting_task_id: parent_task,
                dependency_task_id: child_task,
                wait_mode: AgentTaskWaitMode::All,
                pending_tool_call_id: Some("call-interrupted-wait-fifo"),
                deadline_at: None,
            })
            .expect("insert dependency");
    }

    let recovered = database
        .recover_interrupted_agent_wait_tasks(
            "backend restarted while Agent attempt was active",
            10,
        )
        .expect("recover interrupted waits");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].id, first_parent);
    assert_eq!(
        database
            .agent_task(&second_parent)
            .expect("second parent")
            .expect("second parent")
            .status,
        AgentTaskStatus::Interrupted
    );
    assert!(
        database
            .recover_interrupted_agent_wait_tasks(
                "backend restarted while Agent attempt was active",
                10,
            )
            .expect("second recovery while first waits")
            .is_empty()
    );
}

#[test]
fn agent_store_rejects_cross_team_references_and_dependency_cycles() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
fn register_agent_wait_dependencies_replays_identical_wait_round() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-wait-replay", "wait-replay");
    let worker_id = create_test_agent_worker(&database, &team_id, "wait-replay-worker");
    let waiting = AgentTaskId::new("agent-task-wait-replay-parent").expect("task id");
    let child_a = AgentTaskId::new("agent-task-wait-replay-a").expect("task id");
    let child_b = AgentTaskId::new("agent-task-wait-replay-b").expect("task id");
    for (id, owner) in [
        (&waiting, &coordinator_id),
        (&child_a, &worker_id),
        (&child_b, &worker_id),
    ] {
        database
            .enqueue_agent_task(NewAgentTask {
                id,
                team_id: &team_id,
                owner_instance_id: owner,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue");
    }

    let deps = [child_a.clone(), child_b.clone()];
    let request = RegisterAgentTaskWaitDependencies {
        team_id: &team_id,
        waiting_task_id: &waiting,
        dependency_task_ids: &deps,
        wait_mode: AgentTaskWaitMode::All,
        pending_tool_call_id: Some("call-wait-replay"),
        deadline_at: Some("2026-07-17T12:00:00.000Z"),
        event_instance_id: Some(&coordinator_id),
    };
    assert_eq!(
        database
            .register_agent_task_wait_dependencies(request.clone())
            .expect("create"),
        AgentTaskWaitRegistrationOutcome::Created
    );
    let first_rows = database
        .agent_task_dependencies(&waiting)
        .expect("dependencies");
    assert_eq!(first_rows.len(), 2);
    let first_created: Vec<_> = first_rows
        .iter()
        .map(|row| row.created_at.clone())
        .collect();
    let first_events = database
        .agent_events_after(&team_id, -1)
        .expect("events")
        .into_iter()
        .filter(|event| event.event_type == "task_waiting_requested")
        .count();
    assert_eq!(first_events, 1);

    assert_eq!(
        database
            .register_agent_task_wait_dependencies(request)
            .expect("replay"),
        AgentTaskWaitRegistrationOutcome::Replayed
    );
    let second_rows = database
        .agent_task_dependencies(&waiting)
        .expect("dependencies after replay");
    assert_eq!(second_rows.len(), 2);
    assert_eq!(
        second_rows
            .iter()
            .map(|row| row.created_at.clone())
            .collect::<Vec<_>>(),
        first_created
    );
    let second_events = database
        .agent_events_after(&team_id, -1)
        .expect("events")
        .into_iter()
        .filter(|event| event.event_type == "task_waiting_requested")
        .count();
    assert_eq!(second_events, 1);
}

#[test]
fn register_agent_wait_dependencies_repairs_partial_legacy_subset() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-wait-repair", "wait-repair");
    let worker_id = create_test_agent_worker(&database, &team_id, "wait-repair-worker");
    let waiting = AgentTaskId::new("agent-task-wait-repair-parent").expect("task id");
    let child_a = AgentTaskId::new("agent-task-wait-repair-a").expect("task id");
    let child_b = AgentTaskId::new("agent-task-wait-repair-b").expect("task id");
    for (id, owner) in [
        (&waiting, &coordinator_id),
        (&child_a, &worker_id),
        (&child_b, &worker_id),
    ] {
        database
            .enqueue_agent_task(NewAgentTask {
                id,
                team_id: &team_id,
                owner_instance_id: owner,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue");
    }

    // Simulate a legacy partial multi-row write for the same wait round.
    database
        .insert_agent_task_dependency(NewAgentTaskDependency {
            team_id: &team_id,
            waiting_task_id: &waiting,
            dependency_task_id: &child_a,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-repair"),
            deadline_at: None,
        })
        .expect("partial insert");
    let partial_created = database
        .agent_task_dependencies(&waiting)
        .expect("partial")
        .into_iter()
        .find(|row| row.dependency_task_id == child_a)
        .expect("child a")
        .created_at;

    let deps = [child_a.clone(), child_b.clone()];
    assert_eq!(
        database
            .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
                team_id: &team_id,
                waiting_task_id: &waiting,
                dependency_task_ids: &deps,
                wait_mode: AgentTaskWaitMode::All,
                pending_tool_call_id: Some("call-wait-repair"),
                deadline_at: None,
                event_instance_id: Some(&coordinator_id),
            })
            .expect("repair"),
        AgentTaskWaitRegistrationOutcome::Repaired
    );
    let rows = database
        .agent_task_dependencies(&waiting)
        .expect("repaired rows");
    assert_eq!(rows.len(), 2);
    let child_a_row = rows
        .iter()
        .find(|row| row.dependency_task_id == child_a)
        .expect("child a");
    assert_eq!(child_a_row.created_at, partial_created);
    assert!(rows.iter().any(|row| row.dependency_task_id == child_b));
    let waiting_events = database
        .agent_events_after(&team_id, -1)
        .expect("events")
        .into_iter()
        .filter(|event| {
            event.event_type == "task_waiting_requested" && event.task_id.as_ref() == Some(&waiting)
        })
        .count();
    // Partial insert already created one event; repair must not duplicate for same pending id.
    assert_eq!(waiting_events, 1);
}

#[test]
fn register_agent_wait_dependencies_rolls_back_when_later_dependency_is_invalid() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-wait-rollback", "wait-rollback");
    let worker_id = create_test_agent_worker(&database, &team_id, "wait-rollback-worker");
    let waiting = AgentTaskId::new("agent-task-wait-rollback-parent").expect("task id");
    let child_ok = AgentTaskId::new("agent-task-wait-rollback-ok").expect("task id");
    let missing = AgentTaskId::new("agent-task-wait-rollback-missing").expect("task id");
    for (id, owner) in [(&waiting, &coordinator_id), (&child_ok, &worker_id)] {
        database
            .enqueue_agent_task(NewAgentTask {
                id,
                team_id: &team_id,
                owner_instance_id: owner,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue");
    }

    let deps = [child_ok.clone(), missing];
    let error = database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id: &team_id,
            waiting_task_id: &waiting,
            dependency_task_ids: &deps,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-rollback"),
            deadline_at: None,
            event_instance_id: None,
        })
        .expect_err("missing dependency must fail");
    assert!(matches!(
        error,
        WorkspaceDatabaseError::InvalidAgentRuntimeData { .. }
    ));
    assert!(
        database
            .agent_task_dependencies(&waiting)
            .expect("dependencies")
            .is_empty()
    );
    assert!(
        database
            .agent_events_after(&team_id, -1)
            .expect("events")
            .into_iter()
            .filter(|event| event.event_type == "task_waiting_requested")
            .count()
            == 0
    );
}

#[test]
fn register_agent_wait_dependencies_rejects_metadata_conflict_for_same_tool_call() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-wait-meta", "wait-meta");
    let worker_id = create_test_agent_worker(&database, &team_id, "wait-meta-worker");
    let waiting = AgentTaskId::new("agent-task-wait-meta-parent").expect("task id");
    let child = AgentTaskId::new("agent-task-wait-meta-child").expect("task id");
    for (id, owner) in [(&waiting, &coordinator_id), (&child, &worker_id)] {
        database
            .enqueue_agent_task(NewAgentTask {
                id,
                team_id: &team_id,
                owner_instance_id: owner,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue");
    }
    let deps = [child.clone()];
    database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id: &team_id,
            waiting_task_id: &waiting,
            dependency_task_ids: &deps,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-meta"),
            deadline_at: None,
            event_instance_id: None,
        })
        .expect("create");

    let error = database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id: &team_id,
            waiting_task_id: &waiting,
            dependency_task_ids: &deps,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-meta"),
            deadline_at: Some("2026-07-17T12:00:00.000Z"),
            event_instance_id: None,
        })
        .expect_err("deadline conflict");
    assert!(matches!(
        error,
        WorkspaceDatabaseError::InvalidAgentRuntimeData { message }
            if message.contains("conflicts with existing registration")
    ));
    assert_eq!(
        database
            .agent_task_dependencies(&waiting)
            .expect("unchanged")
            .len(),
        1
    );
}

#[test]
fn register_agent_wait_dependencies_rejects_active_round_when_new_tool_call_arrives() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-wait-active", "wait-active");
    let worker_id = create_test_agent_worker(&database, &team_id, "wait-active-worker");
    let waiting = AgentTaskId::new("agent-task-wait-active-parent").expect("task id");
    let child = AgentTaskId::new("agent-task-wait-active-child").expect("task id");
    for (id, owner) in [(&waiting, &coordinator_id), (&child, &worker_id)] {
        database
            .enqueue_agent_task(NewAgentTask {
                id,
                team_id: &team_id,
                owner_instance_id: owner,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue");
    }
    let deps = [child.clone()];
    database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id: &team_id,
            waiting_task_id: &waiting,
            dependency_task_ids: &deps,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-active-1"),
            deadline_at: None,
            event_instance_id: None,
        })
        .expect("create");

    let error = database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id: &team_id,
            waiting_task_id: &waiting,
            dependency_task_ids: &deps,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-active-2"),
            deadline_at: None,
            event_instance_id: None,
        })
        .expect_err("active wait must block replacement");
    assert!(matches!(
        error,
        WorkspaceDatabaseError::InvalidAgentRuntimeData { message }
            if message.contains("active wait round")
    ));
    let rows = database
        .agent_task_dependencies(&waiting)
        .expect("dependencies");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].pending_tool_call_id.as_deref(),
        Some("call-wait-active-1")
    );
}

#[test]
fn register_agent_wait_dependencies_replaces_terminal_round_and_allows_same_child_again() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-wait-replace", "wait-replace");
    let worker_id = create_test_agent_worker(&database, &team_id, "wait-replace-worker");
    let waiting = AgentTaskId::new("agent-task-wait-replace-parent").expect("task id");
    let child = AgentTaskId::new("agent-task-wait-replace-child").expect("task id");
    for (id, owner) in [(&waiting, &coordinator_id), (&child, &worker_id)] {
        database
            .enqueue_agent_task(NewAgentTask {
                id,
                team_id: &team_id,
                owner_instance_id: owner,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue");
    }
    let deps = [child.clone()];
    database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id: &team_id,
            waiting_task_id: &waiting,
            dependency_task_ids: &deps,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-replace-1"),
            deadline_at: None,
            event_instance_id: Some(&coordinator_id),
        })
        .expect("first wait");

    let attempt_id = AgentAttemptId::new("agent-attempt-wait-replace-child").expect("attempt id");
    database
        .claim_runnable_agent_task(&team_id, &child, &attempt_id)
        .expect("claim child")
        .expect("child claimed");
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &child,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Complete,
                result_json: Some(r#"{"ok":true}"#),
                error_json: None,
                interruption_reason: None,
            })
            .expect("complete child")
    );

    assert_eq!(
        database
            .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
                team_id: &team_id,
                waiting_task_id: &waiting,
                dependency_task_ids: &deps,
                wait_mode: AgentTaskWaitMode::All,
                pending_tool_call_id: Some("call-wait-replace-2"),
                deadline_at: None,
                event_instance_id: Some(&coordinator_id),
            })
            .expect("second wait after terminal"),
        AgentTaskWaitRegistrationOutcome::Replaced
    );
    let rows = database
        .agent_task_dependencies(&waiting)
        .expect("replaced dependencies");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].dependency_task_id, child);
    assert_eq!(
        rows[0].pending_tool_call_id.as_deref(),
        Some("call-wait-replace-2")
    );
    let waiting_events = database
        .agent_events_after(&team_id, -1)
        .expect("events")
        .into_iter()
        .filter(|event| {
            event.event_type == "task_waiting_requested" && event.task_id.as_ref() == Some(&waiting)
        })
        .count();
    assert_eq!(waiting_events, 2);
}

#[test]
fn register_agent_wait_dependencies_rejects_cross_team_and_cycles() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (first_team, first_instance) =
        create_test_agent_team(&mut database, "chat-wait-cross-a", "wait-cross-a");
    let (second_team, second_instance) =
        create_test_agent_team(&mut database, "chat-wait-cross-b", "wait-cross-b");
    let first_task = AgentTaskId::new("agent-task-wait-cross-first").expect("task");
    let second_task = AgentTaskId::new("agent-task-wait-cross-second").expect("task");
    let third_task = AgentTaskId::new("agent-task-wait-cross-third").expect("task");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &first_task,
            team_id: &first_team,
            owner_instance_id: &first_instance,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("first");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &second_task,
            team_id: &second_team,
            owner_instance_id: &second_instance,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("second");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &third_task,
            team_id: &first_team,
            owner_instance_id: &first_instance,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("third");

    let cross = database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id: &first_team,
            waiting_task_id: &first_task,
            dependency_task_ids: std::slice::from_ref(&second_task),
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-cross"),
            deadline_at: None,
            event_instance_id: None,
        })
        .expect_err("cross-team");
    assert!(matches!(
        cross,
        WorkspaceDatabaseError::AgentDomain { ref source }
            if source.code() == AgentDomainErrorCode::CrossTeamReference
    ));

    database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id: &first_team,
            waiting_task_id: &first_task,
            dependency_task_ids: std::slice::from_ref(&third_task),
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-cycle-1"),
            deadline_at: None,
            event_instance_id: None,
        })
        .expect("first edge");
    let cycle = database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id: &first_team,
            waiting_task_id: &third_task,
            dependency_task_ids: std::slice::from_ref(&first_task),
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-cycle-2"),
            deadline_at: None,
            event_instance_id: None,
        })
        .expect_err("cycle");
    assert!(matches!(
        cycle,
        WorkspaceDatabaseError::AgentDomain { ref source }
            if source.code() == AgentDomainErrorCode::DependencyCycle
    ));
    assert!(
        database
            .agent_task_dependencies(&third_task)
            .expect("third deps")
            .is_empty()
    );
}

#[test]
fn register_agent_wait_dependencies_backfills_missing_event_on_replay() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-wait-event-gap", "wait-event-gap");
    let worker_id = create_test_agent_worker(&database, &team_id, "wait-event-gap-worker");
    let waiting = AgentTaskId::new("agent-task-wait-event-gap-parent").expect("task id");
    let child = AgentTaskId::new("agent-task-wait-event-gap-child").expect("task id");
    for (id, owner) in [(&waiting, &coordinator_id), (&child, &worker_id)] {
        database
            .enqueue_agent_task(NewAgentTask {
                id,
                team_id: &team_id,
                owner_instance_id: owner,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue");
    }

    // Insert a dependency row without going through the registration event path.
    let connection = rusqlite::Connection::open(database.database_path()).expect("connection");
    connection
        .execute(
            "INSERT INTO agent_task_dependencies
                (team_id, waiting_task_id, dependency_task_id, wait_mode,
                 pending_tool_call_id, deadline_at, created_at)
             VALUES (?1, ?2, ?3, 'all', ?4, NULL, '2026-07-17T00:00:00.000Z')",
            rusqlite::params![
                team_id.as_str(),
                waiting.as_str(),
                child.as_str(),
                "call-wait-event-gap",
            ],
        )
        .expect("raw dependency insert");
    drop(connection);

    assert_eq!(
        database
            .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
                team_id: &team_id,
                waiting_task_id: &waiting,
                dependency_task_ids: std::slice::from_ref(&child),
                wait_mode: AgentTaskWaitMode::All,
                pending_tool_call_id: Some("call-wait-event-gap"),
                deadline_at: None,
                event_instance_id: Some(&coordinator_id),
            })
            .expect("replay with event backfill"),
        AgentTaskWaitRegistrationOutcome::Replayed
    );
    let waiting_events = database
        .agent_events_after(&team_id, -1)
        .expect("events")
        .into_iter()
        .filter(|event| {
            event.event_type == "task_waiting_requested" && event.task_id.as_ref() == Some(&waiting)
        })
        .count();
    assert_eq!(waiting_events, 1);
}

#[test]
fn register_agent_wait_dependencies_rejects_duplicate_dependency_ids() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, coordinator_id) =
        create_test_agent_team(&mut database, "chat-wait-dup", "wait-dup");
    let worker_id = create_test_agent_worker(&database, &team_id, "wait-dup-worker");
    let waiting = AgentTaskId::new("agent-task-wait-dup-parent").expect("task id");
    let child = AgentTaskId::new("agent-task-wait-dup-child").expect("task id");
    for (id, owner) in [(&waiting, &coordinator_id), (&child, &worker_id)] {
        database
            .enqueue_agent_task(NewAgentTask {
                id,
                team_id: &team_id,
                owner_instance_id: owner,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue");
    }

    let deps = [child.clone(), child.clone()];
    let error = database
        .register_agent_task_wait_dependencies(RegisterAgentTaskWaitDependencies {
            team_id: &team_id,
            waiting_task_id: &waiting,
            dependency_task_ids: &deps,
            wait_mode: AgentTaskWaitMode::All,
            pending_tool_call_id: Some("call-wait-dup"),
            deadline_at: None,
            event_instance_id: None,
        })
        .expect_err("duplicate dependency ids must fail");
    assert!(matches!(
        error,
        WorkspaceDatabaseError::InvalidAgentRuntimeData { message }
            if message.contains("duplicate dependency task id")
    ));
    assert!(
        database
            .agent_task_dependencies(&waiting)
            .expect("dependencies")
            .is_empty()
    );
}

#[test]
fn phase8_creates_multiple_agent_instances_atomically() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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

    let shared = database
        .switch_agent_instance_to_shared_workspace(&instance_id)
        .expect("switch to shared workspace");
    assert_eq!(
        shared.execution_workspace_mode,
        AgentExecutionWorkspaceMode::Shared
    );
    assert!(shared.execution_root_path.is_none());
    assert!(shared.worktree_base_revision.is_none());
    assert!(shared.worktree_branch.is_none());
    assert!(shared.worktree_status.is_none());
}

#[test]
fn plan_worktree_audit_lists_unmerged_isolated_plan_worktrees() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let plan_id = "plan-worktree-audit";
    let phase_id = "plan-worktree-audit-phase";

    database
        .create_plan(NewPlan {
            id: plan_id,
            title: "Audit legacy worktree",
            overview: "Find unmerged isolated plan worktrees.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: phase_id,
                title: "Phase one",
                summary: "Creates a worktree commit.",
                steps: vec![NewPlanStep {
                    id: "plan-worktree-audit-step",
                    title: "Do work",
                    detail: "Finish the implementation.",
                    acceptance: vec!["audit finds it".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan(plan_id, "start")
        .expect("start plan");

    let root_path = workspace
        .path()
        .join(".foco")
        .join("agent-worktrees")
        .join("agent-instance-plan-worktree-audit")
        .display()
        .to_string();
    let (team_id, instance_id) = create_test_isolated_agent_team(
        &mut database,
        "chat-plan-worktree-audit",
        "plan-worktree-audit",
        &root_path,
    );
    let task_id = AgentTaskId::new("agent-task-plan-worktree-audit").expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue task");
    database
        .attach_plan_phase_run(
            plan_id,
            phase_id,
            "chat-plan-worktree-audit",
            &team_id,
            &task_id,
        )
        .expect("attach phase run");
    complete_test_agent_task(
        &mut database,
        &team_id,
        &task_id,
        "agent-attempt-plan-worktree-audit",
    );
    database
        .complete_plan_phase_run(&task_id, Some("0123456789abcdef0123456789abcdef01234567"))
        .expect("complete phase")
        .expect("completed plan");

    let audit = database.plan_worktree_audit().expect("worktree audit");
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].plan_id, plan_id);
    assert_eq!(audit[0].phase_id, phase_id);
    assert_eq!(
        audit[0].implementation_chat_id.as_deref(),
        Some("chat-plan-worktree-audit")
    );
    assert_eq!(audit[0].agent_task_id.as_deref(), Some(task_id.as_str()));
    assert_eq!(audit[0].agent_instance_id, instance_id);
    assert_eq!(audit[0].worktree_status.as_deref(), Some("active"));
    assert_eq!(audit[0].base_revision.as_deref(), Some("base-revision"));
    assert_eq!(
        audit[0].branch.as_deref(),
        Some("foco/agent-worktrees/agent-instance-plan-worktree-audit")
    );

    database
        .record_plan_shared_merge_commit(plan_id, "fedcba9876543210fedcba9876543210fedcba98")
        .expect("record shared merge");
    let merged_audit = database.plan_worktree_audit().expect("audit after merge");
    assert_eq!(merged_audit.len(), 1);
    assert_eq!(merged_audit[0].agent_instance_id, instance_id);

    database
        .update_agent_instance_worktree_status(&instance_id, "deleted")
        .expect("mark worktree deleted");
    assert!(
        database
            .plan_worktree_audit()
            .expect("audit after worktree delete")
            .is_empty()
    );
}

#[test]
fn phase12_rejects_worktree_status_update_for_shared_instance() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, _) =
        create_test_agent_team(&mut database, "chat-agent-phase8-fair", "phase8-fair");
    Connection::open(database.database_path())
        .expect("database connection")
        .execute(
            "UPDATE agent_teams SET max_concurrent_runs = 2 WHERE id = ?1",
            params![team_id.as_str()],
        )
        .expect("raise team run limit");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
    assert_eq!(
        database
            .next_waiting_agent_task_dependency_deadline()
            .expect("next dependency deadline"),
        Some("2000-01-01T00:00:00.000Z".to_string())
    );

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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
fn agent_message_guidance_consumption_is_atomic_and_idempotent() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-agent-guidance", "guidance");
    let message_id = AgentMessageId::new("agent-message-guidance-live").expect("message id");
    database
        .insert_agent_message(NewAgentMessage {
            id: &message_id,
            team_id: &team_id,
            sender_instance_id: Some(&instance_id),
            receiver_instance_id: &instance_id,
            related_task_id: None,
            reply_to_message_id: None,
            kind: AgentMessageKind::Notification,
            content: "apply this live guidance",
        })
        .expect("insert message");

    let payload = json!({
        "type": "guidanceApplied",
        "id": message_id.to_string(),
        "content": "apply this live guidance",
        "source": "agentMessage",
    })
    .to_string();
    let rejected_payload = json!({
        "type": "guidanceApplied",
        "id": message_id.to_string(),
        "content": "apply this live guidance",
        "source": "manualGuidance",
    })
    .to_string();
    let rejected = database
        .insert_agent_message_guidance_run_event_and_consume(
            NewRunEvent {
                id: "run-guidance-event-manual",
                chat_id: "chat-agent-guidance",
                run_id: "run-guidance",
                sequence: 0,
                event_type: "guidance_applied",
                payload_json: &rejected_payload,
            },
            &message_id,
            "manualGuidance",
        )
        .expect_err("manual guidance must not consume an Agent message");
    assert!(
        rejected
            .to_string()
            .contains("agentMessage guidance source")
    );
    assert_eq!(
        database
            .agent_message(&message_id)
            .expect("message read")
            .expect("message")
            .consumed_at,
        None
    );
    assert!(
        database
            .run_events_for_run("run-guidance")
            .expect("events")
            .is_empty()
    );

    assert!(
        database
            .insert_agent_message_guidance_run_event_and_consume(
                NewRunEvent {
                    id: "run-guidance-event-0",
                    chat_id: "chat-agent-guidance",
                    run_id: "run-guidance",
                    sequence: 0,
                    event_type: "guidance_applied",
                    payload_json: &payload,
                },
                &message_id,
                "agentMessage",
            )
            .expect("persist live guidance")
    );
    assert!(
        database
            .agent_message(&message_id)
            .expect("message read")
            .expect("message")
            .consumed_at
            .is_some()
    );
    let events = database.run_events_for_run("run-guidance").expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, "run-guidance-event-0");
    let consumed_events = database
        .agent_events_after(&team_id, -1)
        .expect("Agent events")
        .into_iter()
        .filter(|event| event.event_type == "message_consumed")
        .collect::<Vec<_>>();
    assert_eq!(consumed_events.len(), 1);
    assert_eq!(consumed_events[0].message_id.as_ref(), Some(&message_id));

    assert!(
        !database
            .insert_agent_message_guidance_run_event_and_consume(
                NewRunEvent {
                    id: "run-guidance-event-1",
                    chat_id: "chat-agent-guidance",
                    run_id: "run-guidance",
                    sequence: 1,
                    event_type: "guidance_applied",
                    payload_json: &payload,
                },
                &message_id,
                "agentMessage",
            )
            .expect("duplicate live guidance is a no-op")
    );
    assert_eq!(
        database
            .run_events_for_run("run-guidance")
            .expect("events")
            .len(),
        1
    );
    assert_eq!(
        database
            .agent_events_after(&team_id, -1)
            .expect("Agent events")
            .into_iter()
            .filter(|event| event.event_type == "message_consumed")
            .count(),
        1
    );
}

#[test]
fn phase6_agent_child_tasks_are_team_scoped_and_queued_only_cancellable() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
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
            request_kind: "chat completion",
            agent_team_id: Some(&team_id),
            agent_instance_id: Some(&instance_id),
            agent_task_id: Some(&task_id),
            agent_attempt_id: Some(&attempt_id),
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-06-19T00:00:00Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
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

fn attach_test_plan_merge_run(
    database: &mut WorkspaceDatabase,
    plan_id: &str,
    phase_id: &str,
    suffix: &str,
) -> AgentTaskId {
    database
        .create_plan(NewPlan {
            id: plan_id,
            title: "Plan merge state",
            overview: "A failed fast-forward should leave an auditable state.",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: phase_id,
                title: "Phase one",
                summary: "Needs merge automation.",
                steps: vec![NewPlanStep {
                    id: &format!("{phase_id}-step"),
                    title: "Do work",
                    detail: "Complete the change.",
                    acceptance: vec!["merge state is correct".to_string()],
                }],
            }],
        })
        .expect("create plan");
    database
        .transition_plan(plan_id, "start")
        .expect("start plan");

    let (phase_team_id, phase_instance_id) = create_test_agent_team(
        database,
        &format!("chat-{suffix}-phase"),
        &format!("{suffix}-phase"),
    );
    let phase_task_id = AgentTaskId::new(format!("agent-task-{suffix}-phase")).expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &phase_task_id,
            team_id: &phase_team_id,
            owner_instance_id: &phase_instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue phase task");
    database
        .attach_plan_phase_run(
            plan_id,
            phase_id,
            &format!("chat-{suffix}-phase"),
            &phase_team_id,
            &phase_task_id,
        )
        .expect("attach phase task");
    database
        .complete_plan_phase_run(&phase_task_id, Some("worktree-commit"))
        .expect("complete phase")
        .expect("plan");
    assert!(
        database
            .try_begin_plan_phase_merge_attempt(plan_id, phase_id, "shared workspace HEAD changed")
            .expect("record merge attempt")
    );

    // Older local/manual merge runs record the once-only counter but have no durable
    // `merge_auto` attempt. Keep their lifecycle tests representative of that state.
    let connection = Connection::open(database.database_path()).expect("legacy merge connection");
    let removed = connection
        .execute(
            "DELETE FROM plan_phase_attempts
             WHERE plan_id = ?1 AND phase_id = ?2 AND trigger = 'merge_auto'",
            params![plan_id, phase_id],
        )
        .expect("remove durable merge attempt for legacy fixture");
    assert_eq!(removed, 1, "seed legacy merge fixture");
    drop(connection);

    let (merge_team_id, merge_instance_id) = create_test_agent_team(
        database,
        &format!("chat-{suffix}-merge"),
        &format!("{suffix}-merge"),
    );
    let merge_task_id = AgentTaskId::new(format!("agent-task-{suffix}-merge")).expect("task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &merge_task_id,
            team_id: &merge_team_id,
            owner_instance_id: &merge_instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: "{}",
        })
        .expect("enqueue merge task");
    let running = database
        .attach_plan_phase_merge_run(
            plan_id,
            phase_id,
            &format!("chat-{suffix}-merge"),
            &merge_team_id,
            &merge_task_id,
        )
        .expect("attach merge task");
    assert_eq!(running.status, "running");
    assert_eq!(running.phases[0].status, "running");
    assert!(running.shared_merge_commit_id.is_none());
    merge_task_id
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
        allowed_execution_workspace_modes: AgentExecutionWorkspaceMode::all(),
        permissions: AgentPermissions::default(),
    };
    database
        .create_agent_team(NewAgentTeam {
            id: &team_id,
            chat_id,
            coordinator_instance_id: &instance_id,
            coordinator_definition: &definition,
            coordinator_execution_workspace_mode: AgentExecutionWorkspaceMode::Shared,
            coordinator_execution_root_path: None,
            coordinator_worktree_base_revision: None,
            coordinator_worktree_branch: None,
            coordinator_worktree_status: None,
            max_concurrent_runs: 1,
        })
        .expect("agent team create");
    (team_id, instance_id)
}

fn complete_test_agent_task(
    database: &mut WorkspaceDatabase,
    team_id: &AgentTeamId,
    task_id: &AgentTaskId,
    attempt_id: &str,
) {
    let attempt_id = AgentAttemptId::new(attempt_id).expect("attempt id");
    database
        .claim_runnable_agent_task(team_id, task_id, &attempt_id)
        .expect("claim task")
        .expect("claimed task");
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id,
                task_id,
                expected_status: AgentTaskStatus::Running,
                transition: AgentTaskTransition::Complete,
                result_json: Some(r#"{"ok":true}"#),
                error_json: None,
                interruption_reason: None,
            })
            .expect("complete task")
    );
}

fn create_test_isolated_agent_team(
    database: &mut WorkspaceDatabase,
    chat_id: &str,
    suffix: &str,
    root_path: &str,
) -> (AgentTeamId, AgentInstanceId) {
    database
        .insert_chat(chat_id, &format!("Agent team {suffix}"))
        .expect("chat insert");
    let team_id = AgentTeamId::new(format!("agent-team-{suffix}")).expect("team id");
    let instance_id =
        AgentInstanceId::new(format!("agent-instance-{suffix}")).expect("instance id");
    let definition = phase8_agent_definition(suffix, 1, 1);
    let branch = format!("foco/agent-worktrees/{instance_id}");
    database
        .create_agent_team(NewAgentTeam {
            id: &team_id,
            chat_id,
            coordinator_instance_id: &instance_id,
            coordinator_definition: &definition,
            coordinator_execution_workspace_mode: AgentExecutionWorkspaceMode::IsolatedWorktree,
            coordinator_execution_root_path: Some(root_path),
            coordinator_worktree_base_revision: Some("base-revision"),
            coordinator_worktree_branch: Some(&branch),
            coordinator_worktree_status: Some("active"),
            max_concurrent_runs: 1,
        })
        .expect("isolated agent team create");
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
        allowed_execution_workspace_modes: AgentExecutionWorkspaceMode::all(),
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

fn column_exists(connection: &Connection, table: &str, column: &str) -> bool {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .expect("table info statement");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("table info rows");

    for name in columns {
        if name.expect("column name") == column {
            return true;
        }
    }
    false
}

fn add_workspace_chats_table(connection: &Connection) {
    ensure_messages_table_for_migration_fixture(connection);
}

fn add_workspace_memory_tables(connection: &Connection) {
    // Partial migration fixtures often omit core chat tables. Later indexes
    // (e.g. MIGRATION_037) require `messages` / `chats.metadata_json` to exist.
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS chats (
                id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
                title TEXT NOT NULL CHECK (length(title) > 0),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived_at TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}'
             );
             CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
                chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
                role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
                content TEXT NOT NULL,
                sequence INTEGER NOT NULL CHECK (sequence >= 0),
                created_at TEXT NOT NULL,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                UNIQUE (chat_id, sequence)
             );",
        )
        .expect("workspace core chat tables for migration fixture");
    ensure_chat_metadata_json_column(connection);
    connection
        .execute_batch(WORKSPACE_MEMORY_SCHEMA_SQL)
        .expect("workspace memory migration fixture schema");
}

fn ensure_chat_metadata_json_column(connection: &Connection) {
    let has_metadata_json = connection
        .prepare("PRAGMA table_info(chats)")
        .expect("table info")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("table info rows")
        .filter_map(Result::ok)
        .any(|name| name == "metadata_json");
    if !has_metadata_json {
        connection
            .execute(
                "ALTER TABLE chats ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}'",
                [],
            )
            .expect("add chats.metadata_json for migration fixture");
    }
}

fn ensure_messages_table_for_migration_fixture(connection: &Connection) {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS chats (
                id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
                title TEXT NOT NULL CHECK (length(title) > 0),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived_at TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}'
             );
             CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
                chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
                role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
                content TEXT NOT NULL,
                sequence INTEGER NOT NULL CHECK (sequence >= 0),
                created_at TEXT NOT NULL,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                UNIQUE (chat_id, sequence)
             );",
        )
        .expect("messages table for migration fixture");
    ensure_chat_metadata_json_column(connection);
}

fn add_workspace_agent_plan_reference_tables(connection: &Connection) {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_teams (
                id TEXT PRIMARY KEY NOT NULL
             );
             CREATE TABLE IF NOT EXISTS agent_tasks (
                id TEXT PRIMARY KEY NOT NULL
             );",
        )
        .expect("workspace agent plan reference migration fixture schema");
}

fn add_workspace_memory_dream_tables(connection: &Connection) {
    connection
        .execute_batch(WORKSPACE_MEMORY_DREAM_SCHEMA_SQL)
        .expect("workspace memory dream migration fixture schema");
}

fn add_memory_reference_tables(connection: &Connection) {
    connection
        .execute_batch(MEMORY_REFERENCES_SCHEMA_SQL)
        .expect("memory references migration fixture schema");
}

#[test]
fn latest_completed_llm_usage_for_chat_selects_latest_completed_usage() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-usage", "Usage chat")
        .expect("chat insert");
    database
        .insert_chat("other-chat", "Other chat")
        .expect("other chat insert");

    for request in [
        NewLlmRequest {
            id: "request-spec-update-latest",
            workspace_id: "workspace-1",
            chat_id: Some("chat-usage"),
            request_kind: "workspace spec update",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-07-03T10:00:07Z",
            first_token_at: None,
            completed_at: Some("2026-07-03T10:00:08Z"),
            input_tokens: Some(4),
            output_tokens: Some(3),
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(200),
            final_state: "succeeded",
            request_body_json: None,
            response_body_json: None,
        },
        NewLlmRequest {
            id: "request-memory-retrieval-latest",
            workspace_id: "workspace-1",
            chat_id: Some("chat-usage"),
            request_kind: "memory retrieval",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-07-03T10:00:06Z",
            first_token_at: None,
            completed_at: Some("2026-07-03T10:00:07Z"),
            input_tokens: Some(3),
            output_tokens: Some(2),
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(200),
            final_state: "succeeded",
            request_body_json: None,
            response_body_json: None,
        },
        NewLlmRequest {
            id: "request-running-latest",
            workspace_id: "workspace-1",
            chat_id: Some("chat-usage"),
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-07-03T10:00:05Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: Some(999),
            output_tokens: Some(999),
            cache_read_tokens: Some(9),
            cache_write_tokens: Some(9),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: None,
            final_state: "running",
            request_body_json: None,
            response_body_json: None,
        },
        NewLlmRequest {
            id: "request-empty-tokens",
            workspace_id: "workspace-1",
            chat_id: Some("chat-usage"),
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-07-03T10:00:04Z",
            first_token_at: None,
            completed_at: Some("2026-07-03T10:00:05Z"),
            input_tokens: None,
            output_tokens: Some(7),
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(200),
            final_state: "completed",
            request_body_json: None,
            response_body_json: None,
        },
        NewLlmRequest {
            id: "request-failed",
            workspace_id: "workspace-1",
            chat_id: Some("chat-usage"),
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-07-03T10:00:03Z",
            first_token_at: None,
            completed_at: Some("2026-07-03T10:00:04Z"),
            input_tokens: Some(888),
            output_tokens: Some(888),
            cache_read_tokens: Some(8),
            cache_write_tokens: Some(8),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(500),
            final_state: "failed",
            request_body_json: None,
            response_body_json: None,
        },
        NewLlmRequest {
            id: "request-selected-b",
            workspace_id: "workspace-1",
            chat_id: Some("chat-usage"),
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-07-03T10:00:02Z",
            first_token_at: None,
            completed_at: Some("2026-07-03T10:00:03Z"),
            input_tokens: Some(120),
            output_tokens: Some(40),
            cache_read_tokens: Some(12),
            cache_write_tokens: Some(4),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(200),
            final_state: "succeeded",
            request_body_json: None,
            response_body_json: None,
        },
        NewLlmRequest {
            id: "request-selected-a",
            workspace_id: "workspace-1",
            chat_id: Some("chat-usage"),
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-07-03T10:00:02Z",
            first_token_at: None,
            completed_at: Some("2026-07-03T10:00:03Z"),
            input_tokens: Some(10),
            output_tokens: Some(5),
            cache_read_tokens: Some(1),
            cache_write_tokens: Some(1),
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(200),
            final_state: "completed",
            request_body_json: None,
            response_body_json: None,
        },
        NewLlmRequest {
            id: "request-other-chat",
            workspace_id: "workspace-1",
            chat_id: Some("other-chat"),
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "openai",
            model_id: "gpt-test",
            thinking_level: None,
            request_started_at: "2026-07-03T10:00:06Z",
            first_token_at: None,
            completed_at: Some("2026-07-03T10:00:07Z"),
            input_tokens: Some(777),
            output_tokens: Some(777),
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(200),
            final_state: "succeeded",
            request_body_json: None,
            response_body_json: None,
        },
    ] {
        database
            .insert_llm_request(request)
            .expect("llm request insert");
    }

    let usage = database
        .latest_completed_llm_usage_for_chat("chat-usage")
        .expect("latest usage")
        .expect("usage");
    assert_eq!(usage.input_tokens, 120);
    assert_eq!(usage.output_tokens, 40);
    assert_eq!(usage.cache_read_tokens, Some(12));
    assert_eq!(usage.cache_write_tokens, Some(4));
    assert!(
        database
            .latest_completed_llm_usage_for_chat("missing-chat")
            .expect("missing usage")
            .is_none()
    );
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

#[test]
fn agent_task_for_queued_user_message_prefers_latest_rewrite_task() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    let (team_id, instance_id) =
        create_test_agent_team(&mut database, "chat-rewrite-task", "rewrite-task");
    let old_task_id = AgentTaskId::new("agent-task-rewrite-old").expect("old task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &old_task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: r#"{"queuedUserMessageId":"user-rewrite","visibleAssistantMessageId":"assistant-old","visibleAssistantSequence":1}"#,
        })
        .expect("old task enqueue");
    assert!(
        database
            .update_agent_task_state(AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &old_task_id,
                expected_status: AgentTaskStatus::Queued,
                transition: AgentTaskTransition::Cancel,
                result_json: None,
                error_json: Some(r#"{"message":"rewritten"}"#),
                interruption_reason: None,
            })
            .expect("cancel old task")
    );

    let new_task_id = AgentTaskId::new("agent-task-rewrite-new").expect("new task id");
    database
        .enqueue_agent_task(NewAgentTask {
            id: &new_task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: r#"{"queuedUserMessageId":"user-rewrite","visibleAssistantMessageId":"assistant-new","visibleAssistantSequence":1}"#,
        })
        .expect("new task enqueue");

    let selected = database
        .agent_task_for_queued_user_message(&team_id, "user-rewrite")
        .expect("task lookup")
        .expect("selected task");
    assert_eq!(selected.id, new_task_id);
    assert_eq!(selected.status, AgentTaskStatus::Queued);
}

#[test]
fn rewrite_chat_from_user_message_truncates_persisted_history_and_prompt_state() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .insert_chat("chat-rewrite", "Rewrite chat")
        .expect("chat insert");
    for (id, role, content, sequence, metadata_json) in [
        ("user-1", "user", "first", 0, r#"{"modelId":"model-old"}"#),
        ("assistant-1", "assistant", "first answer", 1, "{}"),
        ("user-2", "user", "second", 2, "{}"),
        (
            "assistant-2",
            "assistant",
            "second answer",
            3,
            r#"{"metrics":{"llmRequestIds":["run-2"]}}"#,
        ),
        ("user-3", "user", "third", 4, "{}"),
        (
            "assistant-3",
            "assistant",
            "third answer",
            5,
            r#"{"metrics":{"llmRequestIds":["run-3"]}}"#,
        ),
    ] {
        database
            .insert_message(NewMessage {
                id,
                chat_id: "chat-rewrite",
                role,
                content,
                sequence,
                metadata_json: Some(metadata_json),
            })
            .expect("message insert");
    }
    for (id, sequence) in [
        ("stable", None),
        ("turn-before", Some(0)),
        ("turn-at", Some(2)),
        ("turn-after", Some(4)),
    ] {
        database
            .insert_prompt_context_injection(NewPromptContextInjection {
                id,
                chat_id: "chat-rewrite",
                kind: if sequence.is_some() {
                    "turn_memory"
                } else {
                    "stable"
                },
                sequence,
                messages_json: "[]",
                memory_keys_json: "[]",
                memory_summaries_json: "[]",
            })
            .expect("prompt injection insert");
    }
    for (id, sequence, source_end) in [
        ("snapshot-before", 0, 1),
        ("snapshot-overlap", 1, 2),
        ("snapshot-after", 4, 4),
    ] {
        database
            .insert_context_compression_snapshot(NewContextCompressionSnapshot {
                id,
                chat_id: "chat-rewrite",
                run_id: id,
                sequence,
                summary: id,
                source_message_start_sequence: 0,
                source_message_end_sequence: source_end,
                original_token_count: 10,
                summary_token_count: 2,
                metadata_json: None,
            })
            .expect("compression snapshot insert");
    }

    let result = database
        .rewrite_chat_from_user_message(RewriteChatFromUserMessage {
            chat_id: "chat-rewrite",
            user_message_id: "user-2",
            expected_content: Some("second"),
            content: "second edited",
            user_metadata_json: r#"{"attachments":[{"id":"attachment-1","name":"notes.txt","contentType":"text/plain","sizeBytes":5,"contentBase64":"aGVsbG8="}],"modelId":"model-new","selectedSkillIds":["skill-1"],"queuedRun":{"status":"queued","assistantMessageId":"assistant-edited","assistantSequence":3,"modelId":"model-new","skillIds":["skill-1"]}}"#,
            chat_queued_run_json: r#"{"status":"queued","userMessageId":"user-2","assistantMessageId":"assistant-edited","assistantSequence":3,"modelId":"model-new","skillIds":["skill-1"],"content":"second edited"}"#,
            assistant_message_id: "assistant-edited",
            assistant_metadata_json: r#"{"streamingState":"streaming"}"#,
            coordinator_task_id: None,
            coordinator_task_input_json: None,
            invalidated_reason: "test rewrite",
            memory_invalidation_reason: "test rewrite",
        })
        .expect("rewrite chat");

    assert_eq!(
        result.removed_message_ids,
        vec!["assistant-2", "user-3", "assistant-3"]
    );
    let messages = database
        .messages_for_chat("chat-rewrite")
        .expect("rewritten messages");
    assert_eq!(
        messages
            .iter()
            .map(|message| (
                message.id.as_str(),
                message.content.as_str(),
                message.sequence
            ))
            .collect::<Vec<_>>(),
        vec![
            ("user-1", "first", 0),
            ("assistant-1", "first answer", 1),
            ("user-2", "second edited", 2),
            ("assistant-edited", "", 3),
        ]
    );
    let edited_metadata: Value =
        serde_json::from_str(&messages[2].metadata_json).expect("metadata");
    assert_eq!(edited_metadata["attachments"][0]["name"], "notes.txt");
    assert_eq!(edited_metadata["selectedSkillIds"][0], "skill-1");
    let injections = database
        .prompt_context_injections_for_chat("chat-rewrite")
        .expect("prompt injections");
    assert_eq!(
        injections
            .iter()
            .map(|injection| injection.id.as_str())
            .collect::<Vec<_>>(),
        vec!["stable", "turn-before"]
    );
    let snapshots = database
        .context_compression_snapshots_for_chat("chat-rewrite")
        .expect("compression snapshots");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].id, "snapshot-before");
}

#[test]
fn rewrite_chat_from_user_message_handles_first_and_terminal_user_turns() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");

    database
        .insert_chat("chat-first-turn", "First turn")
        .expect("first chat insert");
    for (id, role, content, sequence) in [
        ("first-user-1", "user", "first", 0),
        ("first-assistant-1", "assistant", "first answer", 1),
        ("first-user-2", "user", "second", 2),
        ("first-assistant-2", "assistant", "second answer", 3),
    ] {
        database
            .insert_message(NewMessage {
                id,
                chat_id: "chat-first-turn",
                role,
                content,
                sequence,
                metadata_json: Some("{}"),
            })
            .expect("first chat message insert");
    }
    let first_result = database
        .rewrite_chat_from_user_message(RewriteChatFromUserMessage {
            chat_id: "chat-first-turn",
            user_message_id: "first-user-1",
            expected_content: Some("first"),
            content: "first edited",
            user_metadata_json: r#"{"queuedRun":{"status":"queued"}}"#,
            chat_queued_run_json: r#"{"status":"queued"}"#,
            assistant_message_id: "first-assistant-new",
            assistant_metadata_json: r#"{"streamingState":"streaming"}"#,
            coordinator_task_id: None,
            coordinator_task_input_json: None,
            invalidated_reason: "test rewrite",
            memory_invalidation_reason: "test rewrite",
        })
        .expect("rewrite first turn");
    assert_eq!(
        first_result.removed_message_ids,
        vec!["first-assistant-1", "first-user-2", "first-assistant-2"]
    );
    assert_eq!(
        database
            .messages_for_chat("chat-first-turn")
            .expect("first chat messages")
            .into_iter()
            .map(|message| (message.id, message.content, message.sequence))
            .collect::<Vec<_>>(),
        vec![
            ("first-user-1".to_string(), "first edited".to_string(), 0),
            ("first-assistant-new".to_string(), String::new(), 1),
        ]
    );

    database
        .insert_chat("chat-terminal-turn", "Terminal turn")
        .expect("terminal chat insert");
    database
        .insert_message(NewMessage {
            id: "terminal-user",
            chat_id: "chat-terminal-turn",
            role: "user",
            content: "terminal",
            sequence: 0,
            metadata_json: Some("{}"),
        })
        .expect("terminal user insert");
    let terminal_result = database
        .rewrite_chat_from_user_message(RewriteChatFromUserMessage {
            chat_id: "chat-terminal-turn",
            user_message_id: "terminal-user",
            expected_content: Some("terminal"),
            content: "terminal edited",
            user_metadata_json: r#"{"queuedRun":{"status":"queued"}}"#,
            chat_queued_run_json: r#"{"status":"queued"}"#,
            assistant_message_id: "terminal-assistant-new",
            assistant_metadata_json: r#"{"streamingState":"streaming"}"#,
            coordinator_task_id: None,
            coordinator_task_input_json: None,
            invalidated_reason: "test rewrite",
            memory_invalidation_reason: "test rewrite",
        })
        .expect("rewrite terminal turn");
    assert!(terminal_result.removed_message_ids.is_empty());
    assert_eq!(
        database
            .messages_for_chat("chat-terminal-turn")
            .expect("terminal chat messages")
            .into_iter()
            .map(|message| (message.id, message.content, message.sequence))
            .collect::<Vec<_>>(),
        vec![
            (
                "terminal-user".to_string(),
                "terminal edited".to_string(),
                0,
            ),
            ("terminal-assistant-new".to_string(), String::new(), 1),
        ]
    );
}

#[test]
fn rewrite_chat_from_user_message_rolls_back_when_new_assistant_conflicts() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("database");
    database
        .insert_chat("chat-rollback", "Rollback")
        .expect("chat insert");
    for (id, role, content, sequence) in [
        ("user-1", "user", "original", 0),
        ("assistant-1", "assistant", "answer", 1),
        ("assistant-conflict", "assistant", "existing", 2),
    ] {
        database
            .insert_message(NewMessage {
                id,
                chat_id: "chat-rollback",
                role,
                content,
                sequence,
                metadata_json: Some("{}"),
            })
            .expect("message insert");
    }

    let original_chat = database
        .chat("chat-rollback")
        .expect("chat read")
        .expect("chat");
    let original_messages = database
        .messages_for_chat("chat-rollback")
        .expect("original messages");
    database
        .rewrite_chat_from_user_message(RewriteChatFromUserMessage {
            chat_id: "chat-rollback",
            user_message_id: "user-1",
            expected_content: Some("stale original"),
            content: "edited",
            user_metadata_json: r#"{"queuedRun":{"status":"queued"}}"#,
            chat_queued_run_json: r#"{"status":"queued"}"#,
            assistant_message_id: "assistant-new",
            assistant_metadata_json: "{}",
            coordinator_task_id: None,
            coordinator_task_input_json: None,
            invalidated_reason: "test rewrite",
            memory_invalidation_reason: "test rewrite",
        })
        .expect_err("expected content conflict should roll back");

    let messages = database
        .messages_for_chat("chat-rollback")
        .expect("messages after rollback");
    assert_eq!(messages, original_messages);
    let chat = database
        .chat("chat-rollback")
        .expect("chat read")
        .expect("chat");
    assert_eq!(chat, original_chat);
}

fn explain_query_plan(connection: &Connection, sql: &str) -> String {
    explain_query_plan_rows(connection, sql)
        .into_iter()
        .map(|row| format!("{}|{}|{}|{}", row.id, row.parent, row.not_used, row.detail))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone)]
struct QueryPlanRow {
    id: i64,
    parent: i64,
    not_used: i64,
    detail: String,
}

fn explain_query_plan_rows(connection: &Connection, sql: &str) -> Vec<QueryPlanRow> {
    let mut statement = connection
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .expect("prepare explain");
    let parameter_count = statement.parameter_count();
    // Prefer non-null text binds so range/equality estimates stay realistic for planner tests.
    let binds = (0..parameter_count)
        .map(|index| rusqlite::types::Value::Text(format!("explain-bind-{index}")))
        .collect::<Vec<_>>();
    let rows = statement
        .query_map(rusqlite::params_from_iter(binds), |row| {
            Ok(QueryPlanRow {
                id: row.get(0)?,
                parent: row.get(1)?,
                not_used: row.get(2)?,
                detail: row.get(3)?,
            })
        })
        .expect("explain rows");
    rows.map(|row| row.expect("explain row")).collect()
}

#[derive(Debug, Default)]
struct QueryPlanAnalysis {
    details: Vec<String>,
    searches: Vec<String>,
    unconstrained_table_scans: Vec<String>,
    indexes_used: Vec<String>,
    uses_temp_b_tree: bool,
}

fn analyze_query_plan(plan: &str) -> QueryPlanAnalysis {
    let mut analysis = QueryPlanAnalysis::default();
    for line in plan.lines() {
        let detail = line
            .rsplit_once('|')
            .map(|(_, detail)| detail.trim())
            .unwrap_or(line.trim());
        if detail.is_empty() {
            continue;
        }
        analysis.details.push(detail.to_string());
        if detail.contains("USE TEMP B-TREE") {
            analysis.uses_temp_b_tree = true;
        }
        if detail.starts_with("SEARCH ") {
            analysis.searches.push(detail.to_string());
        }
        if let Some(index_name) = index_name_from_plan_detail(detail) {
            analysis.indexes_used.push(index_name.to_string());
        }
        if is_unconstrained_table_scan_detail(detail) {
            analysis.unconstrained_table_scans.push(detail.to_string());
        }
    }
    analysis
}

fn index_name_from_plan_detail(detail: &str) -> Option<&str> {
    for marker in ["USING COVERING INDEX ", "USING INDEX "] {
        if let Some(rest) = detail.split_once(marker).map(|(_, rest)| rest) {
            let name = rest
                .split(|ch: char| ch.is_whitespace() || ch == '(')
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

fn is_unconstrained_table_scan_detail(detail: &str) -> bool {
    let trimmed = detail.trim();
    if !trimmed.starts_with("SCAN ") {
        return false;
    }
    // Index-backed or rowid primary-key access is constrained.
    if trimmed.contains("USING INDEX ")
        || trimmed.contains("USING COVERING INDEX ")
        || trimmed.contains("USING INTEGER PRIMARY KEY")
        || trimmed.contains("USING ROWID")
    {
        return false;
    }
    // Ignore synthetic nodes.
    if trimmed.contains("CONSTANT ROW")
        || trimmed.contains("SUBQUERY")
        || trimmed.contains("CO-ROUTINE")
        || trimmed.contains("AUTOMATIC COVERING INDEX")
        || trimmed.contains("AUTOMATIC INDEX")
    {
        return false;
    }
    true
}

fn plan_uses_index(plan: &str, index_name: &str) -> bool {
    analyze_query_plan(plan)
        .indexes_used
        .iter()
        .any(|used| used == index_name)
}

fn plan_has_unconstrained_scan_on(plan: &str, table: &str) -> bool {
    analyze_query_plan(plan)
        .unconstrained_table_scans
        .iter()
        .any(|detail| {
            // Match "SCAN table" / "SCAN table AS alias" but not "SCAN tables_other".
            let after_scan = detail.trim_start_matches("SCAN ").trim_start();
            after_scan == table
                || after_scan.starts_with(&format!("{table} "))
                || after_scan.starts_with(&format!("{table}\t"))
        })
}

/// Expand `?` / `?N` placeholders with representative literals for EXPLAIN QUERY PLAN.
fn sql_with_explain_literals(sql: &str, values: &[&str]) -> String {
    let mut out = String::with_capacity(sql.len() + values.len() * 8);
    let mut value_index = 0usize;
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '?' {
            i += 1;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let value = values.get(value_index).copied().unwrap_or("NULL");
            value_index += 1;
            if value == "NULL" || value.chars().all(|ch| ch.is_ascii_digit() || ch == '-') {
                out.push_str(value);
            } else {
                out.push('\'');
                out.push_str(&value.replace('\'', "''"));
                out.push('\'');
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn assert_plan_uses_index(plan: &str, index_name: &str) {
    assert!(
        plan_uses_index(plan, index_name),
        "expected index {index_name} in plan analysis {:?}, full plan:\n{plan}",
        analyze_query_plan(plan).indexes_used
    );
}

fn assert_no_unconstrained_table_scan(plan: &str, table: &str) {
    assert!(
        !plan_has_unconstrained_scan_on(plan, table),
        "unexpected unconstrained SCAN on {table}: {:?}\nfull plan:\n{plan}",
        analyze_query_plan(plan).unconstrained_table_scans
    );
}

#[test]
fn has_user_message_since_uses_partial_user_created_at_index() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-history", "History")
        .expect("chat insert");
    drop(database);

    let connection =
        Connection::open(workspace_database_path(workspace.path())).expect("open database");
    connection.execute_batch("BEGIN;").expect("begin fixture");
    let mut insert = connection
        .prepare(
            "INSERT INTO messages (id, chat_id, role, content, sequence, created_at, metadata_json)
             VALUES (?1, 'chat-history', ?2, 'bulk', ?3, ?4, '{}')",
        )
        .expect("prepare insert");
    for sequence in 0..12_000 {
        let role = if sequence % 20 == 0 {
            "user"
        } else {
            "assistant"
        };
        // Keep most history old; one recent user row proves early exit on the partial index.
        let created_at = if sequence == 11_980 {
            "2026-07-10T12:00:00.000Z"
        } else {
            "2026-01-01T00:00:00.000Z"
        };
        insert
            .execute(params![
                format!("msg-{sequence}"),
                role,
                sequence,
                created_at
            ])
            .expect("message insert");
    }
    connection.execute_batch("COMMIT;").expect("commit fixture");
    drop(insert);
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopen database");
    assert!(
        database
            .has_user_message_since("2026-07-01T00:00:00.000Z")
            .expect("recent activity")
    );
    assert!(
        !database
            .has_user_message_since("2026-07-11T00:00:00.000Z")
            .expect("future activity")
    );

    let connection = Connection::open(database.database_path()).expect("open database");
    let plan = explain_query_plan(
        &connection,
        "SELECT EXISTS(
             SELECT 1
             FROM messages
             WHERE role = 'user' AND created_at >= '2026-07-01T00:00:00.000Z'
             LIMIT 1
         )",
    );
    assert_plan_uses_index(&plan, "messages_user_created_at_idx");
    assert_no_unconstrained_table_scan(&plan, "messages");
    let analysis = analyze_query_plan(&plan);
    assert!(
        !analysis.searches.is_empty() || plan_uses_index(&plan, "messages_user_created_at_idx"),
        "expected SEARCH or indexed access, plan:\n{plan}"
    );
}

#[test]
fn chat_kind_filters_use_partial_indexes_and_sql_where() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    drop(database);

    let connection =
        Connection::open(workspace_database_path(workspace.path())).expect("open database");
    connection.execute_batch("BEGIN;").expect("begin fixture");
    {
        let mut insert = connection
            .prepare(
                "INSERT INTO chats (id, title, created_at, updated_at, archived_at, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
            )
            .expect("prepare chat insert");
        for index in 0..2_000 {
            insert
                .execute(params![
                    format!("chat-{index}"),
                    format!("Chat {index}"),
                    format!("2026-06-01T{:02}:00:00.000Z", index % 24),
                    format!("2026-06-02T{:02}:00:00.000Z", index % 24),
                    "{}"
                ])
                .expect("chat insert");
        }
        for index in 0..200 {
            insert
                .execute(params![
                    format!("dream-{index}"),
                    format!("Dream {index}"),
                    format!("2026-06-03T{:02}:00:00.000Z", index % 24),
                    format!("2026-06-04T{:02}:00:00.000Z", index % 24),
                    r#"{"kind":"memory_dream"}"#
                ])
                .expect("dream chat insert");
        }
    }
    connection.execute_batch("COMMIT;").expect("commit fixture");
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopen database");
    let normal = database.chats().expect("normal chats");
    assert_eq!(normal.len(), 2_000);
    assert!(
        normal
            .iter()
            .all(|chat| !chat.metadata_json.contains("memory_dream"))
    );
    let dream = database.dream_transcript_chats().expect("dream chats");
    assert_eq!(dream.len(), 200);
    assert!(dream.iter().all(|chat| {
        chat.metadata_json
            .contains(MEMORY_DREAM_TRANSCRIPT_CHAT_KIND)
    }));

    let page = database.chat_page(50, None).expect("chat page");
    assert_eq!(page.total_count, 2_000);
    assert_eq!(page.chats.len(), 50);

    let connection = Connection::open(database.database_path()).expect("open database");
    // Production visible pagination SQL (chat_page_matching_title without title query).
    let visible_plan = explain_query_plan(
        &connection,
        "SELECT id, title, created_at, updated_at, archived_at, metadata_json
         FROM chats
         WHERE COALESCE(json_extract(metadata_json, '$.kind'), '') != 'memory_dream'
         ORDER BY updated_at DESC, created_at DESC, id DESC
         LIMIT 51",
    );
    // Must use the memory_dream-excluding partial index; falling back only to the generic
    // chats_updated_created_id_idx is no longer accepted.
    assert_plan_uses_index(&visible_plan, "chats_visible_updated_created_id_idx");
    assert_no_unconstrained_table_scan(&visible_plan, "chats");

    let visible_count_plan = explain_query_plan(
        &connection,
        "SELECT COUNT(*)
         FROM chats
         WHERE COALESCE(json_extract(metadata_json, '$.kind'), '') != 'memory_dream'",
    );
    assert_plan_uses_index(&visible_count_plan, "chats_visible_updated_created_id_idx");
    assert_no_unconstrained_table_scan(&visible_count_plan, "chats");

    let dream_plan = explain_query_plan(
        &connection,
        "SELECT id, title, created_at, updated_at, archived_at, metadata_json
         FROM chats
         WHERE json_extract(metadata_json, '$.kind') = 'memory_dream'
         ORDER BY updated_at DESC, created_at DESC, id DESC",
    );
    assert_plan_uses_index(&dream_plan, "chats_memory_dream_updated_created_id_idx");
    assert_no_unconstrained_table_scan(&dream_plan, "chats");

    // Title substring search is interactive and intentionally may scan;
    // chats_title_nocase_idx cannot optimize leading-wildcard LIKE '%query%'.
    let title_plan = explain_query_plan(
        &connection,
        "SELECT id FROM chats
         WHERE COALESCE(json_extract(metadata_json, '$.kind'), '') != 'memory_dream'
           AND title LIKE '%Chat 1%' ESCAPE '\\' COLLATE NOCASE
         ORDER BY updated_at DESC, created_at DESC, id DESC
         LIMIT 20",
    );
    let title_analysis = analyze_query_plan(&title_plan);
    assert!(
        !title_analysis.details.is_empty(),
        "title search plan should be available for interactive exception documentation"
    );
    // Document accepted interactive exception: leading-wildcard substring may unconstrained-scan.
    let _accepted_title_substring_scan = plan_has_unconstrained_scan_on(&title_plan, "chats");
}

#[test]
fn llm_request_audit_query_plans_cover_rows_count_summary_and_breakdown() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database
        .insert_chat("chat-stats", "Stats")
        .expect("chat insert");
    database
        .insert_chat("chat-other", "Other")
        .expect("chat insert");
    drop(database);

    let connection =
        Connection::open(workspace_database_path(workspace.path())).expect("open database");
    connection.execute_batch("BEGIN;").expect("begin fixture");
    {
        let mut insert = connection
            .prepare(
                "INSERT INTO llm_requests (
                   id, workspace_id, chat_id, request_kind, provider_id, model_id,
                   request_started_at, completed_at, input_tokens, output_tokens,
                   cache_read_tokens, cache_write_tokens, reasoning_tokens,
                   total_latency_ms, status_code, final_state, invalidated_at
                 ) VALUES (?1, 'workspace-1', ?2, ?3, 'provider-a', 'model-a',
                           ?4, ?4, ?5, 5, 1, 1, 0, ?6, 200, ?7, ?8)",
            )
            .expect("prepare llm insert");
        for index in 0..12_000 {
            let day = (index % 14) + 1;
            let started_at = format!("2026-07-{day:02}T12:00:00.000Z");
            let kind = match index % 23 {
                0 => "contextCompression",
                1 => "prompt hook",
                2 => "chat title generation",
                _ => "chat completion",
            };
            let chat_id = if index % 11 == 0 {
                "chat-other"
            } else {
                "chat-stats"
            };
            let final_state = if index % 31 == 0 {
                "failed"
            } else if index % 47 == 0 {
                "cancelled"
            } else {
                "succeeded"
            };
            let invalidated_at: Option<&str> = if index % 53 == 0 {
                Some("2026-07-14T00:00:00.000Z")
            } else {
                None
            };
            insert
                .execute(params![
                    format!("req-{index}"),
                    chat_id,
                    kind,
                    started_at,
                    10 + (index % 7),
                    20 + (index % 13),
                    final_state,
                    invalidated_at,
                ])
                .expect("llm request insert");
        }
    }
    connection.execute_batch("COMMIT;").expect("commit fixture");
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopen database");

    let default_filters = LlmRequestAuditFilters {
        request_ids: &[],
        workspace_id: None,
        chat_id: None,
        request_kind: None,
        exclude_request_kinds: MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS,
        provider_id: None,
        model_id: None,
        final_state: None,
        started_after: Some("2026-07-07T00:00:00.000Z"),
        started_before: None,
        valid_only: true,
        limit: Some(100),
        offset: Some(0),
    };
    let kind_filters = LlmRequestAuditFilters {
        request_kind: Some("contextCompression"),
        exclude_request_kinds: &[],
        ..default_filters
    };
    let chat_valid_filters = LlmRequestAuditFilters {
        chat_id: Some("chat-stats"),
        exclude_request_kinds: MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS,
        valid_only: true,
        ..default_filters
    };

    let rows = database
        .llm_request_audit_rows(default_filters)
        .expect("audit rows");
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|row| {
        !MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS.contains(&row.request_kind.as_str())
            && row.invalidated_at.is_none()
            && row.request_started_at.as_str() >= "2026-07-07T00:00:00.000Z"
    }));

    let count = database
        .llm_request_audit_count(default_filters)
        .expect("audit count");
    assert!(count >= rows.len() as i64);

    let summary = database
        .llm_request_audit_summary(default_filters)
        .expect("audit summary");
    assert_eq!(summary.total_requests, count);
    assert!(
        summary.failed_requests > 0,
        "fixture seeds failed/cancelled rows that count as failedRequests"
    );
    assert!(summary.total_tokens > 0);
    assert!(summary.latency_sum > 0);

    let kind_rows = database
        .llm_request_audit_rows(kind_filters)
        .expect("kind rows");
    assert!(!kind_rows.is_empty());
    assert!(
        kind_rows
            .iter()
            .all(|row| row.request_kind == "contextCompression")
    );

    let chat_rows = database
        .llm_request_audit_rows(chat_valid_filters)
        .expect("chat rows");
    assert!(!chat_rows.is_empty());
    assert!(chat_rows.iter().all(|row| {
        row.chat_id.as_deref() == Some("chat-stats") && row.invalidated_at.is_none()
    }));

    let breakdown = database
        .llm_request_audit_request_kind_breakdown(default_filters)
        .expect("kind breakdown");
    assert!(!breakdown.is_empty());
    let completion = breakdown
        .iter()
        .find(|row| row.request_kind == "chat completion")
        .expect("chat completion breakdown");
    assert!(completion.request_count > 0);
    // failedRequests includes cancelled + failed (not only HTTP failures).
    assert!(
        completion.failed_requests > 0,
        "fixture includes failed/cancelled chat completion rows"
    );

    let connection = Connection::open(database.database_path()).expect("open database");

    // Homology: production builders shape the EXPLAIN SQL (shared WHERE builder).
    let rows_sql = llm_request_audit_rows_sql_for_tests(default_filters);
    assert!(rows_sql.contains("request_started_at >= ?"));
    assert!(rows_sql.contains("invalidated_at IS NULL"));
    assert!(rows_sql.contains("request_kind NOT IN"));
    let mut default_binds = vec!["2026-07-07T00:00:00.000Z"];
    default_binds.extend(MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS.iter().copied());
    default_binds.push("100");
    default_binds.push("0");
    let rows_plan = explain_query_plan(
        &connection,
        &sql_with_explain_literals(&rows_sql, &default_binds),
    );
    // Default window: started_at range and/or request_kind exclusion. Real planner may pick
    // llm_requests_request_kind_idx for NOT IN + date predicates without ANALYZE stats.
    // Reject unconstrained table scans and unrelated indexes (e.g. chat_valid alone).
    assert!(
        plan_uses_index(&rows_plan, "llm_requests_started_at_idx")
            || plan_uses_index(&rows_plan, "llm_requests_request_kind_idx"),
        "default window rows should use started_at or request_kind index, plan:\n{rows_plan}"
    );
    assert_no_unconstrained_table_scan(&rows_plan, "llm_requests");

    let kind_rows_sql = llm_request_audit_rows_sql_for_tests(kind_filters);
    assert!(kind_rows_sql.contains("request_kind = ?"));
    let kind_rows_plan = explain_query_plan(
        &connection,
        &sql_with_explain_literals(
            &kind_rows_sql,
            &["contextCompression", "2026-07-07T00:00:00.000Z", "100", "0"],
        ),
    );
    assert!(
        plan_uses_index(&kind_rows_plan, "llm_requests_request_kind_idx")
            || plan_uses_index(&kind_rows_plan, "llm_requests_started_at_idx"),
        "explicit requestKind filter should use kind or started_at index, plan:\n{kind_rows_plan}"
    );
    assert_no_unconstrained_table_scan(&kind_rows_plan, "llm_requests");

    let chat_rows_sql = llm_request_audit_rows_sql_for_tests(chat_valid_filters);
    assert!(chat_rows_sql.contains("chat_id = ?"));
    let mut chat_binds = vec!["chat-stats", "2026-07-07T00:00:00.000Z"];
    chat_binds.extend(MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS.iter().copied());
    chat_binds.push("100");
    chat_binds.push("0");
    let chat_rows_plan = explain_query_plan(
        &connection,
        &sql_with_explain_literals(&chat_rows_sql, &chat_binds),
    );
    assert!(
        plan_uses_index(&chat_rows_plan, "llm_requests_chat_valid_idx")
            || plan_uses_index(&chat_rows_plan, "llm_requests_chat_idx")
            || plan_uses_index(&chat_rows_plan, "llm_requests_started_at_idx"),
        "chatId+valid_only should use chat/valid or started_at index, plan:\n{chat_rows_plan}"
    );
    assert_no_unconstrained_table_scan(&chat_rows_plan, "llm_requests");

    let count_sql = llm_request_audit_count_sql_for_tests(default_filters);
    let mut count_binds = vec!["2026-07-07T00:00:00.000Z"];
    count_binds.extend(MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS.iter().copied());
    let count_plan = explain_query_plan(
        &connection,
        &sql_with_explain_literals(&count_sql, &count_binds),
    );
    assert!(
        plan_uses_index(&count_plan, "llm_requests_started_at_idx")
            || plan_uses_index(&count_plan, "llm_requests_request_kind_idx"),
        "default window count should use started_at or request_kind index, plan:\n{count_plan}"
    );
    assert_no_unconstrained_table_scan(&count_plan, "llm_requests");

    let summary_sql = llm_request_audit_summary_sql_for_tests(default_filters);
    let summary_plan = explain_query_plan(
        &connection,
        &sql_with_explain_literals(&summary_sql, &count_binds),
    );
    assert!(
        plan_uses_index(&summary_plan, "llm_requests_started_at_idx")
            || plan_uses_index(&summary_plan, "llm_requests_request_kind_idx"),
        "default window summary should use started_at or request_kind index, plan:\n{summary_plan}"
    );
    assert_no_unconstrained_table_scan(&summary_plan, "llm_requests");
    // Aggregates may introduce TEMP B-TREE; that is explicitly allowed.
    let _summary_may_use_temp = analyze_query_plan(&summary_plan).uses_temp_b_tree;

    let breakdown_sql = llm_request_audit_request_kind_breakdown_sql_for_tests(default_filters);
    let breakdown_plan = explain_query_plan(
        &connection,
        &sql_with_explain_literals(&breakdown_sql, &count_binds),
    );
    assert!(
        plan_uses_index(&breakdown_plan, "llm_requests_started_at_idx")
            || plan_uses_index(&breakdown_plan, "llm_requests_request_kind_idx"),
        "default window breakdown should use started_at or request_kind index, plan:\n{breakdown_plan}"
    );
    assert_no_unconstrained_table_scan(&breakdown_plan, "llm_requests");
    // GROUP BY request_kind may use TEMP B-TREE; allowed for aggregate paths.
    let _breakdown_may_use_temp = analyze_query_plan(&breakdown_plan).uses_temp_b_tree;

    // Pure time-window (no kind exclusion) must use started_at range index.
    let pure_window = LlmRequestAuditFilters {
        request_ids: &[],
        workspace_id: None,
        chat_id: None,
        request_kind: None,
        exclude_request_kinds: &[],
        provider_id: None,
        model_id: None,
        final_state: None,
        started_after: Some("2026-07-07T00:00:00.000Z"),
        started_before: None,
        valid_only: false,
        limit: Some(100),
        offset: Some(0),
    };
    let pure_sql = llm_request_audit_rows_sql_for_tests(pure_window);
    let pure_plan = explain_query_plan(
        &connection,
        &sql_with_explain_literals(&pure_sql, &["2026-07-07T00:00:00.000Z", "100", "0"]),
    );
    assert_plan_uses_index(&pure_plan, "llm_requests_started_at_idx");
    assert_no_unconstrained_table_scan(&pure_plan, "llm_requests");

    // Decision: no new composite index on llm_requests. Representative plans already use
    // existing started_at/request_kind indexes for the default 7-day window without unconstrained SCAN.
}

#[test]
fn scheduled_tasks_query_plans_use_status_next_run_index() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    drop(database);

    let connection =
        Connection::open(workspace_database_path(workspace.path())).expect("open database");
    connection.execute_batch("BEGIN;").expect("begin fixture");
    {
        let mut insert = connection
            .prepare(
                "INSERT INTO scheduled_tasks (
                    id, title, description, schedule_json, action_json, status,
                    next_run_at, last_run_at, created_at, updated_at, metadata_json
                 ) VALUES (?1, ?2, NULL, '{}', '{}', ?3, ?4, NULL, ?5, ?5, '{}')",
            )
            .expect("prepare scheduled insert");
        for index in 0..12_000 {
            let status = match index % 5 {
                0 => "paused",
                1 => "completed",
                2 => "archived",
                _ => "enabled",
            };
            let next_run_at = if status == "enabled" {
                Some(format!(
                    "2026-07-{:02}T{:02}:00:00.000Z",
                    (index % 28) + 1,
                    index % 24
                ))
            } else {
                None
            };
            let created_at = format!("2026-06-01T{:02}:00:00.000Z", index % 24);
            insert
                .execute(params![
                    format!("scheduled-task-{index}"),
                    format!("Task {index}"),
                    status,
                    next_run_at,
                    created_at,
                ])
                .expect("scheduled task insert");
        }
    }
    connection.execute_batch("COMMIT;").expect("commit fixture");
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopen database");
    let enabled_page = database
        .scheduled_tasks_page(ScheduledTaskListFilter {
            status: Some("enabled"),
            search: None,
            limit: 50,
            offset: 0,
        })
        .expect("enabled page");
    assert_eq!(enabled_page.len(), 50);
    assert!(enabled_page.iter().all(|task| task.status == "enabled"));

    let enabled_count = database
        .scheduled_task_count(ScheduledTaskListFilter {
            status: Some("enabled"),
            search: None,
            limit: 1,
            offset: 0,
        })
        .expect("enabled count");
    assert!(enabled_count >= 50);

    let next_run = database
        .next_enabled_scheduled_task_run_at()
        .expect("next run")
        .expect("some enabled next_run_at");
    assert!(next_run.starts_with("2026-07-"));

    let connection = Connection::open(database.database_path()).expect("open database");
    let due_plan = explain_query_plan(&connection, NEXT_ENABLED_SCHEDULED_TASK_SQL);
    assert_plan_uses_index(&due_plan, "scheduled_tasks_status_next_run_idx");
    assert_no_unconstrained_table_scan(&due_plan, "scheduled_tasks");

    let page_sql = scheduled_tasks_page_sql_for_tests(Some("enabled"), None).expect("page sql");
    assert!(page_sql.contains("status = ?"));
    let page_plan = explain_query_plan(
        &connection,
        &sql_with_explain_literals(&page_sql, &["enabled", "50", "0"]),
    );
    assert_plan_uses_index(&page_plan, "scheduled_tasks_status_next_run_idx");
    assert_no_unconstrained_table_scan(&page_plan, "scheduled_tasks");
    let _page_may_use_temp_for_order = analyze_query_plan(&page_plan).uses_temp_b_tree;

    let count_sql = scheduled_task_count_sql_for_tests(Some("enabled"), None).expect("count sql");
    let count_plan = explain_query_plan(
        &connection,
        &sql_with_explain_literals(&count_sql, &["enabled"]),
    );
    assert_plan_uses_index(&count_plan, "scheduled_tasks_status_next_run_idx");
    assert_no_unconstrained_table_scan(&count_plan, "scheduled_tasks");

    // Decision: no new scheduled_tasks index; status+next_run composite is sufficient.
}

#[test]
fn runnable_agent_tasks_query_plan_uses_runnable_and_dependency_indexes() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    // Structural teams/instances via public APIs; bulk historical rows via SQL for planner scale.
    let mut live_teams = Vec::new();
    for team_index in 0..20 {
        let chat_id = format!("chat-runnable-{team_index}");
        let (team_id, instance_id) =
            create_test_agent_team(&mut database, &chat_id, &format!("runnable-{team_index}"));
        let worker_id = create_test_agent_worker(
            &database,
            &team_id,
            &format!("worker-runnable-{team_index}"),
        );
        live_teams.push((team_id, instance_id, worker_id));
    }
    drop(database);

    let connection =
        Connection::open(workspace_database_path(workspace.path())).expect("open database");
    connection.execute_batch("BEGIN;").expect("begin fixture");
    {
        let mut insert = connection
            .prepare(
                "INSERT INTO agent_tasks (
                    id, team_id, owner_instance_id, origin_instance_id, parent_task_id,
                    sequence, status, input_json, result_json, error_json,
                    created_at, updated_at, started_at, completed_at
                 ) VALUES (?1, ?2, ?3, NULL, NULL, ?4, ?5, '{}', NULL, NULL, ?6, ?6, ?6, ?7)",
            )
            .expect("prepare agent_tasks insert");
        let mut sequence_by_owner = std::collections::HashMap::<String, i64>::new();
        let mut next_sequence = |owner: &str| -> i64 {
            let entry = sequence_by_owner.entry(owner.to_string()).or_insert(0);
            let value = *entry;
            *entry += 1;
            value
        };

        // ~10k completed historical tasks for index selectivity.
        for index in 0..10_000 {
            let team_index = index % live_teams.len();
            let (team_id, instance_id, worker_id) = &live_teams[team_index];
            let owner = if index % 2 == 0 {
                instance_id.as_str()
            } else {
                worker_id.as_str()
            };
            let sequence = next_sequence(owner);
            let created_at = format!("2026-06-01T{:02}:00:00.000Z", index % 24);
            insert
                .execute(params![
                    format!("agent-task-hist-{index}"),
                    team_id.as_str(),
                    owner,
                    sequence,
                    "completed",
                    created_at,
                    created_at,
                ])
                .expect("hist task insert");
        }

        // Queued candidates, sequence-blocked secondaries, and dependency-blocked waiters.
        for (team_index, (team_id, instance_id, worker_id)) in live_teams.iter().enumerate() {
            let owner_seq = next_sequence(instance_id.as_str());
            insert
                .execute(params![
                    format!("agent-task-queued-{team_index}"),
                    team_id.as_str(),
                    instance_id.as_str(),
                    owner_seq,
                    "queued",
                    "2026-07-01T00:00:00.000Z",
                    Option::<String>::None,
                ])
                .expect("queued insert");
            let blocked_seq = next_sequence(instance_id.as_str());
            insert
                .execute(params![
                    format!("agent-task-blocked-seq-{team_index}"),
                    team_id.as_str(),
                    instance_id.as_str(),
                    blocked_seq,
                    "queued",
                    "2026-07-01T00:00:01.000Z",
                    Option::<String>::None,
                ])
                .expect("blocked-seq insert");

            let dep_seq = next_sequence(worker_id.as_str());
            insert
                .execute(params![
                    format!("agent-task-dep-src-{team_index}"),
                    team_id.as_str(),
                    worker_id.as_str(),
                    dep_seq,
                    "queued",
                    "2026-07-01T00:00:02.000Z",
                    Option::<String>::None,
                ])
                .expect("dep source insert");
            let waiting_seq = next_sequence(worker_id.as_str());
            insert
                .execute(params![
                    format!("agent-task-waiting-{team_index}"),
                    team_id.as_str(),
                    worker_id.as_str(),
                    waiting_seq,
                    "queued",
                    "2026-07-01T00:00:03.000Z",
                    Option::<String>::None,
                ])
                .expect("waiting insert");
        }

        let mut dep_insert = connection
            .prepare(
                "INSERT INTO agent_task_dependencies (
                    team_id, waiting_task_id, dependency_task_id, wait_mode,
                    created_at, pending_tool_call_id, deadline_at
                 ) VALUES (?1, ?2, ?3, 'all', '2026-07-01T00:00:04.000Z', NULL, NULL)",
            )
            .expect("prepare dependency insert");
        for (team_index, (team_id, _, _)) in live_teams.iter().enumerate() {
            dep_insert
                .execute(params![
                    team_id.as_str(),
                    format!("agent-task-waiting-{team_index}"),
                    format!("agent-task-dep-src-{team_index}"),
                ])
                .expect("dependency insert");
        }
    }
    connection.execute_batch("COMMIT;").expect("commit fixture");
    drop(connection);

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopen database");
    let runnable = database.runnable_agent_tasks(100).expect("runnable");
    assert!(!runnable.is_empty());
    assert!(
        runnable
            .iter()
            .all(|task| task.status == AgentTaskStatus::Queued)
    );

    let connection = Connection::open(database.database_path()).expect("open database");
    let runnable_sql = sql_with_explain_literals(
        RUNNABLE_AGENT_TASKS_SQL,
        &["2026-07-14T12:00:00.000Z", "100"],
    );
    let plan = explain_query_plan(&connection, &runnable_sql);
    let analysis = analyze_query_plan(&plan);

    // Queued candidate set must use the partial runnable index (not any agent_tasks*).
    assert_plan_uses_index(&plan, "agent_tasks_runnable_idx");
    assert_no_unconstrained_table_scan(&plan, "agent_tasks");
    // Dependency lookups: planner prefers PK (waiting_task_id, dependency_task_id) over
    // agent_task_dependencies_waiting_idx (team_id, waiting_task_id); either is indexed.
    assert!(
        plan_uses_index(&plan, "sqlite_autoindex_agent_task_dependencies_1")
            || plan_uses_index(&plan, "agent_task_dependencies_waiting_idx"),
        "dependency lookup must use PK or waiting_task index, used={:?}\nplan:\n{plan}",
        analysis.indexes_used
    );
    assert!(
        plan_uses_index(&plan, "agent_instances_team_status_idx")
            || plan_uses_index(&plan, "sqlite_autoindex_agent_instances_1"),
        "instance join must use agent_instances PK/status index, used={:?}\nplan:\n{plan}",
        analysis.indexes_used
    );
    assert_no_unconstrained_table_scan(&plan, "agent_instances");
    // ORDER BY last_scheduled_at may require TEMP B-TREE; document as accepted.
    let _sorting_temp_b_tree_is_acceptable = analysis.uses_temp_b_tree;
    // Decision: no new agent indexes; runnable/dependency/instance indexes cover hot paths.
}

#[test]
fn migration_037_indexes_are_created() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let connection = Connection::open(database.database_path()).expect("open database");
    for index_name in [
        "messages_user_created_at_idx",
        "chats_visible_updated_created_id_idx",
        "chats_memory_dream_updated_created_id_idx",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                params![index_name],
                |row| row.get(0),
            )
            .expect("index lookup");
        assert_eq!(count, 1, "missing index {index_name}");
    }
}

#[test]
fn mutate_message_metadata_preserves_unrelated_keys_under_concurrent_connections() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let path = workspace.path().to_path_buf();
    {
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(&path).expect("workspace database");
        database
            .insert_chat("chat-1", "Concurrent metadata")
            .expect("chat");
        database
            .insert_message(NewMessage {
                id: "msg-assistant-1",
                chat_id: "chat-1",
                role: "assistant",
                content: "hello",
                sequence: 0,
                metadata_json: Some("{}"),
            })
            .expect("message");
    }

    let barrier = Arc::new(Barrier::new(2));
    let path_parts = path.clone();
    let barrier_parts = Arc::clone(&barrier);
    let parts_thread = thread::spawn(move || {
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(&path_parts).expect("parts connection");
        barrier_parts.wait();
        database
            .mutate_message_metadata(
                "msg-assistant-1",
                MessageMetadataMutation::SetParts {
                    parts: json!([{"type": "text", "text": "streamed"}]),
                    parts_version: 5,
                    parts_source: "run_events".to_string(),
                },
            )
            .expect("parts mutation");
    });
    let path_spec = path.clone();
    let barrier_spec = Arc::clone(&barrier);
    let spec_thread = thread::spawn(move || {
        let mut database =
            WorkspaceDatabase::open_or_create_ungated(&path_spec).expect("spec connection");
        barrier_spec.wait();
        database
            .mutate_message_metadata(
                "msg-assistant-1",
                MessageMetadataMutation::UpsertSpecUpdate {
                    summary: json!({
                        "id": "job-1-2",
                        "jobId": "job-1",
                        "baseRevision": 1,
                        "revision": 2,
                        "completedAt": "2026-07-14T00:00:00Z",
                        "lines": [{"kind": "added", "text": "line"}],
                        "truncated": false,
                    }),
                },
            )
            .expect("spec mutation");
    });
    parts_thread.join().expect("parts join");
    spec_thread.join().expect("spec join");

    let database = WorkspaceDatabase::open_or_create_ungated(&path).expect("reopen");
    let metadata: Value = serde_json::from_str(
        &database
            .message("msg-assistant-1")
            .expect("message")
            .expect("message row")
            .metadata_json,
    )
    .expect("metadata json");
    assert_eq!(metadata["partsVersion"], 5);
    assert_eq!(metadata["partsSource"], "run_events");
    assert!(metadata["parts"].is_array());
    assert_eq!(metadata["specUpdates"][0]["id"], "job-1-2");

    // Idempotent re-upsert of the same spec update id.
    let mut database = WorkspaceDatabase::open_or_create_ungated(&path).expect("reopen mut");
    database
        .mutate_message_metadata(
            "msg-assistant-1",
            MessageMetadataMutation::UpsertSpecUpdate {
                summary: json!({
                    "id": "job-1-2",
                    "jobId": "job-1",
                    "baseRevision": 1,
                    "revision": 2,
                    "completedAt": "2026-07-14T00:00:00Z",
                    "lines": [{"kind": "added", "text": "line"}],
                    "truncated": false,
                }),
            },
        )
        .expect("idempotent upsert");
    let metadata: Value = serde_json::from_str(
        &database
            .message("msg-assistant-1")
            .expect("message")
            .expect("message row")
            .metadata_json,
    )
    .expect("metadata json");
    assert_eq!(
        metadata["specUpdates"]
            .as_array()
            .expect("specUpdates array")
            .len(),
        1
    );
    assert_eq!(metadata["partsSource"], "run_events");
}

#[test]
fn migration_038_collapses_active_dreams_and_drops_redundant_indexes() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let database_path = workspace_database_path(workspace.path());
    std::fs::create_dir_all(database_path.parent().expect("parent")).expect("mkdir");
    {
        let connection = Connection::open(&database_path).expect("open raw");
        connection
            .execute_batch(
                r#"
                PRAGMA user_version = 37;
                CREATE TABLE chats (
                    id TEXT PRIMARY KEY NOT NULL,
                    title TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    archived_at TEXT,
                    metadata_json TEXT NOT NULL DEFAULT '{}'
                );
                CREATE TABLE memory_dream_jobs (
                    id TEXT PRIMARY KEY NOT NULL,
                    scope TEXT NOT NULL,
                    workspace_id TEXT,
                    trigger_type TEXT NOT NULL,
                    mode TEXT NOT NULL,
                    status TEXT NOT NULL,
                    model_id TEXT,
                    input_summary_json TEXT NOT NULL DEFAULT '{}',
                    output_summary_json TEXT,
                    transcript_chat_id TEXT,
                    error_message TEXT,
                    created_at TEXT NOT NULL,
                    started_at TEXT,
                    completed_at TEXT
                );
                -- All six redundant indexes dropped by MIGRATION_038 (stub tables for DROP safety).
                CREATE INDEX messages_chat_sequence_idx ON chats (id);
                CREATE INDEX run_events_run_sequence_idx ON chats (id);
                CREATE INDEX llm_request_events_request_sequence_idx ON chats (id);
                CREATE INDEX context_compression_snapshots_chat_sequence_idx ON chats (id);
                CREATE INDEX plan_phases_plan_sequence_idx ON chats (id);
                CREATE INDEX plan_steps_phase_sequence_idx ON chats (id);
                INSERT INTO memory_dream_jobs
                    (id, scope, workspace_id, trigger_type, mode, status, input_summary_json, created_at)
                VALUES
                    ('old-queued', 'workspace', 'ws-1', 'manual', 'deterministic_only', 'queued', '{}', '2026-07-01T00:00:00Z'),
                    ('keep-running', 'workspace', 'ws-1', 'manual', 'deterministic_only', 'running', '{}', '2026-07-02T00:00:00Z');
                "#,
            )
            .expect("seed v37 fixture");
    }

    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("migrate to 38");
    assert_eq!(
        database.schema_version().expect("schema version"),
        WORKSPACE_SCHEMA_VERSION
    );
    let connection = Connection::open(database.database_path()).expect("open database");
    let active: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM memory_dream_jobs WHERE status IN ('queued', 'running')",
            [],
            |row| row.get(0),
        )
        .expect("active count");
    assert_eq!(active, 1);
    let kept: String = connection
        .query_row(
            "SELECT id FROM memory_dream_jobs WHERE status IN ('queued', 'running')",
            [],
            |row| row.get(0),
        )
        .expect("kept id");
    assert_eq!(kept, "keep-running");
    let failed_error: String = connection
        .query_row(
            "SELECT error_message FROM memory_dream_jobs WHERE id = 'old-queued'",
            [],
            |row| row.get(0),
        )
        .expect("collapsed error");
    assert!(failed_error.contains("collapsed during schema migration 38"));
    let singleflight: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'memory_dream_jobs_active_singleflight_idx'",
            [],
            |row| row.get(0),
        )
        .expect("singleflight index");
    assert_eq!(singleflight, 1);
    for index_name in [
        "messages_chat_sequence_idx",
        "run_events_run_sequence_idx",
        "llm_request_events_request_sequence_idx",
        "context_compression_snapshots_chat_sequence_idx",
        "plan_phases_plan_sequence_idx",
        "plan_steps_phase_sequence_idx",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [index_name],
                |row| row.get(0),
            )
            .expect("redundant index lookup");
        assert_eq!(count, 0, "expected {index_name} to be dropped");
    }
}

const MIGRATION_038_DROPPED_REDUNDANT_INDEXES: &[&str] = &[
    "messages_chat_sequence_idx",
    "run_events_run_sequence_idx",
    "llm_request_events_request_sequence_idx",
    "context_compression_snapshots_chat_sequence_idx",
    "plan_phases_plan_sequence_idx",
    "plan_steps_phase_sequence_idx",
];

fn named_index_exists(connection: &Connection, name: &str) -> bool {
    connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .expect("index lookup")
        > 0
}

fn assert_plan_uses_unique_autoindex_or_search(plan: &str, table: &str) {
    let lower = plan.to_ascii_lowercase();
    let uses_autoindex = lower.contains("using covering index")
        || lower.contains("using index")
        || lower.contains("autoindex")
        || lower.contains("using integer primary key");
    let searches = lower.contains("search");
    assert!(
        uses_autoindex || searches,
        "expected UNIQUE autoindex/SEARCH for {table}, plan:\n{plan}"
    );
    // Dropped named indexes must not reappear in the plan.
    for index_name in MIGRATION_038_DROPPED_REDUNDANT_INDEXES {
        assert!(
            !plan.contains(index_name),
            "plan must not use dropped index {index_name}:\n{plan}"
        );
    }
}

#[test]
fn migration_038_fresh_and_upgrade_share_dropped_redundant_index_set() {
    // Fresh open (user_version 0 → 38) must never recreate the six dropped indexes.
    let fresh_workspace = tempfile::tempdir().expect("fresh workspace");
    let fresh =
        WorkspaceDatabase::open_or_create_ungated(fresh_workspace.path()).expect("fresh open");
    assert_eq!(
        fresh.schema_version().expect("fresh schema"),
        WORKSPACE_SCHEMA_VERSION
    );
    let fresh_connection = Connection::open(fresh.database_path()).expect("fresh connection");
    for index_name in MIGRATION_038_DROPPED_REDUNDANT_INDEXES {
        assert!(
            !named_index_exists(&fresh_connection, index_name),
            "fresh schema recreated dropped index {index_name}"
        );
    }
    // Retained candidate indexes remain.
    assert!(named_index_exists(
        &fresh_connection,
        "chats_title_nocase_idx"
    ));
    assert!(named_index_exists(
        &fresh_connection,
        "memory_dream_changes_target_fact_ids_idx"
    ));
    assert!(named_index_exists(
        &fresh_connection,
        "memory_dream_changes_new_fact_idx"
    ));
    drop(fresh_connection);
    drop(fresh);

    // Reopen does not recreate dropped indexes.
    let reopened =
        WorkspaceDatabase::open_or_create_ungated(fresh_workspace.path()).expect("reopen");
    let reopened_connection =
        Connection::open(reopened.database_path()).expect("reopen connection");
    for index_name in MIGRATION_038_DROPPED_REDUNDANT_INDEXES {
        assert!(
            !named_index_exists(&reopened_connection, index_name),
            "reopen recreated dropped index {index_name}"
        );
    }
    drop(reopened_connection);
    drop(reopened);

    // Upgrade path already covered by migration_038_collapses_active_dreams_and_drops_redundant_indexes;
    // compare that upgrade also lacks the same six names (seeded then dropped).
    let upgrade_workspace = tempfile::tempdir().expect("upgrade workspace");
    let database_path = workspace_database_path(upgrade_workspace.path());
    std::fs::create_dir_all(database_path.parent().expect("parent")).expect("mkdir");
    {
        let connection = Connection::open(&database_path).expect("open raw");
        connection
            .execute_batch(
                r#"
                PRAGMA user_version = 37;
                CREATE TABLE chats (
                    id TEXT PRIMARY KEY NOT NULL,
                    title TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    archived_at TEXT,
                    metadata_json TEXT NOT NULL DEFAULT '{}'
                );
                CREATE TABLE memory_dream_jobs (
                    id TEXT PRIMARY KEY NOT NULL,
                    scope TEXT NOT NULL,
                    workspace_id TEXT,
                    trigger_type TEXT NOT NULL,
                    mode TEXT NOT NULL,
                    status TEXT NOT NULL,
                    model_id TEXT,
                    input_summary_json TEXT NOT NULL DEFAULT '{}',
                    output_summary_json TEXT,
                    transcript_chat_id TEXT,
                    error_message TEXT,
                    created_at TEXT NOT NULL,
                    started_at TEXT,
                    completed_at TEXT
                );
                CREATE INDEX messages_chat_sequence_idx ON chats (id);
                CREATE INDEX run_events_run_sequence_idx ON chats (id);
                CREATE INDEX llm_request_events_request_sequence_idx ON chats (id);
                CREATE INDEX context_compression_snapshots_chat_sequence_idx ON chats (id);
                CREATE INDEX plan_phases_plan_sequence_idx ON chats (id);
                CREATE INDEX plan_steps_phase_sequence_idx ON chats (id);
                "#,
            )
            .expect("seed v37");
    }
    let upgraded =
        WorkspaceDatabase::open_or_create_ungated(upgrade_workspace.path()).expect("upgrade");
    let upgraded_connection =
        Connection::open(upgraded.database_path()).expect("upgrade connection");
    for index_name in MIGRATION_038_DROPPED_REDUNDANT_INDEXES {
        assert!(
            !named_index_exists(&upgraded_connection, index_name),
            "upgrade left dropped index {index_name}"
        );
    }
}

#[test]
fn dropped_redundant_sequence_indexes_still_use_unique_autoindex() {
    let workspace = tempfile::tempdir().expect("workspace");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    database.insert_chat("chat-seq", "Seq").expect("chat");
    for sequence in 0..40 {
        database
            .insert_message(NewMessage {
                id: &format!("msg-{sequence}"),
                chat_id: "chat-seq",
                role: "user",
                content: "body",
                sequence,
                metadata_json: None,
            })
            .expect("message");
        database
            .insert_run_event(NewRunEvent {
                id: &format!("ev-{sequence}"),
                chat_id: "chat-seq",
                run_id: "run-1",
                sequence,
                event_type: "text",
                payload_json: "{}",
            })
            .expect("run event");
    }
    database
        .insert_llm_request(NewLlmRequest {
            id: "llm-1",
            workspace_id: "ws",
            chat_id: Some("chat-seq"),
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id: "p",
            model_id: "m",
            thinking_level: None,
            request_started_at: "2026-07-14T00:00:00.000Z",
            first_token_at: None,
            completed_at: None,
            input_tokens: Some(1),
            output_tokens: Some(1),
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: Some(200),
            final_state: "succeeded",
            request_body_json: None,
            response_body_json: None,
        })
        .expect("llm request");
    for sequence in 0..10 {
        database
            .insert_llm_request_event(NewLlmRequestEvent {
                id: &format!("lre-{sequence}"),
                llm_request_id: "llm-1",
                sequence,
                event_at: "2026-07-14T00:00:00.000Z",
                event_type: if sequence == 0 { "start" } else { "delta" },
                raw_chunk_json: None,
                normalized_event_json: "{}",
            })
            .expect("llm event");
    }
    for sequence in 0..5 {
        database
            .insert_context_compression_snapshot(NewContextCompressionSnapshot {
                id: &format!("snap-{sequence}"),
                chat_id: "chat-seq",
                run_id: "run-1",
                sequence,
                summary: "summary",
                source_message_start_sequence: 0,
                source_message_end_sequence: sequence,
                original_token_count: 10,
                summary_token_count: 2,
                metadata_json: None,
            })
            .expect("snapshot");
    }
    let plan = database
        .create_plan(NewPlan {
            id: "plan-1",
            title: "Plan",
            overview: "overview",
            status: "ready",
            source_chat_id: None,
            phases: vec![NewPlanPhase {
                id: "phase-1",
                title: "Phase",
                summary: "summary",
                steps: vec![
                    NewPlanStep {
                        id: "step-1",
                        title: "Step 1",
                        detail: "detail",
                        acceptance: vec![],
                    },
                    NewPlanStep {
                        id: "step-2",
                        title: "Step 2",
                        detail: "detail",
                        acceptance: vec![],
                    },
                ],
            }],
        })
        .expect("plan");
    assert_eq!(plan.phases.len(), 1);
    drop(database);

    let connection =
        Connection::open(workspace_database_path(workspace.path())).expect("open database");
    for index_name in MIGRATION_038_DROPPED_REDUNDANT_INDEXES {
        assert!(
            !named_index_exists(&connection, index_name),
            "named index still present: {index_name}"
        );
    }

    let messages_plan = explain_query_plan(
        &connection,
        "SELECT id FROM messages WHERE chat_id = 'chat-seq' ORDER BY sequence ASC",
    );
    assert_plan_uses_unique_autoindex_or_search(&messages_plan, "messages");

    let run_events_plan = explain_query_plan(
        &connection,
        "SELECT id FROM run_events WHERE run_id = 'run-1' ORDER BY sequence ASC",
    );
    assert_plan_uses_unique_autoindex_or_search(&run_events_plan, "run_events");

    let llm_events_plan = explain_query_plan(
        &connection,
        "SELECT id FROM llm_request_events WHERE llm_request_id = 'llm-1' ORDER BY sequence ASC",
    );
    assert_plan_uses_unique_autoindex_or_search(&llm_events_plan, "llm_request_events");

    let snapshots_plan = explain_query_plan(
        &connection,
        "SELECT id FROM context_compression_snapshots WHERE chat_id = 'chat-seq' ORDER BY sequence ASC",
    );
    assert_plan_uses_unique_autoindex_or_search(&snapshots_plan, "context_compression_snapshots");

    let phases_plan = explain_query_plan(
        &connection,
        "SELECT id FROM plan_phases WHERE plan_id = 'plan-1' ORDER BY sequence ASC",
    );
    assert_plan_uses_unique_autoindex_or_search(&phases_plan, "plan_phases");

    let steps_plan = explain_query_plan(
        &connection,
        "SELECT id FROM plan_steps WHERE phase_id = 'phase-1' ORDER BY sequence ASC",
    );
    assert_plan_uses_unique_autoindex_or_search(&steps_plan, "plan_steps");

    // Result ordering still matches production helpers after drop.
    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("reopen database");
    let messages = database.messages_for_chat("chat-seq").expect("messages");
    assert_eq!(messages.len(), 40);
    assert!(messages.windows(2).all(|w| w[0].sequence < w[1].sequence));
    let events = database.run_events_for_run("run-1").expect("events");
    assert_eq!(events.len(), 40);
    assert!(events.windows(2).all(|w| w[0].sequence < w[1].sequence));
}

#[test]
fn retained_index_candidates_have_production_homology_evidence() {
    // Phase 3 candidate decision record (no DROP in this plan):
    // - chats_title_nocase_idx: keep; leading-wildcard LIKE '%query%' cannot use it
    // - memory_dream_changes_*_idx: keep; no production WHERE uses those columns alone
    // - llm_requests: no new composite index without EXPLAIN proof (covered by existing AI stats fixture)
    let workspace = tempfile::tempdir().expect("workspace");
    let database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");
    drop(database);

    let connection =
        Connection::open(workspace_database_path(workspace.path())).expect("open database");
    connection.execute_batch("BEGIN;").expect("begin");
    {
        let mut insert = connection
            .prepare(
                "INSERT INTO chats (id, title, created_at, updated_at, archived_at, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, NULL, '{}')",
            )
            .expect("prepare chats");
        for index in 0..3_000 {
            insert
                .execute(params![
                    format!("chat-{index}"),
                    format!("Title seed {index} alpha"),
                    format!("2026-06-01T{:02}:00:00.000Z", index % 24),
                    format!("2026-06-02T{:02}:00:00.000Z", index % 24),
                ])
                .expect("chat insert");
        }
    }
    {
        let mut insert = connection
            .prepare(
                "INSERT INTO memory_dream_jobs
                    (id, scope, workspace_id, trigger_type, mode, status, input_summary_json, created_at)
                 VALUES (?1, 'workspace', 'ws-1', 'manual', 'deterministic_only', 'completed', '{}', ?2)",
            )
            .expect("prepare jobs");
        for index in 0..50 {
            insert
                .execute(params![
                    format!("job-{index}"),
                    format!("2026-07-01T00:{:02}:00.000Z", index % 60)
                ])
                .expect("job insert");
        }
    }
    {
        let mut insert = connection
            .prepare(
                "INSERT INTO memory_dream_changes
                    (id, job_id, operation, target_fact_ids_json, new_fact_id, before_json,
                     after_json, reason, confidence, risk_level, status, evidence_json,
                     error_message, created_at, applied_at)
                 VALUES (?1, ?2, 'update', ?3, ?4, NULL, NULL, 'reason', 0.5, 'low', 'proposed', '[]',
                         NULL, ?5, NULL)",
            )
            .expect("prepare changes");
        for index in 0..2_000 {
            let job_id = format!("job-{}", index % 50);
            insert
                .execute(params![
                    format!("change-{index}"),
                    job_id,
                    format!(r#"[\"fact-{}\"]"#, index % 100),
                    if index % 3 == 0 {
                        Some(format!("new-fact-{index}"))
                    } else {
                        None
                    },
                    format!("2026-07-02T00:{:02}:00.000Z", index % 60),
                ])
                .expect("change insert");
        }
    }
    connection.execute_batch("COMMIT;").expect("commit");

    // Title leading-wildcard: document that chats_title_nocase_idx is not usable here.
    let title_plan = explain_query_plan(
        &connection,
        "SELECT id FROM chats
         WHERE COALESCE(json_extract(metadata_json, '$.kind'), '') != 'memory_dream'
           AND title LIKE '%alpha%' ESCAPE '\\' COLLATE NOCASE
         ORDER BY updated_at DESC, created_at DESC, id DESC
         LIMIT 20",
    );
    let title_analysis = analyze_query_plan(&title_plan);
    assert!(
        !title_analysis.details.is_empty(),
        "title plan should exist for evidence"
    );
    // Decision: keep chats_title_nocase_idx for non-leading-wildcard / future use; do not DROP in migration 38.
    assert!(
        named_index_exists(&connection, "chats_title_nocase_idx"),
        "title nocase index must remain"
    );
    let _leading_wildcard_may_scan = plan_has_unconstrained_scan_on(&title_plan, "chats");

    // Production dream_changes_for_job: job_id (+ optional status), not target_fact_ids / new_fact_id.
    let dream_changes_plan = explain_query_plan(
        &connection,
        "SELECT id, job_id, operation, target_fact_ids_json, new_fact_id, before_json,
                after_json, reason, confidence, risk_level, status, evidence_json,
                error_message, created_at, applied_at
         FROM memory_dream_changes
         WHERE job_id = 'job-1'
           AND ('proposed' IS NULL OR status = 'proposed')
         ORDER BY created_at ASC, id ASC
         LIMIT 50",
    );
    assert_plan_uses_index(&dream_changes_plan, "memory_dream_changes_job_status_idx");
    // No production query filters by target_fact_ids_json or new_fact_id alone → do not DROP on intuition.
    assert!(named_index_exists(
        &connection,
        "memory_dream_changes_target_fact_ids_idx"
    ));
    assert!(named_index_exists(
        &connection,
        "memory_dream_changes_new_fact_idx"
    ));
    let target_only_plan = explain_query_plan(
        &connection,
        r#"SELECT id FROM memory_dream_changes WHERE target_fact_ids_json = '["fact-1"]' LIMIT 10"#,
    );
    let new_fact_plan = explain_query_plan(
        &connection,
        "SELECT id FROM memory_dream_changes WHERE new_fact_id = 'new-fact-0' LIMIT 10",
    );
    // Evidence only: whether planner can use those indexes when queried (not a production path).
    let _target_idx_may_be_used = plan_uses_index(
        &target_only_plan,
        "memory_dream_changes_target_fact_ids_idx",
    );
    let _new_fact_idx_may_be_used =
        plan_uses_index(&new_fact_plan, "memory_dream_changes_new_fact_idx");

    // AI Statistics: existing llm_request_audit_query_plans_cover_rows_count_summary_and_breakdown
    // already records that started_at / request_kind indexes suffice; no composite index added here.
}

#[test]
fn workspace_pragma_optimize_is_throttled_and_non_fatal_path_safe() {
    let workspace = tempfile::tempdir().expect("workspace");
    let mut database =
        WorkspaceDatabase::open_or_create_ungated(workspace.path()).expect("workspace database");

    assert!(
        database
            .maybe_run_pragma_optimize(true)
            .expect("force optimize"),
        "forced optimize should run"
    );
    assert!(
        !database
            .maybe_run_pragma_optimize(false)
            .expect("throttled optimize"),
        "second optimize within interval should no-op"
    );
    let last_at = database
        .workspace_metadata("sqlite_pragma_optimize_last_at")
        .expect("metadata")
        .expect("last_at stored");
    assert!(
        last_at.contains('T'),
        "expected RFC3339 last_at, got {last_at}"
    );

    // Force still allowed for tests/maintenance escape hatch.
    assert!(
        database
            .maybe_run_pragma_optimize(true)
            .expect("force again")
    );
}

#[test]
fn global_memory_pragma_optimize_is_throttled_process_local() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("memory.sqlite");
    let mut database = MemoryDatabase::open_or_create_global_at(&path).expect("global memory");
    assert!(
        database
            .maybe_run_pragma_optimize(true)
            .expect("force optimize")
    );
    assert!(
        !database
            .maybe_run_pragma_optimize(false)
            .expect("throttled"),
        "process-local throttle should suppress immediate re-run"
    );
}
