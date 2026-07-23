use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use chrono::{Duration as ChronoDuration, SecondsFormat, Utc};
use foco_graph::{CodeGraphWatcher, index_workspace, start_code_graph_watcher};
use foco_store::{
    config::WorkspaceConfig,
    workspace::{WorkspaceDatabase, WorkspaceDatabaseError},
};
use foco_tools::ToolCancellationToken;

use crate::{AppResult, AppState};

/// Lifecycle phase for one canonical execution-root code graph index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CodeGraphIndexPhase {
    Initializing,
    Ready,
    Failed,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct CodeGraphIndexStatus {
    pub(crate) phase: CodeGraphIndexPhase,
    pub(crate) execution_root: PathBuf,
    /// Bounded diagnostic when `phase == Failed`.
    pub(crate) error: Option<String>,
    /// Stage label when failed (for example `index` or `watcher`).
    pub(crate) failed_stage: Option<String>,
}

enum CodeGraphEntryState {
    Initializing,
    Ready {
        /// Held so Drop stops the filesystem watcher for this execution root.
        #[allow(dead_code)]
        watcher: CodeGraphWatcher,
    },
    Failed {
        stage: String,
        error: String,
    },
}

struct CodeGraphEntry {
    /// Monotonic claim generation for this path; stale workers must not publish.
    generation: u64,
    state: CodeGraphEntryState,
}

/// Token returned by a successful [`CodeGraphIndexState::claim`].
///
/// `complete` / `fail` only apply when the token still matches the live entry,
/// so a released or reclaimed path cannot be overwritten by an older worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CodeGraphClaimToken {
    generation: u64,
}

/// Per canonical execution-root code graph lifecycle registry.
///
/// Index work runs outside the registry lock. Waiters use a shared condition
/// variable so blocking Graph tool paths can wait without holding the map lock.
#[derive(Default)]
pub(crate) struct CodeGraphIndexState {
    entries: HashMap<PathBuf, CodeGraphEntry>,
    notify: Arc<Condvar>,
    next_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CodeGraphReadinessError {
    Cancelled,
    TimedOut {
        execution_root: PathBuf,
    },
    Failed {
        execution_root: PathBuf,
        stage: String,
        error: String,
    },
    InvalidPath {
        path: PathBuf,
        error: String,
    },
}

impl std::fmt::Display for CodeGraphReadinessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "code graph readiness wait cancelled"),
            Self::TimedOut { execution_root } => write!(
                f,
                "code graph index is still initializing for execution root '{}'; retry after indexing completes",
                execution_root.display()
            ),
            Self::Failed {
                execution_root,
                stage,
                error,
            } => write!(
                f,
                "code graph index failed for execution root '{}' during {stage}: {error}",
                execution_root.display()
            ),
            Self::InvalidPath { path, error } => {
                write!(
                    f,
                    "failed to resolve code graph execution root '{}': {error}",
                    path.display()
                )
            }
        }
    }
}

impl CodeGraphIndexState {
    fn canonicalize_key(path: &Path) -> Result<PathBuf, String> {
        std::fs::canonicalize(path).map_err(|source| source.to_string())
    }

    /// Claims exclusive initialization for `execution_root`.
    ///
    /// Returns `Some(token)` when this caller should run indexing. Concurrent
    /// claims for the same canonical path return `None`. A previous `Failed`
    /// entry may be reclaimed for an explicit retry. Pass the token to
    /// `complete` / `fail` so a released or superseded claim cannot publish.
    pub(crate) fn claim(
        &mut self,
        execution_root: &Path,
    ) -> Result<Option<CodeGraphClaimToken>, String> {
        let key = Self::canonicalize_key(execution_root)?;
        match self.entries.get(&key) {
            Some(CodeGraphEntry {
                state: CodeGraphEntryState::Initializing | CodeGraphEntryState::Ready { .. },
                ..
            }) => Ok(None),
            Some(CodeGraphEntry {
                state: CodeGraphEntryState::Failed { .. },
                ..
            })
            | None => {
                self.next_generation = self.next_generation.wrapping_add(1);
                let generation = self.next_generation;
                self.entries.insert(
                    key,
                    CodeGraphEntry {
                        generation,
                        state: CodeGraphEntryState::Initializing,
                    },
                );
                self.notify.notify_all();
                Ok(Some(CodeGraphClaimToken { generation }))
            }
        }
    }

