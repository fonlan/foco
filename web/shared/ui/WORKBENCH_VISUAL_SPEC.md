# Precision workbench visual contract

Foco is a dense local coding workbench, not a generic dashboard. The visual
signature is the **active work lane**: an active navigation, chat, task, or
selection surface receives the same restrained focus border and secondary
surface. It carries state through anatomy and spacing, not through a new
product accent on every panel.

## Semantic source of truth

Both themes are defined through the HeroUI semantic variables in `styles.css`.
Components consume `--background`, `--surface`, `--overlay`, `--foreground`,
`--muted`, `--border`, `--accent`, and `--focus`; feature CSS must not recreate
a parallel product palette.

| Role | Light | Dark | Use |
| --- | --- | --- | --- |
| Paper | HeroUI default near-white | HeroUI default near-black | application background |
| Surface | HeroUI white | HeroUI raised neutral | panels and cards |
| Ink | HeroUI eclipse | HeroUI snow | primary text |
| Muted | HeroUI neutral | HeroUI neutral | metadata and quiet affordances |
| Focus / active lane | HeroUI default accent | HeroUI default accent | keyboard focus, selected work, and primary actions |

HeroUI owns the standard light and dark semantic palette, including `accent`,
status colors, overlays, and focus states. Foco maps `--accent-soft` to HeroUI's
neutral `default` state for dense workbench selections and hover surfaces, so
only primary actions and focus affordances use the blue `accent`.
The terminal palette is intentionally outside this UI theme contract.

## Component anatomy and density

| Component | Required anatomy | Workbench treatment |
| --- | --- | --- |
| Button | HeroUI `Button`, `onPress` | 32px compact toolbar targets; quiet by default; pressed offset is subtle |
| TextField / TextArea | `TextField` + `Label` + control + `FieldError` | 32px compact control, 8px radius, visible keyboard ring |
| Select | `Select.Trigger`, `Select.Value`, `Select.Indicator`, `ListBox` | same control height as text fields; selection is keyboard-first |
| Checkbox / Switch | compound control and content | label is part of the accessible control name |
| Modal / menu / popover | HeroUI overlay container and dialog/menu | one border, quiet surface, deliberate elevation, focus restored on close |
| Card / Surface | HeroUI compound surface | base is flat; only overlays and explicitly raised content cast a shadow |

`toolbarButton`, `iconButton`, `formField`, `overlayPanel`, and `surfacePanel`
are intentionally small `tailwind-variants` recipes. They only encode repeated
density, width, focus, and elevation; they do not wrap or flatten HeroUI
compound components.

## Regression sample

`web/shared/ui/ui.test.tsx` is the anatomy fixture for the migration. It locks
button press/pending/disabled states, controlled field validation,
Switch/Checkbox labels, Select/ListBox keyboard selection, Modal dismissal and
focus restoration, ContextMenu behavior, and recipe output. It is the visual
and interaction baseline before each feature phase replaces native JSX.
