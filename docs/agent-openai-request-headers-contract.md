# Agent OpenAI request fields contract

Locked contract for session/multi-turn related fields when Foco calls OpenAI Responses (HTTP or WebSocket). This document separates **official API requirements**, **Agent correlation defaults**, and **Foco client identity**. It does **not** define a Codex compatibility profile.

Implementation status: **runtime injection + wire verification** (Phases 2–3). OpenAIResp adapters receive default Agent correlation headers, fixed Foco identity, and (WebSocket only) `OpenAI-Beta: responses_websockets=2026-02-06` via `genai_chat_options` / `extra_headers`, with `request_overrides` applied after defaults. Phase 3 tests assert the same headers on WebSocket upgrade (`accept_hdr` mock), HTTP raw fixture + audit wire dump, overrides override, non-OpenAIResp negative, and internal one-shot `enable_continuation=false` (no `previous_response_id`).

## Goals

- Make Foco a predictable coding Agent client for OpenAI Responses and Responses WebSocket.
- Default useful correlation headers for gateways, caching, and telemetry without treating product telemetry as hard API requirements.
- Keep client identity as **Foco**, never as `codex_cli_rs`.
- Preserve existing WebSocket session affinity and audit request-id linkage.

## Non-goals

- Codex-compatible settings UI or a “pretend to be Codex CLI” configuration profile.
- Forging ChatGPT-only auth/attestation headers by default.
- Silent HTTP fallback when WebSocket fails (unchanged; see `docs/genai-wire-request-observer.md`).
- Changing WebSocket connection affinity keys.
- Marking the plan **source** chat as a plan session unless that chat is itself a phase `implementation_chat_id`.

---

## Layer model

| Layer | What it is | Default in Foco | Required by OpenAI API? |
| --- | --- | --- | --- |
| **L1 Official / common Responses** | Auth + body multi-turn / cache fields | On where already implemented | Yes for auth; body fields per product path |
| **L2 Responses WebSocket capability** | `OpenAI-Beta: responses_websockets=2026-02-06` | On **only** when `uses_websocket` | Required by Responses WS product path; not used on plain HTTP Responses |
| **L3 Agent correlation** | `session-id`, `thread-id`, `x-client-request-id` (+ optional diagnostic headers) | **On** for OpenAIResp adapters only (HTTP Responses and Responses WebSocket) | No (product/gateway convention) |
| **L4 Foco client identity** | `originator`, `User-Agent`, optional `version` | **On** for OpenAIResp adapters only; fixed Foco values | No |
| **L5 Codex / ChatGPT product headers** | e.g. `ChatGPT-Account-ID`, `x-oai-attestation`, server `x-codex-turn-state`, Codex-style `originator` | **Never** defaulted by Foco | Out of scope |

`request_overrides` (Provider settings header/body overrides) may **override or append** same-named headers after Foco defaults are assembled. Overrides never become a Codex “compatibility mode” product surface.

---

## L1 — OpenAI Responses official / common

Applies to OpenAI Responses HTTP and the prepared request that WebSocket derives from.

| Field | Location | Foco behavior | Default |
| --- | --- | --- | --- |
| `Authorization` | Header | Bearer API key (or override auth). Audit redacts value. | On |
| `prompt_cache_key` | Body / ChatOptions | Already computed and set by Foco for cache-friendly turns | On (existing) |
| `previous_response_id` + `store` | Body | Multi-turn continuation on Responses WebSocket sessions when affinity and prefix match | On for WS continuation path (existing); not invented for one-shot internal kinds |
| `session-id` / `thread-id` as **required** API fields | Header | **Not** required by OpenAI Responses API | Not treated as hard API requirements |

Notes:

- Official multi-turn semantics are primarily **body-side** (`previous_response_id` / store), not `session-id` / `thread-id` headers.
- `session-id` / `thread-id` are product conventions (used by clients such as Codex) for correlation; Foco documents and will inject them under L3, not as L1 API hard requirements.

---

## L2 — Responses WebSocket capability header

