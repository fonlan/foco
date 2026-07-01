// ponytail: Phase 8 will add scheduler callers; keep phased builds quiet meanwhile.
#![allow(dead_code)]

use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::Path,
    time::Instant,
};

use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use foco_providers::{
    NeutralChatRequest, NeutralChatRole, NeutralToolDefinition, ProviderConnectionConfig,
};
use foco_store::{
    config::{GlobalConfig, MemoryDreamSettings, MemorySettings, ModelSettings, WorkspaceConfig},
    memory::{
        MEMORY_DREAM_TRANSCRIPT_CHAT_KIND, MemoryDatabase, MemoryDatabaseError,
        MemoryDreamChangeStatus, MemoryDreamJobRecord, MemoryDreamJobStatus, MemoryDreamRunMode,
        MemoryDreamSafetyPolicy, MemoryDreamScope, MemoryDreamTriggerType, MemoryEdgeRecord,
        MemoryFactRecord, MemoryKind, MemoryProfileRecord, MemoryReferenceRecord,
        MemoryReferenceStatus, MemoryReferenceType, MemoryRelationKind, MemoryScope,
        MemorySourceType, MemoryStatus, NewMemoryDreamChange, NewMemoryDreamJob, NewMemoryEdge,
        NewMemoryFact, NewMemoryReference, NewMemorySource, UpdateMemoryDreamJob, UpdateMemoryFact,
    },
    workspace::{NewMessage, WorkspaceDatabase},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::*;

// ponytail: no config surface yet; add a setting if users need a different pending TTL.
const STALE_PENDING_DAYS: i64 = 30;
const LOW_CONFIDENCE_PENDING_THRESHOLD: f64 = 0.5;
const HIGH_CONFIDENCE_PENDING_THRESHOLD: f64 = 0.85;
const MEMORY_DREAM_PLANNER_TOOL_NAME: &str = "submit_memory_dream_changeset";
const MEMORY_DREAM_PLANNER_MAX_OUTPUT_TOKENS: u32 = 2048;
const MEMORY_DREAM_PLANNER_MAX_EDGE_RECORDS: u32 = 80;
const MEMORY_DREAM_PLANNER_MAX_REFERENCE_RECORDS: u32 = 200;
const MEMORY_DREAM_MAX_REFERENCES_PER_FACT: usize = 12;
const MEMORY_DREAM_TRANSCRIPT_MAX_CHARS: usize = 6_000;
const MEMORY_DREAM_ROLLBACK_GUIDANCE: &str = "Automatic Dream never hard-deletes memory facts. \
Recovery is status/field reversal from applied change beforeJson plus cleanup of Dream-created edges \
or promoted facts recorded in change rows.";
const MEMORY_DREAM_PLANNER_SYSTEM_PROMPT: &str = "\
Plan conservative Foco memory maintenance changes from the provided compact audit input. \
Use the submit_memory_dream_changeset tool exactly once. Do not return prose. \
Return JSON only through the tool. Never request source-code edits, shell commands, git operations, external write-capable tools, or hard deletes. \
Every change must cite provided evidence. Prefer no change over weak evidence. \
Global promotion is allowed only when evidence explicitly states a cross-project or user-wide preference. \
Do not invent fact ids, source ids, edge ids, or quotes.";

#[derive(Clone, Copy, Debug)]
pub(crate) struct MemoryDreamJobRequest<'a> {
    pub(crate) scope: MemoryDreamScope,
    pub(crate) workspace_id: Option<&'a str>,
    pub(crate) trigger_type: MemoryDreamTriggerType,
    pub(crate) mode: MemoryDreamRunMode,
    pub(crate) model_id: Option<&'a str>,
    pub(crate) settings: &'a MemoryDreamSettings,
    pub(crate) config: Option<&'a GlobalConfig>,
    pub(crate) global_memory_database_file: Option<&'a Path>,
    pub(crate) planner: Option<MemoryDreamPlannerRequest<'a>>,
    pub(crate) transcript: Option<MemoryDreamTranscriptRequest<'a>>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MemoryDreamPlannerRequest<'a> {
    pub(crate) config: &'a GlobalConfig,
    pub(crate) workspace_path: &'a Path,
    pub(crate) audit_workspace_id: &'a str,
    pub(crate) audit_chat_id: Option<&'a str>,
    pub(crate) chat_model_id: Option<&'a str>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MemoryDreamTranscriptRequest<'a> {
    pub(crate) workspace_path: &'a Path,
}

#[derive(Clone, Debug)]
pub(crate) struct MemoryDreamJobResult {
    pub(crate) job: MemoryDreamJobRecord,
    pub(crate) applied_changes: usize,
    pub(crate) failed_changes: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct MemoryDreamModelSelection {
    pub(crate) model_id: String,
    pub(crate) provider_id: String,
    pub(crate) provider_config: ProviderConnectionConfig,
    pub(crate) max_output_tokens: u32,
}

pub(crate) fn resolve_memory_dream_model(
    config: &GlobalConfig,
    settings: &MemorySettings,
    chat_model_id: Option<&str>,
) -> Result<MemoryDreamModelSelection, ApiError> {
    if let Some(model_id) = settings
        .dream
        .model_id
        .as_deref()
        .and_then(non_empty_trimmed)
    {
        return dream_provider_for_model(config, model_id, "memory Dream configured model");
    }
    if let Some(model_id) = settings
        .extraction_model_id
        .as_deref()
        .and_then(non_empty_trimmed)
    {
        return dream_provider_for_model(config, model_id, "memory extraction fallback model");
    }
    if let Some(model_id) = chat_model_id.and_then(non_empty_trimmed) {
        return dream_provider_for_model(config, model_id, "current chat fallback model");
    }

    let candidates = config
        .models
        .iter()
        .filter(|model| model.enabled && model.active_provider_id.is_some())
        .collect::<Vec<_>>();
    if candidates.len() == 1 {
        return dream_provider_for_model(config, &candidates[0].id, "only configured chat model");
    }

    Err(ApiError::bad_request(
        "memory Dream model is not configured; set memory.dream.modelId or memory.extraction_model_id",
    ))
}

fn dream_provider_for_model(
    config: &GlobalConfig,
    model_id: &str,
    label: &str,
) -> Result<MemoryDreamModelSelection, ApiError> {
    let model = config
        .models
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| ApiError::bad_request(format!("{label} was not found: {model_id}")))?;
    if !model.enabled {
        return Err(ApiError::bad_request(format!(
            "{label} '{}' is disabled",
            model.id
        )));
    }
    model.limits.as_ref().ok_or_else(|| {
        ApiError::bad_request(format!("{label} '{}' is missing limits", model.id))
    })?;
    let provider_id = model.active_provider_id.as_deref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "{label} '{}' has no active provider selected",
            model.id
        ))
    })?;
    if !model.provider_ids.iter().any(|id| id == provider_id) {
        return Err(ApiError::bad_request(format!(
            "active provider '{}' is not associated with {label} '{}'",
            provider_id, model.id
        )));
    }
    let provider = config
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!("{label} provider '{}' was not found", provider_id))
        })?;
    if !provider.enabled {
        return Err(ApiError::bad_request(format!(
            "{label} provider '{}' is disabled",
            provider.id
        )));
    }

    let max_output_tokens =
        model_max_output_tokens(model)?.min(MEMORY_DREAM_PLANNER_MAX_OUTPUT_TOKENS);

    Ok(MemoryDreamModelSelection {
        model_id: model.id.clone(),
        provider_id: provider.id.clone(),
        provider_config: provider_connection_config(provider)?,
        max_output_tokens,
    })
}

fn model_max_output_tokens(model: &ModelSettings) -> Result<u32, ApiError> {
    let limits = model.limits.as_ref().ok_or_else(|| {
        ApiError::bad_request(format!("enabled model '{}' is missing limits", model.id))
    })?;
    u32::try_from(limits.max_output_tokens).map_err(|_| {
        ApiError::bad_request(format!(
            "model '{}' max output tokens exceed u32: {}",
            model.id, limits.max_output_tokens
        ))
    })
}

