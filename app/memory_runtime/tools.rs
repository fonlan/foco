use std::collections::HashSet;

use foco_providers::NeutralToolDefinition;
use foco_store::{
    config::MemorySettings,
    memory::{
        MemoryDatabase, MemoryFactRecord, MemoryKind, MemoryScope, MemorySourceType,
        MemoryStatus, NewMemoryFact, NewMemorySource,
    },
    workspace::workspace_database_path,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::http::memory::refresh_memory_profile;
use crate::memory_runtime::{
    apply_memory_expiration_to_fact, expire_due_memories, memory_fts_query,
};
use crate::*;

#[derive(Clone)]
pub(crate) struct MemoryToolContext {
    pub(crate) enabled: bool,
    pub(crate) workspace_path: PathBuf,
    pub(crate) global_memory_database_file: PathBuf,
    pub(crate) chat_id: String,
    pub(crate) run_id: String,
    pub(crate) tool_call_id: String,
    pub(crate) target_status: MemoryStatus,
    pub(crate) memory_settings: MemorySettings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MemoryToolSearchScope {
    Global,
    Workspace,
    Chat,
    Auto,
}

impl MemoryToolSearchScope {
    pub(crate) fn parse(value: &str) -> Result<Self, ApiError> {
        match value.trim() {
            "global" => Ok(Self::Global),
            "workspace" => Ok(Self::Workspace),
            "chat" => Ok(Self::Chat),
            "auto" => Ok(Self::Auto),
            other => Err(ApiError::bad_request(format!(
                "unknown memory search scope: {other}"
            ))),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Workspace => "workspace",
            Self::Chat => "chat",
            Self::Auto => "auto",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemorySearchToolInput {
    pub(crate) query: String,
    pub(crate) scope: String,
    pub(crate) limit: Option<u32>,
    pub(crate) include_related: Option<bool>,
    pub(crate) timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemoryWriteToolInput {
    pub(crate) scope: String,
    pub(crate) kind: String,
    pub(crate) fact: String,
    pub(crate) confidence: Option<f64>,
    pub(crate) pinned: Option<bool>,
    pub(crate) reason: Option<String>,
    pub(crate) timeout_ms: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct MemorySearchMatch {
    pub(crate) fact: MemoryFactRecord,
    pub(crate) match_source: String,
    pub(crate) source_count: i64,
}

pub(crate) fn memory_tool_definitions() -> Vec<NeutralToolDefinition> {
    vec![
        NeutralToolDefinition {
            name: MEMORY_SEARCH_TOOL_NAME.to_string(),
            description: "Search active Foco memories in global, workspace, current chat, or automatic combined scope. Returns fact ids, scope, source counts, and match source."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search text for memory facts. Must not be empty."
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["global", "workspace", "chat", "auto"],
                        "description": "Search scope. chat means current chat only; workspace means current workspace only; auto combines current chat, workspace, and global."
                    },
                    "limit": {
                        "type": ["integer", "null"],
                        "minimum": 1,
                        "maximum": MAX_MEMORY_TOOL_SEARCH_LIMIT,
                        "description": "Maximum direct matches per searched scope. Null uses the default."
                    },
                    "includeRelated": {
                        "type": ["boolean", "null"],
                        "description": "When true, include graph-related active memories linked to direct matches."
                    },
                    "timeoutMs": {
                        "type": ["integer", "null"],
                        "minimum": 1,
                        "maximum": MAX_MEMORY_TOOL_TIMEOUT_MS,
                        "description": "Tool timeout in milliseconds. Null uses the default."
                    }
                },
                "required": ["query", "scope", "limit", "includeRelated", "timeoutMs"]
            }),
            strict: true,
        },
        NeutralToolDefinition {
            name: MEMORY_WRITE_TOOL_NAME.to_string(),
            description: "Write a Foco memory fact with a source note. Facts are pending unless the user's current prompt explicitly asked Foco to remember it."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["global", "workspace", "chat"],
                        "description": "Memory storage scope."
                    },
                    "kind": {
                        "type": "string",
                        "enum": ["preference", "project_fact", "project_decision", "procedure", "constraint", "episode", "user_note"],
                        "description": "Memory kind."
                    },
                    "fact": {
                        "type": "string",
                        "description": "Atomic memory fact text. Must not be empty."
                    },
                    "confidence": {
                        "type": ["number", "null"],
                        "minimum": 0,
                        "maximum": 1
                    },
                    "pinned": {
                        "type": ["boolean", "null"]
                    },
                    "reason": {
                        "type": ["string", "null"],
                        "description": "Brief reason this fact should be saved."
                    },
                    "timeoutMs": {
                        "type": ["integer", "null"],
                        "minimum": 1,
                        "maximum": MAX_MEMORY_TOOL_TIMEOUT_MS,
                        "description": "Tool timeout in milliseconds. Null uses the default."
                    }
                },
                "required": ["scope", "kind", "fact", "confidence", "pinned", "reason", "timeoutMs"]
            }),
            strict: true,
        },
    ]
}

