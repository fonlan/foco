use std::path::Path;

use foco_store::workspace::{WorkspaceDatabase, WorkspaceSpecRecord};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    DEFAULT_SPEC_TOOL_TIMEOUT_MS,
    errors::{ToolRuntimeError, tool_timeout_ms},
    parse_arguments,
};

pub(crate) fn read_spec(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: ReadSpecInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_SPEC_TOOL_TIMEOUT_MS)?;
    let database = open_spec_database(workspace_path)?;
    let spec = database
        .workspace_spec()?
        .map(|spec| spec_json(spec, timeout_ms))
        .unwrap_or_else(|| default_spec_json(timeout_ms));

    Ok(spec)
}

pub(crate) fn update_spec(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: UpdateSpecInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_SPEC_TOOL_TIMEOUT_MS)?;
    let mut database = open_spec_database(workspace_path)?;
    let spec = database
        .update_workspace_spec_content(request.expected_revision, &request.content_markdown)?
        .ok_or_else(|| {
            ToolRuntimeError::InvalidArguments(
                "workspace spec revision changed; call read_spec again before update_spec"
                    .to_string(),
            )
        })?;

    Ok(spec_json(spec, timeout_ms))
}

fn open_spec_database(workspace_path: &Path) -> Result<WorkspaceDatabase, ToolRuntimeError> {
    WorkspaceDatabase::open_or_create(workspace_path).map_err(ToolRuntimeError::WorkspaceDatabase)
}

fn spec_json(spec: WorkspaceSpecRecord, timeout_ms: u64) -> Value {
    json!({
        "enabled": spec.enabled,
        "injectEnabled": spec.inject_enabled,
        "revision": spec.revision,
        "contentMarkdown": spec.content_markdown,
        "generatedAt": spec.generated_at,
        "updatedAt": spec.updated_at,
        "timeoutMs": timeout_ms
    })
}

fn default_spec_json(timeout_ms: u64) -> Value {
    json!({
        "enabled": false,
        "injectEnabled": false,
        "revision": 0,
        "contentMarkdown": "",
        "generatedAt": null,
        "updatedAt": null,
        "timeoutMs": timeout_ms
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadSpecInput {
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateSpecInput {
    expected_revision: u64,
    content_markdown: String,
    timeout_ms: Option<u64>,
}
