import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  activeMemory,
  appTestState,
  changeInput,
  defaultComposerPlaceholder,
  sideProjectComposerPlaceholder,
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

describe("app-shell verification surfaces", () => {
  beforeEach(resetAppTestEnvironment);

  it("renders the workspace sidebar and persisted chat tool results", async () => {
    renderApp();

    expect(await screen.findAllByText("Default")).not.toHaveLength(0);
    expect(screen.getAllByText("Tool run").length).toBeGreaterThan(0);
    expect(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByText("Tool run"));

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    const userBubble = screen
      .getByText("Please inspect README.")
      .closest(".message-bubble") as HTMLElement | null;
    const assistantBubble = screen
      .getByText("Done.")
      .closest(".message-bubble") as HTMLElement | null;
    expect(userBubble).toHaveClass("message-bubble-user");
    expect(userBubble).not.toHaveClass("bg-teal-800", "text-white");
    expect(userBubble?.getAttribute("style")).toContain(
      "background-color: var(--foco-user-surface)",
    );
    expect(userBubble?.getAttribute("style")).toContain(
      "border-color: var(--foco-user-border)",
    );
    expect(assistantBubble).toHaveClass("message-bubble-assistant");
    expect(assistantBubble?.getAttribute("style")).toContain(
      "background-color: var(--foco-panel)",
    );
    expect(assistantBubble?.getAttribute("style")).toContain(
      "border-color: var(--foco-border)",
    );
    expect(userBubble?.querySelector("time")).toHaveAttribute(
      "dateTime",
      "2026-06-10T08:00:00.000Z",
    );
    expect(userBubble?.querySelector(".message-model-id")).toBeNull();
    expect(assistantBubble?.querySelector("time")).toHaveAttribute(
      "dateTime",
      "2026-06-10T08:00:02.000Z",
    );
    expect(
      assistantBubble?.querySelector(".message-model-id"),
    ).toHaveTextContent("gpt-test");
    const userRow = userBubble?.closest(".message-row") as HTMLElement | null;
    const assistantRow = assistantBubble?.closest(
      ".message-row",
    ) as HTMLElement | null;
    if (!userBubble || !assistantBubble || !userRow || !assistantRow) {
      throw new Error("Expected message rows");
    }
    const userCopyButton = within(userBubble).getByRole(
      "button",
      { name: "Copy message" },
    );
    const assistantCopyButton = within(assistantBubble).getByRole(
      "button",
      { name: "Copy message" },
    );
    expect(userCopyButton.closest(".message-author-row")).toBe(
      userBubble?.querySelector(".message-author-row"),
    );
    expect(assistantCopyButton.closest(".message-author-row")).toBe(
      assistantBubble?.querySelector(".message-author-row"),
    );
    await userEvent.click(userCopyButton);
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      "Please inspect README.",
    );
    expect(
      within(userRow).getByRole("button", { name: "Copied message" }),
    ).toBeInTheDocument();
    await userEvent.click(assistantCopyButton);
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("Done.");
    const reasoningToggle = screen.getByRole("button", {
      name: "Expand thinking",
    });
    expect(reasoningToggle).toHaveAttribute("aria-expanded", "false");
    expect(within(reasoningToggle).getByText("2 s")).toBeInTheDocument();
    expect(screen.getByText("Need file context. Then answer.")).toBeInTheDocument();
    expect(screen.queryByText("Then answer.")).not.toBeInTheDocument();

    await userEvent.click(reasoningToggle);

    expect(reasoningToggle).toHaveAttribute("aria-expanded", "true");
    expect(within(reasoningToggle).getByText("2 s")).toBeInTheDocument();
    expect(screen.getByText("Need file context.")).toBeInTheDocument();
    expect(screen.getByText("Then answer.")).toBeInTheDocument();
    expect(screen.getByText("edit_file")).toBeInTheDocument();
    expect(screen.getByText("+1")).toHaveClass("text-emerald-700");
    expect(screen.getByText("-1")).toHaveClass("text-rose-700");
    expect(screen.getByText("README.md")).toBeInTheDocument();
    expect(
      screen.getByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"oldStr": "hello"')),
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("Done.")).toBeInTheDocument();
    expect(await screen.findByTestId("mermaid-svg", undefined, { timeout: 5000 })).toBeInTheDocument();
    expect(mermaidMock.render).toHaveBeenCalledWith(
      expect.stringMatching(/^foco-mermaid-/),
      "flowchart TD\n  A --> B",
    );
    expect(screen.getByText("Model: gpt-test")).toBeInTheDocument();
    expect(screen.getByText("Channel: openai")).toBeInTheDocument();
    expect(screen.getByText("Total time: 2 s")).toBeInTheDocument();
    expect(screen.getByText("tokens/s: 20")).toBeInTheDocument();
    expect(screen.getByText("First token latency: 0.25 s")).toBeInTheDocument();
    const memoriesUsedLabel = within(assistantBubble!).getByText("Memories used");
    const finalAnswer = within(assistantBubble!).getByText("Done.");
    expect(
      memoriesUsedLabel.compareDocumentPosition(finalAnswer) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    await userEvent.click(memoriesUsedLabel);
    expect(screen.getByText("Use memory graph retrieval.")).toBeInTheDocument();
    const memoriesSavedLabel = within(assistantBubble!).getByText("Memories saved");
    expect(
      finalAnswer.compareDocumentPosition(memoriesSavedLabel) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    await userEvent.click(memoriesSavedLabel);
    expect(
      screen.getByText("Remember that README was inspected after completion."),
    ).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Side project" }),
    );
    expect(
      await screen.findByPlaceholderText(sideProjectComposerPlaceholder),
    ).toBeInTheDocument();
  });

  it("keeps the thinking duration visible when reply latency is unavailable", async () => {
    const messagesWithUnknownThinkingDuration = {
      ...chatMessages,
      messages: chatMessages.messages.map((message) =>
        message.id === "message-assistant"
          ? {
              ...message,
              metrics: message.metrics
                ? { ...message.metrics, totalLatencyMs: null }
                : message.metrics,
            }
          : message,
      ),
    };
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...messagesWithUnknownThinkingDuration, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    const reasoningToggle = screen.getByRole("button", {
      name: "Expand thinking",
    });

    expect(within(reasoningToggle).getByText("n/a")).toBeInTheDocument();
  });

  it("stops reading after the stream end event without surfacing transport close errors", async () => {
    renderApp();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "finish cleanly",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        text: "Done without transport error.",
        type: "complete",
        metrics: {
          firstTokenLatencyMs: 100,
          modelId: "gpt-test",
          outputTokens: 4,
          providerId: "openai",
          totalLatencyMs: 500,
        },
        reasoning: null,
        stopReason: null,
        usage: null,
      });
      enqueueChatStreamEvent({ type: "streamEnd" });
    });

    expect(await screen.findByText("Done without transport error.")).toBeInTheDocument();
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    await act(async () => {
      appTestState.activeChatStreamController?.error(new TypeError("network error"));
    });

    expect(screen.queryByText("network error")).not.toBeInTheDocument();
    expect(screen.getByText("Model: gpt-test")).toBeInTheDocument();
  });

  it("shows LLM reconnect and context compression badges in the assistant bubble", async () => {
    renderApp();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "recover and compact",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        reason: "provider stream failed",
        reasoning: null,
        text: "",
        toolCalls: [],
        type: "streamReset",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        kind: "rule",
        snapshotId: "ctx-1",
        type: "contextCompression",
      });
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        text: "Recovered after compaction.",
        type: "complete",
        metrics: {
          firstTokenLatencyMs: 100,
          modelId: "gpt-test",
          outputTokens: 4,
          providerId: "openai",
          totalLatencyMs: 500,
        },
        reasoning: null,
        stopReason: null,
        usage: null,
      });
      enqueueChatStreamEvent({ type: "streamEnd" });
    });

    const assistantText = await screen.findByText("Recovered after compaction.");
    const assistantRow = assistantText.closest(".message-row");
    expect(assistantRow).not.toBeNull();
    expect(
      within(assistantRow as HTMLElement).getByText("Reconnected"),
    ).toBeInTheDocument();
    expect(
      within(assistantRow as HTMLElement).getByText("Rule compressed"),
    ).toBeInTheDocument();
  }, 10000);

  it("localizes completed tool status and uses success color", async () => {
    const zhSettings = {
      ...settings,
      general: {
        ...settings.general,
        language: "zh-CN",
      },
    };
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/settings") {
        return jsonResponse(zhSettings);
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    const assistantBubble = screen
      .getByText("Done.")
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    const completedPill = within(assistantBubble).getByText("已完成");
    expect(completedPill).toHaveClass("bg-emerald-50", "text-emerald-700");
    expect(within(assistantBubble).queryByText("completed")).not.toBeInTheDocument();
  });

  it("opens a settings section from the URL and writes section changes back to the URL", async () => {
    window.history.replaceState(null, "", "/settings/models");
    renderApp();

    expect(await screen.findByText("Model settings")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/settings/models");

    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "General" }));

    expect(await screen.findByText("General settings")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/settings/general");
  });

  it("opens a chat from the URL and writes chat selection changes back to the URL", async () => {
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/workspace-1/chat-1");

    await userEvent.click(screen.getByText("Second chat"));

    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/workspace-1/chat-2");
  });

  it("resizes the workspace sidebar from the panel splitter", async () => {
    renderApp();

    const splitter = await screen.findByRole("separator", {
      name: "Resize workspace sidebar",
    });
    const sidebar = splitter.closest(".workspace-sidebar") as HTMLElement | null;
    const appShell = splitter.closest(".app-shell") as HTMLElement | null;

    if (!sidebar || !appShell) {
      throw new Error("Expected workspace sidebar splitter inside app shell");
    }

    expect(splitter).not.toHaveClass("hidden");
    expect(splitter).not.toHaveClass("lg:block");

    vi.spyOn(sidebar, "getBoundingClientRect").mockReturnValue({
      bottom: 800,
      height: 800,
      left: 48,
      right: 336,
      toJSON: () => ({}),
      top: 0,
      width: 288,
      x: 48,
      y: 0,
    } as DOMRect);

    fireEvent.pointerDown(splitter, { clientX: 336, pointerId: 1 });

    await waitFor(() => {
      expect(document.body.style.cursor).toBe("col-resize");
    });

    fireEvent.pointerMove(window, { clientX: 348 });

    await waitFor(() => {
      expect(appShell.style.getPropertyValue("--sidebar-width")).toBe("300px");
      expect(splitter).toHaveAttribute("aria-valuenow", "300");
    });

    fireEvent.pointerUp(window);

    await waitFor(() => {
      expect(document.body.style.cursor).toBe("");
    });
  });

  it("keeps context panel resize from selecting panel text", async () => {
    renderApp();

    const splitter = await screen.findByRole("separator", {
      name: "Resize context panel",
    });

    fireEvent.pointerDown(splitter, { clientX: 900, pointerId: 1 });

    await waitFor(() => {
      expect(document.body.style.cursor).toBe("col-resize");
      expect(document.body.style.userSelect).toBe("none");
    });

    fireEvent.pointerMove(window, { clientX: 880 });
    fireEvent.pointerUp(window);

    await waitFor(() => {
      expect(document.body.style.cursor).toBe("");
      expect(document.body.style.userSelect).toBe("");
    });
  });

  it("prompts to install ripgrep when the search dependency is missing", async () => {
    const missingRipgrepSettings = {
      ...settings,
      nativeTools: {
        ripgrep: {
          available: false,
          installDir: "C:\\Users\\fonla\\.foco\\bin",
          path: null,
        },
      },
    };
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];

      if (path === "/api/settings") {
        return jsonResponse(missingRipgrepSettings);
      }

      if (path === "/api/native/install-ripgrep") {
        expect(init?.method).toBe("POST");
        return jsonResponse({
          ripgrep: {
            available: true,
            installDir: "C:\\Users\\fonla\\.foco\\bin",
            path: "C:\\Users\\fonla\\.foco\\bin\\rg.exe",
          },
        });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    const dialog = await screen.findByRole("dialog", {
      name: "rg command was not found",
    });
    expect(within(dialog).getByText("C:\\Users\\fonla\\.foco\\bin")).toBeInTheDocument();

    await userEvent.click(within(dialog).getByRole("button", { name: "Download ripgrep" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/native/install-ripgrep",
        expect.objectContaining({ method: "POST" }),
      );
    });
    await waitFor(() => {
      expect(
        screen.queryByRole("dialog", { name: "rg command was not found" }),
      ).not.toBeInTheDocument();
    });
  });

  it("closes composer menus when clicking outside", async () => {
    const user = userEvent.setup();
    const { container } = renderApp();

    const modelSummary = await screen.findByLabelText("Model");
    const thinkingSummary = await screen.findByLabelText("Thinking");
    const branchDetails = await waitFor(() => {
      const element = container.querySelector<HTMLDetailsElement>(
        ".composer-branch-select",
      );
      if (!element) {
        throw new Error("branch selector details was not rendered");
      }
      expect(element.tagName).toBe("DETAILS");
      return element;
    });
    const branchSummary = branchDetails.querySelector("summary");
    expect(branchSummary).not.toBeNull();

    await user.click(modelSummary);
    expect(modelSummary.closest("details")).toHaveAttribute("open");
    await user.click(document.body);
    expect(modelSummary.closest("details")).not.toHaveAttribute("open");

    await user.click(thinkingSummary);
    expect(thinkingSummary.closest("details")).toHaveAttribute("open");
    await user.click(document.body);
    expect(thinkingSummary.closest("details")).not.toHaveAttribute("open");

    await user.click(branchSummary as HTMLElement);
    expect(branchDetails).toHaveAttribute("open");
    await user.click(document.body);
    expect(branchDetails).not.toHaveAttribute("open");
  });

  it("keeps Shift+Enter in the composer as a newline", async () => {
    const fetchMock = vi.mocked(fetch);
    const user = userEvent.setup();
    renderApp();

    const composer = await screen.findByPlaceholderText(defaultComposerPlaceholder);
    await user.click(composer);
    await user.keyboard("Line one{Shift>}{Enter}{/Shift}Line two");

    expect(composer).toHaveValue("Line one\nLine two");
    expect(
      fetchMock.mock.calls.some(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/chat/stream",
      ),
    ).toBe(false);
  });

  it("loads opened chat context usage without recomputing while drafting", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    const usage = await screen.findByRole("status", {
      name: "Context usage 47%",
    });
    expect(usage).toHaveTextContent("47%");

    const usageCallsBeforeDraft = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCallsBeforeDraft.length).toBeGreaterThan(0);
    const [, init] = usageCallsBeforeDraft.at(-1)!;
    expect(typeof init?.body).toBe("string");
    expect(JSON.parse(init?.body as string)).toMatchObject({
      assistantDraft: null,
      assistantDraftReasoning: null,
      chatId: "chat-1",
      draftMessage: null,
    });

    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "continue");

    const usageCallsAfterDraft = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCallsAfterDraft).toHaveLength(usageCallsBeforeDraft.length);
  });

  it("expands a collapsed workspace without adding a placeholder chat row", async () => {
    renderApp();

    const workspaceToggle = await screen.findByRole("button", { name: "Default" });
    await userEvent.click(workspaceToggle);
    expect(workspaceToggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("Tool run")).not.toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );

    expect(workspaceToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.queryByRole("button", { name: "New chat" })).not.toBeInTheDocument();
  });

  it("sends a workspace plus chat as a new chat request", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );
    expect(screen.queryByRole("button", { name: "New chat" })).not.toBeInTheDocument();

    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "Fresh task");
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

    expect(JSON.parse(String(chatStreamCall?.[1]?.body))).toEqual(
      expect.objectContaining({
        chatId: null,
        message: "Fresh task",
      }),
    );

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("keeps the active non-default workspace expanded after sending a new chat", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const defaultToggle = await screen.findByRole("button", { name: "Default" });
    const sideToggle = screen.getByRole("button", { name: "Side project" });
    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Side project" }),
    );

    expect(defaultToggle).toHaveAttribute("aria-expanded", "false");
    expect(sideToggle).toHaveAttribute("aria-expanded", "true");

    await userEvent.type(
      screen.getByPlaceholderText(sideProjectComposerPlaceholder),
      "Side task",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) =>
            typeof url === "string" &&
            url === "/api/workspaces/workspace-2/chat/stream",
        ),
      ).toBe(true);
    });

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() => {
      expect(defaultToggle).toHaveAttribute("aria-expanded", "false");
      expect(sideToggle).toHaveAttribute("aria-expanded", "true");
    });
  });

  it("opens the selected chat workspace and collapses the previous workspace", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    const defaultToggle = screen.getByRole("button", { name: "Default" });
    const sideToggle = screen.getByRole("button", { name: "Side project" });
    await userEvent.click(sideToggle);
    await userEvent.click(await screen.findByText("Side note"));

    expect(defaultToggle).toHaveAttribute("aria-expanded", "false");
    expect(sideToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getAllByText("Side note").length).toBeGreaterThan(0);
  });

  it("allows workspace toggles after selecting a historical chat", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    const defaultToggle = screen.getByRole("button", { name: "Default" });
    const sideToggle = screen.getByRole("button", { name: "Side project" });

    expect(defaultToggle).toHaveAttribute("aria-expanded", "true");

    await userEvent.click(defaultToggle);
    expect(defaultToggle).toHaveAttribute("aria-expanded", "false");

    await userEvent.click(sideToggle);
    expect(sideToggle).toHaveAttribute("aria-expanded", "true");
    expect(await screen.findByText("Side note")).toBeInTheDocument();
  });

  it("shows only the first 5 workspace chats until the menu item expands more", async () => {
    renderApp();

    expect(await screen.findByText("Older chat 3")).toBeInTheDocument();
    expect(screen.queryByText("Older chat 4")).not.toBeInTheDocument();
    expect(screen.getByText("7 hidden chats")).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "Show 5 more chats in Default" }),
    );

    expect(screen.getByText("Older chat 4")).toBeInTheDocument();
    expect(screen.getByText("Older chat 8")).toBeInTheDocument();
    expect(screen.queryByText("Older chat 9")).not.toBeInTheDocument();
    expect(screen.getByText("2 hidden chats")).toBeInTheDocument();

    const defaultToggle = screen.getByRole("button", { name: "Default" });
    await userEvent.click(defaultToggle);
    expect(defaultToggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("Older chat 4")).not.toBeInTheDocument();

    await userEvent.click(defaultToggle);
    expect(defaultToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Older chat 3")).toBeInTheDocument();
    expect(screen.queryByText("Older chat 4")).not.toBeInTheDocument();
    expect(screen.getByText("7 hidden chats")).toBeInTheDocument();
  });

  it("opens center chat tabs and closes tabs without deleting chat history", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(within(tabList).getByText("Default")).toBeInTheDocument();

    await userEvent.click(screen.getByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    expect(
      within(tabList).getByRole("tab", { name: /Second chat/ }),
    ).toHaveAttribute("aria-selected", "true");
    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toBeInTheDocument();

    await userEvent.click(within(tabList).getByRole("tab", { name: /Tool run/ }));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    await userEvent.click(
      within(tabList).getByRole("button", { name: "Close chat tab Tool run" }),
    );

    expect(
      within(tabList).queryByRole("tab", { name: /Tool run/ }),
    ).not.toBeInTheDocument();
    expect(
      within(tabList).getByRole("tab", { name: /Second chat/ }),
    ).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("Tool run")).toBeInTheDocument();

    const messageList = document.querySelector(".message-list");
    if (!(messageList instanceof HTMLElement)) {
      throw new Error("Expected message list to exist");
    }
    messageList.scrollTop = 480;

    await userEvent.click(
      within(tabList).getByRole("button", { name: "Close chat tab Second chat" }),
    );

    expect(await screen.findByText("API overview")).toBeInTheDocument();
    expect(messageList.scrollTop).toBe(0);
  });

});
