mod agent_tools;
mod background_command;
mod command_tools;
mod definitions;
mod errors;
mod file_tools;
mod graph_tools;
pub mod output_budget;
mod plan_tools;
mod spec_patch;
mod spec_tools;
mod todo_tools;

pub use background_command::{
    BACKGROUND_COMMAND_RETENTION, BackgroundCommandError, BackgroundCommandLimits,
    BackgroundCommandOutput, BackgroundCommandOutputChunk, BackgroundCommandOutputStream,
    BackgroundCommandRegistry, BackgroundCommandRequest, BackgroundCommandSnapshot,
    BackgroundCommandStatus, BackgroundCommandTermination, MAX_BACKGROUND_COMMAND_OUTPUT_BYTES,
    MAX_BACKGROUND_COMMANDS_PER_WORKSPACE,
};
pub use spec_patch::{SpecPatchError, SpecTextEdit, apply_spec_text_edits};

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
pub const IMAGE_GEN_TOOL: &str = "image_gen";
pub const WRITE_FILE_TOOL: &str = "write_file";
pub const EDIT_FILE_TOOL: &str = "edit_file";
pub const RUN_COMMAND_TOOL: &str = "run_command";
pub const GET_COMMAND_OUTPUT_TOOL: &str = "get_command_output";
pub const STOP_COMMAND_TOOL: &str = "stop_command";
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
pub const CREATE_PLAN_TOOL: &str = "create_plan";
pub const GET_PLANS_TOOL: &str = "get_plans";
pub const UPDATE_PLAN_TOOL: &str = "update_plan";
pub const UPDATE_PLAN_STEP_TOOL: &str = "update_plan_step";
pub const DELETE_PLAN_TOOL: &str = "delete_plan";
pub const READ_SPEC_TOOL: &str = "read_spec";
pub const UPDATE_SPEC_TOOL: &str = "update_spec";
pub const ASK_QUESTION_TOOL: &str = "ask_question";
pub const AGENT_LIST_TOOL: &str = "agent_list";
pub const AGENT_GET_TASK_TOOL: &str = "agent_get_task";
pub const AGENT_SEND_MESSAGE_TOOL: &str = "agent_send_message";
pub const AGENT_DELEGATE_TASK_TOOL: &str = "agent_delegate_task";
pub const AGENT_CANCEL_TASK_TOOL: &str = "agent_cancel_task";
pub const AGENT_WAIT_TASKS_TOOL: &str = "agent_wait_tasks";
pub const AGENT_TRANSFER_TASK_TOOL: &str = "agent_transfer_task";
pub const AGENT_CREATE_INSTANCES_TOOL: &str = "agent_create_instances";

/// Full unscoped `read_file` will not load sources larger than this; use startLine/endLine instead.
const MAX_FULL_READ_BYTES: u64 = 128 * 1024;
const MAX_RANGED_READ_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
/// Hard safety cap on numbered `read_file` content before the shared 128 KiB envelope gate.
const MAX_RANGED_READ_OUTPUT_BYTES: usize = output_budget::TOOL_EXECUTION_HARD_BYTE_LIMIT;
const MAX_FIND_ENTRIES: usize = 200;
const MAX_SEARCH_TEXT_LINE_BYTES: usize = 4 * 1024;
/// Command-level rg stdout/stderr collection ceiling (not the model-facing soft preview).
const MAX_SEARCH_TEXT_FULL_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const SEARCH_RESULTS_DIR: &str = "search-results";
const MAX_SEARCH_RESULT_FILES: usize = 20;
const SEARCH_RESULT_TTL: Duration = Duration::from_secs(60 * 60);
const SEARCH_SNAPSHOT_VERSION: u32 = 1;
/// Reserved headroom inside soft byte budget for search_text metadata fields.
const SEARCH_TEXT_RESPONSE_OVERHEAD_BYTES: usize = 2 * 1024;
const FIND_FILES_RESPONSE_OVERHEAD_BYTES: usize = 1 * 1024;
const MAX_COMMAND_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_COMMAND_CAPTURE_BYTES_PER_STREAM: usize = output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT / 2;
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
const DEFAULT_IMAGE_GEN_TOOL_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_WRITE_FILE_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_SLEEP_TIMEOUT_MS: u64 = 600_000;
const DEFAULT_RUN_COMMAND_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_GET_COMMAND_OUTPUT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_TODO_GRAPH_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_PLAN_TOOL_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_SPEC_TOOL_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_AGENT_TOOL_TIMEOUT_MS: u64 = 10_000;
const MAX_TOOL_TIMEOUT_MS: u64 = 600_000;
const COMMAND_WAIT_POLL_MS: u64 = 25;
const MAX_COMMAND_PIPE_MESSAGES_PER_DRAIN: usize = 8;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
static RIPGREP_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
static BACKGROUND_COMMAND_REGISTRY: OnceLock<BackgroundCommandRegistry> = OnceLock::new();

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuiltinToolContext<'a> {
    pub chat_id: Option<&'a str>,
    pub session_mode: Option<&'a str>,
}

impl<'a> BuiltinToolContext<'a> {
    pub fn for_chat(chat_id: Option<&'a str>) -> Self {
        Self {
            chat_id,
            session_mode: None,
        }
    }

    pub fn is_plan_mode(self) -> bool {
        self.session_mode == Some("plan")
    }
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

/// Returns the host-local registry used by the built-in managed command tools.
pub fn background_command_registry() -> &'static BackgroundCommandRegistry {
    BACKGROUND_COMMAND_REGISTRY.get_or_init(BackgroundCommandRegistry::new)
}

pub fn execute_builtin_tool(
    workspace_path: &Path,
    tool_name: &str,
    arguments: Value,
) -> ToolExecution {
    execute_builtin_tool_with_context(
        workspace_path,
        BuiltinToolContext::default(),
        tool_name,
        arguments,
    )
}

pub fn execute_builtin_tool_for_chat(
    workspace_path: &Path,
    chat_id: Option<&str>,
    tool_name: &str,
    arguments: Value,
) -> ToolExecution {
    execute_builtin_tool_with_context(
        workspace_path,
        BuiltinToolContext::for_chat(chat_id),
        tool_name,
        arguments,
    )
}

