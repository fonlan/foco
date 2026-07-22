use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    BackgroundCommandOutput, BackgroundCommandOutputStream, BackgroundCommandRegistry,
    BackgroundCommandRequest, BackgroundCommandSnapshot, BackgroundCommandStatus,
    BackgroundCommandTermination, COMMAND_WAIT_POLL_MS, CommandOutputLimits,
    DEFAULT_GET_COMMAND_OUTPUT_TIMEOUT_MS, DEFAULT_RUN_COMMAND_TIMEOUT_MS,
    DEFAULT_SLEEP_TIMEOUT_MS, MAX_COMMAND_CAPTURE_BYTES_PER_STREAM, ToolCancellationToken,
    ToolOutputSink,
    errors::{ToolRuntimeError, tool_timeout_ms},
    limited_output_text, parse_arguments, relative_workspace_path, resolve_workspace_path,
    run_command_with_timeout,
};

pub(crate) fn run_command(
    workspace_path: &Path,
    arguments: Value,
    cancellation_token: Option<&ToolCancellationToken>,
    output_sink: Option<&dyn ToolOutputSink>,
    background_commands: &BackgroundCommandRegistry,
    owner_chat_id: Option<&str>,
    owner_run_id: Option<&str>,
) -> Result<Value, ToolRuntimeError> {
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

    reject_privacy_sensitive_recursive_scan(workspace_path, command, &args)?;

    if request.background.unwrap_or(false) {
        let timeout = background_timeout(request.background_timeout_ms)?;
        let snapshot = background_commands
            .start(BackgroundCommandRequest {
                workspace_path: workspace_path.to_path_buf(),
                cwd,
                command: command.to_string(),
                args,
                owner_chat_id: owner_chat_id.map(str::to_string),
                owner_run_id: owner_run_id.map(str::to_string),
                timeout,
            })
            .map_err(background_start_error)?;

        // Give immediate spawn failures a tiny chance to settle without turning long commands
        // back into synchronous tool calls.
        thread::sleep(Duration::from_millis(COMMAND_WAIT_POLL_MS));
        let snapshot = background_commands
            .command(&snapshot.command_id)
            .map_err(background_command_error)?;
        return background_command_response(background_commands, snapshot, None, false);
    }

    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_RUN_COMMAND_TIMEOUT_MS)?;
    let output = run_command_with_timeout(
        command,
        &args,
        &cwd,
        Duration::from_millis(timeout_ms),
        cancellation_token,
        output_sink,
        Some(CommandOutputLimits {
            stdout_bytes: Some(MAX_COMMAND_CAPTURE_BYTES_PER_STREAM),
            stderr_bytes: Some(MAX_COMMAND_CAPTURE_BYTES_PER_STREAM),
            output_delta_bytes: Some(crate::output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT),
            truncate: true,
        }),
    )?;
    let (stdout, stdout_limited) = limited_output_text(&output.stdout);
    let (stderr, stderr_limited) = limited_output_text(&output.stderr);
    let stdout_truncated = output.stdout_truncated || stdout_limited;
    let stderr_truncated = output.stderr_truncated || stderr_limited;
    let output_omitted = stdout_truncated || stderr_truncated || output.output_delta_truncated;

    let mut result = json!({
        "command": command,
        "args": args,
        "cwd": relative_workspace_path(workspace_path, &cwd)?,
        "pid": output.pid,
        "status": output.status.code(),
        "success": output.status.success(),
        "stdout": stdout,
        "stderr": stderr,
        "stdoutBytes": output.stdout_bytes,
        "stderrBytes": output.stderr_bytes,
        "stdoutTruncated": stdout_truncated,
        "stderrTruncated": stderr_truncated,
        "outputDeltaTruncated": output.output_delta_truncated,
        "timeoutMs": timeout_ms
    });
    if output_omitted {
        result["outputOmitted"] = Value::Bool(true);
        result["retryUnsafe"] = Value::Bool(true);
    }
    Ok(result)
}

