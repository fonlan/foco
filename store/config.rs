use std::{
    collections::HashSet,
    env, fmt, fs, io,
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
};

use fancy_regex::Regex;
use foco_agent::{AgentDefinitionId, AgentExecutionWorkspaceMode, AgentPermissions};
use foco_mcp::{McpServerDefinition, McpTransportKind, validate_server_definitions};
use foco_providers::{
    HTTP_PROXY_KIND, ProviderModelRedirect, ProviderRequestOverride, SOCKS_PROXY_KIND,
    normalized_base_url, normalized_proxy_url, parse_provider_kind, validate_model_redirects,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::memory::global_memory_database_path;

pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_WORKSPACE_ID: &str = "default";
pub const DEFAULT_WORKSPACE_NAME: &str = "Default";
const LEGACY_DEFAULT_WORKSPACE_NAME: &str = "Default Workspace";
pub const REDACTED_SECRET: &str = "<redacted>";
pub const DEFAULT_WEB_SERVER_HOST: &str = "127.0.0.1";
pub const DEFAULT_WEB_SERVER_PORT: u16 = 3210;
pub const DEFAULT_APP_LANGUAGE: &str = "en";
pub const SUPPORTED_APP_LANGUAGES: &[&str] = &["zh-CN", "en"];
pub const DEFAULT_APP_THEME: &str = "light";
pub const SUPPORTED_APP_THEMES: &[&str] = &["light", "dark"];
pub const DEFAULT_LLM_REQUEST_RETRY_COUNT: u32 = 3;
pub const MAX_LLM_REQUEST_RETRY_COUNT: u32 = 10;
pub const CHAT_TITLE_GENERATION_DISABLED: &str = "disabled";
pub const CHAT_TITLE_GENERATION_CURRENT_CHAT_MODEL: &str = "current_chat_model";
pub const DEFAULT_API_REQUEST_DETAIL_RETENTION_DAYS: u32 = 3;
pub const DEFAULT_TERMINAL_SHELL: &str = default_terminal_shell_for_current_platform();
pub const DEFAULT_REMOTE_CONNECT_TIMEOUT_MS: u64 = 15_000;
pub const SUPPORTED_TERMINAL_SHELLS: &[&str] = &["powershell", "cmd", "bash", "zsh"];
pub const SUPPORTED_API_PROXY_TYPES: &[&str] = &[HTTP_PROXY_KIND, SOCKS_PROXY_KIND];
pub const WEB_SEARCH_PROVIDER_TAVILY: &str = "tavily";
pub const WEB_SEARCH_PROVIDER_BRAVE: &str = "brave";
pub const SUPPORTED_WEB_SEARCH_PROVIDERS: &[&str] =
    &[WEB_SEARCH_PROVIDER_TAVILY, WEB_SEARCH_PROVIDER_BRAVE];
pub const DEFAULT_SYSTEM_PROMPT_NAME: &str = "Default";
pub const IMAGE_GENERATION_SYSTEM_PROMPT_NAME: &str = "Image Generation";
pub const PLAN_MODE_SYSTEM_PROMPT_NAME: &str = "Plan Mode";
pub const REVIEW_SYSTEM_PROMPT_NAME: &str = "Review";
pub const AGENT_DEFINITION_INITIAL_REVISION: u64 = 1;
pub const AGENT_DEFINITION_NAME_MAX_CHARS: usize = 80;
pub const AGENT_DEFINITION_DESCRIPTION_MAX_CHARS: usize = 500;
pub const AGENT_DEFINITION_SYSTEM_PROMPT_MAX_CHARS: usize = 32_000;
pub const AGENT_DEFINITION_MAX_INSTANCES: u32 = 32;
pub const AGENT_DEFINITION_MAX_ALLOWED_TOOLS: usize = 128;
pub const AGENT_DEFINITION_MAX_ALLOWED_DEFINITIONS: usize = 64;
pub const SPEC_SYSTEM_PROMPT_MAX_CHARS: usize = 32_000;
pub const PLAN_MERGE_AUTOMATION_ISOLATED_AUTO_ONCE: &str = "isolated_auto_once";
pub const PLAN_MERGE_AUTOMATION_DIRECT_AUTO: &str = "direct_auto";
pub const SUPPORTED_PLAN_MERGE_AUTOMATION_MODES: &[&str] = &[
    PLAN_MERGE_AUTOMATION_ISOLATED_AUTO_ONCE,
    PLAN_MERGE_AUTOMATION_DIRECT_AUTO,
];
pub const SUPPORTED_AGENT_THINKING_LEVELS: &[&str] =
    &["none", "minimal", "low", "medium", "high", "xhigh", "max"];
pub const FOCO_CONFIG_DIR_ENV: &str = "FOCO_CONFIG_DIR";
pub const WORKSPACE_HOOK_CONFIG_FILE: &str = "hooks.json";
pub const SUPPORTED_HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PermissionDenied",
    "PostToolUse",
    "PostToolUseFailure",
    "PostToolBatch",
    "Stop",
    "StopFailure",
    "PreCompact",
    "PostCompact",
    "Elicitation",
    "ElicitationResult",
];
pub const UNSUPPORTED_HOOK_EVENTS: &[&str] = &[
    "Setup",
    "UserPromptExpansion",
    "MessageDisplay",
    "Notification",
    "SubagentStart",
    "SubagentStop",
    "TaskCreated",
    "TaskCompleted",
    "TeammateIdle",
    "InstructionsLoaded",
    "ConfigChange",
    "CwdChanged",
    "FileChanged",
    "WorktreeCreate",
    "WorktreeRemove",
];
pub const HOOK_HANDLER_COMMAND: &str = "command";
pub const HOOK_HANDLER_HTTP: &str = "http";
pub const HOOK_HANDLER_MCP_TOOL: &str = "mcp_tool";
pub const HOOK_HANDLER_PROMPT: &str = "prompt";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FocoPaths {
    pub user_profile_dir: PathBuf,
    pub root_dir: PathBuf,
    pub config_file: PathBuf,
    pub memory_database_file: PathBuf,
    pub workspace_dir: PathBuf,
    pub logs_dir: PathBuf,
}

impl FocoPaths {
    pub fn from_user_profile_env() -> Result<Self, ConfigError> {
        let profile =
            env::var_os(user_profile_env_name()).ok_or(ConfigError::MissingUserProfile)?;

        if profile.is_empty() {
            return Err(ConfigError::EmptyUserProfile);
        }

        let user_profile_dir = PathBuf::from(profile);
        let root_dir = match env::var_os(FOCO_CONFIG_DIR_ENV) {
            Some(config_dir) if config_dir.is_empty() => return Err(ConfigError::EmptyConfigDir),
            Some(config_dir) => PathBuf::from(config_dir),
            None => user_profile_dir.join(".foco"),
        };

        Ok(Self::from_root_dir(user_profile_dir, root_dir))
    }

    pub fn from_user_profile(profile: impl Into<PathBuf>) -> Self {
        let user_profile_dir = profile.into();
        let root_dir = user_profile_dir.join(".foco");

        Self::from_root_dir(user_profile_dir, root_dir)
    }

    pub fn from_config_dir(
        user_profile: impl Into<PathBuf>,
        config_dir: impl Into<PathBuf>,
    ) -> Self {
        Self::from_root_dir(user_profile.into(), config_dir.into())
    }

    fn from_root_dir(user_profile_dir: PathBuf, root_dir: PathBuf) -> Self {
        Self {
            user_profile_dir,
            config_file: root_dir.join("config.json"),
            memory_database_file: global_memory_database_path(&root_dir),
            workspace_dir: root_dir.join("workspace"),
            logs_dir: root_dir.join("logs"),
            root_dir,
        }
    }
}

fn user_profile_env_name() -> &'static str {
    if cfg!(windows) { "USERPROFILE" } else { "HOME" }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedGlobalConfig {
    pub config: GlobalConfig,
    pub paths: FocoPaths,
}

pub fn load_or_create_global_config() -> Result<LoadedGlobalConfig, ConfigError> {
    let paths = FocoPaths::from_user_profile_env()?;
    load_or_create_global_config_at_paths(paths)
}

pub fn load_or_create_global_config_at(
    user_profile: impl Into<PathBuf>,
) -> Result<LoadedGlobalConfig, ConfigError> {
    load_or_create_global_config_at_paths(FocoPaths::from_user_profile(user_profile))
}

pub fn load_or_create_global_config_at_paths(
    paths: FocoPaths,
) -> Result<LoadedGlobalConfig, ConfigError> {
    if !paths.config_file.exists() {
        create_first_run_config(&paths)?;
    }

    let mut config = load_global_config(&paths.config_file)?;
    validate_workspace_directories(&config, &paths.config_file)?;
    if rename_legacy_default_workspace(&mut config) {
        save_global_config(&paths.config_file, &config)?;
    }

    Ok(LoadedGlobalConfig { config, paths })
}

pub fn load_global_config(path: impl AsRef<Path>) -> Result<GlobalConfig, ConfigError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let config: GlobalConfig =
        serde_json::from_str(&content).map_err(|source| ConfigError::Json {
            path: path.to_path_buf(),
            source,
        })?;

    config.validate(Some(path))?;

    Ok(config)
}

pub fn workspace_hook_config_path(workspace_path: impl AsRef<Path>) -> PathBuf {
    workspace_path
        .as_ref()
        .join(".foco")
        .join(WORKSPACE_HOOK_CONFIG_FILE)
}

pub fn load_workspace_hook_config(
    workspace_path: impl AsRef<Path>,
) -> Result<HookConfig, ConfigError> {
    let path = workspace_hook_config_path(workspace_path);

    if !path.exists() {
        return Ok(HookConfig::default());
    }

    let content = fs::read_to_string(&path).map_err(|source| ConfigError::Io {
        path: path.clone(),
        source,
    })?;
    let config: HookConfig =
        serde_json::from_str(&content).map_err(|source| ConfigError::Json {
            path: path.clone(),
            source,
        })?;
    validate_hook_config(Some(&path), "hooks", &config)?;

    Ok(config)
}

pub fn save_workspace_hook_config(
    workspace_path: impl AsRef<Path>,
    config: &HookConfig,
) -> Result<(), ConfigError> {
    let path = workspace_hook_config_path(workspace_path);
    let parent = path.parent().ok_or_else(|| ConfigError::Validation {
        path: Some(path.clone()),
        message: "workspace hook config path has no parent directory".to_string(),
    })?;
    create_directory(parent)?;
    validate_hook_config(Some(&path), "hooks", config)?;
    let content = serde_json::to_string_pretty(config).map_err(|source| ConfigError::Json {
        path: path.clone(),
        source,
    })?;
    let temp_file = path.with_extension("json.tmp");

    fs::write(&temp_file, content).map_err(|source| ConfigError::Io {
        path: temp_file.clone(),
        source,
    })?;
    fs::rename(&temp_file, &path).map_err(|source| ConfigError::Io {
        path: path.clone(),
        source,
    })?;

    Ok(())
}

