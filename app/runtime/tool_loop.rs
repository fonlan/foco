use foco_providers::NeutralToolCall;
use serde_json::{Value, json};

use super::{
    ReadOnlyToolProgressAction, ReadOnlyToolProgressDetector, RepeatedToolCallDetector,
    ToolLoopBeforeExecutionAction,
};

/// Source marker for automatic tool-call-loop interruptions (SSE + persisted parts).
pub(crate) const TOOL_CALL_LOOP_GUARD_SOURCE: &str = "toolCallLoopGuard";
/// Max automatic tool-call-loop recoveries per chat run before the guard fails the run.
/// Independent from reasoning-loop recovery counting.
pub(crate) const MAX_TOOL_CALL_LOOP_RECOVERIES_PER_RUN: usize = 3;

/// A provider-submitted call that the repeated-call guard deliberately did not execute.
///
/// These records are observation-only: callers must never turn them into pending work or
/// provider tool messages. `display_id` is intentionally distinct from `provider_call_id`, as
/// providers may reuse a call id across consecutive blocked batches.
#[derive(Clone, Debug)]
pub(crate) struct BlockedToolCall {
    pub(crate) display_id: String,
    pub(crate) provider_call_id: String,
    pub(crate) name: String,
    pub(crate) input: Value,
    pub(crate) output: Value,
}

/// Builds durable, UI-visible terminal call records for one blocked repeated batch.
pub(crate) fn blocked_tool_calls(
    run_id: &str,
    assistant_message_id: &str,
    tool_calls: &[NeutralToolCall],
    blocked_batch_index: usize,
    recovery_limit: usize,
    recovery_available: bool,
) -> Vec<BlockedToolCall> {
    tool_calls
        .iter()
        .enumerate()
        .map(|(batch_index, tool_call)| {
            let reason = if recovery_available {
                format!(
                    "Repeated tool-call batch was blocked and not executed; automatic recovery {blocked_batch_index}/{recovery_limit} will continue."
                )
            } else {
                format!(
                    "Repeated tool-call batch was blocked and not executed; automatic recovery limit {recovery_limit} is exhausted and the run will stop."
                )
            };
            BlockedToolCall {
                display_id: format!(
                    "blocked-tool-call:{run_id}:{assistant_message_id}:{blocked_batch_index}:{batch_index}:{}",
                    tool_call.call_id
                ),
                provider_call_id: tool_call.call_id.clone(),
                name: tool_call.name.clone(),
                input: tool_call.arguments.clone(),
                output: json!({
                    "source": TOOL_CALL_LOOP_GUARD_SOURCE,
                    "executed": false,
                    "originalCallId": tool_call.call_id,
                    "blockedBatchIndex": blocked_batch_index,
                    "recoveryIndex": blocked_batch_index.min(recovery_limit),
                    "recoveryLimit": recovery_limit,
                    "recoveryAvailable": recovery_available,
                    "reason": reason,
                }),
            }
        })
        .collect()
}

/// Per-run guardrails shared by local and remote chat tool loops.
///
/// Transport, persistence, and execution adapters stay outside this state
/// machine; both hosts therefore apply the same transition ordering around an
/// LLM tool-call turn.
#[derive(Default)]
pub(crate) struct ToolLoopGuard {
    tool_rounds: usize,
    repeated_tool_call_detector: RepeatedToolCallDetector,
    read_only_tool_progress_detector: ReadOnlyToolProgressDetector,
}

impl ToolLoopGuard {
    pub(crate) fn check_before_execution(
        &mut self,
        tool_calls: &[NeutralToolCall],
    ) -> Result<ToolLoopBeforeExecutionAction, String> {
        self.repeated_tool_call_detector.check(tool_calls)
    }

    pub(crate) fn reached_round_cap(&self, max_tool_rounds: usize) -> bool {
        self.tool_rounds >= max_tool_rounds
    }

    pub(crate) fn record_executed_round(&mut self) {
        self.tool_rounds = self.tool_rounds.saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) const fn executed_rounds(&self) -> usize {
        self.tool_rounds
    }

    pub(crate) fn reset_after_compression(&mut self) {
        self.tool_rounds = 0;
    }

    pub(crate) fn check_after_execution(
        &mut self,
        tool_calls: &[NeutralToolCall],
    ) -> ReadOnlyToolProgressAction {
        self.read_only_tool_progress_detector.check(tool_calls)
    }
}
