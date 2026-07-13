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
import { useMemo, useState } from "react";

import type {
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
} from "../../api/types";
import { useI18n } from "../../shared/i18n";
import {
  readModelRoutingExpanded,
  writeModelRoutingExpanded,
} from "./model-routing-preferences";

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
  const [expanded, setExpanded] = useState(() => readModelRoutingExpanded(false));
  const [expandedModelIds, setExpandedModelIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [routingModelId, setRoutingModelId] = useState<string | null>(null);
  /** Pending route overrides applied before the network returns. */
  const [optimisticRoutes, setOptimisticRoutes] = useState<
    Record<string, string>
  >({});
  const [error, setError] = useState<string | null>(null);

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

  return (
    <section
      aria-label={t("Model routing")}
      className={`model-routing-panel ${expanded ? "model-routing-panel-expanded" : ""}`}
      data-expanded={expanded ? "true" : "false"}
    >
      <button
        aria-controls="model-routing-tree"
        aria-expanded={expanded}
        className="model-routing-header"
        onClick={toggleExpanded}
        title={expanded ? t("Collapse model routing") : t("Expand model routing")}
        type="button"
      >
        <Route aria-hidden="true" className="size-3.5 shrink-0 text-teal-700" />
        <span className="model-routing-header-label min-w-0 flex-1 truncate">
          {t("Model routing")}
        </span>
        {expanded ? (
          <ChevronDown aria-hidden="true" className="size-3.5 shrink-0 text-stone-500" />
        ) : (
          <ChevronRight aria-hidden="true" className="size-3.5 shrink-0 text-stone-500" />
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
                          <span className="block truncate text-[10px] font-medium leading-3 text-stone-400">
                            {activeProviderLabel}
                          </span>
                        </span>
                        {isModelBusy ? (
                          <LoaderCircle
                            aria-hidden="true"
                            className="size-3.5 shrink-0 animate-spin text-teal-700"
                          />
                        ) : null}
                      </button>
                    </div>

                    {isModelExpanded ? (
                      <ul
                        className="model-routing-providers"
                        role="group"
                      >
                        {model.providerIds.map((providerId) => {
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
                                    className="size-3.5 shrink-0 text-teal-700"
                                  />
                                ) : null}
                                {disabledReason ? (
                                  <span className="sr-only">{disabledReason}</span>
                                ) : null}
                              </button>
                            </li>
                          );
                        })}
                        {!model.providerIds.length ? (
                          <li className="model-routing-empty px-3 py-2 text-xs text-stone-500">
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
            <div className="model-routing-empty px-3 py-3 text-xs text-stone-500">
              {t("No configured models")}
            </div>
          )}
        </div>
      ) : null}
    </section>
  );
}
