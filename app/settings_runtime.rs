use std::{collections::HashSet, path::Path, time::Instant};

use axum::Json;
use foco_agent::build_default_system_prompt;
use foco_providers::{DEFAULT_OPENAI_BASE_URL, OPENAI_CHAT_KIND, OPENAI_RESPONSES_KIND};
use foco_store::{
    config::{
        GlobalConfig, MAX_LLM_REQUEST_RETRY_COUNT, McpServerConfig, ModelSettings,
        ProviderSettings, SUPPORTED_API_PROXY_TYPES, SUPPORTED_APP_LANGUAGES, SUPPORTED_APP_THEMES,
        SUPPORTED_TERMINAL_SHELLS, WEB_SEARCH_PROVIDER_BRAVE, WEB_SEARCH_PROVIDER_TAVILY,
        WebSearchSettings, WorkspaceCommonCommand, WorkspaceConfig,
    },
    workspace::WorkspaceDatabase,
};

use crate::http::settings::known_agent_tool_names;
use crate::*;

pub(crate) async fn settings_response(
    state: &AppState,
    config: &GlobalConfig,
) -> Result<Json<SettingsResponse>, ApiError> {
    let active_workspace_id = config.app.active_workspace_id.clone();
    let mcp_statuses = state.mcp_registry.statuses(&active_workspace_id).await;
    let default_system_prompt = build_default_system_prompt();

    Ok(Json(SettingsResponse {
        general: GeneralSettingsSummary {
            auto_start_enabled: config.app.auto_start_enabled,
            default_team_mode_enabled: config.app.default_team_mode_enabled,
            web_server: WebServerSettingsSummary {
                listen_host: config.app.web_server.listen_host.clone(),
                listen_port: config.app.web_server.listen_port,
                password_enabled: web_auth_enabled(config),
            },
            llm_request_retry_count: config.app.llm_request_retry_count,
            max_llm_request_retry_count: MAX_LLM_REQUEST_RETRY_COUNT,
            language: config.app.language.clone(),
            theme: config.app.theme.clone(),
            hook_audit_enabled: config.hooks.audit_enabled,
            supported_languages: SUPPORTED_APP_LANGUAGES
                .iter()
                .map(|language| AppLanguageSummary {
                    id: *language,
                    name: app_language_name(*language),
                })
                .collect(),
            supported_themes: SUPPORTED_APP_THEMES
                .iter()
                .map(|theme| AppThemeSummary {
                    id: *theme,
                    name: app_theme_name(*theme),
                })
                .collect(),
        },
        agent_tools: {
            let mut tools = known_agent_tool_names(state, config)
                .await
                .into_iter()
                .collect::<Vec<_>>();
            tools.sort();
            tools
        },
        native_tools: NativeToolsSummary {
            browser_probe_port: state.listen_addr.port(),
            ripgrep: {
                let status = state
                    .ripgrep_status
                    .lock()
                    .map_err(|_| ApiError::internal("ripgrep status lock was poisoned"))?;
                ripgrep_tool_summary(&status)
            },
        },
        web_search: web_search_settings_summary(&config.web_search),
        memory: MemorySettingsSummary {
            enabled: config.memory.enabled,
            extraction_mode: config.memory.extraction_mode.clone(),
            retrieval_mode: config.memory.retrieval_mode.clone(),
            retention_days: config.memory.retention_days,
            extraction_model_id: config.memory.extraction_model_id.clone(),
            retrieval_model_id: config.memory.retrieval_model_id.clone(),
            dream: MemoryDreamSettingsSummary {
                enabled: config.memory.dream.enabled,
                auto_enabled: config.memory.dream.auto_enabled,
                mode: config.memory.dream.mode.clone(),
                model_id: config.memory.dream.model_id.clone(),
                workspace_interval_days: config.memory.dream.workspace_interval_days,
                global_interval_days: config.memory.dream.global_interval_days,
                create_transcript_chat: config.memory.dream.create_transcript_chat,
                max_facts_per_run: config.memory.dream.max_facts_per_run,
                max_changes_per_run: config.memory.dream.max_changes_per_run,
                scheduler_scan_minutes: config.memory.dream.scheduler_scan_minutes,
                workspace_threshold_facts: config.memory.dream.workspace_threshold_facts,
                global_threshold_facts: config.memory.dream.global_threshold_facts,
            },
            extraction_modes: vec![
                MemoryExtractionModeSummary {
                    value: "manual",
                    label: "Manual",
                },
                MemoryExtractionModeSummary {
                    value: "pending_review",
                    label: "Pending review",
                },
                MemoryExtractionModeSummary {
                    value: "automatic",
                    label: "Automatic",
                },
                MemoryExtractionModeSummary {
                    value: "disabled",
                    label: "Disabled",
                },
            ],
            retrieval_modes: vec![
                MemoryExtractionModeSummary {
                    value: "fts",
                    label: "SQLite FTS",
                },
                MemoryExtractionModeSummary {
                    value: "llm",
                    label: "Model matching",
                },
            ],
        },
        prompts: PromptSettingsSummary {
            system_prompt: config.prompts.system_prompt.clone(),
            default_system_prompt: default_system_prompt.clone(),
            system_prompts: system_prompt_summaries(&config.prompts, &default_system_prompt),
            files: config
                .prompts
                .files
                .iter()
                .map(|path| {
                    normalize_windows_verbatim_path(path.clone())
                        .display()
                        .to_string()
                })
                .collect(),
            extra_text: config.prompts.extra_text.clone(),
        },
        workspaces: config
            .workspaces
            .iter()
            .map(configured_workspace_summary)
            .collect::<Result<Vec<_>, _>>()?,
        terminal_shells: terminal_shell_summaries(),
        provider_kinds: vec![
            ProviderKindSummary {
                kind: OPENAI_CHAT_KIND,
                label: "OpenAI Chat",
                default_base_url: DEFAULT_OPENAI_BASE_URL,
            },
            ProviderKindSummary {
                kind: OPENAI_RESPONSES_KIND,
                label: "OpenAI Responses",
                default_base_url: DEFAULT_OPENAI_BASE_URL,
            },
        ],
        thinking_levels: vec![
            ThinkingLevelSummary {
                value: "minimal",
                label: "Minimal",
            },
            ThinkingLevelSummary {
                value: "low",
                label: "Low",
            },
            ThinkingLevelSummary {
                value: "medium",
                label: "Medium",
            },
            ThinkingLevelSummary {
                value: "high",
                label: "High",
            },
            ThinkingLevelSummary {
                value: "xhigh",
                label: "Extra High",
            },
        ],
        mcp_transports: vec![
            McpTransportSummary {
                transport: "stdio",
                label: "Stdio",
            },
            McpTransportSummary {
                transport: "streamable-http",
                label: "Streamable HTTP",
            },
        ],
        providers: config
            .providers
            .iter()
            .map(configured_provider_summary)
            .collect(),
        configured_models: config
            .models
            .iter()
            .map(|model| configured_model_summary_for_config(model, config))
            .collect(),
        mcp_servers: config
            .mcp
            .servers
            .iter()
            .map(|server| configured_mcp_server_summary(server, &mcp_statuses))
            .collect(),
        skills: skills_settings_summary(config, &state.user_profile_dir),
    }))
}

