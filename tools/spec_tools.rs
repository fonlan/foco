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
    let current_spec = database.workspace_spec()?;
    let current_revision = current_spec.as_ref().map_or(0, |spec| spec.revision);
    if current_revision != request.expected_revision {
        return Err(revision_conflict());
    }

    let current_content = current_spec
        .as_ref()
        .map_or("", |spec| spec.content_markdown.as_str());
    let line_count_before = markdown_line_count(current_content);
    let (content_markdown, update_mode, edit_count) =
        match (request.edits, request.content_markdown) {
            (Some(edits), None) => {
                let content_markdown = apply_spec_edits(current_content, &edits)?;
                (content_markdown, "patch", edits.len())
            }
            (None, Some(content_markdown)) => (content_markdown, "fullReplacement", 0),
            (Some(_), Some(_)) => {
                return Err(ToolRuntimeError::InvalidArguments(
                    "provide exactly one of edits or contentMarkdown; both were provided"
                        .to_string(),
                ));
            }
            (None, None) => {
                return Err(ToolRuntimeError::InvalidArguments(
                    "provide exactly one of edits or contentMarkdown; neither was provided"
                        .to_string(),
                ));
            }
        };
    let line_count_after = markdown_line_count(&content_markdown);
    let spec = database
        .update_workspace_spec_content(request.expected_revision, &content_markdown)?
        .ok_or_else(revision_conflict)?;

    let mut output = spec_json(spec, timeout_ms);
    output["updateMode"] = json!(update_mode);
    output["editCount"] = json!(edit_count);
    output["lineCountBefore"] = json!(line_count_before);
    output["lineCountAfter"] = json!(line_count_after);

    Ok(output)
}

fn apply_spec_edits(
    current_content: &str,
    edits: &[SpecTextEdit],
) -> Result<String, ToolRuntimeError> {
    if edits.is_empty() {
        return Err(ToolRuntimeError::InvalidArguments(
            "edits must contain at least one edit".to_string(),
        ));
    }

    let mut content = current_content.to_string();
    for (index, edit) in edits.iter().enumerate() {
        if edit.old_text.is_empty() {
            return Err(ToolRuntimeError::InvalidArguments(format!(
                "edits[{index}].oldText must not be empty"
            )));
        }

        let mut matches = content.char_indices().filter_map(|(match_start, _)| {
            content[match_start..]
                .starts_with(&edit.old_text)
                .then_some(match_start)
        });
        let Some(match_start) = matches.next() else {
            return Err(ToolRuntimeError::InvalidArguments(format!(
                "edits[{index}].oldText was not found in the current Project Spec"
            )));
        };
        if matches.next().is_some() {
            return Err(ToolRuntimeError::InvalidArguments(format!(
                "edits[{index}].oldText matched more than once in the current Project Spec"
            )));
        }

        let match_end = match_start + edit.old_text.len();
        content.replace_range(match_start..match_end, &edit.new_text);
    }

    if content == current_content {
        return Err(ToolRuntimeError::InvalidArguments(
            "edits must change the Project Spec content".to_string(),
        ));
    }

    Ok(content)
}

fn markdown_line_count(content: &str) -> usize {
    if content.is_empty() {
        0
    } else {
        content.bytes().filter(|byte| *byte == b'\n').count() + 1
    }
}

fn revision_conflict() -> ToolRuntimeError {
    ToolRuntimeError::InvalidArguments(
        "workspace spec revision changed; call read_spec again before update_spec".to_string(),
    )
}

fn open_spec_database(workspace_path: &Path) -> Result<foco_store::workspace::WorkspaceDatabaseHandle, ToolRuntimeError> {
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
    #[serde(default)]
    content_markdown: Option<String>,
    #[serde(default)]
    edits: Option<Vec<SpecTextEdit>>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpecTextEdit {
    old_text: String,
    new_text: String,
}
