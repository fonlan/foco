use std::collections::VecDeque;

const MAX_NORMALIZED_TAIL_CHARS: usize = 8_192;
const MIN_OBSERVED_CHARS: usize = 768;
const MIN_PERIOD_CHARS: usize = 16;
const MAX_PERIOD_CHARS: usize = 1_024;
const MIN_REPEAT_COUNT: usize = 4;
const DETECTION_STRIDE_CHARS: usize = 32;

/// Fixed user-visible recovery text inserted after a bounded reasoning-loop interruption.
pub(crate) const REASONING_LOOP_RECOVERY_USER_TEXT: &str =
    "repeated reasoning loop, check and continue";
/// Source marker for automatic reasoning-loop interruptions (SSE + persisted parts).
pub(crate) const REASONING_LOOP_GUARD_SOURCE: &str = "reasoningLoopGuard";
/// Source marker for manually submitted guidance messages.
pub(crate) const MANUAL_GUIDANCE_SOURCE: &str = "manualGuidance";
/// Max automatic recoveries per chat run before the guard fails the run.
pub(crate) const MAX_REASONING_LOOP_RECOVERIES_PER_RUN: usize = 3;

/// Whether a guidance / userInterruption source is an automatic progress-guard recovery.
///
/// Automatic guard text is already provider-facing control content and must not be wrapped
/// with the manual "User guidance..." prefix during live injection or history replay.
pub(crate) fn is_automatic_guard_source(source: &str) -> bool {
    source == REASONING_LOOP_GUARD_SOURCE
        || source == super::tool_loop::TOOL_CALL_LOOP_GUARD_SOURCE
}

// ponytail: v1 intentionally favors low false positives: it only recognizes exact periodic
// suffixes after whitespace-run normalization, checks at deterministic character boundaries,
// and keeps a fixed tail. If production examples require fuzzier matching, upgrade the
// comparison to a bounded mismatch/rolling-hash scheme without changing this streaming API.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReasoningLoopDetection {
    pub(crate) period_char_count: usize,
    pub(crate) repeat_count: usize,
    pub(crate) observed_char_count: usize,
}

pub(crate) fn reasoning_loop_guard_message(detection: ReasoningLoopDetection) -> String {
    format!(
        "Runtime progress guard stopped the provider stream after detecting a repeated reasoning loop (period {} characters, repeated {} times across {} observed characters). Partial reasoning was preserved.",
        detection.period_char_count, detection.repeat_count, detection.observed_char_count
    )
}

pub(crate) fn default_guidance_source() -> String {
    MANUAL_GUIDANCE_SOURCE.to_string()
}

#[derive(Debug)]
pub(crate) struct ReasoningLoopDetector {
    normalized_tail: VecDeque<char>,
    normalized_char_count: usize,
    previous_was_whitespace: bool,
    detection: Option<ReasoningLoopDetection>,
}

impl Default for ReasoningLoopDetector {
    fn default() -> Self {
        Self {
            normalized_tail: VecDeque::with_capacity(MAX_NORMALIZED_TAIL_CHARS),
            normalized_char_count: 0,
            previous_was_whitespace: false,
            detection: None,
        }
    }
}

impl ReasoningLoopDetector {
    pub(crate) fn push_delta(&mut self, delta: &str) -> Option<ReasoningLoopDetection> {
        if self.detection.is_some() {
            return self.detection;
        }

        for character in delta.chars() {
            let normalized = if character.is_whitespace() {
                if self.previous_was_whitespace {
                    continue;
                }
                self.previous_was_whitespace = true;
                ' '
            } else {
                self.previous_was_whitespace = false;
                character
            };

            self.normalized_tail.push_back(normalized);
            self.normalized_char_count = self.normalized_char_count.saturating_add(1);
            if self.normalized_tail.len() > MAX_NORMALIZED_TAIL_CHARS {
                self.normalized_tail.pop_front();
            }

            if self.normalized_char_count >= MIN_OBSERVED_CHARS
                && self
                    .normalized_char_count
                    .is_multiple_of(DETECTION_STRIDE_CHARS)
                && let Some(detection) = self.detect_repeated_suffix()
            {
                self.detection = Some(detection);
                return self.detection;
            }
        }

        None
    }

    fn detect_repeated_suffix(&self) -> Option<ReasoningLoopDetection> {
        let tail_len = self.normalized_tail.len();
        let max_period = MAX_PERIOD_CHARS.min(tail_len / MIN_REPEAT_COUNT);

        for period_char_count in MIN_PERIOD_CHARS..=max_period {
            let max_repeat_count = tail_len / period_char_count;
            let mut repeat_count = 1;

            while repeat_count < max_repeat_count
                && self.blocks_match(period_char_count, repeat_count)
            {
                repeat_count += 1;
            }

            let observed_char_count = period_char_count * repeat_count;
            if repeat_count >= MIN_REPEAT_COUNT && observed_char_count >= MIN_OBSERVED_CHARS {
                return Some(ReasoningLoopDetection {
                    period_char_count,
                    repeat_count,
                    observed_char_count,
                });
            }
        }

        None
    }

