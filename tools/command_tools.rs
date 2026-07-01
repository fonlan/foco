use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    COMMAND_WAIT_POLL_MS, DEFAULT_RUN_COMMAND_TIMEOUT_MS, DEFAULT_SLEEP_TIMEOUT_MS,
    ToolCancellationToken, ToolOutputSink,
    errors::{ToolRuntimeError, tool_timeout_ms},
    limited_output_text, parse_arguments, relative_workspace_path, resolve_workspace_path,
    run_command_with_timeout,
};

pub(crate) fn run_command(
    workspace_path: &Path,
    arguments: Value,
    cancellation_token: Option<&ToolCancellationToken>,
    output_sink: Option<&dyn ToolOutputSink>,
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

    reject_privacy_sensitive_recursive_scan(workspace_path, command, &args)?;

    let output = run_command_with_timeout(
        command,
        &args,
        &cwd,
        Duration::from_millis(timeout_ms),
        cancellation_token,
        output_sink,
        None,
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

pub(crate) fn sleep_tool(
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunCommandInput {
    pub(crate) command: String,
    pub(crate) args: Option<Vec<String>>,
    pub(crate) cwd: Option<String>,
    pub(crate) timeout_ms: Option<u64>,
}

fn reject_privacy_sensitive_recursive_scan(
    workspace_path: &Path,
    command: &str,
    args: &[String],
) -> Result<(), ToolRuntimeError> {
    let command_name = command_basename(command);
    let workspace = fs::canonicalize(workspace_path).map_err(|source| ToolRuntimeError::Io {
        path: workspace_path.to_path_buf(),
        source,
    })?;
    let home = home_dir();

    if is_recursive_scan_command(&command_name) {
        reject_recursive_scan_args(&workspace, home.as_deref(), &command_name, args)?;
    }

    if is_shell_command(&command_name) {
        for script in shell_command_scripts(args) {
            reject_recursive_scan_shell_script(&workspace, home.as_deref(), script)?;
        }
    }

    Ok(())
}

fn reject_recursive_scan_shell_script(
    workspace: &Path,
    home: Option<&Path>,
    script: &str,
) -> Result<(), ToolRuntimeError> {
    let words = shell_words(script);
    let mut index = 0;
    while index < words.len() {
        let word = words[index].as_str();
        if is_shell_separator(word) {
            index += 1;
            continue;
        }

        if is_recursive_scan_command(&command_basename(word)) {
            let command_name = command_basename(word);
            let start = index + 1;
            let end = words[start..]
                .iter()
                .position(|candidate| is_shell_separator(candidate))
                .map(|offset| start + offset)
                .unwrap_or(words.len());
            reject_recursive_scan_args(workspace, home, &command_name, &words[start..end])?;
            index = end;
        } else {
            index += 1;
        }
    }

    Ok(())
}

fn reject_recursive_scan_args(
    workspace: &Path,
    home: Option<&Path>,
    command: &str,
    args: &[String],
) -> Result<(), ToolRuntimeError> {
    for arg in args {
        if arg == "--" || arg.is_empty() {
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        let Some(reason) = recursive_scan_path_risk(workspace, home, command, arg) else {
            continue;
        };

        return Err(ToolRuntimeError::InvalidArguments(format!(
            "run_command refuses to run recursive scans outside the workspace ({reason}). Use workspace-relative paths or a narrower explicit path inside the workspace."
        )));
    }

    Ok(())
}

fn recursive_scan_path_risk(
    workspace: &Path,
    home: Option<&Path>,
    command: &str,
    value: &str,
) -> Option<String> {
    if matches!(value, "." | "./") {
        return None;
    }

    if value == "~"
        || value.starts_with("~/")
        || value == "$HOME"
        || value.starts_with("$HOME/")
        || value == "${HOME}"
        || value.starts_with("${HOME}/")
    {
        return Some("target references the user home directory".to_string());
    }

    let path = Path::new(value);
    if path.is_absolute() {
        if path_is_inside(path, workspace) {
            return None;
        }
        if command_uses_path_operands(command) {
            return Some(format!(
                "target is outside the workspace: {}",
                path.display()
            ));
        }
        if let Some(home) = home {
            if path == home {
                return Some(format!(
                    "target is the user home directory: {}",
                    path.display()
                ));
            }
            if path_is_inside(path, &home.join("Pictures")) {
                return Some(format!(
                    "target is inside the macOS Pictures folder: {}",
                    path.display()
                ));
            }
            if path_is_inside(
                path,
                &home.join("Library/Application Support/com.apple.TCC"),
            ) {
                return Some(format!(
                    "target is inside the macOS privacy database folder: {}",
                    path.display()
                ));
            }
        }

        return None;
    }

    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Some(format!("target escapes the workspace: {value}"));
    }

    None
}

fn shell_command_scripts(args: &[String]) -> impl Iterator<Item = &str> {
    args.iter().enumerate().filter_map(|(index, arg)| {
        if shell_arg_enables_command(arg) {
            args.get(index + 1).map(String::as_str)
        } else {
            None
        }
    })
}

fn shell_arg_enables_command(arg: &str) -> bool {
    arg == "-c" || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains('c'))
}

fn shell_words(script: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = script.chars().peekable();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if let Some(quote_char) = quote {
            if ch == quote_char {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }

        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            continue;
        }

        if ch.is_whitespace() {
            push_shell_word(&mut words, &mut current);
            continue;
        }

        if matches!(ch, ';' | '|') {
            push_shell_word(&mut words, &mut current);
            words.push(ch.to_string());
            continue;
        }

        if ch == '&' {
            push_shell_word(&mut words, &mut current);
            if chars.peek() == Some(&'&') {
                let _ = chars.next();
                words.push("&&".to_string());
            } else {
                words.push("&".to_string());
            }
            continue;
        }

        current.push(ch);
    }

    push_shell_word(&mut words, &mut current);
    words
}

fn push_shell_word(words: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        words.push(std::mem::take(current));
    }
}

fn is_recursive_scan_command(command: &str) -> bool {
    matches!(
        command,
        "find" | "fd" | "fdfind" | "rg" | "grep" | "egrep" | "fgrep" | "ag"
    )
}

fn command_uses_path_operands(command: &str) -> bool {
    matches!(command, "find" | "fd" | "fdfind")
}

fn is_shell_command(command: &str) -> bool {
    matches!(command, "bash" | "sh" | "zsh" | "dash" | "ksh")
}

fn is_shell_separator(word: &str) -> bool {
    matches!(word, ";" | "|" | "&&" | "&")
}

fn command_basename(command: &str) -> String {
    Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command)
        .to_string()
}

fn path_is_inside(path: &Path, parent: &Path) -> bool {
    path == parent || path.starts_with(parent)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SleepInput {
    duration_ms: u64,
    timeout_ms: Option<u64>,
}
