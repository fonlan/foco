use std::{
    fmt, fs, io,
    io::Read,
    path::{Component, Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use foco_store::workspace::{
    CodeGraphReferenceRecord, CodeGraphRelatedFileRecord, CodeGraphSymbolRecord,
    CodeGraphSymbolRelationRecord, TodoGraphFilter, TodoGraphRecord, TodoGraphTask,
    TodoGraphTaskPatch, WorkspaceDatabase, WorkspaceDatabaseError,
};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const READ_FILE_TOOL: &str = "read_file";
pub const LIST_FILES_TOOL: &str = "list_files";
pub const SEARCH_TEXT_TOOL: &str = "search_text";
pub const WRITE_FILE_TOOL: &str = "write_file";
pub const PATCH_FILE_TOOL: &str = "patch_file";
pub const RUN_COMMAND_TOOL: &str = "run_command";
pub const SLEEP_TOOL: &str = "sleep";
pub const GRAPH_FIND_SYMBOLS_TOOL: &str = "graph_find_symbols";
pub const GRAPH_FIND_CALLERS_TOOL: &str = "graph_find_callers";
pub const GRAPH_FIND_CALLEES_TOOL: &str = "graph_find_callees";
pub const GRAPH_FIND_REFERENCES_TOOL: &str = "graph_find_references";
pub const GRAPH_RELATED_FILES_TOOL: &str = "graph_related_files";
pub const CREATE_TODO_GRAPH_TOOL: &str = "create_todo_graph";
pub const UPDATE_TODO_GRAPH_TOOL: &str = "update_todo_graph";
pub const GET_TODO_GRAPH_TOOL: &str = "get_todo_graph";
pub const ASK_QUESTION_TOOL: &str = "ask_question";

const MAX_FULL_READ_BYTES: u64 = 1024 * 1024;
const MAX_RANGED_READ_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_RANGED_READ_OUTPUT_BYTES: usize = 512 * 1024;
const MAX_LIST_ENTRIES: usize = 200;
const MAX_SEARCH_MATCHES: usize = 200;
const MAX_COMMAND_OUTPUT_BYTES: usize = 64 * 1024;
const DEFAULT_GRAPH_RESULT_LIMIT: usize = 20;
const MAX_GRAPH_RESULT_LIMIT: usize = 50;
const DEFAULT_FILE_TOOL_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_GRAPH_TOOL_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_SEARCH_TEXT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_WRITE_FILE_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_SLEEP_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_RUN_COMMAND_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_TODO_GRAPH_TIMEOUT_MS: u64 = 10_000;
const MAX_TOOL_TIMEOUT_MS: u64 = 300_000;
const COMMAND_WAIT_POLL_MS: u64 = 25;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
static RIPGREP_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

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

#[derive(Clone, Debug, Default)]
pub struct ToolCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl ToolCancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
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
        patch_file_definition(),
        create_todo_graph_definition(),
        update_todo_graph_definition(),
        get_todo_graph_definition(),
        ask_question_definition(),
        run_command_definition(),
        sleep_definition(),
    ]
}

pub fn set_ripgrep_path(path: Option<PathBuf>) {
    let state = RIPGREP_PATH.get_or_init(|| Mutex::new(None));
    let mut current = state.lock().expect("ripgrep path lock poisoned");
    *current = path;
}

pub fn execute_builtin_tool(
    workspace_path: &Path,
    tool_name: &str,
    arguments: Value,
) -> ToolExecution {
    execute_builtin_tool_for_chat(workspace_path, None, tool_name, arguments)
}

pub fn execute_builtin_tool_for_chat(
    workspace_path: &Path,
    chat_id: Option<&str>,
    tool_name: &str,
    arguments: Value,
) -> ToolExecution {
    execute_builtin_tool_for_chat_with_cancellation(
        workspace_path,
        chat_id,
        tool_name,
        arguments,
        None,
    )
}

pub fn execute_builtin_tool_for_chat_with_cancellation(
    workspace_path: &Path,
    chat_id: Option<&str>,
    tool_name: &str,
    arguments: Value,
    cancellation_token: Option<ToolCancellationToken>,
) -> ToolExecution {
    match execute_builtin_tool_inner(
        workspace_path,
        chat_id,
        tool_name,
        arguments,
        cancellation_token.as_ref(),
    ) {
        Ok(output) => ToolExecution {
            output,
            is_error: false,
        },
        Err(error) => ToolExecution {
            output: tool_error_output(&error),
            is_error: true,
        },
    }
}

fn tool_error_output(error: &ToolRuntimeError) -> Value {
    match error {
        ToolRuntimeError::Cancelled => json!({ "error": error.to_string(), "cancelled": true }),
        ToolRuntimeError::CommandCancelled { command, pid } => json!({
            "error": error.to_string(),
            "command": command,
            "pid": pid,
            "cancelled": true
        }),
        ToolRuntimeError::CommandTimedOut {
            command,
            pid,
            timeout_ms,
        } => json!({
            "error": error.to_string(),
            "command": command,
            "pid": pid,
            "timeoutMs": timeout_ms
        }),
        ToolRuntimeError::PatchRejected(rejection) => rejection.to_json(),
        _ => json!({ "error": error.to_string() }),
    }
}

pub fn builtin_tool_timeout_ms(tool_name: &str, arguments: &Value) -> Result<u64, String> {
    let default_timeout_ms = match tool_name {
        READ_FILE_TOOL | LIST_FILES_TOOL => DEFAULT_FILE_TOOL_TIMEOUT_MS,
        GRAPH_FIND_SYMBOLS_TOOL
        | GRAPH_FIND_CALLERS_TOOL
        | GRAPH_FIND_CALLEES_TOOL
        | GRAPH_FIND_REFERENCES_TOOL
        | GRAPH_RELATED_FILES_TOOL => DEFAULT_GRAPH_TOOL_TIMEOUT_MS,
        SEARCH_TEXT_TOOL => DEFAULT_SEARCH_TEXT_TIMEOUT_MS,
        WRITE_FILE_TOOL | PATCH_FILE_TOOL => DEFAULT_WRITE_FILE_TIMEOUT_MS,
        CREATE_TODO_GRAPH_TOOL | UPDATE_TODO_GRAPH_TOOL | GET_TODO_GRAPH_TOOL => {
            DEFAULT_TODO_GRAPH_TIMEOUT_MS
        }
        ASK_QUESTION_TOOL => {
            return Err(
                "tool 'ask_question' waits for user input and does not use timeoutMs".to_string(),
            );
        }
        RUN_COMMAND_TOOL => DEFAULT_RUN_COMMAND_TIMEOUT_MS,
        SLEEP_TOOL => DEFAULT_SLEEP_TIMEOUT_MS,
        other => return Err(ToolRuntimeError::UnknownTool(other.to_string()).to_string()),
    };

    argument_timeout_ms(arguments, default_timeout_ms).map_err(|error| error.to_string())
}

fn execute_builtin_tool_inner(
    workspace_path: &Path,
    chat_id: Option<&str>,
    tool_name: &str,
    arguments: Value,
    cancellation_token: Option<&ToolCancellationToken>,
) -> Result<Value, ToolRuntimeError> {
    match tool_name {
        READ_FILE_TOOL => read_file(workspace_path, arguments),
        LIST_FILES_TOOL => list_files(workspace_path, arguments),
        GRAPH_FIND_SYMBOLS_TOOL => graph_find_symbols(workspace_path, arguments),
        GRAPH_FIND_CALLERS_TOOL => graph_find_callers(workspace_path, arguments),
        GRAPH_FIND_CALLEES_TOOL => graph_find_callees(workspace_path, arguments),
        GRAPH_FIND_REFERENCES_TOOL => graph_find_references(workspace_path, arguments),
        GRAPH_RELATED_FILES_TOOL => graph_related_files(workspace_path, arguments),
        SEARCH_TEXT_TOOL => search_text(workspace_path, arguments, cancellation_token),
        WRITE_FILE_TOOL => write_file(workspace_path, arguments),
        PATCH_FILE_TOOL => patch_file(workspace_path, arguments),
        CREATE_TODO_GRAPH_TOOL => create_todo_graph(workspace_path, chat_id, arguments),
        UPDATE_TODO_GRAPH_TOOL => update_todo_graph(workspace_path, chat_id, arguments),
        GET_TODO_GRAPH_TOOL => get_todo_graph(workspace_path, chat_id, arguments),
        ASK_QUESTION_TOOL => Err(ToolRuntimeError::InvalidArguments(
            "ask_question must be executed through the chat UI question bridge".to_string(),
        )),
        RUN_COMMAND_TOOL => run_command(workspace_path, arguments, cancellation_token),
        SLEEP_TOOL => sleep_tool(arguments, cancellation_token),
        other => Err(ToolRuntimeError::UnknownTool(other.to_string())),
    }
}

fn read_file(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
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

fn list_files(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: ListFilesInput = parse_arguments(arguments)?;
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
            let relative_path = relative_workspace_path(workspace_path, &entry_path)?;
            if !filter.matches(&relative_path) {
                return Ok(None);
            }

            Ok(Some(json!({
                "path": relative_path,
                "kind": kind,
                "bytes": if metadata.is_file() { Some(metadata.len()) } else { None }
            })))
        })
        .collect::<Result<Vec<_>, ToolRuntimeError>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| {
        left.get("path")
            .and_then(Value::as_str)
            .cmp(&right.get("path").and_then(Value::as_str))
    });
    let truncated = entries.len() > MAX_LIST_ENTRIES;
    entries.truncate(MAX_LIST_ENTRIES);

    Ok(json!({
        "path": input_path,
        "include": filter.include_patterns(),
        "exclude": filter.exclude_patterns(),
        "entries": entries,
        "truncated": truncated,
        "timeoutMs": timeout_ms
    }))
}