fn non_empty_trimmed(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

struct MemoryDreamTranscript {
    chat_id: Option<String>,
    database: Option<WorkspaceDatabase>,
    next_sequence: i64,
}

impl MemoryDreamTranscript {
    fn disabled() -> Self {
        Self {
            chat_id: None,
            database: None,
            next_sequence: 0,
        }
    }

    fn create(
        request: MemoryDreamJobRequest<'_>,
        job_id: &str,
        input_summary_json: &str,
    ) -> Result<Self, ApiError> {
        if !request.settings.create_transcript_chat {
            return Ok(Self::disabled());
        }
        let Some(transcript_request) = request.transcript else {
            return Ok(Self::disabled());
        };

        let mut database = WorkspaceDatabase::open_or_create(transcript_request.workspace_path)
            .map_err(ApiError::from_workspace_error)?;
        let chat_id = unique_id("chat");
        let mut metadata = json!({
            "kind": MEMORY_DREAM_TRANSCRIPT_CHAT_KIND,
            "dreamJobId": job_id,
            "scope": request.scope.as_str(),
            "triggerType": request.trigger_type.as_str(),
        });
        if let Some(workspace_id) = request.workspace_id {
            metadata["workspaceId"] = json!(workspace_id);
        }
        let metadata_json = serde_json::to_string(&metadata).map_err(|source| {
            ApiError::internal(format!(
                "failed to serialize memory Dream transcript metadata: {source}"
            ))
        })?;
        database
            .insert_chat_with_metadata(
                &chat_id,
                &memory_dream_transcript_title(request),
                &metadata_json,
            )
            .map_err(ApiError::from_workspace_error)?;

        let mut transcript = Self {
            chat_id: Some(chat_id),
            database: Some(database),
            next_sequence: 0,
        };
        transcript.record_json("job started", metadata);
        transcript.record_json(
            "input summary",
            serde_json::from_str(input_summary_json).unwrap_or_else(|_| json!({})),
        );
        Ok(transcript)
    }

    fn chat_id(&self) -> Option<&str> {
        self.chat_id.as_deref()
    }

    fn record_json(&mut self, title: &str, payload: Value) {
        let Some(chat_id) = self.chat_id.as_deref() else {
            return;
        };
        let Some(database) = self.database.as_mut() else {
            return;
        };
        let metadata_json = serde_json::to_string(&json!({
            "kind": "memory_dream_transcript_step",
            "title": title,
        }))
        .unwrap_or_else(|_| "{}".to_string());
        let content = memory_dream_transcript_content(title, &payload);
        let message_id = unique_id("msg-system");
        let inserted = database.insert_message(NewMessage {
            id: &message_id,
            chat_id,
            role: "system",
            content: &content,
            sequence: self.next_sequence,
            metadata_json: Some(&metadata_json),
        });
        if inserted.is_ok() {
            self.next_sequence += 1;
        }
    }
}

fn memory_dream_transcript_title(request: MemoryDreamJobRequest<'_>) -> String {
    format!(
        "Memory Dream: {} {}",
        request.scope.as_str(),
        request.trigger_type.as_str()
    )
}

fn memory_dream_transcript_content(title: &str, payload: &Value) -> String {
    let payload = serde_json::to_string_pretty(payload).unwrap_or_else(|_| "{}".to_string());
    format!(
        "{title}\n\n{}",
        compact_text(&payload, MEMORY_DREAM_TRANSCRIPT_MAX_CHARS)
    )
}

pub(crate) async fn run_memory_dream_job(
    database: &mut MemoryDatabase,
    request: MemoryDreamJobRequest<'_>,
) -> Result<MemoryDreamJobResult, ApiError> {
    let started_at = Instant::now();
    let policy = MemoryDreamSafetyPolicy::new(
        request.settings.max_facts_per_run as usize,
        request.settings.max_changes_per_run as usize,
    )
    .map_err(ApiError::from_memory_error)?;
    let latest_success_at = database
        .latest_successful_dream_time(request.scope, request.workspace_id)
        .map_err(ApiError::from_memory_error)?;
    let job_id = unique_id("memory-dream");
    let mut model_resolution_error = None;
    let model_selection = if request.mode == MemoryDreamRunMode::Llm {
        match request.planner {
            Some(planner) => resolve_memory_dream_model(
                planner.config,
                &planner.config.memory,
                planner.chat_model_id,
            )
            .map(Some)
            .unwrap_or_else(|error| {
                model_resolution_error = Some(error.message);
                None
            }),
            None => None,
        }
    } else {
        None
    };
    let job_model_id = model_selection
        .as_ref()
        .map(|selection| selection.model_id.as_str())
        .or(request.model_id);
    let input_summary_json = json!({
        "scope": request.scope.as_str(),
        "workspaceId": request.workspace_id,
        "triggerType": request.trigger_type.as_str(),
        "mode": request.mode.as_str(),
        "modelId": job_model_id,
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
            model_id: job_model_id,
            input_summary_json: &input_summary_json,
            output_summary_json: None,
            transcript_chat_id: None,
            error_message: None,
        })
        .map_err(ApiError::from_memory_error)?;
    tracing::info!(
        job_id = %job_id,
        scope = request.scope.as_str(),
        workspace_id = request.workspace_id,
        trigger_type = request.trigger_type.as_str(),
        mode = request.mode.as_str(),
        model_id = job_model_id,
        "Memory Dream job started"
    );

    let mut transcript = MemoryDreamTranscript::create(request, &job_id, &input_summary_json)?;
    if let Some(transcript_chat_id) = transcript.chat_id() {
        database
            .update_dream_job_status(UpdateMemoryDreamJob {
                id: &job_id,
                status: MemoryDreamJobStatus::Running,
                output_summary_json: None,
                transcript_chat_id: Some(transcript_chat_id),
                error_message: None,
            })
            .map_err(ApiError::from_memory_error)?;
    }

    let run_result = run_memory_dream_job_inner(
        database,
        &job_id,
        request,
        model_selection,
        model_resolution_error,
        &policy,
        &mut transcript,
    )
    .await;
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
            transcript.record_json(
                "final status",
                json!({
                    "status": MemoryDreamJobStatus::Completed.as_str(),
                    "summary": summary.to_json(),
                }),
            );
            tracing::info!(
                job_id = %job_id,
                scope = request.scope.as_str(),
                workspace_id = request.workspace_id,
                candidates_considered = summary.candidates_considered,
                deterministic_changes_proposed = summary.deterministic_changes_proposed,
                llm_changes_proposed = summary.llm_changes_proposed,
                changes_applied = summary.changes_applied,
                changes_skipped = summary.changes_skipped,
                changes_failed = summary.changes_failed,
                profiles_refreshed = summary.profiles_refreshed,
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                "Memory Dream job completed"
            );
            output_summary_json
        }
        Err(error) => {
            let error_message = error.message.clone();
            let failure_summary = DreamRunSummary::failed(&error);
            let output_summary_json = failure_summary.to_json().to_string();
            transcript.record_json(
                "final status",
                json!({
                    "status": MemoryDreamJobStatus::Failed.as_str(),
                    "errorMessage": &error_message,
                    "summary": failure_summary.to_json(),
                }),
            );
            let _ = database.update_dream_job_status(UpdateMemoryDreamJob {
                id: &job_id,
                status: MemoryDreamJobStatus::Failed,
                output_summary_json: Some(&output_summary_json),
                transcript_chat_id: None,
                error_message: Some(&error_message),
            });
            tracing::error!(
                job_id = %job_id,
                scope = request.scope.as_str(),
                workspace_id = request.workspace_id,
                failure_category = failure_summary.failure_category.unwrap_or("unknown"),
                error = %error_message,
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                "Memory Dream job failed"
            );
            return Err(error);
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

async fn run_memory_dream_job_inner(
    database: &mut MemoryDatabase,
    job_id: &str,
    request: MemoryDreamJobRequest<'_>,
    model_selection: Option<MemoryDreamModelSelection>,
    model_resolution_error: Option<String>,
    policy: &MemoryDreamSafetyPolicy,
    transcript: &mut MemoryDreamTranscript,
) -> Result<DreamRunSummary, ApiError> {
    let candidates = database
        .dream_candidate_facts(
            request.scope,
            request.workspace_id,
            request.settings.max_facts_per_run,
        )
        .map_err(ApiError::from_memory_error)?;
    let reference_validation =
        refresh_memory_dream_reference_validation(database, request, &candidates)
            .map_err(ApiError::from_memory_error)?;
    let mut changes = deterministic_changes(database, request.scope, &candidates, policy)
        .map_err(ApiError::from_memory_error)?;
    tracing::info!(
        job_id,
        scope = request.scope.as_str(),
        workspace_id = request.workspace_id,
        candidates_considered = candidates.len(),
        deterministic_changes_proposed = changes.len(),
        "Memory Dream deterministic planning completed"
    );
    transcript.record_json(
        "deterministic candidates",
        json!({
            "candidateCount": candidates.len(),
            "candidateFactIds": candidates
                .iter()
                .take(50)
                .map(|fact| fact.id.as_str())
                .collect::<Vec<_>>(),
            "deterministicChangesProposed": changes.len(),
            "referenceValidation": reference_validation.to_json(),
            "deterministicOperations": changes
                .iter()
                .map(DreamChange::operation)
                .collect::<Vec<_>>(),
        }),
    );

    policy
        .validate_batch_size(candidates.len(), changes.len())
        .map_err(ApiError::from_memory_error)?;
    transcript.record_json(
        "backend validation summary",
        json!({
            "stage": "deterministic",
            "accepted": true,
            "candidateCount": candidates.len(),
            "changeCount": changes.len(),
            "referenceCount": reference_validation.total,
            "invalidReferenceCount": reference_validation.invalid,
            "ambiguousReferenceCount": reference_validation.ambiguous,
            "maxFactsPerRun": request.settings.max_facts_per_run,
            "maxChangesPerRun": request.settings.max_changes_per_run,
        }),
    );

    let mut apply_summary = apply_deterministic_changes(database, job_id, &mut changes, policy)
        .map_err(ApiError::from_memory_error)?;
    let mut profiles_refreshed = refresh_dream_profiles(database, request.scope, &apply_summary)
        .map_err(ApiError::from_memory_error)?;
    tracing::info!(
        job_id,
        scope = request.scope.as_str(),
        workspace_id = request.workspace_id,
        changes_applied = apply_summary.applied,
        changes_failed = apply_summary.failed,
        profiles_refreshed,
        "Memory Dream deterministic changes applied"
    );
    transcript.record_json(
        "applied changes summary",
        json!({
            "stage": "deterministic",
            "applied": apply_summary.applied,
            "failed": apply_summary.failed,
            "profilesRefreshed": profiles_refreshed,
        }),
    );
    let mut llm_changes_proposed = 0;
    let mut changes_skipped = 0;
    let mut llm_planner = if request.mode == MemoryDreamRunMode::Llm {
        "not_configured"
    } else {
        "disabled"
    };
    let mut llm_error = None;

    if request.mode == MemoryDreamRunMode::Llm {
        let candidate_fact_ids = candidates
            .iter()
            .map(|fact| fact.id.clone())
            .collect::<Vec<_>>();
        let candidate_edges = database
            .edges_for_fact_ids(&candidate_fact_ids, MEMORY_DREAM_PLANNER_MAX_EDGE_RECORDS)
            .map_err(ApiError::from_memory_error)?;
        let source_summaries = memory_dream_source_summaries(database, &candidates)
            .map_err(ApiError::from_memory_error)?;
        let profiles = database
            .profiles_for_scope(None, 8)
            .map_err(ApiError::from_memory_error)?;
        let llm_result = match (request.planner, model_selection.as_ref()) {
            (Some(planner), Some(selection)) => {
                run_memory_dream_llm_planner(
                    planner,
                    selection,
                    request,
                    &candidates,
                    &candidate_edges,
                    &reference_validation.references,
                    &source_summaries,
                    &profiles,
                    &changes,
                    policy,
                    transcript,
                )
                .await
            }
            _ => Err(ApiError::bad_request(
                model_resolution_error.unwrap_or_else(|| {
                    "memory Dream LLM planner requires a configured model and audit workspace"
                        .to_string()
                }),
            )),
        };

        match llm_result {
            Ok(validated_changes) => {
                llm_planner = "applied";
                llm_changes_proposed = validated_changes.len();
                tracing::info!(
                    job_id,
                    scope = request.scope.as_str(),
                    workspace_id = request.workspace_id,
                    llm_changes_proposed,
                    "Memory Dream LLM planning completed"
                );
                let llm_apply_summary = apply_llm_changes(
                    database,
                    job_id,
                    &validated_changes,
                    policy,
                    request.workspace_id,
                    request.global_memory_database_file,
                )
                .map_err(ApiError::from_memory_error)?;
                let llm_applied = llm_apply_summary.applied;
                let llm_failed = llm_apply_summary.failed;
                apply_summary.applied += llm_apply_summary.applied;
                apply_summary.failed += llm_apply_summary.failed;
                for changed_scope in llm_apply_summary.changed_scopes {
                    if !apply_summary.changed_scopes.contains(&changed_scope) {
                        apply_summary.changed_scopes.push(changed_scope);
                    }
                }
                profiles_refreshed =
                    refresh_dream_profiles(database, request.scope, &apply_summary)
                        .map_err(ApiError::from_memory_error)?;
                transcript.record_json(
                    "applied changes summary",
                    json!({
                        "stage": "llm",
                        "applied": llm_applied,
                        "failed": llm_failed,
                        "profilesRefreshed": profiles_refreshed,
                    }),
                );
                tracing::info!(
                    job_id,
                    scope = request.scope.as_str(),
                    workspace_id = request.workspace_id,
                    changes_applied = llm_applied,
                    changes_failed = llm_failed,
                    profiles_refreshed,
                    "Memory Dream LLM changes applied"
                );
            }
            Err(error)
                if request.trigger_type != MemoryDreamTriggerType::Manual
                    || apply_summary.applied > 0 =>
            {
                llm_planner = "fallback_deterministic_only";
                transcript.record_json(
                    "LLM planner failure",
                    json!({
                        "fallback": llm_planner,
                        "errorMessage": &error.message,
                    }),
                );
                tracing::warn!(
                    job_id,
                    scope = request.scope.as_str(),
                    workspace_id = request.workspace_id,
                    failure_category = dream_failure_category(&error),
                    error = %error.message,
                    "Memory Dream LLM planner fell back to deterministic-only"
                );
                llm_error = Some(error.message);
                changes_skipped = 1;
            }
            Err(error) => {
                transcript.record_json(
                    "LLM planner failure",
                    json!({
                        "fallback": null,
                        "errorMessage": &error.message,
                    }),
                );
                return Err(error);
            }
        }
    }

    Ok(DreamRunSummary {
        candidates_considered: candidates.len(),
        references_extracted: reference_validation.total,
        references_valid: reference_validation.valid,
        references_invalid: reference_validation.invalid,
        references_ambiguous: reference_validation.ambiguous,
        references_skipped: reference_validation.skipped,
        deterministic_changes_proposed: changes.len(),
        llm_changes_proposed,
        changes_applied: apply_summary.applied,
        changes_skipped,
        changes_failed: apply_summary.failed,
        profiles_refreshed,
        llm_planner,
        llm_error,
        failure_category: None,
        error_message: None,
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
        if fact.status == MemoryStatus::Pending.as_str()
            && high_confidence_pending_is_promotable(database, fact, now)?
            && changed_fact_ids.insert(fact.id.clone())
        {
            changes.push(DreamChange::ActivatePending { fact: fact.clone() });
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

    if scope == MemoryDreamScope::Workspace {
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
    }

    Ok(changes)
}

async fn run_memory_dream_llm_planner(
    planner: MemoryDreamPlannerRequest<'_>,
    selection: &MemoryDreamModelSelection,
    request: MemoryDreamJobRequest<'_>,
    candidates: &[MemoryFactRecord],
    candidate_edges: &[MemoryEdgeRecord],
    candidate_references: &[MemoryReferenceRecord],
    source_summaries: &[Value],
    profiles: &[MemoryProfileRecord],
    deterministic_changes: &[DreamChange],
    policy: &MemoryDreamSafetyPolicy,
    transcript: &mut MemoryDreamTranscript,
) -> Result<Vec<ValidatedDreamPlannerChange>, ApiError> {
    let input = memory_dream_planner_input(
        request,
        candidates,
        candidate_edges,
        candidate_references,
        source_summaries,
        profiles,
        deterministic_changes,
    )?;
    transcript.record_json(
        "LLM planner request summary",
        json!({
            "modelId": &selection.model_id,
            "providerId": &selection.provider_id,
            "candidateFacts": candidates.len(),
            "relevantEdges": candidate_edges.len(),
            "referenceValidation": candidate_references.len(),
            "sourceSummaries": source_summaries.len(),
            "memoryProfiles": profiles.len(),
            "deterministicChanges": deterministic_changes.len(),
        }),
    );
    let output = match request_memory_dream_planner_output(planner, selection, input).await {
        Ok(output) => output,
        Err(error) => {
            transcript.record_json(
                "LLM changeset parse failure",
                json!({ "errorMessage": &error.message }),
            );
            return Err(error);
        }
    };
    transcript.record_json(
        "LLM changeset JSON",
        serde_json::to_value(&output).unwrap_or_else(|_| json!({})),
    );
    let validated =
        validate_memory_dream_planner_output(&output, request.scope, candidates, policy)?;
    transcript.record_json(
        "backend validation summary",
        json!({
            "stage": "llm",
            "accepted": true,
            "changesProposed": output.changes.len(),
            "changesAccepted": validated.len(),
        }),
    );
    Ok(validated)
}

fn memory_dream_source_summaries(
    database: &MemoryDatabase,
    candidates: &[MemoryFactRecord],
) -> Result<Vec<Value>, MemoryDatabaseError> {
    let mut summaries = Vec::new();
    for fact in candidates.iter().take(100) {
        for source in database.sources_for_fact(&fact.id)?.into_iter().take(2) {
            summaries.push(json!({
                "factId": &fact.id,
                "sourceId": source.id,
                "sourceType": source.source_type,
                "title": source.title,
                "contentSummary": compact_text(&source.content, 600),
                "metadata": compact_json_text(&source.metadata_json),
                "createdAt": source.created_at,
            }));
        }
    }

    Ok(summaries)
}

fn refresh_memory_dream_reference_validation(
    database: &mut MemoryDatabase,
    request: MemoryDreamJobRequest<'_>,
    candidates: &[MemoryFactRecord],
) -> Result<DreamReferenceValidationSummary, MemoryDatabaseError> {
    if candidates.is_empty() {
        return Ok(DreamReferenceValidationSummary::default());
    }

    let workspace = memory_dream_reference_workspace(request);
    let (workspace_database, workspace_database_error) = match workspace {
        Some(workspace) => match WorkspaceDatabase::open_or_create(&workspace.path) {
            Ok(database) => (Some(database), None),
            Err(error) => (None, Some(error.to_string())),
        },
        None => (None, None),
    };
    let checked_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    for fact in candidates {
        let owned_references = validated_memory_references_for_fact(
            fact,
            request.config,
            workspace,
            workspace_database.as_ref(),
            workspace_database_error.as_deref(),
            &checked_at,
        )?;
        let references = owned_references
            .iter()
            .map(|reference| NewMemoryReference {
                id: &reference.id,
                fact_id: &reference.fact_id,
                reference_type: reference.reference_type,
                value: &reference.value,
                normalized_value: &reference.normalized_value,
                status: reference.status,
                metadata_json: &reference.metadata_json,
                checked_at: reference.checked_at.as_deref(),
            })
            .collect::<Vec<_>>();
        database.replace_fact_references(&fact.id, &references)?;
    }

    let fact_ids = candidates
        .iter()
        .map(|fact| fact.id.clone())
        .collect::<Vec<_>>();
    let references =
        database.references_for_fact_ids(&fact_ids, MEMORY_DREAM_PLANNER_MAX_REFERENCE_RECORDS)?;
    Ok(DreamReferenceValidationSummary::from_references(references))
}

fn memory_dream_reference_workspace<'a>(
    request: MemoryDreamJobRequest<'a>,
) -> Option<&'a WorkspaceConfig> {
    if request.scope != MemoryDreamScope::Workspace {
        return None;
    }
    let workspace_id = request.workspace_id?;
    request
        .config?
        .workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)
}

fn validated_memory_references_for_fact(
    fact: &MemoryFactRecord,
    config: Option<&GlobalConfig>,
    workspace: Option<&WorkspaceConfig>,
    workspace_database: Option<&WorkspaceDatabase>,
    workspace_database_error: Option<&str>,
    checked_at: &str,
) -> Result<Vec<OwnedMemoryReference>, MemoryDatabaseError> {
    extract_memory_references(&fact.fact)
        .into_iter()
        .take(MEMORY_DREAM_MAX_REFERENCES_PER_FACT)
        .enumerate()
        .map(|(index, reference)| {
            let validation = validate_extracted_memory_reference(
                &reference,
                config,
                workspace,
                workspace_database,
                workspace_database_error,
            )?;
            Ok(OwnedMemoryReference {
                id: format!("memory-reference:{}:{index}", fact.id),
                fact_id: fact.id.clone(),
                reference_type: reference.reference_type,
                value: reference.value,
                normalized_value: reference.normalized_value,
                status: validation.status,
                metadata_json: serde_json::to_string(&validation.metadata).map_err(|source| {
                    MemoryDatabaseError::InvalidMemoryJson {
                        field: "metadata_json",
                        source,
                    }
                })?,
                checked_at: Some(checked_at.to_string()),
            })
        })
        .collect()
}

fn extract_memory_references(value: &str) -> Vec<ExtractedMemoryReference> {
    let mut references = Vec::new();
    let mut seen = HashSet::new();

    for quoted in backtick_fragments(value) {
        push_extracted_reference(&mut references, &mut seen, &quoted, true);
    }
    for token in reference_tokens(value) {
        push_extracted_reference(&mut references, &mut seen, &token, false);
    }

    references
}

fn push_extracted_reference(
    references: &mut Vec<ExtractedMemoryReference>,
    seen: &mut HashSet<(MemoryReferenceType, String)>,
    value: &str,
    quoted: bool,
) {
    let Some(reference_type) = classify_memory_reference(value, quoted) else {
        return;
    };
    let Some(normalized_value) = normalized_reference_value(reference_type, value) else {
        return;
    };
    if seen.insert((reference_type, normalized_value.clone())) {
        references.push(ExtractedMemoryReference {
            reference_type,
            value: reference_value_for_storage(reference_type, value, &normalized_value),
            normalized_value,
        });
    }
}

fn classify_memory_reference(value: &str, quoted: bool) -> Option<MemoryReferenceType> {
    let trimmed = trimmed_reference_value(value);
    if trimmed.is_empty() {
        return None;
    }
    if looks_like_url(trimmed) {
        return Some(MemoryReferenceType::Url);
    }
    if looks_like_workspace_id_reference(trimmed) {
        return Some(MemoryReferenceType::WorkspaceId);
    }
    if looks_like_command_reference(trimmed) {
        return Some(MemoryReferenceType::Command);
    }
    if looks_like_file_reference(trimmed) {
        return Some(MemoryReferenceType::FilePath);
    }
    if quoted && looks_like_symbol_reference(trimmed) {
        return Some(MemoryReferenceType::Symbol);
    }

    None
}

fn validate_extracted_memory_reference(
    reference: &ExtractedMemoryReference,
    config: Option<&GlobalConfig>,
    workspace: Option<&WorkspaceConfig>,
    workspace_database: Option<&WorkspaceDatabase>,
    workspace_database_error: Option<&str>,
) -> Result<ReferenceValidation, MemoryDatabaseError> {
    match reference.reference_type {
        MemoryReferenceType::FilePath => Ok(validate_file_reference(reference, workspace)),
        MemoryReferenceType::Symbol => Ok(validate_symbol_reference(
            reference,
            workspace_database,
            workspace_database_error,
        )),
        MemoryReferenceType::Command => {
            validate_command_reference(reference, workspace).map_err(|source| {
                MemoryDatabaseError::InvalidMemoryInput {
                    message: format!("failed to validate command reference: {source}"),
                }
            })
        }
        MemoryReferenceType::Url => Ok(validate_url_reference(reference)),
        MemoryReferenceType::WorkspaceId => Ok(validate_workspace_id_reference(reference, config)),
    }
}

fn validate_file_reference(
    reference: &ExtractedMemoryReference,
    workspace: Option<&WorkspaceConfig>,
) -> ReferenceValidation {
    let Some(workspace) = workspace else {
        return ReferenceValidation::skipped(json!({
            "reason": "noWorkspaceContext",
        }));
    };
    let candidate = workspace.path.join(&reference.normalized_value);
    match fs::metadata(&candidate) {
        Ok(_) => match path_stays_inside_workspace(&workspace.path, &candidate) {
            true => ReferenceValidation::valid(json!({
                "path": reference.normalized_value,
            })),
            false => ReferenceValidation::invalid(json!({
                "reason": "outsideWorkspace",
                "path": reference.normalized_value,
            })),
        },
        Err(_) => {
            let matches = moved_file_candidates(&workspace.path, &reference.normalized_value, 3);
            match matches.len() {
                0 => ReferenceValidation::invalid(json!({
                    "reason": "notFound",
                    "path": reference.normalized_value,
                })),
                1 => ReferenceValidation::invalid(json!({
                    "reason": "moved",
                    "path": reference.normalized_value,
                    "candidatePath": matches[0],
                })),
                _ => ReferenceValidation::ambiguous(json!({
                    "reason": "multipleMovedCandidates",
                    "path": reference.normalized_value,
                    "candidatePaths": matches,
                })),
            }
        }
    }
}

fn validate_symbol_reference(
    reference: &ExtractedMemoryReference,
    workspace_database: Option<&WorkspaceDatabase>,
    workspace_database_error: Option<&str>,
) -> ReferenceValidation {
    let Some(database) = workspace_database else {
        return ReferenceValidation::skipped(json!({
            "reason": "codeGraphUnavailable",
            "error": workspace_database_error,
        }));
    };
    match database.find_code_graph_symbols(&reference.normalized_value, None, None, 3) {
        Ok(matches) if matches.len() == 1 => ReferenceValidation::valid(json!({
            "symbolId": matches[0].id,
            "path": matches[0].path,
            "name": matches[0].name,
            "kind": matches[0].kind,
        })),
        Ok(matches) if matches.is_empty() => ReferenceValidation::invalid(json!({
            "reason": "symbolNotFound",
            "query": reference.normalized_value,
        })),
        Ok(matches) => ReferenceValidation::ambiguous(json!({
            "reason": "multipleSymbols",
            "matches": matches.into_iter().map(|symbol| json!({
                "symbolId": symbol.id,
                "path": symbol.path,
                "name": symbol.name,
                "kind": symbol.kind,
            })).collect::<Vec<_>>(),
        })),
        Err(error) => ReferenceValidation::skipped(json!({
            "reason": "codeGraphLookupFailed",
            "error": error.to_string(),
        })),
    }
}

fn validate_command_reference(
    reference: &ExtractedMemoryReference,
    workspace: Option<&WorkspaceConfig>,
) -> Result<ReferenceValidation, std::io::Error> {
    let Some(workspace) = workspace else {
        return Ok(ReferenceValidation::skipped(json!({
            "reason": "noWorkspaceContext",
        })));
    };
    let command = reference.normalized_value.as_str();
    if let Some(common_command) = workspace.common_commands.iter().find(|common| {
        command == normalized_command(&common.name)
            || command == normalized_command(&common.command)
            || command.starts_with(&(normalized_command(&common.command) + " "))
    }) {
        return Ok(ReferenceValidation::valid(json!({
            "source": "workspaceCommonCommand",
            "name": common_command.name,
        })));
    }

    let words = command.split_whitespace().collect::<Vec<_>>();
    if words.first() == Some(&"cargo") && workspace.path.join("Cargo.toml").is_file() {
        return Ok(ReferenceValidation::valid(json!({
            "source": "Cargo.toml",
        })));
    }
    if let Some(script) = package_script_from_command(&words) {
        return Ok(match package_json_has_script(&workspace.path, script)? {
            true => ReferenceValidation::valid(json!({
                "source": "package.json",
                "script": script,
            })),
            false => ReferenceValidation::invalid(json!({
                "reason": "packageScriptNotFound",
                "script": script,
            })),
        });
    }
    if matches!(words.first().copied(), Some("npm" | "pnpm" | "yarn"))
        && workspace.path.join("package.json").is_file()
    {
        return Ok(ReferenceValidation::valid(json!({
            "source": "package.json",
        })));
    }

    Ok(ReferenceValidation::invalid(json!({
        "reason": "notInManifestOrCommonCommands",
    })))
}

fn validate_url_reference(reference: &ExtractedMemoryReference) -> ReferenceValidation {
    let host = url_host(&reference.normalized_value);
    match host {
        Some(host) => ReferenceValidation::valid(json!({
            "host": host,
            "networkValidation": "skipped",
        })),
        None => ReferenceValidation::invalid(json!({
            "reason": "invalidUrl",
        })),
    }
}

fn validate_workspace_id_reference(
    reference: &ExtractedMemoryReference,
    config: Option<&GlobalConfig>,
) -> ReferenceValidation {
    let workspace_id = workspace_id_reference_value(&reference.normalized_value);
    let Some(config) = config else {
        return ReferenceValidation::skipped(json!({
            "reason": "noConfigContext",
            "workspaceId": workspace_id,
        }));
    };
    if config
        .workspaces
        .iter()
        .any(|workspace| workspace.id == workspace_id)
    {
        ReferenceValidation::valid(json!({
            "workspaceId": workspace_id,
        }))
    } else {
        ReferenceValidation::invalid(json!({
            "reason": "workspaceNotFound",
            "workspaceId": workspace_id,
        }))
    }
}

fn memory_references_for_fact_json(
    references: &[MemoryReferenceRecord],
    fact_id: &str,
) -> Vec<Value> {
    references
        .iter()
        .filter(|reference| reference.fact_id == fact_id)
        .map(memory_reference_json)
        .collect()
}

fn memory_references_json(references: &[MemoryReferenceRecord]) -> Vec<Value> {
    references.iter().map(memory_reference_json).collect()
}

fn memory_reference_json(reference: &MemoryReferenceRecord) -> Value {
    json!({
        "id": &reference.id,
        "factId": &reference.fact_id,
        "type": &reference.reference_type,
        "value": &reference.value,
        "normalizedValue": &reference.normalized_value,
        "status": &reference.status,
        "metadata": compact_json_text(&reference.metadata_json),
        "checkedAt": &reference.checked_at,
    })
}

fn memory_dream_planner_input(
    request: MemoryDreamJobRequest<'_>,
    candidates: &[MemoryFactRecord],
    candidate_edges: &[MemoryEdgeRecord],
    candidate_references: &[MemoryReferenceRecord],
    source_summaries: &[Value],
    profiles: &[MemoryProfileRecord],
    deterministic_changes: &[DreamChange],
) -> Result<Value, ApiError> {
    let candidates_json = candidates
        .iter()
        .map(|fact| {
            json!({
                "id": &fact.id,
                "scope": &fact.scope,
                "chatId": &fact.chat_id,
                "status": &fact.status,
                "kind": &fact.kind,
                "fact": &fact.fact,
                "confidence": fact.confidence,
                "pinned": fact.pinned,
                "isLatest": fact.is_latest,
                "expiresAt": &fact.expires_at,
                "references": memory_references_for_fact_json(candidate_references, &fact.id),
                "createdAt": &fact.created_at,
                "updatedAt": &fact.updated_at,
            })
        })
        .collect::<Vec<_>>();
    let edges_json = candidate_edges
        .iter()
        .map(|edge| {
            json!({
                "id": &edge.id,
                "sourceFactId": &edge.source_fact_id,
                "targetFactId": &edge.target_fact_id,
                "relation": &edge.relation,
                "metadata": compact_json_text(&edge.metadata_json),
                "createdAt": &edge.created_at,
            })
        })
        .collect::<Vec<_>>();
    let profiles_json = profiles
        .iter()
        .map(|profile| {
            json!({
                "id": &profile.id,
                "scope": &profile.scope,
                "chatId": &profile.chat_id,
                "profileText": compact_text(&profile.profile_text, 2_000),
                "updatedAt": &profile.updated_at,
            })
        })
        .collect::<Vec<_>>();
    let deterministic_json = deterministic_changes
        .iter()
        .map(|change| {
            json!({
                "operation": change.operation(),
                "targetFactIds": change.target_fact_ids(),
                "reason": change.reason(),
                "evidence": change.evidence_json(),
            })
        })
        .collect::<Vec<_>>();
    let audit_hints_json =
        memory_dream_audit_hints_json(candidates, candidate_references, deterministic_changes);

    Ok(json!({
        "job": {
            "scope": request.scope.as_str(),
            "workspaceId": request.workspace_id,
            "triggerType": request.trigger_type.as_str(),
            "mode": request.mode.as_str(),
            "maxChangesPerRun": request.settings.max_changes_per_run,
        },
        "memoryProfiles": profiles_json,
        "candidateFacts": candidates_json,
        "relevantEdges": edges_json,
        "referenceValidation": {
            "records": memory_references_json(candidate_references),
            "validCount": candidate_references.iter().filter(|reference| reference.status == MemoryReferenceStatus::Valid.as_str()).count(),
            "invalidCount": candidate_references.iter().filter(|reference| reference.status == MemoryReferenceStatus::Invalid.as_str()).count(),
            "ambiguousCount": candidate_references.iter().filter(|reference| reference.status == MemoryReferenceStatus::Ambiguous.as_str()).count(),
            "skippedCount": candidate_references.iter().filter(|reference| reference.status == MemoryReferenceStatus::Skipped.as_str()).count(),
            "rule": "Do not expire or reject a memory solely because one path reference is invalid."
        },
        "sourceSummaries": source_summaries,
        "auditHints": audit_hints_json,
        "deterministicValidation": {
            "changesProposed": deterministic_json,
            "changesProposedCount": deterministic_changes.len(),
        }
    }))
}

fn memory_dream_audit_hints_json(
    candidates: &[MemoryFactRecord],
    candidate_references: &[MemoryReferenceRecord],
    deterministic_changes: &[DreamChange],
) -> Value {
    let duplicate_groups = duplicate_fact_groups(candidates)
        .into_values()
        .take(20)
        .map(|facts| {
            let first = facts.first();
            json!({
                "factIds": facts.iter().map(|fact| fact.id.as_str()).collect::<Vec<_>>(),
                "scope": first.map(|fact| fact.scope.as_str()),
                "kind": first.map(|fact| fact.kind.as_str()),
                "reason": "normalized duplicate text",
            })
        })
        .collect::<Vec<_>>();
    let pending_transitions = deterministic_changes
        .iter()
        .filter_map(|change| match change {
            DreamChange::ActivatePending { fact } => Some(json!({
                "operation": "activate",
                "factId": &fact.id,
                "confidence": fact.confidence,
                "reason": change.reason(),
            })),
            DreamChange::RejectPending { fact } => Some(json!({
                "operation": "reject",
                "factId": &fact.id,
                "confidence": fact.confidence,
                "reason": change.reason(),
            })),
            _ => None,
        })
        .take(30)
        .collect::<Vec<_>>();
    let mut references_by_fact: BTreeMap<&str, Vec<&MemoryReferenceRecord>> = BTreeMap::new();
    for reference in candidate_references.iter().filter(|reference| {
        reference.status == MemoryReferenceStatus::Invalid.as_str()
            || reference.status == MemoryReferenceStatus::Ambiguous.as_str()
            || reference.status == MemoryReferenceStatus::Skipped.as_str()
    }) {
        references_by_fact
            .entry(reference.fact_id.as_str())
            .or_default()
            .push(reference);
    }
    let reference_issues = references_by_fact
        .into_iter()
        .take(30)
        .map(|(fact_id, references)| {
            json!({
                "factId": fact_id,
                "references": references
                    .into_iter()
                    .take(5)
                    .map(memory_reference_json)
                    .collect::<Vec<_>>(),
                "rule": "Do not expire or reject a fact solely because one reference is invalid.",
            })
        })
        .collect::<Vec<_>>();
    let update_chain_issues = deterministic_changes
        .iter()
        .filter_map(|change| match change {
            DreamChange::RepairUpdatesChain { source, target } => Some(json!({
                "sourceFactId": &source.id,
                "targetFactId": &target.id,
                "reason": change.reason(),
            })),
            _ => None,
        })
        .take(20)
        .collect::<Vec<_>>();

    json!({
        "duplicateGroups": duplicate_groups,
        "pendingTransitions": pending_transitions,
        "referenceIssues": reference_issues,
        "updateChainIssues": update_chain_issues,
        "rule": "Audit hints are advisory; every proposed LLM change still needs explicit evidence from the provided records.",
    })
}

async fn request_memory_dream_planner_output(
    planner: MemoryDreamPlannerRequest<'_>,
    selection: &MemoryDreamModelSelection,
    input: Value,
) -> Result<MemoryDreamPlannerOutput, ApiError> {
    let mut request = memory_dream_planner_provider_request(selection, &input)?;
    let first_value =
        call_memory_dream_planner_provider(planner, selection, request.clone()).await?;
    match parse_memory_dream_planner_output(first_value.clone()) {
        Ok(output) => Ok(output),
        Err(first_error) => {
            request.messages.push(neutral_text_message(
                NeutralChatRole::Assistant,
                serde_json::to_string(&first_value).unwrap_or_else(|_| "{}".to_string()),
            ));
            request.messages.push(neutral_text_message(
                NeutralChatRole::User,
                format!(
                    "The previous {MEMORY_DREAM_PLANNER_TOOL_NAME} arguments were malformed: {}. Return corrected tool JSON once.",
                    first_error.message
                ),
            ));
            let repaired_value =
                call_memory_dream_planner_provider(planner, selection, request).await?;
            parse_memory_dream_planner_output(repaired_value).map_err(|source| {
                ApiError::bad_request(format!(
                    "malformed memory Dream planner JSON after repair: {}",
                    source.message
                ))
            })
        }
    }
}

fn memory_dream_planner_provider_request(
    selection: &MemoryDreamModelSelection,
    input: &Value,
) -> Result<NeutralChatRequest, ApiError> {
    let input_json = serde_json::to_string_pretty(input).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize memory Dream planner input: {source}"
        ))
    })?;

    Ok(NeutralChatRequest {
        model_id: selection.model_id.clone(),
        messages: vec![
            neutral_text_message(
                NeutralChatRole::System,
                MEMORY_DREAM_PLANNER_SYSTEM_PROMPT.to_string(),
            ),
            neutral_text_message(
                NeutralChatRole::User,
                format!("Memory Dream compact input JSON:\n{input_json}"),
            ),
        ],
        tools: vec![memory_dream_planner_tool_definition()],
        thinking_level: None,
        max_output_tokens: Some(selection.max_output_tokens),
        prompt_cache_key: None,
        prompt_cache_retention: None,
    })
}

