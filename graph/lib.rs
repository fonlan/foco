use std::{
    collections::{HashMap, HashSet},
    fmt, fs, io,
    path::{Component, Path, PathBuf},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime},
};

use chrono::{DateTime, SecondsFormat, Utc};
use foco_store::workspace::{
    NewCodeGraphEdge, NewCodeGraphFileIndex, NewCodeGraphImport, NewCodeGraphReference,
    NewCodeGraphSymbol, WorkspaceDatabase, WorkspaceDatabaseError,
};
use ignore::WalkBuilder;
use notify::{RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use tree_sitter::{Language, Node, Parser, Point};

const DEFAULT_WATCH_DEBOUNCE: Duration = Duration::from_millis(750);
const MAX_SIGNATURE_CHARS: usize = 240;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexReport {
    pub scanned_files: usize,
    pub indexed_files: usize,
    pub unchanged_files: usize,
    pub skipped_files: usize,
    pub deleted_files: usize,
    pub parse_errors: usize,
}

pub fn index_workspace(workspace_path: impl AsRef<Path>) -> Result<IndexReport, CodeGraphError> {
    let workspace_path = canonical_workspace_path(workspace_path.as_ref())?;
    let files = discover_workspace_files(&workspace_path)?;
    let mut database = WorkspaceDatabase::open_or_create(&workspace_path)?;
    let mut report = IndexReport::default();
    let mut live_paths = Vec::new();

    for file_path in files {
        report.scanned_files += 1;
        match index_workspace_file(&workspace_path, &file_path, &mut database)? {
            FileIndexOutcome::Indexed {
                relative_path,
                had_parse_error,
            } => {
                report.indexed_files += 1;
                if had_parse_error {
                    report.parse_errors += 1;
                }
                live_paths.push(relative_path);
            }
            FileIndexOutcome::Unchanged { relative_path } => {
                report.unchanged_files += 1;
                live_paths.push(relative_path);
            }
            FileIndexOutcome::Skipped => {
                report.skipped_files += 1;
            }
        }
    }

    report.deleted_files = database.remove_stale_code_graph_files(&live_paths)?.len();

    Ok(report)
}

pub fn start_code_graph_watcher(
    workspace_path: impl AsRef<Path>,
) -> Result<CodeGraphWatcher, CodeGraphError> {
    start_code_graph_watcher_with_debounce(workspace_path, DEFAULT_WATCH_DEBOUNCE)
}

pub fn start_code_graph_watcher_with_debounce(
    workspace_path: impl AsRef<Path>,
    debounce: Duration,
) -> Result<CodeGraphWatcher, CodeGraphError> {
    let workspace_path = canonical_workspace_path(workspace_path.as_ref())?;
    let (event_tx, event_rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = event_tx.send(event);
    })?;
    watcher.watch(&workspace_path, RecursiveMode::Recursive)?;

    let (stop_tx, stop_rx) = mpsc::channel();
    let worker_workspace_path = workspace_path.clone();
    let handle = thread::spawn(move || {
        let _watcher = watcher;
        let mut pending = false;
        let mut next_index_at = Instant::now();

        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }

            match event_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(event)) => {
                    if event
                        .paths
                        .iter()
                        .any(|path| should_consider_watch_path(&worker_workspace_path, path))
                    {
                        pending = true;
                        next_index_at = Instant::now() + debounce;
                    }
                }
                Ok(Err(error)) => {
                    tracing::warn!(workspace = %worker_workspace_path.display(), error = %error, "code graph watcher event failed");
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }

            if pending && Instant::now() >= next_index_at {
                match index_workspace(&worker_workspace_path) {
                    Ok(report) => {
                        tracing::info!(
                            workspace = %worker_workspace_path.display(),
                            indexed_files = report.indexed_files,
                            unchanged_files = report.unchanged_files,
                            deleted_files = report.deleted_files,
                            parse_errors = report.parse_errors,
                            "code graph watcher refreshed workspace index"
                        );
                    }
                    Err(error) => {
                        tracing::error!(workspace = %worker_workspace_path.display(), error = %error, "code graph watcher refresh failed");
                    }
                }
                pending = false;
            }
        }
    });

    Ok(CodeGraphWatcher {
        workspace_path,
        stop_tx: Some(stop_tx),
        handle: Some(handle),
    })
}