fn graph_find_symbols(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: GraphFindSymbolsInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
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
        "truncated": truncated,
        "timeoutMs": timeout_ms
    }))
}

fn graph_find_callers(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: GraphSymbolLookupInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let database = open_code_graph_database(workspace_path)?;
    let symbol = resolve_graph_symbol(&database, &request)?;
    let limit = graph_limit(request.limit)?;
    let mut callers = database.code_graph_callers(symbol.id, graph_query_limit(limit)?)?;
    let truncated = truncate_records(&mut callers, limit);

    Ok(json!({
        "symbol": symbol_json(symbol),
        "callers": callers.into_iter().map(relation_json).collect::<Vec<_>>(),
        "truncated": truncated,
        "timeoutMs": timeout_ms
    }))
}

fn graph_find_callees(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: GraphSymbolLookupInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let database = open_code_graph_database(workspace_path)?;
    let symbol = resolve_graph_symbol(&database, &request)?;
    let limit = graph_limit(request.limit)?;
    let mut callees = database.code_graph_callees(symbol.id, graph_query_limit(limit)?)?;
    let truncated = truncate_records(&mut callees, limit);

    Ok(json!({
        "symbol": symbol_json(symbol),
        "callees": callees.into_iter().map(relation_json).collect::<Vec<_>>(),
        "truncated": truncated,
        "timeoutMs": timeout_ms
    }))
}

fn graph_find_references(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GraphSymbolLookupInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let database = open_code_graph_database(workspace_path)?;
    let symbol = resolve_graph_symbol(&database, &request)?;
    let limit = graph_limit(request.limit)?;
    let mut references = database.code_graph_references(symbol.id, graph_query_limit(limit)?)?;
    let truncated = truncate_records(&mut references, limit);

    Ok(json!({
        "symbol": symbol_json(symbol),
        "references": references.into_iter().map(reference_json).collect::<Vec<_>>(),
        "truncated": truncated,
        "timeoutMs": timeout_ms
    }))
}

fn graph_related_files(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: GraphRelatedFilesInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let path = normalize_workspace_path_text(&request.path)?;
    let limit = graph_limit(request.limit)?;
    let database = open_code_graph_database(workspace_path)?;
    let mut files = database.code_graph_related_files(&path, graph_query_limit(limit)?)?;
    let truncated = truncate_records(&mut files, limit);

    Ok(json!({
        "path": path,
        "files": files.into_iter().map(related_file_json).collect::<Vec<_>>(),
        "truncated": truncated,
        "timeoutMs": timeout_ms
    }))
}

fn search_text(
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
        MAX_SEARCH_MATCHES.to_string(),
        pattern.to_string(),
        path.to_string_lossy().to_string(),
    ];
    let rg_command = ripgrep_command();
    let output = run_command_with_timeout(
        &rg_command,
        &rg_args,
        workspace_path,
        Duration::from_millis(timeout_ms),
        cancellation_token,
    )?;

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
        "truncated": truncated,
        "timeoutMs": timeout_ms
    }))
}

fn ripgrep_command() -> String {
    RIPGREP_PATH
        .get()
        .and_then(|state| state.lock().ok().and_then(|path| path.clone()))
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "rg".to_string())
}

fn write_file(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
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

    let (content, encoding) = match fs::metadata(&path) {
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

            (content, encoding)
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            if line_range.is_some() {
                return Err(ToolRuntimeError::InvalidArguments(
                    "line-range writes require an existing file".to_string(),
                ));
            }

            (request.content, TextEncoding::Utf8)
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
        "timeoutMs": timeout_ms
    }))
}

fn patch_file(workspace_path: &Path, arguments: Value) -> Result<Value, ToolRuntimeError> {
    let request: PatchFileInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_WRITE_FILE_TIMEOUT_MS)?;
    let path = resolve_workspace_file(workspace_path, &request.path)?;
    let bytes = fs::read(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;
    let (existing_content, encoding) = decode_text_file(&path, &bytes)?;
    let (content, applied_hunks) = apply_file_diff(&existing_content, &request.diff)
        .map_err(ToolRuntimeError::PatchRejected)?;
    let encoded = encode_text_file(&content, encoding);

    fs::write(&path, &encoded).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;

    Ok(json!({
        "path": normalize_workspace_path_text(&request.path)?,
        "bytes": encoded.len(),
        "appliedHunks": applied_hunks,
        "timeoutMs": timeout_ms
    }))
}

fn create_todo_graph(
    workspace_path: &Path,
    chat_id: Option<&str>,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: CreateTodoGraphInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_TODO_GRAPH_TIMEOUT_MS)?;
    let chat_id = required_chat_id(chat_id)?;
    let mut database = open_todo_graph_database(workspace_path)?;
    let graph = database.upsert_todo_graph(
        chat_id,
        request
            .tasks
            .into_iter()
            .map(todo_graph_task_from_input)
            .collect(),
    )?;

    Ok(todo_graph_json(graph, timeout_ms))
}

fn update_todo_graph(
    workspace_path: &Path,
    chat_id: Option<&str>,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: UpdateTodoGraphInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_TODO_GRAPH_TIMEOUT_MS)?;
    let chat_id = required_chat_id(chat_id)?;
    let patch = TodoGraphTaskPatch {
        title: request.patch.title,
        status: request.patch.status,
        depends_on: request.patch.depends_on,
        acceptance: request.patch.acceptance,
        summary: request.patch.summary,
        subtasks: request
            .patch
            .subtasks
            .map(|tasks| tasks.into_iter().map(todo_graph_task_from_input).collect()),
    };
    let mut database = open_todo_graph_database(workspace_path)?;
    let graph = database.update_todo_graph_task(chat_id, &request.task_id, patch)?;

    Ok(todo_graph_json(graph, timeout_ms))
}

fn get_todo_graph(
    workspace_path: &Path,
    chat_id: Option<&str>,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GetTodoGraphInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_TODO_GRAPH_TIMEOUT_MS)?;
    let chat_id = required_chat_id(chat_id)?;
    let status = request
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let task_id = request
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let database = open_todo_graph_database(workspace_path)?;
    let graph = database.filtered_todo_graph(
        chat_id,
        TodoGraphFilter {
            status,
            task_id,
            include_subtasks: request.include_subtasks,
        },
    )?;

    match graph {
        Some(graph) => Ok(todo_graph_json(graph, timeout_ms)),
        None => Ok(json!({
            "chatId": chat_id,
            "tasks": [],
            "exists": false,
            "createdAt": null,
            "updatedAt": null,
            "updatedTask": null,
            "timeoutMs": timeout_ms
        })),
    }
}

fn run_command(
    workspace_path: &Path,
    arguments: Value,
    cancellation_token: Option<&ToolCancellationToken>,
) -> Result<Value, ToolRuntimeError> {
    let request: RunCommandInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_RUN_COMMAND_TIMEOUT_MS)?;
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

    let output = run_command_with_timeout(
        command,
        &args,
        &cwd,
        Duration::from_millis(timeout_ms),
        cancellation_token,
    )?;
    let (stdout, stdout_truncated) = limited_output_text(&output.stdout);
    let (stderr, stderr_truncated) = limited_output_text(&output.stderr);

    Ok(json!({
        "command": command,
        "args": args,
        "cwd": relative_workspace_path(workspace_path, &cwd)?,
        "pid": output.pid,
        "status": output.status.code(),
        "success": output.status.success(),
        "stdout": stdout,
        "stderr": stderr,
        "stdoutTruncated": stdout_truncated,
        "stderrTruncated": stderr_truncated,
        "timeoutMs": timeout_ms
    }))
}

fn sleep_tool(
    arguments: Value,
    cancellation_token: Option<&ToolCancellationToken>,
) -> Result<Value, ToolRuntimeError> {
    let request: SleepInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_SLEEP_TIMEOUT_MS)?;

    if request.duration_ms == 0 || request.duration_ms > timeout_ms {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "durationMs must be between 1 and timeoutMs ({timeout_ms}) milliseconds"
        )));
    }

    let started = Instant::now();
    let duration = Duration::from_millis(request.duration_ms);
    loop {
        if cancellation_token
            .map(ToolCancellationToken::is_cancelled)
            .unwrap_or(false)
        {
            return Err(ToolRuntimeError::Cancelled);
        }

        let elapsed = started.elapsed();
        if elapsed >= duration {
            break;
        }

        thread::sleep(
            duration
                .saturating_sub(elapsed)
                .min(Duration::from_millis(COMMAND_WAIT_POLL_MS)),
        );
    }

    Ok(json!({
        "durationMs": request.duration_ms,
        "timeoutMs": timeout_ms
    }))
}

fn open_code_graph_database(workspace_path: &Path) -> Result<WorkspaceDatabase, ToolRuntimeError> {
    WorkspaceDatabase::open_or_create(workspace_path).map_err(ToolRuntimeError::WorkspaceDatabase)
}

fn open_todo_graph_database(workspace_path: &Path) -> Result<WorkspaceDatabase, ToolRuntimeError> {
    WorkspaceDatabase::open_or_create(workspace_path).map_err(ToolRuntimeError::WorkspaceDatabase)
}

fn required_chat_id(chat_id: Option<&str>) -> Result<&str, ToolRuntimeError> {
    chat_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ToolRuntimeError::InvalidArguments(
                "todo graph tools require an active chat".to_string(),
            )
        })
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

