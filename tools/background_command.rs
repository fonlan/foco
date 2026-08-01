//! In-memory ownership and output retention for managed background commands.
//!
//! This module intentionally stays below the tool JSON layer. Callers own a registry per
//! execution host and use opaque command IDs to inspect or stop processes between tool turns.

use std::{
    collections::{HashMap, VecDeque},
    fmt,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::{Arc, Condvar, Mutex, MutexGuard, Weak},
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime},
};

#[cfg(unix)]
use process_wrap::std::ProcessGroup;
use process_wrap::std::{ChildWrapper, CommandWrap};
#[cfg(windows)]
use process_wrap::std::{CreationFlags, JobObject};
#[cfg(windows)]
use windows::Win32::System::Threading::PROCESS_CREATION_FLAGS;

/// Maximum number of active commands a registry permits for one execution workspace.
pub const MAX_BACKGROUND_COMMANDS_PER_WORKSPACE: usize = 16;
/// Maximum combined stdout and stderr retained for one command.
pub const MAX_BACKGROUND_COMMAND_OUTPUT_BYTES: usize = 1024 * 1024;
/// How long terminal command records remain queryable.
pub const BACKGROUND_COMMAND_RETENTION: Duration = Duration::from_secs(30 * 60);
const OUTPUT_READ_BUFFER_BYTES: usize = 8 * 1024;
// Keep one serialized chunk below the tool response budget even if every byte is JSON-escaped.
const MAX_OUTPUT_CHUNK_BYTES: usize = 4 * 1024;
const MAX_OUTPUT_CHUNK_NEWLINES: usize = 1_999;
const MONITOR_POLL_INTERVAL: Duration = Duration::from_millis(25);
const MONITOR_MAX_POLL_INTERVAL: Duration = Duration::from_millis(250);
const TERMINATION_GRACE_PERIOD: Duration = Duration::from_millis(500);
#[cfg(windows)]
const CREATE_NO_WINDOW: PROCESS_CREATION_FLAGS = PROCESS_CREATION_FLAGS(0x0800_0000);

/// Configurable bounds for a [`BackgroundCommandRegistry`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackgroundCommandLimits {
    pub max_active_per_workspace: usize,
    pub max_output_bytes_per_command: usize,
    pub completed_retention: Duration,
}

impl Default for BackgroundCommandLimits {
    fn default() -> Self {
        Self {
            max_active_per_workspace: MAX_BACKGROUND_COMMANDS_PER_WORKSPACE,
            max_output_bytes_per_command: MAX_BACKGROUND_COMMAND_OUTPUT_BYTES,
            completed_retention: BACKGROUND_COMMAND_RETENTION,
        }
    }
}

/// The lifecycle state of a managed command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundCommandStatus {
    Running,
    Exited,
    Stopped,
    TimedOut,
    Failed,
}

impl BackgroundCommandStatus {
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }
}

/// Why a command stopped before its natural completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundCommandTermination {
    ExplicitStop,
    Timeout,
    HostShutdown,
}

/// The source stream of an output fragment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundCommandOutputStream {
    Stdout,
    Stderr,
}

/// A retained output fragment. Cursors are strictly increasing for one command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundCommandOutputChunk {
    pub cursor: u64,
    pub stream: BackgroundCommandOutputStream,
    pub bytes: Vec<u8>,
}

/// Non-consuming output query result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundCommandOutput {
    pub command_id: String,
    pub chunks: Vec<BackgroundCommandOutputChunk>,
    /// Cursor a caller should supply as `after_cursor` for the next incremental read.
    pub next_cursor: u64,
    /// The first cursor still retained, if output remains in the ring buffer.
    pub earliest_cursor: Option<u64>,
    /// True when the requested cursor predates output discarded from the ring buffer.
    pub cursor_expired: bool,
    /// True after any bytes have been evicted from the command's output ring buffer.
    pub output_truncated: bool,
}

/// Diagnostic ownership and execution details supplied when starting a command.
#[derive(Clone, Debug)]
pub struct BackgroundCommandRequest {
    pub workspace_path: PathBuf,
    pub cwd: PathBuf,
    pub command: String,
    pub args: Vec<String>,
    pub owner_chat_id: Option<String>,
    pub owner_run_id: Option<String>,
    pub timeout: Option<Duration>,
}

/// Immutable and current lifecycle details for one command.
#[derive(Clone, Debug)]
pub struct BackgroundCommandSnapshot {
    pub command_id: String,
    pub pid: u32,
    pub workspace_path: PathBuf,
    pub cwd: PathBuf,
    pub command: String,
    pub args: Vec<String>,
    pub owner_chat_id: Option<String>,
    pub owner_run_id: Option<String>,
    pub started_at: SystemTime,
    pub ended_at: Option<SystemTime>,
    pub status: BackgroundCommandStatus,
    pub exit_code: Option<i32>,
    pub termination: Option<BackgroundCommandTermination>,
    pub error: Option<String>,
    pub retained_output_bytes: usize,
    pub dropped_output_bytes: usize,
}

/// Errors returned by the managed-command runtime.
#[derive(Debug)]
pub enum BackgroundCommandError {
    CommandNotFound(String),
    InvalidWorkspace {
        workspace_path: PathBuf,
        cwd: PathBuf,
    },
    WorkspaceProcessLimit {
        workspace_path: PathBuf,
        max_active: usize,
    },
    Spawn {
        command: String,
        source: io::Error,
    },
    MonitorInitialization {
        source: io::Error,
    },
    CursorExhausted,
    WaitTimedOut {
        command_id: String,
        wait: Duration,
    },
    WaitCancelled {
        command_id: String,
    },
}

impl fmt::Display for BackgroundCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandNotFound(command_id) => {
                write!(formatter, "managed command not found: {command_id}")
            }
            Self::InvalidWorkspace {
                workspace_path,
                cwd,
            } => write!(
                formatter,
                "command cwd '{}' is not inside execution workspace '{}'",
                cwd.display(),
                workspace_path.display()
            ),
            Self::WorkspaceProcessLimit {
                workspace_path,
                max_active,
            } => write!(
                formatter,
                "execution workspace '{}' already has {max_active} active managed commands",
                workspace_path.display()
            ),
            Self::Spawn { command, source } => {
                write!(
                    formatter,
                    "failed to spawn managed command '{command}': {source}"
                )
            }
            Self::MonitorInitialization { source } => {
                write!(
                    formatter,
                    "failed to initialize managed command monitor: {source}"
                )
            }
            Self::CursorExhausted => write!(formatter, "managed command output cursor exhausted"),
            Self::WaitTimedOut { command_id, wait } => write!(
                formatter,
                "managed command stop timed out after {} ms: {command_id}",
                wait.as_millis()
            ),
            Self::WaitCancelled { command_id } => {
                write!(formatter, "managed command stop cancelled: {command_id}")
            }
        }
    }
}

