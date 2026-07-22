use std::collections::HashSet;

use tree_sitter::{Node, Parser};

use crate::CodeGraphError;

use super::{
    ExtractionContext, GraphExtractor,
    ast::{
        child_at, first_identifier, first_node_of_kinds, last_identifier, node_text,
        point_in_range, point_span, preceding_documentation, range_from_node, signature_text,
    },
    facts::{
        ExtractedEdge, ExtractedEdgeTarget, ExtractedGraphFile, ExtractedImport, ExtractedNode,
        ExtractedPosition, ExtractedRange, ExtractedReference, file_local_key, node_local_key,
    },
};

const TREE_SITTER_EXACT: &str = r#"{"semanticVersion":1,"provenance":"tree_sitter","confidence":"exact","resolution":{"status":"resolved","candidates":[]}}"#;
const HEURISTIC_RESOLUTION: &str = r#"{"semanticVersion":1,"provenance":"heuristic","confidence":"heuristic","resolution":{"status":"resolved","candidates":[]}}"#;

pub(crate) struct PythonExtractor;

impl GraphExtractor for PythonExtractor {
    fn extract(
        &self,
        context: ExtractionContext<'_>,
    ) -> Result<ExtractedGraphFile, CodeGraphError> {
        extract_python(context)
    }
}

#[derive(Clone)]
struct Scope {
    qualified_name: String,
    range: ExtractedRange,
    depth: usize,
    containing_symbol: Option<String>,
    is_class: bool,
}

struct Declaration {
    node: ExtractedNode,
    declaration_scope: ExtractedRange,
    visible_from: ExtractedPosition,
    scope_depth: usize,
    containing_symbol: Option<String>,
    callable: bool,
}

