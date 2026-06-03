# Foco

Foco is a local coding-agent workspace with a Rust backend and a React frontend.

## Local Development

Install frontend dependencies once:

```bash
npm install
```

Run the backend. This builds the frontend once, then serves it from the Rust
HTTP server:

```bash
npm run backend
```

The backend binds to `127.0.0.1:3210` by default. Set `FOCO_PORT` to use a
different local port.

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

For live frontend development, run the Vite dev server alongside the backend:

```bash
npm run frontend
```

Run verification:

```bash
cargo check --workspace
npm run typecheck
```

## Release Build

Build the frontend assets and the optimized Rust workspace:

```bash
npm run build:release
```
