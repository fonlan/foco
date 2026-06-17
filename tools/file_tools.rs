use std::{fs, io, path::Path, time::Duration};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    CommandOutputLimits, DEFAULT_FILE_TOOL_TIMEOUT_MS, DEFAULT_SEARCH_TEXT_TIMEOUT_MS,
    DEFAULT_WRITE_FILE_TIMEOUT_MS, LineRange, MAX_FIND_ENTRIES, MAX_FULL_READ_BYTES,
    MAX_RANGED_READ_OUTPUT_BYTES, MAX_RANGED_READ_SOURCE_BYTES, MAX_SEARCH_MATCHES,
    MAX_SEARCH_TEXT_LINE_BYTES, MAX_SEARCH_TEXT_OUTPUT_BYTES, RIPGREP_PATH, TextEncoding,
    ToolCancellationToken, count_text_lines, decode_text_file, encode_text_file,
    errors::{ToolRuntimeError, tool_timeout_ms},
    normalize_read_line_range, normalize_workspace_path_text, numbered_content, parse_arguments,
    parse_optional_line_range, read_line_range, relative_workspace_path, replace_line_range,
    resolve_workspace_file, resolve_workspace_path, resolve_workspace_write_path,
    run_command_with_timeout,
};

pub(crate) fn read_file(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: ReadFileInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_FILE_TOOL_TIMEOUT_MS)?;
    let requested_line_range = parse_optional_line_range(request.start_line, request.end_line)?;
    let path = resolve_workspace_file(workspace_path, &request.path)?;
    let metadata = fs::metadata(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    if !metadata.is_file() {
        return Err(ToolRuntimeError::NotFile(path));
    }

    let max_source_bytes = if requested_line_range.is_some() {
        MAX_RANGED_READ_SOURCE_BYTES
    } else {
        MAX_FULL_READ_BYTES
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
    let (content, _) = decode_text_file(&path, &bytes)?;
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
    let content = numbered_content(&content, content_start_line);
    if line_range.is_some() && content.len() > MAX_RANGED_READ_OUTPUT_BYTES {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "read_file line range output is too large ({} bytes; max {MAX_RANGED_READ_OUTPUT_BYTES}); use a smaller line range",
            content.len()
        )));
    }

    Ok(json!({
        "path": request.path,
        "content": content,
        "bytes": metadata.len(),
        "startLine": line_range.as_ref().map(|range| range.start),
        "endLine": line_range.as_ref().map(|range| range.end),
        "timeoutMs": timeout_ms
    }))
}

pub(crate) fn find_files(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: FindFilesInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_FILE_TOOL_TIMEOUT_MS)?;
    let input_path = request.path;
    let path = resolve_workspace_path(workspace_path, &input_path)?;
    let filter = GlobFilter::new(request.include, request.exclude)?;
    let metadata = fs::metadata(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    if !metadata.is_dir() {
        return Err(ToolRuntimeError::NotDirectory(path));
    }

    let mut entries = Vec::new();
    find_files_in_directory(workspace_path, &path, &filter, &mut entries)?;

    entries.sort_by(|left, right| {
        left.get("path")
            .and_then(Value::as_str)
            .cmp(&right.get("path").and_then(Value::as_str))
    });
    let truncated = entries.len() > MAX_FIND_ENTRIES;
    entries.truncate(MAX_FIND_ENTRIES);

    Ok(json!({
        "path": input_path,
        "include": filter.include_patterns(),
        "exclude": filter.exclude_patterns(),
        "entries": entries,
        "truncated": truncated,
        "timeoutMs": timeout_ms
    }))
}

fn find_files_in_directory(
    workspace_path: &Path,
    directory_path: &Path,
    filter: &GlobFilter,
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
        let file_type = entry.file_type().map_err(|source| ToolRuntimeError::Io {
            path: entry_path.clone(),
            source,
        })?;
        let metadata = entry.metadata().map_err(|source| ToolRuntimeError::Io {
            path: entry_path.clone(),
            source,
        })?;
        let kind = if file_type.is_dir() {
            "directory"
        } else if file_type.is_file() {
            "file"
        } else {
            "other"
        };
        let relative_path = relative_workspace_path(workspace_path, &entry_path)?;

        if filter.matches(&relative_path) {
            entries.push(json!({
                "path": relative_path,
                "kind": kind,
                "bytes": if file_type.is_file() { Some(metadata.len()) } else { None }
            }));
        }

        if file_type.is_dir() {
            find_files_in_directory(workspace_path, &entry_path, filter, entries)?;
        }
    }

    Ok(())
}

