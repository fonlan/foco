# genai wire-request observer fork

Foco depends on a long-lived fork of `genai` so API audit details can observe the final application-layer HTTP request without reimplementing provider adapters. The full upstream source is not vendored in this repository.

## Pinned dependency

- Upstream: `jeremychone/rust-genai`
- Foco fork: `fonlan/rust-genai`
- Upstream baseline: `genai 0.6.4`, commit `bb38ad7d6c2c3bc86ecc84fd6f97a10ad7803e6d`
- Patch branch: `foco/request-observer-0.6.4`
- Pinned patch commit: `5f0ee426ce327ba71f67a6725e3503533ed33632`

The root `Cargo.toml` pins the fork by the full commit SHA. Do not replace the `rev` with a floating branch or tag. The fork is a Foco-maintained dependency and the observer patch is not planned for upstream submission.

## Product boundary

For new audit records:

- **Request** is the actual prepared HTTP method, URL, headers, and body passed to `reqwest`. Sensitive headers and URL userinfo are redacted before persistence.
- **Response** is the provider adapter's final normalized completion or terminal error after streaming ends. Foco does not persist raw SSE frames or individual chunks in the API detail body.
- TLS ciphertext, HTTP/2 frames, and TCP packets are outside this feature's scope.
- Existing `save_request_response_details` and retention settings control whether request/final-response detail is retained. Older records remain readable as legacy normalized payloads.

## Minimal fork patch

The fork adds an optional read-only prepared-request observer at the final genai send boundary:

1. the adapter performs model mapping and builds the provider-specific request;
2. request overrides and adapter-specific headers/body are applied;
3. a single `reqwest::Request` is built;
4. the observer reads that same request;
5. the same request is executed once.

The patch must not serialize the adapter request a second time or send a duplicate request. Foco's provider layer owns redaction and the versioned `ProviderWireRequestDump` / `ProviderFinalResponseDump` envelopes.

## Upgrade procedure

When upgrading genai:

1. Fetch the latest upstream history into the fork and identify the exact upstream release commit to use as the new baseline.
2. Create a version-specific maintenance branch from that upstream commit. Reapply or cherry-pick only the minimal prepared-request observer patch, resolving conflicts explicitly at the final request-build/send boundary.
3. Verify every adapter still passes through the observed boundary after model mapping, provider payload construction, `extra_body`, request overrides, and adapter-specific headers are applied.
4. In the fork, run formatting plus the local observer HTTP/SSE fixture. It must prove that captured headers and body equal what the server receives and that the server receives exactly one request.
5. Push the validated patch commit to `fonlan/rust-genai` before changing Foco. Record its full 40-character SHA.
6. Update the root `Cargo.toml` `rev`, refresh `Cargo.lock`, and confirm the lockfile source resolves to that same fork commit.
7. Run `cargo test -p foco-providers --locked`; the provider fixtures must continue to verify captured request bytes, request overrides, redaction, and final-response aggregation without raw SSE/chunk persistence.
8. Run `cargo check -p foco-app --locked` to catch observer signature or stream-contract conflicts in audit callers.
9. Review the fork diff against the selected upstream baseline and reject unrelated adapter behavior changes.

If a future genai release supplies an equivalent prepared-request hook, Foco may adopt it and retire the fork patch after the same wire-capture and redaction regressions pass.

## License and provenance

`genai` originates from `jeremychone/rust-genai` and is distributed under its upstream MIT or Apache-2.0 license terms. The fork preserves the upstream Git history and license files. Cargo records the exact fork URL and commit in `Cargo.lock`, keeping the third-party source and local patch provenance traceable without storing a second source snapshot in Foco.
