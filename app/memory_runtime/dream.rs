// ponytail: Phase 7/8 will wire this engine; keep Phase 3 buildable without fake callers.
#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use foco_store::{
    config::MemoryDreamSettings,
    memory::{
        MemoryDatabase, MemoryDatabaseError, MemoryDreamChangeStatus, MemoryDreamJobRecord,
        MemoryDreamJobStatus, MemoryDreamRunMode, MemoryDreamSafetyPolicy, MemoryDreamScope,
        MemoryDreamTriggerType, MemoryFactRecord, MemoryKind, MemoryRelationKind, MemoryScope,
        MemoryStatus, NewMemoryDreamChange, NewMemoryDreamJob, NewMemoryEdge, UpdateMemoryDreamJob,
        UpdateMemoryFact,
    },
};
use serde_json::{Value, json};

use crate::*;

// ponytail: no config surface yet; add a setting if users need a different pending TTL.
const STALE_PENDING_DAYS: i64 = 30;
const LOW_CONFIDENCE_PENDING_THRESHOLD: f64 = 0.5;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MemoryDreamJobRequest<'a> {
    pub(crate) scope: MemoryDreamScope,
    pub(crate) workspace_id: Option<&'a str>,
    pub(crate) trigger_type: MemoryDreamTriggerType,
    pub(crate) mode: MemoryDreamRunMode,
    pub(crate) model_id: Option<&'a str>,
    pub(crate) settings: &'a MemoryDreamSettings,
}

#[derive(Clone, Debug)]
pub(crate) struct MemoryDreamJobResult {
    pub(crate) job: MemoryDreamJobRecord,
    pub(crate) applied_changes: usize,
    pub(crate) failed_changes: usize,
}

pub(crate) fn run_memory_dream_job(
    database: &mut MemoryDatabase,
    request: MemoryDreamJobRequest<'_>,
) -> Result<MemoryDreamJobResult, ApiError> {
    let policy = MemoryDreamSafetyPolicy::new(
        request.settings.max_facts_per_run as usize,
        request.settings.max_changes_per_run as usize,
    )
    .map_err(ApiError::from_memory_error)?;
    let latest_success_at = database
        .latest_successful_dream_time(request.scope, request.workspace_id)
        .map_err(ApiError::from_memory_error)?;
    let job_id = unique_id("memory-dream");
    let input_summary_json = json!({
        "scope": request.scope.as_str(),
        "workspaceId": request.workspace_id,
        "triggerType": request.trigger_type.as_str(),
        "mode": request.mode.as_str(),
        "latestSuccessfulDreamAt": latest_success_at,
        "maxFactsPerRun": request.settings.max_facts_per_run,
        "maxChangesPerRun": request.settings.max_changes_per_run,
    })
    .to_string();

    database
        .insert_dream_job(NewMemoryDreamJob {
            id: &job_id,
            scope: request.scope,
            workspace_id: request.workspace_id,
            trigger_type: request.trigger_type,
            mode: request.mode,
            status: MemoryDreamJobStatus::Running,
            model_id: request.model_id,
            input_summary_json: &input_summary_json,
            output_summary_json: None,
            transcript_chat_id: None,
            error_message: None,
        })
        .map_err(ApiError::from_memory_error)?;

    let run_result = run_memory_dream_job_inner(database, &job_id, request, &policy);
    let output_summary_json = match run_result {
        Ok(summary) => {
            let output_summary_json = summary.to_json().to_string();
            database
                .update_dream_job_status(UpdateMemoryDreamJob {
                    id: &job_id,
                    status: MemoryDreamJobStatus::Completed,
                    output_summary_json: Some(&output_summary_json),
                    transcript_chat_id: None,
                    error_message: None,
                })
                .map_err(ApiError::from_memory_error)?;
            output_summary_json
        }
        Err(error) => {
            let error_message = error.to_string();
            let _ = database.update_dream_job_status(UpdateMemoryDreamJob {
                id: &job_id,
                status: MemoryDreamJobStatus::Failed,
                output_summary_json: None,
                transcript_chat_id: None,
                error_message: Some(&error_message),
            });
            return Err(ApiError::from_memory_error(error));
        }
    };

    let job = database
        .dream_jobs_for_scope(request.scope, request.workspace_id, None, 1)
        .map_err(ApiError::from_memory_error)?
        .into_iter()
        .find(|job| job.id == job_id)
        .ok_or_else(|| ApiError::internal("memory Dream job was not found after completion"))?;
    let output_summary: Value =
        serde_json::from_str(&output_summary_json).unwrap_or_else(|_| json!({}));

    Ok(MemoryDreamJobResult {
        job,
        applied_changes: output_summary
            .get("changesApplied")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
        failed_changes: output_summary
            .get("changesFailed")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
    })
}

