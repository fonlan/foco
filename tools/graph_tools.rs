use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use foco_store::workspace::{
    CodeGraphImportRecord, CodeGraphReferenceRecord, CodeGraphRelatedFileRecord,
    CodeGraphSymbolRecord, CodeGraphSymbolRelationRecord, WorkspaceDatabase,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::output_budget::{
    TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT, TOOL_OUTPUT_SOFT_BYTE_LIMIT,
    TOOL_OUTPUT_SOFT_LINE_LIMIT, measure_tool_execution, soft_limit_array_prefix_len_with_overhead,
};
use crate::{
    DEFAULT_GRAPH_EXPLORE_CONTEXT_LINES, DEFAULT_GRAPH_EXPLORE_RESULT_LIMIT,
    DEFAULT_GRAPH_RESULT_LIMIT, DEFAULT_GRAPH_TOOL_TIMEOUT_MS, LineRange,
    MAX_GRAPH_EXPLORE_CONTEXT_LINES, MAX_GRAPH_EXPLORE_OUTPUT_BYTES,
    MAX_GRAPH_EXPLORE_RESULT_LIMIT, MAX_GRAPH_EXPLORE_SYMBOL_LINES, MAX_GRAPH_RESULT_LIMIT,
    MAX_RANGED_READ_SOURCE_BYTES, ToolExecution, count_text_lines, decode_text_file,
    errors::{ToolRuntimeError, tool_timeout_ms},
    normalize_read_line_range, normalize_workspace_path_text, numbered_content, parse_arguments,
    read_line_range, resolve_workspace_file,
};

const GRAPH_LIST_RESPONSE_OVERHEAD_BYTES: usize = 2 * 1024;
const GRAPH_EXPLORE_PAGE_SIZE: i64 = 100;
const MAX_GRAPH_EXPLORE_COLLECTION_SYMBOLS: usize = 1_000;
const GRAPH_RESULTS_DIR: &str = "graph-results";
const MAX_GRAPH_RESULT_FILES: usize = 20;
const GRAPH_RESULT_TTL: Duration = Duration::from_secs(60 * 60);
/// Leaves room for read_file's line number, path, and enclosing response metadata.
const GRAPH_SNAPSHOT_READ_FILE_LINE_RESERVE_BYTES: usize = 1024;
const MAX_GRAPH_SNAPSHOT_LINE_BYTES: usize =
    TOOL_EXECUTION_PAYLOAD_HARD_BYTE_LIMIT - GRAPH_SNAPSHOT_READ_FILE_LINE_RESERVE_BYTES;

pub(crate) fn graph_find_symbols(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GraphFindSymbolsInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let query = non_empty_argument("query", &request.query)?;
    let path = request
        .path
        .as_deref()
        .map(normalize_workspace_path_text)
        .transpose()?;
    let limit = graph_limit(request.limit)?;
    let database = open_code_graph_database(workspace_path)?;
    let mut symbols = database.find_code_graph_symbols(
        query,
        request.kind.as_deref(),
        path.as_deref(),
        graph_query_limit(limit)?,
    )?;
    let limit_truncated = truncate_records(&mut symbols, limit);
    let symbol_values = symbols.into_iter().map(symbol_json).collect::<Vec<_>>();
    let (symbols, soft_truncated) =
        soft_limit_preview_records(symbol_values, GRAPH_LIST_RESPONSE_OVERHEAD_BYTES)?;
    let returned_count = symbols.len();

    Ok(json!({
        "query": query,
        "kind": request.kind,
        "path": path,
        "symbols": symbols,
        "truncated": limit_truncated || soft_truncated,
        "returnedCount": returned_count,
        "timeoutMs": timeout_ms
    }))
}

pub(crate) fn graph_find_callers(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GraphSymbolLookupInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let database = open_code_graph_database(workspace_path)?;
    let symbol = resolve_graph_symbol(&database, &request)?;
    let limit = graph_limit(request.limit)?;
    let mut callers = database.code_graph_callers(symbol.id, graph_query_limit(limit)?)?;
    let limit_truncated = truncate_records(&mut callers, limit);
    let records = callers.into_iter().map(relation_json).collect::<Vec<_>>();
    let (callers, soft_truncated) =
        soft_limit_preview_records(records, GRAPH_LIST_RESPONSE_OVERHEAD_BYTES)?;
    let returned_count = callers.len();

    Ok(json!({
        "symbol": symbol_json(symbol),
        "callers": callers,
        "relationshipSemantics": "static_call_site_approximation",
        "truncated": limit_truncated || soft_truncated,
        "returnedCount": returned_count,
        "timeoutMs": timeout_ms
    }))
}

pub(crate) fn graph_find_callees(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GraphSymbolLookupInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let database = open_code_graph_database(workspace_path)?;
    let symbol = resolve_graph_symbol(&database, &request)?;
    let limit = graph_limit(request.limit)?;
    let mut callees = database.code_graph_callees(symbol.id, graph_query_limit(limit)?)?;
    let limit_truncated = truncate_records(&mut callees, limit);
    let records = callees.into_iter().map(relation_json).collect::<Vec<_>>();
    let (callees, soft_truncated) =
        soft_limit_preview_records(records, GRAPH_LIST_RESPONSE_OVERHEAD_BYTES)?;
    let returned_count = callees.len();

    Ok(json!({
        "symbol": symbol_json(symbol),
        "callees": callees,
        "relationshipSemantics": "static_call_site_approximation",
        "truncated": limit_truncated || soft_truncated,
        "returnedCount": returned_count,
        "timeoutMs": timeout_ms
    }))
}

pub(crate) fn graph_find_children(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GraphFindChildrenInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let database = open_code_graph_database(workspace_path)?;
    let symbol = resolve_graph_symbol(
        &database,
        &GraphSymbolLookupInput {
            symbol_id: request.symbol_id,
            symbol: request.symbol.clone(),
            path: request.path.clone(),
            limit: request.limit,
            timeout_ms: request.timeout_ms,
        },
    )?;
    let limit = graph_limit(request.limit)?;
    let mut children = database.code_graph_children(
        symbol.id,
        request.kind.as_deref(),
        graph_query_limit(limit)?,
    )?;
    let limit_truncated = truncate_records(&mut children, limit);
    let records = children.into_iter().map(symbol_json).collect::<Vec<_>>();
    let (children, soft_truncated) =
        soft_limit_preview_records(records, GRAPH_LIST_RESPONSE_OVERHEAD_BYTES)?;
    let returned_count = children.len();

    Ok(json!({
        "symbol": symbol_json(symbol),
        "kind": request.kind,
        "children": children,
        "truncated": limit_truncated || soft_truncated,
        "returnedCount": returned_count,
        "timeoutMs": timeout_ms
    }))
}

pub(crate) fn graph_find_references(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GraphSymbolLookupInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let database = open_code_graph_database(workspace_path)?;
    let symbol = resolve_graph_symbol(&database, &request)?;
    let limit = graph_limit(request.limit)?;
    let mut references = database.code_graph_references(symbol.id, graph_query_limit(limit)?)?;
    let limit_truncated = truncate_records(&mut references, limit);
    let records = references
        .into_iter()
        .map(reference_json)
        .collect::<Vec<_>>();
    let (references, soft_truncated) =
        soft_limit_preview_records(records, GRAPH_LIST_RESPONSE_OVERHEAD_BYTES)?;
    let returned_count = references.len();

    Ok(json!({
        "symbol": symbol_json(symbol),
        "references": references,
        "truncated": limit_truncated || soft_truncated,
        "returnedCount": returned_count,
        "timeoutMs": timeout_ms
    }))
}

pub(crate) fn graph_related_files(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GraphRelatedFilesInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let path = normalize_workspace_path_text(&request.path)?;
    let limit = graph_limit(request.limit)?;
    let database = open_code_graph_database(workspace_path)?;
    let mut files = database.code_graph_related_files(&path, graph_query_limit(limit)?)?;
    let limit_truncated = truncate_records(&mut files, limit);
    let records = files.into_iter().map(related_file_json).collect::<Vec<_>>();
    let (files, soft_truncated) =
        soft_limit_preview_records(records, GRAPH_LIST_RESPONSE_OVERHEAD_BYTES)?;
    let returned_count = files.len();

    Ok(json!({
        "path": path,
        "files": files,
        "truncated": limit_truncated || soft_truncated,
        "returnedCount": returned_count,
        "timeoutMs": timeout_ms
    }))
}

pub(crate) fn graph_find_imports(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GraphFindImportsInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let path = normalize_workspace_path_text(&request.path)?;
    let limit = graph_limit(request.limit)?;
    let database = open_code_graph_database(workspace_path)?;
    let mut imports =
        database.code_graph_imports(&path, request.resolved, graph_query_limit(limit)?)?;
    let limit_truncated = truncate_records(&mut imports, limit);
    let records = imports.into_iter().map(import_json).collect::<Vec<_>>();
    let (imports, soft_truncated) =
        soft_limit_preview_records(records, GRAPH_LIST_RESPONSE_OVERHEAD_BYTES)?;
    let returned_count = imports.len();

    Ok(json!({
        "path": path,
        "resolved": request.resolved,
        "imports": imports,
        "truncated": limit_truncated || soft_truncated,
        "returnedCount": returned_count,
        "timeoutMs": timeout_ms
    }))
}

pub(crate) fn graph_find_importers(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GraphFindImportersInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let path = normalize_workspace_path_text(&request.path)?;
    let limit = graph_limit(request.limit)?;
    let database = open_code_graph_database(workspace_path)?;
    let mut importers = database.code_graph_importers(&path, graph_query_limit(limit)?)?;
    let limit_truncated = truncate_records(&mut importers, limit);
    let records = importers.into_iter().map(import_json).collect::<Vec<_>>();
    let (importers, soft_truncated) =
        soft_limit_preview_records(records, GRAPH_LIST_RESPONSE_OVERHEAD_BYTES)?;
    let returned_count = importers.len();

    Ok(json!({
        "path": path,
        "importers": importers,
        "relationshipSemantics": "exact_workspace_module_resolution_only",
        "truncated": limit_truncated || soft_truncated,
        "returnedCount": returned_count,
        "timeoutMs": timeout_ms
    }))
}

pub(crate) fn graph_explore(
    workspace_path: &Path,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GraphExploreInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_GRAPH_TOOL_TIMEOUT_MS)?;
    let context_lines = graph_explore_context_lines(request.context_lines)?;
    let preview_limit = graph_explore_limit(request.limit)?;
    let database = open_code_graph_database(workspace_path)?;
    let (symbols, query, path) = resolve_graph_explore_symbols(&database, &request)?;
    let snippets = collect_graph_explore_snippets(workspace_path, symbols, context_lines)?;
    let (output, snapshot) = graph_explore_preview_response(
        query,
        path,
        context_lines,
        timeout_ms,
        &snippets,
        preview_limit,
    )?;
    if let Some(snapshot) = snapshot {
        write_graph_explore_snapshot(workspace_path, &snapshot)?;
    }

    Ok(output)
}

