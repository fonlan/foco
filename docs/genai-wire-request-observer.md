# genai Foco fork patchset

Foco depends on a long-lived `genai` fork for two maintained capabilities: API audit observers at the final application-layer HTTP boundaries, and a first-class Developer chat role with provider-specific native or fallback serialization. The full upstream source is not vendored in this repository.

## Pinned dependency

- Upstream: `jeremychone/rust-genai`
- Foco fork: `fonlan/rust-genai`
- Upstream baseline: `genai 0.6.4`, commit `bb38ad7d6c2c3bc86ecc84fd6f97a10ad7803e6d`
- Patch branch: `foco/request-observer-0.6.4`
- Pinned patch commit: `5db8a1aefd0f60a4386b416d892ea57da987704a`

The root `Cargo.toml` pins the fork by the full commit SHA. Do not replace the `rev` with a floating branch or tag. The fork is a Foco-maintained patchset and its observer and Developer-role patches are not planned for upstream submission.

## Product boundary

For new audit records:

- **Request** is the actual prepared HTTP method, URL, headers, and body passed to `reqwest`. Every HeaderMap field/value visible immediately before the prepared request is sent is retained, with only `Authorization` replaced by `********`; URL userinfo/query credentials and JSON-body credential fields keep their existing redaction rules. Transport or proxy layers may still add or rewrite headers after this observer boundary, and those later mutations are not guaranteed to appear in the audit record.
- **Response** contains the real HTTP status/version/headers captured after `reqwest::Response` is established and before body or SSE consumption, plus the provider adapter's final normalized completion or terminal error. Response headers use the same Authorization-only masking rule. The UI renders status/version, a response-headers JSON block, and one complete final response-envelope JSON block. Foco does not persist raw SSE frames or individual chunks in the API detail body.
- This is a local audit feature, not a general secret scrubber: headers such as `X-API-Key`, `Cookie`, `Set-Cookie`, signatures, and token-named custom headers can be stored in workspace SQLite and shown in details.
- TLS ciphertext, HTTP/2 frames, and TCP packets are outside this feature's scope.
- Existing `save_request_response_details` and retention settings control whether request/final-response detail is retained. Older records remain readable as legacy normalized payloads.

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
- `app/tests/mod.rs::main_chat_real_http_bytes_persist_as_wire_and_detail_api_returns_wire` runs the production main-chat stream against a local provider, then verifies the same final request body and representative response headers (`Content-Type`, `X-Request-ID`, `Set-Cookie`, and masked `Authorization`) travel through the observers into SQLite and the AI statistics detail handler. The final aggregate is returned as `provider_final_response_v1` without chunk-only fields. The companion detail-disabled test verifies one send with no request/response detail.
- `web/app-panels-stats.test.tsx` covers Request/Response headers, status/version, complete final response JSON, successful/failed/partial/legacy states, and nested JSON scrolling. Vertical wheel input stays in an inner code scroller until it reaches the top or bottom, then advances the outer detail scroller; horizontal wheel input remains native to the code block.

These tests are the hard regression against a UI-only or import-only integration. When the fork, genai baseline, adapter code, audit lifecycle, SQLite schema, or statistics detail API changes, rerun both provider tests and the focused app tests before claiming wire capture is complete.

Adapter behavior must be asserted from the actual finalized request rather than assumed to be uniform. In the current genai baseline, OpenAI adapters merge supported arbitrary `extra_body` values, while Anthropic and Gemini expose their provider-specific thinking mapping but do not preserve an arbitrary extra-body key. The dump must reflect those real adapter semantics; tests must not manufacture fields that were not sent.

## Upgrade procedure

When upgrading genai:

1. Fetch the latest upstream history into the fork and identify the exact upstream release commit to use as the new baseline.
2. Create a version-specific maintenance branch from that upstream commit. Reapply or cherry-pick only the Foco patchset: prepared-request observer, response-head observer, first-class Developer role, and the explicit adapter native/fallback mappings. Resolve conflicts explicitly at the final request-build/send and response-established/pre-body boundaries and at each adapter's role serialization boundary.
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