fn run_memory_dream_job_inner(
    database: &mut MemoryDatabase,
    job_id: &str,
    request: MemoryDreamJobRequest<'_>,
    policy: &MemoryDreamSafetyPolicy,
) -> Result<DreamRunSummary, MemoryDatabaseError> {
    let candidates = database.dream_candidate_facts(
        request.scope,
        request.workspace_id,
        request.settings.max_facts_per_run,
    )?;
    let mut changes = deterministic_changes(database, request.scope, &candidates, policy)?;

    policy.validate_batch_size(candidates.len(), changes.len())?;

    let apply_summary = apply_deterministic_changes(database, job_id, &mut changes, policy)?;
    let profiles_refreshed = refresh_dream_profiles(database, request.scope, &apply_summary)?;

    Ok(DreamRunSummary {
        candidates_considered: candidates.len(),
        deterministic_changes_proposed: changes.len(),
        llm_changes_proposed: 0,
        changes_applied: apply_summary.applied,
        changes_skipped: 0,
        changes_failed: apply_summary.failed,
        profiles_refreshed,
        llm_planner: if request.mode == MemoryDreamRunMode::Llm {
            "deferred_to_phase_4"
        } else {
            "disabled"
        },
    })
}

fn deterministic_changes(
    database: &MemoryDatabase,
    scope: MemoryDreamScope,
    candidates: &[MemoryFactRecord],
    policy: &MemoryDreamSafetyPolicy,
) -> Result<Vec<DreamChange>, MemoryDatabaseError> {
    let now = Utc::now();
    let mut changes = Vec::new();
    let mut changed_fact_ids = HashSet::new();

    for fact in candidates {
        if changes.len() >= policy.max_changes_per_run {
            break;
        }
        let kind = MemoryKind::parse(&fact.kind)?;
        if fact
            .expires_at
            .as_deref()
            .is_some_and(|expires_at| timestamp_is_due(expires_at, now))
            && policy.allows_direct_expiration(kind, fact.pinned, true, true)
            && changed_fact_ids.insert(fact.id.clone())
        {
            changes.push(DreamChange::Expire { fact: fact.clone() });
        }
    }

    for duplicate_group in duplicate_fact_groups(candidates).into_values() {
        if changes.len() >= policy.max_changes_per_run {
            break;
        }
        let Some(winner) = duplicate_group
            .iter()
            .max_by(|left, right| better_duplicate(left, right))
            .cloned()
        else {
            continue;
        };
        for loser in duplicate_group {
            if changes.len() >= policy.max_changes_per_run {
                break;
            }
            if loser.id == winner.id || !changed_fact_ids.insert(loser.id.clone()) {
                continue;
            }
            changes.push(DreamChange::MergeDuplicate {
                winner: winner.clone(),
                loser,
            });
        }
    }

    for fact in candidates {
        if changes.len() >= policy.max_changes_per_run {
            break;
        }
        if fact.status != MemoryStatus::Active.as_str() {
            continue;
        }
        for target in database.update_chain_target_facts(&fact.id)? {
            if changes.len() >= policy.max_changes_per_run {
                break;
            }
            if !scope.allows_candidate_fact_scope(MemoryScope::parse(&target.scope)?) {
                continue;
            }
            if (target.is_latest
                || matches!(
                    MemoryStatus::parse(&target.status)?,
                    MemoryStatus::Active | MemoryStatus::Pending
                ))
                && changed_fact_ids.insert(target.id.clone())
            {
                changes.push(DreamChange::RepairUpdatesChain {
                    source: fact.clone(),
                    target,
                });
            }
        }
    }

    for fact in candidates {
        if changes.len() >= policy.max_changes_per_run {
            break;
        }
        if fact.status == MemoryStatus::Pending.as_str()
            && !fact.pinned
            && fact.kind != MemoryKind::UserNote.as_str()
            && fact.confidence.unwrap_or(0.0) < LOW_CONFIDENCE_PENDING_THRESHOLD
            && timestamp_is_older_than(&fact.created_at, now, STALE_PENDING_DAYS)
            && changed_fact_ids.insert(fact.id.clone())
        {
            changes.push(DreamChange::RejectPending { fact: fact.clone() });
        }
    }

    Ok(changes)
}

