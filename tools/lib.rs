use std::{
    fmt, fs, io,
    path::{Component, Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const READ_FILE_TOOL: &str = "read_file";
pub const LIST_FILES_TOOL: &str = "list_files";
pub const SEARCH_TEXT_TOOL: &str = "search_text";
pub const WRITE_FILE_TOOL: &str = "write_file";
pub const RUN_COMMAND_TOOL: &str = "run_command";
pub const GIT_DIFF_TOOL: &str = "git_diff";

const MAX_READ_BYTES: u64 = 512 * 1024;
const MAX_LIST_ENTRIES: usize = 200;
const MAX_SEARCH_MATCHES: usize = 200;
const MAX_COMMAND_OUTPUT_BYTES: usize = 64 * 1024;

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
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Command arguments."
                },
                "cwd": {
                    "type": "string",
                    "description": "Optional workspace-relative working directory. Defaults to the workspace root."
                }
            },
            "required": ["command"]
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
                    "type": "string",
                    "description": "Optional workspace-relative path to diff."
                }
            },
            "required": []
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
        }
    }
}

impl std::error::Error for ToolRuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;

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
}
