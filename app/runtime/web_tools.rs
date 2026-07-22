use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use foco_store::config::{
    WEB_SEARCH_PROVIDER_BRAVE, WEB_SEARCH_PROVIDER_TAVILY, WebSearchSettings,
};
use foco_store::workspace::{WORKSPACE_FOCO_DIR, workspace_foco_dir};
use foco_tools::output_budget::{
    CompleteLinePrefix, CompleteLineTruncateOptions, CompleteLineTruncateOutcome,
    LINE_BOUNDED_FULL_RESULT_PATH_FIELD, LINE_BOUNDED_NEXT_START_LINE_FIELD,
    LINE_BOUNDED_NOTE_FIELD, LINE_BOUNDED_SOFT_BUDGET_EXCEEDED_FIELD, LINE_BOUNDED_TRUNCATED_FIELD,
    TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT, TOOL_OUTPUT_SOFT_BYTE_LIMIT,
    TOOL_OUTPUT_SOFT_LINE_LIMIT, complete_line_count, complete_line_prefix_note,
    for_each_complete_line, measure_tool_execution, peel_last_complete_line,
    soft_limit_array_prefix_len_with_overhead, truncate_to_complete_lines_with_measure,
};
use foco_tools::{ToolExecution, WEB_FETCH_TOOL, WEB_SEARCH_TOOL};
use serde::Deserialize;
use serde_json::{Value, json};

use super::broker_artifacts::{
    BROKERED_WEB_RESULT_MIME, BrokeredTransferFile, MAX_BROKERED_WEB_RESULT_FILE_BYTES,
    MAX_BROKERED_WEB_RESULT_FILE_COUNT, WEB_RESULTS_RELATIVE_DIR, atomic_write_bytes,
    decode_and_verify_transfer_file, ensure_path_under_allowed_root, is_safe_transfer_file_name,
    normalize_workspace_relative_path, package_workspace_file_for_transfer,
};

const DEFAULT_WEB_TOOL_TIMEOUT_MS: u64 = 15_000;
const MAX_WEB_TOOL_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_WEB_SEARCH_RESULT_LIMIT: usize = 5;
const MAX_WEB_SEARCH_RESULT_LIMIT: usize = 10;
const MAX_WEB_FETCH_BYTES: usize = 2 * 1024 * 1024;
const FOCO_WEB_USER_AGENT: &str = "Foco/0.1";

/// Workspace-relative cache directory for large web tool results (credential-free).
const WEB_RESULTS_DIR: &str = "web-results";
const MAX_WEB_RESULT_FILES: usize = 20;
const WEB_RESULT_TTL: Duration = Duration::from_secs(60 * 60);
/// Only prune leftover `.web-*.tmp` files older than this. Fresh temps from concurrent writers
/// must not be deleted mid-publish (otherwise rename/hard_link fails with ENOENT).
const WEB_RESULT_TEMP_ORPHAN_TTL: Duration = Duration::from_secs(15 * 60);
/// Conservative first-pass reserve for sibling metadata (url/title/note/path). Final fit uses
/// measured ToolExecution JSON size, so this is only a lower bound on overhead.
const WEB_RESULT_RESPONSE_OVERHEAD_BYTES: usize = 4 * 1024;
const WEB_RESULT_CACHE_VERSION: u32 = 1;

static WEB_RESULTS_COUNTER: AtomicU64 = AtomicU64::new(0);
/// Serialize prune + publish so process-local concurrent writers cannot exceed the count cap.
static WEB_RESULTS_WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebSearchToolInput {
    query: String,
    max_results: Option<usize>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebFetchToolInput {
    url: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
    timeout_ms: Option<u64>,
}

pub(crate) fn is_web_tool_name(tool_name: &str) -> bool {
    matches!(tool_name, WEB_SEARCH_TOOL | WEB_FETCH_TOOL)
}

/// Package a host-side web cache file referenced by `fullResultPath` for broker transfer.
///
/// Returns `Ok(None)` when the tool result has no cache path (within soft budget).
/// Returns `Err` when a path is claimed but cannot be packaged — callers must not publish
/// a successful ToolResult that points at a missing remote file.
pub(crate) fn package_brokered_web_result_files(
    workspace_path: &Path,
    result: &Value,
) -> Result<Option<Vec<BrokeredTransferFile>>, String> {
    let Some(relative_path) = result
        .get(LINE_BOUNDED_FULL_RESULT_PATH_FIELD)
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    let relative_path = validate_web_result_relative_path(relative_path)?;
    let file = package_workspace_file_for_transfer(
        workspace_path,
        &relative_path,
        BROKERED_WEB_RESULT_MIME,
        MAX_BROKERED_WEB_RESULT_FILE_BYTES,
        WEB_RESULTS_RELATIVE_DIR,
    )?;
    let expected_name = Path::new(&relative_path)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "web result cache path is missing a file name".to_string())?;
    if file.file_name != expected_name {
        return Err(format!(
            "web result cache file name '{}' does not match path '{relative_path}'",
            file.file_name
        ));
    }
    Ok(Some(vec![file]))
}

/// Materialize a brokered web cache file into the sidecar tool workspace and confirm
/// `fullResultPath` is a workspace-relative path under `.foco/web-results/`.
///
/// Does not publish a success result when validation or write fails. Credentials must not
/// appear in the transfer payload (host is responsible for caching credential-free text only).
///
/// Uses the same process-local prune + no-clobber publish critical section as the local
/// `write_web_result_cache` path (1h TTL, 20-file cap). Failures never delete an existing
/// destination cache entry.
pub(crate) fn materialize_brokered_web_result(
    workspace_path: &Path,
    mut result: Value,
    files: Vec<BrokeredTransferFile>,
) -> Result<Value, String> {
    let relative_path = result
        .get(LINE_BOUNDED_FULL_RESULT_PATH_FIELD)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "web broker response claims transferred files but is missing fullResultPath".to_string()
        })?
        .to_string();
    let relative_path = validate_web_result_relative_path(&relative_path)?;

    if files.len() != MAX_BROKERED_WEB_RESULT_FILE_COUNT {
        return Err(format!(
            "brokered web result transfer must contain exactly {MAX_BROKERED_WEB_RESULT_FILE_COUNT} file"
        ));
    }
    let file = files
        .into_iter()
        .next()
        .expect("exactly one web result file");

    if !is_allowed_web_result_mime(&file.mime_type) {
        return Err(format!(
            "brokered web result file '{}' has unsupported MIME type '{}'",
            file.file_name, file.mime_type
        ));
    }
    if !is_safe_transfer_file_name(&file.file_name)
        || !is_valid_web_result_file_name(&file.file_name)
    {
        return Err("brokered web result file name is unsafe".to_string());
    }
    let expected_name = Path::new(&relative_path)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "web result cache path is missing a file name".to_string())?;
    if file.file_name != expected_name {
        return Err(format!(
            "brokered web result file name '{}' does not match fullResultPath '{relative_path}'",
            file.file_name
        ));
    }

    let bytes = decode_and_verify_transfer_file(&file, MAX_BROKERED_WEB_RESULT_FILE_BYTES)?;
    // Cache is credential-free readable text; reject non-UTF-8 to keep read_file safe.
    std::str::from_utf8(&bytes).map_err(|_| {
        format!(
            "brokered web result file '{}' is not valid UTF-8 text",
            file.file_name
        )
    })?;

    // Same critical section as local cache writes: no-clobber check, then prune + publish.
    let _write_guard = WEB_RESULTS_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let results_dir = workspace_foco_dir(workspace_path).join(WEB_RESULTS_DIR);
    fs::create_dir_all(&results_dir).map_err(|source| {
        format!(
            "failed to create sidecar web results directory '{}': {source}",
            results_dir.display()
        )
    })?;

    let destination = workspace_path.join(&relative_path);
    // Ensure destination stays under the workspace root (path already normalized).
    let canonical_workspace = fs::canonicalize(workspace_path).map_err(|source| {
        format!("failed to resolve sidecar workspace for web result: {source}")
    })?;
    // Check destination BEFORE prune: reserve-prune with room_for_new=1 can delete the
    // oldest cache entry when the dir is at the count cap. If that entry is this destination,
    // pruning first would erase a no-clobber collision target and allow overwrite.
    if let Ok(existing) = fs::canonicalize(&destination) {
        if !existing.starts_with(&canonical_workspace) {
            return Err("web result destination escaped the workspace".to_string());
        }
        // Destination already present: do not overwrite. If content matches, treat as
        // idempotent success so a broker replay does not fail a valid cache entry.
        let existing_bytes = fs::read(&destination).map_err(|source| {
            format!(
                "failed to read existing sidecar web result '{}': {source}",
                destination.display()
            )
        })?;
        if existing_bytes == bytes {
            result[LINE_BOUNDED_FULL_RESULT_PATH_FIELD] = json!(relative_path);
            rewrite_web_result_note_if_needed(&mut result, &relative_path);
            return Ok(result);
        }
        return Err(format!(
            "sidecar web result cache path already exists '{}'",
            relative_path
        ));
    } else if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            format!(
                "failed to create sidecar web results directory '{}': {source}",
                parent.display()
            )
        })?;
        let canonical_parent = fs::canonicalize(parent).map_err(|source| {
            format!("failed to resolve sidecar web results directory: {source}")
        })?;
        if !canonical_parent.starts_with(&canonical_workspace) {
            return Err("web result destination escaped the workspace".to_string());
        }
    }

    // Destination does not exist: free TTL-expired entries and reserve room, then publish.
    prune_web_results_dir(&results_dir);

    // No-clobber atomic publish. On failure do NOT delete destination (it should not exist
    // for a new name; if a race created one, leave it alone).
    atomic_write_bytes(&destination, &bytes)?;
    // Post-write exact cap under the same lock (preserve the just-published file).
    prune_web_results_dir_to_cap(&results_dir);

    // Confirm model-facing path is the workspace-relative cache path (never a host temp path).
    result[LINE_BOUNDED_FULL_RESULT_PATH_FIELD] = json!(relative_path);
    rewrite_web_result_note_if_needed(&mut result, &relative_path);
    Ok(result)
}

fn rewrite_web_result_note_if_needed(result: &mut Value, relative_path: &str) {
    if let Some(note) = result.get(LINE_BOUNDED_NOTE_FIELD).and_then(Value::as_str) {
        // Refresh note so it cannot retain a host absolute path if one ever leaked into the note.
        if let Some(next_start) = result
            .get(LINE_BOUNDED_NEXT_START_LINE_FIELD)
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        {
            result[LINE_BOUNDED_NOTE_FIELD] =
                json!(web_result_truncated_note(relative_path, next_start.max(1)));
        } else if note.contains(relative_path) {
            // Keep existing note when it already matches; no nextStartLine to rebuild with.
        } else {
            result[LINE_BOUNDED_NOTE_FIELD] = json!(web_result_truncated_note(relative_path, 1));
        }
    }
}

fn is_allowed_web_result_mime(mime_type: &str) -> bool {
    let normalized = mime_type.trim().to_ascii_lowercase();
    normalized == "text/plain"
        || normalized == BROKERED_WEB_RESULT_MIME
        || normalized.starts_with("text/plain;")
}

/// Validate and normalize a workspace-relative web cache path under `.foco/web-results/`.
pub(crate) fn validate_web_result_relative_path(path: &str) -> Result<String, String> {
    let normalized = normalize_workspace_relative_path(path)?;
    ensure_path_under_allowed_root(&normalized, WEB_RESULTS_RELATIVE_DIR)?;
    let file_name = Path::new(&normalized)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "web result cache path is missing a file name".to_string())?;
    if Path::new(&normalized)
        .parent()
        .map(|parent| parent.to_string_lossy().replace('\\', "/"))
        .as_deref()
        != Some(WEB_RESULTS_RELATIVE_DIR)
    {
        return Err(format!(
            "web result cache path '{normalized}' must be a direct child of {WEB_RESULTS_RELATIVE_DIR}"
        ));
    }
    if !is_valid_web_result_file_name(file_name) {
        return Err(format!(
            "web result cache file name '{file_name}' is invalid"
        ));
    }
    Ok(normalized)
}

fn is_valid_web_result_file_name(file_name: &str) -> bool {
    if !is_safe_transfer_file_name(file_name) {
        return false;
    }
    let Some(stem) = file_name.strip_suffix(".txt") else {
        return false;
    };
    // stem is web-fetch-... or web-search-... (same rules as is_valid_web_result_id)
    is_valid_web_result_id(stem)
}

pub(crate) fn web_tool_timeout_ms(arguments: &Value) -> Result<u64, String> {
    match arguments.get("timeoutMs") {
        Some(Value::Null) | None => Ok(DEFAULT_WEB_TOOL_TIMEOUT_MS),
        Some(Value::Number(timeout_ms)) => {
            let timeout_ms = timeout_ms
                .as_u64()
                .ok_or_else(|| "timeoutMs must be an integer or null".to_string())?;
            if timeout_ms == 0 || timeout_ms > MAX_WEB_TOOL_TIMEOUT_MS {
                Err(format!(
                    "timeoutMs must be between 1 and {MAX_WEB_TOOL_TIMEOUT_MS} milliseconds"
                ))
            } else {
                Ok(timeout_ms)
            }
        }
        Some(_) => Err("timeoutMs must be an integer or null".to_string()),
    }
}

