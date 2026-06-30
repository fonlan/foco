#[cfg(any(windows, target_os = "macos"))]
use std::env;
#[cfg(any(windows, target_os = "macos", test))]
use std::path::Path;

use crate::ApiError;

#[cfg(any(windows, test))]
use crate::AUTO_START_COMMAND;
#[cfg(any(target_os = "macos", test))]
use foco_store::config::FOCO_CONFIG_DIR_ENV;

#[cfg(windows)]
use crate::normalize_windows_verbatim_path;
#[cfg(target_os = "macos")]
use std::{fs, path::PathBuf, process::Command};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{ERROR_SUCCESS, WIN32_ERROR},
    System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
        RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
    },
};

#[cfg(windows)]
const AUTO_START_REGISTRY_VALUE_NAME: &str = "Foco";
#[cfg(windows)]
const AUTO_START_REGISTRY_RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(any(target_os = "macos", test))]
const MACOS_LAUNCH_AGENT_LABEL: &str = "com.foco.app";
#[cfg(target_os = "macos")]
const MACOS_LAUNCH_AGENT_FILE_NAME: &str = "com.foco.app.plist";

#[cfg(windows)]
pub(crate) fn apply_auto_start_setting(enabled: bool) -> Result<(), ApiError> {
    if enabled {
        let exe_path = env::current_exe().map_err(|source| {
            ApiError::internal(format!(
                "failed to resolve current executable path for auto start: {source}"
            ))
        })?;
        let exe_path = normalize_windows_verbatim_path(exe_path);
        set_auto_start_registry_value(&windows_auto_start_command(&exe_path))
    } else {
        remove_auto_start_registry_value()
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn apply_auto_start_setting(enabled: bool) -> Result<(), ApiError> {
    if enabled {
        enable_macos_auto_start()
    } else {
        disable_macos_auto_start()
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn auto_start_enabled_for_response(configured: bool) -> bool {
    match macos_auto_start_status() {
        Ok(status) => status.is_enabled(),
        Err(error) => {
            tracing::warn!(?error, "failed to inspect macOS auto start status");
            configured
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn auto_start_enabled_for_response(configured: bool) -> bool {
    configured
}

#[cfg(all(not(windows), not(target_os = "macos")))]
pub(crate) fn apply_auto_start_setting(enabled: bool) -> Result<(), ApiError> {
    if enabled {
        return Err(ApiError::bad_request(
            "auto start is only supported on Windows and macOS",
        ));
    }

    Ok(())
}

#[cfg(any(windows, test))]
fn windows_auto_start_command(exe_path: &Path) -> String {
    format!("\"{}\" {AUTO_START_COMMAND}", exe_path.display())
}

#[cfg(target_os = "macos")]
fn enable_macos_auto_start() -> Result<(), ApiError> {
    let exe_path = macos_auto_start_executable_path()?;
    let exe_path = exe_path.to_string_lossy();
    let plist_path = macos_launch_agent_path()?;
    let environment = macos_launch_agent_environment();
    let plist = macos_launch_agent_plist(&exe_path, &environment);

    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            ApiError::internal(format!(
                "failed to create macOS LaunchAgents directory {}: {source}",
                parent.display()
            ))
        })?;
    }
    fs::write(&plist_path, plist).map_err(|source| {
        ApiError::internal(format!(
            "failed to write macOS LaunchAgent plist {}: {source}",
            plist_path.display()
        ))
    })?;

    let uid = macos_user_id()?;
    let domain = format!("gui/{uid}");
    let plist_arg = plist_path.to_string_lossy().into_owned();

    if let Err(error) = run_launchctl(&["bootout", &format!("{domain}/{MACOS_LAUNCH_AGENT_LABEL}")])
    {
        tracing::warn!(%error, "failed to unload existing macOS LaunchAgent before enabling auto start");
    }

    let bootstrap_result = run_launchctl(&["bootstrap", &domain, &plist_arg]);
    if bootstrap_result.is_ok() {
        return Ok(());
    }

    let bootstrap_error = bootstrap_result
        .err()
        .unwrap_or_else(|| "unknown error".to_string());
    let load_result = run_launchctl(&["load", &plist_arg]);
    if load_result.is_ok() {
        return Ok(());
    }

    let load_error = load_result
        .err()
        .unwrap_or_else(|| "unknown error".to_string());
    if let Err(error) = fs::remove_file(&plist_path) {
        tracing::warn!(path = %plist_path.display(), %error, "failed to roll back macOS LaunchAgent plist after launchctl failure");
    }

    Err(ApiError::internal(format!(
        "failed to enable macOS auto start with launchctl bootstrap or load: bootstrap: {bootstrap_error}; load: {load_error}"
    )))
}

#[cfg(target_os = "macos")]
fn macos_auto_start_status() -> Result<MacosAutoStartStatus, ApiError> {
    let plist_path = macos_launch_agent_path()?;
    let plist_exists = plist_path.exists();
    let expected_executable_path = macos_auto_start_executable_path()?;
    let plist_executable_path = if plist_exists {
        let plist = fs::read_to_string(&plist_path).map_err(|source| {
            ApiError::internal(format!(
                "failed to read macOS LaunchAgent plist {}: {source}",
                plist_path.display()
            ))
        })?;
        macos_launch_agent_program_argument(&plist).map(PathBuf::from)
    } else {
        None
    };
    let executable_path_matches_current_app = plist_executable_path
        .as_deref()
        .is_some_and(|path| paths_match(path, &expected_executable_path));
    let service_loaded = macos_launch_agent_is_loaded()?;

    Ok(MacosAutoStartStatus {
        plist_exists,
        executable_path_matches_current_app,
        service_loaded,
    })
}

#[cfg(target_os = "macos")]
struct MacosAutoStartStatus {
    plist_exists: bool,
    executable_path_matches_current_app: bool,
    service_loaded: bool,
}

#[cfg(target_os = "macos")]
impl MacosAutoStartStatus {
    fn is_enabled(&self) -> bool {
        self.plist_exists && self.executable_path_matches_current_app && self.service_loaded
    }
}

#[cfg(target_os = "macos")]
fn macos_auto_start_executable_path() -> Result<PathBuf, ApiError> {
    let exe_path = env::current_exe().map_err(|source| {
        ApiError::internal(format!(
            "failed to resolve current executable path for macOS auto start: {source}"
        ))
    })?;

    Ok(macos_auto_start_executable_path_from_current_exe(&exe_path))
}

#[cfg(any(target_os = "macos", test))]
fn macos_auto_start_executable_path_from_current_exe(current_exe: &Path) -> PathBuf {
    current_exe.to_path_buf()
}

#[cfg(target_os = "macos")]
fn macos_launch_agent_is_loaded() -> Result<bool, ApiError> {
    let uid = macos_user_id()?;
    let service = format!("gui/{uid}/{MACOS_LAUNCH_AGENT_LABEL}");
    match run_launchctl(&["print", &service]) {
        Ok(()) => Ok(true),
        Err(error) if launchctl_missing_service_error(&error) => Ok(false),
        Err(error) => {
            tracing::warn!(%error, "failed to inspect macOS LaunchAgent service");
            Ok(false)
        }
    }
}

#[cfg(target_os = "macos")]
fn paths_match(left: &Path, right: &Path) -> bool {
    let left_canonical = fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right_canonical = fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
    left_canonical == right_canonical
}

#[cfg(target_os = "macos")]
fn disable_macos_auto_start() -> Result<(), ApiError> {
    let plist_path = macos_launch_agent_path()?;
    let uid = macos_user_id()?;
    let service = format!("gui/{uid}/{MACOS_LAUNCH_AGENT_LABEL}");
    let plist_arg = plist_path.to_string_lossy().into_owned();

    let bootout_result = run_launchctl(&["bootout", &service]);
    if let Err(bootout_error) = bootout_result {
        if launchctl_missing_service_error(&bootout_error) && !plist_path.exists() {
            tracing::warn!(%bootout_error, "macOS LaunchAgent was already unloaded while disabling auto start");
        } else {
            let unload_result = if plist_path.exists() {
                run_launchctl(&["unload", &plist_arg])
            } else {
                Err(bootout_error.clone())
            };

            match unload_result {
                Ok(()) => {}
                Err(unload_error)
                    if launchctl_missing_service_error(&bootout_error)
                        && launchctl_missing_service_error(&unload_error) => {}
                Err(unload_error) => {
                    return Err(ApiError::internal(format!(
                        "failed to disable macOS auto start with launchctl bootout or unload: bootout: {bootout_error}; unload: {unload_error}"
                    )));
                }
            }
        }
    }

    match fs::remove_file(&plist_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ApiError::internal(format!(
            "failed to remove macOS LaunchAgent plist {}: {error}",
            plist_path.display()
        ))),
    }
}

#[cfg(target_os = "macos")]
fn macos_launch_agent_path() -> Result<PathBuf, ApiError> {
    let home = env::var_os("HOME").ok_or_else(|| {
        ApiError::internal("failed to resolve HOME for macOS LaunchAgent auto start")
    })?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents")
        .join(MACOS_LAUNCH_AGENT_FILE_NAME))
}

#[cfg(target_os = "macos")]
fn macos_user_id() -> Result<String, ApiError> {
    let output = Command::new("id").arg("-u").output().map_err(|source| {
        ApiError::internal(format!(
            "failed to run id -u for macOS LaunchAgent domain: {source}"
        ))
    })?;
    if !output.status.success() {
        return Err(ApiError::internal(format!(
            "failed to resolve macOS user id with id -u: {}",
            command_output_summary(output.status, &output.stdout, &output.stderr)
        )));
    }

    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uid.is_empty() {
        return Err(ApiError::internal(
            "failed to resolve macOS user id with id -u: empty output",
        ));
    }
    Ok(uid)
}

#[cfg(target_os = "macos")]
fn run_launchctl(args: &[&str]) -> Result<(), String> {
    let output = Command::new("launchctl")
        .args(args)
        .output()
        .map_err(|source| format!("failed to run launchctl {}: {source}", args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "launchctl {} failed: {}",
        args.join(" "),
        command_output_summary(output.status, &output.stdout, &output.stderr)
    ))
}

#[cfg(target_os = "macos")]
fn command_output_summary(
    status: std::process::ExitStatus,
    stdout: &[u8],
    stderr: &[u8],
) -> String {
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => format!("exit status {status}"),
        (false, true) => format!("exit status {status}, stdout: {stdout}"),
        (true, false) => format!("exit status {status}, stderr: {stderr}"),
        (false, false) => format!("exit status {status}, stdout: {stdout}, stderr: {stderr}"),
    }
}

#[cfg(target_os = "macos")]
fn launchctl_missing_service_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("could not find service")
        || error.contains("no such process")
        || error.contains("service is not loaded")
        || error.contains("not found")
}

