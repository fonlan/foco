# HeroUI v3 component coverage baseline

Run `npm run audit:heroui -w web` to generate the reviewable production JSX
inventory. The report is sorted by file and line, and maps every native
`button`, `input`, `textarea`, `select`, and hand-written `role="dialog"` to
the HeroUI v3 component required for its migration. It deliberately does not
inspect CSS variables: using `--surface` or `--accent` is not HeroUI component
coverage.

## Classification contract

| Native JSX | Required HeroUI v3 target | Migration state |
| --- | --- | --- |
| `button` | `Button` with `onPress` | Must migrate |
| text-like `input` | `TextField` + `Label` + `Input` | Must migrate |
| `textarea` | `TextField` + `Label` + `TextArea` | Must migrate |
| `select` | `Select` + `Select.Trigger` + `ListBox` | Must migrate |
| `input[type=checkbox]` | `Checkbox` or `Switch` (intent decides) | Must migrate |
| `input[type=radio]` | `RadioGroup` + `Radio` | Must migrate |
| `role="dialog"` | `Modal.Backdrop` + `Modal.Container` + `Modal.Dialog` | Must migrate |
| `input[type=file]` | browser-native input, HeroUI `Button` trigger | Written exception |
| `button[data-heroui-exception="native-form-submit"]` | native submitter for modifier-aware form submission | Written exception |

The audit emits the exception reason, accessibility owner, and removal
condition next to every approved exception. The native submitter exception is
limited to controls that must preserve browser form submission plus modifier
semantics; all other buttons still migrate. No other native interactive element
is an approved exception without adding the same three fields to the audit.
This gives later phases a line-level migration queue instead of a misleading
token-usage count.

## Known look-alikes

`TextField` only counts when it comes from `web/shared/ui` (and therefore
HeroUI). Local components named `TextField`, including the ones in
`SettingsPanel` and `ScheduledTasksPage`, are ordinary local JSX and remain in
the audit until replaced. The report scans actual native DOM tags, so it cannot
mistake a local component name or a semantic color token for a completed
migration.

## Component surface tracked outside the native-tag report

The audit has a line-level source of truth for native controls. The following
matrix records the remaining component categories explicitly so a category with
zero native tags cannot disappear from the review.

| Component category | Current source of truth | Phase-1 status | Later migration / exception policy |
| --- | --- | --- | --- |
| Button | Native-tag audit | Line-level queue | Replace with HeroUI `Button` and `onPress` |
| TextField / Input | Native-tag audit | Line-level queue | Replace with HeroUI compound field |
| TextArea | Native-tag audit | Line-level queue | Replace with HeroUI `TextField` + `TextArea` |
| Select / ListBox | Native-tag audit | Line-level queue | Replace with HeroUI compound select |
| Checkbox / Switch | Native-tag audit | Line-level queue | Use control semantics, never a styled native input |
| Modal | Native tag and `[role="dialog"]` audit | Line-level queue | Replace hand-written overlays with HeroUI `Modal` anatomy |
| Menu / Dropdown | Shared barrel and interaction fixture | No native tag equivalent | Audit bespoke `role="menu"` during feature migration |
| Tabs | Shared barrel | No native tag equivalent | Audit bespoke tablists during feature migration |
| Tooltip | Shared barrel | No native tag equivalent | Audit title-based affordances during feature migration |
| Alert / Toast | Shared barrel | No native tag equivalent | Preserve live-region semantics during feature migration |
| Surface / Card | Shared barrel and visual spec | No native tag equivalent | Use compound HeroUI surfaces, not token-only divs |

The shared barrel exposes the listed v3 component surfaces. Its interaction and
DOM-anatomy contract is covered by `web/shared/ui/ui.test.tsx`.
