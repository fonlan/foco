# Background `run_command` contract

`run_command` can start a bounded, managed background command without changing its existing foreground behavior. This is a tool-runtime contract shared by the local app process and each SSH sidecar; it is deliberately not a Terminal replacement.

## Tools and JSON shape

### `run_command`

The existing strict input gains these fields:

```json
{
  "command": "npm",
  "args": ["run", "frontend"],
  "cwd": "web",
  "timeoutMs": null,
  "background": true,
  "backgroundTimeoutMs": 3600000
}
```

- `background: null` or `false` preserves foreground execution and its existing combined stdout/stderr response contract.
- `background: true` starts a managed process and promptly returns a structured command snapshot. The response includes `processId`, `pid`, `status`, lifecycle timestamps, `exitCode`/`success` where known, `terminationReason` where applicable, output bounds, `chunks`, and `nextCursor`.
- `backgroundTimeoutMs` is a maximum process lifetime in milliseconds. It must be positive when supplied. `null` leaves lifetime unbounded until another cleanup rule applies.
- A successful background start is not proof that the process will continue running. Callers must use the returned `processId` and snapshot `status`.

### `get_command_output`

```json
{
  "processId": "process-...",
  "cursor": 42,
  "waitMs": 1000,
  "timeoutMs": 10000
}
```

This is a read-only, retry-safe, non-consuming incremental read. The response contains `processId`, process state, `fromCursor`, `availableFromCursor`, `nextCursor`, `cursorExpired`, `hasMore`, response-level `truncated`, output-retention metadata, and ordered `chunks`:

```json
{
  "cursor": 43,
  "stream": "stdout",
  "text": "ready\n"
}
```

Pass `nextCursor` as the next `cursor` so previously observed log chunks are not repeated in model context. `cursor: null` reads from the earliest currently retained chunk. `waitMs` is a bounded long-poll: it returns early when output arrives or the process reaches a terminal state.

`cursorExpired: true` means requested earlier output has been evicted from the bounded in-memory buffer. The response still starts at `availableFromCursor`; callers should proceed from `nextCursor`, not retry the expired range. `outputTruncated: true` likewise only means the process ring buffer has already evicted older output.

When a complete-chunk prefix is returned because the response reached the shared 50 KiB / 2,000-line budget, the response has `hasMore: true`, `truncated: true`, and a `note`. This is an explicit successful pagination, not data silently discarded: reuse the same `processId` with `cursor: nextCursor` to retrieve the next complete chunk without repeats. Response-level `truncated` is independent of `outputTruncated` and `cursorExpired`.

### `stop_command`

```json
{
  "processId": "process-...",
  "timeoutMs": 10000
}
```

This synchronously requests idempotent managed termination of the entire process tree and waits for the monitor to observe process exit and drain stdout/stderr pipes. A successful structured result is always terminal; it never has `status: "running"`. A naturally completed command is returned idempotently with its existing terminal state. `timeoutMs` is the maximum wait budget: exceeding it is a tool error, not a successful running snapshot. The result does not replay historic logs. Use `get_command_output` afterwards when retained logs are needed.

Unknown, expired, or cross-workspace handles fail with the same stable "managed command was not found" error.

## State machine and ownership

A command snapshot has one of:

```text
Running → Exited
Running → Stopped
Running → TimedOut
start failure → Failed
```

`Stopped` is an explicit managed stop; `TimedOut` is `backgroundTimeoutMs`; `Failed` is a start/monitoring failure. `terminationReason` distinguishes explicit stop, timeout, and normal host shutdown when available. Native process-group / Job Object handling targets the whole tree, not only the direct child PID.

Registries are in-memory and scoped to their host:

- The local Foco `AppState` owns local workspace commands.
- Every `RemoteSidecarState` owns only commands it launched on that remote host.
- Remote command tools are `SidecarLocal`; process handles and log buffers are never mirrored through the main process or stored in SQLite.

Ownership cleanup terminates running commands when their owning chat or workspace is deleted, when a sidecar closes, or when the host shuts down. A shared local registry checks both workspace and chat ownership before cleanup.

## Bounds, retention, and model context

The registry applies hard limits to active commands and per-command output buffering. stdout/stderr chunks retain stream ordering and cursors, but only for a bounded in-memory window. Terminal records are retained only briefly and then removed through bounded cleanup.

All externally returned chunks are additionally constrained by the normal tool-output envelope budget. Pagination via `hasMore`/`nextCursor` preserves a complete retained range without emitting an oversized tool result.

Runtime tool-state compression preserves structured command continuity fields such as `processId`, `pid`, `status`, `exitCode`, `terminationReason`, `fromCursor`, `nextCursor`, `cursorExpired`, and `hasMore`. It summarizes chunk metadata rather than repeatedly injecting complete historical command logs into later model turns.

## Terminal boundary and non-goals

Managed background commands are non-interactive process executions. They are not a PTY, cannot accept stdin writes, do not share or resume an interactive Terminal session, and do not provide a shell-string execution mode.

They do not persist across Foco or sidecar restart, have no SQLite schema migration, no background-task management UI, no new SSE event type, and no cross-host process lookup. Models must explicitly call `stop_command` for long-lived processes that are no longer needed.
