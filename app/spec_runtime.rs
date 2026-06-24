use std::{fs, path::PathBuf};

use foco_providers::{
    NeutralChatRequest, NeutralChatRole, NeutralToolDefinition, ProviderConnectionConfig,
};
use foco_store::{
    config::{GlobalConfig, ModelSettings},
    memory::MemoryDatabase,
    workspace::{
        CodeChangeStats, CodeGraphFileSummaryRecord, CodeGraphSymbolRecord, NewWorkspaceSpecJob,
        WORKSPACE_SPEC_MAX_MARKDOWN_BYTES, WorkspaceDatabase, WorkspaceSpecJobRecord,
        WorkspaceSpecJobStatus, WorkspaceSpecTriggerType, WorkspaceSpecWriteDecision,
        workspace_database_path,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    ApiError, AppState, PreparedChatContext, api_audit_save_details, audited_provider_tool_request,
    config_snapshot, neutral_text_message, provider_connection_config, unique_id, workspace_by_id,
};

const WORKSPACE_SPEC_TOOL_NAME: &str = "submit_workspace_spec";
const WORKSPACE_SPEC_UPDATE_TOOL_NAME: &str = "submit_workspace_spec_update";
const WORKSPACE_SPEC_TIMEOUT_MS: u64 = 120_000;
const WORKSPACE_SPEC_MAX_OUTPUT_TOKENS: u32 = 4_000;
const WORKSPACE_SPEC_FILE_SUMMARY_LIMIT: i64 = 24;
const WORKSPACE_SPEC_SYMBOL_LIMIT: i64 = 48;
const WORKSPACE_SPEC_MEMORY_PROFILE_LIMIT: u32 = 4;
const WORKSPACE_SPEC_ROOT_FILE_LIMIT: usize = 6;
const WORKSPACE_SPEC_SOURCE_FILE_MAX_CHARS: usize = 6_000;
const WORKSPACE_SPEC_MEMORY_PROFILE_MAX_CHARS: usize = 2_000;
const WORKSPACE_SPEC_CHAT_EXCERPT_MAX_CHARS: usize = 2_000;

// ponytail: root-file heuristic; replace with graph centrality only if generated specs need better recall.
const ROOT_SOURCE_FILE_CANDIDATES: &[&str] = &[
    "README.md",
    "README",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "deno.json",
    "vite.config.ts",
];

#[derive(Clone, Debug)]
pub(crate) struct PreparedWorkspaceSpecJob {
    pub(crate) workspace_id: String,
    pub(crate) workspace_path: PathBuf,
    pub(crate) job_id: String,
    pub(crate) base_revision: u64,
    pub(crate) provider_id: String,
    pub(crate) provider_config: ProviderConnectionConfig,
    pub(crate) request: NeutralChatRequest,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecGenerationInput {
    pub(crate) workspace_id: String,
    pub(crate) base_revision: u64,
    pub(crate) code_graph: WorkspaceSpecCodeGraphInput,
    pub(crate) memory_profiles: Vec<WorkspaceSpecMemoryProfileInput>,
    pub(crate) source_files: Vec<WorkspaceSpecSourceFileInput>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecUpdateInput {
    pub(crate) workspace_id: String,
    pub(crate) chat_id: String,
    pub(crate) current_spec_revision: u64,
    pub(crate) user_message_id: String,
    pub(crate) assistant_message_id: String,
    pub(crate) run_id: String,
    pub(crate) code_change_stats: Option<CodeChangeStats>,
    pub(crate) chat_excerpt: WorkspaceSpecChatExcerptInput,
    pub(crate) current_spec_markdown: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecChatExcerptInput {
    pub(crate) user: String,
    pub(crate) user_truncated: bool,
    pub(crate) assistant: String,
    pub(crate) assistant_truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecCodeGraphInput {
    pub(crate) indexed_files: i64,
    pub(crate) symbol_count: i64,
    pub(crate) reference_count: i64,
    pub(crate) edge_count: i64,
    pub(crate) languages: Vec<String>,
    pub(crate) files: Vec<WorkspaceSpecFileSummaryInput>,
    pub(crate) symbols: Vec<WorkspaceSpecSymbolInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecFileSummaryInput {
    pub(crate) path: String,
    pub(crate) language: Option<String>,
    pub(crate) symbol_count: i64,
    pub(crate) import_count: i64,
    pub(crate) import_modules: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecSymbolInput {
    pub(crate) path: String,
    pub(crate) language: Option<String>,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) signature: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecMemoryProfileInput {
    pub(crate) id: String,
    pub(crate) scope: String,
    pub(crate) profile_text: String,
    pub(crate) truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSpecSourceFileInput {
    pub(crate) path: String,
    pub(crate) size_bytes: u64,
    pub(crate) content: String,
    pub(crate) truncated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceSpecToolOutput {
    content_markdown: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceSpecUpdateToolOutput {
    update_needed: bool,
    content_markdown: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceSpecUpdateOutput {
    NoUpdateNeeded,
    FullReplacementMarkdown(String),
}

#[derive(Debug)]
struct WorkspaceSpecModelSelection {
    model_id: String,
    provider_id: String,
    provider_config: ProviderConnectionConfig,
    max_output_tokens: u32,
}

pub(crate) async fn run_workspace_spec_job(
    state: AppState,
    workspace_id: String,
    job_id: String,
) -> Result<(), ApiError> {
    let config = config_snapshot(&state)?;
    let workspace_path = workspace_by_id(&config, &workspace_id)?.path.clone();
    run_workspace_spec_jobs(config, workspace_id, workspace_path, job_id).await
}

pub(crate) fn queue_workspace_spec_update_job(
    context: &PreparedChatContext,
    final_state: &str,
) -> Result<(), ApiError> {
    if final_state != "succeeded" || !context.agent_primary_chat_output {
        return Ok(());
    }

    let mut database = WorkspaceDatabase::open_or_create(&context.workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let Some(spec) = database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?
        .filter(|spec| spec.enabled && !spec.content_markdown.trim().is_empty())
    else {
        return Ok(());
    };

    let running_job_exists = database
        .running_workspace_spec_job()
        .map_err(ApiError::from_workspace_error)?
        .is_some();
    if let Some(job) = database
        .queued_workspace_spec_update_job()
        .map_err(ApiError::from_workspace_error)?
    {
        let job_id = job.id;
        drop(database);
        if !running_job_exists {
            spawn_workspace_spec_job(
                context.global_config.clone(),
                context.workspace_id.clone(),
                context.workspace_path.clone(),
                job_id,
            );
        }
        return Ok(());
    }

    let input =
        workspace_spec_update_input(context, &database, spec.revision, &spec.content_markdown)?;
    let input_summary_json = serde_json::to_string(&input).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize workspace spec update input: {source}"
        ))
    })?;
    let job = database
        .insert_workspace_spec_job(NewWorkspaceSpecJob {
            id: &unique_id("workspace-spec-job"),
            trigger_type: WorkspaceSpecTriggerType::ChatCompleted.as_str(),
            chat_id: Some(&context.chat_id),
            run_id: Some(&context.llm_request_id),
            model_id: Some(&context.model_id),
            base_revision: Some(spec.revision),
            input_summary_json: Some(&input_summary_json),
        })
        .map_err(ApiError::from_workspace_error)?;
    let job_id = job.id;
    drop(database);

    if !running_job_exists {
        spawn_workspace_spec_job(
            context.global_config.clone(),
            context.workspace_id.clone(),
            context.workspace_path.clone(),
            job_id,
        );
    }

    Ok(())
}

fn spawn_workspace_spec_job(
    config: GlobalConfig,
    workspace_id: String,
    workspace_path: PathBuf,
    job_id: String,
) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::warn!(
            job_id = %job_id,
            workspace_id = %workspace_id,
            "workspace spec update job queued without an active async runtime"
        );
        return;
    };
    handle.spawn(async move {
        let runtime_workspace_id = workspace_id.clone();
        let runtime_job_id = job_id.clone();
        if let Err(error) =
            run_workspace_spec_jobs(config, workspace_id, workspace_path, job_id).await
        {
            tracing::warn!(
                workspace_id = %runtime_workspace_id,
                job_id = %runtime_job_id,
                error = %error.message,
                "workspace spec background job failed"
            );
        }
    });
}

async fn run_workspace_spec_jobs(
    config: GlobalConfig,
    workspace_id: String,
    workspace_path: PathBuf,
    first_job_id: String,
) -> Result<(), ApiError> {
    let mut next_job_id = Some(first_job_id);
    while let Some(job_id) = next_job_id {
        let result =
            run_workspace_spec_job_inner(&config, &workspace_id, &workspace_path, &job_id).await;
        if let Err(error) = &result {
            mark_workspace_spec_job_failed_at_path(&workspace_path, &job_id, &error.message);
            return result;
        }

        next_job_id = queued_workspace_spec_job_id(&workspace_path)?;
    }

    Ok(())
}

async fn run_workspace_spec_job_inner(
    config: &GlobalConfig,
    workspace_id: &str,
    workspace_path: &std::path::Path,
    job_id: &str,
) -> Result<(), ApiError> {
    let Some(job) = workspace_spec_job_for_run(workspace_path, job_id)? else {
        return Ok(());
    };
    if job.trigger_type == WorkspaceSpecTriggerType::ChatCompleted.as_str() {
        return run_workspace_spec_update_job_inner(config, workspace_id, workspace_path, job)
            .await;
    }

    let Some(prepared) =
        prepare_workspace_spec_generation_job(config, workspace_id, workspace_path, job_id)?
    else {
        return Ok(());
    };

    let tool_arguments = audited_provider_tool_request(
        &prepared.workspace_path,
        &prepared.workspace_id,
        None,
        &prepared.provider_id,
        &prepared.provider_config,
        prepared.request.clone(),
        "workspace spec generation",
        WORKSPACE_SPEC_TOOL_NAME,
        "submit workspace spec tool",
        WORKSPACE_SPEC_TIMEOUT_MS,
        config.app.llm_request_retry_count,
        api_audit_save_details(config),
    )
    .await?;
    let content_markdown = parse_workspace_spec_output(tool_arguments)?;
    apply_workspace_spec_job_output(
        &prepared.workspace_path,
        &prepared.job_id,
        prepared.base_revision,
        &content_markdown,
    )
}

async fn run_workspace_spec_update_job_inner(
    config: &GlobalConfig,
    workspace_id: &str,
    workspace_path: &std::path::Path,
    job: WorkspaceSpecJobRecord,
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let Some(spec) = database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?
        .filter(|spec| spec.enabled && !spec.content_markdown.trim().is_empty())
    else {
        database
            .mark_workspace_spec_job_skipped(&job.id, "workspace_spec_disabled")
            .map_err(ApiError::from_workspace_error)?;
        return Ok(());
    };
    let base_revision = job.base_revision.unwrap_or(spec.revision);
    if WorkspaceSpecWriteDecision::for_job_output(base_revision, spec.revision)
        != WorkspaceSpecWriteDecision::WriteFullReplacement
    {
        database
            .mark_workspace_spec_job_skipped(&job.id, "stale_revision")
            .map_err(ApiError::from_workspace_error)?;
        return Ok(());
    }

    let input_summary: WorkspaceSpecUpdateInput = serde_json::from_str(&job.input_summary_json)
        .map_err(|source| {
            ApiError::internal(format!(
                "invalid persisted workspace spec update input: {source}"
            ))
        })?;
    database
        .mark_workspace_spec_job_running(&job.id)
        .map_err(ApiError::from_workspace_error)?;
    drop(database);

    let model = resolve_workspace_spec_model(config, job.model_id.as_deref())?;
    let request = workspace_spec_update_provider_request(
        &model.model_id,
        &config.app.language,
        model.max_output_tokens,
        &input_summary,
    )?;
    let tool_arguments = audited_provider_tool_request(
        workspace_path,
        workspace_id,
        None,
        &model.provider_id,
        &model.provider_config,
        request,
        "workspace spec update",
        WORKSPACE_SPEC_UPDATE_TOOL_NAME,
        "submit workspace spec update tool",
        WORKSPACE_SPEC_TIMEOUT_MS,
        config.app.llm_request_retry_count,
        api_audit_save_details(config),
    )
    .await?;

    apply_workspace_spec_update_job_output(workspace_path, &job.id, base_revision, tool_arguments)
}

pub(crate) fn apply_workspace_spec_update_job_output(
    workspace_path: &std::path::Path,
    job_id: &str,
    base_revision: u64,
    value: Value,
) -> Result<(), ApiError> {
    match parse_workspace_spec_update_output(value)? {
        WorkspaceSpecUpdateOutput::NoUpdateNeeded => {
            let mut database = WorkspaceDatabase::open_or_create(workspace_path)
                .map_err(ApiError::from_workspace_error)?;
            database
                .mark_workspace_spec_job_skipped(job_id, "no_update_needed")
                .map_err(ApiError::from_workspace_error)?;
            Ok(())
        }
        WorkspaceSpecUpdateOutput::FullReplacementMarkdown(content_markdown) => {
            apply_workspace_spec_job_output(
                workspace_path,
                job_id,
                base_revision,
                &content_markdown,
            )
        }
    }
}

#[cfg(test)]
pub(crate) fn prepare_workspace_spec_job(
    config: &GlobalConfig,
    workspace_id: &str,
    workspace: &foco_store::config::WorkspaceConfig,
    job_id: &str,
) -> Result<Option<PreparedWorkspaceSpecJob>, ApiError> {
    prepare_workspace_spec_generation_job(config, workspace_id, &workspace.path, job_id)
}

fn prepare_workspace_spec_generation_job(
    config: &GlobalConfig,
    workspace_id: &str,
    workspace_path: &std::path::Path,
    job_id: &str,
) -> Result<Option<PreparedWorkspaceSpecJob>, ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let Some(job) = database
        .workspace_spec_job(job_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Err(ApiError::bad_request(format!(
            "workspace spec job was not found: {job_id}"
        )));
    };

    if job.status != WorkspaceSpecJobStatus::Queued.as_str() {
        return Ok(None);
    }
    let spec = database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?;
    let Some(spec) = spec.filter(|spec| spec.enabled) else {
        database
            .mark_workspace_spec_job_skipped(job_id, "workspace_spec_disabled")
            .map_err(ApiError::from_workspace_error)?;
        return Ok(None);
    };
    let base_revision = job.base_revision.unwrap_or(spec.revision);
    if WorkspaceSpecWriteDecision::for_job_output(base_revision, spec.revision)
        != WorkspaceSpecWriteDecision::WriteFullReplacement
    {
        database
            .mark_workspace_spec_job_skipped(job_id, "stale_revision")
            .map_err(ApiError::from_workspace_error)?;
        return Ok(None);
    }

    database
        .mark_workspace_spec_job_running(job_id)
        .map_err(ApiError::from_workspace_error)?;
    let input_summary =
        collect_workspace_spec_input(config, workspace_id, workspace_path, base_revision)?;
    let input_summary_json = serde_json::to_string(&input_summary).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize workspace spec input: {source}"
        ))
    })?;
    database
        .update_workspace_spec_job_input_summary(job_id, &input_summary_json)
        .map_err(ApiError::from_workspace_error)?;

    let model = resolve_workspace_spec_model(config, job.model_id.as_deref())?;
    let request = workspace_spec_provider_request(
        &model.model_id,
        &config.app.language,
        model.max_output_tokens,
        &input_summary,
    )?;

    Ok(Some(PreparedWorkspaceSpecJob {
        workspace_id: workspace_id.to_string(),
        workspace_path: workspace_path.to_path_buf(),
        job_id: job.id,
        base_revision,
        provider_id: model.provider_id,
        provider_config: model.provider_config,
        request,
    }))
}

pub(crate) fn apply_workspace_spec_job_output(
    workspace_path: &std::path::Path,
    job_id: &str,
    base_revision: u64,
    content_markdown: &str,
) -> Result<(), ApiError> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let current = database
        .workspace_spec()
        .map_err(ApiError::from_workspace_error)?
        .ok_or_else(|| ApiError::bad_request("workspace spec row is missing"))?;
    match WorkspaceSpecWriteDecision::for_job_output(base_revision, current.revision) {
        WorkspaceSpecWriteDecision::WriteFullReplacement => {}
        WorkspaceSpecWriteDecision::SkipStaleRevision { reason } => {
            database
                .mark_workspace_spec_job_skipped(job_id, reason)
                .map_err(ApiError::from_workspace_error)?;
            return Ok(());
        }
    }

    let Some(updated) = database
        .update_workspace_spec_generated_content(base_revision, content_markdown)
        .map_err(ApiError::from_workspace_error)?
    else {
        database
            .mark_workspace_spec_job_skipped(job_id, "stale_revision")
            .map_err(ApiError::from_workspace_error)?;
        return Ok(());
    };
    let output_json = json!({
        "revision": updated.revision,
        "contentBytes": content_markdown.len(),
    })
    .to_string();
    database
        .mark_workspace_spec_job_completed(job_id, Some(&output_json))
        .map_err(ApiError::from_workspace_error)?;

    Ok(())
}