fn graph_explore_preview_response(
    query: Option<String>,
    path: Option<String>,
    context_lines: usize,
    timeout_ms: u64,
    snippets: &[GraphExploreSnippet],
    preview_limit: usize,
) -> Result<(Value, Option<GraphExploreSnapshot>), ToolRuntimeError> {
    let total_count = snippets.len();
    let requested_count = preview_limit.min(total_count);
    let mut returned_count = requested_count;
    let mut snapshot = None;

    loop {
        let truncated = returned_count < total_count;
        if truncated && snapshot.is_none() {
            snapshot = Some(prepare_graph_explore_snapshot(snippets)?);
        }

        let preview = snippets[..returned_count]
            .iter()
            .map(|snippet| snippet.value.clone())
            .collect::<Vec<_>>();
        let mut output = json!({
            "query": query,
            "path": path,
            "contextLines": context_lines,
            "snippets": preview,
            "truncated": truncated,
            "outputTruncated": returned_count < requested_count,
            "totalCount": total_count,
            "returnedCount": returned_count,
            "timeoutMs": timeout_ms
        });

        if let Some(snapshot) = snapshot.as_ref() {
            let next_start_line = snapshot
                .snippet_start_lines
                .get(returned_count)
                .copied()
                .ok_or_else(|| {
                    ToolRuntimeError::InvalidArguments(
                        "failed to render graph_explore snapshot continuation line".to_string(),
                    )
                })?;
            let full_result_path = graph_snapshot_text_relative_path(&snapshot.id);
            output["nextOffset"] = json!(returned_count);
            output["nextStartLine"] = json!(next_start_line);
            output["fullResultPath"] = json!(&full_result_path);
            output["note"] = json!(format!(
                "Results truncated: showing {returned_count} of {total_count} snippets. The complete stable snapshot is at '{full_result_path}'. Continue with read_file(path='{full_result_path}', startLine={next_start_line}, endLine=<small range>)."
            ));
        }

        if graph_explore_response_fits_soft_budget(&output)? {
            return Ok((output, snapshot));
        }
        let Some(next_returned_count) = returned_count.checked_sub(1) else {
            return Err(ToolRuntimeError::InvalidArguments(
                "graph_explore response metadata exceeds the shared output budget; refine query or path and try again".to_string(),
            ));
        };
        returned_count = next_returned_count;
    }
}

