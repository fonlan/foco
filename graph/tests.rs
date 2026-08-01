use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use rusqlite::Connection;

use crate::{
    WatcherDiagnostics,
    extractors::{
        LanguageKind, extract_file,
        facts::{
            ExtractedEdge, ExtractedEdgeTarget, ExtractedPosition, file_local_key, node_local_key,
        },
    },
    index_workspace,
};

#[test]
fn watcher_diagnostics_tracks_only_counts_without_event_payloads() {
    let mut diagnostics = WatcherDiagnostics::new();
    diagnostics.record_event(true);
    diagnostics.record_event(false);
    diagnostics.record_receive_timeout();
    diagnostics.record_event_error();
    diagnostics.record_debounce_reset();

    let refresh_window = diagnostics.take_refresh_window();

    assert_eq!(
        refresh_window,
        crate::WatcherDiagnosticCounters {
            receive_timeouts: 1,
            events_received: 2,
            relevant_events: 1,
            filtered_events: 1,
            event_errors: 1,
            debounce_resets: 1,
            refreshes: 1,
        }
    );
    assert_eq!(diagnostics.total, refresh_window);
    assert_eq!(
        diagnostics.take_refresh_window(),
        crate::WatcherDiagnosticCounters {
            refreshes: 1,
            ..Default::default()
        }
    );
}

#[test]
fn extraction_facts_are_owned_and_use_stable_local_keys() {
    let extracted = extract_file(
        LanguageKind::Rust,
        "src/lib.rs",
        Path::new("src/lib.rs"),
        "fn helper() {}\nfn caller() { helper(); }\n",
    )
    .expect("extract Rust source");

    let helper = extracted
        .nodes
        .iter()
        .find(|node| node.name == "helper")
        .expect("helper node");

    assert_eq!(
        helper.local_key,
        node_local_key(
            &file_local_key("src/lib.rs"),
            "function",
            "helper",
            ExtractedPosition { line: 0, column: 0 },
        )
    );
}

#[test]
fn extracted_edge_can_describe_a_deferred_cross_file_target() {
    let edge = ExtractedEdge {
        source_local_key: "file:src/lib.rs:node:function:caller:1:3".to_string(),
        target: ExtractedEdgeTarget::Unresolved {
            description: "crate::other::helper".to_string(),
        },
        edge_kind: "calls",
        metadata_json: r#"{"semanticVersion":1,"provenance":"module_resolver","confidence":"candidate","resolution":{"status":"unresolved","candidates":[]}}"#.to_string(),
    };

    assert!(matches!(
        edge.target,
        ExtractedEdgeTarget::Unresolved { .. }
    ));
}

#[test]
fn indexes_workspace_incrementally_and_records_syntax_confirmed_calls() {
    let workspace = tempfile::tempdir().expect("workspace");
    fs::write(
        workspace.path().join("lib.rs"),
        "fn helper() {}\nfn caller() { let value = helper(); }\n",
    )
    .expect("source");

    let initial = index_workspace(workspace.path()).expect("initial index");
    let unchanged = index_workspace(workspace.path()).expect("unchanged index");
    let connection = graph_connection(workspace.path());

    assert_eq!(initial.indexed_files, 1);
    assert_eq!(unchanged.unchanged_files, 1);
    assert_eq!(
        query_count(
            &connection,
            "SELECT COUNT(*) FROM code_graph_edges WHERE edge_kind = 'calls'",
        ),
        1
    );
    assert_eq!(
        query_count(
            &connection,
            "SELECT COUNT(*) FROM code_graph_references WHERE name = 'helper'",
        ),
        1
    );
}

#[test]
fn removes_stale_graph_rows_for_deleted_files() {
    let workspace = tempfile::tempdir().expect("workspace");
    let source_path = workspace.path().join("lib.rs");
    fs::write(&source_path, "fn helper() {}\n").expect("source");
    index_workspace(workspace.path()).expect("initial index");

    fs::remove_file(&source_path).expect("remove source");
    let report = index_workspace(workspace.path()).expect("index after delete");
    let connection = graph_connection(workspace.path());

    assert_eq!(report.deleted_files, 1);
    assert_eq!(
        query_count(&connection, "SELECT COUNT(*) FROM code_graph_files"),
        0
    );
}