fn collect_workspace_spec_input(
    config: &GlobalConfig,
    workspace_id: &str,
    workspace_path: &std::path::Path,
    base_revision: u64,
) -> Result<WorkspaceSpecGenerationInput, ApiError> {
    let database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let context = database
        .code_graph_context()
        .map_err(ApiError::from_workspace_error)?;
    let files = database
        .code_graph_file_summaries(WORKSPACE_SPEC_FILE_SUMMARY_LIMIT)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(file_summary_input)
        .collect();
    let symbols = database
        .find_code_graph_symbols("", None, None, WORKSPACE_SPEC_SYMBOL_LIMIT)
        .map_err(ApiError::from_workspace_error)?
        .into_iter()
        .map(symbol_input)
        .collect();

    Ok(WorkspaceSpecGenerationInput {
        workspace_id: workspace_id.to_string(),
        base_revision,
        code_graph: WorkspaceSpecCodeGraphInput {
            indexed_files: context.indexed_files,
            symbol_count: context.symbols,
            reference_count: context.references,
            edge_count: context.edges,
            languages: context.languages,
            files,
            symbols,
        },
        memory_profiles: workspace_memory_profiles(config, workspace_path)?,
        source_files: root_source_files(workspace_path),
    })
}

fn workspace_spec_update_input(
    context: &PreparedChatContext,
    database: &WorkspaceDatabase,
    current_spec_revision: u64,
    current_spec_markdown: &str,
) -> Result<WorkspaceSpecUpdateInput, ApiError> {
    let user_content = message_content(database, &context.user_message_id)?;
    let assistant_content = message_content(database, &context.assistant_message_id)?;
    let (user, user_truncated) = compact_text(&user_content, WORKSPACE_SPEC_CHAT_EXCERPT_MAX_CHARS);
    let (assistant, assistant_truncated) =
        compact_text(&assistant_content, WORKSPACE_SPEC_CHAT_EXCERPT_MAX_CHARS);
    let code_change_stats = (context.code_change_stats.additions > 0
        || context.code_change_stats.deletions > 0)
        .then_some(context.code_change_stats.clone());

    Ok(WorkspaceSpecUpdateInput {
        workspace_id: context.workspace_id.clone(),
        chat_id: context.chat_id.clone(),
        current_spec_revision,
        user_message_id: context.user_message_id.clone(),
        assistant_message_id: context.assistant_message_id.clone(),
        run_id: context.llm_request_id.clone(),
        code_change_stats,
        chat_excerpt: WorkspaceSpecChatExcerptInput {
            user,
            user_truncated,
            assistant,
            assistant_truncated,
        },
        current_spec_markdown: current_spec_markdown.to_string(),
    })
}

