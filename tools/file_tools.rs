use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use foco_store::workspace::{WORKSPACE_FOCO_DIR, workspace_foco_dir};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::output_budget::{
    CompleteLinePrefix, CompleteLineTruncateOptions, CompleteLineTruncateOutcome,
    LINE_BOUNDED_LAST_RETURNED_LINE_FIELD, LINE_BOUNDED_NEXT_START_LINE_FIELD,
    LINE_BOUNDED_NOTE_FIELD, LINE_BOUNDED_RETURNED_LINES_FIELD, LINE_BOUNDED_TRUNCATED_FIELD,
    SKILL_MD_MAX_BYTES, TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT, TOOL_OUTPUT_SOFT_BYTE_LIMIT,
    TOOL_OUTPUT_SOFT_LINE_LIMIT, complete_line_count, complete_line_prefix_note, measure_tool_execution,
    path_is_skill_md, peel_last_complete_line, serialized_json_size,
    soft_limit_array_prefix_len_with_overhead, truncate_to_complete_lines_with_measure,
};
use crate::{
    CommandOutputLimits, DEFAULT_FILE_TOOL_TIMEOUT_MS, DEFAULT_SEARCH_TEXT_TIMEOUT_MS,
    DEFAULT_WRITE_FILE_TIMEOUT_MS, FIND_FILES_RESPONSE_OVERHEAD_BYTES, LineRange, MAX_FIND_ENTRIES,
    MAX_RANGED_READ_OUTPUT_BYTES, MAX_RANGED_READ_SOURCE_BYTES, MAX_SEARCH_RESULT_FILES,
    MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES, MAX_SEARCH_TEXT_LINE_BYTES, RIPGREP_PATH, SEARCH_RESULT_TTL,
    SEARCH_RESULTS_DIR, SEARCH_SNAPSHOT_VERSION, SEARCH_TEXT_RESPONSE_OVERHEAD_BYTES, TextEncoding,
    ToolCancellationToken, ToolExecution, count_text_lines, decode_text_file, encode_text_file,
    errors::{ToolRuntimeError, tool_timeout_ms},
    normalize_read_line_range, normalize_workspace_path_text, numbered_content, parse_arguments,
    parse_optional_line_range, read_line_range, relative_workspace_path, replace_line_range,
    resolve_workspace_file, resolve_workspace_write_path, run_command_with_timeout,
};

pub(crate) fn read_file(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    read_file_inner(workspace_path, arguments, false)
}

pub(crate) fn read_file_with_external_access(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    read_file_inner(workspace_path, arguments, true)
}

fn read_file_inner(
    workspace_path: &Path,
    arguments: Value,
    allow_external_access: bool,
) -> Result<Value, ToolRuntimeError> {
    let request: ReadFileInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_FILE_TOOL_TIMEOUT_MS)?;
    let requested_line_range = parse_optional_line_range(request.start_line, request.end_line)?;
    // read_file-only resolver: absolute paths inside the execution workspace are internal
    // reads. Shared normalize_workspace_path_text / resolve_workspace_path stay relative-only
    // for write/search/command/graph tools.
    let path = if allow_external_access {
        resolve_read_file_path(workspace_path, &request.path)?
    } else {
        resolve_internal_read_file_path(workspace_path, &request.path)?
    };
    let metadata = fs::metadata(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    if !metadata.is_file() {
        return Err(ToolRuntimeError::NotFile(path));
    }

    let is_skill_md = path_is_skill_md(&path);
    if is_skill_md {
        if requested_line_range.is_some() {
            return Err(ToolRuntimeError::InvalidArguments(format!(
                "read_file path '{}' targets SKILL.md, which must be read in full (startLine and endLine must both be null). Oversized skills cannot be reconstructed by stitching line ranges.",
                request.path
            )));
        }
        if metadata.len() > SKILL_MD_MAX_BYTES as u64 {
            return Err(ToolRuntimeError::InvalidArguments(format!(
                "read_file path '{}' is a SKILL.md that exceeds the maximum size ({} bytes; max {} bytes). The document must fit entirely under the limit; partial reads are not allowed.",
                request.path,
                metadata.len(),
                SKILL_MD_MAX_BYTES
            )));
        }
    }

    // Ordinary full and ranged reads share the 32 MiB text source protection. Soft output
    // truncation is applied after decode (complete-line prefixes), not by refusing the source.
    let max_source_bytes = if is_skill_md {
        SKILL_MD_MAX_BYTES as u64
    } else {
        MAX_RANGED_READ_SOURCE_BYTES
    };

    if metadata.len() > max_source_bytes {
        return Err(ToolRuntimeError::FileTooLarge {
            path,
            bytes: metadata.len(),
            max_bytes: max_source_bytes,
        });
    }

    let bytes = fs::read(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;
    if is_skill_md && bytes.len() > SKILL_MD_MAX_BYTES {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "read_file path '{}' is a SKILL.md that exceeds the maximum size ({} bytes; max {} bytes). The document must fit entirely under the limit; partial reads are not allowed.",
            request.path,
            bytes.len(),
            SKILL_MD_MAX_BYTES
        )));
    }

    let (content, _) = decode_text_file(&path, &bytes)?;
    if is_skill_md && content.len() > SKILL_MD_MAX_BYTES {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "read_file path '{}' is a SKILL.md that exceeds the maximum size ({} UTF-8 bytes; max {} bytes). The document must fit entirely under the limit; partial reads are not allowed.",
            request.path,
            content.len(),
            SKILL_MD_MAX_BYTES
        )));
    }

    let line_range = if let Some(range) = requested_line_range {
        Some(normalize_read_line_range(
            range,
            count_text_lines(&content),
        )?)
    } else {
        None
    };
    let content = if let Some(range) = &line_range {
        read_line_range(&content, range)
    } else {
        content
    };
    let content_start_line = line_range.as_ref().map(|range| range.start).unwrap_or(1);

    if is_skill_md {
        // SKILL.md integrity: full document only, never complete-line soft truncation.
        let numbered = numbered_content(&content, content_start_line);
        if numbered.len() > MAX_RANGED_READ_OUTPUT_BYTES {
            return Err(ToolRuntimeError::InvalidArguments(format!(
                "read_file path '{}' SKILL.md output is too large ({} bytes; max {MAX_RANGED_READ_OUTPUT_BYTES}).",
                request.path,
                numbered.len()
            )));
        }
        return Ok(json!({
            "path": request.path,
            "content": numbered,
            "bytes": metadata.len(),
            "startLine": Value::Null,
            "endLine": Value::Null,
            "timeoutMs": timeout_ms
        }));
    }

    // Ordinary files: budget complete numbered lines without building a huge intermediate string.
    build_ordinary_read_file_response(
        &request.path,
        &content,
        content_start_line,
        line_range.as_ref(),
        metadata.len(),
        timeout_ms,
    )
}

/// Numbered line cost in the final JSON string body: `"<lineNo>\t"` prefix plus the line slice
/// (including its trailing newline), counted with JSON escape expansion so soft budgeting tracks
/// the serialized ToolExecution envelope rather than raw UTF-8 only.
fn numbered_line_measure(line_no: usize, line: &str) -> usize {
    let mut prefix = line_no.to_string();
    prefix.push('\t');
    json_string_body_len(&prefix).saturating_add(json_string_body_len(line))
}

/// UTF-8 length of a JSON string body (without surrounding quotes), matching serde_json escaping.
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

/// Measure ToolExecution JSON size for a `read_file` response with empty content and a
/// worst-case truncation note so content budgeting accounts for the real path/metadata.
fn measure_read_file_response_overhead(
    request_path: &str,
    line_range: Option<&LineRange>,
    source_bytes: u64,
    timeout_ms: u64,
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
    });
    let skeleton = json!({
        "path": request_path,
        "content": "",
        "bytes": source_bytes,
        "startLine": line_range.map(|range| range.start),
        "endLine": line_range.map(|range| range.end),
        "timeoutMs": timeout_ms,
        LINE_BOUNDED_TRUNCATED_FIELD: true,
        LINE_BOUNDED_RETURNED_LINES_FIELD: TOOL_OUTPUT_SOFT_LINE_LIMIT,
        LINE_BOUNDED_LAST_RETURNED_LINE_FIELD: last,
        LINE_BOUNDED_NEXT_START_LINE_FIELD: next,
        LINE_BOUNDED_NOTE_FIELD: note,
    });
    let execution = ToolExecution {
        output: skeleton,
        is_error: false,
    };
    serialized_json_size(&execution).map_err(|error| {
        ToolRuntimeError::InvalidArguments(format!(
            "read_file path '{request_path}': failed to measure response budget ({error})"
        ))
    })
}

