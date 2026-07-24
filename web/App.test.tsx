import { describe, expect, it } from "vitest";

import { activeRunIdFromStartEvent, chatSessionStatusDotClass, deriveChatSessionStatus, expandMessagesWithUserInterruptions, isAutomaticGuardSource, isGuidableActiveRun, isPersistedQueuedRunRunning, isSameContinuousLocalActiveRun, isTerminalActiveRun, mergeLoadedMessagesWithStreamingPlaceholders, normalizeChatMessageSummary, overlayStaleLoadedContextCompressionParts, parseChatStreamEvent, planModeEnabledFromMessages, preserveCachedReasoningDurations, trimInactiveChatMessageCaches } from "./App";
import type { ActiveRunInfo, ChatMessageSummary, ShellMessage } from "./api/types";

describe("remote start run identity", () => {
  it("prefers the stable remote runId over a provider request id", () => {
    const event = parseChatStreamEvent({
      assistantMessageId: "assistant-1",
      chatId: "chat-1",
      llmRequestId: "broker-request-2",
      memoriesUsed: [],
      runId: "remote-run-1",
      type: "start",
      userMessageId: "user-1",
    });

    expect(event).toMatchObject({ type: "start", runId: "remote-run-1" });
    expect(activeRunIdFromStartEvent(event ?? {})).toBe("remote-run-1");
  });
});

describe("remote run terminal identity", () => {
  const activeRun: ActiveRunInfo = {
    acceptingGuidance: true,
    chatId: "chat-1",
    chatKey: "workspace-1:chat-1",
    runId: "remote-run-1",
    workspaceId: "workspace-1",
  };

  it("keeps a delayed activeRun snapshot inert after the same run completed", () => {
    const delayedSnapshot = {
      acceptingGuidance: true,
      chatId: "chat-1",
      lastSequence: 8,
      runId: "remote-run-1",
      workspaceId: "workspace-1",
    };

    expect(isTerminalActiveRun(delayedSnapshot, "remote-run-1")).toBe(true);
    expect(
      deriveChatSessionStatus({
        activeChatKey: "workspace-1:chat-1",
        activeRunInfoByChatKey: {},
        chatKey: "workspace-1:chat-1",
        failedChatKeySet: new Set(),
        openChatKeySet: new Set(["workspace-1:chat-1"]),
        runningChatKeys: new Set(),
        terminalRunId: "remote-run-1",
        workspaceActiveRun: delayedSnapshot,
      }),
    ).toEqual({ activeRun: null, kind: "open" });
  });

  it("only permits guidance for a stable, current run identity", () => {
    expect(isGuidableActiveRun(activeRun, true)).toBe(true);
    expect(isGuidableActiveRun(activeRun, false)).toBe(false);
    expect(isGuidableActiveRun({ ...activeRun, acceptingGuidance: false }, true)).toBe(false);
    expect(isGuidableActiveRun({ ...activeRun, runId: null }, true)).toBe(false);
  });
});

function message(id: string): ShellMessage {
  return {
    content: id,
    createdAt: "2026-01-01T00:00:00Z",
    extractedMemories: [],
    id,
    memoriesUsed: [],
    metrics: null,
    parts: [{ text: id, type: "text" }],
    reasoning: null,
    role: "assistant",
    specUpdates: [],
    toolCalls: [],
  };
}

describe("deriveChatSessionStatus", () => {
  const activeRun: ActiveRunInfo = {
    acceptingGuidance: false,
    chatId: "chat-1",
    chatKey: "workspace-1:chat-1",
    runId: "run-1",
    workspaceId: "workspace-1",
  };

  function status(options: Partial<Parameters<typeof deriveChatSessionStatus>[0]> = {}) {
    return deriveChatSessionStatus({
      activeChatKey: null,
      activeRunInfoByChatKey: {},
      chatKey: "workspace-1:chat-1",
      failedChatKeySet: new Set(),
      openChatKeySet: new Set(),
      runningChatKeys: new Set(),
      ...options,
    });
  }

  it("uses one explicit priority order for chat session UI state", () => {
    expect(
      status({
        failedChatKeySet: new Set(["workspace-1:chat-1"]),
        openChatKeySet: new Set(["workspace-1:chat-1"]),
        runningChatKeys: new Set(["workspace-1:chat-1"]),
        scheduledStatus: "queued",
      }).kind,
    ).toBe("running");
    expect(
      status({
        failedChatKeySet: new Set(["workspace-1:chat-1"]),
        openChatKeySet: new Set(["workspace-1:chat-1"]),
        scheduledStatus: "queued",
      }).kind,
    ).toBe("scheduled");
    expect(
      status({
        failedChatKeySet: new Set(["workspace-1:chat-1"]),
        openChatKeySet: new Set(["workspace-1:chat-1"]),
      }).kind,
    ).toBe("failed");
    expect(status({ openChatKeySet: new Set(["workspace-1:chat-1"]) }).kind).toBe(
      "open",
    );
    expect(status().kind).toBe("idle");
  });

  it("keeps tab spinner and workspace dot classes on the same status kind", () => {
    expect(
      status({ activeRunInfoByChatKey: { "workspace-1:chat-1": activeRun } }).activeRun,
    ).toEqual(activeRun);
    expect(chatSessionStatusDotClass("running")).toBe("session-status-dot-running");
    expect(chatSessionStatusDotClass("failed")).toBe("session-status-dot-error");
  });

  it("treats persisted queuedRun running as visual running without inventing activeRun", () => {
    const result = status({
      openChatKeySet: new Set(["workspace-1:chat-1"]),
      persistedRunning: true,
    });
    expect(result.kind).toBe("running");
    expect(result.activeRun).toBeNull();
  });

  it("does not treat queued or missing persisted run as visual running", () => {
    expect(status({ persistedRunning: false }).kind).toBe("idle");
    expect(isPersistedQueuedRunRunning({ status: "queued" })).toBe(false);
    expect(isPersistedQueuedRunRunning({ status: "running" })).toBe(true);
    expect(isPersistedQueuedRunRunning(null)).toBe(false);
    expect(isPersistedQueuedRunRunning(undefined)).toBe(false);
    expect(isPersistedQueuedRunRunning({ status: "completed" })).toBe(false);
  });

  it("keeps local running, scheduled, failed, open, idle priority with persistedRunning", () => {
    expect(
      status({
        openChatKeySet: new Set(["workspace-1:chat-1"]),
        persistedRunning: true,
        runningChatKeys: new Set(["workspace-1:chat-1"]),
        scheduledStatus: "queued",
      }).kind,
    ).toBe("running");
    expect(
      status({
        openChatKeySet: new Set(["workspace-1:chat-1"]),
        persistedRunning: false,
        scheduledStatus: "queued",
      }).kind,
    ).toBe("scheduled");
    expect(
      status({
        failedChatKeySet: new Set(["workspace-1:chat-1"]),
        openChatKeySet: new Set(["workspace-1:chat-1"]),
        persistedRunning: false,
      }).kind,
    ).toBe("failed");
    expect(
      status({
        openChatKeySet: new Set(["workspace-1:chat-1"]),
        persistedRunning: false,
      }).kind,
    ).toBe("open");
    expect(status({ workspaceActiveRun: {
      acceptingGuidance: true,
      chatId: "chat-1",
      lastSequence: 1,
      runId: "run-1",
      workspaceId: "workspace-1",
    } }).kind).toBe("running");
  });
});

