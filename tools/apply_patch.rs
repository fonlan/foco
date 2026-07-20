//! Codex-compatible `apply_patch` parsing and application.
//!
//! The patch grammar and matching behaviour are an auditable, Rust-native port
//! of OpenAI Codex's `codex-rs/apply-patch` implementation, licensed Apache-2.0.
//! Source: https://github.com/openai/codex/tree/main/codex-rs/apply-patch
//! Foco deliberately keeps its execution-root sandbox stricter than Codex's
//! unrestricted command-line utility.

use std::{
    ffi::OsString,
    fmt,
    fs::{self, FileType},
    io,
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::errors::tool_timeout_ms;
use crate::{
    DEFAULT_APPLY_PATCH_TIMEOUT_MS, ToolCancellationToken, ToolRuntimeError, decode_text_file,
    encode_text_file, parse_arguments,
};

const MAX_PATCH_BYTES: usize = 2 * 1024 * 1024;
const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
const END_PATCH_MARKER: &str = "*** End Patch";
const ADD_FILE_MARKER: &str = "*** Add File: ";
const DELETE_FILE_MARKER: &str = "*** Delete File: ";
const UPDATE_FILE_MARKER: &str = "*** Update File: ";
const MOVE_TO_MARKER: &str = "*** Move to: ";
const EOF_MARKER: &str = "*** End of File";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApplyPatchInput {
    patch: String,
    timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[expect(
    clippy::enum_variant_names,
    reason = "Codex patch grammar names these operations Add File, Delete File, and Update File."
)]
pub(crate) enum PatchHunk {
    AddFile {
        path: String,
        contents: String,
    },
    DeleteFile {
        path: String,
    },
    UpdateFile {
        path: String,
        move_path: Option<String>,
        chunks: Vec<UpdateFileChunk>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UpdateFileChunk {
    pub(crate) change_context: Option<String>,
    pub(crate) old_lines: Vec<String>,
    pub(crate) new_lines: Vec<String>,
    pub(crate) is_end_of_file: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PatchParseError {
    InvalidPatch(String),
    InvalidHunk { line_number: usize, message: String },
}

impl fmt::Display for PatchParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPatch(message) => write!(formatter, "invalid patch: {message}"),
            Self::InvalidHunk {
                line_number,
                message,
            } => write!(formatter, "invalid hunk at line {line_number}: {message}"),
        }
    }
}

impl std::error::Error for PatchParseError {}

#[derive(Default)]
struct ChunkBuilder {
    change_context: Option<String>,
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    is_end_of_file: bool,
}

impl ChunkBuilder {
    fn into_chunk(self, line_number: usize) -> Result<UpdateFileChunk, PatchParseError> {
        if self.old_lines.is_empty() && self.new_lines.is_empty() {
            return Err(PatchParseError::InvalidHunk {
                line_number,
                message: "update chunk is empty".to_string(),
            });
        }

        Ok(UpdateFileChunk {
            change_context: self.change_context,
            old_lines: self.old_lines,
            new_lines: self.new_lines,
            is_end_of_file: self.is_end_of_file,
        })
    }
}

/// Parses a Codex `*** Begin Patch` document without touching the filesystem.
pub(crate) fn parse_patch(patch: &str) -> Result<Vec<PatchHunk>, PatchParseError> {
    if patch.is_empty() {
        return Err(PatchParseError::InvalidPatch(
            "patch must not be empty".to_string(),
        ));
    }
    if patch.len() > MAX_PATCH_BYTES {
        return Err(PatchParseError::InvalidPatch(format!(
            "patch exceeds the {MAX_PATCH_BYTES}-byte input limit"
        )));
    }

    let lines = patch
        .trim()
        .lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect::<Vec<_>>();
    let lines = unwrap_heredoc(&lines)?;
    validate_patch_boundaries(lines)?;

    let mut hunks = Vec::new();
    let mut index = 1;
    while index + 1 < lines.len() {
        let line = lines[index];
        let trimmed = line.trim();
        if trimmed.starts_with(ADD_FILE_MARKER) {
            let (path, header_line) = marker_path(trimmed, ADD_FILE_MARKER, index + 1)?;
            index += 1;
            let mut contents = String::new();
            let mut count = 0;
            while index + 1 < lines.len() && !is_file_marker(lines[index].trim()) {
                let add_line = lines[index];
                let Some(content) = add_line.strip_prefix('+') else {
                    return invalid_hunk(
                        index + 1,
                        "added file content must start with '+'".to_string(),
                    );
                };
                contents.push_str(content);
                contents.push('\n');
                count += 1;
                index += 1;
            }
            if count == 0 {
                return invalid_hunk(
                    header_line,
                    format!("Add file hunk for path '{path}' is empty"),
                );
            }
            hunks.push(PatchHunk::AddFile { path, contents });
            continue;
        }

        if trimmed.starts_with(DELETE_FILE_MARKER) {
            let (path, _) = marker_path(trimmed, DELETE_FILE_MARKER, index + 1)?;
            index += 1;
            if index + 1 < lines.len() && !is_file_marker(lines[index].trim()) {
                return invalid_hunk(
                    index + 1,
                    "Delete file hunk must not contain change lines".to_string(),
                );
            }
            hunks.push(PatchHunk::DeleteFile { path });
            continue;
        }

        if trimmed.starts_with(UPDATE_FILE_MARKER) {
            let (path, header_line) = marker_path(trimmed, UPDATE_FILE_MARKER, index + 1)?;
            index += 1;
            let mut move_path = None;
            if index + 1 < lines.len() && lines[index].trim().starts_with(MOVE_TO_MARKER) {
                let (destination, _) = marker_path(lines[index].trim(), MOVE_TO_MARKER, index + 1)?;
                move_path = Some(destination);
                index += 1;
            }

            let mut chunks = Vec::new();
            let mut current: Option<ChunkBuilder> = None;
            let mut eof_seen = false;
            while index + 1 < lines.len() && !is_file_marker(lines[index].trim()) {
                let change_line = lines[index];
                let change_trimmed = change_line.trim();
                if change_trimmed.starts_with(MOVE_TO_MARKER) {
                    return invalid_hunk(
                        index + 1,
                        "Move to must appear directly after Update File".to_string(),
                    );
                }
                if change_trimmed == EOF_MARKER {
                    let Some(builder) = current.as_mut() else {
                        return invalid_hunk(
                            index + 1,
                            "End of File must follow an update chunk".to_string(),
                        );
                    };
                    builder.is_end_of_file = true;
                    eof_seen = true;
                    index += 1;
                    continue;
                }
                if eof_seen {
                    return invalid_hunk(
                        index + 1,
                        "End of File must be the last line in an update hunk".to_string(),
                    );
                }
                if change_trimmed == "@@" || change_trimmed.starts_with("@@ ") {
                    if let Some(builder) = current.take() {
                        chunks.push(builder.into_chunk(index + 1)?);
                    }
                    current = Some(ChunkBuilder {
                        change_context: change_trimmed
                            .strip_prefix("@@ ")
                            .map(str::to_owned)
                            .filter(|context| !context.is_empty()),
                        ..ChunkBuilder::default()
                    });
                    index += 1;
                    continue;
                }

                let builder = current.get_or_insert_with(ChunkBuilder::default);
                match change_line.chars().next() {
                    Some('+') => builder.new_lines.push(change_line[1..].to_string()),
                    Some('-') => builder.old_lines.push(change_line[1..].to_string()),
                    Some(' ') => {
                        let unchanged = change_line[1..].to_string();
                        builder.old_lines.push(unchanged.clone());
                        builder.new_lines.push(unchanged);
                    }
                    _ => {
                        // Codex accepts an omitted leading space on the first chunk.
                        // Treat it as unchanged context so both sides stay aligned.
                        builder.old_lines.push(change_line.to_string());
                        builder.new_lines.push(change_line.to_string());
                    }
                }
                index += 1;
            }
            if let Some(builder) = current {
                chunks.push(builder.into_chunk(index + 1)?);
            }
            if chunks.is_empty() {
                return invalid_hunk(
                    header_line,
                    format!("Update file hunk for path '{path}' is empty"),
                );
            }
            hunks.push(PatchHunk::UpdateFile {
                path,
                move_path,
                chunks,
            });
            continue;
        }

        return invalid_hunk(index + 1, format!("unexpected patch marker '{line}'"));
    }

    Ok(hunks)
}

#[cfg(test)]
pub(crate) fn apply_patch(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    apply_patch_with_cancellation(workspace_path, arguments, None)
}

pub(crate) fn apply_patch_with_cancellation(
    workspace_path: &Path,
    arguments: Value,
    cancellation_token: Option<&ToolCancellationToken>,
) -> Result<Value, ToolRuntimeError> {
    let request: ApplyPatchInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_APPLY_PATCH_TIMEOUT_MS)?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    ensure_apply_patch_active(cancellation_token, deadline, timeout_ms)?;
    let hunks = parse_patch(&request.patch)
        .map_err(|error| ToolRuntimeError::InvalidArguments(error.to_string()))?;
    if hunks.is_empty() {
        return Err(ToolRuntimeError::InvalidArguments(
            "invalid patch: No files were modified.".to_string(),
        ));
    }

    let workspace = fs::canonicalize(workspace_path).map_err(|source| ToolRuntimeError::Io {
        path: workspace_path.to_path_buf(),
        source,
    })?;
    let mut affected = AffectedPaths::default();
    for hunk in &hunks {
        // Preserve any prior hunk effects, but stop before the next mutation if the run ended.
        ensure_apply_patch_active(cancellation_token, deadline, timeout_ms)?;
        apply_hunk(&workspace, hunk, &mut affected)?;
    }

    Ok(json!({
        "summary": affected.summary(),
        "added": affected.added,
        "modified": affected.modified,
        "deleted": affected.deleted,
        "timeoutMs": timeout_ms,
    }))
}

fn ensure_apply_patch_active(
    cancellation_token: Option<&ToolCancellationToken>,
    deadline: Instant,
    timeout_ms: u64,
) -> Result<(), ToolRuntimeError> {
    if cancellation_token.is_some_and(ToolCancellationToken::is_cancelled) {
        return Err(ToolRuntimeError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "apply_patch timed out after {timeout_ms} ms"
        )));
    }
    Ok(())
}

