use std::{
    env, fs,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{SecondsFormat, Utc};
use foco_store::{
    config::{
        GlobalConfig, ModelLimits, ModelSettings, WorkspaceConfig, load_or_create_global_config,
        save_global_config,
    },
    model_metadata::{
        MODELS_DEV_API_URL, ModelMetadataCache, ModelMetadataError, ModelMetadataRecord,
        parse_models_dev_metadata, read_model_metadata_cache, write_model_metadata_cache,
    },
    workspace::{ChatRecord, WorkspaceDatabase, initialize_workspace_databases},
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

mod logging;

const DEFAULT_PORT: u16 = 3210;
const PORT_ENV: &str = "FOCO_PORT";

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
        .route("/api/model-metadata", get(model_metadata))
        .route("/api/model-metadata/refresh", post(refresh_model_metadata))
        .route("/api/models/manual", post(save_manual_model))
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
    let model = ModelSettings {
        id: model_id.to_string(),
        display_name: display_name.to_string(),
        enabled: request.enabled,
        provider_ids: existing_model
            .map(|model| model.provider_ids.clone())
            .unwrap_or_default(),
        active_provider_id: existing_model.and_then(|model| model.active_provider_id.clone()),
        thinking_level: existing_model.and_then(|model| model.thinking_level.clone()),
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
        configured_models: config.models.iter().map(configured_model_summary).collect(),
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
    }
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
