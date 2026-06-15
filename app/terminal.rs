use std::{
    collections::HashSet,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use axum::{
    body::Bytes,
    extract::ws::{Message, WebSocket},
};
use foco_store::workspace::{TerminalSessionRecord, WorkspaceDatabase};
use futures_util::{SinkExt, StreamExt};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{broadcast, mpsc},
    time,
};

const DEFAULT_COLS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;
const TERMINAL_WEBSOCKET_PING_INTERVAL: Duration = Duration::from_secs(10);
const TERMINAL_WEBSOCKET_PING_PAYLOAD: &[u8] = b"foco-terminal-keepalive";
const OSC7_PREFIX: &str = "\x1b]7;file://foco/";
const OSC_BEL: char = '\x07';
const OSC_ST: &str = "\x1b\\";

#[derive(Clone, Default)]
pub(crate) struct TerminalRegistry {
    active_session_ids: Arc<Mutex<HashSet<String>>>,
}

impl TerminalRegistry {
    fn register(&self, session_id: &str) -> Result<TerminalSessionGuard, String> {
        let mut active_session_ids = self
            .active_session_ids
            .lock()
            .map_err(|_| "terminal session registry lock was poisoned".to_string())?;

        if !active_session_ids.insert(session_id.to_string()) {
            return Err(format!("terminal session is already active: {session_id}"));
        }

        Ok(TerminalSessionGuard {
            registry: self.clone(),
            session_id: session_id.to_string(),
        })
    }

    fn unregister(&self, session_id: &str) {
        if let Ok(mut active_session_ids) = self.active_session_ids.lock() {
            active_session_ids.remove(session_id);
        }
    }
}

struct TerminalSessionGuard {
    registry: TerminalRegistry,
    session_id: String,
}