pub struct CodeGraphWatcher {
    workspace_path: PathBuf,
    stop_tx: Option<mpsc::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl CodeGraphWatcher {
    pub fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }
}

impl Drop for CodeGraphWatcher {
    fn drop(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            if let Err(error) = handle.join() {
                tracing::warn!(?error, "code graph watcher thread join failed");
            }
        }
    }
}

#[derive(Debug)]
pub enum CodeGraphError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Notify(notify::Error),
    Store(WorkspaceDatabaseError),
    TreeSitterLanguage {
        language: &'static str,
        source: tree_sitter::LanguageError,
    },
    TreeSitterParse {
        path: PathBuf,
        language: &'static str,
    },
    WorkspaceRelativePath {
        workspace: PathBuf,
        path: PathBuf,
    },
}

impl fmt::Display for CodeGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {}", path.display(), source),
            Self::Notify(source) => write!(formatter, "filesystem watcher error: {source}"),
            Self::Store(source) => write!(formatter, "{source}"),
            Self::TreeSitterLanguage { language, source } => {
                write!(
                    formatter,
                    "failed to load Tree-sitter language {language}: {source}"
                )
            }
            Self::TreeSitterParse { path, language } => write!(
                formatter,
                "Tree-sitter parser returned no tree for {} as {language}",
                path.display()
            ),
            Self::WorkspaceRelativePath { workspace, path } => write!(
                formatter,
                "path {} is not inside workspace {}",
                path.display(),
                workspace.display()
            ),
        }
    }
}

impl std::error::Error for CodeGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Notify(source) => Some(source),
            Self::Store(source) => Some(source),
            Self::TreeSitterLanguage { source, .. } => Some(source),
            Self::TreeSitterParse { .. } | Self::WorkspaceRelativePath { .. } => None,
        }
    }
}

impl From<WorkspaceDatabaseError> for CodeGraphError {
    fn from(source: WorkspaceDatabaseError) -> Self {
        Self::Store(source)
    }
}

impl From<notify::Error> for CodeGraphError {
    fn from(source: notify::Error) -> Self {
        Self::Notify(source)
    }
}

