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

impl SshCommandResult {
    pub fn success(&self) -> bool {
        self.exit_status == Some(0)
    }

    pub fn stdout_lossy(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn stderr_lossy(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }

    /// Compact multi-line details for diagnostics (no secrets expected).
    pub fn details(&self) -> String {
        let mut parts = Vec::new();
        parts.push(format!(
            "exitStatus: {}",
            self.exit_status
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ));
        let stdout = self.stdout_lossy();
        let stderr = self.stderr_lossy();
        if !stdout.trim().is_empty() {
            parts.push(format!("stdout:\n{}", stdout.trim()));
        }
        if !stderr.trim().is_empty() {
            parts.push(format!("stderr:\n{}", stderr.trim()));
        }
        parts.join("\n")
    }
}

/// Bound for diagnostic stderr retained from a long-lived remote process.
const SPAWNED_STDERR_CAP: usize = 8 * 1024;

/// Long-lived exec channel (e.g. sidecar process).
///
/// After `read_line`, call `start_stdout_drain` so the channel is not back-pressured.
pub struct SshSpawnedExec {
    channel: Option<Channel<client::Msg>>,
    line_buf: Vec<u8>,
    drain: Option<tokio::task::JoinHandle<()>>,
    /// Bounded stderr captured during bootstrap and drain (diagnostic only).
    stderr: Arc<Mutex<Vec<u8>>>,
}

impl std::fmt::Debug for SshSpawnedExec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshSpawnedExec")
            .field("has_channel", &self.channel.is_some())
            .field("draining", &self.drain.is_some())
            .finish()
    }
}

impl SshSpawnedExec {
    fn new(channel: Channel<client::Msg>) -> Self {
        Self {
            channel: Some(channel),
            line_buf: Vec::new(),
            drain: None,
            stderr: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Read the first newline-terminated line (bootstrap JSON), with timeout.
    pub async fn read_line(&mut self, max_wait: Duration) -> Result<String, SshError> {
        if self.channel.is_none() {
            return Err(SshError::new(
                SshErrorKind::RemoteCommandFailed,
                "remote exec channel is already draining or closed",
            ));
        }
        let deadline = tokio::time::Instant::now() + max_wait;
        loop {
            if let Some(pos) = self.line_buf.iter().position(|b| *b == b'\n') {
                let line = self.line_buf.drain(..=pos).collect::<Vec<_>>();
                let text = String::from_utf8_lossy(&line).trim().to_string();
                if text.is_empty() {
                    continue;
                }
                return Ok(text);
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(SshError::new(
                    SshErrorKind::Timeout,
                    "timed out waiting for remote process stdout line",
                ));
            }
            let channel = self.channel.as_mut().expect("channel present");
            let msg = timeout(remaining, channel.wait())
                .await
                .map_err(|_| {
                    SshError::new(
                        SshErrorKind::Timeout,
                        "timed out waiting for remote process stdout line",
                    )
                })?
                .ok_or_else(|| {
                    SshError::new(
                        SshErrorKind::RemoteCommandFailed,
                        "remote process closed stdout before producing a line",
                    )
                })?;
            match msg {
                ChannelMsg::Data { ref data } => self.line_buf.extend_from_slice(data),
                ChannelMsg::ExtendedData { ref data, ext } if ext == 1 => {
                    append_bounded(&self.stderr, data, SPAWNED_STDERR_CAP);
                }
                ChannelMsg::ExitStatus { exit_status } => {
                    return Err(SshError::new(
                        SshErrorKind::RemoteCommandFailed,
                        format!(
                            "remote process exited with status {exit_status} before writing a line"
                        ),
                    ));
                }
                ChannelMsg::ExitSignal {
                    ref signal_name,
                    ref error_message,
                    ..
                } => {
                    return Err(SshError::new(
                        SshErrorKind::RemoteCommandFailed,
                        format!(
                            "remote process killed by signal {signal_name:?}: {}",
                            error_message.trim()
                        ),
                    ));
                }
                _ => {}
            }
        }
    }

    /// Drain remaining stdout forever; keep a bounded stderr ring for diagnostics.
    pub fn start_stdout_drain(&mut self) {
        if self.drain.is_some() {
            return;
        }
        let Some(mut channel) = self.channel.take() else {
            return;
        };
        let stderr = Arc::clone(&self.stderr);
        // Flush any leftover bootstrap bytes into the void (already consumed line).
        self.line_buf.clear();
        self.drain = Some(tokio::spawn(async move {
            loop {
                match channel.wait().await {
                    None => break,
                    Some(ChannelMsg::Data { .. }) => {}
                    Some(ChannelMsg::ExtendedData { ref data, ext }) if ext == 1 => {
                        append_bounded(&stderr, data, SPAWNED_STDERR_CAP);
                    }
                    Some(_) => {}
                }
            }
        }));
    }

    /// Best-effort close of the remote channel (idempotent).
    pub async fn close(&mut self) {
        if let Some(channel) = self.channel.as_ref() {
            let _ = channel.close().await;
        }
        if let Some(handle) = self.drain.take() {
            handle.abort();
            let _ = handle.await;
        }
        self.channel = None;
    }

    pub fn diagnostic_stderr(&self) -> String {
        self.stderr
            .lock()
            .map(|buf| String::from_utf8_lossy(&buf).into_owned())
            .unwrap_or_default()
    }
}

impl Drop for SshSpawnedExec {
    fn drop(&mut self) {
        if let Some(handle) = self.drain.take() {
            handle.abort();
        }
    }
}

fn append_bounded(buf: &Arc<Mutex<Vec<u8>>>, data: &[u8], cap: usize) {
    let Ok(mut guard) = buf.lock() else {
        return;
    };
    if guard.len() >= cap {
        return;
    }
    let room = cap - guard.len();
    guard.extend_from_slice(&data[..data.len().min(room)]);
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
            let matches =
                super::known_hosts::fingerprints_equal(&info.fingerprint_sha256, expected);
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

impl std::fmt::Debug for SshSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshSession")
            .field("profile", &self.profile)
            .field("closed", &self.handle.is_closed())
            .finish()
    }
}

impl SshSession {
    /// TCP connect + host-key check + authenticate.
    pub async fn connect(profile: &ResolvedSshProfile) -> Result<Self, SshError> {
        connect_with_optional_trust(profile, None).await
    }

