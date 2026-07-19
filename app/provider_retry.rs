use foco_providers::ProviderConfigError;

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

#[cfg(test)]
mod tests {
    use super::should_retry_remote_broker_failure;

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
}
