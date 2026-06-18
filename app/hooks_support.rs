use std::{
    fs,
    path::{Path, PathBuf},
};

use axum::Json;
use foco_providers::ProviderConnectionConfig;
use foco_store::{
    config::{
        GlobalConfig, HookConfig, HookEventMap, SUPPORTED_HOOK_EVENTS, UNSUPPORTED_HOOK_EVENTS,
        load_workspace_hook_config, workspace_hook_config_path,
    },
    workspace::{HookRunRecord, WorkspaceDatabase},
};
use serde_json::Value;

use crate::hooks::effective_hook_summaries;
use crate::*;

pub(crate) async fn hooks_settings_response(
    state: &AppState,
    config: &GlobalConfig,
    workspace_id: Option<&str>,
) -> Result<Json<HooksSettingsResponse>, ApiError> {
    let workspace = selected_hooks_workspace(config, workspace_id)?;
    let workspace_config = load_workspace_hook_config(&workspace.path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let effective = effective_hook_summaries(&config.hooks, &workspace.path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let recent_runs = WorkspaceDatabase::open_or_create(&workspace.path)
        .map_err(ApiError::from_workspace_error)?
        .hook_runs(50)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(hook_run_summary_row)
        .collect();

    Ok(Json(HooksSettingsResponse {
        supported_events: SUPPORTED_HOOK_EVENTS.to_vec(),
        unsupported_events: UNSUPPORTED_HOOK_EVENTS.to_vec(),
        global: HookConfigScopeSummary {
            source: "global".to_string(),
            path: display_path(&state.config_file),
            workspace_id: None,
            config: config.hooks.clone(),
        },
        workspace: HookConfigScopeSummary {
            source: "workspace".to_string(),
            path: display_path(&workspace_hook_config_path(&workspace.path)),
            workspace_id: Some(workspace.id.clone()),
            config: workspace_config,
        },
        effective,
        recent_runs,
    }))
}

pub(crate) fn selected_hooks_workspace<'a>(
    config: &'a GlobalConfig,
    workspace_id: Option<&str>,
) -> Result<&'a WorkspaceConfig, ApiError> {
    match workspace_id.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(workspace_id) => workspace_by_id(config, workspace_id),
        None => workspace_by_id(config, &config.app.active_workspace_id),
    }
}

pub(crate) fn hook_run_summary_row(record: HookRunRecord) -> HookRunSummaryRow {
    HookRunSummaryRow {
        id: record.id,
        workspace_id: record.workspace_id,
        chat_id: record.chat_id,
        run_id: record.run_id,
        tool_call_id: record.tool_call_id,
        event: record.event,
        hook_source: record.hook_source,
        handler_type: record.handler_type,
        status: record.status,
        exit_code: record.exit_code,
        stdout_preview: record.stdout_preview,
        stderr_preview: record.stderr_preview,
        started_at: record.started_at,
        completed_at: record.completed_at,
    }
}

pub(crate) fn hook_run_detail_from_record(
    record: HookRunRecord,
) -> Result<HookRunDetail, ApiError> {
    let input = parse_json_value(&record.input_json, "hook run input")?;
    let output = record
        .output_json
        .as_deref()
        .map(|json| parse_json_value(json, "hook run output"))
        .transpose()?;

    Ok(HookRunDetail {
        id: record.id,
        workspace_id: record.workspace_id,
        chat_id: record.chat_id,
        run_id: record.run_id,
        tool_call_id: record.tool_call_id,
        event: record.event,
        hook_source: record.hook_source,
        handler_type: record.handler_type,
        input,
        output,
        status: record.status,
        exit_code: record.exit_code,
        stdout_preview: record.stdout_preview,
        stderr_preview: record.stderr_preview,
        started_at: record.started_at,
        completed_at: record.completed_at,
    })
}

pub(crate) fn claude_hook_settings_paths(base_path: &Path) -> Vec<PathBuf> {
    vec![
        base_path.join(".claude").join("settings.json"),
        base_path.join(".claude").join("settings.local.json"),
    ]
}

pub(crate) fn import_claude_hook_config(
    paths: &[PathBuf],
) -> Result<(HookConfig, Vec<String>, Vec<String>), ApiError> {
    let mut config = HookConfig::default();
    let mut imported_files = Vec::new();
    let mut validation_errors = Vec::new();

    for path in paths {
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(|source| {
            ApiError::internal(format!("failed to read {}: {source}", path.display()))
        })?;
        let value = serde_json::from_str::<Value>(&content).map_err(|source| {
            ApiError::bad_request(format!("failed to parse {}: {source}", path.display()))
        })?;
        let Some(imported) = hook_config_from_claude_settings(&value).map_err(|message| {
            ApiError::bad_request(format!("failed to import {}: {message}", path.display()))
        })?
        else {
            continue;
        };

        imported_files.push(display_path(&path));
        config.disable_all_hooks = imported.disable_all_hooks;
        merge_hook_event_maps(&mut config.hooks, imported.hooks);
    }

    for event in config.hooks.keys() {
        if UNSUPPORTED_HOOK_EVENTS.contains(&event.as_str()) {
            validation_errors.push(format!(
                "{event} is a Claude Code hook event that Foco does not support yet"
            ));
        } else if !SUPPORTED_HOOK_EVENTS.contains(&event.as_str()) {
            validation_errors.push(format!("{event} is not a supported Foco hook event"));
        }
    }

    Ok((config, imported_files, validation_errors))
}

pub(crate) fn hook_config_from_claude_settings(
    value: &Value,
) -> Result<Option<HookConfig>, String> {
    let mut config = HookConfig::default();
    let mut found = false;

    if let Some(disable_all_hooks) = value.get("disableAllHooks") {
        config.disable_all_hooks = disable_all_hooks
            .as_bool()
            .ok_or_else(|| "disableAllHooks must be a boolean".to_string())?;
        found = true;
    }

    if let Some(hooks) = value.get("hooks") {
        config.hooks = serde_json::from_value::<HookEventMap>(hooks.clone())
            .map_err(|source| format!("hooks do not match Foco hook shape: {source}"))?;
        found = true;
    }

    Ok(found.then_some(config))
}

pub(crate) fn merge_hook_event_maps(target: &mut HookEventMap, imported: HookEventMap) {
    for (event, mut groups) in imported {
        target.entry(event).or_default().append(&mut groups);
    }
}

pub(crate) fn default_hook_provider(
    config: &GlobalConfig,
) -> Option<Result<(String, String, ProviderConnectionConfig), ApiError>> {
    let model = config
        .models
        .iter()
        .find(|model| model.enabled && model.active_provider_id.is_some())?;
    let provider_id = model.active_provider_id.as_deref()?;
    let provider = match config.providers.iter().find(|provider| {
        provider.id == provider_id
            && provider.enabled
            && model.provider_ids.iter().any(|id| id == provider_id)
    }) {
        Some(provider) => provider,
        None => return None,
    };

    Some(
        provider_connection_config(provider)
            .map(|provider_config| (model.id.clone(), provider.id.clone(), provider_config)),
    )
}
