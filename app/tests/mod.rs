use super::*;
use std::collections::BTreeSet;

use axum::{
    Json,
    body::to_bytes,
    extract::{Path as AxumPath, Query, State},
    response::IntoResponse,
};
use foco_agent::{
    ToolResource, ToolResourceAccess, ToolResourceLock, context_compression_trigger_tokens,
};
use foco_providers::OPENAI_CHAT_KIND;
use foco_store::{
    config::{DEFAULT_WORKSPACE_ID, DEFAULT_WORKSPACE_NAME, PromptSettings, WebSearchSettings},
    memory::{
        MemoryDreamJobStatus, MemoryDreamRunMode, MemoryDreamScope, MemoryDreamTriggerType,
        MemoryExtractionJobStatus, MemoryFactRecord, MemoryKind, NewMemoryDreamJob,
        NewMemoryExtractionJob, NewMemoryFact, NewMemorySource, UpdateMemoryFact,
    },
    workspace::{
        LlmRequestAuditFilters, NewRunEvent, NewScheduledTask, NewScheduledTaskRun,
        NewTerminalSession, WorkspaceDatabaseSpaceStats,
    },
};
use foco_tools::{
    GRAPH_EXPLORE_TOOL, GRAPH_FIND_SYMBOLS_TOOL, READ_FILE_TOOL, SEARCH_TEXT_TOOL,
    ToolCancellationToken, WEB_FETCH_TOOL, WEB_SEARCH_TOOL,
};
use serde_json::json;

use crate::http::{
    memory::{
        MemoryDreamChangesQuery, MemoryDreamJobsQuery, MemoryDreamRunRequest, memory_dream_changes,
        memory_dream_job, memory_dream_jobs, memory_extraction_job_summaries, run_memory_dream,
    },
    settings::{
        associate_provider_with_local_models, can_save_new_provider_after_model_list_error,
        filter_provider_model_ids,
    },
    terminal::create_terminal_session,
    workspaces::add_workspace,
};
use crate::memory_runtime::scheduler::{
    dispatch_auto_memory_dreams_at, memory_dream_interval_due, reconcile_memory_dream_runs,
};
use crate::memory_runtime::{
    MemoryExtractionEvidenceCandidate, MemoryExtractionTask, MemorySearchToolInput,
    MemoryWriteToolInput, execute_memory_search_tool, execute_memory_write_tool,
    llm_memory_retrieval_candidates, memory_extraction_existing_memory_candidates,
    memory_extraction_provider_request, memory_extraction_target_status, memory_prompt_search,
    memory_prompt_search_terms, memory_retrieval_query_text, neutral_messages_from_record,
    parse_memory_extraction_output, resolve_prompt_context_memory, should_queue_memory_extraction,
    store_extracted_memory_facts, validate_extracted_memory_facts,
};
use crate::prompt::{
    compress_all_runtime_tool_state, compress_runtime_tool_state_if_needed, context_message_groups,
    context_token_breakdown,
};
use crate::runtime::{
    QuestionItem, QuestionItemAnswer, QuestionOption, ToolResourceLockOwner, execute_tool,
    wait_for_tool_resource_lock,
};
use crate::scheduled_tasks::scheduler::ScheduledTaskScheduler;

fn test_neutral_tool_call(call_id: &str, name: &str, arguments: Value) -> NeutralToolCall {
    NeutralToolCall {
        call_id: call_id.to_string(),
        name: name.to_string(),
        arguments,
        thought_signatures: None,
    }
}

fn test_provider_kind() -> foco_providers::ProviderKind {
    foco_providers::parse_provider_kind(foco_providers::OPENAI_RESPONSES_KIND)
        .expect("responses provider kind")
}

fn insert_waiting_coordinator_task(
    database: &mut WorkspaceDatabase,
    chat_id: &str,
    user_message_id: &str,
    suffix: &str,
) -> foco_agent::AgentTaskId {
    let team_id = foco_agent::AgentTeamId::new(format!("agent-team-{suffix}")).expect("team id");
    let instance_id =
        foco_agent::AgentInstanceId::new(format!("agent-instance-{suffix}")).expect("instance id");
    let task_id = foco_agent::AgentTaskId::new(format!("agent-task-{suffix}")).expect("task id");
    let attempt_id =
        foco_agent::AgentAttemptId::new(format!("agent-attempt-{suffix}")).expect("attempt id");
    let definition = AgentDefinitionSettings {
        id: AgentDefinitionId::new(format!("agent-definition-{suffix}")).expect("definition id"),
        revision: 1,
        name: "Waiting coordinator".to_string(),
        description: String::new(),
        provider_id: "provider".to_string(),
        model_id: "model".to_string(),
        model_options: AgentModelOptions::default(),
        system_prompt: "Coordinate.".to_string(),
        allowed_tools: Vec::new(),
        max_instances: 1,
        allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
        permissions: AgentPermissions::default(),
    };
    database
        .create_agent_team(foco_store::workspace::NewAgentTeam {
            id: &team_id,
            chat_id,
            coordinator_instance_id: &instance_id,
            coordinator_definition: &definition,
            max_concurrent_runs: 1,
        })
        .expect("team create");
    let input_json = serde_json::to_string(&json!({
        "queuedUserMessageId": user_message_id,
        "message": "Wait for child task.",
        "attachments": [],
        "skillIds": [],
        "collaborationToolsEnabled": true,
    }))
    .expect("task input json");
    database
        .enqueue_agent_task(foco_store::workspace::NewAgentTask {
            id: &task_id,
            team_id: &team_id,
            owner_instance_id: &instance_id,
            origin_instance_id: None,
            parent_task_id: None,
            input_json: &input_json,
        })
        .expect("enqueue waiting task");
    database
        .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
        .expect("claim waiting task")
        .expect("waiting task claimed");
    database
        .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
            team_id: &team_id,
            task_id: &task_id,
            expected_status: foco_agent::AgentTaskStatus::Running,
            transition: foco_agent::AgentTaskTransition::Wait,
            result_json: Some(r#"{"control":{"kind":"agent_wait_tasks"}}"#),
            error_json: None,
            interruption_reason: None,
        })
        .expect("suspend waiting task");
    task_id
}

struct FixtureAgentRunTask {
    events: Vec<ChatSseEvent>,
}

impl AgentRunTask<ChatSseEvent> for FixtureAgentRunTask {
    fn run(
        self,
        _context: AgentRunContext,
        _input: AgentRunInput,
        events: AgentRunEventEmitter<ChatSseEvent>,
    ) -> AgentRunFuture {
        Box::pin(async move {
            for event in self.events {
                let kind = agent_run_event_kind(&event);
                events.emit(kind, event).expect("fixture event");
            }
            AgentRunOutcome::Completed {
                text: "done".to_string(),
                reasoning: None,
                usage: None,
            }
        })
    }
}

#[tokio::test]
async fn agent_run_executor_preserves_single_agent_sse_sequence() {
    let fixture = vec![
        ChatSseEvent::TextDelta {
            assistant_message_id: "assistant-1".to_string(),
            delta: "done".to_string(),
        },
        ChatSseEvent::ToolCall {
            assistant_message_id: "assistant-1".to_string(),
            tool_call: ChatToolCallSummary {
                id: "call-1".to_string(),
                name: "read_file".to_string(),
                status: "running".to_string(),
                input: json!({ "path": "README.md" }),
                output: None,
                is_error: false,
            },
        },
        ChatSseEvent::ToolResult {
            assistant_message_id: "assistant-1".to_string(),
            tool_call_id: "call-1".to_string(),
            output: json!({ "content": "ok" }),
            is_error: false,
        },
        ChatSseEvent::Complete {
            chat_id: "chat-1".to_string(),
            assistant_message_id: "assistant-1".to_string(),
            text: "done".to_string(),
            reasoning: None,
            usage: None,
            stop_reason: Some("stop".to_string()),
            metrics: ChatReplyMetrics {
                model_id: "model-1".to_string(),
                provider_id: "provider-1".to_string(),
                total_latency_ms: Some(1),
                first_token_latency_ms: Some(1),
                output_tokens: Some(1),
            },
            memories_used: Vec::new(),
        },
    ];
    let expected = fixture
        .iter()
        .map(|event| serde_json::to_string(event).expect("fixture event json"))
        .collect::<Vec<_>>();
    let mut actual = Vec::new();

    let outcome = AgentRunExecutor
        .execute(
            AgentRunContext {
                chat_id: "chat-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                workspace_path: PathBuf::from("workspace"),
                provider_id: "provider-1".to_string(),
                model_id: "model-1".to_string(),
                associations: AgentRunAssociations::default(),
                definition_snapshot: json!({}),
                cancellation: foco_agent::AgentRunCancellation::default(),
            },
            AgentRunInput {
                messages: Vec::new(),
                current_task: None,
                unread_messages: Vec::new(),
                recovery: None,
            },
            FixtureAgentRunTask { events: fixture },
            |event: AgentRunEvent<ChatSseEvent>| {
                assert_eq!(event.associations, AgentRunAssociations::default());
                actual.push(serde_json::to_string(&event.payload).expect("run event json"));
                Ok(())
            },
        )
        .await;

    assert!(matches!(outcome, AgentRunOutcome::Completed { .. }));
    assert_eq!(actual, expected);
}

fn test_file_resource_lock(path: &str, access: ToolResourceAccess) -> ToolResourceLock {
    ToolResourceLock {
        resource: ToolResource::File(path.to_string()),
        access,
    }
}

fn test_workspace_mutation_lock() -> ToolResourceLock {
    ToolResourceLock {
        resource: ToolResource::WorkspaceMutationLease,
        access: ToolResourceAccess::Exclusive,
    }
}

fn test_prepared_chat_context(
    workspace_dir: PathBuf,
    messages: Vec<NeutralChatMessage>,
    message_source_sequences: Vec<Option<i64>>,
    message_context_sources: Vec<PromptContextSource>,
    available_message_tokens: u64,
) -> PreparedChatContext {
    let (_app_shutdown_tx, app_shutdown_rx) = watch::channel(false);
    let mcp_registry = Arc::new(McpRegistry::default());

    PreparedChatContext {
        workspace_id: "workspace-1".to_string(),
        workspace_path: workspace_dir.clone(),
        tool_workspace_path: workspace_dir.clone(),
        memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        provider_id: "openai-responses".to_string(),
        model_id: "gpt-5.4".to_string(),
        user_message_id: "user-1".to_string(),
        queued_user_message_id: None,
        assistant_message_id: "assistant-1".to_string(),
        llm_request_id: "request-1".to_string(),
        assistant_sequence: 1,
        agent_associations: AgentRunAssociations::default(),
        agent_definition_snapshot: None,
        agent_task_input: None,
        agent_unread_messages: Vec::new(),
        agent_allowed_tools: None,
        agent_tool_context: None,
        agent_primary_chat_output: true,
        session_upload_paths: None,
        provider_config: ProviderConnectionConfig {
            kind: test_provider_kind(),
            base_url: None,
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        },
        provider_request: NeutralChatRequest {
            model_id: "gpt-5.4".to_string(),
            messages,
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: Some(16),
            prompt_cache_key: None,
            prompt_cache_retention: None,
        },
        hook_runtime: HookRuntime::new(mcp_registry.clone()),
        global_hooks: HookConfig::default(),
        mcp_registry,
        question_registry: QuestionRegistry::default(),
        tool_resource_locks: ToolResourceLockRegistry::default(),
        app_shutdown_rx,
        context_budget: foco_agent::ContextBudget {
            context_window: 1_000,
            max_output_tokens: 16,
            system_prompt_tokens: 0,
            tool_schema_tokens: 0,
            safety_tokens: 0,
            available_message_tokens,
        },
        global_config: GlobalConfig::first_run(workspace_dir),
        memory_settings: MemorySettings::default(),
        memories_used: Vec::new(),
        memory_target_status: MemoryStatus::Pending,
        request_body_json: "{}".to_string(),
        captured_llm_requests: Vec::new(),
        compression_snapshots: Vec::new(),
        message_source_sequences,
        message_context_sources,
        active_tool_start_index: 1,
        next_runtime_tool_batch_index: 0,
        hook_context_messages: Vec::new(),
        hook_notifications: Vec::new(),
        code_change_baseline: SessionCodeChangeBaselineState::Unavailable {
            reason: "test baseline unavailable".to_string(),
        },
        code_change_stats: CodeChangeStats::default(),
        pending_memory_retrieval: None,
    }
}

#[test]
fn prompt_cache_key_includes_agent_layers_and_resolved_memory() {
    let mut request = NeutralChatRequest {
        model_id: "gpt-5.4".to_string(),
        messages: vec![
            neutral_text_message(NeutralChatRole::System, "base".to_string()),
            neutral_text_message(NeutralChatRole::System, "agent definition".to_string()),
            neutral_text_message(NeutralChatRole::System, "team protocol".to_string()),
            neutral_text_message(NeutralChatRole::User, "user turn".to_string()),
            neutral_text_message(NeutralChatRole::User, "resolved memory A".to_string()),
        ],
        tools: Vec::new(),
        thinking_level: None,
        max_output_tokens: Some(16),
        prompt_cache_key: None,
        prompt_cache_retention: None,
    };
    let source_sequences = vec![None, None, None, Some(7), Some(7)];
    let context_sources = vec![
        PromptContextSource::ReservedPrompt,
        PromptContextSource::AgentDefinition,
        PromptContextSource::AgentTeamProtocol,
        PromptContextSource::CurrentUser { sequence: 7 },
        PromptContextSource::TurnMemory { sequence: 7 },
    ];

    let base_key = prompt_cache_key(
        "workspace-1",
        "chat-1",
        "provider-1",
        "model-1",
        &request,
        &source_sequences,
        &context_sources,
    )
    .expect("base cache key");

    request.messages[1].content = "agent definition changed".to_string();
    let definition_key = prompt_cache_key(
        "workspace-1",
        "chat-1",
        "provider-1",
        "model-1",
        &request,
        &source_sequences,
        &context_sources,
    )
    .expect("definition cache key");
    assert_ne!(base_key, definition_key);

    request.messages[1].content = "agent definition".to_string();
    request.messages[4].content = "resolved memory B".to_string();
    let memory_key = prompt_cache_key(
        "workspace-1",
        "chat-1",
        "provider-1",
        "model-1",
        &request,
        &source_sequences,
        &context_sources,
    )
    .expect("memory cache key");
    assert_ne!(base_key, memory_key);
}

#[tokio::test]
async fn tool_resource_registry_blocks_same_file_read_write() {
    let registry = ToolResourceLockRegistry::default();
    let read_lease = registry
        .acquire(
            "workspace-1",
            vec![test_file_resource_lock(
                "src/main.rs",
                ToolResourceAccess::Read,
            )],
        )
        .await;
    let waiting_registry = registry.clone();
    let waiter = tokio::spawn(async move {
        let _write_lease = waiting_registry
            .acquire(
                "workspace-1",
                vec![test_file_resource_lock(
                    "src/main.rs",
                    ToolResourceAccess::Write,
                )],
            )
            .await;
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!waiter.is_finished());

    drop(read_lease);
    tokio::time::timeout(Duration::from_secs(1), waiter)
        .await
        .expect("same-file write lock should be released")
        .expect("same-file write waiter should not panic");
}

#[tokio::test]
async fn tool_resource_registry_serializes_workspace_mutation_lease() {
    let registry = ToolResourceLockRegistry::default();
    let mutation_lease = registry
        .acquire("workspace-1", vec![test_workspace_mutation_lock()])
        .await;
    let waiting_registry = registry.clone();
    let waiter = tokio::spawn(async move {
        let _second_lease = waiting_registry
            .acquire("workspace-1", vec![test_workspace_mutation_lock()])
            .await;
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!waiter.is_finished());

    drop(mutation_lease);
    tokio::time::timeout(Duration::from_secs(1), waiter)
        .await
        .expect("workspace mutation lease should be released")
        .expect("mutation lease waiter should not panic");
}

#[tokio::test]
async fn tool_resource_registry_allows_read_during_workspace_mutation_lease() {
    let registry = ToolResourceLockRegistry::default();
    let _mutation_lease = registry
        .acquire("workspace-1", vec![test_workspace_mutation_lock()])
        .await;

    let _read_lease = tokio::time::timeout(
        Duration::from_secs(1),
        registry.acquire(
            "workspace-1",
            vec![test_file_resource_lock(
                "src/main.rs",
                ToolResourceAccess::Read,
            )],
        ),
    )
    .await
    .expect("read-only file lock should not wait on workspace mutation lease");
}

#[tokio::test]
async fn tool_resource_registry_workspace_exclusive_blocks_file_access() {
    let registry = ToolResourceLockRegistry::default();
    let workspace_lease = registry
        .acquire(
            "workspace-1",
            vec![ToolResourceLock {
                resource: ToolResource::WorkspaceFiles,
                access: ToolResourceAccess::Exclusive,
            }],
        )
        .await;
    let waiting_registry = registry.clone();
    let waiter = tokio::spawn(async move {
        let _read_lease = waiting_registry
            .acquire(
                "workspace-1",
                vec![test_file_resource_lock(
                    "src/main.rs",
                    ToolResourceAccess::Read,
                )],
            )
            .await;
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!waiter.is_finished());

    drop(workspace_lease);
    tokio::time::timeout(Duration::from_secs(1), waiter)
        .await
        .expect("workspace exclusive lock should be released")
        .expect("file read waiter should not panic");
}

#[tokio::test]
async fn tool_resource_registry_scopes_workspace_exclusive_by_workspace() {
    let registry = ToolResourceLockRegistry::default();
    let _workspace_lease = registry
        .acquire(
            "workspace-1",
            vec![ToolResourceLock {
                resource: ToolResource::WorkspaceFiles,
                access: ToolResourceAccess::Exclusive,
            }],
        )
        .await;

    let _read_lease = tokio::time::timeout(
        Duration::from_secs(1),
        registry.acquire(
            "workspace-2",
            vec![test_file_resource_lock(
                "src/main.rs",
                ToolResourceAccess::Read,
            )],
        ),
    )
    .await
    .expect("different workspace file read should not wait on workspace-1 command");
}

#[tokio::test]
async fn tool_resource_registry_keeps_memory_locks_global() {
    let registry = ToolResourceLockRegistry::default();
    let memory_lease = registry
        .acquire(
            "workspace-1",
            vec![ToolResourceLock {
                resource: ToolResource::Memory("global".to_string()),
                access: ToolResourceAccess::Write,
            }],
        )
        .await;
    let waiting_registry = registry.clone();
    let waiter = tokio::spawn(async move {
        let _read_lease = waiting_registry
            .acquire(
                "workspace-2",
                vec![ToolResourceLock {
                    resource: ToolResource::Memory("global".to_string()),
                    access: ToolResourceAccess::Read,
                }],
            )
            .await;
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!waiter.is_finished());

    drop(memory_lease);
    tokio::time::timeout(Duration::from_secs(1), waiter)
        .await
        .expect("global memory write lock should be released")
        .expect("global memory read waiter should not panic");
}

#[tokio::test]
async fn tool_resource_lock_wait_respects_tool_timeout() {
    let registry = ToolResourceLockRegistry::default();
    let _workspace_lease = registry
        .acquire_with_owner(
            "workspace-1",
            vec![ToolResourceLock {
                resource: ToolResource::WorkspaceFiles,
                access: ToolResourceAccess::Exclusive,
            }],
            ToolResourceLockOwner {
                instance_id: Some("agent-instance-1".to_string()),
                task_id: Some("agent-task-1".to_string()),
                tool_call_id: Some("call-blocker".to_string()),
                tool_name: Some("write_file".to_string()),
            },
        )
        .await;
    let started = Instant::now();
    let result = wait_for_tool_resource_lock(
        &registry,
        "workspace-1",
        vec![test_file_resource_lock(
            "src/main.rs",
            ToolResourceAccess::Read,
        )],
        "read_file",
        Some(10),
        Some(started + Duration::from_millis(10)),
        ToolCancellationToken::default(),
        ToolResourceLockOwner {
            tool_call_id: Some("call-waiter".to_string()),
            tool_name: Some("read_file".to_string()),
            ..ToolResourceLockOwner::default()
        },
    )
    .await;

    let error = match result {
        Ok(_) => panic!("lock wait should time out"),
        Err(error) => error,
    };
    assert!(error.contains("timed out waiting for resource lock"));
    assert!(error.contains("call-blocker"));
    assert!(error.contains("agent-instance-1"));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn tool_resource_lease_releases_after_task_panic() {
    let registry = ToolResourceLockRegistry::default();
    let panic_registry = registry.clone();
    let handle = tokio::spawn(async move {
        let _lease = panic_registry
            .acquire("workspace-1", vec![test_workspace_mutation_lock()])
            .await;
        panic!("fixture panic after acquiring mutation lease");
    });
    assert!(
        handle
            .await
            .expect_err("fixture task should panic")
            .is_panic()
    );

    let _lease = tokio::time::timeout(
        Duration::from_secs(1),
        registry.acquire("workspace-1", vec![test_workspace_mutation_lock()]),
    )
    .await
    .expect("mutation lease should be released when panicking task drops it");
}

#[tokio::test]
async fn execute_tool_reports_timeout_while_waiting_for_resource_lock() {
    let workspace = tempfile::tempdir().expect("workspace");
    fs::write(workspace.path().join("note.txt"), "hello").expect("write note");
    let registry = ToolResourceLockRegistry::default();
    let _workspace_lease = registry
        .acquire(
            "workspace-1",
            vec![ToolResourceLock {
                resource: ToolResource::WorkspaceFiles,
                access: ToolResourceAccess::Exclusive,
            }],
        )
        .await;
    let mcp_registry = Arc::new(McpRegistry::default());

    let outcome = execute_tool(
        mcp_registry.clone(),
        HookRuntime::new(mcp_registry),
        &HookConfig::default(),
        true,
        &ProviderConnectionConfig {
            kind: test_provider_kind(),
            base_url: None,
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        },
        &WebSearchSettings::default(),
        QuestionRegistry::default(),
        mpsc::unbounded_channel().0,
        MemoryToolContext {
            enabled: false,
            workspace_path: workspace.path().to_path_buf(),
            global_memory_database_file: workspace.path().join("memory.sqlite"),
            chat_id: "chat-1".to_string(),
            run_id: "run-1".to_string(),
            tool_call_id: "call-1".to_string(),
            target_status: MemoryStatus::Pending,
            memory_settings: MemorySettings::default(),
        },
        None,
        registry,
        ToolCancellationToken::default(),
        mpsc::unbounded_channel().0,
        "assistant-1",
        "workspace-1",
        workspace.path(),
        workspace.path(),
        "chat-1",
        "run-1",
        "model-1",
        "provider-1",
        2,
        "call-1",
        "read_file",
        json!({
            "path": "note.txt",
            "startLine": null,
            "endLine": null,
            "timeoutMs": 10
        }),
    )
    .await;

    assert!(outcome.execution.is_error);
    assert!(
        outcome.execution.output["error"]
            .as_str()
            .expect("error")
            .contains("timed out waiting for resource lock")
    );
}

fn captured_test_llm_request(
    request_id: &str,
    assistant_message_id: &str,
    output_tokens: i64,
    total_latency_ms: i64,
) -> CapturedLlmRequest {
    CapturedLlmRequest {
        id: request_id.to_string(),
        request_started_at: "2026-06-06T09:00:00Z".to_string(),
        request_body_json: "{}".to_string(),
        events: vec![CapturedAuditEvent {
            event_at: "2026-06-06T09:00:00Z".to_string(),
            event_type: "start".to_string(),
            normalized_event_json: json!({
                "type": "start",
                "chatId": "chat-1",
                "userMessageId": "user-1",
                "assistantMessageId": assistant_message_id,
                "llmRequestId": request_id,
            })
            .to_string(),
        }],
        outcome: ChatAuditOutcome {
            first_token_at: Some("2026-06-06T09:00:00Z".to_string()),
            completed_at: "2026-06-06T09:00:01Z".to_string(),
            first_token_latency_ms: Some(100),
            total_latency_ms,
            input_tokens: Some(100),
            output_tokens: Some(output_tokens),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            status_code: Some(200),
            final_state: "succeeded",
            response_body_json: Some("{}".to_string()),
        },
    }
}

#[test]
fn password_hash_verifies_only_matching_password() {
    let password_hash = hash_password("secret").expect("password hash");

    assert!(password_hash.starts_with("sha256:"));
    assert!(verify_password("secret", &password_hash));
    assert!(!verify_password(" secret ", &password_hash));
    assert!(!verify_password("wrong", &password_hash));
}

#[test]
fn browser_open_addr_uses_loopback_for_unspecified_listen_hosts() {
    assert_eq!(
        browser_addr_for_listen_addr(SocketAddr::from(([0, 0, 0, 0], 3210))).to_string(),
        "127.0.0.1:3210"
    );
    assert_eq!(
        browser_addr_for_listen_addr(SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], 3210)))
            .to_string(),
        "[::1]:3210"
    );
    assert_eq!(
        browser_addr_for_listen_addr(SocketAddr::from(([192, 168, 1, 10], 3210))).to_string(),
        "192.168.1.10:3210"
    );
}

#[test]
fn startup_browser_open_waits_for_bound_listener_and_uses_browser_url() {
    let addr = SocketAddr::from(([0, 0, 0, 0], 3210));
    let mut opened_urls = Vec::new();

    assert!(!open_foco_ui_if_listener_bound(false, addr, |url| {
        opened_urls.push(url.to_string());
    }));
    assert!(opened_urls.is_empty());

    assert!(open_foco_ui_if_listener_bound(true, addr, |url| {
        opened_urls.push(url.to_string());
    }));
    assert_eq!(opened_urls, vec!["http://127.0.0.1:3210"]);
}

#[test]
fn tray_menu_labels_follow_app_language() {
    assert_eq!(
        tray_menu_labels("en").expect("English tray labels"),
        TrayMenuLabels {
            open: "Open Foco",
            quit: "Quit Foco",
        }
    );
    assert_eq!(
        tray_menu_labels("zh-CN").expect("Chinese tray labels"),
        TrayMenuLabels {
            open: "打开 Foco",
            quit: "退出 Foco",
        }
    );

    let error = tray_menu_labels("fr").expect_err("unsupported language should fail");

    assert!(error.contains("app language 'fr' is unsupported"));
}

#[test]
fn background_code_graph_initialization_indexes_workspace_and_keeps_watcher() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-background-graph-test"));
    remove_dir_if_exists(&workspace_dir);
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::write(
        workspace_dir.join("lib.rs"),
        "pub fn helper() -> i32 { 1 }\n",
    )
    .expect("workspace source write");
    let workspaces = vec![WorkspaceConfig {
        id: "workspace-1".to_string(),
        name: "Workspace 1".to_string(),
        path: workspace_dir.clone(),
        pinned: false,
        terminal_shell: DEFAULT_TERMINAL_SHELL.to_string(),
        common_commands: Vec::new(),
    }];
    let indexes = Arc::new(Mutex::new(CodeGraphIndexState::default()));

    let thread = spawn_code_graph_index_initialization(workspaces, indexes.clone())
        .expect("spawn code graph initialization");
    thread.join().expect("code graph initialization thread");

    assert_eq!(
        indexes
            .lock()
            .expect("code graph index lock")
            .watcher_count(),
        1,
        "watcher must be retained after background indexing"
    );
    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let context = database.code_graph_context().expect("code graph context");
    assert_eq!(context.indexed_files, 1);
    drop(database);
    indexes
        .lock()
        .expect("code graph index lock")
        .watchers
        .clear();
    remove_dir_if_exists(&workspace_dir);
}

#[test]
fn startup_code_graph_initialization_selects_recently_active_workspaces() {
    let active_dir = env::temp_dir().join(unique_id("foco-active-graph-test"));
    let inactive_dir = env::temp_dir().join(unique_id("foco-inactive-graph-test"));
    remove_dir_if_exists(&active_dir);
    remove_dir_if_exists(&inactive_dir);
    fs::create_dir_all(&active_dir).expect("active workspace directory");
    fs::create_dir_all(&inactive_dir).expect("inactive workspace directory");

    let active_workspace = WorkspaceConfig {
        id: "active-workspace".to_string(),
        name: "Active Workspace".to_string(),
        path: active_dir.clone(),
        pinned: false,
        terminal_shell: DEFAULT_TERMINAL_SHELL.to_string(),
        common_commands: Vec::new(),
    };
    let inactive_workspace = WorkspaceConfig {
        id: "inactive-workspace".to_string(),
        name: "Inactive Workspace".to_string(),
        path: inactive_dir.clone(),
        pinned: false,
        terminal_shell: DEFAULT_TERMINAL_SHELL.to_string(),
        common_commands: Vec::new(),
    };

    let mut active_database = WorkspaceDatabase::open_or_create(&active_dir).expect("active db");
    active_database
        .insert_chat("chat-active", "Active chat")
        .expect("active chat");
    active_database
        .insert_message(NewMessage {
            id: "msg-active-user",
            chat_id: "chat-active",
            role: "user",
            content: "Index this workspace",
            sequence: 0,
            metadata_json: None,
        })
        .expect("active user message");
    drop(active_database);
    WorkspaceDatabase::open_or_create(&inactive_dir).expect("inactive db");

    let workspaces = vec![active_workspace.clone(), inactive_workspace];
    let selected = recently_active_code_graph_workspaces(&workspaces)
        .expect("recently active code graph workspaces");

    assert_eq!(selected, vec![active_workspace]);

    remove_dir_if_exists(&active_dir);
    remove_dir_if_exists(&inactive_dir);
}

#[test]
fn lazy_code_graph_initialization_indexes_workspace_once() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-lazy-graph-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-lazy-graph-profile"));
    remove_dir_if_exists(&workspace_dir);
    remove_dir_if_exists(&profile_dir);
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");
    fs::write(
        workspace_dir.join("lib.rs"),
        "pub fn lazy_helper() -> i32 { 1 }\n",
    )
    .expect("workspace source write");

    let config = GlobalConfig::first_run(workspace_dir.clone());
    let workspace = config.workspaces[0].clone();
    let state = test_app_state(config, profile_dir.clone());

    spawn_code_graph_workspace_initialization_if_needed(&state, &workspace);
    for _ in 0..50 {
        if state
            .code_graph_indexes
            .lock()
            .expect("code graph index lock")
            .watcher_count()
            == 1
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert_eq!(
        state
            .code_graph_indexes
            .lock()
            .expect("code graph index lock")
            .watcher_count(),
        1,
        "lazy initialization should retain one watcher"
    );
    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let context = database.code_graph_context().expect("code graph context");
    assert_eq!(context.indexed_files, 1);
    drop(database);

    spawn_code_graph_workspace_initialization_if_needed(&state, &workspace);
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert_eq!(
        state
            .code_graph_indexes
            .lock()
            .expect("code graph index lock")
            .watcher_count(),
        1,
        "lazy initialization should not register duplicate watchers"
    );

    state
        .code_graph_indexes
        .lock()
        .expect("code graph index lock")
        .watchers
        .clear();
    remove_dir_if_exists(&workspace_dir);
    remove_dir_if_exists(&profile_dir);
}

#[test]
fn repeated_tool_call_detector_rejects_third_identical_batch() {
    let mut detector = RepeatedToolCallDetector::default();

    assert!(
        detector
            .check(&[test_neutral_tool_call(
                "call-1",
                "read_file",
                json!({ "path": "README.md" }),
            )])
            .is_ok()
    );
    assert!(
        detector
            .check(&[test_neutral_tool_call(
                "call-2",
                "read_file",
                json!({ "path": "README.md" }),
            )])
            .is_ok()
    );

    let error = detector
        .check(&[test_neutral_tool_call(
            "call-3",
            "read_file",
            json!({ "path": "README.md" }),
        )])
        .expect_err("third identical batch should be rejected");

    assert!(error.contains("agent run repeated the same tool call batch 3 times"));
    assert!(error.contains("read_file"));
}

#[test]
fn repeated_tool_call_detector_resets_when_arguments_change() {
    let mut detector = RepeatedToolCallDetector::default();

    assert!(
        detector
            .check(&[test_neutral_tool_call(
                "call-1",
                "read_file",
                json!({ "path": "README.md" }),
            )])
            .is_ok()
    );
    assert!(
        detector
            .check(&[test_neutral_tool_call(
                "call-2",
                "read_file",
                json!({ "path": "README.md" }),
            )])
            .is_ok()
    );
    assert!(
        detector
            .check(&[test_neutral_tool_call(
                "call-3",
                "read_file",
                json!({ "path": "CHANGELOG.md" }),
            )])
            .is_ok()
    );
    assert!(
        detector
            .check(&[test_neutral_tool_call(
                "call-4",
                "read_file",
                json!({ "path": "CHANGELOG.md" }),
            )])
            .is_ok()
    );
}

#[test]
fn read_only_tool_progress_detector_warns_once_and_continues_varied_exploration() {
    let mut detector = ReadOnlyToolProgressDetector::default();

    for index in 1..READ_ONLY_TOOL_BATCH_WARNING_THRESHOLD {
        assert_eq!(
            detector.check(&[test_neutral_tool_call(
                &format!("call-{index}"),
                READ_FILE_TOOL,
                json!({ "path": format!("file-{index}.rs") }),
            )]),
            ReadOnlyToolProgressAction::Continue
        );
    }

    let warning = detector.check(&[test_neutral_tool_call(
        "call-warning",
        SEARCH_TEXT_TOOL,
        json!({ "query": "needle", "path": "." }),
    )]);
    assert!(matches!(warning, ReadOnlyToolProgressAction::Warn(_)));

    for index in
        (READ_ONLY_TOOL_BATCH_WARNING_THRESHOLD + 1)..(READ_ONLY_TOOL_BATCH_WARNING_THRESHOLD + 8)
    {
        assert_eq!(
            detector.check(&[test_neutral_tool_call(
                &format!("call-after-warning-{index}"),
                GRAPH_EXPLORE_TOOL,
                json!({ "query": format!("symbol_{index}") }),
            )]),
            ReadOnlyToolProgressAction::Continue
        );
    }

    assert_eq!(
        detector.check(&[test_neutral_tool_call(
            "call-after-limit-removed",
            GRAPH_FIND_SYMBOLS_TOOL,
            json!({ "query": "still_searching" }),
        )]),
        ReadOnlyToolProgressAction::Continue
    );
}

#[test]
fn read_only_tool_progress_detector_resets_on_non_read_only_tool() {
    let mut detector = ReadOnlyToolProgressDetector::default();

    for index in 1..READ_ONLY_TOOL_BATCH_WARNING_THRESHOLD {
        assert_eq!(
            detector.check(&[test_neutral_tool_call(
                &format!("call-{index}"),
                READ_FILE_TOOL,
                json!({ "path": format!("file-{index}.rs") }),
            )]),
            ReadOnlyToolProgressAction::Continue
        );
    }

    assert_eq!(
        detector.check(&[test_neutral_tool_call(
            "call-edit",
            EDIT_FILE_TOOL,
            json!({ "path": "file.rs" }),
        )]),
        ReadOnlyToolProgressAction::Continue
    );

    assert_eq!(
        detector.check(&[test_neutral_tool_call(
            "call-read-again",
            READ_FILE_TOOL,
            json!({ "path": "file.rs" }),
        )]),
        ReadOnlyToolProgressAction::Continue
    );
}

#[test]
fn normalize_web_server_settings_preserves_updates_and_clears_password_hash() {
    let current = WebServerSettings {
        listen_host: "127.0.0.1".to_string(),
        listen_port: 3210,
        password_hash: Some(hash_password("old-password").expect("old password hash")),
    };

    let preserved = normalize_web_server_settings(
        &current,
        &ManualGeneralSettingsRequest {
            api_audit: None,
            auto_start_enabled: None,
            clear_password: None,
            default_team_mode_enabled: None,
            hook_audit_enabled: None,
            language: "en".to_string(),
            llm_request_retry_count: None,
            theme: "light".to_string(),
            listen_host: "0.0.0.0".to_string(),
            listen_port: 3211,
            password: None,
        },
    )
    .expect("preserve password hash");
    assert_eq!(preserved.password_hash, current.password_hash);

    let updated = normalize_web_server_settings(
        &current,
        &ManualGeneralSettingsRequest {
            api_audit: None,
            auto_start_enabled: None,
            clear_password: None,
            default_team_mode_enabled: None,
            hook_audit_enabled: None,
            language: "en".to_string(),
            llm_request_retry_count: None,
            theme: "light".to_string(),
            listen_host: "127.0.0.1".to_string(),
            listen_port: 3210,
            password: Some("new-password".to_string()),
        },
    )
    .expect("update password hash");
    assert!(verify_password(
        "new-password",
        updated
            .password_hash
            .as_deref()
            .expect("updated password hash")
    ));

    let cleared = normalize_web_server_settings(
        &current,
        &ManualGeneralSettingsRequest {
            api_audit: None,
            auto_start_enabled: None,
            clear_password: Some(true),
            default_team_mode_enabled: None,
            hook_audit_enabled: None,
            language: "en".to_string(),
            llm_request_retry_count: None,
            theme: "light".to_string(),
            listen_host: "127.0.0.1".to_string(),
            listen_port: 3210,
            password: Some("ignored".to_string()),
        },
    )
    .expect("clear password hash");
    assert!(cleared.password_hash.is_none());
}

#[test]
fn normalize_api_proxy_settings_preserves_updates_and_disables_proxy() {
    let current = ApiProxySettings {
        enabled: true,
        proxy_type: "http".to_string(),
        url: "http://127.0.0.1:7890/".to_string(),
    };

    let preserved =
        normalize_api_proxy_settings(&current, None).expect("preserve current proxy settings");
    assert_eq!(preserved, current);

    let updated = normalize_api_proxy_settings(
        &current,
        Some(&ManualApiProxySettingsRequest {
            enabled: true,
            proxy_type: "socks".to_string(),
            url: "127.0.0.1:7891".to_string(),
        }),
    )
    .expect("normalize updated proxy");
    assert!(updated.enabled);
    assert_eq!(updated.proxy_type, "socks");
    assert_eq!(updated.url, "socks5h://127.0.0.1:7891");

    let disabled = normalize_api_proxy_settings(
        &current,
        Some(&ManualApiProxySettingsRequest {
            enabled: false,
            proxy_type: "http".to_string(),
            url: "".to_string(),
        }),
    )
    .expect("disable proxy");
    assert!(!disabled.enabled);
    assert!(disabled.url.is_empty());

    let web_search_settings = WebSearchSettings {
        api_proxy: current.clone(),
        ..WebSearchSettings::default()
    };
    let preserved_web_search_proxy =
        normalize_api_proxy_settings(&web_search_settings.api_proxy, None)
            .expect("preserve current web search proxy settings");
    assert_eq!(preserved_web_search_proxy, current);
}

#[test]
fn select_ripgrep_asset_skips_checksum_assets() {
    let target = ripgrep_asset_target().expect("supported test platform");
    let suffix = if cfg!(windows) { ".zip" } else { ".tar.gz" };
    let selected = select_ripgrep_asset(vec![
        GithubReleaseAsset {
            name: format!("ripgrep-1.0.0-{target}{suffix}.sha256"),
            browser_download_url: "https://example.test/checksum".to_string(),
        },
        GithubReleaseAsset {
            name: format!("ripgrep-1.0.0-{target}{suffix}"),
            browser_download_url: "https://example.test/archive".to_string(),
        },
    ])
    .expect("ripgrep asset");

    assert_eq!(
        selected.browser_download_url,
        "https://example.test/archive"
    );
}

#[test]
fn detect_ripgrep_prefers_foco_bin() {
    let root = tempfile::tempdir().expect("foco root");
    let install_dir = ripgrep_install_dir(root.path());
    fs::create_dir_all(&install_dir).expect("install dir");
    let fake_rg = install_dir.join(ripgrep_executable_name());

    #[cfg(windows)]
    {
        let Some(system_rg) = find_system_ripgrep() else {
            return;
        };
        fs::copy(system_rg, &fake_rg).expect("fake rg");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(
            &fake_rg,
            "#!/bin/sh\n[ \"$1\" = \"--version\" ] && exit 0\nexit 1\n",
        )
        .expect("fake rg");
        let mut permissions = fs::metadata(&fake_rg).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_rg, permissions).expect("permissions");
    }

    let status = detect_ripgrep(root.path());

    assert!(status.available);
    assert_eq!(status.path.as_deref(), Some(fake_rg.as_path()));
    assert_eq!(status.install_dir, install_dir);
}

