# Foco Memory Graph Implementation Plan

This document is an implementation checklist for a local, Supermemory-like
memory graph in Foco. It is not the runtime memory store.

## Goals

- [ ] Provide local memory graph support with three scopes: global, workspace,
      and chat session.
- [ ] Store workspace and chat session memories in each workspace database:
      `<workspace>/.foco/foco.sqlite`.
- [ ] Store global memories locally under the Foco user profile, preferably in
      `%USERPROFILE%/.foco/memory.sqlite`, not inside `config.json`.
- [ ] Keep memories traceable to source evidence such as chat messages, tool
      calls, tool results, user notes, or imported documents.
- [ ] Inject relevant memories into prompt assembly through the same path used
      by chat streaming and context usage.
- [ ] Make memory extraction explicit, reviewable, auditable, and removable.

## Non-Goals for the First Version

- [ ] Do not call the hosted Supermemory API.
- [ ] Do not introduce a separate vector database in the MVP.
- [ ] Do not scan the entire workspace into memory automatically.
- [ ] Do not merge context compression snapshots with long-term memories.
- [ ] Do not silently save model-inferred facts without source evidence.
- [ ] Do not make memory extraction failures block the main chat response.

## Design Decisions

- [ ] Use fact nodes rather than a generic triple store.
- [ ] Use `memory_sources` for raw evidence and `memory_facts` for atomic,
      model-usable facts.
- [ ] Use `memory_edges` only for a small initial relation set:
      `updates`, `extends`, and `derives`.
- [ ] Use SQLite FTS5 for MVP search and ranking.
- [ ] Add embeddings later only after the FTS + graph model is verified.
- [ ] Treat session memory as chat-owned data. Deleting a chat should delete
      unpromoted session memories.
- [ ] Allow explicit promotion from chat session memory to workspace or global
      memory.

## Phase 0 - Scope and Contracts

- [x] Define `MemoryScope` values: `global`, `workspace`, `chat`.
- [x] Define allowed memory statuses: `pending`, `active`, `superseded`,
      `expired`, `rejected`.
- [x] Define allowed memory kinds: `preference`, `project_fact`,
      `project_decision`, `procedure`, `constraint`, `episode`, `user_note`.
- [x] Define source types: `chat_message`, `assistant_message`, `tool_call`,
      `tool_result`, `context_snapshot`, `manual_note`, `imported_document`.
- [x] Define that every non-manual extracted fact must reference at least one
      source.
- [x] Define that memory search excludes `superseded`, `expired`, and
      `rejected` by default.
- [ ] Define that context usage preview must never create or mutate memory data.

## Phase 1 - Storage

- [x] Add a global memory database path under the existing Foco root directory.
- [x] Add a new store module for shared memory storage primitives.
- [x] Add workspace schema migration v7 in `store/workspace.rs`.
- [x] Create `memory_sources`.
- [x] Create `memory_facts`.
- [x] Create `memory_fact_sources`.
- [x] Create `memory_edges`.
- [x] Create `memory_fts_data`.
- [x] Create `memory_fts_index` using SQLite FTS5.
- [x] Create `memory_profiles`.
- [x] Create `memory_extraction_jobs`.
- [x] Add indexes for scope, chat id, status, updated time, source lookup, and
      edge traversal.
- [x] Ensure workspace/session tables live in `<workspace>/.foco/foco.sqlite`.
- [x] Ensure global tables live in the global memory SQLite database.
- [x] Add migration tests for clean database creation.
- [x] Add migration tests for upgrading an existing workspace database.

## Phase 2 - Store API

- [ ] Implement insert/update/delete methods for memory sources.
- [ ] Implement insert/update/delete methods for memory facts.
- [ ] Implement source evidence linking.
- [ ] Implement fact status transitions.
- [ ] Implement fact promotion from chat to workspace and from workspace to
      global.
- [ ] Implement FTS upsert/delete helpers.
- [ ] Implement relation insertion with validation.
- [ ] Reject relation cycles only where they would break update chains.
- [ ] Implement profile read/write methods.
- [ ] Implement search by scope cascade.
- [ ] Add tests for source evidence requirements.
- [ ] Add tests for chat deletion cascading session memories.
- [ ] Add tests for workspace facts surviving chat deletion.

## Phase 3 - Manual API and UI

- [ ] Add memory settings to global config: enabled flag, extraction mode,
      retention settings, and optional dedicated extraction model id.
- [ ] Add API to list memories by scope.
- [ ] Add API to search memories.
- [ ] Add API to create manual memory.
- [ ] Add API to approve pending memory.
- [ ] Add API to reject pending memory.
- [ ] Add API to edit memory text and metadata.
- [ ] Add API to forget memory.
- [ ] Add API to promote memory.
- [ ] Add API to show memory sources.
- [ ] Add a Memory settings tab.
- [ ] Add a pending review queue.
- [ ] Add filters for global, workspace, and chat memories.
- [ ] Add a compact source/evidence viewer.
- [ ] Add a chat-side "memories used" view after prompt injection exists.

