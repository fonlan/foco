use std::collections::HashSet;

use tree_sitter::{Node, Parser};

use crate::CodeGraphError;

use super::{
    ExtractionContext, GraphExtractor,
    ast::{
        child_at, first_identifier, last_identifier, node_text, point_in_range, point_span,
        preceding_documentation, range_from_node, signature_text,
    },
    facts::{
        ExtractedEdge, ExtractedEdgeTarget, ExtractedGraphFile, ExtractedImport, ExtractedNode,
        ExtractedPosition, ExtractedRange, ExtractedReference, file_local_key, node_local_key,
    },
};

const TREE_SITTER_EXACT: &str = r#"{"semanticVersion":1,"provenance":"tree_sitter","confidence":"exact","resolution":{"status":"resolved","candidates":[]}}"#;
const HEURISTIC_RESOLUTION: &str = r#"{"semanticVersion":1,"provenance":"heuristic","confidence":"heuristic","resolution":{"status":"resolved","candidates":[]}}"#;

pub(crate) struct RustExtractor;

impl GraphExtractor for RustExtractor {
    fn extract(
        &self,
        context: ExtractionContext<'_>,
    ) -> Result<ExtractedGraphFile, CodeGraphError> {
        extract_rust(context)
    }
}

#[derive(Clone)]
struct Scope {
    qualified_name: String,
    range: ExtractedRange,
    depth: usize,
    containing_symbol: Option<String>,
    is_impl_or_trait: bool,
}

struct Declaration {
    node: ExtractedNode,
    declaration_scope: ExtractedRange,
    visible_from: ExtractedPosition,
    scope_depth: usize,
    containing_symbol: Option<String>,
    callable: bool,
}

fn extract_rust(context: ExtractionContext<'_>) -> Result<ExtractedGraphFile, CodeGraphError> {
    let file_key = file_local_key(context.relative_path);
    let mut parser = Parser::new();
    let language =
        context
            .language
            .tree_sitter_language()
            .ok_or_else(|| CodeGraphError::TreeSitterParse {
                path: context.file_path.to_path_buf(),
                language: context.language.name(),
            })?;
    parser
        .set_language(&language)
        .map_err(|source| CodeGraphError::TreeSitterLanguage {
            language: context.language.name(),
            source,
        })?;
    let tree = parser
        .parse(context.text, None)
        .ok_or_else(|| CodeGraphError::TreeSitterParse {
            path: context.file_path.to_path_buf(),
            language: context.language.name(),
        })?;
    let root = tree.root_node();

    if root.has_error() {
        return Ok(empty_file(
            file_key,
            "error",
            Some("Tree-sitter parse contains ERROR nodes".to_string()),
        ));
    }

    let lines = context.text.lines().collect::<Vec<_>>();
    let root_scope = Scope {
        qualified_name: String::new(),
        range: range_from_node(root),
        depth: 0,
        containing_symbol: None,
        is_impl_or_trait: false,
    };
    let mut declarations = Vec::new();
    let mut imports = Vec::new();
    collect_declarations(
        root,
        context.text,
        &lines,
        &file_key,
        &root_scope,
        &mut declarations,
        &mut imports,
    );

    let (references, call_edges) = collect_calls(root, context.text, &declarations);
    let contains_edges = declarations.iter().filter_map(|declaration| {
        declaration
            .containing_symbol
            .as_ref()
            .filter(|parent| **parent != declaration.node.local_key)
            .map(|parent| ExtractedEdge {
                source_local_key: parent.clone(),
                target: ExtractedEdgeTarget::Local(declaration.node.local_key.clone()),
                edge_kind: "contains",
                metadata_json: TREE_SITTER_EXACT.to_string(),
            })
    });
    let mut edges = call_edges;
    edges.extend(contains_edges);

    Ok(ExtractedGraphFile {
        local_key: file_key,
        parse_status: "parsed",
        parse_error_message: None,
        nodes: declarations
            .into_iter()
            .map(|declaration| declaration.node)
            .collect(),
        imports,
        references,
        edges: edges.into_iter().collect(),
    })
}