#[test]
fn parse_skill_markdown_requires_description() {
    let error = parse_skill_markdown(
        Path::new("SKILL.md"),
        "---
name: gitmemo
---

# GitMemo
",
    )
    .expect_err("missing description should fail");

    assert!(error.contains("description"));
}

#[test]
fn enabled_skill_frontmatter_messages_list_enabled_skill_frontmatter() {
    let profile_dir = env::temp_dir().join(unique_id("foco-skill-frontmatter-profile-test"));
    let workspace_dir = env::temp_dir().join(unique_id("foco-skill-frontmatter-workspace-test"));
    let skill_dir = profile_dir.join(".agents").join("skills").join("gitmemo");
    let skill_file = skill_dir.join("SKILL.md");

    fs::create_dir_all(&skill_dir).expect("skill test directory");
    fs::write(
        &skill_file,
        "---
name: gitmemo
description: Project memory.
---

# GitMemo

Search memory before repo work.
",
    )
    .expect("skill file write");

    let config = GlobalConfig::first_run(workspace_dir);

    let messages = enabled_skill_frontmatter_messages(&profile_dir, &config, DEFAULT_WORKSPACE_ID)
        .expect("enabled skill frontmatter messages");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, NeutralChatRole::User);
    assert!(messages[0].content.contains(ENABLED_SKILLS_MESSAGE_PREFIX));
    assert!(
        messages[0]
            .content
            .contains(&skill_file.display().to_string())
    );
    assert!(messages[0].content.contains("name: gitmemo"));
    assert!(messages[0].content.contains("description: Project memory."));
    assert!(!messages[0].content.contains("Search memory"));

    fs::remove_dir_all(profile_dir).expect("remove skill test profile");
}

#[test]
fn enabled_skill_frontmatter_messages_skip_disabled_skills() {
    let profile_dir = env::temp_dir().join(unique_id("foco-disabled-skill-profile-test"));
    let workspace_dir = env::temp_dir().join(unique_id("foco-disabled-skill-workspace-test"));
    let skill_dir = profile_dir.join(".agents").join("skills").join("gitmemo");
    let skill_file = skill_dir.join("SKILL.md");

    fs::create_dir_all(&skill_dir).expect("skill test directory");
    fs::write(
        &skill_file,
        "---
name: gitmemo
description: Project memory.
---

# GitMemo
",
    )
    .expect("skill file write");

    let mut config = GlobalConfig::first_run(workspace_dir);
    config.skills.disabled.push("global:gitmemo".to_string());

    let messages = enabled_skill_frontmatter_messages(&profile_dir, &config, DEFAULT_WORKSPACE_ID)
        .expect("enabled skill frontmatter messages");

    assert!(messages.is_empty());

    fs::remove_dir_all(profile_dir).expect("remove skill test profile");
}

#[test]
fn invalid_frontmatter_skill_is_detected_as_disabled_only() {
    let profile_dir = env::temp_dir().join(unique_id("foco-invalid-skill-profile-test"));
    let workspace_dir = env::temp_dir().join(unique_id("foco-invalid-skill-workspace-test"));
    let skill_dir = profile_dir.join(".agents").join("skills").join("broken");
    let skill_file = skill_dir.join("SKILL.md");

    fs::create_dir_all(&skill_dir).expect("skill test directory");
    fs::write(
        &skill_file,
        "---
name: broken
description:
---

# Broken
",
    )
    .expect("skill file write");

    let config = GlobalConfig::first_run(workspace_dir);
    let discovery = discover_skills(&profile_dir, &config.workspaces);

    assert_eq!(discovery.skills.len(), 1);
    assert_eq!(discovery.skills[0].key, "global:broken");
    assert_eq!(discovery.required_disabled, vec!["global:broken"]);
    assert_eq!(discovery.errors.len(), 1);
    assert!(discovery.errors[0].message.contains("description"));

    let summary = skills_settings_summary(&config, &profile_dir);
    assert_eq!(summary.detected.len(), 1);
    assert!(!summary.detected[0].enabled);
    assert!(!summary.detected[0].can_enable);
    assert!(
        summary.detected[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("frontmatter is invalid"))
    );

    fs::remove_dir_all(profile_dir).expect("remove skill test profile");
}

#[test]
fn invalid_skill_does_not_block_enabled_skill_frontmatter_injection() {
    let profile_dir = env::temp_dir().join(unique_id("foco-mixed-skill-profile-test"));
    let workspace_dir = env::temp_dir().join(unique_id("foco-mixed-skill-workspace-test"));
    let good_skill_dir = profile_dir.join(".agents").join("skills").join("gitmemo");
    let bad_skill_dir = profile_dir.join(".agents").join("skills").join("broken");

    fs::create_dir_all(&good_skill_dir).expect("good skill directory");
    fs::create_dir_all(&bad_skill_dir).expect("bad skill directory");
    fs::write(
        good_skill_dir.join("SKILL.md"),
        "---
name: gitmemo
description: Project memory.
---

# GitMemo
",
    )
    .expect("good skill file write");
    fs::write(
        bad_skill_dir.join("SKILL.md"),
        "---
name: broken
description:
---

# Broken
",
    )
    .expect("bad skill file write");

    let config = GlobalConfig::first_run(workspace_dir);
    let messages = enabled_skill_frontmatter_messages(&profile_dir, &config, DEFAULT_WORKSPACE_ID)
        .expect("enabled skill frontmatter messages");

    assert_eq!(messages.len(), 1);
    assert!(messages[0].content.contains("name: gitmemo"));
    assert!(!messages[0].content.contains("name: broken"));

    fs::remove_dir_all(profile_dir).expect("remove skill test profile");
}

#[test]
fn selected_invalid_skill_reports_disabled_before_frontmatter_error() {
    let profile_dir = env::temp_dir().join(unique_id("foco-selected-invalid-skill-profile"));
    let workspace_dir = env::temp_dir().join(unique_id("foco-selected-invalid-skill-workspace"));
    let skill_dir = profile_dir.join(".agents").join("skills").join("broken");

    fs::create_dir_all(&skill_dir).expect("skill test directory");
    fs::write(
        skill_dir.join("SKILL.md"),
        "---
name: broken
description:
---

# Broken
",
    )
    .expect("skill file write");

    let config = GlobalConfig::first_run(workspace_dir);
    let error = message_with_selected_skills(
        &profile_dir,
        &config,
        DEFAULT_WORKSPACE_ID,
        Some(vec!["global:broken".to_string()]),
        "Hello",
    )
    .expect_err("invalid selected skill should fail as disabled");

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert_eq!(error.message, "selected skill 'global:broken' is disabled");

    fs::remove_dir_all(profile_dir).expect("remove skill test profile");
}

#[test]
fn question_registry_rejects_invalid_answer_without_consuming_question() {
    let registry = QuestionRegistry::default();
    let registration = registry
        .register(QuestionRequest {
            id: "question-1".to_string(),
            tool_call_id: "tool-call-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            chat_id: "chat-1".to_string(),
            questions: vec![
                QuestionItem {
                    id: "question-1-item-1".to_string(),
                    question: "Pick a mode.".to_string(),
                    options: vec![QuestionOption {
                        label: "Fast".to_string(),
                        value: "fast".to_string(),
                        description: None,
                    }],
                    allow_free_text: false,
                },
                QuestionItem {
                    id: "question-1-item-2".to_string(),
                    question: "Name the target.".to_string(),
                    options: Vec::new(),
                    allow_free_text: true,
                },
            ],
        })
        .expect("question registration");

    let error = registry
        .answer(
            "question-1",
            QuestionAnswer {
                answers: vec![QuestionItemAnswer {
                    id: "question-1-item-1".to_string(),
                    answer: "manual".to_string(),
                    selected_option_value: None,
                }],
            },
        )
        .expect_err("manual answer should be rejected");

    assert!(error.message.contains("requires answers for all"));
    assert!(
        registry
            .is_pending("question-1")
            .expect("question registry pending check")
    );

    registry
        .answer(
            "question-1",
            QuestionAnswer {
                answers: vec![
                    QuestionItemAnswer {
                        id: "question-1-item-1".to_string(),
                        answer: "fast".to_string(),
                        selected_option_value: Some("fast".to_string()),
                    },
                    QuestionItemAnswer {
                        id: "question-1-item-2".to_string(),
                        answer: "prod".to_string(),
                        selected_option_value: None,
                    },
                ],
            },
        )
        .expect("valid selected option answer");

    let received_answer = registration
        .answer_rx
        .blocking_recv()
        .expect("question answer");
    assert_eq!(
        received_answer.answers[0].selected_option_value.as_deref(),
        Some("fast")
    );
    assert_eq!(received_answer.answers[1].answer, "prod");
    assert!(
        !registry
            .is_pending("question-1")
            .expect("question registry pending check")
    );
}

#[test]
fn new_provider_is_associated_with_matching_local_models() {
    let mut models = vec![
        test_model_settings("matched-without-provider"),
        ModelSettings {
            provider_ids: vec!["existing".to_string()],
            active_provider_id: Some("existing".to_string()),
            ..test_model_settings("matched-with-provider")
        },
        test_model_settings("not-returned"),
    ];

    associate_provider_with_local_models(
        &mut models,
        "new-provider",
        &[
            "matched-without-provider".to_string(),
            "matched-with-provider".to_string(),
        ],
    );

    assert_eq!(models[0].provider_ids, vec!["new-provider"]);
    assert_eq!(
        models[0].active_provider_id.as_deref(),
        Some("new-provider")
    );
    assert_eq!(models[1].provider_ids, vec!["existing", "new-provider"]);
    assert_eq!(models[1].active_provider_id.as_deref(), Some("existing"));
    assert!(models[2].provider_ids.is_empty());
    assert_eq!(models[2].active_provider_id, None);
}

#[test]
fn provider_model_refresh_removes_stale_local_model_associations() {
    let mut models = vec![
        ModelSettings {
            provider_ids: vec!["refreshed-provider".to_string(), "fallback".to_string()],
            active_provider_id: Some("refreshed-provider".to_string()),
            ..test_model_settings("removed-model")
        },
        ModelSettings {
            provider_ids: vec!["fallback".to_string()],
            active_provider_id: Some("fallback".to_string()),
            ..test_model_settings("added-model")
        },
    ];

    associate_provider_with_local_models(
        &mut models,
        "refreshed-provider",
        &["added-model".to_string()],
    );

    assert_eq!(models[0].provider_ids, vec!["fallback"]);
    assert_eq!(models[0].active_provider_id.as_deref(), Some("fallback"));
    assert_eq!(
        models[1].provider_ids,
        vec!["fallback", "refreshed-provider"]
    );
    assert_eq!(models[1].active_provider_id.as_deref(), Some("fallback"));
}

#[test]
fn provider_model_sync_filter_regex_limits_association_changes() {
    let provider = ProviderSettings {
        id: "filtered-provider".to_string(),
        name: "Filtered".to_string(),
        kind: "openai-chat".to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: true,
        model_sync_filter_regex: Some("^gpt-4".to_string()),
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    };
    let mut models = vec![
        test_model_settings("gpt-4.1"),
        ModelSettings {
            provider_ids: vec!["filtered-provider".to_string(), "fallback".to_string()],
            active_provider_id: Some("filtered-provider".to_string()),
            ..test_model_settings("text-embedding-3-large")
        },
    ];
    let provider_models = filter_provider_model_ids(
        &provider,
        vec!["gpt-4.1".to_string(), "text-embedding-3-large".to_string()],
    )
    .expect("filter provider models");

    associate_provider_with_local_models(&mut models, &provider.id, &provider_models);

    assert_eq!(provider_models, vec!["gpt-4.1"]);
    assert_eq!(models[0].provider_ids, vec!["filtered-provider"]);
    assert_eq!(models[1].provider_ids, vec!["fallback"]);
    assert_eq!(models[1].active_provider_id.as_deref(), Some("fallback"));
}

#[test]
fn new_provider_save_ignores_model_list_connection_failures() {
    let missing_models = foco_providers::ProviderConfigError::Connection {
        message: "not found".to_string(),
        status_code: Some(404),
    };
    let unauthorized = foco_providers::ProviderConfigError::Connection {
        message: "unauthorized".to_string(),
        status_code: Some(401),
    };

    assert!(can_save_new_provider_after_model_list_error(
        &missing_models
    ));
    assert!(can_save_new_provider_after_model_list_error(&unauthorized));
    assert!(!can_save_new_provider_after_model_list_error(
        &foco_providers::ProviderConfigError::MissingApiKey,
    ));
}

fn test_agent_definition_input() -> AgentDefinitionInput {
    AgentDefinitionInput {
        name: "Coordinator".to_string(),
        description: "Coordinates work.".to_string(),
        provider_id: "provider".to_string(),
        model_id: "model".to_string(),
        model_options: AgentModelOptions {
            thinking_level: Some("high".to_string()),
            max_output_tokens: Some(800),
        },
        system_prompt: "Coordinate the team.".to_string(),
        allowed_tools: vec![READ_FILE_TOOL.to_string()],
        max_instances: 2,
        allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
        permissions: AgentPermissions::default(),
    }
}

#[tokio::test]
async fn agent_definition_api_manages_revision_validates_tools_and_hides_secrets() {
    let profile = tempfile::tempdir().expect("temp profile");
    let workspace_dir = profile.path().join("workspace");
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(profile.path().join(".foco")).expect("config directory");
    let mut config = prompt_test_config(workspace_dir);
    config.providers[0].api_key = Some("secret-agent-api-key".to_string());
    let state = test_app_state(config, profile.path().to_path_buf());

    let created = crate::http::settings::create_agent_definition(
        State(state.clone()),
        Json(CreateAgentDefinitionRequest {
            definition: test_agent_definition_input(),
        }),
    )
    .await
    .expect("create agent definition")
    .0;
    assert_eq!(created.agent_definitions.len(), 1);
    let definition_id = created.agent_definitions[0].id.clone();
    assert_eq!(
        created.agent_definitions[0].revision,
        AGENT_DEFINITION_INITIAL_REVISION
    );
    let response_json = serde_json::to_string(&created).expect("serialize response");
    assert!(!response_json.contains("secret-agent-api-key"));
    assert!(!response_json.contains("apiKey"));

    let listed = crate::http::settings::agent_definitions(State(state.clone()))
        .await
        .expect("list agent definitions")
        .0;
    let default_definition_id =
        AgentDefinitionId::new("agent-definition-default").expect("default definition id");
    assert!(
        listed
            .agent_definitions
            .iter()
            .any(|definition| definition.id == default_definition_id)
    );
    assert!(
        listed
            .agent_definitions
            .iter()
            .any(|definition| definition.id == definition_id)
    );

    let mut invalid_tool_input = test_agent_definition_input();
    invalid_tool_input.allowed_tools = vec!["not_a_runtime_tool".to_string()];
    let invalid_tool = match crate::http::settings::update_agent_definition(
        State(state.clone()),
        Json(UpdateAgentDefinitionRequest {
            id: definition_id.clone(),
            definition: invalid_tool_input,
        }),
    )
    .await
    {
        Err(error) => error,
        Ok(_) => panic!("unknown tool should fail"),
    };
    assert_eq!(invalid_tool.status, StatusCode::BAD_REQUEST);
    assert!(invalid_tool.message.contains("unknown runtime tool"));

    let mut updated_input = test_agent_definition_input();
    updated_input.description = "Updated coordinator.".to_string();
    let updated = crate::http::settings::update_agent_definition(
        State(state.clone()),
        Json(UpdateAgentDefinitionRequest {
            id: definition_id.clone(),
            definition: updated_input,
        }),
    )
    .await
    .expect("update agent definition")
    .0;
    let updated_definition = updated
        .agent_definitions
        .iter()
        .find(|definition| definition.id == definition_id)
        .expect("updated definition");
    assert_eq!(updated_definition.revision, 2);
    assert_eq!(updated_definition.description, "Updated coordinator.");

    let provider_error = match crate::http::settings::delete_provider(
        State(state.clone()),
        Json(DeleteSettingsItemRequest {
            id: "provider".to_string(),
        }),
    )
    .await
    {
        Err(error) => error,
        Ok(_) => panic!("referenced provider deletion should fail"),
    };
    assert!(
        provider_error
            .message
            .contains("referenced by agent definition")
    );
    let model_error = match crate::http::settings::delete_model(
        State(state.clone()),
        Json(DeleteSettingsItemRequest {
            id: "model".to_string(),
        }),
    )
    .await
    {
        Err(error) => error,
        Ok(_) => panic!("referenced model deletion should fail"),
    };
    assert!(
        model_error
            .message
            .contains("referenced by agent definition")
    );

    let deleted = crate::http::settings::delete_agent_definition(
        State(state),
        Json(DeleteAgentDefinitionRequest { id: definition_id }),
    )
    .await
    .expect("delete agent definition")
    .0;
    assert_eq!(deleted.agent_definitions.len(), 1);
    assert_eq!(deleted.agent_definitions[0].id, default_definition_id);
}

#[tokio::test]
async fn agent_definitions_api_creates_default_agent_when_empty() {
    let profile = tempfile::tempdir().expect("temp profile");
    let workspace_dir = profile.path().join("workspace");
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(profile.path().join(".foco")).expect("config directory");
    let state = test_app_state(
        prompt_test_config(workspace_dir),
        profile.path().to_path_buf(),
    );

    let listed = crate::http::settings::agent_definitions(State(state.clone()))
        .await
        .expect("list agent definitions")
        .0;
    let default_definition_id =
        AgentDefinitionId::new("agent-definition-default").expect("default definition id");

    assert_eq!(listed.agent_definitions.len(), 1);
    let default_definition = &listed.agent_definitions[0];
    assert_eq!(default_definition.id, default_definition_id);
    assert_eq!(default_definition.name, "Default agent");
    assert_eq!(default_definition.provider_id, "provider");
    assert_eq!(default_definition.model_id, "model");
    assert!(default_definition.permissions.can_create_instances);
    assert!(default_definition.permissions.can_delegate);
    assert!(
        default_definition
            .permissions
            .allowed_agent_definition_ids
            .is_empty()
    );

    let listed_again = crate::http::settings::agent_definitions(State(state))
        .await
        .expect("list agent definitions again")
        .0;
    assert_eq!(listed_again.agent_definitions.len(), 1);
}

#[test]
fn agent_definition_tool_validation_rejects_unknown_ids() {
    let definition = AgentDefinitionSettings {
        id: AgentDefinitionId::new("agent-definition-test").expect("definition id"),
        revision: 1,
        name: "Test".to_string(),
        description: String::new(),
        provider_id: "provider".to_string(),
        model_id: "model".to_string(),
        model_options: AgentModelOptions::default(),
        system_prompt: "Test.".to_string(),
        allowed_tools: vec!["missing_tool".to_string()],
        max_instances: 1,
        allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
        permissions: AgentPermissions::default(),
    };
    let error = validate_agent_definition_tool_references(None, &[definition], &HashSet::new())
        .expect_err("unknown tool");
    assert!(error.to_string().contains("unknown runtime tool"));
}

#[test]
fn reorder_models_requires_complete_unique_existing_ids() {
    let mut models = vec![
        test_model_settings("low"),
        test_model_settings("high"),
        test_model_settings("medium"),
    ];

    reorder_models(
        &mut models,
        vec!["high".to_string(), "medium".to_string(), "low".to_string()],
    )
    .expect("reordered models");
    assert_eq!(model_ids(&models), vec!["high", "medium", "low"]);

    let duplicate_error = reorder_models(
        &mut models,
        vec!["high".to_string(), "high".to_string(), "low".to_string()],
    )
    .expect_err("duplicate model ids should fail");
    assert_eq!(duplicate_error.status, StatusCode::BAD_REQUEST);
    assert!(duplicate_error.message.contains("duplicate"));
    assert_eq!(model_ids(&models), vec!["high", "medium", "low"]);

    let missing_error = reorder_models(
        &mut models,
        vec!["high".to_string(), "missing".to_string(), "low".to_string()],
    )
    .expect_err("unknown model ids should fail");
    assert_eq!(missing_error.status, StatusCode::BAD_REQUEST);
    assert!(missing_error.message.contains("not found"));
    assert_eq!(model_ids(&models), vec!["high", "medium", "low"]);
}

#[test]
fn reorder_workspaces_requires_complete_unique_existing_ids() {
    let mut workspaces = vec![
        test_workspace_config("default"),
        test_workspace_config("side"),
        test_workspace_config("archive"),
    ];

    reorder_workspaces(
        &mut workspaces,
        vec![
            "side".to_string(),
            "archive".to_string(),
            "default".to_string(),
        ],
    )
    .expect("reordered workspaces");
    assert_eq!(
        workspace_ids(&workspaces),
        vec!["side", "archive", "default"]
    );

    let duplicate_error = reorder_workspaces(
        &mut workspaces,
        vec![
            "side".to_string(),
            "side".to_string(),
            "default".to_string(),
        ],
    )
    .expect_err("duplicate workspace ids should fail");
    assert_eq!(duplicate_error.status, StatusCode::BAD_REQUEST);
    assert!(duplicate_error.message.contains("duplicate"));
    assert_eq!(
        workspace_ids(&workspaces),
        vec!["side", "archive", "default"]
    );

    let missing_error = reorder_workspaces(
        &mut workspaces,
        vec![
            "side".to_string(),
            "missing".to_string(),
            "default".to_string(),
        ],
    )
    .expect_err("unknown workspace ids should fail");
    assert_eq!(missing_error.status, StatusCode::BAD_REQUEST);
    assert!(missing_error.message.contains("not found"));
    assert_eq!(
        workspace_ids(&workspaces),
        vec!["side", "archive", "default"]
    );
}

#[test]
fn group_pinned_workspaces_keeps_group_order() {
    let mut workspaces = vec![
        test_workspace_config("first"),
        test_workspace_config("second"),
        test_workspace_config("third"),
        test_workspace_config("fourth"),
    ];
    workspaces[2].pinned = true;
    workspaces[0].pinned = true;

    group_pinned_workspaces(&mut workspaces);

    assert_eq!(
        workspace_ids(&workspaces),
        vec!["first", "third", "second", "fourth"]
    );
}

#[test]
fn normalize_windows_verbatim_path_removes_prefixes() {
    assert_eq!(
        normalize_windows_verbatim_path(PathBuf::from(r"\\?\C:\Users\fonla\Repo")),
        PathBuf::from(r"C:\Users\fonla\Repo")
    );
    assert_eq!(
        normalize_windows_verbatim_path(PathBuf::from(r"\\?\UNC\server\share\Repo")),
        PathBuf::from(r"\\server\share\Repo")
    );
}

