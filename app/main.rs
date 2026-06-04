use std::{
    collections::HashSet,
    convert::Infallible,
    env, fs,
    net::{Ipv4Addr, SocketAddr},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State, ws::WebSocketUpgrade},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use chrono::{SecondsFormat, Utc};
use foco_agent::{
    CodeGraphPromptContext, ContextPackItem, PendingToolCall, SkillPromptInfo, SystemPromptInput,
    ToolPromptInfo, build_system_prompt, calculate_context_budget,
    detect_same_file_write_conflicts, estimate_json_tokens, estimate_text_tokens, pack_context,
    plan_context_compression,
};
use foco_graph::{CodeGraphWatcher, index_workspace, start_code_graph_watcher};
use foco_mcp::{
    McpRegistry, McpServerDefinition, McpServerState, McpToolDefinition, is_mcp_tool_name,
};
use foco_providers::{
    DEFAULT_OPENAI_BASE_URL, NeutralChatMessage, NeutralChatRequest, NeutralChatRole,
    NeutralChatStreamEvent, NeutralToolCall, NeutralToolDefinition, NeutralUsage, OPENAI_CHAT_KIND,
    OPENAI_RESPONSES_KIND, ProviderConnectionConfig, normalized_base_url, parse_provider_kind,
    stream_chat, test_provider_connection,
};
use foco_store::{
    config::{
        GlobalConfig, McpServerConfig, ModelLimits, ModelSettings, ProviderSettings, SkillSettings,
        WorkspaceConfig, default_skill_directories_for_profile, load_or_create_global_config,
        save_global_config,
    },
    model_metadata::{
        MODELS_DEV_API_URL, ModelMetadataCache, ModelMetadataError, ModelMetadataRecord,
        parse_models_dev_metadata, read_model_metadata_cache, write_model_metadata_cache,
    },
    workspace::{
        ChatRecord, ContextCompressionSnapshotRecord, MessageRecord, NewContextCompressionSnapshot,
        NewLlmRequest, NewLlmRequestEvent, NewMessage, NewTerminalSession, NewToolCall,
        NewToolResult, ToolCallWithResultRecord, WorkspaceDatabase, initialize_workspace_databases,
    },
};
use foco_tools::{
    RUN_COMMAND_TOOL, SEARCH_TEXT_TOOL, ToolExecution, WRITE_FILE_TOOL, builtin_tool_definitions,
    builtin_tool_timeout_ms, execute_builtin_tool,
};
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::time::timeout;
use tower_http::services::ServeDir;

mod logging;
mod terminal;

const DEFAULT_PORT: u16 = 3210;
const PORT_ENV: &str = "FOCO_PORT";
const MAX_AGENT_TOOL_ROUNDS: usize = 8;
const CONTEXT_COMPRESSION_PRESERVE_RECENT_MESSAGES: usize = 4;
const CONTEXT_COMPRESSION_MAX_MESSAGE_CHARS: usize = 320;
const CONTEXT_COMPRESSION_MAX_MESSAGE_ENTRIES: usize = 16;
const CONTEXT_COMPRESSION_PROMPT_PREFIX: &str = "Context compression snapshot:";
static NEXT_ID_SUFFIX: AtomicU64 = AtomicU64::new(1);

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone)]
struct AppState {
    config: Arc<Mutex<GlobalConfig>>,
    config_file: PathBuf,
    model_metadata_file: PathBuf,
    user_profile_dir: PathBuf,
    terminal_registry: terminal::TerminalRegistry,
    terminal_shutdown_tx: broadcast::Sender<()>,
    mcp_registry: Arc<McpRegistry>,
    _code_graph_watchers: Arc<Vec<CodeGraphWatcher>>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Foco startup failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> AppResult<()> {
    let loaded_config = load_or_create_global_config()?;
    logging::init(&loaded_config.paths.logs_dir)?;

    tracing::info!(
        config = %loaded_config.config.to_redacted_log_json()?,
        "loaded global config"
    );

    let workspace_databases = initialize_workspace_databases(&loaded_config.config.workspaces)?;
    tracing::info!(
        count = workspace_databases.len(),
        "initialized workspace databases"
    );
    let code_graph_watchers = initialize_code_graph_indexes(&loaded_config.config.workspaces)?;
    let mcp_registry = Arc::new(McpRegistry::default());
    sync_all_mcp_workspaces(&mcp_registry, &loaded_config.config).await?;

    let addr = local_addr()?;
    let frontend_dir = frontend_dist_dir()?;
    let (terminal_shutdown_tx, _) = broadcast::channel(16);
    let state = AppState {
        config: Arc::new(Mutex::new(loaded_config.config)),
        config_file: loaded_config.paths.config_file,
        model_metadata_file: loaded_config.paths.root_dir.join("models.dev.json"),
        user_profile_dir: loaded_config.paths.user_profile_dir,
        terminal_registry: terminal::TerminalRegistry::default(),
        terminal_shutdown_tx: terminal_shutdown_tx.clone(),
        mcp_registry: mcp_registry.clone(),
        _code_graph_watchers: Arc::new(code_graph_watchers),
    };
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/workspaces", get(workspaces))
        .route("/api/workspaces/create", post(create_workspace))
        .route("/api/workspaces/add", post(add_workspace))
        .route("/api/settings", get(settings))
        .route("/api/providers/manual", post(save_manual_provider))
        .route("/api/providers/delete", post(delete_provider))
        .route("/api/providers/test", post(test_provider))
        .route("/api/model-metadata", get(model_metadata))
        .route("/api/model-metadata/refresh", post(refresh_model_metadata))
        .route("/api/models/manual", post(save_manual_model))
        .route("/api/models/delete", post(delete_model))
        .route("/api/mcp/servers/manual", post(save_mcp_server))
        .route("/api/mcp/servers/delete", post(delete_mcp_server))
        .route("/api/skills/manual", post(save_skills))
        .route("/api/skills/refresh", post(refresh_skills))
        .route(
            "/api/workspaces/{workspace_id}/chat/stream",
            post(stream_chat_response),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/messages",
            get(chat_messages),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/delete",
            post(delete_chat),
        )
        .route("/api/workspaces/{workspace_id}/git/status", get(git_status))
        .route("/api/workspaces/{workspace_id}/git/diff", get(git_diff))
        .route(
            "/api/workspaces/{workspace_id}/terminal/session",
            post(create_terminal_session),
        )
        .route(
            "/api/workspaces/{workspace_id}/terminal/{session_id}/ws",
            get(terminal_socket),
        )
        .fallback_service(ServeDir::new(frontend_dir))
        .with_state(state);
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "starting local HTTP server");
    println!("Foco is running at http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(terminal_shutdown_tx, mcp_registry))
        .await?;

    Ok(())
}

async fn sync_all_mcp_workspaces(
    registry: &Arc<McpRegistry>,
    config: &GlobalConfig,
) -> Result<(), foco_mcp::McpError> {
    for workspace in &config.workspaces {
        sync_mcp_workspace(registry, workspace, config).await?;
    }

    Ok(())
}

async fn sync_mcp_workspace(
    registry: &Arc<McpRegistry>,
    workspace: &WorkspaceConfig,
    config: &GlobalConfig,
) -> Result<(), foco_mcp::McpError> {
    let definitions = mcp_server_definitions(config)?;

    registry
        .sync_workspace_servers(&workspace.id, &workspace.path, &definitions)
        .await
}

fn mcp_server_definitions(
    config: &GlobalConfig,
) -> Result<Vec<McpServerDefinition>, foco_mcp::McpError> {
    config
        .mcp
        .servers
        .iter()
        .map(McpServerConfig::to_definition)
        .collect()
}

async fn shutdown_signal(
    terminal_shutdown_tx: broadcast::Sender<()>,
    mcp_registry: Arc<McpRegistry>,
) {
    if let Err(source) = tokio::signal::ctrl_c().await {
        tracing::warn!(error = %source, "failed to listen for Ctrl+C shutdown");
        return;
    }

    tracing::info!("shutdown requested; closing terminal sessions");
    let _ = terminal_shutdown_tx.send(());
    if let Err(error) = mcp_registry.stop_all().await {
        tracing::warn!(error = %error, "failed to stop MCP servers");
    }
}

fn initialize_code_graph_indexes(
    workspaces: &[WorkspaceConfig],
) -> AppResult<Vec<CodeGraphWatcher>> {
    let mut watchers = Vec::with_capacity(workspaces.len());

    for workspace in workspaces {
        let report = index_workspace(&workspace.path)?;
        tracing::info!(
            workspace_id = %workspace.id,
            workspace_path = %workspace.path.display(),
            scanned_files = report.scanned_files,
            indexed_files = report.indexed_files,
            unchanged_files = report.unchanged_files,
            skipped_files = report.skipped_files,
            deleted_files = report.deleted_files,
            parse_errors = report.parse_errors,
            "initialized code graph index"
        );
        let watcher = start_code_graph_watcher(&workspace.path)?;
        tracing::info!(
            workspace_id = %workspace.id,
            workspace_path = %workspace.path.display(),
            "started code graph filesystem watcher"
        );
        watchers.push(watcher);
    }

    Ok(watchers)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "foco",
        status: "ok",
    })
}

#[derive(Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
}

async fn workspaces(State(state): State<AppState>) -> Result<Json<WorkspacesResponse>, ApiError> {
    let config = config_snapshot(&state)?;

    workspace_response_from_config(&config)
}

async fn create_workspace(
    State(state): State<AppState>,
    Json(request): Json<WorkspacePathRequest>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let (name, requested_path) = validate_workspace_request(request)?;

    if requested_path.exists() {
        return Err(ApiError::bad_request(format!(
            "workspace path already exists; use add existing directory instead: {}",
            requested_path.display()
        )));
    }

    fs::create_dir_all(&requested_path).map_err(|source| {
        ApiError::internal(format!(
            "failed to create workspace directory {}: {}",
            requested_path.display(),
            source
        ))
    })?;
    let path = canonical_workspace_path(&requested_path)?;
    let mut config = config_snapshot(&state)?;

    reject_registered_workspace_path(&config, &path)?;
    WorkspaceDatabase::open_or_create(&path).map_err(ApiError::from_workspace_error)?;

    let id = unique_workspace_id(&config, &name);
    config.workspaces.push(WorkspaceConfig { id, name, path });
    save_config(&state, config.clone())?;
    sync_all_mcp_workspaces(&state.mcp_registry, &config)
        .await
        .map_err(ApiError::from_mcp_error)?;

    workspace_response_from_config(&config)
}

async fn add_workspace(
    State(state): State<AppState>,
    Json(request): Json<WorkspacePathRequest>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let (name, requested_path) = validate_workspace_request(request)?;

    if !requested_path.is_dir() {
        return Err(ApiError::bad_request(format!(
            "workspace path does not exist or is not a directory: {}",
            requested_path.display()
        )));
    }

    let path = canonical_workspace_path(&requested_path)?;
    let mut config = config_snapshot(&state)?;

    reject_registered_workspace_path(&config, &path)?;
    WorkspaceDatabase::open_or_create(&path).map_err(ApiError::from_workspace_error)?;

    let id = unique_workspace_id(&config, &name);
    config.workspaces.push(WorkspaceConfig { id, name, path });
    save_config(&state, config.clone())?;
    sync_all_mcp_workspaces(&state.mcp_registry, &config)
        .await
        .map_err(ApiError::from_mcp_error)?;

    workspace_response_from_config(&config)
}

async fn settings(State(state): State<AppState>) -> Result<Json<SettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;

    settings_response(&state, &config).await
}

async fn save_manual_provider(
    State(state): State<AppState>,
    Json(request): Json<ManualProviderRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let id = request.id.trim();
    let name = request.name.trim();
    let kind = request.kind.trim();
    let base_url = optional_trimmed_string(request.base_url);
    let existing_provider = config.providers.iter().find(|provider| provider.id == id);
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
    let provider = ProviderSettings {
        id: id.to_string(),
        name: name.to_string(),
        kind: provider_kind.as_str().to_string(),
        enabled: request.enabled,
        base_url: normalized_base_url,
        api_key,
    };

    if let Some(stored_provider) = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == id)
    {
        *stored_provider = provider;
    } else {
        config.providers.push(provider);
    }

    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

