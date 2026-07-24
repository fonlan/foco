import type { ReactNode } from "react";

import {
  Dropdown,
  Label,
  type Key,
} from "@heroui/react";

export type ContextMenuItem = {
  danger?: boolean;
  disabled?: boolean;
  icon?: ReactNode;
  id: string;
  label: string;
  textValue?: string;
};

export type ContextMenuProps = {
  "aria-label": string;
  /** Extra class on the popover surface (e.g. viewport-clamp measurement hooks). */
  className?: string;
  isOpen: boolean;
  items: ContextMenuItem[];
  left: number;
  onAction: (key: Key) => void;
  onOpenChange: (isOpen: boolean) => void;
  top: number;
  /**
   * When false, hide until the caller finishes viewport clamp measurement.
   * Default true.
   */
  positioned?: boolean;
};

/**
 * Right-click / long-press context menu built on HeroUI Dropdown + RAC Menu.
 * Positions via a fixed zero-size trigger at (left, top); React Aria owns
 * focus trap, Arrow/Home/End, Escape, and outside dismissal.
 *
 * Do not position the Menu with `position: fixed` — HeroUI's dropdown popover
 * uses `will-change-transform`, which makes fixed descendants resolve against
 * the popover instead of the viewport and pushes the menu off-screen.
 * Callers may clamp left/top and flip `positioned` after measuring the popover.
 */
export function ContextMenu({
  "aria-label": ariaLabel,
  className,
  isOpen,
  items,
  left,
  onAction,
  onOpenChange,
  positioned = true,
  top,
}: ContextMenuProps) {
  if (!isOpen) {
    return null;
  }

  return (
    <Dropdown isOpen={isOpen} onOpenChange={onOpenChange}>
      <Dropdown.Trigger
        aria-label={ariaLabel}
        className="pointer-events-none fixed size-px overflow-hidden opacity-0"
        style={{ left, top }}
      />
      <Dropdown.Popover
        className={`max-w-[min(18rem,calc(100vw-1rem))] ${className ?? ""}`.trim()}
        placement="bottom start"
        style={{
          visibility: positioned ? "visible" : "hidden",
        }}
      >
        <Dropdown.Menu
          aria-label={ariaLabel}
          className="max-h-[min(70vh,24rem)] overflow-y-auto"
          disabledKeys={items
            .filter((item) => item.disabled)
            .map((item) => item.id)}
          onAction={onAction}
        >
          {items.map((item) => (
            <Dropdown.Item
              id={item.id}
              key={item.id}
              textValue={item.textValue ?? item.label}
              variant={item.danger ? "danger" : undefined}
            >
              {item.icon ? (
                <span className="inline-flex size-4 shrink-0 items-center justify-center text-muted">
                  {item.icon}
                </span>
              ) : null}
              <Label>{item.label}</Label>
            </Dropdown.Item>
          ))}
        </Dropdown.Menu>
      </Dropdown.Popover>
    </Dropdown>
  );
}
