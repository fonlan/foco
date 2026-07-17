# Restricted readonly tools: external path contract

Locked contract for `read_file` / `find_files` / `search_text` when the target is outside the **execution** workspace root. Graph tools, `write_file`, `edit_file`, `run_command`, and the shared `normalize_workspace_path_text` / `resolve_workspace_path` resolvers are **out of scope** and must not gain external absolute-path access from this grant.

## Tool whitelist

Only these tools may receive `allow_external_read_access=true` and participate in ask-before-external-readonly:

| Tool | Target kind | Outside classifier |
|------|-------------|--------------------|
| `read_file` | file | `read_file_target_outside_workspace` |
| `find_files` | directory | `find_files_target_outside_workspace` |
| `search_text` | file **or** directory (rg-compatible) | `search_text_target_outside_workspace` |

Chat-level `allow_all` covers subsequent external readonly calls for the same three tools only. It never authorizes write/edit/run/graph.

Attachment exact allowlist remains **read_file-only** (canonical file equality; not parent dirs / siblings / prefix lookalikes).

## Path result semantics

| Case | Reported path |
|------|----------------|
| Internal (relative or absolute under execution root) | workspace-relative |
| External (authorized absolute outside execution root) | canonical absolute (safe to pass to `read_file`) |

Symlink escapes that canonicalize outside the execution root are treated as external (not internal).

## `search_text` specifics

1. **path** may be a file or a directory, same as ripgrep; not directory-only.
2. **match.path**: internal → relative; external → absolute.
3. **Snapshots** always write under the **execution** workspace `.foco/search-results/`, even when the search root is external. Snapshots are never written under the external root.
4. **fullResultPath** is always an execution-workspace-relative path (e.g. `.foco/search-results/...`). Reading it with `read_file` is an ordinary internal read and does **not** require external authorization.
5. **Continuation** pages load the in-workspace snapshot only. They do **not** re-run external authorization / `ask_question`. The original **query** and **path** must still match the snapshot binding; mismatched query/path → stable invalid continuation error. Empty / null / whitespace continuation means a fresh search (which re-requires authorization for external roots).
6. Unauthorized external first search fails **before** rg and **before** writing a snapshot.

## Runtime authorization order (app)

For whitelist tools only:

1. Non-external → no grant (`false`).
2. `search_text` non-empty continuation → no grant (snapshot path; tool continues without flag).
3. Target under shared workspace (isolated worktree trust) → auto-allow.
4. Skill read roots / configured skill → auto-allow.
5. Attachment exact allowlist → auto-allow **only** for `read_file`.
6. Chat `allow_all` → auto-allow for all three readonly tools in that chat.
7. Otherwise `ask_question` (allow once / allow all for chat / deny), with a per-chat serial prompt lock.

## Non-goals

- No graph tool external absolute paths.
- No `write_file` / `edit_file` / `run_command` external grants via `allow_all` or this flag.
- No widening of shared path normalizers used by write/command/graph.
- No durable DB authorization store or schema migration; session/chat grants only.
- No frontend protocol change for continuation or tool schemas beyond path description text already on the three tools.