fn unwrap_heredoc<'a>(lines: &'a [&'a str]) -> Result<&'a [&'a str], PatchParseError> {
    if matches_patch_boundaries(lines) {
        return Ok(lines);
    }

    match lines {
        [first, middle @ .., last]
            if matches!(first.trim(), "<<EOF" | "<<'EOF'" | "<<\"EOF\"")
                && last.trim_end().ends_with("EOF")
                && middle.len() >= 2 =>
        {
            Ok(middle)
        }
        _ => Ok(lines),
    }
}

fn validate_patch_boundaries(lines: &[&str]) -> Result<(), PatchParseError> {
    let Some(first) = lines.first() else {
        return Err(PatchParseError::InvalidPatch(
            "The first line of the patch must be '*** Begin Patch'".to_string(),
        ));
    };
    if first.trim() != BEGIN_PATCH_MARKER {
        return Err(PatchParseError::InvalidPatch(
            "The first line of the patch must be '*** Begin Patch'".to_string(),
        ));
    }
    let Some(last) = lines.last() else {
        return Err(PatchParseError::InvalidPatch(
            "The last line of the patch must be '*** End Patch'".to_string(),
        ));
    };
    if last.trim() != END_PATCH_MARKER {
        return Err(PatchParseError::InvalidPatch(
            "The last line of the patch must be '*** End Patch'".to_string(),
        ));
    }
    Ok(())
}

fn matches_patch_boundaries(lines: &[&str]) -> bool {
    lines
        .first()
        .is_some_and(|line| line.trim() == BEGIN_PATCH_MARKER)
        && lines
            .last()
            .is_some_and(|line| line.trim() == END_PATCH_MARKER)
}

fn marker_path(
    line: &str,
    marker: &str,
    line_number: usize,
) -> Result<(String, usize), PatchParseError> {
    let path = line
        .strip_prefix(marker)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| PatchParseError::InvalidHunk {
            line_number,
            message: format!("{marker} requires a file path"),
        })?;
    Ok((path.to_string(), line_number))
}

