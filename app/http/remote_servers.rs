use std::{collections::HashSet, fs, path::PathBuf};

use axum::{
    Json,
    extract::{Path as AxumPath, State},
};
use chrono::{SecondsFormat, Utc};
use foco_store::config::{
    DEFAULT_REMOTE_CONNECT_TIMEOUT_MS, RemoteAuthMethod, RemoteServerProfile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::*;

const REMOTE_SERVER_STATUS_CONNECTED: &str = "connected";
const REMOTE_SERVER_STATUS_READY: &str = "ready";
const REMOTE_SERVER_STATUS_ERROR: &str = "error";
const REMOTE_SERVER_STATUS_UNKNOWN: &str = "unknown";
const SIDECAR_INSTALL_STATE_UNKNOWN: &str = "unknown";
const SIDECAR_INSTALL_STATE_NOT_INSTALLED: &str = "notInstalled";
const SIDECAR_INSTALL_STATE_CUSTOM_COMMAND: &str = "customCommand";
const SIDECAR_INSTALL_STATE_AVAILABLE: &str = "available";
const SIDECAR_INSTALL_STATE_MISSING_ASSET: &str = "missingAsset";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RemoteServerInput {
    #[serde(default)]
    id: Option<String>,
    name: String,
    host_alias: String,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    identity_file: Option<String>,
    #[serde(default)]
    auth_method: Option<RemoteAuthMethod>,
    /// Empty string on update means "keep existing password".
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    default_remote_root: Option<String>,
    #[serde(default)]
    foco_command: Option<String>,
    #[serde(default)]
    terminal_shell: Option<String>,
    #[serde(default)]
    connect_timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RemoteServerIdRequest {
    id: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteServerSummary {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) host_alias: String,
    pub(crate) user: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) identity_file: Option<String>,
    pub(crate) auth_method: RemoteAuthMethod,
    pub(crate) password_configured: bool,
    pub(crate) default_remote_root: Option<String>,
    pub(crate) foco_command: Option<String>,
    pub(crate) terminal_shell: Option<String>,
    pub(crate) connect_timeout_ms: u64,
    pub(crate) status: String,
    pub(crate) last_error: Option<String>,
    pub(crate) last_known_target: Option<String>,
    pub(crate) sidecar_version: Option<String>,
    pub(crate) sidecar_install_state: String,
    pub(crate) workspace_count: usize,
    pub(crate) last_checked_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteServersResponse {
    servers: Vec<RemoteServerSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteServerResponse {
    server: RemoteServerSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteRemoteServerResponse {
    deleted: bool,
    references: Vec<RemoteServerWorkspaceReference>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteServerWorkspaceReference {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) remote_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteServerDiagnosticResponse {
    pub(crate) server: RemoteServerSummary,
    pub(crate) result: RemoteServerDiagnosticResult,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteServerDiagnosticResult {
    pub(crate) ok: bool,
    pub(crate) error_kind: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) stages: Vec<RemoteServerDiagnosticStage>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteServerDiagnosticStage {
    pub(crate) stage: String,
    pub(crate) status: String,
    pub(crate) error_kind: Option<String>,
    pub(crate) message: String,
    pub(crate) details: Option<String>,
}

#[derive(Deserialize)]
struct SidecarManifest {
    version: String,
    sidecars: Vec<SidecarManifestEntry>,
}

#[derive(Deserialize)]
struct SidecarManifestEntry {
    target: String,
    path: String,
    sha256: String,
}

pub(crate) async fn list_remote_servers(
    State(state): State<AppState>,
) -> Result<Json<RemoteServersResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let connected_ids = connected_remote_server_ids(&state)?;
    Ok(Json(RemoteServersResponse {
        servers: remote_server_summaries(&config, &connected_ids, Some(&state)),
    }))
}

pub(crate) async fn create_remote_server(
    State(state): State<AppState>,
    Json(input): Json<RemoteServerInput>,
) -> Result<Json<RemoteServerResponse>, ApiError> {
    let mut config = config_update_snapshot(&state).await?;
    let mut server = remote_server_from_input(input, None)?;
    if config
        .remote_servers
        .iter()
        .any(|item| item.id == server.id)
    {
        server.id = unique_remote_server_id(&config);
    }
    reject_duplicate_remote_server(&config, &server, None)?;
    config.remote_servers.push(server.clone());
    save_config(&state, &mut config)?;
    let connected_ids = connected_remote_server_ids(&state)?;
    Ok(Json(RemoteServerResponse {
        server: remote_server_summary(&config, &server, &connected_ids, Some(&state)),
    }))
}

pub(crate) async fn update_remote_server(
    State(state): State<AppState>,
    Json(input): Json<RemoteServerInput>,
) -> Result<Json<RemoteServerResponse>, ApiError> {
    let mut config = config_update_snapshot(&state).await?;
    let id = input
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("remote server id must not be empty"))?
        .to_string();
    let existing = config
        .remote_servers
        .iter()
        .find(|server| server.id == id)
        .cloned()
        .ok_or_else(|| ApiError::bad_request(format!("remote server was not found: {id}")))?;
    let server = remote_server_from_input(input, Some(&existing))?;
    reject_duplicate_remote_server(&config, &server, Some(&id))?;
    let index = config
        .remote_servers
        .iter()
        .position(|item| item.id == id)
        .expect("remote server existed above");
    config.remote_servers[index] = server.clone();
    save_config(&state, &mut config)?;
    let connected_ids = connected_remote_server_ids(&state)?;
    Ok(Json(RemoteServerResponse {
        server: remote_server_summary(&config, &server, &connected_ids, Some(&state)),
    }))
}

pub(crate) async fn delete_remote_server(
    State(state): State<AppState>,
    Json(request): Json<RemoteServerIdRequest>,
) -> Result<Json<DeleteRemoteServerResponse>, ApiError> {
    let mut config = config_update_snapshot(&state).await?;
    let id = request.id.trim();
    let references = remote_server_workspace_references(&config, id);
    if !references.is_empty() {
        return Err(ApiError::conflict(format!(
            "remote server is used by {} workspace(s): {}",
            references.len(),
            references
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    let before = config.remote_servers.len();
    config.remote_servers.retain(|server| server.id != id);
    let deleted = config.remote_servers.len() != before;
    if !deleted {
        return Err(ApiError::bad_request(format!(
            "remote server was not found: {id}"
        )));
    }
    save_config(&state, &mut config)?;
    disconnect_remote_server_id(&state, id)?;
    state.remote_workspace_manager.disconnect_server(id).await?;
    Ok(Json(DeleteRemoteServerResponse {
        deleted,
        references,
    }))
}

pub(crate) async fn test_remote_server(
    State(state): State<AppState>,
    AxumPath(server_id): AxumPath<String>,
) -> Result<Json<RemoteServerDiagnosticResponse>, ApiError> {
    run_remote_server_diagnostic_api(state, server_id, false).await
}

pub(crate) async fn connect_remote_server(
    State(state): State<AppState>,
    AxumPath(server_id): AxumPath<String>,
) -> Result<Json<RemoteServerDiagnosticResponse>, ApiError> {
    run_remote_server_diagnostic_api(state, server_id, true).await
}

pub(crate) async fn disconnect_remote_server(
    State(state): State<AppState>,
    AxumPath(server_id): AxumPath<String>,
) -> Result<Json<RemoteServerResponse>, ApiError> {
    disconnect_remote_server_id(&state, &server_id)?;
    state
        .remote_workspace_manager
        .disconnect_server(&server_id)
        .await?;
    let config = config_snapshot(&state)?;
    let server = remote_server_by_id(&config, &server_id)?.clone();
    let connected_ids = connected_remote_server_ids(&state)?;
    Ok(Json(RemoteServerResponse {
        server: remote_server_summary(&config, &server, &connected_ids, Some(&state)),
    }))
}

pub(crate) async fn remote_server_status(
    State(state): State<AppState>,
    AxumPath(server_id): AxumPath<String>,
) -> Result<Json<RemoteServerResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let server = remote_server_by_id(&config, &server_id)?.clone();
    let connected_ids = connected_remote_server_ids(&state)?;
    Ok(Json(RemoteServerResponse {
        server: remote_server_summary(&config, &server, &connected_ids, Some(&state)),
    }))
}

pub(crate) fn remote_server_summaries(
    config: &GlobalConfig,
    connected_ids: &HashSet<String>,
    state: Option<&AppState>,
) -> Vec<RemoteServerSummary> {
    config
        .remote_servers
        .iter()
        .map(|server| remote_server_summary(config, server, connected_ids, state))
        .collect()
}

pub(crate) fn remote_server_summary(
    config: &GlobalConfig,
    server: &RemoteServerProfile,
    connected_ids: &HashSet<String>,
    state: Option<&AppState>,
) -> RemoteServerSummary {
    let status = state
        .and_then(|state| {
            state
                .remote_workspace_manager
                .server_state(&server.id)
                .ok()
                .flatten()
        })
        .map(|state| state.as_str().to_string())
        .unwrap_or_else(|| remote_server_status_value(server, connected_ids));
    RemoteServerSummary {
        id: server.id.clone(),
        name: server.name.clone(),
        host_alias: server.host_alias.clone(),
        user: server.user.clone(),
        port: server.port,
        identity_file: server
            .identity_file
            .as_ref()
            .map(|path| path.display().to_string()),
        auth_method: server.auth_method,
        password_configured: server.password_configured(),
        default_remote_root: server.default_remote_root.clone(),
        foco_command: server.foco_command.clone(),
        terminal_shell: server.terminal_shell.clone(),
        connect_timeout_ms: server.connect_timeout_ms,
        status: status.clone(),
        last_error: server.last_error.clone(),
        last_known_target: server.last_known_target.clone(),
        sidecar_version: server.last_sidecar_version.clone(),
        sidecar_install_state: remote_server_summary_sidecar_install_state(server, &status),
        workspace_count: workspace_count_for_server(config, &server.id),
        last_checked_at: server.last_checked_at.clone(),
    }
}

fn remote_server_summary_sidecar_install_state(
    server: &RemoteServerProfile,
    status: &str,
) -> String {
    let state = server
        .sidecar_install_state
        .clone()
        .unwrap_or_else(|| SIDECAR_INSTALL_STATE_UNKNOWN.to_string());
    if state == SIDECAR_INSTALL_STATE_NOT_INSTALLED
        && server.last_sidecar_version.is_some()
        && matches!(
            status,
            REMOTE_SERVER_STATUS_CONNECTED | REMOTE_SERVER_STATUS_READY
        )
    {
        return SIDECAR_INSTALL_STATE_AVAILABLE.to_string();
    }
    state
}

pub(crate) fn connected_remote_server_ids(state: &AppState) -> Result<HashSet<String>, ApiError> {
    let mut connections = state
        .remote_server_connections
        .lock()
        .map_err(|_| ApiError::internal("remote server connection lock is poisoned"))?
        .clone();
    connections.extend(state.remote_workspace_manager.server_ids_with_sessions()?);
    Ok(connections)
}

pub(crate) fn remote_server_workspace_references(
    config: &GlobalConfig,
    server_id: &str,
) -> Vec<RemoteServerWorkspaceReference> {
    config
        .workspaces
        .iter()
        .filter_map(|workspace| match &workspace.location {
            WorkspaceLocation::Ssh {
                server_id: workspace_server_id,
                remote_path,
            } if workspace_server_id == server_id => Some(RemoteServerWorkspaceReference {
                id: workspace.id.clone(),
                name: workspace.name.clone(),
                remote_path: remote_path.clone(),
            }),
            _ => None,
        })
        .collect()
}

async fn run_remote_server_diagnostic_api(
    state: AppState,
    server_id: String,
    mark_connected: bool,
) -> Result<Json<RemoteServerDiagnosticResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let server = remote_server_by_id(&config, &server_id)?.clone();
    let mut result = test_remote_server_connection(&server).await;

    let mut config = config_update_snapshot(&state).await?;
    let mut updated = update_remote_server_diagnostic_cache(&mut config, &server_id, &result)?;
    save_config(&state, &mut config)?;

    if mark_connected && result.ok {
        state
            .remote_workspace_manager
            .ensure_server_sidecar(state.clone(), &server_id)
            .await?;
        config = config_update_snapshot(&state).await?;
        updated = remote_server_by_id(&config, &server_id)?.clone();
        result = test_remote_server_connection(&updated).await;
        updated = update_remote_server_diagnostic_cache(&mut config, &server_id, &result)?;
        save_config(&state, &mut config)?;
        let mut connections = state
            .remote_server_connections
            .lock()
            .map_err(|_| ApiError::internal("remote server connection lock is poisoned"))?;
        connections.insert(server_id.clone());
    }
    if !result.ok {
        disconnect_remote_server_id(&state, &server_id)?;
    }

    let connected_ids = connected_remote_server_ids(&state)?;
    Ok(Json(RemoteServerDiagnosticResponse {
        server: remote_server_summary(&config, &updated, &connected_ids, Some(&state)),
        result,
    }))
}

async fn test_remote_server_connection(
    server: &RemoteServerProfile,
) -> RemoteServerDiagnosticResult {
    let mut stages = vec![
        pending_stage("ssh"),
        pending_stage("target"),
        pending_stage("sidecarAsset"),
        pending_stage("remoteInstallDirWritable"),
        pending_stage("focoCommandVersion"),
    ];

    let profile = match crate::ssh_client::resolve_ssh_profile(
        server,
        crate::ssh_client::ResolveSshOptions::default(),
    ) {
        Ok(profile) => profile,
        Err(err) => {
            stages[0] = failed_stage(
                "ssh",
                err.kind_str(),
                format!("SSH configuration could not be resolved: {}", err.message()),
                None,
            );
            return diagnostic_result(stages);
        }
    };

    let session = match crate::ssh_client::SshSession::connect(&profile).await {
        Ok(session) => session,
        Err(err) => {
            stages[0] = failed_stage(
                "ssh",
                err.kind_str(),
                format!("SSH connection failed: {}", err.message()),
                err.host_key.as_ref().map(|key| {
                    format!(
                        "host={} port={} algorithm={} fingerprint={}",
                        key.host, key.port, key.algorithm, key.fingerprint_sha256
                    )
                }),
            );
            return diagnostic_result(stages);
        }
    };

    match session.exec("true").await {
        Ok(result) if result.success() => {
            stages[0] = success_stage(
                "ssh",
                format!(
                    "Rust SSH connected to {}@{}:{} and verified login",
                    profile.user, profile.hostname, profile.port
                ),
                Some(format!(
                    "resolved hostAlias={} hostname={} port={} user={}",
                    profile.host_alias, profile.hostname, profile.port, profile.user
                )),
            );
        }
        Ok(result) => {
            stages[0] = failed_stage(
                "ssh",
                "remote_command_failed",
                "SSH login succeeded but remote `true` returned non-zero",
                Some(result.details()),
            );
            let _ = session.disconnect().await;
            return diagnostic_result(stages);
        }
        Err(err) => {
            stages[0] = failed_stage(
                "ssh",
                err.kind_str(),
                format!("SSH login failed: {}", err.message()),
                None,
            );
            let _ = session.disconnect().await;
            return diagnostic_result(stages);
        }
    }

    let target = match session.exec("uname -s && uname -m").await {
        Ok(result) if result.success() => {
            match normalize_target(&result.stdout_lossy()) {
                Ok(target) => {
                    stages[1] = success_stage(
                        "target",
                        format!("Detected target {target}"),
                        Some(result.details()),
                    );
                    target
                }
                Err(message) => {
                    stages[1] = failed_stage(
                        "target",
                        "target_unsupported",
                        message,
                        Some(result.details()),
                    );
                    let _ = session.disconnect().await;
                    return diagnostic_result(stages);
                }
            }
        }
        Ok(result) => {
            stages[1] = failed_stage(
                "target",
                "remote_command_failed",
                "Failed to run uname on remote server",
                Some(result.details()),
            );
            let _ = session.disconnect().await;
            return diagnostic_result(stages);
        }
        Err(err) => {
            stages[1] = failed_stage(
                "target",
                err.kind_str(),
                format!("Failed to run uname on remote server: {}", err.message()),
                None,
            );
            let _ = session.disconnect().await;
            return diagnostic_result(stages);
        }
    };

    let sidecar = match select_sidecar_asset(&target) {
        Ok(asset) => {
            stages[2] = success_stage(
                "sidecarAsset",
                format!("Found packaged sidecar {} for {target}", asset.version),
                Some(asset.path.display().to_string()),
            );
            Some(asset)
        }
        Err(message)
            if server
                .foco_command
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()) =>
        {
            stages[2] = skipped_stage(
                "sidecarAsset",
                format!("{message}; custom focoCommand will be used"),
            );
            None
        }
        Err(message) => {
            stages[2] = failed_stage("sidecarAsset", "sidecar_asset_missing", message, None);
            let _ = session.disconnect().await;
            return diagnostic_result(stages);
        }
    };

    match session
        .exec("mkdir -p ~/.foco/sidecars && test -w ~/.foco/sidecars")
        .await
    {
        Ok(result) if result.success() => {
            stages[3] = success_stage(
                "remoteInstallDirWritable",
                "Remote install directory is writable",
                Some(result.details()),
            );
        }
        Ok(result) => {
            stages[3] = failed_stage(
                "remoteInstallDirWritable",
                "permission_denied",
                "Remote sidecar install directory is not writable",
                Some(result.details()),
            );
            let _ = session.disconnect().await;
            return diagnostic_result(stages);
        }
        Err(err) => {
            stages[3] = failed_stage(
                "remoteInstallDirWritable",
                err.kind_str(),
                format!(
                    "Remote sidecar install directory check failed: {}",
                    err.message()
                ),
                None,
            );
            let _ = session.disconnect().await;
            return diagnostic_result(stages);
        }
    }

    if let Some(command) = server
        .foco_command
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let version_command = format!(
            "{command} --version && {command} --sidecar-target && {command} --sidecar-build-id"
        );
        match session.exec(&version_command).await {
            Ok(result) if result.success() => {
                stages[4] = success_stage(
                    "focoCommandVersion",
                    "focoCommand responded to --version, --sidecar-target, and --sidecar-build-id",
                    Some(result.details()),
                );
            }
            Ok(result) => {
                stages[4] = failed_stage(
                    "focoCommandVersion",
                    "startup_failed",
                    "focoCommand failed to report version, target, and build identity",
                    Some(result.details()),
                );
                let _ = session.disconnect().await;
                return diagnostic_result(stages);
            }
            Err(err) => {
                stages[4] = failed_stage(
                    "focoCommandVersion",
                    err.kind_str(),
                    format!("focoCommand check failed: {}", err.message()),
                    None,
                );
                let _ = session.disconnect().await;
                return diagnostic_result(stages);
            }
        }
    } else if let Some(asset) = sidecar {
        let remote_path = format!("~/.foco/sidecars/{}/{}/foco", asset.version, target);
        let check_command = format!(
            "test -x {remote_path} && {remote_path} --version && {remote_path} --sidecar-target && {remote_path} --sidecar-build-id"
        );
        match session.exec(&check_command).await {
            Ok(result) if result.success() => {
                stages[4] = success_stage(
                    "focoCommandVersion",
                    "Installed sidecar responded to version, target, and build identity",
                    Some(result.details()),
                );
            }
            Ok(result) => {
                stages[4] = skipped_stage(
                    "focoCommandVersion",
                    format!("Packaged sidecar is available but {remote_path} is not installed yet"),
                );
                let mut details = result.details();
                if !details.is_empty() {
                    details.insert_str(0, "Remote version check output:\n");
                    stages[4].details = Some(details);
                }
            }
            Err(err) => {
                stages[4] = skipped_stage("focoCommandVersion", err.message().to_string());
            }
        }
    }

    let _ = session.disconnect().await;
    diagnostic_result(stages)
}

fn update_remote_server_diagnostic_cache(
    config: &mut GlobalConfig,
    server_id: &str,
    result: &RemoteServerDiagnosticResult,
) -> Result<RemoteServerProfile, ApiError> {
    let server = config
        .remote_servers
        .iter_mut()
        .find(|server| server.id == server_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!("remote server was not found: {server_id}"))
        })?;
    server.last_checked_at = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
    server.last_error = result.message.clone();
    if let Some(target_stage) = result
        .stages
        .iter()
        .find(|stage| stage.stage == "target" && stage.status == "success")
    {
        server.last_known_target = target_stage
            .message
            .strip_prefix("Detected target ")
            .map(str::to_string)
            .or_else(|| server.last_known_target.clone());
    }
    server.sidecar_install_state = Some(sidecar_install_state_from_result(result));
    Ok(server.clone())
}

fn sidecar_install_state_from_result(result: &RemoteServerDiagnosticResult) -> String {
    if result.stages.iter().any(|stage| {
        stage.stage == "sidecarAsset"
            && stage.error_kind.as_deref() == Some("sidecar_asset_missing")
    }) {
        return SIDECAR_INSTALL_STATE_MISSING_ASSET.to_string();
    }
    if result
        .stages
        .iter()
        .any(|stage| stage.stage == "sidecarAsset" && stage.status == "skipped")
    {
        return SIDECAR_INSTALL_STATE_CUSTOM_COMMAND.to_string();
    }
    if result
        .stages
        .iter()
        .any(|stage| stage.stage == "focoCommandVersion" && stage.status == "success")
    {
        return SIDECAR_INSTALL_STATE_AVAILABLE.to_string();
    }
    if result
        .stages
        .iter()
        .any(|stage| stage.stage == "sidecarAsset" && stage.status == "success")
    {
        return SIDECAR_INSTALL_STATE_NOT_INSTALLED.to_string();
    }
    SIDECAR_INSTALL_STATE_UNKNOWN.to_string()
}

fn remote_server_from_input(
    input: RemoteServerInput,
    existing: Option<&RemoteServerProfile>,
) -> Result<RemoteServerProfile, ApiError> {
    let id = existing
        .map(|server| server.id.clone())
        .or_else(|| {
            input
                .id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| unique_id("remote-server"));
    let name = non_empty(input.name, "remote server name")?;
    let host_alias = non_empty(input.host_alias, "remote server hostAlias")?;
    let connect_timeout_ms = input
        .connect_timeout_ms
        .or_else(|| existing.map(|server| server.connect_timeout_ms))
        .unwrap_or(DEFAULT_REMOTE_CONNECT_TIMEOUT_MS);
    if connect_timeout_ms == 0 {
        return Err(ApiError::bad_request(
            "remote server connectTimeoutMs must be greater than 0",
        ));
    }

    let is_create = existing.is_none();
    let auth_method = input
        .auth_method
        .unwrap_or_else(|| existing.map(|server| server.auth_method).unwrap_or_default());

    let user = match optional_non_empty(input.user) {
        Some(user) => Some(user),
        None if is_create => Some("root".to_string()),
        None => None,
    };
    let default_remote_root = match optional_non_empty(input.default_remote_root) {
        Some(root) => Some(root),
        None if is_create => Some("~".to_string()),
        None => None,
    };

    let password = match auth_method {
        RemoteAuthMethod::Key => None,
        RemoteAuthMethod::Password => {
            let incoming = optional_non_empty(input.password);
            match (incoming, existing) {
                (Some(password), _) => Some(password),
                (None, Some(existing)) if existing.password_configured() => existing.password.clone(),
                (None, _) => {
                    return Err(ApiError::bad_request(
                        "remote server password is required when authMethod is password",
                    ));
                }
            }
        }
    };

    Ok(RemoteServerProfile {
        id,
        name,
        host_alias,
        user,
        port: input.port,
        identity_file: optional_non_empty(input.identity_file).map(PathBuf::from),
        auth_method,
        password,
        default_remote_root,
        foco_command: optional_non_empty(input.foco_command),
        terminal_shell: optional_non_empty(input.terminal_shell),
        connect_timeout_ms,
        last_known_target: existing.and_then(|server| server.last_known_target.clone()),
        last_sidecar_version: existing.and_then(|server| server.last_sidecar_version.clone()),
        last_checked_at: existing.and_then(|server| server.last_checked_at.clone()),
        last_error: existing.and_then(|server| server.last_error.clone()),
        sidecar_install_state: existing.and_then(|server| server.sidecar_install_state.clone()),
    })
}

fn non_empty(value: String, field: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::bad_request(format!("{field} must not be empty")));
    }
    Ok(value.to_string())
}

fn optional_non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn unique_remote_server_id(config: &GlobalConfig) -> String {
    loop {
        let id = unique_id("remote-server");
        if !config.remote_servers.iter().any(|server| server.id == id) {
            return id;
        }
    }
}

fn reject_duplicate_remote_server(
    config: &GlobalConfig,
    server: &RemoteServerProfile,
    allowed_id: Option<&str>,
) -> Result<(), ApiError> {
    for existing in &config.remote_servers {
        if allowed_id == Some(existing.id.as_str()) {
            continue;
        }
        if existing.name == server.name {
            return Err(ApiError::bad_request(format!(
                "remote server name is already registered: {}",
                server.name
            )));
        }
    }
    Ok(())
}

fn remote_server_by_id<'a>(
    config: &'a GlobalConfig,
    server_id: &str,
) -> Result<&'a RemoteServerProfile, ApiError> {
    config
        .remote_servers
        .iter()
        .find(|server| server.id == server_id)
        .ok_or_else(|| ApiError::bad_request(format!("remote server was not found: {server_id}")))
}