fn graph_explore_response_fits_soft_budget(output: &Value) -> Result<bool, ToolRuntimeError> {
    let measurement = measure_tool_execution(&ToolExecution {
        output: output.clone(),
        is_error: false,
    })
    .map_err(|source| {
        ToolRuntimeError::InvalidArguments(format!(
            "failed to measure graph_explore response output: {source}"
        ))
    })?;
    Ok(measurement.serialized_bytes <= TOOL_OUTPUT_SOFT_BYTE_LIMIT
        && measurement.text_lines <= TOOL_OUTPUT_SOFT_LINE_LIMIT)
}

fn open_code_graph_database(
    workspace_path: &Path,
) -> Result<foco_store::workspace::WorkspaceDatabaseHandle, ToolRuntimeError> {
    WorkspaceDatabase::open_or_create(workspace_path).map_err(ToolRuntimeError::WorkspaceDatabase)
}

fn resolve_graph_symbol(
    database: &WorkspaceDatabase,
    request: &GraphSymbolLookupInput,
) -> Result<CodeGraphSymbolRecord, ToolRuntimeError> {
    let symbol = request
        .symbol
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match (request.symbol_id, symbol) {
        (Some(_), Some(_)) => Err(ToolRuntimeError::InvalidArguments(
            "provide exactly one of symbolId or symbol".to_string(),
        )),
        (None, None) => Err(ToolRuntimeError::InvalidArguments(
            "provide exactly one of symbolId or symbol".to_string(),
        )),
        (Some(symbol_id), None) => {
            if request.path.is_some() {
                return Err(ToolRuntimeError::InvalidArguments(
                    "path can only be used when resolving by symbol name".to_string(),
                ));
            }

            database.code_graph_symbol(symbol_id)?.ok_or_else(|| {
                ToolRuntimeError::InvalidArguments(format!(
                    "code graph symbol was not found: {symbol_id}"
                ))
            })
        }
        (None, Some(symbol)) => {
            let path = request
                .path
                .as_deref()
                .map(normalize_workspace_path_text)
                .transpose()?;
            let matches = database.find_code_graph_symbols(symbol, None, path.as_deref(), 3)?;

            match matches.len() {
                0 => Err(ToolRuntimeError::InvalidArguments(format!(
                    "code graph symbol was not found: {symbol}"
                ))),
                1 => Ok(matches.into_iter().next().expect("one symbol")),
                _ => {
                    let candidates = matches
                        .into_iter()
                        .map(|candidate| {
                            format!("{}:{}:{}", candidate.id, candidate.path, candidate.name)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    Err(ToolRuntimeError::InvalidArguments(format!(
                        "symbol name is ambiguous; call graph_find_symbols first and pass symbolId. Candidates: {candidates}"
                    )))
                }
            }
        }
    }
}

fn resolve_graph_explore_symbols(
    database: &WorkspaceDatabase,
    request: &GraphExploreInput,
) -> Result<(Vec<CodeGraphSymbolRecord>, Option<String>, Option<String>), ToolRuntimeError> {
    let query = request
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match (request.symbol_id, query) {
        (Some(_), Some(_)) => Err(ToolRuntimeError::InvalidArguments(
            "provide exactly one of symbolId or query".to_string(),
        )),
        (None, None) => Err(ToolRuntimeError::InvalidArguments(
            "provide exactly one of symbolId or query".to_string(),
        )),
        (Some(symbol_id), None) => {
            if request.path.is_some() || request.kind.is_some() {
                return Err(ToolRuntimeError::InvalidArguments(
                    "path and kind can only be used when resolving by query".to_string(),
                ));
            }

            let symbol = database.code_graph_symbol(symbol_id)?.ok_or_else(|| {
                ToolRuntimeError::InvalidArguments(format!(
                    "code graph symbol was not found: {symbol_id}"
                ))
            })?;
            Ok((vec![symbol], None, None))
        }
        (None, Some(query)) => {
            let path = request
                .path
                .as_deref()
                .map(normalize_workspace_path_text)
                .transpose()?;
            let symbols = collect_graph_explore_symbols_pagewise(
                database,
                query,
                request.kind.as_deref(),
                path.as_deref(),
            )?;
            Ok((symbols, Some(query.to_string()), path))
        }
    }
}

fn collect_graph_explore_symbols_pagewise(
    database: &WorkspaceDatabase,
    query: &str,
    kind: Option<&str>,
    path: Option<&str>,
) -> Result<Vec<CodeGraphSymbolRecord>, ToolRuntimeError> {
    let mut symbols = Vec::new();
    let mut offset = 0_i64;

    loop {
        let page = database.find_code_graph_symbols_page(
            query,
            kind,
            path,
            GRAPH_EXPLORE_PAGE_SIZE,
            offset,
        )?;
        if page.is_empty() {
            break;
        }

        for symbol in page {
            if symbols.len() == MAX_GRAPH_EXPLORE_COLLECTION_SYMBOLS {
                return Err(graph_explore_collection_incomplete_error(format!(
                    "matched more than {MAX_GRAPH_EXPLORE_COLLECTION_SYMBOLS} symbols"
                )));
            }
            symbols.push(symbol);
        }

        offset = offset.checked_add(GRAPH_EXPLORE_PAGE_SIZE).ok_or_else(|| {
            graph_explore_collection_incomplete_error("pagination offset overflow".to_string())
        })?;
    }

    Ok(symbols)
}

struct GraphExploreSnippet {
    value: Value,
}

fn collect_graph_explore_snippets(
    workspace_path: &Path,
    symbols: Vec<CodeGraphSymbolRecord>,
    context_lines: usize,
) -> Result<Vec<GraphExploreSnippet>, ToolRuntimeError> {
    let mut snippets = Vec::with_capacity(symbols.len());
    let mut collection_bytes = 0usize;

    for symbol in symbols {
        let value = graph_symbol_source_snippet(workspace_path, symbol, context_lines)?;
        let serialized_bytes = serde_json::to_vec(&value)
            .map_err(|source| {
                ToolRuntimeError::InvalidArguments(format!(
                    "failed to serialize graph_explore snippet: {source}"
                ))
            })?
            .len();
        if collection_bytes.saturating_add(serialized_bytes) > MAX_GRAPH_EXPLORE_OUTPUT_BYTES {
            return Err(graph_explore_collection_incomplete_error(format!(
                "snippet data exceeds the {MAX_GRAPH_EXPLORE_OUTPUT_BYTES}-byte collection safety limit"
            )));
        }
        collection_bytes = collection_bytes.saturating_add(serialized_bytes);
        snippets.push(GraphExploreSnippet { value });
    }

    Ok(snippets)
}

fn graph_explore_collection_incomplete_error(detail: String) -> ToolRuntimeError {
    ToolRuntimeError::InvalidArguments(format!(
        "graph_explore collection incomplete: {detail}; refine query, kind, or path and try again"
    ))
}

struct GraphExploreSnapshot {
    id: String,
    contents: String,
    snippet_start_lines: Vec<usize>,
}

fn prepare_graph_explore_snapshot(
    snippets: &[GraphExploreSnippet],
) -> Result<GraphExploreSnapshot, ToolRuntimeError> {
    let (contents, snippet_start_lines) = render_graph_explore_snapshot(snippets)?;
    Ok(GraphExploreSnapshot {
        id: next_graph_snapshot_id(),
        contents,
        snippet_start_lines,
    })
}

fn write_graph_explore_snapshot(
    workspace_path: &Path,
    snapshot: &GraphExploreSnapshot,
) -> Result<(), ToolRuntimeError> {
    let results_dir = graph_results_dir(workspace_path)?;
    prune_graph_results_dir(&results_dir);

    let path = results_dir.join(format!("{}.txt", snapshot.id));
    fs::write(&path, &snapshot.contents).map_err(|source| ToolRuntimeError::Io { path, source })?;

    Ok(())
}

fn graph_results_dir(workspace_path: &Path) -> Result<PathBuf, ToolRuntimeError> {
    let workspace = fs::canonicalize(workspace_path).map_err(|source| ToolRuntimeError::Io {
        path: workspace_path.to_path_buf(),
        source,
    })?;
    let foco_dir = workspace.join(".foco");
    ensure_non_symlink_directory(&foco_dir)?;
    let results_dir = foco_dir.join(GRAPH_RESULTS_DIR);
    ensure_non_symlink_directory(&results_dir)?;

    let canonical_results_dir =
        fs::canonicalize(&results_dir).map_err(|source| ToolRuntimeError::Io {
            path: results_dir.clone(),
            source,
        })?;
    if !canonical_results_dir.starts_with(&workspace) {
        return Err(ToolRuntimeError::InvalidPath(format!(
            "graph_explore snapshot directory escapes the workspace: {}",
            results_dir.display()
        )));
    }

    Ok(canonical_results_dir)
}

fn ensure_non_symlink_directory(path: &Path) -> Result<(), ToolRuntimeError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(ToolRuntimeError::InvalidPath(format!(
                "graph_explore snapshot directory must not be a symbolic link: {}",
                path.display()
            )));
        }
        Ok(metadata) if metadata.is_dir() => return Ok(()),
        Ok(_) => {
            return Err(ToolRuntimeError::InvalidPath(format!(
                "graph_explore snapshot directory is not a directory: {}",
                path.display()
            )));
        }
        Err(source) if source.kind() == ErrorKind::NotFound => {}
        Err(source) => {
            return Err(ToolRuntimeError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    }

    fs::create_dir(path).map_err(|source| ToolRuntimeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    ensure_non_symlink_directory(path)
}

fn render_graph_explore_snapshot(
    snippets: &[GraphExploreSnippet],
) -> Result<(String, Vec<usize>), ToolRuntimeError> {
    let mut contents = String::new();
    let mut snippet_start_lines = Vec::with_capacity(snippets.len());
    let mut next_line = 1usize;

    for (index, snippet) in snippets.iter().enumerate() {
        snippet_start_lines.push(next_line);
        let symbol = snippet.value.get("symbol").ok_or_else(|| {
            ToolRuntimeError::InvalidArguments(
                "failed to render graph_explore snapshot: snippet is missing symbol".to_string(),
            )
        })?;
        let symbol_id = symbol
            .get("symbolId")
            .and_then(Value::as_i64)
            .ok_or_else(|| {
                ToolRuntimeError::InvalidArguments(
                    "failed to render graph_explore snapshot: symbol is missing symbolId"
                        .to_string(),
                )
            })?;
        let path = snippet
            .value
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ToolRuntimeError::InvalidArguments(
                    "failed to render graph_explore snapshot: snippet is missing path".to_string(),
                )
            })?;
        let name = symbol.get("name").and_then(Value::as_str).ok_or_else(|| {
            ToolRuntimeError::InvalidArguments(
                "failed to render graph_explore snapshot: symbol is missing name".to_string(),
            )
        })?;
        let qualified_name = symbol
            .get("qualifiedName")
            .and_then(Value::as_str)
            .unwrap_or(name);
        let kind = symbol
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let start_line = snippet
            .value
            .get("startLine")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ToolRuntimeError::InvalidArguments(
                    "failed to render graph_explore snapshot: snippet is missing startLine"
                        .to_string(),
                )
            })?;
        let end_line = snippet
            .value
            .get("endLine")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ToolRuntimeError::InvalidArguments(
                    "failed to render graph_explore snapshot: snippet is missing endLine"
                        .to_string(),
                )
            })?;
        let content = snippet
            .value
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ToolRuntimeError::InvalidArguments(
                    "failed to render graph_explore snapshot: snippet is missing content"
                        .to_string(),
                )
            })?;
        if longest_complete_line_bytes(content) > MAX_GRAPH_SNAPSHOT_LINE_BYTES {
            return Err(graph_explore_collection_incomplete_error(format!(
                "a snapshot source line exceeds the {MAX_GRAPH_SNAPSHOT_LINE_BYTES}-byte readable-line safety limit"
            )));
        }

        append_graph_snapshot_line(
            &mut contents,
            &mut next_line,
            &format!("=== graph_explore snippet {} ===", index + 1),
        );
        append_graph_snapshot_line(
            &mut contents,
            &mut next_line,
            &format!("symbolId: {symbol_id}"),
        );
        append_graph_snapshot_line(&mut contents, &mut next_line, &format!("path: {path}"));
        append_graph_snapshot_line(
            &mut contents,
            &mut next_line,
            &format!("symbol: {qualified_name} ({kind})"),
        );
        append_graph_snapshot_line(
            &mut contents,
            &mut next_line,
            &format!("sourceLines: {start_line}-{end_line}"),
        );
        append_graph_snapshot_line(&mut contents, &mut next_line, "source:");
        contents.push_str(content);
        if !content.ends_with('\n') {
            contents.push('\n');
        }
        next_line = next_line.saturating_add(count_text_lines(content));
        append_graph_snapshot_line(
            &mut contents,
            &mut next_line,
            &format!("=== end graph_explore snippet {} ===", index + 1),
        );
        append_graph_snapshot_line(&mut contents, &mut next_line, "");
    }

    Ok((contents, snippet_start_lines))
}

