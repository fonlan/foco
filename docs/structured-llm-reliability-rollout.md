# Structured single-tool LLM reliability — rollout & observation

Internal request kinds covered (unchanged for normal chat tool selection):

| `request_kind` | Forced tool | Safe degrade on protocol/schema failure |
|---|---|---|
| `memory retrieval` | `select_relevant_memory` | FTS → empty selection (do not block chat) |
| `memory extraction` | `submit_memory_extraction` | Complete job with **no facts** + `ignoredReason` |
| `workspace spec update` | `submit_workspace_spec_update` | Do not write Spec; job fails (retryable) |

Transport: `NeutralToolChoice::RequiredSingleTool` when the adapter supports it; otherwise degraded (tools + prompt + repair/text-JSON). Unsupported adapters: Ollama / OllamaCloud / Cohere / BedrockApi.

## Retry budgets (cost ceiling)

Per **one** audited call (extraction/retrieval/spec update each invoke audited **once**):

| Budget | Cap |
|---|---|
| Output-protocol repair | **1** (feedback message; no body re-prompt) |
| Provider transport (`llm_request_retry_count`) | **N** additional tries **per request body** |
| Hard stream attempt cap | `audited_max_stream_attempts(N) = 2N + 2` |

There is **no** outer extraction loop multiplying `N`. Text-JSON recovery runs **before** repair and does not count as an extra LLM attempt.

Audit start events distinguish attempts:

- `retryKind`: `initial` | `provider_retry` | `output_repair`
- `providerRetryIndex` / `providerRetryBudget`
- `outputRepairAttempt` / `outputRepairUsed`
- `attempt` / `maxAttempts`

`llm_requests` columns (schema 41+): `structured_outcome`, `recovery_source`, `attempt_index`.  
Schema 42+: `structured_call_id` links all stream attempts of one audited call (job).

## Gray rollout

1. **Enable only internal kinds** (already the case): chat completion keeps `tool_choice=Auto` and never synthesizes tools from assistant text (local multi-tool path and remote broker chat path).
2. **Observe** via store helpers (no sensitive body text):
   - `WorkspaceDatabase::structured_llm_outcome_breakdown`
   - `WorkspaceDatabase::structured_llm_outcome_kind_summaries`
3. **Slice** by `provider_id`, `model_id`, transport (`http` / `websocket` / `unknown`), and `attempt_index`.
4. **If a provider lacks native forced tools**, rows will still succeed via text-JSON / repair; use breakdown `recovery_source` (`tool_call` vs `text_json` vs `correction_retry`) and `ToolChoiceEnforcement` warn logs to locate degraded adapters.

### Metric definitions (job-level, not attempt-row)

Each audited call writes **one `llm_requests` row per stream attempt** and stamps a shared **`structured_call_id`**. Summaries therefore:

| Field | Meaning |
|---|---|
| `first_attempt_requests` | ≈ number of **jobs** (rows with `attempt_index = 1`) |
| `first_attempt_successes` | Structured success on attempt 1 (`succeeded` / `text_json_recovered`) |
| `terminal_successes` | Structured success on **any** attempt of the job (production: ≤1 success row per job) |
| `total_requests` | All attempt rows counted for jobs in scope (including repair / provider retries) |
| `extra_request_count` | Rows with `attempt_index > 1` for those jobs |
| `first_attempt_success_rate` | `first_attempt_successes / first_attempt_requests` |
| `terminal_success_rate` | **`terminal_successes / first_attempt_requests`** (job-level) |
| `job_terminal_failures` | **`first_attempt_requests - terminal_successes`** |
| `job_terminal_failure_rate` | **`job_terminal_failures / first_attempt_requests`** |
| `first_attempt_provider_failures` | First-attempt rows with `provider_timeout` / `provider_error` (exact first-attempt slice) |
| `first_attempt_protocol_failures` | First-attempt rows with `missing_tool` / `wrong_tool` / `schema_invalid` (exact first-attempt slice) |

Example: attempt 1 `missing_tool` fail + attempt 2 repair success →

- `first_attempt_requests = 1`, `terminal_successes = 1` → **terminal success rate = 100%**
- `job_terminal_failures = 0` → **job terminal failure rate = 0%** (recovered jobs must not inflate the failure rate)
- `first_attempt_protocol_failures = 1` (diagnostic: first attempt was protocol-class; repair recovered)
- must **not** report `1 / 2 = 50%` (that would be attempt-row success share)

Average attempts per job ≈ `total_requests / first_attempt_requests`.

Structured classification wins over `final_state`: a `schema_invalid` row is **not** a terminal reliability success even if `final_state` was briefly `succeeded`.

### Fixed observation windows (job cohort)

When `started_after` / `started_before` are set on `structured_llm_outcome_kind_summaries`:

1. A job belongs to the window if its **first attempt** (`attempt_index = 1`) has `request_started_at` in range.
2. Terminal success for that job considers **all** attempts sharing the same `structured_call_id`, including repair/provider retries whose `request_started_at` is **outside** the window.
3. An orphan later-attempt success inside the window whose first attempt is outside the window **does not** enter the cohort and **must not** cancel unrelated in-window failures.

Without `structured_call_id` (historical rows), multi-attempt jobs cannot be joined across a window boundary. Windowed terminal success then uses the first-attempt row only and never credits unlinked orphan later successes (conservative for the failure gate).

