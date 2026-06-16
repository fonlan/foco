import { useCallback, useEffect, useState } from "react";

import { errorMessage, requestJson } from "../../api/client";
import type {
  AiRequestAuditSummary,
  AiRequestDetailResponse,
  AiStatisticsResponse,
  AiStatsFilterState,
} from "../../api/types";

export function emptyAiStatsFilters(): AiStatsFilterState {
  return {
    chatId: "",
    modelId: "",
    page: "1",
    pageSize: "20",
    providerId: "",
    startedAfter: "",
    startedBefore: "",
    status: "",
    workspaceId: "",
  };
}

export function useAiStatisticsData() {
  const [filters, setFilters] = useState<AiStatsFilterState>(
    emptyAiStatsFilters,
  );
  const [stats, setStats] = useState<AiStatisticsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [selectedRequestId, setSelectedRequestId] = useState<string | null>(null);
  const [detail, setDetail] = useState<AiRequestDetailResponse | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  const loadStats = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const query = aiStatsQuery(filters);
      const data = await requestJson<AiStatisticsResponse>(
        `/api/ai-statistics${query ? `?${query}` : ""}`,
      );
      setStats(data);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }, [filters]);

  useEffect(() => {
    void loadStats();
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
    setFilters((current) => ({
      ...current,
      page: String(Math.min(maxPage, Math.max(1, page))),
    }));
  }, []);

  const openRequestDetail = useCallback(
    async (request: AiRequestAuditSummary) => {
      setSelectedRequestId(request.id);
      setDetail(null);
      setDetailError(null);
      setCopiedKey(null);
      setIsLoadingDetail(true);

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
        setIsLoadingDetail(false);
      }
    },
    [],
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
    stats,
    updateAuditFilters,
  };
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

