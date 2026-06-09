#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    env, fs,
    net::{IpAddr, SocketAddr},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Path as AxumPath, Query, Request, State, ws::WebSocketUpgrade},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose};
use chrono::{Local, SecondsFormat, Utc};
use foco_agent::{
    ContextPackItem, PendingToolCall, SystemPromptInput, ToolPromptInfo, build_system_prompt,
    calculate_context_budget, context_compression_trigger_tokens, detect_same_file_write_conflicts,
    estimate_json_tokens, estimate_text_tokens, pack_context, plan_context_compression,
};
use foco_graph::{CodeGraphWatcher, index_workspace, start_code_graph_watcher};
use foco_mcp::{
    McpRegistry, McpServerDefinition, McpServerState, McpToolDefinition, is_mcp_tool_name,
};
use foco_providers::{
    DEFAULT_OPENAI_BASE_URL, NeutralChatAttachment, NeutralChatMessage, NeutralChatRequest,
    NeutralChatRole, NeutralChatStreamEvent, NeutralToolCall, NeutralToolDefinition, NeutralUsage,
    OPENAI_CHAT_KIND, OPENAI_RESPONSES_KIND, ProviderConfigError, ProviderConnectionConfig,
    normalized_base_url, normalized_proxy_url, parse_provider_kind, stream_chat,
    test_provider_connection,
};
use foco_store::{
    config::{
        ApiProxySettings, DEFAULT_TERMINAL_SHELL, GlobalConfig, HookConfig, HookEventMap,
        McpServerConfig, ModelLimits, ModelSettings, ProviderSettings, SKILL_SCOPE_GLOBAL,
        SKILL_SCOPE_WORKSPACE, SUPPORTED_API_PROXY_TYPES, SUPPORTED_APP_LANGUAGES,
        SUPPORTED_HOOK_EVENTS, SUPPORTED_TERMINAL_SHELLS, SkillSettings, UNSUPPORTED_HOOK_EVENTS,
        WebServerSettings, WorkspaceConfig, load_or_create_global_config,
        load_workspace_hook_config, save_global_config, save_workspace_hook_config,
        workspace_hook_config_path,
    },
    memory::MemoryDatabase,
    model_metadata::{
        MODELS_DEV_API_URL, ModelMetadataCache, ModelMetadataError, ModelMetadataRecord,
        parse_models_dev_metadata, read_model_metadata_cache, write_model_metadata_cache,
    },
    workspace::{
        ChatRecord, ContextCompressionSnapshotRecord, HookRunRecord, LlmRequestAuditFilters,
        LlmRequestAuditRow, LlmRequestEventRecord, LlmRequestRecord, MessageRecord,
        NewContextCompressionSnapshot, NewLlmRequest, NewLlmRequestEvent, NewMessage,
        NewTerminalSession, NewToolCall, NewToolResult, TaskGraphFilter, TaskGraphRecord,
        TaskGraphTask, ToolCallWithResultRecord, WorkspaceDatabase, initialize_workspace_databases,
    },
};
use foco_tools::{
    ASK_QUESTION_TOOL, CREATE_TASK_GRAPH_TOOL, PATCH_FILE_TOOL, RUN_COMMAND_TOOL, SEARCH_TEXT_TOOL,
    SLEEP_TOOL, ToolExecution, UPDATE_TASK_GRAPH_TOOL, WRITE_FILE_TOOL, builtin_tool_definitions,
    builtin_tool_timeout_ms, execute_builtin_tool_for_chat,
};
use futures_util::future::join_all;
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::timeout;

use crate::git_backend::{
    create_git_branch as create_git_branch_in_workspace, git_branches_response, git_diff_response,
    git_status_response, is_git_workspace, switch_git_branch as switch_git_branch_in_workspace,
};
use crate::hooks::{
    EffectiveHookSummary, HookDecision, HookNotification, HookRunRequest, HookRunSummary,
    HookRuntime, effective_hook_summaries,
};

#[cfg(all(windows, not(debug_assertions)))]
use std::sync::atomic::AtomicU32;

mod git_backend;
mod hooks;
mod logging;
mod terminal;

const PORT_ENV: &str = "FOCO_PORT";
const HOST_ENV: &str = "FOCO_HOST";
const MAX_AGENT_TOOL_ROUNDS: usize = 8;
const CONTEXT_COMPRESSION_PRESERVE_RECENT_MESSAGES: usize = 4;
const CONTEXT_COMPRESSION_MAX_MESSAGE_CHARS: usize = 320;
const CONTEXT_COMPRESSION_MAX_MESSAGE_ENTRIES: usize = 16;
const CONTEXT_COMPRESSION_PROMPT_PREFIX: &str = "Context compression snapshot:";
const MAX_CHAT_ATTACHMENTS: usize = 6;
const MAX_CHAT_ATTACHMENT_BYTES: u64 = 10 * 1024 * 1024;
const MAX_CHAT_ATTACHMENT_TOTAL_BYTES: u64 = 24 * 1024 * 1024;
const CHAT_ATTACHMENT_BODY_LIMIT_BYTES: usize = 40 * 1024 * 1024;
const MAX_WORKSPACE_LOGO_BYTES: u64 = 2 * 1024 * 1024;
const WORKSPACE_LOGO_BODY_LIMIT_BYTES: usize = 4 * 1024 * 1024;
const WORKSPACE_LOGO_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
const AGENTS_MESSAGE_PREFIX: &str = "AGENTS.md instructions loaded from";
const ENABLED_SKILLS_MESSAGE_PREFIX: &str =
    "Enabled skill front matter loaded from configured skills";
const ENVIRONMENT_CONTEXT_MESSAGE_PREFIX: &str = "Environment context for this chat";
const SHUTDOWN_MESSAGE: &str = "app shutdown requested";
const AUTH_COOKIE_NAME: &str = "foco_auth";
const PASSWORD_HASH_PREFIX: &str = "sha256";
static NEXT_ID_SUFFIX: AtomicU64 = AtomicU64::new(1);

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Embed)]
#[folder = "../web/dist"]
struct WebAssets;

#[cfg(all(windows, not(debug_assertions)))]
const TRAY_OPEN_ITEM_ID: &str = "foco-open-ui";
#[cfg(all(windows, not(debug_assertions)))]
const TRAY_QUIT_ITEM_ID: &str = "foco-quit";

#[cfg(all(windows, not(debug_assertions)))]
#[derive(Clone)]
struct TrayMenuUpdateNotifier {
    sender: std::sync::mpsc::Sender<TrayMenuLabels>,
    thread_id: Arc<AtomicU32>,
}

#[cfg(all(windows, not(debug_assertions)))]
impl TrayMenuUpdateNotifier {
    fn notify(&self, labels: TrayMenuLabels) -> Result<(), String> {
        use windows_sys::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_NULL};

        let thread_id = self.thread_id.load(Ordering::SeqCst);
        if thread_id == 0 {
            return Err("tray menu message thread is not ready".to_string());
        }

        self.sender
            .send(labels)
            .map_err(|_| "tray menu update receiver is closed".to_string())?;
        let posted = unsafe { PostThreadMessageW(thread_id, WM_NULL, 0, 0) };
        if posted == 0 {
            return Err(format!(
                "failed to wake tray menu message loop: {}",
                std::io::Error::last_os_error()
            ));
        }

        Ok(())
    }
}

#[derive(Clone)]
struct AppState {
    config: Arc<Mutex<GlobalConfig>>,
    config_file: PathBuf,
    model_metadata_file: PathBuf,
    user_profile_dir: PathBuf,
    terminal_registry: terminal::TerminalRegistry,
    terminal_shutdown_tx: broadcast::Sender<()>,
    app_shutdown_rx: watch::Receiver<bool>,
    mcp_registry: Arc<McpRegistry>,
    hook_runtime: HookRuntime,
    question_registry: QuestionRegistry,
    _code_graph_watchers: Arc<Vec<CodeGraphWatcher>>,
    #[cfg(all(windows, not(debug_assertions)))]
    tray_menu_update_notifier: TrayMenuUpdateNotifier,
}

#[derive(Clone, Default)]
struct QuestionRegistry {
    pending: Arc<Mutex<HashMap<String, PendingQuestion>>>,
}

struct PendingQuestion {
    request: QuestionRequest,
    answer_tx: oneshot::Sender<QuestionAnswer>,
}

struct QuestionRegistration {
    answer_rx: oneshot::Receiver<QuestionAnswer>,
    _cleanup: QuestionCleanup,
}

struct QuestionCleanup {
    registry: QuestionRegistry,
    question_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AskQuestionInput {
    questions: Vec<AskQuestionItemInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AskQuestionItemInput {
    question: String,
    options: Option<Vec<QuestionOption>>,
    allow_free_text: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestionOption {
    label: String,
    value: String,
    description: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestionRequest {
    id: String,
    tool_call_id: String,
    workspace_id: String,
    chat_id: String,
    questions: Vec<QuestionItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestionItem {
    id: String,
    question: String,
    options: Vec<QuestionOption>,
    allow_free_text: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestionAnswer {
    answers: Vec<QuestionItemAnswer>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestionItemAnswer {
    id: String,
    answer: String,
    #[serde(default)]
    selected_option_value: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestionAnswerResponse {
    ok: bool,
    question_id: String,
}

impl QuestionRegistry {
    fn register(&self, request: QuestionRequest) -> Result<QuestionRegistration, ApiError> {
        let question_id = request.id.clone();
        let (answer_tx, answer_rx) = oneshot::channel();
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| ApiError::internal("question registry lock is poisoned"))?;

        if pending
            .insert(question_id.clone(), PendingQuestion { request, answer_tx })
            .is_some()
        {
            return Err(ApiError::internal(format!(
                "duplicate pending question id: {question_id}"
            )));
        }

        Ok(QuestionRegistration {
            answer_rx,
            _cleanup: QuestionCleanup {
                registry: self.clone(),
                question_id,
            },
        })
    }

    fn answer(&self, question_id: &str, answer: QuestionAnswer) -> Result<(), ApiError> {
        let question_id = question_id.trim();

        if question_id.is_empty() {
            return Err(ApiError::bad_request("question id must not be empty"));
        }

        let mut pending = self
            .pending
            .lock()
            .map_err(|_| ApiError::internal("question registry lock is poisoned"))?;
        let pending_question = pending.get(question_id).ok_or_else(|| {
            ApiError::bad_request(format!(
                "question is not waiting for an answer: {question_id}"
            ))
        })?;
        validate_question_answer(&pending_question.request, &answer)?;
        let pending_question = pending
            .remove(question_id)
            .expect("pending question should still exist after validation");

        pending_question.answer_tx.send(answer).map_err(|_| {
            ApiError::bad_request(format!(
                "question is no longer waiting for an answer: {question_id}"
            ))
        })
    }

    fn remove(&self, question_id: &str) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(question_id);
        }
    }
}

impl Drop for QuestionCleanup {
    fn drop(&mut self) {
        self.registry.remove(&self.question_id);
    }
}

fn validate_question_answer(
    request: &QuestionRequest,
    answer: &QuestionAnswer,
) -> Result<(), ApiError> {
    if answer.answers.len() != request.questions.len() {
        return Err(ApiError::bad_request(format!(
            "question '{}' requires answers for all {} questions",
            request.id,
            request.questions.len()
        )));
    }

    let mut answered_question_ids = HashSet::new();

    for answer in &answer.answers {
        let question_id = answer.id.trim();

        if question_id.is_empty() {
            return Err(ApiError::bad_request(
                "answer question id must not be empty",
            ));
        }

        if !answered_question_ids.insert(question_id) {
            return Err(ApiError::bad_request(format!(
                "duplicate answer for question item: {question_id}"
            )));
        }

        let question = request
            .questions
            .iter()
            .find(|question| question.id == question_id)
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "answer references unknown question item: {question_id}"
                ))
            })?;

        validate_question_item_answer(question, answer)?;
    }

    for question in &request.questions {
        if !answered_question_ids.contains(question.id.as_str()) {
            return Err(ApiError::bad_request(format!(
                "missing answer for question item: {}",
                question.id
            )));
        }
    }

    Ok(())
}

fn validate_question_item_answer(
    question: &QuestionItem,
    answer: &QuestionItemAnswer,
) -> Result<(), ApiError> {
    let answer_text = answer.answer.trim();
    let selected_option_value = answer
        .selected_option_value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(selected_option_value) = selected_option_value {
        let selected_option = question
            .options
            .iter()
            .find(|option| option.value == selected_option_value)
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "selected option was not found for question item '{}': {selected_option_value}",
                    question.id
                ))
            })?;

        if answer_text != selected_option.value {
            return Err(ApiError::bad_request(
                "answer must match selectedOptionValue when an option is selected",
            ));
        }

        return Ok(());
    }

    if !question.allow_free_text {
        return Err(ApiError::bad_request(format!(
            "question item '{}' requires selecting one of the provided options",
            question.id
        )));
    }

    if answer_text.is_empty() {
        return Err(ApiError::bad_request("answer must not be empty"));
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(error) = run_entrypoint().await {
        eprintln!("Foco startup failed: {error}");
        std::process::exit(1);
    }
}

async fn run_entrypoint() -> AppResult<()> {
    #[cfg(all(windows, not(debug_assertions)))]
    {
        return run_windows_tray_entrypoint();
    }

    #[cfg(any(not(windows), debug_assertions))]
    {
        run_server_until_shutdown(None).await
    }
}

async fn run_server_until_shutdown(
    shutdown_rx: Option<watch::Receiver<bool>>,
    #[cfg(all(windows, not(debug_assertions)))] tray_menu_update_notifier: TrayMenuUpdateNotifier,
) -> AppResult<()> {
    let loaded_config = load_or_create_global_config()?;
    logging::init(&loaded_config.paths.logs_dir)?;

    tracing::info!(
        config = %loaded_config.config.to_redacted_log_json()?,
        "loaded global config"
    );

    let global_memory_database =
        MemoryDatabase::open_or_create_global_at(&loaded_config.paths.memory_database_file)?;
    tracing::info!(
        path = %global_memory_database.database_path().display(),
        "initialized global memory database"
    );
    drop(global_memory_database);

    let workspace_databases = initialize_workspace_databases(&loaded_config.config.workspaces)?;
    tracing::info!(
        count = workspace_databases.len(),
        "initialized workspace databases"
    );
    let code_graph_watchers = initialize_code_graph_indexes(&loaded_config.config.workspaces)?;
    let mcp_registry = Arc::new(McpRegistry::default());
    sync_all_mcp_workspaces(&mcp_registry, &loaded_config.config).await?;
    let hook_runtime = HookRuntime::new(mcp_registry.clone());

    let addr = local_addr(&loaded_config.config)?;
    verify_frontend_assets()?;
    let (terminal_shutdown_tx, _) = broadcast::channel(16);
    let (owned_shutdown_tx, owned_shutdown_rx);
    let (shutdown_tx, app_shutdown_rx) = match shutdown_rx {
        Some(shutdown_rx) => (None, shutdown_rx),
        None => {
            (owned_shutdown_tx, owned_shutdown_rx) = watch::channel(false);
            (Some(owned_shutdown_tx), owned_shutdown_rx)
        }
    };
    let state = AppState {
        config: Arc::new(Mutex::new(loaded_config.config)),
        config_file: loaded_config.paths.config_file,
        model_metadata_file: loaded_config.paths.root_dir.join("models.dev.json"),
        user_profile_dir: loaded_config.paths.user_profile_dir,
        terminal_registry: terminal::TerminalRegistry::default(),
        terminal_shutdown_tx: terminal_shutdown_tx.clone(),
        app_shutdown_rx: app_shutdown_rx.clone(),
        mcp_registry: mcp_registry.clone(),
        hook_runtime,
        question_registry: QuestionRegistry::default(),
        _code_graph_watchers: Arc::new(code_graph_watchers),
        #[cfg(all(windows, not(debug_assertions)))]
        tray_menu_update_notifier,
    };
    let app = app_router(state);
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "starting local HTTP server");
    println!("Foco is running at http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(
            shutdown_tx,
            app_shutdown_rx,
            terminal_shutdown_tx,
            mcp_registry,
        ))
        .await?;

    Ok(())
}

fn app_router(state: AppState) -> Router {
    let auth_state = state.clone();

    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/status", get(auth_status))
        .route("/api/auth/login", post(auth_login))
        .route("/api/auth/logout", post(auth_logout))
        .route("/api/workspaces", get(workspaces))
        .route("/api/workspaces/add", post(add_workspace))
        .route("/api/workspaces/manual", post(save_workspace_settings))
        .route("/api/workspaces/order", post(save_workspace_order))
        .route(
            "/api/workspaces/{workspace_id}/logo",
            get(workspace_logo)
                .post(save_workspace_logo)
                .delete(clear_workspace_logo)
                .layer(DefaultBodyLimit::max(WORKSPACE_LOGO_BODY_LIMIT_BYTES)),
        )
        .route("/api/native/select-directory", post(select_directory))
        .route("/api/native/select-files", post(select_files))
        .route("/api/settings", get(settings))
        .route("/api/settings/general", post(save_general_settings))
        .route("/api/hooks", get(hooks_settings))
        .route("/api/hooks/global", post(save_global_hooks))
        .route("/api/hooks/workspace", post(save_workspace_hooks))
        .route("/api/hooks/import-claude", post(import_claude_hooks))
        .route("/api/hooks/test", post(test_hooks))
        .route("/api/workspaces/{workspace_id}/hooks/runs", get(hook_runs))
        .route(
            "/api/workspaces/{workspace_id}/hooks/runs/{hook_run_id}",
            get(hook_run_detail),
        )
        .route("/api/providers/manual", post(save_manual_provider))
        .route("/api/providers/delete", post(delete_provider))
        .route("/api/providers/test", post(test_provider))
        .route("/api/model-metadata", get(model_metadata))
        .route("/api/model-metadata/refresh", post(refresh_model_metadata))
        .route("/api/models/manual", post(save_manual_model))
        .route("/api/models/delete", post(delete_model))
        .route("/api/models/order", post(save_model_order))
        .route("/api/mcp/servers/manual", post(save_mcp_server))
        .route("/api/mcp/servers/delete", post(delete_mcp_server))
        .route("/api/skills/manual", post(save_skills))
        .route("/api/skills/refresh", post(refresh_skills))
        .route("/api/ai-statistics", get(ai_statistics))
        .route(
            "/api/workspaces/{workspace_id}/chat/stream",
            post(stream_chat_response)
                .layer(DefaultBodyLimit::max(CHAT_ATTACHMENT_BODY_LIMIT_BYTES)),
        )
        .route(
            "/api/workspaces/{workspace_id}/context-usage",
            post(context_usage).layer(DefaultBodyLimit::max(CHAT_ATTACHMENT_BODY_LIMIT_BYTES)),
        )
        .route(
            "/api/chat/questions/{question_id}/answer",
            post(answer_question),
        )
        .route(
            "/api/workspaces/{workspace_id}/ai-statistics/{request_id}",
            get(ai_statistics_detail),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/messages",
            get(chat_messages),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/task-graph",
            get(chat_task_graph),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/delete",
            post(delete_chat),
        )
        .route("/api/workspaces/{workspace_id}/git/status", get(git_status))
        .route("/api/workspaces/{workspace_id}/git/diff", get(git_diff))
        .route(
            "/api/workspaces/{workspace_id}/git/branches",
            get(git_branches),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/branches/switch",
            post(switch_git_branch),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/branches/create",
            post(create_git_branch),
        )
        .route(
            "/api/workspaces/{workspace_id}/terminal/session",
            post(create_terminal_session),
        )
        .route(
            "/api/workspaces/{workspace_id}/terminal/{session_id}/ws",
            get(terminal_socket),
        )
        .fallback(static_asset)
        .layer(middleware::from_fn_with_state(auth_state, require_auth))
        .with_state(state)
}

#[cfg(all(windows, not(debug_assertions)))]
fn run_windows_tray_entrypoint() -> AppResult<()> {
    let loaded_config = load_or_create_global_config()?;
    let addr = local_addr(&loaded_config.config)?;
    let ui_url = format!("http://{}", browser_addr_for_listen_addr(addr));
    let labels = tray_menu_labels(&loaded_config.config.app.language)?;
    let (tray_menu_update_tx, tray_menu_update_rx) = std::sync::mpsc::channel();
    let tray_menu_thread_id = Arc::new(AtomicU32::new(0));
    let tray_menu_update_notifier = TrayMenuUpdateNotifier {
        sender: tray_menu_update_tx,
        thread_id: tray_menu_thread_id.clone(),
    };
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let runtime_thread = std::thread::Builder::new()
        .name("foco-http-runtime".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build Foco HTTP runtime");
            if let Err(error) = runtime.block_on(run_server_until_shutdown(
                Some(shutdown_rx),
                tray_menu_update_notifier,
            )) {
                eprintln!("Foco server failed: {error}");
                std::process::exit(1);
            }
        })?;

    run_windows_tray_loop(
        ui_url,
        shutdown_tx,
        labels,
        tray_menu_update_rx,
        tray_menu_thread_id,
    )?;
    runtime_thread
        .join()
        .map_err(|_| "Foco HTTP runtime thread panicked")?;

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
    shutdown_tx: Option<watch::Sender<bool>>,
    mut app_shutdown_rx: watch::Receiver<bool>,
    terminal_shutdown_tx: broadcast::Sender<()>,
    mcp_registry: Arc<McpRegistry>,
) {
    tokio::select! {
        ctrl_c = tokio::signal::ctrl_c() => {
            if let Err(source) = ctrl_c {
                tracing::warn!(error = %source, "failed to listen for Ctrl+C shutdown");
                return;
            }
            if let Some(shutdown_tx) = shutdown_tx {
                let _ = shutdown_tx.send(true);
            }
        }
        changed = app_shutdown_rx.changed() => {
            if changed.is_err() || !*app_shutdown_rx.borrow() {
                return;
            }
        }
    }

    tracing::info!("shutdown requested; closing terminal sessions");
    let _ = terminal_shutdown_tx.send(());
    if let Err(error) = mcp_registry.stop_all().await {
        tracing::warn!(error = %error, "failed to stop MCP servers");
    }
}

#[cfg(all(windows, not(debug_assertions)))]
fn run_windows_tray_loop(
    ui_url: String,
    shutdown_tx: watch::Sender<bool>,
    labels: TrayMenuLabels,
    tray_menu_update_rx: std::sync::mpsc::Receiver<TrayMenuLabels>,
    tray_menu_thread_id: Arc<AtomicU32>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tray_icon::{
        TrayIconBuilder,
        menu::{Menu, MenuItem, PredefinedMenuItem},
    };
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, TranslateMessage, WM_QUIT,
    };

    let mut message = MSG::default();
    unsafe {
        PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
        tray_menu_thread_id.store(GetCurrentThreadId(), Ordering::SeqCst);
    }
    let tray_menu = Menu::new();
    let open_item = MenuItem::with_id(TRAY_OPEN_ITEM_ID, labels.open, true, None);
    let quit_item = MenuItem::with_id(TRAY_QUIT_ITEM_ID, labels.quit, true, None);
    let separator = PredefinedMenuItem::separator();
    tray_menu.append_items(&[&open_item, &separator, &quit_item])?;
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Foco")
        .with_icon(foco_tray_icon()?)
        .build()?;

    loop {
        drain_tray_events(&ui_url, &shutdown_tx);
        drain_tray_menu_updates(&tray_menu_update_rx, &open_item, &quit_item);

        let message_result = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
        if message_result == -1 {
            return Err(Box::new(std::io::Error::last_os_error()));
        }
        if message_result == 0 || message.message == WM_QUIT {
            break;
        }

        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    Ok(())
}

#[cfg(all(windows, not(debug_assertions)))]
fn drain_tray_menu_updates(
    tray_menu_update_rx: &std::sync::mpsc::Receiver<TrayMenuLabels>,
    open_item: &tray_icon::menu::MenuItem,
    quit_item: &tray_icon::menu::MenuItem,
) {
    while let Ok(labels) = tray_menu_update_rx.try_recv() {
        open_item.set_text(labels.open);
        quit_item.set_text(labels.quit);
    }
}

#[cfg(all(windows, not(debug_assertions)))]
fn drain_tray_events(ui_url: &str, shutdown_tx: &watch::Sender<bool>) {
    use tray_icon::{TrayIconEvent, menu::MenuEvent};
    use windows_sys::Win32::UI::WindowsAndMessaging::PostQuitMessage;

    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if matches!(event, TrayIconEvent::DoubleClick { .. }) {
            open_foco_ui(ui_url);
        }
    }

    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == TRAY_OPEN_ITEM_ID {
            open_foco_ui(ui_url);
        } else if event.id == TRAY_QUIT_ITEM_ID {
            let _ = shutdown_tx.send(true);
            unsafe {
                PostQuitMessage(0);
            }
        }
    }
}

#[cfg(all(windows, not(debug_assertions)))]
fn foco_tray_icon() -> Result<tray_icon::Icon, tray_icon::BadIcon> {
    tray_icon::Icon::from_resource(1, Some((32, 32)))
}

#[cfg(all(windows, not(debug_assertions)))]
fn open_foco_ui(ui_url: &str) {
    if let Err(error) = webbrowser::open(ui_url) {
        tracing::warn!(%ui_url, error = %error, "failed to open Foco web UI");
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

async fn require_auth(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if auth_route_is_public(request.uri().path()) {
        return next.run(request).await;
    }

    let config = match config_snapshot(&state) {
        Ok(config) => config,
        Err(error) => return error.into_response(),
    };

    if !web_auth_enabled(&config) || request_has_valid_auth_cookie(request.headers(), &config) {
        return next.run(request).await;
    }

    ApiError::unauthorized("authentication required").into_response()
}

fn auth_route_is_public(path: &str) -> bool {
    path == "/api/health"
        || path == "/api/auth/status"
        || path == "/api/auth/login"
        || path == "/api/auth/logout"
        || !path.starts_with("/api/")
}

async fn auth_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthStatusResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let enabled = web_auth_enabled(&config);
    let authenticated = !enabled || request_has_valid_auth_cookie(&headers, &config);

    Ok(Json(AuthStatusResponse {
        enabled,
        authenticated,
    }))
}

async fn auth_login(
    State(state): State<AppState>,
    Json(request): Json<AuthLoginRequest>,
) -> Result<Response, ApiError> {
    let config = config_snapshot(&state)?;
    let Some(password_hash) = config.app.web_server.password_hash.as_deref() else {
        return Err(ApiError::bad_request("web authentication is not enabled"));
    };

    if !verify_password(&request.password, password_hash) {
        return Err(ApiError::unauthorized("invalid password"));
    }

    let cookie = auth_cookie(password_hash);
    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(AuthStatusResponse {
            enabled: true,
            authenticated: true,
        }),
    )
        .into_response())
}

async fn auth_logout(State(state): State<AppState>) -> Result<Response, ApiError> {
    let config = config_snapshot(&state)?;

    Ok((
        [(header::SET_COOKIE, expired_auth_cookie())],
        Json(AuthStatusResponse {
            enabled: web_auth_enabled(&config),
            authenticated: false,
        }),
    )
        .into_response())
}

async fn static_asset(uri: axum::http::Uri) -> Response {
    let request_path = uri.path().trim_start_matches('/');
    if request_path.starts_with("api/") {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("API route not found"))
            .expect("static asset response is valid");
    }

    let asset_path = if request_path.is_empty() {
        "index.html"
    } else {
        request_path
    };
    let asset = WebAssets::get(asset_path).or_else(|| {
        if asset_path.rsplit_once('.').is_none() {
            WebAssets::get("index.html")
        } else {
            None
        }
    });

    match asset {
        Some(asset) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, asset.metadata.mimetype())
            .body(Body::from(asset.data.into_owned()))
            .expect("static asset response is valid"),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("frontend asset not found"))
            .expect("static asset response is valid"),
    }
}

#[derive(Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
}

#[derive(Serialize)]
struct AuthStatusResponse {
    enabled: bool,
    authenticated: bool,
}

async fn workspaces(State(state): State<AppState>) -> Result<Json<WorkspacesResponse>, ApiError> {
    let config = config_snapshot(&state)?;

    workspace_response_from_config(&config)
}

async fn add_workspace(
    State(state): State<AppState>,
    Json(request): Json<WorkspacePathRequest>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let (name, requested_path) = validate_workspace_request(request)?;

    if requested_path.exists() && !requested_path.is_dir() {
        return Err(ApiError::bad_request(format!(
            "workspace path exists but is not a directory: {}",
            requested_path.display()
        )));
    }

    if !requested_path.exists() {
        fs::create_dir_all(&requested_path).map_err(|source| {
            ApiError::internal(format!(
                "failed to create workspace directory {}: {}",
                requested_path.display(),
                source
            ))
        })?;
    }

    let path = canonical_workspace_path(&requested_path)?;
    let mut config = config_snapshot(&state)?;

    reject_registered_workspace_path(&config, &path, None)?;
    WorkspaceDatabase::open_or_create(&path).map_err(ApiError::from_workspace_error)?;

    let id = unique_workspace_id(&config, &name);
    config.workspaces.push(WorkspaceConfig {
        id,
        name,
        path,
        pinned: false,
        terminal_shell: DEFAULT_TERMINAL_SHELL.to_string(),
    });
    save_config(&state, config.clone())?;
    sync_all_mcp_workspaces(&state.mcp_registry, &config)
        .await
        .map_err(ApiError::from_mcp_error)?;

    workspace_response_from_config(&config)
}

async fn save_workspace_settings(
    State(state): State<AppState>,
    Json(request): Json<ManualWorkspaceRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let workspace_id = request.id.trim();
    let name = request.name.trim();
    let requested_path = validate_workspace_path(&request.path)?;
    let terminal_shell = normalize_terminal_shell(&request.terminal_shell)?;

    if workspace_id.is_empty() {
        return Err(ApiError::bad_request("workspace id must not be empty"));
    }

    if name.is_empty() {
        return Err(ApiError::bad_request("workspace name must not be empty"));
    }

    if requested_path.exists() && !requested_path.is_dir() {
        return Err(ApiError::bad_request(format!(
            "workspace path exists but is not a directory: {}",
            requested_path.display()
        )));
    }

    if !requested_path.exists() {
        fs::create_dir_all(&requested_path).map_err(|source| {
            ApiError::internal(format!(
                "failed to create workspace directory {}: {}",
                requested_path.display(),
                source
            ))
        })?;
    }

    let path = canonical_workspace_path(&requested_path)?;
    reject_registered_workspace_path(&config, &path, Some(workspace_id))?;

    let workspace = config
        .workspaces
        .iter_mut()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| ApiError::bad_request(format!("workspace was not found: {workspace_id}")))?;

    workspace.name = name.to_string();
    workspace.path = path;
    workspace.pinned = request.pinned;
    workspace.terminal_shell = terminal_shell;
    group_pinned_workspaces(&mut config.workspaces);

    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

async fn save_workspace_order(
    State(state): State<AppState>,
    Json(request): Json<WorkspaceOrderRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;

    reorder_workspaces(&mut config.workspaces, request.workspace_ids)?;
    group_pinned_workspaces(&mut config.workspaces);
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

async fn workspace_logo(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Response, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let Some(logo) = workspace_logo_file(&workspace.path)? else {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("workspace logo was not found"))
            .expect("workspace logo response is valid"));
    };
    let (bytes, _) = read_workspace_logo_file(&logo.path)?;
    let kind = workspace_logo_kind(&bytes)?;
    if kind != logo.kind {
        return Err(ApiError::bad_request(format!(
            "workspace logo changed while it was being read: {}",
            logo.path.display()
        )));
    }

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, kind.content_type)
        .header(header::CACHE_CONTROL, "private, max-age=60")
        .body(Body::from(bytes))
        .expect("workspace logo response is valid"))
}

