use std::{
    fmt, fs, io,
    path::{Component, Path, PathBuf},
    process::Command,
};

use foco_store::workspace::{
    CodeGraphReferenceRecord, CodeGraphRelatedFileRecord, CodeGraphSymbolRecord,
    CodeGraphSymbolRelationRecord, WorkspaceDatabase, WorkspaceDatabaseError,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const READ_FILE_TOOL: &str = "read_file";
pub const LIST_FILES_TOOL: &str = "list_files";
pub const SEARCH_TEXT_TOOL: &str = "search_text";
pub const WRITE_FILE_TOOL: &str = "write_file";
pub const RUN_COMMAND_TOOL: &str = "run_command";
pub const GIT_DIFF_TOOL: &str = "git_diff";
pub const GRAPH_FIND_SYMBOLS_TOOL: &str = "graph_find_symbols";
pub const GRAPH_FIND_CALLERS_TOOL: &str = "graph_find_callers";
pub const GRAPH_FIND_CALLEES_TOOL: &str = "graph_find_callees";
pub const GRAPH_FIND_REFERENCES_TOOL: &str = "graph_find_references";
pub const GRAPH_RELATED_FILES_TOOL: &str = "graph_related_files";

const MAX_READ_BYTES: u64 = 512 * 1024;
const MAX_LIST_ENTRIES: usize = 200;
const MAX_SEARCH_MATCHES: usize = 200;
const MAX_COMMAND_OUTPUT_BYTES: usize = 64 * 1024;
const DEFAULT_GRAPH_RESULT_LIMIT: usize = 20;
const MAX_GRAPH_RESULT_LIMIT: usize = 50;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub strict: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecution {
    pub output: Value,
    pub is_error: bool,
}

pub fn builtin_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        read_file_definition(),
        list_files_definition(),
        graph_find_symbols_definition(),
        graph_find_callers_definition(),
        graph_find_callees_definition(),
        graph_find_references_definition(),
        graph_related_files_definition(),
        search_text_definition(),
        write_file_definition(),
        run_command_definition(),
        git_diff_definition(),
    ]
}

pub fn execute_builtin_tool(
    workspace_path: &Path,
    tool_name: &str,
    arguments: Value,
) -> ToolExecution {
    match execute_builtin_tool_inner(workspace_path, tool_name, arguments) {
        Ok(output) => ToolExecution {
            output,
            is_error: false,
        },
        Err(error) => ToolExecution {
            output: json!({ "error": error.to_string() }),
            is_error: true,
        },
    }
}

fn execute_builtin_tool_inner(
    workspace_path: &Path,
    tool_name: &str,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    match tool_name {
        READ_FILE_TOOL => read_file(workspace_path, arguments),
        LIST_FILES_TOOL => list_files(workspace_path, arguments),
        GRAPH_FIND_SYMBOLS_TOOL => graph_find_symbols(workspace_path, arguments),
        GRAPH_FIND_CALLERS_TOOL => graph_find_callers(workspace_path, arguments),
        GRAPH_FIND_CALLEES_TOOL => graph_find_callees(workspace_path, arguments),
        GRAPH_FIND_REFERENCES_TOOL => graph_find_references(workspace_path, arguments),
        GRAPH_RELATED_FILES_TOOL => graph_related_files(workspace_path, arguments),
        SEARCH_TEXT_TOOL => search_text(workspace_path, arguments),
        WRITE_FILE_TOOL => write_file(workspace_path, arguments),
        RUN_COMMAND_TOOL => run_command(workspace_path, arguments),
        GIT_DIFF_TOOL => git_diff(workspace_path, arguments),
        other => Err(ToolRuntimeError::UnknownTool(other.to_string())),
    }
}

