use std::io::{self, Write};
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};

use crate::ToolExecution;

pub const TOOL_OUTPUT_SOFT_BYTE_LIMIT: usize = 50 * 1024;
pub const TOOL_OUTPUT_SOFT_LINE_LIMIT: usize = 2_000;
pub const TOOL_EXECUTION_HARD_BYTE_LIMIT: usize = 128 * 1024;
/// Single `SKILL.md` hard limit: full document success or explicit failure (no partial load).
pub const SKILL_MD_MAX_BYTES: usize = 64 * 1024;
/// Total UTF-8 bytes of deduplicated selected skill bodies for one provider turn.
pub const SELECTED_SKILLS_MAX_TOTAL_BYTES: usize = 128 * 1024;
pub const TOOL_TRANSPORT_DYNAMIC_FIELD_BYTE_LIMIT: usize = 512;
/// Reserved for the enclosing SSE, Store, provider, or broker record that carries a tool execution.
/// Keeping the normalized execution below this derived ceiling ensures the complete transport record
/// stays under the shared hard limit without making each consumer invent a different constant.
pub const TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES: usize = 8 * 1024;
pub const TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT: usize =
    TOOL_EXECUTION_HARD_BYTE_LIMIT - TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES;

const MAX_ENVELOPE_TOOL_NAME_BYTES: usize = 256;
const MAX_OUTPUT_PREVIEW_BYTES: usize = 4 * 1024;
const READ_ONLY_RETRY_GUIDANCE: &str = "Retry with a narrower path, query, range, or limit, or use the tool's continuation mechanism when available.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolOutputBudgetState {
    WithinBudget,
    SoftLimitPreview,
    ReadOnlyRecoverableFailure,
    RetryUnsafeOutputOmitted,
    HardLimitFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolOutputBudgetReason {
    SoftByteLimit,
    SoftLineLimit,
    HardByteLimit,
    SerializationFailure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolOutputSemantics {
    ReadOnly,
    RetryUnsafe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolOutputMeasurement {
    pub serialized_bytes: usize,
    pub text_lines: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BudgetedToolExecution {
    pub execution: ToolExecution,
    pub state: ToolOutputBudgetState,
    pub original_measurement: Option<ToolOutputMeasurement>,
}

#[derive(Default)]
struct JsonSizeCounter {
    bytes: usize,
}

impl Write for JsonSizeCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::other("serialized JSON size overflow"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn serialized_json_size<T>(value: &T) -> Result<usize, serde_json::Error>
where
    T: Serialize + ?Sized,
{
    let mut counter = JsonSizeCounter::default();
    serde_json::to_writer(&mut counter, value)?;
    Ok(counter.bytes)
}

pub fn measure_tool_execution(
    execution: &ToolExecution,
) -> Result<ToolOutputMeasurement, serde_json::Error> {
    Ok(ToolOutputMeasurement {
        serialized_bytes: serialized_json_size(execution)?,
        text_lines: value_text_lines(&execution.output),
    })
}

pub fn stable_json_array_prefix_len(
    records: &[Value],
    max_serialized_bytes: usize,
) -> Result<usize, serde_json::Error> {
    if max_serialized_bytes < 2 {
        return Ok(0);
    }

    let mut bytes = 2_usize;
    let mut count = 0_usize;
    for record in records {
        let record_bytes = serialized_json_size(record)?;
        let separator_bytes = usize::from(count > 0);
        let Some(next_bytes) = bytes
            .checked_add(separator_bytes)
            .and_then(|current| current.checked_add(record_bytes))
        else {
            break;
        };
        if next_bytes > max_serialized_bytes {
            break;
        }
        bytes = next_bytes;
        count += 1;
    }
    Ok(count)
}

/// How many whole records fit under the shared soft byte and soft line budgets.
///
/// Record boundaries are never split. Line accounting uses nested text newlines so
/// multi-line match text still counts toward the 2,000-line soft limit.
pub fn soft_limit_array_prefix_len(records: &[Value]) -> Result<usize, serde_json::Error> {
    soft_limit_array_prefix_len_with_overhead(records, 0)
}

/// Like [`soft_limit_array_prefix_len`], but reserves `metadata_overhead_bytes` of the soft
/// byte budget for sibling response fields (query, path, continuation, notes, …).
pub fn soft_limit_array_prefix_len_with_overhead(
    records: &[Value],
    metadata_overhead_bytes: usize,
) -> Result<usize, serde_json::Error> {
    let max_bytes = TOOL_OUTPUT_SOFT_BYTE_LIMIT.saturating_sub(metadata_overhead_bytes);
    let byte_count = stable_json_array_prefix_len(records, max_bytes)?;
    let mut lines = 0_usize;
    let mut line_count = 0_usize;
    for record in records.iter().take(byte_count) {
        let record_lines = value_text_lines(record).max(1);
        let Some(next_lines) = lines.checked_add(record_lines) else {
            break;
        };
        if next_lines > TOOL_OUTPUT_SOFT_LINE_LIMIT {
            break;
        }
        lines = next_lines;
        line_count += 1;
    }
    Ok(line_count.min(byte_count))
}

/// Suggest an inclusive 1-based line range that keeps numbered `read_file` content under soft limits.
pub fn suggest_read_file_line_range(content: &str, content_start_line: usize) -> (usize, usize) {
    let start = content_start_line.max(1);
    if content.is_empty() {
        return (start, start);
    }

    let mut end = start;
    let mut bytes = 0_usize;
    let mut lines = 0_usize;
    for (offset, line) in content.split_inclusive('\n').enumerate() {
        let line_no = start.saturating_add(offset);
        // numbered_content prefixes each line with "{n}\t"
        let numbered_bytes = line.len().saturating_add(line_no.to_string().len() + 1);
        let Some(next_bytes) = bytes.checked_add(numbered_bytes) else {
            break;
        };
        let next_lines = lines.saturating_add(1);
        if next_bytes > TOOL_OUTPUT_SOFT_BYTE_LIMIT || next_lines > TOOL_OUTPUT_SOFT_LINE_LIMIT {
            break;
        }
        bytes = next_bytes;
        lines = next_lines;
        end = line_no;
    }

    if lines == 0 {
        (start, start)
    } else {
        (start, end)
    }
}

pub fn normalize_tool_execution(
    tool_name: &str,
    semantics: ToolOutputSemantics,
    execution: ToolExecution,
) -> BudgetedToolExecution {
    normalize_tool_execution_with_measurement(
        tool_name,
        semantics,
        execution,
        TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT,
        serialized_json_size,
    )
}

pub fn normalize_tool_execution_for_envelope<F>(
    tool_name: &str,
    semantics: ToolOutputSemantics,
    execution: ToolExecution,
    measure_envelope: F,
) -> BudgetedToolExecution
where
    F: Fn(&ToolExecution) -> Result<usize, serde_json::Error>,
{
    normalize_tool_execution_with_measurement(
        tool_name,
        semantics,
        execution,
        TOOL_EXECUTION_HARD_BYTE_LIMIT,
        measure_envelope,
    )
}

fn normalize_tool_execution_with_measurement<F>(
    tool_name: &str,
    semantics: ToolOutputSemantics,
    execution: ToolExecution,
    hard_byte_limit: usize,
    measure_envelope: F,
) -> BudgetedToolExecution
where
    F: Fn(&ToolExecution) -> Result<usize, serde_json::Error>,
{
    let measurement = match measure_envelope(&execution) {
        Ok(serialized_bytes) => ToolOutputMeasurement {
            serialized_bytes,
            text_lines: value_text_lines(&execution.output),
        },
        Err(error) => {
            return BudgetedToolExecution {
                execution: serialization_fallback(tool_name, semantics, execution.is_error, &error),
                state: ToolOutputBudgetState::HardLimitFallback,
                original_measurement: None,
            };
        }
    };

    let Some(reason) = budget_reason(measurement, hard_byte_limit) else {
        return BudgetedToolExecution {
            execution,
            state: ToolOutputBudgetState::WithinBudget,
            original_measurement: Some(measurement),
        };
    };

    // SKILL.md is an integrity-critical instruction document: allow full successful results
    // through the soft byte/line caps when the file itself is within SKILL_MD_MAX_BYTES.
    // Hard envelope limits still apply.
    if semantics == ToolOutputSemantics::ReadOnly
        && tool_name == "read_file"
        && !execution.is_error
        && reason != ToolOutputBudgetReason::HardByteLimit
        && read_file_output_is_skill_md_within_limit(&execution.output)
    {
        return BudgetedToolExecution {
            execution,
            state: ToolOutputBudgetState::WithinBudget,
            original_measurement: Some(measurement),
        };
    }

    if execution.is_error {
        let state = if reason == ToolOutputBudgetReason::HardByteLimit {
            ToolOutputBudgetState::HardLimitFallback
        } else {
            ToolOutputBudgetState::SoftLimitPreview
        };
        let execution =
            original_failure_omission(tool_name, &execution.output, measurement, reason, state);
        return finalize_budgeted_execution(
            tool_name,
            semantics,
            execution,
            state,
            measurement,
            hard_byte_limit,
            &measure_envelope,
        );
    }

    let (execution, state) = match semantics {
        ToolOutputSemantics::ReadOnly => (
            read_only_failure(tool_name, measurement, reason),
            ToolOutputBudgetState::ReadOnlyRecoverableFailure,
        ),
        ToolOutputSemantics::RetryUnsafe => (
            retry_unsafe_omission(tool_name, &execution.output, measurement, reason),
            ToolOutputBudgetState::RetryUnsafeOutputOmitted,
        ),
    };

    finalize_budgeted_execution(
        tool_name,
        semantics,
        execution,
        state,
        measurement,
        hard_byte_limit,
        &measure_envelope,
    )
}

fn finalize_budgeted_execution<F>(
    tool_name: &str,
    semantics: ToolOutputSemantics,
    execution: ToolExecution,
    state: ToolOutputBudgetState,
    measurement: ToolOutputMeasurement,
    hard_byte_limit: usize,
    measure_envelope: &F,
) -> BudgetedToolExecution
where
    F: Fn(&ToolExecution) -> Result<usize, serde_json::Error>,
{
    let fallback_is_error = execution.is_error;
    if measure_envelope(&execution)
        .is_ok_and(|serialized_bytes| serialized_bytes <= hard_byte_limit)
    {
        BudgetedToolExecution {
            execution,
            state,
            original_measurement: Some(measurement),
        }
    } else {
        BudgetedToolExecution {
            execution: minimal_hard_limit_fallback(
                tool_name,
                semantics,
                measurement,
                fallback_is_error,
            ),
            state: ToolOutputBudgetState::HardLimitFallback,
            original_measurement: Some(measurement),
        }
    }
}

fn budget_reason(
    measurement: ToolOutputMeasurement,
    hard_byte_limit: usize,
) -> Option<ToolOutputBudgetReason> {
    if measurement.serialized_bytes > hard_byte_limit {
        Some(ToolOutputBudgetReason::HardByteLimit)
    } else if measurement.serialized_bytes > TOOL_OUTPUT_SOFT_BYTE_LIMIT {
        Some(ToolOutputBudgetReason::SoftByteLimit)
    } else if measurement.text_lines > TOOL_OUTPUT_SOFT_LINE_LIMIT {
        Some(ToolOutputBudgetReason::SoftLineLimit)
    } else {
        None
    }
}

fn value_text_lines(value: &Value) -> usize {
    match value {
        Value::String(text) => {
            if text.is_empty() {
                0
            } else {
                text.bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    .saturating_add(1)
            }
        }
        Value::Array(values) => values.iter().fold(0_usize, |lines, value| {
            lines.saturating_add(value_text_lines(value))
        }),
        Value::Object(values) => values.values().fold(0_usize, |lines, value| {
            lines.saturating_add(value_text_lines(value))
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) => 0,
    }
}

fn bounded_tool_name(tool_name: &str) -> (&str, bool) {
    if tool_name.len() <= MAX_ENVELOPE_TOOL_NAME_BYTES {
        return (tool_name, false);
    }

    let mut end = MAX_ENVELOPE_TOOL_NAME_BYTES;
    while !tool_name.is_char_boundary(end) {
        end -= 1;
    }
    (&tool_name[..end], true)
}

fn common_budget_fields(
    tool_name: &str,
    state: ToolOutputBudgetState,
    measurement: ToolOutputMeasurement,
    reason: ToolOutputBudgetReason,
) -> Value {
    let (tool_name, tool_name_truncated) = bounded_tool_name(tool_name);
    json!({
        "budgetState": state,
        "toolName": tool_name,
        "toolNameTruncated": tool_name_truncated,
        "originalBytes": measurement.serialized_bytes,
        "originalLines": measurement.text_lines,
        "reason": reason,
        "softLimitBytes": TOOL_OUTPUT_SOFT_BYTE_LIMIT,
        "softLimitLines": TOOL_OUTPUT_SOFT_LINE_LIMIT,
        "hardLimitBytes": TOOL_EXECUTION_HARD_BYTE_LIMIT,
        "envelopeReserveBytes": TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES,
    })
}

fn read_only_failure(
    tool_name: &str,
    measurement: ToolOutputMeasurement,
    reason: ToolOutputBudgetReason,
) -> ToolExecution {
    let mut output = common_budget_fields(
        tool_name,
        ToolOutputBudgetState::ReadOnlyRecoverableFailure,
        measurement,
        reason,
    );
    output["error"] = Value::String(
        "Tool completed, but its read-only result exceeded the shared output budget and was omitted."
            .to_string(),
    );
    output["retryable"] = Value::Bool(true);
    output["guidance"] = Value::String(READ_ONLY_RETRY_GUIDANCE.to_string());
    ToolExecution {
        output,
        is_error: true,
    }
}

fn output_text_preview(value: &Value) -> Option<(&'static str, String, bool)> {
    let (source, text) = match value {
        Value::String(text) => ("output", text.as_str()),
        Value::Object(fields) => [
            "error", "message", "summary", "status", "path", "stderr", "stdout", "result",
        ]
        .into_iter()
        .find_map(|field| {
            fields
                .get(field)
                .and_then(Value::as_str)
                .map(|text| (field, text))
        })?,
        _ => return None,
    };
    let (preview, truncated) = bounded_utf8_prefix(text, MAX_OUTPUT_PREVIEW_BYTES);
    Some((source, preview.to_string(), truncated))
}

pub fn bounded_utf8_prefix(text: &str, max_bytes: usize) -> (&str, bool) {
    if text.len() <= max_bytes {
        return (text, false);
    }

    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
}

/// Returns true when the path's file name is exactly `SKILL.md` (case-sensitive).
pub fn path_is_skill_md(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "SKILL.md")
}

fn read_file_output_is_skill_md_within_limit(output: &Value) -> bool {
    let Some(path) = output.get("path").and_then(Value::as_str) else {
        return false;
    };
    if !path_is_skill_md(Path::new(path)) {
        return false;
    }
    match output.get("bytes").and_then(Value::as_u64) {
        Some(bytes) => bytes <= SKILL_MD_MAX_BYTES as u64,
        None => false,
    }
}

fn attach_output_preview(output: &mut Value, original_output: &Value) {
    let Some((source, preview, truncated)) = output_text_preview(original_output) else {
        return;
    };
    output["outputPreview"] = Value::String(preview);
    output["outputPreviewSource"] = Value::String(source.to_string());
    output["outputPreviewTruncated"] = Value::Bool(truncated);
}

fn retry_unsafe_omission(
    tool_name: &str,
    original_output: &Value,
    measurement: ToolOutputMeasurement,
    reason: ToolOutputBudgetReason,
) -> ToolExecution {
    let mut output = common_budget_fields(
        tool_name,
        ToolOutputBudgetState::RetryUnsafeOutputOmitted,
        measurement,
        reason,
    );
    output["summary"] = Value::String(
        "Tool completed successfully, but its output exceeded the shared output budget and was omitted."
            .to_string(),
    );
    output["outputOmitted"] = Value::Bool(true);
    output["retryUnsafe"] = Value::Bool(true);
    attach_output_preview(&mut output, original_output);
    ToolExecution {
        output,
        is_error: false,
    }
}

fn original_failure_omission(
    tool_name: &str,
    original_output: &Value,
    measurement: ToolOutputMeasurement,
    reason: ToolOutputBudgetReason,
    state: ToolOutputBudgetState,
) -> ToolExecution {
    let mut output = common_budget_fields(tool_name, state, measurement, reason);
    output["error"] = Value::String(
        "Tool failed, and its oversized error output was explicitly omitted by the shared output budget."
            .to_string(),
    );
    output["outputOmitted"] = Value::Bool(true);
    attach_output_preview(&mut output, original_output);
    ToolExecution {
        output,
        is_error: true,
    }
}

fn serialization_fallback(
    tool_name: &str,
    semantics: ToolOutputSemantics,
    original_is_error: bool,
    error: &serde_json::Error,
) -> ToolExecution {
    let (tool_name, tool_name_truncated) = bounded_tool_name(tool_name);
    let retry_unsafe = !original_is_error && semantics == ToolOutputSemantics::RetryUnsafe;
    ToolExecution {
        output: json!({
            "budgetState": ToolOutputBudgetState::HardLimitFallback,
            "toolName": tool_name,
            "toolNameTruncated": tool_name_truncated,
            "reason": ToolOutputBudgetReason::SerializationFailure,
            "error": format!("Tool result could not be measured for safe serialization: {error}"),
            "outputOmitted": true,
            "retryUnsafe": retry_unsafe,
            "hardLimitBytes": TOOL_EXECUTION_HARD_BYTE_LIMIT,
            "envelopeReserveBytes": TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES,
        }),
        is_error: original_is_error || semantics == ToolOutputSemantics::ReadOnly,
    }
}

fn minimal_hard_limit_fallback(
    tool_name: &str,
    semantics: ToolOutputSemantics,
    measurement: ToolOutputMeasurement,
    is_error: bool,
) -> ToolExecution {
    let (tool_name, _) = bounded_tool_name(tool_name);
    let retry_unsafe = !is_error && semantics == ToolOutputSemantics::RetryUnsafe;
    ToolExecution {
        output: json!({
            "budgetState": ToolOutputBudgetState::HardLimitFallback,
            "toolName": tool_name,
            "originalBytes": measurement.serialized_bytes,
            "reason": ToolOutputBudgetReason::HardByteLimit,
            "outputOmitted": true,
            "retryUnsafe": retry_unsafe,
            "hardLimitBytes": TOOL_EXECUTION_HARD_BYTE_LIMIT,
            "envelopeReserveBytes": TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES,
        }),
        is_error,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn serialized_json_size_matches_final_utf8_json() {
        let execution = ToolExecution {
            output: json!({ "text": "你好\nworld", "items": [1, 2, 3] }),
            is_error: false,
        };

        assert_eq!(
            serialized_json_size(&execution).expect("measure execution"),
            serde_json::to_vec(&execution)
                .expect("serialize execution")
                .len()
        );
    }

    #[test]
    fn stable_json_array_prefix_len_keeps_original_order_without_cloning() {
        let records = vec![json!({ "id": 1 }), json!({ "id": 2 }), json!({ "id": 3 })];
        let first_two_bytes = serde_json::to_vec(&records[..2])
            .expect("serialize prefix")
            .len();

        assert_eq!(
            stable_json_array_prefix_len(&records, first_two_bytes).expect("measure prefix"),
            2
        );
    }

    #[test]
    fn normalize_tool_execution_preserves_small_results() {
        let execution = ToolExecution {
            output: json!({ "ok": true }),
            is_error: false,
        };

        let budgeted = normalize_tool_execution(
            "read_file",
            ToolOutputSemantics::ReadOnly,
            execution.clone(),
        );

        assert_eq!(budgeted.execution, execution);
        assert_eq!(budgeted.state, ToolOutputBudgetState::WithinBudget);
    }

    #[test]
    fn normalize_tool_execution_returns_retryable_error_for_large_read_only_result() {
        let execution = ToolExecution {
            output: json!({ "content": "x".repeat(TOOL_OUTPUT_SOFT_BYTE_LIMIT) }),
            is_error: false,
        };

        let budgeted =
            normalize_tool_execution("read_file", ToolOutputSemantics::ReadOnly, execution);

        assert!(budgeted.execution.is_error);
        assert_eq!(budgeted.execution.output["retryable"], true);
        assert_eq!(
            budgeted.execution.output["budgetState"],
            "readOnlyRecoverableFailure"
        );
    }

    #[test]
    fn normalize_tool_execution_omits_large_retry_unsafe_result_without_failing() {
        let execution = ToolExecution {
            output: json!({ "stdout": "x".repeat(TOOL_OUTPUT_SOFT_BYTE_LIMIT) }),
            is_error: false,
        };

        let budgeted =
            normalize_tool_execution("run_command", ToolOutputSemantics::RetryUnsafe, execution);

        assert!(!budgeted.execution.is_error);
        assert_eq!(budgeted.execution.output["outputOmitted"], true);
        assert_eq!(budgeted.execution.output["retryUnsafe"], true);
        assert_eq!(budgeted.execution.output["outputPreviewSource"], "stdout");
        assert_eq!(budgeted.execution.output["outputPreviewTruncated"], true);
    }

    #[test]
    fn normalize_tool_execution_omits_soft_limit_failure_without_changing_failure() {
        let execution = ToolExecution {
            output: json!({ "error": "x".repeat(TOOL_OUTPUT_SOFT_BYTE_LIMIT) }),
            is_error: true,
        };

        let budgeted =
            normalize_tool_execution("run_command", ToolOutputSemantics::RetryUnsafe, execution);

        assert!(budgeted.execution.is_error);
        assert_eq!(budgeted.state, ToolOutputBudgetState::SoftLimitPreview);
        assert_eq!(budgeted.execution.output["outputOmitted"], true);
        assert!(budgeted.execution.output.get("retryUnsafe").is_none());
        assert_eq!(budgeted.execution.output["outputPreviewSource"], "error");
    }

    #[test]
    fn normalize_tool_execution_preserves_failure_semantics_at_hard_limit() {
        let execution = ToolExecution {
            output: json!({ "error": "x".repeat(TOOL_EXECUTION_HARD_BYTE_LIMIT) }),
            is_error: true,
        };

        let budgeted =
            normalize_tool_execution("run_command", ToolOutputSemantics::RetryUnsafe, execution);

        assert!(budgeted.execution.is_error);
        assert_eq!(budgeted.execution.output["outputOmitted"], true);
        assert!(budgeted.execution.output.get("retryUnsafe").is_none());
        assert_eq!(
            budgeted.execution.output["budgetState"],
            "hardLimitFallback"
        );
        assert!(
            serialized_json_size(&budgeted.execution).expect("measure fallback")
                <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
        );
    }

    #[test]
    fn normalize_tool_execution_reserves_space_for_transport_envelope() {
        let execution = ToolExecution {
            output: json!({
                "content": "x".repeat(TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT)
            }),
            is_error: false,
        };

        let budgeted =
            normalize_tool_execution("read_file", ToolOutputSemantics::ReadOnly, execution);

        assert_eq!(
            budgeted.execution.output["envelopeReserveBytes"],
            TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES
        );
        assert!(
            serialized_json_size(&budgeted.execution).expect("measure reserved result")
                <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
        );
    }

    #[test]
    fn normalize_tool_execution_applies_line_soft_limit() {
        let execution = ToolExecution {
            output: json!({ "content": "line\n".repeat(TOOL_OUTPUT_SOFT_LINE_LIMIT) }),
            is_error: false,
        };

        let budgeted =
            normalize_tool_execution("search_text", ToolOutputSemantics::ReadOnly, execution);

        assert_eq!(budgeted.execution.output["reason"], "softLineLimit");
    }

    #[test]
    fn normalize_tool_execution_allows_skill_md_read_above_soft_limit() {
        let content_len = TOOL_OUTPUT_SOFT_BYTE_LIMIT + 4 * 1024;
        let execution = ToolExecution {
            output: json!({
                "path": ".agents/skills/mid/SKILL.md",
                "content": "x".repeat(content_len),
                "bytes": content_len,
                "startLine": null,
                "endLine": null,
            }),
            is_error: false,
        };

        let budgeted = normalize_tool_execution(
            "read_file",
            ToolOutputSemantics::ReadOnly,
            execution.clone(),
        );

        assert_eq!(budgeted.state, ToolOutputBudgetState::WithinBudget);
        assert_eq!(budgeted.execution, execution);
    }

    #[test]
    fn path_is_skill_md_matches_exact_file_name_only() {
        assert!(path_is_skill_md(Path::new("SKILL.md")));
        assert!(path_is_skill_md(Path::new(
            "/home/u/.agents/skills/x/SKILL.md"
        )));
        assert!(!path_is_skill_md(Path::new("skill.md")));
        assert!(!path_is_skill_md(Path::new("references/SKILL.md.bak")));
        assert!(!path_is_skill_md(Path::new("references/large.md")));
    }

    #[test]
    fn soft_byte_boundary_is_measured_on_final_serialized_execution() {
        // Grow content until the full ToolExecution JSON is just over the soft limit,
        // then shrink one char so it sits exactly at the soft limit.
        let mut content = String::from("x");
        let mut over_limit = ToolExecution {
            output: json!({ "content": content.clone() }),
            is_error: false,
        };
        while serialized_json_size(&over_limit).expect("measure") <= TOOL_OUTPUT_SOFT_BYTE_LIMIT {
            content.push('x');
            over_limit.output = json!({ "content": content.clone() });
        }
        let over_bytes = serialized_json_size(&over_limit).expect("measure over");
        assert_eq!(over_bytes, TOOL_OUTPUT_SOFT_BYTE_LIMIT + 1);

        content.pop();
        let at_limit = ToolExecution {
            output: json!({ "content": content }),
            is_error: false,
        };
        let at_bytes = serialized_json_size(&at_limit).expect("measure at");
        assert_eq!(at_bytes, TOOL_OUTPUT_SOFT_BYTE_LIMIT);

        let within = normalize_tool_execution("read_file", ToolOutputSemantics::ReadOnly, at_limit);
        assert_eq!(within.state, ToolOutputBudgetState::WithinBudget);
        assert!(!within.execution.is_error);

        let over = normalize_tool_execution("read_file", ToolOutputSemantics::ReadOnly, over_limit);
        assert_eq!(
            over.state,
            ToolOutputBudgetState::ReadOnlyRecoverableFailure
        );
        assert!(over.execution.is_error);
        assert_eq!(over.execution.output["reason"], "softByteLimit");
        assert_eq!(
            serde_json::to_vec(&over.execution)
                .expect("serialize over result")
                .len(),
            serialized_json_size(&over.execution).expect("remeasure over result")
        );
        assert!(
            serialized_json_size(&over.execution).expect("remeasure")
                <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
        );
    }

    #[test]
    fn soft_line_boundary_accepts_exact_limit_and_rejects_plus_one() {
        // value_text_lines counts trailing-newline-free text as N lines for N newlines + 1.
        // "line\n" * 1999 ends with newline → 1999 lines; plus final "line" → 2000 lines.
        let at_limit_text = format!("{}line", "line\n".repeat(TOOL_OUTPUT_SOFT_LINE_LIMIT - 1));
        let at_limit = ToolExecution {
            output: json!({ "content": at_limit_text }),
            is_error: false,
        };
        assert_eq!(
            measure_tool_execution(&at_limit)
                .expect("measure at")
                .text_lines,
            TOOL_OUTPUT_SOFT_LINE_LIMIT
        );
        // Keep total serialized size under the soft byte limit so only the line gate fires.
        assert!(serialized_json_size(&at_limit).expect("bytes at") <= TOOL_OUTPUT_SOFT_BYTE_LIMIT);
        let within =
            normalize_tool_execution("search_text", ToolOutputSemantics::ReadOnly, at_limit);
        assert_eq!(within.state, ToolOutputBudgetState::WithinBudget);

        let over_text = format!("{}line", "line\n".repeat(TOOL_OUTPUT_SOFT_LINE_LIMIT));
        let over_limit = ToolExecution {
            output: json!({ "content": over_text }),
            is_error: false,
        };
        assert_eq!(
            measure_tool_execution(&over_limit)
                .expect("measure over")
                .text_lines,
            TOOL_OUTPUT_SOFT_LINE_LIMIT + 1
        );
        assert!(
            serialized_json_size(&over_limit).expect("bytes over") <= TOOL_OUTPUT_SOFT_BYTE_LIMIT
        );
        let over =
            normalize_tool_execution("search_text", ToolOutputSemantics::ReadOnly, over_limit);
        assert_eq!(
            over.state,
            ToolOutputBudgetState::ReadOnlyRecoverableFailure
        );
        assert_eq!(over.execution.output["reason"], "softLineLimit");
        assert!(over.execution.is_error);
    }

    #[test]
    fn multibyte_utf8_and_json_escape_use_final_serialized_bytes() {
        // Chinese characters are multi-byte; quotes inside strings become JSON escapes (`\"`).
        // 20_000 × 3-byte chars + escaped quotes exceeds the 50 KiB soft limit on final UTF-8 JSON.
        let text = format!("\"你好\"{}", "界".repeat(20_000));
        let execution = ToolExecution {
            output: json!({ "content": text }),
            is_error: false,
        };
        let measured = serialized_json_size(&execution).expect("measure");
        let vec_len = serde_json::to_vec(&execution).expect("to_vec").len();
        assert_eq!(measured, vec_len);
        assert!(
            measured > TOOL_OUTPUT_SOFT_BYTE_LIMIT,
            "measured={measured} soft={TOOL_OUTPUT_SOFT_BYTE_LIMIT}"
        );

        let budgeted =
            normalize_tool_execution("read_file", ToolOutputSemantics::ReadOnly, execution);
        assert!(budgeted.execution.is_error);
        let result_bytes = serde_json::to_vec(&budgeted.execution)
            .expect("serialize result")
            .len();
        assert_eq!(
            result_bytes,
            serialized_json_size(&budgeted.execution).expect("remeasure")
        );
        assert!(result_bytes <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT);
    }

    #[test]
    fn hard_byte_boundary_forces_fallback_when_payload_exceeds_reserve() {
        // Payload over the hard envelope limit must still land under the reserved payload ceiling.
        let execution = ToolExecution {
            output: json!({
                "blob": "x".repeat(TOOL_EXECUTION_HARD_BYTE_LIMIT)
            }),
            is_error: false,
        };
        let original = serialized_json_size(&execution).expect("measure original");
        assert!(original > TOOL_EXECUTION_HARD_BYTE_LIMIT);

        let budgeted =
            normalize_tool_execution("run_command", ToolOutputSemantics::RetryUnsafe, execution);
        assert!(!budgeted.execution.is_error);
        assert!(
            matches!(
                budgeted.state,
                ToolOutputBudgetState::HardLimitFallback
                    | ToolOutputBudgetState::RetryUnsafeOutputOmitted
            ),
            "{:?}",
            budgeted.state
        );
        assert_eq!(budgeted.execution.output["outputOmitted"], true);
        assert_eq!(budgeted.execution.output["retryUnsafe"], true);
        assert!(
            serialized_json_size(&budgeted.execution).expect("measure fallback")
                <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
        );
    }

    #[test]
    fn hard_byte_boundary_accepts_exact_payload_limit_and_rejects_plus_one() {
        // Hard gate uses TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT for bare ToolExecution.
        // Soft limits fire first for large payloads; exact hard size must still avoid hard fallback
        // until the measured size exceeds the hard ceiling by one byte.
        let mut content = "x".repeat(TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT);
        let mut over_limit = ToolExecution {
            output: json!({ "content": content.clone() }),
            is_error: false,
        };
        // Grow or shrink to land exactly one byte over the payload hard ceiling.
        while serialized_json_size(&over_limit).expect("measure")
            > TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT + 1
        {
            content.pop();
            over_limit.output = json!({ "content": content.clone() });
        }
        while serialized_json_size(&over_limit).expect("measure")
            < TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT + 1
        {
            content.push('x');
            over_limit.output = json!({ "content": content.clone() });
        }
        let over_bytes = serialized_json_size(&over_limit).expect("measure over");
        assert_eq!(over_bytes, TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT + 1);
        assert_eq!(
            over_bytes,
            serde_json::to_vec(&over_limit).expect("to_vec over").len()
        );

        content.pop();
        let at_limit = ToolExecution {
            output: json!({ "content": content }),
            is_error: false,
        };
        let at_bytes = serialized_json_size(&at_limit).expect("measure at");
        assert_eq!(at_bytes, TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT);
        assert_eq!(
            at_bytes,
            serde_json::to_vec(&at_limit).expect("to_vec at").len()
        );

        let at_budgeted =
            normalize_tool_execution("read_file", ToolOutputSemantics::ReadOnly, at_limit);
        // Exact hard payload size is still under the hard gate (`>` not `>=`), so only soft
        // recovery may apply — never hardByteLimit / HardLimitFallback.
        assert_eq!(
            at_budgeted.state,
            ToolOutputBudgetState::ReadOnlyRecoverableFailure
        );
        assert!(at_budgeted.execution.is_error);
        assert_eq!(at_budgeted.execution.output["reason"], "softByteLimit");
        assert!(
            serialized_json_size(&at_budgeted.execution).expect("measure at result")
                <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
        );
        // Original at-limit execution was over soft but not over hard.
        assert!(at_bytes > TOOL_OUTPUT_SOFT_BYTE_LIMIT);
        assert!(at_bytes <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT);

        let over_budgeted =
            normalize_tool_execution("read_file", ToolOutputSemantics::ReadOnly, over_limit);
        // One byte over the payload hard ceiling must take the hard-reason path (not stay WithinBudget).
        // Read-only shrink usually lands as recoverable failure with reason hardByteLimit; only when
        // that still cannot fit does state become HardLimitFallback.
        assert_ne!(over_budgeted.state, ToolOutputBudgetState::WithinBudget);
        assert!(over_budgeted.execution.is_error);
        assert_eq!(over_budgeted.execution.output["reason"], "hardByteLimit");
        assert!(
            serialized_json_size(&over_budgeted.execution).expect("measure over result")
                <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
        );
        assert_eq!(
            serde_json::to_vec(&over_budgeted.execution)
                .expect("to_vec result")
                .len(),
            serialized_json_size(&over_budgeted.execution).expect("remeasure")
        );
    }

    #[test]
    fn hard_byte_envelope_accepts_exact_limit_and_rejects_plus_one() {
        // Envelope-aware hard limit: measure a synthetic transport wrapper, not bare ToolExecution.
        #[derive(serde::Serialize)]
        struct SyntheticEnvelope<'a> {
            #[serde(rename = "type")]
            event_type: &'static str,
            tool_call_id: &'static str,
            output: &'a Value,
            is_error: bool,
        }

        let measure = |execution: &ToolExecution| {
            serialized_json_size(&SyntheticEnvelope {
                event_type: "toolResult",
                tool_call_id: "call-boundary",
                output: &execution.output,
                is_error: execution.is_error,
            })
        };

        let mut content = "x".repeat(TOOL_EXECUTION_HARD_BYTE_LIMIT);
        let mut over_limit = ToolExecution {
            output: json!({ "content": content.clone() }),
            is_error: false,
        };
        while measure(&over_limit).expect("measure envelope") > TOOL_EXECUTION_HARD_BYTE_LIMIT + 1 {
            content.pop();
            over_limit.output = json!({ "content": content.clone() });
        }
        while measure(&over_limit).expect("measure envelope") < TOOL_EXECUTION_HARD_BYTE_LIMIT + 1 {
            content.push('x');
            over_limit.output = json!({ "content": content.clone() });
        }
        let over_envelope = measure(&over_limit).expect("measure over envelope");
        assert_eq!(over_envelope, TOOL_EXECUTION_HARD_BYTE_LIMIT + 1);

        content.pop();
        let at_limit = ToolExecution {
            output: json!({ "content": content }),
            is_error: false,
        };
        let at_envelope = measure(&at_limit).expect("measure at envelope");
        assert_eq!(at_envelope, TOOL_EXECUTION_HARD_BYTE_LIMIT);

        let at_budgeted = normalize_tool_execution_for_envelope(
            "run_command",
            ToolOutputSemantics::RetryUnsafe,
            at_limit,
            measure,
        );
        // Exact hard envelope size is still soft-only: hard gate uses `>` not `>=`.
        assert_eq!(
            at_budgeted.state,
            ToolOutputBudgetState::RetryUnsafeOutputOmitted
        );
        assert!(!at_budgeted.execution.is_error);
        assert_eq!(at_budgeted.execution.output["reason"], "softByteLimit");
        assert_eq!(at_budgeted.execution.output["outputOmitted"], true);
        assert_eq!(at_budgeted.execution.output["retryUnsafe"], true);
        assert!(
            measure(&at_budgeted.execution).expect("remeasure at")
                <= TOOL_EXECUTION_HARD_BYTE_LIMIT
        );

        let over_budgeted = normalize_tool_execution_for_envelope(
            "run_command",
            ToolOutputSemantics::RetryUnsafe,
            over_limit,
            measure,
        );
        // Hard+1 must leave the soft-only path: either hard fallback or retry-unsafe omit after
        // hard reason. Final envelope must fit the hard ceiling.
        assert_ne!(over_budgeted.state, ToolOutputBudgetState::WithinBudget);
        assert!(!over_budgeted.execution.is_error);
        assert_eq!(over_budgeted.execution.output["outputOmitted"], true);
        assert_eq!(over_budgeted.execution.output["retryUnsafe"], true);
        assert_eq!(over_budgeted.execution.output["reason"], "hardByteLimit");
        assert!(
            measure(&over_budgeted.execution).expect("remeasure over")
                <= TOOL_EXECUTION_HARD_BYTE_LIMIT
        );
        // Confirm hard path was required: over-limit original would not pass soft-only sizing.
        assert!(over_envelope > TOOL_EXECUTION_HARD_BYTE_LIMIT);
    }
}
