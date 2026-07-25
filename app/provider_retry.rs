use std::time::Duration;

use foco_providers::{ProviderConfigError, ProviderStreamFailureKind};

/// Cap for exponential backoff waits between provider attempts.
const PROVIDER_RETRY_BACKOFF_CAP: Duration = Duration::from_secs(30);
/// Base wait for the first retry (1-based retry ordinal 1 → 1s before jitter).
const PROVIDER_RETRY_BACKOFF_BASE_SECS: u64 = 1;

/// Retry class used by local chat, structured LLM, and remote broker paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProviderRetryClass {
    Capacity,
    RateLimit,
    TransientServer,
    Network,
    NonRetryable,
}

impl ProviderRetryClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Capacity => "capacity",
            Self::RateLimit => "rate_limit",
            Self::TransientServer => "transient_server",
            Self::Network => "network",
            Self::NonRetryable => "non_retryable",
        }
    }

    pub(crate) fn is_retryable(self) -> bool {
        !matches!(self, Self::NonRetryable)
    }
}

/// Decision for one failed provider attempt.
///
/// `attempt_count` is the number of **additional** retries already used (0 on the first failure).
/// `max_attempts` is the configured **additional** retry budget (`llm_request_retry_count`), not
/// total attempts. Retry is allowed when `attempt_count < max_attempts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProviderRetryDecision {
    pub class: ProviderRetryClass,
    pub retryable: bool,
}

pub(crate) fn is_retryable_provider_status(status_code: u16) -> bool {
    matches!(status_code, 408 | 409 | 429 | 500..=599)
}

pub(crate) fn classify_provider_retry_class(error: &ProviderConfigError) -> ProviderRetryClass {
    match error {
        ProviderConfigError::ProviderStream(detail) => match detail.kind {
            ProviderStreamFailureKind::Capacity => ProviderRetryClass::Capacity,
            ProviderStreamFailureKind::RateLimit => ProviderRetryClass::RateLimit,
            ProviderStreamFailureKind::ServerError => ProviderRetryClass::TransientServer,
            ProviderStreamFailureKind::Auth
            | ProviderStreamFailureKind::Permission
            | ProviderStreamFailureKind::InvalidRequest
            | ProviderStreamFailureKind::ContextLength
            | ProviderStreamFailureKind::ProtocolParse
            | ProviderStreamFailureKind::Other => ProviderRetryClass::NonRetryable,
        },
        ProviderConfigError::Connection { status_code, .. } => match *status_code {
            None => ProviderRetryClass::Network,
            Some(code) if is_retryable_provider_status(code) => {
                if code == 429 {
                    ProviderRetryClass::RateLimit
                } else if (500..=599).contains(&code) {
                    ProviderRetryClass::TransientServer
                } else {
                    // 408/409: treat as transient network/server class.
                    ProviderRetryClass::Network
                }
            }
            Some(_) => ProviderRetryClass::NonRetryable,
        },
        ProviderConfigError::EmptyBaseUrl
        | ProviderConfigError::EmptyProxyUrl
        | ProviderConfigError::InvalidBaseUrl { .. }
        | ProviderConfigError::InvalidProxyUrl { .. }
        | ProviderConfigError::InvalidRequest(_)
        | ProviderConfigError::MissingRequiredField(_)
        | ProviderConfigError::MissingApiKey
        | ProviderConfigError::UnsupportedKind(_)
        | ProviderConfigError::UnsupportedProxyKind(_)
        | ProviderConfigError::UnsupportedProxyForWebSocket { .. } => {
            ProviderRetryClass::NonRetryable
        }
    }
}

pub(crate) fn is_retryable_provider_stream_error(error: &ProviderConfigError) -> bool {
    classify_provider_retry_class(error).is_retryable()
}

pub(crate) fn should_retry_provider_stream_error(
    error: &ProviderConfigError,
    attempt_count: u32,
    max_attempts: u32,
) -> bool {
    attempt_count < max_attempts && is_retryable_provider_stream_error(error)
}

pub(crate) fn provider_retry_decision(
    error: &ProviderConfigError,
    attempt_count: u32,
    max_attempts: u32,
) -> ProviderRetryDecision {
    let class = classify_provider_retry_class(error);
    ProviderRetryDecision {
        class,
        retryable: attempt_count < max_attempts && class.is_retryable(),
    }
}