    /// Publishes Ready only when `token` still owns the path entry.
    pub(crate) fn complete(
        &mut self,
        execution_root: &Path,
        token: CodeGraphClaimToken,
        watcher: CodeGraphWatcher,
    ) {
        let Ok(key) = Self::canonicalize_key(execution_root) else {
            tracing::error!(
                execution_root = %execution_root.display(),
                "failed to canonicalize path when completing code graph index"
            );
            return;
        };
        let Some(entry) = self.entries.get_mut(&key) else {
            tracing::debug!(
                execution_root = %execution_root.display(),
                generation = token.generation,
                "ignoring code graph complete for released path"
            );
            return;
        };
        if entry.generation != token.generation {
            tracing::debug!(
                execution_root = %execution_root.display(),
                token_generation = token.generation,
                live_generation = entry.generation,
                "ignoring stale code graph complete"
            );
            return;
        }
        entry.state = CodeGraphEntryState::Ready { watcher };
        self.notify.notify_all();
    }

    /// Publishes Failed only when `token` still owns the path entry.
    pub(crate) fn fail(
        &mut self,
        execution_root: &Path,
        token: CodeGraphClaimToken,
        stage: &str,
        error: impl Into<String>,
    ) {
        let error = error.into();
        let Ok(key) = Self::canonicalize_key(execution_root) else {
            tracing::error!(
                execution_root = %execution_root.display(),
                stage,
                error = %error,
                "failed to canonicalize path when recording code graph failure"
            );
            return;
        };
        let Some(entry) = self.entries.get_mut(&key) else {
            tracing::debug!(
                execution_root = %execution_root.display(),
                generation = token.generation,
                stage,
                "ignoring code graph fail for released path"
            );
            return;
        };
        if entry.generation != token.generation {
            tracing::debug!(
                execution_root = %execution_root.display(),
                token_generation = token.generation,
                live_generation = entry.generation,
                stage,
                "ignoring stale code graph fail"
            );
            return;
        }
        entry.state = CodeGraphEntryState::Failed {
            stage: stage.to_string(),
            error,
        };
        self.notify.notify_all();
    }

    /// Drops the entry and stops its watcher (if any). Safe when absent.
    pub(crate) fn release(&mut self, execution_root: &Path) -> Result<(), String> {
        let key = Self::canonicalize_key(execution_root)?;
        self.entries.remove(&key);
        self.notify.notify_all();
        Ok(())
    }

    pub(crate) fn status(
        &self,
        execution_root: &Path,
    ) -> Result<Option<CodeGraphIndexStatus>, String> {
        let key = Self::canonicalize_key(execution_root)?;
        Ok(self.entries.get(&key).map(|entry| match &entry.state {
            CodeGraphEntryState::Initializing => CodeGraphIndexStatus {
                phase: CodeGraphIndexPhase::Initializing,
                execution_root: key.clone(),
                error: None,
                failed_stage: None,
            },
            CodeGraphEntryState::Ready { .. } => CodeGraphIndexStatus {
                phase: CodeGraphIndexPhase::Ready,
                execution_root: key.clone(),
                error: None,
                failed_stage: None,
            },
            CodeGraphEntryState::Failed { stage, error } => CodeGraphIndexStatus {
                phase: CodeGraphIndexPhase::Failed,
                execution_root: key.clone(),
                error: Some(error.clone()),
                failed_stage: Some(stage.clone()),
            },
        }))
    }

    #[cfg(test)]
    pub(crate) fn watcher_count(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| matches!(entry.state, CodeGraphEntryState::Ready { .. }))
            .count()
    }

    #[cfg(test)]
    pub(crate) fn clear_watchers(&mut self) {
        self.entries.clear();
        self.notify.notify_all();
    }

    #[cfg(test)]
    pub(crate) fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

