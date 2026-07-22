use std::path::Path;

use foco_store::workspace::{WorkspaceDatabase, WorkspaceSpecRecord};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::output_budget::{
    CompleteLinePrefix, CompleteLineTruncateOptions, CompleteLineTruncateOutcome,
    LINE_BOUNDED_LAST_RETURNED_LINE_FIELD, LINE_BOUNDED_NEXT_START_LINE_FIELD,
    LINE_BOUNDED_NOTE_FIELD, LINE_BOUNDED_RETURNED_LINES_FIELD,
    LINE_BOUNDED_SOFT_BUDGET_EXCEEDED_FIELD, LINE_BOUNDED_TRUNCATED_FIELD,
    TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES, TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT,
    TOOL_OUTPUT_SOFT_BYTE_LIMIT, TOOL_OUTPUT_SOFT_LINE_LIMIT, complete_line_count,
    complete_line_prefix_note, for_each_complete_line, measure_tool_execution,
    peel_last_complete_line, serialized_json_size, truncate_to_complete_lines_with_measure,
};
use crate::{
    DEFAULT_SPEC_TOOL_TIMEOUT_MS, ToolExecution,
    errors::{ToolRuntimeError, tool_timeout_ms},
    parse_arguments,
    spec_patch::{SpecPatchError, SpecTextEdit, apply_spec_text_edits},
};

pub(crate) fn read_spec(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: ReadSpecInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_SPEC_TOOL_TIMEOUT_MS)?;
    let database = open_spec_database(workspace_path)?;
    let spec = database.workspace_spec()?;
    let (record, content) = match spec {
        Some(record) => {
            let content = record.content_markdown.clone();
            (Some(record), content)
        }
        None => (None, String::new()),
    };

    let revision = record.as_ref().map_or(0, |spec| spec.revision);
    let start_line = request.start_line;
    if let Some(start) = start_line {
        let expected = request.expected_revision.ok_or_else(|| {
            ToolRuntimeError::InvalidArguments(
                "read_spec continuation requires expectedRevision from the first page; startLine is set but expectedRevision is null"
                    .to_string(),
            )
        })?;
        if expected != revision {
            return Err(read_revision_conflict(expected, revision));
        }
        if start == 0 {
            return Err(ToolRuntimeError::InvalidArguments(
                "startLine must be a positive 1-based line number".to_string(),
            ));
        }
    } else if let Some(expected) = request.expected_revision
        && expected != revision
    {
        // Optional first-page revision pin: reject if the caller supplied a stale revision.
        return Err(read_revision_conflict(expected, revision));
    }

    let content_start_line = start_line.unwrap_or(1).max(1);
    let total_lines = complete_line_count(&content);
    let total_bytes = content.len();
    let page_content = slice_content_from_line(&content, content_start_line, total_lines)?;

    let mut response = match &record {
        Some(spec) => base_spec_fields(spec, timeout_ms),
        None => default_spec_fields(timeout_ms),
    };
    response["totalLines"] = json!(total_lines);
    response["totalBytes"] = json!(total_bytes);
    response["startLine"] = json!(content_start_line);

    build_read_spec_page_response(response, &page_content, content_start_line)
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
    let line_count_before = complete_line_count(current_content);
    let content_markdown = request
        .content_markdown
        .filter(|value| !value.trim().is_empty());
    let (content_markdown, update_mode, edit_count) = match (request.edits, content_markdown) {
        (Some(edits), None) => {
            let content_markdown =
                apply_spec_text_edits(current_content, &edits).map_err(map_patch_error)?;
            (content_markdown, "patch", edits.len())
        }
        (None, Some(content_markdown)) => (content_markdown, "fullReplacement", 0),
        (Some(_), Some(_)) => {
            return Err(ToolRuntimeError::InvalidArguments(
                "provide exactly one of edits or contentMarkdown; both were provided".to_string(),
            ));
        }
        (None, None) => {
            return Err(ToolRuntimeError::InvalidArguments(
                "provide exactly one of edits or contentMarkdown; neither was provided".to_string(),
            ));
        }
    };
    let line_count_after = complete_line_count(&content_markdown);
    let total_lines = line_count_after;
    let total_bytes = content_markdown.len();
    let spec = database
        .update_workspace_spec_content(request.expected_revision, &content_markdown)?
        .ok_or_else(revision_conflict)?;

    let mut output = base_spec_fields(&spec, timeout_ms);
    output["contentMarkdown"] = json!(content_markdown);
    output["updateMode"] = json!(update_mode);
    output["editCount"] = json!(edit_count);
    output["lineCountBefore"] = json!(line_count_before);
    output["lineCountAfter"] = json!(line_count_after);
    output["totalLines"] = json!(total_lines);
    output["totalBytes"] = json!(total_bytes);
    output["contentOmitted"] = json!(false);

    fit_update_spec_success_response(output)
}

