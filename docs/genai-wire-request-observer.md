# genai Foco fork patchset

Foco depends on a long-lived `genai` fork for two maintained capabilities: API audit observers at the final application-layer HTTP boundaries, and a first-class Developer chat role with provider-specific native or fallback serialization. The full upstream source is not vendored in this repository.

## Pinned dependency

- Upstream: `jeremychone/rust-genai`
- Foco fork: `fonlan/rust-genai`
- Upstream baseline: `genai 0.6.4`, commit `bb38ad7d6c2c3bc86ecc84fd6f97a10ad7803e6d`
- Patch branch: `foco/request-observer-0.6.4`
- Pinned patch commit: `d9a1e023920c531adb26137bece5df5b1cbf4d32`

The root `Cargo.toml` pins the fork by the full commit SHA. Do not replace the `rev` with a floating branch or tag. The fork is a Foco-maintained patchset and its observer and Developer-role patches are not planned for upstream submission.

## Product boundary

For new audit records:

- **Request** is the actual prepared HTTP method, URL, headers, and body passed to `reqwest`. Every HeaderMap field/value visible immediately before the prepared request is sent is retained, with only `Authorization` replaced by `********`; URL userinfo/query credentials and JSON-body credential fields keep their existing redaction rules. Transport or proxy layers may still add or rewrite headers after this observer boundary, and those later mutations are not guaranteed to appear in the audit record.
- **Response** contains the real HTTP status/version/headers captured after `reqwest::Response` is established and before body or SSE consumption, plus the provider adapter's final normalized completion or terminal error. For a detail-enabled OpenAI Responses failure, that terminal `provider_final_response_v1` envelope may include one bounded `streamDiagnostic`; it is a single redacted failure snapshot, not an SSE/WebSocket frame history. Response headers use the same Authorization-only masking rule. The UI renders status/version, a response-headers JSON block, and one complete final response-envelope JSON block. Foco does not persist raw SSE frames or individual chunks in the API detail body.
- This is a local audit feature, not a general secret scrubber: headers such as `X-API-Key`, `Cookie`, `Set-Cookie`, signatures, and token-named custom headers can be stored in workspace audit storage (Zstd segment files under `.foco/llm-audit/segments/`, indexed from SQLite) and shown in details.
- TLS ciphertext, HTTP/2 frames, and TCP packets are outside this feature's scope.
- Existing `save_request_response_details` and retention settings control whether request/final-response detail is retained. When details are off, both `request_body_json` and `response_body_json` are `NULL`. Non-empty detail columns accept only version-1 dumps: HTTP `provider_request_v1`, WebSocket `provider_websocket_request_v1`, and terminal `provider_final_response_v1`. Legacy Neutral/normalized/`{error}`/`{cancelled}`/`legacy_text_v1` payloads are rejected on write and pruned to `NULL` on database open (never forged into v1). Normalized state stays in `llm_request_events.normalized_event_json`, run events, and structured columns only. Main-process wire is the sole detail source of truth for local and SSH; sidecar mirrors keep structured columns only with detail always `NULL`.
- **OpenAI Responses WebSocket request dump (`provider_websocket_request_v1`)** captures the derived `ws`/`wss` URL (scheme-only transform from the Responses HTTP endpoint), redacted headers (Authorization → `********`), the serialized `response.create` client frame, `frameSent`, `connectionReused`, and optional upgrade handshake metadata observed on that turn only. `frameSent` is `true` only after the client successfully wrote `response.create` to the socket; connect/send failures may still persist a diagnostic dump with `frameSent=false` and must not be treated as observed wire. Reused-socket turns set `connectionReused=true`, omit handshake metadata, and must not invent HTTP `101` / `200` response heads for `status_code` or `provider_final_response_v1.http`. Failed upgrades that return an HTTP status (for example 401/429/5xx) preserve that real status on the connection error; DNS/TLS/other failures keep `status_code` null. Terminal normalized completion still uses `provider_final_response_v1` (with real handshake head only when this turn performed the upgrade).
- **Structured `llm_requests.status_code` is independent of the detail switch.** Whenever a real HTTP Response head is observed (including a WebSocket upgrade status on a new connection), Foco persists that status into the structured column (local workspace DB and SSH `remote-workspace-audit` mirror). Full head dumps (version/headers) and wire envelopes still require `save_request_response_details=true`. DNS/TLS/connect failures, cancel before a Response, or a reused WebSocket turn without a new handshake leave `status_code` as `NULL` (UI `n/a`). List/detail APIs read only this structured column; they never invent status from `final_state` or optional wire dumps. Historical rows with `status_code IS NULL` may be backfilled once from retained legal `provider_final_response_v1` (`http.status`, else failed-envelope `statusCode` in 100–599); cleaned/non-v1 detail stays `NULL`.

