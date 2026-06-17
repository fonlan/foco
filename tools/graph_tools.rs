use std::{fs, path::Path};

use foco_store::workspace::{
    CodeGraphReferenceRecord, CodeGraphRelatedFileRecord, CodeGraphSymbolRecord,
    CodeGraphSymbolRelationRecord, WorkspaceDatabase,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    DEFAULT_GRAPH_EXPLORE_CONTEXT_LINES, DEFAULT_GRAPH_EXPLORE_RESULT_LIMIT,
    DEFAULT_GRAPH_RESULT_LIMIT, DEFAULT_GRAPH_TOOL_TIMEOUT_MS, LineRange,
    MAX_GRAPH_EXPLORE_CONTEXT_LINES, MAX_GRAPH_EXPLORE_OUTPUT_BYTES,
    MAX_GRAPH_EXPLORE_RESULT_LIMIT, MAX_GRAPH_EXPLORE_SYMBOL_LINES, MAX_GRAPH_RESULT_LIMIT,
    MAX_RANGED_READ_SOURCE_BYTES, count_text_lines, decode_text_file,
    errors::{ToolRuntimeError, tool_timeout_ms},
    normalize_read_line_range, normalize_workspace_path_text, numbered_content, parse_arguments,
    read_line_range, resolve_workspace_file,
};

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
    let truncated = truncate_records(&mut symbols, limit);

    Ok(json!({
        "query": query,
        "kind": request.kind,
        "path": path,
        "symbols": symbols.into_iter().map(symbol_json).collect::<Vec<_>>(),
        "truncated": truncated,
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
    let truncated = truncate_records(&mut callers, limit);

    Ok(json!({
        "symbol": symbol_json(symbol),
        "callers": callers.into_iter().map(relation_json).collect::<Vec<_>>(),
        "truncated": truncated,
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
    let truncated = truncate_records(&mut callees, limit);

    Ok(json!({
        "symbol": symbol_json(symbol),
        "callees": callees.into_iter().map(relation_json).collect::<Vec<_>>(),
        "truncated": truncated,
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
    let truncated = truncate_records(&mut references, limit);

    Ok(json!({
        "symbol": symbol_json(symbol),
        "references": references.into_iter().map(reference_json).collect::<Vec<_>>(),
        "truncated": truncated,
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
    let truncated = truncate_records(&mut files, limit);

    Ok(json!({
        "path": path,
        "files": files.into_iter().map(related_file_json).collect::<Vec<_>>(),
        "truncated": truncated,
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
    let database = open_code_graph_database(workspace_path)?;
    let (symbols, query, path, truncated_matches) =
        resolve_graph_explore_symbols(&database, &request)?;
    let mut snippets = Vec::new();
    let mut output_bytes = 0usize;
    let mut output_truncated = false;

    for symbol in symbols {
        let snippet = graph_symbol_source_snippet(workspace_path, symbol, context_lines)?;
        let content_bytes = snippet["content"]
            .as_str()
            .map(str::len)
            .unwrap_or_default();
        if output_bytes.saturating_add(content_bytes) > MAX_GRAPH_EXPLORE_OUTPUT_BYTES {
            output_truncated = true;
            break;
        }
        output_bytes = output_bytes.saturating_add(content_bytes);
        snippets.push(snippet);
    }

    Ok(json!({
        "query": query,
        "path": path,
        "contextLines": context_lines,
        "snippets": snippets,
        "truncated": truncated_matches || output_truncated,
        "outputTruncated": output_truncated,
        "timeoutMs": timeout_ms
    }))
}

fn open_code_graph_database(workspace_path: &Path) -> Result<WorkspaceDatabase, ToolRuntimeError> {
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
) -> Result<
    (
        Vec<CodeGraphSymbolRecord>,
        Option<String>,
        Option<String>,
        bool,
    ),
    ToolRuntimeError,
> {
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
            Ok((vec![symbol], None, None, false))
        }
        (None, Some(query)) => {
            let limit = graph_explore_limit(request.limit)?;
            let path = request
                .path
                .as_deref()
                .map(normalize_workspace_path_text)
                .transpose()?;
            let mut symbols = database.find_code_graph_symbols(
                query,
                request.kind.as_deref(),
                path.as_deref(),
                graph_query_limit(limit)?,
            )?;
            let truncated = truncate_records(&mut symbols, limit);
            Ok((symbols, Some(query.to_string()), path, truncated))
        }
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

fn symbol_json(symbol: CodeGraphSymbolRecord) -> Value {
    json!({
        "symbolId": symbol.id,
        "path": symbol.path,
        "language": symbol.language,
        "name": symbol.name,
        "kind": symbol.kind,
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