/// Waits until the code graph for `execution_root` is Ready, or until the
/// deadline / cancellation token fires.
///
/// Paths that are not registered yet are treated as ready so ordinary main
/// workspace queries keep their existing cold-query behavior. Callers that need
/// a warm index must claim/spawn initialization first.
pub(crate) fn wait_for_code_graph_ready(
    indexes: &Arc<Mutex<CodeGraphIndexState>>,
    execution_root: &Path,
    deadline: Option<Instant>,
    cancellation_token: Option<&ToolCancellationToken>,
) -> Result<(), CodeGraphReadinessError> {
    let key = CodeGraphIndexState::canonicalize_key(execution_root).map_err(|error| {
        CodeGraphReadinessError::InvalidPath {
            path: execution_root.to_path_buf(),
            error,
        }
    })?;

    let mut guard = indexes
        .lock()
        .map_err(|_| CodeGraphReadinessError::Failed {
            execution_root: key.clone(),
            stage: "registry".to_string(),
            error: "code graph index lock poisoned".to_string(),
        })?;
    let notify = guard.notify.clone();

    loop {
        if cancellation_token.is_some_and(ToolCancellationToken::is_cancelled) {
            return Err(CodeGraphReadinessError::Cancelled);
        }

        match guard.entries.get(&key).map(|entry| &entry.state) {
            None | Some(CodeGraphEntryState::Ready { .. }) => return Ok(()),
            Some(CodeGraphEntryState::Failed { stage, error }) => {
                return Err(CodeGraphReadinessError::Failed {
                    execution_root: key,
                    stage: stage.clone(),
                    error: error.clone(),
                });
            }
            Some(CodeGraphEntryState::Initializing) => {}
        }

        let remaining = match deadline {
            Some(deadline) => match deadline.checked_duration_since(Instant::now()) {
                Some(remaining) if !remaining.is_zero() => remaining,
                _ => {
                    return Err(CodeGraphReadinessError::TimedOut {
                        execution_root: key,
                    });
                }
            },
            None => Duration::from_millis(50),
        };
        // Always poll in short slices so ToolCancellationToken can be observed
        // without waiting for the full tool deadline or index completion.
        const CANCEL_POLL: Duration = Duration::from_millis(50);
        let wait_for = remaining.min(CANCEL_POLL);

        let (next_guard, wait_result) =
            notify
                .wait_timeout(guard, wait_for)
                .map_err(|_| CodeGraphReadinessError::Failed {
                    execution_root: key.clone(),
                    stage: "registry".to_string(),
                    error: "code graph index lock poisoned while waiting".to_string(),
                })?;
        guard = next_guard;

        if wait_result.timed_out() {
            if let Some(deadline) = deadline
                && deadline.checked_duration_since(Instant::now()).is_none()
            {
                return Err(CodeGraphReadinessError::TimedOut {
                    execution_root: key,
                });
            }
            // Short poll slice or spurious timeout before deadline: re-check.
        }
    }
}

pub(crate) fn recently_active_code_graph_workspaces(
    workspaces: &[WorkspaceConfig],
) -> Result<Vec<WorkspaceConfig>, WorkspaceDatabaseError> {
    let since = (Utc::now() - ChronoDuration::days(7)).to_rfc3339_opts(SecondsFormat::Millis, true);
    let mut active_workspaces = Vec::new();

    for workspace in workspaces {
        if workspace.is_remote() {
            continue;
        }
        let database = WorkspaceDatabase::open_or_create(&workspace.path)?;
        if database.has_user_message_since(&since)? {
            active_workspaces.push(workspace.clone());
        }
    }

    tracing::info!(
        workspace_count = workspaces.len(),
        active_workspace_count = active_workspaces.len(),
        inactive_workspace_count = workspaces.len().saturating_sub(active_workspaces.len()),
        since,
        "selected recently active workspaces for startup code graph initialization"
    );

    Ok(active_workspaces)
}

pub(crate) fn spawn_code_graph_index_initialization(
    workspaces: Vec<WorkspaceConfig>,
    indexes: Arc<Mutex<CodeGraphIndexState>>,
) -> AppResult<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("foco-code-graph-startup".to_string())
        .spawn(move || initialize_code_graph_indexes(&workspaces, &indexes))
        .map_err(Into::into)
}

fn initialize_code_graph_indexes(
    workspaces: &[WorkspaceConfig],
    indexes: &Arc<Mutex<CodeGraphIndexState>>,
) {
    let all_started_at = Instant::now();
    tracing::info!(
        workspace_count = workspaces.len(),
        "background code graph initialization started"
    );
    for workspace in workspaces {
        initialize_code_graph_workspace_if_needed(workspace.clone(), indexes.clone());
    }
    tracing::info!(
        elapsed_ms = all_started_at.elapsed().as_millis() as u64,
        "background code graph initialization completed"
    );
}