fn read_file(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: ReadFileInput = parse_arguments(arguments)?;
    let path = resolve_workspace_file(workspace_path, &request.path)?;
    let metadata = fs::metadata(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    if !metadata.is_file() {
        return Err(ToolRuntimeError::NotFile(path));
    }

    if metadata.len() > MAX_READ_BYTES {
        return Err(ToolRuntimeError::FileTooLarge {
            path,
            bytes: metadata.len(),
            max_bytes: MAX_READ_BYTES,
        });
    }

    let content = fs::read_to_string(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    Ok(json!({
        "path": request.path,
        "content": content,
        "bytes": metadata.len()
    }))
}

fn list_files(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: ListFilesInput = parse_arguments(arguments)?;
    let input_path = request.path;
    let path = resolve_workspace_path(workspace_path, &input_path)?;
    let metadata = fs::metadata(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    if !metadata.is_dir() {
        return Err(ToolRuntimeError::NotDirectory(path));
    }

    let mut entries = fs::read_dir(&path)
        .map_err(|source| ToolRuntimeError::Io {
            path: path.clone(),
            source,
        })?
        .map(|entry| {
            let entry = entry.map_err(|source| ToolRuntimeError::Io {
                path: path.clone(),
                source,
            })?;
            let entry_path = entry.path();
            let metadata = entry.metadata().map_err(|source| ToolRuntimeError::Io {
                path: entry_path.clone(),
                source,
            })?;
            let kind = if metadata.is_dir() {
                "directory"
            } else if metadata.is_file() {
                "file"
            } else {
                "other"
            };

            Ok(json!({
                "path": relative_workspace_path(workspace_path, &entry_path)?,
                "kind": kind,
                "bytes": if metadata.is_file() { Some(metadata.len()) } else { None }
            }))
        })
        .collect::<Result<Vec<_>, ToolRuntimeError>>()?;

    entries.sort_by(|left, right| {
        left.get("path")
            .and_then(Value::as_str)
            .cmp(&right.get("path").and_then(Value::as_str))
    });
    let truncated = entries.len() > MAX_LIST_ENTRIES;
    entries.truncate(MAX_LIST_ENTRIES);

    Ok(json!({
        "path": input_path,
        "entries": entries,
        "truncated": truncated
    }))
}

fn graph_find_symbols(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: GraphFindSymbolsInput = parse_arguments(arguments)?;
    let query = non_empty_argument("query", &request.query)?;
    let path = request
        .path
        .as_deref()
        .map(normalize_workspace_path_text)
        .transpose()?;
    let limit = graph_limit(request.limit)?;
    let database = open_code_graph_database(workspace_path)?;
    let mut symbols = database.find_code_graph_symbols(
        query,
        request.kind.as_deref(),
        path.as_deref(),
        graph_query_limit(limit)?,
    )?;
    let truncated = truncate_records(&mut symbols, limit);

    Ok(json!({
        "query": query,
        "kind": request.kind,
        "path": path,
        "symbols": symbols.into_iter().map(symbol_json).collect::<Vec<_>>(),
        "truncated": truncated
    }))
}

fn graph_find_callers(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: GraphSymbolLookupInput = parse_arguments(arguments)?;
    let database = open_code_graph_database(workspace_path)?;
    let symbol = resolve_graph_symbol(&database, &request)?;
    let limit = graph_limit(request.limit)?;
    let mut callers = database.code_graph_callers(symbol.id, graph_query_limit(limit)?)?;
    let truncated = truncate_records(&mut callers, limit);

    Ok(json!({
        "symbol": symbol_json(symbol),
        "callers": callers.into_iter().map(relation_json).collect::<Vec<_>>(),
        "truncated": truncated
    }))
}

fn graph_find_callees(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: GraphSymbolLookupInput = parse_arguments(arguments)?;
    let database = open_code_graph_database(workspace_path)?;
    let symbol = resolve_graph_symbol(&database, &request)?;
    let limit = graph_limit(request.limit)?;
    let mut callees = database.code_graph_callees(symbol.id, graph_query_limit(limit)?)?;
    let truncated = truncate_records(&mut callees, limit);

    Ok(json!({
        "symbol": symbol_json(symbol),
        "callees": callees.into_iter().map(relation_json).collect::<Vec<_>>(),
        "truncated": truncated
    }))
}

fn graph_find_references(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GraphSymbolLookupInput = parse_arguments(arguments)?;
    let database = open_code_graph_database(workspace_path)?;
    let symbol = resolve_graph_symbol(&database, &request)?;
    let limit = graph_limit(request.limit)?;
    let mut references = database.code_graph_references(symbol.id, graph_query_limit(limit)?)?;
    let truncated = truncate_records(&mut references, limit);

    Ok(json!({
        "symbol": symbol_json(symbol),
        "references": references.into_iter().map(reference_json).collect::<Vec<_>>(),
        "truncated": truncated
    }))
}

fn graph_related_files(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: GraphRelatedFilesInput = parse_arguments(arguments)?;
    let path = normalize_workspace_path_text(&request.path)?;
    let limit = graph_limit(request.limit)?;
    let database = open_code_graph_database(workspace_path)?;
    let mut files = database.code_graph_related_files(&path, graph_query_limit(limit)?)?;
    let truncated = truncate_records(&mut files, limit);

    Ok(json!({
        "path": path,
        "files": files.into_iter().map(related_file_json).collect::<Vec<_>>(),
        "truncated": truncated
    }))
}

fn search_text(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: SearchTextInput = parse_arguments(arguments)?;
    let input_path = request.path;
    let path = resolve_workspace_path(workspace_path, &input_path)?;
    let pattern = request.query.trim();

    if pattern.is_empty() {
        return Err(ToolRuntimeError::InvalidArguments(
            "query must not be empty".to_string(),
        ));
    }

    let output = Command::new("rg")
        .arg("--json")
        .arg("--line-number")
        .arg("--max-count")
        .arg(MAX_SEARCH_MATCHES.to_string())
        .arg(pattern)
        .arg(&path)
        .current_dir(workspace_path)
        .output()
        .map_err(|source| ToolRuntimeError::Command {
            command: "rg".to_string(),
            source,
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if output.status.code() == Some(1) {
            return Ok(json!({
                "query": pattern,
                "path": input_path,
                "matches": [],
                "truncated": false
            }));
        }

        return Err(ToolRuntimeError::CommandFailed {
            command: "rg".to_string(),
            status: output.status.code(),
            stderr,
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut matches = Vec::new();
    let mut truncated = false;
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
            truncated = true;
            break;
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

        matches.push(json!({
            "path": relative_workspace_path(workspace_path, Path::new(absolute_path))?,
            "line": line_number,
            "text": text
        }));
    }

    Ok(json!({
        "query": pattern,
        "path": input_path,
        "matches": matches,
        "truncated": truncated
    }))
}

fn write_file(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: WriteFileInput = parse_arguments(arguments)?;
    let path = resolve_workspace_write_path(workspace_path, &request.path)?;

    if let Ok(metadata) = fs::metadata(&path)
        && !metadata.is_file()
    {
        return Err(ToolRuntimeError::NotFile(path));
    }

    fs::write(&path, &request.content).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    Ok(json!({
        "path": normalize_workspace_path_text(&request.path)?,
        "bytes": request.content.len()
    }))
}

fn run_command(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: RunCommandInput = parse_arguments(arguments)?;
    let command = request.command.trim();
    let args = request.args.unwrap_or_default();
    let cwd = match request.cwd.as_deref() {
        Some(cwd) => resolve_workspace_path(workspace_path, cwd)?,
        None => fs::canonicalize(workspace_path).map_err(|source| ToolRuntimeError::Io {
            path: workspace_path.to_path_buf(),
            source,
        })?,
    };

    if command.is_empty() {
        return Err(ToolRuntimeError::InvalidArguments(
            "command must not be empty".to_string(),
        ));
    }

    if !fs::metadata(&cwd)
        .map_err(|source| ToolRuntimeError::Io {
            path: cwd.clone(),
            source,
        })?
        .is_dir()
    {
        return Err(ToolRuntimeError::NotDirectory(cwd));
    }

    let output = Command::new(command)
        .args(&args)
        .current_dir(&cwd)
        .output()
        .map_err(|source| ToolRuntimeError::Command {
            command: command.to_string(),
            source,
        })?;
    let (stdout, stdout_truncated) = limited_output_text(&output.stdout);
    let (stderr, stderr_truncated) = limited_output_text(&output.stderr);

    Ok(json!({
        "command": command,
        "args": args,
        "cwd": relative_workspace_path(workspace_path, &cwd)?,
        "status": output.status.code(),
        "success": output.status.success(),
        "stdout": stdout,
        "stderr": stderr,
        "stdoutTruncated": stdout_truncated,
        "stderrTruncated": stderr_truncated
    }))
}

fn git_diff(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: GitDiffInput = parse_arguments(arguments)?;
    let workspace = fs::canonicalize(workspace_path).map_err(|source| ToolRuntimeError::Io {
        path: workspace_path.to_path_buf(),
        source,
    })?;
    ensure_git_repository(&workspace)?;

    let path = request
        .path
        .as_deref()
        .map(normalize_workspace_path_text)
        .transpose()?;
    let mut status_args = vec!["status".to_string(), "--short".to_string()];
    let mut diff_args = vec!["diff".to_string()];
    let mut staged_diff_args = vec!["diff".to_string(), "--cached".to_string()];

    if let Some(path) = &path {
        status_args.push("--".to_string());
        status_args.push(path.clone());
        diff_args.push("--".to_string());
        diff_args.push(path.clone());
        staged_diff_args.push("--".to_string());
        staged_diff_args.push(path.clone());
    }

    Ok(json!({
        "path": path,
        "status": run_git_text(&workspace, &status_args)?,
        "diff": run_git_text(&workspace, &diff_args)?,
        "stagedDiff": run_git_text(&workspace, &staged_diff_args)?
    }))
}

fn open_code_graph_database(workspace_path: &Path) -> Result<WorkspaceDatabase, ToolRuntimeError> {
    WorkspaceDatabase::open_or_create(workspace_path).map_err(ToolRuntimeError::WorkspaceDatabase)
}

fn resolve_graph_symbol(
    database: &WorkspaceDatabase,
    request: &GraphSymbolLookupInput,
) -> Result<CodeGraphSymbolRecord, ToolRuntimeError> {
    let symbol = request
        .symbol
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match (request.symbol_id, symbol) {
        (Some(_), Some(_)) => Err(ToolRuntimeError::InvalidArguments(
            "provide exactly one of symbolId or symbol".to_string(),
        )),
        (None, None) => Err(ToolRuntimeError::InvalidArguments(
            "provide exactly one of symbolId or symbol".to_string(),
        )),
        (Some(symbol_id), None) => {
            if request.path.is_some() {
                return Err(ToolRuntimeError::InvalidArguments(
                    "path can only be used when resolving by symbol name".to_string(),
                ));
            }

            database.code_graph_symbol(symbol_id)?.ok_or_else(|| {
                ToolRuntimeError::InvalidArguments(format!(
                    "code graph symbol was not found: {symbol_id}"
                ))
            })
        }
        (None, Some(symbol)) => {
            let path = request
                .path
                .as_deref()
                .map(normalize_workspace_path_text)
                .transpose()?;
            let matches = database.find_code_graph_symbols(symbol, None, path.as_deref(), 3)?;

            match matches.len() {
                0 => Err(ToolRuntimeError::InvalidArguments(format!(
                    "code graph symbol was not found: {symbol}"
                ))),
                1 => Ok(matches.into_iter().next().expect("one symbol")),
                _ => {
                    let candidates = matches
                        .into_iter()
                        .map(|candidate| {
                            format!("{}:{}:{}", candidate.id, candidate.path, candidate.name)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    Err(ToolRuntimeError::InvalidArguments(format!(
                        "symbol name is ambiguous; call graph_find_symbols first and pass symbolId. Candidates: {candidates}"
                    )))
                }
            }
        }
    }
}

fn non_empty_argument<'a>(name: &str, value: &'a str) -> Result<&'a str, ToolRuntimeError> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        Err(ToolRuntimeError::InvalidArguments(format!(
            "{name} must not be empty"
        )))
    } else {
        Ok(trimmed)
    }
}

fn graph_limit(limit: Option<usize>) -> Result<usize, ToolRuntimeError> {
    let limit = limit.unwrap_or(DEFAULT_GRAPH_RESULT_LIMIT);

    if limit == 0 || limit > MAX_GRAPH_RESULT_LIMIT {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "limit must be between 1 and {MAX_GRAPH_RESULT_LIMIT}"
        )));
    }

    Ok(limit)
}