describe("trimInactiveChatMessageCaches", () => {
  it("keeps active and running chat caches intact while trimming old inactive caches", () => {
    const messagesByKey = Object.fromEntries(
      Array.from({ length: 6 }, (_, index) => [
        `chat-${index + 1}`,
        Array.from({ length: 4 }, (_unused, messageIndex) =>
          message(`chat-${index + 1}-message-${messageIndex + 1}`),
        ),
      ]),
    );

    const result = trimInactiveChatMessageCaches(messagesByKey, Object.keys(messagesByKey), {
      activeChatKey: "chat-2",
      fullCacheLimit: 2,
      openChatKeys: new Set(Object.keys(messagesByKey)),
      pageLimit: 2,
      runningChatKeys: new Set(["chat-3"]),
    });

    expect(result.trimmedChatKeys).toEqual(["chat-1", "chat-4"]);
    expect(result.messagesByKey["chat-1"].map((item) => item.id)).toEqual([
      "chat-1-message-3",
      "chat-1-message-4",
    ]);
    expect(result.messagesByKey["chat-2"]).toBe(messagesByKey["chat-2"]);
    expect(result.messagesByKey["chat-3"]).toBe(messagesByKey["chat-3"]);
    expect(result.messagesByKey["chat-5"]).toBe(messagesByKey["chat-5"]);
    expect(result.messagesByKey["chat-6"]).toBe(messagesByKey["chat-6"]);
  });
});

describe("normalizeChatMessageSummary", () => {
  function summary(status: unknown): ChatMessageSummary {
    return {
      ...message("assistant-1"),
      status: status as ChatMessageSummary["status"],
    };
  }

  it("keeps only UI-supported loaded message statuses", () => {
    expect(normalizeChatMessageSummary(summary("streaming")).status).toBe("streaming");
    expect(normalizeChatMessageSummary(summary("error")).status).toBe("error");
    expect(normalizeChatMessageSummary(summary("failed")).status).toBe("error");
    expect(normalizeChatMessageSummary(summary("complete")).status).toBeUndefined();
    expect(normalizeChatMessageSummary(summary(null)).status).toBeUndefined();
  });

  it("marks durable error parts as error status even without status field", () => {
    const result = normalizeChatMessageSummary({
      ...message("assistant-failed"),
      status: undefined,
      parts: [{ type: "error", text: "Reply has not started: workspace database is busy. Please retry." }],
      content: "Reply has not started: workspace database is busy. Please retry.",
    });
    expect(result.status).toBe("error");
    expect(result.parts).toEqual([
      {
        type: "error",
        text: "Reply has not started: workspace database is busy. Please retry.",
      },
    ]);
  });

  it("treats historically healed pre-stream failures as error bubbles", () => {
    const result = normalizeChatMessageSummary({
      ...message("assistant-historical-heal"),
      status: "error",
      parts: [
        {
          type: "error",
          text: "Reply has not started: workspace database is busy. Please retry.",
        },
      ],
      content: "Reply has not started: workspace database is busy. Please retry.",
    });
    expect(result.status).toBe("error");
    expect(result.parts.some((part) => part.type === "error")).toBe(true);
  });
});

