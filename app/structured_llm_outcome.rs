//! Stable classification for single-tool structured LLM requests.
//!
//! Codes never include model/prompt body text. They are persisted on `llm_requests`
//! for baseline first-attempt / terminal success metrics.

use foco_store::workspace::{
    STRUCTURED_LLM_OUTCOME_MISSING_TOOL, STRUCTURED_LLM_OUTCOME_OTHER,
    STRUCTURED_LLM_OUTCOME_PROVIDER_ERROR, STRUCTURED_LLM_OUTCOME_PROVIDER_TIMEOUT,
    STRUCTURED_LLM_OUTCOME_SCHEMA_INVALID, STRUCTURED_LLM_OUTCOME_SEMANTIC_INVALID,
    STRUCTURED_LLM_OUTCOME_SUCCEEDED, STRUCTURED_LLM_OUTCOME_TEXT_JSON_RECOVERED,
    STRUCTURED_LLM_OUTCOME_WRONG_TOOL, STRUCTURED_LLM_RECOVERY_CORRECTION_RETRY,
    STRUCTURED_LLM_RECOVERY_NONE, STRUCTURED_LLM_RECOVERY_TEXT_JSON,
    STRUCTURED_LLM_RECOVERY_TOOL_CALL, StructuredLlmRequestClassification,
    WorkspaceDatabase, WorkspaceDatabaseError,
};
use std::path::Path;

/// Successful tool-call path (real ToolCall arguments).
pub fn classification_succeeded_tool_call(attempt_index: i64) -> StructuredLlmRequestClassification<'static> {
    StructuredLlmRequestClassification {
        structured_outcome: STRUCTURED_LLM_OUTCOME_SUCCEEDED,
        recovery_source: STRUCTURED_LLM_RECOVERY_TOOL_CALL,
        attempt_index,
    }
}

/// Successful text-JSON recovery path.
pub fn classification_text_json_recovered(
    attempt_index: i64,
) -> StructuredLlmRequestClassification<'static> {
    StructuredLlmRequestClassification {
        structured_outcome: STRUCTURED_LLM_OUTCOME_TEXT_JSON_RECOVERED,
        recovery_source: STRUCTURED_LLM_RECOVERY_TEXT_JSON,
        attempt_index,
    }
}

/// Successful path after a correction retry (later phases / explicit retry feedback).
pub fn classification_correction_retry_success(
    attempt_index: i64,
) -> StructuredLlmRequestClassification<'static> {
    StructuredLlmRequestClassification {
        structured_outcome: STRUCTURED_LLM_OUTCOME_SUCCEEDED,
        recovery_source: STRUCTURED_LLM_RECOVERY_CORRECTION_RETRY,
        attempt_index,
    }
}

/// Classify a provider/stream/tool-protocol failure message into a stable code.
///
/// Does not capture the message body itself—only maps known patterns.
pub fn classify_provider_tool_failure_message(message: &str) -> StructuredLlmRequestClassification<'static> {
    let attempt_index = 1;
    let outcome = classify_provider_tool_failure_outcome(message);
    StructuredLlmRequestClassification {
        structured_outcome: outcome,
        recovery_source: STRUCTURED_LLM_RECOVERY_NONE,
        attempt_index,
    }
}

pub fn classify_provider_tool_failure_outcome(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("timed out after") {
        STRUCTURED_LLM_OUTCOME_PROVIDER_TIMEOUT
    } else if lower.contains("did not call")
        || lower.contains("returned text instead of")
        || lower.contains("returned empty text")
    {
        STRUCTURED_LLM_OUTCOME_MISSING_TOOL
    } else if lower.contains("unsupported tool") || lower.contains("called unsupported tool") {
        STRUCTURED_LLM_OUTCOME_WRONG_TOOL
    } else if lower.contains("stream failed")
        || lower.contains("stream error")
        || lower.contains("provider")
    {
        STRUCTURED_LLM_OUTCOME_PROVIDER_ERROR
    } else {
        STRUCTURED_LLM_OUTCOME_OTHER
    }
}

/// Classify caller-side parse / business validation errors after tool arguments arrive.
pub fn classify_caller_structured_failure_outcome(message: &str) -> Option<&'static str> {
    let lower = message.to_ascii_lowercase();
    if lower.contains("malformed") && lower.contains("json") {
        return Some(STRUCTURED_LLM_OUTCOME_SCHEMA_INVALID);
    }
    if lower.starts_with("extracted fact ")
        || lower.contains("unknown fact key")
        || lower.contains("semantic")
        || lower.contains("evidence id")
        || lower.contains("invalid edit")
        || lower.contains("oldtext")
        || lower.contains("edit did not match")
    {
        return Some(STRUCTURED_LLM_OUTCOME_SEMANTIC_INVALID);
    }
    None
}

