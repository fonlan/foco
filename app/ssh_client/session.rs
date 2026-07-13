//! High-level SSH session wrapping `russh::client::Handle`.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::client::{self, Handle};
use russh::keys::{self, PrivateKey, PrivateKeyWithHashAlg, PublicKey};
use russh::{Channel, ChannelMsg, Disconnect, Preferred};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::warn;

use super::config::{ResolvedSshProfile, StrictHostKeyChecking};
use super::error::{SshError, SshErrorKind, map_russh_error};
use super::known_hosts::{
    HostKeyStatus, host_key_info, trust_host_key_if_fingerprint_matches, verify_server_key_path,
};

/// Result of a finished remote command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshCommandResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_status: Option<u32>,
}

/// Long-lived exec channel (e.g. sidecar process).
pub struct SshSpawnedExec {
    pub channel: Channel<client::Msg>,
}

#[derive(Default)]
struct HostKeyGate {
    presented: Option<PublicKey>,
    decision: Option<Result<(), SshError>>,
}

struct ClientHandler {
    hostname: String,
    port: u16,
    known_hosts_path: std::path::PathBuf,
    policy: StrictHostKeyChecking,
    expected_fingerprint: Option<String>,
    gate: Arc<Mutex<HostKeyGate>>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        if let Ok(mut gate) = self.gate.lock() {
            gate.presented = Some(server_public_key.clone());
        }

        if let Some(expected) = self.expected_fingerprint.as_deref() {
            let info = host_key_info(
                &self.hostname,
                self.port,
                server_public_key,
                &self.known_hosts_path,
            );
            let matches = info
                .fingerprint_sha256
                .eq_ignore_ascii_case(expected.trim());
            if !matches {
                let err = SshError::with_host_key(
                    SshErrorKind::HostKeyChanged,
                    format!(
                        "host key fingerprint mismatch during trust reconnect: expected {}, got {}",
                        expected, info.fingerprint_sha256
                    ),
                    info,
                );
                if let Ok(mut gate) = self.gate.lock() {
                    gate.decision = Some(Err(err));
                }
                return Ok(false);
            }
            if let Ok(mut gate) = self.gate.lock() {
                gate.decision = Some(Ok(()));
            }
            return Ok(true);
        }

        let decision = match verify_server_key_path(
            &self.hostname,
            self.port,
            server_public_key,
            &self.known_hosts_path,
            self.policy,
        ) {
            Ok(HostKeyStatus::Trusted) => Ok(()),
            Ok(HostKeyStatus::Unknown { info }) => Err(SshError::with_host_key(
                SshErrorKind::HostKeyUnknown,
                format!(
                    "unknown SSH host key for {}:{} ({}, {})",
                    info.host, info.port, info.algorithm, info.fingerprint_sha256
                ),
                info,
            )),
            Ok(HostKeyStatus::Changed { info }) => Err(SshError::with_host_key(
                SshErrorKind::HostKeyChanged,
                format!(
                    "SSH host key for {}:{} has changed ({})",
                    info.host, info.port, info.fingerprint_sha256
                ),
                info,
            )),
            Err(err) => Err(err),
        };

        let accept = decision.is_ok();
        if let Ok(mut gate) = self.gate.lock() {
            gate.decision = Some(decision);
        }
        Ok(accept)
    }
}

/// Authenticated SSH client session.
pub struct SshSession {
    handle: Handle<ClientHandler>,
    profile: ResolvedSshProfile,
}

impl SshSession {
    /// TCP connect + host-key check + authenticate.
    pub async fn connect(profile: &ResolvedSshProfile) -> Result<Self, SshError> {
        connect_with_optional_trust(profile, None).await
    }

    pub fn profile(&self) -> &ResolvedSshProfile {
        &self.profile
    }

    /// Run a remote command to completion, capturing stdout/stderr/exit.
    pub async fn exec(&mut self, command: &str) -> Result<SshCommandResult, SshError> {
        self.exec_with_stdin(command, &[]).await
    }

