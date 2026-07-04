# SSH Remote Workspaces v4: Remote Server Boundary And Protocol

Date: 2026-07-04

Scope: Phase 1 freezes the Remote Server / Remote Workspace boundary and the sidecar broker protocol. This phase is documentation only; later phases implement config, APIs, runtime, and UI against this contract.

ponytail: first version stays per-workspace and on-demand. There is no always-on remote daemon and no server-wide sidecar pool; if many remote workspaces need faster cold start later, add a server connection pool or ControlMaster layer without changing the domain boundary below.

## Boundary Rule

`RemoteServerProfile` is a local, global configuration and diagnostics entity. It answers: how does local Foco reach a machine, what target did we last detect, which sidecar asset/version applies, where should remote workspaces default to, and which workspaces reference this server?

`WorkspaceLocation` is the workspace entity. A remote workspace is `serverId + remotePath` plus the workspace DB/runtime state that lives beside that remote path. It answers: where is this project's source, which `.foco/foco.sqlite` owns chats/plans/memory/hooks/code graph, and which sidecar session serves this workspace now?

The first release must not turn server management into a permanent remote daemon. Local main starts an SSH command only when testing a server, installing/checking a sidecar asset, or opening/prewarming a remote workspace. The remote process that serves HTTP/WebSocket APIs remains workspace-scoped.

## Domain Objects

### RemoteServerProfile

Owned by local main and persisted in global config.

Minimum later fields:

- `id`: stable local id used by workspace references.
- `name`: UI label.
- `hostAlias`: OpenSSH host alias or host name; this is the canonical connection input.
- `user`, `port`, `identityFile`: optional convenience fields only; authentication remains OpenSSH/ssh-agent responsibility.
- `defaultRemoteRoot`: suggested base directory for new remote workspaces.
- `focoCommand`: optional remote command override for users who install Foco/sidecar themselves.
- `terminalShell`: server-level default shell suggestion; a workspace may override it.
- `connectTimeoutMs`: server-scoped diagnostic/session timeout.
- `lastKnownTarget`, `lastSidecarVersion`, `lastCheckedAt`, `lastError`: cached non-secret diagnostics.

Never persist SSH passwords, private key contents, provider API keys, or provider secrets in this profile.

### WorkspaceLocation

Owned by workspace config. Later shape:

```json
{
  "type": "ssh",
  "serverId": "remote-server-...",
  "remotePath": "/home/alice/project",
  "terminalShell": "/bin/zsh"
}
```

Local workspaces keep the existing local path behavior. SSH workspaces must reference `serverId` instead of duplicating host config. Display can be merged as `serverNameOrAlias:/remote/path`, but persistence stays normalized.

## Execution Locations

Execution location labels:

- `local main`: the user's Foco backend process, owner of global config, provider secrets, OpenSSH subprocesses, local HTTP API, and UI broker endpoints.
- `remote sidecar`: the per-workspace Foco sidecar running on the remote host, bound to remote `127.0.0.1`, owner of remote workspace files and remote workspace DB.
- `brokered local service`: a local-main capability exposed to the sidecar over the authenticated control WebSocket.
- `merged view`: local main combines local/global data with sidecar data before returning it to the UI.

## Server-Scoped Capabilities

| Capability | Scope owner | Execution location | Notes |
| --- | --- | --- | --- |
| Create/update/delete/list server profiles | `RemoteServerProfile` | local main | Settings-only management. Delete is blocked while referenced by workspaces unless a later migration flow is explicit. |
| SSH test | `RemoteServerProfile` | local main | Use system `ssh` with `BatchMode=yes`; let OpenSSH parse `~/.ssh/config`. |
| Target probe | `RemoteServerProfile` | local main SSH command | Run minimal remote commands such as `uname -s` and `uname -m`; cache normalized target on the server profile. |
| Sidecar asset selection/check | `RemoteServerProfile` | local main | Select packaged sidecar by target and verify local manifest/sha256 before upload. |
| Remote sidecar install/version check | `RemoteServerProfile` | local main SSH command | Check or install `~/.foco/sidecars/<version>/<target>/foco`; cache last installed version/target. |
| Default remote root | `RemoteServerProfile` | local main | Pure config used to prefill Add Workspace SSH path. |
| Connection diagnostics | `RemoteServerProfile` | local main plus short remote commands | Return staged diagnostics: SSH, target, sidecar asset, install dir permission, command/version. |
| Reference count/workspace list | `RemoteServerProfile` plus workspace config | merged view | Count workspaces whose location references `serverId`; used by Settings UI and delete guard. |
| Connect/disconnect/status for server page | aggregate sessions for a server | merged view | Server status is derived from diagnostic cache and per-workspace sidecar sessions, not from a server daemon. |

