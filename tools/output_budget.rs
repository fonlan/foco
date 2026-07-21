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

    match truncate_to_complete_lines_with_measure(
        content,
        CompleteLineTruncateOptions {
            content_start_line: start,
            ..CompleteLineTruncateOptions::shared_defaults()
        },
        |line_no, line| line.len().saturating_add(line_no.to_string().len() + 1),
    ) {
        CompleteLineTruncateOutcome::Full(prefix)
        | CompleteLineTruncateOutcome::Truncated(prefix) => {
            if prefix.returned_lines == 0 {
                (start, start)
            } else {
                (start, prefix.last_returned_line)
            }
        }
        CompleteLineTruncateOutcome::SingleLineExceedsHardLimit { line_number, .. } => {
            (line_number, line_number)
        }
    }
}

/// Model-visible fields for complete-line truncation (local, SSH, and tool prompts share these names).
pub const LINE_BOUNDED_TRUNCATED_FIELD: &str = "truncated";
pub const LINE_BOUNDED_NEXT_START_LINE_FIELD: &str = "nextStartLine";
pub const LINE_BOUNDED_FULL_RESULT_PATH_FIELD: &str = "fullResultPath";
pub const LINE_BOUNDED_NOTE_FIELD: &str = "note";
pub const LINE_BOUNDED_RETURNED_LINES_FIELD: &str = "returnedLines";
pub const LINE_BOUNDED_LAST_RETURNED_LINE_FIELD: &str = "lastReturnedLine";

/// Budgets for [`truncate_to_complete_lines`] / [`truncate_to_complete_lines_with_measure`].
///
/// Soft limits come from the shared tool output budget constants. The hard byte ceiling is the
/// payload hard limit (full envelope minus reserve) so a single oversize line still has room for
/// metadata in the final ToolExecution JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompleteLineTruncateOptions {
    /// Soft UTF-8 byte budget for measured line content.
    pub soft_byte_limit: usize,
    /// Soft complete-line count budget.
    pub soft_line_limit: usize,
    /// Hard UTF-8 byte ceiling for a single complete line (no mid-line split).
    pub hard_byte_limit: usize,
    /// 1-based line number of the first line in the content slice.
    pub content_start_line: usize,
}

impl CompleteLineTruncateOptions {
    /// Shared soft (50 KiB / 2,000 lines) and payload hard defaults from `output_budget` constants.
    pub const fn shared_defaults() -> Self {
        Self {
            soft_byte_limit: TOOL_OUTPUT_SOFT_BYTE_LIMIT,
            soft_line_limit: TOOL_OUTPUT_SOFT_LINE_LIMIT,
            hard_byte_limit: TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT,
            content_start_line: 1,
        }
    }

    pub const fn with_content_start_line(mut self, content_start_line: usize) -> Self {
        self.content_start_line = if content_start_line == 0 {
            1
        } else {
            content_start_line
        };
        self
    }
}

/// Complete-line UTF-8 prefix under soft/hard budgets (no mid-character or mid-line splits).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompleteLinePrefix {
    /// Text made only of complete lines from the input.
    pub text: String,
    /// Number of complete lines included in `text`.
    pub returned_lines: usize,
    /// 1-based line number of the last included line (`content_start_line` when empty).
    pub last_returned_line: usize,
    /// Next 1-based line for continuation when `truncated`; `None` when not truncated.
    pub next_start_line: Option<usize>,
    /// Whether input content was cut after a complete line boundary.
    pub truncated: bool,
}

/// Outcome of complete-line truncation for read_file / Web callers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompleteLineTruncateOutcome {
    /// Entire content fits under soft byte and line budgets.
    Full(CompleteLinePrefix),
    /// Soft-truncated complete-line prefix; success path for tools (`is_error=false`, `truncated=true`).
    Truncated(CompleteLinePrefix),
    /// A single complete line cannot fit under the hard byte ceiling without splitting.
    SingleLineExceedsHardLimit {
        line_number: usize,
        line_bytes: usize,
        hard_limit_bytes: usize,
    },
}