pub(crate) async fn execute_web_tool(
    settings: &WebSearchSettings,
    tool_name: &str,
    arguments: Value,
    timeout: Duration,
    tool_workspace_path: &Path,
) -> Result<Value, String> {
    match tool_name {
        WEB_SEARCH_TOOL => {
            let input = serde_json::from_value::<WebSearchToolInput>(arguments)
                .map_err(|source| format!("web_search arguments do not match schema: {source}"))?;
            execute_web_search(settings, input, timeout, tool_workspace_path).await
        }
        WEB_FETCH_TOOL => {
            let input = serde_json::from_value::<WebFetchToolInput>(arguments)
                .map_err(|source| format!("web_fetch arguments do not match schema: {source}"))?;
            execute_web_fetch(input, timeout, tool_workspace_path).await
        }
        _ => Err(format!("unknown web tool '{tool_name}'")),
    }
}

async fn execute_web_search(
    settings: &WebSearchSettings,
    input: WebSearchToolInput,
    timeout: Duration,
    tool_workspace_path: &Path,
) -> Result<Value, String> {
    web_tool_timeout_ms_from_input(input.timeout_ms)?;
    // Only the FocoFunction path reaches here. Provider-native search is server-side and must
    // never be routed into this executor (no NeutralToolCall for native search).
    if !web_search_function_execution_allowed(settings) {
        return Err(
            "web_search function tool is disabled or missing an active Tavily/Brave API key"
                .to_string(),
        );
    }
    let query = input.query.trim();
    if query.is_empty() {
        return Err("query must not be empty".to_string());
    }
    let max_results = normalize_web_search_limit(input.max_results)?;
    let provider = settings
        .active_fallback_provider()
        .ok_or_else(|| "web_search has no active fallback provider with an API key".to_string())?;
    let api_key = settings
        .api_key_for_provider(provider)
        .ok_or_else(|| format!("web_search provider '{provider}' is missing an API key"))?;
    let mut client_builder = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(FOCO_WEB_USER_AGENT);
    if settings.api_proxy.enabled {
        let proxy = reqwest::Proxy::all(settings.api_proxy.url.trim())
            .map_err(|source| format!("failed to configure web_search proxy: {source}"))?;
        client_builder = client_builder.proxy(proxy);
    }
    let client = client_builder
        .build()
        .map_err(|source| format!("failed to create web_search HTTP client: {source}"))?;
    let results = match provider {
        WEB_SEARCH_PROVIDER_TAVILY => tavily_search(&client, api_key, query, max_results).await?,
        WEB_SEARCH_PROVIDER_BRAVE => brave_search(&client, api_key, query, max_results).await?,
        other => return Err(format!("web_search provider '{other}' is unsupported")),
    };

    finalize_web_search_output(
        tool_workspace_path,
        provider,
        query,
        results,
        timeout.as_millis().min(u128::from(u64::MAX)) as u64,
    )
}

/// Build model-facing web_search output with optional complete-record soft truncation + cache.
///
/// Cache is written only after measuring that the full untruncated ToolExecution exceeds the soft
/// budget. Conservative record-prefix estimates never force truncation on their own.
///
/// Model-facing `query` is part of the soft budget: when results alone cannot fit, the echoed
/// query may be shortened in the preview while the cache keeps the full original query. A
/// `truncated=true` success is returned only when the candidate fits the soft budget, or when a
/// single complete result record remains soft-over but under the hard envelope after the echoed
/// query has already been reduced to an empty preview (soft-over never reintroduces a full
/// oversized query via `truncated=true`).
fn finalize_web_search_output(
    tool_workspace_path: &Path,
    provider: &str,
    query: &str,
    results: Vec<Value>,
    timeout_ms: u64,
) -> Result<Value, String> {
    let total_results = results.len();

    // 1) Measure the complete untruncated response first. No cache when within soft budget.
    let full_candidate = assemble_web_search_response(
        provider,
        query,
        &results,
        total_results,
        total_results,
        timeout_ms,
        None,
    );
    let full_measure = measure_tool_execution(&ToolExecution {
        output: full_candidate.clone(),
        is_error: false,
    })
    .map_err(|source| format!("failed to measure web_search response: {source}"))?;
    let full_within_soft = full_measure.serialized_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT
        && full_measure.text_lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT;
    if full_within_soft {
        return Ok(full_candidate);
    }

    // 2) Soft budget exceeded: write credential-free full cache once, then peel complete records.
    let full_cache_text = render_web_search_cache_text(provider, query, &results);
    let full_result_path = write_web_result_cache(
        tool_workspace_path,
        "search",
        &full_cache_text,
        Some(provider),
        Some(query),
    )?;

    // Initial estimate is only a peel starting point; final fit uses measured ToolExecution size.
    let mut returned_count =
        soft_limit_array_prefix_len_with_overhead(&results, WEB_RESULT_RESPONSE_OVERHEAD_BYTES)
            .map_err(|source| format!("failed to measure web_search results budget: {source}"))?;
    if returned_count == 0 && total_results > 0 {
        returned_count = 1;
    }
    // Full already over soft: never claim "all records" on the truncated path when there are
    // results to peel. Empty result sets still take the truncated path solely to fit metadata.
    if returned_count >= total_results && total_results > 0 {
        returned_count = total_results.saturating_sub(1).max(1);
    }

    for _ in 0..=total_results.saturating_add(2) {
        match try_fit_web_search_truncated_candidate(
            provider,
            query,
            &results,
            total_results,
            returned_count,
            timeout_ms,
            &full_result_path,
        )? {
            WebSearchFitOutcome::WithinSoft(candidate) => return Ok(candidate),
            WebSearchFitOutcome::SingleRecordSoftOver(candidate) => return Ok(candidate),
            WebSearchFitOutcome::HardLimitExceeded { measured_bytes } => {
                return Err(format!(
                    "web_search result exceeds the hard output ceiling ({measured_bytes} bytes measured; max {}). Cannot return a single complete record without splitting it; reduce result size or maxResults.",
                    TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
                ));
            }
            WebSearchFitOutcome::NeedFewerResults => {
                if returned_count <= 1 {
                    // Metadata (query) and zero/one results still cannot form a soft-budget
                    // success and are not a legitimate single-record soft-over exception.
                    return Err(
                        "web_search: unable to fit response under the shared soft output budget even after reducing results and preview query; refine the query or maxResults"
                            .to_string(),
                    );
                }
                returned_count = returned_count.saturating_sub(1).max(1);
            }
        }
    }

    Err("web_search: unable to fit response under the shared soft output budget".to_string())
}

enum WebSearchFitOutcome {
    WithinSoft(Value),
    /// Single complete result record under hard, still over soft after query was reduced to empty.
    SingleRecordSoftOver(Value),
    HardLimitExceeded {
        measured_bytes: usize,
    },
    NeedFewerResults,
}

fn assemble_web_search_response(
    provider: &str,
    query: &str,
    results: &[Value],
    total_results: usize,
    returned_count: usize,
    timeout_ms: u64,
    truncation: Option<(&str, usize)>,
) -> Value {
    let preview_results = results
        .iter()
        .take(returned_count)
        .cloned()
        .collect::<Vec<_>>();
    let mut response = json!({
        "provider": provider,
        "query": query,
        "results": preview_results,
        "totalResults": total_results,
        "returnedResults": returned_count,
        LINE_BOUNDED_TRUNCATED_FIELD: false,
        "timeoutMs": timeout_ms
    });
    if let Some((full_result_path, next_start_line)) = truncation {
        let note = web_result_truncated_note(full_result_path, next_start_line);
        response[LINE_BOUNDED_TRUNCATED_FIELD] = json!(true);
        response[LINE_BOUNDED_NEXT_START_LINE_FIELD] = json!(next_start_line);
        response[LINE_BOUNDED_FULL_RESULT_PATH_FIELD] = json!(full_result_path);
        response[LINE_BOUNDED_NOTE_FIELD] = json!(note);
    }
    response
}

/// Fit a truncated web_search candidate under soft budget by optionally shortening the echoed
/// query. Cache line numbers always use the full original query.
fn try_fit_web_search_truncated_candidate(
    provider: &str,
    full_query: &str,
    results: &[Value],
    total_results: usize,
    returned_count: usize,
    timeout_ms: u64,
    full_result_path: &str,
) -> Result<WebSearchFitOutcome, String> {
    // Prefer the full query. If that overflows soft, binary-search a shorter preview query.
    // When the preview query must be shortened, point nextStartLine at the cache start so the
    // model can recover the full query + remaining results without re-requesting the network.
    let results_next =
        web_search_next_start_line_in_cache(provider, full_query, results, returned_count);

    let try_with_query =
        |display_query: &str, next_start_line: usize| -> Result<(Value, usize, usize), String> {
            let candidate = assemble_web_search_response(
                provider,
                display_query,
                results,
                total_results,
                returned_count,
                timeout_ms,
                Some((full_result_path, next_start_line)),
            );
            let measurement = measure_tool_execution(&ToolExecution {
                output: candidate.clone(),
                is_error: false,
            })
            .map_err(|source| format!("failed to measure web_search response: {source}"))?;
            Ok((
                candidate,
                measurement.serialized_bytes,
                measurement.text_lines,
            ))
        };

    let (full_q_candidate, full_q_bytes, full_q_lines) = try_with_query(full_query, results_next)?;
    if full_q_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT && full_q_lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT {
        return Ok(WebSearchFitOutcome::WithinSoft(full_q_candidate));
    }

    // Binary-search the largest character-prefix of the query that keeps the candidate within soft.
    // Any shortening forces nextStartLine=1 so the full original query is recovered from cache.
    let char_count = full_query.chars().count();
    let mut lo = 0_usize;
    let mut hi = char_count;
    let mut best: Option<Value> = None;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let display_query: String = full_query.chars().take(mid).collect();
        let next_start_line = if mid == char_count { results_next } else { 1 };
        let (candidate, bytes, lines) = try_with_query(&display_query, next_start_line)?;
        if bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT && lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT {
            best = Some(candidate);
            lo = mid.saturating_add(1);
        } else if mid == 0 {
            break;
        } else {
            hi = mid.saturating_sub(1);
        }
    }
    if let Some(candidate) = best {
        return Ok(WebSearchFitOutcome::WithinSoft(candidate));
    }

    // Even an empty query preview cannot fit soft with this many results.
    let (empty_q_candidate, empty_q_bytes, empty_q_lines) = try_with_query("", 1)?;
    if empty_q_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT && empty_q_lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT
    {
        // Should have been found by binary search at mid=0; keep as a safe fallback.
        return Ok(WebSearchFitOutcome::WithinSoft(empty_q_candidate));
    }

    if returned_count == 1 {
        // Soft-over exception only for the unsplittable single result record.
        // Keep the echoed query empty so oversized/multi-line query metadata cannot
        // ride `truncated=true` past the soft budget. Full original query remains in cache
        // (nextStartLine=1 on empty_q_candidate).
        if empty_q_bytes <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT {
            return Ok(WebSearchFitOutcome::SingleRecordSoftOver(empty_q_candidate));
        }
        return Ok(WebSearchFitOutcome::HardLimitExceeded {
            measured_bytes: empty_q_bytes.max(full_q_bytes),
        });
    }

    if empty_q_bytes > TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT && returned_count <= 1 {
        return Ok(WebSearchFitOutcome::HardLimitExceeded {
            measured_bytes: empty_q_bytes,
        });
    }

    Ok(WebSearchFitOutcome::NeedFewerResults)
}

