/**
 * HeroUI v3 barrel exports for Foco.
 *
 * Prefer importing from `web/shared/ui` so call sites share one entry.
 * Do not flatten compound components into prop bags.
 * Use `onPress` (not `onClick`) for pressable HeroUI controls.
 *
 * Docs (fetch before implementing new usage):
 * - Button: https://heroui.com/docs/react/components/button.mdx
 * - Modal: https://heroui.com/docs/react/components/modal.mdx
 * - TextField: https://heroui.com/docs/react/components/text-field.mdx
 * - Select: https://heroui.com/docs/react/components/select.mdx
 * - Switch: https://heroui.com/docs/react/components/switch.mdx
 * - Tooltip: https://heroui.com/docs/react/components/tooltip.mdx
 * - Dropdown: https://heroui.com/docs/react/components/dropdown.mdx
 * - Popover: https://heroui.com/docs/react/components/popover.mdx
 */

export {
  Button,
  CloseButton,
  Description,
  Dropdown,
  FieldError,
  Input,
  Label,
  ListBox,
  Modal,
  Popover,
  Select,
  Spinner,
  Switch,
  TextArea,
  TextField,
  Tooltip,
  cn,
  tv,
} from "@heroui/react";

export type { ButtonProps } from "@heroui/react";

export { iconButton } from "./variants";
