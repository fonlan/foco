use std::{
    collections::HashSet,
    fmt, io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use notify::{RecursiveMode, Watcher};

mod extractors;
mod indexing;
mod resolver;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

const DEFAULT_WATCH_DEBOUNCE: Duration = Duration::from_millis(750);
const WATCHER_COMMAND_QUEUE_CAPACITY: usize = 2;
const MAX_PENDING_WATCH_PATHS: usize = 1_024;

enum WatcherCommand {
    EventsReady,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WatcherFallbackReason {
    DirtyPathOverflow,
    UnclassifiableEvent,
    NotifyError,
}

impl WatcherFallbackReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::DirtyPathOverflow => "dirty_path_overflow",
            Self::UnclassifiableEvent => "unclassifiable_event",
            Self::NotifyError => "notify_error",
        }
    }
}

#[derive(Default)]
struct PendingWatchEvents {
    dirty_paths: HashSet<PathBuf>,
    events_received: u64,
    relevant_events: u64,
    filtered_events: u64,
    event_errors: u64,
    fallback_reason: Option<WatcherFallbackReason>,
}

impl PendingWatchEvents {
    fn has_work(&self) -> bool {
        !self.dirty_paths.is_empty() || self.fallback_reason.is_some()
    }

    fn record_fallback(&mut self, reason: WatcherFallbackReason) {
        if self.fallback_reason.is_none() {
            self.fallback_reason = Some(reason);
        }
    }
}

/// Coalesces notify callbacks before they enter the worker command stream.
///
/// The callback must never enqueue one message per filesystem event: build and
/// Git activity may produce far more notifications than the graph worker can
/// consume. A single wakeup represents the bounded set accumulated so far.
struct WatchEventAccumulator {
    pending: Mutex<PendingWatchEvents>,
    wake_scheduled: AtomicBool,
    command_tx: mpsc::SyncSender<WatcherCommand>,
    workspace_path: PathBuf,
}

impl WatchEventAccumulator {
    fn new(workspace_path: PathBuf, command_tx: mpsc::SyncSender<WatcherCommand>) -> Self {
        Self {
            pending: Mutex::new(PendingWatchEvents::default()),
            wake_scheduled: AtomicBool::new(false),
            command_tx,
            workspace_path,
        }
    }

    fn record(&self, event: notify::Result<notify::Event>) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pending.events_received = pending.events_received.saturating_add(1);

        match event {
            Ok(event) => self.record_event(&mut pending, event),
            Err(_) => {
                pending.event_errors = pending.event_errors.saturating_add(1);
                pending.record_fallback(WatcherFallbackReason::NotifyError);
            }
        }

        let should_wake = pending.has_work();
        drop(pending);
        if should_wake {
            self.schedule_wakeup();
        }
    }

    fn record_event(&self, pending: &mut PendingWatchEvents, event: notify::Event) {
        if event.paths.is_empty() {
            pending.record_fallback(WatcherFallbackReason::UnclassifiableEvent);
            return;
        }

        let mut relevant = false;
        for path in event.paths {
            let Some(path) = normalize_watch_path(&self.workspace_path, &path) else {
                pending.filtered_events = pending.filtered_events.saturating_add(1);
                continue;
            };
            if !indexing::should_consider_watch_path(&self.workspace_path, &path)
                || path_is_existing_directory(&path)
            {
                pending.filtered_events = pending.filtered_events.saturating_add(1);
                continue;
            }

            relevant = true;
            if pending.fallback_reason.is_none() {
                if pending.dirty_paths.len() < MAX_PENDING_WATCH_PATHS {
                    pending.dirty_paths.insert(path);
                } else {
                    pending.dirty_paths.clear();
                    pending.record_fallback(WatcherFallbackReason::DirtyPathOverflow);
                }
            }
        }

        if relevant {
            pending.relevant_events = pending.relevant_events.saturating_add(1);
        }
    }

    fn schedule_wakeup(&self) {
        if self.wake_scheduled.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = self.command_tx.try_send(WatcherCommand::EventsReady);
    }

    fn take_pending(&self) -> PendingWatchEvents {
        // Clear the wake flag before draining the set. A concurrent callback
        // can then schedule one follow-up wakeup instead of being stranded
        // behind this drain.
        self.wake_scheduled.store(false, Ordering::Release);
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::mem::take(&mut *pending)
    }
}

