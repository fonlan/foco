use std::{
    fs,
    path::Path,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SleepInput {
    duration_ms: u64,
    timeout_ms: Option<u64>,
}