async fn delete_provider(
    State(state): State<AppState>,
    Json(request): Json<DeleteSettingsItemRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let id = request.id.trim();

    if id.is_empty() {
        return Err(ApiError::bad_request("provider id must not be empty"));
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

async fn save_mcp_server(
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

async fn delete_mcp_server(
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

async fn save_skills(
    State(state): State<AppState>,
    Json(request): Json<ManualSkillsRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let mut directories = normalize_skill_directories(request.directories)?;
    if directories.is_empty() {
        directories = default_skill_directories_for_profile(&state.user_profile_dir);
    }
    let discovery = discover_skills(&config.workspaces, &directories);
    let disabled = normalize_manual_disabled_skill_ids(
        request.disabled,
        request.enabled,
        &discovery.skills,
        &config.skills.disabled,
    )?;

    config.skills.directories = directories;
    config.skills.detected = discovery.skills;
    config.skills.disabled = disabled;
    refresh_derived_enabled_skills(&mut config);

    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

async fn refresh_skills(State(state): State<AppState>) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let discovery = discover_skills(&config.workspaces, &config.skills.directories);

    config.skills.detected = discovery.skills;
    refresh_derived_enabled_skills(&mut config);

    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

async fn test_provider(
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

async fn model_metadata(
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

async fn refresh_model_metadata(
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

async fn save_manual_model(
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

    validate_model_provider_references(&config, &provider_ids, active_provider_id.as_deref())?;

    let model = ModelSettings {
        id: model_id.to_string(),
        display_name: display_name.to_string(),
        enabled: request.enabled,
        provider_ids,
        active_provider_id,
        thinking_level,
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

    save_config(&state, config.clone())?;

    let cache = read_model_metadata_cache(&state.model_metadata_file)
        .map_err(ApiError::from_model_metadata_error)?;

    Ok(Json(model_metadata_response(
        cache,
        &config,
        &state.model_metadata_file,
    )))
}

async fn delete_model(
    State(state): State<AppState>,
    Json(request): Json<DeleteSettingsItemRequest>,
) -> Result<Json<ModelMetadataResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let id = request.id.trim();

    if id.is_empty() {
        return Err(ApiError::bad_request("model id must not be empty"));
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

async fn stream_chat_response(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<ChatStreamRequest>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let config = config_snapshot(&state)?;
    let chat_context = prepare_chat_context(&state, &config, &workspace_id, request).await?;

    Ok(Sse::new(chat_context.into_sse_stream()).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    ))
}

async fn chat_messages(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
) -> Result<Json<ChatMessagesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = workspace_id.trim();
    let chat_id = chat_id.trim();
    let workspace = config
        .workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| ApiError::bad_request(format!("workspace was not found: {workspace_id}")))?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;

    if database
        .chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
        .is_none()
    {
        return Err(ApiError::bad_request(format!(
            "chat was not found: {chat_id}"
        )));
    }

    let mut messages = Vec::new();
    for message in database
        .messages_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
    {
        if message.role != "user" && message.role != "assistant" {
            continue;
        }

        messages.push(chat_message_summary(&database, message)?);
    }

    Ok(Json(ChatMessagesResponse { messages }))
}

async fn delete_chat(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = workspace_id.trim();
    let chat_id = chat_id.trim();
    let workspace = workspace_by_id(&config, workspace_id)?;

    if chat_id.is_empty() {
        return Err(ApiError::bad_request("chat id must not be empty"));
    }

    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;

    if !database
        .delete_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
    {
        return Err(ApiError::bad_request(format!(
            "chat was not found: {chat_id}"
        )));
    }

    workspace_response_from_config(&config)
}

async fn git_status(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Json<GitStatusResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    ensure_git_workspace(&workspace.path)?;
    let status = run_git_command(&workspace.path, &["status", "--short"])?;

    Ok(Json(GitStatusResponse {
        is_git_repository: true,
        files: parse_git_status_files(&status),
        status,
    }))
}

async fn git_diff(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Query(query): Query<GitDiffQuery>,
) -> Result<Json<GitDiffResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let path = query
        .path
        .as_deref()
        .map(normalize_workspace_relative_path)
        .transpose()?;

    ensure_git_workspace(&workspace.path)?;

    let mut diff_args = vec!["diff"];
    let mut staged_diff_args = vec!["diff", "--cached"];
    if let Some(path) = path.as_deref() {
        diff_args.push("--");
        diff_args.push(path);
        staged_diff_args.push("--");
        staged_diff_args.push(path);
    }

    let status = run_git_command(&workspace.path, &["status", "--short"])?;
    let diff = run_git_command(&workspace.path, &diff_args)?;
    let staged_diff = run_git_command(&workspace.path, &staged_diff_args)?;

    Ok(Json(GitDiffResponse {
        path,
        files: parse_git_status_files(&status),
        status,
        diff,
        staged_diff,
    }))
}

async fn create_terminal_session(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Json<TerminalSessionResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let working_directory = database
        .latest_terminal_working_directory()
        .map_err(ApiError::from_workspace_error)?
        .map(|path| terminal::shell_path(Path::new(&path)).display().to_string())
        .unwrap_or_else(|| terminal::shell_path(&workspace.path).display().to_string());

    if !Path::new(&working_directory).is_dir() {
        return Err(ApiError::bad_request(format!(
            "terminal working directory does not exist: {working_directory}"
        )));
    }

    let session_id = unique_id("terminal");
    database
        .upsert_terminal_session(NewTerminalSession {
            id: &session_id,
            name: "Workspace Terminal",
            working_directory: &working_directory,
            metadata_json: None,
        })
        .map_err(ApiError::from_workspace_error)?;

    Ok(Json(TerminalSessionResponse {
        id: session_id,
        name: "Workspace Terminal".to_string(),
        working_directory,
    }))
}

async fn terminal_socket(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    AxumPath((workspace_id, session_id)): AxumPath<(String, String)>,
    Query(query): Query<TerminalSocketQuery>,
) -> Result<Response, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let session = database
        .terminal_session(&session_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!("terminal session was not found: {session_id}"))
        })?;

    if session.closed_at.is_some() {
        return Err(ApiError::bad_request(format!(
            "terminal session is closed: {session_id}"
        )));
    }

    let shutdown_rx = state.terminal_shutdown_tx.subscribe();
    let registry = state.terminal_registry.clone();
    let workspace_path = workspace.path.clone();

    Ok(ws.on_upgrade(move |socket| {
        terminal::handle_terminal_socket(
            socket,
            shutdown_rx,
            registry,
            workspace_path,
            session,
            query.cols.unwrap_or(80),
            query.rows.unwrap_or(24),
        )
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspacePathRequest {
    name: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManualModelRequest {
    model_id: String,
    display_name: String,
    enabled: bool,
    metadata_key: Option<String>,
    context_window: Option<u64>,
    max_output_tokens: Option<u64>,
    provider_ids: Option<Vec<String>>,
    active_provider_id: Option<String>,
    thinking_level: Option<String>,
    clear_thinking_level: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManualProviderRequest {
    id: String,
    name: String,
    kind: String,
    enabled: bool,
    base_url: Option<String>,
    api_key: Option<String>,
    clear_api_key: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManualMcpServerRequest {
    id: String,
    name: String,
    enabled: bool,
    transport: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManualSkillsRequest {
    directories: Vec<String>,
    disabled: Option<Vec<String>>,
    enabled: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestProviderRequest {
    provider_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteSettingsItemRequest {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatStreamRequest {
    chat_id: Option<String>,
    model_id: String,
    thinking_level: Option<String>,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitDiffQuery {
    path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalSocketQuery {
    cols: Option<u16>,
    rows: Option<u16>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsResponse {
    provider_kinds: Vec<ProviderKindSummary>,
    thinking_levels: Vec<ThinkingLevelSummary>,
    providers: Vec<ConfiguredProviderSummary>,
    configured_models: Vec<ConfiguredModelSummary>,
    mcp_transports: Vec<McpTransportSummary>,
    mcp_servers: Vec<ConfiguredMcpServerSummary>,
    skills: SkillsSettingsSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderKindSummary {
    kind: &'static str,
    label: &'static str,
    default_base_url: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ThinkingLevelSummary {
    value: &'static str,
    label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct McpTransportSummary {
    transport: &'static str,
    label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfiguredProviderSummary {
    id: String,
    name: String,
    kind: String,
    kind_label: &'static str,
    enabled: bool,
    base_url: Option<String>,
    has_api_key: bool,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfiguredMcpServerSummary {
    id: String,
    name: String,
    enabled: bool,
    transport: String,
    transport_label: &'static str,
    command: Option<String>,
    args: Vec<String>,
    url: Option<String>,
    state: String,
    error: Option<String>,
    tool_count: usize,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillsSettingsSummary {
    directories: Vec<String>,
    detected: Vec<ConfiguredSkillSummary>,
    errors: Vec<SkillDiscoveryErrorSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfiguredSkillSummary {
    id: String,
    name: String,
    description: String,
    path: String,
    enabled: bool,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillDiscoveryErrorSummary {
    path: String,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderTestResponse {
    ok: bool,
    message: String,
    model_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspacesResponse {
    active_workspace_id: String,
    workspaces: Vec<WorkspaceSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelMetadataResponse {
    source_url: Option<String>,
    fetched_at: Option<String>,
    cache_path: String,
    models: Vec<ModelMetadataRecord>,
    configured_models: Vec<ConfiguredModelSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfiguredModelSummary {
    id: String,
    display_name: String,
    enabled: bool,
    metadata_key: Option<String>,
    metadata_source_url: Option<String>,
    metadata_refreshed_at: Option<String>,
    context_window: Option<u64>,
    max_output_tokens: Option<u64>,
    can_enable: bool,
    missing_limits: Vec<&'static str>,
    provider_ids: Vec<String>,
    active_provider_id: Option<String>,
    thinking_level: Option<String>,
    supports_thinking: bool,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSummary {
    id: String,
    name: String,
    path: String,
    chats: Vec<ChatSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatSummary {
    id: String,
    title: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessagesResponse {
    messages: Vec<ChatMessageSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GitStatusResponse {
    is_git_repository: bool,
    status: String,
    files: Vec<GitStatusFileSummary>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitStatusFileSummary {
    path: String,
    index_status: String,
    worktree_status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GitDiffResponse {
    path: Option<String>,
    status: String,
    diff: String,
    staged_diff: String,
    files: Vec<GitStatusFileSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalSessionResponse {
    id: String,
    name: String,
    working_directory: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessageSummary {
    id: String,
    role: String,
    content: String,
    reasoning: Option<String>,
    tool_calls: Vec<ChatToolCallSummary>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatToolCallSummary {
    id: String,
    name: String,
    status: String,
    input: Value,
    output: Option<Value>,
    is_error: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
enum ChatSseEvent {
    Start {
        chat_id: String,
        user_message_id: String,
        assistant_message_id: String,
        llm_request_id: String,
    },
    TextDelta {
        assistant_message_id: String,
        delta: String,
    },
    ReasoningDelta {
        assistant_message_id: String,
        delta: String,
    },
    ToolCall {
        assistant_message_id: String,
        tool_call: ChatToolCallSummary,
    },
    ToolResult {
        assistant_message_id: String,
        tool_call_id: String,
        output: Value,
        is_error: bool,
    },
    GitDiffRefresh {
        workspace_id: String,
    },
    Usage {
        usage: NeutralUsage,
    },
    Complete {
        chat_id: String,
        assistant_message_id: String,
        text: String,
        reasoning: Option<String>,
        usage: Option<NeutralUsage>,
        stop_reason: Option<String>,
    },
    Error {
        message: String,
    },
}

struct PreparedChatContext {
    workspace_id: String,
    workspace_path: PathBuf,
    chat_id: String,
    provider_id: String,
    model_id: String,
    user_message_id: String,
    assistant_message_id: String,
    llm_request_id: String,
    assistant_sequence: i64,
    provider_config: ProviderConnectionConfig,
    provider_request: NeutralChatRequest,
    mcp_registry: Arc<McpRegistry>,
    context_budget: foco_agent::ContextBudget,
    request_body_json: String,
    compression_snapshots: Vec<ContextCompressionSnapshotRecord>,
    message_source_sequences: Vec<Option<i64>>,
    active_tool_start_index: usize,
}

struct CapturedAuditEvent {
    event_at: String,
    event_type: String,
    normalized_event_json: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutedToolCall {
    id: String,
    name: String,
    input: Value,
    output: Value,
    is_error: bool,
    started_at: String,
    completed_at: String,
}

struct ChatAuditOutcome {
    first_token_at: Option<String>,
    completed_at: String,
    first_token_latency_ms: Option<i64>,
    total_latency_ms: i64,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
    final_state: &'static str,
    response_body_json: Option<String>,
}

impl PreparedChatContext {
    fn into_sse_stream(mut self) -> impl futures_util::Stream<Item = Result<Event, Infallible>> {
        async_stream::stream! {
            let request_started_at = utc_timestamp();
            let started_at = Instant::now();
            let start_event = ChatSseEvent::Start {
                chat_id: self.chat_id.clone(),
                user_message_id: self.user_message_id.clone(),
                assistant_message_id: self.assistant_message_id.clone(),
                llm_request_id: self.llm_request_id.clone(),
            };
            let mut events = vec![captured_event(&start_event)];
            let mut assistant_text = String::new();
            let mut assistant_reasoning = String::new();
            let mut first_token_at = None;
            let mut first_token_latency_ms = None;
            let mut seen_tool_call_ids = HashSet::new();
            let mut executed_tool_calls = Vec::new();
            let mut provider_completions = Vec::new();
            let mut total_usage = NeutralUsage::default();
            let mut final_usage = None;

            yield Ok(sse_event(&start_event));

            for turn_index in 0..=MAX_AGENT_TOOL_ROUNDS {
                let turn_active_tool_start_index = match ensure_context_compression(&mut self) {
                    Ok(index) => index,
                    Err(error) => {
                        let message = error.message;
                        let event = ChatSseEvent::Error {
                            message: message.clone(),
                        };
                        events.push(captured_event(&event));
                        let outcome = failed_audit_outcome(started_at, &message);

                        if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &[]) {
                            let event = ChatSseEvent::Error {
                                message: persist_error.message,
                            };
                            yield Ok(sse_event(&event));
                        } else {
                            yield Ok(sse_event(&event));
                        }

                        return;
                    }
                };
                let packed_messages = match pack_neutral_messages(
                    self.provider_request.messages.clone(),
                    &self.message_source_sequences,
                    &self.context_budget,
                    turn_active_tool_start_index,
                ) {
                    Ok(messages) => messages,
                    Err(error) => {
                        let message = error.message;
                        let event = ChatSseEvent::Error {
                            message: message.clone(),
                        };
                        events.push(captured_event(&event));
                        let outcome = failed_audit_outcome(started_at, &message);

                        if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &[]) {
                            let event = ChatSseEvent::Error {
                                message: persist_error.message,
                            };
                            yield Ok(sse_event(&event));
                        } else {
                            yield Ok(sse_event(&event));
                        }

                        return;
                    }
                };
                let mut turn_request = self.provider_request.clone();
                turn_request.messages = packed_messages;
                match serialize_provider_request(&turn_request) {
                    Ok(request_body_json) => {
                        self.request_body_json = request_body_json;
                    }
                    Err(error) => {
                        let message = error.message;
                        let event = ChatSseEvent::Error {
                            message: message.clone(),
                        };
                        events.push(captured_event(&event));
                        let outcome = failed_audit_outcome(started_at, &message);

                        if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &[]) {
                            let event = ChatSseEvent::Error {
                                message: persist_error.message,
                            };
                            yield Ok(sse_event(&event));
                        } else {
                            yield Ok(sse_event(&event));
                        }

                        return;
                    }
                }
                let mut provider_stream = match stream_chat(&self.provider_config, turn_request).await {
                    Ok(provider_stream) => provider_stream,
                    Err(error) => {
                        let message = error.to_string();
                        let event = ChatSseEvent::Error {
                            message: message.clone(),
                        };
                        events.push(captured_event(&event));
                        let outcome = failed_audit_outcome(started_at, &message);

                        if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &[]) {
                            let event = ChatSseEvent::Error {
                                message: persist_error.message,
                            };
                            yield Ok(sse_event(&event));
                        } else {
                            yield Ok(sse_event(&event));
                        }

                        return;
                    }
                };
                let mut turn_text = String::new();
                let mut turn_reasoning = String::new();
                let mut completed_turn = false;

                while let Some(event_result) = provider_stream.next_event().await {
                    let provider_event = match event_result {
                        Ok(provider_event) => provider_event,
                        Err(error) => {
                            let message = error.to_string();
                            let event = ChatSseEvent::Error {
                                message: message.clone(),
                            };
                            events.push(captured_event(&event));
                            let outcome = failed_audit_outcome(started_at, &message);

                            if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &[]) {
                                let event = ChatSseEvent::Error {
                                    message: persist_error.message,
                                };
                                yield Ok(sse_event(&event));
                            } else {
                                yield Ok(sse_event(&event));
                            }

                            return;
                        }
                    };

                    events.push(captured_provider_event(&provider_event));

                    match provider_event {
                        NeutralChatStreamEvent::Start => {}
                        NeutralChatStreamEvent::TextDelta { delta } => {
                            capture_first_token(started_at, &mut first_token_at, &mut first_token_latency_ms);
                            assistant_text.push_str(&delta);
                            turn_text.push_str(&delta);
                            let event = ChatSseEvent::TextDelta {
                                assistant_message_id: self.assistant_message_id.clone(),
                                delta,
                            };
                            yield Ok(sse_event(&event));
                        }
                        NeutralChatStreamEvent::ReasoningDelta { delta } => {
                            capture_first_token(started_at, &mut first_token_at, &mut first_token_latency_ms);
                            assistant_reasoning.push_str(&delta);
                            turn_reasoning.push_str(&delta);
                            let event = ChatSseEvent::ReasoningDelta {
                                assistant_message_id: self.assistant_message_id.clone(),
                                delta,
                            };
                            yield Ok(sse_event(&event));
                        }
                        NeutralChatStreamEvent::ThoughtSignatureDelta { delta: _ } => {
                            capture_first_token(started_at, &mut first_token_at, &mut first_token_latency_ms);
                        }
                        NeutralChatStreamEvent::ToolCall { tool_call } => {
                            capture_first_token(started_at, &mut first_token_at, &mut first_token_latency_ms);
                            if seen_tool_call_ids.insert(tool_call.call_id.clone()) {
                                let event = ChatSseEvent::ToolCall {
                                    assistant_message_id: self.assistant_message_id.clone(),
                                    tool_call: pending_tool_call_summary(&tool_call),
                                };
                                events.push(captured_event(&event));
                                yield Ok(sse_event(&event));
                            }
                        }
                        NeutralChatStreamEvent::Usage { usage } => {
                            let event = ChatSseEvent::Usage { usage };
                            yield Ok(sse_event(&event));
                        }
                        NeutralChatStreamEvent::Complete {
                            text,
                            reasoning,
                            tool_calls,
                            usage,
                            stop_reason,
                            response_id,
                        } => {
                            completed_turn = true;

                            if turn_text.is_empty() && !text.is_empty() {
                                let message = "provider completed without streaming assistant text deltas".to_string();
                                let event = ChatSseEvent::Error {
                                    message: message.clone(),
                                };
                                events.push(captured_event(&event));
                                let outcome = failed_audit_outcome(started_at, &message);

                                if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &[]) {
                                    let event = ChatSseEvent::Error {
                                        message: persist_error.message,
                                    };
                                    yield Ok(sse_event(&event));
                                } else {
                                    yield Ok(sse_event(&event));
                                }

                                return;
                            }

                            if turn_text.is_empty() && tool_calls.is_empty() {
                                let message = "provider completed without assistant text or tool calls".to_string();
                                let event = ChatSseEvent::Error {
                                    message: message.clone(),
                                };
                                events.push(captured_event(&event));
                                let outcome = failed_audit_outcome(started_at, &message);

                                if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &[]) {
                                    let event = ChatSseEvent::Error {
                                        message: persist_error.message,
                                    };
                                    yield Ok(sse_event(&event));
                                } else {
                                    yield Ok(sse_event(&event));
                                }

                                return;
                            }

                            provider_completions.push(json!({
                                "turnIndex": turn_index,
                                "text": text.clone(),
                                "reasoning": reasoning.clone(),
                                "toolCalls": tool_calls.clone(),
                                "usage": usage.clone(),
                                "stopReason": stop_reason.clone(),
                                "responseId": response_id.clone()
                            }));
                            if turn_reasoning.is_empty() {
                                if let Some(reasoning) = reasoning.as_deref() {
                                    assistant_reasoning.push_str(reasoning);
                                    turn_reasoning.push_str(reasoning);
                                }
                            }

                            if let Some(usage) = &usage {
                                merge_usage(&mut total_usage, usage);
                                final_usage = Some(total_usage.clone());
                            }

                            if tool_calls.is_empty() {
                                let assistant_message_text =
                                    assistant_message_text(&assistant_text, &executed_tool_calls);
                                let complete_event = ChatSseEvent::Complete {
                                    chat_id: self.chat_id.clone(),
                                    assistant_message_id: self.assistant_message_id.clone(),
                                    text: assistant_message_text.clone(),
                                    reasoning: non_empty_string(&assistant_reasoning),
                                    usage: final_usage.clone(),
                                    stop_reason: stop_reason.clone(),
                                };
                                events.push(captured_event(&complete_event));
                                let completed_at = utc_timestamp();
                                let outcome = ChatAuditOutcome {
                                    first_token_at,
                                    completed_at,
                                    first_token_latency_ms,
                                    total_latency_ms: elapsed_millis(started_at),
                                    input_tokens: final_usage.as_ref().and_then(|usage| usage.input_tokens),
                                    output_tokens: final_usage.as_ref().and_then(|usage| usage.output_tokens),
                                    cache_read_tokens: final_usage.as_ref().and_then(|usage| usage.cache_read_tokens),
                                    cache_write_tokens: final_usage.as_ref().and_then(|usage| usage.cache_write_tokens),
                                    final_state: "succeeded",
                                    response_body_json: Some(json!({
                                        "text": assistant_message_text.clone(),
                                        "reasoning": non_empty_string(&assistant_reasoning),
                                        "providerCompletions": provider_completions.clone(),
                                        "toolCalls": executed_tool_calls.clone(),
                                        "usage": final_usage.clone(),
                                        "stopReason": stop_reason.clone()
                                    }).to_string()),
                                };

                                match persist_chat_result(&self, &request_started_at, outcome, &events, Some(&assistant_message_text), non_empty_string(&assistant_reasoning).as_deref(), &executed_tool_calls) {
                                    Ok(()) => {
                                        yield Ok(sse_event(&complete_event));
                                    }
                                    Err(error) => {
                                        let event = ChatSseEvent::Error {
                                            message: error.message,
                                        };
                                        yield Ok(sse_event(&event));
                                    }
                                }

                                return;
                            }

                            if turn_index >= MAX_AGENT_TOOL_ROUNDS {
                                let message = format!(
                                    "agent run exceeded {MAX_AGENT_TOOL_ROUNDS} tool continuation rounds"
                                );
                                let event = ChatSseEvent::Error {
                                    message: message.clone(),
                                };
                                events.push(captured_event(&event));
                                let outcome = failed_audit_outcome(started_at, &message);

                                if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &executed_tool_calls) {
                                    let event = ChatSseEvent::Error {
                                        message: persist_error.message,
                                    };
                                    yield Ok(sse_event(&event));
                                } else {
                                    yield Ok(sse_event(&event));
                                }

                                return;
                            }

                            let pending_tool_calls = pending_tool_calls(&tool_calls);
                            if let Err(error) = detect_same_file_write_conflicts(&pending_tool_calls) {
                                let message = error.to_string();
                                let event = ChatSseEvent::Error {
                                    message: message.clone(),
                                };
                                events.push(captured_event(&event));
                                let outcome = failed_audit_outcome(started_at, &message);

                                if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &executed_tool_calls) {
                                    let event = ChatSseEvent::Error {
                                        message: persist_error.message,
                                    };
                                    yield Ok(sse_event(&event));
                                } else {
                                    yield Ok(sse_event(&event));
                                }

                                return;
                            }

                            for tool_call in &tool_calls {
                                capture_first_token(started_at, &mut first_token_at, &mut first_token_latency_ms);
                                if seen_tool_call_ids.insert(tool_call.call_id.clone()) {
                                    let event = ChatSseEvent::ToolCall {
                                        assistant_message_id: self.assistant_message_id.clone(),
                                        tool_call: pending_tool_call_summary(tool_call),
                                    };
                                    events.push(captured_event(&event));
                                    yield Ok(sse_event(&event));
                                }
                            }

                            let next_tool_results = match execute_tool_calls_parallel(
                                self.mcp_registry.clone(),
                                &self.workspace_id,
                                &self.workspace_path,
                                tool_calls.clone(),
                            ).await {
                                Ok(tool_results) => tool_results,
                                Err(error) => {
                                    let message = error.message;
                                    let event = ChatSseEvent::Error {
                                        message: message.clone(),
                                    };
                                    events.push(captured_event(&event));
                                    let outcome = failed_audit_outcome(started_at, &message);

                                    if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &executed_tool_calls) {
                                        let event = ChatSseEvent::Error {
                                            message: persist_error.message,
                                        };
                                        yield Ok(sse_event(&event));
                                    } else {
                                        yield Ok(sse_event(&event));
                                    }

                                    return;
                                }
                            };
                            for executed_tool_call in &next_tool_results {
                                let result_event = ChatSseEvent::ToolResult {
                                    assistant_message_id: self.assistant_message_id.clone(),
                                    tool_call_id: executed_tool_call.id.clone(),
                                    output: executed_tool_call.output.clone(),
                                    is_error: executed_tool_call.is_error,
                                };
                                events.push(captured_event(&result_event));
                                yield Ok(sse_event(&result_event));
                            }
                            if tool_results_affect_git_diff(&next_tool_results) {
                                let event = ChatSseEvent::GitDiffRefresh {
                                    workspace_id: self.workspace_id.clone(),
                                };
                                events.push(captured_event(&event));
                                yield Ok(sse_event(&event));
                            }

                            append_tool_state_messages(
                                &mut self.provider_request.messages,
                                &mut self.message_source_sequences,
                                tool_calls,
                                &next_tool_results,
                                turn_text,
                                non_empty_string(&turn_reasoning),
                            );
                            executed_tool_calls.extend(next_tool_results);

                            break;
                        }
                        NeutralChatStreamEvent::Error { message } => {
                            let event = ChatSseEvent::Error {
                                message: message.clone(),
                            };
                            events.push(captured_event(&event));
                            let outcome = failed_audit_outcome(started_at, &message);

                            if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &executed_tool_calls) {
                                let event = ChatSseEvent::Error {
                                    message: persist_error.message,
                                };
                                yield Ok(sse_event(&event));
                            } else {
                                yield Ok(sse_event(&event));
                            }

                            return;
                        }
                    }
                }

                if completed_turn {
                    continue;
                }

                let message = "provider stream ended without a completion event".to_string();
                let event = ChatSseEvent::Error {
                    message: message.clone(),
                };
                events.push(captured_event(&event));
                let outcome = failed_audit_outcome(started_at, &message);

                if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, None, &executed_tool_calls) {
                    let event = ChatSseEvent::Error {
                        message: persist_error.message,
                    };
                    yield Ok(sse_event(&event));
                } else {
                    yield Ok(sse_event(&event));
                }

                return;
            }
        }
    }
}

async fn prepare_chat_context(
    state: &AppState,
    config: &GlobalConfig,
    workspace_id: &str,
    request: ChatStreamRequest,
) -> Result<PreparedChatContext, ApiError> {
    let workspace_id = workspace_id.trim();
    let message = request.message.trim();
    let model_id = request.model_id.trim();
    let thinking_level = optional_trimmed_string(request.thinking_level);

    if workspace_id.is_empty() {
        return Err(ApiError::bad_request("workspace id must not be empty"));
    }

    if message.is_empty() {
        return Err(ApiError::bad_request("message must not be empty"));
    }

    if model_id.is_empty() {
        return Err(ApiError::bad_request("model id must not be empty"));
    }

    let workspace = config
        .workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| ApiError::bad_request(format!("workspace was not found: {workspace_id}")))?;
    let model = config
        .models
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| ApiError::bad_request(format!("model was not found: {model_id}")))?;

    if !model.enabled {
        return Err(ApiError::bad_request(format!(
            "model '{}' is disabled",
            model.id
        )));
    }

    let limits = model.limits.as_ref().ok_or_else(|| {
        ApiError::bad_request(format!("enabled model '{}' is missing limits", model.id))
    })?;
    let max_output_tokens = u32::try_from(limits.max_output_tokens).map_err(|_| {
        ApiError::bad_request(format!(
            "model '{}' max output tokens exceed u32: {}",
            model.id, limits.max_output_tokens
        ))
    })?;
    let active_provider_id = model.active_provider_id.as_deref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "model '{}' has no active provider selected",
            model.id
        ))
    })?;

    if !model
        .provider_ids
        .iter()
        .any(|provider_id| provider_id == active_provider_id)
    {
        return Err(ApiError::bad_request(format!(
            "active provider '{}' is not associated with model '{}'",
            active_provider_id, model.id
        )));
    }

    let provider = config
        .providers
        .iter()
        .find(|provider| provider.id == active_provider_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "active provider '{}' was not found",
                active_provider_id
            ))
        })?;

    if !provider.enabled {
        return Err(ApiError::bad_request(format!(
            "provider '{}' is disabled",
            provider.id
        )));
    }

    let provider_config = provider_connection_config(provider)?;
    sync_mcp_workspace(&state.mcp_registry, workspace, config)
        .await
        .map_err(ApiError::from_mcp_error)?;
    let mcp_tools = state.mcp_registry.tool_definitions(&workspace.id).await;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let chat_id = optional_trimmed_string(request.chat_id);
    let chat_id = match chat_id {
        Some(chat_id) => {
            if database
                .chat(&chat_id)
                .map_err(ApiError::from_workspace_error)?
                .is_none()
            {
                return Err(ApiError::bad_request(format!(
                    "chat was not found: {chat_id}"
                )));
            }
            chat_id
        }
        None => {
            let chat_id = unique_id("chat");
            database
                .insert_chat(&chat_id, &chat_title(message))
                .map_err(ApiError::from_workspace_error)?;
            chat_id
        }
    };
    let existing_messages = database
        .messages_for_chat(&chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let compression_snapshots = database
        .context_compression_snapshots_for_chat(&chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let user_sequence = next_message_sequence(&existing_messages);
    let assistant_sequence = user_sequence + 1;
    let user_message_id = unique_id("msg-user");
    let assistant_message_id = unique_id("msg-assistant");
    let llm_request_id = unique_id("llm");

    database
        .insert_message(NewMessage {
            id: &user_message_id,
            chat_id: &chat_id,
            role: "user",
            content: message,
            sequence: user_sequence,
            metadata_json: Some("{}"),
        })
        .map_err(ApiError::from_workspace_error)?;

    let builtin_tool_definitions = builtin_tool_definitions();
    let mut neutral_tools = builtin_tool_definitions
        .iter()
        .cloned()
        .map(neutral_tool_definition)
        .collect::<Vec<_>>();
    neutral_tools.extend(mcp_tools.iter().map(neutral_mcp_tool_definition));
    let code_graph_context = database
        .code_graph_context()
        .map_err(ApiError::from_workspace_error)?;
    let tool_prompt_infos = builtin_tool_definitions
        .iter()
        .map(|tool| ToolPromptInfo {
            name: tool.name.to_string(),
            description: tool.description.to_string(),
        })
        .chain(mcp_tools.iter().map(|tool| ToolPromptInfo {
            name: tool.name.clone(),
            description: format!(
                "{} MCP server '{}': {}",
                tool.original_name, tool.server_name, tool.description
            ),
        }))
        .collect();
    let system_prompt = build_system_prompt(SystemPromptInput {
        workspace_id: workspace.id.clone(),
        workspace_name: workspace.name.clone(),
        workspace_path: workspace.path.display().to_string(),
        code_graph: CodeGraphPromptContext {
            indexed_files: code_graph_context.indexed_files,
            symbols: code_graph_context.symbols,
            references: code_graph_context.references,
            edges: code_graph_context.edges,
            languages: code_graph_context.languages,
        },
        skills: enabled_skill_prompts(&config)?,
        tools: tool_prompt_infos,
    });
    let context_budget = calculate_context_budget(
        limits.context_window,
        limits.max_output_tokens,
        estimate_text_tokens(&system_prompt),
        estimate_tool_schema_tokens(&neutral_tools),
    )
    .map_err(|source| ApiError::bad_request(source.to_string()))?;

    let covered_sequences = snapshot_covered_sequences(&compression_snapshots);
    let mut neutral_messages =
        Vec::with_capacity(existing_messages.len() + compression_snapshots.len() + 2);
    let mut message_source_sequences = Vec::with_capacity(neutral_messages.capacity());
    neutral_messages.push(neutral_text_message(NeutralChatRole::System, system_prompt));
    message_source_sequences.push(None);
    for snapshot in &compression_snapshots {
        neutral_messages.push(compression_snapshot_message(snapshot));
        message_source_sequences.push(None);
    }
    for existing_message in existing_messages {
        if covered_sequences.contains(&existing_message.sequence) {
            continue;
        }

        let sequence = existing_message.sequence;
        neutral_messages.push(neutral_message_from_record(existing_message)?);
        message_source_sequences.push(Some(sequence));
    }
    neutral_messages.push(neutral_text_message(
        NeutralChatRole::User,
        message.to_string(),
    ));
    message_source_sequences.push(Some(user_sequence));
    let active_tool_start_index = neutral_messages.len();

    let provider_request = NeutralChatRequest {
        model_id: model.id.clone(),
        messages: neutral_messages,
        tools: neutral_tools,
        thinking_level: thinking_level.or_else(|| model.thinking_level.clone()),
        max_output_tokens: Some(max_output_tokens),
    };
    let request_body_json = serde_json::to_string(&provider_request).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize provider-neutral chat request: {source}"
        ))
    })?;

    Ok(PreparedChatContext {
        workspace_id: workspace.id.clone(),
        workspace_path: workspace.path.clone(),
        chat_id,
        provider_id: provider.id.clone(),
        model_id: model.id.clone(),
        user_message_id,
        assistant_message_id,
        llm_request_id,
        assistant_sequence,
        provider_config,
        provider_request,
        mcp_registry: state.mcp_registry.clone(),
        context_budget,
        request_body_json,
        compression_snapshots,
        message_source_sequences,
        active_tool_start_index,
    })
}

fn neutral_message_from_record(message: MessageRecord) -> Result<NeutralChatMessage, ApiError> {
    let role = match message.role.as_str() {
        "system" => NeutralChatRole::System,
        "user" => NeutralChatRole::User,
        "assistant" => NeutralChatRole::Assistant,
        "tool" => NeutralChatRole::Tool,
        other => {
            return Err(ApiError::bad_request(format!(
                "chat contains unsupported message role '{other}'"
            )));
        }
    };

    if role != NeutralChatRole::Tool {
        let reasoning = if role == NeutralChatRole::Assistant {
            assistant_reasoning_from_metadata(&message.metadata_json)?
        } else {
            None
        };

        return Ok(NeutralChatMessage {
            role,
            content: message.content,
            reasoning,
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
        });
    }

    let metadata = parse_json_value(&message.metadata_json, "tool message metadata")?;
    let tool_call_id = metadata
        .get("toolCallId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "tool message '{}' is missing metadata.toolCallId",
                message.id
            ))
        })?;
    let tool_name = metadata
        .get("toolName")
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(NeutralChatMessage {
        role,
        content: message.content,
        reasoning: None,
        tool_calls: Vec::new(),
        tool_call_id: Some(tool_call_id),
        tool_name,
    })
}

fn ensure_context_compression(context: &mut PreparedChatContext) -> Result<usize, ApiError> {
    if context.provider_request.messages.len() != context.message_source_sequences.len() {
        return Err(ApiError::internal(
            "context message source sequence count does not match prompt message count",
        ));
    }

    let latest_user_index = context
        .provider_request
        .messages
        .iter()
        .rposition(|message| message.role == NeutralChatRole::User);
    let pack_items = context
        .provider_request
        .messages
        .iter()
        .enumerate()
        .map(|(index, message)| ContextPackItem {
            id: format!("message-{index}"),
            estimated_tokens: if index == 0 {
                0
            } else {
                neutral_message_estimated_tokens(message)
            },
            must_keep: message.role == NeutralChatRole::System
                || context.message_source_sequences[index].is_none()
                || Some(index) == latest_user_index
                || index >= context.active_tool_start_index,
        })
        .collect::<Vec<_>>();

    let Some(plan) = plan_context_compression(
        &pack_items,
        context.context_budget.available_message_tokens,
        context.active_tool_start_index,
        CONTEXT_COMPRESSION_PRESERVE_RECENT_MESSAGES,
    ) else {
        return Ok(context.active_tool_start_index);
    };

    let summary = context_compression_summary(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &plan.covered_indices,
    )?;
    let summary_token_count = estimate_text_tokens(&summary);

    if summary_token_count >= plan.original_tokens {
        return Ok(context.active_tool_start_index);
    }

    let covered_sequences =
        compression_covered_sequences(&context.message_source_sequences, &plan.covered_indices)?;
    let snapshot_id = unique_id("ctx");
    let snapshot_sequence = next_context_snapshot_sequence(&context.compression_snapshots)?;
    let metadata_json = json!({
        "coveredSequences": covered_sequences,
        "triggerTokens": plan.trigger_tokens,
        "availableMessageTokens": context.context_budget.available_message_tokens
    })
    .to_string();
    let original_token_count = i64::try_from(plan.original_tokens)
        .map_err(|_| ApiError::internal("context compression original token count exceeds i64"))?;
    let summary_token_count_i64 = i64::try_from(summary_token_count)
        .map_err(|_| ApiError::internal("context compression summary token count exceeds i64"))?;
    let source_message_start_sequence = covered_sequences
        .first()
        .copied()
        .ok_or_else(|| ApiError::internal("context compression has no source message sequence"))?;
    let source_message_end_sequence = covered_sequences
        .last()
        .copied()
        .ok_or_else(|| ApiError::internal("context compression has no source message sequence"))?;

    let mut database = WorkspaceDatabase::open_or_create(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    database
        .insert_context_compression_snapshot(NewContextCompressionSnapshot {
            id: &snapshot_id,
            chat_id: &context.chat_id,
            run_id: &context.llm_request_id,
            sequence: snapshot_sequence,
            summary: &summary,
            source_message_start_sequence,
            source_message_end_sequence,
            original_token_count,
            summary_token_count: summary_token_count_i64,
            metadata_json: Some(&metadata_json),
        })
        .map_err(ApiError::from_workspace_error)?;

    let created_at = utc_timestamp();
    let snapshot = ContextCompressionSnapshotRecord {
        id: snapshot_id,
        chat_id: context.chat_id.clone(),
        run_id: context.llm_request_id.clone(),
        sequence: snapshot_sequence,
        summary: summary.clone(),
        source_message_start_sequence,
        source_message_end_sequence,
        original_token_count,
        summary_token_count: summary_token_count_i64,
        created_at,
        metadata_json,
    };

    context.provider_request.messages = replace_covered_messages_with_snapshot(
        &context.provider_request.messages,
        &plan.covered_indices,
        compression_snapshot_message(&snapshot),
    );
    context.message_source_sequences = replace_covered_sequences_with_snapshot(
        &context.message_source_sequences,
        &plan.covered_indices,
    );
    context.active_tool_start_index =
        compressed_active_tool_start_index(context.active_tool_start_index, &plan.covered_indices);
    context.compression_snapshots.push(snapshot);

    Ok(context.active_tool_start_index)
}

fn snapshot_covered_sequences(snapshots: &[ContextCompressionSnapshotRecord]) -> HashSet<i64> {
    let mut sequences = HashSet::new();

    for snapshot in snapshots {
        if let Ok(metadata) = serde_json::from_str::<Value>(&snapshot.metadata_json) {
            if let Some(covered_sequences) =
                metadata.get("coveredSequences").and_then(Value::as_array)
            {
                for sequence in covered_sequences.iter().filter_map(Value::as_i64) {
                    sequences.insert(sequence);
                }
                continue;
            }
        }

        for sequence in
            snapshot.source_message_start_sequence..=snapshot.source_message_end_sequence
        {
            sequences.insert(sequence);
        }
    }

    sequences
}

fn compression_snapshot_message(snapshot: &ContextCompressionSnapshotRecord) -> NeutralChatMessage {
    neutral_text_message(
        NeutralChatRole::System,
        format!(
            "{CONTEXT_COMPRESSION_PROMPT_PREFIX}\n\
             - snapshot id: {}\n\
             - source message sequence range: {}..={}\n\
             - original tokens: {}\n\
             - summary tokens: {}\n\n{}",
            snapshot.id,
            snapshot.source_message_start_sequence,
            snapshot.source_message_end_sequence,
            snapshot.original_token_count,
            snapshot.summary_token_count,
            snapshot.summary
        ),
    )
}

fn context_compression_summary(
    messages: &[NeutralChatMessage],
    message_source_sequences: &[Option<i64>],
    covered_indices: &[usize],
) -> Result<String, ApiError> {
    if messages.len() != message_source_sequences.len() {
        return Err(ApiError::internal(
            "context message source sequence count does not match prompt message count",
        ));
    }

    let mut lines = vec![
        "Structured summary of earlier chat messages that were removed from the live prompt."
            .to_string(),
    ];

    for index in covered_indices
        .iter()
        .copied()
        .take(CONTEXT_COMPRESSION_MAX_MESSAGE_ENTRIES)
    {
        let message = messages.get(index).ok_or_else(|| {
            ApiError::internal("context compression covered message index is out of bounds")
        })?;
        let sequence = message_source_sequences
            .get(index)
            .and_then(|sequence| *sequence)
            .ok_or_else(|| {
                ApiError::internal(
                    "context compression can only cover messages with database sequences",
                )
            })?;

        lines.push(format!(
            "- sequence {sequence}, role {}: {}",
            neutral_role_label(&message.role),
            compact_message_for_compression(message)
        ));
    }

    if covered_indices.len() > CONTEXT_COMPRESSION_MAX_MESSAGE_ENTRIES {
        lines.push(format!(
            "- {} additional older messages were omitted from this snapshot.",
            covered_indices.len() - CONTEXT_COMPRESSION_MAX_MESSAGE_ENTRIES
        ));
    }

    Ok(lines.join("\n"))
}

fn compact_message_for_compression(message: &NeutralChatMessage) -> String {
    let mut content = truncate_for_context_snapshot(&message.content);

    if !message.tool_calls.is_empty() {
        let names = message
            .tool_calls
            .iter()
            .map(|tool_call| tool_call.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if content.is_empty() {
            content = format!("tool calls: {names}");
        } else {
            content.push_str("; tool calls: ");
            content.push_str(&names);
        }
    }

    if let Some(tool_name) = message.tool_name.as_deref() {
        if content.is_empty() {
            content = format!("tool result for {tool_name}");
        } else {
            content.push_str("; tool result for ");
            content.push_str(tool_name);
        }
    }

    if content.is_empty() {
        "(empty message content)".to_string()
    } else {
        content
    }
}

fn truncate_for_context_snapshot(value: &str) -> String {
    let trimmed = value.trim();
    let mut output = String::new();

    for (index, character) in trimmed.chars().enumerate() {
        if index >= CONTEXT_COMPRESSION_MAX_MESSAGE_CHARS {
            output.push_str("...");
            return output;
        }

        if character.is_control() && character != '\n' && character != '\t' {
            output.push(' ');
        } else {
            output.push(character);
        }
    }

    output
}

fn compression_covered_sequences(
    message_source_sequences: &[Option<i64>],
    covered_indices: &[usize],
) -> Result<Vec<i64>, ApiError> {
    let mut sequences = Vec::with_capacity(covered_indices.len());

    for index in covered_indices {
        let sequence = message_source_sequences
            .get(*index)
            .and_then(|sequence| *sequence)
            .ok_or_else(|| {
                ApiError::internal(
                    "context compression can only cover messages with database sequences",
                )
            })?;
        sequences.push(sequence);
    }

    Ok(sequences)
}

fn replace_covered_messages_with_snapshot(
    messages: &[NeutralChatMessage],
    covered_indices: &[usize],
    snapshot_message: NeutralChatMessage,
) -> Vec<NeutralChatMessage> {
    let covered = covered_indices.iter().copied().collect::<HashSet<_>>();
    let first_covered = covered_indices.first().copied();
    let mut next_messages = Vec::with_capacity(messages.len() - covered.len() + 1);

    for (index, message) in messages.iter().enumerate() {
        if Some(index) == first_covered {
            next_messages.push(snapshot_message.clone());
        }

        if covered.contains(&index) {
            continue;
        }

        next_messages.push(message.clone());
    }

    next_messages
}

fn replace_covered_sequences_with_snapshot(
    message_source_sequences: &[Option<i64>],
    covered_indices: &[usize],
) -> Vec<Option<i64>> {
    let covered = covered_indices.iter().copied().collect::<HashSet<_>>();
    let first_covered = covered_indices.first().copied();
    let mut next_sequences = Vec::with_capacity(message_source_sequences.len() - covered.len() + 1);

    for (index, sequence) in message_source_sequences.iter().enumerate() {
        if Some(index) == first_covered {
            next_sequences.push(None);
        }

        if covered.contains(&index) {
            continue;
        }

        next_sequences.push(*sequence);
    }

    next_sequences
}

fn compressed_active_tool_start_index(
    active_tool_start_index: usize,
    covered_indices: &[usize],
) -> usize {
    let removed_before_active_tool = covered_indices
        .iter()
        .filter(|index| **index < active_tool_start_index)
        .count();

    active_tool_start_index - removed_before_active_tool + 1
}

fn next_context_snapshot_sequence(
    snapshots: &[ContextCompressionSnapshotRecord],
) -> Result<i64, ApiError> {
    let next = snapshots
        .iter()
        .map(|snapshot| snapshot.sequence)
        .max()
        .unwrap_or(-1)
        + 1;

    if next < 0 {
        return Err(ApiError::internal(
            "context compression snapshot sequence overflowed",
        ));
    }

    Ok(next)
}

fn serialize_provider_request(request: &NeutralChatRequest) -> Result<String, ApiError> {
    serde_json::to_string(request).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize provider-neutral chat request: {source}"
        ))
    })
}

fn neutral_role_label(role: &NeutralChatRole) -> &'static str {
    match role {
        NeutralChatRole::System => "system",
        NeutralChatRole::User => "user",
        NeutralChatRole::Assistant => "assistant",
        NeutralChatRole::Tool => "tool",
    }
}

fn persist_chat_result(
    context: &PreparedChatContext,
    request_started_at: &str,
    outcome: ChatAuditOutcome,
    events: &[CapturedAuditEvent],
    assistant_text: Option<&str>,
    assistant_reasoning: Option<&str>,
    tool_calls: &[ExecutedToolCall],
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;

    database
        .insert_llm_request(NewLlmRequest {
            id: &context.llm_request_id,
            workspace_id: &context.workspace_id,
            chat_id: Some(&context.chat_id),
            provider_id: &context.provider_id,
            model_id: &context.model_id,
            request_started_at,
            first_token_at: outcome.first_token_at.as_deref(),
            completed_at: Some(&outcome.completed_at),
            input_tokens: outcome.input_tokens,
            output_tokens: outcome.output_tokens,
            cache_read_tokens: outcome.cache_read_tokens,
            cache_write_tokens: outcome.cache_write_tokens,
            first_token_latency_ms: outcome.first_token_latency_ms,
            total_latency_ms: Some(outcome.total_latency_ms),
            status_code: None,
            final_state: outcome.final_state,
            request_body_json: Some(&context.request_body_json),
            response_body_json: outcome.response_body_json.as_deref(),
        })
        .map_err(ApiError::from_workspace_error)?;

    for (index, event) in events.iter().enumerate() {
        let sequence = i64::try_from(index).map_err(|_| {
            ApiError::internal("too many LLM request events to fit SQLite sequence")
        })?;
        let id = format!("{}-event-{sequence}", context.llm_request_id);

        database
            .insert_llm_request_event(NewLlmRequestEvent {
                id: &id,
                llm_request_id: &context.llm_request_id,
                sequence,
                event_at: &event.event_at,
                event_type: &event.event_type,
                raw_chunk_json: None,
                normalized_event_json: &event.normalized_event_json,
            })
            .map_err(ApiError::from_workspace_error)?;
    }

    if let Some(assistant_text) = assistant_text {
        let metadata_json = assistant_message_metadata_json(assistant_reasoning)?;
        database
            .insert_message(NewMessage {
                id: &context.assistant_message_id,
                chat_id: &context.chat_id,
                role: "assistant",
                content: assistant_text,
                sequence: context.assistant_sequence,
                metadata_json: Some(&metadata_json),
            })
            .map_err(ApiError::from_workspace_error)?;
    }

    for tool_call in tool_calls {
        let input_json = serde_json::to_string(&tool_call.input).map_err(|source| {
            ApiError::internal(format!("failed to serialize tool input: {source}"))
        })?;
        let output_json = serde_json::to_string(&tool_call.output).map_err(|source| {
            ApiError::internal(format!("failed to serialize tool output: {source}"))
        })?;
        let result_id = format!("{}-result", tool_call.id);

        database
            .insert_tool_call(NewToolCall {
                id: &tool_call.id,
                chat_id: &context.chat_id,
                run_id: &context.llm_request_id,
                message_id: Some(&context.assistant_message_id),
                tool_name: &tool_call.name,
                input_json: &input_json,
                status: if tool_call.is_error {
                    "error"
                } else {
                    "completed"
                },
                started_at: &tool_call.started_at,
                completed_at: Some(&tool_call.completed_at),
            })
            .map_err(ApiError::from_workspace_error)?;
        database
            .insert_tool_result(NewToolResult {
                id: &result_id,
                tool_call_id: &tool_call.id,
                output_json: &output_json,
                is_error: tool_call.is_error,
                created_at: &tool_call.completed_at,
            })
            .map_err(ApiError::from_workspace_error)?;
    }

    Ok(())
}

fn neutral_text_message(role: NeutralChatRole, content: String) -> NeutralChatMessage {
    NeutralChatMessage {
        role,
        content,
        reasoning: None,
        tool_calls: Vec::new(),
        tool_call_id: None,
        tool_name: None,
    }
}

fn estimate_tool_schema_tokens(tools: &[NeutralToolDefinition]) -> u64 {
    tools
        .iter()
        .map(|tool| {
            estimate_text_tokens(&tool.name)
                + estimate_text_tokens(&tool.description)
                + estimate_json_tokens(&tool.input_schema)
        })
        .sum()
}

fn pack_neutral_messages(
    messages: Vec<NeutralChatMessage>,
    message_source_sequences: &[Option<i64>],
    budget: &foco_agent::ContextBudget,
    active_tool_start_index: usize,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    if messages.len() != message_source_sequences.len() {
        return Err(ApiError::internal(
            "context message source sequence count does not match prompt message count",
        ));
    }

    let latest_user_index = messages
        .iter()
        .rposition(|message| message.role == NeutralChatRole::User);
    let pack_items = messages
        .iter()
        .enumerate()
        .map(|(index, message)| ContextPackItem {
            id: format!("message-{index}"),
            estimated_tokens: if index == 0 {
                0
            } else {
                neutral_message_estimated_tokens(message)
            },
            must_keep: message.role == NeutralChatRole::System
                || Some(index) == latest_user_index
                || index >= active_tool_start_index,
        })
        .collect::<Vec<_>>();
    let packed = pack_context(&pack_items, budget.available_message_tokens)
        .map_err(|source| ApiError::bad_request(source.to_string()))?;

    Ok(packed
        .selected_indices
        .into_iter()
        .map(|index| messages[index].clone())
        .collect())
}

fn neutral_message_estimated_tokens(message: &NeutralChatMessage) -> u64 {
    let mut tokens = estimate_text_tokens(&message.content);

    for tool_call in &message.tool_calls {
        tokens += neutral_tool_call_estimated_tokens(tool_call);
    }

    if let Some(tool_call_id) = &message.tool_call_id {
        tokens += estimate_text_tokens(tool_call_id);
    }

    if let Some(tool_name) = &message.tool_name {
        tokens += estimate_text_tokens(tool_name);
    }

    tokens
}

fn neutral_tool_call_estimated_tokens(tool_call: &NeutralToolCall) -> u64 {
    let thought_tokens = tool_call
        .thought_signatures
        .as_ref()
        .map(|signatures| {
            signatures
                .iter()
                .map(|value| estimate_text_tokens(value))
                .sum::<u64>()
        })
        .unwrap_or(0);

    estimate_text_tokens(&tool_call.call_id)
        + estimate_text_tokens(&tool_call.name)
        + estimate_json_tokens(&tool_call.arguments)
        + thought_tokens
}

fn pending_tool_calls(tool_calls: &[NeutralToolCall]) -> Vec<PendingToolCall> {
    tool_calls
        .iter()
        .map(|tool_call| PendingToolCall {
            id: tool_call.call_id.clone(),
            name: tool_call.name.clone(),
            arguments: tool_call.arguments.clone(),
        })
        .collect()
}

async fn execute_tool_calls_parallel(
    mcp_registry: Arc<McpRegistry>,
    workspace_id: &str,
    workspace_path: &Path,
    tool_calls: Vec<NeutralToolCall>,
) -> Result<Vec<ExecutedToolCall>, ApiError> {
    let tasks = tool_calls.into_iter().map(|tool_call| {
        let workspace_path = workspace_path.to_path_buf();
        let workspace_id = workspace_id.to_string();
        let mcp_registry = mcp_registry.clone();

        tokio::spawn(async move {
            let started_at_text = utc_timestamp();
            let tool_execution = execute_tool(
                mcp_registry,
                &workspace_id,
                &workspace_path,
                &tool_call.name,
                tool_call.arguments.clone(),
            )
            .await;
            let completed_at_text = utc_timestamp();

            executed_tool_call(
                tool_call,
                tool_execution,
                started_at_text,
                completed_at_text,
            )
        })
    });
    let results = join_all(tasks).await;
    let mut executed_tool_calls = Vec::with_capacity(results.len());

    for result in results {
        executed_tool_calls.push(result.map_err(|source| {
            ApiError::internal(format!("tool execution worker failed: {source}"))
        })?);
    }

    Ok(executed_tool_calls)
}

async fn execute_tool(
    mcp_registry: Arc<McpRegistry>,
    workspace_id: &str,
    workspace_path: &Path,
    tool_name: &str,
    arguments: Value,
) -> ToolExecution {
    if is_mcp_tool_name(tool_name) {
        match mcp_registry
            .execute_tool(workspace_id, tool_name, arguments)
            .await
        {
            Ok(execution) => ToolExecution {
                output: execution.output,
                is_error: execution.is_error,
            },
            Err(error) => ToolExecution {
                output: json!({ "error": error.to_string() }),
                is_error: true,
            },
        }
    } else {
        let timeout_ms = match builtin_tool_timeout_ms(tool_name, &arguments) {
            Ok(timeout_ms) => timeout_ms,
            Err(error) => {
                return ToolExecution {
                    output: json!({ "error": error }),
                    is_error: true,
                };
            }
        };
        let tool_name = tool_name.to_string();
        let worker = tokio::task::spawn_blocking({
            let workspace_path = workspace_path.to_path_buf();
            let tool_name = tool_name.clone();
            move || execute_builtin_tool(&workspace_path, &tool_name, arguments)
        });
        let execution: Result<ToolExecution, String> =
            if matches!(tool_name.as_str(), RUN_COMMAND_TOOL | SEARCH_TEXT_TOOL) {
                worker
                    .await
                    .map_err(|source| format!("tool execution worker failed: {source}"))
            } else {
                timeout(Duration::from_millis(timeout_ms), worker)
                    .await
                    .map_err(|_| format!("tool '{tool_name}' timed out after {timeout_ms} ms"))
                    .and_then(|result| {
                        result.map_err(|source| format!("tool execution worker failed: {source}"))
                    })
            };

        match execution {
            Ok(execution) => execution,
            Err(error) => ToolExecution {
                output: json!({ "error": error }),
                is_error: true,
            },
        }
    }
}

fn append_tool_state_messages(
    messages: &mut Vec<NeutralChatMessage>,
    message_source_sequences: &mut Vec<Option<i64>>,
    tool_calls: Vec<NeutralToolCall>,
    tool_results: &[ExecutedToolCall],
    assistant_text: String,
    assistant_reasoning: Option<String>,
) {
    messages.push(NeutralChatMessage {
        role: NeutralChatRole::Assistant,
        content: assistant_text,
        reasoning: assistant_reasoning,
        tool_calls,
        tool_call_id: None,
        tool_name: None,
    });
    message_source_sequences.push(None);

    for tool_result in tool_results {
        messages.push(NeutralChatMessage {
            role: NeutralChatRole::Tool,
            content: serde_json::to_string(&tool_result.output)
                .expect("tool outputs are always JSON serializable"),
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_result.id.clone()),
            tool_name: Some(tool_result.name.clone()),
        });
        message_source_sequences.push(None);
    }
}

fn tool_results_affect_git_diff(tool_results: &[ExecutedToolCall]) -> bool {
    tool_results.iter().any(|tool_result| {
        matches!(
            tool_result.name.as_str(),
            WRITE_FILE_TOOL | RUN_COMMAND_TOOL
        )
    })
}

fn merge_usage(total: &mut NeutralUsage, next: &NeutralUsage) {
    add_usage_tokens(&mut total.input_tokens, next.input_tokens);
    add_usage_tokens(&mut total.output_tokens, next.output_tokens);
    add_usage_tokens(&mut total.cache_read_tokens, next.cache_read_tokens);
    add_usage_tokens(&mut total.cache_write_tokens, next.cache_write_tokens);
}

fn add_usage_tokens(total: &mut Option<i64>, next: Option<i64>) {
    if let Some(next) = next {
        *total = Some(total.unwrap_or(0) + next);
    }
}

fn failed_audit_outcome(started_at: Instant, message: &str) -> ChatAuditOutcome {
    ChatAuditOutcome {
        first_token_at: None,
        completed_at: utc_timestamp(),
        first_token_latency_ms: None,
        total_latency_ms: elapsed_millis(started_at),
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_write_tokens: None,
        final_state: "failed",
        response_body_json: Some(json!({ "error": message }).to_string()),
    }
}

fn captured_provider_event(event: &NeutralChatStreamEvent) -> CapturedAuditEvent {
    let event_type = match event {
        NeutralChatStreamEvent::Start => "start",
        NeutralChatStreamEvent::TextDelta { .. } => "text_delta",
        NeutralChatStreamEvent::ReasoningDelta { .. } => "reasoning_delta",
        NeutralChatStreamEvent::ThoughtSignatureDelta { .. } => "thought_signature_delta",
        NeutralChatStreamEvent::ToolCall { .. } => "tool_call",
        NeutralChatStreamEvent::Usage { .. } => "usage",
        NeutralChatStreamEvent::Complete { .. } => "completion",
        NeutralChatStreamEvent::Error { .. } => "error",
    };

    CapturedAuditEvent {
        event_at: utc_timestamp(),
        event_type: event_type.to_string(),
        normalized_event_json: serde_json::to_string(event)
            .expect("provider-neutral chat stream events are always serializable"),
    }
}

fn captured_event(event: &ChatSseEvent) -> CapturedAuditEvent {
    let event_type = match event {
        ChatSseEvent::Start { .. } => "start",
        ChatSseEvent::TextDelta { .. } => "text_delta",
        ChatSseEvent::ReasoningDelta { .. } => "reasoning_delta",
        ChatSseEvent::ToolCall { .. } => "tool_call",
        ChatSseEvent::ToolResult { .. } => "tool_result",
        ChatSseEvent::GitDiffRefresh { .. } => "git_diff_refresh",
        ChatSseEvent::Usage { .. } => "usage",
        ChatSseEvent::Complete { .. } => "completion",
        ChatSseEvent::Error { .. } => "error",
    };

    CapturedAuditEvent {
        event_at: utc_timestamp(),
        event_type: event_type.to_string(),
        normalized_event_json: serde_json::to_string(event)
            .expect("chat SSE events are always serializable"),
    }
}

fn sse_event(event: &ChatSseEvent) -> Event {
    let data = serde_json::to_string(event).expect("chat SSE events are always serializable");

    Event::default().data(data)
}

fn capture_first_token(
    started_at: Instant,
    first_token_at: &mut Option<String>,
    first_token_latency_ms: &mut Option<i64>,
) {
    if first_token_at.is_none() {
        *first_token_at = Some(utc_timestamp());
        *first_token_latency_ms = Some(elapsed_millis(started_at));
    }
}

fn elapsed_millis(started_at: Instant) -> i64 {
    i64::try_from(started_at.elapsed().as_millis())
        .expect("request latency should fit in i64 milliseconds")
}

fn next_message_sequence(messages: &[MessageRecord]) -> i64 {
    messages
        .iter()
        .map(|message| message.sequence)
        .max()
        .map(|sequence| sequence + 1)
        .unwrap_or(0)
}

fn chat_title(message: &str) -> String {
    let first_line = message
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("New Chat")
        .trim();
    let mut title = first_line.chars().take(60).collect::<String>();

    if title.is_empty() {
        title = "New Chat".to_string();
    }

    title
}

fn unique_id(prefix: &str) -> String {
    let timestamp = Utc::now().timestamp_millis();
    let suffix = NEXT_ID_SUFFIX.fetch_add(1, Ordering::Relaxed);

    format!("{prefix}-{timestamp}-{suffix}")
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    fn from_config_error(error: foco_store::config::ConfigError) -> Self {
        Self::internal(error.to_string())
    }

    fn from_workspace_error(error: foco_store::workspace::WorkspaceDatabaseError) -> Self {
        Self::internal(error.to_string())
    }

    fn from_model_metadata_error(error: ModelMetadataError) -> Self {
        Self::internal(error.to_string())
    }

    fn from_provider_config_error(error: foco_providers::ProviderConfigError) -> Self {
        Self::bad_request(error.to_string())
    }

    fn from_mcp_error(error: foco_mcp::McpError) -> Self {
        Self::bad_request(error.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

fn validate_workspace_request(
    request: WorkspacePathRequest,
) -> Result<(String, PathBuf), ApiError> {
    let name = request.name.trim().to_string();
    let path = PathBuf::from(request.path.trim());

    if name.is_empty() {
        return Err(ApiError::bad_request("workspace name must not be empty"));
    }

    if path.as_os_str().is_empty() {
        return Err(ApiError::bad_request("workspace path must not be empty"));
    }

    if !path.is_absolute() {
        return Err(ApiError::bad_request(format!(
            "workspace path must be absolute: {}",
            path.display()
        )));
    }

    Ok((name, path))
}

fn config_snapshot(state: &AppState) -> Result<GlobalConfig, ApiError> {
    let config = state
        .config
        .lock()
        .map_err(|_| ApiError::internal("global config lock is poisoned"))?;

    Ok(config.clone())
}

fn save_config(state: &AppState, config: GlobalConfig) -> Result<(), ApiError> {
    save_global_config(&state.config_file, &config).map_err(ApiError::from_config_error)?;

    let mut stored_config = state
        .config
        .lock()
        .map_err(|_| ApiError::internal("global config lock is poisoned"))?;
    *stored_config = config;

    Ok(())
}

async fn settings_response(
    state: &AppState,
    config: &GlobalConfig,
) -> Result<Json<SettingsResponse>, ApiError> {
    let active_workspace_id = config.app.active_workspace_id.clone();
    let mcp_statuses = state.mcp_registry.statuses(&active_workspace_id).await;

    Ok(Json(SettingsResponse {
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
        skills: skills_settings_summary(config),
    }))
}

fn configured_provider_summary(provider: &ProviderSettings) -> ConfiguredProviderSummary {
    ConfiguredProviderSummary {
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
        warnings: provider_warnings(provider),
    }
}

fn configured_mcp_server_summary(
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

fn mcp_server_warnings(server: &McpServerConfig) -> Vec<String> {
    let mut warnings = Vec::new();

    if !server.enabled {
        warnings.push("MCP server is disabled.".to_string());
    }

    if let Err(error) = server.to_definition() {
        warnings.push(error.to_string());
    }

    warnings
}

fn skills_settings_summary(config: &GlobalConfig) -> SkillsSettingsSummary {
    let disabled_skill_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(&config.workspaces, &config.skills.directories);

    SkillsSettingsSummary {
        directories: config
            .skills
            .directories
            .iter()
            .map(|directory| directory.display().to_string())
            .collect(),
        detected: discovery
            .skills
            .iter()
            .map(|skill| {
                configured_skill_summary(skill, !disabled_skill_ids.contains(skill.id.as_str()))
            })
            .collect(),
        errors: discovery.errors,
    }
}

fn configured_skill_summary(skill: &SkillSettings, enabled: bool) -> ConfiguredSkillSummary {
    ConfiguredSkillSummary {
        id: skill.id.clone(),
        name: skill.name.clone(),
        description: skill.description.clone(),
        path: skill.path.display().to_string(),
        enabled,
        warnings: skill_warnings(skill, enabled),
    }
}

fn skill_warnings(skill: &SkillSettings, enabled: bool) -> Vec<String> {
    let mut warnings = Vec::new();

    if !enabled {
        warnings.push("Skill is disabled.".to_string());
    }

    if let Err(message) = parse_skill_file(&skill.path) {
        warnings.push(message);
    }

    warnings
}

struct SkillDiscovery {
    skills: Vec<SkillSettings>,
    errors: Vec<SkillDiscoveryErrorSummary>,
}

#[derive(Debug)]
struct ParsedSkillFile {
    id: String,
    name: String,
    description: String,
    instructions: String,
}

fn normalize_skill_directories(values: Vec<String>) -> Result<Vec<PathBuf>, ApiError> {
    let mut directories = Vec::new();
    let mut seen = HashSet::new();

    for value in values {
        let trimmed = value.trim();

        if trimmed.is_empty() {
            continue;
        }

        let path = PathBuf::from(trimmed);
        validate_skill_directory_input(&path)?;

        if seen.insert(path.clone()) {
            directories.push(path);
        }
    }

    Ok(directories)
}

fn validate_skill_directory_input(path: &Path) -> Result<(), ApiError> {
    if path.as_os_str().is_empty() {
        return Err(ApiError::bad_request(
            "skill directory path must not be empty",
        ));
    }

    if path.is_absolute() {
        return Ok(());
    }

    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(ApiError::bad_request(format!(
                "relative skill directory path must stay inside each workspace: {}",
                path.display()
            )));
        }
    }

    Ok(())
}

fn normalize_skill_ids(values: Vec<String>) -> Result<Vec<String>, ApiError> {
    let mut ids = Vec::new();
    let mut seen = HashSet::new();

    for value in values {
        let id = value.trim();

        if id.is_empty() {
            continue;
        }

        validate_skill_id(id).map_err(ApiError::bad_request)?;
        if seen.insert(id.to_string()) {
            ids.push(id.to_string());
        }
    }

    Ok(ids)
}

fn normalize_manual_disabled_skill_ids(
    requested_disabled: Option<Vec<String>>,
    requested_enabled: Option<Vec<String>>,
    discovered_skills: &[SkillSettings],
    existing_disabled: &[String],
) -> Result<Vec<String>, ApiError> {
    if let Some(values) = requested_disabled {
        let mut disabled = normalize_skill_ids(values)?;

        if let Some(enabled_values) = requested_enabled {
            let enabled = normalize_skill_ids(enabled_values)?;
            let form_ids = disabled
                .iter()
                .chain(enabled.iter())
                .cloned()
                .collect::<HashSet<_>>();
            let mut seen = disabled.iter().cloned().collect::<HashSet<_>>();

            for id in existing_disabled {
                let trimmed = id.trim();

                if trimmed.is_empty() {
                    continue;
                }

                validate_skill_id(trimmed).map_err(ApiError::bad_request)?;
                if !form_ids.contains(trimmed) && seen.insert(trimmed.to_string()) {
                    disabled.push(trimmed.to_string());
                }
            }
        }

        return Ok(disabled);
    }

    if let Some(values) = requested_enabled {
        let enabled = normalize_skill_ids(values)?;
        let enabled_ids = enabled.iter().map(String::as_str).collect::<HashSet<_>>();

        return Ok(discovered_skills
            .iter()
            .filter(|skill| !enabled_ids.contains(skill.id.as_str()))
            .map(|skill| skill.id.clone())
            .collect());
    }

    normalize_skill_ids(existing_disabled.to_vec())
}

fn refresh_derived_enabled_skills(config: &mut GlobalConfig) {
    let disabled_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    config.skills.enabled = config
        .skills
        .detected
        .iter()
        .filter(|skill| !disabled_ids.contains(skill.id.as_str()))
        .map(|skill| skill.id.clone())
        .collect();
}

fn discover_skills(workspaces: &[WorkspaceConfig], directories: &[PathBuf]) -> SkillDiscovery {
    let mut skills = Vec::new();
    let mut errors = Vec::new();
    let mut seen_ids = HashSet::new();

    for directory in resolved_skill_directories(workspaces, directories) {
        let candidates = match skill_file_candidates(&directory) {
            Ok(candidates) => candidates,
            Err(message) => {
                errors.push(SkillDiscoveryErrorSummary {
                    path: directory.display().to_string(),
                    message,
                });
                continue;
            }
        };

        for path in candidates {
            match parse_skill_file(&path) {
                Ok(parsed) => {
                    if !seen_ids.insert(parsed.id.clone()) {
                        errors.push(SkillDiscoveryErrorSummary {
                            path: path.display().to_string(),
                            message: format!("duplicate skill id '{}'", parsed.id),
                        });
                        continue;
                    }

                    skills.push(SkillSettings {
                        id: parsed.id,
                        name: parsed.name,
                        description: parsed.description,
                        path,
                    });
                }
                Err(message) => errors.push(SkillDiscoveryErrorSummary {
                    path: path.display().to_string(),
                    message,
                }),
            }
        }
    }

    skills.sort_by(|left, right| left.id.cmp(&right.id));

    SkillDiscovery { skills, errors }
}

fn resolved_skill_directories(
    workspaces: &[WorkspaceConfig],
    directories: &[PathBuf],
) -> Vec<PathBuf> {
    let mut resolved = Vec::new();

    for directory in directories {
        if directory.is_absolute() {
            resolved.push(directory.clone());
            continue;
        }

        for workspace in workspaces {
            resolved.push(workspace.path.join(directory));
        }
    }

    resolved
}

fn skill_file_candidates(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let metadata = match fs::metadata(directory) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(format!(
                "failed to inspect skill directory {}: {}",
                directory.display(),
                source
            ));
        }
    };
    if !metadata.is_dir() {
        return Err(format!(
            "skill path is not a directory: {}",
            directory.display()
        ));
    }

    let mut candidates = Vec::new();
    let direct_skill = directory.join("SKILL.md");
    if direct_skill.is_file() {
        candidates.push(direct_skill);
    }

    let entries = fs::read_dir(directory).map_err(|source| {
        format!(
            "failed to read skill directory {}: {}",
            directory.display(),
            source
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| {
            format!(
                "failed to read skill directory entry under {}: {}",
                directory.display(),
                source
            )
        })?;
        let file_type = entry.file_type().map_err(|source| {
            format!(
                "failed to read skill directory entry type under {}: {}",
                directory.display(),
                source
            )
        })?;

        if file_type.is_dir() {
            let nested_skill = entry.path().join("SKILL.md");
            if nested_skill.is_file() {
                candidates.push(nested_skill);
            }
        }
    }

    candidates.sort();

    Ok(candidates)
}

fn parse_skill_file(path: &Path) -> Result<ParsedSkillFile, String> {
    let content = fs::read_to_string(path)
        .map_err(|source| format!("failed to read skill file {}: {}", path.display(), source))?;

    parse_skill_markdown(path, &content)
}

fn parse_skill_markdown(path: &Path, content: &str) -> Result<ParsedSkillFile, String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = content.lines();

    if lines.next().map(str::trim) != Some("---") {
        return Err(format!(
            "skill file {} must start with YAML frontmatter delimiter '---'",
            path.display()
        ));
    }

    let mut frontmatter = Vec::new();
    let mut has_closing_delimiter = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            has_closing_delimiter = true;
            break;
        }

        frontmatter.push(line);
    }

    if !has_closing_delimiter {
        return Err(format!(
            "skill file {} is missing closing YAML frontmatter delimiter '---'",
            path.display()
        ));
    }

    let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    if body.is_empty() {
        return Err(format!(
            "skill file {} must contain instructions after frontmatter",
            path.display()
        ));
    }

    let id = skill_frontmatter_field(path, &frontmatter, "name")?;
    validate_skill_id(&id).map_err(|error| format!("skill file {}: {}", path.display(), error))?;
    let description = skill_frontmatter_field(path, &frontmatter, "description")?;

    Ok(ParsedSkillFile {
        id: id.clone(),
        name: id,
        description,
        instructions: body,
    })
}

fn skill_frontmatter_field(
    path: &Path,
    frontmatter: &[&str],
    field: &str,
) -> Result<String, String> {
    for line in frontmatter {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };

        if key.trim() != field {
            continue;
        }

        let value = unquote_frontmatter_value(value.trim());
        if value.trim().is_empty() {
            return Err(format!(
                "skill file {} frontmatter field '{}' must not be empty",
                path.display(),
                field
            ));
        }

        return Ok(value.trim().to_string());
    }

    Err(format!(
        "skill file {} frontmatter is missing required field '{}'",
        path.display(),
        field
    ))
}

fn unquote_frontmatter_value(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        let quote = bytes[0];

        if (quote == b'"' || quote == b'\'') && bytes[value.len() - 1] == quote {
            return value[1..value.len() - 1].to_string();
        }
    }

    value.to_string()
}

fn validate_skill_id(id: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("skill id must not be empty".to_string());
    }

    if id.chars().any(char::is_whitespace) {
        return Err(format!("skill id '{}' must not contain whitespace", id));
    }

    Ok(())
}

fn enabled_skill_prompts(config: &GlobalConfig) -> Result<Vec<SkillPromptInfo>, ApiError> {
    let mut skills = Vec::new();
    let disabled_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(&config.workspaces, &config.skills.directories);
    if let Some(error) = discovery.errors.first() {
        return Err(ApiError::bad_request(format!(
            "skill discovery failed for {}: {}",
            error.path, error.message
        )));
    }

    for skill in discovery
        .skills
        .iter()
        .filter(|skill| !disabled_ids.contains(skill.id.as_str()))
    {
        let parsed = parse_skill_file(&skill.path).map_err(ApiError::bad_request)?;

        if parsed.id != skill.id {
            return Err(ApiError::bad_request(format!(
                "enabled skill '{}' file now declares skill id '{}'",
                skill.id, parsed.id
            )));
        }

        skills.push(SkillPromptInfo {
            id: parsed.id,
            name: parsed.name,
            description: parsed.description,
            instructions: parsed.instructions,
        });
    }

    Ok(skills)
}

fn mcp_server_state_name(state: McpServerState) -> &'static str {
    match state {
        McpServerState::Disabled => "disabled",
        McpServerState::Connected => "connected",
        McpServerState::Error => "error",
        McpServerState::Stopped => "stopped",
    }
}

fn mcp_transport_label(transport: &str) -> &'static str {
    match transport {
        "stdio" => "Stdio",
        "streamable-http" => "Streamable HTTP",
        _ => "Unsupported",
    }
}

fn provider_warnings(provider: &ProviderSettings) -> Vec<String> {
    let mut warnings = Vec::new();

    if !provider.enabled {
        warnings.push("Provider is disabled.".to_string());
    }

    if provider
        .api_key
        .as_deref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        warnings.push("Provider has no API key.".to_string());
    }

    if parse_provider_kind(&provider.kind).is_err() {
        warnings.push(format!("Provider kind '{}' is unsupported.", provider.kind));
    }

    warnings
}

fn configured_model_summary_for_config(
    model: &ModelSettings,
    config: &GlobalConfig,
) -> ConfiguredModelSummary {
    let mut summary = configured_model_summary(model);
    summary.supports_thinking = model_supports_thinking(model, config);
    summary.warnings = model_warnings(model, config, summary.can_enable, summary.supports_thinking);
    summary
}

fn workspace_response_from_config(
    config: &GlobalConfig,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let mut workspaces = Vec::with_capacity(config.workspaces.len());

    for workspace in &config.workspaces {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        let chats = database
            .chats()
            .map_err(ApiError::from_workspace_error)?
            .into_iter()
            .map(chat_summary)
            .collect();

        workspaces.push(WorkspaceSummary {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
            path: workspace.path.display().to_string(),
            chats,
        });
    }

    Ok(Json(WorkspacesResponse {
        active_workspace_id: config.app.active_workspace_id.clone(),
        workspaces,
    }))
}

fn workspace_by_id<'a>(
    config: &'a GlobalConfig,
    workspace_id: &str,
) -> Result<&'a WorkspaceConfig, ApiError> {
    let workspace_id = workspace_id.trim();

    if workspace_id.is_empty() {
        return Err(ApiError::bad_request("workspace id must not be empty"));
    }

    config
        .workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| ApiError::bad_request(format!("workspace was not found: {workspace_id}")))
}

