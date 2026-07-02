import { describe, expect, it } from "vitest";

import { nextAutoRunnablePlan, trimInactiveChatMessageCaches } from "./App";
import type { Plan, ShellMessage } from "./api/types";

function plan(id: string, status: Plan["status"]): Plan {
  return {
    id,
    status,
    title: id,
    overview: "",
    sortOrder: 0,
    sourceChatId: null,
    activePhaseId: null,
    pauseRequestedAt: null,
    completedAt: null,
    completedByUserAt: null,
    errorMessage: null,
    sharedMergeCommitId: null,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    phases: [],
  };
}

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

describe("nextAutoRunnablePlan", () => {
  it("starts the first draft, ready, or failed plan in list order", () => {
    expect(
      nextAutoRunnablePlan([
        plan("implemented", "implemented"),
        plan("failed", "failed"),
        plan("ready", "ready"),
      ]),
    ).toEqual({ planId: "failed", action: "start" });
  });

  it("resumes paused plans and ignores terminal plans", () => {
    expect(
      nextAutoRunnablePlan([
        plan("completed", "completed"),
        plan("cancelled", "cancelled"),
        plan("paused", "paused"),
      ]),
    ).toEqual({ planId: "paused", action: "resume" });
    expect(
      nextAutoRunnablePlan([
        plan("implemented", "implemented"),
        plan("completed", "completed"),
        plan("cancelled", "cancelled"),
      ]),
    ).toBeNull();
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