fn is_file_marker(line: &str) -> bool {
    line.starts_with(ADD_FILE_MARKER)
        || line.starts_with(DELETE_FILE_MARKER)
        || line.starts_with(UPDATE_FILE_MARKER)
        || line == END_PATCH_MARKER
}

fn invalid_hunk<T>(line_number: usize, message: String) -> Result<T, PatchParseError> {
    Err(PatchParseError::InvalidHunk {
        line_number,
        message,
    })
}

#[derive(Default)]
struct AffectedPaths {
    added: Vec<String>,
    modified: Vec<String>,
    deleted: Vec<String>,
}

impl AffectedPaths {
    fn summary(&self) -> String {
        let mut output = String::from("Success. Updated the following files:\n");
        for path in &self.added {
            output.push_str("A ");
            output.push_str(path);
            output.push('\n');
        }
        for path in &self.modified {
            output.push_str("M ");
            output.push_str(path);
            output.push('\n');
        }
        for path in &self.deleted {
            output.push_str("D ");
            output.push_str(path);
            output.push('\n');
        }
        output
    }
}

fn apply_hunk(
    workspace: &Path,
    hunk: &PatchHunk,
    affected: &mut AffectedPaths,
) -> Result<(), ToolRuntimeError> {
    match hunk {
        PatchHunk::AddFile { path, contents } => {
            let target = resolve_patch_target(workspace, path)?;
            prepare_write_target(workspace, &target, path)?;
            fs::write(&target, contents).map_err(|source| ToolRuntimeError::Io {
                path: target,
                source,
            })?;
            affected.added.push(display_patch_path(path));
        }
        PatchHunk::DeleteFile { path } => {
            let target = resolve_existing_patch_file(workspace, path)?;
            fs::remove_file(&target).map_err(|source| ToolRuntimeError::Io {
                path: target,
                source,
            })?;
            affected.deleted.push(display_patch_path(path));
        }
        PatchHunk::UpdateFile {
            path,
            move_path,
            chunks,
        } => {
            let source = resolve_existing_patch_file(workspace, path)?;
            let bytes = fs::read(&source).map_err(|source_error| ToolRuntimeError::Io {
                path: source.clone(),
                source: source_error,
            })?;
            let (original, encoding) = decode_text_file(&source, &bytes)?;
            let updated = apply_update_chunks(&original, path, chunks)
                .map_err(|error| ToolRuntimeError::InvalidArguments(error.to_string()))?;
            let encoded = encode_text_file(&updated, encoding);

            if let Some(move_path) = move_path {
                let destination = resolve_patch_target(workspace, move_path)?;
                if destination == source {
                    return Err(ToolRuntimeError::InvalidArguments(format!(
                        "invalid patch: Move destination must differ from source: {move_path}"
                    )));
                }
                prepare_write_target(workspace, &destination, move_path)?;
                fs::write(&destination, encoded).map_err(|source_error| ToolRuntimeError::Io {
                    path: destination.clone(),
                    source: source_error,
                })?;
                fs::remove_file(&source).map_err(|source_error| ToolRuntimeError::Io {
                    path: source,
                    source: source_error,
                })?;
                affected.modified.push(display_patch_path(move_path));
            } else {
                fs::write(&source, encoded).map_err(|source_error| ToolRuntimeError::Io {
                    path: source,
                    source: source_error,
                })?;
                affected.modified.push(display_patch_path(path));
            }
        }
    }

    Ok(())
}

