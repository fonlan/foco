use std::fmt;

use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use super::types::ScheduleSpec;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviewNextRunRequest {
    pub schedule: ScheduleSpec,
    #[serde(default)]
    pub count: Option<usize>,
    #[serde(default)]
    pub now: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviewNextRunResponse {
    pub next_run_at: Option<String>,
    pub next_runs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScheduledTaskError {
    InvalidTimestamp { field: &'static str, value: String },
    InvalidInterval { every_seconds: u64 },
    TimestampOverflow,
    CronUnsupported,
}

const PREVIEW_NEXT_RUN_LIMIT: usize = 5;

impl fmt::Display for ScheduledTaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimestamp { field, value } => {
                write!(formatter, "{field} must be an RFC 3339 timestamp: {value}")
            }
            Self::InvalidInterval { every_seconds } => {
                write!(
                    formatter,
                    "interval every_seconds must be between 1 and i64::MAX seconds, got {every_seconds}"
                )
            }
            Self::TimestampOverflow => formatter.write_str("scheduled timestamp is out of range"),
            Self::CronUnsupported => formatter.write_str(
                "cron schedules are deferred for MVP; one_shot_at and interval are supported",
            ),
        }
    }
}

impl std::error::Error for ScheduledTaskError {}

pub(crate) fn preview_next_run(
    request: PreviewNextRunRequest,
) -> Result<PreviewNextRunResponse, ScheduledTaskError> {
    let now = match request.now {
        Some(now) => parse_utc_timestamp("now", &now)?,
        None => Utc::now(),
    };
    let count = request.count.unwrap_or(1).clamp(1, PREVIEW_NEXT_RUN_LIMIT);
    let next_runs = next_runs_after(&request.schedule, now, count)?
        .into_iter()
        .map(format_utc_timestamp)
        .collect::<Vec<_>>();
    Ok(PreviewNextRunResponse {
        next_run_at: next_runs.first().cloned(),
        next_runs,
    })
}

fn next_runs_after(
    schedule: &ScheduleSpec,
    now: DateTime<Utc>,
    count: usize,
) -> Result<Vec<DateTime<Utc>>, ScheduledTaskError> {
    let mut runs = Vec::with_capacity(count);
    let mut cursor = now;
    for _ in 0..count {
        let Some(next) = next_run_after(schedule, cursor)? else {
            break;
        };
        cursor = next;
        runs.push(next);
    }
    Ok(runs)
}

pub(crate) fn next_run_after(
    schedule: &ScheduleSpec,
    now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, ScheduledTaskError> {
    match schedule {
        ScheduleSpec::OneShotAt { run_at } => {
            let run_at = parse_utc_timestamp("schedule.run_at", run_at)?;
            Ok((run_at > now).then_some(run_at))
        }
        ScheduleSpec::Interval {
            every_seconds,
            start_at,
        } => interval_next_run(*every_seconds, start_at.as_deref(), now).map(Some),
        ScheduleSpec::Cron {
            expression: _,
            timezone: _,
        } => Err(ScheduledTaskError::CronUnsupported),
    }
}

fn interval_next_run(
    every_seconds: u64,
    start_at: Option<&str>,
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>, ScheduledTaskError> {
    let every_seconds_i64 = i64::try_from(every_seconds)
        .map_err(|_| ScheduledTaskError::InvalidInterval { every_seconds })?;
    if every_seconds_i64 == 0 {
        return Err(ScheduledTaskError::InvalidInterval { every_seconds });
    }

    let interval = ChronoDuration::seconds(every_seconds_i64);
    let anchor = match start_at {
        Some(start_at) => parse_utc_timestamp("schedule.start_at", start_at)?,
        None => now
            .checked_add_signed(interval)
            .ok_or(ScheduledTaskError::TimestampOverflow)?,
    };
    if anchor > now {
        return Ok(anchor);
    }

    let elapsed_seconds = now.signed_duration_since(anchor).num_seconds();
    let periods = elapsed_seconds / every_seconds_i64 + 1;
    let offset_seconds = periods
        .checked_mul(every_seconds_i64)
        .ok_or(ScheduledTaskError::TimestampOverflow)?;
    anchor
        .checked_add_signed(ChronoDuration::seconds(offset_seconds))
        .ok_or(ScheduledTaskError::TimestampOverflow)
}

fn parse_utc_timestamp(
    field: &'static str,
    value: &str,
) -> Result<DateTime<Utc>, ScheduledTaskError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| ScheduledTaskError::InvalidTimestamp {
            field,
            value: value.to_string(),
        })
}

