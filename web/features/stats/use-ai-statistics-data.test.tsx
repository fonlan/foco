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
