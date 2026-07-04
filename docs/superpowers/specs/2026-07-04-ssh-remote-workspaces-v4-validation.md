# SSH Remote Workspaces v4 Validation and E2E Notes

This phase keeps automated coverage lightweight: CI does not boot a real SSH server. The durable check is a fake sidecar plus config/script self-checks; real SSH remains a manual E2E pass before release.

## Automated Checks

Run these from the repo root:

```bash
cargo test -p foco-app fake_sidecar_http_and_websocket_proxy_forward_bearer_token
cargo test -p foco-app llm_stream_broker_rpc_round_trips_through_pending_channel
cargo test -p foco-store config_loads_remote_servers_locations_and_mcp_execution_hosts
npm run test:sidecars
npm run test:remote-server-diagnostics
```

Coverage intent:

- Fake sidecar HTTP proxy verifies a remote workspace request is routed through `/api/remote/workspace/*` and includes the sidecar bearer token.
- Fake sidecar WebSocket proxy verifies terminal-style WebSocket upgrade and bidirectional frames through the sidecar tunnel path.
- Broker RPC test verifies `llm.stream` requests enter the sidecar broker channel and responses are delivered through the pending request channel.
- Config test verifies `remoteServers`, legacy `path` workspaces, explicit local `location`, SSH `location`, and MCP `executionHost` defaults/values.
- `test:sidecars` verifies manifest generation, copy, sha256 verification, and tamper detection.
- `test:remote-server-diagnostics` guards the diagnostic stage list, major error kinds, BatchMode SSH, sidecar target selection, sha256 verification, and install-directory writability check.

## Remote Servers and Workspaces

Remote server profiles live in Settings -> Remote Servers. They hold SSH connection parameters, cached target/diagnostic metadata, sidecar install status, optional default remote root, and optional terminal shell/foco command preferences.

Workspaces reference a server by `serverId` and store their own `remotePath`. The remote sidecar is workspace-scoped: opening or retrying a workspace starts a sidecar for that workspace path. The Remote Servers page aggregates status and workspace counts; it is not a remote daemon manager.

Before deleting a remote server, remove or move every workspace that references it. The API and UI intentionally block deletion while references exist so existing workspaces do not become orphaned.

## Manual SSH E2E Checklist

Use a Linux x64 or arm64 host reachable through your normal OpenSSH config. Do not use password auth for the happy path; Foco invokes SSH in BatchMode.

1. Add a server in Settings -> Remote Servers using a host alias from `~/.ssh/config`.
2. Test the server and confirm staged diagnostics reach SSH, target detection, sidecar asset, install directory, and version/install state.
3. Add an SSH workspace using the server and an absolute remote path.
4. Open the workspace for the first time and confirm the sidecar uploads or the custom `focoCommand` is used.
5. Quit/reopen or disconnect/retry and confirm reconnect restores the workspace without duplicate queued chat side effects.
6. Files: list tree, read a file, save a small edit, rename/delete a scratch file.
7. Git: status, diff, stage/unstage a scratch change, branch list, and non-destructive branch switch where safe.
8. Terminal: create a terminal session, run `pwd`, confirm it executes on the remote path.
9. Chat: send a short prompt with a local provider and confirm streaming returns through brokered `llm.stream`.
10. Agent: open Agent runtime/task views and confirm unsupported remote runtime surfaces fail explicitly rather than writing local workspace DB state.
11. Hooks: read/save hook settings and inspect a hook run/audit view if the workspace has hooks configured.
12. MCP: set one MCP server to `workspace` and verify its command dependency exists on the remote host; set one HTTP MCP server to `local` and verify it stays local.
13. Scheduled tasks: list/create a harmless task and confirm remote unsupported/available state is clear in the UI.
14. Statistics: open AI statistics/detail smoke paths and confirm remote data is shown or unavailable state is explicit.
15. Disconnect the server and confirm workspace status badges move offline; retry and confirm status returns to ready.

## Limits

- Remote scheduled tasks are not guaranteed to run while local Foco is closed. The first v4 release keeps the broker, provider calls, and scheduler coordination tied to the local app process.
- Remote commands, terminal shells, hooks, and workspace-host MCP servers depend on binaries installed on the remote host. Foco does not copy arbitrary tool dependencies.
- Only Linux x64 and Linux arm64 sidecar targets are expected in the first packaged manifest.
- Server management is profile-based. There is no long-lived server daemon independent of workspace sidecars.

## Security Notes

- Foco does not store SSH passwords. SSH auth is delegated to OpenSSH config/agent/keys, and diagnostics use BatchMode.
- Foco does not send provider API keys to the remote sidecar. Provider calls and web/image/provider-secret work stay in the local broker.
- Packaged sidecars are selected by target and verified against `sidecars/manifest.json` sha256 before use.
- Default sidecar install path is `~/.foco/sidecars/<version>/<target>/foco` on the remote host. Session token files live under `~/.foco/remote-sessions/` with private permissions and are removed on normal shutdown.
- Sidecar HTTP and control WebSocket bind to loopback and require a per-session bearer token. Tokens are session-scoped and rotate when a new sidecar session starts.