## Minimal fork patch

The fork exports `PreparedRequestObserver` and `ResponseHeadObserver`, and exposes the capture-aware streaming entry point `exec_chat_stream_observed_with_response`. The observers are optional and read-only at the final genai streaming HTTP boundaries:

1. the adapter performs model mapping and builds the provider-specific request;
2. request overrides and adapter-specific headers/body are applied;
3. a single `reqwest::Request` is built and passed to `PreparedRequestObserver` immediately before send;
4. the same request is executed once;
5. when `reqwest::Response` exists, `ResponseHeadObserver` receives its status/version/HeaderMap before status validation, body reads, or SSE decoding.

The observers must not serialize the adapter request twice, send a duplicate request, consume the response body early, or fabricate response metadata. Availability is deliberately tied to whether a real HTTP response was established:

| Outcome | Request observer | Response-head observer |
| --- | --- | --- |
| 2xx streaming response | Available before the single send | Available before the first body/SSE read |
| Non-2xx HTTP response | Available before the single send | Available before status/error handling |
| HTTP response established, then stream/body decoding fails | Available | Available; the captured head remains attached to the failed/partial final envelope |
| DNS, TLS, connect, or proxy failure before an HTTP response | Available when the request was prepared | Unavailable; Foco must not synthesize status, version, or headers |

Foco's provider layer owns Authorization masking, existing URL/body credential redaction, and the versioned `ProviderWireRequestDump` / `ProviderFinalResponseDump` envelopes. Neither observer promises visibility into headers inserted or rewritten by transport/proxy code after its boundary.


## OpenAI Responses prepare + transport-neutral decoder

In addition to the HTTP observers, the fork exposes helpers so Foco can drive OpenAI Responses over WebSocket without duplicating adapter serialization:

- `Client::prepare_chat_stream_request` returns the finalized HTTP URL, headers, and Responses JSON body after model mapping, tools, reasoning, request overrides, and auth overrides — the same payload that `exec_chat_stream` would send. Existing `exec_chat_stream` / `exec_chat_stream_observed_with_response` remain compatible.
- `adapter::OpenAIRespEventDecoder` is a transport-neutral JSON event state machine shared by the SSE `OpenAIRespStreamer` and host WebSocket transports. Callers feed SSE `data:` lines or WebSocket text frames into `decode_json_to_chat_event`; terminal `response.completed` / `failed` / `incomplete`, top-level `error`, and EOF without a terminal event are handled uniformly.
- `OpenAIRespStreamDiagnostic` is the bounded failure-only diagnostic boundary shared by that decoder. It records `provider_error_event`, `response_failed`, `invalid_json`, `transport_error`, or `unexpected_eof`; the source transport; the prior accepted event and sequence; extracted OpenAI `code` / `type` / `message` / `param`; and a credential-redacted payload snapshot capped at 16 KiB. Snapshots always include original byte count, SHA-256, and truncation state. The original provider message and code win whenever the failure payload supplies them; the generic fallback is only used when those fields are absent. Successful deltas and ignorable unknown events never create a diagnostic. Existing stream APIs are unchanged; capture-aware callers use `exec_chat_stream_observed_with_response_and_diagnostics`, while host WebSocket transports read `OpenAIRespEventDecoder::diagnostics` and report their own socket errors with `record_transport_error`.
- Foco serializes this optional field only for a failed, detail-enabled turn. The persisted failed envelope has a 32 KiB ceiling: if necessary, the optional diagnostic is replaced by a SHA-256/byte-count truncation summary while response status and correlation headers remain available. Detail-disabled turns, successful streams, and sidecar SQLite rows retain no diagnostic detail. The normal API-audit retention job clears the whole request/response detail columns, including this optional field, without a schema migration.
- `adapter::openai_resp_websocket_create_payload` converts an HTTP Responses body into a WebSocket `response.create` client event by removing `stream` / `background` and setting `type: "response.create"`.
- WebSocket URL derivation (scheme-only `http→ws` / `https→wss`) and the tungstenite connection remain Foco provider-layer concerns; genai does not add a WebSocket dependency.
- Foco's `OpenAiRespWsSessionRegistry` (AppState) owns run-scoped connection reuse and `previous_response_id` continuation. Affinity is `workspaceId + runAffinityId + providerId + modelId` (local: assistant message id; SSH broker: remote `runId`, never broker RPC id). Live socket reuse additionally requires matching connection identity (kind, base URL, API key hash, proxy, request overrides, model redirects). Continuation requires matching routing fingerprint (including ChatOptions) and a content hash of the committed message prefix — length alone is not enough. Hard-bounded max sessions (waits for eviction; never grows past the limit). One-shot internal kinds omit session context. Connection rebuild, config/identity mismatch, prefix mismatch, failures, and `previous_response_not_found` clear continuation. Reused turns do not fabricate an HTTP 101 response head for wire audit and emit `provider_websocket_request_v1` with `connectionReused=true`. SSH control disconnect/reconnect and remote active-run terminal (heartbeat removals) invalidate Provider sessions. The kind never silently falls back to HTTP Responses.
- **Independent protocol `openai-responses-websocket`**: still under the OpenAI service; configuration uses only the existing HTTP `base_url` (no `websocket_url`). Runtime path: adapter builds the full Responses HTTP URL → scheme-only `http→ws` / `https→wss` (host, port, path, query preserved). OpenAI Chat and generic OpenAI-compatible Chat do not open WebSocket. Custom gateways must expose WebSocket on the same Responses path. Connection limits: single-connection max age 60 minutes, one in-flight response per connection, `previous_response_id` is connection-scoped and invalidated on failure/disconnect. API proxy is rejected with this protocol. First version: no silent HTTP fallback; proxy disabled rather than auto-fallback.
- **Agent correlation / identity headers (OpenAIResp)** are specified in `docs/agent-openai-request-headers-contract.md` (L1 official body fields vs L2 WS `OpenAI-Beta` vs L3 `session-id`/`thread-id`/`x-client-request-id` vs L4 fixed Foco identity). Correlation headers are orthogonal to WebSocket affinity keys and do not invent Codex auth headers.

## Developer role adapter contract

`NeutralChatRole::System`, `NeutralChatRole::Developer`, and `NeutralChatRole::User` remain distinct through Foco's neutral request layer and the `genai` chat model. Foco only extracts leading System messages into `ChatRequest.system`; it must not merge Developer into that top-level field or rewrite Developer back to System at the provider boundary. API roles define instruction priority. XML tags inside message content only organize prompt sections and do not replace or strengthen the API role boundary.

The fork then applies an explicit per-adapter contract:

- OpenAI Chat Completions (`AdapterKind::OpenAI`) natively emits Developer as an independent `messages` entry with `role: "developer"`. A custom endpoint configured as Foco's OpenAI Chat provider must accept that role.
- OpenAI Responses keeps `ChatRequest.system` in `instructions` and emits Developer as an independent `input` message with `role: "developer"`; it does not fold Developer into `instructions`.
- OpenAI-compatible adapters without an established shared Developer protocol, including DeepSeek, OpenRouter, xAI, Groq, and Together, semantically fall back to an independent `role: "system"` message. They must not blindly receive an unknown `developer` role.
- Anthropic/MiniMax route Developer content through their top-level system mechanism; Gemini/Vertex route it through `systemInstruction`. Other supported non-OpenAI adapters use their existing system-equivalent mechanism.