describe("isSameContinuousLocalActiveRun", () => {
  it("requires a local active run with a recorded run id", () => {
    expect(
      isSameContinuousLocalActiveRun({
        hasLocalActiveRun: false,
        hasOpenLocalStream: true,
        localRunId: "run-a",
        serverActiveRunId: null,
      }),
    ).toBe(false);
    expect(
      isSameContinuousLocalActiveRun({
        hasLocalActiveRun: true,
        hasOpenLocalStream: true,
        localRunId: null,
        serverActiveRunId: null,
      }),
    ).toBe(false);
  });

  it("matches when the server reports the same run id", () => {
    expect(
      isSameContinuousLocalActiveRun({
        hasLocalActiveRun: true,
        hasOpenLocalStream: false,
        localRunId: "run-a",
        serverActiveRunId: "run-a",
      }),
    ).toBe(true);
  });

  it("rejects a different server run id even with an open local stream", () => {
    expect(
      isSameContinuousLocalActiveRun({
        hasLocalActiveRun: true,
        hasOpenLocalStream: true,
        localRunId: "run-a",
        serverActiveRunId: "run-b",
      }),
    ).toBe(false);
  });

  it("trusts temporary null server activeRun only while a local stream is open", () => {
    expect(
      isSameContinuousLocalActiveRun({
        hasLocalActiveRun: true,
        hasOpenLocalStream: true,
        localRunId: "run-a",
        serverActiveRunId: null,
      }),
    ).toBe(true);
    expect(
      isSameContinuousLocalActiveRun({
        hasLocalActiveRun: true,
        hasOpenLocalStream: false,
        localRunId: "run-a",
        serverActiveRunId: null,
      }),
    ).toBe(false);
  });
});