pub fn classification_for_caller_failure(
    message: &str,
    attempt_index: i64,
) -> Option<StructuredLlmRequestClassification<'static>> {
    let outcome = classify_caller_structured_failure_outcome(message)?;
    Some(StructuredLlmRequestClassification {
        structured_outcome: outcome,
        recovery_source: STRUCTURED_LLM_RECOVERY_NONE,
        attempt_index,
    })
}

pub fn persist_structured_classification(
    workspace_path: &Path,
    request_id: &str,
    classification: StructuredLlmRequestClassification<'_>,
) -> Result<(), WorkspaceDatabaseError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)?;
    database.set_llm_request_structured_classification(request_id, classification)
}

pub fn persist_structured_classification_on_database(
    database: &mut WorkspaceDatabase,
    request_id: &str,
    classification: StructuredLlmRequestClassification<'_>,
) -> Result<(), WorkspaceDatabaseError> {
    database.set_llm_request_structured_classification(request_id, classification)
}

/// Adjust attempt index while reusing outcome/recovery codes.
pub fn with_attempt_index(
    classification: StructuredLlmRequestClassification<'static>,
    attempt_index: i64,
) -> StructuredLlmRequestClassification<'static> {
    StructuredLlmRequestClassification {
        structured_outcome: classification.structured_outcome,
        recovery_source: classification.recovery_source,
        attempt_index,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_provider_and_tool_protocol_failures() {
        assert_eq!(
            classify_provider_tool_failure_outcome(
                "memory extraction timed out after 60000 ms"
            ),
            STRUCTURED_LLM_OUTCOME_PROVIDER_TIMEOUT
        );
        assert_eq!(
            classify_provider_tool_failure_outcome("memory extraction did not call submit tool"),
            STRUCTURED_LLM_OUTCOME_MISSING_TOOL
        );
        assert_eq!(
            classify_provider_tool_failure_outcome(
                "memory extraction returned text instead of submit tool: hello"
            ),
            STRUCTURED_LLM_OUTCOME_MISSING_TOOL
        );
        assert_eq!(
            classify_provider_tool_failure_outcome(
                "memory extraction completed with unsupported tool 'foo'"
            ),
            STRUCTURED_LLM_OUTCOME_WRONG_TOOL
        );
        assert_eq!(
            classify_provider_tool_failure_outcome("memory extraction stream failed: boom"),
            STRUCTURED_LLM_OUTCOME_PROVIDER_ERROR
        );
    }

    #[test]
    fn classifies_caller_schema_and_semantic_failures() {
        assert_eq!(
            classify_caller_structured_failure_outcome(
                "malformed memory extraction JSON: missing field `facts`"
            ),
            Some(STRUCTURED_LLM_OUTCOME_SCHEMA_INVALID)
        );
        assert_eq!(
            classify_caller_structured_failure_outcome(
                "extracted fact fact-1 has invalid confidence"
            ),
            Some(STRUCTURED_LLM_OUTCOME_SEMANTIC_INVALID)
        );
        assert_eq!(
            classify_caller_structured_failure_outcome(
                "memory retrieval model returned unknown fact key 'x'"
            ),
            Some(STRUCTURED_LLM_OUTCOME_SEMANTIC_INVALID)
        );
        assert_eq!(
            classify_caller_structured_failure_outcome("provider unavailable"),
            None
        );
    }

    #[test]
    fn recovery_classifications_are_stable() {
        let tool = classification_succeeded_tool_call(1);
        assert_eq!(tool.structured_outcome, STRUCTURED_LLM_OUTCOME_SUCCEEDED);
        assert_eq!(tool.recovery_source, STRUCTURED_LLM_RECOVERY_TOOL_CALL);
        assert_eq!(tool.attempt_index, 1);

        let recovered = classification_text_json_recovered(2);
        assert_eq!(
            recovered.structured_outcome,
            STRUCTURED_LLM_OUTCOME_TEXT_JSON_RECOVERED
        );
        assert_eq!(recovered.recovery_source, STRUCTURED_LLM_RECOVERY_TEXT_JSON);
        assert_eq!(recovered.attempt_index, 2);

        let correction = classification_correction_retry_success(3);
        assert_eq!(
            correction.recovery_source,
            STRUCTURED_LLM_RECOVERY_CORRECTION_RETRY
        );
    }
}