fn graph_query_limit(limit: usize) -> Result<i64, ToolRuntimeError> {
    i64::try_from(limit + 1).map_err(|_| {
        ToolRuntimeError::InvalidArguments("limit is too large for SQLite".to_string())
    })
}

fn truncate_records<T>(records: &mut Vec<T>, limit: usize) -> bool {
    let truncated = records.len() > limit;
    records.truncate(limit);
    truncated
}

fn symbol_json(symbol: CodeGraphSymbolRecord) -> Value {
    json!({
        "symbolId": symbol.id,
        "path": symbol.path,
        "language": symbol.language,
        "name": symbol.name,
        "kind": symbol.kind,
        "startLine": symbol.start_line,
        "startColumn": symbol.start_column,
        "endLine": symbol.end_line,
        "endColumn": symbol.end_column,
        "signature": symbol.signature,
        "documentation": symbol.documentation
    })
}

fn relation_json(relation: CodeGraphSymbolRelationRecord) -> Value {
    json!({
        "edgeId": relation.edge_id,
        "edgeKind": relation.edge_kind,
        "metadata": relation.metadata_json,
        "source": symbol_json(relation.source),
        "target": symbol_json(relation.target)
    })
}

fn reference_json(reference: CodeGraphReferenceRecord) -> Value {
    json!({
        "referenceId": reference.id,
        "path": reference.path,
        "language": reference.language,
        "name": reference.name,
        "startLine": reference.start_line,
        "startColumn": reference.start_column,
        "endLine": reference.end_line,
        "endColumn": reference.end_column,
        "symbol": reference.symbol.map(symbol_json)
    })
}

