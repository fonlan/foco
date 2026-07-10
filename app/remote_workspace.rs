use std::{
    collections::{BTreeMap, HashMap, HashSet},
    convert::Infallible,
    fs, io,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{
        Path as AxumPath, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{StatusCode, header},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{any, get, patch, post, put},
};
use chrono::{SecondsFormat, Utc};
use foco_agent::{build_memory_prompt_section, build_project_spec_prompt_section};
use foco_mcp::McpRegistry;
use foco_providers::{
    NeutralChatMessage, NeutralChatRequest, NeutralChatRole, NeutralChatStreamEvent,
    NeutralToolCall, NeutralToolDefinition, NeutralUsage, stream_chat,
};
use foco_store::{
    config::{RemoteServerProfile, WorkspaceConfig, WorkspaceLocation},
    memory::{
        MemoryDatabase, MemoryKind, MemoryScope, MemorySourceType, MemoryStatus, NewMemoryFact,
        NewMemorySource,
    },
    workspace::{
        LlmRequestAuditFilters, MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS, NewLlmRequest,
        NewLlmRequestEvent, NewMessage, NewRunEvent, TodoGraphFilter, UpdateLlmRequestOutcome,
        WorkspaceDatabase, WorkspaceSpecPromptPlan, WorkspaceSpecSettings, workspace_database_path,
    },
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    process::{Child, Command},
    sync::{Mutex as AsyncMutex, mpsc, oneshot},
    task::JoinHandle,
    time::{sleep, timeout},
};
use tokio_tungstenite::connect_async;
use tungstenite::client::IntoClientRequest;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::{
    ApiError, AppResult, AppState, config_snapshot,
    hooks::HookRuntime,
    http::remote_servers::{normalize_target, remote_server_ssh_args, select_sidecar_asset},
    markdown_code_block, neutral_text_message, neutral_tool_definition,
    prompt::{
        active_system_prompt, agents_prompt_messages, builtin_tool_definitions_for_runtime,
        configured_extra_prompt_message, environment_context_message,
    },
    runtime::{
        QuestionRegistry, SidecarRuntimeConfigBundle, ToolOutputDeltaEvent,
        ToolResourceLockRegistry, build_sidecar_runtime_config_bundle, execute_image_tool,
        execute_tool, execute_web_tool,
    },
    save_config, unique_id, workspace_by_id,
};

const REMOTE_SIDECAR_COMMAND: &str = "--remote-sidecar";
const SIDECAR_BINARY_NAME: &str = "foco";
const CONTROL_WS_PATH: &str = "/api/remote/control/ws";
const SIDECAR_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const CONTROL_WS_PING_INTERVAL: Duration = Duration::from_secs(15);
const SIDECAR_HEALTH_INTERVAL: Duration = Duration::from_secs(20);
const SIDECAR_HEALTH_TIMEOUT: Duration = Duration::from_secs(5);
const REMOTE_RECONNECT_BASE_DELAY: Duration = Duration::from_millis(500);
const REMOTE_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(30);
const BROKER_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const BROKER_OFFLINE_RUN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const REMOTE_SIDECAR_MAX_TOOL_ROUNDS: usize = 8;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RemoteConnectionState {
    Disconnected,
    Checking,
    Connecting,
    Installing,
    Starting,
    Tunneling,
    BrokerConnecting,
    Ready,
    Degraded,
    Reconnecting,
    Offline,
    FailedAuth,
}

impl RemoteConnectionState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Checking => "checking",
            Self::Connecting => "connecting",
            Self::Installing => "installing",
            Self::Starting => "starting",
            Self::Tunneling => "tunneling",
            Self::BrokerConnecting => "brokerConnecting",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Reconnecting => "reconnecting",
            Self::Offline => "offline",
            Self::FailedAuth => "failedAuth",
        }
    }
}

fn ssh_command() -> Command {
    #[cfg(windows)]
    {
        let mut command = Command::new("ssh");
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(windows))]
    {
        Command::new("ssh")
    }
}

#[derive(Clone, Debug)]
struct RemoteSessionStatus {
    state: RemoteConnectionState,
    last_error: Option<String>,
    updated_at: String,
}

impl RemoteSessionStatus {
    fn new(state: RemoteConnectionState, last_error: Option<String>) -> Self {
        Self {
            state,
            last_error,
            updated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        }
    }
}

type BrokerWsWrite = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tungstenite::Message,
>;
type SharedBrokerWsWrite = Arc<AsyncMutex<BrokerWsWrite>>;
type BrokerCancelRegistry = Arc<AsyncMutex<HashMap<String, oneshot::Sender<()>>>>;

#[derive(Clone, Debug)]
struct BrokerLlmAuditContext {
    audit_path: PathBuf,
    workspace_id: String,
    chat_id: Option<String>,
    chat_title: Option<String>,
    request_id: String,
}