fn ensure_git_workspace(workspace_path: &Path) -> Result<(), ApiError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
        .map_err(|source| ApiError::internal(format!("failed to run git: {source}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    if output.status.success() && stdout.trim() == "true" {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(ApiError::bad_request(format!(
            "workspace is not a git repository: {}",
            workspace_path.display()
        )))
    } else {
        Err(ApiError::bad_request(format!(
            "workspace is not a git repository: {} ({stderr})",
            workspace_path.display()
        )))
    }
}

fn run_git_command(workspace_path: &Path, args: &[&str]) -> Result<String, ApiError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .args(args)
        .output()
        .map_err(|source| ApiError::internal(format!("failed to run git: {source}")))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    Err(ApiError::bad_request(format!(
        "git {} exited with status {:?}: {}",
        args.join(" "),
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

fn normalize_workspace_relative_path(input: &str) -> Result<String, ApiError> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(ApiError::bad_request("path must not be empty"));
    }

    let requested = Path::new(trimmed);
    if requested.is_absolute() {
        return Err(ApiError::bad_request(format!(
            "path must be relative to the workspace: {trimmed}"
        )));
    }

    for component in requested.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(ApiError::bad_request(format!(
                "path escapes the workspace: {trimmed}"
            )));
        }
    }

    Ok(trimmed.replace('\\', "/"))
}