fn todo_graph_json(graph: TodoGraphRecord, timeout_ms: u64) -> Value {
    json!({
        "chatId": graph.chat_id,
        "tasks": graph.tasks,
        "exists": true,
        "createdAt": graph.created_at,
        "updatedAt": graph.updated_at,
        "updatedTask": graph.updated_task,
        "timeoutMs": timeout_ms
    })
}

fn todo_graph_task_from_input(task: TodoGraphTaskInput) -> TodoGraphTask {
    let _server_generated_timestamps = (task.created_at, task.updated_at);

    TodoGraphTask {
        id: task.id,
        title: task.title,
        status: task.status,
        depends_on: task.depends_on,
        acceptance: task.acceptance,
        summary: task.summary,
        created_at: String::new(),
        updated_at: String::new(),
        subtasks: task
            .subtasks
            .into_iter()
            .map(todo_graph_task_from_input)
            .collect(),
    }
}

fn parse_arguments<T>(arguments: Value) -> Result<T, ToolRuntimeError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(arguments).map_err(|source| {
        ToolRuntimeError::InvalidArguments(format!("tool arguments do not match schema: {source}"))
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TextEncoding {
    Utf8,
    Utf8Bom,
    Utf16LeBom,
    Utf16BeBom,
}

fn decode_text_file(path: &Path, bytes: &[u8]) -> Result<(String, TextEncoding), ToolRuntimeError> {
    if let Some(content) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        let content = std::str::from_utf8(content)
            .map_err(|_| ToolRuntimeError::UnsupportedEncoding(path.to_path_buf()))?;
        return Ok((content.to_string(), TextEncoding::Utf8Bom));
    }

    if let Some(content) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16_file(path, content, TextEncoding::Utf16LeBom);
    }

    if let Some(content) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16_file(path, content, TextEncoding::Utf16BeBom);
    }

    let content = std::str::from_utf8(bytes)
        .map_err(|_| ToolRuntimeError::UnsupportedEncoding(path.to_path_buf()))?;
    Ok((content.to_string(), TextEncoding::Utf8))
}

fn decode_utf16_file(
    path: &Path,
    bytes: &[u8],
    encoding: TextEncoding,
) -> Result<(String, TextEncoding), ToolRuntimeError> {
    if bytes.len() % 2 != 0 {
        return Err(ToolRuntimeError::UnsupportedEncoding(path.to_path_buf()));
    }

    let units = bytes
        .chunks_exact(2)
        .map(|chunk| match encoding {
            TextEncoding::Utf16LeBom => u16::from_le_bytes([chunk[0], chunk[1]]),
            TextEncoding::Utf16BeBom => u16::from_be_bytes([chunk[0], chunk[1]]),
            TextEncoding::Utf8 | TextEncoding::Utf8Bom => unreachable!("utf16 decoder encoding"),
        })
        .collect::<Vec<_>>();
    let content = String::from_utf16(&units)
        .map_err(|_| ToolRuntimeError::UnsupportedEncoding(path.to_path_buf()))?;

    Ok((content, encoding))
}

fn encode_text_file(content: &str, encoding: TextEncoding) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => content.as_bytes().to_vec(),
        TextEncoding::Utf8Bom => {
            let mut bytes = vec![0xEF, 0xBB, 0xBF];
            bytes.extend_from_slice(content.as_bytes());
            bytes
        }
        TextEncoding::Utf16LeBom => {
            let mut bytes = vec![0xFF, 0xFE];
            for unit in content.encode_utf16() {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            bytes
        }
        TextEncoding::Utf16BeBom => {
            let mut bytes = vec![0xFE, 0xFF];
            for unit in content.encode_utf16() {
                bytes.extend_from_slice(&unit.to_be_bytes());
            }
            bytes
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LineRange {
    start: usize,
    end: usize,
}

impl LineRange {
    fn new(start: usize, end: usize) -> Result<Self, ToolRuntimeError> {
        if start == 0 || end == 0 || end < start {
            return Err(ToolRuntimeError::InvalidArguments(
                "line ranges are 1-based inclusive ranges and must satisfy startLine <= endLine"
                    .to_string(),
            ));
        }

        Ok(Self { start, end })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LineSpan {
    start_byte: usize,
    end_byte: usize,
    line_ending: Option<&'static str>,
}

fn parse_optional_line_range(
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<Option<LineRange>, ToolRuntimeError> {
    match (start_line, end_line) {
        (None, None) => Ok(None),
        (Some(start), Some(end)) => LineRange::new(start, end).map(Some),
        _ => {
            Err(ToolRuntimeError::InvalidArguments(
                "startLine and endLine must both be null for full-file reads or both be integers for line-range reads".to_string(),
            ))
        }
    }
}

fn validate_line_range(range: LineRange, line_count: usize) -> Result<(), ToolRuntimeError> {
    if range.end > line_count {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "line range {}-{} is outside the file; file has {line_count} lines",
            range.start, range.end
        )));
    }

    Ok(())
}

fn normalize_read_line_range(
    range: LineRange,
    line_count: usize,
) -> Result<LineRange, ToolRuntimeError> {
    if range.start > line_count {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "line range {}-{} is outside the file; file has {line_count} lines",
            range.start, range.end
        )));
    }

    Ok(LineRange {
        end: range.end.min(line_count),
        ..range
    })
}

fn count_text_lines(content: &str) -> usize {
    line_spans(content).len()
}

fn read_line_range(content: &str, range: &LineRange) -> String {
    let spans = line_spans(content);
    let start = spans[range.start - 1].start_byte;
    let end = spans[range.end - 1].end_byte;
    content[start..end].to_string()
}

fn numbered_content(content: &str, start_line: usize) -> String {
    let mut numbered = String::new();
    for (index, span) in line_spans(content).into_iter().enumerate() {
        numbered.push_str(&(start_line + index).to_string());
        numbered.push('\t');
        numbered.push_str(&content[span.start_byte..span.end_byte]);
    }

    numbered
}

fn replace_line_range(
    existing_content: &str,
    range: LineRange,
    replacement: &str,
) -> Result<String, ToolRuntimeError> {
    let spans = line_spans(existing_content);
    validate_line_range(range, spans.len())?;

    let start = spans[range.start - 1].start_byte;
    let replaced_end = spans[range.end - 1].end_byte;
    let mut replacement = replacement.to_string();

    if let Some(line_ending) = spans[range.end - 1].line_ending
        && !ends_with_line_ending(&replacement)
    {
        replacement.push_str(line_ending);
    }

    let mut content =
        String::with_capacity(existing_content.len() - (replaced_end - start) + replacement.len());
    content.push_str(&existing_content[..start]);
    content.push_str(&replacement);
    content.push_str(&existing_content[replaced_end..]);

    Ok(content)
}

fn line_spans(content: &str) -> Vec<LineSpan> {
    let bytes = content.as_bytes();
    let mut spans = Vec::new();
    let mut start = 0;
    let mut index = 0;

    while index < bytes.len() {
        let (end, line_ending) = match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => (index + 2, Some("\r\n")),
            b'\r' => (index + 1, Some("\r")),
            b'\n' => (index + 1, Some("\n")),
            _ => {
                index += 1;
                continue;
            }
        };

        spans.push(LineSpan {
            start_byte: start,
            end_byte: end,
            line_ending,
        });
        start = end;
        index = end;
    }

    if start < bytes.len() {
        spans.push(LineSpan {
            start_byte: start,
            end_byte: bytes.len(),
            line_ending: None,
        });
    }

    spans
}

fn ends_with_line_ending(content: &str) -> bool {
    content.ends_with('\n') || content.ends_with('\r')
}

#[derive(Clone, Debug, PartialEq)]
struct FileLine {
    body: String,
    line_ending: Option<String>,
}

#[derive(Debug, PartialEq)]
struct DiffHunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<DiffLine>,
}

#[derive(Debug, PartialEq)]
enum DiffLine {
    Context(String),
    Remove(String),
    Add { body: String, no_newline: bool },
}

impl DiffHunk {
    fn actual_line_counts(&self) -> (usize, usize) {
        let old_lines = self
            .lines
            .iter()
            .filter(|line| matches!(line, DiffLine::Context(_) | DiffLine::Remove(_)))
            .count();
        let new_lines = self
            .lines
            .iter()
            .filter(|line| matches!(line, DiffLine::Context(_) | DiffLine::Add { .. }))
            .count();

        (old_lines, new_lines)
    }

    fn suggested_read_range(&self) -> LineRange {
        suggested_read_range_around(self.old_start.max(1), self.old_count.max(1))
    }
}

#[derive(Debug)]
struct PatchRejection {
    message: String,
    hunk: Option<PatchHunkDiagnostic>,
    first_mismatch: Option<PatchMismatchDiagnostic>,
    suggested_read_range: Option<LineRange>,
}

#[derive(Debug)]
struct PatchHunkDiagnostic {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    actual_old_count: usize,
    actual_new_count: usize,
}

#[derive(Debug)]
struct PatchMismatchDiagnostic {
    kind: String,
    line: usize,
    expected: String,
    actual: String,
}