async fn tavily_search(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
    max_results: usize,
) -> Result<Vec<Value>, String> {
    let response = client
        .post("https://api.tavily.com/search")
        .bearer_auth(api_key)
        .json(&json!({
            "query": query,
            "max_results": max_results,
            "search_depth": "basic"
        }))
        .send()
        .await
        .map_err(|source| format!("Tavily search request failed: {source}"))?;
    let status = response.status();
    let body_bytes = read_web_response_body_limited(response, "Tavily search").await?;
    let body = String::from_utf8_lossy(&body_bytes).into_owned();
    if !status.is_success() {
        return Err(format_web_status_error("Tavily search", status, &body));
    }
    let value = serde_json::from_str::<Value>(&body)
        .map_err(|source| format!("failed to parse Tavily response JSON: {source}"))?;
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| "Tavily response is missing results array".to_string())?;

    Ok(results
        .iter()
        .take(max_results)
        .map(|item| {
            json!({
                "title": item.get("title").and_then(Value::as_str).unwrap_or_default(),
                "url": item.get("url").and_then(Value::as_str).unwrap_or_default(),
                "snippet": item
                    .get("content")
                    .or_else(|| item.get("snippet"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                "publishedAt": item
                    .get("published_date")
                    .or_else(|| item.get("publishedAt"))
                    .and_then(Value::as_str),
                "score": item.get("score").and_then(Value::as_f64)
            })
        })
        .collect())
}

async fn brave_search(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
    max_results: usize,
) -> Result<Vec<Value>, String> {
    let mut url = reqwest::Url::parse("https://api.search.brave.com/res/v1/web/search")
        .map_err(|source| format!("invalid Brave search URL: {source}"))?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("count", &max_results.to_string())
        .append_pair("text_decorations", "false");
    let response = client
        .get(url)
        .header("X-Subscription-Token", api_key)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|source| format!("Brave search request failed: {source}"))?;
    let status = response.status();
    let body_bytes = read_web_response_body_limited(response, "Brave search").await?;
    let body = String::from_utf8_lossy(&body_bytes).into_owned();
    if !status.is_success() {
        return Err(format_web_status_error("Brave search", status, &body));
    }
    let value = serde_json::from_str::<Value>(&body)
        .map_err(|source| format!("failed to parse Brave response JSON: {source}"))?;
    let results = value
        .get("web")
        .and_then(|web| web.get("results"))
        .and_then(Value::as_array)
        .ok_or_else(|| "Brave response is missing web.results array".to_string())?;

    Ok(results
        .iter()
        .take(max_results)
        .map(|item| {
            json!({
                "title": item.get("title").and_then(Value::as_str).unwrap_or_default(),
                "url": item.get("url").and_then(Value::as_str).unwrap_or_default(),
                "snippet": item
                    .get("description")
                    .or_else(|| item.get("snippet"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                "publishedAt": item
                    .get("age")
                    .or_else(|| item.get("page_age"))
                    .and_then(Value::as_str),
                "score": null
            })
        })
        .collect())
}

async fn execute_web_fetch(
    input: WebFetchToolInput,
    timeout: Duration,
    tool_workspace_path: &Path,
) -> Result<Value, String> {
    web_tool_timeout_ms_from_input(input.timeout_ms)?;
    let requested_line_range = parse_web_fetch_line_range(input.start_line, input.end_line)?;
    let url = parse_fetch_url(&input.url)?;
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(FOCO_WEB_USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|source| format!("failed to create web_fetch HTTP client: {source}"))?;
    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|source| format!("web_fetch request failed: {source}"))?;
    let final_url = response.url().to_string();
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    // Bound both success and error bodies (incl. chunked 4xx/5xx without Content-Length).
    let bytes = read_web_response_body_limited(response, "web_fetch").await?;
    if !status.is_success() {
        let body = String::from_utf8_lossy(&bytes);
        return Err(format_web_status_error("web_fetch", status, &body));
    }
    let raw_text = String::from_utf8_lossy(&bytes).to_string();
    let (title, text) = if content_type
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("html")
    {
        (html_title(&raw_text), html_to_text(&raw_text))
    } else {
        (None, normalize_web_text(&raw_text))
    };
    let text = text.trim().to_string();

    finalize_web_fetch_output(
        tool_workspace_path,
        WebFetchFinalizeInput {
            requested_url: input.url,
            final_url,
            status: status.as_u16(),
            content_type,
            title,
            full_text: text,
            response_bytes: bytes.len(),
            requested_line_range,
            timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        },
    )
}

struct WebFetchFinalizeInput {
    requested_url: String,
    final_url: String,
    status: u16,
    content_type: Option<String>,
    title: Option<String>,
    full_text: String,
    response_bytes: usize,
    requested_line_range: Option<(usize, usize)>,
    timeout_ms: u64,
}

/// Build model-facing web_fetch output with complete-line soft truncation + optional cache.
///
/// Decision order:
/// 1. Measure the full untruncated ToolExecution (requested range or full body).
/// 2. If within soft budget → return as-is, no cache.
/// 3. Otherwise peel complete lines to fit soft (accounting for path/note), write cache once,
///    and return truncated success with absolute nextStartLine.
fn finalize_web_fetch_output(
    tool_workspace_path: &Path,
    input: WebFetchFinalizeInput,
) -> Result<Value, String> {
    let line_count = web_text_line_count(&input.full_text);
    let (preview_plain, content_start_line, range_start, range_end) =
        if let Some(range) = input.requested_line_range {
            let range = normalize_web_fetch_line_range(range, line_count)?;
            let plain = extract_plain_line_range(&input.full_text, range);
            (plain, range.0, Some(range.0), Some(range.1))
        } else {
            (input.full_text.clone(), 1_usize, None, None)
        };

    let numbered = range_start.is_some();
    let full_prefix = complete_line_prefix_from_plain(&preview_plain, content_start_line);

    // 1) Full untruncated ToolExecution first — never cache when it already fits soft.
    {
        let text = if numbered {
            number_plain_lines(&full_prefix.text, content_start_line)
        } else {
            strip_complete_line_endings_for_preview(&full_prefix.text)
        };
        let full_response = assemble_web_fetch_response(
            &input,
            &text,
            line_count,
            range_start,
            range_end,
            &full_prefix,
            None,
        );
        let full_measure = measure_tool_execution(&ToolExecution {
            output: full_response.clone(),
            is_error: false,
        })
        .map_err(|source| format!("failed to measure web_fetch response: {source}"))?;

        if full_measure.serialized_bytes > TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
            && full_prefix.returned_lines <= 1
        {
            return Err(format!(
                "web_fetch: a single complete line exceeds the hard output ceiling ({} bytes measured; max {}) and cannot be returned without splitting the line. Refine the source or request a different range.",
                full_measure.serialized_bytes, TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
            ));
        }

        let within_soft = full_measure.serialized_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT
            && full_measure.text_lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT;
        if within_soft {
            return Ok(full_response);
        }
    }

    // 2) Soft budget exceeded: bulk complete-line prefix, then envelope-fit with path/note.
    // Reduced content soft budget is only for building the truncated preview after the full
    // untruncated ToolExecution was already measured over soft — not for the cache decision.
    let soft_byte_limit = TOOL_OUTPUT_SOFT_BYTE_LIMIT
        .saturating_sub(WEB_RESULT_RESPONSE_OVERHEAD_BYTES)
        .max(1);
    let hard_byte_limit = TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
        .saturating_sub(WEB_RESULT_RESPONSE_OVERHEAD_BYTES)
        .max(1);
    let soft_line_limit = TOOL_OUTPUT_SOFT_LINE_LIMIT.saturating_sub(8).max(1);
    let options = CompleteLineTruncateOptions {
        soft_byte_limit,
        soft_line_limit,
        hard_byte_limit,
        content_start_line,
    };
    let outcome = if numbered {
        truncate_to_complete_lines_with_measure(&preview_plain, options, |line_no, line| {
            web_fetch_numbered_line_measure(line_no, line)
        })
    } else {
        truncate_to_complete_lines_with_measure(&preview_plain, options, |_line_no, line| {
            json_string_body_len(line)
        })
    };
    let mut prefix = match outcome {
        CompleteLineTruncateOutcome::SingleLineExceedsHardLimit {
            line_number,
            line_bytes,
            hard_limit_bytes,
        } => {
            return Err(format!(
                "web_fetch line {line_number} is {line_bytes} UTF-8 bytes and exceeds the hard output ceiling ({hard_limit_bytes} bytes). Cannot return it without splitting a line; refine the source or request a different range."
            ));
        }
        CompleteLineTruncateOutcome::Full(prefix)
        | CompleteLineTruncateOutcome::Truncated(prefix) => {
            // Do not invent truncated/nextStartLine past EOF when the content prefix is already
            // complete. Envelope fitting peels multi-line Full results or marks softBudgetExceeded
            // for a single soft-over line that is the entire content.
            if !prefix.truncated && prefix.returned_lines == 0 {
                let text = if numbered {
                    number_plain_lines(&prefix.text, content_start_line)
                } else {
                    strip_complete_line_endings_for_preview(&prefix.text)
                };
                return Ok(assemble_web_fetch_response(
                    &input,
                    &text,
                    line_count,
                    range_start,
                    range_end,
                    &prefix,
                    None,
                ));
            }
            prefix
        }
    };

    // Peel with a conservative dummy path before creating the real cache file.
    fit_web_fetch_response_to_envelope_with_path(
        &input,
        &preview_plain,
        content_start_line,
        range_start,
        range_end,
        line_count,
        numbered,
        &mut prefix,
        None,
    )?;

    // Full content returned (including single soft-over line under softBudgetExceeded): no cache.
    if !prefix.truncated {
        let text = if numbered {
            number_plain_lines(&prefix.text, content_start_line)
        } else {
            strip_complete_line_endings_for_preview(&prefix.text)
        };
        return Ok(assemble_web_fetch_response(
            &input,
            &text,
            line_count,
            range_start,
            range_end,
            &prefix,
            None,
        ));
    }

    // Cache always stores the full credential-free readable body (not just the range).
    let full_result_path = write_web_result_cache(
        tool_workspace_path,
        "fetch",
        &input.full_text,
        None,
        Some(&input.requested_url),
    )?;

    // Re-fit with the real fullResultPath / note lengths so JSON envelope stays within budget.
    fit_web_fetch_response_to_envelope_with_path(
        &input,
        &preview_plain,
        content_start_line,
        range_start,
        range_end,
        line_count,
        numbered,
        &mut prefix,
        Some(full_result_path.as_str()),
    )?;

    let text = if numbered {
        number_plain_lines(&prefix.text, content_start_line)
    } else {
        strip_complete_line_endings_for_preview(&prefix.text)
    };
    Ok(assemble_web_fetch_response(
        &input,
        &text,
        line_count,
        range_start,
        range_end,
        &prefix,
        Some(full_result_path),
    ))
}

/// Build a non-truncated complete-line prefix covering the entire plain slice.
fn complete_line_prefix_from_plain(plain: &str, content_start_line: usize) -> CompleteLinePrefix {
    let start = content_start_line.max(1);
    let returned_lines = complete_line_count(plain);
    let last_returned_line = if returned_lines == 0 {
        start
    } else {
        start.saturating_add(returned_lines).saturating_sub(1)
    };
    CompleteLinePrefix {
        text: plain.to_string(),
        returned_lines,
        last_returned_line,
        next_start_line: None,
        truncated: false,
        soft_budget_exceeded: false,
    }
}

fn assemble_web_fetch_response(
    input: &WebFetchFinalizeInput,
    text: &str,
    line_count: usize,
    range_start: Option<usize>,
    range_end: Option<usize>,
    prefix: &CompleteLinePrefix,
    full_result_path: Option<String>,
) -> Value {
    let mut response = json!({
        "url": input.requested_url,
        "finalUrl": input.final_url,
        "status": input.status,
        "contentType": input.content_type,
        "title": input.title,
        "text": text,
        LINE_BOUNDED_TRUNCATED_FIELD: prefix.truncated,
        "bytes": input.response_bytes,
        "lineCount": line_count,
        "startLine": range_start,
        "endLine": range_end,
        "timeoutMs": input.timeout_ms
    });
    if prefix.soft_budget_exceeded {
        response[LINE_BOUNDED_SOFT_BUDGET_EXCEEDED_FIELD] = json!(true);
        response["returnedLines"] = json!(prefix.returned_lines);
        response["lastReturnedLine"] = json!(prefix.last_returned_line);
        if let Some(path) = full_result_path.as_ref() {
            response[LINE_BOUNDED_FULL_RESULT_PATH_FIELD] = json!(path);
        }
        response[LINE_BOUNDED_NOTE_FIELD] = json!(complete_line_prefix_note(prefix));
    } else if prefix.truncated {
        let next_start_line = prefix
            .next_start_line
            .unwrap_or(prefix.last_returned_line.saturating_add(1));
        response[LINE_BOUNDED_NEXT_START_LINE_FIELD] = json!(next_start_line);
        response["returnedLines"] = json!(prefix.returned_lines);
        response["lastReturnedLine"] = json!(prefix.last_returned_line);
        if let Some(path) = full_result_path {
            let note = web_result_truncated_note(&path, next_start_line);
            response[LINE_BOUNDED_FULL_RESULT_PATH_FIELD] = json!(path);
            response[LINE_BOUNDED_NOTE_FIELD] = json!(note);
        }
    }
    response
}

fn fit_web_fetch_response_to_envelope_with_path(
    input: &WebFetchFinalizeInput,
    full_preview: &str,
    content_start_line: usize,
    range_start: Option<usize>,
    range_end: Option<usize>,
    line_count: usize,
    numbered: bool,
    prefix: &mut CompleteLinePrefix,
    full_result_path: Option<&str>,
) -> Result<(), String> {
    // Dummy path sized like a real cache id so first-pass peel leaves room for the published path.
    let dummy_path = format!(
        "{WORKSPACE_FOCO_DIR}/{WEB_RESULTS_DIR}/web-fetch-99999999999999999999-9999999999-0123456789abcdef.txt"
    );

    for _ in 0..=TOOL_OUTPUT_SOFT_LINE_LIMIT.saturating_add(2) {
        let text = if numbered {
            number_plain_lines(&prefix.text, content_start_line)
        } else {
            strip_complete_line_endings_for_preview(&prefix.text)
        };
        let path_for_measure = if prefix.truncated || prefix.soft_budget_exceeded {
            Some(
                full_result_path
                    .map(str::to_string)
                    .unwrap_or_else(|| dummy_path.clone()),
            )
        } else {
            None
        };
        let response = assemble_web_fetch_response(
            input,
            &text,
            line_count,
            range_start,
            range_end,
            prefix,
            path_for_measure,
        );
        let measurement = measure_tool_execution(&ToolExecution {
            output: response,
            is_error: false,
        })
        .map_err(|source| format!("failed to measure web_fetch response: {source}"))?;

        let within_soft = measurement.serialized_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT
            && measurement.text_lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT;
        if within_soft {
            return Ok(());
        }

        if measurement.serialized_bytes > TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT {
            if prefix.returned_lines <= 1 {
                return Err(format!(
                    "web_fetch: a single complete line exceeds the hard output ceiling ({} bytes measured; max {}) and cannot be returned without splitting the line. Refine the source or request a different range.",
                    measurement.serialized_bytes, TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT
                ));
            }
        } else if prefix.returned_lines <= 1 {
            // Soft-over single complete line under hard.
            if prefix.truncated {
                return Ok(());
            }
            // Entire content is this one soft-over line: no fake nextStartLine past EOF.
            if !prefix.soft_budget_exceeded {
                prefix.soft_budget_exceeded = true;
                prefix.next_start_line = None;
                continue;
            }
            return Ok(());
        }

        let Some(peeled) = peel_last_complete_line(&prefix.text) else {
            return Ok(());
        };
        if peeled.len() == prefix.text.len() {
            return Ok(());
        }
        prefix.text = peeled.to_string();
        prefix.returned_lines = prefix.returned_lines.saturating_sub(1);
        prefix.last_returned_line = prefix
            .last_returned_line
            .saturating_sub(1)
            .max(content_start_line);
        prefix.truncated = true;
        prefix.soft_budget_exceeded = false;
        prefix.next_start_line = Some(prefix.last_returned_line.saturating_add(1));
        if prefix.returned_lines == 0 && !full_preview.is_empty() {
            return Err(
                "web_fetch: unable to fit any complete line under the shared soft output budget after accounting for metadata. Use a narrower range."
                    .to_string(),
            );
        }
    }

    Err("web_fetch: unable to fit response under the shared soft output budget".to_string())
}

/// UTF-8 length of a JSON string body (without surrounding quotes), matching serde_json escaping.
fn json_string_body_len(s: &str) -> usize {
    let mut len = 0_usize;
    for byte in s.bytes() {
        len = len.saturating_add(match byte {
            b'"' | b'\\' | b'\n' | b'\r' | b'\t' => 2,
            0x00..=0x1F => 6, // \u00XX
            _ => 1,
        });
    }
    len
}

/// Numbered line cost in the final JSON string body: `"<lineNo>\t"` + body with escape expansion.
fn web_fetch_numbered_line_measure(line_no: usize, line: &str) -> usize {
    let body = line.trim_end_matches(['\r', '\n']);
    let mut prefix = line_no.to_string();
    prefix.push('\t');
    // Join separator `\n` between numbered lines is escaped as two JSON bytes.
    json_string_body_len(&prefix)
        .saturating_add(json_string_body_len(body))
        .saturating_add(2)
}

/// Convert complete-line slices (which may retain trailing endings) into the model preview form
/// that joins lines with `\n` and omits a trailing final newline (matching historical web_fetch).
fn strip_complete_line_endings_for_preview(plain: &str) -> String {
    if plain.is_empty() {
        return String::new();
    }
    let mut parts = Vec::new();
    for_each_complete_line(plain, |_, line| {
        parts.push(line.trim_end_matches(['\r', '\n']));
        true
    });
    parts.join("\n")
}

/// Numbered-line soft truncation: each line's cost includes absolute line number + tab.
fn number_plain_lines(plain: &str, start_line: usize) -> String {
    if plain.is_empty() {
        return String::new();
    }
    let mut parts = Vec::new();
    for_each_complete_line(plain, |offset, line| {
        let body = line.trim_end_matches(['\r', '\n']);
        parts.push(format!("{}\t{body}", start_line.saturating_add(offset)));
        true
    });
    parts.join("\n")
}

fn extract_plain_line_range(text: &str, range: (usize, usize)) -> String {
    let mut parts = Vec::new();
    for_each_complete_line(text, |offset, line| {
        let line_number = offset + 1;
        if line_number > range.1 {
            return false;
        }
        if line_number >= range.0 {
            parts.push(line.trim_end_matches(['\r', '\n']));
        }
        true
    });
    parts.join("\n")
}

fn render_web_search_cache_text(provider: &str, query: &str, results: &[Value]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Foco web_search result v{WEB_RESULT_CACHE_VERSION}\n"
    ));
    out.push_str(&format!("provider: {provider}\n"));
    out.push_str(&format!("query: {query}\n"));
    out.push_str(&format!("totalResults: {}\n", results.len()));
    out.push('\n');
    for (index, result) in results.iter().enumerate() {
        out.push_str(&format!("## result {}\n", index + 1));
        out.push_str(&format!(
            "title: {}\n",
            result
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ));
        out.push_str(&format!(
            "url: {}\n",
            result
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ));
        out.push_str(&format!(
            "snippet: {}\n",
            result
                .get("snippet")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ));
        if let Some(published) = result.get("publishedAt").and_then(Value::as_str) {
            out.push_str(&format!("publishedAt: {published}\n"));
        }
        if let Some(score) = result.get("score").and_then(Value::as_f64) {
            out.push_str(&format!("score: {score}\n"));
        }
        out.push('\n');
    }
    out
}

