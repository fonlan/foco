# Foco Request Storm SQLite Boundary

Date: 2026-07-03

Scope: Phase 1 is read-only boundary finding for the Plan/Auto-run/Statistics request storm. No production code changes in this phase.

ponytail: this is a static source trace, not a browser trace. It can prove where duplicate calls can originate and where every backend request opens SQLite, but it does not measure real machine limits. Upgrade path is a short browser recovery trace plus backend open-attempt counters.

## Frontend Refresh Triggers

### `loadActivePlans(workspaceId)`

Defined in `web/App.tsx:1906`. It always issues `GET /api/workspaces/{workspaceId}/plans?view=active&limit=50`, then updates active plan state. It does not single-flight or abort an in-flight request.

Callers and timers:

- `handlePlanRefresh` at `web/App.tsx:1963`: SSE `planRefresh`/`plan_refresh` opens the Plan tab and calls `loadActivePlans`.
- Plan actions at `web/App.tsx:1976`, phase retry at `web/App.tsx:2017`, delete at `web/App.tsx:2077`, and worktree cleanup at `web/App.tsx:2111`: mutate a plan, then call `loadActivePlans`.
- Plan panel visibility effect at `web/App.tsx:2610`: opening the right panel on the Plan tab, or changing active workspace while it is open, calls `loadActivePlans` immediately.
- Auto-run enabled interval at `web/App.tsx:2669`: every `PLAN_AUTO_RUN_REFRESH_MS` (3000ms), calls both `loadPlanAutoRunState` and `loadActivePlans`.
- Running/in-flight plan interval at `web/App.tsx:2686`: when auto-run is enabled or the Plan panel is visible, and any active plan is in flight, another 3000ms interval calls `loadActivePlans`.
- Pending phase retry interval at `web/App.tsx:2714`: while retry refresh is pending, a separate interval calls `loadActivePlans` until the phase stops running.

Current risk path: enable auto-run while the Plan panel is open and a plan/phase is running. The auto-run interval and running-plan interval both call `loadActivePlans` every 3s, and the Plan panel open/SSE refresh/action callbacks can overlap those ticks. There is no request coalescing by workspace/view key.

### `loadPlanAutoRunState(workspaceId)`

Defined in `web/App.tsx:943`. It issues `GET /api/workspaces/{workspaceId}/plans/auto-run` and stores the state. Errors are written to `activePlansError`; they do not directly retry.

Callers and timers:

- Initial state effect at `web/App.tsx:2655`: when the active workspace has no cached auto-run state, load once.
- Auto-run enabled interval at `web/App.tsx:2669`: every 3000ms while auto-run is enabled.
- The checkbox setter `setPlanAutoRunEnabledForWorkspace` at `web/App.tsx:964` uses `PUT /plans/auto-run`, then updates the same local state.

Current risk path: after auto-run is enabled, the GET `/plans/auto-run` poll and GET `/plans` poll are launched together every 3s. On a stalled browser returning to life, queued timer ticks can stack with Plan panel refreshes.

### `loadChatStatistics(workspaceId, chatId)`

Defined in `web/App.tsx:2378`. It issues `GET /api/workspaces/{workspaceId}/chats/{chatId}/statistics`. It does not single-flight or abort in-flight requests.

Callers:

- Stats panel visibility effect at `web/App.tsx:2563`: opening the Stats tab for a non-pending active chat calls `loadChatStatistics` immediately.
- Main active-run stream handler: `complete` at `web/App.tsx:7221`, `gitDiffRefresh` at `web/App.tsx:7382`, and `memoryExtractionComplete` at `web/App.tsx:7423` each call `loadChatStatistics`.
- Queued/recovered run stream handler: `complete` at `web/App.tsx:8227`, `gitDiffRefresh` at `web/App.tsx:8397`, and `memoryExtractionComplete` at `web/App.tsx:8440` each call `loadChatStatistics`.

Current risk path: while a chat run is active and the Stats tab is open, stream events can trigger stats refreshes close together. On page visibility recovery, `recoverActiveChatStreams` (`web/App.tsx:3805`) resubscribes active streams; replayed `complete`/`gitDiffRefresh`/`memoryExtractionComplete` events can re-trigger stats calls. There is no periodic stats interval in the current file, but event replay can still burst.

## Backend DB Open Paths

Routes are wired in `app/http/router.rs`: `/api/workspaces/{workspace_id}/plans` at `app/http/router.rs:308`, `/plans/auto-run` at `app/http/router.rs:312`, and `/chats/{chat_id}/statistics` at `app/http/router.rs:485`.