pub(crate) fn get_command_output(
    workspace_path: &Path,
    arguments: Value,
    background_commands: &BackgroundCommandRegistry,
    owner_chat_id: Option<&str>,
) -> Result<Value, ToolRuntimeError> {
    let request: GetCommandOutputInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GET_COMMAND_OUTPUT_TIMEOUT_MS)?;
    let started = Instant::now();
    let wait_ms = request
        .wait_ms
        .unwrap_or(0)
        .min(timeout_ms.saturating_sub(1));

    loop {
        let snapshot = owned_command_snapshot(
            background_commands,
            workspace_path,
            owner_chat_id,
            &request.process_id,
        )?;
        let output = background_commands
            .output_after(&request.process_id, request.cursor)
            .map_err(background_command_error)?;
        if !output.chunks.is_empty()
            || snapshot.status.is_terminal()
            || started.elapsed() >= Duration::from_millis(wait_ms)
        {
            return command_output_response(snapshot, output, request.cursor);
        }
        thread::sleep(
            Duration::from_millis(COMMAND_WAIT_POLL_MS)
                .min(Duration::from_millis(wait_ms).saturating_sub(started.elapsed())),
        );
    }
}

pub(crate) fn stop_command(
    workspace_path: &Path,
    arguments: Value,
    background_commands: &BackgroundCommandRegistry,
    owner_chat_id: Option<&str>,
) -> Result<Value, ToolRuntimeError> {
    let request: StopCommandInput = parse_arguments(arguments)?;
    let _timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GET_COMMAND_OUTPUT_TIMEOUT_MS)?;
    owned_command_snapshot(
        background_commands,
        workspace_path,
        owner_chat_id,
        &request.process_id,
    )?;
    let snapshot = background_commands
        .stop(&request.process_id)
        .map_err(background_command_error)?;
    background_command_response(background_commands, snapshot, None, true)
}

fn background_command_response(
    registry: &BackgroundCommandRegistry,
    snapshot: BackgroundCommandSnapshot,
    cursor: Option<u64>,
    include_cursor_range: bool,
) -> Result<Value, ToolRuntimeError> {
    let output = registry
        .output_after(&snapshot.command_id, cursor)
        .map_err(background_command_error)?;
    let mut result = command_output_response(snapshot, output, cursor)?;
    if !include_cursor_range {
        let object = result.as_object_mut().ok_or_else(|| {
            ToolRuntimeError::InvalidArguments("invalid managed command response".to_string())
        })?;
        object.remove("fromCursor");
        object.remove("availableFromCursor");
        object.remove("cursorExpired");
        object.remove("retainedOutputBytes");
    }
    Ok(result)
}

fn command_output_response(
    snapshot: BackgroundCommandSnapshot,
    output: BackgroundCommandOutput,
    requested_cursor: Option<u64>,
) -> Result<Value, ToolRuntimeError> {
    let available_from_cursor = output.earliest_cursor.unwrap_or(output.next_cursor);
    let from_cursor = requested_cursor.unwrap_or(available_from_cursor);
    let cursor_before_output = requested_cursor.unwrap_or(available_from_cursor.saturating_sub(1));
    let response_base = json!({
        "processId": snapshot.command_id,
        "pid": snapshot.pid,
        "status": background_status_name(snapshot.status),
        "startedAt": unix_millis(snapshot.started_at),
        "endedAt": snapshot.ended_at.map(unix_millis),
        "exitCode": snapshot.exit_code,
        "success": background_success(&snapshot),
        "terminationReason": snapshot.termination.map(background_termination_name),
        "fromCursor": from_cursor,
        "availableFromCursor": available_from_cursor,
        "cursorExpired": output.cursor_expired,
        "retainedOutputBytes": snapshot.retained_output_bytes,
        "outputTruncated": output.output_truncated,
    });
    let (chunks, next_cursor, has_more) =
        limited_chunks(&response_base, &output, cursor_before_output)?;
    command_output_page(response_base, chunks, next_cursor, has_more)
}

