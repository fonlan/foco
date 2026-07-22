use std::{fs, path::Path};

use foco_agent::{ToolPromptInfo, build_default_system_prompt};
use foco_mcp::McpToolDefinition;
use foco_providers::{
    NeutralChatMessage, NeutralChatRole, NeutralToolDefinition, NeutralToolKind, ProviderKind,
    WebSearchMode, WebSearchRoute, WebSearchRouteInput, resolve_web_search_route,
    upstream_provider_model_id,
};
use foco_store::config::{
    DEFAULT_SYSTEM_PROMPT_NAME, IMAGE_GENERATION_SYSTEM_PROMPT_NAME, PLAN_MODE_SYSTEM_PROMPT_NAME,
    PromptSettings, ProviderSettings, REVIEW_SYSTEM_PROMPT_NAME, WebSearchSettings,
};
use foco_tools::{SEARCH_TEXT_TOOL, WEB_SEARCH_TOOL, builtin_tool_definitions};
use serde_json::json;

use crate::{
    AGENTS_MESSAGE_PREFIX, ApiError, EXTRA_PROMPT_MESSAGE_PREFIX, PROMPT_FILE_MESSAGE_PREFIX,
    http::settings::{
        SystemPromptSummary, default_image_generation_system_prompt,
        default_plan_mode_system_prompt, default_review_system_prompt,
    },
    markdown_code_block, neutral_text_message,
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
    if name == IMAGE_GENERATION_SYSTEM_PROMPT_NAME {
        return Ok(default_image_generation_system_prompt());
    }
    if name == PLAN_MODE_SYSTEM_PROMPT_NAME {
        return Ok(default_plan_mode_system_prompt());
    }
    if name == REVIEW_SYSTEM_PROMPT_NAME {
        return Ok(default_review_system_prompt());
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
    web_search_function_available: bool,
) -> Vec<foco_tools::ToolDefinition> {
    builtin_tool_definitions()
        .into_iter()
        .filter(|tool| ripgrep_available || tool.name != SEARCH_TEXT_TOOL)
        .filter(|tool| web_search_function_available || tool.name != WEB_SEARCH_TOOL)
        .collect()
}

/// Resolve the single web-search route for a chat turn after active model/provider routing.
///
/// Callers must pass the already-resolved active provider and model settings for this turn.
/// Capability is never inferred from tool names.
pub(crate) fn resolve_web_search_route_for_turn(
    web_search: &WebSearchSettings,
    model_mode: WebSearchMode,
    provider: &ProviderSettings,
    model_id: &str,
) -> WebSearchRoute {
    let provider_kind = parse_provider_kind_for_web_search(&provider.kind);
    let upstream_model_id = provider_kind
        .and_then(|_| upstream_provider_model_id(model_id, &provider.model_redirects).ok())
        .unwrap_or(model_id);
    resolve_web_search_route(WebSearchRouteInput {
        enabled: web_search.enabled,
        fallback_available: web_search.fallback_available(),
        provider_kind,
        upstream_model_id,
        mode: model_mode,
    })
}

fn parse_provider_kind_for_web_search(kind: &str) -> Option<ProviderKind> {
    foco_providers::parse_provider_kind(kind).ok()
}

/// Provider-native web search capability injected into `provider_request.tools` only.
///
/// This is not a Foco-executable builtin: `execute_tool_with_runtime` must never run it.
pub(crate) fn provider_native_web_search_tool_definition() -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: WEB_SEARCH_TOOL.to_string(),
        description: "Provider-native web search executed by the model provider (not Foco's Tavily/Brave function tool).".to_string(),
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {},
        }),
        strict: false,
        kind: NeutralToolKind::ProviderWebSearch,
    }
}

/// Whether the Foco function `web_search` tool should appear in the executable runtime catalog.
pub(crate) fn web_search_function_tool_available(route: WebSearchRoute) -> bool {
    matches!(route, WebSearchRoute::FocoFunction)
}

/// Whether a provider-native web search capability should be injected into the provider request.
pub(crate) fn web_search_provider_native_available(route: WebSearchRoute) -> bool {
    matches!(route, WebSearchRoute::ProviderNative)
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
        })
        .chain(memory_tools.iter().map(|tool| ToolPromptInfo {
            name: tool.name.clone(),
        }))
        .chain(mcp_tools.iter().map(|tool| ToolPromptInfo {
            name: tool.name.clone(),
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
        NeutralChatRole::Developer,
        format!(
            "## Prompt File Context\n\nSource: {prefix}\n\nPath: `{}`\n\n{}",
            path.display(),
            markdown_code_block("markdown", content.trim())
        ),
    )))
}

fn extra_prompt_message(content: &str) -> Option<NeutralChatMessage> {
    let content = content.trim();
    if content.is_empty() {
        return None;
    }

    Some(neutral_text_message(
        NeutralChatRole::Developer,
        format!(
            "## Extra Prompt Context\n\nSource: {EXTRA_PROMPT_MESSAGE_PREFIX}\n\n{}",
            markdown_code_block("markdown", content)
        ),
    ))
}