impl PatchRejection {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hunk: None,
            first_mismatch: None,
            suggested_read_range: None,
        }
    }

    fn with_hunk_summary(mut self, hunk: &DiffHunk) -> Self {
        let (actual_old_count, actual_new_count) = hunk.actual_line_counts();
        self.hunk = Some(PatchHunkDiagnostic {
            old_start: hunk.old_start,
            old_count: hunk.old_count,
            new_start: hunk.new_start,
            new_count: hunk.new_count,
            actual_old_count,
            actual_new_count,
        });
        self
    }

    fn with_actual_counts(mut self, actual_old_count: usize, actual_new_count: usize) -> Self {
        if let Some(hunk) = self.hunk.as_mut() {
            hunk.actual_old_count = actual_old_count;
            hunk.actual_new_count = actual_new_count;
        }
        self
    }

    fn with_first_mismatch(
        mut self,
        kind: &str,
        line: usize,
        expected: &str,
        actual: &str,
    ) -> Self {
        self.first_mismatch = Some(PatchMismatchDiagnostic {
            kind: kind.to_string(),
            line,
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
        self
    }

    fn with_suggested_read_range(mut self, range: LineRange) -> Self {
        self.suggested_read_range = Some(range);
        self
    }

    fn to_json(&self) -> Value {
        let mut output = json!({
            "error": self.message,
            "suggestion": "Run read_file for suggestedReadRange before retrying patch_file; do not retry by only changing hunk counts or headers."
        });

        if let Some(hunk) = &self.hunk {
            output["hunk"] = json!({
                "declaredOldStart": hunk.old_start,
                "declaredOldCount": hunk.old_count,
                "declaredNewStart": hunk.new_start,
                "declaredNewCount": hunk.new_count,
                "actualOldCount": hunk.actual_old_count,
                "actualNewCount": hunk.actual_new_count
            });
        }

        if let Some(mismatch) = &self.first_mismatch {
            output["firstMismatch"] = json!({
                "kind": mismatch.kind,
                "line": mismatch.line,
                "expected": mismatch.expected,
                "actual": mismatch.actual
            });
        }

        if let Some(range) = self.suggested_read_range {
            output["suggestedReadRange"] = json!({
                "startLine": range.start,
                "endLine": range.end
            });
        }

        output
    }
}

impl fmt::Display for PatchRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

fn suggested_read_range_around(line: usize, old_count: usize) -> LineRange {
    let start = line.saturating_sub(3).max(1);
    let end = line + old_count.max(1) + 3;
    LineRange { start, end }
}

fn apply_file_diff(content: &str, diff: &str) -> Result<(String, usize), PatchRejection> {
    let hunks = parse_file_diff(diff)?;
    let mut lines = split_file_lines(content);
    let default_line_ending = default_line_ending(&lines);
    let mut line_delta = 0isize;

    for hunk in &hunks {
        let original_index = if hunk.old_count == 0 {
            hunk.old_start
        } else {
            hunk.old_start.checked_sub(1).ok_or_else(|| {
                PatchRejection::new("diff hunk old start must be 1-based")
                    .with_hunk_summary(hunk)
                    .with_suggested_read_range(hunk.suggested_read_range())
            })?
        };
        let patched_index = original_index as isize + line_delta;
        if patched_index < 0 {
            return Err(PatchRejection::new(format!(
                "diff hunk starting at original line {} resolves before the file start",
                hunk.old_start
            ))
            .with_hunk_summary(hunk)
            .with_suggested_read_range(hunk.suggested_read_range()));
        }
        let start_index = usize::try_from(patched_index).map_err(|_| {
            PatchRejection::new("diff hunk line index is too large")
                .with_hunk_summary(hunk)
                .with_suggested_read_range(hunk.suggested_read_range())
        })?;
        if start_index > lines.len() {
            return Err(PatchRejection::new(format!(
                "diff hunk starting at original line {} is outside the file; file has {} lines",
                hunk.old_start,
                lines.len()
            ))
            .with_hunk_summary(hunk)
            .with_suggested_read_range(hunk.suggested_read_range()));
        }

        let mut index = start_index;
        let mut replacement = Vec::new();
        for diff_line in &hunk.lines {
            match diff_line {
                DiffLine::Context(body) => {
                    let existing = lines.get(index).ok_or_else(|| {
                        PatchRejection::new(format!(
                            "diff context at original line {} extends past the file end",
                            hunk.old_start
                        ))
                        .with_hunk_summary(hunk)
                        .with_suggested_read_range(hunk.suggested_read_range())
                    })?;
                    validate_patch_line("context", body, existing, index + 1, hunk)?;
                    replacement.push(existing.clone());
                    index += 1;
                }
                DiffLine::Remove(body) => {
                    let existing = lines.get(index).ok_or_else(|| {
                        PatchRejection::new(format!(
                            "diff removal at original line {} extends past the file end",
                            hunk.old_start
                        ))
                        .with_hunk_summary(hunk)
                        .with_suggested_read_range(hunk.suggested_read_range())
                    })?;
                    validate_patch_line("removal", body, existing, index + 1, hunk)?;
                    index += 1;
                }
                DiffLine::Add { body, no_newline } => {
                    replacement.push(FileLine {
                        body: body.clone(),
                        line_ending: if *no_newline {
                            None
                        } else {
                            Some(default_line_ending.clone())
                        },
                    });
                }
            }
        }

        let consumed = index - start_index;
        lines.splice(start_index..index, replacement.iter().cloned());
        line_delta += replacement.len() as isize - consumed as isize;
    }

    Ok((join_file_lines(&lines), hunks.len()))
}

fn parse_file_diff(diff: &str) -> Result<Vec<DiffHunk>, PatchRejection> {
    if diff.trim().is_empty() {
        return Err(PatchRejection::new("diff must not be empty"));
    }

    let mut hunks = Vec::new();
    let mut current_hunk: Option<DiffHunk> = None;

    for line in diff.lines() {
        if line.starts_with("@@ ") {
            if let Some(hunk) = current_hunk.take() {
                validate_diff_hunk(&hunk)?;
                hunks.push(hunk);
            }
            current_hunk = Some(parse_hunk_header(line)?);
            continue;
        }

        let Some(hunk) = current_hunk.as_mut() else {
            continue;
        };

        if line == r"\ No newline at end of file" {
            mark_previous_added_line_no_newline(hunk)?;
            continue;
        }

        if line.is_empty() {
            return Err(PatchRejection::new(
                "diff hunk lines must start with space, -, +, or a no-newline marker",
            )
            .with_hunk_summary(hunk)
            .with_suggested_read_range(hunk.suggested_read_range()));
        }
        let (prefix, body) = line.split_at(1);

        match prefix {
            " " => hunk.lines.push(DiffLine::Context(body.to_string())),
            "-" => hunk.lines.push(DiffLine::Remove(body.to_string())),
            "+" => hunk.lines.push(DiffLine::Add {
                body: body.to_string(),
                no_newline: false,
            }),
            _ => {
                return Err(PatchRejection::new(format!(
                    "invalid diff hunk line prefix: {prefix}"
                ))
                .with_hunk_summary(hunk)
                .with_suggested_read_range(hunk.suggested_read_range()));
            }
        }
    }

    if let Some(hunk) = current_hunk {
        validate_diff_hunk(&hunk)?;
        hunks.push(hunk);
    }

    if hunks.is_empty() {
        return Err(PatchRejection::new(
            "diff must contain at least one unified diff hunk",
        ));
    }

    Ok(hunks)
}

fn parse_hunk_header(line: &str) -> Result<DiffHunk, PatchRejection> {
    let header = line
        .strip_prefix("@@ ")
        .and_then(|value| value.split_once(" @@").map(|(header, _)| header))
        .ok_or_else(|| PatchRejection::new(format!("invalid unified diff hunk header: {line}")))?;
    let mut parts = header.split_whitespace();
    let old_range = parts
        .next()
        .ok_or_else(|| PatchRejection::new(format!("missing old range in hunk header: {line}")))?;
    let new_range = parts
        .next()
        .ok_or_else(|| PatchRejection::new(format!("missing new range in hunk header: {line}")))?;

    if parts.next().is_some() {
        return Err(PatchRejection::new(format!(
            "invalid unified diff hunk header: {line}"
        )));
    }

    let (old_start, old_count) = parse_hunk_range(old_range, '-')?;
    let (new_start, new_count) = parse_hunk_range(new_range, '+')?;

    Ok(DiffHunk {
        old_start,
        old_count,
        new_start,
        new_count,
        lines: Vec::new(),
    })
}

fn parse_hunk_range(range: &str, prefix: char) -> Result<(usize, usize), PatchRejection> {
    let value = range.strip_prefix(prefix).ok_or_else(|| {
        PatchRejection::new(format!("diff hunk range must start with {prefix}: {range}"))
    })?;
    let (start, count) = match value.split_once(',') {
        Some((start, count)) => (
            start,
            count.parse::<usize>().map_err(|_| {
                PatchRejection::new(format!("invalid diff hunk line count: {range}"))
            })?,
        ),
        None => (value, 1),
    };
    let start = start
        .parse::<usize>()
        .map_err(|_| PatchRejection::new(format!("invalid diff hunk line start: {range}")))?;

    if start == 0 && count != 0 {
        return Err(PatchRejection::new(format!(
            "diff hunk range start may be 0 only when count is 0: {range}"
        )));
    }

    Ok((start, count))
}

fn validate_diff_hunk(hunk: &DiffHunk) -> Result<(), PatchRejection> {
    let (old_lines, new_lines) = hunk.actual_line_counts();

    if old_lines != hunk.old_count || new_lines != hunk.new_count {
        return Err(PatchRejection::new(format!(
            "diff hunk line counts do not match header -{},{} +{},{}",
            hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
        ))
        .with_hunk_summary(hunk)
        .with_actual_counts(old_lines, new_lines)
        .with_suggested_read_range(hunk.suggested_read_range()));
    }

    Ok(())
}

fn mark_previous_added_line_no_newline(hunk: &mut DiffHunk) -> Result<(), PatchRejection> {
    match hunk.lines.last_mut() {
        Some(DiffLine::Add { no_newline, .. }) => {
            *no_newline = true;
            Ok(())
        }
        Some(DiffLine::Context(_) | DiffLine::Remove(_)) => Ok(()),
        None => Err(PatchRejection::new(
            "no-newline marker must follow a diff line",
        )),
    }
}

fn validate_patch_line(
    line_kind: &str,
    expected_body: &str,
    existing: &FileLine,
    line_number: usize,
    hunk: &DiffHunk,
) -> Result<(), PatchRejection> {
    if existing.body == expected_body {
        Ok(())
    } else {
        Err(PatchRejection::new(format!(
            "diff {line_kind} line does not match file line {line_number}"
        ))
        .with_hunk_summary(hunk)
        .with_first_mismatch(line_kind, line_number, expected_body, &existing.body)
        .with_suggested_read_range(suggested_read_range_around(line_number, hunk.old_count)))
    }
}

fn split_file_lines(content: &str) -> Vec<FileLine> {
    line_spans(content)
        .into_iter()
        .map(|span| {
            let raw_line = &content[span.start_byte..span.end_byte];
            let body_end = span
                .line_ending
                .map(|line_ending| raw_line.len() - line_ending.len())
                .unwrap_or(raw_line.len());

            FileLine {
                body: raw_line[..body_end].to_string(),
                line_ending: span.line_ending.map(str::to_string),
            }
        })
        .collect()
}

fn default_line_ending(lines: &[FileLine]) -> String {
    lines
        .iter()
        .find_map(|line| line.line_ending.clone())
        .unwrap_or_else(|| "\n".to_string())
}

fn join_file_lines(lines: &[FileLine]) -> String {
    let mut content = String::new();
    for line in lines {
        content.push_str(&line.body);
        if let Some(line_ending) = &line.line_ending {
            content.push_str(line_ending);
        }
    }
    content
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

fn tool_timeout_ms(
    timeout_ms: Option<u64>,
    default_timeout_ms: u64,
) -> Result<u64, ToolRuntimeError> {
    let timeout_ms = timeout_ms.unwrap_or(default_timeout_ms);

    if timeout_ms == 0 || timeout_ms > MAX_TOOL_TIMEOUT_MS {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "timeoutMs must be between 1 and {MAX_TOOL_TIMEOUT_MS} milliseconds"
        )));
    }

    Ok(timeout_ms)
}