fn assemble_read_file_response(
    request_path: &str,
    numbered: &str,
    line_range: Option<&LineRange>,
    source_bytes: u64,
    timeout_ms: u64,
    prefix: &CompleteLinePrefix,
) -> Value {
    let mut response = json!({
        "path": request_path,
        "content": numbered,
        "bytes": source_bytes,
        "startLine": line_range.map(|range| range.start),
        "endLine": line_range.map(|range| range.end),
        "timeoutMs": timeout_ms,
        LINE_BOUNDED_TRUNCATED_FIELD: prefix.truncated,
        LINE_BOUNDED_RETURNED_LINES_FIELD: prefix.returned_lines,
        LINE_BOUNDED_LAST_RETURNED_LINE_FIELD: prefix.last_returned_line,
        LINE_BOUNDED_NEXT_START_LINE_FIELD: prefix.next_start_line,
    });
    if prefix.truncated {
        response[LINE_BOUNDED_NOTE_FIELD] = Value::String(complete_line_prefix_note(prefix));
    }
    response
}

fn build_ordinary_read_file_response(
    request_path: &str,
    content: &str,
    content_start_line: usize,
    line_range: Option<&LineRange>,
    source_bytes: u64,
    timeout_ms: u64,
) -> Result<Value, ToolRuntimeError> {
    let content_start_line = content_start_line.max(1);

    if content.is_empty() {
        let prefix = CompleteLinePrefix {
            text: String::new(),
            returned_lines: 0,
            last_returned_line: content_start_line,
            next_start_line: None,
            truncated: false,
        };
        return Ok(assemble_read_file_response(
            request_path,
            "",
            line_range,
            source_bytes,
            timeout_ms,
            &prefix,
        ));
    }

    // Dynamic overhead from the actual path and response metadata (not a fixed 3 KiB pad).
    let overhead = measure_read_file_response_overhead(
        request_path,
        line_range,
        source_bytes,
        timeout_ms,
        content_start_line,
    )?;
    let soft_byte_limit = TOOL_OUTPUT_SOFT_BYTE_LIMIT.saturating_sub(overhead);
    let hard_byte_limit = TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT.saturating_sub(overhead);

    // Reserve complete lines for non-content string fields (path, optional note) so the final
    // ToolExecution measurement stays within the shared soft line limit when truncated=false.
    let path_lines = complete_line_count(request_path).max(1);
    let metadata_line_reserve = path_lines.saturating_add(2); // note + margin
    let soft_line_limit = TOOL_OUTPUT_SOFT_LINE_LIMIT
        .saturating_sub(metadata_line_reserve)
        .max(1);

    let options = CompleteLineTruncateOptions {
        soft_byte_limit,
        soft_line_limit,
        hard_byte_limit: hard_byte_limit.max(1),
        content_start_line,
    };

    let outcome = truncate_to_complete_lines_with_measure(content, options, numbered_line_measure);

    match outcome {
        CompleteLineTruncateOutcome::SingleLineExceedsHardLimit {
            line_number,
            line_bytes,
            hard_limit_bytes,
        } => Err(ToolRuntimeError::InvalidArguments(format!(
            "read_file path '{request_path}': a single complete line (line {line_number}, {line_bytes} numbered UTF-8 bytes) exceeds the hard output ceiling ({hard_limit_bytes} bytes) and cannot be returned without splitting the line. Refine the source or request a different range."
        ))),
        CompleteLineTruncateOutcome::Full(prefix)
        | CompleteLineTruncateOutcome::Truncated(prefix) => {
            fit_read_file_response_to_envelope(
                request_path,
                content,
                content_start_line,
                line_range,
                source_bytes,
                timeout_ms,
                prefix,
            )
        }
    }
}

/// Build the final response and, if the serialized ToolExecution still exceeds the soft budget,
/// peel complete lines until it fits. A single soft-over/hard-under line is kept with
/// `truncated: true` so normalize preserves the success.
fn fit_read_file_response_to_envelope(
    request_path: &str,
    full_content: &str,
    content_start_line: usize,
    line_range: Option<&LineRange>,
    source_bytes: u64,
    timeout_ms: u64,
    mut prefix: CompleteLinePrefix,
) -> Result<Value, ToolRuntimeError> {
    // Peel at most soft_line_limit + 2 times (guard against pathological loops).
    for _ in 0..=TOOL_OUTPUT_SOFT_LINE_LIMIT.saturating_add(2) {
        let numbered = numbered_content(&prefix.text, content_start_line);
        let response = assemble_read_file_response(
            request_path,
            &numbered,
            line_range,
            source_bytes,
            timeout_ms,
            &prefix,
        );
        let execution = ToolExecution {
            output: response.clone(),
            is_error: false,
        };
        let measurement = measure_tool_execution(&execution).map_err(|error| {
            ToolRuntimeError::InvalidArguments(format!(
                "read_file path '{request_path}': failed to measure response ({error})"
            ))
        })?;

        let within_soft = measurement.serialized_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT
            && measurement.text_lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT;
        if within_soft {
            return Ok(response);
        }

        if measurement.serialized_bytes > TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT {
            if prefix.returned_lines <= 1 {
                return Err(ToolRuntimeError::InvalidArguments(format!(
                    "read_file path '{request_path}': a single complete line exceeds the hard output ceiling ({} bytes measured; max {}) and cannot be returned without splitting the line. Refine the source or request a different range.",
                    measurement.serialized_bytes, TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
                )));
            }
        } else if prefix.returned_lines <= 1 {
            // Soft-over single complete line under hard: force truncated markers and keep.
            if !prefix.truncated {
                prefix.truncated = true;
                prefix.next_start_line = Some(prefix.last_returned_line.saturating_add(1));
                continue;
            }
            return Ok(response);
        }

        // Peel one complete line from the prefix and re-measure.
        let Some(peeled) = peel_last_complete_line(&prefix.text) else {
            return Ok(response);
        };
        if peeled.len() == prefix.text.len() {
            return Ok(response);
        }
        let more_after_peel = peeled.len() < full_content.len();
        prefix.text = peeled.to_string();
        prefix.returned_lines = prefix.returned_lines.saturating_sub(1);
        prefix.last_returned_line = prefix.last_returned_line.saturating_sub(1).max(content_start_line);
        prefix.truncated = more_after_peel || prefix.returned_lines > 0;
        // After peeling there is always remaining content relative to the original request
        // when we started over budget with multiple lines; mark truncated for continuation.
        prefix.truncated = true;
        prefix.next_start_line = Some(prefix.last_returned_line.saturating_add(1));
        if prefix.returned_lines == 0 {
            return Err(ToolRuntimeError::InvalidArguments(format!(
                "read_file path '{request_path}': unable to fit any complete line under the shared soft output budget after accounting for path and metadata. Use a shorter path or a narrower range."
            )));
        }
    }

    Err(ToolRuntimeError::InvalidArguments(format!(
        "read_file path '{request_path}': unable to fit response under the shared soft output budget."
    )))
}

fn resolve_read_file_path(workspace_path: &Path, input: &str) -> Result<PathBuf, ToolRuntimeError> {
    resolve_internal_read_file_path(workspace_path, input)
        .or_else(|_| resolve_external_read_file_path(workspace_path, input))
}

/// Resolve a `read_file` path that must land inside the execution workspace root.
///
/// Accepts workspace-relative paths (same rules as other tools) and absolute paths whose
/// canonical target is a component-level member of the canonical workspace root. Symlink
/// escape targets outside the root are rejected as internal paths.
fn resolve_internal_read_file_path(
    workspace_path: &Path,
    input: &str,
) -> Result<PathBuf, ToolRuntimeError> {
    let path = resolve_internal_read_path(workspace_path, input)?;
    let metadata = fs::metadata(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    if metadata.is_file() {
        Ok(path)
    } else {
        Err(ToolRuntimeError::NotFile(path))
    }
}

fn resolve_internal_read_path(
    workspace_path: &Path,
    input: &str,
) -> Result<PathBuf, ToolRuntimeError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ToolRuntimeError::InvalidPath(
            "path must not be empty".to_string(),
        ));
    }

    let requested = Path::new(trimmed);
    let workspace = fs::canonicalize(workspace_path).map_err(|source| ToolRuntimeError::Io {
        path: workspace_path.to_path_buf(),
        source,
    })?;

    let path = if requested.is_absolute() {
        fs::canonicalize(requested).map_err(|source| ToolRuntimeError::Io {
            path: requested.to_path_buf(),
            source,
        })?
    } else {
        // Relative inputs keep the same component checks as other workspace tools.
        let normalized = normalize_workspace_path_text(trimmed)?;
        let joined = workspace.join(Path::new(&normalized));
        fs::canonicalize(&joined).map_err(|source| ToolRuntimeError::Io {
            path: joined,
            source,
        })?
    };

    if !path.starts_with(&workspace) {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "path escapes the workspace: {trimmed}"
        )));
    }

    Ok(path)
}