fn longest_complete_line_bytes(content: &str) -> usize {
    let bytes = content.as_bytes();
    let mut longest = 0;
    let mut start = 0;
    let mut index = 0;

    while index < bytes.len() {
        let end = match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => index + 2,
            b'\r' | b'\n' => index + 1,
            _ => {
                index += 1;
                continue;
            }
        };
        longest = longest.max(end - start);
        start = end;
        index = end;
    }

    if start < bytes.len() {
        longest.max(bytes.len() - start)
    } else {
        longest
    }
}

fn append_graph_snapshot_line(contents: &mut String, next_line: &mut usize, line: &str) {
    contents.push_str(line);
    contents.push('\n');
    *next_line = next_line.saturating_add(1);
}

fn graph_snapshot_text_relative_path(snapshot_id: &str) -> String {
    format!(".foco/{GRAPH_RESULTS_DIR}/{snapshot_id}.txt")
}

static GRAPH_RESULTS_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_graph_snapshot_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let counter = GRAPH_RESULTS_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("graph-explore-{nanos}-{counter}")
}

/// Best-effort cache cleanup mirrors search_text retention without allowing cleanup failures to
/// turn an otherwise usable graph result into an error.
fn prune_graph_results_dir(results_dir: &Path) {
    let Ok(read_dir) = fs::read_dir(results_dir) else {
        return;
    };

    let now = SystemTime::now();
    let mut snapshots: Vec<(PathBuf, SystemTime)> = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        let is_snapshot = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("graph-explore-") && name.ends_with(".txt"));
        if !is_snapshot {
            continue;
        }

        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        if now
            .duration_since(modified)
            .is_ok_and(|age| age > GRAPH_RESULT_TTL)
        {
            let _ = fs::remove_file(path);
            continue;
        }
        snapshots.push((path, modified));
    }

    if snapshots.len() < MAX_GRAPH_RESULT_FILES {
        return;
    }

    snapshots.sort_by_key(|(_, modified)| *modified);
    let remove_count = snapshots.len() + 1 - MAX_GRAPH_RESULT_FILES;
    for (path, _) in snapshots.into_iter().take(remove_count) {
        let _ = fs::remove_file(path);
    }
}

