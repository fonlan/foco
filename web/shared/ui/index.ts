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
 * - Alert: https://heroui.com/docs/react/components/alert.mdx
 * - Card: https://heroui.com/docs/react/components/card.mdx
 * - Tabs: https://heroui.com/docs/react/components/tabs.mdx
 * - Toast: https://heroui.com/docs/react/components/toast.mdx
 * - Badge: https://heroui.com/docs/react/components/badge.mdx
 * - Spinner: https://heroui.com/docs/react/components/spinner.mdx
 * - Skeleton: https://heroui.com/docs/react/components/skeleton.mdx
 */

export {
  Alert,
  Badge,
  Button,
  Card,
  Checkbox,
  Chip,
  CloseButton,
  Description,
  Dropdown,
  EmptyState,
  FieldError,
  Form,
  Header,
  Input,
  Label,
  ListBox,
  Menu,
  Modal,
  Popover,
  Select,
  Separator,
  Skeleton,
  Spinner,
  Surface,
  Switch,
  Tabs,
  TextArea,
  TextField,
  Toast,
  Tooltip,
  cn,
  tv,
  useOverlayState,
} from "@heroui/react";

export type {
  ButtonProps,
  CheckboxProps,
  Key,
  Selection,
  SwitchProps,
} from "@heroui/react";

export {
  formField,
  iconButton,
  overlayPanel,
  surfacePanel,
  toolbarButton,
} from "./variants";
export { ContextMenu } from "./ContextMenu";
export type { ContextMenuItem, ContextMenuProps } from "./ContextMenu";
