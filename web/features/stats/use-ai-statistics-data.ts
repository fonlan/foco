import { useCallback, useEffect, useRef, useState } from "react";

import { errorMessage, requestJson } from "../../shared/api-client";
import type {
  AiRequestAuditSummary,
  AiRequestDetailResponse,
  AiStatisticsResponse,
  AiStatsFilterState,
} from "../../api/types";

const AI_STATS_POLL_INTERVAL_MS = 5000;
const AI_REQUEST_DETAIL_POLL_INTERVAL_MS = 1000;

type StatsLoadSource = "auto" | "user";

export function emptyAiStatsFilters(page = 1): AiStatsFilterState {
  return {
    chatId: "",
    modelId: "",
    page: String(positivePage(page)),
    pageSize: "20",
    providerId: "",
    requestIds: "",
    requestKind: "",
    startedAfter: defaultStartedAfterDatetimeLocal(),
    startedBefore: "",
    status: "",
    workspaceId: "",
  };
}

export function useAiStatisticsData(
  initialPage = 1,
  initialFilters?: Partial<AiStatsFilterState>,
) {
  const [filters, setFilters] = useState<AiStatsFilterState>(() =>
    aiStatsFiltersFromInitialState(initialPage, initialFilters),
  );
  const [stats, setStats] = useState<AiStatisticsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [selectedRequestId, setSelectedRequestId] = useState<string | null>(null);
  const [detail, setDetail] = useState<AiRequestDetailResponse | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [autoRefreshEnabled, setAutoRefreshEnabled] = useState(true);
  const selectedRequestRef = useRef<AiRequestAuditSummary | null>(null);
  const filtersRef = useRef(filters);
  const statsRef = useRef<AiStatisticsResponse | null>(null);
  const isStatsRequestInFlightRef = useRef(false);
  const isDetailRequestInFlightRef = useRef(false);
  const shouldReloadStatsAfterCurrentRequestRef = useRef(false);
  const pendingStatsReloadSourceRef = useRef<StatsLoadSource>("user");
  const autoRefreshEnabledRef = useRef(true);
  const copiedTimerRef = useRef<number | null>(null);

  filtersRef.current = filters;
  statsRef.current = stats;
  autoRefreshEnabledRef.current = autoRefreshEnabled;

  const loadRequestDetail = useCallback(
    async (request: AiRequestAuditSummary, showLoading: boolean) => {
      if (isDetailRequestInFlightRef.current) {
        return;
      }
      isDetailRequestInFlightRef.current = true;
      setDetailError(null);
      if (showLoading) {
        setDetail(null);
        setCopiedKey(null);
        setIsLoadingDetail(true);
      }

      try {
        const data = await requestJson<AiRequestDetailResponse>(
          `/api/workspaces/${encodeURIComponent(
            request.workspaceId,
          )}/ai-statistics/${encodeURIComponent(request.id)}`,
        );
        if (selectedRequestRef.current?.id === request.id) {
          setDetail(data);
          selectedRequestRef.current = data.request;
        }
      } catch (requestError) {
        if (selectedRequestRef.current?.id === request.id) {
          setDetailError(errorMessage(requestError));
        }
      } finally {
        isDetailRequestInFlightRef.current = false;
        if (showLoading) {
          setIsLoadingDetail(false);
        }
      }
    },
    [],
  );

  const loadStats = useCallback(
    async (
      showLoading = true,
      queueIfInFlight = false,
      source: StatsLoadSource = "user",
    ) => {
      if (isStatsRequestInFlightRef.current) {
        if (queueIfInFlight) {
          if (!shouldReloadStatsAfterCurrentRequestRef.current) {
            shouldReloadStatsAfterCurrentRequestRef.current = true;
            pendingStatsReloadSourceRef.current = source;
          } else if (source === "user") {
            // User-driven reloads take precedence over auto-queued ones.
            pendingStatsReloadSourceRef.current = "user";
          }
        }
        return;
      }

      // Drop auto-originated loads that race past pause.
      if (source === "auto" && !autoRefreshEnabledRef.current) {
        return;
      }

      isStatsRequestInFlightRef.current = true;
      if (showLoading) {
        setIsLoading(true);
      }
      setError(null);

      try {
        const query = aiStatsQuery(filtersRef.current);
        const data = await requestJson<AiStatisticsResponse>(
          `/api/ai-statistics${query ? `?${query}` : ""}`,
        );
        setStats(data);
        const selectedRequest = selectedRequestRef.current;
        if (selectedRequest) {
          const refreshedRequest =
            data.requests.find(
              (request) =>
                request.id === selectedRequest.id &&
                request.workspaceId === selectedRequest.workspaceId,
            ) ?? selectedRequest;
          selectedRequestRef.current = refreshedRequest;
          const shouldRefreshDetail =
            selectedRequest.finalState === "running" ||
            refreshedRequest.finalState === "running" ||
            refreshedRequest.finalState !== selectedRequest.finalState;
          // Auto list polls must not start a new detail request after pause.
          // User-driven loads (filters, open detail follow-up via list) still may.
          if (
            shouldRefreshDetail &&
            (source === "user" || autoRefreshEnabledRef.current)
          ) {
            void loadRequestDetail(refreshedRequest, false);
          }
        }
      } catch (requestError) {
        setError(errorMessage(requestError));
      } finally {
        isStatsRequestInFlightRef.current = false;
        if (showLoading) {
          setIsLoading(false);
        }
        if (shouldReloadStatsAfterCurrentRequestRef.current) {
          shouldReloadStatsAfterCurrentRequestRef.current = false;
          const pendingSource = pendingStatsReloadSourceRef.current;
          pendingStatsReloadSourceRef.current = "user";
          if (
            isAiStatsDocumentVisible() &&
            (pendingSource === "user" || autoRefreshEnabledRef.current)
          ) {
            void loadStats(false, false, pendingSource);
          }
        }
      }
    },
    [loadRequestDetail],
  );

  useEffect(() => {
    if (!isAiStatsDocumentVisible()) {
      return;
    }

    void loadStats(true, true);
  }, [filters, loadStats]);

  const initialFiltersKey = JSON.stringify(initialFilters ?? {});
  useEffect(() => {
    setFilters((current) => {
      const next = aiStatsFiltersFromInitialState(initialPage, initialFilters);
      return sameAiStatsFilters(current, next) ? current : next;
    });
  }, [initialPage, initialFiltersKey]);

  useEffect(() => {
    if (!autoRefreshEnabled) {
      return;
    }

    let disposed = false;
    let timeoutId: number | null = null;
    const schedule = () => {
      if (
        disposed ||
        !autoRefreshEnabledRef.current ||
        !isAiStatsDocumentVisible() ||
        timeoutId !== null
      ) {
        return;
      }
      timeoutId = window.setTimeout(async () => {
        timeoutId = null;
        await loadStats(false, false, "auto");
        schedule();
      }, AI_STATS_POLL_INTERVAL_MS);
    };
    const handleVisibilityChange = () => {
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
        timeoutId = null;
      }
      if (!autoRefreshEnabledRef.current) {
        return;
      }
      if (isAiStatsDocumentVisible()) {
        void loadStats(statsRef.current === null, false, "auto");
        schedule();
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    schedule();
    return () => {
      disposed = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [autoRefreshEnabled, loadStats]);

  useEffect(() => {
    const selectedRequest = selectedRequestRef.current;
    if (
      !autoRefreshEnabled ||
      !selectedRequest ||
      detail?.request.finalState !== "running"
    ) {
      return;
    }

    let disposed = false;
    let timeoutId: number | null = null;
    const schedule = () => {
      if (disposed || !isAiStatsDocumentVisible() || timeoutId !== null) {
        return;
      }
      timeoutId = window.setTimeout(async () => {
        timeoutId = null;
        const current = selectedRequestRef.current;
        if (current && autoRefreshEnabledRef.current) {
          await loadRequestDetail(current, false);
        }
        schedule();
      }, AI_REQUEST_DETAIL_POLL_INTERVAL_MS);
    };
    const handleVisibilityChange = () => {
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
        timeoutId = null;
      }
      schedule();
    };
    document.addEventListener("visibilitychange", handleVisibilityChange);
    schedule();

    return () => {
      disposed = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [autoRefreshEnabled, detail?.request.finalState, loadRequestDetail]);

  useEffect(() => {
    return () => {
      if (copiedTimerRef.current !== null) {
        window.clearTimeout(copiedTimerRef.current);
      }
    };
  }, []);

  const updateAuditFilters = useCallback((update: Partial<AiStatsFilterState>) => {
    setFilters((current) => ({
      ...current,
      ...update,
      page: "1",
    }));
  }, []);

  const goToAuditPage = useCallback((page: number, totalPages: number) => {
    const maxPage = Math.max(1, totalPages);
    const nextPage = Math.min(maxPage, positivePage(page));
    setFilters((current) => ({
      ...current,
      page: String(nextPage),
    }));
    return nextPage;
  }, []);

  const setAuditPage = useCallback((page: number) => {
    const nextPageText = String(positivePage(page));
    setFilters((current) =>
      current.page === nextPageText ? current : { ...current, page: nextPageText },
    );
  }, []);

  const openRequestDetail = useCallback(
    async (request: AiRequestAuditSummary) => {
      selectedRequestRef.current = request;
      setSelectedRequestId(request.id);
      await loadRequestDetail(request, true);
    },
    [loadRequestDetail],
  );

  const copyAuditText = useCallback(async (key: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedKey(key);
      if (copiedTimerRef.current !== null) {
        window.clearTimeout(copiedTimerRef.current);
      }
      copiedTimerRef.current = window.setTimeout(() => {
        setCopiedKey((current) => (current === key ? null : current));
        copiedTimerRef.current = null;
      }, 1600);
    } catch (copyError) {
      setDetailError(errorMessage(copyError));
    }
  }, []);

  const closeRequestDetail = useCallback(() => {
    selectedRequestRef.current = null;
    setSelectedRequestId(null);
    setDetail(null);
    setDetailError(null);
    setCopiedKey(null);
  }, []);

  const pauseAutoRefresh = useCallback(() => {
    autoRefreshEnabledRef.current = false;
    // Drop auto-queued reloads so pause cannot be pierced by a pending auto follow-up.
    if (pendingStatsReloadSourceRef.current === "auto") {
      shouldReloadStatsAfterCurrentRequestRef.current = false;
    }
    setAutoRefreshEnabled(false);
  }, []);

  const resumeAutoRefresh = useCallback(() => {
    autoRefreshEnabledRef.current = true;
    setAutoRefreshEnabled(true);
    if (isAiStatsDocumentVisible()) {
      // Queue when a list request is already in flight so resume still syncs once.
      void loadStats(statsRef.current === null, true, "auto");
    }
  }, [loadStats]);

  return {
    autoRefreshEnabled,
    closeRequestDetail,
    copiedKey,
    copyAuditText,
    detail,
    detailError,
    error,
    filters,
    goToAuditPage,
    isLoading,
    isLoadingDetail,
    loadStats,
    openRequestDetail,
    pauseAutoRefresh,
    resumeAutoRefresh,
    selectedRequestId,
    setAuditPage,
    stats,
    updateAuditFilters,
  };
}

function positivePage(value: number) {
  return Number.isSafeInteger(value) && value > 0 ? value : 1;
}

function aiStatsFiltersFromInitialState(
  initialPage: number,
  initialFilters: Partial<AiStatsFilterState> | undefined,
) {
  return {
    ...emptyAiStatsFilters(initialPage),
    ...initialFilters,
    page: String(positivePage(Number(initialFilters?.page ?? initialPage))),
  };
}

function sameAiStatsFilters(left: AiStatsFilterState, right: AiStatsFilterState) {
  return (Object.keys(left) as (keyof AiStatsFilterState)[]).every(
    (key) => left[key] === right[key],
  );
}

function isAiStatsDocumentVisible() {
  return document.visibilityState !== "hidden";
}

function aiStatsQuery(filters: AiStatsFilterState) {
  const params = new URLSearchParams();
  const entries: [string, string][] = [
    ["workspaceId", filters.workspaceId],
    ["requestId", filters.requestIds],
    ["chatId", filters.chatId],
    ["providerId", filters.providerId],
    ["modelId", filters.modelId],
    ["requestKind", filters.requestKind],
    ["status", filters.status],
    ["startedAfter", datetimeLocalToRfc3339(filters.startedAfter)],
    ["startedBefore", datetimeLocalToRfc3339(filters.startedBefore)],
    ["page", filters.page.trim()],
    ["pageSize", filters.pageSize.trim()],
  ];

  for (const [key, value] of entries) {
    if (value) {
      params.set(key, value);
    }
  }

  return params.toString();
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

function defaultStartedAfterDatetimeLocal() {
  const date = new Date(Date.now() - 7 * 24 * 60 * 60 * 1000);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  return `${year}-${month}-${day}T${hours}:${minutes}`;
}