/// 1-based line of the first cache line belonging to result index `returned_count` (0-based
/// first omitted). When all results returned, returns total lines + 1.
fn web_search_next_start_line_in_cache(
    provider: &str,
    query: &str,
    results: &[Value],
    returned_count: usize,
) -> usize {
    if returned_count >= results.len() {
        let full = render_web_search_cache_text(provider, query, results);
        return web_text_line_count(&full).saturating_add(1).max(1);
    }
    let prefix = render_web_search_cache_text(provider, query, &results[..returned_count]);
    // Next line after the last line of the returned-results prefix.
    web_text_line_count(&prefix).saturating_add(1).max(1)
}

fn web_result_truncated_note(full_result_path: &str, next_start_line: usize) -> String {
    format!(
        "Result truncated at a complete line/record boundary under the shared soft output budget (max {TOOL_OUTPUT_SOFT_BYTE_LIMIT} bytes or {TOOL_OUTPUT_SOFT_LINE_LIMIT} lines). Full credential-free result is cached at '{full_result_path}'. Continue with read_file path='{full_result_path}' startLine={next_start_line} and a non-null inclusive endLine; do not re-request the network. This is an explicit truncated success (is_error=false), not hidden data loss."
    )
}

fn write_web_result_cache(
    tool_workspace_path: &Path,
    kind: &str,
    body: &str,
    provider: Option<&str>,
    label: Option<&str>,
) -> Result<String, String> {
    // Process-local mutex: prune + publish is atomic so concurrent writers cannot race past the
    // count cap. Cross-process races remain best-effort (same as search_text snapshots).
    let _write_guard = WEB_RESULTS_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let results_dir = workspace_foco_dir(tool_workspace_path).join(WEB_RESULTS_DIR);
    fs::create_dir_all(&results_dir).map_err(|source| {
        format!(
            "failed to create web results cache directory '{}': {source}",
            results_dir.display()
        )
    })?;
    prune_web_results_dir(&results_dir);

    // Optional sidecar metadata reserved for future diagnostics; body is authoritative.
    let _ = (provider, label);

    // Exclusive create + random entropy so concurrent Foco processes cannot overwrite each other.
    const MAX_ID_ATTEMPTS: usize = 32;
    let mut last_error = String::from("failed to allocate a unique web result cache id");
    for _ in 0..MAX_ID_ATTEMPTS {
        let result_id = next_web_result_id(kind);
        if !is_valid_web_result_id(&result_id) {
            return Err("generated web result cache id is invalid".to_string());
        }
        let relative_path = format!("{WORKSPACE_FOCO_DIR}/{WEB_RESULTS_DIR}/{result_id}.txt");
        let file_path = results_dir.join(format!("{result_id}.txt"));
        let temporary_path = results_dir.join(format!(".{result_id}.tmp"));

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(mut file) => {
                if let Err(source) = file.write_all(body.as_bytes()) {
                    let _ = fs::remove_file(&temporary_path);
                    return Err(format!(
                        "failed to write web result cache temporary file '{}': {source}",
                        temporary_path.display()
                    ));
                }
                if let Err(source) = file.sync_all() {
                    let _ = fs::remove_file(&temporary_path);
                    return Err(format!(
                        "failed to sync web result cache temporary file '{}': {source}",
                        temporary_path.display()
                    ));
                }
                drop(file);

                // Atomic no-clobber publish: hard_link fails with AlreadyExists if dest exists
                // (unlike rename, which replaces on POSIX). Same-directory hard links work on
                // local workspaces; readers only see a complete file after the link succeeds.
                match publish_web_result_cache_file(&temporary_path, &file_path) {
                    Ok(()) => {
                        // Enforce exact cap after publish (room=0) under the write lock.
                        prune_web_results_dir_to_cap(&results_dir);
                        return Ok(relative_path);
                    }
                    Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                        let _ = fs::remove_file(&temporary_path);
                        last_error = format!(
                            "web result cache path already exists '{}'",
                            file_path.display()
                        );
                        continue;
                    }
                    Err(source) => {
                        let _ = fs::remove_file(&temporary_path);
                        return Err(format!(
                            "failed to finalize web result cache file '{}': {source}",
                            file_path.display()
                        ));
                    }
                }
            }
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                last_error = format!(
                    "web result cache temporary path already exists '{}'",
                    temporary_path.display()
                );
                continue;
            }
            Err(source) => {
                return Err(format!(
                    "failed to create web result cache temporary file '{}': {source}",
                    temporary_path.display()
                ));
            }
        }
    }

    Err(last_error)
}

/// Publish a fully written temp file to `destination` without replacing an existing file.
///
/// Uses same-directory `hard_link` so an existing destination fails atomically with
/// `AlreadyExists` (POSIX `rename` would overwrite). On success the temporary name is removed.
fn publish_web_result_cache_file(temporary_path: &Path, destination: &Path) -> io::Result<()> {
    fs::hard_link(temporary_path, destination)?;
    // Best-effort: destination is already durable via the new directory entry.
    let _ = fs::remove_file(temporary_path);
    Ok(())
}

fn next_web_result_id(kind: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let counter = WEB_RESULTS_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut entropy = [0u8; 8];
    // Best-effort random; zeros still ok with create_new retry loop.
    let _ = getrandom::fill(&mut entropy);
    let entropy_hex = entropy
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let kind = if kind == "search" || kind == "fetch" {
        kind
    } else {
        "web"
    };
    format!("web-{kind}-{nanos}-{counter}-{entropy_hex}")
}

fn is_valid_web_result_id(result_id: &str) -> bool {
    let Some(name) = result_id.strip_prefix("web-") else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

/// Best-effort cleanup: TTL + count cap. Failures ignored intentionally.
///
/// Temporary `.web-*.tmp` files are only removed when older than
/// [`WEB_RESULT_TEMP_ORPHAN_TTL`], so concurrent writers that still hold an open temp are not
/// unlinked mid-publish.
///
/// `room_for_new` reserves slots for upcoming publishes (pre-write uses 1; post-write uses 0 so the
/// just-published file is not evicted solely to keep spare capacity).
fn prune_web_results_dir(results_dir: &Path) {
    prune_web_results_dir_inner(results_dir, 1);
}

fn prune_web_results_dir_to_cap(results_dir: &Path) {
    prune_web_results_dir_inner(results_dir, 0);
}

fn prune_web_results_dir_inner(results_dir: &Path, room_for_new: usize) {
    let Ok(read_dir) = fs::read_dir(results_dir) else {
        return;
    };

    let now = SystemTime::now();
    let mut files: Vec<(PathBuf, SystemTime)> = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");

        if file_name.starts_with(".web-") && file_name.ends_with(".tmp") {
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            if now
                .duration_since(modified)
                .is_ok_and(|age| age > WEB_RESULT_TEMP_ORPHAN_TTL)
            {
                let _ = fs::remove_file(&path);
            }
            continue;
        }

        let is_result_file = file_name.starts_with("web-")
            && file_name.ends_with(".txt")
            && !file_name.contains(".tmp");
        if !is_result_file {
            continue;
        }

        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        if now
            .duration_since(modified)
            .is_ok_and(|age| age > WEB_RESULT_TTL)
        {
            let _ = fs::remove_file(&path);
            continue;
        }

        files.push((path, modified));
    }

    let keep = MAX_WEB_RESULT_FILES.saturating_sub(room_for_new);
    if files.len() <= keep {
        return;
    }

    files.sort_by_key(|(_, modified)| *modified);
    let remove_count = files.len() - keep;
    for (path, _) in files.into_iter().take(remove_count) {
        let _ = fs::remove_file(path);
    }
}

pub(crate) fn web_search_enabled(settings: &WebSearchSettings) -> bool {
    // Master switch alone is not enough for the Foco function path: Tavily/Brave still need a key.
    // Provider-native search is gated separately via resolve_web_search_route / ProviderNative tools.
    settings.enabled && settings.fallback_available()
}

/// True when the Foco function `web_search` executor may run (master switch + active fallback key).
pub(crate) fn web_search_function_execution_allowed(settings: &WebSearchSettings) -> bool {
    web_search_enabled(settings)
}

fn normalize_web_search_limit(limit: Option<usize>) -> Result<usize, String> {
    let limit = limit.unwrap_or(DEFAULT_WEB_SEARCH_RESULT_LIMIT);
    if !(1..=MAX_WEB_SEARCH_RESULT_LIMIT).contains(&limit) {
        return Err(format!(
            "maxResults must be between 1 and {MAX_WEB_SEARCH_RESULT_LIMIT}"
        ));
    }

    Ok(limit)
}

fn web_tool_timeout_ms_from_input(timeout_ms: Option<u64>) -> Result<u64, String> {
    match timeout_ms {
        None => Ok(DEFAULT_WEB_TOOL_TIMEOUT_MS),
        Some(timeout_ms) if timeout_ms > 0 && timeout_ms <= MAX_WEB_TOOL_TIMEOUT_MS => {
            Ok(timeout_ms)
        }
        Some(_) => Err(format!(
            "timeoutMs must be between 1 and {MAX_WEB_TOOL_TIMEOUT_MS} milliseconds"
        )),
    }
}

fn parse_fetch_url(value: &str) -> Result<reqwest::Url, String> {
    let url =
        reqwest::Url::parse(value.trim()).map_err(|source| format!("invalid URL: {source}"))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        scheme => Err(format!(
            "web_fetch only supports http and https URLs, got '{scheme}'"
        )),
    }
}