fn duplicate_fact_groups(
    candidates: &[MemoryFactRecord],
) -> BTreeMap<(String, String, String), Vec<MemoryFactRecord>> {
    let mut groups = BTreeMap::new();
    for fact in candidates {
        if !matches!(fact.status.as_str(), "active" | "pending") {
            continue;
        }
        let normalized = normalized_duplicate_text(&fact.fact);
        if normalized.is_empty() {
            continue;
        }
        groups
            .entry((fact.scope.clone(), fact.kind.clone(), normalized))
            .or_insert_with(Vec::new)
            .push(fact.clone());
    }

    groups
        .into_iter()
        .filter(|(_, facts)| facts.len() > 1)
        .collect()
}

fn better_duplicate(left: &MemoryFactRecord, right: &MemoryFactRecord) -> Ordering {
    duplicate_rank(left).cmp(&duplicate_rank(right))
}

fn duplicate_rank(fact: &MemoryFactRecord) -> (bool, bool, i32, String, String) {
    (
        fact.pinned,
        fact.status == MemoryStatus::Active.as_str(),
        (fact.confidence.unwrap_or(-1.0) * 1000.0).round() as i32,
        fact.updated_at.clone(),
        fact.id.clone(),
    )
}

fn apply_deterministic_changes(
    database: &mut MemoryDatabase,
    job_id: &str,
    changes: &mut [DreamChange],
    policy: &MemoryDreamSafetyPolicy,
) -> Result<ApplySummary, MemoryDatabaseError> {
    let mut summary = ApplySummary::default();
    for change in changes {
        match apply_deterministic_change(database, job_id, change, policy) {
            Ok(applied) => {
                summary.applied += 1;
                let changed_scope = (applied.scope, applied.chat_id);
                if !summary.changed_scopes.contains(&changed_scope) {
                    summary.changed_scopes.push(changed_scope);
                }
            }
            Err(error) => {
                summary.failed += 1;
                insert_failed_change(database, job_id, change, &error.to_string())?;
            }
        }
    }

    Ok(summary)
}

fn apply_deterministic_change(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &DreamChange,
    policy: &MemoryDreamSafetyPolicy,
) -> Result<AppliedChange, MemoryDatabaseError> {
    match change {
        DreamChange::Expire { fact } => apply_status_change(
            database,
            job_id,
            change,
            fact,
            MemoryStatus::Expired,
            "expired due memory fact",
            policy,
        ),
        DreamChange::RejectPending { fact } => apply_status_change(
            database,
            job_id,
            change,
            fact,
            MemoryStatus::Rejected,
            "rejected stale low-confidence pending memory fact",
            policy,
        ),
        DreamChange::RepairUpdatesChain { target, .. } => apply_status_change(
            database,
            job_id,
            change,
            target,
            MemoryStatus::Superseded,
            "repaired stale is_latest value along updates chain",
            policy,
        ),
        DreamChange::MergeDuplicate { winner, loser } => {
            let current = current_unchanged_fact(database, loser, policy)?;
            let sources = database.sources_for_fact(&loser.id)?;
            for source in sources {
                database.link_fact_source(&winner.id, &source.id)?;
            }
            database.insert_edge(NewMemoryEdge {
                id: &unique_id("memory-edge"),
                source_fact_id: &winner.id,
                target_fact_id: &loser.id,
                relation: MemoryRelationKind::Derives,
                metadata_json: &json!({
                    "operation": "memory_dream_duplicate_merge",
                    "dreamJobId": job_id
                })
                .to_string(),
            })?;
            update_fact_status(database, &loser.id, MemoryStatus::Superseded)?;
            let after = database
                .fact(&loser.id)?
                .ok_or_else(|| missing_fact(&loser.id))?;
            insert_applied_change(database, job_id, change, &current, &after)?;
            Ok(AppliedChange {
                scope: MemoryScope::parse(&after.scope)?,
                chat_id: after.chat_id,
            })
        }
    }
}

