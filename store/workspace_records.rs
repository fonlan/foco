use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::workspace::WorkspaceDatabaseError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatRecord {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
    pub metadata_json: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeChangeStats {
    pub additions: usize,
    pub deletions: usize,
}

impl CodeChangeStats {
    pub(crate) fn from_metadata(value: &Value) -> Result<Self, WorkspaceDatabaseError> {
        let Some(additions) = value.get("additions").and_then(Value::as_u64) else {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "message metadata.codeChangeStats.additions must be an unsigned integer"
                    .to_string(),
            });
        };
        let Some(deletions) = value.get("deletions").and_then(Value::as_u64) else {
            return Err(WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "message metadata.codeChangeStats.deletions must be an unsigned integer"
                    .to_string(),
            });
        };

        let additions = usize::try_from(additions).map_err(|_| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "message metadata.codeChangeStats.additions is too large".to_string(),
            }
        })?;
        let deletions = usize::try_from(deletions).map_err(|_| {
            WorkspaceDatabaseError::InvalidMessageMetadata {
                message: "message metadata.codeChangeStats.deletions is too large".to_string(),
            }
        })?;

        Ok(Self {
            additions,
            deletions,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewMessage<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub role: &'a str,
    pub content: &'a str,
    pub sequence: i64,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageRecord {
    pub id: String,
    pub chat_id: String,
    pub role: String,
    pub content: String,
    pub sequence: i64,
    pub created_at: String,
    pub metadata_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRunEvent<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub run_id: &'a str,
    pub sequence: i64,
    pub event_type: &'a str,
    pub payload_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunEventRecord {
    pub id: String,
    pub chat_id: String,
    pub run_id: String,
    pub sequence: i64,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewToolCall<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub run_id: &'a str,
    pub message_id: Option<&'a str>,
    pub tool_name: &'a str,
    pub input_json: &'a str,
    pub status: &'a str,
    pub started_at: &'a str,
    pub completed_at: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewToolResult<'a> {
    pub id: &'a str,
    pub tool_call_id: &'a str,
    pub output_json: &'a str,
    pub is_error: bool,
    pub created_at: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCallWithResultRecord {
    pub id: String,
    pub chat_id: String,
    pub run_id: String,
    pub message_id: Option<String>,
    pub tool_name: String,
    pub input_json: String,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub result: Option<ToolResultRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolResultRecord {
    pub id: String,
    pub tool_call_id: String,
    pub output_json: String,
    pub is_error: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCallCountRecord {
    pub tool_name: String,
    pub call_count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewLlmRequest<'a> {
    pub id: &'a str,
    pub workspace_id: &'a str,
    pub chat_id: Option<&'a str>,
    pub provider_id: &'a str,
    pub model_id: &'a str,
    pub request_started_at: &'a str,
    pub first_token_at: Option<&'a str>,
    pub completed_at: Option<&'a str>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub status_code: Option<i64>,
    pub final_state: &'a str,
    pub request_body_json: Option<&'a str>,
    pub response_body_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateLlmRequestOutcome<'a> {
    pub first_token_at: Option<&'a str>,
    pub completed_at: Option<&'a str>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub status_code: Option<i64>,
    pub final_state: &'a str,
    pub response_body_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LlmRequestRecord {
    pub id: String,
    pub workspace_id: Option<String>,
    pub chat_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub request_started_at: String,
    pub first_token_at: Option<String>,
    pub completed_at: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub cache_ratio: Option<f64>,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub status_code: Option<i64>,
    pub final_state: String,
    pub request_body_json: Option<String>,
    pub response_body_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmRequestMetricsRecord {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub output_tokens: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LlmRequestAuditFilters<'a> {
    pub workspace_id: Option<&'a str>,
    pub chat_id: Option<&'a str>,
    pub provider_id: Option<&'a str>,
    pub model_id: Option<&'a str>,
    pub final_state: Option<&'a str>,
    pub started_after: Option<&'a str>,
    pub started_before: Option<&'a str>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LlmRequestAuditRow {
    pub id: String,
    pub workspace_id: Option<String>,
    pub chat_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub request_started_at: String,
    pub first_token_at: Option<String>,
    pub completed_at: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub cache_ratio: Option<f64>,
    pub first_token_latency_ms: Option<i64>,
    pub total_latency_ms: Option<i64>,
    pub status_code: Option<i64>,
    pub final_state: String,
}

#[derive(Clone, Debug, Default)]
pub struct LlmRequestAuditSummaryRow {
    pub total_requests: i64,
    pub failed_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub total_cache_write_tokens: i64,
    pub total_tokens: i64,
    pub latency_count: i64,
    pub latency_sum: i64,
}

#[derive(Clone, Debug)]
pub struct LlmRequestAuditTrendPoint {
    pub bucket: String,
    pub request_count: i64,
    pub total_tokens: i64,
}

#[derive(Clone, Debug)]
pub struct LlmRequestAuditModelBreakdown {
    pub model_id: String,
    pub request_count: i64,
    pub total_tokens: i64,
}

#[derive(Clone, Debug)]
pub struct LlmRequestAuditProviderBreakdown {
    pub provider_id: String,
    pub request_count: i64,
    pub success_count: i64,
    pub total_tokens: i64,
    pub latency_count: i64,
    pub latency_sum: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewLlmRequestEvent<'a> {
    pub id: &'a str,
    pub llm_request_id: &'a str,
    pub sequence: i64,
    pub event_at: &'a str,
    pub event_type: &'a str,
    pub raw_chunk_json: Option<&'a str>,
    pub normalized_event_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmRequestEventRecord {
    pub id: String,
    pub llm_request_id: String,
    pub sequence: i64,
    pub event_at: String,
    pub event_type: String,
    pub raw_chunk_json: Option<String>,
    pub normalized_event_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewContextCompressionSnapshot<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub run_id: &'a str,
    pub sequence: i64,
    pub summary: &'a str,
    pub source_message_start_sequence: i64,
    pub source_message_end_sequence: i64,
    pub original_token_count: i64,
    pub summary_token_count: i64,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextCompressionSnapshotRecord {
    pub id: String,
    pub chat_id: String,
    pub run_id: String,
    pub sequence: i64,
    pub summary: String,
    pub source_message_start_sequence: i64,
    pub source_message_end_sequence: i64,
    pub original_token_count: i64,
    pub summary_token_count: i64,
    pub created_at: String,
    pub metadata_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPromptContextInjection<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub kind: &'a str,
    pub sequence: Option<i64>,
    pub messages_json: &'a str,
    pub memory_keys_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptContextInjectionRecord {
    pub id: String,
    pub chat_id: String,
    pub kind: String,
    pub sequence: Option<i64>,
    pub messages_json: String,
    pub memory_keys_json: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTerminalSession<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub working_directory: &'a str,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSessionRecord {
    pub id: String,
    pub name: String,
    pub working_directory: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub metadata_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewHookRun<'a> {
    pub id: &'a str,
    pub workspace_id: &'a str,
    pub chat_id: Option<&'a str>,
    pub run_id: Option<&'a str>,
    pub tool_call_id: Option<&'a str>,
    pub event: &'a str,
    pub hook_source: &'a str,
    pub handler_type: &'a str,
    pub input_json: &'a str,
    pub output_json: Option<&'a str>,
    pub status: &'a str,
    pub exit_code: Option<i64>,
    pub stdout_preview: Option<&'a str>,
    pub stderr_preview: Option<&'a str>,
    pub started_at: &'a str,
    pub completed_at: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookRunRecord {
    pub id: String,
    pub workspace_id: String,
    pub chat_id: Option<String>,
    pub run_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub event: String,
    pub hook_source: String,
    pub handler_type: String,
    pub input_json: String,
    pub output_json: Option<String>,
    pub status: String,
    pub exit_code: Option<i64>,
    pub stdout_preview: Option<String>,
    pub stderr_preview: Option<String>,
    pub started_at: String,
    pub completed_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoGraphTask {
    pub id: String,
    pub title: String,
    pub status: String,
    pub depends_on: Vec<String>,
    pub acceptance: Vec<String>,
    pub summary: String,
    pub created_at: String,
    pub updated_at: String,
    pub subtasks: Vec<TodoGraphTask>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoGraphRecord {
    pub chat_id: String,
    pub tasks: Vec<TodoGraphTask>,
    pub created_at: String,
    pub updated_at: String,
    pub updated_task: Option<TodoGraphTask>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoGraphTaskPatch {
    pub title: Option<String>,
    pub status: Option<String>,
    pub depends_on: Option<Vec<String>>,
    pub acceptance: Option<Vec<String>>,
    pub summary: Option<String>,
    pub subtasks: Option<Vec<TodoGraphTask>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TodoGraphFilter<'a> {
    pub status: Option<&'a str>,
    pub task_id: Option<&'a str>,
    pub include_subtasks: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCodeGraphFileIndex<'a> {
    pub path: &'a str,
    pub language: Option<&'a str>,
    pub size_bytes: Option<i64>,
    pub modified_at: Option<&'a str>,
    pub content_hash: &'a str,
    pub parse_status: &'a str,
    pub parse_error_message: Option<&'a str>,
    pub symbols: &'a [NewCodeGraphSymbol<'a>],
    pub imports: &'a [NewCodeGraphImport<'a>],
    pub references: &'a [NewCodeGraphReference<'a>],
    pub edges: &'a [NewCodeGraphEdge<'a>],
    pub fts_body: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCodeGraphSymbol<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
    pub signature: Option<&'a str>,
    pub documentation: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCodeGraphImport<'a> {
    pub module: &'a str,
    pub imported_symbol: Option<&'a str>,
    pub alias: Option<&'a str>,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCodeGraphReference<'a> {
    pub name: &'a str,
    pub symbol_index: Option<usize>,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCodeGraphEdge<'a> {
    pub source_symbol_index: usize,
    pub target_symbol_index: usize,
    pub edge_kind: &'a str,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphContextRecord {
    pub indexed_files: i64,
    pub symbols: i64,
    pub references: i64,
    pub edges: i64,
    pub languages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphSymbolRecord {
    pub id: i64,
    pub path: String,
    pub language: Option<String>,
    pub name: String,
    pub kind: String,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
    pub signature: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphSymbolRelationRecord {
    pub edge_id: i64,
    pub edge_kind: String,
    pub metadata_json: String,
    pub source: CodeGraphSymbolRecord,
    pub target: CodeGraphSymbolRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphReferenceRecord {
    pub id: i64,
    pub path: String,
    pub language: Option<String>,
    pub name: String,
    pub start_line: Option<i64>,
    pub start_column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
    pub symbol: Option<CodeGraphSymbolRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeGraphRelatedFileRecord {
    pub path: String,
    pub language: Option<String>,
    pub relation: String,
    pub score: i64,
}
