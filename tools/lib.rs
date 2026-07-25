mod agent_tools;
mod apply_patch;
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

use foco_store::workspace::WorkspaceDatabaseError;
#[cfg(unix)]
use process_wrap::std::ProcessGroup;
use process_wrap::std::{ChildWrapper, CommandWrap};
#[cfg(windows)]
use process_wrap::std::{CreationFlags, JobObject};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(windows)]
use windows::Win32::System::Threading::PROCESS_CREATION_FLAGS;

use crate::errors::{ToolRuntimeError, tool_error_output};

pub const READ_FILE_TOOL: &str = "read_file";
pub const FIND_FILES_TOOL: &str = "find_files";
pub const SEARCH_TEXT_TOOL: &str = "search_text";
pub const WEB_SEARCH_TOOL: &str = "web_search";
pub const WEB_FETCH_TOOL: &str = "web_fetch";
pub const IMAGE_GEN_TOOL: &str = "image_gen";
pub const WRITE_FILE_TOOL: &str = "write_file";
pub const EDIT_FILE_TOOL: &str = "edit_file";
pub const APPLY_PATCH_TOOL: &str = "apply_patch";
pub const RUN_COMMAND_TOOL: &str = "run_command";
pub const GET_COMMAND_OUTPUT_TOOL: &str = "get_command_output";
pub const STOP_COMMAND_TOOL: &str = "stop_command";
pub const SLEEP_TOOL: &str = "sleep";
pub const GRAPH_FIND_SYMBOLS_TOOL: &str = "graph_find_symbols";
pub const GRAPH_FIND_CALLERS_TOOL: &str = "graph_find_callers";
pub const GRAPH_FIND_CALLEES_TOOL: &str = "graph_find_callees";
pub const GRAPH_FIND_CHILDREN_TOOL: &str = "graph_find_children";
pub const GRAPH_FIND_REFERENCES_TOOL: &str = "graph_find_references";
pub const GRAPH_FIND_IMPORTS_TOOL: &str = "graph_find_imports";
pub const GRAPH_FIND_IMPORTERS_TOOL: &str = "graph_find_importers";
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

/// Ordinary text source protection for `read_file` (full and ranged). Soft output truncation is
/// separate: large sources under this ceiling still return complete-line prefixes successfully.
const MAX_RANGED_READ_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
/// Hard safety cap on numbered `read_file` content before the shared 128 KiB envelope gate
/// (SKILL.md full-document path only; ordinary reads use complete-line soft truncation first).
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
const DEFAULT_APPLY_PATCH_TIMEOUT_MS: u64 = 10_000;
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
const CREATE_NO_WINDOW: PROCESS_CREATION_FLAGS = PROCESS_CREATION_FLAGS(0x0800_0000);
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuiltinToolContext<'a> {
    pub chat_id: Option<&'a str>,
    pub run_id: Option<&'a str>,
    pub session_mode: Option<&'a str>,
}

