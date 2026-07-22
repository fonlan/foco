use std::{fs, path::Path};

use rusqlite::Connection;

use crate::{
    extractors::{
        LanguageKind, extract_file,
        facts::{
            ExtractedEdge, ExtractedEdgeTarget, ExtractedPosition, file_local_key, node_local_key,
        },
    },
    index_workspace,
};

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
    };

    assert!(matches!(
        edge.target,
        ExtractedEdgeTarget::Unresolved { .. }
    ));
}

#[test]
fn indexes_workspace_incrementally_and_preserves_legacy_reference_edges() {
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
            "SELECT COUNT(*) FROM code_graph_edges WHERE edge_kind = 'references'",
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
fn semantic_fixture_preserves_legacy_function_invocations_as_reference_edges() {
    let workspace = semantic_fixture_workspace("rust_workspace");

    index_workspace(workspace.path()).expect("index fixture");

    let connection = graph_connection(workspace.path());
    let relation = connection
        .query_row(
            "SELECT edge.edge_kind, edge.metadata_json
             FROM code_graph_edges edge
             JOIN code_graph_symbols source ON source.id = edge.source_symbol_id
             JOIN code_graph_symbols target ON target.id = edge.target_symbol_id
             WHERE source.name = 'result' AND target.name = 'local_helper'",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .expect("legacy render relation");

    assert_eq!(relation, ("references".to_string(), "{}".to_string()));
}

#[test]
fn semantic_fixture_preserves_same_name_legacy_mislink() {
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
        .expect("legacy same-name relation");

    assert_eq!(relation, 20);
}

#[test]
fn semantic_fixture_keeps_cross_file_imports_without_cross_file_edges() {
    let workspace = semantic_fixture_workspace("rust_workspace");

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

    assert_eq!(cross_file_edge_count, 0);
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
fn typescript_family_extractor_preserves_import_and_reexport_baseline() {
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

    assert_eq!(import, ("./index".to_string(), None, None));
    assert_eq!(reexport_import_count, 0);
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
fn typescript_and_tsx_fixture_keeps_cross_file_edges_unresolved() {
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

    assert_eq!(cross_file_edge_count, 0);
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
#[ignore = "run with --release to compare the fixed semantic graph performance fixture"]
fn semantic_fixture_performance_baseline_indexes_fixed_workspace() {
    let workspace = semantic_fixture_workspace("performance_rust_workspace");
    let started_at = std::time::Instant::now();

    let report = index_workspace(workspace.path()).expect("index performance fixture");

    assert!(
        report.indexed_files > 0,
        "performance fixture must be indexed"
    );
    eprintln!(
        "semantic graph performance fixture indexed {} files in {} ms",
        report.indexed_files,
        started_at.elapsed().as_millis()
    );
}

#[test]
fn index_workspace_shares_store_gate_with_critical_reservation() {
    use foco_store::workspace::{
        WORKSPACE_DATABASE_ORDINARY_GATE_TIMEOUT, open_workspace_database,
        open_workspace_database_critical,
    };
    use std::time::Instant;

    let workspace = tempfile::tempdir().expect("workspace");
    fs::write(workspace.path().join("lib.rs"), "fn gated_symbol() {}\n").expect("source");

    let ordinary_1 = open_workspace_database(workspace.path()).expect("ordinary 1");
    let ordinary_2 = open_workspace_database(workspace.path()).expect("ordinary 2");
    let critical = open_workspace_database_critical(workspace.path()).expect("critical");

    let workspace_path = workspace.path().to_path_buf();
    let started_at = Instant::now();
    let index_error =
        index_workspace(&workspace_path).expect_err("ordinary gate must reject index");

    assert!(started_at.elapsed() >= WORKSPACE_DATABASE_ORDINARY_GATE_TIMEOUT);
    assert!(
        index_error
            .to_string()
            .contains("workspace database concurrency limit reached")
    );

    drop(ordinary_2);
    let report = index_workspace(workspace.path()).expect("index after ordinary release");

    assert_eq!(report.indexed_files, 1);

    drop(ordinary_1);
    drop(critical);
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
