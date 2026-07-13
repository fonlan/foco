# SSH Remote Workspaces v4 Validation and E2E Notes

This phase covers pure-Rust SSH client validation. Automated tests use an **in-process russh server fixture** (no system `sshd`/`ssh`). Real remote hosts remain a manual release checklist.

## Automated Checks

Run these from the repo root:

```bash
# In-process SSH transport E2E (password/key/host-key/exec/stdin/direct-tcpip/ProxyJump boundary)
cargo test -p foco-app -- e2e_server

# SSH client unit + source guards (no system OpenSSH spawn in production)
cargo test -p foco-app -- ssh_client

# Config / API auth + redaction
cargo test -p foco-store -- remote_server
cargo test -p foco-app -- remote_servers

# Existing remote workspace / sidecar fixtures
cargo test -p foco-app fake_sidecar_http_and_websocket_proxy_forward_bearer_token
cargo test -p foco-app llm_stream_broker_rpc_round_trips_through_pending_channel
cargo test -p foco-store config_loads_remote_servers_locations_and_mcp_execution_hosts
npm run test:sidecars
npm run test:remote-server-diagnostics

# Frontend remote server dialog / host-key UX
npm run test -w web -- app-settings
npm run typecheck -w web
```

Coverage intent:

- `e2e_server`: embedded russh server with temporary host/client keys and known_hosts; password/public-key success and failure; unknown/known/changed host keys; trust write; exec stdout/stderr/exit; large stdin; bootstrap line; direct-tcpip loopback; ProxyJump unsupported; no real `~/.ssh` credentials.
- Source guards: production Rust must not spawn system `ssh`/`scp`/`sftp` or reintroduce BatchMode/askpass options.
- Config/API: old config load, `authMethod` default, root/`~` create defaults, password retain/clear, `passwordConfigured`, redacted summaries.
- Fake sidecar HTTP/WS and broker RPC tests unchanged in intent.
- Frontend: SSH hostname/IP labels, new-server root/`~`, auth tabs, password write-only, identity browse, unknown host-key trust dialog, changed host-key hard fail.

## Remote Servers and Workspaces

Remote server profiles live in Settings -> Remote Servers. They hold SSH connection parameters (`hostAlias` as hostname/IP/alias, user, port, identity, auth method, optional password), cached target/diagnostic metadata, sidecar install status, optional default remote root, and optional terminal shell/foco command preferences.

Workspaces reference a server by `serverId` and store their own `remotePath`. The remote sidecar is workspace-scoped: opening or retrying a workspace starts a sidecar for that workspace path. The Remote Servers page aggregates status and workspace counts; it is not a remote daemon manager.

Before deleting a remote server, remove or move every workspace that references it. The API and UI intentionally block deletion while references exist so existing workspaces do not become orphaned.

## Manual SSH E2E Checklist

Use a Linux x64 or arm64 host. Auth may be public key (Agent or IdentityFile) or password stored in Foco global config. Foco does **not** invoke system OpenSSH.

1. Add a server in Settings -> Remote Servers (hostname/IP or config alias). New servers default to user `root`, remote root `~`, auth method Key.
2. On first unknown host key, confirm the SHA-256 fingerprint (trust writes known_hosts). If the host key **changed**, Foco hard-fails until you fix known_hosts out-of-band.
3. Test the server and confirm staged diagnostics reach SSH, target detection, sidecar asset, install directory, and version/install state.
4. Add an SSH workspace using the server and a remote path (`~` is expanded before persistence).
5. Open the workspace for the first time and confirm the sidecar uploads or the custom `focoCommand` is used.
6. Quit/reopen or disconnect/retry and confirm reconnect restores the workspace without duplicate queued chat side effects.
7. Files: list tree, read a file, save a small edit, rename/delete a scratch file.
8. Git: status, diff, stage/unstage a scratch change, branch list, and non-destructive branch switch where safe.
9. Terminal: create a terminal session, run `pwd`, confirm it executes on the remote path.
10. Chat: send a short prompt with a local provider and confirm streaming returns through brokered `llm.stream`.
11. Agent: open Agent runtime/task views and confirm unsupported remote runtime surfaces fail explicitly rather than writing local workspace DB state.
12. Hooks / MCP / scheduled tasks / statistics: same smoke as prior releases.
13. Disconnect the server and confirm workspace status badges move offline; retry and confirm status returns to ready.

## Limits

- Remote scheduled tasks are not guaranteed to run while local Foco is closed. The first v4 release keeps the broker, provider calls, and scheduler coordination tied to the local app process.
- Remote commands, terminal shells, hooks, and workspace-host MCP servers depend on binaries installed on the remote host. Foco does not copy arbitrary tool dependencies.
- Only Linux x64 and Linux arm64 sidecar targets are expected in the first packaged manifest.
- Server management is profile-based. There is no long-lived server daemon independent of workspace sidecars.
- `ProxyCommand` / `ProxyJump` are unsupported (hard error). Encrypted private keys require SSH Agent (or an unencrypted key file).

## Security Notes

- Passwords may be stored in **local global config** only. Summaries expose `passwordConfigured`, never the secret. Passwords never enter logs, error details, sidecar bundles, or remote SQLite.
- Host keys use OpenSSH-compatible `known_hosts`. Unknown keys require explicit fingerprint confirmation; changed keys refuse automatic overwrite.
- Foco does not send provider API keys to the remote sidecar. Provider calls and web/image/provider-secret work stay in the local broker.
- Packaged sidecars are selected by target and verified against `sidecars/manifest.json` sha256 (build identity) before use.
- Default sidecar install path is `~/.foco/sidecars/<version>/<target>/foco` on the remote host. Session token files live under `~/.foco/remote-sessions/` with private permissions and are removed on normal shutdown.
- Sidecar HTTP and control WebSocket bind to loopback and require a per-session bearer token. Tokens are session-scoped and rotate when a new sidecar session starts.
- Production path never spawns system `ssh`/`scp`/`sftp` or askpass helpers.