pub(crate) fn should_retry_remote_broker_failure(
    code: &str,
    retryable: bool,
    attempt_count: u32,
    max_attempts: u32,
) -> bool {
    attempt_count < max_attempts
        && retryable
        && matches!(
            code,
            "provider_error"
                | "provider_stream_error"
                | "stream_error"
                | "stream_interrupted"
                | "stream_incomplete"
                | "empty_completion"
        )
}

/// Classify a remote broker failure using structured fields when present.
///
/// Prefer `failure_kind` / code / type / message over the bare `retryable` flag so local and
/// remote paths share the same decision table. The broker `retryable` flag remains a gate for
/// unknown legacy payloads.
pub(crate) fn classify_remote_broker_retry_class(
    code: &str,
    retryable_flag: bool,
    status_code: Option<i64>,
    failure_kind: Option<&str>,
    provider_code: Option<&str>,
    provider_type: Option<&str>,
    message: &str,
) -> ProviderRetryClass {
    if let Some(kind) = failure_kind {
        return match kind {
            "capacity" => ProviderRetryClass::Capacity,
            "rate_limit" => ProviderRetryClass::RateLimit,
            "server_error" | "transient_server" => ProviderRetryClass::TransientServer,
            "network" => ProviderRetryClass::Network,
            "auth"
            | "permission"
            | "invalid_request"
            | "context_length"
            | "protocol_parse"
            | "other"
            | "non_retryable" => ProviderRetryClass::NonRetryable,
            _ => ProviderRetryClass::NonRetryable,
        };
    }

    let status = status_code.and_then(|code| u16::try_from(code).ok());
    let structured = foco_providers::classify_provider_stream_failure_kind(
        provider_code,
        provider_type,
        message,
        status,
    );
    match structured {
        ProviderStreamFailureKind::Capacity => ProviderRetryClass::Capacity,
        ProviderStreamFailureKind::RateLimit => ProviderRetryClass::RateLimit,
        ProviderStreamFailureKind::ServerError => ProviderRetryClass::TransientServer,
        ProviderStreamFailureKind::Auth
        | ProviderStreamFailureKind::Permission
        | ProviderStreamFailureKind::InvalidRequest
        | ProviderStreamFailureKind::ContextLength
        | ProviderStreamFailureKind::ProtocolParse => ProviderRetryClass::NonRetryable,
        ProviderStreamFailureKind::Other => {
            if !retryable_flag {
                return ProviderRetryClass::NonRetryable;
            }
            if !matches!(
                code,
                "provider_error"
                    | "provider_stream_error"
                    | "stream_error"
                    | "stream_interrupted"
                    | "stream_incomplete"
                    | "empty_completion"
            ) {
                return ProviderRetryClass::NonRetryable;
            }
            match status {
                None => ProviderRetryClass::Network,
                Some(code) if is_retryable_provider_status(code) => {
                    if code == 429 {
                        ProviderRetryClass::RateLimit
                    } else if (500..=599).contains(&code) {
                        ProviderRetryClass::TransientServer
                    } else {
                        ProviderRetryClass::Network
                    }
                }
                Some(_) => ProviderRetryClass::NonRetryable,
            }
        }
    }
}

/// Deterministic exponential backoff base (no jitter) for a retry class.
///
/// `retry_ordinal` is 1-based: first retry waits 1s, then 2s, 4s, 8s, 16s, then caps at 30s.
/// Non-retryable classes and ordinal 0 return `None`.
pub(crate) fn provider_retry_backoff_base(
    class: ProviderRetryClass,
    retry_ordinal: u32,
) -> Option<Duration> {
    if !class.is_retryable() || retry_ordinal == 0 {
        return None;
    }
    let shift = retry_ordinal.saturating_sub(1).min(63);
    let secs = PROVIDER_RETRY_BACKOFF_BASE_SECS
        .checked_shl(shift)
        .unwrap_or(u64::MAX)
        .min(PROVIDER_RETRY_BACKOFF_CAP.as_secs());
    Some(Duration::from_secs(secs))
}

