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

pub(crate) struct TypeScriptFamilyExtractor;

impl GraphExtractor for TypeScriptFamilyExtractor {
    fn extract(
        &self,
        context: ExtractionContext<'_>,
    ) -> Result<ExtractedGraphFile, CodeGraphError> {
        extract_typescript_family(context)
    }
}

#[derive(Clone)]
struct Scope {
    qualified_name: String,
    range: ExtractedRange,
    depth: usize,
    containing_symbol: Option<String>,
    is_class_or_interface: bool,
}

struct Declaration {
    node: ExtractedNode,
    declaration_scope: ExtractedRange,
    visible_from: ExtractedPosition,
    scope_depth: usize,
    containing_symbol: Option<String>,
    callable: bool,
}

fn extract_typescript_family(
    context: ExtractionContext<'_>,
) -> Result<ExtractedGraphFile, CodeGraphError> {
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
        is_class_or_interface: false,
    };
    let mut declarations = Vec::new();
    let mut imports = Vec::new();
    collect_declarations(
        root,
        context.text,
        &lines,
        &file_key,
        &root_scope,
        false,
        false,
        &mut declarations,
        &mut imports,
    );
    let (references, call_edges) = collect_calls(root, context.text, &declarations);
    let contains_edges = declarations.iter().filter_map(|declaration| {
        declaration
            .containing_symbol
            .as_ref()
            .filter(|parent| *parent != &declaration.node.local_key)
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
    exported: bool,
    default_exported: bool,
    declarations: &mut Vec<Declaration>,
    imports: &mut Vec<ExtractedImport>,
) {
    match node.kind() {
        "import_statement" => collect_module_bindings(node, text, false, imports),
        "export_statement" if node.child_by_field_name("source").is_some() => {
            collect_module_bindings(node, text, true, imports)
        }
        _ => {}
    }

    let declaration = typescript_declaration(
        node,
        text,
        lines,
        file_key,
        scope,
        exported,
        default_exported,
    );
    let has_declaration = declaration.is_some();
    let child_scope = declaration.as_ref().map_or_else(
        || child_scope_for_non_symbol(node, scope),
        |declaration| Scope {
            qualified_name: declaration.node.qualified_name.clone(),
            range: declaration.node.range,
            depth: scope.depth.saturating_add(1),
            containing_symbol: Some(declaration.node.local_key.clone()),
            is_class_or_interface: matches!(declaration.node.kind, "class" | "trait"),
        },
    );

    if let Some(declaration) = declaration {
        declarations.push(declaration);
    }
    let child_exported = !has_declaration && (exported || node.kind() == "export_statement");
    let child_default_exported =
        !has_declaration && (default_exported || is_default_export_statement(node, text));
    for index in 0..node.child_count() {
        if let Some(child) = child_at(node, index) {
            collect_declarations(
                child,
                text,
                lines,
                file_key,
                &child_scope,
                child_exported,
                child_default_exported,
                declarations,
                imports,
            );
        }
    }
}

fn child_scope_for_non_symbol(node: Node<'_>, scope: &Scope) -> Scope {
    if matches!(
        node.kind(),
        "statement_block" | "formal_parameters" | "arrow_function"
    ) {
        Scope {
            qualified_name: scope.qualified_name.clone(),
            range: range_from_node(node),
            depth: scope.depth.saturating_add(1),
            containing_symbol: scope.containing_symbol.clone(),
            is_class_or_interface: scope.is_class_or_interface,
        }
    } else {
        scope.clone()
    }
}

fn typescript_declaration(
    node: Node<'_>,
    text: &str,
    lines: &[&str],
    file_key: &str,
    scope: &Scope,
    exported: bool,
    default_exported: bool,
) -> Option<Declaration> {
    let (kind, name_node, callable) = match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            ("function", node.child_by_field_name("name")?, true)
        }
        "method_definition" | "method_signature" if scope.is_class_or_interface => {
            ("method", node.child_by_field_name("name")?, true)
        }
        "class_declaration" => ("class", node.child_by_field_name("name")?, true),
        "interface_declaration" => ("trait", node.child_by_field_name("name")?, false),
        "enum_declaration" => ("enum", node.child_by_field_name("name")?, false),
        "type_alias_declaration" => ("type_alias", node.child_by_field_name("name")?, false),
        "variable_declarator" => {
            let callable = node.child_by_field_name("value").is_some_and(|value| {
                matches!(value.kind(), "arrow_function" | "function_expression")
            });
            ("variable", node.child_by_field_name("name")?, callable)
        }
        "required_parameter" | "optional_parameter" => ("variable", first_identifier(node)?, false),
        _ => return None,
    };
    let name = node_text(name_node, text)?;
    let qualified_name = if scope.qualified_name.is_empty() {
        name.clone()
    } else {
        format!("{}::{name}", scope.qualified_name)
    };
    let range = range_from_node(node);
    let source = node_text(node, text).unwrap_or_default();
    let visibility = accessibility(node, text).or_else(|| exported.then_some("public"));
    let metadata_json = declaration_metadata(exported, default_exported, &source);

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
        // `let`/`const`/parameter bindings only become viable after their
        // declaration point; declarations with other kinds keep their normal
        // language-level visibility rules.
        visible_from: range.start,
        scope_depth: scope.depth,
        containing_symbol: scope.containing_symbol.clone(),
        callable,
    })
}