- `GET /plans`: `app/http/plans.rs:253` resolves the workspace and calls `WorkspaceDatabase::open_or_create(&workspace.path)` at `app/http/plans.rs:272` on every request, then `database.plans(...)`.
- `GET /plans/auto-run`: `app/http/plans.rs:326` calls `crate::plan_auto_run::plan_auto_run_state`, which opens the workspace DB at `app/plan_auto_run.rs:165` on every request.
- `PUT /plans/auto-run`: `app/http/plans.rs:340` calls `set_plan_auto_run_enabled`, which opens the workspace DB at `app/plan_auto_run.rs:176` on every request.
- `GET /chats/{chat_id}/statistics`: `app/http/chat.rs:1842` opens the workspace DB at `app/http/chat.rs:1850` on every request, runs several chat/statistics queries, then also opens workspace/global memory DBs at `app/http/chat.rs:1898` and `app/http/chat.rs:1909`.

`WorkspaceDatabase::open_or_create` is at `store/workspace.rs:624`. Each call validates the workspace directory, creates `.foco`, constructs `.foco/foco.sqlite`, opens a fresh `rusqlite::Connection` via `open_connection` (`store/workspace.rs:11997`), and runs migrations (`store/workspace.rs:641`). There is no shared connection or per-workspace open gate here.

Current SQLite diagnostics are too thin: `WorkspaceDatabaseError::Sqlite` stores `rusqlite::Error`, but its `Display` at `store/workspace.rs:10830` only emits `{path} SQLite error: {source}`. The `open_connection` error mapping at `store/workspace.rs:11999` does not log or expose SQLite extended code or OS errno separately, so `unable to open database file` loses the reason needed to distinguish path, permission, fd exhaustion, and other IO cases.

## Chat Run Cancellation Boundary

The run event persistence path also opens the workspace DB per event. `ActiveChatRunRegistration::record_event` at `app/runtime/subscriptions.rs:384` calls `WorkspaceDatabase::open_or_create(workspace_path)` at `app/runtime/subscriptions.rs:400`, inserts a run event, and may persist assistant/tool draft state in the same request.

For background agent runs, `run_chat_context_in_background` passes `record_event` as the event delivery callback at `app/runtime/chat_run.rs:61`. If recording fails, the callback stores `delivery_error` and returns an error string. After the executor returns, `app/runtime/chat_run.rs:72` logs `failed to record chat run event`, calls `cancellation.cancel()` at `app/runtime/chat_run.rs:78`, and tries to record a final error event at `app/runtime/chat_run.rs:80`.

There are also direct persistence-failure exits inside the stream path. For example, if `persist_running_llm_request` fails after opening the workspace DB (`app/prompt/compression.rs:1588`), the SSE stream yields `ChatSseEvent::Error` and returns at `app/main.rs:2721`. Final result persistence uses `persist_chat_result` (`app/prompt/compression.rs:1446`), which opens the workspace DB at `app/prompt/compression.rs:1455` and propagates errors to the stream as error events.

## Minimal Repro/Risk Matrix

| Interface | Minimal current trigger | Request storm risk |
| --- | --- | --- |
| `GET /plans` | Open Plan tab | Auto-run enabled + Plan tab open + in-flight plan creates overlapping 3s polls; SSE plan refresh/action callbacks can land during a tick. |
| `GET /plans/auto-run` | Active workspace first loads state | Auto-run enabled polls every 3s and launches alongside `GET /plans`. |
| `GET /chats/{chat_id}/statistics` | Open Stats tab for a chat | Running/recovered stream events can replay several stats-refresh events close together; no in-flight coalescing. |
| chat run event writes | Any streamed run event | Every event opens workspace DB; failure bubbles through the event callback and cancels/ends the run path. |

## Protection Points For Later Phases

The smallest high-leverage frontend point is to coalesce by request key around `loadActivePlans`, `loadPlanAutoRunState`, and `loadChatStatistics`, because the duplicate triggers are all funneled through those three callbacks.

The smallest high-leverage backend point is around `WorkspaceDatabase::open_or_create` or a thin app-level wrapper for workspace DB access, because `/plans`, `/plans/auto-run`, `/statistics`, and run-event persistence all converge there. Improving `WorkspaceDatabaseError::Sqlite` formatting/logging would make the next failure actionable without changing API shape.
