//! In-process russh SSH server fixture and end-to-end client tests.
//!
//! Does not spawn system `ssh`/`sshd` and does not read the developer's real
//! `~/.ssh` credentials. Temporary host/client keys and known_hosts live under
//! `tempfile` directories owned by each test.

#![cfg(test)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use foco_store::config::{RemoteAuthMethod, RemoteServerProfile};
use russh::keys::{Algorithm, HashAlg, PrivateKey, PublicKey};
use russh::server::{Auth, Msg, Server as _, Session};
use russh::{Channel, ChannelId, ChannelMsg, MethodKind, MethodSet};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, oneshot};

use super::config::{ResolveSshOptions, ResolvedSshProfile, resolve_ssh_profile};
use super::error::SshErrorKind;
use super::session::{SshSession, trust_host_key};

const USER: &str = "foco";
const PASSWORD: &str = "test-password-not-real";
const AUTH_REJECTION: Duration = Duration::from_millis(1);

/// Shared credentials and counters for every accepted connection.
#[derive(Clone)]
struct FixtureState {
    password: String,
    allowed_pubkey: PublicKey,
    reject_pubkey: bool,
    reject_password: bool,
    #[allow(dead_code)] // retained for host-key identity assertions
    host_key: Arc<PrivateKey>,
    stdin_bytes: Arc<AtomicUsize>,
    exec_count: Arc<AtomicUsize>,
    direct_tcpip_count: Arc<AtomicUsize>,
    drop_after_accept: Arc<AtomicBool>,
}

struct TestSshServer {
    state: FixtureState,
}

struct TestHandler {
    state: FixtureState,
    channels: Arc<Mutex<HashMap<ChannelId, Channel<Msg>>>>,
    stdin: Arc<Mutex<HashMap<ChannelId, Vec<u8>>>>,
    pending_exec: Arc<Mutex<HashMap<ChannelId, String>>>,
}

impl russh::server::Server for TestSshServer {
    type Handler = TestHandler;

