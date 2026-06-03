# Foco TODO

> Implementation rule: complete this file from top to bottom. Every completed feature must update the matching checkbox from `[ ]` to `[x]` in the same change.

> Scope rule: if a later checkbox is required to finish an earlier milestone, move that checkbox upward first instead of silently implementing out of order.

## 00. Repository Skeleton

- [x] Create the root Cargo workspace.
- [x] Create internal Rust crates without `src` directories: `app/main.rs`, `agent/lib.rs`, `providers/lib.rs`, `tools/lib.rs`, `graph/lib.rs`, `mcp/lib.rs`, and `store/lib.rs`.
- [x] Configure each crate with explicit `path` entries in its `Cargo.toml`.
- [x] Create the frontend workspace under `web/` without a `src` directory.
- [x] Add `web/main.tsx`, `web/App.tsx`, and `web/styles.css`.
- [x] Configure React 19, TypeScript, Vite, and Tailwind CSS.
- [x] Add `lucide-react` and use it as the only frontend icon source.
- [x] Add root development commands for backend, frontend, tests, and release build.
- [x] Add a root README with the local development workflow and release build command.
- [x] Verify that `cargo check` and the frontend typecheck can run on the empty skeleton.

## 01. Minimal Local App

- [ ] Start a Tokio runtime from `app/main.rs`.
- [ ] Start an Axum HTTP server bound to `127.0.0.1` on a configurable local port.
- [ ] Add `GET /api/health`.
- [ ] Serve a minimal frontend page from the local HTTP server.
- [ ] Add frontend API wiring that calls `GET /api/health`.
- [ ] Show the server health state in the frontend.
- [ ] Verify that the app can be started locally and reached in a browser.

## 02. First-Run Config And Logging

- [ ] Create `%USERPROFILE%\.foco` on first startup.
- [ ] Create `%USERPROFILE%\.foco\config.json` on first startup.
- [ ] Create `%USERPROFILE%\.foco\workspace` on first startup.
- [ ] Register `Default Workspace` pointing to `%USERPROFILE%\.foco\workspace`.
- [ ] Store global app settings, provider settings, model settings, MCP config, skill config, and workspace list in JSON.
- [ ] Add strict config validation and return explicit startup errors for invalid config.
- [ ] Add `tracing` based logging.
- [ ] Write logs to `%USERPROFILE%\.foco\logs`.
- [ ] Rotate logs by day using `foco-YYYY-MM-DD.log`.
- [ ] Redact provider secrets from normal logs.
- [ ] Add tests for first-run initialization and config loading.

## 03. Workspace Database Foundation

- [ ] Create `<workspace>\.foco` when a workspace is added.
- [ ] Create `<workspace>\.foco\foco.sqlite` for each workspace.
- [ ] Add SQLite migration execution.
- [ ] Add migration backup safety before schema upgrades.
- [ ] Add core tables for chats, messages, run events, tool calls, tool results, terminal sessions, and LLM request records.
- [ ] Add code graph tables for files, symbols, edges, references, imports, FTS data, file hashes, and parse status.
- [ ] Add repository helpers in `store` for workspace metadata, chats, messages, run events, and LLM request records.
- [ ] Add integration tests for workspace database creation and migrations.

## 04. Minimal Product Shell UI

- [ ] Build the main three-column layout.
- [ ] Build the left workspace sidebar.
- [ ] Show the `Default Workspace` in the sidebar.
- [ ] Allow creating a new workspace from the sidebar.
- [ ] Allow adding an existing directory as a workspace from the sidebar.
- [ ] Show expandable chat history under each workspace.
- [ ] Build the center chat panel.
- [ ] Render assistant messages on the left.
- [ ] Render user messages on the right.
- [ ] Add the send box layout.
- [ ] Add top-level navigation for chat, settings, and AI statistics.
- [ ] Add a placeholder collapsed terminal panel below the send box.
- [ ] Add a placeholder collapsed right git diff panel.

## 05. Model Metadata

- [ ] Fetch model metadata from `https://models.dev/api.json`.
- [ ] Cache fetched metadata at `%USERPROFILE%\.foco\models.dev.json`.
- [ ] Track metadata source and refresh time for every model.
- [ ] Extract context window, max output tokens, pricing, modality, tool support, and cache support where available.
- [ ] Add manual metadata refresh in settings.
- [ ] Block enabling a model when required limits are missing.
- [ ] Let users manually fill missing model limits in settings.
- [ ] Add unit tests for model metadata parsing.