fn related_file_json(file: CodeGraphRelatedFileRecord) -> Value {
    json!({
        "path": file.path,
        "language": file.language,
        "relation": file.relation,
        "score": file.score
    })
}

fn parse_arguments<T>(arguments: Value) -> Result<T, ToolRuntimeError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(arguments).map_err(|source| {
        ToolRuntimeError::InvalidArguments(format!("tool arguments do not match schema: {source}"))
    })
}

fn resolve_workspace_file(workspace_path: &Path, input: &str) -> Result<PathBuf, ToolRuntimeError> {
    let path = resolve_workspace_path(workspace_path, input)?;
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

fn resolve_workspace_path(workspace_path: &Path, input: &str) -> Result<PathBuf, ToolRuntimeError> {
    let trimmed = normalize_workspace_path_text(input)?;
    let requested = Path::new(&trimmed);

    let workspace = fs::canonicalize(workspace_path).map_err(|source| ToolRuntimeError::Io {
        path: workspace_path.to_path_buf(),
        source,
    })?;
    let path =
        fs::canonicalize(workspace.join(requested)).map_err(|source| ToolRuntimeError::Io {
            path: workspace.join(requested),
            source,
        })?;

    if !path.starts_with(&workspace) {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "path escapes the workspace: {trimmed}"
        )));
    }

    Ok(path)
}

fn resolve_workspace_write_path(
    workspace_path: &Path,
    input: &str,
) -> Result<PathBuf, ToolRuntimeError> {
    let trimmed = normalize_workspace_path_text(input)?;
    let requested = Path::new(&trimmed);
    let Some(file_name) = requested.file_name() else {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "write_file path must include a file name: {trimmed}"
        )));
    };
    let parent = requested.parent().unwrap_or_else(|| Path::new("."));
    let workspace = fs::canonicalize(workspace_path).map_err(|source| ToolRuntimeError::Io {
        path: workspace_path.to_path_buf(),
        source,
    })?;
    let parent_path =
        fs::canonicalize(workspace.join(parent)).map_err(|source| ToolRuntimeError::Io {
            path: workspace.join(parent),
            source,
        })?;

    if !parent_path.starts_with(&workspace) {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "path escapes the workspace: {trimmed}"
        )));
    }

    let path = parent_path.join(file_name);
    if path.exists() {
        let canonical_path = fs::canonicalize(&path).map_err(|source| ToolRuntimeError::Io {
            path: path.clone(),
            source,
        })?;

        if !canonical_path.starts_with(&workspace) {
            return Err(ToolRuntimeError::InvalidPath(format!(
                "path escapes the workspace: {trimmed}"
            )));
        }

        Ok(canonical_path)
    } else {
        Ok(path)
    }
}

fn normalize_workspace_path_text(input: &str) -> Result<String, ToolRuntimeError> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(ToolRuntimeError::InvalidPath(
            "path must not be empty".to_string(),
        ));
    }

    let requested = Path::new(trimmed);
    if requested.is_absolute() {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "path must be relative to the workspace: {trimmed}"
        )));
    }

    for component in requested.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(ToolRuntimeError::InvalidPath(format!(
                "path escapes the workspace: {trimmed}"
            )));
        }
    }

    Ok(trimmed.replace('\\', "/"))
}

