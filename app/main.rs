use std::{
    collections::HashSet,
    convert::Infallible,
    env, fs,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use chrono::{SecondsFormat, Utc};
use foco_providers::{
    DEFAULT_OPENAI_BASE_URL, NeutralChatMessage, NeutralChatRequest, NeutralChatRole,
    NeutralChatStreamEvent, NeutralToolCall, NeutralToolDefinition, NeutralUsage, OPENAI_CHAT_KIND,
    OPENAI_RESPONSES_KIND, ProviderConnectionConfig, normalized_base_url, parse_provider_kind,
    stream_chat, test_provider_connection,
};
use foco_store::{
    config::{
        GlobalConfig, ModelLimits, ModelSettings, ProviderSettings, WorkspaceConfig,
        load_or_create_global_config, save_global_config,
    },
    model_metadata::{
        MODELS_DEV_API_URL, ModelMetadataCache, ModelMetadataError, ModelMetadataRecord,
        parse_models_dev_metadata, read_model_metadata_cache, write_model_metadata_cache,
    },
    workspace::{
        ChatRecord, MessageRecord, NewLlmRequest, NewLlmRequestEvent, NewMessage, NewToolCall,
        NewToolResult, ToolCallWithResultRecord, WorkspaceDatabase, initialize_workspace_databases,
    },
};
use foco_tools::{ToolExecution, builtin_tool_definitions, execute_builtin_tool};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

mod logging;

const DEFAULT_PORT: u16 = 3210;
const PORT_ENV: &str = "FOCO_PORT";
static NEXT_ID_SUFFIX: AtomicU64 = AtomicU64::new(1);

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone)]
struct AppState {
    config: Arc<Mutex<GlobalConfig>>,
    config_file: PathBuf,
    model_metadata_file: PathBuf,
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

    let addr = local_addr()?;
    let frontend_dir = frontend_dist_dir()?;
    let state = AppState {
        config: Arc::new(Mutex::new(loaded_config.config)),
        config_file: loaded_config.paths.config_file,
        model_metadata_file: loaded_config.paths.root_dir.join("models.dev.json"),
    };
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/workspaces", get(workspaces))
        .route("/api/workspaces/create", post(create_workspace))
        .route("/api/workspaces/add", post(add_workspace))
        .route("/api/settings", get(settings))
        .route("/api/providers/manual", post(save_manual_provider))
        .route("/api/providers/test", post(test_provider))
        .route("/api/model-metadata", get(model_metadata))
        .route("/api/model-metadata/refresh", post(refresh_model_metadata))
        .route("/api/models/manual", post(save_manual_model))
        .route(
            "/api/workspaces/{workspace_id}/chat/stream",
            post(stream_chat_response),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/messages",
            get(chat_messages),
        )
        .fallback_service(ServeDir::new(frontend_dir))
        .with_state(state);
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "starting local HTTP server");
    println!("Foco is running at http://{addr}");
    axum::serve(listener, app).await?;

    Ok(())
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

    workspace_response_from_config(&config)
}

async fn settings(State(state): State<AppState>) -> Result<Json<SettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;

    Ok(Json(settings_response(&config)))
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

    Ok(Json(settings_response(&config)))
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