impl CompleteLineTruncateOutcome {
    pub fn as_prefix(&self) -> Option<&CompleteLinePrefix> {
        match self {
            Self::Full(prefix) | Self::Truncated(prefix) => Some(prefix),
            Self::SingleLineExceedsHardLimit { .. } => None,
        }
    }

    /// Model-facing note for success paths; `None` for hard single-line failures.
    pub fn model_note(&self) -> Option<String> {
        match self {
            Self::Full(prefix) if !prefix.truncated => None,
            Self::Full(prefix) | Self::Truncated(prefix) => Some(complete_line_prefix_note(prefix)),
            Self::SingleLineExceedsHardLimit {
                line_number,
                line_bytes,
                hard_limit_bytes,
            } => Some(format!(
                "Line {line_number} is {line_bytes} UTF-8 bytes and exceeds the hard output ceiling ({hard_limit_bytes} bytes). Cannot return it without splitting a line; refine the source or request a different range."
            )),
        }
    }

    /// Recoverable error message when a single line exceeds the hard ceiling.
    pub fn single_line_hard_limit_error(&self) -> Option<String> {
        match self {
            Self::SingleLineExceedsHardLimit {
                line_number,
                line_bytes,
                hard_limit_bytes,
            } => Some(format!(
                "A single complete line (line {line_number}, {line_bytes} UTF-8 bytes) exceeds the hard output ceiling ({hard_limit_bytes} bytes) and cannot be returned without splitting the line. Refine the source content or use a tool path that does not require this line intact."
            )),
            _ => None,
        }
    }
}

/// Walk complete lines of `content` using the same endings as file-tool `line_spans`:
/// `\r\n`, `\n`, and lone `\r`. Each visit receives the 0-based line offset and the line slice
/// including its trailing line ending when present. A trailing newline does **not** create an
/// extra empty line. Return `false` from `visit` to stop early.
pub fn for_each_complete_line<'a, F>(content: &'a str, mut visit: F)
where
    F: FnMut(usize, &'a str) -> bool,
{
    if content.is_empty() {
        return;
    }
    let bytes = content.as_bytes();
    let mut start = 0_usize;
    let mut index = 0_usize;
    let mut offset = 0_usize;
    while index < bytes.len() {
        let end = match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => index + 2,
            b'\r' | b'\n' => index + 1,
            _ => {
                index += 1;
                continue;
            }
        };
        if !visit(offset, &content[start..end]) {
            return;
        }
        offset = offset.saturating_add(1);
        start = end;
        index = end;
    }
    if start < bytes.len() {
        let _ = visit(offset, &content[start..]);
    }
}

/// Drop the last complete line from `prefix_text` (supports `\r\n` / `\n` / `\r`).
///
/// Returns `Some("")` when only one line remains, or `None` when `prefix_text` is empty.
pub fn peel_last_complete_line(prefix_text: &str) -> Option<&str> {
    if prefix_text.is_empty() {
        return None;
    }
    let mut prev_end = 0_usize;
    let mut end = 0_usize;
    let mut count = 0_usize;
    for_each_complete_line(prefix_text, |_, line| {
        prev_end = end;
        end = end.saturating_add(line.len());
        count = count.saturating_add(1);
        true
    });
    if count == 0 {
        None
    } else if count == 1 {
        Some("")
    } else {
        Some(&prefix_text[..prev_end])
    }
}

/// Take a UTF-8 complete-line prefix of `content` under shared soft/hard budgets.
///
/// Line measurement uses raw UTF-8 byte length of each line (including its trailing line ending
/// when present). Callers that emit numbered lines (e.g. `read_file`) should use
/// [`truncate_to_complete_lines_with_measure`] instead.
pub fn truncate_to_complete_lines(
    content: &str,
    options: CompleteLineTruncateOptions,
) -> CompleteLineTruncateOutcome {
    truncate_to_complete_lines_with_measure(content, options, |_line_no, line| line.len())
}

