import { useEffect, type RefObject } from "react";

import type { BrowserRoute } from "../api/types";
import { currentBrowserRoute } from "../shared/browser-route";

export function useDocumentLanguage(language: string) {
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);
}

export function useDocumentTheme(theme: string) {
  useEffect(() => {
    document.documentElement.dataset.focoTheme = theme;
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

type PanelResizeEffectOptions = {
  isResizing: boolean;
  maxHeightRatio: number;
  maxWidth: number;
  minHeight: number;
  minWidth: number;
  mobileBreakpoint: number;
  onResizeEnd: () => void;
  setHeight: (value: number | ((current: number) => number)) => void;
  setWidth: (value: number | ((current: number) => number)) => void;
};

export function useRightPanelResizeEffect({
  isResizing,
  maxHeightRatio,
  maxWidth,
  minHeight,
  minWidth,
  mobileBreakpoint,
  onResizeEnd,
  setHeight,
  setWidth,
}: PanelResizeEffectOptions) {
  useEffect(() => {
    if (!isResizing) {
      return;
    }

    function handlePointerMove(event: PointerEvent) {
      if (window.innerWidth < mobileBreakpoint) {
        const maxHeight = Math.floor(window.innerHeight * maxHeightRatio);
        const nextHeight = window.innerHeight - event.clientY;
        setHeight(Math.min(Math.max(nextHeight, minHeight), maxHeight));
        return;
      }

      const nextWidth = window.innerWidth - event.clientX;
      setWidth(Math.min(Math.max(nextWidth, minWidth), maxWidth));
    }

    function handlePointerUp() {
      onResizeEnd();
    }

    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = window.innerWidth < mobileBreakpoint ? "row-resize" : "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);

    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [
    isResizing,
    maxHeightRatio,
    maxWidth,
    minHeight,
    minWidth,
    mobileBreakpoint,
    onResizeEnd,
    setHeight,
    setWidth,
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