impl std::error::Error for BackgroundCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source, .. } | Self::MonitorInitialization { source } => Some(source),
            _ => None,
        }
    }
}

/// Cloneable, host-owned registry for managed background commands.
#[derive(Clone)]
pub struct BackgroundCommandRegistry {
    inner: Arc<BackgroundCommandRegistryInner>,
}

struct BackgroundCommandRegistryInner {
    limits: BackgroundCommandLimits,
    state: Mutex<RegistryState>,
}

#[derive(Default)]
struct RegistryState {
    entries: HashMap<String, Arc<BackgroundCommandEntry>>,
    pending_starts_by_workspace: HashMap<PathBuf, usize>,
    next_command_sequence: u64,
}

struct BackgroundCommandEntry {
    command_id: String,
    pid: u32,
    workspace_path: PathBuf,
    cwd: PathBuf,
    command: String,
    args: Vec<String>,
    owner_chat_id: Option<String>,
    owner_run_id: Option<String>,
    started_at: SystemTime,
    started_monotonic: Instant,
    timeout: Option<Duration>,
    child: Mutex<Box<dyn ChildWrapper>>,
    monitor_wake: Condvar,
    monitor_wake_state: Mutex<bool>,
    state: Mutex<BackgroundCommandEntryState>,
    output_limit: usize,
}

struct BackgroundCommandEntryState {
    status: BackgroundCommandStatus,
    ended_at: Option<SystemTime>,
    ended_monotonic: Option<Instant>,
    exit_code: Option<i32>,
    termination: Option<BackgroundCommandTermination>,
    error: Option<String>,
    requested_termination: Option<RequestedTermination>,
    output: OutputRingBuffer,
}

struct RequestedTermination {
    reason: BackgroundCommandTermination,
    requested_at: Instant,
    graceful_signal_sent: bool,
    force_kill_sent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TerminationAction {
    Graceful,
    Force,
}

struct OutputRingBuffer {
    chunks: VecDeque<BackgroundCommandOutputChunk>,
    retained_bytes: usize,
    dropped_bytes: usize,
    next_cursor: u64,
}

impl OutputRingBuffer {
    fn new() -> Self {
        Self {
            chunks: VecDeque::new(),
            retained_bytes: 0,
            dropped_bytes: 0,
            next_cursor: 0,
        }
    }

    fn append(
        &mut self,
        stream: BackgroundCommandOutputStream,
        bytes: &[u8],
        max_bytes: usize,
    ) -> Result<(), BackgroundCommandError> {
        if bytes.is_empty() {
            return Ok(());
        }

        let retained_slice = if bytes.len() > max_bytes {
            self.dropped_bytes = self.dropped_bytes.saturating_add(bytes.len() - max_bytes);
            &bytes[bytes.len() - max_bytes..]
        } else {
            bytes
        };

        let mut remaining = retained_slice;
        while !remaining.is_empty() {
            let chunk_end = output_chunk_end(remaining);
            let chunk_bytes = &remaining[..chunk_end];
            while self.retained_bytes.saturating_add(chunk_bytes.len()) > max_bytes {
                let Some(evicted) = self.chunks.pop_front() else {
                    break;
                };
                self.retained_bytes = self.retained_bytes.saturating_sub(evicted.bytes.len());
                self.dropped_bytes = self.dropped_bytes.saturating_add(evicted.bytes.len());
            }

            self.next_cursor = self
                .next_cursor
                .checked_add(1)
                .ok_or(BackgroundCommandError::CursorExhausted)?;
            self.retained_bytes = self.retained_bytes.saturating_add(chunk_bytes.len());
            self.chunks.push_back(BackgroundCommandOutputChunk {
                cursor: self.next_cursor,
                stream,
                bytes: chunk_bytes.to_vec(),
            });
            remaining = &remaining[chunk_end..];
        }
        Ok(())
    }

    fn read_after(&self, command_id: &str, after_cursor: Option<u64>) -> BackgroundCommandOutput {
        let earliest_cursor = self.chunks.front().map(|chunk| chunk.cursor);
        let cursor_expired = after_cursor.is_some_and(|cursor| {
            self.dropped_bytes > 0
                && earliest_cursor.is_some_and(|earliest| cursor < earliest.saturating_sub(1))
        });
        let chunks = self
            .chunks
            .iter()
            .filter(|chunk| after_cursor.is_none_or(|cursor| chunk.cursor > cursor))
            .cloned()
            .collect();

        BackgroundCommandOutput {
            command_id: command_id.to_string(),
            chunks,
            next_cursor: self.next_cursor,
            earliest_cursor,
            cursor_expired,
            output_truncated: self.dropped_bytes > 0,
        }
    }
}

fn output_chunk_end(bytes: &[u8]) -> usize {
    let byte_limit = bytes.len().min(MAX_OUTPUT_CHUNK_BYTES);
    let mut newlines = 0usize;
    for (index, byte) in bytes.iter().take(byte_limit).enumerate() {
        if *byte == b'\n' {
            newlines = newlines.saturating_add(1);
            if newlines > MAX_OUTPUT_CHUNK_NEWLINES {
                return index;
            }
        }
    }
    byte_limit
}

impl BackgroundCommandRegistry {
    pub fn new() -> Self {
        Self::with_limits(BackgroundCommandLimits::default())
    }

    pub fn with_limits(limits: BackgroundCommandLimits) -> Self {
        Self {
            inner: Arc::new(BackgroundCommandRegistryInner {
                limits: BackgroundCommandLimits {
                    max_active_per_workspace: limits.max_active_per_workspace.max(1),
                    max_output_bytes_per_command: limits.max_output_bytes_per_command.max(1),
                    completed_retention: limits.completed_retention,
                },
                state: Mutex::new(RegistryState::default()),
            }),
        }
    }

