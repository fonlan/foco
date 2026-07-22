use std::collections::{BTreeSet, HashMap, HashSet};

use foco_store::workspace::{
    CodeGraphResolverFileRecord, CodeGraphResolverImportRecord, CodeGraphResolverReferenceRecord,
    CodeGraphResolverSnapshot, CodeGraphResolverSymbolRecord, NewCodeGraphImportResolution,
    NewCodeGraphImportResolutionCandidate, NewCodeGraphResolvedCall, WorkspaceDatabase,
};
use serde_json::{Value, json};

use crate::CodeGraphError;

const MODULE_RESOLVER_EXACT_CALL: &str = r#"{"semanticVersion":1,"provenance":"module_resolver","confidence":"exact","resolution":{"status":"resolved","candidates":[]}}"#;

#[derive(Clone)]
enum PathResolution<'a> {
    External,
    Unresolved,
    Candidates(Vec<&'a CodeGraphResolverFileRecord>),
}

#[derive(Clone)]
struct OwnedResolution {
    import_id: i64,
    resolution: &'static str,
    target_file_id: Option<i64>,
    target_symbol_id: Option<i64>,
    candidates: Vec<NewCodeGraphImportResolutionCandidate>,
    candidates_json: String,
    metadata_json: String,
}

pub(crate) fn resolve_workspace_imports(
    workspace_path: &std::path::Path,
) -> Result<(), CodeGraphError> {
    let snapshot = {
        let database = WorkspaceDatabase::open_or_create(workspace_path)?;
        database.code_graph_resolver_snapshot()?
    };
    let (resolutions, calls) = resolve_snapshot(&snapshot);
    let borrowed_resolutions = resolutions
        .iter()
        .map(|resolution| NewCodeGraphImportResolution {
            import_id: resolution.import_id,
            resolution: resolution.resolution,
            target_file_id: resolution.target_file_id,
            target_symbol_id: resolution.target_symbol_id,
            candidates: &resolution.candidates,
            candidates_json: &resolution.candidates_json,
            metadata_json: &resolution.metadata_json,
        })
        .collect::<Vec<_>>();
    let borrowed_calls = calls
        .iter()
        .map(
            |(source_symbol_id, target_symbol_id)| NewCodeGraphResolvedCall {
                source_symbol_id: *source_symbol_id,
                target_symbol_id: *target_symbol_id,
                metadata_json: MODULE_RESOLVER_EXACT_CALL,
            },
        )
        .collect::<Vec<_>>();
    let mut database = WorkspaceDatabase::open_or_create(workspace_path)?;
    database.replace_code_graph_import_resolutions(&borrowed_resolutions, &borrowed_calls)?;

    Ok(())
}

fn resolve_snapshot(
    snapshot: &CodeGraphResolverSnapshot,
) -> (Vec<OwnedResolution>, Vec<(i64, i64)>) {
    let files_by_path = snapshot
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<HashMap<_, _>>();
    let files_by_id = snapshot
        .files
        .iter()
        .map(|file| (file.id, file))
        .collect::<HashMap<_, _>>();
    let mut symbols_by_file = HashMap::<i64, Vec<&CodeGraphResolverSymbolRecord>>::new();
    for symbol in &snapshot.symbols {
        symbols_by_file
            .entry(symbol.file_id)
            .or_default()
            .push(symbol);
    }
    let symbols_by_id = snapshot
        .symbols
        .iter()
        .map(|symbol| (symbol.id, symbol))
        .collect::<HashMap<_, _>>();
    let mut references_by_file = HashMap::<i64, Vec<&CodeGraphResolverReferenceRecord>>::new();
    for reference in &snapshot.references {
        references_by_file
            .entry(reference.file_id)
            .or_default()
            .push(reference);
    }

    let mut resolutions = Vec::with_capacity(snapshot.imports.len());
    let mut calls = HashSet::new();
    for import in &snapshot.imports {
        let mut resolution = resolve_import(import, &files_by_path, &symbols_by_file);
        enrich_candidate_metadata(&mut resolution, &files_by_id, &symbols_by_id);
        if resolution.resolution == "exact"
            && let Some(target_symbol_id) = resolution.target_symbol_id
        {
            let local_name = import
                .alias
                .as_deref()
                .or(import.imported_symbol.as_deref());
            if let Some(local_name) = local_name
                && let Some(references) = references_by_file.get(&import.file_id)
            {
                for reference in references.iter().filter(|reference| {
                    reference.name == local_name && reference.symbol_id.is_none()
                }) {
                    if let Some(source_symbol) = containing_symbol(
                        symbols_by_file
                            .get(&import.file_id)
                            .map(Vec::as_slice)
                            .unwrap_or_default(),
                        reference,
                    ) && source_symbol.id != target_symbol_id
                    {
                        calls.insert((source_symbol.id, target_symbol_id));
                    }
                }
            }
        }
        resolutions.push(resolution);
    }

    let mut calls = calls.into_iter().collect::<Vec<_>>();
    calls.sort_unstable();
    (resolutions, calls)
}