fn message_content(database: &WorkspaceDatabase, message_id: &str) -> Result<String, ApiError> {
    database
        .message(message_id)
        .map_err(ApiError::from_workspace_error)
        .map(|message| message.map(|message| message.content).unwrap_or_default())
}

fn workspace_memory_profiles(
    config: &GlobalConfig,
    workspace_path: &std::path::Path,
) -> Result<Vec<WorkspaceSpecMemoryProfileInput>, ApiError> {
    if !config.memory.enabled {
        return Ok(Vec::new());
    }

    let database = MemoryDatabase::open_workspace_at(workspace_database_path(workspace_path))
        .map_err(ApiError::from_memory_error)?;
    database
        .profiles_for_scope(None, WORKSPACE_SPEC_MEMORY_PROFILE_LIMIT)
        .map_err(ApiError::from_memory_error)
        .map(|profiles| {
            profiles
                .into_iter()
                .map(|profile| {
                    let (profile_text, truncated) = compact_text(
                        &profile.profile_text,
                        WORKSPACE_SPEC_MEMORY_PROFILE_MAX_CHARS,
                    );
                    WorkspaceSpecMemoryProfileInput {
                        id: profile.id,
                        scope: profile.scope,
                        profile_text,
                        truncated,
                    }
                })
                .collect()
        })
}

