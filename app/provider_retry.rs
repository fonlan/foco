use std::time::Duration;

use foco_providers::ProviderConfigError;

/// Cap for 429 exponential backoff waits.
const PROVIDER_429_BACKOFF_CAP: Duration = Duration::from_secs(30);
/// Base wait for the first 429 retry (1-based retry ordinal 1 → 1s).
const PROVIDER_429_BACKOFF_BASE_SECS: u64 = 1;

pub(crate) fn is_retryable_provider_status(status_code: u16) -> bool {
    matches!(status_code, 408 | 409 | 429 | 500..=599)
}

pub(crate) fn is_retryable_provider_stream_error(error: &ProviderConfigError) -> bool {
    match error {
        ProviderConfigError::Connection { status_code, .. } => {
            status_code.is_none_or(is_retryable_provider_status)
        }
        ProviderConfigError::EmptyBaseUrl
        | ProviderConfigError::EmptyProxyUrl
        | ProviderConfigError::InvalidBaseUrl { .. }
        | ProviderConfigError::InvalidProxyUrl { .. }
        | ProviderConfigError::InvalidRequest(_)
        | ProviderConfigError::MissingRequiredField(_)
        | ProviderConfigError::MissingApiKey
        | ProviderConfigError::UnsupportedKind(_)
        | ProviderConfigError::UnsupportedProxyKind(_)
        | ProviderConfigError::UnsupportedProxyForWebSocket { .. } => false,
    }
}

pub(crate) fn should_retry_provider_stream_error(
    error: &ProviderConfigError,
    attempt_count: u32,
    max_attempts: u32,
) -> bool {
    attempt_count < max_attempts && is_retryable_provider_stream_error(error)
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

/// Deterministic 429 staircase backoff for the next provider attempt.
///
/// `retry_ordinal` is 1-based: the first 429 retry waits 1s, then 2s, 4s, 8s, 16s,
/// then caps at 30s. Non-429 statuses and missing status codes return `None`.
/// Callers own the actual async wait so chat paths can stay cancel/shutdown aware.
pub(crate) fn provider_429_retry_backoff(
    status_code: Option<u16>,
    retry_ordinal: u32,
) -> Option<Duration> {
    if status_code != Some(429) || retry_ordinal == 0 {
        return None;
    }

    // 1-based ordinal → exponent 0,1,2,... with saturating shift so huge ordinals
    // never panic and still clamp to the 30s cap.
    let shift = retry_ordinal.saturating_sub(1).min(63);
    let secs = PROVIDER_429_BACKOFF_BASE_SECS
        .checked_shl(shift)
        .unwrap_or(u64::MAX)
        .min(PROVIDER_429_BACKOFF_CAP.as_secs());
    Some(Duration::from_secs(secs))
}

/// Convenience for `i64` audit/status fields used by internal LLM paths.
pub(crate) fn provider_429_retry_backoff_i64(
    status_code: Option<i64>,
    retry_ordinal: u32,
) -> Option<Duration> {
    let status = status_code.and_then(|code| u16::try_from(code).ok());
    provider_429_retry_backoff(status, retry_ordinal)
}

#[cfg(test)]
mod tests {
    use super::{
        provider_429_retry_backoff, provider_429_retry_backoff_i64,
        should_retry_remote_broker_failure,
    };
    use std::time::Duration;

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
    fn provider_429_backoff_follows_staircase_and_caps() {
        assert_eq!(
            provider_429_retry_backoff(Some(429), 1),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            provider_429_retry_backoff(Some(429), 2),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            provider_429_retry_backoff(Some(429), 3),
            Some(Duration::from_secs(4))
        );
        assert_eq!(
            provider_429_retry_backoff(Some(429), 4),
            Some(Duration::from_secs(8))
        );
        assert_eq!(
            provider_429_retry_backoff(Some(429), 5),
            Some(Duration::from_secs(16))
        );
        assert_eq!(
            provider_429_retry_backoff(Some(429), 6),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            provider_429_retry_backoff(Some(429), 7),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            provider_429_retry_backoff(Some(429), 100),
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
}
