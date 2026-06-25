import {
  Activity,
  ArrowDown,
  ArrowUp,
  BarChart3,
  Bot,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Code2,
  Copy,
  Eye,
  LoaderCircle,
  MessageSquare,
  RefreshCw,
  SlidersHorizontal,
  X,
  type LucideIcon,
} from "lucide-react";
import {
  CSSProperties,
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import type {
  AiRequestAuditSummary,
  AiRequestDetailResponse,
  AiStatisticsSummary,
  AiStatsFilterState,
  AppLanguageId,
  JsonValue,
  SettingsResponse,
  Translate,
  WorkspaceSummary,
} from "../../api/types";
import { type AiStatsColumnId } from "../../app/constants";
import { useI18n } from "../../shared/i18n";
import {
  readAiStatsVisibleColumnIds,
  writeAiStatsVisibleColumnIds,
} from "./ai-stats-preferences";
import { useAiStatisticsData } from "./use-ai-statistics-data";

type AiStatsColumn = {
  cellClassName: string;
  headerClassName?: string;
  id: AiStatsColumnId;
  label: string;
  render: (request: AiRequestAuditSummary) => ReactNode;
};

export function ApiStatsPanel({
  onRoutePageChange,
  routePage,
  settings,
  workspaces,
}: {
  onRoutePageChange: (page: number) => void;
  routePage: number;
  settings: SettingsResponse | null;
  workspaces: WorkspaceSummary[];
}) {
  const { language, t } = useI18n();
  const {
    closeRequestDetail,
    copiedKey,
    copyAuditText,
    detail,
    detailError,
    error,
    filters,
    goToAuditPage: updateAuditPage,
    isLoading,
    isLoadingDetail,
    loadStats,
    openRequestDetail,
    selectedRequestId,
    setAuditPage,
    stats,
    updateAuditFilters,
  } = useAiStatisticsData(routePage);
  const [visibleColumnIds, setVisibleColumnIds] = useState<
    Set<AiStatsColumnId>
  >(readAiStatsVisibleColumnIds);
  const requests = stats?.requests ?? [];
  const summary = stats?.summary ?? emptyAiStatisticsSummary();
  const totalCount = stats?.totalCount ?? summary.totalRequests;
  const currentPage = stats?.page ?? positiveIntegerText(filters.page, 1);
  const pageSize = stats?.pageSize ?? positiveIntegerText(filters.pageSize, 20);
  const totalPages =
    stats?.totalPages ?? (totalCount ? Math.ceil(totalCount / pageSize) : 0);
  const paginationItems = auditPaginationItems(currentPage, totalPages);
  const pageStart = requests.length ? (currentPage - 1) * pageSize + 1 : 0;
  const pageEnd = requests.length
    ? Math.min(totalCount, pageStart + requests.length - 1)
    : 0;
  const selectedWorkspace =
    workspaces.find((workspace) => workspace.id === filters.workspaceId) ?? null;
  const chatOptions = (selectedWorkspace ? [selectedWorkspace] : workspaces)
    .flatMap((workspace) =>
      workspace.chats.map((chat) => ({
        label: selectedWorkspace
          ? chat.title
          : `${workspace.name} / ${chat.title}`,
        value: chat.id,
      })),
    );
  const providerOptions = auditOptions(
    settings?.providers.map((provider) => ({
      label: provider.name,
      value: provider.id,
    })) ?? [],
    requests.map((request) => request.providerId),
  );
  const modelOptions = auditOptions(
    settings?.configuredModels
      .map((model) => ({
        label: model.displayName,
        value: model.id,
      }))
      .sort((left, right) => left.label.localeCompare(right.label)) ?? [],
    requests.map((request) => request.modelId),
  );
  const statusOptions = auditOptions(
    ["succeeded", "failed", "running", "cancelled", "completed"].map((status) => ({
      label: auditStatusText(status, t),
      value: status,
    })),
    requests.map((request) => request.finalState),
    (status) => auditStatusText(status, t),
  );
  const totalInputTokens = summary.totalInputTokens;
  const totalOutputTokens = summary.totalOutputTokens;
  const aiStatsColumns: AiStatsColumn[] = [
    {
      cellClassName: "px-4 py-3 font-medium text-stone-900",
      id: "requestTime",
      label: t("Request time"),
      render: (request) =>
        formatAuditDate(request.requestStartedAt, language),
    },
    {
      cellClassName: "max-w-[10rem] truncate px-4 py-3 text-stone-700",
      id: "workspace",
      label: t("Workspace"),
      render: (request) => request.workspaceName,
    },
    {
      cellClassName: "max-w-[12rem] truncate px-4 py-3 text-stone-600",
      id: "chat",
      label: t("Chat"),
      render: (request) => request.chatTitle ?? request.chatId ?? "n/a",
    },
    {
      cellClassName:
        "max-w-[12rem] truncate px-4 py-3 font-mono text-xs text-stone-700",
      id: "provider",
      label: t("Provider"),
      render: (request) => request.providerId,
    },
    {
      cellClassName:
        "max-w-[14rem] truncate px-4 py-3 font-mono text-xs text-stone-700",
      id: "model",
      label: t("Model"),
      render: (request) => request.modelId,
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "inputTokens",
      label: t("Input tokens"),
      render: (request) =>
        formatNullableCompactNumber(request.inputTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "outputTokens",
      label: t("Output tokens"),
      render: (request) =>
        formatNullableCompactNumber(request.outputTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "cacheRead",
      label: t("Cache read"),
      render: (request) =>
        formatNullableCompactNumber(request.cacheReadTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "cacheWrite",
      label: t("Cache write"),
      render: (request) =>
        formatNullableCompactNumber(request.cacheWriteTokens, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "cacheRatio",
      label: t("Cache ratio"),
      render: (request) => formatPercent(request.cacheRatio, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "latency",
      label: t("Latency"),
      render: (request) =>
        formatNullableLatencySeconds(request.totalLatencyMs, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "firstToken",
      label: t("First token"),
      render: (request) =>
        formatNullableLatencySeconds(request.firstTokenLatencyMs, language),
    },
    {
      cellClassName: "px-4 py-3 text-right font-mono",
      headerClassName: "text-right",
      id: "statusCode",
      label: t("Status code"),
      render: (request) => formatNullableNumber(request.statusCode, language),
    },
    {
      cellClassName: "px-4 py-3",
      id: "status",
      label: t("Status"),
      render: (request) => (
        <span className={auditStatusClass(request.finalState)}>
          {auditStatusText(request.finalState, t)}
        </span>
      ),
    },
    {
      cellClassName: "px-4 py-3 text-right",
      headerClassName: "text-right",
      id: "details",
      label: t("Details"),
      render: (request) => (
        <button
          aria-label={t("View request details")}
          className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
          onClick={() => void openRequestDetail(request)}
          title={t("View request details")}
          type="button"
        >
          <Eye aria-hidden="true" className="size-4" />
        </button>
      ),
    },
  ];
  const visibleColumns = aiStatsColumns.filter((column) =>
    visibleColumnIds.has(column.id),
  );

  function goToAuditPage(page: number) {
    const nextPage = updateAuditPage(page, totalPages);
    onRoutePageChange(nextPage);
  }

  function updateFilters(update: Partial<AiStatsFilterState>) {
    updateAuditFilters(update);
    onRoutePageChange(1);
  }

  function toggleAiStatsColumn(columnId: AiStatsColumnId) {
    setVisibleColumnIds((current) => {
      if (current.has(columnId) && current.size === 1) {
        return current;
      }

      const next = new Set(current);
      if (next.has(columnId)) {
        next.delete(columnId);
      } else {
        next.add(columnId);
      }

      return next;
    });
  }

  useEffect(() => {
    writeAiStatsVisibleColumnIds(visibleColumnIds);
  }, [visibleColumnIds]);

  useEffect(() => {
    setAuditPage(routePage);
  }, [routePage, setAuditPage]);

  return (
    <div className="panel-scroll h-full min-h-0 flex-1 overflow-y-auto px-3 py-4 sm:px-5 sm:py-6">
      <div className="flex w-full min-w-0 flex-col gap-5">
        <section className="rounded-2xl border border-stone-200 bg-white/80 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex min-w-0 items-center gap-3">
              <span className="inline-flex size-10 items-center justify-center rounded-xl bg-teal-50 text-teal-800">
                <BarChart3 aria-hidden="true" className="size-5" />
              </span>
              <div className="min-w-0">
                <h2 className="truncate text-lg font-semibold text-stone-950">
                  {t("API details")}
                </h2>
                <p className="mt-1 truncate text-xs font-medium text-stone-500">
                  {filters.workspaceId
                    ? selectedWorkspace?.name ?? filters.workspaceId
                    : t("All workspaces")}
                </p>
              </div>
            </div>
            <button
              aria-label={t("Refresh request audit")}
              className="inline-flex size-10 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100"
              disabled={isLoading}
              onClick={() => void loadStats()}
              title={t("Refresh request audit")}
              type="button"
            >
              <RefreshCw
                aria-hidden="true"
                className="api-refresh-icon size-4"
                data-loading={isLoading ? "true" : "false"}
              />
            </button>
          </div>
        </section>

        <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <StatsCard
            icon={Activity}
            label={t("Total requests")}
            value={formatNumber(totalCount, language)}
          />
          <StatsCard
            icon={MessageSquare}
            label={t("Total tokens")}
            value={formatCompactNumber(summary.totalTokens, language)}
          />
          <StatsCard
            icon={Bot}
            label={t("Input tokens")}
            value={formatCompactNumber(totalInputTokens, language)}
          />
          <StatsCard
            icon={SlidersHorizontal}
            label={t("Average latency")}
            value={formatNullableLatencySeconds(summary.averageLatencyMs, language)}
          />
        </section>

        <section className="min-w-0 rounded-2xl border border-stone-200 bg-white/85 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
            <div>
              <h3 className="text-sm font-semibold text-stone-950">
                {t("Request audit")}
              </h3>
              <p className="mt-1 text-xs text-stone-500">
                {t("requests {count}", {
                  count: formatNumber(totalCount, language),
                })}
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <div className="text-xs text-stone-500">
                {t("Output tokens")}:{" "}
                {formatCompactNumber(totalOutputTokens, language)}
              </div>
              <details className="relative">
                <summary className="inline-flex h-9 cursor-pointer list-none items-center gap-2 rounded-lg border border-stone-200 bg-white px-3 text-xs font-semibold text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 [&::-webkit-details-marker]:hidden">
                  <SlidersHorizontal aria-hidden="true" className="size-4" />
                  {t("Columns")}
                </summary>
                <div className="absolute right-0 z-20 mt-2 w-56 rounded-xl border border-stone-200 bg-white p-2 shadow-[0_18px_42px_rgba(75,63,42,0.16)]">
                  {aiStatsColumns.map((column) => (
                    <label
                      className="flex min-h-9 cursor-pointer items-center gap-2 rounded-lg px-2 text-sm font-medium text-stone-700 hover:bg-stone-50"
                      key={column.id}
                    >
                      <input
                        checked={visibleColumnIds.has(column.id)}
                        className="size-4 rounded border-stone-300 text-teal-700 focus:ring-teal-200"
                        disabled={
                          visibleColumnIds.has(column.id) &&
                          visibleColumnIds.size === 1
                        }
                        onChange={() => toggleAiStatsColumn(column.id)}
                        type="checkbox"
                      />
                      <span className="min-w-0 truncate">{column.label}</span>
                    </label>
                  ))}
                </div>
              </details>
            </div>
          </div>
          <div className="grid gap-3 border-b border-stone-200 bg-stone-50/70 px-4 py-4 md:grid-cols-2 xl:grid-cols-7">
            <FilterSelect
              label={t("Workspace")}
              onChange={(value) =>
                updateFilters({
                  chatId: "",
                  workspaceId: value,
                })
              }
              options={workspaces.map((workspace) => ({
                label: workspace.name,
                value: workspace.id,
              }))}
              placeholder={t("All workspaces")}
              value={filters.workspaceId}
            />
            <FilterSelect
              label={t("Chat")}
              onChange={(value) => updateFilters({ chatId: value })}
              options={chatOptions}
              placeholder={t("All chats")}
              value={filters.chatId}
            />
            <FilterSelect
              label={t("Provider")}
              onChange={(value) => updateFilters({ providerId: value })}
              options={providerOptions}
              placeholder={t("All providers")}
              value={filters.providerId}
            />
            <FilterSelect
              label={t("Model")}
              onChange={(value) => updateFilters({ modelId: value })}
              options={modelOptions}
              placeholder={t("All models")}
              value={filters.modelId}
            />
            <FilterSelect
              label={t("Status")}
              onChange={(value) => updateFilters({ status: value })}
              options={statusOptions}
              placeholder={t("All statuses")}
              value={filters.status}
            />
            <label className="block">
              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                {t("Started after")}
              </span>
              <input
                className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                onChange={(event) =>
                  updateFilters({
                    startedAfter: event.target.value,
                  })
                }
                type="datetime-local"
                value={filters.startedAfter}
              />
            </label>
            <label className="block">
              <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                {t("Started before")}
              </span>
              <input
                className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                onChange={(event) =>
                  updateFilters({
                    startedBefore: event.target.value,
                  })
                }
                type="datetime-local"
                value={filters.startedBefore}
              />
            </label>
          </div>
          {error ? (
            <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
              {error}
            </div>
          ) : null}
          <div
            className="panel-scroll min-w-0 overflow-x-auto"
            onWheel={(event) => {
              // overflow-x-auto forces overflow-y to compute to auto in Chromium,
              // so this container traps vertical wheel events even though the table
              // never overflows vertically. Forward them to the scrollable ancestor.
              if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) {
                return;
              }
              const deltaUnit =
                event.deltaMode === 1
                  ? 16
                  : event.deltaMode === 2
                    ? event.currentTarget.clientHeight
                    : 1;
              let node: HTMLElement | null = event.currentTarget.parentElement;
              while (node) {
                const overflowY = window.getComputedStyle(node).overflowY;
                if (
                  /(auto|scroll)/.test(overflowY) &&
                  node.scrollHeight > node.clientHeight
                ) {
                  node.scrollTop += event.deltaY * deltaUnit;
                  event.preventDefault();
                  return;
                }
                node = node.parentElement;
              }
            }}
          >
            <table className="w-full min-w-max text-left text-sm">
              <thead className="border-b border-stone-200 bg-white text-xs font-semibold text-stone-500">
                <tr>
                  {visibleColumns.map((column) => (
                    <th
                      className={`whitespace-nowrap px-4 py-3 ${column.headerClassName ?? ""
                        }`}
                      key={column.id}
                    >
                      {column.label}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-stone-100">
                {requests.length ? (
                  requests.map((request) => (
                    <tr key={request.id} className="align-top hover:bg-teal-50/40">
                      {visibleColumns.map((column) => (
                        <td
                          className={`whitespace-nowrap ${column.cellClassName}`}
                          key={column.id}
                        >
                          {column.render(request)}
                        </td>
                      ))}
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td
                      className="px-4 py-10 text-center text-sm text-stone-500"
                      colSpan={visibleColumns.length}
                    >
                      {isLoading ? t("Loading...") : t("No recorded requests")}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-stone-200 px-4 py-3 text-sm">
            <div className="text-stone-500">
              {t("Showing {start}-{end} of {total}", {
                end: formatNumber(pageEnd, language),
                start: formatNumber(pageStart, language),
                total: formatNumber(totalCount, language),
              })}
            </div>
            <div className="flex flex-wrap items-center justify-end gap-3">
              <label className="flex items-center gap-2 text-xs font-semibold text-stone-500">
                <span>{t("Page size")}</span>
                <input
                  className="h-9 w-20 rounded-lg border border-stone-300 bg-white px-2 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  max={500}
                  min={1}
                  onChange={(event) =>
                    updateFilters({ pageSize: event.target.value })
                  }
                  type="number"
                  value={filters.pageSize}
                />
              </label>
              <nav
                aria-label={t("Request audit pagination")}
                className="flex items-center gap-1"
              >
                <button
                  aria-label={t("Previous page")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                  disabled={isLoading || currentPage <= 1}
                  onClick={() => goToAuditPage(currentPage - 1)}
                  title={t("Previous page")}
                  type="button"
                >
                  <ChevronLeft aria-hidden="true" className="size-4" />
                </button>
                {paginationItems.map((item, index) =>
                  item === "ellipsis" ? (
                    <span
                      aria-hidden="true"
                      className="inline-flex size-9 items-center justify-center text-stone-400"
                      key={`ellipsis-${index}`}
                    >
                      ...
                    </span>
                  ) : (
                    <button
                      aria-current={item === currentPage ? "page" : undefined}
                      aria-label={t("Go to page {page}", {
                        page: formatNumber(item, language),
                      })}
                      className={`inline-flex size-9 items-center justify-center rounded-lg border text-sm font-semibold shadow-sm ${item === currentPage
                          ? "border-teal-700 bg-teal-700 text-white"
                          : "border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800"
                        }`}
                      disabled={isLoading}
                      key={item}
                      onClick={() => goToAuditPage(item)}
                      title={t("Go to page {page}", {
                        page: formatNumber(item, language),
                      })}
                      type="button"
                    >
                      {formatNumber(item, language)}
                    </button>
                  ),
                )}
                <button
                  aria-label={t("Next page")}
                  className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-teal-200 hover:bg-teal-50 hover:text-teal-800 disabled:cursor-not-allowed disabled:bg-stone-100 disabled:text-stone-400"
                  disabled={
                    isLoading || totalPages === 0 || currentPage >= totalPages
                  }
                  onClick={() => goToAuditPage(currentPage + 1)}
                  title={t("Next page")}
                  type="button"
                >
                  <ChevronRight aria-hidden="true" className="size-4" />
                </button>
              </nav>
            </div>
          </div>
        </section>
      </div>
      {selectedRequestId ? (
        <AiRequestDetailDialog
          copiedKey={copiedKey}
          detail={detail}
          error={detailError}
          isLoading={isLoadingDetail}
          onClose={closeRequestDetail}
          onCopy={(key, text) => void copyAuditText(key, text)}
        />
      ) : null}
    </div>
  );
}

function FilterSelect({
  label,
  onChange,
  options,
  placeholder,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  options: { label: string; value: string }[];
  placeholder: string;
  value: string;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
        {label}
      </span>
      <select
        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        <option value="">{placeholder}</option>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function AiRequestDetailDialog({
  copiedKey,
  detail,
  error,
  isLoading,
  onClose,
  onCopy,
}: {
  copiedKey: string | null;
  detail: AiRequestDetailResponse | null;
  error: string | null;
  isLoading: boolean;
  onClose: () => void;
  onCopy: (key: string, text: string) => void;
}) {
  const { language, t } = useI18n();
  const request = detail?.request ?? null;

  return (
    <div
      className="fixed inset-0 z-50 flex min-h-0 items-center justify-center overflow-y-auto bg-stone-950/35 p-4 backdrop-blur-sm"
      onClick={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
      role="presentation"
    >
      <section
        aria-labelledby="ai-request-detail-title"
        aria-modal="true"
        className="flex h-[min(90dvh,56rem)] w-full max-w-6xl flex-col overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
        role="dialog"
      >
        <div className="flex shrink-0 items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div className="min-w-0">
            <h2
              className="truncate text-base font-semibold text-stone-950"
              id="ai-request-detail-title"
            >
              {t("Request details")}
            </h2>
            <p className="mt-1 truncate text-xs font-medium text-stone-500">
              {request ? request.id : t("Loading...")}
            </p>
          </div>
          <button
            aria-label={t("Close request details")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 shadow-sm hover:border-rose-200 hover:bg-rose-50 hover:text-rose-700"
            onClick={onClose}
            title={t("Close")}
            type="button"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">
          {error ? (
            <div className="mb-4 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
              {error}
            </div>
          ) : null}
          {isLoading ? (
            <div className="flex items-center gap-2 py-8 text-sm text-stone-500">
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              {t("Loading...")}
            </div>
          ) : null}
          {request ? (
            <div className="grid gap-4">
              <div className="grid gap-3 rounded-xl border border-stone-200 bg-stone-50/70 px-3 py-3 md:grid-cols-2 xl:grid-cols-4">
                <AuditMeta label={t("Workspace")} value={request.workspaceName} />
                <AuditMeta
                  label={t("Chat")}
                  value={request.chatTitle ?? request.chatId ?? "n/a"}
                />
                <AuditMeta label={t("Provider")} value={request.providerId} />
                <AuditMeta label={t("Model")} value={request.modelId} />
                <AuditMeta
                  label={t("Request time")}
                  value={formatAuditDate(request.requestStartedAt, language)}
                />
                <AuditMeta
                  label={t("Latency")}
                  value={formatNullableLatencySeconds(
                    request.totalLatencyMs,
                    language,
                  )}
                />
                <AuditMeta
                  label={t("First token")}
                  value={formatNullableLatencySeconds(
                    request.firstTokenLatencyMs,
                    language,
                  )}
                />
                <AuditMeta
                  label={t("Status")}
                  value={auditStatusText(request.finalState, t)}
                />
              </div>
              <div className="grid gap-4 xl:grid-cols-2">
                <AuditJsonBlock
                  copied={copiedKey === "request"}
                  label={t("Request body")}
                  onCopy={() =>
                    onCopy("request", auditJsonText(request.requestBody))
                  }
                  value={request.requestBody}
                />
                <AuditJsonBlock
                  copied={copiedKey === "response"}
                  label={t("Response body")}
                  onCopy={() =>
                    onCopy("response", auditJsonText(request.responseBody))
                  }
                  value={request.responseBody}
                />
              </div>
            </div>
          ) : null}
        </div>
      </section>
    </div>
  );
}

function AuditMeta({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <div className="text-xs font-semibold text-stone-500">{label}</div>
      <div className="mt-1 truncate text-sm font-medium text-stone-950">
        {value}
      </div>
    </div>
  );
}

function AuditJsonBlock({
  copied,
  label,
  onCopy,
  value,
}: {
  copied: boolean;
  label: string;
  onCopy: () => void;
  value: JsonValue | null;
}) {
  const { t } = useI18n();
  const jsonText = auditJsonText(value);
  const jsonValue = useMemo(
    () => (value === null ? null : normalizedJsonValue(value)),
    [value],
  );
  const collapsiblePaths = useMemo(
    () => collectJsonContainerPaths(jsonValue, "root"),
    [jsonValue],
  );
  const [collapsedPaths, setCollapsedPaths] = useState<Set<string>>(new Set());

  useEffect(() => {
    setCollapsedPaths(new Set());
  }, [jsonText]);

  const collapseAll = useCallback(() => {
    setCollapsedPaths(new Set(collapsiblePaths));
  }, [collapsiblePaths]);

  const expandAll = useCallback(() => {
    setCollapsedPaths(new Set());
  }, []);

  const togglePath = useCallback((path: string) => {
    setCollapsedPaths((current) => {
      const next = new Set(current);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  return (
    <section className="audit-json-block min-w-0">
      <div className="audit-json-header">
        <span className="audit-json-title">
          <Code2 aria-hidden="true" className="size-4" />
          <span>{label}</span>
        </span>
        <div className="audit-json-actions">
          <button
            aria-label={t("Collapse all {label}", { label })}
            className="audit-json-icon-button"
            disabled={collapsiblePaths.length === 0}
            onClick={collapseAll}
            title={t("Collapse all")}
            type="button"
          >
            <ArrowUp aria-hidden="true" className="size-3.5" />
          </button>
          <button
            aria-label={t("Expand all {label}", { label })}
            className="audit-json-icon-button"
            disabled={collapsedPaths.size === 0}
            onClick={expandAll}
            title={t("Expand all")}
            type="button"
          >
            <ArrowDown aria-hidden="true" className="size-3.5" />
          </button>
          <button
            aria-label={t("Copy {label}", { label })}
            className="audit-json-icon-button"
            onClick={onCopy}
            title={t("Copy {label}", { label })}
            type="button"
          >
            {copied ? (
              <CheckCircle2 aria-hidden="true" className="size-3.5" />
            ) : (
              <Copy aria-hidden="true" className="size-3.5" />
            )}
          </button>
        </div>
      </div>
      <div className="audit-json-code panel-scroll">
        <code>
          <JsonTreeNode
            collapsedPaths={collapsedPaths}
            depth={0}
            isLast
            onToggle={togglePath}
            path="root"
            value={jsonValue}
          />
        </code>
      </div>
    </section>
  );
}

function StatsCard({
  icon: Icon,
  label,
  value,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
}) {
  return (
    <article className="rounded-2xl border border-stone-200/80 bg-white p-5 shadow-sm">
      <div className="flex items-center gap-2 text-sm font-semibold text-stone-600">
        <Icon aria-hidden="true" className="size-4" />
        <span>{label}</span>
      </div>
      <div className="mt-4 font-mono text-3xl font-semibold text-stone-950">
        {value}
      </div>
    </article>
  );
}

function emptyAiStatisticsSummary(): AiStatisticsSummary {
  return {
    averageLatencyMs: null,
    failedRequests: 0,
    modelBreakdown: [],
    providerBreakdown: [],
    totalCacheReadTokens: 0,
    totalCacheWriteTokens: 0,
    totalInputTokens: 0,
    totalOutputTokens: 0,
    totalRequests: 0,
    totalTokens: 0,
    trend: [],
  };
}

function positiveIntegerText(value: string, fallback: number) {
  const parsed = Number(value);

  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : fallback;
}

function auditPaginationItems(
  currentPage: number,
  totalPages: number,
): Array<number | "ellipsis"> {
  if (totalPages <= 0) {
    return [];
  }

  const pages = new Set<number>([1, totalPages]);
  for (
    let page = Math.max(1, currentPage - 2);
    page <= Math.min(totalPages, currentPage + 2);
    page += 1
  ) {
    pages.add(page);
  }

  const sortedPages = Array.from(pages).sort((left, right) => left - right);
  const items: Array<number | "ellipsis"> = [];

  for (const page of sortedPages) {
    const previous = items[items.length - 1];
    if (typeof previous === "number" && page - previous > 1) {
      items.push("ellipsis");
    }
    items.push(page);
  }

  return items;
}

function datetimeLocalToRfc3339(value: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return "";
  }

  const date = new Date(trimmed);
  if (Number.isNaN(date.getTime())) {
    throw new Error(`invalid date time: ${value}`);
  }

  return date.toISOString().replace(/\.\d{3}Z$/, "Z");
}

function auditOptions(
  configuredOptions: { label: string; value: string }[],
  observedValues: string[],
  labelForObserved: (value: string) => string = (value) => value,
) {
  const optionsByValue = new Map<string, { label: string; value: string }>();

  for (const option of configuredOptions) {
    if (option.value) {
      optionsByValue.set(option.value, option);
    }
  }

  for (const value of observedValues) {
    if (value && !optionsByValue.has(value)) {
      optionsByValue.set(value, { label: labelForObserved(value), value });
    }
  }

  return Array.from(optionsByValue.values()).sort((left, right) =>
    left.label.localeCompare(right.label),
  );
}

function auditStatusText(status: string, t: Translate) {
  if (status === "succeeded" || status === "completed") {
    return t("succeeded");
  }

  if (status === "failed") {
    return t("failed");
  }

  if (status === "running") {
    return t("running");
  }

  if (status === "cancelled") {
    return t("cancelled");
  }

  return status;
}

function auditStatusClass(status: string) {
  const base = "inline-flex rounded-md px-2 py-1 text-xs font-semibold";

  if (status === "succeeded" || status === "completed") {
    return `${base} bg-teal-100 text-teal-800`;
  }

  if (status === "failed") {
    return `${base} bg-rose-100 text-rose-700`;
  }

  if (status === "cancelled") {
    return `${base} bg-amber-100 text-amber-800`;
  }

  return `${base} bg-stone-100 text-stone-600`;
}

function formatJsonValue(value: JsonValue) {
  return JSON.stringify(normalizedJsonValue(value), null, 2);
}

function normalizedJsonValue(value: JsonValue): JsonValue {
  let current = value;

  for (let index = 0; index < 4; index += 1) {
    if (typeof current !== "string") {
      return current;
    }

    const trimmed = current.trim();
    const looksLikeJson =
      trimmed.startsWith("{") ||
      trimmed.startsWith("[") ||
      trimmed.startsWith('"{') ||
      trimmed.startsWith('"[');
    if (!looksLikeJson) {
      return current;
    }

    try {
      const parsed = JSON.parse(trimmed);
      if (!isJsonValue(parsed)) {
        return current;
      }
      current = parsed;
    } catch {
      return current;
    }
  }

  return current;
}

function auditJsonText(value: JsonValue | null) {
  return value === null ? "null" : formatJsonValue(value);
}

function JsonTreeNode({
  collapsedPaths,
  depth,
  isLast,
  name,
  onToggle,
  path,
  value,
}: {
  collapsedPaths: Set<string>;
  depth: number;
  isLast: boolean;
  name?: string;
  onToggle: (path: string) => void;
  path: string;
  value: JsonValue | null;
}) {
  if (Array.isArray(value)) {
    return (
      <JsonContainerNode
        collapsedPaths={collapsedPaths}
        closeToken="]"
        depth={depth}
        entries={value.map((item, index) => [String(index), item] as [
          string,
          JsonValue,
        ])}
        isLast={isLast}
        name={name}
        onToggle={onToggle}
        openToken="["
        path={path}
        valueKind="array"
      />
    );
  }

  if (isObjectRecord(value)) {
    return (
      <JsonContainerNode
        collapsedPaths={collapsedPaths}
        closeToken="}"
        depth={depth}
        entries={Object.entries(value) as [string, JsonValue][]}
        isLast={isLast}
        name={name}
        onToggle={onToggle}
        openToken="{"
        path={path}
        valueKind="object"
      />
    );
  }

  return (
    <JsonLine depth={depth}>
      <JsonKey name={name} />
      <JsonPrimitive value={value} />
      {isLast ? null : <JsonPunctuation>,</JsonPunctuation>}
    </JsonLine>
  );
}

function JsonContainerNode({
  closeToken,
  collapsedPaths,
  depth,
  entries,
  isLast,
  name,
  onToggle,
  openToken,
  path,
  valueKind,
}: {
  closeToken: "]" | "}";
  collapsedPaths: Set<string>;
  depth: number;
  entries: [string, JsonValue][];
  isLast: boolean;
  name?: string;
  onToggle: (path: string) => void;
  openToken: "[" | "{";
  path: string;
  valueKind: "array" | "object";
}) {
  const { t } = useI18n();
  const isCollapsible = entries.length > 0;
  const isCollapsed = collapsedPaths.has(path);

  if (!isCollapsible) {
    return (
      <JsonLine depth={depth}>
        <JsonTogglePlaceholder />
        <JsonKey name={name} />
        <JsonPunctuation>{openToken}</JsonPunctuation>
        <JsonPunctuation>{closeToken}</JsonPunctuation>
        {isLast ? null : <JsonPunctuation>,</JsonPunctuation>}
      </JsonLine>
    );
  }

  return (
    <>
      <JsonLine depth={depth}>
        <button
          aria-label={
            isCollapsed ? t("Expand JSON node") : t("Collapse JSON node")
          }
          className="audit-json-node-toggle"
          onClick={() => onToggle(path)}
          type="button"
        >
          <ChevronRight
            aria-hidden="true"
            className={
              isCollapsed
                ? "audit-json-node-toggle-icon"
                : "audit-json-node-toggle-icon audit-json-node-toggle-icon-open"
            }
          />
        </button>
        <JsonKey name={name} />
        <JsonPunctuation>{openToken}</JsonPunctuation>
        {isCollapsed ? (
          <>
            <span className="audit-json-collapsed-marker">
              {jsonContainerSummary(valueKind, entries.length)}
            </span>
            <JsonPunctuation>{closeToken}</JsonPunctuation>
            {isLast ? null : <JsonPunctuation>,</JsonPunctuation>}
          </>
        ) : null}
      </JsonLine>
      {isCollapsed
        ? null
        : entries.map(([entryName, entryValue], index) => (
          <JsonTreeNode
            collapsedPaths={collapsedPaths}
            depth={depth + 1}
            isLast={index === entries.length - 1}
            key={jsonChildPath(path, entryName)}
            name={valueKind === "object" ? entryName : undefined}
            onToggle={onToggle}
            path={jsonChildPath(path, entryName)}
            value={entryValue}
          />
        ))}
      {isCollapsed ? null : (
        <JsonLine depth={depth}>
          <JsonTogglePlaceholder />
          <JsonPunctuation>{closeToken}</JsonPunctuation>
          {isLast ? null : <JsonPunctuation>,</JsonPunctuation>}
        </JsonLine>
      )}
    </>
  );
}

function JsonLine({
  children,
  depth,
}: {
  children: ReactNode;
  depth: number;
}) {
  return (
    <span
      className="audit-json-line"
      style={{ "--audit-json-depth": depth } as CSSProperties}
    >
      {children}
    </span>
  );
}

function JsonKey({ name }: { name?: string }) {
  if (typeof name === "undefined") {
    return null;
  }

  return (
    <>
      <span className="audit-json-token audit-json-token-key">
        {JSON.stringify(name)}
      </span>
      <JsonPunctuation>: </JsonPunctuation>
    </>
  );
}

function JsonPrimitive({ value }: { value: JsonValue | null }) {
  if (typeof value === "string") {
    return (
      <span className="audit-json-token audit-json-token-string">
        {JSON.stringify(value)}
      </span>
    );
  }

  if (typeof value === "number") {
    return (
      <span className="audit-json-token audit-json-token-number">
        {String(value)}
      </span>
    );
  }

  return (
    <span className="audit-json-token audit-json-token-literal">
      {value === null ? "null" : String(value)}
    </span>
  );
}

function JsonPunctuation({ children }: { children: ReactNode }) {
  return (
    <span className="audit-json-token audit-json-token-punctuation">
      {children}
    </span>
  );
}

function JsonTogglePlaceholder() {
  return <span aria-hidden="true" className="audit-json-node-toggle-spacer" />;
}

function collectJsonContainerPaths(value: JsonValue | null, path: string) {
  const paths: string[] = [];

  if (Array.isArray(value)) {
    if (value.length > 0) {
      paths.push(path);
    }
    value.forEach((item, index) => {
      paths.push(...collectJsonContainerPaths(item, jsonChildPath(path, index)));
    });
    return paths;
  }

  if (isObjectRecord(value)) {
    const entries = Object.entries(value) as [string, JsonValue][];
    if (entries.length > 0) {
      paths.push(path);
    }
    entries.forEach(([key, item]) => {
      paths.push(...collectJsonContainerPaths(item, jsonChildPath(path, key)));
    });
  }

  return paths;
}

function jsonChildPath(path: string, segment: string | number) {
  return `${path}/${encodeURIComponent(String(segment))}`;
}

function jsonContainerSummary(kind: "array" | "object", count: number) {
  return kind === "array" ? `... ${count} items ` : `... ${count} keys `;
}

function formatAuditDate(value: string, language: AppLanguageId = "en") {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(language, {
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
    second: "2-digit",
    year: "numeric",
  }).format(date);
}

function formatNullableNumber(
  value: number | null,
  language: AppLanguageId = "en",
) {
  return value === null ? "n/a" : formatNumber(value, language);
}

function formatNullableCompactNumber(
  value: number | null,
  language: AppLanguageId = "en",
) {
  return value === null ? "n/a" : formatCompactNumber(value, language);
}

function formatNullableLatencySeconds(
  value: number | null,
  language: AppLanguageId = "en",
) {
  if (value === null) {
    return "n/a";
  }

  return `${new Intl.NumberFormat(language, {
    maximumFractionDigits: 2,
  }).format(value / 1000)} s`;
}

function formatLatencySeconds(value: number, language: AppLanguageId = "en") {
  return `${new Intl.NumberFormat(language, {
    maximumFractionDigits: 0,
  }).format(value / 1000)} s`;
}

function formatPercent(value: number | null, language: AppLanguageId = "en") {
  if (value === null) {
    return "n/a";
  }

  return new Intl.NumberFormat(language, {
    maximumFractionDigits: 1,
    style: "percent",
  }).format(value);
}

function formatNumber(value: number, language: AppLanguageId = "en") {
  return new Intl.NumberFormat(language).format(value);
}

function formatCompactNumber(value: number, _language: AppLanguageId = "en") {
  return new Intl.NumberFormat("en", {
    maximumFractionDigits: 1,
    notation: "compact",
  }).format(value);
}

function isJsonValue(value: unknown): value is JsonValue {
  if (
    value === null ||
    typeof value === "boolean" ||
    typeof value === "number" ||
    typeof value === "string"
  ) {
    return true;
  }

  if (Array.isArray(value)) {
    return value.every(isJsonValue);
  }

  if (isObjectRecord(value)) {
    return Object.values(value).every(isJsonValue);
  }

  return false;
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