fn command_output_page(
    mut response: Value,
    chunks: Vec<Value>,
    next_cursor: u64,
    has_more: bool,
) -> Result<Value, ToolRuntimeError> {
    let process_id = response
        .get("processId")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ToolRuntimeError::InvalidArguments("invalid managed command response".to_string())
        })?;
    let truncated = has_more;
    let note = truncated.then(|| {
        format!(
            "This response was explicitly paginated under the shared output budget; no retained output was silently discarded. Continue with get_command_output using the same processId={process_id} and cursor={next_cursor} (the returned nextCursor) to receive the next complete chunk without repeats. This response-level truncated flag is separate from outputTruncated and cursorExpired, which only describe the retained process buffer."
        )
    });
    let object = response.as_object_mut().ok_or_else(|| {
        ToolRuntimeError::InvalidArguments("invalid managed command response".to_string())
    })?;
    object.insert("chunks".to_string(), Value::Array(chunks));
    object.insert("nextCursor".to_string(), json!(next_cursor));
    object.insert("hasMore".to_string(), json!(has_more));
    object.insert("truncated".to_string(), Value::Bool(truncated));
    if let Some(note) = note {
        object.insert("note".to_string(), Value::String(note));
    }
    Ok(response)
}

fn limited_chunks(
    response_base: &Value,
    output: &BackgroundCommandOutput,
    cursor_before_output: u64,
) -> Result<(Vec<Value>, u64, bool), ToolRuntimeError> {
    let mut chunks = Vec::new();
    let mut next_cursor = cursor_before_output;

    for (index, chunk) in output.chunks.iter().enumerate() {
        let candidate = json!({
            "cursor": chunk.cursor,
            "stream": match chunk.stream {
                BackgroundCommandOutputStream::Stdout => "stdout",
                BackgroundCommandOutputStream::Stderr => "stderr",
            },
            "text": String::from_utf8_lossy(&chunk.bytes),
        });
        let candidate_next_cursor = chunk.cursor;
        let candidate_has_more = index + 1 < output.chunks.len();
        chunks.push(candidate);
        let candidate_response = command_output_page(
            response_base.clone(),
            chunks.clone(),
            candidate_next_cursor,
            candidate_has_more,
        )?;
        let measurement = crate::output_budget::measure_tool_execution(&crate::ToolExecution {
            output: candidate_response,
            is_error: false,
        })
        .map_err(|source| {
            ToolRuntimeError::InvalidArguments(format!(
                "failed to measure managed command output: {source}"
            ))
        })?;
        if measurement.serialized_bytes > crate::output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT
            || measurement.text_lines > crate::output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT
        {
            let _ = chunks.pop();
            break;
        }

        next_cursor = candidate_next_cursor;
    }

    let has_more = chunks.len() < output.chunks.len();
    Ok((chunks, next_cursor, has_more))
}

fn owned_command_snapshot(
    registry: &BackgroundCommandRegistry,
    workspace_path: &Path,
    owner_chat_id: Option<&str>,
    process_id: &str,
) -> Result<BackgroundCommandSnapshot, ToolRuntimeError> {
    let snapshot = registry
        .command(process_id)
        .map_err(background_command_error)?;
    let workspace = fs::canonicalize(workspace_path).map_err(|source| ToolRuntimeError::Io {
        path: workspace_path.to_path_buf(),
        source,
    })?;
    if snapshot.workspace_path != workspace || snapshot.owner_chat_id.as_deref() != owner_chat_id {
        return Err(managed_command_not_found_error());
    }
    Ok(snapshot)
}