fn accessibility(node: Node<'_>, text: &str) -> Option<&'static str> {
    for index in 0..node.child_count() {
        let child = child_at(node, index)?;
        if child.kind() == "accessibility_modifier" {
            return node_text(child, text).and_then(|value| match value.as_str() {
                "public" => Some("public"),
                "private" => Some("private"),
                "protected" => Some("protected"),
                _ => None,
            });
        }
    }
    None
}

fn declaration_metadata(exported: bool, default_exported: bool, source: &str) -> String {
    let async_marker = source.split_whitespace().any(|token| token == "async");
    let static_marker = source.split_whitespace().any(|token| token == "static");
    format!(
        r#"{{"semanticVersion":1,"provenance":"tree_sitter","confidence":"exact","exported":{exported},"defaultExport":{default_exported},"async":{async_marker},"static":{static_marker}}}"#
    )
}

fn is_default_export_statement(node: Node<'_>, text: &str) -> bool {
    node.kind() == "export_statement"
        && node_text(node, text)
            .is_some_and(|source| source.trim_start().starts_with("export default"))
}

fn collect_module_bindings(
    node: Node<'_>,
    text: &str,
    is_reexport: bool,
    imports: &mut Vec<ExtractedImport>,
) {
    let Some(source_node) = node.child_by_field_name("source") else {
        return;
    };
    let Some(module) = node_text(source_node, text) else {
        return;
    };
    let module = module.trim_matches(['"', '\'', '`']);
    let statement_range = range_from_node(node);
    let mut binding_count = 0;
    for index in 0..node.child_count() {
        let Some(child) = child_at(node, index) else {
            continue;
        };
        match child.kind() {
            "import_clause" => {
                binding_count +=
                    collect_import_clause(child, text, module, statement_range, imports);
            }
            "export_clause" => {
                binding_count +=
                    collect_export_clause(child, text, module, statement_range, imports);
            }
            "namespace_export" if is_reexport => {
                let alias = child
                    .child_by_field_name("name")
                    .or_else(|| last_identifier(child))
                    .and_then(|name| node_text(name, text));
                push_module_binding(module, "*", alias.as_deref(), statement_range, imports);
                binding_count += 1;
            }
            _ => {}
        }
    }
    if is_reexport && binding_count == 0 {
        // `export * from "module"` has no named binding node, but is still a
        // concrete module dependency.
        push_module_binding(module, "*", None, statement_range, imports);
    }
}

fn collect_import_clause(
    clause: Node<'_>,
    text: &str,
    module: &str,
    range: ExtractedRange,
    imports: &mut Vec<ExtractedImport>,
) -> usize {
    let mut count = 0;
    for index in 0..clause.child_count() {
        let Some(child) = child_at(clause, index) else {
            continue;
        };
        match child.kind() {
            "identifier" => {
                if let Some(alias) = node_text(child, text) {
                    push_module_binding(module, "default", Some(&alias), range, imports);
                    count += 1;
                }
            }
            "named_imports" => {
                for specifier_index in 0..child.child_count() {
                    if let Some(specifier) = child_at(child, specifier_index)
                        && specifier.kind() == "import_specifier"
                    {
                        count += collect_specifier(specifier, text, module, range, imports);
                    }
                }
            }
            "namespace_import" => {
                let alias = child
                    .child_by_field_name("name")
                    .or_else(|| last_identifier(child))
                    .and_then(|name| node_text(name, text));
                if let Some(alias) = alias {
                    push_module_binding(module, "*", Some(&alias), range, imports);
                    count += 1;
                }
            }
            _ => {}
        }
    }
    count
}

fn collect_export_clause(
    clause: Node<'_>,
    text: &str,
    module: &str,
    range: ExtractedRange,
    imports: &mut Vec<ExtractedImport>,
) -> usize {
    let mut count = 0;
    for index in 0..clause.child_count() {
        if let Some(specifier) = child_at(clause, index)
            && specifier.kind() == "export_specifier"
        {
            count += collect_specifier(specifier, text, module, range, imports);
        }
    }
    count
}

fn collect_specifier(
    specifier: Node<'_>,
    text: &str,
    module: &str,
    range: ExtractedRange,
    imports: &mut Vec<ExtractedImport>,
) -> usize {
    let name = specifier
        .child_by_field_name("name")
        .or_else(|| first_identifier(specifier))
        .and_then(|name| node_text(name, text));
    let alias = specifier
        .child_by_field_name("alias")
        .and_then(|alias| node_text(alias, text));
    let Some(name) = name else {
        return 0;
    };
    push_module_binding(module, &name, alias.as_deref(), range, imports);
    1
}

fn push_module_binding(
    module: &str,
    imported_symbol: &str,
    alias: Option<&str>,
    range: ExtractedRange,
    imports: &mut Vec<ExtractedImport>,
) {
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
    let target = match node.kind() {
        "call_expression" => node
            .child_by_field_name("function")
            .and_then(|function| call_target(function, text)),
        "new_expression" => node
            .child_by_field_name("constructor")
            .or_else(|| first_identifier(node))
            .and_then(|constructor| call_target(constructor, text)),
        _ => None,
    };
    if let Some((name, range, heuristic, is_this_member_call)) = target {
        let source = containing_callable(declarations, range.start);
        let target = if is_this_member_call {
            source.and_then(|source| resolve_same_owner_method(declarations, &name, source))
        } else if heuristic {
            None
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
                    metadata_json: if heuristic {
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

fn call_target(node: Node<'_>, text: &str) -> Option<(String, ExtractedRange, bool, bool)> {
    match node.kind() {
        "identifier" => {
            node_text(node, text).map(|name| (name, range_from_node(node), false, false))
        }
        "member_expression" => {
            let property = node.child_by_field_name("property")?;
            let receiver_is_this = node
                .child_by_field_name("object")
                .is_some_and(|receiver| receiver.kind() == "this");
            node_text(property, text)
                .map(|name| (name, range_from_node(property), true, receiver_is_this))
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
            && declaration.node.kind == "method"
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
