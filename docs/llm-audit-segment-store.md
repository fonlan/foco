# LLM Audit Segment Store

## Why

Workspace SQLite (`.foco/foco.sqlite`) was storing full provider wire dumps in
`llm_requests.request_body_json` / `response_body_json`. On active workspaces this
grew to multi‑GB TEXT payloads inside a single database that also holds chats,
plans, tool calls, code graph, and run events.

Observed failure modes on this repo’s local DB:

| Symptom | Likely driver |
| --- | --- |
| `database disk image is malformed (11)` | Large page churn + concurrent Immediate writers + multi‑GB B-tree |
| `file is not a database (26)` / zeroed header | Severe corruption or interrupted write of page 0 after extreme growth |
| Repeated `*.corrupt-*.bak` / recovery rebuilds | Same root pressure: audit dumps + run_events + graph in one file |

Contributing factors (not mutually exclusive):

1. **Multi‑GB TEXT in SQLite** — request dumps alone were ~4.5 GB / ~11k rows (avg ~390 KB, max ~1.7 MB). Every insert/update rewrites large overflow pages.
2. **Shared hot database** — chat run events, plan polling, stats, and audit writes all open the same workspace DB (see request-storm boundary notes).
3. **WAL + large transactions** — Immediate transactions with huge payloads amplify checkpoint and lock pressure.
4. **VACUUM / migration backups** — `VACUUM INTO` of multi‑GB DBs is slow and disk-heavy; partial failure leaves awkward states.
5. **Process kill / crash mid-write** — more likely to damage a huge single file than small append-only segments.

## Design (schema v49+)

| Layer | Role |
| --- | --- |
| **SQLite** | Event index, structured metrics (`llm_requests` columns), pre-aggregated `llm_request_usage_rollups`, segment **locators**, `transport` |
| **Segment files** | Append-only Zstd records under `.foco/llm-audit/segments/seg-*.focoaud` |
| **API** | Unchanged: `LlmRequestRecord.request_body_json` / `response_body_json` are hydrated on read |

### Segment record

- File magic `FOCOAUD1`
- Each record: magic `FDAT`, kind (request/response), uncompressed/compressed lengths, SHA-256, Zstd payload
- Append + `fsync` **before** SQLite locator commit (orphan bytes OK; missing bytes not OK)
- Rotate active segment at 256 MiB

### Write path

1. Normalize/redact v1 dump
2. Append to segment → locator
3. Insert/update SQLite with locator columns; **TEXT dump columns stay NULL**
4. Update usage rollups as before

### Read path

1. Load structured row + locators
2. If locator present, decompress from segment
3. Else fall back to legacy TEXT (pre-migration rows)

### Maintenance

`WorkspaceDatabase::run_pending_one_time_maintenance` offloads a batch of legacy
TEXT rows per tick (`migrate_llm_audit_details_to_segments_batch`). Multi‑GB
workspaces need many ticks (or call the batch API in a loop offline).

Retention (`prune_llm_request_details_before`) clears TEXT **and** locators.
Segment bytes are append-only; GC of unreferenced ranges is not implemented yet.

## Operational notes

- Keep `save_request_response_details` and retention days configured; dumps are still sensitive.
- After upgrading, expect `.foco/llm-audit/segments/` to grow while SQLite shrinks only after migration + optional `VACUUM`.
- Do not delete segment files while locators still reference them.
