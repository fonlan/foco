import { tv } from "tailwind-variants";

/**
 * Thin semantic extension for icon-only toolbar buttons.
 * Keeps HeroUI Button compound API; only adds repeated class recipes.
 */
export const iconButton = tv({
  base: "shrink-0",
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