fn root_source_files(workspace_path: &std::path::Path) -> Vec<WorkspaceSpecSourceFileInput> {
    let mut files = Vec::new();
    for relative_path in ROOT_SOURCE_FILE_CANDIDATES {
        if files.len() >= WORKSPACE_SPEC_ROOT_FILE_LIMIT {
            break;
        }
        let path = workspace_path.join(relative_path);
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let (content, truncated) = compact_text(&content, WORKSPACE_SPEC_SOURCE_FILE_MAX_CHARS);
        files.push(WorkspaceSpecSourceFileInput {
            path: (*relative_path).to_string(),
            size_bytes: metadata.len(),
            content,
            truncated,
        });
    }
    files
}

fn workspace_spec_provider_request(
    model_id: &str,
    app_language: &str,
    max_output_tokens: u32,
    input_summary: &WorkspaceSpecGenerationInput,
) -> Result<NeutralChatRequest, ApiError> {
    let input_json = serde_json::to_string_pretty(input_summary).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize workspace spec evidence: {source}"
        ))
    })?;
    let system_prompt = format!(
        "Generate a concise Project Spec Markdown document from provided evidence. \
Use exactly these sections: # Project Spec, ## Purpose, ## Product Surface, ## Architecture, ## Data And Persistence, ## Runtime Flows, ## UI Contracts, ## Agent And Tool Contracts, ## Operational Constraints, ## Open Questions. \
Prefer facts evidenced by code graph summaries, workspace memory profiles, or root source reads. Put unknowns under Open Questions. Do not invent product claims. Keep the Markdown under {WORKSPACE_SPEC_MAX_MARKDOWN_BYTES} bytes. {} Use the submit_workspace_spec tool exactly once.",
        workspace_spec_language_instruction(app_language)
    );

    Ok(NeutralChatRequest {
        model_id: model_id.to_string(),
        messages: vec![
            neutral_text_message(NeutralChatRole::System, system_prompt),
            neutral_text_message(
                NeutralChatRole::User,
                format!("Evidence JSON:\n{input_json}"),
            ),
        ],
        tools: vec![workspace_spec_tool_definition()],
        thinking_level: None,
        max_output_tokens: Some(max_output_tokens),
        prompt_cache_key: None,
        prompt_cache_retention: None,
    })
}

