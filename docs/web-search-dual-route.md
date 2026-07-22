# Web Search Dual-Route Contract

Foco routes web search through a single central resolver so local and SSH remote runs stay consistent.

## Configuration

| Layer | Field | Meaning |
| --- | --- | --- |
| Global | `web_search.enabled` | Master switch for any online search path |
| Global | Tavily / Brave API keys | Function-path fallback only; not required when only native search is used |
| Model | `webSearchMode` | `auto` (default) / `native` / `function` / `disabled` |

`webSearchMode` is stored in GlobalConfig JSON with serde defaults. Missing fields deserialize as `auto`. No SQLite migration is required.

## Route resolution

Input: master switch, fallback key availability, active provider kind, upstream model id (after redirects), and model mode.

Output (exactly one per turn):

- `Disabled` — master off, mode disabled, or no usable path
- `ProviderNative` — provider-native web search capability on the LLM request
- `FocoFunction` — executable Foco `web_search` tool (Tavily/Brave)

Rules:

- `auto` uses native only when the capability table confirms **Supported**; **Unknown** conservatively falls back to FocoFunction (if a key exists) or Disabled
- Explicit `native` requires a protocol that supports native tools; unknown model capability may save with a warning
- Explicit `function` requires an active fallback key at execution time
- The same turn never exposes both native and function `web_search`

## Provider × model matrix (regression)

Central capability (`provider_protocol_supports_native_web_search`) currently allows native only for **OpenAI Responses** adapters (`openai-responses`, `openai-responses-websocket`, `xai-responses`). Other protocols fall back to FocoFunction when a key is configured.

| Provider kind | Example model | Auto + key | Auto no key | Notes |
| --- | --- | --- | --- | --- |
| Master switch off | any | Disabled | Disabled | Overrides all modes except that mode still cannot enable search |
| `openai-responses` / WS | `gpt-4o`, `gpt-5*` | ProviderNative | ProviderNative | Confirmed native table |
| `xai-responses` | `grok-*` | ProviderNative | ProviderNative | Reuses OpenAI Responses adapter |
| `xai` (Chat Completions) | `grok-*` | FocoFunction | Disabled | Unchanged Chat Completions path |
| `anthropic` | Claude | FocoFunction | Disabled | Protocol not native-gated in Foco yet |
| `gemini` | Gemini | FocoFunction | Disabled | Protocol not native-gated in Foco yet |
| `openai` (Chat) / DeepSeek / Ollama | any | FocoFunction | Disabled | Function fallback only |
| Responses + unknown model | custom gateway id | FocoFunction | Disabled | Auto must not optimistically send native |

Explicit overrides: `native` on a non-OpenAIResp protocol → Disabled; `function` without key → Disabled; `disabled` → Disabled.

Regression tests (mock wire only, no live search):

- `foco-providers`: `web_search_route_matrix_covers_providers_modes_and_fallback`, wire fixture native vs function
- `foco-store`: config master/fallback independence matrix
- `foco-app`: `prepare_prompt_context_web_search_route_matrix`, `remote_sidecar_web_search_route_matrix`

## Tool kinds

Neutral tools carry an explicit `kind`:

- `Function` (default) — ordinary function tools, including Foco `web_search`
- `ProviderWebSearch` — provider-native search; serialized as the vendor native tool, never upgraded from a custom name alone

Native tools are injected into `provider_request.tools` only. They do **not** enter the Foco executable runtime catalog or agent allowlist.

## Local vs SSH

| Concern | Local | SSH sidecar |
| --- | --- | --- |
| Route decision | `resolve_web_search_route_for_turn` | Same resolver using secret-free runtime bundle |
| Function execution | Host process | Broker `web.search` on host |
| Native execution | Provider stream (`llm.stream`) | Same provider stream via broker |
| Secrets | Host only | Bundle carries `enabled`, `fallbackAvailable`, provider `kind` + redirects; **no** Tavily/Brave or provider API keys |

Remote `RemoteToolCatalog` registers broker `web_search` only on the FocoFunction path. ProviderNative never adds a broker route.

## Failure isolation

- Native search failures surface as provider/stream errors; Foco does not silently switch to Tavily/Brave mid-turn.
- Function path failures return the existing `web_search` tool error envelope; Foco does not upgrade them to a native tool retry.
- Stream normalization drops provider-native `WebSearch` tool-call chunks so they never enter the Tavily/Brave executor; function name `web_search` still executes normally.

## xAI Responses

Provider kind `xai-responses` reuses the OpenAI Responses adapter (default base `https://api.x.ai/v1/`). Existing `xai` Chat Completions is unchanged. OpenAI-only Agent headers and Fast/priority tiers apply only to OpenAI Responses, not xAI Responses.

## Smoke checklist (manual / audit)

Use Provider audit fixtures (no real search keys required for native wire inspection):

1. OpenAI Responses + `gpt-4o` + enabled, no Tavily key → request tools include native `type=web_search`; no Tavily/Brave secrets in body.
2. xAI Responses + `grok-*` → same native wire shape via Responses adapter.
3. OpenAI Chat / DeepSeek / Ollama + Tavily key → function tool `name=web_search` with `query` schema; execution hits Tavily/Brave only.
4. SSH remote: bundle JSON has no API keys; catalog omits broker `web_search` on ProviderNative; function path brokers `web.search` on host.
