use std::{collections::HashSet, path::Path, time::Instant};

use axum::Json;
use base64::{Engine as _, engine::general_purpose};
use foco_agent::build_default_system_prompt;
use foco_providers::supported_provider_kinds;
use foco_store::{
    config::{
        DEFAULT_SYSTEM_PROMPT_NAME, GlobalConfig, MAX_LLM_REQUEST_RETRY_COUNT, McpServerConfig,
        ModelSettings, PLAN_MERGE_AUTOMATION_DIRECT_AUTO, PLAN_MERGE_AUTOMATION_ISOLATED_AUTO_ONCE,
        PLAN_MODE_SYSTEM_PROMPT_NAME, ProviderSettings, REVIEW_SYSTEM_PROMPT_NAME,
        SUPPORTED_API_PROXY_TYPES, SUPPORTED_APP_LANGUAGES, SUPPORTED_APP_THEMES,
        SUPPORTED_TERMINAL_SHELLS, WEB_SEARCH_PROVIDER_BRAVE, WEB_SEARCH_PROVIDER_TAVILY,
        WebSearchSettings, WorkspaceCommonCommand, WorkspaceConfig,
    },
    workspace::{ChatPageCursor, WorkspaceDatabase},
};

use crate::http::settings::{
    AboutSettingsSummary, ApiAuditSettingsSummary, ApiProxySettingsSummary, ApiProxyTypeSummary,
    AppLanguageSummary, AppThemeSummary, ConfiguredMcpServerSummary, ConfiguredModelSummary,
    ConfiguredProviderSummary, ConfiguredSkillStoreSummary, ConfiguredSkillSummary,
    ConfiguredWorkspaceSummary, GeneralSettingsSummary, IMAGE_AGENT_SYSTEM_PROMPT_NAME,
    McpTransportSummary, MemoryDreamSettingsSummary, MemoryExtractionModeSummary,
    MemorySettingsSummary, NativeToolsSummary, PlanMergeAutomationModeSummary, PlanSettingsSummary,
    PromptSettingsSummary, ProviderKindSummary, SettingsResponse, SkillsSettingsSummary,
    SpecSettingsSummary, SystemPromptSummary, TerminalShellSummary, ThinkingLevelSummary,
    WebSearchProviderSummary, WebSearchSettingsSummary, WebServerSettingsSummary,
    WorkspaceCommonCommandSummary, default_image_agent_system_prompt_for_config,
    default_plan_mode_system_prompt, default_review_system_prompt, known_agent_tool_names,
};
use crate::platform::autostart_windows::auto_start_enabled_for_response;
use crate::*;

