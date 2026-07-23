# Legacy `--foco-*` token inventory (Phase 1)

Color aliases in `web/styles.css` map directly to HeroUI default semantic
tokens. They are transitional and must be removed as modules migrate.

## Layout / typography (keep; not color)

| Token | Role |
| --- | --- |
| `--foco-header-height` | chrome height |
| `--foco-font-ui` | UI font size |
| `--foco-font-body` | body font size |
| `--foco-font-compact` | compact font size |
| `--foco-font-micro` | micro font size |
| `--foco-font-display` | display face stack |
| `--foco-line-body` | body line-height |

## Color aliases → HeroUI (delete after call-site migration)

| Legacy | HeroUI target |
| --- | --- |
| `--foco-canvas` | `--background` |
| `--foco-canvas-raised` | `--background-secondary` |
| `--foco-rail` | `--background-inverse` |
| `--foco-rail-soft` | `--surface-tertiary` |
| `--foco-sidebar` | `--surface` |
| `--foco-panel` | `--surface` |
| `--foco-panel-soft` | `--surface-secondary` |
| `--foco-panel-muted` | `--default` |
| `--foco-panel-selected` | `--accent-soft` |
| `--foco-border` | `--border` |
| `--foco-border-strong` | `--border-secondary` |
| `--foco-border-active` | `--accent` |
| `--foco-text` | `--foreground` |
| `--foco-text-muted` | `--muted` |
| `--foco-text-faint` | `--muted` |
| `--foco-text-subtle` | `--muted` |
| `--foco-foreground` | `--foreground` |
| `--foco-muted` | `--muted` |
| `--foco-muted-foreground` | `--muted` |
| `--foco-primary` | `--foreground` |
| `--foco-primary-hover` | `--accent` |
| `--foco-accent` | `--accent` |
| `--foco-accent-hover` | `--accent-hover` |
| `--foco-accent-strong` | `--accent-soft-foreground` |
| `--foco-accent-soft` | `--accent-soft` |
| `--foco-accent-muted` | `--accent-soft-hover` |
| `--foco-accent-line` | `--accent` |
| `--foco-user-surface` | `--accent-soft` |
| `--foco-user-border` | `--accent` |
| `--foco-user-avatar` | `--accent` |
| `--foco-success` | `--success` |
| `--foco-success-soft` | `--success-soft` |
| `--foco-warning` | `--warning` |
| `--foco-warning-soft` | `--warning-soft` |
| `--foco-error` | `--danger` |
| `--foco-error-soft` | `--danger-soft` |
| `--foco-danger` | `--danger` |
| `--foco-shadow-subtle` | `--surface-shadow` |
| `--foco-shadow-soft` | `--overlay-shadow` |
| `--foco-shadow-raised` | `--overlay-shadow` |
| `--foco-shadow-accent` | `--overlay-shadow` |
| `--foco-focus-ring` | focus mix from `--focus` |
| `--foco-sidebar-gradient` | `--surface` |
| `--foco-header-surface` | `--surface` |
| `--foco-row-hover` | `--surface-hover` |
| `--foco-chat-row-hover` | `--surface-hover` |
| `--foco-active-surface-gradient` | `--accent-soft` |
| `--foco-active-inset-shadow` | `none` |
| `--foco-toolbar-surface` | `--surface` |
| `--foco-main-panel-gradient` | `--background` |
| `--foco-row-border` | `--border` |
| `--foco-empty-surface` | `--surface-secondary` |
| `--foco-composer-shell-gradient` | `--background` |
| `--foco-composer-top-shadow` | `none` |
| `--foco-context-tabs-surface` | `--surface` |
| `--foco-graph-line` | `--border-secondary` |
| `--foco-settings-main-gradient` | `--background` |
| `--foco-settings-card-surface` | `--surface` |
| `--foco-json-surface` | `--surface` |
| `--foco-json-header` | `--surface-secondary` |
| `--foco-json-border` | `--border` |
| `--foco-json-text` | `--foreground` |
| `--foco-json-key` | `--accent-soft-foreground` |
| `--foco-json-string` | `--warning` |
| `--foco-json-number` | `--success` |
| `--foco-json-literal` | `--danger` |
| `--foco-json-punctuation` | `--muted` |

## Known call-site files (non-exhaustive)

- `web/styles.css` (majority of references)
- `web/features/skill-store/skill-store.css`
- `web/features/chat/ChatPanel.tsx`
- `web/features/agents/AgentTranscriptPanel.tsx`
- `web/features/context/ContextPanel.tsx`
- `web/app-shell.test.tsx`