/// Apply full jitter in `[0, base]` using a unit sample in `[0.0, 1.0]`.
pub(crate) fn apply_full_jitter(base: Duration, unit_sample: f64) -> Duration {
    if base.is_zero() {
        return Duration::ZERO;
    }
    let sample = unit_sample.clamp(0.0, 1.0);
    let nanos = (base.as_secs_f64() * sample * 1_000_000_000.0).round() as u128;
    Duration::from_nanos(nanos.min(u128::from(u64::MAX)) as u64)
}

/// Compute the wait before the next provider attempt.
///
/// Priority:
/// 1. `retry_after` when present (clamped to the backoff cap)
/// 2. exponential base for the retry class
/// then full jitter, then clamp to `remaining_deadline` when provided.
pub(crate) fn provider_retry_backoff(
    class: ProviderRetryClass,
    retry_ordinal: u32,
    retry_after: Option<Duration>,
    remaining_deadline: Option<Duration>,
    unit_sample: f64,
) -> Option<Duration> {
    if !class.is_retryable() || retry_ordinal == 0 {
        return None;
    }
    let base = retry_after
        .map(|value| value.min(PROVIDER_RETRY_BACKOFF_CAP))
        .or_else(|| provider_retry_backoff_base(class, retry_ordinal))?;
    let mut wait = apply_full_jitter(base, unit_sample);
    if let Some(remaining) = remaining_deadline {
        if remaining.is_zero() {
            return None;
        }
        wait = wait.min(remaining);
    }
    Some(wait)
}

/// Random unit sample for production jitter.
pub(crate) fn random_unit_sample() -> f64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0)
        .hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    let bits = hasher.finish();
    // Map to (0, 1] so the first retry is never a pure zero-wait when base > 0.
    ((bits % 10_000) as f64 + 1.0) / 10_001.0
}

/// Backward-compatible 429 staircase helper used by older call sites.
///
/// Prefer [`provider_retry_backoff`] for new code. Non-429 statuses return `None` so existing
/// callers that only waited on 429 keep their previous behavior until migrated.
pub(crate) fn provider_429_retry_backoff(
    status_code: Option<u16>,
    retry_ordinal: u32,
) -> Option<Duration> {
    if status_code != Some(429) {
        return None;
    }
    provider_retry_backoff_base(ProviderRetryClass::RateLimit, retry_ordinal)
}

/// Convenience for `i64` audit/status fields used by internal LLM paths.
pub(crate) fn provider_429_retry_backoff_i64(
    status_code: Option<i64>,
    retry_ordinal: u32,
) -> Option<Duration> {
    let status = status_code.and_then(|code| u16::try_from(code).ok());
    provider_429_retry_backoff(status, retry_ordinal)
}