fn map_patch_error(error: SpecPatchError) -> ToolRuntimeError {
    ToolRuntimeError::InvalidArguments(error.message())
}

fn revision_conflict() -> ToolRuntimeError {
    ToolRuntimeError::InvalidArguments(
        "workspace spec revision changed; call read_spec again before update_spec".to_string(),
    )
}

fn read_revision_conflict(expected: u64, current: u64) -> ToolRuntimeError {
    ToolRuntimeError::InvalidArguments(format!(
        "workspace spec revision changed during read_spec continuation (expectedRevision={expected}, currentRevision={current}); restart from the first page without startLine"
    ))
}

fn open_spec_database(
    workspace_path: &Path,
) -> Result<foco_store::workspace::WorkspaceDatabaseHandle, ToolRuntimeError> {
    WorkspaceDatabase::open_or_create(workspace_path).map_err(ToolRuntimeError::WorkspaceDatabase)
}

fn base_spec_fields(spec: &WorkspaceSpecRecord, timeout_ms: u64) -> Value {
    json!({
        "enabled": spec.enabled,
        "injectEnabled": spec.inject_enabled,
        "revision": spec.revision,
        "generatedAt": spec.generated_at,
        "updatedAt": spec.updated_at,
        "timeoutMs": timeout_ms
    })
}

fn default_spec_fields(timeout_ms: u64) -> Value {
    json!({
        "enabled": false,
        "injectEnabled": false,
        "revision": 0,
        "generatedAt": null,
        "updatedAt": null,
        "timeoutMs": timeout_ms
    })
}

/// Slice markdown from a 1-based complete-line start through EOF.
///
/// `start_line > total_lines` yields an empty final page (recoverable EOF continuation).
fn slice_content_from_line(
    content: &str,
    start_line: usize,
    total_lines: usize,
) -> Result<&str, ToolRuntimeError> {
    if content.is_empty() || start_line > total_lines {
        return Ok("");
    }
    if start_line == 1 {
        return Ok(content);
    }

    let mut start_byte = None;
    let mut cursor = 0_usize;
    for_each_complete_line(content, |offset, line| {
        let line_no = offset.saturating_add(1);
        if line_no == start_line {
            start_byte = Some(cursor);
            return false;
        }
        cursor = cursor.saturating_add(line.len());
        true
    });

    match start_byte {
        Some(byte) if byte <= content.len() && content.is_char_boundary(byte) => {
            Ok(&content[byte..])
        }
        Some(_) => Err(ToolRuntimeError::InvalidArguments(
            "read_spec startLine does not land on a UTF-8 character boundary".to_string(),
        )),
        None => Ok(""),
    }
}

fn json_string_body_len(s: &str) -> usize {
    let mut len = 0_usize;
    for byte in s.bytes() {
        len = len.saturating_add(match byte {
            b'"' | b'\\' | b'\n' | b'\r' | b'\t' => 2,
            0x00..=0x1F => 6, // \u00XX
            _ => 1,
        });
    }
    len
}

fn content_markdown_line_measure(_line_no: usize, line: &str) -> usize {
    json_string_body_len(line)
}

fn measure_read_spec_response_overhead(
    skeleton_base: &Value,
    content_start_line: usize,
) -> Result<usize, ToolRuntimeError> {
    let last = content_start_line
        .saturating_add(TOOL_OUTPUT_SOFT_LINE_LIMIT)
        .saturating_sub(1)
        .max(1);
    let next = last.saturating_add(1);
    let note = complete_line_prefix_note(&CompleteLinePrefix {
        text: String::new(),
        returned_lines: TOOL_OUTPUT_SOFT_LINE_LIMIT,
        last_returned_line: last,
        next_start_line: Some(next),
        truncated: true,
        soft_budget_exceeded: false,
    });
    let mut skeleton = skeleton_base.clone();
    if let Some(object) = skeleton.as_object_mut() {
        object.insert("contentMarkdown".to_string(), json!(""));
        object.insert(LINE_BOUNDED_TRUNCATED_FIELD.to_string(), json!(true));
        object.insert(
            LINE_BOUNDED_RETURNED_LINES_FIELD.to_string(),
            json!(TOOL_OUTPUT_SOFT_LINE_LIMIT),
        );
        object.insert(
            LINE_BOUNDED_LAST_RETURNED_LINE_FIELD.to_string(),
            json!(last),
        );
        object.insert(LINE_BOUNDED_NEXT_START_LINE_FIELD.to_string(), json!(next));
        object.insert(LINE_BOUNDED_NOTE_FIELD.to_string(), json!(note));
    }
    let execution = ToolExecution {
        output: skeleton,
        is_error: false,
    };
    serialized_json_size(&execution).map_err(|error| {
        ToolRuntimeError::InvalidArguments(format!(
            "read_spec: failed to measure response budget ({error})"
        ))
    })
}