## Workspace-Scoped Routes And Entrypoints

These APIs are workspace-scoped because they use remote source files, remote `.foco/foco.sqlite`, a remote PTY, or a workspace-specific runtime. For SSH workspaces, local main should proxy them to the workspace sidecar after ensuring the sidecar session is ready.

| Surface | Current route or entrypoint | Execution for SSH workspace |
| --- | --- | --- |
| Workspace metadata/chats | `GET /api/workspaces`, `GET /api/workspaces/{workspace_id}/chats`, `GET /api/workspaces/search-chats` | merged view for lists; remote sidecar for that workspace's chats/search. |
| Files | `/api/workspaces/{workspace_id}/files`, `/files/children`, `/files/content`, `/files/blob`, `/files/save`, `/files/delete`, `/files/rename` | remote sidecar. |
| Workspace logo | `/api/workspaces/{workspace_id}/logo`, `/logo/thumbnail` | remote sidecar for persisted logo bytes; local main may proxy/cache thumbnails later. |
| Git | `/git/status`, `/git/diff`, `/git/stage`, `/git/unstage`, `/git/discard`, `/git/commit`, `/git/commit-message`, `/git/branches`, `/git/branches/switch`, `/git/branches/create` | remote sidecar; commit message LLM call is brokered local service when needed. |
| Terminal | `/terminal/session`, `/terminal/{session_id}/ws` | remote sidecar owns PTY and session DB; local main WebSocket proxies to sidecar. |
| Chat queue/stream/runs | `/chat/queue`, `/chat/stream`, `/chat/runs/{run_id}/stream`, `/chat/runs/{run_id}/cancel`, `/chat/guidance`, `/context-usage` | remote sidecar owns run state and workspace DB; provider calls and UI questions use brokered local service. |
| Pending questions | `/api/chat/questions/pending`, `/api/chat/questions/{question_id}/answer` | merged view/local main because answering is a UI action; remote sidecar creates question requests through broker. |
| Multi-agent runtime | `/agent-team/*`, `/agent-tasks/{task_id}/action`, agent scheduler background entrypoints | remote sidecar owns team/task DB and scheduler; LLM/tools with secrets use brokered local service. |
| Plans | `/plans`, `/plans/auto-run`, `/plans/order`, `/plans/worktrees/*`, `/plans/{plan_id}/*` | remote sidecar, including isolated worktrees and remote Git operations. |
| Spec | `/spec`, `/spec/settings`, `/spec/generate`, `/spec/jobs`, `/spec/jobs/{job_id}/retry` | remote sidecar for workspace state; generation LLM calls use brokered local service. |
| Hooks | `/hooks/runs`, `/hooks/runs/{hook_run_id}` plus hook execution background entrypoints | remote sidecar for workspace hook runs and command hooks; local/global hook definitions arrive via runtime config. |
| Scheduled tasks | `/scheduled-tasks/*`, `/scheduled-task-runs/*`, scheduler background entrypoints | remote sidecar. Local Foco may wake/connect the sidecar, but jobs are not promised while local Foco is closed. |
| Workspace/chat memory | `/memory/*` when scoped to the workspace/chat; extraction/dream background entrypoints | remote sidecar for workspace/chat memory DB; LLM extraction/dream calls use brokered local service. |
| Global memory | `/memory/*` when scoped global | brokered local service or merged view; global memory DB remains local. |
| Skills | workspace skill discovery/install into workspace | remote sidecar for workspace files; global skill store/marketplace remains local main and can be sent read-only in runtime config. |
| MCP | MCP server runtime/tool calls | route by `executionHost`: workspace-host in remote sidecar, local-host as brokered local service, merged definitions for UI/agent prompts. |
| Code graph | graph indexer startup/watchers and graph tools | remote sidecar; local main must not watch remote paths. |
| AI statistics | `/api/ai-statistics`, `/ai-statistics/{request_id}`, chat statistics | merged view for global pages; remote sidecar for remote workspace details. Broker transport audit stays local-only and should not duplicate user-visible stats. |