async fn call_memory_dream_planner_provider(
    planner: MemoryDreamPlannerRequest<'_>,
    selection: &MemoryDreamModelSelection,
    request: NeutralChatRequest,
) -> Result<Value, ApiError> {
    audited_provider_tool_request(
        planner.workspace_path,
        planner.audit_workspace_id,
        planner.audit_chat_id,
        &selection.provider_id,
        &selection.provider_config,
        request,
        "memory Dream planner",
        MEMORY_DREAM_PLANNER_TOOL_NAME,
        "submit changeset tool",
        planner.config.memory.dream.llm_timeout_ms,
        planner.config.app.llm_request_retry_count,
        api_audit_save_details(planner.config),
    )
    .await
}

fn memory_dream_planner_tool_definition() -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: MEMORY_DREAM_PLANNER_TOOL_NAME.to_string(),
        description: "Submit a conservative Foco memory Dream changeset.".to_string(),
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "summary": { "type": "string" },
                "changes": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "operation": {
                                "type": "string",
                                "enum": ["update", "supersede", "expire", "merge", "reject", "promote_to_global", "add_edge"]
                            },
                            "targetFactIds": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "newFact": {
                                "anyOf": [
                                    {
                                        "type": "object",
                                        "additionalProperties": false,
                                        "properties": {
                                            "fact": { "type": ["string", "null"] },
                                            "kind": { "type": ["string", "null"] },
                                            "confidence": { "type": ["number", "null"], "minimum": 0, "maximum": 1 },
                                            "relation": { "type": ["string", "null"] },
                                            "expiresAt": { "type": ["string", "null"] }
                                        },
                                        "required": ["fact", "kind", "confidence", "relation", "expiresAt"]
                                    },
                                    { "type": "null" }
                                ]
                            },
                            "reason": { "type": "string" },
                            "confidence": { "type": ["number", "null"], "minimum": 0, "maximum": 1 },
                            "riskLevel": { "type": "string", "enum": ["low", "medium", "high"] },
                            "evidence": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "properties": {
                                        "sourceType": {
                                            "type": "string",
                                            "enum": ["memory_fact", "memory_source", "chat_summary", "edge", "profile"]
                                        },
                                        "sourceId": { "type": "string" },
                                        "quote": { "type": "string" }
                                    },
                                    "required": ["sourceType", "sourceId", "quote"]
                                }
                            }
                        },
                        "required": ["operation", "targetFactIds", "newFact", "reason", "confidence", "riskLevel", "evidence"]
                    }
                }
            },
            "required": ["summary", "changes"]
        }),
        strict: true,
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MemoryDreamPlannerOutput {
    summary: String,
    changes: Vec<MemoryDreamPlannerChange>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MemoryDreamPlannerChange {
    operation: String,
    target_fact_ids: Vec<String>,
    new_fact: Option<DreamPlannerNewFact>,
    reason: String,
    confidence: Option<f64>,
    risk_level: String,
    evidence: Vec<DreamPlannerEvidence>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DreamPlannerNewFact {
    fact: Option<String>,
    kind: Option<String>,
    confidence: Option<f64>,
    relation: Option<String>,
    expires_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DreamPlannerEvidence {
    source_type: String,
    source_id: String,
    quote: String,
}

#[derive(Clone, Debug)]
struct ValidatedDreamPlannerChange {
    operation: DreamPlannerOperation,
    target_facts: Vec<MemoryFactRecord>,
    new_fact: Option<DreamPlannerNewFact>,
    reason: String,
    confidence: Option<f64>,
    risk_level: String,
    evidence: Vec<DreamPlannerEvidence>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DreamPlannerOperation {
    Update,
    Supersede,
    Expire,
    Merge,
    Reject,
    PromoteToGlobal,
    AddEdge,
}

impl DreamPlannerOperation {
    fn parse(value: &str) -> Result<Self, ApiError> {
        match value.trim() {
            "update" => Ok(Self::Update),
            "supersede" => Ok(Self::Supersede),
            "expire" => Ok(Self::Expire),
            "merge" => Ok(Self::Merge),
            "reject" => Ok(Self::Reject),
            "promote_to_global" => Ok(Self::PromoteToGlobal),
            "add_edge" => Ok(Self::AddEdge),
            other => Err(ApiError::bad_request(format!(
                "memory Dream planner returned unknown operation '{other}'"
            ))),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Update => "update",
            Self::Supersede => "supersede",
            Self::Expire => "expire",
            Self::Merge => "merge",
            Self::Reject => "reject",
            Self::PromoteToGlobal => "promote_to_global",
            Self::AddEdge => "add_edge",
        }
    }
}

fn parse_memory_dream_planner_output(value: Value) -> Result<MemoryDreamPlannerOutput, ApiError> {
    serde_json::from_value(value).map_err(|source| {
        ApiError::bad_request(format!("malformed memory Dream planner JSON: {source}"))
    })
}

fn validate_memory_dream_planner_output(
    output: &MemoryDreamPlannerOutput,
    scope: MemoryDreamScope,
    candidates: &[MemoryFactRecord],
    policy: &MemoryDreamSafetyPolicy,
) -> Result<Vec<ValidatedDreamPlannerChange>, ApiError> {
    if output.changes.len() > policy.max_changes_per_run {
        return Err(ApiError::bad_request(format!(
            "memory Dream planner returned {} changes; max is {}",
            output.changes.len(),
            policy.max_changes_per_run
        )));
    }
    let candidates_by_id = candidates
        .iter()
        .map(|fact| (fact.id.as_str(), fact))
        .collect::<HashMap<_, _>>();
    let mut validated = Vec::new();

    for (index, change) in output.changes.iter().enumerate() {
        let operation = DreamPlannerOperation::parse(&change.operation)?;
        if change.target_fact_ids.is_empty() {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner change {index} must target at least one fact"
            )));
        }
        let mut target_facts = Vec::new();
        for target_id in &change.target_fact_ids {
            let target_id = target_id.trim();
            let fact = candidates_by_id.get(target_id).ok_or_else(|| {
                ApiError::bad_request(format!(
                    "memory Dream planner change {index} references non-candidate fact id '{target_id}'"
                ))
            })?;
            target_facts.push((*fact).clone());
        }
        if operation == DreamPlannerOperation::Merge && target_facts.len() < 2 {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner merge change {index} must target at least two facts"
            )));
        }
        if operation == DreamPlannerOperation::AddEdge && target_facts.len() != 2 {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner add_edge change {index} must target exactly two facts"
            )));
        }
        if change.reason.trim().is_empty() {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner change {index} reason must not be empty"
            )));
        }
        if let Some(confidence) = change.confidence
            && !(0.0..=1.0).contains(&confidence)
        {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner change {index} confidence must be between 0 and 1"
            )));
        }
        if !matches!(change.risk_level.as_str(), "low" | "medium" | "high") {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner change {index} riskLevel is unsupported"
            )));
        }
        let has_cross_project_evidence = validate_planner_evidence(index, &change.evidence)?;
        validate_operation_payload(
            index,
            operation,
            change.new_fact.as_ref(),
            scope,
            &target_facts,
            &change.reason,
            &change.evidence,
            has_cross_project_evidence,
            policy,
        )?;

        validated.push(ValidatedDreamPlannerChange {
            operation,
            target_facts,
            new_fact: change.new_fact.clone(),
            reason: change.reason.trim().to_string(),
            confidence: change.confidence,
            risk_level: change.risk_level.clone(),
            evidence: change.evidence.clone(),
        });
    }

    Ok(validated)
}