fn resolve_external_read_file_path(
    workspace_path: &Path,
    input: &str,
) -> Result<PathBuf, ToolRuntimeError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ToolRuntimeError::InvalidPath(
            "path must not be empty".to_string(),
        ));
    }

    let requested = Path::new(trimmed);
    let path = if requested.is_absolute() {
        fs::canonicalize(requested).map_err(|source| ToolRuntimeError::Io {
            path: requested.to_path_buf(),
            source,
        })?
    } else {
        let joined = workspace_path.join(requested);
        fs::canonicalize(&joined).map_err(|source| ToolRuntimeError::Io {
            path: joined,
            source,
        })?
    };

    let workspace = fs::canonicalize(workspace_path).map_err(|source| ToolRuntimeError::Io {
        path: workspace_path.to_path_buf(),
        source,
    })?;
    if path.starts_with(&workspace) {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "path is inside workspace: {}",
            path.display()
        )));
    }

    Ok(path)
}

pub(crate) fn read_file_target_outside_workspace(
    workspace_path: &Path,
    input: &str,
) -> Result<Option<PathBuf>, ToolRuntimeError> {
    match resolve_external_read_file_path(workspace_path, input) {
        Ok(path) => Ok(Some(path)),
        // Absolute or relative internal paths resolve here and are not external targets.
        Err(_) => resolve_internal_read_file_path(workspace_path, input).map(|_| None),
    }
}

/// Classify a `find_files` directory path: `Some(canonical)` when outside the execution root.
pub(crate) fn find_files_target_outside_workspace(
    workspace_path: &Path,
    input: &str,
) -> Result<Option<PathBuf>, ToolRuntimeError> {
    match resolve_external_path(workspace_path, input) {
        Ok(path) => {
            let metadata = fs::metadata(&path).map_err(|source| ToolRuntimeError::Io {
                path: path.clone(),
                source,
            })?;
            if metadata.is_dir() {
                Ok(Some(path))
            } else {
                Err(ToolRuntimeError::NotDirectory(path))
            }
        }
        Err(_) => resolve_internal_find_files_directory(workspace_path, input).map(|_| None),
    }
}

/// Classify a `search_text` path (file or directory): `Some(canonical)` when outside the root.
pub(crate) fn search_text_target_outside_workspace(
    workspace_path: &Path,
    input: &str,
) -> Result<Option<PathBuf>, ToolRuntimeError> {
    match resolve_external_path(workspace_path, input) {
        Ok(path) => Ok(Some(path)),
        Err(_) => resolve_internal_read_path(workspace_path, input).map(|_| None),
    }
}