fn non_empty_argument<'a>(name: &str, value: &'a str) -> Result<&'a str, ToolRuntimeError> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        Err(ToolRuntimeError::InvalidArguments(format!(
            "{name} must not be empty"
        )))
    } else {
        Ok(trimmed)
    }
}

fn graph_limit(limit: Option<usize>) -> Result<usize, ToolRuntimeError> {
    let limit = limit.unwrap_or(DEFAULT_GRAPH_RESULT_LIMIT);

    if limit == 0 || limit > MAX_GRAPH_RESULT_LIMIT {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "limit must be between 1 and {MAX_GRAPH_RESULT_LIMIT}"
        )));
    }

    Ok(limit)
}

fn graph_explore_limit(limit: Option<usize>) -> Result<usize, ToolRuntimeError> {
    let limit = limit.unwrap_or(DEFAULT_GRAPH_EXPLORE_RESULT_LIMIT);

    if limit == 0 || limit > MAX_GRAPH_EXPLORE_RESULT_LIMIT {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "limit must be between 1 and {MAX_GRAPH_EXPLORE_RESULT_LIMIT}"
        )));
    }

    Ok(limit)
}

fn graph_explore_context_lines(context_lines: Option<usize>) -> Result<usize, ToolRuntimeError> {
    let context_lines = context_lines.unwrap_or(DEFAULT_GRAPH_EXPLORE_CONTEXT_LINES);

    if context_lines > MAX_GRAPH_EXPLORE_CONTEXT_LINES {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "contextLines must be between 0 and {MAX_GRAPH_EXPLORE_CONTEXT_LINES}"
        )));
    }

    Ok(context_lines)
}