fn apply_update_chunks(
    original_contents: &str,
    path: &str,
    chunks: &[UpdateFileChunk],
) -> Result<String, PatchApplicationError> {
    let mut original_lines = original_contents
        .split('\n')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if original_lines.last().is_some_and(String::is_empty) {
        original_lines.pop();
    }
    let replacements = compute_replacements(&original_lines, path, chunks)?;
    let mut lines = apply_replacements(original_lines, &replacements);
    if !lines.last().is_some_and(String::is_empty) {
        lines.push(String::new());
    }
    Ok(lines.join("\n"))
}

#[derive(Debug)]
struct PatchApplicationError(String);

impl fmt::Display for PatchApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PatchApplicationError {}

fn compute_replacements(
    original_lines: &[String],
    path: &str,
    chunks: &[UpdateFileChunk],
) -> Result<Vec<(usize, usize, Vec<String>)>, PatchApplicationError> {
    let mut replacements = Vec::new();
    let mut line_index = 0;
    for chunk in chunks {
        if let Some(context) = &chunk.change_context {
            let Some(index) = seek_sequence(
                original_lines,
                std::slice::from_ref(context),
                line_index,
                false,
            ) else {
                return Err(PatchApplicationError(format!(
                    "Failed to find context '{context}' in {path}"
                )));
            };
            line_index = index + 1;
        }

        if chunk.old_lines.is_empty() {
            replacements.push((original_lines.len(), 0, chunk.new_lines.clone()));
            continue;
        }

        let mut pattern = chunk.old_lines.as_slice();
        let mut replacement = chunk.new_lines.as_slice();
        let mut found = seek_sequence(original_lines, pattern, line_index, chunk.is_end_of_file);
        if found.is_none() && pattern.last().is_some_and(String::is_empty) {
            pattern = &pattern[..pattern.len() - 1];
            if replacement.last().is_some_and(String::is_empty) {
                replacement = &replacement[..replacement.len() - 1];
            }
            found = seek_sequence(original_lines, pattern, line_index, chunk.is_end_of_file);
        }
        let Some(start) = found else {
            return Err(PatchApplicationError(format!(
                "Failed to find expected lines in {path}:\n{}",
                chunk.old_lines.join("\n")
            )));
        };
        replacements.push((start, pattern.len(), replacement.to_vec()));
        line_index = start + pattern.len();
    }
    replacements.sort_by_key(|(start, _, _)| *start);
    Ok(replacements)
}