pub(crate) async fn settings_response(
    state: &AppState,
    config: &GlobalConfig,
) -> Result<Json<SettingsResponse>, ApiError> {
    let active_workspace_id = config.app.active_workspace_id.clone();
    let mcp_statuses = state.mcp_registry.statuses(&active_workspace_id).await;
    let default_system_prompt = build_default_system_prompt();

    Ok(Json(SettingsResponse {
        app_version: crate::update_runtime::current_version().to_string(),
        general: GeneralSettingsSummary {
            auto_start_enabled: auto_start_enabled_for_response(config.app.auto_start_enabled),
            default_team_mode_enabled: config.app.default_team_mode_enabled,
            web_server: WebServerSettingsSummary {
                listen_host: config.app.web_server.listen_host.clone(),
                listen_port: config.app.web_server.listen_port,
                password_enabled: web_auth_enabled(config),
            },
            llm_request_retry_count: config.app.llm_request_retry_count,
            max_llm_request_retry_count: MAX_LLM_REQUEST_RETRY_COUNT,
            api_audit: ApiAuditSettingsSummary {
                request_detail_retention_days: config.app.api_audit.request_detail_retention_days,
                save_request_response_details: config.app.api_audit.save_request_response_details,
            },
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
            extraction_llm_timeout_ms: config.memory.extraction_llm_timeout_ms,
            retrieval_llm_timeout_ms: config.memory.retrieval_llm_timeout_ms,
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
                llm_timeout_ms: config.memory.dream.llm_timeout_ms,
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
        spec: SpecSettingsSummary {
            auto_enabled: config.spec.auto_enabled,
            generation_model_id: config.spec.generation_model_id.clone(),
            generation_system_prompt: config.spec.generation_system_prompt.clone(),
            update_system_prompt: config.spec.update_system_prompt.clone(),
            default_generation_system_prompt:
                crate::spec_runtime::default_workspace_spec_generation_system_prompt(),
            default_update_system_prompt:
                crate::spec_runtime::default_workspace_spec_update_system_prompt(),
            llm_timeout_ms: config.spec.llm_timeout_ms,
        },
        plan: PlanSettingsSummary {
            merge_automation_mode: config.plan.merge_automation_mode.clone(),
            merge_automation_modes: vec![
                PlanMergeAutomationModeSummary {
                    value: PLAN_MERGE_AUTOMATION_ISOLATED_AUTO_ONCE,
                    label: "Isolated auto once",
                },
                PlanMergeAutomationModeSummary {
                    value: PLAN_MERGE_AUTOMATION_DIRECT_AUTO,
                    label: "Direct auto",
                },
            ],
        },
        prompts: PromptSettingsSummary {
            system_prompt: config.prompts.system_prompt.clone(),
            default_system_prompt: default_system_prompt.clone(),
            default_image_generation_system_prompt: default_image_agent_system_prompt_for_config(
                config,
            )?,
            default_plan_mode_system_prompt: default_plan_mode_system_prompt(),
            default_review_system_prompt: default_review_system_prompt(),
            system_prompts: settings_system_prompt_summaries(config, &default_system_prompt)?,
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
            .map(|workspace| {
                configured_workspace_summary(
                    workspace,
                    remote_server_for_workspace(config, workspace),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        remote_servers: crate::http::remote_servers::remote_server_summaries(
            config,
            &crate::http::remote_servers::connected_remote_server_ids(state)?,
            Some(state),
        ),
        terminal_shells: terminal_shell_summaries(),
        provider_kinds: supported_provider_kinds()
            .iter()
            .map(|kind| ProviderKindSummary {
                kind: kind.as_str(),
                label: kind.label(),
                default_base_url: kind.default_base_url(),
            })
            .collect(),
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
        about: AboutSettingsSummary {
            version: crate::update_runtime::current_version().to_string(),
        },
        update: crate::update_runtime::update_status_summary(state, config)?,
    }))
}

fn settings_system_prompt_summaries(
    config: &GlobalConfig,
    default_system_prompt: &str,
) -> Result<Vec<SystemPromptSummary>, ApiError> {
    let mut summaries = system_prompt_summaries(&config.prompts, default_system_prompt);
    summaries.retain(|prompt| prompt.name != IMAGE_AGENT_SYSTEM_PROMPT_NAME);
    if !summaries
        .iter()
        .any(|prompt| prompt.name == PLAN_MODE_SYSTEM_PROMPT_NAME)
    {
        let insert_at = summaries
            .iter()
            .position(|prompt| prompt.name == DEFAULT_SYSTEM_PROMPT_NAME)
            .map(|index| index + 1)
            .unwrap_or(summaries.len());
        summaries.insert(
            insert_at,
            SystemPromptSummary {
                name: PLAN_MODE_SYSTEM_PROMPT_NAME.to_string(),
                content: default_plan_mode_system_prompt(),
            },
        );
    }
    if !summaries
        .iter()
        .any(|prompt| prompt.name == REVIEW_SYSTEM_PROMPT_NAME)
    {
        let insert_at = summaries
            .iter()
            .position(|prompt| prompt.name == PLAN_MODE_SYSTEM_PROMPT_NAME)
            .map(|index| index + 1)
            .or_else(|| {
                summaries
                    .iter()
                    .position(|prompt| prompt.name == DEFAULT_SYSTEM_PROMPT_NAME)
                    .map(|index| index + 1)
            })
            .unwrap_or(summaries.len());
        summaries.insert(
            insert_at,
            SystemPromptSummary {
                name: REVIEW_SYSTEM_PROMPT_NAME.to_string(),
                content: default_review_system_prompt(),
            },
        );
    }
    Ok(summaries)
}

pub(crate) fn configured_workspace_summary(
    workspace: &WorkspaceConfig,
    server: Option<&foco_store::config::RemoteServerProfile>,
) -> Result<ConfiguredWorkspaceSummary, ApiError> {
    let remote = remote_workspace_fields(workspace, server, None);
    Ok(ConfiguredWorkspaceSummary {
        id: workspace.id.clone(),
        name: workspace.name.clone(),
        path: workspace.display_path(server),
        display_path: workspace.display_path(server),
        server_id: remote.server_id,
        server_name: remote.server_name,
        remote_path: remote.remote_path,
        connection_status: remote.connection_status,
        last_remote_error: remote.last_remote_error,
        logo_url: workspace_logo_url(workspace)?,
        pinned: workspace.pinned,
        terminal_shell: workspace.terminal_shell.clone(),
        common_commands: workspace_common_command_summaries(&workspace.common_commands),
        is_default: workspace.id == foco_store::config::DEFAULT_WORKSPACE_ID,
    })
}

pub(crate) struct RemoteWorkspaceResponseFields {
    pub(crate) server_id: Option<String>,
    pub(crate) server_name: Option<String>,
    pub(crate) remote_path: Option<String>,
    pub(crate) connection_status: String,
    pub(crate) last_remote_error: Option<String>,
}

pub(crate) fn remote_workspace_fields(
    workspace: &WorkspaceConfig,
    server: Option<&foco_store::config::RemoteServerProfile>,
    remote_manager: Option<&crate::remote_workspace::RemoteWorkspaceManager>,
) -> RemoteWorkspaceResponseFields {
    let live_state = workspace.server_id().and_then(|server_id| {
        remote_manager.and_then(|manager| {
            manager
                .workspace_state(server_id, &workspace.id)
                .ok()
                .flatten()
        })
    });
    RemoteWorkspaceResponseFields {
        server_id: workspace.server_id().map(str::to_string),
        server_name: server.map(remote_server_display_name),
        remote_path: workspace.remote_path().map(str::to_string),
        connection_status: live_state
            .map(|state| state.as_str())
            .or_else(|| {
                server.map(|server| {
                    if server
                        .last_error
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty())
                    {
                        "offline"
                    } else if server.last_checked_at.is_some() {
                        "ready"
                    } else {
                        "disconnected"
                    }
                })
            })
            .unwrap_or(if workspace.is_remote() {
                "missingServer"
            } else {
                "local"
            })
            .to_string(),
        last_remote_error: server.and_then(|server| server.last_error.clone()),
    }
}

pub(crate) fn remote_server_display_name(
    server: &foco_store::config::RemoteServerProfile,
) -> String {
    let name = server.name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    server.host_alias.clone()
}

fn remote_server_for_workspace<'a>(
    config: &'a GlobalConfig,
    workspace: &WorkspaceConfig,
) -> Option<&'a foco_store::config::RemoteServerProfile> {
    let server_id = workspace.server_id()?;
    config
        .remote_servers
        .iter()
        .find(|server| server.id == server_id)
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
        auto_sync_models: provider.auto_sync_models,
        model_sync_filter_regex: provider.model_sync_filter_regex.clone(),
        request_overrides: provider.request_overrides.clone(),
        model_redirects: provider.model_redirects.clone(),
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
        execution_host: match server.execution_host {
            foco_store::config::McpExecutionHost::Auto => "auto",
            foco_store::config::McpExecutionHost::Local => "local",
            foco_store::config::McpExecutionHost::Workspace => "workspace",
        }
        .to_string(),
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
        translation_model_id: config.skills.translation_model_id.clone(),
    }
}

pub(crate) fn configured_skill_summary(
    skill: &SkillSettings,
    enabled: bool,
    can_enable: bool,
) -> ConfiguredSkillSummary {
    let store = crate::http::skill_store::skill_store_metadata_for_skill(skill).map(|metadata| {
        ConfiguredSkillStoreSummary {
            skill_id: metadata.skill_id,
            source: metadata.source,
            updateable: true,
        }
    });
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
        store,
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

pub(crate) const WORKSPACE_CHAT_PAGE_LIMIT: usize = 5;

pub(crate) fn workspace_response_from_config(
    config: &GlobalConfig,
    active_chat_runs: &ActiveChatRunRegistry,
    remote_manager: Option<&crate::remote_workspace::RemoteWorkspaceManager>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let response_started_at = Instant::now();
    tracing::debug!(
        workspace_count = config.workspaces.len(),
        "workspace response build started"
    );
    let mut workspaces = Vec::with_capacity(config.workspaces.len());

    for workspace in &config.workspaces {
        let workspace_started_at = Instant::now();
        tracing::debug!(
            workspace_id = %workspace.id,
            workspace_path = %workspace.path.display(),
            "workspace summary build started"
        );
        if workspace.is_remote() {
            let server = remote_server_for_workspace(config, workspace);
            let remote = remote_workspace_fields(workspace, server, remote_manager);
            workspaces.push(WorkspaceSummary {
                id: workspace.id.clone(),
                name: workspace.name.clone(),
                path: workspace.display_path(server),
                display_path: workspace.display_path(server),
                server_id: remote.server_id,
                server_name: remote.server_name,
                remote_path: remote.remote_path,
                connection_status: remote.connection_status,
                last_remote_error: remote.last_remote_error,
                logo_url: None,
                pinned: workspace.pinned,
                terminal_shell: workspace.terminal_shell.clone(),
                common_commands: workspace_common_command_summaries(&workspace.common_commands),
                chats: Vec::new(),
                chat_pagination: WorkspaceChatPagination {
                    total: 0,
                    limit: WORKSPACE_CHAT_PAGE_LIMIT,
                    has_more: false,
                    next_cursor: None,
                },
            });
            tracing::debug!(
                workspace_id = %workspace.id,
                elapsed_ms = workspace_started_at.elapsed().as_millis() as u64,
                "remote workspace summary deferred to sidecar"
            );
            continue;
        }

        let database_started_at = Instant::now();
        tracing::debug!(
            workspace_id = %workspace.id,
            workspace_path = %workspace.path.display(),
            "workspace summary database open started"
        );
        let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        tracing::debug!(
            workspace_id = %workspace.id,
            elapsed_ms = database_started_at.elapsed().as_millis() as u64,
            "workspace summary database opened"
        );
        let chats_started_at = Instant::now();
        tracing::debug!(
            workspace_id = %workspace.id,
            "workspace summary chats query started"
        );
        let chat_page = database
            .chat_page(WORKSPACE_CHAT_PAGE_LIMIT, None)
            .map_err(ApiError::from_workspace_error)?;
        tracing::debug!(
            workspace_id = %workspace.id,
            chat_count = chat_page.chats.len(),
            elapsed_ms = chats_started_at.elapsed().as_millis() as u64,
            "workspace summary chats query completed"
        );
        let summaries_started_at = Instant::now();
        tracing::debug!(
            workspace_id = %workspace.id,
            chat_count = chat_page.chats.len(),
            "workspace summary chat summaries started"
        );
        let chat_ids = chat_page
            .chats
            .iter()
            .map(|chat| chat.id.clone())
            .collect::<Vec<_>>();
        let code_change_stats_by_chat = database
            .code_change_stats_for_chats(&chat_ids)
            .map_err(ApiError::from_workspace_error)?;
        let chats = chat_page
            .chats
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
        let chat_pagination = WorkspaceChatPagination {
            total: chat_page.total_count,
            limit: WORKSPACE_CHAT_PAGE_LIMIT,
            has_more: chat_page.has_more,
            next_cursor: chat_page.next_cursor.map(encode_workspace_chat_cursor),
        };
        tracing::debug!(
            workspace_id = %workspace.id,
            chat_count = chats.len(),
            elapsed_ms = summaries_started_at.elapsed().as_millis() as u64,
            "workspace summary chat summaries completed"
        );
        let logo_started_at = Instant::now();
        tracing::debug!(
            workspace_id = %workspace.id,
            "workspace summary logo lookup started"
        );
        let logo_url = workspace_logo_url(workspace)?;
        tracing::debug!(
            workspace_id = %workspace.id,
            elapsed_ms = logo_started_at.elapsed().as_millis() as u64,
            "workspace summary logo lookup completed"
        );

        let server = remote_server_for_workspace(config, workspace);
        let remote = remote_workspace_fields(workspace, server, remote_manager);
        workspaces.push(WorkspaceSummary {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
            path: workspace.display_path(server),
            display_path: workspace.display_path(server),
            server_id: remote.server_id,
            server_name: remote.server_name,
            remote_path: remote.remote_path,
            connection_status: remote.connection_status,
            last_remote_error: remote.last_remote_error,
            logo_url,
            pinned: workspace.pinned,
            terminal_shell: workspace.terminal_shell.clone(),
            common_commands: workspace_common_command_summaries(&workspace.common_commands),
            chats,
            chat_pagination,
        });
        tracing::debug!(
            workspace_id = %workspace.id,
            elapsed_ms = workspace_started_at.elapsed().as_millis() as u64,
            "workspace summary build completed"
        );
    }

    tracing::debug!(
        workspace_count = workspaces.len(),
        elapsed_ms = response_started_at.elapsed().as_millis() as u64,
        "workspace response build completed"
    );
    Ok(Json(WorkspacesResponse {
        active_workspace_id: config.app.active_workspace_id.clone(),
        workspaces,
    }))
}

pub(crate) fn encode_workspace_chat_cursor(cursor: ChatPageCursor) -> String {
    let json = serde_json::to_vec(&cursor).expect("workspace chat cursor serializes");
    general_purpose::URL_SAFE_NO_PAD.encode(json)
}

pub(crate) fn configured_model_summary(model: &ModelSettings) -> ConfiguredModelSummary {
    let context_window = model.limits.as_ref().map(|limits| limits.context_window);
    let max_output_tokens = model.limits.as_ref().map(|limits| limits.max_output_tokens);
    let requires_limits = model_outputs_text(model);
    let mut missing_limits = Vec::new();

    if requires_limits && context_window.is_none() {
        missing_limits.push("contextWindow");
    }

    if requires_limits && max_output_tokens.is_none() {
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
        input_modalities: model.input_modalities.clone(),
        output_modalities: model.output_modalities.clone(),
        thinking_level: model.thinking_level.clone(),
        system_prompt_name: model.system_prompt_name.clone(),
        supports_thinking: false,
        warnings: Vec::new(),
    }
}

fn model_outputs_text(model: &ModelSettings) -> bool {
    model.output_modalities.is_empty()
        || model
            .output_modalities
            .iter()
            .any(|modality| modality == "text")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_shell_summaries_include_supported_shells() {
        let shells = terminal_shell_summaries()
            .into_iter()
            .map(|summary| summary.shell)
            .collect::<Vec<_>>();

        assert_eq!(shells, vec!["powershell", "cmd", "bash", "zsh"]);
    }
}