#[test]
fn skills_settings_summary_strips_windows_verbatim_directory_prefixes() {
    let user_profile_dir = PathBuf::from(r"\\?\C:\Users\fonla");
    let mut config = GlobalConfig::first_run(PathBuf::from(r"\\?\C:\Users\fonla\.foco\workspace"));
    config.workspaces[0].path = PathBuf::from(r"\\?\C:\Users\fonla\Projects\Foco");

    let summary = skills_settings_summary(&config, &user_profile_dir);

    assert!(
        summary
            .directories
            .iter()
            .all(|directory| !directory.starts_with(r"\\?\"))
    );
}

#[test]
fn discover_skills_ignores_missing_directories() {
    let profile_dir = env::temp_dir().join(unique_id("foco-missing-skill-profile-test"));
    let workspace_dir = env::temp_dir().join(unique_id("foco-missing-skill-test"));

    fs::create_dir_all(&profile_dir).expect("profile directory");
    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let workspaces = vec![WorkspaceConfig {
        id: "default".to_string(),
        name: "Default".to_string(),
        path: workspace_dir.clone(),
        pinned: false,
        terminal_shell: DEFAULT_TERMINAL_SHELL.to_string(),
        common_commands: Vec::new(),
    }];
    let discovery = discover_skills(&profile_dir, &workspaces);

    assert!(discovery.errors.is_empty());
    assert!(discovery.skills.is_empty());

    remove_dir_if_exists(&profile_dir);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn prompt_messages_read_workspace_and_configured_prompt_files() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-agents-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-profile-agents-test"));
    let codex_dir = profile_dir.join(".codex");
    let configured_prompt_file = codex_dir.join("AGENTS.md");

    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&codex_dir).expect("codex directory");
    fs::write(workspace_dir.join("AGENTS.md"), "Workspace instructions.\n")
        .expect("workspace AGENTS write");
    fs::write(&configured_prompt_file, "Configured prompt instructions.\n")
        .expect("configured prompt write");

    let agents_messages = agents_prompt_messages(&workspace_dir).expect("agents messages");
    let prompt_messages = configured_prompt_messages(&PromptSettings {
        system_prompts: Vec::new(),
        system_prompt: None,
        files: vec![configured_prompt_file],
        extra_text: "Extra prompt instructions.\n".to_string(),
    })
    .expect("configured prompt messages");
    let extra_prompt_message = configured_extra_prompt_message(&PromptSettings {
        system_prompts: Vec::new(),
        system_prompt: None,
        files: Vec::new(),
        extra_text: "Extra prompt instructions.\n".to_string(),
    })
    .expect("extra prompt message");

    assert_eq!(agents_messages.len(), 1);
    assert_eq!(agents_messages[0].role, NeutralChatRole::User);
    assert!(agents_messages[0].content.contains(AGENTS_MESSAGE_PREFIX));
    assert!(
        agents_messages[0]
            .content
            .contains("Workspace instructions.")
    );
    assert_eq!(prompt_messages.len(), 1);
    assert_eq!(prompt_messages[0].role, NeutralChatRole::User);
    assert!(
        prompt_messages[0]
            .content
            .contains(PROMPT_FILE_MESSAGE_PREFIX)
    );
    assert!(
        prompt_messages[0]
            .content
            .contains("Configured prompt instructions.")
    );
    assert_eq!(extra_prompt_message.role, NeutralChatRole::System);
    assert!(
        extra_prompt_message
            .content
            .contains(EXTRA_PROMPT_MESSAGE_PREFIX)
    );
    assert!(
        extra_prompt_message
            .content
            .contains("Extra prompt instructions.")
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[test]
fn append_tool_state_messages_interleaves_each_tool_call_and_result() {
    let mut messages = Vec::new();
    let mut message_source_sequences = Vec::new();
    let mut message_context_sources = Vec::new();
    let mut next_runtime_tool_batch_index = 0;
    let tool_calls = vec![
        NeutralToolCall {
            call_id: "call-1".to_string(),
            name: "find_files".to_string(),
            arguments: json!({ "path": "." }),
            thought_signatures: None,
        },
        NeutralToolCall {
            call_id: "call-2".to_string(),
            name: "read_file".to_string(),
            arguments: json!({ "path": "README.md" }),
            thought_signatures: None,
        },
    ];
    let tool_results = vec![
        ExecutedToolCall {
            id: "call-1".to_string(),
            name: "find_files".to_string(),
            input: json!({ "path": "." }),
            output: json!({ "entries": [] }),
            is_error: false,
            started_at: "2026-06-05T07:00:00Z".to_string(),
            completed_at: "2026-06-05T07:00:01Z".to_string(),
        },
        ExecutedToolCall {
            id: "call-2".to_string(),
            name: "read_file".to_string(),
            input: json!({ "path": "README.md" }),
            output: json!({ "content": "hello" }),
            is_error: false,
            started_at: "2026-06-05T07:00:01Z".to_string(),
            completed_at: "2026-06-05T07:00:02Z".to_string(),
        },
    ];

    append_tool_state_messages(
        &mut messages,
        &mut message_source_sequences,
        &mut message_context_sources,
        &mut next_runtime_tool_batch_index,
        tool_calls,
        &tool_results,
        "Checking files.".to_string(),
        Some("Need workspace evidence.".to_string()),
    );

    assert_eq!(
        messages
            .iter()
            .map(|message| &message.role)
            .collect::<Vec<_>>(),
        vec![
            &NeutralChatRole::Assistant,
            &NeutralChatRole::Tool,
            &NeutralChatRole::Assistant,
            &NeutralChatRole::Tool
        ]
    );
    assert_eq!(messages[0].tool_calls[0].call_id, "call-1");
    assert_eq!(messages[1].tool_call_id.as_deref(), Some("call-1"));
    assert_eq!(messages[2].tool_calls[0].call_id, "call-2");
    assert_eq!(messages[3].tool_call_id.as_deref(), Some("call-2"));
    assert_eq!(messages[0].content, "Checking files.");
    assert_eq!(
        messages[0].reasoning.as_deref(),
        Some("Need workspace evidence.")
    );
    assert!(messages[2].content.is_empty());
    assert_eq!(message_source_sequences, vec![None, None, None, None]);
    assert_eq!(
        message_context_sources,
        vec![
            PromptContextSource::RuntimeToolState { batch_index: 0 },
            PromptContextSource::RuntimeToolState { batch_index: 0 },
            PromptContextSource::RuntimeToolState { batch_index: 0 },
            PromptContextSource::RuntimeToolState { batch_index: 0 },
        ]
    );
    assert_eq!(next_runtime_tool_batch_index, 1);
}

#[test]
fn context_message_groups_do_not_double_count_reserved_prompt_tokens() {
    let messages = vec![
        neutral_text_message(NeutralChatRole::System, "system prompt".repeat(100)),
        neutral_text_message(NeutralChatRole::System, "tools prompt".repeat(100)),
        neutral_text_message(NeutralChatRole::User, "hello".to_string()),
    ];
    let message_source_sequences = vec![None, None, Some(0)];
    let message_context_sources = vec![
        PromptContextSource::ReservedPrompt,
        PromptContextSource::ReservedPrompt,
        PromptContextSource::CurrentUser { sequence: 0 },
    ];

    let groups = context_message_groups(
        &messages,
        &message_source_sequences,
        &message_context_sources,
        messages.len(),
    )
    .expect("message groups");
    let reserved_tokens = groups
        .iter()
        .filter(|group| group.source_bucket == PromptContextSourceBucket::ReservedPrompt)
        .map(|group| group.estimated_tokens)
        .sum::<u64>();
    let breakdown = context_token_breakdown(&groups);

    assert_eq!(reserved_tokens, 0);
    assert_eq!(
        breakdown
            .by_source
            .iter()
            .find(|entry| entry.source == PromptContextSourceBucket::ReservedPrompt)
            .expect("reserved prompt breakdown")
            .required_tokens,
        0
    );
}

#[test]
fn context_token_breakdown_handles_every_source_bucket() {
    // Guards against a repeat of the panic where a source bucket existed on the
    // enum but was missing from the SOURCES list inside context_token_breakdown.
    for bucket in [
        PromptContextSourceBucket::ReservedPrompt,
        PromptContextSourceBucket::StableInjection,
        PromptContextSourceBucket::TodoGraph,
        PromptContextSourceBucket::CompressionSnapshot,
        PromptContextSourceBucket::PersistedHistory,
        PromptContextSourceBucket::TurnMemory,
        PromptContextSourceBucket::CurrentUser,
        PromptContextSourceBucket::AssistantDraft,
        PromptContextSourceBucket::HookContext,
        PromptContextSourceBucket::Guidance,
        PromptContextSourceBucket::RuntimeGuard,
        PromptContextSourceBucket::RuntimeAssistant,
        PromptContextSourceBucket::RuntimeToolState,
        PromptContextSourceBucket::RuntimeToolStateSnapshot,
    ] {
        let groups = vec![ContextMessageGroup {
            message_indices: vec![0],
            estimated_tokens: 10,
            must_keep: true,
            source_bucket: bucket,
            runtime_tool_batch_index: None,
        }];
        let breakdown = context_token_breakdown(&groups);
        let entry = breakdown
            .by_source
            .iter()
            .find(|entry| entry.source == bucket)
            .unwrap_or_else(|| panic!("source bucket {:?} missing from breakdown", bucket));
        assert_eq!(entry.tokens, 10);
        assert_eq!(entry.required_tokens, 10);
    }
}

#[test]
fn compress_runtime_tool_state_keeps_recent_batches_verbatim() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-runtime-tool-compress-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut context = test_prepared_chat_context(
        workspace_dir.clone(),
        vec![neutral_text_message(
            NeutralChatRole::System,
            "system".to_string(),
        )],
        vec![None],
        vec![PromptContextSource::ReservedPrompt],
        80,
    );
    context.active_tool_start_index = context.provider_request.messages.len();

    for batch_index in 0..4 {
        let call_id = format!("call-{batch_index}");
        let tool_calls = vec![NeutralToolCall {
            call_id: call_id.clone(),
            name: "read_file".to_string(),
            arguments: json!({
                "path": "app/main.rs",
                "startLine": batch_index * 10 + 1,
                "endLine": batch_index * 10 + 10,
                "timeoutMs": 10000
            }),
            thought_signatures: None,
        }];
        let tool_results = vec![ExecutedToolCall {
            id: call_id,
            name: "read_file".to_string(),
            input: tool_calls[0].arguments.clone(),
            output: json!({
                "path": "app/main.rs",
                "bytes": 40_000,
                "content": "x".repeat(2_000),
                "timeoutMs": 10000
            }),
            is_error: false,
            started_at: "2026-06-13T09:00:00Z".to_string(),
            completed_at: "2026-06-13T09:00:01Z".to_string(),
        }];
        append_tool_state_messages(
            &mut context.provider_request.messages,
            &mut context.message_source_sequences,
            &mut context.message_context_sources,
            &mut context.next_runtime_tool_batch_index,
            tool_calls,
            &tool_results,
            String::new(),
            None,
        );
    }

    assert!(
        compress_runtime_tool_state_if_needed(&mut context, true).expect("runtime compression")
    );
    let snapshot_count = context
        .message_context_sources
        .iter()
        .filter(|source| matches!(source, PromptContextSource::RuntimeToolStateSnapshot))
        .count();
    let remaining_tool_messages = context
        .provider_request
        .messages
        .iter()
        .filter(|message| message.role == NeutralChatRole::Tool)
        .count();
    let remaining_batch_indices = context
        .message_context_sources
        .iter()
        .filter_map(|source| match source {
            PromptContextSource::RuntimeToolState { batch_index } => Some(*batch_index),
            _ => None,
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(snapshot_count, 1);
    assert_eq!(
        remaining_tool_messages,
        CONTEXT_COMPRESSION_PRESERVE_RECENT_TOOL_BATCHES
    );
    assert_eq!(remaining_batch_indices, BTreeSet::from([2, 3]));
    assert!(context.provider_request.messages.iter().any(|message| {
        message
            .content
            .contains("Runtime tool-state compression snapshot")
    }));

    for batch_index in 4..6 {
        let call_id = format!("call-{batch_index}");
        let tool_calls = vec![NeutralToolCall {
            call_id: call_id.clone(),
            name: "read_file".to_string(),
            arguments: json!({ "path": "app/main.rs", "timeoutMs": 10000 }),
            thought_signatures: None,
        }];
        let tool_results = vec![ExecutedToolCall {
            id: call_id,
            name: "read_file".to_string(),
            input: tool_calls[0].arguments.clone(),
            output: json!({
                "path": "app/main.rs",
                "bytes": 40_000,
                "content": "y".repeat(2_000),
                "timeoutMs": 10000
            }),
            is_error: false,
            started_at: "2026-06-13T09:00:00Z".to_string(),
            completed_at: "2026-06-13T09:00:01Z".to_string(),
        }];
        append_tool_state_messages(
            &mut context.provider_request.messages,
            &mut context.message_source_sequences,
            &mut context.message_context_sources,
            &mut context.next_runtime_tool_batch_index,
            tool_calls,
            &tool_results,
            String::new(),
            None,
        );
    }
    assert!(compress_runtime_tool_state_if_needed(&mut context, true).expect("second compression"));
    let snapshot_count = context
        .message_context_sources
        .iter()
        .filter(|source| matches!(source, PromptContextSource::RuntimeToolStateSnapshot))
        .count();
    assert_eq!(snapshot_count, 1);

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn compress_all_runtime_tool_state_removes_tool_protocol_messages() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-runtime-tool-clear-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut context = test_prepared_chat_context(
        workspace_dir.clone(),
        vec![neutral_text_message(
            NeutralChatRole::System,
            "system".to_string(),
        )],
        vec![None],
        vec![PromptContextSource::ReservedPrompt],
        80,
    );
    context.active_tool_start_index = context.provider_request.messages.len();

    for batch_index in 0..3 {
        let call_id = format!("call-{batch_index}");
        let tool_calls = vec![NeutralToolCall {
            call_id: call_id.clone(),
            name: "read_file".to_string(),
            arguments: json!({ "path": "app/main.rs", "timeoutMs": 10000 }),
            thought_signatures: None,
        }];
        let tool_results = vec![ExecutedToolCall {
            id: call_id,
            name: "read_file".to_string(),
            input: tool_calls[0].arguments.clone(),
            output: json!({ "content": "x".repeat(500), "timeoutMs": 10000 }),
            is_error: false,
            started_at: "2026-06-13T09:00:00Z".to_string(),
            completed_at: "2026-06-13T09:00:01Z".to_string(),
        }];
        append_tool_state_messages(
            &mut context.provider_request.messages,
            &mut context.message_source_sequences,
            &mut context.message_context_sources,
            &mut context.next_runtime_tool_batch_index,
            tool_calls,
            &tool_results,
            String::new(),
            None,
        );
    }

    assert!(compress_all_runtime_tool_state(&mut context).expect("runtime tool-state clear"));

    assert!(
        context
            .provider_request
            .messages
            .iter()
            .all(|message| message.tool_calls.is_empty())
    );
    assert!(
        context
            .provider_request
            .messages
            .iter()
            .all(|message| message.role != NeutralChatRole::Tool)
    );
    assert_eq!(
        context
            .message_context_sources
            .iter()
            .filter(|source| matches!(source, PromptContextSource::RuntimeToolState { .. }))
            .count(),
        0
    );
    assert_eq!(
        context
            .message_context_sources
            .iter()
            .filter(|source| matches!(source, PromptContextSource::RuntimeToolStateSnapshot))
            .count(),
        1
    );
    assert!(context.provider_request.messages.iter().any(|message| {
        message
            .content
            .contains("all prior in-progress tool calls/results")
    }));

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn recover_after_tool_round_cap_compresses_pending_tool_calls_once() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-tool-round-recover-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut context = test_prepared_chat_context(
        workspace_dir.clone(),
        vec![neutral_text_message(
            NeutralChatRole::User,
            "use tools".to_string(),
        )],
        vec![Some(0)],
        vec![PromptContextSource::CurrentUser { sequence: 0 }],
        200,
    );
    context.active_tool_start_index = context.provider_request.messages.len();

    let recovered = recover_after_tool_round_cap(
        &mut context,
        vec![NeutralToolCall {
            call_id: "pending-call".to_string(),
            name: "read_file".to_string(),
            arguments: json!({ "path": "README.md", "timeoutMs": 10000 }),
            thought_signatures: None,
        }],
        "Need one more file.".to_string(),
        Some("Checking evidence.".to_string()),
    )
    .expect("recover after tool round cap");

    assert!(recovered);
    assert!(
        context
            .provider_request
            .messages
            .iter()
            .all(|message| message.tool_calls.is_empty())
    );
    assert!(
        context
            .provider_request
            .messages
            .iter()
            .any(|message| message.content.contains("pending-call"))
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn neutral_messages_from_record_replays_complete_tool_state_and_skips_incomplete_calls() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-tool-state-replay-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");

    database
        .insert_chat("chat-1", "Tool chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Done.",
            sequence: 0,
            metadata_json: Some(r#"{"reasoning":"Used tools."}"#),
        })
        .expect("assistant message insert");
    for (id, name, input, output, started_at) in [
        (
            "call-1",
            "find_files",
            r#"{"path":"."}"#,
            r#"{"entries":[]}"#,
            "2026-06-05T07:00:00Z",
        ),
        (
            "call-2",
            "read_file",
            r#"{"path":"README.md"}"#,
            r#"{"content":"hello"}"#,
            "2026-06-05T07:00:01Z",
        ),
    ] {
        database
            .insert_tool_call(NewToolCall {
                id,
                chat_id: "chat-1",
                run_id: "run-1",
                message_id: Some("assistant-1"),
                tool_name: name,
                input_json: input,
                status: "completed",
                started_at,
                completed_at: Some(started_at),
            })
            .expect("tool call insert");
        let result_id = format!("{id}-result");
        database
            .insert_tool_result(NewToolResult {
                id: &result_id,
                tool_call_id: id,
                output_json: output,
                is_error: false,
                created_at: started_at,
            })
            .expect("tool result insert");
    }
    database
        .insert_tool_call(NewToolCall {
            id: "call-incomplete",
            chat_id: "chat-1",
            run_id: "run-1",
            message_id: Some("assistant-1"),
            tool_name: "run_command",
            input_json: r#"{"command":"git status"}"#,
            status: "cancelled",
            started_at: "2026-06-05T07:00:02Z",
            completed_at: Some("2026-06-05T07:00:03Z"),
        })
        .expect("incomplete cancelled tool call insert");
    database
        .insert_tool_call(NewToolCall {
            id: "call-incomplete-completed",
            chat_id: "chat-1",
            run_id: "run-1",
            message_id: Some("assistant-1"),
            tool_name: "run_command",
            input_json: r#"{"command":"git commit"}"#,
            status: "completed",
            started_at: "2026-06-05T07:00:04Z",
            completed_at: Some("2026-06-05T07:00:05Z"),
        })
        .expect("incomplete completed tool call insert");

    let message = database
        .messages_for_chat("chat-1")
        .expect("messages")
        .into_iter()
        .next()
        .expect("assistant message");
    let messages =
        neutral_messages_from_record(&database, message).expect("neutral message replay");

    assert_eq!(messages.len(), 5);
    assert_eq!(messages[0].role, NeutralChatRole::Assistant);
    assert_eq!(messages[0].tool_calls[0].call_id, "call-1");
    assert_eq!(messages[1].role, NeutralChatRole::Tool);
    assert_eq!(messages[1].tool_call_id.as_deref(), Some("call-1"));
    assert_eq!(messages[2].role, NeutralChatRole::Assistant);
    assert_eq!(messages[2].tool_calls[0].call_id, "call-2");
    assert_eq!(messages[3].role, NeutralChatRole::Tool);
    assert_eq!(messages[3].tool_call_id.as_deref(), Some("call-2"));
    assert_eq!(messages[4].role, NeutralChatRole::Assistant);
    assert_eq!(messages[4].content, "Done.");
    assert_eq!(messages[4].reasoning.as_deref(), Some("Used tools."));
    assert!(messages.iter().all(|message| {
        message.tool_calls.iter().all(|tool_call| {
            tool_call.call_id != "call-incomplete"
                && tool_call.call_id != "call-incomplete-completed"
        })
    }));

    drop(messages);
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn neutral_messages_from_record_replays_stored_assistant_parts_in_order() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-part-replay-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");

    database
        .insert_chat("chat-1", "Part replay chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Before.After.",
            sequence: 0,
            metadata_json: Some(
                r#"{"reasoning":"Think one.Think two.","parts":[{"type":"reasoning","text":"Think one."},{"type":"text","text":"Before."},{"type":"toolCall","tool_call_id":"call-1"},{"type":"reasoning","text":"Think two."},{"type":"text","text":"After."}],"partsVersion":2,"partsSource":"live_sse"}"#,
            ),
        })
        .expect("assistant message insert");
    database
        .insert_tool_call(NewToolCall {
            id: "call-1",
            chat_id: "chat-1",
            run_id: "run-1",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"README.md"}"#,
            status: "completed",
            started_at: "2026-06-21T10:00:00Z",
            completed_at: Some("2026-06-21T10:00:01Z"),
        })
        .expect("tool call insert");
    database
        .insert_tool_result(NewToolResult {
            id: "call-1-result",
            tool_call_id: "call-1",
            output_json: r#"{"content":"hello"}"#,
            is_error: false,
            created_at: "2026-06-21T10:00:01Z",
        })
        .expect("tool result insert");

    let message = database
        .messages_for_chat("chat-1")
        .expect("messages")
        .into_iter()
        .next()
        .expect("assistant message");
    let messages =
        neutral_messages_from_record(&database, message).expect("neutral message replay");

    assert_eq!(messages.len(), 6);
    assert_eq!(messages[0].role, NeutralChatRole::Assistant);
    assert!(messages[0].content.is_empty());
    assert_eq!(messages[0].reasoning.as_deref(), Some("Think one."));
    assert_eq!(messages[1].role, NeutralChatRole::Assistant);
    assert_eq!(messages[1].content, "Before.");
    assert!(messages[1].reasoning.is_none());
    assert_eq!(messages[2].role, NeutralChatRole::Assistant);
    assert_eq!(messages[2].tool_calls[0].call_id, "call-1");
    assert_eq!(messages[3].role, NeutralChatRole::Tool);
    assert_eq!(messages[3].tool_call_id.as_deref(), Some("call-1"));
    assert_eq!(messages[4].role, NeutralChatRole::Assistant);
    assert!(messages[4].content.is_empty());
    assert_eq!(messages[4].reasoning.as_deref(), Some("Think two."));
    assert_eq!(messages[5].role, NeutralChatRole::Assistant);
    assert_eq!(messages[5].content, "After.");

    drop(messages);
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn normalized_chat_attachments_rejects_invalid_payloads() {
    let invalid_base64 = normalized_chat_attachments(vec![ChatAttachmentInput {
        id: "att-1".to_string(),
        name: "image.png".to_string(),
        content_type: "image/png".to_string(),
        content_base64: Some("not base64".to_string()),
        path: None,
        size_bytes: 1,
    }])
    .expect_err("invalid base64 should fail");
    assert!(invalid_base64.message.contains("invalid base64"));

    let size_mismatch = normalized_chat_attachments(vec![ChatAttachmentInput {
        id: "att-1".to_string(),
        name: "image.png".to_string(),
        content_type: "image/png".to_string(),
        content_base64: Some("SGVsbG8=".to_string()),
        path: None,
        size_bytes: 6,
    }])
    .expect_err("size mismatch should fail");
    assert!(size_mismatch.message.contains("sizeBytes"));

    let text_base64 = normalized_chat_attachments(vec![ChatAttachmentInput {
        id: "att-1".to_string(),
        name: "note.txt".to_string(),
        content_type: "text/plain".to_string(),
        content_base64: Some("SGVsbG8=".to_string()),
        path: None,
        size_bytes: 5,
    }])
    .expect_err("text base64 should fail");
    assert!(text_base64.message.contains("must use path"));
}

#[test]
fn text_attachments_use_original_path_in_user_prompt() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-text-attachment-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let attachment_path = workspace_dir.join("note.txt");
    let attachment_path_string = attachment_path.display().to_string();
    fs::write(&attachment_path, "Hello").expect("write attachment");

    let attachments = normalized_chat_attachments(vec![ChatAttachmentInput {
        id: "att-1".to_string(),
        name: "note.txt".to_string(),
        content_type: "text/plain".to_string(),
        content_base64: None,
        path: Some(attachment_path_string.clone()),
        size_bytes: 5,
    }])
    .expect("path attachment");

    assert_eq!(attachments[0].content_base64, None);
    assert_eq!(
        attachments[0].path.as_deref(),
        Some(attachment_path_string.as_str())
    );

    let message = neutral_user_message("Review it".to_string(), attachments.clone());
    assert!(message.content.contains("# Files mentioned by the user:"));
    assert!(message.content.contains("## note.txt:"));
    assert!(message.content.contains(&attachment_path_string));
    assert!(message.content.contains("## My request for Foco:"));
    assert!(!message.content.contains("SGVsbG8="));

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let metadata_json = user_message_metadata_json(&attachments).expect("attachment metadata");
    let stored_message = MessageRecord {
        id: "user-1".to_string(),
        chat_id: "chat-1".to_string(),
        role: "user".to_string(),
        content: "Review it".to_string(),
        sequence: 0,
        created_at: "2026-06-08T10:00:00Z".to_string(),
        metadata_json,
    };
    let replayed_messages =
        neutral_messages_from_record(&database, stored_message).expect("neutral messages");
    assert!(
        replayed_messages[0]
            .content
            .contains(&attachment_path_string)
    );

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn active_chat_run_registry_accepts_matching_guidance() {
    let registry = ActiveChatRunRegistry::default();
    let (guidance_tx, mut guidance_rx) = mpsc::unbounded_channel();
    let _registration = registry
        .register(
            "run-1".to_string(),
            "workspace-1".to_string(),
            "chat-1".to_string(),
            "assistant-1".to_string(),
            1,
            Vec::new(),
            true,
            0,
            guidance_tx,
        )
        .expect("register active run");

    let guidance = registry
        .push_guidance(
            "workspace-1",
            ChatGuidanceRequest {
                chat_id: "chat-1".to_string(),
                run_id: "run-1".to_string(),
                message: "Prefer the simpler implementation.".to_string(),
                attachments: Vec::new(),
            },
        )
        .expect("push guidance");

    assert_eq!(guidance.content, "Prefer the simpler implementation.");
    let received = guidance_rx.try_recv().expect("guidance delivered");
    assert_eq!(received.id, guidance.id);
    assert_eq!(received.content, guidance.content);
}

#[test]
fn active_chat_run_record_event_persists_streaming_assistant_message() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-streaming-assistant-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    database
        .insert_chat("chat-1", "Streaming chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "user-1",
            chat_id: "chat-1",
            role: "user",
            content: "Tell me something.",
            sequence: 0,
            metadata_json: None,
        })
        .expect("user message insert");
    drop(database);

    let registry = ActiveChatRunRegistry::default();
    let (guidance_tx, _guidance_rx) = mpsc::unbounded_channel();
    let mut registration = registry
        .register(
            "run-1".to_string(),
            "workspace-1".to_string(),
            "chat-1".to_string(),
            "assistant-1".to_string(),
            1,
            Vec::new(),
            true,
            0,
            guidance_tx,
        )
        .expect("register active run");

    registration
        .record_event(
            &workspace_dir,
            "chat-1",
            &ChatSseEvent::TextDelta {
                assistant_message_id: "assistant-1".to_string(),
                delta: "Partial".to_string(),
            },
        )
        .expect("text delta record");
    registration
        .record_event(
            &workspace_dir,
            "chat-1",
            &ChatSseEvent::ReasoningDelta {
                assistant_message_id: "assistant-1".to_string(),
                delta: "Thinking".to_string(),
            },
        )
        .expect("reasoning delta record");
    registration
        .record_event(
            &workspace_dir,
            "chat-1",
            &ChatSseEvent::Error {
                message: "provider failed".to_string(),
            },
        )
        .expect("error record");

    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database reopen");
    let messages = database
        .messages_for_chat("chat-1")
        .expect("messages for chat");
    let assistant = messages
        .iter()
        .find(|message| message.id == "assistant-1")
        .expect("assistant message");
    let metadata = parse_json_value(&assistant.metadata_json, "assistant metadata")
        .expect("assistant metadata json");

    assert_eq!(assistant.content, "Partial");
    assert_eq!(assistant.sequence, 1);
    assert_eq!(metadata["reasoning"], "Thinking");
    assert_eq!(metadata["streamingState"], "failed");

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn active_chat_run_finish_suspended_clears_streaming_assistant_state() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-suspended-assistant-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    database
        .insert_chat("chat-1", "Suspended chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "user-1",
            chat_id: "chat-1",
            role: "user",
            content: "Delegate this.",
            sequence: 0,
            metadata_json: None,
        })
        .expect("user message insert");
    drop(database);

    let registry = ActiveChatRunRegistry::default();
    let (guidance_tx, _guidance_rx) = mpsc::unbounded_channel();
    let mut registration = registry
        .register(
            "run-1".to_string(),
            "workspace-1".to_string(),
            "chat-1".to_string(),
            "assistant-1".to_string(),
            1,
            Vec::new(),
            true,
            0,
            guidance_tx,
        )
        .expect("register active run");

    registration
        .record_event(
            &workspace_dir,
            "chat-1",
            &ChatSseEvent::TextDelta {
                assistant_message_id: "assistant-1".to_string(),
                delta: "Waiting on worker.".to_string(),
            },
        )
        .expect("text delta record");
    registration
        .finish_suspended(&workspace_dir, "chat-1")
        .expect("finish suspended run");

    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database reopen");
    let assistant = database
        .message("assistant-1")
        .expect("assistant read")
        .expect("assistant message");
    let metadata = parse_json_value(&assistant.metadata_json, "assistant metadata")
        .expect("assistant metadata json");

    assert_eq!(assistant.content, "Waiting on worker.");
    assert!(metadata.get("streamingState").is_none());
    assert!(
        registry
            .active_run_for_chat("workspace-1", "chat-1")
            .expect("active run lookup")
            .is_none()
    );

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn active_chat_run_private_output_records_events_without_main_chat_draft() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-private-agent-stream-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    database
        .insert_chat("chat-1", "Private agent stream")
        .expect("chat insert");
    drop(database);

    let registry = ActiveChatRunRegistry::default();
    let (guidance_tx, _guidance_rx) = mpsc::unbounded_channel();
    let mut registration = registry
        .register(
            "agent-task-private-run".to_string(),
            "workspace-1".to_string(),
            "chat-1".to_string(),
            "assistant-private".to_string(),
            1,
            Vec::new(),
            false,
            0,
            guidance_tx,
        )
        .expect("register private active run");

    assert!(
        registry
            .active_run_for_chat("workspace-1", "chat-1")
            .expect("active run lookup")
            .is_none()
    );

    registration
        .record_event(
            &workspace_dir,
            "chat-1",
            &ChatSseEvent::TextDelta {
                assistant_message_id: "assistant-private".to_string(),
                delta: "Worker-only text".to_string(),
            },
        )
        .expect("private text delta record");
    registration
        .record_event(
            &workspace_dir,
            "chat-1",
            &ChatSseEvent::ToolCall {
                assistant_message_id: "assistant-private".to_string(),
                tool_call: ChatToolCallSummary {
                    id: "tool-private".to_string(),
                    name: "read_file".to_string(),
                    status: "running".to_string(),
                    input: json!({ "path": "README.md" }),
                    output: None,
                    is_error: false,
                },
            },
        )
        .expect("private tool call record");

    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database reopen");
    assert!(
        database
            .messages_for_chat("chat-1")
            .expect("messages for chat")
            .is_empty()
    );
    assert!(
        database
            .tool_calls_for_chat("chat-1")
            .expect("tool calls for chat")
            .is_empty()
    );
    assert_eq!(
        database
            .run_events_for_run("agent-task-private-run")
            .expect("run events for private run")
            .len(),
        2
    );

    drop(database);
    registration.finish();
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn active_chat_run_record_event_persists_tools_before_cancelled_history_reload() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-cancelled-tool-history-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    database
        .insert_chat("chat-1", "Cancelled tool chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "user-1",
            chat_id: "chat-1",
            role: "user",
            content: "Inspect the workspace.",
            sequence: 0,
            metadata_json: None,
        })
        .expect("user message insert");
    drop(database);

    let registry = ActiveChatRunRegistry::default();
    let (guidance_tx, _guidance_rx) = mpsc::unbounded_channel();
    let mut registration = registry
        .register(
            "run-1".to_string(),
            "workspace-1".to_string(),
            "chat-1".to_string(),
            "assistant-1".to_string(),
            1,
            Vec::new(),
            true,
            0,
            guidance_tx,
        )
        .expect("register active run");

    for event in [
        ChatSseEvent::ToolCall {
            assistant_message_id: "assistant-1".to_string(),
            tool_call: ChatToolCallSummary {
                id: "tool-reset".to_string(),
                name: "read_file".to_string(),
                status: "running".to_string(),
                input: json!({ "path": "discarded.txt" }),
                output: None,
                is_error: false,
            },
        },
        ChatSseEvent::StreamReset {
            assistant_message_id: "assistant-1".to_string(),
            reason: "retry provider turn".to_string(),
            text: String::new(),
            reasoning: None,
            tool_calls: Vec::new(),
        },
        ChatSseEvent::ToolCall {
            assistant_message_id: "assistant-1".to_string(),
            tool_call: ChatToolCallSummary {
                id: "tool-1".to_string(),
                name: "read_file".to_string(),
                status: "running".to_string(),
                input: json!({ "path": "README.md" }),
                output: None,
                is_error: false,
            },
        },
        ChatSseEvent::ToolResult {
            assistant_message_id: "assistant-1".to_string(),
            tool_call_id: "tool-1".to_string(),
            output: json!({ "content": "hello" }),
            is_error: false,
        },
        ChatSseEvent::ToolCall {
            assistant_message_id: "assistant-1".to_string(),
            tool_call: ChatToolCallSummary {
                id: "tool-2".to_string(),
                name: "run_command".to_string(),
                status: "running".to_string(),
                input: json!({ "command": "cargo test" }),
                output: None,
                is_error: false,
            },
        },
    ] {
        registration
            .record_event(&workspace_dir, "chat-1", &event)
            .expect("tool event record");
    }
    registration.cancellation().cancel();
    registration
        .record_event(
            &workspace_dir,
            "chat-1",
            &ChatSseEvent::Error {
                message: "chat run cancelled".to_string(),
            },
        )
        .expect("cancel event record");

    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database reopen");
    let tool_calls = database
        .tool_calls_for_chat("chat-1")
        .expect("tool calls for chat");
    assert_eq!(tool_calls.len(), 2);
    assert_eq!(tool_calls[0].message_id.as_deref(), Some("assistant-1"));
    assert_eq!(tool_calls[0].status, "completed");
    assert_eq!(
        tool_calls[0]
            .result
            .as_ref()
            .expect("completed tool result")
            .output_json,
        r#"{"content":"hello"}"#
    );
    assert_eq!(tool_calls[1].message_id.as_deref(), Some("assistant-1"));
    assert_eq!(tool_calls[1].status, "cancelled");
    assert!(tool_calls[1].result.is_none());

    let messages = database.messages_for_chat("chat-1").expect("messages");
    let summaries = chat_message_summaries(&mut database, &workspace_dir, None, "chat-1", messages)
        .expect("message summaries");
    let assistant = summaries
        .iter()
        .find(|message| message.id == "assistant-1")
        .expect("assistant summary");
    assert_eq!(assistant.tool_calls.len(), 2);
    assert_eq!(assistant.tool_calls[0].status, "completed");
    assert_eq!(assistant.tool_calls[1].status, "cancelled");
    assert_eq!(
        assistant
            .parts
            .iter()
            .filter(|part| matches!(part, ChatMessagePart::ToolCall { .. }))
            .count(),
        2
    );

    let saved = database
        .message("assistant-1")
        .expect("assistant read")
        .expect("assistant message");
    let metadata = parse_json_value(&saved.metadata_json, "assistant metadata")
        .expect("assistant metadata json");
    assert_eq!(metadata["streamingState"], "cancelled");

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[tokio::test]
async fn team_run_id_override_keeps_tool_finalization_idempotent() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-team-tool-run-id-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-team-tool-run-id-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let state = test_app_state(config.clone(), profile_dir.clone());
    let task_run_id = "agent-task-tool-run";
    let context = prepare_chat_context(
        &state,
        &config,
        &workspace_id,
        ChatStreamRequest {
            queued_user_message_id: None,
            run_id_override: Some(task_run_id.to_string()),
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: "Use a tool".to_string(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("chat context");
    assert_eq!(context.llm_request_id, task_run_id);

    let registry = ActiveChatRunRegistry::default();
    let (guidance_tx, _guidance_rx) = mpsc::unbounded_channel();
    let mut registration = registry
        .register(
            task_run_id.to_string(),
            workspace_id,
            context.chat_id.clone(),
            context.assistant_message_id.clone(),
            context.assistant_sequence,
            Vec::new(),
            true,
            0,
            guidance_tx,
        )
        .expect("register active run");
    let tool_call_event = ChatSseEvent::ToolCall {
        assistant_message_id: context.assistant_message_id.clone(),
        tool_call: ChatToolCallSummary {
            id: "call-team-tool".to_string(),
            name: "read_file".to_string(),
            status: "running".to_string(),
            input: json!({ "path": "README.md" }),
            output: None,
            is_error: false,
        },
    };
    let tool_result_event = ChatSseEvent::ToolResult {
        assistant_message_id: context.assistant_message_id.clone(),
        tool_call_id: "call-team-tool".to_string(),
        output: json!({ "content": "hello" }),
        is_error: false,
    };
    for event in [&tool_call_event, &tool_result_event] {
        registration
            .record_event(&workspace_dir, &context.chat_id, event)
            .expect("record tool event");
    }

    persist_chat_result(
        &context,
        "2026-06-20T08:00:00Z",
        ChatAuditOutcome {
            first_token_at: Some("2026-06-20T08:00:00Z".to_string()),
            completed_at: "2026-06-20T08:00:01Z".to_string(),
            first_token_latency_ms: Some(100),
            total_latency_ms: 1_000,
            input_tokens: Some(10),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            status_code: Some(200),
            final_state: "succeeded",
            response_body_json: Some(r#"{"text":"Done."}"#.to_string()),
        },
        &[
            captured_event(&tool_call_event),
            captured_event(&tool_result_event),
        ],
        Some("Done."),
        None,
        &[ExecutedToolCall {
            id: "call-team-tool".to_string(),
            name: "read_file".to_string(),
            input: json!({ "path": "README.md" }),
            output: json!({ "content": "hello" }),
            is_error: false,
            started_at: "2026-06-20T08:00:00Z".to_string(),
            completed_at: "2026-06-20T08:00:01Z".to_string(),
        }],
    )
    .expect("final persistence should reuse the task run id");

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let tool_calls = database
        .tool_calls_for_chat(&context.chat_id)
        .expect("tool calls for chat");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].run_id, task_run_id);
    assert_eq!(tool_calls[0].status, "completed");
    assert!(tool_calls[0].result.is_some());

    drop(database);
    registration.finish();
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[test]
fn active_chat_run_registry_rejects_stale_guidance_run() {
    let registry = ActiveChatRunRegistry::default();
    let error = registry
        .push_guidance(
            "workspace-1",
            ChatGuidanceRequest {
                chat_id: "chat-1".to_string(),
                run_id: "missing-run".to_string(),
                message: "Use this now.".to_string(),
                attachments: Vec::new(),
            },
        )
        .expect_err("missing run should fail");

    assert!(error.message.contains("active chat run was not found"));
}

#[test]
fn active_chat_run_registry_rejects_guidance_after_complete_event() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-guidance-complete-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    database
        .insert_chat("chat-1", "Complete guidance chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "user-1",
            chat_id: "chat-1",
            role: "user",
            content: "Tell me something.",
            sequence: 0,
            metadata_json: None,
        })
        .expect("user message insert");
    drop(database);

    let registry = ActiveChatRunRegistry::default();
    let (guidance_tx, _guidance_rx) = mpsc::unbounded_channel();
    let mut registration = registry
        .register(
            "run-1".to_string(),
            "workspace-1".to_string(),
            "chat-1".to_string(),
            "assistant-1".to_string(),
            1,
            Vec::new(),
            true,
            0,
            guidance_tx,
        )
        .expect("register active run");

    registration
        .record_event(
            &workspace_dir,
            "chat-1",
            &ChatSseEvent::Complete {
                chat_id: "chat-1".to_string(),
                assistant_message_id: "assistant-1".to_string(),
                text: "Done.".to_string(),
                reasoning: None,
                usage: None,
                stop_reason: Some("stop".to_string()),
                metrics: ChatReplyMetrics {
                    model_id: "model-1".to_string(),
                    provider_id: "provider-1".to_string(),
                    total_latency_ms: Some(1_000),
                    first_token_latency_ms: Some(100),
                    output_tokens: Some(5),
                },
                memories_used: Vec::new(),
            },
        )
        .expect("complete record");

    let summary = registry
        .active_run_for_chat("workspace-1", "chat-1")
        .expect("active run lookup")
        .expect("run remains subscribable until stream end");
    assert!(!summary.accepting_guidance);

    let error = registry
        .push_guidance(
            "workspace-1",
            ChatGuidanceRequest {
                chat_id: "chat-1".to_string(),
                run_id: "run-1".to_string(),
                message: "Use this now.".to_string(),
                attachments: Vec::new(),
            },
        )
        .expect_err("completed run should reject guidance");

    assert!(error.message.contains("no longer accepting guidance"));

    drop(registration);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn user_attachments_round_trip_into_neutral_history_and_message_parts() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-user-attachment-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let metadata_json = user_message_metadata_json(&[NeutralChatAttachment {
        id: "att-1".to_string(),
        name: "screenshot.png".to_string(),
        content_type: "image/png".to_string(),
        size_bytes: 5,
        content_base64: Some("SGVsbG8=".to_string()),
        path: None,
    }])
    .expect("attachment metadata");
    let message = MessageRecord {
        id: "user-1".to_string(),
        chat_id: "chat-1".to_string(),
        role: "user".to_string(),
        content: "See attached.".to_string(),
        sequence: 0,
        created_at: "2026-06-08T10:00:00Z".to_string(),
        metadata_json,
    };

    let neutral_messages =
        neutral_messages_from_record(&database, message.clone()).expect("neutral messages");
    assert_eq!(neutral_messages.len(), 1);
    assert_eq!(neutral_messages[0].attachments.len(), 1);
    assert_eq!(neutral_messages[0].attachments[0].name, "screenshot.png");

    let parts = chat_message_parts(&message, None, &[], &[]).expect("message parts");
    assert_eq!(parts.len(), 2);
    assert!(matches!(parts[0], ChatMessagePart::Text { .. }));
    match &parts[1] {
        ChatMessagePart::Attachment { attachment } => {
            assert_eq!(attachment.name, "screenshot.png");
            assert_eq!(
                attachment.preview_data_url.as_deref(),
                Some("data:image/png;base64,SGVsbG8=")
            );
        }
        other => panic!("expected attachment part, got {other:?}"),
    }

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn chat_message_parts_ignore_unknown_tool_call_audit_events() {
    let message = MessageRecord {
        id: "assistant-1".to_string(),
        chat_id: "chat-1".to_string(),
        role: "assistant".to_string(),
        content: "Done".to_string(),
        sequence: 1,
        created_at: "2026-06-08T10:00:01Z".to_string(),
        metadata_json: "{}".to_string(),
    };
    let events = vec![
        LlmRequestEventRecord {
            id: "event-1".to_string(),
            llm_request_id: "request-1".to_string(),
            sequence: 0,
            event_at: "2026-06-08T10:00:01Z".to_string(),
            event_type: "start".to_string(),
            raw_chunk_json: None,
            normalized_event_json: json!({
                "assistantMessageId": "assistant-1"
            })
            .to_string(),
        },
        LlmRequestEventRecord {
            id: "event-2".to_string(),
            llm_request_id: "request-1".to_string(),
            sequence: 1,
            event_at: "2026-06-08T10:00:02Z".to_string(),
            event_type: "completion".to_string(),
            raw_chunk_json: None,
            normalized_event_json: json!({}).to_string(),
        },
        LlmRequestEventRecord {
            id: "event-3".to_string(),
            llm_request_id: "request-1".to_string(),
            sequence: 2,
            event_at: "2026-06-08T10:00:03Z".to_string(),
            event_type: "tool_call".to_string(),
            raw_chunk_json: None,
            normalized_event_json: json!({
                "assistantMessageId": "assistant-1",
                "toolCall": {
                    "id": "missing-call",
                    "name": "read_file"
                }
            })
            .to_string(),
        },
        LlmRequestEventRecord {
            id: "event-4".to_string(),
            llm_request_id: "request-1".to_string(),
            sequence: 3,
            event_at: "2026-06-08T10:00:04Z".to_string(),
            event_type: "text_delta".to_string(),
            raw_chunk_json: None,
            normalized_event_json: json!({
                "assistantMessageId": "assistant-1",
                "delta": "Done"
            })
            .to_string(),
        },
    ];

    let parts = chat_message_parts(&message, None, &[], &events).expect("message parts");
    assert_eq!(parts.len(), 1);
    match &parts[0] {
        ChatMessagePart::Text { text } => assert_eq!(text, "Done"),
        other => panic!("expected text part, got {other:?}"),
    }
}

#[test]
fn finalized_assistant_parts_persist_compact_tool_references_in_stream_order() {
    let tool_call = ChatToolCallSummary {
        id: "tool-1".to_string(),
        name: "read_file".to_string(),
        status: "completed".to_string(),
        input: json!({ "path": "README.md" }),
        output: Some(json!({ "content": "large result" })),
        is_error: false,
    };
    let events = [
        (
            "text_delta",
            json!({
                "assistantMessageId": "assistant-1",
                "delta": "Before."
            }),
        ),
        (
            "tool_call",
            json!({
                "assistantMessageId": "assistant-1",
                "toolCall": { "id": "tool-1" }
            }),
        ),
        (
            "text_delta",
            json!({
                "assistantMessageId": "assistant-1",
                "delta": "After."
            }),
        ),
    ]
    .into_iter()
    .map(|(event_type, value)| CapturedAuditEvent {
        event_at: "2026-06-18T10:00:00Z".to_string(),
        event_type: event_type.to_string(),
        normalized_event_json: value.to_string(),
    })
    .collect::<Vec<_>>();

    let stored_parts = finalized_assistant_message_parts(
        "assistant-1",
        &events,
        "Before.After.",
        None,
        std::slice::from_ref(&tool_call),
    )
    .expect("stored parts");
    let metadata_json = assistant_message_metadata_json(
        None,
        &[],
        &CodeChangeStats::default(),
        None,
        Some(&stored_parts),
    )
    .expect("assistant metadata");
    assert!(!metadata_json.contains("large result"));
    assert!(metadata_json.contains(r#""partsVersion":2"#));
    assert!(metadata_json.contains(r#""partsSource":"live_sse""#));

    let parts = assistant_parts_from_metadata(&metadata_json, std::slice::from_ref(&tool_call))
        .expect("hydrate parts")
        .expect("stored parts present");
    assert!(matches!(&parts[0], ChatMessagePart::Text { text } if text == "Before."));
    assert!(
        matches!(&parts[1], ChatMessagePart::ToolCall { tool_call } if tool_call.id == "tool-1")
    );
    assert!(matches!(&parts[2], ChatMessagePart::Text { text } if text == "After."));
}

#[test]
fn compact_audit_events_keeps_only_final_tool_call_delta() {
    let events = vec![
        CapturedAuditEvent {
            event_at: "2026-06-18T10:00:00Z".to_string(),
            event_type: "start".to_string(),
            normalized_event_json: json!({ "type": "start" }).to_string(),
        },
        CapturedAuditEvent {
            event_at: "2026-06-18T10:00:01Z".to_string(),
            event_type: "tool_call".to_string(),
            normalized_event_json: json!({
                "type": "toolCall",
                "toolCall": { "callId": "call-1", "arguments": { "path": "REA" } }
            })
            .to_string(),
        },
        CapturedAuditEvent {
            event_at: "2026-06-18T10:00:02Z".to_string(),
            event_type: "tool_call".to_string(),
            normalized_event_json: json!({
                "type": "toolCall",
                "toolCall": { "callId": "call-1", "arguments": { "path": "README.md" } }
            })
            .to_string(),
        },
        CapturedAuditEvent {
            event_at: "2026-06-18T10:00:03Z".to_string(),
            event_type: "text_delta".to_string(),
            normalized_event_json: json!({ "type": "textDelta", "delta": "done" }).to_string(),
        },
    ];

    let compacted = compact_audit_events(&events, true)
        .into_iter()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(compacted, vec![0, 2, 3]);

    let summary_only = compact_audit_events(&events, false)
        .into_iter()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(summary_only, vec![0]);
}

#[test]
fn api_audit_vacuum_requires_large_enough_freelist() {
    assert!(should_vacuum_workspace_database(
        WorkspaceDatabaseSpaceStats {
            page_size_bytes: 4096,
            page_count: 1_770_262,
            freelist_count: 1_437_845,
        }
    ));
    assert!(!should_vacuum_workspace_database(
        WorkspaceDatabaseSpaceStats {
            page_size_bytes: 4096,
            page_count: 100_000,
            freelist_count: 20_000,
        }
    ));
    assert!(!should_vacuum_workspace_database(
        WorkspaceDatabaseSpaceStats {
            page_size_bytes: 4096,
            page_count: 1_000_000,
            freelist_count: 1_000,
        }
    ));
}

#[test]
fn historical_chat_materializes_interleaved_parts_once_from_run_events() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-history-parts-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    database
        .insert_chat("chat-1", "History order")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Before.After.",
            sequence: 0,
            metadata_json: Some(
                r#"{"reasoning":"Think one.Think two.","parts":[{"type":"reasoning","text":"Think one.Think two."},{"type":"toolCall","tool_call_id":"tool-1"},{"type":"text","text":"Before.After."}],"partsVersion":2,"partsSource":"live_sse"}"#,
            ),
        })
        .expect("assistant insert");
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
            model_id: "gpt-test",
            request_started_at: "2026-06-18T10:00:00Z",
            first_token_at: Some("2026-06-18T10:00:00Z"),
            completed_at: Some("2026-06-18T10:00:01Z"),
            input_tokens: Some(10),
            output_tokens: Some(10),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            first_token_latency_ms: Some(10),
            total_latency_ms: Some(1000),
            status_code: Some(200),
            final_state: "succeeded",
            request_body_json: Some("{}"),
            response_body_json: Some("{}"),
        })
        .expect("request insert");
    database
        .insert_llm_request_event(NewLlmRequestEvent {
            id: "request-event-0",
            llm_request_id: "request-1",
            sequence: 0,
            event_at: "2026-06-18T10:00:00Z",
            event_type: "start",
            raw_chunk_json: None,
            normalized_event_json: r#"{"assistantMessageId":"assistant-1"}"#,
        })
        .expect("request start event insert");
    for (sequence, event_type, value) in [
        (
            0,
            "reasoning_delta",
            json!({ "assistant_message_id": "assistant-1", "delta": "Think one." }),
        ),
        (
            1,
            "text_delta",
            json!({ "assistant_message_id": "assistant-1", "delta": "Before." }),
        ),
        (
            2,
            "tool_call",
            json!({ "assistant_message_id": "assistant-1", "tool_call": { "id": "tool-1" } }),
        ),
        (
            3,
            "reasoning_delta",
            json!({ "assistant_message_id": "assistant-1", "delta": "Think two." }),
        ),
        (
            4,
            "text_delta",
            json!({ "assistant_message_id": "assistant-1", "delta": "After." }),
        ),
    ] {
        database
            .insert_run_event(NewRunEvent {
                id: &format!("run-event-{sequence}"),
                chat_id: "chat-1",
                run_id: "run-1",
                sequence,
                event_type,
                payload_json: &value.to_string(),
            })
            .expect("run event insert");
    }
    database
        .insert_tool_call(NewToolCall {
            id: "tool-1",
            chat_id: "chat-1",
            run_id: "request-1",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"README.md"}"#,
            status: "completed",
            started_at: "2026-06-18T10:00:00Z",
            completed_at: Some("2026-06-18T10:00:01Z"),
        })
        .expect("tool insert");
    database
        .insert_tool_result(NewToolResult {
            id: "tool-result-1",
            tool_call_id: "tool-1",
            output_json: r#"{"content":"large result"}"#,
            is_error: false,
            created_at: "2026-06-18T10:00:01Z",
        })
        .expect("tool result insert");

    let messages = database.messages_for_chat("chat-1").expect("messages");
    let summary = chat_message_summaries(&mut database, &workspace_dir, None, "chat-1", messages)
        .expect("message summaries")
        .into_iter()
        .next()
        .expect("assistant summary");
    assert!(
        matches!(&summary.parts[0], ChatMessagePart::Reasoning { text } if text == "Think one.")
    );
    assert!(matches!(&summary.parts[1], ChatMessagePart::Text { text } if text == "Before."));
    assert!(
        matches!(&summary.parts[2], ChatMessagePart::ToolCall { tool_call } if tool_call.id == "tool-1")
    );
    assert!(
        matches!(&summary.parts[3], ChatMessagePart::Reasoning { text } if text == "Think two.")
    );
    assert!(matches!(&summary.parts[4], ChatMessagePart::Text { text } if text == "After."));

    let saved = database
        .message("assistant-1")
        .expect("saved message read")
        .expect("saved message");
    assert!(saved.metadata_json.contains(r#""tool_call_id":"tool-1""#));
    assert!(saved.metadata_json.contains(r#""partsVersion":2"#));
    assert!(
        saved
            .metadata_json
            .contains(r#""partsSource":"run_events""#)
    );
    assert!(!saved.metadata_json.contains("large result"));

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn historical_chat_materializes_streaming_draft_parts_from_run_events() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-streaming-history-parts-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    database
        .insert_chat("chat-1", "Streaming history order")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Before.After.",
            sequence: 0,
            metadata_json: Some(
                r#"{"reasoning":"Think one.Think two.","streamingState":"streaming"}"#,
            ),
        })
        .expect("assistant insert");
    for (sequence, event_type, value) in [
        (
            0,
            "reasoning_delta",
            json!({ "assistantMessageId": "assistant-1", "delta": "Think one." }),
        ),
        (
            1,
            "text_delta",
            json!({ "assistantMessageId": "assistant-1", "delta": "Before." }),
        ),
        (
            2,
            "tool_call",
            json!({ "assistantMessageId": "assistant-1", "toolCall": { "id": "tool-1" } }),
        ),
        (
            3,
            "reasoning_delta",
            json!({ "assistantMessageId": "assistant-1", "delta": "Think two." }),
        ),
        (
            4,
            "text_delta",
            json!({ "assistantMessageId": "assistant-1", "delta": "After." }),
        ),
    ] {
        database
            .insert_run_event(NewRunEvent {
                id: &format!("run-event-{sequence}"),
                chat_id: "chat-1",
                run_id: "agent-task-1",
                sequence,
                event_type,
                payload_json: &value.to_string(),
            })
            .expect("run event insert");
    }
    database
        .insert_tool_call(NewToolCall {
            id: "tool-1",
            chat_id: "chat-1",
            run_id: "agent-task-1",
            message_id: Some("assistant-1"),
            tool_name: "read_file",
            input_json: r#"{"path":"README.md"}"#,
            status: "running",
            started_at: "2026-06-18T10:00:00Z",
            completed_at: None,
        })
        .expect("tool insert");

    let messages = database.messages_for_chat("chat-1").expect("messages");
    let summary = chat_message_summaries(&mut database, &workspace_dir, None, "chat-1", messages)
        .expect("message summaries")
        .into_iter()
        .next()
        .expect("assistant summary");
    assert!(
        matches!(&summary.parts[0], ChatMessagePart::Reasoning { text } if text == "Think one.")
    );
    assert!(matches!(&summary.parts[1], ChatMessagePart::Text { text } if text == "Before."));
    assert!(
        matches!(&summary.parts[2], ChatMessagePart::ToolCall { tool_call } if tool_call.id == "tool-1")
    );
    assert!(
        matches!(&summary.parts[3], ChatMessagePart::Reasoning { text } if text == "Think two.")
    );
    assert!(matches!(&summary.parts[4], ChatMessagePart::Text { text } if text == "After."));

    let saved = database
        .message("assistant-1")
        .expect("saved message read")
        .expect("saved message");
    assert!(
        saved
            .metadata_json
            .contains(r#""streamingState":"streaming""#)
    );
    assert!(
        saved
            .metadata_json
            .contains(r#""partsSource":"run_events""#)
    );
    assert!(saved.metadata_json.contains(r#""partsVersion":2"#));

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn historical_chat_fallback_parts_are_not_cached_as_run_events() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-history-fallback-parts-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    database
        .insert_chat("chat-1", "Fallback history")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Fallback answer.",
            sequence: 0,
            metadata_json: Some(r#"{"reasoning":"Fallback reasoning."}"#),
        })
        .expect("assistant insert");

    let messages = database.messages_for_chat("chat-1").expect("messages");
    let summary = chat_message_summaries(&mut database, &workspace_dir, None, "chat-1", messages)
        .expect("message summaries")
        .into_iter()
        .next()
        .expect("assistant summary");
    assert!(
        matches!(&summary.parts[0], ChatMessagePart::Reasoning { text } if text == "Fallback reasoning.")
    );
    assert!(
        matches!(&summary.parts[1], ChatMessagePart::Text { text } if text == "Fallback answer.")
    );

    let saved = database
        .message("assistant-1")
        .expect("saved message read")
        .expect("saved message");
    assert!(!saved.metadata_json.contains("partsSource"));
    assert!(!saved.metadata_json.contains("partsVersion"));
    assert!(!saved.metadata_json.contains(r#""parts""#));

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn workspace_logo_file_detects_manual_logo_png() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-logo-test"));
    let logo_dir = workspace_dir.join(".foco");
    fs::create_dir_all(&logo_dir).expect("logo directory");
    fs::write(logo_dir.join("logo.png"), b"\x89PNG\r\n\x1A\nlogo").expect("write manual logo");

    let logo = workspace_logo_file(&workspace_dir)
        .expect("logo lookup")
        .expect("manual logo");
    assert_eq!(logo.kind.content_type, "image/png");
    assert_eq!(
        logo.path.file_name().and_then(|name| name.to_str()),
        Some("logo.png")
    );

    let mut workspace = test_workspace_config("workspace-1");
    workspace.path = workspace_dir.clone();
    let logo_url = workspace_logo_url(&workspace)
        .expect("logo url lookup")
        .expect("logo url");
    assert!(logo_url.starts_with("/api/workspaces/workspace-1/logo?v="));

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn save_workspace_logo_file_uses_detected_extension_and_removes_old_logo() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-logo-save-test"));
    let logo_dir = workspace_dir.join(".foco");
    fs::create_dir_all(&logo_dir).expect("logo directory");
    fs::write(logo_dir.join("logo.png"), b"\x89PNG\r\n\x1A\nold").expect("write old logo");
    let jpeg = &[0xFF, 0xD8, 0xFF, 0xE0, b'l', b'o', b'g', b'o'];
    let kind = workspace_logo_kind(jpeg).expect("jpeg kind");

    save_workspace_logo_file(&workspace_dir, jpeg, kind).expect("save logo");

    assert!(!logo_dir.join("logo.png").exists());
    assert!(logo_dir.join("logo.jpg").exists());
    let logo = workspace_logo_file(&workspace_dir)
        .expect("logo lookup")
        .expect("saved logo");
    assert_eq!(logo.kind.content_type, "image/jpeg");
    assert_eq!(
        logo.path.file_name().and_then(|name| name.to_str()),
        Some("logo.jpg")
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn clear_workspace_logo_file_removes_logo_candidates() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-logo-clear-test"));
    let logo_dir = workspace_dir.join(".foco");
    fs::create_dir_all(&logo_dir).expect("logo directory");
    fs::write(logo_dir.join("logo.png"), b"\x89PNG\r\n\x1A\nold").expect("write png logo");
    fs::write(
        logo_dir.join("logo.jpg"),
        [0xFF, 0xD8, 0xFF, 0xE0, b'l', b'o', b'g', b'o'],
    )
    .expect("write jpeg logo");

    clear_workspace_logo_file(&workspace_dir).expect("clear logo");

    assert!(!logo_dir.join("logo.png").exists());
    assert!(!logo_dir.join("logo.jpg").exists());
    assert!(
        workspace_logo_file(&workspace_dir)
            .expect("logo lookup")
            .is_none()
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn workspace_logo_file_rejects_extension_type_mismatch() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-logo-mismatch-test"));
    let logo_dir = workspace_dir.join(".foco");
    fs::create_dir_all(&logo_dir).expect("logo directory");
    fs::write(
        logo_dir.join("logo.png"),
        [0xFF, 0xD8, 0xFF, 0xE0, b'l', b'o', b'g', b'o'],
    )
    .expect("write mismatched logo");

    let error = workspace_logo_file(&workspace_dir).expect_err("mismatch should fail");
    assert!(error.message.contains("does not match"));

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn agent_scheduler_reconciliation_interrupts_active_attempt_without_replaying_queue() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-agent-reconcile-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let config = prompt_test_config(workspace_dir.clone());
    let workspace = config.workspaces[0].clone();
    let team_id = foco_agent::AgentTeamId::new("agent-team-reconcile").expect("team id");
    let instance_id =
        foco_agent::AgentInstanceId::new("agent-instance-reconcile").expect("instance id");
    let active_task = foco_agent::AgentTaskId::new("agent-task-reconcile-active").expect("task id");
    let queued_task = foco_agent::AgentTaskId::new("agent-task-reconcile-queued").expect("task id");
    let attempt_id =
        foco_agent::AgentAttemptId::new("agent-attempt-reconcile").expect("attempt id");
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace.path).expect("workspace database");
        database
            .insert_chat("chat-reconcile", "Reconcile")
            .expect("chat insert");
        let definition = AgentDefinitionSettings {
            id: AgentDefinitionId::new("agent-definition-reconcile").expect("definition id"),
            revision: 1,
            name: "Reconcile coordinator".to_string(),
            description: String::new(),
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            model_options: AgentModelOptions::default(),
            system_prompt: "Be precise.".to_string(),
            allowed_tools: Vec::new(),
            max_instances: 1,
            allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
            permissions: AgentPermissions::default(),
        };
        database
            .create_agent_team(foco_store::workspace::NewAgentTeam {
                id: &team_id,
                chat_id: "chat-reconcile",
                coordinator_instance_id: &instance_id,
                coordinator_definition: &definition,
                max_concurrent_runs: 1,
            })
            .expect("team create");
        for task_id in [&active_task, &queued_task] {
            database
                .enqueue_agent_task(foco_store::workspace::NewAgentTask {
                    id: task_id,
                    team_id: &team_id,
                    owner_instance_id: &instance_id,
                    origin_instance_id: None,
                    parent_task_id: None,
                    input_json: "{}",
                })
                .expect("enqueue");
        }
        database
            .claim_runnable_agent_task(&team_id, &active_task, &attempt_id)
            .expect("claim")
            .expect("claimed");
    }

    let state = test_app_state(config, workspace_dir.clone());
    reconcile_agent_runtime(&state).expect("reconcile");
    let database = WorkspaceDatabase::open_or_create(&workspace.path).expect("workspace database");
    assert_eq!(
        database
            .agent_task(&active_task)
            .expect("active task")
            .expect("active task")
            .status,
        foco_agent::AgentTaskStatus::Interrupted
    );
    assert_eq!(
        database
            .agent_task(&queued_task)
            .expect("queued task")
            .expect("queued task")
            .status,
        foco_agent::AgentTaskStatus::Queued
    );
    assert_eq!(
        database
            .agent_instance(&instance_id)
            .expect("instance")
            .expect("instance")
            .status,
        foco_agent::AgentInstanceStatus::Paused
    );
    assert_eq!(
        database
            .agent_attempts_for_task(&active_task)
            .expect("attempts")[0]
            .status,
        foco_agent::AgentAttemptStatus::Interrupted
    );
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[tokio::test]
async fn agent_scheduler_exits_when_app_shutdown_channel_closes() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-agent-shutdown-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let state = test_app_state(
        prompt_test_config(workspace_dir.clone()),
        workspace_dir.clone(),
    );
    let (scheduler, wake_rx) = AgentScheduler::new();
    let task = scheduler.spawn(state, wake_rx);
    timeout(Duration::from_secs(1), task)
        .await
        .expect("scheduler shutdown timeout")
        .expect("scheduler task");
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[tokio::test]
async fn agent_team_api_enables_and_controls_a_coordinator_snapshot() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-agent-team-api-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut config = prompt_test_config(workspace_dir.clone());
    config.models.push(ModelSettings {
        id: "client-selection".to_string(),
        display_name: "Client selection".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    let definition_id =
        AgentDefinitionId::new("agent-definition-api-coordinator").expect("definition id");
    config.agent_definitions.push(AgentDefinitionSettings {
        id: definition_id.clone(),
        revision: 1,
        name: "API coordinator".to_string(),
        description: String::new(),
        provider_id: "provider".to_string(),
        model_id: "model".to_string(),
        model_options: AgentModelOptions::default(),
        system_prompt: "Coordinate.".to_string(),
        allowed_tools: Vec::new(),
        max_instances: 1,
        allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
        permissions: AgentPermissions::default(),
    });
    let workspace_id = config.workspaces[0].id.clone();
    let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    database
        .insert_chat("chat-agent-api", "Agent API")
        .expect("chat insert");
    drop(database);

    let mut state = test_app_state(config, workspace_dir.clone());
    let (scheduler, mut scheduler_rx) = AgentScheduler::new();
    state.agent_scheduler = scheduler;
    let _response = crate::http::agents::enable_agent_team(
        State(state.clone()),
        AxumPath((workspace_id.clone(), "chat-agent-api".to_string())),
        Json(foco_agent::TeamActivationRequest {
            coordinator_definition_id: definition_id,
        }),
    )
    .await
    .expect("enable team");
    assert_eq!(scheduler_rx.recv().await, Some(()));

    let queued = crate::http::chat::queue_chat_message(
        State(state.clone()),
        AxumPath(workspace_id.clone()),
        Json(QueueChatMessageRequest {
            chat_id: Some("chat-agent-api".to_string()),
            model_id: "client-selection".to_string(),
            provider_id: Some("provider".to_string()),
            thinking_level: None,
            skill_ids: None,
            message: "First Coordinator task".to_string(),
            team_mode_enabled: false,
            defer_start: false,
            attachments: Vec::new(),
        }),
    )
    .await
    .expect("queue Coordinator task")
    .0;
    let task_id = queued.agent_task_id.expect("Agent task id");
    assert_eq!(scheduler_rx.recv().await, Some(()));
    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    let team = database
        .agent_team_for_chat("chat-agent-api")
        .expect("team lookup")
        .expect("enabled team");
    assert_eq!(
        team.max_concurrent_runs,
        DEFAULT_AGENT_TEAM_MAX_CONCURRENT_RUNS
    );
    let task = database.agent_task(&task_id).expect("task").expect("task");
    assert_eq!(task.status, foco_agent::AgentTaskStatus::Queued);
    let input = serde_json::from_str::<CoordinatorTaskInput>(&task.input_json)
        .expect("Coordinator task input");
    assert!(!input.collaboration_tools_enabled);
    let user_message = database
        .message(&queued.user_message_id)
        .expect("user message")
        .expect("user message");
    let user_metadata =
        parse_json_value(&user_message.metadata_json, "user metadata").expect("user metadata");
    assert_eq!(
        user_metadata["queuedRun"]["modelId"],
        json!("client-selection")
    );
    drop(database);
    let cancel_request =
        serde_json::from_value(json!({ "action": "cancel" })).expect("task action request");
    let _response = crate::http::agents::agent_task_action(
        State(state.clone()),
        AxumPath((workspace_id.clone(), task_id.to_string())),
        Json(cancel_request),
    )
    .await
    .expect("cancel queued Coordinator task");
    assert_eq!(scheduler_rx.recv().await, Some(()));

    for (action, expected) in [
        ("pause", foco_agent::AgentTeamStatus::Paused),
        ("resume", foco_agent::AgentTeamStatus::Active),
        ("stop", foco_agent::AgentTeamStatus::Stopped),
    ] {
        let request = serde_json::from_value(json!({
            "scope": "team",
            "action": action,
        }))
        .expect("runtime action request");
        let _response = crate::http::agents::agent_runtime_action(
            State(state.clone()),
            AxumPath((workspace_id.clone(), "chat-agent-api".to_string())),
            Json(request),
        )
        .await
        .expect("team action");
        let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        assert_eq!(
            database
                .agent_team_for_chat("chat-agent-api")
                .expect("team")
                .expect("team")
                .status,
            expected
        );
        let _ = scheduler_rx.try_recv();
    }

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[tokio::test]
async fn team_chat_task_sse_returns_while_coordinator_task_is_still_queued() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-agent-team-queued-stream-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-agent-team-queued-stream-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace = config.workspaces[0].clone();
    let chat_id = "chat-agent-queued-stream";
    let team_id = foco_agent::AgentTeamId::new("agent-team-queued-stream").expect("team id");
    let instance_id =
        foco_agent::AgentInstanceId::new("agent-instance-queued-stream").expect("instance id");
    let running_task_id =
        foco_agent::AgentTaskId::new("agent-task-queued-stream-running").expect("task id");
    let queued_task_id =
        foco_agent::AgentTaskId::new("agent-task-queued-stream-waiting").expect("task id");
    let attempt_id =
        foco_agent::AgentAttemptId::new("agent-attempt-queued-stream").expect("attempt id");

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        database
            .insert_chat(chat_id, "Queued Coordinator stream")
            .expect("chat insert");
        let definition = AgentDefinitionSettings {
            id: AgentDefinitionId::new("agent-definition-queued-stream").expect("definition id"),
            revision: 1,
            name: "Queued stream coordinator".to_string(),
            description: String::new(),
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            model_options: AgentModelOptions::default(),
            system_prompt: "Coordinate.".to_string(),
            allowed_tools: Vec::new(),
            max_instances: 1,
            allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
            permissions: AgentPermissions::default(),
        };
        database
            .create_agent_team(foco_store::workspace::NewAgentTeam {
                id: &team_id,
                chat_id,
                coordinator_instance_id: &instance_id,
                coordinator_definition: &definition,
                max_concurrent_runs: 1,
            })
            .expect("team create");
        database
            .enqueue_agent_task(foco_store::workspace::NewAgentTask {
                id: &running_task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue running task");
        database
            .claim_runnable_agent_task(&team_id, &running_task_id, &attempt_id)
            .expect("claim running task")
            .expect("running task claimed");
        database
            .enqueue_agent_task(foco_store::workspace::NewAgentTask {
                id: &queued_task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue queued task");
    }

    let state = test_app_state(config, profile_dir.clone());
    let response = timeout(
        Duration::from_millis(200),
        crate::http::chat::team_chat_task_sse(&state, &workspace, &queued_task_id),
    )
    .await
    .expect("queued Coordinator stream should not wait for task start")
    .expect("queued Coordinator stream response");
    drop(response);

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    assert_eq!(
        database
            .agent_task(&queued_task_id)
            .expect("queued task")
            .expect("queued task")
            .status,
        foco_agent::AgentTaskStatus::Queued
    );
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn queue_chat_message_creates_default_team_for_normal_send() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-agent-normal-queue-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-agent-normal-queue-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let mut state = test_app_state(config, profile_dir.clone());
    let (scheduler, mut scheduler_rx) = AgentScheduler::new();
    state.agent_scheduler = scheduler;

    let queued = crate::http::chat::queue_chat_message(
        State(state),
        AxumPath(workspace_id.clone()),
        Json(QueueChatMessageRequest {
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: Some("provider".to_string()),
            thinking_level: None,
            skill_ids: None,
            message: "Normal send".to_string(),
            team_mode_enabled: false,
            defer_start: false,
            attachments: Vec::new(),
        }),
    )
    .await
    .expect("queue normal message")
    .0;
    let task_id = queued.agent_task_id.expect("Agent task id");
    assert_eq!(scheduler_rx.recv().await, Some(()));

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    let team = database
        .agent_team_for_chat(&queued.chat_id)
        .expect("team lookup")
        .expect("default team");
    assert_eq!(
        team.max_concurrent_runs,
        DEFAULT_AGENT_TEAM_MAX_CONCURRENT_RUNS
    );
    let task = database
        .agent_task(&task_id)
        .expect("task lookup")
        .expect("Coordinator task");
    assert_eq!(task.team_id, team.id);
    let input = serde_json::from_str::<CoordinatorTaskInput>(&task.input_json)
        .expect("Coordinator task input");
    assert_eq!(input.queued_user_message_id, queued.user_message_id);
    assert_eq!(
        input.visible_assistant_message_id.as_deref(),
        Some(queued.assistant_message_id.as_str())
    );
    assert_eq!(input.visible_assistant_sequence, Some(1));
    assert_eq!(input.message, "Normal send");
    assert!(!input.collaboration_tools_enabled);
    let chat = database
        .chat(&queued.chat_id)
        .expect("chat lookup")
        .expect("queued chat");
    let chat_metadata =
        parse_json_value(&chat.metadata_json, "chat metadata").expect("chat metadata");
    assert_eq!(
        chat_metadata["queuedRun"]["assistantMessageId"],
        json!(queued.assistant_message_id)
    );
    assert_eq!(chat_metadata["queuedRun"]["assistantSequence"], json!(1));
    let user_message = database
        .message(&queued.user_message_id)
        .expect("user message lookup")
        .expect("queued user message");
    let user_metadata =
        parse_json_value(&user_message.metadata_json, "user metadata").expect("user metadata");
    assert_eq!(
        user_metadata["queuedRun"]["assistantMessageId"],
        json!(queued.assistant_message_id)
    );
    assert_eq!(user_metadata["queuedRun"]["assistantSequence"], json!(1));
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn queue_chat_message_defer_start_does_not_wake_scheduler() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-agent-deferred-queue-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-agent-deferred-queue-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let mut state = test_app_state(config, profile_dir.clone());
    let (scheduler, mut scheduler_rx) = AgentScheduler::new();
    state.agent_scheduler = scheduler;

    let queued = crate::http::chat::queue_chat_message(
        State(state.clone()),
        AxumPath(workspace_id.clone()),
        Json(QueueChatMessageRequest {
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: Some("provider".to_string()),
            thinking_level: None,
            skill_ids: None,
            message: "Deferred send".to_string(),
            team_mode_enabled: false,
            defer_start: true,
            attachments: Vec::new(),
        }),
    )
    .await
    .expect("queue deferred message")
    .0;

    assert!(queued.agent_task_id.is_some());
    assert!(
        timeout(Duration::from_millis(50), scheduler_rx.recv())
            .await
            .is_err()
    );

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    let task = database
        .agent_task(&queued.agent_task_id.expect("Agent task id"))
        .expect("task lookup")
        .expect("Coordinator task");
    assert_eq!(task.status, foco_agent::AgentTaskStatus::Queued);
    let input = serde_json::from_str::<CoordinatorTaskInput>(&task.input_json)
        .expect("Coordinator task input");
    assert!(input.defer_until_workspace_idle);
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn queue_chat_message_internal_marks_scheduled_origin() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-scheduled-queue-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-scheduled-queue-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let mut state = test_app_state(config, profile_dir.clone());
    let (scheduler, mut scheduler_rx) = AgentScheduler::new();
    state.agent_scheduler = scheduler;

    let queued = crate::http::chat::queue_chat_message_internal(
        &state,
        &workspace_id,
        crate::http::chat::QueueChatMessageInput {
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: Some("provider".to_string()),
            thinking_level: None,
            skill_ids: None,
            message: "Scheduled prompt".to_string(),
            team_mode_enabled: false,
            defer_start: false,
            attachments: Vec::new(),
            agent_definition_id: None,
            origin: crate::http::chat::QueuedChatMessageOrigin::ScheduledTask {
                task_id: "scheduled-task-test".to_string(),
                run_id: "scheduled-run-test".to_string(),
                trigger_reason: "scheduled".to_string(),
            },
        },
    )
    .await
    .expect("queue scheduled message");

    assert!(queued.agent_team_id.is_some());
    assert!(queued.agent_task_id.is_some());
    assert_eq!(scheduler_rx.recv().await, Some(()));

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    let chat = database
        .chat(&queued.chat_id)
        .expect("chat lookup")
        .expect("scheduled chat");
    let chat_metadata =
        parse_json_value(&chat.metadata_json, "chat metadata").expect("chat metadata");
    assert_eq!(chat_metadata["source"], json!("scheduled_task"));
    assert_eq!(
        chat_metadata["scheduledTaskId"],
        json!("scheduled-task-test")
    );
    assert_eq!(
        chat_metadata["scheduledTaskRunId"],
        json!("scheduled-run-test")
    );
    assert_eq!(chat_metadata["triggerReason"], json!("scheduled"));
    assert_eq!(
        chat_metadata["queuedRun"]["assistantMessageId"],
        json!(queued.assistant_message_id)
    );

    let user_message = database
        .message(&queued.user_message_id)
        .expect("user message lookup")
        .expect("scheduled user message");
    assert_eq!(user_message.content, "Scheduled prompt");
    let user_metadata =
        parse_json_value(&user_message.metadata_json, "user metadata").expect("user metadata");
    assert_eq!(user_metadata["source"], json!("scheduled_task"));
    assert_eq!(
        user_metadata["scheduledTaskRunId"],
        json!("scheduled-run-test")
    );
    assert_eq!(
        user_metadata["queuedRun"]["assistantMessageId"],
        json!(queued.assistant_message_id)
    );
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn scheduled_task_dispatch_queues_visible_chat_and_completes_one_shot() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-scheduled-dispatch-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-scheduled-dispatch-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let mut state = test_app_state(config, profile_dir.clone());
    let (agent_scheduler, mut agent_scheduler_rx) = AgentScheduler::new();
    state.agent_scheduler = agent_scheduler;
    let metadata_json = format!(
        r#"{{"workspaceId":"{workspace_id}","concurrencyPolicy":"skip_if_running","misfirePolicy":"catch_up_once"}}"#
    );

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        database
            .insert_scheduled_task(NewScheduledTask {
                id: "scheduled-task-due",
                title: "Due task",
                description: None,
                schedule_json: r#"{"type":"one_shot_at","run_at":"2020-01-01T00:00:00Z"}"#,
                action_json: r#"{"type":"agent_prompt","prompt":"Scheduled prompt","session_mode":"create_new_chat","model_id":"model","provider_id":"provider","skill_ids":[],"collaboration_tools_enabled":false}"#,
                status: "enabled",
                next_run_at: Some("2020-01-01T00:00:00Z"),
                metadata_json: Some(&metadata_json),
            })
            .expect("scheduled task insert");
    }

    crate::scheduled_tasks::scheduler::dispatch_due_scheduled_tasks(&state)
        .await
        .expect("dispatch due scheduled tasks");
    assert_eq!(agent_scheduler_rx.recv().await, Some(()));

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    let task = database
        .scheduled_task("scheduled-task-due")
        .expect("scheduled task lookup")
        .expect("scheduled task");
    assert_eq!(task.status, "completed");
    assert_eq!(task.next_run_at, None);

    let runs = database
        .scheduled_task_runs_for_task("scheduled-task-due")
        .expect("scheduled task runs");
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(run.status, "queued");
    assert_eq!(run.trigger_reason, "scheduled");
    let chat_id = run.chat_id.as_deref().expect("run chat id");
    let user_message_id = run.user_message_id.as_deref().expect("run user message id");
    assert!(run.agent_task_id.is_some());

    let user_message = database
        .message(user_message_id)
        .expect("user message lookup")
        .expect("scheduled user message");
    assert_eq!(user_message.chat_id, chat_id);
    assert_eq!(user_message.content, "Scheduled prompt");
    let user_metadata =
        parse_json_value(&user_message.metadata_json, "user metadata").expect("user metadata");
    assert_eq!(user_metadata["source"], json!("scheduled_task"));
    assert_eq!(
        user_metadata["scheduledTaskId"],
        json!("scheduled-task-due")
    );
    assert_eq!(user_metadata["scheduledTaskRunId"], json!(run.id));
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn scheduled_task_dispatch_ignores_paused_due_task() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-scheduled-paused-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-scheduled-paused-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let state = test_app_state(config, profile_dir.clone());
    let metadata_json = format!(
        r#"{{"workspaceId":"{workspace_id}","concurrencyPolicy":"skip_if_running","misfirePolicy":"catch_up_once"}}"#
    );

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        database
            .insert_scheduled_task(NewScheduledTask {
                id: "scheduled-task-paused",
                title: "Paused task",
                description: None,
                schedule_json: r#"{"type":"interval","every_seconds":60,"start_at":"2020-01-01T00:00:00Z"}"#,
                action_json: r#"{"type":"agent_prompt","prompt":"Paused prompt","session_mode":"create_new_chat","model_id":"model","provider_id":"provider","skill_ids":[],"collaboration_tools_enabled":false}"#,
                status: "paused",
                next_run_at: Some("2020-01-01T00:00:00Z"),
                metadata_json: Some(&metadata_json),
            })
            .expect("scheduled task insert");
    }

    crate::scheduled_tasks::scheduler::dispatch_due_scheduled_tasks(&state)
        .await
        .expect("dispatch due scheduled tasks");

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    assert!(
        database
            .scheduled_task_runs_for_task("scheduled-task-paused")
            .expect("scheduled task runs")
            .is_empty()
    );
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn scheduled_task_run_now_queues_manual_run_without_advancing_schedule() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-scheduled-run-now-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-scheduled-run-now-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let mut state = test_app_state(config, profile_dir.clone());
    let (agent_scheduler, mut agent_scheduler_rx) = AgentScheduler::new();
    state.agent_scheduler = agent_scheduler;
    let metadata_json = format!(
        r#"{{"workspaceId":"{workspace_id}","concurrencyPolicy":"skip_if_running","misfirePolicy":"catch_up_once"}}"#
    );

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        database
            .insert_scheduled_task(NewScheduledTask {
                id: "scheduled-task-run-now",
                title: "Run now task",
                description: None,
                schedule_json: r#"{"type":"interval","every_seconds":60,"start_at":"2099-01-01T00:00:00Z"}"#,
                action_json: r#"{"type":"agent_prompt","prompt":"Manual prompt","session_mode":"create_new_chat","model_id":"model","provider_id":"provider","skill_ids":[],"collaboration_tools_enabled":false}"#,
                status: "enabled",
                next_run_at: Some("2099-01-01T00:00:00Z"),
                metadata_json: Some(&metadata_json),
            })
            .expect("scheduled task insert");
    }

    let run = crate::scheduled_tasks::scheduler::run_scheduled_task_now(
        &state,
        &workspace_id,
        "scheduled-task-run-now",
    )
    .await
    .expect("run scheduled task now");
    assert_eq!(agent_scheduler_rx.recv().await, Some(()));
    assert_eq!(run.status, "queued");
    assert_eq!(run.trigger_reason, "manual");

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    let task = database
        .scheduled_task("scheduled-task-run-now")
        .expect("scheduled task lookup")
        .expect("scheduled task");
    assert_eq!(task.status, "enabled");
    assert_eq!(task.next_run_at.as_deref(), Some("2099-01-01T00:00:00Z"));
    assert!(task.last_run_at.is_some());
    let user_message = database
        .message(run.user_message_id.as_deref().expect("user message id"))
        .expect("user message lookup")
        .expect("user message");
    assert_eq!(user_message.content, "Manual prompt");
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn scheduled_task_reconciliation_dispatches_stale_pending_run() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-scheduled-reconcile-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-scheduled-reconcile-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let mut state = test_app_state(config, profile_dir.clone());
    let (agent_scheduler, mut agent_scheduler_rx) = AgentScheduler::new();
    state.agent_scheduler = agent_scheduler;
    let metadata_json = format!(
        r#"{{"workspaceId":"{workspace_id}","concurrencyPolicy":"skip_if_running","misfirePolicy":"catch_up_once"}}"#
    );

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        database
            .insert_scheduled_task(NewScheduledTask {
                id: "scheduled-task-reconcile",
                title: "Reconcile task",
                description: None,
                schedule_json: r#"{"type":"one_shot_at","run_at":"2020-01-01T00:00:00Z"}"#,
                action_json: r#"{"type":"agent_prompt","prompt":"Recovered prompt","session_mode":"create_new_chat","model_id":"model","provider_id":"provider","skill_ids":[],"collaboration_tools_enabled":false}"#,
                status: "completed",
                next_run_at: None,
                metadata_json: Some(&metadata_json),
            })
            .expect("scheduled task insert");
        database
            .insert_scheduled_task_run(NewScheduledTaskRun {
                id: "scheduled-run-reconcile",
                task_id: "scheduled-task-reconcile",
                trigger_reason: "scheduled",
                status: "pending",
                scheduled_at: "2020-01-01T00:00:00Z",
                queued_at: None,
                started_at: None,
                completed_at: None,
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
            .expect("stale scheduled run insert");
    }

    crate::scheduled_tasks::scheduler::reconcile_scheduled_task_runs(&state)
        .await
        .expect("reconcile scheduled task runs");
    assert_eq!(agent_scheduler_rx.recv().await, Some(()));

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    let run = database
        .scheduled_task_run("scheduled-run-reconcile")
        .expect("scheduled run lookup")
        .expect("scheduled run");
    assert_eq!(run.status, "queued");
    assert!(run.agent_task_id.is_some());
    let user_message = database
        .message(run.user_message_id.as_deref().expect("user message id"))
        .expect("user message lookup")
        .expect("user message");
    assert_eq!(user_message.content, "Recovered prompt");
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn scheduled_task_run_status_tracks_agent_task_lifecycle() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-scheduled-sync-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-scheduled-sync-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let mut state = test_app_state(config, profile_dir.clone());
    let (agent_scheduler, _agent_scheduler_rx) = AgentScheduler::new();
    state.agent_scheduler = agent_scheduler;
    let metadata_json = format!(
        r#"{{"workspaceId":"{workspace_id}","concurrencyPolicy":"skip_if_running","misfirePolicy":"catch_up_once"}}"#
    );

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        database
            .insert_scheduled_task(NewScheduledTask {
                id: "scheduled-task-sync",
                title: "Sync task",
                description: None,
                schedule_json: r#"{"type":"one_shot_at","run_at":"2020-01-01T00:00:00Z"}"#,
                action_json: r#"{"type":"agent_prompt","prompt":"Sync prompt","session_mode":"create_new_chat","model_id":"model","provider_id":"provider","skill_ids":[],"collaboration_tools_enabled":false}"#,
                status: "enabled",
                next_run_at: Some("2020-01-01T00:00:00Z"),
                metadata_json: Some(&metadata_json),
            })
            .expect("scheduled task insert");
    }

    crate::scheduled_tasks::scheduler::dispatch_due_scheduled_tasks(&state)
        .await
        .expect("dispatch due scheduled tasks");
    let run = WorkspaceDatabase::open_or_create(&workspace_dir)
        .expect("database")
        .scheduled_task_runs_for_task("scheduled-task-sync")
        .expect("scheduled task runs")
        .into_iter()
        .next()
        .expect("scheduled run");
    assert_eq!(run.status, "queued");
    let agent_task_id = run.agent_task_id.clone().expect("agent task id");
    let attempt_id =
        foco_agent::AgentAttemptId::new("agent-attempt-scheduled-sync").expect("attempt id");

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        let task = database
            .agent_task(&agent_task_id)
            .expect("agent task lookup")
            .expect("agent task");
        database
            .claim_runnable_agent_task(&task.team_id, &agent_task_id, &attempt_id)
            .expect("claim agent task")
            .expect("claimed agent task");
    }
    crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task(
        &workspace_dir,
        &agent_task_id,
    )
    .expect("sync running scheduled run");

    {
        let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        let running = database
            .scheduled_task_run(&run.id)
            .expect("scheduled run lookup")
            .expect("scheduled run");
        assert_eq!(running.status, "running");
        assert_eq!(running.agent_attempt_id.as_ref(), Some(&attempt_id));
        assert_eq!(
            running.active_run_id.as_deref(),
            Some(agent_task_id.as_str())
        );
        assert!(running.started_at.is_some());
    }

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        let task = database
            .agent_task(&agent_task_id)
            .expect("agent task lookup")
            .expect("agent task");
        database
            .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                team_id: &task.team_id,
                task_id: &task.id,
                expected_status: foco_agent::AgentTaskStatus::Running,
                transition: foco_agent::AgentTaskTransition::Complete,
                result_json: Some(r#"{"text":"done"}"#),
                error_json: None,
                interruption_reason: None,
            })
            .expect("complete agent task");
    }
    crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task(
        &workspace_dir,
        &agent_task_id,
    )
    .expect("sync completed scheduled run");

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    let completed = database
        .scheduled_task_run(&run.id)
        .expect("scheduled run lookup")
        .expect("scheduled run");
    assert_eq!(completed.status, "succeeded");
    assert_eq!(completed.agent_attempt_id.as_ref(), Some(&attempt_id));
    assert!(completed.completed_at.is_some());
    assert!(completed.chat_id.is_some());
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn scheduled_task_run_cancel_cancels_queued_agent_task() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-scheduled-cancel-queued-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-scheduled-cancel-queued-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let mut state = test_app_state(config, profile_dir.clone());
    let (agent_scheduler, _agent_scheduler_rx) = AgentScheduler::new();
    state.agent_scheduler = agent_scheduler;
    let metadata_json = format!(
        r#"{{"workspaceId":"{workspace_id}","concurrencyPolicy":"skip_if_running","misfirePolicy":"catch_up_once"}}"#
    );

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        database
            .insert_scheduled_task(NewScheduledTask {
                id: "scheduled-task-cancel-queued",
                title: "Cancel queued task",
                description: None,
                schedule_json: r#"{"type":"one_shot_at","run_at":"2020-01-01T00:00:00Z"}"#,
                action_json: r#"{"type":"agent_prompt","prompt":"Cancel queued prompt","session_mode":"create_new_chat","model_id":"model","provider_id":"provider","skill_ids":[],"collaboration_tools_enabled":false}"#,
                status: "enabled",
                next_run_at: Some("2020-01-01T00:00:00Z"),
                metadata_json: Some(&metadata_json),
            })
            .expect("scheduled task insert");
    }

    crate::scheduled_tasks::scheduler::dispatch_due_scheduled_tasks(&state)
        .await
        .expect("dispatch due scheduled tasks");
    let run = WorkspaceDatabase::open_or_create(&workspace_dir)
        .expect("database")
        .scheduled_task_runs_for_task("scheduled-task-cancel-queued")
        .expect("scheduled task runs")
        .into_iter()
        .next()
        .expect("scheduled run");
    let agent_task_id = run.agent_task_id.clone().expect("agent task id");

    let cancelled = crate::scheduled_tasks::scheduler::cancel_scheduled_task_run(
        &state,
        &workspace_id,
        &run.id,
    )
    .expect("cancel scheduled run");
    assert_eq!(cancelled.status, "cancelled");
    assert_eq!(
        cancelled.error_message.as_deref(),
        Some("cancelled explicitly")
    );
    assert!(cancelled.completed_at.is_some());

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    let task = database
        .agent_task(&agent_task_id)
        .expect("agent task lookup")
        .expect("agent task");
    assert_eq!(task.status, foco_agent::AgentTaskStatus::Cancelled);
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn scheduled_task_run_cancel_signals_running_active_run() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-scheduled-cancel-running-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-scheduled-cancel-running-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let mut state = test_app_state(config, profile_dir.clone());
    let (agent_scheduler, _agent_scheduler_rx) = AgentScheduler::new();
    state.agent_scheduler = agent_scheduler;
    let metadata_json = format!(
        r#"{{"workspaceId":"{workspace_id}","concurrencyPolicy":"skip_if_running","misfirePolicy":"catch_up_once"}}"#
    );

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        database
            .insert_scheduled_task(NewScheduledTask {
                id: "scheduled-task-cancel-running",
                title: "Cancel running task",
                description: None,
                schedule_json: r#"{"type":"one_shot_at","run_at":"2020-01-01T00:00:00Z"}"#,
                action_json: r#"{"type":"agent_prompt","prompt":"Cancel running prompt","session_mode":"create_new_chat","model_id":"model","provider_id":"provider","skill_ids":[],"collaboration_tools_enabled":false}"#,
                status: "enabled",
                next_run_at: Some("2020-01-01T00:00:00Z"),
                metadata_json: Some(&metadata_json),
            })
            .expect("scheduled task insert");
    }

    crate::scheduled_tasks::scheduler::dispatch_due_scheduled_tasks(&state)
        .await
        .expect("dispatch due scheduled tasks");
    let run = WorkspaceDatabase::open_or_create(&workspace_dir)
        .expect("database")
        .scheduled_task_runs_for_task("scheduled-task-cancel-running")
        .expect("scheduled task runs")
        .into_iter()
        .next()
        .expect("scheduled run");
    let agent_task_id = run.agent_task_id.clone().expect("agent task id");
    let attempt_id =
        foco_agent::AgentAttemptId::new("agent-attempt-scheduled-cancel").expect("attempt id");

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        let task = database
            .agent_task(&agent_task_id)
            .expect("agent task lookup")
            .expect("agent task");
        database
            .claim_runnable_agent_task(&task.team_id, &agent_task_id, &attempt_id)
            .expect("claim agent task")
            .expect("claimed agent task");
    }
    crate::scheduled_tasks::scheduler::sync_scheduled_task_runs_for_agent_task(
        &workspace_dir,
        &agent_task_id,
    )
    .expect("sync running scheduled run");

    let synced = WorkspaceDatabase::open_or_create(&workspace_dir)
        .expect("database")
        .scheduled_task_run(&run.id)
        .expect("scheduled run lookup")
        .expect("scheduled run");
    let (guidance_tx, _guidance_rx) = mpsc::unbounded_channel();
    let registration = state
        .active_chat_runs
        .register(
            agent_task_id.to_string(),
            workspace_id.clone(),
            synced.chat_id.clone().expect("chat id"),
            "assistant-scheduled-active".to_string(),
            1,
            Vec::new(),
            true,
            0,
            guidance_tx,
        )
        .expect("register active run");
    let cancellation_rx = registration.cancellation().subscribe();
    assert!(!*cancellation_rx.borrow());

    let cancelling = crate::scheduled_tasks::scheduler::cancel_scheduled_task_run(
        &state,
        &workspace_id,
        &run.id,
    )
    .expect("cancel running scheduled run");
    assert_eq!(cancelling.status, "running");
    assert!(*cancellation_rx.borrow());

    drop(registration);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn queue_chat_message_resumes_paused_coordinator_without_active_work() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-agent-resume-queue-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-agent-resume-queue-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let chat_id = "chat-paused-coordinator";
    let team_id = foco_agent::AgentTeamId::new("agent-team-paused-coordinator").expect("team id");
    let instance_id =
        foco_agent::AgentInstanceId::new("agent-instance-paused-coordinator").expect("instance id");
    let interrupted_task_id =
        foco_agent::AgentTaskId::new("agent-task-paused-coordinator-first").expect("task id");
    let attempt_id = foco_agent::AgentAttemptId::new("agent-attempt-paused-coordinator-first")
        .expect("attempt id");

    {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        database
            .insert_chat(chat_id, "Paused Coordinator")
            .expect("chat insert");
        let definition = AgentDefinitionSettings {
            id: AgentDefinitionId::new("agent-definition-paused-coordinator")
                .expect("definition id"),
            revision: 1,
            name: "Paused coordinator".to_string(),
            description: String::new(),
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            model_options: AgentModelOptions::default(),
            system_prompt: "Continue chats.".to_string(),
            allowed_tools: Vec::new(),
            max_instances: 1,
            allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
            permissions: AgentPermissions::default(),
        };
        database
            .create_agent_team(foco_store::workspace::NewAgentTeam {
                id: &team_id,
                chat_id,
                coordinator_instance_id: &instance_id,
                coordinator_definition: &definition,
                max_concurrent_runs: 1,
            })
            .expect("team create");
        database
            .enqueue_agent_task(foco_store::workspace::NewAgentTask {
                id: &interrupted_task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("enqueue interrupted task");
        database
            .claim_runnable_agent_task(&team_id, &interrupted_task_id, &attempt_id)
            .expect("claim task")
            .expect("claimed task");
        database
            .update_agent_task_state(foco_store::workspace::AgentTaskStateUpdate {
                team_id: &team_id,
                task_id: &interrupted_task_id,
                expected_status: foco_agent::AgentTaskStatus::Running,
                transition: foco_agent::AgentTaskTransition::Interrupt,
                result_json: None,
                error_json: Some(r#"{"message":"restart"}"#),
                interruption_reason: Some("restart"),
            })
            .expect("interrupt task");
        database
            .transition_agent_instance_status(&instance_id, foco_agent::AgentInstanceStatus::Paused)
            .expect("pause coordinator");
    }

    let mut state = test_app_state(config, profile_dir.clone());
    let (scheduler, mut scheduler_rx) = AgentScheduler::new();
    state.agent_scheduler = scheduler;
    let queued = crate::http::chat::queue_chat_message(
        State(state),
        AxumPath(workspace_id),
        Json(QueueChatMessageRequest {
            chat_id: Some(chat_id.to_string()),
            model_id: "model".to_string(),
            provider_id: Some("provider".to_string()),
            thinking_level: None,
            skill_ids: None,
            message: "Continue after restart".to_string(),
            team_mode_enabled: false,
            defer_start: false,
            attachments: Vec::new(),
        }),
    )
    .await
    .expect("queue follow-up message")
    .0;
    let new_task_id = queued.agent_task_id.expect("Agent task id");
    assert_eq!(scheduler_rx.recv().await, Some(()));

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    assert_eq!(
        database
            .agent_instance(&instance_id)
            .expect("instance")
            .expect("instance")
            .status,
        foco_agent::AgentInstanceStatus::Idle
    );
    assert_eq!(
        database
            .agent_task(&interrupted_task_id)
            .expect("interrupted task")
            .expect("interrupted task")
            .status,
        foco_agent::AgentTaskStatus::Interrupted
    );
    let new_task = database
        .agent_task(&new_task_id)
        .expect("new task")
        .expect("new task");
    assert_eq!(new_task.status, foco_agent::AgentTaskStatus::Queued);
    assert_eq!(new_task.sequence, 1);
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[test]
fn queued_team_runs_cleanup_only_their_own_uploaded_attachments() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-agent-upload-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let first = write_session_attachment_file(
        &workspace_dir,
        "chat-agent-upload",
        0,
        "attachment",
        "note.txt",
        "aGVsbG8=",
        5,
    )
    .expect("first upload");
    let second = write_session_attachment_file(
        &workspace_dir,
        "chat-agent-upload",
        0,
        "attachment",
        "note.txt",
        "aGVsbG8=",
        5,
    )
    .expect("second upload");
    assert_ne!(first, second);
    cleanup_chat_session_upload_files(
        &workspace_dir,
        "chat-agent-upload",
        std::slice::from_ref(&first),
    )
    .expect("cleanup first upload");
    assert!(!Path::new(&first).exists());
    assert!(Path::new(&second).is_file());
    cleanup_chat_session_upload_files(
        &workspace_dir,
        "chat-agent-upload",
        std::slice::from_ref(&second),
    )
    .expect("cleanup second upload");
    assert!(
        !chat_session_upload_dir(&workspace_dir, "chat-agent-upload")
            .expect("session dir")
            .exists()
    );
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn persist_chat_result_writes_audit_status_code_and_queues_memory_extraction() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-audit-status-code-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let team_id = foco_agent::AgentTeamId::new("agent-team-audit").expect("team id");
    let instance_id =
        foco_agent::AgentInstanceId::new("agent-instance-audit").expect("instance id");
    let task_id = foco_agent::AgentTaskId::new("agent-task-audit").expect("task id");
    let attempt_id = foco_agent::AgentAttemptId::new("agent-attempt-audit").expect("attempt id");
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        database
            .insert_chat("chat-1", "Status code chat")
            .expect("chat insert");
        database
            .upsert_message_content(NewMessage {
                id: "assistant-1",
                chat_id: "chat-1",
                role: "assistant",
                content: "D",
                sequence: 1,
                metadata_json: Some(r#"{"streamingState":"streaming"}"#),
            })
            .expect("streaming assistant draft insert");
        let definition = AgentDefinitionSettings {
            id: AgentDefinitionId::new("agent-definition-audit").expect("definition id"),
            revision: 1,
            name: "Audit agent".to_string(),
            description: String::new(),
            provider_id: "openai-responses".to_string(),
            model_id: "gpt-5.4".to_string(),
            model_options: AgentModelOptions::default(),
            system_prompt: "Be precise.".to_string(),
            allowed_tools: Vec::new(),
            max_instances: 1,
            allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
            permissions: AgentPermissions::default(),
        };
        database
            .create_agent_team(foco_store::workspace::NewAgentTeam {
                id: &team_id,
                chat_id: "chat-1",
                coordinator_instance_id: &instance_id,
                coordinator_definition: &definition,
                max_concurrent_runs: 1,
            })
            .expect("agent team create");
        database
            .enqueue_agent_task(foco_store::workspace::NewAgentTask {
                id: &task_id,
                team_id: &team_id,
                owner_instance_id: &instance_id,
                origin_instance_id: None,
                parent_task_id: None,
                input_json: "{}",
            })
            .expect("agent task enqueue");
        database
            .claim_runnable_agent_task(&team_id, &task_id, &attempt_id)
            .expect("agent task claim")
            .expect("claimed agent task");
    }
    let (_app_shutdown_tx, app_shutdown_rx) = watch::channel(false);
    let mcp_registry = Arc::new(McpRegistry::default());
    let context = PreparedChatContext {
        workspace_id: "workspace-1".to_string(),
        workspace_path: workspace_dir.clone(),
        tool_workspace_path: workspace_dir.clone(),
        memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        provider_id: "openai-responses".to_string(),
        model_id: "gpt-5.4".to_string(),
        user_message_id: "user-1".to_string(),
        queued_user_message_id: None,
        assistant_message_id: "assistant-1".to_string(),
        llm_request_id: "request-1".to_string(),
        assistant_sequence: 1,
        agent_associations: AgentRunAssociations {
            team_id: Some(team_id.clone()),
            instance_id: Some(instance_id.clone()),
            task_id: Some(task_id.clone()),
            attempt_id: Some(attempt_id.clone()),
        },
        agent_definition_snapshot: None,
        agent_task_input: None,
        agent_unread_messages: Vec::new(),
        agent_allowed_tools: None,
        agent_tool_context: None,
        agent_primary_chat_output: true,
        session_upload_paths: None,
        provider_config: ProviderConnectionConfig {
            kind: test_provider_kind(),
            base_url: None,
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        },
        provider_request: NeutralChatRequest {
            model_id: "gpt-5.4".to_string(),
            messages: vec![neutral_text_message(
                NeutralChatRole::User,
                "Hello".to_string(),
            )],
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: Some(16),
            prompt_cache_key: None,
            prompt_cache_retention: None,
        },
        hook_runtime: HookRuntime::new(mcp_registry.clone()),
        global_hooks: HookConfig::default(),
        mcp_registry,
        question_registry: QuestionRegistry::default(),
        tool_resource_locks: ToolResourceLockRegistry::default(),
        app_shutdown_rx,
        context_budget: foco_agent::ContextBudget {
            context_window: 1_000,
            max_output_tokens: 16,
            system_prompt_tokens: 0,
            tool_schema_tokens: 0,
            safety_tokens: 0,
            available_message_tokens: 984,
        },
        global_config: GlobalConfig::first_run(workspace_dir.clone()),
        memory_settings: MemorySettings {
            enabled: true,
            extraction_mode: "pending_review".to_string(),
            retrieval_mode: "fts".to_string(),
            retention_days: None,
            extraction_model_id: Some("extract-model".to_string()),
            retrieval_model_id: None,
            dream: MemoryDreamSettings::default(),
        },
        memories_used: Vec::new(),
        memory_target_status: MemoryStatus::Pending,
        request_body_json: "{}".to_string(),
        captured_llm_requests: Vec::new(),
        compression_snapshots: Vec::new(),
        message_source_sequences: vec![Some(0)],
        message_context_sources: vec![PromptContextSource::StoredMessage { sequence: 0 }],
        active_tool_start_index: 1,
        next_runtime_tool_batch_index: 0,
        hook_context_messages: Vec::new(),
        hook_notifications: Vec::new(),
        code_change_baseline: SessionCodeChangeBaselineState::Unavailable {
            reason: "test baseline unavailable".to_string(),
        },
        code_change_stats: CodeChangeStats::default(),
        pending_memory_retrieval: None,
    };
    let outcome = ChatAuditOutcome {
        first_token_at: Some("2026-06-06T09:00:00Z".to_string()),
        completed_at: "2026-06-06T09:00:01Z".to_string(),
        first_token_latency_ms: Some(100),
        total_latency_ms: 1_000,
        input_tokens: Some(10),
        output_tokens: Some(5),
        cache_read_tokens: Some(0),
        cache_write_tokens: Some(0),
        status_code: Some(200),
        final_state: "succeeded",
        response_body_json: Some(r#"{"text":"Done."}"#.to_string()),
    };
    let event = captured_event(&ChatSseEvent::Complete {
        chat_id: "chat-1".to_string(),
        assistant_message_id: "assistant-1".to_string(),
        text: "Done.".to_string(),
        reasoning: None,
        usage: None,
        stop_reason: Some("stop".to_string()),
        metrics: ChatReplyMetrics {
            model_id: "gpt-5.4".to_string(),
            provider_id: "openai-responses".to_string(),
            total_latency_ms: Some(1_000),
            first_token_latency_ms: Some(100),
            output_tokens: Some(5),
        },
        memories_used: Vec::new(),
    });

    persist_running_llm_request(&context, "request-1", "2026-06-06T09:00:00Z", "{}", &[])
        .expect("persist running LLM request");
    persist_chat_result(
        &context,
        "2026-06-06T09:00:00Z",
        outcome,
        &[event],
        Some("Done."),
        None,
        &[],
    )
    .expect("persist chat result");

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let request = database
        .llm_request("request-1")
        .expect("llm request read")
        .expect("llm request");

    assert_eq!(request.status_code, Some(200));
    assert_eq!(request.agent_team_id, Some(team_id));
    assert_eq!(request.agent_instance_id, Some(instance_id));
    assert_eq!(request.agent_task_id, Some(task_id));
    assert_eq!(request.agent_attempt_id, Some(attempt_id));
    let assistant = database
        .messages_for_chat("chat-1")
        .expect("chat messages read")
        .into_iter()
        .find(|message| message.id == "assistant-1")
        .expect("assistant message");
    let metadata = parse_json_value(&assistant.metadata_json, "assistant metadata")
        .expect("assistant metadata json");
    assert_eq!(assistant.content, "Done.");
    assert!(metadata.get("streamingState").is_none());

    let memory_database =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
    let jobs = memory_database
        .extraction_jobs_for_scope(Some("chat-1"), Some(MemoryExtractionJobStatus::Queued), 10)
        .expect("memory extraction jobs");

    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].model_id.as_deref(), Some("extract-model"));
    assert!(jobs[0].input_json.contains("\"targetStatus\":\"pending\""));
    assert!(
        jobs[0]
            .input_json
            .contains("\"assistantMessageId\":\"assistant-1\"")
    );

    drop(database);
    drop(memory_database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn persist_chat_result_clears_completed_queued_run_metadata() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-queued-run-clear-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        database
            .insert_chat_with_metadata(
                "chat-1",
                "Queued chat",
                r#"{"queuedRun":{"status":"running","userMessageId":"user-1","assistantMessageId":"assistant-1","modelId":"gpt-5.4","providerId":"openai-responses","content":"Hello"}}"#,
            )
            .expect("chat insert");
        database
            .insert_message(NewMessage {
                id: "user-1",
                chat_id: "chat-1",
                role: "user",
                content: "Hello",
                sequence: 0,
                metadata_json: Some(
                    r#"{"queuedRun":{"status":"running","assistantMessageId":"assistant-1","modelId":"gpt-5.4","providerId":"openai-responses"}}"#,
                ),
            })
            .expect("message insert");
    }
    let mut context = test_prepared_chat_context(
        workspace_dir.clone(),
        vec![neutral_text_message(
            NeutralChatRole::User,
            "Hello".to_string(),
        )],
        vec![Some(0)],
        vec![PromptContextSource::StoredMessage { sequence: 0 }],
        984,
    );
    context.queued_user_message_id = Some("user-1".to_string());
    let outcome = ChatAuditOutcome {
        first_token_at: Some("2026-06-06T09:00:00Z".to_string()),
        completed_at: "2026-06-06T09:00:01Z".to_string(),
        first_token_latency_ms: Some(100),
        total_latency_ms: 1_000,
        input_tokens: Some(10),
        output_tokens: Some(5),
        cache_read_tokens: Some(0),
        cache_write_tokens: Some(0),
        status_code: Some(200),
        final_state: "succeeded",
        response_body_json: Some(r#"{"text":"Done."}"#.to_string()),
    };

    persist_chat_result(
        &context,
        "2026-06-06T09:00:00Z",
        outcome,
        &[],
        Some("Done."),
        None,
        &[],
    )
    .expect("persist chat result");

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let chat_metadata = parse_json_value(
        &database
            .chat("chat-1")
            .expect("chat read")
            .expect("chat")
            .metadata_json,
        "chat metadata",
    )
    .expect("chat metadata json");
    let message_metadata = parse_json_value(
        &database
            .message("user-1")
            .expect("message read")
            .expect("message")
            .metadata_json,
        "message metadata",
    )
    .expect("message metadata json");

    assert!(chat_metadata.get("queuedRun").is_none());
    assert!(message_metadata.get("queuedRun").is_none());

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn chat_summary_clears_stale_pending_run_without_resumable_agent_task() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-stale-chat-queue-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");

    for status in ["queued", "running"] {
        let chat_id = format!("chat-{status}");
        let user_message_id = format!("user-{status}");
        let chat_metadata_json = format!(
            r#"{{"queuedRun":{{"status":"{status}","userMessageId":"{user_message_id}","modelId":"gpt-5.4","providerId":"openai-responses","content":"Hello"}}}}"#
        );
        database
            .insert_chat_with_metadata(&chat_id, "Stale queued chat", &chat_metadata_json)
            .expect("chat insert");

        let chat = database.chat(&chat_id).expect("chat read").expect("chat");
        let summary = chat_summary(&mut database, chat, CodeChangeStats::default(), None)
            .expect("chat summary");
        let chat_metadata = parse_json_value(
            &database
                .chat(&chat_id)
                .expect("chat read")
                .expect("chat")
                .metadata_json,
            "chat metadata",
        )
        .expect("chat metadata json");

        assert!(summary.queued_run.is_none());
        assert!(chat_metadata.get("queuedRun").is_none());
    }

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn chat_message_summaries_clear_stale_pending_message_without_resumable_agent_task() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-stale-message-queue-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");

    for status in ["queued", "running"] {
        let chat_id = format!("chat-{status}");
        let user_message_id = format!("user-{status}");
        let message_metadata_json = format!(
            r#"{{"queuedRun":{{"status":"{status}","modelId":"gpt-5.4","providerId":"openai-responses"}}}}"#
        );
        database
            .insert_chat(&chat_id, "Stale queued message")
            .expect("chat insert");
        database
            .insert_message(NewMessage {
                id: &user_message_id,
                chat_id: &chat_id,
                role: "user",
                content: "Hello",
                sequence: 0,
                metadata_json: Some(&message_metadata_json),
            })
            .expect("message insert");

        let messages = database
            .messages_for_chat(&chat_id)
            .expect("messages for chat");
        let summaries =
            chat_message_summaries(&mut database, &workspace_dir, None, &chat_id, messages)
                .expect("message summaries");
        let message_metadata = parse_json_value(
            &database
                .message(&user_message_id)
                .expect("message read")
                .expect("message")
                .metadata_json,
            "message metadata",
        )
        .expect("message metadata json");

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].pending_mode, None);
        assert!(summaries[0].queued_run.is_none());
        assert!(message_metadata.get("queuedRun").is_none());
    }

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[tokio::test]
async fn chat_messages_return_memory_dream_transcript_steps_as_read_only_chat() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-dream-transcript-messages-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-dream-transcript-messages-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(profile_dir.join(".foco")).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace_id = config.workspaces[0].id.clone();
    let state = test_app_state(config, profile_dir.clone());
    let chat_id = "dream-transcript-chat";
    let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    database
        .insert_chat_with_metadata(
            chat_id,
            "Memory Dream: global manual",
            &json!({ "kind": foco_store::memory::MEMORY_DREAM_TRANSCRIPT_CHAT_KIND }).to_string(),
        )
        .expect("transcript chat insert");
    database
        .insert_message(NewMessage {
            id: "dream-transcript-step",
            chat_id,
            role: "system",
            content: "job started",
            sequence: 0,
            metadata_json: Some(&json!({ "kind": "memory_dream_transcript_step" }).to_string()),
        })
        .expect("transcript message insert");
    drop(database);

    let Json(response) = crate::http::chat::chat_messages(
        State(state),
        AxumPath((workspace_id, chat_id.to_string())),
    )
    .await
    .expect("chat messages response");

    let chat = response.chat.expect("chat metadata");
    assert_eq!(chat.id, chat_id);
    assert_eq!(chat.title, "Memory Dream: global manual");
    assert_eq!(
        chat.kind.as_deref(),
        Some(foco_store::memory::MEMORY_DREAM_TRANSCRIPT_CHAT_KIND)
    );
    assert!(chat.read_only);
    assert_eq!(response.messages.len(), 1);
    assert_eq!(response.messages[0].id, "dream-transcript-step");
    assert_eq!(response.messages[0].role, "assistant");
    assert_eq!(response.messages[0].content, "job started");

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[test]
fn chat_message_summaries_keep_normal_system_messages_hidden() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-normal-system-hidden-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    let chat_id = "normal-chat";
    database
        .insert_chat(chat_id, "Normal chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "system-message",
            chat_id,
            role: "system",
            content: "internal instruction",
            sequence: 0,
            metadata_json: Some(&json!({ "kind": "memory_dream_transcript_step" }).to_string()),
        })
        .expect("system message insert");

    let messages = database
        .messages_for_chat(chat_id)
        .expect("messages for chat");
    let summaries = chat_message_summaries(&mut database, &workspace_dir, None, chat_id, messages)
        .expect("message summaries");

    assert!(summaries.is_empty());

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn queued_run_stays_visible_while_coordinator_task_is_waiting() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-waiting-queue-summary-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
    let chat_id = "chat-waiting-queued-run";
    let user_message_id = "user-waiting-queued-run";
    let chat_metadata_json = format!(
        r#"{{"queuedRun":{{"status":"running","userMessageId":"{user_message_id}","modelId":"gpt-5.4","providerId":"openai-responses","content":"Hello"}}}}"#
    );
    let message_metadata_json =
        r#"{"queuedRun":{"status":"running","modelId":"gpt-5.4","providerId":"openai-responses"}}"#;

    database
        .insert_chat_with_metadata(chat_id, "Waiting queued run", &chat_metadata_json)
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: user_message_id,
            chat_id,
            role: "user",
            content: "Hello",
            sequence: 0,
            metadata_json: Some(message_metadata_json),
        })
        .expect("message insert");
    insert_waiting_coordinator_task(
        &mut database,
        chat_id,
        user_message_id,
        "waiting-queue-summary",
    );

    let chat = database.chat(chat_id).expect("chat read").expect("chat");
    let summary =
        chat_summary(&mut database, chat, CodeChangeStats::default(), None).expect("chat summary");
    let messages = database
        .messages_for_chat(chat_id)
        .expect("messages for chat");
    let summaries = chat_message_summaries(&mut database, &workspace_dir, None, chat_id, messages)
        .expect("message summaries");
    let chat_metadata = parse_json_value(
        &database
            .chat(chat_id)
            .expect("chat read")
            .expect("chat")
            .metadata_json,
        "chat metadata",
    )
    .expect("chat metadata json");
    let message_metadata = parse_json_value(
        &database
            .message(user_message_id)
            .expect("message read")
            .expect("message")
            .metadata_json,
        "message metadata",
    )
    .expect("message metadata json");

    assert!(summary.queued_run.is_some());
    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].queued_run.is_some());
    assert!(chat_metadata.get("queuedRun").is_some());
    assert!(message_metadata.get("queuedRun").is_some());

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[tokio::test]
async fn team_chat_task_sse_stays_open_while_coordinator_task_is_waiting() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-waiting-team-stream-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-waiting-team-stream-profile"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&profile_dir).expect("profile directory");

    let config = prompt_test_config(workspace_dir.clone());
    let workspace = config.workspaces[0].clone();
    let chat_id = "chat-waiting-team-stream";
    let user_message_id = "user-waiting-team-stream";
    let task_id = {
        let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("database");
        database
            .insert_chat(chat_id, "Waiting team stream")
            .expect("chat insert");
        insert_waiting_coordinator_task(
            &mut database,
            chat_id,
            user_message_id,
            "waiting-team-stream",
        )
    };

    let state = test_app_state(config, profile_dir.clone());
    let response = crate::http::chat::team_chat_task_sse(&state, &workspace, &task_id)
        .await
        .expect("waiting Coordinator stream response");
    let body = response.into_response().into_body();
    let completed = timeout(Duration::from_millis(200), to_bytes(body, usize::MAX)).await;

    assert!(
        completed.is_err(),
        "waiting Coordinator stream should stay open instead of sending StreamEnd"
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[test]
fn persist_chat_result_writes_each_captured_llm_request() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-multi-audit-request-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        database
            .insert_chat("chat-1", "Multi audit chat")
            .expect("chat insert");
    }
    let (_app_shutdown_tx, app_shutdown_rx) = watch::channel(false);
    let mcp_registry = Arc::new(McpRegistry::default());
    let context = PreparedChatContext {
        workspace_id: "workspace-1".to_string(),
        workspace_path: workspace_dir.clone(),
        tool_workspace_path: workspace_dir.clone(),
        memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        provider_id: "openai-responses".to_string(),
        model_id: "gpt-5.4".to_string(),
        user_message_id: "user-1".to_string(),
        queued_user_message_id: None,
        assistant_message_id: "assistant-1".to_string(),
        llm_request_id: "run-1".to_string(),
        assistant_sequence: 1,
        agent_associations: AgentRunAssociations::default(),
        agent_definition_snapshot: None,
        agent_task_input: None,
        agent_unread_messages: Vec::new(),
        agent_allowed_tools: None,
        agent_tool_context: None,
        agent_primary_chat_output: true,
        session_upload_paths: None,
        provider_config: ProviderConnectionConfig {
            kind: test_provider_kind(),
            base_url: None,
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        },
        provider_request: NeutralChatRequest {
            model_id: "gpt-5.4".to_string(),
            messages: vec![neutral_text_message(
                NeutralChatRole::User,
                "Hello".to_string(),
            )],
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: Some(16),
            prompt_cache_key: None,
            prompt_cache_retention: None,
        },
        hook_runtime: HookRuntime::new(mcp_registry.clone()),
        global_hooks: HookConfig::default(),
        mcp_registry,
        question_registry: QuestionRegistry::default(),
        tool_resource_locks: ToolResourceLockRegistry::default(),
        app_shutdown_rx,
        context_budget: foco_agent::ContextBudget {
            context_window: 1_000,
            max_output_tokens: 16,
            system_prompt_tokens: 0,
            tool_schema_tokens: 0,
            safety_tokens: 0,
            available_message_tokens: 984,
        },
        global_config: GlobalConfig::first_run(workspace_dir.clone()),
        memory_settings: MemorySettings {
            enabled: false,
            extraction_mode: "manual".to_string(),
            retrieval_mode: "fts".to_string(),
            retention_days: None,
            extraction_model_id: None,
            retrieval_model_id: None,
            dream: MemoryDreamSettings::default(),
        },
        memories_used: Vec::new(),
        memory_target_status: MemoryStatus::Pending,
        request_body_json: "{}".to_string(),
        captured_llm_requests: vec![
            captured_test_llm_request("request-1", "assistant-1", 10, 1_000),
            captured_test_llm_request("request-2", "assistant-1", 20, 1_500),
        ],
        compression_snapshots: Vec::new(),
        message_source_sequences: vec![Some(0)],
        message_context_sources: vec![PromptContextSource::StoredMessage { sequence: 0 }],
        active_tool_start_index: 1,
        next_runtime_tool_batch_index: 0,
        hook_context_messages: Vec::new(),
        hook_notifications: Vec::new(),
        code_change_baseline: SessionCodeChangeBaselineState::Unavailable {
            reason: "test baseline unavailable".to_string(),
        },
        code_change_stats: CodeChangeStats::default(),
        pending_memory_retrieval: None,
    };
    let outcome = ChatAuditOutcome {
        first_token_at: Some("2026-06-06T09:00:00Z".to_string()),
        completed_at: "2026-06-06T09:00:02Z".to_string(),
        first_token_latency_ms: Some(100),
        total_latency_ms: 2_500,
        input_tokens: Some(30),
        output_tokens: Some(30),
        cache_read_tokens: Some(0),
        cache_write_tokens: Some(0),
        status_code: Some(200),
        final_state: "succeeded",
        response_body_json: Some(r#"{"text":"Done."}"#.to_string()),
    };

    persist_chat_result(
        &context,
        "2026-06-06T09:00:00Z",
        outcome,
        &[],
        Some("Done."),
        None,
        &[],
    )
    .expect("persist chat result");

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    assert!(
        database
            .llm_request("run-1")
            .expect("run audit lookup")
            .is_none()
    );
    let single_agent_request = database
        .llm_request("request-1")
        .expect("first request lookup")
        .expect("first request");
    assert_eq!(single_agent_request.agent_team_id, None);
    assert_eq!(single_agent_request.agent_instance_id, None);
    assert_eq!(single_agent_request.agent_task_id, None);
    assert_eq!(single_agent_request.agent_attempt_id, None);
    assert!(
        database
            .llm_request("request-2")
            .expect("second request lookup")
            .is_some()
    );

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn persist_chat_result_for_worker_skips_main_chat_and_memory_extraction() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-worker-private-output-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        database
            .insert_chat("chat-1", "Worker private chat")
            .expect("chat insert");
    }
    let mut context = test_prepared_chat_context(
        workspace_dir.clone(),
        vec![neutral_text_message(
            NeutralChatRole::User,
            "Worker input".to_string(),
        )],
        vec![Some(0)],
        vec![PromptContextSource::StoredMessage { sequence: 0 }],
        984,
    );
    context.agent_primary_chat_output = false;
    context.memory_settings = MemorySettings {
        enabled: true,
        extraction_mode: "pending_review".to_string(),
        retrieval_mode: "fts".to_string(),
        retention_days: None,
        extraction_model_id: Some("extract-model".to_string()),
        retrieval_model_id: None,
        dream: MemoryDreamSettings::default(),
    };
    let outcome = ChatAuditOutcome {
        first_token_at: Some("2026-06-06T09:00:00Z".to_string()),
        completed_at: "2026-06-06T09:00:01Z".to_string(),
        first_token_latency_ms: Some(100),
        total_latency_ms: 1_000,
        input_tokens: Some(10),
        output_tokens: Some(5),
        cache_read_tokens: Some(0),
        cache_write_tokens: Some(0),
        status_code: Some(200),
        final_state: "succeeded",
        response_body_json: Some(r#"{"text":"Private result."}"#.to_string()),
    };

    persist_chat_result(
        &context,
        "2026-06-06T09:00:00Z",
        outcome,
        &[],
        Some("Private result."),
        None,
        &[],
    )
    .expect("persist worker private chat result");

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    assert!(
        database
            .messages_for_chat("chat-1")
            .expect("chat messages read")
            .is_empty()
    );
    assert!(
        database
            .llm_request("request-1")
            .expect("llm request read")
            .is_some()
    );
    let memory_database =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
    assert!(
        memory_database
            .extraction_jobs_for_scope(Some("chat-1"), Some(MemoryExtractionJobStatus::Queued), 10)
            .expect("memory extraction jobs")
            .is_empty()
    );

    drop(database);
    drop(memory_database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn persist_chat_result_writes_cancelled_captured_llm_request() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-cancelled-audit-request-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        database
            .insert_chat("chat-1", "Cancelled audit chat")
            .expect("chat insert");
    }
    let mut context = test_prepared_chat_context(
        workspace_dir.clone(),
        vec![neutral_text_message(
            NeutralChatRole::User,
            "Hello".to_string(),
        )],
        vec![Some(0)],
        vec![PromptContextSource::StoredMessage { sequence: 0 }],
        984,
    );
    context.llm_request_id = "run-1".to_string();
    let turn_events = vec![CapturedAuditEvent {
        event_at: "2026-06-06T09:00:00Z".to_string(),
        event_type: "start".to_string(),
        normalized_event_json: json!({
            "type": "start",
            "chatId": "chat-1",
            "userMessageId": "user-1",
            "assistantMessageId": "assistant-1",
            "llmRequestId": "llm-cancelled",
            "runId": "run-1",
            "turnIndex": 1,
        })
        .to_string(),
    }];
    context.captured_llm_requests.push(CapturedLlmRequest {
        id: "llm-succeeded".to_string(),
        request_started_at: "2026-06-06T08:59:00Z".to_string(),
        request_body_json: "{}".to_string(),
        events: Vec::new(),
        outcome: ChatAuditOutcome {
            first_token_at: Some("2026-06-06T08:59:00Z".to_string()),
            completed_at: "2026-06-06T08:59:01Z".to_string(),
            first_token_latency_ms: Some(100),
            total_latency_ms: 1_000,
            input_tokens: Some(10),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            status_code: Some(200),
            final_state: "succeeded",
            response_body_json: Some("{}".to_string()),
        },
    });
    context.capture_cancelled_llm_request(
        "llm-cancelled",
        "2026-06-06T09:00:00Z",
        r#"{"model":"gpt-5.4"}"#,
        &turn_events,
        Instant::now(),
        "chat run cancelled",
    );
    let outcome = cancelled_audit_outcome(Instant::now(), "chat run cancelled");

    persist_chat_result(
        &context,
        "2026-06-06T08:59:00Z",
        outcome,
        &[],
        None,
        None,
        &[],
    )
    .expect("persist cancelled chat result");

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    assert!(
        database
            .llm_request("run-1")
            .expect("run audit lookup")
            .is_none()
    );
    let cancelled_request = database
        .llm_request("llm-cancelled")
        .expect("cancelled request lookup")
        .expect("cancelled request");
    assert_eq!(cancelled_request.final_state, "cancelled");
    assert!(
        cancelled_request
            .response_body_json
            .as_deref()
            .expect("cancelled response json")
            .contains("chat run cancelled")
    );
    assert_eq!(
        database
            .llm_request_audit_count(LlmRequestAuditFilters {
                final_state: Some("cancelled"),
                ..LlmRequestAuditFilters::default()
            })
            .expect("cancelled audit count"),
        1
    );

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn persist_failed_chat_result_keeps_tool_calls_linked_to_assistant_message() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-failed-tool-call-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        database
            .insert_chat("chat-1", "Failed tool chat")
            .expect("chat insert");
    }
    let (_app_shutdown_tx, app_shutdown_rx) = watch::channel(false);
    let mcp_registry = Arc::new(McpRegistry::default());
    let context = PreparedChatContext {
        workspace_id: "workspace-1".to_string(),
        workspace_path: workspace_dir.clone(),
        tool_workspace_path: workspace_dir.clone(),
        memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        provider_id: "openai-responses".to_string(),
        model_id: "gpt-5.4".to_string(),
        user_message_id: "user-1".to_string(),
        queued_user_message_id: None,
        assistant_message_id: "assistant-1".to_string(),
        llm_request_id: "request-1".to_string(),
        assistant_sequence: 1,
        agent_associations: AgentRunAssociations::default(),
        agent_definition_snapshot: None,
        agent_task_input: None,
        agent_unread_messages: Vec::new(),
        agent_allowed_tools: None,
        agent_tool_context: None,
        agent_primary_chat_output: true,
        session_upload_paths: None,
        provider_config: ProviderConnectionConfig {
            kind: test_provider_kind(),
            base_url: None,
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        },
        provider_request: NeutralChatRequest {
            model_id: "gpt-5.4".to_string(),
            messages: vec![neutral_text_message(
                NeutralChatRole::User,
                "Hello".to_string(),
            )],
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: Some(16),
            prompt_cache_key: None,
            prompt_cache_retention: None,
        },
        hook_runtime: HookRuntime::new(mcp_registry.clone()),
        global_hooks: HookConfig::default(),
        mcp_registry,
        question_registry: QuestionRegistry::default(),
        tool_resource_locks: ToolResourceLockRegistry::default(),
        app_shutdown_rx,
        context_budget: foco_agent::ContextBudget {
            context_window: 1_000,
            max_output_tokens: 16,
            system_prompt_tokens: 0,
            tool_schema_tokens: 0,
            safety_tokens: 0,
            available_message_tokens: 984,
        },
        global_config: GlobalConfig::first_run(workspace_dir.clone()),
        memory_settings: MemorySettings {
            enabled: false,
            extraction_mode: "manual".to_string(),
            retrieval_mode: "fts".to_string(),
            retention_days: None,
            extraction_model_id: None,
            retrieval_model_id: None,
            dream: MemoryDreamSettings::default(),
        },
        memories_used: Vec::new(),
        memory_target_status: MemoryStatus::Pending,
        request_body_json: "{}".to_string(),
        captured_llm_requests: Vec::new(),
        compression_snapshots: Vec::new(),
        message_source_sequences: vec![Some(0)],
        message_context_sources: vec![PromptContextSource::StoredMessage { sequence: 0 }],
        active_tool_start_index: 1,
        next_runtime_tool_batch_index: 0,
        hook_context_messages: Vec::new(),
        hook_notifications: Vec::new(),
        code_change_baseline: SessionCodeChangeBaselineState::Unavailable {
            reason: "test baseline unavailable".to_string(),
        },
        code_change_stats: CodeChangeStats::default(),
        pending_memory_retrieval: None,
    };
    let outcome = ChatAuditOutcome {
        first_token_at: Some("2026-06-06T09:00:00Z".to_string()),
        completed_at: "2026-06-06T09:00:01Z".to_string(),
        first_token_latency_ms: Some(100),
        total_latency_ms: 1_000,
        input_tokens: Some(10),
        output_tokens: Some(5),
        cache_read_tokens: Some(0),
        cache_write_tokens: Some(0),
        status_code: None,
        final_state: "failed",
        response_body_json: Some(
            r#"{"error":"agent run exceeded 128 tool continuation rounds"}"#.to_string(),
        ),
    };
    let event = captured_event(&ChatSseEvent::Error {
        message: "agent run exceeded 128 tool continuation rounds".to_string(),
    });
    let tool_calls = vec![ExecutedToolCall {
        id: "tool-call-1".to_string(),
        name: "read_file".to_string(),
        input: json!({ "path": "README.md" }),
        output: json!({ "content": "hello" }),
        is_error: false,
        started_at: "2026-06-06T09:00:00Z".to_string(),
        completed_at: "2026-06-06T09:00:01Z".to_string(),
    }];

    persist_chat_result(
        &context,
        "2026-06-06T09:00:00Z",
        outcome,
        &[event],
        None,
        None,
        &tool_calls,
    )
    .expect("failed chat result with tool calls should persist");

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let request = database
        .llm_request("request-1")
        .expect("llm request read")
        .expect("llm request");
    let messages = database
        .messages_for_chat("chat-1")
        .expect("chat messages read");
    let tool_calls = database
        .tool_calls_for_chat("chat-1")
        .expect("chat tool calls read");

    assert_eq!(request.final_state, "failed");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].id, "assistant-1");
    assert!(messages[0].content.is_empty());
    let metadata = parse_json_value(&messages[0].metadata_json, "assistant metadata")
        .expect("assistant metadata json");
    assert_eq!(metadata["streamingState"], "failed");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].message_id.as_deref(), Some("assistant-1"));

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn active_chat_run_subscription_replays_cached_events_after_sequence() {
    let registry = ActiveChatRunRegistry::default();
    let (guidance_tx, _guidance_rx) = mpsc::unbounded_channel();
    let workspace_dir = env::temp_dir().join(unique_id("foco-active-run-replay-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        database
            .insert_chat("chat-1", "Active run")
            .expect("chat insert");
    }
    let mut registration = registry
        .register(
            "run-1".to_string(),
            "workspace-1".to_string(),
            "chat-1".to_string(),
            "assistant-1".to_string(),
            1,
            Vec::new(),
            true,
            0,
            guidance_tx,
        )
        .expect("register active run");
    let first_event = ChatSseEvent::TextDelta {
        assistant_message_id: "assistant-1".to_string(),
        delta: "hello".to_string(),
    };
    let second_event = ChatSseEvent::TextDelta {
        assistant_message_id: "assistant-1".to_string(),
        delta: " world".to_string(),
    };
    registration
        .record_event(&workspace_dir, "chat-1", &first_event)
        .expect("record first event");
    registration
        .record_event(&workspace_dir, "chat-1", &second_event)
        .expect("record second event");

    let subscription = registry
        .subscribe("workspace-1", "run-1", Some(0))
        .expect("subscribe active run");
    assert_eq!(subscription.replay.len(), 1);
    assert_eq!(subscription.replay[0].sequence, 1);
    assert!(subscription.replay[0].payload_json.contains(" world"));

    registration.finish();
    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let run_events = database
        .run_events_for_run("run-1")
        .expect("run events for run");
    assert_eq!(run_events.len(), 2);
    assert!(registry.subscribe("workspace-1", "run-1", Some(0)).is_err());
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn active_chat_run_registration_continues_persisted_run_event_sequence() {
    let registry = ActiveChatRunRegistry::default();
    let workspace_dir = env::temp_dir().join(unique_id("foco-active-run-sequence-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        database
            .insert_chat("chat-1", "Active run sequence")
            .expect("chat insert");
    }

    let (first_guidance_tx, _first_guidance_rx) = mpsc::unbounded_channel();
    let mut first_registration = registry
        .register(
            "run-1".to_string(),
            "workspace-1".to_string(),
            "chat-1".to_string(),
            "assistant-1".to_string(),
            1,
            Vec::new(),
            true,
            0,
            first_guidance_tx,
        )
        .expect("register first active run");
    first_registration
        .record_event(
            &workspace_dir,
            "chat-1",
            &ChatSseEvent::TextDelta {
                assistant_message_id: "assistant-1".to_string(),
                delta: "before wait".to_string(),
            },
        )
        .expect("record first attempt event");
    first_registration.finish();

    let next_sequence = WorkspaceDatabase::open_or_create(&workspace_dir)
        .expect("workspace database")
        .next_run_event_sequence("run-1")
        .expect("next run event sequence");
    let (second_guidance_tx, _second_guidance_rx) = mpsc::unbounded_channel();
    let mut second_registration = registry
        .register(
            "run-1".to_string(),
            "workspace-1".to_string(),
            "chat-1".to_string(),
            "assistant-2".to_string(),
            2,
            Vec::new(),
            true,
            next_sequence,
            second_guidance_tx,
        )
        .expect("register second active run");
    second_registration
        .record_event(
            &workspace_dir,
            "chat-1",
            &ChatSseEvent::TextDelta {
                assistant_message_id: "assistant-2".to_string(),
                delta: "after wait".to_string(),
            },
        )
        .expect("record second attempt event");

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let run_events = database
        .run_events_for_run("run-1")
        .expect("run events for run");
    let sequences = run_events
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>();
    assert_eq!(sequences, vec![0, 1]);

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn session_code_changed_files_counts_net_changes_from_session_baseline() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-session-net-code-stats-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    gix::init(&workspace_dir).expect("init git repo");
    fs::write(workspace_dir.join("README.md"), "value = 1\n").expect("seed file");

    let baseline = session_code_change_baseline_for_workspace(&workspace_dir);
    fs::write(workspace_dir.join("README.md"), "value = 2\n").expect("first edit");
    fs::write(workspace_dir.join("README.md"), "value = 3\n").expect("second edit");

    let changed_files = session_code_changed_files_for_workspace(&baseline, &workspace_dir)
        .expect("session changed files");

    assert_eq!(changed_files.len(), 1);
    assert_eq!(changed_files[0].0, "README.md");
    assert_eq!(changed_files[0].1.additions, 1);
    assert_eq!(changed_files[0].1.deletions, 1);

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn session_code_changed_files_excludes_preexisting_dirty_content() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-session-dirty-code-stats-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    gix::init(&workspace_dir).expect("init git repo");
    fs::write(workspace_dir.join("README.md"), "preexisting dirty\n")
        .expect("dirty before baseline");

    let baseline = session_code_change_baseline_for_workspace(&workspace_dir);
    fs::write(workspace_dir.join("README.md"), "current turn\n").expect("edit during session");

    let changed_files = session_code_changed_files_for_workspace(&baseline, &workspace_dir)
        .expect("session changed files");

    assert_eq!(changed_files.len(), 1);
    assert_eq!(changed_files[0].0, "README.md");
    assert_eq!(changed_files[0].1.additions, 1);
    assert_eq!(changed_files[0].1.deletions, 1);

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn text_code_change_stats_ignores_line_ending_only_changes() {
    let old = normalize_line_endings_for_code_change_stats("one\ntwo\nthree\n");
    let new = normalize_line_endings_for_code_change_stats("one\r\ntwo\r\nthree\r\n");

    let stats = text_code_change_stats(&old, &new);

    assert_eq!(stats.additions, 0);
    assert_eq!(stats.deletions, 0);
}

#[test]
fn git_diff_summary_uses_chinese_heading_for_chinese_language() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-zh-diff-summary-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    gix::init(&workspace_dir).expect("init git repo");

    let baseline = session_code_change_baseline_for_workspace(&workspace_dir);
    fs::write(workspace_dir.join("note.txt"), "新内容\n").expect("write changed file");

    let summary = git_diff_summary("完成。\n", &baseline, &workspace_dir, "zh-CN");

    assert!(summary.text.contains("### 本轮代码变更\n\n"));
    assert!(summary.text.contains("- `note.txt`: +1 / -0"));
    assert_eq!(summary.stats.additions, 1);
    assert_eq!(summary.stats.deletions, 0);

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn chat_code_change_stats_sum_assistant_metadata() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-chat-code-stats-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");

    database
        .insert_chat("chat-1", "Code stats chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Done.",
            sequence: 0,
            metadata_json: Some(r#"{"codeChangeStats":{"additions":3,"deletions":2}}"#),
        })
        .expect("assistant message insert");
    database
        .insert_message(NewMessage {
            id: "assistant-2",
            chat_id: "chat-1",
            role: "assistant",
            content: "Done again.",
            sequence: 1,
            metadata_json: Some(r#"{"codeChangeStats":{"additions":4,"deletions":0}}"#),
        })
        .expect("assistant message insert");

    let stats = database
        .chat_code_change_stats()
        .expect("chat code change stats");
    let chat_stats = stats.get("chat-1").expect("chat stats");

    assert_eq!(chat_stats.additions, 7);
    assert_eq!(chat_stats.deletions, 2);

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn memory_extraction_rejects_malformed_json() {
    let error = parse_memory_extraction_output(json!({
        "facts": [{
            "scope": "chat",
            "kind": "preference",
            "fact": "Prefer concise replies.",
            "confidence": 0.8,
            "evidenceReferences": []
        }]
    }))
    .expect_err("missing relationCandidates should fail");

    assert!(error.message.contains("malformed memory extraction JSON"));
}

#[test]
fn memory_extraction_rejects_missing_evidence() {
    let output = parse_memory_extraction_output(json!({
        "facts": [{
            "scope": "chat",
            "kind": "preference",
            "fact": "Prefer concise replies.",
            "confidence": 0.8,
            "relationCandidates": [],
            "evidenceReferences": [{
                "evidenceId": "missing",
                "quote": "concise"
            }]
        }]
    }))
    .expect("valid extraction JSON");
    let evidence = vec![MemoryExtractionEvidenceCandidate {
        evidence_id: "user_message".to_string(),
        source_type: MemorySourceType::ChatMessage,
        source_id: "user-1".to_string(),
        title: "User message".to_string(),
        content: "Please keep replies concise.".to_string(),
        metadata: json!({}),
    }];
    let evidence_by_id = evidence
        .iter()
        .map(|item| (item.evidence_id.as_str(), item))
        .collect::<HashMap<_, _>>();

    let error = validate_extracted_memory_facts(&output, &evidence_by_id)
        .expect_err("unknown evidence id should fail");

    assert!(error.message.contains("unknown evidence id 'missing'"));
}

#[test]
fn memory_list_summarizes_failed_extraction_jobs() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-job-summary-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut workspace_database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        workspace_database
            .insert_chat("chat-1", "Failed extraction")
            .expect("chat insert");
    }
    let mut memory_database =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
    memory_database
        .insert_extraction_job(NewMemoryExtractionJob {
            id: "job-1",
            scope: MemoryScope::Chat,
            chat_id: Some("chat-1"),
            status: MemoryExtractionJobStatus::Failed,
            model_id: Some("model-1"),
            input_json: r#"{"safe":"ok"}"#,
            output_json: None,
            error_message: Some("memory extraction provider failed"),
        })
        .expect("failed job insert");
    memory_database
        .insert_extraction_job(NewMemoryExtractionJob {
            id: "job-ignored",
            scope: MemoryScope::Chat,
            chat_id: Some("chat-1"),
            status: MemoryExtractionJobStatus::Failed,
            model_id: Some("model-1"),
            input_json: r#"{"safe":"ok"}"#,
            output_json: None,
            error_message: Some("malformed memory extraction JSON: unknown field `query`"),
        })
        .expect("failed job insert");

    let summaries = memory_extraction_job_summaries(
        MemoryScope::Workspace,
        &memory_database,
        None,
        MemoryExtractionJobStatus::Failed,
        10,
    )
    .expect("job summaries");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "job-1");
    assert_eq!(
        summaries[0].error_message.as_deref(),
        Some("memory extraction provider failed")
    );

    drop(memory_database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[tokio::test]
async fn manual_memory_dream_http_runs_workspace_dream_and_lists_audit() {
    let profile = tempfile::tempdir().expect("profile");
    let workspace = tempfile::tempdir().expect("workspace");
    let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
    config.memory.enabled = true;
    config.memory.dream.enabled = true;
    config.memory.dream.mode = MemoryDreamRunMode::DeterministicOnly.as_str().to_string();
    let workspace_id = config.workspaces[0].id.clone();
    let state = test_app_state(config, profile.path().to_path_buf());

    {
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
        let mut memory_database =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory database");
        insert_test_memory_fact(
            &mut memory_database,
            "source-expired",
            "fact-expired",
            MemoryScope::Workspace,
            None,
            "Temporary fact",
            false,
        );
        memory_database
            .update_fact(UpdateMemoryFact {
                id: "fact-expired",
                expires_at: Some("2000-01-01T00:00:00.000Z"),
                ..UpdateMemoryFact::default()
            })
            .expect("expire fact");
    }

    let run_request: MemoryDreamRunRequest = serde_json::from_value(json!({
        "scope": "workspace",
        "workspaceId": workspace_id,
        "triggerType": "manual",
        "mode": "deterministic_only"
    }))
    .expect("run request");
    let Json(run_response) = run_memory_dream(State(state.clone()), Json(run_request))
        .await
        .expect("manual dream run");
    let run_response = serde_json::to_value(run_response).expect("run response json");
    assert_eq!(run_response["status"], "completed");
    let job_id = run_response["jobId"].as_str().expect("job id").to_string();
    assert!(run_response["transcriptChatId"].as_str().is_some());

    let jobs_query: MemoryDreamJobsQuery =
        serde_json::from_value(json!({ "workspaceId": workspace_id })).expect("jobs query");
    let Json(jobs_response) = memory_dream_jobs(State(state.clone()), Query(jobs_query))
        .await
        .expect("dream jobs");
    let jobs_response = serde_json::to_value(jobs_response).expect("jobs response json");
    assert_eq!(jobs_response["jobs"][0]["id"], job_id);
    assert_eq!(
        jobs_response["jobs"][0]["transcriptWorkspaceId"],
        workspace_id
    );
    assert_eq!(jobs_response["jobs"][0]["changeCounts"]["expired"], 1);

    let Json(job_response) = memory_dream_job(State(state.clone()), AxumPath(job_id.clone()))
        .await
        .expect("dream job detail");
    let job_response = serde_json::to_value(job_response).expect("job response json");
    assert_eq!(job_response["job"]["id"], job_id);

    let changes_query: MemoryDreamChangesQuery =
        serde_json::from_value(json!({})).expect("changes query");
    let Json(changes_response) =
        memory_dream_changes(State(state), AxumPath(job_id.clone()), Query(changes_query))
            .await
            .expect("dream changes");
    let changes_response = serde_json::to_value(changes_response).expect("changes response json");
    assert_eq!(changes_response["changes"][0]["operation"], "expire");
    assert_eq!(
        changes_response["changes"][0]["targetFactIds"][0],
        "fact-expired"
    );
}

#[tokio::test]
async fn manual_memory_dream_http_rejects_validation_errors() {
    let profile = tempfile::tempdir().expect("profile");
    let workspace = tempfile::tempdir().expect("workspace");
    let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
    config.memory.enabled = true;
    config.memory.dream.enabled = true;
    let state = test_app_state(config, profile.path().to_path_buf());
    let run_request: MemoryDreamRunRequest = serde_json::from_value(json!({
        "scope": "workspace",
        "triggerType": "manual",
        "mode": "deterministic_only"
    }))
    .expect("run request");

    let error = run_memory_dream(State(state), Json(run_request))
        .await
        .expect_err("missing workspaceId should fail");

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("workspaceId"));
}

#[tokio::test]
async fn manual_memory_dream_http_rejects_active_run_conflict() {
    let profile = tempfile::tempdir().expect("profile");
    let workspace = tempfile::tempdir().expect("workspace");
    let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
    config.memory.enabled = true;
    config.memory.dream.enabled = true;
    let workspace_id = config.workspaces[0].id.clone();
    let state = test_app_state(config, profile.path().to_path_buf());

    {
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
        let mut memory_database =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory database");
        memory_database
            .insert_dream_job(NewMemoryDreamJob {
                id: "active-dream",
                scope: MemoryDreamScope::Workspace,
                workspace_id: Some(&workspace_id),
                trigger_type: MemoryDreamTriggerType::Manual,
                mode: MemoryDreamRunMode::DeterministicOnly,
                status: MemoryDreamJobStatus::Running,
                model_id: None,
                input_summary_json: "{}",
                output_summary_json: None,
                transcript_chat_id: None,
                error_message: None,
            })
            .expect("active dream job");
    }

    let run_request: MemoryDreamRunRequest = serde_json::from_value(json!({
        "scope": "workspace",
        "workspaceId": workspace_id,
        "triggerType": "manual",
        "mode": "deterministic_only"
    }))
    .expect("run request");
    let error = run_memory_dream(State(state), Json(run_request))
        .await
        .expect_err("active dream should conflict");

    assert_eq!(error.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn manual_memory_dream_http_rejects_disabled_memory() {
    let profile = tempfile::tempdir().expect("profile");
    let workspace = tempfile::tempdir().expect("workspace");
    let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
    config.memory.enabled = false;
    config.memory.dream.enabled = true;
    let state = test_app_state(config, profile.path().to_path_buf());
    let run_request: MemoryDreamRunRequest = serde_json::from_value(json!({
        "scope": "global",
        "triggerType": "manual",
        "mode": "deterministic_only"
    }))
    .expect("run request");

    let error = run_memory_dream(State(state), Json(run_request))
        .await
        .expect_err("disabled memory should fail");

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("memory is disabled"));
}

#[test]
fn auto_memory_dream_scheduler_interval_uses_injected_clock() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-06-23T00:00:00.000Z")
        .expect("timestamp")
        .with_timezone(&Utc);

    assert!(memory_dream_interval_due(None, 7, now).expect("missing success is due"));
    assert!(
        !memory_dream_interval_due(Some("2026-06-17T00:00:00.000Z"), 7, now)
            .expect("six days old is not due")
    );
    assert!(
        memory_dream_interval_due(Some("2026-06-16T00:00:00.000Z"), 7, now)
            .expect("seven days old is due")
    );
}

#[tokio::test]
async fn auto_memory_dream_scheduler_runs_due_jobs_without_scheduled_task_rows() {
    let profile = tempfile::tempdir().expect("profile");
    let workspace = tempfile::tempdir().expect("workspace");
    let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
    config.memory.enabled = true;
    config.memory.dream.enabled = true;
    config.memory.dream.auto_enabled = true;
    config.memory.dream.mode = MemoryDreamRunMode::DeterministicOnly.as_str().to_string();
    config.memory.dream.create_transcript_chat = false;
    let workspace_id = config.workspaces[0].id.clone();
    let state = test_app_state(config, profile.path().to_path_buf());

    {
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
        let mut memory_database =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory database");
        insert_test_memory_fact(
            &mut memory_database,
            "source-auto-expired",
            "fact-auto-expired",
            MemoryScope::Workspace,
            None,
            "Temporary auto fact",
            false,
        );
        memory_database
            .update_fact(UpdateMemoryFact {
                id: "fact-auto-expired",
                expires_at: Some("2000-01-01T00:00:00.000Z"),
                ..UpdateMemoryFact::default()
            })
            .expect("expire fact");
    }

    let scan = dispatch_auto_memory_dreams_at(&state, Utc::now())
        .await
        .expect("auto dream scan");

    assert_eq!(scan.runs_started, 2);
    let global_database = MemoryDatabase::open_or_create_global_at(&state.memory_database_file)
        .expect("global memory database");
    let global_jobs = global_database
        .dream_jobs_for_scope(MemoryDreamScope::Global, None, None, 10)
        .expect("global dream jobs");
    assert_eq!(global_jobs.len(), 1);
    assert_eq!(global_jobs[0].trigger_type, "auto_interval");

    let workspace_database =
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
    assert!(
        workspace_database
            .scheduled_tasks(None)
            .expect("scheduled tasks")
            .is_empty()
    );
    drop(workspace_database);

    let workspace_memory =
        MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
            .expect("workspace memory database");
    let workspace_jobs = workspace_memory
        .dream_jobs_for_scope(MemoryDreamScope::Workspace, Some(&workspace_id), None, 10)
        .expect("workspace dream jobs");
    assert_eq!(workspace_jobs.len(), 1);
    assert_eq!(workspace_jobs[0].trigger_type, "auto_interval");
}

#[tokio::test]
async fn auto_memory_dream_scheduler_uses_threshold_trigger() {
    let profile = tempfile::tempdir().expect("profile");
    let workspace = tempfile::tempdir().expect("workspace");
    let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
    config.memory.enabled = true;
    config.memory.dream.enabled = true;
    config.memory.dream.auto_enabled = true;
    config.memory.dream.mode = MemoryDreamRunMode::DeterministicOnly.as_str().to_string();
    config.memory.dream.create_transcript_chat = false;
    config.memory.dream.workspace_threshold_facts = 1;
    config.memory.dream.global_threshold_facts = 999;
    let workspace_id = config.workspaces[0].id.clone();
    let state = test_app_state(config, profile.path().to_path_buf());

    {
        let mut global_database =
            MemoryDatabase::open_or_create_global_at(&state.memory_database_file)
                .expect("global memory database");
        insert_completed_dream_job(
            &mut global_database,
            "recent-global-dream",
            MemoryDreamScope::Global,
            None,
        );
    }
    {
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
        let mut memory_database =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory database");
        insert_test_memory_fact(
            &mut memory_database,
            "source-threshold",
            "fact-threshold",
            MemoryScope::Workspace,
            None,
            "Threshold fact",
            false,
        );
    }

    let scan = dispatch_auto_memory_dreams_at(&state, Utc::now())
        .await
        .expect("auto dream scan");

    assert_eq!(scan.runs_started, 1);
    let workspace_memory =
        MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
            .expect("workspace memory database");
    let workspace_jobs = workspace_memory
        .dream_jobs_for_scope(MemoryDreamScope::Workspace, Some(&workspace_id), None, 10)
        .expect("workspace dream jobs");
    assert_eq!(workspace_jobs.len(), 1);
    assert_eq!(workspace_jobs[0].trigger_type, "auto_threshold");
}

#[tokio::test]
async fn auto_memory_dream_scheduler_skips_in_process_active_run() {
    let profile = tempfile::tempdir().expect("profile");
    let workspace = tempfile::tempdir().expect("workspace");
    let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
    config.memory.enabled = true;
    config.memory.dream.enabled = true;
    config.memory.dream.auto_enabled = true;
    config.memory.dream.mode = MemoryDreamRunMode::DeterministicOnly.as_str().to_string();
    config.memory.dream.create_transcript_chat = false;
    let workspace_id = config.workspaces[0].id.clone();
    let state = test_app_state(config, profile.path().to_path_buf());

    {
        let mut global_database =
            MemoryDatabase::open_or_create_global_at(&state.memory_database_file)
                .expect("global memory database");
        insert_completed_dream_job(
            &mut global_database,
            "recent-global-dream",
            MemoryDreamScope::Global,
            None,
        );
    }
    state
        .memory_dream_runs
        .lock()
        .await
        .insert(format!("workspace:{workspace_id}"));

    let scan = dispatch_auto_memory_dreams_at(&state, Utc::now())
        .await
        .expect("auto dream scan");

    assert_eq!(scan.runs_started, 0);
    assert_eq!(scan.skipped_active, 1);
    let workspace_memory =
        MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
            .expect("workspace memory database");
    let workspace_jobs = workspace_memory
        .dream_jobs_for_scope(MemoryDreamScope::Workspace, Some(&workspace_id), None, 10)
        .expect("workspace dream jobs");
    assert!(workspace_jobs.is_empty());
}

#[tokio::test]
async fn auto_memory_dream_startup_reconciles_interrupted_runs() {
    let profile = tempfile::tempdir().expect("profile");
    let workspace = tempfile::tempdir().expect("workspace");
    let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
    config.memory.enabled = true;
    config.memory.dream.enabled = true;
    let workspace_id = config.workspaces[0].id.clone();
    let state = test_app_state(config, profile.path().to_path_buf());

    {
        WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
        let mut memory_database =
            MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
                .expect("workspace memory database");
        memory_database
            .insert_dream_job(NewMemoryDreamJob {
                id: "interrupted-dream",
                scope: MemoryDreamScope::Workspace,
                workspace_id: Some(&workspace_id),
                trigger_type: MemoryDreamTriggerType::Manual,
                mode: MemoryDreamRunMode::Llm,
                status: MemoryDreamJobStatus::Running,
                model_id: Some("model-1"),
                input_summary_json: "{}",
                output_summary_json: None,
                transcript_chat_id: None,
                error_message: None,
            })
            .expect("active dream job");
    }

    let reconciled = reconcile_memory_dream_runs(&state).expect("reconcile dream jobs");

    assert_eq!(reconciled, 1);
    let memory_database =
        MemoryDatabase::open_workspace_at(workspace_database_path(workspace.path()))
            .expect("workspace memory database");
    let jobs = memory_database
        .dream_jobs_for_scope(MemoryDreamScope::Workspace, Some(&workspace_id), None, 10)
        .expect("workspace dream jobs");
    assert_eq!(jobs[0].status, MemoryDreamJobStatus::Failed.as_str());
    assert!(
        jobs[0]
            .error_message
            .as_deref()
            .expect("error message")
            .contains("interrupted")
    );
}

fn insert_completed_dream_job(
    database: &mut MemoryDatabase,
    id: &str,
    scope: MemoryDreamScope,
    workspace_id: Option<&str>,
) {
    database
        .insert_dream_job(NewMemoryDreamJob {
            id,
            scope,
            workspace_id,
            trigger_type: MemoryDreamTriggerType::Manual,
            mode: MemoryDreamRunMode::DeterministicOnly,
            status: MemoryDreamJobStatus::Completed,
            model_id: None,
            input_summary_json: "{}",
            output_summary_json: Some(r#"{"summary":"seed"}"#),
            transcript_chat_id: None,
            error_message: None,
        })
        .expect("completed dream job");
}

#[test]
fn memory_tool_schemas_are_strict_and_require_all_properties() {
    for tool in memory_tool_definitions() {
        assert!(tool.strict, "{} must be strict", tool.name);
        assert_strict_schema_object(&tool.name, &tool.input_schema);
    }
}

#[test]
fn memory_prompt_search_filters_stop_terms() {
    let terms = memory_prompt_search_terms("renderer prompt assembly");

    assert!(terms.contains(&"renderer".to_string()));
    assert!(terms.contains(&"assembly".to_string()));
    assert!(!terms.contains(&"prompt".to_string()));
}

#[test]
fn memory_prompt_search_adds_cjk_contains_terms() {
    let search =
        memory_prompt_search("公式渲染现在支持吗？").expect("prompt search should keep CJK terms");

    assert!(search.contains_terms.contains(&"公式渲染".to_string()));
    assert!(search.contains_terms.contains(&"渲染".to_string()));
}

#[test]
fn memory_write_tool_uses_target_status_and_sources() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-write-tool-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let context = memory_tool_test_context(&workspace_dir, MemoryStatus::Pending);

    let output = execute_memory_write_tool(
        &context,
        MemoryWriteToolInput {
            scope: "chat".to_string(),
            kind: "preference".to_string(),
            fact: "Prefer compact implementation notes.".to_string(),
            confidence: Some(0.8),
            pinned: Some(true),
            reason: Some("User preference from this run.".to_string()),
            timeout_ms: None,
        },
    )
    .expect("write memory");

    assert_eq!(output["summary"]["status"], "pending");
    assert_eq!(output["summary"]["scope"], "chat");
    assert_eq!(output["summary"]["sourceCount"], 1);
    let tool_calls = vec![ExecutedToolCall {
        id: "tool-call-1".to_string(),
        name: MEMORY_WRITE_TOOL_NAME.to_string(),
        input: json!({}),
        output: output.clone(),
        is_error: false,
        started_at: "2026-06-06T09:00:00Z".to_string(),
        completed_at: "2026-06-06T09:00:01Z".to_string(),
    }];
    let summaries = tool_written_memory_summaries(&tool_calls);

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].fact, "Prefer compact implementation notes.");
    assert_eq!(summaries[0].status, "pending");

    let database = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
        .expect("workspace memory database");
    let facts = database
        .list_facts_for_scope(Some("chat-1"), MemoryStatus::Pending, None, None, 10)
        .expect("pending facts");

    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].fact, "Prefer compact implementation notes.");
    assert!(facts[0].pinned);
    let sources = database
        .sources_for_fact(&facts[0].id)
        .expect("fact sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_type, "manual_note");
    assert_eq!(sources[0].source_id.as_deref(), Some("tool-call-1"));

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn memory_write_tool_allows_active_when_prompt_requested_memory() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-write-active-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let context = memory_tool_test_context(&workspace_dir, MemoryStatus::Active);

    let output = execute_memory_write_tool(
        &context,
        MemoryWriteToolInput {
            scope: "workspace".to_string(),
            kind: "project_fact".to_string(),
            fact: "Phase 7 exposes memory tools to the agent.".to_string(),
            confidence: None,
            pinned: None,
            reason: None,
            timeout_ms: None,
        },
    )
    .expect("write active memory");

    assert_eq!(output["summary"]["status"], "active");
    let database = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
        .expect("workspace memory database");
    let facts = database
        .list_facts_for_scope(None, MemoryStatus::Active, None, None, 10)
        .expect("active facts");

    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].scope, "workspace");

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn memory_tool_execution_rejects_when_memory_disabled() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-tool-disabled-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut context = memory_tool_test_context(&workspace_dir, MemoryStatus::Active);
    context.enabled = false;

    let error = execute_memory_tool(
        &context,
        MEMORY_WRITE_TOOL_NAME,
        json!({
            "scope": "workspace",
            "kind": "project_fact",
            "fact": "This should not be saved.",
            "confidence": null,
            "pinned": null,
            "reason": null,
            "timeoutMs": null
        }),
    )
    .expect_err("disabled memory tool should fail");

    assert_eq!(error, "memory tools are disabled in settings");

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn memory_search_tool_respects_scope_and_reports_sources() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-search-tool-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let context = memory_tool_test_context(&workspace_dir, MemoryStatus::Active);
    seed_memory_tool_fact(
        &context,
        MemoryScope::Global,
        None,
        "global-fact-1",
        "Global phoenix keyword memory.",
    );
    seed_memory_tool_fact(
        &context,
        MemoryScope::Workspace,
        None,
        "workspace-fact-1",
        "Workspace phoenix keyword memory.",
    );
    seed_memory_tool_fact(
        &context,
        MemoryScope::Chat,
        Some("chat-1"),
        "chat-fact-1",
        "Chat phoenix keyword memory.",
    );

    let chat_output = execute_memory_search_tool(
        &context,
        MemorySearchToolInput {
            query: "phoenix".to_string(),
            scope: "chat".to_string(),
            limit: Some(10),
            include_related: Some(false),
            timeout_ms: None,
        },
    )
    .expect("chat search");
    let chat_memories = chat_output["memories"].as_array().expect("chat memories");

    assert_eq!(chat_memories.len(), 1);
    assert_eq!(chat_memories[0]["scope"], "chat");
    assert_eq!(chat_memories[0]["id"], "chat-fact-1");
    assert_eq!(chat_memories[0]["sourceCount"], 1);

    let auto_output = execute_memory_search_tool(
        &context,
        MemorySearchToolInput {
            query: "phoenix".to_string(),
            scope: "auto".to_string(),
            limit: Some(10),
            include_related: Some(false),
            timeout_ms: None,
        },
    )
    .expect("auto search");
    let ids = auto_output["summary"]["factIds"]
        .as_array()
        .expect("fact ids")
        .iter()
        .map(|id| id.as_str().expect("id").to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        ids,
        BTreeSet::from([
            "chat-fact-1".to_string(),
            "global-fact-1".to_string(),
            "workspace-fact-1".to_string(),
        ])
    );
    assert_eq!(auto_output["summary"]["sourceCount"], 3);

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn memory_extraction_request_includes_existing_candidates_and_strict_prompt_rules() {
    let evidence = vec![MemoryExtractionEvidenceCandidate {
        evidence_id: "user_message".to_string(),
        source_type: MemorySourceType::ChatMessage,
        source_id: "user-1".to_string(),
        title: "User message".to_string(),
        content: "Remember that I prefer concise replies.".to_string(),
        metadata: json!({"role":"user"}),
    }];
    let existing = vec![MemoryFactRecord {
        id: "fact-1".to_string(),
        scope: "workspace".to_string(),
        chat_id: None,
        status: "active".to_string(),
        kind: "preference".to_string(),
        fact: "Prefer concise replies.".to_string(),
        confidence: Some(0.9),
        pinned: false,
        is_latest: true,
        expires_at: None,
        metadata_json: "{}".to_string(),
        created_at: "2026-06-11T00:00:00Z".to_string(),
        updated_at: "2026-06-11T00:00:00Z".to_string(),
    }];

    let request = memory_extraction_provider_request(
        "model-1",
        "workspace-1",
        "chat-1",
        "run-1",
        "provider-1",
        "zh-CN",
        512,
        &evidence,
        &existing,
    )
    .expect("memory extraction request");

    assert_eq!(request.messages[0].role, NeutralChatRole::System);
    let system_prompt = &request.messages[0].content;
    assert!(system_prompt.contains("Simplified Chinese"));
    assert!(system_prompt.contains("Use the submit_memory_extraction tool exactly once"));
    assert!(system_prompt.contains("Do not return prose"));
    assert!(system_prompt.contains("unlikely to change often"));
    assert!(system_prompt.contains("duplicates or near-duplicates"));
    assert!(system_prompt.contains("provided evidenceIds"));
    assert_eq!(request.messages[1].role, NeutralChatRole::User);
    assert!(
        request.messages[1]
            .content
            .contains("Existing memory candidates JSON")
    );
    assert!(request.messages[1].content.contains("workspace:fact-1"));
    assert!(
        request.messages[1]
            .content
            .contains("Prefer concise replies.")
    );
    assert!(request.messages[1].content.contains("Evidence JSON"));
}

