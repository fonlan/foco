use std::{collections::HashSet, fs, path::PathBuf, process::Output, time::Duration};

use axum::{
    Json,
    extract::{Path as AxumPath, State},
};
use chrono::{SecondsFormat, Utc};
use foco_store::config::{DEFAULT_REMOTE_CONNECT_TIMEOUT_MS, RemoteServerProfile};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{process::Command, time::timeout};

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
    pub(crate) default_remote_root: Option<String>,
    pub(crate) foco_command: Option<String>,
    pub(crate) terminal_shell: Option<String>,
    pub(crate) connect_timeout_ms: u64,
    pub(crate) status: String,
    pub(crate) last_error: Option<String>,
    pub(crate) last_known_target: Option<String>,
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
        servers: remote_server_summaries(&config, &connected_ids),
    }))
}

pub(crate) async fn create_remote_server(
    State(state): State<AppState>,
    Json(input): Json<RemoteServerInput>,
) -> Result<Json<RemoteServerResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
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
    save_config(&state, config.clone())?;
    let connected_ids = connected_remote_server_ids(&state)?;
    Ok(Json(RemoteServerResponse {
        server: remote_server_summary(&config, &server, &connected_ids),
    }))
}

pub(crate) async fn update_remote_server(
    State(state): State<AppState>,
    Json(input): Json<RemoteServerInput>,
) -> Result<Json<RemoteServerResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
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
    save_config(&state, config.clone())?;
    let connected_ids = connected_remote_server_ids(&state)?;
    Ok(Json(RemoteServerResponse {
        server: remote_server_summary(&config, &server, &connected_ids),
    }))
}

pub(crate) async fn delete_remote_server(
    State(state): State<AppState>,
    Json(request): Json<RemoteServerIdRequest>,
) -> Result<Json<DeleteRemoteServerResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
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
    save_config(&state, config)?;
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
        server: remote_server_summary(&config, &server, &connected_ids),
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
        server: remote_server_summary(&config, &server, &connected_ids),
    }))
}

pub(crate) fn remote_server_summaries(
    config: &GlobalConfig,
    connected_ids: &HashSet<String>,
) -> Vec<RemoteServerSummary> {
    config
        .remote_servers
        .iter()
        .map(|server| remote_server_summary(config, server, connected_ids))
        .collect()
}