describe("mergeLoadedMessagesWithStreamingPlaceholders", () => {
  it("keeps a cached streaming assistant placeholder after its loaded user message", () => {
    const loadedUser = { ...message("user-1"), role: "user" as const };
    const placeholder = { ...message("assistant-stream"), status: "streaming" as const };

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      [loadedUser],
      [loadedUser, placeholder],
      true,
    );

    expect(result.messages.map((item) => item.id)).toEqual(["user-1", "assistant-stream"]);
    expect(result.messages[1]).toBe(placeholder);
    expect(result.preservedCachePrefix).toBe(false);
  });

  it("lets a loaded assistant with the same id replace the cached placeholder", () => {
    const loadedUser = { ...message("user-1"), role: "user" as const };
    const loadedAssistant = { ...message("assistant-stream"), content: "Done" };
    const placeholder = { ...message("assistant-stream"), status: "streaming" as const };

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      [loadedUser, loadedAssistant],
      [loadedUser, placeholder],
      true,
    );

    expect(result.messages).toEqual([loadedUser, loadedAssistant]);
    expect(result.messages[1].status).toBeUndefined();
    expect(result.preservedCachePrefix).toBe(false);
  });

  it("keeps a live completed compression lifecycle when a stale same-run assistant snapshot arrives", () => {
    const loadedUser = { ...message("user-1"), role: "user" as const };
    const staleAssistant = {
      ...message("assistant-live"),
      parts: [{ text: "Server text", type: "text" as const }],
      status: "streaming" as const,
    };
    const liveAssistant = {
      ...staleAssistant,
      parts: [
        ...staleAssistant.parts,
        {
          detail: {
            completedAt: "2026-07-23T10:00:02Z",
            compressionId: "compression-1",
            kind: "llm" as const,
            snapshotId: "snapshot-1",
            startedAt: "2026-07-23T10:00:00Z",
            status: "completed" as const,
          },
          id: "compression-1",
          kind: "llm" as const,
          status: "completed" as const,
          type: "contextCompression" as const,
        },
      ],
    };

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      [loadedUser, staleAssistant],
      [loadedUser, liveAssistant],
      { preserveLiveContextCompressionParts: true },
    );

    expect(result.messages[1]?.parts).toEqual(liveAssistant.parts);
  });

  it("does not overlay compression parts for an ordinary authoritative reload", () => {
    const loadedAssistant = {
      ...message("assistant-live"),
      parts: [{ text: "Server replacement", type: "text" as const }],
    };
    const cachedAssistant = {
      ...loadedAssistant,
      parts: [
        ...loadedAssistant.parts,
        {
          detail: {
            compressionId: "compression-1",
            kind: "llm" as const,
            startedAt: "2026-07-23T10:00:00Z",
            status: "start" as const,
          },
          id: "compression-1",
          kind: "llm" as const,
          status: "start" as const,
          type: "contextCompression" as const,
        },
      ],
    };

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      [loadedAssistant],
      [cachedAssistant],
    );

    expect(result.messages[0]?.parts).toEqual(loadedAssistant.parts);
  });

  it.each([
    ["start after an older response", "start", null, null],
    [
      "completed after an older response",
      "completed",
      "snapshot-live",
      900,
    ],
  ] as const)(
    "keeps exactly one live compression lifecycle for %s",
    (_label, status, snapshotId, summaryTokenCount) => {
      const loadedAssistant = {
        ...message("assistant-live"),
        parts: [{ text: "Older server text", type: "text" as const }],
      };
      const liveAssistant = {
        ...loadedAssistant,
        parts: [
          ...loadedAssistant.parts,
          {
            detail: {
              compressionId: "compression-live",
              kind: "llm" as const,
              modelId: "gpt-test",
              originalTokenCount: 5000,
              providerId: "openai",
              snapshotId,
              startedAt: "2026-07-23T10:00:00Z",
              status,
              summaryTokenCount,
            },
            id: "compression-live",
            kind: "llm" as const,
            status,
            type: "contextCompression" as const,
          },
        ],
      };

      const result = overlayStaleLoadedContextCompressionParts(
        [loadedAssistant],
        [liveAssistant],
      );
      const compressionParts = result[0]?.parts.filter(
        (part) => part.type === "contextCompression",
      );

      expect(compressionParts).toHaveLength(1);
      expect(compressionParts?.[0]).toMatchObject({
        detail: {
          compressionId: "compression-live",
          originalTokenCount: 5000,
          snapshotId,
          summaryTokenCount,
        },
        status,
      });
      expect(result[0]?.parts).toHaveLength(2);
    },
  );

  it("uses a persisted completed server part to upgrade a local start", () => {
    const loadedAssistant = {
      ...message("assistant-live"),
      parts: [
        {
          detail: {
            completedAt: "2026-07-23T10:00:02Z",
            compressionId: "compression-upgrade",
            kind: "llm" as const,
            originalTokenCount: 5000,
            snapshotId: "snapshot-server",
            startedAt: "2026-07-23T10:00:00Z",
            status: "completed" as const,
            summaryTokenCount: 900,
          },
          id: "compression-upgrade",
          kind: "llm" as const,
          status: "completed" as const,
          type: "contextCompression" as const,
        },
      ],
    };
    const localStart = {
      ...loadedAssistant,
      parts: [
        {
          detail: {
            compressionId: "compression-upgrade",
            kind: "llm" as const,
            originalTokenCount: 5000,
            startedAt: "2026-07-23T10:00:00Z",
            status: "start" as const,
          },
          id: "compression-upgrade",
          kind: "llm" as const,
          status: "start" as const,
          type: "contextCompression" as const,
        },
      ],
    };

    const result = overlayStaleLoadedContextCompressionParts(
      [loadedAssistant],
      [localStart],
    );
    const compressionParts = result[0]?.parts.filter(
      (part) => part.type === "contextCompression",
    );

    expect(compressionParts).toHaveLength(1);
    expect(compressionParts?.[0]).toMatchObject({
      detail: { snapshotId: "snapshot-server", summaryTokenCount: 900 },
      status: "completed",
    });
  });

  it("does not let an older server start downgrade a local completed part", () => {
    const serverStart = {
      ...message("assistant-live"),
      parts: [
        {
          detail: {
            compressionId: "compression-no-downgrade",
            kind: "llm" as const,
            originalTokenCount: 5000,
            startedAt: "2026-07-23T10:00:00Z",
            status: "start" as const,
          },
          id: "compression-no-downgrade",
          kind: "llm" as const,
          status: "start" as const,
          type: "contextCompression" as const,
        },
      ],
    };
    const localCompleted = {
      ...serverStart,
      parts: [
        {
          detail: {
            completedAt: "2026-07-23T10:00:02Z",
            compressionId: "compression-no-downgrade",
            kind: "llm" as const,
            originalTokenCount: 5000,
            snapshotId: "snapshot-local",
            startedAt: "2026-07-23T10:00:00Z",
            status: "completed" as const,
            summaryTokenCount: 900,
          },
          id: "compression-no-downgrade",
          kind: "llm" as const,
          status: "completed" as const,
          type: "contextCompression" as const,
        },
      ],
    };

    const result = overlayStaleLoadedContextCompressionParts(
      [serverStart],
      [localCompleted],
    );
    const compressionParts = result[0]?.parts.filter(
      (part) => part.type === "contextCompression",
    );

    expect(compressionParts).toHaveLength(1);
    expect(compressionParts?.[0]).toMatchObject({
      detail: { snapshotId: "snapshot-local", summaryTokenCount: 900 },
      status: "completed",
    });
  });

  it("keeps same-second compression lifecycles distinct by compression ID", () => {
    const liveAssistant = {
      ...message("assistant-live"),
      parts: ["compression-a", "compression-b"].map((compressionId) => ({
        detail: {
          completedAt: "2026-07-23T10:00:02Z",
          compressionId,
          kind: "llm" as const,
          originalTokenCount: 5000,
          snapshotId: `snapshot-${compressionId}`,
          startedAt: "2026-07-23T10:00:00Z",
          status: "completed" as const,
          summaryTokenCount: 900,
        },
        id: compressionId,
        kind: "llm" as const,
        status: "completed" as const,
        type: "contextCompression" as const,
      })),
    };

    const result = overlayStaleLoadedContextCompressionParts(
      [{ ...message("assistant-live"), parts: [] }],
      [liveAssistant],
    );
    const compressionParts = result[0]?.parts.filter(
      (part) => part.type === "contextCompression",
    );

    expect(compressionParts).toHaveLength(2);
    expect(compressionParts?.map((part) => part.detail.compressionId)).toEqual([
      "compression-a",
      "compression-b",
    ]);
  });

  it("does not resurrect a deleted compression part from a zero-overlap cache", () => {
    const staleCache = [
      {
        ...message("assistant-deleted"),
        parts: [
          {
            detail: {
              compressionId: "compression-deleted",
              kind: "llm" as const,
              startedAt: "2026-07-23T10:00:00Z",
              status: "completed" as const,
            },
            id: "compression-deleted",
            kind: "llm" as const,
            status: "completed" as const,
            type: "contextCompression" as const,
          },
        ],
      },
    ];
    const serverRewrite = [
      {
        ...message("assistant-rewritten"),
        content: "Authoritative rewrite.",
        parts: [{ text: "Authoritative rewrite.", type: "text" as const }],
      },
    ];

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      serverRewrite,
      staleCache,
      { preserveLiveContextCompressionParts: true },
    );

    expect(result.preservedCachePrefix).toBe(false);
    expect(result.messages).toEqual(serverRewrite);
    expect(result.messages[0]?.parts).not.toContainEqual(
      expect.objectContaining({ type: "contextCompression" }),
    );
  });

  it("drops cached streaming placeholders when preserveStreaming is false", () => {
    const loadedUser = { ...message("user-1"), role: "user" as const };
    const placeholder = { ...message("assistant-stream"), status: "streaming" as const };

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      [loadedUser],
      [loadedUser, placeholder],
      false,
    );

    expect(result.messages).toEqual([loadedUser]);
    expect(result.preservedCachePrefix).toBe(false);
  });

  it("preserves older cached pages when the latest page overlaps stable message ids", () => {
    const older = [
      { ...message("old-1"), role: "user" as const, content: "Earlier note." },
      { ...message("old-2"), content: "Earlier answer." },
    ];
    const recent = [
      { ...message("user-1"), role: "user" as const, content: "Please inspect README." },
      { ...message("assistant-1"), content: "Done." },
    ];
    const loaded = recent.map((item) => ({ ...item, content: `${item.content} (server)` }));

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      loaded,
      [...older, ...recent],
      false,
    );

    expect(result.preservedCachePrefix).toBe(true);
    expect(result.messages.map((item) => item.id)).toEqual([
      "old-1",
      "old-2",
      "user-1",
      "assistant-1",
    ]);
    expect(result.messages[0]?.content).toBe("Earlier note.");
    expect(result.messages[2]?.content).toBe("Please inspect README. (server)");
    expect(result.messages[3]?.content).toBe("Done. (server)");
  });

  it("does not resurrect cache history when the latest page has no id overlap", () => {
    const staleCache = [
      { ...message("old-1"), role: "user" as const, content: "Deleted history." },
      { ...message("old-2"), content: "Also deleted." },
    ];
    const rewritten = [
      { ...message("new-user"), role: "user" as const, content: "Rewritten prompt." },
      { ...message("new-assistant"), content: "Rewritten answer." },
    ];

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      rewritten,
      staleCache,
      false,
    );

    expect(result.preservedCachePrefix).toBe(false);
    expect(result.messages).toEqual(rewritten);
    expect(result.messages.map((item) => item.id)).not.toContain("old-1");
  });

  it("does not resurrect a streaming placeholder when there is no id overlap", () => {
    const staleCache = [
      { ...message("old-1"), role: "user" as const, content: "Deleted history." },
      {
        ...message("assistant-stream"),
        status: "streaming" as const,
        content: "Orphan thinking…",
      },
    ];
    const rewritten = [
      { ...message("new-user"), role: "user" as const, content: "Rewritten prompt." },
      { ...message("new-assistant"), content: "Rewritten answer." },
    ];

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      rewritten,
      staleCache,
      true,
    );

    expect(result.preservedCachePrefix).toBe(false);
    expect(result.messages).toEqual(rewritten);
    expect(result.messages.map((item) => item.id)).not.toContain("assistant-stream");
    expect(result.messages.map((item) => item.id)).not.toContain("old-1");
  });

  it("keeps cache history and appends server tail on active-run disjoint refresh", () => {
    const older = [
      { ...message("old-1"), role: "user" as const, content: "Earlier note." },
      { ...message("old-2"), content: "Earlier answer." },
    ];
    const previousAttempt = [
      { ...message("user-1"), role: "user" as const, content: "Please inspect README." },
      { ...message("assistant-1"), content: "Done." },
    ];
    const newAttemptTail = [
      { ...message("user-2"), role: "user" as const, content: "Continue after worker." },
      { ...message("assistant-2"), content: "Recovered answer." },
    ];

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      newAttemptTail,
      [...older, ...previousAttempt],
      {
        preserveDisjointActiveRunCache: true,
        preserveStreamingPlaceholders: true,
      },
    );

    expect(result.preservedCachePrefix).toBe(true);
    expect(result.messages.map((item) => item.id)).toEqual([
      "old-1",
      "old-2",
      "user-1",
      "assistant-1",
      "user-2",
      "assistant-2",
    ]);
    expect(result.messages[0]?.content).toBe("Earlier note.");
    expect(result.messages[4]?.content).toBe("Continue after worker.");
    expect(result.messages[5]?.content).toBe("Recovered answer.");
  });

  it("lets server versions override same ids when preserving cache (overlap prefix path)", () => {
    // Overlap on user-2 uses the prefix-preserve branch; server page replaces
    // the overlapping suffix while older cache prefix is kept.
    const cache = [
      { ...message("old-1"), role: "user" as const, content: "Earlier note." },
      { ...message("user-2"), role: "user" as const, content: "Continue (local)." },
    ];
    const loaded = [
      { ...message("user-2"), role: "user" as const, content: "Continue (server)." },
      { ...message("assistant-2"), content: "Recovered answer." },
    ];

    const result = mergeLoadedMessagesWithStreamingPlaceholders(loaded, cache, {
      preserveDisjointActiveRunCache: true,
      preserveStreamingPlaceholders: true,
    });

    expect(result.preservedCachePrefix).toBe(true);
    expect(result.messages.map((item) => item.id)).toEqual([
      "old-1",
      "user-2",
      "assistant-2",
    ]);
    expect(result.messages[1]?.content).toBe("Continue (server).");
    expect(result.messages.filter((item) => item.id === "user-2")).toHaveLength(1);
  });

  it("appends only server-only messages without duplicates on pure zero-overlap preserve", () => {
    const cache = [
      { ...message("old-1"), role: "user" as const, content: "Earlier note." },
      { ...message("assistant-old"), content: "Earlier answer (local)." },
    ];
    const loaded = [
      { ...message("user-new"), role: "user" as const, content: "New attempt user." },
      { ...message("assistant-new"), content: "New attempt answer." },
    ];

    const result = mergeLoadedMessagesWithStreamingPlaceholders(loaded, cache, {
      preserveDisjointActiveRunCache: true,
      preserveStreamingPlaceholders: true,
    });

    expect(result.preservedCachePrefix).toBe(true);
    expect(result.messages.map((item) => item.id)).toEqual([
      "old-1",
      "assistant-old",
      "user-new",
      "assistant-new",
    ]);
    expect(result.messages[0]?.content).toBe("Earlier note.");
    expect(result.messages[1]?.content).toBe("Earlier answer (local).");
    expect(result.messages[2]?.content).toBe("New attempt user.");
  });

  it("keeps streaming placeholder without duplicates on active-run disjoint refresh", () => {
    const older = { ...message("old-1"), role: "user" as const, content: "Earlier note." };
    const previousUser = {
      ...message("user-1"),
      role: "user" as const,
      content: "Please inspect README.",
    };
    const placeholder = {
      ...message("assistant-stream"),
      status: "streaming" as const,
      content: "Thinking…",
      parts: [
        { text: "Reasoning live", type: "reasoning" as const, liveDurationMs: 1500 },
        { text: "Partial answer", type: "text" as const },
      ],
    };
    const newAttemptUser = {
      ...message("user-2"),
      role: "user" as const,
      content: "Continue after worker.",
    };

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      [newAttemptUser],
      [older, previousUser, placeholder],
      {
        preserveDisjointActiveRunCache: true,
        preserveStreamingPlaceholders: true,
      },
    );

    expect(result.preservedCachePrefix).toBe(true);
    expect(result.messages.map((item) => item.id)).toEqual([
      "old-1",
      "user-1",
      "assistant-stream",
      "user-2",
    ]);
    expect(result.messages[2]).toBe(placeholder);
    expect(result.messages.filter((item) => item.id === "assistant-stream")).toHaveLength(1);
  });

  it("drops cache history on ordinary zero-overlap even when only streaming preserve is true", () => {
    const staleCache = [
      { ...message("old-1"), role: "user" as const, content: "Deleted history." },
      { ...message("old-2"), content: "Also deleted." },
    ];
    const rewritten = [
      { ...message("new-user"), role: "user" as const, content: "Rewritten prompt." },
      { ...message("new-assistant"), content: "Rewritten answer." },
    ];

    const result = mergeLoadedMessagesWithStreamingPlaceholders(rewritten, staleCache, {
      preserveDisjointActiveRunCache: false,
      preserveStreamingPlaceholders: true,
    });

    expect(result.preservedCachePrefix).toBe(false);
    expect(result.messages).toEqual(rewritten);
  });

  it("keeps a local streaming assistant when preserveStreaming is true and ids overlap", () => {
    const loadedUser = { ...message("user-1"), role: "user" as const };
    const older = { ...message("old-1"), role: "user" as const, content: "Earlier note." };
    const placeholder = {
      ...message("assistant-stream"),
      status: "streaming" as const,
      content: "Thinking…",
      parts: [
        { text: "Reasoning live", type: "reasoning" as const, liveDurationMs: 1500 },
        { text: "Partial answer", type: "text" as const },
      ],
    };

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      [loadedUser],
      [older, loadedUser, placeholder],
      true,
    );

    expect(result.preservedCachePrefix).toBe(true);
    expect(result.messages.map((item) => item.id)).toEqual([
      "old-1",
      "user-1",
      "assistant-stream",
    ]);
    expect(result.messages[2]).toBe(placeholder);
    expect(result.messages[2]?.parts).toEqual(placeholder.parts);
  });
});

