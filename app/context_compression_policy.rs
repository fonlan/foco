use std::time::{Duration, Instant};

use crate::{
    LLM_REQUEST_TIMEOUT_MS,
    provider_retry::{ProviderRetryClass, provider_retry_backoff_for_class_with_retry_after},
};

/// Maximum additional provider attempts for one compression request body.
///
/// The configured chat retry budget remains an upper bound, but compression has a smaller
/// independent cap because its input can be substantially larger than a normal chat turn.
pub(crate) const CONTEXT_COMPRESSION_MAX_RETRIES: u32 = 2;

/// Whether compression was opportunistic or required before another provider request can fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContextCompressionMode {
    Normal,
    RequiredOverflow,
}

impl ContextCompressionMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::RequiredOverflow => "required_overflow",
        }
    }
}

impl From<crate::prompt::LlmContextCompressionMode> for ContextCompressionMode {
    fn from(value: crate::prompt::LlmContextCompressionMode) -> Self {
        match value {
            crate::prompt::LlmContextCompressionMode::Normal => Self::Normal,
            crate::prompt::LlmContextCompressionMode::RequiredOverflow => Self::RequiredOverflow,
        }
    }
}

/// Terminal or next-step action after one failed compression provider attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContextCompressionFailureAction {
    Retry,
    /// A preventive checkpoint failed; retain the original context and continue the chat turn.
    ContinueWithoutCompression,
    /// The prompt cannot safely be sent until a checkpoint frees space.
    FailRequiredOverflow,
    /// User cancellation, shutdown, or an interrupted run stops retry scheduling immediately.
    Stop,
}

impl ContextCompressionFailureAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::ContinueWithoutCompression => "continue_without_compression",
            Self::FailRequiredOverflow => "fail_required_overflow",
            Self::Stop => "stop",
        }
    }
}

/// Per-compression retry limits. The total deadline deliberately never exceeds the provider
/// request deadline, and callers must give each attempt only the remaining duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContextCompressionRetryBudget {
    pub(crate) max_retries: u32,
    pub(crate) total_deadline: Duration,
}

/// A deadline shared by every provider request that contributes to one compression attempt.
/// Hierarchical compression can issue more than one request, so applying the timeout per request
/// would silently exceed the retry budget.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ContextCompressionAttemptDeadline {
    started_at: Instant,
    total: Duration,
}

impl ContextCompressionAttemptDeadline {
    pub(crate) fn new(total: Duration) -> Self {
        Self {
            started_at: Instant::now(),
            total,
        }
    }

    pub(crate) fn remaining(self) -> Option<Duration> {
        self.total
            .checked_sub(self.started_at.elapsed())
            .filter(|remaining| !remaining.is_zero())
    }

    pub(crate) fn remaining_millis(self) -> Option<u64> {
        self.remaining().map(|remaining| {
            let millis = remaining.as_millis().min(u128::from(u64::MAX));
            (millis as u64).max(1)
        })
    }
}

impl ContextCompressionRetryBudget {
    pub(crate) fn from_configured_retry_count(configured_retry_count: u32) -> Self {
        Self {
            max_retries: configured_retry_count.min(CONTEXT_COMPRESSION_MAX_RETRIES),
            total_deadline: Duration::from_millis(LLM_REQUEST_TIMEOUT_MS),
        }
    }

    pub(crate) fn remaining_deadline(self, elapsed: Duration) -> Option<Duration> {
        self.total_deadline
            .checked_sub(elapsed)
            .filter(|remaining| !remaining.is_zero())
    }

    pub(crate) fn retry_backoff(
        self,
        class: ProviderRetryClass,
        retries_used: u32,
        elapsed: Duration,
        retry_after: Option<Duration>,
    ) -> Option<Duration> {
        let remaining = self.remaining_deadline(elapsed)?;
        provider_retry_backoff_for_class_with_retry_after(
            class,
            retries_used.saturating_add(1),
            retry_after,
            Some(remaining),
        )
    }
}