fn apply_status_change(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &DreamChange,
    fact: &MemoryFactRecord,
    status: MemoryStatus,
    reason: &str,
    policy: &MemoryDreamSafetyPolicy,
) -> Result<AppliedChange, MemoryDatabaseError> {
    let current = current_unchanged_fact(database, fact, policy)?;
    update_fact_status(database, &fact.id, status)?;
    let after = database
        .fact(&fact.id)?
        .ok_or_else(|| missing_fact(&fact.id))?;
    insert_applied_change_with_reason(database, job_id, change, &current, &after, reason)?;
    Ok(AppliedChange {
        scope: MemoryScope::parse(&after.scope)?,
        chat_id: after.chat_id,
    })
}

fn current_unchanged_fact(
    database: &MemoryDatabase,
    fact: &MemoryFactRecord,
    policy: &MemoryDreamSafetyPolicy,
) -> Result<MemoryFactRecord, MemoryDatabaseError> {
    let current = database
        .fact(&fact.id)?
        .ok_or_else(|| missing_fact(&fact.id))?;
    policy.validate_updated_at(&fact.updated_at, &current.updated_at)?;
    Ok(current)
}

fn update_fact_status(
    database: &mut MemoryDatabase,
    id: &str,
    status: MemoryStatus,
) -> Result<(), MemoryDatabaseError> {
    let updated = database.update_fact(UpdateMemoryFact {
        id,
        status: Some(status),
        is_latest: Some(false),
        ..UpdateMemoryFact::default()
    })?;
    if !updated {
        return Err(missing_fact(id));
    }

    Ok(())
}

fn insert_applied_change(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &DreamChange,
    before: &MemoryFactRecord,
    after: &MemoryFactRecord,
) -> Result<(), MemoryDatabaseError> {
    insert_applied_change_with_reason(database, job_id, change, before, after, change.reason())
}

fn insert_applied_change_with_reason(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &DreamChange,
    before: &MemoryFactRecord,
    after: &MemoryFactRecord,
    reason: &str,
) -> Result<(), MemoryDatabaseError> {
    let target_fact_ids_json = json!(change.target_fact_ids()).to_string();
    let before_json = serde_json::to_string(before).expect("memory fact record serializes");
    let after_json = serde_json::to_string(after).expect("memory fact record serializes");
    let evidence_json = change.evidence_json().to_string();

    database.insert_dream_change(NewMemoryDreamChange {
        id: &unique_id("memory-dream-change"),
        job_id,
        operation: change.operation(),
        target_fact_ids_json: &target_fact_ids_json,
        new_fact_id: None,
        before_json: Some(&before_json),
        after_json: Some(&after_json),
        reason,
        confidence: change.confidence(),
        risk_level: "low",
        status: MemoryDreamChangeStatus::Applied,
        evidence_json: &evidence_json,
        error_message: None,
    })
}

fn insert_failed_change(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &DreamChange,
    error_message: &str,
) -> Result<(), MemoryDatabaseError> {
    let target_fact_ids_json = json!(change.target_fact_ids()).to_string();
    let before_json = change
        .primary_fact()
        .map(|fact| serde_json::to_string(fact).expect("memory fact record serializes"));
    let evidence_json = change.evidence_json().to_string();

    database.insert_dream_change(NewMemoryDreamChange {
        id: &unique_id("memory-dream-change"),
        job_id,
        operation: change.operation(),
        target_fact_ids_json: &target_fact_ids_json,
        new_fact_id: None,
        before_json: before_json.as_deref(),
        after_json: None,
        reason: change.reason(),
        confidence: change.confidence(),
        risk_level: "low",
        status: MemoryDreamChangeStatus::Failed,
        evidence_json: &evidence_json,
        error_message: Some(error_message),
    })
}

fn refresh_dream_profiles(
    database: &mut MemoryDatabase,
    scope: MemoryDreamScope,
    apply_summary: &ApplySummary,
) -> Result<usize, MemoryDatabaseError> {
    if apply_summary.applied == 0 {
        return Ok(0);
    }

    let mut refreshed = 0;
    match scope {
        MemoryDreamScope::Global => {
            database.refresh_profile_from_active_facts(
                MemoryScope::Global,
                None,
                MEMORY_PROFILE_REFRESH_FACT_LIMIT,
            )?;
            refreshed += 1;
        }
        MemoryDreamScope::Workspace => {
            database.refresh_profile_from_active_facts(
                MemoryScope::Workspace,
                None,
                MEMORY_PROFILE_REFRESH_FACT_LIMIT,
            )?;
            refreshed += 1;
            for (fact_scope, chat_id) in &apply_summary.changed_scopes {
                if *fact_scope == MemoryScope::Chat
                    && let Some(chat_id) = chat_id.as_deref()
                {
                    database.refresh_profile_from_active_facts(
                        MemoryScope::Chat,
                        Some(chat_id),
                        MEMORY_PROFILE_REFRESH_FACT_LIMIT,
                    )?;
                    refreshed += 1;
                }
            }
        }
    }

    Ok(refreshed)
}