fn workspace_spec_update_provider_request(
    model_id: &str,
    app_language: &str,
    max_output_tokens: u32,
    input_summary: &WorkspaceSpecUpdateInput,
) -> Result<NeutralChatRequest, ApiError> {
    let input_json = serde_json::to_string_pretty(input_summary).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize workspace spec update input: {source}"
        ))
    })?;
    let system_prompt = format!(
        "Decide whether the Project Spec needs an update after the latest completed chat turn. \
If the turn did not change durable product behavior, architecture, runtime flows, data contracts, commands, settings, or operational constraints, submit updateNeeded=false and contentMarkdown=null. \
If an update is needed, submit a full replacement Project Spec Markdown document using the existing section shape. Preserve accurate existing facts unless the turn supersedes them. Do not invent product claims. Keep the Markdown under {WORKSPACE_SPEC_MAX_MARKDOWN_BYTES} bytes. {} Use the submit_workspace_spec_update tool exactly once.",
        workspace_spec_language_instruction(app_language)
    );

    Ok(NeutralChatRequest {
        model_id: model_id.to_string(),
        messages: vec![
            neutral_text_message(NeutralChatRole::System, system_prompt),
            neutral_text_message(
                NeutralChatRole::User,
                format!("Workspace spec update input JSON:\n{input_json}"),
            ),
        ],
        tools: vec![workspace_spec_update_tool_definition()],
        thinking_level: None,
        max_output_tokens: Some(max_output_tokens),
        prompt_cache_key: None,
        prompt_cache_retention: None,
    })
}

