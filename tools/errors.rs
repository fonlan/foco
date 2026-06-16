use std::{fmt, io, path::PathBuf};

use foco_store::workspace::WorkspaceDatabaseError;
use serde_json::{Value, json};

use crate::{
    ASK_QUESTION_TOOL, CREATE_TODO_GRAPH_TOOL, DEFAULT_FILE_TOOL_TIMEOUT_MS,
    DEFAULT_GRAPH_TOOL_TIMEOUT_MS, DEFAULT_RUN_COMMAND_TIMEOUT_MS, DEFAULT_SEARCH_TEXT_TIMEOUT_MS,
    DEFAULT_SLEEP_TIMEOUT_MS, DEFAULT_TODO_GRAPH_TIMEOUT_MS, DEFAULT_WEB_TOOL_TIMEOUT_MS,
    DEFAULT_WRITE_FILE_TIMEOUT_MS, EDIT_FILE_TOOL, FIND_FILES_TOOL, GET_TODO_GRAPH_TOOL,
    GRAPH_EXPLORE_TOOL, GRAPH_FIND_CALLEES_TOOL, GRAPH_FIND_CALLERS_TOOL,
    GRAPH_FIND_REFERENCES_TOOL, GRAPH_FIND_SYMBOLS_TOOL, GRAPH_RELATED_FILES_TOOL,
    MAX_TOOL_TIMEOUT_MS, READ_FILE_TOOL, RUN_COMMAND_TOOL, SEARCH_TEXT_TOOL, SLEEP_TOOL,
    UPDATE_TODO_GRAPH_TOOL, WEB_FETCH_TOOL, WEB_SEARCH_TOOL, WRITE_FILE_TOOL,
};

pub(crate) fn tool_error_output(error: &ToolRuntimeError) -> Value {
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
        _ => json!({ "error": error.to_string() }),
    }
}

pub fn builtin_tool_timeout_ms(tool_name: &str, arguments: &Value) -> Result<u64, String> {
    let default_timeout_ms = match tool_name {
        READ_FILE_TOOL | FIND_FILES_TOOL => DEFAULT_FILE_TOOL_TIMEOUT_MS,
        GRAPH_FIND_SYMBOLS_TOOL
        | GRAPH_FIND_CALLERS_TOOL
        | GRAPH_FIND_CALLEES_TOOL
        | GRAPH_FIND_REFERENCES_TOOL
        | GRAPH_RELATED_FILES_TOOL
        | GRAPH_EXPLORE_TOOL => DEFAULT_GRAPH_TOOL_TIMEOUT_MS,
        SEARCH_TEXT_TOOL => DEFAULT_SEARCH_TEXT_TIMEOUT_MS,
        WEB_SEARCH_TOOL | WEB_FETCH_TOOL => DEFAULT_WEB_TOOL_TIMEOUT_MS,
        WRITE_FILE_TOOL | EDIT_FILE_TOOL => DEFAULT_WRITE_FILE_TIMEOUT_MS,
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

pub(crate) fn tool_timeout_ms(
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

#[derive(Debug)]
pub(crate) enum ToolRuntimeError {
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
            Self::Command { source, .. } => Some(source),
            Self::InvalidToolOutput { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::Cancelled
            | Self::CommandCancelled { .. }
            | Self::CommandTimedOut { .. }
            | Self::CommandFailed { .. }
            | Self::FileTooLarge { .. }
            | Self::InvalidArguments(_)
            | Self::InvalidPath(_)
            | Self::NotDirectory(_)
            | Self::NotFile(_)
            | Self::UnsupportedEncoding(_)
            | Self::UnknownTool(_) => None,
        }
    }
}
