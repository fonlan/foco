# Foco

Foco is a local coding-agent workspace with a Rust backend and a React frontend.

## Local Development

Install frontend dependencies once:

```bash
npm install
```

Run the backend:

```bash
npm run backend
```

Run the frontend:

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
