# Foco theme notes

Production UI colors use HeroUI default semantic tokens directly: `--background`,
`--surface`, `--foreground`, `--accent`, `--border`, `--muted`, `--danger`, and
related tokens. The terminal is the sole intentional exception: its fixed palette
preserves terminal readability and is not part of the application theme.

## HeroUI component completion contract

Foco's production UI has completed its HeroUI v3 component migration. Interactive
UI must use the shared `web/shared/ui` compound-component exports and React Aria
press semantics (`onPress` for `Button`); semantic tokens describe appearance but
never substitute for component migration.

`npm run audit:heroui -w web` is a required source-contract guard. It scans only
production TSX, reports file and line for each native interactive control or
hand-written dialog, and exits non-zero for anything not in its documented
central allowlist. The allowlist is intentionally limited to browser file input,
the modifier-aware native chat submitter, the native plan drag handle, and the
composite closable chat tab. Each entry has an accessibility owner and removal
condition in the audit output.

Use `Modal` for centered overlays and `Drawer` for edge-attached editors. Keep
Monaco, xterm, Recharts and Mermaid internal DOM untouched; migrate only Foco's
surrounding controls, toolbars and overlays.

## Layout / typography / safe-area

| Token | Role |
| --- | --- |
| `--foco-header-height` | chrome height |
| `--foco-touch-target` | minimum touch target (~44px) |
| `--foco-safe-top` / `right` / `bottom` / `left` | safe-area insets |
| `--foco-font-ui` | UI font size |
| `--foco-font-body` | body font size |
| `--foco-font-compact` | compact font size |
| `--foco-font-micro` | micro font size |
| `--foco-font-display` | display face stack |
| `--foco-line-body` | body line-height |
