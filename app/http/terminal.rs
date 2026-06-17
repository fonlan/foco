use std::path::Path;

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State, ws::WebSocketUpgrade},
    response::Response,
};
use foco_store::workspace::{NewTerminalSession, WorkspaceDatabase};
use serde::{Deserialize, Serialize};

use crate::{ApiError, AppState, config_snapshot, terminal, unique_id, workspace_by_id};

pub(crate) async fn create_terminal_session(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
) -> Result<Json<TerminalSessionResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let working_directory = terminal::shell_path(&workspace.path).display().to_string();

    if !Path::new(&working_directory).is_dir() {
        return Err(ApiError::bad_request(format!(
            "terminal working directory does not exist: {working_directory}"
        )));
    }

    let session_id = unique_id("terminal");
    database
        .upsert_terminal_session(NewTerminalSession {
            id: &session_id,
            name: "Workspace Terminal",
            working_directory: &working_directory,
            metadata_json: None,
        })
        .map_err(ApiError::from_workspace_error)?;

    Ok(Json(TerminalSessionResponse {
        id: session_id,
        name: "Workspace Terminal".to_string(),
        working_directory,
    }))
}

pub(crate) async fn terminal_socket(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    AxumPath((workspace_id, session_id)): AxumPath<(String, String)>,
    Query(query): Query<TerminalSocketQuery>,
) -> Result<Response, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let database = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?;
    let session = database
        .terminal_session(&session_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| {
            ApiError::bad_request(format!("terminal session was not found: {session_id}"))
        })?;

    if session.closed_at.is_some() {
        return Err(ApiError::bad_request(format!(
            "terminal session is closed: {session_id}"
        )));
    }

    let shutdown_rx = state.terminal_shutdown_tx.subscribe();
    let registry = state.terminal_registry.clone();
    let workspace_path = workspace.path.clone();
    let terminal_shell = workspace.terminal_shell.clone();

    Ok(ws.on_upgrade(move |socket| {
        terminal::handle_terminal_socket(
            socket,
            shutdown_rx,
            registry,
            workspace_path,
            terminal_shell,
            session,
            query.cols.unwrap_or(80),
            query.rows.unwrap_or(24),
        )
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalSocketQuery {
    cols: Option<u16>,
    rows: Option<u16>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalSessionResponse {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) working_directory: String,
}