fn validate_planner_evidence(
    index: usize,
    evidence: &[DreamPlannerEvidence],
) -> Result<bool, ApiError> {
    if evidence.is_empty() {
        return Err(ApiError::bad_request(format!(
            "memory Dream planner change {index} must include evidence"
        )));
    }
    let mut has_cross_project_evidence = false;
    for item in evidence {
        if !matches!(
            item.source_type.as_str(),
            "memory_fact" | "memory_source" | "chat_summary" | "edge" | "profile"
        ) {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner change {index} has unsupported evidence sourceType '{}'",
                item.source_type
            )));
        }
        if item.source_id.trim().is_empty() || item.quote.trim().is_empty() {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner change {index} evidence must include sourceId and quote"
            )));
        }
        let quote = item.quote.to_ascii_lowercase();
        if quote.contains("cross-project")
            || quote.contains("user-wide")
            || quote.contains("all projects")
            || quote.contains("across workspaces")
            || quote.contains("global preference")
        {
            has_cross_project_evidence = true;
        }
    }

    Ok(has_cross_project_evidence)
}

fn validate_operation_payload(
    index: usize,
    operation: DreamPlannerOperation,
    new_fact: Option<&DreamPlannerNewFact>,
    scope: MemoryDreamScope,
    target_facts: &[MemoryFactRecord],
    reason: &str,
    evidence: &[DreamPlannerEvidence],
    has_cross_project_evidence: bool,
    policy: &MemoryDreamSafetyPolicy,
) -> Result<(), ApiError> {
    if scope == MemoryDreamScope::Global {
        validate_global_planner_operation(index, operation, target_facts)?;
    }

    match operation {
        DreamPlannerOperation::Update => {
            let fact = new_fact
                .and_then(|new_fact| new_fact.fact.as_deref())
                .and_then(non_empty_trimmed)
                .ok_or_else(|| {
                    ApiError::bad_request(format!(
                        "memory Dream planner update change {index} requires newFact.fact"
                    ))
                })?;
            if fact.len() > 4_000 {
                return Err(ApiError::bad_request(format!(
                    "memory Dream planner update change {index} newFact.fact is too long"
                )));
            }
        }
        DreamPlannerOperation::AddEdge => {
            let relation = new_fact
                .and_then(|new_fact| new_fact.relation.as_deref())
                .and_then(non_empty_trimmed)
                .ok_or_else(|| {
                    ApiError::bad_request(format!(
                        "memory Dream planner add_edge change {index} requires newFact.relation"
                    ))
                })?;
            relation_kind_from_str(relation).map_err(ApiError::from_memory_error)?;
        }
        DreamPlannerOperation::PromoteToGlobal => {
            if scope != MemoryDreamScope::Workspace {
                return Err(ApiError::bad_request(
                    "memory Dream planner can only promote workspace candidates to global",
                ));
            }
            validate_global_promotion_candidate(
                index,
                target_facts,
                new_fact,
                reason,
                evidence,
                has_cross_project_evidence,
                policy,
            )?;
        }
        DreamPlannerOperation::Supersede
        | DreamPlannerOperation::Expire
        | DreamPlannerOperation::Merge
        | DreamPlannerOperation::Reject => {}
    }

    Ok(())
}

fn validate_global_planner_operation(
    index: usize,
    operation: DreamPlannerOperation,
    target_facts: &[MemoryFactRecord],
) -> Result<(), ApiError> {
    match operation {
        DreamPlannerOperation::Expire => {
            let now = Utc::now();
            if target_facts.iter().any(|fact| {
                !fact
                    .expires_at
                    .as_deref()
                    .is_some_and(|expires_at| timestamp_is_due(expires_at, now))
            }) {
                return Err(ApiError::bad_request(format!(
                    "global memory Dream expire change {index} can only target due expired facts"
                )));
            }
        }
        DreamPlannerOperation::Merge => {
            let Some(first) = target_facts.first() else {
                return Ok(());
            };
            let first_text = normalized_duplicate_text(&first.fact);
            let first_kind = &first.kind;
            if target_facts.iter().any(|fact| {
                normalized_duplicate_text(&fact.fact) != first_text || &fact.kind != first_kind
            }) {
                return Err(ApiError::bad_request(format!(
                    "global memory Dream merge change {index} can only merge exact duplicates"
                )));
            }
        }
        DreamPlannerOperation::Update
        | DreamPlannerOperation::Supersede
        | DreamPlannerOperation::Reject
        | DreamPlannerOperation::PromoteToGlobal
        | DreamPlannerOperation::AddEdge => {
            return Err(ApiError::bad_request(format!(
                "global memory Dream planner operation '{}' is not conservative enough",
                operation.as_str()
            )));
        }
    }

    Ok(())
}

fn validate_global_promotion_candidate(
    index: usize,
    target_facts: &[MemoryFactRecord],
    new_fact: Option<&DreamPlannerNewFact>,
    reason: &str,
    evidence: &[DreamPlannerEvidence],
    has_cross_project_evidence: bool,
    policy: &MemoryDreamSafetyPolicy,
) -> Result<(), ApiError> {
    if !policy.allows_automatic_global_promotion(has_cross_project_evidence) {
        return Err(ApiError::bad_request(
            "memory Dream planner global promotion requires explicit cross-project evidence",
        ));
    }

    for fact in target_facts {
        if MemoryScope::parse(&fact.scope).map_err(ApiError::from_memory_error)?
            == MemoryScope::Global
        {
            return Err(ApiError::bad_request(
                "memory Dream planner can only promote workspace or chat candidates",
            ));
        }
        if fact.status != MemoryStatus::Active.as_str() || !fact.is_latest {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner promote_to_global change {index} can only promote active latest facts"
            )));
        }
        if MemoryKind::parse(&fact.kind).map_err(ApiError::from_memory_error)?
            != MemoryKind::Preference
        {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner promote_to_global change {index} only supports preference facts"
            )));
        }
        if text_mentions_project_specific_path(&fact.fact) {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner promote_to_global change {index} rejects project-specific file path evidence"
            )));
        }
    }

    if let Some(new_fact) = new_fact {
        if let Some(kind) = new_fact.kind.as_deref().and_then(non_empty_trimmed)
            && MemoryKind::parse(kind).map_err(ApiError::from_memory_error)?
                != MemoryKind::Preference
        {
            return Err(ApiError::bad_request(format!(
                "memory Dream planner promote_to_global change {index} only supports preference facts"
            )));
        }
        if let Some(fact) = new_fact.fact.as_deref().and_then(non_empty_trimmed) {
            if fact.len() > 4_000 {
                return Err(ApiError::bad_request(format!(
                    "memory Dream planner promote_to_global change {index} newFact.fact is too long"
                )));
            }
            if text_mentions_project_specific_path(fact) {
                return Err(ApiError::bad_request(format!(
                    "memory Dream planner promote_to_global change {index} rejects project-specific file path evidence"
                )));
            }
        }
    }

    if text_mentions_project_specific_path(reason)
        || evidence
            .iter()
            .any(|item| text_mentions_project_specific_path(&item.quote))
    {
        return Err(ApiError::bad_request(format!(
            "memory Dream planner promote_to_global change {index} rejects project-specific file path evidence"
        )));
    }

    Ok(())
}