pub(crate) fn is_memory_tool_name(tool_name: &str) -> bool {
    matches!(tool_name, MEMORY_SEARCH_TOOL_NAME | MEMORY_WRITE_TOOL_NAME)
}

pub(crate) fn memory_tool_timeout_ms(arguments: &Value) -> Result<u64, String> {
    match arguments.get("timeoutMs") {
        Some(Value::Null) | None => Ok(DEFAULT_MEMORY_TOOL_TIMEOUT_MS),
        Some(Value::Number(timeout_ms)) => {
            let timeout_ms = timeout_ms
                .as_u64()
                .ok_or_else(|| "timeoutMs must be an integer or null".to_string())?;
            if timeout_ms == 0 || timeout_ms > MAX_MEMORY_TOOL_TIMEOUT_MS {
                Err(format!(
                    "timeoutMs must be between 1 and {MAX_MEMORY_TOOL_TIMEOUT_MS} milliseconds"
                ))
            } else {
                Ok(timeout_ms)
            }
        }
        Some(_) => Err("timeoutMs must be an integer or null".to_string()),
    }
}

pub(crate) fn execute_memory_tool(
    context: &MemoryToolContext,
    tool_name: &str,
    arguments: Value,
) -> Result<Value, String> {
    if !context.enabled {
        return Err("memory tools are disabled in settings".to_string());
    }

    match tool_name {
        MEMORY_SEARCH_TOOL_NAME => {
            let input =
                serde_json::from_value::<MemorySearchToolInput>(arguments).map_err(|source| {
                    format!("memory_search arguments do not match schema: {source}")
                })?;
            execute_memory_search_tool(context, input).map_err(|error| error.message)
        }
        MEMORY_WRITE_TOOL_NAME => {
            let input =
                serde_json::from_value::<MemoryWriteToolInput>(arguments).map_err(|source| {
                    format!("memory_write arguments do not match schema: {source}")
                })?;
            execute_memory_write_tool(context, input).map_err(|error| error.message)
        }
        other => Err(format!("unknown memory tool: {other}")),
    }
}

