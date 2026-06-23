use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
};
use foco_store::{
    config::{
        SUPPORTED_HOOK_EVENTS, UNSUPPORTED_HOOK_EVENTS, save_workspace_hook_config,
        workspace_hook_config_path,
    },
    workspace::WorkspaceDatabase,
};
use serde_json::json;

use crate::hooks::HookRunRequest;
use crate::*;

pub(crate) async fn hooks_settings(
    State(state): State<AppState>,
    Query(query): Query<HooksQuery>,
) -> Result<Json<HooksSettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    hooks_settings_response(&state, &config, query.workspace_id.as_deref()).await
}

pub(crate) async fn save_global_hooks(
    State(state): State<AppState>,
    Json(request): Json<SaveGlobalHooksRequest>,
) -> Result<Json<HooksSettingsResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let audit_enabled = config.hooks.audit_enabled;

    config.hooks = request.config;
    config.hooks.audit_enabled = audit_enabled;
    config
        .validate(Some(&state.config_file))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    save_config(&state, config.clone())?;

    hooks_settings_response(&state, &config, None).await
}

pub(crate) async fn save_workspace_hooks(
    State(state): State<AppState>,
    Json(request): Json<SaveWorkspaceHooksRequest>,
) -> Result<Json<HooksSettingsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &request.workspace_id)?.clone();
    let mut validation_config = config.clone();

    validation_config.hooks = request.config.clone();
    validation_config
        .validate(Some(&workspace_hook_config_path(&workspace.path)))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    save_workspace_hook_config(&workspace.path, &request.config)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    hooks_settings_response(&state, &config, Some(&workspace.id)).await
}

pub(crate) async fn import_claude_hooks(
    State(state): State<AppState>,
    Json(request): Json<ImportClaudeHooksRequest>,
) -> Result<Json<ImportClaudeHooksResponse>, ApiError> {
    let mut config = config_snapshot(&state)?;
    let target = request.target.trim();
    if target != "global" && target != "workspace" {
        return Err(ApiError::bad_request(
            "hook import target must be 'global' or 'workspace'",
        ));
    }
    let workspace = if target == "workspace" {
        let workspace_id = request.workspace_id.as_deref().ok_or_else(|| {
            ApiError::bad_request("workspaceId is required for workspace hook import")
        })?;
        Some(workspace_by_id(&config, workspace_id)?.clone())
    } else {
        None
    };

    let (import_source, save_path, source_paths) = if target == "global" {
        (
            "global",
            state.config_file.clone(),
            claude_hook_settings_paths(&state.user_profile_dir),
        )
    } else {
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| ApiError::internal("workspace hook import lost selected workspace"))?;
        (
            "workspace",
            workspace_hook_config_path(&workspace.path),
            claude_hook_settings_paths(&workspace.path),
        )
    };
    let (mut imported, imported_files, mut validation_errors) =
        import_claude_hook_config(&source_paths)?;

    if imported_files.is_empty() {
        validation_errors.push(format!(
            "no Claude hook settings were found under {}",
            source_paths
                .first()
                .and_then(|path| path.parent())
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| ".claude".to_string())
        ));
    }

    if validation_errors.is_empty() {
        let mut validation_config = config.clone();
        if target == "global" {
            imported.audit_enabled = config.hooks.audit_enabled;
        }
        validation_config.hooks = imported.clone();
        if let Err(error) = validation_config.validate(Some(&save_path)) {
            validation_errors.push(error.to_string());
        }
    }

    if !validation_errors.is_empty() {
        return Ok(Json(ImportClaudeHooksResponse {
            saved: false,
            target: import_source.to_string(),
            path: display_path(&save_path),
            imported_files,
            validation_errors,
            config: imported,
        }));
    }

    if target == "global" {
        config.hooks = imported.clone();
        save_config(&state, config)?;
    } else {
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| ApiError::internal("workspace hook import lost selected workspace"))?;
        save_workspace_hook_config(&workspace.path, &imported)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }

    Ok(Json(ImportClaudeHooksResponse {
        saved: true,
        target: import_source.to_string(),
        path: display_path(&save_path),
        imported_files,
        validation_errors,
        config: imported,
    }))
}

pub(crate) async fn test_hooks(
    State(state): State<AppState>,
    Json(request): Json<TestHookRequest>,
) -> Result<Json<HookRunSummary>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &request.workspace_id)?;
    let event = request.event.trim();

    if UNSUPPORTED_HOOK_EVENTS.contains(&event) {
        return Err(ApiError::bad_request(format!(
            "{event} is a Claude Code hook event that Foco does not support yet"
        )));
    }
    if !SUPPORTED_HOOK_EVENTS.contains(&event) {
        return Err(ApiError::bad_request(format!(
            "{event} is unsupported; expected one of {}",
            SUPPORTED_HOOK_EVENTS.join(", ")
        )));
    }

    let provider = default_hook_provider(&config).transpose()?;
    let summary = state
        .hook_runtime
        .run_hooks(HookRunRequest {
            global_config: &config.hooks,
            api_audit_save_details: api_audit_save_details(&config),
            workspace_id: &workspace.id,
            workspace_path: &workspace.path,
            event,
            match_value: optional_trimmed_string(request.match_value),
            chat_id: None,
            run_id: None,
            session_id: None,
            tool_call_id: None,
            model_id: provider.as_ref().map(|provider| provider.0.as_str()),
            provider_id: provider.as_ref().map(|provider| provider.1.as_str()),
            provider_config: provider.as_ref().map(|provider| &provider.2),
            llm_request_retry_count: config.app.llm_request_retry_count,
            permission_mode: None,
            payload: request.payload.unwrap_or_else(|| json!({})),
        })
        .await;

    Ok(Json(summary))
}

pub(crate) async fn hook_run_detail(
    State(state): State<AppState>,
    AxumPath((workspace_id, hook_run_id)): AxumPath<(String, String)>,
) -> Result<Json<HookRunDetailResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let hook_run_id = hook_run_id.trim();

    if hook_run_id.is_empty() {
        return Err(ApiError::bad_request("hook run id must not be empty"));
    }

    let record = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?
        .hook_run(hook_run_id)
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request(format!("hook run was not found: {hook_run_id}")))?;

    if record.workspace_id != workspace.id {
        return Err(ApiError::bad_request(format!(
            "hook run '{}' does not belong to workspace '{}'",
            record.id, workspace.id
        )));
    }

    Ok(Json(HookRunDetailResponse {
        run: hook_run_detail_from_record(record)?,
    }))
}

pub(crate) async fn hook_runs(
    State(state): State<AppState>,
    AxumPath(workspace_id): AxumPath<String>,
    Query(query): Query<HookRunsQuery>,
) -> Result<Json<HookRunsResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let workspace = workspace_by_id(&config, &workspace_id)?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?
        .hook_runs(limit)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .filter(|record| record.workspace_id == workspace.id)
        .map(hook_run_summary_row)
        .collect();

    Ok(Json(HookRunsResponse { runs }))
}
