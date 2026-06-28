use std::collections::BTreeMap;

#[cfg(all(windows, not(debug_assertions)))]
use crate::platform::tray_windows::tray_menu_labels;
use crate::runtime::spawn_api_audit_cleanup_once;
use axum::{
    Json,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use fancy_regex::Regex;
use foco_agent::build_default_system_prompt;
use foco_providers::{
    ProviderConfigError, fetch_provider_model_ids, normalized_base_url, parse_provider_kind,
    test_provider_connection,
};
use foco_store::{
    config::{
        IMAGE_GENERATION_SYSTEM_PROMPT_NAME, PlanSettings, PromptSettings,
        REVIEW_SYSTEM_PROMPT_NAME, SpecSettings,
    },
    model_metadata::{
        MODELS_DEV_API_URL, parse_models_dev_metadata, read_model_metadata_cache,
        write_model_metadata_cache,
    },
};

use crate::*;

const DEFAULT_AGENT_DEFINITION_ID: &str = "agent-definition-default";
pub(crate) const REVIEW_AGENT_DEFINITION_ID: &str = "agent-definition-review";
pub(crate) const IMAGE_AGENT_DEFINITION_ID: &str = "agent-definition-image-gen";
pub(crate) const IMAGE_AGENT_SYSTEM_PROMPT_NAME: &str = IMAGE_GENERATION_SYSTEM_PROMPT_NAME;
const DEFAULT_AGENT_SYSTEM_PROMPT: &str = "<agent_definition_prompt>\n<identity>You are Foco's default coding agent.</identity>\n<instructions>Complete simple tasks directly. For complex tasks, consider creating and coordinating multiple worker agents when they can help with parallel investigation, implementation, review, or verification. After completing non-trivial implementation work, when agent team tools are available, create a review-focused worker agent when practical to independently inspect the diff, run or recommend validation, and surface issues before finalizing.</instructions>\n</agent_definition_prompt>";
const REVIEW_AGENT_SYSTEM_PROMPT: &str = r#"<agent_definition_prompt>
<identity>You are Foco's built-in code review agent.</identity>
<instructions>Review code changes with a bug-finding mindset. Prioritize correctness, regressions, security, data loss risks, and missing tests. Present findings first, ordered by severity with concrete file and line references. If no issues are found, say so clearly and mention residual test gaps or risks.</instructions>
<boundaries>Do not edit files or broaden into implementation work unless the user explicitly asks. Keep summaries brief and secondary to findings.</boundaries>
</agent_definition_prompt>"#;
const IMAGE_AGENT_SYSTEM_PROMPT: &str = "<agent_definition_prompt>\n<identity>You are Foco's image generation agent.</identity>\n<instructions>Turn the user's request into a precise image prompt, call image_gen, and return the generated file paths with concise notes. Do not modify source files unless explicitly asked.</instructions>\n<tool_defaults>Use image_gen with model &quot;gpt-image-2&quot; unless the user explicitly asks for another configured image model.</tool_defaults>\n</agent_definition_prompt>";
const PLAN_MODE_SYSTEM_PROMPT: &str = r#"<agent_definition_prompt>
<identity>You are Foco Plan Mode, a planning partner for software work.</identity>
<instructions>Help the user refine requirements before implementation. Work from the current repository context and available read-only tools. Plan Mode is for planning only, not implementation.</instructions>
<workflow>
1. Understand the current project context first: relevant files, docs, tests, recent behavior, and constraints.
2. If the request is underspecified, ask one focused clarifying question at a time. If the next step is clear, state the assumptions and continue.
3. For non-trivial changes, present 2-3 viable approaches with trade-offs and a recommendation.
4. Turn the chosen approach into a concrete plan with scope, affected components, data flow, risks, and the smallest useful validation.
5. Keep plans narrow. Split oversized work into phases and identify what should not be built yet.
</workflow>
<plan_creation>When the plan is settled, or when the user accepts your recommended approach, call create_plan to create the workspace implementation plan. Use plan tools to create or update explicit workspace plans so later implementation work can proceed from the agreed scope.</plan_creation>
<boundaries>Do not edit files, run mutating commands, install dependencies, or claim to complete implementation work. Do not use planning as a reason to broaden scope beyond what the user asked for.</boundaries>
</agent_definition_prompt>"#;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualGeneralSettingsRequest {
    pub(crate) auto_start_enabled: Option<bool>,
    pub(crate) default_team_mode_enabled: Option<bool>,
    pub(crate) api_audit: Option<ManualApiAuditSettingsRequest>,
    pub(crate) listen_host: String,
    pub(crate) listen_port: u32,
    pub(crate) llm_request_retry_count: Option<u32>,
    pub(crate) language: String,
    pub(crate) theme: String,
    pub(crate) hook_audit_enabled: Option<bool>,
    pub(crate) password: Option<String>,
    pub(crate) clear_password: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualApiAuditSettingsRequest {
    pub(crate) request_detail_retention_days: u32,
    pub(crate) save_request_response_details: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualWebSearchSettingsRequest {
    pub(crate) enabled: bool,
    pub(crate) active_provider: String,
    pub(crate) api_proxy: Option<ManualApiProxySettingsRequest>,
    pub(crate) tavily_api_key: Option<String>,
    pub(crate) brave_api_key: Option<String>,
    pub(crate) clear_tavily_api_key: Option<bool>,
    pub(crate) clear_brave_api_key: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualMemorySettingsRequest {
    pub(crate) enabled: bool,
    pub(crate) extraction_mode: String,
    pub(crate) retrieval_mode: String,
    pub(crate) retention_days: Option<u32>,
    pub(crate) extraction_model_id: Option<String>,
    pub(crate) retrieval_model_id: Option<String>,
    pub(crate) dream: Option<ManualMemoryDreamSettingsRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualMemoryDreamSettingsRequest {
    pub(crate) enabled: bool,
    pub(crate) auto_enabled: bool,
    pub(crate) mode: String,
    pub(crate) model_id: Option<String>,
    pub(crate) workspace_interval_days: u32,
    pub(crate) global_interval_days: u32,
    pub(crate) create_transcript_chat: bool,
    pub(crate) max_facts_per_run: u32,
    pub(crate) max_changes_per_run: u32,
    pub(crate) scheduler_scan_minutes: u32,
    pub(crate) workspace_threshold_facts: Option<u32>,
    pub(crate) global_threshold_facts: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualSpecSettingsRequest {
    pub(crate) auto_enabled: bool,
    pub(crate) generation_model_id: Option<String>,
    pub(crate) generation_system_prompt: Option<String>,
    pub(crate) update_system_prompt: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualPlanSettingsRequest {
    pub(crate) merge_automation_mode: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualPromptSettingsRequest {
    pub(crate) system_prompts: Option<Vec<ManualSystemPromptRequest>>,
    pub(crate) system_prompt: Option<String>,
    pub(crate) files: Vec<String>,
    pub(crate) extra_text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualSystemPromptRequest {
    pub(crate) name: String,
    pub(crate) content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualApiProxySettingsRequest {
    pub(crate) enabled: bool,
    pub(crate) proxy_type: String,
    pub(crate) url: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentDefinitionInput {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) model_options: AgentModelOptions,
    pub(crate) system_prompt: String,
    pub(crate) allowed_tools: Vec<String>,
    pub(crate) max_instances: u32,
    #[serde(default = "default_agent_execution_workspace_modes")]
    pub(crate) allowed_execution_workspace_modes: Vec<AgentExecutionWorkspaceMode>,
    pub(crate) permissions: AgentPermissions,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreateAgentDefinitionRequest {
    pub(crate) definition: AgentDefinitionInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UpdateAgentDefinitionRequest {
    pub(crate) id: AgentDefinitionId,
    pub(crate) definition: AgentDefinitionInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DeleteAgentDefinitionRequest {
    pub(crate) id: AgentDefinitionId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualModelRequest {
    pub(crate) model_id: String,
    pub(crate) display_name: String,
    pub(crate) enabled: bool,
    pub(crate) metadata_key: Option<String>,
    pub(crate) context_window: Option<u64>,
    pub(crate) max_output_tokens: Option<u64>,
    pub(crate) provider_ids: Option<Vec<String>>,
    pub(crate) active_provider_id: Option<String>,
    pub(crate) input_modalities: Option<Vec<String>>,
    pub(crate) output_modalities: Option<Vec<String>>,
    pub(crate) thinking_level: Option<String>,
    pub(crate) clear_thinking_level: Option<bool>,
    pub(crate) system_prompt_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelOrderRequest {
    pub(crate) model_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualProviderRequest {
    pub(crate) api_proxy: Option<ManualApiProxySettingsRequest>,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) enabled: bool,
    pub(crate) base_url: Option<String>,
    pub(crate) api_key: Option<String>,
    pub(crate) clear_api_key: Option<bool>,
    #[serde(default)]
    pub(crate) auto_sync_models: bool,
    pub(crate) model_sync_filter_regex: Option<String>,
    pub(crate) request_overrides: Vec<ProviderRequestOverride>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualMcpServerRequest {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) transport: String,
    pub(crate) command: Option<String>,
    pub(crate) args: Option<Vec<String>>,
    pub(crate) url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualSkillsRequest {
    pub(crate) disabled: Option<Vec<String>>,
    pub(crate) enabled: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestProviderRequest {
    pub(crate) provider_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteSettingsItemRequest {
    pub(crate) id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsResponse {
    pub(crate) general: GeneralSettingsSummary,
    pub(crate) agent_tools: Vec<String>,
    pub(crate) native_tools: NativeToolsSummary,
    pub(crate) web_search: WebSearchSettingsSummary,
    pub(crate) memory: MemorySettingsSummary,
    pub(crate) spec: SpecSettingsSummary,
    pub(crate) plan: PlanSettingsSummary,
    pub(crate) prompts: PromptSettingsSummary,
    pub(crate) workspaces: Vec<ConfiguredWorkspaceSummary>,
    pub(crate) terminal_shells: Vec<TerminalShellSummary>,
    pub(crate) provider_kinds: Vec<ProviderKindSummary>,
    pub(crate) thinking_levels: Vec<ThinkingLevelSummary>,
    pub(crate) providers: Vec<ConfiguredProviderSummary>,
    pub(crate) configured_models: Vec<ConfiguredModelSummary>,
    pub(crate) mcp_transports: Vec<McpTransportSummary>,
    pub(crate) mcp_servers: Vec<ConfiguredMcpServerSummary>,
    pub(crate) skills: SkillsSettingsSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentDefinitionsResponse {
    pub(crate) agent_definitions: Vec<AgentDefinitionSettings>,
    pub(crate) default_role_prompts: BTreeMap<AgentDefinitionId, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeToolsSummary {
    pub(crate) browser_probe_port: u16,
    pub(crate) ripgrep: RipgrepToolSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeneralSettingsSummary {
    pub(crate) auto_start_enabled: bool,
    pub(crate) default_team_mode_enabled: bool,
    pub(crate) api_audit: ApiAuditSettingsSummary,
    pub(crate) web_server: WebServerSettingsSummary,
    pub(crate) llm_request_retry_count: u32,
    pub(crate) max_llm_request_retry_count: u32,
    pub(crate) language: String,
    pub(crate) theme: String,
    pub(crate) hook_audit_enabled: bool,
    pub(crate) supported_languages: Vec<AppLanguageSummary>,
    pub(crate) supported_themes: Vec<AppThemeSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApiAuditSettingsSummary {
    pub(crate) request_detail_retention_days: u32,
    pub(crate) save_request_response_details: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebSearchSettingsSummary {
    pub(crate) enabled: bool,
    pub(crate) active_provider: String,
    pub(crate) providers: Vec<WebSearchProviderSummary>,
    pub(crate) api_proxy: ApiProxySettingsSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebSearchProviderSummary {
    pub(crate) provider: &'static str,
    pub(crate) label: &'static str,
    pub(crate) has_api_key: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemorySettingsSummary {
    pub(crate) enabled: bool,
    pub(crate) extraction_mode: String,
    pub(crate) retrieval_mode: String,
    pub(crate) retention_days: Option<u32>,
    pub(crate) extraction_model_id: Option<String>,
    pub(crate) retrieval_model_id: Option<String>,
    pub(crate) dream: MemoryDreamSettingsSummary,
    pub(crate) extraction_modes: Vec<MemoryExtractionModeSummary>,
    pub(crate) retrieval_modes: Vec<MemoryExtractionModeSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryDreamSettingsSummary {
    pub(crate) enabled: bool,
    pub(crate) auto_enabled: bool,
    pub(crate) mode: String,
    pub(crate) model_id: Option<String>,
    pub(crate) workspace_interval_days: u32,
    pub(crate) global_interval_days: u32,
    pub(crate) create_transcript_chat: bool,
    pub(crate) max_facts_per_run: u32,
    pub(crate) max_changes_per_run: u32,
    pub(crate) scheduler_scan_minutes: u32,
    pub(crate) workspace_threshold_facts: u32,
    pub(crate) global_threshold_facts: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryExtractionModeSummary {
    pub(crate) value: &'static str,
    pub(crate) label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpecSettingsSummary {
    pub(crate) auto_enabled: bool,
    pub(crate) generation_model_id: Option<String>,
    pub(crate) generation_system_prompt: Option<String>,
    pub(crate) update_system_prompt: Option<String>,
    pub(crate) default_generation_system_prompt: String,
    pub(crate) default_update_system_prompt: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanSettingsSummary {
    pub(crate) merge_automation_mode: String,
    pub(crate) merge_automation_modes: Vec<PlanMergeAutomationModeSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanMergeAutomationModeSummary {
    pub(crate) value: &'static str,
    pub(crate) label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptSettingsSummary {
    pub(crate) system_prompt: Option<String>,
    pub(crate) default_system_prompt: String,
    pub(crate) default_image_generation_system_prompt: Option<String>,
    pub(crate) default_plan_mode_system_prompt: String,
    pub(crate) default_review_system_prompt: String,
    pub(crate) system_prompts: Vec<SystemPromptSummary>,
    pub(crate) files: Vec<String>,
    pub(crate) extra_text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SystemPromptSummary {
    pub(crate) name: String,
    pub(crate) content: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApiProxySettingsSummary {
    pub(crate) enabled: bool,
    pub(crate) proxy_type: String,
    pub(crate) url: String,
    pub(crate) supported_types: Vec<ApiProxyTypeSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApiProxyTypeSummary {
    pub(crate) proxy_type: &'static str,
    pub(crate) label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebServerSettingsSummary {
    pub(crate) listen_host: String,
    pub(crate) listen_port: u16,
    pub(crate) password_enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppLanguageSummary {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppThemeSummary {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfiguredWorkspaceSummary {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) pinned: bool,
    pub(crate) terminal_shell: String,
    pub(crate) common_commands: Vec<WorkspaceCommonCommandSummary>,
    pub(crate) is_default: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceCommonCommandSummary {
    pub(crate) name: String,
    pub(crate) command: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalShellSummary {
    pub(crate) shell: &'static str,
    pub(crate) label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderKindSummary {
    pub(crate) kind: &'static str,
    pub(crate) label: &'static str,
    pub(crate) default_base_url: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThinkingLevelSummary {
    pub(crate) value: &'static str,
    pub(crate) label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpTransportSummary {
    pub(crate) transport: &'static str,
    pub(crate) label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfiguredProviderSummary {
    pub(crate) api_proxy: ApiProxySettingsSummary,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) kind_label: &'static str,
    pub(crate) enabled: bool,
    pub(crate) base_url: Option<String>,
    pub(crate) has_api_key: bool,
    pub(crate) auto_sync_models: bool,
    pub(crate) model_sync_filter_regex: Option<String>,
    pub(crate) request_overrides: Vec<ProviderRequestOverride>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfiguredMcpServerSummary {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) transport: String,
    pub(crate) transport_label: &'static str,
    pub(crate) command: Option<String>,
    pub(crate) args: Vec<String>,
    pub(crate) url: Option<String>,
    pub(crate) state: String,
    pub(crate) error: Option<String>,
    pub(crate) tool_count: usize,
    pub(crate) warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillsSettingsSummary {
    pub(crate) directories: Vec<String>,
    pub(crate) detected: Vec<ConfiguredSkillSummary>,
    pub(crate) errors: Vec<SkillDiscoveryErrorSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfiguredSkillSummary {
    pub(crate) key: String,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) path: String,
    pub(crate) scope: String,
    pub(crate) workspace_id: Option<String>,
    pub(crate) workspace_name: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) can_enable: bool,
    pub(crate) warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderTestResponse {
    pub(crate) ok: bool,
    pub(crate) message: String,
    pub(crate) model_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderModelsResponse {
    pub(crate) provider_id: String,
    pub(crate) models: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderModelsRefreshResponse {
    pub(crate) settings: SettingsResponse,
    pub(crate) providers: Vec<ProviderModelsResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelMetadataResponse {
    pub(crate) source_url: Option<String>,
    pub(crate) fetched_at: Option<String>,
    pub(crate) cache_path: String,
    pub(crate) models: Vec<ModelMetadataRecord>,
    pub(crate) configured_models: Vec<ConfiguredModelSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfiguredModelSummary {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) enabled: bool,
    pub(crate) metadata_key: Option<String>,
    pub(crate) metadata_source_url: Option<String>,
    pub(crate) metadata_refreshed_at: Option<String>,
    pub(crate) context_window: Option<u64>,
    pub(crate) max_output_tokens: Option<u64>,
    pub(crate) can_enable: bool,
    pub(crate) missing_limits: Vec<&'static str>,
    pub(crate) provider_ids: Vec<String>,
    pub(crate) active_provider_id: Option<String>,
    pub(crate) input_modalities: Vec<String>,
    pub(crate) output_modalities: Vec<String>,
    pub(crate) thinking_level: Option<String>,
    pub(crate) system_prompt_name: String,
    pub(crate) supports_thinking: bool,
    pub(crate) warnings: Vec<String>,
}

pub(crate) fn default_image_generation_system_prompt() -> String {
    IMAGE_AGENT_SYSTEM_PROMPT.to_string()
}

pub(crate) fn default_plan_mode_system_prompt() -> String {
    PLAN_MODE_SYSTEM_PROMPT.to_string()
}

pub(crate) fn default_review_system_prompt() -> String {
    REVIEW_AGENT_SYSTEM_PROMPT.to_string()
}

pub(crate) async fn settings(
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;

    settings_response(&state, &config).await
}

pub(crate) async fn agent_definitions(
    State(state): State<AppState>,
) -> Result<Json<AgentDefinitionsResponse>, ApiError> {
    let config = ensure_default_agent_definition(&state).await?;

    Ok(agent_definitions_response(&config))
}

async fn ensure_default_agent_definition(state: &AppState) -> Result<GlobalConfig, ApiError> {
    let mut config = config_snapshot(state)?;
    let mut changed = false;
    let default_id = default_agent_definition_id()?;

    if !config
        .agent_definitions
        .iter()
        .any(|definition| definition.id == default_id)
    {
        if let Some(definition) = default_agent_definition_for_config(&config, default_id.clone()) {
            config.agent_definitions.insert(0, definition);
            changed = true;
        }
    }

    if refresh_builtin_agent_definitions(&mut config)? {
        changed = true;
    }

    if changed {
        validate_agent_definition_update(state, &config).await?;
        save_config(state, config.clone())?;
    }
    Ok(config)
}

fn default_agent_definition_id() -> Result<AgentDefinitionId, ApiError> {
    AgentDefinitionId::new(DEFAULT_AGENT_DEFINITION_ID)
        .map_err(|error| ApiError::internal(error.message().to_string()))
}

fn review_agent_definition_id() -> Result<AgentDefinitionId, ApiError> {
    AgentDefinitionId::new(REVIEW_AGENT_DEFINITION_ID)
        .map_err(|error| ApiError::internal(error.message().to_string()))
}

fn image_agent_definition_id() -> Result<AgentDefinitionId, ApiError> {
    AgentDefinitionId::new(IMAGE_AGENT_DEFINITION_ID)
        .map_err(|error| ApiError::internal(error.message().to_string()))
}

pub(crate) fn default_image_agent_system_prompt_for_config(
    config: &GlobalConfig,
) -> Result<Option<String>, ApiError> {
    let image_id = image_agent_definition_id()?;
    Ok(image_agent_definition_for_config(config, image_id)
        .map(|definition| definition.system_prompt))
}

fn ensure_review_agent_definition(config: &mut GlobalConfig) -> Result<bool, ApiError> {
    let review_id = review_agent_definition_id()?;
    let Some(mut definition) = review_agent_definition_for_config(config, review_id.clone())?
    else {
        return Ok(false);
    };

    if let Some(stored_index) = config
        .agent_definitions
        .iter()
        .position(|definition| definition.id == review_id)
    {
        let stored = &config.agent_definitions[stored_index];
        definition.provider_id = stored.provider_id.clone();
        definition.model_id = stored.model_id.clone();
        definition.model_options = stored.model_options.clone();
        definition.system_prompt = stored.system_prompt.clone();
        definition.allowed_tools = stored.allowed_tools.clone();
        definition.allowed_execution_workspace_modes =
            stored.allowed_execution_workspace_modes.clone();
        definition.permissions = stored.permissions.clone();
        definition.revision = stored.revision;
        let stored = &mut config.agent_definitions[stored_index];
        if stored != &definition {
            *stored = definition;
            Ok(true)
        } else {
            Ok(false)
        }
    } else {
        config.agent_definitions.push(definition);
        Ok(true)
    }
}

fn refresh_review_agent_system_prompt(config: &mut GlobalConfig) -> Result<bool, ApiError> {
    let review_id = review_agent_definition_id()?;
    let system_prompt =
        crate::prompt::active_system_prompt(&config.prompts, REVIEW_SYSTEM_PROMPT_NAME)?;
    let Some(definition) = config
        .agent_definitions
        .iter_mut()
        .find(|definition| definition.id == review_id)
    else {
        return Ok(false);
    };

    if definition.system_prompt == system_prompt {
        return Ok(false);
    }
    definition.system_prompt = system_prompt;
    Ok(true)
}

fn ensure_image_agent_definition(config: &mut GlobalConfig) -> Result<bool, ApiError> {
    let image_id = image_agent_definition_id()?;
    let image_definition = image_agent_definition_for_config(config, image_id.clone());

    match image_definition {
        Some(mut definition) => {
            if let Some(stored_index) = config
                .agent_definitions
                .iter()
                .position(|definition| definition.id == image_id)
            {
                let stored = &config.agent_definitions[stored_index];
                let preserve_runner = image_agent_runner_selection_valid(config, stored);
                let stored_provider_id = stored.provider_id.clone();
                let stored_model_id = stored.model_id.clone();
                let stored_model_options = stored.model_options.clone();
                let stored_revision = stored.revision;
                let stored_system_prompt = stored.system_prompt.clone();

                let stored = &mut config.agent_definitions[stored_index];
                if !stored.system_prompt.trim().is_empty() {
                    definition.system_prompt = stored_system_prompt;
                }
                if preserve_runner {
                    definition.provider_id = stored_provider_id;
                    definition.model_id = stored_model_id;
                    definition.model_options = stored_model_options;
                    definition.revision = stored_revision;
                }
                if stored != &definition {
                    *stored = definition;
                    Ok(true)
                } else {
                    Ok(false)
                }
            } else {
                let default_id = default_agent_definition_id()?;
                let insert_index = config
                    .agent_definitions
                    .iter()
                    .position(|definition| definition.id != default_id)
                    .unwrap_or(config.agent_definitions.len());
                config.agent_definitions.insert(insert_index, definition);
                Ok(true)
            }
        }
        None => {
            let definition_count = config.agent_definitions.len();
            config
                .agent_definitions
                .retain(|definition| definition.id != image_id);
            Ok(config.agent_definitions.len() != definition_count)
        }
    }
}

fn image_agent_runner_selection_valid(
    config: &GlobalConfig,
    definition: &AgentDefinitionSettings,
) -> bool {
    let Some(model) = config
        .models
        .iter()
        .find(|model| model.id == definition.model_id)
    else {
        return false;
    };
    model.enabled
        && model.limits.is_some()
        && model_outputs_text(model)
        && model
            .provider_ids
            .iter()
            .any(|provider_id| provider_id == &definition.provider_id)
        && config
            .providers
            .iter()
            .any(|provider| provider.enabled && provider.id == definition.provider_id)
}

fn refresh_builtin_agent_definitions(config: &mut GlobalConfig) -> Result<bool, ApiError> {
    let mut changed = ensure_review_agent_definition(config)?;
    if ensure_image_agent_definition(config)? {
        changed = true;
    }
    if refresh_default_agent_permissions(config)? {
        changed = true;
    }
    Ok(changed)
}

fn refresh_default_agent_permissions(config: &mut GlobalConfig) -> Result<bool, ApiError> {
    let default_id = default_agent_definition_id()?;
    let allowed_agent_definition_ids = default_agent_allowed_definition_ids(config, &default_id);
    let Some(default_definition) = config
        .agent_definitions
        .iter_mut()
        .find(|definition| definition.id == default_id)
    else {
        return Ok(false);
    };

    let mut changed = false;
    if default_definition.permissions.allowed_agent_definition_ids != allowed_agent_definition_ids {
        default_definition.permissions.allowed_agent_definition_ids = allowed_agent_definition_ids;
        changed = true;
    }
    if !default_definition.permissions.can_create_instances {
        default_definition.permissions.can_create_instances = true;
        changed = true;
    }
    if !default_definition.permissions.can_delegate {
        default_definition.permissions.can_delegate = true;
        changed = true;
    }
    Ok(changed)
}

fn default_agent_allowed_definition_ids(
    config: &GlobalConfig,
    default_id: &AgentDefinitionId,
) -> Vec<AgentDefinitionId> {
    config
        .agent_definitions
        .iter()
        .filter(|definition| definition.id != *default_id)
        .map(|definition| definition.id.clone())
        .collect()
}

fn review_agent_definition_for_config(
    config: &GlobalConfig,
    id: AgentDefinitionId,
) -> Result<Option<AgentDefinitionSettings>, ApiError> {
    let Some(model) = default_agent_runner_model(config) else {
        return Ok(None);
    };
    let Some(provider_id) = model.active_provider_id.clone() else {
        return Ok(None);
    };
    let allowed_tools = [
        foco_tools::READ_FILE_TOOL,
        foco_tools::FIND_FILES_TOOL,
        foco_tools::SEARCH_TEXT_TOOL,
        foco_tools::WEB_FETCH_TOOL,
        foco_tools::RUN_COMMAND_TOOL,
        foco_tools::GRAPH_FIND_SYMBOLS_TOOL,
        foco_tools::GRAPH_FIND_CALLERS_TOOL,
        foco_tools::GRAPH_FIND_CALLEES_TOOL,
        foco_tools::GRAPH_FIND_REFERENCES_TOOL,
        foco_tools::GRAPH_RELATED_FILES_TOOL,
        foco_tools::GRAPH_EXPLORE_TOOL,
        foco_tools::ASK_QUESTION_TOOL,
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    let system_prompt =
        crate::prompt::active_system_prompt(&config.prompts, REVIEW_SYSTEM_PROMPT_NAME)?;

    Ok(Some(AgentDefinitionSettings {
        id,
        revision: AGENT_DEFINITION_INITIAL_REVISION,
        name: "Review".to_string(),
        description: "Built-in agent for focused code review and verification.".to_string(),
        provider_id,
        model_id: model.id.clone(),
        model_options: AgentModelOptions {
            thinking_level: model.thinking_level.clone(),
            max_output_tokens: None,
        },
        system_prompt,
        allowed_tools,
        max_instances: 1,
        allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
        permissions: AgentPermissions::default(),
    }))
}

fn image_agent_definition_for_config(
    config: &GlobalConfig,
    id: AgentDefinitionId,
) -> Option<AgentDefinitionSettings> {
    if !config
        .models
        .iter()
        .any(|model| image_model_available(config, model))
    {
        return None;
    }
    let model = default_agent_runner_model(config)?;
    let provider_id = model.active_provider_id.clone()?;
    let allowed_tools = [
        foco_tools::IMAGE_GEN_TOOL,
        foco_tools::ASK_QUESTION_TOOL,
        foco_tools::READ_FILE_TOOL,
        foco_tools::FIND_FILES_TOOL,
    ]
    .into_iter()
    .map(str::to_string)
    .collect();

    Some(AgentDefinitionSettings {
        id,
        revision: AGENT_DEFINITION_INITIAL_REVISION,
        name: "Image generation agent".to_string(),
        description: "Built-in agent dedicated to generating images with an image-output model."
            .to_string(),
        provider_id,
        model_id: model.id.clone(),
        model_options: AgentModelOptions {
            thinking_level: model.thinking_level.clone(),
            max_output_tokens: None,
        },
        system_prompt: IMAGE_AGENT_SYSTEM_PROMPT.to_string(),
        allowed_tools,
        max_instances: 1,
        allowed_execution_workspace_modes: vec![foco_agent::AgentExecutionWorkspaceMode::Shared],
        permissions: AgentPermissions::default(),
    })
}
fn default_agent_definition_for_config(
    config: &GlobalConfig,
    id: AgentDefinitionId,
) -> Option<AgentDefinitionSettings> {
    let model = default_agent_runner_model(config)?;
    let provider_id = model.active_provider_id.clone()?;
    let mut allowed_tools = foco_tools::builtin_tool_definitions()
        .into_iter()
        .map(|definition| definition.name.to_string())
        .collect::<Vec<_>>();
    allowed_tools.sort();
    allowed_tools.dedup();
    let allowed_agent_definition_ids = default_agent_allowed_definition_ids(config, &id);

    Some(AgentDefinitionSettings {
        id,
        revision: AGENT_DEFINITION_INITIAL_REVISION,
        name: "Default agent".to_string(),
        description: "Built-in default agent for chat and Team coordination.".to_string(),
        provider_id,
        model_id: model.id.clone(),
        model_options: AgentModelOptions {
            thinking_level: model.thinking_level.clone(),
            max_output_tokens: None,
        },
        system_prompt: DEFAULT_AGENT_SYSTEM_PROMPT.to_string(),
        allowed_tools,
        max_instances: 1,
        allowed_execution_workspace_modes: foco_agent::AgentExecutionWorkspaceMode::all(),
        permissions: AgentPermissions {
            can_create_instances: true,
            can_delegate: true,
            allowed_agent_definition_ids,
        },
    })
}

fn default_agent_runner_model(config: &GlobalConfig) -> Option<&ModelSettings> {
    config.models.iter().find(|model| {
        model.enabled
            && model.limits.is_some()
            && model_outputs_text(model)
            && model
                .active_provider_id
                .as_ref()
                .is_some_and(|provider_id| {
                    model.provider_ids.iter().any(|id| id == provider_id)
                        && config
                            .providers
                            .iter()
                            .any(|provider| provider.enabled && provider.id == *provider_id)
                })
    })
}

fn model_outputs_text(model: &ModelSettings) -> bool {
    model.output_modalities.is_empty()
        || model
            .output_modalities
            .iter()
            .any(|modality| modality == "text")
}

pub(crate) async fn create_agent_definition(
    State(state): State<AppState>,
    Json(request): Json<CreateAgentDefinitionRequest>,
) -> Result<Json<AgentDefinitionsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let id = AgentDefinitionId::new(unique_id("agent-definition"))
        .map_err(|error| ApiError::internal(error.message().to_string()))?;
    config.agent_definitions.push(agent_definition_from_input(
        id,
        AGENT_DEFINITION_INITIAL_REVISION,
        request.definition,
    ));
    refresh_builtin_agent_definitions(&mut config)?;
    validate_agent_definition_update(&state, &config).await?;
    save_config(&state, config.clone())?;

    Ok(agent_definitions_response(&config))
}

pub(crate) async fn update_agent_definition(
    State(state): State<AppState>,
    Json(request): Json<UpdateAgentDefinitionRequest>,
) -> Result<Json<AgentDefinitionsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let image_id = image_agent_definition_id()?;
    let updates_image_agent = request.id == image_id;
    let stored_index = config
        .agent_definitions
        .iter()
        .position(|definition| definition.id == request.id)
        .ok_or_else(|| {
            ApiError::bad_request(format!("agent definition was not found: {}", request.id))
        })?;
    let revision = config.agent_definitions[stored_index]
        .revision
        .checked_add(1)
        .ok_or_else(|| ApiError::internal("agent definition revision overflow"))?;
    config.agent_definitions[stored_index] =
        agent_definition_from_input(request.id, revision, request.definition);
    if updates_image_agent
        && !image_agent_runner_selection_valid(&config, &config.agent_definitions[stored_index])
    {
        return Err(ApiError::bad_request(
            "Image generation agent requires an enabled text-output runner model with an enabled provider",
        ));
    }
    refresh_builtin_agent_definitions(&mut config)?;
    validate_agent_definition_update(&state, &config).await?;
    save_config(&state, config.clone())?;

    Ok(agent_definitions_response(&config))
}

pub(crate) async fn delete_agent_definition(
    State(state): State<AppState>,
    Json(request): Json<DeleteAgentDefinitionRequest>,
) -> Result<Json<AgentDefinitionsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let default_id = default_agent_definition_id()?;
    let review_id = review_agent_definition_id()?;
    let image_id = image_agent_definition_id()?;
    if request.id == default_id || request.id == review_id || request.id == image_id {
        return Err(ApiError::bad_request(format!(
            "built-in agent definition '{}' cannot be deleted",
            request.id
        )));
    }
    if let Some(dependent) = config.agent_definitions.iter().find(|definition| {
        definition.id != default_id
            && definition
                .permissions
                .allowed_agent_definition_ids
                .contains(&request.id)
    }) {
        return Err(ApiError::bad_request(format!(
            "agent definition '{}' is referenced by agent definition '{}'",
            request.id, dependent.id
        )));
    }
    let definition_count = config.agent_definitions.len();
    config
        .agent_definitions
        .retain(|definition| definition.id != request.id);
    if config.agent_definitions.len() == definition_count {
        return Err(ApiError::bad_request(format!(
            "agent definition was not found: {}",
            request.id
        )));
    }
    if !config
        .agent_definitions
        .iter()
        .any(|definition| definition.id == default_id)
    {
        if let Some(definition) = default_agent_definition_for_config(&config, default_id) {
            config.agent_definitions.insert(0, definition);
        }
    }
    refresh_builtin_agent_definitions(&mut config)?;
    validate_agent_definition_update(&state, &config).await?;
    save_config(&state, config.clone())?;

    Ok(agent_definitions_response(&config))
}

fn agent_definition_from_input(
    id: AgentDefinitionId,
    revision: u64,
    input: AgentDefinitionInput,
) -> AgentDefinitionSettings {
    AgentDefinitionSettings {
        id,
        revision,
        name: input.name,
        description: input.description,
        provider_id: input.provider_id,
        model_id: input.model_id,
        model_options: input.model_options,
        system_prompt: input.system_prompt,
        allowed_tools: input.allowed_tools,
        max_instances: input.max_instances,
        allowed_execution_workspace_modes: input.allowed_execution_workspace_modes,
        permissions: input.permissions,
    }
}

async fn validate_agent_definition_update(
    state: &AppState,
    config: &GlobalConfig,
) -> Result<(), ApiError> {
    config
        .validate(Some(&state.config_file))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let known_tools = known_agent_tool_names(state, config).await;
    validate_agent_definition_tool_references(
        Some(&state.config_file),
        &config.agent_definitions,
        &known_tools,
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))
}

pub(crate) async fn known_agent_tool_names(
    state: &AppState,
    config: &GlobalConfig,
) -> HashSet<String> {
    let mut tools = foco_tools::builtin_tool_definitions()
        .into_iter()
        .map(|definition| definition.name.to_string())
        .collect::<HashSet<_>>();
    tools.extend(
        memory_tool_definitions()
            .into_iter()
            .map(|definition| definition.name),
    );
    tools.extend(
        state
            .mcp_registry
            .tool_definitions(&config.app.active_workspace_id)
            .await
            .into_iter()
            .map(|definition| definition.name),
    );
    tools
}

fn agent_definitions_response(config: &GlobalConfig) -> Json<AgentDefinitionsResponse> {
    Json(AgentDefinitionsResponse {
        agent_definitions: config.agent_definitions.clone(),
        default_role_prompts: default_agent_role_prompts(config),
    })
}

fn default_agent_role_prompts(config: &GlobalConfig) -> BTreeMap<AgentDefinitionId, String> {
    let mut prompts = BTreeMap::new();
    if let Ok(default_id) = default_agent_definition_id() {
        prompts.insert(default_id, DEFAULT_AGENT_SYSTEM_PROMPT.to_string());
    }
    if let Ok(review_id) = review_agent_definition_id() {
        if let Ok(prompt) =
            crate::prompt::active_system_prompt(&config.prompts, REVIEW_SYSTEM_PROMPT_NAME)
        {
            prompts.insert(review_id, prompt);
        }
    }
    if let Ok(image_id) = image_agent_definition_id() {
        if let Some(definition) = image_agent_definition_for_config(config, image_id.clone()) {
            prompts.insert(image_id, definition.system_prompt);
        }
    }
    prompts
}

pub(crate) async fn save_general_settings(
    State(state): State<AppState>,
    Json(request): Json<ManualGeneralSettingsRequest>,
) -> Result<Response, ApiError> {
    let mut config = config_snapshot(&state)?;
    let current_language = config.app.language.clone();
    let should_set_auth_cookie = request
        .password
        .as_ref()
        .is_some_and(|password| !password.trim().is_empty());
    let should_clear_auth_cookie = request.clear_password.unwrap_or(false);

    config.app.web_server = normalize_web_server_settings(&config.app.web_server, &request)?;
    let previous_api_audit = config.app.api_audit.clone();
    config.app.api_audit =
        normalize_api_audit_settings(&config.app.api_audit, request.api_audit.as_ref())?;
    if let Some(retry_count) = request.llm_request_retry_count {
        config.app.llm_request_retry_count = retry_count;
    }
    config.app.language = normalize_app_language(&request.language)?;
    config.app.theme = normalize_app_theme(&request.theme)?;
    if let Some(hook_audit_enabled) = request.hook_audit_enabled {
        config.hooks.audit_enabled = hook_audit_enabled;
    }
    if let Some(auto_start_enabled) = request.auto_start_enabled {
        apply_auto_start_setting(auto_start_enabled)?;
        config.app.auto_start_enabled = auto_start_enabled;
    }
    if let Some(default_team_mode_enabled) = request.default_team_mode_enabled {
        config.app.default_team_mode_enabled = default_team_mode_enabled;
    }
    validate_tray_menu_language(&config.app.language)?;

    save_config(&state, config.clone())?;
    if config.app.api_audit != previous_api_audit {
        spawn_api_audit_cleanup_once(state.clone(), config.clone());
    }
    notify_tray_menu_language_change(&state, &current_language, &config.app.language)?;

    let response = settings_response(&state, &config).await?;
    if should_set_auth_cookie {
        let password_hash = config
            .app
            .web_server
            .password_hash
            .as_deref()
            .ok_or_else(|| ApiError::internal("saved password hash is missing"))?;
        return Ok(([(header::SET_COOKIE, auth_cookie(password_hash))], response).into_response());
    }
    if should_clear_auth_cookie {
        return Ok(([(header::SET_COOKIE, expired_auth_cookie())], response).into_response());
    }

    Ok(response.into_response())
}

pub(crate) async fn save_web_search_settings(
    State(state): State<AppState>,
    Json(request): Json<ManualWebSearchSettingsRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let active_provider = request.active_provider.trim();

    if !SUPPORTED_WEB_SEARCH_PROVIDERS.contains(&active_provider) {
        return Err(ApiError::bad_request(format!(
            "web search provider '{active_provider}' is unsupported"
        )));
    }

    config.web_search.enabled = request.enabled;
    config.web_search.active_provider = active_provider.to_string();
    config.web_search.api_proxy =
        normalize_api_proxy_settings(&config.web_search.api_proxy, request.api_proxy.as_ref())?;
    apply_web_search_api_key_update(
        &mut config.web_search.tavily_api_key,
        request.tavily_api_key,
        request.clear_tavily_api_key.unwrap_or(false),
    );
    apply_web_search_api_key_update(
        &mut config.web_search.brave_api_key,
        request.brave_api_key,
        request.clear_brave_api_key.unwrap_or(false),
    );
    config
        .validate(Some(&state.config_file))
        .map_err(ApiError::from_config_error)?;
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

fn apply_web_search_api_key_update(
    current: &mut Option<String>,
    next: Option<String>,
    clear: bool,
) {
    match optional_trimmed_string(next) {
        Some(value) => *current = Some(value),
        None if clear => *current = None,
        None => {}
    }
}

pub(crate) async fn save_memory_settings(
    State(state): State<AppState>,
    Json(request): Json<ManualMemorySettingsRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let extraction_model_id = request
        .extraction_model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let retrieval_model_id = request
        .retrieval_model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let dream = match request.dream {
        Some(dream) => memory_dream_settings_from_request(&config.memory.dream, dream),
        None => config.memory.dream.clone(),
    };

    config.memory = MemorySettings {
        enabled: request.enabled,
        extraction_mode: request.extraction_mode.trim().to_string(),
        retrieval_mode: request.retrieval_mode.trim().to_string(),
        retention_days: request.retention_days,
        extraction_model_id,
        retrieval_model_id,
        dream,
    };
    config
        .validate(Some(&state.config_file))
        .map_err(ApiError::from_config_error)?;
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

pub(crate) async fn save_spec_settings(
    State(state): State<AppState>,
    Json(request): Json<ManualSpecSettingsRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    config.spec = SpecSettings {
        auto_enabled: request.auto_enabled,
        generation_model_id: optional_trimmed_string(request.generation_model_id),
        generation_system_prompt: optional_trimmed_string(request.generation_system_prompt),
        update_system_prompt: optional_trimmed_string(request.update_system_prompt),
    };
    config
        .validate(Some(&state.config_file))
        .map_err(ApiError::from_config_error)?;
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

pub(crate) async fn save_plan_settings(
    State(state): State<AppState>,
    Json(request): Json<ManualPlanSettingsRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    config.plan = PlanSettings {
        merge_automation_mode: request.merge_automation_mode.trim().to_string(),
    };
    config
        .validate(Some(&state.config_file))
        .map_err(ApiError::from_config_error)?;
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

fn memory_dream_settings_from_request(
    current: &MemoryDreamSettings,
    request: ManualMemoryDreamSettingsRequest,
) -> MemoryDreamSettings {
    MemoryDreamSettings {
        enabled: request.enabled,
        auto_enabled: request.auto_enabled,
        mode: request.mode.trim().to_string(),
        model_id: request
            .model_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        workspace_interval_days: request.workspace_interval_days,
        global_interval_days: request.global_interval_days,
        create_transcript_chat: request.create_transcript_chat,
        max_facts_per_run: request.max_facts_per_run,
        max_changes_per_run: request.max_changes_per_run,
        scheduler_scan_minutes: request.scheduler_scan_minutes,
        workspace_threshold_facts: request
            .workspace_threshold_facts
            .unwrap_or(current.workspace_threshold_facts),
        global_threshold_facts: request
            .global_threshold_facts
            .unwrap_or(current.global_threshold_facts),
    }
}

pub(crate) async fn save_prompt_settings(
    State(state): State<AppState>,
    Json(request): Json<ManualPromptSettingsRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let system_prompts = normalize_system_prompt_requests(
        request.system_prompts,
        request.system_prompt,
        &build_default_system_prompt(),
    )?;
    let system_prompts = system_prompts
        .into_iter()
        .filter(|prompt| prompt.name != IMAGE_GENERATION_SYSTEM_PROMPT_NAME)
        .collect();

    config.prompts = PromptSettings {
        system_prompts,
        system_prompt: None,
        files: normalize_prompt_file_paths(request.files)?,
        extra_text: request.extra_text.trim().to_string(),
    };
    refresh_builtin_agent_definitions(&mut config)?;
    refresh_review_agent_system_prompt(&mut config)?;
    config
        .validate(Some(&state.config_file))
        .map_err(ApiError::from_config_error)?;
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

#[cfg(all(windows, not(debug_assertions)))]
fn validate_tray_menu_language(language: &str) -> Result<(), ApiError> {
    tray_menu_labels(language)
        .map(|_| ())
        .map_err(ApiError::internal)
}

#[cfg(any(not(windows), debug_assertions))]
fn validate_tray_menu_language(_language: &str) -> Result<(), ApiError> {
    Ok(())
}

#[cfg(all(windows, not(debug_assertions)))]
fn notify_tray_menu_language_change(
    state: &AppState,
    current_language: &str,
    next_language: &str,
) -> Result<(), ApiError> {
    if current_language == next_language {
        return Ok(());
    }

    state
        .tray_menu_update_notifier
        .notify(tray_menu_labels(next_language).map_err(ApiError::internal)?)
        .map_err(ApiError::internal)
}

#[cfg(any(not(windows), debug_assertions))]
fn notify_tray_menu_language_change(
    _state: &AppState,
    _current_language: &str,
    _next_language: &str,
) -> Result<(), ApiError> {
    Ok(())
}

pub(crate) async fn save_manual_provider(
    State(state): State<AppState>,
    Json(request): Json<ManualProviderRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let id = request.id.trim();
    let name = request.name.trim();
    let kind = request.kind.trim();
    let base_url = optional_trimmed_string(request.base_url);
    let existing_provider = config.providers.iter().find(|provider| provider.id == id);
    let is_new_provider = existing_provider.is_none();
    let api_key = match optional_trimmed_string(request.api_key) {
        Some(value) => Some(value),
        None if request.clear_api_key.unwrap_or(false) => None,
        None => existing_provider.and_then(|provider| provider.api_key.clone()),
    };

    if id.is_empty() {
        return Err(ApiError::bad_request("provider id must not be empty"));
    }

    if name.is_empty() {
        return Err(ApiError::bad_request("provider name must not be empty"));
    }

    let provider_kind =
        parse_provider_kind(kind).map_err(|source| ApiError::bad_request(source.to_string()))?;
    let normalized_base_url = match base_url {
        Some(value) => Some(
            normalized_base_url(&value)
                .map_err(|source| ApiError::bad_request(source.to_string()))?,
        ),
        None => None,
    };
    let model_sync_filter_regex = optional_trimmed_string(request.model_sync_filter_regex);
    validate_provider_model_sync_filter(model_sync_filter_regex.as_deref())?;
    let current_api_proxy = existing_provider
        .map(|provider| provider.api_proxy.clone())
        .unwrap_or_default();
    let api_proxy = normalize_api_proxy_settings(&current_api_proxy, request.api_proxy.as_ref())?;
    for request_override in &request.request_overrides {
        request_override
            .validate()
            .map_err(|source| ApiError::bad_request(source.to_string()))?;
    }
    let provider = ProviderSettings {
        id: id.to_string(),
        name: name.to_string(),
        kind: provider_kind.as_str().to_string(),
        enabled: request.enabled,
        base_url: normalized_base_url,
        api_key,
        auto_sync_models: request.auto_sync_models,
        model_sync_filter_regex,
        request_overrides: request.request_overrides,
        api_proxy,
    };

    if is_new_provider {
        match fetch_provider_model_ids(&provider_connection_config(&provider)?).await {
            Ok(model_ids) => {
                let model_ids = filter_provider_model_ids(&provider, model_ids)?;
                associate_provider_with_local_models(&mut config.models, &provider.id, &model_ids);
            }
            Err(source) if can_save_new_provider_after_model_list_error(&source) => {
                tracing::warn!(
                    provider_id = %provider.id,
                    provider_kind = %provider.kind,
                    error = ?source,
                    "saving new provider without model associations because model list could not be fetched"
                );
            }
            Err(source) => return Err(ApiError::from_provider_config_error(source)),
        }
    }

    if let Some(stored_provider) = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == id)
    {
        *stored_provider = provider;
    } else {
        config.providers.push(provider);
    }

    refresh_builtin_agent_definitions(&mut config)?;

    config
        .validate(Some(&state.config_file))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

pub(crate) fn associate_provider_with_local_models(
    models: &mut [ModelSettings],
    provider_id: &str,
    provider_model_ids: &[String],
) {
    let provider_model_ids = provider_model_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    for model in models {
        if provider_model_ids.contains(model.id.as_str()) {
            if !model.provider_ids.iter().any(|id| id == provider_id) {
                model.provider_ids.push(provider_id.to_string());
            }
            if model.active_provider_id.is_none() {
                model.active_provider_id = Some(provider_id.to_string());
            }
        } else if model.provider_ids.iter().any(|id| id == provider_id) {
            model.provider_ids.retain(|id| id != provider_id);
            if model.active_provider_id.as_deref() == Some(provider_id) {
                model.active_provider_id = model.provider_ids.first().cloned();
            }
        }
    }
}

pub(crate) fn can_save_new_provider_after_model_list_error(error: &ProviderConfigError) -> bool {
    matches!(error, ProviderConfigError::Connection { .. })
}

pub(crate) async fn refresh_provider_models(
    State(state): State<AppState>,
) -> Result<Json<ProviderModelsRefreshResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let providers = config.providers.clone();
    let refreshed_providers = sync_provider_model_associations(&mut config, providers).await?;

    config
        .validate(Some(&state.config_file))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    save_config(&state, config.clone())?;

    let Json(settings) = settings_response(&state, &config).await?;
    Ok(Json(ProviderModelsRefreshResponse {
        providers: refreshed_providers,
        settings,
    }))
}

pub(crate) async fn sync_auto_provider_models_once(state: &AppState) -> Result<usize, ApiError> {
    let mut config = config_snapshot(state)?;
    let providers = config
        .providers
        .iter()
        .filter(|provider| provider.enabled && provider.auto_sync_models)
        .cloned()
        .collect::<Vec<_>>();

    if providers.is_empty() {
        return Ok(0);
    }

    let provider_count = providers.len();
    let previous_providers = config.providers.clone();
    let previous_models = config.models.clone();
    sync_provider_model_associations(&mut config, providers).await?;

    if config.providers != previous_providers || config.models != previous_models {
        refresh_builtin_agent_definitions(&mut config)?;
        config
            .validate(Some(&state.config_file))
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        save_config(state, config)?;
    }

    Ok(provider_count)
}

async fn sync_provider_model_associations(
    config: &mut GlobalConfig,
    providers: Vec<ProviderSettings>,
) -> Result<Vec<ProviderModelsResponse>, ApiError> {
    let mut refreshed_providers = Vec::new();

    for provider in providers {
        let models = match provider_connection_config(&provider) {
            Ok(connection_config) => match fetch_provider_model_ids(&connection_config).await {
                Ok(model_ids) => {
                    let model_ids = filter_provider_model_ids(&provider, model_ids)?;
                    associate_provider_with_local_models(
                        &mut config.models,
                        &provider.id,
                        &model_ids,
                    );
                    model_ids
                }
                Err(source) => {
                    tracing::warn!(
                        provider_id = %provider.id,
                        error = ?source,
                        "disabling provider after model list sync failed"
                    );
                    disable_provider(&mut config.providers, &provider.id);
                    Vec::new()
                }
            },
            Err(source) => {
                tracing::warn!(
                    provider_id = %provider.id,
                    error = ?source,
                    "disabling provider after provider config build failed"
                );
                disable_provider(&mut config.providers, &provider.id);
                Vec::new()
            }
        };

        refreshed_providers.push(ProviderModelsResponse {
            provider_id: provider.id,
            models,
        });
    }

    Ok(refreshed_providers)
}

pub(crate) fn filter_provider_model_ids(
    provider: &ProviderSettings,
    model_ids: Vec<String>,
) -> Result<Vec<String>, ApiError> {
    let Some(pattern) = provider
        .model_sync_filter_regex
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(model_ids);
    };

    let regex = Regex::new(pattern)
        .map_err(|source| ApiError::bad_request(format!("invalid model sync regex: {source}")))?;
    let mut filtered_model_ids = Vec::new();
    for model_id in model_ids {
        if regex.is_match(&model_id).map_err(|source| {
            ApiError::bad_request(format!("model sync regex match failed: {source}"))
        })? {
            filtered_model_ids.push(model_id);
        }
    }

    Ok(filtered_model_ids)
}

fn validate_provider_model_sync_filter(pattern: Option<&str>) -> Result<(), ApiError> {
    if let Some(pattern) = pattern.map(str::trim).filter(|value| !value.is_empty()) {
        Regex::new(pattern).map_err(|source| {
            ApiError::bad_request(format!("invalid model sync regex: {source}"))
        })?;
    }

    Ok(())
}

fn disable_provider(providers: &mut [ProviderSettings], provider_id: &str) {
    if let Some(provider) = providers
        .iter_mut()
        .find(|provider| provider.id == provider_id)
    {
        provider.enabled = false;
    }
}

pub(crate) async fn delete_provider(
    State(state): State<AppState>,
    Json(request): Json<DeleteSettingsItemRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let id = request.id.trim();

    if id.is_empty() {
        return Err(ApiError::bad_request("provider id must not be empty"));
    }

    let image_id = image_agent_definition_id()?;
    if let Some(definition) = config
        .agent_definitions
        .iter()
        .find(|definition| definition.id != image_id && definition.provider_id == id)
    {
        return Err(ApiError::bad_request(format!(
            "provider '{id}' is referenced by agent definition '{}'",
            definition.id
        )));
    }

    let provider_count = config.providers.len();
    config.providers.retain(|provider| provider.id != id);

    if config.providers.len() == provider_count {
        return Err(ApiError::bad_request(format!(
            "provider was not found: {id}"
        )));
    }

    for model in &mut config.models {
        model.provider_ids.retain(|provider_id| provider_id != id);
        if model.active_provider_id.as_deref() == Some(id) {
            model.active_provider_id = model.provider_ids.first().cloned();
        }
    }

    refresh_builtin_agent_definitions(&mut config)?;
    config
        .validate(Some(&state.config_file))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

pub(crate) async fn save_mcp_server(
    State(state): State<AppState>,
    Json(request): Json<ManualMcpServerRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let id = request.id.trim();
    let name = request.name.trim();
    let transport = request.transport.trim();

    if id.is_empty() {
        return Err(ApiError::bad_request("MCP server id must not be empty"));
    }

    if name.is_empty() {
        return Err(ApiError::bad_request("MCP server name must not be empty"));
    }

    foco_mcp::McpTransportKind::parse(transport)
        .map_err(|source| ApiError::bad_request(source.to_string()))?;

    let server = McpServerConfig {
        id: id.to_string(),
        name: name.to_string(),
        enabled: request.enabled,
        transport: transport.to_string(),
        command: optional_trimmed_string(request.command),
        args: request.args.unwrap_or_default(),
        url: optional_trimmed_string(request.url),
    };
    let definition = server
        .to_definition()
        .map_err(|source| ApiError::bad_request(source.to_string()))?;
    foco_mcp::validate_server_definitions(&[definition])
        .map_err(|source| ApiError::bad_request(source.to_string()))?;

    if let Some(stored_server) = config.mcp.servers.iter_mut().find(|server| server.id == id) {
        *stored_server = server;
    } else {
        config.mcp.servers.push(server);
    }

    save_config(&state, config.clone())?;
    sync_all_mcp_workspaces(&state.mcp_registry, &config)
        .await
        .map_err(ApiError::from_mcp_error)?;

    settings_response(&state, &config).await
}

pub(crate) async fn delete_mcp_server(
    State(state): State<AppState>,
    Json(request): Json<DeleteSettingsItemRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let id = request.id.trim();

    if id.is_empty() {
        return Err(ApiError::bad_request("MCP server id must not be empty"));
    }

    let server_count = config.mcp.servers.len();
    config.mcp.servers.retain(|server| server.id != id);

    if config.mcp.servers.len() == server_count {
        return Err(ApiError::bad_request(format!(
            "MCP server was not found: {id}"
        )));
    }

    save_config(&state, config.clone())?;
    sync_all_mcp_workspaces(&state.mcp_registry, &config)
        .await
        .map_err(ApiError::from_mcp_error)?;

    settings_response(&state, &config).await
}

pub(crate) async fn save_skills(
    State(state): State<AppState>,
    Json(request): Json<ManualSkillsRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let discovery = discover_skills(&state.user_profile_dir, &config.workspaces);
    let disabled =
        normalize_manual_disabled_skill_ids(request.disabled, request.enabled, &discovery.skills)?;

    config.skills.directories.clear();
    config.skills.detected = discovery.skills;
    config.skills.disabled = merge_disabled_skill_keys(disabled, &discovery.required_disabled);
    refresh_derived_enabled_skills(&mut config);

    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

pub(crate) async fn refresh_skills(
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let discovery = discover_skills(&state.user_profile_dir, &config.workspaces);

    config.skills.detected = discovery.skills;
    config.skills.disabled =
        merge_disabled_skill_keys(config.skills.disabled.clone(), &discovery.required_disabled);
    refresh_derived_enabled_skills(&mut config);

    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

pub(crate) async fn test_provider(
    State(state): State<AppState>,
    Json(request): Json<TestProviderRequest>,
) -> Result<Json<ProviderTestResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let provider_id = request.provider_id.trim();
    let provider = config
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| ApiError::bad_request(format!("provider was not found: {provider_id}")))?;

    if !provider.enabled {
        return Err(ApiError::bad_request(format!(
            "provider '{}' is disabled",
            provider.id
        )));
    }

    let connection_config = provider_connection_config(provider)?;
    let model_count = test_provider_connection(&connection_config)
        .await
        .map_err(ApiError::from_provider_config_error)?;

    Ok(Json(ProviderTestResponse {
        ok: true,
        message: format!("Connected; provider returned {model_count} models"),
        model_count,
    }))
}

pub(crate) async fn provider_models(
    State(state): State<AppState>,
    Json(request): Json<TestProviderRequest>,
) -> Result<Json<ProviderModelsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let provider_id = request.provider_id.trim();
    let provider = config
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| ApiError::bad_request(format!("provider was not found: {provider_id}")))?;

    let connection_config = provider_connection_config(provider)?;
    let models = fetch_provider_model_ids(&connection_config)
        .await
        .map_err(ApiError::from_provider_config_error)?;
    let models = filter_provider_model_ids(provider, models)?;

    Ok(Json(ProviderModelsResponse {
        provider_id: provider.id.clone(),
        models,
    }))
}

pub(crate) async fn model_metadata(
    State(state): State<AppState>,
) -> Result<Json<ModelMetadataResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let cache = read_model_metadata_cache(&state.model_metadata_file)
        .map_err(ApiError::from_model_metadata_error)?;

    Ok(Json(model_metadata_response(
        cache,
        &config,
        &state.model_metadata_file,
    )))
}

pub(crate) async fn refresh_model_metadata(
    State(state): State<AppState>,
) -> Result<Json<ModelMetadataResponse>, ApiError> {
    let fetched_at = utc_timestamp();
    let content = reqwest::get(MODELS_DEV_API_URL)
        .await
        .map_err(|source| {
            ApiError::internal(format!("failed to fetch models.dev metadata: {source}"))
        })?
        .error_for_status()
        .map_err(|source| {
            ApiError::internal(format!("models.dev metadata request failed: {source}"))
        })?
        .text()
        .await
        .map_err(|source| {
            ApiError::internal(format!("failed to read models.dev metadata: {source}"))
        })?;
    let cache = parse_models_dev_metadata(&content, MODELS_DEV_API_URL, &fetched_at)
        .map_err(ApiError::from_model_metadata_error)?;

    write_model_metadata_cache(&state.model_metadata_file, &cache)
        .map_err(ApiError::from_model_metadata_error)?;

    let config = config_snapshot(&state)?;

    Ok(Json(model_metadata_response(
        Some(cache),
        &config,
        &state.model_metadata_file,
    )))
}

pub(crate) async fn save_manual_model(
    State(state): State<AppState>,
    Json(request): Json<ManualModelRequest>,
) -> Result<Json<ModelMetadataResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let model_id = request.model_id.trim();
    let display_name = request.display_name.trim();
    let context_window = request.context_window.filter(|value| *value > 0);
    let max_output_tokens = request.max_output_tokens.filter(|value| *value > 0);
    let requested_provider_ids = request.provider_ids;
    let requested_active_provider_id = request.active_provider_id;
    let requested_input_modalities = request.input_modalities;
    let requested_output_modalities = request.output_modalities;
    let requested_thinking_level = request.thinking_level;
    let clear_thinking_level = request.clear_thinking_level.unwrap_or(false);
    let requested_system_prompt_name = request.system_prompt_name;
    let metadata_key = request
        .metadata_key
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let metadata_record = match metadata_key.as_deref() {
        Some(key) => cached_model_record(&state.model_metadata_file, key)
            .map_err(ApiError::from_model_metadata_error)?,
        None => None,
    };

    if model_id.is_empty() {
        return Err(ApiError::bad_request("model id must not be empty"));
    }

    if display_name.is_empty() {
        return Err(ApiError::bad_request("display name must not be empty"));
    }

    if metadata_key.is_some() && metadata_record.is_none() {
        return Err(ApiError::bad_request(format!(
            "model metadata key was not found in cache: {}",
            metadata_key.as_deref().unwrap_or_default()
        )));
    }

    let existing_model = config.models.iter().find(|model| model.id == model_id);
    let input_modalities = normalize_model_modalities(
        requested_input_modalities,
        existing_model.map(|model| model.input_modalities.as_slice()),
        metadata_record
            .as_ref()
            .map(|record| record.input_modalities.as_slice()),
        &["text"],
    );
    let output_modalities = normalize_model_modalities(
        requested_output_modalities,
        existing_model.map(|model| model.output_modalities.as_slice()),
        metadata_record
            .as_ref()
            .map(|record| record.output_modalities.as_slice()),
        &["text"],
    );
    let requires_text_limits = output_modalities.iter().any(|modality| modality == "text");

    if request.enabled
        && requires_text_limits
        && (context_window.is_none() || max_output_tokens.is_none())
    {
        return Err(ApiError::bad_request(
            "enabled text-output model requires context window and max output tokens",
        ));
    }

    let limits = match (context_window, max_output_tokens) {
        (Some(context_window), Some(max_output_tokens)) => {
            if context_window == 0 {
                return Err(ApiError::bad_request(
                    "context window must be greater than 0",
                ));
            }

            if max_output_tokens == 0 {
                return Err(ApiError::bad_request(
                    "max output tokens must be greater than 0",
                ));
            }

            Some(ModelLimits {
                context_window,
                max_output_tokens,
            })
        }
        (None, None) => None,
        _ => {
            return Err(ApiError::bad_request(
                "context window and max output tokens must be saved together",
            ));
        }
    };

    let provider_ids = normalize_model_provider_ids(requested_provider_ids, existing_model)?;
    let active_provider_id = match requested_active_provider_id {
        Some(value) => optional_trimmed_string(Some(value)),
        None => existing_model.and_then(|model| model.active_provider_id.clone()),
    };
    let active_provider_id = if provider_ids.is_empty() {
        None
    } else {
        active_provider_id
    };
    let thinking_level = match requested_thinking_level {
        Some(value) => optional_trimmed_string(Some(value)),
        None if clear_thinking_level => None,
        None => existing_model.and_then(|model| model.thinking_level.clone()),
    };
    let system_prompt_name = match requested_system_prompt_name {
        Some(value) => {
            let value = value.trim().to_string();
            if value.is_empty() {
                return Err(ApiError::bad_request(
                    "model system prompt name must not be empty",
                ));
            }
            value
        }
        None => existing_model
            .map(|model| model.system_prompt_name.clone())
            .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT_NAME.to_string()),
    };

    validate_model_provider_references(&config, &provider_ids, active_provider_id.as_deref())?;

    let model = ModelSettings {
        id: model_id.to_string(),
        display_name: display_name.to_string(),
        enabled: request.enabled,
        provider_ids,
        active_provider_id,
        thinking_level,
        system_prompt_name,
        metadata_key: metadata_key
            .clone()
            .or_else(|| metadata_record.as_ref().map(|record| record.key.clone())),
        metadata_source_url: metadata_record
            .as_ref()
            .map(|record| record.source_url.clone()),
        metadata_refreshed_at: metadata_record
            .as_ref()
            .map(|record| record.refreshed_at.clone()),
        limits,
        input_modalities,
        output_modalities,
    };

    if let Some(stored_model) = config.models.iter_mut().find(|model| model.id == model_id) {
        *stored_model = model;
    } else {
        config.models.push(model);
    }

    refresh_builtin_agent_definitions(&mut config)?;

    config
        .validate(Some(&state.config_file))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    save_config(&state, config.clone())?;

    let cache = read_model_metadata_cache(&state.model_metadata_file)
        .map_err(ApiError::from_model_metadata_error)?;

    Ok(Json(model_metadata_response(
        cache,
        &config,
        &state.model_metadata_file,
    )))
}

pub(crate) async fn delete_model(
    State(state): State<AppState>,
    Json(request): Json<DeleteSettingsItemRequest>,
) -> Result<Json<ModelMetadataResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let id = request.id.trim();

    if id.is_empty() {
        return Err(ApiError::bad_request("model id must not be empty"));
    }

    let image_id = image_agent_definition_id()?;
    if let Some(definition) = config
        .agent_definitions
        .iter()
        .find(|definition| definition.id != image_id && definition.model_id == id)
    {
        return Err(ApiError::bad_request(format!(
            "model '{id}' is referenced by agent definition '{}'",
            definition.id
        )));
    }

    let model_count = config.models.len();
    config.models.retain(|model| model.id != id);

    if config.models.len() == model_count {
        return Err(ApiError::bad_request(format!("model was not found: {id}")));
    }

    refresh_builtin_agent_definitions(&mut config)?;
    config
        .validate(Some(&state.config_file))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    save_config(&state, config.clone())?;

    let cache = read_model_metadata_cache(&state.model_metadata_file)
        .map_err(ApiError::from_model_metadata_error)?;

    Ok(Json(model_metadata_response(
        cache,
        &config,
        &state.model_metadata_file,
    )))
}

pub(crate) async fn save_model_order(
    State(state): State<AppState>,
    Json(request): Json<ModelOrderRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;

    reorder_models(&mut config.models, request.model_ids)?;
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}