async fn save_workspace_logo(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<WorkspaceLogoRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let bytes = workspace_logo_request_bytes(&request)?;
    let kind = workspace_logo_kind(&bytes)?;

    save_workspace_logo_file(&workspace.path, &bytes, kind)?;

    settings_response(&state, &config).await
}

async fn clear_workspace_logo(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    clear_workspace_logo_file(&workspace.path)?;

    settings_response(&state, &config).await
}

async fn select_directory() -> Result<Json<SelectDirectoryResponse>, ApiError> {
    let path = native_select_directory()?;

    Ok(Json(SelectDirectoryResponse { path }))
}

async fn select_files() -> Result<Json<SelectFilesResponse>, ApiError> {
    let files = native_select_files()?;

    Ok(Json(SelectFilesResponse { files }))
}

fn normalize_web_server_settings(
    current: &WebServerSettings,
    request: &ManualGeneralSettingsRequest,
) -> Result<WebServerSettings, ApiError> {
    let listen_host = request.listen_host.trim();

    if listen_host.is_empty() {
        return Err(ApiError::bad_request(
            "web server listen host must not be empty",
        ));
    }

    listen_host
        .parse::<IpAddr>()
        .map_err(|_| ApiError::bad_request("web server listen host must be an IP address"))?;

    if request.listen_port == 0 || request.listen_port > u16::MAX.into() {
        return Err(ApiError::bad_request(
            "web server listen port must be a number from 1 to 65535",
        ));
    }

    let password_hash = if request.clear_password.unwrap_or(false) {
        None
    } else if let Some(password) = &request.password {
        if password.trim().is_empty() {
            return Err(ApiError::bad_request(
                "web authentication password must not be empty",
            ));
        }
        Some(hash_password(password)?)
    } else {
        current.password_hash.clone()
    };

    Ok(WebServerSettings {
        listen_host: listen_host.to_string(),
        listen_port: request.listen_port as u16,
        password_hash,
    })
}

fn normalize_api_proxy_settings(
    current: &ApiProxySettings,
    request: Option<&ManualApiProxySettingsRequest>,
) -> Result<ApiProxySettings, ApiError> {
    let Some(request) = request else {
        return Ok(current.clone());
    };
    let proxy_type = request.proxy_type.trim();

    if !SUPPORTED_API_PROXY_TYPES.contains(&proxy_type) {
        return Err(ApiError::bad_request(format!(
            "AI API proxy type '{proxy_type}' is unsupported; expected one of {}",
            SUPPORTED_API_PROXY_TYPES.join(", ")
        )));
    }

    let proxy_url = request.url.trim();
    if request.enabled && proxy_url.is_empty() {
        return Err(ApiError::bad_request(
            "AI API proxy URL must not be empty when enabled",
        ));
    }

    let normalized_url = if request.enabled || !proxy_url.is_empty() {
        normalized_proxy_url(proxy_type, proxy_url)
            .map_err(|source| ApiError::bad_request(source.to_string()))?
    } else {
        String::new()
    };

    Ok(ApiProxySettings {
        enabled: request.enabled,
        proxy_type: proxy_type.to_string(),
        url: normalized_url,
    })
}

fn normalize_app_language(language: &str) -> Result<String, ApiError> {
    let language = language.trim();

    if SUPPORTED_APP_LANGUAGES.contains(&language) {
        return Ok(language.to_string());
    }

    Err(ApiError::bad_request(format!(
        "app language '{language}' is unsupported; expected one of {}",
        SUPPORTED_APP_LANGUAGES.join(", ")
    )))
}

fn app_language_name(language: &str) -> &'static str {
    match language {
        "zh-CN" => "简体中文",
        "en" => "English",
        _ => "Unknown",
    }
}

async fn settings(State(state): State<AppState>) -> Result<Json<SettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;

    settings_response(&state, &config).await
}

async fn hooks_settings(
    State(state): State<AppState>,
    Query(query): Query<HooksQuery>,
) -> Result<Json<HooksSettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    hooks_settings_response(&state, &config, query.workspace_id.as_deref()).await
}

async fn save_global_hooks(
    State(state): State<AppState>,
    Json(request): Json<SaveGlobalHooksRequest>,
) -> Result<Json<HooksSettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let audit_enabled = config.hooks.audit_enabled;

    config.hooks = request.config;
    config.hooks.audit_enabled = audit_enabled;
    config
        .validate(Some(&state.config_file))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    save_config(&state, config.clone())?;

    hooks_settings_response(&state, &config, None).await
}

async fn save_workspace_hooks(
    State(state): State<AppState>,
    Json(request): Json<SaveWorkspaceHooksRequest>,
) -> Result<Json<HooksSettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &request.workspace_id)?.clone();
    let mut validation_config = config.clone();

    validation_config.hooks = request.config.clone();
    validation_config
        .validate(Some(&workspace_hook_config_path(&workspace.path)))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    save_workspace_hook_config(&workspace.path, &request.config)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    hooks_settings_response(&state, &config, Some(&workspace.id)).await
}

async fn import_claude_hooks(
    State(state): State<AppState>,
    Json(request): Json<ImportClaudeHooksRequest>,
) -> Result<Json<ImportClaudeHooksResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let target = request.target.trim();
    if target != "global" && target != "workspace" {
        return Err(ApiError::bad_request(
            "hook import target must be 'global' or 'workspace'",
        ));
    }
    let workspace = if target == "workspace" {
        let workspace_id = request.workspace_id.as_deref().ok_or_else(|| {
            ApiError::bad_request("workspaceId is required for workspace hook import")
        })?;
        Some(workspace_by_id(&config, workspace_id)?.clone())
    } else {
        None
    };

    let (import_source, save_path, source_paths) = if target == "global" {
        (
            "global",
            state.config_file.clone(),
            claude_hook_settings_paths(&state.user_profile_dir),
        )
    } else {
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| ApiError::internal("workspace hook import lost selected workspace"))?;
        (
            "workspace",
            workspace_hook_config_path(&workspace.path),
            claude_hook_settings_paths(&workspace.path),
        )
    };
    let (mut imported, imported_files, mut validation_errors) =
        import_claude_hook_config(&source_paths)?;

    if imported_files.is_empty() {
        validation_errors.push(format!(
            "no Claude hook settings were found under {}",
            source_paths
                .first()
                .and_then(|path| path.parent())
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| ".claude".to_string())
        ));
    }

    if validation_errors.is_empty() {
        let mut validation_config = config.clone();
        if target == "global" {
            imported.audit_enabled = config.hooks.audit_enabled;
        }
        validation_config.hooks = imported.clone();
        if let Err(error) = validation_config.validate(Some(&save_path)) {
            validation_errors.push(error.to_string());
        }
    }

    if !validation_errors.is_empty() {
        return Ok(Json(ImportClaudeHooksResponse {
            saved: false,
            target: import_source.to_string(),
            path: display_path(&save_path),
            imported_files,
            validation_errors,
            config: imported,
        }));
    }

    if target == "global" {
        config.hooks = imported.clone();
        save_config(&state, config)?;
    } else {
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| ApiError::internal("workspace hook import lost selected workspace"))?;
        save_workspace_hook_config(&workspace.path, &imported)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }

    Ok(Json(ImportClaudeHooksResponse {
        saved: true,
        target: import_source.to_string(),
        path: display_path(&save_path),
        imported_files,
        validation_errors,
        config: imported,
    }))
}

async fn test_hooks(
    State(state): State<AppState>,
    Json(request): Json<TestHookRequest>,
) -> Result<Json<HookRunSummary>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &request.workspace_id)?;
    let event = request.event.trim();

    if UNSUPPORTED_HOOK_EVENTS.contains(&event) {
        return Err(ApiError::bad_request(format!(
            "{event} is a Claude Code hook event that Foco does not support yet"
        )));
    }
    if !SUPPORTED_HOOK_EVENTS.contains(&event) {
        return Err(ApiError::bad_request(format!(
            "{event} is unsupported; expected one of {}",
            SUPPORTED_HOOK_EVENTS.join(", ")
        )));
    }

    let provider = default_hook_provider(&config).transpose()?;
    let summary = state
        .hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &config.hooks,
            workspace_id: &workspace.id,
            workspace_path: &workspace.path,
            event,
            match_value: optional_trimmed_string(request.match_value),
            chat_id: None,
            run_id: None,
            session_id: None,
            tool_call_id: None,
            model_id: provider.as_ref().map(|provider| provider.0.as_str()),
            provider_id: provider.as_ref().map(|provider| provider.1.as_str()),
            provider_config: provider.as_ref().map(|provider| &provider.2),
            permission_mode: None,
            payload: request.payload.unwrap_or_else(|| json!({})),
        })
        .await;

    Ok(Json(summary))
}

async fn hook_run_detail(
    State(state): State<AppState>,
    AxumPath((workspace_id, hook_run_id)): AxumPath<(String, String)>,
) -> Result<Json<HookRunDetailResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let hook_run_id = hook_run_id.trim();

    if hook_run_id.is_empty() {
        return Err(ApiError::bad_request("hook run id must not be empty"));
    }

    let record = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?
        .hook_run(hook_run_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("hook run was not found: {hook_run_id}")))?;

    if record.workspace_id != workspace.id {
        return Err(ApiError::bad_request(format!(
            "hook run '{}' does not belong to workspace '{}'",
            record.id, workspace.id
        )));
    }

    Ok(Json(HookRunDetailResponse {
        run: hook_run_detail_from_record(record)?,
    }))
}

async fn hook_runs(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Query(query): Query<HookRunsQuery>,
) -> Result<Json<HookRunsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?
        .hook_runs(limit)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .filter(|record| record.workspace_id == workspace.id)
        .map(hook_run_summary_row)
        .collect();

    Ok(Json(HookRunsResponse { runs }))
}

async fn save_general_settings(
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
    config.app.language = normalize_app_language(&request.language)?;
    if let Some(hook_audit_enabled) = request.hook_audit_enabled {
        config.hooks.audit_enabled = hook_audit_enabled;
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
    let current_api_proxy = existing_provider
        .map(|provider| provider.api_proxy.clone())
        .unwrap_or_default();
    let api_proxy = normalize_api_proxy_settings(&current_api_proxy, request.api_proxy.as_ref())?;
    let provider = ProviderSettings {
        id: id.to_string(),
        name: name.to_string(),
        kind: provider_kind.as_str().to_string(),
        enabled: request.enabled,
        base_url: normalized_base_url,
        api_key,
        api_proxy,
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
    let discovery = discover_skills(&state.user_profile_dir, &config.workspaces);
    let disabled =
        normalize_manual_disabled_skill_ids(request.disabled, request.enabled, &discovery.skills)?;

    config.skills.directories.clear();
    config.skills.detected = discovery.skills;
    config.skills.disabled = disabled;
    refresh_derived_enabled_skills(&mut config);

    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
}

async fn refresh_skills(State(state): State<AppState>) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let discovery = discover_skills(&state.user_profile_dir, &config.workspaces);

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

async fn save_model_order(
    State(state): State<AppState>,
    Json(request): Json<ModelOrderRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;

    reorder_models(&mut config.models, request.model_ids)?;
    save_config(&state, config.clone())?;

    settings_response(&state, &config).await
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

async fn context_usage(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<ContextUsageRequest>,
) -> Result<Json<ContextUsageResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let prompt_context = prepare_prompt_context(
        &state,
        &config,
        &workspace_id,
        request.into_prompt_request(),
    )
    .await?;

    Ok(Json(context_usage_response(&prompt_context)?))
}

async fn answer_question(
    State(state): State<AppState>,
    AxumPath(question_id): AxumPath<String>,
    Json(answer): Json<QuestionAnswer>,
) -> Result<Json<QuestionAnswerResponse>, ApiError> {
    let question_id = question_id.trim().to_string();

    state.question_registry.answer(&question_id, answer)?;

    Ok(Json(QuestionAnswerResponse {
        ok: true,
        question_id,
    }))
}

async fn ai_statistics(
    State(state): State<AppState>,
    Query(query): Query<AiStatisticsQuery>,
) -> Result<Json<AiStatisticsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let filters = normalized_ai_statistics_query(query)?;
    let workspaces = ai_statistics_workspaces(&config, filters.workspace_id.as_deref())?;
    let mut requests = Vec::new();
    let mut total_count = 0_i64;
    let page_limit = filters
        .offset
        .checked_add(filters.page_size)
        .ok_or_else(|| ApiError::bad_request("AI statistics page limit is too large"))?;

    for workspace in workspaces {
        let database = WorkspaceDatabase::open_or_create(&workspace.path)
            .map_err(ApiError::from_workspace_error)?;
        let chat_titles = chat_title_map(&database)?;
        let audit_filters = LlmRequestAuditFilters {
            workspace_id: None,
            chat_id: filters.chat_id.as_deref(),
            provider_id: filters.provider_id.as_deref(),
            model_id: filters.model_id.as_deref(),
            final_state: filters.status.as_deref(),
            started_after: filters.started_after.as_deref(),
            started_before: filters.started_before.as_deref(),
            limit: Some(page_limit),
            offset: Some(0),
        };

        total_count += database
            .llm_request_audit_count(audit_filters)
            .map_err(ApiError::from_workspace_error)?;
        let rows = database
            .llm_request_audit_rows(audit_filters)
            .map_err(ApiError::from_workspace_error)?;

        requests.extend(
            rows.into_iter()
                .map(|row| ai_request_audit_summary(row, workspace, &chat_titles)),
        );
    }

    requests.sort_by(|left, right| {
        right
            .request_started_at
            .cmp(&left.request_started_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    let start = usize::try_from(filters.offset).expect("non-negative offset fits usize");
    let page_size = usize::try_from(filters.page_size).expect("positive page size fits usize");
    let requests = requests.into_iter().skip(start).take(page_size).collect();
    let total_pages = if total_count == 0 {
        0
    } else {
        (total_count + filters.page_size - 1) / filters.page_size
    };

    Ok(Json(AiStatisticsResponse {
        page: filters.page,
        page_size: filters.page_size,
        requests,
        total_count,
        total_pages,
    }))
}

async fn ai_statistics_detail(
    State(state): State<AppState>,
    AxumPath((workspace_id, request_id)): AxumPath<(String, String)>,
) -> Result<Json<AiRequestDetailResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let request_id = request_id.trim();

    if request_id.is_empty() {
        return Err(ApiError::bad_request("request id must not be empty"));
    }

    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let chat_titles = chat_title_map(&database)?;
    let request = database
        .llm_request(request_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("LLM request was not found: {request_id}")))?;
    let events = database
        .llm_request_events(request_id)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(ai_request_audit_event_summary)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(AiRequestDetailResponse {
        request: ai_request_audit_detail(request, workspace, &chat_titles)?,
        events,
    }))
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

    let llm_request_events = database
        .llm_request_events_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let mut messages = Vec::new();
    for message in database
        .messages_for_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?
    {
        if message.role != "user" && message.role != "assistant" {
            continue;
        }

        messages.push(chat_message_summary(
            &database,
            message,
            &llm_request_events,
        )?);
    }

    Ok(Json(ChatMessagesResponse { messages }))
}

async fn chat_task_graph(
    State(state): State<AppState>,
    AxumPath((workspace_id, chat_id)): AxumPath<(String, String)>,
    Query(query): Query<TaskGraphQuery>,
) -> Result<Json<TaskGraphResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_id = workspace_id.trim();
    let chat_id = chat_id.trim();
    let workspace = workspace_by_id(&config, workspace_id)?;
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

    let status = optional_trimmed_string(query.status);
    let task_id = optional_trimmed_string(query.task_id);
    let graph = database
        .filtered_task_graph(
            chat_id,
            TaskGraphFilter {
                status: status.as_deref(),
                task_id: task_id.as_deref(),
                include_subtasks: query.include_subtasks.unwrap_or(true),
            },
        )
        .map_err(ApiError::from_workspace_error)?;

    Ok(Json(task_graph_response(chat_id, graph)))
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

    Ok(Json(git_status_response(&workspace.path)?))
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

    Ok(Json(git_diff_response(&workspace.path, path)?))
}

async fn git_branches(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Json<GitBranchesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    if !is_git_workspace(&workspace.path)? {
        return Ok(Json(GitBranchesResponse {
            is_git_repository: false,
            current_branch: None,
            branches: Vec::new(),
        }));
    }

    Ok(Json(git_branches_response(&workspace.path)?))
}

async fn switch_git_branch(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitBranchRequest>,
) -> Result<Json<GitBranchesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    switch_git_branch_in_workspace(&workspace.path, request.name)?;

    Ok(Json(git_branches_response(&workspace.path)?))
}

