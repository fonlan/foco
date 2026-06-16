use super::*;
use std::collections::BTreeSet;

use foco_store::config::{DEFAULT_WORKSPACE_ID, DEFAULT_WORKSPACE_NAME};

fn test_neutral_tool_call(call_id: &str, name: &str, arguments: Value) -> NeutralToolCall {
    NeutralToolCall {
        call_id: call_id.to_string(),
        name: name.to_string(),
        arguments,
        thought_signatures: None,
    }
}

fn test_file_resource_lock(path: &str, access: ToolResourceAccess) -> ToolResourceLock {
    ToolResourceLock {
        resource: ToolResource::File(path.to_string()),
        access,
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
        memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        provider_id: "openai-responses".to_string(),
        model_id: "gpt-5.4".to_string(),
        user_message_id: "user-1".to_string(),
        assistant_message_id: "assistant-1".to_string(),
        llm_request_id: "request-1".to_string(),
        assistant_sequence: 1,
        provider_config: ProviderConnectionConfig {
            kind: foco_providers::ProviderKind::OpenAiResponses,
            base_url: None,
            api_key: Some("test-key".to_string()),
            proxy_url: None,
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
        initial_git_diff_stats: None,
        code_change_stats: CodeChangeStats::default(),
        pending_memory_retrieval: None,
    }
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
async fn tool_resource_registry_allows_different_file_writes() {
    let registry = ToolResourceLockRegistry::default();
    let _first_lease = registry
        .acquire(
            "workspace-1",
            vec![test_file_resource_lock(
                "src/a.rs",
                ToolResourceAccess::Write,
            )],
        )
        .await;

    let _second_lease = tokio::time::timeout(
        Duration::from_secs(1),
        registry.acquire(
            "workspace-1",
            vec![test_file_resource_lock(
                "src/b.rs",
                ToolResourceAccess::Write,
            )],
        ),
    )
    .await
    .expect("different file writes should not block each other");
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
        .acquire(
            "workspace-1",
            vec![ToolResourceLock {
                resource: ToolResource::WorkspaceFiles,
                access: ToolResourceAccess::Exclusive,
            }],
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
        ToolCancellationToken::new(),
    )
    .await;

    let error = match result {
        Ok(_) => panic!("lock wait should time out"),
        Err(error) => error,
    };
    assert!(error.contains("timed out waiting for resource lock"));
    assert!(started.elapsed() < Duration::from_secs(1));
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
        &ProviderConnectionConfig {
            kind: foco_providers::ProviderKind::OpenAiResponses,
            base_url: None,
            api_key: Some("test-key".to_string()),
            proxy_url: None,
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
        registry,
        ToolCancellationToken::new(),
        "workspace-1",
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
            open: "鎵撳紑 Foco",
            quit: "閫€鍑?Foco",
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
    let watchers = Arc::new(Mutex::new(Vec::new()));

    let thread = spawn_code_graph_index_initialization(workspaces, watchers.clone())
        .expect("spawn code graph initialization");
    thread.join().expect("code graph initialization thread");

    assert_eq!(
        watchers.lock().expect("watcher lock").len(),
        1,
        "watcher must be retained after background indexing"
    );
    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let context = database.code_graph_context().expect("code graph context");
    assert_eq!(context.indexed_files, 1);
    drop(database);
    watchers.lock().expect("watcher lock").clear();
    remove_dir_if_exists(&workspace_dir);
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

    for index in (READ_ONLY_TOOL_BATCH_WARNING_THRESHOLD + 1)
        ..(READ_ONLY_TOOL_BATCH_WARNING_THRESHOLD + 8)
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
            auto_start_enabled: None,
            clear_password: None,
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
            auto_start_enabled: None,
            clear_password: None,
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
            auto_start_enabled: None,
            clear_password: Some(true),
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
    let workspace_dir =
        env::temp_dir().join(unique_id("foco-skill-frontmatter-workspace-test"));
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

    let messages =
        enabled_skill_frontmatter_messages(&profile_dir, &config, DEFAULT_WORKSPACE_ID)
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

    let messages =
        enabled_skill_frontmatter_messages(&profile_dir, &config, DEFAULT_WORKSPACE_ID)
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
    let messages =
        enabled_skill_frontmatter_messages(&profile_dir, &config, DEFAULT_WORKSPACE_ID)
            .expect("enabled skill frontmatter messages");

    assert_eq!(messages.len(), 1);
    assert!(messages[0].content.contains("name: gitmemo"));
    assert!(!messages[0].content.contains("name: broken"));

    fs::remove_dir_all(profile_dir).expect("remove skill test profile");
}

#[test]
fn selected_invalid_skill_reports_disabled_before_frontmatter_error() {
    let profile_dir = env::temp_dir().join(unique_id("foco-selected-invalid-skill-profile"));
    let workspace_dir =
        env::temp_dir().join(unique_id("foco-selected-invalid-skill-workspace"));
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
            .pending
            .lock()
            .expect("question registry lock")
            .contains_key("question-1")
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
            .pending
            .lock()
            .expect("question registry lock")
            .contains_key("question-1")
    );
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
    let mut config =
        GlobalConfig::first_run(PathBuf::from(r"\\?\C:\Users\fonla\.foco\workspace"));
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

    assert_eq!(agents_messages.len(), 1);
    assert_eq!(agents_messages[0].role, NeutralChatRole::User);
    assert!(agents_messages[0].content.contains(AGENTS_MESSAGE_PREFIX));
    assert!(
        agents_messages[0]
            .content
            .contains("Workspace instructions.")
    );
    assert_eq!(prompt_messages.len(), 2);
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
    assert!(
        prompt_messages[1]
            .content
            .contains(EXTRA_PROMPT_MESSAGE_PREFIX)
    );
    assert!(
        prompt_messages[1]
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
    assert!(
        compress_runtime_tool_state_if_needed(&mut context, true).expect("second compression")
    );
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
fn neutral_messages_from_record_replays_saved_tool_state_in_order() {
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

    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
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
fn user_attachments_round_trip_into_neutral_history_and_message_parts() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-user-attachment-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
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
fn persist_chat_result_writes_audit_status_code() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-audit-status-code-test"));
    fs::create_dir_all(&workspace_dir).expect("workspace directory");
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
    }
    let (_app_shutdown_tx, app_shutdown_rx) = watch::channel(false);
    let mcp_registry = Arc::new(McpRegistry::default());
    let context = PreparedChatContext {
        workspace_id: "workspace-1".to_string(),
        workspace_path: workspace_dir.clone(),
        memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        provider_id: "openai-responses".to_string(),
        model_id: "gpt-5.4".to_string(),
        user_message_id: "user-1".to_string(),
        assistant_message_id: "assistant-1".to_string(),
        llm_request_id: "request-1".to_string(),
        assistant_sequence: 1,
        provider_config: ProviderConnectionConfig {
            kind: foco_providers::ProviderKind::OpenAiResponses,
            base_url: None,
            api_key: Some("test-key".to_string()),
            proxy_url: None,
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
        initial_git_diff_stats: None,
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

    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let request = database
        .llm_request("request-1")
        .expect("llm request read")
        .expect("llm request");

    assert_eq!(request.status_code, Some(200));
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
        memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        provider_id: "openai-responses".to_string(),
        model_id: "gpt-5.4".to_string(),
        user_message_id: "user-1".to_string(),
        assistant_message_id: "assistant-1".to_string(),
        llm_request_id: "run-1".to_string(),
        assistant_sequence: 1,
        provider_config: ProviderConnectionConfig {
            kind: foco_providers::ProviderKind::OpenAiResponses,
            base_url: None,
            api_key: Some("test-key".to_string()),
            proxy_url: None,
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
        initial_git_diff_stats: None,
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

    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    assert!(
        database
            .llm_request("run-1")
            .expect("run audit lookup")
            .is_none()
    );
    assert!(
        database
            .llm_request("request-1")
            .expect("first request lookup")
            .is_some()
    );
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

    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
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
fn persist_failed_chat_result_keeps_tool_calls_without_assistant_message() {
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
        memory_database_file: workspace_dir.join("global-memory.sqlite"),
        chat_id: "chat-1".to_string(),
        provider_id: "openai-responses".to_string(),
        model_id: "gpt-5.4".to_string(),
        user_message_id: "user-1".to_string(),
        assistant_message_id: "assistant-1".to_string(),
        llm_request_id: "request-1".to_string(),
        assistant_sequence: 1,
        provider_config: ProviderConnectionConfig {
            kind: foco_providers::ProviderKind::OpenAiResponses,
            base_url: None,
            api_key: Some("test-key".to_string()),
            proxy_url: None,
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
        initial_git_diff_stats: None,
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

    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let request = database
        .llm_request("request-1")
        .expect("llm request read")
        .expect("llm request");
    let messages = database
        .messages_for_chat("chat-1")
        .expect("chat messages read");

    assert_eq!(request.final_state, "failed");
    assert!(messages.is_empty());

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
    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
    let run_events = database
        .run_events_for_run("run-1")
        .expect("run events for run");
    assert_eq!(run_events.len(), 2);
    assert!(registry.subscribe("workspace-1", "run-1", Some(0)).is_err());
    drop(database);
    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
}

#[test]
fn git_diff_changed_files_lists_only_files_changed_since_turn_start() {
    let initial = GitDiffResponse {
        path: None,
        status: String::new(),
        staged_diff: String::new(),
        diff: [
            "diff --git a/README.md b/README.md",
            "--- a/README.md",
            "+++ b/README.md",
            "@@ -1 +1 @@",
            "-old",
            "+new",
            "",
        ]
        .join("\n"),
        files: Vec::new(),
    };
    let final_diff = GitDiffResponse {
        path: None,
        status: String::new(),
        staged_diff: String::new(),
        diff: [
            "diff --git a/README.md b/README.md",
            "--- a/README.md",
            "+++ b/README.md",
            "@@ -1 +1 @@",
            "-old",
            "+new",
            "diff --git a/app/main.rs b/app/main.rs",
            "--- a/app/main.rs",
            "+++ b/app/main.rs",
            "@@ -0,0 +1,2 @@",
            "+line one",
            "+line two",
            "",
        ]
        .join("\n"),
        files: Vec::new(),
    };

    let changed_files =
        git_diff_changed_files(&git_diff_stats(&initial), &git_diff_stats(&final_diff));

    assert_eq!(changed_files.len(), 1);
    assert_eq!(changed_files[0].0, "app/main.rs");
    assert_eq!(changed_files[0].1.additions, 2);
    assert_eq!(changed_files[0].1.deletions, 0);

    let cleared_files = git_diff_changed_files(&git_diff_stats(&initial), &BTreeMap::new());

    assert_eq!(cleared_files.len(), 1);
    assert_eq!(cleared_files[0].0, "README.md");
    assert_eq!(cleared_files[0].1.additions, 0);
    assert_eq!(cleared_files[0].1.deletions, 0);
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
    let search = memory_prompt_search("鍏紡娓叉煋鐜板湪鏀寔鍚?)
        .expect("prompt search should keep CJK terms");

    assert!(search.contains_terms.contains(&"鍏紡娓叉煋".to_string()));
    assert!(search.contains_terms.contains(&"娓叉煋".to_string()));
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
        512,
        &evidence,
        &existing,
    )
    .expect("memory extraction request");

    assert_eq!(request.messages[0].role, NeutralChatRole::System);
    assert!(
        request.messages[0]
            .content
            .contains("unlikely to change often")
    );
    assert!(
        request.messages[0]
            .content
            .contains("duplicates or near-duplicates")
    );
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

    let candidates = memory_extraction_existing_memory_candidates(
        &global_memory,
        &workspace_memory,
        "chat-1",
    )
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

    let facts = validate_extracted_memory_facts(&output, &evidence_by_id)
        .expect("valid extracted fact");

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
    };
    let automatic_settings = MemorySettings {
        enabled: true,
        extraction_mode: "automatic".to_string(),
        retrieval_mode: "fts".to_string(),
        retention_days: None,
        extraction_model_id: None,
        retrieval_model_id: None,
    };
    let manual_settings = MemorySettings {
        enabled: true,
        extraction_mode: "manual".to_string(),
        retrieval_mode: "fts".to_string(),
        retention_days: None,
        extraction_model_id: None,
        retrieval_model_id: None,
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

    let message = database
        .messages_for_chat("chat-1")
        .expect("messages")
        .into_iter()
        .next()
        .expect("assistant message");
    let events = database
        .llm_request_events_for_chat("chat-1")
        .expect("llm request events");
    let summary = chat_message_summary(&database, &workspace_dir, None, message, &events)
        .expect("message summary");
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

    let message = database
        .messages_for_chat("chat-1")
        .expect("messages")
        .into_iter()
        .next()
        .expect("assistant message");
    let summary = chat_message_summary(&database, &workspace_dir, None, message, &[])
        .expect("message summary");

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

    let message = database
        .messages_for_chat("chat-1")
        .expect("messages")
        .into_iter()
        .next()
        .expect("assistant message");
    let events = database
        .llm_request_events_for_chat("chat-1")
        .expect("llm request events");
    let summary = chat_message_summary(&database, &workspace_dir, None, message, &events)
        .expect("message summary");
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
    let existing_workspace_dir =
        env::temp_dir().join(unique_id("foco-existing-workspace-test"));
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
                general_purpose::STANDARD
                    .encode([0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
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
        response_workspace.logo_url.as_deref().is_some_and(
            |logo_url| logo_url.starts_with("/api/workspaces/new-workspace/logo?v=")
        )
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

    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
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
            .contains("Configured prompt chat instructions.")
    );
    assert!(
        prompt_messages[1]
            .content
            .contains("Extra configured prompt.")
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
    }

    let existing_context = prepare_chat_context(
        &state,
        &config,
        &config.workspaces[0].id,
        ChatStreamRequest {
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
    assert_eq!(
        existing_prompt_messages[0].content,
        prompt_messages[0].content
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
    assert_eq!(
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
    let final_request_json: Value = serde_json::from_str(&context.request_body_json)
        .expect("final request body is valid JSON");
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
    let profile_dir =
        env::temp_dir().join(unique_id("foco-memory-stream-failure-profile-test"));

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
    let profile_dir =
        env::temp_dir().join(unique_id("foco-memory-tools-disabled-profile-test"));

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
async fn prepare_prompt_context_hides_search_text_when_ripgrep_unavailable() {
    let workspace_dir = env::temp_dir().join(unique_id("foco-ripgrep-tools-disabled-test"));
    let profile_dir =
        env::temp_dir().join(unique_id("foco-ripgrep-tools-disabled-profile-test"));

    fs::create_dir_all(&workspace_dir).expect("workspace directory");

    let mut config = GlobalConfig::first_run(workspace_dir.clone());
    config.providers.push(ProviderSettings {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        kind: OPENAI_CHAT_KIND.to_string(),
        enabled: true,
        base_url: None,
        api_key: None,
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
        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
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
                    summary: "Implementation was started before the run was cancelled."
                        .to_string(),
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
            chat_id: Some("chat-1".to_string()),
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("缁х画瀹屾垚鍓╀笅鐨勫伐浣?.to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
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
        .position(|message| message.content == "缁х画瀹屾垚鍓╀笅鐨勫伐浣?)
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
        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
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
        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
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
    assert_eq!(
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
        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
                .expect("workspace memory database");
        insert_test_memory_fact(
            &mut memory,
            "source-cjk-formula",
            "fact-cjk-formula",
            MemoryScope::Workspace,
            None,
            "Markdown 棰勮宸茬粡鏀寔鍏紡娓叉煋銆?,
            false,
        );
        insert_test_memory_fact(
            &mut memory,
            "source-cjk-unrelated",
            "fact-cjk-unrelated",
            MemoryScope::Workspace,
            None,
            "璐﹀崟鍙戠エ浣跨敤鏈堝害璐㈠姟鏍囩銆?,
            false,
        );
    }

    let mut context = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            chat_id: Some("chat-1".to_string()),
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("鐜板湪 markdown 棰勮鏀寔鍏紡鍚楋紵".to_string()),
            assistant_draft: None,
            assistant_draft_reasoning: None,
            attachments: Vec::new(),
        },
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
    assert!(request_text.contains("Markdown 棰勮宸茬粡鏀寔鍏紡娓叉煋銆?));
    assert!(!request_text.contains("璐﹀崟鍙戠エ浣跨敤鏈堝害璐㈠姟鏍囩銆?));

    fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    remove_dir_if_exists(&profile_dir);
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
        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
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
        PromptAssemblyPurpose::ContextPreview,
    )
    .await
    .expect("prompt context");
    let usage = context_usage_response(&prompt_context, None).expect("context usage");

    assert!(usage.used_message_tokens > 0);
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
        context_compression_trigger_tokens(usage.available_message_tokens)
    );
    assert_eq!(usage.compression_trigger_percent, 80);
    let prompt_context_with_assistant = prepare_prompt_context(
        &state,
        &config,
        &config.workspaces[0].id,
        PromptContextRequest {
            chat_id: None,
            model_id: "model".to_string(),
            provider_id: None,
            thinking_level: None,
            skill_ids: None,
            message: Some("Preview workspace memory usage".to_string()),
            assistant_draft: Some("Streaming assistant reply adds context.".to_string()),
            assistant_draft_reasoning: Some(
                "Streaming reasoning also adds context.".to_string(),
            ),
            attachments: Vec::new(),
        },
        PromptAssemblyPurpose::ContextPreview,
    )
    .await
    .expect("prompt context with assistant draft");
    let usage_with_assistant =
        context_usage_response(&prompt_context_with_assistant, None).expect("context usage");
    assert!(usage_with_assistant.used_message_tokens > usage.used_message_tokens);

    let response_input_tokens = prompt_context_with_assistant
        .context_budget
        .system_prompt_tokens
        + prompt_context_with_assistant
            .context_budget
            .tool_schema_tokens
        + 1_500;
    let usage_from_response = context_usage_response(
        &prompt_context_with_assistant,
        Some(&NeutralUsage {
            input_tokens: Some(response_input_tokens as i64),
            output_tokens: Some(250),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
        }),
    )
    .expect("context usage from response usage");
    assert_eq!(usage_from_response.used_message_tokens, 1_750);

    let database =
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
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
        let mut memory =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
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
    if path.exists() {
        fs::remove_dir_all(path).expect("remove test directory");
    }
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
            let mut database = MemoryDatabase::open_workspace_at(workspace_database_path(
                &context.workspace_path,
            ))
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
        tool_resource_locks: ToolResourceLockRegistry::default(),
        _code_graph_watchers: Arc::new(Mutex::new(Vec::new())),
    }
}