pub(crate) fn configured_workspace_summary(
    workspace: &WorkspaceConfig,
) -> Result<ConfiguredWorkspaceSummary, ApiError> {
    Ok(ConfiguredWorkspaceSummary {
        id: workspace.id.clone(),
        name: workspace.name.clone(),
        path: display_path(&workspace.path),
        logo_url: workspace_logo_url(workspace)?,
        pinned: workspace.pinned,
        terminal_shell: workspace.terminal_shell.clone(),
        common_commands: workspace_common_command_summaries(&workspace.common_commands),
        is_default: workspace.id == foco_store::config::DEFAULT_WORKSPACE_ID,
    })
}

pub(crate) fn workspace_common_command_summaries(
    commands: &[WorkspaceCommonCommand],
) -> Vec<WorkspaceCommonCommandSummary> {
    commands
        .iter()
        .map(|command| WorkspaceCommonCommandSummary {
            name: command.name.clone(),
            command: command.command.clone(),
        })
        .collect()
}

pub(crate) fn terminal_shell_summaries() -> Vec<TerminalShellSummary> {
    SUPPORTED_TERMINAL_SHELLS
        .iter()
        .map(|shell| TerminalShellSummary {
            shell: *shell,
            label: terminal_shell_label(shell),
        })
        .collect()
}