fn assemble_read_spec_response(
    mut base: Value,
    content_markdown: &str,
    prefix: &CompleteLinePrefix,
    content_start_line: usize,
) -> Value {
    base["contentMarkdown"] = json!(content_markdown);
    base["startLine"] = json!(content_start_line);
    base[LINE_BOUNDED_TRUNCATED_FIELD] = json!(prefix.truncated);
    base[LINE_BOUNDED_RETURNED_LINES_FIELD] = json!(prefix.returned_lines);
    base[LINE_BOUNDED_LAST_RETURNED_LINE_FIELD] = json!(prefix.last_returned_line);
    if let Some(next) = prefix.next_start_line {
        base[LINE_BOUNDED_NEXT_START_LINE_FIELD] = json!(next);
    } else {
        base[LINE_BOUNDED_NEXT_START_LINE_FIELD] = Value::Null;
    }
    if prefix.soft_budget_exceeded {
        base[LINE_BOUNDED_SOFT_BUDGET_EXCEEDED_FIELD] = json!(true);
    }
    if prefix.truncated || prefix.soft_budget_exceeded {
        let mut note = complete_line_prefix_note(prefix);
        if prefix.truncated {
            note.push_str(
                " Continue with the same expectedRevision as this page's revision and startLine=nextStartLine.",
            );
        }
        base[LINE_BOUNDED_NOTE_FIELD] = json!(note);
    }
    base
}

fn build_read_spec_page_response(
    base: Value,
    page_content: &str,
    content_start_line: usize,
) -> Result<Value, ToolRuntimeError> {
    if page_content.is_empty() {
        let prefix = CompleteLinePrefix {
            text: String::new(),
            returned_lines: 0,
            last_returned_line: content_start_line,
            next_start_line: None,
            truncated: false,
            soft_budget_exceeded: false,
        };
        return Ok(assemble_read_spec_response(
            base,
            "",
            &prefix,
            content_start_line,
        ));
    }

    let overhead = measure_read_spec_response_overhead(&base, content_start_line)?;
    // Leave room for the outer SSE ToolResult envelope so a full (non-truncated) page is not
    // later rewritten into a read-only soft-limit failure by the runtime normalizer.
    let soft_byte_limit = TOOL_OUTPUT_SOFT_BYTE_LIMIT
        .saturating_sub(overhead)
        .saturating_sub(TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES);
    let hard_byte_limit = TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT.saturating_sub(overhead);
    let metadata_line_reserve = 4_usize; // note + metadata strings
    let soft_line_limit = TOOL_OUTPUT_SOFT_LINE_LIMIT
        .saturating_sub(metadata_line_reserve)
        .max(1);

    let options = CompleteLineTruncateOptions {
        soft_byte_limit,
        soft_line_limit,
        hard_byte_limit: hard_byte_limit.max(1),
        content_start_line,
    };
    let outcome = truncate_to_complete_lines_with_measure(
        page_content,
        options,
        content_markdown_line_measure,
    );

    match outcome {
        CompleteLineTruncateOutcome::SingleLineExceedsHardLimit {
            line_number,
            line_bytes,
            hard_limit_bytes,
        } => Err(ToolRuntimeError::InvalidArguments(format!(
            "read_spec: a single complete line (line {line_number}, {line_bytes} UTF-8 bytes in JSON string body) exceeds the hard output ceiling ({hard_limit_bytes} bytes) and cannot be returned without splitting the line"
        ))),
        CompleteLineTruncateOutcome::Full(prefix)
        | CompleteLineTruncateOutcome::Truncated(prefix) => {
            fit_read_spec_response_to_envelope(base, page_content, content_start_line, prefix)
        }
    }
}

