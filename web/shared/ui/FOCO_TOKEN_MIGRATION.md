# Foco theme notes

Production UI colors use HeroUI default semantic tokens directly: `--background`,
`--surface`, `--foreground`, `--accent`, `--border`, `--muted`, `--danger`, and
related tokens. The terminal is the sole intentional exception: its fixed palette
preserves terminal readability and is not part of the application theme.

Prefer HeroUI components and semantic tokens for new UI.

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