fn normalize_watch_path(workspace_path: &Path, path: &Path) -> Option<PathBuf> {
    let relative_path = path.strip_prefix(workspace_path).ok()?;
    let mut normalized = PathBuf::from(workspace_path);
    for component in relative_path.components() {
        match component {
            std::path::Component::Normal(component) => normalized.push(component),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return None;
            }
        }
    }
    Some(normalized)
}

fn path_is_existing_directory(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

/// Counters emitted at bounded watcher lifecycle points rather than once per
/// filesystem event. They deliberately contain no event paths or payloads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct WatcherDiagnosticCounters {
    events_received: u64,
    relevant_events: u64,
    filtered_events: u64,
    event_errors: u64,
    event_batches: u64,
    dirty_paths: u64,
    fallback_refreshes: u64,
    debounce_resets: u64,
    refreshes: u64,
}

impl WatcherDiagnosticCounters {
    fn record_batch(&mut self, batch: &PendingWatchEvents) {
        self.events_received = self.events_received.saturating_add(batch.events_received);
        self.relevant_events = self.relevant_events.saturating_add(batch.relevant_events);
        self.filtered_events = self.filtered_events.saturating_add(batch.filtered_events);
        self.event_errors = self.event_errors.saturating_add(batch.event_errors);
        self.event_batches = self.event_batches.saturating_add(1);
        self.dirty_paths = self
            .dirty_paths
            .saturating_add(u64::try_from(batch.dirty_paths.len()).unwrap_or(u64::MAX));
        if batch.fallback_reason.is_some() {
            self.fallback_refreshes = self.fallback_refreshes.saturating_add(1);
        }
    }

    fn record_debounce_reset(&mut self) {
        self.debounce_resets = self.debounce_resets.saturating_add(1);
    }

    fn record_refresh(&mut self) {
        self.refreshes = self.refreshes.saturating_add(1);
    }
}

struct WatcherDiagnostics {
    started_at: Instant,
    total: WatcherDiagnosticCounters,
    since_last_refresh: WatcherDiagnosticCounters,
}