    /// Starts a command and immediately returns its durable in-memory handle.
    pub fn start(
        &self,
        request: BackgroundCommandRequest,
    ) -> Result<BackgroundCommandSnapshot, BackgroundCommandError> {
        let (workspace_path, cwd) =
            normalize_execution_paths(&request.workspace_path, &request.cwd)?;
        let command_label = command_label(&request.command, &request.args);
        let command_id = self.reserve_command_slot(&workspace_path)?;

        let mut child = match spawn_managed_child(&request.command, &request.args, &cwd) {
            Ok(child) => child,
            Err(source) => {
                self.release_command_slot(&workspace_path);
                return Err(BackgroundCommandError::Spawn {
                    command: command_label,
                    source,
                });
            }
        };
        let pid = child.id();
        let stdout = match child.stdout().take() {
            Some(stdout) => stdout,
            None => {
                cleanup_unregistered_child(&mut child);
                self.release_command_slot(&workspace_path);
                return Err(BackgroundCommandError::MonitorInitialization {
                    source: io::Error::other("failed to capture managed command stdout"),
                });
            }
        };
        let stderr = match child.stderr().take() {
            Some(stderr) => stderr,
            None => {
                cleanup_unregistered_child(&mut child);
                self.release_command_slot(&workspace_path);
                return Err(BackgroundCommandError::MonitorInitialization {
                    source: io::Error::other("failed to capture managed command stderr"),
                });
            }
        };

        let entry = Arc::new(BackgroundCommandEntry {
            command_id: command_id.clone(),
            pid,
            workspace_path: workspace_path.clone(),
            cwd,
            command: request.command,
            args: request.args,
            owner_chat_id: request.owner_chat_id,
            owner_run_id: request.owner_run_id,
            started_at: SystemTime::now(),
            started_monotonic: Instant::now(),
            timeout: request.timeout,
            child: Mutex::new(child),
            monitor_wake: Condvar::new(),
            monitor_wake_state: Mutex::new(false),
            state: Mutex::new(BackgroundCommandEntryState {
                status: BackgroundCommandStatus::Running,
                ended_at: None,
                ended_monotonic: None,
                exit_code: None,
                termination: None,
                error: None,
                requested_termination: None,
                output: OutputRingBuffer::new(),
            }),
            output_limit: self.inner.limits.max_output_bytes_per_command,
        });

        let stdout_reader =
            match spawn_output_reader(entry.clone(), BackgroundCommandOutputStream::Stdout, stdout)
            {
                Ok(reader) => reader,
                Err(source) => {
                    entry.force_terminate();
                    self.release_command_slot(&workspace_path);
                    return Err(BackgroundCommandError::MonitorInitialization { source });
                }
            };
        let stderr_reader =
            match spawn_output_reader(entry.clone(), BackgroundCommandOutputStream::Stderr, stderr)
            {
                Ok(reader) => reader,
                Err(source) => {
                    entry.force_terminate();
                    let _ = stdout_reader.join();
                    self.release_command_slot(&workspace_path);
                    return Err(BackgroundCommandError::MonitorInitialization { source });
                }
            };

        let registry = Arc::downgrade(&self.inner);
        let monitor_entry = entry.clone();
        let monitor = thread::Builder::new()
            .name(format!("foco-command-monitor-{pid}"))
            .spawn(move || {
                monitor_background_command(monitor_entry, registry, stdout_reader, stderr_reader)
            });
        if let Err(source) = monitor {
            entry.force_terminate();
            self.release_command_slot(&workspace_path);
            return Err(BackgroundCommandError::MonitorInitialization { source });
        }

        self.publish_command(command_id, entry.clone(), &workspace_path);
        Ok(entry.snapshot())
    }

    pub fn command(
        &self,
        command_id: &str,
    ) -> Result<BackgroundCommandSnapshot, BackgroundCommandError> {
        self.prune_completed();
        let entry = self.entry(command_id)?;
        Ok(entry.snapshot())
    }

    /// Reads output without consuming it; retries with the same cursor are idempotent.
    pub fn output_after(
        &self,
        command_id: &str,
        after_cursor: Option<u64>,
    ) -> Result<BackgroundCommandOutput, BackgroundCommandError> {
        self.prune_completed();
        let entry = self.entry(command_id)?;
        Ok(entry.output_after(after_cursor))
    }

    /// Requests graceful process-tree termination. The returned snapshot may still be running
    /// briefly while the monitor observes the exit and drains its pipes.
    pub fn stop(
        &self,
        command_id: &str,
    ) -> Result<BackgroundCommandSnapshot, BackgroundCommandError> {
        let entry = self.entry(command_id)?;
        entry.request_termination(BackgroundCommandTermination::ExplicitStop);
        Ok(entry.snapshot())
    }