fn parse_web_fetch_line_range(
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<Option<(usize, usize)>, String> {
    match (start_line, end_line) {
        (None, None) => Ok(None),
        (Some(start), Some(end)) if start > 0 && start <= end => Ok(Some((start, end))),
        (Some(_), Some(_)) => Err(
            "startLine and endLine must be a 1-based inclusive range with startLine <= endLine"
                .to_string(),
        ),
        _ => Err(
            "startLine and endLine must both be null for full-page fetches or both be integers for ranged fetches"
                .to_string(),
        ),
    }
}

fn normalize_web_fetch_line_range(
    range: (usize, usize),
    line_count: usize,
) -> Result<(usize, usize), String> {
    if line_count == 0 || range.0 > line_count {
        return Err(format!(
            "web_fetch line range {}-{} is outside the readable text; text has {line_count} lines",
            range.0, range.1
        ));
    }

    Ok((range.0, range.1.min(line_count)))
}

fn web_text_line_count(text: &str) -> usize {
    complete_line_count(text)
}

fn format_web_status_error(context: &str, status: reqwest::StatusCode, body: &str) -> String {
    let preview = body.trim();
    if preview.is_empty() {
        format!("{context} returned HTTP {status}")
    } else {
        let (preview, _) = truncate_chars(preview.to_string(), 800);
        format!("{context} returned HTTP {status}: {preview}")
    }
}

/// Read an HTTP response body with the shared raw web response size ceiling.
///
/// Applies to success and error statuses, including chunked transfers without Content-Length.
async fn read_web_response_body_limited(
    mut response: reqwest::Response,
    context: &str,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_WEB_FETCH_BYTES as u64)
    {
        return Err(format!(
            "{context} response is too large to read (max {MAX_WEB_FETCH_BYTES} bytes)"
        ));
    }

    let mut body = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_WEB_FETCH_BYTES as u64) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|source| format!("failed to read {context} response: {source}"))?
    {
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| format!("{context} response size overflowed while reading"))?;
        if next_len > MAX_WEB_FETCH_BYTES {
            return Err(format!(
                "{context} response is too large to read (exceeded {MAX_WEB_FETCH_BYTES} bytes)"
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let start = lower[start..].find('>').map(|offset| start + offset + 1)?;
    let end = lower[start..]
        .find("</title>")
        .map(|offset| start + offset)?;
    let title = html[start..end].trim();
    (!title.is_empty()).then(|| decode_basic_html_entities(title))
}

fn html_to_text(html: &str) -> String {
    let without_scripts = regex::Regex::new("(?is)<script\\b[^>]*>.*?</script>")
        .expect("valid script regex")
        .replace_all(html, " ");
    let without_styles = regex::Regex::new("(?is)<style\\b[^>]*>.*?</style>")
        .expect("valid style regex")
        .replace_all(&without_scripts, " ");
    let with_breaks = regex::Regex::new("(?i)<\\s*(br|p|div|li|h[1-6]|tr)\\b[^>]*>")
        .expect("valid block regex")
        .replace_all(&without_styles, "\n");
    let without_tags = regex::Regex::new("(?is)<[^>]+>")
        .expect("valid tag regex")
        .replace_all(&with_breaks, " ");
    normalize_web_text(&decode_basic_html_entities(&without_tags))
}

fn decode_basic_html_entities(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

fn normalize_web_text(value: &str) -> String {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_chars(value: String, max_chars: usize) -> (String, bool) {
    if value.chars().count() <= max_chars {
        return (value, false);
    }

    (value.chars().take(max_chars).collect(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Response, StatusCode, header};
    use axum::routing::get;
    use foco_tools::ToolExecution;
    use foco_tools::output_budget::{
        TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES, TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT,
        TOOL_OUTPUT_SOFT_BYTE_LIMIT, ToolOutputSemantics, complete_line_count,
        for_each_complete_line, is_line_bounded_budget_success, measure_tool_execution,
        normalize_tool_execution,
    };
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    fn large_body_lines(line_count: usize, line_width: usize) -> String {
        (0..line_count)
            .map(|i| format!("line-{i:05}-{}", "x".repeat(line_width)))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn web_fetch_small_result_is_not_truncated_and_writes_no_cache() {
        let workspace = tempfile::tempdir().expect("workspace");
        let body = "hello\nworld\n";
        let output = finalize_web_fetch_output(
            workspace.path(),
            WebFetchFinalizeInput {
                requested_url: "https://example.test/page".to_string(),
                final_url: "https://example.test/page".to_string(),
                status: 200,
                content_type: Some("text/plain".to_string()),
                title: None,
                full_text: body.trim().to_string(),
                response_bytes: body.len(),
                requested_line_range: None,
                timeout_ms: 15_000,
            },
        )
        .expect("finalize");

        assert_eq!(output["truncated"], false);
        assert!(output.get("fullResultPath").is_none() || output["fullResultPath"].is_null());
        assert_eq!(output["text"], "hello\nworld");
        let cache_dir = workspace.path().join(".foco/web-results");
        assert!(
            !cache_dir.exists()
                || fs::read_dir(&cache_dir)
                    .map(|entries| entries.count() == 0)
                    .unwrap_or(true),
            "small result must not create cache files"
        );
    }

    #[test]
    fn web_fetch_large_result_caches_full_body_and_returns_complete_line_preview() {
        let workspace = tempfile::tempdir().expect("workspace");
        let full_text = large_body_lines(3_000, 40);
        assert!(full_text.len() > TOOL_OUTPUT_SOFT_BYTE_LIMIT);

        let output = finalize_web_fetch_output(
            workspace.path(),
            WebFetchFinalizeInput {
                requested_url: "https://example.test/big".to_string(),
                final_url: "https://example.test/big".to_string(),
                status: 200,
                content_type: Some("text/plain".to_string()),
                title: Some("Big".to_string()),
                full_text: full_text.clone(),
                response_bytes: full_text.len(),
                requested_line_range: None,
                timeout_ms: 15_000,
            },
        )
        .expect("finalize large");

        assert_eq!(output["truncated"], true);
        assert!(output["is_error"].is_null() || !output["is_error"].as_bool().unwrap_or(false));
        let full_path = output["fullResultPath"].as_str().expect("fullResultPath");
        assert!(
            full_path.starts_with(".foco/web-results/web-fetch-"),
            "path={full_path}"
        );
        assert!(full_path.ends_with(".txt"));
        let next = output["nextStartLine"].as_u64().expect("nextStartLine") as usize;
        assert!(next >= 2, "nextStartLine={next}");
        let note = output["note"].as_str().expect("note");
        assert!(note.contains("read_file"));
        assert!(note.contains(full_path));
        assert!(note.contains(&format!("startLine={next}")));
        assert!(!note.contains("re-request") || note.contains("do not re-request"));

        let preview = output["text"].as_str().expect("text");
        assert!(preview.len() <= TOOL_OUTPUT_SOFT_BYTE_LIMIT);
        assert!(!preview.is_empty());
        // Complete lines only: preview is a prefix of full_text at a line boundary.
        assert!(
            full_text.starts_with(preview.trim_end_matches('\n')) || full_text.starts_with(preview),
            "preview must be a complete-line prefix of full text"
        );

        let cache_abs = workspace.path().join(full_path);
        let cached = fs::read_to_string(&cache_abs).expect("read cache");
        assert_eq!(cached, full_text, "cache must contain full readable body");
        assert!(!cached.contains("Authorization"));
        assert!(!cached.contains("api_key"));
        assert!(!cached.contains("Bearer "));

        // nextStartLine continues without gap/duplication when reading remaining cache lines.
        let all_lines: Vec<&str> = full_text.lines().collect();
        let preview_lines: Vec<&str> = preview.lines().collect();
        assert_eq!(preview_lines.len() + 1, next);
        assert_eq!(preview_lines.as_slice(), &all_lines[..preview_lines.len()]);
        assert_eq!(all_lines[next - 1], all_lines[preview_lines.len()]);

        let execution = ToolExecution {
            output: output.clone(),
            is_error: false,
        };
        let measured = measure_tool_execution(&execution).expect("measure");
        assert!(
            measured.serialized_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT
                || is_line_bounded_budget_success(&execution),
            "serialized={} soft={}",
            measured.serialized_bytes,
            TOOL_OUTPUT_SOFT_BYTE_LIMIT
        );
        // When truncated with fullResultPath, normalize must keep success (not hard-limit fallback).
        assert!(is_line_bounded_budget_success(&execution));
        let normalized =
            normalize_tool_execution(WEB_FETCH_TOOL, ToolOutputSemantics::ReadOnly, execution);
        assert!(!normalized.execution.is_error);
        assert_eq!(normalized.execution.output["truncated"], true);
        assert!(
            measure_tool_execution(&normalized.execution)
                .expect("remeasure")
                .serialized_bytes
                <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT + TOOL_EXECUTION_ENVELOPE_RESERVE_BYTES
        );
    }

    #[test]
    fn web_fetch_json_escape_heavy_body_stays_within_soft_envelope() {
        let workspace = tempfile::tempdir().expect("workspace");
        // Backslashes/quotes double in JSON; raw ~48KiB can serialize near ~96KiB without fit.
        let heavy = "\\\"".repeat(400);
        let full_text = (0..2_500)
            .map(|i| format!("row-{i:04}-{heavy}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(full_text.len() > TOOL_OUTPUT_SOFT_BYTE_LIMIT / 2);

        let output = finalize_web_fetch_output(
            workspace.path(),
            WebFetchFinalizeInput {
                requested_url: "https://example.test/escape".to_string(),
                final_url: "https://example.test/escape".to_string(),
                status: 200,
                content_type: Some("text/plain".to_string()),
                title: None,
                full_text: full_text.clone(),
                response_bytes: full_text.len(),
                requested_line_range: None,
                timeout_ms: 1_000,
            },
        )
        .expect("finalize escape-heavy");

        assert_eq!(output["truncated"], true);
        assert!(output["fullResultPath"].as_str().is_some());
        let execution = ToolExecution {
            output: output.clone(),
            is_error: false,
        };
        assert!(is_line_bounded_budget_success(&execution));
        let measured = measure_tool_execution(&execution).expect("measure");
        assert!(
            measured.serialized_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT,
            "escape-heavy response must fit soft after envelope-aware peel: {}",
            measured.serialized_bytes
        );
        let normalized =
            normalize_tool_execution(WEB_FETCH_TOOL, ToolOutputSemantics::ReadOnly, execution);
        assert!(!normalized.execution.is_error);
        assert_eq!(normalized.execution.output["truncated"], true);
        assert!(normalized.execution.output.get("fullResultPath").is_some());
    }

    #[test]
    fn web_fetch_lone_cr_line_boundaries_match_cache_and_next_start_line() {
        let workspace = tempfile::tempdir().expect("workspace");
        // Mix lone CR with LF so shared CR-aware counting must stay consistent with cache.
        let mut chunks = Vec::new();
        for i in 0..2_200 {
            if i % 3 == 0 {
                chunks.push(format!("cr-{i:04}-{}", "c".repeat(28)));
            } else {
                chunks.push(format!("lf-{i:04}-{}", "l".repeat(28)));
            }
        }
        // Build with lone CR every third line ending.
        let mut full_text = String::new();
        for (i, chunk) in chunks.iter().enumerate() {
            full_text.push_str(chunk);
            if i + 1 < chunks.len() {
                if i % 3 == 0 {
                    full_text.push('\r');
                } else {
                    full_text.push('\n');
                }
            }
        }
        assert!(complete_line_count(&full_text) > 2_000);

        let output = finalize_web_fetch_output(
            workspace.path(),
            WebFetchFinalizeInput {
                requested_url: "https://example.test/cr".to_string(),
                final_url: "https://example.test/cr".to_string(),
                status: 200,
                content_type: Some("text/plain".to_string()),
                title: None,
                full_text: full_text.clone(),
                response_bytes: full_text.len(),
                requested_line_range: None,
                timeout_ms: 1_000,
            },
        )
        .expect("finalize cr");

        assert_eq!(output["truncated"], true);
        let next = output["nextStartLine"].as_u64().expect("next") as usize;
        let returned = output["returnedLines"].as_u64().expect("returned") as usize;
        assert_eq!(returned + 1, next);

        let full_path = output["fullResultPath"].as_str().expect("path");
        let cached = fs::read_to_string(workspace.path().join(full_path)).expect("cache");
        assert_eq!(cached, full_text);
        assert_eq!(
            complete_line_count(&cached),
            complete_line_count(&full_text)
        );
        // Cache line at nextStartLine must be the first omitted line (1-based).
        let mut line_no = 0_usize;
        let mut omitted = None;
        for_each_complete_line(&cached, |offset, line| {
            line_no = offset + 1;
            if line_no == next {
                omitted = Some(line.trim_end_matches(['\r', '\n']).to_string());
                return false;
            }
            true
        });
        assert!(
            omitted.is_some(),
            "nextStartLine={next} must exist in cache"
        );
    }

    #[test]
    fn web_fetch_ranged_overflow_uses_absolute_next_start_line_and_full_cache() {
        let workspace = tempfile::tempdir().expect("workspace");
        let full_text = large_body_lines(800, 200);
        let range = (50_usize, 700_usize);

        let output = finalize_web_fetch_output(
            workspace.path(),
            WebFetchFinalizeInput {
                requested_url: "https://example.test/range".to_string(),
                final_url: "https://example.test/range".to_string(),
                status: 200,
                content_type: Some("text/plain".to_string()),
                title: None,
                full_text: full_text.clone(),
                response_bytes: full_text.len(),
                requested_line_range: Some(range),
                timeout_ms: 15_000,
            },
        )
        .expect("ranged finalize");

        assert_eq!(output["truncated"], true);
        assert_eq!(output["startLine"], 50);
        assert_eq!(output["endLine"], 700);
        let next = output["nextStartLine"].as_u64().expect("next") as usize;
        assert!(next > 50, "absolute nextStartLine should be > range start");
        assert!(next <= 701, "nextStartLine={next}");

        let preview = output["text"].as_str().expect("text");
        // Numbered absolute lines in ranged mode.
        assert!(
            preview
                .lines()
                .next()
                .is_some_and(|line| line.starts_with("50\t")),
            "first preview line should be numbered from absolute start: {preview:?}"
        );

        let full_path = output["fullResultPath"].as_str().expect("path");
        let cached = fs::read_to_string(workspace.path().join(full_path)).expect("cache");
        assert_eq!(cached, full_text);
    }

    #[test]
    fn web_fetch_crlf_and_no_trailing_newline_are_preserved_in_cache() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut lines = Vec::new();
        for i in 0..2_500 {
            lines.push(format!("row-{i:04}-{}", "y".repeat(30)));
        }
        // True CRLF line endings and no trailing final newline.
        let full_text = lines.join("\r\n");
        assert!(!full_text.ends_with('\n'));
        assert!(full_text.contains("\r\n"));
        assert_eq!(complete_line_count(&full_text), 2_500);

        let output = finalize_web_fetch_output(
            workspace.path(),
            WebFetchFinalizeInput {
                requested_url: "https://example.test/crlf".to_string(),
                final_url: "https://example.test/crlf".to_string(),
                status: 200,
                content_type: Some("text/plain".to_string()),
                title: None,
                full_text: full_text.clone(),
                response_bytes: full_text.len(),
                requested_line_range: None,
                timeout_ms: 1_000,
            },
        )
        .expect("finalize");

        assert_eq!(output["truncated"], true);
        let full_path = output["fullResultPath"].as_str().expect("path");
        let cached = fs::read_to_string(workspace.path().join(full_path)).expect("cache");
        assert_eq!(cached, full_text);
        assert!(!cached.ends_with('\n'));
        assert!(cached.contains("\r\n"));
        let next = output["nextStartLine"].as_u64().expect("next") as usize;
        let returned = output["returnedLines"].as_u64().expect("returned") as usize;
        assert_eq!(returned + 1, next);
        assert_eq!(complete_line_count(&cached), 2_500);
    }

    #[test]
    fn web_fetch_utf8_multibyte_lines_are_not_split() {
        let workspace = tempfile::tempdir().expect("workspace");
        let line = "你好世界".repeat(20);
        let full_text = (0..2_500)
            .map(|i| format!("{i:05}-{line}"))
            .collect::<Vec<_>>()
            .join("\n");

        let output = finalize_web_fetch_output(
            workspace.path(),
            WebFetchFinalizeInput {
                requested_url: "https://example.test/utf8".to_string(),
                final_url: "https://example.test/utf8".to_string(),
                status: 200,
                content_type: Some("text/plain".to_string()),
                title: None,
                full_text: full_text.clone(),
                response_bytes: full_text.len(),
                requested_line_range: None,
                timeout_ms: 1_000,
            },
        )
        .expect("utf8 finalize");

        assert_eq!(output["truncated"], true);
        let preview = output["text"].as_str().expect("text");
        assert!(preview.is_char_boundary(preview.len()));
        for ch in preview.chars() {
            let _ = ch;
        }
        let cached = fs::read_to_string(
            workspace
                .path()
                .join(output["fullResultPath"].as_str().expect("path")),
        )
        .expect("cache");
        assert_eq!(cached, full_text);
    }

    #[test]
    fn web_fetch_single_line_over_hard_limit_is_error() {
        let workspace = tempfile::tempdir().expect("workspace");
        let huge_line = "Z".repeat(TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT + 64);
        let err = finalize_web_fetch_output(
            workspace.path(),
            WebFetchFinalizeInput {
                requested_url: "https://example.test/huge-line".to_string(),
                final_url: "https://example.test/huge-line".to_string(),
                status: 200,
                content_type: Some("text/plain".to_string()),
                title: None,
                full_text: huge_line,
                response_bytes: TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT + 64,
                requested_line_range: None,
                timeout_ms: 1_000,
            },
        )
        .expect_err("hard limit");
        assert!(err.contains("hard output ceiling"), "{err}");
    }

    #[test]
    fn web_fetch_just_under_soft_skips_cache_despite_truncated_field_overhead() {
        // Body large enough that reserving a truncated skeleton (path/note) would peel, but the
        // full untruncated ToolExecution still fits the soft budget — must not cache.
        let workspace = tempfile::tempdir().expect("workspace");
        let line = format!("row-{}", "x".repeat(90));
        // ~46 KiB raw text → ToolExecution usually ~47–49 KiB after JSON, still under 50 KiB.
        let line_count = (46 * 1024) / (line.len() + 1);
        let full_text = (0..line_count)
            .map(|i| format!("{i:04}-{line}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(full_text.len() > 40 * 1024);
        assert!(full_text.len() < TOOL_OUTPUT_SOFT_BYTE_LIMIT);

        let output = finalize_web_fetch_output(
            workspace.path(),
            WebFetchFinalizeInput {
                requested_url: "https://example.test/near-soft".to_string(),
                final_url: "https://example.test/near-soft".to_string(),
                status: 200,
                content_type: Some("text/plain".to_string()),
                title: Some("Near soft".to_string()),
                full_text: full_text.clone(),
                response_bytes: full_text.len(),
                requested_line_range: None,
                timeout_ms: 1_000,
            },
        )
        .expect("finalize near-soft");

        let measured = measure_tool_execution(&ToolExecution {
            output: output.clone(),
            is_error: false,
        })
        .expect("measure");
        assert!(
            measured.serialized_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT,
            "test fixture must stay under soft; got {}",
            measured.serialized_bytes
        );
        // Content is large enough that a multi-KiB truncated-skeleton reserve would matter.
        assert!(
            full_text.len() > TOOL_OUTPUT_SOFT_BYTE_LIMIT - WEB_RESULT_RESPONSE_OVERHEAD_BYTES,
            "fixture body should exceed soft minus truncated-path reserve ({})",
            full_text.len()
        );

        assert_eq!(output["truncated"], false);
        assert!(output.get("fullResultPath").is_none() || output["fullResultPath"].is_null());
        assert_eq!(output["text"].as_str().expect("text"), full_text);
        let cache_dir = workspace.path().join(".foco/web-results");
        assert!(
            !cache_dir.exists()
                || fs::read_dir(&cache_dir)
                    .map(|entries| entries.count() == 0)
                    .unwrap_or(true),
            "must not create cache when full response fits soft budget"
        );
    }

    #[test]
    fn web_search_large_results_truncate_on_record_boundary_and_cache_all() {
        let workspace = tempfile::tempdir().expect("workspace");
        let results: Vec<Value> = (0..10)
            .map(|i| {
                json!({
                    "title": format!("Title {i}"),
                    "url": format!("https://example.test/{i}"),
                    "snippet": "s".repeat(12_000),
                    "publishedAt": null,
                    "score": 0.5
                })
            })
            .collect();

        let output = finalize_web_search_output(
            workspace.path(),
            "tavily",
            "rust async",
            results.clone(),
            15_000,
        )
        .expect("search finalize");

        assert_eq!(output["truncated"], true);
        assert_eq!(output["totalResults"], 10);
        let returned = output["returnedResults"].as_u64().expect("returned") as usize;
        assert!(returned < 10 && returned >= 1, "returned={returned}");
        assert_eq!(
            output["results"].as_array().map(|a| a.len()),
            Some(returned)
        );
        // Must not split a result object.
        for item in output["results"].as_array().expect("results") {
            assert!(item.get("title").is_some());
            assert!(item.get("url").is_some());
            assert!(item.get("snippet").is_some());
        }

        let full_path = output["fullResultPath"].as_str().expect("path");
        assert!(full_path.starts_with(".foco/web-results/web-search-"));
        let cached = fs::read_to_string(workspace.path().join(full_path)).expect("cache");
        assert!(cached.contains("# Foco web_search result v1"));
        assert!(cached.contains("provider: tavily"));
        assert!(cached.contains("query: rust async"));
        for i in 0..10 {
            assert!(cached.contains(&format!("## result {}", i + 1)));
            assert!(cached.contains(&format!("https://example.test/{i}")));
        }
        assert!(!cached.contains("sk-"));
        assert!(!cached.contains("Bearer "));
        assert!(!cached.contains("api_key"));
        assert!(!cached.contains("X-Subscription-Token"));

        let next = output["nextStartLine"].as_u64().expect("next") as usize;
        assert!(next >= 2);
        let note = output["note"].as_str().expect("note");
        assert!(note.contains("read_file"));
        assert!(note.contains(full_path));
    }

    #[test]
    fn web_search_small_results_skip_cache() {
        let workspace = tempfile::tempdir().expect("workspace");
        let results = vec![json!({
            "title": "A",
            "url": "https://example.test/a",
            "snippet": "short",
            "publishedAt": null,
            "score": 1.0
        })];
        let output = finalize_web_search_output(workspace.path(), "brave", "q", results, 1_000)
            .expect("small search");
        assert_eq!(output["truncated"], false);
        assert_eq!(output["returnedResults"], 1);
        assert!(output.get("fullResultPath").is_none() || output["fullResultPath"].is_null());
        let cache_dir = workspace.path().join(".foco/web-results");
        assert!(
            !cache_dir.exists()
                || fs::read_dir(&cache_dir)
                    .map(|entries| entries.count() == 0)
                    .unwrap_or(true)
        );
    }

    #[test]
    fn web_search_full_under_soft_skips_cache_despite_record_prefix_estimate() {
        // Two ~24 KiB records: full JSON can still fit soft (~48 KiB + metadata), but a 4 KiB
        // overhead estimate may claim only one record fits. Must measure full first and not cache.
        let workspace = tempfile::tempdir().expect("workspace");
        let results = vec![
            json!({
                "title": "First",
                "url": "https://example.test/1",
                "snippet": "a".repeat(24_000),
                "publishedAt": null,
                "score": 0.9
            }),
            json!({
                "title": "Second",
                "url": "https://example.test/2",
                "snippet": "b".repeat(24_000),
                "publishedAt": null,
                "score": 0.8
            }),
        ];

        let estimate =
            soft_limit_array_prefix_len_with_overhead(&results, WEB_RESULT_RESPONSE_OVERHEAD_BYTES)
                .expect("estimate");
        assert!(
            estimate < results.len(),
            "fixture requires conservative estimate < total (estimate={estimate})"
        );

        let output = finalize_web_search_output(
            workspace.path(),
            "tavily",
            "near soft query",
            results.clone(),
            1_000,
        )
        .expect("search finalize");

        assert_eq!(output["truncated"], false);
        assert_eq!(output["returnedResults"], 2);
        assert_eq!(output["totalResults"], 2);
        assert!(output.get("fullResultPath").is_none() || output["fullResultPath"].is_null());
        let measured = measure_tool_execution(&ToolExecution {
            output: output.clone(),
            is_error: false,
        })
        .expect("measure");
        assert!(
            measured.serialized_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT,
            "full response should fit soft: {}",
            measured.serialized_bytes
        );
        let cache_dir = workspace.path().join(".foco/web-results");
        assert!(
            !cache_dir.exists()
                || fs::read_dir(&cache_dir)
                    .map(|entries| entries.count() == 0)
                    .unwrap_or(true),
            "must not cache when full search response fits soft"
        );
    }

    #[test]
    fn web_search_huge_query_with_empty_results_fits_soft_and_caches_full_query() {
        // Oversized multi-line query must not escape soft budget via truncated=true. Preview may
        // shorten the echoed query; cache keeps the full original query for read_file recovery.
        let workspace = tempfile::tempdir().expect("workspace");
        let query = (0..2_100)
            .map(|i| format!("line-{i}-search-token"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            complete_line_count(&query) > TOOL_OUTPUT_SOFT_LINE_LIMIT,
            "fixture query must exceed soft line limit"
        );

        let output =
            finalize_web_search_output(workspace.path(), "tavily", &query, Vec::new(), 1_000)
                .expect("search finalize with huge query");

        assert_eq!(output["truncated"], true);
        assert_eq!(output["totalResults"], 0);
        assert_eq!(output["returnedResults"], 0);
        let preview_query = output["query"].as_str().expect("query");
        assert!(
            complete_line_count(preview_query) <= TOOL_OUTPUT_SOFT_LINE_LIMIT,
            "preview query lines must not exceed soft line limit"
        );
        assert!(
            preview_query.len() < query.len() || preview_query != query,
            "preview query should be shortened when full query overflows soft budget"
        );

        let full_path = output["fullResultPath"].as_str().expect("path");
        assert!(full_path.starts_with(".foco/web-results/web-search-"));
        let cached = fs::read_to_string(workspace.path().join(full_path)).expect("cache");
        assert!(cached.contains("query: line-0-search-token"));
        assert!(cached.contains("line-2099-search-token"));
        assert_eq!(output["nextStartLine"], 1);

        let measured = measure_tool_execution(&ToolExecution {
            output: output.clone(),
            is_error: false,
        })
        .expect("measure");
        assert!(
            measured.serialized_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT,
            "truncated success must fit soft bytes: {}",
            measured.serialized_bytes
        );
        assert!(
            measured.text_lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT,
            "truncated success must fit soft lines: {}",
            measured.text_lines
        );

        let normalized = normalize_tool_execution(
            WEB_SEARCH_TOOL,
            ToolOutputSemantics::ReadOnly,
            ToolExecution {
                output,
                is_error: false,
            },
        );
        assert!(!normalized.execution.is_error);
        assert_eq!(normalized.execution.output["truncated"], true);
    }

    #[test]
    fn web_search_single_soft_over_record_does_not_reintroduce_huge_query() {
        // Single result soft-over is allowed under hard, but oversized multi-line query must not
        // hitch a ride via truncated=true. Echoed query stays empty; cache keeps the full query.
        let workspace = tempfile::tempdir().expect("workspace");
        let query = (0..2_100)
            .map(|i| format!("line-{i}-search-token"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            complete_line_count(&query) > TOOL_OUTPUT_SOFT_LINE_LIMIT,
            "fixture query must exceed soft line limit"
        );

        // One record that alone exceeds soft but stays under hard.
        let snippet = "S".repeat(TOOL_OUTPUT_SOFT_BYTE_LIMIT + 2 * 1024);
        let results = vec![json!({
            "title": "one",
            "url": "https://example.com/one",
            "snippet": snippet,
            "publishedAt": null,
            "score": 1.0
        })];

        let output = finalize_web_search_output(workspace.path(), "brave", &query, results, 1_000)
            .expect("single-record soft-over with huge query");

        assert_eq!(output["truncated"], true);
        assert_eq!(output["totalResults"], 1);
        assert_eq!(output["returnedResults"], 1);
        assert_eq!(output["nextStartLine"], 1);
        let preview_query = output["query"].as_str().expect("query");
        assert!(
            preview_query.is_empty(),
            "single-record soft-over must not reintroduce full query; got len={}",
            preview_query.len()
        );

        let full_path = output["fullResultPath"].as_str().expect("path");
        assert!(full_path.starts_with(".foco/web-results/web-search-"));
        let cached = fs::read_to_string(workspace.path().join(full_path)).expect("cache");
        assert!(cached.contains("query: line-0-search-token"));
        assert!(cached.contains("line-2099-search-token"));
        assert!(cached.contains("https://example.com/one"));

        let measured = measure_tool_execution(&ToolExecution {
            output: output.clone(),
            is_error: false,
        })
        .expect("measure");
        assert!(
            measured.serialized_bytes > TOOL_OUTPUT_SOFT_BYTE_LIMIT,
            "single record soft-over fixture must exceed soft bytes"
        );
        assert!(
            measured.serialized_bytes <= TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT,
            "single record soft-over must stay under hard: {}",
            measured.serialized_bytes
        );
        // Oversized query must not be part of the soft-over envelope: empty query + record
        // should be far smaller than full query + record.
        let full_q_measure = measure_tool_execution(&ToolExecution {
            output: assemble_web_search_response(
                "brave",
                &query,
                &[json!({
                    "title": "one",
                    "url": "https://example.com/one",
                    "snippet": "S".repeat(TOOL_OUTPUT_SOFT_BYTE_LIMIT + 2 * 1024),
                    "publishedAt": null,
                    "score": 1.0
                })],
                1,
                1,
                1_000,
                Some((full_path, 1)),
            ),
            is_error: false,
        })
        .expect("measure full query candidate");
        assert!(
            measured.serialized_bytes + 8 * 1024 < full_q_measure.serialized_bytes,
            "preview without huge query must be substantially smaller than full-query candidate"
        );

        let normalized = normalize_tool_execution(
            WEB_SEARCH_TOOL,
            ToolOutputSemantics::ReadOnly,
            ToolExecution {
                output,
                is_error: false,
            },
        );
        assert!(!normalized.execution.is_error);
        assert_eq!(normalized.execution.output["truncated"], true);
        assert_eq!(
            normalized.execution.output["query"].as_str().unwrap_or("x"),
            ""
        );
    }

    #[test]
    fn prune_web_results_enforces_count_cap() {
        let workspace = tempfile::tempdir().expect("workspace");
        let dir = workspace.path().join(".foco/web-results");
        fs::create_dir_all(&dir).expect("dir");
        for i in 0..MAX_WEB_RESULT_FILES + 3 {
            // Unique ids via counter.
            let path =
                write_web_result_cache(workspace.path(), "fetch", &format!("body-{i}"), None, None)
                    .expect("write");
            assert!(workspace.path().join(&path).exists());
        }
        let count = fs::read_dir(&dir)
            .expect("read")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("web-") && n.ends_with(".txt"))
            })
            .count();
        assert!(
            count <= MAX_WEB_RESULT_FILES,
            "count={count} cap={MAX_WEB_RESULT_FILES}"
        );
    }

    #[test]
    fn prune_web_results_removes_ttl_expired_files() {
        let workspace = tempfile::tempdir().expect("workspace");
        let dir = workspace.path().join(".foco/web-results");
        fs::create_dir_all(&dir).expect("dir");

        let expired_path = dir.join("web-fetch-expired-old.txt");
        fs::write(&expired_path, b"stale").expect("write expired");
        let stale_time = SystemTime::now()
            .checked_sub(WEB_RESULT_TTL + Duration::from_secs(60))
            .expect("stale time");
        let file = fs::File::options()
            .write(true)
            .open(&expired_path)
            .expect("open expired");
        file.set_modified(stale_time).expect("set mtime");
        drop(file);

        let fresh_rel = write_web_result_cache(workspace.path(), "fetch", "fresh-body", None, None)
            .expect("write fresh");
        let fresh_abs = workspace.path().join(&fresh_rel);
        assert!(fresh_abs.exists());

        // write_web_result_cache prunes before writing; expired should be gone.
        assert!(
            !expired_path.exists(),
            "TTL-expired cache file should be pruned"
        );
        assert!(fresh_abs.exists());
    }

    #[test]
    fn write_web_result_cache_fails_cleanly_on_unwritable_dir() {
        // Use a file path where a directory is required.
        let workspace = tempfile::tempdir().expect("workspace");
        let foco = workspace.path().join(".foco");
        fs::create_dir_all(&foco).expect("foco");
        // Place a file named web-results so create_dir_all fails when writing into it.
        fs::write(foco.join("web-results"), b"not-a-dir").expect("block dir");
        let err = write_web_result_cache(workspace.path(), "fetch", "body", None, None)
            .expect_err("should fail");
        assert!(
            err.contains("failed to create") || err.contains("failed to write"),
            "{err}"
        );
    }

    #[test]
    fn prune_web_results_keeps_fresh_temp_files() {
        let workspace = tempfile::tempdir().expect("workspace");
        let dir = workspace.path().join(".foco/web-results");
        fs::create_dir_all(&dir).expect("dir");

        let fresh_tmp = dir.join(".web-fetch-inprogress-1.tmp");
        fs::write(&fresh_tmp, b"partial").expect("write temp");

        prune_web_results_dir(&dir);

        assert!(
            fresh_tmp.exists(),
            "fresh temp must survive prune so concurrent publish can finish"
        );
    }

    #[test]
    fn prune_web_results_removes_stale_temp_orphans() {
        let workspace = tempfile::tempdir().expect("workspace");
        let dir = workspace.path().join(".foco/web-results");
        fs::create_dir_all(&dir).expect("dir");

        let stale_tmp = dir.join(".web-fetch-orphan-old.tmp");
        fs::write(&stale_tmp, b"orphan").expect("write orphan");
        let stale_time = SystemTime::now()
            .checked_sub(WEB_RESULT_TEMP_ORPHAN_TTL + Duration::from_secs(60))
            .expect("stale time");
        let file = fs::File::options()
            .write(true)
            .open(&stale_tmp)
            .expect("open orphan");
        file.set_modified(stale_time).expect("set mtime");
        drop(file);

        prune_web_results_dir(&dir);

        assert!(
            !stale_tmp.exists(),
            "temp older than orphan TTL should be pruned"
        );
    }

    #[test]
    fn publish_web_result_cache_file_does_not_overwrite_existing() {
        let workspace = tempfile::tempdir().expect("workspace");
        let dir = workspace.path().join(".foco/web-results");
        fs::create_dir_all(&dir).expect("dir");

        let destination = dir.join("web-fetch-collision-dest.txt");
        fs::write(&destination, b"original-published").expect("seed dest");

        let temporary = dir.join(".web-fetch-collision-dest.tmp");
        fs::write(&temporary, b"attacker-body").expect("write temp");

        let err = publish_web_result_cache_file(&temporary, &destination)
            .expect_err("must refuse replace");
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read_to_string(&destination).expect("read dest"),
            "original-published",
            "published cache must not be overwritten"
        );
        // Temp remains for caller cleanup (write_web_result_cache removes it on AlreadyExists).
        assert!(temporary.exists());
    }

    #[test]
    fn publish_web_result_cache_file_succeeds_and_removes_temp() {
        let workspace = tempfile::tempdir().expect("workspace");
        let dir = workspace.path().join(".foco/web-results");
        fs::create_dir_all(&dir).expect("dir");

        let destination = dir.join("web-fetch-publish-ok.txt");
        let temporary = dir.join(".web-fetch-publish-ok.tmp");
        fs::write(&temporary, b"complete-body").expect("write temp");

        publish_web_result_cache_file(&temporary, &destination).expect("publish");
        assert_eq!(
            fs::read_to_string(&destination).expect("read dest"),
            "complete-body"
        );
        assert!(
            !temporary.exists(),
            "temp name should be removed after successful hard_link publish"
        );
    }

    #[test]
    fn write_web_result_cache_survives_concurrent_prune_of_fresh_temps() {
        // Simulate: writer A holds a fresh temp; writer B prunes (via its own write).
        // A's temp must remain so A can still publish.
        let workspace = tempfile::tempdir().expect("workspace");
        let dir = workspace.path().join(".foco/web-results");
        fs::create_dir_all(&dir).expect("dir");

        let a_tmp = dir.join(".web-fetch-writer-a.tmp");
        fs::write(&a_tmp, b"writer-a-body").expect("A temp");
        let a_dest = dir.join("web-fetch-writer-a.txt");

        // B publishes another cache (triggers prune_web_results_dir).
        let b_rel = write_web_result_cache(workspace.path(), "fetch", "writer-b-body", None, None)
            .expect("B write");
        assert!(workspace.path().join(&b_rel).exists());
        assert!(
            a_tmp.exists(),
            "concurrent fresh temp must not be deleted by prune during B's write"
        );

        publish_web_result_cache_file(&a_tmp, &a_dest).expect("A publish after B prune");
        assert_eq!(
            fs::read_to_string(&a_dest).expect("A dest"),
            "writer-a-body"
        );
    }

    #[tokio::test]
    async fn execute_web_fetch_integration_soft_truncates_via_local_http() {
        let body = large_body_lines(2_500, 50);
        let body_for_server = body.clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let app = Router::new().route(
            "/big",
            get(move || {
                let body = body_for_server.clone();
                async move {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                        .body(Body::from(body))
                        .expect("response")
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("serve");
        });

        let workspace = tempfile::tempdir().expect("workspace");
        let url = format!("http://{addr}/big");
        let output = execute_web_tool(
            &WebSearchSettings::default(),
            WEB_FETCH_TOOL,
            json!({ "url": url, "timeoutMs": 5_000 }),
            Duration::from_secs(5),
            workspace.path(),
        )
        .await
        .expect("web_fetch");

        assert_eq!(output["truncated"], true);
        let full_path = output["fullResultPath"].as_str().expect("path");
        let cached = fs::read_to_string(workspace.path().join(full_path)).expect("cache");
        // normalize_web_text trims empty lines; our body has none.
        assert_eq!(cached, body.trim());
        assert!(!cached.contains("Authorization"));

        let _ = shutdown_tx.send(());
        let _ = server.await;
    }

    #[tokio::test]
    async fn execute_web_fetch_rejects_over_2mib_raw_response() {
        let huge = vec![b'a'; MAX_WEB_FETCH_BYTES + 8];
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let app = Router::new().route(
            "/too-big",
            get(move || {
                let huge = huge.clone();
                async move {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "text/plain")
                        .header(header::CONTENT_LENGTH, huge.len().to_string())
                        .body(Body::from(huge))
                        .expect("response")
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("serve");
        });

        let workspace = tempfile::tempdir().expect("workspace");
        let url = format!("http://{addr}/too-big");
        let err = execute_web_tool(
            &WebSearchSettings::default(),
            WEB_FETCH_TOOL,
            json!({ "url": url, "timeoutMs": 5_000 }),
            Duration::from_secs(5),
            workspace.path(),
        )
        .await
        .expect_err("2MiB ceiling");
        assert!(
            err.contains("too large") || err.contains(&MAX_WEB_FETCH_BYTES.to_string()),
            "{err}"
        );

        let _ = shutdown_tx.send(());
        let _ = server.await;
    }

    #[tokio::test]
    async fn execute_web_fetch_rejects_over_2mib_error_body_without_content_length() {
        // Chunked 4xx/5xx bodies must share the raw response ceiling (no unbounded text()).
        let huge = vec![b'e'; MAX_WEB_FETCH_BYTES + 64];
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let app = Router::new().route(
            "/err-huge",
            get(move || {
                let huge = huge.clone();
                async move {
                    // Stream without Content-Length so clients must bound by actual bytes.
                    let chunks = huge
                        .chunks(64 * 1024)
                        .map(|chunk| Ok::<_, std::io::Error>(chunk.to_vec()))
                        .collect::<Vec<_>>();
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header(header::CONTENT_TYPE, "text/plain")
                        .body(Body::from_stream(futures_util::stream::iter(chunks)))
                        .expect("response")
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("serve");
        });

        let workspace = tempfile::tempdir().expect("workspace");
        let url = format!("http://{addr}/err-huge");
        let err = execute_web_tool(
            &WebSearchSettings::default(),
            WEB_FETCH_TOOL,
            json!({ "url": url, "timeoutMs": 10_000 }),
            Duration::from_secs(10),
            workspace.path(),
        )
        .await
        .expect_err("2MiB ceiling on error body");
        assert!(
            err.contains("too large") || err.contains(&MAX_WEB_FETCH_BYTES.to_string()),
            "{err}"
        );
        // Must not surface a multi-MiB error preview as HTTP status text.
        assert!(
            !err.contains(&"e".repeat(1_000)),
            "error must not embed unbounded error body"
        );

        let _ = shutdown_tx.send(());
        let _ = server.await;
    }

    #[test]
    fn write_web_result_cache_respects_count_cap_under_concurrent_writers() {
        let workspace = tempfile::tempdir().expect("workspace");
        let dir = workspace.path().join(".foco/web-results");
        fs::create_dir_all(&dir).expect("dir");

        // Seed near the cap so concurrent publishes would race without the write lock.
        for i in 0..15 {
            write_web_result_cache(workspace.path(), "fetch", &format!("seed-{i}"), None, None)
                .expect("seed");
        }

        let workspace_path = workspace.path().to_path_buf();
        let mut handles = Vec::new();
        for i in 0..12 {
            let path = workspace_path.clone();
            handles.push(std::thread::spawn(move || {
                write_web_result_cache(&path, "fetch", &format!("concurrent-{i}"), None, None)
                    .expect("concurrent write");
            }));
        }
        for handle in handles {
            handle.join().expect("join writer");
        }

        let count = fs::read_dir(&dir)
            .expect("read")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("web-") && n.ends_with(".txt"))
            })
            .count();
        assert!(
            count <= MAX_WEB_RESULT_FILES,
            "concurrent writers exceeded count cap: count={count} cap={MAX_WEB_RESULT_FILES}"
        );
    }

    #[test]
    fn package_and_materialize_web_result_roundtrip_for_sidecar() {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
        use sha2::{Digest, Sha256};

        let host_workspace = tempfile::tempdir().expect("host");
        let sidecar_workspace = tempfile::tempdir().expect("sidecar");
        let body = "line-1\nline-2\nline-3\n";
        let relative = write_web_result_cache(host_workspace.path(), "fetch", body, None, None)
            .expect("cache");
        let host_result = json!({
            "truncated": true,
            "nextStartLine": 2,
            "fullResultPath": relative,
            "note": web_result_truncated_note(&relative, 2),
            "text": "line-1",
        });

        let files = package_brokered_web_result_files(host_workspace.path(), &host_result)
            .expect("package")
            .expect("files present");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].mime_type, BROKERED_WEB_RESULT_MIME);
        assert!(!files[0].data_base64.is_empty());

        let materialized =
            materialize_brokered_web_result(sidecar_workspace.path(), host_result.clone(), files)
                .expect("materialize");
        assert_eq!(
            materialized["fullResultPath"].as_str().expect("path"),
            relative
        );
        assert!(
            materialized["fullResultPath"]
                .as_str()
                .expect("path")
                .starts_with(".foco/web-results/"),
            "must stay workspace-relative"
        );
        assert!(
            !materialized["fullResultPath"]
                .as_str()
                .expect("path")
                .starts_with('/'),
            "must not leak host absolute path"
        );
        let sidecar_bytes =
            fs::read(sidecar_workspace.path().join(&relative)).expect("sidecar file");
        assert_eq!(sidecar_bytes, body.as_bytes());

        // Checksum mismatch must not leave a file.
        let bad = vec![BrokeredTransferFile {
            file_name: Path::new(&relative)
                .file_name()
                .and_then(|v| v.to_str())
                .expect("name")
                .to_string(),
            mime_type: BROKERED_WEB_RESULT_MIME.to_string(),
            bytes: body.len(),
            sha256: "00".repeat(32),
            data_base64: BASE64_STANDARD.encode(body.as_bytes()),
        }];
        let err =
            materialize_brokered_web_result(sidecar_workspace.path(), host_result.clone(), bad)
                .expect_err("checksum");
        assert!(err.contains("checksum"), "{err}");

        // Illegal filename / wrong directory.
        let evil_result = json!({
            "truncated": true,
            "nextStartLine": 1,
            "fullResultPath": "/tmp/host-secret.txt",
        });
        let err = package_brokered_web_result_files(host_workspace.path(), &evil_result)
            .expect_err("absolute path");
        assert!(
            err.contains("workspace-relative") || err.contains("outside"),
            "{err}"
        );

        let escape_result = json!({
            "truncated": true,
            "nextStartLine": 1,
            "fullResultPath": ".foco/image-gen/not-web.png",
        });
        let err = package_brokered_web_result_files(host_workspace.path(), &escape_result)
            .expect_err("wrong dir");
        assert!(err.contains("outside") || err.contains("invalid"), "{err}");

        // Oversized declared size.
        let oversized = vec![BrokeredTransferFile {
            file_name: Path::new(&relative)
                .file_name()
                .and_then(|v| v.to_str())
                .expect("name")
                .to_string(),
            mime_type: BROKERED_WEB_RESULT_MIME.to_string(),
            bytes: MAX_BROKERED_WEB_RESULT_FILE_BYTES + 1,
            sha256: format!("{:x}", Sha256::digest(body.as_bytes())),
            data_base64: BASE64_STANDARD.encode(body.as_bytes()),
        }];
        let err = materialize_brokered_web_result(sidecar_workspace.path(), host_result, oversized)
            .expect_err("oversize");
        assert!(
            err.contains("invalid size") || err.contains("size"),
            "{err}"
        );
    }

    #[test]
    fn package_web_result_returns_none_without_full_result_path() {
        let workspace = tempfile::tempdir().expect("workspace");
        let result = json!({ "truncated": false, "text": "small" });
        let files = package_brokered_web_result_files(workspace.path(), &result).expect("ok");
        assert!(files.is_none());
    }

    #[test]
    fn materialize_rejects_image_mime_and_host_path_in_full_result_path() {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
        use sha2::{Digest, Sha256};

        let workspace = tempfile::tempdir().expect("workspace");
        let body = b"not an image";
        let relative = ".foco/web-results/web-fetch-1-2-abcdef01.txt";
        let result = json!({
            "fullResultPath": relative,
            "truncated": true,
            "nextStartLine": 1,
        });
        let files = vec![BrokeredTransferFile {
            file_name: "web-fetch-1-2-abcdef01.txt".to_string(),
            mime_type: "image/png".to_string(),
            bytes: body.len(),
            sha256: format!("{:x}", Sha256::digest(body)),
            data_base64: BASE64_STANDARD.encode(body),
        }];
        let err =
            materialize_brokered_web_result(workspace.path(), result, files).expect_err("mime");
        assert!(err.contains("MIME"), "{err}");
        assert!(!workspace.path().join(relative).exists());

        let host_path_result = json!({
            "fullResultPath": "/Users/host/.foco/broker-web-transfer/x/web-fetch-1.txt",
            "truncated": true,
            "nextStartLine": 1,
        });
        let err = materialize_brokered_web_result(workspace.path(), host_path_result, vec![])
            .expect_err("host path");
        assert!(
            err.contains("workspace-relative")
                || err.contains("exactly")
                || err.contains("invalid"),
            "{err}"
        );
    }

    fn brokered_web_file(file_name: &str, body: &[u8]) -> BrokeredTransferFile {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
        use sha2::{Digest, Sha256};
        BrokeredTransferFile {
            file_name: file_name.to_string(),
            mime_type: BROKERED_WEB_RESULT_MIME.to_string(),
            bytes: body.len(),
            sha256: format!("{:x}", Sha256::digest(body)),
            data_base64: BASE64_STANDARD.encode(body),
        }
    }

    #[test]
    fn materialize_brokered_web_result_prunes_count_and_ttl() {
        let workspace = tempfile::tempdir().expect("workspace");
        let dir = workspace.path().join(".foco/web-results");
        fs::create_dir_all(&dir).expect("dir");

        // Seed expired file; materialize must prune it.
        let expired_path = dir.join("web-fetch-expired-sidecar-old.txt");
        fs::write(&expired_path, b"stale").expect("write expired");
        let stale_time = SystemTime::now()
            .checked_sub(WEB_RESULT_TTL + Duration::from_secs(60))
            .expect("stale time");
        let file = fs::File::options()
            .write(true)
            .open(&expired_path)
            .expect("open expired");
        file.set_modified(stale_time).expect("set mtime");
        drop(file);

        // Fill to the count cap with unique names, then materialize one more.
        for i in 0..MAX_WEB_RESULT_FILES {
            let name = format!("web-fetch-seed-{i:02}-abcdef0123456789.txt");
            fs::write(dir.join(&name), format!("seed-{i}")).expect("seed");
        }

        let new_name = "web-fetch-sidecar-new-abcdef0123456789.txt";
        let relative = format!(".foco/web-results/{new_name}");
        let body = b"sidecar-materialized-body\n";
        let result = json!({
            "fullResultPath": relative,
            "truncated": true,
            "nextStartLine": 1,
            "note": "host note",
        });
        materialize_brokered_web_result(
            workspace.path(),
            result,
            vec![brokered_web_file(new_name, body)],
        )
        .expect("materialize");

        assert!(!expired_path.exists(), "TTL-expired file must be pruned");
        assert_eq!(
            fs::read(workspace.path().join(&relative)).expect("read new"),
            body
        );

        let count = fs::read_dir(&dir)
            .expect("read")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("web-") && n.ends_with(".txt"))
            })
            .count();
        assert!(
            count <= MAX_WEB_RESULT_FILES,
            "sidecar materialize count={count} cap={MAX_WEB_RESULT_FILES}"
        );
        // Just-published file must survive post-publish cap prune.
        assert!(workspace.path().join(&relative).exists());
    }

    #[test]
    fn materialize_brokered_web_result_does_not_delete_existing_on_collision() {
        let workspace = tempfile::tempdir().expect("workspace");
        let file_name = "web-fetch-keep-abcdef0123456789.txt";
        let relative = format!(".foco/web-results/{file_name}");
        let dest = workspace.path().join(&relative);
        fs::create_dir_all(dest.parent().expect("parent")).expect("mkdir");
        fs::write(&dest, b"original-cache").expect("seed");

        // Different content, same path: must fail and leave original intact.
        let result = json!({
            "fullResultPath": relative,
            "truncated": true,
            "nextStartLine": 1,
        });
        let err = materialize_brokered_web_result(
            workspace.path(),
            result,
            vec![brokered_web_file(file_name, b"replacement-body")],
        )
        .expect_err("collision");
        assert!(
            err.contains("already exists"),
            "expected already-exists error, got: {err}"
        );
        assert_eq!(fs::read(&dest).expect("read"), b"original-cache");

        // Same content replay: idempotent success, still no overwrite churn.
        let result = json!({
            "fullResultPath": relative,
            "truncated": true,
            "nextStartLine": 1,
        });
        let ok = materialize_brokered_web_result(
            workspace.path(),
            result,
            vec![brokered_web_file(file_name, b"original-cache")],
        )
        .expect("idempotent");
        assert_eq!(ok["fullResultPath"].as_str().expect("path"), relative);
        assert_eq!(fs::read(&dest).expect("read"), b"original-cache");
    }

    #[test]
    fn materialize_brokered_web_result_protects_collision_target_at_count_cap() {
        // When the cache dir is full and the collision target is the oldest entry,
        // pre-write reserve-prune must not delete it to make room — no-clobber wins.
        let workspace = tempfile::tempdir().expect("workspace");
        let dir = workspace.path().join(".foco/web-results");
        fs::create_dir_all(&dir).expect("dir");

        let keep_name = "web-fetch-oldest-keep-abcdef0123456789.txt";
        let keep_path = dir.join(keep_name);
        fs::write(&keep_path, b"protected-original").expect("seed keep");
        let oldest = SystemTime::now()
            .checked_sub(Duration::from_secs(3600))
            .expect("oldest time");
        let keep_file = fs::File::options()
            .write(true)
            .open(&keep_path)
            .expect("open keep");
        keep_file.set_modified(oldest).expect("set oldest mtime");
        drop(keep_file);

        // Fill remaining slots with newer seeds so the protected file is strictly oldest.
        for i in 1..MAX_WEB_RESULT_FILES {
            let name = format!("web-fetch-seed-{i:02}-abcdef0123456789.txt");
            let path = dir.join(&name);
            fs::write(&path, format!("seed-{i}")).expect("seed");
            let newer = SystemTime::now()
                .checked_sub(Duration::from_secs(60))
                .expect("newer time");
            let file = fs::File::options()
                .write(true)
                .open(&path)
                .expect("open seed");
            file.set_modified(newer).expect("set seed mtime");
            drop(file);
        }

        let relative = format!(".foco/web-results/{keep_name}");
        let result = json!({
            "fullResultPath": relative,
            "truncated": true,
            "nextStartLine": 1,
        });
        let err = materialize_brokered_web_result(
            workspace.path(),
            result,
            vec![brokered_web_file(
                keep_name,
                b"different-content-must-not-land",
            )],
        )
        .expect_err("full-cap collision");
        assert!(
            err.contains("already exists"),
            "expected already-exists error, got: {err}"
        );
        assert_eq!(
            fs::read(&keep_path).expect("read keep"),
            b"protected-original",
            "oldest collision target must survive full-cap reserve prune"
        );
    }
}
