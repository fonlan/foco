import { useEffect, type RefObject } from "react";

import type { BrowserRoute } from "../api/types";
import { currentBrowserRoute } from "../shared/browser-route";

export function useDocumentLanguage(language: string) {
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);
}

const THEME_COLOR_LIGHT = "#f7f7f7";
const THEME_COLOR_DARK = "#181818";

/** Sync HeroUI v3 theme contract: class + data-theme + color-scheme. */
export function useDocumentTheme(theme: string) {
  useEffect(() => {
    const root = document.documentElement;
    const next = theme === "dark" ? "dark" : "light";

    root.classList.remove("light", "dark");
    root.classList.add(next);
    root.dataset.theme = next;
    root.style.colorScheme = next;

    const themeColor = document.querySelector('meta[name="theme-color"]');
    if (themeColor) {
      themeColor.setAttribute(
        "content",
        next === "dark" ? THEME_COLOR_DARK : THEME_COLOR_LIGHT,
      );
    }
  }, [theme]);
}

type InitialBrowserRouteEffectOptions = {
  canUseApp: boolean;
  hasAppliedInitialBrowserRouteRef: RefObject<boolean>;
  initialBrowserRoute: BrowserRoute;
  isLoading: boolean;
  onApplyRoute: (route: BrowserRoute) => void;
  onReplaceRoute: (route: BrowserRoute) => void;
};

export function useInitialBrowserRouteEffect({
  canUseApp,
  hasAppliedInitialBrowserRouteRef,
  initialBrowserRoute,
  isLoading,
  onApplyRoute,
  onReplaceRoute,
}: InitialBrowserRouteEffectOptions) {
  useEffect(() => {
    if (!canUseApp || isLoading || hasAppliedInitialBrowserRouteRef.current) {
      return;
    }

    hasAppliedInitialBrowserRouteRef.current = true;
    onApplyRoute(initialBrowserRoute);
    onReplaceRoute(initialBrowserRoute);
  }, [
    canUseApp,
    hasAppliedInitialBrowserRouteRef,
    initialBrowserRoute,
    isLoading,
    onApplyRoute,
    onReplaceRoute,
  ]);
}

export function useBrowserPopState(
  applyRouteRef: RefObject<(route: BrowserRoute) => void>,
) {
  useEffect(() => {
    function handlePopState() {
      applyRouteRef.current(currentBrowserRoute());
    }

    window.addEventListener("popstate", handlePopState);
    return () => {
      window.removeEventListener("popstate", handlePopState);
    };
  }, [applyRouteRef]);
}

type PanelResizeDragSession = {
  stacked: boolean;
  startClientX: number;
  startClientY: number;
  startHeight: number;
  startWidth: number;
};

export type { PanelResizeDragSession };

type PanelResizeEffectOptions = {
  dragSessionRef: RefObject<PanelResizeDragSession | null>;
  isResizing: boolean;
  maxHeightRatio: number;
  maxWidth: number;
  minHeight: number;
  minWidth: number;
  /** Apply height during drag without React state (smooth CSS var update). */
  onHeightPreview: (value: number) => void;
  /** Apply width during drag without React state (smooth CSS var update). */
  onWidthPreview: (value: number) => void;
  /** Width below which the panel is stacked under main and resizes by height. */
  stackedBreakpoint: number;
  onResizeEnd: (finalSize: {
    height: number;
    stacked: boolean;
    width: number;
  }) => void;
};

export function useRightPanelResizeEffect({
  dragSessionRef,
  isResizing,
  maxHeightRatio,
  maxWidth,
  minHeight,
  minWidth,
  onHeightPreview,
  onWidthPreview,
  stackedBreakpoint,
  onResizeEnd,
}: PanelResizeEffectOptions) {
  useEffect(() => {
    if (!isResizing) {
      return;
    }

    const session = dragSessionRef.current;
    if (!session) {
      return;
    }

    // Freeze layout axis and start metrics for this pointer session.
    const stacked = session.stacked;
    const startClientX = session.startClientX;
    const startClientY = session.startClientY;
    const startHeight = session.startHeight;
    const startWidth = session.startWidth;
    let lastHeight = startHeight;
    let lastWidth = startWidth;

    function clampHeight(value: number) {
      const maxHeight = Math.floor(window.innerHeight * maxHeightRatio);
      return Math.min(Math.max(value, minHeight), maxHeight);
    }

    function clampWidth(value: number) {
      return Math.min(Math.max(value, minWidth), maxWidth);
    }

    function handlePointerMove(event: PointerEvent) {
      if (stacked) {
        lastHeight = clampHeight(
          startHeight + startClientY - event.clientY,
        );
        onHeightPreview(lastHeight);
        return;
      }

      lastWidth = clampWidth(startWidth + startClientX - event.clientX);
      onWidthPreview(lastWidth);
    }

    function handlePointerUp() {
      onResizeEnd({ height: lastHeight, width: lastWidth, stacked });
    }

    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = stacked ? "row-resize" : "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    window.addEventListener("pointercancel", handlePointerUp);

    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
    };
  }, [
    dragSessionRef,
    isResizing,
    maxHeightRatio,
    maxWidth,
    minHeight,
    minWidth,
    onHeightPreview,
    onWidthPreview,
    stackedBreakpoint,
    onResizeEnd,
  ]);
}

type SidebarResizeEffectOptions = {
  isResizing: boolean;
  onPointerMove: (clientX: number) => void;
  onResizeEnd: () => void;
};

export function useSidebarResizeEffect({
  isResizing,
  onPointerMove,
  onResizeEnd,
}: SidebarResizeEffectOptions) {
  useEffect(() => {
    if (!isResizing) {
      return;
    }

    function handlePointerMove(event: PointerEvent) {
      onPointerMove(event.clientX);
    }

    function handlePointerUp() {
      onResizeEnd();
    }

    document.body.style.cursor = "col-resize";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);

    return () => {
      document.body.style.cursor = "";
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [isResizing, onPointerMove, onResizeEnd]);
}
