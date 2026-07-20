use serde_json::{Value, json};

use crate::{
    AGENT_CANCEL_TASK_TOOL, AGENT_CREATE_INSTANCES_TOOL, AGENT_DELEGATE_TASK_TOOL,
    AGENT_GET_TASK_TOOL, AGENT_LIST_TOOL, AGENT_SEND_MESSAGE_TOOL, AGENT_TRANSFER_TASK_TOOL,
    AGENT_WAIT_TASKS_TOOL, ToolDefinition,
};

/// Matches `foco_agent::validate_agent_id` total length bound.
const AGENT_ID_MAX_LENGTH: u64 = 128;
/// Matches `AgentDefinitionId::PREFIX` without importing `foco-agent`.
/// Also used by unit tests that mirror runtime prefix checks.
#[cfg_attr(not(test), allow(dead_code))]
const AGENT_DEFINITION_ID_PREFIX: &str = "agent-definition-";
/// Matches `AgentInstanceId::PREFIX` without importing `foco-agent`.
#[cfg_attr(not(test), allow(dead_code))]
const AGENT_INSTANCE_ID_PREFIX: &str = "agent-instance-";
/// Suffix must be non-empty ascii lowercase / digit / hyphen (same as runtime).
const AGENT_DEFINITION_ID_PATTERN: &str = "^agent-definition-[a-z0-9-]+$";
const AGENT_INSTANCE_ID_PATTERN: &str = "^agent-instance-[a-z0-9-]+$";

pub(crate) fn agent_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        agent_list_definition(),
        agent_get_task_definition(),
        agent_send_message_definition(),
        agent_delegate_task_definition(),
        agent_cancel_task_definition(),
        agent_wait_tasks_definition(),
        agent_transfer_task_definition(),
        agent_create_instances_definition(),
    ]
}

/// JSON Schema fragment for Agent definition ids.
/// Kept in `foco-tools` (not `foco-agent`) to avoid a tools→agent dependency cycle.
fn agent_definition_id_schema(description: &str, nullable: bool) -> Value {
    agent_id_schema(AGENT_DEFINITION_ID_PATTERN, description, nullable)
}

/// JSON Schema fragment for Agent instance ids.
fn agent_instance_id_schema(description: &str, nullable: bool) -> Value {
    agent_id_schema(AGENT_INSTANCE_ID_PATTERN, description, nullable)
}

fn agent_id_schema(pattern: &str, description: &str, nullable: bool) -> Value {
    // `pattern` + `maxLength` mirror runtime validate_agent_id. OpenAI-style strict tool
    // schemas already accept sibling constraints such as `minimum`; keep both machine-readable
    // constraints so providers that honor them reject illegal IDs before execution.
    let type_value = if nullable {
        json!(["string", "null"])
    } else {
        json!("string")
    };
    json!({
        "type": type_value,
        "pattern": pattern,
        "maxLength": AGENT_ID_MAX_LENGTH,
        "description": description
    })
}

fn agent_list_definition() -> ToolDefinition {
    ToolDefinition {
        name: AGENT_LIST_TOOL,
        description: "List the current Agent team definitions, instances, status, and queue summary visible to this Agent.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["timeoutMs"]
        }),
        strict: true,
    }
}

fn agent_get_task_definition() -> ToolDefinition {
    ToolDefinition {
        name: AGENT_GET_TASK_TOOL,
        description: "Read the status, result, and structured error for a task in the current Agent team.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "Agent task id to inspect. Must belong to the current team."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["taskId", "timeoutMs"]
        }),
        strict: true,
    }
}

fn agent_send_message_definition() -> ToolDefinition {
    ToolDefinition {
        name: AGENT_SEND_MESSAGE_TOOL,
        description: "Send a persistent point-to-point message to another instance in the current Agent team. This does not create a task or wake an idle model run. When the matching receiver task is already running, the message is applied as guidance for that run; otherwise it remains queued for a later attempt.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "receiverInstanceId": {
                    "type": "string",
                    "description": "Target Agent instance id. Names and broadcast are not accepted."
                },
                "kind": {
                    "type": "string",
                    "enum": ["notification", "reply"],
                    "description": "Message kind. Use notification for one-way information and reply for a response to an earlier message."
                },
                "content": {
                    "type": "string",
                    "description": "Message content."
                },
                "replyToMessageId": {
                    "type": ["string", "null"],
                    "description": "Optional message id this reply refers to."
                },
                "relatedTaskId": {
                    "type": ["string", "null"],
                    "description": "Optional related Agent task id in the current team."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["receiverInstanceId", "kind", "content", "replyToMessageId", "relatedTaskId", "timeoutMs"]
        }),
        strict: true,
    }
}