fn initialize_code_graph_workspace_if_needed(
    workspace: WorkspaceConfig,
    indexes: Arc<Mutex<CodeGraphIndexState>>,
) {
    let claim_token = match indexes.lock() {
        Ok(mut guard) => match guard.claim(&workspace.path) {
            Ok(token) => token,
            Err(error) => {
                tracing::error!(
                    workspace_id = %workspace.id,
                    workspace_path = %workspace.path.display(),
                    error = %error,
                    "failed to claim code graph initialization"
                );
                return;
            }
        },
        Err(_) => {
            tracing::error!(
                workspace_id = %workspace.id,
                workspace_path = %workspace.path.display(),
                "code graph index lock poisoned while claiming initialization"
            );
            return;
        }
    };
    let Some(claim_token) = claim_token else {
        return;
    };

    let started_at = Instant::now();
    tracing::info!(
        workspace_id = %workspace.id,
        workspace_path = %workspace.path.display(),
        "background code graph workspace initialization started"
    );
    match initialize_code_graph_workspace(&workspace) {
        Ok(watcher) => {
            if let Ok(mut guard) = indexes.lock() {
                guard.complete(&workspace.path, claim_token, watcher);
            }
            tracing::info!(
                workspace_id = %workspace.id,
                workspace_path = %workspace.path.display(),
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                "background code graph workspace initialization completed"
            );
        }
        Err(error) => {
            if let Ok(mut guard) = indexes.lock() {
                guard.fail(
                    &workspace.path,
                    claim_token,
                    "initialize",
                    error.to_string(),
                );
            }
            tracing::error!(
                workspace_id = %workspace.id,
                workspace_path = %workspace.path.display(),
                error = %error,
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                "failed to initialize code graph index"
            );
        }
    }
}

pub(crate) fn spawn_code_graph_workspace_initialization_if_needed(
    state: &AppState,
    workspace: &WorkspaceConfig,
) {
    if workspace.is_remote() {
        // ponytail: local main never watches remote paths; sidecar owns remote graph index and watcher lifecycle.
        return;
    }
    let claim_token = match state.code_graph_indexes.lock() {
        Ok(mut guard) => match guard.claim(&workspace.path) {
            Ok(token) => token,
            Err(error) => {
                tracing::error!(
                    workspace_id = %workspace.id,
                    workspace_path = %workspace.path.display(),
                    error = %error,
                    "failed to claim lazy code graph initialization"
                );
                return;
            }
        },
        Err(_) => {
            tracing::error!(
                workspace_id = %workspace.id,
                workspace_path = %workspace.path.display(),
                "code graph index lock poisoned while claiming lazy initialization"
            );
            return;
        }
    };
    let Some(claim_token) = claim_token else {
        return;
    };

    let workspace = workspace.clone();
    let worker_workspace = workspace.clone();
    let indexes = state.code_graph_indexes.clone();
    if let Err(error) = std::thread::Builder::new()
        .name(format!("foco-code-graph-{}", workspace.id))
        .spawn(move || {
            let workspace = worker_workspace;
            let started_at = Instant::now();
            tracing::info!(
                workspace_id = %workspace.id,
                workspace_path = %workspace.path.display(),
                "lazy code graph workspace initialization started"
            );
            match initialize_code_graph_workspace(&workspace) {
                Ok(watcher) => {
                    if let Ok(mut guard) = indexes.lock() {
                        guard.complete(&workspace.path, claim_token, watcher);
                    }
                    tracing::info!(
                        workspace_id = %workspace.id,
                        workspace_path = %workspace.path.display(),
                        elapsed_ms = started_at.elapsed().as_millis() as u64,
                        "lazy code graph workspace initialization completed"
                    );
                }
                Err(error) => {
                    if let Ok(mut guard) = indexes.lock() {
                        guard.fail(
                            &workspace.path,
                            claim_token,
                            "initialize",
                            error.to_string(),
                        );
                    }
                    tracing::error!(
                        workspace_id = %workspace.id,
                        workspace_path = %workspace.path.display(),
                        error = %error,
                        elapsed_ms = started_at.elapsed().as_millis() as u64,
                        "failed to initialize lazy code graph index"
                    );
                }
            }
        })
    {
        if let Ok(mut guard) = state.code_graph_indexes.lock() {
            guard.fail(&workspace.path, claim_token, "spawn", error.to_string());
        }
        tracing::error!(
            workspace_id = %workspace.id,
            workspace_path = %workspace.path.display(),
            error = %error,
            "failed to spawn lazy code graph initialization"
        );
    }
}

