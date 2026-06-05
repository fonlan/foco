# Foco

Foco is a local coding-agent workspace with a Rust backend and a React frontend.

## Local Development

Install frontend dependencies once:

```bash
npm install
```

Run the backend. This builds `web/dist` once so the Rust server can start, then
watches Rust backend files and restarts `cargo run -p foco-app` when they
change:

```bash
npm run backend
```

The backend binds to `127.0.0.1:3210` by default. The settings page General tab
persists the web service listen host, port, and UI language to
`%USERPROFILE%\.foco\config.json`. Restart the backend after changing host or
port; language changes apply immediately in the current browser UI. Set
`FOCO_HOST` or `FOCO_PORT` for a one-off startup override.

On first startup, Foco creates `%USERPROFILE%\.foco`, writes
`%USERPROFILE%\.foco\config.json`, registers a `Default Workspace` at
`%USERPROFILE%\.foco\workspace`, initializes the workspace database at
`%USERPROFILE%\.foco\workspace\.foco\foco.sqlite`, and writes daily logs to
`%USERPROFILE%\.foco\logs\foco-YYYY-MM-DD.log`.

The browser UI starts as a three-column product shell. The left sidebar reads
registered workspaces from `GET /api/workspaces`, can create a new workspace
directory with `POST /api/workspaces/create`, and can add an existing directory
with `POST /api/workspaces/add`. Both workspace write APIs update
`%USERPROFILE%\.foco\config.json` and initialize the workspace-local SQLite
database before returning the refreshed workspace list.

Settings can refresh model metadata from `https://models.dev/api.json` through
`POST /api/model-metadata/refresh`. Foco normalizes the fetched model metadata
and caches it at `%USERPROFILE%\.foco\models.dev.json`, including per-model
source URL, refresh time, limits, pricing, modalities, tool support, and cache
support. The settings page also saves manually filled model limits through
`POST /api/models/manual`; enabled models must have both context window and max
output tokens, or config validation fails with an explicit error.

Provider settings are managed from the same settings page and persisted in
`%USERPROFILE%\.foco\config.json`. `POST /api/providers/manual` saves OpenAI
Chat and OpenAI Responses provider configs, `POST /api/providers/test` checks
the selected provider through `genai`, and `POST /api/models/manual` also saves
model-provider associations, the active provider, and the model thinking level.
Capability warnings are shown without silently changing saved choices.

Workspace databases store LLM audit records in `llm_requests` and streamed audit
events in `llm_request_events`. Audit inserts require request time, workspace,
provider, model, status/final state, latency fields, normalized token usage, and
non-secret request/response JSON. Authorization headers and API-key fields are
redacted before persistence, and cache ratio is calculated from normalized input
and cache-read token counts.

The top-level Stats view reads `GET /api/ai-statistics` to show recorded LLM
requests across registered workspaces. It can filter by workspace, chat,
provider, model, status, and time range, and uses `page` and `pageSize` for
server-side pagination with total counts. Request details come from `GET
/api/workspaces/{workspace_id}/ai-statistics/{request_id}` and show the stored
request/response JSON.

The chat panel sends real model requests through
`POST /api/workspaces/{workspace_id}/chat/stream`, which returns server-sent
events. The backend builds provider-neutral `genai` chat requests, supports both
OpenAI Chat and OpenAI Responses providers, streams text deltas into the current
assistant bubble, persists user and assistant messages in the workspace
database, and writes real LLM audit records for the request. The send box lists
enabled models with complete limits and an active provider, plus a thinking
level selector.

For live frontend development, run the Vite dev server alongside the backend.
Frontend edits use Vite HMR, and backend Rust/Cargo edits trigger a browser full
reload while `npm run backend` restarts the Rust server:

```bash
npm run frontend
```

On Windows release builds, double-clicking `foco.exe` starts the local HTTP
server in the background and keeps Foco in the system tray. Use the tray menu to
open the local web UI in the browser or quit Foco cleanly.

Run verification:

```bash
cargo check --workspace
npm run test -w web
npm run typecheck
```

Run the Windows release startup smoke test from Windows, not WSL:

```powershell
npm run test:release-smoke:windows
```

## Release Build

Build the frontend assets and the optimized Foco executable:

```bash
npm run build:release
```

The release build embeds `web/dist` into the Rust executable. On Windows, the
binary uses a tray entry point and `windows_subsystem = "windows"` so release
startup does not open a GUI window.