fn parse_git_status_files(status: &str) -> Vec<GitStatusFileSummary> {
    status.lines().filter_map(parse_git_status_file).collect()
}

fn parse_git_status_file(line: &str) -> Option<GitStatusFileSummary> {
    if line.len() < 4 {
        return None;
    }

    let index_status = line.get(0..1)?.to_string();
    let worktree_status = line.get(1..2)?.to_string();
    let path = line
        .get(3..)?
        .split(" -> ")
        .last()
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .replace('\\', "/");

    if path.is_empty() {
        return None;
    }

    Some(GitStatusFileSummary {
        path,
        index_status,
        worktree_status,
    })
}

fn model_metadata_response(
    cache: Option<ModelMetadataCache>,
    config: &GlobalConfig,
    cache_path: &Path,
) -> ModelMetadataResponse {
    let (source_url, fetched_at, models) = match cache {
        Some(cache) => (Some(cache.source_url), Some(cache.fetched_at), cache.models),
        None => (None, None, Vec::new()),
    };

    ModelMetadataResponse {
        source_url,
        fetched_at,
        cache_path: cache_path.display().to_string(),
        models,
        configured_models: config
            .models
            .iter()
            .map(|model| configured_model_summary_for_config(model, config))
            .collect(),
    }
}

fn configured_model_summary(model: &ModelSettings) -> ConfiguredModelSummary {
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
        supports_thinking: false,
        warnings: Vec::new(),
    }
}

