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

use crate::{
    CommandOutputLimits, DEFAULT_FILE_TOOL_TIMEOUT_MS, DEFAULT_SEARCH_TEXT_TIMEOUT_MS,
    DEFAULT_WRITE_FILE_TIMEOUT_MS, LineRange, MAX_FIND_ENTRIES, MAX_FULL_READ_BYTES,
    MAX_RANGED_READ_OUTPUT_BYTES, MAX_RANGED_READ_SOURCE_BYTES, MAX_SEARCH_MATCHES,
    MAX_SEARCH_RESULT_FILES, MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES, MAX_SEARCH_TEXT_LINE_BYTES,
    MAX_SEARCH_TEXT_OUTPUT_BYTES, RIPGREP_PATH, SEARCH_RESULT_TTL, SEARCH_RESULTS_DIR,
    TextEncoding, ToolCancellationToken, count_text_lines, decode_text_file, encode_text_file,
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

const INTERNAL_FIND_FILES_EXCLUDE_PATTERNS: &[&str] = &[".foco", ".foco/**"];

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
        let relative_path = relative_workspace_path(workspace_path, &entry_path)?;
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

        if file_type.is_dir() && filter.prunes_directory(&relative_path) {
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

        if filter.matches(&relative_path) {
            entries.push(json!({
                "path": relative_path,
                "kind": kind,
                "bytes": file_bytes
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
            stdout_bytes: Some(MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES),
            stderr_bytes: Some(MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES),
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
        let relative_path = relative_workspace_path(workspace_path, Path::new(absolute_path))?;

        entries.push(SearchMatch {
            path: relative_path,
            line: line_number,
            text,
        });
    }

    let total_matches = entries.len();
    // Keep as many leading matches as fit within the response budget; the rest
    // (if any) are still preserved in full on disk below.
    let mut returned = 0usize;
    let mut output_bytes = 0usize;
    for entry in &entries {
        if returned >= MAX_SEARCH_MATCHES {
            break;
        }
        let next_bytes = output_bytes
            .saturating_add(entry.path.len())
            .saturating_add(entry.text.len())
            .saturating_add(32);
        if next_bytes > MAX_SEARCH_TEXT_OUTPUT_BYTES {
            break;
        }
        output_bytes = next_bytes;
        returned += 1;
    }

    let matches = entries[..returned]
        .iter()
        .map(SearchMatch::to_json)
        .collect::<Vec<_>>();

    if returned == total_matches {
        return Ok(json!({
            "query": pattern,
            "path": input_path,
            "matches": matches,
            "truncated": false,
            "timeoutMs": timeout_ms
        }));
    }

    // The response was truncated; persist the complete result set so the model
    // can read it on demand via read_file instead of re-running a broad search.
    let full_results = render_search_results(&entries);
    let full_result_path = write_search_results_file(workspace_path, &full_results)?;
    let note = format!(
        "Results truncated: showing the first {returned} of {total_matches} matches. The complete \
         result set was saved to '{full_result_path}'. Call read_file on that path (use a line \
         range if the file is large) to see every match, or refine the query or path to narrow the \
         search."
    );

    Ok(json!({
        "query": pattern,
        "path": input_path,
        "matches": matches,
        "truncated": true,
        "totalMatches": total_matches,
        "returnedMatches": returned,
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

/// Writes the complete search results into `.foco/search-results/` and returns
/// the workspace-relative path so the model can read it back with read_file.
fn write_search_results_file(
    workspace_path: &Path,
    contents: &str,
) -> Result<String, ToolRuntimeError> {
    let results_dir = workspace_foco_dir(workspace_path).join(SEARCH_RESULTS_DIR);
    fs::create_dir_all(&results_dir).map_err(|source| ToolRuntimeError::Io {
        path: results_dir.clone(),
        source,
    })?;
    prune_search_results_dir(&results_dir);

    let file_name = next_search_results_file_name();
    let file_path = results_dir.join(&file_name);
    fs::write(&file_path, contents).map_err(|source| ToolRuntimeError::Io {
        path: file_path.clone(),
        source,
    })?;

    Ok(format!(
        "{WORKSPACE_FOCO_DIR}/{SEARCH_RESULTS_DIR}/{file_name}"
    ))
}

fn next_search_results_file_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let counter = SEARCH_RESULTS_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("search-{nanos}-{counter}.txt")
}

/// Best-effort cleanup of stale search-result files: drops anything past the
/// retention window and caps the directory so a single new file can be added
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
            .is_some_and(|name| name.starts_with("search-") && name.ends_with(".txt"));
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

    if files.len() < MAX_SEARCH_RESULT_FILES {
        return;
    }

    files.sort_by_key(|(_, modified)| *modified);
    let remove_count = files.len() + 1 - MAX_SEARCH_RESULT_FILES;
    for (path, _) in files.into_iter().take(remove_count) {
        let _ = fs::remove_file(path);
    }
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
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

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
    fn glob_filter_prunes_directories_matched_by_descendant_excludes() {
        let filter = GlobFilter::new(None, Some(vec!["node_modules/**".to_string()]))
            .expect("create glob filter");

        assert!(filter.prunes_directory("node_modules"));
        assert!(!filter.matches("node_modules/package.json"));
        assert!(filter.matches("package.json"));
    }
}
