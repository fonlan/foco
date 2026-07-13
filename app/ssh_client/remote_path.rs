//! Remote path normalization for home-directory shorthand.

use foco_store::config::{is_remote_home_shorthand, needs_remote_home_expansion};

use super::error::{SshError, SshErrorKind};
use super::session::{SshCommandResult, SshSession};

/// Shell-quote a remote path for single-quoted POSIX use.
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Validate a candidate remote path before expansion or persistence.
///
/// Accepts absolute POSIX paths and `~` / `~/...`. Rejects empty, relative,
/// and `~other` homes.
pub fn validate_remote_path_input(path: &str) -> Result<(), SshError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(SshError::new(
            SshErrorKind::Config,
            "remote path must not be empty",
        ));
    }
    if path.starts_with('/') || is_remote_home_shorthand(path) {
        return Ok(());
    }
    Err(SshError::new(
        SshErrorKind::Config,
        format!("remote path must be absolute or a home shorthand (~ or ~/...): {path}"),
    ))
}

/// Build a remote shell command that prints a canonical absolute path for `input`.
pub fn expand_command(input: &str) -> Result<String, SshError> {
    let input = input.trim();
    validate_remote_path_input(input)?;

    if input == "~" {
        return Ok(
            "set -e; home=${HOME:-}; if [ -z \"$home\" ]; then home=$(getent passwd \"$(id -un)\" 2>/dev/null | cut -d: -f6 || true); fi; if [ -z \"$home\" ]; then printf '%s\\n' \"failed to resolve remote HOME\" >&2; exit 2; fi; case \"$home\" in /*) printf '%s\\n' \"$home\";; *) printf '%s\\n' \"HOME is not absolute\" >&2; exit 2;; esac"
                .to_string(),
        );
    }
    if let Some(rest) = input.strip_prefix("~/") {
        return Ok(format!(
            "set -e; home=${{HOME:-}}; if [ -z \"$home\" ]; then home=$(getent passwd \"$(id -un)\" 2>/dev/null | cut -d: -f6 || true); fi; if [ -z \"$home\" ]; then printf '%s\\n' \"failed to resolve remote HOME\" >&2; exit 2; fi; path=\"$home\"/{rest}; case \"$path\" in /*) printf '%s\\n' \"$path\";; *) printf '%s\\n' \"expanded path is not absolute\" >&2; exit 2;; esac",
            rest = shell_quote(rest)
        ));
    }
    // Absolute path: echo through after verifying leading `/`.
    Ok(format!(
        "set -e; path={}; case \"$path\" in /*) printf '%s\\n' \"$path\";; *) printf '%s\\n' \"path must be absolute\" >&2; exit 2;; esac",
        shell_quote(input)
    ))
}

/// Expand `~` / `~/...` via an authenticated session.
///
/// Absolute paths are returned as-is after validation (no remote round-trip).
pub async fn expand_remote_path(
    session: &SshSession,
    input: &str,
) -> Result<String, SshError> {
    let input = input.trim();
    validate_remote_path_input(input)?;

    if !needs_remote_home_expansion(input) {
        return Ok(input.to_string());
    }

    let command = expand_command(input)?;
    let result = session.exec(&command).await?;
    parse_expanded_path_result(&result)
}

fn parse_expanded_path_result(result: &SshCommandResult) -> Result<String, SshError> {
    if result.exit_status.unwrap_or(1) != 0 {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(SshError::new(
            SshErrorKind::RemoteCommandFailed,
            format!(
                "failed to expand remote home path: {}",
                stderr.trim().chars().take(200).collect::<String>()
            ),
        ));
    }
    let stdout = String::from_utf8_lossy(&result.stdout);
    let path = stdout.lines().next().unwrap_or("").trim();
    if path.is_empty() || !path.starts_with('/') {
        return Err(SshError::new(
            SshErrorKind::RemoteCommandFailed,
            format!("remote home expansion did not return an absolute path: {path}"),
        ));
    }
    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("/tmp/a'b"), "'/tmp/a'\\''b'");
    }

    #[test]
    fn validate_accepts_home_and_absolute() {
        validate_remote_path_input("~").expect("~");
        validate_remote_path_input("~/work").expect("~/work");
        validate_remote_path_input("/srv").expect("/srv");
        assert!(validate_remote_path_input("relative").is_err());
        assert!(validate_remote_path_input("~other/x").is_err());
        assert!(validate_remote_path_input("").is_err());
    }

    #[test]
    fn expand_command_quotes_rest() {
        let cmd = expand_command("~/proj ect").expect("cmd");
        assert!(cmd.contains("$home"));
        assert!(cmd.contains("'proj ect'"));
    }

    #[test]
    fn parse_requires_absolute() {
        let err = parse_expanded_path_result(&SshCommandResult {
            stdout: b"not-abs\n".to_vec(),
            stderr: Vec::new(),
            exit_status: Some(0),
        })
        .expect_err("relative");
        assert!(err.message().contains("absolute"));
    }
}
