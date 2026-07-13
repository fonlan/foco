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

#[test]
fn production_audit_writers_do_not_store_business_completion_or_neutral_request_dumps() {
    let app_root = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Sidecar mirror must keep detail columns empty (structured metrics only).
    let remote = fs::read_to_string(app_root.join("remote_workspace.rs")).expect("remote source");
    let persist_start = remote
        .find("fn persist_sidecar_llm_audit_for_kind(")
        .expect("sidecar persist helper");
    let persist_end = remote[persist_start..]
        .find("\nfn neutral_role_for_message")
        .map(|offset| persist_start + offset)
        .expect("sidecar persist helper end");
    let persist_source = &remote[persist_start..persist_end];
    assert!(
        persist_source.contains("request_body_json: None")
            && persist_source.contains("response_body_json: None"),
        "sidecar mirror audit must keep request/response detail NULL"
    );
    assert!(
        !persist_source.contains("serde_json::to_string(&request)")
            && !persist_source.contains("serde_json::to_string(request)"),
        "sidecar must not serialize NeutralChatRequest into audit detail columns"
    );
    assert!(
        !persist_source.contains("providerCompletions") && !persist_source.contains(r#""text":"#),
        "sidecar must not write normalized completion payloads into audit detail columns"
    );

    // Local cancel/finish path must not reintroduce compact cancelled JSON as detail.
    let main = fs::read_to_string(app_root.join("main.rs")).expect("main source");
    let cancel_start = main
        .find("fn cancelled_audit_outcome(")
        .expect("cancelled audit outcome");
    let cancel_end = main[cancel_start..]
        .find("\nfn chat_run_was_cancelled")
        .map(|offset| cancel_start + offset)
        .expect("cancelled audit outcome end");
    let cancel_source = &main[cancel_start..cancel_end];
    assert!(
        cancel_source.contains("response_body_json: None"),
        "cancelled audit outcome must leave response detail NULL"
    );
    assert!(
        !cancel_source.contains("response_body_json: Some")
            && !cancel_source.contains("compact_cancelled_audit_response"),
        "cancelled audit must not assign compact cancelled JSON into response_body_json"
    );

    // Capture-aware production helpers must not fall back to direct stream_chat.
    for relative in ["main.rs", "hooks.rs", "prompt/compression.rs"] {
        let source = fs::read_to_string(app_root.join(relative)).expect("read production source");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production section");
        assert!(
            !production.contains("stream_chat("),
            "{relative} must not call stream_chat directly"
        );
    }
    let remote = fs::read_to_string(app_root.join("remote_workspace.rs")).expect("remote source");
    let remote_production = remote
        .split("#[cfg(test)]")
        .next()
        .expect("remote production");
    assert!(
        remote_production.contains("stream_chat_with_capture_observer"),
        "remote broker must use capture-aware stream helper"
    );
    assert_eq!(
        remote_production.matches("stream_chat(").count(),
        0,
        "remote production must not call bare stream_chat("
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