pub(crate) fn execute_memory_search_tool(
    context: &MemoryToolContext,
    input: MemorySearchToolInput,
) -> Result<Value, ApiError> {
    memory_tool_timeout_ms_from_input(input.timeout_ms)?;
    let query = normalized_required_text("query", &input.query)?;
    let search_query = memory_fts_query(&query).ok_or_else(|| {
        ApiError::bad_request("memory_search query must contain at least one searchable term")
    })?;
    let scope = MemoryToolSearchScope::parse(&input.scope)?;
    let limit = input
        .limit
        .unwrap_or(10)
        .clamp(1, MAX_MEMORY_TOOL_SEARCH_LIMIT);
    let include_related = input.include_related.unwrap_or(false);
    let mut matches = Vec::new();
    let mut seen = HashSet::new();

    match scope {
        MemoryToolSearchScope::Global => {
            let mut database =
                MemoryDatabase::open_or_create_global_at(&context.global_memory_database_file)
                    .map_err(ApiError::from_memory_error)?;
            expire_due_memories(&mut database)?;
            collect_memory_search_matches(
                &mut database,
                &search_query,
                MemoryToolSearchScope::Global,
                None,
                limit,
                include_related,
                &mut seen,
                &mut matches,
            )?;
        }
        MemoryToolSearchScope::Workspace => {
            let mut database =
                MemoryDatabase::open_workspace_at(workspace_database_path(&context.workspace_path))
                    .map_err(ApiError::from_memory_error)?;
            expire_due_memories(&mut database)?;
            collect_memory_search_matches(
                &mut database,
                &search_query,
                MemoryToolSearchScope::Workspace,
                None,
                limit,
                include_related,
                &mut seen,
                &mut matches,
            )?;
        }
        MemoryToolSearchScope::Chat => {
            let mut database =
                MemoryDatabase::open_workspace_at(workspace_database_path(&context.workspace_path))
                    .map_err(ApiError::from_memory_error)?;
            expire_due_memories(&mut database)?;
            collect_memory_search_matches(
                &mut database,
                &search_query,
                MemoryToolSearchScope::Chat,
                Some(&context.chat_id),
                limit,
                include_related,
                &mut seen,
                &mut matches,
            )?;
        }
        MemoryToolSearchScope::Auto => {
            let mut workspace_database =
                MemoryDatabase::open_workspace_at(workspace_database_path(&context.workspace_path))
                    .map_err(ApiError::from_memory_error)?;
            expire_due_memories(&mut workspace_database)?;
            collect_memory_search_matches(
                &mut workspace_database,
                &search_query,
                MemoryToolSearchScope::Chat,
                Some(&context.chat_id),
                limit,
                include_related,
                &mut seen,
                &mut matches,
            )?;
            collect_memory_search_matches(
                &mut workspace_database,
                &search_query,
                MemoryToolSearchScope::Workspace,
                None,
                limit,
                include_related,
                &mut seen,
                &mut matches,
            )?;

            let mut global_database =
                MemoryDatabase::open_or_create_global_at(&context.global_memory_database_file)
                    .map_err(ApiError::from_memory_error)?;
            expire_due_memories(&mut global_database)?;
            collect_memory_search_matches(
                &mut global_database,
                &search_query,
                MemoryToolSearchScope::Global,
                None,
                limit,
                include_related,
                &mut seen,
                &mut matches,
            )?;
        }
    }

    let fact_ids = matches
        .iter()
        .map(|item| item.fact.id.clone())
        .collect::<Vec<_>>();
    let total_source_count = matches.iter().map(|item| item.source_count).sum::<i64>();
    let memories = matches
        .into_iter()
        .map(|item| {
            json!({
                "id": item.fact.id,
                "scope": item.fact.scope,
                "chatId": item.fact.chat_id,
                "status": item.fact.status,
                "kind": item.fact.kind,
                "fact": item.fact.fact,
                "confidence": item.fact.confidence,
                "pinned": item.fact.pinned,
                "isLatest": item.fact.is_latest,
                "updatedAt": item.fact.updated_at,
                "sourceCount": item.source_count,
                "matchSource": item.match_source,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "summary": {
            "scope": scope.as_str(),
            "count": memories.len(),
            "factIds": fact_ids,
            "sourceCount": total_source_count,
        },
        "memories": memories,
    }))
}

pub(crate) fn collect_memory_search_matches(
    database: &mut MemoryDatabase,
    query: &str,
    scope: MemoryToolSearchScope,
    chat_id: Option<&str>,
    limit: u32,
    include_related: bool,
    seen: &mut HashSet<(String, String)>,
    matches: &mut Vec<MemorySearchMatch>,
) -> Result<(), ApiError> {
    let chat_filter = if scope == MemoryToolSearchScope::Chat {
        chat_id
    } else {
        None
    };
    let direct_facts = database
        .search_active_facts_for_scope(query, chat_filter, None, limit)
        .map_err(ApiError::from_memory_error)?;
    let direct_facts = direct_facts
        .into_iter()
        .filter(|fact| memory_search_fact_matches_scope(fact, scope, chat_id))
        .collect::<Vec<_>>();
    let direct_ids = direct_facts
        .iter()
        .map(|fact| fact.id.clone())
        .collect::<Vec<_>>();

    for fact in direct_facts {
        push_memory_search_match(database, fact, "direct", seen, matches)?;
    }

    if include_related {
        let related_facts = database
            .related_active_facts(
                &direct_ids,
                MEMORY_CONTEXT_EDGE_EXPANSION_DEPTH,
                MEMORY_CONTEXT_EDGE_EXPANSION_LIMIT,
            )
            .map_err(ApiError::from_memory_error)?;
        for fact in related_facts {
            push_memory_search_match(database, fact, "related", seen, matches)?;
        }
    }

    Ok(())
}

pub(crate) fn memory_search_fact_matches_scope(
    fact: &MemoryFactRecord,
    scope: MemoryToolSearchScope,
    chat_id: Option<&str>,
) -> bool {
    match scope {
        MemoryToolSearchScope::Global => fact.scope == "global",
        MemoryToolSearchScope::Workspace => fact.scope == "workspace",
        MemoryToolSearchScope::Chat => fact.scope == "chat" && fact.chat_id.as_deref() == chat_id,
        MemoryToolSearchScope::Auto => true,
    }
}

pub(crate) fn push_memory_search_match(
    database: &MemoryDatabase,
    fact: MemoryFactRecord,
    match_source: &str,
    seen: &mut HashSet<(String, String)>,
    matches: &mut Vec<MemorySearchMatch>,
) -> Result<(), ApiError> {
    if !seen.insert((fact.scope.clone(), fact.id.clone())) {
        return Ok(());
    }
    let source_count = database
        .source_count_for_fact(&fact.id)
        .map_err(ApiError::from_memory_error)?;
    matches.push(MemorySearchMatch {
        fact,
        match_source: match_source.to_string(),
        source_count,
    });
    Ok(())
}

pub(crate) fn execute_memory_write_tool(
    context: &MemoryToolContext,
    input: MemoryWriteToolInput,
) -> Result<Value, ApiError> {
    memory_tool_timeout_ms_from_input(input.timeout_ms)?;
    let scope = MemoryScope::parse(input.scope.trim()).map_err(ApiError::from_memory_error)?;
    let kind = MemoryKind::parse(input.kind.trim()).map_err(ApiError::from_memory_error)?;
    let fact = normalized_required_text("fact", &input.fact)?;
    let reason = normalized_optional_text(input.reason);
    let chat_id = (scope == MemoryScope::Chat).then_some(context.chat_id.as_str());
    let mut database = match scope {
        MemoryScope::Global => {
            MemoryDatabase::open_or_create_global_at(&context.global_memory_database_file)
        }
        MemoryScope::Workspace | MemoryScope::Chat => {
            MemoryDatabase::open_workspace_at(workspace_database_path(&context.workspace_path))
        }
    }
    .map_err(ApiError::from_memory_error)?;
    let source_id = unique_id("memory-source");
    let memory_id = unique_id("memory-fact");
    let metadata_json = serde_json::to_string(&json!({
        "source": MEMORY_WRITE_TOOL_NAME,
        "runId": &context.run_id,
        "toolCallId": &context.tool_call_id,
        "reason": reason,
    }))
    .map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize memory tool metadata: {source}"
        ))
    })?;
    let source_content = match reason.as_deref() {
        Some(reason) => format!("{fact}\n\nReason: {reason}"),
        None => fact.clone(),
    };
    database
        .insert_source(NewMemorySource {
            id: &source_id,
            scope,
            chat_id,
            source_type: MemorySourceType::ManualNote,
            source_id: Some(&context.tool_call_id),
            title: "Agent memory write",
            content: &source_content,
            metadata_json: &metadata_json,
        })
        .map_err(ApiError::from_memory_error)?;
    database
        .insert_fact(NewMemoryFact {
            id: &memory_id,
            scope,
            chat_id,
            status: context.target_status,
            kind,
            fact: &fact,
            confidence: input.confidence,
            pinned: input.pinned.unwrap_or(false),
            source_ids: &[source_id.as_str()],
            metadata_json: &metadata_json,
        })
        .map_err(ApiError::from_memory_error)?;
    apply_memory_expiration_to_fact(&mut database, &memory_id, &context.memory_settings)?;
    refresh_memory_profile(&mut database, scope, chat_id)?;
    let memory = database
        .fact(&memory_id)
        .map_err(ApiError::from_memory_error)?
        .ok_or_else(|| ApiError::internal(format!("memory fact was not found: {memory_id}")))?;
    let source_count = database
        .source_count_for_fact(&memory_id)
        .map_err(ApiError::from_memory_error)?;

    Ok(json!({
        "summary": {
            "scope": memory.scope,
            "status": memory.status,
            "factIds": [memory.id],
            "sourceCount": source_count,
        },
        "memory": {
            "id": memory.id,
            "scope": memory.scope,
            "chatId": memory.chat_id,
            "status": memory.status,
            "kind": memory.kind,
            "fact": memory.fact,
            "confidence": memory.confidence,
            "pinned": memory.pinned,
            "isLatest": memory.is_latest,
            "updatedAt": memory.updated_at,
            "sourceCount": source_count,
        }
    }))
}