fn workspace_count_for_server(config: &GlobalConfig, server_id: &str) -> usize {
    config
        .workspaces
        .iter()
        .filter(|workspace| workspace.server_id() == Some(server_id))
        .count()
}

fn remote_server_status_value(
    server: &RemoteServerProfile,
    connected_ids: &HashSet<String>,
) -> String {
    if connected_ids.contains(&server.id) {
        REMOTE_SERVER_STATUS_CONNECTED.to_string()
    } else if server
        .last_error
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        REMOTE_SERVER_STATUS_ERROR.to_string()
    } else if server.last_checked_at.is_some() && remote_server_sidecar_is_available(server) {
        REMOTE_SERVER_STATUS_READY.to_string()
    } else {
        REMOTE_SERVER_STATUS_UNKNOWN.to_string()
    }
}

fn remote_server_sidecar_is_available(server: &RemoteServerProfile) -> bool {
    server
        .sidecar_install_state
        .as_deref()
        .is_some_and(|state| {
            matches!(
                state,
                SIDECAR_INSTALL_STATE_AVAILABLE | SIDECAR_INSTALL_STATE_CUSTOM_COMMAND
            )
        })
}

fn disconnect_remote_server_id(state: &AppState, server_id: &str) -> Result<(), ApiError> {
    let mut connections = state
        .remote_server_connections
        .lock()
        .map_err(|_| ApiError::internal("remote server connection lock is poisoned"))?;
    connections.remove(server_id);
    Ok(())
}

