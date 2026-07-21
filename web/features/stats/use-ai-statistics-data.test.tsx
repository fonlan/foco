import { act, render } from "@testing-library/react";
import { useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAiStatisticsData } from "./use-ai-statistics-data";

function HookProbe({
  onHook,
}: {
  onHook: (hook: ReturnType<typeof useAiStatisticsData>) => void;
}) {
  const hook = useAiStatisticsData();

  useEffect(() => {
    onHook(hook);
  }, [hook, onHook]);

  return null;
}

function aiStatisticsUrls() {
  return vi
    .mocked(fetch)
    .mock.calls.map(([input]) => new URL(String(input), "http://localhost"))
    .filter((url) => url.pathname === "/api/ai-statistics");
}

function detailUrls() {
  return vi
    .mocked(fetch)
    .mock.calls.map(([input]) => new URL(String(input), "http://localhost"))
    .filter((url) => url.pathname.includes("/ai-statistics/"));
}

function setDocumentVisibility(visibilityState: DocumentVisibilityState) {
  Object.defineProperty(document, "visibilityState", {
    configurable: true,
    get: () => visibilityState,
  });
}

describe("useAiStatisticsData", () => {
  beforeEach(() => {
    setDocumentVisibility("visible");
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        new Response(JSON.stringify(emptyAiStatisticsResponse), {
          headers: { "content-type": "application/json" },
        }),
      ),
    );
    vi.mocked(navigator.clipboard.writeText).mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
    setDocumentVisibility("visible");
  });

  it("defaults startedAfter to the last 7 days", async () => {
    const before = Date.now() - 7 * 24 * 60 * 60 * 1000;
    let hook: ReturnType<typeof useAiStatisticsData> | null = null;
    render(<HookProbe onHook={(value) => (hook = value)} />);
    await act(async () => undefined);
    const after = Date.now() - 7 * 24 * 60 * 60 * 1000;

    const startedAfter = hook?.filters.startedAfter ?? "";
    expect(startedAfter).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/);
    expect(hook?.filters.startedBefore).toBe("");

    const startedAfterMs = new Date(startedAfter).getTime();
    expect(startedAfterMs).toBeGreaterThanOrEqual(before - 60_000);
    expect(startedAfterMs).toBeLessThanOrEqual(after + 60_000);

    const fetchMock = vi.mocked(fetch);
    expect(fetchMock).toHaveBeenCalled();
    const requestUrl = String(fetchMock.mock.calls[0]?.[0] ?? "");
    expect(requestUrl).toContain("startedAfter=");
  });

  it("includes requestKind in statistics queries", async () => {
    let hook: ReturnType<typeof useAiStatisticsData> | null = null;
    render(<HookProbe onHook={(value) => (hook = value)} />);
    await act(async () => undefined);

    await act(async () => {
      hook?.updateAuditFilters({ requestKind: "contextCompression" });
    });
    await act(async () => undefined);

    const requestUrls = vi
      .mocked(fetch)
      .mock.calls.map(([input]) => new URL(String(input), "http://localhost"));
    expect(
      requestUrls.some(
        (url) => url.searchParams.get("requestKind") === "contextCompression",
      ),
    ).toBe(true);
  });

  it("clears the previous copied timeout before scheduling another one", async () => {
    const clearTimeoutSpy = vi.spyOn(window, "clearTimeout");
    let hook: ReturnType<typeof useAiStatisticsData> | null = null;

    render(<HookProbe onHook={(value) => (hook = value)} />);
    await act(async () => undefined);

    await act(async () => {
      await hook?.copyAuditText("request", "first");
    });
    expect(clearTimeoutSpy).not.toHaveBeenCalled();

    await act(async () => {
      await hook?.copyAuditText("response", "second");
    });

    expect(clearTimeoutSpy).toHaveBeenCalledTimes(1);
  });

  it("clears the copied timeout on unmount", async () => {
    const clearTimeoutSpy = vi.spyOn(window, "clearTimeout");
    let hook: ReturnType<typeof useAiStatisticsData> | null = null;

    const { unmount } = render(<HookProbe onHook={(value) => (hook = value)} />);
    await act(async () => undefined);

    await act(async () => {
      await hook?.copyAuditText("request", "body");
    });

    unmount();

    expect(clearTimeoutSpy).toHaveBeenCalledTimes(1);
  });

  it("defaults to auto refresh and polls statistics every 5 seconds", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let hook: ReturnType<typeof useAiStatisticsData> | null = null;
    render(<HookProbe onHook={(value) => (hook = value)} />);
    await act(async () => undefined);

    expect(hook?.autoRefreshEnabled).toBe(true);
    const initialCount = aiStatisticsUrls().length;
    expect(initialCount).toBeGreaterThanOrEqual(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });

    expect(aiStatisticsUrls().length).toBeGreaterThan(initialCount);
  });

  it("stops list polling and visibility refresh while auto refresh is paused", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let hook: ReturnType<typeof useAiStatisticsData> | null = null;
    render(<HookProbe onHook={(value) => (hook = value)} />);
    await act(async () => undefined);

    await act(async () => {
      hook?.pauseAutoRefresh();
    });
    expect(hook?.autoRefreshEnabled).toBe(false);

    const pausedCount = aiStatisticsUrls().length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    expect(aiStatisticsUrls().length).toBe(pausedCount);

    setDocumentVisibility("hidden");
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    setDocumentVisibility("visible");
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(aiStatisticsUrls().length).toBe(pausedCount);
  });

  it("reloads immediately and resumes polling after auto refresh starts again", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let hook: ReturnType<typeof useAiStatisticsData> | null = null;
    render(<HookProbe onHook={(value) => (hook = value)} />);
    await act(async () => undefined);

    await act(async () => {
      hook?.pauseAutoRefresh();
    });
    const pausedCount = aiStatisticsUrls().length;

    await act(async () => {
      hook?.resumeAutoRefresh();
    });
    await act(async () => undefined);

    expect(hook?.autoRefreshEnabled).toBe(true);
    expect(aiStatisticsUrls().length).toBeGreaterThan(pausedCount);

    const afterResumeCount = aiStatisticsUrls().length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(aiStatisticsUrls().length).toBeGreaterThan(afterResumeCount);
  });

  it("still loads statistics for filter changes while auto refresh is paused", async () => {
    let hook: ReturnType<typeof useAiStatisticsData> | null = null;
    render(<HookProbe onHook={(value) => (hook = value)} />);
    await act(async () => undefined);

    await act(async () => {
      hook?.pauseAutoRefresh();
    });
    const pausedCount = aiStatisticsUrls().length;

    await act(async () => {
      hook?.updateAuditFilters({ status: "failed" });
    });
    await act(async () => undefined);

    expect(aiStatisticsUrls().length).toBeGreaterThan(pausedCount);
    expect(
      aiStatisticsUrls().some((url) => url.searchParams.get("status") === "failed"),
    ).toBe(true);
  });

  it("does not start a detail request when an in-flight auto list poll finishes after pause", async () => {
    let resolveStats: ((value: Response) => void) | null = null;
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = new URL(String(input), "http://localhost");
      if (url.pathname === "/api/ai-statistics") {
        if (fetchMock.mock.calls.length === 1) {
          return new Response(JSON.stringify(emptyAiStatisticsResponse), {
            headers: { "content-type": "application/json" },
          });
        }
        return await new Promise<Response>((resolve) => {
          resolveStats = resolve;
        });
      }
      if (url.pathname.startsWith("/api/workspaces/")) {
        return new Response(
          JSON.stringify({
            request: runningRequestSummary,
            events: [],
          }),
          { headers: { "content-type": "application/json" } },
        );
      }
      return new Response("{}", {
        headers: { "content-type": "application/json" },
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    let hook: ReturnType<typeof useAiStatisticsData> | null = null;
    render(<HookProbe onHook={(value) => (hook = value)} />);
    await act(async () => undefined);

    await act(async () => {
      await hook?.openRequestDetail(runningRequestSummary);
    });
    const detailCallsBeforePoll = detailUrls().length;
    expect(detailCallsBeforePoll).toBeGreaterThanOrEqual(1);

    // Start an auto list poll while a selected running request is open.
    await act(async () => {
      void hook?.loadStats(false, false, "auto");
    });
    expect(resolveStats).not.toBeNull();

    await act(async () => {
      hook?.pauseAutoRefresh();
    });

    await act(async () => {
      resolveStats?.(
        new Response(
          JSON.stringify({
            ...emptyAiStatisticsResponse,
            requests: [runningRequestSummary],
            totalCount: 1,
          }),
          { headers: { "content-type": "application/json" } },
        ),
      );
    });
    await act(async () => undefined);

    expect(detailUrls().length).toBe(detailCallsBeforePoll);
  });

  it("queues an immediate list reload when resume races an in-flight request", async () => {
    let resolveInFlight: ((value: Response) => void) | null = null;
    let statsCalls = 0;
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = new URL(String(input), "http://localhost");
      if (url.pathname !== "/api/ai-statistics") {
        return new Response("{}", {
          headers: { "content-type": "application/json" },
        });
      }
      statsCalls += 1;
      if (statsCalls === 1) {
        return new Response(JSON.stringify(emptyAiStatisticsResponse), {
          headers: { "content-type": "application/json" },
        });
      }
      if (statsCalls === 2) {
        return await new Promise<Response>((resolve) => {
          resolveInFlight = resolve;
        });
      }
      return new Response(JSON.stringify(emptyAiStatisticsResponse), {
        headers: { "content-type": "application/json" },
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    let hook: ReturnType<typeof useAiStatisticsData> | null = null;
    render(<HookProbe onHook={(value) => (hook = value)} />);
    await act(async () => undefined);
    expect(statsCalls).toBe(1);

    // Start a second list request that stays in flight.
    await act(async () => {
      void hook?.loadStats(false, false, "user");
    });
    expect(statsCalls).toBe(2);
    expect(resolveInFlight).not.toBeNull();

    await act(async () => {
      hook?.pauseAutoRefresh();
    });
    await act(async () => {
      hook?.resumeAutoRefresh();
    });
    // Resume must not drop the sync when a request is already in flight.
    expect(statsCalls).toBe(2);

    await act(async () => {
      resolveInFlight?.(
        new Response(JSON.stringify(emptyAiStatisticsResponse), {
          headers: { "content-type": "application/json" },
        }),
      );
    });
    await act(async () => undefined);

    expect(statsCalls).toBe(3);
  });
});

const runningRequestSummary = {
  id: "request-running",
  workspaceId: "workspace-1",
  workspaceName: "Workspace 1",
  chatId: "chat-1",
  chatTitle: "Chat 1",
  requestKind: "chat",
  providerId: "provider-1",
  modelId: "model-1",
  thinkingLevel: null,
  requestStartedAt: "2026-01-01T00:00:00.000Z",
  firstTokenAt: null,
  completedAt: null,
  inputTokens: 0,
  outputTokens: 0,
  cacheReadTokens: 0,
  cacheWriteTokens: 0,
  reasoningTokens: 0,
  cacheRatio: null,
  firstTokenLatencyMs: null,
  totalLatencyMs: null,
  statusCode: null,
  finalState: "running",
  invalidatedAt: null,
  invalidatedReason: null,
  transport: "http" as const,
};

const emptyAiStatisticsResponse = {
  page: 1,
  pageSize: 20,
  requests: [],
  summary: {
    averageLatencyMs: null,
    failedRequests: 0,
    modelBreakdown: [],
    providerBreakdown: [],
    requestKindBreakdown: [],
    totalCacheReadTokens: 0,
    totalCacheWriteTokens: 0,
    totalInputTokens: 0,
    totalOutputTokens: 0,
    totalRequests: 0,
    totalTokens: 0,
    trend: [],
  },
  totalCount: 0,
  totalPages: 1,
};