pub(crate) fn api_proxy_type_summaries() -> Vec<ApiProxyTypeSummary> {
    SUPPORTED_API_PROXY_TYPES
        .iter()
        .map(|proxy_type| ApiProxyTypeSummary {
            proxy_type: *proxy_type,
            label: api_proxy_type_label(proxy_type),
        })
        .collect()
}

pub(crate) fn api_proxy_type_label(proxy_type: &str) -> &'static str {
    match proxy_type {
        "http" => "HTTP",
        "socks" => "SOCKS",
        _ => "Unknown",
    }
}

pub(crate) fn terminal_shell_label(shell: &str) -> &'static str {
    match shell {
        "powershell" => "PowerShell",
        "cmd" => "Command Prompt",
        "bash" => "Bash",
        "zsh" => "Zsh",
        _ => "Unknown",
    }
}

pub(crate) fn configured_provider_summary(
    provider: &ProviderSettings,
) -> ConfiguredProviderSummary {
    ConfiguredProviderSummary {
        api_proxy: ApiProxySettingsSummary {
            enabled: provider.api_proxy.enabled,
            proxy_type: provider.api_proxy.proxy_type.clone(),
            url: provider.api_proxy.url.clone(),
            supported_types: api_proxy_type_summaries(),
        },
        id: provider.id.clone(),
        name: provider.name.clone(),
        kind: provider.kind.clone(),
        kind_label: provider_kind_label(&provider.kind),
        enabled: provider.enabled,
        base_url: provider.base_url.clone(),
        has_api_key: provider
            .api_key
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        request_overrides: provider.request_overrides.clone(),
        warnings: provider_warnings(provider),
    }
}

pub(crate) fn web_search_settings_summary(
    settings: &WebSearchSettings,
) -> WebSearchSettingsSummary {
    WebSearchSettingsSummary {
        enabled: settings.enabled,
        active_provider: settings.active_provider.clone(),
        api_proxy: ApiProxySettingsSummary {
            enabled: settings.api_proxy.enabled,
            proxy_type: settings.api_proxy.proxy_type.clone(),
            url: settings.api_proxy.url.clone(),
            supported_types: api_proxy_type_summaries(),
        },
        providers: vec![
            WebSearchProviderSummary {
                provider: WEB_SEARCH_PROVIDER_TAVILY,
                label: "Tavily",
                has_api_key: settings
                    .api_key_for_provider(WEB_SEARCH_PROVIDER_TAVILY)
                    .is_some(),
            },
            WebSearchProviderSummary {
                provider: WEB_SEARCH_PROVIDER_BRAVE,
                label: "Brave Search",
                has_api_key: settings
                    .api_key_for_provider(WEB_SEARCH_PROVIDER_BRAVE)
                    .is_some(),
            },
        ],
    }
}

pub(crate) fn configured_mcp_server_summary(
    server: &McpServerConfig,
    statuses: &[foco_mcp::McpServerStatus],
) -> ConfiguredMcpServerSummary {
    let status = statuses.iter().find(|status| status.id == server.id);
    let state = status
        .map(|status| mcp_server_state_name(status.state).to_string())
        .unwrap_or_else(|| {
            if server.enabled {
                "stopped".to_string()
            } else {
                "disabled".to_string()
            }
        });
    let error = status.and_then(|status| status.error.clone());
    let tool_count = status.map(|status| status.tool_count).unwrap_or(0);

    ConfiguredMcpServerSummary {
        id: server.id.clone(),
        name: server.name.clone(),
        enabled: server.enabled,
        transport: server.transport.clone(),
        transport_label: mcp_transport_label(&server.transport),
        command: server.command.clone(),
        args: server.args.clone(),
        url: server.url.clone(),
        state,
        error,
        tool_count,
        warnings: mcp_server_warnings(server),
    }
}