impl WatcherDiagnostics {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            total: WatcherDiagnosticCounters::default(),
            since_last_refresh: WatcherDiagnosticCounters::default(),
        }
    }

    fn record_batch(&mut self, batch: &PendingWatchEvents) {
        self.total.record_batch(batch);
        self.since_last_refresh.record_batch(batch);
    }

    fn record_debounce_reset(&mut self) {
        self.total.record_debounce_reset();
        self.since_last_refresh.record_debounce_reset();
    }

    fn take_refresh_window(&mut self) -> WatcherDiagnosticCounters {
        self.total.record_refresh();
        let mut window = std::mem::take(&mut self.since_last_refresh);
        window.record_refresh();
        window
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexReport {
    pub scanned_files: usize,
    pub indexed_files: usize,
    pub unchanged_files: usize,
    pub skipped_files: usize,
    pub deleted_files: usize,
    pub parse_errors: usize,
    /// Total time spent reading, hashing, detecting, and extracting workspace files.
    pub file_prepare_duration_us: u64,
    /// Total time spent acquiring a workspace database and persisting graph updates.
    pub sqlite_persistence_duration_us: u64,
    /// Total time spent resolving module imports after file persistence completes.
    pub resolver_duration_us: u64,
}

/// Indexes a workspace using owned extraction facts, then opens SQLite only for
/// short batched replacement transactions.
pub fn index_workspace(workspace_path: impl AsRef<Path>) -> Result<IndexReport, CodeGraphError> {
    indexing::index_workspace(workspace_path.as_ref())
}

/// Incrementally refreshes explicitly changed workspace paths without walking
/// the whole workspace. Callers must fall back to [`index_workspace`] when the
/// changed path set is incomplete or cannot be classified safely.
pub fn index_workspace_paths(
    workspace_path: impl AsRef<Path>,
    dirty_paths: impl IntoIterator<Item = PathBuf>,
) -> Result<IndexReport, CodeGraphError> {
    indexing::index_workspace_paths(workspace_path.as_ref(), dirty_paths)
}

pub fn start_code_graph_watcher(
    workspace_path: impl AsRef<Path>,
) -> Result<CodeGraphWatcher, CodeGraphError> {
    start_code_graph_watcher_with_debounce(workspace_path, DEFAULT_WATCH_DEBOUNCE)
}

pub fn start_code_graph_watcher_with_debounce(
    workspace_path: impl AsRef<Path>,
    debounce: Duration,
) -> Result<CodeGraphWatcher, CodeGraphError> {
    let workspace_path = std::fs::canonicalize(workspace_path.as_ref())
        .map_err(|source| io_error(workspace_path.as_ref(), source))?;
    let (command_tx, command_rx) = mpsc::sync_channel(WATCHER_COMMAND_QUEUE_CAPACITY);
    let accumulator = Arc::new(WatchEventAccumulator::new(
        workspace_path.clone(),
        command_tx.clone(),
    ));
    let callback_accumulator = Arc::clone(&accumulator);
    let mut watcher = notify::recommended_watcher(move |event| {
        callback_accumulator.record(event);
    })?;
    watcher.watch(&workspace_path, RecursiveMode::Recursive)?;

    let worker_workspace_path = workspace_path.clone();
    #[cfg(test)]
    let refresh_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
    #[cfg(test)]
    let worker_refresh_count = Arc::clone(&refresh_count);
    let handle = thread::spawn(move || {
        let _watcher = watcher;
        let mut pending = false;
        let mut pending_dirty_paths = HashSet::new();
        let mut pending_requires_full_refresh = false;
        let mut next_index_at = Instant::now();
        let mut diagnostics = WatcherDiagnostics::new();

        loop {
            let command = if pending {
                let remaining_debounce = next_index_at.saturating_duration_since(Instant::now());
                match command_rx.recv_timeout(remaining_debounce) {
                    Ok(command) => Some(command),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match command_rx.recv() {
                    Ok(command) => Some(command),
                    Err(_) => break,
                }
            };

            match command {
                Some(WatcherCommand::Stop) => break,
                Some(WatcherCommand::EventsReady) => {
                    let batch = accumulator.take_pending();
                    let should_index = batch.has_work();
                    diagnostics.record_batch(&batch);
                    if should_index {
                        pending_dirty_paths.extend(batch.dirty_paths);
                        pending_requires_full_refresh |= batch.fallback_reason.is_some();
                        if let Some(reason) = batch.fallback_reason {
                            tracing::warn!(
                                workspace = %worker_workspace_path.display(),
                                index_scope = "full_workspace",
                                index_reason = "watcher_safe_fallback",
                                watcher_fallback_reason = reason.as_str(),
                                "code graph watcher will safely refresh the full workspace index"
                            );
                        }
                        pending = true;
                        next_index_at = Instant::now() + debounce;
                        diagnostics.record_debounce_reset();
                    }
                }
                None => {}
            }

            if pending && Instant::now() >= next_index_at {
                let window = diagnostics.take_refresh_window();
                #[cfg(test)]
                worker_refresh_count.fetch_add(1, Ordering::Relaxed);
                let (index_scope, index_reason, result) = if pending_requires_full_refresh {
                    pending_dirty_paths.clear();
                    (
                        "full_workspace",
                        "watcher_safe_fallback",
                        index_workspace(&worker_workspace_path),
                    )
                } else {
                    let dirty_paths = std::mem::take(&mut pending_dirty_paths);
                    (
                        "dirty_paths",
                        "watcher_debounced_relevant_event",
                        index_workspace_paths(&worker_workspace_path, dirty_paths),
                    )
                };
                match result {
                    Ok(report) => {
                        tracing::info!(
                            workspace = %worker_workspace_path.display(),
                            index_scope,
                            index_reason,
                            watch_event_queue = "bounded_coalescing",
                            watch_event_queue_capacity = WATCHER_COMMAND_QUEUE_CAPACITY,
                            watcher_event_batches = window.event_batches,
                            watcher_dirty_paths = window.dirty_paths,
                            watcher_events_received = window.events_received,
                            watcher_relevant_events = window.relevant_events,
                            watcher_filtered_events = window.filtered_events,
                            watcher_event_errors = window.event_errors,
                            watcher_fallback_refreshes = window.fallback_refreshes,
                            watcher_debounce_resets = window.debounce_resets,
                            watcher_refreshes = window.refreshes,
                            scanned_files = report.scanned_files,
                            indexed_files = report.indexed_files,
                            unchanged_files = report.unchanged_files,
                            skipped_files = report.skipped_files,
                            deleted_files = report.deleted_files,
                            parse_errors = report.parse_errors,
                            file_prepare_duration_us = report.file_prepare_duration_us,
                            sqlite_persistence_duration_us = report.sqlite_persistence_duration_us,
                            resolver_duration_us = report.resolver_duration_us,
                            "code graph watcher refreshed workspace index"
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            workspace = %worker_workspace_path.display(),
                            index_scope,
                            index_reason,
                            watch_event_queue = "bounded_coalescing",
                            watch_event_queue_capacity = WATCHER_COMMAND_QUEUE_CAPACITY,
                            watcher_event_batches = window.event_batches,
                            watcher_dirty_paths = window.dirty_paths,
                            watcher_events_received = window.events_received,
                            watcher_relevant_events = window.relevant_events,
                            watcher_filtered_events = window.filtered_events,
                            watcher_event_errors = window.event_errors,
                            watcher_fallback_refreshes = window.fallback_refreshes,
                            watcher_debounce_resets = window.debounce_resets,
                            watcher_refreshes = window.refreshes,
                            error = %error,
                            "code graph watcher refresh failed"
                        );
                    }
                }
                pending = false;
                pending_requires_full_refresh = false;
            }
        }

        tracing::info!(
            workspace = %worker_workspace_path.display(),
            watcher_lifetime_ms = diagnostics.started_at.elapsed().as_millis() as u64,
            watch_event_queue = "bounded_coalescing",
            watch_event_queue_capacity = WATCHER_COMMAND_QUEUE_CAPACITY,
            watcher_events_received = diagnostics.total.events_received,
            watcher_event_batches = diagnostics.total.event_batches,
            watcher_dirty_paths = diagnostics.total.dirty_paths,
            watcher_relevant_events = diagnostics.total.relevant_events,
            watcher_filtered_events = diagnostics.total.filtered_events,
            watcher_event_errors = diagnostics.total.event_errors,
            watcher_fallback_refreshes = diagnostics.total.fallback_refreshes,
            watcher_debounce_resets = diagnostics.total.debounce_resets,
            watcher_refreshes = diagnostics.total.refreshes,
            "code graph watcher stopped"
        );
    });

    Ok(CodeGraphWatcher {
        workspace_path,
        stop_tx: Some(command_tx),
        handle: Some(handle),
        #[cfg(test)]
        refresh_count,
    })
}

pub struct CodeGraphWatcher {
    workspace_path: PathBuf,
    stop_tx: Option<mpsc::SyncSender<WatcherCommand>>,
    handle: Option<JoinHandle<()>>,
    #[cfg(test)]
    refresh_count: Arc<std::sync::atomic::AtomicU64>,
}

impl CodeGraphWatcher {
    pub fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }

    #[cfg(test)]
    fn refresh_count(&self) -> u64 {
        self.refresh_count.load(Ordering::Relaxed)
    }
}

impl Drop for CodeGraphWatcher {
    fn drop(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(WatcherCommand::Stop);
        }
        if let Some(handle) = self.handle.take()
            && let Err(error) = handle.join()
        {
            tracing::warn!(?error, "code graph watcher thread join failed");
        }
    }
}

