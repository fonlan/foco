//! Stable classification for single-tool structured LLM requests.
//!
//! Codes never include model/prompt body text. They are persisted on `llm_requests`
//! for baseline first-attempt / terminal success metrics and drive output-repair
//! vs provider-retry decisions.

use foco_store::workspace::{
    STRUCTURED_LLM_OUTCOME_MISSING_TOOL, STRUCTURED_LLM_OUTCOME_OTHER,
    STRUCTURED_LLM_OUTCOME_PROVIDER_ERROR, STRUCTURED_LLM_OUTCOME_PROVIDER_TIMEOUT,
    STRUCTURED_LLM_OUTCOME_SCHEMA_INVALID, STRUCTURED_LLM_OUTCOME_SEMANTIC_INVALID,
    STRUCTURED_LLM_OUTCOME_SUCCEEDED, STRUCTURED_LLM_OUTCOME_TEXT_JSON_RECOVERED,
    STRUCTURED_LLM_OUTCOME_WRONG_TOOL, STRUCTURED_LLM_RECOVERY_CORRECTION_RETRY,
    STRUCTURED_LLM_RECOVERY_NONE, STRUCTURED_LLM_RECOVERY_TEXT_JSON,
    STRUCTURED_LLM_RECOVERY_TOOL_CALL, StructuredLlmRequestClassification, WorkspaceDatabase,
    WorkspaceDatabaseError,
};
use std::path::Path;

/// Why this audited single-tool stream attempt was scheduled.
///
/// Distinct from `providerRetryBudget` (configured max). Used in llm_request start events so
/// provider transport retries are never mislabeled as output-protocol repair attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredLlmRetryKind {
    /// First stream attempt for the original request body.
    Initial,
    /// Same request body after a transport-class failure (timeout/429/5xx).
    ProviderRetry,
    /// Request body after the single allowed output-protocol repair message was appended.
    OutputRepair,
}

impl StructuredLlmRetryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::ProviderRetry => "provider_retry",
            Self::OutputRepair => "output_repair",
        }
    }
}

/// Outcome of scheduling the next audited stream attempt (or stop).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredLlmNextAction {
    /// Run another stream attempt with this kind; reset provider retry index for new body when repair.
    Continue {
        retry_kind: StructuredLlmRetryKind,
        /// Provider-retry ordinal for the next attempt body (`0` = first try of that body).
        provider_retry_index: u32,
        /// Whether output repair has been (or is about to be) scheduled for this call.
        output_repair_used: bool,
    },
    Stop,
}

/// Decide the next attempt after a failed audited stream, without reusing the same prompt for output errors.
///
/// Independent budgets:
/// - output repair: at most once (`output_repair_used` becomes true)
/// - provider transport: up to `provider_retry_budget` additional tries on the current body
pub fn next_audited_stream_action(
    failure_kind: StructuredLlmFailureKind,
    status_code: Option<i64>,
    output_repair_used: bool,
    provider_attempts_used: u32,
    provider_retry_budget: u32,
) -> StructuredLlmNextAction {
    if failure_kind.is_output_repair_eligible() && !output_repair_used {
        return StructuredLlmNextAction::Continue {
            retry_kind: StructuredLlmRetryKind::OutputRepair,
            provider_retry_index: 0,
            output_repair_used: true,
        };
    }
    if is_provider_transport_retryable(failure_kind, status_code)
        && provider_attempts_used < provider_retry_budget
    {
        let next_index = provider_attempts_used.saturating_add(1);
        return StructuredLlmNextAction::Continue {
            retry_kind: StructuredLlmRetryKind::ProviderRetry,
            provider_retry_index: next_index,
            output_repair_used,
        };
    }
    StructuredLlmNextAction::Stop
}

/// Structured failure kind for single-tool audited provider requests.
///
/// Distinguished from raw error strings so provider retry and output repair stay independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredLlmFailureKind {
    /// No tool call and empty assistant text.
    MissingTool,
    /// No tool call; model returned non-recoverable prose (text JSON recovery already failed).
    Prose,
    /// Model called a different tool than the required single tool.
    WrongTool,
    /// Tool arguments failed serde/schema validation (caller or post-stream).
    SchemaInvalid,
    /// Business/semantic validation failed after schema parse.
    SemanticInvalid,
    /// Provider/stream timed out.
    ProviderTimeout,
    /// Provider/stream transport or HTTP error (may be retryable by status).
    ProviderError,
    /// Unclassified failure.
    Other,
}

