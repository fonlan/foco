use foco_providers::NeutralToolCall;

use super::{
    ReadOnlyToolProgressAction, ReadOnlyToolProgressDetector, RepeatedToolCallDetector,
    ToolLoopBeforeExecutionAction,
};

/// Source marker for automatic tool-call-loop interruptions (SSE + persisted parts).
pub(crate) const TOOL_CALL_LOOP_GUARD_SOURCE: &str = "toolCallLoopGuard";
/// Max automatic tool-call-loop recoveries per chat run before the guard fails the run.
/// Independent from reasoning-loop recovery counting.
pub(crate) const MAX_TOOL_CALL_LOOP_RECOVERIES_PER_RUN: usize = 3;

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