fn empty_file(
    local_key: String,
    parse_status: &'static str,
    parse_error_message: Option<String>,
) -> ExtractedGraphFile {
    ExtractedGraphFile {
        local_key,
        parse_status,
        parse_error_message,
        nodes: Vec::new(),
        imports: Vec::new(),
        references: Vec::new(),
        edges: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_declarations(
    node: Node<'_>,
    text: &str,
    lines: &[&str],
    file_key: &str,
    scope: &Scope,
    declarations: &mut Vec<Declaration>,
    imports: &mut Vec<ExtractedImport>,
) {
    if node.kind() == "use_declaration" {
        collect_use_import(node, text, imports);
    }

    let declaration = rust_declaration(node, text, lines, file_key, scope);
    let child_scope = declaration.as_ref().map_or_else(
        || child_scope_for_non_symbol(node, scope),
        |declaration| Scope {
            qualified_name: declaration.node.qualified_name.clone(),
            range: declaration.node.range,
            depth: scope.depth.saturating_add(1),
            containing_symbol: Some(declaration.node.local_key.clone()),
            is_impl_or_trait: matches!(declaration.node.kind, "impl" | "trait"),
        },
    );

    if let Some(declaration) = declaration {
        declarations.push(declaration);
    }

    for index in 0..node.child_count() {
        if let Some(child) = child_at(node, index) {
            collect_declarations(
                child,
                text,
                lines,
                file_key,
                &child_scope,
                declarations,
                imports,
            );
        }
    }
}

fn child_scope_for_non_symbol(node: Node<'_>, scope: &Scope) -> Scope {
    if matches!(node.kind(), "block" | "closure_expression") {
        Scope {
            qualified_name: scope.qualified_name.clone(),
            range: range_from_node(node),
            depth: scope.depth.saturating_add(1),
            containing_symbol: scope.containing_symbol.clone(),
            is_impl_or_trait: scope.is_impl_or_trait,
        }
    } else {
        scope.clone()
    }
}

fn rust_declaration(
    node: Node<'_>,
    text: &str,
    lines: &[&str],
    file_key: &str,
    scope: &Scope,
) -> Option<Declaration> {
    let (kind, name_node, callable) = match node.kind() {
        "function_item" if scope.is_impl_or_trait => {
            ("method", node.child_by_field_name("name")?, true)
        }
        "function_item" => ("function", node.child_by_field_name("name")?, true),
        "struct_item" => ("struct", node.child_by_field_name("name")?, false),
        "enum_item" => ("enum", node.child_by_field_name("name")?, false),
        "trait_item" => ("trait", node.child_by_field_name("name")?, false),
        "mod_item" => ("module", node.child_by_field_name("name")?, false),
        "type_item" => ("type_alias", node.child_by_field_name("name")?, false),
        "const_item" | "static_item" => ("variable", node.child_by_field_name("name")?, false),
        "let_declaration" | "parameter" | "self_parameter" => {
            ("variable", first_identifier(node)?, false)
        }
        "impl_item" => ("impl", node.child_by_field_name("type")?, false),
        _ => return None,
    };
    let name = node_text(name_node, text)?;
    let qualified_name = if scope.qualified_name.is_empty() {
        name.clone()
    } else {
        format!("{}::{name}", scope.qualified_name)
    };
    let range = range_from_node(node);
    let visibility = rust_visibility(node, text);
    let metadata_json = if node.kind() == "function_item"
        && node_text(node, text).is_some_and(|source| {
            source.split_whitespace().any(|token| {
                token.trim_matches(|character: char| !character.is_alphanumeric()) == "async"
            })
        }) {
        r#"{"semanticVersion":1,"provenance":"tree_sitter","confidence":"exact","async":true}"#
            .to_string()
    } else {
        TREE_SITTER_EXACT.to_string()
    };

    Some(Declaration {
        node: ExtractedNode {
            local_key: node_local_key(file_key, kind, &qualified_name, range.start),
            name,
            qualified_name,
            kind,
            visibility,
            metadata_json,
            range,
            name_range: range_from_node(name_node),
            signature: signature_text(node, text),
            documentation: preceding_documentation(node, lines),
        },
        declaration_scope: scope.range,
        // Local bindings must not shadow calls that appear before their declaration.
        // Function items remain resolvable throughout their enclosing scope.
        visible_from: range.start,
        scope_depth: scope.depth,
        containing_symbol: scope.containing_symbol.clone(),
        callable,
    })
}

fn rust_visibility(node: Node<'_>, text: &str) -> Option<&'static str> {
    (0..node.child_count()).find_map(|index| {
        let child = child_at(node, index)?;
        (child.kind() == "visibility_modifier")
            .then(|| node_text(child, text))
            .flatten()
            .map(|_| "public")
    })
}

fn collect_use_import(node: Node<'_>, text: &str, imports: &mut Vec<ExtractedImport>) {
    collect_use_tree(node, text, "", range_from_node(node), imports);
}

fn collect_use_tree(
    node: Node<'_>,
    text: &str,
    prefix: &str,
    declaration_range: ExtractedRange,
    imports: &mut Vec<ExtractedImport>,
) {
    if let Some(path) = direct_glob_path(node, text) {
        record_use_import(
            &join_use_path(prefix, &format!("{path}::*")),
            None,
            declaration_range,
            imports,
        );
        return;
    }

    match node.kind() {
        "use_declaration" | "use_tree" | "use_list" => {
            for index in 0..node.child_count() {
                if let Some(child) = child_at(node, index)
                    && child.is_named()
                {
                    collect_use_tree(child, text, prefix, declaration_range, imports);
                }
            }
        }
        "scoped_use_list" => {
            let path = node
                .child_by_field_name("path")
                .and_then(|path| node_text(path, text));
            let Some(path) = path else {
                return;
            };
            let prefix = join_use_path(prefix, &path);
            for index in 0..node.child_count() {
                if let Some(child) = child_at(node, index)
                    && child.kind() == "use_list"
                {
                    collect_use_tree(child, text, &prefix, declaration_range, imports);
                }
            }
        }
        "use_as_clause" => {
            let Some(path) = node
                .child_by_field_name("path")
                .and_then(|path| node_text(path, text))
            else {
                return;
            };
            let alias = node
                .child_by_field_name("alias")
                .and_then(|alias| node_text(alias, text));
            record_use_import(
                &join_use_path(prefix, &path),
                alias.as_deref(),
                declaration_range,
                imports,
            );
        }
        "scoped_identifier" => {
            if let Some(path) = node_text(node, text) {
                record_use_import(
                    &join_use_path(prefix, &path),
                    None,
                    declaration_range,
                    imports,
                );
            }
        }
        "identifier" | "crate" | "self" | "super" | "wildcard" => {
            if let Some(segment) = node_text(node, text) {
                record_use_import(
                    &join_use_path(prefix, &segment),
                    None,
                    declaration_range,
                    imports,
                );
            }
        }
        _ => {
            // Future grammar variants may wrap a use tree. Keep walking only
            // named syntax nodes rather than splitting the declaration text.
            for index in 0..node.child_count() {
                if let Some(child) = child_at(node, index)
                    && child.is_named()
                {
                    collect_use_tree(child, text, prefix, declaration_range, imports);
                }
            }
        }
    }
}

fn direct_glob_path(node: Node<'_>, text: &str) -> Option<String> {
    let has_glob = (0..node.child_count())
        .any(|index| child_at(node, index).is_some_and(|child| child.kind() == "*"));
    has_glob.then(|| {
        (0..node.child_count()).find_map(|index| {
            let child = child_at(node, index)?;
            (child.is_named() && child.kind() != "visibility_modifier")
                .then(|| node_text(child, text))
                .flatten()
        })
    })?
}

fn join_use_path(prefix: &str, path: &str) -> String {
    if prefix.is_empty() {
        path.to_string()
    } else {
        format!("{prefix}::{path}")
    }
}

fn record_use_import(
    path: &str,
    alias: Option<&str>,
    range: ExtractedRange,
    imports: &mut Vec<ExtractedImport>,
) {
    let Some((module, imported_symbol)) = path.rsplit_once("::") else {
        return;
    };
    if module.is_empty() || imported_symbol.is_empty() {
        return;
    }
    imports.push(ExtractedImport {
        module: module.to_string(),
        imported_symbol: Some(imported_symbol.to_string()),
        alias: alias.filter(|value| !value.is_empty()).map(str::to_string),
        range,
    });
}

fn collect_calls(
    root: Node<'_>,
    text: &str,
    declarations: &[Declaration],
) -> (Vec<ExtractedReference>, HashSet<ExtractedEdge>) {
    let mut references = Vec::new();
    let mut edges = HashSet::new();
    collect_calls_recursive(root, text, declarations, &mut references, &mut edges);
    (references, edges)
}

fn collect_calls_recursive(
    node: Node<'_>,
    text: &str,
    declarations: &[Declaration],
    references: &mut Vec<ExtractedReference>,
    edges: &mut HashSet<ExtractedEdge>,
) {
    let (call_name, call_range, heuristic) = match node.kind() {
        "call_expression" => node
            .child_by_field_name("function")
            .and_then(|function| call_target_from_expression(function, text)),
        "method_call_expression" => node.child_by_field_name("method").and_then(|method| {
            node_text(method, text).map(|name| (name, range_from_node(method), true))
        }),
        _ => None,
    }
    .unwrap_or_else(|| (String::new(), range_from_node(node), false));

    if !call_name.is_empty() {
        let source = containing_callable(declarations, call_range.start);
        let target = resolve_visible_declaration(declarations, &call_name, call_range.start);
        if let Some(target) = target {
            references.push(ExtractedReference {
                name: call_name,
                target_local_key: Some(target.node.local_key.clone()),
                range: call_range,
            });
            if let Some(source) = source
                && source.node.local_key != target.node.local_key
                && target.callable
            {
                edges.insert(ExtractedEdge {
                    source_local_key: source.node.local_key.clone(),
                    target: ExtractedEdgeTarget::Local(target.node.local_key.clone()),
                    edge_kind: "calls",
                    metadata_json: if heuristic {
                        HEURISTIC_RESOLUTION.to_string()
                    } else {
                        TREE_SITTER_EXACT.to_string()
                    },
                });
            }
        } else {
            references.push(ExtractedReference {
                name: call_name,
                target_local_key: None,
                range: call_range,
            });
        }
    }

    for index in 0..node.child_count() {
        if let Some(child) = child_at(node, index) {
            collect_calls_recursive(child, text, declarations, references, edges);
        }
    }
}

fn call_target_from_expression(
    function: Node<'_>,
    text: &str,
) -> Option<(String, ExtractedRange, bool)> {
    match function.kind() {
        "identifier" => {
            node_text(function, text).map(|name| (name, range_from_node(function), false))
        }
        "scoped_identifier" | "generic_function" => {
            let name =
                last_identifier(function).and_then(|identifier| node_text(identifier, text))?;
            Some((name, range_from_node(function), true))
        }
        _ => None,
    }
}

fn resolve_visible_declaration<'a>(
    declarations: &'a [Declaration],
    name: &str,
    position: ExtractedPosition,
) -> Option<&'a Declaration> {
    declarations
        .iter()
        .filter(|declaration| {
            declaration.node.name == name
                && (declaration.node.kind != "variable"
                    || point_in_range(
                        declaration.visible_from,
                        declaration.declaration_scope.start,
                        position,
                    ))
                && point_in_range(
                    position,
                    declaration.declaration_scope.start,
                    declaration.declaration_scope.end,
                )
        })
        .max_by_key(|declaration| declaration.scope_depth)
}

fn containing_callable(
    declarations: &[Declaration],
    position: ExtractedPosition,
) -> Option<&Declaration> {
    declarations
        .iter()
        .filter(|declaration| {
            declaration.callable
                && point_in_range(
                    position,
                    declaration.node.range.start,
                    declaration.node.range.end,
                )
        })
        .min_by_key(|declaration| {
            point_span(declaration.node.range.start, declaration.node.range.end)
        })
}
