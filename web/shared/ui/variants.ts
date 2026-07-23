import { tv } from "tailwind-variants";

/**
 * Thin semantic extension for icon-only toolbar buttons.
 * Keeps HeroUI Button compound API; only adds repeated class recipes.
 */
export const iconButton = tv({
  base: "shrink-0 rounded-[var(--foco-control-radius)] focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-1 focus-visible:ring-offset-background",
  variants: {
    density: {
      compact: "size-8 min-w-8",
      comfortable: "size-9 min-w-9",
    },
  },
  defaultVariants: {
    density: "compact",
  },
});

/** A compact workbench toolbar button; behavior stays on HeroUI Button. */
export const toolbarButton = tv({
  base: "h-8 min-w-8 rounded-[var(--foco-control-radius)] px-2 text-xs font-medium shadow-none focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-1 focus-visible:ring-offset-background",
  variants: {
    emphasis: {
      quiet: "bg-transparent",
      active: "bg-surface-secondary text-foreground",
    },
  },
  defaultVariants: { emphasis: "quiet" },
});

/** Shared density for TextField/Input, TextArea and Select compound roots. */
export const formField = tv({
  base: "gap-1.5 text-xs text-foreground",
  variants: {
    density: {
      compact: "[&_[data-slot=input]]:min-h-8 [&_[data-slot=input]]:rounded-[var(--foco-control-radius)] [&_[data-slot=input]]:border [&_[data-slot=input]]:border-border [&_[data-slot=input]]:bg-surface [&_[data-slot=textarea]]:min-h-8 [&_[data-slot=textarea]]:rounded-[var(--foco-control-radius)] [&_[data-slot=textarea]]:border [&_[data-slot=textarea]]:border-border [&_[data-slot=textarea]]:bg-surface [&_[data-slot=select-trigger]]:min-h-8 [&_[data-slot=select-trigger]]:rounded-[var(--foco-control-radius)] [&_[data-slot=select-trigger]]:border-border [&_[data-slot=select-trigger]]:bg-surface",
      comfortable: "[&_[data-slot=input]]:min-h-9 [&_[data-slot=input]]:rounded-[var(--foco-control-radius)] [&_[data-slot=input]]:border [&_[data-slot=input]]:border-border [&_[data-slot=input]]:bg-surface [&_[data-slot=textarea]]:min-h-9 [&_[data-slot=textarea]]:rounded-[var(--foco-control-radius)] [&_[data-slot=textarea]]:border [&_[data-slot=textarea]]:border-border [&_[data-slot=textarea]]:bg-surface [&_[data-slot=select-trigger]]:min-h-9 [&_[data-slot=select-trigger]]:rounded-[var(--foco-control-radius)] [&_[data-slot=select-trigger]]:border-border [&_[data-slot=select-trigger]]:bg-surface",
    },
  },
  defaultVariants: { density: "compact" },
});

/** Width and elevation for Modal, Popover, Dropdown and Select.Popover. */
export const overlayPanel = tv({
  base: "border border-border bg-overlay text-overlay-foreground shadow-[var(--foco-overlay-shadow)]",
  variants: {
    width: {
      narrow: "w-[min(22rem,calc(100vw-1.5rem))]",
      form: "w-[min(30rem,calc(100vw-1.5rem))]",
      wide: "w-[min(42rem,calc(100vw-1.5rem))]",
    },
  },
  defaultVariants: { width: "form" },
});

/** Surface hierarchy for Card and Surface without flattening their anatomy. */
export const surfacePanel = tv({
  base: "border border-border text-foreground",
  variants: {
    level: {
      base: "bg-surface shadow-none",
      raised: "bg-overlay shadow-[var(--foco-surface-shadow)]",
      active: "border-focus bg-surface-secondary shadow-none",
    },
  },
  defaultVariants: { level: "base" },
});
