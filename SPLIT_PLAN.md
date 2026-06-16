# Foco Phased Split Plan

## Goal

This document turns the current analysis into an execution plan for splitting the largest files in the repository without changing external behavior.

The primary targets are:

- `app/main.rs`
- `web/App.tsx`
- `tools/lib.rs`

The secondary targets are:

- `store/memory.rs`
- `store/workspace.rs`

The plan is intentionally phased. The first phases focus on low-risk moves that reduce file size and coupling without redesigning the system.

## Phase Checklist

- [x] Phase 0: Guardrails
- [x] Phase 1: Low-Risk Extraction
- [ ] Phase 2: Route and Panel Extraction
- [ ] Phase 3: Runtime and Prompt Subsystems
- [ ] Phase 4: Frontend State and Effect Extraction
- [ ] Phase 5: Tools and Store Cleanup

## Current Drivers

Observed hotspots during analysis:

- `app/main.rs` is the largest backend file and mixes entrypoint logic, route handlers, chat runtime, prompt assembly, tool execution, platform-specific code, and embedded tests.
- `web/App.tsx` is the largest frontend file and mixes DTO types, app shell state, routing helpers, pure utility functions, feature panels, and leaf components.
- `tools/lib.rs` combines tool registration and most tool implementations in one module.
- `store/memory.rs` and `store/workspace.rs` are large, but their responsibilities are more concentrated than the app entry files.

Important constraint:

- `cargo check --workspace --all-targets` is currently red because tests in `app/main.rs` have drifted from runtime signatures. Splitting must avoid widening that problem during early phases.

## Execution Principles

1. Preserve public behavior.
2. Do not change HTTP routes, JSON shapes, database schema, or user-visible semantics during split phases.
3. Prefer moving code over rewriting code.
4. Split by responsibility boundaries that already exist in the file, not by speculative architecture.
5. Keep the first phases low-risk and reversible.
6. Treat `app/main.rs` test extraction as the first backend priority.

## Target End State

### Backend target shape

```text
app/
  main.rs
  state.rs
  error.rs
  platform/
    mod.rs
    shutdown.rs
    tray_windows.rs
    native_browser.rs
    autostart_windows.rs
  http/
    mod.rs
    router.rs
    auth.rs
    assets.rs
    workspaces.rs
    settings.rs
    hooks.rs
    memory.rs
    chat.rs
    git.rs
    terminal.rs
    stats.rs
  runtime/
    mod.rs
    chat_run.rs
    chat_stream.rs
    subscriptions.rs
    tool_execution.rs
    tool_events.rs
    questions.rs
  prompt/
    mod.rs
    assembly.rs
    compression.rs
    injections.rs
    prompt_files.rs
    environment.rs
  memory_runtime/
    mod.rs
    retrieval.rs
    ranking.rs
    fts.rs
    summaries.rs
    expiration.rs
  graph_runtime/
    mod.rs
    bootstrap.rs
  tests/
    mod.rs
    tool_execution.rs
    prompt_context.rs
    memory_runtime.rs
    git_diff.rs
    chat_stats.rs
```

### Frontend target shape

```text
web/
  App.tsx
  main.tsx
  styles.css
  app/
    routes.ts
    constants.ts
    app-shell.tsx
    app-state.ts
    app-effects.ts
  api/
    types.ts
    client.ts
    chat.ts
    settings.ts
    memory.ts
    git.ts
    terminal.ts
    stats.ts
    hooks.ts
  features/
    chat/
      ChatPage.tsx
      ChatPanel.tsx
      ChatComposer.tsx
      ChatMessageList.tsx
      MessagePartBlock.tsx
      ReasoningBlock.tsx
      ToolCallBlock.tsx
      MarkdownContent.tsx
      MermaidDiagram.tsx
      chat-types.ts
      chat-helpers.ts
    settings/
      SettingsPage.tsx
      GeneralSettingsPanel.tsx
      ProvidersPanel.tsx
      ModelsPanel.tsx
      HooksPanel.tsx
      SkillsPanel.tsx
      settings-types.ts
    memory/
      MemoryPanel.tsx
      MemoryDialog.tsx
      memory-types.ts
    terminal/
      TerminalPanel.tsx
      TerminalSessionPane.tsx
      TerminalCommandButton.tsx
      terminal-types.ts
    git/
      GitPanel.tsx
      GitBranchDialog.tsx
      diff-parser.ts
    workspaces/
      WorkspaceSidebar.tsx
      WorkspaceDialog.tsx
      WorkspaceLogo.tsx
    stats/
      StatsPage.tsx
      AiStatsTable.tsx
      AiCharts.tsx
  shared/
    i18n.ts
    format.ts
    markdown.ts
    browser-route.ts
    ui/
      dialogs/
      nav/
      forms/
```

### Tools target shape