pub(crate) fn mcp_server_warnings(server: &McpServerConfig) -> Vec<String> {
    let mut warnings = Vec::new();

    if !server.enabled {
        warnings.push("MCP server is disabled.".to_string());
    }

    if let Err(error) = server.to_definition() {
        warnings.push(error.to_string());
    }

    warnings
}

pub(crate) fn skills_settings_summary(
    config: &GlobalConfig,
    user_profile_dir: &Path,
) -> SkillsSettingsSummary {
    let disabled_skill_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(user_profile_dir, &config.workspaces);
    let required_disabled_skill_ids = discovery
        .required_disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    SkillsSettingsSummary {
        directories: skill_search_roots(user_profile_dir, &config.workspaces)
            .iter()
            .map(|root| display_path(&root.directory))
            .collect(),
        detected: discovery
            .skills
            .iter()
            .map(|skill| {
                let can_enable = !skill_is_required_disabled(skill, &required_disabled_skill_ids);
                configured_skill_summary(
                    skill,
                    can_enable && !skill_is_disabled(skill, &disabled_skill_ids),
                    can_enable,
                )
            })
            .collect(),
        errors: discovery.errors,
    }
}

pub(crate) fn configured_skill_summary(
    skill: &SkillSettings,
    enabled: bool,
    can_enable: bool,
) -> ConfiguredSkillSummary {
    ConfiguredSkillSummary {
        key: skill.key.clone(),
        id: skill.id.clone(),
        name: skill.name.clone(),
        description: skill.description.clone(),
        path: skill.path.display().to_string(),
        scope: skill.scope.clone(),
        workspace_id: skill.workspace_id.clone(),
        workspace_name: skill.workspace_name.clone(),
        enabled,
        can_enable,
        warnings: skill_warnings(skill, enabled, can_enable),
    }
}

pub(crate) fn skill_warnings(
    skill: &SkillSettings,
    enabled: bool,
    can_enable: bool,
) -> Vec<String> {
    let mut warnings = Vec::new();

    if !enabled {
        warnings.push("Skill is disabled.".to_string());
    }

    if !can_enable {
        warnings
            .push("Skill frontmatter is invalid and must be fixed before enabling.".to_string());
    }

    if let Err(message) = parse_skill_file(&skill.path) {
        warnings.push(message);
    }

    warnings
}

pub(crate) fn configured_model_summary_for_config(
    model: &ModelSettings,
    config: &GlobalConfig,
) -> ConfiguredModelSummary {
    let mut summary = configured_model_summary(model);
    summary.supports_thinking = model_supports_thinking(model, config);
    summary.warnings = model_warnings(model, config, summary.can_enable, summary.supports_thinking);
    summary
}