/// Like [`truncate_to_complete_lines`], but each line's byte cost is supplied by `measure_line`.
///
/// `measure_line` receives the 1-based line number and the line slice (including its trailing
/// `\r\n` / `\n` / `\r` when present). UTF-8 character boundaries are never split because
/// iteration is over whole lines of a valid `&str`. Line endings match file-tool `line_spans`.
pub fn truncate_to_complete_lines_with_measure<F>(
    content: &str,
    options: CompleteLineTruncateOptions,
    mut measure_line: F,
) -> CompleteLineTruncateOutcome
where
    F: FnMut(usize, &str) -> usize,
{
    let start = options.content_start_line.max(1);
    let soft_bytes = options.soft_byte_limit;
    let soft_lines = options.soft_line_limit;
    let hard_bytes = options.hard_byte_limit;

    if content.is_empty() {
        return CompleteLineTruncateOutcome::Full(CompleteLinePrefix {
            text: String::new(),
            returned_lines: 0,
            last_returned_line: start,
            next_start_line: None,
            truncated: false,
        });
    }

    let mut end_byte = 0_usize;
    let mut bytes = 0_usize;
    let mut lines = 0_usize;
    let mut last_line = start;
    let mut remaining_after_prefix = false;
    // Soft-over single complete line must be marked truncated so normalize preserves the
    // success under the Phase 1 `truncated == true` contract, even when it is the entire
    // remaining content (nextStartLine may point past EOF).
    let mut force_truncated = false;
    let mut hard_limit_failure: Option<(usize, usize)> = None;

    for_each_complete_line(content, |offset, line| {
        if hard_limit_failure.is_some() {
            return false;
        }
        let line_no = start.saturating_add(offset);
        let line_bytes = measure_line(line_no, line);

        if lines == 0 {
            if line_bytes > hard_bytes {
                hard_limit_failure = Some((line_no, line_bytes));
                return false;
            }
            // One complete line over soft but under hard: return it fully so callers make progress.
            if line_bytes > soft_bytes {
                end_byte = line.len();
                lines = 1;
                last_line = line_no;
                remaining_after_prefix = end_byte < content.len();
                force_truncated = true;
                return false;
            }
        }

        let Some(next_bytes) = bytes.checked_add(line_bytes) else {
            remaining_after_prefix = true;
            return false;
        };
        let next_lines = lines.saturating_add(1);
        if next_bytes > soft_bytes || next_lines > soft_lines {
            remaining_after_prefix = true;
            return false;
        }

        end_byte = end_byte.saturating_add(line.len());
        bytes = next_bytes;
        lines = next_lines;
        last_line = line_no;
        true
    });

    if let Some((line_number, line_bytes)) = hard_limit_failure {
        return CompleteLineTruncateOutcome::SingleLineExceedsHardLimit {
            line_number,
            line_bytes,
            hard_limit_bytes: hard_bytes,
        };
    }

    debug_assert!(end_byte <= content.len());
    let truncated =
        force_truncated || (remaining_after_prefix && end_byte < content.len());
    let prefix = CompleteLinePrefix {
        text: content[..end_byte].to_string(),
        returned_lines: lines,
        last_returned_line: last_line,
        next_start_line: if truncated {
            Some(last_line.saturating_add(1))
        } else {
            None
        },
        truncated,
    };

    if truncated {
        CompleteLineTruncateOutcome::Truncated(prefix)
    } else {
        CompleteLineTruncateOutcome::Full(prefix)
    }
}

/// Default model-facing note for a complete-line prefix success/truncation.
pub fn complete_line_prefix_note(prefix: &CompleteLinePrefix) -> String {
    if !prefix.truncated {
        return format!(
            "Returned {} complete line(s) through line {} under the shared output budget.",
            prefix.returned_lines, prefix.last_returned_line
        );
    }
    let next = prefix
        .next_start_line
        .unwrap_or(prefix.last_returned_line.saturating_add(1));
    format!(
        "Result truncated at a complete line boundary under the shared soft output budget (max {TOOL_OUTPUT_SOFT_BYTE_LIMIT} bytes or {TOOL_OUTPUT_SOFT_LINE_LIMIT} lines): returned {} line(s) through line {}. Continue with nextStartLine={next}. This is an explicit truncated success (is_error=false), not hidden data loss.",
        prefix.returned_lines, prefix.last_returned_line
    )
}