fn fit_read_spec_response_to_envelope(
    base: Value,
    _full_page_content: &str,
    content_start_line: usize,
    mut prefix: CompleteLinePrefix,
) -> Result<Value, ToolRuntimeError> {
    for _ in 0..=TOOL_OUTPUT_SOFT_LINE_LIMIT.saturating_add(2) {
        let response =
            assemble_read_spec_response(base.clone(), &prefix.text, &prefix, content_start_line);
        let execution = ToolExecution {
            output: response.clone(),
            is_error: false,
        };
        let measurement = measure_tool_execution(&execution).map_err(|error| {
            ToolRuntimeError::InvalidArguments(format!(
                "read_spec: failed to measure response ({error})"
            ))
        })?;

        let within_soft = measurement.serialized_bytes
            <= TOOL_OUTPUT_SOFT_BYTE_LIMIT.saturating_sub(TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES)
            && measurement.text_lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT;
        if within_soft {
            return Ok(response);
        }

        if measurement.serialized_bytes > TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT {
            if prefix.returned_lines <= 1 {
                return Err(ToolRuntimeError::InvalidArguments(format!(
                    "read_spec: a single complete line exceeds the hard output ceiling ({} bytes measured; max {}) and cannot be returned without splitting the line",
                    measurement.serialized_bytes, TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
                )));
            }
        } else if prefix.returned_lines <= 1 {
            if prefix.truncated {
                return Ok(response);
            }
            if !prefix.soft_budget_exceeded {
                prefix.soft_budget_exceeded = true;
                prefix.next_start_line = None;
                continue;
            }
            return Ok(response);
        }

        let Some(peeled) = peel_last_complete_line(&prefix.text) else {
            return Ok(response);
        };
        if peeled.len() == prefix.text.len() {
            return Ok(response);
        }
        prefix.text = peeled.to_string();
        prefix.returned_lines = prefix.returned_lines.saturating_sub(1);
        prefix.last_returned_line = prefix
            .last_returned_line
            .saturating_sub(1)
            .max(content_start_line);
        prefix.truncated = true;
        prefix.soft_budget_exceeded = false;
        prefix.next_start_line = Some(prefix.last_returned_line.saturating_add(1));
        if prefix.returned_lines == 0 {
            return Err(ToolRuntimeError::InvalidArguments(
                "read_spec: unable to fit any complete line under the shared soft output budget after accounting for metadata"
                    .to_string(),
            ));
        }
    }

    Err(ToolRuntimeError::InvalidArguments(
        "read_spec: unable to fit response under the shared soft output budget".to_string(),
    ))
}

/// Keep critical update metadata when the full success body would exceed the soft budget.
///
/// Large successful writes must not be rewritten by the shared normalizer into a retry-unsafe
/// omission that drops `revision` / `updateMode` / line counts.
///
/// Soft-budget checks reserve [`TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES`] so the later SSE
/// `ToolResult` envelope (assistant/call ids + timestamps) still fits under the shared soft limit
/// without a second generic omission pass.
fn fit_update_spec_success_response(mut output: Value) -> Result<Value, ToolRuntimeError> {
    let execution = ToolExecution {
        output: output.clone(),
        is_error: false,
    };
    let measurement = measure_tool_execution(&execution).map_err(|error| {
        ToolRuntimeError::InvalidArguments(format!(
            "update_spec: failed to measure response ({error})"
        ))
    })?;
    // Runtime re-measures the complete SSE ToolResult envelope after tool execution. Keep bare
    // ToolExecution under soft minus envelope reserve so borderline full bodies omit content here
    // (preserving revision/updateMode) instead of losing metadata in the outer normalizer.
    let soft_byte_ceiling =
        TOOL_OUTPUT_SOFT_BYTE_LIMIT.saturating_sub(TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES);
    let within_soft = measurement.serialized_bytes <= soft_byte_ceiling
        && measurement.text_lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT;
    if within_soft {
        return Ok(output);
    }

    // Drop the full body; keep CAS and update metadata so the model does not retry the write.
    if let Some(object) = output.as_object_mut() {
        object.remove("contentMarkdown");
        object.insert("contentOmitted".to_string(), json!(true));
        object.insert(
            "note".to_string(),
            json!(
                "update_spec succeeded; contentMarkdown was omitted because the full body exceeds the shared soft output budget (~50KiB / 2,000 lines). Use read_spec with this revision (and startLine continuation when truncated) to load the stored markdown. Do not retry the same update_spec call."
            ),
        );
        object.insert(LINE_BOUNDED_TRUNCATED_FIELD.to_string(), json!(false));
    }

    let omitted_execution = ToolExecution {
        output: output.clone(),
        is_error: false,
    };
    let omitted_measurement = measure_tool_execution(&omitted_execution).map_err(|error| {
        ToolRuntimeError::InvalidArguments(format!(
            "update_spec: failed to measure omitted response ({error})"
        ))
    })?;
    // Metadata-only success should fit under soft minus envelope reserve; hard ceiling is last resort.
    if omitted_measurement.serialized_bytes > TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "update_spec: success metadata exceeds the hard output ceiling ({} bytes; max {})",
            omitted_measurement.serialized_bytes, TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
        )));
    }
    debug_assert!(
        omitted_measurement.serialized_bytes <= soft_byte_ceiling
            && omitted_measurement.text_lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT,
        "update_spec omitted success should fit soft minus envelope reserve"
    );

    Ok(output)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadSpecInput {
    #[serde(default)]
    start_line: Option<usize>,
    #[serde(default)]
    expected_revision: Option<u64>,
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
