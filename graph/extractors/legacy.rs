use std::collections::{HashMap, HashSet};

use tree_sitter::{Node, Parser};

use crate::CodeGraphError;

use super::{
    ExtractionContext,
    ast::{
        MAX_SIGNATURE_CHARS, child_at, clean_identifier_node, clean_module_text, clean_named_node,
        first_identifier, first_node_of_kinds, has_ancestor_kind, is_identifier_node, node_text,
        point_in_range, point_span, preceding_documentation, range_from_node, signature_text,
    },
    facts::{
        ExtractedEdge, ExtractedEdgeTarget, ExtractedGraphFile, ExtractedImport, ExtractedNode,
        ExtractedReference, file_local_key, node_local_key,
    },
    language::LanguageKind,
};

pub(crate) fn extract(
    context: ExtractionContext<'_>,
) -> Result<ExtractedGraphFile, CodeGraphError> {
    let file_key = file_local_key(context.relative_path);
    let Some(tree_sitter_language) = context.language.tree_sitter_language() else {
        return Ok(empty_file(file_key, "skipped", None));
    };
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_language)
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
    let mut nodes = Vec::new();
    let mut imports = Vec::new();
    collect_symbols_and_imports(
        context.language,
        root,
        context.text,
        &lines,
        &file_key,
        &mut nodes,
        &mut imports,
    );
    let (references, edges) = collect_references(root, context.text, &nodes);

    Ok(ExtractedGraphFile {
        local_key: file_key,
        parse_status: "parsed",
        parse_error_message: None,
        nodes,
        imports,
        references,
        edges,
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

fn collect_symbols_and_imports(
    language: LanguageKind,
    node: Node<'_>,
    text: &str,
    lines: &[&str],
    file_key: &str,
    symbols: &mut Vec<ExtractedNode>,
    imports: &mut Vec<ExtractedImport>,
) {
    if let Some(symbol_kind) = classify_symbol(language, node)
        && let Some((name, name_range)) = symbol_name(language, node, text)
    {
        let range = range_from_node(node);
        symbols.push(ExtractedNode {
            local_key: node_local_key(file_key, symbol_kind, &name, range.start),
            name,
            kind: symbol_kind,
            range,
            name_range,
            signature: signature_text(node, text),
            documentation: preceding_documentation(node, lines),
        });
    }

    if is_import_node(language, node, text)
        && let Some(module) = import_module(node, text)
    {
        imports.push(ExtractedImport {
            module,
            imported_symbol: None,
            alias: None,
            range: range_from_node(node),
        });
    }

    for index in 0..node.child_count() {
        if let Some(child) = child_at(node, index) {
            collect_symbols_and_imports(language, child, text, lines, file_key, symbols, imports);
        }
    }
}

fn collect_references(
    root: Node<'_>,
    text: &str,
    symbols: &[ExtractedNode],
) -> (Vec<ExtractedReference>, Vec<ExtractedEdge>) {
    let symbol_names = symbols
        .iter()
        .map(|symbol| (symbol.name.as_str(), symbol))
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

    (references, edges.into_iter().collect())
}

fn collect_references_recursive(
    node: Node<'_>,
    text: &str,
    symbols: &[ExtractedNode],
    symbol_names: &HashMap<&str, &ExtractedNode>,
    references: &mut Vec<ExtractedReference>,
    edges: &mut HashSet<ExtractedEdge>,
) {
    if is_identifier_node(node)
        && let Some(name) = node_text(node, text)
        && let Some(target_symbol) = symbol_names.get(name.as_str())
    {
        let range = range_from_node(node);

        if range != target_symbol.name_range {
            let source_symbol = containing_symbol(symbols, range.start);
            references.push(ExtractedReference {
                name,
                target_local_key: Some(target_symbol.local_key.clone()),
                range,
            });
            if let Some(source_symbol) = source_symbol
                && source_symbol.local_key != target_symbol.local_key
            {
                edges.insert(ExtractedEdge {
                    source_local_key: source_symbol.local_key.clone(),
                    target: ExtractedEdgeTarget::Local(target_symbol.local_key.clone()),
                    edge_kind: "references",
                });
            }
        }
    }

    for index in 0..node.child_count() {
        if let Some(child) = child_at(node, index) {
            collect_references_recursive(child, text, symbols, symbol_names, references, edges);
        }
    }
}

fn containing_symbol(
    symbols: &[ExtractedNode],
    point: super::facts::ExtractedPosition,
) -> Option<&ExtractedNode> {
    symbols
        .iter()
        .filter(|symbol| point_in_range(point, symbol.range.start, symbol.range.end))
        .min_by_key(|symbol| point_span(symbol.range.start, symbol.range.end))
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
            "method_declaration" | "constructor_declaration" => Some("method"),
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
        LanguageKind::Css => match kind {
            "rule_set" => Some("selector"),
            "declaration" => Some("variable"),
            "import_statement" | "at_rule" => Some("directive"),
            _ => None,
        },
        LanguageKind::Html => match kind {
            "element" | "script_element" | "style_element" => Some("element"),
            _ => None,
        },
        LanguageKind::Vue => match kind {
            "element" | "template_element" | "script_element" | "style_element" => Some("element"),
            _ => None,
        },
        LanguageKind::Ruby => match kind {
            "method" | "singleton_method" => Some("method"),
            "class" | "singleton_class" => Some("class"),
            "module" => Some("module"),
            "assignment" | "operator_assignment" => Some("variable"),
            _ => None,
        },
        LanguageKind::Php => match kind {
            "function_definition" => Some("function"),
            "method_declaration" => Some("method"),
            "class_declaration" | "anonymous_class" => Some("class"),
            "interface_declaration" | "trait_declaration" => Some("trait"),
            "enum_declaration" => Some("enum"),
            "const_declaration" | "property_declaration" | "assignment_expression" => {
                Some("variable")
            }
            _ => None,
        },
        LanguageKind::Shell => match kind {
            "function_definition" => Some("function"),
            "variable_assignment" | "declaration_command" => Some("variable"),
            _ => None,
        },
        LanguageKind::Lua => match kind {
            "function_declaration" => Some("function"),
            "function_definition" if has_ancestor_kind(node, "function_declaration") => None,
            "function_definition" => Some("function"),
            "variable_declaration" | "assignment_statement" => Some("variable"),
            _ => None,
        },
        LanguageKind::Kotlin => match kind {
            "function_declaration" => Some("function"),
            "class_declaration" | "object_declaration" => Some("class"),
            "property_declaration" | "variable_declaration" => Some("variable"),
            _ => None,
        },
        LanguageKind::Swift => match kind {
            "function_declaration" | "init_declaration" => Some("function"),
            "class_declaration" => Some("class"),
            "struct_declaration" => Some("struct"),
            "enum_declaration" => Some("enum"),
            "protocol_declaration" => Some("trait"),
            "typealias_declaration" => Some("type_alias"),
            "property_declaration" => Some("variable"),
            _ => None,
        },
        LanguageKind::Yaml => match kind {
            "block_mapping_pair" | "flow_pair" => Some("variable"),
            _ => None,
        },
        LanguageKind::Dockerfile => match kind {
            "from_instruction" => Some("dependency"),
            "run_instruction" => Some("command"),
            "copy_instruction" | "add_instruction" => Some("file"),
            "env_pair" | "arg_pair" | "label_pair" => Some("variable"),
            _ => None,
        },
        LanguageKind::Json | LanguageKind::Toml | LanguageKind::Markdown => None,
    }
}

fn symbol_name(
    language: LanguageKind,
    node: Node<'_>,
    text: &str,
) -> Option<(String, super::facts::ExtractedRange)> {
    if let Some(name_node) = node.child_by_field_name("name") {
        return clean_identifier_node(name_node, text);
    }

    if let Some(name) = special_symbol_name(language, node, text) {
        return Some(name);
    }

    for field_name in ["pattern", "left", "declarator", "key"] {
        if let Some(child) = node.child_by_field_name(field_name)
            && let Some(identifier) = first_identifier(child)
        {
            return clean_identifier_node(identifier, text);
        }
    }

    if language == LanguageKind::Rust
        && node.kind() == "impl_item"
        && let Some(type_node) = node.child_by_field_name("type")
        && let Some(identifier) = first_identifier(type_node)
    {
        return clean_identifier_node(identifier, text);
    }

    first_identifier(node).and_then(|identifier| clean_identifier_node(identifier, text))
}

fn special_symbol_name(
    language: LanguageKind,
    node: Node<'_>,
    text: &str,
) -> Option<(String, super::facts::ExtractedRange)> {
    match language {
        LanguageKind::Css => first_node_of_kinds(
            node,
            &[
                "id_name",
                "class_name",
                "tag_name",
                "property_name",
                "at_keyword",
                "keyframes_name",
                "identifier",
            ],
        )
        .and_then(|name_node| clean_named_node(name_node, text)),
        LanguageKind::Html | LanguageKind::Vue => first_node_of_kinds(node, &["tag_name"])
            .and_then(|name_node| clean_named_node(name_node, text)),
        LanguageKind::Yaml => node
            .child_by_field_name("key")
            .and_then(|key| clean_named_node(key, text)),
        LanguageKind::Dockerfile => {
            if let Some(name_node) = node.child_by_field_name("name") {
                return clean_named_node(name_node, text);
            }
            first_node_of_kinds(
                node,
                &[
                    "image_spec",
                    "image_name",
                    "path",
                    "expose_port",
                    "shell_command",
                    "variable",
                    "unquoted_string",
                    "string_literal",
                ],
            )
            .and_then(|name_node| clean_named_node(name_node, text))
        }
        _ => None,
    }
}

fn is_import_node(language: LanguageKind, node: Node<'_>, text: &str) -> bool {
    if language == LanguageKind::Ruby && node.kind() == "call" {
        return first_identifier(node)
            .and_then(|identifier| node_text(identifier, text))
            .is_some_and(|name| matches!(name.as_str(), "require" | "load" | "require_relative"));
    }

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
            | (LanguageKind::Css, "import_statement")
            | (LanguageKind::Ruby, "call")
            | (
                LanguageKind::Php,
                "namespace_use_declaration" | "include_expression"
            )
            | (LanguageKind::Kotlin, "import")
            | (LanguageKind::Swift, "import_declaration")
            | (
                LanguageKind::Dockerfile,
                "from_instruction" | "copy_instruction" | "add_instruction"
            )
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
            "string_content",
            "encapsed_string",
            "namespace_name",
            "image_spec",
            "image_name",
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
            .trim_start_matches("@import ")
            .trim_start_matches("require ")
            .trim_start_matches("require_relative ")
            .trim_start_matches("load ")
            .trim_start_matches("include ")
            .trim_start_matches("require_once ")
            .trim_start_matches("include_once ")
            .trim_start_matches("FROM ")
            .trim_start_matches("COPY ")
            .trim_start_matches("ADD ")
            .trim_end_matches(';')
            .trim()
            .chars()
            .take(MAX_SIGNATURE_CHARS)
            .collect()
    })
}