    /// Run a remote command with stdin bytes (used for sidecar binary upload).
    pub async fn exec_with_stdin(
        &mut self,
        command: &str,
        stdin: &[u8],
    ) -> Result<SshCommandResult, SshError> {
        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(map_russh_error)?;
        channel.exec(true, command).await.map_err(map_russh_error)?;
        if !stdin.is_empty() {
            for chunk in stdin.chunks(32 * 1024) {
                channel.data(&chunk[..]).await.map_err(map_russh_error)?;
            }
        }
        channel.eof().await.map_err(map_russh_error)?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_status = None;
        loop {
            match channel.wait().await {
                None => break,
                Some(ChannelMsg::Data { ref data }) => stdout.extend_from_slice(data),
                Some(ChannelMsg::ExtendedData { ref data, ext }) if ext == 1 => {
                    stderr.extend_from_slice(data);
                }
                Some(ChannelMsg::ExitStatus { exit_status: code }) => {
                    exit_status = Some(code);
                }
                Some(_) => {}
            }
        }
        Ok(SshCommandResult {
            stdout,
            stderr,
            exit_status,
        })
    }

    /// Open an exec channel without draining it (long-running sidecar).
    pub async fn spawn_exec(&mut self, command: &str) -> Result<SshSpawnedExec, SshError> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(map_russh_error)?;
        channel.exec(true, command).await.map_err(map_russh_error)?;
        Ok(SshSpawnedExec { channel })
    }

    /// Open a `direct-tcpip` channel (local port-forward style).
    pub async fn open_direct_tcpip(
        &mut self,
        host_to_connect: &str,
        port_to_connect: u16,
        originator_address: &str,
        originator_port: u16,
    ) -> Result<Channel<client::Msg>, SshError> {
        self.handle
            .channel_open_direct_tcpip(
                host_to_connect,
                u32::from(port_to_connect),
                originator_address,
                u32::from(originator_port),
            )
            .await
            .map_err(map_russh_error)
    }

    pub async fn disconnect(self) -> Result<(), SshError> {
        self.handle
            .disconnect(Disconnect::ByApplication, "foco disconnect", "")
            .await
            .map_err(map_russh_error)
    }
}

/// Connect and authenticate. When `expected_fingerprint` is set, accept only that
/// host key and write known_hosts after a successful rematch (trust confirmation).
pub async fn connect_with_optional_trust(
    profile: &ResolvedSshProfile,
    expected_fingerprint: Option<&str>,
) -> Result<SshSession, SshError> {
    let gate = Arc::new(Mutex::new(HostKeyGate::default()));
    let handler = ClientHandler {
        hostname: profile.hostname.clone(),
        port: profile.port,
        known_hosts_path: profile.known_hosts_path.clone(),
        policy: profile.strict_host_key_checking,
        expected_fingerprint: expected_fingerprint.map(str::to_string),
        gate: Arc::clone(&gate),
    };

    let config = Arc::new(client::Config {
        inactivity_timeout: Some(profile.connect_timeout.max(Duration::from_secs(30))),
        keepalive_interval: Some(Duration::from_secs(15)),
        keepalive_max: 3,
        preferred: Preferred::DEFAULT,
        ..Default::default()
    });

    let addr = format!("{}:{}", profile.hostname, profile.port);
    let connect_timeout = profile.connect_timeout;
    let connect_future = async {
        let stream = TcpStream::connect(&addr).await.map_err(SshError::from)?;
        stream.set_nodelay(true).ok();
        client::connect_stream(config, stream, handler)
            .await
            .map_err(|err| {
                if let Ok(gate) = gate.lock() {
                    if let Some(Err(host_err)) = gate.decision.clone() {
                        return host_err;
                    }
                }
                map_russh_error(err)
            })
    };

    let mut handle = match timeout(connect_timeout, connect_future).await {
        Ok(result) => result?,
        Err(_) => {
            return Err(SshError::new(
                SshErrorKind::Timeout,
                format!(
                    "ssh connect timed out after {}ms ({addr})",
                    connect_timeout.as_millis()
                ),
            ));
        }
    };

    if let Ok(gate) = gate.lock() {
        if let Some(Err(err)) = gate.decision.clone() {
            let _ = handle
                .disconnect(Disconnect::ByApplication, "host key rejected", "")
                .await;
            return Err(err);
        }
    }

    if let Some(expected) = expected_fingerprint {
        let presented = gate
            .lock()
            .ok()
            .and_then(|gate| gate.presented.clone())
            .ok_or_else(|| {
                SshError::new(
                    SshErrorKind::HostKeyUnknown,
                    "server did not present a host key during trust reconnect",
                )
            })?;
        trust_host_key_if_fingerprint_matches(
            &profile.hostname,
            profile.port,
            &presented,
            &profile.known_hosts_path,
            expected,
        )?;
    }

    authenticate(&mut handle, profile).await?;

    Ok(SshSession {
        handle,
        profile: profile.clone(),
    })
}

