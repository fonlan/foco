use std::collections::HashSet;

use tree_sitter::{Node, Parser};

use crate::CodeGraphError;

use super::{
    ExtractionContext, GraphExtractor,
    ast::{
        child_at, first_identifier, first_node_of_kinds, node_text, point_in_range, point_span,
        preceding_documentation, range_from_node, signature_text,
    },
    facts::{
        ExtractedEdge, ExtractedEdgeTarget, ExtractedGraphFile, ExtractedImport, ExtractedNode,
        ExtractedPosition, ExtractedRange, ExtractedReference, file_local_key, node_local_key,
    },
};

const TREE_SITTER_EXACT: &str = r#"{"semanticVersion":1,"provenance":"tree_sitter","confidence":"exact","resolution":{"status":"resolved","candidates":[]}}"#;

pub(crate) struct GoExtractor;

impl GraphExtractor for GoExtractor {
    fn extract(
        &self,
        context: ExtractionContext<'_>,
    ) -> Result<ExtractedGraphFile, CodeGraphError> {
        extract_go(context)
    }
}

#[derive(Clone)]
struct Scope {
    qualified_name: String,
    range: ExtractedRange,
    depth: usize,
    containing_symbol: Option<String>,
}

struct Declaration {
    node: ExtractedNode,
    declaration_scope: ExtractedRange,
    visible_from: ExtractedPosition,
    scope_depth: usize,
    containing_symbol: Option<String>,
    callable: bool,
}