fn provider_connection_config(
    provider: &ProviderSettings,
) -> Result<ProviderConnectionConfig, ApiError> {
    Ok(ProviderConnectionConfig {
        kind: parse_provider_kind(&provider.kind)
            .map_err(|source| ApiError::bad_request(source.to_string()))?,
        base_url: provider.base_url.clone(),
        api_key: provider.api_key.clone(),
    })
}

fn normalize_model_provider_ids(
    requested_provider_ids: Option<Vec<String>>,
    existing_model: Option<&ModelSettings>,
) -> Result<Vec<String>, ApiError> {
    let values = match requested_provider_ids {
        Some(values) => values,
        None => {
            return Ok(existing_model
                .map(|model| model.provider_ids.clone())
                .unwrap_or_default());
        }
    };
    let mut seen = std::collections::HashSet::new();
    let mut provider_ids = Vec::new();

    for value in values {
        let provider_id = value.trim();

        if provider_id.is_empty() {
            continue;
        }

        if seen.insert(provider_id.to_string()) {
            provider_ids.push(provider_id.to_string());
        }
    }

    Ok(provider_ids)
}

fn validate_model_provider_references(
    config: &GlobalConfig,
    provider_ids: &[String],
    active_provider_id: Option<&str>,
) -> Result<(), ApiError> {
    for provider_id in provider_ids {
        if !config
            .providers
            .iter()
            .any(|provider| provider.id == *provider_id)
        {
            return Err(ApiError::bad_request(format!(
                "model references missing provider '{}'",
                provider_id
            )));
        }
    }

    if let Some(active_provider_id) = active_provider_id {
        if !provider_ids
            .iter()
            .any(|provider_id| provider_id == active_provider_id)
        {
            return Err(ApiError::bad_request(format!(
                "active provider '{}' is not associated with the model",
                active_provider_id
            )));
        }
    }

    Ok(())
}