fn agent_delegate_task_definition() -> ToolDefinition {
    ToolDefinition {
        name: AGENT_DELEGATE_TASK_TOOL,
        description: "Create an asynchronous child task for an existing instance in the current Agent team. Returns immediately with the task id and selected instance id. Copy IDs exactly from agent_list.definitions[].id or agent_list.instances[].id; never use display names, role names, or hand-constructed IDs. Provide exactly one of targetInstanceId or targetDefinitionId (set the unused field to null). targetDefinitionId only routes to an existing runnable instance in the current team and never auto-creates instances. If no suitable instance exists: call agent_list, then agent_create_instances when allowed, then delegate with a returned instance id.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "targetInstanceId": agent_instance_id_schema(
                    "Exact target Agent instance id. Must use the agent-instance- prefix with a non-empty lowercase/digit/hyphen suffix (max 128 chars total). Copy from agent_list.instances[].id. Provide exactly one of targetInstanceId or targetDefinitionId (set the unused field to null).",
                    true,
                ),
                "targetDefinitionId": agent_definition_id_schema(
                    "Target Agent definition id. Must use the agent-definition- prefix with a non-empty lowercase/digit/hyphen suffix (max 128 chars total). Copy from agent_list.definitions[].id. Routes only to an existing runnable instance in the current team; does not auto-create instances. Provide exactly one of targetInstanceId or targetDefinitionId (set the unused field to null).",
                    true,
                ),
                "input": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "Task message for the child Agent."
                        }
                    },
                    "required": ["message"],
                    "description": "Task input for the child Agent."
                },
                "correlationId": {
                    "type": ["string", "null"],
                    "description": "Optional caller-chosen correlation id for matching the child task with later results."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["targetInstanceId", "targetDefinitionId", "input", "correlationId", "timeoutMs"]
        }),
        strict: true,
    }
}

fn agent_cancel_task_definition() -> ToolDefinition {
    ToolDefinition {
        name: AGENT_CANCEL_TASK_TOOL,
        description: "Cancel a queued child task in the current Agent team. Running and waiting task cancellation must use the runtime API.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "Queued child Agent task id to cancel."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["taskId", "timeoutMs"]
        }),
        strict: true,
    }
}

fn agent_wait_tasks_definition() -> ToolDefinition {
    ToolDefinition {
        name: AGENT_WAIT_TASKS_TOOL,
        description: "Persistently wait for all specified Agent tasks in the current team, suspend the current run, and resume later with a paired tool result.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "taskIds": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "type": "string" },
                    "description": "Agent task ids to wait for. Every task must belong to the current team and be visible to this Agent."
                },
                "mode": {
                    "type": "string",
                    "enum": ["all"],
                    "description": "Wait mode. Phase 7 supports all only."
                },
                "deadlineMs": {
                    "type": ["integer", "null"],
                    "description": "Optional relative deadline in milliseconds. Null means no deadline."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["taskIds", "mode", "deadlineMs", "timeoutMs"]
        }),
        strict: true,
    }
}

fn agent_transfer_task_definition() -> ToolDefinition {
    ToolDefinition {
        name: AGENT_TRANSFER_TASK_TOOL,
        description: "Transfer a queued Agent task to another existing instance in the current team. Running, waiting, and terminal tasks are rejected. Copy targetInstanceId from agent_list.instances[].id; never use display names or hand-constructed IDs.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "Queued Agent task id to transfer."
                },
                "targetInstanceId": agent_instance_id_schema(
                    "Existing target Agent instance id in the same team. Must use the agent-instance- prefix with a non-empty lowercase/digit/hyphen suffix (max 128 chars total). Copy from agent_list.instances[].id.",
                    false,
                ),
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["taskId", "targetInstanceId", "timeoutMs"]
        }),
        strict: true,
    }
}