## Phase 4 - Retrieval and Prompt Injection

- [ ] Extend prompt preparation to retrieve memory context.
- [ ] Insert memory profile context immediately after the system prompt.
- [ ] Insert query-specific retrieved memories after the profile context.
- [ ] Keep memory injection inside the same path used by chat streaming and
      context usage.
- [ ] Add memory token accounting to context usage response.
- [ ] Add a fixed memory budget, initially 10-15 percent of available message
      tokens.
- [ ] Prioritize pinned facts, active profile facts, current chat facts,
      workspace facts, then global facts.
- [ ] Rank FTS matches by textual relevance, scope, recency, confidence,
      pinned status, and `is_latest`.
- [ ] Expand search results through graph edges within a small bounded depth.
- [ ] Exclude superseded facts unless the query explicitly asks for history.
- [ ] Add tests proving context usage does not persist memories.
- [ ] Add tests proving injected memory order is stable.
- [ ] Add tests proving memory token budget is enforced.

## Phase 5 - Extraction Jobs

- [ ] Queue memory extraction after a chat run completes.
- [ ] Do not run extraction during SSE streaming.
- [ ] Use the existing provider-neutral provider path for extraction.
- [ ] Build a strict JSON extraction schema.
- [ ] Require extracted facts to include scope suggestion, kind, fact text,
      confidence, relation candidates, and evidence references.
- [ ] Store extracted facts as `pending` by default.
- [ ] Allow direct `active` writes only for explicit user memory requests.
- [ ] Record extraction job status: `queued`, `running`, `completed`, `failed`.
- [ ] Surface extraction failures in the Memory UI.
- [ ] Redact secrets before storing extraction input/output.
- [ ] Add tests for malformed extraction JSON.
- [ ] Add tests for missing evidence rejection.
- [ ] Add tests for explicit "remember this" behavior.

## Phase 6 - Graph Maintenance

- [ ] Implement `updates` relation behavior.
- [ ] Mark older updated facts as `superseded` when the new fact is approved.
- [ ] Implement `extends` relation behavior without superseding the target.
- [ ] Implement `derives` relation behavior for facts inferred from sources or
      other facts.
- [ ] Maintain `is_latest` for update chains.
- [ ] Implement profile refresh from active facts.
- [ ] Keep profile updates deterministic and source-linked.
- [ ] Add retention handling for expired facts.
- [ ] Add hard delete for user-triggered forget.
- [ ] Add tests for update chains.
- [ ] Add tests for relation validation.
- [ ] Add tests for profile refresh.

## Phase 7 - Agent Tools

- [ ] Add `memory_search` tool.
- [ ] Add `memory_write` tool.
- [ ] Make `memory_write` create `pending` facts unless the user explicitly
      requested saving a memory.
- [ ] Add `memory_update` only if edit/promotion flows need agent access.
- [ ] Keep tool schemas compatible with strict OpenAI Responses requirements.
- [ ] Include `timeoutMs` in memory tools where required by current tool rules.
- [ ] Add tool result summaries that show scope, fact ids, and source counts.
- [ ] Add tests for tool schema strictness.
- [ ] Add tests for tool permission and scope behavior.

## Phase 8 - Verification and Release

- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo check --workspace`.
- [ ] Run focused store tests.
- [ ] Run focused app tests for prompt preparation and context usage.
- [ ] Run `npm run typecheck -w web`.
- [ ] Run focused web tests for Memory UI.
- [ ] Run `npm test` before considering the feature complete.
- [ ] Update `AGENTS.md` with durable memory graph behavior only after the
      implementation is verified.

## Open Questions

- [ ] Should automatic extraction be off by default or on with pending review?
- [ ] Should global memory be searchable from every workspace by default?
- [ ] Should workspace memory be exportable/importable with the workspace?
- [ ] Should explicit user commands such as "remember" bypass pending review?
- [ ] Should memory profiles be rebuilt synchronously on approval or queued as
      background jobs?
- [ ] Should memory extraction use the active chat model or a dedicated model
      setting?

## Acceptance Criteria

- [ ] A user can manually create global, workspace, and chat memory.
- [ ] Workspace and chat memories are persisted in the workspace SQLite file.
- [ ] Global memories are persisted locally outside `config.json`.
- [ ] Relevant memories are included in prompt assembly with token accounting.
- [ ] Context usage preview includes memory cost without mutating memory state.
- [ ] Extracted memories remain reviewable and traceable to sources.
- [ ] Superseded memories are not retrieved by default.
- [ ] Deleting a chat deletes unpromoted chat session memories.
- [ ] Forgetting a memory removes it from search, prompt injection, and profile
      generation.
