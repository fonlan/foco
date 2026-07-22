//! Stable classification for single-tool structured LLM requests.
//!
//! Codes never include model/prompt body text. They are persisted on `llm_requests`
//! for baseline first-attempt / terminal success metrics and drive output-repair
//! vs provider-retry decisions.

use foco_providers::{NeutralChatRequest, NeutralToolChoice};
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

/// Hard cap on stream attempts for one audited single-tool call.
///
/// Budgets are independent and additive (never multiplied by an outer extraction loop):
/// - initial attempt of the original body
/// - up to `provider_retry_budget` additional transport retries of the original body
/// - at most one output-protocol repair body
/// - up to `provider_retry_budget` additional transport retries of the repaired body
///
/// Formula: `2 * provider_retry_budget + 2` (saturating).
pub fn audited_max_stream_attempts(provider_retry_budget: u32) -> u32 {
    provider_retry_budget
        .saturating_add(1) // initial + provider retries of original
        .saturating_add(1) // one output repair
        .saturating_add(provider_retry_budget) // provider retries of repaired body
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

/// Walk a fault sequence through [`next_audited_stream_action`] and count stream attempts.
///
/// Each entry is one completed stream outcome. Counting stops when the state machine returns
/// [`StructuredLlmNextAction::Stop`] (the failing attempt is still counted). Extra failures after
/// stop are ignored so tests can over-provision sequences without inflating the budget.
pub fn count_stream_attempts_until_stop(
    failures: &[(StructuredLlmFailureKind, Option<i64>)],
    provider_retry_budget: u32,
) -> u32 {
    let mut output_repair_used = false;
    let mut provider_attempts_used: u32 = 0;
    let mut attempts: u32 = 0;

    for &(failure_kind, status_code) in failures {
        attempts = attempts.saturating_add(1);
        match next_audited_stream_action(
            failure_kind,
            status_code,
            output_repair_used,
            provider_attempts_used,
            provider_retry_budget,
        ) {
            StructuredLlmNextAction::Continue {
                provider_retry_index,
                output_repair_used: next_output_repair_used,
                ..
            } => {
                provider_attempts_used = provider_retry_index;
                output_repair_used = next_output_repair_used;
            }
            StructuredLlmNextAction::Stop => break,
        }
    }

    attempts
}

/// Protocol-class outcomes that text-JSON recovery may still turn into success before repair.
///
/// Real ToolCall always wins first; this only documents which raw stream failures feed repair
/// after recovery already failed.
pub fn is_protocol_fault_matrix_kind(kind: StructuredLlmFailureKind) -> bool {
    matches!(
        kind,
        StructuredLlmFailureKind::MissingTool
            | StructuredLlmFailureKind::Prose
            | StructuredLlmFailureKind::WrongTool
            | StructuredLlmFailureKind::SchemaInvalid
    )
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
    /// A Thinking-mode provider rejected native forced tool selection on a required-single-tool request.
    ThinkingToolChoiceIncompatible,
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
            Self::ProviderError | Self::ThinkingToolChoiceIncompatible => {
                STRUCTURED_LLM_OUTCOME_PROVIDER_ERROR
            }
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
            Self::ThinkingToolChoiceIncompatible => "thinking_tool_choice_incompatible",
            Self::Other => "other",
        }
    }

    /// Output-protocol failures eligible for a single feedback repair retry.
    pub fn is_output_repair_eligible(self) -> bool {
        matches!(
            self,
            Self::MissingTool
                | Self::Prose
                | Self::WrongTool
                | Self::SchemaInvalid
                | Self::ThinkingToolChoiceIncompatible
        )
    }

    pub fn classification(self, attempt_index: i64) -> StructuredLlmRequestClassification<'static> {
        StructuredLlmRequestClassification {
            structured_outcome: self.structured_outcome(),
            recovery_source: STRUCTURED_LLM_RECOVERY_NONE,
            attempt_index,
            structured_call_id: None,
        }
    }
}