impl<'a> BuiltinToolContext<'a> {
    pub fn for_chat(chat_id: Option<&'a str>) -> Self {
        Self {
            chat_id,
            run_id: None,
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

/// Host-owned services needed by built-in tools that retain in-memory state across calls.
#[derive(Clone, Default)]
pub struct BuiltinToolRuntime {
    background_commands: BackgroundCommandRegistry,
}

impl BuiltinToolRuntime {
    pub fn new(background_commands: BackgroundCommandRegistry) -> Self {
        Self {
            background_commands,
        }
    }

    pub fn background_commands(&self) -> &BackgroundCommandRegistry {
        &self.background_commands
    }
}

/// Typed execution settings shared by built-in tools for one invocation.
#[derive(Clone)]
pub struct BuiltinToolExecutionOptions {
    pub runtime: BuiltinToolRuntime,
    pub cancellation_token: Option<ToolCancellationToken>,
    pub output_sink: Option<Arc<dyn ToolOutputSink>>,
    pub allow_external_read_access: bool,
}

impl Default for BuiltinToolExecutionOptions {
    fn default() -> Self {
        Self {
            runtime: BuiltinToolRuntime::default(),
            cancellation_token: None,
            output_sink: None,
            allow_external_read_access: false,
        }
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

/// Compatibility wrapper for callers that do not own a long-lived tool runtime.
pub fn execute_builtin_tool_with_context_and_options(
    workspace_path: &Path,
    context: BuiltinToolContext<'_>,
    tool_name: &str,
    arguments: Value,
    cancellation_token: Option<ToolCancellationToken>,
    output_sink: Option<Arc<dyn ToolOutputSink>>,
    allow_external_read_access: bool,
) -> ToolExecution {
    execute_builtin_tool_with_context_and_execution_options(
        workspace_path,
        context,
        tool_name,
        arguments,
        BuiltinToolExecutionOptions {
            cancellation_token,
            output_sink,
            allow_external_read_access,
            ..BuiltinToolExecutionOptions::default()
        },
    )
}

pub fn execute_builtin_tool_with_context_and_execution_options(
    workspace_path: &Path,
    context: BuiltinToolContext<'_>,
    tool_name: &str,
    arguments: Value,
    options: BuiltinToolExecutionOptions,
) -> ToolExecution {
    match execute_builtin_tool_inner(
        workspace_path,
        context,
        tool_name,
        arguments,
        options.cancellation_token.as_ref(),
        options.output_sink.as_deref(),
        options.allow_external_read_access,
        options.runtime.background_commands(),
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
    background_commands: &BackgroundCommandRegistry,
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
        GRAPH_FIND_CHILDREN_TOOL => graph_tools::graph_find_children(workspace_path, arguments),
        GRAPH_FIND_REFERENCES_TOOL => graph_tools::graph_find_references(workspace_path, arguments),
        GRAPH_FIND_IMPORTS_TOOL => graph_tools::graph_find_imports(workspace_path, arguments),
        GRAPH_FIND_IMPORTERS_TOOL => graph_tools::graph_find_importers(workspace_path, arguments),
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
        APPLY_PATCH_TOOL => apply_patch::apply_patch_with_cancellation(
            workspace_path,
            arguments,
            cancellation_token,
        ),
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
        RUN_COMMAND_TOOL => command_tools::run_command(
            workspace_path,
            arguments,
            cancellation_token,
            output_sink,
            background_commands,
            context.chat_id,
            context.run_id,
        ),
        GET_COMMAND_OUTPUT_TOOL => command_tools::get_command_output(
            workspace_path,
            arguments,
            background_commands,
            context.chat_id,
        ),
        STOP_COMMAND_TOOL => command_tools::stop_command(
            workspace_path,
            arguments,
            cancellation_token,
            background_commands,
            context.chat_id,
        ),
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
    // Same line-ending rules as numbered_content / read ranges (`\r\n`, `\n`, `\r`).
    output_budget::complete_line_count(content)
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
    let mut child = spawn_foreground_command_process(command, args, cwd).map_err(|source| {
        ToolRuntimeError::Command {
            command: command_label.clone(),
            source,
        }
    })?;
    let pid = child.id();
    let stdout = match child.stdout().take() {
        Some(stdout) => stdout,
        None => {
            terminate_command_process_tree(&mut child);
            return Err(ToolRuntimeError::InvalidArguments(
                "failed to capture stdout".to_string(),
            ));
        }
    };
    let stderr = match child.stderr().take() {
        Some(stderr) => stderr,
        None => {
            terminate_command_process_tree(&mut child);
            return Err(ToolRuntimeError::InvalidArguments(
                "failed to capture stderr".to_string(),
            ));
        }
    };
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
            terminate_command_process_tree(&mut child);

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
            terminate_command_process_tree(&mut child);
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
            terminate_command_process_tree(&mut child);
            return Err(error);
        }

        if exit_status.is_none() {
            match child.try_wait() {
                Ok(status) => exit_status = status,
                Err(source) => {
                    // try_wait failure is rare, but the process tree may still be running.
                    terminate_command_process_tree(&mut child);
                    return Err(ToolRuntimeError::Command {
                        command: command_label,
                        source,
                    });
                }
            }
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
            terminate_command_process_tree(&mut child);

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

/// Spawn a foreground command with the same process-tree boundary as managed background commands:
/// Unix process group leader, Windows Job Object (+ CREATE_NO_WINDOW).
fn spawn_foreground_command_process(
    command: &str,
    args: &[String],
    cwd: &Path,
) -> io::Result<Box<dyn ChildWrapper>> {
    let mut command_process = Command::new(command);
    command_process
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut command = CommandWrap::from(command_process);

    #[cfg(unix)]
    command.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    {
        command.wrap(CreationFlags(CREATE_NO_WINDOW));
        command.wrap(JobObject);
    }

    command.spawn()
}

/// Terminate the entire command process tree (process group / Job Object), not only the direct child.
fn terminate_command_process_tree(child: &mut Box<dyn ChildWrapper>) {
    let _ = child.start_kill();
    let _ = child.wait();
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
        NewCodeGraphEdge, NewCodeGraphFileIndex, NewCodeGraphImport, NewCodeGraphImportResolution,
        NewCodeGraphReference, NewCodeGraphSymbol, NewPlan, NewPlanPhase, NewPlanStep,
        WORKSPACE_SPEC_MAX_MARKDOWN_BYTES, WorkspaceDatabase,
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

    fn execute_builtin_tool_with_runtime(
        workspace_path: &Path,
        runtime: &BuiltinToolRuntime,
        tool_name: &str,
        arguments: Value,
    ) -> ToolExecution {
        execute_builtin_tool_with_context_and_execution_options(
            workspace_path,
            BuiltinToolContext::default(),
            tool_name,
            arguments,
            BuiltinToolExecutionOptions {
                runtime: runtime.clone(),
                ..BuiltinToolExecutionOptions::default()
            },
        )
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
    fn rejects_full_file_read_larger_than_source_protection() {
        let workspace = tempfile::tempdir().expect("workspace");
        let path = workspace.path().join("large.txt");
        let file = fs::File::create(&path).expect("create large file");
        file.set_len(MAX_RANGED_READ_SOURCE_BYTES + 1)
            .expect("set_len beyond ordinary source protection");

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
            error.contains(&format!("max {MAX_RANGED_READ_SOURCE_BYTES}")),
            "{error}"
        );
        assert!(error.contains("startLine/endLine"), "{error}");
        assert!(error.contains("large.txt"), "{error}");
    }

    #[test]
    fn full_file_read_over_soft_budget_returns_truncated_success() {
        let workspace = tempfile::tempdir().expect("workspace");
        // Many short lines so soft byte/line budgets truncate with more content remaining.
        let line = "abcdefghijklmnopqrstuvwxyz0123456789\n";
        let mut content = String::new();
        while content.len() < 60 * 1024 {
            content.push_str(line);
        }
        assert!(content.len() < MAX_RANGED_READ_SOURCE_BYTES as usize);
        fs::write(workspace.path().join("soft.txt"), &content).expect("write soft file");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "soft.txt", "startLine": null, "endLine": null }),
        );

        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["truncated"], true);
        let next = result.output["nextStartLine"]
            .as_u64()
            .expect("nextStartLine") as usize;
        assert!(next >= 2, "nextStartLine={next}");
        let body = result.output["content"].as_str().expect("content");
        assert!(body.starts_with("1\t"));
        assert!(body.len() <= output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT);
        assert!(
            result.output["returnedLines"].as_u64().unwrap_or(0) >= 1,
            "{:?}",
            result.output
        );
        assert!(
            result.output["note"]
                .as_str()
                .unwrap_or_default()
                .contains("nextStartLine"),
            "{:?}",
            result.output["note"]
        );

        // Full ToolExecution JSON (path/bytes/lines/note) must stay under the soft envelope when
        // multi-line soft truncation applied; normalize must preserve truncated success.
        let measured = output_budget::serialized_json_size(&result).expect("measure");
        assert!(
            measured <= output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT,
            "measured={measured}"
        );
        let normalized = output_budget::normalize_tool_execution(
            READ_FILE_TOOL,
            output_budget::ToolOutputSemantics::ReadOnly,
            result.clone(),
        );
        assert!(!normalized.execution.is_error, "{:?}", normalized.execution);
        assert_eq!(normalized.execution.output["truncated"], true);

        // Page with nextStartLine until complete; reassemble unnumbered lines and match original.
        let end_line = count_text_lines(&content);
        let expected_numbered = numbered_content(&content, 1);
        let mut reassembled = body.to_string();
        let mut cursor = next;
        let last_first = result.output["lastReturnedLine"].as_u64().expect("last") as usize;
        assert_eq!(last_first + 1, next);
        for _ in 0..32 {
            let page = execute_builtin_tool(
                workspace.path(),
                READ_FILE_TOOL,
                json!({
                    "path": "soft.txt",
                    "startLine": cursor,
                    "endLine": end_line,
                }),
            );
            assert!(!page.is_error, "{:?}", page.output);
            let page_body = page.output["content"].as_str().expect("page content");
            assert!(
                page_body.starts_with(&format!("{cursor}\t")),
                "page should start at absolute line {cursor}: {}",
                &page_body[..page_body.len().min(40)]
            );
            reassembled.push_str(page_body);
            if page.output["truncated"].as_bool() != Some(true) {
                assert_eq!(page.output["truncated"], false);
                break;
            }
            let page_last = page.output["lastReturnedLine"]
                .as_u64()
                .expect("lastReturnedLine") as usize;
            let page_next = page.output["nextStartLine"]
                .as_u64()
                .expect("nextStartLine") as usize;
            assert_eq!(page_last + 1, page_next);
            cursor = page_next;
        }
        assert_eq!(reassembled, expected_numbered);
    }

    #[test]
    fn ordinary_file_without_trailing_newline_truncates_and_continues() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut content = String::new();
        for i in 0..2500 {
            if i > 0 {
                content.push('\n');
            }
            content.push_str(&format!("row-{i}-payload-xxxxxxxx"));
        }
        // No final newline on last line.
        assert!(!content.ends_with('\n'));
        fs::write(workspace.path().join("no-nl.txt"), &content).expect("write");

        let first = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "no-nl.txt", "startLine": null, "endLine": null }),
        );
        assert!(!first.is_error, "{:?}", first.output);
        assert_eq!(first.output["truncated"], true);
        let next = first.output["nextStartLine"].as_u64().expect("next") as usize;
        let end_line = count_text_lines(&content);
        let second = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({
                "path": "no-nl.txt",
                "startLine": next,
                "endLine": end_line,
            }),
        );
        assert!(!second.is_error, "{:?}", second.output);
        let second_body = second.output["content"].as_str().expect("content");
        assert!(second_body.contains(&format!("{end_line}\t")));
        assert!(second_body.contains("row-2499-payload"));
    }

    #[test]
    fn full_file_read_between_former_128kib_and_source_protection_truncates() {
        let workspace = tempfile::tempdir().expect("workspace");
        // Formerly blocked at 128 KiB full-read source cap; now allowed under 32 MiB with prefix.
        let line = "line-content-padding-xxxxxxxxxxxxxxxx\n";
        let mut content = String::new();
        while content.len() <= 128 * 1024 {
            content.push_str(line);
        }
        assert!(content.len() > 128 * 1024);
        assert!(content.len() < MAX_RANGED_READ_SOURCE_BYTES as usize);
        fs::write(workspace.path().join("mid.txt"), &content).expect("write mid file");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "mid.txt", "startLine": null, "endLine": null }),
        );

        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["truncated"], true);
        assert!(
            result.output["nextStartLine"].as_u64().unwrap_or(0) >= 2,
            "{:?}",
            result.output
        );
        let body = result.output["content"].as_str().expect("content");
        assert!(body.starts_with("1\t"));
        assert!(body.len() <= output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT);
    }

    #[test]
    fn reads_line_range_from_file_larger_than_128kib() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut content = String::from("needle\n");
        while content.len() <= 128 * 1024 {
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
        assert_eq!(result.output["truncated"], false);
    }

    #[test]
    fn rejects_line_range_single_line_over_hard_output_ceiling() {
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
            error.contains("too large")
                || error.contains("hard")
                || error.contains("exceeds")
                || error.contains("splitting"),
            "{error}"
        );
        assert!(error.contains("large-line.txt"), "{error}");
    }

    #[test]
    fn ranged_read_truncates_with_absolute_next_start_line() {
        let workspace = tempfile::tempdir().expect("workspace");
        let line = "abcdefghij0123456789\n";
        let mut content = String::new();
        for _ in 0..4000 {
            content.push_str(line);
        }
        fs::write(workspace.path().join("range.txt"), &content).expect("write range file");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "range.txt", "startLine": 10, "endLine": 3500 }),
        );
        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["truncated"], true);
        assert_eq!(result.output["startLine"], 10);
        assert_eq!(result.output["endLine"], 3500);
        let next = result.output["nextStartLine"]
            .as_u64()
            .expect("nextStartLine") as usize;
        let last = result.output["lastReturnedLine"]
            .as_u64()
            .expect("lastReturnedLine") as usize;
        assert_eq!(last + 1, next);
        assert!(next > 10);
        let body = result.output["content"].as_str().expect("content");
        assert!(body.starts_with("10\t"));
    }

    #[test]
    fn empty_file_read_returns_empty_content_not_truncated() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("empty.txt"), "").expect("write empty");
        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "empty.txt", "startLine": null, "endLine": null }),
        );
        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["content"], "");
        assert_eq!(result.output["truncated"], false);
        assert_eq!(result.output["returnedLines"], 0);
    }

    #[test]
    fn crlf_and_utf8_lines_truncate_on_complete_line_boundaries() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut content = String::new();
        for i in 0..3000 {
            content.push_str(&format!("行-{i}-αβγ\r\n"));
        }
        fs::write(workspace.path().join("crlf.txt"), &content).expect("write crlf");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "crlf.txt", "startLine": null, "endLine": null }),
        );
        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["truncated"], true);
        let body = result.output["content"].as_str().expect("content");
        // Numbered lines must keep CRLF endings intact (complete lines).
        assert!(body.contains("\r\n"), "expected CRLF preserved in {body:?}");
        assert!(body.is_char_boundary(body.len()));
        assert!(!body.ends_with('\r'), "must not split CRLF mid-sequence");
    }

    #[test]
    fn cr_only_file_truncates_and_continues_by_true_line_count() {
        // Review P1: lone `\r` endings must share line_spans semantics with truncation/peel.
        let workspace = tempfile::tempdir().expect("workspace");
        let total_lines = output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT + 800;
        let content = "x\r".repeat(total_lines);
        assert_eq!(count_text_lines(&content), total_lines);
        fs::write(workspace.path().join("cr-only.txt"), &content).expect("write cr-only");

        let first = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "cr-only.txt", "startLine": null, "endLine": null }),
        );
        assert!(!first.is_error, "{:?}", first.output);
        assert_eq!(first.output["truncated"], true);
        let returned = first.output["returnedLines"].as_u64().expect("returned") as usize;
        assert!(
            returned <= output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT,
            "returned={returned} must not bypass soft line limit"
        );
        assert!(returned >= 1, "{:?}", first.output);
        let next = first.output["nextStartLine"].as_u64().expect("next") as usize;
        let last = first.output["lastReturnedLine"].as_u64().expect("last") as usize;
        assert_eq!(last + 1, next);
        let body = first.output["content"].as_str().expect("content");
        assert!(body.starts_with("1\t"));
        assert!(body.contains('\r'));
        // Numbered CR-only content has no LF between source lines.
        assert!(
            !body.contains("\n"),
            "CR-only source lines must not invent LF endings: {body:?}"
        );

        // Continue and reassemble absolute numbered content without overlap/gap.
        let expected_numbered = numbered_content(&content, 1);
        let mut reassembled = body.to_string();
        let mut cursor = next;
        for _ in 0..8 {
            let page = execute_builtin_tool(
                workspace.path(),
                READ_FILE_TOOL,
                json!({
                    "path": "cr-only.txt",
                    "startLine": cursor,
                    "endLine": total_lines,
                }),
            );
            assert!(!page.is_error, "{:?}", page.output);
            let page_body = page.output["content"].as_str().expect("page content");
            assert!(
                page_body.starts_with(&format!("{cursor}\t")),
                "expected absolute line {cursor}, got {}",
                &page_body[..page_body.len().min(40)]
            );
            reassembled.push_str(page_body);
            if page.output["truncated"].as_bool() != Some(true) {
                break;
            }
            let page_last = page.output["lastReturnedLine"]
                .as_u64()
                .expect("lastReturnedLine") as usize;
            let page_next = page.output["nextStartLine"]
                .as_u64()
                .expect("nextStartLine") as usize;
            assert_eq!(page_last + 1, page_next);
            cursor = page_next;
        }
        assert_eq!(reassembled, expected_numbered);

        let normalized = output_budget::normalize_tool_execution(
            READ_FILE_TOOL,
            output_budget::ToolOutputSemantics::ReadOnly,
            first.clone(),
        );
        assert!(!normalized.execution.is_error, "{:?}", normalized.execution);
        assert_eq!(normalized.execution.output["truncated"], true);
    }

    #[test]
    fn single_soft_over_line_under_hard_returns_full_line() {
        let workspace = tempfile::tempdir().expect("workspace");
        let long = "z".repeat(output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT + 256);
        let content = format!("{long}\ntrailer\n");
        fs::write(workspace.path().join("longline.txt"), &content).expect("write longline");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "longline.txt", "startLine": null, "endLine": null }),
        );
        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["truncated"], true);
        assert_eq!(result.output["nextStartLine"], 2);
        assert_eq!(result.output["returnedLines"], 1);
        let body = result.output["content"].as_str().expect("content");
        assert!(body.starts_with("1\t"));
        assert!(body.contains(&long[..32]));
        assert!(!body.contains("trailer"));

        // Continuation from nextStartLine must succeed (real remaining content).
        let cont = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "longline.txt", "startLine": 2, "endLine": 2 }),
        );
        assert!(!cont.is_error, "{:?}", cont.output);
        assert!(
            cont.output["content"]
                .as_str()
                .expect("content")
                .contains("trailer")
        );
    }

    #[test]
    fn single_soft_over_entire_file_no_fake_next_start_line() {
        // Review P2: one soft-over line that is the entire file must not invent nextStartLine=2.
        let workspace = tempfile::tempdir().expect("workspace");
        let long = "w".repeat(output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT + 256);
        fs::write(workspace.path().join("only-long.txt"), &long).expect("write");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "only-long.txt", "startLine": null, "endLine": null }),
        );
        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["truncated"], false);
        assert_eq!(result.output["softBudgetExceeded"], true);
        assert!(
            result.output.get("nextStartLine").is_none()
                || result.output["nextStartLine"].is_null(),
            "must not invent nextStartLine past EOF: {:?}",
            result.output.get("nextStartLine")
        );
        assert_eq!(result.output["returnedLines"], 1);
        let body = result.output["content"].as_str().expect("content");
        assert!(body.contains(&long[..32]));

        let normalized = output_budget::normalize_tool_execution(
            READ_FILE_TOOL,
            output_budget::ToolOutputSemantics::ReadOnly,
            result.clone(),
        );
        assert!(!normalized.execution.is_error, "{:?}", normalized.execution);
        assert_eq!(normalized.execution.output["softBudgetExceeded"], true);
    }

    #[test]
    fn exact_soft_line_limit_with_trailing_newlines_stays_success() {
        // Review P1: "line\n" × 2000 must not become a softLineLimit failure after normalize.
        let workspace = tempfile::tempdir().expect("workspace");
        let content = "line\n".repeat(output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT);
        assert_eq!(
            count_text_lines(&content),
            output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT
        );
        fs::write(workspace.path().join("exact-lines.txt"), &content).expect("write");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": "exact-lines.txt", "startLine": null, "endLine": null }),
        );
        assert!(!result.is_error, "{:?}", result.output);
        let returned = result.output["returnedLines"]
            .as_u64()
            .expect("returnedLines") as usize;
        assert!(returned >= 1, "{:?}", result.output);
        // May be full or truncated depending on path/note line overhead, but never a soft error.
        if result.output["truncated"].as_bool() == Some(true) {
            let next = result.output["nextStartLine"].as_u64().expect("next") as usize;
            let last = result.output["lastReturnedLine"].as_u64().expect("last") as usize;
            assert_eq!(last + 1, next);
            assert!(next <= output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT + 1);
        } else {
            assert_eq!(result.output["truncated"], false);
            assert_eq!(returned, output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT);
        }

        let measured = output_budget::serialized_json_size(&result).expect("measure");
        let normalized = output_budget::normalize_tool_execution(
            READ_FILE_TOOL,
            output_budget::ToolOutputSemantics::ReadOnly,
            result.clone(),
        );
        assert!(!normalized.execution.is_error, "{:?}", normalized.execution);
        if result.output["truncated"].as_bool() == Some(true) {
            // Truncated success may exceed soft only for single soft-over lines; multi-line
            // prefixes must stay under soft after dynamic budgeting.
            if returned > 1 {
                assert!(
                    measured <= output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT,
                    "measured={measured}"
                );
            }
        } else {
            assert!(
                measured <= output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT,
                "measured={measured}"
            );
        }
    }

    #[test]
    fn long_path_metadata_still_keeps_truncated_success_under_soft_envelope() {
        // Review P2: dynamic path cost must not let multi-line truncated success exceed soft.
        let workspace = tempfile::tempdir().expect("workspace");
        let mut dir = workspace.path().to_path_buf();
        // Nested dirs to create a long relative path without hitting OS path limits hard.
        for i in 0..8 {
            dir = dir.join(format!("dir-with-a-quite-long-name-{i:02}"));
        }
        fs::create_dir_all(&dir).expect("mkdirs");
        let line = "abcdefghijklmnopqrstuvwxyz0123456789\n";
        let mut content = String::new();
        while content.len() < 60 * 1024 {
            content.push_str(line);
        }
        let file_name = "payload-with-a-long-file-name-xxxxxxxx.txt";
        fs::write(dir.join(file_name), &content).expect("write");
        let rel = dir
            .strip_prefix(workspace.path())
            .expect("strip")
            .join(file_name);
        let rel = rel.to_string_lossy().replace('\\', "/");
        assert!(rel.len() > 200, "rel path should be long: {rel}");

        let result = execute_builtin_tool(
            workspace.path(),
            READ_FILE_TOOL,
            json!({ "path": rel, "startLine": null, "endLine": null }),
        );
        assert!(!result.is_error, "{:?}", result.output);
        assert_eq!(result.output["truncated"], true);
        let returned = result.output["returnedLines"].as_u64().unwrap_or(0);
        assert!(returned >= 1, "{:?}", result.output);
        if returned > 1 {
            let measured = output_budget::serialized_json_size(&result).expect("measure");
            assert!(
                measured <= output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT,
                "measured={measured} path_len={}",
                rel.len()
            );
        }
        let normalized = output_budget::normalize_tool_execution(
            READ_FILE_TOOL,
            output_budget::ToolOutputSemantics::ReadOnly,
            result.clone(),
        );
        assert!(!normalized.execution.is_error, "{:?}", normalized.execution);
        assert_eq!(normalized.execution.output["truncated"], true);
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
    fn get_plans_schema_limits_page_size_and_limit_and_accepts_next_offset() {
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

        let offset = &properties["offset"];
        assert_eq!(offset["minimum"], 0);
        assert!(offset["type"].as_array().is_some_and(|types| {
            types.iter().any(|value| value == "integer")
                && types.iter().any(|value| value == "null")
        }));
        assert!(
            definition.input_schema["required"]
                .as_array()
                .is_some_and(|required| required.iter().any(|value| value == "offset"))
        );
        assert!(
            definition
                .description
                .contains("takes precedence over page")
        );
        assert!(definition.description.contains("returned nextOffset"));
        assert!(definition.description.contains("view and status unchanged"));
        assert!(
            definition
                .description
                .contains("not a database index or stable snapshot cursor")
        );
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
    fn read_file_output_limits_are_documented_constants() {
        assert_eq!(MAX_RANGED_READ_OUTPUT_BYTES, 128 * 1024);
        assert_eq!(MAX_RANGED_READ_SOURCE_BYTES, 32 * 1024 * 1024);
        assert_eq!(output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT, 50 * 1024);
        assert_eq!(output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT, 2_000);
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
                "offset": null,
                "timeoutMs": null
            }),
        );
        assert!(!default_page.is_error, "{:?}", default_page.output);
        assert_eq!(default_page.output["pageSize"], 10);
        assert_eq!(default_page.output["offset"], 0);
        assert_eq!(default_page.output["returnedCount"], 10);
        assert_eq!(default_page.output["plans"].as_array().unwrap().len(), 10);
        assert_eq!(default_page.output["plans"][0]["id"], "plan-11");
        assert_eq!(default_page.output["truncated"], false);
        assert_eq!(default_page.output["hasMore"], true);
        assert_eq!(default_page.output["nextOffset"], 10);

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
                    "offset": null,
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
                "offset": null,
                "timeoutMs": null
            }),
        );
        assert!(!page_size_wins.is_error, "{:?}", page_size_wins.output);
        assert_eq!(page_size_wins.output["pageSize"], 3);
        assert_eq!(page_size_wins.output["plans"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn get_plans_uses_page_pagination_when_offset_is_null() {
        let workspace = tempfile::tempdir().expect("workspace");
        for index in 0..5 {
            insert_test_plan(workspace.path(), &format!("offset-plan-{index:02}"), None);
        }

        let page = execute_builtin_tool(
            workspace.path(),
            GET_PLANS_TOOL,
            json!({
                "view": "active",
                "status": null,
                "page": 2,
                "pageSize": 2,
                "limit": null,
                "offset": null,
                "timeoutMs": null
            }),
        );
        assert!(!page.is_error, "{:?}", page.output);
        assert_eq!(page.output["page"], 2);
        assert_eq!(page.output["offset"], 2);
        assert_eq!(page.output["returnedCount"], 2);
        assert_eq!(page.output["truncated"], false);
        assert_eq!(page.output["hasMore"], true);
        assert_eq!(page.output["nextOffset"], 4);
        assert_eq!(page.output["totalCount"], 5);
        assert_eq!(page.output["totalPages"], 3);
        assert_eq!(
            page.output["plans"]
                .as_array()
                .expect("plans")
                .iter()
                .map(|plan| plan["id"].as_str().expect("plan id"))
                .collect::<Vec<_>>(),
            vec!["offset-plan-02", "offset-plan-01"]
        );
    }

    #[test]
    fn get_plans_offset_takes_precedence_over_page() {
        let workspace = tempfile::tempdir().expect("workspace");
        for index in 0..5 {
            insert_test_plan(workspace.path(), &format!("offset-plan-{index:02}"), None);
        }

        let offset_page = execute_builtin_tool(
            workspace.path(),
            GET_PLANS_TOOL,
            json!({
                "view": "active",
                "status": null,
                "page": 1,
                "pageSize": 2,
                "limit": null,
                "offset": 3,
                "timeoutMs": null
            }),
        );
        assert!(!offset_page.is_error, "{:?}", offset_page.output);
        assert_eq!(offset_page.output["page"], 1);
        assert_eq!(offset_page.output["offset"], 3);
        assert_eq!(offset_page.output["returnedCount"], 2);
        assert_eq!(offset_page.output["hasMore"], false);
        assert_eq!(offset_page.output["nextOffset"], Value::Null);
        assert_eq!(
            offset_page.output["plans"]
                .as_array()
                .expect("plans")
                .iter()
                .map(|plan| plan["id"].as_str().expect("plan id"))
                .collect::<Vec<_>>(),
            vec!["offset-plan-01", "offset-plan-00"]
        );
    }

    #[test]
    fn get_plans_rejects_negative_offset() {
        let workspace = tempfile::tempdir().expect("workspace");
        let negative_offset = execute_builtin_tool(
            workspace.path(),
            GET_PLANS_TOOL,
            json!({
                "view": "active",
                "status": null,
                "page": null,
                "pageSize": null,
                "limit": null,
                "offset": -1,
                "timeoutMs": null
            }),
        );
        assert!(negative_offset.is_error);
        assert_error_contains(&negative_offset, "offset must be a non-negative integer");
    }

    #[test]
    fn get_plans_reports_empty_and_final_page_continuation_metadata() {
        let workspace = tempfile::tempdir().expect("workspace");

        let empty = execute_builtin_tool(
            workspace.path(),
            GET_PLANS_TOOL,
            json!({
                "view": "active",
                "status": null,
                "page": 1,
                "pageSize": 2,
                "limit": null,
                "offset": null,
                "timeoutMs": null
            }),
        );
        assert!(!empty.is_error, "{:?}", empty.output);
        assert_eq!(empty.output["plans"], json!([]));
        assert_eq!(empty.output["offset"], 0);
        assert_eq!(empty.output["returnedCount"], 0);
        assert_eq!(empty.output["truncated"], false);
        assert_eq!(empty.output["hasMore"], false);
        assert_eq!(empty.output["nextOffset"], Value::Null);

        for index in 0..3 {
            insert_test_plan(
                workspace.path(),
                &format!("final-page-plan-{index:02}"),
                None,
            );
        }
        let final_page = execute_builtin_tool(
            workspace.path(),
            GET_PLANS_TOOL,
            json!({
                "view": "active",
                "status": null,
                "page": 2,
                "pageSize": 2,
                "limit": null,
                "offset": null,
                "timeoutMs": null
            }),
        );
        assert!(!final_page.is_error, "{:?}", final_page.output);
        assert_eq!(final_page.output["offset"], 2);
        assert_eq!(final_page.output["returnedCount"], 1);
        assert_eq!(final_page.output["truncated"], false);
        assert_eq!(final_page.output["hasMore"], false);
        assert_eq!(final_page.output["nextOffset"], Value::Null);
        assert_eq!(final_page.output["plans"][0]["id"], "final-page-plan-00");
    }

    #[test]
    fn get_plans_budget_prefix_uses_returned_count_for_next_offset() {
        let workspace = tempfile::tempdir().expect("workspace");
        for index in 0..4 {
            insert_test_plan_with_overview(
                workspace.path(),
                &format!("budget-plan-{index:02}"),
                "p".repeat(24 * 1024),
            );
        }

        let mut offset = 0_i64;
        let mut seen_plan_ids = Vec::new();
        let mut saw_budget_truncation = false;
        for _ in 0..4 {
            let result = execute_builtin_tool(
                workspace.path(),
                GET_PLANS_TOOL,
                json!({
                    "view": "active",
                    "status": null,
                    "page": 1,
                    "pageSize": 10,
                    "limit": null,
                    "offset": offset,
                    "timeoutMs": null
                }),
            );
            assert!(!result.is_error, "{:?}", result.output);
            assert_eq!(result.output["offset"], offset);
            let plans = result.output["plans"].as_array().expect("plans");
            let returned_count = plans.len() as i64;
            assert!(returned_count > 0);
            assert_eq!(result.output["returnedCount"], returned_count);
            let measurement =
                output_budget::measure_tool_execution(&result).expect("measure result");
            assert!(
                measurement.serialized_bytes
                    <= output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT
                        .saturating_sub(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES)
            );
            assert!(measurement.text_lines <= output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT);
            assert!(
                measurement.serialized_bytes
                    <= output_budget::TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
            );
            let normalized = output_budget::normalize_tool_execution_for_envelope(
                GET_PLANS_TOOL,
                output_budget::ToolOutputSemantics::ReadOnly,
                result.clone(),
                |execution| {
                    output_budget::serialized_json_size(execution).map(|serialized_bytes| {
                        serialized_bytes
                            .saturating_add(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES)
                    })
                },
            );
            assert_eq!(
                normalized.state,
                output_budget::ToolOutputBudgetState::WithinBudget,
                "{:?}",
                normalized.execution
            );
            assert!(!normalized.execution.is_error, "{:?}", normalized.execution);
            saw_budget_truncation |= result.output["truncated"] == true;
            seen_plan_ids.extend(
                plans
                    .iter()
                    .filter_map(|plan| plan["id"].as_str().map(ToOwned::to_owned)),
            );

            let Some(next_offset) = result.output["nextOffset"].as_i64() else {
                assert!(!result.output["hasMore"].as_bool().expect("hasMore"));
                break;
            };
            assert_eq!(next_offset, offset + returned_count);
            assert!(result.output["hasMore"].as_bool().expect("hasMore"));
            offset = next_offset;
        }

        assert!(saw_budget_truncation);
        assert_eq!(
            seen_plan_ids,
            vec![
                "budget-plan-03".to_string(),
                "budget-plan-02".to_string(),
                "budget-plan-01".to_string(),
                "budget-plan-00".to_string(),
            ]
        );
    }

    #[test]
    fn get_plans_rejects_a_single_record_that_exceeds_the_byte_or_line_budget() {
        let byte_workspace = tempfile::tempdir().expect("byte workspace");
        insert_test_plan_with_overview(
            byte_workspace.path(),
            "oversized-byte-plan",
            "x".repeat(output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT),
        );
        let byte_result = execute_builtin_tool(
            byte_workspace.path(),
            GET_PLANS_TOOL,
            json!({
                "view": "active",
                "status": null,
                "page": null,
                "pageSize": null,
                "limit": null,
                "offset": null,
                "timeoutMs": null
            }),
        );
        assert!(byte_result.is_error);
        assert_error_contains(&byte_result, "get_plans_plan_record_exceeds_output_budget");
        assert_error_contains(&byte_result, "oversized-byte-plan");
        assert!(byte_result.output.get("nextOffset").is_none());
        let normalized_byte = output_budget::normalize_tool_execution_for_envelope(
            GET_PLANS_TOOL,
            output_budget::ToolOutputSemantics::ReadOnly,
            byte_result.clone(),
            |execution| {
                output_budget::serialized_json_size(execution).map(|serialized_bytes| {
                    serialized_bytes
                        .saturating_add(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES)
                })
            },
        );
        assert_eq!(
            normalized_byte.state,
            output_budget::ToolOutputBudgetState::WithinBudget
        );
        assert_error_contains(
            &normalized_byte.execution,
            "get_plans_plan_record_exceeds_output_budget",
        );
        assert!(
            output_budget::serialized_json_size(&normalized_byte.execution)
                .expect("measure normalized byte error")
                .saturating_add(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES)
                <= output_budget::TOOL_EXECUTION_HARD_BYTE_LIMIT
        );

        let line_workspace = tempfile::tempdir().expect("line workspace");
        insert_test_plan_with_overview(
            line_workspace.path(),
            "oversized-line-plan",
            "line\n".repeat(output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT + 1),
        );
        let line_result = execute_builtin_tool(
            line_workspace.path(),
            GET_PLANS_TOOL,
            json!({
                "view": "active",
                "status": null,
                "page": null,
                "pageSize": null,
                "limit": null,
                "offset": null,
                "timeoutMs": null
            }),
        );
        assert!(line_result.is_error);
        assert_error_contains(&line_result, "get_plans_plan_record_exceeds_output_budget");
        assert_error_contains(&line_result, "oversized-line-plan");
        assert!(line_result.output.get("nextOffset").is_none());
        let normalized_line = output_budget::normalize_tool_execution_for_envelope(
            GET_PLANS_TOOL,
            output_budget::ToolOutputSemantics::ReadOnly,
            line_result.clone(),
            |execution| {
                output_budget::serialized_json_size(execution).map(|serialized_bytes| {
                    serialized_bytes
                        .saturating_add(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES)
                })
            },
        );
        assert_eq!(
            normalized_line.state,
            output_budget::ToolOutputBudgetState::WithinBudget
        );
        assert_error_contains(
            &normalized_line.execution,
            "get_plans_plan_record_exceeds_output_budget",
        );
        assert!(
            output_budget::serialized_json_size(&normalized_line.execution)
                .expect("measure normalized line error")
                .saturating_add(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES)
                <= output_budget::TOOL_EXECUTION_HARD_BYTE_LIMIT
        );
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
            run_id: None,
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
            run_id: None,
            session_mode: Some("plan"),
        };
        let other_context = BuiltinToolContext {
            chat_id: Some("chat-other"),
            run_id: None,
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
        insert_test_plan_with_overview_and_source_chat(
            workspace_path,
            plan_id,
            "Plan inserted directly for ownership checks.".to_string(),
            source_chat_id,
        );
    }

    fn insert_test_plan_with_overview(workspace_path: &Path, plan_id: &str, overview: String) {
        insert_test_plan_with_overview_and_source_chat(workspace_path, plan_id, overview, None);
    }

    fn insert_test_plan_with_overview_and_source_chat(
        workspace_path: &Path,
        plan_id: &str,
        overview: String,
        source_chat_id: Option<&str>,
    ) {
        let mut database = WorkspaceDatabase::open_or_create(workspace_path).expect("database");
        database
            .create_plan(NewPlan {
                id: plan_id,
                title: "Historical plan",
                overview: &overview,
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
        assert_eq!(
            callees.output["relationshipSemantics"],
            "static_call_site_approximation"
        );

        let children = execute_builtin_tool(
            workspace.path(),
            GRAPH_FIND_CHILDREN_TOOL,
            json!({ "symbolId": public_api_id, "kind": "function", "limit": 5 }),
        );

        assert!(!children.is_error);
        assert_eq!(children.output["children"][0]["name"], "helper");

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
    fn graph_import_tools_return_exact_resolution_and_exact_reverse_dependencies() {
        let workspace = tempfile::tempdir().expect("workspace");
        insert_graph_fixture(workspace.path());
        let mut database = WorkspaceDatabase::open_or_create(workspace.path()).expect("database");
        let snapshot = database
            .code_graph_resolver_snapshot()
            .expect("resolver snapshot");
        let import = snapshot
            .imports
            .iter()
            .find(|import| import.path == "caller.rs")
            .expect("caller import");
        let target_file = snapshot
            .files
            .iter()
            .find(|file| file.path == "lib.rs")
            .expect("target file");
        let target_symbol = snapshot
            .symbols
            .iter()
            .find(|symbol| symbol.file_id == target_file.id && symbol.name == "public_api")
            .expect("target symbol");
        let resolutions = [NewCodeGraphImportResolution {
            import_id: import.id,
            resolution: "exact",
            target_file_id: Some(target_file.id),
            target_symbol_id: Some(target_symbol.id),
            candidates: &[],
            candidates_json: "[]",
            metadata_json: r#"{"provenance":"module_resolver","confidence":"exact"}"#,
        }];
        database
            .replace_code_graph_import_resolutions(&resolutions, &[])
            .expect("store resolution");

        let imports = execute_builtin_tool(
            workspace.path(),
            GRAPH_FIND_IMPORTS_TOOL,
            json!({ "path": "caller.rs", "resolved": null, "limit": null, "timeoutMs": null }),
        );
        let importers = execute_builtin_tool(
            workspace.path(),
            GRAPH_FIND_IMPORTERS_TOOL,
            json!({ "path": "lib.rs", "limit": null, "timeoutMs": null }),
        );

        assert!(!imports.is_error, "{:?}", imports.output);
        assert_eq!(imports.output["imports"][0]["resolution"], "exact");
        assert_eq!(imports.output["imports"][0]["targetPath"], "lib.rs");
        assert_eq!(
            imports.output["imports"][0]["targetSymbol"]["name"],
            "public_api"
        );
        assert!(!importers.is_error, "{:?}", importers.output);
        assert_eq!(importers.output["importers"][0]["sourcePath"], "caller.rs");
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
        assert!(
            run_command
                .description
                .contains("outputTruncated only means")
        );

        let command_output = definitions
            .iter()
            .find(|definition| definition.name == GET_COMMAND_OUTPUT_TOOL)
            .expect("get_command_output definition");
        assert!(
            command_output
                .description
                .contains("hasMore=true and truncated=true")
        );
        assert!(command_output.description.contains("cursor=nextCursor"));

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
        let runtime = BuiltinToolRuntime::default();
        let command = std::env::current_exe()
            .expect("current test executable")
            .to_string_lossy()
            .to_string();
        let started = Instant::now();
        let launch = execute_builtin_tool_with_runtime(
            workspace.path(),
            &runtime,
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
        let read = execute_builtin_tool_with_runtime(
            workspace.path(),
            &runtime,
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

        let first_stop = execute_builtin_tool_with_runtime(
            workspace.path(),
            &runtime,
            STOP_COMMAND_TOOL,
            json!({ "processId": process_id, "timeoutMs": null }),
        );
        assert!(!first_stop.is_error, "{:?}", first_stop.output);
        assert_ne!(first_stop.output["status"], json!("running"));
        assert!(first_stop.output["endedAt"].as_u64().is_some());
        let second_stop = execute_builtin_tool_with_runtime(
            workspace.path(),
            &runtime,
            STOP_COMMAND_TOOL,
            json!({ "processId": first_stop.output["processId"], "timeoutMs": null }),
        );
        assert!(!second_stop.is_error, "{:?}", second_stop.output);
        assert_ne!(second_stop.output["status"], json!("running"));
    }

    #[cfg(unix)]
    #[test]
    fn managed_command_stop_timeout_is_a_tool_error_not_a_running_snapshot() {
        let workspace = tempfile::tempdir().expect("workspace");
        let runtime = BuiltinToolRuntime::default();
        let launch = execute_builtin_tool_with_runtime(
            workspace.path(),
            &runtime,
            RUN_COMMAND_TOOL,
            json!({
                "command": "sh",
                "args": ["-c", "trap '' TERM; while :; do sleep 30; done"],
                "cwd": null,
                "timeoutMs": null,
                "background": true,
                "backgroundTimeoutMs": null
            }),
        );
        assert!(!launch.is_error, "{:?}", launch.output);

        let timed_out = execute_builtin_tool_with_runtime(
            workspace.path(),
            &runtime,
            STOP_COMMAND_TOOL,
            json!({ "processId": launch.output["processId"], "timeoutMs": 1 }),
        );
        let stopped = execute_builtin_tool_with_runtime(
            workspace.path(),
            &runtime,
            STOP_COMMAND_TOOL,
            json!({ "processId": launch.output["processId"], "timeoutMs": null }),
        );

        assert!(timed_out.is_error, "{:?}", timed_out.output);
        assert_eq!(
            timed_out.output["error"],
            json!("managed command stop timed out after 1 ms")
        );
        assert!(!stopped.is_error, "{:?}", stopped.output);
        assert_ne!(stopped.output["status"], json!("running"));
    }

    #[test]
    fn managed_command_hides_cross_workspace_handle_existence() {
        let owner_workspace = tempfile::tempdir().expect("owner workspace");
        let other_workspace = tempfile::tempdir().expect("other workspace");
        let runtime = BuiltinToolRuntime::default();
        let command = std::env::current_exe()
            .expect("current test executable")
            .to_string_lossy()
            .to_string();
        let launch = execute_builtin_tool_with_runtime(
            owner_workspace.path(),
            &runtime,
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

        let foreign = execute_builtin_tool_with_runtime(
            other_workspace.path(),
            &runtime,
            GET_COMMAND_OUTPUT_TOOL,
            json!({
                "processId": process_id,
                "cursor": null,
                "waitMs": null,
                "timeoutMs": null
            }),
        );
        let missing = execute_builtin_tool_with_runtime(
            other_workspace.path(),
            &runtime,
            GET_COMMAND_OUTPUT_TOOL,
            json!({
                "processId": "command-does-not-exist",
                "cursor": null,
                "waitMs": null,
                "timeoutMs": null
            }),
        );
        let stop = execute_builtin_tool_with_runtime(
            owner_workspace.path(),
            &runtime,
            STOP_COMMAND_TOOL,
            json!({ "processId": launch.output["processId"], "timeoutMs": null }),
        );

        assert!(foreign.is_error, "{:?}", foreign.output);
        assert!(missing.is_error, "{:?}", missing.output);
        assert_eq!(foreign.output["error"], missing.output["error"]);
        assert!(!stop.is_error, "{:?}", stop.output);
    }

    #[test]
    fn managed_command_hides_cross_chat_handle_existence() {
        let workspace = tempfile::tempdir().expect("workspace");
        let runtime = BuiltinToolRuntime::default();
        let command = std::env::current_exe()
            .expect("current test executable")
            .to_string_lossy()
            .to_string();
        let launch = execute_builtin_tool_with_context_and_execution_options(
            workspace.path(),
            BuiltinToolContext::for_chat(Some("chat-owner")),
            RUN_COMMAND_TOOL,
            json!({
                "command": command,
                "args": ["--ignored", "--exact", "tests::timeout_child_process"],
                "cwd": null,
                "timeoutMs": null,
                "background": true,
                "backgroundTimeoutMs": null
            }),
            BuiltinToolExecutionOptions {
                runtime: runtime.clone(),
                ..BuiltinToolExecutionOptions::default()
            },
        );
        assert!(!launch.is_error, "{:?}", launch.output);
        let process_id = launch.output["processId"]
            .as_str()
            .expect("process id")
            .to_string();

        let foreign_output = execute_builtin_tool_with_context_and_execution_options(
            workspace.path(),
            BuiltinToolContext::for_chat(Some("chat-other")),
            GET_COMMAND_OUTPUT_TOOL,
            json!({
                "processId": process_id,
                "cursor": null,
                "waitMs": null,
                "timeoutMs": null
            }),
            BuiltinToolExecutionOptions {
                runtime: runtime.clone(),
                ..BuiltinToolExecutionOptions::default()
            },
        );
        let foreign_stop = execute_builtin_tool_with_context_and_execution_options(
            workspace.path(),
            BuiltinToolContext::for_chat(Some("chat-other")),
            STOP_COMMAND_TOOL,
            json!({ "processId": launch.output["processId"], "timeoutMs": null }),
            BuiltinToolExecutionOptions {
                runtime: runtime.clone(),
                ..BuiltinToolExecutionOptions::default()
            },
        );
        let missing = execute_builtin_tool_with_context_and_execution_options(
            workspace.path(),
            BuiltinToolContext::for_chat(Some("chat-other")),
            GET_COMMAND_OUTPUT_TOOL,
            json!({
                "processId": "command-does-not-exist",
                "cursor": null,
                "waitMs": null,
                "timeoutMs": null
            }),
            BuiltinToolExecutionOptions {
                runtime: runtime.clone(),
                ..BuiltinToolExecutionOptions::default()
            },
        );
        let owner_stop = execute_builtin_tool_with_context_and_execution_options(
            workspace.path(),
            BuiltinToolContext::for_chat(Some("chat-owner")),
            STOP_COMMAND_TOOL,
            json!({ "processId": launch.output["processId"], "timeoutMs": null }),
            BuiltinToolExecutionOptions {
                runtime: runtime.clone(),
                ..BuiltinToolExecutionOptions::default()
            },
        );

        assert!(foreign_output.is_error, "{:?}", foreign_output.output);
        assert!(foreign_stop.is_error, "{:?}", foreign_stop.output);
        assert!(missing.is_error, "{:?}", missing.output);
        assert_eq!(foreign_output.output["error"], missing.output["error"]);
        assert_eq!(foreign_stop.output["error"], missing.output["error"]);
        assert!(!owner_stop.is_error, "{:?}", owner_stop.output);
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
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
        );
        assert!(!initial.is_error);
        assert_eq!(initial.output["enabled"], false);
        assert_eq!(initial.output["injectEnabled"], false);
        assert_eq!(initial.output["revision"], 0);
        assert_eq!(initial.output["contentMarkdown"], "");
        assert_eq!(initial.output["generatedAt"], Value::Null);
        assert_eq!(initial.output["updatedAt"], Value::Null);
        assert_eq!(initial.output["truncated"], false);
        assert_eq!(initial.output["totalLines"], 0);
        assert_eq!(initial.output["totalBytes"], 0);

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
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
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
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
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
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
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
                json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
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
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
        );
        assert_eq!(read_back.output["revision"], 1);
        assert_eq!(read_back.output["contentMarkdown"], initial_content);
    }

    #[test]
    fn read_spec_schema_documents_continuation_fields() {
        let definition = builtin_tool_definitions()
            .into_iter()
            .find(|definition| definition.name == READ_SPEC_TOOL)
            .expect("read_spec definition");
        let properties = definition.input_schema["properties"]
            .as_object()
            .expect("read_spec properties");
        let required = definition.input_schema["required"]
            .as_array()
            .expect("read_spec required");

        assert!(definition.strict);
        assert_eq!(definition.input_schema["additionalProperties"], false);
        assert_eq!(properties["startLine"]["type"], json!(["integer", "null"]));
        assert_eq!(
            properties["expectedRevision"]["type"],
            json!(["integer", "null"])
        );
        assert_eq!(
            required,
            &vec![
                json!("startLine"),
                json!("expectedRevision"),
                json!("timeoutMs"),
            ]
        );
        assert!(definition.description.contains("nextStartLine"));
        assert!(definition.description.contains("expectedRevision"));
        assert!(definition.description.contains("truncated=true"));
    }

    #[test]
    fn read_spec_paginates_large_markdown_and_pins_revision() {
        let workspace = tempfile::tempdir().expect("workspace");
        // ~55 KiB of multi-line markdown so the full ToolExecution exceeds the 50 KiB soft budget.
        let line = "abcdefghijklmnopqrstuvwxyz0123456789-spec-line-padding-xx\n";
        let large = format!("# Large Spec\n\n{}", line.repeat(950));
        assert!(large.len() > 50 * 1024);

        let written = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": large,
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!written.is_error, "{}", written.output);
        assert_eq!(written.output["revision"], 1);
        // Large update success must keep metadata even when body is omitted.
        assert!(
            written.output["contentOmitted"].as_bool() == Some(true)
                || written.output.get("contentMarkdown").is_some(),
            "update_spec must return contentOmitted or contentMarkdown: {}",
            written.output
        );
        if written.output["contentOmitted"] == true {
            assert!(written.output.get("contentMarkdown").is_none());
            assert_eq!(written.output["revision"], 1);
            assert_eq!(written.output["updateMode"], "fullReplacement");
            assert_eq!(written.output["editCount"], 0);
            assert!(written.output["lineCountAfter"].as_u64().unwrap_or(0) > 0);
            assert!(
                written.output["note"]
                    .as_str()
                    .expect("note")
                    .contains("read_spec")
            );
        }

        let first = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
        );
        assert!(!first.is_error, "{}", first.output);
        assert_eq!(first.output["revision"], 1);
        assert_eq!(first.output["truncated"], true);
        let next = first.output["nextStartLine"]
            .as_u64()
            .expect("nextStartLine") as usize;
        assert!(next >= 2, "nextStartLine={next}");
        assert!(first.output["returnedLines"].as_u64().unwrap_or(0) >= 1);
        let first_chunk = first.output["contentMarkdown"]
            .as_str()
            .expect("first content")
            .to_string();
        assert!(!first_chunk.is_empty());
        assert!(first_chunk.starts_with("# Large Spec"));

        let second = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({
                "startLine": next,
                "expectedRevision": 1,
                "timeoutMs": null
            }),
        );
        assert!(!second.is_error, "{}", second.output);
        assert_eq!(second.output["revision"], 1);
        let second_chunk = second.output["contentMarkdown"]
            .as_str()
            .expect("second content")
            .to_string();

        // Stale continuation after concurrent write must fail clearly.
        let mut concurrent =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("concurrent database");
        concurrent
            .update_workspace_spec_content(1, "# Spec\n\nConcurrent")
            .expect("concurrent update")
            .expect("cas won");

        let conflict = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({
                "startLine": next,
                "expectedRevision": 1,
                "timeoutMs": null
            }),
        );
        assert!(conflict.is_error);
        assert!(
            conflict.output["error"]
                .as_str()
                .expect("conflict error")
                .contains("revision changed during read_spec continuation")
        );

        // Reassemble first two pages against the original large body (before concurrent write).
        let reassembled = format!("{first_chunk}{second_chunk}");
        assert!(
            large.starts_with(&reassembled)
                || reassembled.starts_with(&large[..reassembled.len().min(large.len())]),
            "continuation pages must be complete-line prefixes of the original body"
        );
        assert_eq!(&large[..first_chunk.len()], first_chunk.as_str());
        if !second_chunk.is_empty() {
            assert_eq!(
                &large[first_chunk.len()..first_chunk.len() + second_chunk.len()],
                second_chunk.as_str()
            );
        }
    }

    #[test]
    fn read_spec_eof_continuation_returns_empty_final_page() {
        let workspace = tempfile::tempdir().expect("workspace");
        let content = "# Spec\n\nLine three";
        let written = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": content,
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!written.is_error);

        let past_eof = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({
                "startLine": 100,
                "expectedRevision": 1,
                "timeoutMs": null
            }),
        );
        assert!(!past_eof.is_error, "{}", past_eof.output);
        assert_eq!(past_eof.output["contentMarkdown"], "");
        assert_eq!(past_eof.output["truncated"], false);
        assert_eq!(past_eof.output["returnedLines"], 0);
        assert_eq!(past_eof.output["revision"], 1);
    }

    #[test]
    fn update_spec_small_result_keeps_content_markdown() {
        let workspace = tempfile::tempdir().expect("workspace");
        let result = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": "# Small\n\nBody",
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!result.is_error, "{}", result.output);
        assert_eq!(result.output["contentOmitted"], false);
        assert_eq!(result.output["contentMarkdown"], "# Small\n\nBody");
        assert_eq!(result.output["revision"], 1);
        assert_eq!(result.output["updateMode"], "fullReplacement");
        assert_eq!(result.output["editCount"], 0);
        assert_eq!(result.output["lineCountBefore"], 0);
        assert_eq!(result.output["lineCountAfter"], 3);
        assert_eq!(result.output["totalLines"], 3);
    }

    #[test]
    fn update_spec_line_counts_use_complete_line_endings() {
        let workspace = tempfile::tempdir().expect("workspace");
        // Lone CR is a complete-line ending for pagination; lineCount* must match totalLines.
        let cr_only = "a\rb";
        let result = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": cr_only,
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!result.is_error, "{}", result.output);
        assert_eq!(result.output["contentOmitted"], false);
        assert_eq!(result.output["lineCountAfter"], 2);
        assert_eq!(result.output["totalLines"], 2);
        assert_eq!(result.output["totalBytes"], cr_only.len());

        let crlf = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 1,
                "contentMarkdown": "a\r\nb\r\n",
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!crlf.is_error, "{}", crlf.output);
        assert_eq!(crlf.output["lineCountBefore"], 2);
        assert_eq!(crlf.output["lineCountAfter"], 2);
        assert_eq!(crlf.output["totalLines"], 2);
    }

    #[test]
    fn update_spec_omits_body_when_bare_execution_fits_but_envelope_reserve_does_not() {
        let workspace = tempfile::tempdir().expect("workspace");
        // Multi-line body large enough that bare ToolExecution with contentMarkdown exceeds
        // soft minus TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES (and usually the soft limit itself).
        let line = "abcdefghijklmnopqrstuvwxyz0123456789-envelope-pad-line\n";
        let large = format!("# Envelope boundary\n\n{}", line.repeat(900));
        assert!(large.len() > 40 * 1024);

        let written = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": large,
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!written.is_error, "{}", written.output);
        assert_eq!(written.output["revision"], 1);
        assert_eq!(written.output["contentOmitted"], true);
        assert!(written.output.get("contentMarkdown").is_none());
        assert_eq!(written.output["updateMode"], "fullReplacement");
        assert_eq!(written.output["editCount"], 0);
        assert!(
            written.output["note"]
                .as_str()
                .expect("note")
                .contains("read_spec")
        );

        let measured = output_budget::measure_tool_execution(&written).expect("measure omitted");
        let soft_with_reserve = output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT
            .saturating_sub(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES);
        assert!(
            measured.serialized_bytes <= soft_with_reserve,
            "omitted success must fit soft minus envelope reserve: {} > {}",
            measured.serialized_bytes,
            soft_with_reserve
        );

        // Outer normalizer must preserve structured contentOmitted metadata (not generic omission).
        let budgeted = output_budget::normalize_tool_execution(
            UPDATE_SPEC_TOOL,
            output_budget::ToolOutputSemantics::RetryUnsafe,
            written.clone(),
        );
        assert!(!budgeted.execution.is_error);
        assert_eq!(budgeted.execution.output["contentOmitted"], true);
        assert_eq!(budgeted.execution.output["revision"], 1);
        assert!(budgeted.execution.output.get("outputOmitted").is_none());

        // Simulate a slightly larger transport envelope soft overage: still preserve metadata.
        let envelope_over = ToolExecution {
            output: {
                let mut output = written.output.clone();
                if let Some(object) = output.as_object_mut() {
                    object.insert(
                        "padding".to_string(),
                        json!(
                            "x".repeat(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES + 64)
                        ),
                    );
                }
                output
            },
            is_error: false,
        };
        let envelope_budgeted = output_budget::normalize_tool_execution_for_envelope(
            UPDATE_SPEC_TOOL,
            output_budget::ToolOutputSemantics::RetryUnsafe,
            envelope_over,
            |execution| {
                // Measure as if SSE wrapper added the padding field size already in output.
                output_budget::serialized_json_size(execution)
            },
        );
        assert!(!envelope_budgeted.execution.is_error);
        assert_eq!(envelope_budgeted.execution.output["contentOmitted"], true);
        assert_eq!(envelope_budgeted.execution.output["revision"], 1);
        assert!(
            envelope_budgeted
                .execution
                .output
                .get("outputOmitted")
                .is_none()
        );
    }

    #[test]
    fn read_spec_continuation_requires_expected_revision() {
        let workspace = tempfile::tempdir().expect("workspace");
        let written = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": "# Spec\n\nBody",
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!written.is_error);

        let missing = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({
                "startLine": 2,
                "expectedRevision": null,
                "timeoutMs": null
            }),
        );
        assert!(missing.is_error);
        assert!(
            missing.output["error"]
                .as_str()
                .expect("error")
                .contains("expectedRevision")
        );

        let zero = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({
                "startLine": 0,
                "expectedRevision": 1,
                "timeoutMs": null
            }),
        );
        assert!(zero.is_error);
        assert!(
            zero.output["error"]
                .as_str()
                .expect("error")
                .contains("positive 1-based")
        );
    }

    /// Page through `read_spec` until `truncated=false`, asserting each page fits the soft
    /// envelope (JSON-serialized ToolExecution) and reassembling `contentMarkdown` bytes.
    fn read_spec_reassemble_all_pages(
        workspace_path: &Path,
        expected_revision: u64,
        expected_content: &str,
    ) {
        let soft_with_reserve = output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT
            .saturating_sub(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES);
        let mut reassembled = String::new();
        let mut start_line: Option<u64> = None;
        let mut pages = 0_usize;
        loop {
            pages = pages.saturating_add(1);
            assert!(pages <= 64, "read_spec pagination did not terminate");
            let page = execute_builtin_tool(
                workspace_path,
                READ_SPEC_TOOL,
                json!({
                    "startLine": start_line,
                    "expectedRevision": if start_line.is_some() {
                        json!(expected_revision)
                    } else {
                        Value::Null
                    },
                    "timeoutMs": null
                }),
            );
            assert!(!page.is_error, "page {pages}: {}", page.output);
            assert_eq!(page.output["revision"], expected_revision);
            assert_eq!(page.output["totalBytes"], expected_content.len());

            let measured = output_budget::measure_tool_execution(&page).expect("measure page");
            // Multi-line soft pages must leave room for the outer SSE envelope; a single soft-over
            // line may set softBudgetExceeded and exceed soft-with-reserve intentionally.
            let soft_budget_exceeded = page.output["softBudgetExceeded"].as_bool() == Some(true);
            if !soft_budget_exceeded {
                assert!(
                    measured.serialized_bytes <= soft_with_reserve
                        || measured.serialized_bytes <= output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT,
                    "page {pages} envelope bytes {} exceed soft budgets",
                    measured.serialized_bytes
                );
                assert!(
                    measured.text_lines <= output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT,
                    "page {pages} text_lines={}",
                    measured.text_lines
                );
            }

            let normalized = output_budget::normalize_tool_execution(
                READ_SPEC_TOOL,
                output_budget::ToolOutputSemantics::ReadOnly,
                page.clone(),
            );
            assert!(
                !normalized.execution.is_error,
                "normalize must preserve read_spec success: {}",
                normalized.execution.output
            );
            assert_eq!(
                normalized.execution.output["contentMarkdown"],
                page.output["contentMarkdown"]
            );
            assert_eq!(
                normalized.execution.output["revision"],
                page.output["revision"]
            );

            // Simulate a slightly larger transport envelope measurement (SSE ToolResult wrapper).
            let envelope_budgeted = output_budget::normalize_tool_execution_for_envelope(
                READ_SPEC_TOOL,
                output_budget::ToolOutputSemantics::ReadOnly,
                page.clone(),
                |execution| {
                    let bare = output_budget::serialized_json_size(execution)?;
                    Ok(bare
                        .saturating_add(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES / 2))
                },
            );
            assert!(
                !envelope_budgeted.execution.is_error,
                "envelope normalize must preserve truncated/full success: {}",
                envelope_budgeted.execution.output
            );
            assert!(
                envelope_budgeted
                    .execution
                    .output
                    .get("outputOmitted")
                    .is_none()
            );

            let chunk = page.output["contentMarkdown"]
                .as_str()
                .expect("contentMarkdown")
                .to_string();
            reassembled.push_str(&chunk);

            if page.output["truncated"].as_bool() != Some(true) {
                assert_eq!(page.output["truncated"], false);
                assert!(
                    page.output.get("nextStartLine").is_none()
                        || page.output["nextStartLine"].is_null()
                );
                break;
            }
            let next = page.output["nextStartLine"]
                .as_u64()
                .expect("nextStartLine on truncated page");
            assert!(next >= 2 || start_line.is_some(), "nextStartLine={next}");
            start_line = Some(next);
        }

        assert_eq!(
            reassembled.as_bytes(),
            expected_content.as_bytes(),
            "reassembled contentMarkdown must match stored bytes exactly"
        );
        let db = WorkspaceDatabase::open_or_create(workspace_path).expect("db");
        let stored = db
            .workspace_spec()
            .expect("spec")
            .expect("spec present")
            .content_markdown;
        assert_eq!(reassembled.as_bytes(), stored.as_bytes());
    }

    #[test]
    fn read_spec_small_body_returns_full_page_under_soft_budget() {
        let workspace = tempfile::tempdir().expect("workspace");
        let content = "# Small Spec\n\nUnder 50 KiB body with a few lines.\n";
        assert!(content.len() < 50 * 1024);
        let written = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": content,
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!written.is_error, "{}", written.output);
        assert_eq!(written.output["contentOmitted"], false);

        let page = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
        );
        assert!(!page.is_error, "{}", page.output);
        assert_eq!(page.output["truncated"], false);
        assert_eq!(page.output["contentMarkdown"], content);
        assert_eq!(page.output["revision"], 1);
        assert_eq!(page.output["totalBytes"], content.len());
        assert!(
            page.output.get("nextStartLine").is_none() || page.output["nextStartLine"].is_null()
        );

        let measured = output_budget::measure_tool_execution(&page).expect("measure");
        assert!(measured.serialized_bytes <= output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT);
        let normalized = output_budget::normalize_tool_execution(
            READ_SPEC_TOOL,
            output_budget::ToolOutputSemantics::ReadOnly,
            page.clone(),
        );
        assert!(!normalized.execution.is_error);
        assert_eq!(normalized.execution.output["contentMarkdown"], content);
    }

    #[test]
    fn read_spec_empty_spec_returns_empty_success() {
        let workspace = tempfile::tempdir().expect("workspace");
        let page = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
        );
        assert!(!page.is_error, "{}", page.output);
        assert_eq!(page.output["revision"], 0);
        assert_eq!(page.output["contentMarkdown"], "");
        assert_eq!(page.output["truncated"], false);
        assert_eq!(page.output["totalLines"], 0);
        assert_eq!(page.output["totalBytes"], 0);
        assert_eq!(page.output["returnedLines"], 0);

        let past_eof = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({
                "startLine": 1,
                "expectedRevision": 0,
                "timeoutMs": null
            }),
        );
        assert!(!past_eof.is_error, "{}", past_eof.output);
        assert_eq!(past_eof.output["contentMarkdown"], "");
        assert_eq!(past_eof.output["truncated"], false);
    }

    #[test]
    fn read_spec_near_64kib_pages_reassemble_to_stored_bytes() {
        let workspace = tempfile::tempdir().expect("workspace");
        // Approach the 64 KiB Spec hard cap with multi-line ASCII so pagination is required.
        let line = "abcdefghijklmnopqrstuvwxyz0123456789-near-64kib-padding-line\n";
        let mut large = String::from("# Near 64 KiB Spec\n\n");
        while large.len() + line.len() < WORKSPACE_SPEC_MAX_MARKDOWN_BYTES - 64 {
            large.push_str(line);
        }
        assert!(large.len() > 50 * 1024);
        assert!(large.len() <= WORKSPACE_SPEC_MAX_MARKDOWN_BYTES);

        let written = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": large,
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!written.is_error, "{}", written.output);
        assert_eq!(written.output["revision"], 1);
        assert_eq!(written.output["contentOmitted"], true);
        assert!(written.output.get("contentMarkdown").is_none());

        read_spec_reassemble_all_pages(workspace.path(), 1, &large);
    }

    #[test]
    fn read_spec_more_than_soft_line_limit_pages_and_reassembles() {
        let workspace = tempfile::tempdir().expect("workspace");
        // > 2,000 complete lines forces soft line-limit pagination even when total bytes are modest.
        let line_count = output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT + 400;
        let mut content = String::with_capacity(line_count * 12);
        for i in 0..line_count {
            content.push_str(&format!("L{i:04}\n"));
        }
        assert!(content.len() < 50 * 1024);
        assert!(
            output_budget::complete_line_count(&content)
                > output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT
        );

        let written = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": content,
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!written.is_error, "{}", written.output);

        let first = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
        );
        assert!(!first.is_error, "{}", first.output);
        assert_eq!(first.output["truncated"], true);
        assert!(
            first.output["returnedLines"].as_u64().unwrap_or(0)
                <= output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT as u64
        );

        read_spec_reassemble_all_pages(workspace.path(), 1, &content);
    }

    #[test]
    fn read_spec_utf8_multibyte_lines_reassemble_without_splitting_chars() {
        let workspace = tempfile::tempdir().expect("workspace");
        // Multi-byte UTF-8 (3-byte CJK) so JSON string body expansion differs from raw len.
        let line = "中文规格行-测试分页与预算-αβγ\n";
        assert!(line.len() > line.chars().count());
        let mut content = String::from("# UTF-8 Spec\n\n");
        while content.len() < 52 * 1024 {
            content.push_str(line);
        }
        assert!(content.len() <= WORKSPACE_SPEC_MAX_MARKDOWN_BYTES);

        let written = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": content,
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!written.is_error, "{}", written.output);
        read_spec_reassemble_all_pages(workspace.path(), 1, &content);
    }

    #[test]
    fn read_spec_single_long_line_soft_over_returns_full_line_without_fake_continuation() {
        let workspace = tempfile::tempdir().expect("workspace");
        // One complete line over soft but under hard: softBudgetExceeded, no past-EOF nextStartLine.
        let long = format!(
            "# {}\n",
            "x".repeat(output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT + 256)
        );
        assert!(long.len() > output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT);
        assert!(long.len() < output_budget::TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT);
        assert!(long.len() <= WORKSPACE_SPEC_MAX_MARKDOWN_BYTES);

        let written = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": long,
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!written.is_error, "{}", written.output);

        let page = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
        );
        assert!(!page.is_error, "{}", page.output);
        assert_eq!(page.output["contentMarkdown"], long);
        assert_eq!(page.output["truncated"], false);
        assert_eq!(page.output["softBudgetExceeded"], true);
        assert!(
            page.output.get("nextStartLine").is_none() || page.output["nextStartLine"].is_null(),
            "must not invent nextStartLine past EOF: {:?}",
            page.output.get("nextStartLine")
        );

        let normalized = output_budget::normalize_tool_execution(
            READ_SPEC_TOOL,
            output_budget::ToolOutputSemantics::ReadOnly,
            page.clone(),
        );
        assert!(
            !normalized.execution.is_error,
            "{}",
            normalized.execution.output
        );
        assert_eq!(normalized.execution.output["softBudgetExceeded"], true);
        assert_eq!(normalized.execution.output["contentMarkdown"], long);
    }

    #[test]
    fn read_spec_last_page_and_stale_revision_restart() {
        let workspace = tempfile::tempdir().expect("workspace");
        let line = "abcdefghijklmnopqrstuvwxyz0123456789-last-page-pad\n";
        let large = format!("# Last page Spec\n\n{}", line.repeat(1100));
        assert!(large.len() > 50 * 1024);

        let written = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": large,
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!written.is_error, "{}", written.output);

        // Walk to the final non-truncated page, then request past-EOF empty final page.
        let mut start_line: Option<u64> = None;
        let mut last_next: Option<u64> = None;
        for _ in 0..64 {
            let page = execute_builtin_tool(
                workspace.path(),
                READ_SPEC_TOOL,
                json!({
                    "startLine": start_line,
                    "expectedRevision": if start_line.is_some() {
                        json!(1)
                    } else {
                        Value::Null
                    },
                    "timeoutMs": null
                }),
            );
            assert!(!page.is_error, "{}", page.output);
            if page.output["truncated"].as_bool() == Some(true) {
                last_next = Some(
                    page.output["nextStartLine"]
                        .as_u64()
                        .expect("nextStartLine"),
                );
                start_line = last_next;
                continue;
            }
            // Final content page.
            assert_eq!(page.output["truncated"], false);
            let total_lines = page.output["totalLines"].as_u64().expect("totalLines");
            let past = execute_builtin_tool(
                workspace.path(),
                READ_SPEC_TOOL,
                json!({
                    "startLine": total_lines.saturating_add(1),
                    "expectedRevision": 1,
                    "timeoutMs": null
                }),
            );
            assert!(!past.is_error, "{}", past.output);
            assert_eq!(past.output["contentMarkdown"], "");
            assert_eq!(past.output["truncated"], false);
            assert_eq!(past.output["returnedLines"], 0);
            break;
        }

        // Concurrent update: stale continuation rejected; first-page re-read sees new revision.
        let mut concurrent =
            WorkspaceDatabase::open_or_create(workspace.path()).expect("concurrent database");
        concurrent
            .update_workspace_spec_content(1, "# Spec\n\nConcurrent after pages")
            .expect("concurrent update")
            .expect("cas won");

        if let Some(stale_start) = last_next {
            let conflict = execute_builtin_tool(
                workspace.path(),
                READ_SPEC_TOOL,
                json!({
                    "startLine": stale_start,
                    "expectedRevision": 1,
                    "timeoutMs": null
                }),
            );
            assert!(conflict.is_error);
            assert!(
                conflict.output["error"]
                    .as_str()
                    .expect("error")
                    .contains("revision changed during read_spec continuation")
            );
        }

        let fresh = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
        );
        assert!(!fresh.is_error, "{}", fresh.output);
        assert_eq!(fresh.output["revision"], 2);
        assert_eq!(
            fresh.output["contentMarkdown"],
            "# Spec\n\nConcurrent after pages"
        );
        assert_eq!(fresh.output["truncated"], false);
    }

    #[test]
    fn read_spec_truncated_success_survives_normalize_and_envelope() {
        let workspace = tempfile::tempdir().expect("workspace");
        let line = "abcdefghijklmnopqrstuvwxyz0123456789-envelope-preserve-line\n";
        let large = format!("# Envelope preserve\n\n{}", line.repeat(950));
        assert!(large.len() > 50 * 1024);

        let written = execute_builtin_tool(
            workspace.path(),
            UPDATE_SPEC_TOOL,
            json!({
                "expectedRevision": 0,
                "contentMarkdown": large,
                "edits": null,
                "timeoutMs": null
            }),
        );
        assert!(!written.is_error, "{}", written.output);

        let first = execute_builtin_tool(
            workspace.path(),
            READ_SPEC_TOOL,
            json!({ "startLine": null, "expectedRevision": null, "timeoutMs": null }),
        );
        assert!(!first.is_error, "{}", first.output);
        assert_eq!(first.output["truncated"], true);
        assert!(
            first.output["nextStartLine"]
                .as_u64()
                .expect("nextStartLine")
                >= 2
        );

        let measured = output_budget::measure_tool_execution(&first).expect("measure");
        let soft_with_reserve = output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT
            .saturating_sub(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES);
        assert!(
            measured.serialized_bytes <= soft_with_reserve,
            "truncated page must fit soft minus envelope reserve: {} > {}",
            measured.serialized_bytes,
            soft_with_reserve
        );

        let budgeted = output_budget::normalize_tool_execution(
            READ_SPEC_TOOL,
            output_budget::ToolOutputSemantics::ReadOnly,
            first.clone(),
        );
        assert!(!budgeted.execution.is_error);
        assert_eq!(budgeted.execution.output["truncated"], true);
        assert_eq!(
            budgeted.execution.output["nextStartLine"],
            first.output["nextStartLine"]
        );
        assert_eq!(budgeted.execution.output["revision"], 1);
        assert!(budgeted.execution.output.get("outputOmitted").is_none());

        // Outer envelope soft overage must still preserve line-bounded truncated success.
        let envelope_over = ToolExecution {
            output: {
                let mut output = first.output.clone();
                if let Some(object) = output.as_object_mut() {
                    object.insert(
                        "padding".to_string(),
                        json!(
                            "x".repeat(output_budget::TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES + 128)
                        ),
                    );
                }
                output
            },
            is_error: false,
        };
        let envelope_budgeted = output_budget::normalize_tool_execution_for_envelope(
            READ_SPEC_TOOL,
            output_budget::ToolOutputSemantics::ReadOnly,
            envelope_over,
            |execution| output_budget::serialized_json_size(execution),
        );
        assert!(!envelope_budgeted.execution.is_error);
        assert_eq!(envelope_budgeted.execution.output["truncated"], true);
        assert_eq!(envelope_budgeted.execution.output["revision"], 1);
        assert!(
            envelope_budgeted
                .execution
                .output
                .get("outputOmitted")
                .is_none()
        );
    }

    #[test]
    fn builtin_tools_include_apply_patch_with_strict_schema() {
        let tool = builtin_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == APPLY_PATCH_TOOL)
            .expect("apply_patch tool definition");

        assert!(tool.strict);
        assert_eq!(
            tool.input_schema,
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": "A non-empty Codex patch document, beginning with *** Begin Patch and ending with *** End Patch."
                    },
                    "timeoutMs": {
                        "type": ["integer", "null"],
                        "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                    }
                },
                "required": ["patch", "timeoutMs"]
            })
        );
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

    #[cfg(unix)]
    #[test]
    fn run_command_timeout_kills_entire_unix_process_group() {
        let workspace = tempfile::tempdir().expect("workspace");
        let pid_path = workspace.path().join("grandchild.pid");
        let started = Instant::now();

        let result = execute_builtin_tool(
            workspace.path(),
            RUN_COMMAND_TOOL,
            json!({
                "command": "sh",
                "args": [
                    "-c",
                    "sleep 30 & echo $! > grandchild.pid; wait"
                ],
                "cwd": null,
                "timeoutMs": 200
            }),
        );

        // Register cleanup as soon as the pid is known so assertion failures cannot leave
        // a 30s sleep process behind.
        let grandchild_pid = wait_for_pid_file(&pid_path, Duration::from_secs(2));
        let _cleanup = ForceKillOnDrop(grandchild_pid);

        assert!(result.is_error);
        assert!(
            result.output["error"]
                .as_str()
                .expect("timeout error")
                .contains("timed out")
        );
        wait_until_process_exits(grandchild_pid, Duration::from_secs(2));
        assert!(
            !process_is_alive(grandchild_pid),
            "grandchild process should be killed with its process group on timeout"
        );
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[cfg(unix)]
    #[test]
    fn run_command_cancellation_kills_entire_unix_process_group() {
        let workspace = tempfile::tempdir().expect("workspace");
        let pid_path = workspace.path().join("grandchild.pid");
        let cancellation = ToolCancellationToken::default();
        let cancellation_trigger = cancellation.clone();
        let pid_path_for_trigger = pid_path.clone();
        let started = Instant::now();
        let trigger = thread::spawn(move || {
            // Wait until the shell has forked the long-lived grandchild and written its pid.
            let _ = wait_for_pid_file(&pid_path_for_trigger, Duration::from_secs(2));
            cancellation_trigger.cancel();
        });

        let result = execute_builtin_tool_for_chat_with_cancellation_and_output_sink(
            workspace.path(),
            None,
            RUN_COMMAND_TOOL,
            json!({
                "command": "sh",
                "args": [
                    "-c",
                    "sleep 30 & echo $! > grandchild.pid; wait"
                ],
                "cwd": null,
                "timeoutMs": 5_000
            }),
            Some(cancellation),
            None,
        );
        trigger.join().expect("join cancellation trigger");

        let grandchild_pid = wait_for_pid_file(&pid_path, Duration::from_secs(2));
        let _cleanup = ForceKillOnDrop(grandchild_pid);

        assert!(result.is_error);
        assert!(
            result.output["error"]
                .as_str()
                .expect("cancellation error")
                .contains("cancelled")
        );
        wait_until_process_exits(grandchild_pid, Duration::from_secs(2));
        assert!(
            !process_is_alive(grandchild_pid),
            "grandchild process should be killed with its process group on cancellation"
        );
        assert!(started.elapsed() < Duration::from_secs(3));
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

    #[cfg(unix)]
    fn wait_for_pid_file(path: &Path, timeout: Duration) -> u32 {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(contents) = fs::read_to_string(path)
                && let Ok(pid) = contents.trim().parse::<u32>()
                && pid > 0
            {
                return pid;
            }
            assert!(
                Instant::now() < deadline,
                "command did not report grandchild pid at {}",
                path.display()
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    fn process_is_alive(pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(unix)]
    fn wait_until_process_exits(pid: u32, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while process_is_alive(pid) {
            assert!(
                Instant::now() < deadline,
                "process {pid} did not exit within {timeout:?}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    fn force_kill_process(pid: u32) {
        let _ = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stderr(Stdio::null())
            .status();
    }

    /// Ensures a test-spawned process is force-killed even if assertions panic.
    #[cfg(unix)]
    struct ForceKillOnDrop(u32);

    #[cfg(unix)]
    impl Drop for ForceKillOnDrop {
        fn drop(&mut self) {
            force_kill_process(self.0);
        }
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
                qualified_name: "public_api",
                kind: "function",
                visibility: Some("public"),
                metadata_json: None,
                start_line: Some(1),
                start_column: Some(1),
                end_line: Some(5),
                end_column: Some(1),
                signature: Some("fn public_api()"),
                documentation: None,
            },
            NewCodeGraphSymbol {
                name: "helper",
                qualified_name: "helper",
                kind: "function",
                visibility: None,
                metadata_json: None,
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
        let lib_edges = [
            NewCodeGraphEdge {
                source_symbol_index: 0,
                target_symbol_index: 1,
                edge_kind: "calls",
                metadata_json: Some(r#"{"provenance":"tree_sitter","confidence":"exact"}"#),
            },
            NewCodeGraphEdge {
                source_symbol_index: 0,
                target_symbol_index: 1,
                edge_kind: "contains",
                metadata_json: Some(r#"{"provenance":"tree_sitter","confidence":"exact"}"#),
            },
        ];
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
            qualified_name: "caller_entry",
            kind: "function",
            visibility: None,
            metadata_json: None,
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