    fn blocks_match(&self, period_char_count: usize, previous_block_offset: usize) -> bool {
        let tail_len = self.normalized_tail.len();
        let latest_start = tail_len - period_char_count;
        let previous_start = tail_len - period_char_count * (previous_block_offset + 1);

        (0..period_char_count).all(|offset| {
            self.normalized_tail[latest_start + offset]
                == self.normalized_tail[previous_start + offset]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_in_chunks(
        text: &str,
        chunk_char_counts: &[usize],
    ) -> (ReasoningLoopDetector, Option<ReasoningLoopDetection>) {
        let mut detector = ReasoningLoopDetector::default();
        let characters = text.chars().collect::<Vec<_>>();
        let mut start = 0;
        let mut chunk_index = 0;
        let mut detection = None;

        while start < characters.len() && detection.is_none() {
            let chunk_len = chunk_char_counts[chunk_index % chunk_char_counts.len()];
            let end = (start + chunk_len).min(characters.len());
            let delta = characters[start..end].iter().collect::<String>();
            detection = detector.push_delta(&delta);
            start = end;
            chunk_index += 1;
        }

        (detector, detection)
    }

    fn assert_not_detected(text: &str) {
        let (_, detection) = feed_in_chunks(text, &[1, 7, 19, 3]);
        assert_eq!(detection, None);
    }

    #[test]
    fn detection_is_independent_of_provider_delta_chunking() {
        let period = "Re-check the same premise, compare the evidence, and restart the analysis. ";
        let text = period.repeat(20);

        let (_, whole) = feed_in_chunks(&text, &[text.chars().count()]);
        let (_, single_char) = feed_in_chunks(&text, &[1]);
        let (_, uneven) = feed_in_chunks(&text, &[3, 41, 2, 17, 89]);

        assert!(whole.is_some());
        assert_eq!(single_char, whole);
        assert_eq!(uneven, whole);
    }

    #[test]
    fn detects_a_repeated_reasoning_section_with_metadata() {
        let period = "Inspect the premise, test the counterexample, then return to the premise. ";
        let text = period.repeat(20);
        let (_, detection) = feed_in_chunks(&text, &[23]);
        let detection = detection.expect("repeated section should be detected");

        assert_eq!(detection.period_char_count, period.chars().count());
        assert!(detection.repeat_count >= MIN_REPEAT_COUNT);
        assert_eq!(
            detection.observed_char_count,
            detection.period_char_count * detection.repeat_count
        );
        assert!(detection.observed_char_count >= MIN_OBSERVED_CHARS);
    }

    #[test]
    fn detects_short_period_spam_after_a_large_observation_window() {
        let period = "still checking...";
        let text = period.repeat(80);
        let (_, detection) = feed_in_chunks(&text, &[5, 13]);
        let detection = detection.expect("short-period spam should be detected");

        assert_eq!(detection.period_char_count, period.chars().count());
        assert!(detection.repeat_count >= MIN_OBSERVED_CHARS / period.chars().count());
    }

    #[test]
    fn ignores_whitespace_run_drift_between_repetitions() {
        let variants = [
            "Review the evidence.\nThen reconsider the premise.\n\n",
            "Review the evidence.  Then reconsider the premise.\t",
            "Review the evidence.\r\nThen reconsider the premise.   ",
        ];
        let text = (0..24)
            .map(|index| variants[index % variants.len()])
            .collect::<String>();
        let (_, detection) = feed_in_chunks(&text, &[2, 31, 7]);

        assert!(detection.is_some());
    }

    #[test]
    fn does_not_detect_before_enough_repeated_text_is_observed() {
        let period = "This substantial section repeats, but not enough times to prove a loop. ";
        let text = period.repeat(MIN_REPEAT_COUNT - 1);

        assert_not_detected(&text);
    }

    #[test]
    fn does_not_flag_a_locally_repeated_short_sentence() {
        let text = format!(
            "I will check the premise carefully. {} Now I will compare the alternatives.",
            "Check again. ".repeat(12)
        );

        assert_not_detected(&text);
    }

    #[test]
    fn does_not_flag_normal_long_reasoning_with_lists_and_code() {
        let text = (0..80)
            .map(|index| {
                format!(
                    "Step {index}: inspect value_{index}, compare it with value_{}, and record the result.\n\
                     ```rust\nlet value_{index} = {index};\n```\n",
                    index + 1
                )
            })
            .collect::<String>();

        assert_not_detected(&text);
    }

    #[test]
    fn does_not_flag_progressively_growing_reasoning() {
        let text = (1..50)
            .map(|count| {
                format!(
                    "Pass {count}: {} conclusion {count}.\n",
                    "check ".repeat(count)
                )
            })
            .collect::<String>();

        assert_not_detected(&text);
    }

    #[test]
    fn keeps_only_a_bounded_normalized_tail() {
        let text = (0..(MAX_NORMALIZED_TAIL_CHARS * 3))
            .map(|index| format!("token-{index:x};"))
            .collect::<String>();
        let (detector, detection) = feed_in_chunks(&text, &[97]);

        assert_eq!(detection, None);
        assert_eq!(detector.normalized_tail.len(), MAX_NORMALIZED_TAIL_CHARS);
        assert_eq!(detector.normalized_char_count, text.chars().count());
    }

    #[test]
    fn automatic_guard_source_predicate_covers_known_sources() {
        assert!(is_automatic_guard_source(REASONING_LOOP_GUARD_SOURCE));
        assert!(is_automatic_guard_source(
            super::super::tool_loop::TOOL_CALL_LOOP_GUARD_SOURCE
        ));
        assert!(!is_automatic_guard_source(MANUAL_GUIDANCE_SOURCE));
        assert!(!is_automatic_guard_source("agentMessage"));
        assert!(!is_automatic_guard_source("userInterruption"));
    }
}
