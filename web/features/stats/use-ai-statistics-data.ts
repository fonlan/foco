import { useCallback, useEffect, useRef, useState } from "react";

import { errorMessage, requestJson } from "../../shared/api-client";
import type {
  AiRequestAuditSummary,
  AiRequestDetailResponse,
  AiStatisticsResponse,
  AiStatsFilterState,
} from "../../api/types";

const AI_STATS_POLL_INTERVAL_MS = 5000;

export function emptyAiStatsFilters(page = 1): AiStatsFilterState {
  return {
    chatId: "",
    modelId: "",
    page: String(positivePage(page)),
    pageSize: "20",
    providerId: "",
    startedAfter: "",
    startedBefore: "",
    status: "",
    workspaceId: "",
  };
}

export function useAiStatisticsData(initialPage = 1) {
  const [filters, setFilters] = useState<AiStatsFilterState>(
    () => emptyAiStatsFilters(initialPage),
  );
  const [stats, setStats] = useState<AiStatisticsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [selectedRequestId, setSelectedRequestId] = useState<string | null>(null);
  const [detail, setDetail] = useState<AiRequestDetailResponse | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const selectedRequestRef = useRef<AiRequestAuditSummary | null>(null);
  const filtersRef = useRef(filters);
  const isStatsRequestInFlightRef = useRef(false);
  const shouldReloadStatsAfterCurrentRequestRef = useRef(false);

  filtersRef.current = filters;

  const loadRequestDetail = useCallback(
    async (request: AiRequestAuditSummary, showLoading: boolean) => {
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
        setDetail(data);
      } catch (requestError) {
        setDetailError(errorMessage(requestError));
      } finally {
        if (showLoading) {
          setIsLoadingDetail(false);
        }
      }
    },
    [],
  );

  const loadStats = useCallback(
    async (showLoading = true, queueIfInFlight = false) => {
      if (isStatsRequestInFlightRef.current) {
        if (queueIfInFlight) {
          shouldReloadStatsAfterCurrentRequestRef.current = true;
        }
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
          if (
            selectedRequest.finalState === "running" ||
            refreshedRequest.finalState === "running" ||
            refreshedRequest.finalState !== selectedRequest.finalState
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
          void loadStats(false);
        }
      }
    },
    [loadRequestDetail],
  );

  useEffect(() => {
    void loadStats(true, true);
  }, [filters, loadStats]);

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      if (document.hidden) {
        return;
      }
      void loadStats(false);
    }, AI_STATS_POLL_INTERVAL_MS);
    const handleVisibilityChange = () => {
      if (!document.hidden) {
        void loadStats(false);
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      window.clearInterval(intervalId);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [loadStats]);

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
      window.setTimeout(() => {
        setCopiedKey((current) => (current === key ? null : current));
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

  return {
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
    selectedRequestId,
    setAuditPage,
    stats,
    updateAuditFilters,
  };
}

function positivePage(value: number) {
  return Number.isSafeInteger(value) && value > 0 ? value : 1;
}

function aiStatsQuery(filters: AiStatsFilterState) {
  const params = new URLSearchParams();
  const entries: [keyof AiStatsFilterState, string][] = [
    ["workspaceId", filters.workspaceId],
    ["chatId", filters.chatId],
    ["providerId", filters.providerId],
    ["modelId", filters.modelId],
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