async fn create_git_branch(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Json(request): Json<GitBranchRequest>,
) -> Result<Json<GitBranchesResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;

    create_git_branch_in_workspace(&workspace.path, request.name)?;

    Ok(Json(git_branches_response(&workspace.path)?))
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
    let terminal_shell = workspace.terminal_shell.clone();

    Ok(ws.on_upgrade(move |socket| {
        terminal::handle_terminal_socket(
            socket,
            shutdown_rx,
            registry,
            workspace_path,
            terminal_shell,
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
struct ManualWorkspaceRequest {
    id: String,
    name: String,
    path: String,
    pinned: bool,
    terminal_shell: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceLogoRequest {
    content_base64: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceOrderRequest {
    workspace_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectDirectoryResponse {
    path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectFilesResponse {
    files: Vec<NativeSelectedFile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeSelectedFile {
    path: String,
    name: String,
    content_type: String,
    size_bytes: u64,
    content_base64: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkspaceLogoKind {
    extension: &'static str,
    content_type: &'static str,
}

#[derive(Debug)]
struct WorkspaceLogoFile {
    path: PathBuf,
    kind: WorkspaceLogoKind,
    version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManualGeneralSettingsRequest {
    listen_host: String,
    listen_port: u32,
    language: String,
    hook_audit_enabled: Option<bool>,
    password: Option<String>,
    clear_password: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManualApiProxySettingsRequest {
    enabled: bool,
    proxy_type: String,
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthLoginRequest {
    password: String,
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
struct ModelOrderRequest {
    model_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManualProviderRequest {
    api_proxy: Option<ManualApiProxySettingsRequest>,
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
    disabled: Option<Vec<String>>,
    enabled: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HooksQuery {
    workspace_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveGlobalHooksRequest {
    config: HookConfig,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveWorkspaceHooksRequest {
    workspace_id: String,
    config: HookConfig,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportClaudeHooksRequest {
    workspace_id: Option<String>,
    target: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HookRunsQuery {
    limit: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestHookRequest {
    workspace_id: String,
    event: String,
    match_value: Option<String>,
    payload: Option<Value>,
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
    skill_ids: Option<Vec<String>>,
    message: String,
    #[serde(default)]
    attachments: Vec<ChatAttachmentInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContextUsageRequest {
    chat_id: Option<String>,
    model_id: String,
    thinking_level: Option<String>,
    skill_ids: Option<Vec<String>>,
    draft_message: Option<String>,
    #[serde(default)]
    attachments: Vec<ChatAttachmentInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatAttachmentInput {
    id: String,
    name: String,
    content_type: String,
    content_base64: Option<String>,
    path: Option<String>,
    size_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatAttachmentPart {
    id: String,
    name: String,
    content_type: String,
    size_bytes: u64,
    path: Option<String>,
    preview_data_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContextUsageResponse {
    used_message_tokens: u64,
    available_message_tokens: u64,
    usage_percent: u64,
    compression_trigger_tokens: u64,
    compression_trigger_percent: u64,
    will_compress_on_next_send: bool,
}

struct PromptContextRequest {
    chat_id: Option<String>,
    model_id: String,
    thinking_level: Option<String>,
    skill_ids: Option<Vec<String>>,
    message: Option<String>,
    attachments: Vec<ChatAttachmentInput>,
}

impl ChatStreamRequest {
    fn into_prompt_request(self) -> PromptContextRequest {
        PromptContextRequest {
            chat_id: self.chat_id,
            model_id: self.model_id,
            thinking_level: self.thinking_level,
            skill_ids: self.skill_ids,
            message: Some(self.message),
            attachments: self.attachments,
        }
    }
}

impl ContextUsageRequest {
    fn into_prompt_request(self) -> PromptContextRequest {
        PromptContextRequest {
            chat_id: self.chat_id,
            model_id: self.model_id,
            thinking_level: self.thinking_level,
            skill_ids: self.skill_ids,
            message: optional_trimmed_string(self.draft_message),
            attachments: self.attachments,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiStatisticsQuery {
    workspace_id: Option<String>,
    chat_id: Option<String>,
    provider_id: Option<String>,
    model_id: Option<String>,
    status: Option<String>,
    started_after: Option<String>,
    started_before: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
    limit: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitDiffQuery {
    path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskGraphQuery {
    status: Option<String>,
    task_id: Option<String>,
    include_subtasks: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitBranchRequest {
    name: String,
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
    general: GeneralSettingsSummary,
    workspaces: Vec<ConfiguredWorkspaceSummary>,
    terminal_shells: Vec<TerminalShellSummary>,
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
struct GeneralSettingsSummary {
    web_server: WebServerSettingsSummary,
    language: String,
    hook_audit_enabled: bool,
    supported_languages: Vec<AppLanguageSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiProxySettingsSummary {
    enabled: bool,
    proxy_type: String,
    url: String,
    supported_types: Vec<ApiProxyTypeSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiProxyTypeSummary {
    proxy_type: &'static str,
    label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebServerSettingsSummary {
    listen_host: String,
    listen_port: u16,
    password_enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppLanguageSummary {
    id: &'static str,
    name: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfiguredWorkspaceSummary {
    id: String,
    name: String,
    path: String,
    logo_url: Option<String>,
    pinned: bool,
    terminal_shell: String,
    is_default: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalShellSummary {
    shell: &'static str,
    label: &'static str,
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
    api_proxy: ApiProxySettingsSummary,
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
struct HooksSettingsResponse {
    supported_events: Vec<&'static str>,
    unsupported_events: Vec<&'static str>,
    global: HookConfigScopeSummary,
    workspace: HookConfigScopeSummary,
    effective: Vec<EffectiveHookSummary>,
    recent_runs: Vec<HookRunSummaryRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HookRunsResponse {
    runs: Vec<HookRunSummaryRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HookConfigScopeSummary {
    source: String,
    path: String,
    workspace_id: Option<String>,
    config: HookConfig,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HookRunSummaryRow {
    id: String,
    workspace_id: String,
    chat_id: Option<String>,
    run_id: Option<String>,
    tool_call_id: Option<String>,
    event: String,
    hook_source: String,
    handler_type: String,
    status: String,
    exit_code: Option<i64>,
    stdout_preview: Option<String>,
    stderr_preview: Option<String>,
    started_at: String,
    completed_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HookRunDetailResponse {
    run: HookRunDetail,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HookRunDetail {
    id: String,
    workspace_id: String,
    chat_id: Option<String>,
    run_id: Option<String>,
    tool_call_id: Option<String>,
    event: String,
    hook_source: String,
    handler_type: String,
    input: Value,
    output: Option<Value>,
    status: String,
    exit_code: Option<i64>,
    stdout_preview: Option<String>,
    stderr_preview: Option<String>,
    started_at: String,
    completed_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportClaudeHooksResponse {
    saved: bool,
    target: String,
    path: String,
    imported_files: Vec<String>,
    validation_errors: Vec<String>,
    config: HookConfig,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfiguredSkillSummary {
    key: String,
    id: String,
    name: String,
    description: String,
    path: String,
    scope: String,
    workspace_id: Option<String>,
    workspace_name: Option<String>,
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
    logo_url: Option<String>,
    pinned: bool,
    terminal_shell: String,
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
struct TaskGraphResponse {
    chat_id: String,
    exists: bool,
    tasks: Vec<TaskGraphTask>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiStatisticsResponse {
    page: i64,
    page_size: i64,
    requests: Vec<AiRequestAuditSummary>,
    total_count: i64,
    total_pages: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiRequestDetailResponse {
    request: AiRequestAuditDetail,
    events: Vec<AiRequestAuditEventSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiRequestAuditSummary {
    id: String,
    workspace_id: String,
    workspace_name: String,
    chat_id: Option<String>,
    chat_title: Option<String>,
    provider_id: String,
    model_id: String,
    request_started_at: String,
    first_token_at: Option<String>,
    completed_at: Option<String>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
    cache_ratio: Option<f64>,
    first_token_latency_ms: Option<i64>,
    total_latency_ms: Option<i64>,
    status_code: Option<i64>,
    final_state: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiRequestAuditDetail {
    id: String,
    workspace_id: String,
    workspace_name: String,
    chat_id: Option<String>,
    chat_title: Option<String>,
    provider_id: String,
    model_id: String,
    request_started_at: String,
    first_token_at: Option<String>,
    completed_at: Option<String>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
    cache_ratio: Option<f64>,
    first_token_latency_ms: Option<i64>,
    total_latency_ms: Option<i64>,
    status_code: Option<i64>,
    final_state: String,
    request_body: Option<Value>,
    response_body: Option<Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiRequestAuditEventSummary {
    id: String,
    sequence: i64,
    event_at: String,
    event_type: String,
    raw_chunk: Option<Value>,
    normalized_event: Value,
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
struct GitBranchesResponse {
    is_git_repository: bool,
    current_branch: Option<String>,
    branches: Vec<String>,
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
    parts: Vec<ChatMessagePart>,
    metrics: Option<ChatReplyMetrics>,
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
enum ChatMessagePart {
    Text { text: String },
    Reasoning { text: String },
    Attachment { attachment: ChatAttachmentPart },
    ToolCall { tool_call: ChatToolCallSummary },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatReplyMetrics {
    model_id: String,
    provider_id: String,
    total_latency_ms: Option<i64>,
    first_token_latency_ms: Option<i64>,
    output_tokens: Option<i64>,
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
    QuestionRequest {
        assistant_message_id: String,
        request: QuestionRequest,
    },
    HookNotification {
        assistant_message_id: String,
        notification: HookNotification,
    },
    GitDiffRefresh {
        workspace_id: String,
    },
    TaskGraphRefresh {
        workspace_id: String,
        chat_id: String,
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
        metrics: ChatReplyMetrics,
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
    hook_runtime: HookRuntime,
    global_hooks: HookConfig,
    question_registry: QuestionRegistry,
    app_shutdown_rx: watch::Receiver<bool>,
    context_budget: foco_agent::ContextBudget,
    request_body_json: String,
    compression_snapshots: Vec<ContextCompressionSnapshotRecord>,
    message_source_sequences: Vec<Option<i64>>,
    active_tool_start_index: usize,
    hook_context_messages: Vec<String>,
    hook_notifications: Vec<HookNotification>,
}

struct PreparedPromptContext {
    workspace_id: String,
    workspace_path: PathBuf,
    model_id: String,
    provider_id: String,
    provider_config: ProviderConnectionConfig,
    provider_request: NeutralChatRequest,
    context_budget: foco_agent::ContextBudget,
    compression_snapshots: Vec<ContextCompressionSnapshotRecord>,
    message_source_sequences: Vec<Option<i64>>,
    active_tool_start_index: usize,
    chat_id: Option<String>,
    raw_message: Option<String>,
    message: Option<String>,
    attachments: Vec<NeutralChatAttachment>,
    next_message_sequence: i64,
}

struct CapturedAuditEvent {
    event_at: String,
    event_type: String,
    normalized_event_json: String,
}

struct NormalizedAiStatisticsFilters {
    workspace_id: Option<String>,
    chat_id: Option<String>,
    provider_id: Option<String>,
    model_id: Option<String>,
    status: Option<String>,
    started_after: Option<String>,
    started_before: Option<String>,
    page: i64,
    page_size: i64,
    offset: i64,
}

struct ContextMessageGroup {
    message_indices: Vec<usize>,
    estimated_tokens: u64,
    must_keep: bool,
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

struct ToolHookOutcome {
    tool_call: ExecutedToolCall,
    hook_summary: HookRunSummary,
}

struct ToolExecutionWithHooks {
    execution: ToolExecution,
    hook_summary: HookRunSummary,
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
    status_code: Option<i64>,
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
            let mut app_shutdown_rx = self.app_shutdown_rx.clone();

            yield Ok(sse_event(&start_event));
            for event in self
                .hook_notifications
                .iter()
                .flat_map(|notification| {
                    [ChatSseEvent::HookNotification {
                        assistant_message_id: self.assistant_message_id.clone(),
                        notification: notification.clone(),
                    }]
                })
            {
                events.push(captured_event(&event));
                yield Ok(sse_event(&event));
            }
            self.hook_notifications.clear();
            append_hook_context_messages(
                &mut self.provider_request.messages,
                &mut self.message_source_sequences,
                &self.hook_context_messages,
            );
            self.hook_context_messages.clear();

            'agent_turns: for turn_index in 0..=MAX_AGENT_TOOL_ROUNDS {
                if *app_shutdown_rx.borrow() {
                    let event = finish_cancelled_chat_run(
                        &self,
                        &request_started_at,
                        started_at,
                        &mut events,
                        &executed_tool_calls,
                    )
                    .await;
                    yield Ok(sse_event(&event));
                    return;
                }

                let turn_active_tool_start_index = match ensure_context_compression(&mut self).await {
                    Ok(index) => index,
                    Err(error) => {
                        let message = error.message;
                        let event = ChatSseEvent::Error {
                            message: message.clone(),
                        };
                        events.push(captured_event(&event));
                        let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            None,
                        )
                        .await;

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
                for notification in std::mem::take(&mut self.hook_notifications) {
                    let event = ChatSseEvent::HookNotification {
                        assistant_message_id: self.assistant_message_id.clone(),
                        notification,
                    };
                    events.push(captured_event(&event));
                    yield Ok(sse_event(&event));
                }
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
                        let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            None,
                        )
                        .await;

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
                        let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            None,
                        )
                        .await;

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
                let mut provider_stream = match tokio::select! {
                    changed = app_shutdown_rx.changed() => {
                        if changed.is_err() || *app_shutdown_rx.borrow() {
                            let event = finish_cancelled_chat_run(
                                &self,
                                &request_started_at,
                                started_at,
                                &mut events,
                                &executed_tool_calls,
                            )
                            .await;
                            yield Ok(sse_event(&event));
                            return;
                        }
                        continue;
                    }
                    provider_stream = stream_chat(&self.provider_config, turn_request) => provider_stream,
                } {
                    Ok(provider_stream) => provider_stream,
                    Err(error) => {
                        let status_code = provider_status_code(&error);
                        let message = error.to_string();
                        let event = ChatSseEvent::Error {
                            message: message.clone(),
                        };
                        events.push(captured_event(&event));
                        let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            status_code,
                        )
                        .await;

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

                loop {
                    let Some(event_result) = (tokio::select! {
                        changed = app_shutdown_rx.changed() => {
                            if changed.is_err() || *app_shutdown_rx.borrow() {
                                let event = finish_cancelled_chat_run(
                                    &self,
                                    &request_started_at,
                                    started_at,
                                    &mut events,
                                    &executed_tool_calls,
                                )
                                .await;
                                yield Ok(sse_event(&event));
                                return;
                            }
                            continue;
                        }
                        event_result = provider_stream.next_event() => event_result,
                    }) else {
                        break;
                    };
                    let provider_event = match event_result {
                        Ok(provider_event) => provider_event,
                        Err(error) => {
                            let status_code = provider_status_code(&error);
                            let message = error.to_string();
                            let event = ChatSseEvent::Error {
                                message: message.clone(),
                            };
                            events.push(captured_event(&event));
                            let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            status_code,
                        )
                        .await;

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
                                let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            None,
                        )
                        .await;

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
                                let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            None,
                        )
                        .await;

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
                                let stop_text = assistant_message_text.clone();
                                let stop_summary = self.hook_runtime.run_hooks(HookRunRequest {
                                    global_config: &self.global_hooks,
                                    workspace_id: &self.workspace_id,
                                    workspace_path: &self.workspace_path,
                                    event: "Stop",
                                    match_value: None,
                                    chat_id: Some(&self.chat_id),
                                    run_id: Some(&self.llm_request_id),
                                    session_id: Some(&self.chat_id),
                                    tool_call_id: None,
                                    model_id: Some(&self.model_id),
                                    provider_id: Some(&self.provider_id),
                                    provider_config: Some(&self.provider_config),
                                    permission_mode: None,
                                    payload: json!({
                                        "text": stop_text,
                                        "reasoning": non_empty_string(&assistant_reasoning),
                                        "usage": final_usage.clone(),
                                        "stopReason": stop_reason.clone(),
                                    }),
                                }).await;
                                for event in hook_notification_events(&self.assistant_message_id, "Stop", &stop_summary) {
                                    events.push(captured_event(&event));
                                    yield Ok(sse_event(&event));
                                }
                                if let Some(reason) = stop_summary.first_block_reason() {
                                    append_hook_context_messages(
                                        &mut self.provider_request.messages,
                                        &mut self.message_source_sequences,
                                        &[
                                            format!("Stop hook blocked the assistant response: {reason}"),
                                            stop_summary.additional_context.join("\n"),
                                        ],
                                    );
                                    continue 'agent_turns;
                                }
                                let total_latency_ms = elapsed_millis(started_at);
                                let metrics = ChatReplyMetrics {
                                    model_id: self.model_id.clone(),
                                    provider_id: self.provider_id.clone(),
                                    total_latency_ms: Some(total_latency_ms),
                                    first_token_latency_ms,
                                    output_tokens: final_usage.as_ref().and_then(|usage| usage.output_tokens),
                                };
                                let complete_event = ChatSseEvent::Complete {
                                    chat_id: self.chat_id.clone(),
                                    assistant_message_id: self.assistant_message_id.clone(),
                                    text: assistant_message_text.clone(),
                                    reasoning: non_empty_string(&assistant_reasoning),
                                    usage: final_usage.clone(),
                                    stop_reason: stop_reason.clone(),
                                    metrics,
                                };
                                let session_end_summary = session_end_hook(
                                    &self,
                                    "succeeded",
                                    json!({
                                        "text": assistant_message_text.clone(),
                                        "reasoning": non_empty_string(&assistant_reasoning),
                                        "usage": final_usage.clone(),
                                        "stopReason": stop_reason.clone(),
                                    }),
                                ).await;
                                for event in hook_notification_events(&self.assistant_message_id, "SessionEnd", &session_end_summary) {
                                    events.push(captured_event(&event));
                                    yield Ok(sse_event(&event));
                                }
                                events.push(captured_event(&complete_event));
                                let completed_at = utc_timestamp();
                                let outcome = ChatAuditOutcome {
                                    first_token_at,
                                    completed_at,
                                    first_token_latency_ms,
                                    total_latency_ms,
                                    input_tokens: final_usage.as_ref().and_then(|usage| usage.input_tokens),
                                    output_tokens: final_usage.as_ref().and_then(|usage| usage.output_tokens),
                                    cache_read_tokens: final_usage.as_ref().and_then(|usage| usage.cache_read_tokens),
                                    cache_write_tokens: final_usage.as_ref().and_then(|usage| usage.cache_write_tokens),
                                    status_code: Some(200),
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
                                let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            None,
                        )
                        .await;

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
                                let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            None,
                        )
                        .await;

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
                                seen_tool_call_ids.insert(tool_call.call_id.clone());
                                let event = ChatSseEvent::ToolCall {
                                    assistant_message_id: self.assistant_message_id.clone(),
                                    tool_call: pending_tool_call_summary(tool_call),
                                };
                                events.push(captured_event(&event));
                                yield Ok(sse_event(&event));
                            }

                            let next_tool_results = match {
                                let (question_event_tx, mut question_event_rx) = mpsc::unbounded_channel();
                                let tool_results = execute_tool_calls_parallel(
                                    self.mcp_registry.clone(),
                                    self.hook_runtime.clone(),
                                    self.global_hooks.clone(),
                                    self.provider_config.clone(),
                                    self.question_registry.clone(),
                                    question_event_tx,
                                    &self.workspace_id,
                                    &self.workspace_path,
                                    &self.chat_id,
                                    &self.llm_request_id,
                                    &self.model_id,
                                    &self.provider_id,
                                    tool_calls.clone(),
                                );
                                tokio::pin!(tool_results);
                                let mut question_events_open = true;

                                loop {
                                    let next = tokio::select! {
                                        changed = app_shutdown_rx.changed() => {
                                            if changed.is_err() || *app_shutdown_rx.borrow() {
                                                let event = finish_cancelled_chat_run(
                                                    &self,
                                                    &request_started_at,
                                                    started_at,
                                                    &mut events,
                                                    &executed_tool_calls,
                                                )
                                                .await;
                                                yield Ok(sse_event(&event));
                                                return;
                                            }
                                            None
                                        }
                                        question_request = question_event_rx.recv(), if question_events_open => {
                                            match question_request {
                                                Some(question_request) => Some(question_request),
                                                None => {
                                                    question_events_open = false;
                                                    None
                                                }
                                            }
                                        }
                                        tool_results = &mut tool_results => break tool_results,
                                    };

                                    if let Some(question_request) = next {
                                        let event = ChatSseEvent::QuestionRequest {
                                            assistant_message_id: self.assistant_message_id.clone(),
                                            request: question_request,
                                        };
                                        events.push(captured_event(&event));
                                        yield Ok(sse_event(&event));
                                    }
                                }
                            } {
                                Ok(tool_results) => tool_results,
                                Err(error) => {
                                    let message = error.message;
                                    let event = ChatSseEvent::Error {
                                        message: message.clone(),
                                    };
                                    events.push(captured_event(&event));
                                    let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            None,
                        )
                        .await;

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
                            let mut next_executed_tool_calls = Vec::with_capacity(next_tool_results.len());
                            let mut batch_hook_summary = HookRunSummary::default();
                            for outcome in next_tool_results {
                                for event in hook_notification_events(&self.assistant_message_id, "ToolHook", &outcome.hook_summary) {
                                    events.push(captured_event(&event));
                                    yield Ok(sse_event(&event));
                                }
                                merge_hook_summaries(&mut batch_hook_summary, outcome.hook_summary);
                                next_executed_tool_calls.push(outcome.tool_call);
                            }
                            let batch_summary = self.hook_runtime.run_hooks(HookRunRequest {
                                global_config: &self.global_hooks,
                                workspace_id: &self.workspace_id,
                                workspace_path: &self.workspace_path,
                                event: "PostToolBatch",
                                match_value: None,
                                chat_id: Some(&self.chat_id),
                                run_id: Some(&self.llm_request_id),
                                session_id: Some(&self.chat_id),
                                tool_call_id: None,
                                model_id: Some(&self.model_id),
                                provider_id: Some(&self.provider_id),
                                provider_config: Some(&self.provider_config),
                                permission_mode: None,
                                payload: json!({
                                    "toolResults": next_executed_tool_calls.clone(),
                                }),
                            }).await;
                            for event in hook_notification_events(&self.assistant_message_id, "PostToolBatch", &batch_summary) {
                                events.push(captured_event(&event));
                                yield Ok(sse_event(&event));
                            }
                            merge_hook_summaries(&mut batch_hook_summary, batch_summary);
                            append_hook_context_messages(
                                &mut self.provider_request.messages,
                                &mut self.message_source_sequences,
                                &batch_hook_summary.additional_context,
                            );
                            for executed_tool_call in &next_executed_tool_calls {
                                let result_event = ChatSseEvent::ToolResult {
                                    assistant_message_id: self.assistant_message_id.clone(),
                                    tool_call_id: executed_tool_call.id.clone(),
                                    output: executed_tool_call.output.clone(),
                                    is_error: executed_tool_call.is_error,
                                };
                                events.push(captured_event(&result_event));
                                yield Ok(sse_event(&result_event));
                            }
                            if tool_results_affect_git_diff(&next_executed_tool_calls) {
                                let event = ChatSseEvent::GitDiffRefresh {
                                    workspace_id: self.workspace_id.clone(),
                                };
                                events.push(captured_event(&event));
                                yield Ok(sse_event(&event));
                            }
                            if tool_results_affect_task_graph(&next_executed_tool_calls) {
                                let event = ChatSseEvent::TaskGraphRefresh {
                                    workspace_id: self.workspace_id.clone(),
                                    chat_id: self.chat_id.clone(),
                                };
                                events.push(captured_event(&event));
                                yield Ok(sse_event(&event));
                            }

                            append_tool_state_messages(
                                &mut self.provider_request.messages,
                                &mut self.message_source_sequences,
                                tool_calls,
                                &next_executed_tool_calls,
                                turn_text,
                                non_empty_string(&turn_reasoning),
                            );
                            executed_tool_calls.extend(next_executed_tool_calls);

                            break;
                        }
                        NeutralChatStreamEvent::Error { message } => {
                            let event = ChatSseEvent::Error {
                                message: message.clone(),
                            };
                            events.push(captured_event(&event));
                            let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            None,
                        )
                        .await;

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
                let outcome = failed_chat_audit_outcome(
                            &self,
                            started_at,
                            &mut events,
                            &message,
                            None,
                        )
                        .await;

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

async fn prepare_prompt_context(
    state: &AppState,
    config: &GlobalConfig,
    workspace_id: &str,
    request: PromptContextRequest,
) -> Result<PreparedPromptContext, ApiError> {
    let workspace_id = workspace_id.trim();
    let model_id = request.model_id.trim();
    let thinking_level = optional_trimmed_string(request.thinking_level);
    let requested_skill_ids = request.skill_ids;
    let raw_message = optional_trimmed_string(request.message);
    let attachments = normalized_chat_attachments(request.attachments)?;

    if workspace_id.is_empty() {
        return Err(ApiError::bad_request("workspace id must not be empty"));
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
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let has_user_turn = raw_message.is_some() || !attachments.is_empty();
    let message = if has_user_turn {
        Some(message_with_selected_skills(
            &state.user_profile_dir,
            config,
            &workspace.id,
            requested_skill_ids,
            raw_message.as_deref().unwrap_or(""),
        )?)
    } else {
        None
    };
    let chat_id = optional_trimmed_string(request.chat_id);
    let is_new_chat = chat_id.is_none();
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
            Some(chat_id)
        }
        None => None,
    };
    let existing_messages = match chat_id.as_deref() {
        Some(chat_id) => database
            .messages_for_chat(chat_id)
            .map_err(ApiError::from_workspace_error)?,
        None => Vec::new(),
    };
    let compression_snapshots = match chat_id.as_deref() {
        Some(chat_id) => database
            .context_compression_snapshots_for_chat(chat_id)
            .map_err(ApiError::from_workspace_error)?,
        None => Vec::new(),
    };
    let user_sequence = next_message_sequence(&existing_messages);

    let builtin_tool_definitions = builtin_tool_definitions();
    let mut neutral_tools = builtin_tool_definitions
        .iter()
        .cloned()
        .map(neutral_tool_definition)
        .collect::<Vec<_>>();
    neutral_tools.extend(mcp_tools.iter().map(neutral_mcp_tool_definition));
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
    let agents_messages = if is_new_chat {
        agents_prompt_messages(&workspace.path, &state.user_profile_dir)?
    } else {
        Vec::new()
    };
    let environment_messages = if is_new_chat {
        vec![environment_context_message(&workspace.path)?]
    } else {
        Vec::new()
    };
    let skill_messages = if is_new_chat {
        enabled_skill_frontmatter_messages(&state.user_profile_dir, config, &workspace.id)?
    } else {
        Vec::new()
    };
    let mut neutral_messages = Vec::with_capacity(
        existing_messages.len()
            + compression_snapshots.len()
            + agents_messages.len()
            + environment_messages.len()
            + skill_messages.len()
            + 2,
    );
    let mut message_source_sequences = Vec::with_capacity(neutral_messages.capacity());
    neutral_messages.push(neutral_text_message(NeutralChatRole::System, system_prompt));
    message_source_sequences.push(None);
    for snapshot in &compression_snapshots {
        neutral_messages.push(compression_snapshot_message(snapshot));
        message_source_sequences.push(None);
    }
    for agents_message in agents_messages {
        neutral_messages.push(agents_message);
        message_source_sequences.push(None);
    }
    for environment_message in environment_messages {
        neutral_messages.push(environment_message);
        message_source_sequences.push(None);
    }
    for skill_message in skill_messages {
        neutral_messages.push(skill_message);
        message_source_sequences.push(None);
    }
    for existing_message in existing_messages {
        if covered_sequences.contains(&existing_message.sequence) {
            continue;
        }

        let sequence = existing_message.sequence;
        for neutral_message in neutral_messages_from_record(&database, existing_message)? {
            neutral_messages.push(neutral_message);
            message_source_sequences.push(Some(sequence));
        }
    }
    if message.is_some() || !attachments.is_empty() {
        neutral_messages.push(neutral_user_message(
            message.clone().unwrap_or_default(),
            attachments.clone(),
        ));
        message_source_sequences.push(Some(user_sequence));
    }
    let active_tool_start_index = neutral_messages.len();

    let provider_request = NeutralChatRequest {
        model_id: model.id.clone(),
        messages: neutral_messages,
        tools: neutral_tools,
        thinking_level: thinking_level.or_else(|| model.thinking_level.clone()),
        max_output_tokens: Some(max_output_tokens),
    };
    Ok(PreparedPromptContext {
        workspace_id: workspace.id.clone(),
        workspace_path: workspace.path.clone(),
        chat_id,
        provider_id: provider.id.clone(),
        model_id: model.id.clone(),
        provider_config,
        provider_request,
        context_budget,
        compression_snapshots,
        message_source_sequences,
        active_tool_start_index,
        raw_message,
        message,
        attachments,
        next_message_sequence: user_sequence,
    })
}

async fn prepare_chat_context(
    state: &AppState,
    config: &GlobalConfig,
    workspace_id: &str,
    request: ChatStreamRequest,
) -> Result<PreparedChatContext, ApiError> {
    let prompt_context =
        prepare_prompt_context(state, config, workspace_id, request.into_prompt_request()).await?;
    let raw_message = prompt_context.raw_message.as_deref().unwrap_or("");
    let message = prompt_context
        .message
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("message must not be empty"))?;
    let user_sequence = prompt_context.next_message_sequence;
    let assistant_sequence = user_sequence + 1;
    let user_message_id = unique_id("msg-user");
    let assistant_message_id = unique_id("msg-assistant");
    let llm_request_id = unique_id("llm");
    let user_prompt_summary = state
        .hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &config.hooks,
            workspace_id: &prompt_context.workspace_id,
            workspace_path: &prompt_context.workspace_path,
            event: "UserPromptSubmit",
            match_value: None,
            chat_id: prompt_context.chat_id.as_deref(),
            run_id: None,
            session_id: prompt_context.chat_id.as_deref(),
            tool_call_id: None,
            model_id: Some(&prompt_context.model_id),
            provider_id: Some(&prompt_context.provider_id),
            provider_config: Some(&prompt_context.provider_config),
            permission_mode: None,
            payload: json!({
                "prompt": raw_message,
                "message": message,
                "attachments": chat_attachment_hook_summaries(&prompt_context.attachments),
            }),
        })
        .await;
    if let Some(reason) = user_prompt_summary.first_block_reason() {
        return Err(ApiError::bad_request(format!(
            "UserPromptSubmit hook blocked message: {reason}"
        )));
    }
    let mut database = WorkspaceDatabase::open_or_create(&prompt_context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let (chat_id, chat_created) = match prompt_context.chat_id.clone() {
        Some(chat_id) => (chat_id, false),
        None => {
            let chat_id = unique_id("chat");
            database
                .insert_chat(
                    &chat_id,
                    &chat_title_for_prompt(raw_message, &prompt_context.attachments),
                )
                .map_err(ApiError::from_workspace_error)?;
            (chat_id, true)
        }
    };
    let session_start_summary = state
        .hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &config.hooks,
            workspace_id: &prompt_context.workspace_id,
            workspace_path: &prompt_context.workspace_path,
            event: "SessionStart",
            match_value: None,
            chat_id: Some(&chat_id),
            run_id: Some(&llm_request_id),
            session_id: Some(&chat_id),
            tool_call_id: None,
            model_id: Some(&prompt_context.model_id),
            provider_id: Some(&prompt_context.provider_id),
            provider_config: Some(&prompt_context.provider_config),
            permission_mode: None,
            payload: json!({
                "chatCreated": chat_created,
                "prompt": raw_message,
                "attachments": chat_attachment_hook_summaries(&prompt_context.attachments),
            }),
        })
        .await;
    let mut hook_notifications = user_prompt_summary.hook_messages("UserPromptSubmit");
    hook_notifications.extend(session_start_summary.hook_messages("SessionStart"));
    let mut hook_context_messages = user_prompt_summary.additional_context;
    hook_context_messages.extend(session_start_summary.additional_context);
    let user_metadata_json = user_message_metadata_json(&prompt_context.attachments)?;

    database
        .insert_message(NewMessage {
            id: &user_message_id,
            chat_id: &chat_id,
            role: "user",
            content: message,
            sequence: user_sequence,
            metadata_json: Some(&user_metadata_json),
        })
        .map_err(ApiError::from_workspace_error)?;

    let request_body_json = serialize_provider_request(&prompt_context.provider_request)?;

    Ok(PreparedChatContext {
        workspace_id: prompt_context.workspace_id,
        workspace_path: prompt_context.workspace_path,
        chat_id,
        provider_id: prompt_context.provider_id,
        model_id: prompt_context.model_id,
        user_message_id,
        assistant_message_id,
        llm_request_id,
        assistant_sequence,
        provider_config: prompt_context.provider_config,
        provider_request: prompt_context.provider_request,
        mcp_registry: state.mcp_registry.clone(),
        hook_runtime: state.hook_runtime.clone(),
        global_hooks: config.hooks.clone(),
        question_registry: state.question_registry.clone(),
        app_shutdown_rx: state.app_shutdown_rx.clone(),
        context_budget: prompt_context.context_budget,
        request_body_json,
        compression_snapshots: prompt_context.compression_snapshots,
        message_source_sequences: prompt_context.message_source_sequences,
        active_tool_start_index: prompt_context.active_tool_start_index,
        hook_context_messages,
        hook_notifications,
    })
}

fn neutral_messages_from_record(
    database: &WorkspaceDatabase,
    message: MessageRecord,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
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

    if role != NeutralChatRole::Assistant && role != NeutralChatRole::Tool {
        let attachments = if role == NeutralChatRole::User {
            message_attachments_from_metadata(&message.metadata_json)?
        } else {
            Vec::new()
        };
        if role == NeutralChatRole::User {
            return Ok(vec![neutral_user_message(message.content, attachments)]);
        }

        return Ok(vec![NeutralChatMessage {
            role,
            content: message.content,
            attachments,
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
        }]);
    }

    if role == NeutralChatRole::Assistant {
        let reasoning = if role == NeutralChatRole::Assistant {
            assistant_reasoning_from_metadata(&message.metadata_json)?
        } else {
            None
        };
        let tool_calls = database
            .tool_calls_for_message(&message.id)
            .map_err(ApiError::from_workspace_error)?;

        if tool_calls.is_empty() {
            return Ok(vec![NeutralChatMessage {
                role,
                content: message.content,
                attachments: Vec::new(),
                reasoning,
                tool_calls: Vec::new(),
                tool_call_id: None,
                tool_name: None,
            }]);
        }

        let mut messages = Vec::with_capacity(tool_calls.len() * 2 + 1);
        for tool_call in tool_calls {
            let result = tool_call.result.clone().ok_or_else(|| {
                ApiError::bad_request(format!(
                    "assistant message '{}' tool call '{}' is missing a tool result",
                    message.id, tool_call.id
                ))
            })?;

            messages.push(NeutralChatMessage {
                role: NeutralChatRole::Assistant,
                content: String::new(),
                attachments: Vec::new(),
                reasoning: None,
                tool_calls: vec![neutral_tool_call_from_record(&tool_call)?],
                tool_call_id: None,
                tool_name: None,
            });
            messages.push(NeutralChatMessage {
                role: NeutralChatRole::Tool,
                content: result.output_json,
                attachments: Vec::new(),
                reasoning: None,
                tool_calls: Vec::new(),
                tool_call_id: Some(tool_call.id),
                tool_name: Some(tool_call.tool_name),
            });
        }

        if !message.content.trim().is_empty() {
            messages.push(NeutralChatMessage {
                role: NeutralChatRole::Assistant,
                content: message.content,
                attachments: Vec::new(),
                reasoning,
                tool_calls: Vec::new(),
                tool_call_id: None,
                tool_name: None,
            });
        }

        return Ok(messages);
    }

    if role != NeutralChatRole::Tool {
        return Err(ApiError::internal(
            "unsupported neutral message role while rebuilding chat history",
        ));
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

    Ok(vec![NeutralChatMessage {
        role,
        content: message.content,
        attachments: Vec::new(),
        reasoning: None,
        tool_calls: Vec::new(),
        tool_call_id: Some(tool_call_id),
        tool_name,
    }])
}

fn neutral_tool_call_from_record(
    record: &ToolCallWithResultRecord,
) -> Result<NeutralToolCall, ApiError> {
    Ok(NeutralToolCall {
        call_id: record.id.clone(),
        name: record.tool_name.clone(),
        arguments: parse_json_value(&record.input_json, "tool call input")?,
        thought_signatures: None,
    })
}

fn neutral_tool_message_from_executed_tool_call(
    tool_result: &ExecutedToolCall,
) -> NeutralChatMessage {
    NeutralChatMessage {
        role: NeutralChatRole::Tool,
        content: serde_json::to_string(&tool_result.output)
            .expect("tool outputs are always JSON serializable"),
        attachments: Vec::new(),
        reasoning: None,
        tool_calls: Vec::new(),
        tool_call_id: Some(tool_result.id.clone()),
        tool_name: Some(tool_result.name.clone()),
    }
}

fn neutral_assistant_tool_call_message(
    tool_call: NeutralToolCall,
    assistant_text: String,
    assistant_reasoning: Option<String>,
) -> NeutralChatMessage {
    NeutralChatMessage {
        role: NeutralChatRole::Assistant,
        content: assistant_text,
        attachments: Vec::new(),
        reasoning: assistant_reasoning,
        tool_calls: vec![tool_call],
        tool_call_id: None,
        tool_name: None,
    }
}

fn interleaved_tool_state_messages(
    tool_calls: Vec<NeutralToolCall>,
    tool_results: &[ExecutedToolCall],
    assistant_text: String,
    assistant_reasoning: Option<String>,
) -> Vec<NeutralChatMessage> {
    let mut messages = Vec::with_capacity(tool_calls.len() * 2);
    let mut assistant_text = Some(assistant_text);
    let mut assistant_reasoning = assistant_reasoning;

    for tool_call in tool_calls {
        messages.push(neutral_assistant_tool_call_message(
            tool_call.clone(),
            assistant_text.take().unwrap_or_default(),
            assistant_reasoning.take(),
        ));

        let tool_result = tool_results
            .iter()
            .find(|tool_result| tool_result.id == tool_call.call_id)
            .expect("executed tool results must match completed tool calls");
        messages.push(neutral_tool_message_from_executed_tool_call(tool_result));
    }

    messages
}

fn agents_prompt_messages(
    workspace_path: &Path,
    user_profile_dir: &Path,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    let mut messages = Vec::new();

    for path in [
        workspace_path.join("AGENTS.md"),
        user_profile_dir.join(".codex").join("AGENTS.md"),
    ] {
        if let Some(message) = agents_prompt_message(&path)? {
            messages.push(message);
        }
    }

    Ok(messages)
}

fn agents_prompt_message(path: &Path) -> Result<Option<NeutralChatMessage>, ApiError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ApiError::internal(format!(
                "failed to inspect {}: {source}",
                path.display()
            )));
        }
    };

    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "AGENTS.md path is not a file: {}",
            path.display()
        )));
    }

    let content = fs::read_to_string(path).map_err(|source| {
        ApiError::internal(format!("failed to read {}: {source}", path.display()))
    })?;

    if content.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(neutral_text_message(
        NeutralChatRole::User,
        format!(
            "{AGENTS_MESSAGE_PREFIX} {}:\n\n{}",
            path.display(),
            content.trim()
        ),
    )))
}

fn environment_context_message(workspace_path: &Path) -> Result<NeutralChatMessage, ApiError> {
    let now = Local::now();
    let shell = detected_shell()?;
    let wsl = is_wsl_environment();

    Ok(neutral_text_message(
        NeutralChatRole::User,
        format!(
            "{ENVIRONMENT_CONTEXT_MESSAGE_PREFIX}:\n\
             - workspace directory: {}\n\
             - shell type: {}\n\
             - shell executable: {}\n\
             - current date: {}\n\
             - local timestamp: {}\n\
             - time zone: {}\n\
             - wsl: {}",
            workspace_path.display(),
            shell.kind,
            shell.executable,
            now.format("%Y-%m-%d"),
            now.to_rfc3339_opts(SecondsFormat::Secs, false),
            now.offset(),
            wsl
        ),
    ))
}

struct DetectedShell {
    kind: String,
    executable: String,
}

fn detected_shell() -> Result<DetectedShell, ApiError> {
    if cfg!(windows) {
        return Ok(DetectedShell {
            kind: "powershell".to_string(),
            executable: "powershell.exe".to_string(),
        });
    }

    let shell = env::var("SHELL").map_err(|source| {
        ApiError::internal(format!(
            "failed to detect shell from SHELL environment: {source}"
        ))
    })?;
    let shell = non_empty_string(shell.trim()).ok_or_else(|| {
        ApiError::bad_request("SHELL environment variable is empty; cannot detect shell type")
    })?;
    let kind = Path::new(&shell)
        .file_stem()
        .and_then(|name| name.to_str())
        .and_then(non_empty_string)
        .ok_or_else(|| {
            ApiError::bad_request(format!("failed to detect shell type from SHELL={shell}"))
        })?;

    Ok(DetectedShell {
        kind,
        executable: shell,
    })
}

fn is_wsl_environment() -> bool {
    if env::var_os("WSL_DISTRO_NAME").is_some() || env::var_os("WSL_INTEROP").is_some() {
        return true;
    }

    if !cfg!(target_os = "linux") {
        return false;
    }

    fs::read_to_string("/proc/version")
        .map(|version| version.to_ascii_lowercase().contains("microsoft"))
        .unwrap_or(false)
}

async fn ensure_context_compression(context: &mut PreparedChatContext) -> Result<usize, ApiError> {
    if context.provider_request.messages.len() != context.message_source_sequences.len() {
        return Err(ApiError::internal(
            "context message source sequence count does not match prompt message count",
        ));
    }

    let message_groups = context_message_groups(
        &context.provider_request.messages,
        &context.message_source_sequences,
        context.active_tool_start_index,
    )?;
    let pack_items = pack_items_from_message_groups(&message_groups);

    let Some(plan) = plan_context_compression(
        &pack_items,
        context.context_budget.available_message_tokens,
        active_tool_start_group_index(&message_groups, context.active_tool_start_index),
        CONTEXT_COMPRESSION_PRESERVE_RECENT_MESSAGES,
    ) else {
        return Ok(context.active_tool_start_index);
    };
    let covered_indices = message_group_indices(&message_groups, &plan.covered_indices)?;

    let summary = context_compression_summary(
        &context.provider_request.messages,
        &context.message_source_sequences,
        &covered_indices,
    )?;
    let summary_token_count = estimate_text_tokens(&summary);

    if summary_token_count >= plan.original_tokens {
        return Ok(context.active_tool_start_index);
    }

    let covered_sequences =
        compression_covered_sequences(&context.message_source_sequences, &covered_indices)?;
    let pre_summary = context
        .hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &context.global_hooks,
            workspace_id: &context.workspace_id,
            workspace_path: &context.workspace_path,
            event: "PreCompact",
            match_value: None,
            chat_id: Some(&context.chat_id),
            run_id: Some(&context.llm_request_id),
            session_id: Some(&context.chat_id),
            tool_call_id: None,
            model_id: Some(&context.model_id),
            provider_id: Some(&context.provider_id),
            provider_config: Some(&context.provider_config),
            permission_mode: None,
            payload: json!({
                "coveredSequences": covered_sequences,
                "originalTokenCount": plan.original_tokens,
                "summaryTokenCount": summary_token_count,
                "summary": summary.clone(),
            }),
        })
        .await;
    context
        .hook_notifications
        .extend(pre_summary.hook_messages("PreCompact"));
    append_hook_context_messages(
        &mut context.provider_request.messages,
        &mut context.message_source_sequences,
        &pre_summary.additional_context,
    );
    if pre_summary.first_block_reason().is_some() {
        return Ok(context.active_tool_start_index);
    }
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
        &covered_indices,
        compression_snapshot_message(&snapshot),
    );
    context.message_source_sequences = replace_covered_sequences_with_snapshot(
        &context.message_source_sequences,
        &covered_indices,
    );
    context.active_tool_start_index =
        compressed_active_tool_start_index(context.active_tool_start_index, &covered_indices);
    context.compression_snapshots.push(snapshot);

    let post_summary = context
        .hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &context.global_hooks,
            workspace_id: &context.workspace_id,
            workspace_path: &context.workspace_path,
            event: "PostCompact",
            match_value: None,
            chat_id: Some(&context.chat_id),
            run_id: Some(&context.llm_request_id),
            session_id: Some(&context.chat_id),
            tool_call_id: None,
            model_id: Some(&context.model_id),
            provider_id: Some(&context.provider_id),
            provider_config: Some(&context.provider_config),
            permission_mode: None,
            payload: json!({
                "snapshotId": context.compression_snapshots.last().map(|snapshot| snapshot.id.clone()),
            }),
        })
        .await;
    context
        .hook_notifications
        .extend(post_summary.hook_messages("PostCompact"));
    append_hook_context_messages(
        &mut context.provider_request.messages,
        &mut context.message_source_sequences,
        &post_summary.additional_context,
    );

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

    if !message.attachments.is_empty() {
        let names = message
            .attachments
            .iter()
            .map(|attachment| attachment.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if content.is_empty() {
            content = format!("attachments: {names}");
        } else {
            content.push_str("; attachments: ");
            content.push_str(&names);
        }
    }

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
            status_code: outcome.status_code,
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
        attachments: Vec::new(),
        reasoning: None,
        tool_calls: Vec::new(),
        tool_call_id: None,
        tool_name: None,
    }
}

fn neutral_user_message(
    content: String,
    attachments: Vec<NeutralChatAttachment>,
) -> NeutralChatMessage {
    let content = user_message_with_attachment_paths(&content, &attachments);

    NeutralChatMessage {
        role: NeutralChatRole::User,
        content,
        attachments,
        reasoning: None,
        tool_calls: Vec::new(),
        tool_call_id: None,
        tool_name: None,
    }
}

fn user_message_with_attachment_paths(
    content: &str,
    attachments: &[NeutralChatAttachment],
) -> String {
    let path_attachments = attachments
        .iter()
        .filter_map(|attachment| {
            attachment.path.as_ref().map(|path| {
                (
                    markdown_safe_single_line(&attachment.name),
                    markdown_safe_single_line(path),
                )
            })
        })
        .collect::<Vec<_>>();

    if path_attachments.is_empty() {
        return content.to_string();
    }

    let mut message = String::from("# Files mentioned by the user:\n\n");
    for (name, path) in path_attachments {
        message.push_str("## ");
        message.push_str(&name);
        message.push_str(": ");
        message.push_str(&path);
        message.push_str("\n\n");
    }
    message.push_str("## My request for Foco:\n");
    message.push_str(content);

    message
}

fn markdown_safe_single_line(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_string()
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

fn context_message_groups(
    messages: &[NeutralChatMessage],
    message_source_sequences: &[Option<i64>],
    active_tool_start_index: usize,
) -> Result<Vec<ContextMessageGroup>, ApiError> {
    if messages.len() != message_source_sequences.len() {
        return Err(ApiError::internal(
            "context message source sequence count does not match prompt message count",
        ));
    }

    let latest_user_index = messages
        .iter()
        .rposition(|message| message.role == NeutralChatRole::User);
    let mut groups = Vec::new();
    let mut index = 0;

    while index < messages.len() {
        let source_sequence = message_source_sequences[index];
        let mut message_indices = vec![index];
        index += 1;

        if source_sequence.is_some() {
            while index < messages.len() && message_source_sequences[index] == source_sequence {
                message_indices.push(index);
                index += 1;
            }
        }

        let estimated_tokens = message_indices
            .iter()
            .map(|message_index| {
                if *message_index == 0 {
                    0
                } else {
                    neutral_message_estimated_tokens(&messages[*message_index])
                }
            })
            .sum();
        let must_keep = message_indices.iter().any(|message_index| {
            messages[*message_index].role == NeutralChatRole::System
                || message_source_sequences[*message_index].is_none()
                || Some(*message_index) == latest_user_index
                || *message_index >= active_tool_start_index
        });

        groups.push(ContextMessageGroup {
            message_indices,
            estimated_tokens,
            must_keep,
        });
    }

    Ok(groups)
}

fn pack_items_from_message_groups(groups: &[ContextMessageGroup]) -> Vec<ContextPackItem> {
    groups
        .iter()
        .enumerate()
        .map(|(index, group)| ContextPackItem {
            id: format!("message-group-{index}"),
            estimated_tokens: group.estimated_tokens,
            must_keep: group.must_keep,
        })
        .collect()
}

fn context_usage_response(
    context: &PreparedPromptContext,
) -> Result<ContextUsageResponse, ApiError> {
    let message_groups = context_message_groups(
        &context.provider_request.messages,
        &context.message_source_sequences,
        context.active_tool_start_index,
    )?;
    let pack_items = pack_items_from_message_groups(&message_groups);
    let used_message_tokens = pack_items
        .iter()
        .map(|item| item.estimated_tokens)
        .sum::<u64>();
    let available_message_tokens = context.context_budget.available_message_tokens;
    let compression_trigger_tokens = context_compression_trigger_tokens(available_message_tokens);
    let usage_percent = percentage_ceil(used_message_tokens, available_message_tokens);
    let compression_trigger_percent =
        percentage_ceil(compression_trigger_tokens, available_message_tokens);
    let will_compress_on_next_send = plan_context_compression(
        &pack_items,
        available_message_tokens,
        active_tool_start_group_index(&message_groups, context.active_tool_start_index),
        CONTEXT_COMPRESSION_PRESERVE_RECENT_MESSAGES,
    )
    .is_some();

    Ok(ContextUsageResponse {
        used_message_tokens,
        available_message_tokens,
        usage_percent,
        compression_trigger_tokens,
        compression_trigger_percent,
        will_compress_on_next_send,
    })
}

fn percentage_ceil(value: u64, total: u64) -> u64 {
    if total == 0 {
        0
    } else {
        value.saturating_mul(100).div_ceil(total)
    }
}

fn active_tool_start_group_index(
    groups: &[ContextMessageGroup],
    active_tool_start_index: usize,
) -> usize {
    groups
        .iter()
        .position(|group| {
            group
                .message_indices
                .iter()
                .any(|message_index| *message_index >= active_tool_start_index)
        })
        .unwrap_or(groups.len())
}

fn message_group_indices(
    groups: &[ContextMessageGroup],
    group_indices: &[usize],
) -> Result<Vec<usize>, ApiError> {
    let mut message_indices = Vec::new();

    for group_index in group_indices {
        let group = groups.get(*group_index).ok_or_else(|| {
            ApiError::internal("context compression covered group index is out of bounds")
        })?;
        message_indices.extend(group.message_indices.iter().copied());
    }

    Ok(message_indices)
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

    let message_groups =
        context_message_groups(&messages, message_source_sequences, active_tool_start_index)?;
    let pack_items = pack_items_from_message_groups(&message_groups);
    let packed = pack_context(&pack_items, budget.available_message_tokens)
        .map_err(|source| ApiError::bad_request(source.to_string()))?;

    let selected_indices = message_group_indices(&message_groups, &packed.selected_indices)?;
    Ok(selected_indices
        .into_iter()
        .map(|index| messages[index].clone())
        .collect())
}

fn neutral_message_estimated_tokens(message: &NeutralChatMessage) -> u64 {
    let mut tokens = estimate_text_tokens(&message.content);

    for attachment in &message.attachments {
        tokens += neutral_attachment_estimated_tokens(attachment);
    }

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

fn neutral_attachment_estimated_tokens(attachment: &NeutralChatAttachment) -> u64 {
    estimate_text_tokens(&attachment.name)
        + estimate_text_tokens(&attachment.content_type)
        + attachment
            .path
            .as_deref()
            .map(estimate_text_tokens)
            .unwrap_or(0)
        + estimate_text_tokens(&format!("{} bytes", attachment.size_bytes))
        + 32
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
    hook_runtime: HookRuntime,
    global_hooks: HookConfig,
    provider_config: ProviderConnectionConfig,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    workspace_id: &str,
    workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    tool_calls: Vec<NeutralToolCall>,
) -> Result<Vec<ToolHookOutcome>, ApiError> {
    if tool_calls
        .iter()
        .any(tool_call_requires_sequential_execution)
    {
        let mut executed_tool_calls = Vec::with_capacity(tool_calls.len());
        for tool_call in tool_calls {
            executed_tool_calls.push(
                execute_tool_call(
                    mcp_registry.clone(),
                    hook_runtime.clone(),
                    global_hooks.clone(),
                    provider_config.clone(),
                    question_registry.clone(),
                    question_event_tx.clone(),
                    workspace_id,
                    workspace_path,
                    chat_id,
                    run_id,
                    model_id,
                    provider_id,
                    tool_call,
                )
                .await,
            );
        }
        return Ok(executed_tool_calls);
    }

    let tasks = tool_calls.into_iter().map(|tool_call| {
        let workspace_path = workspace_path.to_path_buf();
        let workspace_id = workspace_id.to_string();
        let chat_id = chat_id.to_string();
        let run_id = run_id.to_string();
        let model_id = model_id.to_string();
        let provider_id = provider_id.to_string();
        let mcp_registry = mcp_registry.clone();
        let hook_runtime = hook_runtime.clone();
        let global_hooks = global_hooks.clone();
        let provider_config = provider_config.clone();
        let question_registry = question_registry.clone();
        let question_event_tx = question_event_tx.clone();

        tokio::spawn(async move {
            execute_tool_call(
                mcp_registry,
                hook_runtime,
                global_hooks,
                provider_config,
                question_registry,
                question_event_tx,
                &workspace_id,
                &workspace_path,
                &chat_id,
                &run_id,
                &model_id,
                &provider_id,
                tool_call,
            )
            .await
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

async fn execute_tool_call(
    mcp_registry: Arc<McpRegistry>,
    hook_runtime: HookRuntime,
    global_hooks: HookConfig,
    provider_config: ProviderConnectionConfig,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    workspace_id: &str,
    workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    tool_call: NeutralToolCall,
) -> ToolHookOutcome {
    let started_at_text = utc_timestamp();
    let tool_execution = execute_tool(
        mcp_registry,
        hook_runtime.clone(),
        &global_hooks,
        &provider_config,
        question_registry,
        question_event_tx,
        workspace_id,
        workspace_path,
        chat_id,
        run_id,
        model_id,
        provider_id,
        &tool_call.call_id,
        &tool_call.name,
        tool_call.arguments.clone(),
    )
    .await;
    let completed_at_text = utc_timestamp();
    let mut hook_summary = tool_execution.hook_summary;

    let executed = executed_tool_call(
        tool_call,
        tool_execution.execution,
        started_at_text,
        completed_at_text,
    );
    let post_event = if executed.is_error {
        "PostToolUseFailure"
    } else {
        "PostToolUse"
    };
    let post_summary = hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &global_hooks,
            workspace_id,
            workspace_path,
            event: post_event,
            match_value: Some(executed.name.clone()),
            chat_id: Some(chat_id),
            run_id: Some(run_id),
            session_id: Some(chat_id),
            tool_call_id: Some(&executed.id),
            model_id: Some(model_id),
            provider_id: Some(provider_id),
            provider_config: Some(&provider_config),
            permission_mode: None,
            payload: json!({
                "toolName": executed.name.clone(),
                "toolInput": executed.input.clone(),
                "toolOutput": executed.output.clone(),
                "isError": executed.is_error,
            }),
        })
        .await;
    merge_hook_summaries(&mut hook_summary, post_summary);

    ToolHookOutcome {
        tool_call: executed,
        hook_summary,
    }
}

async fn execute_tool(
    mcp_registry: Arc<McpRegistry>,
    hook_runtime: HookRuntime,
    global_hooks: &HookConfig,
    provider_config: &ProviderConnectionConfig,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    workspace_id: &str,
    workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    tool_call_id: &str,
    tool_name: &str,
    mut arguments: Value,
) -> ToolExecutionWithHooks {
    let pre_summary = hook_runtime
        .run_hooks(HookRunRequest {
            global_config: global_hooks,
            workspace_id,
            workspace_path,
            event: "PreToolUse",
            match_value: Some(tool_name.to_string()),
            chat_id: Some(chat_id),
            run_id: Some(run_id),
            session_id: Some(chat_id),
            tool_call_id: Some(tool_call_id),
            model_id: Some(model_id),
            provider_id: Some(provider_id),
            provider_config: Some(provider_config),
            permission_mode: None,
            payload: json!({
                "toolName": tool_name,
                "toolInput": arguments.clone(),
            }),
        })
        .await;
    let blocking_decision = pre_summary
        .decisions
        .iter()
        .find(|decision| {
            matches!(
                decision,
                HookDecision::Block { .. } | HookDecision::Deny { .. } | HookDecision::Ask { .. }
            )
        })
        .cloned();
    let mut hook_summary = pre_summary;
    if let Some(updated_input) = hook_updated_input(&hook_summary) {
        arguments = updated_input;
    }
    if let Some(decision) = blocking_decision {
        match decision {
            HookDecision::Allow => {}
            HookDecision::Block { reason } | HookDecision::Deny { reason } => {
                return ToolExecutionWithHooks {
                    execution: ToolExecution {
                        output: json!({ "error": format!("PreToolUse hook blocked '{tool_name}': {reason}") }),
                        is_error: true,
                    },
                    hook_summary,
                };
            }
            HookDecision::Ask { reason } => {
                let permission_request_summary = hook_runtime
                    .run_hooks(HookRunRequest {
                        global_config: global_hooks,
                        workspace_id,
                        workspace_path,
                        event: "PermissionRequest",
                        match_value: Some(tool_name.to_string()),
                        chat_id: Some(chat_id),
                        run_id: Some(run_id),
                        session_id: Some(chat_id),
                        tool_call_id: Some(tool_call_id),
                        model_id: Some(model_id),
                        provider_id: Some(provider_id),
                        provider_config: Some(provider_config),
                        permission_mode: Some("ask"),
                        payload: json!({
                            "toolName": tool_name,
                            "toolInput": arguments.clone(),
                            "reason": reason,
                        }),
                    })
                    .await;
                let permission_request_decision = permission_request_summary
                    .decisions
                    .iter()
                    .find(|decision| {
                        matches!(
                            decision,
                            HookDecision::Allow
                                | HookDecision::Block { .. }
                                | HookDecision::Deny { .. }
                                | HookDecision::Ask { .. }
                        )
                    })
                    .cloned();
                merge_hook_summaries(&mut hook_summary, permission_request_summary);

                if let Some(updated_input) = hook_updated_input(&hook_summary) {
                    arguments = updated_input;
                }

                let prompt_reason = match permission_request_decision {
                    Some(HookDecision::Allow) => None,
                    Some(HookDecision::Block { reason }) | Some(HookDecision::Deny { reason }) => {
                        let denied_summary = hook_runtime
                            .run_hooks(HookRunRequest {
                                global_config: global_hooks,
                                workspace_id,
                                workspace_path,
                                event: "PermissionDenied",
                                match_value: Some(tool_name.to_string()),
                                chat_id: Some(chat_id),
                                run_id: Some(run_id),
                                session_id: Some(chat_id),
                                tool_call_id: Some(tool_call_id),
                                model_id: Some(model_id),
                                provider_id: Some(provider_id),
                                provider_config: Some(provider_config),
                                permission_mode: Some("deny"),
                                payload: json!({
                                    "toolName": tool_name,
                                    "toolInput": arguments.clone(),
                                    "reason": reason,
                                }),
                            })
                            .await;
                        let retry_message = permission_denied_retry_message(&denied_summary);
                        merge_hook_summaries(&mut hook_summary, denied_summary);
                        return ToolExecutionWithHooks {
                            execution: ToolExecution {
                                output: json!({
                                    "error": format!("PermissionRequest hook denied '{tool_name}': {reason}"),
                                    "retry": retry_message,
                                }),
                                is_error: true,
                            },
                            hook_summary,
                        };
                    }
                    Some(HookDecision::Ask { reason }) => Some(reason),
                    None => Some(reason),
                };

                if let Some(prompt_reason) = prompt_reason {
                    let permission = execute_hook_permission_question(
                        question_registry.clone(),
                        question_event_tx.clone(),
                        workspace_id,
                        chat_id,
                        tool_call_id,
                        tool_name,
                        &prompt_reason,
                    )
                    .await;
                    if let Err(reason) = permission {
                        let denied_summary = hook_runtime
                            .run_hooks(HookRunRequest {
                                global_config: global_hooks,
                                workspace_id,
                                workspace_path,
                                event: "PermissionDenied",
                                match_value: Some(tool_name.to_string()),
                                chat_id: Some(chat_id),
                                run_id: Some(run_id),
                                session_id: Some(chat_id),
                                tool_call_id: Some(tool_call_id),
                                model_id: Some(model_id),
                                provider_id: Some(provider_id),
                                provider_config: Some(provider_config),
                                permission_mode: Some("deny"),
                                payload: json!({
                                    "toolName": tool_name,
                                    "toolInput": arguments.clone(),
                                    "reason": reason,
                                }),
                            })
                            .await;
                        let retry_message = permission_denied_retry_message(&denied_summary);
                        merge_hook_summaries(&mut hook_summary, denied_summary);
                        return ToolExecutionWithHooks {
                            execution: ToolExecution {
                                output: json!({
                                    "error": format!("PreToolUse hook permission denied for '{tool_name}': {reason}"),
                                    "retry": retry_message,
                                }),
                                is_error: true,
                            },
                            hook_summary,
                        };
                    }
                }
            }
        }
    }

    if tool_name == ASK_QUESTION_TOOL {
        let ask_question = execute_ask_question(
            hook_runtime,
            global_hooks,
            provider_config,
            question_registry,
            question_event_tx,
            workspace_id,
            workspace_path,
            chat_id,
            run_id,
            model_id,
            provider_id,
            tool_call_id,
            arguments,
        )
        .await;
        merge_hook_summaries(&mut hook_summary, ask_question.hook_summary);
        return ToolExecutionWithHooks {
            execution: ask_question.execution,
            hook_summary,
        };
    }

    let execution = if is_mcp_tool_name(tool_name) {
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
                return ToolExecutionWithHooks {
                    execution: ToolExecution {
                        output: json!({ "error": error }),
                        is_error: true,
                    },
                    hook_summary,
                };
            }
        };
        let tool_name = tool_name.to_string();
        let worker = tokio::task::spawn_blocking({
            let workspace_path = workspace_path.to_path_buf();
            let chat_id = chat_id.to_string();
            let tool_name = tool_name.clone();
            move || {
                execute_builtin_tool_for_chat(
                    &workspace_path,
                    Some(&chat_id),
                    &tool_name,
                    arguments,
                )
            }
        });
        let execution: Result<ToolExecution, String> = if matches!(
            tool_name.as_str(),
            RUN_COMMAND_TOOL | SEARCH_TEXT_TOOL | SLEEP_TOOL
        ) {
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
    };

    ToolExecutionWithHooks {
        execution,
        hook_summary,
    }
}

async fn execute_ask_question(
    hook_runtime: HookRuntime,
    global_hooks: &HookConfig,
    provider_config: &ProviderConnectionConfig,
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    workspace_id: &str,
    workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    model_id: &str,
    provider_id: &str,
    tool_call_id: &str,
    arguments: Value,
) -> ToolExecutionWithHooks {
    let mut hook_summary = HookRunSummary::default();
    let input = match serde_json::from_value::<AskQuestionInput>(arguments) {
        Ok(input) => input,
        Err(source) => {
            return ToolExecutionWithHooks {
                execution: ToolExecution {
                    output: json!({
                        "error": format!("ask_question arguments do not match schema: {source}")
                    }),
                    is_error: true,
                },
                hook_summary,
            };
        }
    };
    let request = match question_request_from_input(workspace_id, chat_id, tool_call_id, input) {
        Ok(request) => request,
        Err(error) => {
            return ToolExecutionWithHooks {
                execution: ToolExecution {
                    output: json!({ "error": error.message }),
                    is_error: true,
                },
                hook_summary,
            };
        }
    };
    let elicitation_summary = hook_runtime
        .run_hooks(HookRunRequest {
            global_config: global_hooks,
            workspace_id,
            workspace_path,
            event: "Elicitation",
            match_value: Some(ASK_QUESTION_TOOL.to_string()),
            chat_id: Some(chat_id),
            run_id: Some(run_id),
            session_id: Some(chat_id),
            tool_call_id: Some(tool_call_id),
            model_id: Some(model_id),
            provider_id: Some(provider_id),
            provider_config: Some(provider_config),
            permission_mode: None,
            payload: json!({
                "questionRequest": request.clone(),
            }),
        })
        .await;
    let block_reason = elicitation_summary.first_block_reason();
    let elicitation_action = elicitation_action(&elicitation_summary, &request);
    merge_hook_summaries(&mut hook_summary, elicitation_summary);
    if let Some(reason) = block_reason {
        return ToolExecutionWithHooks {
            execution: ToolExecution {
                output: json!({ "error": format!("Elicitation hook blocked question '{}': {reason}", request.id) }),
                is_error: true,
            },
            hook_summary,
        };
    }
    if let Some(action) = elicitation_action {
        match action {
            ElicitationAction::Accept(answer) => {
                let execution = ToolExecution {
                    output: question_answer_output(&request, answer),
                    is_error: false,
                };
                let result_summary = hook_runtime
                    .run_hooks(HookRunRequest {
                        global_config: global_hooks,
                        workspace_id,
                        workspace_path,
                        event: "ElicitationResult",
                        match_value: Some(ASK_QUESTION_TOOL.to_string()),
                        chat_id: Some(chat_id),
                        run_id: Some(run_id),
                        session_id: Some(chat_id),
                        tool_call_id: Some(tool_call_id),
                        model_id: Some(model_id),
                        provider_id: Some(provider_id),
                        provider_config: Some(provider_config),
                        permission_mode: None,
                        payload: json!({
                            "questionRequest": request,
                            "questionResult": execution.output.clone(),
                            "isError": execution.is_error,
                        }),
                    })
                    .await;
                let execution = apply_elicitation_result_action(execution, &result_summary);
                merge_hook_summaries(&mut hook_summary, result_summary);
                return ToolExecutionWithHooks {
                    execution,
                    hook_summary,
                };
            }
            ElicitationAction::Decline(reason) | ElicitationAction::Cancel(reason) => {
                return ToolExecutionWithHooks {
                    execution: ToolExecution {
                        output: json!({ "error": reason }),
                        is_error: true,
                    },
                    hook_summary,
                };
            }
        }
    }

    let registration = match question_registry.register(request.clone()) {
        Ok(registration) => registration,
        Err(error) => {
            return ToolExecutionWithHooks {
                execution: ToolExecution {
                    output: json!({ "error": error.message }),
                    is_error: true,
                },
                hook_summary,
            };
        }
    };

    if question_event_tx.send(request.clone()).is_err() {
        return ToolExecutionWithHooks {
            execution: ToolExecution {
                output: json!({
                    "error": format!("failed to show question '{}' because the chat stream is closed", request.id)
                }),
                is_error: true,
            },
            hook_summary,
        };
    }

    let execution = match registration.answer_rx.await {
        Ok(answer) => {
            let output = question_answer_output(&request, answer);
            ToolExecution {
                output,
                is_error: false,
            }
        }
        Err(_) => ToolExecution {
            output: json!({
                "error": format!("question '{}' was cancelled before the user answered", request.id)
            }),
            is_error: true,
        },
    };
    let result_summary = hook_runtime
        .run_hooks(HookRunRequest {
            global_config: global_hooks,
            workspace_id,
            workspace_path,
            event: "ElicitationResult",
            match_value: Some(ASK_QUESTION_TOOL.to_string()),
            chat_id: Some(chat_id),
            run_id: Some(run_id),
            session_id: Some(chat_id),
            tool_call_id: Some(tool_call_id),
            model_id: Some(model_id),
            provider_id: Some(provider_id),
            provider_config: Some(provider_config),
            permission_mode: None,
            payload: json!({
                "questionRequest": request,
                "questionResult": execution.output.clone(),
                "isError": execution.is_error,
            }),
        })
        .await;
    let execution = apply_elicitation_result_action(execution, &result_summary);
    merge_hook_summaries(&mut hook_summary, result_summary);

    ToolExecutionWithHooks {
        execution,
        hook_summary,
    }
}

fn question_answer_output(request: &QuestionRequest, answer: QuestionAnswer) -> Value {
    let mut answers_by_id = answer
        .answers
        .into_iter()
        .map(|answer| (answer.id.clone(), answer))
        .collect::<HashMap<_, _>>();
    let answers = request
        .questions
        .iter()
        .filter_map(|question| {
            answers_by_id.remove(&question.id).map(|answer| {
                json!({
                    "id": question.id,
                    "question": question.question,
                    "answer": answer.answer,
                    "selectedOptionValue": answer.selected_option_value,
                })
            })
        })
        .collect::<Vec<_>>();

    json!({
        "questionId": request.id,
        "answers": answers,
    })
}

enum ElicitationAction {
    Accept(QuestionAnswer),
    Decline(String),
    Cancel(String),
}

fn hook_updated_input(summary: &HookRunSummary) -> Option<Value> {
    summary
        .hook_specific_outputs
        .iter()
        .rev()
        .find_map(|output| {
            output
                .get("updatedInput")
                .or_else(|| output.get("input"))
                .or_else(|| {
                    output
                        .get("decision")
                        .and_then(|decision| decision.get("updatedInput"))
                })
                .cloned()
        })
}

fn permission_denied_retry_message(summary: &HookRunSummary) -> Option<String> {
    summary.hook_specific_outputs.iter().find_map(|output| {
        if output
            .get("retry")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let suffix = output
                .get("updatedInput")
                .or_else(|| output.get("input"))
                .or_else(|| {
                    output
                        .get("decision")
                        .and_then(|decision| decision.get("updatedInput"))
                })
                .map(|_| " with updated input")
                .unwrap_or_default();
            Some(format!("PermissionDenied hook requested retry{suffix}."))
        } else {
            None
        }
    })
}

fn elicitation_action(
    summary: &HookRunSummary,
    request: &QuestionRequest,
) -> Option<ElicitationAction> {
    summary
        .hook_specific_outputs
        .iter()
        .find_map(|output| match hook_action(output).as_deref() {
            Some("accept") | Some("accepted") => {
                hook_question_answer(output.get("content"), request).map(ElicitationAction::Accept)
            }
            Some("decline") | Some("declined") => Some(ElicitationAction::Decline(
                hook_action_reason(output, "Elicitation hook declined the question"),
            )),
            Some("cancel") | Some("cancelled") | Some("canceled") => {
                Some(ElicitationAction::Cancel(hook_action_reason(
                    output,
                    "Elicitation hook cancelled the question",
                )))
            }
            _ => None,
        })
}

fn apply_elicitation_result_action(
    mut execution: ToolExecution,
    summary: &HookRunSummary,
) -> ToolExecution {
    for output in &summary.hook_specific_outputs {
        match hook_action(output).as_deref() {
            Some("accept") | Some("accepted") => {
                if let Some(content) = output.get("content") {
                    execution.output = content.clone();
                    execution.is_error = false;
                }
            }
            Some("decline") | Some("declined") | Some("cancel") | Some("cancelled")
            | Some("canceled") => {
                execution.output = json!({ "error": hook_action_reason(output, "ElicitationResult hook rejected the question result") });
                execution.is_error = true;
            }
            _ => {}
        }
    }

    execution
}

fn hook_action(output: &Value) -> Option<String> {
    output
        .get("action")
        .and_then(Value::as_str)
        .map(|action| action.trim().to_ascii_lowercase())
}

fn hook_action_reason(output: &Value, default_reason: &str) -> String {
    output
        .get("reason")
        .or_else(|| output.get("message"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(default_reason)
        .to_string()
}

fn hook_question_answer(
    content: Option<&Value>,
    request: &QuestionRequest,
) -> Option<QuestionAnswer> {
    let content = content?;

    if let Ok(answer) = serde_json::from_value::<QuestionAnswer>(content.clone()) {
        return Some(answer);
    }

    let answers = request
        .questions
        .iter()
        .map(|question| {
            let answer = hook_answer_for_question(content, question);
            QuestionItemAnswer {
                id: question.id.clone(),
                selected_option_value: matching_option_value(question, &answer),
                answer,
            }
        })
        .collect::<Vec<_>>();

    Some(QuestionAnswer { answers })
}

fn hook_answer_for_question(content: &Value, question: &QuestionItem) -> String {
    if let Some(value) = content.get(&question.id) {
        return hook_answer_text(value);
    }

    if let Some(value) = content.get(&question.question) {
        return hook_answer_text(value);
    }

    if let Some(value) = content.get("answer") {
        return hook_answer_text(value);
    }

    if let Some(value) = content.get("value") {
        return hook_answer_text(value);
    }

    hook_answer_text(content)
}

fn hook_answer_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn matching_option_value(question: &QuestionItem, answer: &str) -> Option<String> {
    question
        .options
        .iter()
        .find(|option| option.value == answer || option.label == answer)
        .map(|option| option.value.clone())
}

async fn execute_hook_permission_question(
    question_registry: QuestionRegistry,
    question_event_tx: mpsc::UnboundedSender<QuestionRequest>,
    workspace_id: &str,
    chat_id: &str,
    tool_call_id: &str,
    tool_name: &str,
    reason: &str,
) -> Result<(), String> {
    let request_id = unique_id("hook-question");
    let request = QuestionRequest {
        id: request_id.clone(),
        tool_call_id: tool_call_id.to_string(),
        workspace_id: workspace_id.to_string(),
        chat_id: chat_id.to_string(),
        questions: vec![QuestionItem {
            id: format!("{request_id}-item-1"),
            question: format!("Hook asks whether to allow tool '{tool_name}': {reason}"),
            options: vec![
                QuestionOption {
                    label: "Allow".to_string(),
                    value: "allow".to_string(),
                    description: Some("Run the tool once.".to_string()),
                },
                QuestionOption {
                    label: "Deny".to_string(),
                    value: "deny".to_string(),
                    description: Some("Block this tool call.".to_string()),
                },
            ],
            allow_free_text: false,
        }],
    };
    let registration = question_registry
        .register(request.clone())
        .map_err(|source| source.message)?;

    if question_event_tx.send(request.clone()).is_err() {
        return Err(format!(
            "failed to show hook permission question '{}' because the chat stream is closed",
            request.id
        ));
    }

    let answer = registration
        .answer_rx
        .await
        .map_err(|_| format!("hook permission question '{}' was cancelled", request.id))?;
    let selected = answer
        .answers
        .first()
        .and_then(|answer| answer.selected_option_value.as_deref())
        .unwrap_or_default();

    if selected == "allow" {
        Ok(())
    } else {
        Err("user denied hook permission request".to_string())
    }
}

fn question_request_from_input(
    workspace_id: &str,
    chat_id: &str,
    tool_call_id: &str,
    input: AskQuestionInput,
) -> Result<QuestionRequest, ApiError> {
    if input.questions.is_empty() {
        return Err(ApiError::bad_request(
            "ask_question requires at least one question",
        ));
    }

    let request_id = unique_id("question");
    let mut questions = Vec::with_capacity(input.questions.len());

    for (index, item) in input.questions.into_iter().enumerate() {
        let item_number = index + 1;
        let question = non_empty_trimmed(item.question, &format!("question {item_number}"))?;
        let options = normalize_question_options(item.options.unwrap_or_default())?;

        if !item.allow_free_text && options.is_empty() {
            return Err(ApiError::bad_request(format!(
                "ask_question item {item_number} requires options when allowFreeText is false"
            )));
        }

        questions.push(QuestionItem {
            id: format!("{request_id}-item-{item_number}"),
            question,
            options,
            allow_free_text: item.allow_free_text,
        });
    }

    Ok(QuestionRequest {
        id: request_id,
        tool_call_id: tool_call_id.to_string(),
        workspace_id: workspace_id.to_string(),
        chat_id: chat_id.to_string(),
        questions,
    })
}

fn normalize_question_options(
    options: Vec<QuestionOption>,
) -> Result<Vec<QuestionOption>, ApiError> {
    let mut seen_values = HashSet::new();
    let mut normalized = Vec::with_capacity(options.len());

    for option in options {
        let label = non_empty_trimmed(option.label, "option label")?;
        let value = non_empty_trimmed(option.value, "option value")?;
        let description = option
            .description
            .map(|description| description.trim().to_string())
            .filter(|description| !description.is_empty());

        if !seen_values.insert(value.clone()) {
            return Err(ApiError::bad_request(format!(
                "ask_question option value is duplicated: {value}"
            )));
        }

        normalized.push(QuestionOption {
            label,
            value,
            description,
        });
    }

    Ok(normalized)
}

fn non_empty_trimmed(value: String, field_name: &str) -> Result<String, ApiError> {
    let value = value.trim().to_string();

    if value.is_empty() {
        Err(ApiError::bad_request(format!(
            "{field_name} must not be empty"
        )))
    } else {
        Ok(value)
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
    for message in interleaved_tool_state_messages(
        tool_calls,
        tool_results,
        assistant_text,
        assistant_reasoning,
    ) {
        messages.push(message);
        message_source_sequences.push(None);
    }
}

fn append_hook_context_messages(
    messages: &mut Vec<NeutralChatMessage>,
    message_source_sequences: &mut Vec<Option<i64>>,
    contexts: &[String],
) {
    for context in contexts.iter().filter(|context| !context.trim().is_empty()) {
        messages.push(neutral_text_message(
            NeutralChatRole::System,
            format!("Hook additional context:\n\n{}", context.trim()),
        ));
        message_source_sequences.push(None);
    }
}

fn hook_notification_events(
    assistant_message_id: &str,
    event: &str,
    summary: &HookRunSummary,
) -> Vec<ChatSseEvent> {
    summary
        .hook_messages(event)
        .into_iter()
        .map(|notification| ChatSseEvent::HookNotification {
            assistant_message_id: assistant_message_id.to_string(),
            notification,
        })
        .collect()
}

fn merge_hook_summaries(target: &mut HookRunSummary, source: HookRunSummary) {
    target.decisions.extend(source.decisions);
    target.additional_context.extend(source.additional_context);
    target.system_messages.extend(source.system_messages);
    target.errors.extend(source.errors);
}

fn tool_call_requires_sequential_execution(tool_call: &NeutralToolCall) -> bool {
    matches!(
        tool_call.name.as_str(),
        ASK_QUESTION_TOOL | CREATE_TASK_GRAPH_TOOL | UPDATE_TASK_GRAPH_TOOL
    )
}

fn tool_results_affect_git_diff(tool_results: &[ExecutedToolCall]) -> bool {
    tool_results.iter().any(|tool_result| {
        matches!(
            tool_result.name.as_str(),
            WRITE_FILE_TOOL | PATCH_FILE_TOOL | RUN_COMMAND_TOOL
        )
    })
}

fn tool_results_affect_task_graph(tool_results: &[ExecutedToolCall]) -> bool {
    tool_results.iter().any(|tool_result| {
        matches!(
            tool_result.name.as_str(),
            CREATE_TASK_GRAPH_TOOL | UPDATE_TASK_GRAPH_TOOL
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
        status_code: None,
        final_state: "failed",
        response_body_json: Some(json!({ "error": message }).to_string()),
    }
}

fn failed_provider_audit_outcome(
    started_at: Instant,
    message: &str,
    status_code: Option<i64>,
) -> ChatAuditOutcome {
    ChatAuditOutcome {
        status_code,
        ..failed_audit_outcome(started_at, message)
    }
}

async fn failed_chat_audit_outcome(
    context: &PreparedChatContext,
    started_at: Instant,
    events: &mut Vec<CapturedAuditEvent>,
    message: &str,
    status_code: Option<i64>,
) -> ChatAuditOutcome {
    let stop_failure_summary = context
        .hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &context.global_hooks,
            workspace_id: &context.workspace_id,
            workspace_path: &context.workspace_path,
            event: "StopFailure",
            match_value: None,
            chat_id: Some(&context.chat_id),
            run_id: Some(&context.llm_request_id),
            session_id: Some(&context.chat_id),
            tool_call_id: None,
            model_id: Some(&context.model_id),
            provider_id: Some(&context.provider_id),
            provider_config: Some(&context.provider_config),
            permission_mode: None,
            payload: json!({
                "message": message,
                "statusCode": status_code,
            }),
        })
        .await;
    for event in hook_notification_events(
        &context.assistant_message_id,
        "StopFailure",
        &stop_failure_summary,
    ) {
        events.push(captured_event(&event));
    }

    if let Some(status_code) = status_code {
        failed_provider_audit_outcome(started_at, message, Some(status_code))
    } else {
        failed_audit_outcome(started_at, message)
    }
}

fn provider_status_code(error: &ProviderConfigError) -> Option<i64> {
    error.status_code().map(i64::from)
}

fn cancelled_audit_outcome(started_at: Instant, message: &str) -> ChatAuditOutcome {
    ChatAuditOutcome {
        first_token_at: None,
        completed_at: utc_timestamp(),
        first_token_latency_ms: None,
        total_latency_ms: elapsed_millis(started_at),
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_write_tokens: None,
        status_code: None,
        final_state: "cancelled",
        response_body_json: Some(json!({ "cancelled": message }).to_string()),
    }
}

async fn finish_cancelled_chat_run(
    context: &PreparedChatContext,
    request_started_at: &str,
    started_at: Instant,
    events: &mut Vec<CapturedAuditEvent>,
    executed_tool_calls: &[ExecutedToolCall],
) -> ChatSseEvent {
    let session_end_summary = session_end_hook(
        context,
        "cancelled",
        json!({
            "reason": SHUTDOWN_MESSAGE,
        }),
    )
    .await;
    for event in hook_notification_events(
        &context.assistant_message_id,
        "SessionEnd",
        &session_end_summary,
    ) {
        events.push(captured_event(&event));
    }
    let event = ChatSseEvent::Error {
        message: SHUTDOWN_MESSAGE.to_string(),
    };
    events.push(captured_event(&event));
    let outcome = cancelled_audit_outcome(started_at, SHUTDOWN_MESSAGE);

    if let Err(error) = persist_chat_result(
        context,
        request_started_at,
        outcome,
        events,
        None,
        None,
        executed_tool_calls,
    ) {
        return ChatSseEvent::Error {
            message: error.message,
        };
    }

    event
}

async fn session_end_hook(
    context: &PreparedChatContext,
    final_state: &str,
    payload: Value,
) -> HookRunSummary {
    context
        .hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &context.global_hooks,
            workspace_id: &context.workspace_id,
            workspace_path: &context.workspace_path,
            event: "SessionEnd",
            match_value: None,
            chat_id: Some(&context.chat_id),
            run_id: Some(&context.llm_request_id),
            session_id: Some(&context.chat_id),
            tool_call_id: None,
            model_id: Some(&context.model_id),
            provider_id: Some(&context.provider_id),
            provider_config: Some(&context.provider_config),
            permission_mode: None,
            payload: json!({
                "finalState": final_state,
                "details": payload,
            }),
        })
        .await
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
        ChatSseEvent::QuestionRequest { .. } => "question_request",
        ChatSseEvent::HookNotification { .. } => "hook_notification",
        ChatSseEvent::GitDiffRefresh { .. } => "git_diff_refresh",
        ChatSseEvent::TaskGraphRefresh { .. } => "task_graph_refresh",
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

fn chat_title_for_prompt(message: &str, attachments: &[NeutralChatAttachment]) -> String {
    if message.trim().is_empty() {
        if let Some(attachment) = attachments.first() {
            return chat_title(&attachment.name);
        }
    }

    chat_title(message)
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

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
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
        match error {
            foco_store::workspace::WorkspaceDatabaseError::InvalidTaskGraph { .. }
            | foco_store::workspace::WorkspaceDatabaseError::MissingTaskGraph { .. } => {
                Self::bad_request(error.to_string())
            }
            _ => Self::internal(error.to_string()),
        }
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
    let path = validate_workspace_path(&request.path)?;

    if name.is_empty() {
        return Err(ApiError::bad_request("workspace name must not be empty"));
    }

    Ok((name, path))
}

fn validate_workspace_path(path: &str) -> Result<PathBuf, ApiError> {
    let path = PathBuf::from(path.trim());

    if path.as_os_str().is_empty() {
        return Err(ApiError::bad_request("workspace path must not be empty"));
    }

    if !path.is_absolute() {
        return Err(ApiError::bad_request(format!(
            "workspace path must be absolute: {}",
            path.display()
        )));
    }

    Ok(path)
}

fn native_select_directory() -> Result<Option<String>, ApiError> {
    if !(cfg!(windows) || is_wsl_environment()) {
        return Err(ApiError::bad_request(
            "native directory picker is only available on Windows",
        ));
    }

    let script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

[ComImport]
[Guid("DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7")]
public class FileOpenDialogCom
{
}

[ComImport]
[Guid("D57C7288-D4AD-4768-BE02-9D969532D960")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IFileOpenDialog
{
    [PreserveSig]
    int Show(IntPtr parent);
    void SetFileTypes(uint cFileTypes, IntPtr rgFilterSpec);
    void SetFileTypeIndex(uint iFileType);
    void GetFileTypeIndex(out uint piFileType);
    void Advise(IntPtr pfde, out uint pdwCookie);
    void Unadvise(uint dwCookie);
    void SetOptions(uint fos);
    void GetOptions(out uint pfos);
    void SetDefaultFolder(IShellItem psi);
    void SetFolder(IShellItem psi);
    void GetFolder(out IShellItem ppsi);
    void GetCurrentSelection(out IShellItem ppsi);
    void SetFileName([MarshalAs(UnmanagedType.LPWStr)] string pszName);
    void GetFileName([MarshalAs(UnmanagedType.LPWStr)] out string pszName);
    void SetTitle([MarshalAs(UnmanagedType.LPWStr)] string pszTitle);
    void SetOkButtonLabel([MarshalAs(UnmanagedType.LPWStr)] string pszText);
    void SetFileNameLabel([MarshalAs(UnmanagedType.LPWStr)] string pszLabel);
    void GetResult(out IShellItem ppsi);
    void AddPlace(IShellItem psi, int fdap);
    void SetDefaultExtension([MarshalAs(UnmanagedType.LPWStr)] string pszDefaultExtension);
    void Close(int hr);
    void SetClientGuid(ref Guid guid);
    void ClearClientData();
    void SetFilter(IntPtr pFilter);
    void GetResults(out IntPtr ppenum);
    void GetSelectedItems(out IntPtr ppsai);
}

[ComImport]
[Guid("43826D1E-E718-42EE-BC55-A1E261C37BFE")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IShellItem
{
    void BindToHandler(IntPtr pbc, ref Guid bhid, ref Guid riid, out IntPtr ppv);
    void GetParent(out IShellItem ppsi);
    void GetDisplayName(uint sigdnName, [MarshalAs(UnmanagedType.LPWStr)] out string ppszName);
    void GetAttributes(uint sfgaoMask, out uint psfgaoAttribs);
    void Compare(IShellItem psi, uint hint, out int piOrder);
}

public static class ModernFolderPicker
{
    private const uint FOS_PICKFOLDERS = 0x00000020;
    private const uint FOS_FORCEFILESYSTEM = 0x00000040;
    private const uint FOS_PATHMUSTEXIST = 0x00000800;
    private const uint SIGDN_FILESYSPATH = 0x80058000;
    private const int HRESULT_CANCELLED = unchecked((int)0x800704C7);

    public static string Pick()
    {
        IFileOpenDialog dialog = (IFileOpenDialog)new FileOpenDialogCom();
        uint options;
        dialog.GetOptions(out options);
        dialog.SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST);
        dialog.SetTitle("Choose workspace path");
        dialog.SetOkButtonLabel("Select");

        int result = dialog.Show(IntPtr.Zero);
        if (result == HRESULT_CANCELLED)
        {
            return null;
        }

        if (result != 0)
        {
            Marshal.ThrowExceptionForHR(result);
        }

        IShellItem item;
        dialog.GetResult(out item);

        string path;
        item.GetDisplayName(SIGDN_FILESYSPATH, out path);
        return path;
    }
}
'@

$selectedPath = [ModernFolderPicker]::Pick()
if ($selectedPath) {
  Write-Output $selectedPath
}
"#;
    let output = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-STA", "-Command", script])
        .stdin(Stdio::null())
        .output()
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to launch native directory picker: {source}"
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ApiError::internal(format!(
            "native directory picker failed{}",
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        )));
    }

    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selected_path.is_empty() {
        return Ok(None);
    }

    if is_wsl_environment() && !cfg!(windows) {
        let output = Command::new("wslpath")
            .args(["-u", &selected_path])
            .stdin(Stdio::null())
            .output()
            .map_err(|source| {
                ApiError::internal(format!("failed to convert selected Windows path: {source}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(ApiError::internal(format!(
                "failed to convert selected Windows path{}",
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!(": {stderr}")
                }
            )));
        }

        return Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ));
    }

    Ok(Some(selected_path))
}

fn native_select_files() -> Result<Vec<NativeSelectedFile>, ApiError> {
    if !(cfg!(windows) || is_wsl_environment()) {
        return Err(ApiError::bad_request(
            "native file picker is only available on Windows",
        ));
    }

    let script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms

$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.CheckFileExists = $true
$dialog.CheckPathExists = $true
$dialog.Multiselect = $true
$dialog.Title = "Choose attachments"

if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  ConvertTo-Json -InputObject @($dialog.FileNames) -Compress
} else {
  Write-Output "[]"
}
"#;
    let output = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-STA", "-Command", script])
        .stdin(Stdio::null())
        .output()
        .map_err(|source| {
            ApiError::internal(format!("failed to launch native file picker: {source}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ApiError::internal(format!(
            "native file picker failed{}",
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(Vec::new());
    }

    let mut selected_paths = serde_json::from_str::<Vec<String>>(&stdout).map_err(|source| {
        ApiError::internal(format!(
            "native file picker returned invalid JSON: {source}"
        ))
    })?;
    if is_wsl_environment() && !cfg!(windows) {
        selected_paths = selected_paths
            .into_iter()
            .map(windows_path_to_wsl_path)
            .collect::<Result<Vec<_>, _>>()?;
    }

    native_selected_files_from_paths(selected_paths)
}

fn windows_path_to_wsl_path(path: String) -> Result<String, ApiError> {
    let output = Command::new("wslpath")
        .args(["-u", &path])
        .stdin(Stdio::null())
        .output()
        .map_err(|source| {
            ApiError::internal(format!("failed to convert selected Windows path: {source}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ApiError::internal(format!(
            "failed to convert selected Windows path{}",
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn native_selected_files_from_paths(
    paths: Vec<String>,
) -> Result<Vec<NativeSelectedFile>, ApiError> {
    if paths.len() > MAX_CHAT_ATTACHMENTS {
        return Err(ApiError::bad_request(format!(
            "at most {MAX_CHAT_ATTACHMENTS} attachments are allowed"
        )));
    }

    let mut files = Vec::with_capacity(paths.len());
    let mut total_size = 0_u64;
    for path in paths {
        let path = path.trim().to_string();
        if path.is_empty() {
            return Err(ApiError::bad_request(
                "selected file path must not be empty",
            ));
        }

        let path_buf = PathBuf::from(&path);
        let metadata = fs::metadata(&path_buf).map_err(|source| {
            ApiError::bad_request(format!("selected file is not readable: {path}: {source}"))
        })?;
        if !metadata.is_file() {
            return Err(ApiError::bad_request(format!(
                "selected attachment path must be a file: {path}"
            )));
        }

        let name = path_buf
            .file_name()
            .map(|value| value.to_string_lossy().trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::bad_request(format!("selected file has no name: {path}")))?;
        let size_bytes = metadata.len();
        if size_bytes > MAX_CHAT_ATTACHMENT_BYTES {
            return Err(ApiError::bad_request(format!(
                "attachment {name} exceeds the {} byte limit",
                MAX_CHAT_ATTACHMENT_BYTES
            )));
        }

        total_size = total_size
            .checked_add(size_bytes)
            .ok_or_else(|| ApiError::bad_request("attachment total size exceeds u64"))?;
        if total_size > MAX_CHAT_ATTACHMENT_TOTAL_BYTES {
            return Err(ApiError::bad_request(format!(
                "attachments exceed the {} byte total limit",
                MAX_CHAT_ATTACHMENT_TOTAL_BYTES
            )));
        }

        let content_type = attachment_content_type_for_path(&path_buf);
        let content_base64 = if content_type.starts_with("image/") {
            let bytes = fs::read(&path_buf).map_err(|source| {
                ApiError::bad_request(format!("failed to read selected image {name}: {source}"))
            })?;
            Some(general_purpose::STANDARD.encode(bytes))
        } else {
            None
        };

        files.push(NativeSelectedFile {
            path,
            name,
            content_type,
            size_bytes,
            content_base64,
        });
    }

    Ok(files)
}

fn workspace_logo_request_bytes(request: &WorkspaceLogoRequest) -> Result<Vec<u8>, ApiError> {
    let content_base64 = request
        .content_base64
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let Some(content_base64) = content_base64 else {
        return Err(ApiError::bad_request(
            "workspace logo request must include contentBase64",
        ));
    };

    if content_base64.starts_with("data:") {
        return Err(ApiError::bad_request(
            "workspace logo contentBase64 must be raw base64, not a data URL",
        ));
    }

    let bytes = general_purpose::STANDARD
        .decode(content_base64)
        .map_err(|source| {
            ApiError::bad_request(format!(
                "workspace logo contentBase64 is invalid base64: {source}"
            ))
        })?;
    validate_workspace_logo_size(bytes.len() as u64)?;
    Ok(bytes)
}

fn workspace_logo_url(workspace: &WorkspaceConfig) -> Result<Option<String>, ApiError> {
    Ok(workspace_logo_file(&workspace.path)?
        .map(|logo| format!("/api/workspaces/{}/logo?v={}", workspace.id, logo.version)))
}

fn workspace_logo_file(workspace_path: &Path) -> Result<Option<WorkspaceLogoFile>, ApiError> {
    let logo_dir = workspace_path.join(".foco");
    if !logo_dir.exists() {
        return Ok(None);
    }
    if !logo_dir.is_dir() {
        return Err(ApiError::bad_request(format!(
            "workspace logo directory is not a directory: {}",
            logo_dir.display()
        )));
    }

    for extension in WORKSPACE_LOGO_EXTENSIONS {
        let path = logo_dir.join(format!("logo.{extension}"));
        if !path.exists() {
            continue;
        }

        let (bytes, metadata) = read_workspace_logo_file(&path)?;
        let kind = workspace_logo_kind(&bytes)?;
        if !workspace_logo_extension_matches(extension, kind) {
            return Err(ApiError::bad_request(format!(
                "workspace logo extension .{} does not match detected {}: {}",
                extension,
                kind.content_type,
                path.display()
            )));
        }

        return Ok(Some(WorkspaceLogoFile {
            path,
            kind,
            version: workspace_logo_version(&metadata),
        }));
    }

    Ok(None)
}

fn save_workspace_logo_file(
    workspace_path: &Path,
    bytes: &[u8],
    kind: WorkspaceLogoKind,
) -> Result<(), ApiError> {
    validate_workspace_logo_size(bytes.len() as u64)?;

    let logo_dir = workspace_path.join(".foco");
    fs::create_dir_all(&logo_dir).map_err(|source| {
        ApiError::internal(format!(
            "failed to create workspace logo directory {}: {}",
            logo_dir.display(),
            source
        ))
    })?;

    remove_workspace_logo_files(&logo_dir)?;

    let target = logo_dir.join(format!("logo.{}", kind.extension));
    fs::write(&target, bytes).map_err(|source| {
        ApiError::internal(format!(
            "failed to write workspace logo {}: {}",
            target.display(),
            source
        ))
    })
}

fn clear_workspace_logo_file(workspace_path: &Path) -> Result<(), ApiError> {
    let logo_dir = workspace_path.join(".foco");
    if !logo_dir.exists() {
        return Ok(());
    }
    if !logo_dir.is_dir() {
        return Err(ApiError::bad_request(format!(
            "workspace logo directory is not a directory: {}",
            logo_dir.display()
        )));
    }

    remove_workspace_logo_files(&logo_dir)
}

fn remove_workspace_logo_files(logo_dir: &Path) -> Result<(), ApiError> {
    for extension in WORKSPACE_LOGO_EXTENSIONS {
        let path = logo_dir.join(format!("logo.{extension}"));
        if !path.exists() {
            continue;
        }
        if !path.is_file() {
            return Err(ApiError::bad_request(format!(
                "workspace logo path must be a file: {}",
                path.display()
            )));
        }
        fs::remove_file(&path).map_err(|source| {
            ApiError::internal(format!(
                "failed to remove old workspace logo {}: {}",
                path.display(),
                source
            ))
        })?;
    }

    Ok(())
}

fn read_workspace_logo_file(path: &Path) -> Result<(Vec<u8>, fs::Metadata), ApiError> {
    let metadata = fs::metadata(path).map_err(|source| {
        ApiError::bad_request(format!(
            "workspace logo file is not readable: {}: {}",
            path.display(),
            source
        ))
    })?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "workspace logo path must be a file: {}",
            path.display()
        )));
    }
    validate_workspace_logo_size(metadata.len())?;

    let bytes = fs::read(path).map_err(|source| {
        ApiError::bad_request(format!(
            "failed to read workspace logo {}: {}",
            path.display(),
            source
        ))
    })?;
    Ok((bytes, metadata))
}

fn validate_workspace_logo_size(size_bytes: u64) -> Result<(), ApiError> {
    if size_bytes == 0 {
        return Err(ApiError::bad_request(
            "workspace logo file must not be empty",
        ));
    }
    if size_bytes > MAX_WORKSPACE_LOGO_BYTES {
        return Err(ApiError::bad_request(format!(
            "workspace logo exceeds the {} byte limit",
            MAX_WORKSPACE_LOGO_BYTES
        )));
    }

    Ok(())
}

fn workspace_logo_kind(bytes: &[u8]) -> Result<WorkspaceLogoKind, ApiError> {
    if bytes.starts_with(b"\x89PNG\r\n\x1A\n") {
        return Ok(WorkspaceLogoKind {
            extension: "png",
            content_type: "image/png",
        });
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok(WorkspaceLogoKind {
            extension: "jpg",
            content_type: "image/jpeg",
        });
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok(WorkspaceLogoKind {
            extension: "gif",
            content_type: "image/gif",
        });
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Ok(WorkspaceLogoKind {
            extension: "webp",
            content_type: "image/webp",
        });
    }

    Err(ApiError::bad_request(
        "workspace logo must be a PNG, JPEG, WebP, or GIF image",
    ))
}

fn workspace_logo_extension_matches(extension: &str, kind: WorkspaceLogoKind) -> bool {
    match kind.extension {
        "jpg" => extension == "jpg" || extension == "jpeg",
        detected => extension == detected,
    }
}

fn workspace_logo_version(metadata: &fs::Metadata) -> String {
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    format!("{modified_ms}-{}", metadata.len())
}

fn attachment_content_type_for_path(path: &Path) -> String {
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "avif" => "image/avif",
        "bat" => "text/plain",
        "bmp" => "image/bmp",
        "c" => "text/plain",
        "cmd" => "text/plain",
        "cpp" => "text/plain",
        "cs" => "text/plain",
        "css" => "text/css",
        "csv" => "text/csv",
        "gif" => "image/gif",
        "go" => "text/plain",
        "h" => "text/plain",
        "hpp" => "text/plain",
        "htm" => "text/html",
        "html" => "text/html",
        "java" => "text/plain",
        "jpeg" | "jpg" => "image/jpeg",
        "js" => "text/javascript",
        "json" => "application/json",
        "jsx" => "text/javascript",
        "md" => "text/markdown",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "ps1" => "text/plain",
        "py" => "text/x-python",
        "rs" => "text/plain",
        "sh" => "text/x-shellscript",
        "toml" => "application/toml",
        "ts" => "text/typescript",
        "tsx" => "text/typescript",
        "txt" => "text/plain",
        "webp" => "image/webp",
        "xml" => "application/xml",
        "yaml" | "yml" => "application/yaml",
        _ => "application/octet-stream",
    }
    .to_string()
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
        general: GeneralSettingsSummary {
            web_server: WebServerSettingsSummary {
                listen_host: config.app.web_server.listen_host.clone(),
                listen_port: config.app.web_server.listen_port,
                password_enabled: web_auth_enabled(config),
            },
            language: config.app.language.clone(),
            hook_audit_enabled: config.hooks.audit_enabled,
            supported_languages: SUPPORTED_APP_LANGUAGES
                .iter()
                .map(|language| AppLanguageSummary {
                    id: *language,
                    name: app_language_name(*language),
                })
                .collect(),
        },
        workspaces: config
            .workspaces
            .iter()
            .map(configured_workspace_summary)
            .collect::<Result<Vec<_>, _>>()?,
        terminal_shells: terminal_shell_summaries(),
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
        skills: skills_settings_summary(config, &state.user_profile_dir),
    }))
}

async fn hooks_settings_response(
    state: &AppState,
    config: &GlobalConfig,
    workspace_id: Option<&str>,
) -> Result<Json<HooksSettingsResponse>, ApiError> {
    let workspace = selected_hooks_workspace(config, workspace_id)?;
    let workspace_config = load_workspace_hook_config(&workspace.path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let effective = effective_hook_summaries(&config.hooks, &workspace.path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let recent_runs = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?
        .hook_runs(50)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(hook_run_summary_row)
        .collect();

    Ok(Json(HooksSettingsResponse {
        supported_events: SUPPORTED_HOOK_EVENTS.to_vec(),
        unsupported_events: UNSUPPORTED_HOOK_EVENTS.to_vec(),
        global: HookConfigScopeSummary {
            source: "global".to_string(),
            path: display_path(&state.config_file),
            workspace_id: None,
            config: config.hooks.clone(),
        },
        workspace: HookConfigScopeSummary {
            source: "workspace".to_string(),
            path: display_path(&workspace_hook_config_path(&workspace.path)),
            workspace_id: Some(workspace.id.clone()),
            config: workspace_config,
        },
        effective,
        recent_runs,
    }))
}

fn selected_hooks_workspace<'a>(
    config: &'a GlobalConfig,
    workspace_id: Option<&str>,
) -> Result<&'a WorkspaceConfig, ApiError> {
    match workspace_id.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(workspace_id) => workspace_by_id(config, workspace_id),
        None => workspace_by_id(config, &config.app.active_workspace_id),
    }
}

fn hook_run_summary_row(record: HookRunRecord) -> HookRunSummaryRow {
    HookRunSummaryRow {
        id: record.id,
        workspace_id: record.workspace_id,
        chat_id: record.chat_id,
        run_id: record.run_id,
        tool_call_id: record.tool_call_id,
        event: record.event,
        hook_source: record.hook_source,
        handler_type: record.handler_type,
        status: record.status,
        exit_code: record.exit_code,
        stdout_preview: record.stdout_preview,
        stderr_preview: record.stderr_preview,
        started_at: record.started_at,
        completed_at: record.completed_at,
    }
}

fn hook_run_detail_from_record(record: HookRunRecord) -> Result<HookRunDetail, ApiError> {
    let input = parse_json_value(&record.input_json, "hook run input")?;
    let output = record
        .output_json
        .as_deref()
        .map(|json| parse_json_value(json, "hook run output"))
        .transpose()?;

    Ok(HookRunDetail {
        id: record.id,
        workspace_id: record.workspace_id,
        chat_id: record.chat_id,
        run_id: record.run_id,
        tool_call_id: record.tool_call_id,
        event: record.event,
        hook_source: record.hook_source,
        handler_type: record.handler_type,
        input,
        output,
        status: record.status,
        exit_code: record.exit_code,
        stdout_preview: record.stdout_preview,
        stderr_preview: record.stderr_preview,
        started_at: record.started_at,
        completed_at: record.completed_at,
    })
}

fn claude_hook_settings_paths(base_path: &Path) -> Vec<PathBuf> {
    vec![
        base_path.join(".claude").join("settings.json"),
        base_path.join(".claude").join("settings.local.json"),
    ]
}

fn import_claude_hook_config(
    paths: &[PathBuf],
) -> Result<(HookConfig, Vec<String>, Vec<String>), ApiError> {
    let mut config = HookConfig::default();
    let mut imported_files = Vec::new();
    let mut validation_errors = Vec::new();

    for path in paths {
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(|source| {
            ApiError::internal(format!("failed to read {}: {source}", path.display()))
        })?;
        let value = serde_json::from_str::<Value>(&content).map_err(|source| {
            ApiError::bad_request(format!("failed to parse {}: {source}", path.display()))
        })?;
        let Some(imported) = hook_config_from_claude_settings(&value).map_err(|message| {
            ApiError::bad_request(format!("failed to import {}: {message}", path.display()))
        })?
        else {
            continue;
        };

        imported_files.push(display_path(&path));
        config.disable_all_hooks = imported.disable_all_hooks;
        merge_hook_event_maps(&mut config.hooks, imported.hooks);
    }

    for event in config.hooks.keys() {
        if UNSUPPORTED_HOOK_EVENTS.contains(&event.as_str()) {
            validation_errors.push(format!(
                "{event} is a Claude Code hook event that Foco does not support yet"
            ));
        } else if !SUPPORTED_HOOK_EVENTS.contains(&event.as_str()) {
            validation_errors.push(format!("{event} is not a supported Foco hook event"));
        }
    }

    Ok((config, imported_files, validation_errors))
}

fn hook_config_from_claude_settings(value: &Value) -> Result<Option<HookConfig>, String> {
    let mut config = HookConfig::default();
    let mut found = false;

    if let Some(disable_all_hooks) = value.get("disableAllHooks") {
        config.disable_all_hooks = disable_all_hooks
            .as_bool()
            .ok_or_else(|| "disableAllHooks must be a boolean".to_string())?;
        found = true;
    }

    if let Some(hooks) = value.get("hooks") {
        config.hooks = serde_json::from_value::<HookEventMap>(hooks.clone())
            .map_err(|source| format!("hooks do not match Foco hook shape: {source}"))?;
        found = true;
    }

    Ok(found.then_some(config))
}

fn merge_hook_event_maps(target: &mut HookEventMap, imported: HookEventMap) {
    for (event, mut groups) in imported {
        target.entry(event).or_default().append(&mut groups);
    }
}

fn default_hook_provider(
    config: &GlobalConfig,
) -> Option<Result<(String, String, ProviderConnectionConfig), ApiError>> {
    let model = config
        .models
        .iter()
        .find(|model| model.enabled && model.active_provider_id.is_some())?;
    let provider_id = model.active_provider_id.as_deref()?;
    let provider = match config.providers.iter().find(|provider| {
        provider.id == provider_id
            && provider.enabled
            && model.provider_ids.iter().any(|id| id == provider_id)
    }) {
        Some(provider) => provider,
        None => return None,
    };

    Some(
        provider_connection_config(provider)
            .map(|provider_config| (model.id.clone(), provider.id.clone(), provider_config)),
    )
}

fn configured_workspace_summary(
    workspace: &WorkspaceConfig,
) -> Result<ConfiguredWorkspaceSummary, ApiError> {
    Ok(ConfiguredWorkspaceSummary {
        id: workspace.id.clone(),
        name: workspace.name.clone(),
        path: display_path(&workspace.path),
        logo_url: workspace_logo_url(workspace)?,
        pinned: workspace.pinned,
        terminal_shell: workspace.terminal_shell.clone(),
        is_default: workspace.id == foco_store::config::DEFAULT_WORKSPACE_ID,
    })
}

fn terminal_shell_summaries() -> Vec<TerminalShellSummary> {
    SUPPORTED_TERMINAL_SHELLS
        .iter()
        .map(|shell| TerminalShellSummary {
            shell: *shell,
            label: terminal_shell_label(shell),
        })
        .collect()
}

fn api_proxy_type_summaries() -> Vec<ApiProxyTypeSummary> {
    SUPPORTED_API_PROXY_TYPES
        .iter()
        .map(|proxy_type| ApiProxyTypeSummary {
            proxy_type: *proxy_type,
            label: api_proxy_type_label(proxy_type),
        })
        .collect()
}

fn api_proxy_type_label(proxy_type: &str) -> &'static str {
    match proxy_type {
        "http" => "HTTP",
        "socks" => "SOCKS",
        _ => "Unknown",
    }
}

fn terminal_shell_label(shell: &str) -> &'static str {
    match shell {
        "powershell" => "PowerShell",
        "cmd" => "Command Prompt",
        "bash" => "Bash",
        "zsh" => "Zsh",
        _ => "Unknown",
    }
}

fn configured_provider_summary(provider: &ProviderSettings) -> ConfiguredProviderSummary {
    ConfiguredProviderSummary {
        api_proxy: ApiProxySettingsSummary {
            enabled: provider.api_proxy.enabled,
            proxy_type: provider.api_proxy.proxy_type.clone(),
            url: provider.api_proxy.url.clone(),
            supported_types: api_proxy_type_summaries(),
        },
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

fn skills_settings_summary(
    config: &GlobalConfig,
    user_profile_dir: &Path,
) -> SkillsSettingsSummary {
    let disabled_skill_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(user_profile_dir, &config.workspaces);

    SkillsSettingsSummary {
        directories: skill_search_roots(user_profile_dir, &config.workspaces)
            .iter()
            .map(|root| display_path(&root.directory))
            .collect(),
        detected: discovery
            .skills
            .iter()
            .map(|skill| {
                configured_skill_summary(skill, !skill_is_disabled(skill, &disabled_skill_ids))
            })
            .collect(),
        errors: discovery.errors,
    }
}

fn configured_skill_summary(skill: &SkillSettings, enabled: bool) -> ConfiguredSkillSummary {
    ConfiguredSkillSummary {
        key: skill.key.clone(),
        id: skill.id.clone(),
        name: skill.name.clone(),
        description: skill.description.clone(),
        path: skill.path.display().to_string(),
        scope: skill.scope.clone(),
        workspace_id: skill.workspace_id.clone(),
        workspace_name: skill.workspace_name.clone(),
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

fn message_with_selected_skills(
    user_profile_dir: &Path,
    config: &GlobalConfig,
    workspace_id: &str,
    requested_skill_keys: Option<Vec<String>>,
    message: &str,
) -> Result<String, ApiError> {
    let Some(requested_skill_keys) = requested_skill_keys else {
        return Ok(message.to_string());
    };
    let requested_skill_keys = normalize_skill_keys(requested_skill_keys)?;
    if requested_skill_keys.is_empty() {
        return Ok(message.to_string());
    }

    let disabled_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(user_profile_dir, &config.workspaces);
    if let Some(error) = discovery.errors.first() {
        return Err(ApiError::bad_request(format!(
            "skill discovery failed for {}: {}",
            error.path, error.message
        )));
    }

    let available_skills = discovery
        .skills
        .iter()
        .filter(|skill| skill_applies_to_workspace(skill, workspace_id))
        .collect::<Vec<_>>();
    let skills_by_key = available_skills
        .iter()
        .map(|skill| (skill.key.as_str(), *skill))
        .collect::<HashMap<_, _>>();
    let mut links = Vec::with_capacity(requested_skill_keys.len());
    for skill_key in requested_skill_keys {
        let skill = match skills_by_key.get(skill_key.as_str()).copied() {
            Some(skill) => skill,
            None => unique_skill_by_legacy_id(&available_skills, &skill_key)?,
        };
        if skill_is_disabled(skill, &disabled_ids) {
            return Err(ApiError::bad_request(format!(
                "selected skill '{}' is disabled",
                skill.key
            )));
        }

        let parsed = parse_skill_file(&skill.path).map_err(ApiError::bad_request)?;
        if parsed.id != skill.id {
            return Err(ApiError::bad_request(format!(
                "selected skill '{}' file now declares skill id '{}'",
                skill.key, parsed.id
            )));
        }

        links.push(format!("[${}]({})", skill.name, skill.path.display()));
    }

    Ok(format!("{} {}", links.join(" "), message))
}

struct SkillDiscovery {
    skills: Vec<SkillSettings>,
    errors: Vec<SkillDiscoveryErrorSummary>,
}

#[derive(Clone, Debug)]
struct SkillSearchRoot {
    directory: PathBuf,
    scope: &'static str,
    workspace_id: Option<String>,
    workspace_name: Option<String>,
}

#[derive(Debug)]
struct ParsedSkillFile {
    id: String,
    name: String,
    description: String,
    frontmatter: String,
}

fn normalize_skill_keys(values: Vec<String>) -> Result<Vec<String>, ApiError> {
    let mut keys = Vec::new();
    let mut seen = HashSet::new();

    for value in values {
        let key = value.trim();

        if key.is_empty() {
            continue;
        }

        validate_skill_key(key).map_err(ApiError::bad_request)?;
        if seen.insert(key.to_string()) {
            keys.push(key.to_string());
        }
    }

    Ok(keys)
}

fn normalize_manual_disabled_skill_ids(
    requested_disabled: Option<Vec<String>>,
    requested_enabled: Option<Vec<String>>,
    discovered_skills: &[SkillSettings],
) -> Result<Vec<String>, ApiError> {
    let discovered_keys = discovered_skills
        .iter()
        .map(|skill| skill.key.as_str())
        .collect::<HashSet<_>>();

    if let Some(values) = requested_disabled {
        let disabled = normalize_skill_keys(values)?;

        for key in &disabled {
            if !discovered_keys.contains(key.as_str()) {
                return Err(ApiError::bad_request(format!(
                    "disabled skill was not found: {key}"
                )));
            }
        }

        if let Some(enabled_values) = requested_enabled {
            let enabled = normalize_skill_keys(enabled_values)?;
            let enabled_keys = enabled.iter().map(String::as_str).collect::<HashSet<_>>();
            if let Some(key) = disabled
                .iter()
                .find(|key| enabled_keys.contains(key.as_str()))
            {
                return Err(ApiError::bad_request(format!(
                    "skill cannot be both enabled and disabled: {key}"
                )));
            }
        }

        return Ok(disabled);
    }

    if let Some(values) = requested_enabled {
        let enabled = normalize_skill_keys(values)?;
        let enabled_ids = enabled.iter().map(String::as_str).collect::<HashSet<_>>();
        for key in &enabled {
            if !discovered_keys.contains(key.as_str()) {
                return Err(ApiError::bad_request(format!(
                    "enabled skill was not found: {key}"
                )));
            }
        }

        return Ok(discovered_skills
            .iter()
            .filter(|skill| !enabled_ids.contains(skill.key.as_str()))
            .map(|skill| skill.key.clone())
            .collect());
    }

    Ok(Vec::new())
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
        .filter(|skill| !skill_is_disabled(skill, &disabled_ids))
        .map(|skill| skill.key.clone())
        .collect();
}

fn discover_skills(user_profile_dir: &Path, workspaces: &[WorkspaceConfig]) -> SkillDiscovery {
    let mut skills = Vec::new();
    let mut errors = Vec::new();
    let mut seen_keys = HashSet::new();

    for root in skill_search_roots(user_profile_dir, workspaces) {
        let candidates = match skill_file_candidates(&root.directory) {
            Ok(candidates) => candidates,
            Err(message) => {
                errors.push(SkillDiscoveryErrorSummary {
                    path: root.directory.display().to_string(),
                    message,
                });
                continue;
            }
        };

        for path in candidates {
            match parse_skill_file(&path) {
                Ok(parsed) => {
                    let key = skill_key(&root, &parsed.id);
                    if !seen_keys.insert(key.clone()) {
                        errors.push(SkillDiscoveryErrorSummary {
                            path: path.display().to_string(),
                            message: format!(
                                "duplicate skill id '{}' in {} skill scope",
                                parsed.id,
                                skill_scope_label(&root)
                            ),
                        });
                        continue;
                    }

                    skills.push(SkillSettings {
                        key,
                        id: parsed.id,
                        name: parsed.name,
                        description: parsed.description,
                        path,
                        scope: root.scope.to_string(),
                        workspace_id: root.workspace_id.clone(),
                        workspace_name: root.workspace_name.clone(),
                    });
                }
                Err(message) => errors.push(SkillDiscoveryErrorSummary {
                    path: path.display().to_string(),
                    message,
                }),
            }
        }
    }

    skills.sort_by(|left, right| {
        left.scope
            .cmp(&right.scope)
            .then_with(|| left.workspace_name.cmp(&right.workspace_name))
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.path.cmp(&right.path))
    });

    SkillDiscovery { skills, errors }
}

fn skill_search_roots(
    user_profile_dir: &Path,
    workspaces: &[WorkspaceConfig],
) -> Vec<SkillSearchRoot> {
    let mut roots = Vec::new();

    roots.push(SkillSearchRoot {
        directory: user_profile_dir.join(".agents").join("skills"),
        scope: SKILL_SCOPE_GLOBAL,
        workspace_id: None,
        workspace_name: None,
    });

    for workspace in workspaces {
        for directory in [
            workspace.path.join(".agents").join("skills"),
            workspace.path.join(".claude").join("skills"),
        ] {
            roots.push(SkillSearchRoot {
                directory,
                scope: SKILL_SCOPE_WORKSPACE,
                workspace_id: Some(workspace.id.clone()),
                workspace_name: Some(workspace.name.clone()),
            });
        }
    }

    roots
}

fn skill_key(root: &SkillSearchRoot, skill_id: &str) -> String {
    match root.scope {
        SKILL_SCOPE_GLOBAL => format!("global:{skill_id}"),
        SKILL_SCOPE_WORKSPACE => {
            let workspace_id = root.workspace_id.as_deref().unwrap_or_default();
            format!("workspace:{workspace_id}:{skill_id}")
        }
        scope => format!("{scope}:{skill_id}"),
    }
}

fn skill_scope_label(root: &SkillSearchRoot) -> String {
    match root.scope {
        SKILL_SCOPE_GLOBAL => "global".to_string(),
        SKILL_SCOPE_WORKSPACE => format!(
            "workspace '{}'",
            root.workspace_name
                .as_deref()
                .or(root.workspace_id.as_deref())
                .unwrap_or("")
        ),
        scope => scope.to_string(),
    }
}

fn skill_is_disabled(skill: &SkillSettings, disabled_ids: &HashSet<&str>) -> bool {
    disabled_ids.contains(skill.key.as_str()) || disabled_ids.contains(skill.id.as_str())
}

fn skill_applies_to_workspace(skill: &SkillSettings, workspace_id: &str) -> bool {
    skill.scope == SKILL_SCOPE_GLOBAL
        || (skill.scope == SKILL_SCOPE_WORKSPACE
            && skill.workspace_id.as_deref() == Some(workspace_id))
}

fn unique_skill_by_legacy_id<'a>(
    skills: &[&'a SkillSettings],
    legacy_id: &str,
) -> Result<&'a SkillSettings, ApiError> {
    let matches = skills
        .iter()
        .copied()
        .filter(|skill| skill.id == legacy_id)
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [skill] => Ok(*skill),
        [] => Err(ApiError::bad_request(format!(
            "selected skill was not found: {legacy_id}"
        ))),
        _ => Err(ApiError::bad_request(format!(
            "selected skill id '{legacy_id}' is ambiguous; use a scoped skill key"
        ))),
    }
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
        frontmatter: frontmatter.join("\n"),
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

fn validate_skill_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("skill key must not be empty".to_string());
    }

    if key.chars().any(char::is_whitespace) {
        return Err(format!("skill key '{}' must not contain whitespace", key));
    }

    Ok(())
}

fn enabled_skill_frontmatter_messages(
    user_profile_dir: &Path,
    config: &GlobalConfig,
    workspace_id: &str,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    let disabled_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(user_profile_dir, &config.workspaces);
    if let Some(error) = discovery.errors.first() {
        return Err(ApiError::bad_request(format!(
            "skill discovery failed for {}: {}",
            error.path, error.message
        )));
    }

    let mut entries = Vec::new();
    for skill in discovery.skills.iter().filter(|skill| {
        skill_applies_to_workspace(skill, workspace_id) && !skill_is_disabled(skill, &disabled_ids)
    }) {
        let parsed = parse_skill_file(&skill.path).map_err(ApiError::bad_request)?;

        if parsed.id != skill.id {
            return Err(ApiError::bad_request(format!(
                "enabled skill '{}' file now declares skill id '{}'",
                skill.key, parsed.id
            )));
        }

        entries.push(skill_frontmatter_entry(&skill.path, parsed));
    }

    if entries.is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![neutral_text_message(
        NeutralChatRole::User,
        format!(
            "{ENABLED_SKILLS_MESSAGE_PREFIX}:\n\n{}",
            entries.join("\n\n")
        ),
    )])
}

fn skill_frontmatter_entry(path: &Path, skill: ParsedSkillFile) -> String {
    format!(
        "path: {}\n---\n{}\n---",
        path.display(),
        skill.frontmatter.trim()
    )
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
            path: display_path(&workspace.path),
            logo_url: workspace_logo_url(workspace)?,
            pinned: workspace.pinned,
            terminal_shell: workspace.terminal_shell.clone(),
            chats,
        });
    }

    Ok(Json(WorkspacesResponse {
        active_workspace_id: config.app.active_workspace_id.clone(),
        workspaces,
    }))
}

fn task_graph_response(chat_id: &str, graph: Option<TaskGraphRecord>) -> TaskGraphResponse {
    match graph {
        Some(graph) => TaskGraphResponse {
            chat_id: graph.chat_id,
            exists: true,
            tasks: graph.tasks,
            created_at: Some(graph.created_at),
            updated_at: Some(graph.updated_at),
        },
        None => TaskGraphResponse {
            chat_id: chat_id.to_string(),
            exists: false,
            tasks: Vec::new(),
            created_at: None,
            updated_at: None,
        },
    }
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
        proxy_url: provider
            .api_proxy
            .enabled
            .then(|| provider.api_proxy.url.clone()),
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

fn reorder_models(models: &mut Vec<ModelSettings>, model_ids: Vec<String>) -> Result<(), ApiError> {
    if model_ids.len() != models.len() {
        return Err(ApiError::bad_request(format!(
            "model order must contain exactly {} model ids",
            models.len()
        )));
    }

    let mut seen = HashSet::new();
    let stored_model_ids = models
        .iter()
        .map(|model| model.id.clone())
        .collect::<HashSet<_>>();
    let mut normalized_model_ids = Vec::with_capacity(model_ids.len());

    for raw_model_id in model_ids {
        let model_id = raw_model_id.trim().to_string();

        if model_id.is_empty() {
            return Err(ApiError::bad_request("model id must not be empty"));
        }

        if !seen.insert(model_id.clone()) {
            return Err(ApiError::bad_request(format!(
                "model order contains duplicate id: {model_id}"
            )));
        }

        if !stored_model_ids.contains(&model_id) {
            return Err(ApiError::bad_request(format!(
                "model was not found: {model_id}"
            )));
        }

        normalized_model_ids.push(model_id);
    }

    let mut stored_models = models
        .drain(..)
        .map(|model| (model.id.clone(), model))
        .collect::<HashMap<_, _>>();
    let reordered_models = normalized_model_ids
        .into_iter()
        .map(|model_id| {
            stored_models
                .remove(&model_id)
                .expect("model id was validated before reorder")
        })
        .collect();

    *models = reordered_models;

    Ok(())
}

fn reorder_workspaces(
    workspaces: &mut Vec<WorkspaceConfig>,
    workspace_ids: Vec<String>,
) -> Result<(), ApiError> {
    if workspace_ids.len() != workspaces.len() {
        return Err(ApiError::bad_request(format!(
            "workspace order must contain exactly {} workspace ids",
            workspaces.len()
        )));
    }

    let mut seen = HashSet::new();
    let stored_workspace_ids = workspaces
        .iter()
        .map(|workspace| workspace.id.clone())
        .collect::<HashSet<_>>();
    let mut normalized_workspace_ids = Vec::with_capacity(workspace_ids.len());

    for raw_workspace_id in workspace_ids {
        let workspace_id = raw_workspace_id.trim().to_string();

        if workspace_id.is_empty() {
            return Err(ApiError::bad_request("workspace id must not be empty"));
        }

        if !seen.insert(workspace_id.clone()) {
            return Err(ApiError::bad_request(format!(
                "workspace order contains duplicate id: {workspace_id}"
            )));
        }

        if !stored_workspace_ids.contains(&workspace_id) {
            return Err(ApiError::bad_request(format!(
                "workspace was not found: {workspace_id}"
            )));
        }

        normalized_workspace_ids.push(workspace_id);
    }

    let mut stored_workspaces = workspaces
        .drain(..)
        .map(|workspace| (workspace.id.clone(), workspace))
        .collect::<HashMap<_, _>>();
    let reordered_workspaces = normalized_workspace_ids
        .into_iter()
        .map(|workspace_id| {
            stored_workspaces
                .remove(&workspace_id)
                .expect("workspace id was validated before reorder")
        })
        .collect();

    *workspaces = reordered_workspaces;

    Ok(())
}

fn group_pinned_workspaces(workspaces: &mut Vec<WorkspaceConfig>) {
    let mut pinned = Vec::new();
    let mut unpinned = Vec::new();

    for workspace in workspaces.drain(..) {
        if workspace.pinned {
            pinned.push(workspace);
        } else {
            unpinned.push(workspace);
        }
    }

    pinned.extend(unpinned);
    *workspaces = pinned;
}

fn normalize_terminal_shell(shell: &str) -> Result<String, ApiError> {
    let shell = shell.trim();

    if SUPPORTED_TERMINAL_SHELLS.contains(&shell) {
        return Ok(shell.to_string());
    }

    Err(ApiError::bad_request(format!(
        "terminal shell '{shell}' is unsupported; expected one of {}",
        SUPPORTED_TERMINAL_SHELLS.join(", ")
    )))
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

fn normalized_ai_statistics_query(
    query: AiStatisticsQuery,
) -> Result<NormalizedAiStatisticsFilters, ApiError> {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.or(query.limit).unwrap_or(50);

    if page < 1 {
        return Err(ApiError::bad_request(
            "AI statistics page must be a positive integer",
        ));
    }

    if page_size < 1 {
        return Err(ApiError::bad_request(
            "AI statistics page size must be a positive integer",
        ));
    }

    let page_size = page_size.min(500);
    let offset = (page - 1)
        .checked_mul(page_size)
        .ok_or_else(|| ApiError::bad_request("AI statistics page offset is too large"))?;

    Ok(NormalizedAiStatisticsFilters {
        workspace_id: optional_trimmed_string(query.workspace_id),
        chat_id: optional_trimmed_string(query.chat_id),
        provider_id: optional_trimmed_string(query.provider_id),
        model_id: optional_trimmed_string(query.model_id),
        status: optional_trimmed_string(query.status),
        started_after: optional_trimmed_string(query.started_after),
        started_before: optional_trimmed_string(query.started_before),
        page,
        page_size,
        offset,
    })
}

fn ai_statistics_workspaces<'a>(
    config: &'a GlobalConfig,
    workspace_id: Option<&str>,
) -> Result<Vec<&'a WorkspaceConfig>, ApiError> {
    if let Some(workspace_id) = workspace_id {
        return Ok(vec![workspace_by_id(config, workspace_id)?]);
    }

    Ok(config.workspaces.iter().collect())
}

fn chat_title_map(database: &WorkspaceDatabase) -> Result<HashMap<String, String>, ApiError> {
    Ok(database
        .chats()
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(|chat| (chat.id, chat.title))
        .collect())
}

fn ai_request_audit_summary(
    row: LlmRequestAuditRow,
    workspace: &WorkspaceConfig,
    chat_titles: &HashMap<String, String>,
) -> AiRequestAuditSummary {
    AiRequestAuditSummary {
        id: row.id,
        workspace_id: workspace.id.clone(),
        workspace_name: workspace.name.clone(),
        chat_title: row
            .chat_id
            .as_ref()
            .and_then(|chat_id| chat_titles.get(chat_id).cloned()),
        chat_id: row.chat_id,
        provider_id: row.provider_id,
        model_id: row.model_id,
        request_started_at: row.request_started_at,
        first_token_at: row.first_token_at,
        completed_at: row.completed_at,
        input_tokens: row.input_tokens,
        output_tokens: row.output_tokens,
        cache_read_tokens: row.cache_read_tokens,
        cache_write_tokens: row.cache_write_tokens,
        cache_ratio: row.cache_ratio,
        first_token_latency_ms: row.first_token_latency_ms,
        total_latency_ms: row.total_latency_ms,
        status_code: row.status_code,
        final_state: row.final_state,
    }
}

fn ai_request_audit_detail(
    request: LlmRequestRecord,
    workspace: &WorkspaceConfig,
    chat_titles: &HashMap<String, String>,
) -> Result<AiRequestAuditDetail, ApiError> {
    Ok(AiRequestAuditDetail {
        id: request.id,
        workspace_id: workspace.id.clone(),
        workspace_name: workspace.name.clone(),
        chat_title: request
            .chat_id
            .as_ref()
            .and_then(|chat_id| chat_titles.get(chat_id).cloned()),
        chat_id: request.chat_id,
        provider_id: request.provider_id,
        model_id: request.model_id,
        request_started_at: request.request_started_at,
        first_token_at: request.first_token_at,
        completed_at: request.completed_at,
        input_tokens: request.input_tokens,
        output_tokens: request.output_tokens,
        cache_read_tokens: request.cache_read_tokens,
        cache_write_tokens: request.cache_write_tokens,
        cache_ratio: request.cache_ratio,
        first_token_latency_ms: request.first_token_latency_ms,
        total_latency_ms: request.total_latency_ms,
        status_code: request.status_code,
        final_state: request.final_state,
        request_body: parse_optional_json_value(request.request_body_json, "LLM request body")?,
        response_body: parse_optional_json_value(request.response_body_json, "LLM response body")?,
    })
}

fn ai_request_audit_event_summary(
    event: LlmRequestEventRecord,
) -> Result<AiRequestAuditEventSummary, ApiError> {
    Ok(AiRequestAuditEventSummary {
        id: event.id,
        sequence: event.sequence,
        event_at: event.event_at,
        event_type: event.event_type,
        raw_chunk: parse_optional_json_value(event.raw_chunk_json, "LLM raw stream chunk")?,
        normalized_event: parse_json_value(
            &event.normalized_event_json,
            "LLM normalized stream event",
        )?,
    })
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
        status: "running".to_string(),
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

fn user_message_metadata_json(attachments: &[NeutralChatAttachment]) -> Result<String, ApiError> {
    if attachments.is_empty() {
        return Ok("{}".to_string());
    }

    serde_json::to_string(&json!({ "attachments": attachments })).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize user message metadata: {source}"
        ))
    })
}

fn message_attachments_from_metadata(
    metadata_json: &str,
) -> Result<Vec<NeutralChatAttachment>, ApiError> {
    let metadata = parse_json_value(metadata_json, "user message metadata")?;
    let Some(attachments) = metadata.get("attachments") else {
        return Ok(Vec::new());
    };

    if attachments.is_null() {
        return Ok(Vec::new());
    }

    let attachments = serde_json::from_value::<Vec<NeutralChatAttachment>>(attachments.clone())
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to parse user message attachments: {source}"
            ))
        })?;
    validate_stored_chat_attachments(&attachments)?;

    Ok(attachments)
}

fn normalized_chat_attachments(
    inputs: Vec<ChatAttachmentInput>,
) -> Result<Vec<NeutralChatAttachment>, ApiError> {
    if inputs.len() > MAX_CHAT_ATTACHMENTS {
        return Err(ApiError::bad_request(format!(
            "at most {MAX_CHAT_ATTACHMENTS} attachments are allowed"
        )));
    }

    let mut attachments = Vec::with_capacity(inputs.len());
    let mut seen_ids = HashSet::new();
    let mut total_size = 0_u64;

    for (index, input) in inputs.into_iter().enumerate() {
        let id = input.id.trim().to_string();
        let name = input.name.trim().to_string();
        let content_type = input.content_type.trim().to_string();
        let content_base64 = input
            .content_base64
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let path = input
            .path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        if id.is_empty() {
            return Err(ApiError::bad_request(format!(
                "attachment {} id must not be empty",
                index + 1
            )));
        }

        if !seen_ids.insert(id.clone()) {
            return Err(ApiError::bad_request(format!(
                "duplicate attachment id: {id}"
            )));
        }

        if name.is_empty() {
            return Err(ApiError::bad_request(format!(
                "attachment {id} name must not be empty"
            )));
        }

        if content_type.is_empty() {
            return Err(ApiError::bad_request(format!(
                "attachment {name} content type must not be empty"
            )));
        }

        if input.size_bytes > MAX_CHAT_ATTACHMENT_BYTES {
            return Err(ApiError::bad_request(format!(
                "attachment {name} exceeds the {} byte limit",
                MAX_CHAT_ATTACHMENT_BYTES
            )));
        }

        total_size = total_size
            .checked_add(input.size_bytes)
            .ok_or_else(|| ApiError::bad_request("attachment total size exceeds u64"))?;
        if total_size > MAX_CHAT_ATTACHMENT_TOTAL_BYTES {
            return Err(ApiError::bad_request(format!(
                "attachments exceed the {} byte total limit",
                MAX_CHAT_ATTACHMENT_TOTAL_BYTES
            )));
        }

        if is_inline_binary_attachment(&content_type) {
            if path.is_some() {
                return Err(ApiError::bad_request(format!(
                    "attachment {name} images must use contentBase64, not path"
                )));
            }

            let content_base64 = content_base64.ok_or_else(|| {
                ApiError::bad_request(format!(
                    "attachment {name} image contentBase64 must not be empty"
                ))
            })?;
            validate_attachment_base64(&name, &content_base64, input.size_bytes)?;

            attachments.push(NeutralChatAttachment {
                id,
                name,
                content_type,
                size_bytes: input.size_bytes,
                content_base64: Some(content_base64),
                path: None,
            });
            continue;
        }

        if content_base64.is_some() {
            return Err(ApiError::bad_request(format!(
                "attachment {name} must use path; contentBase64 is only accepted for image attachments"
            )));
        }

        let path = path.ok_or_else(|| {
            ApiError::bad_request(format!("attachment {name} path must not be empty"))
        })?;
        validate_attachment_file_path(&name, &path, input.size_bytes)?;

        attachments.push(NeutralChatAttachment {
            id,
            name,
            content_type,
            size_bytes: input.size_bytes,
            content_base64: None,
            path: Some(path),
        });
    }

    Ok(attachments)
}

fn validate_stored_chat_attachments(attachments: &[NeutralChatAttachment]) -> Result<(), ApiError> {
    for attachment in attachments {
        if attachment.id.trim().is_empty() {
            return Err(ApiError::internal("stored attachment id must not be empty"));
        }
        if attachment.name.trim().is_empty() {
            return Err(ApiError::internal(
                "stored attachment name must not be empty",
            ));
        }
        if attachment.content_type.trim().is_empty() {
            return Err(ApiError::internal(
                "stored attachment content type must not be empty",
            ));
        }
        if attachment.content_base64.is_none() && attachment.path.is_none() {
            return Err(ApiError::internal(
                "stored attachment must have contentBase64 or path",
            ));
        }
    }

    Ok(())
}

fn is_inline_binary_attachment(content_type: &str) -> bool {
    content_type.starts_with("image/")
}

fn validate_attachment_base64(
    name: &str,
    content_base64: &str,
    size_bytes: u64,
) -> Result<(), ApiError> {
    if content_base64.contains(',') {
        return Err(ApiError::bad_request(format!(
            "attachment {name} contentBase64 must be raw base64, not a data URL"
        )));
    }

    let decoded = general_purpose::STANDARD
        .decode(content_base64.as_bytes())
        .map_err(|source| {
            ApiError::bad_request(format!("attachment {name} has invalid base64: {source}"))
        })?;
    let decoded_len = u64::try_from(decoded.len())
        .map_err(|_| ApiError::bad_request(format!("attachment {name} size exceeds u64")))?;

    if decoded_len != size_bytes {
        return Err(ApiError::bad_request(format!(
            "attachment {name} sizeBytes does not match decoded content"
        )));
    }

    Ok(())
}

fn validate_attachment_file_path(name: &str, path: &str, size_bytes: u64) -> Result<(), ApiError> {
    let path_value = Path::new(path);
    if !path_value.is_absolute() {
        return Err(ApiError::bad_request(format!(
            "attachment {name} path must be absolute"
        )));
    }

    if path_value
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ApiError::bad_request(format!(
            "attachment {name} path must not contain parent directory components"
        )));
    }

    let metadata = fs::metadata(path_value).map_err(|source| {
        ApiError::bad_request(format!("attachment {name} path is not readable: {source}"))
    })?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "attachment {name} path must point to a file"
        )));
    }

    if metadata.len() != size_bytes {
        return Err(ApiError::bad_request(format!(
            "attachment {name} sizeBytes does not match file size"
        )));
    }

    Ok(())
}

fn chat_attachment_hook_summaries(attachments: &[NeutralChatAttachment]) -> Vec<Value> {
    attachments
        .iter()
        .map(|attachment| {
            json!({
                "id": attachment.id,
                "name": attachment.name,
                "contentType": attachment.content_type,
                "path": attachment.path.as_deref(),
                "sizeBytes": attachment.size_bytes,
            })
        })
        .collect()
}

fn chat_attachment_message_parts(metadata_json: &str) -> Result<Vec<ChatMessagePart>, ApiError> {
    Ok(message_attachments_from_metadata(metadata_json)?
        .into_iter()
        .map(|attachment| ChatMessagePart::Attachment {
            attachment: chat_attachment_part(attachment),
        })
        .collect())
}

fn chat_attachment_part(attachment: NeutralChatAttachment) -> ChatAttachmentPart {
    let preview_data_url = if attachment.content_type.starts_with("image/") {
        attachment.content_base64.as_ref().map(|content_base64| {
            format!("data:{};base64,{}", attachment.content_type, content_base64)
        })
    } else {
        None
    };

    ChatAttachmentPart {
        id: attachment.id,
        name: attachment.name,
        content_type: attachment.content_type,
        size_bytes: attachment.size_bytes,
        path: attachment.path,
        preview_data_url,
    }
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
    llm_request_events: &[LlmRequestEventRecord],
) -> Result<ChatMessageSummary, ApiError> {
    let tool_calls = database
        .tool_calls_for_message(&message.id)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(chat_tool_call_summary)
        .collect::<Result<Vec<_>, _>>()?;
    let reasoning = if message.role == "assistant" {
        assistant_reasoning_from_metadata(&message.metadata_json)?
    } else {
        None
    };
    let parts = chat_message_parts(
        &message,
        reasoning.as_deref(),
        &tool_calls,
        llm_request_events,
    )?;
    let metrics = if message.role == "assistant" {
        assistant_reply_metrics(database, &message.id, llm_request_events)?
    } else {
        None
    };

    Ok(ChatMessageSummary {
        id: message.id,
        reasoning,
        role: message.role,
        content: message.content,
        tool_calls,
        parts,
        metrics,
    })
}

fn assistant_reply_metrics(
    database: &WorkspaceDatabase,
    message_id: &str,
    llm_request_events: &[LlmRequestEventRecord],
) -> Result<Option<ChatReplyMetrics>, ApiError> {
    let request_ids = assistant_message_request_ids(message_id, llm_request_events)?;
    let Some(request_id) = request_ids.first() else {
        return Ok(None);
    };

    if request_ids.len() > 1 {
        return Err(ApiError::internal(format!(
            "assistant message '{message_id}' is linked to multiple LLM requests"
        )));
    }

    let request = database
        .llm_request(request_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::internal(format!(
                "assistant message '{message_id}' is linked to missing LLM request '{request_id}'"
            ))
        })?;

    Ok(Some(chat_reply_metrics_from_request(&request)))
}

fn chat_reply_metrics_from_request(request: &LlmRequestRecord) -> ChatReplyMetrics {
    ChatReplyMetrics {
        model_id: request.model_id.clone(),
        provider_id: request.provider_id.clone(),
        total_latency_ms: request.total_latency_ms,
        first_token_latency_ms: request.first_token_latency_ms,
        output_tokens: request.output_tokens,
    }
}

fn chat_message_parts(
    message: &MessageRecord,
    reasoning: Option<&str>,
    tool_calls: &[ChatToolCallSummary],
    llm_request_events: &[LlmRequestEventRecord],
) -> Result<Vec<ChatMessagePart>, ApiError> {
    if message.role != "assistant" {
        let mut parts = fallback_chat_message_parts(&message.content, None, &[]);
        if message.role == "user" {
            parts.extend(chat_attachment_message_parts(&message.metadata_json)?);
        }
        return Ok(parts);
    }

    let request_ids = assistant_message_request_ids(&message.id, llm_request_events)?;
    if request_ids.is_empty() {
        return Ok(fallback_chat_message_parts(
            &message.content,
            reasoning,
            tool_calls,
        ));
    }

    let tool_calls_by_id = tool_calls
        .iter()
        .map(|tool_call| (tool_call.id.as_str(), tool_call))
        .collect::<HashMap<_, _>>();
    let request_ids = request_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut seen_tool_call_ids = HashSet::new();
    let mut parts = Vec::new();

    for event in llm_request_events
        .iter()
        .filter(|event| request_ids.contains(event.llm_request_id.as_str()))
    {
        match event.event_type.as_str() {
            "text_delta" => {
                let value = parse_json_value(&event.normalized_event_json, "LLM text event")?;
                if event_matches_assistant_message(&value, &message.id) {
                    if let Some(delta) = string_json_field(&value, "delta", "delta") {
                        push_text_part(&mut parts, delta);
                    }
                }
            }
            "reasoning_delta" => {
                let value = parse_json_value(&event.normalized_event_json, "LLM reasoning event")?;
                if event_matches_assistant_message(&value, &message.id) {
                    if let Some(delta) = string_json_field(&value, "delta", "delta") {
                        push_reasoning_part(&mut parts, delta);
                    }
                }
            }
            "tool_call" => {
                let value = parse_json_value(&event.normalized_event_json, "LLM tool call event")?;
                if !event_matches_assistant_message(&value, &message.id) {
                    continue;
                }

                let Some(tool_call_value) =
                    value.get("toolCall").or_else(|| value.get("tool_call"))
                else {
                    continue;
                };
                let Some(tool_call_id) = string_json_field(tool_call_value, "id", "callId")
                    .or_else(|| string_json_field(tool_call_value, "call_id", "callId"))
                else {
                    continue;
                };

                if !seen_tool_call_ids.insert(tool_call_id.to_string()) {
                    continue;
                }

                let Some(tool_call) = tool_calls_by_id.get(tool_call_id) else {
                    return Err(ApiError::internal(format!(
                        "tool call event referenced unknown tool call id: {tool_call_id}"
                    )));
                };

                parts.push(ChatMessagePart::ToolCall {
                    tool_call: (*tool_call).clone(),
                });
            }
            _ => {}
        }
    }

    if parts.is_empty() {
        Ok(fallback_chat_message_parts(
            &message.content,
            reasoning,
            tool_calls,
        ))
    } else {
        Ok(parts)
    }
}

fn assistant_message_request_ids(
    message_id: &str,
    llm_request_events: &[LlmRequestEventRecord],
) -> Result<Vec<String>, ApiError> {
    let mut request_ids = Vec::new();
    for event in llm_request_events
        .iter()
        .filter(|event| event.event_type == "start")
    {
        let value = parse_json_value(&event.normalized_event_json, "LLM start event")?;
        if string_json_field(&value, "assistantMessageId", "assistant_message_id")
            == Some(message_id)
        {
            request_ids.push(event.llm_request_id.clone());
        }
    }

    Ok(request_ids)
}

fn event_matches_assistant_message(value: &Value, message_id: &str) -> bool {
    match string_json_field(value, "assistantMessageId", "assistant_message_id") {
        Some(assistant_message_id) => assistant_message_id == message_id,
        None => true,
    }
}

fn string_json_field<'a>(value: &'a Value, primary: &str, alternate: &str) -> Option<&'a str> {
    value
        .get(primary)
        .or_else(|| value.get(alternate))
        .and_then(Value::as_str)
}

fn fallback_chat_message_parts(
    content: &str,
    reasoning: Option<&str>,
    tool_calls: &[ChatToolCallSummary],
) -> Vec<ChatMessagePart> {
    let mut parts = Vec::new();
    if let Some(reasoning) = reasoning {
        push_reasoning_part(&mut parts, reasoning);
    }
    push_text_part(&mut parts, content);
    parts.extend(
        tool_calls
            .iter()
            .cloned()
            .map(|tool_call| ChatMessagePart::ToolCall { tool_call }),
    );
    parts
}

fn push_text_part(parts: &mut Vec<ChatMessagePart>, text: &str) {
    if text.is_empty() {
        return;
    }

    match parts.last_mut() {
        Some(ChatMessagePart::Text { text: existing }) => existing.push_str(text),
        _ => parts.push(ChatMessagePart::Text {
            text: text.to_string(),
        }),
    }
}

fn push_reasoning_part(parts: &mut Vec<ChatMessagePart>, text: &str) {
    if text.is_empty() {
        return;
    }

    match parts.last_mut() {
        Some(ChatMessagePart::Reasoning { text: existing }) => existing.push_str(text),
        _ => parts.push(ChatMessagePart::Reasoning {
            text: text.to_string(),
        }),
    }
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

fn parse_optional_json_value(
    value: Option<String>,
    field: &str,
) -> Result<Option<Value>, ApiError> {
    value
        .as_deref()
        .map(|value| parse_json_value(value, field))
        .transpose()
}

fn canonical_workspace_path(path: &Path) -> Result<PathBuf, ApiError> {
    fs::canonicalize(path)
        .map(normalize_windows_verbatim_path)
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to resolve workspace path {}: {}",
                path.display(),
                source
            ))
        })
}

fn reject_registered_workspace_path(
    config: &GlobalConfig,
    path: &Path,
    allowed_workspace_id: Option<&str>,
) -> Result<(), ApiError> {
    for workspace in &config.workspaces {
        if allowed_workspace_id == Some(workspace.id.as_str()) {
            continue;
        }

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

fn normalize_windows_verbatim_path(path: PathBuf) -> PathBuf {
    let value = path.display().to_string();

    if let Some(stripped) = value.strip_prefix("\\\\?\\UNC\\") {
        return PathBuf::from(format!("\\\\{stripped}"));
    }

    if let Some(stripped) = value.strip_prefix("\\\\?\\") {
        return PathBuf::from(stripped);
    }

    path
}

fn display_path(path: &Path) -> String {
    normalize_windows_verbatim_path(path.to_path_buf())
        .display()
        .to_string()
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

fn local_addr(config: &GlobalConfig) -> Result<SocketAddr, String> {
    let host = match env::var(HOST_ENV) {
        Ok(value) => parse_listen_host(HOST_ENV, &value)?,
        Err(env::VarError::NotPresent) => parse_listen_host(
            "app.web_server.listen_host",
            &config.app.web_server.listen_host,
        )?,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(format!("{HOST_ENV} must be valid Unicode"));
        }
    };
    let port = match env::var(PORT_ENV) {
        Ok(value) => parse_port(&value)?,
        Err(env::VarError::NotPresent) => config.app.web_server.listen_port,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(format!("{PORT_ENV} must be valid Unicode"));
        }
    };

    Ok(SocketAddr::from((host, port)))
}

fn parse_listen_host(label: &str, value: &str) -> Result<IpAddr, String> {
    let host = value.trim();

    if host.is_empty() {
        return Err(format!("{label} must not be empty"));
    }

    host.parse::<IpAddr>()
        .map_err(|_| format!("{label} must be an IP address"))
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

#[cfg(any(test, all(windows, not(debug_assertions))))]
#[derive(Debug, PartialEq, Eq)]
struct TrayMenuLabels {
    open: &'static str,
    quit: &'static str,
}

#[cfg(any(test, all(windows, not(debug_assertions))))]
fn tray_menu_labels(language: &str) -> Result<TrayMenuLabels, String> {
    match language {
        "zh-CN" => Ok(TrayMenuLabels {
            open: "打开 Foco",
            quit: "退出 Foco",
        }),
        "en" => Ok(TrayMenuLabels {
            open: "Open Foco",
            quit: "Quit Foco",
        }),
        _ => Err(format!(
            "app language '{language}' is unsupported; expected one of {}",
            SUPPORTED_APP_LANGUAGES.join(", ")
        )),
    }
}

#[cfg(any(test, all(windows, not(debug_assertions))))]
fn browser_addr_for_listen_addr(addr: SocketAddr) -> SocketAddr {
    let host = match addr.ip() {
        IpAddr::V4(ip) if ip.octets() == [0, 0, 0, 0] => IpAddr::from([127, 0, 0, 1]),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1]),
        ip => ip,
    };

    SocketAddr::from((host, addr.port()))
}

fn web_auth_enabled(config: &GlobalConfig) -> bool {
    config.app.web_server.password_hash.is_some()
}

fn request_has_valid_auth_cookie(headers: &HeaderMap, config: &GlobalConfig) -> bool {
    let Some(password_hash) = config.app.web_server.password_hash.as_deref() else {
        return true;
    };

    headers
        .get(header::COOKIE)
        .and_then(|header| header.to_str().ok())
        .and_then(|cookie| cookie_value(cookie, AUTH_COOKIE_NAME))
        .is_some_and(|value| constant_time_eq(value.as_bytes(), password_hash.as_bytes()))
}

fn cookie_value(cookie: &str, name: &str) -> Option<String> {
    cookie.split(';').find_map(|part| {
        let (cookie_name, cookie_value) = part.trim().split_once('=')?;
        (cookie_name == name).then(|| cookie_value.to_string())
    })
}

fn auth_cookie(password_hash: &str) -> String {
    format!("{AUTH_COOKIE_NAME}={password_hash}; Path=/; HttpOnly; SameSite=Strict")
}

fn expired_auth_cookie() -> String {
    format!("{AUTH_COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0")
}

fn hash_password(password: &str) -> Result<String, ApiError> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|source| {
        ApiError::internal(format!("failed to generate password salt: {source}"))
    })?;
    let digest = password_digest(&salt, password);

    Ok(format!(
        "{PASSWORD_HASH_PREFIX}:{}:{}",
        hex_encode(&salt),
        hex_encode(&digest)
    ))
}

fn verify_password(password: &str, password_hash: &str) -> bool {
    let Some((algorithm, rest)) = password_hash.split_once(':') else {
        return false;
    };
    let Some((salt_hex, digest_hex)) = rest.split_once(':') else {
        return false;
    };

    if algorithm != PASSWORD_HASH_PREFIX {
        return false;
    }

    let Some(salt) = hex_decode(salt_hex) else {
        return false;
    };
    let Some(expected_digest) = hex_decode(digest_hex) else {
        return false;
    };

    let actual_digest = password_digest(&salt, password);
    constant_time_eq(&actual_digest, &expected_digest)
}

fn password_digest(salt: &[u8], password: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(salt);
    hasher.update(password.as_bytes());
    hasher.finalize().to_vec()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push_str(&format!("{byte:02x}"));
    }
    value
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }

    value
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let text = std::str::from_utf8(chunk).ok()?;
            u8::from_str_radix(text, 16).ok()
        })
        .collect()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let diff = left
        .iter()
        .zip(right.iter())
        .fold(0u8, |acc, (left, right)| acc | (left ^ right));

    diff == 0
}

fn verify_frontend_assets() -> Result<(), String> {
    if WebAssets::get("index.html").is_some() {
        return Ok(());
    }

    let app_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_dir = app_dir
        .parent()
        .ok_or_else(|| "app crate must live inside the Foco repository".to_string())?;
    let index_file = repo_dir.join("web").join("dist").join("index.html");

    Err(format!(
        "frontend build missing at {}. Run `npm run build -w web` before starting the backend or release build.",
        index_file.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use foco_store::config::{DEFAULT_WORKSPACE_ID, DEFAULT_WORKSPACE_NAME};

    #[test]
    fn password_hash_verifies_only_matching_password() {
        let password_hash = hash_password("secret").expect("password hash");

        assert!(password_hash.starts_with("sha256:"));
        assert!(verify_password("secret", &password_hash));
        assert!(!verify_password(" secret ", &password_hash));
        assert!(!verify_password("wrong", &password_hash));
    }

    #[test]
    fn browser_open_addr_uses_loopback_for_unspecified_listen_hosts() {
        assert_eq!(
            browser_addr_for_listen_addr(SocketAddr::from(([0, 0, 0, 0], 3210))).to_string(),
            "127.0.0.1:3210"
        );
        assert_eq!(
            browser_addr_for_listen_addr(SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], 3210)))
                .to_string(),
            "[::1]:3210"
        );
        assert_eq!(
            browser_addr_for_listen_addr(SocketAddr::from(([192, 168, 1, 10], 3210))).to_string(),
            "192.168.1.10:3210"
        );
    }

    #[test]
    fn tray_menu_labels_follow_app_language() {
        assert_eq!(
            tray_menu_labels("en").expect("English tray labels"),
            TrayMenuLabels {
                open: "Open Foco",
                quit: "Quit Foco",
            }
        );
        assert_eq!(
            tray_menu_labels("zh-CN").expect("Chinese tray labels"),
            TrayMenuLabels {
                open: "打开 Foco",
                quit: "退出 Foco",
            }
        );

        let error = tray_menu_labels("fr").expect_err("unsupported language should fail");

        assert!(error.contains("app language 'fr' is unsupported"));
    }

    #[test]
    fn normalize_web_server_settings_preserves_updates_and_clears_password_hash() {
        let current = WebServerSettings {
            listen_host: "127.0.0.1".to_string(),
            listen_port: 3210,
            password_hash: Some(hash_password("old-password").expect("old password hash")),
        };

        let preserved = normalize_web_server_settings(
            &current,
            &ManualGeneralSettingsRequest {
                clear_password: None,
                hook_audit_enabled: None,
                language: "en".to_string(),
                listen_host: "0.0.0.0".to_string(),
                listen_port: 3211,
                password: None,
            },
        )
        .expect("preserve password hash");
        assert_eq!(preserved.password_hash, current.password_hash);

        let updated = normalize_web_server_settings(
            &current,
            &ManualGeneralSettingsRequest {
                clear_password: None,
                hook_audit_enabled: None,
                language: "en".to_string(),
                listen_host: "127.0.0.1".to_string(),
                listen_port: 3210,
                password: Some("new-password".to_string()),
            },
        )
        .expect("update password hash");
        assert!(verify_password(
            "new-password",
            updated
                .password_hash
                .as_deref()
                .expect("updated password hash")
        ));

        let cleared = normalize_web_server_settings(
            &current,
            &ManualGeneralSettingsRequest {
                clear_password: Some(true),
                hook_audit_enabled: None,
                language: "en".to_string(),
                listen_host: "127.0.0.1".to_string(),
                listen_port: 3210,
                password: Some("ignored".to_string()),
            },
        )
        .expect("clear password hash");
        assert!(cleared.password_hash.is_none());
    }

    #[test]
    fn normalize_api_proxy_settings_preserves_updates_and_disables_proxy() {
        let current = ApiProxySettings {
            enabled: true,
            proxy_type: "http".to_string(),
            url: "http://127.0.0.1:7890/".to_string(),
        };

        let preserved =
            normalize_api_proxy_settings(&current, None).expect("preserve current proxy settings");
        assert_eq!(preserved, current);

        let updated = normalize_api_proxy_settings(
            &current,
            Some(&ManualApiProxySettingsRequest {
                enabled: true,
                proxy_type: "socks".to_string(),
                url: "127.0.0.1:7891".to_string(),
            }),
        )
        .expect("normalize updated proxy");
        assert!(updated.enabled);
        assert_eq!(updated.proxy_type, "socks");
        assert_eq!(updated.url, "socks5h://127.0.0.1:7891");

        let disabled = normalize_api_proxy_settings(
            &current,
            Some(&ManualApiProxySettingsRequest {
                enabled: false,
                proxy_type: "http".to_string(),
                url: "".to_string(),
            }),
        )
        .expect("disable proxy");
        assert!(!disabled.enabled);
        assert!(disabled.url.is_empty());
    }

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
    fn enabled_skill_frontmatter_messages_list_enabled_skill_frontmatter() {
        let profile_dir = env::temp_dir().join(unique_id("foco-skill-frontmatter-profile-test"));
        let workspace_dir =
            env::temp_dir().join(unique_id("foco-skill-frontmatter-workspace-test"));
        let skill_dir = profile_dir.join(".agents").join("skills").join("gitmemo");
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

        let config = GlobalConfig::first_run(workspace_dir);

        let messages =
            enabled_skill_frontmatter_messages(&profile_dir, &config, DEFAULT_WORKSPACE_ID)
                .expect("enabled skill frontmatter messages");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, NeutralChatRole::User);
        assert!(messages[0].content.contains(ENABLED_SKILLS_MESSAGE_PREFIX));
        assert!(
            messages[0]
                .content
                .contains(&skill_file.display().to_string())
        );
        assert!(messages[0].content.contains("name: gitmemo"));
        assert!(messages[0].content.contains("description: Project memory."));
        assert!(!messages[0].content.contains("Search memory"));

        fs::remove_dir_all(profile_dir).expect("remove skill test profile");
    }

    #[test]
    fn enabled_skill_frontmatter_messages_skip_disabled_skills() {
        let profile_dir = env::temp_dir().join(unique_id("foco-disabled-skill-profile-test"));
        let workspace_dir = env::temp_dir().join(unique_id("foco-disabled-skill-workspace-test"));
        let skill_dir = profile_dir.join(".agents").join("skills").join("gitmemo");
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

        let mut config = GlobalConfig::first_run(workspace_dir);
        config.skills.disabled.push("global:gitmemo".to_string());

        let messages =
            enabled_skill_frontmatter_messages(&profile_dir, &config, DEFAULT_WORKSPACE_ID)
                .expect("enabled skill frontmatter messages");

        assert!(messages.is_empty());

        fs::remove_dir_all(profile_dir).expect("remove skill test profile");
    }

    #[test]
    fn question_registry_rejects_invalid_answer_without_consuming_question() {
        let registry = QuestionRegistry::default();
        let registration = registry
            .register(QuestionRequest {
                id: "question-1".to_string(),
                tool_call_id: "tool-call-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                chat_id: "chat-1".to_string(),
                questions: vec![
                    QuestionItem {
                        id: "question-1-item-1".to_string(),
                        question: "Pick a mode.".to_string(),
                        options: vec![QuestionOption {
                            label: "Fast".to_string(),
                            value: "fast".to_string(),
                            description: None,
                        }],
                        allow_free_text: false,
                    },
                    QuestionItem {
                        id: "question-1-item-2".to_string(),
                        question: "Name the target.".to_string(),
                        options: Vec::new(),
                        allow_free_text: true,
                    },
                ],
            })
            .expect("question registration");

        let error = registry
            .answer(
                "question-1",
                QuestionAnswer {
                    answers: vec![QuestionItemAnswer {
                        id: "question-1-item-1".to_string(),
                        answer: "manual".to_string(),
                        selected_option_value: None,
                    }],
                },
            )
            .expect_err("manual answer should be rejected");

        assert!(error.message.contains("requires answers for all"));
        assert!(
            registry
                .pending
                .lock()
                .expect("question registry lock")
                .contains_key("question-1")
        );

        registry
            .answer(
                "question-1",
                QuestionAnswer {
                    answers: vec![
                        QuestionItemAnswer {
                            id: "question-1-item-1".to_string(),
                            answer: "fast".to_string(),
                            selected_option_value: Some("fast".to_string()),
                        },
                        QuestionItemAnswer {
                            id: "question-1-item-2".to_string(),
                            answer: "prod".to_string(),
                            selected_option_value: None,
                        },
                    ],
                },
            )
            .expect("valid selected option answer");

        let received_answer = registration
            .answer_rx
            .blocking_recv()
            .expect("question answer");
        assert_eq!(
            received_answer.answers[0].selected_option_value.as_deref(),
            Some("fast")
        );
        assert_eq!(received_answer.answers[1].answer, "prod");
        assert!(
            !registry
                .pending
                .lock()
                .expect("question registry lock")
                .contains_key("question-1")
        );
    }

    #[test]
    fn reorder_models_requires_complete_unique_existing_ids() {
        let mut models = vec![
            test_model_settings("low"),
            test_model_settings("high"),
            test_model_settings("medium"),
        ];

        reorder_models(
            &mut models,
            vec!["high".to_string(), "medium".to_string(), "low".to_string()],
        )
        .expect("reordered models");
        assert_eq!(model_ids(&models), vec!["high", "medium", "low"]);

        let duplicate_error = reorder_models(
            &mut models,
            vec!["high".to_string(), "high".to_string(), "low".to_string()],
        )
        .expect_err("duplicate model ids should fail");
        assert_eq!(duplicate_error.status, StatusCode::BAD_REQUEST);
        assert!(duplicate_error.message.contains("duplicate"));
        assert_eq!(model_ids(&models), vec!["high", "medium", "low"]);

        let missing_error = reorder_models(
            &mut models,
            vec!["high".to_string(), "missing".to_string(), "low".to_string()],
        )
        .expect_err("unknown model ids should fail");
        assert_eq!(missing_error.status, StatusCode::BAD_REQUEST);
        assert!(missing_error.message.contains("not found"));
        assert_eq!(model_ids(&models), vec!["high", "medium", "low"]);
    }

    #[test]
    fn reorder_workspaces_requires_complete_unique_existing_ids() {
        let mut workspaces = vec![
            test_workspace_config("default"),
            test_workspace_config("side"),
            test_workspace_config("archive"),
        ];

        reorder_workspaces(
            &mut workspaces,
            vec![
                "side".to_string(),
                "archive".to_string(),
                "default".to_string(),
            ],
        )
        .expect("reordered workspaces");
        assert_eq!(
            workspace_ids(&workspaces),
            vec!["side", "archive", "default"]
        );

        let duplicate_error = reorder_workspaces(
            &mut workspaces,
            vec![
                "side".to_string(),
                "side".to_string(),
                "default".to_string(),
            ],
        )
        .expect_err("duplicate workspace ids should fail");
        assert_eq!(duplicate_error.status, StatusCode::BAD_REQUEST);
        assert!(duplicate_error.message.contains("duplicate"));
        assert_eq!(
            workspace_ids(&workspaces),
            vec!["side", "archive", "default"]
        );

        let missing_error = reorder_workspaces(
            &mut workspaces,
            vec![
                "side".to_string(),
                "missing".to_string(),
                "default".to_string(),
            ],
        )
        .expect_err("unknown workspace ids should fail");
        assert_eq!(missing_error.status, StatusCode::BAD_REQUEST);
        assert!(missing_error.message.contains("not found"));
        assert_eq!(
            workspace_ids(&workspaces),
            vec!["side", "archive", "default"]
        );
    }

    #[test]
    fn group_pinned_workspaces_keeps_group_order() {
        let mut workspaces = vec![
            test_workspace_config("first"),
            test_workspace_config("second"),
            test_workspace_config("third"),
            test_workspace_config("fourth"),
        ];
        workspaces[2].pinned = true;
        workspaces[0].pinned = true;

        group_pinned_workspaces(&mut workspaces);

        assert_eq!(
            workspace_ids(&workspaces),
            vec!["first", "third", "second", "fourth"]
        );
    }

    #[test]
    fn normalize_windows_verbatim_path_removes_prefixes() {
        assert_eq!(
            normalize_windows_verbatim_path(PathBuf::from(r"\\?\C:\Users\fonla\Repo")),
            PathBuf::from(r"C:\Users\fonla\Repo")
        );
        assert_eq!(
            normalize_windows_verbatim_path(PathBuf::from(r"\\?\UNC\server\share\Repo")),
            PathBuf::from(r"\\server\share\Repo")
        );
    }

    #[test]
    fn skills_settings_summary_strips_windows_verbatim_directory_prefixes() {
        let user_profile_dir = PathBuf::from(r"\\?\C:\Users\fonla");
        let mut config =
            GlobalConfig::first_run(PathBuf::from(r"\\?\C:\Users\fonla\.foco\workspace"));
        config.workspaces[0].path = PathBuf::from(r"\\?\C:\Users\fonla\Projects\Foco");

        let summary = skills_settings_summary(&config, &user_profile_dir);

        assert!(
            summary
                .directories
                .iter()
                .all(|directory| !directory.starts_with(r"\\?\"))
        );
    }

    #[test]
    fn discover_skills_ignores_missing_directories() {
        let profile_dir = env::temp_dir().join(unique_id("foco-missing-skill-profile-test"));
        let workspace_dir = env::temp_dir().join(unique_id("foco-missing-skill-test"));

        fs::create_dir_all(&profile_dir).expect("profile directory");
        fs::create_dir_all(&workspace_dir).expect("workspace directory");

        let workspaces = vec![WorkspaceConfig {
            id: "default".to_string(),
            name: "Default".to_string(),
            path: workspace_dir.clone(),
            pinned: false,
            terminal_shell: DEFAULT_TERMINAL_SHELL.to_string(),
        }];
        let discovery = discover_skills(&profile_dir, &workspaces);

        assert!(discovery.errors.is_empty());
        assert!(discovery.skills.is_empty());

        remove_dir_if_exists(&profile_dir);
        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    }

    #[test]
    fn agents_prompt_messages_read_workspace_and_codex_agents_files() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-agents-test"));
        let profile_dir = env::temp_dir().join(unique_id("foco-profile-agents-test"));
        let codex_dir = profile_dir.join(".codex");

        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        fs::create_dir_all(&codex_dir).expect("codex directory");
        fs::write(workspace_dir.join("AGENTS.md"), "Workspace instructions.\n")
            .expect("workspace AGENTS write");
        fs::write(codex_dir.join("AGENTS.md"), "Codex instructions.\n")
            .expect("codex AGENTS write");

        let messages =
            agents_prompt_messages(&workspace_dir, &profile_dir).expect("agents messages");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, NeutralChatRole::User);
        assert!(messages[0].content.contains(AGENTS_MESSAGE_PREFIX));
        assert!(messages[0].content.contains("Workspace instructions."));
        assert_eq!(messages[1].role, NeutralChatRole::User);
        assert!(messages[1].content.contains("Codex instructions."));

        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
        remove_dir_if_exists(&profile_dir);
    }

    #[test]
    fn append_tool_state_messages_interleaves_each_tool_call_and_result() {
        let mut messages = Vec::new();
        let mut message_source_sequences = Vec::new();
        let tool_calls = vec![
            NeutralToolCall {
                call_id: "call-1".to_string(),
                name: "list_files".to_string(),
                arguments: json!({ "path": "." }),
                thought_signatures: None,
            },
            NeutralToolCall {
                call_id: "call-2".to_string(),
                name: "read_file".to_string(),
                arguments: json!({ "path": "README.md" }),
                thought_signatures: None,
            },
        ];
        let tool_results = vec![
            ExecutedToolCall {
                id: "call-1".to_string(),
                name: "list_files".to_string(),
                input: json!({ "path": "." }),
                output: json!({ "entries": [] }),
                is_error: false,
                started_at: "2026-06-05T07:00:00Z".to_string(),
                completed_at: "2026-06-05T07:00:01Z".to_string(),
            },
            ExecutedToolCall {
                id: "call-2".to_string(),
                name: "read_file".to_string(),
                input: json!({ "path": "README.md" }),
                output: json!({ "content": "hello" }),
                is_error: false,
                started_at: "2026-06-05T07:00:01Z".to_string(),
                completed_at: "2026-06-05T07:00:02Z".to_string(),
            },
        ];

        append_tool_state_messages(
            &mut messages,
            &mut message_source_sequences,
            tool_calls,
            &tool_results,
            "Checking files.".to_string(),
            Some("Need workspace evidence.".to_string()),
        );

        assert_eq!(
            messages
                .iter()
                .map(|message| &message.role)
                .collect::<Vec<_>>(),
            vec![
                &NeutralChatRole::Assistant,
                &NeutralChatRole::Tool,
                &NeutralChatRole::Assistant,
                &NeutralChatRole::Tool
            ]
        );
        assert_eq!(messages[0].tool_calls[0].call_id, "call-1");
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("call-1"));
        assert_eq!(messages[2].tool_calls[0].call_id, "call-2");
        assert_eq!(messages[3].tool_call_id.as_deref(), Some("call-2"));
        assert_eq!(messages[0].content, "Checking files.");
        assert_eq!(
            messages[0].reasoning.as_deref(),
            Some("Need workspace evidence.")
        );
        assert!(messages[2].content.is_empty());
        assert_eq!(message_source_sequences, vec![None, None, None, None]);
    }

    #[test]
    fn neutral_messages_from_record_replays_saved_tool_state_in_order() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-tool-state-replay-test"));
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");

        database
            .insert_chat("chat-1", "Tool chat")
            .expect("chat insert");
        database
            .insert_message(NewMessage {
                id: "assistant-1",
                chat_id: "chat-1",
                role: "assistant",
                content: "Done.",
                sequence: 0,
                metadata_json: Some(r#"{"reasoning":"Used tools."}"#),
            })
            .expect("assistant message insert");
        for (id, name, input, output, started_at) in [
            (
                "call-1",
                "list_files",
                r#"{"path":"."}"#,
                r#"{"entries":[]}"#,
                "2026-06-05T07:00:00Z",
            ),
            (
                "call-2",
                "read_file",
                r#"{"path":"README.md"}"#,
                r#"{"content":"hello"}"#,
                "2026-06-05T07:00:01Z",
            ),
        ] {
            database
                .insert_tool_call(NewToolCall {
                    id,
                    chat_id: "chat-1",
                    run_id: "run-1",
                    message_id: Some("assistant-1"),
                    tool_name: name,
                    input_json: input,
                    status: "completed",
                    started_at,
                    completed_at: Some(started_at),
                })
                .expect("tool call insert");
            let result_id = format!("{id}-result");
            database
                .insert_tool_result(NewToolResult {
                    id: &result_id,
                    tool_call_id: id,
                    output_json: output,
                    is_error: false,
                    created_at: started_at,
                })
                .expect("tool result insert");
        }

        let message = database
            .messages_for_chat("chat-1")
            .expect("messages")
            .into_iter()
            .next()
            .expect("assistant message");
        let messages =
            neutral_messages_from_record(&database, message).expect("neutral message replay");

        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0].role, NeutralChatRole::Assistant);
        assert_eq!(messages[0].tool_calls[0].call_id, "call-1");
        assert_eq!(messages[1].role, NeutralChatRole::Tool);
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("call-1"));
        assert_eq!(messages[2].role, NeutralChatRole::Assistant);
        assert_eq!(messages[2].tool_calls[0].call_id, "call-2");
        assert_eq!(messages[3].role, NeutralChatRole::Tool);
        assert_eq!(messages[3].tool_call_id.as_deref(), Some("call-2"));
        assert_eq!(messages[4].role, NeutralChatRole::Assistant);
        assert_eq!(messages[4].content, "Done.");
        assert_eq!(messages[4].reasoning.as_deref(), Some("Used tools."));

        drop(messages);
        drop(database);
        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    }

    #[test]
    fn normalized_chat_attachments_rejects_invalid_payloads() {
        let invalid_base64 = normalized_chat_attachments(vec![ChatAttachmentInput {
            id: "att-1".to_string(),
            name: "image.png".to_string(),
            content_type: "image/png".to_string(),
            content_base64: Some("not base64".to_string()),
            path: None,
            size_bytes: 1,
        }])
        .expect_err("invalid base64 should fail");
        assert!(invalid_base64.message.contains("invalid base64"));

        let size_mismatch = normalized_chat_attachments(vec![ChatAttachmentInput {
            id: "att-1".to_string(),
            name: "image.png".to_string(),
            content_type: "image/png".to_string(),
            content_base64: Some("SGVsbG8=".to_string()),
            path: None,
            size_bytes: 6,
        }])
        .expect_err("size mismatch should fail");
        assert!(size_mismatch.message.contains("sizeBytes"));

        let text_base64 = normalized_chat_attachments(vec![ChatAttachmentInput {
            id: "att-1".to_string(),
            name: "note.txt".to_string(),
            content_type: "text/plain".to_string(),
            content_base64: Some("SGVsbG8=".to_string()),
            path: None,
            size_bytes: 5,
        }])
        .expect_err("text base64 should fail");
        assert!(text_base64.message.contains("must use path"));
    }

    #[test]
    fn text_attachments_use_original_path_in_user_prompt() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-text-attachment-test"));
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        let attachment_path = workspace_dir.join("note.txt");
        let attachment_path_string = attachment_path.display().to_string();
        fs::write(&attachment_path, "Hello").expect("write attachment");

        let attachments = normalized_chat_attachments(vec![ChatAttachmentInput {
            id: "att-1".to_string(),
            name: "note.txt".to_string(),
            content_type: "text/plain".to_string(),
            content_base64: None,
            path: Some(attachment_path_string.clone()),
            size_bytes: 5,
        }])
        .expect("path attachment");

        assert_eq!(attachments[0].content_base64, None);
        assert_eq!(
            attachments[0].path.as_deref(),
            Some(attachment_path_string.as_str())
        );

        let message = neutral_user_message("Review it".to_string(), attachments.clone());
        assert!(message.content.contains("# Files mentioned by the user:"));
        assert!(message.content.contains("## note.txt:"));
        assert!(message.content.contains(&attachment_path_string));
        assert!(message.content.contains("## My request for Foco:"));
        assert!(!message.content.contains("SGVsbG8="));

        let database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let metadata_json = user_message_metadata_json(&attachments).expect("attachment metadata");
        let stored_message = MessageRecord {
            id: "user-1".to_string(),
            chat_id: "chat-1".to_string(),
            role: "user".to_string(),
            content: "Review it".to_string(),
            sequence: 0,
            created_at: "2026-06-08T10:00:00Z".to_string(),
            metadata_json,
        };
        let replayed_messages =
            neutral_messages_from_record(&database, stored_message).expect("neutral messages");
        assert!(
            replayed_messages[0]
                .content
                .contains(&attachment_path_string)
        );

        drop(database);
        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    }

    #[test]
    fn user_attachments_round_trip_into_neutral_history_and_message_parts() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-user-attachment-test"));
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        let database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let metadata_json = user_message_metadata_json(&[NeutralChatAttachment {
            id: "att-1".to_string(),
            name: "screenshot.png".to_string(),
            content_type: "image/png".to_string(),
            size_bytes: 5,
            content_base64: Some("SGVsbG8=".to_string()),
            path: None,
        }])
        .expect("attachment metadata");
        let message = MessageRecord {
            id: "user-1".to_string(),
            chat_id: "chat-1".to_string(),
            role: "user".to_string(),
            content: "See attached.".to_string(),
            sequence: 0,
            created_at: "2026-06-08T10:00:00Z".to_string(),
            metadata_json,
        };

        let neutral_messages =
            neutral_messages_from_record(&database, message.clone()).expect("neutral messages");
        assert_eq!(neutral_messages.len(), 1);
        assert_eq!(neutral_messages[0].attachments.len(), 1);
        assert_eq!(neutral_messages[0].attachments[0].name, "screenshot.png");

        let parts = chat_message_parts(&message, None, &[], &[]).expect("message parts");
        assert_eq!(parts.len(), 2);
        assert!(matches!(parts[0], ChatMessagePart::Text { .. }));
        match &parts[1] {
            ChatMessagePart::Attachment { attachment } => {
                assert_eq!(attachment.name, "screenshot.png");
                assert_eq!(
                    attachment.preview_data_url.as_deref(),
                    Some("data:image/png;base64,SGVsbG8=")
                );
            }
            other => panic!("expected attachment part, got {other:?}"),
        }

        drop(database);
        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    }

    #[test]
    fn workspace_logo_file_detects_manual_logo_png() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-logo-test"));
        let logo_dir = workspace_dir.join(".foco");
        fs::create_dir_all(&logo_dir).expect("logo directory");
        fs::write(logo_dir.join("logo.png"), b"\x89PNG\r\n\x1A\nlogo").expect("write manual logo");

        let logo = workspace_logo_file(&workspace_dir)
            .expect("logo lookup")
            .expect("manual logo");
        assert_eq!(logo.kind.content_type, "image/png");
        assert_eq!(
            logo.path.file_name().and_then(|name| name.to_str()),
            Some("logo.png")
        );

        let mut workspace = test_workspace_config("workspace-1");
        workspace.path = workspace_dir.clone();
        let logo_url = workspace_logo_url(&workspace)
            .expect("logo url lookup")
            .expect("logo url");
        assert!(logo_url.starts_with("/api/workspaces/workspace-1/logo?v="));

        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    }

    #[test]
    fn save_workspace_logo_file_uses_detected_extension_and_removes_old_logo() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-logo-save-test"));
        let logo_dir = workspace_dir.join(".foco");
        fs::create_dir_all(&logo_dir).expect("logo directory");
        fs::write(logo_dir.join("logo.png"), b"\x89PNG\r\n\x1A\nold").expect("write old logo");
        let jpeg = &[0xFF, 0xD8, 0xFF, 0xE0, b'l', b'o', b'g', b'o'];
        let kind = workspace_logo_kind(jpeg).expect("jpeg kind");

        save_workspace_logo_file(&workspace_dir, jpeg, kind).expect("save logo");

        assert!(!logo_dir.join("logo.png").exists());
        assert!(logo_dir.join("logo.jpg").exists());
        let logo = workspace_logo_file(&workspace_dir)
            .expect("logo lookup")
            .expect("saved logo");
        assert_eq!(logo.kind.content_type, "image/jpeg");
        assert_eq!(
            logo.path.file_name().and_then(|name| name.to_str()),
            Some("logo.jpg")
        );

        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    }

    #[test]
    fn clear_workspace_logo_file_removes_logo_candidates() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-logo-clear-test"));
        let logo_dir = workspace_dir.join(".foco");
        fs::create_dir_all(&logo_dir).expect("logo directory");
        fs::write(logo_dir.join("logo.png"), b"\x89PNG\r\n\x1A\nold").expect("write png logo");
        fs::write(
            logo_dir.join("logo.jpg"),
            [0xFF, 0xD8, 0xFF, 0xE0, b'l', b'o', b'g', b'o'],
        )
        .expect("write jpeg logo");

        clear_workspace_logo_file(&workspace_dir).expect("clear logo");

        assert!(!logo_dir.join("logo.png").exists());
        assert!(!logo_dir.join("logo.jpg").exists());
        assert!(
            workspace_logo_file(&workspace_dir)
                .expect("logo lookup")
                .is_none()
        );

        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    }

    #[test]
    fn workspace_logo_file_rejects_extension_type_mismatch() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-logo-mismatch-test"));
        let logo_dir = workspace_dir.join(".foco");
        fs::create_dir_all(&logo_dir).expect("logo directory");
        fs::write(
            logo_dir.join("logo.png"),
            [0xFF, 0xD8, 0xFF, 0xE0, b'l', b'o', b'g', b'o'],
        )
        .expect("write mismatched logo");

        let error = workspace_logo_file(&workspace_dir).expect_err("mismatch should fail");
        assert!(error.message.contains("does not match"));

        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    }

    #[test]
    fn persist_chat_result_writes_audit_status_code() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-audit-status-code-test"));
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        {
            let mut database =
                WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
            database
                .insert_chat("chat-1", "Status code chat")
                .expect("chat insert");
        }
        let (_app_shutdown_tx, app_shutdown_rx) = watch::channel(false);
        let mcp_registry = Arc::new(McpRegistry::default());
        let context = PreparedChatContext {
            workspace_id: "workspace-1".to_string(),
            workspace_path: workspace_dir.clone(),
            chat_id: "chat-1".to_string(),
            provider_id: "openai-responses".to_string(),
            model_id: "gpt-5.4".to_string(),
            user_message_id: "user-1".to_string(),
            assistant_message_id: "assistant-1".to_string(),
            llm_request_id: "request-1".to_string(),
            assistant_sequence: 1,
            provider_config: ProviderConnectionConfig {
                kind: foco_providers::ProviderKind::OpenAiResponses,
                base_url: None,
                api_key: Some("test-key".to_string()),
                proxy_url: None,
            },
            provider_request: NeutralChatRequest {
                model_id: "gpt-5.4".to_string(),
                messages: vec![neutral_text_message(
                    NeutralChatRole::User,
                    "Hello".to_string(),
                )],
                tools: Vec::new(),
                thinking_level: None,
                max_output_tokens: Some(16),
            },
            hook_runtime: HookRuntime::new(mcp_registry.clone()),
            global_hooks: HookConfig::default(),
            mcp_registry,
            question_registry: QuestionRegistry::default(),
            app_shutdown_rx,
            context_budget: foco_agent::ContextBudget {
                context_window: 1_000,
                max_output_tokens: 16,
                system_prompt_tokens: 0,
                tool_schema_tokens: 0,
                safety_tokens: 0,
                available_message_tokens: 984,
            },
            request_body_json: "{}".to_string(),
            compression_snapshots: Vec::new(),
            message_source_sequences: vec![Some(0)],
            active_tool_start_index: 1,
            hook_context_messages: Vec::new(),
            hook_notifications: Vec::new(),
        };
        let outcome = ChatAuditOutcome {
            first_token_at: Some("2026-06-06T09:00:00Z".to_string()),
            completed_at: "2026-06-06T09:00:01Z".to_string(),
            first_token_latency_ms: Some(100),
            total_latency_ms: 1_000,
            input_tokens: Some(10),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            status_code: Some(200),
            final_state: "succeeded",
            response_body_json: Some(r#"{"text":"Done."}"#.to_string()),
        };
        let event = captured_event(&ChatSseEvent::Complete {
            chat_id: "chat-1".to_string(),
            assistant_message_id: "assistant-1".to_string(),
            text: "Done.".to_string(),
            reasoning: None,
            usage: None,
            stop_reason: Some("stop".to_string()),
            metrics: ChatReplyMetrics {
                model_id: "gpt-5.4".to_string(),
                provider_id: "openai-responses".to_string(),
                total_latency_ms: Some(1_000),
                first_token_latency_ms: Some(100),
                output_tokens: Some(5),
            },
        });

        persist_chat_result(
            &context,
            "2026-06-06T09:00:00Z",
            outcome,
            &[event],
            Some("Done."),
            None,
            &[],
        )
        .expect("persist chat result");

        let database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let request = database
            .llm_request("request-1")
            .expect("llm request read")
            .expect("llm request");

        assert_eq!(request.status_code, Some(200));

        drop(database);
        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    }

    #[test]
    fn chat_message_summary_includes_assistant_reply_metrics() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-message-metrics-test"));
        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        let mut database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");

        database
            .insert_chat("chat-1", "Metrics chat")
            .expect("chat insert");
        database
            .insert_message(NewMessage {
                id: "assistant-1",
                chat_id: "chat-1",
                role: "assistant",
                content: "Done.",
                sequence: 0,
                metadata_json: Some("{}"),
            })
            .expect("assistant message insert");
        database
            .insert_llm_request(NewLlmRequest {
                id: "request-1",
                workspace_id: "workspace-1",
                chat_id: Some("chat-1"),
                provider_id: "openai-responses",
                model_id: "gpt-5.4",
                request_started_at: "2026-06-06T09:00:00Z",
                first_token_at: Some("2026-06-06T09:00:00Z"),
                completed_at: Some("2026-06-06T09:00:02Z"),
                input_tokens: Some(100),
                output_tokens: Some(40),
                cache_read_tokens: Some(0),
                cache_write_tokens: Some(0),
                first_token_latency_ms: Some(250),
                total_latency_ms: Some(2000),
                status_code: None,
                final_state: "succeeded",
                request_body_json: Some("{}"),
                response_body_json: Some("{}"),
            })
            .expect("llm request insert");
        database
            .insert_llm_request_event(NewLlmRequestEvent {
                id: "request-1-event-0",
                llm_request_id: "request-1",
                sequence: 0,
                event_at: "2026-06-06T09:00:00Z",
                event_type: "start",
                raw_chunk_json: None,
                normalized_event_json: r#"{"type":"start","chatId":"chat-1","userMessageId":"user-1","assistantMessageId":"assistant-1","llmRequestId":"request-1"}"#,
            })
            .expect("llm start event insert");

        let message = database
            .messages_for_chat("chat-1")
            .expect("messages")
            .into_iter()
            .next()
            .expect("assistant message");
        let events = database
            .llm_request_events_for_chat("chat-1")
            .expect("llm request events");
        let summary = chat_message_summary(&database, message, &events).expect("message summary");
        let metrics = summary.metrics.expect("assistant metrics");

        assert_eq!(metrics.model_id, "gpt-5.4");
        assert_eq!(metrics.provider_id, "openai-responses");
        assert_eq!(metrics.total_latency_ms, Some(2000));
        assert_eq!(metrics.first_token_latency_ms, Some(250));
        assert_eq!(metrics.output_tokens, Some(40));

        drop(database);
        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
    }

    #[test]
    fn pack_neutral_messages_keeps_saved_tool_state_group_together() {
        let messages = vec![
            neutral_text_message(NeutralChatRole::System, "system".to_string()),
            neutral_text_message(NeutralChatRole::User, "old question".to_string()),
            neutral_assistant_tool_call_message(
                NeutralToolCall {
                    call_id: "call-1".to_string(),
                    name: "read_file".to_string(),
                    arguments: json!({ "path": "README.md" }),
                    thought_signatures: None,
                },
                String::new(),
                None,
            ),
            NeutralChatMessage {
                role: NeutralChatRole::Tool,
                content: r#"{"content":"hello"}"#.to_string(),
                attachments: Vec::new(),
                reasoning: None,
                tool_calls: Vec::new(),
                tool_call_id: Some("call-1".to_string()),
                tool_name: Some("read_file".to_string()),
            },
            neutral_text_message(NeutralChatRole::User, "latest".to_string()),
        ];
        let message_source_sequences = vec![None, Some(0), Some(1), Some(1), Some(2)];
        let groups = context_message_groups(&messages, &message_source_sequences, messages.len())
            .expect("message groups");
        let required_tokens = groups
            .iter()
            .filter(|group| group.must_keep)
            .map(|group| group.estimated_tokens)
            .sum::<u64>();
        let tool_group_tokens = groups
            .iter()
            .find(|group| group.message_indices == vec![2, 3])
            .expect("tool state group")
            .estimated_tokens;
        let tight_budget = foco_agent::ContextBudget {
            context_window: 1_000,
            max_output_tokens: 1,
            system_prompt_tokens: 0,
            tool_schema_tokens: 0,
            safety_tokens: 0,
            available_message_tokens: required_tokens + tool_group_tokens - 1,
        };

        let packed = pack_neutral_messages(
            messages.clone(),
            &message_source_sequences,
            &tight_budget,
            messages.len(),
        )
        .expect("packed messages");

        assert!(
            packed
                .iter()
                .all(|message| message.role != NeutralChatRole::Tool)
        );

        let full_budget = foco_agent::ContextBudget {
            available_message_tokens: required_tokens + tool_group_tokens,
            ..tight_budget
        };
        let packed = pack_neutral_messages(
            messages,
            &message_source_sequences,
            &full_budget,
            message_source_sequences.len(),
        )
        .expect("packed messages");
        let tool_position = packed
            .iter()
            .position(|message| message.role == NeutralChatRole::Tool)
            .expect("tool message should be kept");

        assert_eq!(packed[tool_position - 1].role, NeutralChatRole::Assistant);
        assert_eq!(packed[tool_position - 1].tool_calls[0].call_id, "call-1");
        assert_eq!(
            packed[tool_position].tool_call_id.as_deref(),
            Some("call-1")
        );
    }

    #[tokio::test]
    async fn add_workspace_creates_missing_directory_and_registers_it() {
        let existing_workspace_dir =
            env::temp_dir().join(unique_id("foco-existing-workspace-test"));
        let profile_dir = env::temp_dir().join(unique_id("foco-add-workspace-profile-test"));
        let new_workspace_dir = env::temp_dir().join(unique_id("foco-new-workspace-test"));

        fs::create_dir_all(&existing_workspace_dir).expect("existing workspace directory");
        fs::create_dir_all(profile_dir.join(".foco")).expect("profile config directory");

        let config = GlobalConfig::first_run(existing_workspace_dir.clone());
        let state = test_app_state(config, profile_dir.clone());

        let _response = add_workspace(
            State(state.clone()),
            Json(WorkspacePathRequest {
                name: "New Workspace".to_string(),
                path: new_workspace_dir.display().to_string(),
            }),
        )
        .await
        .expect("add workspace");

        assert!(new_workspace_dir.is_dir());
        assert!(
            WorkspaceDatabase::open_or_create(&new_workspace_dir)
                .expect("workspace database")
                .database_path()
                .is_file()
        );

        let registered_path = normalize_windows_verbatim_path(
            fs::canonicalize(&new_workspace_dir).expect("new workspace canonical path"),
        );
        let config = state.config.lock().expect("config lock");
        assert!(config.workspaces.iter().any(
            |workspace| workspace.name == "New Workspace" && workspace.path == registered_path
        ));
        drop(config);

        fs::remove_dir_all(existing_workspace_dir).expect("remove existing workspace directory");
        fs::remove_dir_all(new_workspace_dir).expect("remove new workspace directory");
        fs::remove_dir_all(profile_dir).expect("remove profile directory");
    }

    #[tokio::test]
    async fn prepare_chat_context_injects_initial_context_only_for_new_chat() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-chat-agents-workspace-test"));
        let profile_dir = env::temp_dir().join(unique_id("foco-chat-agents-profile-test"));
        let codex_dir = profile_dir.join(".codex");
        let skill_dir = profile_dir.join(".agents").join("skills").join("gitmemo");

        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        fs::create_dir_all(&codex_dir).expect("codex directory");
        fs::create_dir_all(&skill_dir).expect("skill directory");
        fs::write(
            workspace_dir.join("AGENTS.md"),
            "Workspace chat instructions.\n",
        )
        .expect("workspace AGENTS write");
        fs::write(codex_dir.join("AGENTS.md"), "Codex chat instructions.\n")
            .expect("codex AGENTS write");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---