fn apply_replacements(
    mut lines: Vec<String>,
    replacements: &[(usize, usize, Vec<String>)],
) -> Vec<String> {
    for (start, old_length, replacement) in replacements.iter().rev() {
        lines.splice(
            *start..start.saturating_add(*old_length),
            replacement.iter().cloned(),
        );
    }
    lines
}

fn seek_sequence(lines: &[String], pattern: &[String], start: usize, eof: bool) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }
    if pattern.len() > lines.len() {
        return None;
    }
    let last_start = lines.len() - pattern.len();
    if !eof && start > last_start {
        return None;
    }
    let search_start = if eof { last_start } else { start };
    for matcher in [
        LineMatch::Exact,
        LineMatch::TrimEnd,
        LineMatch::Trim,
        LineMatch::Normalized,
    ] {
        for index in search_start..=last_start {
            if lines[index..index + pattern.len()]
                .iter()
                .zip(pattern)
                .all(|(line, expected)| matcher.matches(line, expected))
            {
                return Some(index);
            }
        }
    }
    None
}

#[derive(Clone, Copy)]
enum LineMatch {
    Exact,
    TrimEnd,
    Trim,
    Normalized,
}

impl LineMatch {
    fn matches(self, line: &str, expected: &str) -> bool {
        match self {
            Self::Exact => line == expected,
            Self::TrimEnd => line.trim_end() == expected.trim_end(),
            Self::Trim => line.trim() == expected.trim(),
            Self::Normalized => normalize_line(line) == normalize_line(expected),
        }
    }
}

fn normalize_line(input: &str) -> String {
    input
        .trim()
        .chars()
        .map(|character| match character {
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => '-',
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            '\u{00A0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
            | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{202F}' | '\u{205F}'
            | '\u{3000}' => ' ',
            other => other,
        })
        .collect()
}

fn resolve_existing_patch_file(
    workspace: &Path,
    raw_path: &str,
) -> Result<PathBuf, ToolRuntimeError> {
    let target = resolve_patch_target(workspace, raw_path)?;
    validate_existing_regular_file(workspace, &target, raw_path)?;
    Ok(target)
}

fn resolve_patch_target(workspace: &Path, raw_path: &str) -> Result<PathBuf, ToolRuntimeError> {
    let path_text = raw_path.trim();
    if path_text.is_empty() {
        return Err(ToolRuntimeError::InvalidPath(
            "apply_patch path must not be empty".to_string(),
        ));
    }
    let requested = Path::new(path_text);
    if requested.file_name().is_none() {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "apply_patch path must include a file name: {path_text}"
        )));
    }
    if requested
        .components()
        .any(|component| matches!(component, Component::ParentDir))
        || (!requested.is_absolute()
            && requested
                .components()
                .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir)))
    {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "apply_patch path escapes the workspace: {path_text}"
        )));
    }

    let target = if requested.is_absolute() {
        canonicalize_absolute_target(requested)?
    } else {
        workspace.join(requested)
    };
    if !target.starts_with(workspace) {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "apply_patch path escapes the workspace: {path_text}"
        )));
    }
    let relative = target.strip_prefix(workspace).map_err(|_| {
        ToolRuntimeError::InvalidPath(format!(
            "apply_patch path escapes the workspace: {path_text}"
        ))
    })?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "apply_patch path escapes the workspace: {path_text}"
        )));
    }

    Ok(target)
}