struct GlobFilter {
    include: Vec<String>,
    exclude: Vec<String>,
    include_set: Option<GlobSet>,
    exclude_set: Option<GlobSet>,
}

impl GlobFilter {
    fn new(
        include: Option<Vec<String>>,
        exclude: Option<Vec<String>>,
    ) -> Result<Self, ToolRuntimeError> {
        let include = normalize_glob_patterns("include", include)?;
        let exclude = normalize_glob_patterns("exclude", exclude)?;
        let include_set = compile_glob_set("include", &include)?;
        let exclude_set = compile_glob_set("exclude", &exclude)?;

        Ok(Self {
            include,
            exclude,
            include_set,
            exclude_set,
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
    let request: SearchTextInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_SEARCH_TEXT_TIMEOUT_MS)?;
    let input_path = request.path;
    let path = resolve_workspace_path(workspace_path, &input_path)?;
    let pattern = request.query.trim();

    if pattern.is_empty() {
        return Err(ToolRuntimeError::InvalidArguments(
            "query must not be empty".to_string(),
        ));
    }

    let rg_args = vec![
        "--json".to_string(),
        "--line-number".to_string(),
        "--max-count".to_string(),
        (MAX_SEARCH_MATCHES + 1).to_string(),
        "--max-columns".to_string(),
        MAX_SEARCH_TEXT_LINE_BYTES.to_string(),
        pattern.to_string(),
        path.to_string_lossy().to_string(),
    ];
    let rg_command = ripgrep_command();
    let output = match run_command_with_timeout(
        &rg_command,
        &rg_args,
        workspace_path,
        Duration::from_millis(timeout_ms),
        cancellation_token,
        None,
        Some(CommandOutputLimits {
            stdout_bytes: Some(MAX_SEARCH_TEXT_OUTPUT_BYTES),
            stderr_bytes: Some(MAX_SEARCH_TEXT_OUTPUT_BYTES),
        }),
    ) {
        Ok(output) => output,
        Err(ToolRuntimeError::CommandOutputTooLarge { bytes, .. }) => {
            return Err(search_text_too_many_matches_error(
                pattern,
                &input_path,
                Some(bytes),
            ));
        }
        Err(error) => return Err(error),
    };
    if output.stdout.len() > MAX_SEARCH_TEXT_OUTPUT_BYTES {
        return Err(search_text_too_many_matches_error(
            pattern,
            &input_path,
            Some(output.stdout.len()),
        ));
    }
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if output.status.code() == Some(1) {
            return Ok(json!({
                "query": pattern,
                "path": input_path,
                "matches": [],
                "truncated": false,
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
    let mut matches = Vec::new();
    let mut output_bytes = 0usize;
    for line in stdout.lines() {
        let event: Value =
            serde_json::from_str(line).map_err(|source| ToolRuntimeError::InvalidToolOutput {
                command: "rg".to_string(),
                source,
            })?;

        if event.get("type").and_then(Value::as_str) != Some("match") {
            continue;
        }
        if matches.len() >= MAX_SEARCH_MATCHES {
            return Err(search_text_too_many_matches_error(
                pattern,
                &input_path,
                None,
            ));
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
        let relative_path = relative_workspace_path(workspace_path, Path::new(absolute_path))?;
        output_bytes = output_bytes
            .saturating_add(relative_path.len())
            .saturating_add(text.len())
            .saturating_add(32);
        if output_bytes > MAX_SEARCH_TEXT_OUTPUT_BYTES {
            return Err(search_text_too_many_matches_error(
                pattern,
                &input_path,
                Some(output_bytes),
            ));
        }

        matches.push(json!({
            "path": relative_path,
            "line": line_number,
            "text": text
        }));
    }

    Ok(json!({
        "query": pattern,
        "path": input_path,
        "matches": matches,
        "truncated": false,
        "timeoutMs": timeout_ms
    }))
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
        "search_text matched too much text for query '{pattern}' in '{input_path}'{output_detail}; refine the query with a more specific pattern or narrower path before searching again"
    ))
}

fn ripgrep_command() -> String {
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
struct ReadFileInput {
    path: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
    timeout_ms: Option<u64>,
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
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteFileInput {
    path: String,
    content: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditFileInput {
    path: String,
    old_str: String,
    new_str: String,
    replace_all: Option<bool>,
    timeout_ms: Option<u64>,
}