/// Whether a successful tool result already applied the complete-line budget contract.
///
/// Contract (Phase 1 / shared soft-budget preserve rule):
/// - Non-error outputs with **`truncated: true`** and a positive integer `nextStartLine`
///   (optional `fullResultPath` / `note`) are treated as actively generated safe truncated
///   successes and must not be re-written into a read-only soft-limit error by
///   [`normalize_tool_execution`].
/// - **`truncated: false` never bypasses** soft-limit normalization, including single-line
///   full results. Callers that return one soft-over/hard-under complete line must set
///   `truncated: true` and a positive `nextStartLine` so the success is preserved.
/// - Hard envelope limits still apply.
pub fn is_line_bounded_budget_success(execution: &ToolExecution) -> bool {
    if execution.is_error {
        return false;
    }
    let Some(fields) = execution.output.as_object() else {
        return false;
    };
    matches!(
        fields.get(LINE_BOUNDED_TRUNCATED_FIELD),
        Some(Value::Bool(true))
    ) && fields
        .get(LINE_BOUNDED_NEXT_START_LINE_FIELD)
        .and_then(Value::as_u64)
        .is_some_and(|line| line >= 1)
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

    // Tools that already applied complete-line budgeting (`truncated: true` + positive nextStartLine,
    // optional fullResultPath / note) must not be re-converted into a soft-limit read-only error.
    // Soft overages (e.g. one long line under the hard ceiling) stay success. `truncated: false`
    // never bypasses soft-limit normalization. Hard envelope limits still apply.
    if !execution.is_error
        && reason != ToolOutputBudgetReason::HardByteLimit
        && is_line_bounded_budget_success(&execution)
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

/// Count complete lines using the same semantics as [`for_each_complete_line`] /
/// file-tool `line_spans`: `\r\n`, `\n`, and lone `\r` are line endings; a trailing
/// newline does **not** create an extra empty line.
///
/// Examples: `""` → 0, `"a"` → 1, `"a\n"` → 1, `"a\nb\n"` → 2, `"a\rb\r"` → 2.
pub fn complete_line_count(text: &str) -> usize {
    let mut count = 0_usize;
    for_each_complete_line(text, |_, _| {
        count = count.saturating_add(1);
        true
    });
    count
}

fn value_text_lines(value: &Value) -> usize {
    match value {
        Value::String(text) => complete_line_count(text),
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
        // complete_line_count("line\n" * N) == N; need N > soft line limit.
        let execution = ToolExecution {
            output: json!({ "content": "line\n".repeat(TOOL_OUTPUT_SOFT_LINE_LIMIT + 1) }),
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
        // complete_line_count matches split_inclusive: trailing newline does not add an extra line.
        // "line\n" * 1999 + "line" → 2000 complete lines; "line\n" * 2000 + "line" → 2001.
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
    fn complete_line_count_matches_line_spans_endings() {
        assert_eq!(complete_line_count(""), 0);
        assert_eq!(complete_line_count("a"), 1);
        assert_eq!(complete_line_count("a\n"), 1);
        assert_eq!(complete_line_count("a\nb"), 2);
        assert_eq!(complete_line_count("a\nb\n"), 2);
        assert_eq!(
            complete_line_count(&"line\n".repeat(TOOL_OUTPUT_SOFT_LINE_LIMIT)),
            TOOL_OUTPUT_SOFT_LINE_LIMIT
        );
        // CR-only and CRLF must match file-tool line_spans (not split_inclusive('\\n') alone).
        assert_eq!(complete_line_count("a\r"), 1);
        assert_eq!(complete_line_count("a\rb"), 2);
        assert_eq!(complete_line_count("a\rb\r"), 2);
        assert_eq!(complete_line_count("a\r\n"), 1);
        assert_eq!(complete_line_count("a\r\nb\r\n"), 2);
        assert_eq!(complete_line_count(&"x\r".repeat(3000)), 3000);
        assert_eq!(complete_line_count("a\rb\nc\r\nd"), 4);
    }

    #[test]
    fn peel_last_complete_line_handles_cr_lf_and_crlf() {
        assert_eq!(peel_last_complete_line(""), None);
        assert_eq!(peel_last_complete_line("only"), Some(""));
        assert_eq!(peel_last_complete_line("a\nb\n"), Some("a\n"));
        assert_eq!(peel_last_complete_line("a\rb\r"), Some("a\r"));
        assert_eq!(peel_last_complete_line("a\r\nb\r\n"), Some("a\r\n"));
        assert_eq!(peel_last_complete_line("a\rb\nc"), Some("a\rb\n"));
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

    #[test]
    fn truncate_to_complete_lines_keeps_utf8_and_line_boundaries() {
        let content = "alpha\n你好世界\nbeta\n";
        let full = truncate_to_complete_lines(
            content,
            CompleteLineTruncateOptions {
                soft_byte_limit: content.len(),
                soft_line_limit: 10,
                hard_byte_limit: TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT,
                content_start_line: 1,
            },
        );
        let prefix = full.as_prefix().expect("prefix");
        assert!(!prefix.truncated);
        assert_eq!(prefix.text, "alpha\n你好世界\nbeta\n");
        assert_eq!(prefix.returned_lines, 3);
        assert_eq!(prefix.last_returned_line, 3);
        assert_eq!(prefix.next_start_line, None);

        let outcome = truncate_to_complete_lines(
            content,
            CompleteLineTruncateOptions {
                soft_byte_limit: "alpha\n".len() + "你好世界\n".len(),
                soft_line_limit: 10,
                hard_byte_limit: TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT,
                content_start_line: 10,
            },
        );
        match outcome {
            CompleteLineTruncateOutcome::Truncated(prefix) => {
                assert_eq!(prefix.text, "alpha\n你好世界\n");
                assert_eq!(prefix.returned_lines, 2);
                assert_eq!(prefix.last_returned_line, 11);
                assert_eq!(prefix.next_start_line, Some(12));
                assert!(prefix.truncated);
                assert!(prefix.text.is_char_boundary(prefix.text.len()));
                assert!(!prefix.text.contains("beta"));
            }
            other => panic!("expected truncated prefix, got {other:?}"),
        }
    }

    #[test]
    fn truncate_to_complete_lines_single_soft_over_line_under_hard_makes_progress() {
        let long_line = "x".repeat(TOOL_OUTPUT_SOFT_BYTE_LIMIT + 64);
        let content = format!("{long_line}\nmore\n");
        let outcome = truncate_to_complete_lines(
            content.as_str(),
            CompleteLineTruncateOptions::shared_defaults(),
        );
        match outcome {
            CompleteLineTruncateOutcome::Truncated(prefix) => {
                assert!(prefix.text.starts_with(&long_line));
                assert!(prefix.text.ends_with('\n'));
                assert_eq!(prefix.returned_lines, 1);
                assert_eq!(prefix.last_returned_line, 1);
                assert_eq!(prefix.next_start_line, Some(2));
                assert!(prefix.text.len() > TOOL_OUTPUT_SOFT_BYTE_LIMIT);
                assert!(prefix.text.len() <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT);
            }
            other => panic!("expected truncated single-line progress, got {other:?}"),
        }
    }

    #[test]
    fn truncate_to_complete_lines_single_soft_over_entire_content_still_truncated_true() {
        // Soft-over single line that is the entire content must still set truncated=true so
        // normalize preserves the success (Phase 1: truncated:false never bypasses soft limits).
        let long_line = "z".repeat(TOOL_OUTPUT_SOFT_BYTE_LIMIT + 32);
        let outcome = truncate_to_complete_lines(
            &long_line,
            CompleteLineTruncateOptions::shared_defaults(),
        );
        match outcome {
            CompleteLineTruncateOutcome::Truncated(prefix) => {
                assert_eq!(prefix.text, long_line);
                assert_eq!(prefix.returned_lines, 1);
                assert_eq!(prefix.last_returned_line, 1);
                assert_eq!(prefix.next_start_line, Some(2));
                assert!(prefix.truncated);
            }
            other => panic!("expected truncated soft-over full content, got {other:?}"),
        }
    }

    #[test]
    fn truncate_to_complete_lines_single_line_over_hard_is_error_outcome() {
        let long_line = "y".repeat(TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT + 1);
        let outcome =
            truncate_to_complete_lines(&long_line, CompleteLineTruncateOptions::shared_defaults());
        match outcome {
            CompleteLineTruncateOutcome::SingleLineExceedsHardLimit {
                line_number,
                line_bytes,
                hard_limit_bytes,
            } => {
                assert_eq!(line_number, 1);
                assert_eq!(line_bytes, long_line.len());
                assert_eq!(hard_limit_bytes, TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT);
            }
            other => panic!("expected hard single-line failure, got {other:?}"),
        }
        assert!(outcome.single_line_hard_limit_error().is_some());
    }

    #[test]
    fn truncate_to_complete_lines_untruncated_omits_next_start_line() {
        let outcome = truncate_to_complete_lines(
            "one\ntwo\n",
            CompleteLineTruncateOptions::shared_defaults(),
        );
        let prefix = outcome.as_prefix().expect("full");
        assert!(!prefix.truncated);
        assert_eq!(prefix.next_start_line, None);
        assert!(matches!(outcome, CompleteLineTruncateOutcome::Full(_)));
        assert!(outcome.model_note().is_none());
    }

    #[test]
    fn truncate_to_complete_lines_respects_cr_only_line_endings() {
        // Review P1: lone `\r` is a line ending (same as line_spans); soft line limit must apply.
        let content = "x\r".repeat(TOOL_OUTPUT_SOFT_LINE_LIMIT + 500);
        assert_eq!(
            complete_line_count(&content),
            TOOL_OUTPUT_SOFT_LINE_LIMIT + 500
        );
        let outcome = truncate_to_complete_lines(
            &content,
            CompleteLineTruncateOptions {
                soft_byte_limit: TOOL_OUTPUT_SOFT_BYTE_LIMIT,
                soft_line_limit: TOOL_OUTPUT_SOFT_LINE_LIMIT,
                hard_byte_limit: TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT,
                content_start_line: 1,
            },
        );
        match outcome {
            CompleteLineTruncateOutcome::Truncated(prefix) => {
                assert_eq!(prefix.returned_lines, TOOL_OUTPUT_SOFT_LINE_LIMIT);
                assert_eq!(prefix.last_returned_line, TOOL_OUTPUT_SOFT_LINE_LIMIT);
                assert_eq!(
                    prefix.next_start_line,
                    Some(TOOL_OUTPUT_SOFT_LINE_LIMIT + 1)
                );
                assert_eq!(
                    complete_line_count(&prefix.text),
                    TOOL_OUTPUT_SOFT_LINE_LIMIT
                );
                assert!(prefix.text.ends_with('\r'));
                assert!(!prefix.text.contains('\n'));
            }
            other => panic!("expected truncated CR-only prefix, got {other:?}"),
        }
    }

    #[test]
    fn truncate_to_complete_lines_crlf_counts_as_single_ending() {
        let content = "a\r\nb\r\nc\r\n";
        let outcome = truncate_to_complete_lines(
            content,
            CompleteLineTruncateOptions {
                soft_byte_limit: content.len(),
                soft_line_limit: 2,
                hard_byte_limit: TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT,
                content_start_line: 5,
            },
        );
        match outcome {
            CompleteLineTruncateOutcome::Truncated(prefix) => {
                assert_eq!(prefix.text, "a\r\nb\r\n");
                assert_eq!(prefix.returned_lines, 2);
                assert_eq!(prefix.last_returned_line, 6);
                assert_eq!(prefix.next_start_line, Some(7));
            }
            other => panic!("expected truncated CRLF prefix, got {other:?}"),
        }
    }

    #[test]
    fn normalize_preserves_line_bounded_success_over_soft_under_hard() {
        let content = "z".repeat(TOOL_OUTPUT_SOFT_BYTE_LIMIT + 128);
        let execution = ToolExecution {
            output: json!({
                "content": content,
                "truncated": true,
                "nextStartLine": 2,
                "returnedLines": 1,
                "lastReturnedLine": 1,
                "note": "explicit truncated success",
            }),
            is_error: false,
        };
        let measured = serialized_json_size(&execution).expect("measure");
        assert!(measured > TOOL_OUTPUT_SOFT_BYTE_LIMIT);
        assert!(measured <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT);

        let budgeted = normalize_tool_execution(
            "read_file",
            ToolOutputSemantics::ReadOnly,
            execution.clone(),
        );
        assert_eq!(budgeted.state, ToolOutputBudgetState::WithinBudget);
        assert!(!budgeted.execution.is_error);
        assert_eq!(budgeted.execution.output["truncated"], true);
        assert_eq!(budgeted.execution.output["nextStartLine"], 2);
        assert_eq!(budgeted.execution, execution);
    }

    #[test]
    fn normalize_still_errors_read_only_soft_overage_without_truncated_field() {
        let execution = ToolExecution {
            output: json!({ "content": "x".repeat(TOOL_OUTPUT_SOFT_BYTE_LIMIT) }),
            is_error: false,
        };
        let budgeted =
            normalize_tool_execution("read_file", ToolOutputSemantics::ReadOnly, execution);
        assert!(budgeted.execution.is_error);
        assert_eq!(
            budgeted.state,
            ToolOutputBudgetState::ReadOnlyRecoverableFailure
        );
    }

    #[test]
    fn is_line_bounded_budget_success_requires_truncated_true_and_next_start_line() {
        assert!(!is_line_bounded_budget_success(&ToolExecution {
            output: json!({ "content": "ok" }),
            is_error: false,
        }));
        // truncated:false never preserves over-soft (including single-line markers).
        assert!(!is_line_bounded_budget_success(&ToolExecution {
            output: json!({ "content": "ok", "truncated": false, "nextStartLine": 2 }),
            is_error: false,
        }));
        assert!(!is_line_bounded_budget_success(&ToolExecution {
            output: json!({
                "content": "ok",
                "truncated": false,
                "returnedLines": 1,
                "lastReturnedLine": 1,
            }),
            is_error: false,
        }));
        assert!(!is_line_bounded_budget_success(&ToolExecution {
            output: json!({ "content": "ok", "truncated": true }),
            is_error: false,
        }));
        assert!(!is_line_bounded_budget_success(&ToolExecution {
            output: json!({ "content": "ok", "truncated": true, "nextStartLine": 0 }),
            is_error: false,
        }));
        assert!(!is_line_bounded_budget_success(&ToolExecution {
            output: json!({ "content": "ok", "truncated": true, "nextStartLine": 2 }),
            is_error: true,
        }));
        assert!(is_line_bounded_budget_success(&ToolExecution {
            output: json!({ "content": "ok", "truncated": true, "nextStartLine": 2 }),
            is_error: false,
        }));
        // Multi-line full result without truncation marker must not bypass soft limits.
        assert!(!is_line_bounded_budget_success(&ToolExecution {
            output: json!({
                "content": "ok",
                "truncated": false,
                "returnedLines": 2,
                "lastReturnedLine": 2,
            }),
            is_error: false,
        }));
    }

    #[test]
    fn normalize_soft_overage_with_truncated_false_still_errors_read_only() {
        let execution = ToolExecution {
            output: json!({
                "content": "x".repeat(TOOL_OUTPUT_SOFT_BYTE_LIMIT),
                "truncated": false,
            }),
            is_error: false,
        };
        let budgeted =
            normalize_tool_execution("read_file", ToolOutputSemantics::ReadOnly, execution);
        assert!(budgeted.execution.is_error);
        assert_eq!(
            budgeted.state,
            ToolOutputBudgetState::ReadOnlyRecoverableFailure
        );
    }

    #[test]
    fn shared_defaults_use_output_budget_constants() {
        let options = CompleteLineTruncateOptions::shared_defaults();
        assert_eq!(options.soft_byte_limit, TOOL_OUTPUT_SOFT_BYTE_LIMIT);
        assert_eq!(options.soft_line_limit, TOOL_OUTPUT_SOFT_LINE_LIMIT);
        assert_eq!(
            options.hard_byte_limit,
            TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
        );
        assert_eq!(options.content_start_line, 1);
    }
}