fn argument_timeout_ms(
    arguments: &Value,
    default_timeout_ms: u64,
) -> Result<u64, ToolRuntimeError> {
    match arguments.get("timeoutMs") {
        Some(Value::Null) | None => tool_timeout_ms(None, default_timeout_ms),
        Some(Value::Number(timeout_ms)) => {
            let timeout_ms = timeout_ms.as_u64().ok_or_else(|| {
                ToolRuntimeError::InvalidArguments(
                    "timeoutMs must be an integer or null".to_string(),
                )
            })?;

            tool_timeout_ms(Some(timeout_ms), default_timeout_ms)
        }
        Some(_) => Err(ToolRuntimeError::InvalidArguments(
            "timeoutMs must be an integer or null".to_string(),
        )),
    }
}

fn run_command_with_timeout(
    command: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    cancellation_token: Option<&ToolCancellationToken>,
) -> Result<CommandRunOutput, ToolRuntimeError> {
    let command_label = command_label(command, args);
    let mut command_process = Command::new(command);
    command_process
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command_process.creation_flags(CREATE_NO_WINDOW);

    let mut child = command_process
        .spawn()
        .map_err(|source| ToolRuntimeError::Command {
            command: command_label.clone(),
            source,
        })?;
    let pid = child.id();
    let stdout = child.stdout.take().ok_or_else(|| {
        ToolRuntimeError::InvalidArguments("failed to capture stdout".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        ToolRuntimeError::InvalidArguments("failed to capture stderr".to_string())
    })?;
    let stdout_handle = read_command_pipe(stdout);
    let stderr_handle = read_command_pipe(stderr);
    let started = Instant::now();
    let deadline = started + timeout;
    let timeout_ms = timeout.as_millis().min(u128::from(u64::MAX)) as u64;

    loop {
        if cancellation_token
            .map(ToolCancellationToken::is_cancelled)
            .unwrap_or(false)
        {
            let _ = child.kill();
            let _ = child.wait();

            return Err(ToolRuntimeError::CommandCancelled {
                command: command_label,
                pid,
            });
        }

        if let Some(status) = child
            .try_wait()
            .map_err(|source| ToolRuntimeError::Command {
                command: command_label.clone(),
                source,
            })?
        {
            let stdout = receive_command_pipe(
                &command_label,
                stdout_handle,
                deadline,
                timeout_ms,
                pid,
                cancellation_token,
            )?;
            let stderr = receive_command_pipe(
                &command_label,
                stderr_handle,
                deadline,
                timeout_ms,
                pid,
                cancellation_token,
            )?;
            return Ok(CommandRunOutput {
                pid,
                status,
                stdout,
                stderr,
            });
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();

            return Err(ToolRuntimeError::CommandTimedOut {
                command: command_label,
                pid,
                timeout_ms,
            });
        }

        thread::sleep(
            remaining_until(deadline)
                .unwrap_or(Duration::ZERO)
                .min(Duration::from_millis(COMMAND_WAIT_POLL_MS)),
        );
    }
}

struct CommandRunOutput {
    pid: u32,
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn read_command_pipe<T>(mut pipe: T) -> mpsc::Receiver<io::Result<Vec<u8>>>
where
    T: Read + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut output = Vec::new();
        let result = pipe.read_to_end(&mut output).map(|_| output);
        let _ = tx.send(result);
    });
    rx
}

fn receive_command_pipe(
    command: &str,
    receiver: mpsc::Receiver<io::Result<Vec<u8>>>,
    deadline: Instant,
    timeout_ms: u64,
    pid: u32,
    cancellation_token: Option<&ToolCancellationToken>,
) -> Result<Vec<u8>, ToolRuntimeError> {
    loop {
        if cancellation_token
            .map(ToolCancellationToken::is_cancelled)
            .unwrap_or(false)
        {
            return Err(ToolRuntimeError::CommandCancelled {
                command: command.to_string(),
                pid,
            });
        }

        let Some(remaining) = remaining_until(deadline) else {
            return Err(ToolRuntimeError::CommandTimedOut {
                command: command.to_string(),
                pid,
                timeout_ms,
            });
        };
        match receiver.recv_timeout(remaining.min(Duration::from_millis(COMMAND_WAIT_POLL_MS))) {
            Ok(Ok(output)) => return Ok(output),
            Ok(Err(source)) => {
                return Err(ToolRuntimeError::Command {
                    command: command.to_string(),
                    source,
                });
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(ToolRuntimeError::Command {
                    command: command.to_string(),
                    source: io::Error::other("output reader thread exited without result"),
                });
            }
        }
    }
}

fn remaining_until(deadline: Instant) -> Option<Duration> {
    deadline.checked_duration_since(Instant::now())
}

fn command_label(command: &str, args: &[String]) -> String {
    if args.is_empty() {
        command.to_string()
    } else {
        format!("{} {}", command, args.join(" "))
    }
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
        description: "Read a text file inside the active workspace, optionally restricted to a 1-based inclusive line range. The returned content is prefixed with real 1-based file line numbers for patch targeting; line-number prefixes are not file content and must not be copied into write_file content or patch_file context/removal lines.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative file path."
                },
                "startLine": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based first line to read. Must be null when endLine is null."
                },
                "endLine": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based last line to read, inclusive. Values beyond the file length read through the final line. Must be null when startLine is null."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 5000."
                }
            },
            "required": ["path", "startLine", "endLine", "timeoutMs"]
        }),
        strict: true,
    }
}

fn list_files_definition() -> ToolDefinition {
    ToolDefinition {
        name: LIST_FILES_TOOL,
        description: "List files and directories in a workspace-relative directory, optionally filtered by glob patterns.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative directory path. Use . for the workspace root."
                },
                "include": {
                    "type": ["array", "null"],
                    "items": { "type": "string" },
                    "description": "Optional glob patterns matched against returned workspace-relative paths. Null or an empty array includes everything not excluded."
                },
                "exclude": {
                    "type": ["array", "null"],
                    "items": { "type": "string" },
                    "description": "Optional glob patterns matched against returned workspace-relative paths to omit."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 5000."
                }
            },
            "required": ["path", "include", "exclude", "timeoutMs"]
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
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["query", "kind", "path", "limit", "timeoutMs"]
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
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["symbolId", "symbol", "path", "limit", "timeoutMs"]
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
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["symbolId", "symbol", "path", "limit", "timeoutMs"]
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
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["symbolId", "symbol", "path", "limit", "timeoutMs"]
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
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["path", "limit", "timeoutMs"]
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
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["query", "path", "timeoutMs"]
        }),
        strict: true,
    }
}

fn write_file_definition() -> ToolDefinition {
    ToolDefinition {
        name: WRITE_FILE_TOOL,
        description: "Write a complete text file, or replace a precise 1-based inclusive line range inside an existing workspace file. Prefer the line-range mode for small single-location edits after reading the target lines.",
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
                    "description": "Complete file content when startLine/endLine are null, or replacement text for the selected line range when both are integers. For line-range writes, include only the replacement lines for that range."
                },
                "startLine": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based first line to replace, inclusive. Set both startLine and endLine to integers for line-range mode; set both to null for a complete-file write."
                },
                "endLine": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based last line to replace, inclusive. Set both startLine and endLine to integers for line-range mode; set both to null for a complete-file write."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["path", "content", "startLine", "endLine", "timeoutMs"]
        }),
        strict: true,
    }
}