fn resolve_import(
    import: &CodeGraphResolverImportRecord,
    files_by_path: &HashMap<&str, &CodeGraphResolverFileRecord>,
    symbols_by_file: &HashMap<i64, Vec<&CodeGraphResolverSymbolRecord>>,
) -> OwnedResolution {
    let path_resolution = module_path_resolution(import, files_by_path);
    match path_resolution {
        PathResolution::External => owned_resolution(import.id, "external", None, None, Vec::new()),
        PathResolution::Unresolved => {
            owned_resolution(import.id, "unresolved", None, None, Vec::new())
        }
        PathResolution::Candidates(files) if files.len() != 1 => {
            let candidates = files
                .into_iter()
                .map(|file| NewCodeGraphImportResolutionCandidate {
                    target_file_id: file.id,
                    target_symbol_id: None,
                })
                .collect();
            owned_resolution(import.id, "candidate", None, None, candidates)
        }
        PathResolution::Candidates(files) => {
            let target_file = files[0];
            let imported_symbol = import.imported_symbol.as_deref();
            if matches!(imported_symbol, None | Some("*")) {
                return owned_resolution(
                    import.id,
                    "exact",
                    Some(target_file.id),
                    None,
                    Vec::new(),
                );
            }
            let imported_symbol = imported_symbol.unwrap_or_default();
            let matches = symbols_by_file
                .get(&target_file.id)
                .into_iter()
                .flatten()
                .filter(|symbol| {
                    (symbol.name == imported_symbol && symbol_is_exported(symbol))
                        || (imported_symbol == "default" && symbol_is_default_export(symbol))
                })
                .copied()
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [symbol] => owned_resolution(
                    import.id,
                    "exact",
                    Some(target_file.id),
                    Some(symbol.id),
                    Vec::new(),
                ),
                [] => owned_resolution(import.id, "exact", Some(target_file.id), None, Vec::new()),
                _ => {
                    let candidates = matches
                        .into_iter()
                        .map(|symbol| NewCodeGraphImportResolutionCandidate {
                            target_file_id: target_file.id,
                            target_symbol_id: Some(symbol.id),
                        })
                        .collect();
                    owned_resolution(import.id, "candidate", None, None, candidates)
                }
            }
        }
    }
}

fn enrich_candidate_metadata(
    resolution: &mut OwnedResolution,
    files_by_id: &HashMap<i64, &CodeGraphResolverFileRecord>,
    symbols_by_id: &HashMap<i64, &CodeGraphResolverSymbolRecord>,
) {
    let candidates = Value::Array(
        resolution
            .candidates
            .iter()
            .map(|candidate| {
                let file = files_by_id.get(&candidate.target_file_id);
                let symbol = candidate
                    .target_symbol_id
                    .and_then(|symbol_id| symbols_by_id.get(&symbol_id));
                json!({
                    "targetFileId": candidate.target_file_id,
                    "targetSymbolId": candidate.target_symbol_id,
                    "path": file.map(|file| file.path.as_str()),
                    "language": file.and_then(|file| file.language.as_deref()),
                    "targetSymbol": symbol.map(|symbol| json!({
                        "id": symbol.id,
                        "name": symbol.name,
                        "qualifiedName": symbol.qualified_name,
                        "kind": symbol.kind,
                        "startLine": symbol.start_line,
                        "startColumn": symbol.start_column,
                    })),
                })
            })
            .collect(),
    );
    resolution.candidates_json = candidates.to_string();
    resolution.metadata_json = json!({
        "semanticVersion": 1,
        "provenance": "module_resolver",
        "confidence": resolution.resolution,
        "resolution": {
            "status": resolution.resolution,
            "candidates": candidates,
        },
    })
    .to_string();
}

fn owned_resolution(
    import_id: i64,
    resolution: &'static str,
    target_file_id: Option<i64>,
    target_symbol_id: Option<i64>,
    candidates: Vec<NewCodeGraphImportResolutionCandidate>,
) -> OwnedResolution {
    let candidates_value = Value::Array(
        candidates
            .iter()
            .map(|candidate| {
                json!({
                "targetFileId": candidate.target_file_id,
                "targetSymbolId": candidate.target_symbol_id,
                })
            })
            .collect(),
    );
    let candidates_json = candidates_value.to_string();
    let metadata_json = json!({
        "semanticVersion": 1,
        "provenance": "module_resolver",
        "confidence": resolution,
        "resolution": {
            "status": resolution,
            "candidates": candidates_value,
        },
    })
    .to_string();

    OwnedResolution {
        import_id,
        resolution,
        target_file_id,
        target_symbol_id,
        candidates,
        candidates_json,
        metadata_json,
    }
}

