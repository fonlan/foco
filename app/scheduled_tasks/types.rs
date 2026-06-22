use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ScheduleSpec {
    OneShotAt {
        run_at: String,
    },
    Interval {
        every_seconds: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start_at: Option<String>,
    },
    Cron {
        expression: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timezone: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ScheduledAction {
    AgentPrompt {
        prompt: String,
        session_mode: ScheduledSessionMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_definition_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thinking_level: Option<String>,
        #[serde(default)]
        skill_ids: Vec<String>,
        #[serde(default)]
        collaboration_tools_enabled: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScheduledSessionMode {
    CreateNewChat,
    ReuseChat { chat_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledTaskMetadata {
    pub workspace_id: String,
    #[serde(default)]
    pub concurrency_policy: ScheduledConcurrencyPolicy,
    #[serde(default)]
    pub misfire_policy: ScheduledMisfirePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScheduledConcurrencyPolicy {
    SkipIfRunning,
    QueueAfterCurrent,
}

impl Default for ScheduledConcurrencyPolicy {
    fn default() -> Self {
        Self::SkipIfRunning
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScheduledMisfirePolicy {
    Skip,
    CatchUpOnce,
}

impl Default for ScheduledMisfirePolicy {
    fn default() -> Self {
        Self::CatchUpOnce
    }
}
