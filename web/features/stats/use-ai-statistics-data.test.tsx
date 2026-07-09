import { act, render } from "@testing-library/react";
import { useEffect } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

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

describe("useAiStatisticsData", () => {
  beforeEach(() => {
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
});

const emptyAiStatisticsResponse = {
  page: 1,
  pageSize: 20,
  requests: [],
  summary: {
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
  },
  totalCount: 0,
  totalPages: 1,
};