fn background_timeout(timeout_ms: Option<u64>) -> Result<Option<Duration>, ToolRuntimeError> {
    match timeout_ms {
        None => Ok(None),
        Some(0) => Err(ToolRuntimeError::InvalidArguments(
            "backgroundTimeoutMs must be greater than zero when provided".to_string(),
        )),
        Some(timeout_ms) => Ok(Some(Duration::from_millis(timeout_ms))),
    }
}

fn background_start_error(error: crate::BackgroundCommandError) -> ToolRuntimeError {
    ToolRuntimeError::InvalidArguments(format!("failed to start managed command: {error}"))
}

fn background_command_error(_error: crate::BackgroundCommandError) -> ToolRuntimeError {
    managed_command_not_found_error()
}

fn managed_command_not_found_error() -> ToolRuntimeError {
    ToolRuntimeError::InvalidArguments("managed command was not found".to_string())
}

fn background_status_name(status: BackgroundCommandStatus) -> &'static str {
    match status {
        BackgroundCommandStatus::Running => "running",
        BackgroundCommandStatus::Exited => "exited",
        BackgroundCommandStatus::Stopped => "stopped",
        BackgroundCommandStatus::TimedOut => "timed_out",
        BackgroundCommandStatus::Failed => "failed",
    }
}

fn background_termination_name(termination: BackgroundCommandTermination) -> &'static str {
    match termination {
        BackgroundCommandTermination::ExplicitStop => "explicit_stop",
        BackgroundCommandTermination::Timeout => "timeout",
        BackgroundCommandTermination::HostShutdown => "host_shutdown",
    }
}

fn background_success(snapshot: &BackgroundCommandSnapshot) -> Option<bool> {
    match snapshot.status {
        BackgroundCommandStatus::Running => None,
        BackgroundCommandStatus::Exited => Some(snapshot.exit_code == Some(0)),
        BackgroundCommandStatus::Stopped
        | BackgroundCommandStatus::TimedOut
        | BackgroundCommandStatus::Failed => Some(false),
    }
}

