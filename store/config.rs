use std::{
    collections::HashSet,
    env, fmt, fs, io,
    path::{Path, PathBuf},
};

use foco_providers::{normalized_base_url, parse_provider_kind};
use serde::{Deserialize, Serialize};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_WORKSPACE_ID: &str = "default";
pub const DEFAULT_WORKSPACE_NAME: &str = "Default Workspace";
pub const REDACTED_SECRET: &str = "<redacted>";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FocoPaths {
    pub root_dir: PathBuf,
    pub config_file: PathBuf,
    pub workspace_dir: PathBuf,
    pub logs_dir: PathBuf,
}

impl FocoPaths {
    pub fn from_user_profile_env() -> Result<Self, ConfigError> {
        let profile = env::var_os("USERPROFILE").ok_or(ConfigError::MissingUserProfile)?;

        if profile.is_empty() {
            return Err(ConfigError::EmptyUserProfile);
        }

        Ok(Self::from_user_profile(profile))
    }

    pub fn from_user_profile(profile: impl Into<PathBuf>) -> Self {
        let root_dir = profile.into().join(".foco");

        Self {
            config_file: root_dir.join("config.json"),
            workspace_dir: root_dir.join("workspace"),
            logs_dir: root_dir.join("logs"),
            root_dir,
        }
    }
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

    let config = load_global_config(&paths.config_file)?;
    validate_workspace_directories(&config, &paths.config_file)?;

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
            },
            providers: Vec::new(),
            models: Vec::new(),
            mcp: McpConfig {
                servers: Vec::new(),
            },
            skills: SkillConfig {
                directories: Vec::new(),
                enabled: Vec::new(),
            },
            workspaces: vec![WorkspaceConfig {
                id: DEFAULT_WORKSPACE_ID.to_string(),
                name: DEFAULT_WORKSPACE_NAME.to_string(),
                path: default_workspace_path,
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
        }

        for skill_id in &self.skills.enabled {
            validate_id(config_path, "skills.enabled", skill_id)?;
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

        serde_json::to_string(&redacted)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppSettings {
    pub active_workspace_id: String,
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
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillConfig {
    pub directories: Vec<PathBuf>,
    pub enabled: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum ConfigError {
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
            Self::EmptyUserProfile => write!(formatter, "USERPROFILE is empty"),
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
            Self::MissingUserProfile => write!(formatter, "USERPROFILE is not set"),
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
            Self::EmptyUserProfile | Self::MissingUserProfile | Self::Validation { .. } => None,
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
    fn first_run_creates_config_workspace_and_default_workspace() {
        let profile = tempfile::tempdir().expect("temp profile");

        let loaded =
            load_or_create_global_config_at(profile.path()).expect("first-run config should load");

        assert!(loaded.paths.root_dir.is_dir());
        assert!(loaded.paths.config_file.is_file());
        assert!(loaded.paths.workspace_dir.is_dir());
        assert_eq!(
            loaded.config.app.active_workspace_id,
            DEFAULT_WORKSPACE_ID.to_string()
        );
        assert_eq!(loaded.config.workspaces.len(), 1);
        assert_eq!(loaded.config.workspaces[0].name, DEFAULT_WORKSPACE_NAME);
        assert_eq!(loaded.config.workspaces[0].path, loaded.paths.workspace_dir);
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
  "skills": {{ "directories": [], "enabled": [] }},
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
        });

        let log_json = config.to_redacted_log_json().expect("redacted json");

        assert!(!log_json.contains("sk-test-secret"));
        assert!(log_json.contains(REDACTED_SECRET));
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