## 06. Provider And Model Settings

- [ ] Add provider management UI in settings.
- [ ] Persist provider settings in `%USERPROFILE%\.foco\config.json`.
- [ ] Support OpenAI Responses provider config through `genai`.
- [ ] Support OpenAI Chat provider config through `genai`.
- [ ] Add provider connection testing.
- [ ] Add model management UI in settings.
- [ ] Allow one logical model to associate with multiple providers.
- [ ] Allow selecting the active provider for each model.
- [ ] Persist model thinking level options per model where supported.
- [ ] Show provider and model capability warnings without silently changing user choices.

## 07. LLM Request Audit Foundation

- [ ] Add `llm_requests` storage with request time, workspace, chat, provider, model, token fields, latency fields, status code, and final state.
- [ ] Add `llm_request_events` storage for streamed chunks and normalized events.
- [ ] Record complete non-secret request bodies.
- [ ] Record complete non-secret response bodies.
- [ ] Redact authorization headers and API keys before persistence.
- [ ] Normalize usage data into input, output, cache read, and cache write token fields.
- [ ] Calculate cache ratio from normalized token fields.
- [ ] Record request start time, first-token latency, total latency, status code, and final state.
- [ ] Add integration tests for request auditing with mocked provider calls.

## 08. Minimal Streaming Chat

- [ ] Build a provider-neutral request model around `genai`.
- [ ] Build a provider-neutral streaming event model for text deltas, usage, errors, and completion.
- [ ] Add streaming response handling for OpenAI Responses.
- [ ] Add streaming response handling for OpenAI Chat.
- [ ] Fail loudly when a provider cannot return required fields for an enabled model.
- [ ] Persist user messages and assistant messages in the workspace database.
- [ ] Stream markdown deltas into assistant messages in real time.
- [ ] Add model selection in the send box.
- [ ] Add thinking level selection in the send box.
- [ ] Populate LLM audit records for real chat requests.
- [ ] Verify a real prompt can stream into the chat UI.

## 09. Built-In Tool Runtime Foundation

- [ ] Define the internal tool schema format.
- [ ] Add tool call events to the provider-neutral streaming event model.
- [ ] Normalize tool call arguments before execution.
- [ ] Store complete tool inputs and outputs in the workspace database.
- [ ] Show compact tool call summaries in chat bubbles by default.
- [ ] Add expandable full tool output views.
- [ ] Add workspace path validation for file tools.
- [ ] Add `read_file`.
- [ ] Add `list_files`.
- [ ] Add `search_text` backed by ripgrep.
- [ ] Verify tool calls and results are visible inside the matching assistant message.

## 10. First Agent Loop

- [ ] Implement the main coding-agent run loop.
- [ ] Build prompt assembly from system rules, workspace context, chat history, and tool schemas.
- [ ] Add context budget calculation from model metadata.
- [ ] Preserve recent messages and active tool state during context packing.
- [ ] Execute multiple independent tool calls in parallel.
- [ ] Detect same-file write conflicts inside one model turn and fail the turn with a clear error.
- [ ] Continue a model run after tool results are returned.
- [ ] Make each agent run append-only in persisted events.
- [ ] Support cancellation of active model runs from the UI.
- [ ] Add run retry controls.
- [ ] Add unit tests for context budget calculation and tool conflict detection.

## 11. File Editing, Shell, And Git Diff

- [ ] Add `write_file`.
- [ ] Add `run_command`.
- [ ] Add `git_diff`.
- [ ] Add backend git status and diff APIs.
- [ ] Render current workspace diff in the right panel.
- [ ] Refresh diff after file writes and git-affecting commands.
- [ ] Add file-level diff navigation.
- [ ] Show clear errors when the workspace is not a git repository.
- [ ] Verify a chat run can read, edit, and show git diff for a file.

## 12. Context Compression

- [ ] Add structured context compression before the model context limit is reached.
- [ ] Store compression snapshots in the workspace database.
- [ ] Include compression snapshots in prompt assembly.
- [ ] Preserve active tool state across compression.
- [ ] Add tests for context packing with compression.