/// Public helper for UI trust confirmation: reconnect and write known_hosts only
/// when the live fingerprint still matches the user-confirmed value.
pub async fn trust_and_connect(
    profile: &ResolvedSshProfile,
    expected_fingerprint_sha256: &str,
) -> Result<SshSession, SshError> {
    connect_with_optional_trust(profile, Some(expected_fingerprint_sha256)).await
}

async fn authenticate(
    handle: &mut Handle<ClientHandler>,
    profile: &ResolvedSshProfile,
) -> Result<(), SshError> {
    let user = profile.user.as_str();

    for path in &profile.auth.identity_files {
        if try_public_key_auth(handle, user, path).await? {
            return Ok(());
        }
    }

    if let Some(password) = profile.auth.password.as_deref() {
        let result = handle
            .authenticate_password(user, password)
            .await
            .map_err(map_russh_error)?;
        if result.success() {
            return Ok(());
        }
    }

    if profile.auth.use_agent && try_agent_auth(handle, user).await? {
        return Ok(());
    }

    if profile.auth.identity_files.is_empty() {
        for candidate in default_identity_candidates() {
            if candidate.is_file() && try_public_key_auth(handle, user, &candidate).await? {
                return Ok(());
            }
        }
    }

    Err(SshError::new(
        SshErrorKind::AuthenticationFailed,
        format!("ssh authentication failed for user {user}"),
    ))
}

async fn try_public_key_auth(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    path: &Path,
) -> Result<bool, SshError> {
    let key = match load_private_key(path) {
        Ok(key) => key,
        Err(err) => {
            warn!(
              path = %path.display(),
              error = %err.message(),
              "skipping unreadable SSH identity file"
            );
            return Ok(false);
        }
    };
    let hash_alg = handle
        .best_supported_rsa_hash()
        .await
        .map_err(map_russh_error)?
        .flatten();
    let key = PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg);
    let result = handle
        .authenticate_publickey(user, key)
        .await
        .map_err(map_russh_error)?;
    Ok(result.success())
}

async fn try_agent_auth(handle: &mut Handle<ClientHandler>, user: &str) -> Result<bool, SshError> {
    #[cfg(unix)]
    {
        use russh::keys::agent::client::AgentClient;

        let socket = match std::env::var_os("SSH_AUTH_SOCK") {
            Some(path) if !path.is_empty() => path,
            _ => return Ok(false),
        };
        let Ok(mut agent) = AgentClient::connect_uds(socket).await else {
            return Ok(false);
        };
        let Ok(identities) = agent.request_identities().await else {
            return Ok(false);
        };
        for identity in identities {
            let pubkey = identity.public_key().into_owned();
            let hash_alg = handle
                .best_supported_rsa_hash()
                .await
                .map_err(map_russh_error)?
                .flatten();
            match handle
                .authenticate_publickey_with(user, pubkey, hash_alg, &mut agent)
                .await
            {
                Ok(result) if result.success() => return Ok(true),
                _ => continue,
            }
        }
        Ok(false)
    }
    #[cfg(not(unix))]
    {
        let _ = (handle, user);
        Ok(false)
    }
}

fn load_private_key(path: &Path) -> Result<PrivateKey, SshError> {
    keys::load_secret_key(path, None).map_err(|source| {
        SshError::new(
            SshErrorKind::AuthenticationFailed,
            format!("failed to load SSH identity {}: {source}", path.display()),
        )
    })
}

fn default_identity_candidates() -> Vec<std::path::PathBuf> {
    let Some(home) = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
    else {
        return Vec::new();
    };
    let ssh = home.join(".ssh");
    [
        "id_ed25519",
        "id_rsa",
        "id_ecdsa",
        "id_ecdsa_sk",
        "id_ed25519_sk",
    ]
    .into_iter()
    .map(|name| ssh.join(name))
    .collect()
}