fn relative_workspace_path(workspace_path: &Path, path: &Path) -> Result<String, ToolRuntimeError> {
    let workspace = fs::canonicalize(workspace_path).map_err(|source| ToolRuntimeError::Io {
        path: workspace_path.to_path_buf(),
        source,
    })?;
    let path = fs::canonicalize(path).map_err(|source| ToolRuntimeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let relative = path.strip_prefix(&workspace).map_err(|_| {
        ToolRuntimeError::InvalidPath(format!("path is outside workspace: {}", path.display()))
    })?;

    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn ensure_git_repository(workspace_path: &Path) -> Result<(), ToolRuntimeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
        .map_err(|source| ToolRuntimeError::Command {
            command: "git".to_string(),
            source,
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    if output.status.success() && stdout.trim() == "true" {
        return Ok(());
    }

    Err(ToolRuntimeError::NotGitRepository {
        path: workspace_path.to_path_buf(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn run_git_text(workspace_path: &Path, args: &[String]) -> Result<String, ToolRuntimeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .args(args)
        .output()
        .map_err(|source| ToolRuntimeError::Command {
            command: "git".to_string(),
            source,
        })?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    Err(ToolRuntimeError::CommandFailed {
        command: format!("git {}", args.join(" ")),
        status: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn limited_output_text(output: &[u8]) -> (String, bool) {
    let truncated = output.len() > MAX_COMMAND_OUTPUT_BYTES;
    let bytes = if truncated {
        &output[..MAX_COMMAND_OUTPUT_BYTES]
    } else {
        output
    };

    (String::from_utf8_lossy(bytes).to_string(), truncated)
}

fn read_file_definition() -> ToolDefinition {
    ToolDefinition {
        name: READ_FILE_TOOL,
        description: "Read a UTF-8 text file inside the active workspace.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative file path."
                }
            },
            "required": ["path"]
        }),
        strict: true,
    }
}

fn list_files_definition() -> ToolDefinition {
    ToolDefinition {
        name: LIST_FILES_TOOL,
        description: "List files and directories in a workspace-relative directory.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative directory path. Use . for the workspace root."
                }
            },
            "required": ["path"]
        }),
        strict: true,
    }
}

fn graph_find_symbols_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_FIND_SYMBOLS_TOOL,
        description: "Find indexed code graph symbols by name, signature, or documentation. Prefer this before full-text search when locating code.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Symbol name or partial text to find."
                },
                "kind": {
                    "type": ["string", "null"],
                    "description": "Optional symbol kind such as function, method, struct, class, enum, trait, variable, or constant."
                },
                "path": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative file or directory path to restrict the query."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Optional result limit from 1 to 50. Defaults to 20."
                }
            },
            "required": ["query", "kind", "path", "limit"]
        }),
        strict: true,
    }
}

fn graph_find_callers_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_FIND_CALLERS_TOOL,
        description: "Find code graph symbols that reference the requested symbol. Use symbolId from graph_find_symbols when names are ambiguous.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "symbolId": {
                    "type": ["integer", "null"],
                    "description": "Exact code graph symbol id returned by graph_find_symbols."
                },
                "symbol": {
                    "type": ["string", "null"],
                    "description": "Symbol name to resolve when it is unique."
                },
                "path": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative file or directory path used only with symbol."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Optional result limit from 1 to 50. Defaults to 20."
                }
            },
            "required": ["symbolId", "symbol", "path", "limit"]
        }),
        strict: true,
    }
}

fn graph_find_callees_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_FIND_CALLEES_TOOL,
        description: "Find code graph symbols referenced by the requested symbol. Use symbolId from graph_find_symbols when names are ambiguous.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "symbolId": {
                    "type": ["integer", "null"],
                    "description": "Exact code graph symbol id returned by graph_find_symbols."
                },
                "symbol": {
                    "type": ["string", "null"],
                    "description": "Symbol name to resolve when it is unique."
                },
                "path": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative file or directory path used only with symbol."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Optional result limit from 1 to 50. Defaults to 20."
                }
            },
            "required": ["symbolId", "symbol", "path", "limit"]
        }),
        strict: true,
    }
}

fn graph_find_references_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_FIND_REFERENCES_TOOL,
        description: "Find indexed reference locations for the requested symbol. Use symbolId from graph_find_symbols when names are ambiguous.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "symbolId": {
                    "type": ["integer", "null"],
                    "description": "Exact code graph symbol id returned by graph_find_symbols."
                },
                "symbol": {
                    "type": ["string", "null"],
                    "description": "Symbol name to resolve when it is unique."
                },
                "path": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative file or directory path used only with symbol."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Optional result limit from 1 to 50. Defaults to 20."
                }
            },
            "required": ["symbolId", "symbol", "path", "limit"]
        }),
        strict: true,
    }
}

fn graph_related_files_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_RELATED_FILES_TOOL,
        description: "Find files related to an indexed workspace file through code graph edges or shared imports.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative indexed file path."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Optional result limit from 1 to 50. Defaults to 20."
                }
            },
            "required": ["path", "limit"]
        }),
        strict: true,
    }
}

fn search_text_definition() -> ToolDefinition {
    ToolDefinition {
        name: SEARCH_TEXT_TOOL,
        description: "Search workspace text with ripgrep and return matching lines.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Ripgrep search pattern."
                },
                "path": {
                    "type": "string",
                    "description": "Workspace-relative path to search. Use . for the workspace root."
                }
            },
            "required": ["query", "path"]
        }),
        strict: true,
    }
}