async fn stream_chat_response(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<ChatStreamRequest>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let config = config_snapshot(&state)?;
    let chat_context = prepare_chat_context(&config, &workspace_id, request)?;

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
struct TestProviderRequest {
    provider_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatStreamRequest {
    chat_id: Option<String>,
    model_id: String,
    thinking_level: Option<String>,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsResponse {
    provider_kinds: Vec<ProviderKindSummary>,
    thinking_levels: Vec<ThinkingLevelSummary>,
    providers: Vec<ConfiguredProviderSummary>,
    configured_models: Vec<ConfiguredModelSummary>,
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
struct ChatMessageSummary {
    id: String,
    role: String,
    content: String,
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
        delta: String,
    },
    ReasoningDelta {
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
    Usage {
        usage: NeutralUsage,
    },
    Complete {
        chat_id: String,
        assistant_message_id: String,
        text: String,
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
    request_body_json: String,
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
    fn into_sse_stream(self) -> impl futures_util::Stream<Item = Result<Event, Infallible>> {
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

            yield Ok(sse_event(&start_event));

            let mut provider_stream = match stream_chat(&self.provider_config, self.provider_request.clone()).await {
                Ok(provider_stream) => provider_stream,
                Err(error) => {
                    let message = error.to_string();
                    let event = ChatSseEvent::Error {
                        message: message.clone(),
                    };
                    events.push(captured_event(&event));
                    let outcome = failed_audit_outcome(started_at, &message);

                    if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, &[]) {
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

            let mut assistant_text = String::new();
            let mut first_token_at = None;
            let mut first_token_latency_ms = None;
            let mut seen_tool_call_ids = HashSet::new();

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

                        if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, &[]) {
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
                        let event = ChatSseEvent::TextDelta { delta };
                        yield Ok(sse_event(&event));
                    }
                    NeutralChatStreamEvent::ReasoningDelta { delta } => {
                        capture_first_token(started_at, &mut first_token_at, &mut first_token_latency_ms);
                        let event = ChatSseEvent::ReasoningDelta { delta };
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
                        if assistant_text.is_empty() && !text.is_empty() {
                            let message = "provider completed without streaming assistant text deltas".to_string();
                            let event = ChatSseEvent::Error {
                                message: message.clone(),
                            };
                            events.push(captured_event(&event));
                            let outcome = failed_audit_outcome(started_at, &message);

                            if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, &[]) {
                                let event = ChatSseEvent::Error {
                                    message: persist_error.message,
                                };
                                yield Ok(sse_event(&event));
                            } else {
                                yield Ok(sse_event(&event));
                            }

                            return;
                        }

                        if assistant_text.is_empty() && tool_calls.is_empty() {
                            let message = "provider completed without assistant text or tool calls".to_string();
                            let event = ChatSseEvent::Error {
                                message: message.clone(),
                            };
                            events.push(captured_event(&event));
                            let outcome = failed_audit_outcome(started_at, &message);

                            if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, &[]) {
                                let event = ChatSseEvent::Error {
                                    message: persist_error.message,
                                };
                                yield Ok(sse_event(&event));
                            } else {
                                yield Ok(sse_event(&event));
                            }

                            return;
                        }

                        let mut executed_tool_calls = Vec::new();
                        for tool_call in tool_calls {
                            capture_first_token(started_at, &mut first_token_at, &mut first_token_latency_ms);
                            if seen_tool_call_ids.insert(tool_call.call_id.clone()) {
                                let event = ChatSseEvent::ToolCall {
                                    assistant_message_id: self.assistant_message_id.clone(),
                                    tool_call: pending_tool_call_summary(&tool_call),
                                };
                                events.push(captured_event(&event));
                                yield Ok(sse_event(&event));
                            }

                            let started_at_text = utc_timestamp();
                            let tool_execution = execute_builtin_tool(
                                &self.workspace_path,
                                &tool_call.name,
                                tool_call.arguments.clone(),
                            );
                            let completed_at_text = utc_timestamp();
                            let executed_tool_call = executed_tool_call(
                                tool_call,
                                tool_execution,
                                started_at_text,
                                completed_at_text,
                            );
                            let result_event = ChatSseEvent::ToolResult {
                                assistant_message_id: self.assistant_message_id.clone(),
                                tool_call_id: executed_tool_call.id.clone(),
                                output: executed_tool_call.output.clone(),
                                is_error: executed_tool_call.is_error,
                            };
                            events.push(captured_event(&result_event));
                            yield Ok(sse_event(&result_event));
                            executed_tool_calls.push(executed_tool_call);
                        }

                        let assistant_message_text =
                            assistant_message_text(&assistant_text, &executed_tool_calls);
                        let completed_at = utc_timestamp();
                        let outcome = ChatAuditOutcome {
                            first_token_at,
                            completed_at,
                            first_token_latency_ms,
                            total_latency_ms: elapsed_millis(started_at),
                            input_tokens: usage.as_ref().and_then(|usage| usage.input_tokens),
                            output_tokens: usage.as_ref().and_then(|usage| usage.output_tokens),
                            cache_read_tokens: usage.as_ref().and_then(|usage| usage.cache_read_tokens),
                            cache_write_tokens: usage.as_ref().and_then(|usage| usage.cache_write_tokens),
                            final_state: "succeeded",
                            response_body_json: Some(json!({
                                "text": text,
                                "reasoning": reasoning,
                                "toolCalls": executed_tool_calls,
                                "usage": usage,
                                "stopReason": stop_reason,
                                "responseId": response_id
                            }).to_string()),
                        };

                        match persist_chat_result(&self, &request_started_at, outcome, &events, Some(&assistant_message_text), &executed_tool_calls) {
                            Ok(()) => {
                                let event = ChatSseEvent::Complete {
                                    chat_id: self.chat_id.clone(),
                                    assistant_message_id: self.assistant_message_id.clone(),
                                    text: assistant_message_text,
                                    usage,
                                    stop_reason,
                                };
                                yield Ok(sse_event(&event));
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
                    NeutralChatStreamEvent::Error { message } => {
                        let event = ChatSseEvent::Error {
                            message: message.clone(),
                        };
                        events.push(captured_event(&event));
                        let outcome = failed_audit_outcome(started_at, &message);

                        if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, &[]) {
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

            let message = "provider stream ended without a completion event".to_string();
            let event = ChatSseEvent::Error {
                message: message.clone(),
            };
            events.push(captured_event(&event));
            let outcome = failed_audit_outcome(started_at, &message);

            if let Err(persist_error) = persist_chat_result(&self, &request_started_at, outcome, &events, None, &[]) {
                let event = ChatSseEvent::Error {
                    message: persist_error.message,
                };
                yield Ok(sse_event(&event));
            } else {
                yield Ok(sse_event(&event));
            }
        }
    }
}

fn prepare_chat_context(
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

    let mut neutral_messages = Vec::with_capacity(existing_messages.len() + 1);
    for existing_message in existing_messages {
        neutral_messages.push(neutral_message_from_record(existing_message)?);
    }
    neutral_messages.push(NeutralChatMessage {
        role: NeutralChatRole::User,
        content: message.to_string(),
    });

    let provider_request = NeutralChatRequest {
        model_id: model.id.clone(),
        messages: neutral_messages,
        tools: builtin_tool_definitions()
            .into_iter()
            .map(neutral_tool_definition)
            .collect(),
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
        request_body_json,
    })
}

fn neutral_message_from_record(message: MessageRecord) -> Result<NeutralChatMessage, ApiError> {
    let role = match message.role.as_str() {
        "system" => NeutralChatRole::System,
        "user" => NeutralChatRole::User,
        "assistant" => NeutralChatRole::Assistant,
        "tool" => {
            return Err(ApiError::bad_request(
                "chat contains tool-role messages; tool replay belongs to TODO step 10",
            ));
        }
        other => {
            return Err(ApiError::bad_request(format!(
                "chat contains unsupported message role '{other}'"
            )));
        }
    };

    Ok(NeutralChatMessage {
        role,
        content: message.content,
    })
}

fn persist_chat_result(
    context: &PreparedChatContext,
    request_started_at: &str,
    outcome: ChatAuditOutcome,
    events: &[CapturedAuditEvent],
    assistant_text: Option<&str>,
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
        database
            .insert_message(NewMessage {
                id: &context.assistant_message_id,
                chat_id: &context.chat_id,
                role: "assistant",
                content: assistant_text,
                sequence: context.assistant_sequence,
                metadata_json: Some("{}"),
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

fn settings_response(config: &GlobalConfig) -> SettingsResponse {
    SettingsResponse {
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
    }
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