    /// Requests managed termination and waits for the monitor to record a terminal snapshot.
    ///
    /// A successful return guarantees that the process tree has exited and both captured output
    /// pipes have been drained. Concurrent callers share one termination request but each waits
    /// independently for the same terminal state.
    pub fn stop_and_wait(
        &self,
        command_id: &str,
        wait: Duration,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<BackgroundCommandSnapshot, BackgroundCommandError> {
        let entry = self.entry(command_id)?;
        entry.request_termination(BackgroundCommandTermination::ExplicitStop);
        wait_for_entry_terminal(&entry, wait, is_cancelled)
    }

    /// Requests termination for every command owned by a workspace.
    ///
    /// The returned count is how many running commands newly received a stop request.
    /// Snapshots may still report `Running` briefly while the monitor drains pipes.
    pub fn stop_for_workspace(
        &self,
        workspace_path: &Path,
    ) -> Result<usize, BackgroundCommandError> {
        let workspace_path = workspace_owner_path(workspace_path)?;
        Ok(self.stop_matching(|entry| entry.workspace_path == workspace_path))
    }

    /// Stops every command owned by a workspace and waits until each reaches a terminal status.
    ///
    /// Use this before deleting an execution workspace directory so managed process trees are
    /// not still writing into a path that is about to be removed. After the wait budget elapses,
    /// remaining processes are force-killed and given a short additional window to finish.
    pub fn stop_and_wait_for_workspace(
        &self,
        workspace_path: &Path,
        wait: Duration,
    ) -> Result<usize, BackgroundCommandError> {
        let workspace_path = workspace_owner_path(workspace_path)?;
        let entries = {
            let state = lock_recover(&self.inner.state);
            state
                .entries
                .values()
                .filter(|entry| entry.workspace_path == workspace_path)
                .cloned()
                .collect::<Vec<_>>()
        };
        let mut requested = 0usize;
        for entry in &entries {
            if entry.request_termination(BackgroundCommandTermination::ExplicitStop) {
                requested += 1;
            }
        }

        let deadline = Instant::now() + wait;
        for entry in &entries {
            wait_entry_terminal_or_force(entry, deadline);
        }
        Ok(requested)
    }

    /// Backwards-compatible alias for [`Self::stop_for_workspace`].
    pub fn stop_workspace(&self, workspace_path: &Path) -> Result<usize, BackgroundCommandError> {
        self.stop_for_workspace(workspace_path)
    }

    /// Requests termination for every command owned by a chat, across its execution workspaces.
    pub fn stop_for_chat(&self, chat_id: &str) -> usize {
        self.stop_matching(|entry| entry.owner_chat_id.as_deref() == Some(chat_id))
    }

    /// Requests termination for every command owned by one chat in one workspace.
    ///
    /// Hosts with a shared registry must use this scoped variant because chat IDs are only
    /// guaranteed to be meaningful within their workspace.
    pub fn stop_for_workspace_chat(
        &self,
        workspace_path: &Path,
        chat_id: &str,
    ) -> Result<usize, BackgroundCommandError> {
        let workspace_path = workspace_owner_path(workspace_path)?;
        Ok(self.stop_matching(|entry| {
            entry.workspace_path == workspace_path
                && entry.owner_chat_id.as_deref() == Some(chat_id)
        }))
    }

    /// Force-terminates all active process trees. Hosts should call this during orderly shutdown.
    pub fn shutdown_all(&self) {
        let entries = {
            let state = lock_recover(&self.inner.state);
            state.entries.values().cloned().collect::<Vec<_>>()
        };
        for entry in entries {
            entry.request_termination(BackgroundCommandTermination::HostShutdown);
            if entry.snapshot().status == BackgroundCommandStatus::Running {
                entry.force_terminate();
            }
        }
    }

    /// Backwards-compatible alias for [`Self::shutdown_all`].
    pub fn shutdown(&self) {
        self.shutdown_all();
    }

    pub fn prune_completed(&self) {
        let mut state = lock_recover(&self.inner.state);
        prune_completed_entries(&mut state, self.inner.limits.completed_retention);
    }

    fn stop_matching(&self, predicate: impl Fn(&BackgroundCommandEntry) -> bool) -> usize {
        let entries = {
            let state = lock_recover(&self.inner.state);
            state
                .entries
                .values()
                .filter(|entry| predicate(entry))
                .cloned()
                .collect::<Vec<_>>()
        };
        entries
            .iter()
            .filter(|entry| entry.request_termination(BackgroundCommandTermination::ExplicitStop))
            .count()
    }

    fn entry(
        &self,
        command_id: &str,
    ) -> Result<Arc<BackgroundCommandEntry>, BackgroundCommandError> {
        let state = lock_recover(&self.inner.state);
        state
            .entries
            .get(command_id)
            .cloned()
            .ok_or_else(|| BackgroundCommandError::CommandNotFound(command_id.to_string()))
    }

    fn reserve_command_slot(
        &self,
        workspace_path: &Path,
    ) -> Result<String, BackgroundCommandError> {
        let mut state = lock_recover(&self.inner.state);
        prune_completed_entries(&mut state, self.inner.limits.completed_retention);
        let active = state
            .entries
            .values()
            .filter(|entry| {
                entry.workspace_path == workspace_path
                    && entry.snapshot().status == BackgroundCommandStatus::Running
            })
            .count();
        let pending = state
            .pending_starts_by_workspace
            .get(workspace_path)
            .copied()
            .unwrap_or_default();
        if active.saturating_add(pending) >= self.inner.limits.max_active_per_workspace {
            return Err(BackgroundCommandError::WorkspaceProcessLimit {
                workspace_path: workspace_path.to_path_buf(),
                max_active: self.inner.limits.max_active_per_workspace,
            });
        }

        state.next_command_sequence = state.next_command_sequence.saturating_add(1);
        let command_id = format!("command-{:016x}", state.next_command_sequence);
        *state
            .pending_starts_by_workspace
            .entry(workspace_path.to_path_buf())
            .or_default() += 1;
        Ok(command_id)
    }

    fn release_command_slot(&self, workspace_path: &Path) {
        let mut state = lock_recover(&self.inner.state);
        release_pending_slot(&mut state, workspace_path);
    }

    fn publish_command(
        &self,
        command_id: String,
        entry: Arc<BackgroundCommandEntry>,
        workspace_path: &Path,
    ) {
        let mut state = lock_recover(&self.inner.state);
        release_pending_slot(&mut state, workspace_path);
        state.entries.insert(command_id, entry);
    }
}

impl Default for BackgroundCommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BackgroundCommandRegistryInner {
    fn drop(&mut self) {
        let entries = {
            let state = lock_recover(&self.state);
            state.entries.values().cloned().collect::<Vec<_>>()
        };
        for entry in entries {
            entry.request_termination(BackgroundCommandTermination::HostShutdown);
            entry.force_terminate();
        }
    }
}

impl BackgroundCommandEntry {
    fn snapshot(&self) -> BackgroundCommandSnapshot {
        let state = lock_recover(&self.state);
        self.snapshot_from_state(&state)
    }

    fn terminal_snapshot_before(&self, deadline: Instant) -> Option<BackgroundCommandSnapshot> {
        let state = lock_recover(&self.state);
        if state.status.is_terminal()
            && state
                .ended_monotonic
                .is_some_and(|ended_at| ended_at <= deadline)
        {
            Some(self.snapshot_from_state(&state))
        } else {
            None
        }
    }

    fn snapshot_from_state(
        &self,
        state: &BackgroundCommandEntryState,
    ) -> BackgroundCommandSnapshot {
        BackgroundCommandSnapshot {
            command_id: self.command_id.clone(),
            pid: self.pid,
            workspace_path: self.workspace_path.clone(),
            cwd: self.cwd.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            owner_chat_id: self.owner_chat_id.clone(),
            owner_run_id: self.owner_run_id.clone(),
            started_at: self.started_at,
            ended_at: state.ended_at,
            status: state.status,
            exit_code: state.exit_code,
            termination: state.termination,
            error: state.error.clone(),
            retained_output_bytes: state.output.retained_bytes,
            dropped_output_bytes: state.output.dropped_bytes,
        }
    }

    fn output_after(&self, after_cursor: Option<u64>) -> BackgroundCommandOutput {
        let state = lock_recover(&self.state);
        state.output.read_after(&self.command_id, after_cursor)
    }

    fn append_output(
        &self,
        stream: BackgroundCommandOutputStream,
        bytes: &[u8],
    ) -> Result<(), BackgroundCommandError> {
        let mut state = lock_recover(&self.state);
        state.output.append(stream, bytes, self.output_limit)?;
        self.notify_monitor();
        Ok(())
    }

    fn request_termination(&self, reason: BackgroundCommandTermination) -> bool {
        let mut state = lock_recover(&self.state);
        if state.status.is_terminal() || state.requested_termination.is_some() {
            return false;
        }
        state.requested_termination = Some(RequestedTermination {
            reason,
            requested_at: Instant::now(),
            graceful_signal_sent: false,
            force_kill_sent: false,
        });
        drop(state);
        self.notify_monitor();
        true
    }