pub(crate) fn remote_server_summary(
    config: &GlobalConfig,
    server: &RemoteServerProfile,
    connected_ids: &HashSet<String>,
) -> RemoteServerSummary {
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
        default_remote_root: server.default_remote_root.clone(),
        foco_command: server.foco_command.clone(),
        terminal_shell: server.terminal_shell.clone(),
        connect_timeout_ms: server.connect_timeout_ms,
        status: remote_server_status_value(server, connected_ids),
        last_error: server.last_error.clone(),
        last_known_target: server.last_known_target.clone(),
        sidecar_install_state: server
            .sidecar_install_state
            .clone()
            .unwrap_or_else(|| SIDECAR_INSTALL_STATE_UNKNOWN.to_string()),
        workspace_count: workspace_count_for_server(config, &server.id),
        last_checked_at: server.last_checked_at.clone(),
    }
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
    let result = test_remote_server_connection(&server).await;

    let mut config = config_snapshot(&state)?;
    let updated = update_remote_server_diagnostic_cache(&mut config, &server_id, &result)?;
    save_config(&state, config.clone())?;

    if mark_connected && result.ok {
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
        server: remote_server_summary(&config, &updated, &connected_ids),
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

    match run_ssh(server, &["-G", server.host_alias.as_str()], false).await {
        Ok(output) if output.status.success() => {
            stages[0] = success_stage(
                "ssh",
                "OpenSSH configuration parsed and BatchMode login verified",
                output_details(&output),
            );
        }
        Ok(output) => {
            let kind = classify_ssh_failure(&output);
            stages[0] = failed_stage(
                "ssh",
                kind,
                "OpenSSH could not resolve this server",
                output_details(&output),
            );
            return diagnostic_result(stages);
        }
        Err(message) => {
            stages[0] = failed_stage("ssh", "startup_failed", message, None);
            return diagnostic_result(stages);
        }
    }

    match run_ssh(server, &["true"], true).await {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let kind = classify_ssh_failure(&output);
            stages[0] = failed_stage(
                "ssh",
                kind,
                "SSH BatchMode login failed",
                output_details(&output),
            );
            return diagnostic_result(stages);
        }
        Err(message) => {
            stages[0] = failed_stage("ssh", "startup_failed", message, None);
            return diagnostic_result(stages);
        }
    }

    let target = match run_ssh(server, &["uname -s && uname -m"], true).await {
        Ok(output) if output.status.success() => {
            match normalize_target(&String::from_utf8_lossy(&output.stdout)) {
                Ok(target) => {
                    stages[1] = success_stage(
                        "target",
                        format!("Detected target {target}"),
                        output_details(&output),
                    );
                    target
                }
                Err(message) => {
                    stages[1] = failed_stage(
                        "target",
                        "target_unsupported",
                        message,
                        output_details(&output),
                    );
                    return diagnostic_result(stages);
                }
            }
        }
        Ok(output) => {
            let kind = classify_ssh_failure(&output);
            stages[1] = failed_stage(
                "target",
                kind,
                "Failed to run uname on remote server",
                output_details(&output),
            );
            return diagnostic_result(stages);
        }
        Err(message) => {
            stages[1] = failed_stage("target", "startup_failed", message, None);
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
            return diagnostic_result(stages);
        }
    };

    match run_ssh(
        server,
        &["mkdir -p ~/.foco/sidecars && test -w ~/.foco/sidecars"],
        true,
    )
    .await
    {
        Ok(output) if output.status.success() => {
            stages[3] = success_stage(
                "remoteInstallDirWritable",
                "Remote install directory is writable",
                output_details(&output),
            );
        }
        Ok(output) => {
            stages[3] = failed_stage(
                "remoteInstallDirWritable",
                "permission_denied",
                "Remote sidecar install directory is not writable",
                output_details(&output),
            );
            return diagnostic_result(stages);
        }
        Err(message) => {
            stages[3] = failed_stage("remoteInstallDirWritable", "startup_failed", message, None);
            return diagnostic_result(stages);
        }
    }

    if let Some(command) = server
        .foco_command
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let version_command = format!("{command} --version");
        match run_ssh(server, &[version_command.as_str()], true).await {
            Ok(output) if output.status.success() => {
                stages[4] = success_stage(
                    "focoCommandVersion",
                    "focoCommand responded to --version",
                    output_details(&output),
                );
            }
            Ok(output) => {
                stages[4] = failed_stage(
                    "focoCommandVersion",
                    "startup_failed",
                    "focoCommand failed to report a version",
                    output_details(&output),
                );
                return diagnostic_result(stages);
            }
            Err(message) => {
                stages[4] = failed_stage("focoCommandVersion", "startup_failed", message, None);
                return diagnostic_result(stages);
            }
        }
    } else if let Some(asset) = sidecar {
        let remote_path = format!("~/.foco/sidecars/{}/{}/foco", asset.version, target);
        let check_command = format!("test -x {remote_path} && {remote_path} --version");
        match run_ssh(server, &[check_command.as_str()], true).await {
            Ok(output) if output.status.success() => {
                stages[4] = success_stage(
                    "focoCommandVersion",
                    "Installed sidecar responded to --version",
                    output_details(&output),
                );
            }
            Ok(output) => {
                stages[4] = skipped_stage(
                    "focoCommandVersion",
                    format!("Packaged sidecar is available but {remote_path} is not installed yet"),
                );
                let mut details = output_details(&output).unwrap_or_default();
                if !details.is_empty() {
                    details.insert_str(0, "Remote version check output:\n");
                    stages[4].details = Some(details);
                }
            }
            Err(message) => {
                stages[4] = skipped_stage("focoCommandVersion", message);
            }
        }
    }

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

    Ok(RemoteServerProfile {
        id,
        name,
        host_alias,
        user: optional_non_empty(input.user),
        port: input.port,
        identity_file: optional_non_empty(input.identity_file).map(PathBuf::from),
        default_remote_root: optional_non_empty(input.default_remote_root),
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
    } else if server.last_checked_at.is_some() {
        REMOTE_SERVER_STATUS_READY.to_string()
    } else {
        REMOTE_SERVER_STATUS_UNKNOWN.to_string()
    }
}