#[derive(Debug)]
pub enum CodeGraphError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Notify(notify::Error),
    Store(foco_store::workspace::WorkspaceDatabaseError),
    TreeSitterLanguage {
        language: &'static str,
        source: tree_sitter::LanguageError,
    },
    TreeSitterParse {
        path: PathBuf,
        language: &'static str,
    },
    WorkspaceRelativePath {
        workspace: PathBuf,
        path: PathBuf,
    },
}

impl fmt::Display for CodeGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {}", path.display(), source),
            Self::Notify(source) => write!(formatter, "filesystem watcher error: {source}"),
            Self::Store(source) => write!(formatter, "{source}"),
            Self::TreeSitterLanguage { language, source } => {
                write!(
                    formatter,
                    "failed to load Tree-sitter language {language}: {source}"
                )
            }
            Self::TreeSitterParse { path, language } => write!(
                formatter,
                "Tree-sitter parser returned no tree for {} as {language}",
                path.display()
            ),
            Self::WorkspaceRelativePath { workspace, path } => write!(
                formatter,
                "path {} is not inside workspace {}",
                path.display(),
                workspace.display()
            ),
        }
    }
}

impl std::error::Error for CodeGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Notify(source) => Some(source),
            Self::Store(source) => Some(source),
            Self::TreeSitterLanguage { source, .. } => Some(source),
            Self::TreeSitterParse { .. } | Self::WorkspaceRelativePath { .. } => None,
        }
    }
}

impl From<foco_store::workspace::WorkspaceDatabaseError> for CodeGraphError {
    fn from(source: foco_store::workspace::WorkspaceDatabaseError) -> Self {
        Self::Store(source)
    }
}

impl From<notify::Error> for CodeGraphError {
    fn from(source: notify::Error) -> Self {
        Self::Notify(source)
    }
}

pub(crate) fn io_error(path: &Path, source: io::Error) -> CodeGraphError {
    CodeGraphError::Io {
        path: path.to_path_buf(),
        source,
    }
}