fn format_utc_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn preview(schedule: ScheduleSpec, now: &str) -> Result<Option<String>, ScheduledTaskError> {
        Ok(preview_next_run(PreviewNextRunRequest {
            count: None,
            schedule,
            now: Some(now.to_string()),
        })?
        .next_run_at)
    }

    #[test]
    fn one_shot_preview_returns_future_timestamp_in_utc() {
        let next = preview(
            ScheduleSpec::OneShotAt {
                run_at: "2026-01-01T09:30:00+08:00".to_string(),
            },
            "2026-01-01T00:00:00Z",
        )
        .expect("preview");

        assert_eq!(next.as_deref(), Some("2026-01-01T01:30:00.000Z"));
    }

    #[test]
    fn one_shot_preview_returns_none_when_past_or_now() {
        let schedule = ScheduleSpec::OneShotAt {
            run_at: "2026-01-01T00:00:00Z".to_string(),
        };

        assert_eq!(
            preview(schedule.clone(), "2026-01-01T00:00:00Z").expect("equal preview"),
            None
        );
        assert_eq!(
            preview(schedule, "2026-01-01T00:00:01Z").expect("past preview"),
            None
        );
    }

    #[test]
    fn interval_preview_returns_next_boundary_after_now() {
        let schedule = ScheduleSpec::Interval {
            every_seconds: 60,
            start_at: Some("2026-01-01T00:00:00Z".to_string()),
        };

        assert_eq!(
            preview(schedule.clone(), "2026-01-01T00:02:00Z").expect("boundary preview"),
            Some("2026-01-01T00:03:00.000Z".to_string())
        );
        assert_eq!(
            preview(schedule, "2026-01-01T00:02:30Z").expect("between preview"),
            Some("2026-01-01T00:03:00.000Z".to_string())
        );
    }

    #[test]
    fn interval_preview_uses_future_start_or_now_plus_interval() {
        assert_eq!(
            preview(
                ScheduleSpec::Interval {
                    every_seconds: 60,
                    start_at: Some("2026-01-01T00:10:00Z".to_string()),
                },
                "2026-01-01T00:02:30Z",
            )
            .expect("future start preview"),
            Some("2026-01-01T00:10:00.000Z".to_string())
        );
        assert_eq!(
            preview(
                ScheduleSpec::Interval {
                    every_seconds: 60,
                    start_at: None,
                },
                "2026-01-01T00:02:30Z",
            )
            .expect("no start preview"),
            Some("2026-01-01T00:03:30.000Z".to_string())
        );
    }

    #[test]
    fn interval_preview_can_return_next_five_runs() {
        let preview = preview_next_run(PreviewNextRunRequest {
            count: Some(5),
            schedule: ScheduleSpec::Interval {
                every_seconds: 60,
                start_at: Some("2026-01-01T00:00:00Z".to_string()),
            },
            now: Some("2026-01-01T00:02:00Z".to_string()),
        })
        .expect("preview");

        assert_eq!(
            preview.next_runs,
            vec![
                "2026-01-01T00:03:00.000Z",
                "2026-01-01T00:04:00.000Z",
                "2026-01-01T00:05:00.000Z",
                "2026-01-01T00:06:00.000Z",
                "2026-01-01T00:07:00.000Z",
            ]
        );
        assert_eq!(
            preview.next_run_at.as_deref(),
            Some("2026-01-01T00:03:00.000Z")
        );
    }

    #[test]
    fn interval_preview_rejects_zero_seconds() {
        let error = preview(
            ScheduleSpec::Interval {
                every_seconds: 0,
                start_at: None,
            },
            "2026-01-01T00:00:00Z",
        )
        .expect_err("zero interval should fail");

        assert!(matches!(
            error,
            ScheduledTaskError::InvalidInterval { every_seconds: 0 }
        ));
    }

    #[test]
    fn cron_schedule_preserves_timezone_but_preview_is_deferred() {
        let schedule = ScheduleSpec::Cron {
            expression: "0 9 * * *".to_string(),
            timezone: Some("Asia/Shanghai".to_string()),
        };
        let json = serde_json::to_value(&schedule).expect("schedule json");

        assert_eq!(
            json,
            json!({
                "type": "cron",
                "expression": "0 9 * * *",
                "timezone": "Asia/Shanghai"
            })
        );
        assert_eq!(
            serde_json::from_value::<ScheduleSpec>(json).expect("schedule round-trip"),
            schedule
        );
        assert!(matches!(
            preview(schedule, "2026-01-01T00:00:00Z"),
            Err(ScheduledTaskError::CronUnsupported)
        ));
    }
}