pub(crate) fn memory_tool_timeout_ms_from_input(timeout_ms: Option<u64>) -> Result<u64, ApiError> {
    let timeout_ms = timeout_ms.unwrap_or(DEFAULT_MEMORY_TOOL_TIMEOUT_MS);
    if timeout_ms == 0 || timeout_ms > MAX_MEMORY_TOOL_TIMEOUT_MS {
        Err(ApiError::bad_request(format!(
            "timeoutMs must be between 1 and {MAX_MEMORY_TOOL_TIMEOUT_MS} milliseconds"
        )))
    } else {
        Ok(timeout_ms)
    }
}

pub(crate) fn memory_extraction_tool_definition() -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: MEMORY_EXTRACTION_TOOL_NAME.to_string(),
        description: "Submit extracted Foco memory facts with direct source evidence references."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "facts": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "scope": {
                                "type": "string",
                                "enum": ["global", "workspace", "chat"],
                                "description": "Suggested storage scope for the fact."
                            },
                            "kind": {
                                "type": "string",
                                "enum": ["preference", "project_fact", "project_decision", "procedure", "constraint", "episode"],
                                "description": "Memory kind. Do not use user_note for automatic extraction."
                            },
                            "fact": {
                                "type": "string",
                                "description": "Atomic durable fact text, directly supported by evidence."
                            },
                            "confidence": {
                                "type": ["number", "null"],
                                "minimum": 0,
                                "maximum": 1
                            },
                            "relationCandidates": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "properties": {
                                        "relation": {
                                            "type": "string",
                                            "enum": ["updates", "extends", "derives"]
                                        },
                                        "targetFactId": {
                                            "type": ["string", "null"]
                                        },
                                        "targetFact": {
                                            "type": ["string", "null"]
                                        },
                                        "reason": {
                                            "type": ["string", "null"]
                                        }
                                    },
                                    "required": ["relation", "targetFactId", "targetFact", "reason"]
                                }
                            },
                            "evidenceReferences": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "properties": {
                                        "evidenceId": {
                                            "type": "string",
                                            "description": "Must match one of the provided evidenceIds."
                                        },
                                        "quote": {
                                            "type": ["string", "null"]
                                        }
                                    },
                                    "required": ["evidenceId", "quote"]
                                }
                            }
                        },
                        "required": ["scope", "kind", "fact", "confidence", "relationCandidates", "evidenceReferences"]
                    }
                }
            },
            "required": ["facts"]
        }),
        strict: true,
    }
}

pub(crate) fn memory_retrieval_tool_definition() -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: MEMORY_RETRIEVAL_TOOL_NAME.to_string(),
        description: "Submit selected relevant Foco memory fact keys for the current user request."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "factKeys": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Relevant memory fact keys from the candidate list, ordered by injection priority. Use an empty array when no memory is relevant."
                }
            },
            "required": ["factKeys"]
        }),
        strict: true,
    }
}