describe("preserveCachedReasoningDurations", () => {
  it("preserves cached reasoning durations without mutating refreshed messages", () => {
    const refreshedMessages: ShellMessage[] = [
      {
        ...message("assistant-1"),
        parts: [
          { text: "First thought", type: "reasoning" },
          { text: "Answer", type: "text" },
          { text: "Second thought", type: "reasoning" },
        ],
      },
      {
        ...message("assistant-2"),
        parts: [{ durationMs: 3000, text: "Server thought", type: "reasoning" }],
      },
    ];
    const cachedMessages: ShellMessage[] = [
      {
        ...message("assistant-1"),
        parts: [
          { durationMs: 1000, text: "First thought", type: "reasoning" },
          { text: "Answer", type: "text" },
          { liveDurationMs: 2000, text: "Second thought", type: "reasoning" },
        ],
      },
      {
        ...message("assistant-2"),
        parts: [{ durationMs: 1000, text: "Server thought", type: "reasoning" }],
      },
    ];
    const originalRefreshedParts = refreshedMessages[0].parts;

    const result = preserveCachedReasoningDurations(refreshedMessages, cachedMessages);

    expect(result[0].parts).toEqual([
      { durationMs: 1000, text: "First thought", type: "reasoning" },
      { text: "Answer", type: "text" },
      { liveDurationMs: 2000, text: "Second thought", type: "reasoning" },
    ]);
    expect(result[1].parts).toEqual([
      { durationMs: 3000, text: "Server thought", type: "reasoning" },
    ]);
    expect(refreshedMessages[0].parts).toEqual([
      { text: "First thought", type: "reasoning" },
      { text: "Answer", type: "text" },
      { text: "Second thought", type: "reasoning" },
    ]);
    expect(result[0].parts).not.toBe(originalRefreshedParts);
  });
});