fn graph_query_limit(limit: usize) -> Result<i64, ToolRuntimeError> {
    i64::try_from(limit + 1).map_err(|_| {
        ToolRuntimeError::InvalidArguments("limit is too large for SQLite".to_string())
    })
}

fn truncate_records<T>(records: &mut Vec<T>, limit: usize) -> bool {
    let truncated = records.len() > limit;
    records.truncate(limit);
    truncated
}

fn soft_limit_preview_records(
    records: Vec<Value>,
    overhead_bytes: usize,
) -> Result<(Vec<Value>, bool), ToolRuntimeError> {
    let count =
        soft_limit_array_prefix_len_with_overhead(&records, overhead_bytes).map_err(|source| {
            ToolRuntimeError::InvalidArguments(format!(
                "failed to measure graph result size: {source}"
            ))
        })?;
    let truncated = count < records.len();
    Ok((records.into_iter().take(count).collect(), truncated))
}

fn symbol_json(symbol: CodeGraphSymbolRecord) -> Value {
    json!({
        "symbolId": symbol.id,
        "path": symbol.path,
        "language": symbol.language,
        "name": symbol.name,
        "qualifiedName": symbol.qualified_name,
        "kind": symbol.kind,
        "visibility": symbol.visibility,
        "metadata": symbol.metadata_json,
        "startLine": symbol.start_line,
        "startColumn": symbol.start_column,
        "endLine": symbol.end_line,
        "endColumn": symbol.end_column,
        "signature": symbol.signature,
        "documentation": symbol.documentation
    })
}