fn model_supports_thinking(model: &ModelSettings, config: &GlobalConfig) -> bool {
    let has_responses_provider = model.provider_ids.iter().any(|provider_id| {
        config
            .providers
            .iter()
            .any(|provider| provider.id == *provider_id && provider.kind == OPENAI_RESPONSES_KIND)
    });
    let id = model.id.to_ascii_lowercase();

    has_responses_provider
        || id.starts_with("o1")
        || id.starts_with("o3")
        || id.starts_with("o4")
        || id.starts_with("gpt-5")
        || id.contains("reasoning")
        || id.contains("thinking")
}

fn model_warnings(
    model: &ModelSettings,
    config: &GlobalConfig,
    can_enable: bool,
    supports_thinking: bool,
) -> Vec<String> {
    let mut warnings = Vec::new();

    if model.enabled && !can_enable {
        warnings.push("Enabled model is missing required limits.".to_string());
    }

    if model.enabled && model.provider_ids.is_empty() {
        warnings.push("Enabled model is not associated with any provider.".to_string());
    }

    if let Some(active_provider_id) = &model.active_provider_id {
        if !model
            .provider_ids
            .iter()
            .any(|provider_id| provider_id == active_provider_id)
        {
            warnings.push(format!(
                "Active provider '{}' is not associated with this model.",
                active_provider_id
            ));
        }
    } else if !model.provider_ids.is_empty() {
        warnings.push("Model has providers but no active provider selected.".to_string());
    }

    for provider_id in &model.provider_ids {
        match config
            .providers
            .iter()
            .find(|provider| provider.id == *provider_id)
        {
            Some(provider) if !provider.enabled => {
                warnings.push(format!("Provider '{}' is disabled.", provider.name));
            }
            Some(_) => {}
            None => warnings.push(format!("Provider '{}' does not exist.", provider_id)),
        }
    }

    if model.thinking_level.is_some() && !supports_thinking {
        warnings.push(
            "Thinking level is saved, but Foco cannot verify this model supports thinking options."
                .to_string(),
        );
    }

    warnings
}

