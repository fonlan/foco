use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Component, Path, PathBuf},
    time::{Instant, SystemTime},
};

use chrono::{DateTime, SecondsFormat, Utc};
use foco_store::workspace::{
    NewCodeGraphEdge, NewCodeGraphFileIndex, NewCodeGraphImport, NewCodeGraphReference,
    NewCodeGraphSymbol, WorkspaceDatabase,
};
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};

use crate::{CodeGraphError, IndexReport, io_error};

use crate::extractors::{
    detect_language, extract_file,
    facts::{ExtractedEdgeTarget, ExtractedGraphFile, ExtractedPosition, file_local_key},
};

const CODE_GRAPH_WRITE_BATCH_SIZE: usize = 16;

pub(crate) fn index_workspace(workspace_path: &Path) -> Result<IndexReport, CodeGraphError> {
    let workspace_path = canonical_workspace_path(workspace_path)?;
    let files = discover_workspace_files(&workspace_path)?;

    // Short permit: load existing hashes only.
    let existing_hashes = {
        let database = WorkspaceDatabase::open_or_create(&workspace_path)?;
        database.code_graph_file_hashes()?
    };
    let mut report = IndexReport::default();
    let mut live_paths = Vec::new();
    let mut pending_writes = Vec::new();

    for file_path in files {
        report.scanned_files += 1;
        let prepare_started_at = Instant::now();
        let prepared = prepare_workspace_file(&workspace_path, &file_path, &existing_hashes)?;
        report.file_prepare_duration_us = report
            .file_prepare_duration_us
            .saturating_add(duration_micros(prepare_started_at.elapsed()));
        match prepared {
            FilePrepareOutcome::Indexed(prepared) => {
                if prepared.had_parse_error {
                    report.parse_errors += 1;
                }
                live_paths.push(prepared.relative_path.clone());
                pending_writes.push(*prepared);
                report.indexed_files += 1;
            }
            FilePrepareOutcome::Unchanged { relative_path } => {
                report.unchanged_files += 1;
                live_paths.push(relative_path);
            }
            FilePrepareOutcome::Skipped => report.skipped_files += 1,
        }

        // Flush writes in small batches so a permit is not held for the whole workspace.
        if pending_writes.len() >= CODE_GRAPH_WRITE_BATCH_SIZE {
            let write_started_at = Instant::now();
            flush_prepared_indexes(&workspace_path, &mut pending_writes)?;
            report.sqlite_persistence_duration_us = report
                .sqlite_persistence_duration_us
                .saturating_add(duration_micros(write_started_at.elapsed()));
        }
    }

    if !pending_writes.is_empty() {
        let write_started_at = Instant::now();
        flush_prepared_indexes(&workspace_path, &mut pending_writes)?;
        report.sqlite_persistence_duration_us = report
            .sqlite_persistence_duration_us
            .saturating_add(duration_micros(write_started_at.elapsed()));
    }

    // Short permit: stale cleanup only.
    {
        let write_started_at = Instant::now();
        let mut database = WorkspaceDatabase::open_or_create(&workspace_path)?;
        report.deleted_files = database.remove_stale_code_graph_files(&live_paths)?.len();
        report.sqlite_persistence_duration_us = report
            .sqlite_persistence_duration_us
            .saturating_add(duration_micros(write_started_at.elapsed()));
    }
    // A resolver-only pass refreshes dependency-derived relationships even when
    // importer content hashes are unchanged. It does not reparse those files.
    let resolver_started_at = Instant::now();
    crate::resolver::resolve_workspace_imports(&workspace_path)?;
    report.resolver_duration_us = duration_micros(resolver_started_at.elapsed());

    Ok(report)
}