pub fn save_global_config(
    path: impl AsRef<Path>,
    config: &GlobalConfig,
) -> Result<(), ConfigError> {
    let path = path.as_ref();

    config.validate(Some(path))?;
    validate_workspace_directories(config, path)?;

    let content = serde_json::to_string_pretty(config).map_err(|source| ConfigError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    let parent = path.parent().ok_or_else(|| ConfigError::Validation {
        path: Some(path.to_path_buf()),
        message: "global config path has no parent directory".to_string(),
    })?;
    let mut temp_file =
        tempfile::NamedTempFile::new_in(parent).map_err(|source| ConfigError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    temp_file
        .write_all(content.as_bytes())
        .map_err(|source| ConfigError::Io {
            path: temp_file.path().to_path_buf(),
            source,
        })?;
    temp_file
        .as_file()
        .sync_all()
        .map_err(|source| ConfigError::Io {
            path: temp_file.path().to_path_buf(),
            source,
        })?;
    temp_file.persist(path).map_err(|error| ConfigError::Io {
        path: path.to_path_buf(),
        source: error.error,
    })?;

    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalConfig {
    pub schema_version: u32,
    pub app: AppSettings,
    #[serde(default)]
    pub hooks: HookConfig,
    #[serde(default)]
    pub memory: MemorySettings,
    #[serde(default)]
    pub prompts: PromptSettings,
    #[serde(default)]
    pub web_search: WebSearchSettings,
    #[serde(default)]
    pub spec: SpecSettings,
    #[serde(default)]
    pub plan: PlanSettings,
    pub providers: Vec<ProviderSettings>,
    pub models: Vec<ModelSettings>,
    #[serde(default, rename = "agentDefinitions")]
    pub agent_definitions: Vec<AgentDefinitionSettings>,
    pub mcp: McpConfig,
    pub skills: SkillConfig,
    #[serde(default, rename = "remoteServers")]
    pub remote_servers: Vec<RemoteServerProfile>,
    pub workspaces: Vec<WorkspaceConfig>,
}

impl GlobalConfig {
    pub fn first_run(default_workspace_path: PathBuf) -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            app: AppSettings {
                active_workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
                language: DEFAULT_APP_LANGUAGE.to_string(),
                theme: DEFAULT_APP_THEME.to_string(),
                llm_request_retry_count: DEFAULT_LLM_REQUEST_RETRY_COUNT,
                chat_title_generation_model_id: default_chat_title_generation_model_id(),
                runtime_tool_state_compression_enabled: false,
                auto_start_enabled: false,
                auto_update_check_enabled: false,
                default_team_mode_enabled: true,
                api_audit: ApiAuditSettings::default(),
                web_server: WebServerSettings::default(),
            },
            hooks: HookConfig::default(),
            memory: MemorySettings::default(),
            prompts: PromptSettings::default(),
            web_search: WebSearchSettings::default(),
            spec: SpecSettings::default(),
            plan: PlanSettings::default(),
            providers: Vec::new(),
            models: Vec::new(),
            agent_definitions: Vec::new(),
            mcp: McpConfig {
                servers: Vec::new(),
            },
            skills: SkillConfig {
                directories: Vec::new(),
                detected: Vec::new(),
                disabled: Vec::new(),
                disabled_locations: Vec::new(),
                enabled: Vec::new(),
                translation_model_id: None,
            },
            remote_servers: Vec::new(),
            workspaces: vec![WorkspaceConfig {
                id: DEFAULT_WORKSPACE_ID.to_string(),
                name: DEFAULT_WORKSPACE_NAME.to_string(),
                path: default_workspace_path,
                location: WorkspaceLocation::Local,
                pinned: false,
                terminal_shell: default_terminal_shell(),
                common_commands: Vec::new(),
            }],
        }
    }

    pub fn resolve_active_model_provider(
        &self,
        model_id: &str,
    ) -> Result<(&ModelSettings, &ProviderSettings), ModelRouteError> {
        let model = self
            .models
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| ModelRouteError::ModelNotFound(model_id.to_string()))?;
        if !model.enabled {
            return Err(ModelRouteError::ModelDisabled(model.id.clone()));
        }

        let active_provider_id = model
            .active_provider_id
            .as_deref()
            .ok_or_else(|| ModelRouteError::ActiveProviderMissing(model.id.clone()))?;
        if !model
            .provider_ids
            .iter()
            .any(|provider_id| provider_id == active_provider_id)
        {
            return Err(ModelRouteError::ActiveProviderNotAssociated {
                model_id: model.id.clone(),
                provider_id: active_provider_id.to_string(),
            });
        }

        let provider = self
            .providers
            .iter()
            .find(|provider| provider.id == active_provider_id)
            .ok_or_else(|| ModelRouteError::ActiveProviderNotFound {
                model_id: model.id.clone(),
                provider_id: active_provider_id.to_string(),
            })?;
        if !provider.enabled {
            return Err(ModelRouteError::ActiveProviderDisabled {
                model_id: model.id.clone(),
                provider_id: provider.id.clone(),
            });
        }

        Ok((model, provider))
    }

    pub fn validate(&self, config_path: Option<&Path>) -> Result<(), ConfigError> {
        if self.schema_version != CONFIG_SCHEMA_VERSION {
            return invalid_config(
                config_path,
                format!(
                    "unsupported schema_version {}; expected {}",
                    self.schema_version, CONFIG_SCHEMA_VERSION
                ),
            );
        }

        require_non_empty(
            config_path,
            "app.active_workspace_id",
            &self.app.active_workspace_id,
        )?;
        validate_llm_request_retry_count(config_path, self.app.llm_request_retry_count)?;
        validate_api_audit_settings(config_path, &self.app.api_audit)?;
        validate_app_language(config_path, &self.app.language)?;
        validate_app_theme(config_path, &self.app.theme)?;
        validate_web_server_settings(config_path, &self.app.web_server)?;
        validate_hook_config(config_path, "hooks", &self.hooks)?;
        validate_memory_settings(config_path, &self.memory, &self.models)?;
        validate_prompt_settings(config_path, &self.prompts)?;
        validate_web_search_settings(config_path, &self.web_search)?;
        validate_spec_settings(config_path, &self.spec, &self.models, &self.providers)?;
        validate_plan_settings(config_path, &self.plan, &self.models, &self.providers)?;
        require_non_empty_list(config_path, "workspaces", self.workspaces.len())?;

        let mut remote_server_ids = HashSet::new();
        for server in &self.remote_servers {
            validate_remote_server_profile(config_path, server)?;
            if !remote_server_ids.insert(server.id.as_str()) {
                return invalid_config(
                    config_path,
                    format!("duplicate remote server id '{}'", server.id),
                );
            }
        }

        let mut workspace_ids = HashSet::new();

        for workspace in &self.workspaces {
            validate_id(config_path, "workspace.id", &workspace.id)?;
            require_non_empty(config_path, "workspace.name", &workspace.name)?;
            validate_workspace_location(config_path, workspace, &remote_server_ids)?;
            validate_terminal_shell(
                config_path,
                "workspace.terminal_shell",
                &workspace.terminal_shell,
            )?;
            validate_workspace_common_commands(
                config_path,
                &workspace.id,
                &workspace.common_commands,
            )?;

            if !workspace_ids.insert(workspace.id.as_str()) {
                return invalid_config(
                    config_path,
                    format!("duplicate workspace id '{}'", workspace.id),
                );
            }
        }

        if !workspace_ids.contains(self.app.active_workspace_id.as_str()) {
            return invalid_config(
                config_path,
                format!(
                    "app.active_workspace_id '{}' does not match any workspace",
                    self.app.active_workspace_id
                ),
            );
        }

        validate_unique_named_items(
            config_path,
            "providers",
            self.providers.iter().map(|provider| provider.id.as_str()),
        )?;
        for provider in &self.providers {
            validate_id(config_path, "provider.id", &provider.id)?;
            require_non_empty(config_path, "provider.name", &provider.name)?;
            require_non_empty(config_path, "provider.kind", &provider.kind)?;
            parse_provider_kind(&provider.kind).map_err(|source| ConfigError::Validation {
                path: config_path.map(Path::to_path_buf),
                message: source.to_string(),
            })?;
            if let Some(base_url) = &provider.base_url {
                normalized_base_url(base_url).map_err(|source| ConfigError::Validation {
                    path: config_path.map(Path::to_path_buf),
                    message: source.to_string(),
                })?;
            }
            validate_api_proxy_settings(config_path, "provider.api_proxy", &provider.api_proxy)?;
            for request_override in &provider.request_overrides {
                request_override
                    .validate()
                    .map_err(|source| ConfigError::Validation {
                        path: config_path.map(Path::to_path_buf),
                        message: source.to_string(),
                    })?;
            }
            validate_model_redirects(&provider.model_redirects).map_err(|source| {
                ConfigError::Validation {
                    path: config_path.map(Path::to_path_buf),
                    message: source.to_string(),
                }
            })?;
            validate_provider_model_sync_filter(
                config_path,
                &provider.id,
                provider.model_sync_filter_regex.as_deref(),
            )?;
        }

        validate_unique_named_items(
            config_path,
            "models",
            self.models.iter().map(|model| model.id.as_str()),
        )?;
        let provider_ids: HashSet<&str> = self
            .providers
            .iter()
            .map(|provider| provider.id.as_str())
            .collect();
        for model in &self.models {
            validate_id(config_path, "model.id", &model.id)?;
            require_non_empty(config_path, "model.display_name", &model.display_name)?;

            if let Some(metadata_key) = &model.metadata_key {
                validate_id(config_path, "model.metadata_key", metadata_key)?;
                require_non_empty(
                    config_path,
                    "model.metadata_source_url",
                    model.metadata_source_url.as_deref().unwrap_or_default(),
                )?;
                require_non_empty(
                    config_path,
                    "model.metadata_refreshed_at",
                    model.metadata_refreshed_at.as_deref().unwrap_or_default(),
                )?;
            }

            if model.enabled && model_outputs_text(model) {
                let limits = model
                    .limits
                    .as_ref()
                    .ok_or_else(|| ConfigError::Validation {
                        path: config_path.map(Path::to_path_buf),
                        message: format!("enabled model '{}' is missing limits", model.id),
                    })?;

                if limits.context_window == 0 {
                    return invalid_config(
                        config_path,
                        format!(
                            "enabled model '{}' context_window must be greater than 0",
                            model.id
                        ),
                    );
                }

                if limits.max_output_tokens == 0 {
                    return invalid_config(
                        config_path,
                        format!(
                            "enabled model '{}' max_output_tokens must be greater than 0",
                            model.id
                        ),
                    );
                }
            }

            if let Some(active_provider_id) = &model.active_provider_id {
                validate_id(config_path, "model.active_provider_id", active_provider_id)?;

                if !model.provider_ids.iter().any(|id| id == active_provider_id) {
                    return invalid_config(
                        config_path,
                        format!(
                            "model '{}' active_provider_id '{}' is not in provider_ids",
                            model.id, active_provider_id
                        ),
                    );
                }
            }

            if let Some(thinking_level) = &model.thinking_level {
                validate_id(config_path, "model.thinking_level", thinking_level)?;
            }

            require_non_empty(
                config_path,
                "model.system_prompt_name",
                &model.system_prompt_name,
            )?;
            if !prompt_settings_contains_system_prompt(&self.prompts, &model.system_prompt_name) {
                return invalid_config(
                    config_path,
                    format!(
                        "model '{}' system_prompt_name '{}' references missing system prompt",
                        model.id, model.system_prompt_name
                    ),
                );
            }

            for provider_id in &model.provider_ids {
                validate_id(config_path, "model.provider_ids", provider_id)?;

                if !provider_ids.contains(provider_id.as_str()) {
                    return invalid_config(
                        config_path,
                        format!(
                            "model '{}' references missing provider '{}'",
                            model.id, provider_id
                        ),
                    );
                }
            }
        }

        validate_agent_definitions(
            config_path,
            &self.agent_definitions,
            &self.providers,
            &self.models,
        )?;

        validate_unique_named_items(
            config_path,
            "mcp.servers",
            self.mcp.servers.iter().map(|server| server.id.as_str()),
        )?;
        for server in &self.mcp.servers {
            validate_id(config_path, "mcp.server.id", &server.id)?;
            require_non_empty(config_path, "mcp.server.name", &server.name)?;
            require_non_empty(config_path, "mcp.server.transport", &server.transport)?;
        }
        let mcp_definitions = self
            .mcp
            .servers
            .iter()
            .map(McpServerConfig::to_definition)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| ConfigError::Validation {
                path: config_path.map(Path::to_path_buf),
                message: source.to_string(),
            })?;
        validate_server_definitions(&mcp_definitions).map_err(|source| {
            ConfigError::Validation {
                path: config_path.map(Path::to_path_buf),
                message: source.to_string(),
            }
        })?;

        validate_unique_named_items(
            config_path,
            "skills.detected",
            self.skills.detected.iter().map(|skill| {
                if skill.key.is_empty() {
                    skill.id.as_str()
                } else {
                    skill.key.as_str()
                }
            }),
        )?;
        for skill in &self.skills.detected {
            if !skill.key.is_empty() {
                validate_id(config_path, "skills.detected.key", &skill.key)?;
            }
            validate_id(config_path, "skills.detected.id", &skill.id)?;
            require_non_empty(config_path, "skills.detected.name", &skill.name)?;
            require_non_empty(
                config_path,
                "skills.detected.description",
                &skill.description,
            )?;
            validate_skill_scope(config_path, &skill.scope)?;
            if skill.scope == SKILL_SCOPE_WORKSPACE {
                require_non_empty(
                    config_path,
                    "skills.detected.workspace_id",
                    skill.workspace_id.as_deref().unwrap_or_default(),
                )?;
                require_non_empty(
                    config_path,
                    "skills.detected.workspace_name",
                    skill.workspace_name.as_deref().unwrap_or_default(),
                )?;
            }
            if !skill.path.is_absolute() {
                return invalid_config(
                    config_path,
                    format!(
                        "skill '{}' path must be absolute: {}",
                        skill.id,
                        skill.path.display()
                    ),
                );
            }
        }
        for skill_id in &self.skills.enabled {
            validate_id(config_path, "skills.enabled", skill_id)?;
        }
        for skill_id in &self.skills.disabled {
            validate_id(config_path, "skills.disabled", skill_id)?;
        }
        for location_id in &self.skills.disabled_locations {
            validate_skill_location_id(config_path, location_id)?;
        }
        validate_skill_translation_model(
            config_path,
            self.skills.translation_model_id.as_deref(),
            &self.models,
            &self.providers,
        )?;

        Ok(())
    }

    pub fn to_redacted_log_json(&self) -> Result<String, serde_json::Error> {
        let mut redacted = self.clone();

        for provider in &mut redacted.providers {
            if provider.api_key.is_some() {
                provider.api_key = Some(REDACTED_SECRET.to_string());
            }
        }
        if redacted.app.web_server.password_hash.is_some() {
            redacted.app.web_server.password_hash = Some(REDACTED_SECRET.to_string());
        }
        redacted.web_search.redact_secrets();
        for server in &mut redacted.mcp.servers {
            server.args.fill(REDACTED_SECRET.to_string());
            if server.url.is_some() {
                server.url = Some(REDACTED_SECRET.to_string());
            }
        }
        for server in &mut redacted.remote_servers {
            if server.password.is_some() {
                server.password = Some(REDACTED_SECRET.to_string());
            }
        }

        serde_json::to_string(&redacted)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppSettings {
    pub active_workspace_id: String,
    #[serde(default = "default_app_language")]
    pub language: String,
    #[serde(default = "default_app_theme")]
    pub theme: String,
    #[serde(default = "default_llm_request_retry_count")]
    pub llm_request_retry_count: u32,
    #[serde(default = "default_chat_title_generation_model_id")]
    pub chat_title_generation_model_id: Option<String>,
    #[serde(default)]
    pub runtime_tool_state_compression_enabled: bool,
    #[serde(default)]
    pub auto_start_enabled: bool,
    #[serde(default)]
    pub auto_update_check_enabled: bool,
    #[serde(default = "default_true")]
    pub default_team_mode_enabled: bool,
    #[serde(default)]
    pub api_audit: ApiAuditSettings,
    #[serde(default)]
    pub web_server: WebServerSettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApiAuditSettings {
    #[serde(default = "default_api_request_detail_retention_days")]
    pub request_detail_retention_days: u32,
    #[serde(default = "default_true")]
    pub save_request_response_details: bool,
}

impl Default for ApiAuditSettings {
    fn default() -> Self {
        Self {
            request_detail_retention_days: DEFAULT_API_REQUEST_DETAIL_RETENTION_DAYS,
            save_request_response_details: true,
        }
    }
}

fn default_api_request_detail_retention_days() -> u32 {
    DEFAULT_API_REQUEST_DETAIL_RETENTION_DAYS
}

fn default_app_language() -> String {
    DEFAULT_APP_LANGUAGE.to_string()
}

fn default_app_theme() -> String {
    DEFAULT_APP_THEME.to_string()
}

fn default_llm_request_retry_count() -> u32 {
    DEFAULT_LLM_REQUEST_RETRY_COUNT
}

fn default_chat_title_generation_model_id() -> Option<String> {
    Some(CHAT_TITLE_GENERATION_CURRENT_CHAT_MODEL.to_string())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApiProxySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_api_proxy_type")]
    pub proxy_type: String,
    #[serde(default)]
    pub url: String,
}

impl Default for ApiProxySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_type: default_api_proxy_type(),
            url: String::new(),
        }
    }
}

fn default_api_proxy_type() -> String {
    HTTP_PROXY_KIND.to_string()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebSearchSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_web_search_provider")]
    pub active_provider: String,
    #[serde(default)]
    pub tavily_api_key: Option<String>,
    #[serde(default)]
    pub brave_api_key: Option<String>,
    #[serde(default)]
    pub api_proxy: ApiProxySettings,
}

impl WebSearchSettings {
    pub fn api_key_for_provider(&self, provider: &str) -> Option<&str> {
        match provider {
            WEB_SEARCH_PROVIDER_TAVILY => self.tavily_api_key.as_deref(),
            WEB_SEARCH_PROVIDER_BRAVE => self.brave_api_key.as_deref(),
            _ => None,
        }
        .map(str::trim)
        .filter(|value| !value.is_empty())
    }

    fn redact_secrets(&mut self) {
        if self.tavily_api_key.is_some() {
            self.tavily_api_key = Some(REDACTED_SECRET.to_string());
        }
        if self.brave_api_key.is_some() {
            self.brave_api_key = Some(REDACTED_SECRET.to_string());
        }
    }
}

impl Default for WebSearchSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            active_provider: default_web_search_provider(),
            tavily_api_key: None,
            brave_api_key: None,
            api_proxy: ApiProxySettings::default(),
        }
    }
}

fn default_web_search_provider() -> String {
    WEB_SEARCH_PROVIDER_TAVILY.to_string()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebServerSettings {
    pub listen_host: String,
    pub listen_port: u16,
    #[serde(default)]
    pub password_hash: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookConfig {
    #[serde(default)]
    pub disable_all_hooks: bool,
    #[serde(default)]
    pub audit_enabled: bool,
    #[serde(default)]
    #[serde(flatten)]
    pub hooks: HookEventMap,
}

pub const DEFAULT_MEMORY_RETRIEVAL_LLM_TIMEOUT_MS: u64 = 60_000;
pub const DEFAULT_MEMORY_EXTRACTION_LLM_TIMEOUT_MS: u64 = 120_000;
pub const DEFAULT_MEMORY_DREAM_LLM_TIMEOUT_MS: u64 = 120_000;
pub const DEFAULT_SPEC_LLM_TIMEOUT_MS: u64 = 120_000;
const MAX_BACKGROUND_LLM_TIMEOUT_MS: u64 = 600_000;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemorySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_memory_extraction_mode")]
    pub extraction_mode: String,
    #[serde(default = "default_memory_retrieval_mode")]
    pub retrieval_mode: String,
    #[serde(default)]
    pub retention_days: Option<u32>,
    #[serde(default)]
    pub extraction_model_id: Option<String>,
    #[serde(default)]
    pub retrieval_model_id: Option<String>,
    #[serde(default = "default_memory_extraction_llm_timeout_ms")]
    pub extraction_llm_timeout_ms: u64,
    #[serde(default = "default_memory_retrieval_llm_timeout_ms")]
    pub retrieval_llm_timeout_ms: u64,
    #[serde(default)]
    pub dream: MemoryDreamSettings,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            extraction_mode: default_memory_extraction_mode(),
            retrieval_mode: default_memory_retrieval_mode(),
            retention_days: None,
            extraction_model_id: None,
            retrieval_model_id: None,
            extraction_llm_timeout_ms: default_memory_extraction_llm_timeout_ms(),
            retrieval_llm_timeout_ms: default_memory_retrieval_llm_timeout_ms(),
            dream: MemoryDreamSettings::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryDreamSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auto_enabled: bool,
    #[serde(default = "default_memory_dream_mode")]
    pub mode: String,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default = "default_memory_dream_workspace_interval_days")]
    pub workspace_interval_days: u32,
    #[serde(default = "default_memory_dream_global_interval_days")]
    pub global_interval_days: u32,
    #[serde(default = "default_true")]
    pub create_transcript_chat: bool,
    #[serde(default = "default_memory_dream_max_facts_per_run")]
    pub max_facts_per_run: u32,
    #[serde(default = "default_memory_dream_max_changes_per_run")]
    pub max_changes_per_run: u32,
    #[serde(default = "default_memory_dream_scheduler_scan_minutes")]
    pub scheduler_scan_minutes: u32,
    #[serde(default = "default_memory_dream_workspace_threshold_facts")]
    pub workspace_threshold_facts: u32,
    #[serde(default = "default_memory_dream_global_threshold_facts")]
    pub global_threshold_facts: u32,
    #[serde(default = "default_memory_dream_llm_timeout_ms")]
    pub llm_timeout_ms: u64,
}