/// Spawns background index initialization for an arbitrary local execution root
/// (shared workspace or isolated worktree). Concurrent calls for the same
/// canonical path only start one worker.
///
/// Wired by later plan phases for worktree prewarm; kept here so the registry
/// claim/complete/fail path is shared with main-workspace initialization.
#[allow(dead_code)]
pub(crate) fn spawn_code_graph_execution_root_initialization_if_needed(
    indexes: Arc<Mutex<CodeGraphIndexState>>,
    execution_root: PathBuf,
    label: impl Into<String>,
) {
    let label = label.into();
    let claim_token = match indexes.lock() {
        Ok(mut guard) => match guard.claim(&execution_root) {
            Ok(token) => token,
            Err(error) => {
                tracing::error!(
                    execution_root = %execution_root.display(),
                    label = %label,
                    error = %error,
                    "failed to claim code graph execution-root initialization"
                );
                return;
            }
        },
        Err(_) => {
            tracing::error!(
                execution_root = %execution_root.display(),
                label = %label,
                "code graph index lock poisoned while claiming execution-root initialization"
            );
            return;
        }
    };
    let Some(claim_token) = claim_token else {
        return;
    };

    let worker_root = execution_root.clone();
    let worker_label = label.clone();
    let worker_indexes = indexes.clone();
    if let Err(error) = std::thread::Builder::new()
        .name(format!("foco-code-graph-{}", sanitize_thread_label(&label)))
        .spawn(move || {
            let started_at = Instant::now();
            tracing::info!(
                execution_root = %worker_root.display(),
                label = %worker_label,
                "code graph execution-root initialization started"
            );
            match initialize_code_graph_execution_root(&worker_root) {
                Ok(watcher) => {
                    if let Ok(mut guard) = worker_indexes.lock() {
                        guard.complete(&worker_root, claim_token, watcher);
                    }
                    tracing::info!(
                        execution_root = %worker_root.display(),
                        label = %worker_label,
                        elapsed_ms = started_at.elapsed().as_millis() as u64,
                        "code graph execution-root initialization completed"
                    );
                }
                Err(error) => {
                    if let Ok(mut guard) = worker_indexes.lock() {
                        guard.fail(&worker_root, claim_token, "initialize", error.to_string());
                    }
                    tracing::error!(
                        execution_root = %worker_root.display(),
                        label = %worker_label,
                        error = %error,
                        elapsed_ms = started_at.elapsed().as_millis() as u64,
                        "failed to initialize code graph for execution root"
                    );
                }
            }
        })
    {
        if let Ok(mut guard) = indexes.lock() {
            guard.fail(&execution_root, claim_token, "spawn", error.to_string());
        }
        tracing::error!(
            execution_root = %execution_root.display(),
            label = %label,
            error = %error,
            "failed to spawn code graph execution-root initialization"
        );
    }
}

fn sanitize_thread_label(label: &str) -> String {
    label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .take(48)
        .collect()
}

fn initialize_code_graph_workspace(workspace: &WorkspaceConfig) -> AppResult<CodeGraphWatcher> {
    initialize_code_graph_execution_root(&workspace.path)
}