fn write_file_definition() -> ToolDefinition {
    ToolDefinition {
        name: WRITE_FILE_TOOL,
        description: "Write a complete UTF-8 text file inside the active workspace.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative file path. Parent directories must already exist."
                },
                "content": {
                    "type": "string",
                    "description": "Complete file content to write."
                }
            },
            "required": ["path", "content"]
        }),
        strict: true,
    }
}

fn run_command_definition() -> ToolDefinition {
    ToolDefinition {
        name: RUN_COMMAND_TOOL,
        description: "Run a local command in the active workspace without invoking a shell.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Executable name or path. Do not include arguments here."
                },
                "args": {
                    "type": ["array", "null"],
                    "items": { "type": "string" },
                    "description": "Command arguments."
                },
                "cwd": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative working directory. Defaults to the workspace root."
                }
            },
            "required": ["command", "args", "cwd"]
        }),
        strict: true,
    }
}

fn git_diff_definition() -> ToolDefinition {
    ToolDefinition {
        name: GIT_DIFF_TOOL,
        description: "Return git status plus unstaged and staged diffs for the active workspace.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative path to diff."
                }
            },
            "required": ["path"]
        }),
        strict: true,
    }
}

#[derive(Deserialize)]
struct ReadFileInput {
    path: String,
}

#[derive(Deserialize)]
struct ListFilesInput {
    path: String,
}

#[derive(Deserialize)]
struct SearchTextInput {
    query: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphFindSymbolsInput {
    query: String,
    kind: Option<String>,
    path: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphSymbolLookupInput {
    symbol_id: Option<i64>,
    symbol: Option<String>,
    path: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct GraphRelatedFilesInput {
    path: String,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct WriteFileInput {
    path: String,
    content: String,
}

#[derive(Deserialize)]
struct RunCommandInput {
    command: String,
    args: Option<Vec<String>>,
    cwd: Option<String>,
}

#[derive(Deserialize)]
struct GitDiffInput {
    path: Option<String>,
}

#[derive(Debug)]
enum ToolRuntimeError {
    Command {
        command: String,
        source: io::Error,
    },
    CommandFailed {
        command: String,
        status: Option<i32>,
        stderr: String,
    },
    FileTooLarge {
        path: PathBuf,
        bytes: u64,
        max_bytes: u64,
    },
    InvalidArguments(String),
    InvalidPath(String),
    InvalidToolOutput {
        command: String,
        source: serde_json::Error,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
    NotGitRepository {
        path: PathBuf,
        stderr: String,
    },
    NotDirectory(PathBuf),
    NotFile(PathBuf),
    UnknownTool(String),
    WorkspaceDatabase(WorkspaceDatabaseError),
}

impl fmt::Display for ToolRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command { command, source } => {
                write!(formatter, "failed to run {command}: {source}")
            }
            Self::CommandFailed {
                command,
                status,
                stderr,
            } => write!(
                formatter,
                "{command} exited with status {:?}: {}",
                status, stderr
            ),
            Self::FileTooLarge {
                path,
                bytes,
                max_bytes,
            } => write!(
                formatter,
                "{} is too large to read ({bytes} bytes; max {max_bytes})",
                path.display()
            ),
            Self::InvalidArguments(message) => write!(formatter, "{message}"),
            Self::InvalidPath(message) => write!(formatter, "{message}"),
            Self::InvalidToolOutput { command, source } => {
                write!(formatter, "{command} returned invalid JSON: {source}")
            }
            Self::Io { path, source } => write!(formatter, "{}: {}", path.display(), source),
            Self::NotGitRepository { path, stderr } => {
                if stderr.is_empty() {
                    write!(
                        formatter,
                        "workspace is not a git repository: {}",
                        path.display()
                    )
                } else {
                    write!(
                        formatter,
                        "workspace is not a git repository: {} ({stderr})",
                        path.display()
                    )
                }
            }
            Self::NotDirectory(path) => write!(formatter, "{} is not a directory", path.display()),
            Self::NotFile(path) => write!(formatter, "{} is not a file", path.display()),
            Self::UnknownTool(tool) => write!(formatter, "unknown built-in tool '{tool}'"),
            Self::WorkspaceDatabase(source) => {
                write!(formatter, "code graph database error: {source}")
            }
        }
    }
}

impl std::error::Error for ToolRuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WorkspaceDatabase(source) => Some(source),
            Self::Command { .. }
            | Self::CommandFailed { .. }
            | Self::FileTooLarge { .. }
            | Self::InvalidArguments(_)
            | Self::InvalidPath(_)
            | Self::InvalidToolOutput { .. }
            | Self::Io { .. }
            | Self::NotGitRepository { .. }
            | Self::NotDirectory(_)
            | Self::NotFile(_)
            | Self::UnknownTool(_) => None,
        }
    }
}

