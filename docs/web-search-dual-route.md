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

## xAI Responses

Provider kind `xai-responses` reuses the OpenAI Responses adapter (default base `https://api.x.ai/v1/`). Existing `xai` Chat Completions is unchanged. OpenAI-only Agent headers and Fast/priority tiers apply only to OpenAI Responses, not xAI Responses.