impl StructuredLlmFailureKind {
    /// Stable `structured_outcome` code for audit columns.
    ///
    /// `Prose` maps to `missing_tool` (Phase 1 baseline: "未调用工具 / 只回 prose").
    pub fn structured_outcome(self) -> &'static str {
        match self {
            Self::MissingTool | Self::Prose => STRUCTURED_LLM_OUTCOME_MISSING_TOOL,
            Self::WrongTool => STRUCTURED_LLM_OUTCOME_WRONG_TOOL,
            Self::SchemaInvalid => STRUCTURED_LLM_OUTCOME_SCHEMA_INVALID,
            Self::SemanticInvalid => STRUCTURED_LLM_OUTCOME_SEMANTIC_INVALID,
            Self::ProviderTimeout => STRUCTURED_LLM_OUTCOME_PROVIDER_TIMEOUT,
            Self::ProviderError => STRUCTURED_LLM_OUTCOME_PROVIDER_ERROR,
            Self::Other => STRUCTURED_LLM_OUTCOME_OTHER,
        }
    }

    /// Short stable label for repair instructions (never model body text).
    pub fn category_label(self) -> &'static str {
        match self {
            Self::MissingTool => "missing_tool",
            Self::Prose => "prose",
            Self::WrongTool => "wrong_tool",
            Self::SchemaInvalid => "schema_invalid",
            Self::SemanticInvalid => "semantic_invalid",
            Self::ProviderTimeout => "provider_timeout",
            Self::ProviderError => "provider_error",
            Self::Other => "other",
        }
    }

    /// Output-protocol failures eligible for a single feedback repair retry.
    pub fn is_output_repair_eligible(self) -> bool {
        matches!(
            self,
            Self::MissingTool | Self::Prose | Self::WrongTool | Self::SchemaInvalid
        )
    }

    pub fn classification(
        self,
        attempt_index: i64,
    ) -> StructuredLlmRequestClassification<'static> {
        StructuredLlmRequestClassification {
            structured_outcome: self.structured_outcome(),
            recovery_source: STRUCTURED_LLM_RECOVERY_NONE,
            attempt_index,
        }
    }
}

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

/// Successful path after a correction (output-protocol) repair retry.
pub fn classification_correction_retry_success(
    attempt_index: i64,
    text_json_recovered: bool,
) -> StructuredLlmRequestClassification<'static> {
    StructuredLlmRequestClassification {
        structured_outcome: if text_json_recovered {
            STRUCTURED_LLM_OUTCOME_TEXT_JSON_RECOVERED
        } else {
            STRUCTURED_LLM_OUTCOME_SUCCEEDED
        },
        recovery_source: STRUCTURED_LLM_RECOVERY_CORRECTION_RETRY,
        attempt_index,
    }
}

/// Build a short, deterministic user message for one output-protocol repair attempt.
///
/// Includes expected tool name and failure category only—never previous model body text.
pub fn build_output_repair_user_message(
    expected_tool_name: &str,
    failure_kind: StructuredLlmFailureKind,
) -> String {
    format!(
        "Previous response failed the structured tool protocol.\n\
Failure category: {category}\n\
Required action: call the tool exactly named `{tool}` with valid JSON arguments matching its schema. \
Do not reply with prose only. Do not call any other tool.",
        category = failure_kind.category_label(),
        tool = expected_tool_name,
    )
}

/// Whether an HTTP status is eligible for existing provider transport retry (408/409/429/5xx).
pub fn is_provider_retryable_status(status_code: Option<i64>) -> bool {
    matches!(status_code, Some(408 | 409 | 429) | Some(500..=599))
}

/// Provider/stream failures that should use the existing provider retry budget (not repair).
pub fn is_provider_transport_retryable(
    kind: StructuredLlmFailureKind,
    status_code: Option<i64>,
) -> bool {
    match kind {
        StructuredLlmFailureKind::ProviderTimeout => true,
        StructuredLlmFailureKind::ProviderError => {
            // No status → treat as network-class (retryable), matching ProviderConfigError::Connection.
            status_code.is_none() || is_provider_retryable_status(status_code)
        }
        _ => false,
    }
}

/// Classify a protocol-layer tool stream failure from structured inputs.
#[allow(dead_code)] // helper for tests and future stream-path call sites
pub fn classify_tool_stream_failure(
    kind: StructuredLlmFailureKind,
) -> StructuredLlmRequestClassification<'static> {
    kind.classification(1)
}

/// Classify a provider/stream/tool-protocol failure message into a stable code.
///
/// Prefer structured kinds when available; this string path remains for legacy callers.
#[allow(dead_code)] // string path kept for external/legacy call sites
pub fn classify_provider_tool_failure_message(message: &str) -> StructuredLlmRequestClassification<'static> {
    let attempt_index = 1;
    let kind = classify_provider_tool_failure_kind(message, None);
    kind.classification(attempt_index)
}

pub fn classify_provider_tool_failure_outcome(message: &str) -> &'static str {
    classify_provider_tool_failure_kind(message, None).structured_outcome()
}

