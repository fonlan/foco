use std::collections::HashSet;
use std::path::Path;

use foco_store::config::{
    AgentDefinitionSettings, GlobalConfig, HookConfig, McpConfig, MemorySettings, ModelSettings,
    PlanSettings, PromptSettings, SpecSettings,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ApiError;
use crate::skills::{
    discover_skills, parse_skill_file, skill_is_disabled, skill_is_required_disabled,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidecarRuntimeConfigBundle {
    pub(crate) config_generation: u64,
    pub(crate) hash: String,
    pub(crate) payload: SidecarRuntimeConfigPayload,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidecarRuntimeConfigPayload {
    pub(crate) agent_definitions: Vec<AgentDefinitionSettings>,
    pub(crate) prompts: PromptSettings,
    pub(crate) models: Vec<ModelSettings>,
    pub(crate) hooks: HookConfig,
    pub(crate) mcp: McpConfig,
    pub(crate) memory: MemorySettings,
    pub(crate) spec: SpecSettings,
    pub(crate) plan: PlanSettings,
    pub(crate) selected_skills: Vec<SidecarRuntimeSkillContent>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidecarRuntimeSkillContent {
    pub(crate) key: String,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) content_markdown: String,
}

pub(crate) fn build_sidecar_runtime_config_bundle(
    user_profile_dir: &Path,
    config: &GlobalConfig,
    workspace_id: &str,
    requested_skill_keys: Option<&[String]>,
    config_generation: u64,
) -> Result<SidecarRuntimeConfigBundle, ApiError> {
    let payload = SidecarRuntimeConfigPayload {
        agent_definitions: config.agent_definitions.clone(),
        prompts: config.prompts.clone(),
        models: config.models.clone(),
        hooks: config.hooks.clone(),
        mcp: config.mcp.clone(),
        memory: config.memory.clone(),
        spec: config.spec.clone(),
        plan: config.plan.clone(),
        selected_skills: selected_skill_content(
            user_profile_dir,
            config,
            workspace_id,
            requested_skill_keys,
        )?,
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

fn selected_skill_content(
    user_profile_dir: &Path,
    config: &GlobalConfig,
    workspace_id: &str,
    requested_skill_keys: Option<&[String]>,
) -> Result<Vec<SidecarRuntimeSkillContent>, ApiError> {
    let disabled_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(user_profile_dir, &config.workspaces);
    let required_disabled_ids = discovery
        .required_disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let requested = requested_skill_keys
        .filter(|keys| !keys.is_empty())
        .map(|keys| keys.iter().map(String::as_str).collect::<HashSet<_>>());
    let Some(requested) = requested else {
        return Ok(Vec::new());
    };

    let mut selected = Vec::new();
    for skill in discovery.skills.iter().filter(|skill| {
        skill_applies_to_workspace(skill, workspace_id)
            && !skill_is_disabled(skill, &disabled_ids)
            && !skill_is_required_disabled(skill, &required_disabled_ids)
            && (requested.contains(skill.key.as_str()) || requested.contains(skill.id.as_str()))
    }) {
        let parsed = parse_skill_file(&skill.path).map_err(ApiError::bad_request)?;
        if parsed.id != skill.id {
            return Err(ApiError::bad_request(format!(
                "selected skill '{}' file now declares skill id '{}'",
                skill.key, parsed.id
            )));
        }
        selected.push(SidecarRuntimeSkillContent {
            key: skill.key.clone(),
            id: skill.id.clone(),
            name: parsed.name,
            path: skill.path.display().to_string(),
            content_markdown: parsed.markdown,
        });
    }

    Ok(selected)
}

fn skill_applies_to_workspace(
    skill: &foco_store::config::SkillSettings,
    workspace_id: &str,
) -> bool {
    skill.scope == foco_store::config::SKILL_SCOPE_GLOBAL
        || (skill.scope == foco_store::config::SKILL_SCOPE_WORKSPACE
            && skill.workspace_id.as_deref() == Some(workspace_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use foco_store::config::{DEFAULT_WORKSPACE_ID, ProviderSettings};

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
            api_proxy: Default::default(),
        });

        let bundle = build_sidecar_runtime_config_bundle(
            profile.path(),
            &config,
            DEFAULT_WORKSPACE_ID,
            None,
            7,
        )
        .expect("bundle");
        let json = serde_json::to_string(&bundle).expect("bundle json");

        assert_eq!(bundle.config_generation, 7);
        assert!(bundle.hash.starts_with("sha256:"));
        assert!(!json.contains("secret-key"));
        assert!(!json.contains("providers"));
    }
}