fn patch_file_definition() -> ToolDefinition {
    ToolDefinition {
        name: PATCH_FILE_TOOL,
        description: "Apply a strict single-file unified diff to an existing text file inside the active workspace. Use this mainly for multi-hunk or multi-location edits when the diff was produced or checked from current file content; prefer write_file line-range mode for small single-location edits. Before calling this tool, use read_file to confirm the target lines and ensure every context/removal line in the diff exactly matches the current file. If this tool fails, read the suggested range before retrying.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative existing file path to patch."
                },
                "diff": {
                    "type": "string",
                    "description": "Unified diff text containing one or more valid @@ -old,count +new,count @@ hunks for this file. Header line counts, context lines, and removed lines must exactly match the current file lines previously confirmed with read_file, excluding read_file line-number prefixes."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["path", "diff", "timeoutMs"]
        }),
        strict: true,
    }
}

fn create_todo_graph_definition() -> ToolDefinition {
    ToolDefinition {
        name: CREATE_TODO_GRAPH_TOOL,
        description: "Create or replace the current chat's todo graph. Use this instead of plain todo lists to preserve task context, dependencies, acceptance criteria, summaries, and nested subtasks.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "tasks": {
                    "type": "array",
                    "items": todo_graph_task_schema(),
                    "description": "Top-level tasks for the current chat todo graph."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["tasks", "timeoutMs"]
        }),
        strict: true,
    }
}

fn update_todo_graph_definition() -> ToolDefinition {
    ToolDefinition {
        name: UPDATE_TODO_GRAPH_TOOL,
        description: "Patch one task in the current chat's todo graph without resending the entire graph. Pass the task id and only the fields that should change.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "Id of the task to patch."
                },
                "patch": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "title": {
                            "type": ["string", "null"],
                            "description": "New task title, or null to leave unchanged."
                        },
                        "status": {
                            "type": ["string", "null"],
                            "enum": ["pending", "ready", "running", "blocked", "completed", "failed", "cancelled", null],
                            "description": "New task status, or null to leave unchanged."
                        },
                        "dependsOn": {
                            "type": ["array", "null"],
                            "items": { "type": "string" },
                            "description": "Complete replacement dependency id list, or null to leave unchanged."
                        },
                        "acceptance": {
                            "type": ["array", "null"],
                            "items": { "type": "string" },
                            "description": "Complete replacement acceptance criteria list, or null to leave unchanged."
                        },
                        "summary": {
                            "type": ["string", "null"],
                            "description": "New task progress/context summary, or null to leave unchanged."
                        },
                        "subtasks": {
                            "type": ["array", "null"],
                            "items": todo_graph_task_schema(),
                            "description": "Complete replacement nested subtask list, or null to leave unchanged."
                        }
                    },
                    "required": ["title", "status", "dependsOn", "acceptance", "summary", "subtasks"]
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["taskId", "patch", "timeoutMs"]
        }),
        strict: true,
    }
}

fn get_todo_graph_definition() -> ToolDefinition {
    ToolDefinition {
        name: GET_TODO_GRAPH_TOOL,
        description: "Read the current chat's todo graph, optionally filtering tasks by id or status such as completed, pending, ready, running, blocked, failed, or cancelled.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "status": {
                    "type": ["string", "null"],
                    "enum": ["pending", "ready", "running", "blocked", "completed", "failed", "cancelled", null],
                    "description": "Optional task status filter. Null returns all statuses."
                },
                "taskId": {
                    "type": ["string", "null"],
                    "description": "Optional exact task id filter. Null returns all task ids."
                },
                "includeSubtasks": {
                    "type": "boolean",
                    "description": "When filtering, include matching task subtasks in the returned task objects."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["status", "taskId", "includeSubtasks", "timeoutMs"]
        }),
        strict: true,
    }
}

fn ask_question_definition() -> ToolDefinition {
    ToolDefinition {
        name: ASK_QUESTION_TOOL,
        description: "Ask the user one or more blocking questions through the Foco UI when required information is missing. Provide choices when an answer should be selected from known options; otherwise allow free-form input.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "questions": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "Clear question to show the user."
                            },
                            "options": {
                                "type": ["array", "null"],
                                "items": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "Short visible option label."
                                        },
                                        "value": {
                                            "type": "string",
                                            "description": "Exact value returned when the user selects this option."
                                        },
                                        "description": {
                                            "type": ["string", "null"],
                                            "description": "Optional one-sentence explanation of this option."
                                        }
                                    },
                                    "required": ["label", "value", "description"]
                                },
                                "description": "Optional choices for this question. Null means free-form input only."
                            },
                            "allowFreeText": {
                                "type": "boolean",
                                "description": "Whether the user may type an answer manually."
                            }
                        },
                        "required": ["question", "options", "allowFreeText"]
                    },
                    "description": "Questions that must all be answered before the tool returns."
                }
            },
            "required": ["questions"]
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
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional command timeout in milliseconds. Defaults to 60000."
                }
            },
            "required": ["command", "args", "cwd", "timeoutMs"]
        }),
        strict: true,
    }
}

fn sleep_definition() -> ToolDefinition {
    ToolDefinition {
        name: SLEEP_TOOL,
        description: "Pause tool execution for the requested duration.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "durationMs": {
                    "type": "integer",
                    "description": "Pause duration in milliseconds."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 300000."
                }
            },
            "required": ["durationMs", "timeoutMs"]
        }),
        strict: true,
    }
}

fn todo_graph_task_schema() -> Value {
    todo_graph_task_schema_with_depth(3)
}

