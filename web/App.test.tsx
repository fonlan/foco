import { describe, expect, it } from "vitest";

import { chatSessionStatusDotClass, deriveChatSessionStatus, expandMessagesWithUserInterruptions, mergeLoadedMessagesWithStreamingPlaceholders, normalizeChatMessageSummary, preserveCachedReasoningDurations, trimInactiveChatMessageCaches } from "./App";
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
});
