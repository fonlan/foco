use axum::{
    Json,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use foco_agent::build_default_system_prompt;
use foco_providers::{
    fetch_provider_model_ids, normalized_base_url, parse_provider_kind, test_provider_connection,
};
use foco_store::{
    config::PromptSettings,
    model_metadata::{
        MODELS_DEV_API_URL, parse_models_dev_metadata, read_model_metadata_cache,
        write_model_metadata_cache,
    },
};

use crate::*;

const DEFAULT_AGENT_DEFINITION_ID: &str = "agent-definition-default";
const DEFAULT_AGENT_SYSTEM_PROMPT: &str = "You are Foco's default coding agent. Complete the user's task directly. When Agent team tools are available, coordinate or create worker agents only when that materially reduces the work.";

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
    let default_id = default_agent_definition_id()?;
    if config
        .agent_definitions
        .iter()
        .any(|definition| definition.id == default_id)
    {
        if refresh_default_agent_permissions(&mut config)? {
            validate_agent_definition_update(state, &config).await?;
            save_config(state, config.clone())?;
        }
        return Ok(config);
    }

    let Some(definition) = default_agent_definition_for_config(&config, default_id) else {
        return Ok(config);
    };
    config.agent_definitions.insert(0, definition);
    validate_agent_definition_update(state, &config).await?;
    save_config(state, config.clone())?;
    Ok(config)
}

fn default_agent_definition_id() -> Result<AgentDefinitionId, ApiError> {
    AgentDefinitionId::new(DEFAULT_AGENT_DEFINITION_ID)
        .map_err(|error| ApiError::internal(error.message().to_string()))
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

fn default_agent_definition_for_config(
    config: &GlobalConfig,
    id: AgentDefinitionId,
) -> Option<AgentDefinitionSettings> {
    let model = config.models.iter().find(|model| {
        model.enabled
            && model.limits.is_some()
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
    })?;
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
    refresh_default_agent_permissions(&mut config)?;
    validate_agent_definition_update(&state, &config).await?;
    save_config(&state, config.clone())?;

    Ok(agent_definitions_response(&config))
}

pub(crate) async fn update_agent_definition(
    State(state): State<AppState>,
    Json(request): Json<UpdateAgentDefinitionRequest>,
) -> Result<Json<AgentDefinitionsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let stored = config
        .agent_definitions
        .iter_mut()
        .find(|definition| definition.id == request.id)
        .ok_or_else(|| {
            ApiError::bad_request(format!("agent definition was not found: {}", request.id))
        })?;
    let revision = stored
        .revision
        .checked_add(1)
        .ok_or_else(|| ApiError::internal("agent definition revision overflow"))?;
    *stored = agent_definition_from_input(request.id, revision, request.definition);
    refresh_default_agent_permissions(&mut config)?;
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
    refresh_default_agent_permissions(&mut config)?;
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
    })
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

    config.memory = MemorySettings {
        enabled: request.enabled,
        extraction_mode: request.extraction_mode.trim().to_string(),
        retrieval_mode: request.retrieval_mode.trim().to_string(),
        retention_days: request.retention_days,
        extraction_model_id,
        retrieval_model_id,
    };
    config
        .validate(Some(&state.config_file))
        .map_err(ApiError::from_config_error)?;
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
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

    config.prompts = PromptSettings {
        system_prompts,
        system_prompt: None,
        files: normalize_prompt_file_paths(request.files)?,
        extra_text: request.extra_text.trim().to_string(),
    };
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
        request_overrides: request.request_overrides,
        api_proxy,
    };

    if is_new_provider {
        let model_ids = fetch_provider_model_ids(&provider_connection_config(&provider)?)
            .await
            .map_err(ApiError::from_provider_config_error)?;
        associate_provider_with_local_models(&mut config.models, &provider.id, &model_ids);
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

pub(crate) async fn refresh_provider_models(
    State(state): State<AppState>,
) -> Result<Json<ProviderModelsRefreshResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let providers = config.providers.clone();
    let mut refreshed_providers = Vec::new();

    for provider in providers {
        let models = match provider_connection_config(&provider) {
            Ok(connection_config) => match fetch_provider_model_ids(&connection_config).await {
                Ok(model_ids) => {
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
                        "disabling provider after model list refresh failed"
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

    if let Some(definition) = config
        .agent_definitions
        .iter()
        .find(|definition| definition.provider_id == id)
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
    let context_window = request.context_window;
    let max_output_tokens = request.max_output_tokens;
    let requested_provider_ids = request.provider_ids;
    let requested_active_provider_id = request.active_provider_id;
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

    if request.enabled && (context_window.is_none() || max_output_tokens.is_none()) {
        return Err(ApiError::bad_request(
            "enabled model requires context window and max output tokens",
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

    let existing_model = config.models.iter().find(|model| model.id == model_id);
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
    };

    if let Some(stored_model) = config.models.iter_mut().find(|model| model.id == model_id) {
        *stored_model = model;
    } else {
        config.models.push(model);
    }

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

    if let Some(definition) = config
        .agent_definitions
        .iter()
        .find(|definition| definition.model_id == id)
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