    fn termination_action(&self, now: Instant) -> Option<TerminationAction> {
        let mut state = lock_recover(&self.state);
        let requested = state.requested_termination.as_mut()?;
        if !requested.graceful_signal_sent {
            requested.graceful_signal_sent = true;
            return Some(TerminationAction::Graceful);
        }
        if !requested.force_kill_sent
            && now.saturating_duration_since(requested.requested_at) >= TERMINATION_GRACE_PERIOD
        {
            requested.force_kill_sent = true;
            return Some(TerminationAction::Force);
        }
        None
    }

    fn termination_reason(&self) -> Option<BackgroundCommandTermination> {
        let state = lock_recover(&self.state);
        state
            .requested_termination
            .as_ref()
            .map(|request| request.reason)
    }

    fn check_timeout(&self, now: Instant) {
        if self
            .timeout
            .is_some_and(|timeout| now.saturating_duration_since(self.started_monotonic) >= timeout)
        {
            self.request_termination(BackgroundCommandTermination::Timeout);
        }
    }

    fn try_wait(&self) -> io::Result<Option<ExitStatus>> {
        let mut child = lock_recover(&self.child);
        child.try_wait()
    }

    fn send_termination(&self, action: TerminationAction) -> io::Result<()> {
        let mut child = lock_recover(&self.child);
        match action {
            #[cfg(unix)]
            TerminationAction::Graceful => child.signal(15),
            #[cfg(not(unix))]
            TerminationAction::Graceful => child.start_kill(),
            TerminationAction::Force => child.start_kill(),
        }
    }

    fn force_terminate(&self) {
        let mut child = lock_recover(&self.child);
        let _ = child.start_kill();
        self.notify_monitor();
    }

    fn notify_monitor(&self) {
        let mut signalled = lock_recover(&self.monitor_wake_state);
        *signalled = true;
        self.monitor_wake.notify_one();
    }

    fn wait_for_monitor_signal(&self, wait: Duration) -> bool {
        let mut signalled = lock_recover(&self.monitor_wake_state);
        if *signalled {
            *signalled = false;
            return true;
        }
        let (mut signalled, _) = self
            .monitor_wake
            .wait_timeout(signalled, wait)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let was_signalled = *signalled;
        *signalled = false;
        was_signalled
    }

    fn finish(
        &self,
        status: BackgroundCommandStatus,
        exit_status: Option<ExitStatus>,
        error: Option<String>,
    ) {
        let mut state = lock_recover(&self.state);
        if state.status.is_terminal() {
            return;
        }
        state.status = status;
        state.ended_at = Some(SystemTime::now());
        state.ended_monotonic = Some(Instant::now());
        state.exit_code = exit_status.and_then(|status| status.code());
        state.termination = state
            .requested_termination
            .as_ref()
            .map(|request| request.reason);
        state.error = error;
    }
}

fn normalize_execution_paths(
    workspace_path: &Path,
    cwd: &Path,
) -> Result<(PathBuf, PathBuf), BackgroundCommandError> {
    let workspace_path =
        std::fs::canonicalize(workspace_path).map_err(|source| BackgroundCommandError::Spawn {
            command: "canonicalize execution workspace".to_string(),
            source,
        })?;
    let cwd = std::fs::canonicalize(cwd).map_err(|source| BackgroundCommandError::Spawn {
        command: "canonicalize command cwd".to_string(),
        source,
    })?;
    if !cwd.starts_with(&workspace_path) {
        return Err(BackgroundCommandError::InvalidWorkspace {
            workspace_path,
            cwd,
        });
    }
    Ok((workspace_path, cwd))
}

fn workspace_owner_path(workspace_path: &Path) -> Result<PathBuf, BackgroundCommandError> {
    match std::fs::canonicalize(workspace_path) {
        Ok(path) => Ok(path),
        // Workspace configuration stores canonical local paths. If the directory was removed
        // before host cleanup runs, preserve that stored path so existing entries still match.
        Err(source)
            if matches!(
                source.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(workspace_path.to_path_buf())
        }
        Err(source) => Err(BackgroundCommandError::Spawn {
            command: "canonicalize execution workspace".to_string(),
            source,
        }),
    }
}

fn spawn_managed_child(
    command: &str,
    args: &[String],
    cwd: &Path,
) -> io::Result<Box<dyn ChildWrapper>> {
    let mut command_process = std::process::Command::new(command);
    command_process
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut command = CommandWrap::from(command_process);

    #[cfg(unix)]
    command.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    {
        command.wrap(CreationFlags(CREATE_NO_WINDOW));
        command.wrap(JobObject);
    }

    command.spawn()
}

fn cleanup_unregistered_child(child: &mut Box<dyn ChildWrapper>) {
    let _ = child.start_kill();
    let _ = child.wait();
}

fn spawn_output_reader<T>(
    entry: Arc<BackgroundCommandEntry>,
    stream: BackgroundCommandOutputStream,
    mut pipe: T,
) -> io::Result<JoinHandle<Result<(), BackgroundCommandError>>>
where
    T: Read + Send + 'static,
{
    thread::Builder::new()
        .name(format!("foco-command-output-{}", entry.pid))
        .spawn(move || {
            let mut buffer = [0_u8; OUTPUT_READ_BUFFER_BYTES];
            loop {
                let read =
                    pipe.read(&mut buffer)
                        .map_err(|source| BackgroundCommandError::Spawn {
                            command: format!("read managed command {} output", entry.command_id),
                            source,
                        })?;
                if read == 0 {
                    return Ok(());
                }
                entry.append_output(stream, &buffer[..read])?;
            }
        })
}

/// Waits until `entry` is terminal. If `deadline` elapses first, force-kills the process tree
/// and waits a short additional window for the monitor to record the terminal status.
fn wait_entry_terminal_or_force(entry: &BackgroundCommandEntry, deadline: Instant) {
    loop {
        if entry.snapshot().status.is_terminal() {
            return;
        }
        if Instant::now() >= deadline {
            entry.force_terminate();
            let force_deadline = Instant::now() + Duration::from_secs(2);
            while !entry.snapshot().status.is_terminal() && Instant::now() < force_deadline {
                thread::sleep(MONITOR_POLL_INTERVAL);
            }
            return;
        }
        thread::sleep(MONITOR_POLL_INTERVAL);
    }
}

fn wait_for_entry_terminal(
    entry: &BackgroundCommandEntry,
    wait: Duration,
    is_cancelled: impl Fn() -> bool,
) -> Result<BackgroundCommandSnapshot, BackgroundCommandError> {
    let deadline = Instant::now() + wait;
    loop {
        if is_cancelled() {
            return Err(BackgroundCommandError::WaitCancelled {
                command_id: entry.command_id.clone(),
            });
        }
        if let Some(snapshot) = entry.terminal_snapshot_before(deadline) {
            return Ok(snapshot);
        }

        let now = Instant::now();
        if now >= deadline {
            return Err(BackgroundCommandError::WaitTimedOut {
                command_id: entry.command_id.clone(),
                wait,
            });
        }
        thread::sleep(MONITOR_POLL_INTERVAL.min(deadline.saturating_duration_since(now)));
    }
}

fn monitor_background_command(
    entry: Arc<BackgroundCommandEntry>,
    registry: Weak<BackgroundCommandRegistryInner>,
    stdout_reader: JoinHandle<Result<(), BackgroundCommandError>>,
    stderr_reader: JoinHandle<Result<(), BackgroundCommandError>>,
) {
    let mut poll_interval = MONITOR_POLL_INTERVAL;
    let exit_status = loop {
        let now = Instant::now();
        entry.check_timeout(now);
        if let Some(action) = entry.termination_action(now) {
            let _ = entry.send_termination(action);
        }

        match entry.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if entry.wait_for_monitor_signal(poll_interval) {
                    poll_interval = MONITOR_POLL_INTERVAL;
                } else {
                    poll_interval = (poll_interval * 2).min(MONITOR_MAX_POLL_INTERVAL);
                }
            }
            Err(source) => {
                entry.force_terminate();
                entry.finish(
                    BackgroundCommandStatus::Failed,
                    None,
                    Some(format!("failed to wait for managed command: {source}")),
                );
                break None;
            }
        }
    };

