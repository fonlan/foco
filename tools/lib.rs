mod agent_tools;
mod command_tools;
mod definitions;
mod errors;
mod file_tools;
mod graph_tools;
mod todo_tools;

use std::{
    fs, io,
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

use foco_store::workspace::WorkspaceDatabaseError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::errors::{ToolRuntimeError, tool_error_output};

pub const READ_FILE_TOOL: &str = "read_file";
pub const FIND_FILES_TOOL: &str = "find_files";
pub const SEARCH_TEXT_TOOL: &str = "search_text";
pub const WEB_SEARCH_TOOL: &str = "web_search";
pub const WEB_FETCH_TOOL: &str = "web_fetch";
pub const WRITE_FILE_TOOL: &str = "write_file";
pub const EDIT_FILE_TOOL: &str = "edit_file";
pub const RUN_COMMAND_TOOL: &str = "run_command";
pub const SLEEP_TOOL: &str = "sleep";
pub const GRAPH_FIND_SYMBOLS_TOOL: &str = "graph_find_symbols";
pub const GRAPH_FIND_CALLERS_TOOL: &str = "graph_find_callers";
pub const GRAPH_FIND_CALLEES_TOOL: &str = "graph_find_callees";
pub const GRAPH_FIND_REFERENCES_TOOL: &str = "graph_find_references";
pub const GRAPH_RELATED_FILES_TOOL: &str = "graph_related_files";
pub const GRAPH_EXPLORE_TOOL: &str = "graph_explore";
pub const CREATE_TODO_GRAPH_TOOL: &str = "create_todo_graph";
pub const UPDATE_TODO_GRAPH_TOOL: &str = "update_todo_graph";
pub const GET_TODO_GRAPH_TOOL: &str = "get_todo_graph";
pub const ASK_QUESTION_TOOL: &str = "ask_question";
pub const AGENT_LIST_TOOL: &str = "agent_list";
pub const AGENT_GET_TASK_TOOL: &str = "agent_get_task";
pub const AGENT_SEND_MESSAGE_TOOL: &str = "agent_send_message";
pub const AGENT_DELEGATE_TASK_TOOL: &str = "agent_delegate_task";
pub const AGENT_CANCEL_TASK_TOOL: &str = "agent_cancel_task";
pub const AGENT_WAIT_TASKS_TOOL: &str = "agent_wait_tasks";
pub const AGENT_TRANSFER_TASK_TOOL: &str = "agent_transfer_task";

const MAX_FULL_READ_BYTES: u64 = 1024 * 1024;
const MAX_RANGED_READ_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_RANGED_READ_OUTPUT_BYTES: usize = 512 * 1024;
const MAX_FIND_ENTRIES: usize = 200;
const MAX_SEARCH_MATCHES: usize = 200;
const MAX_SEARCH_TEXT_LINE_BYTES: usize = 4 * 1024;
const MAX_SEARCH_TEXT_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_COMMAND_OUTPUT_BYTES: usize = 64 * 1024;
const DEFAULT_GRAPH_RESULT_LIMIT: usize = 20;
const MAX_GRAPH_RESULT_LIMIT: usize = 50;
const DEFAULT_GRAPH_EXPLORE_RESULT_LIMIT: usize = 5;
const MAX_GRAPH_EXPLORE_RESULT_LIMIT: usize = 20;
const DEFAULT_GRAPH_EXPLORE_CONTEXT_LINES: usize = 2;
const MAX_GRAPH_EXPLORE_CONTEXT_LINES: usize = 20;
const MAX_GRAPH_EXPLORE_SYMBOL_LINES: usize = 240;
const MAX_GRAPH_EXPLORE_OUTPUT_BYTES: usize = 512 * 1024;
const DEFAULT_FILE_TOOL_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_GRAPH_TOOL_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_SEARCH_TEXT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_WEB_TOOL_TIMEOUT_MS: u64 = 15_000;
const DEFAULT_WRITE_FILE_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_SLEEP_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_RUN_COMMAND_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_TODO_GRAPH_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_AGENT_TOOL_TIMEOUT_MS: u64 = 10_000;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolOutputChunk {
    pub stream: ToolOutputStream,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolOutputStream {
    Stdout,
    Stderr,
}

pub trait ToolOutputSink: Send + Sync {
    fn output_chunk(&self, chunk: ToolOutputChunk);
}

#[derive(Clone, Debug, Default)]
pub struct ToolCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl ToolCancellationToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

pub fn builtin_tool_definitions() -> Vec<ToolDefinition> {
    definitions::builtin_tool_definitions()
}

pub fn agent_tool_definitions() -> Vec<ToolDefinition> {
    agent_tools::agent_tool_definitions()
}

pub fn builtin_tool_timeout_ms(tool_name: &str, arguments: &Value) -> Result<u64, String> {
    errors::builtin_tool_timeout_ms(tool_name, arguments)
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
    execute_builtin_tool_for_chat_with_cancellation_and_output_sink(
        workspace_path,
        chat_id,
        tool_name,
        arguments,
        cancellation_token,
        None,
    )
}

pub fn execute_builtin_tool_for_chat_with_cancellation_and_output_sink(
    workspace_path: &Path,
    chat_id: Option<&str>,
    tool_name: &str,
    arguments: Value,
    cancellation_token: Option<ToolCancellationToken>,
    output_sink: Option<Arc<dyn ToolOutputSink>>,
) -> ToolExecution {
    match execute_builtin_tool_inner(
        workspace_path,
        chat_id,
        tool_name,
        arguments,
        cancellation_token.as_ref(),
        output_sink.as_deref(),
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

fn execute_builtin_tool_inner(
    workspace_path: &Path,
    chat_id: Option<&str>,
    tool_name: &str,
    arguments: Value,
    cancellation_token: Option<&ToolCancellationToken>,
    output_sink: Option<&dyn ToolOutputSink>,
) -> Result<Value, ToolRuntimeError> {
    match tool_name {
        READ_FILE_TOOL => file_tools::read_file(workspace_path, arguments),
        FIND_FILES_TOOL => file_tools::find_files(workspace_path, arguments),
        GRAPH_FIND_SYMBOLS_TOOL => graph_tools::graph_find_symbols(workspace_path, arguments),
        GRAPH_FIND_CALLERS_TOOL => graph_tools::graph_find_callers(workspace_path, arguments),
        GRAPH_FIND_CALLEES_TOOL => graph_tools::graph_find_callees(workspace_path, arguments),
        GRAPH_FIND_REFERENCES_TOOL => graph_tools::graph_find_references(workspace_path, arguments),
        GRAPH_RELATED_FILES_TOOL => graph_tools::graph_related_files(workspace_path, arguments),
        GRAPH_EXPLORE_TOOL => graph_tools::graph_explore(workspace_path, arguments),
        SEARCH_TEXT_TOOL => file_tools::search_text(workspace_path, arguments, cancellation_token),
        WEB_SEARCH_TOOL | WEB_FETCH_TOOL => Err(ToolRuntimeError::InvalidArguments(format!(
            "{tool_name} requires app web runtime configuration"
        ))),
        WRITE_FILE_TOOL => file_tools::write_file(workspace_path, arguments),
        EDIT_FILE_TOOL => file_tools::edit_file(workspace_path, arguments),
        CREATE_TODO_GRAPH_TOOL => todo_tools::create_todo_graph(workspace_path, chat_id, arguments),
        UPDATE_TODO_GRAPH_TOOL => todo_tools::update_todo_graph(workspace_path, chat_id, arguments),
        GET_TODO_GRAPH_TOOL => todo_tools::get_todo_graph(workspace_path, chat_id, arguments),
        ASK_QUESTION_TOOL => Err(ToolRuntimeError::InvalidArguments(
            "ask_question must be executed through the chat UI question bridge".to_string(),
        )),
        RUN_COMMAND_TOOL => {
            command_tools::run_command(workspace_path, arguments, cancellation_token, output_sink)
        }
        SLEEP_TOOL => command_tools::sleep_tool(arguments, cancellation_token),
        other => Err(ToolRuntimeError::UnknownTool(other.to_string())),
    }
}

pub(crate) fn parse_arguments<T>(arguments: Value) -> Result<T, ToolRuntimeError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(arguments).map_err(|source| {
        ToolRuntimeError::InvalidArguments(format!("tool arguments do not match schema: {source}"))
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TextEncoding {
    Utf8,
    Utf8Bom,
    Utf16LeBom,
    Utf16BeBom,
}

pub(crate) fn decode_text_file(
    path: &Path,
    bytes: &[u8],
) -> Result<(String, TextEncoding), ToolRuntimeError> {
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

pub(crate) fn encode_text_file(content: &str, encoding: TextEncoding) -> Vec<u8> {
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
pub(crate) struct LineRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl LineRange {
    pub(crate) fn new(start: usize, end: usize) -> Result<Self, ToolRuntimeError> {
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

pub(crate) fn parse_optional_line_range(
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

pub(crate) fn normalize_read_line_range(
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

pub(crate) fn count_text_lines(content: &str) -> usize {
    line_spans(content).len()
}

pub(crate) fn read_line_range(content: &str, range: &LineRange) -> String {
    let spans = line_spans(content);
    let start = spans[range.start - 1].start_byte;
    let end = spans[range.end - 1].end_byte;
    content[start..end].to_string()
}

pub(crate) fn numbered_content(content: &str, start_line: usize) -> String {
    let mut numbered = String::new();
    for (index, span) in line_spans(content).into_iter().enumerate() {
        numbered.push_str(&(start_line + index).to_string());
        numbered.push('\t');
        numbered.push_str(&content[span.start_byte..span.end_byte]);
    }

    numbered
}

pub(crate) fn replace_line_range(
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

pub(crate) fn resolve_workspace_file(
    workspace_path: &Path,
    input: &str,
) -> Result<PathBuf, ToolRuntimeError> {
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

pub(crate) fn resolve_workspace_path(
    workspace_path: &Path,
    input: &str,
) -> Result<PathBuf, ToolRuntimeError> {
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

pub(crate) fn resolve_workspace_write_path(
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

pub(crate) fn normalize_workspace_path_text(input: &str) -> Result<String, ToolRuntimeError> {
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

pub(crate) fn relative_workspace_path(
    workspace_path: &Path,
    path: &Path,
) -> Result<String, ToolRuntimeError> {
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

pub(crate) fn run_command_with_timeout(
    command: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    cancellation_token: Option<&ToolCancellationToken>,
    output_sink: Option<&dyn ToolOutputSink>,
    output_limits: Option<CommandOutputLimits>,
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
    let mut stdout_output = Vec::new();
    let mut stderr_output = Vec::new();
    let mut stdout_complete = false;
    let mut stderr_complete = false;
    let mut exit_status = None;

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

        if let Err(error) = drain_command_pipe(
            &command_label,
            &stdout_handle,
            ToolOutputStream::Stdout,
            &mut stdout_output,
            &mut stdout_complete,
            pid,
            output_sink,
            output_limits.and_then(|limits| limits.stdout_bytes),
        ) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        if let Err(error) = drain_command_pipe(
            &command_label,
            &stderr_handle,
            ToolOutputStream::Stderr,
            &mut stderr_output,
            &mut stderr_complete,
            pid,
            output_sink,
            output_limits.and_then(|limits| limits.stderr_bytes),
        ) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }

        if exit_status.is_none() {
            exit_status = child
                .try_wait()
                .map_err(|source| ToolRuntimeError::Command {
                    command: command_label.clone(),
                    source,
                })?;
        }

        if let Some(status) = exit_status {
            if stdout_complete && stderr_complete {
                return Ok(CommandRunOutput {
                    pid,
                    status,
                    stdout: stdout_output,
                    stderr: stderr_output,
                });
            }
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

pub(crate) struct CommandRunOutput {
    pub(crate) pid: u32,
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

#[derive(Clone, Copy)]
pub(crate) struct CommandOutputLimits {
    pub(crate) stdout_bytes: Option<usize>,
    pub(crate) stderr_bytes: Option<usize>,
}

enum CommandPipeMessage {
    Chunk(Vec<u8>),
    Complete,
}

fn read_command_pipe<T>(mut pipe: T) -> mpsc::Receiver<io::Result<CommandPipeMessage>>
where
    T: Read + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => {
                    let _ = tx.send(Ok(CommandPipeMessage::Complete));
                    break;
                }
                Ok(bytes_read) => {
                    if tx
                        .send(Ok(CommandPipeMessage::Chunk(buffer[..bytes_read].to_vec())))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(source) => {
                    let _ = tx.send(Err(source));
                    break;
                }
            }
        }
    });
    rx
}

fn drain_command_pipe(
    command: &str,
    receiver: &mpsc::Receiver<io::Result<CommandPipeMessage>>,
    stream: ToolOutputStream,
    output: &mut Vec<u8>,
    complete: &mut bool,
    pid: u32,
    output_sink: Option<&dyn ToolOutputSink>,
    output_limit: Option<usize>,
) -> Result<(), ToolRuntimeError> {
    if *complete {
        return Ok(());
    }

    loop {
        match receiver.try_recv() {
            Ok(Ok(CommandPipeMessage::Chunk(chunk))) => {
                if let Some(output_sink) = output_sink {
                    output_sink.output_chunk(ToolOutputChunk {
                        stream: stream.clone(),
                        text: String::from_utf8_lossy(&chunk).to_string(),
                    });
                }
                output.extend_from_slice(&chunk);
                if let Some(limit) = output_limit
                    && output.len() > limit
                {
                    return Err(ToolRuntimeError::CommandOutputTooLarge {
                        command: command.to_string(),
                        pid,
                        stream: stream.clone(),
                        bytes: output.len(),
                        max_bytes: limit,
                    });
                }
            }
            Ok(Ok(CommandPipeMessage::Complete)) => {
                *complete = true;
                return Ok(());
            }
            Ok(Err(source)) => {
                return Err(ToolRuntimeError::Command {
                    command: command.to_string(),
                    source,
                });
            }
            Err(mpsc::TryRecvError::Empty) => return Ok(()),
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err(ToolRuntimeError::Command {
                    command: command.to_string(),
                    source: io::Error::other(format!(
                        "{stream:?} reader thread exited without result for pid {pid}"
                    )),
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

pub(crate) fn limited_output_text(output: &[u8]) -> (String, bool) {
    let truncated = output.len() > MAX_COMMAND_OUTPUT_BYTES;
    let bytes = if truncated {
        &output[..MAX_COMMAND_OUTPUT_BYTES]
    } else {
        output
    };

    (String::from_utf8_lossy(bytes).to_string(), truncated)
}

impl From<WorkspaceDatabaseError> for ToolRuntimeError {
    fn from(source: WorkspaceDatabaseError) -> Self {
        Self::WorkspaceDatabase(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command_tools::RunCommandInput,
        file_tools::{EditFileInput, ReadFileInput, WriteFileInput, ripgrep_command},
    };
    use foco_store::workspace::{
        NewCodeGraphEdge, NewCodeGraphFileIndex, NewCodeGraphImport, NewCodeGraphReference,
        NewCodeGraphSymbol, WorkspaceDatabase,
    };
    use serde_json::json;
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
    fn finds_workspace_files() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("a.txt"), "a").expect("write a");
        fs::create_dir(workspace.path().join("nested")).expect("create nested");
        fs::write(workspace.path().join("nested").join("b.txt"), "b").expect("write b");

        let result =
            execute_builtin_tool(workspace.path(), FIND_FILES_TOOL, json!({ "path": "." }));

        assert!(!result.is_error);
        let entries = result.output["entries"].as_array().expect("entries");
        assert_eq!(entries[0]["path"], "a.txt");
        assert_eq!(entries[1]["path"], "nested");
        assert_eq!(entries[2]["path"], "nested/b.txt");
    }

    #[test]
    fn finds_workspace_files_with_glob_filters() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::create_dir(workspace.path().join("src")).expect("create src");
        fs::write(workspace.path().join("src").join("lib.rs"), "a").expect("write lib");
        fs::write(workspace.path().join("b.txt"), "b").expect("write b");
        fs::write(workspace.path().join("src").join("test.rs"), "test").expect("write test");

        let result = execute_builtin_tool(
            workspace.path(),
            FIND_FILES_TOOL,
            json!({
                "path": ".",
                "include": ["**/*.rs"],
                "exclude": ["**/test.rs"],
                "timeoutMs": null
            }),
        );

        assert!(!result.is_error);
        let entries = result.output["entries"].as_array().expect("entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["path"], "src/lib.rs");
    }

    #[test]
    fn rejects_missing_required_tool_arguments() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(workspace.path(), FIND_FILES_TOOL, json!({}));

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

        let graph_symbols: graph_tools::GraphFindSymbolsInput = parse_arguments(json!({
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

        let graph_lookup: graph_tools::GraphSymbolLookupInput = parse_arguments(json!({
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

        let edit_file: EditFileInput = parse_arguments(json!({
            "path": "note.txt",
            "oldStr": "hello",
            "newStr": "hi",
            "replaceAll": null,
            "timeoutMs": null
        }))
        .expect("edit file input");
        assert_eq!(edit_file.path, "note.txt");
        assert_eq!(edit_file.old_str, "hello");
        assert_eq!(edit_file.new_str, "hi");
        assert_eq!(edit_file.replace_all, None);
        assert_eq!(edit_file.timeout_ms, None);
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
    fn graph_explore_returns_symbol_source_snippets() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(
            workspace.path().join("lib.rs"),
            "fn public_api() {\n    helper();\n}\n\n// gap\n\nfn helper() {\n    println!(\"helper\");\n}\n",
        )
        .expect("write lib");
        insert_graph_fixture(workspace.path());

        let result = execute_builtin_tool(
            workspace.path(),
            GRAPH_EXPLORE_TOOL,
            json!({
                "query": "helper",
                "kind": "function",
                "path": "lib.rs",
                "limit": 5,
                "contextLines": 1,
                "timeoutMs": null
            }),
        );

        assert!(!result.is_error);
        let snippets = result.output["snippets"].as_array().expect("snippets");
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0]["symbol"]["name"], "helper");
        assert_eq!(snippets[0]["path"], "lib.rs");
        assert_eq!(snippets[0]["startLine"], 6);
        assert_eq!(snippets[0]["endLine"], 9);
        assert!(
            snippets[0]["content"]
                .as_str()
                .expect("content")
                .contains("7\tfn helper()")
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
        assert_eq!(result.output["linesAdded"], 1);
        assert_eq!(result.output["linesRemoved"], 0);
        assert_eq!(fs::read(&path).expect("read note bytes"), "你好".as_bytes());
    }

    #[test]
    fn reports_write_file_line_change_stats() {
        let workspace = tempfile::tempdir().expect("workspace");
        let path = workspace.path().join("note.txt");
        fs::write(&path, "one\ntwo\nthree\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            WRITE_FILE_TOOL,
            json!({
                "path": "note.txt",
                "content": "two\nfour\nfive",
                "startLine": 2,
                "endLine": 3,
                "timeoutMs": null
            }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["linesAdded"], 2);
        assert_eq!(result.output["linesRemoved"], 1);
    }

    #[test]
    fn edits_workspace_file_with_single_match() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\ntwo\nthree\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            EDIT_FILE_TOOL,
            json!({
                "path": "note.txt",
                "oldStr": "two",
                "newStr": "TWO",
                "replaceAll": null,
                "timeoutMs": null
            }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["replacements"], 1);
        assert_eq!(result.output["replaceAll"], false);
        assert_eq!(result.output["linesAdded"], 1);
        assert_eq!(result.output["linesRemoved"], 1);
        assert_eq!(
            fs::read_to_string(workspace.path().join("note.txt")).expect("read note"),
            "one\nTWO\nthree\n"
        );
    }

    #[test]
    fn rejects_edit_file_when_old_str_is_missing() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\ntwo\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            EDIT_FILE_TOOL,
            json!({
                "path": "note.txt",
                "oldStr": "three",
                "newStr": "THREE",
                "replaceAll": false,
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
                .contains("oldStr was not found")
        );
        assert_eq!(
            fs::read_to_string(workspace.path().join("note.txt")).expect("read note"),
            "one\ntwo\n"
        );
    }

    #[test]
    fn rejects_edit_file_when_old_str_matches_multiple_without_replace_all() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\ntwo\ntwo\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            EDIT_FILE_TOOL,
            json!({
                "path": "note.txt",
                "oldStr": "two",
                "newStr": "TWO",
                "replaceAll": false,
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
                .contains("oldStr matched 2 times")
        );
        assert_eq!(
            fs::read_to_string(workspace.path().join("note.txt")).expect("read note"),
            "one\ntwo\ntwo\n"
        );
    }

    #[test]
    fn edits_all_matches_when_replace_all_is_true() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "one\ntwo\ntwo\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            EDIT_FILE_TOOL,
            json!({
                "path": "note.txt",
                "oldStr": "two",
                "newStr": "TWO",
                "replaceAll": true,
                "timeoutMs": null
            }),
        );

        assert!(!result.is_error);
        assert_eq!(result.output["replacements"], 2);
        assert_eq!(result.output["replaceAll"], true);
        assert_eq!(
            fs::read_to_string(workspace.path().join("note.txt")).expect("read note"),
            "one\nTWO\nTWO\n"
        );
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
        let cancellation_token = ToolCancellationToken::default();
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
