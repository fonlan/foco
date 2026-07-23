import {
  Bot,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  LoaderCircle,
  Route,
  Server,
} from "lucide-react";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";

import {
  MODEL_ROUTING_RESIZE_STEP_PX,
} from "../../app/constants";
import type {
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
} from "../../api/types";
import { useI18n } from "../../shared/i18n";
import {
  clampModelRoutingPanelHeight,
  modelRoutingPanelHeightBounds,
  panelHeightFromRatio,
  ratioFromPanelHeight,
  readModelRoutingExpanded,
  readModelRoutingHeightRatio,
  writeModelRoutingExpanded,
  writeModelRoutingHeightRatio,
} from "./model-routing-preferences";

type ResizeDragSnapshot = {
  availableHeight: number;
  startHeight: number;
  startY: number;
};

function findWorkspaceNavSibling(panel: HTMLElement | null): HTMLElement | null {
  if (!panel) {
    return null;
  }
  const previous = panel.previousElementSibling;
  if (previous instanceof HTMLElement && previous.classList.contains("workspace-nav")) {
    return previous;
  }
  return null;
}

function measureFlexibleStack(panel: HTMLElement | null): {
  availableHeight: number;
  panelHeight: number;
} | null {
  if (!panel) {
    return null;
  }
  const nav = findWorkspaceNavSibling(panel);
  const panelHeight = panel.getBoundingClientRect().height;
  if (nav) {
    const navHeight = nav.getBoundingClientRect().height;
    return {
      availableHeight: navHeight + panelHeight,
      panelHeight,
    };
  }
  // Fallback when sibling layout is missing (tests/isolated mounts).
  const parentHeight = panel.parentElement?.getBoundingClientRect().height ?? panelHeight;
  return {
    availableHeight: Math.max(panelHeight, parentHeight),
    panelHeight,
  };
}