/// Successful tool-call path (real ToolCall arguments).
pub fn classification_succeeded_tool_call(
    attempt_index: i64,
) -> StructuredLlmRequestClassification<'static> {
    StructuredLlmRequestClassification {
        structured_outcome: STRUCTURED_LLM_OUTCOME_SUCCEEDED,
        recovery_source: STRUCTURED_LLM_RECOVERY_TOOL_CALL,
        attempt_index,
        structured_call_id: None,
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
        structured_call_id: None,
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
        structured_call_id: None,
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

/// Rebuild a single-tool request for its one allowed output-repair attempt.
///
/// Most protocol failures retain native forced tool selection. The one exception is a provider's
/// explicit Thinking-mode rejection of that selection: retry with the same single tool definition
/// but automatic tool choice, and rely on the deterministic correction message to require the tool.
pub fn build_output_repair_request(
    base_request: &NeutralChatRequest,
    expected_tool_name: &str,
    failure_kind: StructuredLlmFailureKind,
) -> NeutralChatRequest {
    let mut repaired = base_request.clone();
    if matches!(
        failure_kind,
        StructuredLlmFailureKind::ThinkingToolChoiceIncompatible
    ) {
        repaired.tool_choice = NeutralToolChoice::Auto;
    }
    repaired.messages.push(crate::neutral_text_message(
        foco_providers::NeutralChatRole::User,
        build_output_repair_user_message(expected_tool_name, failure_kind),
    ));
    repaired
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
pub fn classify_provider_tool_failure_message(
    message: &str,
) -> StructuredLlmRequestClassification<'static> {
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

/// Classify a provider failure for a request that may have native required-single-tool selection.
///
/// The Thinking/tool-choice compatibility path is intentionally narrow: providers must return an
/// HTTP 400 and explicitly mention Thinking, tool choice, and rejection/unsupported semantics.
/// All other errors retain the generic classifier and existing transport retry behavior.
pub fn classify_required_single_tool_provider_failure_kind(
    message: &str,
    status_code: Option<i64>,
    tool_choice: &NeutralToolChoice,
) -> StructuredLlmFailureKind {
    if matches!(status_code, Some(400))
        && tool_choice.required_tool_name().is_some()
        && is_thinking_tool_choice_incompatibility(message)
    {
        StructuredLlmFailureKind::ThinkingToolChoiceIncompatible
    } else {
        classify_provider_tool_failure_kind(message, status_code)
    }
}

fn is_thinking_tool_choice_incompatibility(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase().replace(['_', '-'], " ");
    let mentions_tool_choice = normalized.contains("tool choice");
    let rejects_tool_choice = [
        "unsupported",
        "not supported",
        "does not support",
        "doesn't support",
        "cannot use",
        "can't use",
        "not allowed",
        "rejected",
        "reject",
        "incompatible",
        "only supports auto",
        "only support auto",
        "must be auto",
    ]
    .iter()
    .any(|signal| normalized.contains(signal));

    normalized.contains("thinking") && mentions_tool_choice && rejects_tool_choice
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
        structured_call_id: classification.structured_call_id,
    }
}

/// Attach a durable job call id shared across stream attempts of one audited call.
pub fn with_structured_call_id<'a>(
    classification: StructuredLlmRequestClassification<'static>,
    structured_call_id: &'a str,
) -> StructuredLlmRequestClassification<'a> {
    StructuredLlmRequestClassification {
        structured_outcome: classification.structured_outcome,
        recovery_source: classification.recovery_source,
        attempt_index: classification.attempt_index,
        structured_call_id: Some(structured_call_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_provider_and_tool_protocol_failures() {
        assert_eq!(
            classify_provider_tool_failure_kind("memory extraction timed out after 60000 ms", None),
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
    fn thinking_tool_choice_compatibility_requires_required_tool_and_http_400() {
        let required = NeutralToolChoice::required_single_tool("select_relevant_memory");
        let message = "Thinking mode only supports auto tool_choice.";

        let failure =
            classify_required_single_tool_provider_failure_kind(message, Some(400), &required);
        assert_eq!(
            failure,
            StructuredLlmFailureKind::ThinkingToolChoiceIncompatible
        );
        assert_eq!(
            failure.structured_outcome(),
            STRUCTURED_LLM_OUTCOME_PROVIDER_ERROR
        );
        assert_eq!(
            failure.category_label(),
            "thinking_tool_choice_incompatible"
        );
        assert!(failure.is_output_repair_eligible());
        assert!(!is_provider_transport_retryable(failure, Some(400)));

        assert_ne!(
            classify_required_single_tool_provider_failure_kind(
                message,
                Some(400),
                &NeutralToolChoice::Auto,
            ),
            StructuredLlmFailureKind::ThinkingToolChoiceIncompatible
        );
        assert_ne!(
            classify_required_single_tool_provider_failure_kind(message, Some(401), &required),
            StructuredLlmFailureKind::ThinkingToolChoiceIncompatible
        );
        assert_ne!(
            classify_required_single_tool_provider_failure_kind(
                "tool choice is unsupported",
                Some(400),
                &required,
            ),
            StructuredLlmFailureKind::ThinkingToolChoiceIncompatible
        );
    }

    #[test]
    fn compatibility_repair_downgrades_only_tool_choice_and_keeps_the_single_tool_prompt() {
        let base = NeutralChatRequest {
            model_id: "model".to_string(),
            messages: vec![crate::neutral_text_message(
                foco_providers::NeutralChatRole::User,
                "base request".to_string(),
            )],
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: None,
            tool_choice: NeutralToolChoice::required_single_tool("select_relevant_memory"),
        };

        let compatibility_repair = build_output_repair_request(
            &base,
            "select_relevant_memory",
            StructuredLlmFailureKind::ThinkingToolChoiceIncompatible,
        );
        assert_eq!(compatibility_repair.tool_choice, NeutralToolChoice::Auto);
        assert_eq!(compatibility_repair.tools, base.tools);
        assert_eq!(compatibility_repair.messages.len(), 2);
        assert!(
            compatibility_repair.messages[1]
                .content
                .contains("select_relevant_memory")
        );

        let protocol_repair = build_output_repair_request(
            &base,
            "select_relevant_memory",
            StructuredLlmFailureKind::Prose,
        );
        assert_eq!(protocol_repair.tool_choice, base.tool_choice);
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
        assert_eq!(
            correction.structured_outcome,
            STRUCTURED_LLM_OUTCOME_SUCCEEDED
        );

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
            next_audited_stream_action(StructuredLlmFailureKind::MissingTool, None, false, 0, 2,),
            StructuredLlmNextAction::Continue {
                retry_kind: StructuredLlmRetryKind::OutputRepair,
                provider_retry_index: 0,
                output_repair_used: true,
            }
        );
        // Second protocol failure after repair → stop (no feedback-less re-prompt)
        assert_eq!(
            next_audited_stream_action(StructuredLlmFailureKind::Prose, None, true, 0, 2,),
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

    /// Fault matrix: each protocol-class failure repairs once; semantic/other stop; transport uses budget.
    #[test]
    fn fault_matrix_maps_to_repair_provider_or_stop() {
        let budget = 2u32;

        for kind in [
            StructuredLlmFailureKind::MissingTool,
            StructuredLlmFailureKind::Prose,
            StructuredLlmFailureKind::WrongTool,
            StructuredLlmFailureKind::SchemaInvalid,
        ] {
            assert!(is_protocol_fault_matrix_kind(kind));
            assert_eq!(
                next_audited_stream_action(kind, None, false, 0, budget),
                StructuredLlmNextAction::Continue {
                    retry_kind: StructuredLlmRetryKind::OutputRepair,
                    provider_retry_index: 0,
                    output_repair_used: true,
                },
                "{kind:?} should schedule one output repair"
            );
            // After repair, same protocol failure must not re-prompt without feedback.
            assert_eq!(
                next_audited_stream_action(kind, None, true, 0, budget),
                StructuredLlmNextAction::Stop,
                "{kind:?} after repair must stop"
            );
        }

        // Semantic: never repair, never provider-retry.
        assert!(!is_protocol_fault_matrix_kind(
            StructuredLlmFailureKind::SemanticInvalid
        ));
        assert_eq!(
            next_audited_stream_action(
                StructuredLlmFailureKind::SemanticInvalid,
                Some(429),
                false,
                0,
                budget
            ),
            StructuredLlmNextAction::Stop
        );

        // Timeout / 429 / 5xx / bare network → provider retry path only.
        for (kind, status) in [
            (StructuredLlmFailureKind::ProviderTimeout, None),
            (StructuredLlmFailureKind::ProviderError, Some(429)),
            (StructuredLlmFailureKind::ProviderError, Some(503)),
            (StructuredLlmFailureKind::ProviderError, None),
        ] {
            assert_eq!(
                next_audited_stream_action(kind, status, false, 0, budget),
                StructuredLlmNextAction::Continue {
                    retry_kind: StructuredLlmRetryKind::ProviderRetry,
                    provider_retry_index: 1,
                    output_repair_used: false,
                },
                "{kind:?} status={status:?} should provider-retry"
            );
            assert!(!kind.is_output_repair_eligible());
        }

        // Non-retryable 4xx provider error → stop immediately.
        assert_eq!(
            next_audited_stream_action(
                StructuredLlmFailureKind::ProviderError,
                Some(400),
                false,
                0,
                budget
            ),
            StructuredLlmNextAction::Stop
        );
    }

    #[test]
    fn audited_max_stream_attempts_is_linear_not_multiplied() {
        // Formula: 2 * budget + 2 (initial + repair + provider on both bodies).
        assert_eq!(audited_max_stream_attempts(0), 2);
        assert_eq!(audited_max_stream_attempts(1), 4);
        assert_eq!(audited_max_stream_attempts(2), 6);
        assert_eq!(audited_max_stream_attempts(3), 8);

        // Explicitly not budget * outer_extraction_attempts (legacy bug was outer * (retry+1)).
        let llm_request_retry_count = 3u32;
        let removed_outer_extraction_attempts = 3u32; // historical constant, must not multiply
        let actual_cap = audited_max_stream_attempts(llm_request_retry_count);
        let forbidden_multiplied =
            llm_request_retry_count.saturating_add(1) * removed_outer_extraction_attempts;
        assert!(
            actual_cap < forbidden_multiplied,
            "cap {actual_cap} must not reach outer×provider product {forbidden_multiplied}"
        );
        assert_eq!(actual_cap, 8);
    }

    #[test]
    fn retry_budget_caps_protocol_only_and_mixed_paths() {
        let budget = 2u32;
        let cap = audited_max_stream_attempts(budget);

        // Protocol-only: one repair then stop → 2 attempts, never multiplies.
        let protocol_only = count_stream_attempts_until_stop(
            &[
                (StructuredLlmFailureKind::MissingTool, None),
                (StructuredLlmFailureKind::Prose, None),
                (StructuredLlmFailureKind::WrongTool, None), // ignored after stop
            ],
            budget,
        );
        assert_eq!(protocol_only, 2);
        assert!(protocol_only <= cap);

        // Provider-only on original body: initial + budget retries → budget+1.
        let provider_only = count_stream_attempts_until_stop(
            &[
                (StructuredLlmFailureKind::ProviderError, Some(503)),
                (StructuredLlmFailureKind::ProviderError, Some(503)),
                (StructuredLlmFailureKind::ProviderError, Some(503)),
                (StructuredLlmFailureKind::ProviderError, Some(503)), // stop
            ],
            budget,
        );
        assert_eq!(provider_only, budget + 1);
        assert!(provider_only <= cap);

        // Worst mixed path: exhaust provider on original, repair, exhaust provider on repaired.
        let worst = count_stream_attempts_until_stop(
            &[
                (StructuredLlmFailureKind::ProviderError, Some(503)), // → p1
                (StructuredLlmFailureKind::ProviderError, Some(503)), // → p2
                (StructuredLlmFailureKind::MissingTool, None),        // → repair
                (StructuredLlmFailureKind::ProviderError, Some(503)), // → p1 repaired
                (StructuredLlmFailureKind::ProviderError, Some(503)), // → p2 repaired
                (StructuredLlmFailureKind::ProviderError, Some(503)), // stop
                (StructuredLlmFailureKind::MissingTool, None),        // ignored
            ],
            budget,
        );
        assert_eq!(worst, cap);
        assert_eq!(worst, 6);

        // Schema invalid repairs once like missing_tool (args validation path).
        assert_eq!(
            count_stream_attempts_until_stop(
                &[
                    (StructuredLlmFailureKind::SchemaInvalid, None),
                    (StructuredLlmFailureKind::SchemaInvalid, None),
                ],
                budget
            ),
            2
        );

        // Semantic invalid never burns repair budget.
        assert_eq!(
            count_stream_attempts_until_stop(
                &[(StructuredLlmFailureKind::SemanticInvalid, None)],
                budget
            ),
            1
        );
    }

    #[test]
    fn extraction_call_site_budget_is_single_audited_call() {
        // Memory extraction invokes audited_provider_tool_request once (no outer attempt loop).
        // Total stream attempts for one extraction job is therefore ≤ audited_max_stream_attempts.
        let llm_request_retry_count = 2u32;
        let audited_calls_per_extraction_job = 1u32;
        let total_cap = audited_max_stream_attempts(llm_request_retry_count)
            .saturating_mul(audited_calls_per_extraction_job);
        assert_eq!(total_cap, 6);

        // Even if someone reintroduced a 3-attempt outer loop, the product would be 18 —
        // this documents the intended single-call bound for regression tests.
        let legacy_outer = 3u32;
        assert_ne!(
            total_cap,
            audited_max_stream_attempts(llm_request_retry_count).saturating_mul(legacy_outer)
        );
    }

    #[test]
    fn classifies_fault_matrix_messages_for_protocol_and_semantic() {
        // Empty / missing tool
        assert_eq!(
            classify_provider_tool_failure_kind("memory extraction did not call submit tool", None),
            StructuredLlmFailureKind::MissingTool
        );
        // Prose (non-recoverable text after text-JSON failed)
        assert_eq!(
            classify_provider_tool_failure_kind(
                "memory retrieval returned text instead of select tool: sorry",
                None
            ),
            StructuredLlmFailureKind::Prose
        );
        // Wrong tool
        assert_eq!(
            classify_provider_tool_failure_kind(
                "workspace spec update completed with unsupported tool 'other'",
                None
            ),
            StructuredLlmFailureKind::WrongTool
        );
        // Schema (caller)
        assert_eq!(
            classify_caller_structured_failure_kind(
                "malformed memory extraction JSON: missing field `facts`"
            ),
            Some(StructuredLlmFailureKind::SchemaInvalid)
        );
        // Semantic (caller)
        assert_eq!(
            classify_caller_structured_failure_kind(
                "extracted fact 0 references unknown evidence id 'x'"
            ),
            Some(StructuredLlmFailureKind::SemanticInvalid)
        );
        assert_eq!(
            classify_caller_structured_failure_kind(
                "invalid edit: oldText did not match exactly once"
            ),
            Some(StructuredLlmFailureKind::SemanticInvalid)
        );
    }
}