```text
tools/
  lib.rs
  definitions.rs
  errors.rs
  common/
    fs.rs
    paths.rs
    timeout.rs
    output.rs
  file_tools/
    mod.rs
    read_file.rs
    find_files.rs
    search_text.rs
  graph_tools/
    mod.rs
    find_symbols.rs
    find_callers.rs
    find_callees.rs
    find_references.rs
    related_files.rs
    explore.rs
  todo_tools/
    mod.rs
    create.rs
    update.rs
    get.rs
  command_tools/
    mod.rs
    run_command.rs
    sleep.rs
```

## Phase 0: Guardrails

- [x] Phase 0 complete

Goal: establish split rules before code starts moving.

Scope:

- No behavior changes.
- No schema changes.
- No route changes.
- No API contract changes.
- No opportunistic refactors inside moved functions.

Rules:

1. Move code first, simplify later.
2. Keep names stable during extraction unless a rename is required for visibility.
3. Export through old top-level modules when practical so callers do not all change at once.
4. Do not start with `execute_tool` or prompt assembly internals.

Exit condition:

- The first implementation wave is explicitly locked to file extraction, not redesign.

### Recorded backend guardrails for `app/main.rs`

Current extraction anchors already present in `app/main.rs`:

- [x] embedded tests under the bottom-level `#[cfg(test)] mod tests`
- [x] Windows auto-start helpers centered on `apply_auto_start_setting`
- [x] native browser picker helpers centered on `native_browser_probe`, `select_directory`, and `select_files`
- [x] static frontend asset serving centered on `WebAssets`, `verify_frontend_assets`, and `static_asset`

First-wave rules for the backend split:

1. Phase 1 may move only the four anchor groups listed above out of `app/main.rs`.
2. During extraction, keep function bodies and request/response behavior stable unless a visibility or import adjustment is required to compile.
3. Keep route wiring and handler call sites stable while code moves; module extraction is allowed, route redesign is not.
4. Treat test drift as a separate problem from file splitting: move embedded tests first, then repair test signatures only when a move requires it.

Explicit non-goals before Phase 1 completes:

- no `execute_tool` orchestration changes
- no prompt assembly or memory retrieval redesign
- no `AppState` reshaping beyond what extraction visibility requires
- no route tree, JSON contract, or persistence changes

## Phase 1: Low-Risk Extraction

- [x] Phase 1 complete

Goal: reduce file size quickly by moving pure or loosely-coupled sections first.

### Backend

Start with `app/main.rs`.

Move first:

- [x] Embedded tests into `app/tests/`.
- [x] Windows auto-start registry helpers into `app/platform/autostart_windows.rs`.
- [x] Native browser helper logic into `app/platform/native_browser.rs` or `app/http/workspaces.rs` support functions.
- [x] Static asset serving into `app/http/assets.rs`.

Why this first:

- These sections have relatively clear boundaries.
- They reduce noise in the main file without forcing runtime orchestration changes.
- Test extraction lowers merge and signature-drift pain immediately.

### Frontend

Start with `web/App.tsx`.

Move first:

- [x] DTO and form-state types into `web/api/types.ts`.
- [x] global constants into `web/app/constants.ts`.
- [x] browser route helpers into `web/shared/browser-route.ts`.
- [x] diff parsing helpers into `web/features/git/diff-parser.ts`.
- [x] i18n helpers into `web/shared/i18n.ts`.

Why this first:

- Most of these are pure declarations or pure functions.
- They shrink the file substantially with minimal runtime risk.
- They clarify later feature boundaries.

### Tools

Start with `tools/lib.rs`.

Move first:

- [x] tool definitions and schema registration into `tools/definitions.rs`.
- [x] shared runtime error and timeout helpers into `tools/errors.rs`.

Why this first:

- It decouples registration from implementation.
- It prepares the file for later per-tool extraction without changing public APIs.

Validation after each move:

- [x] `npm run build -w web` for frontend-only extractions.
- [x] `cargo check --workspace --lib` or the narrowest available Rust check if the moved code is outside known broken test paths.
- [ ] `git diff -- <path>` when no narrower executable validation is available.

Exit condition:

- [x] `app/main.rs` no longer contains embedded test bulk.
- [x] `web/App.tsx` no longer contains most DTO and helper blocks.
- [x] `tools/lib.rs` no longer mixes registration and every helper definition inline.

## Phase 2: Route and Panel Extraction

- [x] Phase 2 complete

Goal: turn giant entry files into composition roots.

### Backend

Split `app/main.rs` handlers by domain:

- [x] `app/http/auth.rs`
- [x] `app/http/workspaces.rs`
- [x] `app/http/settings.rs`
- [x] `app/http/hooks.rs`
- [x] `app/http/memory.rs`
- [x] `app/http/chat.rs`
- [x] `app/http/git.rs`
- [x] `app/http/terminal.rs`

Leave in `main.rs`:

- application startup
- state initialization
- route assembly
- top-level shared structs only if extraction would cause churn

Preferred order:

- [x] auth
- [x] assets
- [x] git
- [x] terminal
- [x] workspaces
- [x] hooks
- [x] settings
- [x] memory
- [x] chat

Reason for this order:

- auth/assets/git/terminal have smaller local surfaces.
- chat and settings are broader and should move after smaller extractions prove the module boundaries.

