import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  activeMemory,
  appTestState,
  changeInput,
  chatStreamResponse,
  chatSummary,
  defaultComposerPlaceholder,
  chatMemory,
  chatMessages,
  deferred,
  enqueueChatStreamEvent,
  enqueueChatStreamEventForRun,
  jsonResponse,
  memoryExtractionJob,
  memorySource,
  mermaidMock,
  mockFetch,
  pendingMemory,
  renderApp,
  resetAppTestEnvironment,
  secondaryWorkspace,
  settings,
  todoGraph,
  workspace,
  workspaceMemory,
} from "./test-utils/app-test-harness";

describe("app-chat-stream verification surfaces", () => {
  beforeEach(resetAppTestEnvironment);

  it("updates context usage from latest response usage during a stream", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(
      await screen.findByRole("status", { name: "Context usage 0%" }),
    ).toHaveTextContent("0%");
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    expect(
      screen.getByRole("status", { name: "Context usage 0%" }),
    ).toHaveTextContent("0%");
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Partial answer.",
        type: "textDelta",
      });
    });

    expect(
      await screen.findByRole("status", { name: "Context usage 0%" }),
    ).toHaveTextContent("0%");
    const usageCalls = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCalls).toHaveLength(0);

    await act(async () => {
      enqueueChatStreamEvent({
        type: "usage",
        usage: {
          cacheReadTokens: 0,
          cacheWriteTokens: 0,
          inputTokens: 70000,
          outputTokens: 1000,
        },
      });
    });

    expect(
      await screen.findByRole("status", { name: "Context usage 64%" }),
    ).toHaveTextContent("64%");
    const usageCallsAfterUsage = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    const [, usageInit] = usageCallsAfterUsage.at(-1)!;
    expect(typeof usageInit?.body).toBe("string");
    expect(JSON.parse(usageInit?.body as string)).toMatchObject({
      chatId: "chat-1",
      latestResponseUsage: {
        cacheReadTokens: 0,
        cacheWriteTokens: 0,
        inputTokens: 70000,
        outputTokens: 1000,
      },
    });

    const usageCallCountBeforeComplete = usageCallsAfterUsage.length;
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: 10,
          modelId: "model-1",
          outputTokens: 9000,
          providerId: "provider-1",
          totalLatencyMs: 1000,
        },
        reasoning: null,
        stopReason: "completed",
        text: "Final answer.",
        type: "complete",
        usage: {
          cacheReadTokens: 0,
          cacheWriteTokens: 0,
          inputTokens: 999999,
          outputTokens: 9000,
        },
      });
    });

    const usageCallsAfterComplete = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCallsAfterComplete).toHaveLength(usageCallCountBeforeComplete);
    expect(
      screen.getByRole("status", { name: "Context usage 64%" }),
    ).toHaveTextContent("64%");

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("does not estimate context usage from streaming deltas", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(
      await screen.findByRole("status", { name: "Context usage 0%" }),
    ).toHaveTextContent("0%");
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    const usageCallCountBeforeDeltas = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    ).length;

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Part one. ",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Part two.",
        type: "textDelta",
      });
    });

    expect(
      fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/context-usage",
      ),
    ).toHaveLength(usageCallCountBeforeDeltas);
    expect(
      screen.getByRole("status", { name: "Context usage 0%" }),
    ).toHaveTextContent("0%");

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("keeps context usage isolated between open chats", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    expect(
      await screen.findByRole("status", { name: "Context usage 0%" }),
    ).toHaveTextContent("0%");
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(true),
    );

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        type: "usage",
        usage: {
          cacheReadTokens: 0,
          cacheWriteTokens: 0,
          inputTokens: 70000,
          outputTokens: 1000,
        },
      });
    });
    expect(
      await screen.findByRole("status", { name: "Context usage 64%" }),
    ).toHaveTextContent("64%");

    await userEvent.click(await screen.findByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    expect(
      await screen.findByRole("status", { name: "Context usage 0%" }),
    ).toHaveTextContent("0%");

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        type: "usage",
        usage: {
          cacheReadTokens: 0,
          cacheWriteTokens: 0,
          inputTokens: 70000,
          outputTokens: 1000,
        },
      });
    });

    expect(
      screen.getByRole("status", { name: "Context usage 0%" }),
    ).toHaveTextContent("0%");

    await userEvent.click(screen.getByRole("tab", { name: /Tool run/ }));
    expect(
      await screen.findByRole("status", { name: "Context usage 64%" }),
    ).toHaveTextContent("64%");

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream")?.close();
    });
  });

  it("collapses streaming thinking once answer text starts", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Need file context.",
        type: "reasoningDelta",
      });
    });
    const thinkingToggle = await screen.findByRole("button", {
      name: "Collapse thinking",
    });
    expect(thinkingToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Need file context.")).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Final answer.",
        type: "textDelta",
      });
    });

    await waitFor(() => {
      expect(thinkingToggle).toHaveAttribute("aria-expanded", "false");
    });
    expect(
      screen.getByText("Need file context.", { selector: "span" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Final answer.")).toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("tracks each streaming thinking block duration independently", async () => {
    const nowSpy = vi.spyOn(Date, "now");
    nowSpy.mockReturnValue(1_000);

    try {
      renderApp();
      await userEvent.click(await screen.findByText("Tool run"));
      await userEvent.type(
        await screen.findByPlaceholderText(defaultComposerPlaceholder),
        "multi think",
      );
      await userEvent.click(screen.getByRole("button", { name: "Send message" }));
      await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

      await act(async () => {
        enqueueChatStreamEvent({
          assistantMessageId: "message-assistant-stream",
          delta: "First plan.",
          type: "reasoningDelta",
        });
      });

      nowSpy.mockReturnValue(2_000);
      await act(async () => {
        enqueueChatStreamEvent({
          assistantMessageId: "message-assistant-stream",
          delta: "Interim answer.",
          type: "textDelta",
        });
      });

      nowSpy.mockReturnValue(5_000);
      await act(async () => {
        enqueueChatStreamEvent({
          assistantMessageId: "message-assistant-stream",
          delta: "Second plan.",
          type: "reasoningDelta",
        });
      });

      nowSpy.mockReturnValue(7_000);
      await act(async () => {
        enqueueChatStreamEvent({
          assistantMessageId: "message-assistant-stream",
          chatId: "chat-1",
          memoriesUsed: [],
          metrics: {
            firstTokenLatencyMs: null,
            modelId: "gpt-test",
            outputTokens: null,
            providerId: "openai",
            totalLatencyMs: 9_000,
          },
          reasoning: "First plan.Second plan.",
          stopReason: null,
          text: "Interim answer.",
          type: "complete",
          usage: null,
        });
        appTestState.activeChatStreamController?.close();
      });

      const answer = await screen.findByText("Interim answer.");
      const assistantRow = answer.closest(".message-row") as HTMLElement | null;
      expect(assistantRow).not.toBeNull();
      const thinkingToggles = within(assistantRow as HTMLElement).getAllByRole(
        "button",
        { name: "Expand thinking" },
      );
      expect(thinkingToggles).toHaveLength(2);
      expect(within(thinkingToggles[0]).getByText("1 s")).toBeInTheDocument();
      expect(within(thinkingToggles[1]).getByText("2 s")).toBeInTheDocument();
      expect(within(assistantRow as HTMLElement).getByText("First plan.", { selector: "span" })).toBeInTheDocument();
      expect(within(assistantRow as HTMLElement).getByText("Second plan.", { selector: "span" })).toBeInTheDocument();
      expect(answer).toBeInTheDocument();
    } finally {
      nowSpy.mockRestore();
    }
  });

  it("sends guidance to the active run without ending the current stream", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "start work",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "prefer the simpler path",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Send guidance" }),
    );

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url === "/api/workspaces/workspace-1/chat/guidance",
        ),
      ).toBe(true);
    });
    const guidanceCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/guidance",
    );
    expect(JSON.parse(String(guidanceCall?.[1]?.body))).toMatchObject({
      chatId: "chat-1",
      message: "prefer the simpler path",
      runId: "request-stream",
    });
    const pendingGuidanceMessage = screen.getByText("prefer the simpler path");
    const pendingGuidanceRow = pendingGuidanceMessage.closest(".message-row");
    expect(pendingGuidanceRow).not.toBeNull();
    expect(
      within(pendingGuidanceRow as HTMLElement).getByText("Guidance pending"),
    ).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Initial answer.",
        type: "textDelta",
      });
    });
    const initialAnswer = await screen.findByText("Initial answer.");
    expect(initialAnswer).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        content: "prefer the simpler path",
        id: "guidance-1",
        interruptedAssistantMetrics: {
          firstTokenLatencyMs: 250,
          modelId: "gpt-test",
          outputTokens: 10,
          providerId: "openai",
          totalLatencyMs: 2000,
        },
        parts: [],
        type: "guidanceApplied",
      });
    });
    const guidanceMessage = screen.getByText("prefer the simpler path");
    expect(guidanceMessage).toBeInTheDocument();
    const guidanceRow = guidanceMessage.closest(".message-row");
    expect(guidanceRow).not.toBeNull();
    expect(
      within(guidanceRow as HTMLElement).queryByText("Guidance pending"),
    ).not.toBeInTheDocument();
    const interruptedAssistantRow = initialAnswer.closest(".message-row");
    expect(interruptedAssistantRow).not.toBeNull();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText("Model: gpt-test"),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText("Channel: openai"),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText("Total time: 2 s"),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText("tokens/s: 5"),
    ).toBeInTheDocument();
    expect(
      within(interruptedAssistantRow as HTMLElement).getByText(
        "First token latency: 0.25 s",
      ),
    ).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Adjusted answer.",
        type: "textDelta",
      });
    });
    const guidedAnswer = await screen.findByText("Adjusted answer.");
    expect(
      guidanceMessage.compareDocumentPosition(guidedAnswer) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    // The interrupted bubble keeps its original content and does not absorb the
    // post-guidance response text, even though the backend emits that text under
    // the original assistant message id.
    const guidedAnswerRow = guidedAnswer.closest(".message-row");
    expect(guidedAnswerRow).not.toBeNull();
    const initialAnswerRow = initialAnswer.closest(".message-row");
    expect(initialAnswerRow).not.toBeNull();
    expect(
      within(initialAnswerRow as HTMLElement).queryByText("Adjusted answer."),
    ).not.toBeInTheDocument();
    expect(
      within(guidedAnswerRow as HTMLElement).queryByText("Initial answer."),
    ).not.toBeInTheDocument();

    // Tool calls emitted after the guidance boundary must attach to the new
    // bubble and resolve to a terminal status, never getting stuck "running".
    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        toolCall: {
          id: "call-guided",
          input: {},
          isError: false,
          name: "noop",
          output: null,
          status: "running",
        },
        type: "toolCall",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        isError: false,
        output: "ok",
        toolCallId: "call-guided",
        type: "toolResult",
      });
    });
    expect(
      within(guidedAnswerRow as HTMLElement).getByText(/noop/),
    ).toBeInTheDocument();
    expect(
      within(guidedAnswerRow as HTMLElement).queryByText(/running/i),
    ).not.toBeInTheDocument();
    expect(
      within(initialAnswerRow as HTMLElement).queryByText(/noop/),
    ).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("cancels the active run id after a later provider attempt starts", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "start work",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        llmRequestId: "llm-turn-2",
        type: "streamAttemptStart",
      });
    });

    await userEvent.click(screen.getByRole("button", { name: "Cancel run" }));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url === "/api/workspaces/workspace-1/chat/runs/request-stream/cancel",
        ),
      ).toBe(true);
    });
    expect(
      fetchMock.mock.calls.some(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/chat/runs/llm-turn-2/cancel",
      ),
    ).toBe(false);
  });

  it("queues a message during an active run and sends it after the stream ends", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "next task",
    );
    await userEvent.click(screen.getByLabelText("Model"));
    await userEvent.click(screen.getByRole("button", { name: "Anthropic: GPT Test" }));
    await userEvent.click(screen.getByLabelText("Thinking"));
    await userEvent.click(screen.getByRole("button", { name: "Thinking: High" }));
    fireEvent.click(screen.getByRole("button", { name: "Send guidance" }), {
      ctrlKey: true,
    });
    const pendingQueuedMessage = await screen.findByText("next task");
    const pendingQueuedRow = pendingQueuedMessage.closest(".message-row");
    expect(pendingQueuedRow).not.toBeNull();
    expect(
      within(pendingQueuedRow as HTMLElement).getByText("Queued"),
    ).toBeInTheDocument();
    const streamCallsBeforeComplete = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(streamCallsBeforeComplete).toHaveLength(1);

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: null,
          modelId: "gpt-test",
          outputTokens: null,
          providerId: "openai",
          totalLatencyMs: 10,
        },
        reasoning: null,
        stopReason: null,
        text: "Done.",
        type: "complete",
        usage: null,
      });
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() => {
      const streamCalls = fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/chat/stream",
      );
      expect(streamCalls).toHaveLength(2);
    });
    const secondStreamCall = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    )[1];
    expect(JSON.parse(String(secondStreamCall[1]?.body))).toMatchObject({
      chatId: "chat-1",
      message: "next task",
      providerId: "anthropic",
      thinkingLevel: "high",
    });
    const effectiveQueuedMessage = screen.getByText("next task");
    const effectiveQueuedRow = effectiveQueuedMessage.closest(".message-row");
    expect(effectiveQueuedRow).not.toBeNull();
    expect(
      within(effectiveQueuedRow as HTMLElement).queryByText("Queued"),
    ).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("withdraws a queued message before it is sent", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "next task",
    );
    fireEvent.click(screen.getByRole("button", { name: "Send guidance" }), {
      ctrlKey: true,
    });
    const pendingQueuedMessage = await screen.findByText("next task");
    const pendingQueuedRow = pendingQueuedMessage.closest(".message-row");
    expect(pendingQueuedRow).not.toBeNull();

    await userEvent.click(
      within(pendingQueuedRow as HTMLElement).getByRole("button", {
        name: "Withdraw queued message",
      }),
    );

    expect(screen.queryByText("next task")).not.toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: null,
          modelId: "gpt-test",
          outputTokens: null,
          providerId: "openai",
          totalLatencyMs: 10,
        },
        reasoning: null,
        stopReason: null,
        text: "Done.",
        type: "complete",
        usage: null,
      });
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Cancel run" }),
      ).not.toBeInTheDocument(),
    );
    const streamCalls = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(streamCalls).toHaveLength(1);
  });

  it("converts a queued message into active-run guidance", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "next task",
    );
    fireEvent.click(screen.getByRole("button", { name: "Send guidance" }), {
      ctrlKey: true,
    });
    const pendingQueuedMessage = await screen.findByText("next task");
    const pendingQueuedRow = pendingQueuedMessage.closest(".message-row");
    expect(pendingQueuedRow).not.toBeNull();

    await userEvent.click(
      within(pendingQueuedRow as HTMLElement).getByRole("button", {
        name: "Convert queued message to guidance",
      }),
    );

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url === "/api/workspaces/workspace-1/chat/guidance",
        ),
      ).toBe(true);
    });
    const guidanceCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/guidance",
    );
    expect(JSON.parse(String(guidanceCall?.[1]?.body))).toMatchObject({
      chatId: "chat-1",
      message: "next task",
      runId: "request-stream",
    });

    const pendingGuidanceMessage = screen.getByText("next task");
    const pendingGuidanceRow = pendingGuidanceMessage.closest(".message-row");
    expect(pendingGuidanceRow).not.toBeNull();
    expect(
      within(pendingGuidanceRow as HTMLElement).getByText("Guidance pending"),
    ).toBeInTheDocument();
    expect(
      within(pendingGuidanceRow as HTMLElement).queryByText("Queued"),
    ).not.toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        content: "next task",
        id: "guidance-1",
        interruptedAssistantMetrics: null,
        parts: [],
        type: "guidanceApplied",
      });
    });
    const guidanceMessage = screen.getByText("next task");
    const guidanceRow = guidanceMessage.closest(".message-row");
    expect(guidanceRow).not.toBeNull();
    expect(
      within(guidanceRow as HTMLElement).queryByText("Guidance pending"),
    ).not.toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "guidance-1-assistant",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: null,
          modelId: "gpt-test",
          outputTokens: null,
          providerId: "openai",
          totalLatencyMs: 10,
        },
        reasoning: null,
        stopReason: null,
        text: "Guided done.",
        type: "complete",
        usage: null,
      });
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Cancel run" }),
      ).not.toBeInTheDocument(),
    );
    const streamCalls = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(streamCalls).toHaveLength(1);
  });

  it("starts another chat stream while a different chat is still running", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(true),
    );

    await userEvent.click(await screen.findByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "second task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const streamCalls = fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/chat/stream",
      );
      expect(streamCalls).toHaveLength(2);
    });
    const guidanceCalls = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/guidance",
    );
    expect(guidanceCalls).toHaveLength(0);
    const secondStreamCall = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    )[1];
    expect(JSON.parse(String(secondStreamCall[1]?.body))).toMatchObject({
      chatId: "chat-2",
      message: "second task",
    });

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream")?.close();
      appTestState.chatStreamControllers.get("request-stream-2")?.close();
    });
  });

  it("starts a new chat instead of sending guidance while another chat is running", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(true),
    );

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "new chat task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const streamCalls = fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/chat/stream",
      );
      expect(streamCalls).toHaveLength(2);
    });
    const guidanceCalls = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/guidance",
    );
    expect(guidanceCalls).toHaveLength(0);
    const secondStreamCall = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    )[1];
    expect(JSON.parse(String(secondStreamCall[1]?.body))).toMatchObject({
      chatId: "queued-chat-2",
      message: "new chat task",
      queuedUserMessageId: "queued-user-2",
    });

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream")?.close();
      appTestState.chatStreamControllers.get("request-stream-2")?.close();
    });
  });

  it("opens a new chat tab before the stream start event arrives", async () => {
    const fetchMock = vi.mocked(fetch);
    let delayedStreamController: ReadableStreamDefaultController<Uint8Array> | null =
      null;
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chat/stream") {
        const stream = new ReadableStream<Uint8Array>({
          start(controller) {
            delayedStreamController = controller;
          },
        });

        return Promise.resolve(
          new Response(stream, {
            headers: { "Content-Type": "text/event-stream" },
            status: 200,
          }),
        );
      }

      return mockFetch(input, init);
    });
    renderApp();

    await userEvent.click(
      await screen.findByRole("button", { name: "New chat in Default" }),
    );
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "memory-gated chat",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(
      await within(tabList).findByRole("tab", { name: /memory-gated chat/ }),
    ).toHaveAttribute("aria-selected", "true");
    expect(
      within(tabList).getByRole("status", { name: "Chat is running" }),
    ).toBeInTheDocument();
    const streamCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(JSON.parse(String(streamCall?.[1]?.body))).toMatchObject({
      chatId: "queued-chat-1",
      message: "memory-gated chat",
      queuedUserMessageId: "queued-user-1",
    });

    await act(async () => {
      delayedStreamController?.close();
    });
  });

  it("schedules a new workspace chat until the current workspace run finishes", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation(async (input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chat/stream") {
        const body =
          typeof init?.body === "string"
            ? (JSON.parse(init.body) as { chatId?: string | null; message?: string })
            : {};
        if (body.chatId && body.message === "Scheduled task") {
          appTestState.workspaceResponseWorkspaces = [
            {
              ...workspace,
              chats: [
                ...workspace.chats,
                chatSummary(
                  body.chatId,
                  "Scheduled task",
                  "2026-06-05T12:00:00Z",
                  "2026-06-05T12:00:00Z",
                ),
              ],
            },
            secondaryWorkspace,
          ];
          return chatStreamResponse(body.chatId);
        }
      }

      return mockFetch(input, init);
    });
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(true),
    );

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "Scheduled task",
    );
    fireEvent.click(screen.getByRole("button", { name: "Send message" }), {
      ctrlKey: true,
    });

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const scheduledHistoryTitle = await within(workspaceList).findByText(
      "Scheduled task",
    );
    const scheduledHistoryButton = scheduledHistoryTitle.closest("button");
    if (!scheduledHistoryButton) {
      throw new Error("Expected scheduled chat history item button");
    }
    expect(
      scheduledHistoryButton.querySelector(".session-status-dot"),
    ).toHaveClass("session-status-dot-scheduled");

    const scheduledMessage = screen
      .getAllByText("Scheduled task")
      .find((element) => element.closest(".message-row"));
    const scheduledMessageRow = scheduledMessage?.closest(".message-row");
    expect(scheduledMessageRow).not.toBeNull();
    expect(
      within(scheduledMessageRow as HTMLElement).getByText("Queued"),
    ).toBeInTheDocument();

    const tabListBeforeComplete = await screen.findByRole("tablist", { name: "Chat" });
    await userEvent.click(
      within(tabListBeforeComplete).getByRole("tab", { name: /Tool run/ }),
    );
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    const streamCallsBeforeComplete = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(streamCallsBeforeComplete).toHaveLength(1);

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: null,
          modelId: "gpt-test",
          outputTokens: null,
          providerId: "openai",
          totalLatencyMs: 10,
        },
        reasoning: null,
        stopReason: null,
        text: "Done.",
        type: "complete",
        usage: null,
      });
      appTestState.chatStreamControllers.get("request-stream")?.close();
    });

    await waitFor(() => {
      const streamCalls = fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/chat/stream",
      );
      expect(streamCalls).toHaveLength(2);
    });

    const secondStreamCall = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    )[1];
    expect(JSON.parse(String(secondStreamCall[1]?.body))).toMatchObject({
      chatId: "queued-chat-2",
      message: "Scheduled task",
      queuedUserMessageId: "queued-user-2",
    });

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream-2", {
        assistantMessageId: "message-assistant-stream-2",
        delta: "Scheduled answer.",
        type: "textDelta",
      });
    });

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    const activeMessageList = document.querySelector(".message-list");
    if (!(activeMessageList instanceof HTMLElement)) {
      throw new Error("Expected message list");
    }
    expect(within(activeMessageList).getByText("Please inspect README.")).toBeInTheDocument();
    expect(within(activeMessageList).queryByText("Scheduled task")).not.toBeInTheDocument();
    expect(within(activeMessageList).queryByText("Scheduled answer.")).not.toBeInTheDocument();

    await userEvent.click(within(tabList).getByRole("tab", { name: /Scheduled task/ }));
    const scheduledMessageList = document.querySelector(".message-list");
    if (!(scheduledMessageList instanceof HTMLElement)) {
      throw new Error("Expected scheduled message list");
    }
    expect(await within(scheduledMessageList).findByText("Scheduled task")).toBeInTheDocument();
    expect(await within(scheduledMessageList).findByText("Scheduled answer.")).toBeInTheDocument();

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream-2")?.close();
    });
  });

  it("schedules a new workspace chat when Ctrl is held before clicking send", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(true),
    );

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "Held Ctrl scheduled task",
    );
    const sendButton = screen.getByRole("button", { name: "Send message" });
    fireEvent.keyDown(window, { ctrlKey: true, key: "Control" });
    await waitFor(() =>
      expect(sendButton).toHaveAttribute("title", "Send to queue"),
    );
    fireEvent.click(sendButton);
    fireEvent.keyUp(window, { ctrlKey: false, key: "Control" });

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const scheduledHistoryButton = (
      await within(workspaceList).findByText("Held Ctrl scheduled task")
    ).closest("button");
    if (!scheduledHistoryButton) {
      throw new Error("Expected scheduled chat history item button");
    }
    expect(
      scheduledHistoryButton.querySelector(".session-status-dot"),
    ).toHaveClass("session-status-dot-scheduled");

    const streamCallsBeforeComplete = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(streamCallsBeforeComplete).toHaveLength(1);

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream")?.close();
    });

    await waitFor(() => {
      const streamCalls = fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/chat/stream",
      );
      expect(streamCalls).toHaveLength(2);
    });

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream-2")?.close();
    });
  }, 10000);

  it("schedules a new workspace chat with Ctrl+Enter", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "first task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() =>
      expect(appTestState.chatStreamControllers.has("request-stream")).toBe(true),
    );

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    const composer = screen.getByPlaceholderText(defaultComposerPlaceholder);
    await userEvent.type(composer, "Keyboard scheduled task");
    fireEvent.keyDown(composer, {
      ctrlKey: true,
      key: "Enter",
    });

    const streamCallsBeforeComplete = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    expect(streamCallsBeforeComplete).toHaveLength(1);
    const scheduledMessageRow = (
      await screen.findAllByText("Keyboard scheduled task")
    )
      .find((element) => element.closest(".message-row"))
      ?.closest(".message-row");
    expect(scheduledMessageRow).not.toBeNull();
    expect(
      within(scheduledMessageRow as HTMLElement).getByText("Queued"),
    ).toBeInTheDocument();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const firstScheduledHistoryButton = (
      await within(workspaceList).findByText("Keyboard scheduled task")
    ).closest("button");
    if (!firstScheduledHistoryButton) {
      throw new Error("Expected first scheduled chat history button");
    }
    expect(
      firstScheduledHistoryButton.querySelector(".session-status-dot"),
    ).toHaveClass("session-status-dot-scheduled");

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "Click scheduled task",
    );
    fireEvent.click(screen.getByRole("button", { name: "Send message" }), {
      ctrlKey: true,
    });

    const secondScheduledHistoryButton = (
      await within(workspaceList).findByText("Click scheduled task")
    ).closest("button");
    if (!secondScheduledHistoryButton) {
      throw new Error("Expected second scheduled chat history button");
    }
    expect(
      secondScheduledHistoryButton.querySelector(".session-status-dot"),
    ).toHaveClass("session-status-dot-scheduled");
    expect(firstScheduledHistoryButton).not.toBe(secondScheduledHistoryButton);

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream")?.close();
    });

    await waitFor(() => {
      const streamCalls = fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/chat/stream",
      );
      expect(streamCalls).toHaveLength(2);
    });

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream-2")?.close();
    });

    await waitFor(() => {
      const streamCalls = fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/chat/stream",
      );
      expect(streamCalls).toHaveLength(3);
    });

    await act(async () => {
      appTestState.chatStreamControllers.get("request-stream-3")?.close();
    });
  }, 10000);

  it("shows the queue tooltip while Ctrl is held over the send button", async () => {
    renderApp();

    const sendButton = await screen.findByRole("button", {
      name: "Send message",
    });
    expect(sendButton).toHaveAttribute("title", "Send");

    fireEvent.mouseEnter(sendButton);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Send");

    fireEvent.keyDown(window, { ctrlKey: true, key: "Control" });
    await waitFor(() =>
      expect(screen.getByRole("tooltip")).toHaveTextContent("Send to queue"),
    );
    await waitFor(() =>
      expect(sendButton).toHaveAttribute("title", "Send to queue"),
    );

    fireEvent.keyUp(window, { ctrlKey: false, key: "Control" });
    await waitFor(() =>
      expect(screen.getByRole("tooltip")).toHaveTextContent("Send"),
    );
    await waitFor(() => expect(sendButton).toHaveAttribute("title", "Send"));

    fireEvent.mouseLeave(sendButton);
    await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull());
  });

  it("adds browser file attachments into the composer and sends them with the chat request", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await screen.findByText("Tool run");
    const addAttachmentButton = screen.getByRole("button", { name: "Add attachment" });
    await waitFor(() => expect(addAttachmentButton).toBeEnabled());
    const fileInput = document.querySelector<HTMLInputElement>(
      'input[type="file"][multiple]',
    );
    expect(fileInput).not.toBeNull();
    await userEvent.upload(
      fileInput as HTMLInputElement,
      new File(["Hello"], "note.txt", { type: "text/plain" }),
    );
    expect(await screen.findByText("note.txt")).toBeInTheDocument();

    await userEvent.click(screen.getByLabelText("Model"));
    await userEvent.click(screen.getByRole("button", { name: "Anthropic: GPT Test" }));
    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "Review it");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url === "/api/workspaces/workspace-1/chat/stream",
        ),
      ).toBe(true);
    });
    const chatStreamCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );
    const body = JSON.parse(String(chatStreamCall?.[1]?.body));

    expect(body).toEqual(
      expect.objectContaining({
        attachments: [
          expect.objectContaining({
            contentBase64: "SGVsbG8=",
            contentType: "text/plain",
            name: "note.txt",
            sizeBytes: 5,
          }),
        ],
        message: "Review it",
        providerId: "anthropic",
      }),
    );


    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("waits for a streaming Mermaid fence to close before rendering", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "diagram");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "```mermaid\nflowchart TD",
        type: "textDelta",
      });
    });

    expect(await screen.findByText("flowchart TD")).toBeInTheDocument();
    expect(screen.queryByText("Mermaid diagram failed to render.")).not.toBeInTheDocument();
    expect(mermaidMock.render).not.toHaveBeenCalled();

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "\n  A --> B\n```",
        type: "textDelta",
      });
    });

    expect(await screen.findByTestId("mermaid-svg")).toBeInTheDocument();
    expect(mermaidMock.render).toHaveBeenCalledWith(
      expect.stringMatching(/^foco-mermaid-/),
      "flowchart TD\n  A --> B",
    );

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("shows retrieved memories as soon as the chat stream starts", async () => {
    renderApp();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "use memory",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await userEvent.click(await screen.findByText("Memories used"));
    expect(screen.getByText("Use memory before streaming.")).toBeInTheDocument();
    expect(screen.queryByText("Model: gpt-test")).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("shows saved memories from the current chat stream", async () => {
    renderApp();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "remember this",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        text: "Saved.",
        type: "complete",
        metrics: {
          firstTokenLatencyMs: 100,
          modelId: "gpt-test",
          outputTokens: 2,
          providerId: "openai",
          totalLatencyMs: 500,
        },
        reasoning: null,
        stopReason: null,
        usage: null,
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        extractedMemories: [
          {
            chatId: "chat-1",
            fact: "Prefer seeing saved memories immediately.",
            id: "stream-saved-memory-1",
            kind: "preference",
            scope: "chat",
            status: "pending",
          },
        ],
        type: "memoryExtractionComplete",
      });
    });

    const assistantBubble = (await screen.findByText("Saved.")).closest(
      ".message-bubble",
    );
    expect(assistantBubble).not.toBeNull();
    const memoriesSavedLabel = within(assistantBubble as HTMLElement).getByText(
      "Memories saved",
    );
    await userEvent.click(memoriesSavedLabel);
    expect(
      screen.getByText("Prefer seeing saved memories immediately."),
    ).toBeInTheDocument();
  });

  it("appends stream errors after already rendered assistant text", async () => {
    renderApp();

    await userEvent.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "debug");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Partial answer.",
        type: "textDelta",
      });
      enqueueChatStreamEvent({
        message:
          "skill discovery failed for C:\\Users\\fonla\\Documents\\Repos\\Rutar\\.agents\\skills\\vercel-react-native-skills\\SKILL.md: skill file C:\\Users\\fonla\\Documents\\Repos\\Rutar\\.agents\\skills\\vercel-react-native-skills\\SKILL.md frontmatter field 'description' must not be empty",
        type: "error",
      });
    });

    expect(await screen.findByText("Partial answer.")).toBeInTheDocument();
    expect(
      screen.getAllByText(
        /Rutar\\.agents\\skills\\vercel-react-native-skills\\SKILL\.md/,
      ).length,
    ).toBeGreaterThan(0);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("shows hook blocking notifications in the active chat", async () => {
    renderApp();

    await userEvent.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "danger");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        notification: {
          event: "PreToolUse",
          level: "error",
          message: "Hook blocked run_command: denied",
        },
        type: "hookNotification",
      });
    });

    expect(await screen.findByText("Hook blocked run_command: denied")).toBeInTheDocument();
    expect(
      screen.getByText("[PreToolUse] Hook blocked run_command: denied"),
    ).toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("reflects chat tab and running state in workspace chat dots", async () => {
    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyTitle = await within(workspaceList).findByText("Tool run");
    const historyButton = historyTitle.closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }

    const statusDot = () => historyButton.querySelector(".session-status-dot");
    expect(statusDot()).toHaveClass("session-status-dot-idle");

    await userEvent.click(historyButton);
    await screen.findByText("Please inspect README.");
    expect(statusDot()).toHaveClass("session-status-dot-open");

    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "continue");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() =>
      expect(statusDot()).toHaveClass("session-status-dot-running"),
    );

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() =>
      expect(statusDot()).toHaveClass("session-status-dot-open"),
    );

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    await userEvent.click(
      within(tabList).getByRole("button", { name: "Close chat tab Tool run" }),
    );

    expect(statusDot()).toHaveClass("session-status-dot-idle");
  });

  it("marks workspace chat dots red after an interrupted stream", async () => {
    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyTitle = await within(workspaceList).findByText("Tool run");
    const historyButton = historyTitle.closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }

    const statusDot = () => historyButton.querySelector(".session-status-dot");

    await userEvent.click(historyButton);
    await screen.findByText("Please inspect README.");
    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "continue");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() =>
      expect(statusDot()).toHaveClass("session-status-dot-running"),
    );

    await act(async () => {
      enqueueChatStreamEvent({
        message: "network disconnected",
        type: "error",
      });
    });

    await waitFor(() =>
      expect(statusDot()).toHaveClass("session-status-dot-error"),
    );

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() =>
      expect(statusDot()).toHaveClass("session-status-dot-error"),
    );

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    await userEvent.click(
      within(tabList).getByRole("button", { name: "Close chat tab Tool run" }),
    );

    expect(statusDot()).toHaveClass("session-status-dot-idle");
  });

  it("shows a running spinner instead of a close button on a streaming chat tab", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(
      within(tabList).getByRole("button", { name: "Close chat tab Tool run" }),
    ).toBeInTheDocument();

    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "continue");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    expect(
      await within(tabList).findByRole("status", { name: "Chat is running" }),
    ).toBeInTheDocument();
    expect(
      within(tabList).queryByRole("button", { name: "Close chat tab Tool run" }),
    ).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("reattaches to an active run when loading chat messages", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({
          ...chatMessages,
          activeRun: {
            chatId: "chat-1",
            lastSequence: 0,
            runId: "request-stream",
            workspaceId: "workspace-1",
          },
        });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url ===
              "/api/workspaces/workspace-1/chat/runs/request-stream/stream?afterSequence=-1",
        ),
      ).toBe(true);
    });

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        delta: "Still running.",
        type: "textDelta",
      });
    });

    expect(await screen.findByText("Still running.")).toBeInTheDocument();
    expect(screen.getByRole("status", { name: "Chat is running" })).toBeInTheDocument();

    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        type: "usage",
        usage: {
          cacheReadTokens: 0,
          cacheWriteTokens: 0,
          inputTokens: 70000,
          outputTokens: 1000,
        },
      });
    });

    expect(
      await screen.findByRole("status", { name: "Context usage 64%" }),
    ).toHaveTextContent("64%");
    const usageCalls = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    const [, usageInit] = usageCalls.at(-1)!;
    expect(typeof usageInit?.body).toBe("string");
    expect(JSON.parse(usageInit?.body as string)).toMatchObject({
      chatId: "chat-1",
      latestResponseUsage: {
        cacheReadTokens: 0,
        cacheWriteTokens: 0,
        inputTokens: 70000,
        outputTokens: 1000,
      },
    });

    const usageCallCountBeforeComplete = usageCalls.length;
    await act(async () => {
      enqueueChatStreamEventForRun("request-stream", {
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        metrics: {
          firstTokenLatencyMs: 10,
          modelId: "model-1",
          outputTokens: 9000,
          providerId: "provider-1",
          totalLatencyMs: 1000,
        },
        reasoning: null,
        stopReason: "completed",
        text: "Final answer.",
        type: "complete",
        usage: {
          cacheReadTokens: 0,
          cacheWriteTokens: 0,
          inputTokens: 999999,
          outputTokens: 9000,
        },
      });
    });

    const usageCallsAfterComplete = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCallsAfterComplete).toHaveLength(usageCallCountBeforeComplete);
    expect(
      screen.getByRole("status", { name: "Context usage 64%" }),
    ).toHaveTextContent("64%");

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

});