/// Absolute-path external target that is not under the execution workspace root.
/// Shared by `read_file` (file-only wrappers) and find/search directory-or-file roots.
fn resolve_external_path(workspace_path: &Path, input: &str) -> Result<PathBuf, ToolRuntimeError> {
    resolve_external_read_file_path(workspace_path, input)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathListMode {
    /// Entry / match paths are workspace-relative (default internal behavior).
    WorkspaceRelative,
    /// Entry / match paths are canonical absolute paths under an external search root.
    AbsoluteExternal,
}

pub(crate) fn find_files(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    find_files_inner(workspace_path, arguments, false)
}

pub(crate) fn find_files_with_external_access(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    find_files_inner(workspace_path, arguments, true)
}

fn find_files_inner(
    workspace_path: &Path,
    arguments: Value,
    allow_external_access: bool,
) -> Result<Value, ToolRuntimeError> {
    let request: FindFilesInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_FILE_TOOL_TIMEOUT_MS)?;
    let input_path = request.path;
    // find_files-only resolver: relative + absolute-in-execution-root are internal.
    // Outside absolute directories require allow_external_access. Does not use
    // normalize_workspace_path_text / resolve_workspace_path (write/command/graph stay relative-only).
    let (path, list_mode) =
        resolve_find_files_directory(workspace_path, &input_path, allow_external_access)?;
    let filter = GlobFilter::new(request.include, request.exclude)?;
    let metadata = fs::metadata(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    if !metadata.is_dir() {
        return Err(ToolRuntimeError::NotDirectory(path));
    }

    // Glob match/prune paths are always relative to the search root (`path`).
    // Reported entry paths are workspace-relative (internal) or absolute (external).
    let mut entries = Vec::new();
    find_files_in_directory(
        workspace_path,
        &path,
        &path,
        &filter,
        list_mode,
        &mut entries,
    )?;

    entries.sort_by(|left, right| {
        left.get("path")
            .and_then(Value::as_str)
            .cmp(&right.get("path").and_then(Value::as_str))
    });
    let hard_truncated = entries.len() > MAX_FIND_ENTRIES;
    if hard_truncated {
        entries.truncate(MAX_FIND_ENTRIES);
    }
    let hard_capped_len = entries.len();
    let preview_count =
        soft_limit_array_prefix_len_with_overhead(&entries, FIND_FILES_RESPONSE_OVERHEAD_BYTES)
            .map_err(|source| {
                ToolRuntimeError::InvalidArguments(format!(
                    "failed to measure find_files result size: {source}"
                ))
            })?;
    let soft_truncated = preview_count < hard_capped_len;
    let returned_entries = entries[..preview_count].to_vec();
    let truncated = hard_truncated || soft_truncated;
    let mut response = json!({
        "path": input_path,
        "include": filter.include_patterns(),
        "exclude": filter.exclude_patterns(),
        "entries": returned_entries,
        "truncated": truncated,
        "totalEntries": hard_capped_len,
        "returnedEntries": preview_count,
        "timeoutMs": timeout_ms
    });
    if soft_truncated {
        response["note"] = Value::String(format!(
            "find_files returned the first {preview_count} of {hard_capped_len} collected entries under the soft output budget (max {TOOL_OUTPUT_SOFT_BYTE_LIMIT} bytes or {TOOL_OUTPUT_SOFT_LINE_LIMIT} lines). Narrow include/exclude or path, or re-run after refining globs; results are sorted by path."
        ));
        response["retryable"] = Value::Bool(true);
    } else if hard_truncated {
        response["note"] = Value::String(format!(
            "find_files stopped after collecting {MAX_FIND_ENTRIES} entries. Narrow include/exclude or path."
        ));
        response["retryable"] = Value::Bool(true);
    }

    Ok(response)
}

fn resolve_find_files_directory(
    workspace_path: &Path,
    input: &str,
    allow_external_access: bool,
) -> Result<(PathBuf, PathListMode), ToolRuntimeError> {
    if allow_external_access {
        resolve_find_files_directory_with_external(workspace_path, input)
    } else {
        resolve_internal_find_files_directory(workspace_path, input)
            .map(|path| (path, PathListMode::WorkspaceRelative))
    }
}

fn resolve_find_files_directory_with_external(
    workspace_path: &Path,
    input: &str,
) -> Result<(PathBuf, PathListMode), ToolRuntimeError> {
    match resolve_internal_find_files_directory(workspace_path, input) {
        Ok(path) => Ok((path, PathListMode::WorkspaceRelative)),
        Err(_) => resolve_external_path(workspace_path, input)
            .map(|path| (path, PathListMode::AbsoluteExternal)),
    }
}

fn resolve_internal_find_files_directory(
    workspace_path: &Path,
    input: &str,
) -> Result<PathBuf, ToolRuntimeError> {
    let path = resolve_internal_read_path(workspace_path, input)?;
    let metadata = fs::metadata(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;
    if metadata.is_dir() {
        Ok(path)
    } else {
        Err(ToolRuntimeError::NotDirectory(path))
    }
}

const INTERNAL_FIND_FILES_EXCLUDE_PATTERNS: &[&str] = &[".foco", ".foco/**"];

fn find_files_in_directory(
    report_root: &Path,
    search_root: &Path,
    directory_path: &Path,
    filter: &GlobFilter,
    list_mode: PathListMode,
    entries: &mut Vec<Value>,
) -> Result<(), ToolRuntimeError> {
    let mut directory_entries = fs::read_dir(directory_path)
        .map_err(|source| ToolRuntimeError::Io {
            path: directory_path.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| ToolRuntimeError::Io {
            path: directory_path.to_path_buf(),
            source,
        })?;
    directory_entries.sort_by_key(|entry| entry.path());

    for entry in directory_entries {
        if entries.len() > MAX_FIND_ENTRIES {
            return Ok(());
        }
        let entry_path = entry.path();
        // Glob include/exclude/prune always use paths relative to the search root.
        let glob_relative_path = relative_path_under_root(search_root, &entry_path)?;
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(source) if source.kind() == io::ErrorKind::NotFound => continue,
            Err(source) => {
                return Err(ToolRuntimeError::Io {
                    path: entry_path.clone(),
                    source,
                });
            }
        };

        if file_type.is_dir() && filter.prunes_directory(&glob_relative_path) {
            continue;
        }

        let file_bytes = if file_type.is_file() {
            match entry.metadata() {
                Ok(metadata) => Some(metadata.len()),
                Err(source) if source.kind() == io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(ToolRuntimeError::Io {
                        path: entry_path.clone(),
                        source,
                    });
                }
            }
        } else {
            None
        };
        let kind = if file_type.is_dir() {
            "directory"
        } else if file_type.is_file() {
            "file"
        } else {
            "other"
        };

        if filter.matches(&glob_relative_path) {
            let reported_path = match list_mode {
                // Internal results stay workspace-relative even when search starts in a subdirectory.
                PathListMode::WorkspaceRelative => {
                    relative_path_under_root(report_root, &entry_path)?
                }
                PathListMode::AbsoluteExternal => absolute_path_string(&entry_path)?,
            };
            entries.push(json!({
                "path": reported_path,
                "kind": kind,
                "bytes": file_bytes
            }));
        }

        if file_type.is_dir() {
            find_files_in_directory(
                report_root,
                search_root,
                &entry_path,
                filter,
                list_mode,
                entries,
            )?;
        }
    }

    Ok(())
}

fn relative_path_under_root(root: &Path, path: &Path) -> Result<String, ToolRuntimeError> {
    // Same strip semantics as relative_workspace_path, but the root may be an external search dir.
    relative_workspace_path(root, path)
}

fn absolute_path_string(path: &Path) -> Result<String, ToolRuntimeError> {
    let canonical = fs::canonicalize(path).map_err(|source| ToolRuntimeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(canonical.to_string_lossy().replace('\\', "/"))
}

struct GlobFilter {
    include: Vec<String>,
    exclude: Vec<String>,
    include_set: Option<GlobSet>,
    exclude_set: Option<GlobSet>,
    prune_set: Option<GlobSet>,
}

impl GlobFilter {
    fn new(
        include: Option<Vec<String>>,
        exclude: Option<Vec<String>>,
    ) -> Result<Self, ToolRuntimeError> {
        let include = normalize_glob_patterns("include", include)?;
        let exclude = normalize_glob_patterns("exclude", exclude)?;
        let effective_exclude = effective_exclude_patterns(&exclude);
        let prune_patterns = directory_prune_patterns(&effective_exclude);
        let include_set = compile_glob_set("include", &include)?;
        let exclude_set = compile_glob_set("exclude", &effective_exclude)?;
        let prune_set = compile_glob_set("exclude", &prune_patterns)?;

        Ok(Self {
            include,
            exclude,
            include_set,
            exclude_set,
            prune_set,
        })
    }

    fn matches(&self, path: &str) -> bool {
        if let Some(include_set) = &self.include_set
            && !include_set.is_match(path)
        {
            return false;
        }

        if let Some(exclude_set) = &self.exclude_set
            && exclude_set.is_match(path)
        {
            return false;
        }

        true
    }

    fn prunes_directory(&self, path: &str) -> bool {
        self.prune_set
            .as_ref()
            .is_some_and(|prune_set| prune_set.is_match(path))
    }

    fn include_patterns(&self) -> &[String] {
        &self.include
    }

    fn exclude_patterns(&self) -> &[String] {
        &self.exclude
    }
}

fn normalize_glob_patterns(
    field_name: &str,
    patterns: Option<Vec<String>>,
) -> Result<Vec<String>, ToolRuntimeError> {
    patterns
        .unwrap_or_default()
        .into_iter()
        .map(|pattern| {
            let pattern = pattern.trim();
            if pattern.is_empty() {
                return Err(ToolRuntimeError::InvalidArguments(format!(
                    "{field_name} glob patterns must not be empty"
                )));
            }

            Ok(pattern.replace('\\', "/"))
        })
        .collect()
}

fn effective_exclude_patterns(exclude: &[String]) -> Vec<String> {
    INTERNAL_FIND_FILES_EXCLUDE_PATTERNS
        .iter()
        .map(|pattern| (*pattern).to_string())
        .chain(exclude.iter().cloned())
        .collect()
}

fn directory_prune_patterns(exclude: &[String]) -> Vec<String> {
    let mut patterns = Vec::new();
    for pattern in exclude {
        patterns.push(pattern.clone());
        if let Some(parent_pattern) = pattern.strip_suffix("/**")
            && !parent_pattern.is_empty()
        {
            patterns.push(parent_pattern.to_string());
        }
    }
    patterns
}

fn compile_glob_set(
    field_name: &str,
    patterns: &[String],
) -> Result<Option<GlobSet>, ToolRuntimeError> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|source| {
            ToolRuntimeError::InvalidArguments(format!(
                "{field_name} glob pattern '{pattern}' is invalid: {source}"
            ))
        })?;
        builder.add(glob);
    }

    builder.build().map(Some).map_err(|source| {
        ToolRuntimeError::InvalidArguments(format!(
            "{field_name} glob patterns are invalid: {source}"
        ))
    })
}

pub(crate) fn search_text(
    workspace_path: &Path,
    arguments: Value,
    cancellation_token: Option<&ToolCancellationToken>,
) -> Result<Value, ToolRuntimeError> {
    search_text_inner(workspace_path, arguments, cancellation_token, false)
}

pub(crate) fn search_text_with_external_access(
    workspace_path: &Path,
    arguments: Value,
    cancellation_token: Option<&ToolCancellationToken>,
) -> Result<Value, ToolRuntimeError> {
    search_text_inner(workspace_path, arguments, cancellation_token, true)
}

fn search_text_inner(
    workspace_path: &Path,
    arguments: Value,
    cancellation_token: Option<&ToolCancellationToken>,
    allow_external_access: bool,
) -> Result<Value, ToolRuntimeError> {
    let request: SearchTextInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_SEARCH_TEXT_TIMEOUT_MS)?;
    let pattern = request.query.trim();
    if pattern.is_empty() {
        return Err(ToolRuntimeError::InvalidArguments(
            "query must not be empty".to_string(),
        ));
    }
    let input_path = request.path.trim();
    if input_path.is_empty() {
        return Err(ToolRuntimeError::InvalidArguments(
            "path must not be empty".to_string(),
        ));
    }

    // Treat missing, null, empty, and whitespace-only continuation as a fresh search.
    // Only a non-empty token (after trim) enters the snapshot pagination path.
    // Continuation pages do not re-check external authorization: the snapshot was created
    // only after an authorized initial search and is stored under the execution workspace.
    if let Some(continuation) = request
        .continuation
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        return search_text_continue(
            workspace_path,
            pattern,
            input_path,
            continuation,
            timeout_ms,
        );
    }

    search_text_initial(
        workspace_path,
        pattern,
        input_path,
        timeout_ms,
        cancellation_token,
        allow_external_access,
    )
}

