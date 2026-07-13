use std::collections::HashSet;
use std::path::Path;

use foco_store::config::{
    AgentDefinitionSettings, GlobalConfig, HookConfig, McpConfig, MemorySettings, ModelSettings,
    PlanSettings, PromptSettings, SKILL_SCOPE_GLOBAL, SpecSettings,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ApiError;
use crate::skills::{
    SkillPromptEntry, discover_skills, parse_skill_file, skill_is_disabled,
    skill_is_required_disabled,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidecarRuntimeConfigBundle {
    pub(crate) config_generation: u64,
    pub(crate) hash: String,
    pub(crate) payload: SidecarRuntimeConfigPayload,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidecarRuntimeAppSettings {
    #[serde(default)]
    pub(crate) runtime_tool_state_compression_enabled: bool,
    #[serde(default = "default_sidecar_app_language")]
    pub(crate) language: String,
    #[serde(default = "default_sidecar_llm_request_retry_count")]
    pub(crate) llm_request_retry_count: u32,
}

fn default_sidecar_app_language() -> String {
    "en".to_string()
}

fn default_sidecar_llm_request_retry_count() -> u32 {
    foco_store::config::DEFAULT_LLM_REQUEST_RETRY_COUNT
}

impl Default for SidecarRuntimeAppSettings {
    fn default() -> Self {
        Self {
            runtime_tool_state_compression_enabled: false,
            language: default_sidecar_app_language(),
            llm_request_retry_count: default_sidecar_llm_request_retry_count(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidecarRuntimeConfigPayload {
    #[serde(default)]
    pub(crate) app: SidecarRuntimeAppSettings,
    pub(crate) agent_definitions: Vec<AgentDefinitionSettings>,
    pub(crate) prompts: PromptSettings,
    pub(crate) models: Vec<ModelSettings>,
    #[serde(default)]
    pub(crate) hooks: HookConfig,
    pub(crate) mcp: McpConfig,
    pub(crate) memory: MemorySettings,
    pub(crate) spec: SpecSettings,
    pub(crate) plan: PlanSettings,
    #[serde(default)]
    pub(crate) global_skills: Vec<SidecarRuntimeSkillContent>,
    #[serde(default)]
    pub(crate) disabled_skill_keys: Vec<String>,
    #[serde(default)]
    pub(crate) disabled_skill_location_ids: Vec<String>,
    #[serde(default)]
    pub(crate) required_disabled_skill_keys: Vec<String>,
    #[serde(default)]
    pub(crate) selected_skills: Vec<SidecarRuntimeSkillContent>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidecarRuntimeSkillContent {
    pub(crate) key: String,
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: String,
    pub(crate) path: String,
    #[serde(default = "default_global_skill_scope")]
    pub(crate) scope: String,
    pub(crate) content_markdown: String,
}

fn default_global_skill_scope() -> String {
    SKILL_SCOPE_GLOBAL.to_string()
}

impl SidecarRuntimeSkillContent {
    pub(crate) fn prompt_entry(&self) -> SkillPromptEntry {
        SkillPromptEntry {
            key: self.key.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            scope: self.scope.clone(),
            path: self.path.clone(),
        }
    }
}

pub(crate) fn build_sidecar_runtime_config_bundle(
    user_profile_dir: &Path,
    config: &GlobalConfig,
    config_generation: u64,
) -> Result<SidecarRuntimeConfigBundle, ApiError> {
    let (global_skills, required_disabled_skill_keys) =
        global_skill_content(user_profile_dir, config)?;
    let payload = SidecarRuntimeConfigPayload {
        app: SidecarRuntimeAppSettings {
            runtime_tool_state_compression_enabled: config
                .app
                .runtime_tool_state_compression_enabled,
            language: config.app.language.clone(),
            llm_request_retry_count: config.app.llm_request_retry_count,
        },
        agent_definitions: config.agent_definitions.clone(),
        prompts: config.prompts.clone(),
        models: config.models.clone(),
        hooks: config.hooks.clone(),
        mcp: sidecar_mcp_config(&config.mcp),
        memory: config.memory.clone(),
        spec: config.spec.clone(),
        plan: config.plan.clone(),
        global_skills,
        disabled_skill_keys: config.skills.disabled.clone(),
        disabled_skill_location_ids: config.skills.disabled_locations.clone(),
        required_disabled_skill_keys,
        selected_skills: Vec::new(),
    };
    let payload_json = serde_json::to_vec(&payload).map_err(|source| {
        ApiError::internal(format!("sidecar config serialization failed: {source}"))
    })?;
    let hash = format!("sha256:{:x}", Sha256::digest(&payload_json));

    Ok(SidecarRuntimeConfigBundle {
        config_generation,
        hash,
        payload,
    })
}

fn sidecar_mcp_config(config: &McpConfig) -> McpConfig {
    let mut config = config.clone();
    config.servers.retain(|server| {
        server.enabled
            && server.to_definition().is_ok_and(|definition| {
                definition.effective_execution_host() == foco_mcp::McpExecutionHost::Workspace
            })
    });
    config
}

fn global_skill_content(
    user_profile_dir: &Path,
    config: &GlobalConfig,
) -> Result<(Vec<SidecarRuntimeSkillContent>, Vec<String>), ApiError> {
    let disabled_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(user_profile_dir, config);
    let required_disabled_ids = discovery
        .required_disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let required_disabled_skill_keys = discovery
        .required_disabled
        .iter()
        .filter(|key| key.starts_with("global:"))
        .cloned()
        .collect::<Vec<_>>();

    let global_skills = discovery
        .skills
        .iter()
        .filter(|skill| {
            skill.scope == SKILL_SCOPE_GLOBAL
                && !skill_is_disabled(skill, &disabled_ids)
                && !skill_is_required_disabled(skill, &required_disabled_ids)
        })
        .map(|skill| {
            let parsed = parse_skill_file(&skill.path).map_err(ApiError::bad_request)?;
            if parsed.id != skill.id {
                return Err(ApiError::bad_request(format!(
                    "global skill '{}' file now declares skill id '{}'",
                    skill.key, parsed.id
                )));
            }

            Ok(SidecarRuntimeSkillContent {
                key: skill.key.clone(),
                id: skill.id.clone(),
                name: parsed.name,
                description: skill.description.clone(),
                path: skill.path.display().to_string(),
                scope: skill.scope.clone(),
                content_markdown: parsed.markdown,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    Ok((global_skills, required_disabled_skill_keys))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use foco_store::config::{McpExecutionHost, McpServerConfig, ProviderSettings};
    use serde_json::Value;

    fn write_skill(root: &Path, id: &str, description: &str, instructions: &str) {
        let skill_dir = root.join(id);
        fs::create_dir_all(&skill_dir).expect("skill directory");
        fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                "---\nname: {id}\ndescription: {description}\n---\n\n# {id}\n\n{instructions}\n"
            ),
        )
        .expect("skill file");
    }

    #[test]
    fn sidecar_runtime_bundle_omits_provider_api_keys_and_has_stable_hash() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace = tempfile::tempdir().expect("workspace");
        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config.providers.push(ProviderSettings {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            kind: "openai".to_string(),
            enabled: true,
            base_url: None,
            api_key: Some("secret-key".to_string()),
            auto_sync_models: false,
            model_sync_filter_regex: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
            api_proxy: Default::default(),
        });

        let bundle =
            build_sidecar_runtime_config_bundle(profile.path(), &config, 7).expect("bundle");
        let json = serde_json::to_string(&bundle).expect("bundle json");

        assert_eq!(bundle.config_generation, 7);
        assert!(bundle.hash.starts_with("sha256:"));
        assert!(!json.contains("secret-key"));
        assert!(!json.contains("providers"));
    }

    #[test]
    fn sidecar_runtime_bundle_syncs_runtime_tool_state_compression_setting() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace = tempfile::tempdir().expect("workspace");
        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());

        let disabled = build_sidecar_runtime_config_bundle(profile.path(), &config, 1)
            .expect("disabled runtime bundle");
        assert!(!disabled.payload.app.runtime_tool_state_compression_enabled);

        config.app.runtime_tool_state_compression_enabled = true;
        let enabled = build_sidecar_runtime_config_bundle(profile.path(), &config, 2)
            .expect("enabled runtime bundle");
        let enabled_json = serde_json::to_value(&enabled).expect("enabled bundle json");

        assert!(enabled.payload.app.runtime_tool_state_compression_enabled);
        assert_eq!(
            enabled_json["payload"]["app"]["runtimeToolStateCompressionEnabled"],
            Value::Bool(true)
        );
        assert_ne!(disabled.hash, enabled.hash);
    }

    #[test]
    fn sidecar_runtime_bundle_syncs_only_workspace_hosted_mcp_servers() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace = tempfile::tempdir().expect("workspace");
        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config.mcp.servers = vec![
            McpServerConfig {
                id: "auto-stdio".to_string(),
                name: "Auto stdio".to_string(),
                enabled: true,
                transport: "stdio".to_string(),
                command: Some("workspace-secret-command".to_string()),
                args: vec!["workspace-arg".to_string()],
                url: None,
                execution_host: McpExecutionHost::Auto,
            },
            McpServerConfig {
                id: "auto-http".to_string(),
                name: "Auto HTTP".to_string(),
                enabled: true,
                transport: "streamable-http".to_string(),
                command: None,
                args: Vec::new(),
                url: Some("https://user:local-secret@example.test/mcp".to_string()),
                execution_host: McpExecutionHost::Auto,
            },
            McpServerConfig {
                id: "explicit-local".to_string(),
                name: "Explicit local".to_string(),
                enabled: true,
                transport: "stdio".to_string(),
                command: Some("local-secret-command".to_string()),
                args: Vec::new(),
                url: None,
                execution_host: McpExecutionHost::Local,
            },
            McpServerConfig {
                id: "explicit-workspace".to_string(),
                name: "Explicit workspace".to_string(),
                enabled: true,
                transport: "streamable-http".to_string(),
                command: None,
                args: Vec::new(),
                url: Some("https://remote.example.test/mcp".to_string()),
                execution_host: McpExecutionHost::Workspace,
            },
        ];

        let bundle = build_sidecar_runtime_config_bundle(profile.path(), &config, 1)
            .expect("runtime bundle");
        let ids = bundle
            .payload
            .mcp
            .servers
            .iter()
            .map(|server| server.id.as_str())
            .collect::<Vec<_>>();
        let serialized = serde_json::to_string(&bundle).expect("bundle json");

        assert_eq!(ids, vec!["auto-stdio", "explicit-workspace"]);
        assert!(!serialized.contains("local-secret"));
        assert!(!serialized.contains("user:local-secret"));
    }

    #[test]
    fn sidecar_runtime_bundle_syncs_app_language() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace = tempfile::tempdir().expect("workspace");
        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config.app.language = "zh-CN".to_string();

        let bundle = build_sidecar_runtime_config_bundle(profile.path(), &config, 1)
            .expect("runtime bundle");
        let json = serde_json::to_value(&bundle).expect("bundle json");

        assert_eq!(bundle.payload.app.language, "zh-CN");
        assert_eq!(
            json["payload"]["app"]["language"],
            Value::String("zh-CN".into())
        );
    }

    #[test]
    fn sidecar_runtime_bundle_syncs_only_enabled_global_skills_and_disabled_filters() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace = tempfile::tempdir().expect("workspace");
        let global_root = profile.path().join(".agents").join("skills");
        let workspace_root = workspace.path().join(".agents").join("skills");
        write_skill(
            &global_root,
            "enabled",
            "Enabled skill.",
            "Enabled instructions.",
        );
        write_skill(
            &global_root,
            "disabled",
            "Disabled skill.",
            "Disabled instructions.",
        );
        write_skill(
            &workspace_root,
            "workspace-only",
            "Workspace skill.",
            "Workspace instructions.",
        );
        let broken_dir = global_root.join("broken");
        fs::create_dir_all(&broken_dir).expect("broken skill directory");
        fs::write(
            broken_dir.join("SKILL.md"),
            "---\nname: broken\ndescription:\n---\n\n# Broken\n",
        )
        .expect("broken skill file");

        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config.skills.disabled = vec!["global:disabled".to_string(), "legacy-id".to_string()];
        config.skills.disabled_locations =
            vec![format!("workspace:{}:claude", config.workspaces[0].id)];

        let bundle = build_sidecar_runtime_config_bundle(profile.path(), &config, 1)
            .expect("runtime bundle");

        assert_eq!(
            bundle.payload.global_skills,
            vec![SidecarRuntimeSkillContent {
                key: "global:enabled".to_string(),
                id: "enabled".to_string(),
                name: "enabled".to_string(),
                description: "Enabled skill.".to_string(),
                path: global_root
                    .join("enabled")
                    .join("SKILL.md")
                    .display()
                    .to_string(),
                scope: SKILL_SCOPE_GLOBAL.to_string(),
                content_markdown: "---\nname: enabled\ndescription: Enabled skill.\n---\n\n# enabled\n\nEnabled instructions.\n".to_string(),
            }]
        );
        assert_eq!(bundle.payload.disabled_skill_keys, config.skills.disabled);
        assert_eq!(
            bundle.payload.disabled_skill_location_ids,
            config.skills.disabled_locations
        );
        assert_eq!(
            bundle.payload.required_disabled_skill_keys,
            vec!["global:broken".to_string()]
        );
        assert!(bundle.payload.selected_skills.is_empty());
    }

    #[test]
    fn sidecar_runtime_bundle_excludes_skills_in_disabled_global_location() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace = tempfile::tempdir().expect("workspace");
        write_skill(
            &profile.path().join(".agents").join("skills"),
            "hidden",
            "Hidden skill.",
            "Hidden instructions.",
        );
        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config.skills.disabled_locations = vec!["global:agents".to_string()];

        let bundle = build_sidecar_runtime_config_bundle(profile.path(), &config, 1)
            .expect("runtime bundle");

        assert!(bundle.payload.global_skills.is_empty());
        assert_eq!(
            bundle.payload.disabled_skill_location_ids,
            vec!["global:agents".to_string()]
        );
    }

    #[test]
    fn sidecar_runtime_bundle_hash_changes_when_global_skill_content_changes() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace = tempfile::tempdir().expect("workspace");
        let global_root = profile.path().join(".agents").join("skills");
        write_skill(
            &global_root,
            "changing",
            "Changing skill.",
            "First instructions.",
        );
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let before = build_sidecar_runtime_config_bundle(profile.path(), &config, 1)
            .expect("bundle before change");
        write_skill(
            &global_root,
            "changing",
            "Changing skill.",
            "Second instructions.",
        );

        let after = build_sidecar_runtime_config_bundle(profile.path(), &config, 2)
            .expect("bundle after change");

        assert_ne!(before.hash, after.hash);
    }

    #[test]
    fn sidecar_runtime_bundle_syncs_global_hooks_and_hashes_changes() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace = tempfile::tempdir().expect("workspace");
        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config.hooks = serde_json::from_value(serde_json::json!({
            "PreCompact": [{
                "matcher": "llm",
                "hooks": [{
                    "type": "prompt",
                    "prompt": "Return an allow decision."
                }]
            }]
        }))
        .expect("hook config");

        let before = build_sidecar_runtime_config_bundle(profile.path(), &config, 1)
            .expect("runtime bundle");
        assert_eq!(before.payload.hooks, config.hooks);
        let before_json = serde_json::to_string(&before).expect("bundle json");
        assert!(before_json.contains("Return an allow decision."));

        config
            .hooks
            .hooks
            .get_mut("PreCompact")
            .expect("PreCompact groups")[0]
            .hooks[0]
            .prompt = Some("Return a block decision.".to_string());
        let after = build_sidecar_runtime_config_bundle(profile.path(), &config, 2)
            .expect("updated runtime bundle");

        assert_ne!(before.hash, after.hash);
    }

    #[test]
    fn sidecar_runtime_bundle_deserializes_legacy_skill_payload_defaults() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace = tempfile::tempdir().expect("workspace");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let bundle = build_sidecar_runtime_config_bundle(profile.path(), &config, 1)
            .expect("runtime bundle");
        let mut value = serde_json::to_value(bundle).expect("bundle value");
        let payload = value
            .get_mut("payload")
            .and_then(Value::as_object_mut)
            .expect("payload object");
        payload.remove("app");
        payload.remove("hooks");
        payload.remove("globalSkills");
        payload.remove("disabledSkillKeys");
        payload.remove("disabledSkillLocationIds");
        payload.remove("requiredDisabledSkillKeys");
        payload.insert(
            "selectedSkills".to_string(),
            serde_json::json!([{
                "key": "global:legacy",
                "id": "legacy",
                "name": "Legacy",
                "path": "/legacy/SKILL.md",
                "contentMarkdown": "# Legacy"
            }]),
        );

        let parsed = serde_json::from_value::<SidecarRuntimeConfigBundle>(value)
            .expect("legacy bundle parse");

        assert!(!parsed.payload.app.runtime_tool_state_compression_enabled);
        assert_eq!(
            parsed.payload.app.llm_request_retry_count,
            foco_store::config::DEFAULT_LLM_REQUEST_RETRY_COUNT
        );
        assert_eq!(parsed.payload.hooks, HookConfig::default());
        assert!(parsed.payload.global_skills.is_empty());
        assert!(parsed.payload.disabled_skill_keys.is_empty());
        assert!(parsed.payload.disabled_skill_location_ids.is_empty());
        assert!(parsed.payload.required_disabled_skill_keys.is_empty());
        assert_eq!(parsed.payload.selected_skills[0].description, "");
        assert_eq!(parsed.payload.selected_skills[0].scope, SKILL_SCOPE_GLOBAL);
    }
}
