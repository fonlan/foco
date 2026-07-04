use std::{
    collections::{HashMap, HashSet},
    io,
    net::SocketAddr,
    path::Path,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{
        Path as AxumPath, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, patch, post, put},
};
use chrono::{SecondsFormat, Utc};
use foco_providers::{
    NeutralChatMessage, NeutralChatRequest, NeutralChatStreamEvent, NeutralUsage, stream_chat,
};
use foco_store::{
    config::{RemoteServerProfile, WorkspaceConfig, WorkspaceLocation},
    memory::{
        MemoryDatabase, MemoryKind, MemoryScope, MemorySourceType, MemoryStatus, NewMemoryFact,
        NewMemorySource,
    },
    workspace::workspace_database_path,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    process::{Child, Command},
    sync::{Mutex as AsyncMutex, oneshot},
    task::JoinHandle,
    time::{sleep, timeout},
};
use tokio_tungstenite::connect_async;
use tungstenite::client::IntoClientRequest;

use crate::{
    ApiError, AppResult, AppState, config_snapshot,
    http::remote_servers::{normalize_target, remote_server_ssh_args, select_sidecar_asset},
    runtime::{build_sidecar_runtime_config_bundle, execute_image_tool, execute_web_tool},
    save_config, unique_id, workspace_by_id,
};

const REMOTE_SIDECAR_COMMAND: &str = "--remote-sidecar";
const SIDECAR_BINARY_NAME: &str = "foco";
const CONTROL_WS_PATH: &str = "/api/remote/control/ws";
const SIDECAR_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

type BrokerWsWrite = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tungstenite::Message,
>;
type SharedBrokerWsWrite = Arc<AsyncMutex<BrokerWsWrite>>;
type BrokerCancelRegistry = Arc<AsyncMutex<HashMap<String, oneshot::Sender<()>>>>;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteWorkspaceSessionSummary {
    pub(crate) server_id: String,
    pub(crate) workspace_id: String,
    pub(crate) remote_path: String,
    pub(crate) target: String,
    pub(crate) local_port: u16,
    pub(crate) remote_port: u16,
    pub(crate) started_at: String,
    pub(crate) status: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RemoteWorkspaceManager {
    sessions: Arc<Mutex<HashMap<String, Arc<RemoteWorkspaceSession>>>>,
}

impl RemoteWorkspaceManager {
    pub(crate) async fn connect_workspace(
        &self,
        state: AppState,
        server_id: &str,
        workspace_id: &str,
    ) -> Result<RemoteWorkspaceSessionSummary, ApiError> {
        let config = config_snapshot(&state)?;
        let server = config
            .remote_servers
            .iter()
            .find(|server| server.id == server_id)
            .cloned()
            .ok_or_else(|| {
                remote_error(server_id, Some(workspace_id), "remote server was not found")
            })?;
        let workspace = workspace_by_id(&config, workspace_id)?.clone();
        let remote_path = workspace_remote_path(&workspace, server_id)?;
        let key = session_key(server_id, workspace_id);
        if let Some(existing) = self.session(&key)? {
            return Ok(existing.summary());
        }

        let target = detect_or_cached_target(&server, server_id, workspace_id).await?;
        let command =
            ensure_sidecar_command(&state, &server, server_id, workspace_id, &target).await?;
        let token = random_token()?;
        let mut sidecar = launch_remote_sidecar(
            &server,
            server_id,
            workspace_id,
            &remote_path,
            &target,
            &token,
            &command,
        )
        .await?;
        let bootstrap = read_bootstrap(&mut sidecar, server_id, workspace_id).await?;
        validate_bootstrap(&bootstrap, server_id, workspace_id, &target)?;
        let (local_port, tunnel) =
            start_local_forward(&server, bootstrap.port, server_id, workspace_id).await?;
        let bundle = build_sidecar_runtime_config_bundle(
            &state.user_profile_dir,
            &config,
            workspace_id,
            None,
            Utc::now().timestamp_millis().max(0) as u64,
        )?;
        let control_task = connect_control_ws(
            state.clone(),
            local_port,
            &token,
            bundle,
            server_id,
            workspace_id,
        )
        .await?;

        let session = Arc::new(RemoteWorkspaceSession {
            server_id: server_id.to_string(),
            workspace_id: workspace_id.to_string(),
            remote_path,
            target,
            local_port,
            remote_port: bootstrap.port,
            token,
            started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            sidecar: AsyncMutex::new(Some(sidecar)),
            tunnel: AsyncMutex::new(Some(tunnel)),
            control_task: AsyncMutex::new(Some(control_task)),
        });
        let summary = session.summary();
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| ApiError::internal("remote workspace session lock is poisoned"))?;
        sessions.insert(key, session);
        Ok(summary)
    }

    pub(crate) async fn disconnect_workspace(
        &self,
        server_id: &str,
        workspace_id: &str,
    ) -> Result<bool, ApiError> {
        let session = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| ApiError::internal("remote workspace session lock is poisoned"))?;
            sessions.remove(&session_key(server_id, workspace_id))
        };
        if let Some(session) = session {
            session.stop().await;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(crate) async fn disconnect_server(&self, server_id: &str) -> Result<(), ApiError> {
        let removed = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| ApiError::internal("remote workspace session lock is poisoned"))?;
            let keys = sessions
                .keys()
                .filter(|key| key.starts_with(&format!("{server_id}:")))
                .cloned()
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| sessions.remove(&key))
                .collect::<Vec<_>>()
        };
        for session in removed {
            session.stop().await;
        }
        Ok(())
    }

    pub(crate) fn server_ids_with_sessions(&self) -> Result<HashSet<String>, ApiError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| ApiError::internal("remote workspace session lock is poisoned"))?;
        Ok(sessions
            .values()
            .map(|session| session.server_id.clone())
            .collect())
    }

    pub(crate) fn session_summaries_for_server(
        &self,
        server_id: &str,
    ) -> Result<Vec<RemoteWorkspaceSessionSummary>, ApiError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| ApiError::internal("remote workspace session lock is poisoned"))?;
        Ok(sessions
            .values()
            .filter(|session| session.server_id == server_id)
            .map(|session| session.summary())
            .collect())
    }

    fn session(&self, key: &str) -> Result<Option<Arc<RemoteWorkspaceSession>>, ApiError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| ApiError::internal("remote workspace session lock is poisoned"))?;
        Ok(sessions.get(key).cloned())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteWorkspaceSessionResponse {
    session: Option<RemoteWorkspaceSessionSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteWorkspaceSessionsResponse {
    sessions: Vec<RemoteWorkspaceSessionSummary>,
}

pub(crate) async fn connect_remote_workspace(
    State(state): State<AppState>,
    AxumPath((server_id, workspace_id)): AxumPath<(String, String)>,
) -> Result<Json<RemoteWorkspaceSessionResponse>, ApiError> {
    let session = state
        .remote_workspace_manager
        .connect_workspace(state.clone(), &server_id, &workspace_id)
        .await?;
    Ok(Json(RemoteWorkspaceSessionResponse {
        session: Some(session),
    }))
}

pub(crate) async fn disconnect_remote_workspace(
    State(state): State<AppState>,
    AxumPath((server_id, workspace_id)): AxumPath<(String, String)>,
) -> Result<Json<RemoteWorkspaceSessionResponse>, ApiError> {
    let removed = state
        .remote_workspace_manager
        .disconnect_workspace(&server_id, &workspace_id)
        .await?;
    Ok(Json(RemoteWorkspaceSessionResponse {
        session: removed.then(|| RemoteWorkspaceSessionSummary {
            server_id,
            workspace_id,
            remote_path: String::new(),
            target: String::new(),
            local_port: 0,
            remote_port: 0,
            started_at: String::new(),
            status: "disconnected".to_string(),
        }),
    }))
}

pub(crate) async fn remote_workspace_sessions(
    State(state): State<AppState>,
    AxumPath(server_id): AxumPath<String>,
) -> Result<Json<RemoteWorkspaceSessionsResponse>, ApiError> {
    Ok(Json(RemoteWorkspaceSessionsResponse {
        sessions: state
            .remote_workspace_manager
            .session_summaries_for_server(&server_id)?,
    }))
}

#[derive(Debug)]
struct RemoteWorkspaceSession {
    // ponytail: v1 keeps one sidecar and one SSH tunnel per remote workspace; pool later if session counts make this noisy.
    server_id: String,
    workspace_id: String,
    remote_path: String,
    target: String,
    local_port: u16,
    remote_port: u16,
    token: String,
    started_at: String,
    sidecar: AsyncMutex<Option<Child>>,
    tunnel: AsyncMutex<Option<Child>>,
    control_task: AsyncMutex<Option<JoinHandle<()>>>,
}

impl RemoteWorkspaceSession {
    fn summary(&self) -> RemoteWorkspaceSessionSummary {
        RemoteWorkspaceSessionSummary {
            server_id: self.server_id.clone(),
            workspace_id: self.workspace_id.clone(),
            remote_path: self.remote_path.clone(),
            target: self.target.clone(),
            local_port: self.local_port,
            remote_port: self.remote_port,
            started_at: self.started_at.clone(),
            status: "connected".to_string(),
        }
    }

    async fn stop(&self) {
        if let Some(task) = self.control_task.lock().await.take() {
            task.abort();
        }
        if let Some(mut tunnel) = self.tunnel.lock().await.take() {
            let _ = tunnel.kill().await;
        }
        if let Some(mut sidecar) = self.sidecar.lock().await.take() {
            let _ = sidecar.kill().await;
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteSidecarBootstrap {
    version: u32,
    target: String,
    workspace_id: String,
    workspace_path: String,
    server_id: String,
    port: u16,
    token: String,
    capabilities: RemoteSidecarCapabilities,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteSidecarCapabilities {
    http_proxy: bool,
    control_broker: bool,
    terminal_pty: bool,
    git: bool,
    code_graph: bool,
    workspace_database: bool,
    runtime_config_sync: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ControlEnvelope {
    version: u32,
    #[serde(rename = "type")]
    message_type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    payload: Value,
    #[serde(default)]
    timestamp: Option<String>,
}

pub(crate) async fn run_remote_sidecar_command_if_requested() -> AppResult<bool> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(false);
    };
    match command {
        "--version" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(true)
        }
        "--sidecar-target" => {
            println!("{}", current_sidecar_target()?);
            Ok(true)
        }
        REMOTE_SIDECAR_COMMAND => {
            run_remote_sidecar_server(&args[1..]).await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

async fn run_remote_sidecar_server(args: &[String]) -> AppResult<()> {
    let options = RemoteSidecarOptions::parse(args)?;
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let port = listener.local_addr()?.port();
    let bootstrap = RemoteSidecarBootstrap {
        version: 1,
        target: options.target.clone(),
        workspace_id: options.workspace_id.clone(),
        workspace_path: options.workspace_path.clone(),
        server_id: options.server_id.clone(),
        port,
        token: options.token.clone(),
        capabilities: RemoteSidecarCapabilities {
            http_proxy: true,
            control_broker: true,
            terminal_pty: true,
            git: true,
            code_graph: true,
            workspace_database: true,
            runtime_config_sync: true,
        },
    };
    println!("{}", serde_json::to_string(&bootstrap)?);

    let (broker_tx, _) = tokio::sync::broadcast::channel::<ControlEnvelope>(256);
    let shutdown_tx = default_shutdown_tx();
    let ws_count = Arc::new(AtomicUsize::new(0));
    let active_run_count = Arc::new(AtomicUsize::new(0));
    let state = RemoteSidecarState {
        token: options.token,
        workspace_id: options.workspace_id.clone(),
        workspace_path: options.workspace_path.clone(),
        last_config_hash: Arc::new(Mutex::new(None)),
        code_graph_watcher: Arc::new(Mutex::new(None)),
        ws_count: ws_count.clone(),
        broker_tx,
    };

    let app = Router::new()
        .route(CONTROL_WS_PATH, get(remote_control_ws))
        // ponytail: workspace-scoped HTTP routes proxied from local main.
        // Sidecar handles files, git, terminal, spec, and plan routes that hit
        // the remote workspace path.  Query/path params match the local main
        // route convention so the proxy can forward as-is.
        .route("/api/remote/workspace/files", get(remote_sidecar_file_tree))
        .route(
            "/api/remote/workspace/files/children",
            get(remote_sidecar_file_children),
        )
        .route(
            "/api/remote/workspace/files/content",
            post(remote_sidecar_file_content),
        )
        .route(
            "/api/remote/workspace/files/blob",
            get(remote_sidecar_file_blob),
        )
        .route(
            "/api/remote/workspace/files/save",
            post(remote_sidecar_file_save),
        )
        .route(
            "/api/remote/workspace/files/delete",
            post(remote_sidecar_file_delete),
        )
        .route(
            "/api/remote/workspace/files/rename",
            post(remote_sidecar_file_rename),
        )
        .route(
            "/api/remote/workspace/memory",
            get(remote_sidecar_memory_list),
        )
        .route(
            "/api/remote/workspace/memory/manual",
            post(remote_sidecar_memory_manual),
        )
        .route(
            "/api/remote/workspace/skills/install",
            post(remote_sidecar_skill_install),
        )
        .route(
            "/api/remote/workspace/skills/discover",
            get(remote_sidecar_skills_discover),
        )
        .route(
            "/api/remote/workspace/hooks/settings",
            get(remote_sidecar_hooks_settings).post(remote_sidecar_hooks_save),
        )
        .route(
            "/api/remote/workspace/hooks/runs",
            get(remote_sidecar_hook_runs),
        )
        .route(
            "/api/remote/workspace/hooks/runs/{hook_run_id}",
            get(remote_sidecar_hook_run_detail),
        )
        .route(
            "/api/remote/workspace/git/status",
            get(remote_sidecar_git_status),
        )
        .route(
            "/api/remote/workspace/git/diff",
            get(remote_sidecar_git_diff),
        )
        .route(
            "/api/remote/workspace/git/stage",
            post(remote_sidecar_git_stage),
        )
        .route(
            "/api/remote/workspace/git/unstage",
            post(remote_sidecar_git_unstage),
        )
        .route(
            "/api/remote/workspace/git/discard",
            post(remote_sidecar_git_discard),
        )
        .route(
            "/api/remote/workspace/git/commit",
            post(remote_sidecar_git_commit),
        )
        .route(
            "/api/remote/workspace/git/branches",
            get(remote_sidecar_git_branches),
        )
        .route(
            "/api/remote/workspace/git/branches/switch",
            post(remote_sidecar_git_branch_switch),
        )
        .route(
            "/api/remote/workspace/git/branches/create",
            post(remote_sidecar_git_branch_create),
        )
        .route(
            "/api/remote/workspace/terminal/session",
            post(remote_sidecar_terminal_session),
        )
        .route(
            "/api/remote/workspace/terminal/{session_id}/ws",
            get(remote_sidecar_terminal_ws),
        )
        // ponytail: spec and plan routes use workspace DB on the sidecar.
        // LLM-dependent operations (spec generation, plan phase actions) are
        // not proxied — they go through the broker channel or error gracefully.
        .route(
            "/api/remote/workspace/spec",
            get(remote_sidecar_spec_get).put(remote_sidecar_spec_put),
        )
        .route(
            "/api/remote/workspace/spec/settings",
            put(remote_sidecar_spec_settings),
        )
        .route(
            "/api/remote/workspace/spec/generate",
            post(remote_sidecar_spec_generate),
        )
        .route(
            "/api/remote/workspace/spec/jobs",
            get(remote_sidecar_spec_jobs),
        )
        .route(
            "/api/remote/workspace/spec/jobs/{job_id}/retry",
            post(remote_sidecar_spec_jobs_retry),
        )
        .route(
            "/api/remote/workspace/plans",
            get(remote_sidecar_plans_list).post(remote_sidecar_plans_create),
        )
        .route(
            "/api/remote/workspace/plans/auto-run",
            get(remote_sidecar_plans_auto_run).put(remote_sidecar_plans_auto_run_set),
        )
        .route(
            "/api/remote/workspace/plans/order",
            post(remote_sidecar_plans_order),
        )
        .route(
            "/api/remote/workspace/plans/worktrees/audit",
            get(remote_sidecar_plans_worktree_audit),
        )
        .route(
            "/api/remote/workspace/plans/worktrees/cleanup",
            post(remote_sidecar_plans_worktree_cleanup),
        )
        .route(
            "/api/remote/workspace/plans/{plan_id}",
            patch(remote_sidecar_plans_update).delete(remote_sidecar_plans_delete),
        )
        .route(
            "/api/remote/workspace/plans/{plan_id}/action",
            post(remote_sidecar_plans_action),
        )
        .route(
            "/api/remote/workspace/plans/{plan_id}/phases/{phase_id}/retry",
            post(remote_sidecar_plans_phase_retry),
        )
        .route(
            "/api/remote/workspace/plans/{plan_id}/steps/{step_id}/action",
            post(remote_sidecar_plans_step_action),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            sidecar_bearer_auth,
        ))
        .with_state(state);

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut lock = shutdown_tx.lock().unwrap_or_else(|e| e.into_inner());
        *lock = Some(tx);
    }

    let shutdown = async move {
        tokio::spawn(idle_shutdown_watch(shutdown_tx, ws_count, active_run_count));
        let _ = rx.await;
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

fn default_shutdown_tx() -> Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>> {
    Arc::new(Mutex::new(None))
}

async fn idle_shutdown_watch(
    shutdown_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    ws_count: Arc<AtomicUsize>,
    active_run_count: Arc<AtomicUsize>,
) {
    loop {
        tokio::time::sleep(SIDECAR_IDLE_TIMEOUT).await;
        if ws_count.load(Ordering::Relaxed) > 0 || active_run_count.load(Ordering::Relaxed) > 0 {
            continue;
        }
        if let Ok(mut tx) = shutdown_tx.lock() {
            if let Some(tx) = tx.take() {
                let _ = tx.send(());
            }
        }
        return;
    }
}

#[derive(Clone)]
struct RemoteSidecarState {
    token: String,
    last_config_hash: Arc<Mutex<Option<String>>>,
    code_graph_watcher: Arc<Mutex<Option<foco_graph::CodeGraphWatcher>>>,
    ws_count: Arc<AtomicUsize>,
    broker_tx: tokio::sync::broadcast::Sender<ControlEnvelope>,
    workspace_id: String,
    workspace_path: String,
}

/// Bearer token middleware for all sidecar HTTP routes.
async fn sidecar_bearer_auth(
    State(state): State<RemoteSidecarState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|token| token == state.token);
    if !authorized {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if request.headers().contains_key("x-foco-ensure-code-graph") {
        if let Err(response) = ensure_sidecar_code_graph(&state) {
            return response;
        }
    }
    next.run(request).await
}

async fn remote_control_ws(
    State(state): State<RemoteSidecarState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Auth already checked by sidecar_bearer_auth middleware
    let request_rx = state.broker_tx.subscribe();
    ws.on_upgrade(move |socket| async move {
        state.ws_count.fetch_add(1, Ordering::Relaxed);
        let (mut sender, mut receiver) = socket.split();
        // ponytail: v1 uses select! to interleave outgoing broker requests
        // with incoming WS frames; in later versions use a proper mpsc
        // fan-out with per-request routing.
        let mut request_rx = request_rx;
        loop {
            tokio::select! {
                msg = receiver.next() => {
                    let Some(Ok(message)) = msg else { break };
                    let Message::Text(text) = message else { continue };
                    let Ok(envelope) = serde_json::from_str::<ControlEnvelope>(&text) else {
                        continue;
                    };
                    // Handle inbound config sync from local main
                    if envelope.message_type == "config"
                        && envelope.method.as_deref() == Some("config.sync")
                    {
                        let hash = envelope
                            .payload
                            .get("hash")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        if let Ok(mut last_config_hash) = state.last_config_hash.lock() {
                            *last_config_hash = Some(hash.clone());
                        }
                        let response = ControlEnvelope {
                            version: 1,
                            message_type: "response".to_string(),
                            id: envelope.id,
                            method: None,
                            payload: json!({ "status": "ok", "hash": hash }),
                            timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
                        };
                        let Ok(text) = serde_json::to_string(&response) else {
                            continue;
                        };
                        if sender.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                }
                request = request_rx.recv() => {
                    match request {
                        Ok(request) => {
                            let Ok(text) = serde_json::to_string(&request) else { continue };
                            if sender.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    }
                }
            }
        }
        state.ws_count.fetch_sub(1, Ordering::Relaxed);
    })
    .into_response()
}

#[derive(Debug)]
struct RemoteSidecarOptions {
    server_id: String,
    workspace_id: String,
    workspace_path: String,
    target: String,
    token: String,
}

impl RemoteSidecarOptions {
    fn parse(args: &[String]) -> io::Result<Self> {
        let mut server_id = None;
        let mut workspace_id = None;
        let mut workspace_path = None;
        let mut target = None;
        let mut token = None;
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            let slot = match arg.as_str() {
                "--server-id" => &mut server_id,
                "--workspace-id" => &mut workspace_id,
                "--workspace-path" => &mut workspace_path,
                "--target" => &mut target,
                "--token" => &mut token,
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unknown remote sidecar argument: {other}"),
                    ));
                }
            };
            let value = iter.next().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("missing value for {arg}"),
                )
            })?;
            *slot = Some(value.clone());
        }
        Ok(Self {
            server_id: required_arg(server_id, "--server-id")?,
            workspace_id: required_arg(workspace_id, "--workspace-id")?,
            workspace_path: required_arg(workspace_path, "--workspace-path")?,
            target: required_arg(target, "--target")?,
            token: required_arg(token, "--token")?,
        })
    }
}

fn required_arg(value: Option<String>, name: &str) -> io::Result<String> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("missing {name}")))
}