fn todo_graph_task_schema_with_depth(depth: usize) -> Value {
    let subtasks_schema = if depth == 0 {
        json!({
            "type": "array",
            "items": {
                "type": "object",
                "additionalProperties": false,
                "properties": {},
                "required": []
            },
            "maxItems": 0
        })
    } else {
        json!({
            "type": "array",
            "items": todo_graph_task_schema_with_depth(depth - 1)
        })
    };

    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "id": {
                "type": "string",
                "description": "Stable unique task id inside the graph."
            },
            "title": {
                "type": "string",
                "description": "Short human-readable task title."
            },
            "status": {
                "type": "string",
                "enum": ["pending", "ready", "running", "blocked", "completed", "failed", "cancelled"],
                "description": "Task execution status."
            },
            "dependsOn": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Task ids that must be completed or resolved before this task can proceed."
            },
            "acceptance": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Acceptance criteria for this task."
            },
            "summary": {
                "type": "string",
                "description": "Current context, decisions, blockers, and progress summary for interruption recovery."
            },
            "createdAt": {
                "type": ["string", "null"],
                "description": "Ignored on input; the server writes the task creation timestamp."
            },
            "updatedAt": {
                "type": ["string", "null"],
                "description": "Ignored on input; the server writes the task update timestamp."
            },
            "subtasks": subtasks_schema
        },
        "required": ["id", "title", "status", "dependsOn", "acceptance", "summary", "createdAt", "updatedAt", "subtasks"]
    })
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
struct ListFilesInput {
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
struct GraphFindSymbolsInput {
    query: String,
    kind: Option<String>,
    path: Option<String>,
    limit: Option<usize>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphSymbolLookupInput {
    symbol_id: Option<i64>,
    symbol: Option<String>,
    path: Option<String>,
    limit: Option<usize>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphRelatedFilesInput {
    path: String,
    limit: Option<usize>,
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
struct PatchFileInput {
    path: String,
    diff: String,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTodoGraphInput {
    tasks: Vec<TodoGraphTaskInput>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTodoGraphInput {
    task_id: String,
    patch: TodoGraphPatchInput,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetTodoGraphInput {
    status: Option<String>,
    task_id: Option<String>,
    include_subtasks: bool,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TodoGraphPatchInput {
    title: Option<String>,
    status: Option<String>,
    depends_on: Option<Vec<String>>,
    acceptance: Option<Vec<String>>,
    summary: Option<String>,
    subtasks: Option<Vec<TodoGraphTaskInput>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TodoGraphTaskInput {
    id: String,
    title: String,
    status: String,
    depends_on: Vec<String>,
    acceptance: Vec<String>,
    summary: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    subtasks: Vec<TodoGraphTaskInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunCommandInput {
    command: String,
    args: Option<Vec<String>>,
    cwd: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SleepInput {
    duration_ms: u64,
    timeout_ms: Option<u64>,
}

#[derive(Debug)]
enum ToolRuntimeError {
    Cancelled,
    Command {
        command: String,
        source: io::Error,
    },
    CommandCancelled {
        command: String,
        pid: u32,
    },
    CommandTimedOut {
        command: String,
        pid: u32,
        timeout_ms: u64,
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
    NotDirectory(PathBuf),
    NotFile(PathBuf),
    PatchRejected(PatchRejection),
    UnsupportedEncoding(PathBuf),
    UnknownTool(String),
    WorkspaceDatabase(WorkspaceDatabaseError),
}

impl fmt::Display for ToolRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(formatter, "tool execution cancelled"),
            Self::Command { command, source } => {
                write!(formatter, "failed to run {command}: {source}")
            }
            Self::CommandCancelled { command, pid } => {
                write!(formatter, "{command} (pid {pid}) was cancelled")
            }
            Self::CommandTimedOut {
                command,
                pid,
                timeout_ms,
            } => write!(
                formatter,
                "{command} (pid {pid}) timed out after {timeout_ms} ms"
            ),
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
            Self::NotDirectory(path) => write!(formatter, "{} is not a directory", path.display()),
            Self::NotFile(path) => write!(formatter, "{} is not a file", path.display()),
            Self::PatchRejected(rejection) => write!(formatter, "{rejection}"),
            Self::UnsupportedEncoding(path) => write!(
                formatter,
                "{} uses an unsupported text encoding; supported encodings are UTF-8, UTF-8 BOM, UTF-16 LE BOM, and UTF-16 BE BOM",
                path.display()
            ),
            Self::UnknownTool(tool) => write!(formatter, "unknown built-in tool '{tool}'"),
            Self::WorkspaceDatabase(source) => {
                write!(formatter, "workspace database error: {source}")
            }
        }
    }
}

impl std::error::Error for ToolRuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WorkspaceDatabase(source) => Some(source),
            Self::Cancelled
            | Self::Command { .. }
            | Self::CommandCancelled { .. }
            | Self::CommandTimedOut { .. }
            | Self::CommandFailed { .. }
            | Self::FileTooLarge { .. }
            | Self::InvalidArguments(_)
            | Self::InvalidPath(_)
            | Self::InvalidToolOutput { .. }
            | Self::Io { .. }
            | Self::NotDirectory(_)
            | Self::NotFile(_)
            | Self::PatchRejected(_)
            | Self::UnsupportedEncoding(_)
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
            json!({ "path": "../outside.txt", "startLine": null, "endLine": null }),
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
            json!({ "path": "note.txt", "startLine": null, "endLine": null }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["content"], "1\thello");
    }

    #[test]
    fn reads_workspace_file_with_line_numbers_without_trailing_newline() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\ntwo\nthree").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "note.txt", "startLine": null, "endLine": null }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["content"], "1\tone\n2\ttwo\n3\tthree");
    }

    #[test]
    fn reads_workspace_file_line_range() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\ntwo\nthree\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "note.txt", "startLine": 2, "endLine": 3 }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["content"], "2\ttwo\n3\tthree\n");
        assert_eq!(result.output["startLine"], 2);
        assert_eq!(result.output["endLine"], 3);
    }

    #[test]
    fn rejects_full_file_read_larger_than_limit() {
        let workspace = tempfile::tempdir().expect("workspace");
        let content = "x".repeat(MAX_FULL_READ_BYTES as usize + 1);
        fs::write(workspace.path().join("large.txt"), content).expect("write large file");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "large.txt", "startLine": null, "endLine": null }),
        );

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("too large to read")
        );
    }

    #[test]
    fn reads_line_range_from_file_larger_than_full_read_limit() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut content = String::from("needle\n");
        while content.len() <= MAX_FULL_READ_BYTES as usize {
            content.push_str("padding line\n");
        }
        fs::write(workspace.path().join("large.txt"), content).expect("write large file");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "large.txt", "startLine": 1, "endLine": 1 }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["content"], "1\tneedle\n");
        assert_eq!(result.output["startLine"], 1);
        assert_eq!(result.output["endLine"], 1);
    }

    #[test]
    fn rejects_line_range_output_larger_than_limit() {
        let workspace = tempfile::tempdir().expect("workspace");
        let first_line = "x".repeat(MAX_RANGED_READ_OUTPUT_BYTES);
        fs::write(
            workspace.path().join("large-line.txt"),
            format!("{first_line}\nsmall\n"),
        )
        .expect("write large line file");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "large-line.txt", "startLine": 1, "endLine": 1 }),
        );

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("line range output is too large")
        );
    }

    #[test]
    fn reads_line_range_to_end_when_end_line_exceeds_file() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\ntwo\nthree\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "note.txt", "startLine": 2, "endLine": 160 }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["content"], "2\ttwo\n3\tthree\n");
        assert_eq!(result.output["startLine"], 2);
        assert_eq!(result.output["endLine"], 3);
    }

    #[test]
    fn rejects_read_line_range_start_outside_file() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "note.txt", "startLine": 2, "endLine": 2 }),
        );

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("file has 1 lines")
        );
    }

    #[test]
    fn rejects_partial_read_line_range() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "note.txt", "startLine": 1, "endLine": null }),
        );

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("startLine and endLine must both be null")
        );
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
    fn lists_workspace_files_with_glob_filters() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("a.rs"), "a").expect("write a");
        fs::write(workspace.path().join("b.txt"), "b").expect("write b");
        fs::write(workspace.path().join("test.rs"), "test").expect("write test");

        let result = execute_builtin_tool(
            workspace.path(),
            LIST_FILES_TOOL,
            json!({
                "path": ".",
                "include": ["*.rs"],
                "exclude": ["test.rs"],
                "timeoutMs": null
            }),
        );

        assert!(!result.is_error);
        let entries = result.output["entries"].as_array().expect("entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["path"], "a.rs");
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
        let read_file: ReadFileInput = parse_arguments(json!({
            "path": "note.txt",
            "startLine": null,
            "endLine": null,
            "timeoutMs": null
        }))
        .expect("read file input");
        assert_eq!(read_file.path, "note.txt");
        assert_eq!(read_file.start_line, None);
        assert_eq!(read_file.end_line, None);
        assert_eq!(read_file.timeout_ms, None);

        let graph_symbols: GraphFindSymbolsInput = parse_arguments(json!({
            "query": "helper",
            "kind": null,
            "path": null,
            "limit": null,
            "timeoutMs": null
        }))
        .expect("graph symbols input");
        assert_eq!(graph_symbols.query, "helper");
        assert_eq!(graph_symbols.kind, None);
        assert_eq!(graph_symbols.path, None);
        assert_eq!(graph_symbols.limit, None);
        assert_eq!(graph_symbols.timeout_ms, None);

        let graph_lookup: GraphSymbolLookupInput = parse_arguments(json!({
            "symbolId": null,
            "symbol": "helper",
            "path": null,
            "limit": null,
            "timeoutMs": null
        }))
        .expect("graph lookup input");
        assert_eq!(graph_lookup.symbol_id, None);
        assert_eq!(graph_lookup.symbol.as_deref(), Some("helper"));
        assert_eq!(graph_lookup.path, None);
        assert_eq!(graph_lookup.limit, None);
        assert_eq!(graph_lookup.timeout_ms, None);

        let run_command: RunCommandInput = parse_arguments(json!({
            "command": "git",
            "args": null,
            "cwd": null,
            "timeoutMs": null
        }))
        .expect("run command input");
        assert_eq!(run_command.command, "git");
        assert_eq!(run_command.args, None);
        assert_eq!(run_command.cwd, None);
        assert_eq!(run_command.timeout_ms, None);

        let write_file: WriteFileInput = parse_arguments(json!({
            "path": "note.txt",
            "content": "hello",
            "startLine": null,
            "endLine": null,
            "timeoutMs": null
        }))
        .expect("write file input");
        assert_eq!(write_file.path, "note.txt");
        assert_eq!(write_file.content, "hello");
        assert_eq!(write_file.start_line, None);
        assert_eq!(write_file.end_line, None);
        assert_eq!(write_file.timeout_ms, None);
    }

    #[test]
    fn todo_graph_tools_round_trip_current_chat() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
        database
            .insert_chat("chat-1", "ToDo graph chat")
            .expect("chat insert");
        drop(database);

        let create = execute_builtin_tool_for_chat(
            workspace.path(),
            Some("chat-1"),
            CREATE_TODO_GRAPH_TOOL,
            json!({
                "tasks": [
                    {
                        "id": "plan",
                        "title": "Plan work",
                        "status": "ready",
                        "dependsOn": [],
                        "acceptance": ["Plan is clear"],
                        "summary": "Find the smallest path.",
                        "createdAt": null,
                        "updatedAt": null,
                        "subtasks": [
                            {
                                "id": "probe",
                                "title": "Probe code",
                                "status": "pending",
                                "dependsOn": ["plan"],
                                "acceptance": ["Entrypoints identified"],
                                "summary": "",
                                "createdAt": null,
                                "updatedAt": null,
                                "subtasks": []
                            }
                        ]
                    }
                ],
                "timeoutMs": null
            }),
        );

        assert!(!create.is_error, "{:?}", create.output);
        assert_eq!(create.output["exists"], true);
        assert_eq!(create.output["tasks"][0]["id"], "plan");
        assert!(create.output["tasks"][0]["createdAt"].is_string());

        let update = execute_builtin_tool_for_chat(
            workspace.path(),
            Some("chat-1"),
            UPDATE_TODO_GRAPH_TOOL,
            json!({
                "taskId": "probe",
                "patch": {
                    "title": null,
                    "status": "completed",
                    "dependsOn": null,
                    "acceptance": null,
                    "summary": "Found store, tools, app, and web entrypoints.",
                    "subtasks": null
                },
                "timeoutMs": null
            }),
        );

        assert!(!update.is_error, "{:?}", update.output);
        assert_eq!(update.output["updatedTask"]["id"], "probe");
        assert_eq!(update.output["updatedTask"]["status"], "completed");

        let completed = execute_builtin_tool_for_chat(
            workspace.path(),
            Some("chat-1"),
            GET_TODO_GRAPH_TOOL,
            json!({
                "status": "completed",
                "taskId": null,
                "includeSubtasks": false,
                "timeoutMs": null
            }),
        );

        assert!(!completed.is_error, "{:?}", completed.output);
        assert_eq!(
            completed.output["tasks"].as_array().expect("tasks").len(),
            1
        );
        assert_eq!(completed.output["tasks"][0]["id"], "probe");
        assert_eq!(completed.output["tasks"][0]["subtasks"], json!([]));
    }

    #[test]
    fn ripgrep_command_uses_configured_path() {
        let configured = PathBuf::from("/tmp/foco-rg");
        set_ripgrep_path(Some(configured.clone()));

        assert_eq!(ripgrep_command(), configured.to_string_lossy());

        set_ripgrep_path(None);
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
            json!({ "path": "note.txt", "content": "hello", "startLine": null, "endLine": null }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["path"], "note.txt");
        assert_eq!(
            fs::read_to_string(workspace.path().join("note.txt")).expect("read note"),
            "hello"
        );
    }

    #[test]
    fn writes_workspace_file_line_range() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\r\ntwo\r\nthree\r\n")
            .expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            WRITE_FILE_TOOL,
            json!({ "path": "note.txt", "content": "TWO", "startLine": 2, "endLine": 2 }),
        );

        assert!(!result.is_error);
        assert_eq!(
            fs::read_to_string(workspace.path().join("note.txt")).expect("read note"),
            "one\r\nTWO\r\nthree\r\n"
        );
    }

    #[test]
    fn writes_existing_file_with_same_utf16le_bom_encoding() {
        let workspace = tempfile::tempdir().expect("workspace");
        let path = workspace.path().join("note.txt");
        fs::write(
            &path,
            encode_text_file("one\ntwo\n", TextEncoding::Utf16LeBom),
        )
        .expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            WRITE_FILE_TOOL,
            json!({ "path": "note.txt", "content": "TWO", "startLine": 2, "endLine": 2 }),
        );

        assert!(!result.is_error);
        let bytes = fs::read(&path).expect("read note bytes");
        assert!(bytes.starts_with(&[0xFF, 0xFE]));
        let (content, encoding) = decode_text_file(&path, &bytes).expect("decode note");
        assert_eq!(encoding, TextEncoding::Utf16LeBom);
        assert_eq!(content, "one\nTWO\n");
    }

    #[test]
    fn writes_new_file_as_utf8() {
        let workspace = tempfile::tempdir().expect("workspace");
        let path = workspace.path().join("note.txt");

        let result = execute_builtin_tool(
            workspace.path(),
            WRITE_FILE_TOOL,
            json!({ "path": "note.txt", "content": "你好", "startLine": null, "endLine": null }),
        );

        assert!(!result.is_error);
        assert_eq!(fs::read(&path).expect("read note bytes"), "你好".as_bytes());
    }

    #[test]
    fn patches_workspace_file_with_unified_diff() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\ntwo\nthree\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            PATCH_FILE_TOOL,
            json!({
                "path": "note.txt",
                "diff": "@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n",
                "timeoutMs": null
            }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["appliedHunks"], 1);
        assert_eq!(
            fs::read_to_string(workspace.path().join("note.txt")).expect("read note"),
            "one\nTWO\nthree\n"
        );
    }

    #[test]
    fn rejects_patch_file_when_context_does_not_match() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\ntwo\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            PATCH_FILE_TOOL,
            json!({
                "path": "note.txt",
                "diff": "@@ -1,2 +1,2 @@\n one\n-three\n+THREE\n",
                "timeoutMs": null
            }),
        );

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("diff removal line does not match")
        );
        assert_eq!(result.output["hunk"]["declaredOldStart"], 1);
        assert_eq!(result.output["hunk"]["declaredOldCount"], 2);
        assert_eq!(result.output["hunk"]["declaredNewStart"], 1);
        assert_eq!(result.output["hunk"]["declaredNewCount"], 2);
        assert_eq!(result.output["hunk"]["actualOldCount"], 2);
        assert_eq!(result.output["hunk"]["actualNewCount"], 2);
        assert_eq!(result.output["firstMismatch"]["kind"], "removal");
        assert_eq!(result.output["firstMismatch"]["line"], 2);
        assert_eq!(result.output["firstMismatch"]["expected"], "three");
        assert_eq!(result.output["firstMismatch"]["actual"], "two");
        assert_eq!(result.output["suggestedReadRange"]["startLine"], 1);
        assert_eq!(result.output["suggestedReadRange"]["endLine"], 7);
    }

    #[test]
    fn rejects_patch_file_with_hunk_count_diagnostics() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\ntwo\nthree\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            PATCH_FILE_TOOL,
            json!({
                "path": "note.txt",
                "diff": "@@ -1,2 +1,2 @@\n one\n-two\n+TWO\n three\n",
                "timeoutMs": null
            }),
        );

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("diff hunk line counts do not match")
        );
        assert_eq!(result.output["hunk"]["declaredOldStart"], 1);
        assert_eq!(result.output["hunk"]["declaredOldCount"], 2);
        assert_eq!(result.output["hunk"]["declaredNewStart"], 1);
        assert_eq!(result.output["hunk"]["declaredNewCount"], 2);
        assert_eq!(result.output["hunk"]["actualOldCount"], 3);
        assert_eq!(result.output["hunk"]["actualNewCount"], 3);
        assert_eq!(result.output["suggestedReadRange"]["startLine"], 1);
        assert_eq!(result.output["suggestedReadRange"]["endLine"], 6);
    }

    #[test]
    fn sleeps_for_requested_duration() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(
            workspace.path(),
            SLEEP_TOOL,
            json!({ "durationMs": 1, "timeoutMs": null }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["durationMs"], 1);
    }

    #[test]
    fn sleep_tool_stops_when_cancelled() {
        let workspace = tempfile::tempdir().expect("workspace");
        let cancellation_token = ToolCancellationToken::new();
        cancellation_token.cancel();

        let result = execute_builtin_tool_for_chat_with_cancellation(
            workspace.path(),
            None,
            SLEEP_TOOL,
            json!({ "durationMs": 60_000, "timeoutMs": null }),
            Some(cancellation_token),
        );

        assert!(result.is_error);
        assert_eq!(result.output["cancelled"], true);
    }

    #[test]
    fn rejects_existing_file_with_unsupported_encoding() {
        let workspace = tempfile::tempdir().expect("workspace");
        let path = workspace.path().join("note.txt");
        fs::write(&path, [0xFF, 0x00, 0xFF]).expect("write invalid text");

        let result = execute_builtin_tool(
            workspace.path(),
            WRITE_FILE_TOOL,
            json!({ "path": "note.txt", "content": "hello", "startLine": null, "endLine": null }),
        );

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("unsupported text encoding")
        );
    }

    #[test]
    fn rejects_line_range_write_for_new_file() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(
            workspace.path(),
            WRITE_FILE_TOOL,
            json!({ "path": "note.txt", "content": "hello", "startLine": 1, "endLine": 1 }),
        );

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("line-range writes require an existing file")
        );
    }

    #[test]
    fn rejects_write_path_outside_workspace() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(
            workspace.path(),
            WRITE_FILE_TOOL,
            json!({ "path": "../note.txt", "content": "hello", "startLine": null, "endLine": null }),
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
    fn builtin_tools_do_not_include_git_diff() {
        let tool_names = builtin_tool_definitions()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(!tool_names.contains(&"git_diff"));
    }

    #[test]
    fn removed_git_diff_tool_reports_unknown_tool() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(workspace.path(), "git_diff", json!({}));

        assert!(result.is_error);
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("unknown built-in tool")
        );
    }

    #[test]
    fn run_command_can_return_workspace_git_diff() {
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

        let status = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": "git",
                "args": ["status", "--short"],
                "cwd": null,
                "timeoutMs": null
            }),
        );
        let diff = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": "git",
                "args": ["diff"],
                "cwd": null,
                "timeoutMs": null
            }),
        );

        assert!(!status.is_error);
        assert!(!diff.is_error);
        assert!(
            status.output["pid"].as_u64().expect("status pid") > 0,
            "run_command should include the spawned process pid"
        );
        assert!(
            diff.output["pid"].as_u64().expect("diff pid") > 0,
            "run_command should include the spawned process pid"
        );
        assert!(
            status.output["stdout"]
                .as_str()
                .expect("status")
                .contains("M note.txt")
        );
        assert!(
            diff.output["stdout"]
                .as_str()
                .expect("diff")
                .contains("-before")
        );
        assert!(
            diff.output["stdout"]
                .as_str()
                .expect("diff")
                .contains("+after")
        );
    }

    #[test]
    fn run_command_times_out() {
        let workspace = tempfile::tempdir().expect("workspace");
        let command = std::env::current_exe()
            .expect("current test executable")
            .to_string_lossy()
            .to_string();

        let result = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": command,
                "args": ["--ignored", "--exact", "tests::timeout_child_process"],
                "cwd": null,
                "timeoutMs": 1
            }),
        );

        assert!(result.is_error);
        assert!(
            result.output["pid"].as_u64().expect("timeout pid") > 0,
            "timed out run_command should include the spawned process pid"
        );
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("timed out")
        );
    }

    #[test]
    fn run_command_times_out_when_grandchild_keeps_stdout_open() {
        let workspace = tempfile::tempdir().expect("workspace");
        let command = std::env::current_exe()
            .expect("current test executable")
            .to_string_lossy()
            .to_string();

        let result = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": command,
                "args": ["--ignored", "--exact", "tests::pipe_holder_parent_process"],
                "cwd": null,
                "timeoutMs": 100
            }),
        );

        assert!(result.is_error);
        assert!(
            result.output["pid"].as_u64().expect("timeout pid") > 0,
            "timed out run_command should include the spawned process pid"
        );
        assert!(
            result
                .output
                .get("error")
                .and_then(Value::as_str)
                .expect("error")
                .contains("timed out")
        );
    }

    #[test]
    #[ignore]
    fn timeout_child_process() {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    #[test]
    #[ignore]
    fn pipe_holder_parent_process() {
        let command = std::env::current_exe().expect("current test executable");
        let _child = Command::new(command)
            .args(["--ignored", "--exact", "tests::pipe_holder_child_process"])
            .spawn()
            .expect("spawn pipe holder child");
    }

    #[test]
    #[ignore]
    fn pipe_holder_child_process() {
        std::thread::sleep(std::time::Duration::from_secs(2));
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