pub(crate) fn normalize_target(uname_output: &str) -> Result<String, String> {
    let mut lines = uname_output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let os = lines
        .next()
        .ok_or_else(|| "uname did not return an operating system".to_string())?
        .to_ascii_lowercase();
    let arch = lines
        .next()
        .ok_or_else(|| "uname did not return an architecture".to_string())?
        .to_ascii_lowercase();
    if os != "linux" {
        return Err(format!("unsupported remote operating system: {os}"));
    }
    match arch.as_str() {
        "x86_64" | "amd64" => Ok("linux-x64".to_string()),
        "aarch64" | "arm64" => Ok("linux-arm64".to_string()),
        _ => Err(format!("unsupported remote architecture: {arch}")),
    }
}

pub(crate) struct SelectedSidecarAsset {
    pub(crate) version: String,
    pub(crate) target: String,
    pub(crate) sha256: String,
    pub(crate) path: PathBuf,
}

pub(crate) fn select_sidecar_asset(target: &str) -> Result<SelectedSidecarAsset, String> {
    for root in sidecar_roots() {
        let manifest_path = root.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }
        let manifest_text = fs::read_to_string(&manifest_path)
            .map_err(|source| format!("failed to read {}: {source}", manifest_path.display()))?;
        let manifest: SidecarManifest = serde_json::from_str(&manifest_text)
            .map_err(|source| format!("failed to parse {}: {source}", manifest_path.display()))?;
        let Some(entry) = manifest
            .sidecars
            .iter()
            .find(|entry| entry.target == target)
        else {
            continue;
        };
        let asset_path = root.join(entry.path.split('/').collect::<PathBuf>());
        let bytes = fs::read(&asset_path).map_err(|source| {
            format!(
                "failed to read sidecar asset {}: {source}",
                asset_path.display()
            )
        })?;
        let digest = format!("{:x}", Sha256::digest(&bytes));
        if digest != entry.sha256 {
            return Err(format!(
                "sidecar asset sha256 mismatch for {target}: expected {}, got {digest}",
                entry.sha256
            ));
        }
        return Ok(SelectedSidecarAsset {
            version: manifest.version,
            target: target.to_string(),
            sha256: digest,
            path: asset_path,
        });
    }
    Err(format!(
        "packaged sidecar asset for {target} was not found; build or download sidecars/manifest.json first"
    ))
}

