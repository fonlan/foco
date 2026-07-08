import { describe, expect, it } from "vitest";

import { chatSessionStatusDotClass, deriveChatSessionStatus, mergeLoadedMessagesWithStreamingPlaceholders, normalizeChatMessageSummary, preserveCachedReasoningDurations, trimInactiveChatMessageCaches } from "./App";
import type { ActiveChatRunSummary, ActiveRunInfo, ChatMessageSummary, ShellMessage } from "./api/types";

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
    expect(normalizeChatMessageSummary(summary("complete")).status).toBeUndefined();
    expect(normalizeChatMessageSummary(summary(null)).status).toBeUndefined();
  });
});

describe("mergeLoadedMessagesWithStreamingPlaceholders", () => {
  const activeRun: ActiveChatRunSummary = {
    acceptingGuidance: false,
    chatId: "chat-1",
    lastSequence: 0,
    runId: "run-1",
    workspaceId: "workspace-1",
  };

  it("keeps a cached streaming assistant placeholder after its loaded user message", () => {
    const loadedUser = { ...message("user-1"), role: "user" as const };
    const placeholder = { ...message("assistant-stream"), status: "streaming" as const };

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      [loadedUser],
      [loadedUser, placeholder],
      activeRun,
    );

    expect(result.map((item) => item.id)).toEqual(["user-1", "assistant-stream"]);
    expect(result[1]).toBe(placeholder);
  });

  it("lets a loaded assistant with the same id replace the cached placeholder", () => {
    const loadedUser = { ...message("user-1"), role: "user" as const };
    const loadedAssistant = { ...message("assistant-stream"), content: "Done" };
    const placeholder = { ...message("assistant-stream"), status: "streaming" as const };

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      [loadedUser, loadedAssistant],
      [loadedUser, placeholder],
      activeRun,
    );

    expect(result).toEqual([loadedUser, loadedAssistant]);
    expect(result[1].status).toBeUndefined();
  });

  it("drops cached streaming placeholders when there is no active run", () => {
    const loadedUser = { ...message("user-1"), role: "user" as const };
    const placeholder = { ...message("assistant-stream"), status: "streaming" as const };

    const result = mergeLoadedMessagesWithStreamingPlaceholders(
      [loadedUser],
      [loadedUser, placeholder],
      null,
    );

    expect(result).toEqual([loadedUser]);
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