### Frontend

Split UI by feature.

Move next:

- [x] leaf chat rendering blocks into `web/features/chat/`
- [x] terminal panel pieces into `web/features/terminal/`
- [x] workspace shell components into `web/features/workspaces/`
- [x] dialog components into their relevant feature folders

Leave in `App.tsx`:

- top-level app state
- cross-feature orchestration
- page/layout switching

Preferred order:

- [x] leaf components with small prop surfaces
- [x] terminal feature
- [x] workspace shell
- [x] larger chat panel structure

Exit condition:

- [x] `app/main.rs` reads like an entrypoint, not an application dump.
- [x] `web/App.tsx` reads like an app shell, not a monolith of every component.

## Phase 3: Runtime and Prompt Subsystems

- [x] Phase 3 complete

Goal: isolate the highest-complexity backend logic after outer layers are already cleaner.

### Backend runtime extraction

Move from `app/main.rs` into `app/runtime/`:

- [x] chat run queueing
- [x] stream subscription management
- [x] tool call loop helpers
- [x] tool output event plumbing
- [x] question registry runtime helpers

Move from `app/main.rs` into `app/prompt/`:

- [x] prompt assembly
- [x] prompt injection helpers
- [x] environment prompt generation
- [x] context compression
- [x] prompt file loading

Move from `app/main.rs` into `app/memory_runtime/`:

- [x] memory retrieval
- [x] ranking and FTS query helpers
- [x] memory usage summaries
- [x] expiration helpers

Important rule:

- do not mix runtime extraction with semantic redesign in the same step.

Risk note:

- This is the highest-coupling phase. Only start after Phase 1 and Phase 2 stabilize the outer structure.

Exit condition:

- [x] prompt assembly, tool execution, and memory retrieval are no longer buried in `app/main.rs`.

## Phase 4: Frontend State and Effect Extraction

- [x] Phase 4 complete

Goal: reduce `web/App.tsx` to a coordinator rather than a state monolith.

Move from `web/App.tsx` into `web/app/` and feature hooks:

- [x] route-driven state transitions
- [x] persisted UI preferences
- [x] feature-specific side effects
- [x] local feature data loaders where they do not need global knowledge

Likely end-state roles:

- `App.tsx`: shell composition and root-level providers
- `app-state.ts`: app-wide state wiring
- `app-effects.ts`: cross-feature effects
- feature directories: feature-local state and rendering

Do not force this too early. The component and type layers should already be separated first.

Exit condition:

- [x] `App.tsx` is mostly composition and top-level wiring.

## Phase 5: Tools and Store Cleanup

- [x] Phase 5 complete

Goal: finish medium-priority splits after the primary app files are under control.

### Tools

Split implementations by tool family:

- [x] file tools
- [x] graph tools
- [x] todo tools
- [x] command tools

Keep the current public exports stable through `tools/lib.rs`.

### Store

Split `store/memory.rs` only after app-layer consumers are cleaner.

Suggested internal shape:

```text
store/memory/
  mod.rs
  schema.rs
  migrations.rs
  records.rs
  repository.rs
  queries.rs
  validation.rs
```

Split `store/workspace.rs` similarly:

```text
store/workspace/
  mod.rs
  schema.rs
  migrations.rs
  records.rs
  chats.rs
  messages.rs
  llm_audit.rs
  todo_graph.rs
  code_graph.rs
```

These are maintenance improvements, not the first-line priority.

Exit condition:

- [x] store modules are internally organized, but no higher-priority app split is blocked on them.

## Recommended First Implementation Wave

If work starts now, the recommended first wave is:

- [ ] Move `app/main.rs` tests out of the file.
- [ ] Move `web/App.tsx` types and pure helpers out of the file.
- [ ] Move `tools/lib.rs` definitions and common helpers out of the file.
- [ ] Split `app/main.rs` handlers into `app/http/` one domain at a time.
- [ ] Split leaf frontend components before touching top-level state.

This sequence gives the best ratio of size reduction to regression risk.

## Sections to Avoid Early

Do not start with these unless earlier phases are already done:

- backend `execute_tool` orchestration
- backend prompt assembly internals
- backend memory retrieval ranking pipeline
- frontend top-level state redesign
- store schema/repository redesign

These areas are valuable split targets, but they are also the places where a simple extraction can accidentally become a refactor.

## Progress Markers

The split effort is on track when the following become true:

- `app/main.rs` is primarily startup and route composition.
- `web/App.tsx` is primarily app-shell composition.
- tests are not embedded at the bottom of the backend entrypoint.
- pure helpers and DTO definitions are no longer trapped inside giant files.
- per-domain modules can be owned and reviewed independently.

## Success Criteria

The plan is successful when:

1. The largest files are no longer acting as catch-all containers.
2. New features can be added by touching one domain module instead of one giant file.
3. Small changes stop forcing unrelated merge conflicts in `app/main.rs` and `web/App.tsx`.
4. Validation can be run against narrower slices of the system.
5. Follow-up cleanup becomes optional instead of urgent.