fn relation_kind_from_str(value: &str) -> Result<MemoryRelationKind, MemoryDatabaseError> {
    match value.trim() {
        "updates" => Ok(MemoryRelationKind::Updates),
        "extends" => Ok(MemoryRelationKind::Extends),
        "derives" => Ok(MemoryRelationKind::Derives),
        other => Err(MemoryDatabaseError::InvalidMemoryInput {
            message: format!("unknown memory relation kind: {other}"),
        }),
    }
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

fn high_confidence_pending_is_promotable(
    database: &MemoryDatabase,
    fact: &MemoryFactRecord,
    now: DateTime<Utc>,
) -> Result<bool, MemoryDatabaseError> {
    if fact.status != MemoryStatus::Pending.as_str()
        || fact.kind == MemoryKind::UserNote.as_str()
        || fact.confidence.unwrap_or(0.0) < HIGH_CONFIDENCE_PENDING_THRESHOLD
        || fact
            .expires_at
            .as_deref()
            .is_some_and(|expires_at| timestamp_is_due(expires_at, now))
    {
        return Ok(false);
    }

    let sources = database.sources_for_fact(&fact.id)?;
    if sources.is_empty()
        || !sources.iter().any(|source| {
            matches!(
                source.source_type.as_str(),
                "chat_message" | "tool_call" | "tool_result" | "context_snapshot" | "manual_note"
            )
        })
    {
        return Ok(false);
    }

    let references = database.references_for_fact_ids(std::slice::from_ref(&fact.id), 16)?;
    Ok(references
        .iter()
        .all(|reference| reference.status == MemoryReferenceStatus::Valid.as_str()))
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
        DreamChange::ActivatePending { fact } => {
            let current = current_unchanged_fact(database, fact, policy)?;
            let updated = database.update_fact(UpdateMemoryFact {
                id: &fact.id,
                status: Some(MemoryStatus::Active),
                is_latest: Some(true),
                ..UpdateMemoryFact::default()
            })?;
            if !updated {
                return Err(missing_fact(&fact.id));
            }
            let after = database
                .fact(&fact.id)?
                .ok_or_else(|| missing_fact(&fact.id))?;
            insert_applied_change_with_reason(
                database,
                job_id,
                change,
                &current,
                &after,
                "activated high-confidence pending memory fact with supporting evidence",
            )?;
            Ok(AppliedChange {
                scope: MemoryScope::parse(&after.scope)?,
                chat_id: after.chat_id,
            })
        }
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

fn apply_llm_changes(
    database: &mut MemoryDatabase,
    job_id: &str,
    changes: &[ValidatedDreamPlannerChange],
    policy: &MemoryDreamSafetyPolicy,
    workspace_id: Option<&str>,
    global_memory_database_file: Option<&Path>,
) -> Result<ApplySummary, MemoryDatabaseError> {
    let mut summary = ApplySummary::default();
    for change in changes {
        match apply_llm_change(
            database,
            job_id,
            change,
            policy,
            workspace_id,
            global_memory_database_file,
        ) {
            Ok(applied_scopes) => {
                summary.applied += 1;
                for changed_scope in applied_scopes {
                    if !summary.changed_scopes.contains(&changed_scope) {
                        summary.changed_scopes.push(changed_scope);
                    }
                }
            }
            Err(error) => {
                summary.failed += 1;
                insert_failed_llm_change(database, job_id, change, &error.to_string())?;
            }
        }
    }

    Ok(summary)
}

fn apply_llm_change(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &ValidatedDreamPlannerChange,
    policy: &MemoryDreamSafetyPolicy,
    workspace_id: Option<&str>,
    global_memory_database_file: Option<&Path>,
) -> Result<Vec<(MemoryScope, Option<String>)>, MemoryDatabaseError> {
    match change.operation {
        DreamPlannerOperation::Expire => {
            apply_llm_status_change(database, job_id, change, MemoryStatus::Expired, policy)
        }
        DreamPlannerOperation::Supersede => {
            apply_llm_status_change(database, job_id, change, MemoryStatus::Superseded, policy)
        }
        DreamPlannerOperation::Reject => {
            apply_llm_status_change(database, job_id, change, MemoryStatus::Rejected, policy)
        }
        DreamPlannerOperation::Update => apply_llm_update(database, job_id, change, policy),
        DreamPlannerOperation::Merge => apply_llm_merge(database, job_id, change, policy),
        DreamPlannerOperation::AddEdge => apply_llm_add_edge(database, job_id, change),
        DreamPlannerOperation::PromoteToGlobal => apply_llm_promote_to_global(
            database,
            job_id,
            change,
            policy,
            workspace_id,
            global_memory_database_file,
        ),
    }
}

fn apply_llm_status_change(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &ValidatedDreamPlannerChange,
    status: MemoryStatus,
    policy: &MemoryDreamSafetyPolicy,
) -> Result<Vec<(MemoryScope, Option<String>)>, MemoryDatabaseError> {
    let mut changed_scopes = Vec::new();
    for fact in &change.target_facts {
        let current = current_unchanged_fact(database, fact, policy)?;
        if status == MemoryStatus::Expired {
            let kind = MemoryKind::parse(&current.kind)?;
            if !policy.allows_direct_expiration(kind, current.pinned, false, true) {
                return Err(MemoryDatabaseError::InvalidMemoryInput {
                    message: "memory Dream LLM cannot directly expire pinned or user_note facts"
                        .to_string(),
                });
            }
        }
        update_fact_status(database, &fact.id, status)?;
        let after = database
            .fact(&fact.id)?
            .ok_or_else(|| missing_fact(&fact.id))?;
        insert_applied_llm_change(database, job_id, change, Some(&current), Some(&after))?;
        changed_scopes.push((MemoryScope::parse(&after.scope)?, after.chat_id));
    }

    Ok(changed_scopes)
}

fn apply_llm_update(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &ValidatedDreamPlannerChange,
    policy: &MemoryDreamSafetyPolicy,
) -> Result<Vec<(MemoryScope, Option<String>)>, MemoryDatabaseError> {
    let Some(new_fact) = change.new_fact.as_ref() else {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "update change requires newFact".to_string(),
        });
    };
    let fact_text = new_fact
        .fact
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
            message: "update change requires newFact.fact".to_string(),
        })?;
    let kind = match new_fact.kind.as_deref().and_then(non_empty_trimmed) {
        Some(kind) => Some(MemoryKind::parse(kind)?),
        None => None,
    };
    let mut changed_scopes = Vec::new();

    for fact in &change.target_facts {
        let current = current_unchanged_fact(database, fact, policy)?;
        let updated = database.update_fact(UpdateMemoryFact {
            id: &fact.id,
            kind,
            fact: Some(fact_text),
            confidence: new_fact.confidence,
            expires_at: new_fact.expires_at.as_deref().and_then(non_empty_trimmed),
            ..UpdateMemoryFact::default()
        })?;
        if !updated {
            return Err(missing_fact(&fact.id));
        }
        let after = database
            .fact(&fact.id)?
            .ok_or_else(|| missing_fact(&fact.id))?;
        insert_applied_llm_change(database, job_id, change, Some(&current), Some(&after))?;
        changed_scopes.push((MemoryScope::parse(&after.scope)?, after.chat_id));
    }

    Ok(changed_scopes)
}

fn apply_llm_merge(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &ValidatedDreamPlannerChange,
    policy: &MemoryDreamSafetyPolicy,
) -> Result<Vec<(MemoryScope, Option<String>)>, MemoryDatabaseError> {
    let Some((winner, losers)) = change.target_facts.split_first() else {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "merge change requires target facts".to_string(),
        });
    };
    let mut changed_scopes = Vec::new();

    for loser in losers {
        let current = current_unchanged_fact(database, loser, policy)?;
        for source in database.sources_for_fact(&loser.id)? {
            database.link_fact_source(&winner.id, &source.id)?;
        }
        database.insert_edge(NewMemoryEdge {
            id: &unique_id("memory-edge"),
            source_fact_id: &winner.id,
            target_fact_id: &loser.id,
            relation: MemoryRelationKind::Derives,
            metadata_json: &json!({
                "operation": "memory_dream_llm_merge",
                "dreamJobId": job_id
            })
            .to_string(),
        })?;
        update_fact_status(database, &loser.id, MemoryStatus::Superseded)?;
        let after = database
            .fact(&loser.id)?
            .ok_or_else(|| missing_fact(&loser.id))?;
        insert_applied_llm_change(database, job_id, change, Some(&current), Some(&after))?;
        changed_scopes.push((MemoryScope::parse(&after.scope)?, after.chat_id));
    }

    Ok(changed_scopes)
}

fn apply_llm_add_edge(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &ValidatedDreamPlannerChange,
) -> Result<Vec<(MemoryScope, Option<String>)>, MemoryDatabaseError> {
    let Some(new_fact) = change.new_fact.as_ref() else {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "add_edge change requires newFact".to_string(),
        });
    };
    let relation = new_fact
        .relation
        .as_deref()
        .map(relation_kind_from_str)
        .transpose()?
        .ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
            message: "add_edge change requires newFact.relation".to_string(),
        })?;
    let [source, target] = change.target_facts.as_slice() else {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "add_edge change requires exactly two target facts".to_string(),
        });
    };

    database.insert_edge(NewMemoryEdge {
        id: &unique_id("memory-edge"),
        source_fact_id: &source.id,
        target_fact_id: &target.id,
        relation,
        metadata_json: &json!({
            "operation": "memory_dream_llm_add_edge",
            "dreamJobId": job_id,
            "reason": &change.reason,
        })
        .to_string(),
    })?;
    insert_applied_llm_change(database, job_id, change, None, None)?;

    Ok(vec![(
        MemoryScope::parse(&source.scope)?,
        source.chat_id.clone(),
    )])
}

fn apply_llm_promote_to_global(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &ValidatedDreamPlannerChange,
    policy: &MemoryDreamSafetyPolicy,
    workspace_id: Option<&str>,
    global_memory_database_file: Option<&Path>,
) -> Result<Vec<(MemoryScope, Option<String>)>, MemoryDatabaseError> {
    let workspace_id = workspace_id.ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
        message: "promote_to_global requires workspace_id".to_string(),
    })?;
    let global_memory_database_file =
        global_memory_database_file.ok_or_else(|| MemoryDatabaseError::InvalidMemoryInput {
            message: "promote_to_global requires global memory database path".to_string(),
        })?;
    let current_facts = change
        .target_facts
        .iter()
        .map(|fact| current_unchanged_fact(database, fact, policy))
        .collect::<Result<Vec<_>, _>>()?;
    let Some(primary_fact) = current_facts.first() else {
        return Err(MemoryDatabaseError::InvalidMemoryInput {
            message: "promote_to_global requires target facts".to_string(),
        });
    };
    let promoted_fact_id = unique_id("memory-fact");
    let source_fact_ids = current_facts
        .iter()
        .map(|fact| fact.id.clone())
        .collect::<Vec<_>>();
    let fact_text = change
        .new_fact
        .as_ref()
        .and_then(|new_fact| new_fact.fact.as_deref())
        .and_then(non_empty_trimmed)
        .unwrap_or(primary_fact.fact.as_str());
    let confidence = change
        .new_fact
        .as_ref()
        .and_then(|new_fact| new_fact.confidence)
        .or(change.confidence)
        .or(primary_fact.confidence);
    let expires_at = change
        .new_fact
        .as_ref()
        .and_then(|new_fact| new_fact.expires_at.as_deref())
        .and_then(non_empty_trimmed);
    let fact_metadata_json = promoted_fact_metadata_json(
        &primary_fact.metadata_json,
        job_id,
        workspace_id,
        &source_fact_ids,
        &change.evidence,
    )?;
    let mut global_database =
        MemoryDatabase::open_or_create_global_at(global_memory_database_file)?;
    let mut promoted_source_ids = Vec::new();

    for fact in &current_facts {
        let sources = database.sources_for_fact(&fact.id)?;
        if sources.is_empty() {
            let source_id = unique_id("memory-source");
            let source_metadata_json =
                promoted_source_metadata_json("{}", job_id, workspace_id, &fact.id)?;
            global_database.insert_source(NewMemorySource {
                id: &source_id,
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::ManualNote,
                source_id: Some(&fact.id),
                title: "Memory Dream promotion origin",
                content: &fact.fact,
                metadata_json: &source_metadata_json,
            })?;
            promoted_source_ids.push(source_id);
            continue;
        }

        for source in sources {
            let source_id = unique_id("memory-source");
            let source_metadata_json = promoted_source_metadata_json(
                &source.metadata_json,
                job_id,
                workspace_id,
                &fact.id,
            )?;
            global_database.insert_source(NewMemorySource {
                id: &source_id,
                scope: MemoryScope::Global,
                chat_id: None,
                source_type: MemorySourceType::parse(&source.source_type)?,
                source_id: source.source_id.as_deref().or(Some(fact.id.as_str())),
                title: &source.title,
                content: &source.content,
                metadata_json: &source_metadata_json,
            })?;
            promoted_source_ids.push(source_id);
        }
    }

    let promoted_source_refs = promoted_source_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    global_database.insert_fact(NewMemoryFact {
        id: &promoted_fact_id,
        scope: MemoryScope::Global,
        chat_id: None,
        status: MemoryStatus::Active,
        kind: MemoryKind::Preference,
        fact: fact_text,
        confidence,
        pinned: false,
        source_ids: &promoted_source_refs,
        metadata_json: &fact_metadata_json,
    })?;
    if let Some(expires_at) = expires_at {
        global_database.update_fact(UpdateMemoryFact {
            id: &promoted_fact_id,
            expires_at: Some(expires_at),
            ..UpdateMemoryFact::default()
        })?;
    }
    let promoted_fact = global_database
        .fact(&promoted_fact_id)?
        .ok_or_else(|| missing_fact(&promoted_fact_id))?;
    global_database.refresh_profile_from_active_facts(
        MemoryScope::Global,
        None,
        MEMORY_PROFILE_REFRESH_FACT_LIMIT,
    )?;
    insert_applied_llm_change_with_new_fact(
        database,
        job_id,
        change,
        Some(primary_fact),
        Some(&promoted_fact),
        Some(&promoted_fact_id),
    )?;

    Ok(vec![(MemoryScope::Global, None)])
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

fn insert_applied_llm_change(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &ValidatedDreamPlannerChange,
    before: Option<&MemoryFactRecord>,
    after: Option<&MemoryFactRecord>,
) -> Result<(), MemoryDatabaseError> {
    insert_applied_llm_change_with_new_fact(database, job_id, change, before, after, None)
}