#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct MacosLaunchAgentEnvironment {
    path: String,
    config_dir: Option<String>,
}

#[cfg(target_os = "macos")]
fn macos_launch_agent_environment() -> MacosLaunchAgentEnvironment {
    MacosLaunchAgentEnvironment {
        path: env::var("PATH")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(default_macos_launch_agent_path),
        config_dir: env::var(FOCO_CONFIG_DIR_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty()),
    }
}

#[cfg(target_os = "macos")]
fn default_macos_launch_agent_path() -> String {
    "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string()
}

#[cfg(any(target_os = "macos", test))]
fn macos_launch_agent_plist(
    executable_path: &str,
    environment: &MacosLaunchAgentEnvironment,
) -> String {
    let mut environment_entries = format!(
        "        <key>PATH</key>\n\
        <string>{}</string>\n",
        xml_escape(&environment.path)
    );
    if let Some(config_dir) = environment.config_dir.as_deref() {
        environment_entries.push_str(&format!(
            "        <key>{}</key>\n\
        <string>{}</string>\n",
            xml_escape(FOCO_CONFIG_DIR_ENV),
            xml_escape(config_dir)
        ));
    }

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
    <key>Label</key>\n\
    <string>{}</string>\n\
    <key>ProgramArguments</key>\n\
    <array>\n\
        <string>{}</string>\n\
    </array>\n\
    <key>EnvironmentVariables</key>\n\
    <dict>\n\
{}\
    </dict>\n\
    <key>RunAtLoad</key>\n\
    <true/>\n\
</dict>\n\
</plist>\n",
        xml_escape(MACOS_LAUNCH_AGENT_LABEL),
        xml_escape(executable_path),
        environment_entries
    )
}

