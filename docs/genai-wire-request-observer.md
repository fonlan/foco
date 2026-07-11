# genai wire-request observer patch

Foco vendors `genai 0.6.4` under `vendor/genai` and uses it through the workspace dependency in `Cargo.toml`. The vendored copy exists so API audit details can observe the final application-layer HTTP request without reimplementing provider adapters.

## Product boundary

For new audit records:

- **Request** is the actual prepared HTTP method, URL, headers, and body passed to `reqwest`. Sensitive headers and URL userinfo are redacted before persistence.
- **Response** is the provider adapter's final normalized completion or terminal error after streaming ends. Foco does not persist raw SSE frames or individual chunks in the API detail body.
- TLS ciphertext, HTTP/2 frames, and TCP packets are outside this feature's scope.
- Existing `save_request_response_details` and retention settings control whether request/final-response detail is retained. Older records remain readable as legacy normalized payloads.

## Minimal patch

The local change adds an optional read-only prepared-request observer at the final genai send boundary:

1. the adapter performs model mapping and builds the provider-specific request;
2. request overrides and adapter-specific headers/body are applied;
3. a single `reqwest::Request` is built;
4. the observer reads that same request;
5. the same request is executed once.

The patch must not serialize the adapter request a second time or send a duplicate request. Foco's provider layer owns redaction and the versioned `ProviderWireRequestDump` / `ProviderFinalResponseDump` envelopes.

## Upgrade checklist

When upgrading genai:

1. Compare the vendored source against the exact upstream release and reapply only the prepared-request observer change.
2. Verify every adapter still goes through the observed final send boundary after request overrides are applied.
3. Run `cargo test -p foco-providers --locked`; the local HTTP fixture asserts that server-received request bytes match the captured body, overrides are present, secrets are redacted in the dump, and intermediate SSE-only data is absent from the final response envelope.
4. Run `cargo check -p foco-app --locked` to catch observer signature or stream-contract conflicts in audit callers.
5. Review `git diff -- vendor/genai` and reject adapter behavior changes unrelated to the observer.

If upstream introduces an equivalent prepared-request hook, prefer migrating to it and deleting the local difference while preserving Foco's dump and redaction contracts.
