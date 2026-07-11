use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

const KNOWN_LEGACY_DIRECT_CALLS: &[(&str, usize)] = &[];

#[test]
fn audited_app_provider_calls_do_not_expand_legacy_stream_chat_usage() {
    let app_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rust_files(app_root, &mut files);
    let mut actual = BTreeSet::new();

    for path in files {
        let relative = path.strip_prefix(app_root).expect("relative path");
        if relative.starts_with("tests")
            || relative == Path::new("runtime/provider_audit.rs")
            || relative == Path::new("provider_audit_source_guard.rs")
        {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read Rust source");
        let direct_calls = source.match_indices("stream_chat(").count();
        if direct_calls > 0 {
            actual.insert((relative.to_string_lossy().replace('\\', "/"), direct_calls));
        }
    }

    let expected = KNOWN_LEGACY_DIRECT_CALLS
        .iter()
        .map(|(path, count)| ((*path).to_string(), *count))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "direct app stream_chat usage changed; audited calls must use the capture-aware helper and the inventory must be updated only for an explicit compatibility exception"
    );
}

#[test]
fn local_audited_paths_do_not_serialize_neutral_requests_as_detail_bodies() {
    let app_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in ["main.rs", "hooks.rs", "prompt/compression.rs"] {
        let source = fs::read_to_string(app_root.join(relative)).expect("read Rust source");
        assert!(
            !source.contains("serde_json::to_string(&hook_request)"),
            "{relative} must not serialize a neutral request as an audit detail body"
        );
    }

    let main = fs::read_to_string(app_root.join("main.rs")).expect("read main source");
    let helper_start = main
        .find("pub(crate) async fn audited_provider_text_request")
        .expect("audited text helper");
    let helper_source = &main[helper_start..];
    assert!(
        !helper_source.contains("serialize_provider_request("),
        "internal audited provider helpers must persist only captured provider wire requests"
    );
    assert!(
        !main.contains("fn serialize_provider_request"),
        "app runtime must not retain a neutral-request audit serialization helper"
    );

    let compression =
        fs::read_to_string(app_root.join("prompt/compression.rs")).expect("read compression");
    let compression_start = compression
        .find("async fn llm_context_compression_summary")
        .expect("compression helper");
    let compression_end = compression[compression_start..]
        .find("fn persist_context_compression_snapshot")
        .map(|offset| compression_start + offset)
        .expect("compression helper end");
    assert!(
        !compression[compression_start..compression_end].contains("serialize_provider_request("),
        "LLM context compression must persist only captured provider wire requests"
    );
}

fn collect_rust_files(root: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("read app directory") {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some(".mem") {
            continue;
        }
        if path.is_dir() {
            collect_rust_files(&path, output);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            output.push(path);
        }
    }
}