Server-scoped APIs such as `Remote Servers list/create/update/delete/test/connect/disconnect/status` must not be mounted under `/api/workspaces/{workspace_id}`. They operate on `RemoteServerProfile` and aggregate workspace references.

## Sidecar Launch Flow

1. Local main resolves the remote workspace: workspace config gives `serverId + remotePath`; server profile gives SSH parameters and sidecar preferences.
2. Local main probes/uses the cached target and selects a sidecar asset from the packaged manifest.
3. Local main verifies the local sidecar sha256, then checks or installs it on the remote host unless `focoCommand` says to use an existing remote command.
4. Local main starts the sidecar over SSH with workspace id/path, server id, a freshly generated token, and a request for `127.0.0.1:0` binding.
5. Sidecar writes one bootstrap JSON object to stdout and then switches to normal logs on stderr or structured log lines that are not parsed as bootstrap.
6. Local main parses the bootstrap, establishes local port forwarding from `127.0.0.1:<localEphemeral>` to remote `127.0.0.1:<bootstrap.port>`.
7. Local main connects to `ws://127.0.0.1:<localEphemeral>/api/remote/control/ws` with the bootstrap token.
8. Runtime config is synced over the control WebSocket before workspace traffic is considered ready.

Broker traffic uses the local-main-initiated WebSocket connection to the sidecar through the local forward. It does not use SSH reverse tunnels.

## Sidecar Bootstrap JSON

The sidecar must print exactly one bootstrap JSON object on stdout, one line, before local main starts proxying traffic.

```json
{
  "version": 1,
  "target": "linux-x64",
  "workspaceId": "workspace-...",
  "workspacePath": "/home/alice/project",
  "serverId": "remote-server-...",
  "port": 43127,
  "token": "opaque-random-session-token",
  "capabilities": {
    "httpProxy": true,
    "controlBroker": true,
    "terminalPty": true,
    "git": true,
    "codeGraph": true,
    "workspaceDatabase": true,
    "runtimeConfigSync": true
  }
}
```

Field rules:

- `version`: bootstrap schema version, starting at `1`.
- `target`: normalized sidecar target selected by local main and confirmed by sidecar.
- `workspaceId`: local workspace id from config; sidecar rejects mismatches in HTTP/WS routes.
- `workspacePath`: canonical or absolute remote path used for this sidecar's workspace DB and file operations.
- `serverId`: local server profile id for diagnostics correlation; sidecar treats it as opaque.
- `port`: remote loopback port where sidecar listens. It must be bound to remote `127.0.0.1`, not `0.0.0.0`.
- `token`: bearer token required for sidecar HTTP and WebSocket requests. It is generated per session and never persisted world-readable.
- `capabilities`: explicit feature flags so local main can fail early or degrade known surfaces.

## Control/Broker WebSocket

Endpoint on the sidecar:

```http
GET /api/remote/control/ws
Authorization: Bearer <token>
```

This single authenticated WebSocket carries sidecar-to-local broker RPC, local-to-sidecar config/control messages, cancellation, streaming chunks, and heartbeat. Messages are JSON text frames in v1. Binary payloads should use existing HTTP upload/download routes or a later framed extension, not ad hoc base64 inside every broker call.

### Message Envelope

Every message uses this common envelope:

```json
{
  "version": 1,
  "type": "request",
  "id": "broker-request-...",
  "method": "llm.stream",
  "payload": {},
  "timestamp": "2026-07-04T00:00:00Z"
}
```

Rules:

- `version`: control protocol version, starting at `1`.
- `type`: one of `request`, `response`, `stream`, `error`, `cancel`, `heartbeat`, `config`.
- `id`: required for `request`, `response`, `stream`, `error`, and `cancel`; omitted or ignored for heartbeat.
- `method`: required on `request`; examples include `llm.stream`, `memory.global.search`, `mcp.local.call`, `ui.askQuestion`, `web.fetch`, `image.generate`, `config.sync`.
- `payload`: method-specific JSON object. Keep provider secrets out of sidecar-originated payloads.
- `timestamp`: sender timestamp for diagnostics, not ordering.

