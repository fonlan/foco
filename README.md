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
`%USERPROFILE%\.foco\workspace`, and writes daily logs to
`%USERPROFILE%\.foco\logs\foco-YYYY-MM-DD.log`.

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
