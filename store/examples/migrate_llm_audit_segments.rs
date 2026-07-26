//! Offline migration: offload legacy `llm_requests` TEXT dumps into Zstd segments.
//!
//! Usage:
//!   cargo run -p foco-store --example migrate_llm_audit_segments -- /path/to/workspace
//!
//! Quit Foco before running. Optionally pass `--vacuum` after the batch size.

use std::{env, process, time::Instant};

use foco_store::workspace::WorkspaceDatabase;

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let vacuum = args.iter().any(|arg| arg == "--vacuum");
    args.retain(|arg| arg != "--vacuum");
    let workspace = args.first().map(String::as_str).unwrap_or(".");
    let batch = args
        .get(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(128);

    println!("workspace={workspace}");
    println!("batch_size={batch}");
    println!("vacuum={vacuum}");

    let started = Instant::now();
    let mut database = match WorkspaceDatabase::open_or_create_ungated(workspace) {
        Ok(database) => database,
        Err(error) => {
            eprintln!("open failed: {error}");
            process::exit(1);
        }
    };
    println!(
        "schema_version={}",
        database.schema_version().unwrap_or_default()
    );

    let mut total = 0usize;
    loop {
        match database.migrate_llm_audit_details_to_segments_batch(batch) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                if total % (batch * 4) == 0 || n < batch {
                    println!("migrated_rows={total} (+{n}) elapsed={:?}", started.elapsed());
                }
            }
            Err(error) => {
                eprintln!("migration failed after {total} rows: {error}");
                process::exit(1);
            }
        }
    }
    println!(
        "migration complete: rows={total} elapsed={:?}",
        started.elapsed()
    );

    if vacuum {
        println!("running VACUUM (may take several minutes)...");
        let vacuum_started = Instant::now();
        if let Err(error) = database.vacuum() {
            eprintln!("VACUUM failed: {error}");
            process::exit(1);
        }
        println!("VACUUM done in {:?}", vacuum_started.elapsed());
    }

    if let Ok(stats) = database.space_stats() {
        let used = stats.page_size_bytes.saturating_mul(stats.page_count);
        let free = stats.page_size_bytes.saturating_mul(stats.freelist_count);
        println!(
            "sqlite pages={} page_size={} used_bytes≈{} freelist_bytes≈{}",
            stats.page_count, stats.page_size_bytes, used, free
        );
    }
}