/// Decide the next compression action without inspecting free-form provider error text.
///
/// `retries_used` counts completed additional attempts (zero after the initial request fails).
pub(crate) fn context_compression_failure_action(
    mode: ContextCompressionMode,
    retry_class: ProviderRetryClass,
    retries_used: u32,
    budget: ContextCompressionRetryBudget,
    remaining_deadline: Option<Duration>,
    stop_requested: bool,
) -> ContextCompressionFailureAction {
    if stop_requested {
        return ContextCompressionFailureAction::Stop;
    }
    if retry_class.is_retryable()
        && retries_used < budget.max_retries
        && remaining_deadline.is_some_and(|remaining| !remaining.is_zero())
    {
        return ContextCompressionFailureAction::Retry;
    }
    match mode {
        ContextCompressionMode::Normal => {
            ContextCompressionFailureAction::ContinueWithoutCompression
        }
        ContextCompressionMode::RequiredOverflow => {
            ContextCompressionFailureAction::FailRequiredOverflow
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_failure_degrades_after_retry_budget_is_exhausted() {
        let budget = ContextCompressionRetryBudget::from_configured_retry_count(3);
        assert_eq!(
            context_compression_failure_action(
                ContextCompressionMode::Normal,
                ProviderRetryClass::TransientServer,
                budget.max_retries,
                budget,
                Some(Duration::from_secs(1)),
                false,
            ),
            ContextCompressionFailureAction::ContinueWithoutCompression
        );
    }

    #[test]
    fn required_overflow_failure_remains_terminal_after_retry_budget_is_exhausted() {
        let budget = ContextCompressionRetryBudget::from_configured_retry_count(3);
        assert_eq!(
            context_compression_failure_action(
                ContextCompressionMode::RequiredOverflow,
                ProviderRetryClass::TransientServer,
                budget.max_retries,
                budget,
                Some(Duration::from_secs(1)),
                false,
            ),
            ContextCompressionFailureAction::FailRequiredOverflow
        );
    }

    #[test]
    fn only_structured_transient_classes_retry_before_the_budget_is_exhausted() {
        let budget = ContextCompressionRetryBudget::from_configured_retry_count(10);
        assert_eq!(budget.max_retries, CONTEXT_COMPRESSION_MAX_RETRIES);
        assert_eq!(
            context_compression_failure_action(
                ContextCompressionMode::Normal,
                ProviderRetryClass::TransientServer,
                0,
                budget,
                Some(Duration::from_secs(1)),
                false,
            ),
            ContextCompressionFailureAction::Retry
        );
        assert_eq!(
            context_compression_failure_action(
                ContextCompressionMode::Normal,
                ProviderRetryClass::NonRetryable,
                0,
                budget,
                Some(Duration::from_secs(1)),
                false,
            ),
            ContextCompressionFailureAction::ContinueWithoutCompression
        );
    }

    #[test]
    fn cancellation_or_shutdown_stops_retry_scheduling_immediately() {
        let budget = ContextCompressionRetryBudget::from_configured_retry_count(2);
        assert_eq!(
            context_compression_failure_action(
                ContextCompressionMode::RequiredOverflow,
                ProviderRetryClass::Network,
                0,
                budget,
                Some(Duration::from_secs(30)),
                true,
            ),
            ContextCompressionFailureAction::Stop
        );
    }

    #[test]
    fn total_budget_and_backoff_never_exceed_the_provider_deadline() {
        let budget = ContextCompressionRetryBudget::from_configured_retry_count(2);
        assert_eq!(
            budget.total_deadline,
            Duration::from_millis(LLM_REQUEST_TIMEOUT_MS)
        );
        assert_eq!(budget.remaining_deadline(budget.total_deadline), None);
        assert_eq!(
            budget.retry_backoff(
                ProviderRetryClass::RateLimit,
                0,
                budget.total_deadline - Duration::from_millis(10),
                None,
            ),
            Some(Duration::from_millis(10))
        );
    }
}