describe("expandMessagesWithUserInterruptions", () => {
  const interruptedMetrics = {
    firstTokenLatencyMs: 100,
    llmRequestIds: [] as string[],
    modelId: "gpt-test",
    outputTokens: 5,
    providerId: "openai",
    totalLatencyMs: 1000,
  };
  const finalMetrics = {
    firstTokenLatencyMs: 200,
    llmRequestIds: ["req-final"] as string[],
    modelId: "gpt-test",
    outputTokens: 20,
    providerId: "openai",
    totalLatencyMs: 3000,
  };

  it("expands a single userInterruption into stable virtual bubbles", () => {
    const toolCall = {
      id: "call-1",
      input: {},
      isError: false,
      name: "read_file",
      output: null,
      status: "completed" as const,
    };
    const assistant: ShellMessage = {
      ...message("msg-assistant-1"),
      content: "before after",
      metrics: finalMetrics,
      parts: [
        { text: "looping…", type: "reasoning" },
        { text: "before", type: "text" },
        {
          content: "repeated reasoning loop, check and continue",
          id: "interrupt-1",
          interruptedAssistantMetrics: interruptedMetrics,
          source: "reasoningLoopGuard",
          type: "userInterruption",
        },
        { text: "after", type: "text" },
        { toolCall, type: "toolCall" },
      ],
      toolCalls: [toolCall],
    };

    const expanded = expandMessagesWithUserInterruptions([
      {
        ...message("user-1"),
        content: "hello",
        parts: [{ text: "hello", type: "text" }],
        role: "user",
      },
      assistant,
    ]);

    expect(expanded.map((item) => item.id)).toEqual([
      "user-1",
      "msg-assistant-1",
      "interrupt-1",
      "interrupt-1-assistant",
    ]);
    expect(expanded[1]).toMatchObject({
      content: "before",
      id: "msg-assistant-1",
      metrics: interruptedMetrics,
      reasoning: "looping…",
      role: "assistant",
    });
    expect(expanded[1].parts.map((part) => part.type)).toEqual(["reasoning", "text"]);
    expect(expanded[2]).toMatchObject({
      content: "repeated reasoning loop, check and continue",
      id: "interrupt-1",
      role: "user",
      syntheticSource: "reasoningLoopGuard",
    });
    expect(expanded[2].pendingMode).toBeUndefined();
    expect(expanded[3]).toMatchObject({
      content: "after",
      id: "interrupt-1-assistant",
      metrics: finalMetrics,
      role: "assistant",
    });
    expect(expanded[3].parts.map((part) => part.type)).toEqual(["text", "toolCall"]);
    expect(expanded[3].toolCalls).toEqual([toolCall]);
  });

  it("keeps stable ids across multiple interruptions", () => {
    const assistant: ShellMessage = {
      ...message("msg-assistant-root"),
      metrics: finalMetrics,
      parts: [
        { text: "r1", type: "reasoning" },
        {
          content: "repeated reasoning loop, check and continue",
          id: "interrupt-a",
          interruptedAssistantMetrics: interruptedMetrics,
          source: "reasoningLoopGuard",
          type: "userInterruption",
        },
        { text: "mid", type: "text" },
        {
          content: "repeated reasoning loop, check and continue",
          id: "interrupt-b",
          interruptedAssistantMetrics: {
            ...interruptedMetrics,
            totalLatencyMs: 1500,
          },
          source: "reasoningLoopGuard",
          type: "userInterruption",
        },
        { text: "final", type: "text" },
      ],
    };

    const expanded = expandMessagesWithUserInterruptions([assistant]);
    expect(expanded.map((item) => item.id)).toEqual([
      "msg-assistant-root",
      "interrupt-a",
      "interrupt-a-assistant",
      "interrupt-b",
      "interrupt-b-assistant",
    ]);
    expect(expanded[0].metrics).toEqual(interruptedMetrics);
    expect(expanded[2].metrics).toEqual({
      ...interruptedMetrics,
      totalLatencyMs: 1500,
    });
    expect(expanded[4].metrics).toEqual(finalMetrics);
    expect(expanded[1].syntheticSource).toBe("reasoningLoopGuard");
    expect(expanded[3].syntheticSource).toBe("reasoningLoopGuard");
  });

  it("leaves messages without interruptions unchanged", () => {
    const messages = [message("a"), message("b")];
    expect(expandMessagesWithUserInterruptions(messages)).toBe(messages);
  });

  it("normalizes and expands userInterruption from API-shaped parts", () => {
    const summary: ChatMessageSummary = {
      content: "partial final",
      createdAt: "2026-01-01T00:00:00Z",
      extractedMemories: [],
      id: "assistant-hist",
      memoriesUsed: [],
      metrics: finalMetrics,
      parts: [
        { text: "partial", type: "text" },
        {
          content: "repeated reasoning loop, check and continue",
          id: "hist-interrupt",
          interruptedAssistantMetrics: interruptedMetrics,
          source: "reasoningLoopGuard",
          type: "userInterruption",
        },
        { text: "final", type: "text" },
      ],
      reasoning: null,
      role: "assistant",
      specUpdates: [],
      toolCalls: [],
    };
    const expanded = expandMessagesWithUserInterruptions([
      normalizeChatMessageSummary(summary),
    ]);
    expect(expanded.map((item) => item.id)).toEqual([
      "assistant-hist",
      "hist-interrupt",
      "hist-interrupt-assistant",
    ]);
    expect(expanded[1].syntheticSource).toBe("reasoningLoopGuard");
  });

  it("expands toolCallLoopGuard interruptions as synthetic user bubbles", () => {
    const loopError =
      "Runtime progress guard stopped the provider stream after detecting a repeated tool-call batch (read_file). The repeated batch was not executed.";
    const assistant: ShellMessage = {
      ...message("msg-assistant-tool-loop"),
      content: "before after",
      metrics: finalMetrics,
      parts: [
        { text: "before", type: "text" },
        {
          content: loopError,
          id: "tool-loop-interrupt-1",
          interruptedAssistantMetrics: interruptedMetrics,
          source: "toolCallLoopGuard",
          type: "userInterruption",
        },
        { text: "after", type: "text" },
      ],
    };

    const expanded = expandMessagesWithUserInterruptions([assistant]);
    expect(expanded.map((item) => item.id)).toEqual([
      "msg-assistant-tool-loop",
      "tool-loop-interrupt-1",
      "tool-loop-interrupt-1-assistant",
    ]);
    expect(expanded[1]).toMatchObject({
      content: loopError,
      id: "tool-loop-interrupt-1",
      role: "user",
      syntheticSource: "toolCallLoopGuard",
    });
    expect(expanded[1].pendingMode).toBeUndefined();
    expect(expanded[2]).toMatchObject({
      content: "after",
      id: "tool-loop-interrupt-1-assistant",
      metrics: finalMetrics,
      role: "assistant",
    });
  });

  it("keeps stable ids across mixed automatic guard interruptions", () => {
    const assistant: ShellMessage = {
      ...message("msg-assistant-mixed"),
      metrics: finalMetrics,
      parts: [
        { text: "r1", type: "reasoning" },
        {
          content: "repeated reasoning loop, check and continue",
          id: "interrupt-reasoning",
          interruptedAssistantMetrics: interruptedMetrics,
          source: "reasoningLoopGuard",
          type: "userInterruption",
        },
        { text: "mid", type: "text" },
        {
          content: "repeated tool-call batch blocked",
          id: "interrupt-tool-loop",
          interruptedAssistantMetrics: {
            ...interruptedMetrics,
            totalLatencyMs: 1500,
          },
          source: "toolCallLoopGuard",
          type: "userInterruption",
        },
        { text: "final", type: "text" },
      ],
    };

    const expanded = expandMessagesWithUserInterruptions([assistant]);
    expect(expanded.map((item) => item.id)).toEqual([
      "msg-assistant-mixed",
      "interrupt-reasoning",
      "interrupt-reasoning-assistant",
      "interrupt-tool-loop",
      "interrupt-tool-loop-assistant",
    ]);
    expect(expanded[1].syntheticSource).toBe("reasoningLoopGuard");
    expect(expanded[3].syntheticSource).toBe("toolCallLoopGuard");
  });
});

