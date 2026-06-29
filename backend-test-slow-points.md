# Backend test slow point notes

Phase 2 records the smallest useful timing pass before changing backend tests.
The measurements below were taken on Windows in the isolated worktree with a
warm Cargo test build unless noted otherwise. Rust's `--report-time` and
`--format=json` test harness flags were rejected by the stable toolchain because
they require nightly `-Z unstable-options`, so per-test timings use a small
PowerShell loop around exact Cargo test filters.

## Commands used

```powershell
cargo test -p foco-store --test workspace_database -- --list
cargo test -p foco-app -- --list
cargo test -p foco-store --test workspace_database
cargo test -p foco-store --test workspace_database -- --test-threads=1
cargo test -p foco-app
cargo test -p foco-app tests::lazy_code_graph_initialization_indexes_workspace_once -- --exact --nocapture
```

The first cold `foco-store` list/build pass compiled and linked the store test
target in about 1m18s. Later runs were warm-build runs, with Cargo reporting
about 0.3s to 0.6s before test execution. Treat the cold compile separately from
test execution.

## Test binary timing

Sorted by observed warm execution time:

| Test target | Warm execution result | Notes |
| --- | ---: | --- |
| `foco-app` unit target (`app/main.rs`, includes `app/tests/mod.rs`) | 12.53s, failed | 263 passed, 2 failed. The target is large and mixes app unit tests with the `tests::` module from `app/tests/mod.rs`. |
| `foco-store --test workspace_database` default parallel run | 6.00s, failed | 70 passed, 7 failed. Failures are migration fixtures, not ignored tests. |
| `foco-store --test workspace_database -- --test-threads=1` | 10.94s, failed | 70 passed, 7 failed. Slower but exposes the same migration failures. |

Default backend scripts remain:

```json
"test:backend": "cargo test --workspace -j 4 -- --test-threads=4",
"test:backend:calm": "cargo test --workspace -j 2 -- --test-threads=2"
```

`app/Cargo.toml` still has `autotests = false`, so `app/tests/mod.rs` is not a
separate integration-test binary. It is compiled into the app unit test target
and runs under names like `tests::lazy_code_graph_initialization_indexes_workspace_once`.

Only `tools/lib.rs` currently contains `#[ignore]` tests. The default Rust test
harness does not run ignored tests unless `--ignored` or `--include-ignored` is
passed, and the backend npm scripts do not pass those flags.

## Sampled slow candidates

Per-test wall-clock samples include Cargo invocation and test binary startup, so
use them as a ranking signal, not as exact in-test duration. A direct nocapture
rerun of `tests::lazy_code_graph_initialization_indexes_workspace_once` reported
0.27s in-test time after 0.55s warm Cargo overhead.

### `app/tests/mod.rs`

Top candidates worth changing first:

| Candidate | Sample wall time | Classification | Why it is worth moving |
| --- | ---: | --- | --- |
| `tests::lazy_code_graph_initialization_indexes_workspace_once` | 1962ms | Fixed wait + file system + code graph init | Polls watcher count up to 250 times at 20ms and then sleeps another 20ms. Replace polling with a completion signal from code graph initialization or expose a join handle in tests. |
| `tests::scheduled_task_dispatch_queues_visible_chat_and_completes_one_shot` | 1079ms | Background scheduler/DB fixture | Creates app state and exercises dispatch path; likely pays repeated SQLite setup plus scheduler state transitions. |
| `tests::team_chat_task_sse_stays_open_during_interrupted_wait_recovery` | 1054ms | SSE/background wait | Uses streaming body timing; inspect whether a state notification can replace bounded waits. |
| `tests::team_chat_task_sse_stays_open_while_coordinator_task_is_waiting` | 996ms | SSE/background wait | Same SSE wait family as above. Candidate for shared helper or event-driven completion. |
| `tests::scheduled_task_run_cancel_signals_running_active_run` | 948ms | Background cancellation path | Recreates scheduler/run fixture; check for avoidable sleeps and repeated DB setup. |
| `tests::auto_memory_dream_scheduler_runs_due_jobs_without_scheduled_task_rows` | 941ms | Background scheduler/DB fixture | Scheduler path with persistent setup; likely SQLite-heavy rather than CPU-heavy. |
| `tests::background_code_graph_initialization_indexes_workspace_and_keeps_watcher` | 857ms wall, 0.51s in-test when run directly | File system + SQLite + code graph init | Spawns and joins real background indexer, writes temp source, opens workspace DB, retains watcher. |
| `tests::startup_code_graph_initialization_selects_recently_active_workspaces` | 854ms | SQLite/file system | Opens two workspace DBs just to classify active vs inactive workspaces. |

Fixed waits found in `app/tests/mod.rs`:

- Lines 468, 491, 547, and 609 use `tokio::time::sleep(20ms)` to assert lock waiters are blocked. `ToolResourceLockRegistry` already uses `tokio::sync::Notify`; these tests can probably use an explicit test-side readiness notification from the spawned waiter before asserting it is blocked.
- Lines 996 and 1014 use `std::thread::sleep(20ms)` in lazy code graph initialization. The production function currently spawns and detaches a thread, so tests cannot await completion without polling. A test-only join handle or completion channel would remove the polling ceiling.
- Lines 6950, 7023, 7144 and related SSE tests use 200ms timeouts around body/stream reads. These are bounded waits, not unconditional sleeps, but they are still likely contributors when repeated.
- `remove_dir_if_exists` backs off up to 10 times, sleeping `20ms * attempt`, and can hide Windows file-handle cleanup delays.

The full `foco-app` run also failed in two places during this pass:

- `tests::agent_definition_api_manages_revision_validates_tools_and_hides_secrets` expected one agent definition but saw two at `app/tests/mod.rs:1848`. This looks like shared/default fixture state, not a slow-test issue.
- `tests::prepare_prompt_context_injects_existing_todo_graph_for_followup_run` failed deleting a temp workspace at `app/tests/mod.rs:12328` with Windows error 32. This is consistent with retained file handles/watchers and should be considered when changing code graph initialization tests.

### `store/tests/workspace_database.rs`

Top candidates worth changing first:

| Candidate | Sample wall time | Classification | Why it is worth moving |
| --- | ---: | --- | --- |
| `workspace_connections_wait_for_concurrent_writer_lock` | included in full run; explicit 100ms sleep | Fixed wait + SQLite writer lock | Holds a WAL writer lock and sleeps 100ms to assert another writer blocks. Replace with a channel from the writer thread once it reaches the write attempt, then release the lock. |
| `migrates_v9_without_creating_teams_for_existing_chats` | single-run failed; creates 501 chats | SQLite migration + backup + bulk fixture | Bulk insert plus migration/backup makes it a real DB hotspot. It currently fails when run now because latest migration expects newer tables. |
| `migrates_v14_scheduled_task_tables_without_losing_existing_data` | single-run failed | SQLite migration + backup | Manual historical schema does not include `memory_extraction_jobs`, and migration to latest fails. Needs fixture repair before meaningful timing. |
| `migrates_v17_workspace_spec_tables_and_creates_backup` | single-run/full-run failed | SQLite migration + backup | Creates old schema and verifies backup. Also fails on missing `memory_extraction_jobs`. |
| `migrates_v16_memory_references_table` | single-run/full-run failed | SQLite migration + schema fixture | Fails on missing `main.chats`, which points to an under-specified historical schema fixture. |
| `vacuum_reclaims_workspace_database_freelist_pages` | full run passed | SQLite/file-system hotspot | Writes a 1 MiB request body, prunes, then VACUUMs. This is intentionally I/O-heavy and should stay isolated. |
| `backs_up_existing_database_before_migration` and `failed_agent_schema_migration_rolls_back_and_preserves_backup` | full run passed | Backup/file-system hotspot | Exercise physical backup creation. Useful but naturally file-system bound. |

The store migration failures are currently the most important finding. Both
default parallel and single-thread runs fail the same seven tests:

- `migrates_v13_agent_message_foreign_keys_to_current_table`
- `migrates_v14_scheduled_task_tables_without_losing_existing_data`
- `migrates_v15_memory_dream_tables`
- `migrates_v16_memory_references_table`
- `migrates_v17_workspace_spec_tables_and_creates_backup`
- `migrates_v7_task_graphs_table_to_todo_graphs`
- `migrates_v9_without_creating_teams_for_existing_chats`

Most fail with `no such table: memory_extraction_jobs`; `migrates_v16_memory_references_table`
fails with `no such table: main.chats`. These tests need schema fixture updates before
their timings can be trusted. Until then, optimizing them for speed is premature.

## Cause buckets

- Compile/link: cold `foco-store` test target build was about 1m18s. Warm Cargo
  overhead is subsecond and should be separated from test runtime.
- Test execution: warm `foco-app` execution reached 12.53s before failing;
  warm `workspace_database` reached 6.00s default parallel and 10.94s single-thread.
- Fixed waits: app lock tests use repeated 20ms sleeps; store writer-lock test uses
  a hard 100ms sleep; code graph lazy initialization can poll for up to 5s.
- SQLite/file-system: store migration/backup/vacuum tests and app code graph tests
  repeatedly create temp workspaces, SQLite databases, watchers, and backup files.
- Event/status notification opportunities: `ToolResourceLockRegistry` already has
  an internal `Notify`; test waiters could signal when they have reached the blocked
  acquisition. Code graph lazy initialization lacks a join/completion surface, so a
  test-only handle or channel would remove polling without changing product behavior.
