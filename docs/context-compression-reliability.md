# Context compression provider-recovery operations

LLM context compression is an isolated provider workflow. It never reuses a failed
compression stream as chat output and writes a checkpoint only after the final summary has
been successfully persisted.

## Default policy

- A compression can make at most two additional attempts, further bounded by the configured
  `app.llm_request_retry_count` and the shared 300,000 ms provider deadline.
- Retryable provider classes are capacity, rate limit, transient server, and network failures.
  Authentication, validation, context-length, protocol, and other non-retryable failures do
  not retry.
- `Normal` compression is preventative. After its retry budget is exhausted it emits
  `skipped` with `action: continue_without_compression` and the chat continues without a
  partial checkpoint.
- `RequiredOverflow` compression is a safety boundary. It emits `failed` with
  `action: fail_required_overflow`; the next over-budget chat request is not sent.
- A checkpoint snapshot that cannot be persisted emits terminal `failed` with
  `action: snapshot_persistence_failed`. The original chat context is unchanged, and the
  provider request ID remains available for audit correlation without exposing a request body.
- Cancellation, application shutdown, and an interrupted remote run stop further retry
  scheduling immediately.

The UI merges events by `compressionId`. A live `start`, zero or more `retrying` events, and
one terminal event therefore remain one card across SSE replay, reconnect, and history reload.
Older events without mode, attempt, outcome, action, or error fields retain their original
start/completed/failed rendering.

## Diagnostics and incident triage

Structured runtime logs record `provider_id`, `model_id`, `compression_mode`,
`input_token_count`, `retry_class`, `attempt_index`, `action`, and final success token counts.
They deliberately do not include checkpoint request bodies or credentials. A `skipped` action
is the degradation counter; aggregate it by provider/model and time range when measuring
incidents.

For an OpenAI Responses incident, capture the provider request ID and its UTC time window,
then query the workspace AI Statistics view for `requestKind=contextCompression`. The durable
audit request IDs are also sent as `x-client-request-id`, so they can be joined with provider
support logs without exposing request contents in the chat UI. Narrow the result to the chat
run and time range, inspect each attempt's final state/status code, and—only when request-detail
capture was enabled—the bounded, redacted `streamDiagnostic`. In a remote workspace, query the
main-process `remote-workspace-audit` mirror through the same AI Statistics API; sidecar detail
columns intentionally remain empty.

Typical local SQL investigation (read-only, with a copied request ID/time range) is:

```sql
SELECT id, request_kind, final_state, status_code, request_started_at, completed_at
FROM llm_requests
WHERE request_kind = 'contextCompression'
  AND request_started_at >= :incident_start_utc
  AND request_started_at < :incident_end_utc
ORDER BY request_started_at;
```

Do not raise the retry cap for a single provider. The cap and deadline are shared safeguards:
provider-specific behaviour belongs in the structured retry classifier, while operators should
use the existing retry-count and provider timeout settings only after reviewing the audit rows
and runtime outcomes.
