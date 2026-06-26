use std::{env, fs, path::Path};

use chrono::{Local, SecondsFormat};
use foco_providers::{NeutralChatMessage, NeutralChatRole};

use crate::{
    ApiError, ENVIRONMENT_CONTEXT_MESSAGE_PREFIX, git_backend::is_git_workspace,
    neutral_text_message, non_empty_string, xml_text_escape,
};

pub(crate) fn environment_context_message(
    workspace_path: &Path,
) -> Result<NeutralChatMessage, ApiError> {
    let now = Local::now();
    let shell = detected_shell()?;
    let git_repository = is_git_workspace(workspace_path)?;
    let wsl = is_wsl_environment();

    Ok(neutral_text_message(
        NeutralChatRole::User,
        format!(
            "<environment_context>\n\
             <source>{}</source>\n\
             <workspace_directory>{}</workspace_directory>\n\
             <git_repository>{}</git_repository>\n\
             <shell type=\"{}\">{}</shell>\n\
             <current_date>{}</current_date>\n\
             <local_timestamp>{}</local_timestamp>\n\
             <time_zone>{}</time_zone>\n\
             <wsl>{}</wsl>\n\
             </environment_context>",
            xml_text_escape(ENVIRONMENT_CONTEXT_MESSAGE_PREFIX),
            xml_text_escape(&workspace_path.display().to_string()),
            git_repository,
            xml_text_escape(&shell.kind),
            xml_text_escape(&shell.executable),
            now.format("%Y-%m-%d"),
            now.to_rfc3339_opts(SecondsFormat::Secs, false),
            now.offset(),
            wsl
        ),
    ))
}

struct DetectedShell {
    kind: String,
    executable: String,
}

fn detected_shell() -> Result<DetectedShell, ApiError> {
    if cfg!(windows) {
        return Ok(DetectedShell {
            kind: "powershell".to_string(),
            executable: "powershell.exe".to_string(),
        });
    }

    let shell = env::var("SHELL").map_err(|source| {
        ApiError::internal(format!(
            "failed to detect shell from SHELL environment: {source}"
        ))
    })?;
    let shell = non_empty_string(shell.trim()).ok_or_else(|| {
        ApiError::bad_request("SHELL environment variable is empty; cannot detect shell type")
    })?;
    let kind = Path::new(&shell)
        .file_stem()
        .and_then(|name| name.to_str())
        .and_then(non_empty_string)
        .ok_or_else(|| {
            ApiError::bad_request(format!("failed to detect shell type from SHELL={shell}"))
        })?;

    Ok(DetectedShell {
        kind,
        executable: shell,
    })
}

pub(crate) fn is_wsl_environment() -> bool {
    if env::var_os("WSL_DISTRO_NAME").is_some() || env::var_os("WSL_INTEROP").is_some() {
        return true;
    }

    if !cfg!(target_os = "linux") {
        return false;
    }

    fs::read_to_string("/proc/version")
        .map(|version| version.to_ascii_lowercase().contains("microsoft"))
        .unwrap_or(false)
}