fn sidecar_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            roots.push(exe_dir.join("resources").join("sidecars"));
            roots.push(exe_dir.join("sidecars"));
            if let Some(contents_dir) = exe_dir.parent() {
                roots.push(contents_dir.join("Resources").join("sidecars"));
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join("sidecars"));
    }
    roots
}

fn diagnostic_result(stages: Vec<RemoteServerDiagnosticStage>) -> RemoteServerDiagnosticResult {
    let failed = stages.iter().find(|stage| stage.status == "failed");
    RemoteServerDiagnosticResult {
        ok: failed.is_none(),
        error_kind: failed.and_then(|stage| stage.error_kind.clone()),
        message: failed.map(|stage| stage.message.clone()),
        stages,
    }
}

fn pending_stage(stage: &str) -> RemoteServerDiagnosticStage {
    RemoteServerDiagnosticStage {
        stage: stage.to_string(),
        status: "pending".to_string(),
        error_kind: None,
        message: String::new(),
        details: None,
    }
}

fn success_stage(
    stage: &str,
    message: impl Into<String>,
    details: Option<String>,
) -> RemoteServerDiagnosticStage {
    RemoteServerDiagnosticStage {
        stage: stage.to_string(),
        status: "success".to_string(),
        error_kind: None,
        message: message.into(),
        details,
    }
}