fn search_text_initial(
    workspace_path: &Path,
    pattern: &str,
    input_path: &str,
    timeout_ms: u64,
    cancellation_token: Option<&ToolCancellationToken>,
    allow_external_access: bool,
) -> Result<Value, ToolRuntimeError> {
    // search_text-only resolver (file or directory). Outside absolute targets require
    // allow_external_access and fail before running rg or writing a snapshot.
    let (path, list_mode) =
        resolve_search_text_path(workspace_path, input_path, allow_external_access)?;
    let rg_args = vec![
        "--json".to_string(),
        "--line-number".to_string(),
        "--max-columns".to_string(),
        MAX_SEARCH_TEXT_LINE_BYTES.to_string(),
        pattern.to_string(),
        path.to_string_lossy().to_string(),
    ];
    let rg_command = ripgrep_command();
    // cwd stays the execution workspace so snapshots land under workspace .foco/search-results/
    // even when searching an external path.
    let output = match run_command_with_timeout(
        &rg_command,
        &rg_args,
        workspace_path,
        Duration::from_millis(timeout_ms),
        cancellation_token,
        None,
        Some(CommandOutputLimits {
            stdout_bytes: Some(MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES),
            stderr_bytes: Some(MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES),
            output_delta_bytes: None,
            truncate: false,
        }),
    ) {
        Ok(output) => output,
        Err(ToolRuntimeError::CommandOutputTooLarge { bytes, .. }) => {
            return Err(search_text_too_many_matches_error(
                pattern,
                input_path,
                Some(bytes),
            ));
        }
        Err(error) => return Err(error),
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if output.status.code() == Some(1) {
            return Ok(json!({
                "query": pattern,
                "path": input_path,
                "matches": [],
                "truncated": false,
                "totalMatches": 0,
                "returnedMatches": 0,
                "timeoutMs": timeout_ms
            }));
        }

        return Err(ToolRuntimeError::CommandFailed {
            command: rg_command,
            status: output.status.code(),
            stderr,
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let incomplete = search_text_collection_is_incomplete(&output.stdout, &output.stderr);
    let mut entries = Vec::new();
    for line in stdout.lines() {
        let event: Value =
            serde_json::from_str(line).map_err(|source| ToolRuntimeError::InvalidToolOutput {
                command: "rg".to_string(),
                source,
            })?;

        if event.get("type").and_then(Value::as_str) != Some("match") {
            continue;
        }

        let data = event.get("data").ok_or_else(|| {
            ToolRuntimeError::InvalidArguments("rg match event is missing data".to_string())
        })?;
        let absolute_path = data
            .get("path")
            .and_then(|path| path.get("text"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ToolRuntimeError::InvalidArguments("rg match event is missing path".to_string())
            })?;
        let line_number = data.get("line_number").and_then(Value::as_u64);
        let text = data
            .get("lines")
            .and_then(|lines| lines.get("text"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim_end_matches(['\r', '\n'])
            .to_string();
        let reported_path = match list_mode {
            PathListMode::WorkspaceRelative => {
                relative_workspace_path(workspace_path, Path::new(absolute_path))?
            }
            PathListMode::AbsoluteExternal => absolute_path_string(Path::new(absolute_path))?,
        };

        entries.push(SearchMatch {
            path: reported_path,
            line: line_number,
            text,
        });
    }

    // Stable order for continuation pages (path, then line, then text).
    entries.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.text.cmp(&right.text))
    });

    if incomplete {
        return Err(search_text_incomplete_collection_error(pattern, input_path));
    }

    let total_matches = entries.len();
    if total_matches == 0 {
        return Ok(json!({
            "query": pattern,
            "path": input_path,
            "matches": [],
            "truncated": false,
            "totalMatches": 0,
            "returnedMatches": 0,
            "timeoutMs": timeout_ms
        }));
    }

    let match_values = entries.iter().map(SearchMatch::to_json).collect::<Vec<_>>();
    let returned = soft_limit_array_prefix_len_with_overhead(
        &match_values,
        SEARCH_TEXT_RESPONSE_OVERHEAD_BYTES,
    )
    .map_err(|source| {
        ToolRuntimeError::InvalidArguments(format!(
            "failed to measure search_text result size: {source}"
        ))
    })?;
    let returned = returned.max(1.min(total_matches));
    let matches = match_values[..returned].to_vec();

    if returned == total_matches {
        return Ok(json!({
            "query": pattern,
            "path": input_path,
            "matches": matches,
            "truncated": false,
            "totalMatches": total_matches,
            "returnedMatches": returned,
            "timeoutMs": timeout_ms
        }));
    }

    // Snapshots always live under the execution workspace (not the external search root).
    let snapshot_id =
        write_search_snapshot_file(workspace_path, pattern, input_path, &entries, false)?;
    let full_result_path = search_snapshot_text_relative_path(&snapshot_id);
    let next_offset = returned;
    let continuation = format!("{snapshot_id}:{next_offset}");
    let note = format!(
        "Results truncated under the soft output budget (max {TOOL_OUTPUT_SOFT_BYTE_LIMIT} bytes or {TOOL_OUTPUT_SOFT_LINE_LIMIT} lines): showing matches 0..{returned} of {total_matches} (stable path/line order). Use continuation='{continuation}' with the same query and path to page further without re-running the search. fullResultPath '{full_result_path}' is the same snapshot (plain text lines); read_file it with a small line range if needed."
    );

    Ok(json!({
        "query": pattern,
        "path": input_path,
        "matches": matches,
        "truncated": true,
        "totalMatches": total_matches,
        "returnedMatches": returned,
        "nextOffset": next_offset,
        "continuation": continuation,
        "fullResultPath": full_result_path,
        "note": note,
        "timeoutMs": timeout_ms
    }))
}

fn resolve_search_text_path(
    workspace_path: &Path,
    input: &str,
    allow_external_access: bool,
) -> Result<(PathBuf, PathListMode), ToolRuntimeError> {
    if allow_external_access {
        match resolve_internal_read_path(workspace_path, input) {
            Ok(path) => Ok((path, PathListMode::WorkspaceRelative)),
            Err(_) => resolve_external_path(workspace_path, input)
                .map(|path| (path, PathListMode::AbsoluteExternal)),
        }
    } else {
        resolve_internal_read_path(workspace_path, input)
            .map(|path| (path, PathListMode::WorkspaceRelative))
    }
}

fn search_text_continue(
    workspace_path: &Path,
    pattern: &str,
    input_path: &str,
    continuation: &str,
    timeout_ms: u64,
) -> Result<Value, ToolRuntimeError> {
    let (snapshot_id, offset) = parse_search_continuation(continuation)?;
    let snapshot = load_search_snapshot(workspace_path, &snapshot_id)?;
    if snapshot.version != SEARCH_SNAPSHOT_VERSION {
        return Err(search_continuation_invalid_error(
            "snapshot version is unsupported; re-run search_text without continuation",
        ));
    }
    if snapshot.query != pattern || snapshot.path != input_path {
        return Err(search_continuation_invalid_error(
            "continuation does not match the provided query/path binding; re-run search_text without continuation or pass the original query and path",
        ));
    }
    if offset > snapshot.matches.len() {
        return Err(search_continuation_invalid_error(
            "continuation offset is past the end of the snapshot; re-run search_text without continuation",
        ));
    }

    let total_matches = snapshot.matches.len();
    let remaining = &snapshot.matches[offset..];
    if remaining.is_empty() {
        return Ok(json!({
            "query": pattern,
            "path": input_path,
            "matches": [],
            "truncated": false,
            "totalMatches": total_matches,
            "returnedMatches": 0,
            "nextOffset": offset,
            "continuation": null,
            "fullResultPath": search_snapshot_text_relative_path(&snapshot_id),
            "note": "No more matches in this snapshot.",
            "timeoutMs": timeout_ms
        }));
    }

    let match_values = remaining
        .iter()
        .map(|entry| {
            json!({
                "path": entry.path,
                "line": entry.line,
                "text": entry.text
            })
        })
        .collect::<Vec<_>>();
    let returned = soft_limit_array_prefix_len_with_overhead(
        &match_values,
        SEARCH_TEXT_RESPONSE_OVERHEAD_BYTES,
    )
    .map_err(|source| {
        ToolRuntimeError::InvalidArguments(format!(
            "failed to measure search_text continuation size: {source}"
        ))
    })?;
    let returned = returned.max(1.min(match_values.len()));
    let matches = match_values[..returned].to_vec();
    let next_offset = offset + returned;
    let truncated = next_offset < total_matches;
    let full_result_path = search_snapshot_text_relative_path(&snapshot_id);
    let continuation_token = if truncated {
        Some(format!("{snapshot_id}:{next_offset}"))
    } else {
        None
    };
    let note = if truncated {
        format!(
            "Continuation page: showing matches {offset}..{next_offset} of {total_matches}. Use continuation='{}' with the same query and path for the next page.",
            continuation_token.as_deref().unwrap_or_default()
        )
    } else {
        format!(
            "Continuation page: showing matches {offset}..{next_offset} of {total_matches} (final page)."
        )
    };

    Ok(json!({
        "query": pattern,
        "path": input_path,
        "matches": matches,
        "truncated": truncated,
        "totalMatches": total_matches,
        "returnedMatches": returned,
        "nextOffset": next_offset,
        "continuation": continuation_token,
        "fullResultPath": full_result_path,
        "note": note,
        "timeoutMs": timeout_ms
    }))
}

struct SearchMatch {
    path: String,
    line: Option<u64>,
    text: String,
}

impl SearchMatch {
    fn to_json(&self) -> Value {
        json!({
            "path": self.path,
            "line": self.line,
            "text": self.text
        })
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchSnapshotFile {
    version: u32,
    query: String,
    path: String,
    incomplete: bool,
    matches: Vec<SearchSnapshotMatch>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchSnapshotMatch {
    path: String,
    line: Option<u64>,
    text: String,
}

impl From<&SearchMatch> for SearchSnapshotMatch {
    fn from(value: &SearchMatch) -> Self {
        Self {
            path: value.path.clone(),
            line: value.line,
            text: value.text.clone(),
        }
    }
}

impl From<SearchSnapshotMatch> for SearchMatch {
    fn from(value: SearchSnapshotMatch) -> Self {
        Self {
            path: value.path,
            line: value.line,
            text: value.text,
        }
    }
}

fn parse_search_continuation(continuation: &str) -> Result<(String, usize), ToolRuntimeError> {
    let trimmed = continuation.trim();
    let Some((snapshot_id, offset_text)) = trimmed.rsplit_once(':') else {
        return Err(search_continuation_invalid_error(
            "continuation must be '{snapshotId}:{offset}'",
        ));
    };
    if !is_valid_search_snapshot_id(snapshot_id) {
        return Err(search_continuation_invalid_error(
            "continuation snapshot id is invalid",
        ));
    }
    let offset = offset_text.parse::<usize>().map_err(|_| {
        search_continuation_invalid_error("continuation offset must be a non-negative integer")
    })?;
    Ok((snapshot_id.to_string(), offset))
}

fn is_valid_search_snapshot_id(snapshot_id: &str) -> bool {
    let Some(name) = snapshot_id.strip_prefix("search-") else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn search_snapshot_text_relative_path(snapshot_id: &str) -> String {
    format!("{WORKSPACE_FOCO_DIR}/{SEARCH_RESULTS_DIR}/{snapshot_id}.txt")
}

fn search_snapshot_file_path(workspace_path: &Path, snapshot_id: &str) -> PathBuf {
    workspace_foco_dir(workspace_path)
        .join(SEARCH_RESULTS_DIR)
        .join(format!("{snapshot_id}.json"))
}

fn write_search_snapshot_file(
    workspace_path: &Path,
    query: &str,
    path: &str,
    entries: &[SearchMatch],
    incomplete: bool,
) -> Result<String, ToolRuntimeError> {
    let results_dir = workspace_foco_dir(workspace_path).join(SEARCH_RESULTS_DIR);
    fs::create_dir_all(&results_dir).map_err(|source| ToolRuntimeError::Io {
        path: results_dir.clone(),
        source,
    })?;
    prune_search_results_dir(&results_dir);

    let snapshot_id = next_search_snapshot_id();
    let file_path = search_snapshot_file_path(workspace_path, &snapshot_id);
    let snapshot = SearchSnapshotFile {
        version: SEARCH_SNAPSHOT_VERSION,
        query: query.to_string(),
        path: path.to_string(),
        incomplete,
        matches: entries.iter().map(SearchSnapshotMatch::from).collect(),
    };
    let contents = serde_json::to_vec_pretty(&snapshot).map_err(|source| {
        ToolRuntimeError::InvalidArguments(format!(
            "failed to serialize search_text snapshot: {source}"
        ))
    })?;
    fs::write(&file_path, contents).map_err(|source| ToolRuntimeError::Io {
        path: file_path.clone(),
        source,
    })?;

    // Also write a human-readable line dump so fullResultPath can be read via read_file line ranges.
    let text_path = results_dir.join(format!("{snapshot_id}.txt"));
    let text_contents = render_search_results(entries);
    let _ = fs::write(&text_path, text_contents);

    Ok(snapshot_id)
}

fn load_search_snapshot(
    workspace_path: &Path,
    snapshot_id: &str,
) -> Result<SearchSnapshotFile, ToolRuntimeError> {
    if !is_valid_search_snapshot_id(snapshot_id) {
        return Err(search_continuation_invalid_error(
            "continuation snapshot id is invalid",
        ));
    }
    let file_path = search_snapshot_file_path(workspace_path, snapshot_id);
    let bytes = fs::read(&file_path).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            search_continuation_expired_error(
                "snapshot file is missing (expired, pruned after 1 hour, or removed by the 20-file cap); re-run search_text without continuation",
            )
        } else {
            ToolRuntimeError::Io {
                path: file_path.clone(),
                source,
            }
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|_| {
        search_continuation_invalid_error(
            "snapshot file is corrupt or invalid; re-run search_text without continuation",
        )
    })
}

fn render_search_results(entries: &[SearchMatch]) -> String {
    let mut rendered = String::new();
    for entry in entries {
        rendered.push_str(&entry.path);
        if let Some(line) = entry.line {
            rendered.push(':');
            rendered.push_str(&line.to_string());
        }
        rendered.push_str(": ");
        rendered.push_str(&entry.text);
        rendered.push('\n');
    }
    rendered
}

static SEARCH_RESULTS_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_search_snapshot_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let counter = SEARCH_RESULTS_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("search-{nanos}-{counter}")
}

/// Best-effort cleanup of stale search-result files: drops anything past the
/// retention window and caps the directory so a single new snapshot can be added
/// without exceeding the limit. All failures are ignored intentionally.
fn prune_search_results_dir(results_dir: &Path) {
    let Ok(read_dir) = fs::read_dir(results_dir) else {
        return;
    };

    let now = SystemTime::now();
    let mut files: Vec<(PathBuf, SystemTime)> = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        let is_result_file = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.starts_with("search-") && (name.ends_with(".json") || name.ends_with(".txt"))
            });
        if !is_result_file {
            continue;
        }

        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        if now
            .duration_since(modified)
            .is_ok_and(|age| age > SEARCH_RESULT_TTL)
        {
            let _ = fs::remove_file(&path);
            continue;
        }

        files.push((path, modified));
    }

    // Count unique snapshot ids (.json + .txt pairs) toward the cap.
    let mut snapshot_ids: Vec<(String, SystemTime)> = Vec::new();
    for (path, modified) in &files {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let id = name
            .strip_suffix(".json")
            .or_else(|| name.strip_suffix(".txt"))
            .unwrap_or(name)
            .to_string();
        if let Some((_, existing)) = snapshot_ids
            .iter_mut()
            .find(|(existing_id, _)| existing_id == &id)
        {
            if *modified > *existing {
                *existing = *modified;
            }
        } else {
            snapshot_ids.push((id, *modified));
        }
    }

    if snapshot_ids.len() < MAX_SEARCH_RESULT_FILES {
        return;
    }

    snapshot_ids.sort_by_key(|(_, modified)| *modified);
    let remove_count = snapshot_ids.len() + 1 - MAX_SEARCH_RESULT_FILES;
    for (id, _) in snapshot_ids.into_iter().take(remove_count) {
        let _ = fs::remove_file(results_dir.join(format!("{id}.json")));
        let _ = fs::remove_file(results_dir.join(format!("{id}.txt")));
    }
}

fn search_text_collection_is_incomplete(stdout: &[u8], stderr: &[u8]) -> bool {
    stdout.len() >= MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES
        || stderr.len() >= MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES
}

fn search_text_incomplete_collection_error(pattern: &str, input_path: &str) -> ToolRuntimeError {
    ToolRuntimeError::InvalidArguments(format!(
        "search_text collection is incomplete for query '{pattern}' in '{input_path}': ripgrep output hit the command safety ceiling ({MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES} bytes). Refine the query or narrow the path before searching again; collected partial matches are not reported as a complete total."
    ))
}

fn search_text_too_many_matches_error(
    pattern: &str,
    input_path: &str,
    output_bytes: Option<usize>,
) -> ToolRuntimeError {
    let output_detail = output_bytes
        .map(|bytes| format!("; collected output reached {bytes} bytes"))
        .unwrap_or_default();
    ToolRuntimeError::InvalidArguments(format!(
        "search_text matched too much text for query '{pattern}' in '{input_path}'{output_detail}; refine the query with a more specific pattern or narrower path before searching again (collection is incomplete, not a full total)"
    ))
}

fn search_continuation_invalid_error(detail: &str) -> ToolRuntimeError {
    ToolRuntimeError::InvalidArguments(format!("search_text continuation is invalid: {detail}"))
}

fn search_continuation_expired_error(detail: &str) -> ToolRuntimeError {
    ToolRuntimeError::InvalidArguments(format!("search_text continuation expired: {detail}"))
}

pub(crate) fn ripgrep_command() -> String {
    RIPGREP_PATH
        .get()
        .and_then(|state| state.lock().ok().and_then(|path| path.clone()))
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "rg".to_string())
}