#[test]
fn watcher_reindexes_a_modified_python_file() {
    let workspace = tempfile::tempdir().expect("workspace");
    let source_path = workspace.path().join("app.py");
    fs::write(&source_path, "def first():\n    return 1\n").expect("initial source");
    index_workspace(workspace.path()).expect("initial index");
    let watcher =
        crate::start_code_graph_watcher_with_debounce(workspace.path(), Duration::from_millis(20))
            .expect("start watcher");

    fs::write(
        &source_path,
        "def first():\n    return 1\n\ndef added():\n    return first()\n",
    )
    .expect("modified source");
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut indexed = false;
    while Instant::now() < deadline {
        let connection = graph_connection(workspace.path());
        if query_count(
            &connection,
            "SELECT COUNT(*) FROM code_graph_symbols WHERE qualified_name = 'added'",
        ) == 1
        {
            indexed = true;
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }

    drop(watcher);

    assert!(
        indexed,
        "watcher did not persist the modified Python symbol"
    );
}

#[test]
fn indexes_ets_files_as_typescript() {
    let workspace = tempfile::tempdir().expect("workspace");
    fs::write(
        workspace.path().join("Widget.ets"),
        "export function build_title(value: string): string { return value.trim(); }\n",
    )
    .expect("ets source");

    let report = index_workspace(workspace.path()).expect("index ets");
    let connection = graph_connection(workspace.path());

    assert_eq!(report.indexed_files, 1);
    assert_eq!(
        query_count(
            &connection,
            "SELECT COUNT(*) FROM code_graph_files WHERE path = 'Widget.ets' AND language = 'typescript'",
        ),
        1
    );
}

#[test]
fn semantic_fixture_records_function_invocations_as_calls_edges() {
    let workspace = semantic_fixture_workspace("rust_workspace");

    index_workspace(workspace.path()).expect("index fixture");

    let connection = graph_connection(workspace.path());
    let relation = connection
        .query_row(
            "SELECT edge.edge_kind, edge.metadata_json
             FROM code_graph_edges edge
             JOIN code_graph_symbols source ON source.id = edge.source_symbol_id
             JOIN code_graph_symbols target ON target.id = edge.target_symbol_id
             WHERE source.name = 'render' AND target.name = 'local_helper'",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .expect("render calls local helper");

    assert_eq!(
        relation,
        (
            "calls".to_string(),
            r#"{"semanticVersion":1,"provenance":"tree_sitter","confidence":"exact","resolution":{"status":"resolved","candidates":[]}}"#.to_string()
        )
    );
}

#[test]
fn semantic_fixture_resolves_same_name_calls_in_the_nearest_lexical_scope() {
    let workspace = semantic_fixture_workspace("rust_workspace");

    index_workspace(workspace.path()).expect("index fixture");

    let connection = graph_connection(workspace.path());
    let relation = connection
        .query_row(
            "SELECT target.start_line
             FROM code_graph_edges edge
             JOIN code_graph_symbols source ON source.id = edge.source_symbol_id
             JOIN code_graph_symbols target ON target.id = edge.target_symbol_id
             WHERE source.name = 'call_outer' AND target.name = 'same_name'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("outer same-name relation");

    assert_eq!(relation, 13);
}

#[test]
fn semantic_fixture_resolves_cross_file_rust_imports_and_calls() {
    let workspace = semantic_fixture_workspace("rust_workspace");

    index_workspace(workspace.path()).expect("index fixture");

    let connection = graph_connection(workspace.path());
    let relation = connection
        .query_row(
            "SELECT edge.metadata_json
             FROM code_graph_edges edge
             JOIN code_graph_symbols source ON source.id = edge.source_symbol_id
             JOIN code_graph_symbols target ON target.id = edge.target_symbol_id
             WHERE source.name = 'local_helper' AND target.name = 'decorate'
               AND source.file_id <> target.file_id",
            [],
            |row| row.get::<_, String>(0),
        )
        .expect("cross-file decorate call");
    let resolution = connection
        .query_row(
            "SELECT resolution.resolution
             FROM code_graph_import_resolutions resolution
             JOIN code_graph_imports import ON import.id = resolution.import_id
             JOIN code_graph_files file ON file.id = import.file_id
             WHERE file.path = 'src/lib.rs' AND import.module = 'crate::formatting'",
            [],
            |row| row.get::<_, String>(0),
        )
        .expect("formatting resolution");

    assert_eq!(resolution, "exact");
    assert!(relation.contains("\"provenance\":\"module_resolver\""));
}

#[test]
fn resolver_keeps_ambiguous_and_external_typescript_imports_non_exact() {
    let workspace = tempfile::tempdir().expect("workspace");
    fs::create_dir_all(workspace.path().join("src")).expect("source directory");
    fs::write(
        workspace.path().join("src/consumer.ts"),
        "import { value } from './shared';\nimport { join } from 'node:path';\nexport function caller() { return value(); }\n",
    )
    .expect("consumer source");
    fs::write(
        workspace.path().join("src/shared.ts"),
        "export function value() { return 'ts'; }\n",
    )
    .expect("typescript source");
    fs::write(
        workspace.path().join("src/shared.tsx"),
        "export function value() { return 'tsx'; }\n",
    )
    .expect("tsx source");

    index_workspace(workspace.path()).expect("index workspace");

    let connection = graph_connection(workspace.path());
    let (candidate_count, external_count, cross_file_calls, candidates_json) = (
        query_count(
            &connection,
            "SELECT COUNT(*) FROM code_graph_import_resolutions WHERE resolution = 'candidate'",
        ),
        query_count(
            &connection,
            "SELECT COUNT(*) FROM code_graph_import_resolutions WHERE resolution = 'external'",
        ),
        query_count(
            &connection,
            "SELECT COUNT(*)
             FROM code_graph_edges edge
             JOIN code_graph_symbols source ON source.id = edge.source_symbol_id
             JOIN code_graph_symbols target ON target.id = edge.target_symbol_id
             WHERE source.file_id <> target.file_id",
        ),
        connection
            .query_row(
                "SELECT candidates_json
                 FROM code_graph_import_resolutions
                 WHERE resolution = 'candidate'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("candidate metadata"),
    );

    assert_eq!(candidate_count, 1);
    assert_eq!(external_count, 1);
    assert_eq!(cross_file_calls, 0);
    assert!(candidates_json.contains("\"path\":\"src/shared.ts\""));
    assert!(candidates_json.contains("\"language\":\"typescript\""));
}

#[test]
fn resolver_does_not_turn_a_locally_resolved_shadow_into_an_import_call() {
    let workspace = tempfile::tempdir().expect("workspace");
    fs::create_dir_all(workspace.path().join("src")).expect("source directory");
    fs::write(
        workspace.path().join("src/consumer.ts"),
        "import { value } from './producer';\nexport function caller() { const value = () => 1; return value(); }\n",
    )
    .expect("consumer source");
    fs::write(
        workspace.path().join("src/producer.ts"),
        "export function value() { return 2; }\n",
    )
    .expect("producer source");

    index_workspace(workspace.path()).expect("index workspace");

    let connection = graph_connection(workspace.path());
    let imported_call_count = query_count(
        &connection,
        "SELECT COUNT(*)
         FROM code_graph_edges edge
         JOIN code_graph_symbols source ON source.id = edge.source_symbol_id
         JOIN code_graph_symbols target ON target.id = edge.target_symbol_id
         WHERE source.name = 'caller' AND target.name = 'value'
           AND source.file_id <> target.file_id",
    );

    assert_eq!(imported_call_count, 0);
}

#[test]
fn resolver_resolves_dotted_typescript_specifiers_with_implicit_extensions() {
    let workspace = tempfile::tempdir().expect("workspace");
    fs::create_dir_all(workspace.path().join("src")).expect("source directory");
    fs::write(
        workspace.path().join("src/consumer.ts"),
        "import { value } from './feature.test';\nexport function caller() { return value(); }\n",
    )
    .expect("consumer source");
    fs::write(
        workspace.path().join("src/feature.test.ts"),
        "export function value() { return 'ok'; }\n",
    )
    .expect("producer source");

    index_workspace(workspace.path()).expect("index workspace");

    let connection = graph_connection(workspace.path());
    let resolution = connection
        .query_row(
            "SELECT resolution.resolution
             FROM code_graph_import_resolutions resolution
             JOIN code_graph_imports import ON import.id = resolution.import_id
             WHERE import.module = './feature.test'",
            [],
            |row| row.get::<_, String>(0),
        )
        .expect("dotted specifier resolution");

    assert_eq!(resolution, "exact");
}

#[test]
fn resolver_connects_default_imports_to_unique_default_callable_exports() {
    let workspace = tempfile::tempdir().expect("workspace");
    fs::create_dir_all(workspace.path().join("src")).expect("source directory");
    fs::write(
        workspace.path().join("src/consumer.ts"),
        "import render from './render';\nexport function caller() { return render(); }\n",
    )
    .expect("consumer source");
    fs::write(
        workspace.path().join("src/render.ts"),
        "export default function render() { return 'ok'; }\n",
    )
    .expect("producer source");

    index_workspace(workspace.path()).expect("index workspace");

    let connection = graph_connection(workspace.path());
    let default_call_count = query_count(
        &connection,
        "SELECT COUNT(*)
         FROM code_graph_edges edge
         JOIN code_graph_symbols source ON source.id = edge.source_symbol_id
         JOIN code_graph_symbols target ON target.id = edge.target_symbol_id
         WHERE source.name = 'caller' AND target.name = 'render'
           AND source.file_id <> target.file_id",
    );

    assert_eq!(default_call_count, 1);
}

#[test]
fn resolver_refreshes_unchanged_importers_after_their_target_is_deleted() {
    let workspace = tempfile::tempdir().expect("workspace");
    fs::create_dir_all(workspace.path().join("src")).expect("source directory");
    fs::write(
        workspace.path().join("src/consumer.ts"),
        "import { value } from './producer';\nexport function caller() { return value(); }\n",
    )
    .expect("consumer source");
    let producer_path = workspace.path().join("src/producer.ts");
    fs::write(&producer_path, "export function value() { return 'ok'; }\n")
        .expect("producer source");

    index_workspace(workspace.path()).expect("initial index");
    fs::remove_file(&producer_path).expect("delete producer");
    let report = index_workspace(workspace.path()).expect("refresh after delete");

    let connection = graph_connection(workspace.path());
    let (unresolved_count, cross_file_calls) = (
        query_count(
            &connection,
            "SELECT COUNT(*)
             FROM code_graph_import_resolutions resolution
             JOIN code_graph_imports import ON import.id = resolution.import_id
             JOIN code_graph_files file ON file.id = import.file_id
             WHERE file.path = 'src/consumer.ts' AND resolution.resolution = 'unresolved'",
        ),
        query_count(
            &connection,
            "SELECT COUNT(*)
             FROM code_graph_edges edge
             JOIN code_graph_symbols source ON source.id = edge.source_symbol_id
             JOIN code_graph_symbols target ON target.id = edge.target_symbol_id
             WHERE source.file_id <> target.file_id",
        ),
    );

    assert_eq!(report.deleted_files, 1);
    assert_eq!(unresolved_count, 1);
    assert_eq!(cross_file_calls, 0);
}

#[test]
fn semantic_fixture_reports_error_files_without_partial_symbols() {
    let workspace = semantic_fixture_workspace("rust_workspace");

    let report = index_workspace(workspace.path()).expect("index fixture");
    let connection = graph_connection(workspace.path());
    let broken_symbol_count = query_count(
        &connection,
        "SELECT COUNT(*)
         FROM code_graph_symbols symbol
         JOIN code_graph_files file ON file.id = symbol.file_id
         WHERE file.path = 'src/broken.rs'",
    );

    assert_eq!(report.parse_errors, 1);
    assert_eq!(broken_symbol_count, 0);
    assert_eq!(
        connection
            .query_row(
                "SELECT parse_status.status
                 FROM code_graph_parse_status parse_status
                 JOIN code_graph_files file ON file.id = parse_status.file_id
                 WHERE file.path = 'src/broken.rs'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("broken file parse status"),
        "error"
    );
}

#[test]
fn typescript_family_extractor_records_import_aliases_and_reexports() {
    let workspace = semantic_fixture_workspace("typescript_workspace");

    index_workspace(workspace.path()).expect("index fixture");

    let connection = graph_connection(workspace.path());
    let import = connection
        .query_row(
            "SELECT import.module, import.imported_symbol, import.alias
             FROM code_graph_imports import
             JOIN code_graph_files file ON file.id = import.file_id
             WHERE file.path = 'src/Panel.tsx'",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .expect("tsx import");
    let reexport_import_count = query_count(
        &connection,
        "SELECT COUNT(*)
         FROM code_graph_imports import
         JOIN code_graph_files file ON file.id = import.file_id
         WHERE file.path = 'src/public.ts'",
    );

    assert_eq!(
        import,
        ("./index".to_string(), Some("render".to_string()), None)
    );
    assert_eq!(reexport_import_count, 1);
    assert_eq!(
        query_count(
            &connection,
            "SELECT COUNT(*)
             FROM code_graph_imports import
             JOIN code_graph_files file ON file.id = import.file_id
             WHERE file.path = 'src/index.ts' AND import.module = './public'",
        ),
        1
    );
}

#[test]
fn rust_extractor_does_not_turn_a_shadowed_local_variable_call_into_a_function_call() {
    let extracted = extract_file(
        LanguageKind::Rust,
        "src/lib.rs",
        Path::new("src/lib.rs"),
        "fn helper() {}\nfn caller() { let helper = || {}; helper(); }\n",
    )
    .expect("extract Rust source");

    assert!(
        !extracted.edges.iter().any(|edge| edge.edge_kind == "calls"),
        "a shadowing local must block a strong function call edge"
    );
}

#[test]
fn rust_extractor_resolves_qualified_calls_by_the_final_segment() {
    let extracted = extract_file(
        LanguageKind::Rust,
        "src/lib.rs",
        Path::new("src/lib.rs"),
        "struct Service;\nimpl Service { fn helper() {} fn caller() { Self::helper(); } }\n",
    )
    .expect("extract Rust source");

    assert!(extracted.edges.iter().any(|edge| {
        edge.edge_kind == "calls" && edge.metadata_json.contains("\"confidence\":\"heuristic\"")
    }));
}

#[test]
fn rust_extractor_does_not_apply_a_later_local_shadow_before_its_declaration() {
    let extracted = extract_file(
        LanguageKind::Rust,
        "src/lib.rs",
        Path::new("src/lib.rs"),
        "fn helper() {}\nfn caller() { helper(); let helper = || {}; }\n",
    )
    .expect("extract Rust source");

    assert!(extracted.edges.iter().any(|edge| edge.edge_kind == "calls"));
}

#[test]
fn rust_extractor_collects_public_braced_alias_and_glob_use_imports() {
    let extracted = extract_file(
        LanguageKind::Rust,
        "src/lib.rs",
        Path::new("src/lib.rs"),
        "pub use crate::{alpha as renamed, beta};\nuse std::fmt::*;\n",
    )
    .expect("extract Rust source");

    let imports = extracted
        .imports
        .iter()
        .map(|import| {
            (
                import.module.as_str(),
                import.imported_symbol.as_deref(),
                import.alias.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    assert!(imports.contains(&("crate", Some("alpha"), Some("renamed"))));
    assert!(imports.contains(&("crate", Some("beta"), None)));
    assert!(
        imports.contains(&("std::fmt", Some("*"), None)),
        "glob import missing from {imports:?}"
    );
}

#[test]
fn typescript_extractor_only_emits_calls_for_call_or_new_syntax() {
    let extracted = extract_file(
        LanguageKind::TypeScript,
        "src/example.ts",
        Path::new("src/example.ts"),
        "class Service {}\nfunction create() { const value = Service; return new Service(); }\n",
    )
    .expect("extract TypeScript source");

    assert_eq!(
        extracted
            .edges
            .iter()
            .filter(|edge| edge.edge_kind == "calls")
            .count(),
        1,
        "only the new-expression should create a constructor call; the variable read must not"
    );
}

#[test]
fn typescript_extractor_resolves_calls_to_arrow_function_bindings() {
    let extracted = extract_file(
        LanguageKind::TypeScript,
        "src/example.ts",
        Path::new("src/example.ts"),
        "const helper = () => {};\nfunction caller() { helper(); }\n",
    )
    .expect("extract TypeScript source");

    assert!(extracted.edges.iter().any(|edge| edge.edge_kind == "calls"));
}

#[test]
fn typescript_extractor_resolves_member_calls_by_the_member_name() {
    let extracted = extract_file(
        LanguageKind::TypeScript,
        "src/example.ts",
        Path::new("src/example.ts"),
        "class Service { run() {} create() { this.run(); } }\n",
    )
    .expect("extract TypeScript source");

    assert!(extracted.edges.iter().any(|edge| {
        edge.edge_kind == "calls" && edge.metadata_json.contains("\"confidence\":\"heuristic\"")
    }));
}

#[test]
fn typescript_extractor_keeps_unconstrained_member_calls_unresolved() {
    let extracted = extract_file(
        LanguageKind::TypeScript,
        "src/example.ts",
        Path::new("src/example.ts"),
        "class Service { render() {} }\nfunction caller() { const service = () => {}; service.render(); }\n",
    )
    .expect("extract TypeScript source");

    assert!(
        !extracted.edges.iter().any(|edge| edge.edge_kind == "calls"),
        "an arbitrary receiver must not resolve to an unrelated class method"
    );
    assert!(
        extracted.references.iter().any(|reference| {
            reference.name == "render" && reference.target_local_key.is_none()
        })
    );
}

#[test]
fn typescript_extractor_collects_mixed_imports_and_star_reexports() {
    let extracted = extract_file(
        LanguageKind::TypeScript,
        "src/example.ts",
        Path::new("src/example.ts"),
        "import Default, { named as local } from 'module';\nimport * as namespace from 'other';\nexport { source as publicName } from 'third';\nexport * from 'fourth';\n",
    )
    .expect("extract TypeScript source");

    let imports = extracted
        .imports
        .iter()
        .map(|import| {
            (
                import.module.as_str(),
                import.imported_symbol.as_deref(),
                import.alias.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    assert!(imports.contains(&("module", Some("default"), Some("Default"))));
    assert!(imports.contains(&("module", Some("named"), Some("local"))));
    assert!(imports.contains(&("other", Some("*"), Some("namespace"))));
    assert!(imports.contains(&("third", Some("source"), Some("publicName"))));
    assert!(imports.contains(&("fourth", Some("*"), None)));
}

#[test]
fn typescript_extractor_does_not_mark_nested_symbols_as_exported() {
    let extracted = extract_file(
        LanguageKind::TypeScript,
        "src/example.ts",
        Path::new("src/example.ts"),
        "export function outer() { function inner() {} inner(); }\n",
    )
    .expect("extract TypeScript source");
    let inner = extracted
        .nodes
        .iter()
        .find(|node| node.name == "inner")
        .expect("nested function symbol");

    assert_eq!(inner.visibility, None);
    assert!(inner.metadata_json.contains("\"exported\":false"));
}

#[test]
fn typescript_and_tsx_fixture_resolves_exact_imports_and_cross_file_calls() {
    let workspace = semantic_fixture_workspace("typescript_workspace");

    index_workspace(workspace.path()).expect("index fixture");

    let connection = graph_connection(workspace.path());
    let cross_file_edge_count = query_count(
        &connection,
        "SELECT COUNT(*)
         FROM code_graph_edges edge
         JOIN code_graph_symbols source ON source.id = edge.source_symbol_id
         JOIN code_graph_symbols target ON target.id = edge.target_symbol_id
         WHERE source.file_id <> target.file_id",
    );

    let resolution_rows = connection
        .prepare(
            "SELECT file.path, import.module, resolution.resolution
             FROM code_graph_import_resolutions resolution
             JOIN code_graph_imports import ON import.id = resolution.import_id
             JOIN code_graph_files file ON file.id = import.file_id
             ORDER BY file.path, import.module",
        )
        .expect("resolution query")
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .expect("resolution rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect resolutions");
    assert_eq!(
        resolution_rows,
        vec![
            (
                "src/Panel.tsx".to_string(),
                "./index".to_string(),
                "exact".to_string()
            ),
            (
                "src/index.ts".to_string(),
                "./public".to_string(),
                "exact".to_string()
            ),
            (
                "src/public.ts".to_string(),
                "./format".to_string(),
                "exact".to_string()
            ),
        ]
    );
    assert_eq!(cross_file_edge_count, 1);
}

#[test]
fn fallback_extractor_keeps_additional_tree_sitter_languages() {
    let extracted = extract_file(
        LanguageKind::Ruby,
        "greeter.rb",
        Path::new("greeter.rb"),
        "class Greeter\n  def call\n  end\nend\n",
    )
    .expect("extract Ruby source");

    assert!(extracted.nodes.iter().any(|node| node.name == "Greeter"));
}

#[test]
fn fallback_extractor_preserves_additional_tree_sitter_language_coverage() {
    let cases = [
        (
            LanguageKind::Css,
            "style.css",
            ".hero { color: red; }",
            "hero",
        ),
        (LanguageKind::Html, "index.html", "<div>Hello</div>", "div"),
        (
            LanguageKind::Vue,
            "App.vue",
            "<template><AppShell></AppShell></template><script>export default {}</script>",
            "template",
        ),
        (
            LanguageKind::Php,
            "greeter.php",
            "<?php class Greeter { public function call() {} }",
            "Greeter",
        ),
        (
            LanguageKind::Shell,
            "build.sh",
            "build() { echo hi; }\nNAME=value\n",
            "build",
        ),
        (
            LanguageKind::Lua,
            "greeter.lua",
            "local function greet() end\n",
            "greet",
        ),
        (
            LanguageKind::Kotlin,
            "Greeter.kt",
            "package demo\nimport kotlin.text.trim\nclass Greeter {\n  fun greet() {}\n}\nval title = \"hi\"\n",
            "Greeter",
        ),
        (
            LanguageKind::Swift,
            "Greeter.swift",
            "class Greeter { func greet() {} }\n",
            "Greeter",
        ),
        (
            LanguageKind::Yaml,
            "compose.yaml",
            "services:\n  api:\n    image: example/api\n",
            "services",
        ),
        (
            LanguageKind::Dockerfile,
            "Dockerfile",
            "FROM alpine AS base\nRUN echo hello\n",
            "alpine",
        ),
    ];

    for (language, path, source, expected_symbol) in cases {
        let extracted = extract_file(language, path, Path::new(path), source).expect(path);

        assert_eq!(extracted.parse_status, "parsed", "{path}");
        assert!(
            extracted
                .nodes
                .iter()
                .any(|node| node.name == expected_symbol),
            "{path} missing symbol {expected_symbol}: {:?}",
            extracted.nodes
        );
    }
}

#[test]
fn python_extractor_matches_the_checked_in_semantic_golden() {
    let source = include_str!("tests/fixtures/semantic_baseline/python_workspace/app.py");
    let extracted = extract_file(LanguageKind::Python, "app.py", Path::new("app.py"), source)
        .expect("extract Python fixture");

    assert_eq!(
        stable_extracted_graph_summary(&extracted),
        include_str!("tests/fixtures/extracted_graph_goldens/python.txt")
    );
}

#[test]
fn python_extractor_collects_multiline_relative_imports_from_ast_bindings() {
    let extracted = extract_file(
        LanguageKind::Python,
        "app.py",
        Path::new("app.py"),
        "from .helpers import (\n    first,\n    second as renamed,\n)\n",
    )
    .expect("extract multiline Python import");
    let imports = extracted
        .imports
        .iter()
        .map(|import| {
            (
                import.module.as_str(),
                import.imported_symbol.as_deref(),
                import.alias.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        imports,
        vec![
            (".helpers", Some("first"), None),
            (".helpers", Some("second"), Some("renamed")),
        ]
    );
}

#[test]
fn python_extractor_prefers_a_visible_local_lambda_over_an_outer_function() {
    let extracted = extract_file(
        LanguageKind::Python,
        "app.py",
        Path::new("app.py"),
        "def helper():\n    return 1\n\ndef caller():\n    helper = lambda: 2\n    return helper()\n",
    )
    .expect("extract Python shadowing source");
    let caller = extracted
        .nodes
        .iter()
        .find(|node| node.qualified_name == "caller")
        .expect("caller");
    let local_helper = extracted
        .nodes
        .iter()
        .find(|node| node.qualified_name == "caller::helper")
        .expect("local helper binding");
    let outer_helper = extracted
        .nodes
        .iter()
        .find(|node| node.qualified_name == "helper")
        .expect("outer helper");
    let target = extracted
        .edges
        .iter()
        .find_map(|edge| {
            (edge.edge_kind == "calls" && edge.source_local_key == caller.local_key).then(|| {
                let ExtractedEdgeTarget::Local(target) = &edge.target else {
                    return None;
                };
                Some(target.as_str())
            })?
        })
        .expect("local lambda call");

    assert_eq!(target, local_helper.local_key);
    assert_ne!(target, outer_helper.local_key);
}

#[test]
fn go_extractor_matches_the_checked_in_semantic_golden() {
    let source = include_str!("tests/fixtures/semantic_baseline/go_workspace/main.go");
    let extracted = extract_file(LanguageKind::Go, "main.go", Path::new("main.go"), source)
        .expect("extract Go fixture");

    assert_eq!(
        stable_extracted_graph_summary(&extracted),
        include_str!("tests/fixtures/extracted_graph_goldens/go.txt")
    );
}

#[test]
fn go_extractor_prefers_a_visible_short_var_function_literal_over_an_outer_function() {
    let extracted = extract_file(
        LanguageKind::Go,
        "main.go",
        Path::new("main.go"),
        "package demo\n\nfunc helper() {}\n\nfunc caller() {\n\thelper := func() {}\n\thelper()\n}\n",
    )
    .expect("extract Go shadowing source");
    let caller = extracted
        .nodes
        .iter()
        .find(|node| node.qualified_name == "demo::caller")
        .expect("caller");
    let local_helper = extracted
        .nodes
        .iter()
        .find(|node| node.qualified_name == "demo::caller::helper")
        .expect("local helper binding");
    let outer_helper = extracted
        .nodes
        .iter()
        .find(|node| node.qualified_name == "demo::helper")
        .expect("outer helper");
    let target = extracted
        .edges
        .iter()
        .find_map(|edge| {
            (edge.edge_kind == "calls" && edge.source_local_key == caller.local_key).then(|| {
                let ExtractedEdgeTarget::Local(target) = &edge.target else {
                    return None;
                };
                Some(target.as_str())
            })?
        })
        .expect("local function literal call");

    assert_eq!(target, local_helper.local_key);
    assert_ne!(target, outer_helper.local_key);
}

#[test]
fn python_and_go_fixtures_persist_language_specific_symbols_and_calls() {
    let python_workspace = semantic_fixture_workspace("python_workspace");
    let go_workspace = semantic_fixture_workspace("go_workspace");

    index_workspace(python_workspace.path()).expect("index Python fixture");
    index_workspace(go_workspace.path()).expect("index Go fixture");

    let python = graph_connection(python_workspace.path());
    let go = graph_connection(go_workspace.path());
    let python_format_start = python
        .query_row(
            "SELECT start_line FROM code_graph_symbols WHERE qualified_name = 'Greeter::format'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("Python method position");
    let go_package_start = go
        .query_row(
            "SELECT start_line FROM code_graph_symbols WHERE qualified_name = 'greeting'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("Go package position");

    assert_eq!(python_format_start, 4);
    assert_eq!(go_package_start, 0);
    assert_eq!(
        query_count(
            &python,
            "SELECT COUNT(*) FROM code_graph_edges WHERE edge_kind = 'calls'",
        ),
        2
    );
    assert_eq!(
        query_count(
            &go,
            "SELECT COUNT(*) FROM code_graph_edges WHERE edge_kind = 'calls'",
        ),
        2
    );
    assert_eq!(
        query_count(
            &python,
            "SELECT COUNT(*) FROM code_graph_imports WHERE module = 'toolkit.helpers' AND imported_symbol = 'helper' AND alias = 'helper_alias'",
        ),
        1
    );
    assert_eq!(
        query_count(
            &go,
            "SELECT COUNT(*) FROM code_graph_imports WHERE module = 'fmt' AND alias = 'fmt'",
        ),
        1
    );
}

#[test]
fn legacy_extraction_does_not_allocate_unpersistable_unresolved_edges() {
    let extracted = extract_file(
        LanguageKind::Ruby,
        "greeter.rb",
        Path::new("greeter.rb"),
        "class Greeter\n  def call\n    external\n  end\nend\n",
    )
    .expect("extract Ruby source");

    assert!(
        extracted
            .edges
            .iter()
            .all(|edge| !matches!(edge.target, ExtractedEdgeTarget::Unresolved { .. }))
    );
}

#[test]
#[ignore = "run with --release to measure medium and large execution-root graph indexing"]
fn semantic_fixture_release_performance_reports_medium_and_large_execution_roots() {
    let medium_workspace = semantic_fixture_workspace("performance_rust_workspace");
    run_execution_root_performance_sample("medium", medium_workspace.path());

    let large_workspace = semantic_fixture_workspace("performance_rust_workspace");
    add_large_execution_root_sources(large_workspace.path());
    run_execution_root_performance_sample("large", large_workspace.path());
}

fn run_execution_root_performance_sample(name: &str, workspace: &Path) {
    let workspace_bytes = fixture_file_bytes(workspace);
    let full_started_at = std::time::Instant::now();

    let full_report = index_workspace(workspace).expect("index performance fixture");

    assert!(
        full_report.indexed_files > 0,
        "performance fixture must be indexed"
    );
    let incremental_path = workspace.join("src/consumer.rs");
    fs::write(
        &incremental_path,
        "use crate::generated::{step_00, step_20, step_40};\n\npub fn render_batch(value: usize) -> usize {\n    step_40(step_20(step_00(value)))\n}\n\n// benchmark mutation\n",
    )
    .expect("mutate performance fixture");
    let incremental_started_at = std::time::Instant::now();
    let incremental_report = index_workspace(workspace).expect("incremental index");
    let resolver_started_at = std::time::Instant::now();
    crate::resolver::resolve_workspace_imports(workspace).expect("resolver-only pass");

    let connection = graph_connection(workspace);
    let caller_query_started_at = std::time::Instant::now();
    let caller_count = query_count(
        &connection,
        "SELECT COUNT(*)
         FROM code_graph_edges edge
         JOIN code_graph_symbols target ON target.id = edge.target_symbol_id
         WHERE target.name = 'step_20'",
    );
    let caller_query_duration = caller_query_started_at.elapsed();
    let callee_query_started_at = std::time::Instant::now();
    let callee_count = query_count(
        &connection,
        "SELECT COUNT(*)
         FROM code_graph_edges edge
         JOIN code_graph_symbols source ON source.id = edge.source_symbol_id
         WHERE source.name = 'render_batch'",
    );
    let callee_query_duration = callee_query_started_at.elapsed();

    eprintln!(
        "code_graph_release_benchmark scenario={} execution_root=temp-isolated files={} bytes={} cold_index_ms={} cold_file_prepare_us={} cold_sqlite_persistence_us={} cold_resolver_us={} incremental_ms={} incremental_indexed={} incremental_file_prepare_us={} incremental_sqlite_persistence_us={} incremental_resolver_us={} resolver_only_ms={} callers={} caller_query_ms={} callees={} callee_query_ms={}",
        name,
        full_report.scanned_files,
        workspace_bytes,
        full_started_at.elapsed().as_millis(),
        full_report.file_prepare_duration_us,
        full_report.sqlite_persistence_duration_us,
        full_report.resolver_duration_us,
        incremental_started_at.elapsed().as_millis(),
        incremental_report.indexed_files,
        incremental_report.file_prepare_duration_us,
        incremental_report.sqlite_persistence_duration_us,
        incremental_report.resolver_duration_us,
        resolver_started_at.elapsed().as_millis(),
        caller_count,
        caller_query_duration.as_millis(),
        callee_count,
        callee_query_duration.as_millis(),
    );
}

fn add_large_execution_root_sources(workspace: &Path) {
    let generated = fs::read_to_string(workspace.join("src/generated.rs"))
        .expect("read medium fixture generated source");
    for module_index in 0..32 {
        let renamed = generated.replace("step_", &format!("module_{module_index}_step_"));
        fs::write(
            workspace
                .join("src")
                .join(format!("large_module_{module_index}.rs")),
            renamed,
        )
        .expect("write large execution-root source");
    }
}

fn semantic_fixture_workspace(name: &str) -> tempfile::TempDir {
    let workspace = tempfile::tempdir().expect("fixture workspace");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("semantic_baseline")
        .join(name);

    copy_fixture_tree(&fixture, workspace.path());
    workspace
}

fn copy_fixture_tree(source: &Path, destination: &Path) {
    for entry in fs::read_dir(source).expect("read fixture directory") {
        let entry = entry.expect("fixture directory entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().expect("fixture file type");

        if file_type.is_dir() {
            fs::create_dir_all(&destination_path).expect("create fixture directory");
            copy_fixture_tree(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).expect("copy fixture file");
        }
    }
}

fn graph_connection(workspace_path: &Path) -> Connection {
    Connection::open(workspace_path.join(".foco").join("foco.sqlite")).expect("open graph database")
}

fn query_count(connection: &Connection, sql: &str) -> i64 {
    connection
        .query_row(sql, [], |row| row.get(0))
        .expect("query count")
}

fn fixture_file_bytes(path: &Path) -> u64 {
    let mut bytes = 0_u64;
    for entry in fs::read_dir(path).expect("read fixture directory") {
        let entry = entry.expect("fixture directory entry");
        let entry_path = entry.path();
        if entry.file_type().expect("fixture file type").is_dir() {
            bytes = bytes.saturating_add(fixture_file_bytes(&entry_path));
        } else {
            bytes = bytes.saturating_add(entry.metadata().expect("fixture metadata").len());
        }
    }
    bytes
}

fn stable_extracted_graph_summary(
    extracted: &crate::extractors::facts::ExtractedGraphFile,
) -> String {
    let nodes_by_key = extracted
        .nodes
        .iter()
        .map(|node| (node.local_key.as_str(), node.qualified_name.as_str()))
        .collect::<std::collections::HashMap<_, _>>();
    let mut lines = vec![format!("parse_status|{}", extracted.parse_status)];
    lines.extend(
        extracted
            .nodes
            .iter()
            .map(|node| format!("node|{}|{}", node.kind, node.qualified_name)),
    );
    lines.extend(extracted.imports.iter().map(|import| {
        format!(
            "import|{}|{}|{}",
            import.module,
            import.imported_symbol.as_deref().unwrap_or_default(),
            import.alias.as_deref().unwrap_or_default()
        )
    }));
    lines.extend(extracted.references.iter().map(|reference| {
        format!(
            "reference|{}|{}",
            reference.name,
            reference
                .target_local_key
                .as_deref()
                .and_then(|key| nodes_by_key.get(key).copied())
                .unwrap_or("unresolved")
        )
    }));
    lines.extend(extracted.edges.iter().filter_map(|edge| {
        let ExtractedEdgeTarget::Local(target) = &edge.target else {
            return None;
        };
        let source = nodes_by_key.get(edge.source_local_key.as_str())?;
        let target = nodes_by_key.get(target.as_str())?;
        (edge.edge_kind == "calls").then(|| {
            let confidence = if edge.metadata_json.contains("\"confidence\":\"exact\"") {
                "exact"
            } else {
                "heuristic"
            };
            format!("call|{source}|{target}|{confidence}")
        })
    }));
    lines.sort();
    format!("{}\n\n", lines.join("\n"))
}