    #[allow(dead_code)] // Used by diagnostics / future reconnect paths.
    pub fn profile(&self) -> &ResolvedSshProfile {
        &self.profile
    }

    /// Run a remote command to completion, capturing stdout/stderr/exit.
    pub async fn exec(&self, command: &str) -> Result<SshCommandResult, SshError> {
        self.exec_with_stdin(command, &[]).await
    }

    /// Run a remote command with stdin bytes (used for sidecar binary upload).
    pub async fn exec_with_stdin(
        &self,
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
                channel
                    .data(std::io::Cursor::new(chunk))
                    .await
                    .map_err(map_russh_error)?;
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
    pub async fn spawn_exec(&self, command: &str) -> Result<SshSpawnedExec, SshError> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(map_russh_error)?;
        channel.exec(true, command).await.map_err(map_russh_error)?;
        Ok(SshSpawnedExec::new(channel))
    }

    /// Open a `direct-tcpip` channel (local port-forward style).
    pub async fn open_direct_tcpip(
        &self,
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

    /// Whether the underlying SSH transport has closed.
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    /// Request disconnect; safe to call multiple times / while shared via `Arc`.
    pub async fn disconnect(&self) -> Result<(), SshError> {
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
    connect_with_optional_trust_inner(profile, expected_fingerprint, true).await
}

/// Public helper for UI trust confirmation: reconnect, write known_hosts when the
/// live fingerprint still matches the user-confirmed value, then disconnect.
///
/// Does **not** authenticate — callers must retry test/connect afterwards so auth
/// failures are not mislabeled as trust failures after known_hosts is written.
pub async fn trust_host_key(
    profile: &ResolvedSshProfile,
    expected_fingerprint_sha256: &str,
) -> Result<(), SshError> {
    let session =
        connect_with_optional_trust_inner(profile, Some(expected_fingerprint_sha256), false)
            .await?;
    let _ = session.disconnect().await;
    Ok(())
}

/// Reconnect with fingerprint check + known_hosts write, then authenticate.
/// Prefer [`trust_host_key`] for the trust API; this remains for callers that
/// want a fully authenticated session after confirming the host key.
#[allow(dead_code)]
pub async fn trust_and_connect(
    profile: &ResolvedSshProfile,
    expected_fingerprint_sha256: &str,
) -> Result<SshSession, SshError> {
    connect_with_optional_trust_inner(profile, Some(expected_fingerprint_sha256), true).await
}

async fn connect_with_optional_trust_inner(
    profile: &ResolvedSshProfile,
    expected_fingerprint: Option<&str>,
    do_authenticate: bool,
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

    let host_key_rejection = gate
        .lock()
        .ok()
        .and_then(|guard| match guard.decision.clone() {
            Some(Err(err)) => Some(err),
            _ => None,
        });
    if let Some(err) = host_key_rejection {
        let _ = handle
            .disconnect(Disconnect::ByApplication, "host key rejected", "")
            .await;
        return Err(err);
    }

    if let Some(expected) = expected_fingerprint {
        let presented = gate
            .lock()
            .ok()
            .and_then(|guard| guard.presented.clone())
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

    // Drop secrets from the retained profile regardless of authentication.
    let mut stored = profile.clone();
    stored.auth.password = None;

    if !do_authenticate {
        return Ok(SshSession {
            handle,
            profile: stored,
        });
    }

    authenticate(&mut handle, profile).await?;

    Ok(SshSession {
        handle,
        profile: stored,
    })
}

async fn authenticate(
    handle: &mut Handle<ClientHandler>,
    profile: &ResolvedSshProfile,
) -> Result<(), SshError> {
    let user = profile.user.as_str();
    let auth_timeout = profile.connect_timeout;

    let auth_future = async {
        match profile.auth.method {
            foco_store::config::RemoteAuthMethod::Password => {
                authenticate_password(handle, user, &profile.auth).await
            }
            foco_store::config::RemoteAuthMethod::Key => {
                authenticate_public_keys(handle, user, &profile.auth).await
            }
        }
    };

    match timeout(auth_timeout, auth_future).await {
        Ok(result) => result,
        Err(_) => Err(SshError::new(
            SshErrorKind::Timeout,
            format!(
                "ssh authentication timed out after {}ms for user {user}",
                auth_timeout.as_millis()
            ),
        )),
    }
}

async fn authenticate_password(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    auth: &super::config::SshAuthConfig,
) -> Result<(), SshError> {
    let password = auth.password.as_deref().filter(|value| !value.is_empty());
    let Some(password) = password else {
        return Err(SshError::new(
            SshErrorKind::AuthenticationFailed,
            format!("ssh password authentication requires a password for user {user}"),
        ));
    };

    // Bounded attempts: password once, then keyboard-interactive once.
    let password_result = handle
        .authenticate_password(user, password)
        .await
        .map_err(map_russh_error)?;
    if password_result.success() {
        return Ok(());
    }

    if try_keyboard_interactive_password(handle, user, password).await? {
        return Ok(());
    }

    Err(SshError::new(
        SshErrorKind::AuthenticationFailed,
        format!("ssh password authentication failed for user {user}"),
    ))
}

/// Answer keyboard-interactive only when every prompt looks like a password request.
async fn try_keyboard_interactive_password(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    password: &str,
) -> Result<bool, SshError> {
    use russh::client::KeyboardInteractiveAuthResponse;

    let mut response = handle
        .authenticate_keyboard_interactive_start(user, None)
        .await
        .map_err(map_russh_error)?;

    // Cap interactive rounds so a chatty server cannot loop forever.
    for _ in 0..4 {
        match response {
            KeyboardInteractiveAuthResponse::Success => return Ok(true),
            KeyboardInteractiveAuthResponse::Failure { .. } => return Ok(false),
            KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                if prompts.is_empty() {
                    response = handle
                        .authenticate_keyboard_interactive_respond(Vec::new())
                        .await
                        .map_err(map_russh_error)?;
                    continue;
                }
                let mut answers = Vec::with_capacity(prompts.len());
                for prompt in &prompts {
                    if !is_password_keyboard_prompt(&prompt.prompt) {
                        return Err(SshError::new(
                            SshErrorKind::AuthenticationFailed,
                            format!(
                                "ssh keyboard-interactive prompt is not a password prompt (refusing to send login password): {}",
                                sanitize_prompt_for_error(&prompt.prompt)
                            ),
                        ));
                    }
                    answers.push(password.to_string());
                }
                response = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await
                    .map_err(map_russh_error)?;
            }
        }
    }

    Err(SshError::new(
        SshErrorKind::AuthenticationFailed,
        "ssh keyboard-interactive authentication exceeded the prompt limit",
    ))
}

fn is_password_keyboard_prompt(prompt: &str) -> bool {
    let normalized = prompt.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    // Reject multi-factor / OTP style prompts even if they contain "password".
    const REJECT: &[&str] = &[
        "otp",
        "one-time",
        "one time",
        "verification code",
        "verify code",
        "authenticator",
        "2fa",
        "mfa",
        "token",
        "pin",
        "sms",
        "duo",
        "yubi",
    ];
    if REJECT.iter().any(|needle| normalized.contains(needle)) {
        return false;
    }
    normalized.contains("password") || normalized.contains("passphrase")
}

fn sanitize_prompt_for_error(prompt: &str) -> String {
    prompt.chars().take(80).collect()
}

async fn authenticate_public_keys(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    auth: &super::config::SshAuthConfig,
) -> Result<(), SshError> {
    // Stable order: configured identity files (explicit profile or ssh_config) → agent → defaults.
    const MAX_KEY_TRIES: usize = 8;
    let mut attempts = 0usize;
    let mut saw_encrypted_key = false;

    for path in &auth.identity_files {
        if attempts >= MAX_KEY_TRIES {
            break;
        }
        attempts += 1;
        match try_public_key_auth(handle, user, path).await {
            Ok(true) => return Ok(()),
            Ok(false) => continue,
            Err(err) if is_encrypted_key_error(&err) => {
                saw_encrypted_key = true;
                warn!(
                  path = %path.display(),
                  "skipping encrypted SSH identity (add to SSH Agent or use an unencrypted key)"
                );
            }
            Err(err) => {
                warn!(
                  path = %path.display(),
                  error = %err.message(),
                  "skipping SSH identity file"
                );
            }
        }
    }

    if auth.use_agent && attempts < MAX_KEY_TRIES {
        if try_agent_auth(handle, user).await? {
            return Ok(());
        }
        attempts += 1;
    }

    if auth.try_default_identities {
        for candidate in default_identity_candidates() {
            if attempts >= MAX_KEY_TRIES {
                break;
            }
            if !candidate.is_file() {
                continue;
            }
            if auth.identity_files.iter().any(|path| path == &candidate) {
                continue;
            }
            attempts += 1;
            match try_public_key_auth(handle, user, &candidate).await {
                Ok(true) => return Ok(()),
                Ok(false) => continue,
                Err(err) if is_encrypted_key_error(&err) => {
                    saw_encrypted_key = true;
                    warn!(
                      path = %candidate.display(),
                      "skipping encrypted default SSH identity (add to SSH Agent or use an unencrypted key)"
                    );
                    continue;
                }
                Err(err) => {
                    warn!(
                      path = %candidate.display(),
                      error = %err.message(),
                      "skipping default SSH identity file"
                    );
                }
            }
        }
    }

    let mut message = format!(
        "ssh public-key authentication failed for user {user}; ensure an unencrypted private key is configured, or add the key to SSH Agent"
    );
    if saw_encrypted_key {
        message.push_str(
            " (at least one identity file is encrypted; login password is not used to decrypt private keys)",
        );
    }
    Err(SshError::new(SshErrorKind::AuthenticationFailed, message))
}

fn is_encrypted_key_error(err: &SshError) -> bool {
    err.message().contains("encrypted")
        || err.message().contains("SSH Agent")
        || err.message().contains("unencrypted")
}

async fn try_public_key_auth(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    path: &Path,
) -> Result<bool, SshError> {
    let key = load_private_key(path)?;
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
        const MAX_AGENT_KEYS: usize = 8;
        for identity in identities.into_iter().take(MAX_AGENT_KEYS) {
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
    #[cfg(windows)]
    {
        use russh::keys::agent::client::AgentClient;

        // Order: OpenSSH named pipe (SSH_AUTH_SOCK or default) → Pageant.
        if let Some(path) = windows_agent_pipe_path() {
            if let Ok(mut agent) = AgentClient::connect_named_pipe(&path).await {
                if try_agent_identities_windows_pipe(handle, user, &mut agent).await? {
                    return Ok(true);
                }
            }
        }
        if let Ok(mut agent) = AgentClient::connect_pageant().await {
            if try_agent_identities_pageant(handle, user, &mut agent).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (handle, user);
        Ok(false)
    }
}

#[cfg(windows)]
fn windows_agent_pipe_path() -> Option<std::ffi::OsString> {
    if let Some(path) = std::env::var_os("SSH_AUTH_SOCK") {
        if !path.is_empty() {
            return Some(path);
        }
    }
    // OpenSSH for Windows default agent pipe.
    Some(std::ffi::OsString::from(r"\\.\pipe\openssh-ssh-agent"))
}

#[cfg(windows)]
async fn try_agent_identities_windows_pipe(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    agent: &mut russh::keys::agent::client::AgentClient<
        tokio::net::windows::named_pipe::NamedPipeClient,
    >,
) -> Result<bool, SshError> {
    try_agent_identities_loop(handle, user, agent).await
}

#[cfg(windows)]
async fn try_agent_identities_pageant(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    agent: &mut russh::keys::agent::client::AgentClient<pageant::PageantStream>,
) -> Result<bool, SshError> {
    try_agent_identities_loop(handle, user, agent).await
}

#[cfg(windows)]
async fn try_agent_identities_loop<S>(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    agent: &mut russh::keys::agent::client::AgentClient<S>,
) -> Result<bool, SshError>
where
    S: russh::keys::agent::client::AgentStream + Unpin + Send + 'static,
{
    let Ok(identities) = agent.request_identities().await else {
        return Ok(false);
    };
    const MAX_AGENT_KEYS: usize = 8;
    for identity in identities.into_iter().take(MAX_AGENT_KEYS) {
        let pubkey = identity.public_key().into_owned();
        let hash_alg = handle
            .best_supported_rsa_hash()
            .await
            .map_err(map_russh_error)?
            .flatten();
        match handle
            .authenticate_publickey_with(user, pubkey, hash_alg, agent)
            .await
        {
            Ok(result) if result.success() => return Ok(true),
            _ => continue,
        }
    }
    Ok(false)
}

fn load_private_key(path: &Path) -> Result<PrivateKey, SshError> {
    match keys::load_secret_key(path, None) {
        Ok(key) => Ok(key),
        Err(keys::Error::KeyIsEncrypted) => Err(SshError::new(
            SshErrorKind::AuthenticationFailed,
            format!(
                "SSH identity {} is encrypted; add it to SSH Agent or use an unencrypted private key (login password is not used to decrypt keys)",
                path.display()
            ),
        )),
        Err(source) => {
            let message = source.to_string();
            if message.to_ascii_lowercase().contains("encrypt")
                || message.to_ascii_lowercase().contains("passphrase")
                || message.to_ascii_lowercase().contains("password")
            {
                return Err(SshError::new(
                    SshErrorKind::AuthenticationFailed,
                    format!(
                        "SSH identity {} appears encrypted or needs a passphrase; add it to SSH Agent or use an unencrypted private key (login password is not used to decrypt keys)",
                        path.display()
                    ),
                ));
            }
            Err(SshError::new(
                SshErrorKind::AuthenticationFailed,
                format!("failed to load SSH identity {}: {source}", path.display()),
            ))
        }
    }
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

// Compile-time Send checks for axum handlers that call SshSession::connect.
const _: () = {
    fn assert_send<T: Send>(_: T) {}
    fn check_connect_is_send(profile: ResolvedSshProfile) {
        assert_send(async move {
            let _ = SshSession::connect(&profile).await;
        });
    }
    let _ = check_connect_is_send;
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_prompts_accept_password_words() {
        assert!(is_password_keyboard_prompt("Password:"));
        assert!(is_password_keyboard_prompt("Enter passphrase for key"));
        assert!(is_password_keyboard_prompt("user's password"));
    }

    #[test]
    fn password_prompts_reject_otp_and_unknown() {
        assert!(!is_password_keyboard_prompt("OTP code:"));
        assert!(!is_password_keyboard_prompt("Verification code"));
        assert!(!is_password_keyboard_prompt("Authenticator PIN"));
        assert!(!is_password_keyboard_prompt("Enter 2FA token"));
        assert!(!is_password_keyboard_prompt("Username:"));
        assert!(!is_password_keyboard_prompt(""));
    }

    #[test]
    fn sanitize_prompt_truncates() {
        let long = "p".repeat(120);
        assert_eq!(sanitize_prompt_for_error(&long).len(), 80);
    }

    #[test]
    fn command_result_success_and_details() {
        let ok = SshCommandResult {
            stdout: b"hi\n".to_vec(),
            stderr: Vec::new(),
            exit_status: Some(0),
        };
        assert!(ok.success());
        assert!(ok.details().contains("exitStatus: 0"));
        assert!(ok.details().contains("stdout:"));

        let fail = SshCommandResult {
            stdout: Vec::new(),
            stderr: b"nope".to_vec(),
            exit_status: Some(1),
        };
        assert!(!fail.success());
    }
}

#[cfg(test)]
mod source_guards {
    use std::fs;
    use std::path::PathBuf;

    fn production_rust_sources() -> Vec<PathBuf> {
        let mut roots = vec![
            PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("store"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("agent"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("tools"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("providers"),
        ];
        let mut files = Vec::new();
        while let Some(root) = roots.pop() {
            let Ok(entries) = fs::read_dir(&root) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if matches!(name, "target" | "node_modules" | ".git" | ".foco") {
                        continue;
                    }
                    roots.push(path);
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    files.push(path);
                }
            }
        }
        files
    }

    #[test]
    fn production_rust_does_not_spawn_system_ssh() {
        // Built from parts so this test file does not contain the banned literals.
        let forbidden = [
            format!("Command::new(\"{}\")", "ssh"),
            format!("Command::new(\"{}\")", "scp"),
            format!("Command::new(\"{}\")", "sftp"),
            format!("{}={}", "BatchMode", "yes"),
            format!("{}=", "ServerAliveInterval"),
            format!("{}=", "ExitOnForwardFailure"),
            ["SSH_", "ASKPASS"].concat(),
            ["remote_server_", "ssh_args"].concat(),
        ];
        let mut violations = Vec::new();
        for path in production_rust_sources() {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            // Strip this module so the guard source is not a self-hit.
            let scan = if path.ends_with("session.rs") {
                text.split("mod source_guards")
                    .next()
                    .unwrap_or(text.as_str())
            } else {
                text.as_str()
            };
            for needle in &forbidden {
                if scan.contains(needle) {
                    violations.push(format!("{} contains {needle}", path.display()));
                }
            }
        }
        assert!(
            violations.is_empty(),
            "system OpenSSH spawn/options must not return:\n{}",
            violations.join("\n")
        );
    }
}