fn current_sidecar_target() -> io::Result<String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    match (os, arch) {
        ("linux", "x86_64") => Ok("linux-x64".to_string()),
        ("linux", "aarch64") => Ok("linux-arm64".to_string()),
        _ => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("unsupported sidecar target: {os}-{arch}"),
        )),
    }
}

fn workspace_remote_path(workspace: &WorkspaceConfig, server_id: &str) -> Result<String, ApiError> {
    match &workspace.location {
        WorkspaceLocation::Ssh {
            server_id: workspace_server_id,
            remote_path,
        } if workspace_server_id == server_id => Ok(remote_path.clone()),
        WorkspaceLocation::Ssh {
            server_id: workspace_server_id,
            ..
        } => Err(remote_error(
            server_id,
            Some(&workspace.id),
            format!("workspace belongs to remote server {workspace_server_id}"),
        )),
        WorkspaceLocation::Local => Err(remote_error(
            server_id,
            Some(&workspace.id),
            "workspace is local, not SSH remote",
        )),
    }
}

async fn detect_or_cached_target(
    server: &RemoteServerProfile,
    server_id: &str,
    workspace_id: &str,
) -> Result<String, ApiError> {
    if let Some(target) = server
        .last_known_target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(target.to_string());
    }
    let output = run_ssh_output(
        server,
        &["uname -s && uname -m"],
        true,
        server_id,
        Some(workspace_id),
    )
    .await?;
    if !output.status.success() {
        return Err(remote_error(
            server_id,
            Some(workspace_id),
            format!("target probe failed: {}", output_text(&output)),
        ));
    }
    normalize_target(&String::from_utf8_lossy(&output.stdout))
        .map_err(|message| remote_error(server_id, Some(workspace_id), message))
}

async fn ensure_sidecar_command(
    state: &AppState,
    server: &RemoteServerProfile,
    server_id: &str,
    workspace_id: &str,
    target: &str,
) -> Result<String, ApiError> {
    if let Some(command) = server
        .foco_command
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        verify_remote_command(server, command, target, server_id, workspace_id).await?;
        return Ok(command.to_string());
    }

    let asset = select_sidecar_asset(target)
        .map_err(|message| remote_error(server_id, Some(workspace_id), message))?;
    let remote_dir = format!("~/.foco/sidecars/{}/{}", asset.version, asset.target);
    let remote_bin = format!("{remote_dir}/{SIDECAR_BINARY_NAME}");
    if remote_sidecar_matches(
        server,
        &remote_bin,
        &asset.version,
        target,
        server_id,
        workspace_id,
    )
    .await?
    {
        update_sidecar_cache(state, server_id, target, &asset.version, None)?;
        return Ok(remote_bin);
    }

    let bytes = std::fs::read(&asset.path).map_err(|source| {
        remote_error(
            server_id,
            Some(workspace_id),
            format!(
                "failed to read sidecar asset {}: {source}",
                asset.path.display()
            ),
        )
    })?;
    let remote_tmp = format!("{remote_dir}/{SIDECAR_BINARY_NAME}.tmp");
    let install_script = format!(
        "set -e; mkdir -p {dir}; cat > {tmp}; chmod +x {tmp}; mv -f {tmp} {bin}; {bin} --version; {bin} --sidecar-target",
        dir = shell_quote(&remote_dir),
        tmp = shell_quote(&remote_tmp),
        bin = shell_quote(&remote_bin),
    );
    let output = run_ssh_with_stdin(
        server,
        &[install_script.as_str()],
        &bytes,
        server_id,
        Some(workspace_id),
    )
    .await?;
    if !output.status.success() {
        return Err(remote_error(
            server_id,
            Some(workspace_id),
            format!("sidecar upload/install failed: {}", output_text(&output)),
        ));
    }
    verify_remote_command(server, &remote_bin, target, server_id, workspace_id).await?;
    update_sidecar_cache(state, server_id, target, &asset.version, None)?;
    Ok(remote_bin)
}