fn disconnect_remote_server_id(state: &AppState, server_id: &str) -> Result<(), ApiError> {
    let mut connections = state
        .remote_server_connections
        .lock()
        .map_err(|_| ApiError::internal("remote server connection lock is poisoned"))?;
    connections.remove(server_id);
    Ok(())
}

async fn run_ssh(
    server: &RemoteServerProfile,
    extra_args: &[&str],
    batch_mode: bool,
) -> Result<Output, String> {
    let timeout_ms = server.connect_timeout_ms.max(1);
    let args = remote_server_ssh_args(server, extra_args, batch_mode);

    let child = Command::new("ssh").args(&args).output();
    timeout(Duration::from_millis(timeout_ms + 1_000), child)
        .await
        .map_err(|_| format!("ssh command timed out after {timeout_ms}ms"))?
        .map_err(|source| format!("failed to run ssh: {source}"))
}

pub(crate) fn remote_server_ssh_args(
    server: &RemoteServerProfile,
    extra_args: &[&str],
    batch_mode: bool,
) -> Vec<String> {
    let timeout_ms = server.connect_timeout_ms.max(1);
    let mut args = Vec::new();
    if batch_mode {
        args.push("-o".to_string());
        args.push("BatchMode=yes".to_string());
    }
    args.push("-o".to_string());
    args.push(format!("ConnectTimeout={}", timeout_ms.div_ceil(1_000)));
    if let Some(user) = server
        .user
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.push("-l".to_string());
        args.push(user.to_string());
    }
    if let Some(port) = server.port {
        args.push("-p".to_string());
        args.push(port.to_string());
    }
    if let Some(identity_file) = &server.identity_file {
        args.push("-i".to_string());
        args.push(identity_file.display().to_string());
    }
    if extra_args.first() == Some(&"-G") {
        args.extend(extra_args.iter().map(|arg| (*arg).to_string()));
    } else {
        args.push(server.host_alias.clone());
        args.extend(extra_args.iter().map(|arg| (*arg).to_string()));
    }
    args
}

fn classify_ssh_failure(output: &Output) -> &'static str {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .to_ascii_lowercase();
    if text.contains("permission denied")
        || text.contains("authentication failed")
        || text.contains("publickey")
        || text.contains("too many authentication failures")
    {
        "authentication_failed"
    } else if text.contains("could not resolve hostname")
        || text.contains("name or service not known")
        || text.contains("connection timed out")
        || text.contains("operation timed out")
        || text.contains("connection refused")
        || text.contains("no route to host")
        || text.contains("network is unreachable")
        || text.contains("host key verification failed")
    {
        "host_unreachable"
    } else {
        "startup_failed"
    }
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

fn output_details(output: &Output) -> Option<String> {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let mut details = Vec::new();
    details.push(format!("exitStatus: {}", output.status));
    if !stdout.is_empty() {
        details.push(format!("stdout:\n{stdout}"));
    }
    if !stderr.is_empty() {
        details.push(format!("stderr:\n{stderr}"));
    }
    Some(details.join("\n"))
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

        let summary = remote_server_summary(&config, &config.remote_servers[0], &HashSet::new());
        assert_eq!(summary.workspace_count, 1);
        assert_eq!(summary.status, REMOTE_SERVER_STATUS_UNKNOWN);
    }
}
