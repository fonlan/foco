# Chat-not-found diagnostics

`chat was not found` may be raised after the browser has already loaded the
same chat from another endpoint. Treat it as a correlation problem, not proof
that the chat ID in a tab parameter is invalid. In particular, encoded tab
parameters such as `fog%2Fchat-…` are workspace/chat routing values, not chat
record IDs.

## Contract

Every instrumented missing-chat failure creates one `chat_not_found` `warn`
event with the following optional structured fields:

| Field | Purpose | Browser-visible |
| --- | --- | --- |
| `diagnosticId` | Opaque, copyable correlation reference | Yes |
| `operation` | Safe operation name, for example `context.usage` | Yes |
| `phase` | Safe failure stage, for example `existing-chat-lookup` | Yes |
| `workspaceId`, `chatId` | Durable routing identifiers | No |
| `runtimeTopology` | `local` or `remote-sidecar` | No |
| `databaseIdentity` | `db-sha256:<digest>` of the normalized database path | No |
| `queuedUserMessageId`, `runId`, `agentTaskId` | Available durable run associations | No |

`databaseIdentity` is a one-way identity scoped to the diagnostic format. It
allows an operator to distinguish workspace database views without emitting a
machine path to logs intended for browser consumption or to HTTP clients.

The HTTP status code and the legacy JSON shape remain compatible:

```json
{
  "error": "chat was not found (diagnostic reference: chat-not-found-…)",
  "diagnostic": {
    "diagnosticId": "chat-not-found-…",
    "operation": "context.usage",
    "phase": "existing-chat-lookup"
  }
}
```

Older clients can continue to read `error`; clients talking to an older server
must treat an absent `diagnostic` object as normal compatibility fallback. The
same opaque reference is also attached to the response header
`x-foco-chat-not-found-diagnostic-id`. The safe operation and phase are
available as `x-foco-chat-not-found-operation` and
`x-foco-chat-not-found-phase`, allowing an SSH proxy completion event to be
joined with the sidecar `chat_not_found` event without logging normal requests.

## Source matrix

| Surface | Current source | Operation / phase | Available durable correlation |
| --- | --- | --- | --- |
| Local queue | `app/prompt/assembly.rs`, `app/http/chat.rs` | `chat.prompt-assembly` / `existing-chat-lookup`; `chat.queue` / `durable-chat-lookup` or `queue-result-readback` | workspace, chat; readback also queued user message and Agent task |
| Local stream and context preview | `app/prompt/assembly.rs` | `chat.prompt-assembly` or `context.usage` / `existing-chat-lookup` | workspace, chat, optional queued user message |
| Local scheduler claim | `app/runtime/agent_scheduler.rs` → `WorkspaceDatabase::claim_agent_chat_queued_run` | `agent.scheduler` / `queued-run-claim` | workspace, chat, queued user message, run, Agent task |
| Local edit | `app/http/chat.rs` → `WorkspaceDatabase::rewrite_chat_from_user_message` | `chat.edit` / `rewrite-chat-from-user-message` | workspace, chat, user message, optional Agent task |
| Local messages, todo, statistics, delete, Agent setup | `app/http/chat.rs`, `app/http/agents.rs` | their named operation / existence, read, or delete phase | workspace and chat |
| Workspace persistence | `store/workspace.rs` | typed `WorkspaceDatabaseError::ChatNotFound` source from queue mutation, start/claim/clear, rewrite, todo mutation | chat; application caller adds workspace/database and run context |
| SSH sidecar message/edit/delete/statistics/todo/team | `app/remote_workspace.rs` | same operation names with `runtimeTopology=remote-sidecar` | workspace, chat; edit includes message; team includes task when available |
| SSH sidecar queue/stream and context usage | `app/remote_workspace.rs` | queue/stream preflight and `context.usage` use the same contract | workspace, chat, queued user message, visible assistant/run/task when available |

The source matrix intentionally excludes successful requests. There is no
per-request success logging added by this contract.

## Privacy and presentation rules

Never add message body, attachment metadata or bytes, prompt content, tool
input/output, token counts, credentials, or unredacted filesystem paths to the
event or response. Database open/query failures may emit a matching `warn` or
`error` only when a failure or suspicious identity mismatch occurs.

The UI must attribute `operation=context.usage` (and other auxiliary endpoint
operations such as statistics, todo, or Agent-team refresh) to that auxiliary
request. It must not mark the primary chat run as failed solely because one of
those requests failed. Safe localized wording may name the operation stage and
show the diagnostic reference; detailed correlation remains in local logs.