fn canonicalize_absolute_target(requested: &Path) -> Result<PathBuf, ToolRuntimeError> {
    let mut unresolved = Vec::<OsString>::new();
    let mut ancestor = requested;
    loop {
        match fs::canonicalize(ancestor) {
            Ok(mut canonical) => {
                for component in unresolved.iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                let component = ancestor.file_name().ok_or_else(|| {
                    ToolRuntimeError::InvalidPath(format!(
                        "apply_patch path must include a file name: {}",
                        requested.display()
                    ))
                })?;
                unresolved.push(component.to_os_string());
                ancestor = ancestor.parent().ok_or_else(|| {
                    ToolRuntimeError::InvalidPath(format!(
                        "apply_patch path escapes the workspace: {}",
                        requested.display()
                    ))
                })?;
            }
            Err(source) => {
                return Err(ToolRuntimeError::Io {
                    path: ancestor.to_path_buf(),
                    source,
                });
            }
        }
    }
}

fn prepare_write_target(
    workspace: &Path,
    target: &Path,
    raw_path: &str,
) -> Result<(), ToolRuntimeError> {
    match fs::symlink_metadata(target) {
        Ok(_) => validate_existing_regular_file(workspace, target, raw_path),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            ensure_safe_parent_dirs(workspace, target, raw_path)
        }
        Err(source) => Err(ToolRuntimeError::Io {
            path: target.to_path_buf(),
            source,
        }),
    }
}

fn validate_existing_regular_file(
    workspace: &Path,
    target: &Path,
    raw_path: &str,
) -> Result<(), ToolRuntimeError> {
    validate_existing_components(workspace, target, raw_path, true)
}

fn ensure_safe_parent_dirs(
    workspace: &Path,
    target: &Path,
    raw_path: &str,
) -> Result<(), ToolRuntimeError> {
    let parent = target.parent().ok_or_else(|| {
        ToolRuntimeError::InvalidPath(format!(
            "apply_patch path must include a file name: {raw_path}"
        ))
    })?;
    let relative_parent = parent.strip_prefix(workspace).map_err(|_| {
        ToolRuntimeError::InvalidPath(format!(
            "apply_patch path escapes the workspace: {raw_path}"
        ))
    })?;
    let mut current = workspace.to_path_buf();
    for component in relative_parent.components() {
        let Component::Normal(component) = component else {
            return Err(ToolRuntimeError::InvalidPath(format!(
                "apply_patch path escapes the workspace: {raw_path}"
            )));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                validate_directory_component(workspace, &current, &metadata.file_type(), raw_path)?
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|source| ToolRuntimeError::Io {
                    path: current.clone(),
                    source,
                })?;
                let metadata =
                    fs::symlink_metadata(&current).map_err(|source| ToolRuntimeError::Io {
                        path: current.clone(),
                        source,
                    })?;
                validate_directory_component(workspace, &current, &metadata.file_type(), raw_path)?;
            }
            Err(source) => {
                return Err(ToolRuntimeError::Io {
                    path: current.clone(),
                    source,
                });
            }
        }
    }
    Ok(())
}

fn validate_existing_components(
    workspace: &Path,
    target: &Path,
    raw_path: &str,
    target_must_be_file: bool,
) -> Result<(), ToolRuntimeError> {
    let relative = target.strip_prefix(workspace).map_err(|_| {
        ToolRuntimeError::InvalidPath(format!(
            "apply_patch path escapes the workspace: {raw_path}"
        ))
    })?;
    let mut current = workspace.to_path_buf();
    let components = relative.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            return Err(ToolRuntimeError::InvalidPath(format!(
                "apply_patch path escapes the workspace: {raw_path}"
            )));
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|source| ToolRuntimeError::Io {
            path: current.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ToolRuntimeError::InvalidPath(format!(
                "apply_patch rejects symlink paths: {raw_path}"
            )));
        }
        if index + 1 == components.len() {
            if target_must_be_file && !metadata.is_file() {
                return Err(ToolRuntimeError::NotFile(current));
            }
        } else {
            validate_directory_component(workspace, &current, &metadata.file_type(), raw_path)?;
        }
    }
    Ok(())
}