fn normalized_duplicate_text(value: &str) -> String {
    let mut normalized = String::new();
    let mut last_was_space = true;
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            normalized.push(character);
            last_was_space = false;
        } else if !last_was_space {
            normalized.push(' ');
            last_was_space = true;
        }
    }

    normalized.trim().to_string()
}

fn timestamp_is_due(value: &str, now: DateTime<Utc>) -> bool {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc) <= now)
        .unwrap_or(false)
}

fn timestamp_is_older_than(value: &str, now: DateTime<Utc>, days: i64) -> bool {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| now - timestamp.with_timezone(&Utc) >= ChronoDuration::days(days))
        .unwrap_or(false)
}

fn missing_fact(id: &str) -> MemoryDatabaseError {
    MemoryDatabaseError::InvalidMemoryInput {
        message: format!("memory fact was not found: {id}"),
    }
}

#[derive(Clone, Debug)]
enum DreamChange {
    Expire {
        fact: MemoryFactRecord,
    },
    MergeDuplicate {
        winner: MemoryFactRecord,
        loser: MemoryFactRecord,
    },
    RepairUpdatesChain {
        source: MemoryFactRecord,
        target: MemoryFactRecord,
    },
    RejectPending {
        fact: MemoryFactRecord,
    },
}

impl DreamChange {
    fn operation(&self) -> &'static str {
        match self {
            Self::Expire { .. } => "expire",
            Self::MergeDuplicate { .. } => "merge",
            Self::RepairUpdatesChain { .. } => "repair_updates_chain",
            Self::RejectPending { .. } => "reject",
        }
    }

    fn reason(&self) -> &'static str {
        match self {
            Self::Expire { .. } => "memory fact expires_at is due",
            Self::MergeDuplicate { .. } => "exact duplicate memory fact",
            Self::RepairUpdatesChain { .. } => "updates chain target should not be latest",
            Self::RejectPending { .. } => "stale low-confidence pending memory fact",
        }
    }

    fn target_fact_ids(&self) -> Vec<&str> {
        match self {
            Self::Expire { fact } | Self::RejectPending { fact } => vec![fact.id.as_str()],
            Self::MergeDuplicate { winner, loser } => vec![winner.id.as_str(), loser.id.as_str()],
            Self::RepairUpdatesChain { source, target } => {
                vec![source.id.as_str(), target.id.as_str()]
            }
        }
    }

    fn primary_fact(&self) -> Option<&MemoryFactRecord> {
        match self {
            Self::Expire { fact } | Self::RejectPending { fact } => Some(fact),
            Self::MergeDuplicate { loser, .. } => Some(loser),
            Self::RepairUpdatesChain { target, .. } => Some(target),
        }
    }

    fn confidence(&self) -> Option<f64> {
        self.primary_fact().and_then(|fact| fact.confidence)
    }

    fn evidence_json(&self) -> Value {
        match self {
            Self::Expire { fact } => json!([{
                "sourceType": "memory_fact",
                "sourceId": fact.id,
                "quote": fact.fact,
                "expiresAt": fact.expires_at,
            }]),
            Self::MergeDuplicate { winner, loser } => json!([
                {"sourceType": "memory_fact", "sourceId": winner.id, "quote": winner.fact},
                {"sourceType": "memory_fact", "sourceId": loser.id, "quote": loser.fact}
            ]),
            Self::RepairUpdatesChain { source, target } => json!([
                {"sourceType": "memory_fact", "sourceId": source.id, "quote": source.fact},
                {"sourceType": "memory_fact", "sourceId": target.id, "quote": target.fact}
            ]),
            Self::RejectPending { fact } => json!([{
                "sourceType": "memory_fact",
                "sourceId": fact.id,
                "quote": fact.fact,
                "confidence": fact.confidence,
                "createdAt": fact.created_at,
            }]),
        }
    }
}