/// Backoff for a classified retry using production jitter.
pub(crate) fn provider_retry_backoff_for_class(
    class: ProviderRetryClass,
    retry_ordinal: u32,
    remaining_deadline: Option<Duration>,
) -> Option<Duration> {
    provider_retry_backoff(
        class,
        retry_ordinal,
        None,
        remaining_deadline,
        random_unit_sample(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use foco_providers::{ProviderStreamErrorDetail, ProviderStreamFailureKind};
    use std::time::Duration;

    fn stream_error(kind: ProviderStreamFailureKind, message: &str) -> ProviderConfigError {
        ProviderConfigError::ProviderStream(Box::new(ProviderStreamErrorDetail {
            message: message.to_string(),
            status_code: None,
            kind,
            code: None,
            error_type: None,
            param: None,
            event_type: Some("error".to_string()),
            diagnostic_kind: Some("provider_error_event".to_string()),
            model_id: Some("gpt-test".to_string()),
            adapter: Some("OpenAI Responses".to_string()),
        }))
    }

    #[test]
    fn remote_broker_retry_classifier_accepts_known_transient_codes_before_limit() {
        assert!(should_retry_remote_broker_failure(
            "provider_stream_error",
            true,
            0,
            1,
        ));
    }

    #[test]
    fn remote_broker_retry_classifier_rejects_unknown_codes_and_exhausted_attempts() {
        assert!(!should_retry_remote_broker_failure(
            "unknown_error",
            true,
            0,
            1,
        ));
        assert!(!should_retry_remote_broker_failure(
            "stream_error",
            true,
            1,
            1,
        ));
    }

    #[test]
    fn structured_stream_errors_classify_retryability() {
        assert!(is_retryable_provider_stream_error(&stream_error(
            ProviderStreamFailureKind::Capacity,
            "model capacity exceeded",
        )));
        assert!(is_retryable_provider_stream_error(&stream_error(
            ProviderStreamFailureKind::RateLimit,
            "rate limited",
        )));
        assert!(is_retryable_provider_stream_error(&stream_error(
            ProviderStreamFailureKind::ServerError,
            "server_error",
        )));
        assert!(!is_retryable_provider_stream_error(&stream_error(
            ProviderStreamFailureKind::Auth,
            "invalid api key",
        )));
        assert!(!is_retryable_provider_stream_error(&stream_error(
            ProviderStreamFailureKind::InvalidRequest,
            "bad request",
        )));
        assert!(!is_retryable_provider_stream_error(&stream_error(
            ProviderStreamFailureKind::ProtocolParse,
            "Failed to parse stream data",
        )));
        assert!(!is_retryable_provider_stream_error(&stream_error(
            ProviderStreamFailureKind::ContextLength,
            "context length exceeded",
        )));
    }

    #[test]
    fn connection_status_none_remains_network_retryable() {
        let network = ProviderConfigError::Connection {
            message: "connection reset".to_string(),
            status_code: None,
        };
        assert_eq!(
            classify_provider_retry_class(&network),
            ProviderRetryClass::Network
        );
        assert!(should_retry_provider_stream_error(&network, 0, 3));
        assert!(!should_retry_provider_stream_error(&network, 3, 3));
    }

    #[test]
    fn provider_retry_backoff_follows_staircase_and_respects_deadline() {
        assert_eq!(
            provider_retry_backoff_base(ProviderRetryClass::RateLimit, 1),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            provider_retry_backoff_base(ProviderRetryClass::Capacity, 2),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            provider_retry_backoff_base(ProviderRetryClass::TransientServer, 6),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            provider_retry_backoff(
                ProviderRetryClass::RateLimit,
                1,
                None,
                Some(Duration::from_millis(250)),
                1.0,
            ),
            Some(Duration::from_millis(250))
        );
        assert_eq!(
            provider_retry_backoff(
                ProviderRetryClass::RateLimit,
                1,
                None,
                Some(Duration::ZERO),
                1.0,
            ),
            None
        );
        assert_eq!(
            provider_retry_backoff(
                ProviderRetryClass::NonRetryable,
                1,
                None,
                None,
                1.0,
            ),
            None
        );
    }

    #[test]
    fn provider_retry_backoff_prefers_retry_after() {
        assert_eq!(
            provider_retry_backoff(
                ProviderRetryClass::RateLimit,
                1,
                Some(Duration::from_secs(7)),
                None,
                1.0,
            ),
            Some(Duration::from_secs(7))
        );
        assert_eq!(
            provider_retry_backoff(
                ProviderRetryClass::RateLimit,
                1,
                Some(Duration::from_secs(120)),
                None,
                1.0,
            ),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn provider_429_backoff_skips_non_429_and_invalid_ordinals() {
        assert_eq!(provider_429_retry_backoff(None, 1), None);
        assert_eq!(provider_429_retry_backoff(Some(408), 1), None);
        assert_eq!(provider_429_retry_backoff(Some(500), 1), None);
        assert_eq!(provider_429_retry_backoff(Some(429), 0), None);
        assert_eq!(
            provider_429_retry_backoff_i64(Some(429), 1),
            Some(Duration::from_secs(1))
        );
        assert_eq!(provider_429_retry_backoff_i64(Some(-1), 1), None);
        assert_eq!(provider_429_retry_backoff_i64(None, 1), None);
    }

    #[test]
    fn remote_structured_failure_kind_drives_class() {
        assert_eq!(
            classify_remote_broker_retry_class(
                "stream_error",
                true,
                None,
                Some("capacity"),
                Some("model_capacity"),
                None,
                "no capacity",
            ),
            ProviderRetryClass::Capacity
        );
        assert_eq!(
            classify_remote_broker_retry_class(
                "stream_error",
                true,
                None,
                Some("protocol_parse"),
                None,
                None,
                "Failed to parse stream data",
            ),
            ProviderRetryClass::NonRetryable
        );
    }
}