fn agent_create_instances_definition() -> ToolDefinition {
    ToolDefinition {
        name: AGENT_CREATE_INSTANCES_TOOL,
        description: "Create one or more worker Agent instances for an allowed definition in the current team. Creation is atomic and never routes work implicitly. Copy definitionId from agent_list.definitions[].id; never use display names or hand-constructed IDs.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "definitionId": agent_definition_id_schema(
                    "Agent definition id to instantiate. Must use the agent-definition- prefix with a non-empty lowercase/digit/hyphen suffix (max 128 chars total). Copy from agent_list.definitions[].id. Must be allowed by the caller permissions.",
                    false,
                ),
                "count": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Number of instances to create atomically."
                },
                "executionWorkspaceMode": {
                    "type": "string",
                    "enum": ["shared", "isolated_worktree"],
                    "description": "Execution workspace mode for created workers. Use shared for the main workspace or isolated_worktree for an explicit Foco-managed Git worktree."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["definitionId", "count", "executionWorkspaceMode", "timeoutMs"]
        }),
        strict: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use std::collections::BTreeSet;

    #[test]
    fn agent_tool_schemas_are_openai_responses_strict_compatible() {
        for definition in agent_tool_definitions() {
            assert!(definition.strict, "{} must be strict", definition.name);
            assert_eq!(
                definition
                    .input_schema
                    .get("type")
                    .and_then(|value| value.as_str()),
                Some("object"),
                "{} must use an object schema",
                definition.name
            );
            assert_eq!(
                definition
                    .input_schema
                    .get("additionalProperties")
                    .and_then(|value| value.as_bool()),
                Some(false),
                "{} must reject extra properties",
                definition.name
            );
            let properties = definition
                .input_schema
                .get("properties")
                .and_then(|value| value.as_object())
                .expect("properties object");
            let required = definition
                .input_schema
                .get("required")
                .and_then(|value| value.as_array())
                .expect("required array")
                .iter()
                .map(|value| value.as_str().expect("required string"))
                .collect::<Vec<_>>();

            assert!(
                properties.contains_key("timeoutMs"),
                "{} must expose timeoutMs",
                definition.name
            );
            assert_eq!(
                required.len(),
                properties.len(),
                "{} must require every property",
                definition.name
            );
            for property in properties.keys() {
                assert!(
                    required.contains(&property.as_str()),
                    "{} must require property {}",
                    definition.name,
                    property
                );
            }
            assert_strict_schema_children(&definition.name, &definition.input_schema);
        }
    }

    #[test]
    fn agent_id_schemas_match_runtime_format_contract() {
        let definitions = agent_tool_definitions();
        let by_name = |name: &str| {
            definitions
                .iter()
                .find(|definition| definition.name == name)
                .unwrap_or_else(|| panic!("missing tool {name}"))
        };

        let delegate = by_name(AGENT_DELEGATE_TASK_TOOL);
        assert_agent_id_property(
            &delegate.input_schema["properties"]["targetDefinitionId"],
            AGENT_DEFINITION_ID_PATTERN,
            true,
            &["agent_list.definitions[].id", "does not auto-create"],
        );
        assert_agent_id_property(
            &delegate.input_schema["properties"]["targetInstanceId"],
            AGENT_INSTANCE_ID_PATTERN,
            true,
            &["agent_list.instances[].id", "exactly one of"],
        );

        let transfer = by_name(AGENT_TRANSFER_TASK_TOOL);
        assert_agent_id_property(
            &transfer.input_schema["properties"]["targetInstanceId"],
            AGENT_INSTANCE_ID_PATTERN,
            false,
            &["agent_list.instances[].id"],
        );

        let create = by_name(AGENT_CREATE_INSTANCES_TOOL);
        assert_agent_id_property(
            &create.input_schema["properties"]["definitionId"],
            AGENT_DEFINITION_ID_PATTERN,
            false,
            &["agent_list.definitions[].id"],
        );

        // Fixed degradation policy: keep both pattern and maxLength as public machine
        // constraints (same family as existing `minimum` on integer fields). Do not drop
        // either without updating this test and provider strict-schema guidance.
        for schema in [
            &delegate.input_schema["properties"]["targetDefinitionId"],
            &delegate.input_schema["properties"]["targetInstanceId"],
            &transfer.input_schema["properties"]["targetInstanceId"],
            &create.input_schema["properties"]["definitionId"],
        ] {
            assert_eq!(
                schema.get("maxLength").and_then(|value| value.as_u64()),
                Some(AGENT_ID_MAX_LENGTH)
            );
            assert!(
                schema
                    .get("pattern")
                    .and_then(|value| value.as_str())
                    .is_some()
            );
        }
    }

    #[test]
    fn agent_id_schema_patterns_accept_runtime_valid_ids_and_reject_invalid() {
        // Compile the published schema pattern strings with a real regex engine so a
        // typo or over-broad pattern cannot pass by mapping to parallel hand logic.
        let definition_re =
            Regex::new(AGENT_DEFINITION_ID_PATTERN).expect("definition id pattern compiles");
        let instance_re =
            Regex::new(AGENT_INSTANCE_ID_PATTERN).expect("instance id pattern compiles");

        // Valid ids: short, multi-segment, timestamp-style, hyphenated suffix.
        for valid in [
            "agent-definition-1",
            "agent-definition-worker",
            "agent-definition-1700000000000-1",
            "agent-definition-a-b-c",
        ] {
            assert!(
                definition_re.is_match(valid) && valid.len() <= AGENT_ID_MAX_LENGTH as usize,
                "expected valid definition id: {valid}"
            );
            assert!(valid.starts_with(AGENT_DEFINITION_ID_PREFIX));
            assert!(
                matches_runtime_agent_id(valid, AGENT_DEFINITION_ID_PREFIX),
                "schema-valid definition id must also satisfy runtime rules: {valid}"
            );
        }
        // Invalid definition ids: missing prefix, empty suffix, uppercase, underscore,
        // wrong type prefix, leading capital, regex metacharacters.
        for (label, invalid) in [
            ("missing_prefix", "definition-1"),
            ("empty_suffix", "agent-definition-"),
            ("uppercase", "agent-definition-UPPER"),
            ("underscore", "agent-definition-with_underscore"),
            ("wrong_type_prefix", "agent-instance-1"),
            ("leading_capital_prefix", "Agent-definition-1"),
            ("regex_metachar", "agent-definition-.*"),
            ("display_name", "Review"),
        ] {
            assert!(
                !definition_re.is_match(invalid),
                "expected invalid definition id ({label}): {invalid}"
            );
            assert!(
                !matches_runtime_agent_id(invalid, AGENT_DEFINITION_ID_PREFIX),
                "runtime must reject definition id ({label}): {invalid}"
            );
        }

        for valid in [
            "agent-instance-1",
            "agent-instance-review",
            "agent-instance-1782058615186-10",
        ] {
            assert!(
                instance_re.is_match(valid) && valid.len() <= AGENT_ID_MAX_LENGTH as usize,
                "expected valid instance id: {valid}"
            );
            assert!(valid.starts_with(AGENT_INSTANCE_ID_PREFIX));
            assert!(
                matches_runtime_agent_id(valid, AGENT_INSTANCE_ID_PREFIX),
                "schema-valid instance id must also satisfy runtime rules: {valid}"
            );
        }
        for (label, invalid) in [
            ("missing_prefix", "instance-1"),
            ("empty_suffix", "agent-instance-"),
            ("uppercase", "agent-instance-UPPER"),
            ("underscore", "agent-instance-with_underscore"),
            ("wrong_type_prefix", "agent-definition-1"),
            ("display_name", "worker-1"),
        ] {
            assert!(
                !instance_re.is_match(invalid),
                "expected invalid instance id ({label}): {invalid}"
            );
            assert!(
                !matches_runtime_agent_id(invalid, AGENT_INSTANCE_ID_PREFIX),
                "runtime must reject instance id ({label}): {invalid}"
            );
        }

        // Pattern allows suffix charset only; total length is maxLength (and runtime).
        let too_long_definition = format!(
            "{AGENT_DEFINITION_ID_PREFIX}{}",
            "a".repeat(AGENT_ID_MAX_LENGTH as usize)
        );
        assert!(too_long_definition.len() > AGENT_ID_MAX_LENGTH as usize);
        assert!(
            definition_re.is_match(&too_long_definition),
            "pattern alone does not enforce maxLength"
        );
        assert!(
            !matches_runtime_agent_id(&too_long_definition, AGENT_DEFINITION_ID_PREFIX),
            "runtime rejects oversized definition id"
        );
        let too_long_instance = format!(
            "{AGENT_INSTANCE_ID_PREFIX}{}",
            "a".repeat(AGENT_ID_MAX_LENGTH as usize)
        );
        assert!(too_long_instance.len() > AGENT_ID_MAX_LENGTH as usize);
        assert!(instance_re.is_match(&too_long_instance));
        assert!(!matches_runtime_agent_id(
            &too_long_instance,
            AGENT_INSTANCE_ID_PREFIX
        ));

        // Boundary: exact maxLength remains valid for both schema pattern and runtime.
        let max_len_suffix_len = AGENT_ID_MAX_LENGTH as usize - AGENT_DEFINITION_ID_PREFIX.len();
        let max_len_definition = format!(
            "{AGENT_DEFINITION_ID_PREFIX}{}",
            "a".repeat(max_len_suffix_len)
        );
        assert_eq!(max_len_definition.len(), AGENT_ID_MAX_LENGTH as usize);
        assert!(definition_re.is_match(&max_len_definition));
        assert!(matches_runtime_agent_id(
            &max_len_definition,
            AGENT_DEFINITION_ID_PREFIX
        ));
    }

    #[test]
    fn agent_delegate_task_description_documents_id_source_and_no_auto_create() {
        let description = agent_delegate_task_definition().description;
        for fragment in [
            "agent_list.definitions[].id",
            "agent_list.instances[].id",
            "exactly one of targetInstanceId or targetDefinitionId",
            "never auto-creates",
            "agent_create_instances",
            "display names",
        ] {
            assert!(
                description.contains(fragment),
                "delegate description missing `{fragment}`: {description}"
            );
        }

        let schema = agent_delegate_task_definition().input_schema;
        let target_instance = schema["properties"]["targetInstanceId"]["description"]
            .as_str()
            .expect("targetInstanceId description");
        let target_definition = schema["properties"]["targetDefinitionId"]["description"]
            .as_str()
            .expect("targetDefinitionId description");
        assert!(target_instance.contains("exactly one of targetInstanceId or targetDefinitionId"));
        assert!(
            target_definition.contains("exactly one of targetInstanceId or targetDefinitionId")
        );
        assert!(target_definition.contains("does not auto-create"));
        assert!(target_instance.contains("agent_list.instances[].id"));
        assert!(target_definition.contains("agent_list.definitions[].id"));

        let create_description = agent_create_instances_definition().description;
        assert!(create_description.contains("agent_list.definitions[].id"));
        let transfer_description = agent_transfer_task_definition().description;
        assert!(transfer_description.contains("agent_list.instances[].id"));
    }

    fn assert_agent_id_property(
        schema: &Value,
        expected_pattern: &str,
        nullable: bool,
        description_fragments: &[&str],
    ) {
        if nullable {
            assert_eq!(schema["type"], json!(["string", "null"]));
        } else {
            assert_eq!(schema["type"], json!("string"));
        }
        assert_eq!(
            schema.get("pattern").and_then(|value| value.as_str()),
            Some(expected_pattern)
        );
        assert_eq!(
            schema.get("maxLength").and_then(|value| value.as_u64()),
            Some(AGENT_ID_MAX_LENGTH)
        );
        let description = schema
            .get("description")
            .and_then(|value| value.as_str())
            .expect("agent id property description");
        assert!(!description.is_empty());
        for fragment in description_fragments {
            assert!(
                description.contains(fragment),
                "agent id description missing `{fragment}`: {description}"
            );
        }
    }

    /// Runtime-aligned id rules (mirrors `foco_agent::validate_agent_id` without a crate edge).
    fn matches_runtime_agent_id(value: &str, prefix: &str) -> bool {
        let Some(suffix) = value
            .strip_prefix(prefix)
            .filter(|suffix| !suffix.is_empty())
        else {
            return false;
        };
        value.len() <= AGENT_ID_MAX_LENGTH as usize
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    }

    fn assert_strict_schema_object(tool_name: &str, schema: &serde_json::Value) {
        let schema_object = schema.as_object().expect("schema object");
        if schema_object.get("type").and_then(|value| value.as_str()) == Some("object") {
            assert_eq!(
                schema_object.get("additionalProperties"),
                Some(&serde_json::Value::Bool(false)),
                "{tool_name} object schema must reject unknown properties"
            );
            let properties = schema_object
                .get("properties")
                .and_then(|value| value.as_object())
                .expect("properties object");
            let required = schema_object
                .get("required")
                .and_then(|value| value.as_array())
                .expect("required array");
            let property_names = properties
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            let required_names = required
                .iter()
                .map(|name| name.as_str().expect("required name"))
                .collect::<BTreeSet<_>>();

            assert_eq!(
                required_names, property_names,
                "{tool_name} required keys must match object properties"
            );
        }
    }

    fn assert_strict_schema_children(tool_name: &str, schema: &serde_json::Value) {
        if schema.get("type").and_then(|value| value.as_str()) == Some("object") {
            assert_strict_schema_object(tool_name, schema);
        }
        if let Some(properties) = schema
            .as_object()
            .and_then(|object| object.get("properties"))
        {
            for value in properties.as_object().expect("properties object").values() {
                assert_strict_schema_children(tool_name, value);
            }
        }
        if let Some(items) = schema.get("items") {
            assert_strict_schema_children(tool_name, items);
        }
    }
}