fn workspace_spec_tool_definition() -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: WORKSPACE_SPEC_TOOL_NAME.to_string(),
        description: "Submit the generated Project Spec Markdown.".to_string(),
        strict: true,
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "contentMarkdown": {
                    "type": "string",
                    "description": "Full replacement Markdown for the Project Spec."
                }
            },
            "required": ["contentMarkdown"]
        }),
    }
}

fn workspace_spec_update_tool_definition() -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: WORKSPACE_SPEC_UPDATE_TOOL_NAME.to_string(),
        description: "Submit whether the Project Spec needs an update and, when needed, the full replacement Markdown.".to_string(),
        strict: true,
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "updateNeeded": {
                    "type": "boolean",
                    "description": "True only when the completed chat turn changed durable project spec content."
                },
                "contentMarkdown": {
                    "type": ["string", "null"],
                    "description": "Full replacement Markdown when updateNeeded is true; null when updateNeeded is false."
                }
            },
            "required": ["updateNeeded", "contentMarkdown"]
        }),
    }
}

// ponytail: local mapping is enough for the two supported app languages; extend with SUPPORTED_APP_LANGUAGES.
fn workspace_spec_language_instruction(app_language: &str) -> &'static str {
    match app_language {
        "zh-CN" => {
            "Write the generated Project Spec in Simplified Chinese. Preserve code identifiers, file paths, commands, API names, and proper nouns when translation would reduce accuracy."
        }
        _ => {
            "Write the generated Project Spec in English. Preserve code identifiers, file paths, commands, API names, and proper nouns when translation would reduce accuracy."
        }
    }
}