/// Refreshes a bounded, caller-provided set of workspace paths without walking
/// the workspace. Watchers use this for ordinary source saves; full discovery
/// remains the recovery path for unclassifiable filesystem events.
pub(crate) fn index_workspace_paths(
    workspace_path: &Path,
    dirty_paths: impl IntoIterator<Item = PathBuf>,
) -> Result<IndexReport, CodeGraphError> {
    let workspace_path = canonical_workspace_path(workspace_path)?;
    let mut files = dirty_paths
        .into_iter()
        .filter_map(|path| normalize_dirty_path(&workspace_path, &path))
        .filter(|path| should_consider_watch_path(&workspace_path, path))
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();

    let existing_hashes = {
        let database = WorkspaceDatabase::open_or_create(&workspace_path)?;
        database.code_graph_file_hashes()?
    };
    let resolver_dependents = {
        let database = WorkspaceDatabase::open_or_create(&workspace_path)?;
        let mut dependents = HashMap::new();
        for file_path in &files {
            let relative_path = workspace_relative_path(&workspace_path, file_path)?;
            let paths = database.code_graph_resolution_dependent_paths(&relative_path)?;
            dependents.insert(relative_path, paths);
        }
        dependents
    };
    let mut report = IndexReport::default();
    let mut pending_writes = Vec::new();
    let mut affected_paths = HashSet::new();
    let mut requires_full_resolver = false;

    for file_path in files {
        report.scanned_files = report.scanned_files.saturating_add(1);
        let relative_path = workspace_relative_path(&workspace_path, &file_path)?;
        let prepare_started_at = Instant::now();
        match fs::metadata(&file_path) {
            Ok(metadata) if metadata.is_file() => {
                if !existing_hashes.contains_key(&relative_path) {
                    // A newly created target can satisfy imports that were
                    // previously unresolved, so its complete impact is not
                    // represented by durable reverse-resolution rows yet.
                    requires_full_resolver = true;
                }
                let prepared =
                    prepare_workspace_file(&workspace_path, &file_path, &existing_hashes)?;
                report.file_prepare_duration_us = report
                    .file_prepare_duration_us
                    .saturating_add(duration_micros(prepare_started_at.elapsed()));
                match prepared {
                    FilePrepareOutcome::Indexed(prepared) => {
                        if prepared.had_parse_error {
                            report.parse_errors = report.parse_errors.saturating_add(1);
                        }
                        pending_writes.push(*prepared);
                        affected_paths.insert(relative_path.clone());
                        if let Some(dependents) = resolver_dependents.get(&relative_path) {
                            affected_paths.extend(dependents.iter().cloned());
                        }
                        report.indexed_files = report.indexed_files.saturating_add(1);
                    }
                    FilePrepareOutcome::Unchanged { .. } => {
                        report.unchanged_files = report.unchanged_files.saturating_add(1);
                    }
                    FilePrepareOutcome::Skipped => {
                        report.skipped_files = report.skipped_files.saturating_add(1);
                    }
                }
            }
            Ok(_) => {
                // Directories are filtered by the watcher while they exist. A
                // directory that disappeared before the callback is handled by
                // the safe full-refresh fallback in the watcher instead.
                report.skipped_files = report.skipped_files.saturating_add(1);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // Deletion can invalidate exact, candidate, and unresolved
                // import state. Use the full resolver as the correctness path.
                requires_full_resolver = true;
                let write_started_at = Instant::now();
                let mut database = WorkspaceDatabase::open_or_create(&workspace_path)?;
                if database.delete_code_graph_file(&relative_path)? {
                    report.deleted_files = report.deleted_files.saturating_add(1);
                }
                report.sqlite_persistence_duration_us = report
                    .sqlite_persistence_duration_us
                    .saturating_add(duration_micros(write_started_at.elapsed()));
            }
            Err(source) => return Err(io_error(&file_path, source)),
        }
    }

    if !pending_writes.is_empty() {
        let write_started_at = Instant::now();
        flush_prepared_indexes(&workspace_path, &mut pending_writes)?;
        report.sqlite_persistence_duration_us = report
            .sqlite_persistence_duration_us
            .saturating_add(duration_micros(write_started_at.elapsed()));
    }

    let resolver_started_at = Instant::now();
    if requires_full_resolver {
        crate::resolver::resolve_workspace_imports(&workspace_path)?;
    } else {
        crate::resolver::resolve_workspace_imports_for_paths(&workspace_path, &affected_paths)?;
    }
    report.resolver_duration_us = duration_micros(resolver_started_at.elapsed());

    Ok(report)
}