    let stdout_error = join_output_reader(stdout_reader);
    let stderr_error = join_output_reader(stderr_reader);
    let reader_error = stdout_error.or(stderr_error);
    if let Some(error) = reader_error {
        entry.finish(BackgroundCommandStatus::Failed, exit_status, Some(error));
    } else if let Some(reason) = entry.termination_reason() {
        let status = match reason {
            BackgroundCommandTermination::Timeout => BackgroundCommandStatus::TimedOut,
            BackgroundCommandTermination::ExplicitStop
            | BackgroundCommandTermination::HostShutdown => BackgroundCommandStatus::Stopped,
        };
        entry.finish(status, exit_status, None);
    } else {
        entry.finish(BackgroundCommandStatus::Exited, exit_status, None);
    }

    if let Some(registry) = registry.upgrade() {
        let mut state = lock_recover(&registry.state);
        prune_completed_entries(&mut state, registry.limits.completed_retention);
    }
}

fn join_output_reader(handle: JoinHandle<Result<(), BackgroundCommandError>>) -> Option<String> {
    match handle.join() {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error.to_string()),
        Err(_) => Some("managed command output reader panicked".to_string()),
    }
}

fn release_pending_slot(state: &mut RegistryState, workspace_path: &Path) {
    let should_remove =
        if let Some(pending) = state.pending_starts_by_workspace.get_mut(workspace_path) {
            *pending = pending.saturating_sub(1);
            *pending == 0
        } else {
            false
        };
    if should_remove {
        state.pending_starts_by_workspace.remove(workspace_path);
    }
}

fn prune_completed_entries(state: &mut RegistryState, retention: Duration) {
    let now = Instant::now();
    state.entries.retain(|_, entry| {
        let snapshot = entry.snapshot();
        if !snapshot.status.is_terminal() {
            return true;
        }
        let entry_state = lock_recover(&entry.state);
        entry_state
            .ended_monotonic
            .is_none_or(|ended| now.saturating_duration_since(ended) < retention)
    });
}

fn command_label(command: &str, args: &[String]) -> String {
    if args.is_empty() {
        command.to_string()
    } else {
        format!("{command} {}", args.join(" "))
    }
}