export function ModelRoutingPanel({
  models,
  onRouteChange,
  providers,
}: {
  models: ConfiguredModelSummary[];
  onRouteChange: (
    modelId: string,
    providerId: string,
  ) => Promise<{ ok: true } | { ok: false; error: string }>;
  providers: ConfiguredProviderSummary[];
}) {
  const { t } = useI18n();
  const panelRef = useRef<HTMLElement | null>(null);
  const resizeDragRef = useRef<ResizeDragSnapshot | null>(null);
  const heightRatioRef = useRef(readModelRoutingHeightRatio());

  const [expanded, setExpanded] = useState(() => readModelRoutingExpanded(false));
  const [heightRatio, setHeightRatio] = useState(() => readModelRoutingHeightRatio());
  const [panelHeightPx, setPanelHeightPx] = useState<number | null>(null);
  const [availableHeightPx, setAvailableHeightPx] = useState(0);
  const [isResizing, setIsResizing] = useState(false);
  const [expandedModelIds, setExpandedModelIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [routingModelId, setRoutingModelId] = useState<string | null>(null);
  /** Pending route overrides applied before the network returns. */
  const [optimisticRoutes, setOptimisticRoutes] = useState<
    Record<string, string>
  >({});
  const [error, setError] = useState<string | null>(null);

  heightRatioRef.current = heightRatio;

  const providerById = useMemo(
    () => new Map(providers.map((provider) => [provider.id, provider])),
    [providers],
  );

  const sortedModels = useMemo(
    () =>
      [...models].sort((left, right) =>
        left.displayName.localeCompare(right.displayName),
      ),
    [models],
  );

  const applyHeightFromRatio = useCallback((ratio: number) => {
    const measured = measureFlexibleStack(panelRef.current);
    if (!measured || measured.availableHeight <= 0) {
      return;
    }
    const nextHeight = panelHeightFromRatio(ratio, measured.availableHeight);
    setAvailableHeightPx(measured.availableHeight);
    setPanelHeightPx(nextHeight);
  }, []);

  const commitPanelHeight = useCallback(
    (nextHeight: number, availableHeight: number) => {
      const clamped = clampModelRoutingPanelHeight(nextHeight, availableHeight);
      const nextRatio = ratioFromPanelHeight(clamped, availableHeight);
      setAvailableHeightPx(availableHeight);
      setPanelHeightPx(clamped);
      setHeightRatio(nextRatio);
      writeModelRoutingHeightRatio(nextRatio);
    },
    [],
  );

  useLayoutEffect(() => {
    if (!expanded) {
      setPanelHeightPx(null);
      return;
    }
    applyHeightFromRatio(heightRatioRef.current);
  }, [applyHeightFromRatio, expanded]);

  useEffect(() => {
    if (!expanded) {
      return;
    }

    function recompute() {
      // Drag freezes availableHeight; skip RO feedback while the pointer is active.
      if (resizeDragRef.current) {
        return;
      }
      applyHeightFromRatio(heightRatioRef.current);
    }

    const panel = panelRef.current;
    const parent = panel?.parentElement ?? null;
    const nav = findWorkspaceNavSibling(panel);
    const observer = new ResizeObserver(recompute);
    if (parent) {
      observer.observe(parent);
    }
    if (nav) {
      observer.observe(nav);
    }
    // Do not observe the panel itself: height commits would re-enter recompute.
    window.addEventListener("resize", recompute);
    window.addEventListener("orientationchange", recompute);

    return () => {
      observer.disconnect();
      window.removeEventListener("resize", recompute);
      window.removeEventListener("orientationchange", recompute);
    };
  }, [applyHeightFromRatio, expanded]);

  useEffect(() => {
    if (!isResizing) {
      return;
    }

    function handlePointerMove(event: PointerEvent) {
      const drag = resizeDragRef.current;
      if (!drag) {
        return;
      }
      const nextHeight =
        drag.startHeight + (drag.startY - event.clientY);
      commitPanelHeight(nextHeight, drag.availableHeight);
    }

    function handlePointerUp() {
      resizeDragRef.current = null;
      setIsResizing(false);
    }

    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "row-resize";
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
  }, [commitPanelHeight, isResizing]);

  function effectiveActiveProviderId(model: ConfiguredModelSummary) {
    return optimisticRoutes[model.id] ?? model.activeProviderId;
  }

  function clearOptimisticRoute(modelId: string) {
    setOptimisticRoutes((current) => {
      if (!(modelId in current)) {
        return current;
      }
      const next = { ...current };
      delete next[modelId];
      return next;
    });
  }

  function toggleExpanded() {
    setExpanded((current) => {
      const next = !current;
      writeModelRoutingExpanded(next);
      return next;
    });
  }

  function toggleModel(modelId: string) {
    setExpandedModelIds((current) => {
      const next = new Set(current);
      if (next.has(modelId)) {
        next.delete(modelId);
      } else {
        next.add(modelId);
      }
      return next;
    });
  }

  async function selectProvider(model: ConfiguredModelSummary, providerId: string) {
    if (
      effectiveActiveProviderId(model) === providerId ||
      routingModelId === model.id
    ) {
      return;
    }

    const provider = providerById.get(providerId);
    if (!provider?.enabled || !model.enabled || !model.canEnable) {
      return;
    }

    setError(null);
    setRoutingModelId(model.id);
    setOptimisticRoutes((current) => ({ ...current, [model.id]: providerId }));
    const result = await onRouteChange(model.id, providerId);
    setRoutingModelId((current) => (current === model.id ? null : current));
    // Success: parent settings are the source of truth. Failure: drop the
    // pending override so the UI rolls back to the previous route.
    clearOptimisticRoute(model.id);
    if (!result.ok) {
      setError(result.error);
    }
  }

  function handleResizePointerDown(event: ReactPointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const measured = measureFlexibleStack(panelRef.current);
    if (!measured || measured.availableHeight <= 0) {
      return;
    }
    const startHeight = clampModelRoutingPanelHeight(
      measured.panelHeight || panelHeightPx || panelHeightFromRatio(
        heightRatio,
        measured.availableHeight,
      ),
      measured.availableHeight,
    );
    resizeDragRef.current = {
      availableHeight: measured.availableHeight,
      startHeight,
      startY: event.clientY,
    };
    setAvailableHeightPx(measured.availableHeight);
    setPanelHeightPx(startHeight);
    event.currentTarget.setPointerCapture(event.pointerId);
    setIsResizing(true);
  }

  function handleResizeKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") {
      return;
    }
    event.preventDefault();
    const measured = measureFlexibleStack(panelRef.current);
    if (!measured || measured.availableHeight <= 0) {
      return;
    }
    const currentHeight =
      panelHeightPx ??
      panelHeightFromRatio(heightRatio, measured.availableHeight);
    const delta =
      event.key === "ArrowUp"
        ? MODEL_ROUTING_RESIZE_STEP_PX
        : -MODEL_ROUTING_RESIZE_STEP_PX;
    commitPanelHeight(currentHeight + delta, measured.availableHeight);
  }

  const heightBounds = modelRoutingPanelHeightBounds(availableHeightPx);
  const ariaValueMin = heightBounds.min;
  const ariaValueMax = heightBounds.max;
  const ariaValueNow =
    panelHeightPx ??
    (availableHeightPx > 0
      ? panelHeightFromRatio(heightRatio, availableHeightPx)
      : ariaValueMin);

  const panelStyle = (
    expanded && panelHeightPx != null
      ? {
          "--model-routing-panel-height": `${panelHeightPx}px`,
        }
      : undefined
  ) as CSSProperties | undefined;

  return (
    <section
      aria-label={t("Model routing")}
      className={`model-routing-panel ${expanded ? "model-routing-panel-expanded" : ""}`}
      data-expanded={expanded ? "true" : "false"}
      ref={panelRef}
      style={panelStyle}
    >
      {expanded ? (
        <div
          aria-label={t("Resize model routing panel")}
          aria-orientation="horizontal"
          aria-valuemax={ariaValueMax}
          aria-valuemin={ariaValueMin}
          aria-valuenow={ariaValueNow}
          className={`model-routing-resize-splitter ${
            isResizing ? "model-routing-resize-splitter-active" : ""
          }`}
          onKeyDown={handleResizeKeyDown}
          onPointerDown={handleResizePointerDown}
          role="separator"
          tabIndex={0}
        />
      ) : null}

      <button
        aria-controls="model-routing-tree"
        aria-expanded={expanded}
        className="model-routing-header"
        onClick={toggleExpanded}
        title={expanded ? t("Collapse model routing") : t("Expand model routing")}
        type="button"
      >
        <Route aria-hidden="true" className="size-3.5 shrink-0 text-[var(--accent-soft-foreground)]" />
        <span className="model-routing-header-label min-w-0 flex-1 truncate">
          {t("Model routing")}
        </span>
        {expanded ? (
          <ChevronDown aria-hidden="true" className="size-3.5 shrink-0 text-[var(--muted)]" />
        ) : (
          <ChevronRight aria-hidden="true" className="size-3.5 shrink-0 text-[var(--muted)]" />
        )}
      </button>

      {expanded ? (
        <div
          className="model-routing-body panel-scroll"
          id="model-routing-tree"
        >
          {error ? (
            <div
              className="model-routing-error"
              role="alert"
            >
              <CircleAlert aria-hidden="true" className="size-3.5 shrink-0" />
              <span className="min-w-0 flex-1">{error}</span>
            </div>
          ) : null}

          {sortedModels.length ? (
            <ul className="model-routing-tree" role="tree">
              {sortedModels.map((model) => {
                const isModelExpanded = expandedModelIds.has(model.id);
                const isModelBusy = routingModelId === model.id;
                const modelDisabled = !model.enabled || !model.canEnable;
                const activeProviderId = effectiveActiveProviderId(model);
                const activeProvider = activeProviderId
                  ? providerById.get(activeProviderId)
                  : null;
                const activeProviderLabel =
                  activeProvider?.name ?? activeProviderId ?? t("No route");
                const visibleProviderIds = model.providerIds.filter(
                  (providerId) => providerById.get(providerId)?.enabled !== false,
                );

                return (
                  <li
                    className={`model-routing-model ${modelDisabled ? "model-routing-item-muted" : ""}`}
                    data-model-id={model.id}
                    key={model.id}
                    role="treeitem"
                    aria-expanded={isModelExpanded}
                  >
                    <div className="model-routing-model-row">
                      <button
                        aria-expanded={isModelExpanded}
                        aria-label={t("Toggle providers for {name}", {
                          name: model.displayName,
                        })}
                        className="model-routing-model-toggle"
                        disabled={isModelBusy}
                        onClick={() => toggleModel(model.id)}
                        type="button"
                      >
                        {isModelExpanded ? (
                          <ChevronDown
                            aria-hidden="true"
                            className="size-3.5 shrink-0"
                          />
                        ) : (
                          <ChevronRight
                            aria-hidden="true"
                            className="size-3.5 shrink-0"
                          />
                        )}
                      </button>
                      <button
                        className="model-routing-model-button"
                        disabled={isModelBusy}
                        onClick={() => toggleModel(model.id)}
                        title={
                          modelDisabled
                            ? t("Model unavailable: {name}", {
                                name: model.displayName,
                              })
                            : `${model.displayName} · ${activeProviderLabel}`
                        }
                        type="button"
                      >
                        <span
                          aria-hidden="true"
                          className={`model-routing-route-dot ${
                            activeProviderId ? "model-routing-route-dot-active" : ""
                          }`}
                        />
                        <Bot aria-hidden="true" className="size-3.5 shrink-0" />
                        <span className="min-w-0 flex-1 truncate text-left">
                          <span className="block truncate font-medium">
                            {model.displayName}
                          </span>
                          <span className="block truncate text-[10px] font-medium leading-3 text-[var(--muted)]">
                            {activeProviderLabel}
                          </span>
                        </span>
                        {isModelBusy ? (
                          <LoaderCircle
                            aria-hidden="true"
                            className="size-3.5 shrink-0 animate-spin text-[var(--accent-soft-foreground)]"
                          />
                        ) : null}
                      </button>
                    </div>

                    {isModelExpanded ? (
                      <ul
                        className="model-routing-providers"
                        role="group"
                      >
                        {visibleProviderIds.map((providerId) => {
                          const provider = providerById.get(providerId);
                          const isActive = activeProviderId === providerId;
                          const providerDisabled =
                            modelDisabled || !provider || !provider.enabled;
                          const disabledReason = modelDisabled
                            ? t("Model is disabled")
                            : !provider
                              ? t("Provider not found")
                              : !provider.enabled
                                ? t("Provider is disabled")
                                : null;
                          const providerLabel = provider?.name ?? providerId;

                          return (
                            <li key={providerId} role="none">
                              <button
                                aria-current={isActive ? "true" : undefined}
                                aria-disabled={providerDisabled || isModelBusy}
                                className={`model-routing-provider-button ${
                                  isActive ? "model-routing-provider-active" : ""
                                } ${providerDisabled ? "model-routing-item-muted" : ""}`}
                                disabled={
                                  providerDisabled ||
                                  isModelBusy ||
                                  isActive
                                }
                                onClick={() => void selectProvider(model, providerId)}
                                role="radio"
                                aria-checked={isActive}
                                title={
                                  disabledReason
                                    ? `${providerLabel}: ${disabledReason}`
                                    : isActive
                                      ? t("Current route: {provider}", {
                                          provider: providerLabel,
                                        })
                                      : t("Route {model} via {provider}", {
                                          model: model.displayName,
                                          provider: providerLabel,
                                        })
                                }
                                type="button"
                              >
                                <Server
                                  aria-hidden="true"
                                  className="size-3.5 shrink-0"
                                />
                                <span className="min-w-0 flex-1 truncate text-left">
                                  {providerLabel}
                                </span>
                                {isActive ? (
                                  <CheckCircle2
                                    aria-hidden="true"
                                    className="size-3.5 shrink-0 text-[var(--accent-soft-foreground)]"
                                  />
                                ) : null}
                                {disabledReason ? (
                                  <span className="sr-only">{disabledReason}</span>
                                ) : null}
                              </button>
                            </li>
                          );
                        })}
                        {!visibleProviderIds.length ? (
                          <li className="model-routing-empty px-3 py-2 text-xs text-[var(--muted)]">
                            {t("No linked providers")}
                          </li>
                        ) : null}
                      </ul>
                    ) : null}
                  </li>
                );
              })}
            </ul>
          ) : (
            <div className="model-routing-empty px-3 py-3 text-xs text-[var(--muted)]">
              {t("No configured models")}
            </div>
          )}
        </div>
      ) : null}
    </section>
  );
}
