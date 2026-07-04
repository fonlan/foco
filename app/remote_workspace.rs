use std::{
    collections::{HashMap, HashSet},
    io,
    net::SocketAddr,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Path as AxumPath, State, WebSocketUpgrade, ws::Message},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use chrono::{SecondsFormat, Utc};
use foco_store::config::{RemoteServerProfile, WorkspaceConfig, WorkspaceLocation};
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
    runtime::build_sidecar_runtime_config_bundle,
    save_config, unique_id, workspace_by_id,
};

const REMOTE_SIDECAR_COMMAND: &str = "--remote-sidecar";
const SIDECAR_BINARY_NAME: &str = "foco";
const CONTROL_WS_PATH: &str = "/api/remote/control/ws";

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

#[derive(Clone, Default)]
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
        let control_task =
            connect_control_ws(local_port, &token, bundle, server_id, workspace_id).await?;

        let session = Arc::new(RemoteWorkspaceSession {
            server_id: server_id.to_string(),
            workspace_id: workspace_id.to_string(),
            remote_path,
            target,
            local_port,
            remote_port: bootstrap.port,
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

struct RemoteWorkspaceSession {
    // ponytail: v1 keeps one sidecar and one SSH tunnel per remote workspace; pool later if session counts make this noisy.
    server_id: String,
    workspace_id: String,
    remote_path: String,
    target: String,
    local_port: u16,
    remote_port: u16,
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

    let app = Router::new()
        .route(CONTROL_WS_PATH, get(remote_control_ws))
        .with_state(RemoteSidecarState {
            token: options.token,
            last_config_hash: Arc::new(Mutex::new(None)),
        });
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
struct RemoteSidecarState {
    token: String,
    last_config_hash: Arc<Mutex<Option<String>>>,
}

async fn remote_control_ws(
    State(state): State<RemoteSidecarState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let authorized = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|token| token == state.token);
    if !authorized {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    ws.on_upgrade(move |socket| async move {
        let (mut sender, mut receiver) = socket.split();
        while let Some(Ok(message)) = receiver.next().await {
            let Message::Text(text) = message else {
                continue;
            };
            let Ok(envelope) = serde_json::from_str::<ControlEnvelope>(&text) else {
                continue;
            };
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
                Ok((mut write, mut read)) => {
                    if let Some(tx) = ready_tx.take() {
                        let _ = tx.send(Ok(()));
                    }
                    while let Some(message) = read.next().await {
                        match message {
                            Ok(tungstenite::Message::Ping(bytes)) => {
                                let _ = write.send(tungstenite::Message::Pong(bytes)).await;
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