### Request

Sidecar calls local capabilities by sending `request`.

```json
{
  "version": 1,
  "type": "request",
  "id": "broker-request-123",
  "method": "llm.stream",
  "payload": {
    "providerId": "openai",
    "modelId": "gpt-4.1",
    "messages": []
  }
}
```

Local main may also send control requests to the sidecar, primarily `config.sync`, `shutdown`, `health.snapshot`, or future debug commands. Direction is defined by method, not by a second socket.

### Response

Final non-streaming success or final streaming completion:

```json
{
  "version": 1,
  "type": "response",
  "id": "broker-request-123",
  "payload": {
    "status": "ok",
    "usage": {
      "inputTokens": 100,
      "outputTokens": 20
    }
  }
}
```

### Stream Chunk

Streaming data uses `stream` messages with the same request id.

```json
{
  "version": 1,
  "type": "stream",
  "id": "broker-request-123",
  "payload": {
    "sequence": 4,
    "kind": "textDelta",
    "delta": "hello"
  }
}
```

Required stream fields:

- `sequence`: monotonic per request.
- `kind`: `textDelta`, `toolCallDelta`, `usageDelta`, `log`, or a method-specific value.
- `delta` or other method-specific payload fields.

The receiver should treat repeated `sequence` values as duplicates after reconnect/replay once later phases add resumability.

### Error

```json
{
  "version": 1,
  "type": "error",
  "id": "broker-request-123",
  "payload": {
    "code": "provider_auth_failed",
    "message": "Provider credentials are unavailable",
    "retryable": false,
    "details": {}
  }
}
```

Errors are terminal for that request unless the method explicitly defines a retry or resume operation. Include safe details only; never send provider secrets or private key material.

### Cancel

Either side may cancel an in-flight request.

```json
{
  "version": 1,
  "type": "cancel",
  "id": "broker-request-123",
  "payload": {
    "reason": "user_cancelled"
  }
}
```

The side that owns the work must stop streaming, release resources, and answer with either a final `error` using code `cancelled` or a final `response` if the operation already completed.

### Heartbeat

```json
{
  "version": 1,
  "type": "heartbeat",
  "payload": {
    "direction": "ping",
    "time": "2026-07-04T00:00:00Z"
  }
}
```

Use app-level heartbeat in addition to WebSocket ping/pong so both sides can report broker health in diagnostics. Missing heartbeats degrades the workspace session before it is marked offline.

### Config Sync

Runtime config travels over the same control socket after connection:

```json
{
  "version": 1,
  "type": "config",
  "id": "config-sync-1",
  "method": "config.sync",
  "payload": {
    "configGeneration": 42,
    "hash": "sha256:...",
    "bundle": {}
  }
}
```

The bundle may include agent definitions, prompts, model metadata, hooks, MCP definitions, memory/spec/plan settings, and selected skill contents. It must not include provider API keys. Sidecar replies with `response` for the same id after applying or confirming no-op by hash.

## Security Defaults

- Sidecar listens only on remote loopback.
- Local forwarding binds only local `127.0.0.1`.
- All sidecar HTTP and WebSocket traffic requires `Authorization: Bearer <token>`.
- Tokens are per sidecar session and generated by local main.
- No SSH reverse tunnel in v1.
- OpenSSH owns authentication. Foco stores host aliases and non-secret preferences, not credentials.
- Provider calls remain local-main brokered services so provider secrets do not need to exist on the remote host.

## Later Phase Contract Checks

Later phases should add the smallest runnable checks against this document:

- Config tests for `RemoteServerProfile` and `WorkspaceLocation` loading/saving.
- Protocol serialization tests for bootstrap and control messages.
- Router/proxy tests proving server-scoped APIs are outside workspace routes and workspace-scoped APIs proxy to sidecar for SSH workspaces.
- Tool routing tests proving provider secrets and UI-only abilities stay in brokered local services.