#[cfg(any(target_os = "macos", test))]
fn macos_launch_agent_program_argument(plist: &str) -> Option<String> {
    let program_arguments_index = plist.find("<key>ProgramArguments</key>")?;
    let program_arguments = &plist[program_arguments_index..];
    let array_index = program_arguments.find("<array>")?;
    let array = &program_arguments[array_index..];
    let string_start = array.find("<string>")? + "<string>".len();
    let string_end = array[string_start..].find("</string>")? + string_start;
    Some(xml_unescape(&array[string_start..string_end]))
}

#[cfg(any(target_os = "macos", test))]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(any(target_os = "macos", test))]
fn xml_unescape(value: &str) -> String {
    value
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

#[cfg(windows)]
fn set_auto_start_registry_value(command: &str) -> Result<(), ApiError> {
    let value_name = wide_null(AUTO_START_REGISTRY_VALUE_NAME);
    let data_wide = wide_null(command);
    let data_bytes = wide_bytes(&data_wide);
    let data_len = u32::try_from(data_bytes.len())
        .map_err(|_| ApiError::internal("auto start registry command is too large"))?;
    let key = open_auto_start_registry_key()?;

    let set_result = unsafe {
        RegSetValueExW(
            key,
            value_name.as_ptr(),
            0,
            REG_SZ,
            data_bytes.as_ptr(),
            data_len,
        )
    };
    let close_result = unsafe { RegCloseKey(key) };

    if set_result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("set", set_result));
    }
    if close_result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("close", close_result));
    }

    Ok(())
}