Full-population queries (no time filter) still aggregate all filtered attempt rows; with production call ids this matches the job-cohort definition when the full history is present.

### No invented "protocol terminal failure rate"

Summaries **must not**:

```text
# WRONG — do not reintroduce
protocol_terminal ≈ max(0, job_terminal_failures - first_attempt_provider_failures) / first_attempt_requests
```

Cross-job counterexample:

| Job | Attempt 1 | Later | Terminal |
|---|---|---|---|
| A | `provider_error` | success | success |
| B | `missing_tool` | (none) | protocol fail |

Then `job_terminal_failures = 1`, `first_attempt_provider_failures = 1`. The broken subtraction reports **0%**, while the only terminal failure is protocol (**50%** of jobs). The same aggregate can also over-count when a job starts as protocol and ends as provider. The formula has **no direction guarantee**.

Until terminal outcomes are labeled by failure class per job:

- Use **`job_terminal_failure_rate`** as the **conservative upper bound** on any terminal-failure slice (includes provider outages, semantic failures, protocol, and other).
- Use **`first_attempt_protocol_failures` / `first_attempt_requests`** only as a **first-attempt diagnostic**, not as terminal protocol attribution.
- Prefer breakdown + `recovery_source=correction_retry` share to confirm repair helps rather than only inflating attempts.

### Success criteria (reliability + cost)

Track over a fixed window (e.g. 7 days) for the three baseline kinds, using **job-cohort** summaries (first-attempt attribution + `structured_call_id`):

| Metric | Definition | Target |
|---|---|---|
| First-attempt tool success rate | `first_attempt_success_rate` | Improve vs pre-rollout baseline |
| Terminal success rate | Job-level `terminal_success_rate` (after repair/provider retry) | High |
| Job terminal failure rate (gate) | `job_terminal_failure_rate` | **&lt; 2%** of jobs (conservative upper bound on protocol-class terminal failures) |
| First-attempt protocol failures | `first_attempt_protocol_failures / first_attempt_requests` | Improve vs baseline (diagnostic; not a terminal-attribution gate) |
| Average attempts / call | `total_requests / first_attempt_requests` | No large rise vs baseline (repair is at most +1) |
| Extra request share | `extra_request_count / total_requests` | Prefer &lt; ~30% unless provider outage |
| Latency / tokens | Sum of per-attempt latency and usage on `llm_requests` | Average request count must not climb without reliability gain |

Notes on the **&lt; 2%** gate:

- The plan goal is protocol-class **terminal** failure &lt; 2%. Without per-job terminal class labels that rate is **not exactly observable**.
- `job_terminal_failure_rate &lt; 2%` is a **stricter / conservative** gate: if overall job terminal failures stay under 2%, any protocol subset is also under 2%.
- Pure provider outages **do** count toward this gate. During a known provider outage, slice by `first_attempt_provider_failures` / breakdown and do not treat the temporary rise as a protocol regression by itself.
- Do **not** subtract provider counts from job failures to claim a protocol terminal rate.
- Prefer windows on data written after schema 42 (`structured_call_id` present). Pre-call-id history is fine for full-population baselines; windowed multi-attempt rates on that history are conservative.

### Rollback / narrow

- Forced tool is request-local: no global kill switch required for chat.
- To stop repair only would require a code change; practical ops lever is reducing `app.llm_request_retry_count` to cap transport cost.
- Extraction ignore-on-protocol-failure remains preferred over wrong fact writes.

## Local vs remote parity

| Path | Coverage |
|---|---|
| Local `resolve_audited_single_tool_arguments` | ToolCall prefer, bare/fenced JSON, reject multi-tool/prose |
| Local `next_audited_stream_action` / `count_stream_attempts_until_stop` | Fault matrix + budget |
| Local `audited_provider_tool_request` | Output repair (1×) + provider transport budget; audit `retryKind`; schema validate repair |
| Provider wire `tool_choice` | OpenAI chat / Responses serialization tests |
| Remote broker `recover_broker_single_tool_call_from_text` | Spec kinds recover; **chat completion does not** |
| Remote `classify_remote_single_tool_protocol_failure` | Rejects **mixed** expected+wrong tool sets as `wrong_tool` (local parity) |
| Remote `remote_sidecar_broker_tool_request_with_args_validate` | Same output-repair state machine for missing/prose/wrong tool/**schema_invalid** (1×); Spec update wires serde shape validate |

Memory extraction/retrieval on SSH run audited on the sidecar (local audited path). Spec update uses broker `llm.stream` + sidecar repair loop so prose / wrong tool / schema-invalid args get one correction retry before job failure.

### Remote single-tool wrapper scope

`remote_sidecar_broker_tool_request` / `_with_args_validate` is a **generic internal single-tool** helper. Protocol repair (missing tool / prose / wrong tool, 1×) therefore applies to every caller, not only Spec update:

| Caller | Protocol repair (1×) | Schema-shape validate repair |
|---|---|---|
| Workspace Spec update | yes | yes (`validate_workspace_spec_update_tool_arguments`) |
| Workspace Spec generation | yes | no |
| Workspace Spec compaction / update compaction | yes | no |
| Git commit message generation | yes | no |
| Ordinary chat completion | **not used** | n/a |

This is intentional for internal single-tool paths (extra cost at most +1 request on protocol failure). Ordinary multi-tool chat never enters this wrapper and never synthesizes tools from assistant text.