fn normalize_dirty_path(workspace_path: &Path, path: &Path) -> Option<PathBuf> {
    if path.starts_with(workspace_path) {
        return Some(path.to_path_buf());
    }

    // macOS exposes `/var` as a symlink to `/private/var`; watcher callbacks
    // and callers can therefore use a spelling that differs from the canonical
    // workspace root. Canonicalizing the parent also works after a file delete.
    let parent = path.parent()?;
    let file_name = path.file_name()?;
    let canonical_parent = fs::canonicalize(parent).ok()?;
    let normalized = canonical_parent.join(file_name);
    normalized.starts_with(workspace_path).then_some(normalized)
}

fn duration_micros(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

struct PreparedFileIndex {
    relative_path: String,
    language: &'static str,
    size_bytes: Option<i64>,
    modified_at: Option<String>,
    content_hash: String,
    graph: ExtractedGraphFile,
    fts_body: String,
    had_parse_error: bool,
}

enum FilePrepareOutcome {
    Indexed(Box<PreparedFileIndex>),
    Unchanged { relative_path: String },
    Skipped,
}

fn flush_prepared_indexes(
    workspace_path: &Path,
    pending: &mut Vec<PreparedFileIndex>,
) -> Result<(), CodeGraphError> {
    if pending.is_empty() {
        return Ok(());
    }
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)?;
    for prepared in pending.drain(..) {
        let graph = &prepared.graph;
        debug_assert_eq!(graph.local_key, file_local_key(&prepared.relative_path));
        let node_indices = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.local_key.as_str(), index))
            .collect::<HashMap<_, _>>();
        let symbol_rows = graph
            .nodes
            .iter()
            .map(|node| NewCodeGraphSymbol {
                name: &node.name,
                qualified_name: &node.qualified_name,
                kind: node.kind,
                visibility: node.visibility,
                metadata_json: Some(&node.metadata_json),
                start_line: Some(position_line(node.range.start)),
                start_column: Some(position_column(node.range.start)),
                end_line: Some(position_line(node.range.end)),
                end_column: Some(position_column(node.range.end)),
                signature: node.signature.as_deref(),
                documentation: node.documentation.as_deref(),
            })
            .collect::<Vec<_>>();
        let import_rows = graph
            .imports
            .iter()
            .map(|import| NewCodeGraphImport {
                module: &import.module,
                imported_symbol: import.imported_symbol.as_deref(),
                alias: import.alias.as_deref(),
                start_line: Some(position_line(import.range.start)),
                start_column: Some(position_column(import.range.start)),
            })
            .collect::<Vec<_>>();
        let reference_rows = graph
            .references
            .iter()
            .map(|reference| NewCodeGraphReference {
                name: &reference.name,
                symbol_index: reference
                    .target_local_key
                    .as_deref()
                    .and_then(|key| node_indices.get(key).copied()),
                start_line: Some(position_line(reference.range.start)),
                start_column: Some(position_column(reference.range.start)),
                end_line: Some(position_line(reference.range.end)),
                end_column: Some(position_column(reference.range.end)),
            })
            .collect::<Vec<_>>();
        let edge_rows = graph
            .edges
            .iter()
            .filter_map(|edge| {
                let ExtractedEdgeTarget::Local(target_local_key) = &edge.target else {
                    return None;
                };
                let source_symbol_index =
                    node_indices.get(edge.source_local_key.as_str()).copied()?;
                let target_symbol_index = node_indices.get(target_local_key.as_str()).copied()?;
                Some(NewCodeGraphEdge {
                    source_symbol_index,
                    target_symbol_index,
                    edge_kind: edge.edge_kind,
                    metadata_json: Some(&edge.metadata_json),
                })
            })
            .collect::<Vec<_>>();

        database.replace_code_graph_file_index(NewCodeGraphFileIndex {
            path: &prepared.relative_path,
            language: Some(prepared.language),
            size_bytes: prepared.size_bytes,
            modified_at: prepared.modified_at.as_deref(),
            content_hash: &prepared.content_hash,
            parse_status: graph.parse_status,
            parse_error_message: graph.parse_error_message.as_deref(),
            symbols: &symbol_rows,
            imports: &import_rows,
            references: &reference_rows,
            edges: &edge_rows,
            fts_body: &prepared.fts_body,
        })?;
    }
    Ok(())
}