    fn new_client(&mut self, _: Option<SocketAddr>) -> Self::Handler {
        TestHandler {
            state: self.state.clone(),
            channels: Arc::new(Mutex::new(HashMap::new())),
            stdin: Arc::new(Mutex::new(HashMap::new())),
            pending_exec: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl russh::server::Handler for TestHandler {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        if self.state.reject_password || user != USER || password != self.state.password {
            return Ok(Auth::reject());
        }
        Ok(Auth::Accept)
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        if self.state.reject_pubkey || user != USER || public_key != &self.state.allowed_pubkey {
            return Ok(Auth::reject());
        }
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let id = channel.id();
        {
            let mut channels = self.channels.lock().await;
            channels.insert(id, channel);
        }
        {
            let mut stdin = self.stdin.lock().await;
            stdin.insert(id, Vec::new());
        }
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.state.direct_tcpip_count.fetch_add(1, Ordering::SeqCst);
        let host = host_to_connect.to_string();
        let port = port_to_connect as u16;
        // Only loopback targets in the fixture (matches production sidecar forward).
        if host != "127.0.0.1" && host != "localhost" {
            // Drop handle → automatic reject.
            return Ok(());
        }
        reply.accept().await;
        tokio::spawn(async move {
            let Ok(mut stream) = TcpStream::connect((host.as_str(), port)).await else {
                let _ = channel.close().await;
                return;
            };
            let mut channel = channel;
            let mut buf = [0u8; 16 * 1024];
            loop {
                tokio::select! {
                    msg = channel.wait() => {
                        match msg {
                            Some(ChannelMsg::Data { ref data }) => {
                                if stream.write_all(data).await.is_err() {
                                    break;
                                }
                            }
                            Some(ChannelMsg::Eof) | None => {
                                let _ = stream.shutdown().await;
                                break;
                            }
                            _ => {}
                        }
                    }
                    read = stream.read(&mut buf) => {
                        match read {
                            Ok(0) => {
                                let _ = channel.eof().await;
                                break;
                            }
                            Ok(n) => {
                                if channel.data(&buf[..n]).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
            let _ = channel.close().await;
        });
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let mut stdin = self.stdin.lock().await;
        if let Some(buf) = stdin.get_mut(&channel) {
            buf.extend_from_slice(data);
            self.state
                .stdin_bytes
                .fetch_add(data.len(), Ordering::SeqCst);
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        // Complete stdin-dependent commands after EOF (data may arrive after exec_request).
        let command = {
            let mut pending = self.pending_exec.lock().await;
            pending.remove(&channel)
        };
        if let Some(command) = command {
            if command.starts_with("echo-stdin-len") {
                let stdin = {
                    let map = self.stdin.lock().await;
                    map.get(&channel).cloned().unwrap_or_default()
                };
                let msg = format!("{}\n", stdin.len());
                session.data(channel, msg.into_bytes())?;
                session.exit_status_request(channel, 0)?;
                session.eof(channel)?;
                session.close(channel)?;
            }
        }
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.state.exec_count.fetch_add(1, Ordering::SeqCst);
        let command = String::from_utf8_lossy(data).into_owned();
        session.channel_success(channel)?;

        if command.starts_with("bootstrap") {
            // Long-lived channel: one bootstrap line, then hold until disconnect.
            let line =
                format!("{{\"ok\":true,\"port\":9,\"token\":\"fixture\",\"version\":\"0.0.0\"}}\n");
            session.data(channel, line.into_bytes())?;
            if self.state.drop_after_accept.load(Ordering::SeqCst) {
                // Controllable disconnect without sleep races.
                session.disconnect(russh::Disconnect::ByApplication, "fixture drop", "")?;
            }
            return Ok(());
        }

        if command.starts_with("echo-stdin-len") {
            // Defer response until channel_eof so full stdin is available.
            let mut pending = self.pending_exec.lock().await;
            pending.insert(channel, command);
            return Ok(());
        }

        if command.starts_with("fail ") {
            let msg = command.trim_start_matches("fail ").to_string() + "\n";
            session.extended_data(channel, 1, msg.into_bytes())?;
            session.exit_status_request(channel, 7)?;
            session.eof(channel)?;
            session.close(channel)?;
            return Ok(());
        }

        // Default: stdout = command + newline, exit 0.
        let out = format!("{command}\n");
        session.data(channel, out.into_bytes())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
}

struct EmbeddedSsh {
    addr: SocketAddr,
    dir: tempfile::TempDir,
    known_hosts: PathBuf,
    client_key_path: PathBuf,
    wrong_client_key_path: PathBuf,
    host_key: Arc<PrivateKey>,
    state: FixtureState,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<()>>,
}

impl EmbeddedSsh {
    async fn start() -> Self {
        Self::start_with(|_s| {}).await
    }

    async fn start_with(configure: impl FnOnce(&mut FixtureState)) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let host_key =
            Arc::new(PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("host key"));
        let client_key =
            PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("client key");
        let wrong_key =
            PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("wrong key");

        let client_key_path = dir.path().join("id_ed25519");
        let wrong_client_key_path = dir.path().join("id_wrong");
        write_openssh_key(&client_key_path, &client_key);
        write_openssh_key(&wrong_client_key_path, &wrong_key);

        let known_hosts = dir.path().join("known_hosts");
        // Empty known_hosts until a test pre-trusts the host key.

        let mut state = FixtureState {
            password: PASSWORD.to_string(),
            allowed_pubkey: client_key.public_key().clone(),
            reject_pubkey: false,
            reject_password: false,
            host_key: host_key.clone(),
            stdin_bytes: Arc::new(AtomicUsize::new(0)),
            exec_count: Arc::new(AtomicUsize::new(0)),
            direct_tcpip_count: Arc::new(AtomicUsize::new(0)),
            drop_after_accept: Arc::new(AtomicBool::new(false)),
        };
        configure(&mut state);

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");

        let config = russh::server::Config {
            auth_rejection_time: AUTH_REJECTION,
            auth_rejection_time_initial: Some(Duration::from_millis(0)),
            keys: vec![(*host_key).clone()],
            methods: {
                let mut methods = MethodSet::empty();
                methods.push(MethodKind::Password);
                methods.push(MethodKind::PublicKey);
                methods
            },
            inactivity_timeout: Some(Duration::from_secs(30)),
            keepalive_interval: Some(Duration::from_secs(2)),
            keepalive_max: 3,
            nodelay: true,
            ..Default::default()
        };
        let config = Arc::new(config);
        let mut server = TestSshServer {
            state: state.clone(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let join = tokio::spawn(async move {
            let run = server.run_on_socket(config, &listener);
            let handle = run.handle();
            tokio::select! {
                _ = shutdown_rx => {
                    handle.shutdown("test done".into());
                }
                result = run => {
                    let _ = result;
                }
            }
        });

        // Wait until the port accepts TCP (not SSH handshake yet).
        let ready_deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            if TcpStream::connect(addr).await.is_ok() {
                break;
            }
            if tokio::time::Instant::now() >= ready_deadline {
                panic!("embedded SSH fixture not listening on {addr}");
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        Self {
            addr,
            dir,
            known_hosts,
            client_key_path,
            wrong_client_key_path,
            host_key,
            state,
            shutdown: Some(shutdown_tx),
            join: Some(join),
        }
    }

    fn port(&self) -> u16 {
        self.addr.port()
    }

    fn pretrust_host_key(&self) {
        russh::keys::known_hosts::learn_known_hosts_path(
            "127.0.0.1",
            self.port(),
            self.host_key.public_key(),
            &self.known_hosts,
        )
        .expect("learn known_hosts");
    }

    fn write_changed_host_key(&self) {
        let other =
            PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("other host key");
        russh::keys::known_hosts::learn_known_hosts_path(
            "127.0.0.1",
            self.port(),
            other.public_key(),
            &self.known_hosts,
        )
        .expect("learn stale known_hosts");
    }

    fn profile_password(&self) -> ResolvedSshProfile {
        self.profile(RemoteAuthMethod::Password, None)
    }

    fn profile_key(&self, identity: &Path) -> ResolvedSshProfile {
        self.profile(RemoteAuthMethod::Key, Some(identity.to_path_buf()))
    }

    fn profile(&self, method: RemoteAuthMethod, identity: Option<PathBuf>) -> ResolvedSshProfile {
        let server = RemoteServerProfile {
            id: "fixture".into(),
            name: "Fixture".into(),
            host_alias: "127.0.0.1".into(),
            user: Some(USER.into()),
            port: Some(self.port()),
            identity_file: identity,
            auth_method: method,
            password: if method == RemoteAuthMethod::Password {
                Some(PASSWORD.into())
            } else {
                None
            },
            connect_timeout_ms: 5_000,
            ..RemoteServerProfile::default()
        };
        let mut resolved = resolve_ssh_profile(
            &server,
            ResolveSshOptions {
                skip_ssh_config: true,
                use_agent: Some(false),
                ..Default::default()
            },
        )
        .expect("resolve");
        resolved.known_hosts_path = self.known_hosts.clone();
        resolved.auth.try_default_identities = false;
        resolved.auth.use_agent = false;
        resolved
    }
}

impl Drop for EmbeddedSsh {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            join.abort();
        }
        // Keep dir until drop of tempfile.
        let _ = &self.dir;
    }
}

fn write_openssh_key(path: &Path, key: &PrivateKey) {
    let openssh = key
        .to_openssh(russh::keys::ssh_key::LineEnding::LF)
        .expect("serialize openssh key");
    std::fs::write(path, openssh.as_bytes()).expect("write key");
}

#[tokio::test]
async fn password_auth_exec_stdout_stderr_exit() {
    let fixture = EmbeddedSsh::start().await;
    fixture.pretrust_host_key();
    let profile = fixture.profile_password();
    let session = SshSession::connect(&profile).await.expect("connect");
    let ok = session.exec("hello-world").await.expect("exec");
    assert!(ok.success());
    assert_eq!(ok.stdout_lossy().trim(), "hello-world");

    let fail = session.exec("fail boom").await.expect("fail exec");
    assert!(!fail.success());
    assert_eq!(fail.exit_status, Some(7));
    assert!(fail.stderr_lossy().contains("boom"));
    session.disconnect().await.expect("disconnect");
}

#[tokio::test]
async fn public_key_auth_and_wrong_key_fail() {
    let fixture = EmbeddedSsh::start().await;
    fixture.pretrust_host_key();

    let good = fixture.profile_key(&fixture.client_key_path);
    let session = SshSession::connect(&good).await.expect("key connect");
    let result = session.exec("true").await.expect("exec");
    assert!(result.success());
    session.disconnect().await.ok();

    let bad = fixture.profile_key(&fixture.wrong_client_key_path);
    let err = SshSession::connect(&bad).await.expect_err("wrong key");
    assert_eq!(err.kind, SshErrorKind::AuthenticationFailed);
}

#[tokio::test]
async fn wrong_password_fails_authentication() {
    let fixture = EmbeddedSsh::start().await;
    fixture.pretrust_host_key();
    let mut profile = fixture.profile_password();
    profile.auth.password = Some("not-the-password".into());
    let err = SshSession::connect(&profile)
        .await
        .expect_err("bad password");
    assert_eq!(err.kind, SshErrorKind::AuthenticationFailed);
}

#[tokio::test]
async fn reject_password_switch_fails_authentication() {
    let fixture = EmbeddedSsh::start_with(|state| {
        state.reject_password = true;
    })
    .await;
    fixture.pretrust_host_key();
    let profile = fixture.profile_password();
    let err = SshSession::connect(&profile)
        .await
        .expect_err("server rejects all passwords");
    assert_eq!(err.kind, SshErrorKind::AuthenticationFailed);
}

#[tokio::test]
async fn unknown_host_key_returns_structured_error() {
    let fixture = EmbeddedSsh::start().await;
    // known_hosts intentionally empty
    let profile = fixture.profile_password();
    let err = SshSession::connect(&profile)
        .await
        .expect_err("unknown host key");
    assert_eq!(err.kind, SshErrorKind::HostKeyUnknown);
    let info = err.host_key.expect("host key payload");
    assert!(info.fingerprint_sha256.starts_with("SHA256:"));
    assert_eq!(info.host, "127.0.0.1");
    assert_eq!(info.port, fixture.port());
}

#[tokio::test]
async fn known_host_key_connects_and_changed_hard_fails() {
    let fixture = EmbeddedSsh::start().await;
    fixture.pretrust_host_key();
    let profile = fixture.profile_password();
    let session = SshSession::connect(&profile).await.expect("known ok");
    session.disconnect().await.ok();

    // Stale different key for same host:port.
    std::fs::write(&fixture.known_hosts, b"").expect("clear");
    fixture.write_changed_host_key();
    let err = SshSession::connect(&profile)
        .await
        .expect_err("changed host key");
    assert_eq!(err.kind, SshErrorKind::HostKeyChanged);
    assert!(err.host_key.is_some());
}

#[tokio::test]
async fn trust_host_key_writes_known_hosts_then_connects() {
    let fixture = EmbeddedSsh::start().await;
    let profile = fixture.profile_password();
    let err = SshSession::connect(&profile)
        .await
        .expect_err("unknown first");
    assert_eq!(err.kind, SshErrorKind::HostKeyUnknown);
    let fingerprint = err
        .host_key
        .as_ref()
        .map(|k| k.fingerprint_sha256.as_str())
        .expect("fingerprint");

    trust_host_key(&profile, fingerprint).await.expect("trust");
    assert!(fixture.known_hosts.is_file());

    let session = SshSession::connect(&profile).await.expect("after trust");
    let result = session.exec("ping").await.expect("exec");
    assert!(result.success());
    session.disconnect().await.ok();
}

#[tokio::test]
async fn large_stdin_upload_counts_bytes() {
    let fixture = EmbeddedSsh::start().await;
    fixture.pretrust_host_key();
    let profile = fixture.profile_password();
    let session = SshSession::connect(&profile).await.expect("connect");
    let payload = vec![0xABu8; 200_000];
    let result = session
        .exec_with_stdin("echo-stdin-len", &payload)
        .await
        .expect("upload");
    assert!(result.success());
    assert_eq!(result.stdout_lossy().trim(), "200000");
    assert!(fixture.state.stdin_bytes.load(Ordering::SeqCst) >= 200_000);
    session.disconnect().await.ok();
}

#[tokio::test]
async fn bootstrap_long_lived_channel_reads_line() {
    let fixture = EmbeddedSsh::start().await;
    fixture.pretrust_host_key();
    let profile = fixture.profile_password();
    let session = SshSession::connect(&profile).await.expect("connect");
    let mut spawned = session.spawn_exec("bootstrap").await.expect("spawn");
    let line = spawned
        .read_line(Duration::from_secs(3))
        .await
        .expect("bootstrap line");
    assert!(line.contains("\"ok\":true"));
    spawned.start_stdout_drain();
    session.disconnect().await.ok();
}

#[tokio::test]
async fn direct_tcpip_loopback_echo() {
    let fixture = EmbeddedSsh::start().await;
    fixture.pretrust_host_key();

    // Local echo server that the SSH server will dial via direct-tcpip.
    let echo = TcpListener::bind("127.0.0.1:0").await.expect("echo bind");
    let echo_port = echo.local_addr().expect("echo addr").port();
    let echo_task = tokio::spawn(async move {
        let (mut sock, _) = echo.accept().await.expect("accept");
        let mut buf = [0u8; 64];
        let n = sock.read(&mut buf).await.expect("read");
        sock.write_all(&buf[..n]).await.expect("write");
        let _ = sock.shutdown().await;
    });

    let profile = fixture.profile_password();
    let session = SshSession::connect(&profile).await.expect("connect");
    let mut channel = session
        .open_direct_tcpip("127.0.0.1", echo_port, "127.0.0.1", 0)
        .await
        .expect("direct-tcpip");
    channel.data(&b"hello-forward"[..]).await.expect("send");
    let mut got = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while got.len() < b"hello-forward".len() {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timeout waiting for forward reply; got={got:?}");
        }
        match tokio::time::timeout(remaining, channel.wait()).await {
            Ok(Some(ChannelMsg::Data { ref data })) => got.extend_from_slice(data),
            Ok(Some(ChannelMsg::Eof)) | Ok(None) => break,
            Ok(Some(_)) => {}
            Err(_) => panic!("timeout channel wait; got={got:?}"),
        }
    }
    assert_eq!(&got, b"hello-forward");
    assert_eq!(fixture.state.direct_tcpip_count.load(Ordering::SeqCst), 1);
    session.disconnect().await.ok();
    echo_task.await.expect("echo task");
}

#[tokio::test]
async fn disconnect_cleans_up_without_hang() {
    let fixture = EmbeddedSsh::start().await;
    fixture.pretrust_host_key();
    let profile = fixture.profile_password();
    let session = SshSession::connect(&profile).await.expect("connect");
    // Controllable cancel: disconnect immediately after connect.
    tokio::time::timeout(Duration::from_secs(2), session.disconnect())
        .await
        .expect("disconnect timed out")
        .expect("disconnect");
    let closed_deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if session.is_closed() {
            break;
        }
        if tokio::time::Instant::now() >= closed_deadline {
            panic!("session did not report closed after disconnect");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // Further I/O must not hang after disconnect.
    let err = tokio::time::timeout(Duration::from_secs(2), session.exec("true"))
        .await
        .expect("post-disconnect exec timed out")
        .expect_err("exec after disconnect should fail");
    assert_ne!(err.kind, SshErrorKind::HostKeyUnknown);
}

#[tokio::test]
async fn server_forced_drop_closes_session_without_hang() {
    let fixture = EmbeddedSsh::start_with(|state| {
        state.drop_after_accept.store(true, Ordering::SeqCst);
    })
    .await;
    fixture.pretrust_host_key();
    let profile = fixture.profile_password();
    let session = SshSession::connect(&profile).await.expect("connect");
    let mut spawned = session.spawn_exec("bootstrap").await.expect("spawn");
    let line = spawned
        .read_line(Duration::from_secs(3))
        .await
        .expect("bootstrap line before drop");
    assert!(line.contains("\"ok\":true"));

    let closed_deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if session.is_closed() {
            break;
        }
        // Controllable disconnect path may surface on next channel op.
        let _ = tokio::time::timeout(Duration::from_millis(50), session.exec("true")).await;
        if tokio::time::Instant::now() >= closed_deadline {
            panic!("session did not close after fixture drop_after_accept");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn idle_session_survives_server_keepalive() {
    // Fixture keepalive_interval=2s; prove idle client still usable after a beat.
    let fixture = EmbeddedSsh::start().await;
    fixture.pretrust_host_key();
    let profile = fixture.profile_password();
    let session = SshSession::connect(&profile).await.expect("connect");
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(
        !session.is_closed(),
        "idle session closed before client keepalive max"
    );
    let result = tokio::time::timeout(Duration::from_secs(3), session.exec("still-alive"))
        .await
        .expect("exec after idle timed out")
        .expect("exec after idle");
    assert!(result.success());
    assert_eq!(result.stdout_lossy().trim(), "still-alive");
    session.disconnect().await.ok();
}

#[tokio::test]
async fn proxy_jump_unsupported_is_stable_error() {
    // Documented boundary: ProxyJump is not implemented; resolve fails hard.
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config");
    std::fs::write(
        &config_path,
        "Host final\n  HostName 10.0.0.9\n  ProxyJump bastion\n",
    )
    .expect("write config");
    let server = RemoteServerProfile {
        id: "x".into(),
        name: "x".into(),
        host_alias: "final".into(),
        ..RemoteServerProfile::default()
    };
    let err = resolve_ssh_profile(
        &server,
        ResolveSshOptions {
            ssh_config_path: Some(config_path),
            ..Default::default()
        },
    )
    .expect_err("proxy jump");
    assert_eq!(err.kind, SshErrorKind::ProxyUnsupported);
    assert!(err.message().contains("ProxyJump"));
}

#[tokio::test]
async fn fingerprint_matches_host_key_algorithm() {
    let fixture = EmbeddedSsh::start().await;
    let profile = fixture.profile_password();
    let err = SshSession::connect(&profile).await.expect_err("unknown");
    let info = err.host_key.expect("info");
    let expected = format!(
        "SHA256:{}",
        fixture
            .host_key
            .public_key()
            .fingerprint(HashAlg::Sha256)
            .to_string()
            .trim_start_matches("SHA256:")
    );
    // russh fingerprint format may already include SHA256: prefix.
    assert!(
        info.fingerprint_sha256 == expected
            || info.fingerprint_sha256.trim_start_matches("SHA256:")
                == expected.trim_start_matches("SHA256:"),
        "got {} expected {}",
        info.fingerprint_sha256,
        expected
    );
    assert!(
        info.algorithm.contains("ed25519") || info.algorithm.contains("ssh-ed25519"),
        "algorithm={}",
        info.algorithm
    );
}