fn graph_symbol_source_snippet(
    workspace_path: &Path,
    symbol: CodeGraphSymbolRecord,
    context_lines: usize,
) -> Result<Value, ToolRuntimeError> {
    let start_line = symbol
        .start_line
        .and_then(positive_i64_to_usize)
        .ok_or_else(|| {
            ToolRuntimeError::InvalidArguments(format!(
                "code graph symbol {}:{} has no startLine",
                symbol.path, symbol.name
            ))
        })?;
    let end_line = symbol
        .end_line
        .and_then(positive_i64_to_usize)
        .ok_or_else(|| {
            ToolRuntimeError::InvalidArguments(format!(
                "code graph symbol {}:{} has no endLine",
                symbol.path, symbol.name
            ))
        })?;
    if end_line < start_line {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "code graph symbol {}:{} has invalid line range {start_line}-{end_line}",
            symbol.path, symbol.name
        )));
    }

    let symbol_line_count = end_line - start_line + 1;
    if symbol_line_count > MAX_GRAPH_EXPLORE_SYMBOL_LINES {
        return Err(ToolRuntimeError::InvalidArguments(format!(
            "code graph symbol {}:{} spans {symbol_line_count} lines; max {MAX_GRAPH_EXPLORE_SYMBOL_LINES}",
            symbol.path, symbol.name
        )));
    }

    let path = resolve_workspace_file(workspace_path, &symbol.path)?;
    let metadata = fs::metadata(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;
    if metadata.len() > MAX_RANGED_READ_SOURCE_BYTES {
        return Err(ToolRuntimeError::FileTooLarge {
            path,
            bytes: metadata.len(),
            max_bytes: MAX_RANGED_READ_SOURCE_BYTES,
        });
    }
    let bytes = fs::read(&path).map_err(|source| ToolRuntimeError::Io {
        path: path.clone(),
        source,
    })?;
    let (content, _) = decode_text_file(&path, &bytes)?;
    let line_count = count_text_lines(&content);
    let range = normalize_read_line_range(
        LineRange::new(
            start_line.saturating_sub(context_lines).max(1),
            end_line + context_lines,
        )?,
        line_count,
    )?;
    let snippet = numbered_content(&read_line_range(&content, &range), range.start);

    let symbol_path = symbol.path.clone();

    Ok(json!({
        "symbol": symbol_json(symbol),
        "path": symbol_path,
        "startLine": range.start,
        "endLine": range.end,
        "content": snippet
    }))
}