fn prepare_workspace_file(
    workspace_path: &Path,
    file_path: &Path,
    existing_hashes: &HashMap<String, String>,
) -> Result<FilePrepareOutcome, CodeGraphError> {
    let bytes = fs::read(file_path).map_err(|source| io_error(file_path, source))?;
    let text = std::str::from_utf8(&bytes);
    let language = detect_language(file_path, text.ok());
    let Some(language) = language else {
        return Ok(FilePrepareOutcome::Skipped);
    };

    let relative_path = workspace_relative_path(workspace_path, file_path)?;
    let content_hash = content_hash(&bytes);
    if existing_hashes.get(&relative_path) == Some(&content_hash) {
        return Ok(FilePrepareOutcome::Unchanged { relative_path });
    }

    let metadata = fs::metadata(file_path).map_err(|source| io_error(file_path, source))?;
    let modified_at = metadata.modified().ok().map(system_time_to_timestamp);
    let graph = match text {
        Ok(text) => extract_file(language, &relative_path, file_path, text)?,
        Err(_) => ExtractedGraphFile {
            local_key: file_local_key(&relative_path),
            parse_status: "error",
            parse_error_message: Some("file is not valid UTF-8".to_string()),
            nodes: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            edges: Vec::new(),
        },
    };
    let had_parse_error = graph.parse_status == "error";
    let size_bytes = i64::try_from(bytes.len()).ok();
    let fts_body = text.unwrap_or_default().to_string();

    Ok(FilePrepareOutcome::Indexed(Box::new(PreparedFileIndex {
        relative_path,
        language: language.name(),
        size_bytes,
        modified_at,
        content_hash,
        graph,
        fts_body,
        had_parse_error,
    })))
}

pub(crate) fn should_consider_watch_path(workspace_path: &Path, path: &Path) -> bool {
    path.starts_with(workspace_path) && !is_internal_path(workspace_path, path)
}

fn discover_workspace_files(workspace_path: &Path) -> Result<Vec<PathBuf>, CodeGraphError> {
    let mut files = Vec::new();
    let mut builder = WalkBuilder::new(workspace_path);
    builder
        .standard_filters(true)
        .hidden(false)
        .parents(true)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .require_git(false);

    for entry in builder.build() {
        let entry = entry.map_err(|source| CodeGraphError::Io {
            path: workspace_path.to_path_buf(),
            source: io::Error::other(source),
        })?;
        let path = entry.path();

        if path == workspace_path || is_internal_path(workspace_path, path) {
            continue;
        }

        if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            files.push(path.to_path_buf());
        }
    }

    files.sort();
    Ok(files)
}

fn is_internal_path(workspace_path: &Path, path: &Path) -> bool {
    let Ok(relative_path) = path.strip_prefix(workspace_path) else {
        return true;
    };

    relative_path.components().any(|component| {
        let Component::Normal(value) = component else {
            return false;
        };
        matches!(
            value.to_string_lossy().as_ref(),
            ".git" | ".foco" | ".codegraph" | ".mem" | "node_modules" | "target" | "dist"
        )
    })
}

fn workspace_relative_path(workspace_path: &Path, path: &Path) -> Result<String, CodeGraphError> {
    let relative_path =
        path.strip_prefix(workspace_path)
            .map_err(|_| CodeGraphError::WorkspaceRelativePath {
                workspace: workspace_path.to_path_buf(),
                path: path.to_path_buf(),
            })?;
    let value = relative_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");

    Ok(value)
}

fn canonical_workspace_path(path: &Path) -> Result<PathBuf, CodeGraphError> {
    fs::canonicalize(path).map_err(|source| io_error(path, source))
}

fn content_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn system_time_to_timestamp(value: SystemTime) -> String {
    let value: DateTime<Utc> = value.into();
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

// Persistence retains the existing 0-based Tree-sitter coordinates. Keeping
// this conversion in one module is the boundary future UI-coordinate changes
// must update, rather than leaking database row ids into extractors.
fn position_line(position: ExtractedPosition) -> i64 {
    i64::from(position.line)
}

fn position_column(position: ExtractedPosition) -> i64 {
    i64::from(position.column)
}