enum FileIndexOutcome {
    Indexed {
        relative_path: String,
        had_parse_error: bool,
    },
    Unchanged {
        relative_path: String,
    },
    Skipped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LanguageKind {
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
    C,
    Cpp,
    CSharp,
    Java,
    Json,
    Toml,
    Markdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExtractedFile {
    parse_status: &'static str,
    parse_error_message: Option<String>,
    symbols: Vec<ExtractedSymbol>,
    imports: Vec<ExtractedImport>,
    references: Vec<ExtractedReference>,
    edges: Vec<ExtractedEdge>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExtractedSymbol {
    name: String,
    kind: &'static str,
    start: Point,
    end: Point,
    name_start: Point,
    name_end: Point,
    signature: Option<String>,
    documentation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExtractedImport {
    module: String,
    imported_symbol: Option<String>,
    alias: Option<String>,
    start: Point,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExtractedReference {
    name: String,
    symbol_index: Option<usize>,
    start: Point,
    end: Point,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ExtractedEdge {
    source_symbol_index: usize,
    target_symbol_index: usize,
    edge_kind: &'static str,
}

fn index_workspace_file(
    workspace_path: &Path,
    file_path: &Path,
    database: &mut WorkspaceDatabase,
) -> Result<FileIndexOutcome, CodeGraphError> {
    let bytes = fs::read(file_path).map_err(|source| io_error(file_path, source))?;
    let text = std::str::from_utf8(&bytes);
    let language = detect_language(file_path, text.ok());
    let Some(language) = language else {
        return Ok(FileIndexOutcome::Skipped);
    };

    let relative_path = workspace_relative_path(workspace_path, file_path)?;
    let content_hash = content_hash(&bytes);
    if database.code_graph_file_hash(&relative_path)? == Some(content_hash.clone()) {
        return Ok(FileIndexOutcome::Unchanged { relative_path });
    }

    let metadata = fs::metadata(file_path).map_err(|source| io_error(file_path, source))?;
    let modified_at = metadata.modified().ok().map(system_time_to_timestamp);
    let extracted = match text {
        Ok(text) => extract_file(language, text, file_path)?,
        Err(_) => ExtractedFile {
            parse_status: "error",
            parse_error_message: Some("file is not valid UTF-8".to_string()),
            symbols: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            edges: Vec::new(),
        },
    };
    let had_parse_error = extracted.parse_status == "error";
    let size_bytes = i64::try_from(bytes.len()).ok();
    let fts_body = text.unwrap_or_default();
    let symbol_rows = extracted
        .symbols
        .iter()
        .map(|symbol| NewCodeGraphSymbol {
            name: &symbol.name,
            kind: symbol.kind,
            start_line: Some(point_row(symbol.start)),
            start_column: Some(point_column(symbol.start)),
            end_line: Some(point_row(symbol.end)),
            end_column: Some(point_column(symbol.end)),
            signature: symbol.signature.as_deref(),
            documentation: symbol.documentation.as_deref(),
        })
        .collect::<Vec<_>>();
    let import_rows = extracted
        .imports
        .iter()
        .map(|import| NewCodeGraphImport {
            module: &import.module,
            imported_symbol: import.imported_symbol.as_deref(),
            alias: import.alias.as_deref(),
            start_line: Some(point_row(import.start)),
            start_column: Some(point_column(import.start)),
        })
        .collect::<Vec<_>>();
    let reference_rows = extracted
        .references
        .iter()
        .map(|reference| NewCodeGraphReference {
            name: &reference.name,
            symbol_index: reference.symbol_index,
            start_line: Some(point_row(reference.start)),
            start_column: Some(point_column(reference.start)),
            end_line: Some(point_row(reference.end)),
            end_column: Some(point_column(reference.end)),
        })
        .collect::<Vec<_>>();
    let edge_rows = extracted
        .edges
        .iter()
        .map(|edge| NewCodeGraphEdge {
            source_symbol_index: edge.source_symbol_index,
            target_symbol_index: edge.target_symbol_index,
            edge_kind: edge.edge_kind,
            metadata_json: None,
        })
        .collect::<Vec<_>>();

    database.replace_code_graph_file_index(NewCodeGraphFileIndex {
        path: &relative_path,
        language: Some(language.name()),
        size_bytes,
        modified_at: modified_at.as_deref(),
        content_hash: &content_hash,
        parse_status: extracted.parse_status,
        parse_error_message: extracted.parse_error_message.as_deref(),
        symbols: &symbol_rows,
        imports: &import_rows,
        references: &reference_rows,
        edges: &edge_rows,
        fts_body,
    })?;

    Ok(FileIndexOutcome::Indexed {
        relative_path,
        had_parse_error,
    })
}

fn extract_file(
    language: LanguageKind,
    text: &str,
    file_path: &Path,
) -> Result<ExtractedFile, CodeGraphError> {
    let Some(tree_sitter_language) = language.tree_sitter_language() else {
        return Ok(ExtractedFile {
            parse_status: "skipped",
            parse_error_message: None,
            symbols: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            edges: Vec::new(),
        });
    };
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_language)
        .map_err(|source| CodeGraphError::TreeSitterLanguage {
            language: language.name(),
            source,
        })?;
    let tree = parser
        .parse(text, None)
        .ok_or_else(|| CodeGraphError::TreeSitterParse {
            path: file_path.to_path_buf(),
            language: language.name(),
        })?;
    let root = tree.root_node();

    if root.has_error() {
        return Ok(ExtractedFile {
            parse_status: "error",
            parse_error_message: Some("Tree-sitter parse contains ERROR nodes".to_string()),
            symbols: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            edges: Vec::new(),
        });
    }

    let lines = text.lines().collect::<Vec<_>>();
    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    collect_symbols_and_imports(language, root, text, &lines, &mut symbols, &mut imports);
    let (references, edges) = collect_references(root, text, &symbols);

    Ok(ExtractedFile {
        parse_status: "parsed",
        parse_error_message: None,
        symbols,
        imports,
        references,
        edges,
    })
}

fn collect_symbols_and_imports(
    language: LanguageKind,
    node: Node<'_>,
    text: &str,
    lines: &[&str],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    if let Some(symbol_kind) = classify_symbol(language, node) {
        if let Some((name, name_start, name_end)) = symbol_name(language, node, text) {
            symbols.push(ExtractedSymbol {
                name,
                kind: symbol_kind,
                start: node.start_position(),
                end: node.end_position(),
                name_start,
                name_end,
                signature: signature_text(node, text),
                documentation: preceding_documentation(node, lines),
            });
        }
    }

    if is_import_node(language, node) {
        if let Some(module) = import_module(node, text) {
            imports.push(ExtractedImport {
                module,
                imported_symbol: None,
                alias: None,
                start: node.start_position(),
            });
        }
    }

    for index in 0..node.child_count() {
        if let Some(child) = child_at(node, index) {
            collect_symbols_and_imports(language, child, text, lines, symbols, imports);
        }
    }
}

fn collect_references(
    root: Node<'_>,
    text: &str,
    symbols: &[ExtractedSymbol],
) -> (Vec<ExtractedReference>, Vec<ExtractedEdge>) {
    let symbol_names = symbols
        .iter()
        .enumerate()
        .map(|(index, symbol)| (symbol.name.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut references = Vec::new();
    let mut edges = HashSet::new();

    collect_references_recursive(
        root,
        text,
        symbols,
        &symbol_names,
        &mut references,
        &mut edges,
    );

    (
        references,
        edges.into_iter().collect::<Vec<ExtractedEdge>>(),
    )
}

fn collect_references_recursive(
    node: Node<'_>,
    text: &str,
    symbols: &[ExtractedSymbol],
    symbol_names: &HashMap<&str, usize>,
    references: &mut Vec<ExtractedReference>,
    edges: &mut HashSet<ExtractedEdge>,
) {
    if is_identifier_node(node) {
        if let Some(name) = node_text(node, text) {
            if let Some(target_symbol_index) = symbol_names.get(name.as_str()).copied() {
                let target_symbol = &symbols[target_symbol_index];

                if node.start_position() != target_symbol.name_start
                    || node.end_position() != target_symbol.name_end
                {
                    let source_symbol_index =
                        containing_symbol_index(symbols, node.start_position());
                    references.push(ExtractedReference {
                        name,
                        symbol_index: Some(target_symbol_index),
                        start: node.start_position(),
                        end: node.end_position(),
                    });
                    if let Some(source_symbol_index) = source_symbol_index {
                        if source_symbol_index != target_symbol_index {
                            edges.insert(ExtractedEdge {
                                source_symbol_index,
                                target_symbol_index,
                                edge_kind: "references",
                            });
                        }
                    }
                }
            }
        }
    }

    for index in 0..node.child_count() {
        if let Some(child) = child_at(node, index) {
            collect_references_recursive(child, text, symbols, symbol_names, references, edges);
        }
    }
}

fn classify_symbol(language: LanguageKind, node: Node<'_>) -> Option<&'static str> {
    let kind = node.kind();

    match language {
        LanguageKind::Rust => match kind {
            "function_item" if has_ancestor_kind(node, "impl_item") => Some("method"),
            "function_item" => Some("function"),
            "struct_item" => Some("struct"),
            "enum_item" => Some("enum"),
            "trait_item" => Some("trait"),
            "const_item" | "static_item" | "let_declaration" => Some("variable"),
            "type_item" => Some("type_alias"),
            "impl_item" => Some("impl"),
            _ => None,
        },
        LanguageKind::TypeScript | LanguageKind::Tsx => match kind {
            "function_declaration" => Some("function"),
            "method_definition" | "method_signature" => Some("method"),
            "class_declaration" => Some("class"),
            "interface_declaration" => Some("trait"),
            "enum_declaration" => Some("enum"),
            "type_alias_declaration" => Some("type_alias"),
            "variable_declarator" => Some("variable"),
            _ => None,
        },
        LanguageKind::JavaScript => match kind {
            "function_declaration" | "generator_function_declaration" => Some("function"),
            "method_definition" => Some("method"),
            "class_declaration" => Some("class"),
            "variable_declarator" => Some("variable"),
            _ => None,
        },
        LanguageKind::Python => match kind {
            "function_definition" => Some("function"),
            "class_definition" => Some("class"),
            "assignment" => Some("variable"),
            _ => None,
        },
        LanguageKind::Go => match kind {
            "function_declaration" => Some("function"),
            "method_declaration" => Some("method"),
            "type_spec" => Some("type_alias"),
            "var_spec" | "const_spec" => Some("variable"),
            _ => None,
        },
        LanguageKind::C => match kind {
            "function_definition" => Some("function"),
            "struct_specifier" => Some("struct"),
            "enum_specifier" => Some("enum"),
            _ => None,
        },
        LanguageKind::Cpp => match kind {
            "function_definition" => Some("function"),
            "class_specifier" => Some("class"),
            "struct_specifier" => Some("struct"),
            "enum_specifier" => Some("enum"),
            _ => None,
        },
        LanguageKind::CSharp => match kind {
            "method_declaration" => Some("method"),
            "constructor_declaration" => Some("method"),
            "class_declaration" => Some("class"),
            "struct_declaration" => Some("struct"),
            "enum_declaration" => Some("enum"),
            "interface_declaration" => Some("trait"),
            "field_declaration" | "property_declaration" | "variable_declaration" => {
                Some("variable")
            }
            _ => None,
        },
        LanguageKind::Java => match kind {
            "method_declaration" | "constructor_declaration" => Some("method"),
            "class_declaration" => Some("class"),
            "enum_declaration" => Some("enum"),
            "interface_declaration" => Some("trait"),
            "field_declaration" | "variable_declarator" => Some("variable"),
            _ => None,
        },
        LanguageKind::Json | LanguageKind::Toml | LanguageKind::Markdown => None,
    }
}

fn symbol_name(
    language: LanguageKind,
    node: Node<'_>,
    text: &str,
) -> Option<(String, Point, Point)> {
    if let Some(name_node) = node.child_by_field_name("name") {
        return clean_identifier_node(name_node, text);
    }

    for field_name in ["pattern", "left", "declarator"] {
        if let Some(child) = node.child_by_field_name(field_name) {
            if let Some(identifier) = first_identifier(child) {
                return clean_identifier_node(identifier, text);
            }
        }
    }

    if language == LanguageKind::Rust && node.kind() == "impl_item" {
        if let Some(type_node) = node.child_by_field_name("type") {
            if let Some(identifier) = first_identifier(type_node) {
                return clean_identifier_node(identifier, text);
            }
        }
    }

    first_identifier(node).and_then(|identifier| clean_identifier_node(identifier, text))
}

fn is_import_node(language: LanguageKind, node: Node<'_>) -> bool {
    matches!(
        (language, node.kind()),
        (LanguageKind::Rust, "use_declaration")
            | (
                LanguageKind::TypeScript | LanguageKind::Tsx,
                "import_statement"
            )
            | (LanguageKind::JavaScript, "import_statement")
            | (
                LanguageKind::Python,
                "import_statement" | "import_from_statement"
            )
            | (LanguageKind::Go, "import_declaration" | "import_spec")
            | (LanguageKind::C | LanguageKind::Cpp, "preproc_include")
            | (LanguageKind::CSharp, "using_directive")
            | (LanguageKind::Java, "import_declaration")
    )
}

fn import_module(node: Node<'_>, text: &str) -> Option<String> {
    if let Some(string_node) = first_node_of_kinds(
        node,
        &[
            "string",
            "string_literal",
            "interpreted_string_literal",
            "raw_string_literal",
            "system_lib_string",
        ],
    ) {
        return node_text(string_node, text).map(clean_module_text);
    }

    node_text(node, text).map(|value| {
        value
            .trim()
            .trim_start_matches("use ")
            .trim_start_matches("import ")
            .trim_start_matches("from ")
            .trim_end_matches(';')
            .trim()
            .chars()
            .take(MAX_SIGNATURE_CHARS)
            .collect()
    })
}

fn first_identifier(node: Node<'_>) -> Option<Node<'_>> {
    first_node_of_kinds(
        node,
        &[
            "identifier",
            "type_identifier",
            "field_identifier",
            "property_identifier",
            "shorthand_property_identifier",
        ],
    )
}

fn first_node_of_kinds<'tree>(node: Node<'tree>, kinds: &[&str]) -> Option<Node<'tree>> {
    if kinds.contains(&node.kind()) {
        return Some(node);
    }

    for index in 0..node.child_count() {
        if let Some(child) = child_at(node, index) {
            if let Some(found) = first_node_of_kinds(child, kinds) {
                return Some(found);
            }
        }
    }

    None
}

fn child_at(node: Node<'_>, index: usize) -> Option<Node<'_>> {
    let index = u32::try_from(index).ok()?;

    node.child(index)
}

fn is_identifier_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "identifier"
            | "type_identifier"
            | "field_identifier"
            | "property_identifier"
            | "shorthand_property_identifier"
    )
}

fn has_ancestor_kind(node: Node<'_>, kind: &str) -> bool {
    let mut current = node.parent();

    while let Some(node) = current {
        if node.kind() == kind {
            return true;
        }
        current = node.parent();
    }

    false
}

fn clean_identifier_node(node: Node<'_>, text: &str) -> Option<(String, Point, Point)> {
    let value = node_text(node, text)?;
    let value = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('`')
        .to_string();

    if value.is_empty() || value.chars().any(char::is_whitespace) {
        None
    } else {
        Some((value, node.start_position(), node.end_position()))
    }
}

fn node_text(node: Node<'_>, text: &str) -> Option<String> {
    node.utf8_text(text.as_bytes())
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn signature_text(node: Node<'_>, text: &str) -> Option<String> {
    node_text(node, text).map(|value| {
        value
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .chars()
            .take(MAX_SIGNATURE_CHARS)
            .collect()
    })
}

fn preceding_documentation(node: Node<'_>, lines: &[&str]) -> Option<String> {
    let mut row = node.start_position().row;
    let mut docs = Vec::new();

    while row > 0 {
        row -= 1;
        let line = lines.get(row)?.trim();

        if line.is_empty() {
            if docs.is_empty() {
                continue;
            }
            break;
        }

        if is_comment_line(line) {
            docs.push(clean_comment_line(line));
        } else {
            break;
        }
    }

    docs.reverse();
    let documentation = docs
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if documentation.is_empty() {
        None
    } else {
        Some(documentation)
    }
}

fn is_comment_line(line: &str) -> bool {
    line.starts_with("//")
        || line.starts_with("///")
        || line.starts_with("//!")
        || line.starts_with('#')
        || line.starts_with('*')
        || line.starts_with("/*")
        || line.starts_with("/**")
}

fn clean_comment_line(line: &str) -> String {
    line.trim()
        .trim_start_matches("///")
        .trim_start_matches("//!")
        .trim_start_matches("//")
        .trim_start_matches("/**")
        .trim_start_matches("/*")
        .trim_start_matches('*')
        .trim_end_matches("*/")
        .trim()
        .to_string()
}

fn containing_symbol_index(symbols: &[ExtractedSymbol], point: Point) -> Option<usize> {
    symbols
        .iter()
        .enumerate()
        .filter(|(_, symbol)| point_in_range(point, symbol.start, symbol.end))
        .min_by_key(|(_, symbol)| point_span(symbol.start, symbol.end))
        .map(|(index, _)| index)
}

fn point_in_range(point: Point, start: Point, end: Point) -> bool {
    point_compare(point, start) >= 0 && point_compare(point, end) <= 0
}

fn point_span(start: Point, end: Point) -> usize {
    end.row.saturating_sub(start.row) * 100_000 + end.column.saturating_sub(start.column)
}

fn point_compare(left: Point, right: Point) -> i8 {
    match left.row.cmp(&right.row) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Equal => match left.column.cmp(&right.column) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => 0,
        },
    }
}

fn detect_language(file_path: &Path, text: Option<&str>) -> Option<LanguageKind> {
    detect_language_by_extension(file_path).or_else(|| detect_language_by_content(text?))
}

fn detect_language_by_extension(file_path: &Path) -> Option<LanguageKind> {
    let extension = file_path
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();

    match extension.as_str() {
        "rs" => Some(LanguageKind::Rust),
        "ts" | "mts" | "cts" | "ets" => Some(LanguageKind::TypeScript),
        "tsx" => Some(LanguageKind::Tsx),
        "js" | "mjs" | "cjs" | "jsx" => Some(LanguageKind::JavaScript),
        "py" | "pyw" => Some(LanguageKind::Python),
        "go" => Some(LanguageKind::Go),
        "c" => Some(LanguageKind::C),
        "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => Some(LanguageKind::Cpp),
        "cs" => Some(LanguageKind::CSharp),
        "java" => Some(LanguageKind::Java),
        "json" => Some(LanguageKind::Json),
        "toml" => Some(LanguageKind::Toml),
        "md" | "markdown" => Some(LanguageKind::Markdown),
        _ => None,
    }
}

fn detect_language_by_content(text: &str) -> Option<LanguageKind> {
    let first_line = text.lines().next()?.trim().to_ascii_lowercase();

    if !first_line.starts_with("#!") {
        return None;
    }

    if first_line.contains("python") {
        Some(LanguageKind::Python)
    } else if first_line.contains("node") || first_line.contains("deno") {
        Some(LanguageKind::JavaScript)
    } else {
        None
    }
}

impl LanguageKind {
    fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Java => "java",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Markdown => "markdown",
        }
    }

    fn tree_sitter_language(self) -> Option<Language> {
        match self {
            Self::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
            Self::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Self::Tsx => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            Self::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
            Self::Python => Some(tree_sitter_python::LANGUAGE.into()),
            Self::Go => Some(tree_sitter_go::LANGUAGE.into()),
            Self::C => Some(tree_sitter_c::LANGUAGE.into()),
            Self::Cpp => Some(tree_sitter_cpp::LANGUAGE.into()),
            Self::CSharp => Some(tree_sitter_c_sharp::LANGUAGE.into()),
            Self::Java => Some(tree_sitter_java::LANGUAGE.into()),
            Self::Json => Some(tree_sitter_json::LANGUAGE.into()),
            Self::Toml => Some(tree_sitter_toml_ng::LANGUAGE.into()),
            Self::Markdown => None,
        }
    }
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

fn should_consider_watch_path(workspace_path: &Path, path: &Path) -> bool {
    path.starts_with(workspace_path) && !is_internal_path(workspace_path, path)
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

fn io_error(path: &Path, source: io::Error) -> CodeGraphError {
    CodeGraphError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn content_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn system_time_to_timestamp(value: SystemTime) -> String {
    let value: DateTime<Utc> = value.into();
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn point_row(point: Point) -> i64 {
    i64::try_from(point.row).unwrap_or(i64::MAX)
}

fn point_column(point: Point) -> i64 {
    i64::try_from(point.column).unwrap_or(i64::MAX)
}

fn clean_module_text(value: String) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('`')
        .trim_matches('<')
        .trim_matches('>')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn indexes_workspace_incrementally_and_populates_graph_tables() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(
            workspace.path().join(".gitignore"),
            "ignored.rs\nnode_modules/\n",
        )
        .expect("gitignore");
        fs::write(
            workspace.path().join("lib.rs"),
            r#"
use std::fs;

/// A demo struct.
struct Thing;

fn helper() -> Thing {
    Thing
}

fn caller() {
    let value = helper();
}
"#,
        )
        .expect("source");
        fs::write(workspace.path().join("ignored.rs"), "fn ignored() {}\n").expect("ignored");

        let report = index_workspace(workspace.path()).expect("initial index");

        assert_eq!(report.indexed_files, 1);
        assert_eq!(report.unchanged_files, 0);
        assert_eq!(report.deleted_files, 0);
        assert_eq!(report.parse_errors, 0);

        let database_path = workspace.path().join(".foco").join("foco.sqlite");
        let connection = Connection::open(database_path).expect("open graph database");
        assert_eq!(
            query_count(&connection, "SELECT COUNT(*) FROM code_graph_files"),
            1
        );
        assert_eq!(
            query_count(
                &connection,
                "SELECT COUNT(*) FROM code_graph_files WHERE path = 'lib.rs'"
            ),
            1
        );
        assert_eq!(
            query_count(
                &connection,
                "SELECT COUNT(*) FROM code_graph_files WHERE path = 'ignored.rs'"
            ),
            0
        );
        assert_eq!(
            query_count(
                &connection,
                "SELECT COUNT(*) FROM code_graph_symbols WHERE name = 'helper'"
            ),
            1
        );
        assert_eq!(
            query_count(
                &connection,
                "SELECT COUNT(*) FROM code_graph_imports WHERE module LIKE '%std::fs%'"
            ),
            1
        );
        assert!(
            query_count(
                &connection,
                "SELECT COUNT(*) FROM code_graph_references WHERE name = 'helper'"
            ) >= 1
        );
        assert!(query_count(&connection, "SELECT COUNT(*) FROM code_graph_edges") >= 1);
        assert!(
            query_count(
                &connection,
                "SELECT COUNT(*) FROM code_graph_fts_index WHERE code_graph_fts_index MATCH 'helper'"
            ) >= 1
        );

        let unchanged = index_workspace(workspace.path()).expect("unchanged index");
        assert_eq!(unchanged.indexed_files, 0);
        assert_eq!(unchanged.unchanged_files, 1);

        fs::write(
            workspace.path().join("lib.rs"),
            r#"
use std::path::PathBuf;

struct Thing;

fn helper() -> Thing {
    Thing
}

fn caller() {
    let value = helper();
}
"#,
        )
        .expect("modified source");
        let changed = index_workspace(workspace.path()).expect("changed index");
        assert_eq!(changed.indexed_files, 1);
        assert_eq!(changed.unchanged_files, 0);
    }

    #[test]
    fn removes_stale_graph_rows_for_deleted_files() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source_path = workspace.path().join("lib.rs");
        fs::write(&source_path, "fn helper() {}\n").expect("source");
        index_workspace(workspace.path()).expect("initial index");

        fs::remove_file(&source_path).expect("remove source");
        let report = index_workspace(workspace.path()).expect("index after delete");

        assert_eq!(report.deleted_files, 1);
        let database_path = workspace.path().join(".foco").join("foco.sqlite");
        let connection = Connection::open(database_path).expect("open graph database");
        assert_eq!(
            query_count(&connection, "SELECT COUNT(*) FROM code_graph_files"),
            0
        );
        assert_eq!(
            query_count(&connection, "SELECT COUNT(*) FROM code_graph_symbols"),
            0
        );
    }

    #[test]
    fn indexes_ets_files_as_typescript() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(
            workspace.path().join("Widget.ets"),
            r#"
export function buildTitle(value: string): string {
    return value.trim();
}
"#,
        )
        .expect("ets source");

        let report = index_workspace(workspace.path()).expect("index ets");

        assert_eq!(report.indexed_files, 1);
        assert_eq!(report.skipped_files, 0);
        assert_eq!(report.parse_errors, 0);

        let database_path = workspace.path().join(".foco").join("foco.sqlite");
        let connection = Connection::open(database_path).expect("open graph database");
        assert_eq!(
            query_count(
                &connection,
                "SELECT COUNT(*) FROM code_graph_files WHERE path = 'Widget.ets'"
            ),
            1
        );
        assert_eq!(
            query_count(
                &connection,
                "SELECT COUNT(*) FROM code_graph_symbols WHERE name = 'buildTitle'"
            ),
            1
        );
    }

    fn query_count(connection: &Connection, sql: &str) -> i64 {
        connection
            .query_row(sql, [], |row| row.get(0))
            .expect("query count")
    }
}