pub fn execute_builtin_tool_with_context(
    workspace_path: &Path,
    context: BuiltinToolContext<'_>,
    tool_name: &str,
    arguments: Value,
) -> ToolExecution {
    execute_builtin_tool_with_context_and_cancellation(
        workspace_path,
        context,
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
    execute_builtin_tool_with_context_and_cancellation(
        workspace_path,
        BuiltinToolContext::for_chat(chat_id),
        tool_name,
        arguments,
        cancellation_token,
    )
}

pub fn execute_builtin_tool_with_context_and_cancellation(
    workspace_path: &Path,
    context: BuiltinToolContext<'_>,
    tool_name: &str,
    arguments: Value,
    cancellation_token: Option<ToolCancellationToken>,
) -> ToolExecution {
    execute_builtin_tool_with_context_cancellation_and_output_sink(
        workspace_path,
        context,
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
    execute_builtin_tool_with_context_cancellation_and_output_sink(
        workspace_path,
        BuiltinToolContext::for_chat(chat_id),
        tool_name,
        arguments,
        cancellation_token,
        output_sink,
    )
}

pub fn execute_builtin_tool_with_context_cancellation_and_output_sink(
    workspace_path: &Path,
    context: BuiltinToolContext<'_>,
    tool_name: &str,
    arguments: Value,
    cancellation_token: Option<ToolCancellationToken>,
    output_sink: Option<Arc<dyn ToolOutputSink>>,
) -> ToolExecution {
    execute_builtin_tool_with_context_and_options(
        workspace_path,
        context,
        tool_name,
        arguments,
        cancellation_token,
        output_sink,
        false,
    )
}

pub fn execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
    workspace_path: &Path,
    chat_id: Option<&str>,
    tool_name: &str,
    arguments: Value,
    cancellation_token: Option<ToolCancellationToken>,
    output_sink: Option<Arc<dyn ToolOutputSink>>,
    allow_external_read_access: bool,
) -> ToolExecution {
    execute_builtin_tool_with_context_and_options(
        workspace_path,
        BuiltinToolContext::for_chat(chat_id),
        tool_name,
        arguments,
        cancellation_token,
        output_sink,
        allow_external_read_access,
    )
}

pub fn execute_builtin_tool_with_context_and_options(
    workspace_path: &Path,
    context: BuiltinToolContext<'_>,
    tool_name: &str,
    arguments: Value,
    cancellation_token: Option<ToolCancellationToken>,
    output_sink: Option<Arc<dyn ToolOutputSink>>,
    allow_external_read_access: bool,
) -> ToolExecution {
    match execute_builtin_tool_inner(
        workspace_path,
        context,
        tool_name,
        arguments,
        cancellation_token.as_ref(),
        output_sink.as_deref(),
        allow_external_read_access,
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

pub fn read_file_target_outside_workspace(
    workspace_path: &Path,
    input: &str,
) -> Result<Option<PathBuf>, String> {
    file_tools::read_file_target_outside_workspace(workspace_path, input)
        .map_err(|error| error.to_string())
}

pub fn find_files_target_outside_workspace(
    workspace_path: &Path,
    input: &str,
) -> Result<Option<PathBuf>, String> {
    file_tools::find_files_target_outside_workspace(workspace_path, input)
        .map_err(|error| error.to_string())
}

pub fn search_text_target_outside_workspace(
    workspace_path: &Path,
    input: &str,
) -> Result<Option<PathBuf>, String> {
    file_tools::search_text_target_outside_workspace(workspace_path, input)
        .map_err(|error| error.to_string())
}

fn execute_builtin_tool_inner(
    workspace_path: &Path,
    context: BuiltinToolContext<'_>,
    tool_name: &str,
    arguments: Value,
    cancellation_token: Option<&ToolCancellationToken>,
    output_sink: Option<&dyn ToolOutputSink>,
    // Supported read-only tools only: `read_file` / `find_files` / `search_text` may parse
    // absolute paths outside the execution root when this flag is true (after app-layer
    // authorization). write/edit/run/graph never consume it and stay on execution-root resolvers.
    // Shared-workspace auto-trust for Plan worktrees lives in app authorization helpers, not here.
    allow_external_read_access: bool,
) -> Result<Value, ToolRuntimeError> {
    match tool_name {
        READ_FILE_TOOL if allow_external_read_access => {
            file_tools::read_file_with_external_access(workspace_path, arguments)
        }
        READ_FILE_TOOL => file_tools::read_file(workspace_path, arguments),
        FIND_FILES_TOOL if allow_external_read_access => {
            file_tools::find_files_with_external_access(workspace_path, arguments)
        }
        FIND_FILES_TOOL => file_tools::find_files(workspace_path, arguments),
        GRAPH_FIND_SYMBOLS_TOOL => graph_tools::graph_find_symbols(workspace_path, arguments),
        GRAPH_FIND_CALLERS_TOOL => graph_tools::graph_find_callers(workspace_path, arguments),
        GRAPH_FIND_CALLEES_TOOL => graph_tools::graph_find_callees(workspace_path, arguments),
        GRAPH_FIND_REFERENCES_TOOL => graph_tools::graph_find_references(workspace_path, arguments),
        GRAPH_RELATED_FILES_TOOL => graph_tools::graph_related_files(workspace_path, arguments),
        GRAPH_EXPLORE_TOOL => graph_tools::graph_explore(workspace_path, arguments),
        SEARCH_TEXT_TOOL if allow_external_read_access => {
            file_tools::search_text_with_external_access(
                workspace_path,
                arguments,
                cancellation_token,
            )
        }
        SEARCH_TEXT_TOOL => file_tools::search_text(workspace_path, arguments, cancellation_token),
        WEB_SEARCH_TOOL | WEB_FETCH_TOOL | IMAGE_GEN_TOOL => {
            Err(ToolRuntimeError::InvalidArguments(format!(
                "{tool_name} requires app runtime configuration"
            )))
        }
        WRITE_FILE_TOOL => file_tools::write_file(workspace_path, arguments),
        EDIT_FILE_TOOL => file_tools::edit_file(workspace_path, arguments),
        CREATE_TODO_GRAPH_TOOL => {
            todo_tools::create_todo_graph(workspace_path, context.chat_id, arguments)
        }
        UPDATE_TODO_GRAPH_TOOL => {
            todo_tools::update_todo_graph(workspace_path, context.chat_id, arguments)
        }
        GET_TODO_GRAPH_TOOL => {
            todo_tools::get_todo_graph(workspace_path, context.chat_id, arguments)
        }
        CREATE_PLAN_TOOL => plan_tools::create_plan(workspace_path, context, arguments),
        GET_PLANS_TOOL => plan_tools::get_plans(workspace_path, arguments),
        UPDATE_PLAN_TOOL => plan_tools::update_plan(workspace_path, context, arguments),
        UPDATE_PLAN_STEP_TOOL => plan_tools::update_plan_step(workspace_path, context, arguments),
        DELETE_PLAN_TOOL => plan_tools::delete_plan(workspace_path, context, arguments),
        READ_SPEC_TOOL => spec_tools::read_spec(workspace_path, arguments),
        UPDATE_SPEC_TOOL => spec_tools::update_spec(workspace_path, arguments),
        ASK_QUESTION_TOOL => Err(ToolRuntimeError::InvalidArguments(
            "ask_question must be executed through the chat UI question bridge".to_string(),
        )),
        RUN_COMMAND_TOOL => {
            command_tools::run_command(workspace_path, arguments, cancellation_token, output_sink)
        }
        GET_COMMAND_OUTPUT_TOOL => command_tools::get_command_output(workspace_path, arguments),
        STOP_COMMAND_TOOL => command_tools::stop_command(workspace_path, arguments),
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
    // Execution-root boundary only for tools that still use this resolver (write/edit/run/graph).
    // read_file / find_files / search_text use dedicated internal/external resolvers when authorized.
    // Do not auto-trust a separate shared workspace root here.
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
    let mut stdout_capture = CommandStreamCapture::default();
    let mut stderr_capture = CommandStreamCapture::default();
    let mut delta_budget =
        CommandDeltaBudget::new(output_limits.and_then(|limits| limits.output_delta_bytes));
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
            &mut stdout_capture,
            &mut stdout_complete,
            pid,
            output_sink,
            &mut delta_budget,
            output_limits.and_then(|limits| limits.stdout_bytes),
            output_limits.is_some_and(|limits| limits.truncate),
        ) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        if let Err(error) = drain_command_pipe(
            &command_label,
            &stderr_handle,
            ToolOutputStream::Stderr,
            &mut stderr_capture,
            &mut stderr_complete,
            pid,
            output_sink,
            &mut delta_budget,
            output_limits.and_then(|limits| limits.stderr_bytes),
            output_limits.is_some_and(|limits| limits.truncate),
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
                    stdout: stdout_capture.output,
                    stderr: stderr_capture.output,
                    stdout_bytes: stdout_capture.observed_bytes,
                    stderr_bytes: stderr_capture.observed_bytes,
                    stdout_truncated: stdout_capture.truncated,
                    stderr_truncated: stderr_capture.truncated,
                    output_delta_truncated: delta_budget.truncated,
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
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
    pub(crate) stdout_truncated: bool,
    pub(crate) stderr_truncated: bool,
    pub(crate) output_delta_truncated: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct CommandOutputLimits {
    pub(crate) stdout_bytes: Option<usize>,
    pub(crate) stderr_bytes: Option<usize>,
    pub(crate) output_delta_bytes: Option<usize>,
    pub(crate) truncate: bool,
}

#[derive(Default)]
struct CommandStreamCapture {
    output: Vec<u8>,
    observed_bytes: usize,
    truncated: bool,
}

struct CommandDeltaBudget {
    max_bytes: Option<usize>,
    emitted_bytes: usize,
    truncated: bool,
}

impl CommandDeltaBudget {
    fn new(max_bytes: Option<usize>) -> Self {
        Self {
            max_bytes,
            emitted_bytes: 0,
            truncated: false,
        }
    }
}

enum CommandPipeMessage {
    Chunk(Vec<u8>),
    Complete,
}

fn read_command_pipe<T>(mut pipe: T) -> mpsc::Receiver<io::Result<CommandPipeMessage>>
where
    T: Read + Send + 'static,
{
    let (tx, rx) = mpsc::sync_channel(8);
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

const COMMAND_OUTPUT_DELTA_TRUNCATION_NOTICE: &str =
    "\n[command output truncated: further stdout/stderr output omitted]\n";

fn drain_command_pipe(
    command: &str,
    receiver: &mpsc::Receiver<io::Result<CommandPipeMessage>>,
    stream: ToolOutputStream,
    capture: &mut CommandStreamCapture,
    complete: &mut bool,
    pid: u32,
    output_sink: Option<&dyn ToolOutputSink>,
    delta_budget: &mut CommandDeltaBudget,
    output_limit: Option<usize>,
    truncate_at_limit: bool,
) -> Result<(), ToolRuntimeError> {
    if *complete {
        return Ok(());
    }

    for _ in 0..MAX_COMMAND_PIPE_MESSAGES_PER_DRAIN {
        match receiver.try_recv() {
            Ok(Ok(CommandPipeMessage::Chunk(chunk))) => {
                capture.observed_bytes = capture.observed_bytes.saturating_add(chunk.len());
                let visible_chunk = if truncate_at_limit {
                    let remaining = output_limit
                        .map(|limit| limit.saturating_sub(capture.output.len()))
                        .unwrap_or(chunk.len());
                    if chunk.len() > remaining {
                        capture.truncated = true;
                    }
                    &chunk[..chunk.len().min(remaining)]
                } else {
                    chunk.as_slice()
                };
                capture.output.extend_from_slice(visible_chunk);
                emit_command_output_delta(output_sink, &stream, visible_chunk, delta_budget);
                if capture.truncated {
                    emit_command_output_truncation_notice(output_sink, &stream, delta_budget);
                }
                if !truncate_at_limit
                    && let Some(limit) = output_limit
                    && capture.output.len() > limit
                {
                    return Err(ToolRuntimeError::CommandOutputTooLarge {
                        command: command.to_string(),
                        pid,
                        stream: stream.clone(),
                        bytes: capture.output.len(),
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
    Ok(())
}

fn emit_command_output_delta(
    output_sink: Option<&dyn ToolOutputSink>,
    stream: &ToolOutputStream,
    chunk: &[u8],
    budget: &mut CommandDeltaBudget,
) {
    let Some(output_sink) = output_sink else {
        return;
    };
    if chunk.is_empty() || budget.truncated {
        return;
    }

    let text = String::from_utf8_lossy(chunk);
    let remaining = budget
        .max_bytes
        .map(|limit| limit.saturating_sub(budget.emitted_bytes))
        .unwrap_or(text.len());
    let notice_reserve = budget
        .max_bytes
        .map(|_| COMMAND_OUTPUT_DELTA_TRUNCATION_NOTICE.len().min(remaining))
        .unwrap_or(0);
    let content_budget = remaining.saturating_sub(notice_reserve);
    let (visible_text, truncated) = output_budget::bounded_utf8_prefix(&text, content_budget);
    if !visible_text.is_empty() {
        output_sink.output_chunk(ToolOutputChunk {
            stream: stream.clone(),
            text: visible_text.to_string(),
        });
        budget.emitted_bytes = budget.emitted_bytes.saturating_add(visible_text.len());
    }
    if truncated {
        emit_command_output_truncation_notice(Some(output_sink), stream, budget);
    }
}

fn emit_command_output_truncation_notice(
    output_sink: Option<&dyn ToolOutputSink>,
    stream: &ToolOutputStream,
    budget: &mut CommandDeltaBudget,
) {
    let Some(output_sink) = output_sink else {
        return;
    };
    if budget.truncated {
        return;
    }

    let remaining = budget
        .max_bytes
        .map(|limit| limit.saturating_sub(budget.emitted_bytes))
        .unwrap_or(COMMAND_OUTPUT_DELTA_TRUNCATION_NOTICE.len());
    let (notice, _) =
        output_budget::bounded_utf8_prefix(COMMAND_OUTPUT_DELTA_TRUNCATION_NOTICE, remaining);
    if !notice.is_empty() {
        output_sink.output_chunk(ToolOutputChunk {
            stream: stream.clone(),
            text: notice.to_string(),
        });
        budget.emitted_bytes = budget.emitted_bytes.saturating_add(notice.len());
    }
    budget.truncated = true;
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
        NewCodeGraphSymbol, NewPlan, NewPlanPhase, NewPlanStep, WORKSPACE_SPEC_MAX_MARKDOWN_BYTES,
        WorkspaceDatabase,
    };
    use serde_json::json;
    use std::{
        collections::BTreeSet,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct CountingOutputSink {
        bytes: Arc<AtomicUsize>,
        truncation_notices: Arc<AtomicUsize>,
    }

    impl ToolOutputSink for CountingOutputSink {
        fn output_chunk(&self, chunk: ToolOutputChunk) {
            self.bytes.fetch_add(chunk.text.len(), Ordering::Relaxed);
            if chunk.text.contains(COMMAND_OUTPUT_DELTA_TRUNCATION_NOTICE) {
                self.truncation_notices.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

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
    fn reads_external_file_only_with_explicit_access() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        let outside_path = outside.path().join("outside.txt");
        fs::write(&outside_path, "outside").expect("write outside");
        let relative_escape = format!(
            "../{}/outside.txt",
            outside
                .path()
                .file_name()
                .expect("outside dir name")
                .to_string_lossy()
        );

        let denied = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": relative_escape, "startLine": null, "endLine": null }),
        );
        assert!(denied.is_error);

        let allowed = execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
            workspace.path(),
            Some("chat-external-read-test"),
            READ_FILE_TOOL,
            json!({ "path": outside_path.to_string_lossy(), "startLine": null, "endLine": null }),
            None,
            None,
            true,
        );
        assert!(!allowed.is_error);
        assert_eq!(allowed.output["content"], "1\toutside");
    }

    #[test]
    fn detects_read_file_target_outside_workspace() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        let outside_path = outside.path().join("outside.txt");
        fs::write(&outside_path, "outside").expect("write outside");
        fs::write(workspace.path().join("inside.txt"), "inside").expect("write inside");

        assert!(
            read_file_target_outside_workspace(workspace.path(), outside_path.to_str().unwrap())
                .expect("outside path")
                .is_some()
        );
        assert_eq!(
            read_file_target_outside_workspace(workspace.path(), "inside.txt")
                .expect("inside path"),
            None
        );

        let inside_absolute =
            fs::canonicalize(workspace.path().join("inside.txt")).expect("canonicalize inside");
        assert_eq!(
            read_file_target_outside_workspace(
                workspace.path(),
                inside_absolute.to_str().expect("utf8 path")
            )
            .expect("absolute inside path"),
            None
        );
    }

    #[test]
    fn reads_workspace_file_via_absolute_path_without_external_access() {
        let workspace = tempfile::tempdir().expect("workspace");
        let note_path = workspace.path().join("note.txt");
        fs::write(&note_path, "hello absolute").expect("write note");
        let absolute = fs::canonicalize(&note_path).expect("canonicalize note");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({
                "path": absolute.to_string_lossy(),
                "startLine": null,
                "endLine": null
            }),
        );

        assert!(
            !result.is_error,
            "absolute internal path should read without external access: {:?}",
            result.output
        );
        assert_eq!(result.output["content"], "1\thello absolute");
        assert_eq!(result.output["path"], absolute.to_string_lossy().as_ref());
    }

    #[test]
    fn absolute_internal_read_file_path_does_not_require_external_access_flag() {
        let workspace = tempfile::tempdir().expect("workspace");
        let note_path = workspace.path().join("note.txt");
        fs::write(&note_path, "flag free").expect("write note");
        let absolute = fs::canonicalize(&note_path).expect("canonicalize note");

        let without_flag = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({
                "path": absolute.to_string_lossy(),
                "startLine": null,
                "endLine": null
            }),
        );
        let with_flag =
            execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
                workspace.path(),
                Some("chat-absolute-internal"),
                READ_FILE_TOOL,
                json!({
                    "path": absolute.to_string_lossy(),
                    "startLine": null,
                    "endLine": null
                }),
                None,
                None,
                true,
            );

        assert!(!without_flag.is_error, "{:?}", without_flag.output);
        assert!(!with_flag.is_error, "{:?}", with_flag.output);
        assert_eq!(without_flag.output["content"], "1\tflag free");
        assert_eq!(with_flag.output["content"], "1\tflag free");
    }

    #[test]
    fn absolute_path_outside_workspace_still_requires_external_access() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        let outside_path = outside.path().join("secret.txt");
        fs::write(&outside_path, "secret").expect("write outside");
        let absolute = fs::canonicalize(&outside_path).expect("canonicalize outside");

        let denied = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({
                "path": absolute.to_string_lossy(),
                "startLine": null,
                "endLine": null
            }),
        );
        assert!(denied.is_error);
        let denied_error = denied
            .output
            .get("error")
            .and_then(Value::as_str)
            .expect("error");
        assert!(
            denied_error.contains("escapes the workspace")
                || denied_error.contains("path must be relative"),
            "{denied_error}"
        );

        let allowed =
            execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
                workspace.path(),
                Some("chat-absolute-external"),
                READ_FILE_TOOL,
                json!({
                    "path": absolute.to_string_lossy(),
                    "startLine": null,
                    "endLine": null
                }),
                None,
                None,
                true,
            );
        assert!(!allowed.is_error, "{:?}", allowed.output);
        assert_eq!(allowed.output["content"], "1\tsecret");
    }

    #[cfg(unix)]
    #[test]
    fn absolute_workspace_symlink_escape_is_not_internal() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        let outside_file = outside.path().join("escaped.txt");
        fs::write(&outside_file, "escaped").expect("write outside");
        let link_path = workspace.path().join("link.txt");
        symlink(&outside_file, &link_path).expect("create symlink");

        // Absolute path that points at the symlink entry under the workspace; canonicalize
        // follows the link, so the real target is outside and must not count as internal.
        let absolute_under_workspace = fs::canonicalize(workspace.path())
            .expect("canon workspace")
            .join("link.txt");

        let denied = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({
                "path": absolute_under_workspace.to_string_lossy(),
                "startLine": null,
                "endLine": null
            }),
        );
        assert!(
            denied.is_error,
            "symlink escape must not count as internal: {:?}",
            denied.output
        );

        assert!(
            read_file_target_outside_workspace(
                workspace.path(),
                absolute_under_workspace.to_str().expect("utf8")
            )
            .expect("classify")
            .is_some(),
            "symlink escape should classify as external"
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
        let error = result
            .output
            .get("error")
            .and_then(Value::as_str)
            .expect("error");
        assert!(error.contains("too large to read"), "{error}");
        assert!(
            error.contains(&format!("max {MAX_FULL_READ_BYTES}")),
            "{error}"
        );
        assert!(error.contains("startLine/endLine"), "{error}");
        assert!(error.contains("large.txt"), "{error}");
    }

    #[test]
    fn rejects_full_file_read_over_soft_budget_with_suggested_range() {
        let workspace = tempfile::tempdir().expect("workspace");
        // Under the 128 KiB full-read source cap, but over the 50 KiB soft numbered output budget.
        let content = format!("{}\n", "y".repeat(60 * 1024));
        assert!(content.len() < MAX_FULL_READ_BYTES as usize);
        fs::write(workspace.path().join("soft.txt"), content).expect("write soft file");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "soft.txt", "startLine": null, "endLine": null }),
        );

        assert!(result.is_error);
        let error = result
            .output
            .get("error")
            .and_then(Value::as_str)
            .expect("error");
        assert!(error.contains("soft output budget"), "{error}");
        assert!(error.contains("startLine="), "{error}");
        assert!(error.contains("endLine="), "{error}");
        assert!(error.contains("soft.txt"), "{error}");
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
        let error = result
            .output
            .get("error")
            .and_then(Value::as_str)
            .expect("error");
        assert!(
            error.contains("too large") || error.contains("hard max"),
            "{error}"
        );
        assert!(
            error.contains(&format!("{MAX_RANGED_READ_OUTPUT_BYTES}"))
                || error.contains("soft output budget"),
            "{error}"
        );
        assert!(error.contains("startLine"), "{error}");
        assert!(error.contains("large-line.txt"), "{error}");
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
    fn get_plans_schema_limits_page_size_and_limit_to_ten() {
        let definition = builtin_tool_definitions()
            .into_iter()
            .find(|definition| definition.name == GET_PLANS_TOOL)
            .expect("get_plans definition");
        let properties = definition.input_schema["properties"]
            .as_object()
            .expect("get_plans properties");

        for property_name in ["pageSize", "limit"] {
            let property = &properties[property_name];
            assert_eq!(property["minimum"], 1);
            assert_eq!(property["maximum"], 10);
            let description = property["description"]
                .as_str()
                .expect("pagination description");
            assert!(description.contains("1 to 10"));
            assert!(description.contains("defaults to 10"));
        }
    }

    #[test]
    fn read_file_schema_documents_128kib_output_limits() {
        let definition = builtin_tool_definitions()
            .into_iter()
            .find(|definition| definition.name == READ_FILE_TOOL)
            .expect("read_file definition");
        assert!(
            definition.description.contains("128KiB"),
            "{}",
            definition.description
        );
        assert!(
            definition.description.contains("50KiB") || definition.description.contains("soft"),
            "{}",
            definition.description
        );
        assert!(
            definition.description.contains("fail") || definition.description.contains("fails"),
            "{}",
            definition.description
        );
        let path_description = definition.input_schema["properties"]["path"]["description"]
            .as_str()
            .expect("path description");
        assert!(
            path_description.contains("absolute"),
            "path schema should document absolute path support: {path_description}"
        );
        assert!(
            path_description.contains("Workspace-relative")
                || path_description.contains("workspace-relative"),
            "path schema should still document relative paths: {path_description}"
        );
        assert!(
            path_description.contains("write_file")
                || path_description.contains("graph")
                || path_description.contains("do not accept absolute"),
            "path schema must not imply write/graph tools accept absolute external paths: {path_description}"
        );
        let start_description = definition.input_schema["properties"]["startLine"]["description"]
            .as_str()
            .expect("startLine description");
        let end_description = definition.input_schema["properties"]["endLine"]["description"]
            .as_str()
            .expect("endLine description");
        assert!(
            start_description.contains("50KiB") || start_description.contains("128KiB"),
            "{start_description}"
        );
        assert!(
            end_description.contains("50KiB") || end_description.contains("soft"),
            "{end_description}"
        );
    }

    #[test]
    fn read_file_output_limits_are_128kib() {
        assert_eq!(MAX_FULL_READ_BYTES, 128 * 1024);
        assert_eq!(MAX_RANGED_READ_OUTPUT_BYTES, 128 * 1024);
        assert_eq!(MAX_RANGED_READ_SOURCE_BYTES, 32 * 1024 * 1024);
    }

    #[test]
    fn read_file_rejects_line_range_for_skill_md() {
        let workspace = tempfile::tempdir().expect("workspace");
        let skill_dir = workspace.path().join(".agents").join("skills").join("demo");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: demo\ndescription: demo skill\n---\n\n# Demo\n\nline1\nline2\nline3\n",
        )
        .expect("skill");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({
                "path": ".agents/skills/demo/SKILL.md",
                "startLine": 1,
                "endLine": 2,
            }),
        );
        assert!(result.is_error);
        let message = result.output["error"].as_str().unwrap_or_default();
        assert!(
            message.contains("SKILL.md") && message.contains("full"),
            "{message}"
        );
    }

    #[test]
    fn read_file_rejects_oversized_skill_md() {
        let workspace = tempfile::tempdir().expect("workspace");
        let skill_dir = workspace.path().join(".agents").join("skills").join("huge");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        let header = "---\nname: huge\ndescription: huge\n---\n\n";
        let body_len = output_budget::SKILL_MD_MAX_BYTES - header.len() + 1;
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("{header}{}", "x".repeat(body_len)),
        )
        .expect("skill");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({
                "path": ".agents/skills/huge/SKILL.md",
                "startLine": null,
                "endLine": null,
            }),
        );
        assert!(result.is_error);
        let message = result.output["error"].as_str().unwrap_or_default();
        assert!(message.contains("SKILL.md"), "{message}");
        assert!(
            message.contains("exceeds") || message.contains("maximum"),
            "{message}"
        );
    }

    #[test]
    fn read_file_returns_full_skill_md_between_soft_and_hard_limits() {
        let workspace = tempfile::tempdir().expect("workspace");
        let skill_dir = workspace.path().join(".agents").join("skills").join("mid");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        // 56 KiB is above the 50 KiB soft tool budget but under the 64 KiB skill hard limit.
        let target = 56 * 1024;
        let header = "---\nname: mid\ndescription: mid size\n---\n\n";
        let body_len = target - header.len();
        let content = format!("{header}{}", "m".repeat(body_len));
        assert!(content.len() > output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT);
        assert!(content.len() <= output_budget::SKILL_MD_MAX_BYTES);
        fs::write(skill_dir.join("SKILL.md"), &content).expect("skill");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({
                "path": ".agents/skills/mid/SKILL.md",
                "startLine": null,
                "endLine": null,
            }),
        );
        assert!(!result.is_error, "{:?}", result.output);
        let body = result.output["content"].as_str().expect("content");
        assert!(body.contains("name: mid"));
        // Numbered content still contains the full body payload.
        assert!(body.contains(&"m".repeat(32)));
        assert_eq!(result.output["bytes"], content.len() as u64);
    }

    #[test]
    fn read_file_allows_ranged_read_for_skill_reference_files() {
        let workspace = tempfile::tempdir().expect("workspace");
        let skill_dir = workspace.path().join(".agents").join("skills").join("demo");
        let references = skill_dir.join("references");
        fs::create_dir_all(&references).expect("references");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: demo\ndescription: demo\n---\n\nSee references.\n",
        )
        .expect("skill");
        let large = (1..=50)
            .map(|line| format!("line-{line}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(references.join("large.md"), &large).expect("reference");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({
                "path": ".agents/skills/demo/references/large.md",
                "startLine": 2,
                "endLine": 4,
            }),
        );
        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["startLine"], 2);
        assert_eq!(result.output["endLine"], 4);
        let body = result.output["content"].as_str().expect("content");
        assert!(body.contains("line-2"));
        assert!(body.contains("line-4"));
        assert!(!body.contains("line-50"));
    }

    #[test]
    fn read_file_schema_documents_skill_md_full_read_rules() {
        let definition = builtin_tool_definitions()
            .into_iter()
            .find(|definition| definition.name == READ_FILE_TOOL)
            .expect("read_file definition");
        assert!(
            definition.description.contains("SKILL.md"),
            "{}",
            definition.description
        );
        let start_description = definition.input_schema["properties"]["startLine"]["description"]
            .as_str()
            .expect("startLine description");
        assert!(
            start_description.contains("SKILL.md"),
            "{start_description}"
        );
    }

    #[test]
    fn strict_tool_schemas_require_every_property() {
        for tool in builtin_tool_definitions() {
            if tool.strict {
                assert_strict_required_matches_properties(tool.name, &tool.input_schema);
            }
        }
    }

    fn assert_strict_required_matches_properties(path: &str, schema: &Value) {
        let Some(schema_object) = schema.as_object() else {
            return;
        };

        if let Some(properties_value) = schema_object.get("properties") {
            assert_eq!(
                schema_object.get("additionalProperties"),
                Some(&Value::Bool(false)),
                "{path} object schema must reject unknown properties"
            );
            let properties = properties_value.as_object().expect("properties object");
            let required = schema_object
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
                "{path} required keys must match object properties"
            );

            for (name, value) in properties {
                assert_strict_required_matches_properties(
                    &format!("{path}.properties.{name}"),
                    value,
                );
            }
        }

        if let Some(items) = schema_object.get("items") {
            assert_strict_required_matches_properties(&format!("{path}.items"), items);
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
    fn get_plans_limits_results_to_ten_and_prefers_page_size_over_limit() {
        let workspace = tempfile::tempdir().expect("workspace");
        for index in 0..12 {
            insert_test_plan(workspace.path(), &format!("plan-{index:02}"), None);
        }

        let default_page = execute_builtin_tool(
            workspace.path(),
            GET_PLANS_TOOL,
            json!({
                "view": "active",
                "status": null,
                "page": null,
                "pageSize": null,
                "limit": null,
                "timeoutMs": null
            }),
        );
        assert!(!default_page.is_error, "{:?}", default_page.output);
        assert_eq!(default_page.output["pageSize"], 10);
        assert_eq!(default_page.output["plans"].as_array().unwrap().len(), 10);
        assert_eq!(default_page.output["plans"][0]["id"], "plan-11");

        for pagination in [
            json!({ "pageSize": 100, "limit": null }),
            json!({ "pageSize": null, "limit": 100 }),
        ] {
            let result = execute_builtin_tool(
                workspace.path(),
                GET_PLANS_TOOL,
                json!({
                    "view": "active",
                    "status": null,
                    "page": 1,
                    "pageSize": pagination["pageSize"],
                    "limit": pagination["limit"],
                    "timeoutMs": null
                }),
            );
            assert!(!result.is_error, "{:?}", result.output);
            assert_eq!(result.output["pageSize"], 10);
            assert_eq!(result.output["plans"].as_array().unwrap().len(), 10);
        }

        let page_size_wins = execute_builtin_tool(
            workspace.path(),
            GET_PLANS_TOOL,
            json!({
                "view": "active",
                "status": null,
                "page": 1,
                "pageSize": 3,
                "limit": 9,
                "timeoutMs": null
            }),
        );
        assert!(!page_size_wins.is_error, "{:?}", page_size_wins.output);
        assert_eq!(page_size_wins.output["pageSize"], 3);
        assert_eq!(page_size_wins.output["plans"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn generic_update_plan_tool_cannot_set_execution_status() {
        let workspace = tempfile::tempdir().expect("workspace");
        insert_test_plan(workspace.path(), "plan-tool-status-guard", None);

        let result = execute_builtin_tool(
            workspace.path(),
            UPDATE_PLAN_TOOL,
            json!({
                "planId": "plan-tool-status-guard",
                "title": null,
                "overview": null,
                "status": "implemented",
                "errorMessage": null,
                "timeoutMs": null
            }),
        );

        assert!(result.is_error);
        assert_error_contains(&result, "cannot be changed");
        let database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
        assert_eq!(
            database
                .plan("plan-tool-status-guard")
                .expect("plan lookup")
                .expect("plan")
                .status,
            "ready"
        );
    }

    #[test]
    fn plan_mode_rejects_plan_status_mutations_but_allows_content_updates() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
        database
            .insert_chat("chat-plan", "Plan mode chat")
            .expect("chat insert");
        drop(database);
        let context = BuiltinToolContext {
            chat_id: Some("chat-plan"),
            session_mode: Some("plan"),
        };

        for status in ["running", "failed", "implemented"] {
            let invalid_create = execute_builtin_tool_with_context(
                workspace.path(),
                context,
                CREATE_PLAN_TOOL,
                plan_tool_create_input(&format!("plan-{status}"), Some(status), Some("other-chat")),
            );
            assert!(invalid_create.is_error, "{status} should be rejected");
            assert_error_contains(&invalid_create, "Plan Mode cannot modify");
        }

        let omitted_status = execute_builtin_tool_with_context(
            workspace.path(),
            context,
            CREATE_PLAN_TOOL,
            plan_tool_create_input("plan-omitted", None, Some("other-chat")),
        );
        assert!(!omitted_status.is_error, "{:?}", omitted_status.output);
        assert_eq!(omitted_status.output["plan"]["status"], "ready");
        assert_eq!(omitted_status.output["plan"]["sourceChatId"], "chat-plan");

        let ready_status = execute_builtin_tool_with_context(
            workspace.path(),
            context,
            CREATE_PLAN_TOOL,
            plan_tool_create_input("plan-ready", Some("ready"), Some("other-chat")),
        );
        assert!(!ready_status.is_error, "{:?}", ready_status.output);

        let update_status = execute_builtin_tool_with_context(
            workspace.path(),
            context,
            UPDATE_PLAN_TOOL,
            json!({
                "planId": "plan-ready",
                "title": null,
                "overview": null,
                "status": "failed",
                "errorMessage": null,
                "timeoutMs": null
            }),
        );
        assert!(update_status.is_error);
        assert_error_contains(&update_status, "Plan Mode cannot modify");

        let update_error = execute_builtin_tool_with_context(
            workspace.path(),
            context,
            UPDATE_PLAN_TOOL,
            json!({
                "planId": "plan-ready",
                "title": null,
                "overview": null,
                "status": null,
                "errorMessage": "not from Plan Mode",
                "timeoutMs": null
            }),
        );
        assert!(update_error.is_error);
        assert_error_contains(&update_error, "Plan Mode cannot modify");

        let update_title = execute_builtin_tool_with_context(
            workspace.path(),
            context,
            UPDATE_PLAN_TOOL,
            json!({
                "planId": "plan-ready",
                "title": "Updated title",
                "overview": "Updated overview",
                "status": null,
                "errorMessage": null,
                "timeoutMs": null
            }),
        );
        assert!(!update_title.is_error, "{:?}", update_title.output);
        assert_eq!(update_title.output["plan"]["title"], "Updated title");
        assert_eq!(update_title.output["plan"]["overview"], "Updated overview");
        assert_eq!(update_title.output["plan"]["status"], "ready");

        let update_step_status = execute_builtin_tool_with_context(
            workspace.path(),
            context,
            UPDATE_PLAN_STEP_TOOL,
            json!({
                "planId": "plan-ready",
                "stepId": "step-plan-ready",
                "title": null,
                "detail": null,
                "acceptance": null,
                "status": "completed",
                "timeoutMs": null
            }),
        );
        assert!(update_step_status.is_error);
        assert_error_contains(&update_step_status, "Plan Mode cannot modify");

        let update_step_detail = execute_builtin_tool_with_context(
            workspace.path(),
            context,
            UPDATE_PLAN_STEP_TOOL,
            json!({
                "planId": "plan-ready",
                "stepId": "step-plan-ready",
                "title": "Updated step",
                "detail": "Updated detail",
                "acceptance": ["Updated acceptance"],
                "status": null,
                "timeoutMs": null
            }),
        );
        assert!(
            !update_step_detail.is_error,
            "{:?}",
            update_step_detail.output
        );
        let step = &update_step_detail.output["plan"]["phases"][0]["steps"][0];
        assert_eq!(step["title"], "Updated step");
        assert_eq!(step["detail"], "Updated detail");
        assert_eq!(step["acceptance"], json!(["Updated acceptance"]));
        assert_eq!(step["status"], "pending");
    }

    #[test]
    fn delete_plan_requires_current_chat_ownership() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("workspace database");
        database
            .insert_chat("chat-owner", "Owner chat")
            .expect("owner chat insert");
        database
            .insert_chat("chat-other", "Other chat")
            .expect("other chat insert");
        drop(database);

        let owner_context = BuiltinToolContext {
            chat_id: Some("chat-owner"),
            session_mode: Some("plan"),
        };
        let other_context = BuiltinToolContext {
            chat_id: Some("chat-other"),
            session_mode: Some("plan"),
        };

        let create_owned = execute_builtin_tool_with_context(
            workspace.path(),
            owner_context,
            CREATE_PLAN_TOOL,
            plan_tool_create_input("plan-owned", None, Some("chat-other")),
        );
        assert!(!create_owned.is_error, "{:?}", create_owned.output);

        let denied = execute_builtin_tool_with_context(
            workspace.path(),
            other_context,
            DELETE_PLAN_TOOL,
            json!({ "planId": "plan-owned", "timeoutMs": null }),
        );
        assert!(denied.is_error);
        assert_error_contains(&denied, "current chat");
        assert_plan_exists(workspace.path(), "plan-owned");

        insert_test_plan(workspace.path(), "plan-historical", None);
        let denied_historical = execute_builtin_tool_with_context(
            workspace.path(),
            owner_context,
            DELETE_PLAN_TOOL,
            json!({ "planId": "plan-historical", "timeoutMs": null }),
        );
        assert!(denied_historical.is_error);
        assert_error_contains(&denied_historical, "current chat");
        assert_plan_exists(workspace.path(), "plan-historical");

        let missing_chat = execute_builtin_tool_with_context(
            workspace.path(),
            BuiltinToolContext::default(),
            DELETE_PLAN_TOOL,
            json!({ "planId": "plan-owned", "timeoutMs": null }),
        );
        assert!(missing_chat.is_error);
        assert_error_contains(&missing_chat, "current chat id");
        assert_plan_exists(workspace.path(), "plan-owned");

        let deleted = execute_builtin_tool_with_context(
            workspace.path(),
            owner_context,
            DELETE_PLAN_TOOL,
            json!({ "planId": "plan-owned", "timeoutMs": null }),
        );
        assert!(!deleted.is_error, "{:?}", deleted.output);
        assert_eq!(deleted.output["deleted"], true);
        assert_eq!(deleted.output["planId"], "plan-owned");
        assert_plan_missing(workspace.path(), "plan-owned");

        let missing = execute_builtin_tool_with_context(
            workspace.path(),
            owner_context,
            DELETE_PLAN_TOOL,
            json!({ "planId": "plan-missing", "timeoutMs": null }),
        );
        assert!(missing.is_error);
        assert_error_contains(&missing, "plan was not found");
    }

    fn insert_test_plan(workspace_path: &Path, plan_id: &str, source_chat_id: Option<&str>) {
        let mut database = WorkspaceDatabase::open_or_create(workspace_path).expect("database");
        database
            .create_plan(NewPlan {
                id: plan_id,
                title: "Historical plan",
                overview: "Plan inserted directly for ownership checks.",
                status: "ready",
                source_chat_id,
                phases: vec![NewPlanPhase {
                    id: &format!("phase-{plan_id}"),
                    title: "Phase 1",
                    summary: "",
                    steps: vec![NewPlanStep {
                        id: &format!("step-{plan_id}"),
                        title: "Step 1",
                        detail: "",
                        acceptance: Vec::new(),
                    }],
                }],
            })
            .expect("insert test plan");
    }

    fn assert_plan_exists(workspace_path: &Path, plan_id: &str) {
        assert!(
            WorkspaceDatabase::open_or_create(workspace_path)
                .expect("database")
                .plan(plan_id)
                .expect("plan query")
                .is_some()
        );
    }

    fn assert_plan_missing(workspace_path: &Path, plan_id: &str) {
        assert!(
            WorkspaceDatabase::open_or_create(workspace_path)
                .expect("database")
                .plan(plan_id)
                .expect("plan query")
                .is_none()
        );
    }

    fn plan_tool_create_input(
        plan_id: &str,
        status: Option<&str>,
        source_chat_id: Option<&str>,
    ) -> Value {
        let phase_id = format!("phase-{plan_id}");
        let step_id = format!("step-{plan_id}");
        json!({
            "id": plan_id,
            "title": format!("Plan {plan_id}"),
            "overview": "Test plan",
            "status": status,
            "sourceChatId": source_chat_id,
            "phases": [{
                "id": phase_id,
                "title": "Phase 1",
                "summary": null,
                "steps": [{
                    "id": step_id,
                    "title": "Step 1",
                    "detail": null,
                    "acceptance": []
                }]
            }],
            "timeoutMs": null
        })
    }

    fn assert_error_contains(execution: &ToolExecution, expected: &str) {
        let error = execution
            .output
            .get("error")
            .and_then(Value::as_str)
            .expect("error message");
        assert!(error.contains(expected), "{error}");
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
            json!({ "query": "beta", "path": ".", "continuation": null, "timeoutMs": null }),
        );

        assert!(!result.is_error);
        let matches = result.output["matches"].as_array().expect("matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["path"], "note.txt");
        assert_eq!(matches[0]["line"], 2);
        assert_eq!(matches[0]["text"], "beta");
    }

    #[test]
    fn search_text_treats_missing_null_empty_and_blank_continuation_as_initial_search() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "alpha\nbeta\n").expect("write note");

        let with_null = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({ "query": "beta", "path": ".", "continuation": null, "timeoutMs": null }),
        );
        let with_empty = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({ "query": "beta", "path": ".", "continuation": "", "timeoutMs": null }),
        );
        let with_whitespace = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({ "query": "beta", "path": ".", "continuation": "  \t  ", "timeoutMs": null }),
        );
        // Field omitted: serde default leaves continuation as None (fresh search).
        let missing_field = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({ "query": "beta", "path": ".", "timeoutMs": null }),
        );

        for (label, result) in [
            ("null", &with_null),
            ("empty", &with_empty),
            ("whitespace", &with_whitespace),
            ("missing", &missing_field),
        ] {
            assert!(
                !result.is_error,
                "{label} continuation must start a fresh search, got {:?}",
                result.output
            );
            let error = result.output.get("error").and_then(Value::as_str);
            if let Some(error) = error {
                assert!(
                    !(error.contains("continuation") && error.contains("invalid")),
                    "{label} must not report a continuation format error: {error}"
                );
            }
            let matches = result.output["matches"].as_array().expect("matches");
            assert_eq!(matches.len(), 1, "{label}");
            assert_eq!(matches[0]["path"], "note.txt", "{label}");
            assert_eq!(matches[0]["line"], 2, "{label}");
            assert_eq!(matches[0]["text"], "beta", "{label}");
            assert_eq!(
                result.output["totalMatches"].as_u64().expect("total"),
                1,
                "{label}"
            );
            assert_eq!(result.output["truncated"], false, "{label}");
        }

        // Equivalence: all four inputs produce the same first-page payload shape.
        assert_eq!(with_null.output["matches"], with_empty.output["matches"]);
        assert_eq!(
            with_null.output["matches"],
            with_whitespace.output["matches"]
        );
        assert_eq!(with_null.output["matches"], missing_field.output["matches"]);
        assert_eq!(
            with_null.output["totalMatches"],
            with_empty.output["totalMatches"]
        );
        assert_eq!(
            with_null.output["totalMatches"],
            with_whitespace.output["totalMatches"]
        );
        assert_eq!(
            with_null.output["totalMatches"],
            missing_field.output["totalMatches"]
        );
    }

    #[test]
    fn search_text_truncates_large_results_to_a_workspace_file() {
        let workspace = tempfile::tempdir().expect("workspace");
        // Enough long matches that soft 50 KiB budget forces a multi-page snapshot.
        let total = 400;
        let mut content = String::new();
        for index in 0..total {
            content.push_str(&format!("needle {index} {}\n", "x".repeat(200)));
        }
        fs::write(workspace.path().join("big.txt"), content).expect("write big");

        let result = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({ "query": "needle", "path": ".", "continuation": null, "timeoutMs": null }),
        );

        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["truncated"], true);
        assert_eq!(
            result.output["totalMatches"].as_u64().expect("total"),
            total as u64
        );
        let returned = result.output["matches"].as_array().expect("matches").len();
        assert!(returned > 0);
        assert!(returned < total);

        let continuation = result.output["continuation"]
            .as_str()
            .expect("continuation");
        assert!(continuation.contains(':'));

        let full_path = result.output["fullResultPath"]
            .as_str()
            .expect("full result path");
        assert!(full_path.starts_with(".foco/search-results/"));
        assert!(full_path.ends_with(".txt"));

        // Continuation pages the same snapshot without re-running a broad search.
        let page_two = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "needle",
                "path": ".",
                "continuation": continuation,
                "timeoutMs": null
            }),
        );
        assert!(!page_two.is_error, "{:?}", page_two.output);
        assert_eq!(
            page_two.output["totalMatches"].as_u64().expect("total"),
            total as u64
        );
        let page_two_matches = page_two.output["matches"].as_array().expect("matches");
        assert!(!page_two_matches.is_empty());
        // First page and second page must not repeat the first match text.
        let first_page_first = result.output["matches"][0]["text"]
            .as_str()
            .expect("first match");
        let second_page_first = page_two_matches[0]["text"].as_str().expect("second");
        assert_ne!(first_page_first, second_page_first);

        // Mismatched query/path binding fails closed.
        let mismatch = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "other",
                "path": ".",
                "continuation": continuation,
                "timeoutMs": null
            }),
        );
        assert!(mismatch.is_error);
        let error = mismatch
            .output
            .get("error")
            .and_then(Value::as_str)
            .expect("error");
        assert!(
            error.contains("continuation") && error.contains("invalid"),
            "{error}"
        );

        // The model can read the complete results back through ranged read_file.
        let read_back = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": full_path, "startLine": 1, "endLine": 20 }),
        );

        assert!(!read_back.is_error, "{:?}", read_back.output);
        let read_content = read_back.output["content"].as_str().expect("content");
        assert!(read_content.contains("needle"));
    }

    #[test]
    fn search_text_rejects_expired_or_missing_continuation() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "needle\n").expect("write");

        let missing = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "needle",
                "path": ".",
                "continuation": "search-missing-1:0",
                "timeoutMs": null
            }),
        );
        assert!(missing.is_error);
        let error = missing
            .output
            .get("error")
            .and_then(Value::as_str)
            .expect("error");
        assert!(
            error.contains("expired") || error.contains("missing"),
            "{error}"
        );
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

    /// Path-tool isolation under a nested worktree execution root: write/edit absolute shared
    /// paths and parent escapes stay rejected even when `allow_external_read_access` is true.
    /// That flag only enables authorized external roots for read_file / find_files / search_text.
    #[test]
    fn nested_worktree_path_tools_stay_on_execution_root_despite_external_read_flag() {
        let shared = tempfile::tempdir().expect("shared");
        let worktree = shared
            .path()
            .join(".foco")
            .join("agent-worktrees")
            .join("iso-wt");
        fs::create_dir_all(&worktree).expect("worktree");
        let shared_file = shared.path().join("secret.txt");
        fs::write(&shared_file, "shared secret").expect("write shared");
        fs::write(worktree.join("local.txt"), "local").expect("write local");

        let write = execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
            &worktree,
            Some("chat-path-isolation"),
            WRITE_FILE_TOOL,
            json!({
                "path": shared_file.to_string_lossy(),
                "content": "nope",
                "startLine": null,
                "endLine": null
            }),
            None,
            None,
            true,
        );
        assert!(write.is_error, "{:?}", write.output);

        let edit = execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
            &worktree,
            Some("chat-path-isolation"),
            EDIT_FILE_TOOL,
            json!({
                "path": shared_file.to_string_lossy(),
                "oldStr": "shared secret",
                "newStr": "edited",
                "replaceAll": false
            }),
            None,
            None,
            true,
        );
        assert!(edit.is_error, "{:?}", edit.output);

        // Without the grant, absolute paths outside the execution root stay rejected.
        let find_denied = execute_builtin_tool(
            &worktree,
            FIND_FILES_TOOL,
            json!({
                "path": shared.path().to_string_lossy(),
                "include": null,
                "exclude": null,
                "timeoutMs": 5000
            }),
        );
        assert!(find_denied.is_error, "{:?}", find_denied.output);

        // With the grant, find_files may list the authorized external directory (absolute entry paths).
        let find = execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
            &worktree,
            Some("chat-path-isolation"),
            FIND_FILES_TOOL,
            json!({
                "path": shared.path().to_string_lossy(),
                "include": null,
                "exclude": null,
                "timeoutMs": 5000
            }),
            None,
            None,
            true,
        );
        assert!(!find.is_error, "{:?}", find.output);
        let entries = find.output["entries"].as_array().expect("entries");
        assert!(
            entries.iter().any(|entry| {
                entry["path"].as_str().is_some_and(|path| {
                    path.contains("secret.txt") && Path::new(path).is_absolute()
                })
            }),
            "external find_files should report absolute entry paths: {:?}",
            find.output
        );

        let parent_write =
            execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
                &worktree,
                Some("chat-path-isolation"),
                WRITE_FILE_TOOL,
                json!({
                    "path": "../../../secret.txt",
                    "content": "via parent",
                    "startLine": null,
                    "endLine": null
                }),
                None,
                None,
                true,
            );
        assert!(parent_write.is_error, "{:?}", parent_write.output);

        assert_eq!(
            fs::read_to_string(&shared_file).expect("shared content"),
            "shared secret"
        );

        // Relative path under the worktree still works.
        let local_read = execute_builtin_tool(
            &worktree,
            READ_FILE_TOOL,
            json!({ "path": "local.txt", "startLine": null, "endLine": null }),
        );
        assert!(!local_read.is_error, "{:?}", local_read.output);
        assert_eq!(local_read.output["content"], "1\tlocal");
    }

    #[test]
    fn find_files_internal_absolute_path_does_not_need_external_flag() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("a.txt"), "a").expect("write a");
        let absolute = fs::canonicalize(workspace.path()).expect("canonicalize workspace");

        let result = execute_builtin_tool(
            workspace.path(),
            FIND_FILES_TOOL,
            json!({
                "path": absolute.to_string_lossy(),
                "include": null,
                "exclude": null,
                "timeoutMs": 5000
            }),
        );
        assert!(!result.is_error, "{:?}", result.output);
        let entries = result.output["entries"].as_array().expect("entries");
        assert!(
            entries
                .iter()
                .any(|entry| entry["path"].as_str() == Some("a.txt")),
            "internal absolute directory should still report workspace-relative paths: {:?}",
            result.output
        );
    }

    #[test]
    fn find_files_external_requires_flag_and_returns_absolute_paths() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("ext.txt"), "ext").expect("write ext");
        let outside_abs = fs::canonicalize(outside.path()).expect("canonicalize outside");

        let denied = execute_builtin_tool(
            workspace.path(),
            FIND_FILES_TOOL,
            json!({
                "path": outside_abs.to_string_lossy(),
                "include": null,
                "exclude": null,
                "timeoutMs": 5000
            }),
        );
        assert!(denied.is_error, "{:?}", denied.output);

        let allowed = execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
            workspace.path(),
            Some("chat-find-external"),
            FIND_FILES_TOOL,
            json!({
                "path": outside_abs.to_string_lossy(),
                "include": ["**/*.txt"],
                "exclude": null,
                "timeoutMs": 5000
            }),
            None,
            None,
            true,
        );
        assert!(!allowed.is_error, "{:?}", allowed.output);
        let entries = allowed.output["entries"].as_array().expect("entries");
        assert_eq!(entries.len(), 1);
        let path = entries[0]["path"].as_str().expect("path");
        assert!(Path::new(path).is_absolute(), "{path}");
        assert!(
            path.ends_with("ext.txt") || path.contains("ext.txt"),
            "{path}"
        );
    }

    #[test]
    fn search_text_external_requires_flag_absolute_matches_and_workspace_snapshot() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("hit.txt"), "unique-token-xyz\n").expect("write hit");
        let outside_abs = fs::canonicalize(outside.path()).expect("canonicalize outside");

        let denied = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "unique-token-xyz",
                "path": outside_abs.to_string_lossy(),
                "continuation": null,
                "timeoutMs": 10000
            }),
        );
        assert!(denied.is_error, "{:?}", denied.output);
        assert!(
            !workspace
                .path()
                .join(".foco")
                .join("search-results")
                .exists()
                || fs::read_dir(workspace.path().join(".foco").join("search-results"))
                    .map(|dir| dir.count() == 0)
                    .unwrap_or(true),
            "denied external search must not create snapshots"
        );

        let allowed = execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
            workspace.path(),
            Some("chat-search-external"),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "unique-token-xyz",
                "path": outside_abs.to_string_lossy(),
                "continuation": null,
                "timeoutMs": 10000
            }),
            None,
            None,
            true,
        );
        assert!(!allowed.is_error, "{:?}", allowed.output);
        let matches = allowed.output["matches"].as_array().expect("matches");
        assert_eq!(matches.len(), 1);
        let match_path = matches[0]["path"].as_str().expect("match path");
        assert!(Path::new(match_path).is_absolute(), "{match_path}");
        assert!(
            match_path.ends_with("hit.txt") || match_path.contains("hit.txt"),
            "{match_path}"
        );

        // If a snapshot is present (truncated), fullResultPath must be under the execution workspace.
        if let Some(full) = allowed.output["fullResultPath"].as_str() {
            assert!(
                full.starts_with(".foco/search-results/")
                    || full.starts_with(".foco\\search-results\\"),
                "snapshot must stay in execution workspace: {full}"
            );
            assert!(
                workspace.path().join(full).exists() || {
                    // path may use forward slashes
                    workspace.path().join(full.replace('\\', "/")).exists()
                },
                "fullResultPath should resolve under workspace: {full}"
            );
        }
    }

    #[test]
    fn search_text_internal_match_paths_remain_workspace_relative() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("src.txt"), "internal-token-abc\n").expect("write");
        let absolute = fs::canonicalize(workspace.path().join("src.txt")).expect("canon");

        let relative = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "internal-token-abc",
                "path": ".",
                "continuation": null,
                "timeoutMs": 10000
            }),
        );
        assert!(!relative.is_error, "{:?}", relative.output);
        assert_eq!(
            relative.output["matches"][0]["path"].as_str(),
            Some("src.txt")
        );

        let abs_internal = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "internal-token-abc",
                "path": absolute.to_string_lossy(),
                "continuation": null,
                "timeoutMs": 10000
            }),
        );
        assert!(!abs_internal.is_error, "{:?}", abs_internal.output);
        assert_eq!(
            abs_internal.output["matches"][0]["path"].as_str(),
            Some("src.txt")
        );
    }

    /// External search may paginate without re-checking the external-read flag: the snapshot
    /// lives under the execution workspace and was created only after an authorized initial search.
    #[test]
    fn search_text_external_continuation_pages_without_external_flag() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        let total = 400;
        let mut content = String::new();
        for index in 0..total {
            content.push_str(&format!("ext-needle {index} {}\n", "y".repeat(200)));
        }
        fs::write(outside.path().join("big-ext.txt"), content).expect("write big");
        let outside_abs = fs::canonicalize(outside.path()).expect("canon outside");

        let first = execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
            workspace.path(),
            Some("chat-search-ext-page"),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "ext-needle",
                "path": outside_abs.to_string_lossy(),
                "continuation": null,
                "timeoutMs": 15000
            }),
            None,
            None,
            true,
        );
        assert!(!first.is_error, "{:?}", first.output);
        assert_eq!(first.output["truncated"], true);
        let continuation = first.output["continuation"]
            .as_str()
            .expect("continuation for truncated external search");
        let full_path = first.output["fullResultPath"]
            .as_str()
            .expect("fullResultPath");
        assert!(
            full_path.starts_with(".foco/search-results/")
                || full_path.starts_with(".foco\\search-results\\"),
            "snapshot under execution workspace: {full_path}"
        );
        let match_path = first.output["matches"][0]["path"]
            .as_str()
            .expect("match path");
        assert!(
            Path::new(match_path).is_absolute(),
            "external matches stay absolute: {match_path}"
        );

        // Continuation must not re-run rg against the external root, so no external flag is required.
        let page_two = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "ext-needle",
                "path": outside_abs.to_string_lossy(),
                "continuation": continuation,
                "timeoutMs": 15000
            }),
        );
        assert!(!page_two.is_error, "{:?}", page_two.output);
        assert_eq!(
            page_two.output["totalMatches"].as_u64().expect("total"),
            total as u64
        );
        let page_two_matches = page_two.output["matches"].as_array().expect("matches");
        assert!(!page_two_matches.is_empty());
        let first_page_first = first.output["matches"][0]["text"]
            .as_str()
            .expect("first page first");
        let second_page_first = page_two_matches[0]["text"].as_str().expect("second");
        assert_ne!(first_page_first, second_page_first);

        // Query/path binding still applies on external continuation pages.
        let mismatch = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "other-query",
                "path": outside_abs.to_string_lossy(),
                "continuation": continuation,
                "timeoutMs": 15000
            }),
        );
        assert!(mismatch.is_error, "{:?}", mismatch.output);
        let error = mismatch
            .output
            .get("error")
            .and_then(Value::as_str)
            .expect("error");
        assert!(
            error.contains("continuation") && error.contains("invalid"),
            "{error}"
        );

        // Empty / blank continuation restarts a search and still requires the external grant.
        for continuation_value in [json!(""), json!("   ")] {
            let restarted = execute_builtin_tool(
                workspace.path(),
                SEARCH_TEXT_TOOL,
                json!({
                    "query": "ext-needle",
                    "path": outside_abs.to_string_lossy(),
                    "continuation": continuation_value,
                    "timeoutMs": 15000
                }),
            );
            assert!(
                restarted.is_error,
                "blank continuation without external flag must fail: {:?}",
                restarted.output
            );
        }

        // fullResultPath is workspace-relative; reading the snapshot never needs external grant.
        let snap_read = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({
                "path": full_path,
                "startLine": 1,
                "endLine": 5,
                "timeoutMs": 10000
            }),
        );
        assert!(
            !snap_read.is_error,
            "fullResultPath must be readable without external grant: {:?}",
            snap_read.output
        );
        assert!(
            snap_read.output["content"]
                .as_str()
                .is_some_and(|content| content.contains("ext-needle")),
            "{:?}",
            snap_read.output
        );
    }

    /// search_text.path accepts a single file (rg-compatible), not only directories.
    #[test]
    fn search_text_external_file_path_absolute_match_and_workspace_snapshot() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        let total = 350;
        let mut content = String::new();
        for index in 0..total {
            content.push_str(&format!("file-needle {index} {}\n", "z".repeat(180)));
        }
        let outside_file = outside.path().join("only-file.txt");
        fs::write(&outside_file, content).expect("write outside file");
        let outside_file_abs = fs::canonicalize(&outside_file).expect("canon file");

        let denied = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "file-needle",
                "path": outside_file_abs.to_string_lossy(),
                "continuation": null,
                "timeoutMs": 15000
            }),
        );
        assert!(
            denied.is_error,
            "external file path without grant must fail: {:?}",
            denied.output
        );

        let allowed = execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
            workspace.path(),
            Some("chat-search-ext-file"),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "file-needle",
                "path": outside_file_abs.to_string_lossy(),
                "continuation": null,
                "timeoutMs": 15000
            }),
            None,
            None,
            true,
        );
        assert!(!allowed.is_error, "{:?}", allowed.output);
        assert_eq!(allowed.output["truncated"], true);
        let match_path = allowed.output["matches"][0]["path"]
            .as_str()
            .expect("match path");
        assert!(
            Path::new(match_path).is_absolute(),
            "external file search match must be absolute: {match_path}"
        );
        assert!(
            match_path.ends_with("only-file.txt") || match_path.contains("only-file.txt"),
            "{match_path}"
        );

        let full_path = allowed.output["fullResultPath"]
            .as_str()
            .expect("fullResultPath required when truncated");
        assert!(
            full_path.starts_with(".foco/search-results/")
                || full_path.starts_with(".foco\\search-results\\"),
            "snapshot must stay under execution workspace .foco: {full_path}"
        );
        let snap_abs = workspace.path().join(full_path.replace('\\', "/"));
        assert!(
            snap_abs.exists(),
            "snapshot file must exist under workspace: {}",
            snap_abs.display()
        );
        assert!(
            !full_path.contains(outside_file_abs.to_string_lossy().as_ref()),
            "fullResultPath must not point at external root: {full_path}"
        );

        // Continuation without external flag still works; query/path binding holds.
        let continuation = allowed.output["continuation"]
            .as_str()
            .expect("continuation");
        let page = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "file-needle",
                "path": outside_file_abs.to_string_lossy(),
                "continuation": continuation,
                "timeoutMs": 15000
            }),
        );
        assert!(!page.is_error, "{:?}", page.output);
        assert_eq!(
            page.output["totalMatches"].as_u64().expect("total"),
            total as u64
        );

        // Snapshot dump is an internal workspace read — no external grant.
        let snap_read = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({
                "path": full_path,
                "startLine": 1,
                "endLine": 3,
                "timeoutMs": 10000
            }),
        );
        assert!(
            !snap_read.is_error,
            "reading fullResultPath must not require external grant: {:?}",
            snap_read.output
        );
    }

    #[cfg(unix)]
    #[test]
    fn find_files_and_search_text_symlink_escape_is_external() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("escaped.txt"), "escaped-token\n").expect("write outside");
        let link_dir = workspace.path().join("linked-dir");
        symlink(outside.path(), &link_dir).expect("symlink dir");

        let absolute_link = fs::canonicalize(workspace.path())
            .expect("canon workspace")
            .join("linked-dir");

        assert!(
            find_files_target_outside_workspace(
                workspace.path(),
                absolute_link.to_str().expect("utf8")
            )
            .expect("classify find")
            .is_some(),
            "symlink dir escape should classify as external for find_files"
        );
        assert!(
            search_text_target_outside_workspace(
                workspace.path(),
                absolute_link.to_str().expect("utf8")
            )
            .expect("classify search")
            .is_some(),
            "symlink dir escape should classify as external for search_text"
        );

        let find_denied = execute_builtin_tool(
            workspace.path(),
            FIND_FILES_TOOL,
            json!({
                "path": absolute_link.to_string_lossy(),
                "include": null,
                "exclude": null,
                "timeoutMs": 5000
            }),
        );
        assert!(
            find_denied.is_error,
            "find_files symlink escape without grant: {:?}",
            find_denied.output
        );

        let find_allowed =
            execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
                workspace.path(),
                Some("chat-find-symlink"),
                FIND_FILES_TOOL,
                json!({
                    "path": absolute_link.to_string_lossy(),
                    "include": ["**/*.txt"],
                    "exclude": null,
                    "timeoutMs": 5000
                }),
                None,
                None,
                true,
            );
        assert!(!find_allowed.is_error, "{:?}", find_allowed.output);
        let entries = find_allowed.output["entries"].as_array().expect("entries");
        assert!(
            entries.iter().any(|entry| {
                entry["path"].as_str().is_some_and(|path| {
                    Path::new(path).is_absolute() && path.contains("escaped.txt")
                })
            }),
            "authorized symlink find should list absolute external paths: {:?}",
            find_allowed.output
        );

        let search_denied = execute_builtin_tool(
            workspace.path(),
            SEARCH_TEXT_TOOL,
            json!({
                "query": "escaped-token",
                "path": absolute_link.to_string_lossy(),
                "continuation": null,
                "timeoutMs": 10000
            }),
        );
        assert!(
            search_denied.is_error,
            "search_text symlink escape without grant: {:?}",
            search_denied.output
        );

        let search_allowed =
            execute_builtin_tool_for_chat_with_cancellation_and_output_sink_and_external_read_access(
                workspace.path(),
                Some("chat-search-symlink"),
                SEARCH_TEXT_TOOL,
                json!({
                    "query": "escaped-token",
                    "path": absolute_link.to_string_lossy(),
                    "continuation": null,
                    "timeoutMs": 10000
                }),
                None,
                None,
                true,
            );
        assert!(!search_allowed.is_error, "{:?}", search_allowed.output);
        let match_path = search_allowed.output["matches"][0]["path"]
            .as_str()
            .expect("match path");
        assert!(
            Path::new(match_path).is_absolute(),
            "external symlink search match must be absolute: {match_path}"
        );
    }

    #[test]
    fn read_file_find_files_search_text_schema_document_external_paths() {
        let definitions = builtin_tool_definitions();
        for name in [READ_FILE_TOOL, FIND_FILES_TOOL, SEARCH_TEXT_TOOL] {
            let definition = definitions
                .iter()
                .find(|definition| definition.name == name)
                .unwrap_or_else(|| panic!("{name} definition"));
            let path_description = definition.input_schema["properties"]["path"]["description"]
                .as_str()
                .expect("path description");
            assert!(
                path_description.contains("absolute") || path_description.contains("Absolute"),
                "{name}: {path_description}"
            );
            assert!(
                path_description.contains("execution")
                    || path_description.contains("external")
                    || path_description.contains("confirmation"),
                "{name}: {path_description}"
            );
        }

        // Graph path tools must not claim external absolute support.
        let graph = definitions
            .iter()
            .find(|definition| definition.name == GRAPH_FIND_SYMBOLS_TOOL)
            .expect("graph_find_symbols");
        let graph_path = graph.input_schema["properties"]["path"]["description"]
            .as_str()
            .expect("graph path");
        assert!(
            graph_path.contains("workspace-relative") || graph_path.contains("Workspace-relative"),
            "{graph_path}"
        );
        assert!(
            !graph_path.contains("user confirmation")
                && !graph_path.contains("external-read grant"),
            "graph must not advertise external grants: {graph_path}"
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
    fn managed_command_tools_expose_strict_start_read_and_stop_contracts() {
        let definitions = builtin_tool_definitions();
        let run_command = definitions
            .iter()
            .find(|definition| definition.name == RUN_COMMAND_TOOL)
            .expect("run_command definition");
        let required = run_command.input_schema["required"]
            .as_array()
            .expect("run_command required fields");

        assert!(run_command.strict);
        assert!(required.contains(&json!("background")));
        assert!(required.contains(&json!("backgroundTimeoutMs")));

        for tool_name in [GET_COMMAND_OUTPUT_TOOL, STOP_COMMAND_TOOL] {
            let definition = definitions
                .iter()
                .find(|definition| definition.name == tool_name)
                .unwrap_or_else(|| panic!("{tool_name} definition"));
            assert!(definition.strict);
            assert_eq!(definition.input_schema["additionalProperties"], false);
            assert!(
                definition.input_schema["required"]
                    .as_array()
                    .expect("required fields")
                    .contains(&json!("processId"))
            );
        }
    }

    #[test]
    fn managed_background_command_returns_a_reusable_handle_and_idempotent_stop() {
        let workspace = tempfile::tempdir().expect("workspace");
        let command = std::env::current_exe()
            .expect("current test executable")
            .to_string_lossy()
            .to_string();
        let started = Instant::now();
        let launch = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": command,
                "args": ["--ignored", "--exact", "tests::timeout_child_process"],
                "cwd": null,
                "timeoutMs": null,
                "background": true,
                "backgroundTimeoutMs": null
            }),
        );

        assert!(!launch.is_error, "{:?}", launch.output);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(launch.output["processId"].as_str().is_some());
        assert!(launch.output["pid"].as_u64().is_some());
        assert!(launch.output["startedAt"].as_u64().is_some());
        assert!(launch.output["chunks"].is_array());
        assert!(launch.output["nextCursor"].as_u64().is_some());
        assert!(launch.output["hasMore"].is_boolean());

        let process_id = launch.output["processId"]
            .as_str()
            .expect("process id")
            .to_string();
        let cursor = launch.output["nextCursor"].as_u64();
        let read = execute_builtin_tool(
            workspace.path(),
            GET_COMMAND_OUTPUT_TOOL,
            json!({
                "processId": process_id,
                "cursor": cursor,
                "waitMs": null,
                "timeoutMs": null
            }),
        );
        assert!(!read.is_error, "{:?}", read.output);
        assert_eq!(read.output["fromCursor"], json!(cursor));
        assert!(read.output["availableFromCursor"].as_u64().is_some());
        assert!(read.output["cursorExpired"].is_boolean());
        assert!(read.output["retainedOutputBytes"].as_u64().is_some());

        let first_stop = execute_builtin_tool(
            workspace.path(),
            STOP_COMMAND_TOOL,
            json!({ "processId": process_id, "timeoutMs": null }),
        );
        assert!(!first_stop.is_error, "{:?}", first_stop.output);
        let second_stop = execute_builtin_tool(
            workspace.path(),
            STOP_COMMAND_TOOL,
            json!({ "processId": first_stop.output["processId"], "timeoutMs": null }),
        );
        assert!(!second_stop.is_error, "{:?}", second_stop.output);
    }

    #[test]
    fn managed_command_hides_cross_workspace_handle_existence() {
        let owner_workspace = tempfile::tempdir().expect("owner workspace");
        let other_workspace = tempfile::tempdir().expect("other workspace");
        let command = std::env::current_exe()
            .expect("current test executable")
            .to_string_lossy()
            .to_string();
        let launch = execute_builtin_tool(
            owner_workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": command,
                "args": ["--ignored", "--exact", "tests::timeout_child_process"],
                "cwd": null,
                "timeoutMs": null,
                "background": true,
                "backgroundTimeoutMs": null
            }),
        );
        assert!(!launch.is_error, "{:?}", launch.output);
        let process_id = launch.output["processId"]
            .as_str()
            .expect("process id")
            .to_string();

        let foreign = execute_builtin_tool(
            other_workspace.path(),
            GET_COMMAND_OUTPUT_TOOL,
            json!({
                "processId": process_id,
                "cursor": null,
                "waitMs": null,
                "timeoutMs": null
            }),
        );
        let missing = execute_builtin_tool(
            other_workspace.path(),
            GET_COMMAND_OUTPUT_TOOL,
            json!({
                "processId": "command-does-not-exist",
                "cursor": null,
                "waitMs": null,
                "timeoutMs": null
            }),
        );
        let stop = execute_builtin_tool(
            owner_workspace.path(),
            STOP_COMMAND_TOOL,
            json!({ "processId": launch.output["processId"], "timeoutMs": null }),
        );

        assert!(foreign.is_error, "{:?}", foreign.output);
        assert!(missing.is_error, "{:?}", missing.output);
        assert_eq!(foreign.output["error"], missing.output["error"]);
        assert!(!stop.is_error, "{:?}", stop.output);
    }

    #[test]
    fn update_spec_schema_supports_nullable_patch_and_replacement_payloads() {
        let definition = builtin_tool_definitions()
            .into_iter()
            .find(|definition| definition.name == UPDATE_SPEC_TOOL)
            .expect("update_spec definition");
        let properties = definition.input_schema["properties"]
            .as_object()
            .expect("update_spec properties");
        let required = definition.input_schema["required"]
            .as_array()
            .expect("update_spec required");

        assert!(definition.strict);
        assert_eq!(definition.input_schema["additionalProperties"], false);
        assert_eq!(
            properties["contentMarkdown"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(properties["edits"]["type"], json!(["array", "null"]));
        assert_eq!(properties["edits"]["items"]["additionalProperties"], false);
        assert_eq!(
            properties["edits"]["items"]["required"],
            json!(["oldText", "newText"])
        );
        assert_eq!(
            properties["edits"]["items"]["properties"]["oldText"]["type"],
            "string"
        );
        assert_eq!(
            properties["edits"]["items"]["properties"]["newText"]["type"],
            "string"
        );
        assert_eq!(
            required,
            &vec![
                json!("expectedRevision"),
                json!("contentMarkdown"),
                json!("edits"),
                json!("timeoutMs"),
            ]
        );
        assert!(definition.description.contains("Call read_spec first"));
        assert!(
            definition
                .description
                .contains("latest revision and exact content")
        );
        assert!(definition.description.contains("Prefer edits"));
        assert!(
            definition
                .description
                .contains("exactly one non-null update payload")
        );
    }

    #[test]
    fn spec_tools_round_trip_with_revision_conflict() {
        let workspace = tempfile::tempdir().expect("workspace");

        let initial = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "timeoutMs": null }),
        );
        assert!(!initial.is_error);
        assert_eq!(initial.output["enabled"], false);
        assert_eq!(initial.output["injectEnabled"], false);
        assert_eq!(initial.output["revision"], 0);
        assert_eq!(initial.output["contentMarkdown"], "");
        assert_eq!(initial.output["generatedAt"], Value::Null);
        assert_eq!(initial.output["updatedAt"], Value::Null);

        // Legacy calls that omit the newer edits field remain valid.
        let first_update = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": "# Project Spec\n\nVersion one",
                "timeoutMs": null
            }),
        );
        assert!(!first_update.is_error);
        assert_eq!(first_update.output["revision"], 1);
        assert_eq!(
            first_update.output["contentMarkdown"],
            "# Project Spec\n\nVersion one"
        );
        assert_eq!(first_update.output["updateMode"], "fullReplacement");
        assert_eq!(first_update.output["editCount"], 0);
        assert_eq!(first_update.output["lineCountBefore"], 0);
        assert_eq!(first_update.output["lineCountAfter"], 3);

        let stale_update = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": "# Project Spec\n\nStale write",
                "timeoutMs": null
            }),
        );
        assert!(stale_update.is_error);
        assert!(
            stale_update.output["error"]
                .as_str()
                .expect("stale error")
                .contains("call read_spec again")
        );

        let second_update = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 1,
                "contentMarkdown": "# Project Spec\n\nVersion two",
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!second_update.is_error);
        assert_eq!(second_update.output["revision"], 2);
        assert_eq!(
            second_update.output["contentMarkdown"],
            "# Project Spec\n\nVersion two"
        );
        assert_eq!(second_update.output["updateMode"], "fullReplacement");
        assert_eq!(second_update.output["editCount"], 0);
        assert_eq!(second_update.output["lineCountBefore"], 3);
        assert_eq!(second_update.output["lineCountAfter"], 3);

        let read_back = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "timeoutMs": null }),
        );
        assert!(!read_back.is_error);
        assert_eq!(
            read_back.output["revision"],
            second_update.output["revision"]
        );
        assert_eq!(
            read_back.output["contentMarkdown"],
            second_update.output["contentMarkdown"]
        );
    }

    #[test]
    fn update_spec_applies_single_edit_and_deletes_text() {
        let workspace = tempfile::tempdir().expect("workspace");
        let initial = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": "# Spec\n\nKeep\nRemove me",
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!initial.is_error);

        let single_edit = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 1,
                "contentMarkdown": null,
                "edits": [{ "oldText": "Keep", "newText": "Keep updated" }],
                "timeoutMs": null
            }),
        );
        assert!(!single_edit.is_error, "{}", single_edit.output);
        assert_eq!(single_edit.output["revision"], 2);
        assert_eq!(single_edit.output["editCount"], 1);
        assert_eq!(
            single_edit.output["contentMarkdown"],
            "# Spec\n\nKeep updated\nRemove me"
        );

        let deletion = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 2,
                "contentMarkdown": null,
                "edits": [{ "oldText": "\nRemove me", "newText": "" }],
                "timeoutMs": null
            }),
        );
        assert!(!deletion.is_error, "{}", deletion.output);
        assert_eq!(deletion.output["revision"], 3);
        assert_eq!(deletion.output["editCount"], 1);
        assert_eq!(deletion.output["lineCountBefore"], 4);
        assert_eq!(deletion.output["lineCountAfter"], 3);
        assert_eq!(deletion.output["contentMarkdown"], "# Spec\n\nKeep updated");
    }

    #[test]
    fn update_spec_applies_ordered_exact_text_edits() {
        let workspace = tempfile::tempdir().expect("workspace");
        let initial_content = "# Spec\n\nAlpha\nBeta";
        let initial = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": initial_content,
                "timeoutMs": null
            }),
        );
        assert!(!initial.is_error);

        let patched = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 1,
                "contentMarkdown": null,
                "edits": [
                    { "oldText": "Alpha", "newText": "Gamma\nDelta" },
                    { "oldText": "Delta\nBeta", "newText": "Delta\nEpsilon" }
                ],
                "timeoutMs": null
            }),
        );

        assert!(!patched.is_error, "{}", patched.output);
        assert_eq!(patched.output["revision"], 2);
        assert_eq!(
            patched.output["contentMarkdown"],
            "# Spec\n\nGamma\nDelta\nEpsilon"
        );
        assert_eq!(patched.output["updateMode"], "patch");
        assert_eq!(patched.output["editCount"], 2);
        assert_eq!(patched.output["lineCountBefore"], 4);
        assert_eq!(patched.output["lineCountAfter"], 5);

        let stale_patch = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 1,
                "contentMarkdown": null,
                "edits": [{ "oldText": "Gamma", "newText": "Stale" }],
                "timeoutMs": null
            }),
        );
        assert!(stale_patch.is_error);
        assert!(
            stale_patch.output["error"]
                .as_str()
                .expect("stale patch error")
                .contains("call read_spec again")
        );
    }

    #[test]
    fn update_spec_does_not_overwrite_changes_made_after_read_spec() {
        let workspace = tempfile::tempdir().expect("workspace");
        let initial = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": "# Spec\n\nOriginal",
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!initial.is_error);

        let read_before_concurrent_write = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "timeoutMs": null }),
        );
        assert_eq!(read_before_concurrent_write.output["revision"], 1);

        let mut concurrent_database =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("concurrent database");
        let concurrent = concurrent_database
            .update_workspace_spec_content(1, "# Spec\n\nConcurrent update")
            .expect("concurrent update")
            .expect("concurrent update won CAS");
        assert_eq!(concurrent.revision, 2);

        let rejected = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": read_before_concurrent_write.output["revision"],
                "contentMarkdown": null,
                "edits": [{ "oldText": "Original", "newText": "Agent update" }],
                "timeoutMs": null
            }),
        );
        assert!(rejected.is_error);
        assert!(
            rejected.output["error"]
                .as_str()
                .expect("revision conflict error")
                .contains("call read_spec again")
        );

        let read_back = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "timeoutMs": null }),
        );
        assert_eq!(read_back.output["revision"], 2);
        assert_eq!(
            read_back.output["contentMarkdown"],
            "# Spec\n\nConcurrent update"
        );
    }

    #[test]
    fn update_spec_rejects_invalid_patches_without_partial_writes() {
        let workspace = tempfile::tempdir().expect("workspace");
        let initial_content = "# Spec\n\nAlpha\nAlpha";
        let initial = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": initial_content,
                "timeoutMs": null
            }),
        );
        assert!(!initial.is_error);

        let invalid_payloads = [
            json!({
                "expectedRevision": 1,
                "contentMarkdown": null,
                "edits": [],
                "timeoutMs": null
            }),
            json!({
                "expectedRevision": 1,
                "contentMarkdown": null,
                "edits": [{ "oldText": "", "newText": "x" }],
                "timeoutMs": null
            }),
            json!({
                "expectedRevision": 1,
                "contentMarkdown": null,
                "edits": [{ "oldText": "Alpha", "newText": "Changed" }],
                "timeoutMs": null
            }),
            json!({
                "expectedRevision": 1,
                "contentMarkdown": null,
                "edits": [
                    { "oldText": "# Spec", "newText": "# Changed" },
                    { "oldText": "Missing", "newText": "Never applied" }
                ],
                "timeoutMs": null
            }),
            json!({
                "expectedRevision": 1,
                "contentMarkdown": null,
                "edits": [
                    { "oldText": "# Spec", "newText": "# Changed" },
                    { "oldText": "# Changed", "newText": "# Spec" }
                ],
                "timeoutMs": null
            }),
            json!({
                "expectedRevision": 1,
                "contentMarkdown": "replacement",
                "edits": [{ "oldText": "# Spec", "newText": "# Changed" }],
                "timeoutMs": null
            }),
            json!({
                "expectedRevision": 1,
                "contentMarkdown": null,
                "edits": null,
                "timeoutMs": null
            }),
        ];

        for payload in invalid_payloads {
            let rejected = execute_builtin_tool(workspace.path(), UPDATE_SPEC_TOOL, payload);
            assert!(rejected.is_error, "expected rejection: {}", rejected.output);

            let read_back = execute_builtin_tool(
                workspace.path(),
                READ_SPEC_TOOL,
                json!({ "timeoutMs": null }),
            );
            assert_eq!(read_back.output["revision"], 1);
            assert_eq!(read_back.output["contentMarkdown"], initial_content);
        }

        let oversized = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 1,
                "contentMarkdown": null,
                "edits": [{
                    "oldText": "# Spec",
                    "newText": "x".repeat(WORKSPACE_SPEC_MAX_MARKDOWN_BYTES + 1)
                }],
                "timeoutMs": null
            }),
        );
        assert!(oversized.is_error);

        let read_back = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "timeoutMs": null }),
        );
        assert_eq!(read_back.output["revision"], 1);
        assert_eq!(read_back.output["contentMarkdown"], initial_content);
    }

    #[test]
    fn builtin_tools_include_spec_tools() {
        let tool_names = builtin_tool_definitions()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(tool_names.contains(&READ_SPEC_TOOL));
        assert!(tool_names.contains(&UPDATE_SPEC_TOOL));
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
    fn command_output_delta_budget_counts_lossy_utf8_bytes() {
        let streamed_bytes = Arc::new(AtomicUsize::new(0));
        let truncation_notices = Arc::new(AtomicUsize::new(0));
        let sink = CountingOutputSink {
            bytes: streamed_bytes.clone(),
            truncation_notices: truncation_notices.clone(),
        };
        let mut budget = CommandDeltaBudget::new(Some(1_024));

        emit_command_output_delta(
            Some(&sink),
            &ToolOutputStream::Stdout,
            &vec![0xff; 4_096],
            &mut budget,
        );

        assert!(streamed_bytes.load(Ordering::Relaxed) <= 1_024);
        assert_eq!(truncation_notices.load(Ordering::Relaxed), 1);
        assert!(budget.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn run_command_caps_captured_and_streamed_output_without_failing() {
        let workspace = tempfile::tempdir().expect("workspace");
        let streamed_bytes = Arc::new(AtomicUsize::new(0));
        let truncation_notices = Arc::new(AtomicUsize::new(0));
        let result = execute_builtin_tool_for_chat_with_cancellation_and_output_sink(
            workspace.path(),
            None,
            RUN_COMMAND_TOOL,
            json!({
                "command": "sh",
                "args": ["-c", "head -c 60000 /dev/zero | tr '\\0' x"],
                "cwd": null,
                "timeoutMs": null
            }),
            None,
            Some(Arc::new(CountingOutputSink {
                bytes: streamed_bytes.clone(),
                truncation_notices: truncation_notices.clone(),
            })),
        );

        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["stdoutTruncated"], true);
        assert_eq!(result.output["outputDeltaTruncated"], true);
        assert_eq!(result.output["outputOmitted"], true);
        assert_eq!(result.output["retryUnsafe"], true);
        assert_eq!(result.output["stdoutBytes"], 60_000);
        assert!(
            result.output["stdout"].as_str().expect("stdout").len()
                <= MAX_COMMAND_CAPTURE_BYTES_PER_STREAM
        );
        assert!(
            streamed_bytes.load(Ordering::Relaxed) <= output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT
        );
        assert_eq!(truncation_notices.load(Ordering::Relaxed), 1);
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
    fn run_command_blocks_recursive_scan_that_escapes_workspace() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": "find",
                "args": ["..", "-name", "npm"],
                "cwd": null,
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
                .contains("run_command refuses to run recursive scans outside the workspace")
        );
    }

    #[test]
    fn run_command_blocks_shell_recursive_scan_that_escapes_workspace() {
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": "bash",
                "args": ["-lc", "find .. -name npm 2>/dev/null | head -20"],
                "cwd": null,
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
                .contains("run_command refuses to run recursive scans outside the workspace")
        );
    }

    #[test]
    fn run_command_blocks_shell_recursive_scan_of_user_home() {
        let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
        else {
            return;
        };
        let home = home.to_string_lossy();
        let workspace = tempfile::tempdir().expect("workspace");

        let result = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": "bash",
                "args": ["-lc", format!("find {home} -path '*bin/npm' -type f 2>/dev/null | head -20")],
                "cwd": null,
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
                .contains("run_command refuses to run recursive scans outside the workspace")
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_command_allows_recursive_scan_inside_workspace() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("note.txt"), "hello\n").expect("write note");

        let result = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": "find",
                "args": [".", "-maxdepth", "1", "-name", "note.txt"],
                "cwd": null,
                "timeoutMs": null
            }),
        );

        assert!(!result.is_error);
        assert!(
            result.output["stdout"]
                .as_str()
                .expect("stdout")
                .contains("note.txt")
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_command_timeout_remains_responsive_during_continuous_stdout() {
        let workspace = tempfile::tempdir().expect("workspace");
        let started = Instant::now();

        let result = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": "sh",
                "args": ["-c", "while :; do printf '0123456789abcdef'; done"],
                "cwd": null,
                "timeoutMs": 100
            }),
        );

        assert!(result.is_error);
        assert!(
            result.output["error"]
                .as_str()
                .expect("timeout error")
                .contains("timed out")
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn run_command_cancellation_remains_responsive_during_continuous_stdout() {
        let workspace = tempfile::tempdir().expect("workspace");
        let cancellation = ToolCancellationToken::default();
        let cancellation_trigger = cancellation.clone();
        let started = Instant::now();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            cancellation_trigger.cancel();
        });

        let result = execute_builtin_tool_for_chat_with_cancellation_and_output_sink(
            workspace.path(),
            None,
            RUN_COMMAND_TOOL,
            json!({
                "command": "sh",
                "args": ["-c", "while :; do printf '0123456789abcdef'; done"],
                "cwd": null,
                "timeoutMs": 5_000
            }),
            Some(cancellation),
            None,
        );
        trigger.join().expect("join cancellation trigger");

        assert!(result.is_error);
        assert!(
            result.output["error"]
                .as_str()
                .expect("cancellation error")
                .contains("cancelled")
        );
        assert!(started.elapsed() < Duration::from_secs(2));
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