fn lock_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_ring_reads_are_idempotent_and_preserve_stream_identity() {
        let mut output = OutputRingBuffer::new();
        output
            .append(BackgroundCommandOutputStream::Stdout, b"one", 16)
            .expect("append stdout");
        output
            .append(BackgroundCommandOutputStream::Stderr, b"two", 16)
            .expect("append stderr");

        let first = output.read_after("command-1", None);
        let replay = output.read_after("command-1", None);

        assert_eq!(first, replay);
        assert_eq!(
            first
                .chunks
                .iter()
                .map(|chunk| chunk.stream)
                .collect::<Vec<_>>(),
            vec![
                BackgroundCommandOutputStream::Stdout,
                BackgroundCommandOutputStream::Stderr,
            ]
        );
    }

    #[test]
    fn output_ring_marks_expired_cursor_after_evicting_old_output() {
        let mut output = OutputRingBuffer::new();
        output
            .append(BackgroundCommandOutputStream::Stdout, b"abcdef", 6)
            .expect("append first output");
        output
            .append(BackgroundCommandOutputStream::Stdout, b"ghijkl", 6)
            .expect("append second output");

        let result = output.read_after("command-1", Some(0));

        assert!(result.cursor_expired);
    }

    #[cfg(unix)]
    #[test]
    fn registry_reports_an_immediately_exited_command_as_terminal() {
        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::new();
        let command = registry
            .start(background_request(workspace.path(), "exit 0"))
            .expect("start command");

        let terminal = wait_for_terminal(&registry, &command.command_id);

        assert_eq!(terminal.status, BackgroundCommandStatus::Exited);
        assert_eq!(terminal.exit_code, Some(0));
    }

    #[cfg(unix)]
    #[test]
    fn registry_stops_the_entire_unix_process_group() {
        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::new();
        let command = registry
            .start(BackgroundCommandRequest {
                workspace_path: workspace.path().to_path_buf(),
                cwd: workspace.path().to_path_buf(),
                command: "sh".to_string(),
                args: vec!["-c".to_string(), "sleep 30 & echo $!; wait".to_string()],
                owner_chat_id: None,
                owner_run_id: None,
                timeout: None,
            })
            .expect("start command");

        let child_pid = wait_for_output_pid(&registry, &command.command_id);
        registry.stop(&command.command_id).expect("request stop");
        let terminal = wait_for_terminal(&registry, &command.command_id);
        let alive = std::process::Command::new("kill")
            .args(["-0", &child_pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .expect("check process");

        assert_eq!(terminal.status, BackgroundCommandStatus::Stopped);
        assert!(
            !alive.success(),
            "child process should be killed with its process group"
        );
    }

    #[cfg(unix)]
    #[test]
    fn registry_stop_and_wait_returns_a_terminal_idempotent_snapshot() {
        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::new();
        let command = registry
            .start(background_request(workspace.path(), "sleep 30"))
            .expect("start command");

        let first = registry
            .stop_and_wait(&command.command_id, Duration::from_secs(3), || false)
            .expect("stop and wait");
        let second = registry
            .stop_and_wait(&command.command_id, Duration::from_secs(3), || false)
            .expect("idempotent stop and wait");

        assert_eq!(first.status, BackgroundCommandStatus::Stopped);
        assert_eq!(
            first.termination,
            Some(BackgroundCommandTermination::ExplicitStop)
        );
        assert!(first.ended_at.is_some());
        assert_eq!(second.status, BackgroundCommandStatus::Stopped);
    }

    #[cfg(unix)]
    #[test]
    fn registry_stop_and_wait_returns_timeout_instead_of_a_running_snapshot() {
        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::new();
        let command = registry
            .start(background_request(workspace.path(), "sleep 30"))
            .expect("start command");

        let error = registry
            .stop_and_wait(&command.command_id, Duration::from_millis(1), || false)
            .expect_err("short wait must time out before the monitor observes termination");
        let terminal = wait_for_terminal(&registry, &command.command_id);

        assert!(matches!(error, BackgroundCommandError::WaitTimedOut { .. }));
        assert_eq!(terminal.status, BackgroundCommandStatus::Stopped);
    }

    #[cfg(unix)]
    #[test]
    fn registry_stop_and_wait_returns_cancellation_instead_of_a_terminal_snapshot() {
        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::new();
        let command = registry
            .start(background_request(workspace.path(), "sleep 30"))
            .expect("start command");

        let error = registry
            .stop_and_wait(&command.command_id, Duration::from_secs(3), || true)
            .expect_err("cancelled wait must not report a successful snapshot");
        let terminal = wait_for_terminal(&registry, &command.command_id);

        assert!(matches!(
            error,
            BackgroundCommandError::WaitCancelled { .. }
        ));
        assert_eq!(terminal.status, BackgroundCommandStatus::Stopped);
    }

    #[cfg(unix)]
    #[test]
    fn registry_shutdown_stops_the_entire_unix_process_group() {
        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::new();
        let command = registry
            .start(BackgroundCommandRequest {
                workspace_path: workspace.path().to_path_buf(),
                cwd: workspace.path().to_path_buf(),
                command: "sh".to_string(),
                args: vec!["-c".to_string(), "sleep 30 & echo $!; wait".to_string()],
                owner_chat_id: None,
                owner_run_id: None,
                timeout: None,
            })
            .expect("start command");

        let child_pid = wait_for_output_pid(&registry, &command.command_id);
        registry.shutdown_all();
        let terminal = wait_for_terminal(&registry, &command.command_id);
        let alive = std::process::Command::new("kill")
            .args(["-0", &child_pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .expect("check process");

        assert_eq!(terminal.status, BackgroundCommandStatus::Stopped);
        assert_eq!(
            terminal.termination,
            Some(BackgroundCommandTermination::HostShutdown)
        );
        assert!(
            !alive.success(),
            "child process should be killed with its process group during host shutdown"
        );
    }

    #[cfg(unix)]
    #[test]
    fn registry_times_out_commands_without_changing_foreground_execution() {
        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::new();
        let command = registry
            .start(BackgroundCommandRequest {
                workspace_path: workspace.path().to_path_buf(),
                cwd: workspace.path().to_path_buf(),
                command: "sh".to_string(),
                args: vec!["-c".to_string(), "sleep 30".to_string()],
                owner_chat_id: Some("chat-1".to_string()),
                owner_run_id: Some("run-1".to_string()),
                timeout: Some(Duration::from_millis(25)),
            })
            .expect("start command");

        let terminal = wait_for_terminal(&registry, &command.command_id);

        assert_eq!(terminal.status, BackgroundCommandStatus::TimedOut);
    }

    #[cfg(unix)]
    #[test]
    fn registry_enforces_the_per_workspace_active_process_limit() {
        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::with_limits(BackgroundCommandLimits {
            max_active_per_workspace: 1,
            ..BackgroundCommandLimits::default()
        });
        let first = registry
            .start(background_request(workspace.path(), "sleep 30"))
            .expect("start first command");

        let error = registry
            .start(background_request(workspace.path(), "sleep 30"))
            .expect_err("second active command should be rejected");
        registry
            .stop(&first.command_id)
            .expect("stop first command");
        let _ = wait_for_terminal(&registry, &first.command_id);

        assert!(matches!(
            error,
            BackgroundCommandError::WorkspaceProcessLimit { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn registry_enforces_the_process_limit_when_concurrent_starts_race() {
        use std::sync::{Arc, Barrier};

        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::with_limits(BackgroundCommandLimits {
            max_active_per_workspace: 1,
            ..BackgroundCommandLimits::default()
        });
        let barrier = Arc::new(Barrier::new(5));
        let handles = (0..4)
            .map(|_| {
                let registry = registry.clone();
                let workspace_path = workspace.path().to_path_buf();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    registry.start(background_request(&workspace_path, "sleep 30"))
                })
            })
            .collect::<Vec<_>>();

        barrier.wait();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("start task"))
            .collect::<Vec<_>>();
        let started = results
            .iter()
            .filter_map(|result| result.as_ref().ok())
            .collect::<Vec<_>>();

        registry.shutdown_all();
        for command in &started {
            let _ = wait_for_terminal(&registry, &command.command_id);
        }

        assert_eq!(started.len(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| {
                    matches!(
                        result,
                        Err(BackgroundCommandError::WorkspaceProcessLimit { .. })
                    )
                })
                .count(),
            3
        );
    }

    #[cfg(unix)]
    #[test]
    fn registry_allows_concurrent_output_reads_while_stop_is_idempotent() {
        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::new();
        let command = registry
            .start(background_request(
                workspace.path(),
                "while :; do printf x; sleep 0.01; done",
            ))
            .expect("start command");
        let readers = (0..4)
            .map(|_| {
                let registry = registry.clone();
                let command_id = command.command_id.clone();
                thread::spawn(move || {
                    for _ in 0..20 {
                        registry
                            .output_after(&command_id, None)
                            .expect("concurrent output read");
                    }
                })
            })
            .collect::<Vec<_>>();
        let stoppers = (0..2)
            .map(|_| {
                let registry = registry.clone();
                let command_id = command.command_id.clone();
                thread::spawn(move || {
                    registry
                        .stop_and_wait(&command_id, Duration::from_secs(3), || false)
                        .expect("concurrent stop and wait")
                })
            })
            .collect::<Vec<_>>();

        for reader in readers {
            reader.join().expect("output reader task");
        }
        for stopper in stoppers {
            let _ = stopper.join().expect("stop task");
        }
        let terminal = wait_for_terminal(&registry, &command.command_id);

        assert_eq!(terminal.status, BackgroundCommandStatus::Stopped);
        assert_eq!(
            terminal.termination,
            Some(BackgroundCommandTermination::ExplicitStop)
        );
    }

    #[cfg(windows)]
    #[test]
    fn registry_shutdown_stops_the_windows_job_tree() {
        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::new();
        let command = registry
            .start(BackgroundCommandRequest {
                workspace_path: workspace.path().to_path_buf(),
                cwd: workspace.path().to_path_buf(),
                command: "cmd".to_string(),
                args: vec![
                    "/C".to_string(),
                    "start /B cmd /C ping -n 30 127.0.0.1 >NUL & ping -n 30 127.0.0.1 >NUL"
                        .to_string(),
                ],
                owner_chat_id: None,
                owner_run_id: None,
                timeout: None,
            })
            .expect("start command");

        registry.shutdown_all();
        let terminal = wait_for_terminal(&registry, &command.command_id);

        assert_eq!(terminal.status, BackgroundCommandStatus::Stopped);
        assert_eq!(
            terminal.termination,
            Some(BackgroundCommandTermination::HostShutdown)
        );
    }

    #[cfg(unix)]
    #[test]
    fn registry_stops_only_commands_owned_by_the_deleted_chat_in_its_workspace() {
        let workspace = tempfile::tempdir().expect("first workspace");
        let other_workspace = tempfile::tempdir().expect("second workspace");
        let registry = BackgroundCommandRegistry::new();
        let mut first_request = background_request(workspace.path(), "sleep 30");
        first_request.owner_chat_id = Some("chat-1".to_string());
        let first = registry.start(first_request).expect("start first command");
        let mut second_request = background_request(other_workspace.path(), "sleep 30");
        second_request.owner_chat_id = Some("chat-1".to_string());
        let second = registry
            .start(second_request)
            .expect("start second command");

        let stopped = registry
            .stop_for_workspace_chat(workspace.path(), "chat-1")
            .expect("stop first workspace chat");
        let terminal = wait_for_terminal(&registry, &first.command_id);
        let other = registry
            .command(&second.command_id)
            .expect("second command");
        registry.shutdown_all();

        assert_eq!(stopped, 1);
        assert_eq!(terminal.status, BackgroundCommandStatus::Stopped);
        assert_eq!(other.status, BackgroundCommandStatus::Running);
    }

    #[cfg(unix)]
    #[test]
    fn registry_stops_commands_after_the_workspace_directory_is_removed() {
        let workspace = tempfile::tempdir().expect("workspace");
        let workspace_path = std::fs::canonicalize(workspace.path()).expect("canonical workspace");
        let registry = BackgroundCommandRegistry::new();
        let command = registry
            .start(background_request(&workspace_path, "sleep 30"))
            .expect("start command");

        std::fs::remove_dir_all(&workspace_path).expect("remove workspace");
        let stopped = registry
            .stop_for_workspace(&workspace_path)
            .expect("stop removed workspace command");
        let terminal = wait_for_terminal(&registry, &command.command_id);

        assert_eq!(stopped, 1);
        assert_eq!(terminal.status, BackgroundCommandStatus::Stopped);
    }

    #[cfg(unix)]
    #[test]
    fn registry_stop_and_wait_for_workspace_reaches_terminal_before_return() {
        let workspace = tempfile::tempdir().expect("workspace");
        let other = tempfile::tempdir().expect("other workspace");
        let registry = BackgroundCommandRegistry::new();
        let target = registry
            .start(background_request(workspace.path(), "sleep 30"))
            .expect("start target command");
        let other_command = registry
            .start(background_request(other.path(), "sleep 30"))
            .expect("start other command");
        let target_pid = target.pid;
        assert!(process_is_alive(target_pid));

        let stopped = registry
            .stop_and_wait_for_workspace(workspace.path(), Duration::from_secs(3))
            .expect("stop and wait");
        let target_snapshot = registry
            .command(&target.command_id)
            .expect("target command");
        let other_snapshot = registry
            .command(&other_command.command_id)
            .expect("other command");

        assert_eq!(stopped, 1);
        assert!(
            target_snapshot.status.is_terminal(),
            "stop_and_wait must not return while the workspace command is still running"
        );
        assert_eq!(target_snapshot.status, BackgroundCommandStatus::Stopped);
        assert!(
            !process_is_alive(target_pid),
            "process tree must exit before stop_and_wait returns"
        );
        assert_eq!(other_snapshot.status, BackgroundCommandStatus::Running);
        assert!(process_is_alive(other_command.pid));

        registry.shutdown_all();
    }

    #[cfg(unix)]
    #[test]
    fn registry_prunes_terminal_records_after_the_configured_retention() {
        let workspace = tempfile::tempdir().expect("workspace");
        let registry = BackgroundCommandRegistry::with_limits(BackgroundCommandLimits {
            completed_retention: Duration::from_millis(100),
            ..BackgroundCommandLimits::default()
        });
        let command = registry
            .start(background_request(workspace.path(), "true"))
            .expect("start command");
        let _ = wait_for_terminal(&registry, &command.command_id);

        thread::sleep(Duration::from_millis(150));
        registry.prune_completed();
        let result = registry.command(&command.command_id);

        assert!(matches!(
            result,
            Err(BackgroundCommandError::CommandNotFound(_))
        ));
    }

    #[cfg(unix)]
    fn background_request(workspace_path: &Path, script: &str) -> BackgroundCommandRequest {
        BackgroundCommandRequest {
            workspace_path: workspace_path.to_path_buf(),
            cwd: workspace_path.to_path_buf(),
            command: "sh".to_string(),
            args: vec!["-c".to_string(), script.to_string()],
            owner_chat_id: None,
            owner_run_id: None,
            timeout: None,
        }
    }

    #[cfg(unix)]
    fn wait_for_output_pid(registry: &BackgroundCommandRegistry, command_id: &str) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let output = registry
                .output_after(command_id, None)
                .expect("command output");
            let bytes = output
                .chunks
                .iter()
                .flat_map(|chunk| chunk.bytes.iter().copied())
                .collect::<Vec<_>>();
            if let Ok(pid) = String::from_utf8_lossy(&bytes).trim().parse() {
                return pid;
            }
            assert!(
                Instant::now() < deadline,
                "command did not report grandchild pid"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(any(unix, windows))]
    fn wait_for_terminal(
        registry: &BackgroundCommandRegistry,
        command_id: &str,
    ) -> BackgroundCommandSnapshot {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let snapshot = registry.command(command_id).expect("command status");
            if snapshot.status.is_terminal() {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "command did not reach a terminal status"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    fn process_is_alive(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}