impl Drop for TerminalSessionGuard {
    fn drop(&mut self) {
        self.registry.unregister(&self.session_id);
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
enum ClientTerminalEvent {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
enum ServerTerminalEvent<'a> {
    Started { cwd: &'a str },
    Output { data: &'a str },
    Cwd { cwd: &'a str },
    Exit { status: String },
    Error { message: String },
}

enum PtyOutput {
    Output(String),
    Error(String),
    Exit(String),
    Closed,
}

struct SpawnedTerminal {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

pub(crate) async fn handle_terminal_socket(
    socket: WebSocket,
    mut shutdown_rx: broadcast::Receiver<()>,
    registry: TerminalRegistry,
    workspace_path: PathBuf,
    terminal_shell: String,
    session: TerminalSessionRecord,
    cols: u16,
    rows: u16,
) {
    let guard = match registry.register(&session.id) {
        Ok(guard) => guard,
        Err(message) => {
            send_socket_error(socket, message).await;
            return;
        }
    };

    let cwd = PathBuf::from(&session.working_directory);
    if !cwd.is_dir() {
        send_socket_error(
            socket,
            format!(
                "terminal working directory does not exist: {}",
                cwd.display()
            ),
        )
        .await;
        return;
    }

    let terminal = match spawn_terminal(&cwd, &terminal_shell, cols, rows) {
        Ok(terminal) => terminal,
        Err(message) => {
            send_socket_error(socket, message).await;
            return;
        }
    };

    let (mut sender, mut receiver) = socket.split();
    if send_terminal_event(
        &mut sender,
        &ServerTerminalEvent::Started {
            cwd: &session.working_directory,
        },
    )
    .await
    .is_err()
    {
        return;
    }

    let (output_tx, mut output_rx) = mpsc::unbounded_channel();
    let reader = match terminal.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(source) => {
            let mut child = terminal.child;
            let _ = child.kill();
            if let Err(message) = close_terminal_session(&workspace_path, &session.id) {
                tracing::warn!(session_id = %session.id, error = %message, "failed to close terminal session");
            }
            let _ = send_terminal_event(
                &mut sender,
                &ServerTerminalEvent::Error {
                    message: format!("failed to open terminal output reader: {source}"),
                },
            )
            .await;
            return;
        }
    };
    spawn_reader(reader, output_tx.clone());

    let master = terminal.master;
    let mut writer = terminal.writer;
    let mut child_killer = terminal.child.clone_killer();
    spawn_child_waiter(terminal.child, output_tx);
    let mut cwd_tracker = CwdTracker::default();
    let mut exit_status = None;
    let mut child_exited = false;
    let mut heartbeat = time::interval(TERMINAL_WEBSOCKET_PING_INTERVAL);
    heartbeat.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            websocket_message = receiver.next() => {
                match websocket_message {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientTerminalEvent>(text.as_str()) {
                            Ok(ClientTerminalEvent::Input { data }) => {
                                if let Err(source) = writer.write_all(data.as_bytes()).and_then(|_| writer.flush()) {
                                    let _ = send_terminal_event(
                                        &mut sender,
                                        &ServerTerminalEvent::Error {
                                            message: format!("failed to write terminal input: {source}"),
                                        },
                                    ).await;
                                    break;
                                }
                            }
                            Ok(ClientTerminalEvent::Resize { cols, rows }) => {
                                if let Err(message) = resize_terminal(master.as_ref(), cols, rows) {
                                    let _ = send_terminal_event(
                                        &mut sender,
                                        &ServerTerminalEvent::Error { message },
                                    ).await;
                                }
                            }
                            Err(source) => {
                                let _ = send_terminal_event(
                                    &mut sender,
                                    &ServerTerminalEvent::Error {
                                        message: format!("invalid terminal event: {source}"),
                                    },
                                ).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(source)) => {
                        tracing::debug!(error = %source, session_id = %session.id, "terminal websocket read failed");
                        break;
                    }
                }
            }
            output = output_rx.recv() => {
                match output {
                    Some(PtyOutput::Output(data)) => {
                        if let Some(cwd) = cwd_tracker.observe(&data) {
                            match persist_terminal_cwd(&workspace_path, &session.id, &cwd) {
                                Ok(()) => {
                                    let _ = send_terminal_event(
                                        &mut sender,
                                        &ServerTerminalEvent::Cwd { cwd: &cwd },
                                    ).await;
                                }
                                Err(message) => {
                                    let _ = send_terminal_event(
                                        &mut sender,
                                        &ServerTerminalEvent::Error { message },
                                    ).await;
                                }
                            }
                        }

                        if send_terminal_event(
                            &mut sender,
                            &ServerTerminalEvent::Output { data: &data },
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    Some(PtyOutput::Error(message)) => {
                        let _ = send_terminal_event(
                            &mut sender,
                            &ServerTerminalEvent::Error { message },
                        ).await;
                        break;
                    }
                    Some(PtyOutput::Exit(status)) => {
                        child_exited = true;
                        exit_status = Some(status);
                        break;
                    }
                    Some(PtyOutput::Closed) | None => {
                        exit_status = Some("terminal output stream closed".to_string());
                        break;
                    }
                }
            }
            _ = heartbeat.tick() => {
                if sender
                    .send(Message::Ping(Bytes::from_static(TERMINAL_WEBSOCKET_PING_PAYLOAD)))
                    .await
                    .is_err()
                {
                    exit_status = Some("terminal websocket heartbeat failed".to_string());
                    break;
                }
            }
            shutdown = shutdown_rx.recv() => {
                if shutdown.is_ok() {
                    exit_status = Some("app shutdown".to_string());
                }
                break;
            }
        }
    }

    drop(guard);
    drop(writer);
    drop(master);

    if !child_exited {
        let _ = child_killer.kill();
    }

    if let Err(message) = close_terminal_session(&workspace_path, &session.id) {
        tracing::warn!(session_id = %session.id, error = %message, "failed to close terminal session");
    }

    let _ = send_terminal_event(
        &mut sender,
        &ServerTerminalEvent::Exit {
            status: exit_status.unwrap_or_else(|| "terminal exited".to_string()),
        },
    )
    .await;
}

async fn send_socket_error(mut socket: WebSocket, message: String) {
    let _ = socket
        .send(Message::Text(
            serde_json::to_string(&ServerTerminalEvent::Error { message })
                .unwrap_or_else(|_| r#"{"type":"error","message":"terminal error"}"#.to_string())
                .into(),
        ))
        .await;
}

pub(crate) fn shell_path(path: &Path) -> PathBuf {
    let value = path.display().to_string();

    if let Some(stripped) = value.strip_prefix("\\\\?\\UNC\\") {
        return PathBuf::from(format!("\\\\{stripped}"));
    }

    if let Some(stripped) = value.strip_prefix("\\\\?\\") {
        return PathBuf::from(stripped);
    }

    path.to_path_buf()
}

async fn send_terminal_event(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    event: &ServerTerminalEvent<'_>,
) -> Result<(), axum::Error> {
    let payload = serde_json::to_string(event).map_err(axum::Error::new)?;

    sender.send(Message::Text(payload.into())).await
}

fn spawn_terminal(
    cwd: &Path,
    terminal_shell: &str,
    cols: u16,
    rows: u16,
) -> Result<SpawnedTerminal, String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: normalized_terminal_size(rows, DEFAULT_ROWS),
            cols: normalized_terminal_size(cols, DEFAULT_COLS),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|source| format!("failed to create Windows PTY: {source}"))?;
    let mut command = terminal_command(terminal_shell)?;
    let shell_cwd = shell_path(cwd);

    command.cwd(&shell_cwd);

    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|source| format!("failed to spawn {terminal_shell} terminal shell: {source}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|source| format!("failed to open terminal input writer: {source}"))?;

    Ok(SpawnedTerminal {
        master: pair.master,
        writer,
        child,
    })
}

fn terminal_command(terminal_shell: &str) -> Result<CommandBuilder, String> {
    match terminal_shell {
        "powershell" => {
            let mut command = CommandBuilder::new("powershell.exe");
            command.arg("-NoLogo");
            command.arg("-NoProfile");
            command.arg("-NoExit");
            command.arg("-Command");
            command.arg(powershell_prompt_command());
            Ok(command)
        }
        "cmd" => Ok(CommandBuilder::new("cmd.exe")),
        "bash" => Ok(CommandBuilder::new("bash")),
        "zsh" => Ok(CommandBuilder::new("zsh")),
        _ => Err(format!("unsupported terminal shell: {terminal_shell}")),
    }
}

fn spawn_reader(mut reader: Box<dyn Read + Send>, output_tx: mpsc::UnboundedSender<PtyOutput>) {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    let _ = output_tx.send(PtyOutput::Closed);
                    break;
                }
                Ok(count) => {
                    let data = String::from_utf8_lossy(&buffer[..count]).to_string();
                    if output_tx.send(PtyOutput::Output(data)).is_err() {
                        break;
                    }
                }
                Err(source) => {
                    let _ = output_tx.send(PtyOutput::Error(format!(
                        "failed to read terminal output: {source}"
                    )));
                    break;
                }
            }
        }
    });
}

fn spawn_child_waiter(
    mut child: Box<dyn Child + Send + Sync>,
    output_tx: mpsc::UnboundedSender<PtyOutput>,
) {
    thread::spawn(move || match child.wait() {
        Ok(status) => {
            let _ = output_tx.send(PtyOutput::Exit(format!("{status:?}")));
        }
        Err(source) => {
            let _ = output_tx.send(PtyOutput::Error(format!(
                "failed to wait for terminal process: {source}"
            )));
        }
    });
}

fn resize_terminal(master: &dyn MasterPty, cols: u16, rows: u16) -> Result<(), String> {
    master
        .resize(PtySize {
            rows: normalized_terminal_size(rows, DEFAULT_ROWS),
            cols: normalized_terminal_size(cols, DEFAULT_COLS),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|source| format!("failed to resize terminal: {source}"))
}

fn persist_terminal_cwd(workspace_path: &Path, session_id: &str, cwd: &str) -> Result<(), String> {
    let cwd_path = shell_path(Path::new(cwd));

    if !cwd_path.is_absolute() {
        return Err(format!("terminal reported a non-absolute cwd: {cwd}"));
    }

    if !cwd_path.is_dir() {
        return Err(format!(
            "terminal reported a cwd that is not a directory: {}",
            cwd_path.display()
        ));
    }

    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(|source| format!("failed to open workspace database: {source}"))?;
    database
        .update_terminal_working_directory(session_id, &cwd_path.display().to_string())
        .map_err(|source| source.to_string())
}

fn close_terminal_session(workspace_path: &Path, session_id: &str) -> Result<(), String> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(|source| format!("failed to open workspace database: {source}"))?;
    database
        .close_terminal_session(session_id)
        .map_err(|source| source.to_string())
}

fn powershell_prompt_command() -> String {
    concat!(
        "$esc=[char]27;",
        "$bel=[char]7;",
        "$global:FocoOriginalPrompt=(Get-Command prompt).ScriptBlock;",
        "function global:prompt {",
        "$path=(Get-Location).ProviderPath;",
        "[Console]::Out.Write($esc + ']7;file://foco/' + [uri]::EscapeDataString($path) + $bel);",
        "& $global:FocoOriginalPrompt",
        "}"
    )
    .to_string()
}

fn normalized_terminal_size(value: u16, default_value: u16) -> u16 {
    if value == 0 { default_value } else { value }
}

#[derive(Default)]
struct CwdTracker {
    buffer: String,
    last_cwd: Option<String>,
}

impl CwdTracker {
    fn observe(&mut self, data: &str) -> Option<String> {
        self.buffer.push_str(data);
        if self.buffer.len() > 8192 {
            self.buffer = self
                .buffer
                .chars()
                .rev()
                .take(4096)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
        }

        let cwd = extract_latest_cwd(&self.buffer)?;
        if self.last_cwd.as_deref() == Some(cwd.as_str()) {
            return None;
        }

        self.last_cwd = Some(cwd.clone());
        Some(cwd)
    }
}

fn extract_latest_cwd(input: &str) -> Option<String> {
    let mut search_start = 0;
    let mut latest = None;

    while let Some(relative_start) = input[search_start..].find(OSC7_PREFIX) {
        let value_start = search_start + relative_start + OSC7_PREFIX.len();
        let rest = &input[value_start..];
        let bel_end = rest.find(OSC_BEL);
        let st_end = rest.find(OSC_ST);
        let value_end = match (bel_end, st_end) {
            (Some(bel), Some(st)) => bel.min(st),
            (Some(bel), None) => bel,
            (None, Some(st)) => st,
            (None, None) => break,
        };
        latest = percent_decode(&rest[..value_end]);
        search_start = value_start + value_end;
    }

    latest
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            decoded.push(hex_value(high)? * 16 + hex_value(low)?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