fn provider_kind_label(kind: &str) -> &'static str {
    match kind {
        OPENAI_CHAT_KIND => "OpenAI Chat",
        OPENAI_RESPONSES_KIND => "OpenAI Responses",
        _ => "Unsupported",
    }
}

fn optional_trimmed_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn cached_model_record(
    cache_path: &Path,
    key: &str,
) -> Result<Option<ModelMetadataRecord>, ModelMetadataError> {
    let cache = read_model_metadata_cache(cache_path)?;

    Ok(cache.and_then(|cache| cache.models.into_iter().find(|model| model.key == key)))
}

fn utc_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn chat_summary(chat: ChatRecord) -> ChatSummary {
    ChatSummary {
        id: chat.id,
        title: chat.title,
        created_at: chat.created_at,
        updated_at: chat.updated_at,
    }
}

fn neutral_tool_definition(definition: foco_tools::ToolDefinition) -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: definition.name.to_string(),
        description: definition.description.to_string(),
        input_schema: definition.input_schema,
        strict: definition.strict,
    }
}

fn neutral_mcp_tool_definition(definition: &McpToolDefinition) -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: definition.name.clone(),
        description: format!(
            "MCP server '{}', tool '{}': {}",
            definition.server_name, definition.original_name, definition.description
        ),
        input_schema: definition.input_schema.clone(),
        strict: false,
    }
}