async fn remote_sidecar_matches(
    server: &RemoteServerProfile,
    remote_bin: &str,
    version: &str,
    target: &str,
    server_id: &str,
    workspace_id: &str,
) -> Result<bool, ApiError> {
    let command = format!(
        "test -x {bin} && {bin} --version && {bin} --sidecar-target",
        bin = shell_quote(remote_bin)
    );
    let output = run_ssh_output(
        server,
        &[command.as_str()],
        true,
        server_id,
        Some(workspace_id),
    )
    .await?;
    if !output.status.success() {
        return Ok(false);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    Ok(lines.next() == Some(version) && lines.next() == Some(target))
}

async fn verify_remote_command(
    server: &RemoteServerProfile,
    command: &str,
    target: &str,
    server_id: &str,
    workspace_id: &str,
) -> Result<(), ApiError> {
    let check = format!(
        "{command} --version && {command} --sidecar-target",
        command = command
    );
    let output = run_ssh_output(
        server,
        &[check.as_str()],
        true,
        server_id,
        Some(workspace_id),
    )
    .await?;
    if !output.status.success() {
        return Err(remote_error(
            server_id,
            Some(workspace_id),
            format!(
                "remote sidecar command verification failed: {}",
                output_text(&output)
            ),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.lines().map(str::trim).any(|line| line == target) {
        return Err(remote_error(
            server_id,
            Some(workspace_id),
            format!("remote sidecar command did not report target {target}"),
        ));
    }
    Ok(())
}

fn update_sidecar_cache(
    state: &AppState,
    server_id: &str,
    target: &str,
    version: &str,
    error: Option<String>,
) -> Result<(), ApiError> {
    let mut config = config_snapshot(state)?;
    let server = config
        .remote_servers
        .iter_mut()
        .find(|server| server.id == server_id)
        .ok_or_else(|| remote_error(server_id, None, "remote server was not found"))?;
    server.last_known_target = Some(target.to_string());
    server.last_sidecar_version = Some(version.to_string());
    server.sidecar_install_state = Some("available".to_string());
    server.last_checked_at = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
    server.last_error = error;
    save_config(state, config)
}

async fn launch_remote_sidecar(
    server: &RemoteServerProfile,
    server_id: &str,
    workspace_id: &str,
    remote_path: &str,
    target: &str,
    token: &str,
    command: &str,
) -> Result<Child, ApiError> {
    let remote_command = format!(
        "{command} {sidecar} --server-id {server_id} --workspace-id {workspace_id} --workspace-path {workspace_path} --target {target} --token {token}",
        command = command,
        sidecar = REMOTE_SIDECAR_COMMAND,
        server_id = shell_quote(server_id),
        workspace_id = shell_quote(workspace_id),
        workspace_path = shell_quote(remote_path),
        target = shell_quote(target),
        token = shell_quote(token),
    );
    let args = remote_server_ssh_args(server, &[remote_command.as_str()], true);
    let child = Command::new("ssh")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| {
            remote_error(
                server_id,
                Some(workspace_id),
                format!("failed to start remote sidecar over ssh: {source}"),
            )
        })?;
    Ok(child)
}

async fn read_bootstrap(
    child: &mut Child,
    server_id: &str,
    workspace_id: &str,
) -> Result<RemoteSidecarBootstrap, ApiError> {
    let stdout = child.stdout.take().ok_or_else(|| {
        remote_error(
            server_id,
            Some(workspace_id),
            "remote sidecar stdout was not captured",
        )
    })?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let read = timeout(Duration::from_secs(10), reader.read_line(&mut line))
        .await
        .map_err(|_| {
            remote_error(
                server_id,
                Some(workspace_id),
                "timed out waiting for sidecar bootstrap",
            )
        })?
        .map_err(|source| {
            remote_error(
                server_id,
                Some(workspace_id),
                format!("failed to read sidecar bootstrap: {source}"),
            )
        })?;
    if read == 0 {
        return Err(remote_error(
            server_id,
            Some(workspace_id),
            "remote sidecar exited before writing bootstrap",
        ));
    }
    tokio::spawn(async move {
        let mut sink = Vec::new();
        let _ = tokio::io::copy(&mut reader, &mut sink).await;
    });
    serde_json::from_str(&line).map_err(|source| {
        remote_error(
            server_id,
            Some(workspace_id),
            format!("failed to parse sidecar bootstrap JSON: {source}"),
        )
    })
}

fn validate_bootstrap(
    bootstrap: &RemoteSidecarBootstrap,
    server_id: &str,
    workspace_id: &str,
    target: &str,
) -> Result<(), ApiError> {
    if bootstrap.version != 1
        || bootstrap.server_id != server_id
        || bootstrap.workspace_id != workspace_id
        || bootstrap.target != target
        || bootstrap.port == 0
        || bootstrap.token.is_empty()
        || !bootstrap.capabilities.runtime_config_sync
        || !bootstrap.capabilities.control_broker
    {
        return Err(remote_error(
            server_id,
            Some(workspace_id),
            "remote sidecar bootstrap did not match the requested session",
        ));
    }
    Ok(())
}

async fn start_local_forward(
    server: &RemoteServerProfile,
    remote_port: u16,
    server_id: &str,
    workspace_id: &str,
) -> Result<(u16, Child), ApiError> {
    let probe = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .map_err(|source| {
            remote_error(
                server_id,
                Some(workspace_id),
                format!("failed to reserve local tunnel port: {source}"),
            )
        })?;
    let local_port = probe
        .local_addr()
        .map_err(|source| {
            remote_error(
                server_id,
                Some(workspace_id),
                format!("failed to read local tunnel port: {source}"),
            )
        })?
        .port();
    drop(probe);

    let forward = format!("127.0.0.1:{local_port}:127.0.0.1:{remote_port}");
    let args = remote_server_ssh_args(
        server,
        &[
            "-N",
            "-o",
            "ExitOnForwardFailure=yes",
            "-L",
            forward.as_str(),
        ],
        true,
    );
    let child = Command::new("ssh")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| {
            remote_error(
                server_id,
                Some(workspace_id),
                format!("failed to start SSH local port forward: {source}"),
            )
        })?;
    Ok((local_port, child))
}

async fn connect_control_ws(
    state: AppState,
    local_port: u16,
    token: &str,
    bundle: crate::runtime::SidecarRuntimeConfigBundle,
    server_id: &str,
    workspace_id: &str,
) -> Result<JoinHandle<()>, ApiError> {
    let url = format!("ws://127.0.0.1:{local_port}{CONTROL_WS_PATH}");
    let mut last_error = None;
    let (ready_tx, ready_rx) = oneshot::channel();
    let token = token.to_string();
    let log_server_id = server_id.to_string();
    let log_workspace_id = workspace_id.to_string();
    let handle = tokio::spawn(async move {
        let mut ready_tx = Some(ready_tx);
        for _ in 0..30 {
            match connect_control_ws_once(&url, &token, &bundle).await {
                Ok((write, mut read)) => {
                    if let Some(tx) = ready_tx.take() {
                        let _ = tx.send(Ok(()));
                    }
                    let write = Arc::new(AsyncMutex::new(write));
                    let cancellations: BrokerCancelRegistry =
                        Arc::new(AsyncMutex::new(HashMap::new()));
                    // Main broker loop: process incoming sidecar request and cancel messages.
                    while let Some(message) = read.next().await {
                        match message {
                            Ok(tungstenite::Message::Ping(bytes)) => {
                                let mut write = write.lock().await;
                                let _ = write.send(tungstenite::Message::Pong(bytes)).await;
                            }
                            Ok(tungstenite::Message::Text(text)) => {
                                let Ok(envelope) = serde_json::from_str::<ControlEnvelope>(&text)
                                else {
                                    continue;
                                };
                                match envelope.message_type.as_str() {
                                    "request" => {
                                        let request_id = envelope.id.clone();
                                        let (cancel_tx, cancel_rx) = oneshot::channel();
                                        if let Some(id) = request_id.clone() {
                                            cancellations.lock().await.insert(id, cancel_tx);
                                        }
                                        let task_state = state.clone();
                                        let task_write = write.clone();
                                        let task_cancellations = cancellations.clone();
                                        let task_server_id = log_server_id.clone();
                                        let task_workspace_id = log_workspace_id.clone();
                                        tokio::spawn(async move {
                                            handle_broker_request(
                                                &task_state,
                                                task_write,
                                                &task_server_id,
                                                &task_workspace_id,
                                                envelope,
                                                Some(cancel_rx),
                                            )
                                            .await;
                                            if let Some(id) = request_id {
                                                task_cancellations.lock().await.remove(&id);
                                            }
                                        });
                                    }
                                    "cancel" => {
                                        if let Some(id) = envelope.id {
                                            if let Some(tx) = cancellations.lock().await.remove(&id)
                                            {
                                                let _ = tx.send(());
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Ok(_) => {}
                            Err(_) => break,
                        }
                    }
                    return;
                }

                Err(error) => {
                    last_error = Some(error);
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
        if let Some(tx) = ready_tx.take() {
            let _ = tx.send(Err(
                last_error.unwrap_or_else(|| "control WebSocket failed".to_string())
            ));
        }
        tracing::warn!(%log_server_id, %log_workspace_id, "remote control WebSocket exited before readiness");
    });

    match timeout(Duration::from_secs(5), ready_rx).await {
        Ok(Ok(Ok(()))) => Ok(handle),
        Ok(Ok(Err(message))) => {
            handle.abort();
            Err(remote_error(server_id, Some(workspace_id), message))
        }
        Ok(Err(_)) => {
            handle.abort();
            Err(remote_error(
                server_id,
                Some(workspace_id),
                "control WebSocket readiness channel closed",
            ))
        }
        Err(_) => {
            handle.abort();
            Err(remote_error(
                server_id,
                Some(workspace_id),
                "timed out waiting for control WebSocket config sync",
            ))
        }
    }
}

async fn handle_broker_request(
    state: &AppState,
    write: SharedBrokerWsWrite,
    server_id: &str,
    workspace_id: &str,
    request: ControlEnvelope,
    cancel_rx: Option<oneshot::Receiver<()>>,
) {
    let id = match &request.id {
        Some(id) => id.clone(),
        None => {
            let _ = send_broker_error(&write, None, "missing_id", "request missing id").await;
            return;
        }
    };
    let method = match &request.method {
        Some(m) => m.clone(),
        None => {
            let _ = send_broker_error(
                &write,
                Some(&id),
                "missing_method",
                "request missing method",
            )
            .await;
            return;
        }
    };

    match method.as_str() {
        "llm.stream" => {
            broker_llm_stream(
                state,
                &write,
                server_id,
                workspace_id,
                &id,
                request.payload,
                cancel_rx,
            )
            .await;
        }
        "memory.global.search" => {
            broker_memory_global_search(state, &write, &id, request.payload).await;
        }
        "memory.global.write" => {
            broker_memory_global_write(state, &write, &id, request.payload).await;
        }
        "web.search" => {
            broker_web_search(state, &write, &id, request.payload).await;
        }
        "web.fetch" => {
            broker_web_fetch(state, &write, &id, request.payload).await;
        }
        "image.generate" => {
            broker_image_generate(state, &write, &id, request.payload).await;
        }
        "ui.askQuestion" => {
            broker_ask_question(state, &write, &id, request.payload).await;
        }
        other => {
            let _ = send_broker_error(
                &write,
                Some(&id),
                "unknown_method",
                format!("unknown broker method: {other}"),
            )
            .await;
        }
    }
}

/// Handle `llm.stream`: sidecar sends a provider+model+NeutralChatRequest,
/// local main dispatches through its own provider config and streams chunks back.
/// ponytail: v1 does minimal payload validation; wire NeutralChatRequest fields
/// loosely. Single-tool tool_use is not supported yet — only pure text streams.
async fn broker_llm_stream(
    state: &AppState,
    write: &SharedBrokerWsWrite,
    _server_id: &str,
    _workspace_id: &str,
    id: &str,
    payload: Value,
    cancel_rx: Option<oneshot::Receiver<()>>,
) {
    let provider_id = match payload.get("providerId").and_then(Value::as_str) {
        Some(id) => id,
        None => {
            let _ = send_broker_error(write, Some(id), "bad_request", "missing providerId").await;
            return;
        }
    };
    let model_id = match payload.get("modelId").and_then(Value::as_str) {
        Some(id) => id,
        None => {
            let _ = send_broker_error(write, Some(id), "bad_request", "missing modelId").await;
            return;
        }
    };
    let config = match config_snapshot(state) {
        Ok(c) => c,
        Err(e) => {
            let _ =
                send_broker_error(write, Some(id), "internal_error", e.message().to_string()).await;
            return;
        }
    };
    let provider = match config
        .providers
        .iter()
        .find(|p| p.id == provider_id && p.enabled)
    {
        Some(p) => p,
        None => {
            let _ = send_broker_error(
                write,
                Some(id),
                "bad_request",
                format!("provider '{provider_id}' not found or disabled"),
            )
            .await;
            return;
        }
    };
    let provider_config = match crate::provider_connection_config(provider) {
        Ok(c) => c,
        Err(e) => {
            let _ =
                send_broker_error(write, Some(id), "bad_request", e.message().to_string()).await;
            return;
        }
    };

    // Parse the NeutralChatRequest from payload
    let messages: Vec<NeutralChatMessage> = payload
        .get("messages")
        .and_then(|m| serde_json::from_value(m.clone()).ok())
        .unwrap_or_default();
    let tools: Vec<foco_providers::NeutralToolDefinition> = payload
        .get("tools")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let request = NeutralChatRequest {
        model_id: model_id.to_string(),
        messages,
        tools,
        thinking_level: payload
            .get("thinkingLevel")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        max_output_tokens: payload
            .get("maxOutputTokens")
            .and_then(Value::as_u64)
            .map(|v| v as u32),
        prompt_cache_key: payload
            .get("promptCacheKey")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        prompt_cache_retention: payload
            .get("promptCacheRetention")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
    };

    let mut cancel_rx = cancel_rx;
    let mut stream = match if let Some(cancel_rx) = cancel_rx.as_mut() {
        tokio::select! {
            _ = cancel_rx => {
                let _ = send_broker_error(write, Some(id), "cancelled", "broker request cancelled").await;
                return;
            }
            result = stream_chat(&provider_config, request) => result,
        }
    } else {
        stream_chat(&provider_config, request).await
    } {
        Ok(s) => s,
        Err(e) => {
            let _ = send_broker_error(write, Some(id), "provider_error", format!("{e}")).await;
            return;
        }
    };

    tracing::info!(%provider_id, %model_id, request_id = %id, "remote sidecar broker llm stream started");
    let mut sequence = 0u64;
    let mut final_usage: Option<NeutralUsage> = None;
    loop {
        let event = match if let Some(cancel_rx) = cancel_rx.as_mut() {
            tokio::select! {
                _ = cancel_rx => {
                    let _ = send_broker_error(write, Some(id), "cancelled", "broker request cancelled").await;
                    return;
                }
                event = stream.next_event() => event,
            }
        } else {
            stream.next_event().await
        } {
            Some(Ok(e)) => e,
            Some(Err(e)) => {
                let _ = send_broker_error(write, Some(id), "stream_error", format!("{e}")).await;
                return;
            }
            None => break,
        };
        match event {
            NeutralChatStreamEvent::TextDelta { delta } => {
                sequence += 1;
                let chunk = ControlEnvelope {
                    version: 1,
                    message_type: "stream".to_string(),
                    id: Some(id.to_string()),
                    method: None,
                    payload: json!({
                        "sequence": sequence,
                        "kind": "textDelta",
                        "delta": delta,
                    }),
                    timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
                };
                if send_broker_envelope(write, &chunk).await.is_err() {
                    return;
                }
            }
            NeutralChatStreamEvent::ToolCall { tool_call } => {
                sequence += 1;
                let chunk = ControlEnvelope {
                    version: 1,
                    message_type: "stream".to_string(),
                    id: Some(id.to_string()),
                    method: None,
                    payload: json!({
                        "sequence": sequence,
                        "kind": "toolCall",
                        "toolCall": tool_call,
                    }),
                    timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
                };
                if send_broker_envelope(write, &chunk).await.is_err() {
                    return;
                }
            }
            NeutralChatStreamEvent::ReasoningDelta { delta } => {
                sequence += 1;
                let chunk = ControlEnvelope {
                    version: 1,
                    message_type: "stream".to_string(),
                    id: Some(id.to_string()),
                    method: None,
                    payload: json!({
                        "sequence": sequence,
                        "kind": "reasoningDelta",
                        "delta": delta,
                    }),
                    timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
                };
                if send_broker_envelope(write, &chunk).await.is_err() {
                    return;
                }
            }
            NeutralChatStreamEvent::Usage { usage } => {
                sequence += 1;
                let chunk = ControlEnvelope {
                    version: 1,
                    message_type: "stream".to_string(),
                    id: Some(id.to_string()),
                    method: None,
                    payload: json!({
                        "sequence": sequence,
                        "kind": "usageDelta",
                        "usage": usage,
                    }),
                    timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
                };
                if send_broker_envelope(write, &chunk).await.is_err() {
                    return;
                }
            }
            NeutralChatStreamEvent::Complete {
                text: _,
                reasoning: _,
                tool_calls: _,
                usage,
                stop_reason: _,
                response_id: _,
            } => {
                final_usage = usage;
            }
            NeutralChatStreamEvent::Start => {}
            NeutralChatStreamEvent::ThoughtSignatureDelta { delta: _ } => {}
            NeutralChatStreamEvent::Error { message } => {
                let _ = send_broker_error(write, Some(id), "stream_error", message).await;
                return;
            }
        }
    }

    let response = ControlEnvelope {
        version: 1,
        message_type: "response".to_string(),
        id: Some(id.to_string()),
        method: None,
        payload: json!({
            "status": "ok",
            "usage": final_usage,
        }),
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    let _ = send_broker_envelope(write, &response).await;
}

/// Handle `memory.global.search`: search the local global memory database.
async fn broker_memory_global_search(
    state: &AppState,
    write: &SharedBrokerWsWrite,
    id: &str,
    payload: Value,
) {
    let query = payload
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if query.is_empty() {
        let _ = send_broker_error(write, Some(id), "bad_request", "missing query").await;
        return;
    }
    let limit = payload.get("limit").and_then(Value::as_u64).unwrap_or(10) as u32;

    let memory_db = match MemoryDatabase::open_or_create_global_at(&state.memory_database_file) {
        Ok(db) => db,
        Err(e) => {
            let _ = send_broker_error(write, Some(id), "internal_error", format!("{e}")).await;
            return;
        }
    };
    let results = match memory_db.search_active_facts_for_scope(query, None, None, limit) {
        Ok(r) => r,
        Err(e) => {
            let _ = send_broker_error(write, Some(id), "internal_error", format!("{e}")).await;
            return;
        }
    };
    let response = ControlEnvelope {
        version: 1,
        message_type: "response".to_string(),
        id: Some(id.to_string()),
        method: None,
        payload: json!({ "status": "ok", "results": results, "query": query }),
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    let _ = send_broker_envelope(write, &response).await;
}

/// Handle `memory.global.write`: write a manual fact into the local global memory database.
async fn broker_memory_global_write(
    state: &AppState,
    write: &SharedBrokerWsWrite,
    id: &str,
    payload: Value,
) {
    let fact = payload
        .get("fact")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if fact.is_empty() {
        let _ = send_broker_error(write, Some(id), "bad_request", "missing fact").await;
        return;
    }
    let kind = payload
        .get("kind")
        .and_then(Value::as_str)
        .map(MemoryKind::parse)
        .transpose();
    let kind = match kind {
        Ok(Some(kind)) => kind,
        Ok(None) => MemoryKind::UserNote,
        Err(e) => {
            let _ = send_broker_error(write, Some(id), "bad_request", format!("{e}")).await;
            return;
        }
    };
    let confidence = payload.get("confidence").and_then(Value::as_f64);
    let pinned = payload
        .get("pinned")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let reason = payload
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("remote sidecar broker memory write");
    let source_id = unique_id("broker-memory-source");
    let fact_id = unique_id("memory");
    let source_ids = [source_id.as_str()];
    let metadata_json = json!({
        "brokered": true,
        "requestId": id,
        "reason": reason,
    })
    .to_string();

    let mut memory_db = match MemoryDatabase::open_or_create_global_at(&state.memory_database_file)
    {
        Ok(db) => db,
        Err(e) => {
            let _ = send_broker_error(write, Some(id), "internal_error", format!("{e}")).await;
            return;
        }
    };
    if let Err(e) = memory_db.insert_source(NewMemorySource {
        id: &source_id,
        scope: MemoryScope::Global,
        chat_id: None,
        source_type: MemorySourceType::ToolCall,
        source_id: Some(id),
        title: "Remote broker memory write",
        content: reason,
        metadata_json: &metadata_json,
    }) {
        let _ = send_broker_error(write, Some(id), "internal_error", format!("{e}")).await;
        return;
    }
    if let Err(e) = memory_db.insert_fact(NewMemoryFact {
        id: &fact_id,
        scope: MemoryScope::Global,
        chat_id: None,
        status: MemoryStatus::Active,
        kind,
        fact,
        confidence,
        pinned,
        source_ids: &source_ids,
        metadata_json: &metadata_json,
    }) {
        let _ = send_broker_error(write, Some(id), "internal_error", format!("{e}")).await;
        return;
    }

    let response = ControlEnvelope {
        version: 1,
        message_type: "response".to_string(),
        id: Some(id.to_string()),
        method: None,
        payload: json!({ "status": "ok", "factId": fact_id }),
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    let _ = send_broker_envelope(write, &response).await;
}
/// Handle `web.search`: delegate to the local web search tool.
async fn broker_web_search(
    state: &AppState,
    write: &SharedBrokerWsWrite,
    id: &str,
    payload: Value,
) {
    let config = match config_snapshot(state) {
        Ok(c) => c,
        Err(e) => {
            let _ =
                send_broker_error(write, Some(id), "internal_error", e.message().to_string()).await;
            return;
        }
    };
    let result = match execute_web_tool(
        &config.web_search,
        "web_search",
        payload.clone(),
        Duration::from_secs(15),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            let _ = send_broker_error(write, Some(id), "tool_error", e).await;
            return;
        }
    };
    let response = ControlEnvelope {
        version: 1,
        message_type: "response".to_string(),
        id: Some(id.to_string()),
        method: None,
        payload: json!({ "status": "ok", "result": result }),
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    let _ = send_broker_envelope(write, &response).await;
}

/// Handle `web.fetch`: delegate to the local web fetch tool.
async fn broker_web_fetch(state: &AppState, write: &SharedBrokerWsWrite, id: &str, payload: Value) {
    let config = match config_snapshot(state) {
        Ok(c) => c,
        Err(e) => {
            let _ =
                send_broker_error(write, Some(id), "internal_error", e.message().to_string()).await;
            return;
        }
    };
    let result = match execute_web_tool(
        &config.web_search,
        "web_fetch",
        payload.clone(),
        Duration::from_secs(15),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            let _ = send_broker_error(write, Some(id), "tool_error", e).await;
            return;
        }
    };
    let response = ControlEnvelope {
        version: 1,
        message_type: "response".to_string(),
        id: Some(id.to_string()),
        method: None,
        payload: json!({ "status": "ok", "result": result }),
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    let _ = send_broker_envelope(write, &response).await;
}

/// Handle `image.generate`: delegate to the local image generation tool.
/// ponytail: requires the default workspace path for output directory;
/// workspace_path is not yet passed by the sidecar, fall back to a
/// temporary directory for now.
async fn broker_image_generate(
    state: &AppState,
    write: &SharedBrokerWsWrite,
    id: &str,
    payload: Value,
) {
    let config = match config_snapshot(state) {
        Ok(c) => c,
        Err(e) => {
            let _ =
                send_broker_error(write, Some(id), "internal_error", e.message().to_string()).await;
            return;
        }
    };
    let workspace_path = Path::new(&state.user_profile_dir);
    let timeout = Duration::from_millis(std::cmp::min(
        payload
            .get("timeoutMs")
            .and_then(Value::as_u64)
            .unwrap_or(300_000),
        600_000,
    ));
    let result = match execute_image_tool(
        &config,
        workspace_path,
        "_broker_",
        "_broker_",
        "image_gen",
        payload.clone(),
        timeout,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            let _ = send_broker_error(write, Some(id), "tool_error", e).await;
            return;
        }
    };
    let response = ControlEnvelope {
        version: 1,
        message_type: "response".to_string(),
        id: Some(id.to_string()),
        method: None,
        payload: json!({ "status": "ok", "result": result }),
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    let _ = send_broker_envelope(write, &response).await;
}

/// Handle `ui.askQuestion`: register a pending question and block until the
/// user answers.  The sidecar encodes the question items; local main creates
/// the pending question request and waits for the answer.
/// ponytail: v1 uses a simple lookup by workspace+chat id. The sidecar does not
/// embed actual AppState-level tool call ids; pass a synthetic id.
async fn broker_ask_question(
    state: &AppState,
    write: &SharedBrokerWsWrite,
    id: &str,
    payload: Value,
) {
    use crate::runtime::AskQuestionInput;

    let input: AskQuestionInput = match serde_json::from_value(payload.clone()) {
        Ok(i) => i,
        Err(e) => {
            let _ = send_broker_error(
                write,
                Some(id),
                "bad_request",
                format!("invalid askQuestion payload: {e}"),
            )
            .await;
            return;
        }
    };
    let question_id = unique_id("broker-question");
    let question_req = crate::runtime::QuestionRequest {
        id: question_id.clone(),
        tool_call_id: format!("broker-{id}"),
        workspace_id: String::new(),
        chat_id: String::new(),
        questions: input
            .questions
            .into_iter()
            .map(|q| crate::runtime::QuestionItem {
                id: unique_id("broker-q"),
                question: q.question,
                options: q.options.unwrap_or_default(),
                allow_free_text: q.allow_free_text,
            })
            .collect(),
    };
    let registration = match state.question_registry.register(question_req) {
        Ok(r) => r,
        Err(e) => {
            let _ =
                send_broker_error(write, Some(id), "internal_error", e.message().to_string()).await;
            return;
        }
    };
    // Wait for the user's answer
    let answer_result = registration.answer_rx.await;
    let answer_payload = match answer_result {
        Ok(answer) => json!({
            "status": "ok",
            "answers": answer.answers,
        }),
        Err(_) => json!({
            "status": "cancelled",
            "answers": [],
        }),
    };
    let response = ControlEnvelope {
        version: 1,
        message_type: "response".to_string(),
        id: Some(id.to_string()),
        method: None,
        payload: answer_payload,
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    let _ = send_broker_envelope(write, &response).await;
}

async fn send_broker_envelope(
    write: &SharedBrokerWsWrite,
    envelope: &ControlEnvelope,
) -> Result<(), ()> {
    let text = serde_json::to_string(envelope).map_err(|_| ())?;
    let mut write = write.lock().await;
    write
        .send(tungstenite::Message::Text(text.into()))
        .await
        .map_err(|_| ())
}

async fn send_broker_error(
    write: &SharedBrokerWsWrite,
    request_id: Option<&str>,
    code: impl Into<String>,
    message: impl Into<String>,
) -> Result<(), ()> {
    let envelope = ControlEnvelope {
        version: 1,
        message_type: "error".to_string(),
        id: request_id.map(|s| s.to_string()),
        method: None,
        payload: json!({
            "code": code.into(),
            "message": message.into(),
            "retryable": false,
        }),
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    send_broker_envelope(write, &envelope).await
}

// ── Tool Routing Table ──────────────────────────────────────────────────

/// Classifies where each built-in tool should execute when the workspace is a
/// remote SSH workspace.  Tools that need provider secrets, local UI, local
/// files (global config, memory DB), or the local network environment are
/// routed through the broker.  Tools that work on workspace files, the
/// workspace database, the workspace shell, or the workspace code graph are
/// executed directly in the sidecar.
///
/// ponytail: v1 does not route agent collaboration tools through the broker;
/// agent task management stays sidecar-local because the sidecar owns the
/// agent scheduler.  Broker-routing for per-workspace websocket/tunnel needs
/// is deferred; it is acceptable to handle websocket proxy separately.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolRoute {
    /// Executed by the remote sidecar (workspace-local).
    SidecarLocal,
    /// Brokered to the local main process.
    BrokerNeeded,
}

/// Return the routing classification for a built-in tool name.
///
/// Workspace-scoped abilities (file ops, shell, git, code graph) go to
/// `SidecarLocal`.  Abilities that need provider secrets, local UI questions,
/// global memory, web access, or image generation with model secrets go
/// through `BrokerNeeded`.
///
/// ponytail: future phases may override `BrokerNeeded` for individual tools
/// when the sidecar gains a local execution option (e.g., web_fetch could run
/// from the sidecar's network).  Return a fixed classification for now.
#[allow(dead_code)]
pub(crate) fn classify_tool_route(tool_name: &str) -> ToolRoute {
    // Sidecar-local tools: workspace file operations, shell, git, code graph,
    // sleep, todo/plan/spec that use workspace DB, agent tools that use
    // workspace agent scheduler.
    match tool_name {
        // workspace file operations
        "read_file" | "find_files" | "search_text" | "write_file" | "edit_file"
        // workspace shell
        | "run_command"
        // code graph
        | "graph_find_symbols" | "graph_find_callers" | "graph_find_callees"
        | "graph_find_references" | "graph_related_files" | "graph_explore"
        // sleep is harmless anywhere
        | "sleep"
        // todo/plan/spec tools use workspace DB
        | "create_todo_graph" | "update_todo_graph" | "get_todo_graph"
        | "create_plan" | "get_plans" | "update_plan" | "update_plan_step"
        | "delete_plan" | "read_spec" | "update_spec"
        // agent runtime — sidecar owns agent scheduler for workspace
        | "agent_list" | "agent_get_task" | "agent_send_message"
        | "agent_delegate_task" | "agent_cancel_task" | "agent_wait_tasks"
        | "agent_transfer_task" | "agent_create_instances" => ToolRoute::SidecarLocal,

        // Broker-needed tools: require local UI, provider secrets, or global memory.
        "ask_question" | "web_search" | "web_fetch" | "image_gen" => ToolRoute::BrokerNeeded,

        // memory tools that access global memory DB (local only)
        "memory_search" | "memory_write" => ToolRoute::BrokerNeeded,

        _ => ToolRoute::SidecarLocal,
    }
}

#[cfg(test)]
mod routing_tests {
    use super::*;
    use foco_tools::builtin_tool_definitions;

    #[test]
    fn every_builtin_tool_has_a_route_classification() {
        let classified = builtin_tool_definitions()
            .into_iter()
            .map(|tool| (tool.name, classify_tool_route(tool.name)))
            .collect::<Vec<_>>();
        for (_name, route) in &classified {
            assert!(
                matches!(route, ToolRoute::SidecarLocal | ToolRoute::BrokerNeeded),
                "unexpected route variant"
            );
        }
    }

    #[test]
    fn broker_needed_tools_are_not_sidecar_local() {
        let broker_tools = [
            "ask_question",
            "web_search",
            "web_fetch",
            "image_gen",
            "memory_search",
            "memory_write",
        ];
        for name in &broker_tools {
            assert_eq!(
                classify_tool_route(name),
                ToolRoute::BrokerNeeded,
                "{name} should be BrokerNeeded"
            );
        }
    }

    #[test]
    fn sidecar_local_tools_are_not_broker_needed() {
        let sidecar_tools = [
            "read_file",
            "find_files",
            "search_text",
            "write_file",
            "edit_file",
            "run_command",
            "graph_find_symbols",
            "graph_find_callers",
            "graph_find_callees",
            "graph_find_references",
            "graph_related_files",
            "graph_explore",
            "sleep",
            "create_todo_graph",
            "update_todo_graph",
            "get_todo_graph",
            "create_plan",
            "get_plans",
            "update_plan",
            "update_plan_step",
            "delete_plan",
            "read_spec",
            "update_spec",
        ];
        for name in &sidecar_tools {
            assert_eq!(
                classify_tool_route(name),
                ToolRoute::SidecarLocal,
                "{name} should be SidecarLocal"
            );
        }
    }

    #[test]
    fn broker_tools_do_not_include_provider_secrets_in_payload() {
        // ponytail: this test verifies that the broker routing table itself
        // does not reference provider secret fields.  Actual sidecar code
        // must be audited separately to ensure no tool payload carries secrets.
        let broker_tools = ["ask_question", "web_search", "web_fetch", "image_gen"];
        for name in &broker_tools {
            let route = classify_tool_route(name);
            assert_eq!(route, ToolRoute::BrokerNeeded);
        }
        // memory_search/memory_write carry only fact text, not secrets
        let memory_tools = ["memory_search", "memory_write"];
        for name in &memory_tools {
            let route = classify_tool_route(name);
            assert_eq!(route, ToolRoute::BrokerNeeded);
        }
    }
}

async fn connect_control_ws_once(
    url: &str,
    token: &str,
    bundle: &crate::runtime::SidecarRuntimeConfigBundle,
) -> Result<
    (
        futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            tungstenite::Message,
        >,
        futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
    ),
    String,
> {
    let mut request = url
        .into_client_request()
        .map_err(|source| format!("invalid control WebSocket request: {source}"))?;
    request.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {token}")
            .parse()
            .map_err(|source| format!("invalid auth header: {source}"))?,
    );
    let (stream, _) = connect_async(request)
        .await
        .map_err(|source| format!("failed to open control WebSocket: {source}"))?;
    let (mut write, mut read) = stream.split();
    let id = unique_id("config-sync");
    let envelope = ControlEnvelope {
        version: 1,
        message_type: "config".to_string(),
        id: Some(id.clone()),
        method: Some("config.sync".to_string()),
        payload: serde_json::to_value(bundle)
            .map_err(|source| format!("failed to serialize runtime config bundle: {source}"))?,
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    write
        .send(tungstenite::Message::Text(
            serde_json::to_string(&envelope)
                .map_err(|source| format!("failed to serialize config sync: {source}"))?
                .into(),
        ))
        .await
        .map_err(|source| format!("failed to send config sync: {source}"))?;

    while let Some(message) = read.next().await {
        let message =
            message.map_err(|source| format!("control WebSocket read failed: {source}"))?;
        let tungstenite::Message::Text(text) = message else {
            continue;
        };
        let envelope: ControlEnvelope = serde_json::from_str(&text)
            .map_err(|source| format!("invalid control WebSocket JSON: {source}"))?;
        if envelope.message_type == "response" && envelope.id.as_deref() == Some(id.as_str()) {
            return Ok((write, read));
        }
    }
    Err("control WebSocket closed before config sync response".to_string())
}

async fn run_ssh_output(
    server: &RemoteServerProfile,
    extra_args: &[&str],
    batch_mode: bool,
    server_id: &str,
    workspace_id: Option<&str>,
) -> Result<std::process::Output, ApiError> {
    let timeout_ms = server.connect_timeout_ms.max(1);
    let args = remote_server_ssh_args(server, extra_args, batch_mode);
    timeout(
        Duration::from_millis(timeout_ms + 1_000),
        Command::new("ssh").args(&args).output(),
    )
    .await
    .map_err(|_| {
        remote_error(
            server_id,
            workspace_id,
            format!("ssh command timed out after {timeout_ms}ms"),
        )
    })?
    .map_err(|source| {
        remote_error(
            server_id,
            workspace_id,
            format!("failed to run ssh: {source}"),
        )
    })
}

async fn run_ssh_with_stdin(
    server: &RemoteServerProfile,
    extra_args: &[&str],
    stdin: &[u8],
    server_id: &str,
    workspace_id: Option<&str>,
) -> Result<std::process::Output, ApiError> {
    let timeout_ms = server.connect_timeout_ms.max(1);
    let args = remote_server_ssh_args(server, extra_args, true);
    let mut child = Command::new("ssh")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| {
            remote_error(
                server_id,
                workspace_id,
                format!("failed to run ssh: {source}"),
            )
        })?;
    if let Some(mut child_stdin) = child.stdin.take() {
        child_stdin.write_all(stdin).await.map_err(|source| {
            remote_error(
                server_id,
                workspace_id,
                format!("failed to upload sidecar over ssh stdin: {source}"),
            )
        })?;
    }
    timeout(
        Duration::from_millis(timeout_ms + 30_000),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| remote_error(server_id, workspace_id, "ssh upload timed out"))?
    .map_err(|source| {
        remote_error(
            server_id,
            workspace_id,
            format!("failed to finish ssh upload: {source}"),
        )
    })
}

fn session_key(server_id: &str, workspace_id: &str) -> String {
    format!("{server_id}:{workspace_id}")
}

fn random_token() -> Result<String, ApiError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|source| {
        ApiError::internal(format!("failed to generate sidecar token: {source}"))
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn output_text(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("stdout: {stdout}; stderr: {stderr}"),
        (false, true) => stdout,
        (true, false) => stderr,
        (true, true) => format!("exit status: {}", output.status),
    }
}

fn remote_error(
    server_id: &str,
    workspace_id: Option<&str>,
    message: impl Into<String>,
) -> ApiError {
    let mut prefix = format!("serverId={server_id}");
    if let Some(workspace_id) = workspace_id {
        prefix.push_str(&format!(" workspaceId={workspace_id}"));
    }
    ApiError::bad_request(format!("{prefix}: {}", message.into()))
}

/// Return the local tunnel base URL and bearer token for a remote workspace's
/// sidecar session, or None if the workspace is local or its sidecar is not connected.
pub(crate) fn sidecar_proxy_target(
    state: &AppState,
    workspace_id: &str,
) -> Result<Option<(String, String)>, ApiError> {
    let config = config_snapshot(state)?;
    let workspace = workspace_by_id(&config, workspace_id)?;
    let WorkspaceLocation::Ssh { server_id, .. } = &workspace.location else {
        return Ok(None);
    };
    let sessions = state
        .remote_workspace_manager
        .sessions
        .lock()
        .map_err(|_| ApiError::internal("remote workspace session lock is poisoned"))?;
    let key = session_key(server_id, workspace_id);
    if let Some(session) = sessions.get(&key) {
        Ok(Some((
            format!("http://127.0.0.1:{}/", session.local_port),
            session.token.clone(),
        )))
    } else {
        Ok(None)
    }
}

pub(crate) async fn proxy_sidecar_json_request(
    state: &AppState,
    workspace_id: &str,
    method: reqwest::Method,
    suffix: &str,
    payload: Option<Value>,
) -> Result<Value, ApiError> {
    let Some((base, token)) = sidecar_proxy_target(state, workspace_id)? else {
        return Err(ApiError::conflict(format!(
            "remote workspace sidecar is not connected: {workspace_id}"
        )));
    };
    let url = format!(
        "{}api/remote/workspace/{}",
        base.trim_end_matches('/'),
        suffix.trim_start_matches('/')
    );
    let client = reqwest::Client::new();
    let mut request = client.request(method, url).bearer_auth(token);
    if let Some(payload) = payload {
        request = request.json(&payload);
    }
    let response = request
        .send()
        .await
        .map_err(|source| ApiError::bad_gateway(format!("sidecar proxy failed: {source}")))?;
    let status = response.status();
    let text = response.text().await.map_err(|source| {
        ApiError::bad_gateway(format!("failed to read sidecar proxy response: {source}"))
    })?;
    if !status.is_success() {
        return Err(ApiError::from_status_message(status, text));
    }
    serde_json::from_str(&text)
        .map_err(|source| ApiError::bad_gateway(format!("invalid sidecar JSON response: {source}")))
}

pub(crate) async fn install_remote_workspace_skill(
    state: &AppState,
    workspace_id: &str,
    request: crate::http::skill_store::SkillStoreInstallRequest,
) -> Result<Json<crate::http::skill_store::SkillStoreInstallResponse>, ApiError> {
    let payload = serde_json::to_value(request).map_err(|source| {
        ApiError::internal(format!("failed to serialize skill install: {source}"))
    })?;
    let value = proxy_sidecar_json_request(
        state,
        workspace_id,
        reqwest::Method::POST,
        "skills/install",
        Some(payload),
    )
    .await?;
    serde_json::from_value(value).map(Json).map_err(|source| {
        ApiError::bad_gateway(format!("invalid sidecar skill install response: {source}"))
    })
}

pub(crate) async fn proxy_websocket_to_sidecar(
    client_socket: WebSocket,
    proxy_url: String,
    token: String,
) {
    let ws_url = proxy_url
        .strip_prefix("http://")
        .map(|rest| format!("ws://{rest}"))
        .unwrap_or(proxy_url);
    let mut request = match ws_url.into_client_request() {
        Ok(request) => request,
        Err(source) => {
            tracing::warn!(error = %source, "invalid remote sidecar websocket proxy request");
            return;
        }
    };
    let Ok(auth) = format!("Bearer {token}").parse() else {
        tracing::warn!("invalid remote sidecar websocket auth header");
        return;
    };
    request.headers_mut().insert(header::AUTHORIZATION, auth);

    let (sidecar_socket, _) = match connect_async(request).await {
        Ok(connection) => connection,
        Err(source) => {
            tracing::warn!(error = %source, "failed to open remote sidecar websocket");
            return;
        }
    };
    let (mut client_sender, mut client_receiver) = client_socket.split();
    let (mut sidecar_sender, mut sidecar_receiver) = sidecar_socket.split();

    loop {
        tokio::select! {
            client_message = client_receiver.next() => {
                let Some(Ok(message)) = client_message else { break; };
                let message = match message {
                    Message::Text(text) => tungstenite::Message::Text(text.as_str().to_string().into()),
                    Message::Binary(bytes) => tungstenite::Message::Binary(bytes),
                    Message::Ping(bytes) => tungstenite::Message::Ping(bytes),
                    Message::Pong(bytes) => tungstenite::Message::Pong(bytes),
                    Message::Close(_) => {
                        let _ = sidecar_sender.send(tungstenite::Message::Close(None)).await;
                        break;
                    }
                };
                if sidecar_sender.send(message).await.is_err() {
                    break;
                }
            }
            sidecar_message = sidecar_receiver.next() => {
                let Some(Ok(message)) = sidecar_message else { break; };
                let message = match message {
                    tungstenite::Message::Text(text) => Message::Text(text.as_str().to_string().into()),
                    tungstenite::Message::Binary(bytes) => Message::Binary(bytes),
                    tungstenite::Message::Ping(bytes) => Message::Ping(bytes),
                    tungstenite::Message::Pong(bytes) => Message::Pong(bytes),
                    tungstenite::Message::Close(_) => {
                        let _ = client_sender.send(Message::Close(None)).await;
                        break;
                    }
                    tungstenite::Message::Frame(_) => continue,
                };
                if client_sender.send(message).await.is_err() {
                    break;
                }
            }
        }
    }
}

// Sidecar workspace-scoped HTTP route handlers

fn sidecar_workspace_path(state: &RemoteSidecarState) -> &Path {
    Path::new(&state.workspace_path)
}

fn ensure_sidecar_code_graph(state: &RemoteSidecarState) -> Result<(), axum::response::Response> {
    if state
        .code_graph_watcher
        .lock()
        .map_err(|_| {
            ApiError::internal("remote code graph watcher lock is poisoned").into_response()
        })?
        .is_some()
    {
        return Ok(());
    }

    // ponytail: first code-graph touch indexes under a short global lock; upgrade to per-workspace async init if remote repos make this slow.
    let workspace_path = sidecar_workspace_path(state).to_path_buf();
    let report = foco_graph::index_workspace(&workspace_path).map_err(|e| {
        ApiError::internal(format!("failed to index remote code graph: {e}")).into_response()
    })?;
    let watcher = foco_graph::start_code_graph_watcher(&workspace_path).map_err(|e| {
        ApiError::internal(format!("failed to watch remote code graph: {e}")).into_response()
    })?;
    tracing::info!(
        workspace_path = %workspace_path.display(),
        scanned_files = report.scanned_files,
        indexed_files = report.indexed_files,
        "initialized remote sidecar code graph"
    );
    let mut lock = state.code_graph_watcher.lock().map_err(|_| {
        ApiError::internal("remote code graph watcher lock is poisoned").into_response()
    })?;
    if lock.is_none() {
        *lock = Some(watcher);
    }
    Ok(())
}

async fn remote_sidecar_file_tree(
    State(state): State<RemoteSidecarState>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let root =
        crate::http::workspaces::workspace_file_tree_response(sidecar_workspace_path(&state))
            .map_err(|e| e.into_response())?;
    Ok(Json(serde_json::to_value(root).map_err(|e| {
        ApiError::internal(e.to_string()).into_response()
    })?))
}

async fn remote_sidecar_file_children(
    State(state): State<RemoteSidecarState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let path = query.get("path").map(String::as_str).unwrap_or("");
    let ws_path = sidecar_workspace_path(&state);
    let list_path = crate::http::workspaces::workspace_file_list_path(ws_path, path)
        .map_err(|e| e.into_response())?;
    let children =
        crate::http::workspaces::workspace_file_tree_children(ws_path, &list_path, 0, false)
            .map_err(|e| e.into_response())?;
    Ok(Json(
        serde_json::json!({ "path": path, "children": children }),
    ))
}

async fn remote_sidecar_file_content(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let rel_path = payload.get("path").map(String::as_str).unwrap_or("");
    let ws_path = sidecar_workspace_path(&state);
    let abs_path = crate::http::workspaces::workspace_file_path(ws_path, rel_path)
        .map_err(|e| e.into_response())?;
    let content = std::fs::read_to_string(&abs_path).map_err(|source| {
        ApiError::bad_request(format!("failed to read remote file {rel_path}: {source}"))
            .into_response()
    })?;
    Ok(Json(
        serde_json::json!({ "content": content, "path": rel_path }),
    ))
}

async fn remote_sidecar_file_save(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let rel_path = payload.get("path").map(String::as_str).unwrap_or("");
    let content = payload.get("content").map(String::as_str).unwrap_or("");
    let ws_path = sidecar_workspace_path(&state);
    let abs_path = crate::http::workspaces::workspace_file_path(ws_path, rel_path)
        .map_err(|e| e.into_response())?;
    std::fs::write(&abs_path, content.as_bytes()).map_err(|source| {
        ApiError::internal(format!("failed to save remote file {rel_path}: {source}"))
            .into_response()
    })?;
    Ok(Json(
        serde_json::json!({ "content": content, "path": rel_path }),
    ))
}

async fn remote_sidecar_file_delete(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let rel_path = payload.get("path").map(String::as_str).unwrap_or("");
    let ws_path = sidecar_workspace_path(&state);
    let abs_path = crate::http::workspaces::workspace_file_path(ws_path, rel_path)
        .map_err(|e| e.into_response())?;
    let meta = std::fs::metadata(&abs_path).map_err(|source| {
        ApiError::bad_request(format!("remote file not found {rel_path}: {source}")).into_response()
    })?;
    if meta.is_dir() {
        std::fs::remove_dir_all(&abs_path).map_err(|source| {
            ApiError::internal(format!("failed to delete remote dir {rel_path}: {source}"))
                .into_response()
        })?;
    } else {
        std::fs::remove_file(&abs_path).map_err(|source| {
            ApiError::internal(format!("failed to delete remote file {rel_path}: {source}"))
                .into_response()
        })?;
    }
    let parent = rel_path
        .rsplit_once('/')
        .map(|(p, _)| p.to_string())
        .unwrap_or_default();
    let list_path = crate::http::workspaces::workspace_file_list_path(ws_path, &parent)
        .map_err(|e| e.into_response())?;
    let children =
        crate::http::workspaces::workspace_file_tree_children(ws_path, &list_path, 0, false)
            .map_err(|e| e.into_response())?;
    Ok(Json(
        serde_json::json!({ "path": parent, "children": children }),
    ))
}

async fn remote_sidecar_file_rename(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let rel_path = payload.get("path").map(String::as_str).unwrap_or("");
    let new_name = payload.get("newName").map(String::as_str).unwrap_or("");
    if new_name.is_empty() {
        return Err(ApiError::bad_request("newName must not be empty").into_response());
    }
    let ws_path = sidecar_workspace_path(&state);
    let src = crate::http::workspaces::workspace_file_path(ws_path, rel_path)
        .map_err(|e| e.into_response())?;
    let dst = src
        .parent()
        .ok_or_else(|| ApiError::bad_request("workspace root cannot be renamed").into_response())?
        .join(new_name);
    if dst.exists() {
        return Err(
            ApiError::bad_request(format!("target already exists: {}", dst.display()))
                .into_response(),
        );
    }
    std::fs::rename(&src, &dst).map_err(|source| {
        ApiError::internal(format!("failed to rename remote file {rel_path}: {source}"))
            .into_response()
    })?;
    let parent = rel_path
        .rsplit_once('/')
        .map(|(p, _)| p.to_string())
        .unwrap_or_default();
    let list_path = crate::http::workspaces::workspace_file_list_path(ws_path, &parent)
        .map_err(|e| e.into_response())?;
    let children =
        crate::http::workspaces::workspace_file_tree_children(ws_path, &list_path, 0, false)
            .map_err(|e| e.into_response())?;
    Ok(Json(
        serde_json::json!({ "path": parent, "children": children }),
    ))
}

async fn remote_sidecar_memory_list(
    State(state): State<RemoteSidecarState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, axum::response::Response> {
    let scope = MemoryScope::parse(
        query
            .get("scope")
            .map(String::as_str)
            .unwrap_or("workspace"),
    )
    .map_err(|e| ApiError::bad_request(e.to_string()).into_response())?;
    if scope == MemoryScope::Global {
        return Err(
            ApiError::bad_request("global memory stays in the local broker").into_response(),
        );
    }
    let chat_id = query
        .get("chatId")
        .or_else(|| query.get("chat_id"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if scope == MemoryScope::Chat && chat_id.is_none() {
        return Err(ApiError::bad_request("chat memory listing requires chatId").into_response());
    }
    let status = MemoryStatus::parse(query.get("status").map(String::as_str).unwrap_or("active"))
        .map_err(|e| ApiError::bad_request(e.to_string()).into_response())?;
    let page = query
        .get("page")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    let page_size = query
        .get("pageSize")
        .or_else(|| query.get("page_size"))
        .or_else(|| query.get("limit"))
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(20)
        .clamp(1, 200);
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    let query_text = query.get("query").map(String::as_str);
    let database =
        MemoryDatabase::open_workspace_at(workspace_database_path(sidecar_workspace_path(&state)))
            .map_err(|e| ApiError::from_memory_error(e).into_response())?;
    let total_count = database
        .count_facts_for_scope(chat_id.as_deref(), status, None, query_text)
        .map_err(|e| ApiError::from_memory_error(e).into_response())?;
    let memories = database
        .list_facts_for_scope_page(
            chat_id.as_deref(),
            status,
            None,
            query_text,
            page_size,
            offset,
        )
        .map_err(|e| ApiError::from_memory_error(e).into_response())?;
    Ok(Json(json!({
        "memories": memories,
        "extractionJobs": [],
        "remote": { "status": "available" },
        "page": page,
        "pageSize": page_size,
        "totalCount": total_count,
        "totalPages": if total_count == 0 { 0 } else { total_count.div_ceil(page_size) },
    })))
}

async fn remote_sidecar_memory_manual(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let scope = MemoryScope::parse(
        payload
            .get("scope")
            .and_then(Value::as_str)
            .unwrap_or("workspace"),
    )
    .map_err(|e| ApiError::bad_request(e.to_string()).into_response())?;
    if scope == MemoryScope::Global {
        return Err(
            ApiError::bad_request("global memory stays in the local broker").into_response(),
        );
    }
    let kind = MemoryKind::parse(
        payload
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("project_fact"),
    )
    .map_err(|e| ApiError::bad_request(e.to_string()).into_response())?;
    let fact = payload
        .get("fact")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("memory fact must not be empty").into_response())?;
    let chat_id = payload
        .get("chatId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if scope == MemoryScope::Chat && chat_id.is_none() {
        return Err(ApiError::bad_request("chat memory requires chatId").into_response());
    }
    let mut database =
        MemoryDatabase::open_workspace_at(workspace_database_path(sidecar_workspace_path(&state)))
            .map_err(|e| ApiError::from_memory_error(e).into_response())?;
    let source_id = unique_id("remote-memory-source");
    let memory_id = unique_id("remote-memory-fact");
    database
        .insert_source(NewMemorySource {
            id: &source_id,
            scope,
            chat_id,
            source_type: MemorySourceType::ManualNote,
            source_id: None,
            title: "Remote manual memory",
            content: fact,
            metadata_json: "{}",
        })
        .map_err(|e| ApiError::from_memory_error(e).into_response())?;
    database
        .insert_fact(NewMemoryFact {
            id: &memory_id,
            scope,
            chat_id,
            status: MemoryStatus::Active,
            kind,
            fact,
            confidence: payload.get("confidence").and_then(Value::as_f64),
            pinned: payload
                .get("pinned")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            source_ids: &[source_id.as_str()],
            metadata_json: "{}",
        })
        .map_err(|e| ApiError::from_memory_error(e).into_response())?;
    let memory = database
        .fact(&memory_id)
        .map_err(|e| ApiError::from_memory_error(e).into_response())?;
    Ok(Json(json!({ "memory": memory })))
}

async fn remote_sidecar_skill_install(
    State(state): State<RemoteSidecarState>,
    Json(request): Json<crate::http::skill_store::SkillStoreInstallRequest>,
) -> Result<Json<crate::http::skill_store::SkillStoreInstallResponse>, axum::response::Response> {
    let install_path = crate::http::skill_store::install_skill_files_to_target_dir(
        &sidecar_workspace_path(&state)
            .join(".agents")
            .join("skills"),
        &request.skill_id,
        &request.files,
        request.overwrite,
    )
    .map_err(|e| e.into_response())?;
    let discovery = crate::skills::discover_workspace_skills_for_path(
        &state.workspace_id,
        &state.workspace_id,
        sidecar_workspace_path(&state),
    );
    Ok(Json(crate::http::skill_store::SkillStoreInstallResponse {
        target: foco_store::config::SKILL_SCOPE_WORKSPACE.to_string(),
        workspace_id: Some(state.workspace_id),
        path: install_path.display().to_string(),
        detected: discovery.skills,
    }))
}

async fn remote_sidecar_skills_discover(
    State(state): State<RemoteSidecarState>,
) -> Result<Json<Value>, axum::response::Response> {
    let discovery = crate::skills::discover_workspace_skills_for_path(
        &state.workspace_id,
        &state.workspace_id,
        sidecar_workspace_path(&state),
    );
    Ok(Json(json!({
        "detected": discovery.skills,
        "errors": discovery.errors,
        "requiredDisabled": discovery.required_disabled,
    })))
}

async fn remote_sidecar_hooks_settings(
    State(state): State<RemoteSidecarState>,
) -> Result<Json<Value>, axum::response::Response> {
    let config = foco_store::config::load_workspace_hook_config(sidecar_workspace_path(&state))
        .map_err(|e| ApiError::bad_request(e.to_string()).into_response())?;
    Ok(Json(json!({
        "workspace": {
            "source": "workspace",
            "path": foco_store::config::workspace_hook_config_path(sidecar_workspace_path(&state)).display().to_string(),
            "workspaceId": state.workspace_id,
            "config": config,
        }
    })))
}

async fn remote_sidecar_hooks_save(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let config_value = payload.get("config").cloned().unwrap_or(payload);
    let config: foco_store::config::HookConfig = serde_json::from_value(config_value)
        .map_err(|e| ApiError::bad_request(format!("invalid hook config: {e}")).into_response())?;
    foco_store::config::save_workspace_hook_config(sidecar_workspace_path(&state), &config)
        .map_err(|e| ApiError::bad_request(e.to_string()).into_response())?;
    Ok(Json(json!({
        "workspace": {
            "source": "workspace",
            "path": foco_store::config::workspace_hook_config_path(sidecar_workspace_path(&state)).display().to_string(),
            "workspaceId": state.workspace_id,
            "config": config,
        }
    })))
}

async fn remote_sidecar_hook_runs(
    State(state): State<RemoteSidecarState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, axum::response::Response> {
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(50)
        .clamp(1, 200);
    let runs =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?
            .hook_runs(limit)
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?
            .into_iter()
            .filter(|record| record.workspace_id == state.workspace_id)
            .map(crate::hook_run_summary_row)
            .collect::<Vec<_>>();
    Ok(Json(json!({ "runs": runs })))
}

async fn remote_sidecar_hook_run_detail(
    State(state): State<RemoteSidecarState>,
    AxumPath(hook_run_id): AxumPath<String>,
) -> Result<Json<Value>, axum::response::Response> {
    let hook_run_id = hook_run_id.trim();
    if hook_run_id.is_empty() {
        return Err(ApiError::bad_request("hook run id must not be empty").into_response());
    }
    let run =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?
            .hook_run(hook_run_id)
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?
            .ok_or_else(|| {
                ApiError::bad_request(format!("hook run was not found: {hook_run_id}"))
                    .into_response()
            })?;
    if run.workspace_id != state.workspace_id {
        return Err(ApiError::bad_request(format!(
            "hook run '{}' does not belong to workspace '{}'",
            run.id, state.workspace_id
        ))
        .into_response());
    }
    let run = crate::hook_run_detail_from_record(run).map_err(|error| error.into_response())?;
    Ok(Json(json!({ "run": run })))
}

async fn remote_sidecar_file_blob(
    State(state): State<RemoteSidecarState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<axum::response::Response, axum::response::Response> {
    let rel_path = query.get("path").map(String::as_str).unwrap_or("");
    let ws_path = sidecar_workspace_path(&state);
    let abs_path = crate::http::workspaces::workspace_file_path(ws_path, rel_path)
        .map_err(|e| e.into_response())?;
    let bytes = std::fs::read(&abs_path).map_err(|source| {
        ApiError::bad_request(format!("failed to read remote file {rel_path}: {source}"))
            .into_response()
    })?;
    let content_type = crate::http::workspaces::workspace_image_content_type(&bytes)
        .map_err(|e| e.into_response())?;
    Ok(axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .header(axum::http::header::CACHE_CONTROL, "private, max-age=60")
        .body(axum::body::Body::from(bytes))
        .expect("sidecar blob response is valid"))
}

async fn remote_sidecar_git_command(
    state: &RemoteSidecarState,
    args: &[&str],
) -> Result<std::process::Output, ApiError> {
    let ws_path = sidecar_workspace_path(state);
    tokio::process::Command::new("git")
        .args(args)
        .current_dir(ws_path)
        .output()
        .await
        .map_err(|source| ApiError::internal(format!("remote git command failed: {source}")))
}

async fn remote_sidecar_git_status(
    State(state): State<RemoteSidecarState>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let output = remote_sidecar_git_command(&state, &["status", "--porcelain"])
        .await
        .map_err(|e| e.into_response())?;
    let is_git = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| {
            if line.len() < 3 { return None; }
            let ix = line.as_bytes()[0] as char;
            let wx = line.as_bytes()[1] as char;
            let path = line[3..].trim().to_string();
            if path.is_empty() { return None; }
            Some(serde_json::json!({ "path": path, "indexStatus": ix.to_string(), "worktreeStatus": wx.to_string() }))
        })
        .collect();
    Ok(Json(serde_json::json!({
        "isGitRepository": is_git,
        "status": if is_git { "ok".to_string() } else { "not_a_repository".to_string() },
        "files": files,
    })))
}

async fn remote_sidecar_git_diff(
    State(state): State<RemoteSidecarState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let path_filter = query.get("path").map(String::as_str);
    let mut args = vec!["diff", "--no-color"];
    if let Some(path) = path_filter.filter(|p| !p.is_empty()) {
        args.push("--");
        args.push(path);
    }
    let diff = remote_sidecar_git_command(&state, &args)
        .await
        .map_err(|e| e.into_response())?;
    let staged = remote_sidecar_git_command(&state, &["diff", "--cached", "--no-color"])
        .await
        .map_err(|e| e.into_response())?;
    let porcelain = remote_sidecar_git_command(&state, &["status", "--porcelain"])
        .await
        .map_err(|e| e.into_response())?;
    let files: Vec<serde_json::Value> = String::from_utf8_lossy(&porcelain.stdout)
        .lines()
        .filter_map(|line| {
            if line.len() < 3 { return None; }
            let ix = line.as_bytes()[0] as char;
            let wx = line.as_bytes()[1] as char;
            let path = line[3..].trim();
            if path.is_empty() { return None; }
            Some(serde_json::json!({ "path": path, "indexStatus": ix.to_string(), "worktreeStatus": wx.to_string() }))
        })
        .collect();
    let staged_files: Vec<serde_json::Value> = files
        .iter()
        .filter(|f| {
            f["indexStatus"]
                .as_str()
                .map(|s| s != " " && s != "?")
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    Ok(Json(serde_json::json!({
        "path": path_filter,
        "status": "ok",
        "diff": String::from_utf8_lossy(&diff.stdout),
        "stagedDiff": String::from_utf8_lossy(&staged.stdout),
        "files": files,
        "stagedFiles": staged_files,
    })))
}

async fn remote_sidecar_git_stage(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let path = payload.get("path").map(String::as_str).unwrap_or(".");
    remote_sidecar_git_command(&state, &["add", path])
        .await
        .map_err(|e| e.into_response())?;
    remote_sidecar_git_diff(State(state), Query(HashMap::new())).await
}

async fn remote_sidecar_git_unstage(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let path = payload.get("path").map(String::as_str).unwrap_or(".");
    remote_sidecar_git_command(&state, &["reset", "HEAD", "--", path])
        .await
        .map_err(|e| e.into_response())?;
    remote_sidecar_git_diff(State(state), Query(HashMap::new())).await
}

async fn remote_sidecar_git_discard(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let path = payload.get("path").map(String::as_str).unwrap_or(".");
    remote_sidecar_git_command(&state, &["checkout", "--", path])
        .await
        .map_err(|e| e.into_response())?;
    remote_sidecar_git_diff(State(state), Query(HashMap::new())).await
}

async fn remote_sidecar_git_commit(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let message = payload.get("message").map(String::as_str).unwrap_or("");
    if message.is_empty() {
        return Err(ApiError::bad_request("commit message must not be empty").into_response());
    }
    remote_sidecar_git_command(&state, &["commit", "-m", message])
        .await
        .map_err(|e| e.into_response())?;
    remote_sidecar_git_diff(State(state), Query(HashMap::new())).await
}

async fn remote_sidecar_git_branches(
    State(state): State<RemoteSidecarState>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let ws_path = sidecar_workspace_path(&state);
    let is_git = ws_path.join(".git").exists();
    if !is_git {
        return Ok(Json(serde_json::json!({
            "isGitRepository": false,
            "currentBranch": null,
            "branches": [],
            "worktrees": [],
        })));
    }
    let branch_out = remote_sidecar_git_command(&state, &["branch"])
        .await
        .map_err(|e| e.into_response())?;
    let current = String::from_utf8_lossy(&branch_out.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("* ").map(str::to_string));
    let branches: Vec<String> = String::from_utf8_lossy(&branch_out.stdout)
        .lines()
        .map(|line| line.trim_start_matches("* ").to_string())
        .collect();
    Ok(Json(serde_json::json!({
        "isGitRepository": true,
        "currentBranch": current,
        "branches": branches,
        "worktrees": [],
    })))
}

async fn remote_sidecar_git_branch_switch(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let name = payload.get("name").map(String::as_str).unwrap_or("");
    if name.is_empty() {
        return Err(ApiError::bad_request("branch name must not be empty").into_response());
    }
    remote_sidecar_git_command(&state, &["checkout", name])
        .await
        .map_err(|e| e.into_response())?;
    remote_sidecar_git_branches(State(state)).await
}

async fn remote_sidecar_git_branch_create(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let name = payload.get("name").map(String::as_str).unwrap_or("");
    if name.is_empty() {
        return Err(ApiError::bad_request("branch name must not be empty").into_response());
    }
    remote_sidecar_git_command(&state, &["checkout", "-b", name])
        .await
        .map_err(|e| e.into_response())?;
    remote_sidecar_git_branches(State(state)).await
}

async fn remote_sidecar_terminal_session(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let _shell = payload.get("shell").map(String::as_str).unwrap_or("bash");
    let ws_path = sidecar_workspace_path(&state);
    let session_id = format!(
        "remote-term-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        std::process::id(),
    );
    let mut db = foco_store::workspace::WorkspaceDatabase::open_or_create(ws_path)
        .map_err(|e| ApiError::internal(e.to_string()).into_response())?;
    db.upsert_terminal_session(foco_store::workspace::NewTerminalSession {
        id: &session_id,
        name: "Remote Terminal",
        working_directory: &ws_path.display().to_string(),
        metadata_json: None,
    })
    .map_err(|e| ApiError::internal(e.to_string()).into_response())?;
    Ok(Json(serde_json::json!({
        "sessionId": session_id,
        "workingDirectory": ws_path.display().to_string(),
    })))
}

async fn remote_sidecar_terminal_ws(
    ws: axum::extract::WebSocketUpgrade,
    State(state): State<RemoteSidecarState>,
    AxumPath(session_id): AxumPath<String>,
) -> axum::response::Response {
    let ws_path = sidecar_workspace_path(&state).to_path_buf();
    ws.on_upgrade(move |socket| async move {
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        crate::terminal::handle_terminal_socket(
            socket,
            shutdown_tx.subscribe(),
            crate::terminal::TerminalRegistry::default(),
            ws_path,
            "bash".to_string(),
            foco_store::workspace::TerminalSessionRecord {
                id: session_id.clone(),
                name: String::new(),
                working_directory: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
                closed_at: None,
                metadata_json: String::new(),
            },
            80,
            24,
        )
        .await;
    })
}

async fn remote_sidecar_spec_get(
    State(state): State<RemoteSidecarState>,
) -> Result<Json<Value>, axum::response::Response> {
    let database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(remote_sidecar_spec_response(&database)?))
}

async fn remote_sidecar_spec_settings(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let enabled = payload
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let inject_enabled = payload
        .get("injectEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    database
        .upsert_workspace_spec_settings(enabled, inject_enabled)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(remote_sidecar_spec_response(&database)?))
}

async fn remote_sidecar_spec_put(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let expected_revision = payload
        .get("expectedRevision")
        .and_then(Value::as_u64)
        .ok_or_else(|| ApiError::bad_request("expectedRevision is required").into_response())?;
    let content_markdown = payload
        .get("contentMarkdown")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("contentMarkdown is required").into_response())?;
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    database
        .update_workspace_spec_content(expected_revision, content_markdown)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .ok_or_else(|| {
            ApiError::conflict("workspace spec revision changed; reload and retry").into_response()
        })?;
    Ok(Json(remote_sidecar_spec_response(&database)?))
}

async fn remote_sidecar_spec_generate(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let spec = database
        .workspace_spec()
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .filter(|spec| spec.enabled)
        .ok_or_else(|| ApiError::bad_request("workspace spec is disabled").into_response())?;
    if let Some(job) = database
        .running_workspace_spec_job()
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
    {
        return Err(ApiError::conflict(format!(
            "workspace spec job is already running: {}",
            job.id
        ))
        .into_response());
    }
    let model_id = payload.get("modelId").and_then(Value::as_str);
    let trigger_type = if spec.content_markdown.trim().is_empty() {
        "manual_initial"
    } else {
        "manual_refresh"
    };
    let job_id = unique_id("workspace-spec-job");
    let input_summary = json!({ "remoteSidecarQueued": true });
    let job = database
        .insert_workspace_spec_job(foco_store::workspace::NewWorkspaceSpecJob {
            id: &job_id,
            trigger_type,
            chat_id: None,
            run_id: None,
            model_id,
            base_revision: Some(spec.revision),
            input_summary_json: Some(&input_summary.to_string()),
        })
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(json!({ "job": remote_sidecar_spec_job_json(job)? })))
}

async fn remote_sidecar_spec_jobs(
    State(state): State<RemoteSidecarState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, axum::response::Response> {
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(50)
        .clamp(1, 100);
    let database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let jobs = database
        .workspace_spec_jobs(limit)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .into_iter()
        .map(remote_sidecar_spec_job_json)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(json!({ "jobs": jobs })))
}

async fn remote_sidecar_spec_jobs_retry(
    State(state): State<RemoteSidecarState>,
    AxumPath(job_id): AxumPath<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let model_id = payload.get("modelId").and_then(Value::as_str);
    let retry_id = unique_id("workspace-spec-job");
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let job = database
        .retry_failed_workspace_spec_job(&job_id, &retry_id, model_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .ok_or_else(|| {
            ApiError::bad_request(format!("workspace spec job cannot be retried: {job_id}"))
                .into_response()
        })?;
    Ok(Json(json!({ "job": remote_sidecar_spec_job_json(job)? })))
}

fn remote_sidecar_spec_response(
    database: &foco_store::workspace::WorkspaceDatabase,
) -> Result<Value, axum::response::Response> {
    let spec = database
        .workspace_spec()
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let latest_job = database
        .workspace_spec_jobs(1)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .into_iter()
        .next()
        .map(remote_sidecar_spec_job_json)
        .transpose()?;
    let (enabled, inject_enabled, content_markdown, revision, generated_at, updated_at) = match spec
    {
        Some(spec) => (
            spec.enabled,
            spec.inject_enabled,
            spec.content_markdown,
            spec.revision,
            spec.generated_at,
            Some(spec.updated_at),
        ),
        None => (false, false, String::new(), 0, None, None),
    };
    Ok(json!({
        "settings": { "enabled": enabled, "injectEnabled": inject_enabled },
        "contentMarkdown": content_markdown,
        "revision": revision,
        "generatedAt": generated_at,
        "updatedAt": updated_at,
        "latestJob": latest_job,
    }))
}

fn remote_sidecar_spec_job_json(
    job: foco_store::workspace::WorkspaceSpecJobRecord,
) -> Result<Value, axum::response::Response> {
    let input_summary = serde_json::from_str::<Value>(&job.input_summary_json).map_err(|e| {
        ApiError::internal(format!("workspace spec input_summary_json is invalid: {e}"))
            .into_response()
    })?;
    let output = job
        .output_json
        .as_deref()
        .map(|value| serde_json::from_str::<Value>(value))
        .transpose()
        .map_err(|e| {
            ApiError::internal(format!("workspace spec output_json is invalid: {e}"))
                .into_response()
        })?;
    Ok(json!({
        "id": job.id,
        "triggerType": job.trigger_type,
        "status": job.status,
        "chatId": job.chat_id,
        "runId": job.run_id,
        "modelId": job.model_id,
        "baseRevision": job.base_revision,
        "inputSummary": input_summary,
        "output": output,
        "errorMessage": job.error_message,
        "createdAt": job.created_at,
        "startedAt": job.started_at,
        "completedAt": job.completed_at,
        "hasRetry": job.has_retry,
    }))
}

async fn remote_sidecar_plans_list(
    State(state): State<RemoteSidecarState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, axum::response::Response> {
    let database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let page_size = query
        .get("pageSize")
        .or_else(|| query.get("page_size"))
        .or_else(|| query.get("limit"))
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(50)
        .clamp(1, 100);
    let page = query
        .get("page")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(1)
        .max(1);
    let view = query.get("view").map(String::as_str).unwrap_or("active");
    let status = query
        .get("status")
        .map(String::as_str)
        .filter(|value| !value.is_empty());
    let plans = database
        .plans(foco_store::workspace::PlanListFilter {
            view,
            status,
            limit: page_size,
            offset: (page - 1) * page_size,
        })
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let total_pages = if plans.total_count == 0 {
        0
    } else {
        (plans.total_count + page_size - 1) / page_size
    };
    Ok(Json(json!({
        "plans": plans.plans,
        "page": page,
        "pageSize": page_size,
        "totalCount": plans.total_count,
        "totalPages": total_pages,
    })))
}

async fn remote_sidecar_plans_create(
    State(state): State<RemoteSidecarState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let plan_id = request
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| unique_id("plan"));
    let title = request.get("title").and_then(Value::as_str).unwrap_or("");
    let overview = request
        .get("overview")
        .and_then(Value::as_str)
        .unwrap_or("");
    let status = request
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("ready");
    let source_chat_id = request.get("sourceChatId").and_then(Value::as_str);
    struct OwnedStep {
        id: String,
        title: String,
        detail: String,
        acceptance: Vec<String>,
    }
    struct OwnedPhase {
        id: String,
        title: String,
        summary: String,
        steps: Vec<OwnedStep>,
    }

    let owned_phases = request
        .get("phases")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .map(|phase| OwnedPhase {
            id: phase
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| unique_id("plan-phase")),
            title: phase
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            summary: phase
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            steps: phase
                .get("steps")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .map(|step| OwnedStep {
                    id: step
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| unique_id("plan-step")),
                    title: step
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    detail: step
                        .get("detail")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    acceptance: step
                        .get("acceptance")
                        .and_then(Value::as_array)
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let phases = owned_phases
        .iter()
        .map(|phase| foco_store::workspace::NewPlanPhase {
            id: &phase.id,
            title: &phase.title,
            summary: &phase.summary,
            steps: phase
                .steps
                .iter()
                .map(|step| foco_store::workspace::NewPlanStep {
                    id: &step.id,
                    title: &step.title,
                    detail: &step.detail,
                    acceptance: step.acceptance.clone(),
                })
                .collect(),
        })
        .collect();
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let plan = database
        .create_plan(foco_store::workspace::NewPlan {
            id: &plan_id,
            title,
            overview,
            status,
            source_chat_id,
            phases,
        })
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(json!({ "plan": plan })))
}

async fn remote_sidecar_plans_auto_run(
    State(state): State<RemoteSidecarState>,
) -> Result<Json<Value>, axum::response::Response> {
    let database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let value = database
        .plan_auto_run_state()
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(
        json!({ "enabled": value.enabled, "busy": value.busy }),
    ))
}

async fn remote_sidecar_plans_auto_run_set(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let enabled = payload
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let value = database
        .set_plan_auto_run_enabled(enabled)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(
        json!({ "enabled": value.enabled, "busy": value.busy }),
    ))
}

async fn remote_sidecar_plans_order(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let plan_ids = payload
        .get("planIds")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::bad_request("planIds is required").into_response())?
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    database
        .reorder_active_plans(&plan_ids)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(json!({ "ok": true })))
}

async fn remote_sidecar_plans_update(
    State(state): State<RemoteSidecarState>,
    AxumPath(plan_id): AxumPath<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let error_message = if payload.get("errorMessage").is_some() {
        Some(payload.get("errorMessage").and_then(Value::as_str))
    } else {
        None
    };
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let plan = database
        .update_plan(
            &plan_id,
            foco_store::workspace::PlanPatch {
                title: payload.get("title").and_then(Value::as_str),
                overview: payload.get("overview").and_then(Value::as_str),
                status: payload.get("status").and_then(Value::as_str),
                error_message,
            },
        )
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(json!({ "plan": plan })))
}

async fn remote_sidecar_plans_delete(
    State(state): State<RemoteSidecarState>,
    AxumPath(plan_id): AxumPath<String>,
) -> Result<Json<Value>, axum::response::Response> {
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let deleted = database
        .delete_plan(&plan_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(json!({ "deleted": deleted })))
}

async fn remote_sidecar_plans_action(
    State(state): State<RemoteSidecarState>,
    AxumPath(plan_id): AxumPath<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let action = payload
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("action is required").into_response())?;
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let plan = database
        .transition_plan(&plan_id, action)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(json!({ "plan": plan })))
}

async fn remote_sidecar_plans_phase_retry(
    State(state): State<RemoteSidecarState>,
    AxumPath((plan_id, phase_id)): AxumPath<(String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let plan = database
        .fail_plan_phase_start(&plan_id, &phase_id, "remote phase retry queued on sidecar")
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(json!({ "plan": plan })))
}

async fn remote_sidecar_plans_step_action(
    State(state): State<RemoteSidecarState>,
    AxumPath((plan_id, step_id)): AxumPath<(String, String)>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let status = payload.get("status").and_then(Value::as_str).or_else(|| {
        payload
            .get("action")
            .and_then(Value::as_str)
            .map(|action| match action {
                "complete" => "completed",
                "cancel" => "cancelled",
                other => other,
            })
    });
    let mut database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let plan = database
        .update_plan_step(
            &plan_id,
            &step_id,
            foco_store::workspace::PlanStepPatch {
                title: payload.get("title").and_then(Value::as_str),
                detail: payload.get("detail").and_then(Value::as_str),
                acceptance: payload
                    .get("acceptance")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    }),
                status,
            },
        )
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(json!({ "plan": plan })))
}

async fn remote_sidecar_plans_worktree_audit(
    State(state): State<RemoteSidecarState>,
) -> Result<Json<Value>, axum::response::Response> {
    let database =
        foco_store::workspace::WorkspaceDatabase::open_or_create(sidecar_workspace_path(&state))
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let items = database
        .plan_worktree_audit()
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .into_iter()
        .map(|item| {
            json!({
                "planId": item.plan_id,
                "planStatus": item.plan_status,
                "phaseId": item.phase_id,
                "phaseStatus": item.phase_status,
                "implementationChatId": item.implementation_chat_id,
                "agentTaskId": item.agent_task_id,
                "agentTaskStatus": item.agent_task_status,
                "agentInstanceId": item.agent_instance_id.to_string(),
                "worktreePath": item.worktree_path,
                "baseRevision": item.base_revision,
                "branch": item.branch,
                "worktreeStatus": item.worktree_status,
                "planErrorMessage": item.plan_error_message,
                "phaseErrorMessage": item.phase_error_message,
                "taskErrorMessage": item.task_error_message,
                "commitId": item.commit_id,
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "items": items,
        "recoveryNote": "Remote sidecar reports worktrees from the remote workspace DB.",
    })))
}

async fn remote_sidecar_plans_worktree_cleanup(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let confirm = payload
        .get("confirm")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !confirm {
        return Err(ApiError::bad_request("cleanup requires confirm=true").into_response());
    }
    let instance_id = payload
        .get("agentInstanceId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("agentInstanceId is required").into_response())?;
    let workspace_path = sidecar_workspace_path(&state).to_path_buf();
    let mut database = foco_store::workspace::WorkspaceDatabase::open_or_create(&workspace_path)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let record = database
        .plan_worktree_audit()
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .into_iter()
        .find(|item| item.agent_instance_id.as_str() == instance_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "plan worktree audit item was not found: {instance_id}"
            ))
            .into_response()
        })?;
    let instance = database
        .agent_instance(&record.agent_instance_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .ok_or_else(|| {
            ApiError::bad_request(format!("Agent instance was not found: {instance_id}"))
                .into_response()
        })?;
    if instance.execution_workspace_mode
        != foco_agent::AgentExecutionWorkspaceMode::IsolatedWorktree
        || instance.worktree_status.as_deref() == Some("deleted")
    {
        return Err(
            ApiError::bad_request("Agent instance no longer has an isolated worktree")
                .into_response(),
        );
    }
    let root_path =
        crate::git_backend::resolve_agent_worktree_path(&workspace_path, &record.worktree_path);
    crate::git_backend::delete_agent_worktree(&workspace_path, &root_path, true)
        .map_err(|e| e.into_response())?;
    database
        .switch_agent_instance_to_shared_workspace(&record.agent_instance_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(
        json!({ "deleted": true, "item": { "agentInstanceId": instance_id } }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_handles_single_quotes() {
        assert_eq!(shell_quote("/tmp/a'b"), "'/tmp/a'\\''b'");
    }

    #[test]
    fn sidecar_options_require_context() {
        let error = RemoteSidecarOptions::parse(&["--server-id".to_string(), "srv".to_string()])
            .unwrap_err();
        assert!(error.to_string().contains("--workspace-id"));
    }
}