"Native support" means the finalized provider JSON preserves the Developer role value. "Semantic fallback" means the adapter preserves the instruction content and ordering through its supported system mechanism while changing the wire role/field. Capability selection belongs in the fork adapters; Foco's provider conversion must remain provider-agnostic.

## End-to-end acceptance in Foco

Foco's completion gate is not the fork test alone. The repository keeps multiple layers of real HTTP regressions:

- `providers/lib.rs::tests::captures_finalized_requests_for_four_primary_adapters` starts local servers for OpenAI Chat, OpenAI Responses, Anthropic, and Gemini. It compares the observer dump with the request actually received by the server; precisely fixes System/Developer/User placement for native OpenAI and semantic fallback adapters; verifies provider-specific final mappings (model redirect, tools, thinking, prompt-cache-related options and supported overrides); checks Authorization-only header masking plus existing URL/body credential redaction; and rejects duplicate sends. `openai_compatible_fixture_falls_back_developer_to_system` adds a real DeepSeek-compatible request proving that the shared OpenAI-compatible path does not blindly emit `role: "developer"`.
- `providers/lib.rs::tests::captures_final_wire_request_and_only_final_response`, `captures_http_response_head_for_non_success_stream`, and `connection_failure_before_http_response_does_not_fabricate_response_head` fix the response-head availability matrix: successful and non-2xx responses preserve real status/version/headers before body consumption, while a connect failure has no fabricated HTTP head.
- `providers/lib.rs::tests::http_status_preserves_non_default_success_status_without_detail_dumps`, `http_status_is_captured_without_detail_dumps`, `http_status_survives_stream_decode_failure_after_response_head`, and `connection_failure_http_status_is_none_without_response` lock structured status capture independent of detail dumps: non-default 2xx (e.g. 201), non-2xx, post-head decode failure, and pre-response connect failure.
- `providers/lib.rs::tests::openai_resp_detail_stream_exposes_failed_frame_diagnostic` and `websocket_failed_provider_event_preserves_decoder_diagnostic_and_handshake_head` lock HTTP SSE and WebSocket failure parity: provider `code`/`message`, redacted payload, terminal envelope, and the independently observed HTTP/upgrade head are retained together. The fork decoder fixture suite additionally covers top-level `error`, `response.failed`, invalid JSON, EventSource errors, and EOF before a terminal event.
- `app/tests/mod.rs::main_chat_real_http_bytes_persist_as_wire_and_detail_api_returns_wire` runs the production main-chat stream against a local provider, then verifies the same final request body and representative response headers (`Content-Type`, `X-Request-ID`, `Set-Cookie`, and masked `Authorization`) travel through the observers into SQLite and the AI statistics detail handler. The final aggregate is returned as `provider_final_response_v1` without chunk-only fields. `main_chat_responses_failed_stream_persists_diagnostic_and_http_head` and `main_chat_retry_attempts_keep_independent_failed_stream_diagnostics` cover failed SSE diagnostics and prove retry attempts retain separate failed envelopes. The companion detail-disabled test verifies one send with no request/response detail.
- `app/remote_workspace.rs::remote_ssh_sidecar_chat_turn_persists_real_wire_to_profile_audit_mirror`, `remote_ssh_sidecar_failed_responses_stream_persists_diagnostic_only_in_main_audit_mirror`, and `broker_control_llm_stream_persists_real_provider_wire_and_exposes_same_request_id_to_detail_api` cover the real SSH path: control broker → mock provider HTTP → main-process profile audit mirror → Detail API, with sidecar detail columns always `NULL`. The failed Responses fixture uses the actual broker request id and proves `streamDiagnostic`, response head, and trace header remain in the main-process mirror only. List/detail assert the same structured `statusCode` as SQLite.
- `app/remote_workspace.rs::broker_control_llm_stream_persists_status_code_without_request_response_details` and `broker_control_llm_stream_persists_http_failure_status_code_independently_of_final_state` prove details-off still stores real `status_code` (including non-default 2xx) with NULL dumps, and HTTP failures keep failed final_state with the observed status (not hard-coded 200).
- `app/tests/mod.rs::proxy_workspace_route_path_keeps_ai_statistics_on_main_process` and `remote_ai_statistics_detail_http_reads_main_process_audit_mirror` lock the HTTP boundary: workspace-scoped AI Statistics detail must **not** be remote-proxied to the sidecar (where detail is always `NULL`). The real App Router serves detail from the main-process audit mirror; chat statistics / files / context-usage stay proxied as before.
- `web/app-panels-stats.test.tsx` covers Request/Response headers, status/version, complete final response JSON, successful/failed/partial/malformed/unavailable states (no legacy normalized body rendering), and nested JSON scrolling. Vertical wheel input stays in an inner code scroller until it reaches the top or bottom, then advances the outer detail scroller; horizontal wheel input remains native to the code block.