fn validate_directory_component(
    workspace: &Path,
    path: &Path,
    file_type: &FileType,
    raw_path: &str,
) -> Result<(), ToolRuntimeError> {
    if file_type.is_symlink() {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "apply_patch rejects symlink paths: {raw_path}"
        )));
    }
    if !file_type.is_dir() {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "apply_patch parent is not a directory: {raw_path}"
        )));
    }
    let canonical = fs::canonicalize(path).map_err(|source| ToolRuntimeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !canonical.starts_with(workspace) {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "apply_patch path escapes the workspace: {raw_path}"
        )));
    }
    Ok(())
}

fn display_patch_path(path: &str) -> String {
    path.trim().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    fn patch(body: &str) -> String {
        format!("*** Begin Patch\n{body}\n*** End Patch")
    }

    #[test]
    fn parse_patch_accepts_heredoc_and_multiple_file_operations() {
        let parsed = parse_patch(&format!(
            "<<'EOF'\n{}\nEOF",
            patch(
                "*** Add File: add.txt\n+hello\n*** Delete File: remove.txt\n*** Update File: old.txt\n*** Move to: new.txt\n@@ section\n-old\n+new"
            )
        ))
        .expect("patch should parse");

        assert_eq!(parsed.len(), 3);
        assert!(matches!(parsed[0], PatchHunk::AddFile { .. }));
        assert!(matches!(parsed[1], PatchHunk::DeleteFile { .. }));
        assert!(matches!(parsed[2], PatchHunk::UpdateFile { .. }));
    }

    #[test]
    fn parse_patch_reports_invalid_hunk_line_for_empty_update() {
        let error = parse_patch(&patch("*** Update File: test.txt")).expect_err("must fail");

        assert_eq!(
            error.to_string(),
            "invalid hunk at line 2: Update file hunk for path 'test.txt' is empty"
        );
    }

    #[test]
    fn parse_patch_accepts_unmarked_first_change_context_and_empty_added_line() {
        let parsed =
            parse_patch("*** Begin Patch\n*** Update File: test.txt\nold\n+\n*** End Patch")
                .expect("patch should parse");
        let PatchHunk::UpdateFile { chunks, .. } = &parsed[0] else {
            panic!("expected update hunk");
        };

        assert_eq!(chunks[0].old_lines, vec!["old"]);
        assert_eq!(chunks[0].new_lines, vec!["old", ""]);
    }

    #[test]
    fn parse_patch_rejects_missing_boundaries() {
        let error = parse_patch("not a patch").expect_err("must fail");

        assert_eq!(
            error.to_string(),
            "invalid patch: The first line of the patch must be '*** Begin Patch'"
        );
    }

    #[test]
    fn apply_patch_rejects_empty_patch_input() {
        let directory = tempdir().expect("temp directory");

        let error = apply_patch(directory.path(), json!({ "patch": "", "timeoutMs": null }))
            .expect_err("empty patch must fail");

        assert_eq!(error.to_string(), "invalid patch: patch must not be empty");
    }

    #[test]
    fn apply_patch_rejects_unknown_input_fields() {
        let directory = tempdir().expect("temp directory");

        let error = apply_patch(
            directory.path(),
            json!({
                "patch": patch("*** Add File: test.txt\n+test"),
                "timeoutMs": null,
                "extra": true,
            }),
        )
        .expect_err("unknown field must fail");

        assert!(error.to_string().contains("unknown field `extra`"));
    }

    #[test]
    fn apply_patch_updates_multiple_chunks_using_unicode_fuzzy_matching() {
        let directory = tempdir().expect("temp directory");
        fs::write(
            directory.path().join("unicode.txt"),
            "first\nlocal import – avoids top‑level dep\nthird\n",
        )
        .expect("seed file");
        let result = apply_patch(
            directory.path(),
            json!({
                "patch": patch("*** Update File: unicode.txt\n@@\n-first\n+FIRST\n@@\n-local import - avoids top-level dep\n+changed"),
                "timeoutMs": null,
            }),
        )
        .expect("patch should apply");

        assert_eq!(
            fs::read_to_string(directory.path().join("unicode.txt")).expect("updated file"),
            "FIRST\nchanged\nthird\n"
        );
        assert_eq!(
            result["summary"],
            "Success. Updated the following files:\nM unicode.txt\n"
        );
    }

    #[test]
    fn apply_patch_adds_nested_file_moves_and_deletes() {
        let directory = tempdir().expect("temp directory");
        fs::write(directory.path().join("move.txt"), "old\n").expect("seed move source");
        fs::write(directory.path().join("delete.txt"), "delete\n").expect("seed delete source");
        let result = apply_patch(
            directory.path(),
            json!({
                "patch": patch("*** Add File: nested/add.txt\n+added\n*** Update File: move.txt\n*** Move to: nested/moved.txt\n@@\n-old\n+new\n*** Delete File: delete.txt"),
                "timeoutMs": null,
            }),
        )
        .expect("patch should apply");

        assert_eq!(
            fs::read_to_string(directory.path().join("nested/add.txt")).expect("added file"),
            "added\n"
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("nested/moved.txt")).expect("moved file"),
            "new\n"
        );
        assert!(!directory.path().join("move.txt").exists());
        assert!(!directory.path().join("delete.txt").exists());
        assert_eq!(
            result["summary"],
            "Success. Updated the following files:\nA nested/add.txt\nM nested/moved.txt\nD delete.txt\n"
        );
    }

    #[test]
    fn apply_patch_retains_successful_prefix_when_later_hunk_fails() {
        let directory = tempdir().expect("temp directory");
        let error = apply_patch(
            directory.path(),
            json!({
                "patch": patch("*** Add File: created.txt\n+created\n*** Update File: missing.txt\n@@\n-old\n+new"),
                "timeoutMs": null,
            }),
        )
        .expect_err("second hunk must fail");

        assert!(error.to_string().contains("missing.txt"));
        assert_eq!(
            fs::read_to_string(directory.path().join("created.txt")).expect("first hunk persists"),
            "created\n"
        );
    }

    #[test]
    fn apply_patch_rejects_workspace_escape_paths() {
        let directory = tempdir().expect("temp directory");
        let error = apply_patch(
            directory.path(),
            json!({
                "patch": patch("*** Add File: ../outside.txt\n+nope"),
                "timeoutMs": null,
            }),
        )
        .expect_err("path must be rejected");

        assert_eq!(
            error.to_string(),
            "apply_patch path escapes the workspace: ../outside.txt"
        );
    }

    #[cfg(unix)]
    #[test]
    fn apply_patch_rejects_symlink_escape_while_creating_parent_directories() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().expect("temp directory");
        let outside = tempdir().expect("outside directory");
        symlink(outside.path(), directory.path().join("linked")).expect("symlink");
        let error = apply_patch(
            directory.path(),
            json!({
                "patch": patch("*** Add File: linked/escape.txt\n+nope"),
                "timeoutMs": null,
            }),
        )
        .expect_err("symlink must be rejected");

        assert_eq!(
            error.to_string(),
            "apply_patch rejects symlink paths: linked/escape.txt"
        );
    }

    #[test]
    fn apply_patch_accepts_absolute_internal_path_and_eof_addition() {
        let directory = tempdir().expect("temp directory");
        let target = directory.path().join("absolute.txt");
        fs::write(&target, "before\n").expect("seed file");
        apply_patch(
            directory.path(),
            json!({
                "patch": patch(&format!(
                    "*** Update File: {}\n@@\n+after\n*** End of File",
                    target.display()
                )),
                "timeoutMs": null,
            }),
        )
        .expect("patch should apply");

        assert_eq!(
            fs::read_to_string(target).expect("updated file"),
            "before\nafter\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn apply_patch_rejects_dangling_target_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().expect("temp directory");
        symlink("missing-target.txt", directory.path().join("link.txt")).expect("symlink");
        let error = apply_patch(
            directory.path(),
            json!({
                "patch": patch("*** Add File: link.txt\n+nope"),
                "timeoutMs": null,
            }),
        )
        .expect_err("dangling symlink must be rejected");

        assert_eq!(
            error.to_string(),
            "apply_patch rejects symlink paths: link.txt"
        );
    }
}