#[test]
fn memory_extraction_existing_candidates_include_active_and_pending_memories() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-extract-candidates-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut workspace_database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        workspace_database
            .insert_chat("chat-1", "Memory candidates")
            .expect("chat insert");
    }
    let mut workspace_memory =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
    let mut global_memory =
        MemoryDatabase::open_or_create_global_at(workspace_dir.join("global-memory.sqlite"))
            .expect("global memory database");

    insert_test_memory_fact(
        &mut workspace_memory,
        "workspace-active-source",
        "workspace-active",
        MemoryScope::Workspace,
        None,
        "Workspace active memory.",
        false,
    );
    insert_test_memory_fact_with_status(
        &mut workspace_memory,
        "chat-pending-source",
        "chat-pending",
        MemoryScope::Chat,
        Some("chat-1"),
        MemoryStatus::Pending,
        "Chat pending memory.",
        false,
    );
    insert_test_memory_fact(
        &mut global_memory,
        "global-active-source",
        "global-active",
        MemoryScope::Global,
        None,
        "Global active memory.",
        false,
    );

    let candidates =
        memory_extraction_existing_memory_candidates(&global_memory, &workspace_memory, "chat-1")
            .expect("existing memory candidates");
    let facts = candidates
        .iter()
        .map(|fact| fact.fact.as_str())
        .collect::<HashSet<_>>();

    assert!(facts.contains("Workspace active memory."));
    assert!(facts.contains("Chat pending memory."));
    assert!(facts.contains("Global active memory."));

    drop(global_memory);
    drop(workspace_memory);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn memory_extraction_validates_required_fact_fields() {
    let output = parse_memory_extraction_output(json!({
        "facts": [{
            "scope": "chat",
            "kind": "preference",
            "fact": "Prefer concise replies.",
            "confidence": 0.8,
            "relationCandidates": [{
                "relation": "derives",
                "targetFactId": null,
                "targetFact": null,
                "reason": "User asked for concise replies."
            }],
            "evidenceReferences": [{
                "evidenceId": "user_message",
                "quote": "concise"
            }]
        }]
    }))
    .expect("valid extraction JSON");
    let evidence = vec![MemoryExtractionEvidenceCandidate {
        evidence_id: "user_message".to_string(),
        source_type: MemorySourceType::ChatMessage,
        source_id: "user-1".to_string(),
        title: "User message".to_string(),
        content: "Please keep replies concise.".to_string(),
        metadata: json!({}),
    }];
    let evidence_by_id = evidence
        .iter()
        .map(|item| (item.evidence_id.as_str(), item))
        .collect::<HashMap<_, _>>();

    let facts =
        validate_extracted_memory_facts(&output, &evidence_by_id).expect("valid extracted fact");

    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].scope, MemoryScope::Chat);
    assert_eq!(facts[0].kind, MemoryKind::Preference);
    assert_eq!(facts[0].evidence_ids, vec!["user_message"]);
    assert!(facts[0].metadata_json.contains("relationCandidates"));
}