#[cfg(windows)]
fn remove_auto_start_registry_value() -> Result<(), ApiError> {
    let key = open_existing_auto_start_registry_key()?;
    let value_name = wide_null(AUTO_START_REGISTRY_VALUE_NAME);

    let delete_result = unsafe { RegDeleteValueW(key, value_name.as_ptr()) };
    let close_result = unsafe { RegCloseKey(key) };

    if delete_result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("delete", delete_result));
    }
    if close_result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("close", close_result));
    }

    Ok(())
}

#[cfg(windows)]
fn open_auto_start_registry_key() -> Result<HKEY, ApiError> {
    let key_path = wide_null(AUTO_START_REGISTRY_RUN_KEY);
    let mut key = std::ptr::null_mut();

    let result = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key_path.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };

    if result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("open", result));
    }

    Ok(key)
}

#[cfg(windows)]
fn open_existing_auto_start_registry_key() -> Result<HKEY, ApiError> {
    let key_path = wide_null(AUTO_START_REGISTRY_RUN_KEY);
    let mut key = std::ptr::null_mut();

    let result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            key_path.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
    };

    if result != ERROR_SUCCESS {
        return Err(auto_start_registry_error("open", result));
    }

    Ok(key)
}

#[cfg(windows)]
fn auto_start_registry_error(action: &str, code: WIN32_ERROR) -> ApiError {
    ApiError::internal(format!(
        "failed to {action} Windows auto start registry entry: Win32 error {code}"
    ))
}

#[cfg(windows)]
fn wide_null(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn wide_bytes(value: &[u16]) -> Vec<u8> {
    value.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        MacosLaunchAgentEnvironment, macos_auto_start_executable_path_from_current_exe,
        macos_launch_agent_plist, macos_launch_agent_program_argument, windows_auto_start_command,
    };

    #[test]
    fn windows_auto_start_command_includes_internal_flag() {
        let command = windows_auto_start_command(Path::new(r"C:\Program Files\Foco\foco.exe"));

        assert_eq!(command, r#""C:\Program Files\Foco\foco.exe" --auto-start"#);
    }

    #[test]
    fn macos_launch_agent_plist_contains_required_keys() {
        let environment = MacosLaunchAgentEnvironment {
            path: "/opt/homebrew/bin:/usr/bin:/bin".to_string(),
            config_dir: Some("/Users/foco/.foco-dev".to_string()),
        };
        let plist =
            macos_launch_agent_plist("/Applications/Foco.app/Contents/MacOS/foco", &environment);

        assert!(plist.contains("<key>Label</key>"));
        assert!(plist.contains("<string>com.foco.app</string>"));
        assert!(plist.contains("<key>ProgramArguments</key>"));
        assert!(plist.contains("<string>/Applications/Foco.app/Contents/MacOS/foco</string>"));
        assert!(plist.contains("<key>EnvironmentVariables</key>"));
        assert!(plist.contains("<key>PATH</key>"));
        assert!(plist.contains("<string>/opt/homebrew/bin:/usr/bin:/bin</string>"));
        assert!(plist.contains("<key>FOCO_CONFIG_DIR</key>"));
        assert!(plist.contains("<string>/Users/foco/.foco-dev</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains("<true/>"));
    }

    #[test]
    fn macos_launch_agent_plist_omits_empty_optional_config_dir() {
        let environment = MacosLaunchAgentEnvironment {
            path: "/usr/bin:/bin".to_string(),
            config_dir: None,
        };
        let plist = macos_launch_agent_plist("/Applications/Foco & Tools/foco", &environment);

        assert!(!plist.contains("FOCO_CONFIG_DIR"));
        assert!(plist.contains("<string>/Applications/Foco &amp; Tools/foco</string>"));
    }

    #[test]
    fn macos_launch_agent_program_argument_reads_first_argument() {
        let environment = MacosLaunchAgentEnvironment {
            path: "/usr/bin:/bin".to_string(),
            config_dir: None,
        };
        let plist = macos_launch_agent_plist("/Applications/Foco & Tools/foco", &environment);

        assert_eq!(
            macos_launch_agent_program_argument(&plist).as_deref(),
            Some("/Applications/Foco & Tools/foco")
        );
    }

    #[test]
    fn macos_auto_start_executable_path_uses_current_app_bundle_binary() {
        let current_exe = Path::new("/Applications/Foco.app/Contents/MacOS/foco");

        assert_eq!(
            macos_auto_start_executable_path_from_current_exe(current_exe),
            current_exe
        );
    }
}