fn pending_tool_call_summary(tool_call: &NeutralToolCall) -> ChatToolCallSummary {
    ChatToolCallSummary {
        id: tool_call.call_id.clone(),
        name: tool_call.name.clone(),
        status: "pending".to_string(),
        input: tool_call.arguments.clone(),
        output: None,
        is_error: false,
    }
}

fn executed_tool_call(
    tool_call: NeutralToolCall,
    execution: ToolExecution,
    started_at: String,
    completed_at: String,
) -> ExecutedToolCall {
    ExecutedToolCall {
        id: tool_call.call_id,
        name: tool_call.name,
        input: tool_call.arguments,
        output: execution.output,
        is_error: execution.is_error,
        started_at,
        completed_at,
    }
}

fn assistant_message_text(assistant_text: &str, tool_calls: &[ExecutedToolCall]) -> String {
    if assistant_text.is_empty() && !tool_calls.is_empty() {
        "Tool calls completed.".to_string()
    } else {
        assistant_text.to_string()
    }
}

fn assistant_reasoning_from_metadata(metadata_json: &str) -> Result<Option<String>, ApiError> {
    let metadata = parse_json_value(metadata_json, "assistant message metadata")?;
    let Some(reasoning) = metadata.get("reasoning") else {
        return Ok(None);
    };

    if reasoning.is_null() {
        return Ok(None);
    }

    let reasoning = reasoning.as_str().ok_or_else(|| {
        ApiError::internal("assistant message metadata.reasoning must be a string")
    })?;

    Ok(non_empty_string(reasoning))
}

fn assistant_message_metadata_json(reasoning: Option<&str>) -> Result<String, ApiError> {
    let Some(reasoning) = reasoning else {
        return Ok("{}".to_string());
    };

    serde_json::to_string(&json!({ "reasoning": reasoning })).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize assistant message metadata: {source}"
        ))
    })
}

fn non_empty_string(value: &str) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn chat_message_summary(
    database: &WorkspaceDatabase,
    message: MessageRecord,
) -> Result<ChatMessageSummary, ApiError> {
    let tool_calls = database
        .tool_calls_for_message(&message.id)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(chat_tool_call_summary)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ChatMessageSummary {
        id: message.id,
        reasoning: if message.role == "assistant" {
            assistant_reasoning_from_metadata(&message.metadata_json)?
        } else {
            None
        },
        role: message.role,
        content: message.content,
        tool_calls,
    })
}

fn chat_tool_call_summary(
    record: ToolCallWithResultRecord,
) -> Result<ChatToolCallSummary, ApiError> {
    let input = parse_json_value(&record.input_json, "tool call input")?;
    let (output, is_error) = match record.result {
        Some(result) => (
            Some(parse_json_value(&result.output_json, "tool result output")?),
            result.is_error,
        ),
        None => (None, false),
    };

    Ok(ChatToolCallSummary {
        id: record.id,
        name: record.tool_name,
        status: record.status,
        input,
        output,
        is_error,
    })
}

fn parse_json_value(value: &str, field: &str) -> Result<Value, ApiError> {
    serde_json::from_str(value)
        .map_err(|source| ApiError::internal(format!("failed to parse {field}: {source}")))
}

fn canonical_workspace_path(path: &Path) -> Result<PathBuf, ApiError> {
    fs::canonicalize(path).map_err(|source| {
        ApiError::internal(format!(
            "failed to resolve workspace path {}: {}",
            path.display(),
            source
        ))
    })
}

fn reject_registered_workspace_path(config: &GlobalConfig, path: &Path) -> Result<(), ApiError> {
    for workspace in &config.workspaces {
        let registered_path = canonical_workspace_path(&workspace.path)?;

        if registered_path == path {
            return Err(ApiError::bad_request(format!(
                "workspace path is already registered as '{}': {}",
                workspace.name,
                path.display()
            )));
        }
    }

    Ok(())
}

fn unique_workspace_id(config: &GlobalConfig, name: &str) -> String {
    let base = workspace_id_slug(name);

    if !workspace_id_exists(config, &base) {
        return base;
    }

    for index in 2.. {
        let candidate = format!("{base}-{index}");

        if !workspace_id_exists(config, &candidate) {
            return candidate;
        }
    }

    unreachable!("unbounded workspace id suffix search always returns");
}

fn workspace_id_exists(config: &GlobalConfig, id: &str) -> bool {
    config.workspaces.iter().any(|workspace| workspace.id == id)
}

fn workspace_id_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;

    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator && !slug.is_empty() {
            slug.push('-');
            last_was_separator = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "workspace".to_string()
    } else {
        slug
    }
}

fn local_addr() -> Result<SocketAddr, String> {
    let port = match env::var(PORT_ENV) {
        Ok(value) => parse_port(&value)?,
        Err(env::VarError::NotPresent) => DEFAULT_PORT,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(format!("{PORT_ENV} must be valid Unicode"));
        }
    };

    Ok(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
}

fn parse_port(value: &str) -> Result<u16, String> {
    let port = value
        .parse::<u16>()
        .map_err(|_| format!("{PORT_ENV} must be a number from 1 to 65535"))?;

    if port == 0 {
        return Err(format!("{PORT_ENV} must be a number from 1 to 65535"));
    }

    Ok(port)
}

fn frontend_dist_dir() -> Result<PathBuf, String> {
    let app_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_dir = app_dir
        .parent()
        .ok_or_else(|| "app crate must live inside the Foco repository".to_string())?;
    let dist_dir = repo_dir.join("web").join("dist");
    let index_file = dist_dir.join("index.html");

    if !index_file.is_file() {
        return Err(format!(
            "frontend build missing at {}. Run `npm run build -w web` before starting the backend.",
            index_file.display()
        ));
    }

    Ok(dist_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn enabled_skill_prompts_read_skill_instructions() {
        let skill_dir = env::temp_dir().join(unique_id("foco-skill-test"));
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

        let mut config = GlobalConfig::first_run(env::temp_dir());
        config.skills.directories = vec![skill_dir.clone()];

        let skills = enabled_skill_prompts(&config).expect("enabled skill prompts");

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "gitmemo");
        assert!(skills[0].instructions.contains("Search memory"));

        fs::remove_dir_all(skill_dir).expect("remove skill test directory");
    }

    #[test]
    fn enabled_skill_prompts_skip_disabled_discovered_skills() {
        let skill_dir = env::temp_dir().join(unique_id("foco-disabled-skill-test"));
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

        let mut config = GlobalConfig::first_run(env::temp_dir());
        config.skills.directories = vec![skill_dir.clone()];
        config.skills.disabled.push("gitmemo".to_string());

        let skills = enabled_skill_prompts(&config).expect("enabled skill prompts");

        assert!(skills.is_empty());

        fs::remove_dir_all(skill_dir).expect("remove skill test directory");
    }

    #[test]
    fn discover_skills_ignores_missing_directories() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-missing-skill-test"));
        let missing_absolute = workspace_dir.join("missing-skills");

        fs::create_dir_all(&workspace_dir).expect("workspace directory");

        let workspaces = vec![WorkspaceConfig {
            id: "default".to_string(),
            name: "Default".to_string(),
            path: workspace_dir.clone(),
        }];
        let discovery = discover_skills(
            &workspaces,
            &[
                missing_absolute,
                PathBuf::from(".agents").join("skills"),
                PathBuf::from(".claude").join("skills"),
            ],
        );

        assert!(discovery.errors.is_empty());
        assert!(discovery.skills.is_empty());

        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    }

    #[test]
    fn discover_skills_reports_non_directory_paths() {
        let profile_dir = env::temp_dir().join(unique_id("foco-skill-file-path-test"));
        let skill_path = profile_dir.join("skills");

        fs::create_dir_all(&profile_dir).expect("profile directory");
        fs::write(&skill_path, "not a directory").expect("skill path file write");

        let discovery = discover_skills(&[], &[skill_path]);

        assert_eq!(discovery.errors.len(), 1);
        assert!(discovery.errors[0].message.contains("not a directory"));
        assert!(discovery.skills.is_empty());

        fs::remove_dir_all(profile_dir).expect("remove profile directory");
    }

    #[test]
    fn manual_skill_save_keeps_hidden_disabled_ids() {
        let discovered = vec![
            test_skill_settings("gitmemo"),
            test_skill_settings("newskill"),
        ];
        let disabled = normalize_manual_disabled_skill_ids(
            Some(Vec::new()),
            Some(vec!["newskill".to_string()]),
            &discovered,
            &["gitmemo".to_string()],
        )
        .expect("disabled skill ids");

        assert_eq!(disabled, vec!["gitmemo"]);
    }

    #[test]
    fn manual_skill_save_reenables_visible_disabled_ids() {
        let discovered = vec![test_skill_settings("gitmemo")];
        let disabled = normalize_manual_disabled_skill_ids(
            Some(Vec::new()),
            Some(vec!["gitmemo".to_string()]),
            &discovered,
            &["gitmemo".to_string()],
        )
        .expect("disabled skill ids");

        assert!(disabled.is_empty());
    }

    #[test]
    fn discover_skills_expands_workspace_relative_directories() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-skill-test"));
        let skill_dir = workspace_dir.join(".agents").join("skills").join("gitmemo");
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

        let workspaces = vec![WorkspaceConfig {
            id: "default".to_string(),
            name: "Default".to_string(),
            path: workspace_dir.clone(),
        }];
        let discovery = discover_skills(&workspaces, &[PathBuf::from(".agents").join("skills")]);

        assert!(discovery.errors.is_empty());
        assert_eq!(discovery.skills.len(), 1);
        assert_eq!(discovery.skills[0].id, "gitmemo");
        assert_eq!(discovery.skills[0].path, skill_file);

        fs::remove_dir_all(workspace_dir).expect("remove skill test directory");
    }

    fn test_skill_settings(id: &str) -> SkillSettings {
        SkillSettings {
            id: id.to_string(),
            name: id.to_string(),
            description: "Test skill.".to_string(),
            path: env::temp_dir().join(id).join("SKILL.md"),
        }
    }
}