## 13. Terminal Panel

- [ ] Add backend terminal session management.
- [ ] Add PTY support for Windows.
- [ ] Add `xterm.js` frontend integration.
- [ ] Keep the terminal panel collapsed by default.
- [ ] Support terminal resize events.
- [ ] Persist terminal working directory per workspace session.
- [ ] Add terminal lifecycle cleanup on app shutdown.

## 14. Code Graph Index

- [ ] Add Tree-sitter parser setup for common programming languages.
- [ ] Add language detection by file extension and file content where needed.
- [ ] Add initial workspace scan using ignore rules.
- [ ] Add symbol extraction for functions, methods, classes, structs, enums, traits, variables, and imports.
- [ ] Add reference and edge extraction where the parser supports it.
- [ ] Add SQLite FTS indexes for files, symbols, and documentation text.
- [ ] Add incremental indexing based on file hash changes.
- [ ] Add filesystem watching with debounce.
- [ ] Reparse updated files and delete stale graph rows.
- [ ] Add integration tests for code graph incremental indexing.

## 15. Code Graph Agent Tools

- [ ] Add graph query tools for finding symbols.
- [ ] Add graph query tools for finding callers and callees.
- [ ] Add graph query tools for finding references.
- [ ] Add graph query tools for finding related files.
- [ ] Feed graph query results into the agent context as compact structured context.
- [ ] Update prompt assembly to include code graph context.
- [ ] Verify the agent can use graph tools to locate relevant code without a full-text search first.

## 16. MCP

- [ ] Integrate `rmcp` for MCP client support.
- [ ] Add MCP settings UI.
- [ ] Support stdio MCP servers.
- [ ] Support Streamable HTTP MCP servers.
- [ ] Persist MCP server definitions in global config.
- [ ] Start and stop MCP servers per workspace as configured.
- [ ] List MCP tools in the agent tool registry.
- [ ] Execute MCP tools through the same tool event pipeline as built-in tools.
- [ ] Show MCP server status and errors in settings.

## 17. Skills

- [ ] Auto-detect agent skills in configured skill directories.
- [ ] Store detected skills and enabled state in global config.
- [ ] Add skill management UI in settings.
- [ ] Include enabled skill instructions in prompt assembly.
- [ ] Refresh skill discovery from settings.
- [ ] Surface invalid skill files as explicit settings errors.

## 18. AI Statistics Page

- [ ] Add an AI statistics page.
- [ ] Show a table with request time, provider, model, input tokens, output tokens, cache read tokens, cache write tokens, cache ratio, request latency, first-token latency, status code, and details.
- [ ] Add filtering by workspace, chat, provider, model, status, and time range.
- [ ] Add a details drawer or modal for full request and response content.
- [ ] Show streamed chunks and normalized events in the details view.
- [ ] Add copy buttons for request and response details.
- [ ] Verify recorded real chat requests appear in the statistics page.

## 19. Tray And Windows Release

- [ ] Add a tray icon entry point for Windows.
- [ ] Make double-click startup minimize to the system tray instead of opening a GUI window.
- [ ] Open the browser to the local web UI from the tray menu.
- [ ] Add a tray menu item to quit Foco cleanly.
- [ ] Add graceful shutdown for HTTP server, file watchers, MCP processes, terminal sessions, and active agent runs.
- [ ] Add Windows release profile optimization.
- [ ] Build web assets before Rust release packaging.
- [ ] Embed built web assets into the Rust executable.
- [ ] Bundle required native assets into the single executable where possible.
- [ ] Verify the release executable starts without a console window on Windows.

## 20. Final Verification

- [ ] Verify first startup creates config, logs, default workspace, and workspace database.
- [ ] Verify the local HTTP UI is reachable after double-click startup.
- [ ] Verify settings can create providers, models, MCP servers, and skills.
- [ ] Verify a full chat run can stream text, call tools, show tool results, and persist history.
- [ ] Verify git diff, terminal, AI statistics, and code graph tools work in the same workspace.
- [ ] Add frontend tests for settings, chat, workspace sidebar, git diff panel, terminal panel, and AI statistics page.
- [ ] Add a smoke test for Windows release startup.