fn initialize_code_graph_execution_root(execution_root: &Path) -> AppResult<CodeGraphWatcher> {
    let index_started_at = Instant::now();
    let report = index_workspace(execution_root)?;
    tracing::info!(
        execution_root = %execution_root.display(),
        scanned_files = report.scanned_files,
        indexed_files = report.indexed_files,
        unchanged_files = report.unchanged_files,
        skipped_files = report.skipped_files,
        deleted_files = report.deleted_files,
        parse_errors = report.parse_errors,
        elapsed_ms = index_started_at.elapsed().as_millis() as u64,
        "initialized code graph index"
    );
    let watcher_started_at = Instant::now();
    let watcher = start_code_graph_watcher(execution_root)?;
    tracing::info!(
        execution_root = %execution_root.display(),
        elapsed_ms = watcher_started_at.elapsed().as_millis() as u64,
        "started code graph filesystem watcher"
    );

    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    #[test]
    fn claim_is_exclusive_until_fail_allows_retry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path();
        let mut state = CodeGraphIndexState::default();

        let token = state.claim(path).expect("claim").expect("first claim");
        assert!(state.claim(path).expect("second claim").is_none());
        state.fail(path, token, "index", "boom");
        assert!(state.claim(path).expect("retry after fail").is_some());
        assert_eq!(
            state.status(path).expect("status").map(|s| s.phase),
            Some(CodeGraphIndexPhase::Initializing)
        );
    }

    #[test]
    fn stale_complete_after_release_is_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path();
        let mut state = CodeGraphIndexState::default();
        let stale = state.claim(path).expect("claim").expect("token");
        state.release(path).expect("release");
        assert_eq!(state.entry_count(), 0);
        // complete requires a real watcher; fail is enough to prove generation gating.
        state.fail(path, stale, "index", "should not reinsert");
        assert_eq!(state.entry_count(), 0);
        assert!(state.status(path).expect("status").is_none());
    }

    #[test]
    fn stale_fail_after_reclaim_is_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path();
        let mut state = CodeGraphIndexState::default();
        let stale = state.claim(path).expect("claim").expect("token");
        state.release(path).expect("release");
        let live = state.claim(path).expect("reclaim").expect("new token");
        assert_ne!(stale, live);
        state.fail(path, stale, "index", "stale worker");
        assert_eq!(
            state.status(path).expect("status").map(|s| s.phase),
            Some(CodeGraphIndexPhase::Initializing),
            "stale fail must not overwrite the live claim"
        );
        state.fail(path, live, "index", "live failure");
        assert_eq!(
            state.status(path).expect("status").map(|s| s.phase),
            Some(CodeGraphIndexPhase::Failed)
        );
    }

    #[test]
    fn wait_returns_immediately_when_unregistered() {
        let dir = tempfile::tempdir().expect("tempdir");
        let indexes = Arc::new(Mutex::new(CodeGraphIndexState::default()));
        wait_for_code_graph_ready(&indexes, dir.path(), Some(Instant::now()), None)
            .expect("unregistered path is ready-equivalent");
    }

    #[test]
    fn wait_observes_ready_and_failed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().to_path_buf();
        let indexes = Arc::new(Mutex::new(CodeGraphIndexState::default()));
        let token = {
            let mut guard = indexes.lock().expect("lock");
            guard.claim(&path).expect("claim").expect("token")
        };

        let barrier = Arc::new(Barrier::new(2));
        let indexes_worker = indexes.clone();
        let path_worker = path.clone();
        let barrier_worker = barrier.clone();
        let handle = std::thread::spawn(move || {
            barrier_worker.wait();
            std::thread::sleep(Duration::from_millis(30));
            // complete requires a real watcher; publish Failed for this unit test.
            indexes_worker
                .lock()
                .expect("lock")
                .fail(&path_worker, token, "index", "test failure");
        });

        barrier.wait();
        let error = wait_for_code_graph_ready(
            &indexes,
            &path,
            Some(Instant::now() + Duration::from_secs(2)),
            None,
        )
        .expect_err("failed index should surface");
        match error {
            CodeGraphReadinessError::Failed { stage, error, .. } => {
                assert_eq!(stage, "index");
                assert!(error.contains("test failure"));
            }
            other => panic!("unexpected error: {other}"),
        }
        handle.join().expect("worker");
    }

    #[test]
    fn wait_times_out_while_initializing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().to_path_buf();
        let indexes = Arc::new(Mutex::new(CodeGraphIndexState::default()));
        {
            let mut guard = indexes.lock().expect("lock");
            assert!(guard.claim(&path).expect("claim").is_some());
        }
        let error = wait_for_code_graph_ready(
            &indexes,
            &path,
            Some(Instant::now() + Duration::from_millis(40)),
            None,
        )
        .expect_err("should time out");
        assert!(matches!(error, CodeGraphReadinessError::TimedOut { .. }));
    }

    #[test]
    fn wait_observes_cancellation_while_initializing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().to_path_buf();
        let indexes = Arc::new(Mutex::new(CodeGraphIndexState::default()));
        {
            let mut guard = indexes.lock().expect("lock");
            assert!(guard.claim(&path).expect("claim").is_some());
        }
        let token = ToolCancellationToken::default();
        let cancel = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            cancel.cancel();
        });
        let error = wait_for_code_graph_ready(
            &indexes,
            &path,
            Some(Instant::now() + Duration::from_secs(5)),
            Some(&token),
        )
        .expect_err("should cancel before long deadline");
        assert!(matches!(error, CodeGraphReadinessError::Cancelled));
    }

    #[test]
    fn release_removes_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path();
        let mut state = CodeGraphIndexState::default();
        assert!(state.claim(path).expect("claim").is_some());
        assert_eq!(state.entry_count(), 1);
        state.release(path).expect("release");
        assert_eq!(state.entry_count(), 0);
    }
}