name: gitmemo
description: Project memory.
---

# GitMemo

Search memory before repo work.
",
        )
        .expect("skill file write");

        let mut config = GlobalConfig::first_run(workspace_dir.clone());
        config.providers.push(ProviderSettings {
            id: "provider".to_string(),
            name: "Provider".to_string(),
            kind: OPENAI_CHAT_KIND.to_string(),
            enabled: true,
            base_url: None,
            api_key: None,
            api_proxy: ApiProxySettings::default(),
        });
        config.models.push(ModelSettings {
            id: "model".to_string(),
            display_name: "Model".to_string(),
            enabled: true,
            provider_ids: vec!["provider".to_string()],
            active_provider_id: Some("provider".to_string()),
            thinking_level: None,
            metadata_key: None,
            metadata_source_url: None,
            metadata_refreshed_at: None,
            limits: Some(ModelLimits {
                context_window: 100_000,
                max_output_tokens: 1_024,
            }),
        });
        let state = test_app_state(config.clone(), profile_dir.clone());

        let new_context = prepare_chat_context(
            &state,
            &config,
            &config.workspaces[0].id,
            ChatStreamRequest {
                chat_id: None,
                model_id: "model".to_string(),
                thinking_level: None,
                skill_ids: None,
                message: "Hello".to_string(),
                attachments: Vec::new(),
            },
        )
        .await
        .expect("new chat context");
        let injected_messages = new_context
            .provider_request
            .messages
            .iter()
            .filter(|message| message.content.contains(AGENTS_MESSAGE_PREFIX))
            .collect::<Vec<_>>();

        assert_eq!(injected_messages.len(), 2);
        assert!(
            injected_messages[0]
                .content
                .contains("Workspace chat instructions.")
        );
        assert!(
            injected_messages[1]
                .content
                .contains("Codex chat instructions.")
        );
        let skill_messages = new_context
            .provider_request
            .messages
            .iter()
            .filter(|message| message.content.contains(ENABLED_SKILLS_MESSAGE_PREFIX))
            .collect::<Vec<_>>();

        assert_eq!(skill_messages.len(), 1);
        assert!(skill_messages[0].content.contains("name: gitmemo"));
        assert!(
            skill_messages[0]
                .content
                .contains("description: Project memory.")
        );
        assert!(!skill_messages[0].content.contains("Search memory"));
        let environment_messages = new_context
            .provider_request
            .messages
            .iter()
            .filter(|message| message.content.contains(ENVIRONMENT_CONTEXT_MESSAGE_PREFIX))
            .collect::<Vec<_>>();

        assert_eq!(environment_messages.len(), 1);
        assert_eq!(environment_messages[0].role, NeutralChatRole::User);
        assert!(environment_messages[0].content.contains(&format!(
            "- workspace directory: {}",
            workspace_dir.display()
        )));
        assert!(environment_messages[0].content.contains("- shell type: "));
        assert!(
            environment_messages[0]
                .content
                .contains("- shell executable: ")
        );
        assert!(environment_messages[0].content.contains("- current date: "));
        assert!(environment_messages[0].content.contains("- time zone: "));
        assert!(environment_messages[0].content.contains("- wsl: "));

        {
            let database =
                WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
            let stored_messages = database
                .messages_for_chat(&new_context.chat_id)
                .expect("stored messages");
            assert_eq!(stored_messages.len(), 1);
            assert_eq!(stored_messages[0].content, "Hello");
        }

        let existing_context = prepare_chat_context(
            &state,
            &config,
            &config.workspaces[0].id,
            ChatStreamRequest {
                chat_id: Some(new_context.chat_id.clone()),
                model_id: "model".to_string(),
                thinking_level: None,
                skill_ids: None,
                message: "Next".to_string(),
                attachments: Vec::new(),
            },
        )
        .await
        .expect("existing chat context");

        assert!(
            existing_context
                .provider_request
                .messages
                .iter()
                .all(|message| !message.content.contains(AGENTS_MESSAGE_PREFIX))
        );
        assert!(
            existing_context
                .provider_request
                .messages
                .iter()
                .all(|message| !message.content.contains(ENABLED_SKILLS_MESSAGE_PREFIX))
        );
        assert!(
            existing_context
                .provider_request
                .messages
                .iter()
                .all(|message| !message.content.contains(ENVIRONMENT_CONTEXT_MESSAGE_PREFIX))
        );

        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
        remove_dir_if_exists(&profile_dir);
    }

    #[tokio::test]
    async fn prepare_chat_context_prefixes_selected_skills_in_user_message() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-selected-skill-workspace-test"));
        let profile_dir = env::temp_dir().join(unique_id("foco-selected-skill-profile-test"));
        let skill_dir = workspace_dir
            .join(".agents")
            .join("skills")
            .join("web-design-guidelines");
        let skill_file = skill_dir.join("SKILL.md");

        fs::create_dir_all(&workspace_dir).expect("workspace directory");
        fs::create_dir_all(&skill_dir).expect("skill directory");
        fs::write(
            &skill_file,
            "---
name: web-design-guidelines
description: UI design guidance.
---

# Web Design Guidelines

Use the existing product UI conventions.
",
        )
        .expect("skill file write");

        let mut config = GlobalConfig::first_run(workspace_dir.clone());
        config.providers.push(ProviderSettings {
            id: "provider".to_string(),
            name: "Provider".to_string(),
            kind: OPENAI_CHAT_KIND.to_string(),
            enabled: true,
            base_url: None,
            api_key: None,
            api_proxy: ApiProxySettings::default(),
        });
        config.models.push(ModelSettings {
            id: "model".to_string(),
            display_name: "Model".to_string(),
            enabled: true,
            provider_ids: vec!["provider".to_string()],
            active_provider_id: Some("provider".to_string()),
            thinking_level: None,
            metadata_key: None,
            metadata_source_url: None,
            metadata_refreshed_at: None,
            limits: Some(ModelLimits {
                context_window: 100_000,
                max_output_tokens: 1_024,
            }),
        });
        let state = test_app_state(config.clone(), profile_dir.clone());
        let expected_message = format!(
            "[$web-design-guidelines]({}) Settings single-column layout.",
            skill_file.display()
        );

        let context = prepare_chat_context(
            &state,
            &config,
            &config.workspaces[0].id,
            ChatStreamRequest {
                chat_id: None,
                model_id: "model".to_string(),
                thinking_level: None,
                skill_ids: Some(vec!["workspace:default:web-design-guidelines".to_string()]),
                message: "Settings single-column layout.".to_string(),
                attachments: Vec::new(),
            },
        )
        .await
        .expect("selected skill chat context");

        let latest_user_message = context
            .provider_request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == NeutralChatRole::User)
            .expect("latest user message");
        assert_eq!(latest_user_message.content, expected_message);

        {
            let database =
                WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
            let stored_messages = database
                .messages_for_chat(&context.chat_id)
                .expect("stored messages");
            assert_eq!(stored_messages.len(), 1);
            assert_eq!(stored_messages[0].content, expected_message);
        }

        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
        remove_dir_if_exists(&profile_dir);
    }

    #[tokio::test]
    async fn context_usage_preview_does_not_persist_chat_messages() {
        let workspace_dir = env::temp_dir().join(unique_id("foco-context-usage-workspace-test"));
        let profile_dir = env::temp_dir().join(unique_id("foco-context-usage-profile-test"));

        fs::create_dir_all(&workspace_dir).expect("workspace directory");

        let mut config = GlobalConfig::first_run(workspace_dir.clone());
        config.providers.push(ProviderSettings {
            id: "provider".to_string(),
            name: "Provider".to_string(),
            kind: OPENAI_CHAT_KIND.to_string(),
            enabled: true,
            base_url: None,
            api_key: None,
            api_proxy: ApiProxySettings::default(),
        });
        config.models.push(ModelSettings {
            id: "model".to_string(),
            display_name: "Model".to_string(),
            enabled: true,
            provider_ids: vec!["provider".to_string()],
            active_provider_id: Some("provider".to_string()),
            thinking_level: None,
            metadata_key: None,
            metadata_source_url: None,
            metadata_refreshed_at: None,
            limits: Some(ModelLimits {
                context_window: 10_000,
                max_output_tokens: 1_000,
            }),
        });
        let state = test_app_state(config.clone(), profile_dir.clone());

        let prompt_context = prepare_prompt_context(
            &state,
            &config,
            &config.workspaces[0].id,
            PromptContextRequest {
                chat_id: None,
                model_id: "model".to_string(),
                thinking_level: None,
                skill_ids: None,
                message: Some("Preview usage".to_string()),
                attachments: Vec::new(),
            },
        )
        .await
        .expect("prompt context");
        let usage = context_usage_response(&prompt_context).expect("context usage");

        assert!(usage.used_message_tokens > 0);
        assert_eq!(
            usage.compression_trigger_tokens,
            context_compression_trigger_tokens(usage.available_message_tokens)
        );
        assert_eq!(usage.compression_trigger_percent, 80);

        let database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        assert!(database.chats().expect("chat list").is_empty());

        drop(database);
        fs::remove_dir_all(workspace_dir).expect("remove workspace directory");
        remove_dir_if_exists(&profile_dir);
    }

    #[test]
    fn discover_skills_reports_non_directory_paths() {
        let profile_dir = env::temp_dir().join(unique_id("foco-skill-file-path-test"));
        let skill_path = profile_dir.join(".agents").join("skills");

        fs::create_dir_all(profile_dir.join(".agents")).expect("profile skill parent");
        fs::write(&skill_path, "not a directory").expect("skill path file write");

        let discovery = discover_skills(&profile_dir, &[]);

        assert_eq!(discovery.errors.len(), 1);
        assert!(discovery.errors[0].message.contains("not a directory"));
        assert!(discovery.skills.is_empty());

        fs::remove_dir_all(profile_dir).expect("remove profile directory");
    }

    #[test]
    fn manual_skill_save_uses_explicit_disabled_skill_keys() {
        let discovered = vec![
            test_skill_settings("global:gitmemo", "gitmemo"),
            test_skill_settings("workspace:default:gitmemo", "gitmemo"),
        ];
        let disabled = normalize_manual_disabled_skill_ids(
            Some(vec!["workspace:default:gitmemo".to_string()]),
            None,
            &discovered,
        )
        .expect("disabled skill ids");

        assert_eq!(disabled, vec!["workspace:default:gitmemo"]);
    }

    #[test]
    fn manual_skill_save_derives_disabled_keys_from_enabled_keys() {
        let discovered = vec![
            test_skill_settings("global:gitmemo", "gitmemo"),
            test_skill_settings("workspace:default:gitmemo", "gitmemo"),
        ];
        let disabled = normalize_manual_disabled_skill_ids(
            None,
            Some(vec!["global:gitmemo".to_string()]),
            &discovered,
        )
        .expect("disabled skill ids");

        assert_eq!(disabled, vec!["workspace:default:gitmemo"]);
    }

    #[test]
    fn discover_skills_reads_workspace_skill_directories() {
        let profile_dir = env::temp_dir().join(unique_id("foco-workspace-skill-profile-test"));
        let workspace_dir = env::temp_dir().join(unique_id("foco-workspace-skill-test"));
        let skill_dir = workspace_dir.join(".agents").join("skills").join("gitmemo");
        let skill_file = skill_dir.join("SKILL.md");

        fs::create_dir_all(&profile_dir).expect("profile directory");
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
            pinned: false,
            terminal_shell: DEFAULT_TERMINAL_SHELL.to_string(),
        }];
        let discovery = discover_skills(&profile_dir, &workspaces);

        assert!(discovery.errors.is_empty());
        assert_eq!(discovery.skills.len(), 1);
        assert_eq!(discovery.skills[0].key, "workspace:default:gitmemo");
        assert_eq!(discovery.skills[0].id, "gitmemo");
        assert_eq!(discovery.skills[0].scope, SKILL_SCOPE_WORKSPACE);
        assert_eq!(discovery.skills[0].workspace_id.as_deref(), Some("default"));
        assert_eq!(
            discovery.skills[0].workspace_name.as_deref(),
            Some("Default")
        );
        assert_eq!(discovery.skills[0].path, skill_file);

        fs::remove_dir_all(profile_dir).expect("remove profile directory");
        fs::remove_dir_all(workspace_dir).expect("remove skill test directory");
    }

    fn test_skill_settings(key: &str, id: &str) -> SkillSettings {
        SkillSettings {
            key: key.to_string(),
            id: id.to_string(),
            name: id.to_string(),
            description: "Test skill.".to_string(),
            path: env::temp_dir().join(id).join("SKILL.md"),
            scope: if key.starts_with("workspace:") {
                SKILL_SCOPE_WORKSPACE.to_string()
            } else {
                SKILL_SCOPE_GLOBAL.to_string()
            },
            workspace_id: key
                .starts_with("workspace:")
                .then(|| DEFAULT_WORKSPACE_ID.to_string()),
            workspace_name: key
                .starts_with("workspace:")
                .then(|| DEFAULT_WORKSPACE_NAME.to_string()),
        }
    }

    fn test_model_settings(id: &str) -> ModelSettings {
        ModelSettings {
            id: id.to_string(),
            display_name: id.to_string(),
            enabled: false,
            provider_ids: Vec::new(),
            active_provider_id: None,
            thinking_level: None,
            metadata_key: None,
            metadata_source_url: None,
            metadata_refreshed_at: None,
            limits: None,
        }
    }

    fn model_ids(models: &[ModelSettings]) -> Vec<&str> {
        models.iter().map(|model| model.id.as_str()).collect()
    }

    fn test_workspace_config(id: &str) -> WorkspaceConfig {
        WorkspaceConfig {
            id: id.to_string(),
            name: id.to_string(),
            path: env::temp_dir().join(id),
            pinned: false,
            terminal_shell: DEFAULT_TERMINAL_SHELL.to_string(),
        }
    }

    fn workspace_ids(workspaces: &[WorkspaceConfig]) -> Vec<&str> {
        workspaces
            .iter()
            .map(|workspace| workspace.id.as_str())
            .collect()
    }

    fn remove_dir_if_exists(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("remove test directory");
        }
    }

    fn test_app_state(config: GlobalConfig, user_profile_dir: PathBuf) -> AppState {
        let (terminal_shutdown_tx, _) = broadcast::channel(1);
        let (_app_shutdown_tx, app_shutdown_rx) = watch::channel(false);
        let mcp_registry = Arc::new(McpRegistry::default());

        AppState {
            config: Arc::new(Mutex::new(config)),
            config_file: user_profile_dir.join(".foco").join("config.json"),
            model_metadata_file: user_profile_dir.join(".foco").join("models.dev.json"),
            user_profile_dir,
            terminal_registry: terminal::TerminalRegistry::default(),
            terminal_shutdown_tx,
            app_shutdown_rx,
            hook_runtime: HookRuntime::new(mcp_registry.clone()),
            mcp_registry,
            question_registry: QuestionRegistry::default(),
            _code_graph_watchers: Arc::new(Vec::new()),
        }
    }
}