impl Default for MemoryDreamSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_enabled: false,
            mode: default_memory_dream_mode(),
            model_id: None,
            workspace_interval_days: default_memory_dream_workspace_interval_days(),
            global_interval_days: default_memory_dream_global_interval_days(),
            create_transcript_chat: true,
            max_facts_per_run: default_memory_dream_max_facts_per_run(),
            max_changes_per_run: default_memory_dream_max_changes_per_run(),
            scheduler_scan_minutes: default_memory_dream_scheduler_scan_minutes(),
            workspace_threshold_facts: default_memory_dream_workspace_threshold_facts(),
            global_threshold_facts: default_memory_dream_global_threshold_facts(),
            llm_timeout_ms: default_memory_dream_llm_timeout_ms(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecSettings {
    #[serde(default = "default_true")]
    pub auto_enabled: bool,
    #[serde(default)]
    pub generation_model_id: Option<String>,
    #[serde(default)]
    pub generation_system_prompt: Option<String>,
    #[serde(default)]
    pub update_system_prompt: Option<String>,
    #[serde(default = "default_spec_llm_timeout_ms")]
    pub llm_timeout_ms: u64,
}

impl Default for SpecSettings {
    fn default() -> Self {
        Self {
            auto_enabled: true,
            generation_model_id: None,
            generation_system_prompt: None,
            update_system_prompt: None,
            llm_timeout_ms: default_spec_llm_timeout_ms(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanSettings {
    #[serde(default = "default_plan_merge_automation_mode")]
    pub merge_automation_mode: String,
    #[serde(default)]
    pub mode_model_id: Option<String>,
}

impl Default for PlanSettings {
    fn default() -> Self {
        Self {
            merge_automation_mode: default_plan_merge_automation_mode(),
            mode_model_id: None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptSettings {
    #[serde(default)]
    pub system_prompts: Vec<SystemPromptSettings>,
    #[serde(default, skip_serializing)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub files: Vec<PathBuf>,
    #[serde(default)]
    pub extra_text: String,
    /// Optional override for dedicated context-checkpoint System prompt.
    /// Empty/whitespace is treated as unset (use built-in default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_compression_system_prompt: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemPromptSettings {
    pub name: String,
    pub content: String,
}

pub type HookEventMap = std::collections::BTreeMap<String, Vec<HookMatcherGroup>>;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookMatcherGroup {
    #[serde(default = "default_true")]
    #[serde(skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    #[serde(default)]
    pub hooks: Vec<HookHandler>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookHandler {
    #[serde(default = "default_true")]
    #[serde(skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(rename = "type")]
    pub handler_type: String,
    #[serde(default)]
    #[serde(rename = "if", alias = "ifFilter")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub if_filter: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(default)]
    #[serde(rename = "async", alias = "asyncHook")]
    pub async_hook: bool,
    #[serde(default)]
    pub async_rewake: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
}

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn default_memory_extraction_mode() -> String {
    "manual".to_string()
}

fn default_memory_retrieval_mode() -> String {
    "fts".to_string()
}

fn default_memory_dream_mode() -> String {
    "llm".to_string()
}

fn default_memory_dream_workspace_interval_days() -> u32 {
    7
}

fn default_memory_dream_global_interval_days() -> u32 {
    30
}

fn default_memory_dream_max_facts_per_run() -> u32 {
    200
}

fn default_memory_dream_max_changes_per_run() -> u32 {
    50
}

fn default_memory_dream_scheduler_scan_minutes() -> u32 {
    60
}

fn default_memory_dream_workspace_threshold_facts() -> u32 {
    50
}

fn default_memory_dream_global_threshold_facts() -> u32 {
    50
}

fn default_memory_extraction_llm_timeout_ms() -> u64 {
    DEFAULT_MEMORY_EXTRACTION_LLM_TIMEOUT_MS
}

fn default_memory_retrieval_llm_timeout_ms() -> u64 {
    DEFAULT_MEMORY_RETRIEVAL_LLM_TIMEOUT_MS
}

fn default_memory_dream_llm_timeout_ms() -> u64 {
    DEFAULT_MEMORY_DREAM_LLM_TIMEOUT_MS
}

fn default_spec_llm_timeout_ms() -> u64 {
    DEFAULT_SPEC_LLM_TIMEOUT_MS
}

fn default_plan_merge_automation_mode() -> String {
    PLAN_MERGE_AUTOMATION_ISOLATED_AUTO_ONCE.to_string()
}

fn default_system_prompt_name() -> String {
    DEFAULT_SYSTEM_PROMPT_NAME.to_string()
}

impl Default for WebServerSettings {
    fn default() -> Self {
        Self {
            listen_host: DEFAULT_WEB_SERVER_HOST.to_string(),
            listen_port: DEFAULT_WEB_SERVER_PORT,
            password_hash: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSettings {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    #[serde(default)]
    pub auto_sync_models: bool,
    #[serde(default)]
    pub model_sync_filter_regex: Option<String>,
    #[serde(default)]
    pub request_overrides: Vec<ProviderRequestOverride>,
    #[serde(default, alias = "modelRedirects")]
    pub model_redirects: Vec<ProviderModelRedirect>,
    #[serde(default)]
    pub api_proxy: ApiProxySettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSettings {
    pub id: String,
    pub display_name: String,
    pub enabled: bool,
    pub provider_ids: Vec<String>,
    pub active_provider_id: Option<String>,
    pub thinking_level: Option<String>,
    #[serde(default = "default_system_prompt_name")]
    pub system_prompt_name: String,
    pub metadata_key: Option<String>,
    pub metadata_source_url: Option<String>,
    pub metadata_refreshed_at: Option<String>,
    pub limits: Option<ModelLimits>,
    #[serde(default)]
    pub input_modalities: Vec<String>,
    #[serde(default)]
    pub output_modalities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelLimits {
    pub context_window: u64,
    pub max_output_tokens: u64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentModelOptions {
    #[serde(default)]
    pub thinking_level: Option<String>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentDefinitionSettings {
    pub id: AgentDefinitionId,
    pub revision: u64,
    pub name: String,
    pub description: String,
    pub provider_id: String,
    pub model_id: String,
    pub model_options: AgentModelOptions,
    pub system_prompt: String,
    pub allowed_tools: Vec<String>,
    pub max_instances: u32,
    #[serde(default = "default_agent_execution_workspace_modes")]
    pub allowed_execution_workspace_modes: Vec<AgentExecutionWorkspaceMode>,
    pub permissions: AgentPermissions,
}

pub fn default_agent_execution_workspace_modes() -> Vec<AgentExecutionWorkspaceMode> {
    AgentExecutionWorkspaceMode::all()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpConfig {
    pub servers: Vec<McpServerConfig>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum McpExecutionHost {
    Auto,
    Local,
    Workspace,
}

impl Default for McpExecutionHost {
    fn default() -> Self {
        Self::Auto
    }
}

impl From<McpExecutionHost> for foco_mcp::McpExecutionHost {
    fn from(value: McpExecutionHost) -> Self {
        match value {
            McpExecutionHost::Auto => Self::Auto,
            McpExecutionHost::Local => Self::Local,
            McpExecutionHost::Workspace => Self::Workspace,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub transport: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    #[serde(default, rename = "executionHost")]
    pub execution_host: McpExecutionHost,
}

impl McpServerConfig {
    pub fn to_definition(&self) -> Result<McpServerDefinition, foco_mcp::McpError> {
        Ok(McpServerDefinition {
            id: self.id.clone(),
            name: self.name.clone(),
            enabled: self.enabled,
            transport: McpTransportKind::parse(&self.transport)?,
            command: self.command.clone(),
            args: self.args.clone(),
            url: self.url.clone(),
            execution_host: self.execution_host.into(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillConfig {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub directories: Vec<PathBuf>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub detected: Vec<SkillSettings>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub disabled: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub disabled_locations: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enabled: Vec<String>,
    #[serde(default)]
    #[serde(rename = "translationModelId", skip_serializing_if = "Option::is_none")]
    pub translation_model_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillSettings {
    #[serde(default)]
    pub key: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    #[serde(default = "default_skill_scope")]
    pub scope: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteAuthMethod {
    #[default]
    Key,
    Password,
}

/// SSH remote server connection profile (global config).
///
/// Passwords are sensitive and must never appear in API summaries, logs, or
/// sidecar bundles. Prefer [`RemoteServerProfile::password_configured`].
#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteServerProfile {
    pub id: String,
    pub name: String,
    pub host_alias: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_file: Option<PathBuf>,
    /// Authentication strategy. Missing field in old configs deserializes as `Key`.
    #[serde(default)]
    pub auth_method: RemoteAuthMethod,
    /// Login password for `authMethod=password`. Sensitive; redacted in logs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_remote_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foco_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_shell: Option<String>,
    #[serde(default = "default_remote_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_known_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sidecar_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_checked_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidecar_install_state: Option<String>,
}

impl RemoteServerProfile {
    pub fn password_configured(&self) -> bool {
        self.password
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    }
}

impl Default for RemoteServerProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            host_alias: String::new(),
            user: None,
            port: None,
            identity_file: None,
            auth_method: RemoteAuthMethod::Key,
            password: None,
            default_remote_root: None,
            foco_command: None,
            terminal_shell: None,
            connect_timeout_ms: DEFAULT_REMOTE_CONNECT_TIMEOUT_MS,
            last_known_target: None,
            last_sidecar_version: None,
            last_checked_at: None,
            last_error: None,
            sidecar_install_state: None,
        }
    }
}

impl fmt::Debug for RemoteServerProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteServerProfile")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("host_alias", &self.host_alias)
            .field("user", &self.user)
            .field("port", &self.port)
            .field("identity_file", &self.identity_file)
            .field("auth_method", &self.auth_method)
            .field("password", &self.password.as_ref().map(|_| REDACTED_SECRET))
            .field("default_remote_root", &self.default_remote_root)
            .field("foco_command", &self.foco_command)
            .field("terminal_shell", &self.terminal_shell)
            .field("connect_timeout_ms", &self.connect_timeout_ms)
            .field("last_known_target", &self.last_known_target)
            .field("last_sidecar_version", &self.last_sidecar_version)
            .field("last_checked_at", &self.last_checked_at)
            .field("last_error", &self.last_error)
            .field("sidecar_install_state", &self.sidecar_install_state)
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "path_is_empty")]
    pub path: PathBuf,
    #[serde(default = "default_workspace_location")]
    pub location: WorkspaceLocation,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default = "default_terminal_shell")]
    pub terminal_shell: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub common_commands: Vec<WorkspaceCommonCommand>,
}

impl WorkspaceConfig {
    pub fn is_remote(&self) -> bool {
        self.location.is_remote()
    }

    pub fn display_path(&self, server: Option<&RemoteServerProfile>) -> String {
        self.location.display_path(&self.path, server)
    }

    pub fn local_path(&self) -> Option<&Path> {
        self.location.local_path(&self.path)
    }

    pub fn remote_path(&self) -> Option<&str> {
        self.location.remote_path()
    }

    pub fn server_id(&self) -> Option<&str> {
        self.location.server_id()
    }

    pub fn workspace_key(&self) -> String {
        self.location.workspace_key(&self.path)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum WorkspaceLocation {
    Local,
    Ssh {
        #[serde(rename = "serverId")]
        server_id: String,
        #[serde(rename = "remotePath")]
        remote_path: String,
    },
}

impl WorkspaceLocation {
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Ssh { .. })
    }

    pub fn display_path(&self, legacy_path: &Path, server: Option<&RemoteServerProfile>) -> String {
        match self {
            Self::Local => display_config_path(legacy_path),
            Self::Ssh {
                server_id,
                remote_path,
            } => {
                let server_name = server
                    .map(|server| server.name.trim())
                    .filter(|name| !name.is_empty())
                    .or_else(|| {
                        server
                            .map(|server| server.host_alias.trim())
                            .filter(|alias| !alias.is_empty())
                    })
                    .unwrap_or(server_id);
                format!("{server_name}:{remote_path}")
            }
        }
    }

    pub fn local_path<'a>(&'a self, legacy_path: &'a Path) -> Option<&'a Path> {
        match self {
            Self::Local => Some(legacy_path),
            Self::Ssh { .. } => None,
        }
    }

    pub fn remote_path(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Ssh { remote_path, .. } => Some(remote_path),
        }
    }

    pub fn server_id(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Ssh { server_id, .. } => Some(server_id),
        }
    }

    pub fn workspace_key(&self, legacy_path: &Path) -> String {
        match self {
            Self::Local => format!("local:{}", display_config_path(legacy_path)),
            Self::Ssh {
                server_id,
                remote_path,
            } => format!("ssh:{server_id}:{remote_path}"),
        }
    }
}

impl Default for WorkspaceLocation {
    fn default() -> Self {
        Self::Local
    }
}

fn default_workspace_location() -> WorkspaceLocation {
    WorkspaceLocation::Local
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceCommonCommand {
    pub name: String,
    pub command: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelRouteError {
    ModelNotFound(String),
    ModelDisabled(String),
    ActiveProviderMissing(String),
    ActiveProviderNotAssociated {
        model_id: String,
        provider_id: String,
    },
    ActiveProviderNotFound {
        model_id: String,
        provider_id: String,
    },
    ActiveProviderDisabled {
        model_id: String,
        provider_id: String,
    },
}

impl fmt::Display for ModelRouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelNotFound(model_id) => write!(formatter, "model was not found: {model_id}"),
            Self::ModelDisabled(model_id) => write!(formatter, "model '{model_id}' is disabled"),
            Self::ActiveProviderMissing(model_id) => {
                write!(formatter, "model '{model_id}' has no active provider")
            }
            Self::ActiveProviderNotAssociated {
                model_id,
                provider_id,
            } => write!(
                formatter,
                "active provider '{provider_id}' is not associated with model '{model_id}'"
            ),
            Self::ActiveProviderNotFound {
                model_id,
                provider_id,
            } => write!(
                formatter,
                "active provider '{provider_id}' for model '{model_id}' was not found"
            ),
            Self::ActiveProviderDisabled {
                model_id,
                provider_id,
            } => write!(
                formatter,
                "active provider '{provider_id}' for model '{model_id}' is disabled"
            ),
        }
    }
}

impl std::error::Error for ModelRouteError {}

#[derive(Debug)]
pub enum ConfigError {
    EmptyConfigDir,
    EmptyUserProfile,
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    MissingUserProfile,
    Validation {
        path: Option<PathBuf>,
        message: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyConfigDir => write!(formatter, "{FOCO_CONFIG_DIR_ENV} is empty"),
            Self::EmptyUserProfile => write!(formatter, "{} is empty", user_profile_env_name()),
            Self::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
            Self::Json { path, source } => {
                write!(
                    formatter,
                    "{} contains invalid JSON config: {}",
                    path.display(),
                    source
                )
            }
            Self::MissingUserProfile => write!(formatter, "{} is not set", user_profile_env_name()),
            Self::Validation {
                path: Some(path),
                message,
            } => write!(
                formatter,
                "{} contains invalid config: {}",
                path.display(),
                message
            ),
            Self::Validation {
                path: None,
                message,
            } => write!(formatter, "invalid config: {message}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::EmptyConfigDir
            | Self::EmptyUserProfile
            | Self::MissingUserProfile
            | Self::Validation { .. } => None,
        }
    }
}

fn create_first_run_config(paths: &FocoPaths) -> Result<(), ConfigError> {
    create_directory(&paths.root_dir)?;
    create_directory(&paths.workspace_dir)?;

    let config = GlobalConfig::first_run(paths.workspace_dir.clone());
    let content = serde_json::to_string_pretty(&config).map_err(|source| ConfigError::Json {
        path: paths.config_file.clone(),
        source,
    })?;
    let temp_file = paths.root_dir.join("config.json.tmp");

    fs::write(&temp_file, content).map_err(|source| ConfigError::Io {
        path: temp_file.clone(),
        source,
    })?;
    fs::rename(&temp_file, &paths.config_file).map_err(|source| ConfigError::Io {
        path: paths.config_file.clone(),
        source,
    })?;

    Ok(())
}

fn rename_legacy_default_workspace(config: &mut GlobalConfig) -> bool {
    let Some(workspace) = config
        .workspaces
        .iter_mut()
        .find(|workspace| workspace.id == DEFAULT_WORKSPACE_ID)
    else {
        return false;
    };

    if workspace.name != LEGACY_DEFAULT_WORKSPACE_NAME {
        return false;
    }

    workspace.name = DEFAULT_WORKSPACE_NAME.to_string();
    true
}

pub const SKILL_SCOPE_GLOBAL: &str = "global";
pub const SKILL_SCOPE_WORKSPACE: &str = "workspace";

fn default_skill_scope() -> String {
    SKILL_SCOPE_GLOBAL.to_string()
}

pub const fn default_terminal_shell_for_current_platform() -> &'static str {
    if cfg!(windows) {
        "powershell"
    } else if cfg!(target_os = "macos") {
        "zsh"
    } else {
        "bash"
    }
}

pub fn default_terminal_shell_for_platform(platform: &str) -> &'static str {
    match platform.as_bytes() {
        b"windows" => "powershell",
        b"macos" => "zsh",
        _ => "bash",
    }
}

fn default_terminal_shell() -> String {
    default_terminal_shell_for_current_platform().to_string()
}

fn default_remote_connect_timeout_ms() -> u64 {
    DEFAULT_REMOTE_CONNECT_TIMEOUT_MS
}

fn display_config_path(path: &Path) -> String {
    path.display().to_string()
}

fn path_is_empty(path: &Path) -> bool {
    path.as_os_str().is_empty()
}

fn validate_skill_scope(config_path: Option<&Path>, scope: &str) -> Result<(), ConfigError> {
    match scope {
        SKILL_SCOPE_GLOBAL | SKILL_SCOPE_WORKSPACE => Ok(()),
        _ => invalid_config(
            config_path,
            format!("skills.detected.scope '{scope}' is unsupported; expected global or workspace"),
        ),
    }
}

fn validate_skill_location_id(
    config_path: Option<&Path>,
    location_id: &str,
) -> Result<(), ConfigError> {
    let valid = location_id == "global:agents"
        || location_id
            .strip_prefix("workspace:")
            .and_then(|value| value.split_once(':'))
            .is_some_and(|(workspace_id, location)| {
                !workspace_id.is_empty()
                    && !workspace_id.chars().any(char::is_whitespace)
                    && matches!(location, "agents" | "claude")
            });

    if valid {
        Ok(())
    } else {
        invalid_config(
            config_path,
            format!("skills.disabled_locations contains invalid skill location id '{location_id}'"),
        )
    }
}

fn create_directory(path: &Path) -> Result<(), ConfigError> {
    fs::create_dir_all(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_workspace_directories(
    config: &GlobalConfig,
    config_path: &Path,
) -> Result<(), ConfigError> {
    for workspace in &config.workspaces {
        let Some(path) = workspace.local_path() else {
            continue;
        };
        if !path.is_dir() {
            return invalid_config(
                Some(config_path),
                format!(
                    "workspace '{}' path does not exist or is not a directory: {}",
                    workspace.id,
                    path.display()
                ),
            );
        }
    }

    Ok(())
}

fn validate_remote_server_profile(
    config_path: Option<&Path>,
    server: &RemoteServerProfile,
) -> Result<(), ConfigError> {
    validate_id(config_path, "remoteServers.id", &server.id)?;
    require_non_empty(config_path, "remoteServers.name", &server.name)?;
    require_non_empty(config_path, "remoteServers.hostAlias", &server.host_alias)?;
    if let Some(user) = &server.user {
        require_non_empty(config_path, "remoteServers.user", user)?;
    }
    if server.port == Some(0) {
        return invalid_config(config_path, "remoteServers.port must be greater than 0");
    }
    if let Some(root) = &server.default_remote_root {
        require_remote_default_root_path(config_path, "remoteServers.defaultRemoteRoot", root)?;
    }
    match server.auth_method {
        RemoteAuthMethod::Password => {
            if !server.password_configured() {
                return invalid_config(
                    config_path,
                    "remoteServers.password is required when authMethod is password",
                );
            }
        }
        RemoteAuthMethod::Key => {
            if server
                .password
                .as_ref()
                .is_some_and(|value| !value.is_empty())
            {
                // Allow accidental leftover empty? Reject any password material in key mode
                // so secrets are not retained after switching auth methods.
                return invalid_config(
                    config_path,
                    "remoteServers.password must be empty when authMethod is key",
                );
            }
        }
    }
    if let Some(command) = &server.foco_command {
        require_non_empty(config_path, "remoteServers.focoCommand", command)?;
    }
    if let Some(shell) = &server.terminal_shell {
        validate_terminal_shell(config_path, "remoteServers.terminalShell", shell)?;
    }
    if server.connect_timeout_ms == 0 {
        return invalid_config(
            config_path,
            "remoteServers.connectTimeoutMs must be greater than 0",
        );
    }
    if let Some(error) = &server.last_error {
        require_non_empty(config_path, "remoteServers.lastError", error)?;
    }
    if let Some(state) = &server.sidecar_install_state {
        require_non_empty(config_path, "remoteServers.sidecarInstallState", state)?;
    }

    Ok(())
}

fn validate_workspace_location(
    config_path: Option<&Path>,
    workspace: &WorkspaceConfig,
    remote_server_ids: &HashSet<&str>,
) -> Result<(), ConfigError> {
    match &workspace.location {
        WorkspaceLocation::Local => {
            if !workspace.path.is_absolute() {
                return invalid_config(
                    config_path,
                    format!(
                        "workspace '{}' path must be absolute: {}",
                        workspace.id,
                        workspace.path.display()
                    ),
                );
            }
        }
        WorkspaceLocation::Ssh {
            server_id,
            remote_path,
        } => {
            if !remote_server_ids.contains(server_id.as_str()) {
                return invalid_config(
                    config_path,
                    format!(
                        "workspace '{}' remote server was not found: {}",
                        workspace.id, server_id
                    ),
                );
            }
            require_remote_absolute_path(
                config_path,
                "workspace.location.remotePath",
                remote_path,
            )?;
        }
    }

    Ok(())
}

fn require_remote_absolute_path(
    config_path: Option<&Path>,
    field: &str,
    value: &str,
) -> Result<(), ConfigError> {
    let value = value.trim();
    if value.is_empty() {
        return invalid_config(config_path, format!("{field} must not be empty"));
    }
    if !value.starts_with('/') {
        return invalid_config(
            config_path,
            format!("{field} must be an absolute remote path: {value}"),
        );
    }

    Ok(())
}

/// Default remote root may be an absolute POSIX path or `~` / `~/...`.
/// Other-user homes (`~other`) and relative paths are rejected.
fn require_remote_default_root_path(
    config_path: Option<&Path>,
    field: &str,
    value: &str,
) -> Result<(), ConfigError> {
    let value = value.trim();
    if value.is_empty() {
        return invalid_config(config_path, format!("{field} must not be empty"));
    }
    if is_remote_home_shorthand(value) || value.starts_with('/') {
        return Ok(());
    }
    invalid_config(
        config_path,
        format!("{field} must be an absolute remote path or home shorthand (~ or ~/...): {value}"),
    )
}

/// `~` or `~/...` only — not `~user` or relative segments.
pub fn is_remote_home_shorthand(value: &str) -> bool {
    let value = value.trim();
    value == "~" || value.starts_with("~/")
}

/// True when the path still needs remote `$HOME` expansion before persistence.
pub fn needs_remote_home_expansion(value: &str) -> bool {
    is_remote_home_shorthand(value)
}

fn require_non_empty(
    config_path: Option<&Path>,
    field: &str,
    value: &str,
) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return invalid_config(config_path, format!("{field} must not be empty"));
    }

    Ok(())
}

fn validate_api_proxy_settings(
    config_path: Option<&Path>,
    field: &str,
    settings: &ApiProxySettings,
) -> Result<(), ConfigError> {
    let proxy_type = settings.proxy_type.trim();

    if !SUPPORTED_API_PROXY_TYPES.contains(&proxy_type) {
        return invalid_config(
            config_path,
            format!(
                "{field}.proxy_type '{proxy_type}' is unsupported; expected one of {}",
                SUPPORTED_API_PROXY_TYPES.join(", ")
            ),
        );
    }

    let proxy_url = settings.url.trim();
    if settings.enabled && proxy_url.is_empty() {
        return invalid_config(
            config_path,
            format!("{field}.url must not be empty when enabled"),
        );
    }

    if settings.enabled || !proxy_url.is_empty() {
        normalized_proxy_url(proxy_type, proxy_url).map_err(|source| ConfigError::Validation {
            path: config_path.map(Path::to_path_buf),
            message: source.to_string(),
        })?;
    }

    Ok(())
}

fn validate_provider_model_sync_filter(
    config_path: Option<&Path>,
    provider_id: &str,
    pattern: Option<&str>,
) -> Result<(), ConfigError> {
    let Some(pattern) = pattern.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    Regex::new(pattern).map_err(|source| ConfigError::Validation {
        path: config_path.map(Path::to_path_buf),
        message: format!("provider '{provider_id}' model_sync_filter_regex is invalid: {source}"),
    })?;

    Ok(())
}

fn validate_web_search_settings(
    config_path: Option<&Path>,
    settings: &WebSearchSettings,
) -> Result<(), ConfigError> {
    let active_provider = settings.active_provider.trim();
    if !SUPPORTED_WEB_SEARCH_PROVIDERS.contains(&active_provider) {
        return invalid_config(
            config_path,
            format!(
                "web_search.active_provider '{active_provider}' is unsupported; expected one of {}",
                SUPPORTED_WEB_SEARCH_PROVIDERS.join(", ")
            ),
        );
    }

    validate_optional_secret(
        config_path,
        "web_search.tavily_api_key",
        &settings.tavily_api_key,
    )?;
    validate_optional_secret(
        config_path,
        "web_search.brave_api_key",
        &settings.brave_api_key,
    )?;
    if settings.enabled && settings.api_key_for_provider(active_provider).is_none() {
        return invalid_config(
            config_path,
            format!("web_search.{active_provider} api key must be set when web search is enabled"),
        );
    }
    validate_api_proxy_settings(config_path, "web_search.api_proxy", &settings.api_proxy)?;

    Ok(())
}

fn validate_optional_secret(
    config_path: Option<&Path>,
    field: &str,
    value: &Option<String>,
) -> Result<(), ConfigError> {
    if value
        .as_deref()
        .is_some_and(|secret| secret.trim().is_empty())
    {
        return invalid_config(config_path, format!("{field} must not be empty when set"));
    }

    Ok(())
}

fn validate_web_server_settings(
    config_path: Option<&Path>,
    settings: &WebServerSettings,
) -> Result<(), ConfigError> {
    require_non_empty(
        config_path,
        "app.web_server.listen_host",
        &settings.listen_host,
    )?;

    settings
        .listen_host
        .parse::<IpAddr>()
        .map_err(|_| ConfigError::Validation {
            path: config_path.map(Path::to_path_buf),
            message: format!(
                "app.web_server.listen_host must be an IP address: {}",
                settings.listen_host
            ),
        })?;

    if settings.listen_port == 0 {
        return invalid_config(
            config_path,
            "app.web_server.listen_port must be a number from 1 to 65535",
        );
    }

    if let Some(password_hash) = &settings.password_hash {
        validate_password_hash(config_path, password_hash)?;
    }

    Ok(())
}

fn validate_password_hash(
    config_path: Option<&Path>,
    password_hash: &str,
) -> Result<(), ConfigError> {
    let parts = password_hash.split(':').collect::<Vec<_>>();

    if parts.len() != 3 || parts[0] != "sha256" {
        return invalid_config(
            config_path,
            "app.web_server.password_hash must use sha256:<salt_hex>:<hash_hex>",
        );
    }

    if parts[1].len() != 32 || !parts[1].bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid_config(
            config_path,
            "app.web_server.password_hash salt must be 16 bytes of hex",
        );
    }

    if parts[2].len() != 64 || !parts[2].bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid_config(
            config_path,
            "app.web_server.password_hash digest must be 32 bytes of hex",
        );
    }

    Ok(())
}

fn validate_agent_definitions(
    config_path: Option<&Path>,
    definitions: &[AgentDefinitionSettings],
    providers: &[ProviderSettings],
    models: &[ModelSettings],
) -> Result<(), ConfigError> {
    let definition_ids = definitions
        .iter()
        .map(|definition| definition.id.as_str())
        .collect::<HashSet<_>>();
    if definition_ids.len() != definitions.len() {
        return invalid_config(config_path, "agentDefinitions contains duplicate ids");
    }

    let mut normalized_names = HashSet::new();
    for definition in definitions {
        let field = format!("agentDefinitions['{}']", definition.id);

        if definition.revision == 0 {
            return invalid_config(
                config_path,
                format!("{field}.revision must be greater than 0"),
            );
        }
        validate_bounded_agent_text(
            config_path,
            &format!("{field}.name"),
            &definition.name,
            AGENT_DEFINITION_NAME_MAX_CHARS,
            true,
        )?;
        validate_bounded_agent_text(
            config_path,
            &format!("{field}.description"),
            &definition.description,
            AGENT_DEFINITION_DESCRIPTION_MAX_CHARS,
            false,
        )?;
        validate_bounded_agent_text(
            config_path,
            &format!("{field}.systemPrompt"),
            &definition.system_prompt,
            AGENT_DEFINITION_SYSTEM_PROMPT_MAX_CHARS,
            true,
        )?;

        let normalized_name = definition.name.to_lowercase();
        if !normalized_names.insert(normalized_name) {
            return invalid_config(
                config_path,
                format!(
                    "agentDefinitions contains duplicate case-insensitive name '{}'",
                    definition.name
                ),
            );
        }

        validate_id(
            config_path,
            &format!("{field}.providerId"),
            &definition.provider_id,
        )?;

        let model = models
            .iter()
            .find(|model| model.id == definition.model_id)
            .ok_or_else(|| ConfigError::Validation {
                path: config_path.map(Path::to_path_buf),
                message: format!(
                    "{field}.modelId references missing model '{}'",
                    definition.model_id
                ),
            })?;
        if !model.enabled {
            return invalid_config(
                config_path,
                format!("{field}.modelId references disabled model '{}'", model.id),
            );
        }
        let active_provider_id =
            model
                .active_provider_id
                .as_deref()
                .ok_or_else(|| ConfigError::Validation {
                    path: config_path.map(Path::to_path_buf),
                    message: format!(
                        "{field}.modelId references model '{}' without an active provider",
                        model.id
                    ),
                })?;
        let active_provider = providers
            .iter()
            .find(|provider| provider.id == active_provider_id)
            .ok_or_else(|| ConfigError::Validation {
                path: config_path.map(Path::to_path_buf),
                message: format!(
                    "{field}.modelId references model '{}' with missing active provider '{}'",
                    model.id, active_provider_id
                ),
            })?;
        if !active_provider.enabled {
            return invalid_config(
                config_path,
                format!(
                    "{field}.modelId references model '{}' with disabled active provider '{}'",
                    model.id, active_provider.id
                ),
            );
        }
        if !model_outputs_text(model) {
            return invalid_config(
                config_path,
                format!(
                    "{field}.modelId references model '{}' without text output",
                    model.id
                ),
            );
        }
        let limits = model
            .limits
            .as_ref()
            .ok_or_else(|| ConfigError::Validation {
                path: config_path.map(Path::to_path_buf),
                message: format!(
                    "{field}.modelId references model '{}' without limits",
                    model.id
                ),
            })?;
        if limits.context_window == 0 || limits.max_output_tokens == 0 {
            return invalid_config(
                config_path,
                format!(
                    "{field}.modelId references model '{}' with invalid limits",
                    model.id
                ),
            );
        }

        if let Some(thinking_level) = &definition.model_options.thinking_level
            && !SUPPORTED_AGENT_THINKING_LEVELS.contains(&thinking_level.as_str())
        {
            return invalid_config(
                config_path,
                format!(
                    "{field}.modelOptions.thinkingLevel '{}' is unsupported; expected one of {}",
                    thinking_level,
                    SUPPORTED_AGENT_THINKING_LEVELS.join(", ")
                ),
            );
        }
        if let Some(max_output_tokens) = definition.model_options.max_output_tokens {
            if max_output_tokens == 0 {
                return invalid_config(
                    config_path,
                    format!("{field}.modelOptions.maxOutputTokens must be greater than 0"),
                );
            }
            if u64::from(max_output_tokens) > limits.max_output_tokens {
                return invalid_config(
                    config_path,
                    format!(
                        "{field}.modelOptions.maxOutputTokens {max_output_tokens} exceeds model '{}' limit {}",
                        model.id, limits.max_output_tokens
                    ),
                );
            }
        }

        if definition.max_instances == 0
            || definition.max_instances > AGENT_DEFINITION_MAX_INSTANCES
        {
            return invalid_config(
                config_path,
                format!(
                    "{field}.maxInstances must be between 1 and {AGENT_DEFINITION_MAX_INSTANCES}"
                ),
            );
        }
        if definition.allowed_execution_workspace_modes.is_empty() {
            return invalid_config(
                config_path,
                format!("{field}.allowedExecutionWorkspaceModes must not be empty"),
            );
        }
        let mut allowed_workspace_modes = HashSet::new();
        for mode in &definition.allowed_execution_workspace_modes {
            if !allowed_workspace_modes.insert(*mode) {
                return invalid_config(
                    config_path,
                    format!(
                        "{field}.allowedExecutionWorkspaceModes contains duplicate mode '{}'",
                        mode.as_str()
                    ),
                );
            }
        }
        if definition.allowed_tools.len() > AGENT_DEFINITION_MAX_ALLOWED_TOOLS {
            return invalid_config(
                config_path,
                format!(
                    "{field}.allowedTools must contain no more than {AGENT_DEFINITION_MAX_ALLOWED_TOOLS} entries"
                ),
            );
        }
        let mut allowed_tools = HashSet::new();
        for tool_name in &definition.allowed_tools {
            require_non_empty(config_path, &format!("{field}.allowedTools"), tool_name)?;
            if tool_name.trim() != tool_name {
                return invalid_config(
                    config_path,
                    format!(
                        "{field}.allowedTools entry '{tool_name}' must not have surrounding whitespace"
                    ),
                );
            }
            if !allowed_tools.insert(tool_name.as_str()) {
                return invalid_config(
                    config_path,
                    format!("{field}.allowedTools contains duplicate tool '{tool_name}'"),
                );
            }
        }

        let allowed_definition_ids = &definition.permissions.allowed_agent_definition_ids;
        if allowed_definition_ids.len() > AGENT_DEFINITION_MAX_ALLOWED_DEFINITIONS {
            return invalid_config(
                config_path,
                format!(
                    "{field}.permissions.allowedAgentDefinitionIds must contain no more than {AGENT_DEFINITION_MAX_ALLOWED_DEFINITIONS} entries"
                ),
            );
        }
        if !definition.permissions.can_create_instances && !allowed_definition_ids.is_empty() {
            return invalid_config(
                config_path,
                format!(
                    "{field}.permissions.allowedAgentDefinitionIds must be empty when canCreateInstances is false"
                ),
            );
        }
        let mut allowed_ids = HashSet::new();
        for allowed_id in allowed_definition_ids {
            if !allowed_ids.insert(allowed_id.as_str()) {
                return invalid_config(
                    config_path,
                    format!(
                        "{field}.permissions.allowedAgentDefinitionIds contains duplicate id '{allowed_id}'"
                    ),
                );
            }
            if !definition_ids.contains(allowed_id.as_str()) {
                return invalid_config(
                    config_path,
                    format!(
                        "{field}.permissions.allowedAgentDefinitionIds references missing definition '{allowed_id}'"
                    ),
                );
            }
        }
    }

    Ok(())
}

fn model_outputs_text(model: &ModelSettings) -> bool {
    model.output_modalities.is_empty()
        || model
            .output_modalities
            .iter()
            .any(|modality| modality == "text")
}

fn validate_skill_translation_model(
    config_path: Option<&Path>,
    model_id: Option<&str>,
    models: &[ModelSettings],
    providers: &[ProviderSettings],
) -> Result<(), ConfigError> {
    let Some(model_id) = model_id else {
        return Ok(());
    };
    require_non_empty(config_path, "skills.translationModelId", model_id)?;

    let Some(model) = models.iter().find(|model| model.id == model_id) else {
        return invalid_config(
            config_path,
            format!("skills.translationModelId references missing model '{model_id}'"),
        );
    };
    if !model.enabled || !model_outputs_text(model) {
        return invalid_config(
            config_path,
            format!(
                "skills.translationModelId references disabled or non-text-output model '{model_id}'"
            ),
        );
    }
    let active_provider_id =
        model
            .active_provider_id
            .as_deref()
            .ok_or_else(|| ConfigError::Validation {
                path: config_path.map(Path::to_path_buf),
                message: format!(
                    "skills.translationModelId model '{model_id}' has no active provider selected"
                ),
            })?;
    let active_provider_enabled = providers
        .iter()
        .find(|provider| provider.id == active_provider_id)
        .is_some_and(|provider| provider.enabled);
    if !active_provider_enabled {
        return invalid_config(
            config_path,
            format!(
                "skills.translationModelId model '{model_id}' references missing or disabled active provider '{active_provider_id}'"
            ),
        );
    }

    Ok(())
}

pub fn validate_agent_definition_tool_references(
    config_path: Option<&Path>,
    definitions: &[AgentDefinitionSettings],
    known_tools: &HashSet<String>,
) -> Result<(), ConfigError> {
    for definition in definitions {
        for tool_name in &definition.allowed_tools {
            if !known_tools.contains(tool_name) {
                return invalid_config(
                    config_path,
                    format!(
                        "agent definition '{}' references unknown runtime tool '{}'",
                        definition.id, tool_name
                    ),
                );
            }
        }
    }

    Ok(())
}

fn validate_bounded_agent_text(
    config_path: Option<&Path>,
    field: &str,
    value: &str,
    max_chars: usize,
    required: bool,
) -> Result<(), ConfigError> {
    if required {
        require_non_empty(config_path, field, value)?;
    }
    if value.chars().count() > max_chars {
        return invalid_config(
            config_path,
            format!("{field} must contain no more than {max_chars} characters"),
        );
    }
    if (required || !value.is_empty()) && value.trim() != value {
        return invalid_config(
            config_path,
            format!("{field} must not have surrounding whitespace"),
        );
    }

    Ok(())
}

fn validate_memory_settings(
    config_path: Option<&Path>,
    settings: &MemorySettings,
    models: &[ModelSettings],
) -> Result<(), ConfigError> {
    match settings.extraction_mode.as_str() {
        "manual" | "pending_review" | "automatic" | "disabled" => {}
        other => {
            return invalid_config(
                config_path,
                format!("memory.extraction_mode has unsupported value '{other}'"),
            );
        }
    }

    match settings.retrieval_mode.as_str() {
        "fts" | "llm" => {}
        other => {
            return invalid_config(
                config_path,
                format!("memory.retrieval_mode has unsupported value '{other}'"),
            );
        }
    }

    if settings.retention_days == Some(0) {
        return invalid_config(config_path, "memory.retention_days must be greater than 0");
    }

    validate_background_llm_timeout_ms(
        config_path,
        "memory.extraction_llm_timeout_ms",
        settings.extraction_llm_timeout_ms,
    )?;
    validate_background_llm_timeout_ms(
        config_path,
        "memory.retrieval_llm_timeout_ms",
        settings.retrieval_llm_timeout_ms,
    )?;

    if let Some(model_id) = &settings.extraction_model_id {
        require_non_empty(config_path, "memory.extraction_model_id", model_id)?;

        if !models.iter().any(|model| model.id == *model_id) {
            return invalid_config(
                config_path,
                format!("memory.extraction_model_id references missing model '{model_id}'"),
            );
        }
    }

    if let Some(model_id) = &settings.retrieval_model_id {
        require_non_empty(config_path, "memory.retrieval_model_id", model_id)?;

        if !models.iter().any(|model| model.id == *model_id) {
            return invalid_config(
                config_path,
                format!("memory.retrieval_model_id references missing model '{model_id}'"),
            );
        }
    }

    validate_memory_dream_settings(config_path, &settings.dream, models)?;

    Ok(())
}

fn validate_memory_dream_settings(
    config_path: Option<&Path>,
    settings: &MemoryDreamSettings,
    models: &[ModelSettings],
) -> Result<(), ConfigError> {
    match settings.mode.as_str() {
        "deterministic_only" | "llm" => {}
        other => {
            return invalid_config(
                config_path,
                format!("memory.dream.mode has unsupported value '{other}'"),
            );
        }
    }

    if let Some(model_id) = &settings.model_id {
        require_non_empty(config_path, "memory.dream.model_id", model_id)?;

        if !models
            .iter()
            .any(|model| model.id == *model_id && model.enabled)
        {
            return invalid_config(
                config_path,
                format!("memory.dream.model_id references missing or disabled model '{model_id}'"),
            );
        }
    }

    if settings.workspace_interval_days == 0 {
        return invalid_config(
            config_path,
            "memory.dream.workspace_interval_days must be greater than 0",
        );
    }
    if settings.global_interval_days == 0 {
        return invalid_config(
            config_path,
            "memory.dream.global_interval_days must be greater than 0",
        );
    }
    if settings.max_facts_per_run == 0 {
        return invalid_config(
            config_path,
            "memory.dream.max_facts_per_run must be greater than 0",
        );
    }
    if settings.max_changes_per_run == 0 {
        return invalid_config(
            config_path,
            "memory.dream.max_changes_per_run must be greater than 0",
        );
    }
    if settings.scheduler_scan_minutes == 0 {
        return invalid_config(
            config_path,
            "memory.dream.scheduler_scan_minutes must be greater than 0",
        );
    }
    if settings.workspace_threshold_facts == 0 {
        return invalid_config(
            config_path,
            "memory.dream.workspace_threshold_facts must be greater than 0",
        );
    }
    if settings.global_threshold_facts == 0 {
        return invalid_config(
            config_path,
            "memory.dream.global_threshold_facts must be greater than 0",
        );
    }
    validate_background_llm_timeout_ms(
        config_path,
        "memory.dream.llm_timeout_ms",
        settings.llm_timeout_ms,
    )?;

    Ok(())
}

fn validate_spec_settings(
    config_path: Option<&Path>,
    settings: &SpecSettings,
    models: &[ModelSettings],
    providers: &[ProviderSettings],
) -> Result<(), ConfigError> {
    if let Some(model_id) = &settings.generation_model_id {
        require_non_empty(config_path, "spec.generation_model_id", model_id)?;

        let model = models.iter().find(|model| model.id == *model_id);
        let active_provider_enabled = model
            .and_then(|model| model.active_provider_id.as_deref())
            .and_then(|provider_id| providers.iter().find(|provider| provider.id == provider_id))
            .is_some_and(|provider| provider.enabled);
        if !model.is_some_and(|model| model.enabled) || !active_provider_enabled {
            return invalid_config(
                config_path,
                format!(
                    "spec.generation_model_id references missing, disabled, or providerless model '{model_id}'"
                ),
            );
        }
    }

    validate_spec_system_prompt(
        config_path,
        "spec.generation_system_prompt",
        settings.generation_system_prompt.as_deref(),
    )?;
    validate_spec_system_prompt(
        config_path,
        "spec.update_system_prompt",
        settings.update_system_prompt.as_deref(),
    )?;
    validate_background_llm_timeout_ms(
        config_path,
        "spec.llm_timeout_ms",
        settings.llm_timeout_ms,
    )?;

    Ok(())
}

fn validate_background_llm_timeout_ms(
    config_path: Option<&Path>,
    name: &str,
    value: u64,
) -> Result<(), ConfigError> {
    if value == 0 || value > MAX_BACKGROUND_LLM_TIMEOUT_MS {
        return invalid_config(
            config_path,
            format!("{name} must be between 1 and {MAX_BACKGROUND_LLM_TIMEOUT_MS}"),
        );
    }

    Ok(())
}

fn validate_spec_system_prompt(
    config_path: Option<&Path>,
    field: &str,
    value: Option<&str>,
) -> Result<(), ConfigError> {
    let Some(value) = value else {
        return Ok(());
    };
    require_non_empty(config_path, field, value)?;
    if value.chars().count() > SPEC_SYSTEM_PROMPT_MAX_CHARS {
        return invalid_config(
            config_path,
            format!("{field} must be no longer than {SPEC_SYSTEM_PROMPT_MAX_CHARS} characters"),
        );
    }
    Ok(())
}

fn validate_plan_settings(
    config_path: Option<&Path>,
    settings: &PlanSettings,
    models: &[ModelSettings],
    providers: &[ProviderSettings],
) -> Result<(), ConfigError> {
    if !SUPPORTED_PLAN_MERGE_AUTOMATION_MODES.contains(&settings.merge_automation_mode.as_str()) {
        return invalid_config(
            config_path,
            format!(
                "plan.merge_automation_mode must be one of: {}",
                SUPPORTED_PLAN_MERGE_AUTOMATION_MODES.join(", ")
            ),
        );
    }

    if let Some(model_id) = &settings.mode_model_id {
        require_non_empty(config_path, "plan.mode_model_id", model_id)?;

        let model = models.iter().find(|model| model.id == *model_id);
        let active_provider_enabled = model
            .and_then(|model| model.active_provider_id.as_deref())
            .and_then(|provider_id| providers.iter().find(|provider| provider.id == provider_id))
            .is_some_and(|provider| provider.enabled);
        if !model.is_some_and(|model| model.enabled) || !active_provider_enabled {
            return invalid_config(
                config_path,
                format!(
                    "plan.mode_model_id references missing, disabled, or providerless model '{model_id}'"
                ),
            );
        }
    }

    Ok(())
}

fn validate_prompt_settings(
    config_path: Option<&Path>,
    settings: &PromptSettings,
) -> Result<(), ConfigError> {
    let mut system_prompt_names = HashSet::new();
    let mut prompt_files = HashSet::new();

    if settings
        .system_prompt
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return invalid_config(config_path, "prompts.system_prompt must not be empty");
    }

    for prompt in &settings.system_prompts {
        require_non_empty(config_path, "prompts.system_prompts.name", &prompt.name)?;
        require_non_empty(
            config_path,
            "prompts.system_prompts.content",
            &prompt.content,
        )?;

        if !system_prompt_names.insert(prompt.name.as_str()) {
            return invalid_config(
                config_path,
                format!("duplicate system prompt name '{}'", prompt.name),
            );
        }
    }

    if !settings.system_prompts.is_empty()
        && !system_prompt_names.contains(DEFAULT_SYSTEM_PROMPT_NAME)
    {
        return invalid_config(
            config_path,
            format!(
                "prompts.system_prompts must include '{}'",
                DEFAULT_SYSTEM_PROMPT_NAME
            ),
        );
    }

    for file in &settings.files {
        if !file.is_absolute() {
            return invalid_config(
                config_path,
                format!("prompt file path must be absolute: {}", file.display()),
            );
        }

        if !prompt_files.insert(file) {
            return invalid_config(
                config_path,
                format!("duplicate prompt file path: {}", file.display()),
            );
        }
    }

    if let Some(prompt) = settings.context_compression_system_prompt.as_deref() {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return invalid_config(
                config_path,
                "prompts.context_compression_system_prompt must not be empty or whitespace-only",
            );
        }
        if trimmed.chars().count() > SPEC_SYSTEM_PROMPT_MAX_CHARS {
            return invalid_config(
                config_path,
                format!(
                    "prompts.context_compression_system_prompt must be no longer than {SPEC_SYSTEM_PROMPT_MAX_CHARS} characters"
                ),
            );
        }
    }

    Ok(())
}

fn prompt_settings_contains_system_prompt(settings: &PromptSettings, name: &str) -> bool {
    name == DEFAULT_SYSTEM_PROMPT_NAME
        || name == IMAGE_GENERATION_SYSTEM_PROMPT_NAME
        || name == PLAN_MODE_SYSTEM_PROMPT_NAME
        || name == REVIEW_SYSTEM_PROMPT_NAME
        || settings
            .system_prompts
            .iter()
            .any(|prompt| prompt.name == name)
}

fn validate_hook_config(
    config_path: Option<&Path>,
    field: &str,
    config: &HookConfig,
) -> Result<(), ConfigError> {
    for (event, groups) in &config.hooks {
        if UNSUPPORTED_HOOK_EVENTS.contains(&event.as_str()) {
            return invalid_config(
                config_path,
                format!(
                    "{field}.{event} is a Claude Code hook event that Foco does not support yet"
                ),
            );
        }

        if !SUPPORTED_HOOK_EVENTS.contains(&event.as_str()) {
            return invalid_config(
                config_path,
                format!(
                    "{field}.{event} is unsupported; expected one of {}",
                    SUPPORTED_HOOK_EVENTS.join(", ")
                ),
            );
        }

        for (group_index, group) in groups.iter().enumerate() {
            if group.hooks.is_empty() {
                return invalid_config(
                    config_path,
                    format!("{field}.{event}[{group_index}].hooks must not be empty"),
                );
            }

            for (handler_index, handler) in group.hooks.iter().enumerate() {
                validate_hook_handler(
                    config_path,
                    &format!("{field}.{event}[{group_index}].hooks[{handler_index}]"),
                    event,
                    handler,
                )?;
            }
        }
    }

    Ok(())
}

fn validate_hook_handler(
    config_path: Option<&Path>,
    field: &str,
    event: &str,
    handler: &HookHandler,
) -> Result<(), ConfigError> {
    require_non_empty(config_path, &format!("{field}.type"), &handler.handler_type)?;

    if handler.if_filter.is_some() && !is_tool_hook_event(event) {
        return invalid_config(
            config_path,
            format!("{field}.if is only supported for tool hook events"),
        );
    }

    if let Some(timeout) = handler.timeout
        && timeout == 0
    {
        return invalid_config(
            config_path,
            format!("{field}.timeout must be greater than 0"),
        );
    }

    match handler.handler_type.as_str() {
        HOOK_HANDLER_COMMAND => {
            require_non_empty(
                config_path,
                &format!("{field}.command"),
                handler.command.as_deref().unwrap_or_default(),
            )?;
            require_empty_hook_field(config_path, field, "url", handler.url.as_deref())?;
            require_empty_hook_field(config_path, field, "serverId", handler.server_id.as_deref())?;
            require_empty_hook_field(config_path, field, "toolName", handler.tool_name.as_deref())?;
            require_empty_hook_field(config_path, field, "prompt", handler.prompt.as_deref())?;
        }
        HOOK_HANDLER_HTTP => {
            let url = handler.url.as_deref().unwrap_or_default();
            require_non_empty(config_path, &format!("{field}.url"), url)?;
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return invalid_config(
                    config_path,
                    format!("{field}.url must start with http:// or https://"),
                );
            }
            require_empty_hook_field(config_path, field, "command", handler.command.as_deref())?;
            require_empty_hook_field(config_path, field, "serverId", handler.server_id.as_deref())?;
            require_empty_hook_field(config_path, field, "toolName", handler.tool_name.as_deref())?;
            require_empty_hook_field(config_path, field, "prompt", handler.prompt.as_deref())?;
            if !handler.args.is_empty() {
                return invalid_config(
                    config_path,
                    format!("{field}.args is only valid for command hooks"),
                );
            }
        }
        HOOK_HANDLER_MCP_TOOL => {
            require_non_empty(
                config_path,
                &format!("{field}.serverId"),
                handler.server_id.as_deref().unwrap_or_default(),
            )?;
            require_non_empty(
                config_path,
                &format!("{field}.toolName"),
                handler.tool_name.as_deref().unwrap_or_default(),
            )?;
            require_empty_hook_field(config_path, field, "command", handler.command.as_deref())?;
            require_empty_hook_field(config_path, field, "url", handler.url.as_deref())?;
            require_empty_hook_field(config_path, field, "prompt", handler.prompt.as_deref())?;
            if !handler.args.is_empty() {
                return invalid_config(
                    config_path,
                    format!("{field}.args is only valid for command hooks"),
                );
            }
        }
        HOOK_HANDLER_PROMPT => {
            require_non_empty(
                config_path,
                &format!("{field}.prompt"),
                handler.prompt.as_deref().unwrap_or_default(),
            )?;
            require_empty_hook_field(config_path, field, "command", handler.command.as_deref())?;
            require_empty_hook_field(config_path, field, "url", handler.url.as_deref())?;
            require_empty_hook_field(config_path, field, "serverId", handler.server_id.as_deref())?;
            require_empty_hook_field(config_path, field, "toolName", handler.tool_name.as_deref())?;
            if !handler.args.is_empty() {
                return invalid_config(
                    config_path,
                    format!("{field}.args is only valid for command hooks"),
                );
            }
        }
        other => {
            return invalid_config(
                config_path,
                format!(
                    "{field}.type '{other}' is unsupported; expected command, http, mcp_tool, or prompt"
                ),
            );
        }
    }

    Ok(())
}

fn require_empty_hook_field(
    config_path: Option<&Path>,
    field: &str,
    name: &str,
    value: Option<&str>,
) -> Result<(), ConfigError> {
    if value.map(|value| !value.trim().is_empty()).unwrap_or(false) {
        return invalid_config(
            config_path,
            format!("{field}.{name} is not valid for this hook handler type"),
        );
    }

    Ok(())
}

fn is_tool_hook_event(event: &str) -> bool {
    matches!(
        event,
        "PreToolUse"
            | "PermissionRequest"
            | "PermissionDenied"
            | "PostToolUse"
            | "PostToolUseFailure"
            | "PostToolBatch"
    )
}

fn validate_llm_request_retry_count(
    config_path: Option<&Path>,
    retry_count: u32,
) -> Result<(), ConfigError> {
    if retry_count <= MAX_LLM_REQUEST_RETRY_COUNT {
        return Ok(());
    }

    invalid_config(
        config_path,
        format!(
            "app.llm_request_retry_count must be no greater than {MAX_LLM_REQUEST_RETRY_COUNT}"
        ),
    )
}

fn validate_api_audit_settings(
    config_path: Option<&Path>,
    settings: &ApiAuditSettings,
) -> Result<(), ConfigError> {
    if settings.request_detail_retention_days > 0 {
        return Ok(());
    }

    invalid_config(
        config_path,
        "app.api_audit.request_detail_retention_days must be greater than 0".to_string(),
    )
}

fn validate_app_language(config_path: Option<&Path>, language: &str) -> Result<(), ConfigError> {
    if SUPPORTED_APP_LANGUAGES.contains(&language) {
        return Ok(());
    }

    invalid_config(
        config_path,
        format!(
            "app.language '{language}' is unsupported; expected one of {}",
            SUPPORTED_APP_LANGUAGES.join(", ")
        ),
    )
}

fn validate_app_theme(config_path: Option<&Path>, theme: &str) -> Result<(), ConfigError> {
    if SUPPORTED_APP_THEMES.contains(&theme) {
        return Ok(());
    }

    invalid_config(
        config_path,
        format!(
            "app.theme '{theme}' is unsupported; expected one of {}",
            SUPPORTED_APP_THEMES.join(", ")
        ),
    )
}

fn validate_terminal_shell(
    config_path: Option<&Path>,
    field: &str,
    shell: &str,
) -> Result<(), ConfigError> {
    if SUPPORTED_TERMINAL_SHELLS.contains(&shell) {
        return Ok(());
    }

    invalid_config(
        config_path,
        format!(
            "{field} '{shell}' is unsupported; expected one of {}",
            SUPPORTED_TERMINAL_SHELLS.join(", ")
        ),
    )
}

fn validate_workspace_common_commands(
    config_path: Option<&Path>,
    workspace_id: &str,
    commands: &[WorkspaceCommonCommand],
) -> Result<(), ConfigError> {
    for (index, command) in commands.iter().enumerate() {
        let field = format!("workspace '{workspace_id}' common_commands[{index}]");
        require_non_empty(config_path, &format!("{field}.name"), &command.name)?;
        require_non_empty(config_path, &format!("{field}.command"), &command.command)?;
    }

    Ok(())
}

fn require_non_empty_list(
    config_path: Option<&Path>,
    field: &str,
    len: usize,
) -> Result<(), ConfigError> {
    if len == 0 {
        return invalid_config(config_path, format!("{field} must not be empty"));
    }

    Ok(())
}

fn validate_id(config_path: Option<&Path>, field: &str, id: &str) -> Result<(), ConfigError> {
    require_non_empty(config_path, field, id)?;

    if id.chars().any(char::is_whitespace) {
        return invalid_config(
            config_path,
            format!("{field} '{id}' must not contain whitespace"),
        );
    }

    Ok(())
}

fn validate_unique_named_items<'a>(
    config_path: Option<&Path>,
    field: &str,
    ids: impl Iterator<Item = &'a str>,
) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();

    for id in ids {
        if !seen.insert(id) {
            return invalid_config(config_path, format!("{field} contains duplicate id '{id}'"));
        }
    }

    Ok(())
}

fn invalid_config<T>(
    config_path: Option<&Path>,
    message: impl Into<String>,
) -> Result<T, ConfigError> {
    Err(ConfigError::Validation {
        path: config_path.map(Path::to_path_buf),
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_terminal_shell_matches_platform_family() {
        assert_eq!(default_terminal_shell_for_platform("windows"), "powershell");
        assert_eq!(default_terminal_shell_for_platform("macos"), "zsh");
        assert_eq!(default_terminal_shell_for_platform("linux"), "bash");
        assert_eq!(default_terminal_shell_for_platform("freebsd"), "bash");
    }

    #[test]
    fn user_profile_env_matches_platform() {
        if cfg!(windows) {
            assert_eq!(user_profile_env_name(), "USERPROFILE");
        } else {
            assert_eq!(user_profile_env_name(), "HOME");
        }
    }

    #[test]
    fn first_run_creates_config_workspace_and_default_workspace() {
        let profile = tempfile::tempdir().expect("temp profile");

        let loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        assert!(loaded.paths.root_dir.is_dir());
        assert!(loaded.paths.config_file.is_file());
        assert_eq!(
            loaded.paths.memory_database_file,
            loaded.paths.root_dir.join("memory.sqlite")
        );
        assert!(loaded.paths.workspace_dir.is_dir());
        assert_eq!(
            loaded.config.app.active_workspace_id,
            DEFAULT_WORKSPACE_ID.to_string()
        );
        assert_eq!(loaded.config.app.language, DEFAULT_APP_LANGUAGE);
        assert_eq!(loaded.config.app.theme, DEFAULT_APP_THEME);
        assert!(!loaded.config.app.auto_update_check_enabled);
        assert_eq!(loaded.config.app.web_server, WebServerSettings::default());
        assert_eq!(loaded.config.workspaces.len(), 1);
        assert_eq!(loaded.config.workspaces[0].name, DEFAULT_WORKSPACE_NAME);
        assert_eq!(loaded.config.workspaces[0].path, loaded.paths.workspace_dir);
        assert!(!loaded.config.workspaces[0].pinned);
        assert_eq!(
            loaded.config.workspaces[0].terminal_shell,
            DEFAULT_TERMINAL_SHELL
        );
        assert!(loaded.config.workspaces[0].common_commands.is_empty());
        assert!(loaded.config.skills.directories.is_empty());
    }

    #[test]
    fn provider_model_redirects_default_and_alias_deserialize() {
        let snake: ProviderSettings = serde_json::from_value(serde_json::json!({
            "id": "qwen",
            "name": "Qwen",
            "kind": "openai-chat",
            "enabled": true,
            "base_url": null,
            "api_key": null
        }))
        .expect("provider without redirects should deserialize");
        assert!(snake.model_redirects.is_empty());

        let camel: ProviderSettings = serde_json::from_value(serde_json::json!({
            "id": "qwen",
            "name": "Qwen",
            "kind": "openai-chat",
            "enabled": true,
            "base_url": null,
            "api_key": null,
            "modelRedirects": [
                { "from": "qwen/qwen3.6-35b-a3b", "to": "qwen3.6-35b-a3b" }
            ]
        }))
        .expect("provider camelCase redirects should deserialize");
        assert_eq!(camel.model_redirects[0].to, "qwen3.6-35b-a3b");
    }

    #[test]
    fn provider_model_redirects_are_validated() {
        let mut config = GlobalConfig::first_run(PathBuf::from("/tmp/workspace"));
        config.providers.push(ProviderSettings {
            id: "qwen".to_string(),
            name: "Qwen".to_string(),
            kind: "openai-chat".to_string(),
            enabled: true,
            base_url: None,
            api_key: None,
            auto_sync_models: false,
            model_sync_filter_regex: None,
            request_overrides: Vec::new(),
            model_redirects: vec![
                ProviderModelRedirect {
                    from: "qwen/one".to_string(),
                    to: "qwen".to_string(),
                },
                ProviderModelRedirect {
                    from: "qwen/two".to_string(),
                    to: "qwen".to_string(),
                },
            ],
            api_proxy: ApiProxySettings::default(),
        });

        let error = config
            .validate(None)
            .expect_err("duplicate redirect target should fail");

        assert!(
            error
                .to_string()
                .contains("duplicate model redirect target")
        );
    }

    #[test]
    fn config_dir_paths_do_not_add_nested_foco_directory() {
        let profile = PathBuf::from("/tmp/foco-profile");
        let config_dir = PathBuf::from("/tmp/foco-dev");

        let paths = FocoPaths::from_config_dir(profile.clone(), config_dir.clone());

        assert_eq!(paths.user_profile_dir, profile);
        assert_eq!(paths.root_dir, config_dir);
        assert_eq!(
            paths.config_file,
            PathBuf::from("/tmp/foco-dev/config.json")
        );
        assert_eq!(
            paths.workspace_dir,
            PathBuf::from("/tmp/foco-dev/workspace")
        );
        assert_eq!(
            paths.memory_database_file,
            PathBuf::from("/tmp/foco-dev/memory.sqlite")
        );
        assert_eq!(paths.logs_dir, PathBuf::from("/tmp/foco-dev/logs"));
    }

    #[test]
    fn remote_server_profile_and_ssh_workspace_validate_without_secrets() {
        let workspace_dir = tempfile::tempdir().expect("workspace");
        let mut config = GlobalConfig::first_run(workspace_dir.path().to_path_buf());
        config.remote_servers.push(RemoteServerProfile {
            id: "prod".to_string(),
            name: "Prod".to_string(),
            host_alias: "prod-box".to_string(),
            user: Some("deploy".to_string()),
            port: Some(22),
            identity_file: None,
            auth_method: RemoteAuthMethod::Key,
            password: None,
            default_remote_root: Some("/srv".to_string()),
            foco_command: Some("foco".to_string()),
            terminal_shell: Some("bash".to_string()),
            connect_timeout_ms: DEFAULT_REMOTE_CONNECT_TIMEOUT_MS,
            last_known_target: Some("linux-x64".to_string()),
            last_sidecar_version: Some("0.1.0".to_string()),
            last_checked_at: Some("2026-07-04T00:00:00Z".to_string()),
            last_error: None,
            sidecar_install_state: Some("available".to_string()),
        });
        config.workspaces.push(WorkspaceConfig {
            id: "remote".to_string(),
            name: "Remote".to_string(),
            path: PathBuf::new(),
            location: WorkspaceLocation::Ssh {
                server_id: "prod".to_string(),
                remote_path: "/srv/project".to_string(),
            },
            pinned: false,
            terminal_shell: "bash".to_string(),
            common_commands: Vec::new(),
        });

        config.validate(None).expect("remote config validates");
        assert!(config.workspaces[1].is_remote());
        assert_eq!(config.workspaces[1].server_id(), Some("prod"));
        assert_eq!(config.workspaces[1].remote_path(), Some("/srv/project"));
        assert_eq!(
            config.workspaces[1].display_path(Some(&config.remote_servers[0])),
            "Prod:/srv/project"
        );
        let serialized = serde_json::to_string(&config.remote_servers).expect("serialize servers");
        assert!(!serialized.contains("s3cret"));
        assert!(!serialized.contains("privateKey"));
        assert!(!serialized.contains("apiKey"));
        // Key-mode profiles omit password entirely.
        assert!(!serialized.contains("\"password\""));
    }

    #[test]
    fn remote_server_defaults_accept_home_shorthand_and_password_auth() {
        let workspace_dir = tempfile::tempdir().expect("workspace");
        let mut config = GlobalConfig::first_run(workspace_dir.path().to_path_buf());
        config.remote_servers.push(RemoteServerProfile {
            id: "home".to_string(),
            name: "Home".to_string(),
            host_alias: "box".to_string(),
            user: Some("root".to_string()),
            auth_method: RemoteAuthMethod::Password,
            password: Some("s3cret".to_string()),
            default_remote_root: Some("~/projects".to_string()),
            connect_timeout_ms: DEFAULT_REMOTE_CONNECT_TIMEOUT_MS,
            ..RemoteServerProfile::default()
        });
        config
            .validate(None)
            .expect("home + password config validates");

        let debug = format!("{:?}", config.remote_servers[0]);
        assert!(debug.contains(REDACTED_SECRET));
        assert!(!debug.contains("s3cret"));

        let log_json = config.to_redacted_log_json().expect("redacted json");
        assert!(log_json.contains(REDACTED_SECRET));
        assert!(!log_json.contains("s3cret"));

        config.remote_servers[0].default_remote_root = Some("~other/path".to_string());
        let err = config.validate(None).expect_err("~other should fail");
        assert!(err.to_string().contains("home shorthand"));

        config.remote_servers[0].default_remote_root = Some("~/ok".to_string());
        config.remote_servers[0].password = None;
        let err = config
            .validate(None)
            .expect_err("password auth without password fails");
        assert!(err.to_string().contains("password is required"));
    }

    #[test]
    fn remote_server_missing_auth_method_deserializes_as_key() {
        let json = r#"{
            "id": "legacy",
            "name": "Legacy",
            "hostAlias": "legacy-box",
            "connectTimeoutMs": 15000
        }"#;
        let profile: RemoteServerProfile =
            serde_json::from_str(json).expect("legacy profile deserializes");
        assert_eq!(profile.auth_method, RemoteAuthMethod::Key);
        assert!(profile.password.is_none());
        assert!(!profile.password_configured());
    }

    #[test]
    fn config_loads_remote_servers_locations_and_mcp_execution_hosts() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());
        let local_path = profile.path().join("local");
        let legacy_path = profile.path().join("legacy");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        fs::write(
            &paths.config_file,
            format!(
                r#"{{
  "schema_version": 1,
  "app": {{ "active_workspace_id": "local" }},
  "providers": [],
  "models": [],
  "remoteServers": [
    {{
      "id": "srv",
      "name": "Build Server",
      "hostAlias": "build-box",
      "defaultRemoteRoot": "/work",
      "lastKnownTarget": "linux-x64",
      "sidecarInstallState": "available"
    }}
  ],
  "mcp": {{
    "servers": [
      {{ "id": "auto", "name": "Auto", "enabled": true, "transport": "stdio", "command": "tool", "args": [] }},
      {{ "id": "local", "name": "Local", "enabled": true, "transport": "streamable-http", "url": "http://127.0.0.1:3000/mcp", "args": [], "executionHost": "local" }},
      {{ "id": "workspace", "name": "Workspace", "enabled": true, "transport": "stdio", "command": "tool", "args": [], "executionHost": "workspace" }}
    ]
  }},
  "skills": {{ "directories": [], "detected": [], "enabled": [] }},
  "workspaces": [
    {{ "id": "legacy", "name": "Legacy Path", "path": {:?} }},
    {{ "id": "local", "name": "Local", "path": {:?}, "location": {{ "type": "local" }} }},
    {{ "id": "ssh", "name": "SSH", "path": "", "location": {{ "type": "ssh", "serverId": "srv", "remotePath": "/work/repo" }} }}
  ]
}}"#,
                legacy_path, local_path
            ),
        )
        .expect("config write");

        let loaded = load_global_config(&paths.config_file).expect("remote config should load");

        assert_eq!(loaded.remote_servers.len(), 1);
        assert_eq!(loaded.remote_servers[0].host_alias, "build-box");
        assert_eq!(loaded.workspaces[0].location, WorkspaceLocation::Local);
        assert_eq!(
            loaded.workspaces[0].local_path(),
            Some(legacy_path.as_path())
        );
        assert_eq!(loaded.workspaces[1].location, WorkspaceLocation::Local);
        assert_eq!(
            loaded.workspaces[1].local_path(),
            Some(local_path.as_path())
        );
        assert_eq!(loaded.workspaces[2].server_id(), Some("srv"));
        assert_eq!(loaded.workspaces[2].remote_path(), Some("/work/repo"));
        assert_eq!(loaded.mcp.servers[0].execution_host, McpExecutionHost::Auto);
        assert_eq!(
            loaded.mcp.servers[1].execution_host,
            McpExecutionHost::Local
        );
        assert_eq!(
            loaded.mcp.servers[2].execution_host,
            McpExecutionHost::Workspace
        );
    }

    #[test]
    fn ssh_workspace_rejects_relative_remote_path() {
        let workspace_dir = tempfile::tempdir().expect("workspace");
        let mut config = GlobalConfig::first_run(workspace_dir.path().to_path_buf());
        config.remote_servers.push(RemoteServerProfile {
            id: "prod".to_string(),
            name: "Prod".to_string(),
            host_alias: "prod-box".to_string(),
            connect_timeout_ms: DEFAULT_REMOTE_CONNECT_TIMEOUT_MS,
            ..RemoteServerProfile::default()
        });
        config.workspaces.push(WorkspaceConfig {
            id: "remote".to_string(),
            name: "Remote".to_string(),
            path: PathBuf::new(),
            location: WorkspaceLocation::Ssh {
                server_id: "prod".to_string(),
                remote_path: "relative/project".to_string(),
            },
            pinned: false,
            terminal_shell: "bash".to_string(),
            common_commands: Vec::new(),
        });

        let error = config
            .validate(None)
            .expect_err("relative remote path fails");
        assert!(error.to_string().contains("absolute remote path"));
    }

    #[test]
    fn api_audit_settings_default_and_validate() {
        let mut config = GlobalConfig::first_run(PathBuf::from("/tmp/foco-workspace"));

        assert_eq!(
            config.app.api_audit.request_detail_retention_days,
            DEFAULT_API_REQUEST_DETAIL_RETENTION_DAYS
        );
        assert!(config.app.api_audit.save_request_response_details);

        config.app.api_audit.request_detail_retention_days = 0;
        assert!(config.validate(None).is_err());
    }

    #[test]
    fn load_or_create_keeps_skill_directories_unmanaged() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let config = GlobalConfig::first_run(paths.workspace_dir.clone());
        fs::write(
            &paths.config_file,
            serde_json::to_string_pretty(&config).expect("serialize config"),
        )
        .expect("config write");

        let loaded =
            load_or_create_global_config_at(profile.path()).expect("existing config should load");

        assert!(loaded.config.skills.directories.is_empty());

        let saved = load_global_config(&paths.config_file).expect("saved config reload");
        assert_eq!(saved.skills.directories, loaded.config.skills.directories);
    }

    #[test]
    fn load_or_create_renames_legacy_default_workspace() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        fs::write(
            &paths.config_file,
            format!(
                r#"{{
  "schema_version": 1,
  "app": {{ "active_workspace_id": "default" }},
  "providers": [],
  "models": [],
  "mcp": {{ "servers": [] }},
  "skills": {{ "directories": [], "detected": [], "enabled": [] }},
  "workspaces": [
    {{ "id": "default", "name": "Default Workspace", "path": {:?} }}
  ]
}}"#,
                paths.workspace_dir
            ),
        )
        .expect("config write");

        let loaded =
            load_or_create_global_config_at(profile.path()).expect("legacy config should load");

        assert_eq!(loaded.config.workspaces[0].name, DEFAULT_WORKSPACE_NAME);

        let saved = load_global_config(&paths.config_file).expect("saved config reload");
        assert_eq!(saved.workspaces[0].name, DEFAULT_WORKSPACE_NAME);
    }

    #[test]
    fn load_or_create_keeps_user_named_default_workspace() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir.clone());
        config.workspaces[0].name = "My Workspace".to_string();
        fs::write(
            &paths.config_file,
            serde_json::to_string_pretty(&config).expect("serialize config"),
        )
        .expect("config write");

        let loaded =
            load_or_create_global_config_at(profile.path()).expect("existing config should load");

        assert_eq!(loaded.config.workspaces[0].name, "My Workspace");
    }

    #[test]
    fn load_or_create_accepts_legacy_skill_config() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        fs::write(
            &paths.config_file,
            format!(
                r#"{{
  "schema_version": 1,
  "app": {{ "active_workspace_id": "default" }},
  "providers": [],
  "models": [],
  "mcp": {{ "servers": [] }},
  "skills": {{ "enabled": ["legacy-skill"] }},
  "workspaces": [
    {{ "id": "default", "name": "Default Workspace", "path": {:?} }}
  ]
}}"#,
                paths.workspace_dir
            ),
        )
        .expect("config write");

        let loaded = load_or_create_global_config_at(profile.path())
            .expect("legacy skill config should load");

        assert_eq!(loaded.config.skills.enabled, vec!["legacy-skill"]);
        assert!(loaded.config.skills.disabled.is_empty());
        assert!(loaded.config.skills.disabled_locations.is_empty());
        assert!(loaded.config.skills.directories.is_empty());
    }

    #[test]
    fn disabled_skill_locations_round_trip_and_accept_removed_workspace_ids() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");
        loaded.config.skills.disabled_locations = vec![
            "global:agents".to_string(),
            "workspace:removed-workspace:claude".to_string(),
        ];

        save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect("disabled locations should save");
        let serialized = fs::read_to_string(&loaded.paths.config_file)
            .expect("serialized config should be readable");
        assert!(serialized.contains("\"disabled_locations\""));

        let reloaded = load_global_config(&loaded.paths.config_file)
            .expect("disabled locations should reload");

        assert_eq!(
            reloaded.skills.disabled_locations,
            vec![
                "global:agents".to_string(),
                "workspace:removed-workspace:claude".to_string(),
            ]
        );
    }

    #[test]
    fn empty_disabled_skill_locations_are_omitted_from_config() {
        let profile = tempfile::tempdir().expect("temp profile");
        let loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect("empty disabled locations should save");
        let serialized = fs::read_to_string(&loaded.paths.config_file)
            .expect("serialized config should be readable");

        assert!(!serialized.contains("\"disabled_locations\""));
    }

    #[test]
    fn invalid_disabled_skill_location_is_rejected() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");
        loaded.config.skills.disabled_locations = vec!["/tmp/skills".to_string()];

        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("absolute path must not be accepted as a location id");

        assert!(error.to_string().contains("invalid skill location id"));
    }

    #[test]
    fn load_accepts_legacy_app_settings_without_web_server() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        fs::write(
            &paths.config_file,
            format!(
                r#"{{
  "schema_version": 1,
  "app": {{ "active_workspace_id": "default" }},
  "providers": [],
  "models": [],
  "mcp": {{ "servers": [] }},
  "skills": {{ "directories": [], "detected": [], "enabled": [] }},
  "workspaces": [
    {{ "id": "default", "name": "Default Workspace", "path": {:?} }}
  ]
}}"#,
                paths.workspace_dir
            ),
        )
        .expect("config write");

        let loaded = load_global_config(&paths.config_file).expect("legacy config should load");

        assert_eq!(loaded.app.language, DEFAULT_APP_LANGUAGE);
        assert_eq!(loaded.app.theme, DEFAULT_APP_THEME);
        assert!(!loaded.app.auto_start_enabled);
        assert!(loaded.app.default_team_mode_enabled);
        assert_eq!(loaded.app.web_server, WebServerSettings::default());
        assert!(!loaded.workspaces[0].pinned);
        assert_eq!(loaded.workspaces[0].terminal_shell, DEFAULT_TERMINAL_SHELL);
    }

    #[test]
    fn load_rejects_unknown_config_fields() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        fs::write(
            &paths.config_file,
            format!(
                r#"{{
  "schema_version": 1,
  "app": {{ "active_workspace_id": "default" }},
  "providers": [],
  "models": [],
  "mcp": {{ "servers": [] }},
  "skills": {{ "directories": [], "detected": [], "enabled": [] }},
  "workspaces": [
    {{ "id": "default", "name": "Default Workspace", "path": {:?} }}
  ],
  "unexpected": true
}}"#,
                paths.workspace_dir
            ),
        )
        .expect("config write");

        let error = load_global_config(&paths.config_file).expect_err("unknown field should fail");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn load_rejects_invalid_web_server_settings() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.app.web_server.listen_host = "localhost".to_string();

        let error = save_global_config(&paths.config_file, &config)
            .expect_err("non-IP listen host should fail");

        assert!(
            error
                .to_string()
                .contains("listen_host must be an IP address")
        );

        config.app.web_server.listen_host = DEFAULT_WEB_SERVER_HOST.to_string();
        config.app.web_server.listen_port = 0;
        let error = save_global_config(&paths.config_file, &config)
            .expect_err("zero listen port should fail");

        assert!(error.to_string().contains("listen_port must be a number"));

        config.app.web_server.listen_port = DEFAULT_WEB_SERVER_PORT;
        config.app.web_server.password_hash = Some("plain-password".to_string());
        let error = save_global_config(&paths.config_file, &config)
            .expect_err("plain password hash should fail");

        assert!(error.to_string().contains("password_hash must use sha256"));
    }

    #[test]
    fn load_rejects_invalid_api_proxy_settings() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.providers.push(ProviderSettings {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            kind: "openai-chat".to_string(),
            enabled: true,
            base_url: None,
            api_key: None,
            auto_sync_models: false,
            model_sync_filter_regex: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
            api_proxy: ApiProxySettings {
                enabled: true,
                proxy_type: HTTP_PROXY_KIND.to_string(),
                url: String::new(),
            },
        });

        let error = save_global_config(&paths.config_file, &config)
            .expect_err("enabled proxy without URL should fail");
        assert!(
            error
                .to_string()
                .contains("provider.api_proxy.url must not be empty")
        );

        config.providers[0].api_proxy.url = "127.0.0.1:7890".to_string();
        config.providers[0].api_proxy.proxy_type = "ftp".to_string();
        let error = save_global_config(&paths.config_file, &config)
            .expect_err("unsupported proxy type should fail");
        assert!(error.to_string().contains("provider.api_proxy.proxy_type"));

        config.providers[0].api_proxy.proxy_type = SOCKS_PROXY_KIND.to_string();
        config.providers[0].api_proxy.url = "http://127.0.0.1:7890".to_string();
        let error = save_global_config(&paths.config_file, &config)
            .expect_err("proxy URL type mismatch should fail");
        assert!(error.to_string().contains("does not match proxy type"));
    }

    #[test]
    fn load_rejects_invalid_provider_model_sync_filter_regex() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.providers.push(ProviderSettings {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            kind: "openai-chat".to_string(),
            enabled: true,
            base_url: None,
            api_key: None,
            auto_sync_models: true,
            model_sync_filter_regex: Some("(".to_string()),
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
            api_proxy: ApiProxySettings::default(),
        });

        let error = save_global_config(&paths.config_file, &config)
            .expect_err("invalid provider model sync regex should fail");
        assert!(
            error
                .to_string()
                .contains("model_sync_filter_regex is invalid")
        );
    }

    #[test]
    fn load_accepts_provider_model_sync_filter_regex_with_negative_lookahead() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.providers.push(ProviderSettings {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            kind: "openai-chat".to_string(),
            enabled: true,
            base_url: None,
            api_key: None,
            auto_sync_models: true,
            model_sync_filter_regex: Some("^(?!gpt).*".to_string()),
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
            api_proxy: ApiProxySettings::default(),
        });

        save_global_config(&paths.config_file, &config)
            .expect("negative lookahead provider model sync regex should save");
    }

    #[test]
    fn load_rejects_invalid_web_search_api_proxy_settings() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.web_search.api_proxy = ApiProxySettings {
            enabled: true,
            proxy_type: HTTP_PROXY_KIND.to_string(),
            url: String::new(),
        };

        let error = save_global_config(&paths.config_file, &config)
            .expect_err("enabled web search proxy without URL should fail");
        assert!(
            error
                .to_string()
                .contains("web_search.api_proxy.url must not be empty")
        );

        config.web_search.api_proxy.proxy_type = SOCKS_PROXY_KIND.to_string();
        config.web_search.api_proxy.url = "http://127.0.0.1:7890".to_string();
        let error = save_global_config(&paths.config_file, &config)
            .expect_err("web search proxy URL type mismatch should fail");
        assert!(error.to_string().contains("does not match proxy type"));
    }

    #[test]
    fn load_rejects_enabled_web_search_without_active_provider_key() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.web_search.enabled = true;
        config.web_search.active_provider = WEB_SEARCH_PROVIDER_TAVILY.to_string();

        let error = save_global_config(&paths.config_file, &config)
            .expect_err("enabled web search without token should fail");
        assert!(
            error
                .to_string()
                .contains("web_search.tavily api key must be set")
        );

        config.web_search.tavily_api_key = Some("token".to_string());
        save_global_config(&paths.config_file, &config)
            .expect("enabled web search with active token should save");
    }

    #[test]
    fn load_rejects_unsupported_app_language() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.app.language = "fr".to_string();

        let error = save_global_config(&paths.config_file, &config)
            .expect_err("unsupported language should fail");

        assert!(
            error
                .to_string()
                .contains("app.language 'fr' is unsupported")
        );
    }

    #[test]
    fn load_rejects_unsupported_app_theme() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.app.theme = "sepia".to_string();

        let error = save_global_config(&paths.config_file, &config)
            .expect_err("unsupported theme should fail");

        assert!(
            error
                .to_string()
                .contains("app.theme 'sepia' is unsupported")
        );
    }

    #[test]
    fn load_rejects_unsupported_workspace_terminal_shell() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.workspaces[0].terminal_shell = "fish".to_string();

        let error = save_global_config(&paths.config_file, &config)
            .expect_err("unsupported terminal shell should fail");

        assert!(
            error
                .to_string()
                .contains("workspace.terminal_shell 'fish' is unsupported")
        );
    }

    #[test]
    fn load_rejects_empty_custom_system_prompt() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.prompts.system_prompt = Some("   ".to_string());

        let error = save_global_config(&paths.config_file, &config)
            .expect_err("empty custom system prompt should fail");

        assert!(
            error
                .to_string()
                .contains("prompts.system_prompt must not be empty")
        );
    }

    #[test]
    fn load_rejects_duplicate_system_prompt_name() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.prompts.system_prompts = vec![
            SystemPromptSettings {
                name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
                content: "Default prompt.".to_string(),
            },
            SystemPromptSettings {
                name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
                content: "Duplicate prompt.".to_string(),
            },
        ];

        let error = save_global_config(&paths.config_file, &config)
            .expect_err("duplicate system prompt name should fail");

        assert!(
            error
                .to_string()
                .contains("duplicate system prompt name 'Default'")
        );
    }

    #[test]
    fn model_system_prompt_must_exist() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        loaded.config.models.push(ModelSettings {
            id: "manual-model".to_string(),
            display_name: "Manual Model".to_string(),
            enabled: false,
            provider_ids: Vec::new(),
            active_provider_id: None,
            thinking_level: None,
            system_prompt_name: "Missing".to_string(),
            metadata_key: None,
            metadata_source_url: None,
            metadata_refreshed_at: None,
            limits: None,
            input_modalities: vec!["text".to_string()],
            output_modalities: vec!["text".to_string()],
        });

        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("missing model system prompt should fail");

        assert!(
            error
                .to_string()
                .contains("system_prompt_name 'Missing' references missing system prompt")
        );
    }

    #[test]
    fn image_generation_system_prompt_is_builtin_for_models() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        loaded.config.models.push(ModelSettings {
            id: "image-model".to_string(),
            display_name: "Image Model".to_string(),
            enabled: false,
            provider_ids: Vec::new(),
            active_provider_id: None,
            thinking_level: None,
            system_prompt_name: IMAGE_GENERATION_SYSTEM_PROMPT_NAME.to_string(),
            metadata_key: None,
            metadata_source_url: None,
            metadata_refreshed_at: None,
            limits: None,
            input_modalities: vec!["text".to_string()],
            output_modalities: vec!["image".to_string()],
        });

        save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect("image generation system prompt should be builtin");
    }

    #[test]
    fn load_rejects_workspace_common_command_without_command() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");
        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.workspaces[0]
            .common_commands
            .push(WorkspaceCommonCommand {
                name: "Dev".to_string(),
                command: " ".to_string(),
            });

        let error = save_global_config(&paths.config_file, &config)
            .expect_err("empty common command should fail");

        assert!(
            error
                .to_string()
                .contains("workspace 'default' common_commands[0].command must not be empty")
        );
    }

    #[test]
    fn load_rejects_active_workspace_that_is_not_registered() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());

        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");

        let mut config = GlobalConfig::first_run(paths.workspace_dir);
        config.app.active_workspace_id = "missing".to_string();
        fs::write(
            &paths.config_file,
            serde_json::to_string_pretty(&config).expect("serialize config"),
        )
        .expect("config write");

        let error =
            load_global_config(&paths.config_file).expect_err("missing workspace should fail");

        assert!(error.to_string().contains("does not match any workspace"));
    }

    #[test]
    fn provider_api_keys_are_redacted_for_logs() {
        let mut config = GlobalConfig::first_run(PathBuf::from(r"C:\Users\foco\.foco\workspace"));
        config.providers.push(ProviderSettings {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            kind: "openai".to_string(),
            enabled: true,
            base_url: None,
            api_key: Some("sk-test-secret".to_string()),
            auto_sync_models: false,
            model_sync_filter_regex: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
            api_proxy: ApiProxySettings::default(),
        });

        let log_json = config.to_redacted_log_json().expect("redacted json");

        assert!(!log_json.contains("sk-test-secret"));
        assert!(log_json.contains(REDACTED_SECRET));
    }

    #[test]
    fn web_auth_password_hash_is_redacted_for_logs() {
        let mut config = GlobalConfig::first_run(PathBuf::from(r"C:\Users\foco\.foco\workspace"));
        config.app.web_server.password_hash = Some(
            "sha256:00112233445566778899aabbccddeeff:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
        );

        let log_json = config.to_redacted_log_json().expect("redacted json");

        assert!(!log_json.contains("00112233445566778899aabbccddeeff"));
        assert!(log_json.contains(REDACTED_SECRET));
    }

    #[test]
    fn mcp_credentials_are_redacted_for_logs() {
        let mut config = GlobalConfig::first_run(PathBuf::from(r"C:\Users\foco\.foco\workspace"));
        config.mcp.servers.push(McpServerConfig {
            id: "docs".to_string(),
            name: "Docs".to_string(),
            enabled: true,
            transport: "stdio".to_string(),
            command: Some("docs-mcp".to_string()),
            args: vec!["--api-key".to_string(), "mcp-secret".to_string()],
            url: None,
            execution_host: McpExecutionHost::Local,
        });
        config.mcp.servers.push(McpServerConfig {
            id: "remote-docs".to_string(),
            name: "Remote Docs".to_string(),
            enabled: true,
            transport: "streamable-http".to_string(),
            command: None,
            args: Vec::new(),
            url: Some("https://example.test/mcp?token=mcp-secret".to_string()),
            execution_host: McpExecutionHost::Local,
        });

        let log_json = config.to_redacted_log_json().expect("redacted json");

        assert!(!log_json.contains("mcp-secret"));
        assert!(!log_json.contains("token="));
        assert!(log_json.contains("docs-mcp"));
        assert!(log_json.contains(REDACTED_SECRET));
    }

    #[test]
    fn hook_config_rejects_unsupported_events_and_non_tool_if_filters() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");
        loaded.config.hooks.hooks.insert(
            "Setup".to_string(),
            vec![HookMatcherGroup {
                enabled: true,
                matcher: None,
                hooks: vec![HookHandler {
                    enabled: true,
                    handler_type: HOOK_HANDLER_COMMAND.to_string(),
                    if_filter: None,
                    command: Some("echo ok".to_string()),
                    args: Vec::new(),
                    shell: None,
                    url: None,
                    server_id: None,
                    tool_name: None,
                    prompt: None,
                    timeout: None,
                    async_hook: false,
                    async_rewake: false,
                    status_message: None,
                    input: None,
                }],
            }],
        );
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("unsupported hook event should fail");
        assert!(error.to_string().contains("does not support yet"));

        loaded.config.hooks.hooks.clear();
        loaded.config.hooks.hooks.insert(
            "SessionStart".to_string(),
            vec![HookMatcherGroup {
                enabled: true,
                matcher: None,
                hooks: vec![HookHandler {
                    enabled: true,
                    handler_type: HOOK_HANDLER_COMMAND.to_string(),
                    if_filter: Some("run_command(git *)".to_string()),
                    command: Some("echo ok".to_string()),
                    args: Vec::new(),
                    shell: None,
                    url: None,
                    server_id: None,
                    tool_name: None,
                    prompt: None,
                    timeout: None,
                    async_hook: false,
                    async_rewake: false,
                    status_message: None,
                    input: None,
                }],
            }],
        );
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("non-tool if filter should fail");
        assert!(error.to_string().contains("if is only supported"));
    }

    #[test]
    fn save_global_config_updates_existing_file() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        loaded.config.workspaces[0].name = "Renamed Workspace".to_string();
        save_global_config(&loaded.paths.config_file, &loaded.config).expect("config save");

        let reloaded = load_global_config(&loaded.paths.config_file).expect("config reload");

        assert_eq!(reloaded.workspaces[0].name, "Renamed Workspace");
    }

    #[test]
    fn automatic_memory_extraction_mode_can_be_saved() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        loaded.config.memory.enabled = true;
        loaded.config.memory.extraction_mode = "automatic".to_string();

        save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect("automatic memory extraction mode should save");
    }

    #[test]
    fn model_memory_retrieval_mode_can_be_saved() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        loaded.config.memory.enabled = true;
        loaded.config.memory.retrieval_mode = "llm".to_string();

        save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect("model memory retrieval mode should save");
    }

    #[test]
    fn memory_retrieval_model_must_exist() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        loaded.config.memory.retrieval_model_id = Some("missing-model".to_string());

        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("missing retrieval model should fail");

        assert!(
            error
                .to_string()
                .contains("memory.retrieval_model_id references missing model")
        );
    }

    #[test]
    fn memory_dream_defaults_are_loaded_for_old_configs() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());
        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");

        let config = GlobalConfig::first_run(paths.workspace_dir.clone());
        let mut json = serde_json::to_value(&config).expect("config json");
        json.get_mut("memory")
            .and_then(Value::as_object_mut)
            .expect("memory object")
            .remove("dream");
        fs::write(
            &paths.config_file,
            serde_json::to_string_pretty(&json).expect("serialize config"),
        )
        .expect("config write");

        let loaded = load_global_config(&paths.config_file).expect("old config should load");

        assert_eq!(loaded.memory.dream, MemoryDreamSettings::default());
    }

    #[test]
    fn spec_defaults_are_loaded_for_old_configs() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());
        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");

        let config = GlobalConfig::first_run(paths.workspace_dir.clone());
        let mut json = serde_json::to_value(&config).expect("config json");
        json.as_object_mut().expect("config object").remove("spec");
        fs::write(
            &paths.config_file,
            serde_json::to_string_pretty(&json).expect("serialize config"),
        )
        .expect("config write");

        let loaded = load_global_config(&paths.config_file).expect("old config should load");

        assert_eq!(loaded.spec, SpecSettings::default());
    }

    #[test]
    fn plan_defaults_are_loaded_for_old_configs() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());
        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");

        let config = GlobalConfig::first_run(paths.workspace_dir.clone());
        let mut json = serde_json::to_value(&config).expect("config json");
        json.as_object_mut().expect("config object").remove("plan");
        fs::write(
            &paths.config_file,
            serde_json::to_string_pretty(&json).expect("serialize config"),
        )
        .expect("config write");

        let loaded = load_global_config(&paths.config_file).expect("old config should load");

        assert_eq!(loaded.plan, PlanSettings::default());
    }

    #[test]
    fn app_defaults_are_loaded_for_old_configs() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());
        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");

        let config = GlobalConfig::first_run(paths.workspace_dir.clone());
        let mut json = serde_json::to_value(&config).expect("config json");
        let app = json
            .get_mut("app")
            .and_then(Value::as_object_mut)
            .expect("app object");
        app.remove("auto_update_check_enabled");
        app.remove("chat_title_generation_model_id");
        app.remove("runtime_tool_state_compression_enabled");
        fs::write(
            &paths.config_file,
            serde_json::to_string_pretty(&json).expect("serialize config"),
        )
        .expect("config write");

        let loaded = load_global_config(&paths.config_file).expect("old config should load");

        assert!(!loaded.app.auto_update_check_enabled);
        assert_eq!(
            loaded.app.chat_title_generation_model_id.as_deref(),
            Some(CHAT_TITLE_GENERATION_CURRENT_CHAT_MODEL)
        );
        assert!(!loaded.app.runtime_tool_state_compression_enabled);
    }

    #[test]
    fn plan_settings_are_validated() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        loaded.config.plan.merge_automation_mode = "surprise".to_string();
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("bad plan merge mode should fail");

        assert!(error.to_string().contains("plan.merge_automation_mode"));

        loaded.config.plan.merge_automation_mode = default_plan_merge_automation_mode();
        loaded.config.plan.mode_model_id = Some("missing-model".to_string());
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("missing plan mode model should fail");
        assert!(error.to_string().contains("plan.mode_model_id"));

        add_enabled_spec_model(&mut loaded.config, "plan-mode-model");
        loaded.config.plan.mode_model_id = Some("plan-mode-model".to_string());
        save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect("valid plan mode model should save");
        let reloaded = load_global_config(&loaded.paths.config_file).expect("config reload");
        assert_eq!(
            reloaded.plan.mode_model_id.as_deref(),
            Some("plan-mode-model")
        );

        loaded.config.models.iter_mut().for_each(|model| {
            if model.id == "plan-mode-model" {
                model.enabled = false;
            }
        });
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("disabled plan mode model should fail");
        assert!(error.to_string().contains("plan.mode_model_id"));
    }

    #[test]
    fn plan_mode_model_id_defaults_for_old_configs() {
        let profile = tempfile::tempdir().expect("temp profile");
        let paths = FocoPaths::from_user_profile(profile.path());
        fs::create_dir_all(&paths.workspace_dir).expect("workspace directory");
        fs::create_dir_all(&paths.root_dir).expect("root directory");

        let config = GlobalConfig::first_run(paths.workspace_dir.clone());
        let mut json = serde_json::to_value(&config).expect("config json");
        let plan = json
            .get_mut("plan")
            .and_then(Value::as_object_mut)
            .expect("plan object");
        plan.remove("modeModelId");
        fs::write(
            &paths.config_file,
            serde_json::to_string_pretty(&json).expect("serialize config"),
        )
        .expect("config write");

        let loaded = load_global_config(&paths.config_file).expect("old config should load");
        assert_eq!(loaded.plan.mode_model_id, None);
    }

    #[test]
    fn spec_settings_can_be_saved() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");
        add_enabled_spec_model(&mut loaded.config, "spec-model");
        loaded.config.spec = SpecSettings {
            auto_enabled: false,
            generation_model_id: Some("spec-model".to_string()),
            generation_system_prompt: Some("Generate a concise project spec.".to_string()),
            update_system_prompt: Some("Update the project spec if needed.".to_string()),
            llm_timeout_ms: 120_000,
        };

        save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect("spec settings should save");
        let reloaded = load_global_config(&loaded.paths.config_file).expect("config reload");

        assert_eq!(reloaded.spec, loaded.config.spec);
    }

    #[test]
    fn spec_settings_are_validated() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        loaded.config.spec.generation_model_id = Some("missing-model".to_string());
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("missing spec model should fail");
        assert!(error.to_string().contains("spec.generation_model_id"));

        add_enabled_spec_model(&mut loaded.config, "spec-model");
        loaded.config.spec.generation_model_id = Some("spec-model".to_string());
        loaded.config.spec.generation_system_prompt = Some(" ".to_string());
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("blank spec prompt should fail");
        assert!(
            error
                .to_string()
                .contains("spec.generation_system_prompt must not be empty")
        );
    }

    #[test]
    fn memory_dream_settings_can_be_saved() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");
        loaded
            .config
            .models
            .push(enabled_memory_model("dream-model"));
        loaded.config.memory.dream = MemoryDreamSettings {
            enabled: true,
            auto_enabled: true,
            mode: "deterministic_only".to_string(),
            model_id: Some("dream-model".to_string()),
            workspace_interval_days: 3,
            global_interval_days: 14,
            create_transcript_chat: false,
            max_facts_per_run: 25,
            max_changes_per_run: 10,
            scheduler_scan_minutes: 15,
            workspace_threshold_facts: 40,
            global_threshold_facts: 80,
            llm_timeout_ms: 120_000,
        };

        save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect("dream settings should save");
        let reloaded = load_global_config(&loaded.paths.config_file).expect("config reload");

        assert_eq!(reloaded.memory.dream, loaded.config.memory.dream);
    }

    #[test]
    fn memory_dream_settings_are_validated() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");
        loaded
            .config
            .models
            .push(enabled_memory_model("dream-model"));

        loaded.config.memory.dream.mode = "agent".to_string();
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("unsupported dream mode should fail");
        assert!(
            error
                .to_string()
                .contains("memory.dream.mode has unsupported value")
        );

        loaded.config.memory.dream.mode = "llm".to_string();
        loaded.config.memory.dream.workspace_interval_days = 0;
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("zero workspace interval should fail");
        assert!(
            error
                .to_string()
                .contains("memory.dream.workspace_interval_days must be greater than 0")
        );

        loaded.config.memory.dream.workspace_interval_days = 7;
        loaded.config.memory.dream.max_changes_per_run = 0;
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("zero change limit should fail");
        assert!(
            error
                .to_string()
                .contains("memory.dream.max_changes_per_run must be greater than 0")
        );

        loaded.config.memory.dream.max_changes_per_run = 50;
        loaded.config.memory.dream.workspace_threshold_facts = 0;
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("zero workspace threshold should fail");
        assert!(
            error
                .to_string()
                .contains("memory.dream.workspace_threshold_facts must be greater than 0")
        );

        loaded.config.memory.dream.workspace_threshold_facts = 50;
        loaded.config.memory.dream.model_id = Some("missing-model".to_string());
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("missing dream model should fail");
        assert!(
            error
                .to_string()
                .contains("memory.dream.model_id references missing or disabled model")
        );

        loaded.config.models.push(ModelSettings {
            enabled: false,
            ..enabled_memory_model("disabled-dream-model")
        });
        loaded.config.memory.dream.model_id = Some("disabled-dream-model".to_string());
        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("disabled dream model should fail");
        assert!(
            error
                .to_string()
                .contains("memory.dream.model_id references missing or disabled model")
        );
    }

    fn enabled_memory_model(id: &str) -> ModelSettings {
        ModelSettings {
            id: id.to_string(),
            display_name: id.to_string(),
            enabled: true,
            provider_ids: Vec::new(),
            active_provider_id: None,
            thinking_level: None,
            system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
            metadata_key: None,
            metadata_source_url: None,
            metadata_refreshed_at: None,
            limits: Some(ModelLimits {
                context_window: 128_000,
                max_output_tokens: 16_384,
            }),
            input_modalities: vec!["text".to_string()],
            output_modalities: vec!["text".to_string()],
        }
    }

    fn add_enabled_spec_model(config: &mut GlobalConfig, id: &str) {
        config.providers.push(ProviderSettings {
            id: "spec-provider".to_string(),
            name: "Spec Provider".to_string(),
            kind: foco_providers::OPENAI_RESPONSES_KIND.to_string(),
            enabled: true,
            base_url: None,
            api_key: Some("key".to_string()),
            auto_sync_models: false,
            model_sync_filter_regex: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
            api_proxy: ApiProxySettings::default(),
        });
        config.models.push(ModelSettings {
            provider_ids: vec!["spec-provider".to_string()],
            active_provider_id: Some("spec-provider".to_string()),
            ..enabled_memory_model(id)
        });
    }

    #[test]
    fn disabled_model_can_be_saved_without_limits() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        loaded.config.models.push(ModelSettings {
            id: "manual-model".to_string(),
            display_name: "Manual Model".to_string(),
            enabled: false,
            provider_ids: Vec::new(),
            active_provider_id: None,
            thinking_level: None,
            system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
            metadata_key: None,
            metadata_source_url: None,
            metadata_refreshed_at: None,
            limits: None,
            input_modalities: vec!["text".to_string()],
            output_modalities: vec!["text".to_string()],
        });

        save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect("disabled model without limits should save");
    }

    #[test]
    fn enabled_model_requires_limits() {
        let profile = tempfile::tempdir().expect("temp profile");
        let mut loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        loaded.config.models.push(ModelSettings {
            id: "manual-model".to_string(),
            display_name: "Manual Model".to_string(),
            enabled: true,
            provider_ids: Vec::new(),
            active_provider_id: None,
            thinking_level: None,
            system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
            metadata_key: None,
            metadata_source_url: None,
            metadata_refreshed_at: None,
            limits: None,
            input_modalities: vec!["text".to_string()],
            output_modalities: vec!["text".to_string()],
        });

        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("enabled model without limits should fail");

        assert!(error.to_string().contains("is missing limits"));
    }

    fn config_with_valid_agent_definition() -> GlobalConfig {
        let mut config = GlobalConfig::first_run(std::env::temp_dir().join("foco-agent-config"));
        config.providers.push(ProviderSettings {
            id: "provider-1".to_string(),
            name: "Provider 1".to_string(),
            kind: foco_providers::OPENAI_RESPONSES_KIND.to_string(),
            enabled: true,
            base_url: None,
            api_key: Some("secret-key".to_string()),
            auto_sync_models: false,
            model_sync_filter_regex: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
            api_proxy: ApiProxySettings::default(),
        });
        config.models.push(ModelSettings {
            id: "model-1".to_string(),
            display_name: "Model 1".to_string(),
            enabled: true,
            provider_ids: vec!["provider-1".to_string()],
            active_provider_id: Some("provider-1".to_string()),
            thinking_level: None,
            system_prompt_name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
            metadata_key: None,
            metadata_source_url: None,
            metadata_refreshed_at: None,
            limits: Some(ModelLimits {
                context_window: 128_000,
                max_output_tokens: 16_384,
            }),
            input_modalities: vec!["text".to_string()],
            output_modalities: vec!["text".to_string()],
        });
        config.agent_definitions.push(AgentDefinitionSettings {
            id: AgentDefinitionId::new("agent-definition-coordinator")
                .expect("agent definition id"),
            revision: AGENT_DEFINITION_INITIAL_REVISION,
            name: "Coordinator".to_string(),
            description: "Coordinates work.".to_string(),
            provider_id: "provider-1".to_string(),
            model_id: "model-1".to_string(),
            model_options: AgentModelOptions {
                thinking_level: Some("high".to_string()),
                max_output_tokens: Some(8_192),
            },
            system_prompt: "Coordinate the team.".to_string(),
            allowed_tools: vec!["read_file".to_string()],
            max_instances: 1,
            allowed_execution_workspace_modes: AgentExecutionWorkspaceMode::all(),
            permissions: AgentPermissions::default(),
        });
        config
    }

    #[test]
    fn active_model_provider_route_reports_stable_errors() {
        let config = config_with_valid_agent_definition();
        let (model, provider) = config
            .resolve_active_model_provider("model-1")
            .expect("active route");
        assert_eq!(model.id, "model-1");
        assert_eq!(provider.id, "provider-1");
        assert_eq!(
            config
                .resolve_active_model_provider("missing")
                .expect_err("missing model")
                .to_string(),
            "model was not found: missing"
        );

        let mut disabled_model = config.clone();
        disabled_model.models[0].enabled = false;
        assert_eq!(
            disabled_model
                .resolve_active_model_provider("model-1")
                .expect_err("disabled model")
                .to_string(),
            "model 'model-1' is disabled"
        );

        let mut missing_route = config.clone();
        missing_route.models[0].active_provider_id = None;
        assert_eq!(
            missing_route
                .resolve_active_model_provider("model-1")
                .expect_err("missing active provider")
                .to_string(),
            "model 'model-1' has no active provider"
        );

        let mut unassociated = config.clone();
        unassociated.models[0].active_provider_id = Some("other".to_string());
        assert_eq!(
            unassociated
                .resolve_active_model_provider("model-1")
                .expect_err("unassociated provider")
                .to_string(),
            "active provider 'other' is not associated with model 'model-1'"
        );

        let mut missing_provider = config.clone();
        missing_provider.models[0]
            .provider_ids
            .push("missing-provider".to_string());
        missing_provider.models[0].active_provider_id = Some("missing-provider".to_string());
        assert_eq!(
            missing_provider
                .resolve_active_model_provider("model-1")
                .expect_err("missing provider")
                .to_string(),
            "active provider 'missing-provider' for model 'model-1' was not found"
        );

        let mut disabled_provider = config;
        disabled_provider.providers[0].enabled = false;
        assert_eq!(
            disabled_provider
                .resolve_active_model_provider("model-1")
                .expect_err("disabled provider")
                .to_string(),
            "active provider 'provider-1' for model 'model-1' is disabled"
        );
    }

    #[test]
    fn agent_provider_id_remains_compatible_when_model_route_changes() {
        let mut config = config_with_valid_agent_definition();
        config.providers.push(ProviderSettings {
            id: "provider-2".to_string(),
            name: "Provider 2".to_string(),
            kind: foco_providers::OPENAI_RESPONSES_KIND.to_string(),
            enabled: true,
            base_url: None,
            api_key: Some("secret-key-2".to_string()),
            auto_sync_models: false,
            model_sync_filter_regex: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
            api_proxy: ApiProxySettings::default(),
        });
        config.models[0].provider_ids.push("provider-2".to_string());
        config.models[0].active_provider_id = Some("provider-2".to_string());
        config
            .providers
            .retain(|provider| provider.id != "provider-1");
        config.models[0]
            .provider_ids
            .retain(|provider_id| provider_id != "provider-1");

        config
            .validate(None)
            .expect("stale legacy agent provider remains a compatibility field");
        let json = serde_json::to_string(&config).expect("serialize config");
        let loaded: GlobalConfig = serde_json::from_str(&json).expect("load legacy providerId");
        assert_eq!(loaded.agent_definitions[0].provider_id, "provider-1");
        assert_eq!(
            loaded
                .resolve_active_model_provider("model-1")
                .expect("current route")
                .1
                .id,
            "provider-2"
        );
    }

    #[test]
    fn agent_definition_round_trips_with_strict_schema() {
        let config = config_with_valid_agent_definition();
        config.validate(None).expect("valid agent definition");
        let json = serde_json::to_string(&config.agent_definitions[0]).expect("serialize");
        let round_trip: AgentDefinitionSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(round_trip, config.agent_definitions[0]);

        let mut value = serde_json::to_value(round_trip).expect("definition value");
        value
            .as_object_mut()
            .expect("definition object")
            .insert("unexpected".to_string(), Value::Bool(true));
        let error = serde_json::from_value::<AgentDefinitionSettings>(value)
            .expect_err("unknown definition field should fail");
        assert!(error.to_string().contains("unknown field"));

        let mut value =
            serde_json::to_value(&config.agent_definitions[0]).expect("definition value");
        value
            .as_object_mut()
            .expect("definition object")
            .remove("providerId");
        let error = serde_json::from_value::<AgentDefinitionSettings>(value)
            .expect_err("missing definition field should fail");
        assert!(error.to_string().contains("missing field"));
    }

    #[test]
    fn agent_definition_accepts_max_as_known_thinking_level() {
        let mut config = config_with_valid_agent_definition();
        config.agent_definitions[0].model_options.thinking_level = Some("max".to_string());

        config
            .validate(None)
            .expect("max should be accepted by general agent config validation");
    }

    #[test]
    fn agent_definition_requires_at_least_one_execution_workspace_mode() {
        let mut config = config_with_valid_agent_definition();
        config.agent_definitions[0]
            .allowed_execution_workspace_modes
            .clear();

        let error = config.validate(None).expect_err("empty modes should fail");
        assert!(
            error
                .to_string()
                .contains("allowedExecutionWorkspaceModes must not be empty")
        );
    }

    #[test]
    fn agent_definition_rejects_invalid_model_and_options() {
        let mut config = config_with_valid_agent_definition();
        config.agent_definitions[0].provider_id = "invalid provider".to_string();
        assert!(
            config
                .validate(None)
                .expect_err("invalid legacy provider id")
                .to_string()
                .contains("must not contain whitespace")
        );

        let mut config = config_with_valid_agent_definition();
        config.agent_definitions[0].model_id = "missing".to_string();
        assert!(
            config
                .validate(None)
                .expect_err("missing model")
                .to_string()
                .contains("missing model")
        );

        let mut config = config_with_valid_agent_definition();
        config.agent_definitions[0].model_options.max_output_tokens = Some(20_000);
        assert!(
            config
                .validate(None)
                .expect_err("output limit")
                .to_string()
                .contains("exceeds model")
        );

        let mut config = config_with_valid_agent_definition();
        config.agent_definitions[0].model_options.thinking_level = Some("extreme".to_string());
        assert!(
            config
                .validate(None)
                .expect_err("thinking level")
                .to_string()
                .contains("is unsupported")
        );
    }

    #[test]
    fn agent_definition_rejects_duplicate_names_invalid_permissions_and_limits() {
        let mut config = config_with_valid_agent_definition();
        let mut duplicate = config.agent_definitions[0].clone();
        duplicate.id =
            AgentDefinitionId::new("agent-definition-worker").expect("agent definition id");
        duplicate.name = "coordinator".to_string();
        config.agent_definitions.push(duplicate);
        assert!(
            config
                .validate(None)
                .expect_err("duplicate name")
                .to_string()
                .contains("duplicate case-insensitive name")
        );

        let mut config = config_with_valid_agent_definition();
        config.agent_definitions[0]
            .permissions
            .allowed_agent_definition_ids
            .push(AgentDefinitionId::new("agent-definition-missing").expect("definition id"));
        assert!(
            config
                .validate(None)
                .expect_err("disabled instance creation")
                .to_string()
                .contains("must be empty")
        );

        let mut config = config_with_valid_agent_definition();
        config.agent_definitions[0].max_instances = AGENT_DEFINITION_MAX_INSTANCES + 1;
        assert!(
            config
                .validate(None)
                .expect_err("instance limit")
                .to_string()
                .contains("maxInstances")
        );

        let mut config = config_with_valid_agent_definition();
        config.agent_definitions[0].system_prompt =
            "x".repeat(AGENT_DEFINITION_SYSTEM_PROMPT_MAX_CHARS + 1);
        assert!(
            config
                .validate(None)
                .expect_err("system prompt length")
                .to_string()
                .contains("systemPrompt")
        );
    }

    #[test]
    fn agent_definition_tool_references_require_runtime_catalog_entries() {
        let config = config_with_valid_agent_definition();
        let known_tools = HashSet::from(["read_file".to_string()]);
        validate_agent_definition_tool_references(None, &config.agent_definitions, &known_tools)
            .expect("known tool");

        let error = validate_agent_definition_tool_references(
            None,
            &config.agent_definitions,
            &HashSet::new(),
        )
        .expect_err("unknown tool should fail");
        assert!(
            error
                .to_string()
                .contains("unknown runtime tool 'read_file'")
        );
    }

    #[test]
    fn deleting_definition_does_not_mutate_existing_runtime_snapshot() {
        let mut config = config_with_valid_agent_definition();
        let snapshot = config.agent_definitions[0].clone();
        config.agent_definitions.clear();

        assert_eq!(snapshot.revision, AGENT_DEFINITION_INITIAL_REVISION);
        assert_eq!(snapshot.provider_id, "provider-1");
        assert_eq!(snapshot.model_id, "model-1");
        assert!(config.agent_definitions.is_empty());
    }
}