These tests are the hard regression against a UI-only or import-only integration. When the fork, genai baseline, adapter code, audit lifecycle, SQLite schema, or statistics detail API changes, rerun both provider tests and the focused app tests before claiming wire capture is complete.

Adapter behavior must be asserted from the actual finalized request rather than assumed to be uniform. In the current genai baseline, OpenAI adapters merge supported arbitrary `extra_body` values, while Anthropic and Gemini expose their provider-specific thinking mapping but do not preserve an arbitrary extra-body key. The dump must reflect those real adapter semantics; tests must not manufacture fields that were not sent.

## Upgrade procedure

When upgrading genai:

1. Fetch the latest upstream history into the fork and identify the exact upstream release commit to use as the new baseline.
2. Create a version-specific maintenance branch from that upstream commit. Reapply or cherry-pick only the Foco patchset: prepared-request observer, response-head observer, first-class Developer role, the explicit adapter native/fallback mappings, OpenAI Responses prepare_chat_stream_request, transport-neutral OpenAIRespEventDecoder, and openai_resp_websocket_create_payload. Resolve conflicts explicitly at the final request-build/send and response-established/pre-body boundaries and at each adapter's role serialization boundary.
3. Verify every streaming adapter still passes through both observed boundaries after model mapping, provider payload construction, `extra_body`, request overrides, and adapter-specific headers are applied. Separately verify Developer remains native only for OpenAI Chat/Responses and continues to use the documented semantic fallback for every other adapter.
4. In the fork, run formatting plus the local observer HTTP/SSE fixtures. They must prove that captured request data equals what the server receives, response heads are captured for successful/non-2xx/pre-decode-failure paths, and the server receives exactly one request.
5. Push the validated patch commit to `fonlan/rust-genai` before changing Foco. Record its full 40-character SHA.
6. Update the root `Cargo.toml` `rev`, refresh `Cargo.lock`, and confirm the lockfile source resolves to that same fork commit.
7. Run `cargo test -p foco-providers --locked`; the provider fixtures must continue to verify captured request bytes, request overrides, redaction, and final-response aggregation without raw SSE/chunk persistence.
8. Run `cargo check -p foco-app --locked` to catch observer signature or stream-contract conflicts in audit callers.
9. Review the fork diff against the selected upstream baseline and reject unrelated adapter behavior changes.

If a future genai release supplies equivalent prepared-request and response-head hooks, Foco may adopt them and retire the fork patch after the same wire-capture and redaction regressions pass.

## License and provenance

`genai` originates from `jeremychone/rust-genai` and is distributed under its upstream MIT or Apache-2.0 license terms. The fork preserves the upstream Git history and license files. Cargo records the exact fork URL and commit in `Cargo.lock`, keeping the third-party source and local patch provenance traceable without storing a second source snapshot in Foco.