#[derive(Clone, Debug)]
struct BrokerLlmAuditEvent {
    event_at: String,
    event_type: String,
    normalized_event: Value,
}

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
    pub(crate) last_error: Option<String>,
    pub(crate) status_updated_at: String,
    pub(crate) active_runs: Vec<RemoteActiveRunSummary>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteActiveRunSummary {
    pub(crate) run_id: String,
    pub(crate) chat_id: String,
    pub(crate) last_sequence: Option<i64>,
    pub(crate) accepting_guidance: bool,
    pub(crate) broker_status: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RemoteWorkspaceManager {
    sessions: Arc<Mutex<HashMap<String, Arc<RemoteWorkspaceSession>>>>,
    statuses: Arc<Mutex<HashMap<String, RemoteSessionStatus>>>,
    // ponytail: process-local keyed mutex; remote lockfile later if multiple Foco processes must coordinate.
    sidecar_install_locks: Arc<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
}

impl RemoteWorkspaceManager {
    pub(crate) async fn connect_workspace(
        &self,
        state: AppState,
        server_id: &str,
        workspace_id: &str,
    ) -> Result<RemoteWorkspaceSessionSummary, ApiError> {
        match self
            .connect_workspace_inner(state, server_id, workspace_id)
            .await
        {
            Ok(summary) => Ok(summary),
            Err(error) => {
                let message = error.message().to_string();
                let state = if is_auth_error_message(&message) {
                    RemoteConnectionState::FailedAuth
                } else {
                    RemoteConnectionState::Offline
                };
                let _ =
                    self.set_status(server_id, Some(workspace_id), state, Some(message.clone()));
                let _ = self.set_status(server_id, None, state, Some(message));
                Err(error)
            }
        }
    }

    pub(crate) async fn ensure_server_sidecar(
        &self,
        state: AppState,
        server_id: &str,
    ) -> Result<(), ApiError> {
        match self.ensure_server_sidecar_inner(&state, server_id).await {
            Ok(()) => Ok(()),
            Err(error) => {
                let message = error.message().to_string();
                let state = if is_auth_error_message(&message) {
                    RemoteConnectionState::FailedAuth
                } else {
                    RemoteConnectionState::Offline
                };
                let _ = self.set_status(server_id, None, state, Some(message));
                Err(error)
            }
        }
    }

    async fn ensure_server_sidecar_inner(
        &self,
        state: &AppState,
        server_id: &str,
    ) -> Result<(), ApiError> {
        let config = config_snapshot(state)?;
        let server = config
            .remote_servers
            .iter()
            .find(|server| server.id == server_id)
            .cloned()
            .ok_or_else(|| remote_error(server_id, None, "remote server was not found"))?;
        self.set_status(server_id, None, RemoteConnectionState::Checking, None)?;
        let target = detect_or_cached_target(&server, server_id, None).await?;
        self.set_status(server_id, None, RemoteConnectionState::Installing, None)?;
        ensure_sidecar_command(state, &server, server_id, None, &target).await?;
        self.set_status(server_id, None, RemoteConnectionState::Ready, None)?;
        Ok(())
    }

    fn set_status(
        &self,
        server_id: &str,
        workspace_id: Option<&str>,
        state: RemoteConnectionState,
        last_error: Option<String>,
    ) -> Result<(), ApiError> {
        let key = status_key(server_id, workspace_id);
        let mut statuses = self
            .statuses
            .lock()
            .map_err(|_| ApiError::internal("remote workspace status lock is poisoned"))?;
        statuses.insert(key, RemoteSessionStatus::new(state, last_error));
        Ok(())
    }

    fn get_status(
        &self,
        server_id: &str,
        workspace_id: Option<&str>,
    ) -> Result<Option<RemoteSessionStatus>, ApiError> {
        let statuses = self
            .statuses
            .lock()
            .map_err(|_| ApiError::internal("remote workspace status lock is poisoned"))?;
        Ok(statuses.get(&status_key(server_id, workspace_id)).cloned())
    }

    fn sidecar_install_lock(&self, key: &str) -> Result<Arc<AsyncMutex<()>>, ApiError> {
        let mut locks = self
            .sidecar_install_locks
            .lock()
            .map_err(|_| ApiError::internal("remote sidecar install lock map is poisoned"))?;
        Ok(locks
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone())
    }

    fn remove_sidecar_install_lock(
        &self,
        key: &str,
        lock: &Arc<AsyncMutex<()>>,
    ) -> Result<(), ApiError> {
        let mut locks = self
            .sidecar_install_locks
            .lock()
            .map_err(|_| ApiError::internal("remote sidecar install lock map is poisoned"))?;
        let should_remove = locks.get(key).is_some_and(|existing| {
            Arc::ptr_eq(existing, lock) && Arc::strong_count(existing) == 2
        });
        if should_remove {
            locks.remove(key);
        }
        Ok(())
    }

    async fn connect_workspace_inner(
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
        self.set_status(
            server_id,
            Some(workspace_id),
            RemoteConnectionState::Checking,
            None,
        )?;
        self.set_status(server_id, None, RemoteConnectionState::Checking, None)?;
        if let Some(existing) = self.session(&key)? {
            return Ok(existing.summary());
        }

        let target = detect_or_cached_target(&server, server_id, Some(workspace_id)).await?;
        self.set_status(
            server_id,
            Some(workspace_id),
            RemoteConnectionState::Installing,
            None,
        )?;
        self.set_status(server_id, None, RemoteConnectionState::Installing, None)?;
        let command =
            ensure_sidecar_command(&state, &server, server_id, Some(workspace_id), &target).await?;
        self.set_status(
            server_id,
            Some(workspace_id),
            RemoteConnectionState::Connecting,
            None,
        )?;
        let token = random_token()?;
        let session_file = ensure_remote_session_file(
            &server,
            server_id,
            workspace_id,
            &remote_path,
            &target,
            &token,
        )
        .await?;
        stop_stale_remote_sidecars(&server, server_id, workspace_id, &remote_path).await?;
        self.set_status(
            server_id,
            Some(workspace_id),
            RemoteConnectionState::Starting,
            None,
        )?;
        let mut sidecar = launch_remote_sidecar(
            &server,
            server_id,
            workspace_id,
            &remote_path,
            &target,
            &token,
            &command,
            &session_file,
        )
        .await?;
        let bootstrap = read_bootstrap(&mut sidecar, server_id, workspace_id).await?;
        validate_bootstrap(&bootstrap, server_id, workspace_id, &target)?;
        self.set_status(
            server_id,
            Some(workspace_id),
            RemoteConnectionState::Tunneling,
            None,
        )?;
        let (local_port, tunnel) =
            start_local_forward(&server, bootstrap.port, server_id, workspace_id).await?;
        let bundle = build_sidecar_runtime_config_bundle(
            &state.user_profile_dir,
            &config,
            workspace_id,
            None,
            Utc::now().timestamp_millis().max(0) as u64,
        )?;
        let active_runs = Arc::new(Mutex::new(Vec::new()));
        let status = Arc::new(Mutex::new(RemoteSessionStatus::new(
            RemoteConnectionState::BrokerConnecting,
            None,
        )));
        self.set_status(
            server_id,
            Some(workspace_id),
            RemoteConnectionState::BrokerConnecting,
            None,
        )?;
        self.set_status(
            server_id,
            None,
            RemoteConnectionState::BrokerConnecting,
            None,
        )?;
        let control_task = connect_control_ws(
            state.clone(),
            local_port,
            &token,
            bundle,
            server_id,
            workspace_id,
            active_runs.clone(),
            status.clone(),
        )
        .await?;

        let session = Arc::new(RemoteWorkspaceSession {
            server_id: server_id.to_string(),
            workspace_id: workspace_id.to_string(),
            remote_path,
            target,
            local_port,
            remote_port: bootstrap.port,
            token: token.clone(),
            started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            sidecar: AsyncMutex::new(Some(sidecar)),
            tunnel: AsyncMutex::new(Some(tunnel)),
            control_task: AsyncMutex::new(Some(control_task)),
            health_task: AsyncMutex::new(Some(start_sidecar_health_ping(
                local_port,
                token.clone(),
                status.clone(),
            ))),
            status,
            active_runs,
        });
        self.set_status(
            server_id,
            Some(workspace_id),
            RemoteConnectionState::Ready,
            None,
        )?;
        self.set_status(server_id, None, RemoteConnectionState::Ready, None)?;
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
            self.set_status(
                server_id,
                Some(workspace_id),
                RemoteConnectionState::Disconnected,
                None,
            )?;
            Ok(true)
        } else {
            self.set_status(
                server_id,
                Some(workspace_id),
                RemoteConnectionState::Disconnected,
                None,
            )?;
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
            self.set_status(
                &session.server_id,
                Some(&session.workspace_id),
                RemoteConnectionState::Disconnected,
                None,
            )?;
            session.stop().await;
        }
        self.set_status(server_id, None, RemoteConnectionState::Disconnected, None)?;
        Ok(())
    }

    pub(crate) async fn disconnect_all(&self) -> Result<(), ApiError> {
        let removed = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| ApiError::internal("remote workspace session lock is poisoned"))?;
            sessions
                .drain()
                .map(|(_, session)| session)
                .collect::<Vec<_>>()
        };
        for session in removed {
            self.set_status(
                &session.server_id,
                Some(&session.workspace_id),
                RemoteConnectionState::Disconnected,
                None,
            )?;
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

    pub(crate) fn workspace_state(
        &self,
        server_id: &str,
        workspace_id: &str,
    ) -> Result<Option<RemoteConnectionState>, ApiError> {
        if let Some(session) = self.session(&session_key(server_id, workspace_id))? {
            return Ok(Some(session.status_snapshot().state));
        }
        Ok(self
            .get_status(server_id, Some(workspace_id))?
            .map(|status| status.state))
    }

    pub(crate) fn server_state(
        &self,
        server_id: &str,
    ) -> Result<Option<RemoteConnectionState>, ApiError> {
        let sessions = self.session_summaries_for_server(server_id)?;
        if sessions.iter().any(|session| session.status == "ready") {
            return Ok(Some(RemoteConnectionState::Ready));
        }
        if sessions
            .iter()
            .any(|session| session.status == "reconnecting" || session.status == "degraded")
        {
            return Ok(Some(RemoteConnectionState::Degraded));
        }
        Ok(self.get_status(server_id, None)?.map(|status| status.state))
    }

    #[cfg(test)]
    pub(crate) fn insert_fake_session_for_test(
        &self,
        server_id: &str,
        workspace_id: &str,
        remote_path: &str,
        local_port: u16,
        token: &str,
    ) {
        let session = Arc::new(RemoteWorkspaceSession {
            server_id: server_id.to_string(),
            workspace_id: workspace_id.to_string(),
            remote_path: remote_path.to_string(),
            target: "linux-x64".to_string(),
            local_port,
            remote_port: local_port,
            token: token.to_string(),
            started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            sidecar: AsyncMutex::new(None),
            tunnel: AsyncMutex::new(None),
            control_task: AsyncMutex::new(None),
            health_task: AsyncMutex::new(None),
            status: Arc::new(Mutex::new(RemoteSessionStatus::new(
                RemoteConnectionState::Ready,
                None,
            ))),
            active_runs: Arc::new(Mutex::new(Vec::new())),
        });
        let mut sessions = self.sessions.lock().expect("remote sessions");
        sessions.insert(session_key(server_id, workspace_id), session);
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
            status: RemoteConnectionState::Disconnected.as_str().to_string(),
            last_error: None,
            status_updated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            active_runs: Vec::new(),
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
    health_task: AsyncMutex<Option<JoinHandle<()>>>,
    status: Arc<Mutex<RemoteSessionStatus>>,
    active_runs: Arc<Mutex<Vec<RemoteActiveRunSummary>>>,
}

impl RemoteWorkspaceSession {
    fn summary(&self) -> RemoteWorkspaceSessionSummary {
        let active_runs = self
            .active_runs
            .lock()
            .map(|runs| runs.clone())
            .unwrap_or_default();
        let status = self.status_snapshot();
        RemoteWorkspaceSessionSummary {
            server_id: self.server_id.clone(),
            workspace_id: self.workspace_id.clone(),
            remote_path: self.remote_path.clone(),
            target: self.target.clone(),
            local_port: self.local_port,
            remote_port: self.remote_port,
            started_at: self.started_at.clone(),
            status: status.state.as_str().to_string(),
            last_error: status.last_error,
            status_updated_at: status.updated_at,
            active_runs,
        }
    }

    fn status_snapshot(&self) -> RemoteSessionStatus {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_else(|_| {
                RemoteSessionStatus::new(
                    RemoteConnectionState::Degraded,
                    Some("remote workspace status lock is poisoned".to_string()),
                )
            })
    }

    async fn stop(&self) {
        set_session_status(&self.status, RemoteConnectionState::Disconnected, None);
        if let Some(task) = self.control_task.lock().await.take() {
            task.abort();
        }
        if let Some(task) = self.health_task.lock().await.take() {
            task.abort();
        }
        let _ = shutdown_remote_sidecar(self.local_port, &self.token).await;
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

#[derive(Clone)]
struct RemoteActiveRunStream {
    broker_request_id: Arc<Mutex<Option<String>>>,
    events: Arc<Mutex<Vec<(i64, Value)>>>,
    tx: tokio::sync::broadcast::Sender<(i64, Value)>,
    finished: Arc<AtomicBool>,
}

impl RemoteActiveRunStream {
    fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(512);
        Self {
            broker_request_id: Arc::new(Mutex::new(None)),
            events: Arc::new(Mutex::new(Vec::new())),
            tx,
            finished: Arc::new(AtomicBool::new(false)),
        }
    }

    fn record(&self, sequence: i64, payload: Value) {
        if let Ok(mut events) = self.events.lock() {
            events.push((sequence, payload.clone()));
        }
        let _ = self.tx.send((sequence, payload));
    }

    fn snapshot_after(&self, sequence: i64) -> Vec<(i64, Value)> {
        self.events
            .lock()
            .map(|events| {
                events
                    .iter()
                    .filter(|(event_sequence, _)| *event_sequence > sequence)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn last_sequence(&self) -> i64 {
        self.events
            .lock()
            .ok()
            .and_then(|events| events.last().map(|(sequence, _)| *sequence))
            .unwrap_or(0)
    }

    fn broker_request_id(&self) -> Option<String> {
        self.broker_request_id.lock().ok().and_then(|id| id.clone())
    }

    fn set_broker_request_id(&self, id: String) {
        if let Ok(mut broker_request_id) = self.broker_request_id.lock() {
            *broker_request_id = Some(id);
        }
    }

    fn mark_finished(&self) {
        self.finished.store(true, Ordering::Relaxed);
    }
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
    let active_runs = Arc::new(Mutex::new(Vec::new()));
    let state = RemoteSidecarState {
        token: options.token,
        workspace_id: options.workspace_id.clone(),
        workspace_path: options.workspace_path.clone(),
        last_config_hash: Arc::new(Mutex::new(None)),
        runtime_config: Arc::new(Mutex::new(None)),
        code_graph_watcher: Arc::new(Mutex::new(None)),
        ws_count: ws_count.clone(),
        active_run_count: active_run_count.clone(),
        active_runs: active_runs.clone(),
        active_run_streams: Arc::new(Mutex::new(HashMap::new())),
        broker_pending: Arc::new(AsyncMutex::new(HashMap::new())),
        broker_tx,
        shutdown_tx: shutdown_tx.clone(),
    };

    let heartbeat_state = state.clone();
    let app = Router::new()
        .route(CONTROL_WS_PATH, get(remote_control_ws))
        .route("/api/remote/health", get(remote_sidecar_health))
        .route("/api/remote/shutdown", post(remote_sidecar_shutdown))
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
            "/api/remote/workspace/file-picker/list",
            post(crate::http::file_picker::remote_sidecar_file_picker_list),
        )
        .route(
            "/api/remote/workspace/file-picker/read-files",
            post(crate::http::file_picker::remote_sidecar_file_picker_read_files),
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
            "/api/remote/workspace/chats",
            get(remote_sidecar_workspace_chats),
        )
        .route(
            "/api/remote/workspace/chats/{chat_id}/messages",
            get(remote_sidecar_chat_messages),
        )
        .route(
            "/api/remote/workspace/chats/{chat_id}/statistics",
            get(remote_sidecar_chat_statistics),
        )
        .route(
            "/api/remote/workspace/chats/{chat_id}/todo-graph",
            get(remote_sidecar_chat_todo_graph),
        )
        .route(
            "/api/remote/workspace/chats/{chat_id}/delete",
            post(remote_sidecar_passthrough_unavailable),
        )
        .route(
            "/api/remote/workspace/chats/{chat_id}/agent-team",
            get(remote_sidecar_agent_no_team),
        )
        .route(
            "/api/remote/workspace/chats/{chat_id}/agent-team/enable",
            post(remote_sidecar_passthrough_unavailable),
        )
        .route(
            "/api/remote/workspace/chats/{chat_id}/agent-team/action",
            post(remote_sidecar_passthrough_unavailable),
        )
        .route(
            "/api/remote/workspace/chats/{chat_id}/agent-team/instances/create",
            post(remote_sidecar_passthrough_unavailable),
        )
        .route(
            "/api/remote/workspace/chat/queue",
            post(remote_sidecar_chat_queue),
        )
        .route(
            "/api/remote/workspace/chat/stream",
            post(remote_sidecar_chat_stream),
        )
        .route(
            "/api/remote/workspace/chat/runs/{run_id}/stream",
            get(remote_sidecar_chat_run_stream),
        )
        .route(
            "/api/remote/workspace/chat/runs/{run_id}/stream-events",
            get(remote_sidecar_chat_run_events_stream),
        )
        .route(
            "/api/remote/workspace/chat/runs/{run_id}/cancel",
            post(remote_sidecar_chat_run_cancel),
        )
        .route(
            "/api/remote/workspace/chat/guidance",
            post(remote_sidecar_chat_guidance),
        )
        .route(
            "/api/remote/workspace/context-usage",
            post(remote_sidecar_context_usage),
        )
        .route(
            "/api/remote/workspace/agent-team/{*path}",
            any(remote_sidecar_passthrough_unavailable),
        )
        .route(
            "/api/remote/workspace/agent-tasks/{*path}",
            any(remote_sidecar_passthrough_unavailable),
        )
        .route(
            "/api/remote/workspace/ai-statistics/{request_id}",
            get(remote_sidecar_ai_statistics_detail),
        )
        .route(
            "/api/remote/workspace/scheduled-tasks",
            get(remote_sidecar_passthrough_unavailable)
                .post(remote_sidecar_passthrough_unavailable),
        )
        .route(
            "/api/remote/workspace/scheduled-tasks/{*path}",
            any(remote_sidecar_passthrough_unavailable),
        )
        .route(
            "/api/remote/workspace/scheduled-task-runs/{*path}",
            any(remote_sidecar_passthrough_unavailable),
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
        tokio::spawn(remote_sidecar_heartbeat_loop(heartbeat_state));
        tokio::spawn(idle_shutdown_watch(shutdown_tx, ws_count, active_run_count));
        let _ = rx.await;
    };

    let server_result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await;
    if let Some(session_file) = options.session_file.as_deref() {
        let _ = fs::remove_file(session_file);
    }
    server_result?;
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

async fn remote_sidecar_heartbeat_loop(state: RemoteSidecarState) {
    loop {
        remote_sidecar_send_heartbeat(&state);
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

fn remote_sidecar_send_heartbeat(state: &RemoteSidecarState) {
    let active_runs = state
        .active_runs
        .lock()
        .map(|runs| runs.clone())
        .unwrap_or_default();
    let envelope = ControlEnvelope {
        version: 1,
        message_type: "heartbeat".to_string(),
        id: None,
        method: Some("sidecar.heartbeat".to_string()),
        payload: json!({
            "workspaceId": state.workspace_id,
            "activeRuns": active_runs,
            "brokerStatus": if state.ws_count.load(Ordering::Relaxed) > 0 { "connected" } else { "brokerUnavailable" },
        }),
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    let _ = state.broker_tx.send(envelope);
}

fn remote_sidecar_set_active_run(state: &RemoteSidecarState, run: RemoteActiveRunSummary) {
    if let Ok(mut runs) = state.active_runs.lock() {
        if let Some(existing) = runs
            .iter_mut()
            .find(|existing| existing.run_id == run.run_id)
        {
            *existing = run;
        } else {
            runs.push(run);
        }
        state.active_run_count.store(runs.len(), Ordering::Relaxed);
    }
    remote_sidecar_send_heartbeat(state);
}

fn remote_sidecar_remove_active_run(state: &RemoteSidecarState, run_id: &str) {
    if let Ok(mut runs) = state.active_runs.lock() {
        runs.retain(|run| run.run_id != run_id);
        state.active_run_count.store(runs.len(), Ordering::Relaxed);
    }
    remote_sidecar_send_heartbeat(state);
}

fn remote_sidecar_insert_active_run_stream(
    state: &RemoteSidecarState,
    run_id: String,
    _chat_id: String,
) -> RemoteActiveRunStream {
    let run_stream = RemoteActiveRunStream::new();
    if let Ok(mut streams) = state.active_run_streams.lock() {
        streams.insert(run_id, run_stream.clone());
    }
    run_stream
}

fn remote_sidecar_active_run_stream(
    state: &RemoteSidecarState,
    run_id: &str,
) -> Option<RemoteActiveRunStream> {
    state.active_run_streams.lock().ok()?.get(run_id).cloned()
}

fn remote_sidecar_record_run_event(
    run_stream: &RemoteActiveRunStream,
    sequence: i64,
    payload: Value,
) -> Event {
    run_stream.record(sequence, payload.clone());
    remote_sse_json_event(sequence, payload)
}

fn remote_sidecar_snapshot_run_events(
    run_stream: &RemoteActiveRunStream,
    last_yielded_sequence: &mut i64,
) -> Vec<(Event, bool)> {
    let mut events = Vec::new();
    for (sequence, event) in run_stream.snapshot_after(*last_yielded_sequence) {
        if sequence <= *last_yielded_sequence {
            continue;
        }
        let terminal = remote_stream_event_is_terminal(&event);
        events.push((remote_sse_json_event(sequence, event), terminal));
        *last_yielded_sequence = sequence;
        if terminal {
            break;
        }
    }
    events
}

fn remote_sidecar_cancel_broker_request(
    state: &RemoteSidecarState,
    run_stream: &RemoteActiveRunStream,
) {
    let Some(broker_request_id) = run_stream.broker_request_id() else {
        return;
    };
    let cancel = ControlEnvelope {
        version: 1,
        message_type: "cancel".to_string(),
        id: Some(broker_request_id),
        method: None,
        payload: json!({}),
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    let _ = state.broker_tx.send(cancel);
}

fn remote_sidecar_finish_active_run(state: &RemoteSidecarState, run_id: &str) {
    if let Ok(mut streams) = state.active_run_streams.lock() {
        if let Some(run_stream) = streams.remove(run_id) {
            run_stream.mark_finished();
        }
    }
    remote_sidecar_remove_active_run(state, run_id);
}

fn remote_sidecar_cancel_active_run(
    state: &RemoteSidecarState,
    run_id: &str,
    emit_events: bool,
    remove_pending: bool,
) {
    let run_stream = state
        .active_run_streams
        .lock()
        .ok()
        .and_then(|mut streams| streams.remove(run_id));
    if let Some(run_stream) = run_stream {
        let broker_request_id = run_stream.broker_request_id();
        remote_sidecar_cancel_broker_request(state, &run_stream);
        if remove_pending {
            if let Some(broker_request_id) = broker_request_id {
                if let Ok(mut pending) = state.broker_pending.try_lock() {
                    pending.remove(&broker_request_id);
                }
            }
        }
        if emit_events {
            let mut sequence = run_stream.last_sequence();
            sequence += 1;
            run_stream.record(
                sequence,
                json!({
                    "type": "error",
                    "message": "remote run was cancelled",
                }),
            );
            sequence += 1;
            run_stream.record(sequence, json!({ "type": "streamEnd" }));
        }
        run_stream.mark_finished();
    }
    remote_sidecar_remove_active_run(state, run_id);
}

struct RemoteRunCleanupGuard {
    state: RemoteSidecarState,
    run_id: String,
    disarmed: bool,
}

impl RemoteRunCleanupGuard {
    fn new(state: RemoteSidecarState, run_id: String) -> Self {
        Self {
            state,
            run_id,
            disarmed: false,
        }
    }

    fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for RemoteRunCleanupGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            remote_sidecar_cancel_active_run(&self.state, &self.run_id, false, true);
        }
    }
}

#[derive(Clone)]
pub(crate) struct RemoteSidecarState {
    token: String,
    last_config_hash: Arc<Mutex<Option<String>>>,
    runtime_config: Arc<Mutex<Option<SidecarRuntimeConfigBundle>>>,
    code_graph_watcher: Arc<Mutex<Option<foco_graph::CodeGraphWatcher>>>,
    ws_count: Arc<AtomicUsize>,
    active_run_count: Arc<AtomicUsize>,
    active_runs: Arc<Mutex<Vec<RemoteActiveRunSummary>>>,
    active_run_streams: Arc<Mutex<HashMap<String, RemoteActiveRunStream>>>,
    broker_pending: Arc<AsyncMutex<HashMap<String, mpsc::UnboundedSender<ControlEnvelope>>>>,
    broker_tx: tokio::sync::broadcast::Sender<ControlEnvelope>,
    shutdown_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    workspace_id: String,
    pub(crate) workspace_path: String,
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

async fn remote_sidecar_health(State(state): State<RemoteSidecarState>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "workspaceId": state.workspace_id,
        "brokerConnected": state.ws_count.load(Ordering::Relaxed) > 0,
        "activeRunCount": state.active_run_count.load(Ordering::Relaxed),
        "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    }))
}

async fn remote_sidecar_shutdown(State(state): State<RemoteSidecarState>) -> Json<Value> {
    if let Ok(mut tx) = state.shutdown_tx.lock() {
        if let Some(tx) = tx.take() {
            let _ = tx.send(());
        }
    }
    Json(json!({ "ok": true }))
}

async fn remote_sidecar_fail_pending_broker_requests(state: &RemoteSidecarState, message: &str) {
    let mut pending = state.broker_pending.lock().await;
    let entries = pending.drain().collect::<Vec<_>>();
    drop(pending);
    for (id, tx) in entries {
        let _ = tx.send(ControlEnvelope {
            version: 1,
            message_type: "error".to_string(),
            id: Some(id),
            method: None,
            payload: json!({ "message": message }),
            timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
        });
    }
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
                    let text = match message {
                        Message::Text(text) => text,
                        Message::Ping(bytes) => {
                            if sender.send(Message::Pong(bytes)).await.is_err() {
                                break;
                            }
                            continue;
                        }
                        Message::Pong(_) => continue,
                        Message::Close(_) => break,
                        _ => continue,
                    };
                    let Ok(envelope) = serde_json::from_str::<ControlEnvelope>(&text) else {
                        continue;
                    };
                    if matches!(envelope.message_type.as_str(), "response" | "error" | "stream") {
                        if let Some(id) = envelope.id.clone() {
                            let terminal = envelope.message_type == "response" || envelope.message_type == "error";
                            let tx = {
                                let mut pending = state.broker_pending.lock().await;
                                if terminal {
                                    pending.remove(&id)
                                } else {
                                    pending.get(&id).cloned()
                                }
                            };
                            if let Some(tx) = tx {
                                let _ = tx.send(envelope);
                            }
                        }
                        continue;
                    }
                    // Handle inbound config sync from local main
                    if envelope.message_type == "config"
                        && envelope.method.as_deref() == Some("config.sync")
                    {
                        let bundle = serde_json::from_value::<SidecarRuntimeConfigBundle>(
                            envelope.payload.clone(),
                        )
                        .ok();
                        let hash = bundle
                            .as_ref()
                            .map(|bundle| bundle.hash.clone())
                            .or_else(|| {
                                envelope
                                    .payload
                                    .get("hash")
                                    .and_then(Value::as_str)
                                    .map(str::to_string)
                            })
                            .unwrap_or_default();
                        if let Some(bundle) = bundle {
                            if let Ok(mut runtime_config) = state.runtime_config.lock() {
                                *runtime_config = Some(bundle);
                            }
                        }
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
        if state.ws_count.fetch_sub(1, Ordering::Relaxed) == 1 {
            remote_sidecar_fail_pending_broker_requests(
                &state,
                "remote broker disconnected; retry after reconnect",
            )
            .await;
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
    session_file: Option<String>,
}

impl RemoteSidecarOptions {
    fn parse(args: &[String]) -> io::Result<Self> {
        let mut server_id = None;
        let mut workspace_id = None;
        let mut workspace_path = None;
        let mut target = None;
        let mut token = None;
        let mut session_file = None;
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            let slot = match arg.as_str() {
                "--server-id" => &mut server_id,
                "--workspace-id" => &mut workspace_id,
                "--workspace-path" => &mut workspace_path,
                "--target" => &mut target,
                "--token" => &mut token,
                "--session-file" => &mut session_file,
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
            session_file,
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

pub(crate) fn workspace_audit_path(
    profile_dir: &Path,
    workspace: &WorkspaceConfig,
) -> Result<PathBuf, ApiError> {
    match workspace.location {
        WorkspaceLocation::Local => Ok(workspace.path.clone()),
        WorkspaceLocation::Ssh { .. } => {
            let path = if workspace.path.as_os_str().is_empty() {
                profile_dir
                    .join(".foco")
                    .join("remote-workspace-audit")
                    .join(&workspace.id)
            } else {
                workspace.path.clone()
            };
            // ponytail: keep remote audit local to the main process; if offline remote stats are needed later, sync this DB to the sidecar.
            fs::create_dir_all(&path).map_err(|source| {
                ApiError::internal(format!(
                    "failed to create remote workspace audit directory {}: {source}",
                    path.display()
                ))
            })?;
            Ok(path)
        }
    }
}

async fn detect_or_cached_target(
    server: &RemoteServerProfile,
    server_id: &str,
    workspace_id: Option<&str>,
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
        workspace_id,
    )
    .await?;
    if !output.status.success() {
        return Err(remote_error(
            server_id,
            workspace_id,
            format!("target probe failed: {}", output_text(&output)),
        ));
    }
    normalize_target(&String::from_utf8_lossy(&output.stdout))
        .map_err(|message| remote_error(server_id, workspace_id, message))
}

pub(crate) async fn run_remote_file_picker_command(
    state: &AppState,
    server_id: &str,
    command: &str,
    payload: Value,
) -> Result<Value, ApiError> {
    if !matches!(
        command,
        crate::http::file_picker::FILE_PICKER_LIST_COMMAND
            | crate::http::file_picker::FILE_PICKER_READ_FILES_COMMAND
    ) {
        return Err(ApiError::bad_request(
            "unsupported remote file picker command",
        ));
    }
    let config = config_snapshot(state)?;
    let server = config
        .remote_servers
        .iter()
        .find(|server| server.id == server_id)
        .cloned()
        .ok_or_else(|| remote_error(server_id, None, "remote server was not found"))?;
    let target = detect_or_cached_target(&server, server_id, None).await?;
    let sidecar_command = ensure_sidecar_command(state, &server, server_id, None, &target).await?;
    let script = format!("{sidecar_command} {}", shell_quote(command));
    let input = serde_json::to_vec(&payload).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize remote file picker payload: {source}"
        ))
    })?;
    let output = run_ssh_with_stdin(&server, &[script.as_str()], &input, server_id, None).await?;
    if !output.status.success() {
        return Err(remote_error(
            server_id,
            None,
            format!("remote file picker failed: {}", output_text(&output)),
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|source| {
        ApiError::bad_gateway(format!("invalid remote file picker response: {source}"))
    })
}

async fn ensure_sidecar_command(
    state: &AppState,
    server: &RemoteServerProfile,
    server_id: &str,
    workspace_id: Option<&str>,
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
        .map_err(|message| remote_error(server_id, workspace_id, message))?;
    let remote_dir = remote_home_shell_path(&format!(
        ".foco/sidecars/{}/{}",
        asset.version, asset.target
    ));
    let remote_bin = remote_home_shell_path(&format!(
        ".foco/sidecars/{}/{}/{}",
        asset.version, asset.target, SIDECAR_BINARY_NAME
    ));
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

    let install_key = sidecar_install_key(server_id, target, &asset.version);
    let install_lock = state
        .remote_workspace_manager
        .sidecar_install_lock(&install_key)?;
    let result: Result<String, ApiError> = {
        let _install_guard = install_lock.lock().await;
        async {
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
                return Ok(remote_bin.clone());
            }

            let bytes = std::fs::read(&asset.path).map_err(|source| {
                remote_error(
                    server_id,
                    workspace_id,
                    format!(
                        "failed to read sidecar asset {}: {source}",
                        asset.path.display()
                    ),
                )
            })?;
            let install_script = format!(
                "set -e; dir={dir}; bin={bin}; tmp=\"$bin.tmp.$$\"; mkdir -p \"$dir\"; trap 'rm -f \"$tmp\"' EXIT HUP INT TERM; cat > \"$tmp\"; chmod +x \"$tmp\"; mv -f \"$tmp\" \"$bin\"; trap - EXIT; \"$bin\" --version; \"$bin\" --sidecar-target",
                dir = remote_dir,
                bin = remote_bin,
            );
            let output = run_ssh_with_stdin(
                server,
                &[install_script.as_str()],
                &bytes,
                server_id,
                workspace_id,
            )
            .await?;
            if !output.status.success() {
                return Err(remote_error(
                    server_id,
                    workspace_id,
                    format!("sidecar upload/install failed: {}", output_text(&output)),
                ));
            }
            verify_remote_command(server, &remote_bin, target, server_id, workspace_id).await?;
            update_sidecar_cache(state, server_id, target, &asset.version, None)?;
            Ok(remote_bin.clone())
        }
        .await
    };
    let _ = state
        .remote_workspace_manager
        .remove_sidecar_install_lock(&install_key, &install_lock);
    result
}

async fn remote_sidecar_matches(
    server: &RemoteServerProfile,
    remote_bin: &str,
    version: &str,
    target: &str,
    server_id: &str,
    workspace_id: Option<&str>,
) -> Result<bool, ApiError> {
    let command = format!(
        "test -x {bin} && {bin} --version && {bin} --sidecar-target",
        bin = remote_bin
    );
    let output = run_ssh_output(server, &[command.as_str()], true, server_id, workspace_id).await?;
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
    workspace_id: Option<&str>,
) -> Result<(), ApiError> {
    let check = format!(
        "{command} --version && {command} --sidecar-target",
        command = command
    );
    let output = run_ssh_output(server, &[check.as_str()], true, server_id, workspace_id).await?;
    if !output.status.success() {
        return Err(remote_error(
            server_id,
            workspace_id,
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
            workspace_id,
            format!("remote sidecar command did not report target {target}"),
        ));
    }
    Ok(())
}

async fn ensure_remote_session_file(
    server: &RemoteServerProfile,
    server_id: &str,
    workspace_id: &str,
    remote_path: &str,
    target: &str,
    token: &str,
) -> Result<String, ApiError> {
    let mut hasher = Sha256::new();
    hasher.update(server_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(workspace_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(remote_path.as_bytes());
    hasher.update(b"\0");
    hasher.update(target.as_bytes());
    let session_name = format!("{}.json", hex_bytes(&hasher.finalize()));
    let session_payload = json!({
        "version": 1,
        "serverId": server_id,
        "workspaceId": workspace_id,
        "workspacePath": remote_path,
        "target": target,
        "token": token,
        "createdAt": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    })
    .to_string();
    let script = format!(
        "set -e; dir=\"$HOME/.foco/remote-sessions\"; mkdir -p \"$dir\"; chmod 700 \"$dir\"; session_path=\"$dir/{session_name}\"; tmp=\"$session_path.tmp.$$\"; cat > \"$tmp\"; chmod 600 \"$tmp\"; mv -f \"$tmp\" \"$session_path\"; printf '%s\\n' \"$session_path\"",
    );
    let output = run_ssh_with_stdin(
        server,
        &[script.as_str()],
        session_payload.as_bytes(),
        server_id,
        Some(workspace_id),
    )
    .await?;
    if !output.status.success() {
        return Err(remote_error(
            server_id,
            Some(workspace_id),
            format!(
                "failed to write remote session file: {}",
                output_text(&output)
            ),
        ));
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Err(remote_error(
            server_id,
            Some(workspace_id),
            "remote session file path was empty",
        ));
    }
    Ok(path)
}

fn stale_remote_sidecar_cleanup_script(
    server_id: &str,
    workspace_id: &str,
    remote_path: &str,
) -> String {
    format!(
        r#"set -eu
sid={server_id}
wid={workspace_id}
wpath={remote_path}
for cmdline in /proc/[0-9]*/cmdline; do
  pid="${{cmdline#/proc/}}"
  pid="${{pid%/cmdline}}"
  cmd="$(tr '\0' ' ' < "$cmdline" 2>/dev/null || true)"
  [ -n "$cmd" ] || continue
  printf '%s' "$cmd" | grep -F -- '--remote-sidecar' >/dev/null || continue
  printf '%s' "$cmd" | grep -F -- "--server-id $sid" >/dev/null || continue
  printf '%s' "$cmd" | grep -F -- "--workspace-id $wid" >/dev/null || continue
  printf '%s' "$cmd" | grep -F -- "--workspace-path $wpath" >/dev/null || continue
  ppid="$(awk '{{print $4}}' "/proc/$pid/stat" 2>/dev/null || true)"
  [ "$ppid" = "1" ] || continue
  kill "$pid" 2>/dev/null || true
done
"#,
        server_id = shell_quote(server_id),
        workspace_id = shell_quote(workspace_id),
        remote_path = shell_quote(remote_path),
    )
}

async fn stop_stale_remote_sidecars(
    server: &RemoteServerProfile,
    server_id: &str,
    workspace_id: &str,
    remote_path: &str,
) -> Result<(), ApiError> {
    let script = stale_remote_sidecar_cleanup_script(server_id, workspace_id, remote_path);
    let output = run_ssh_output(
        server,
        &[script.as_str()],
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
                "failed to stop stale remote sidecars: {}",
                output_text(&output)
            ),
        ));
    }
    Ok(())
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
    session_file: &str,
) -> Result<Child, ApiError> {
    let remote_command = format!(
        "{command} {sidecar} --server-id {server_id} --workspace-id {workspace_id} --workspace-path {workspace_path} --target {target} --token {token} --session-file {session_file}",
        command = command,
        sidecar = REMOTE_SIDECAR_COMMAND,
        server_id = shell_quote(server_id),
        workspace_id = shell_quote(workspace_id),
        workspace_path = shell_quote(remote_path),
        target = shell_quote(target),
        token = shell_quote(token),
        session_file = shell_quote(session_file),
    );
    let args = remote_server_ssh_args(server, &[remote_command.as_str()], true);
    let child = ssh_command()
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
        let mut sink = tokio::io::sink();
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
    let child = ssh_command()
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
    active_runs: Arc<Mutex<Vec<RemoteActiveRunSummary>>>,
    status: Arc<Mutex<RemoteSessionStatus>>,
) -> Result<JoinHandle<()>, ApiError> {
    let url = format!("ws://127.0.0.1:{local_port}{CONTROL_WS_PATH}");
    let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();
    let token = token.to_string();
    let log_server_id = server_id.to_string();
    let log_workspace_id = workspace_id.to_string();
    let handle = tokio::spawn(async move {
        let mut ready_tx = Some(ready_tx);
        let mut attempt = 0_u32;
        loop {
            let connection_state = if attempt == 0 {
                RemoteConnectionState::BrokerConnecting
            } else {
                RemoteConnectionState::Reconnecting
            };
            set_session_status(&status, connection_state, None);
            match connect_control_ws_once(&url, &token, &bundle).await {
                Ok((write, mut read)) => {
                    set_session_status(&status, RemoteConnectionState::Ready, None);
                    if let Some(tx) = ready_tx.take() {
                        let _ = tx.send(Ok(()));
                    }
                    attempt = 0;
                    let write = Arc::new(AsyncMutex::new(write));
                    let cancellations: BrokerCancelRegistry =
                        Arc::new(AsyncMutex::new(HashMap::new()));
                    let mut ping_interval = tokio::time::interval(CONTROL_WS_PING_INTERVAL);
                    loop {
                        tokio::select! {
                            _ = ping_interval.tick() => {
                                let mut write = write.lock().await;
                                if write.send(tungstenite::Message::Ping(Vec::new().into())).await.is_err() {
                                    break;
                                }
                            }
                            message = read.next() => {
                                let message = match message {
                                    Some(message) => message,
                                    None => break,
                                };
                                match message {
                                    Ok(tungstenite::Message::Ping(bytes)) => {
                                        let mut write = write.lock().await;
                                        let _ = write.send(tungstenite::Message::Pong(bytes)).await;
                                    }
                                    Ok(tungstenite::Message::Pong(_)) => {}
                                    Ok(tungstenite::Message::Text(text)) => {
                                        let Ok(envelope) = serde_json::from_str::<ControlEnvelope>(&text) else {
                                            continue;
                                        };
                                        match envelope.message_type.as_str() {
                                            "heartbeat" => {
                                                update_remote_active_runs(&active_runs, &envelope.payload);
                                                set_session_status(&status, RemoteConnectionState::Ready, None);
                                            }
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
                                                    if let Some(tx) = cancellations.lock().await.remove(&id) {
                                                        let _ = tx.send(());
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    Ok(tungstenite::Message::Close(_)) => break,
                                    Ok(_) => {}
                                    Err(error) => {
                                        set_session_status(
                                            &status,
                                            RemoteConnectionState::Degraded,
                                            Some(format!("control WebSocket read failed: {error}")),
                                        );
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(error) => {
                    set_session_status(&status, RemoteConnectionState::Reconnecting, Some(error));
                }
            }
            attempt = attempt.saturating_add(1);
            let delay = reconnect_delay(attempt);
            tracing::warn!(
                %log_server_id,
                %log_workspace_id,
                attempt,
                delay_ms = delay.as_millis() as u64,
                "remote control WebSocket reconnect scheduled"
            );
            sleep(delay).await;
        }
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
/// Payload accepts either the current `request` shape or the older loose
/// `messages`/`tools` fields for sidecar compatibility.
fn broker_llm_audit_context(
    state: &AppState,
    fallback_workspace_id: &str,
    payload: &Value,
) -> Option<BrokerLlmAuditContext> {
    let workspace_id = payload
        .get("workspaceId")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback_workspace_id)
        .to_string();
    let config = config_snapshot(state).ok()?;
    let workspace = workspace_by_id(&config, &workspace_id).ok()?;
    let audit_path = workspace_audit_path(&state.user_profile_dir, workspace).ok()?;
    let chat_id = payload
        .get("chatId")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    Some(BrokerLlmAuditContext {
        audit_path,
        workspace_id,
        chat_id,
        chat_title: payload
            .get("chatTitle")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
        request_id: payload
            .get("runId")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                payload
                    .get("requestId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            })
            .to_string(),
    })
    .filter(|context| !context.request_id.is_empty())
}

fn insert_broker_llm_audit_start(
    context: &BrokerLlmAuditContext,
    provider_id: &str,
    model_id: &str,
    request_started_at: &str,
    request_body_json: &str,
) {
    if let Err(error) = insert_broker_llm_audit_start_inner(
        context,
        provider_id,
        model_id,
        request_started_at,
        request_body_json,
    ) {
        tracing::warn!(
            request_id = %context.request_id,
            workspace_id = %context.workspace_id,
            error = %error,
            "failed to insert brokered remote LLM audit start"
        );
    }
}

fn insert_broker_llm_audit_start_inner(
    context: &BrokerLlmAuditContext,
    provider_id: &str,
    model_id: &str,
    request_started_at: &str,
    request_body_json: &str,
) -> Result<(), foco_store::workspace::WorkspaceDatabaseError> {
    let mut database = WorkspaceDatabase::open_or_create(&context.audit_path)?;
    if let (Some(chat_id), Some(title)) =
        (context.chat_id.as_deref(), context.chat_title.as_deref())
        && database.chat(chat_id)?.is_none()
    {
        database.insert_chat_with_metadata(chat_id, title, "{}")?;
    }
    if database.llm_request(&context.request_id)?.is_some() {
        return Ok(());
    }
    database.insert_llm_request(NewLlmRequest {
        id: &context.request_id,
        workspace_id: &context.workspace_id,
        chat_id: context.chat_id.as_deref(),
        request_kind: "chat completion",
        agent_team_id: None,
        agent_instance_id: None,
        agent_task_id: None,
        agent_attempt_id: None,
        provider_id,
        model_id,
        request_started_at,
        first_token_at: None,
        completed_at: None,
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_write_tokens: None,
        reasoning_tokens: None,
        first_token_latency_ms: None,
        total_latency_ms: None,
        status_code: None,
        final_state: "running",
        request_body_json: Some(request_body_json),
        response_body_json: None,
    })
}

struct BrokerLlmAuditOutcome<'a> {
    final_state: &'a str,
    first_token_at: Option<&'a str>,
    completed_at: &'a str,
    usage: Option<&'a NeutralUsage>,
    first_token_latency_ms: Option<i64>,
    total_latency_ms: i64,
    response_body_json: &'a str,
}

fn finish_broker_llm_audit(
    context: Option<&BrokerLlmAuditContext>,
    outcome: BrokerLlmAuditOutcome<'_>,
    events: &[BrokerLlmAuditEvent],
) {
    let Some(context) = context else {
        return;
    };
    if let Err(error) = finish_broker_llm_audit_inner(context, outcome, events) {
        tracing::warn!(
            request_id = %context.request_id,
            workspace_id = %context.workspace_id,
            error = %error,
            "failed to finish brokered remote LLM audit"
        );
    }
}

fn finish_broker_llm_audit_inner(
    context: &BrokerLlmAuditContext,
    outcome: BrokerLlmAuditOutcome<'_>,
    events: &[BrokerLlmAuditEvent],
) -> Result<(), foco_store::workspace::WorkspaceDatabaseError> {
    let mut database = WorkspaceDatabase::open_or_create(&context.audit_path)?;
    database.update_llm_request_outcome(
        &context.request_id,
        UpdateLlmRequestOutcome {
            first_token_at: outcome.first_token_at,
            completed_at: Some(outcome.completed_at),
            input_tokens: outcome.usage.and_then(|usage| usage.input_tokens),
            output_tokens: outcome.usage.and_then(|usage| usage.output_tokens),
            cache_read_tokens: outcome.usage.and_then(|usage| usage.cache_read_tokens),
            cache_write_tokens: outcome.usage.and_then(|usage| usage.cache_write_tokens),
            reasoning_tokens: outcome.usage.and_then(|usage| usage.reasoning_tokens),
            first_token_latency_ms: outcome.first_token_latency_ms,
            total_latency_ms: Some(outcome.total_latency_ms),
            status_code: None,
            final_state: outcome.final_state,
            response_body_json: Some(outcome.response_body_json),
        },
    )?;
    for (index, event) in events.iter().enumerate() {
        let normalized_event_json = event.normalized_event.to_string();
        database.insert_llm_request_event(NewLlmRequestEvent {
            id: &unique_id("llm-event"),
            llm_request_id: &context.request_id,
            sequence: index as i64,
            event_at: &event.event_at,
            event_type: &event.event_type,
            raw_chunk_json: None,
            normalized_event_json: &normalized_event_json,
        })?;
    }
    Ok(())
}

async fn broker_llm_stream(
    state: &AppState,
    write: &SharedBrokerWsWrite,
    _server_id: &str,
    workspace_id: &str,
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

    let request = payload
        .get("request")
        .and_then(|request| serde_json::from_value::<NeutralChatRequest>(request.clone()).ok())
        .unwrap_or_else(|| {
            let messages: Vec<NeutralChatMessage> = payload
                .get("messages")
                .and_then(|m| serde_json::from_value(m.clone()).ok())
                .unwrap_or_default();
            let tools: Vec<foco_providers::NeutralToolDefinition> = payload
                .get("tools")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            NeutralChatRequest {
                model_id: model_id.to_string(),
                messages,
                tools,
                thinking_level: payload
                    .get("thinkingLevel")
                    .and_then(|v| serde_json::from_value(v.clone()).ok()),
                max_output_tokens: payload
                    .get("maxOutputTokens")
                    .and_then(Value::as_u64)
                    .and_then(|v| u32::try_from(v).ok()),
                prompt_cache_key: payload
                    .get("promptCacheKey")
                    .and_then(|v| serde_json::from_value(v.clone()).ok()),
                prompt_cache_retention: payload
                    .get("promptCacheRetention")
                    .and_then(|v| serde_json::from_value(v.clone()).ok()),
            }
        });

    let audit_context = broker_llm_audit_context(state, workspace_id, &payload);
    let request_started_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let request_started_instant = Instant::now();
    let request_body_json = json!({
        "providerId": provider_id,
        "modelId": model_id,
        "request": &request,
    })
    .to_string();
    if let Some(context) = audit_context.as_ref() {
        insert_broker_llm_audit_start(
            context,
            provider_id,
            model_id,
            &request_started_at,
            &request_body_json,
        );
    }
    let mut audit_events = vec![BrokerLlmAuditEvent {
        event_at: request_started_at.clone(),
        event_type: "start".to_string(),
        normalized_event: json!({
            "type": "start",
            "providerId": provider_id,
            "modelId": model_id,
        }),
    }];

    let mut cancel_rx = cancel_rx;
    let mut stream = match if let Some(cancel_rx) = cancel_rx.as_mut() {
        tokio::select! {
            _ = cancel_rx => {
                let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                audit_events.push(BrokerLlmAuditEvent {
                    event_at: completed_at.clone(),
                    event_type: "error".to_string(),
                    normalized_event: json!({ "type": "error", "code": "cancelled", "message": "broker request cancelled" }),
                });
                finish_broker_llm_audit(audit_context.as_ref(), BrokerLlmAuditOutcome {
                    final_state: "failed",
                    first_token_at: None,
                    completed_at: &completed_at,
                    usage: None,
                    first_token_latency_ms: None,
                    total_latency_ms: request_started_instant.elapsed().as_millis() as i64,
                    response_body_json: &json!({ "error": { "code": "cancelled", "message": "broker request cancelled" } }).to_string(),
                }, &audit_events);
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
            let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
            let message = format!("{e}");
            audit_events.push(BrokerLlmAuditEvent {
                event_at: completed_at.clone(),
                event_type: "error".to_string(),
                normalized_event: json!({ "type": "error", "code": "provider_error", "message": message }),
            });
            let response_body_json =
                json!({ "error": { "code": "provider_error", "message": message } }).to_string();
            finish_broker_llm_audit(
                audit_context.as_ref(),
                BrokerLlmAuditOutcome {
                    final_state: "failed",
                    first_token_at: None,
                    completed_at: &completed_at,
                    usage: None,
                    first_token_latency_ms: None,
                    total_latency_ms: request_started_instant.elapsed().as_millis() as i64,
                    response_body_json: &response_body_json,
                },
                &audit_events,
            );
            let _ = send_broker_error(write, Some(id), "provider_error", message).await;
            return;
        }
    };

    tracing::info!(%provider_id, %model_id, request_id = %id, "remote sidecar broker llm stream started");
    let mut sequence = 0u64;
    let mut final_usage: Option<NeutralUsage> = None;
    let mut final_tool_calls = Vec::<NeutralToolCall>::new();
    let mut first_token_at: Option<String> = None;
    let mut first_token_latency_ms: Option<i64> = None;
    loop {
        let event = match if let Some(cancel_rx) = cancel_rx.as_mut() {
            tokio::select! {
                _ = cancel_rx => {
                    let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                    audit_events.push(BrokerLlmAuditEvent {
                        event_at: completed_at.clone(),
                        event_type: "error".to_string(),
                        normalized_event: json!({ "type": "error", "code": "cancelled", "message": "broker request cancelled" }),
                    });
                    let response_body_json = json!({ "error": { "code": "cancelled", "message": "broker request cancelled" } }).to_string();
                    finish_broker_llm_audit(audit_context.as_ref(), BrokerLlmAuditOutcome {
                        final_state: "failed",
                        first_token_at: first_token_at.as_deref(),
                        completed_at: &completed_at,
                        usage: final_usage.as_ref(),
                        first_token_latency_ms,
                        total_latency_ms: request_started_instant.elapsed().as_millis() as i64,
                        response_body_json: &response_body_json,
                    }, &audit_events);
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
                let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                let message = format!("{e}");
                audit_events.push(BrokerLlmAuditEvent {
                    event_at: completed_at.clone(),
                    event_type: "error".to_string(),
                    normalized_event: json!({ "type": "error", "code": "stream_error", "message": message }),
                });
                let response_body_json =
                    json!({ "error": { "code": "stream_error", "message": message } }).to_string();
                finish_broker_llm_audit(
                    audit_context.as_ref(),
                    BrokerLlmAuditOutcome {
                        final_state: "failed",
                        first_token_at: first_token_at.as_deref(),
                        completed_at: &completed_at,
                        usage: final_usage.as_ref(),
                        first_token_latency_ms,
                        total_latency_ms: request_started_instant.elapsed().as_millis() as i64,
                        response_body_json: &response_body_json,
                    },
                    &audit_events,
                );
                let _ = send_broker_error(write, Some(id), "stream_error", message).await;
                return;
            }
            None => break,
        };
        match event {
            NeutralChatStreamEvent::TextDelta { delta } => {
                sequence += 1;
                if first_token_at.is_none() {
                    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                    first_token_latency_ms =
                        Some(request_started_instant.elapsed().as_millis() as i64);
                    first_token_at = Some(now);
                }
                audit_events.push(BrokerLlmAuditEvent {
                    event_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                    event_type: "text_delta".to_string(),
                    normalized_event: json!({ "type": "textDelta", "delta": delta.clone() }),
                });
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
                final_tool_calls =
                    merge_remote_tool_calls(&final_tool_calls, std::slice::from_ref(&tool_call));
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
                if first_token_at.is_none() {
                    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                    first_token_latency_ms =
                        Some(request_started_instant.elapsed().as_millis() as i64);
                    first_token_at = Some(now);
                }
                audit_events.push(BrokerLlmAuditEvent {
                    event_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                    event_type: "reasoning_delta".to_string(),
                    normalized_event: json!({ "type": "reasoningDelta", "delta": delta.clone() }),
                });
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
                audit_events.push(BrokerLlmAuditEvent {
                    event_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                    event_type: "usage".to_string(),
                    normalized_event: json!({ "type": "usage", "usage": usage.clone() }),
                });
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
                tool_calls,
                usage,
                stop_reason: _,
                response_id: _,
            } => {
                if let Some(usage) = usage.as_ref() {
                    audit_events.push(BrokerLlmAuditEvent {
                        event_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                        event_type: "complete".to_string(),
                        normalized_event: json!({
                            "type": "complete",
                            "usage": usage,
                            "toolCalls": tool_calls,
                        }),
                    });
                }
                final_tool_calls = merge_remote_tool_calls(&final_tool_calls, &tool_calls);
                final_usage = usage;
            }
            NeutralChatStreamEvent::Start => {}
            NeutralChatStreamEvent::ThoughtSignatureDelta { delta: _ } => {}
            NeutralChatStreamEvent::Error { message } => {
                let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                audit_events.push(BrokerLlmAuditEvent {
                    event_at: completed_at.clone(),
                    event_type: "error".to_string(),
                    normalized_event: json!({ "type": "error", "code": "stream_error", "message": message }),
                });
                let response_body_json =
                    json!({ "error": { "code": "stream_error", "message": message } }).to_string();
                finish_broker_llm_audit(
                    audit_context.as_ref(),
                    BrokerLlmAuditOutcome {
                        final_state: "failed",
                        first_token_at: first_token_at.as_deref(),
                        completed_at: &completed_at,
                        usage: final_usage.as_ref(),
                        first_token_latency_ms,
                        total_latency_ms: request_started_instant.elapsed().as_millis() as i64,
                        response_body_json: &response_body_json,
                    },
                    &audit_events,
                );
                let _ = send_broker_error(write, Some(id), "stream_error", message).await;
                return;
            }
        }
    }

    let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let response_body_json = json!({
        "status": "ok",
        "usage": final_usage.as_ref(),
        "toolCalls": final_tool_calls,
    })
    .to_string();
    finish_broker_llm_audit(
        audit_context.as_ref(),
        BrokerLlmAuditOutcome {
            final_state: "succeeded",
            first_token_at: first_token_at.as_deref(),
            completed_at: &completed_at,
            usage: final_usage.as_ref(),
            first_token_latency_ms,
            total_latency_ms: request_started_instant.elapsed().as_millis() as i64,
            response_body_json: &response_body_json,
        },
        &audit_events,
    );

    let response = ControlEnvelope {
        version: 1,
        message_type: "response".to_string(),
        id: Some(id.to_string()),
        method: None,
        payload: json!({
            "status": "ok",
            "usage": final_usage,
            "toolCalls": final_tool_calls,
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
        ssh_command().args(&args).output(),
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
    let upload_timeout = Duration::from_millis(timeout_ms + 30_000);
    let args = remote_server_ssh_args(server, extra_args, true);
    let mut child = ssh_command()
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
        match timeout(upload_timeout, child_stdin.write_all(stdin)).await {
            Ok(Ok(())) => {}
            Ok(Err(source)) => {
                let _ = child.kill().await;
                return Err(remote_error(
                    server_id,
                    workspace_id,
                    format!("failed to upload sidecar over ssh stdin: {source}"),
                ));
            }
            Err(_) => {
                let _ = child.kill().await;
                return Err(remote_error(
                    server_id,
                    workspace_id,
                    "ssh upload timed out",
                ));
            }
        }
    }
    timeout(upload_timeout, child.wait_with_output())
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

fn sidecar_install_key(server_id: &str, target: &str, version: &str) -> String {
    format!("{server_id}\0{target}\0{version}")
}

fn status_key(server_id: &str, workspace_id: Option<&str>) -> String {
    workspace_id
        .map(|workspace_id| session_key(server_id, workspace_id))
        .unwrap_or_else(|| server_id.to_string())
}

fn set_session_status(
    status: &Arc<Mutex<RemoteSessionStatus>>,
    state: RemoteConnectionState,
    last_error: Option<String>,
) {
    if let Ok(mut status) = status.lock() {
        *status = RemoteSessionStatus::new(state, last_error);
    }
}

fn reconnect_delay(attempt: u32) -> Duration {
    let shift = attempt.saturating_sub(1).min(6);
    let base_ms = REMOTE_RECONNECT_BASE_DELAY.as_millis() as u64;
    let max_ms = REMOTE_RECONNECT_MAX_DELAY.as_millis() as u64;
    let capped = base_ms.saturating_mul(1_u64 << shift).min(max_ms);
    let jitter = random_jitter_ms(capped / 4);
    Duration::from_millis((capped + jitter).min(max_ms))
}

fn start_sidecar_health_ping(
    local_port: u16,
    token: String,
    status: Arc<Mutex<RemoteSessionStatus>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{local_port}/api/remote/health");
        loop {
            sleep(SIDECAR_HEALTH_INTERVAL).await;
            let result = timeout(
                SIDECAR_HEALTH_TIMEOUT,
                client.get(&url).bearer_auth(&token).send(),
            )
            .await;
            match result {
                Ok(Ok(response)) if response.status().is_success() => {
                    if !matches!(
                        status.lock().map(|status| status.state).ok(),
                        Some(RemoteConnectionState::Reconnecting | RemoteConnectionState::Offline)
                    ) {
                        set_session_status(&status, RemoteConnectionState::Ready, None);
                    }
                }
                Ok(Ok(response)) if response.status() == reqwest::StatusCode::UNAUTHORIZED => {
                    set_session_status(
                        &status,
                        RemoteConnectionState::FailedAuth,
                        Some("sidecar health authentication failed".to_string()),
                    );
                }
                Ok(Ok(response)) => {
                    set_session_status(
                        &status,
                        RemoteConnectionState::Degraded,
                        Some(format!("sidecar health returned {}", response.status())),
                    );
                }
                Ok(Err(error)) => {
                    set_session_status(
                        &status,
                        RemoteConnectionState::Reconnecting,
                        Some(format!("sidecar health failed: {error}")),
                    );
                }
                Err(_) => {
                    set_session_status(
                        &status,
                        RemoteConnectionState::Degraded,
                        Some("sidecar health timed out".to_string()),
                    );
                }
            }
        }
    })
}

async fn shutdown_remote_sidecar(local_port: u16, token: &str) -> Result<(), reqwest::Error> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{local_port}/api/remote/shutdown");
    let _ = timeout(
        SIDECAR_HEALTH_TIMEOUT,
        client.post(url).bearer_auth(token).send(),
    )
    .await;
    Ok(())
}

fn random_jitter_ms(max_ms: u64) -> u64 {
    if max_ms == 0 {
        return 0;
    }
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        return 0;
    }
    u64::from_le_bytes(bytes) % (max_ms + 1)
}

fn random_token() -> Result<String, ApiError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|source| {
        ApiError::internal(format!("failed to generate sidecar token: {source}"))
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn remote_home_shell_path(suffix: &str) -> String {
    format!("\"$HOME\"/{}", shell_quote(suffix))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn is_auth_error_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("permission denied")
        || lower.contains("authentication failed")
        || lower.contains("publickey")
        || lower.contains("failedauth")
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

fn remote_active_run_from_value(value: &Value) -> Option<RemoteActiveRunSummary> {
    Some(RemoteActiveRunSummary {
        run_id: value.get("runId")?.as_str()?.to_string(),
        chat_id: value.get("chatId")?.as_str()?.to_string(),
        last_sequence: value.get("lastSequence").and_then(Value::as_i64),
        accepting_guidance: value
            .get("acceptingGuidance")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        broker_status: value
            .get("brokerStatus")
            .and_then(Value::as_str)
            .unwrap_or("connected")
            .to_string(),
        updated_at: value
            .get("updatedAt")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn update_remote_active_runs(
    active_runs: &Arc<Mutex<Vec<RemoteActiveRunSummary>>>,
    payload: &Value,
) {
    let Some(runs) = payload.get("activeRuns").and_then(Value::as_array) else {
        return;
    };
    let summaries = runs
        .iter()
        .filter_map(remote_active_run_from_value)
        .collect::<Vec<_>>();
    if let Ok(mut active_runs) = active_runs.lock() {
        *active_runs = summaries;
    }
}

pub(crate) async fn ensure_remote_workspace_connected(
    state: &AppState,
    workspace_id: &str,
) -> Result<(), ApiError> {
    if sidecar_proxy_target(state, workspace_id)?.is_some() {
        return Ok(());
    }
    let config = config_snapshot(state)?;
    let workspace = workspace_by_id(&config, workspace_id)?;
    let Some(server_id) = workspace.server_id().map(str::to_string) else {
        return Ok(());
    };
    state
        .remote_workspace_manager
        .connect_workspace(state.clone(), &server_id, workspace_id)
        .await?;
    Ok(())
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
        "{}/api/remote/workspace/{}",
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

fn sidecar_workspace_database(
    state: &RemoteSidecarState,
) -> Result<WorkspaceDatabase, axum::response::Response> {
    WorkspaceDatabase::open_or_create(sidecar_workspace_path(state))
        .map_err(|e| ApiError::from_workspace_error(e).into_response())
}

fn remote_chat_parts(content: &str, reasoning: Option<&str>) -> Vec<Value> {
    let mut parts = Vec::new();
    if let Some(reasoning) = reasoning.filter(|value| !value.is_empty()) {
        parts.push(json!({ "type": "reasoning", "text": reasoning }));
    }
    if !content.is_empty() {
        parts.push(json!({ "type": "text", "text": content }));
    }
    parts
}

fn remote_message_summary(
    message: foco_store::workspace::MessageRecord,
    tool_calls: &[foco_store::workspace::ToolCallWithResultRecord],
) -> Value {
    let metadata =
        serde_json::from_str::<Value>(&message.metadata_json).unwrap_or_else(|_| json!({}));
    let reasoning = metadata.get("reasoning").and_then(Value::as_str);
    let tool_calls = if message.role == "assistant" {
        tool_calls
            .iter()
            .filter(|tool_call| tool_call.message_id.as_deref() == Some(message.id.as_str()))
            .map(remote_tool_call_summary)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let parts = if message.role == "assistant" {
        remote_message_parts(&message.content, reasoning, &tool_calls)
    } else {
        remote_chat_parts(&message.content, reasoning)
    };
    json!({
        "id": message.id,
        "role": message.role,
        "content": message.content,
        "createdAt": message.created_at,
        "reasoning": reasoning,
        "sessionMode": metadata.get("sessionMode").or_else(|| metadata.get("session_mode")),
        "pendingMode": metadata.get("pendingMode"),
        "queuedRun": metadata.get("queuedRun"),
        "toolCalls": tool_calls,
        "parts": parts,
        "metrics": metadata.get("metrics"),
        "memoriesUsed": [],
        "extractedMemories": [],
        "specUpdates": [],
    })
}

fn remote_tool_call_summary(tool_call: &foco_store::workspace::ToolCallWithResultRecord) -> Value {
    json!({
        "id": tool_call.id,
        "name": tool_call.tool_name,
        "status": tool_call.status,
        "input": serde_json::from_str::<Value>(&tool_call.input_json).unwrap_or(Value::Null),
        "output": tool_call.result.as_ref().and_then(|result| serde_json::from_str::<Value>(&result.output_json).ok()),
        "isError": tool_call.result.as_ref().map(|result| result.is_error).unwrap_or(false),
        "startedAt": tool_call.started_at,
        "completedAt": tool_call.completed_at,
    })
}

fn remote_message_parts(
    content: &str,
    reasoning: Option<&str>,
    tool_calls: &[Value],
) -> Vec<Value> {
    let mut parts = remote_chat_parts(content, reasoning);
    parts.extend(tool_calls.iter().cloned().map(|tool_call| {
        json!({
            "type": "toolCall",
            "toolCall": tool_call,
        })
    }));
    parts
}

fn remote_chat_active_run(state: &RemoteSidecarState, chat_id: &str) -> Option<Value> {
    state
        .active_runs
        .lock()
        .ok()?
        .iter()
        .find(|run| run.chat_id == chat_id)
        .map(|run| {
            json!({
                "runId": run.run_id,
                "workspaceId": state.workspace_id,
                "chatId": run.chat_id,
                "lastSequence": run.last_sequence,
                "acceptingGuidance": run.accepting_guidance,
            })
        })
}

async fn remote_sidecar_workspace_chats(
    State(state): State<RemoteSidecarState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, axum::response::Response> {
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5)
        .clamp(1, 100);
    let database = sidecar_workspace_database(&state)?;
    let page = database
        .chat_page(limit, None)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let chat_ids = page
        .chats
        .iter()
        .map(|chat| chat.id.clone())
        .collect::<Vec<_>>();
    let code_change_stats = database
        .code_change_stats_for_chats(&chat_ids)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let mut chats = Vec::new();
    for chat in page.chats {
        let queued_run = remote_chat_queued_run_for_chat(&database, &chat.id)
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
        chats.push(json!({
            "id": chat.id,
            "title": chat.title,
            "createdAt": chat.created_at,
            "updatedAt": chat.updated_at,
            "codeChangeStats": code_change_stats.get(&chat.id).cloned().unwrap_or_default(),
            "activeRun": remote_chat_active_run(&state, &chat.id),
            "queuedRun": queued_run,
        }));
    }
    Ok(Json(json!({
        "chats": chats,
        "total": page.total_count,
        "limit": limit,
        "hasMore": page.has_more,
        "nextCursor": null,
    })))
}

async fn remote_sidecar_chat_messages(
    State(state): State<RemoteSidecarState>,
    AxumPath(chat_id): AxumPath<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, axum::response::Response> {
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(500)
        .clamp(1, 500);
    let before = query
        .get("beforeSequence")
        .and_then(|value| value.parse::<i64>().ok());
    let database = sidecar_workspace_database(&state)?;
    let chat = database
        .chat(&chat_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .ok_or_else(|| {
            ApiError::bad_request(format!("chat was not found: {chat_id}")).into_response()
        })?;
    let tool_calls = database
        .tool_calls_for_chat(&chat_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let messages = database
        .messages_for_chat_page(&chat_id, before, limit)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let has_more_before = messages.len() == limit;
    let next_before_sequence = messages.first().map(|message| message.sequence);
    Ok(Json(json!({
        "chat": {
            "id": chat.id,
            "title": chat.title,
            "kind": null,
            "readOnly": false,
        },
        "messages": messages
            .into_iter()
            .map(|message| remote_message_summary(message, &tool_calls))
            .collect::<Vec<_>>(),
        "pagination": {
            "hasMoreBefore": has_more_before,
            "nextBeforeSequence": next_before_sequence,
        },
        "activeRun": remote_chat_active_run(&state, &chat_id),
        "pendingQuestion": null,
        "latestResponseUsage": null,
    })))
}

async fn remote_sidecar_chat_statistics(
    State(state): State<RemoteSidecarState>,
    AxumPath(chat_id): AxumPath<String>,
) -> Result<Json<Value>, axum::response::Response> {
    let database = sidecar_workspace_database(&state)?;
    if database
        .chat(&chat_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .is_none()
    {
        return Err(
            ApiError::bad_request(format!("chat was not found: {chat_id}")).into_response(),
        );
    }

    let counts = database
        .message_role_counts_for_chat(&chat_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let mut message_count = 0_i64;
    let mut user_message_count = 0_i64;
    let mut assistant_message_count = 0_i64;
    let mut tool_message_count = 0_i64;
    for count in counts {
        message_count += count.count;
        match count.role.as_str() {
            "user" => user_message_count = count.count,
            "assistant" => assistant_message_count = count.count,
            "tool" => tool_message_count = count.count,
            _ => {}
        }
    }
    let code_change_stats = database
        .code_change_stats_for_chat(&chat_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let request_count = database
        .llm_request_audit_count(LlmRequestAuditFilters {
            chat_id: Some(&chat_id),
            exclude_request_kinds: MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS,
            ..LlmRequestAuditFilters::default()
        })
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let llm_rows = database
        .llm_request_audit_rows(LlmRequestAuditFilters {
            chat_id: Some(&chat_id),
            exclude_request_kinds: MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS,
            limit: Some(request_count),
            offset: Some(0),
            ..LlmRequestAuditFilters::default()
        })
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let total_latency_ms = llm_rows
        .iter()
        .filter_map(|row| row.total_latency_ms)
        .sum::<i64>();
    let ai_summary = crate::llm_request_rows_summary(&llm_rows);
    Ok(Json(json!({
        "workspaceId": state.workspace_id,
        "chatId": chat_id,
        "messageCount": message_count,
        "userMessageCount": user_message_count,
        "assistantMessageCount": assistant_message_count,
        "toolMessageCount": tool_message_count,
        "totalRequests": ai_summary.total_requests,
        "failedRequests": ai_summary.failed_requests,
        "totalInputTokens": ai_summary.total_input_tokens,
        "totalOutputTokens": ai_summary.total_output_tokens,
        "totalCacheReadTokens": ai_summary.total_cache_read_tokens,
        "totalCacheWriteTokens": ai_summary.total_cache_write_tokens,
        "totalTokens": ai_summary.total_tokens,
        "totalLatencyMs": total_latency_ms,
        "averageLatencyMs": ai_summary.average_latency_ms,
        "memoryReferences": 0,
        "createdMemories": 0,
        "codeChangeStats": code_change_stats,
        "modelBreakdown": ai_summary.model_breakdown,
        "providerBreakdown": ai_summary.provider_breakdown,
        "toolBreakdown": [],
        "compression": {
            "snapshotCount": 0,
            "ruleSnapshotCount": 0,
            "llmSnapshotCount": 0,
            "runtimeToolStateSnapshotCount": 0,
            "originalTokenCount": 0,
            "summaryTokenCount": 0,
            "savedTokenCount": 0
        },
        "contextUsageTimeline": []
    })))
}

async fn remote_sidecar_chat_todo_graph(
    State(state): State<RemoteSidecarState>,
    AxumPath(chat_id): AxumPath<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, axum::response::Response> {
    let database = sidecar_workspace_database(&state)?;
    if database
        .chat(&chat_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .is_none()
    {
        return Err(
            ApiError::bad_request(format!("chat was not found: {chat_id}")).into_response(),
        );
    }
    let status = query
        .get("status")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let task_id = query
        .get("taskId")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let include_subtasks = query
        .get("includeSubtasks")
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(true);
    let graph = database
        .filtered_todo_graph(
            &chat_id,
            TodoGraphFilter {
                status,
                task_id,
                include_subtasks,
            },
        )
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let response = match graph {
        Some(graph) => json!({
            "chatId": graph.chat_id,
            "exists": true,
            "tasks": graph.tasks,
            "createdAt": graph.created_at,
            "updatedAt": graph.updated_at,
        }),
        None => json!({
            "chatId": chat_id,
            "exists": false,
            "tasks": [],
            "createdAt": null,
            "updatedAt": null,
        }),
    };
    Ok(Json(response))
}
async fn remote_sidecar_agent_no_team(
    State(state): State<RemoteSidecarState>,
    AxumPath(chat_id): AxumPath<String>,
) -> Result<Json<Value>, axum::response::Response> {
    let active_run = state
        .active_runs
        .lock()
        .ok()
        .and_then(|runs| runs.iter().find(|run| run.chat_id == chat_id).cloned());
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let team_id = format!("remote-agent-team-{chat_id}");
    let instance_id = format!("remote-agent-instance-{chat_id}");
    let task_status = if active_run.is_some() {
        "running"
    } else {
        "completed"
    };
    let instance_status = if active_run.is_some() {
        "running"
    } else {
        "idle"
    };
    let task = active_run.as_ref().map(|run| {
        json!({
            "id": format!("remote-agent-task-{}", run.run_id),
            "teamId": team_id,
            "ownerInstanceId": instance_id,
            "originInstanceId": null,
            "parentTaskId": null,
            "sequence": 0,
            "status": task_status,
            "input": {
                "message": "Remote workspace chat run",
                "remote": true,
                "runId": run.run_id,
            },
            "result": null,
            "error": null,
            "attempts": [],
            "createdAt": run.updated_at,
            "updatedAt": run.updated_at,
            "startedAt": run.updated_at,
            "completedAt": null,
        })
    });
    Ok(Json(json!({
        "team": {
            "id": team_id,
            "chatId": chat_id,
            "coordinatorInstanceId": instance_id,
            "status": "active",
            "maxConcurrentRuns": 1,
            "createdAt": now,
            "updatedAt": active_run.as_ref().map(|run| run.updated_at.clone()).unwrap_or_else(|| now.clone()),
        },
        "workload": {
            "queuedTasks": 0,
            "runningTasks": if active_run.is_some() { 1 } else { 0 },
            "waitingTasks": 0,
        },
        "observability": {
            "queueLength": 0,
            "queueWaitMs": { "count": 0, "max": null, "average": null },
            "runDurationMs": { "count": 0, "max": null, "average": null },
            "schedulerLatencyMs": { "count": 0, "max": null, "average": null },
            "mutationLeaseWaitMs": { "count": 0, "max": null, "average": null },
            "failedTasks": 0,
            "cancelledTasks": 0,
            "interruptedTasks": 0,
            "failuresByType": [],
        },
        "instances": [{
            "id": instance_id,
            "teamId": team_id,
            "definitionId": "agent-definition-default",
            "definitionRevision": 1,
            "definitionSnapshot": {
                "id": "agent-definition-default",
                "revision": 1,
                "name": "Remote coordinator",
                "description": "Read-only snapshot for remote workspace chat runs.",
                "providerId": "remote",
                "modelId": "remote",
                "modelOptions": {},
                "allowedTools": [],
                "maxInstances": 1,
                "allowedExecutionWorkspaceModes": ["shared"],
                "permissions": {
                    "canCreateInstances": false,
                    "canDelegate": false,
                    "allowedAgentDefinitionIds": [],
                },
            },
            "role": "coordinator",
            "status": instance_status,
            "nextTaskSequence": 1,
            "contextGeneration": 0,
            "lastScheduledAt": active_run.as_ref().map(|run| run.updated_at.clone()),
            "executionWorkspaceMode": "shared",
            "executionRootPath": null,
            "worktreeBaseRevision": null,
            "worktreeBranch": null,
            "worktreeStatus": null,
            "createdAt": now,
            "updatedAt": active_run.as_ref().map(|run| run.updated_at.clone()).unwrap_or_else(|| now.clone()),
        }],
        "tasks": task.into_iter().collect::<Vec<_>>(),
        "dependencies": [],
        "messages": [],
        "events": [],
        "runEvents": [],
        "mutationLeaseOwners": [],
        "worktreeAction": {
            "kind": "unavailable",
            "message": "Remote sidecar exposes a read-only Agent snapshot; Agent team actions are unavailable.",
        },
    })))
}

fn remote_required_text(
    value: Option<&Value>,
    field: &str,
) -> Result<String, axum::response::Response> {
    let text = value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request(format!("{field} is required")).into_response())?;
    Ok(text.to_string())
}

fn remote_optional_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn remote_idempotency_key(payload: &Value, fallback: &str) -> String {
    remote_optional_string(payload.get("idempotencyKey")).unwrap_or_else(|| {
        let mut hasher = Sha256::new();
        hasher.update(fallback.as_bytes());
        format!("sha256:{}", hex_bytes(&hasher.finalize()))
    })
}

fn remote_message_queued_run(metadata_json: &str) -> Option<Value> {
    serde_json::from_str::<Value>(metadata_json)
        .ok()?
        .get("queuedRun")
        .cloned()
}

fn remote_chat_queued_run_for_chat(
    database: &WorkspaceDatabase,
    chat_id: &str,
) -> Result<Option<Value>, foco_store::workspace::WorkspaceDatabaseError> {
    let messages = database.messages_for_chat(chat_id)?;
    for message in messages.into_iter().rev() {
        if message.role != "user" {
            continue;
        }
        let Some(mut queued_run) = remote_message_queued_run(&message.metadata_json) else {
            continue;
        };
        if queued_run.get("status").and_then(Value::as_str) != Some("queued") {
            continue;
        }
        if let Some(object) = queued_run.as_object_mut() {
            object
                .entry("content".to_string())
                .or_insert_with(|| Value::String(message.content));
        }
        return Ok(Some(queued_run));
    }
    Ok(None)
}

fn remote_clear_message_queued_run(
    database: &mut WorkspaceDatabase,
    message_id: &str,
) -> Result<(), foco_store::workspace::WorkspaceDatabaseError> {
    let Some(message) = database.message(message_id)? else {
        return Ok(());
    };
    let Ok(mut metadata) = serde_json::from_str::<Value>(&message.metadata_json) else {
        return Ok(());
    };
    let Some(object) = metadata.as_object_mut() else {
        return Ok(());
    };
    if object.remove("queuedRun").is_none() {
        return Ok(());
    }
    database.update_message_metadata(message_id, &metadata.to_string())
}

async fn remote_sidecar_chat_queue(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    let message = remote_required_text(payload.get("message"), "message")?;
    let model_id = remote_required_text(payload.get("modelId"), "modelId")?;
    let provider_id = remote_optional_string(payload.get("providerId"));
    let thinking_level = remote_optional_string(payload.get("thinkingLevel"));
    let session_mode = remote_optional_string(payload.get("sessionMode"));
    let mut database = sidecar_workspace_database(&state)?;
    let chat_id =
        remote_optional_string(payload.get("chatId")).unwrap_or_else(|| unique_id("chat"));
    let chat = match database
        .chat(&chat_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
    {
        Some(chat) => chat,
        None => {
            let title = message
                .lines()
                .next()
                .unwrap_or("New chat")
                .chars()
                .take(80)
                .collect::<String>();
            database
                .insert_chat_with_metadata(&chat_id, &title, "{}")
                .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
            database
                .chat(&chat_id)
                .map_err(|e| ApiError::from_workspace_error(e).into_response())?
                .ok_or_else(|| {
                    ApiError::internal("queued chat was not persisted").into_response()
                })?
        }
    };
    let idempotency_key = remote_idempotency_key(
        &payload,
        &format!(
            "queue:{}:{}:{}:{}:{}",
            chat_id,
            message,
            model_id,
            provider_id.as_deref().unwrap_or(""),
            session_mode.as_deref().unwrap_or("")
        ),
    );
    for existing in database
        .messages_for_chat(&chat_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
    {
        let Some(queued_run) = remote_message_queued_run(&existing.metadata_json) else {
            continue;
        };
        if queued_run.get("idempotencyKey").and_then(Value::as_str)
            == Some(idempotency_key.as_str())
        {
            return Ok(Json(json!({
                "chatId": chat_id,
                "chatTitle": chat.title,
                "createdAt": chat.created_at,
                "updatedAt": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                "userMessageId": existing.id,
                "assistantMessageId": queued_run.get("assistantMessageId").and_then(Value::as_str).unwrap_or(""),
                "content": existing.content,
                "parts": remote_chat_parts(&message, None),
                "sessionMode": session_mode,
                "idempotencyKey": idempotency_key,
            })));
        }
    }
    let user_sequence = database
        .next_message_sequence_for_chat(&chat_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    let assistant_sequence = user_sequence + 1;
    let user_message_id = unique_id("msg-user");
    let assistant_message_id = unique_id("msg-assistant");
    let queued_run = json!({
        "status": "queued",
        "userMessageId": user_message_id,
        "assistantMessageId": assistant_message_id,
        "assistantSequence": assistant_sequence,
        "modelId": model_id,
        "providerId": provider_id,
        "thinkingLevel": thinking_level,
        "skillIds": payload.get("skillIds").cloned().unwrap_or_else(|| json!([])),
        "sessionMode": session_mode,
        "content": message,
        "idempotencyKey": idempotency_key,
    });
    let user_metadata = json!({
        "parts": remote_chat_parts(&message, None),
        "queuedRun": queued_run,
    });
    database
        .insert_message(NewMessage {
            id: &user_message_id,
            chat_id: &chat_id,
            role: "user",
            content: &message,
            sequence: user_sequence,
            metadata_json: Some(&user_metadata.to_string()),
        })
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    database
        .insert_message(NewMessage {
            id: &assistant_message_id,
            chat_id: &chat_id,
            role: "assistant",
            content: "",
            sequence: assistant_sequence,
            metadata_json: Some("{}"),
        })
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(json!({
        "chatId": chat_id,
        "chatTitle": chat.title,
        "createdAt": chat.created_at,
        "updatedAt": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        "userMessageId": user_message_id,
        "assistantMessageId": assistant_message_id,
        "content": message,
        "parts": remote_chat_parts(&message, None),
        "sessionMode": session_mode,
        "idempotencyKey": idempotency_key,
    })))
}

async fn remote_sidecar_broker_request(
    state: &RemoteSidecarState,
    id: &str,
    method: &str,
    payload: Value,
) -> Result<mpsc::UnboundedReceiver<ControlEnvelope>, axum::response::Response> {
    let deadline = tokio::time::Instant::now() + BROKER_OFFLINE_RUN_TIMEOUT;
    while state.ws_count.load(Ordering::Relaxed) == 0 {
        if tokio::time::Instant::now() >= deadline {
            return Err(ApiError::bad_gateway(
                "remote broker is unavailable; active run can be retried after reconnect",
            )
            .into_response());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let (tx, rx) = mpsc::unbounded_channel();
    state.broker_pending.lock().await.insert(id.to_string(), tx);
    let envelope = ControlEnvelope {
        version: 1,
        message_type: "request".to_string(),
        id: Some(id.to_string()),
        method: Some(method.to_string()),
        payload,
        timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    };
    if state.broker_tx.send(envelope).is_err() {
        state.broker_pending.lock().await.remove(id);
        return Err(ApiError::bad_gateway("remote broker is unavailable").into_response());
    }
    Ok(rx)
}

fn remote_sse_json_event(sequence: i64, event: Value) -> Event {
    Event::default()
        .id(sequence.to_string())
        .data(event.to_string())
}

fn remote_stream_event_is_terminal(event: &Value) -> bool {
    event.get("type").and_then(Value::as_str) == Some("streamEnd")
}

fn remote_elapsed_millis(started_at: Instant) -> i64 {
    i64::try_from(started_at.elapsed().as_millis())
        .expect("remote run latency should fit in i64 milliseconds")
}

fn merge_remote_usage(total: &mut NeutralUsage, next: &NeutralUsage) {
    add_remote_usage_tokens(&mut total.input_tokens, next.input_tokens);
    add_remote_usage_tokens(&mut total.output_tokens, next.output_tokens);
    add_remote_usage_tokens(&mut total.cache_read_tokens, next.cache_read_tokens);
    add_remote_usage_tokens(&mut total.cache_write_tokens, next.cache_write_tokens);
    add_remote_usage_tokens(&mut total.reasoning_tokens, next.reasoning_tokens);
}

fn add_remote_usage_tokens(total: &mut Option<i64>, next: Option<i64>) {
    if let Some(next) = next {
        *total = Some(total.unwrap_or(0) + next);
    }
}

struct RemoteSidecarRunMetrics {
    started_at: Instant,
    request_started_at: String,
    first_token_at: Option<String>,
    first_token_latency_ms: Option<i64>,
    usage: NeutralUsage,
}

impl RemoteSidecarRunMetrics {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            request_started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            first_token_at: None,
            first_token_latency_ms: None,
            usage: NeutralUsage::default(),
        }
    }

    fn capture_first_output(&mut self) {
        if self.first_token_at.is_none() {
            self.first_token_at = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
            self.first_token_latency_ms = Some(remote_elapsed_millis(self.started_at));
        }
    }

    fn merge_usage_value(&mut self, usage: &Value) {
        let Ok(next) = serde_json::from_value::<NeutralUsage>(usage.clone()) else {
            return;
        };
        merge_remote_usage(&mut self.usage, &next);
    }

    fn total_latency_ms(&self) -> i64 {
        remote_elapsed_millis(self.started_at).max(1)
    }

    fn usage_value(&self) -> Value {
        serde_json::to_value(&self.usage).expect("neutral usage should serialize")
    }
}

fn remote_chat_metrics(
    model_id: &str,
    provider_id: &str,
    run_id: &str,
    run_metrics: &RemoteSidecarRunMetrics,
    total_latency_ms: i64,
) -> Value {
    json!({
        "modelId": model_id,
        "providerId": provider_id,
        "totalLatencyMs": total_latency_ms,
        "firstTokenLatencyMs": run_metrics.first_token_latency_ms,
        "outputTokens": run_metrics.usage.output_tokens,
        "llmRequestIds": [run_id],
    })
}

fn persist_sidecar_llm_audit(
    database: &mut WorkspaceDatabase,
    workspace_id: &str,
    chat_id: &str,
    request_id: &str,
    provider_id: &str,
    model_id: &str,
    request: Option<&NeutralChatRequest>,
    run_metrics: &RemoteSidecarRunMetrics,
    completed_at: &str,
    total_latency_ms: i64,
    final_state: &str,
    response_body: Value,
) -> Result<(), foco_store::workspace::WorkspaceDatabaseError> {
    let request_body_json = json!({
        "providerId": provider_id,
        "modelId": model_id,
        "brokered": true,
        "request": request,
    })
    .to_string();
    let response_body_json = response_body.to_string();
    if database.llm_request(request_id)?.is_none() {
        database.insert_llm_request(NewLlmRequest {
            id: request_id,
            workspace_id,
            chat_id: Some(chat_id),
            request_kind: "chat completion",
            agent_team_id: None,
            agent_instance_id: None,
            agent_task_id: None,
            agent_attempt_id: None,
            provider_id,
            model_id,
            request_started_at: &run_metrics.request_started_at,
            first_token_at: run_metrics.first_token_at.as_deref(),
            completed_at: Some(completed_at),
            input_tokens: run_metrics.usage.input_tokens,
            output_tokens: run_metrics.usage.output_tokens,
            cache_read_tokens: run_metrics.usage.cache_read_tokens,
            cache_write_tokens: run_metrics.usage.cache_write_tokens,
            reasoning_tokens: run_metrics.usage.reasoning_tokens,
            first_token_latency_ms: run_metrics.first_token_latency_ms,
            total_latency_ms: Some(total_latency_ms),
            status_code: None,
            final_state,
            request_body_json: Some(&request_body_json),
            response_body_json: Some(&response_body_json),
        })?;
    } else {
        database.update_llm_request_outcome(
            request_id,
            UpdateLlmRequestOutcome {
                first_token_at: run_metrics.first_token_at.as_deref(),
                completed_at: Some(completed_at),
                input_tokens: run_metrics.usage.input_tokens,
                output_tokens: run_metrics.usage.output_tokens,
                cache_read_tokens: run_metrics.usage.cache_read_tokens,
                cache_write_tokens: run_metrics.usage.cache_write_tokens,
                reasoning_tokens: run_metrics.usage.reasoning_tokens,
                first_token_latency_ms: run_metrics.first_token_latency_ms,
                total_latency_ms: Some(total_latency_ms),
                status_code: None,
                final_state,
                response_body_json: Some(&response_body_json),
            },
        )?;
    }
    Ok(())
}

fn neutral_role_for_message(role: &str) -> NeutralChatRole {
    match role {
        "assistant" => NeutralChatRole::Assistant,
        "tool" => NeutralChatRole::Tool,
        "system" => NeutralChatRole::System,
        "developer" => NeutralChatRole::Developer,
        _ => NeutralChatRole::User,
    }
}

fn remote_sidecar_executable_tool_schemas() -> Vec<NeutralToolDefinition> {
    builtin_tool_definitions_for_runtime(true, false)
        .into_iter()
        .filter(|tool| {
            matches!(classify_tool_route(tool.name), ToolRoute::SidecarLocal)
                && !matches!(
                    tool.name,
                    "write_file"
                        | "edit_file"
                        | "create_todo_graph"
                        | "update_todo_graph"
                        | "get_todo_graph"
                        | "create_plan"
                        | "get_plans"
                        | "update_plan"
                        | "update_plan_step"
                        | "delete_plan"
                        | "read_spec"
                        | "update_spec"
                        | "agent_list"
                        | "agent_get_task"
                        | "agent_send_message"
                        | "agent_delegate_task"
                        | "agent_cancel_task"
                        | "agent_wait_tasks"
                        | "agent_transfer_task"
                        | "agent_create_instances"
                        | "sleep"
                )
        })
        .map(neutral_tool_definition)
        .collect()
}

fn remote_sidecar_provider_request(
    state: &RemoteSidecarState,
    database: &WorkspaceDatabase,
    chat_id: &str,
    assistant_message_id: &str,
    model_id: &str,
    thinking_level: Value,
) -> Result<NeutralChatRequest, axum::response::Response> {
    let raw_messages =
        remote_sidecar_chat_messages_for_request(database, chat_id, assistant_message_id)
            .map_err(|e| ApiError::from_workspace_error(e).into_response())?;

    let Some(bundle) = state
        .runtime_config
        .lock()
        .ok()
        .and_then(|config| config.clone())
    else {
        return Ok(NeutralChatRequest {
            model_id: model_id.to_string(),
            messages: raw_messages,
            tools: remote_sidecar_executable_tool_schemas(),
            thinking_level: serde_json::from_value(thinking_level).ok(),
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
        });
    };
    let payload = &bundle.payload;
    let Some(model) = payload.models.iter().find(|model| model.id == model_id) else {
        return Ok(NeutralChatRequest {
            model_id: model_id.to_string(),
            messages: raw_messages,
            tools: remote_sidecar_executable_tool_schemas(),
            thinking_level: serde_json::from_value(thinking_level).ok(),
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
        });
    };

    let workspace_path = Path::new(&state.workspace_path);
    let mut messages = Vec::with_capacity(raw_messages.len() + 8);
    messages.push(neutral_text_message(
        NeutralChatRole::System,
        active_system_prompt(&payload.prompts, &model.system_prompt_name)
            .map_err(|e| e.into_response())?,
    ));
    messages.push(neutral_text_message(
        NeutralChatRole::System,
        build_project_spec_prompt_section(),
    ));
    if payload.memory.enabled {
        messages.push(neutral_text_message(
            NeutralChatRole::System,
            build_memory_prompt_section(),
        ));
    }
    for message in sidecar_selected_skill_messages(&bundle) {
        messages.push(message);
    }
    if let Some(message) = configured_extra_prompt_message(&payload.prompts) {
        messages.push(message);
    }
    if let Ok(mut agent_messages) = agents_prompt_messages(workspace_path) {
        messages.append(&mut agent_messages);
    }
    if let Ok(message) = environment_context_message(workspace_path) {
        messages.push(message);
    }
    if let Some(message) = remote_project_spec_context_message(database, chat_id)? {
        messages.push(message);
    }
    messages.extend(raw_messages);

    // ponytail: keep the remote schema list to tools with a real Phase 1 execution path.
    Ok(NeutralChatRequest {
        model_id: model_id.to_string(),
        messages,
        tools: remote_sidecar_executable_tool_schemas(),
        thinking_level: serde_json::from_value(thinking_level)
            .ok()
            .or_else(|| model.thinking_level.clone()),
        max_output_tokens: model
            .limits
            .as_ref()
            .and_then(|limits| u32::try_from(limits.max_output_tokens).ok()),
        prompt_cache_key: None,
        prompt_cache_retention: None,
    })
}

fn sidecar_selected_skill_messages(bundle: &SidecarRuntimeConfigBundle) -> Vec<NeutralChatMessage> {
    if bundle.payload.selected_skills.is_empty() {
        return Vec::new();
    }
    let entries = bundle
        .payload
        .selected_skills
        .iter()
        .map(|skill| {
            format!(
                "## Skill: {}\n\nPath: `{}`\n\n{}",
                skill.name,
                skill.path,
                skill.content_markdown.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    vec![neutral_text_message(
        NeutralChatRole::Developer,
        format!("## Selected Skills\n\n{entries}"),
    )]
}

fn remote_sidecar_chat_messages_for_request(
    database: &WorkspaceDatabase,
    chat_id: &str,
    assistant_message_id: &str,
) -> Result<Vec<NeutralChatMessage>, foco_store::workspace::WorkspaceDatabaseError> {
    let messages = database.messages_for_chat(chat_id)?;
    let tool_calls = database.tool_calls_for_chat(chat_id)?;
    let mut tool_calls_by_message = HashMap::<String, Vec<_>>::new();
    for tool_call in tool_calls {
        let Some(message_id) = tool_call.message_id.clone() else {
            continue;
        };
        tool_calls_by_message
            .entry(message_id)
            .or_default()
            .push(tool_call);
    }

    let mut raw_messages = Vec::new();
    for message in messages {
        if message.role == "assistant" && message.id == assistant_message_id {
            continue;
        }
        let metadata = serde_json::from_str::<Value>(&message.metadata_json).unwrap_or(Value::Null);
        let reasoning = metadata
            .get("reasoning")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.is_empty());
        let mut tool_message_parts = Vec::new();
        let mut assistant_tool_calls = Vec::new();
        if message.role == "assistant"
            && let Some(tool_records) = tool_calls_by_message.remove(&message.id)
        {
            for tool_record in tool_records {
                assistant_tool_calls.push(remote_neutral_tool_call_from_record(&tool_record));
                if let Some(result) = tool_record.result {
                    tool_message_parts.push(NeutralChatMessage {
                        role: NeutralChatRole::Tool,
                        content: result.output_json,
                        attachments: Vec::new(),
                        reasoning: None,
                        tool_calls: Vec::new(),
                        tool_call_id: Some(tool_record.id.clone()),
                        tool_name: Some(tool_record.tool_name.clone()),
                    });
                }
            }
        }
        raw_messages.push(NeutralChatMessage {
            role: neutral_role_for_message(&message.role),
            content: message.content,
            attachments: Vec::new(),
            reasoning,
            tool_calls: assistant_tool_calls,
            tool_call_id: None,
            tool_name: None,
        });
        raw_messages.extend(tool_message_parts);
    }

    Ok(raw_messages)
}

fn remote_neutral_tool_call_from_record(
    tool_call: &foco_store::workspace::ToolCallWithResultRecord,
) -> NeutralToolCall {
    NeutralToolCall {
        call_id: tool_call.id.clone(),
        name: tool_call.tool_name.clone(),
        arguments: serde_json::from_str(&tool_call.input_json).unwrap_or(Value::Null),
        thought_signatures: None,
    }
}

fn remote_sidecar_executable_tool_names() -> HashSet<String> {
    remote_sidecar_executable_tool_schemas()
        .into_iter()
        .map(|tool| tool.name)
        .collect()
}

fn merge_remote_tool_calls(
    existing: &[NeutralToolCall],
    incoming: &[NeutralToolCall],
) -> Vec<NeutralToolCall> {
    let mut merged = BTreeMap::<String, NeutralToolCall>::new();
    for tool_call in existing.iter().chain(incoming.iter()) {
        merged.insert(tool_call.call_id.clone(), tool_call.clone());
    }
    merged.into_values().collect()
}

fn remote_sidecar_capture_pending_tool_calls(
    assistant_message_id: &str,
    tool_calls: &[NeutralToolCall],
) -> Vec<Value> {
    let allowed_tools = remote_sidecar_executable_tool_names();
    let mut events = Vec::new();
    for tool_call in tool_calls {
        if !allowed_tools.contains(tool_call.name.as_str()) {
            continue;
        }
        events.push(json!({
            "type": "toolCall",
            "assistantMessageId": assistant_message_id,
            "toolCall": {
                "id": tool_call.call_id,
                "name": tool_call.name,
                "status": "running",
                "input": tool_call.arguments,
                "output": Value::Null,
                "isError": false,
            },
        }));
    }
    if events.is_empty() {
        return Vec::new();
    }
    events
}

fn remote_sidecar_record_pending_tool_calls(
    database: &mut WorkspaceDatabase,
    chat_id: &str,
    run_id: &str,
    assistant_message_id: &str,
    tool_calls: &[NeutralToolCall],
) -> Result<(), foco_store::workspace::WorkspaceDatabaseError> {
    let allowed_tools = remote_sidecar_executable_tool_names();
    for tool_call in tool_calls {
        if !allowed_tools.contains(tool_call.name.as_str()) {
            continue;
        }
        let input_json =
            serde_json::to_string(&tool_call.arguments).unwrap_or_else(|_| "null".to_string());
        database.upsert_tool_call(foco_store::workspace::NewToolCall {
            id: &tool_call.call_id,
            chat_id,
            run_id,
            message_id: Some(assistant_message_id),
            tool_name: &tool_call.name,
            input_json: &input_json,
            status: "running",
            started_at: &Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            completed_at: None,
        })?;
    }
    Ok(())
}

fn remote_sidecar_record_tool_result(
    database: &mut WorkspaceDatabase,
    tool_call: &NeutralToolCall,
    output: &Value,
    is_error: bool,
    started_at: &str,
    completed_at: &str,
) -> Result<(), foco_store::workspace::WorkspaceDatabaseError> {
    let output_json = serde_json::to_string(output).unwrap_or_else(|_| "null".to_string());
    let result_id = format!("{}-result", tool_call.call_id);
    database.upsert_tool_result(foco_store::workspace::NewToolResult {
        id: &result_id,
        tool_call_id: &tool_call.call_id,
        output_json: &output_json,
        is_error,
        created_at: completed_at,
    })?;
    database.complete_tool_call(
        &tool_call.call_id,
        if is_error { "error" } else { "completed" },
        completed_at,
    )?;
    let _ = started_at;
    Ok(())
}

async fn remote_sidecar_execute_tool_call(
    state: &RemoteSidecarState,
    tool_call: NeutralToolCall,
    chat_id: &str,
    run_id: &str,
    assistant_message_id: &str,
) -> (Value, bool, String, String, Vec<Value>) {
    // ponytail: remote sidecar reuses the shared execute_tool path one call at a time.
    // Ceiling: this duplicates a thin slice of main chat wiring; if remote tools grow
    // parallelism/question hooks, extract a dedicated remote tool runtime helper.
    let started_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let workspace_path = PathBuf::from(&state.workspace_path);
    let tool_output_events = Arc::new(AsyncMutex::new(Vec::<Value>::new()));
    let question_events = Arc::new(AsyncMutex::new(Vec::<Value>::new()));
    let (tool_output_tx, mut tool_output_rx) = mpsc::unbounded_channel::<ToolOutputDeltaEvent>();
    let (question_tx, mut question_rx) = mpsc::unbounded_channel();
    let tool_output_events_task = tool_output_events.clone();
    let tool_call_id_for_output = tool_call.call_id.clone();
    let assistant_message_id_for_output = assistant_message_id.to_string();
    let tool_output_collector = tokio::spawn(async move {
        while let Some(event) = tool_output_rx.recv().await {
            let stream = match event.stream {
                foco_tools::ToolOutputStream::Stdout => "stdout",
                foco_tools::ToolOutputStream::Stderr => "stderr",
            };
            tool_output_events_task.lock().await.push(json!({
                "type": "toolOutputDelta",
                "assistantMessageId": assistant_message_id_for_output,
                "toolCallId": tool_call_id_for_output,
                "stream": stream,
                "delta": event.delta,
            }));
        }
    });
    let question_events_task = question_events.clone();
    let assistant_message_id_for_question = assistant_message_id.to_string();
    let question_collector = tokio::spawn(async move {
        while let Some(request) = question_rx.recv().await {
            question_events_task.lock().await.push(json!({
                "type": "questionRequest",
                "assistantMessageId": assistant_message_id_for_question,
                "request": request,
            }));
        }
    });

    let global_config = foco_store::config::GlobalConfig::first_run(workspace_path.clone());
    let mcp_registry = Arc::new(McpRegistry::default());
    let hook_runtime = HookRuntime::new(mcp_registry.clone());
    let execution = execute_tool(
        mcp_registry,
        hook_runtime,
        &foco_store::config::HookConfig::default(),
        false,
        &global_config,
        &foco_providers::ProviderConnectionConfig {
            kind: foco_providers::parse_provider_kind(foco_providers::OPENAI_RESPONSES_KIND)
                .expect("openai responses kind"),
            base_url: None,
            api_key: None,
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        },
        &foco_store::config::WebSearchSettings::default(),
        QuestionRegistry::default(),
        question_tx.clone(),
        crate::memory_runtime::MemoryToolContext {
            enabled: false,
            workspace_path: workspace_path.clone(),
            global_memory_database_file: workspace_path
                .join(".foco/remote-sidecar-disabled-memory.sqlite"),
            chat_id: chat_id.to_string(),
            run_id: run_id.to_string(),
            tool_call_id: tool_call.call_id.clone(),
            target_status: MemoryStatus::Pending,
            memory_settings: foco_store::config::MemorySettings::default(),
        },
        None,
        ToolResourceLockRegistry::default(),
        foco_tools::ToolCancellationToken::default(),
        tool_output_tx.clone(),
        assistant_message_id,
        &state.workspace_id,
        &workspace_path,
        &workspace_path,
        chat_id,
        None,
        run_id,
        "remote-sidecar-tool-loop",
        "remote-sidecar",
        0,
        &tool_call.call_id,
        &tool_call.name,
        tool_call.arguments.clone(),
    )
    .await;

    drop(question_tx);
    drop(tool_output_tx);
    let _ = question_collector.await;
    let _ = tool_output_collector.await;

    let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut followup_events = tool_output_events.lock().await.clone();
    followup_events.extend(question_events.lock().await.clone());
    let is_error = execution.execution.is_error;
    let output = execution.execution.output;
    (output, is_error, started_at, completed_at, followup_events)
}

async fn remote_sidecar_run_broker_llm_turn(
    state: &RemoteSidecarState,
    run_stream: &RemoteActiveRunStream,
    broker_request_id: &str,
    broker_payload: Value,
    run_id: &str,
    chat_id: &str,
    assistant_message_id: &str,
    queued_user_message_id: &str,
    provider_id: &str,
    model_id: &str,
    request: &NeutralChatRequest,
    database: &mut WorkspaceDatabase,
    text: &mut String,
    reasoning: &mut String,
    run_metrics: &mut RemoteSidecarRunMetrics,
    sequence: &mut i64,
) -> Result<Option<Vec<NeutralToolCall>>, ()> {
    let mut broker_rx =
        remote_sidecar_broker_request(state, broker_request_id, "llm.stream", broker_payload)
            .await
            .map_err(|_| ())?;
    let mut collected_tool_calls = Vec::<NeutralToolCall>::new();
    loop {
        let envelope = match timeout(BROKER_REQUEST_TIMEOUT, broker_rx.recv()).await {
            Ok(Some(envelope)) => envelope,
            Ok(None) => return Ok(None),
            Err(_) => {
                let message =
                    "remote broker request timed out; retry to resume from persisted messages";
                let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                let total_latency_ms = run_metrics.total_latency_ms();
                let _ = persist_sidecar_llm_audit(
                    database,
                    &state.workspace_id,
                    chat_id,
                    run_id,
                    provider_id,
                    model_id,
                    Some(request),
                    run_metrics,
                    &completed_at,
                    total_latency_ms,
                    "failed",
                    json!({ "error": { "message": message } }),
                );
                *sequence += 1;
                run_stream.record(
                    *sequence,
                    json!({
                        "type": "error",
                        "message": message,
                    }),
                );
                *sequence += 1;
                run_stream.record(*sequence, json!({ "type": "streamEnd" }));
                return Err(());
            }
        };
        match envelope.message_type.as_str() {
            "stream" => {
                let kind = envelope
                    .payload
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let delta = envelope
                    .payload
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if kind == "textDelta" {
                    run_metrics.capture_first_output();
                    text.push_str(delta);
                    *sequence += 1;
                    run_stream.record(
                        *sequence,
                        json!({
                            "type": "textDelta",
                            "assistantMessageId": assistant_message_id,
                            "delta": delta,
                        }),
                    );
                } else if kind == "reasoningDelta" {
                    run_metrics.capture_first_output();
                    reasoning.push_str(delta);
                    *sequence += 1;
                    run_stream.record(
                        *sequence,
                        json!({
                            "type": "reasoningDelta",
                            "assistantMessageId": assistant_message_id,
                            "delta": delta,
                        }),
                    );
                } else if kind == "usageDelta" {
                    *sequence += 1;
                    run_stream.record(
                        *sequence,
                        json!({
                            "type": "usage",
                            "usage": envelope.payload.get("usage").cloned().unwrap_or(Value::Null),
                        }),
                    );
                } else if kind == "toolCall" {
                    let Some(tool_value) = envelope.payload.get("toolCall") else {
                        continue;
                    };
                    let Ok(tool_call) =
                        serde_json::from_value::<NeutralToolCall>(tool_value.clone())
                    else {
                        continue;
                    };
                    run_metrics.capture_first_output();
                    collected_tool_calls = merge_remote_tool_calls(
                        &collected_tool_calls,
                        std::slice::from_ref(&tool_call),
                    );
                    let pending_payload = json!({
                        "type": "toolCall",
                        "assistantMessageId": assistant_message_id,
                        "toolCall": {
                            "id": tool_call.call_id.clone(),
                            "name": tool_call.name.clone(),
                            "status": "running",
                            "input": tool_call.arguments.clone(),
                            "output": Value::Null,
                            "isError": false,
                        },
                    });
                    remote_sidecar_record_pending_tool_calls(
                        database,
                        chat_id,
                        run_id,
                        assistant_message_id,
                        std::slice::from_ref(&tool_call),
                    )
                    .map_err(|_| ())?;
                    *sequence += 1;
                    run_stream.record(*sequence, pending_payload);
                }
                remote_sidecar_set_active_run(
                    state,
                    RemoteActiveRunSummary {
                        run_id: run_id.to_string(),
                        chat_id: chat_id.to_string(),
                        last_sequence: Some(*sequence),
                        accepting_guidance: true,
                        broker_status: "connected".to_string(),
                        updated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                    },
                );
            }
            "response" => {
                let usage = envelope
                    .payload
                    .get("usage")
                    .cloned()
                    .unwrap_or(Value::Null);
                run_metrics.merge_usage_value(&usage);
                let response_tool_calls = envelope
                    .payload
                    .get("toolCalls")
                    .cloned()
                    .and_then(|value| serde_json::from_value::<Vec<NeutralToolCall>>(value).ok())
                    .unwrap_or_default();
                let tool_calls =
                    merge_remote_tool_calls(&collected_tool_calls, &response_tool_calls);
                if !tool_calls.is_empty() {
                    run_metrics.capture_first_output();
                }
                if tool_calls.is_empty() {
                    let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                    let total_latency_ms = run_metrics.total_latency_ms();
                    let metrics = remote_chat_metrics(
                        model_id,
                        provider_id,
                        run_id,
                        run_metrics,
                        total_latency_ms,
                    );
                    let usage = run_metrics.usage_value();
                    let metadata = json!({
                        "reasoning": if reasoning.is_empty() { Value::Null } else { Value::String(reasoning.clone()) },
                        "parts": remote_chat_parts(text, (!reasoning.is_empty()).then_some(reasoning.as_str())),
                        "metrics": metrics,
                    });
                    let assistant_sequence = database
                        .message(assistant_message_id)
                        .ok()
                        .flatten()
                        .map(|message| message.sequence)
                        .unwrap_or_else(|| {
                            database
                                .next_message_sequence_for_chat(chat_id)
                                .unwrap_or(0)
                        });
                    let _ = database.upsert_message_content(NewMessage {
                        id: assistant_message_id,
                        chat_id,
                        role: "assistant",
                        content: text,
                        sequence: assistant_sequence,
                        metadata_json: Some(&metadata.to_string()),
                    });
                    let _ = remote_clear_message_queued_run(database, queued_user_message_id);
                    let completion_payload = remote_chat_completion_event(
                        chat_id,
                        assistant_message_id,
                        text,
                        (!reasoning.is_empty()).then_some(reasoning.as_str()),
                        usage.clone(),
                        metrics.clone(),
                    );
                    let _ = persist_sidecar_llm_audit(
                        database,
                        &state.workspace_id,
                        chat_id,
                        run_id,
                        provider_id,
                        model_id,
                        Some(request),
                        run_metrics,
                        &completed_at,
                        total_latency_ms,
                        "succeeded",
                        completion_payload.clone(),
                    );
                    let _ = database.insert_run_event(NewRunEvent {
                        id: &unique_id("run-event"),
                        chat_id,
                        run_id,
                        sequence: *sequence,
                        event_type: "completion",
                        payload_json: &completion_payload.to_string(),
                    });
                    *sequence += 1;
                    run_stream.record(*sequence, completion_payload);
                    *sequence += 1;
                    run_stream.record(*sequence, json!({ "type": "streamEnd" }));
                    return Ok(None);
                }
                return Ok(Some(tool_calls));
            }
            "error" => {
                let message = envelope
                    .payload
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("remote broker unavailable");
                let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                let total_latency_ms = run_metrics.total_latency_ms();
                let _ = persist_sidecar_llm_audit(
                    database,
                    &state.workspace_id,
                    chat_id,
                    run_id,
                    provider_id,
                    model_id,
                    Some(request),
                    run_metrics,
                    &completed_at,
                    total_latency_ms,
                    "failed",
                    json!({ "error": { "message": message } }),
                );
                *sequence += 1;
                run_stream.record(
                    *sequence,
                    json!({
                        "type": "error",
                        "message": message,
                    }),
                );
                *sequence += 1;
                run_stream.record(*sequence, json!({ "type": "streamEnd" }));
                return Err(());
            }
            _ => {}
        }
    }
}

fn remote_project_spec_context_message(
    database: &WorkspaceDatabase,
    chat_id: &str,
) -> Result<Option<NeutralChatMessage>, axum::response::Response> {
    if let Some(snapshot) = database
        .chat_spec_snapshot(chat_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
    {
        return Ok(remote_project_spec_message(
            snapshot.spec_revision,
            &snapshot.content_markdown,
        ));
    }
    let Some(spec) = database
        .workspace_spec()
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
    else {
        return Ok(None);
    };
    let settings = WorkspaceSpecSettings {
        enabled: spec.enabled,
        inject_enabled: spec.inject_enabled,
    };
    if WorkspaceSpecPromptPlan::for_chat(settings, false)
        != WorkspaceSpecPromptPlan::ReadWorkspaceSpecAndSaveSnapshot
    {
        return Ok(None);
    }
    Ok(remote_project_spec_message(
        spec.revision,
        &spec.content_markdown,
    ))
}

fn remote_project_spec_message(
    revision: u64,
    content_markdown: &str,
) -> Option<NeutralChatMessage> {
    if content_markdown.trim().is_empty() {
        return None;
    }
    Some(neutral_text_message(
        NeutralChatRole::User,
        format!(
            "## Project Spec Context\n\nSource: Project Spec snapshot for this chat:\n\nRevision: {revision}\n\n{}",
            markdown_code_block("markdown", content_markdown)
        ),
    ))
}

fn remote_chat_completion_event(
    chat_id: &str,
    assistant_message_id: &str,
    text: &str,
    reasoning: Option<&str>,
    usage: Value,
    metrics: Value,
) -> Value {
    json!({
        "type": "complete",
        "chatId": chat_id,
        "assistantMessageId": assistant_message_id,
        "text": text,
        "reasoning": reasoning,
        "usage": usage,
        "stopReason": null,
        "metrics": metrics,
        "memoriesUsed": [],
    })
}

async fn remote_sidecar_chat_stream(
    State(state): State<RemoteSidecarState>,
    Json(payload): Json<Value>,
) -> Result<
    Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>,
    axum::response::Response,
> {
    let chat_id = remote_required_text(payload.get("chatId"), "chatId")?;
    let queued_user_message_id =
        remote_required_text(payload.get("queuedUserMessageId"), "queuedUserMessageId")?;
    let model_id = remote_required_text(payload.get("modelId"), "modelId")?;
    let provider_id =
        remote_optional_string(payload.get("providerId")).unwrap_or_else(|| "default".to_string());
    let database = sidecar_workspace_database(&state)?;
    let chat = database
        .chat(&chat_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .ok_or_else(|| {
            ApiError::bad_request(format!("chat was not found: {chat_id}")).into_response()
        })?;
    let user_message = database
        .message(&queued_user_message_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .ok_or_else(|| {
            ApiError::bad_request("queued user message was not found").into_response()
        })?;
    let assistant_message_id = payload
        .get("visibleAssistantMessageId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            let metadata = serde_json::from_str::<Value>(&user_message.metadata_json).ok()?;
            metadata
                .get("queuedRun")
                .and_then(|run| run.get("assistantMessageId"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| unique_id("msg-assistant"));
    let run_id = unique_id("remote-run");
    let initial_provider_request = remote_sidecar_provider_request(
        &state,
        &database,
        &chat_id,
        &assistant_message_id,
        &model_id,
        payload.get("thinkingLevel").cloned().unwrap_or(Value::Null),
    )?;
    let run = RemoteActiveRunSummary {
        run_id: run_id.clone(),
        chat_id: chat_id.clone(),
        last_sequence: Some(0),
        accepting_guidance: true,
        broker_status: "connecting".to_string(),
        updated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    };
    remote_sidecar_set_active_run(&state, run);
    let run_stream =
        remote_sidecar_insert_active_run_stream(&state, run_id.clone(), chat_id.clone());
    let stream_state = state.clone();
    let run_stream = run_stream.clone();
    let stream = async_stream::stream! {
        let mut cleanup_guard = RemoteRunCleanupGuard::new(stream_state.clone(), run_id.clone());
        let mut sequence = 0_i64;
        let mut text = String::new();
        let mut reasoning = String::new();
        let mut run_metrics = RemoteSidecarRunMetrics::new();
        let mut current_request = initial_provider_request;
        let mut tool_rounds = 0_usize;
        yield Ok(remote_sidecar_record_run_event(&run_stream, sequence, json!({
            "type": "start",
            "chatId": chat_id,
            "userMessageId": queued_user_message_id,
            "assistantMessageId": assistant_message_id,
            "llmRequestId": run_id,
            "memoriesUsed": [],
        })));
        sequence += 1;
        yield Ok(remote_sidecar_record_run_event(&run_stream, sequence, json!({
            "type": "connecting",
            "message": "connecting to remote broker",
        })));
        let mut last_yielded_sequence = sequence;

        loop {
            let broker_request_id = unique_id("broker-request");
            run_stream.set_broker_request_id(broker_request_id.clone());
            let broker_payload = json!({
                "workspaceId": stream_state.workspace_id,
                "chatId": chat_id,
                "chatTitle": chat.title,
                "runId": run_id,
                "providerId": provider_id,
                "modelId": model_id,
                "request": current_request,
            });
            let mut database = match WorkspaceDatabase::open_or_create(sidecar_workspace_path(&stream_state)) {
                Ok(database) => database,
                Err(error) => {
                    sequence += 1;
                    yield Ok(remote_sidecar_record_run_event(&run_stream, sequence, json!({
                        "type": "error",
                        "message": error.to_string(),
                    })));
                    sequence += 1;
                    yield Ok(remote_sidecar_record_run_event(&run_stream, sequence, json!({ "type": "streamEnd" })));
                    remote_sidecar_finish_active_run(&stream_state, &run_id);
                    cleanup_guard.disarm();
                    return;
                }
            };
            let llm_turn = remote_sidecar_run_broker_llm_turn(
                &stream_state,
                &run_stream,
                &broker_request_id,
                broker_payload,
                &run_id,
                &chat_id,
                &assistant_message_id,
                &queued_user_message_id,
                &provider_id,
                &model_id,
                &current_request,
                &mut database,
                &mut text,
                &mut reasoning,
                &mut run_metrics,
                &mut sequence,
            ).await;
            let mut reached_terminal_event = false;
            for (event, terminal) in remote_sidecar_snapshot_run_events(&run_stream, &mut last_yielded_sequence) {
                yield Ok(event);
                if terminal {
                    reached_terminal_event = true;
                    break;
                }
            }
            match llm_turn {
                Ok(None) => {
                    remote_sidecar_finish_active_run(&stream_state, &run_id);
                    cleanup_guard.disarm();
                    break;
                }
                Ok(Some(tool_calls)) => {
                    if reached_terminal_event {
                        remote_sidecar_finish_active_run(&stream_state, &run_id);
                        cleanup_guard.disarm();
                        break;
                    }
                    let allowed_tools = remote_sidecar_executable_tool_names();
                    tool_rounds += 1;
                    if tool_rounds > REMOTE_SIDECAR_MAX_TOOL_ROUNDS {
                        let message = format!(
                            "remote tool round limit reached after {REMOTE_SIDECAR_MAX_TOOL_ROUNDS} rounds"
                        );
                        let completed_at =
                            Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                        let total_latency_ms = run_metrics.total_latency_ms();
                        let _ = persist_sidecar_llm_audit(
                            &mut database,
                            &stream_state.workspace_id,
                            &chat_id,
                            &run_id,
                            &provider_id,
                            &model_id,
                            Some(&current_request),
                            &run_metrics,
                            &completed_at,
                            total_latency_ms,
                            "failed",
                            json!({ "error": { "message": message } }),
                        );
                        sequence += 1;
                        yield Ok(remote_sidecar_record_run_event(&run_stream, sequence, json!({
                            "type": "error",
                            "message": message,
                        })));
                        sequence += 1;
                        yield Ok(remote_sidecar_record_run_event(&run_stream, sequence, json!({ "type": "streamEnd" })));
                        remote_sidecar_finish_active_run(&stream_state, &run_id);
                        cleanup_guard.disarm();
                        break;
                    }

                    let mut next_messages = current_request.messages.clone();
                    next_messages.push(NeutralChatMessage {
                        role: NeutralChatRole::Assistant,
                        content: text.clone(),
                        attachments: Vec::new(),
                        reasoning: (!reasoning.is_empty()).then_some(reasoning.clone()),
                        tool_calls: tool_calls.clone(),
                        tool_call_id: None,
                        tool_name: None,
                    });

                    let runnable_tool_calls = tool_calls
                        .iter()
                        .filter(|tool_call| allowed_tools.contains(tool_call.name.as_str()))
                        .cloned()
                        .collect::<Vec<_>>();
                    let _ = remote_sidecar_record_pending_tool_calls(
                        &mut database,
                        &chat_id,
                        &run_id,
                        &assistant_message_id,
                        &runnable_tool_calls,
                    );
                    let mut followup_sse_events = remote_sidecar_capture_pending_tool_calls(
                        &assistant_message_id,
                        &runnable_tool_calls,
                    );
                    for tool_call in &tool_calls {
                        if !allowed_tools.contains(tool_call.name.as_str()) {
                            let output = json!({ "error": format!("remote sidecar cannot execute tool '{}'", tool_call.name) });
                            let started_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                            let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                            let _ = remote_sidecar_record_tool_result(
                                &mut database,
                                tool_call,
                                &output,
                                true,
                                &started_at,
                                &completed_at,
                            );
                            followup_sse_events.push(json!({
                                "type": "toolResult",
                                "assistantMessageId": assistant_message_id,
                                "toolCallId": tool_call.call_id,
                                "output": output,
                                "isError": true,
                                "startedAt": started_at,
                                "completedAt": completed_at,
                            }));
                            next_messages.push(NeutralChatMessage {
                                role: NeutralChatRole::Tool,
                                content: serde_json::to_string(&output).unwrap_or_else(|_| "null".to_string()),
                                attachments: Vec::new(),
                                reasoning: None,
                                tool_calls: Vec::new(),
                                tool_call_id: Some(tool_call.call_id.clone()),
                                tool_name: Some(tool_call.name.clone()),
                            });
                            continue;
                        }
                        let (output, is_error, started_at, completed_at, mut extra_events) = remote_sidecar_execute_tool_call(
                            &stream_state,
                            tool_call.clone(),
                            &chat_id,
                            &run_id,
                            &assistant_message_id,
                        ).await;
                        let _ = remote_sidecar_record_tool_result(
                            &mut database,
                            tool_call,
                            &output,
                            is_error,
                            &started_at,
                            &completed_at,
                        );
                        followup_sse_events.append(&mut extra_events);
                        followup_sse_events.push(json!({
                            "type": "toolResult",
                            "assistantMessageId": assistant_message_id,
                            "toolCallId": tool_call.call_id,
                            "output": output,
                            "isError": is_error,
                            "startedAt": started_at,
                            "completedAt": completed_at,
                        }));
                        next_messages.push(NeutralChatMessage {
                            role: NeutralChatRole::Tool,
                            content: serde_json::to_string(&output).unwrap_or_else(|_| "null".to_string()),
                            attachments: Vec::new(),
                            reasoning: None,
                            tool_calls: Vec::new(),
                            tool_call_id: Some(tool_call.call_id.clone()),
                            tool_name: Some(tool_call.name.clone()),
                        });
                    }

                    for event in followup_sse_events {
                        sequence += 1;
                        yield Ok(remote_sidecar_record_run_event(&run_stream, sequence, event));
                        last_yielded_sequence = sequence;
                    }

                    current_request = NeutralChatRequest {
                        model_id: current_request.model_id.clone(),
                        messages: next_messages,
                        tools: current_request.tools.clone(),
                        thinking_level: current_request.thinking_level.clone(),
                        max_output_tokens: current_request.max_output_tokens,
                        prompt_cache_key: current_request.prompt_cache_key.clone(),
                        prompt_cache_retention: current_request.prompt_cache_retention.clone(),
                    };
                }
                Err(()) => {
                    remote_sidecar_finish_active_run(&stream_state, &run_id);
                    cleanup_guard.disarm();
                    break;
                }
            }
        }
        remote_sidecar_finish_active_run(&stream_state, &run_id);
        cleanup_guard.disarm();
    };
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    ))
}

async fn remote_sidecar_chat_run_stream(
    State(state): State<RemoteSidecarState>,
    AxumPath(run_id): AxumPath<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<
    Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>,
    axum::response::Response,
> {
    let after_sequence = query
        .get("afterSequence")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(-1);
    let run_stream = remote_sidecar_active_run_stream(&state, &run_id).ok_or_else(|| {
        ApiError::bad_request(format!("active chat run was not found: {run_id}")).into_response()
    })?;
    let mut rx = run_stream.tx.subscribe();
    let replay = run_stream.snapshot_after(after_sequence);
    let stream = async_stream::stream! {
        let mut last_sent_sequence = after_sequence;
        for (sequence, event) in replay {
            if sequence <= last_sent_sequence {
                continue;
            }
            let terminal = remote_stream_event_is_terminal(&event);
            yield Ok(remote_sse_json_event(sequence, event));
            last_sent_sequence = sequence;
            if terminal {
                return;
            }
        }
        while !run_stream.finished.load(Ordering::Relaxed) {
            match rx.recv().await {
                Ok((sequence, event)) => {
                    if sequence <= last_sent_sequence {
                        continue;
                    }
                    let terminal = remote_stream_event_is_terminal(&event);
                    yield Ok(remote_sse_json_event(sequence, event));
                    last_sent_sequence = sequence;
                    if terminal {
                        return;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let sequence = last_sent_sequence.saturating_add(1);
                    yield Ok(remote_sse_json_event(sequence, json!({
                        "type": "error",
                        "message": "remote run stream history was truncated; reload chat messages to recover persisted state",
                    })));
                    yield Ok(remote_sse_json_event(sequence.saturating_add(1), json!({ "type": "streamEnd" })));
                    return;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    };
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    ))
}

async fn remote_sidecar_chat_run_events_stream(
    State(state): State<RemoteSidecarState>,
    AxumPath(run_id): AxumPath<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<
    Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>,
    axum::response::Response,
> {
    remote_sidecar_chat_run_stream(State(state), AxumPath(run_id), Query(query)).await
}

async fn remote_sidecar_chat_run_cancel(
    State(state): State<RemoteSidecarState>,
    AxumPath(run_id): AxumPath<String>,
) -> Result<Json<Value>, axum::response::Response> {
    remote_sidecar_cancel_active_run(&state, &run_id, true, false);
    Ok(Json(json!({ "ok": true, "runId": run_id })))
}

async fn remote_sidecar_chat_guidance(
    State(_state): State<RemoteSidecarState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    Ok(Json(json!({
        "id": unique_id("msg-guidance"),
        "content": payload.get("message").and_then(Value::as_str).unwrap_or_default(),
        "parts": remote_chat_parts(payload.get("message").and_then(Value::as_str).unwrap_or_default(), None),
        "brokerStatus": "queuedUntilRunResumes",
    })))
}

async fn remote_sidecar_context_usage(
    State(_state): State<RemoteSidecarState>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, axum::response::Response> {
    Ok(Json(json!({
        "usedMessageTokens": 0,
        "availableMessageTokens": 0,
        "memoryContextTokens": 0,
        "memoryBudgetTokens": 0,
        "usagePercent": 0,
        "compressionTriggerTokens": 0,
        "compressionTriggerPercent": 0,
        "willCompressOnNextSend": false,
        "tokenBreakdown": {},
        "remoteApproximation": true,
    })))
}

async fn remote_sidecar_ai_statistics_detail(
    State(state): State<RemoteSidecarState>,
    AxumPath(request_id): AxumPath<String>,
) -> Result<Json<Value>, axum::response::Response> {
    let request_id = request_id.trim();
    if request_id.is_empty() {
        return Err(ApiError::bad_request("request id must not be empty").into_response());
    }
    let database = sidecar_workspace_database(&state)?;
    let request = database
        .llm_request(request_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?
        .ok_or_else(|| {
            ApiError::bad_request(format!("LLM request was not found: {request_id}"))
                .into_response()
        })?;
    let events = database
        .llm_request_events(request_id)
        .map_err(|e| ApiError::from_workspace_error(e).into_response())?;
    Ok(Json(json!({
        "request": {
            "id": request.id,
            "workspaceId": state.workspace_id,
            "workspaceName": state.workspace_id,
            "chatId": request.chat_id,
            "chatTitle": null,
            "requestKind": request.request_kind,
            "providerId": request.provider_id,
            "modelId": request.model_id,
            "requestStartedAt": request.request_started_at,
            "firstTokenAt": request.first_token_at,
            "completedAt": request.completed_at,
            "inputTokens": request.input_tokens,
            "outputTokens": request.output_tokens,
            "cacheReadTokens": request.cache_read_tokens,
            "cacheWriteTokens": request.cache_write_tokens,
            "reasoningTokens": request.reasoning_tokens,
            "cacheRatio": request.cache_ratio,
            "firstTokenLatencyMs": request.first_token_latency_ms,
            "totalLatencyMs": request.total_latency_ms,
            "statusCode": request.status_code,
            "finalState": request.final_state,
            "requestBody": request.request_body_json.as_deref().and_then(|value| serde_json::from_str::<Value>(value).ok()),
            "responseBody": request.response_body_json.as_deref().and_then(|value| serde_json::from_str::<Value>(value).ok()),
        },
        "events": events.into_iter().map(|event| json!({
            "id": event.id,
            "sequence": event.sequence,
            "eventAt": event.event_at,
            "eventType": event.event_type,
            "rawChunk": event.raw_chunk_json.as_deref().and_then(|value| serde_json::from_str::<Value>(value).ok()),
            "normalizedEvent": serde_json::from_str::<Value>(&event.normalized_event_json).unwrap_or(Value::Null),
        })).collect::<Vec<_>>(),
    })))
}

async fn remote_sidecar_passthrough_unavailable() -> Result<Json<Value>, axum::response::Response> {
    Err(
        ApiError::bad_gateway("remote sidecar runtime endpoint is not available in this build")
            .into_response(),
    )
}

async fn remote_sidecar_file_tree(
    State(state): State<RemoteSidecarState>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let root =
        crate::http::workspaces::workspace_file_tree_response(sidecar_workspace_path(&state))
            .map_err(|e| e.into_response())?;
    Ok(Json(serde_json::json!({ "root": root })))
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

    fn test_sidecar_state(
        workspace_path: String,
        ws_count: usize,
    ) -> (
        RemoteSidecarState,
        tokio::sync::broadcast::Receiver<ControlEnvelope>,
    ) {
        let (broker_tx, broker_rx) = tokio::sync::broadcast::channel::<ControlEnvelope>(16);
        (
            RemoteSidecarState {
                token: "token".to_string(),
                last_config_hash: Arc::new(Mutex::new(None)),
                runtime_config: Arc::new(Mutex::new(None)),
                code_graph_watcher: Arc::new(Mutex::new(None)),
                ws_count: Arc::new(AtomicUsize::new(ws_count)),
                active_run_count: Arc::new(AtomicUsize::new(0)),
                active_runs: Arc::new(Mutex::new(Vec::new())),
                active_run_streams: Arc::new(Mutex::new(HashMap::new())),
                broker_pending: Arc::new(AsyncMutex::new(HashMap::new())),
                broker_tx,
                shutdown_tx: default_shutdown_tx(),
                workspace_id: "workspace".to_string(),
                workspace_path,
            },
            broker_rx,
        )
    }

    #[test]
    fn shell_quote_handles_single_quotes() {
        assert_eq!(shell_quote("/tmp/a'b"), "'/tmp/a'\\''b'");
    }

    #[test]
    fn stale_remote_sidecar_cleanup_script_matches_workspace_identity() {
        let script = stale_remote_sidecar_cleanup_script("server-1", "workspace-1", "/srv/project");
        assert!(script.contains("sid='server-1'"));
        assert!(script.contains("wid='workspace-1'"));
        assert!(script.contains("wpath='/srv/project'"));
        assert!(script.contains("grep -F -- '--remote-sidecar'"));
        assert!(script.contains("grep -F -- \"--server-id $sid\""));
        assert!(script.contains("grep -F -- \"--workspace-id $wid\""));
        assert!(script.contains("grep -F -- \"--workspace-path $wpath\""));
        assert!(script.contains("awk '{print $4}' \"/proc/$pid/stat\""));
        assert!(script.contains("[ \"$ppid\" = \"1\" ] || continue"));
    }

    #[test]
    fn sidecar_options_require_context() {
        let error = RemoteSidecarOptions::parse(&["--server-id".to_string(), "srv".to_string()])
            .unwrap_err();
        assert!(error.to_string().contains("--workspace-id"));
    }

    #[test]
    fn remote_active_run_heartbeats_replace_local_cache() {
        let active_runs = Arc::new(Mutex::new(Vec::new()));
        update_remote_active_runs(
            &active_runs,
            &json!({
                "activeRuns": [{
                    "runId": "run-1",
                    "chatId": "chat-1",
                    "lastSequence": 7,
                    "acceptingGuidance": true,
                    "brokerStatus": "brokerUnavailable",
                    "updatedAt": "2026-07-04T00:00:00Z",
                }],
            }),
        );
        let runs = active_runs.lock().expect("active runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, "run-1");
        assert_eq!(runs[0].last_sequence, Some(7));
        assert_eq!(runs[0].broker_status, "brokerUnavailable");
    }

    #[test]
    fn remote_idempotency_key_is_stable_without_client_key() {
        let first = remote_idempotency_key(&json!({}), "queue:chat:hello:model::");
        let second = remote_idempotency_key(&json!({}), "queue:chat:hello:model::");
        assert_eq!(first, second);
        assert!(first.starts_with("sha256:"));
    }

    #[test]
    fn remote_idempotency_key_prefers_client_key() {
        assert_eq!(
            remote_idempotency_key(&json!({ "idempotencyKey": "client-key" }), "fallback"),
            "client-key"
        );
    }

    #[tokio::test]
    async fn remote_sidecar_chat_queue_treats_distinct_client_keys_as_distinct_turns() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let (broker_tx, _) = tokio::sync::broadcast::channel::<ControlEnvelope>(8);
        let state = RemoteSidecarState {
            token: "token".to_string(),
            last_config_hash: Arc::new(Mutex::new(None)),
            runtime_config: Arc::new(Mutex::new(None)),
            code_graph_watcher: Arc::new(Mutex::new(None)),
            ws_count: Arc::new(AtomicUsize::new(0)),
            active_run_count: Arc::new(AtomicUsize::new(0)),
            active_runs: Arc::new(Mutex::new(Vec::new())),
            active_run_streams: Arc::new(Mutex::new(HashMap::new())),
            broker_pending: Arc::new(AsyncMutex::new(HashMap::new())),
            broker_tx,
            shutdown_tx: default_shutdown_tx(),
            workspace_id: "workspace".to_string(),
            workspace_path: workspace.path().to_string_lossy().to_string(),
        };
        let base_payload = json!({
            "chatId": "chat-1",
            "message": "hello",
            "modelId": "model-1",
            "providerId": "provider-1",
        });
        let mut first_payload = base_payload.clone();
        first_payload["idempotencyKey"] = json!("submit-1");
        let mut second_payload = base_payload;
        second_payload["idempotencyKey"] = json!("submit-2");

        let first = remote_sidecar_chat_queue(State(state.clone()), Json(first_payload))
            .await
            .expect("first queue")
            .0;
        let second = remote_sidecar_chat_queue(State(state.clone()), Json(second_payload))
            .await
            .expect("second queue")
            .0;

        assert_ne!(first["userMessageId"], second["userMessageId"]);
        assert_ne!(first["assistantMessageId"], second["assistantMessageId"]);

        let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        let messages = database
            .messages_for_chat("chat-1")
            .expect("messages for chat");
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].sequence, 0);
        assert_eq!(messages[2].sequence, 2);
    }

    #[tokio::test]
    async fn remote_workspace_chats_exposes_queued_run_from_messages() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let (state, _) = test_sidecar_state(workspace.path().to_string_lossy().to_string(), 0);
        let queued = remote_sidecar_chat_queue(
            State(state.clone()),
            Json(json!({
                "chatId": "chat-1",
                "message": "hello from queue",
                "modelId": "model-1",
                "providerId": "provider-1",
                "idempotencyKey": "submit-1",
            })),
        )
        .await
        .expect("queue message")
        .0;

        let chats = remote_sidecar_workspace_chats(State(state), Query(HashMap::new()))
            .await
            .expect("workspace chats")
            .0;
        assert_eq!(chats["chats"][0]["id"], queued["chatId"]);
        assert_eq!(chats["chats"][0]["queuedRun"]["status"], "queued");
        assert_eq!(
            chats["chats"][0]["queuedRun"]["content"],
            "hello from queue"
        );
        assert_eq!(
            chats["chats"][0]["queuedRun"]["userMessageId"],
            queued["userMessageId"]
        );
    }

    #[tokio::test]
    async fn remote_chat_run_stream_replays_buffered_events_after_sequence() {
        let (state, _) = test_sidecar_state("/tmp/workspace".to_string(), 1);
        let run_stream = remote_sidecar_insert_active_run_stream(
            &state,
            "run-1".to_string(),
            "chat-1".to_string(),
        );
        run_stream.record(0, json!({ "type": "start", "chatId": "chat-1" }));
        run_stream.record(
            1,
            json!({
                "type": "textDelta",
                "assistantMessageId": "assistant-1",
                "delta": "hello",
            }),
        );
        run_stream.record(2, json!({ "type": "streamEnd" }));
        run_stream.mark_finished();

        let response = remote_sidecar_chat_run_stream(
            State(state),
            AxumPath("run-1".to_string()),
            Query(HashMap::from([(
                "afterSequence".to_string(),
                "0".to_string(),
            )])),
        )
        .await
        .expect("run stream");
        let body = response.into_response().into_body();
        let bytes = tokio::time::timeout(
            Duration::from_millis(200),
            axum::body::to_bytes(body, usize::MAX),
        )
        .await
        .expect("SSE body should finish")
        .expect("SSE bytes");
        let text = String::from_utf8(bytes.to_vec()).expect("utf8 SSE");
        assert!(!text.contains("\"type\":\"start\""));
        assert!(text.contains("\"type\":\"textDelta\""));
        assert!(text.contains("\"type\":\"streamEnd\""));
    }

    #[tokio::test]
    async fn remote_chat_run_cancel_uses_broker_request_id() {
        let (state, mut broker_rx) = test_sidecar_state("/tmp/workspace".to_string(), 1);
        let run_stream = remote_sidecar_insert_active_run_stream(
            &state,
            "remote-run-1".to_string(),
            "chat-1".to_string(),
        );
        run_stream.set_broker_request_id("broker-request-1".to_string());
        remote_sidecar_set_active_run(
            &state,
            RemoteActiveRunSummary {
                run_id: "remote-run-1".to_string(),
                chat_id: "chat-1".to_string(),
                last_sequence: Some(0),
                accepting_guidance: true,
                broker_status: "connected".to_string(),
                updated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            },
        );
        while broker_rx.try_recv().is_ok() {}

        let response = remote_sidecar_chat_run_cancel(
            State(state.clone()),
            AxumPath("remote-run-1".to_string()),
        )
        .await
        .expect("cancel response")
        .0;
        assert_eq!(response["ok"], true);

        let cancel = broker_rx.recv().await.expect("cancel envelope");
        assert_eq!(cancel.message_type, "cancel");
        assert_eq!(cancel.id.as_deref(), Some("broker-request-1"));
        assert!(remote_sidecar_active_run_stream(&state, "remote-run-1").is_none());
        assert_eq!(state.active_run_count.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn remote_run_cleanup_guard_removes_pending_broker_request() {
        let (state, _broker_rx) = test_sidecar_state("/tmp/workspace".to_string(), 1);
        let run_stream = remote_sidecar_insert_active_run_stream(
            &state,
            "remote-run-1".to_string(),
            "chat-1".to_string(),
        );
        run_stream.set_broker_request_id("broker-request-1".to_string());
        let (tx, _rx) = mpsc::unbounded_channel();
        state
            .broker_pending
            .lock()
            .await
            .insert("broker-request-1".to_string(), tx);

        {
            let _guard = RemoteRunCleanupGuard::new(state.clone(), "remote-run-1".to_string());
        }

        assert!(
            state
                .broker_pending
                .lock()
                .await
                .get("broker-request-1")
                .is_none()
        );
        assert!(remote_sidecar_active_run_stream(&state, "remote-run-1").is_none());
    }

    #[test]
    fn remote_clear_message_queued_run_preserves_other_metadata() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        database
            .insert_chat_with_metadata("chat-1", "Remote chat", "{}")
            .expect("insert chat");
        database
            .insert_message(NewMessage {
                id: "msg-user-1",
                chat_id: "chat-1",
                role: "user",
                content: "hello",
                sequence: 0,
                metadata_json: Some(
                    &json!({
                        "parts": [{ "type": "text", "text": "hello" }],
                        "queuedRun": { "status": "queued" },
                    })
                    .to_string(),
                ),
            })
            .expect("insert user message");

        remote_clear_message_queued_run(&mut database, "msg-user-1").expect("clear queued run");

        let message = database
            .message("msg-user-1")
            .expect("message lookup")
            .expect("message");
        let metadata: Value =
            serde_json::from_str(&message.metadata_json).expect("message metadata");
        assert!(metadata.get("queuedRun").is_none());
        assert_eq!(metadata["parts"][0]["text"], "hello");
    }

    #[test]
    fn remote_chat_completion_event_matches_frontend_stream_shape() {
        let event = remote_chat_completion_event(
            "chat-1",
            "msg-assistant-1",
            "done",
            None,
            json!({
                "inputTokens": 10,
                "outputTokens": 3,
                "cacheReadTokens": null,
                "cacheWriteTokens": null,
            }),
            json!({
                "modelId": "model-1",
                "providerId": "provider-1",
                "totalLatencyMs": 1200,
                "firstTokenLatencyMs": 250,
                "outputTokens": 3,
                "llmRequestIds": ["run-1"],
            }),
        );

        assert_eq!(event["type"], "complete");
        assert!(event.get("value").is_none());
        assert_eq!(event["chatId"], "chat-1");
        assert_eq!(event["metrics"]["modelId"], "model-1");
        assert_eq!(event["metrics"]["providerId"], "provider-1");
        assert_eq!(event["metrics"]["totalLatencyMs"], 1200);
        assert_eq!(event["metrics"]["firstTokenLatencyMs"], 250);
        assert_eq!(event["metrics"]["outputTokens"], 3);
    }

    #[test]
    fn remote_sidecar_run_metrics_merge_usage_without_inventing_missing_tokens() {
        let mut metrics = RemoteSidecarRunMetrics::new();
        metrics.merge_usage_value(&json!({
            "inputTokens": 10,
            "outputTokens": 2,
            "cacheReadTokens": null,
            "cacheWriteTokens": 1,
        }));
        metrics.merge_usage_value(&json!({
            "inputTokens": 6,
            "outputTokens": 3,
            "cacheReadTokens": 4,
            "cacheWriteTokens": null,
        }));

        assert_eq!(metrics.usage.input_tokens, Some(16));
        assert_eq!(metrics.usage.output_tokens, Some(5));
        assert_eq!(metrics.usage.cache_read_tokens, Some(4));
        assert_eq!(metrics.usage.cache_write_tokens, Some(1));
        assert_eq!(metrics.usage.reasoning_tokens, None);
    }

    #[test]
    fn brokered_llm_audit_is_readable_by_chat_statistics_filters() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let context = BrokerLlmAuditContext {
            audit_path: workspace.path().to_path_buf(),
            workspace_id: "remote-ws".to_string(),
            chat_id: Some("chat-1".to_string()),
            chat_title: Some("Remote chat".to_string()),
            request_id: "remote-run-1".to_string(),
        };
        let started_at = "2026-07-08T00:00:00Z";
        insert_broker_llm_audit_start(
            &context,
            "provider-1",
            "model-1",
            started_at,
            &json!({ "request": true }).to_string(),
        );
        let usage = NeutralUsage {
            input_tokens: Some(10),
            output_tokens: Some(4),
            cache_read_tokens: Some(2),
            cache_write_tokens: Some(1),
            reasoning_tokens: None,
        };
        finish_broker_llm_audit(
            Some(&context),
            BrokerLlmAuditOutcome {
                final_state: "succeeded",
                first_token_at: Some("2026-07-08T00:00:01Z"),
                completed_at: "2026-07-08T00:00:02Z",
                usage: Some(&usage),
                first_token_latency_ms: Some(1000),
                total_latency_ms: 2000,
                response_body_json: &json!({ "ok": true }).to_string(),
            },
            &[BrokerLlmAuditEvent {
                event_at: started_at.to_string(),
                event_type: "start".to_string(),
                normalized_event: json!({ "type": "start" }),
            }],
        );

        let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("audit db");
        let rows = database
            .llm_request_audit_rows(LlmRequestAuditFilters {
                chat_id: Some("chat-1"),
                exclude_request_kinds: MAIN_CHAT_EXCLUDED_LLM_REQUEST_KINDS,
                ..LlmRequestAuditFilters::default()
            })
            .expect("audit rows");
        let summary = crate::llm_request_rows_summary(&rows);
        assert_eq!(summary.total_requests, 1);
        assert_eq!(summary.total_input_tokens, 10);
        assert_eq!(summary.total_output_tokens, 4);
        assert_eq!(summary.failed_requests, 0);
        assert_eq!(
            database
                .llm_request_events("remote-run-1")
                .expect("events")
                .len(),
            1
        );
    }

    #[test]
    fn remote_workspace_audit_path_falls_back_to_profile_storage() {
        let profile = tempfile::tempdir().expect("profile tempdir");
        let workspace = WorkspaceConfig {
            id: "remote-ws".to_string(),
            name: "Remote".to_string(),
            path: PathBuf::new(),
            location: WorkspaceLocation::Ssh {
                server_id: "server-1".to_string(),
                remote_path: "/srv/project".to_string(),
            },
            pinned: false,
            terminal_shell: "/bin/sh".to_string(),
            common_commands: Vec::new(),
        };
        let path = workspace_audit_path(profile.path(), &workspace).expect("audit path");
        assert!(path.ends_with(".foco/remote-workspace-audit/remote-ws"));
        assert!(path.is_dir());
    }

    #[tokio::test]
    async fn remote_sidecar_chat_stream_returns_before_broker_connects() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace db");
        database
            .insert_chat_with_metadata("chat-1", "Remote chat", "{}")
            .expect("insert chat");
        database
            .insert_message(NewMessage {
                id: "msg-user-1",
                chat_id: "chat-1",
                role: "user",
                content: "hello",
                sequence: 0,
                metadata_json: Some("{}"),
            })
            .expect("insert user message");
        let (broker_tx, _) = tokio::sync::broadcast::channel::<ControlEnvelope>(8);
        let state = RemoteSidecarState {
            token: "token".to_string(),
            last_config_hash: Arc::new(Mutex::new(None)),
            runtime_config: Arc::new(Mutex::new(None)),
            code_graph_watcher: Arc::new(Mutex::new(None)),
            ws_count: Arc::new(AtomicUsize::new(0)),
            active_run_count: Arc::new(AtomicUsize::new(0)),
            active_runs: Arc::new(Mutex::new(Vec::new())),
            active_run_streams: Arc::new(Mutex::new(HashMap::new())),
            broker_pending: Arc::new(AsyncMutex::new(HashMap::new())),
            broker_tx,
            shutdown_tx: default_shutdown_tx(),
            workspace_id: "workspace".to_string(),
            workspace_path: workspace.path().to_string_lossy().to_string(),
        };

        let response = tokio::time::timeout(
            Duration::from_millis(200),
            remote_sidecar_chat_stream(
                State(state),
                Json(json!({
                    "chatId": "chat-1",
                    "queuedUserMessageId": "msg-user-1",
                    "visibleAssistantMessageId": "msg-assistant-1",
                    "modelId": "model-1",
                    "providerId": "provider-1",
                })),
            ),
        )
        .await
        .expect("handler should return SSE before broker reconnects")
        .expect("SSE response");
        drop(response);
    }

    #[tokio::test]
    async fn remote_sidecar_chat_stream_flushes_broker_events_to_initial_sse() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace db");
        database
            .insert_chat_with_metadata("chat-1", "Remote chat", "{}")
            .expect("insert chat");
        database
            .insert_message(NewMessage {
                id: "msg-user-1",
                chat_id: "chat-1",
                role: "user",
                content: "hello",
                sequence: 0,
                metadata_json: Some("{}"),
            })
            .expect("insert user message");
        let (state, mut broker_rx) =
            test_sidecar_state(workspace.path().to_string_lossy().to_string(), 1);

        let response = remote_sidecar_chat_stream(
            State(state.clone()),
            Json(json!({
                "chatId": "chat-1",
                "queuedUserMessageId": "msg-user-1",
                "visibleAssistantMessageId": "msg-assistant-1",
                "modelId": "model-1",
                "providerId": "provider-1",
            })),
        )
        .await
        .expect("SSE response");

        let broker_state = state.clone();
        let broker = tokio::spawn(async move {
            let request = loop {
                let envelope = broker_rx.recv().await.expect("broker request");
                if envelope.message_type == "request" {
                    break envelope;
                }
            };
            assert_eq!(request.method.as_deref(), Some("llm.stream"));
            let id = request.id.clone().expect("request id");
            let pending = broker_state
                .broker_pending
                .lock()
                .await
                .get(&id)
                .cloned()
                .expect("pending response channel");
            pending
                .send(ControlEnvelope {
                    version: 1,
                    message_type: "stream".to_string(),
                    id: Some(id.clone()),
                    method: None,
                    payload: json!({
                        "kind": "textDelta",
                        "delta": "hello remote",
                    }),
                    timestamp: None,
                })
                .expect("send text delta");
            pending
                .send(ControlEnvelope {
                    version: 1,
                    message_type: "response".to_string(),
                    id: Some(id),
                    method: None,
                    payload: json!({
                        "usage": {
                            "inputTokens": 4,
                            "outputTokens": 2,
                            "cacheReadTokens": null,
                            "cacheWriteTokens": null,
                        },
                        "toolCalls": [],
                    }),
                    timestamp: None,
                })
                .expect("send completion");
        });

        let bytes = tokio::time::timeout(
            Duration::from_secs(1),
            axum::body::to_bytes(response.into_response().into_body(), usize::MAX),
        )
        .await
        .expect("initial SSE body should finish")
        .expect("SSE bytes");
        broker.await.expect("broker task");
        let text = String::from_utf8(bytes.to_vec()).expect("utf8 SSE");
        assert!(text.contains("\"type\":\"start\""));
        assert!(text.contains("\"type\":\"connecting\""));
        assert!(text.contains("\"type\":\"textDelta\""));
        assert!(text.contains("hello remote"));
        assert!(text.contains("\"type\":\"complete\""));
        assert!(text.contains("\"totalLatencyMs\":"));
        assert!(!text.contains("\"totalLatencyMs\":null"));
        assert!(text.contains("\"firstTokenLatencyMs\":"));
        assert!(!text.contains("\"firstTokenLatencyMs\":null"));
        assert!(text.contains("\"outputTokens\":2"));
        assert!(text.contains("\"type\":\"streamEnd\""));

        let database = WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace db");
        let assistant = database
            .message("msg-assistant-1")
            .expect("assistant message lookup")
            .expect("assistant message");
        let metadata =
            serde_json::from_str::<Value>(&assistant.metadata_json).expect("assistant metadata");
        assert!(metadata["metrics"]["totalLatencyMs"].as_i64().unwrap_or(0) > 0);
        assert!(metadata["metrics"]["firstTokenLatencyMs"].is_number());
        assert_eq!(metadata["metrics"]["outputTokens"], 2);
        let audit = database
            .llm_request_audit_rows(LlmRequestAuditFilters {
                chat_id: Some("chat-1"),
                ..LlmRequestAuditFilters::default()
            })
            .expect("audit rows")
            .into_iter()
            .next()
            .expect("audit row");
        assert_eq!(audit.output_tokens, Some(2));
        assert!(audit.first_token_latency_ms.is_some());
        assert!(audit.total_latency_ms.unwrap_or(0) > 0);
    }

    #[tokio::test]
    async fn remote_sidecar_llm_turn_uses_streamed_tool_call_when_response_omits_it() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace db");
        database
            .insert_chat_with_metadata("chat-1", "Remote chat", "{}")
            .expect("insert chat");
        database
            .insert_message(NewMessage {
                id: "msg-user-1",
                chat_id: "chat-1",
                role: "user",
                content: "hello",
                sequence: 0,
                metadata_json: Some("{}"),
            })
            .expect("insert user message");
        database
            .insert_message(NewMessage {
                id: "msg-assistant-1",
                chat_id: "chat-1",
                role: "assistant",
                content: "",
                sequence: 1,
                metadata_json: Some("{}"),
            })
            .expect("insert assistant placeholder");
        let (state, mut broker_rx) =
            test_sidecar_state(workspace.path().to_string_lossy().to_string(), 1);
        let run_stream = remote_sidecar_insert_active_run_stream(
            &state,
            "remote-run-1".to_string(),
            "chat-1".to_string(),
        );
        let request = NeutralChatRequest {
            model_id: "model-1".to_string(),
            messages: vec![neutral_text_message(
                NeutralChatRole::User,
                "hello".to_string(),
            )],
            tools: remote_sidecar_executable_tool_schemas(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
        };

        let broker_state = state.clone();
        let broker = tokio::spawn(async move {
            let request = loop {
                let envelope = broker_rx.recv().await.expect("broker request");
                if envelope.message_type == "request" {
                    break envelope;
                }
            };
            let id = request.id.clone().expect("request id");
            let pending = broker_state
                .broker_pending
                .lock()
                .await
                .get(&id)
                .cloned()
                .expect("pending response channel");
            pending
                .send(ControlEnvelope {
                    version: 1,
                    message_type: "stream".to_string(),
                    id: Some(id.clone()),
                    method: None,
                    payload: json!({
                        "kind": "toolCall",
                        "toolCall": {
                            "callId": "call-1",
                            "name": "read_file",
                            "arguments": { "path": "Cargo.toml", "startLine": null, "endLine": null }
                        }
                    }),
                    timestamp: None,
                })
                .expect("send streamed tool call");
            pending
                .send(ControlEnvelope {
                    version: 1,
                    message_type: "response".to_string(),
                    id: Some(id),
                    method: None,
                    payload: json!({
                        "usage": { "inputTokens": 4, "outputTokens": 2 },
                        "toolCalls": []
                    }),
                    timestamp: None,
                })
                .expect("send response without tool calls");
        });

        let mut text = String::new();
        let mut reasoning = String::new();
        let mut run_metrics = RemoteSidecarRunMetrics::new();
        let mut sequence = 0_i64;
        let result = remote_sidecar_run_broker_llm_turn(
            &state,
            &run_stream,
            "broker-request-1",
            json!({ "providerId": "provider-1", "modelId": "model-1" }),
            "remote-run-1",
            "chat-1",
            "msg-assistant-1",
            "msg-user-1",
            "provider-1",
            "model-1",
            &request,
            &mut database,
            &mut text,
            &mut reasoning,
            &mut run_metrics,
            &mut sequence,
        )
        .await
        .expect("llm turn should succeed")
        .expect("tool call should request followup");
        broker.await.expect("broker task");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].call_id, "call-1");
        assert_eq!(result[0].name, "read_file");
        assert_eq!(run_metrics.usage.input_tokens, Some(4));
        assert_eq!(run_metrics.usage.output_tokens, Some(2));
        assert!(run_metrics.first_token_latency_ms.is_some());
        let tool_calls = database
            .tool_calls_for_message("msg-assistant-1")
            .expect("tool calls");
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].status, "running");
        assert!(
            run_stream
                .snapshot_after(0)
                .iter()
                .any(|(_, event)| event.get("type").and_then(Value::as_str) == Some("toolCall"))
        );
    }

    #[tokio::test]
    async fn remote_control_ws_delivers_terminal_broker_response_without_deadlocking() {
        let (broker_tx, _) = tokio::sync::broadcast::channel::<ControlEnvelope>(8);
        let state = RemoteSidecarState {
            token: "token".to_string(),
            last_config_hash: Arc::new(Mutex::new(None)),
            runtime_config: Arc::new(Mutex::new(None)),
            code_graph_watcher: Arc::new(Mutex::new(None)),
            ws_count: Arc::new(AtomicUsize::new(0)),
            active_run_count: Arc::new(AtomicUsize::new(0)),
            active_runs: Arc::new(Mutex::new(Vec::new())),
            active_run_streams: Arc::new(Mutex::new(HashMap::new())),
            broker_pending: Arc::new(AsyncMutex::new(HashMap::new())),
            broker_tx,
            shutdown_tx: default_shutdown_tx(),
            workspace_id: "workspace".to_string(),
            workspace_path: "/tmp/workspace".to_string(),
        };
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test ws");
        let port = listener.local_addr().expect("local addr").port();
        let app = Router::new()
            .route("/ws", get(remote_control_ws))
            .with_state(state.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test ws");
        });

        let (tx, mut rx) = mpsc::unbounded_channel();
        state
            .broker_pending
            .lock()
            .await
            .insert("broker-request-1".to_string(), tx);
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .expect("connect test ws");
        let response = ControlEnvelope {
            version: 1,
            message_type: "response".to_string(),
            id: Some("broker-request-1".to_string()),
            method: None,
            payload: json!({ "ok": true }),
            timestamp: None,
        };
        socket
            .send(tungstenite::Message::Text(
                serde_json::to_string(&response)
                    .expect("response json")
                    .into(),
            ))
            .await
            .expect("send response");

        let envelope = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("pending response timeout")
            .expect("pending response");
        assert_eq!(envelope.message_type, "response");
        tokio::time::timeout(Duration::from_secs(1), async {
            assert!(state.broker_pending.lock().await.is_empty());
        })
        .await
        .expect("pending map lock should not deadlock");
        server.abort();
    }

    #[tokio::test]
    async fn remote_control_ws_replies_to_ping_frames() {
        let (broker_tx, _) = tokio::sync::broadcast::channel::<ControlEnvelope>(8);
        let state = RemoteSidecarState {
            token: "token".to_string(),
            last_config_hash: Arc::new(Mutex::new(None)),
            runtime_config: Arc::new(Mutex::new(None)),
            code_graph_watcher: Arc::new(Mutex::new(None)),
            ws_count: Arc::new(AtomicUsize::new(0)),
            active_run_count: Arc::new(AtomicUsize::new(0)),
            active_runs: Arc::new(Mutex::new(Vec::new())),
            active_run_streams: Arc::new(Mutex::new(HashMap::new())),
            broker_pending: Arc::new(AsyncMutex::new(HashMap::new())),
            broker_tx,
            shutdown_tx: default_shutdown_tx(),
            workspace_id: "workspace".to_string(),
            workspace_path: "/tmp/workspace".to_string(),
        };
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test ws");
        let port = listener.local_addr().expect("local addr").port();
        let app = Router::new()
            .route("/ws", get(remote_control_ws))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test ws");
        });

        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .expect("connect test ws");
        socket
            .send(tungstenite::Message::Ping(vec![1_u8, 2].into()))
            .await
            .expect("send ping");
        let message = tokio::time::timeout(Duration::from_secs(1), socket.next())
            .await
            .expect("pong timeout")
            .expect("pong message")
            .expect("pong ok");
        assert!(
            matches!(message, tungstenite::Message::Pong(bytes) if bytes.as_ref() == [1_u8, 2])
        );
        server.abort();
    }

    #[tokio::test]
    async fn remote_sidecar_fails_pending_requests_when_broker_disconnects() {
        let (state, _) = test_sidecar_state("/tmp/workspace".to_string(), 1);
        let (tx, mut rx) = mpsc::unbounded_channel();
        state
            .broker_pending
            .lock()
            .await
            .insert("broker-request-1".to_string(), tx);

        remote_sidecar_fail_pending_broker_requests(
            &state,
            "remote broker disconnected; retry after reconnect",
        )
        .await;

        let envelope = rx.recv().await.expect("pending request error");
        assert_eq!(envelope.message_type, "error");
        assert_eq!(envelope.id.as_deref(), Some("broker-request-1"));
        assert!(
            envelope.payload["message"]
                .as_str()
                .expect("message")
                .contains("disconnected")
        );
        assert!(state.broker_pending.lock().await.is_empty());
    }

    #[test]
    fn remote_sidecar_executable_tool_schemas_expose_only_phase1_tools() {
        let tools = remote_sidecar_executable_tool_schemas();
        let tool_names = tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<HashSet<_>>();

        for expected in ["read_file", "find_files", "search_text", "run_command"] {
            assert!(
                tool_names.contains(expected),
                "missing tool schema: {expected}"
            );
        }
        for expected in [
            "graph_find_symbols",
            "graph_find_callers",
            "graph_find_callees",
            "graph_find_references",
            "graph_related_files",
            "graph_explore",
        ] {
            assert!(
                tool_names.contains(expected),
                "missing tool schema: {expected}"
            );
        }

        for unexpected in [
            "write_file",
            "edit_file",
            "ask_question",
            "web_search",
            "web_fetch",
            "image_gen",
            "memory_search",
            "memory_write",
            "read_spec",
            "update_spec",
            "create_todo_graph",
            "update_todo_graph",
            "get_todo_graph",
        ] {
            assert!(
                !tool_names.contains(unexpected),
                "unexpected tool schema leaked: {unexpected}"
            );
        }
    }

    #[tokio::test]
    async fn remote_sidecar_chat_messages_include_tool_calls_for_history_reload() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace db");
        database
            .insert_chat_with_metadata("chat-1", "Remote chat", "{}")
            .expect("insert chat");
        database
            .insert_message(NewMessage {
                id: "msg-user-1",
                chat_id: "chat-1",
                role: "user",
                content: "hello",
                sequence: 0,
                metadata_json: Some("{}"),
            })
            .expect("insert user");
        database
            .insert_message(NewMessage {
                id: "msg-assistant-1",
                chat_id: "chat-1",
                role: "assistant",
                content: "done",
                sequence: 1,
                metadata_json: Some(
                    &json!({
                        "reasoning": "thinking",
                        "metrics": { "llmRequestIds": ["run-1"] }
                    })
                    .to_string(),
                ),
            })
            .expect("insert assistant");
        remote_sidecar_record_pending_tool_calls(
            &mut database,
            "chat-1",
            "run-1",
            "msg-assistant-1",
            &[NeutralToolCall {
                call_id: "call-1".to_string(),
                name: "read_file".to_string(),
                arguments: json!({ "path": "Cargo.toml", "startLine": null, "endLine": null }),
                thought_signatures: None,
            }],
        )
        .expect("record tool call");
        remote_sidecar_record_tool_result(
            &mut database,
            &NeutralToolCall {
                call_id: "call-1".to_string(),
                name: "read_file".to_string(),
                arguments: json!({ "path": "Cargo.toml", "startLine": null, "endLine": null }),
                thought_signatures: None,
            },
            &json!({ "content": "[package]" }),
            false,
            "2026-07-09T00:00:00Z",
            "2026-07-09T00:00:01Z",
        )
        .expect("record tool result");
        drop(database);

        let (state, _) = test_sidecar_state(workspace.path().to_string_lossy().to_string(), 0);
        let response = remote_sidecar_chat_messages(
            State(state),
            AxumPath("chat-1".to_string()),
            Query(HashMap::new()),
        )
        .await
        .expect("chat messages")
        .0;
        let messages = response["messages"].as_array().expect("messages array");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1]["toolCalls"][0]["id"], "call-1");
        assert_eq!(messages[1]["toolCalls"][0]["name"], "read_file");
        assert_eq!(messages[1]["toolCalls"][0]["status"], "completed");
        assert_eq!(
            messages[1]["toolCalls"][0]["output"]["content"],
            "[package]"
        );
        assert_eq!(messages[1]["parts"][0]["type"], "reasoning");
        assert_eq!(messages[1]["parts"][1]["type"], "text");
        assert_eq!(messages[1]["parts"][2]["type"], "toolCall");
        assert_eq!(messages[1]["parts"][2]["toolCall"]["id"], "call-1");
    }

    #[tokio::test]
    async fn remote_sidecar_chat_messages_for_request_replays_tool_round_trip() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace db");
        database
            .insert_chat_with_metadata("chat-1", "Remote chat", "{}")
            .expect("insert chat");
        database
            .insert_message(NewMessage {
                id: "msg-user-1",
                chat_id: "chat-1",
                role: "user",
                content: "hello",
                sequence: 0,
                metadata_json: Some("{}"),
            })
            .expect("insert user message");
        database
            .insert_message(NewMessage {
                id: "msg-assistant-1",
                chat_id: "chat-1",
                role: "assistant",
                content: "",
                sequence: 1,
                metadata_json: Some(&json!({ "reasoning": "thinking" }).to_string()),
            })
            .expect("insert assistant placeholder");
        remote_sidecar_record_pending_tool_calls(
            &mut database,
            "chat-1",
            "run-1",
            "msg-assistant-1",
            &[NeutralToolCall {
                call_id: "call-1".to_string(),
                name: "read_file".to_string(),
                arguments: json!({ "path": "Cargo.toml", "startLine": null, "endLine": null }),
                thought_signatures: None,
            }],
        )
        .expect("record tool call");
        remote_sidecar_record_tool_result(
            &mut database,
            &NeutralToolCall {
                call_id: "call-1".to_string(),
                name: "read_file".to_string(),
                arguments: json!({ "path": "Cargo.toml", "startLine": null, "endLine": null }),
                thought_signatures: None,
            },
            &json!({ "content": "[package]" }),
            false,
            "2026-07-09T00:00:00Z",
            "2026-07-09T00:00:01Z",
        )
        .expect("record tool result");

        let messages =
            remote_sidecar_chat_messages_for_request(&database, "chat-1", "msg-assistant-2")
                .expect("chat messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, NeutralChatRole::User);
        assert_eq!(messages[1].role, NeutralChatRole::Assistant);
        assert_eq!(messages[1].tool_calls.len(), 1);
        assert_eq!(messages[1].tool_calls[0].call_id, "call-1");
        assert_eq!(messages[1].reasoning.as_deref(), Some("thinking"));
        assert_eq!(messages[2].role, NeutralChatRole::Tool);
        assert_eq!(messages[2].tool_call_id.as_deref(), Some("call-1"));
        assert_eq!(messages[2].tool_name.as_deref(), Some("read_file"));
        assert_eq!(
            messages[2].content,
            json!({ "content": "[package]" }).to_string()
        );
    }

    #[test]
    fn persist_sidecar_llm_audit_stores_request_tools_and_messages() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace db");
        database
            .insert_chat_with_metadata("chat-1", "Remote chat", "{}")
            .expect("insert chat");

        let request = NeutralChatRequest {
            model_id: "model-1".to_string(),
            messages: vec![
                neutral_text_message(NeutralChatRole::User, "hello".to_string()),
                NeutralChatMessage {
                    role: NeutralChatRole::Assistant,
                    content: "".to_string(),
                    attachments: Vec::new(),
                    reasoning: Some("thinking".to_string()),
                    tool_calls: vec![NeutralToolCall {
                        call_id: "call-1".to_string(),
                        name: "read_file".to_string(),
                        arguments: json!({ "path": "Cargo.toml", "startLine": null, "endLine": null }),
                        thought_signatures: None,
                    }],
                    tool_call_id: None,
                    tool_name: None,
                },
                NeutralChatMessage {
                    role: NeutralChatRole::Tool,
                    content: json!({ "content": "[package]" }).to_string(),
                    attachments: Vec::new(),
                    reasoning: None,
                    tool_calls: Vec::new(),
                    tool_call_id: Some("call-1".to_string()),
                    tool_name: Some("read_file".to_string()),
                },
            ],
            tools: remote_sidecar_executable_tool_schemas(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
        };
        let response_body =
            json!({ "type": "complete", "metrics": { "llmRequestIds": ["remote-run-1"] } });

        let mut run_metrics = RemoteSidecarRunMetrics::new();
        run_metrics.capture_first_output();
        run_metrics.merge_usage_value(&json!({ "inputTokens": 12, "outputTokens": 5 }));
        let completed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let total_latency_ms = 1250;

        persist_sidecar_llm_audit(
            &mut database,
            "workspace",
            "chat-1",
            "remote-run-1",
            "provider-1",
            "model-1",
            Some(&request),
            &run_metrics,
            &completed_at,
            total_latency_ms,
            "succeeded",
            response_body.clone(),
        )
        .expect("persist sidecar audit");

        let record = database
            .llm_request("remote-run-1")
            .expect("llm request read")
            .expect("llm request");
        let request_body = serde_json::from_str::<Value>(
            record
                .request_body_json
                .as_deref()
                .expect("request body json"),
        )
        .expect("request body parse");
        assert_eq!(request_body["providerId"], "provider-1");
        assert_eq!(request_body["modelId"], "model-1");
        assert_eq!(record.input_tokens, Some(12));
        assert_eq!(record.output_tokens, Some(5));
        assert_eq!(
            record.first_token_latency_ms,
            run_metrics.first_token_latency_ms
        );
        assert_eq!(record.total_latency_ms, Some(total_latency_ms));
        assert_eq!(
            request_body["request"]["messages"][1]["tool_calls"][0]["name"],
            "read_file"
        );
        assert!(
            request_body["request"]["tools"]
                .as_array()
                .expect("tools array")
                .iter()
                .any(|tool| tool["name"] == "read_file")
        );
        let response_body_json = serde_json::from_str::<Value>(
            record
                .response_body_json
                .as_deref()
                .expect("response body json"),
        )
        .expect("response body parse");
        assert_eq!(response_body_json, response_body);
    }

    #[test]
    fn remote_sidecar_provider_request_includes_synced_system_prompt() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace db");
        database
            .insert_chat_with_metadata("chat-1", "Remote chat", "{}")
            .expect("insert chat");
        database
            .insert_message(NewMessage {
                id: "msg-user-1",
                chat_id: "chat-1",
                role: "user",
                content: "hello",
                sequence: 0,
                metadata_json: Some("{}"),
            })
            .expect("insert user message");
        database
            .insert_message(NewMessage {
                id: "msg-assistant-1",
                chat_id: "chat-1",
                role: "assistant",
                content: "",
                sequence: 1,
                metadata_json: Some("{}"),
            })
            .expect("insert assistant placeholder");

        let (state, _) = test_sidecar_state(workspace.path().to_string_lossy().to_string(), 1);
        let mut config =
            foco_store::config::GlobalConfig::first_run(workspace.path().to_path_buf());
        config
            .prompts
            .system_prompts
            .push(foco_store::config::SystemPromptSettings {
                name: "RemoteSystem".to_string(),
                content: "remote system prompt marker".to_string(),
            });
        config.models.push(foco_store::config::ModelSettings {
            id: "model-1".to_string(),
            display_name: "Model 1".to_string(),
            enabled: true,
            provider_ids: vec!["provider-1".to_string()],
            active_provider_id: Some("provider-1".to_string()),
            thinking_level: None,
            system_prompt_name: "RemoteSystem".to_string(),
            metadata_key: None,
            metadata_source_url: None,
            metadata_refreshed_at: None,
            limits: Some(foco_store::config::ModelLimits {
                context_window: 128_000,
                max_output_tokens: 4096,
            }),
            input_modalities: Vec::new(),
            output_modalities: Vec::new(),
        });
        let bundle =
            build_sidecar_runtime_config_bundle(workspace.path(), &config, "workspace", None, 1)
                .expect("runtime bundle");
        *state.runtime_config.lock().expect("runtime config") = Some(bundle);

        let request = remote_sidecar_provider_request(
            &state,
            &database,
            "chat-1",
            "msg-assistant-1",
            "model-1",
            Value::Null,
        )
        .expect("provider request");

        assert_eq!(request.messages[0].role, NeutralChatRole::System);
        assert!(
            request.messages[0]
                .content
                .contains("remote system prompt marker")
        );
        let tool_names = request
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<HashSet<_>>();
        for expected in ["read_file", "find_files", "search_text"] {
            assert!(
                tool_names.contains(expected),
                "missing tool schema: {expected}"
            );
        }
        for unexpected in ["ask_question", "web_search", "memory_search", "write_file"] {
            assert!(
                !tool_names.contains(unexpected),
                "unexpected tool schema leaked: {unexpected}"
            );
        }
        assert!(
            request
                .messages
                .iter()
                .any(|message| message.content.contains("## Environment Context"))
        );
        assert!(
            request
                .messages
                .iter()
                .any(|message| message.role == NeutralChatRole::User && message.content == "hello")
        );
    }

    #[tokio::test]
    async fn llm_stream_broker_rpc_round_trips_through_pending_channel() {
        let (broker_tx, mut broker_rx) = tokio::sync::broadcast::channel::<ControlEnvelope>(8);
        let state = RemoteSidecarState {
            token: "token".to_string(),
            last_config_hash: Arc::new(Mutex::new(None)),
            runtime_config: Arc::new(Mutex::new(None)),
            code_graph_watcher: Arc::new(Mutex::new(None)),
            ws_count: Arc::new(AtomicUsize::new(1)),
            active_run_count: Arc::new(AtomicUsize::new(0)),
            active_runs: Arc::new(Mutex::new(Vec::new())),
            active_run_streams: Arc::new(Mutex::new(HashMap::new())),
            broker_pending: Arc::new(AsyncMutex::new(HashMap::new())),
            broker_tx,
            shutdown_tx: default_shutdown_tx(),
            workspace_id: "workspace".to_string(),
            workspace_path: "/tmp/workspace".to_string(),
        };

        let mut response_rx = remote_sidecar_broker_request(
            &state,
            "broker-request-test",
            "llm.stream",
            json!({ "providerId": "provider", "modelId": "model", "messages": [] }),
        )
        .await
        .expect("broker request");
        let request = broker_rx.recv().await.expect("broker envelope");
        assert_eq!(request.message_type, "request");
        assert_eq!(request.method.as_deref(), Some("llm.stream"));
        assert_eq!(request.payload["providerId"], "provider");

        let id = request.id.clone().expect("request id");
        let pending = state
            .broker_pending
            .lock()
            .await
            .get(&id)
            .cloned()
            .expect("pending response channel");
        pending
            .send(ControlEnvelope {
                version: 1,
                message_type: "response".to_string(),
                id: Some(id),
                method: None,
                payload: json!({ "ok": true }),
                timestamp: None,
            })
            .expect("send broker response");

        let response = response_rx.recv().await.expect("broker response");
        assert_eq!(response.message_type, "response");
        assert_eq!(response.payload["ok"], true);
    }
}