fn parse_workspace_spec_output(value: Value) -> Result<String, ApiError> {
    let output: WorkspaceSpecToolOutput = serde_json::from_value(value).map_err(|source| {
        ApiError::bad_request(format!(
            "malformed workspace spec generation JSON: {source}"
        ))
    })?;
    let content = output.content_markdown.trim().to_string();
    if content.is_empty() {
        return Err(ApiError::bad_request(
            "workspace spec generation returned empty Markdown",
        ));
    }
    Ok(content)
}

fn parse_workspace_spec_update_output(value: Value) -> Result<WorkspaceSpecUpdateOutput, ApiError> {
    let output: WorkspaceSpecUpdateToolOutput =
        serde_json::from_value(value).map_err(|source| {
            ApiError::bad_request(format!("malformed workspace spec update JSON: {source}"))
        })?;
    if !output.update_needed {
        return Ok(WorkspaceSpecUpdateOutput::NoUpdateNeeded);
    }

    let content = output
        .content_markdown
        .unwrap_or_default()
        .trim()
        .to_string();
    if content.is_empty() {
        return Err(ApiError::bad_request(
            "workspace spec update requested but returned empty Markdown",
        ));
    }
    Ok(WorkspaceSpecUpdateOutput::FullReplacementMarkdown(content))
}

fn resolve_workspace_spec_model(
    config: &GlobalConfig,
    requested_model_id: Option<&str>,
) -> Result<WorkspaceSpecModelSelection, ApiError> {
    let model = match requested_model_id.and_then(non_empty_trimmed) {
        Some(model_id) => config
            .models
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "workspace spec generation model was not found: {model_id}"
                ))
            })?,
        None => only_configured_generation_model(config)?,
    };
    workspace_spec_model_selection(config, model)
}

fn only_configured_generation_model(config: &GlobalConfig) -> Result<&ModelSettings, ApiError> {
    let candidates = config
        .models
        .iter()
        .filter(|model| model.enabled && model.active_provider_id.is_some())
        .collect::<Vec<_>>();
    if candidates.len() == 1 {
        return Ok(candidates[0]);
    }

    Err(ApiError::bad_request(
        "workspace spec generation model is not configured; pass modelId",
    ))
}