fn unix_millis(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
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
    pub(crate) background: Option<bool>,
    pub(crate) background_timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetCommandOutputInput {
    process_id: String,
    cursor: Option<u64>,
    wait_ms: Option<u64>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StopCommandInput {
    process_id: String,
    timeout_ms: Option<u64>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paginated_command_output_keeps_the_first_unreturned_chunk_reachable() {
        let output = test_output(vec![
            test_chunk(
                1,
                BackgroundCommandOutputStream::Stdout,
                vec![b'a'; 30 * 1024],
            ),
            test_chunk(
                2,
                BackgroundCommandOutputStream::Stderr,
                vec![b'b'; 30 * 1024],
            ),
            test_chunk(
                3,
                BackgroundCommandOutputStream::Stdout,
                vec![b'c'; 30 * 1024],
            ),
        ]);

        let first = command_output_response(test_snapshot(), output.clone(), None)
            .expect("first command output response");
        assert_eq!(first["chunks"].as_array().expect("chunks").len(), 1);
        assert_eq!(first["chunks"][0]["cursor"], 1);
        assert_eq!(first["chunks"][0]["stream"], "stdout");
        assert_eq!(first["nextCursor"], 1);
        assert_eq!(first["hasMore"], true);
        assert_eq!(first["truncated"], true);
        assert!(
            first["note"]
                .as_str()
                .expect("pagination note")
                .contains("same processId=command-test and cursor=1")
        );
        assert!(
            crate::output_budget::measure_tool_execution(&crate::ToolExecution {
                output: first.clone(),
                is_error: false,
            })
            .expect("measure first response")
            .serialized_bytes
                <= crate::output_budget::TOOL_OUTPUT_SOFT_BYTE_LIMIT
        );

        let after_cursor = first["nextCursor"].as_u64().expect("next cursor");
        let remaining = BackgroundCommandOutput {
            chunks: output
                .chunks
                .into_iter()
                .filter(|chunk| chunk.cursor > after_cursor)
                .collect(),
            ..output
        };
        let second = command_output_response(test_snapshot(), remaining, Some(after_cursor))
            .expect("second command output response");
        assert_eq!(second["chunks"][0]["cursor"], 2);
        assert_eq!(second["chunks"][0]["stream"], "stderr");
    }

    #[test]
    fn command_output_paginates_when_text_lines_exceed_the_shared_limit() {
        let chunk = b"line\n".repeat(1_100);
        let output = test_output(vec![
            test_chunk(1, BackgroundCommandOutputStream::Stdout, chunk.clone()),
            test_chunk(2, BackgroundCommandOutputStream::Stderr, chunk),
        ]);

        let response = command_output_response(test_snapshot(), output, None)
            .expect("line-bounded command output response");

        assert_eq!(response["chunks"].as_array().expect("chunks").len(), 1);
        assert_eq!(response["nextCursor"], 1);
        assert_eq!(response["hasMore"], true);
        assert_eq!(response["truncated"], true);
        assert!(
            crate::output_budget::measure_tool_execution(&crate::ToolExecution {
                output: response,
                is_error: false,
            })
            .expect("measure line-bounded response")
            .text_lines
                <= crate::output_budget::TOOL_OUTPUT_SOFT_LINE_LIMIT
        );
    }

    #[test]
    fn complete_command_output_does_not_claim_response_pagination() {
        let response = command_output_response(
            test_snapshot(),
            test_output(vec![test_chunk(
                1,
                BackgroundCommandOutputStream::Stdout,
                b"ready\n".to_vec(),
            )]),
            None,
        )
        .expect("complete command output response");

        assert_eq!(response["hasMore"], false);
        assert_eq!(response["truncated"], false);
        assert!(response.get("note").is_none());
    }

    #[test]
    fn response_pagination_does_not_change_process_buffer_truncation_metadata() {
        let mut output = test_output(vec![
            test_chunk(
                1,
                BackgroundCommandOutputStream::Stdout,
                vec![b'a'; 30 * 1024],
            ),
            test_chunk(
                2,
                BackgroundCommandOutputStream::Stderr,
                vec![b'b'; 30 * 1024],
            ),
        ]);
        output.cursor_expired = true;
        output.output_truncated = true;

        let response = command_output_response(test_snapshot(), output, Some(0))
            .expect("paginated command output response");

        assert_eq!(response["cursorExpired"], true);
        assert_eq!(response["outputTruncated"], true);
        assert_eq!(response["truncated"], true);
        assert_eq!(response["hasMore"], true);
    }

    fn test_output(chunks: Vec<crate::BackgroundCommandOutputChunk>) -> BackgroundCommandOutput {
        let next_cursor = chunks.last().map_or(0, |chunk| chunk.cursor);
        BackgroundCommandOutput {
            command_id: "command-test".to_string(),
            chunks,
            next_cursor,
            earliest_cursor: Some(1),
            cursor_expired: false,
            output_truncated: false,
        }
    }

    fn test_chunk(
        cursor: u64,
        stream: BackgroundCommandOutputStream,
        bytes: Vec<u8>,
    ) -> crate::BackgroundCommandOutputChunk {
        crate::BackgroundCommandOutputChunk {
            cursor,
            stream,
            bytes,
        }
    }

    fn test_snapshot() -> BackgroundCommandSnapshot {
        BackgroundCommandSnapshot {
            command_id: "command-test".to_string(),
            pid: 1,
            workspace_path: PathBuf::from("/workspace"),
            cwd: PathBuf::from("/workspace"),
            command: "test".to_string(),
            args: Vec::new(),
            owner_chat_id: None,
            owner_run_id: None,
            started_at: SystemTime::UNIX_EPOCH,
            ended_at: None,
            status: BackgroundCommandStatus::Running,
            exit_code: None,
            termination: None,
            error: None,
            retained_output_bytes: 8 * 1024,
            dropped_output_bytes: 0,
        }
    }
}