| Header | Value | When |
| --- | --- | --- |
| `OpenAI-Beta` | `responses_websockets=2026-02-06` | **Only** on the `openai-responses-websocket` / `uses_websocket` path (upgrade + reused turns that send application headers through the same prepared set) |

Rules:

- Do **not** send this header on plain HTTP `openai-responses` / non-WebSocket OpenAI Chat.
- Do **not** silently fall back to HTTP if the WebSocket path fails.
- `request_overrides` may replace the value if the operator intentionally sets `OpenAI-Beta`.

---

## L3 — Agent correlation headers (default on, OpenAIResp only)

Target adapters: OpenAI Responses HTTP and OpenAI Responses WebSocket (`AdapterKind::OpenAIResp` / Foco OpenAIResp kinds). Other providers (Chat Completions, Anthropic, Gemini, OpenAI-compatible chat, etc.) do **not** get these defaults.

| Header | Value source | Default |
| --- | --- | --- |
| `session-id` | See [Session vs thread mapping](#session-vs-thread-mapping) | On |
| `thread-id` | See mapping | On |
| `x-client-request-id` | Per LLM attempt / broker RPC id; **prefer the same id as the LLM audit request id** so AI Statistics list/detail and wire dumps correlate | On |
| `x-foco-run-id` | Active chat / remote run id (diagnostic) | Optional; may be enabled with correlation defaults |
| `x-foco-workspace-id` | Workspace id (diagnostic) | Optional; may be enabled with correlation defaults |

Rules:

- Correlation headers are **Agent-useful**, not OpenAI hard requirements.
- Local main-process turns and SSH sidecar / broker-forwarded LLM turns use the **same mapping** (store lookup of plan phase binding is local=SSH same SQLite shape).
- Internal one-shot requests (context compression, hooks, memory extraction/retrieval/dream, Spec generate/update/compaction) **inherit** the current chat context’s `session-id` / `thread-id` mapping when they run under that chat. They do **not** use `previous_response_id` continuation.
- `request_overrides` can override any of these names after defaults are applied.

---

## L4 — Client identity (fixed Foco, not Codex)

| Header | Value | Default |
| --- | --- | --- |
| `originator` | `foco` | On (OpenAIResp) |
| `User-Agent` | `foco/<app-version> (...)` with OS/arch detail as implemented | On (OpenAIResp) |
| `version` | `<app-version>` | Optional companion to User-Agent |

Rules:

- Foco’s default identity is **`foco`**, **not** `codex_cli_rs`.
- There is **no** settings switch to “act as Codex CLI” or to swap originator to Codex product strings.
- Operators may still set custom `originator` / `User-Agent` via `request_overrides` if a gateway requires it; that is manual configuration, not a first-class Codex compatibility mode.

---

## L5 — Never default-forge

Foco must **not** invent or default-send:

| Header / field | Reason |
| --- | --- |
| `ChatGPT-Account-ID` | ChatGPT account product path; not Foco API-key Agent default |
| `x-oai-attestation` | Attestation / ChatGPT client trust; not forged |
| Server-driven `x-codex-turn-state` | Server/product state; not a client default |
| Codex-only window/telemetry ids that imply Codex product identity | Out of scope for default Agent headers |

If a user needs a header for a private gateway, they use `request_overrides` explicitly.

---

## Session vs thread mapping

### Product semantics (convention, not OpenAI hard standard)

| Concept | Meaning |
| --- | --- |
| `session-id` | Outer conversation **container** (stable across related threads when appropriate) |
| `thread-id` | One **conversation line** inside that container |

### Foco mapping table

| Context | `session-id` | `thread-id` | How recognized |
| --- | --- | --- | --- |
| **Normal chat** (not a plan phase implementation chat) | `chat_id` | `chat_id` | Default when chat is not bound as any phase `implementation_chat_id` |
| **Plan phase implementation chat** | `plan_id` | that phase’s `implementation_chat_id` | Reverse-lookup: current `chat_id` equals `plan_phases.implementation_chat_id` (local = SSH same store). Enable **only after** the phase has bound an implementation chat; never guess before bind. Retry that creates a **new** implementation chat → new `thread-id`, same `session-id` (`plan_id`). |
| **Subagent** under a plan phase implementation parent | inherited `plan_id` | subagent `chat_id` | Parent chat maps as plan implementation (above) |
| **Subagent** under a normal parent | subagent stable key (own `chat_id`, same as normal chat convention) | subagent `chat_id` | Parent is not a plan implementation chat |
| **Plan merge chat** (if a distinct merge chat exists) | `plan_id` | merge `chat_id` | Same session container as phase implementation chats |
| **Internal one-shot** (compression / hook / memory / spec) | same as the chat context they run under | same as that chat context | Inherit; no `previous_response_id` continuation |
| **SSH broker LLM turn** | same mapping as local for the remote chat/plan binding | same | Store lookup on remote workspace DB; audit/broker RPC id still drives `x-client-request-id` |

### Invariants

1. **Do not** label the plan **source** chat (the chat that created the plan) as a plan session **unless** that chat is itself written as a phase `implementation_chat_id`.
2. **Do not** change WebSocket **session affinity** keys. Affinity remains:

   `workspaceId + runAffinityId + providerId + modelId`

   (plus existing connection-identity checks for live socket reuse: kind, base URL, API key hash, proxy, request overrides, model redirects). Correlation headers are orthogonal to affinity.
3. **`request_overrides`** may override same-named correlation or identity headers after defaults.
4. **`x-client-request-id`** prefers the durable LLM audit request id (local SQLite id / broker RPC id used end-to-end for AI Statistics), so request dumps and list/detail rows stay joinable.
5. Mapping applies only when Foco injects L3 headers (OpenAIResp). Other adapters remain unaffected.

### Worked examples

**Normal chat** `chat-abc`:

- `session-id: chat-abc`
- `thread-id: chat-abc`

**Plan** `plan-xyz`, phase 1 implementation chat `chat-impl-1`:

- `session-id: plan-xyz`
- `thread-id: chat-impl-1`

**Phase 1 retry** with new implementation chat `chat-impl-1b`:

- `session-id: plan-xyz` (unchanged)
- `thread-id: chat-impl-1b`

**Subagent** `chat-sub-9` under `chat-impl-1`:

- `session-id: plan-xyz`
- `thread-id: chat-sub-9`

**Merge chat** `chat-merge-m` for the same plan:

- `session-id: plan-xyz`
- `thread-id: chat-merge-m`

**Source chat** that only created the plan (never became an implementation chat):

- `session-id` / `thread-id` remain that source `chat_id` (normal mapping).

---

## Relationship to existing subsystems

| Subsystem | Interaction |
| --- | --- |
| `prompt_cache_key` | L1 body/options; independent of session/thread headers |
| OpenAI Responses WS `previous_response_id` | L1 multi-turn; connection-scoped; cleared on failure/prefix mismatch; not used for internal one-shots |
| Provider `request_overrides` | Always applied after Foco defaults; can append or replace headers |
| AI Statistics / wire audit | Request headers (incl. correlation) appear in v1 dumps when details are on; `Authorization` only is redacted |
| SSH remote | Same mapping; broker id is `x-client-request-id` / audit join key; affinity never uses broker RPC id as `runAffinityId` |

See also:

- `docs/genai-wire-request-observer.md` — wire dumps, WS prepare path, affinity, no silent HTTP fallback.
- Project Spec — “Provider 请求 Session/Thread ID 映射” (must stay consistent with this contract).

---

## Acceptance checklist (Phase 1 documentation)

- [x] Two (actually layered L1–L4) field contracts with default on/off called out
- [x] Foco default identity is `foco`, not `codex_cli_rs`
- [x] Explicitly no Codex compatibility settings surface
- [x] Session vs thread semantics and Foco mapping documented
- [x] Mapping covers main chat, plan phase implementation, subagent, merge, internal one-shot, SSH broker
- [x] Plan implementation: `session-id=plan_id`, `thread-id=implementation_chat_id`
- [x] Normal chat: `session-id=thread-id=chat_id`
- [x] Per-turn request id correlatable with audit id
- [x] WebSocket session affinity key semantics unchanged
- [x] `request_overrides` may override or append same-named headers