impl From<WorkspaceDatabaseError> for ToolRuntimeError {
    fn from(source: WorkspaceDatabaseError) -> Self {
        Self::WorkspaceDatabase(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foco_store::workspace::{
        NewCodeGraphEdge, NewCodeGraphFileIndex, NewCodeGraphImport, NewCodeGraphReference,
        NewCodeGraphSymbol, WorkspaceDatabase,
    };
    use std::collections::BTreeSet;

    #[test]
    fn rejects_paths_outside_workspace() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "../outside.txt" }),
        );

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("escapes the workspace")
        );
    }

    #[test]
    fn reads_workspace_file() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "hello").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "note.txt" }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["content"], "hello");
    }

    #[test]
    fn lists_workspace_files() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("a.txt"), "a").expect("write a");

        let result =
            execute_builtin_tool(workspace.path(), LIST_FILES_TOOL, json!({ "path": "." }));

        assert!(!result.is_error);
        let entries = result.output["entries"].as_array().expect("entries");
        assert_eq!(entries[0]["path"], "a.txt");
    }

    #[test]
    fn rejects_missing_required_tool_arguments() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(workspace.path(), LIST_FILES_TOOL, json!({}));

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("missing field `path`")
        );
    }

    #[test]
    fn strict_tool_schemas_require_every_property() {
        for tool in builtin_tool_definitions() {
            if !tool.strict {
                continue;
            }

            let schema = tool.input_schema.as_object().expect("schema object");
            assert_eq!(
                schema.get("additionalProperties"),
                Some(&Value::Bool(false)),
                "{} schema must reject unknown properties",
                tool.name
            );

            let properties = schema
                .get("properties")
                .and_then(Value::as_object)
                .expect("properties object");
            let required = schema
                .get("required")
                .and_then(Value::as_array)
                .expect("required array");
            let property_names = properties
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            let required_names = required
                .iter()
                .map(|name| name.as_str().expect("required name"))
                .collect::<BTreeSet<_>>();

            assert_eq!(
                required_names, property_names,
                "{} schema required keys must match properties",
                tool.name
            );
        }
    }

    #[test]
    fn accepts_null_for_optional_tool_arguments() {
        let graph_symbols: GraphFindSymbolsInput = parse_arguments(json!({
            "query": "helper",
            "kind": null,
            "path": null,
            "limit": null
        }))
        .expect("graph symbols input");
        assert_eq!(graph_symbols.query, "helper");
        assert_eq!(graph_symbols.kind, None);
        assert_eq!(graph_symbols.path, None);
        assert_eq!(graph_symbols.limit, None);

        let graph_lookup: GraphSymbolLookupInput = parse_arguments(json!({
            "symbolId": null,
            "symbol": "helper",
            "path": null,
            "limit": null
        }))
        .expect("graph lookup input");
        assert_eq!(graph_lookup.symbol_id, None);
        assert_eq!(graph_lookup.symbol.as_deref(), Some("helper"));
        assert_eq!(graph_lookup.path, None);
        assert_eq!(graph_lookup.limit, None);

        let run_command: RunCommandInput = parse_arguments(json!({
            "command": "git",
            "args": null,
            "cwd": null
        }))
        .expect("run command input");
        assert_eq!(run_command.command, "git");
        assert_eq!(run_command.args, None);
        assert_eq!(run_command.cwd, None);

        let git_diff: GitDiffInput =
            parse_arguments(json!({ "path": null })).expect("git diff input");
        assert_eq!(git_diff.path, None);
    }

    #[test]
    fn searches_workspace_text() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "alpha\nbeta\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({ "query": "beta", "path": "." }),
        );

        assert!(!result.is_error);
        let matches = result.output["matches"].as_array().expect("matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["path"], "note.txt");
        assert_eq!(matches[0]["line"], 2);
        assert_eq!(matches[0]["text"], "beta");
    }

    #[test]
    fn graph_tools_return_symbols_and_relationships() {
        let workspace = tempfile::tempdir().expect("workspace");
        insert_graph_fixture(workspace.path());

        let symbols = execute_builtin_tool(
            workspace.path(),
            GRAPH_FIND_SYMBOLS_TOOL,
            json!({ "query": "helper", "limit": 5 }),
        );

        assert!(!symbols.is_error);
        let symbol_id = symbols.output["symbols"][0]["symbolId"]
            .as_i64()
            .expect("symbol id");
        assert_eq!(symbols.output["symbols"][0]["path"], "lib.rs");

        let references = execute_builtin_tool(
            workspace.path(),
            GRAPH_FIND_REFERENCES_TOOL,
            json!({ "symbolId": symbol_id, "limit": 5 }),
        );

        assert!(!references.is_error);
        assert_eq!(references.output["references"][0]["path"], "lib.rs");
        assert_eq!(references.output["references"][0]["name"], "helper");

        let public_api = execute_builtin_tool(
            workspace.path(),
            GRAPH_FIND_SYMBOLS_TOOL,
            json!({ "query": "public_api", "path": "lib.rs", "limit": 5 }),
        );
        let public_api_id = public_api.output["symbols"][0]["symbolId"]
            .as_i64()
            .expect("public api id");
        let callees = execute_builtin_tool(
            workspace.path(),
            GRAPH_FIND_CALLEES_TOOL,
            json!({ "symbolId": public_api_id, "limit": 5 }),
        );

        assert!(!callees.is_error);
        assert_eq!(callees.output["callees"][0]["target"]["name"], "helper");

        let related_files = execute_builtin_tool(
            workspace.path(),
            GRAPH_RELATED_FILES_TOOL,
            json!({ "path": "lib.rs", "limit": 5 }),
        );

        assert!(!related_files.is_error);
        assert_eq!(related_files.output["files"][0]["path"], "caller.rs");
        assert_eq!(
            related_files.output["files"][0]["relation"],
            "shared_import"
        );
    }

    #[test]
    fn writes_workspace_file() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(
            workspace.path(),
            WRITE_FILE_TOOL,
            json!({ "path": "note.txt", "content": "hello" }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["path"], "note.txt");
        assert_eq!(
            fs::read_to_string(workspace.path().join("note.txt")).expect("read note"),
            "hello"
        );
    }

    #[test]
    fn rejects_write_path_outside_workspace() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(
            workspace.path(),
            WRITE_FILE_TOOL,
            json!({ "path": "../note.txt", "content": "hello" }),
        );

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("escapes the workspace")
        );
    }

    #[test]
    fn runs_command_and_returns_nonzero_status() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": "git",
                "args": ["rev-parse", "--is-inside-work-tree"]
            }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["success"], false);
        assert_eq!(result.output["status"], 128);
        assert!(
            result.output["stderr"]
                .as_str()
                .expect("stderr")
                .contains("not a git repository")
        );
    }

    #[test]
    fn git_diff_reports_non_git_workspace() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(workspace.path(), GIT_DIFF_TOOL, json!({}));

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("workspace is not a git repository")
        );
    }

    #[test]
    fn git_diff_returns_workspace_diff() {
        let workspace = tempfile::tempdir().expect("workspace");
        run_test_command(workspace.path(), "git", &["init"]);
        run_test_command(
            workspace.path(),
            "git",
            &["config", "user.email", "foco@example.test"],
        );
        run_test_command(
            workspace.path(),
            "git",
            &["config", "user.name", "Foco Test"],
        );
        fs::write(workspace.path().join("note.txt"), "before\n").expect("write note");
        run_test_command(workspace.path(), "git", &["add", "note.txt"]);
        run_test_command(workspace.path(), "git", &["commit", "-m", "initial"]);
        fs::write(workspace.path().join("note.txt"), "after\n").expect("rewrite note");

        let result = execute_builtin_tool(workspace.path(), GIT_DIFF_TOOL, json!({}));

        assert!(!result.is_error);
        assert!(
            result.output["status"]
                .as_str()
                .expect("status")
                .contains("M note.txt")
        );
        assert!(
            result.output["diff"]
                .as_str()
                .expect("diff")
                .contains("-before")
        );
        assert!(
            result.output["diff"]
                .as_str()
                .expect("diff")
                .contains("+after")
        );
    }

    fn run_test_command(workspace_path: &Path, command: &str, args: &[&str]) {
        let output = Command::new(command)
            .args(args)
            .current_dir(workspace_path)
            .output()
            .expect("run test command");

        assert!(
            output.status.success(),
            "{} {} failed: {}",
            command,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn insert_graph_fixture(workspace_path: &Path) {
        let mut database = WorkspaceDatabase::open_or_create(workspace_path).expect("database");
        let lib_symbols = [
            NewCodeGraphSymbol {
                name: "public_api",
                kind: "function",
                start_line: Some(1),
                start_column: Some(1),
                end_line: Some(5),
                end_column: Some(1),
                signature: Some("fn public_api()"),
                documentation: None,
            },
            NewCodeGraphSymbol {
                name: "helper",
                kind: "function",
                start_line: Some(7),
                start_column: Some(1),
                end_line: Some(9),
                end_column: Some(1),
                signature: Some("fn helper()"),
                documentation: None,
            },
        ];
        let lib_imports = [NewCodeGraphImport {
            module: "crate::shared",
            imported_symbol: None,
            alias: None,
            start_line: Some(0),
            start_column: Some(0),
        }];
        let lib_references = [NewCodeGraphReference {
            name: "helper",
            symbol_index: Some(1),
            start_line: Some(3),
            start_column: Some(5),
            end_line: Some(3),
            end_column: Some(11),
        }];
        let lib_edges = [NewCodeGraphEdge {
            source_symbol_index: 0,
            target_symbol_index: 1,
            edge_kind: "references",
            metadata_json: None,
        }];
        database
            .replace_code_graph_file_index(NewCodeGraphFileIndex {
                path: "lib.rs",
                language: Some("rust"),
                size_bytes: Some(64),
                modified_at: Some("2026-06-04T00:00:00.000Z"),
                content_hash: "lib-hash",
                parse_status: "parsed",
                parse_error_message: None,
                symbols: &lib_symbols,
                imports: &lib_imports,
                references: &lib_references,
                edges: &lib_edges,
                fts_body: "fn public_api() { helper(); } fn helper() {}",
            })
            .expect("lib graph index");
        let caller_symbols = [NewCodeGraphSymbol {
            name: "caller_entry",
            kind: "function",
            start_line: Some(1),
            start_column: Some(1),
            end_line: Some(3),
            end_column: Some(1),
            signature: Some("fn caller_entry()"),
            documentation: None,
        }];
        let caller_imports = [NewCodeGraphImport {
            module: "crate::shared",
            imported_symbol: None,
            alias: None,
            start_line: Some(0),
            start_column: Some(0),
        }];
        database
            .replace_code_graph_file_index(NewCodeGraphFileIndex {
                path: "caller.rs",
                language: Some("rust"),
                size_bytes: Some(32),
                modified_at: Some("2026-06-04T00:00:00.000Z"),
                content_hash: "caller-hash",
                parse_status: "parsed",
                parse_error_message: None,
                symbols: &caller_symbols,
                imports: &caller_imports,
                references: &[],
                edges: &[],
                fts_body: "fn caller_entry() {}",
            })
            .expect("caller graph index");
    }
}