#[derive(Default)]
struct ApplySummary {
    applied: usize,
    failed: usize,
    changed_scopes: Vec<(MemoryScope, Option<String>)>,
}

struct AppliedChange {
    scope: MemoryScope,
    chat_id: Option<String>,
}

struct DreamRunSummary {
    candidates_considered: usize,
    deterministic_changes_proposed: usize,
    llm_changes_proposed: usize,
    changes_applied: usize,
    changes_skipped: usize,
    changes_failed: usize,
    profiles_refreshed: usize,
    llm_planner: &'static str,
}

impl DreamRunSummary {
    fn to_json(&self) -> Value {
        json!({
            "candidatesConsidered": self.candidates_considered,
            "deterministicChangesProposed": self.deterministic_changes_proposed,
            "llmChangesProposed": self.llm_changes_proposed,
            "changesApplied": self.changes_applied,
            "changesSkipped": self.changes_skipped,
            "changesFailed": self.changes_failed,
            "profilesRefreshed": self.profiles_refreshed,
            "llmPlanner": self.llm_planner,
        })
    }
}

fn now_minus_days(days: i64) -> String {
    (Utc::now() - ChronoDuration::days(days)).to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use foco_store::{
        memory::{MemorySourceType, NewMemoryFact, NewMemorySource},
        workspace::{WorkspaceDatabase, workspace_database_path},
    };

    use super::*;

    #[test]
    fn deterministic_dream_expires_due_facts() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut database =
            MemoryDatabase::open_or_create_global_at(temp_dir.path().join("memory.sqlite"))
                .expect("global memory database");
        insert_test_fact(
            &mut database,
            "fact-expired",
            MemoryScope::Global,
            None,
            MemoryStatus::Active,
            MemoryKind::Preference,
            "Temporary preference.",
            Some(0.9),
        );
        database
            .update_fact(UpdateMemoryFact {
                id: "fact-expired",
                expires_at: Some(&now_minus_days(1)),
                ..UpdateMemoryFact::default()
            })
            .expect("set expiration");

        let result = run_memory_dream_job(&mut database, test_request(MemoryDreamScope::Global))
            .expect("dream run");

        assert_eq!(result.applied_changes, 1);
        assert_eq!(result.failed_changes, 0);
        assert_eq!(
            database
                .fact("fact-expired")
                .expect("fact")
                .expect("fact exists")
                .status,
            "expired"
        );
        assert_eq!(
            database
                .dream_changes_for_job(&result.job.id, Some(MemoryDreamChangeStatus::Applied), 10)
                .expect("changes")[0]
                .operation,
            "expire"
        );
    }

    #[test]
    fn deterministic_dream_merges_exact_duplicates() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut database =
            MemoryDatabase::open_or_create_global_at(temp_dir.path().join("memory.sqlite"))
                .expect("global memory database");
        insert_test_fact(
            &mut database,
            "fact-old",
            MemoryScope::Global,
            None,
            MemoryStatus::Active,
            MemoryKind::Preference,
            "Prefer compact replies.",
            Some(0.7),
        );
        insert_test_fact(
            &mut database,
            "fact-new",
            MemoryScope::Global,
            None,
            MemoryStatus::Active,
            MemoryKind::Preference,
            "prefer compact replies",
            Some(0.9),
        );

        let result = run_memory_dream_job(&mut database, test_request(MemoryDreamScope::Global))
            .expect("dream run");

        assert_eq!(result.applied_changes, 1);
        assert_eq!(result.failed_changes, 0);
        assert_eq!(
            database
                .fact("fact-old")
                .expect("fact")
                .expect("fact exists")
                .status,
            "superseded"
        );
        assert_eq!(
            database
                .dream_changes_for_job(&result.job.id, Some(MemoryDreamChangeStatus::Applied), 10)
                .expect("changes")[0]
                .operation,
            "merge"
        );
    }

    #[test]
    fn deterministic_dream_repairs_updates_chain_latest_flag() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut database =
            MemoryDatabase::open_or_create_global_at(temp_dir.path().join("memory.sqlite"))
                .expect("global memory database");
        insert_test_fact(
            &mut database,
            "fact-old",
            MemoryScope::Global,
            None,
            MemoryStatus::Active,
            MemoryKind::Preference,
            "Use old workflow.",
            Some(0.8),
        );
        insert_test_fact(
            &mut database,
            "fact-new",
            MemoryScope::Global,
            None,
            MemoryStatus::Active,
            MemoryKind::Preference,
            "Use new workflow.",
            Some(0.8),
        );
        database
            .insert_edge(NewMemoryEdge {
                id: "edge-update",
                source_fact_id: "fact-new",
                target_fact_id: "fact-old",
                relation: MemoryRelationKind::Updates,
                metadata_json: "{}",
            })
            .expect("updates edge");
        database
            .update_fact(UpdateMemoryFact {
                id: "fact-old",
                status: Some(MemoryStatus::Active),
                is_latest: Some(true),
                ..UpdateMemoryFact::default()
            })
            .expect("make stale latest");

        let result = run_memory_dream_job(&mut database, test_request(MemoryDreamScope::Global))
            .expect("dream run");

        assert_eq!(result.applied_changes, 1);
        assert_eq!(result.failed_changes, 0);
        let old = database
            .fact("fact-old")
            .expect("fact")
            .expect("fact exists");
        assert_eq!(old.status, "superseded");
        assert!(!old.is_latest);
    }

    #[test]
    fn deterministic_dream_rejects_stale_low_confidence_pending_facts() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let workspace_dir = temp_dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_dir).expect("workspace dir");
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let mut database =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
                .expect("workspace memory database");
        insert_test_fact(
            &mut database,
            "fact-pending",
            MemoryScope::Workspace,
            None,
            MemoryStatus::Pending,
            MemoryKind::ProjectFact,
            "Old weak candidate.",
            Some(0.1),
        );
        database
            .update_fact(UpdateMemoryFact {
                id: "fact-pending",
                metadata_json: Some(
                    &json!({ "createdAtOverride": now_minus_days(45) }).to_string(),
                ),
                ..UpdateMemoryFact::default()
            })
            .expect("touch pending fact");

        let mut candidates = database
            .dream_candidate_facts(MemoryDreamScope::Workspace, Some("workspace-1"), 10)
            .expect("candidates");
        candidates[0].created_at = now_minus_days(45);
        let policy = MemoryDreamSafetyPolicy::new(10, 10).expect("policy");
        let changes =
            deterministic_changes(&database, MemoryDreamScope::Workspace, &candidates, &policy)
                .expect("changes");

        assert!(matches!(changes[0], DreamChange::RejectPending { .. }));
    }

    #[test]
    fn duplicate_normalization_ignores_case_whitespace_and_punctuation() {
        assert_eq!(
            normalized_duplicate_text(" Prefer   compact replies! "),
            normalized_duplicate_text("prefer, compact replies")
        );
    }

    fn test_request(scope: MemoryDreamScope) -> MemoryDreamJobRequest<'static> {
        let settings = Box::leak(Box::new(MemoryDreamSettings {
            enabled: true,
            auto_enabled: false,
            mode: "deterministic_only".to_string(),
            model_id: None,
            workspace_interval_days: 7,
            global_interval_days: 30,
            create_transcript_chat: false,
            max_facts_per_run: 100,
            max_changes_per_run: 10,
            scheduler_scan_minutes: 60,
        }));
        MemoryDreamJobRequest {
            scope,
            workspace_id: (scope == MemoryDreamScope::Workspace).then_some("workspace-1"),
            trigger_type: MemoryDreamTriggerType::Manual,
            mode: MemoryDreamRunMode::DeterministicOnly,
            model_id: None,
            settings,
        }
    }

    fn insert_test_fact(
        database: &mut MemoryDatabase,
        id: &str,
        scope: MemoryScope,
        chat_id: Option<&str>,
        status: MemoryStatus,
        kind: MemoryKind,
        fact: &str,
        confidence: Option<f64>,
    ) {
        let source_id = format!("{id}-source");
        database
            .insert_source(NewMemorySource {
                id: &source_id,
                scope,
                chat_id,
                source_type: MemorySourceType::ManualNote,
                source_id: None,
                title: "Test memory source",
                content: fact,
                metadata_json: "{}",
            })
            .expect("source insert");
        database
            .insert_fact(NewMemoryFact {
                id,
                scope,
                chat_id,
                status,
                kind,
                fact,
                confidence,
                pinned: false,
                source_ids: &[source_id.as_str()],
                metadata_json: "{}",
            })
            .expect("fact insert");
    }
}