fn skipped_stage(stage: &str, message: impl Into<String>) -> RemoteServerDiagnosticStage {
    RemoteServerDiagnosticStage {
        stage: stage.to_string(),
        status: "skipped".to_string(),
        error_kind: None,
        message: message.into(),
        details: None,
    }
}

fn failed_stage(
    stage: &str,
    error_kind: &'static str,
    message: impl Into<String>,
    details: Option<String>,
) -> RemoteServerDiagnosticStage {
    RemoteServerDiagnosticStage {
        stage: stage.to_string(),
        status: "failed".to_string(),
        error_kind: Some(error_kind.to_string()),
        message: message.into(),
        details,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_supported_linux_targets() {
        assert_eq!(normalize_target("Linux\nx86_64\n").unwrap(), "linux-x64");
        assert_eq!(normalize_target("Linux\naarch64\n").unwrap(), "linux-arm64");
    }

    #[test]
    fn rejects_unsupported_targets() {
        assert!(
            normalize_target("Darwin\narm64\n")
                .unwrap_err()
                .contains("unsupported")
        );
        assert!(
            normalize_target("Linux\nriscv64\n")
                .unwrap_err()
                .contains("unsupported")
        );
    }

    #[test]
    fn server_summary_counts_referencing_workspaces() {
        let workspace_dir = tempfile::tempdir().expect("workspace");
        let mut config = GlobalConfig::first_run(workspace_dir.path().to_path_buf());
        config.remote_servers.push(RemoteServerProfile {
            id: "srv".to_string(),
            name: "Srv".to_string(),
            host_alias: "srv".to_string(),
            ..RemoteServerProfile::default()
        });
        config.workspaces.push(WorkspaceConfig {
            id: "remote".to_string(),
            name: "Remote".to_string(),
            path: PathBuf::new(),
            location: WorkspaceLocation::Ssh {
                server_id: "srv".to_string(),
                remote_path: "/repo".to_string(),
            },
            pinned: false,
            terminal_shell: "bash".to_string(),
            common_commands: Vec::new(),
        });

        let summary =
            remote_server_summary(&config, &config.remote_servers[0], &HashSet::new(), None);
        assert_eq!(summary.workspace_count, 1);
        assert_eq!(summary.status, REMOTE_SERVER_STATUS_UNKNOWN);
    }

    #[test]
    fn server_summary_treats_ready_version_as_available_sidecar() {
        let workspace_dir = tempfile::tempdir().expect("workspace");
        let mut config = GlobalConfig::first_run(workspace_dir.path().to_path_buf());
        config.remote_servers.push(RemoteServerProfile {
            id: "srv".to_string(),
            name: "Srv".to_string(),
            host_alias: "srv".to_string(),
            last_checked_at: Some("2026-07-07T00:00:00Z".to_string()),
            last_sidecar_version: Some("0.1.0".to_string()),
            sidecar_install_state: Some(SIDECAR_INSTALL_STATE_AVAILABLE.to_string()),
            ..RemoteServerProfile::default()
        });
        assert_eq!(
            remote_server_summary(&config, &config.remote_servers[0], &HashSet::new(), None)
                .sidecar_install_state,
            SIDECAR_INSTALL_STATE_AVAILABLE
        );

        config.remote_servers[0].sidecar_install_state =
            Some(SIDECAR_INSTALL_STATE_NOT_INSTALLED.to_string());
        let connected_ids = HashSet::from(["srv".to_string()]);
        let connected_summary =
            remote_server_summary(&config, &config.remote_servers[0], &connected_ids, None);
        assert_eq!(connected_summary.status, REMOTE_SERVER_STATUS_CONNECTED);
        assert_eq!(
            connected_summary.sidecar_install_state,
            SIDECAR_INSTALL_STATE_AVAILABLE
        );
    }

    #[test]
    fn server_summary_exposes_auth_flags_without_password() {
        let workspace_dir = tempfile::tempdir().expect("workspace");
        let mut config = GlobalConfig::first_run(workspace_dir.path().to_path_buf());
        config.remote_servers.push(RemoteServerProfile {
            id: "srv".to_string(),
            name: "Srv".to_string(),
            host_alias: "srv".to_string(),
            auth_method: RemoteAuthMethod::Password,
            password: Some("super-secret-password".to_string()),
            ..RemoteServerProfile::default()
        });

        let summary =
            remote_server_summary(&config, &config.remote_servers[0], &HashSet::new(), None);
        assert_eq!(summary.auth_method, RemoteAuthMethod::Password);
        assert!(summary.password_configured);
        let json = serde_json::to_string(&summary).expect("summary json");
        assert!(!json.contains("super-secret-password"));
        assert!(!json.contains("\"password\":"));
        assert!(json.contains("passwordConfigured"));
        assert!(json.contains("\"authMethod\":\"password\""));
    }

    #[test]
    fn create_applies_root_and_home_defaults() {
        let server = remote_server_from_input(
            RemoteServerInput {
                id: None,
                name: "Box".to_string(),
                host_alias: "box.example".to_string(),
                user: None,
                port: None,
                identity_file: None,
                auth_method: None,
                password: None,
                default_remote_root: None,
                foco_command: None,
                terminal_shell: None,
                connect_timeout_ms: None,
            },
            None,
        )
        .expect("create");
        assert_eq!(server.user.as_deref(), Some("root"));
        assert_eq!(server.default_remote_root.as_deref(), Some("~"));
        assert_eq!(server.auth_method, RemoteAuthMethod::Key);
        assert!(!server.password_configured());
    }

    #[test]
    fn edit_does_not_apply_create_defaults() {
        let existing = RemoteServerProfile {
            id: "srv".to_string(),
            name: "Srv".to_string(),
            host_alias: "old".to_string(),
            user: Some("deploy".to_string()),
            default_remote_root: Some("/srv".to_string()),
            auth_method: RemoteAuthMethod::Password,
            password: Some("keep-me".to_string()),
            ..RemoteServerProfile::default()
        };
        let updated = remote_server_from_input(
            RemoteServerInput {
                id: Some(existing.id.clone()),
                name: "Srv".to_string(),
                host_alias: "new".to_string(),
                user: None,
                port: None,
                identity_file: None,
                auth_method: Some(RemoteAuthMethod::Password),
                password: None,
                default_remote_root: None,
                foco_command: None,
                terminal_shell: None,
                connect_timeout_ms: None,
            },
            Some(&existing),
        )
        .expect("update");
        assert_eq!(updated.user, None);
        assert_eq!(updated.default_remote_root, None);
        assert_eq!(updated.password.as_deref(), Some("keep-me"));
        assert_ne!(updated.user.as_deref(), Some("root"));
        assert_ne!(updated.default_remote_root.as_deref(), Some("~"));
    }

    #[test]
    fn password_mode_requires_password_on_create_and_clears_on_key() {
        let err = remote_server_from_input(
            RemoteServerInput {
                id: None,
                name: "Box".to_string(),
                host_alias: "box".to_string(),
                user: None,
                port: None,
                identity_file: None,
                auth_method: Some(RemoteAuthMethod::Password),
                password: None,
                default_remote_root: None,
                foco_command: None,
                terminal_shell: None,
                connect_timeout_ms: None,
            },
            None,
        )
        .expect_err("password required");
        assert!(format!("{err:?}").contains("password is required"));

        let existing = RemoteServerProfile {
            id: "srv".to_string(),
            name: "Srv".to_string(),
            host_alias: "box".to_string(),
            auth_method: RemoteAuthMethod::Password,
            password: Some("secret".to_string()),
            ..RemoteServerProfile::default()
        };
        let as_key = remote_server_from_input(
            RemoteServerInput {
                id: Some(existing.id.clone()),
                name: existing.name.clone(),
                host_alias: existing.host_alias.clone(),
                user: existing.user.clone(),
                port: existing.port,
                identity_file: None,
                auth_method: Some(RemoteAuthMethod::Key),
                password: Some("should-be-ignored".to_string()),
                default_remote_root: existing.default_remote_root.clone(),
                foco_command: None,
                terminal_shell: None,
                connect_timeout_ms: Some(existing.connect_timeout_ms),
            },
            Some(&existing),
        )
        .expect("switch to key");
        assert_eq!(as_key.auth_method, RemoteAuthMethod::Key);
        assert!(as_key.password.is_none());
    }
}