pub(crate) fn workspace_response_from_config(
    config: &GlobalConfig,
    active_chat_runs: &ActiveChatRunRegistry,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let response_started_at = Instant::now();
    tracing::info!(
        workspace_count = config.workspaces.len(),
        "workspace response build started"
    );
    let mut workspaces = Vec::with_capacity(config.workspaces.len());

    for workspace in &config.workspaces {
        let workspace_started_at = Instant::now();
        tracing::info!(
            workspace_id = %workspace.id,
            workspace_path = %workspace.path.display(),
            "workspace summary build started"
        );
        let database_started_at = Instant::now();
        tracing::info!(
            workspace_id = %workspace.id,
            workspace_path = %workspace.path.display(),
            "workspace summary database open started"
        );
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        tracing::info!(
            workspace_id = %workspace.id,
            elapsed_ms = database_started_at.elapsed().as_millis() as u64,
            "workspace summary database opened"
        );
        let stats_started_at = Instant::now();
        tracing::info!(
            workspace_id = %workspace.id,
            "workspace summary code change stats started"
        );
        let code_change_stats_by_chat = database
            .chat_code_change_stats()
            .map_err(ApiError::from_workspace_error)?;
        tracing::info!(
            workspace_id = %workspace.id,
            chat_count = code_change_stats_by_chat.len(),
            elapsed_ms = stats_started_at.elapsed().as_millis() as u64,
            "workspace summary code change stats completed"
        );
        let chats_started_at = Instant::now();
        tracing::info!(
            workspace_id = %workspace.id,
            "workspace summary chats query started"
        );
        let chat_records = database.chats().map_err(ApiError::from_workspace_error)?;
        tracing::info!(
            workspace_id = %workspace.id,
            chat_count = chat_records.len(),
            elapsed_ms = chats_started_at.elapsed().as_millis() as u64,
            "workspace summary chats query completed"
        );
        let summaries_started_at = Instant::now();
        tracing::info!(
            workspace_id = %workspace.id,
            chat_count = chat_records.len(),
            "workspace summary chat summaries started"
        );
        let chats = chat_records
            .into_iter()
            .map(|chat| {
                let active_run = active_chat_runs.active_run_for_chat(&workspace.id, &chat.id)?;
                let code_change_stats = code_change_stats_by_chat
                    .get(&chat.id)
                    .cloned()
                    .unwrap_or_default();
                chat_summary(&mut database, chat, code_change_stats, active_run)
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        tracing::info!(
            workspace_id = %workspace.id,
            chat_count = chats.len(),
            elapsed_ms = summaries_started_at.elapsed().as_millis() as u64,
            "workspace summary chat summaries completed"
        );
        let logo_started_at = Instant::now();
        tracing::info!(
            workspace_id = %workspace.id,
            "workspace summary logo lookup started"
        );
        let logo_url = workspace_logo_url(workspace)?;
        tracing::info!(
            workspace_id = %workspace.id,
            elapsed_ms = logo_started_at.elapsed().as_millis() as u64,
            "workspace summary logo lookup completed"
        );

        workspaces.push(WorkspaceSummary {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
            path: display_path(&workspace.path),
            logo_url,
            pinned: workspace.pinned,
            terminal_shell: workspace.terminal_shell.clone(),
            common_commands: workspace_common_command_summaries(&workspace.common_commands),
            chats,
        });
        tracing::info!(
            workspace_id = %workspace.id,
            elapsed_ms = workspace_started_at.elapsed().as_millis() as u64,
            "workspace summary build completed"
        );
    }

    tracing::info!(
        workspace_count = workspaces.len(),
        elapsed_ms = response_started_at.elapsed().as_millis() as u64,
        "workspace response build completed"
    );
    Ok(Json(WorkspacesResponse {
        active_workspace_id: config.app.active_workspace_id.clone(),
        workspaces,
    }))
}

pub(crate) fn configured_model_summary(model: &ModelSettings) -> ConfiguredModelSummary {
    let context_window = model.limits.as_ref().map(|limits| limits.context_window);
    let max_output_tokens = model.limits.as_ref().map(|limits| limits.max_output_tokens);
    let mut missing_limits = Vec::new();

    if context_window.is_none() {
        missing_limits.push("contextWindow");
    }

    if max_output_tokens.is_none() {
        missing_limits.push("maxOutputTokens");
    }

    ConfiguredModelSummary {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        enabled: model.enabled,
        metadata_key: model.metadata_key.clone(),
        metadata_source_url: model.metadata_source_url.clone(),
        metadata_refreshed_at: model.metadata_refreshed_at.clone(),
        context_window,
        max_output_tokens,
        can_enable: missing_limits.is_empty(),
        missing_limits,
        provider_ids: model.provider_ids.clone(),
        active_provider_id: model.active_provider_id.clone(),
        thinking_level: model.thinking_level.clone(),
        system_prompt_name: model.system_prompt_name.clone(),
        supports_thinking: false,
        warnings: Vec::new(),
    }
}
