use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

use chrono::{Duration as ChronoDuration, SecondsFormat, Utc};
use foco_graph::{CodeGraphWatcher, index_workspace, start_code_graph_watcher};
use foco_store::{
    config::WorkspaceConfig,
    workspace::{WorkspaceDatabase, WorkspaceDatabaseError},
};

use crate::{AppResult, AppState};

#[derive(Default)]
pub(crate) struct CodeGraphIndexState {
    initializing: HashSet<PathBuf>,
    initialized: HashSet<PathBuf>,
    watchers: Vec<CodeGraphWatcher>,
}

impl CodeGraphIndexState {
    fn claim(&mut self, workspace_path: &Path) -> bool {
        let workspace_path = workspace_path.to_path_buf();
        if self.initialized.contains(&workspace_path) || self.initializing.contains(&workspace_path)
        {
            return false;
        }
        self.initializing.insert(workspace_path);
        true
    }

    fn complete(&mut self, workspace_path: &Path, watcher: CodeGraphWatcher) {
        self.initializing.remove(workspace_path);
        self.initialized.insert(workspace_path.to_path_buf());
        self.watchers.push(watcher);
    }

    fn fail(&mut self, workspace_path: &Path) {
        self.initializing.remove(workspace_path);
    }

    #[cfg(test)]
    pub(crate) fn watcher_count(&self) -> usize {
        self.watchers.len()
    }

    #[cfg(test)]
    pub(crate) fn clear_watchers(&mut self) {
        self.watchers.clear();
    }
}

pub(crate) fn recently_active_code_graph_workspaces(
    workspaces: &[WorkspaceConfig],
) -> Result<Vec<WorkspaceConfig>, WorkspaceDatabaseError> {
    let since = (Utc::now() - ChronoDuration::days(7)).to_rfc3339_opts(SecondsFormat::Millis, true);
    let mut active_workspaces = Vec::new();

    for workspace in workspaces {
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
    if !indexes
        .lock()
        .expect("code graph index lock poisoned")
        .claim(&workspace.path)
    {
        return;
    }

    let started_at = Instant::now();
    tracing::info!(
        workspace_id = %workspace.id,
        workspace_path = %workspace.path.display(),
        "background code graph workspace initialization started"
    );
    match initialize_code_graph_workspace(&workspace) {
        Ok(watcher) => {
            indexes
                .lock()
                .expect("code graph index lock poisoned")
                .complete(&workspace.path, watcher);
            tracing::info!(
                workspace_id = %workspace.id,
                workspace_path = %workspace.path.display(),
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                "background code graph workspace initialization completed"
            );
        }
        Err(error) => {
            indexes
                .lock()
                .expect("code graph index lock poisoned")
                .fail(&workspace.path);
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
    if !state
        .code_graph_indexes
        .lock()
        .expect("code graph index lock poisoned")
        .claim(&workspace.path)
    {
        return;
    }

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
                    indexes
                        .lock()
                        .expect("code graph index lock poisoned")
                        .complete(&workspace.path, watcher);
                    tracing::info!(
                        workspace_id = %workspace.id,
                        workspace_path = %workspace.path.display(),
                        elapsed_ms = started_at.elapsed().as_millis() as u64,
                        "lazy code graph workspace initialization completed"
                    );
                }
                Err(error) => {
                    indexes
                        .lock()
                        .expect("code graph index lock poisoned")
                        .fail(&workspace.path);
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
        state
            .code_graph_indexes
            .lock()
            .expect("code graph index lock poisoned")
            .fail(&workspace.path);
        tracing::error!(
            workspace_id = %workspace.id,
            workspace_path = %workspace.path.display(),
            error = %error,
            "failed to spawn lazy code graph initialization"
        );
    }
}

fn initialize_code_graph_workspace(workspace: &WorkspaceConfig) -> AppResult<CodeGraphWatcher> {
    let index_started_at = Instant::now();
    let report = index_workspace(&workspace.path)?;
    tracing::info!(
        workspace_id = %workspace.id,
        workspace_path = %workspace.path.display(),
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
    let watcher = start_code_graph_watcher(&workspace.path)?;
    tracing::info!(
        workspace_id = %workspace.id,
        workspace_path = %workspace.path.display(),
        elapsed_ms = watcher_started_at.elapsed().as_millis() as u64,
        "started code graph filesystem watcher"
    );

    Ok(watcher)
}
