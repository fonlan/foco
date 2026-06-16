use chrono::{Duration as ChronoDuration, SecondsFormat, Utc};
use foco_store::{
    config::MemorySettings,
    memory::{MemoryDatabase, UpdateMemoryFact},
};

use crate::ApiError;

pub(crate) fn expire_due_memories(database: &mut MemoryDatabase) -> Result<u64, ApiError> {
    database
        .expire_due_facts(&current_memory_timestamp())
        .map_err(ApiError::from_memory_error)
}

pub(crate) fn apply_memory_expiration_to_fact(
    database: &mut MemoryDatabase,
    memory_id: &str,
    memory_settings: &MemorySettings,
) -> Result<(), ApiError> {
    if let Some(expires_at) = memory_expiration_timestamp(memory_settings) {
        database
            .update_fact(UpdateMemoryFact {
                id: memory_id,
                expires_at: Some(&expires_at),
                ..UpdateMemoryFact::default()
            })
            .map_err(ApiError::from_memory_error)?;
    }

    Ok(())
}

fn memory_expiration_timestamp(memory_settings: &MemorySettings) -> Option<String> {
    memory_settings.retention_days.map(|days| {
        (Utc::now() + ChronoDuration::days(i64::from(days)))
            .to_rfc3339_opts(SecondsFormat::Millis, true)
    })
}

fn current_memory_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