fn workspace_spec_model_selection(
    config: &GlobalConfig,
    model: &ModelSettings,
) -> Result<WorkspaceSpecModelSelection, ApiError> {
    if !model.enabled {
        return Err(ApiError::bad_request(format!(
            "workspace spec generation model '{}' is disabled",
            model.id
        )));
    }
    let limits = model.limits.as_ref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "workspace spec generation model '{}' is missing limits",
            model.id
        ))
    })?;
    let provider_id = model.active_provider_id.as_deref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "workspace spec generation model '{}' has no active provider selected",
            model.id
        ))
    })?;
    if !model.provider_ids.iter().any(|id| id == provider_id) {
        return Err(ApiError::bad_request(format!(
            "active provider '{}' is not associated with workspace spec generation model '{}'",
            provider_id, model.id
        )));
    }
    let provider = config
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "workspace spec generation provider '{}' was not found",
                provider_id
            ))
        })?;
    if !provider.enabled {
        return Err(ApiError::bad_request(format!(
            "workspace spec generation provider '{}' is disabled",
            provider.id
        )));
    }
    let max_output_tokens = u32::try_from(limits.max_output_tokens)
        .map_err(|_| {
            ApiError::bad_request(format!(
                "workspace spec generation model '{}' max output tokens exceed u32: {}",
                model.id, limits.max_output_tokens
            ))
        })?
        .min(WORKSPACE_SPEC_MAX_OUTPUT_TOKENS);

    Ok(WorkspaceSpecModelSelection {
        model_id: model.id.clone(),
        provider_id: provider.id.clone(),
        provider_config: provider_connection_config(provider)?,
        max_output_tokens,
    })
}

fn workspace_spec_job_for_run(
    workspace_path: &std::path::Path,
    job_id: &str,
) -> Result<Option<WorkspaceSpecJobRecord>, ApiError> {
    let database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    let Some(job) = database
        .workspace_spec_job(job_id)
        .map_err(ApiError::from_workspace_error)?
    else {
        return Err(ApiError::bad_request(format!(
            "workspace spec job was not found: {job_id}"
        )));
    };
    if job.status != WorkspaceSpecJobStatus::Queued.as_str() {
        return Ok(None);
    }

    Ok(Some(job))
}

fn queued_workspace_spec_job_id(
    workspace_path: &std::path::Path,
) -> Result<Option<String>, ApiError> {
    let database = WorkspaceDatabase::open_or_create(workspace_path)
        .map_err(ApiError::from_workspace_error)?;
    database
        .queued_workspace_spec_job()
        .map_err(ApiError::from_workspace_error)
        .map(|job| job.map(|job| job.id))
}

fn mark_workspace_spec_job_failed_at_path(
    workspace_path: &std::path::Path,
    job_id: &str,
    error_message: &str,
) {
    let Ok(mut database) = WorkspaceDatabase::open_or_create(workspace_path) else {
        return;
    };
    if let Err(error) = database.mark_workspace_spec_job_failed(job_id, error_message) {
        tracing::warn!(
            job_id,
            error = %error,
            "failed to mark workspace spec job failed"
        );
    }
}

fn file_summary_input(summary: CodeGraphFileSummaryRecord) -> WorkspaceSpecFileSummaryInput {
    WorkspaceSpecFileSummaryInput {
        path: summary.path,
        language: summary.language,
        symbol_count: summary.symbol_count,
        import_count: summary.import_count,
        import_modules: summary.import_modules,
    }
}

fn symbol_input(symbol: CodeGraphSymbolRecord) -> WorkspaceSpecSymbolInput {
    WorkspaceSpecSymbolInput {
        path: symbol.path,
        language: symbol.language,
        name: symbol.name,
        kind: symbol.kind,
        signature: symbol.signature,
    }
}

fn compact_text(value: &str, max_chars: usize) -> (String, bool) {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        return (compact, false);
    }
    let mut clipped = compact.chars().take(max_chars).collect::<String>();
    clipped.push_str("...");
    (clipped, true)
}

fn non_empty_trimmed(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}
