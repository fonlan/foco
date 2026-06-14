use std::{
    collections::HashSet,
    env, fmt, fs, io,
    net::IpAddr,
    path::{Path, PathBuf},
};

use foco_mcp::{McpServerDefinition, McpTransportKind, validate_server_definitions};
use foco_providers::{
    HTTP_PROXY_KIND, SOCKS_PROXY_KIND, normalized_base_url, normalized_proxy_url,
    parse_provider_kind,
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
pub const DEFAULT_TERMINAL_SHELL: &str = if cfg!(windows) { "powershell" } else { "bash" };
pub const SUPPORTED_TERMINAL_SHELLS: &[&str] = &["powershell", "cmd", "bash", "zsh"];
pub const SUPPORTED_API_PROXY_TYPES: &[&str] = &[HTTP_PROXY_KIND, SOCKS_PROXY_KIND];
pub const DEFAULT_SYSTEM_PROMPT_NAME: &str = "Default";
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

#[derive(Clone, Debug, PartialEq, Eq)]
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
    let temp_file = path.with_extension("json.tmp");

    fs::write(&temp_file, content).map_err(|source| ConfigError::Io {
        path: temp_file.clone(),
        source,
    })?;
    if path.exists() {
        fs::remove_file(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    fs::rename(&temp_file, path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
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
    pub providers: Vec<ProviderSettings>,
    pub models: Vec<ModelSettings>,
    pub mcp: McpConfig,
    pub skills: SkillConfig,
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
                web_server: WebServerSettings::default(),
            },
            hooks: HookConfig::default(),
            memory: MemorySettings::default(),
            prompts: PromptSettings::default(),
            providers: Vec::new(),
            models: Vec::new(),
            mcp: McpConfig {
                servers: Vec::new(),
            },
            skills: SkillConfig {
                directories: Vec::new(),
                detected: Vec::new(),
                disabled: Vec::new(),
                enabled: Vec::new(),
            },
            workspaces: vec![WorkspaceConfig {
                id: DEFAULT_WORKSPACE_ID.to_string(),
                name: DEFAULT_WORKSPACE_NAME.to_string(),
                path: default_workspace_path,
                pinned: false,
                terminal_shell: default_terminal_shell(),
                common_commands: Vec::new(),
            }],
        }
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
        validate_app_language(config_path, &self.app.language)?;
        validate_app_theme(config_path, &self.app.theme)?;
        validate_web_server_settings(config_path, &self.app.web_server)?;
        validate_hook_config(config_path, "hooks", &self.hooks)?;
        validate_memory_settings(config_path, &self.memory, &self.models)?;
        validate_prompt_settings(config_path, &self.prompts)?;
        require_non_empty_list(config_path, "workspaces", self.workspaces.len())?;

        let mut workspace_ids = HashSet::new();

        for workspace in &self.workspaces {
            validate_id(config_path, "workspace.id", &workspace.id)?;
            require_non_empty(config_path, "workspace.name", &workspace.name)?;

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

            if model.enabled {
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
    #[serde(default)]
    pub web_server: WebServerSettings,
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSettings {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
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
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelLimits {
    pub context_window: u64,
    pub max_output_tokens: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpConfig {
    pub servers: Vec<McpServerConfig>,
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
    pub enabled: Vec<String>,
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default = "default_terminal_shell")]
    pub terminal_shell: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub common_commands: Vec<WorkspaceCommonCommand>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceCommonCommand {
    pub name: String,
    pub command: String,
}

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

fn default_terminal_shell() -> String {
    DEFAULT_TERMINAL_SHELL.to_string()
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
        if !workspace.path.is_dir() {
            return invalid_config(
                Some(config_path),
                format!(
                    "workspace '{}' path does not exist or is not a directory: {}",
                    workspace.id,
                    workspace.path.display()
                ),
            );
        }
    }

    Ok(())
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

    Ok(())
}

fn prompt_settings_contains_system_prompt(settings: &PromptSettings, name: &str) -> bool {
    name == DEFAULT_SYSTEM_PROMPT_NAME
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
        assert!(loaded.config.skills.directories.is_empty());
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
        });

        let error = save_global_config(&loaded.paths.config_file, &loaded.config)
            .expect_err("enabled model without limits should fail");

        assert!(error.to_string().contains("is missing limits"));
    }
}