struct TextChangeStats {
    lines_added: usize,
    lines_removed: usize,
}

fn text_change_stats(old: &str, new: &str) -> Result<TextChangeStats, ToolRuntimeError> {
    let input = gix::diff::blob::InternedInput::new(old.as_bytes(), new.as_bytes());
    let diff =
        gix::diff::blob::diff_with_slider_heuristics(gix::diff::blob::Algorithm::Histogram, &input);
    let hunks = gix::diff::blob::UnifiedDiff::new(
        &diff,
        &input,
        gix::diff::blob::unified_diff::ConsumeBinaryHunk::new(Vec::new(), "\n"),
        gix::diff::blob::unified_diff::ContextSize::default(),
    )
    .consume()
    .map_err(|source| {
        ToolRuntimeError::InvalidArguments(format!("failed to compute file change stats: {source}"))
    })?;
    let mut stats = TextChangeStats {
        lines_added: 0,
        lines_removed: 0,
    };

    for line in String::from_utf8_lossy(&hunks).lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if line.starts_with('+') {
            stats.lines_added += 1;
        } else if line.starts_with('-') {
            stats.lines_removed += 1;
        }
    }

    Ok(stats)
}

pub(crate) fn write_file(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: WriteFileInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_WRITE_FILE_TIMEOUT_MS)?;
    let path = resolve_workspace_write_path(workspace_path, &request.path)?;
    let line_range = match (request.start_line, request.end_line) {
        (None, None) => None,
        (Some(start), Some(end)) => Some(LineRange::new(start, end)?),
        _ => {
            return Err(ToolRuntimeError::InvalidArguments(
                "startLine and endLine must both be null for complete writes or both be integers for line-range writes".to_string(),
            ));
        }
    };

    let (content, encoding, change_stats) = match fs::metadata(&path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(ToolRuntimeError::NotFile(path));
            }

            let bytes = fs::read(&path).map_err(|source| ToolRuntimeError::Io {
                path: path.clone(),
                source,
            })?;
            let (existing_content, encoding) = decode_text_file(&path, &bytes)?;
            let content = if let Some(range) = line_range {
                replace_line_range(&existing_content, range, &request.content)?
            } else {
                request.content
            };
            let change_stats = text_change_stats(&existing_content, &content)?;

            (content, encoding, change_stats)
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            if line_range.is_some() {
                return Err(ToolRuntimeError::InvalidArguments(
                    "line-range writes require an existing file".to_string(),
                ));
            }

            let change_stats = text_change_stats("", &request.content)?;

            (request.content, TextEncoding::Utf8, change_stats)
        }
        Err(source) => {
            return Err(ToolRuntimeError::Io {
                path: path.clone(),
                source,
            });
        }
    };
    let encoded = encode_text_file(&content, encoding);

    fs::write(&path, &encoded).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    Ok(json!({
        "path": normalize_workspace_path_text(&request.path)?,
        "bytes": encoded.len(),
        "linesAdded": change_stats.lines_added,
        "linesRemoved": change_stats.lines_removed,
        "timeoutMs": timeout_ms
    }))
}

