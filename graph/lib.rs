use std::{
    fmt, io,
    path::{Path, PathBuf},
    sync::mpsc,
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
    let (event_tx, event_rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = event_tx.send(event);
    })?;
    watcher.watch(&workspace_path, RecursiveMode::Recursive)?;

    let (stop_tx, stop_rx) = mpsc::channel();
    let worker_workspace_path = workspace_path.clone();
    let handle = thread::spawn(move || {
        let _watcher = watcher;
        let mut pending = false;
        let mut next_index_at = Instant::now();

        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }

            match event_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(event)) => {
                    if event.paths.iter().any(|path| {
                        indexing::should_consider_watch_path(&worker_workspace_path, path)
                    }) {
                        pending = true;
                        next_index_at = Instant::now() + debounce;
                    }
                }
                Ok(Err(error)) => {
                    tracing::warn!(workspace = %worker_workspace_path.display(), error = %error, "code graph watcher event failed");
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }

            if pending && Instant::now() >= next_index_at {
                match index_workspace(&worker_workspace_path) {
                    Ok(report) => {
                        tracing::info!(
                            workspace = %worker_workspace_path.display(),
                            indexed_files = report.indexed_files,
                            unchanged_files = report.unchanged_files,
                            deleted_files = report.deleted_files,
                            parse_errors = report.parse_errors,
                            "code graph watcher refreshed workspace index"
                        );
                    }
                    Err(error) => {
                        tracing::error!(workspace = %worker_workspace_path.display(), error = %error, "code graph watcher refresh failed");
                    }
                }
                pending = false;
            }
        }
    });

    Ok(CodeGraphWatcher {
        workspace_path,
        stop_tx: Some(stop_tx),
        handle: Some(handle),
    })
}

pub struct CodeGraphWatcher {
    workspace_path: PathBuf,
    stop_tx: Option<mpsc::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl CodeGraphWatcher {
    pub fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }
}

impl Drop for CodeGraphWatcher {
    fn drop(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
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
