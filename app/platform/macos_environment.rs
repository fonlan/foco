#[cfg(target_os = "macos")]
use std::{
    collections::HashSet,
    env,
    process::{Command, Stdio},
};

#[cfg(target_os = "macos")]
pub(crate) fn apply_macos_gui_environment() {
    let Some(path) = macos_gui_path() else {
        return;
    };

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
    let output = Command::new("/usr/libexec/path_helper")
        .arg("-s")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    parse_shell_path_assignment(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "macos")]
fn login_shell_path() -> Option<String> {
    let shell = env::var("SHELL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "/bin/zsh".to_string());
    let output = Command::new(shell)
        .args(["-lic", "printenv -0"])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    parse_printenv_path(&output.stdout)
}

#[cfg(target_os = "macos")]
fn append_path_entries(entries: &mut Vec<String>, path: Option<&str>) {
    if let Some(path) = path {
        entries.extend(
            env::split_paths(path)
                .filter(|entry| !entry.as_os_str().is_empty())
                .map(|entry| entry.to_string_lossy().to_string()),
        );
    }
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn parse_shell_path_assignment(output: &str) -> Option<String> {
    output
        .split(';')
        .map(str::trim)
        .find_map(|statement| statement.strip_prefix("PATH="))
        .map(|value| value.trim_matches('"').replace("\\\"", "\""))
        .filter(|value| !value.trim().is_empty())
}

#[cfg(target_os = "macos")]
fn parse_printenv_path(output: &[u8]) -> Option<String> {
    output
        .split(|byte| *byte == 0)
        .find_map(|entry| entry.strip_prefix(b"PATH="))
        .and_then(|value| String::from_utf8(value.to_vec()).ok())
        .filter(|value| !value.trim().is_empty())
}