pub(crate) fn edit_file(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: EditFileInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_WRITE_FILE_TIMEOUT_MS)?;
    if request.old_str.is_empty() {
        return Err(ToolRuntimeError::InvalidArguments(
            "oldStr must not be empty".to_string(),
        ));
    }

    let replace_all = request.replace_all.unwrap_or(false);
    let normalized_path = normalize_workspace_path_text(&request.path)?;
    let path = resolve_workspace_file(workspace_path, &request.path)?;
    let bytes = fs::read(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;
    let (existing_content, encoding) = decode_text_file(&path, &bytes)?;
    let match_count = existing_content.matches(&request.old_str).count();

    if match_count == 0 {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "oldStr was not found in {normalized_path}; call read_file to get the latest file content before retrying"
        )));
    }
    if match_count > 1 && !replace_all {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "oldStr matched {match_count} times in {normalized_path}; set replaceAll to true to replace all matches, or provide a more specific oldStr from the latest read_file output"
        )));
    }

    let content = if replace_all {
        existing_content.replace(&request.old_str, &request.new_str)
    } else {
        existing_content.replacen(&request.old_str, &request.new_str, 1)
    };
    let change_stats = text_change_stats(&existing_content, &content)?;
    let encoded = encode_text_file(&content, encoding);

    fs::write(&path, &encoded).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    Ok(json!({
        "path": normalized_path,
        "bytes": encoded.len(),
        "replacements": match_count,
        "replaceAll": replace_all,
        "linesAdded": change_stats.lines_added,
        "linesRemoved": change_stats.lines_removed,
        "timeoutMs": timeout_ms
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadFileInput {
    pub(crate) path: String,
    pub(crate) start_line: Option<usize>,
    pub(crate) end_line: Option<usize>,
    pub(crate) timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindFilesInput {
    path: String,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchTextInput {
    query: String,
    path: String,
    #[serde(default)]
    continuation: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WriteFileInput {
    pub(crate) path: String,
    pub(crate) content: String,
    pub(crate) start_line: Option<usize>,
    pub(crate) end_line: Option<usize>,
    pub(crate) timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditFileInput {
    pub(crate) path: String,
    pub(crate) old_str: String,
    pub(crate) new_str: String,
    pub(crate) replace_all: Option<bool>,
    pub(crate) timeout_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        time::{Duration, SystemTime},
    };

    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::{MAX_SEARCH_RESULT_FILES, MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES, SEARCH_RESULT_TTL};

    #[test]
    fn find_files_excludes_internal_foco_directory() {
        let workspace = tempdir().expect("create temp workspace");
        fs::write(workspace.path().join("package.json"), "{}").expect("write package.json");
        fs::create_dir(workspace.path().join(".foco")).expect("create .foco");
        fs::write(workspace.path().join(".foco").join("foco.sqlite-shm"), "")
            .expect("write sqlite shm placeholder");

        let output = find_files(
            workspace.path(),
            json!({
                "path": ".",
                "include": null,
                "exclude": null,
                "timeoutMs": 10000
            }),
        )
        .expect("find files succeeds");
        let entries = output["entries"].as_array().expect("entries array");
        let paths = entries
            .iter()
            .map(|entry| entry["path"].as_str().expect("entry path"))
            .collect::<Vec<_>>();

        assert!(paths.contains(&"package.json"));
        assert!(paths.iter().all(|path| !path.starts_with(".foco")));
    }

    #[test]
    fn find_files_external_directory_reports_absolute_paths() {
        let workspace = tempdir().expect("workspace");
        let outside = tempdir().expect("outside");
        fs::write(outside.path().join("remote.txt"), "x").expect("write");
        let outside_abs = fs::canonicalize(outside.path()).expect("canon");

        let denied = find_files(
            workspace.path(),
            json!({
                "path": outside_abs.to_string_lossy(),
                "include": null,
                "exclude": null,
                "timeoutMs": 10000
            }),
        );
        assert!(denied.is_err());

        let output = find_files_with_external_access(
            workspace.path(),
            json!({
                "path": outside_abs.to_string_lossy(),
                "include": null,
                "exclude": null,
                "timeoutMs": 10000
            }),
        )
        .expect("authorized external find");
        let entries = output["entries"].as_array().expect("entries");
        assert_eq!(entries.len(), 1);
        let path = entries[0]["path"].as_str().expect("path");
        assert!(Path::new(path).is_absolute());
        assert!(path.ends_with("remote.txt") || path.contains("remote.txt"));
    }

    #[test]
    fn find_files_internal_subdirectory_globs_are_relative_to_search_root() {
        let workspace = tempdir().expect("workspace");
        let src = workspace.path().join("src");
        fs::create_dir(&src).expect("create src");
        fs::write(src.join("main.rs"), "fn main() {}").expect("write main");
        fs::create_dir(src.join("node_modules")).expect("create node_modules");
        fs::write(src.join("node_modules").join("pkg.js"), "x").expect("write pkg");
        fs::write(workspace.path().join("root.rs"), "root").expect("write root");

        let include_exact = find_files(
            workspace.path(),
            json!({
                "path": "src",
                "include": ["main.rs"],
                "exclude": null,
                "timeoutMs": 10000
            }),
        )
        .expect("include exact under search root");
        let include_paths = include_exact["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .map(|entry| entry["path"].as_str().expect("path").to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            include_paths,
            vec!["src/main.rs".to_string()],
            "include should match relative to path=src, report workspace-relative: {include_paths:?}"
        );

        let exclude_prune = find_files(
            workspace.path(),
            json!({
                "path": "src",
                "include": null,
                "exclude": ["node_modules/**"],
                "timeoutMs": 10000
            }),
        )
        .expect("exclude prune under search root");
        let exclude_paths = exclude_prune["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .map(|entry| entry["path"].as_str().expect("path").to_string())
            .collect::<Vec<_>>();
        assert!(
            exclude_paths.contains(&"src/main.rs".to_string()),
            "{exclude_paths:?}"
        );
        assert!(
            exclude_paths
                .iter()
                .all(|path| !path.contains("node_modules")),
            "node_modules/** relative to src must prune: {exclude_paths:?}"
        );
        assert!(
            !exclude_paths.iter().any(|path| path == "root.rs"),
            "search must stay under src: {exclude_paths:?}"
        );
    }

    #[test]
    fn search_text_external_snapshot_stays_in_workspace() {
        let workspace = tempdir().expect("workspace");
        let outside = tempdir().expect("outside");
        // Many lines so soft budget may truncate and force a snapshot when combined with long paths.
        let mut body = String::new();
        for i in 0..200 {
            body.push_str(&format!(
                "snapshot-marker-{i} pad-pad-pad-pad-pad-pad-pad\n"
            ));
        }
        let outside_file = outside.path().join("big.txt");
        fs::write(&outside_file, body).expect("write");
        let outside_dir_abs = fs::canonicalize(outside.path()).expect("canon dir");
        let outside_file_abs = fs::canonicalize(&outside_file).expect("canon file");

        // Directory target (existing coverage).
        let denied = search_text(
            workspace.path(),
            json!({
                "query": "snapshot-marker",
                "path": outside_dir_abs.to_string_lossy(),
                "continuation": null,
                "timeoutMs": 10000
            }),
            None,
        );
        assert!(denied.is_err());

        let output = search_text_with_external_access(
            workspace.path(),
            json!({
                "query": "snapshot-marker",
                "path": outside_dir_abs.to_string_lossy(),
                "continuation": null,
                "timeoutMs": 10000
            }),
            None,
        )
        .expect("authorized external search");
        assert!(output["totalMatches"].as_u64().unwrap_or(0) >= 1);
        let match_path = output["matches"][0]["path"].as_str().expect("match path");
        assert!(Path::new(match_path).is_absolute());
        if let Some(full) = output["fullResultPath"].as_str() {
            assert!(full.contains("search-results"));
            assert!(!full.starts_with(outside_dir_abs.to_string_lossy().as_ref()));
            let snap = workspace.path().join(full.replace('\\', "/"));
            assert!(
                snap.exists(),
                "snapshot under workspace: {}",
                snap.display()
            );
        }

        // File target is also valid (rg-compatible; not directory-only).
        let file_output = search_text_with_external_access(
            workspace.path(),
            json!({
                "query": "snapshot-marker",
                "path": outside_file_abs.to_string_lossy(),
                "continuation": null,
                "timeoutMs": 10000
            }),
            None,
        )
        .expect("authorized external file search");
        assert!(file_output["totalMatches"].as_u64().unwrap_or(0) >= 1);
        let file_match = file_output["matches"][0]["path"]
            .as_str()
            .expect("file match path");
        assert!(
            Path::new(file_match).is_absolute(),
            "external file match.path must be absolute: {file_match}"
        );
        assert!(
            file_match.ends_with("big.txt") || file_match.contains("big.txt"),
            "{file_match}"
        );
        if let Some(full) = file_output["fullResultPath"].as_str() {
            assert!(
                full.starts_with(".foco/search-results/")
                    || full.starts_with(".foco\\search-results\\"),
                "file-search snapshot stays under execution workspace: {full}"
            );
        }
    }

    #[test]
    fn glob_filter_prunes_directories_matched_by_descendant_excludes() {
        let filter = GlobFilter::new(None, Some(vec!["node_modules/**".to_string()]))
            .expect("create glob filter");

        assert!(filter.prunes_directory("node_modules"));
        assert!(!filter.matches("node_modules/package.json"));
        assert!(filter.matches("package.json"));
    }

    fn touch_mtime(path: &Path, age: Duration) {
        let target = SystemTime::now()
            .checked_sub(age)
            .expect("mtime within epoch");
        let file = fs::File::options()
            .write(true)
            .open(path)
            .expect("open for mtime");
        file.set_modified(target).expect("set mtime");
    }

    #[test]
    fn prune_search_results_dir_drops_ttl_expired_and_caps_snapshot_count() {
        let workspace = tempdir().expect("workspace");
        let results_dir = workspace
            .path()
            .join(WORKSPACE_FOCO_DIR)
            .join(SEARCH_RESULTS_DIR);
        fs::create_dir_all(&results_dir).expect("create results dir");

        // One expired snapshot pair must be removed by TTL.
        let expired_id = "search-expired-1";
        let expired_json = results_dir.join(format!("{expired_id}.json"));
        let expired_txt = results_dir.join(format!("{expired_id}.txt"));
        fs::write(&expired_json, "{}").expect("write expired json");
        fs::write(&expired_txt, "old").expect("write expired txt");
        touch_mtime(&expired_json, SEARCH_RESULT_TTL + Duration::from_secs(5));
        touch_mtime(&expired_txt, SEARCH_RESULT_TTL + Duration::from_secs(5));

        // Fill to the cap with fresh pairs; prune keeps room for one new snapshot.
        for index in 0..MAX_SEARCH_RESULT_FILES {
            let id = format!("search-fresh-{index}");
            let json_path = results_dir.join(format!("{id}.json"));
            let txt_path = results_dir.join(format!("{id}.txt"));
            fs::write(&json_path, format!(r#"{{"id":{index}}}"#)).expect("write json");
            fs::write(&txt_path, format!("match {index}")).expect("write txt");
            // Older indices are older so they are preferred for eviction.
            let age = Duration::from_secs((MAX_SEARCH_RESULT_FILES - index) as u64);
            touch_mtime(&json_path, age);
            touch_mtime(&txt_path, age);
        }

        prune_search_results_dir(&results_dir);

        assert!(!expired_json.exists());
        assert!(!expired_txt.exists());

        let remaining_ids = fs::read_dir(&results_dir)
            .expect("read results")
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                name.strip_suffix(".json")
                    .or_else(|| name.strip_suffix(".txt"))
                    .map(str::to_string)
            })
            .collect::<std::collections::BTreeSet<_>>();
        // Cap is MAX_SEARCH_RESULT_FILES exclusive of the next write: prune leaves at most 19.
        assert!(
            remaining_ids.len() < MAX_SEARCH_RESULT_FILES,
            "remaining={}",
            remaining_ids.len()
        );
        assert!(!remaining_ids.contains("search-fresh-0"));
        assert!(remaining_ids.contains(&format!("search-fresh-{}", MAX_SEARCH_RESULT_FILES - 1)));
    }

    #[test]
    fn search_text_incomplete_collection_error_refuses_full_total() {
        // Command collection ceiling is a hard safety bound (4 MiB), independent of soft preview.
        assert_eq!(MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES, 4 * 1024 * 1024);

        assert!(!search_text_collection_is_incomplete(
            &vec![0u8; MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES - 1],
            &[]
        ));
        assert!(search_text_collection_is_incomplete(
            &vec![0u8; MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES],
            &[]
        ));
        assert!(search_text_collection_is_incomplete(
            &[],
            &vec![0u8; MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES]
        ));

        // incomplete path must not invent totalMatches; error text is the only response surface.
        let error = search_text_incomplete_collection_error("needle", ".");
        let message = error.to_string();
        assert!(message.contains("incomplete"));
        assert!(message.contains("complete total"));
        assert!(message.contains(&MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES.to_string()));
        assert!(!message.contains("totalMatches"));

        // Related refine error used when collection is truncated before match materialization.
        let refine = search_text_too_many_matches_error(
            "needle",
            ".",
            Some(MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES),
        )
        .to_string();
        assert!(
            refine.contains("incomplete") && refine.contains("full total"),
            "{refine}"
        );
    }
}