fn extract_python(context: ExtractionContext<'_>) -> Result<ExtractedGraphFile, CodeGraphError> {
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
        is_class: false,
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

fn collect_declarations(
    node: Node<'_>,
    text: &str,
    lines: &[&str],
    file_key: &str,
    scope: &Scope,
    declarations: &mut Vec<Declaration>,
    imports: &mut Vec<ExtractedImport>,
) {
    if matches!(node.kind(), "import_statement" | "import_from_statement") {
        collect_imports(node, text, imports);
    }

    let declaration = python_declaration(node, text, lines, file_key, scope);
    let child_scope = declaration.as_ref().map_or_else(
        || child_scope_for_non_symbol(node, scope),
        |declaration| Scope {
            qualified_name: declaration.node.qualified_name.clone(),
            range: declaration.node.range,
            depth: scope.depth.saturating_add(1),
            containing_symbol: Some(declaration.node.local_key.clone()),
            is_class: declaration.node.kind == "class",
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
    if matches!(node.kind(), "block" | "parameters" | "lambda") {
        Scope {
            qualified_name: scope.qualified_name.clone(),
            range: range_from_node(node),
            depth: scope.depth.saturating_add(1),
            containing_symbol: scope.containing_symbol.clone(),
            is_class: scope.is_class,
        }
    } else {
        scope.clone()
    }
}

fn python_declaration(
    node: Node<'_>,
    text: &str,
    lines: &[&str],
    file_key: &str,
    scope: &Scope,
) -> Option<Declaration> {
    let (kind, name_node, callable) = match node.kind() {
        "function_definition" => ("function", node.child_by_field_name("name")?, true),
        "class_definition" => ("class", node.child_by_field_name("name")?, true),
        "assignment" => (
            "variable",
            node.child_by_field_name("left")?,
            node.child_by_field_name("right")
                .is_some_and(|value| value.kind() == "lambda"),
        ),
        _ => return None,
    };
    let name_node = if kind == "variable" {
        first_identifier(name_node)?
    } else {
        name_node
    };
    let name = node_text(name_node, text)?;
    let qualified_name = if scope.qualified_name.is_empty() {
        name.clone()
    } else {
        format!("{}::{name}", scope.qualified_name)
    };
    let range = range_from_node(node);
    let exported = scope.depth == 0 && !name.starts_with('_');

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

fn collect_imports(node: Node<'_>, text: &str, imports: &mut Vec<ExtractedImport>) {
    match node.kind() {
        "import_statement" => collect_import_statement_bindings(node, text, imports),
        "import_from_statement" => collect_import_from_bindings(node, text, imports),
        _ => {}
    }
}

fn collect_import_statement_bindings(
    node: Node<'_>,
    text: &str,
    imports: &mut Vec<ExtractedImport>,
) {
    for index in 0..node.named_child_count() {
        let Ok(index) = u32::try_from(index) else {
            continue;
        };
        let Some(child) = node.named_child(index) else {
            continue;
        };
        match child.kind() {
            "dotted_name" => push_import_binding(
                node_text(child, text),
                None,
                None,
                range_from_node(node),
                imports,
            ),
            "aliased_import" => {
                let name = child
                    .child_by_field_name("name")
                    .or_else(|| first_node_of_kinds(child, &["dotted_name"]));
                let alias = child
                    .child_by_field_name("alias")
                    .or_else(|| last_identifier(child));
                push_import_binding(
                    name.and_then(|name| node_text(name, text)),
                    None,
                    alias.and_then(|alias| node_text(alias, text)),
                    range_from_node(node),
                    imports,
                );
            }
            _ => {}
        }
    }
}

fn collect_import_from_bindings(node: Node<'_>, text: &str, imports: &mut Vec<ExtractedImport>) {
    let module_node = node
        .child_by_field_name("module_name")
        .or_else(|| node.child_by_field_name("module"))
        .or_else(|| first_node_of_kinds(node, &["relative_import", "dotted_name"]));
    let Some(module) = module_node.and_then(|module| node_text(module, text)) else {
        return;
    };
    collect_from_import_bindings(node, module_node, &module, text, imports);
}

fn collect_from_import_bindings(
    node: Node<'_>,
    module_node: Option<Node<'_>>,
    module: &str,
    text: &str,
    imports: &mut Vec<ExtractedImport>,
) {
    for index in 0..node.named_child_count() {
        let Ok(index) = u32::try_from(index) else {
            continue;
        };
        let Some(child) = node.named_child(index) else {
            continue;
        };
        if Some(child) == module_node {
            continue;
        }
        match child.kind() {
            "aliased_import" => {
                let name = child
                    .child_by_field_name("name")
                    .or_else(|| first_node_of_kinds(child, &["dotted_name", "identifier"]));
                let alias = child
                    .child_by_field_name("alias")
                    .or_else(|| last_identifier(child));
                push_import_binding(
                    Some(module.to_string()),
                    name.and_then(|name| node_text(name, text)),
                    alias.and_then(|alias| node_text(alias, text)),
                    range_from_node(node),
                    imports,
                );
            }
            "dotted_name" | "identifier" => push_import_binding(
                Some(module.to_string()),
                node_text(child, text),
                None,
                range_from_node(node),
                imports,
            ),
            "wildcard_import" => push_import_binding(
                Some(module.to_string()),
                Some("*".to_string()),
                None,
                range_from_node(node),
                imports,
            ),
            _ => collect_from_import_bindings(child, module_node, module, text, imports),
        }
    }
}

fn push_import_binding(
    module: Option<String>,
    imported_symbol: Option<String>,
    alias: Option<String>,
    range: ExtractedRange,
    imports: &mut Vec<ExtractedImport>,
) {
    let Some(module) = module.filter(|module| !module.is_empty()) else {
        return;
    };
    imports.push(ExtractedImport {
        module,
        imported_symbol,
        alias: alias.filter(|alias| !alias.is_empty()),
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
    if node.kind() == "call"
        && let Some((name, range, self_member)) = node
            .child_by_field_name("function")
            .or_else(|| child_at(node, 0))
            .and_then(|function| call_target(function, text))
    {
        let source = containing_callable(declarations, range.start);
        let target = if self_member {
            source.and_then(|source| resolve_same_owner_method(declarations, &name, source))
        } else {
            resolve_visible_declaration(declarations, &name, range.start)
        };
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
                    metadata_json: if self_member {
                        HEURISTIC_RESOLUTION.to_string()
                    } else {
                        TREE_SITTER_EXACT.to_string()
                    },
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
        "identifier" => node_text(node, text).map(|name| (name, range_from_node(node), false)),
        "attribute" => {
            let attribute = node.child_by_field_name("attribute")?;
            let object_is_self = node
                .child_by_field_name("object")
                .and_then(|object| node_text(object, text))
                .is_some_and(|object| object == "self");
            node_text(attribute, text)
                .map(|name| (name, range_from_node(attribute), object_is_self))
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

fn resolve_same_owner_method<'a>(
    declarations: &'a [Declaration],
    name: &str,
    source: &Declaration,
) -> Option<&'a Declaration> {
    let owner = source.containing_symbol.as_ref()?;
    declarations.iter().find(|declaration| {
        declaration.node.name == name
            && declaration.node.kind == "function"
            && declaration.containing_symbol.as_ref() == Some(owner)
    })
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