#[test]
fn memory_extraction_stores_pending_facts_with_sources() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-extract-store-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut workspace_database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        workspace_database
            .insert_chat("chat-1", "Memory extraction")
            .expect("chat insert");
    }
    let task = MemoryExtractionTask {
        job_id: "job-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        workspace_path: workspace_dir.clone(),
        global_memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        run_id: "run-1".to_string(),
        user_message_id: "user-1".to_string(),
        assistant_message_id: "assistant-1".to_string(),
        model_id: "model-1".to_string(),
        target_status: MemoryStatus::Pending,
        config: GlobalConfig::first_run(workspace_dir.clone()),
    };
    let evidence = vec![MemoryExtractionEvidenceCandidate {
        evidence_id: "user_message".to_string(),
        source_type: MemorySourceType::ChatMessage,
        source_id: "user-1".to_string(),
        title: "User message".to_string(),
        content: "Remember that I prefer concise replies.".to_string(),
        metadata: json!({"role":"user"}),
    }];
    let output = parse_memory_extraction_output(json!({
        "facts": [{
            "scope": "chat",
            "kind": "preference",
            "fact": "Prefer concise replies.",
            "confidence": 0.9,
            "relationCandidates": [],
            "evidenceReferences": [{
                "evidenceId": "user_message",
                "quote": "prefer concise replies"
            }]
        }]
    }))
    .expect("valid extraction JSON");

    let summaries =
        store_extracted_memory_facts(&task, &evidence, &output).expect("store extracted facts");

    let memory_database =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
    let facts = memory_database
        .list_facts_for_scope(Some("chat-1"), MemoryStatus::Pending, None, None, 10)
        .expect("pending facts");

    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].fact, "Prefer concise replies.");
    assert_eq!(facts[0].status, "pending");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, facts[0].id);
    assert_eq!(summaries[0].fact, "Prefer concise replies.");
    assert_eq!(summaries[0].status, "pending");
    let sources = memory_database
        .sources_for_fact(&facts[0].id)
        .expect("fact sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_id.as_deref(), Some("user-1"));

    drop(memory_database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn memory_extraction_materializes_update_relations() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-extract-edge-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut workspace_database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        workspace_database
            .insert_chat("chat-1", "Memory extraction edges")
            .expect("chat insert");
    }
    {
        let mut memory_database =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
                .expect("workspace memory database");
        insert_test_memory_fact(
            &mut memory_database,
            "old-source",
            "old-fact",
            MemoryScope::Workspace,
            None,
            "Use the old memory retrieval plan.",
            false,
        );
    }
    let task = MemoryExtractionTask {
        job_id: "job-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        workspace_path: workspace_dir.clone(),
        global_memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        run_id: "run-1".to_string(),
        user_message_id: "user-1".to_string(),
        assistant_message_id: "assistant-1".to_string(),
        model_id: "model-1".to_string(),
        target_status: MemoryStatus::Active,
        config: GlobalConfig::first_run(workspace_dir.clone()),
    };
    let evidence = vec![MemoryExtractionEvidenceCandidate {
        evidence_id: "assistant_message".to_string(),
        source_type: MemorySourceType::AssistantMessage,
        source_id: "assistant-1".to_string(),
        title: "Assistant message".to_string(),
        content: "Use the new memory graph retrieval plan.".to_string(),
        metadata: json!({"role":"assistant"}),
    }];
    let output = parse_memory_extraction_output(json!({
        "facts": [{
            "scope": "workspace",
            "kind": "project_decision",
            "fact": "Use the new memory graph retrieval plan.",
            "confidence": 0.9,
            "relationCandidates": [{
                "relation": "updates",
                "targetFactId": "workspace:old-fact",
                "targetFact": "Use the old memory retrieval plan.",
                "reason": "The new plan replaces the old plan."
            }],
            "evidenceReferences": [{
                "evidenceId": "assistant_message",
                "quote": "new memory graph retrieval plan"
            }]
        }]
    }))
    .expect("valid extraction JSON");

    let summaries =
        store_extracted_memory_facts(&task, &evidence, &output).expect("store extracted facts");

    let memory_database =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
    assert_eq!(summaries.len(), 1);
    let old_fact = memory_database
        .fact("old-fact")
        .expect("old fact lookup")
        .expect("old fact");
    let new_fact = memory_database
        .fact(&summaries[0].id)
        .expect("new fact lookup")
        .expect("new fact");

    assert_eq!(old_fact.status, "superseded");
    assert!(!old_fact.is_latest);
    assert_eq!(new_fact.status, "active");
    assert!(new_fact.is_latest);

    drop(memory_database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn explicit_remember_this_extraction_stores_active_facts() {
    assert_eq!(
        memory_target_status_for_prompt("remember this: prefer concise replies"),
        MemoryStatus::Active
    );
    assert_eq!(
        memory_target_status_for_prompt("please remember I use Foco"),
        MemoryStatus::Active
    );
    assert_eq!(
        memory_target_status_for_prompt("we discussed a preference"),
        MemoryStatus::Pending
    );

    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-remember-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    {
        let mut workspace_database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        workspace_database
            .insert_chat("chat-1", "Explicit memory")
            .expect("chat insert");
    }
    let task = MemoryExtractionTask {
        job_id: "job-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        workspace_path: workspace_dir.clone(),
        global_memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        run_id: "run-1".to_string(),
        user_message_id: "user-1".to_string(),
        assistant_message_id: "assistant-1".to_string(),
        model_id: "model-1".to_string(),
        target_status: MemoryStatus::Active,
        config: GlobalConfig::first_run(workspace_dir.clone()),
    };
    let evidence = vec![MemoryExtractionEvidenceCandidate {
        evidence_id: "user_message".to_string(),
        source_type: MemorySourceType::ChatMessage,
        source_id: "user-1".to_string(),
        title: "User message".to_string(),
        content: "Remember this: prefer concise replies.".to_string(),
        metadata: json!({"role":"user"}),
    }];
    let output = parse_memory_extraction_output(json!({
        "facts": [{
            "scope": "chat",
            "kind": "preference",
            "fact": "Prefer concise replies.",
            "confidence": 0.9,
            "relationCandidates": [],
            "evidenceReferences": [{
                "evidenceId": "user_message",
                "quote": "prefer concise replies"
            }]
        }]
    }))
    .expect("valid extraction JSON");

    let summaries =
        store_extracted_memory_facts(&task, &evidence, &output).expect("store extracted facts");

    let memory_database =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
    let active_facts = memory_database
        .list_facts_for_scope(Some("chat-1"), MemoryStatus::Active, None, None, 10)
        .expect("active facts");
    let pending_facts = memory_database
        .list_facts_for_scope(Some("chat-1"), MemoryStatus::Pending, None, None, 10)
        .expect("pending facts");

    assert_eq!(active_facts.len(), 1);
    assert_eq!(active_facts[0].fact, "Prefer concise replies.");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, active_facts[0].id);
    assert_eq!(summaries[0].status, "active");
    assert!(pending_facts.is_empty());

    drop(memory_database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn automatic_memory_extraction_targets_active_facts() {
    let pending_review_settings = MemorySettings {
        enabled: true,
        extraction_mode: "pending_review".to_string(),
        retrieval_mode: "fts".to_string(),
        retention_days: None,
        extraction_model_id: None,
        retrieval_model_id: None,
        dream: MemoryDreamSettings::default(),
    };
    let automatic_settings = MemorySettings {
        enabled: true,
        extraction_mode: "automatic".to_string(),
        retrieval_mode: "fts".to_string(),
        retention_days: None,
        extraction_model_id: None,
        retrieval_model_id: None,
        dream: MemoryDreamSettings::default(),
    };
    let manual_settings = MemorySettings {
        enabled: true,
        extraction_mode: "manual".to_string(),
        retrieval_mode: "fts".to_string(),
        retention_days: None,
        extraction_model_id: None,
        retrieval_model_id: None,
        dream: MemoryDreamSettings::default(),
    };

    assert!(should_queue_memory_extraction(&pending_review_settings));
    assert!(should_queue_memory_extraction(&automatic_settings));
    assert!(!should_queue_memory_extraction(&manual_settings));
    assert_eq!(
        memory_extraction_target_status("pending_review", MemoryStatus::Pending),
        MemoryStatus::Pending
    );
    assert_eq!(
        memory_extraction_target_status("pending_review", MemoryStatus::Active),
        MemoryStatus::Active
    );
    assert_eq!(
        memory_extraction_target_status("automatic", MemoryStatus::Pending),
        MemoryStatus::Active
    );
}

#[test]
fn chat_message_summary_includes_assistant_reply_metrics() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-message-metrics-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");

    database
        .insert_chat("chat-1", "Metrics chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Done.",
            sequence: 0,
            metadata_json: Some(
                r#"{"memoriesUsed":[{"id":"fact-1","scope":"workspace","chatId":null,"kind":"project_fact","fact":"Use memory graph retrieval.","pinned":false,"source":"direct"}]}"#,
            ),
        })
        .expect("assistant message insert");
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
            model_id: "gpt-5.4",
            request_started_at: "2026-06-06T09:00:00Z",
            first_token_at: Some("2026-06-06T09:00:00Z"),
            completed_at: Some("2026-06-06T09:00:02Z"),
            input_tokens: Some(100),
            output_tokens: Some(40),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            first_token_latency_ms: Some(250),
            total_latency_ms: Some(2000),
            status_code: None,
            final_state: "succeeded",
            request_body_json: Some("{}"),
            response_body_json: Some("{}"),
        })
        .expect("llm request insert");
    database
        .insert_llm_request_event(NewLlmRequestEvent {
            id: "request-1-event-0",
            llm_request_id: "request-1",
            sequence: 0,
            event_at: "2026-06-06T09:00:00Z",
            event_type: "start",
            raw_chunk_json: None,
            normalized_event_json: r#"{"type":"start","chatId":"chat-1","userMessageId":"user-1","assistantMessageId":"assistant-1","llmRequestId":"request-1"}"#,
        })
        .expect("llm start event insert");

    let messages = database.messages_for_chat("chat-1").expect("messages");
    let summary = chat_message_summaries(&mut database, &workspace_dir, None, "chat-1", messages)
        .expect("message summaries")
        .into_iter()
        .next()
        .expect("assistant summary");
    let metrics = summary.metrics.expect("assistant metrics");

    assert_eq!(metrics.model_id, "gpt-5.4");
    assert_eq!(metrics.provider_id, "openai-responses");
    assert_eq!(metrics.total_latency_ms, Some(2000));
    assert_eq!(metrics.first_token_latency_ms, Some(250));
    assert_eq!(metrics.output_tokens, Some(40));
    assert_eq!(summary.memories_used.len(), 1);
    assert_eq!(summary.memories_used[0].id, "fact-1");
    assert_eq!(summary.memories_used[0].source, "direct");

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn chat_message_summary_includes_assistant_extracted_memories() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-message-memory-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let mut memory_database =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");

    database
        .insert_chat("chat-1", "Memory chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Done.",
            sequence: 0,
            metadata_json: Some("{}"),
        })
        .expect("assistant message insert");
    memory_database
        .insert_source(NewMemorySource {
            id: "source-1",
            scope: MemoryScope::Chat,
            chat_id: Some("chat-1"),
            source_type: MemorySourceType::AssistantMessage,
            source_id: Some("assistant-1"),
            title: "Assistant message",
            content: "Done.",
            metadata_json: "{}",
        })
        .expect("memory source insert");
    memory_database
        .insert_fact(NewMemoryFact {
            id: "fact-1",
            scope: MemoryScope::Chat,
            chat_id: Some("chat-1"),
            status: MemoryStatus::Pending,
            kind: MemoryKind::Episode,
            fact: "Remember that README was inspected after completion.",
            confidence: Some(0.8),
            pinned: false,
            source_ids: &["source-1"],
            metadata_json: "{}",
        })
        .expect("memory fact insert");

    let messages = database.messages_for_chat("chat-1").expect("messages");
    let summary = chat_message_summaries(&mut database, &workspace_dir, None, "chat-1", messages)
        .expect("message summaries")
        .into_iter()
        .next()
        .expect("assistant summary");

    assert_eq!(summary.extracted_memories.len(), 1);
    assert_eq!(summary.extracted_memories[0].id, "fact-1");
    assert_eq!(summary.extracted_memories[0].status, "pending");
    assert_eq!(
        summary.extracted_memories[0].fact,
        "Remember that README was inspected after completion."
    );

    drop(memory_database);
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn chat_message_summary_aggregates_multiple_llm_request_metrics() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-message-multi-metrics-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let mut database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");

    database
        .insert_chat("chat-1", "Multi request metrics chat")
        .expect("chat insert");
    database
        .insert_message(NewMessage {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "assistant",
            content: "Done.",
            sequence: 0,
            metadata_json: Some("{}"),
        })
        .expect("assistant message insert");

    for (index, (request_id, output_tokens, latency_ms)) in [
        ("request-1", 12_i64, 1_000_i64),
        ("request-2", 28_i64, 1_500_i64),
    ]
    .into_iter()
    .enumerate()
    {
        database
            .insert_llm_request(NewLlmRequest {
                id: request_id,
                workspace_id: "workspace-1",
                chat_id: Some("chat-1"),
                agent_team_id: None,
                agent_instance_id: None,
                agent_task_id: None,
                agent_attempt_id: None,
                provider_id: "openai-responses",
                model_id: "gpt-5.4",
                request_started_at: "2026-06-06T09:00:00Z",
                first_token_at: Some("2026-06-06T09:00:00Z"),
                completed_at: Some("2026-06-06T09:00:02Z"),
                input_tokens: Some(100),
                output_tokens: Some(output_tokens),
                cache_read_tokens: Some(0),
                cache_write_tokens: Some(0),
                first_token_latency_ms: Some(if index == 0 { 250 } else { 300 }),
                total_latency_ms: Some(latency_ms),
                status_code: Some(200),
                final_state: "succeeded",
                request_body_json: Some("{}"),
                response_body_json: Some("{}"),
            })
            .expect("llm request insert");
        database
            .insert_llm_request_event(NewLlmRequestEvent {
                id: &format!("{request_id}-event-0"),
                llm_request_id: request_id,
                sequence: 0,
                event_at: "2026-06-06T09:00:00Z",
                event_type: "start",
                raw_chunk_json: None,
                normalized_event_json: &format!(
                    r#"{{"type":"start","chatId":"chat-1","userMessageId":"user-1","assistantMessageId":"assistant-1","llmRequestId":"{request_id}"}}"#
                ),
            })
            .expect("llm start event insert");
    }

    let messages = database.messages_for_chat("chat-1").expect("messages");
    let summary = chat_message_summaries(&mut database, &workspace_dir, None, "chat-1", messages)
        .expect("message summaries")
        .into_iter()
        .next()
        .expect("assistant summary");
    let metrics = summary.metrics.expect("assistant metrics");

    assert_eq!(metrics.total_latency_ms, Some(2500));
    assert_eq!(metrics.first_token_latency_ms, Some(250));
    assert_eq!(metrics.output_tokens, Some(40));

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn pack_neutral_messages_keeps_saved_tool_state_group_together() {
    let messages = vec![
        neutral_text_message(NeutralChatRole::System, "system".to_string()),
        neutral_text_message(NeutralChatRole::User, "old question".to_string()),
        neutral_assistant_tool_call_message(
            NeutralToolCall {
                call_id: "call-1".to_string(),
                name: "read_file".to_string(),
                arguments: json!({ "path": "README.md" }),
                thought_signatures: None,
            },
            String::new(),
            None,
        ),
        NeutralChatMessage {
            role: NeutralChatRole::Tool,
            content: r#"{"content":"hello"}"#.to_string(),
            attachments: Vec::new(),
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: Some("call-1".to_string()),
            tool_name: Some("read_file".to_string()),
        },
        neutral_text_message(NeutralChatRole::User, "latest".to_string()),
    ];
    let message_source_sequences = vec![None, Some(0), Some(1), Some(1), Some(2)];
    let message_context_sources = vec![
        PromptContextSource::ReservedPrompt,
        PromptContextSource::StoredMessage { sequence: 0 },
        PromptContextSource::StoredMessage { sequence: 1 },
        PromptContextSource::StoredMessage { sequence: 1 },
        PromptContextSource::CurrentUser { sequence: 2 },
    ];
    let groups = context_message_groups(
        &messages,
        &message_source_sequences,
        &message_context_sources,
        messages.len(),
    )
    .expect("message groups");
    let required_tokens = groups
        .iter()
        .filter(|group| group.must_keep)
        .map(|group| group.estimated_tokens)
        .sum::<u64>();
    let tool_group_tokens = groups
        .iter()
        .find(|group| group.message_indices == vec![2, 3])
        .expect("tool state group")
        .estimated_tokens;
    let tight_budget = foco_agent::ContextBudget {
        context_window: 1_000,
        max_output_tokens: 1,
        system_prompt_tokens: 0,
        tool_schema_tokens: 0,
        safety_tokens: 0,
        available_message_tokens: required_tokens + tool_group_tokens - 1,
    };

    let packed = pack_neutral_messages(
        messages.clone(),
        &message_source_sequences,
        &message_context_sources,
        &tight_budget,
        messages.len(),
    )
    .expect("packed messages");

    assert!(
        packed
            .iter()
            .all(|message| message.role != NeutralChatRole::Tool)
    );

    let full_budget = foco_agent::ContextBudget {
        available_message_tokens: required_tokens + tool_group_tokens,
        ..tight_budget
    };
    let packed = pack_neutral_messages(
        messages,
        &message_source_sequences,
        &message_context_sources,
        &full_budget,
        message_source_sequences.len(),
    )
    .expect("packed messages");
    let tool_position = packed
        .iter()
        .position(|message| message.role == NeutralChatRole::Tool)
        .expect("tool message should be kept");

    assert_eq!(packed[tool_position - 1].role, NeutralChatRole::Assistant);
    assert_eq!(packed[tool_position - 1].tool_calls[0].call_id, "call-1");
    assert_eq!(
        packed[tool_position].tool_call_id.as_deref(),
        Some("call-1")
    );
}