describe("isAutomaticGuardSource", () => {
  it("recognizes reasoning and tool-call loop guards", () => {
    expect(isAutomaticGuardSource("reasoningLoopGuard")).toBe(true);
    expect(isAutomaticGuardSource("toolCallLoopGuard")).toBe(true);
    expect(isAutomaticGuardSource("manualGuidance")).toBe(false);
    expect(isAutomaticGuardSource("agentMessage")).toBe(false);
    expect(isAutomaticGuardSource("userInterruption")).toBe(false);
    expect(isAutomaticGuardSource(undefined)).toBe(false);
  });
});

describe("planModeEnabledFromMessages", () => {
  function userMessage(
    id: string,
    options: {
      sessionMode?: "plan" | null;
      syntheticSource?: string;
    } = {},
  ): ShellMessage {
    return {
      ...message(id),
      role: "user",
      sessionMode: options.sessionMode,
      syntheticSource: options.syntheticSource,
    };
  }

  it("uses the last real user message sessionMode", () => {
    expect(
      planModeEnabledFromMessages([
        userMessage("u1"),
        message("a1"),
        userMessage("u2", { sessionMode: "plan" }),
        message("a2"),
      ]),
    ).toBe(true);
    expect(
      planModeEnabledFromMessages([
        userMessage("u1", { sessionMode: "plan" }),
        message("a1"),
        userMessage("u2"),
      ]),
    ).toBe(false);
  });

  it("ignores synthetic user interruptions", () => {
    expect(
      planModeEnabledFromMessages([
        userMessage("u1"),
        userMessage("interrupt", {
          sessionMode: "plan",
          syntheticSource: "reasoningLoopGuard",
        }),
        message("a1"),
      ]),
    ).toBe(false);
    expect(
      planModeEnabledFromMessages([
        userMessage("u1", { sessionMode: "plan" }),
        userMessage("interrupt", {
          syntheticSource: "reasoningLoopGuard",
        }),
      ]),
    ).toBe(true);
    expect(
      planModeEnabledFromMessages([
        userMessage("u1"),
        userMessage("tool-loop-interrupt", {
          sessionMode: "plan",
          syntheticSource: "toolCallLoopGuard",
        }),
        message("a1"),
      ]),
    ).toBe(false);
  });

  it("returns false when there is no real user message", () => {
    expect(planModeEnabledFromMessages([])).toBe(false);
    expect(planModeEnabledFromMessages([message("a1")])).toBe(false);
    expect(
      planModeEnabledFromMessages([
        userMessage("interrupt", { syntheticSource: "userInterruption" }),
      ]),
    ).toBe(false);
  });
});
