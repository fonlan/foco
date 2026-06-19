use serde_json::json;

use crate::{
    AGENT_CANCEL_TASK_TOOL, AGENT_CREATE_INSTANCES_TOOL, AGENT_DELEGATE_TASK_TOOL,
    AGENT_GET_TASK_TOOL, AGENT_LIST_TOOL, AGENT_SEND_MESSAGE_TOOL, AGENT_TRANSFER_TASK_TOOL,
    AGENT_WAIT_TASKS_TOOL, ToolDefinition,
};

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
        description: "Send a persistent point-to-point message to another instance in the current Agent team. This does not create a task or wake an idle model run.",
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
        description: "Create an asynchronous child task for an existing instance in the current Agent team. Returns immediately with the task id and selected instance id.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "targetInstanceId": {
                    "type": ["string", "null"],
                    "description": "Exact target Agent instance id. Provide exactly one of targetInstanceId or targetDefinitionId."
                },
                "targetDefinitionId": {
                    "type": ["string", "null"],
                    "description": "Target Agent definition id. Uses an existing instance only; no instance is auto-created. Provide exactly one of targetInstanceId or targetDefinitionId."
                },
                "input": {
                    "type": "object",
                    "description": "JSON task input for the child Agent task."
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
        description: "Transfer a queued Agent task to another existing instance in the current team. Running, waiting, and terminal tasks are rejected.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "Queued Agent task id to transfer."
                },
                "targetInstanceId": {
                    "type": "string",
                    "description": "Existing target Agent instance id in the same team."
                },
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
        description: "Create one or more worker Agent instances for an allowed definition in the current team. Creation is atomic and never routes work implicitly.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "definitionId": {
                    "type": "string",
                    "description": "Agent definition id to instantiate. Must be allowed by the caller permissions."
                },
                "count": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Number of instances to create atomically."
                },
                "maxInstancesPerTeam": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Explicit per-team instance limit for this create request."
                },
                "maxInstancesForDefinition": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Explicit per-definition instance limit for this create request."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["definitionId", "count", "maxInstancesPerTeam", "maxInstancesForDefinition", "timeoutMs"]
        }),
        strict: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }
}
