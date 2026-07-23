# HeroUI v3 component coverage and source contract

Run `npm run audit:heroui -w web` to enforce and generate the reviewable production JSX
inventory. The report is sorted by file and line, and maps every native
`button`, `input`, `textarea`, `select`, and hand-written `role="dialog"` to
the HeroUI v3 component required for its migration. It deliberately does not
inspect CSS variables: using `--surface` or `--accent` is not HeroUI component
coverage. It exits non-zero whenever a production control remains mapped for
migration, so it prevents regressions rather than merely reporting them. The
same output also lists every production TSX consumer of the shared HeroUI
barrel and the components it imports; that list is the final file-level
coverage report rather than a semantic-token inventory.

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
Every exception is additionally bound to one exact production file, one native
element and an explicit `data-heroui-exception` marker. A copied marker, an
unmarked file input, or an unknown exception key fails the source contract.
This gives later phases a line-level migration queue instead of a misleading
token-usage count.

## Final completion status

The source contract completes with zero migratable native controls. The only
approved production exceptions are deliberately centralised in
`scripts/audit-heroui-component-coverage.mjs` and must retain a reason,
accessibility owner and removal condition:

| Remaining native control | Location | Why it remains native |
| --- | --- | --- |
| browser file input | `web/shared/ui/settings-controls.tsx` | Browser permission/file-selection capability requires `input[type=file]`; its visible trigger is HeroUI `Button`. |
| modifier-aware submitter | `web/features/chat/ChatPanel.tsx` | Native form submitter preserves the existing submitter and modifier queue semantics. |
| plan drag handle | `web/features/context/ContextPanel.tsx` | Native draggable/DataTransfer flow is required for plan reordering. |
| composite chat tab | `web/App.tsx` | The tab combines scroll/context-menu/close behavior without nesting controls. |

HeroUI component coverage is real component anatomy, not semantic-token use:

| Component surface | Foco production coverage |
| --- | --- |
| Button / Checkbox / Switch | Shell, chat, terminal, Agent panels, statistics, file tools and settings |
| TextField / Input / TextArea | Composer, statistics filters, configuration and scheduled-task forms |
| Select / ListBox | Routing, settings, statistics filters and scheduled-task selectors |
| Modal / Drawer | Confirmation, details, configuration and side-panel flows |
| Menu / Dropdown / Tooltip / Alert / EmptyState | Shared UI primitives and feature-level overlays/status states |

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

| Component category | Current source of truth | Completion status | Regression policy |
| --- | --- | --- | --- |
| Button | Native-tag audit | Complete, except documented native cases | New native button fails the source contract |
| TextField / Input | Native-tag audit | Complete | New native input fails unless it is the approved browser file input |
| TextArea | Native-tag audit | Complete | New native textarea fails the source contract |
| Select / ListBox | Native-tag audit | Complete | New native select fails the source contract |
| Checkbox / Switch | Native-tag audit | Complete | New styled native input fails the source contract |
| Modal / Drawer | Native tag and `[role="dialog"]` audit | Complete | New hand-written dialog fails the source contract |
| Menu / Dropdown | Shared barrel and interaction fixture | Shared primitives available | Keep menu semantics in HeroUI primitives |
| Tabs | Shared barrel | Shared primitives available | Preserve the single documented composite-tab exception |
| Tooltip | Shared barrel | Shared primitives available | Use HeroUI for new affordances |
| Alert / Toast | Shared barrel | Shared primitives available | Preserve live-region semantics |
| Surface / Card | Shared barrel and visual spec | Shared primitives available | Do not count tokens alone as component migration |

The shared barrel exposes the listed v3 component surfaces. Its interaction and
DOM-anatomy contract is covered by `web/shared/ui/ui.test.tsx`.