pub fn classify_provider_tool_failure_kind(
    message: &str,
    status_code: Option<i64>,
) -> StructuredLlmFailureKind {
    let lower = message.to_ascii_lowercase();
    if lower.contains("timed out after") {
        StructuredLlmFailureKind::ProviderTimeout
    } else if lower.contains("completed with unsupported tool")
        || lower.contains("called unsupported tool")
        || lower.contains("unsupported tool")
    {
        StructuredLlmFailureKind::WrongTool
    } else if lower.contains("returned text instead of") {
        StructuredLlmFailureKind::Prose
    } else if lower.contains("did not call") || lower.contains("returned empty text") {
        StructuredLlmFailureKind::MissingTool
    } else if lower.contains("malformed") && lower.contains("json") {
        StructuredLlmFailureKind::SchemaInvalid
    } else if lower.contains("stream failed")
        || lower.contains("stream error")
        || lower.contains("provider")
        || is_provider_retryable_status(status_code)
    {
        StructuredLlmFailureKind::ProviderError
    } else {
        StructuredLlmFailureKind::Other
    }
}

/// Classify caller-side parse / business validation errors after tool arguments arrive.
pub fn classify_caller_structured_failure_outcome(message: &str) -> Option<&'static str> {
    classify_caller_structured_failure_kind(message).map(|kind| kind.structured_outcome())
}

pub fn classify_caller_structured_failure_kind(message: &str) -> Option<StructuredLlmFailureKind> {
    let lower = message.to_ascii_lowercase();
    if lower.contains("malformed") && lower.contains("json") {
        return Some(StructuredLlmFailureKind::SchemaInvalid);
    }
    if lower.starts_with("extracted fact ")
        || lower.contains("unknown fact key")
        || lower.contains("semantic")
        || lower.contains("evidence id")
        || lower.contains("invalid edit")
        || lower.contains("oldtext")
        || lower.contains("edit did not match")
    {
        return Some(StructuredLlmFailureKind::SemanticInvalid);
    }
    None
}