#[tokio::test]
async fn add_workspace_creates_missing_directory_and_registers_it() {
    let existing_workspace_dir = env::temp_dir().join(unique_id("foco-existing-workspace-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-add-workspace-profile-test"));
    let new_workspace_dir = env::temp_dir().join(unique_id("foco-new-workspace-test"));

    fs::create_dir_all(&existing_workspace_dir).expect("existing workspace directory");
    fs::create_dir_all(profile_dir.join(".foco")).expect("profile config directory");

    let config = GlobalConfig::first_run(existing_workspace_dir.clone());
    let state = test_app_state(config, profile_dir.clone());

    let response = add_workspace(
        State(state.clone()),
        Json(WorkspacePathRequest {
            name: "New Workspace".to_string(),
            path: new_workspace_dir.display().to_string(),
            content_base64: Some(
                general_purpose::STANDARD.encode([0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            ),
        }),
    )
    .await
    .expect("add workspace");

    assert!(new_workspace_dir.is_dir());
    assert!(
        WorkspaceDatabase::open_or_create(&new_workspace_dir)
            .expect("workspace database")
            .database_path()
            .is_file()
    );

    let registered_path = normalize_windows_verbatim_path(
        fs::canonicalize(&new_workspace_dir).expect("new workspace canonical path"),
    );
    let response = response.0;
    let response_workspace = response
        .workspaces
        .first()
        .expect("response workspace first");
    assert_eq!(response_workspace.name, "New Workspace");
    assert!(
        response_workspace
            .logo_url
            .as_deref()
            .is_some_and(|logo_url| logo_url.starts_with("/api/workspaces/new-workspace/logo?v="))
    );
    let logo = workspace_logo_file(&new_workspace_dir)
        .expect("workspace logo lookup")
        .expect("workspace logo file");
    assert_eq!(logo.kind.extension, "png");
    let config = state.config.lock().expect("config lock");
    let registered_workspace = config.workspaces.first().expect("new workspace first");
    assert_eq!(registered_workspace.name, "New Workspace");
    assert_eq!(registered_workspace.path, registered_path);
    drop(config);

    fs::remove_dir_all(existing_workspace_dir).expect("remove existing workspace directory");
    fs::remove_dir_all(new_workspace_dir).expect("remove new workspace directory");
    fs::remove_dir_all(profile_dir).expect("remove profile directory");
}

#[tokio::test]
async fn create_terminal_session_defaults_to_workspace_directory() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-terminal-workspace-test"));
    let nested_dir = workspace_dir.join("nested");
    let profile_dir = env::temp_dir().join(unique_id("foco-terminal-profile-test"));

    fs::create_dir_all(&nested_dir).expect("nested workspace directory");

    let config = GlobalConfig::first_run(workspace_dir.clone());
    let state = test_app_state(config.clone(), profile_dir.clone());
    let previous_directory = terminal::shell_path(&nested_dir).display().to_string();
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        database
            .upsert_terminal_session(NewTerminalSession {
                id: "terminal-previous",
                name: "Previous Terminal",
                working_directory: &previous_directory,
                metadata_json: None,
            })
            .expect("previous terminal session");
        database
            .close_terminal_session("terminal-previous")
            .expect("close previous terminal session");
    }

    let Json(response) =
        create_terminal_session(State(state), AxumPath(config.workspaces[0].id.clone()))
            .await
            .expect("terminal session response");
    let expected_directory = terminal::shell_path(&workspace_dir).display().to_string();

    assert_eq!(response.working_directory, expected_directory);

    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let stored_session = database
        .terminal_session(&response.id)
        .expect("stored terminal session")
        .expect("terminal session exists");

    assert_eq!(stored_session.working_directory, expected_directory);

    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_chat_context_freezes_initial_context_for_chat_replay() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-chat-agents-workspace-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-chat-agents-profile-test"));
    let codex_dir = profile_dir.join(".codex");
    let configured_prompt_file = codex_dir.join("AGENTS.md");
    let skill_dir = profile_dir.join(".agents").join("skills").join("gitmemo");

    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&codex_dir).expect("codex directory");
    fs::create_dir_all(&skill_dir).expect("skill directory");
    fs::write(
        workspace_dir.join("AGENTS.md"),
        "Workspace chat instructions.\n",
    )
    .expect("workspace AGENTS write");
    fs::write(
        &configured_prompt_file,
        "Configured prompt chat instructions.\n",
    )
    .expect("configured prompt write");
    fs::write(
        skill_dir.join("SKILL.md"),
        "---
name: gitmemo
description: Project memory.
---

# GitMemo

Search memory before repo work.
",
    )
    .expect("skill file write");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.prompts.files.push(configured_prompt_file);
    config.prompts.extra_text = "Extra configured prompt.\n".to_string();
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 100_000,
            max_output_tokens: 1_024,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());

    let new_context = prepare_chat_context(
        &state,
        &config,
        &config.workspaces[0].id,
        ChatStreamRequest {
            queued_user_message_id: None,
            run_id_override: None,
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: "Hello".to_string(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("new chat context");
    let injected_messages = new_context
        .provider_request
        .messages
        .iter()
        .filter(|message| message.content.contains(AGENTS_MESSAGE_PREFIX))
        .collect::<Vec<_>>();

    assert_eq!(injected_messages.len(), 1);
    assert!(
        injected_messages[0]
            .content
            .contains("Workspace chat instructions.")
    );
    assert!(injected_messages.iter().all(|message| {
        !message
            .content
            .contains("Configured prompt chat instructions.")
    }));
    let prompt_messages = new_context
        .provider_request
        .messages
        .iter()
        .filter(|message| {
            message.content.contains(PROMPT_FILE_MESSAGE_PREFIX)
                || message.content.contains(EXTRA_PROMPT_MESSAGE_PREFIX)
        })
        .collect::<Vec<_>>();

    assert_eq!(prompt_messages.len(), 2);
    assert!(
        prompt_messages[0]
            .content
            .contains("Extra configured prompt.")
    );
    assert!(
        prompt_messages[1]
            .content
            .contains("Configured prompt chat instructions.")
    );
    let skill_messages = new_context
        .provider_request
        .messages
        .iter()
        .filter(|message| message.content.contains(ENABLED_SKILLS_MESSAGE_PREFIX))
        .collect::<Vec<_>>();

    assert_eq!(skill_messages.len(), 1);
    assert!(skill_messages[0].content.contains("name: gitmemo"));
    assert!(
        skill_messages[0]
            .content
            .contains("description: Project memory.")
    );
    assert!(!skill_messages[0].content.contains("Search memory"));
    let environment_messages = new_context
        .provider_request
        .messages
        .iter()
        .filter(|message| message.content.contains(ENVIRONMENT_CONTEXT_MESSAGE_PREFIX))
        .collect::<Vec<_>>();

    assert_eq!(environment_messages.len(), 1);
    assert_eq!(environment_messages[0].role, NeutralChatRole::User);
    assert!(environment_messages[0].content.contains(&format!(
        "- workspace directory: {}",
        workspace_dir.display()
    )));
    assert!(
        environment_messages[0]
            .content
            .contains("- git repository: ")
    );
    assert!(environment_messages[0].content.contains("- shell type: "));
    assert!(
        environment_messages[0]
            .content
            .contains("- shell executable: ")
    );
    assert!(environment_messages[0].content.contains("- current date: "));
    assert!(environment_messages[0].content.contains("- time zone: "));
    assert!(environment_messages[0].content.contains("- wsl: "));

    {
        let database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let stored_messages = database
            .messages_for_chat(&new_context.chat_id)
            .expect("stored messages");
        assert_eq!(stored_messages.len(), 1);
        assert_eq!(stored_messages[0].content, "Hello");
        let context_injections = database
            .prompt_context_injections_for_chat(&new_context.chat_id)
            .expect("context injections");
        assert_eq!(context_injections.len(), 1);
        assert_eq!(context_injections[0].kind, "stable");
        assert!(
            context_injections[0]
                .messages_json
                .contains(AGENTS_MESSAGE_PREFIX)
        );
        assert!(
            context_injections[0]
                .messages_json
                .contains(ENVIRONMENT_CONTEXT_MESSAGE_PREFIX)
        );
        assert!(
            !context_injections[0]
                .messages_json
                .contains(EXTRA_PROMPT_MESSAGE_PREFIX)
        );
    }

    config.prompts.extra_text = "Updated extra configured prompt.\n".to_string();

    let existing_context = prepare_chat_context(
        &state,
        &config,
        &config.workspaces[0].id,
        ChatStreamRequest {
            queued_user_message_id: None,
            run_id_override: None,
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            chat_id: Some(new_context.chat_id.clone()),
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: "Next".to_string(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("existing chat context");

    let existing_agents_messages = existing_context
        .provider_request
        .messages
        .iter()
        .filter(|message| message.content.contains(AGENTS_MESSAGE_PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(existing_agents_messages.len(), 1);
    assert_eq!(
        existing_agents_messages[0].content,
        injected_messages[0].content
    );
    let existing_prompt_messages = existing_context
        .provider_request
        .messages
        .iter()
        .filter(|message| {
            message.content.contains(PROMPT_FILE_MESSAGE_PREFIX)
                || message.content.contains(EXTRA_PROMPT_MESSAGE_PREFIX)
        })
        .collect::<Vec<_>>();
    assert_eq!(existing_prompt_messages.len(), 2);
    assert!(
        existing_prompt_messages[0]
            .content
            .contains("Updated extra configured prompt.")
    );
    assert!(
        !existing_prompt_messages[0]
            .content
            .contains("Extra configured prompt.")
    );
    assert_eq!(
        existing_prompt_messages[1].content,
        prompt_messages[1].content
    );
    let existing_skill_messages = existing_context
        .provider_request
        .messages
        .iter()
        .filter(|message| message.content.contains(ENABLED_SKILLS_MESSAGE_PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(existing_skill_messages.len(), 1);
    assert_eq!(
        existing_skill_messages[0].content,
        skill_messages[0].content
    );
    let existing_environment_messages = existing_context
        .provider_request
        .messages
        .iter()
        .filter(|message| message.content.contains(ENVIRONMENT_CONTEXT_MESSAGE_PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(existing_environment_messages.len(), 1);
    assert_eq!(
        existing_environment_messages[0].content,
        environment_messages[0].content
    );
    assert_ne!(
        new_context.provider_request.prompt_cache_key,
        existing_context.provider_request.prompt_cache_key
    );
    assert_eq!(
        existing_context
            .provider_request
            .prompt_cache_retention
            .as_deref(),
        Some(PROMPT_CACHE_RETENTION_24H)
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_chat_context_continues_without_deferred_memory() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-failure-workspace-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-memory-failure-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.memory.enabled = true;
    config.memory.retrieval_mode = "unsupported".to_string();
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 100_000,
            max_output_tokens: 1_024,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());

    let mut context = prepare_chat_context(
        &state,
        &config,
        &config.workspaces[0].id,
        ChatStreamRequest {
            queued_user_message_id: None,
            run_id_override: None,
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: "Hello after creation".to_string(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("chat context");

    assert!(context.pending_memory_retrieval.is_some());
    serde_json::from_str::<Value>(&context.request_body_json)
        .expect("deferred request body is valid JSON");
    let error = context
        .resolve_pending_memory(&config)
        .await
        .err()
        .expect("unsupported memory retrieval should fail");
    assert!(
        error
            .message
            .contains("memory retrieval mode 'unsupported' is unsupported")
    );
    assert!(context.pending_memory_retrieval.is_none());
    context
        .finalize_prompt_without_memory()
        .expect("finalize without memory");
    assert!(context.provider_request.prompt_cache_key.is_some());
    assert_eq!(
        context.provider_request.prompt_cache_retention.as_deref(),
        Some(PROMPT_CACHE_RETENTION_24H)
    );
    let final_request_json: Value =
        serde_json::from_str(&context.request_body_json).expect("final request body is valid JSON");
    assert!(
        !final_request_json
            .to_string()
            .contains(MEMORY_RETRIEVED_CONTEXT_MESSAGE_PREFIX)
    );

    {
        let database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let stored_messages = database
            .messages_for_chat(&context.chat_id)
            .expect("stored messages");
        assert_eq!(stored_messages.len(), 1);
        assert_eq!(stored_messages[0].content, "Hello after creation");

        let injections = database
            .prompt_context_injections_for_chat(&context.chat_id)
            .expect("context injections");
        assert_eq!(injections.len(), 1);
        assert_eq!(injections[0].kind, "stable");
        assert_eq!(injections[0].memory_keys_json, "[]");
        assert!(
            injections[0]
                .messages_json
                .contains(ENVIRONMENT_CONTEXT_MESSAGE_PREFIX)
        );
    }

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn chat_stream_starts_when_deferred_memory_fails() {
    let workspace_dir =
        env::temp_dir().join(unique_id("foco-memory-stream-failure-workspace-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-memory-stream-failure-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.memory.enabled = true;
    config.memory.retrieval_mode = "unsupported".to_string();
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 100_000,
            max_output_tokens: 1_024,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());
    let context = prepare_chat_context(
        &state,
        &config,
        &config.workspaces[0].id,
        ChatStreamRequest {
            queued_user_message_id: None,
            run_id_override: None,
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: "Hello after creation".to_string(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("chat context");
    let (guidance_tx, guidance_rx) = mpsc::unbounded_channel();
    drop(guidance_tx);
    let stream = context.into_sse_stream(ChatRunCancellation::new(), guidance_rx);
    tokio::pin!(stream);

    let first = stream.next().await.expect("start event");
    assert!(matches!(first, ChatSseEvent::Start { .. }));

    let second = stream.next().await.expect("attempt start event");
    assert!(
        matches!(second, ChatSseEvent::StreamAttemptStart { .. }),
        "memory retrieval failure must not emit an error before provider start"
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_chat_context_prefixes_selected_skills_in_user_message() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-selected-skill-workspace-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-selected-skill-profile-test"));
    let skill_dir = workspace_dir
        .join(".agents")
        .join("skills")
        .join("web-design-guidelines");
    let skill_file = skill_dir.join("SKILL.md");

    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    fs::create_dir_all(&skill_dir).expect("skill directory");
    fs::write(
        &skill_file,
        "---
name: web-design-guidelines
description: UI design guidance.
---

# Web Design Guidelines

Use the existing product UI conventions.
",
    )
    .expect("skill file write");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 100_000,
            max_output_tokens: 1_024,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());
    let expected_message = format!(
        "[$web-design-guidelines]({}) Settings single-column layout.",
        skill_file.display()
    );

    let context = prepare_chat_context(
        &state,
        &config,
        &config.workspaces[0].id,
        ChatStreamRequest {
            queued_user_message_id: None,
            run_id_override: None,
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: Some(vec!["workspace:default:web-design-guidelines".to_string()]),
            message: "Settings single-column layout.".to_string(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("selected skill chat context");

    let latest_user_message = context
        .provider_request
        .messages
        .iter()
        .rev()
        .find(|message| message.role == NeutralChatRole::User)
        .expect("latest user message");
    assert_eq!(latest_user_message.content, expected_message);

    {
        let database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let stored_messages = database
            .messages_for_chat(&context.chat_id)
            .expect("stored messages");
        assert_eq!(stored_messages.len(), 1);
        assert_eq!(stored_messages[0].content, expected_message);
    }

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_prompt_context_hides_memory_tools_when_memory_disabled() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-tools-disabled-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-memory-tools-disabled-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.memory.enabled = false;
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());
    let context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("hello".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ChatRun,
    )
    .await
    .expect("prompt context");
    let tool_names = context
        .provider_request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<BTreeSet<_>>();

    assert!(!tool_names.contains(MEMORY_SEARCH_TOOL_NAME));
    assert!(!tool_names.contains(MEMORY_WRITE_TOOL_NAME));
    let available_tools_message = context
        .provider_request
        .messages
        .get(1)
        .expect("available tools message");
    assert_eq!(available_tools_message.role, NeutralChatRole::System);
    assert!(available_tools_message.content.contains("Available tools:"));
    assert!(
        !available_tools_message
            .content
            .contains(MEMORY_SEARCH_TOOL_NAME)
    );
    assert!(
        !available_tools_message
            .content
            .contains(MEMORY_WRITE_TOOL_NAME)
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_prompt_context_rejects_memory_dream_transcript_chat() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-dream-transcript-prompt-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-dream-transcript-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let config = prompt_test_config(workspace_dir.clone());
    let state = test_app_state(config.clone(), profile_dir.clone());
    let chat_id = unique_id("chat");
    let mut database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace db");
    database
        .insert_chat_with_metadata(
            &chat_id,
            "Memory Dream: global manual",
            &json!({ "kind": foco_store::memory::MEMORY_DREAM_TRANSCRIPT_CHAT_KIND }).to_string(),
        )
        .expect("insert transcript chat");
    database
        .insert_message(NewMessage {
            id: &unique_id("msg-system"),
            chat_id: &chat_id,
            role: "system",
            content: "job started",
            sequence: 0,
            metadata_json: None,
        })
        .expect("insert transcript message");
    drop(database);

    let result = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: Some(chat_id),
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("continue".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ChatRun,
    )
    .await;
    let error = match result {
        Ok(_) => panic!("memory Dream transcript chat was accepted as a normal prompt"),
        Err(error) => error,
    };

    assert!(
        error
            .message
            .contains("memory Dream transcript chats are read-only")
    );

    remove_dir_if_exists(&workspace_dir);
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_prompt_context_hides_search_text_when_ripgrep_unavailable() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-ripgrep-tools-disabled-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-ripgrep-tools-disabled-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());
    {
        let mut status = state.ripgrep_status.lock().expect("ripgrep status lock");
        status.available = false;
        status.path = None;
    }

    let context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("hello".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ChatRun,
    )
    .await
    .expect("prompt context");
    let tool_names = context
        .provider_request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<BTreeSet<_>>();

    assert!(!tool_names.contains(SEARCH_TEXT_TOOL));
    let available_tools_message = context
        .provider_request
        .messages
        .get(1)
        .expect("available tools message");
    assert_eq!(available_tools_message.role, NeutralChatRole::System);
    assert!(available_tools_message.content.contains("Available tools:"));
    assert!(!available_tools_message.content.contains(SEARCH_TEXT_TOOL));

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_prompt_context_hides_web_search_without_enabled_search_api() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-web-search-disabled-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-web-search-disabled-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let config = prompt_test_config(workspace_dir.clone());
    let state = test_app_state(config.clone(), profile_dir.clone());
    let context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("hello".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ChatRun,
    )
    .await
    .expect("prompt context");
    let tool_names = context
        .provider_request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<BTreeSet<_>>();

    assert!(!tool_names.contains(WEB_SEARCH_TOOL));
    assert!(tool_names.contains(WEB_FETCH_TOOL));
    let available_tools_message = context
        .provider_request
        .messages
        .get(1)
        .expect("available tools message");
    assert!(!available_tools_message.content.contains(WEB_SEARCH_TOOL));
    assert!(available_tools_message.content.contains(WEB_FETCH_TOOL));

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_prompt_context_exposes_web_search_when_search_api_enabled() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-web-search-enabled-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-web-search-enabled-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = prompt_test_config(workspace_dir.clone());
    config.web_search.enabled = true;
    config.web_search.tavily_api_key = Some("tavily-token".to_string());
    let state = test_app_state(config.clone(), profile_dir.clone());
    let context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("hello".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ChatRun,
    )
    .await
    .expect("prompt context");
    let tool_names = context
        .provider_request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<BTreeSet<_>>();

    assert!(tool_names.contains(WEB_SEARCH_TOOL));
    assert!(tool_names.contains(WEB_FETCH_TOOL));
    let available_tools_message = context
        .provider_request
        .messages
        .get(1)
        .expect("available tools message");
    assert!(available_tools_message.content.contains(WEB_SEARCH_TOOL));
    assert!(available_tools_message.content.contains(WEB_FETCH_TOOL));

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_prompt_context_uses_model_system_prompt() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-custom-system-prompt-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-custom-system-prompt-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.prompts.system_prompts = vec![
        SystemPromptSettings {
            name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
            content: "Default Foco system prompt.".to_string(),
        },
        SystemPromptSettings {
            name: "Review".to_string(),
            content: "Review Foco system prompt.".to_string(),
        },
    ];
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: "Review".to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());

    let context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("hello".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ChatRun,
    )
    .await
    .expect("prompt context");

    assert_eq!(
        context.provider_request.messages[0].role,
        NeutralChatRole::System
    );
    assert_eq!(
        context.provider_request.messages[0].content,
        "Review Foco system prompt."
    );
    assert!(
        !context.provider_request.messages[0]
            .content
            .contains("You are Foco")
    );
    assert!(
        !context.provider_request.messages[0]
            .content
            .contains("Available tools:")
    );
    let available_tools_message = context
        .provider_request
        .messages
        .get(1)
        .expect("available tools message");
    assert_eq!(available_tools_message.role, NeutralChatRole::System);
    assert!(available_tools_message.content.contains("Available tools:"));
    assert!(available_tools_message.content.contains("read_file"));

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prompt_cache_key_changes_when_model_system_prompt_changes() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-system-prompt-cache-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-system-prompt-cache-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.prompts.system_prompts = vec![
        SystemPromptSettings {
            name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
            content: "Default cache prompt.".to_string(),
        },
        SystemPromptSettings {
            name: "Review".to_string(),
            content: "Review cache prompt.".to_string(),
        },
    ];
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    config.models.push(ModelSettings {
        id: "review-model".to_string(),
        display_name: "Review Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: "Review".to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());
    let first_context = prepare_chat_context(
        &state,
        &config,
        &config.workspaces[0].id,
        ChatStreamRequest {
            queued_user_message_id: None,
            run_id_override: None,
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: "hello".to_string(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("first chat context");
    let first_cache_key = first_context
        .provider_request
        .prompt_cache_key
        .clone()
        .expect("first cache key");
    assert_eq!(
        first_context.provider_request.messages[0].content,
        "Default cache prompt."
    );

    let second_context = prepare_chat_context(
        &state,
        &config,
        &config.workspaces[0].id,
        ChatStreamRequest {
            queued_user_message_id: None,
            run_id_override: None,
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            chat_id: Some(first_context.chat_id.clone()),
            model_id: "review-model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: "next".to_string(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("second chat context");

    assert_eq!(
        second_context.provider_request.messages[0].content,
        "Review cache prompt."
    );
    assert_ne!(
        first_cache_key,
        second_context
            .provider_request
            .prompt_cache_key
            .expect("second cache key")
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_prompt_context_appends_memory_context_after_current_user() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-prompt-workspace-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-memory-prompt-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.memory.enabled = true;
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());
    {
        let mut workspace_database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        workspace_database
            .insert_chat("chat-1", "Memory prompt chat")
            .expect("chat insert");
    }
    {
        let mut memory = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
        insert_test_memory_fact(
            &mut memory,
            "source-chat-renderer",
            "fact-chat-renderer",
            MemoryScope::Chat,
            Some("chat-1"),
            "Use the renderer pipeline for preview bugs.",
            false,
        );
        insert_test_memory_fact(
            &mut memory,
            "source-workspace-renderer",
            "fact-workspace-renderer",
            MemoryScope::Workspace,
            None,
            "Workspace renderer work should share prompt assembly.",
            false,
        );
        insert_test_memory_fact(
            &mut memory,
            "source-related-renderer",
            "fact-related-renderer",
            MemoryScope::Workspace,
            None,
            "Graph-linked memory is pulled through adjacent edges.",
            false,
        );
        insert_test_memory_fact(
            &mut memory,
            "source-unrelated-billing",
            "fact-unrelated-billing",
            MemoryScope::Workspace,
            None,
            "Billing invoices use monthly finance tags.",
            false,
        );
        memory
            .insert_edge(foco_store::memory::NewMemoryEdge {
                id: "edge-related-renderer",
                source_fact_id: "fact-chat-renderer",
                target_fact_id: "fact-related-renderer",
                relation: foco_store::memory::MemoryRelationKind::Extends,
                metadata_json: "{}",
            })
            .expect("memory edge insert");
    }
    {
        let mut memory = MemoryDatabase::open_or_create_global_at(&state.memory_database_file)
            .expect("global memory database");
        insert_test_memory_fact(
            &mut memory,
            "source-global-renderer",
            "fact-global-renderer",
            MemoryScope::Global,
            None,
            "Global renderer memory should be available.",
            false,
        );
    }

    let mut context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: Some("chat-1".to_string()),
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("renderer prompt assembly".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ChatRun,
    )
    .await
    .expect("prompt context");
    resolve_prompt_context_memory(&mut context, &state.memory_database_file, &config)
        .await
        .expect("resolve memory");
    let messages = &context.provider_request.messages;

    assert!(messages[0].content.contains("You are Foco"));
    let current_user_index = messages
        .iter()
        .position(|message| message.content == "renderer prompt assembly")
        .expect("current user message");
    assert_eq!(
        messages[current_user_index].content,
        "renderer prompt assembly"
    );
    let memory_message = messages
        .get(current_user_index + 1)
        .expect("memory message after current user");
    assert_eq!(memory_message.role, NeutralChatRole::User);
    assert!(
        memory_message
            .content
            .contains(MEMORY_RETRIEVED_CONTEXT_MESSAGE_PREFIX)
    );
    assert!(
        memory_message
            .content
            .contains("Use the renderer pipeline for preview bugs.")
    );
    assert!(
        memory_message
            .content
            .contains("Workspace renderer work should share prompt assembly.")
    );
    assert!(
        memory_message
            .content
            .contains("Graph-linked memory is pulled through adjacent edges.")
    );
    assert!(memory_message.content.contains("source: direct"));
    assert!(memory_message.content.contains("source: related"));
    assert!(
        memory_message
            .content
            .contains("Global renderer memory should be available.")
    );
    assert!(
        !memory_message
            .content
            .contains("Billing invoices use monthly finance tags.")
    );
    assert_eq!(
        context.message_source_sequences[current_user_index],
        context.message_source_sequences[current_user_index + 1]
    );
    assert!(
        context
            .memories_used
            .iter()
            .any(|memory| memory.id == "fact-chat-renderer" && memory.source == "direct")
    );
    assert!(
        context
            .memories_used
            .iter()
            .any(|memory| memory.id == "fact-related-renderer" && memory.source == "related")
    );
    assert!(context.memory_context_tokens > 0);
    assert!(context.memory_context_tokens <= context.memory_budget_tokens);
    assert_eq!(
        context.memory_budget_tokens,
        context
            .context_budget
            .available_message_tokens
            .saturating_mul(MEMORY_CONTEXT_BUDGET_PERCENT)
            / 100
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_prompt_context_injects_existing_todo_graph_for_followup_run() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-todo-graph-context-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-todo-graph-context-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        database
            .insert_chat("chat-1", "Interrupted plan")
            .expect("chat insert");
        database
            .insert_message(NewMessage {
                id: "user-1",
                chat_id: "chat-1",
                role: "user",
                content: "Build the settings panel.",
                sequence: 0,
                metadata_json: Some("{}"),
            })
            .expect("message insert");
        database
            .upsert_todo_graph(
                "chat-1",
                vec![TodoGraphTask {
                    id: "settings-panel".to_string(),
                    title: "Build settings panel".to_string(),
                    status: "running".to_string(),
                    depends_on: Vec::new(),
                    acceptance: vec!["Panel renders current settings".to_string()],
                    summary: "Implementation was started before the run was cancelled.".to_string(),
                    created_at: String::new(),
                    updated_at: String::new(),
                    subtasks: Vec::new(),
                }],
            )
            .expect("todo graph insert");
    }

    let context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: Some("chat-1".to_string()),
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("继续完成剩下的工作".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ChatRun,
    )
    .await
    .expect("prompt context");

    let messages = &context.provider_request.messages;
    let todo_graph_index = messages
        .iter()
        .position(|message| message.content.contains(TODO_GRAPH_CONTEXT_MESSAGE_PREFIX))
        .expect("todo graph context message");
    assert_eq!(messages[todo_graph_index].role, NeutralChatRole::System);
    assert!(
        messages[todo_graph_index]
            .content
            .contains("\"chatId\": \"chat-1\"")
    );
    assert!(
        messages[todo_graph_index]
            .content
            .contains("\"id\": \"settings-panel\"")
    );
    assert!(
        messages[todo_graph_index]
            .content
            .contains("Continue maintaining this graph")
    );
    let current_user_index = messages
        .iter()
        .position(|message| message.content == "继续完成剩下的工作")
        .expect("current user message");
    assert!(todo_graph_index < current_user_index);
    assert_eq!(context.message_source_sequences[todo_graph_index], None);
    assert_eq!(
        context.message_source_sequences[current_user_index],
        Some(1)
    );
    assert!(context.pending_context_injections.is_empty());

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_prompt_context_allocates_after_hidden_worker_messages() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-hidden-worker-sequence-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-hidden-worker-sequence-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let config = prompt_test_config(workspace_dir.clone());
    let state = test_app_state(config.clone(), profile_dir.clone());
    {
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        database
            .insert_chat("chat-hidden-worker-sequence", "Hidden worker sequence")
            .expect("chat insert");
        let team_id =
            foco_agent::AgentTeamId::new("agent-team-hidden-worker-sequence").expect("team id");
        let coordinator_id =
            foco_agent::AgentInstanceId::new("agent-instance-hidden-worker-sequence-coordinator")
                .expect("coordinator id");
        let worker_id =
            foco_agent::AgentInstanceId::new("agent-instance-hidden-worker-sequence-worker")
                .expect("worker id");
        let definition = AgentDefinitionSettings {
            id: AgentDefinitionId::new("agent-definition-hidden-worker-sequence")
                .expect("definition id"),
            revision: 1,
            name: "Hidden worker coordinator".to_string(),
            description: String::new(),
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            model_options: AgentModelOptions::default(),
            system_prompt: "Coordinate.".to_string(),
            allowed_tools: Vec::new(),
            max_instances: 2,
            allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
            permissions: AgentPermissions::default(),
        };
        database
            .create_agent_team(foco_store::workspace::NewAgentTeam {
                id: &team_id,
                chat_id: "chat-hidden-worker-sequence",
                coordinator_instance_id: &coordinator_id,
                coordinator_definition: &definition,
                max_concurrent_runs: 1,
            })
            .expect("team create");
        database
            .create_agent_instances_with_limits(
                &[foco_store::workspace::NewAgentInstance {
                    id: &worker_id,
                    team_id: &team_id,
                    definition: &definition,
                    role: foco_agent::AgentRole::Worker,
                    execution_workspace_mode: foco_agent::AgentExecutionWorkspaceMode::Shared,
                    execution_root_path: None,
                    worktree_base_revision: None,
                    worktree_branch: None,
                    worktree_status: None,
                }],
                2,
                2,
            )
            .expect("worker create");
        let worker_task_id =
            foco_agent::AgentTaskId::new("agent-task-hidden-worker-sequence").expect("task id");
        database
            .enqueue_agent_task(foco_store::workspace::NewAgentTask {
                id: &worker_task_id,
                team_id: &team_id,
                owner_instance_id: &worker_id,
                origin_instance_id: Some(&coordinator_id),
                parent_task_id: None,
                input_json: r#"{"queuedUserMessageId":"user-worker"}"#,
            })
            .expect("worker task enqueue");
        for (id, role, content, sequence) in [
            ("user-main", "user", "Main request", 0),
            ("assistant-main", "assistant", "Main answer", 1),
            ("user-worker", "user", "Worker-only prompt", 2),
        ] {
            database
                .insert_message(NewMessage {
                    id,
                    chat_id: "chat-hidden-worker-sequence",
                    role,
                    content,
                    sequence,
                    metadata_json: None,
                })
                .expect("message insert");
        }
    }

    let context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: Some("chat-hidden-worker-sequence".to_string()),
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("Follow up".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ChatRun,
    )
    .await
    .expect("prompt context");

    assert_eq!(context.next_message_sequence, 3);
    assert!(
        context
            .provider_request
            .messages
            .iter()
            .any(|message| message.content == "Main request")
    );
    assert!(
        context
            .provider_request
            .messages
            .iter()
            .all(|message| !message.content.contains("Worker-only prompt"))
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_chat_context_replays_stable_memory_and_dedupes_turn_memory() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-cache-layout-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-memory-cache-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.memory.enabled = true;
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());
    WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    {
        let mut memory = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
        insert_test_memory_fact(
            &mut memory,
            "source-workspace-renderer",
            "fact-workspace-renderer",
            MemoryScope::Workspace,
            None,
            "Workspace renderer memory should stay in the stable prefix.",
            false,
        );
    }

    let mut first_context = prepare_chat_context(
        &state,
        &config,
        &config.workspaces[0].id,
        ChatStreamRequest {
            queued_user_message_id: None,
            run_id_override: None,
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: "renderer first turn".to_string(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("first chat context");
    first_context
        .resolve_pending_memory(&config)
        .await
        .expect("resolve first context memory");
    let stable_memory_index = first_context
        .provider_request
        .messages
        .iter()
        .position(|message| {
            message.role == NeutralChatRole::System
                && message
                    .content
                    .contains("Workspace renderer memory should stay in the stable prefix.")
        })
        .expect("stable memory message");
    assert!(
        stable_memory_index
            < first_context
                .provider_request
                .messages
                .iter()
                .position(|message| message.content == "renderer first turn")
                .expect("first user index")
    );
    assert_eq!(
        first_context
            .provider_request
            .prompt_cache_retention
            .as_deref(),
        Some(PROMPT_CACHE_RETENTION_24H)
    );
    let first_cache_key = first_context
        .provider_request
        .prompt_cache_key
        .clone()
        .expect("first cache key");
    {
        let database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let injections = database
            .prompt_context_injections_for_chat(&first_context.chat_id)
            .expect("context injections");
        assert_eq!(injections.len(), 1);
        assert_eq!(injections[0].kind, "stable");
        assert!(
            injections[0]
                .memory_keys_json
                .contains("workspace:fact-workspace-renderer")
        );
    }
    {
        let mut memory = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
        insert_test_memory_fact(
            &mut memory,
            "source-chat-renderer",
            "fact-chat-renderer",
            MemoryScope::Chat,
            Some(&first_context.chat_id),
            "Chat renderer delta memory should be appended after the second user.",
            false,
        );
    }

    let mut second_context = prepare_chat_context(
        &state,
        &config,
        &config.workspaces[0].id,
        ChatStreamRequest {
            queued_user_message_id: None,
            run_id_override: None,
            visible_assistant_message_id: None,
            visible_assistant_sequence: None,
            chat_id: Some(first_context.chat_id.clone()),
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: "renderer second turn".to_string(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("second chat context");
    second_context
        .resolve_pending_memory(&config)
        .await
        .expect("resolve second context memory");
    let second_text = second_context
        .provider_request
        .messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        second_text
            .matches("Workspace renderer memory should stay in the stable prefix.")
            .count(),
        1
    );
    let latest_user_index = second_context
        .provider_request
        .messages
        .iter()
        .rposition(|message| message.content == "renderer second turn")
        .expect("second user index");
    let turn_memory = second_context
        .provider_request
        .messages
        .get(latest_user_index + 1)
        .expect("second turn memory");
    assert_eq!(turn_memory.role, NeutralChatRole::User);
    assert!(
        turn_memory
            .content
            .contains("Chat renderer delta memory should be appended after the second user.")
    );
    assert!(
        !turn_memory
            .content
            .contains("Workspace renderer memory should stay in the stable prefix.")
    );
    assert_eq!(
        second_context.message_source_sequences[latest_user_index],
        second_context.message_source_sequences[latest_user_index + 1]
    );
    assert!(second_context.provider_request.prompt_cache_key.is_some());
    assert_ne!(
        second_context.provider_request.prompt_cache_key.as_deref(),
        Some(first_cache_key.as_str())
    );

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn prepare_prompt_context_retrieves_cjk_memory_without_exact_question_match() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-cjk-workspace-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-memory-cjk-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.memory.enabled = true;
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());
    {
        let mut workspace_database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        workspace_database
            .insert_chat("chat-1", "CJK memory prompt chat")
            .expect("chat insert");
    }
    {
        let mut memory = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
        insert_test_memory_fact(
            &mut memory,
            "source-cjk-formula",
            "fact-cjk-formula",
            MemoryScope::Workspace,
            None,
            "Markdown 预览已经支持公式渲染。",
            false,
        );
        insert_test_memory_fact(
            &mut memory,
            "source-cjk-unrelated",
            "fact-cjk-unrelated",
            MemoryScope::Workspace,
            None,
            "账单发票使用月度财务标签。",
            false,
        );
    }

    let mut context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: Some("chat-1".to_string()),
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("现在 markdown 预览支持公式吗？".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ChatRun,
    )
    .await
    .expect("prompt context");
    resolve_prompt_context_memory(&mut context, &state.memory_database_file, &config)
        .await
        .expect("resolve memory");
    let request_json: Value =
        serde_json::from_str(&serialize_provider_request(&context.provider_request).unwrap())
            .expect("request json");
    let request_text = request_json.to_string();

    assert!(request_text.contains("Foco retrieved memory context"));
    assert!(request_text.contains("Markdown 预览已经支持公式渲染。"));
    assert!(!request_text.contains("账单发票使用月度财务标签。"));

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[test]
fn memory_retrieval_query_text_uses_only_current_request_for_first_turn() {
    let query = memory_retrieval_query_text(Some("  Explain the renderer plan.  "), &[])
        .expect("memory retrieval query");

    assert_eq!(
        query,
        format!("{MEMORY_RETRIEVAL_CURRENT_REQUEST_LABEL}\nExplain the renderer plan.")
    );
    assert!(!query.contains(MEMORY_RETRIEVAL_PREVIOUS_ASSISTANT_LABEL));
}

#[test]
fn memory_retrieval_query_text_includes_latest_assistant_response_for_followup() {
    let messages = vec![
        MessageRecord {
            id: "user-1".to_string(),
            chat_id: "chat-1".to_string(),
            role: "user".to_string(),
            content: "Design a renderer plan.".to_string(),
            sequence: 0,
            created_at: "2026-06-17T00:00:00Z".to_string(),
            metadata_json: "{}".to_string(),
        },
        MessageRecord {
            id: "assistant-1".to_string(),
            chat_id: "chat-1".to_string(),
            role: "assistant".to_string(),
            content: "Implement renderer plan B.".to_string(),
            sequence: 1,
            created_at: "2026-06-17T00:00:01Z".to_string(),
            metadata_json: "{}".to_string(),
        },
        MessageRecord {
            id: "assistant-2".to_string(),
            chat_id: "chat-1".to_string(),
            role: "assistant".to_string(),
            content: "  Final conclusion: use plan C.  ".to_string(),
            sequence: 2,
            created_at: "2026-06-17T00:00:02Z".to_string(),
            metadata_json: "{}".to_string(),
        },
    ];

    let query = memory_retrieval_query_text(Some("  Start implementation.  "), &messages)
        .expect("memory retrieval query");

    assert_eq!(
        query,
        format!(
            "{MEMORY_RETRIEVAL_CURRENT_REQUEST_LABEL}\nStart implementation.\n\n{MEMORY_RETRIEVAL_PREVIOUS_ASSISTANT_LABEL}\nFinal conclusion: use plan C."
        )
    );
    assert!(!query.contains("Implement renderer plan B."));
}

#[test]
fn model_memory_retrieval_rejects_too_many_active_memories() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-memory-llm-limit-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let workspace_database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let mut workspace_memory =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
    let global_memory =
        MemoryDatabase::open_or_create_global_at(workspace_dir.join("global-memory.sqlite"))
            .expect("global memory database");

    for index in 0..=MEMORY_RETRIEVAL_LLM_FACT_LIMIT {
        insert_test_memory_fact(
            &mut workspace_memory,
            &format!("source-llm-limit-{index}"),
            &format!("fact-llm-limit-{index}"),
            MemoryScope::Workspace,
            None,
            &format!("LLM retrieval limit memory {index}."),
            false,
        );
    }

    let error = llm_memory_retrieval_candidates(&global_memory, &workspace_memory, None)
        .expect_err("too many active memories should fail");

    assert!(error.message.contains("at most"));
    assert!(error.message.contains("use SQLite FTS"));

    drop(workspace_memory);
    drop(global_memory);
    drop(workspace_database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[tokio::test]
async fn context_usage_preview_does_not_persist_chat_messages() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-context-usage-workspace-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-context-usage-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.memory.enabled = true;
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());
    {
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let mut memory = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
        insert_test_memory_fact(
            &mut memory,
            "source-context",
            "fact-context",
            MemoryScope::Workspace,
            None,
            "Preview usage should include workspace memory.",
            false,
        );
    }

    let prompt_context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("Preview workspace memory usage".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ContextPreview,
    )
    .await
    .expect("prompt context");
    let usage = context_usage_response(
        &prompt_context,
        &NeutralUsage {
            input_tokens: Some(1_500),
            output_tokens: Some(250),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
        },
    )
    .expect("context usage from response usage");

    assert_eq!(usage.used_message_tokens, 1_750);
    assert!(usage.memory_context_tokens > 0);
    assert_eq!(
        usage.memory_context_tokens,
        prompt_context.memory_context_tokens
    );
    assert_eq!(
        usage.memory_budget_tokens,
        prompt_context.memory_budget_tokens
    );
    assert_eq!(
        usage.compression_trigger_tokens,
        prompt_context
            .context_budget
            .system_prompt_tokens
            .saturating_add(prompt_context.context_budget.tool_schema_tokens)
            .saturating_add(context_compression_trigger_tokens(
                prompt_context.context_budget.available_message_tokens
            ))
    );
    assert_eq!(
        usage.compression_trigger_percent,
        usage
            .compression_trigger_tokens
            .saturating_mul(100)
            .div_ceil(usage.available_message_tokens)
    );
    let database = WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    assert!(database.chats().expect("chat list").is_empty());
    let memory_database =
        MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
    assert_eq!(
        memory_database
            .list_active_facts_for_scope(None, 10)
            .expect("workspace memories")
            .len(),
        1
    );

    drop(database);
    drop(memory_database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[tokio::test]
async fn context_usage_preview_does_not_call_model_memory_retrieval() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-context-usage-no-llm-test"));
    let profile_dir = env::temp_dir().join(unique_id("foco-context-usage-no-llm-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.memory.enabled = true;
    config.memory.retrieval_mode = "llm".to_string();
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    let state = test_app_state(config.clone(), profile_dir.clone());
    {
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let mut memory = MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
            .expect("workspace memory database");
        for index in 0..=MEMORY_RETRIEVAL_LLM_FACT_LIMIT {
            insert_test_memory_fact(
                &mut memory,
                &format!("source-preview-{index}"),
                &format!("fact-preview-{index}"),
                MemoryScope::Workspace,
                None,
                &format!("Preview retrieval memory {index}."),
                false,
            );
        }
    }

    let prompt_context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            queued_user_message_id: None,
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("Preview retrieval memory".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
        None,
        PromptAssemblyPurpose::ContextPreview,
    )
    .await
    .expect("context preview should avoid model memory retrieval");

    assert!(prompt_context.memory_context_tokens > 0);

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
}

#[test]
fn discover_skills_reports_non_directory_paths() {
    let profile_dir = env::temp_dir().join(unique_id("foco-skill-file-path-test"));
    let skill_path = profile_dir.join(".agents").join("skills");

    fs::create_dir_all(profile_dir.join(".agents")).expect("profile skill parent");
    fs::write(&skill_path, "not a directory").expect("skill path file write");

    let discovery = discover_skills(&profile_dir, &[]);

    assert_eq!(discovery.errors.len(), 1);
    assert!(discovery.errors[0].message.contains("not a directory"));
    assert!(discovery.skills.is_empty());

    fs::remove_dir_all(profile_dir).expect("remove profile directory");
}

#[test]
fn manual_skill_save_uses_explicit_disabled_skill_keys() {
    let discovered = vec![
        test_skill_settings("global:gitmemo", "gitmemo"),
        test_skill_settings("workspace:default:gitmemo", "gitmemo"),
    ];
    let disabled = normalize_manual_disabled_skill_ids(
        Some(vec!["workspace:default:gitmemo".to_string()]),
        None,
        &discovered,
    )
    .expect("disabled skill ids");

    assert_eq!(disabled, vec!["workspace:default:gitmemo"]);
}

#[test]
fn manual_skill_save_derives_disabled_keys_from_enabled_keys() {
    let discovered = vec![
        test_skill_settings("global:gitmemo", "gitmemo"),
        test_skill_settings("workspace:default:gitmemo", "gitmemo"),
    ];
    let disabled = normalize_manual_disabled_skill_ids(
        None,
        Some(vec!["global:gitmemo".to_string()]),
        &discovered,
    )
    .expect("disabled skill ids");

    assert_eq!(disabled, vec!["workspace:default:gitmemo"]);
}

#[test]
fn required_disabled_skill_keys_survive_manual_enable_request() {
    let discovered = vec![
        test_skill_settings("global:gitmemo", "gitmemo"),
        test_skill_settings("global:broken", "broken"),
    ];
    let user_disabled = normalize_manual_disabled_skill_ids(
        Some(Vec::new()),
        Some(vec![
            "global:gitmemo".to_string(),
            "global:broken".to_string(),
        ]),
        &discovered,
    )
    .expect("manual skill ids");
    let disabled = merge_disabled_skill_keys(user_disabled, &["global:broken".to_string()]);

    assert_eq!(disabled, vec!["global:broken"]);
}

#[test]
fn discover_skills_reads_workspace_skill_directories() {
    let profile_dir = env::temp_dir().join(unique_id("foco-workspace-skill-profile-test"));
    let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-skill-test"));
    let skill_dir = workspace_dir.join(".agents").join("skills").join("gitmemo");
    let skill_file = skill_dir.join("SKILL.md");

    fs::create_dir_all(&profile_dir).expect("profile directory");
    fs::create_dir_all(&skill_dir).expect("skill test directory");
    fs::write(
        &skill_file,
        "---
name: gitmemo
description: Project memory.
---

# GitMemo
",
    )
    .expect("skill file write");

    let workspaces = vec![WorkspaceConfig {
        id: "default".to_string(),
        name: "Default".to_string(),
        path: workspace_dir.clone(),
        pinned: false,
        terminal_shell: DEFAULT_TERMINAL_SHELL.to_string(),
        common_commands: Vec::new(),
    }];
    let discovery = discover_skills(&profile_dir, &workspaces);

    assert!(discovery.errors.is_empty());
    assert_eq!(discovery.skills.len(), 1);
    assert_eq!(discovery.skills[0].key, "workspace:default:gitmemo");
    assert_eq!(discovery.skills[0].id, "gitmemo");
    assert_eq!(discovery.skills[0].scope, SKILL_SCOPE_WORKSPACE);
    assert_eq!(discovery.skills[0].workspace_id.as_deref(), Some("default"));
    assert_eq!(
        discovery.skills[0].workspace_name.as_deref(),
        Some("Default")
    );
    assert_eq!(discovery.skills[0].path, skill_file);

    fs::remove_dir_all(profile_dir).expect("remove profile directory");
    fs::remove_dir_all(workspace_dir).expect("remove skill test directory");
}

fn test_skill_settings(key: &str, id: &str) -> SkillSettings {
    SkillSettings {
        key: key.to_string(),
        id: id.to_string(),
        name: id.to_string(),
        description: "Test skill.".to_string(),
        path: env::temp_dir().join(id).join("SKILL.md"),
        scope: if key.starts_with("workspace:") {
            SKILL_SCOPE_WORKSPACE.to_string()
        } else {
            SKILL_SCOPE_GLOBAL.to_string()
        },
        workspace_id: key
            .starts_with("workspace:")
            .then(|| DEFAULT_WORKSPACE_ID.to_string()),
        workspace_name: key
            .starts_with("workspace:")
            .then(|| DEFAULT_WORKSPACE_NAME.to_string()),
    }
}

fn test_model_settings(id: &str) -> ModelSettings {
    ModelSettings {
        id: id.to_string(),
        display_name: id.to_string(),
        enabled: false,
        provider_ids: Vec::new(),
        active_provider_id: None,
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: None,
    }
}

fn model_ids(models: &[ModelSettings]) -> Vec<&str> {
    models.iter().map(|model| model.id.as_str()).collect()
}

fn test_workspace_config(id: &str) -> WorkspaceConfig {
    WorkspaceConfig {
        id: id.to_string(),
        name: id.to_string(),
        path: env::temp_dir().join(id),
        pinned: false,
        terminal_shell: DEFAULT_TERMINAL_SHELL.to_string(),
        common_commands: Vec::new(),
    }
}

fn workspace_ids(workspaces: &[WorkspaceConfig]) -> Vec<&str> {
    workspaces
        .iter()
        .map(|workspace| workspace.id.as_str())
        .collect()
}

fn remove_dir_if_exists(path: &Path) {
    if !path.exists() {
        return;
    }

    let mut last_error = None;
    for attempt in 0..10 {
        match fs::remove_dir_all(path) {
            Ok(()) => return,
            Err(error) if !path.exists() => return,
            Err(error) => {
                last_error = Some(error);
                std::thread::sleep(std::time::Duration::from_millis(20 * (attempt + 1)));
            }
        }
    }

    panic!(
        "remove test directory '{}': {}",
        path.display(),
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "unknown error".to_string())
    );
}

fn assert_strict_schema_object(tool_name: &str, schema: &Value) {
    let schema_object = schema.as_object().expect("schema object");
    if schema_object.get("type") == Some(&Value::String("object".to_string())) {
        assert_eq!(
            schema_object.get("additionalProperties"),
            Some(&Value::Bool(false)),
            "{tool_name} object schema must reject unknown properties"
        );
        let properties = schema_object
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties object");
        let required = schema_object
            .get("required")
            .and_then(Value::as_array)
            .expect("required array");
        let property_names = properties
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let required_names = required
            .iter()
            .map(|name| name.as_str().expect("required name"))
            .collect::<BTreeSet<_>>();

        assert_eq!(
            required_names, property_names,
            "{tool_name} required keys must match object properties"
        );
    }

    if let Some(properties) = schema_object.get("properties").and_then(Value::as_object) {
        for value in properties.values() {
            assert_strict_schema_children(tool_name, value);
        }
    }
}

fn assert_strict_schema_children(tool_name: &str, schema: &Value) {
    if schema.get("type") == Some(&Value::String("object".to_string())) {
        assert_strict_schema_object(tool_name, schema);
    }
    if let Some(items) = schema.get("items") {
        assert_strict_schema_children(tool_name, items);
    }
}

fn memory_tool_test_context(
    workspace_dir: &Path,
    target_status: MemoryStatus,
) -> MemoryToolContext {
    let mut workspace_database =
        WorkspaceDatabase::open_or_create(workspace_dir).expect("workspace database");
    workspace_database
        .insert_chat("chat-1", "Memory tool test")
        .expect("chat insert");
    drop(workspace_database);

    MemoryToolContext {
        enabled: true,
        workspace_path: workspace_dir.to_path_buf(),
        global_memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        run_id: "run-1".to_string(),
        tool_call_id: "tool-call-1".to_string(),
        target_status,
        memory_settings: MemorySettings {
            enabled: true,
            extraction_mode: "pending_review".to_string(),
            retrieval_mode: "fts".to_string(),
            extraction_model_id: None,
            retrieval_model_id: None,
            retention_days: None,
            dream: MemoryDreamSettings::default(),
        },
    }
}

fn seed_memory_tool_fact(
    context: &MemoryToolContext,
    scope: MemoryScope,
    chat_id: Option<&str>,
    fact_id: &str,
    fact: &str,
) {
    let source_id = format!("{fact_id}-source");
    match scope {
        MemoryScope::Global => {
            let mut database =
                MemoryDatabase::open_or_create_global_at(&context.global_memory_database_file)
                    .expect("global memory database");
            insert_test_memory_fact(
                &mut database,
                &source_id,
                fact_id,
                scope,
                chat_id,
                fact,
                false,
            );
        }
        MemoryScope::Workspace | MemoryScope::Chat => {
            let mut database =
                MemoryDatabase::open_workspace_at(workspace_database_path(&context.workspace_path))
                    .expect("workspace memory database");
            insert_test_memory_fact(
                &mut database,
                &source_id,
                fact_id,
                scope,
                chat_id,
                fact,
                false,
            );
        }
    }
}

fn insert_test_memory_fact(
    database: &mut MemoryDatabase,
    source_id: &str,
    fact_id: &str,
    scope: MemoryScope,
    chat_id: Option<&str>,
    fact: &str,
    pinned: bool,
) {
    insert_test_memory_fact_with_status(
        database,
        source_id,
        fact_id,
        scope,
        chat_id,
        MemoryStatus::Active,
        fact,
        pinned,
    );
}

fn insert_test_memory_fact_with_status(
    database: &mut MemoryDatabase,
    source_id: &str,
    fact_id: &str,
    scope: MemoryScope,
    chat_id: Option<&str>,
    status: MemoryStatus,
    fact: &str,
    pinned: bool,
) {
    database
        .insert_source(NewMemorySource {
            id: source_id,
            scope,
            chat_id,
            source_type: foco_store::memory::MemorySourceType::ManualNote,
            source_id: None,
            title: "Test memory",
            content: fact,
            metadata_json: "{}",
        })
        .expect("memory source insert");
    database
        .insert_fact(NewMemoryFact {
            id: fact_id,
            scope,
            chat_id,
            status,
            kind: MemoryKind::ProjectFact,
            fact,
            confidence: None,
            pinned,
            source_ids: &[source_id],
            metadata_json: "{}",
        })
        .expect("memory fact insert");
}

fn prompt_test_config(workspace_dir: PathBuf) -> GlobalConfig {
    let mut config = GlobalConfig::first_run(workspace_dir);
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
        auto_sync_models: false,
        model_sync_filter_regex: None,
        request_overrides: Vec::new(),
        api_proxy: ApiProxySettings::default(),
    });
    config.models.push(ModelSettings {
        id: "model".to_string(),
        display_name: "Model".to_string(),
        enabled: true,
        provider_ids: vec!["provider".to_string()],
        active_provider_id: Some("provider".to_string()),
        thinking_level: None,
        system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
        metadata_key: None,
        metadata_source_url: None,
        metadata_refreshed_at: None,
        limits: Some(ModelLimits {
            context_window: 20_000,
            max_output_tokens: 1_000,
        }),
    });
    config
}

fn test_app_state(config: GlobalConfig, user_profile_dir: PathBuf) -> AppState {
    let (terminal_shutdown_tx, _) = broadcast::channel(1);
    let (_app_shutdown_tx, app_shutdown_rx) = watch::channel(false);
    let mcp_registry = Arc::new(McpRegistry::default());
    let foco_root_dir = user_profile_dir.join(".foco");
    let (agent_scheduler, _agent_scheduler_rx) = AgentScheduler::new();
    let (scheduled_task_scheduler, _scheduled_task_scheduler_rx) = ScheduledTaskScheduler::new();

    AppState {
        config: Arc::new(Mutex::new(config)),
        config_file: foco_root_dir.join("config.json"),
        memory_database_file: foco_store::memory::global_memory_database_path(
            foco_root_dir.clone(),
        ),
        model_metadata_file: foco_root_dir.join("models.dev.json"),
        listen_addr: SocketAddr::from(([127, 0, 0, 1], 3210)),
        ripgrep_install_lock: Arc::new(AsyncMutex::new(())),
        ripgrep_status: Arc::new(Mutex::new(detect_ripgrep(&foco_root_dir))),
        native_browser_authorizations: NativeBrowserAuthorizations::default(),
        user_profile_dir,
        terminal_registry: terminal::TerminalRegistry::default(),
        terminal_shutdown_tx,
        app_shutdown_rx,
        hook_runtime: HookRuntime::new(mcp_registry.clone()),
        mcp_registry,
        question_registry: QuestionRegistry::default(),
        active_chat_runs: ActiveChatRunRegistry::default(),
        memory_dream_runs: Arc::new(AsyncMutex::new(HashSet::new())),
        agent_scheduler,
        scheduled_task_scheduler,
        tool_resource_locks: ToolResourceLockRegistry::default(),
        code_graph_indexes: Arc::new(Mutex::new(CodeGraphIndexState::default())),
    }
}