fn module_path_resolution<'a>(
    import: &CodeGraphResolverImportRecord,
    files_by_path: &HashMap<&str, &'a CodeGraphResolverFileRecord>,
) -> PathResolution<'a> {
    let language = import.language.as_deref();
    let candidate_paths = match language {
        Some("typescript") | Some("javascript") | Some("tsx") | Some("jsx") | Some("ets") => {
            typescript_module_candidates(import)
        }
        Some("rust") => rust_module_candidates(import),
        _ => return PathResolution::Unresolved,
    };
    let Some(candidate_paths) = candidate_paths else {
        return PathResolution::External;
    };
    let candidates = candidate_paths
        .into_iter()
        .filter_map(|path| files_by_path.get(path.as_str()).copied())
        .collect::<Vec<_>>();
    match candidates.len() {
        0 => PathResolution::Unresolved,
        _ => PathResolution::Candidates(candidates),
    }
}

fn typescript_module_candidates(import: &CodeGraphResolverImportRecord) -> Option<Vec<String>> {
    if !import.module.starts_with('.') {
        return None;
    }
    let base = path_parent(&import.path);
    let path = normalize_join(&base, &import.module);
    let extensions = ["ts", "tsx", "js", "jsx", "mjs", "cjs", "ets"];
    let mut candidates = BTreeSet::new();
    if has_typescript_extension(&path) {
        candidates.insert(path.clone());
    }
    for extension in extensions {
        candidates.insert(format!("{path}.{extension}"));
        candidates.insert(format!("{path}/index.{extension}"));
    }
    Some(candidates.into_iter().collect())
}

fn has_typescript_extension(path: &str) -> bool {
    [".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".ets"]
        .iter()
        .any(|extension| path.ends_with(extension))
}

fn rust_module_candidates(import: &CodeGraphResolverImportRecord) -> Option<Vec<String>> {
    let mut segments = import
        .module
        .split("::")
        .filter(|segment| !segment.is_empty());
    let first = segments.next()?;
    let rest = segments.collect::<Vec<_>>();
    let base = match first {
        "crate" => vec!["src".to_string(), String::new()],
        "self" => vec![rust_module_directory(&import.path)],
        "super" => {
            let mut directory = rust_module_directory(&import.path);
            directory = path_parent(&directory);
            vec![directory]
        }
        _ => return None,
    };
    let mut candidates = BTreeSet::new();
    for base in base {
        let module_path = join_segments(&base, &rest);
        if module_path.is_empty() {
            candidates.insert("src/lib.rs".to_string());
            candidates.insert("lib.rs".to_string());
        } else {
            candidates.insert(format!("{module_path}.rs"));
            candidates.insert(format!("{module_path}/mod.rs"));
        }
    }
    Some(candidates.into_iter().collect())
}

fn rust_module_directory(path: &str) -> String {
    let parent = path_parent(path);
    let file_name = path.rsplit('/').next().unwrap_or(path);
    match file_name {
        "lib.rs" | "main.rs" | "mod.rs" => parent,
        _ => {
            let stem = file_name.strip_suffix(".rs").unwrap_or(file_name);
            join_segments(&parent, &[stem])
        }
    }
}

fn path_parent(path: &str) -> String {
    path.rsplit_once('/')
        .map_or_else(String::new, |(parent, _)| parent.to_string())
}

fn normalize_join(base: &str, module: &str) -> String {
    let mut components = base
        .split('/')
        .filter(|component| !component.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    for component in module.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            _ => components.push(component.to_string()),
        }
    }
    components.join("/")
}

fn join_segments(base: &str, segments: &[&str]) -> String {
    let mut values = base
        .split('/')
        .filter(|component| !component.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    values.extend(segments.iter().map(|segment| (*segment).to_string()));
    values.join("/")
}

fn symbol_is_exported(symbol: &CodeGraphResolverSymbolRecord) -> bool {
    symbol.visibility.as_deref() == Some("public")
        || symbol.metadata_json.contains("\"exported\":true")
}

fn symbol_is_default_export(symbol: &CodeGraphResolverSymbolRecord) -> bool {
    symbol.metadata_json.contains("\"defaultExport\":true")
}

fn containing_symbol<'a>(
    symbols: &'a [&CodeGraphResolverSymbolRecord],
    reference: &CodeGraphResolverReferenceRecord,
) -> Option<&'a CodeGraphResolverSymbolRecord> {
    let line = reference.start_line?;
    let column = reference.start_column?;
    symbols
        .iter()
        .copied()
        .filter(|symbol| {
            let (Some(start_line), Some(start_column), Some(end_line), Some(end_column)) = (
                symbol.start_line,
                symbol.start_column,
                symbol.end_line,
                symbol.end_column,
            ) else {
                return false;
            };
            (line, column) >= (start_line, start_column) && (line, column) <= (end_line, end_column)
        })
        .min_by_key(|symbol| {
            let start_line = symbol.start_line.unwrap_or_default();
            let end_line = symbol.end_line.unwrap_or(start_line);
            let start_column = symbol.start_column.unwrap_or_default();
            let end_column = symbol.end_column.unwrap_or(start_column);
            ((end_line - start_line) * 10_000) + (end_column - start_column)
        })
}