fn positive_i64_to_usize(value: i64) -> Option<usize> {
    usize::try_from(value).ok().filter(|value| *value > 0)
}

fn relation_json(relation: CodeGraphSymbolRelationRecord) -> Value {
    json!({
        "edgeId": relation.edge_id,
        "edgeKind": relation.edge_kind,
        "metadata": relation.metadata_json,
        "source": symbol_json(relation.source),
        "target": symbol_json(relation.target)
    })
}

fn reference_json(reference: CodeGraphReferenceRecord) -> Value {
    json!({
        "referenceId": reference.id,
        "path": reference.path,
        "language": reference.language,
        "name": reference.name,
        "startLine": reference.start_line,
        "startColumn": reference.start_column,
        "endLine": reference.end_line,
        "endColumn": reference.end_column,
        "symbol": reference.symbol.map(symbol_json)
    })
}

fn related_file_json(file: CodeGraphRelatedFileRecord) -> Value {
    json!({
        "path": file.path,
        "language": file.language,
        "relation": file.relation,
        "score": file.score
    })
}

fn import_json(import: CodeGraphImportRecord) -> Value {
    let candidates = serde_json::from_str::<Value>(&import.candidates_json)
        .unwrap_or_else(|_| Value::Array(Vec::new()));
    let metadata = serde_json::from_str::<Value>(&import.metadata_json)
        .unwrap_or_else(|_| Value::String(import.metadata_json.clone()));
    json!({
        "importId": import.id,
        "sourcePath": import.path,
        "language": import.language,
        "module": import.module,
        "importedSymbol": import.imported_symbol,
        "alias": import.alias,
        "startLine": import.start_line,
        "startColumn": import.start_column,
        "resolution": import.resolution,
        "targetPath": import.target_path,
        "targetSymbol": import.target_symbol.map(symbol_json),
        "candidates": candidates,
        "provenance": metadata
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphFindSymbolsInput {
    pub(crate) query: String,
    pub(crate) kind: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) limit: Option<usize>,
    pub(crate) timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphSymbolLookupInput {
    pub(crate) symbol_id: Option<i64>,
    pub(crate) symbol: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) limit: Option<usize>,
    pub(crate) timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphFindChildrenInput {
    pub(crate) symbol_id: Option<i64>,
    pub(crate) symbol: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) limit: Option<usize>,
    pub(crate) timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphExploreInput {
    symbol_id: Option<i64>,
    query: Option<String>,
    kind: Option<String>,
    path: Option<String>,
    limit: Option<usize>,
    context_lines: Option<usize>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphRelatedFilesInput {
    path: String,
    limit: Option<usize>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphFindImportsInput {
    path: String,
    resolved: Option<bool>,
    limit: Option<usize>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphFindImportersInput {
    path: String,
    limit: Option<usize>,
    timeout_ms: Option<u64>,
}
