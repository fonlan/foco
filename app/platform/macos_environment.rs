#![cfg_attr(test, allow(dead_code))]

use std::{collections::HashSet, env};

#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};

const LOGIN_SHELL_PATH_START: &str = "__FOCO_PATH_START__";
const LOGIN_SHELL_PATH_END: &str = "__FOCO_PATH_END__";
const LOGIN_SHELL_PATH_COMMAND: &str =
    "printf '%s\n%s\n%s\n' __FOCO_PATH_START__ \"$PATH\" __FOCO_PATH_END__";

#[cfg(target_os = "macos")]
pub(crate) fn apply_macos_gui_environment() {
    let Some(path) = macos_gui_path() else {
        tracing::warn!("macOS GUI PATH compensation skipped; no PATH entries were discovered");
        return;
    };

    let entry_count = env::split_paths(&path).count();
    tracing::info!(
        path = %path,
        entry_count,
        has_homebrew_path = path_contains_any(&path, &["/opt/homebrew", "/usr/local"]),
        has_nvm_path = path_contains_any(&path, &["/.nvm/"]),
        has_fnm_path = path_contains_any(&path, &["/.fnm/"]),
        has_asdf_path = path_contains_any(&path, &["/.asdf/"]),
        "applied macOS GUI PATH compensation"
    );

    // SAFETY: This runs during startup before Foco spawns worker threads or child
    // processes, so no concurrent environment reads/writes are in flight.
    unsafe {
        env::set_var("PATH", path);
    }
}

#[cfg(target_os = "macos")]
fn macos_gui_path() -> Option<String> {
    let mut entries = Vec::new();
    append_path_entries(&mut entries, login_shell_path().as_deref());
    append_path_entries(&mut entries, path_helper_path().as_deref());
    append_path_entries(&mut entries, env::var("PATH").ok().as_deref());

    dedupe_path_entries(entries)
}

#[cfg(target_os = "macos")]
fn path_helper_path() -> Option<String> {
    let output = match Command::new("/usr/libexec/path_helper")
        .arg("-s")
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            tracing::warn!(%error, "failed to run macOS path_helper for PATH compensation");
            return None;
        }
    };
    if !output.status.success() {
        tracing::warn!(status = %output.status, "macOS path_helper failed during PATH compensation");
        return None;
    }

    parse_shell_path_assignment(&String::from_utf8_lossy(&output.stdout)).or_else(|| {
        tracing::warn!("macOS path_helper output did not contain a PATH assignment");
        None
    })
}

#[cfg(target_os = "macos")]
fn login_shell_path() -> Option<String> {
    let shell = env::var("SHELL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "/bin/zsh".to_string());
    let output = match Command::new(&shell)
        .args(["-lic", LOGIN_SHELL_PATH_COMMAND])
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            tracing::warn!(shell = %shell, %error, "failed to run login shell for PATH compensation");
            return None;
        }
    };
    if !output.status.success() {
        tracing::warn!(
            shell = %shell,
            status = %output.status,
            stderr = %String::from_utf8_lossy(&output.stderr),
            "login shell failed during PATH compensation"
        );
        return None;
    }

    parse_login_shell_path_output(&String::from_utf8_lossy(&output.stdout)).or_else(|| {
        tracing::warn!(shell = %shell, "login shell output did not contain PATH sentinels");
        None
    })
}

fn append_path_entries(entries: &mut Vec<String>, path: Option<&str>) {
    if let Some(path) = path {
        entries.extend(
            env::split_paths(path)
                .filter(|entry| !entry.as_os_str().is_empty())
                .map(|entry| entry.to_string_lossy().to_string()),
        );
    }
}

fn dedupe_path_entries(entries: Vec<String>) -> Option<String> {
    let mut seen = HashSet::new();
    let entries = entries
        .into_iter()
        .filter(|entry| seen.insert(entry.clone()))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        None
    } else {
        Some(entries.join(":"))
    }
}

fn parse_shell_path_assignment(output: &str) -> Option<String> {
    output
        .split(';')
        .map(str::trim)
        .find_map(|statement| statement.strip_prefix("PATH="))
        .map(|value| value.trim_matches('"').replace("\\\"", "\""))
        .filter(|value| !value.trim().is_empty())
}

fn parse_login_shell_path_output(output: &str) -> Option<String> {
    let start = output.find(LOGIN_SHELL_PATH_START)? + LOGIN_SHELL_PATH_START.len();
    let remaining = &output[start..];
    let end = remaining.find(LOGIN_SHELL_PATH_END)?;
    let path = remaining[..end].trim_matches(|ch| ch == '\r' || ch == '\n');
    (!path.trim().is_empty()).then(|| path.to_string())
}

#[cfg(target_os = "macos")]
fn path_contains_any(path: &str, needles: &[&str]) -> bool {
    env::split_paths(path).any(|entry| {
        let entry = entry.to_string_lossy();
        needles.iter().any(|needle| entry.contains(needle))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_login_shell_path_output_ignores_shell_noise() {
        let output = concat!(
            "shell banner\n",
            "__FOCO_PATH_START__\n",
            "/opt/homebrew/bin:/Users/example/.nvm/versions/node/v22/bin:/usr/bin\n",
            "__FOCO_PATH_END__\n",
            "startup footer\n",
        );

        assert_eq!(
            parse_login_shell_path_output(output).as_deref(),
            Some("/opt/homebrew/bin:/Users/example/.nvm/versions/node/v22/bin:/usr/bin")
        );
    }

    #[test]
    fn dedupe_path_entries_keeps_first_occurrence() {
        assert_eq!(
            dedupe_path_entries(vec![
                "/opt/homebrew/bin".to_string(),
                "/usr/bin".to_string(),
                "/opt/homebrew/bin".to_string(),
            ])
            .as_deref(),
            Some("/opt/homebrew/bin:/usr/bin")
        );
    }
}