fn insert_applied_llm_change_with_new_fact(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &ValidatedDreamPlannerChange,
    before: Option<&MemoryFactRecord>,
    after: Option<&MemoryFactRecord>,
    new_fact_id: Option<&str>,
) -> Result<(), MemoryDatabaseError> {
    let target_fact_ids_json = json!(llm_change_target_fact_ids(change)).to_string();
    let before_json =
        before.map(|fact| serde_json::to_string(fact).expect("memory fact record serializes"));
    let after_json =
        after.map(|fact| serde_json::to_string(fact).expect("memory fact record serializes"));
    let evidence_json = serde_json::to_string(&change.evidence).map_err(|source| {
        MemoryDatabaseError::InvalidMemoryJson {
            field: "memory_dream_changes.evidence_json",
            source,
        }
    })?;

    database.insert_dream_change(NewMemoryDreamChange {
        id: &unique_id("memory-dream-change"),
        job_id,
        operation: change.operation.as_str(),
        target_fact_ids_json: &target_fact_ids_json,
        new_fact_id,
        before_json: before_json.as_deref(),
        after_json: after_json.as_deref(),
        reason: &change.reason,
        confidence: change.confidence,
        risk_level: &change.risk_level,
        status: MemoryDreamChangeStatus::Applied,
        evidence_json: &evidence_json,
        error_message: None,
    })
}

fn insert_failed_llm_change(
    database: &mut MemoryDatabase,
    job_id: &str,
    change: &ValidatedDreamPlannerChange,
    error_message: &str,
) -> Result<(), MemoryDatabaseError> {
    let target_fact_ids_json = json!(llm_change_target_fact_ids(change)).to_string();
    let before_json = change
        .target_facts
        .first()
        .map(|fact| serde_json::to_string(fact).expect("memory fact record serializes"));
    let evidence_json = serde_json::to_string(&change.evidence).map_err(|source| {
        MemoryDatabaseError::InvalidMemoryJson {
            field: "memory_dream_changes.evidence_json",
            source,
        }
    })?;

    database.insert_dream_change(NewMemoryDreamChange {
        id: &unique_id("memory-dream-change"),
        job_id,
        operation: change.operation.as_str(),
        target_fact_ids_json: &target_fact_ids_json,
        new_fact_id: None,
        before_json: before_json.as_deref(),
        after_json: None,
        reason: &change.reason,
        confidence: change.confidence,
        risk_level: &change.risk_level,
        status: MemoryDreamChangeStatus::Failed,
        evidence_json: &evidence_json,
        error_message: Some(error_message),
    })
}

fn llm_change_target_fact_ids(change: &ValidatedDreamPlannerChange) -> Vec<&str> {
    change
        .target_facts
        .iter()
        .map(|fact| fact.id.as_str())
        .collect()
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

fn text_mentions_project_specific_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains(":\\")
        || lower.contains("\\\\")
        || lower.contains(".foco")
        || lower.contains(".git")
    {
        return true;
    }

    // ponytail: lexical path detection is enough for Phase 9; Phase 11 can replace it with parsed references.
    for token in lower.split(|character: char| {
        character.is_whitespace()
            || matches!(
                character,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
            )
    }) {
        let token = token
            .trim_matches(|character: char| matches!(character, '.' | ':' | '!' | '?' | '<' | '>'));
        if token.starts_with("http://") || token.starts_with("https://") {
            continue;
        }
        if token.contains('/') || token.contains('\\') {
            return true;
        }
        if matches!(
            token,
            "cargo.toml"
                | "package.json"
                | "pnpm-lock.yaml"
                | "package-lock.json"
                | "tsconfig.json"
                | "vite.config.ts"
                | "auto-dream.md"
                | "agents.md"
        ) {
            return true;
        }
    }

    false
}

fn backtick_fragments(value: &str) -> Vec<String> {
    let mut fragments = Vec::new();
    let mut start = None;
    for (index, character) in value.char_indices() {
        if character != '`' {
            continue;
        }
        if let Some(fragment_start) = start.take() {
            if fragment_start < index {
                fragments.push(value[fragment_start..index].to_string());
            }
        } else {
            start = Some(index + character.len_utf8());
        }
    }
    fragments
}

fn reference_tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| {
            character.is_whitespace()
                || matches!(
                    character,
                    '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
                )
        })
        .map(trimmed_reference_value)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn trimmed_reference_value(value: &str) -> &str {
    value.trim().trim_matches(|character: char| {
        matches!(
            character,
            '"' | '\'' | '`' | '.' | ':' | '!' | '?' | '<' | '>' | ',' | ';'
        )
    })
}

fn looks_like_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn looks_like_workspace_id_reference(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("workspace:")
        || lower.starts_with("workspace_id=")
        || lower.starts_with("workspaceid=")
}

fn looks_like_command_reference(value: &str) -> bool {
    let lower = normalized_command(value);
    matches!(
        lower.split_whitespace().next(),
        Some(
            "cargo"
                | "npm"
                | "pnpm"
                | "yarn"
                | "git"
                | "node"
                | "python"
                | "python3"
                | "powershell"
                | "cmd"
                | "bash"
                | "rg"
        )
    )
}

fn looks_like_file_reference(value: &str) -> bool {
    if looks_like_url(value) {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    if lower.contains('/') || lower.contains('\\') || lower.starts_with("./") {
        return true;
    }
    matches!(
        lower.as_str(),
        "cargo.toml"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "tsconfig.json"
            | "vite.config.ts"
            | "auto-dream.md"
            | "agents.md"
    ) || known_file_extension(&lower)
}

fn known_file_extension(value: &str) -> bool {
    let path = value.split(['#', '?']).next().unwrap_or(value);
    let path = strip_line_suffix(path);
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some(
            "rs" | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "py"
                | "go"
                | "java"
                | "cs"
                | "cpp"
                | "c"
                | "h"
                | "md"
                | "json"
                | "toml"
                | "yaml"
                | "yml"
                | "css"
                | "html"
                | "vue"
                | "lock"
        )
    )
}

fn looks_like_symbol_reference(value: &str) -> bool {
    let value = value.trim_end_matches("()");
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '_' | ':' | '.' | '#' | '<' | '>' | '-')
        })
        && value
            .chars()
            .any(|character| character.is_ascii_alphabetic())
}

fn normalized_reference_value(reference_type: MemoryReferenceType, value: &str) -> Option<String> {
    let trimmed = trimmed_reference_value(value);
    match reference_type {
        MemoryReferenceType::FilePath => {
            let path = strip_line_suffix(trimmed.split('#').next().unwrap_or(trimmed))
                .replace('\\', "/")
                .trim_start_matches("./")
                .to_string();
            (!path.is_empty()).then_some(path)
        }
        MemoryReferenceType::Symbol => {
            let symbol = trimmed.trim_end_matches("()").to_string();
            (!symbol.is_empty()).then_some(symbol)
        }
        MemoryReferenceType::Command => Some(normalized_command(trimmed)),
        MemoryReferenceType::Url => normalized_url_reference(trimmed),
        MemoryReferenceType::WorkspaceId => Some(workspace_id_reference_value(trimmed).to_string()),
    }
}

fn reference_value_for_storage(
    reference_type: MemoryReferenceType,
    value: &str,
    normalized_value: &str,
) -> String {
    if reference_type == MemoryReferenceType::Url {
        normalized_value.to_string()
    } else {
        trimmed_reference_value(value).to_string()
    }
}

fn strip_line_suffix(value: &str) -> &str {
    if let Some((head, tail)) = value.rsplit_once(':')
        && !head.is_empty()
        && tail.chars().all(|character| character.is_ascii_digit())
    {
        return head;
    }
    value
}

fn normalized_command(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn normalized_url_reference(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    let scheme = if lower.starts_with("https://") {
        "https"
    } else if lower.starts_with("http://") {
        "http"
    } else {
        return None;
    };
    let after_scheme = value.split_once("://")?.1;
    let without_fragment = after_scheme
        .split(['#', '?'])
        .next()
        .unwrap_or(after_scheme);
    let host_and_path = without_fragment
        .split_once('/')
        .unwrap_or((without_fragment, ""));
    let host = host_and_path
        .0
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(host_and_path.0)
        .to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    let path = host_and_path.1.trim_end_matches('/');
    if path.is_empty() {
        Some(format!("{scheme}://{host}"))
    } else {
        Some(format!("{scheme}://{host}/{path}"))
    }
}

fn url_host(value: &str) -> Option<&str> {
    value
        .split_once("://")?
        .1
        .split('/')
        .next()
        .filter(|host| !host.is_empty())
}

fn workspace_id_reference_value(value: &str) -> &str {
    value
        .split_once(':')
        .map(|(_, workspace_id)| workspace_id)
        .or_else(|| value.split_once('=').map(|(_, workspace_id)| workspace_id))
        .unwrap_or(value)
        .trim()
}

fn path_stays_inside_workspace(workspace_path: &Path, candidate: &Path) -> bool {
    let Ok(workspace_path) = workspace_path.canonicalize() else {
        return false;
    };
    let Ok(candidate) = candidate.canonicalize() else {
        return false;
    };
    candidate.starts_with(workspace_path)
}

fn moved_file_candidates(
    workspace_path: &Path,
    normalized_path: &str,
    limit: usize,
) -> Vec<String> {
    let Some(file_name) = Path::new(normalized_path)
        .file_name()
        .and_then(|file_name| file_name.to_str())
    else {
        return Vec::new();
    };
    let mut matches = Vec::new();
    let mut remaining_dirs = 256usize;
    collect_moved_file_candidates(
        workspace_path,
        workspace_path,
        file_name,
        limit,
        &mut remaining_dirs,
        &mut matches,
    );
    matches
}

fn collect_moved_file_candidates(
    workspace_path: &Path,
    directory: &Path,
    file_name: &str,
    limit: usize,
    remaining_dirs: &mut usize,
    matches: &mut Vec<String>,
) {
    if matches.len() >= limit || *remaining_dirs == 0 {
        return;
    }
    *remaining_dirs -= 1;
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        if matches.len() >= limit {
            break;
        }
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if is_ignored_reference_search_dir(&path) {
                continue;
            }
            collect_moved_file_candidates(
                workspace_path,
                &path,
                file_name,
                limit,
                remaining_dirs,
                matches,
            );
        } else if file_type.is_file()
            && path.file_name().and_then(|name| name.to_str()) == Some(file_name)
            && let Ok(relative) = path.strip_prefix(workspace_path)
        {
            matches.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn is_ignored_reference_search_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | ".foco" | ".codegraph" | ".mem" | "node_modules" | "target" | "dist")
    )
}

