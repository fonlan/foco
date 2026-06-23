use std::{fs, path::Path};

use foco_agent::{ToolPromptInfo, build_default_system_prompt};
use foco_mcp::McpToolDefinition;
use foco_providers::{NeutralChatMessage, NeutralChatRole, NeutralToolDefinition};
use foco_store::config::{DEFAULT_SYSTEM_PROMPT_NAME, PromptSettings};
use foco_tools::{SEARCH_TEXT_TOOL, WEB_SEARCH_TOOL, builtin_tool_definitions};

use crate::{
    AGENTS_MESSAGE_PREFIX, ApiError, EXTRA_PROMPT_MESSAGE_PREFIX, PROMPT_FILE_MESSAGE_PREFIX,
    SystemPromptSummary, neutral_text_message,
};

pub(crate) fn active_system_prompt(
    settings: &PromptSettings,
    name: &str,
) -> Result<String, ApiError> {
    if let Some(prompt) = settings
        .system_prompts
        .iter()
        .find(|prompt| prompt.name == name)
    {
        return Ok(prompt.content.clone());
    }

    if name == DEFAULT_SYSTEM_PROMPT_NAME {
        return Ok(settings
            .system_prompt
            .clone()
            .unwrap_or_else(build_default_system_prompt));
    }

    Err(ApiError::bad_request(format!(
        "system prompt '{}' was not found",
        name
    )))
}

pub(crate) fn system_prompt_summaries(
    settings: &PromptSettings,
    default_system_prompt: &str,
) -> Vec<SystemPromptSummary> {
    let mut summaries = Vec::new();
    let mut has_default = false;

    for prompt in &settings.system_prompts {
        if prompt.name == DEFAULT_SYSTEM_PROMPT_NAME {
            has_default = true;
        }
        summaries.push(SystemPromptSummary {
            name: prompt.name.clone(),
            content: prompt.content.clone(),
        });
    }

    if !has_default {
        summaries.insert(
            0,
            SystemPromptSummary {
                name: DEFAULT_SYSTEM_PROMPT_NAME.to_string(),
                content: settings
                    .system_prompt
                    .clone()
                    .unwrap_or_else(|| default_system_prompt.to_string()),
            },
        );
    }

    summaries
}

pub(crate) fn builtin_tool_definitions_for_runtime(
    ripgrep_available: bool,
    web_search_available: bool,
) -> Vec<foco_tools::ToolDefinition> {
    builtin_tool_definitions()
        .into_iter()
        .filter(|tool| ripgrep_available || tool.name != SEARCH_TEXT_TOOL)
        .filter(|tool| web_search_available || tool.name != WEB_SEARCH_TOOL)
        .collect()
}

pub(crate) fn tool_prompt_infos(
    builtin_tools: &[foco_tools::ToolDefinition],
    memory_tools: &[NeutralToolDefinition],
    mcp_tools: &[McpToolDefinition],
) -> Vec<ToolPromptInfo> {
    builtin_tools
        .iter()
        .map(|tool| ToolPromptInfo {
            name: tool.name.to_string(),
            description: tool.description.to_string(),
        })
        .chain(memory_tools.iter().map(|tool| ToolPromptInfo {
            name: tool.name.clone(),
            description: tool.description.clone(),
        }))
        .chain(mcp_tools.iter().map(|tool| ToolPromptInfo {
            name: tool.name.clone(),
            description: format!(
                "{} MCP server '{}': {}",
                tool.original_name, tool.server_name, tool.description
            ),
        }))
        .collect()
}

pub(crate) fn agents_prompt_messages(
    workspace_path: &Path,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    let mut messages = Vec::new();
    let path = workspace_path.join("AGENTS.md");

    if let Some(message) = prompt_file_message(&path, AGENTS_MESSAGE_PREFIX, "AGENTS.md path")? {
        messages.push(message);
    }

    Ok(messages)
}

pub(crate) fn configured_prompt_messages(
    settings: &PromptSettings,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    let mut messages = Vec::new();

    for path in &settings.files {
        if let Some(message) = prompt_file_message(path, PROMPT_FILE_MESSAGE_PREFIX, "prompt file")?
        {
            messages.push(message);
        }
    }

    Ok(messages)
}

pub(crate) fn configured_extra_prompt_message(
    settings: &PromptSettings,
) -> Option<NeutralChatMessage> {
    extra_prompt_message(&settings.extra_text)
}

fn prompt_file_message(
    path: &Path,
    prefix: &str,
    field_name: &str,
) -> Result<Option<NeutralChatMessage>, ApiError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ApiError::internal(format!(
                "failed to inspect {}: {source}",
                path.display()
            )));
        }
    };

    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "{field_name} is not a file: {}",
            path.display()
        )));
    }

    let content = fs::read_to_string(path).map_err(|source| {
        ApiError::internal(format!("failed to read {}: {source}", path.display()))
    })?;

    if content.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(neutral_text_message(
        NeutralChatRole::User,
        format!("{prefix} {}:\n\n{}", path.display(), content.trim()),
    )))
}

fn extra_prompt_message(content: &str) -> Option<NeutralChatMessage> {
    let content = content.trim();
    if content.is_empty() {
        return None;
    }

    Some(neutral_text_message(
        NeutralChatRole::System,
        format!("{EXTRA_PROMPT_MESSAGE_PREFIX}\n\n{content}"),
    ))
}