fn extract_go(context: ExtractionContext<'_>) -> Result<ExtractedGraphFile, CodeGraphError> {
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
    let package = package_declaration(root, context.text, &lines, &file_key);
    let root_scope = Scope {
        qualified_name: package
            .as_ref()
            .map(|declaration| declaration.node.name.clone())
            .unwrap_or_default(),
        range: range_from_node(root),
        depth: 0,
        containing_symbol: package
            .as_ref()
            .map(|declaration| declaration.node.local_key.clone()),
    };
    let mut declarations = package.into_iter().collect::<Vec<_>>();
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
        declaration.containing_symbol.as_ref().and_then(|parent| {
            (parent != &declaration.node.local_key).then(|| ExtractedEdge {
                source_local_key: parent.clone(),
                target: ExtractedEdgeTarget::Local(declaration.node.local_key.clone()),
                edge_kind: "contains",
                metadata_json: TREE_SITTER_EXACT.to_string(),
            })
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

fn package_declaration(
    root: Node<'_>,
    text: &str,
    lines: &[&str],
    file_key: &str,
) -> Option<Declaration> {
    let package_clause = first_node_of_kinds(root, &["package_clause"])?;
    let name_node = package_clause
        .child_by_field_name("name")
        .or_else(|| first_node_of_kinds(package_clause, &["package_identifier"]))?;
    let name = node_text(name_node, text)?;
    let range = range_from_node(package_clause);
    Some(Declaration {
        node: ExtractedNode {
            local_key: node_local_key(file_key, "module", &name, range.start),
            name: name.clone(),
            qualified_name: name,
            kind: "module",
            visibility: Some("public"),
            metadata_json: r#"{"semanticVersion":1,"provenance":"tree_sitter","confidence":"exact","exported":true}"#.to_string(),
            range,
            name_range: range_from_node(name_node),
            signature: signature_text(package_clause, text),
            documentation: preceding_documentation(package_clause, lines),
        },
        declaration_scope: range,
        visible_from: range.start,
        scope_depth: 0,
        containing_symbol: None,
        callable: false,
    })
}

fn collect_declarations(
    node: Node<'_>,
    text: &str,
    lines: &[&str],
    file_key: &str,
    scope: &Scope,
    declarations: &mut Vec<Declaration>,
    imports: &mut Vec<ExtractedImport>,
) {
    if node.kind() == "import_spec" {
        collect_import(node, text, imports);
    }

    let declaration = go_declaration(node, text, lines, file_key, scope);
    let child_scope = declaration.as_ref().map_or_else(
        || child_scope_for_non_symbol(node, scope),
        |declaration| Scope {
            qualified_name: declaration.node.qualified_name.clone(),
            range: declaration.node.range,
            depth: scope.depth.saturating_add(1),
            containing_symbol: Some(declaration.node.local_key.clone()),
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
    if matches!(node.kind(), "block" | "parameter_list") {
        Scope {
            qualified_name: scope.qualified_name.clone(),
            range: range_from_node(node),
            depth: scope.depth.saturating_add(1),
            containing_symbol: scope.containing_symbol.clone(),
        }
    } else {
        scope.clone()
    }
}

fn go_declaration(
    node: Node<'_>,
    text: &str,
    lines: &[&str],
    file_key: &str,
    scope: &Scope,
) -> Option<Declaration> {
    let (kind, name_node, callable) = match node.kind() {
        "function_declaration" => ("function", node.child_by_field_name("name")?, true),
        "method_declaration" => ("method", node.child_by_field_name("name")?, true),
        "type_spec" => ("type_alias", node.child_by_field_name("name")?, false),
        "var_spec" | "const_spec" => (
            "variable",
            first_identifier(node)?,
            first_node_of_kinds(node, &["func_literal"]).is_some(),
        ),
        "short_var_declaration" => (
            "variable",
            node.child_by_field_name("left")
                .and_then(first_identifier)
                .or_else(|| first_identifier(node))?,
            first_node_of_kinds(node, &["func_literal"]).is_some(),
        ),
        _ => return None,
    };
    let name = node_text(name_node, text)?;
    let qualified_name = if scope.qualified_name.is_empty() {
        name.clone()
    } else {
        format!("{}::{name}", scope.qualified_name)
    };
    let range = range_from_node(node);
    let exported = name.chars().next().is_some_and(char::is_uppercase);

    Some(Declaration {
        node: ExtractedNode {
            local_key: node_local_key(file_key, kind, &qualified_name, range.start),
            name,
            qualified_name,
            kind,
            visibility: exported.then_some("public"),
            metadata_json: format!(
                r#"{{"semanticVersion":1,"provenance":"tree_sitter","confidence":"exact","exported":{exported}}}"#
            ),
            range,
            name_range: range_from_node(name_node),
            signature: signature_text(node, text),
            documentation: preceding_documentation(node, lines),
        },
        declaration_scope: scope.range,
        visible_from: range.start,
        scope_depth: scope.depth,
        containing_symbol: scope.containing_symbol.clone(),
        callable,
    })
}

fn collect_import(node: Node<'_>, text: &str, imports: &mut Vec<ExtractedImport>) {
    let path_node = node.child_by_field_name("path").or_else(|| {
        first_node_of_kinds(node, &["interpreted_string_literal", "raw_string_literal"])
    });
    let Some(path_node) = path_node else {
        return;
    };
    let Some(module) = node_text(path_node, text).map(|value| value.trim_matches('"').to_string())
    else {
        return;
    };
    let alias = node
        .child_by_field_name("name")
        .and_then(|name| node_text(name, text))
        .filter(|name| name != "." && name != "_");
    imports.push(ExtractedImport {
        module,
        imported_symbol: None,
        alias,
        range: range_from_node(node),
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
    if node.kind() == "call_expression"
        && let Some((name, range, direct)) = node
            .child_by_field_name("function")
            .or_else(|| child_at(node, 0))
            .and_then(|function| call_target(function, text))
    {
        let source = containing_callable(declarations, range.start);
        let target = direct
            .then(|| resolve_visible_declaration(declarations, &name, range.start))
            .flatten();
        if let Some(target) = target {
            references.push(ExtractedReference {
                name,
                target_local_key: Some(target.node.local_key.clone()),
                range,
            });
            if let Some(source) = source
                && source.node.local_key != target.node.local_key
                && target.callable
            {
                edges.insert(ExtractedEdge {
                    source_local_key: source.node.local_key.clone(),
                    target: ExtractedEdgeTarget::Local(target.node.local_key.clone()),
                    edge_kind: "calls",
                    metadata_json: TREE_SITTER_EXACT.to_string(),
                });
            }
        } else {
            references.push(ExtractedReference {
                name,
                target_local_key: None,
                range,
            });
        }
    }

    for index in 0..node.child_count() {
        if let Some(child) = child_at(node, index) {
            collect_calls_recursive(child, text, declarations, references, edges);
        }
    }
}

fn call_target(node: Node<'_>, text: &str) -> Option<(String, ExtractedRange, bool)> {
    match node.kind() {
        "identifier" => node_text(node, text).map(|name| (name, range_from_node(node), true)),
        "selector_expression" => {
            let field = node.child_by_field_name("field")?;
            node_text(field, text).map(|name| (name, range_from_node(field), false))
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