fn package_script_from_command<'a>(words: &'a [&'a str]) -> Option<&'a str> {
    match words {
        ["npm" | "pnpm", "run", script, ..] => Some(*script),
        ["yarn", "run", script, ..] => Some(*script),
        ["yarn", script, ..]
            if !matches!(
                *script,
                "add" | "install" | "remove" | "upgrade" | "dlx" | "exec"
            ) =>
        {
            Some(*script)
        }
        _ => None,
    }
}

fn package_json_has_script(workspace_path: &Path, script: &str) -> Result<bool, std::io::Error> {
    let package_json = workspace_path.join("package.json");
    let content = match fs::read_to_string(package_json) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return Ok(false);
    };
    Ok(value
        .get("scripts")
        .and_then(Value::as_object)
        .is_some_and(|scripts| scripts.get(script).and_then(Value::as_str).is_some()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExtractedMemoryReference {
    reference_type: MemoryReferenceType,
    value: String,
    normalized_value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OwnedMemoryReference {
    id: String,
    fact_id: String,
    reference_type: MemoryReferenceType,
    value: String,
    normalized_value: String,
    status: MemoryReferenceStatus,
    metadata_json: String,
    checked_at: Option<String>,
}

#[derive(Clone, Debug)]
struct ReferenceValidation {
    status: MemoryReferenceStatus,
    metadata: Value,
}

impl ReferenceValidation {
    fn valid(metadata: Value) -> Self {
        Self {
            status: MemoryReferenceStatus::Valid,
            metadata,
        }
    }

    fn invalid(metadata: Value) -> Self {
        Self {
            status: MemoryReferenceStatus::Invalid,
            metadata,
        }
    }

    fn ambiguous(metadata: Value) -> Self {
        Self {
            status: MemoryReferenceStatus::Ambiguous,
            metadata,
        }
    }

    fn skipped(metadata: Value) -> Self {
        Self {
            status: MemoryReferenceStatus::Skipped,
            metadata,
        }
    }
}

fn promoted_fact_metadata_json(
    metadata_json: &str,
    job_id: &str,
    workspace_id: &str,
    source_fact_ids: &[String],
    evidence: &[DreamPlannerEvidence],
) -> Result<String, MemoryDatabaseError> {
    add_dream_promotion_metadata(
        metadata_json,
        json!({
            "dreamJobId": job_id,
            "originWorkspaceId": workspace_id,
            "sourceFactIds": source_fact_ids,
            "evidence": evidence,
        }),
    )
}

fn promoted_source_metadata_json(
    metadata_json: &str,
    job_id: &str,
    workspace_id: &str,
    source_fact_id: &str,
) -> Result<String, MemoryDatabaseError> {
    add_dream_promotion_metadata(
        metadata_json,
        json!({
            "dreamJobId": job_id,
            "originWorkspaceId": workspace_id,
            "sourceFactId": source_fact_id,
        }),
    )
}

fn add_dream_promotion_metadata(
    metadata_json: &str,
    promotion: Value,
) -> Result<String, MemoryDatabaseError> {
    let metadata = serde_json::from_str::<Value>(metadata_json).map_err(|source| {
        MemoryDatabaseError::InvalidMemoryJson {
            field: "metadata_json",
            source,
        }
    })?;
    let mut object = match metadata {
        Value::Object(object) => object,
        _ => serde_json::Map::new(),
    };
    object.insert("dreamPromotion".to_string(), promotion);
    serde_json::to_string(&Value::Object(object)).map_err(|source| {
        MemoryDatabaseError::InvalidMemoryJson {
            field: "metadata_json",
            source,
        }
    })
}

fn compact_json_text(value: &str) -> String {
    serde_json::from_str::<Value>(value)
        .map(|value| compact_text(&value.to_string(), 1_000))
        .unwrap_or_else(|_| compact_text(value, 1_000))
}

fn compact_text(value: &str, max_chars: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        return compact;
    }

    let mut truncated = compact.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
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
    ActivatePending {
        fact: MemoryFactRecord,
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
            Self::ActivatePending { .. } => "activate",
            Self::RejectPending { .. } => "reject",
        }
    }

    fn reason(&self) -> &'static str {
        match self {
            Self::Expire { .. } => "memory fact expires_at is due",
            Self::MergeDuplicate { .. } => "exact duplicate memory fact",
            Self::RepairUpdatesChain { .. } => "updates chain target should not be latest",
            Self::ActivatePending { .. } => {
                "high-confidence pending memory fact has supporting evidence"
            }
            Self::RejectPending { .. } => "stale low-confidence pending memory fact",
        }
    }

    fn target_fact_ids(&self) -> Vec<&str> {
        match self {
            Self::Expire { fact }
            | Self::RejectPending { fact }
            | Self::ActivatePending { fact } => {
                vec![fact.id.as_str()]
            }
            Self::MergeDuplicate { winner, loser } => vec![winner.id.as_str(), loser.id.as_str()],
            Self::RepairUpdatesChain { source, target } => {
                vec![source.id.as_str(), target.id.as_str()]
            }
        }
    }

    fn primary_fact(&self) -> Option<&MemoryFactRecord> {
        match self {
            Self::Expire { fact }
            | Self::RejectPending { fact }
            | Self::ActivatePending { fact } => Some(fact),
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
            Self::ActivatePending { fact } => json!([{
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

#[derive(Default)]
struct DreamReferenceValidationSummary {
    references: Vec<MemoryReferenceRecord>,
    total: usize,
    valid: usize,
    invalid: usize,
    ambiguous: usize,
    skipped: usize,
}

impl DreamReferenceValidationSummary {
    fn from_references(references: Vec<MemoryReferenceRecord>) -> Self {
        let valid = references
            .iter()
            .filter(|reference| reference.status == MemoryReferenceStatus::Valid.as_str())
            .count();
        let invalid = references
            .iter()
            .filter(|reference| reference.status == MemoryReferenceStatus::Invalid.as_str())
            .count();
        let ambiguous = references
            .iter()
            .filter(|reference| reference.status == MemoryReferenceStatus::Ambiguous.as_str())
            .count();
        let skipped = references
            .iter()
            .filter(|reference| reference.status == MemoryReferenceStatus::Skipped.as_str())
            .count();
        Self {
            total: references.len(),
            references,
            valid,
            invalid,
            ambiguous,
            skipped,
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "total": self.total,
            "valid": self.valid,
            "invalid": self.invalid,
            "ambiguous": self.ambiguous,
            "skipped": self.skipped,
            "sample": self.references.iter().take(20).map(memory_reference_json).collect::<Vec<_>>(),
            "rule": "Never expire a fact solely because one path reference is invalid.",
        })
    }
}

struct DreamRunSummary {
    candidates_considered: usize,
    references_extracted: usize,
    references_valid: usize,
    references_invalid: usize,
    references_ambiguous: usize,
    references_skipped: usize,
    deterministic_changes_proposed: usize,
    llm_changes_proposed: usize,
    changes_applied: usize,
    changes_skipped: usize,
    changes_failed: usize,
    profiles_refreshed: usize,
    llm_planner: &'static str,
    llm_error: Option<String>,
    failure_category: Option<&'static str>,
    error_message: Option<String>,
}

impl DreamRunSummary {
    fn failed(error: &ApiError) -> Self {
        Self {
            candidates_considered: 0,
            references_extracted: 0,
            references_valid: 0,
            references_invalid: 0,
            references_ambiguous: 0,
            references_skipped: 0,
            deterministic_changes_proposed: 0,
            llm_changes_proposed: 0,
            changes_applied: 0,
            changes_skipped: 0,
            changes_failed: 0,
            profiles_refreshed: 0,
            llm_planner: "failed",
            llm_error: None,
            failure_category: Some(dream_failure_category(error)),
            error_message: Some(error.message.clone()),
        }
    }

    fn to_json(&self) -> Value {
        let summary = if let Some(failure_category) = self.failure_category {
            format!("Memory Dream failed: {failure_category}")
        } else {
            format!(
                "{} changes applied, {} skipped, {} failed",
                self.changes_applied, self.changes_skipped, self.changes_failed
            )
        };
        json!({
            "summary": summary,
            "candidatesConsidered": self.candidates_considered,
            "referencesExtracted": self.references_extracted,
            "referencesValid": self.references_valid,
            "referencesInvalid": self.references_invalid,
            "referencesAmbiguous": self.references_ambiguous,
            "referencesSkipped": self.references_skipped,
            "deterministicChangesProposed": self.deterministic_changes_proposed,
            "llmChangesProposed": self.llm_changes_proposed,
            "changesApplied": self.changes_applied,
            "changesSkipped": self.changes_skipped,
            "changesFailed": self.changes_failed,
            "profilesRefreshed": self.profiles_refreshed,
            "llmPlanner": self.llm_planner,
            "llmError": self.llm_error,
            "failureCategory": self.failure_category,
            "errorMessage": self.error_message,
            "rollbackGuidance": MEMORY_DREAM_ROLLBACK_GUIDANCE,
        })
    }
}

fn dream_failure_category(error: &ApiError) -> &'static str {
    let message = error.message.to_ascii_lowercase();
    if message.contains("cancel") || message.contains("shutdown") || message.contains("interrupted")
    {
        return "cancelled_shutdown";
    }
    if message.contains("malformed memory dream planner json")
        || message.contains("did not call")
        || message.contains("returned text instead")
        || message.contains("unsupported tool")
    {
        return "malformed_llm_output";
    }
    if message.contains("not configured")
        || message.contains("requires a configured model")
        || message.contains("audit workspace")
    {
        return "config_error";
    }
    if message.contains("model")
        || message.contains("provider")
        || message.contains("stream failed")
        || message.contains("timed out")
    {
        return "model_unavailable";
    }
    if message.contains("target changed before apply")
        || message.contains("sqlite")
        || message.contains("database")
    {
        return "database_conflict";
    }
    if error.status == axum::http::StatusCode::BAD_REQUEST {
        return "validation_failed";
    }

    "config_error"
}

fn now_minus_days(days: i64) -> String {
    (Utc::now() - ChronoDuration::days(days)).to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use foco_store::{
        config::WorkspaceCommonCommand,
        memory::{MemorySourceType, NewMemoryFact, NewMemorySource},
        workspace::{
            NewCodeGraphFileIndex, NewCodeGraphSymbol, WorkspaceDatabase, workspace_database_path,
        },
    };

    use super::*;

    #[tokio::test]
    async fn deterministic_dream_expires_due_facts() {
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
            .await
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

    #[tokio::test]
    async fn deterministic_dream_merges_exact_duplicates() {
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
            .await
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

    #[tokio::test]
    async fn deterministic_dream_repairs_updates_chain_latest_flag() {
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
            .await
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

    #[tokio::test]
    async fn deterministic_dream_activates_high_confidence_pending_facts() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let workspace_dir = temp_dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_dir).expect("workspace dir");
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let mut database =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
                .expect("workspace memory database");
        insert_test_fact(
            &mut database,
            "fact-pending-strong",
            MemoryScope::Workspace,
            None,
            MemoryStatus::Pending,
            MemoryKind::ProjectFact,
            "The project uses Vitest for frontend tests.",
            Some(0.92),
        );

        let result = run_memory_dream_job(&mut database, test_request(MemoryDreamScope::Workspace))
            .await
            .expect("dream run");

        assert_eq!(result.applied_changes, 1);
        let fact = database
            .fact("fact-pending-strong")
            .expect("fact")
            .expect("fact exists");
        assert_eq!(fact.status, "active");
        assert!(fact.is_latest);
        let changes = database
            .dream_changes_for_job(&result.job.id, Some(MemoryDreamChangeStatus::Applied), 10)
            .expect("changes");
        assert_eq!(changes[0].operation, "activate");
    }

    #[test]
    fn audit_hints_surface_reference_issues_and_pending_transitions() {
        let mut pending = test_fact_record("fact-pending", MemoryScope::Workspace);
        pending.status = MemoryStatus::Pending.as_str().to_string();
        pending.confidence = Some(0.9);
        let reference = MemoryReferenceRecord {
            id: "reference-1".to_string(),
            fact_id: "fact-pending".to_string(),
            reference_type: MemoryReferenceType::FilePath.as_str().to_string(),
            value: "missing.rs".to_string(),
            normalized_value: "missing.rs".to_string(),
            status: MemoryReferenceStatus::Invalid.as_str().to_string(),
            metadata_json: r#"{"reason":"notFound"}"#.to_string(),
            checked_at: Some("2026-06-23T00:00:00Z".to_string()),
            created_at: "2026-06-23T00:00:00Z".to_string(),
            updated_at: "2026-06-23T00:00:00Z".to_string(),
        };
        let hints = memory_dream_audit_hints_json(
            &[pending.clone()],
            &[reference],
            &[DreamChange::ActivatePending { fact: pending }],
        );

        assert_eq!(hints["pendingTransitions"][0]["operation"], "activate");
        assert_eq!(hints["referenceIssues"][0]["factId"], "fact-pending");
    }

    #[test]
    fn duplicate_normalization_ignores_case_whitespace_and_punctuation() {
        assert_eq!(
            normalized_duplicate_text(" Prefer   compact replies! "),
            normalized_duplicate_text("prefer, compact replies")
        );
    }

    #[test]
    fn dream_planner_tool_schema_is_strict() {
        let tool = memory_dream_planner_tool_definition();

        assert!(tool.strict);
        assert_eq!(
            tool.input_schema.get("additionalProperties"),
            Some(&Value::Bool(false))
        );
        let change_schema = tool.input_schema["properties"]["changes"]["items"]
            .as_object()
            .expect("change schema object");
        assert_eq!(
            change_schema.get("additionalProperties"),
            Some(&Value::Bool(false))
        );
        assert_eq!(
            change_schema.get("required").and_then(Value::as_array),
            Some(
                &[
                    "operation",
                    "targetFactIds",
                    "newFact",
                    "reason",
                    "confidence",
                    "riskLevel",
                    "evidence"
                ]
                .into_iter()
                .map(|value| Value::String(value.to_string()))
                .collect::<Vec<_>>()
            )
        );
    }

    #[test]
    fn dream_planner_rejects_malformed_output() {
        let error = parse_memory_dream_planner_output(json!({
            "summary": "bad",
            "changes": [{"operation": "expire"}]
        }))
        .expect_err("malformed output should fail");

        assert!(
            error
                .message
                .contains("malformed memory Dream planner JSON")
        );
    }

    #[test]
    fn dream_planner_rejects_unknown_fact_ids() {
        let output = planner_output("expire", vec!["fact-missing"], true);
        let candidates = vec![test_fact_record("fact-1", MemoryScope::Global)];
        let policy = MemoryDreamSafetyPolicy::new(10, 10).expect("policy");

        let error = validate_memory_dream_planner_output(
            &output,
            MemoryDreamScope::Global,
            &candidates,
            &policy,
        )
        .expect_err("unknown target should fail");

        assert!(error.message.contains("non-candidate fact id"));
    }

    #[test]
    fn dream_planner_rejects_missing_evidence() {
        let output = planner_output("expire", vec!["fact-1"], false);
        let candidates = vec![test_fact_record("fact-1", MemoryScope::Global)];
        let policy = MemoryDreamSafetyPolicy::new(10, 10).expect("policy");

        let error = validate_memory_dream_planner_output(
            &output,
            MemoryDreamScope::Global,
            &candidates,
            &policy,
        )
        .expect_err("missing evidence should fail");

        assert!(error.message.contains("must include evidence"));
    }

    #[test]
    fn dream_planner_keeps_global_scope_conservative() {
        let output = planner_output("update", vec!["fact-1"], true);
        let candidates = vec![test_fact_record("fact-1", MemoryScope::Global)];
        let policy = MemoryDreamSafetyPolicy::new(10, 10).expect("policy");

        let error = validate_memory_dream_planner_output(
            &output,
            MemoryDreamScope::Global,
            &candidates,
            &policy,
        )
        .expect_err("global updates should fail");

        assert!(error.message.contains("not conservative enough"));
    }

    #[test]
    fn dream_planner_allows_explicit_workspace_preference_promotion() {
        let output = promote_output("fact-1", Some("Prefer compact replies in all projects."));
        let candidates = vec![test_fact_record("fact-1", MemoryScope::Workspace)];
        let policy = MemoryDreamSafetyPolicy::new(10, 10).expect("policy");

        let validated = validate_memory_dream_planner_output(
            &output,
            MemoryDreamScope::Workspace,
            &candidates,
            &policy,
        )
        .expect("promotion should validate");

        assert_eq!(
            validated[0].operation,
            DreamPlannerOperation::PromoteToGlobal
        );
    }

    #[test]
    fn dream_planner_rejects_project_specific_global_promotion_candidates() {
        let mut project_fact = test_fact_record("fact-project", MemoryScope::Workspace);
        project_fact.kind = MemoryKind::ProjectDecision.as_str().to_string();
        let policy = MemoryDreamSafetyPolicy::new(10, 10).expect("policy");

        let error = validate_memory_dream_planner_output(
            &promote_output(
                "fact-project",
                Some("Prefer compact replies in all projects."),
            ),
            MemoryDreamScope::Workspace,
            &[project_fact],
            &policy,
        )
        .expect_err("project decisions should not promote");
        assert!(error.message.contains("only supports preference facts"));

        let mut path_fact = test_fact_record("fact-path", MemoryScope::Workspace);
        path_fact.fact = "Prefer editing app/main.rs first.".to_string();
        let error = validate_memory_dream_planner_output(
            &promote_output("fact-path", None),
            MemoryDreamScope::Workspace,
            &[path_fact],
            &policy,
        )
        .expect_err("path-specific facts should not promote");
        assert!(error.message.contains("project-specific file path"));
    }

    #[test]
    fn dream_promotion_writes_global_origin_evidence() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let workspace_dir = temp_dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_dir).expect("workspace dir");
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let global_memory_path = temp_dir.path().join("global-memory.sqlite");
        let mut database =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
                .expect("workspace memory database");
        insert_test_fact(
            &mut database,
            "fact-source",
            MemoryScope::Workspace,
            None,
            MemoryStatus::Active,
            MemoryKind::Preference,
            "Prefer concise answers.",
            Some(0.9),
        );
        database
            .insert_dream_job(NewMemoryDreamJob {
                id: "job-1",
                scope: MemoryDreamScope::Workspace,
                workspace_id: Some("workspace-1"),
                trigger_type: MemoryDreamTriggerType::Manual,
                mode: MemoryDreamRunMode::Llm,
                status: MemoryDreamJobStatus::Running,
                model_id: None,
                input_summary_json: "{}",
                output_summary_json: None,
                transcript_chat_id: None,
                error_message: None,
            })
            .expect("dream job");
        let candidate = database
            .fact("fact-source")
            .expect("fact lookup")
            .expect("fact exists");
        let policy = MemoryDreamSafetyPolicy::new(10, 10).expect("policy");
        let output = promote_output(
            "fact-source",
            Some("Prefer concise answers across all projects."),
        );
        let validated = validate_memory_dream_planner_output(
            &output,
            MemoryDreamScope::Workspace,
            &[candidate],
            &policy,
        )
        .expect("validated promotion");

        let summary = apply_llm_changes(
            &mut database,
            "job-1",
            &validated,
            &policy,
            Some("workspace-1"),
            Some(&global_memory_path),
        )
        .expect("apply promotion");

        assert_eq!(summary.applied, 1);
        let global_database =
            MemoryDatabase::open_or_create_global_at(&global_memory_path).expect("global memory");
        let promoted = global_database
            .dream_candidate_facts(MemoryDreamScope::Global, None, 10)
            .expect("global facts");
        assert_eq!(promoted.len(), 1);
        assert_eq!(
            promoted[0].fact,
            "Prefer concise answers across all projects."
        );
        let metadata: Value =
            serde_json::from_str(&promoted[0].metadata_json).expect("promoted metadata");
        assert_eq!(
            metadata["dreamPromotion"]["originWorkspaceId"],
            "workspace-1"
        );
        assert_eq!(
            metadata["dreamPromotion"]["sourceFactIds"][0],
            "fact-source"
        );
        let sources = global_database
            .sources_for_fact(&promoted[0].id)
            .expect("promoted sources");
        assert!(!sources.is_empty());
        let source_metadata: Value =
            serde_json::from_str(&sources[0].metadata_json).expect("source metadata");
        assert_eq!(
            source_metadata["dreamPromotion"]["originWorkspaceId"],
            "workspace-1"
        );
        let changes = database
            .dream_changes_for_job("job-1", Some(MemoryDreamChangeStatus::Applied), 10)
            .expect("dream changes");
        assert_eq!(changes[0].operation, "promote_to_global");
        assert_eq!(
            changes[0].new_fact_id.as_deref(),
            Some(promoted[0].id.as_str())
        );
    }

    #[tokio::test]
    async fn automatic_llm_dream_falls_back_to_deterministic_changes() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut database =
            MemoryDatabase::open_or_create_global_at(temp_dir.path().join("memory.sqlite"))
                .expect("global memory database");
        insert_test_fact(
            &mut database,
            "fact-expired-auto",
            MemoryScope::Global,
            None,
            MemoryStatus::Active,
            MemoryKind::Preference,
            "Temporary auto preference.",
            Some(0.9),
        );
        database
            .update_fact(UpdateMemoryFact {
                id: "fact-expired-auto",
                expires_at: Some(&now_minus_days(1)),
                ..UpdateMemoryFact::default()
            })
            .expect("set expiration");
        let mut request = test_request(MemoryDreamScope::Global);
        request.mode = MemoryDreamRunMode::Llm;
        request.trigger_type = MemoryDreamTriggerType::AutoInterval;

        let result = run_memory_dream_job(&mut database, request)
            .await
            .expect("auto dream should fall back");
        let output: Value = serde_json::from_str(
            result
                .job
                .output_summary_json
                .as_deref()
                .expect("output summary"),
        )
        .expect("summary json");

        assert_eq!(result.applied_changes, 1);
        assert_eq!(
            output["llmPlanner"],
            Value::String("fallback_deterministic_only".to_string())
        );
    }

    #[tokio::test]
    async fn manual_llm_dream_fails_without_planner_or_deterministic_changes() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut database =
            MemoryDatabase::open_or_create_global_at(temp_dir.path().join("memory.sqlite"))
                .expect("global memory database");
        let mut request = test_request(MemoryDreamScope::Global);
        request.mode = MemoryDreamRunMode::Llm;

        let error = run_memory_dream_job(&mut database, request)
            .await
            .expect_err("manual dream without planner should fail");

        assert!(error.message.contains("requires a configured model"));
        let job = database
            .dream_jobs_for_scope(MemoryDreamScope::Global, None, None, 1)
            .expect("dream jobs")
            .into_iter()
            .next()
            .expect("failed job");
        let output: Value = serde_json::from_str(
            job.output_summary_json
                .as_deref()
                .expect("failed output summary"),
        )
        .expect("failed summary json");
        assert_eq!(output["failureCategory"], "config_error");
        assert!(
            output["rollbackGuidance"]
                .as_str()
                .expect("rollback guidance")
                .contains("never hard-deletes")
        );
    }

    #[test]
    fn dream_failure_category_covers_phase10_labels() {
        assert_eq!(
            dream_failure_category(&ApiError::bad_request("requires a configured model")),
            "config_error"
        );
        assert_eq!(
            dream_failure_category(&ApiError::internal("provider stream failed")),
            "model_unavailable"
        );
        assert_eq!(
            dream_failure_category(&ApiError::bad_request(
                "malformed memory Dream planner JSON"
            )),
            "malformed_llm_output"
        );
        assert_eq!(
            dream_failure_category(&ApiError::bad_request("non-candidate fact id")),
            "validation_failed"
        );
        assert_eq!(
            dream_failure_category(&ApiError::internal("target changed before apply")),
            "database_conflict"
        );
        assert_eq!(
            dream_failure_category(&ApiError::internal("app shutdown requested")),
            "cancelled_shutdown"
        );
    }

    #[tokio::test]
    async fn dream_transcript_chat_is_created_and_hidden_from_normal_chat_list() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let workspace_dir = temp_dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_dir).expect("workspace dir");
        WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let mut database =
            MemoryDatabase::open_or_create_global_at(temp_dir.path().join("memory.sqlite"))
                .expect("global memory database");
        let request = MemoryDreamJobRequest {
            scope: MemoryDreamScope::Global,
            workspace_id: None,
            trigger_type: MemoryDreamTriggerType::Manual,
            mode: MemoryDreamRunMode::DeterministicOnly,
            model_id: None,
            settings: test_settings(true),
            config: None,
            global_memory_database_file: None,
            planner: None,
            transcript: Some(MemoryDreamTranscriptRequest {
                workspace_path: &workspace_dir,
            }),
        };

        let result = run_memory_dream_job(&mut database, request)
            .await
            .expect("dream run");
        let transcript_chat_id = result
            .job
            .transcript_chat_id
            .as_deref()
            .expect("transcript chat id");
        let workspace_database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");

        assert!(workspace_database.chats().expect("normal chats").is_empty());
        let dream_chats = workspace_database
            .dream_transcript_chats()
            .expect("dream chats");
        assert_eq!(dream_chats.len(), 1);
        assert_eq!(dream_chats[0].id, transcript_chat_id);
        let metadata: Value =
            serde_json::from_str(&dream_chats[0].metadata_json).expect("chat metadata");
        assert_eq!(metadata["kind"], MEMORY_DREAM_TRANSCRIPT_CHAT_KIND);
        assert_eq!(metadata["dreamJobId"], result.job.id);
        let messages = workspace_database
            .messages_for_chat(transcript_chat_id)
            .expect("transcript messages");
        assert!(
            messages
                .iter()
                .any(|message| message.content.contains("job started"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.content.contains("final status"))
        );
    }

    #[tokio::test]
    async fn memory_dream_extracts_and_validates_references() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let workspace_dir = temp_dir.path().join("workspace");
        std::fs::create_dir_all(workspace_dir.join("app")).expect("app dir");
        std::fs::create_dir_all(workspace_dir.join("web")).expect("web dir");
        std::fs::create_dir_all(workspace_dir.join("a")).expect("a dir");
        std::fs::create_dir_all(workspace_dir.join("b")).expect("b dir");
        std::fs::write(workspace_dir.join("app/main.rs"), "fn public_api() {}\n")
            .expect("main file");
        std::fs::write(
            workspace_dir.join("web/App.tsx"),
            "export function App() {}\n",
        )
        .expect("moved file");
        std::fs::write(workspace_dir.join("a/ambiguous.rs"), "").expect("ambiguous a");
        std::fs::write(workspace_dir.join("b/ambiguous.rs"), "").expect("ambiguous b");
        std::fs::write(
            workspace_dir.join("package.json"),
            r#"{"scripts":{"test":"vitest"}}"#,
        )
        .expect("package json");

        let mut workspace_database =
            WorkspaceDatabase::open_or_create(&workspace_dir).expect("workspace database");
        let main_symbols = [
            NewCodeGraphSymbol {
                name: "public_api",
                kind: "function",
                start_line: Some(1),
                start_column: Some(1),
                end_line: Some(1),
                end_column: Some(16),
                signature: Some("fn public_api()"),
                documentation: None,
            },
            NewCodeGraphSymbol {
                name: "duplicate_symbol",
                kind: "function",
                start_line: Some(3),
                start_column: Some(1),
                end_line: Some(3),
                end_column: Some(20),
                signature: Some("fn duplicate_symbol()"),
                documentation: None,
            },
        ];
        workspace_database
            .replace_code_graph_file_index(NewCodeGraphFileIndex {
                path: "app/main.rs",
                language: Some("rust"),
                size_bytes: Some(64),
                modified_at: Some("2026-06-23T00:00:00Z"),
                content_hash: "main-hash",
                parse_status: "parsed",
                parse_error_message: None,
                symbols: &main_symbols,
                imports: &[],
                references: &[],
                edges: &[],
                fts_body: "fn public_api() {} fn duplicate_symbol() {}",
            })
            .expect("main index");
        let other_symbols = [NewCodeGraphSymbol {
            name: "duplicate_symbol",
            kind: "function",
            start_line: Some(1),
            start_column: Some(1),
            end_line: Some(1),
            end_column: Some(20),
            signature: Some("fn duplicate_symbol()"),
            documentation: None,
        }];
        workspace_database
            .replace_code_graph_file_index(NewCodeGraphFileIndex {
                path: "web/lib.rs",
                language: Some("rust"),
                size_bytes: Some(32),
                modified_at: Some("2026-06-23T00:00:00Z"),
                content_hash: "other-hash",
                parse_status: "parsed",
                parse_error_message: None,
                symbols: &other_symbols,
                imports: &[],
                references: &[],
                edges: &[],
                fts_body: "fn duplicate_symbol() {}",
            })
            .expect("other index");
        drop(workspace_database);

        let mut config = GlobalConfig::first_run(workspace_dir.clone());
        config.workspaces[0].id = "workspace-1".to_string();
        config.workspaces[0]
            .common_commands
            .push(WorkspaceCommonCommand {
                name: "test".to_string(),
                command: "npm run test".to_string(),
            });

        let mut database =
            MemoryDatabase::open_workspace_at(workspace_database_path(&workspace_dir))
                .expect("workspace memory database");
        insert_test_fact(
            &mut database,
            "fact-references",
            MemoryScope::Workspace,
            None,
            MemoryStatus::Active,
            MemoryKind::ProjectFact,
            "Use `app/main.rs`, `missing.rs`, `old/App.tsx`, `old/ambiguous.rs`, \
             `public_api`, `duplicate_symbol`, https://user:secret@example.com/path?token=secret#frag, \
             and run `npm run test`.",
            Some(0.9),
        );

        let result = run_memory_dream_job(
            &mut database,
            MemoryDreamJobRequest {
                scope: MemoryDreamScope::Workspace,
                workspace_id: Some("workspace-1"),
                trigger_type: MemoryDreamTriggerType::Manual,
                mode: MemoryDreamRunMode::DeterministicOnly,
                model_id: None,
                settings: test_settings(false),
                config: Some(&config),
                global_memory_database_file: None,
                planner: None,
                transcript: None,
            },
        )
        .await
        .expect("dream run");

        let references = database
            .references_for_fact_ids(&["fact-references".to_string()], 20)
            .expect("references");
        let reference = |reference_type: &str, normalized_value: &str| {
            references
                .iter()
                .find(|reference| {
                    reference.reference_type == reference_type
                        && reference.normalized_value == normalized_value
                })
                .unwrap_or_else(|| panic!("missing reference {reference_type}:{normalized_value}"))
        };

        assert_eq!(reference("file_path", "app/main.rs").status, "valid");
        assert_eq!(reference("file_path", "missing.rs").status, "invalid");
        let moved = reference("file_path", "old/App.tsx");
        assert_eq!(moved.status, "invalid");
        assert!(moved.metadata_json.contains(r#""reason":"moved""#));
        assert_eq!(
            reference("file_path", "old/ambiguous.rs").status,
            "ambiguous"
        );
        assert_eq!(reference("symbol", "public_api").status, "valid");
        assert_eq!(reference("symbol", "duplicate_symbol").status, "ambiguous");
        assert_eq!(reference("command", "npm run test").status, "valid");
        let url = reference("url", "https://example.com/path");
        assert_eq!(url.status, "valid");
        assert!(!url.value.contains("secret"));
        assert!(!url.value.contains("token"));

        let summary: Value =
            serde_json::from_str(result.job.output_summary_json.as_deref().unwrap())
                .expect("summary json");
        assert!(summary["referencesExtracted"].as_u64().unwrap() >= 8);
        assert!(summary["referencesInvalid"].as_u64().unwrap() >= 2);
        assert!(summary["referencesAmbiguous"].as_u64().unwrap() >= 2);
    }

    fn test_request(scope: MemoryDreamScope) -> MemoryDreamJobRequest<'static> {
        MemoryDreamJobRequest {
            scope,
            workspace_id: (scope == MemoryDreamScope::Workspace).then_some("workspace-1"),
            trigger_type: MemoryDreamTriggerType::Manual,
            mode: MemoryDreamRunMode::DeterministicOnly,
            model_id: None,
            settings: test_settings(false),
            config: None,
            global_memory_database_file: None,
            planner: None,
            transcript: None,
        }
    }

    fn test_settings(create_transcript_chat: bool) -> &'static MemoryDreamSettings {
        Box::leak(Box::new(MemoryDreamSettings {
            enabled: true,
            auto_enabled: false,
            mode: "deterministic_only".to_string(),
            model_id: None,
            workspace_interval_days: 7,
            global_interval_days: 30,
            create_transcript_chat,
            max_facts_per_run: 100,
            max_changes_per_run: 10,
            scheduler_scan_minutes: 60,
            workspace_threshold_facts: 50,
            global_threshold_facts: 50,
            llm_timeout_ms: 120_000,
        }))
    }

    fn planner_output(
        operation: &str,
        target_fact_ids: Vec<&str>,
        include_evidence: bool,
    ) -> MemoryDreamPlannerOutput {
        MemoryDreamPlannerOutput {
            summary: "summary".to_string(),
            changes: vec![MemoryDreamPlannerChange {
                operation: operation.to_string(),
                target_fact_ids: target_fact_ids
                    .into_iter()
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
                new_fact: None,
                reason: "because evidence says so".to_string(),
                confidence: Some(0.9),
                risk_level: "low".to_string(),
                evidence: include_evidence
                    .then(|| {
                        vec![DreamPlannerEvidence {
                            source_type: "memory_fact".to_string(),
                            source_id: "fact-1".to_string(),
                            quote: "evidence quote".to_string(),
                        }]
                    })
                    .unwrap_or_default(),
            }],
        }
    }

    fn promote_output(target_fact_id: &str, fact: Option<&str>) -> MemoryDreamPlannerOutput {
        MemoryDreamPlannerOutput {
            summary: "promote user-wide preference".to_string(),
            changes: vec![MemoryDreamPlannerChange {
                operation: "promote_to_global".to_string(),
                target_fact_ids: vec![target_fact_id.to_string()],
                new_fact: Some(DreamPlannerNewFact {
                    fact: fact.map(str::to_string),
                    kind: Some(MemoryKind::Preference.as_str().to_string()),
                    confidence: Some(0.9),
                    relation: None,
                    expires_at: None,
                }),
                reason: "explicit user-wide preference".to_string(),
                confidence: Some(0.9),
                risk_level: "low".to_string(),
                evidence: vec![DreamPlannerEvidence {
                    source_type: "memory_fact".to_string(),
                    source_id: target_fact_id.to_string(),
                    quote: "The user said this is a user-wide preference across workspaces."
                        .to_string(),
                }],
            }],
        }
    }

    fn test_fact_record(id: &str, scope: MemoryScope) -> MemoryFactRecord {
        MemoryFactRecord {
            id: id.to_string(),
            scope: scope.as_str().to_string(),
            chat_id: None,
            status: MemoryStatus::Active.as_str().to_string(),
            kind: MemoryKind::Preference.as_str().to_string(),
            fact: "Prefer compact replies.".to_string(),
            confidence: Some(0.9),
            pinned: false,
            is_latest: true,
            expires_at: None,
            metadata_json: "{}".to_string(),
            created_at: "2026-06-23T00:00:00.000Z".to_string(),
            updated_at: "2026-06-23T00:00:00.000Z".to_string(),
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