pub fn classification_for_caller_failure(
    message: &str,
    attempt_index: i64,
) -> Option<StructuredLlmRequestClassification<'static>> {
    let kind = classify_caller_structured_failure_kind(message)?;
    Some(kind.classification(attempt_index))
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
#[allow(dead_code)] // helper for re-tagging classifications after multi-attempt paths
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
            classify_provider_tool_failure_kind(
                "memory extraction timed out after 60000 ms",
                None
            ),
            StructuredLlmFailureKind::ProviderTimeout
        );
        assert_eq!(
            classify_provider_tool_failure_kind("memory extraction did not call submit tool", None),
            StructuredLlmFailureKind::MissingTool
        );
        assert_eq!(
            classify_provider_tool_failure_kind(
                "memory extraction returned text instead of submit tool: hello",
                None
            ),
            StructuredLlmFailureKind::Prose
        );
        assert_eq!(
            classify_provider_tool_failure_kind(
                "memory extraction completed with unsupported tool 'foo'",
                None
            ),
            StructuredLlmFailureKind::WrongTool
        );
        assert_eq!(
            classify_provider_tool_failure_kind("memory extraction stream failed: boom", None),
            StructuredLlmFailureKind::ProviderError
        );
        assert_eq!(
            classify_provider_tool_failure_outcome(
                "memory extraction returned text instead of submit tool: hello"
            ),
            STRUCTURED_LLM_OUTCOME_MISSING_TOOL
        );
    }

    #[test]
    fn classifies_caller_schema_and_semantic_failures() {
        assert_eq!(
            classify_caller_structured_failure_kind(
                "malformed memory extraction JSON: missing field `facts`"
            ),
            Some(StructuredLlmFailureKind::SchemaInvalid)
        );
        assert_eq!(
            classify_caller_structured_failure_kind("extracted fact fact-1 has invalid confidence"),
            Some(StructuredLlmFailureKind::SemanticInvalid)
        );
        assert_eq!(
            classify_caller_structured_failure_kind(
                "memory retrieval model returned unknown fact key 'x'"
            ),
            Some(StructuredLlmFailureKind::SemanticInvalid)
        );
        assert_eq!(
            classify_caller_structured_failure_outcome("provider unavailable"),
            None
        );
    }

    #[test]
    fn output_repair_eligibility_and_provider_retry() {
        assert!(StructuredLlmFailureKind::MissingTool.is_output_repair_eligible());
        assert!(StructuredLlmFailureKind::Prose.is_output_repair_eligible());
        assert!(StructuredLlmFailureKind::WrongTool.is_output_repair_eligible());
        assert!(StructuredLlmFailureKind::SchemaInvalid.is_output_repair_eligible());
        assert!(!StructuredLlmFailureKind::SemanticInvalid.is_output_repair_eligible());
        assert!(!StructuredLlmFailureKind::ProviderTimeout.is_output_repair_eligible());
        assert!(!StructuredLlmFailureKind::ProviderError.is_output_repair_eligible());

        assert!(is_provider_transport_retryable(
            StructuredLlmFailureKind::ProviderTimeout,
            None
        ));
        assert!(is_provider_transport_retryable(
            StructuredLlmFailureKind::ProviderError,
            Some(429)
        ));
        assert!(is_provider_transport_retryable(
            StructuredLlmFailureKind::ProviderError,
            Some(503)
        ));
        assert!(is_provider_transport_retryable(
            StructuredLlmFailureKind::ProviderError,
            None
        ));
        assert!(!is_provider_transport_retryable(
            StructuredLlmFailureKind::ProviderError,
            Some(400)
        ));
        assert!(!is_provider_transport_retryable(
            StructuredLlmFailureKind::MissingTool,
            None
        ));
    }

    #[test]
    fn repair_message_is_short_and_deterministic() {
        let message = build_output_repair_user_message(
            "select_relevant_memory",
            StructuredLlmFailureKind::Prose,
        );
        assert!(message.contains("Failure category: prose"));
        assert!(message.contains("`select_relevant_memory`"));
        assert!(!message.contains("hello world"));
        assert_eq!(
            build_output_repair_user_message(
                "select_relevant_memory",
                StructuredLlmFailureKind::Prose,
            ),
            message
        );
    }

    #[test]
    fn output_protocol_errors_do_not_use_provider_retry() {
        for kind in [
            StructuredLlmFailureKind::MissingTool,
            StructuredLlmFailureKind::Prose,
            StructuredLlmFailureKind::WrongTool,
            StructuredLlmFailureKind::SchemaInvalid,
        ] {
            assert!(kind.is_output_repair_eligible());
            assert!(!is_provider_transport_retryable(kind, None));
            assert!(!is_provider_transport_retryable(kind, Some(429)));
        }
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

        let correction = classification_correction_retry_success(3, false);
        assert_eq!(
            correction.recovery_source,
            STRUCTURED_LLM_RECOVERY_CORRECTION_RETRY
        );
        assert_eq!(correction.structured_outcome, STRUCTURED_LLM_OUTCOME_SUCCEEDED);

        let correction_text = classification_correction_retry_success(3, true);
        assert_eq!(
            correction_text.structured_outcome,
            STRUCTURED_LLM_OUTCOME_TEXT_JSON_RECOVERED
        );
        assert_eq!(
            correction_text.recovery_source,
            STRUCTURED_LLM_RECOVERY_CORRECTION_RETRY
        );
    }

    #[test]
    fn retry_kind_labels_are_stable_and_distinct() {
        assert_eq!(StructuredLlmRetryKind::Initial.as_str(), "initial");
        assert_eq!(
            StructuredLlmRetryKind::ProviderRetry.as_str(),
            "provider_retry"
        );
        assert_eq!(
            StructuredLlmRetryKind::OutputRepair.as_str(),
            "output_repair"
        );
        // Provider retry after repair must keep a different label than the repair itself.
        assert_ne!(
            StructuredLlmRetryKind::ProviderRetry.as_str(),
            StructuredLlmRetryKind::OutputRepair.as_str()
        );
    }

    #[test]
    fn next_action_separates_output_repair_from_provider_retry() {
        // missing_tool → one output repair (not provider retry)
        assert_eq!(
            next_audited_stream_action(
                StructuredLlmFailureKind::MissingTool,
                None,
                false,
                0,
                2,
            ),
            StructuredLlmNextAction::Continue {
                retry_kind: StructuredLlmRetryKind::OutputRepair,
                provider_retry_index: 0,
                output_repair_used: true,
            }
        );
        // Second protocol failure after repair → stop (no feedback-less re-prompt)
        assert_eq!(
            next_audited_stream_action(
                StructuredLlmFailureKind::Prose,
                None,
                true,
                0,
                2,
            ),
            StructuredLlmNextAction::Stop
        );
        // 503 after repair → provider_retry with distinct kind
        assert_eq!(
            next_audited_stream_action(
                StructuredLlmFailureKind::ProviderError,
                Some(503),
                true,
                0,
                2,
            ),
            StructuredLlmNextAction::Continue {
                retry_kind: StructuredLlmRetryKind::ProviderRetry,
                provider_retry_index: 1,
                output_repair_used: true,
            }
        );
        // Provider budget exhausted → stop
        assert_eq!(
            next_audited_stream_action(
                StructuredLlmFailureKind::ProviderError,
                Some(429),
                true,
                2,
                2,
            ),
            StructuredLlmNextAction::Stop
        );
        // Semantic invalid never repairs
        assert_eq!(
            next_audited_stream_action(
                StructuredLlmFailureKind::SemanticInvalid,
                None,
                false,
                0,
                2,
            ),
            StructuredLlmNextAction::Stop
        );
    }
}
