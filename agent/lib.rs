use std::{collections::HashMap, fmt};

use serde_json::Value;

const ESTIMATED_CHARS_PER_TOKEN: u64 = 4;
const DEFAULT_CONTEXT_SAFETY_TOKENS: u64 = 256;
const CONTEXT_COMPRESSION_TRIGGER_NUMERATOR: u64 = 4;
const CONTEXT_COMPRESSION_TRIGGER_DENOMINATOR: u64 = 5;
pub const WRITE_FILE_TOOL_NAME: &str = "write_file";
pub const PATCH_FILE_TOOL_NAME: &str = "patch_file";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolPromptInfo {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemPromptInput {
    pub workspace_id: String,
    pub workspace_name: String,
    pub workspace_path: String,
    pub tools: Vec<ToolPromptInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextBudget {
    pub context_window: u64,
    pub max_output_tokens: u64,
    pub system_prompt_tokens: u64,
    pub tool_schema_tokens: u64,
    pub safety_tokens: u64,
    pub available_message_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextPackItem {
    pub id: String,
    pub estimated_tokens: u64,
    pub must_keep: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedContext {
    pub selected_indices: Vec<usize>,
    pub dropped_ids: Vec<String>,
    pub used_message_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextCompressionPlan {
    pub covered_indices: Vec<usize>,
    pub original_tokens: u64,
    pub trigger_tokens: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ContextBudgetError {
    OutputExceedsWindow {
        context_window: u64,
        max_output_tokens: u64,
    },
    ReservedExceedsWindow {
        context_window: u64,
        reserved_tokens: u64,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum ContextPackError {
    RequiredMessagesExceedBudget {
        required_tokens: u64,
        available_tokens: u64,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum ToolConflictError {
    MissingWritePath {
        call_id: String,
    },
    SameFileWrite {
        path: String,
        first_call_id: String,
        second_call_id: String,
    },
}

pub fn build_system_prompt(input: SystemPromptInput) -> String {
    let mut prompt = format!(
        "You are Foco, a local coding agent.\n\
         Follow the user's request directly, inspect the workspace before changing behavior, and report explicit errors instead of hiding missing required data.\n\n\
         Workspace:\n\
         - id: {}\n\
         - name: {}\n\
         - path: {}\n\n\
         Tool rules:\n\
         - Use workspace-relative paths.\n\
         - Use tools when you need current file or workspace evidence.\n\
         - Prefer code graph tools before full-text search when locating symbols, callers, callees, references, or related files.\n\
         - Treat graph tool JSON outputs as compact structured code graph context; use returned symbolId values for follow-up graph queries.\n\
         - Use run_command for git commands such as status and diff; there is no dedicated git_diff tool.\n\
         - After tool results are returned, continue the same run and answer the user.",
        input.workspace_id, input.workspace_name, input.workspace_path
    );

    if !input.tools.is_empty() {
        prompt.push_str("\n\nAvailable tools:");
        for tool in input.tools {
            prompt.push_str("\n- ");
            prompt.push_str(&tool.name);
            prompt.push_str(": ");
            prompt.push_str(&tool.description);
        }
    }

    prompt
}

pub fn estimate_text_tokens(text: &str) -> u64 {
    let char_count = text.chars().count() as u64;

    if char_count == 0 {
        0
    } else {
        char_count.div_ceil(ESTIMATED_CHARS_PER_TOKEN)
    }
}

pub fn estimate_json_tokens(value: &Value) -> u64 {
    estimate_text_tokens(&value.to_string())
}

pub fn calculate_context_budget(
    context_window: u64,
    max_output_tokens: u64,
    system_prompt_tokens: u64,
    tool_schema_tokens: u64,
) -> Result<ContextBudget, ContextBudgetError> {
    calculate_context_budget_with_safety(
        context_window,
        max_output_tokens,
        system_prompt_tokens,
        tool_schema_tokens,
        DEFAULT_CONTEXT_SAFETY_TOKENS,
    )
}

pub fn calculate_context_budget_with_safety(
    context_window: u64,
    max_output_tokens: u64,
    system_prompt_tokens: u64,
    tool_schema_tokens: u64,
    safety_tokens: u64,
) -> Result<ContextBudget, ContextBudgetError> {
    if max_output_tokens >= context_window {
        return Err(ContextBudgetError::OutputExceedsWindow {
            context_window,
            max_output_tokens,
        });
    }

    let reserved_tokens = max_output_tokens
        .saturating_add(system_prompt_tokens)
        .saturating_add(tool_schema_tokens)
        .saturating_add(safety_tokens);

    if reserved_tokens >= context_window {
        return Err(ContextBudgetError::ReservedExceedsWindow {
            context_window,
            reserved_tokens,
        });
    }

    Ok(ContextBudget {
        context_window,
        max_output_tokens,
        system_prompt_tokens,
        tool_schema_tokens,
        safety_tokens,
        available_message_tokens: context_window - reserved_tokens,
    })
}

pub fn pack_context(
    messages: &[ContextPackItem],
    available_tokens: u64,
) -> Result<PackedContext, ContextPackError> {
    let required_tokens = messages
        .iter()
        .filter(|message| message.must_keep)
        .map(|message| message.estimated_tokens)
        .sum::<u64>();

    if required_tokens > available_tokens {
        return Err(ContextPackError::RequiredMessagesExceedBudget {
            required_tokens,
            available_tokens,
        });
    }

    let mut selected = vec![false; messages.len()];
    let mut remaining_tokens = available_tokens - required_tokens;

    for (index, message) in messages.iter().enumerate() {
        if message.must_keep {
            selected[index] = true;
        }
    }

    for (index, message) in messages.iter().enumerate().rev() {
        if selected[index] {
            continue;
        }

        if message.estimated_tokens <= remaining_tokens {
            selected[index] = true;
            remaining_tokens -= message.estimated_tokens;
        }
    }

    let mut selected_indices = Vec::new();
    let mut dropped_ids = Vec::new();
    let mut used_message_tokens = 0;

    for (index, message) in messages.iter().enumerate() {
        if selected[index] {
            selected_indices.push(index);
            used_message_tokens += message.estimated_tokens;
        } else {
            dropped_ids.push(message.id.clone());
        }
    }

    Ok(PackedContext {
        selected_indices,
        dropped_ids,
        used_message_tokens,
    })
}

pub fn plan_context_compression(
    messages: &[ContextPackItem],
    available_tokens: u64,
    active_tool_start_index: usize,
    preserve_recent_messages: usize,
) -> Option<ContextCompressionPlan> {
    if available_tokens == 0 {
        return None;
    }

    let used_tokens = messages
        .iter()
        .map(|message| message.estimated_tokens)
        .sum::<u64>();
    let trigger_tokens = context_compression_trigger_tokens(available_tokens);

    if used_tokens <= trigger_tokens {
        return None;
    }

    let compressible_indices = messages
        .iter()
        .enumerate()
        .filter(|(index, message)| {
            *index < active_tool_start_index && !message.must_keep && message.estimated_tokens > 0
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    if compressible_indices.len() <= preserve_recent_messages {
        return None;
    }

    let covered_count = compressible_indices.len() - preserve_recent_messages;
    let covered_indices = compressible_indices
        .into_iter()
        .take(covered_count)
        .collect::<Vec<_>>();
    let original_tokens = covered_indices
        .iter()
        .map(|index| messages[*index].estimated_tokens)
        .sum::<u64>();

    if original_tokens == 0 {
        return None;
    }

    Some(ContextCompressionPlan {
        covered_indices,
        original_tokens,
        trigger_tokens,
    })
}

pub fn detect_same_file_write_conflicts(
    tool_calls: &[PendingToolCall],
) -> Result<(), ToolConflictError> {
    let mut writes_by_path = HashMap::new();

    for tool_call in tool_calls {
        if !matches!(
            tool_call.name.as_str(),
            WRITE_FILE_TOOL_NAME | PATCH_FILE_TOOL_NAME
        ) {
            continue;
        }

        let Some(path) = tool_call.arguments.get("path").and_then(Value::as_str) else {
            return Err(ToolConflictError::MissingWritePath {
                call_id: tool_call.id.clone(),
            });
        };
        let normalized_path = normalize_workspace_path(path);

        if let Some(first_call_id) = writes_by_path.insert(normalized_path.clone(), &tool_call.id) {
            return Err(ToolConflictError::SameFileWrite {
                path: normalized_path,
                first_call_id: first_call_id.clone(),
                second_call_id: tool_call.id.clone(),
            });
        }
    }

    Ok(())
}

fn normalize_workspace_path(path: &str) -> String {
    path.trim()
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
        .to_ascii_lowercase()
}

pub fn context_compression_trigger_tokens(available_tokens: u64) -> u64 {
    available_tokens.saturating_mul(CONTEXT_COMPRESSION_TRIGGER_NUMERATOR)
        / CONTEXT_COMPRESSION_TRIGGER_DENOMINATOR
}

impl fmt::Display for ContextBudgetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputExceedsWindow {
                context_window,
                max_output_tokens,
            } => write!(
                formatter,
                "model max output tokens ({max_output_tokens}) must be smaller than context window ({context_window})"
            ),
            Self::ReservedExceedsWindow {
                context_window,
                reserved_tokens,
            } => write!(
                formatter,
                "context budget reserved tokens ({reserved_tokens}) exceed context window ({context_window})"
            ),
        }
    }
}

impl std::error::Error for ContextBudgetError {}

impl fmt::Display for ContextPackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiredMessagesExceedBudget {
                required_tokens,
                available_tokens,
            } => write!(
                formatter,
                "required context messages need {required_tokens} tokens but only {available_tokens} are available"
            ),
        }
    }
}

impl std::error::Error for ContextPackError {}

impl fmt::Display for ToolConflictError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingWritePath { call_id } => write!(
                formatter,
                "file-writing tool call '{call_id}' is missing a string path for conflict detection"
            ),
            Self::SameFileWrite {
                path,
                first_call_id,
                second_call_id,
            } => write!(
                formatter,
                "same-file write conflict for '{path}' between tool calls '{first_call_id}' and '{second_call_id}'"
            ),
        }
    }
}

impl std::error::Error for ToolConflictError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn system_prompt_includes_static_workspace_and_tool_rules() {
        let prompt = build_system_prompt(SystemPromptInput {
            workspace_id: "workspace-1".to_string(),
            workspace_name: "Workspace".to_string(),
            workspace_path: "C:/project".to_string(),
            tools: vec![ToolPromptInfo {
                name: "graph_find_symbols".to_string(),
                description: "Find symbols.".to_string(),
            }],
        });

        assert!(prompt.contains("- id: workspace-1"));
        assert!(prompt.contains("- path: C:/project"));
        assert!(prompt.contains("Prefer code graph tools before full-text search"));
        assert!(prompt.contains("graph_find_symbols"));
        assert!(!prompt.contains("Code graph context:"));
        assert!(!prompt.contains("Enabled skills:"));
    }

    #[test]
    fn calculates_context_budget_from_model_limits() {
        let budget =
            calculate_context_budget_with_safety(128_000, 16_384, 100, 300, 256).expect("budget");

        assert_eq!(budget.available_message_tokens, 110_960);
    }

    #[test]
    fn rejects_context_budget_when_reserved_tokens_exceed_window() {
        let error = calculate_context_budget_with_safety(1_000, 800, 100, 80, 50)
            .expect_err("reserved tokens should exceed");

        assert_eq!(
            error,
            ContextBudgetError::ReservedExceedsWindow {
                context_window: 1_000,
                reserved_tokens: 1_030
            }
        );
    }

    #[test]
    fn packs_context_by_dropping_old_optional_messages() {
        let messages = vec![
            ContextPackItem {
                id: "system".to_string(),
                estimated_tokens: 10,
                must_keep: true,
            },
            ContextPackItem {
                id: "old".to_string(),
                estimated_tokens: 80,
                must_keep: false,
            },
            ContextPackItem {
                id: "recent".to_string(),
                estimated_tokens: 30,
                must_keep: false,
            },
            ContextPackItem {
                id: "tool-state".to_string(),
                estimated_tokens: 15,
                must_keep: true,
            },
        ];

        let packed = pack_context(&messages, 60).expect("packed context");

        assert_eq!(packed.selected_indices, vec![0, 2, 3]);
        assert_eq!(packed.dropped_ids, vec!["old"]);
        assert_eq!(packed.used_message_tokens, 55);
    }

    #[test]
    fn plans_compression_for_old_optional_messages_before_active_tools() {
        let messages = vec![
            ContextPackItem {
                id: "system".to_string(),
                estimated_tokens: 0,
                must_keep: true,
            },
            ContextPackItem {
                id: "old-user".to_string(),
                estimated_tokens: 70,
                must_keep: false,
            },
            ContextPackItem {
                id: "old-assistant".to_string(),
                estimated_tokens: 70,
                must_keep: false,
            },
            ContextPackItem {
                id: "recent-user".to_string(),
                estimated_tokens: 70,
                must_keep: false,
            },
            ContextPackItem {
                id: "latest-user".to_string(),
                estimated_tokens: 30,
                must_keep: true,
            },
            ContextPackItem {
                id: "tool-call".to_string(),
                estimated_tokens: 120,
                must_keep: true,
            },
        ];

        let plan = plan_context_compression(&messages, 300, 5, 1).expect("compression plan");

        assert_eq!(plan.covered_indices, vec![1, 2]);
        assert_eq!(plan.original_tokens, 140);
        assert_eq!(plan.trigger_tokens, 240);
    }

    #[test]
    fn skips_compression_before_trigger_threshold() {
        let messages = vec![ContextPackItem {
            id: "message".to_string(),
            estimated_tokens: 50,
            must_keep: false,
        }];

        assert_eq!(plan_context_compression(&messages, 300, 1, 1), None);
    }

    #[test]
    fn detects_same_file_write_conflicts_inside_one_turn() {
        let calls = vec![
            PendingToolCall {
                id: "call-a".to_string(),
                name: WRITE_FILE_TOOL_NAME.to_string(),
                arguments: json!({ "path": "src/main.rs" }),
            },
            PendingToolCall {
                id: "call-b".to_string(),
                name: "read_file".to_string(),
                arguments: json!({ "path": "src/main.rs" }),
            },
            PendingToolCall {
                id: "call-c".to_string(),
                name: PATCH_FILE_TOOL_NAME.to_string(),
                arguments: json!({ "path": ".\\src\\main.rs" }),
            },
        ];

        let error = detect_same_file_write_conflicts(&calls).expect_err("conflict");

        assert_eq!(
            error,
            ToolConflictError::SameFileWrite {
                path: "src/main.rs".to_string(),
                first_call_id: "call-a".to_string(),
                second_call_id: "call-c".to_string(),
            }
        );
    }
}
