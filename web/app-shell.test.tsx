import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFileSync } from "node:fs";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  activeMemory,
  appTestState,
  changeInput,
  defaultComposerPlaceholder,
  sideProjectComposerPlaceholder,
  chatMemory,
  chatMessages,
  chatSummary,
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
  secondChatMessages,
  todoGraph,
  workspace,
  workspaceChats,
  workspaceMemory,
} from "./test-utils/app-test-harness";

function currentChatTabs() {
  return new URLSearchParams(window.location.search).getAll("tab");
}

function currentFileTabs() {
  return new URLSearchParams(window.location.search).getAll("file");
}

function aiStatisticsCallUrlsFromMock(fetchMock: ReturnType<typeof vi.mocked<typeof fetch>>) {
  return fetchMock.mock.calls
    .map(([input]) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      return new URL(rawPath, "http://localhost");
    })
    .filter((url) => url.pathname === "/api/ai-statistics");
}

function getAssistantFinalAnswer(container: HTMLElement) {
  return within(container).getByText((_content, element) =>
    Boolean(
      element?.classList.contains("markdown-content-assistant") &&
        element.textContent?.startsWith("Done."),
    ),
  );
}

describe("app-shell verification surfaces", () => {
  beforeEach(resetAppTestEnvironment);

  it("filters workspace chats across workspaces", async () => {
    appTestState.workspaceResponseWorkspaces = [
      workspace,
      { ...secondaryWorkspace, chats: [] },
    ];
    appTestState.workspaceChatSearchResponseWorkspaces = [
      workspace,
      secondaryWorkspace,
    ];
    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });

    expect(within(workspaceList).queryByText("Side note")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Search chats" }));
    changeInput(screen.getByRole("searchbox", { name: "Search chats" }), "Side");

    await waitFor(() =>
      expect(
        vi
          .mocked(fetch)
          .mock.calls.some(([input]) =>
            String(input).includes("/api/workspaces/search-chats?query=Side"),
          ),
      ).toBe(true),
    );
    expect(await within(workspaceList).findByText("Side note")).toBeInTheDocument();
    expect(within(workspaceList).queryByText("Tool run")).not.toBeInTheDocument();
    expect(within(workspaceList).queryByText("Second chat")).not.toBeInTheDocument();
  });

  it("shows running icons on reload when queuedRun is still running without activeRun", async () => {
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          {
            ...chatSummary(
              "chat-1",
              "Tool run",
              "2026-06-05T10:00:00Z",
              "2026-06-05T10:05:00Z",
            ),
            activeRun: null,
            queuedRun: {
              assistantMessageId: "waiting-assistant",
              content: "waiting for worker",
              modelId: "gpt-test",
              providerId: "openai",
              skillIds: [],
              status: "running",
              thinkingLevel: null,
              userMessageId: "waiting-user",
            },
          },
          ...workspace.chats.filter((chat) => chat.id !== "chat-1"),
        ],
      },
      secondaryWorkspace,
    ];
    window.history.replaceState(null, "", "/?tab=workspace-1%2Fchat-1");

    renderApp();

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyButton = within(workspaceList).getByText("Tool run").closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }

    await waitFor(() =>
      expect(
        within(tabList).getByRole("status", { name: "Chat is running" }),
      ).toBeInTheDocument(),
    );
    expect(tabList.querySelector(".chat-tab-running-spinner")).not.toBeNull();
    expect(historyButton.querySelector(".session-status-dot")).toHaveClass(
      "session-status-dot-running",
    );
    expect(screen.getByRole("button", { name: "Send message" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cancel run" })).not.toBeInTheDocument();
  });

  it("starts a restored queued chat when workspace skillIds is null", async () => {
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          {
            activeRun: null,
            codeChangeStats: { additions: 0, deletions: 0 },
            createdAt: "2026-06-05T13:00:00Z",
            id: "restored-queued-chat",
            queuedRun: {
              assistantMessageId: "restored-assistant",
              content: "resume queued work",
              modelId: "gpt-test",
              providerId: "openai",
              skillIds: null,
              status: "queued",
              thinkingLevel: null,
              userMessageId: "restored-user",
            },
            title: "Restored queued chat",
            updatedAt: "2026-06-05T13:00:00Z",
          },
        ],
      },
      secondaryWorkspace,
    ];

    renderApp();

    await waitFor(() => {
      expect(
        vi.mocked(fetch).mock.calls.some(([input]) =>
          String(input).includes("/api/workspaces/workspace-1/chat/stream"),
        ),
      ).toBe(true);
    });
    expect(appTestState.activeChatStreamController).not.toBeNull();
  });

  it("does not restart the same restored queued chat after a stream error", async () => {
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          {
            activeRun: null,
            codeChangeStats: { additions: 0, deletions: 0 },
            createdAt: "2026-06-05T13:00:00Z",
            id: "restored-queued-chat",
            queuedRun: {
              assistantMessageId: "restored-assistant",
              content: "resume queued work",
              modelId: "gpt-test",
              providerId: "openai",
              skillIds: [],
              status: "queued",
              thinkingLevel: null,
              userMessageId: "restored-user",
            },
            title: "Restored queued chat",
            updatedAt: "2026-06-05T13:00:00Z",
          },
        ],
      },
      secondaryWorkspace,
    ];

    renderApp();

    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());
    await act(async () => {
      enqueueChatStreamEvent({
        message: "invalid provider request",
        type: "error",
      });
      enqueueChatStreamEvent({ type: "streamEnd" });
      appTestState.activeChatStreamController?.close();
    });

    await waitFor(() =>
      expect(
        vi
          .mocked(fetch)
          .mock.calls.filter(([input]) =>
            String(input).includes("/api/workspaces/workspace-1/chat/stream"),
          ),
      ).toHaveLength(1),
    );
    await act(async () => {
      await new Promise((resolve) => window.setTimeout(resolve, 50));
    });
    expect(
      vi
        .mocked(fetch)
        .mock.calls.filter(([input]) =>
          String(input).includes("/api/workspaces/workspace-1/chat/stream"),
        ),
    ).toHaveLength(1);
  });

  it("does not restart a restored queued chat while its first stream is still running", async () => {
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          {
            activeRun: null,
            codeChangeStats: { additions: 0, deletions: 0 },
            createdAt: "2026-06-05T13:00:00Z",
            id: "restored-queued-chat",
            queuedRun: {
              assistantMessageId: "restored-assistant",
              content: "resume queued work",
              modelId: "gpt-test",
              providerId: "openai",
              skillIds: [],
              status: "queued",
              thinkingLevel: null,
              userMessageId: "restored-user",
            },
            title: "Restored queued chat",
            updatedAt: "2026-06-05T13:00:00Z",
          },
        ],
      },
      secondaryWorkspace,
    ];

    renderApp();

    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      await new Promise((resolve) => window.setTimeout(resolve, 50));
    });

    expect(
      vi
        .mocked(fetch)
        .mock.calls.filter(([input]) =>
          String(input).includes("/api/workspaces/workspace-1/chat/stream"),
        ),
    ).toHaveLength(1);

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("refreshes workspaces and renders newly returned chats", async () => {
    renderApp();

    await screen.findByRole("navigation", { name: "Workspace list" });
    const workspaceRequestCount = () =>
      vi
        .mocked(fetch)
        .mock.calls.filter(([input]) => String(input).includes("/api/workspaces"))
        .length;
    const initialWorkspaceRequests = workspaceRequestCount();
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          {
            activeRun: null,
            codeChangeStats: { additions: 0, deletions: 0 },
            createdAt: "2026-06-05T13:00:00Z",
            id: "refreshed-chat",
            title: "Refreshed chat",
            updatedAt: "2026-06-05T13:05:00Z",
          },
          ...workspace.chats,
        ],
      },
      secondaryWorkspace,
    ];

    await userEvent.click(screen.getByRole("button", { name: "Refresh workspaces" }));

    await waitFor(() => {
      expect(workspaceRequestCount()).toBeGreaterThan(initialWorkspaceRequests);
    });
    expect(await screen.findByText("Refreshed chat")).toBeInTheDocument();
  });

  it("syncs open chat tab titles from refreshed workspace chats", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(screen.getByText("Second chat"));
    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toBeInTheDocument();
    expect(within(tabList).getByRole("tab", { name: /Second chat/ })).toBeInTheDocument();

    const generatedTitle = "Generated title";
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          chatSummary(
            "chat-1",
            generatedTitle,
            "2026-06-05T10:00:00Z",
            "2026-06-05T10:06:00Z",
          ),
          ...workspace.chats.slice(1),
        ],
      },
      secondaryWorkspace,
    ];

    await userEvent.click(screen.getByRole("button", { name: "Refresh workspaces" }));

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    expect(await within(workspaceList).findByText(generatedTitle)).toBeInTheDocument();
    await waitFor(() => {
      expect(within(tabList).getByRole("tab", { name: /Generated title/ })).toBeInTheDocument();
    });
    expect(within(tabList).queryByRole("tab", { name: /Tool run/ })).not.toBeInTheDocument();

    const tabTitles = within(tabList)
      .getAllByRole("tab")
      .map((tab) => tab.textContent ?? "");
    expect(tabTitles[0]).toContain(generatedTitle);
    expect(tabTitles[1]).toContain("Second chat");
  });

  it("renders the workspace sidebar and persisted chat tool results", async () => {
    renderApp();

    expect(await screen.findAllByText("Default")).not.toHaveLength(0);
    expect(screen.getAllByText("Tool run").length).toBeGreaterThan(0);
    const composer = await screen.findByPlaceholderText(defaultComposerPlaceholder);
    expect(composer).toBeInTheDocument();
    expect(composer.closest("form")).toHaveClass(
      "message-composer-form",
    );

    await userEvent.click(screen.getByText("Tool run"));

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    const userBubble = screen
      .getByText("Please inspect README.")
      .closest(".message-bubble") as HTMLElement | null;
    const assistantBubble = (await screen.findByLabelText("Edit (edit_file)"))
      .closest(".message-bubble") as HTMLElement | null;
    expect(userBubble).toHaveClass("message-bubble-user");
    expect(userBubble).not.toHaveClass("bg-[var(--accent)]", "text-white");
    expect(userBubble?.getAttribute("style")).toContain(
      "background-color: var(--accent-soft)",
    );
    expect(userBubble?.getAttribute("style")).toContain(
      "border-color: var(--accent)",
    );
    expect(assistantBubble).toHaveClass("message-bubble-assistant");
    expect(assistantBubble?.getAttribute("style")).toContain(
      "background-color: var(--surface)",
    );
    expect(assistantBubble?.getAttribute("style")).toContain(
      "border-color: var(--border)",
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
    expect(reasoningToggle).toHaveClass("text-[var(--muted)]");
    expect(reasoningToggle.closest(".reasoning-block")).toHaveClass(
      "text-[var(--muted)]",
    );
    expect(reasoningToggle).toHaveAttribute("aria-expanded", "false");
    expect(within(reasoningToggle).getByText("2 s")).toBeInTheDocument();
    expect(screen.getByText("Need file context. Then answer.")).toBeInTheDocument();
    expect(screen.queryByText("Then answer.")).not.toBeInTheDocument();

    await userEvent.click(reasoningToggle);

    expect(reasoningToggle).toHaveAttribute("aria-expanded", "true");
    expect(within(reasoningToggle).getByText("2 s")).toBeInTheDocument();
    expect(screen.getByText("Need file context.")).toBeInTheDocument();
    expect(screen.getByText("Then answer.")).toBeInTheDocument();
    expect(within(assistantBubble).getByText("Edit")).toBeInTheDocument();
    expect(screen.getByText("+1")).toHaveClass("text-[var(--success)]");
    expect(screen.getByText("-1")).toHaveClass("text-[var(--danger)]");
    expect(screen.getByText("README.md")).toBeInTheDocument();
    const diffLines = Array.from(
      assistantBubble.querySelectorAll<HTMLElement>(".edit-file-diff-line"),
    );
    const removedLine = diffLines.find((line) => line.textContent === "-hello");
    const addedLine = diffLines.find((line) => line.textContent === "+hello world");
    expect(diffLines.some((line) => line.textContent === "-1hello")).toBe(false);
    expect(diffLines.some((line) => line.textContent === "+1hello world")).toBe(false);
    expect(removedLine).toHaveClass("bg-[var(--danger-soft)]", "text-[var(--danger)]");
    expect(addedLine).toHaveClass("bg-[var(--success-soft)]", "text-[var(--success)]");
    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();
    expect(
      within(assistantBubble).queryByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"oldStr": "hello"')),
      ),
    ).not.toBeInTheDocument();

    await userEvent.click(within(assistantBubble).getByRole("button", { name: "Raw" }));

    expect(within(assistantBubble).getByText("Input")).toBeInTheDocument();
    expect(within(assistantBubble).getByText("Output")).toBeInTheDocument();
    expect(
      within(assistantBubble).getByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"oldStr": "hello"')),
      ),
    ).toBeInTheDocument();

    await userEvent.click(within(assistantBubble).getByRole("button", { name: "Compact" }));

    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();
    expect(Array.from(assistantBubble.querySelectorAll<HTMLElement>(".edit-file-diff-line"))).not.toHaveLength(0);
    expect(getAssistantFinalAnswer(assistantBubble)).toBeInTheDocument();
    expect(await screen.findByTestId("mermaid-svg", undefined, { timeout: 5000 })).toBeInTheDocument();
    expect(mermaidMock.render).toHaveBeenCalledWith(
      expect.stringMatching(/^foco-mermaid-/),
      "flowchart TD\n  A --> B",
    );
    expect(screen.getByText("Model: gpt-test")).toBeInTheDocument();
    expect(screen.getByText("Channel: openai")).toBeInTheDocument();
    expect(screen.getByText("Total time: 2 sec")).toBeInTheDocument();
    expect(screen.getByText("tokens/s: 20")).toBeInTheDocument();
    expect(screen.queryByText(/First token latency/)).not.toBeInTheDocument();
    const memoriesUsedLabel = within(assistantBubble!).getByText("Memories used");
    const finalAnswer = getAssistantFinalAnswer(assistantBubble!);
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

  it("forwards tool call wheel input only at vertical boundaries in compact and raw modes", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    const assistantBubble = (await screen.findByLabelText("Edit (edit_file)"))
      .closest(".message-bubble") as HTMLElement | null;
    const messageList = document.querySelector(
      ".message-list",
    ) as HTMLElement | null;
    if (!assistantBubble || !messageList) {
      throw new Error("Expected assistant bubble and message list");
    }

    messageList.style.overflowY = "auto";
    Object.defineProperties(messageList, {
      clientHeight: { configurable: true, value: 300 },
      scrollHeight: { configurable: true, value: 1200 },
    });
    messageList.scrollTop = 200;

    const compactScroller = assistantBubble.querySelector(
      ".tool-call-scroll",
    ) as HTMLElement | null;
    if (!compactScroller) {
      throw new Error("Expected compact tool-call scroller");
    }
    Object.defineProperties(compactScroller, {
      clientHeight: { configurable: true, value: 120 },
      scrollHeight: { configurable: true, value: 400 },
    });

    compactScroller.scrollTop = 100;
    const midWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: 40,
    });
    compactScroller.dispatchEvent(midWheel);
    expect(messageList.scrollTop).toBe(200);
    expect(midWheel.defaultPrevented).toBe(false);

    compactScroller.scrollTop = 280;
    const bottomWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: 40,
    });
    compactScroller.dispatchEvent(bottomWheel);
    expect(messageList.scrollTop).toBe(240);
    expect(bottomWheel.defaultPrevented).toBe(true);

    compactScroller.scrollTop = 0;
    const topWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: -30,
    });
    compactScroller.dispatchEvent(topWheel);
    expect(messageList.scrollTop).toBe(210);
    expect(topWheel.defaultPrevented).toBe(true);

    compactScroller.scrollTop = 280;
    const horizontalWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaX: 50,
      deltaY: 10,
    });
    compactScroller.dispatchEvent(horizontalWheel);
    expect(messageList.scrollTop).toBe(210);
    expect(horizontalWheel.defaultPrevented).toBe(false);

    await userEvent.click(within(assistantBubble).getByRole("button", { name: "Raw" }));

    const inputLabel = within(assistantBubble).getByText("Input");
    const inputScroller = inputLabel
      .closest(".min-w-0")
      ?.querySelector(".tool-call-scroll") as HTMLElement | null;
    const outputLabel = within(assistantBubble).getByText("Output");
    const outputScroller = outputLabel
      .closest(".min-w-0")
      ?.querySelector(".tool-call-scroll") as HTMLElement | null;
    if (!inputScroller || !outputScroller) {
      throw new Error("Expected raw Input and Output tool-call scrollers");
    }

    for (const scroller of [inputScroller, outputScroller]) {
      Object.defineProperties(scroller, {
        clientHeight: { configurable: true, value: 100 },
        scrollHeight: { configurable: true, value: 360 },
      });
    }

    messageList.scrollTop = 210;
    inputScroller.scrollTop = 50;
    const rawMidWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: 40,
    });
    inputScroller.dispatchEvent(rawMidWheel);
    expect(messageList.scrollTop).toBe(210);
    expect(rawMidWheel.defaultPrevented).toBe(false);

    inputScroller.scrollTop = 260;
    const rawInputBottomWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: 40,
    });
    inputScroller.dispatchEvent(rawInputBottomWheel);
    expect(messageList.scrollTop).toBe(250);
    expect(rawInputBottomWheel.defaultPrevented).toBe(true);

    outputScroller.scrollTop = 0;
    const rawOutputTopWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: -20,
    });
    outputScroller.dispatchEvent(rawOutputTopWheel);
    expect(messageList.scrollTop).toBe(230);
    expect(rawOutputTopWheel.defaultPrevented).toBe(true);
  });

  it("localizes the tool call raw toggle label", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      general: {
        ...appTestState.settingsResponse.general,
        language: "zh-CN",
      },
    };
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    const assistantBubble = (await screen.findByLabelText("编辑 (edit_file)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    const rawButton = within(assistantBubble).getByRole("button", { name: "原始" });
    expect(rawButton).toHaveTextContent("原始");
    expect(rawButton).toHaveClass("h-5", "min-h-0", "py-0", "leading-4");

    await userEvent.click(rawButton);

    const compactButton = within(assistantBubble).getByRole("button", { name: "精简" });
    expect(compactButton).toHaveTextContent(
      "精简",
    );
    expect(compactButton).toHaveClass("h-5", "min-h-0", "py-0", "leading-4");
  });

  it("opens API statistics filtered to the assistant reply request", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    const assistantBubble = (await screen.findByLabelText("Edit (edit_file)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    await userEvent.click(
      within(assistantBubble).getByRole("button", {
        name: "View API requests for this reply",
      }),
    );

    await waitFor(() => {
      expect(window.location.pathname).toBe("/stats");
      const params = new URLSearchParams(window.location.search);
      expect(params.get("requestId")).toBe("request-1,request-2");
    });
    await waitFor(() => {
      const statsCall = aiStatisticsCallUrlsFromMock(fetchMock).find(
        (url) => url.searchParams.get("requestId") === "request-1,request-2",
      );
      expect(statsCall?.searchParams.get("workspaceId")).toBe("workspace-1");
      expect(statsCall?.searchParams.get("chatId")).toBe("chat-1");
    });
  });

  it("hides assistant reply metrics while an active run message is streaming", async () => {
    const activeRun = {
      acceptingGuidance: false,
      chatId: "chat-1",
      lastSequence: 4,
      runId: "active-run-1",
      workspaceId: "workspace-1",
    };
    const streamingMessages = {
      ...chatMessages,
      messages: chatMessages.messages.map((message) =>
        message.id === "message-assistant"
          ? { ...message, status: "streaming" }
          : message,
      ),
    };
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...streamingMessages, activeRun });
      }
      if (path === "/api/workspaces/workspace-1/chat/runs/active-run-1/stream") {
        return new Response(new ReadableStream<Uint8Array>({ start() {} }), {
          headers: { "Content-Type": "text/event-stream" },
          status: 200,
        });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    const assistantBubble = (await screen.findByLabelText("Edit (edit_file)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(([input]) =>
          String(input).includes("/api/workspaces/workspace-1/chat/runs/active-run-1/stream"),
        ),
      ).toBe(true);
    });
    expect(assistantBubble.querySelector(".message-model-id")).toHaveTextContent(
      "gpt-test",
    );
    expect(within(assistantBubble).queryByText("Model: gpt-test")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Total time: 2 sec")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("tokens/s: 20")).not.toBeInTheDocument();
    expect(screen.getByText("Need file context. Then answer.")).toBeInTheDocument();
    expect(within(assistantBubble).getByText("Memories used")).toBeInTheDocument();
  });

  it("renders a plan mode badge only on plan user messages", async () => {
    const planModeMessages = {
      ...chatMessages,
      messages: [
        {
          ...chatMessages.messages[0],
          content: "Use plan mode for this request.",
          id: "message-user-plan",
          parts: [{ text: "Use plan mode for this request.", type: "text" }],
          sessionMode: "plan",
        },
        {
          ...chatMessages.messages[0],
          content: "Use normal mode for this request.",
          createdAt: "2026-06-10T08:00:01.000Z",
          id: "message-user-normal",
          parts: [{ text: "Use normal mode for this request.", type: "text" }],
        },
      ],
    };
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...planModeMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    const planBubble = (await screen.findByText("Use plan mode for this request."))
      .closest(".message-bubble") as HTMLElement | null;
    const normalBubble = screen
      .getByText("Use normal mode for this request.")
      .closest(".message-bubble") as HTMLElement | null;

    if (!planBubble || !normalBubble) {
      throw new Error("Expected user message bubbles");
    }
    expect(within(planBubble).getByText("Plan mode")).toHaveClass("message-run-badge");
    expect(within(normalBubble).queryByText("Plan mode")).not.toBeInTheDocument();
  });

  it("restores Plan mode from the last user message after URL refresh", async () => {
    const planChatMessages = {
      ...chatMessages,
      messages: [
        {
          ...chatMessages.messages[0],
          content: "Plan after refresh.",
          id: "message-user-plan-refresh",
          parts: [{ text: "Plan after refresh.", type: "text" }],
          sessionMode: "plan",
        },
        chatMessages.messages[1],
      ],
    };
    const normalChatMessages = {
      ...secondChatMessages,
      messages: [
        {
          ...secondChatMessages.messages[0],
          content: "Normal after refresh.",
          id: "message-user-normal-refresh",
          parts: [{ text: "Normal after refresh.", type: "text" }],
          sessionMode: null,
        },
        secondChatMessages.messages[1],
      ],
    };
    // chat-1 override is honored by the harness; chat-2 hardcodes secondChatMessages
    // unless fetch is stubbed, so both keys use an explicit messages stub.
    appTestState.chatMessagesResponsesByChatKey = {
      "workspace-1/chat-1": planChatMessages,
    };
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
          return jsonResponse({ ...planChatMessages, activeRun: null });
        }
        if (path === "/api/workspaces/workspace-1/chats/chat-2/messages") {
          return jsonResponse({ ...normalChatMessages, activeRun: null });
        }
        return mockFetch(input, init);
      }),
    );

    // Fresh App mount with URL pointing at the plan chat (browser refresh path).
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    const { unmount } = renderApp();

    expect(await screen.findByText("Plan after refresh.")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Plan mode" })).toHaveAttribute(
        "aria-pressed",
        "true",
      );
    });
    unmount();

    // Re-mount with URL on the normal chat; Plan toggle stays off after messages load.
    window.history.replaceState(null, "", "/workspace-1/chat-2");
    renderApp();

    expect(await screen.findByText("Normal after refresh.")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Plan mode" })).toHaveAttribute(
        "aria-pressed",
        "false",
      );
    });
  });

  it("formats reply total duration human-readably", async () => {
    const messagesWithLongReplyDuration = {
      ...chatMessages,
      messages: chatMessages.messages.map((message) =>
        message.id === "message-assistant"
          ? {
              ...message,
              metrics: message.metrics
                ? { ...message.metrics, totalLatencyMs: 72_000 }
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
        return jsonResponse({ ...messagesWithLongReplyDuration, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    expect(await screen.findByText("Total time: 1 min 12 sec")).toBeInTheDocument();
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
    expect(screen.getByText("Total time: 500 ms")).toBeInTheDocument();
  });

  it("treats a transport close after completion as a finished chat stream", async () => {
    renderApp();

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "finish before remote close",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        assistantMessageId: "message-assistant-stream",
        chatId: "chat-1",
        memoriesUsed: [],
        text: "Remote answer made it through.",
        type: "complete",
        metrics: {
          firstTokenLatencyMs: 100,
          modelId: "gpt-test",
          outputTokens: 5,
          providerId: "openai",
          totalLatencyMs: 500,
        },
        reasoning: null,
        stopReason: null,
        usage: null,
      });
    });

    expect(await screen.findByText("Remote answer made it through.")).toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.error(new TypeError("network error"));
    });

    await waitFor(() => expect(screen.queryByText("network error")).not.toBeInTheDocument());
    expect(screen.getByText("Model: gpt-test")).toBeInTheDocument();
    expect(screen.getByText("Total time: 500 ms")).toBeInTheDocument();
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

  it("keeps failed edit_file errors visible in compact mode", async () => {
    const failedChatMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = failedChatMessages.messages[1];
    const failedToolCall = {
      ...assistantMessage.toolCalls[0],
      isError: true,
      output: { error: "oldStr not found" },
    };
    assistantMessage.toolCalls = [failedToolCall];
    assistantMessage.parts = assistantMessage.parts.map((part: { type: string }) =>
      part.type === "toolCall" ? { ...part, toolCall: failedToolCall } : part,
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...failedChatMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const assistantBubble = (await screen.findByLabelText("Edit (edit_file)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    expect(within(assistantBubble).getByText("oldStr not found")).toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();
    expect(assistantBubble.querySelector(".edit-file-diff-line")).toBeNull();

    await userEvent.click(within(assistantBubble).getByRole("button", { name: "Raw" }));

    expect(within(assistantBubble).getByText("Input")).toBeInTheDocument();
    expect(within(assistantBubble).getByText("Output")).toBeInTheDocument();
    expect(
      within(assistantBubble).getByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"oldStr": "hello"')),
      ),
    ).toBeInTheDocument();
  });

  it("keeps failed apply_patch errors visible without a compact diff", async () => {
    const failedPatchChatMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = failedPatchChatMessages.messages[1];
    const failedToolCall = {
      ...assistantMessage.toolCalls[0],
      input: {
        patch: "*** Begin Patch\n*** Update File: README.md\n@@\n-old\n+new\n*** End Patch",
      },
      isError: true,
      name: "apply_patch",
      output: {
        error: "patch did not apply",
        linesAdded: 99,
        linesRemoved: 88,
      },
    };
    assistantMessage.toolCalls = [failedToolCall];
    assistantMessage.parts = assistantMessage.parts.map((part: { type: string }) =>
      part.type === "toolCall" ? { ...part, toolCall: failedToolCall } : part,
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...failedPatchChatMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const assistantBubble = (await screen.findByLabelText("Patch (apply_patch)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    expect(within(assistantBubble).getByText("patch did not apply")).toBeInTheDocument();
    expect(within(assistantBubble).queryByText("+99")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("-88")).not.toBeInTheDocument();
    expect(assistantBubble.querySelector(".edit-file-diff-line")).toBeNull();
    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();
  });

  it("falls back to the compact summary for malformed successful apply_patch input", async () => {
    const malformedPatchChatMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = malformedPatchChatMessages.messages[1];
    const malformedToolCall = {
      ...assistantMessage.toolCalls[0],
      input: { patch: "not a patch\n+unexpected" },
      name: "apply_patch",
      output: "Patch summary",
    };
    assistantMessage.toolCalls = [malformedToolCall];
    assistantMessage.parts = assistantMessage.parts.map((part: { type: string }) =>
      part.type === "toolCall" ? { ...part, toolCall: malformedToolCall } : part,
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...malformedPatchChatMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const assistantBubble = (await screen.findByLabelText("Patch (apply_patch)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    expect(within(assistantBubble).getByText("Patch summary")).toBeInTheDocument();
    expect(within(assistantBubble).queryByText("+1")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("-1")).not.toBeInTheDocument();
    expect(assistantBubble.querySelector(".edit-file-diff-line")).toBeNull();
  });

  it("does not show change stats for legacy apply_patch object output", async () => {
    const legacyPatchChatMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = legacyPatchChatMessages.messages[1];
    const legacyToolCall = {
      ...assistantMessage.toolCalls[0],
      input: { patch: "not a patch\n+unexpected" },
      name: "apply_patch",
      output: { patchedFiles: 4 },
    };
    assistantMessage.toolCalls = [legacyToolCall];
    assistantMessage.parts = assistantMessage.parts.map((part: { type: string }) =>
      part.type === "toolCall" ? { ...part, toolCall: legacyToolCall } : part,
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...legacyPatchChatMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const assistantBubble = (await screen.findByLabelText("Patch (apply_patch)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    expect(within(assistantBubble).queryByText("+4")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("-4")).not.toBeInTheDocument();
    expect(assistantBubble.querySelector(".edit-file-diff-line")).toBeNull();
  });

  it("keeps managed command errors out of the success summary", async () => {
    const failedCommandMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = failedCommandMessages.messages[1];
    const failedToolCall = {
      ...assistantMessage.toolCalls[0],
      input: { background: true, command: "server" },
      isError: true,
      name: "run_command",
      output: { error: "managed command was not found" },
    };
    assistantMessage.toolCalls = [failedToolCall];
    assistantMessage.parts = assistantMessage.parts.map((part: { type: string }) =>
      part.type === "toolCall" ? { ...part, toolCall: failedToolCall } : part,
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...failedCommandMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    const summary = await screen.findByLabelText("Run (run_command)");
    await userEvent.click(summary);

    const assistantBubble = summary.closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    expect(within(assistantBubble).getByText("managed command was not found")).toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Background process started, no output yet")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Entire process tree terminated")).not.toBeInTheDocument();
  });

  it("shows successful read_file content without the JSON view", async () => {
    const readFileChatMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = readFileChatMessages.messages[1];
    const readFileToolCall = {
      ...assistantMessage.toolCalls[0],
      input: { path: "README.md" },
      name: "read_file",
      output: { content: "alpha\nbeta", path: "README.md" },
    };
    assistantMessage.toolCalls = [readFileToolCall];
    assistantMessage.parts = assistantMessage.parts.map((part: { type: string }) =>
      part.type === "toolCall" ? { ...part, toolCall: readFileToolCall } : part,
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...readFileChatMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const assistantBubble = (await screen.findByLabelText("Read (read_file)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    expect(
      within(assistantBubble).getByText((_content, element) =>
        element?.tagName === "PRE" && element.textContent === "alpha\nbeta",
      ),
    ).toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();
    expect(
      within(assistantBubble).queryByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"content": "alpha')),
      ),
    ).not.toBeInTheDocument();
  });

  it("renders successful read_spec contentMarkdown as markdown in compact mode", async () => {
    const readSpecChatMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = readSpecChatMessages.messages[1];
    const readSpecToolCall = {
      ...assistantMessage.toolCalls[0],
      input: { injectEnabled: true },
      name: "read_spec",
      output: {
        contentMarkdown: "# Current Spec\n\nA **bold** project note.",
        enabled: true,
        injectEnabled: true,
        revision: 4,
      },
    };
    assistantMessage.toolCalls = [readSpecToolCall];
    assistantMessage.parts = assistantMessage.parts.map((part: { type: string }) =>
      part.type === "toolCall" ? { ...part, toolCall: readSpecToolCall } : part,
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...readSpecChatMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const assistantBubble = (await screen.findByLabelText("Read Spec (read_spec)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    expect(
      await within(assistantBubble).findByRole("heading", { name: "Current Spec" }),
    ).toBeInTheDocument();
    expect(within(assistantBubble).getByText("bold").tagName).toBe("STRONG");
    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();
    expect(
      within(assistantBubble).queryByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"contentMarkdown"')),
      ),
    ).not.toBeInTheDocument();

    await userEvent.click(within(assistantBubble).getByRole("button", { name: "Raw" }));

    expect(within(assistantBubble).getByText("Input")).toBeInTheDocument();
    expect(within(assistantBubble).getByText("Output")).toBeInTheDocument();
    expect(
      within(assistantBubble).getByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"contentMarkdown": "# Current Spec')),
      ),
    ).toBeInTheDocument();
  });

  it("renders successful update_spec edits as a compact diff before output markdown", async () => {
    const updateSpecChatMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = updateSpecChatMessages.messages[1];
    const updateSpecToolCall = {
      ...assistantMessage.toolCalls[0],
      input: {
        contentMarkdown: null,
        edits: [
          { oldText: "## Purpose\nOld purpose", newText: "## Purpose\nNew purpose" },
          { oldText: "Legacy flag", newText: "Modern flag" },
        ],
        expectedRevision: 3,
      },
      name: "update_spec",
      output: {
        contentMarkdown: "# Complete Patched Spec\n\nThis full output must stay hidden in compact mode.",
        revision: 4,
        updateMode: "patch",
      },
    };
    assistantMessage.toolCalls = [updateSpecToolCall];
    assistantMessage.parts = assistantMessage.parts.map((part: { type: string }) =>
      part.type === "toolCall" ? { ...part, toolCall: updateSpecToolCall } : part,
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...updateSpecChatMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const assistantBubble = (await screen.findByLabelText("Update Spec (update_spec)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    const diffLines = Array.from(
      assistantBubble.querySelectorAll<HTMLElement>(".edit-file-diff-line"),
    );
    expect(diffLines.map((line) => line.textContent)).toEqual([
      "-## Purpose",
      "-Old purpose",
      "+## Purpose",
      "+New purpose",
      "-Legacy flag",
      "+Modern flag",
    ]);
    expect(diffLines[0]).toHaveClass("bg-[var(--danger-soft)]", "text-[var(--danger)]");
    expect(diffLines[2]).toHaveClass("bg-[var(--success-soft)]", "text-[var(--success)]");
    expect(diffLines[4]).toHaveClass("bg-[var(--danger-soft)]", "text-[var(--danger)]");
    expect(diffLines[5]).toHaveClass("bg-[var(--success-soft)]", "text-[var(--success)]");
    expect(within(assistantBubble).queryByRole("heading", { name: "Complete Patched Spec" })).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();
    expect(
      within(assistantBubble).queryByText((_content, element) =>
        element?.tagName === "PRE" && Boolean(element.textContent?.includes('"contentMarkdown"')),
      ),
    ).not.toBeInTheDocument();

    await userEvent.click(within(assistantBubble).getByRole("button", { name: "Raw" }));

    expect(within(assistantBubble).getByText("Input")).toBeInTheDocument();
    expect(within(assistantBubble).getByText("Output")).toBeInTheDocument();
    expect(
      within(assistantBubble).getByText((_content, element) =>
        element?.tagName === "PRE" && Boolean(element.textContent?.includes('"edits": [')),
      ),
    ).toBeInTheDocument();
    expect(
      within(assistantBubble).getByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"contentMarkdown": "# Complete Patched Spec')),
      ),
    ).toBeInTheDocument();

    await userEvent.click(within(assistantBubble).getByRole("button", { name: "Compact" }));

    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();
    expect(
      Array.from(assistantBubble.querySelectorAll<HTMLElement>(".edit-file-diff-line")).map(
        (line) => line.textContent,
      ),
    ).toEqual([
      "-## Purpose",
      "-Old purpose",
      "+## Purpose",
      "+New purpose",
      "-Legacy flag",
      "+Modern flag",
    ]);
  });

  it("renders successful apply_patch calls as compact multi-file diffs", async () => {
    const applyPatchChatMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = applyPatchChatMessages.messages[1];
    const applyPatchToolCall = {
      ...assistantMessage.toolCalls[0],
      input: {
        patch: [
          "*** Begin Patch",
          "*** Update File: README.md",
          "@@",
          "-old readme",
          "+new readme",
          "*** Add File: docs/new.md",
          "+# New document",
          "+",
          "*** Delete File: legacy.md",
          "*** Update File: moved.md",
          "*** Move to: docs/moved.md",
          "@@",
          "-old location",
          "+new location",
          "*** End of File",
          "*** End Patch",
        ].join("\n"),
      },
      name: "apply_patch",
      output: { linesAdded: 4, lines_removed: 5, patchedFiles: 4 },
    };
    assistantMessage.toolCalls = [applyPatchToolCall];
    assistantMessage.parts = assistantMessage.parts.map((part: { type: string }) =>
      part.type === "toolCall" ? { ...part, toolCall: applyPatchToolCall } : part,
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...applyPatchChatMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const assistantBubble = (await screen.findByLabelText("Patch (apply_patch)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    const expectedDiffLines = [
      "-old readme",
      "+new readme",
      "+# New document",
      "+",
      "-old location",
      "+new location",
    ];
    const diffLines = Array.from(
      assistantBubble.querySelectorAll<HTMLElement>(".edit-file-diff-line"),
    );
    expect(within(assistantBubble).getByText("+4")).toHaveClass(
      "text-[var(--success)]",
    );
    expect(within(assistantBubble).getByText("-5")).toHaveClass(
      "text-[var(--danger)]",
    );
    expect(diffLines.map((line) => line.textContent)).toEqual(expectedDiffLines);
    expect(
      diffLines.some((line) => /\*\*\*|@@|Move to/.test(line.textContent ?? "")),
    ).toBe(false);
    expect(diffLines[0]).toHaveClass("bg-[var(--danger-soft)]", "text-[var(--danger)]");
    expect(diffLines[1]).toHaveClass("bg-[var(--success-soft)]", "text-[var(--success)]");
    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();
    expect(
      within(assistantBubble).queryByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"patch": "*** Begin Patch')),
      ),
    ).not.toBeInTheDocument();

    await userEvent.click(within(assistantBubble).getByRole("button", { name: "Raw" }));

    expect(within(assistantBubble).getByText("Input")).toBeInTheDocument();
    expect(within(assistantBubble).getByText("Output")).toBeInTheDocument();
    expect(
      within(assistantBubble).getByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"patch": "*** Begin Patch')),
      ),
    ).toBeInTheDocument();
    expect(
      within(assistantBubble).getByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"patchedFiles": 4')),
      ),
    ).toBeInTheDocument();

    await userEvent.click(within(assistantBubble).getByRole("button", { name: "Compact" }));

    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();
    expect(
      Array.from(assistantBubble.querySelectorAll<HTMLElement>(".edit-file-diff-line")).map(
        (line) => line.textContent,
      ),
    ).toEqual(expectedDiffLines);
  });

  it("renders successful update_spec contentMarkdown as markdown in compact mode", async () => {
    const updateSpecChatMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = updateSpecChatMessages.messages[1];
    const updateSpecToolCall = {
      ...assistantMessage.toolCalls[0],
      input: { contentMarkdown: "# Old Spec", edits: null, expectedRevision: 3 },
      name: "update_spec",
      output: {
        contentMarkdown: "# Updated Spec\n\nA **bold** project note.",
        revision: 4,
        updateMode: "fullReplacement",
      },
    };
    assistantMessage.toolCalls = [updateSpecToolCall];
    assistantMessage.parts = assistantMessage.parts.map((part: { type: string }) =>
      part.type === "toolCall" ? { ...part, toolCall: updateSpecToolCall } : part,
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...updateSpecChatMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const assistantBubble = (await screen.findByLabelText("Update Spec (update_spec)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    expect(
      await within(assistantBubble).findByRole("heading", { name: "Updated Spec" }),
    ).toBeInTheDocument();
    expect(within(assistantBubble).getByText("bold").tagName).toBe("STRONG");
    expect(assistantBubble.querySelectorAll(".edit-file-diff-line")).toHaveLength(0);
    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();
    expect(
      within(assistantBubble).queryByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"contentMarkdown"')),
      ),
    ).not.toBeInTheDocument();

    await userEvent.click(within(assistantBubble).getByRole("button", { name: "Raw" }));

    expect(within(assistantBubble).getByText("Input")).toBeInTheDocument();
    expect(within(assistantBubble).getByText("Output")).toBeInTheDocument();
    expect(
      within(assistantBubble).getByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"contentMarkdown": "# Updated Spec')),
      ),
    ).toBeInTheDocument();
  });

  it("shows write_file content in compact mode", async () => {
    const writeFileChatMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = writeFileChatMessages.messages[1];
    const writeFileToolCall = {
      ...assistantMessage.toolCalls[0],
      input: { content: "one\ntwo", path: "notes.txt" },
      name: "write_file",
      output: { bytes: 7, linesAdded: 2, linesRemoved: 0, path: "notes.txt" },
    };
    assistantMessage.toolCalls = [writeFileToolCall];
    assistantMessage.parts = assistantMessage.parts.map((part: { type: string }) =>
      part.type === "toolCall" ? { ...part, toolCall: writeFileToolCall } : part,
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...writeFileChatMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const assistantBubble = (await screen.findByLabelText("Write (write_file)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    expect(
      within(assistantBubble).getByText((_content, element) =>
        element?.tagName === "PRE" && element.textContent === "one\ntwo",
      ),
    ).toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Input")).not.toBeInTheDocument();
    expect(within(assistantBubble).queryByText("Output")).not.toBeInTheDocument();

    await userEvent.click(within(assistantBubble).getByRole("button", { name: "Raw" }));

    expect(within(assistantBubble).getByText("Input")).toBeInTheDocument();
    expect(within(assistantBubble).getByText("Output")).toBeInTheDocument();
    expect(
      within(assistantBubble).getByText((_content, element) =>
        element?.tagName === "PRE" &&
        Boolean(element.textContent?.includes('"content": "one')),
      ),
    ).toBeInTheDocument();
  });

  it("uses specific tool icons and keeps unknown tools on the wrench fallback", async () => {
    const iconChatMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = iconChatMessages.messages[1];
    const runCommandToolCall = {
      ...assistantMessage.toolCalls[0],
      id: "tool-run-command",
      input: { args: ["status"], command: "git" },
      name: "run_command",
      output: { status: 0, stdout: "clean", stderr: "" },
    };
    const unknownToolCall = {
      ...assistantMessage.toolCalls[0],
      id: "tool-unknown",
      input: { query: "mystery" },
      name: "mystery_tool",
      output: { message: "unknown result" },
    };
    assistantMessage.toolCalls = [runCommandToolCall, unknownToolCall];
    assistantMessage.parts = assistantMessage.parts.flatMap((part: { type: string }) =>
      part.type === "toolCall"
        ? [
            { ...part, toolCall: runCommandToolCall },
            { ...part, toolCall: unknownToolCall },
          ]
        : [part],
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...iconChatMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));

    const runCommandSummary = (await screen.findByLabelText("Run (run_command)")).closest("summary");
    const unknownSummary = (await screen.findByLabelText("mystery_tool (mystery_tool)")).closest("summary");

    expect(runCommandSummary?.querySelector("svg.lucide-terminal")).toBeInTheDocument();
    expect(unknownSummary?.querySelector("svg.lucide-wrench")).toBeInTheDocument();
  });

  it("renders managed command lifecycle, incremental chunks, and raw protocol fields", async () => {
    const managedCommandMessages = JSON.parse(JSON.stringify(chatMessages));
    const assistantMessage = managedCommandMessages.messages[1];
    const processStartedAt = Date.parse("2026-07-22T04:00:00.000Z");
    const toolCompletedAt = "2026-07-22T04:00:02.000Z";
    const backgroundStart = {
      ...assistantMessage.toolCalls[0],
      id: "tool-background-start",
      completedAt: toolCompletedAt,
      input: {
        args: ["--watch"],
        background: true,
        backgroundTimeoutMs: null,
        command: "server",
        cwd: ".",
        timeoutMs: null,
      },
      name: "run_command",
      output: {
        chunks: [],
        nextCursor: 0,
        pid: 4242,
        processId: "process-demo",
        startedAt: processStartedAt,
        status: "running",
      },
      status: "completed",
    };
    const outputPoll = {
      ...assistantMessage.toolCalls[0],
      id: "tool-command-output",
      completedAt: toolCompletedAt,
      input: { cursor: 4, processId: "process-demo", timeoutMs: 10_000, waitMs: 100 },
      name: "get_command_output",
      output: {
        chunks: [
          { cursor: 20, stream: "stdout", text: "ready\n" },
          { cursor: 21, stream: "stderr", text: "warning\n" },
        ],
        availableFromCursor: 20,
        cursorExpired: true,
        fromCursor: 4,
        hasMore: true,
        nextCursor: 21,
        pid: 4242,
        processId: "process-demo",
        startedAt: processStartedAt,
        status: "running",
      },
      status: "completed",
    };
    const exitedOutputPoll = {
      ...outputPoll,
      id: "tool-command-output-exited",
      output: {
        ...outputPoll.output,
        chunks: [],
        cursorExpired: false,
        endedAt: "2026-07-22T04:00:03.000Z",
        exitCode: 17,
        hasMore: false,
        status: "exited",
      },
    };
    const stoppedCommand = {
      ...assistantMessage.toolCalls[0],
      id: "tool-stop-command",
      completedAt: toolCompletedAt,
      input: { processId: "process-demo", timeoutMs: 10_000 },
      name: "stop_command",
      output: {
        exitCode: null,
        pid: 4242,
        processId: "process-demo",
        status: "stopped",
        terminationReason: "explicit_stop",
      },
      status: "completed",
    };
    const stopRequestedCommand = {
      ...stoppedCommand,
      id: "tool-stop-requested",
      output: {
        ...stoppedCommand.output,
        status: "running",
      },
    };
    assistantMessage.toolCalls = [
      backgroundStart,
      outputPoll,
      exitedOutputPoll,
      stoppedCommand,
      stopRequestedCommand,
    ];
    assistantMessage.parts = assistantMessage.parts.flatMap((part: { type: string }) =>
      part.type === "toolCall"
        ? [
            { ...part, toolCall: backgroundStart },
            { ...part, toolCall: outputPoll },
            { ...part, toolCall: exitedOutputPoll },
            { ...part, toolCall: stoppedCommand },
            { ...part, toolCall: stopRequestedCommand },
          ]
        : [part],
    );
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return jsonResponse({ ...managedCommandMessages, activeRun: null });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    const startSummary = await screen.findByLabelText("Run (run_command)");
    const outputSummaries = await screen.findAllByLabelText(
      "Command Output (get_command_output)",
    );
    const [outputSummary, exitedOutputSummary] = outputSummaries;
    const stopSummaries = await screen.findAllByLabelText("Stop Command (stop_command)");
    const [stopSummary, stopRequestedSummary] = stopSummaries;
    if (
      !outputSummary ||
      !exitedOutputSummary ||
      !stopSummary ||
      !stopRequestedSummary
    ) {
      throw new Error("Expected managed command lifecycle cards");
    }

    const startStatus = within(startSummary).getByText("Backgrounded");
    expect(startStatus).toHaveClass("bg-[var(--success-soft)]", "text-[var(--success)]");
    expect(outputSummary.lastElementChild).toHaveTextContent("completed");
    expect(outputSummary).toHaveTextContent(/process-demo.*Background running/);
    expect(exitedOutputSummary.lastElementChild).toHaveTextContent("completed");
    expect(exitedOutputSummary).toHaveTextContent(
      /process-demo.*Exited · code 17/,
    );
    expect(stopSummary).toHaveTextContent("Stopped");

    await userEvent.click(startSummary);
    expect(await screen.findByText("Background process started, no output yet")).toBeInTheDocument();
    const startBubble = startSummary.closest(".tool-call-block") as HTMLElement | null;
    if (!startBubble) {
      throw new Error("Expected background start tool block");
    }
    expect(within(startBubble).getAllByText("2.0s")).not.toHaveLength(0);

    await userEvent.click(outputSummary);
    expect(await screen.findByText("Earlier output was removed from the retained buffer.")).toBeInTheDocument();
    expect(await screen.findByText("cursor 20–21")).toBeInTheDocument();
    expect(await screen.findByText("ready")).toBeInTheDocument();
    expect(await screen.findByText("warning")).toBeInTheDocument();
    expect(await screen.findByText(/More output is available; continue with nextCursor 21/)).toBeInTheDocument();
    const outputBubble = outputSummary.closest(".tool-call-block") as HTMLElement | null;
    if (!outputBubble) {
      throw new Error("Expected managed command tool block");
    }
    expect(within(outputBubble).getAllByText("2.0s")).not.toHaveLength(0);

    await userEvent.click(stopSummary);
    expect(await screen.findByText("Entire process tree terminated")).toBeInTheDocument();

    await userEvent.click(stopRequestedSummary);
    expect(await screen.findByText("Process tree termination requested")).toBeInTheDocument();

    await userEvent.click(within(outputBubble).getByRole("button", { name: "Raw" }));
    expect(within(outputBubble).getByText("Input")).toBeInTheDocument();
    expect(
      within(outputBubble).getByText((_content, element) =>
        element?.tagName === "PRE" && Boolean(element.textContent?.includes('"cursorExpired": true')),
      ),
    ).toBeInTheDocument();

    await userEvent.click(within(outputBubble).getByRole("button", { name: "Compact" }));
    expect(within(outputBubble).getAllByText("Background running")).not.toHaveLength(0);
    expect(within(outputBubble).getByText("cursor 20–21")).toBeInTheDocument();
    expect(within(outputBubble).getAllByText("2.0s")).not.toHaveLength(0);
    expect(outputBubble.querySelector(".tool-call-scroll")).not.toBeNull();
  });

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
    const assistantBubble = (await screen.findByLabelText("编辑 (edit_file)"))
      .closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }

    const completedPill = within(assistantBubble).getByText("已完成");
    expect(completedPill).toHaveClass("bg-[var(--success-soft)]", "text-[var(--success)]");
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

  it("shows the nav update button when an update is available", async () => {
    const fetchMock = vi.mocked(fetch);
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      update: {
        ...appTestState.settingsResponse.update,
        assetDownloadUrl: "https://github.com/fonlan/foco/releases/download/v0.2.0/Foco-v0.2.0-macos-arm64.dmg",
        assetName: "Foco-v0.2.0-macos-arm64.dmg",
        error: null,
        releaseUrl: "https://github.com/fonlan/foco/releases/tag/v0.2.0",
        targetVersion: "0.2.0",
        updateAvailable: true,
      },
    };
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Install update" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/update/install",
        expect.objectContaining({ method: "POST" }),
      );
    });
    expect(
      await screen.findByText("Foco is installing the update and will restart shortly."),
    ).toBeInTheDocument();
  });

  it("opens scheduled tasks from the nav and URL", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(
      await screen.findByRole("button", { name: "Scheduled tasks" }),
    );

    expect(
      await screen.findByRole("heading", { name: "Scheduled tasks" }),
    ).toBeInTheDocument();
    expect(await screen.findAllByText("Daily workspace summary")).not.toHaveLength(0);
    expect(window.location.pathname).toBe("/scheduled");
    expect(
      fetchMock.mock.calls.some(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/scheduled-tasks?page=1&pageSize=25",
      ),
    ).toBe(true);

    await userEvent.click(screen.getByRole("button", { name: "Foco" }));

    expect(window.location.pathname).toBe("/");
    expect(window.location.search).toBe("");

    window.history.pushState(null, "", "/");
    fireEvent.popState(window);
    window.history.pushState(null, "", "/scheduled");
    fireEvent.popState(window);

    expect(
      await screen.findByRole("heading", { name: "Scheduled tasks" }),
    ).toBeInTheDocument();
  });

  it("creates and runs a scheduled task from the scheduled tasks page", async () => {
    renderApp();

    await userEvent.click(
      await screen.findByRole("button", { name: "Scheduled tasks" }),
    );

    await userEvent.click(await screen.findByRole("button", { name: "Duplicate task" }));
    expect(await screen.findAllByText("Daily workspace summary copy")).not.toHaveLength(0);

    await userEvent.click(await screen.findByRole("button", { name: "New task" }));
    expect(await screen.findByText("Next five runs")).toBeInTheDocument();

    const statusSelect = screen.getByLabelText("Status");
    await userEvent.click(statusSelect);
    expect(screen.getByRole("option", { name: "Enabled" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Paused" })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "Completed" })).not.toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "Archived" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("option", { name: "Enabled" }));
    const agentSelect = screen.getByLabelText("Agent");
    await userEvent.click(agentSelect);
    expect(screen.getByRole("option", { name: "Coordinator" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await userEvent.click(screen.getByRole("option", { name: "Coordinator" }));
    const modelSelect = screen.getByLabelText("Model");
    await userEvent.click(modelSelect);
    expect(screen.getByRole("option", { name: "GPT Test" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await userEvent.click(screen.getByRole("option", { name: "GPT Test" }));
    // Provider is chosen by global model routing, not the scheduled-task form.
    expect(screen.queryByLabelText("Provider")).toBeNull();
    expect(screen.getByRole("checkbox", { name: "Enable Team mode" })).toBeChecked();

    const unitSelect = screen.getByLabelText("Unit");
    await userEvent.click(unitSelect);
    expect(screen.getByRole("option", { name: "Weeks" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Months" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("option", { name: "Months" }));
    const concurrencySelect = screen.getByLabelText("Concurrency");
    await userEvent.click(concurrencySelect);
    await userEvent.click(screen.getByRole("option", { name: "Force run" }));
    await userEvent.type(screen.getByLabelText("Title"), "Morning report");
    await userEvent.type(
      screen.getByLabelText("Prompt"),
      "Summarize open work.",
    );
    await userEvent.click(screen.getByRole("button", { name: "Save task" }));

    expect(await screen.findAllByText("Morning report")).not.toHaveLength(0);
    const createCall = vi.mocked(fetch).mock.calls.find(
      ([url, init]) =>
        url === "/api/workspaces/workspace-1/scheduled-tasks" &&
        init?.method === "POST",
    );
    const createBody = JSON.parse(String(createCall?.[1]?.body ?? "{}"));
    expect(createBody.schedule).toMatchObject({
      every_seconds: 2592000,
      type: "interval",
    });
    expect(createBody.status).toBe("enabled");
    expect(createBody.concurrencyPolicy).toBe("force_run");
    expect(createBody.action).toMatchObject({
      agent_definition_id: "agent-definition-coordinator",
      collaboration_tools_enabled: true,
      model_id: "gpt-test",
    });
    // Provider is resolved from the model route when the task runs.
    expect(createBody.action.provider_id).toBeUndefined();

    await userEvent.click(screen.getByRole("button", { name: "Pause task" }));
    expect(await screen.findAllByText("Paused")).not.toHaveLength(0);

    await userEvent.click(screen.getByRole("button", { name: "Run task now" }));
    expect(await screen.findByText("Manual")).toBeInTheDocument();

    await userEvent.click(screen.getAllByRole("button", { name: "Open chat" })[0]!);
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(currentChatTabs()).toEqual(["workspace-1/chat-1"]);
  });

  it("opens a chat from the URL and writes chat selection changes back to the URL", async () => {
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(currentChatTabs()).toEqual(["workspace-1/chat-1"]);

    await userEvent.click(screen.getByText("Second chat"));

    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    expect(currentChatTabs()).toEqual(["workspace-1/chat-1", "workspace-1/chat-2"]);
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
    const originalInnerWidth = window.innerWidth;
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 1440,
    });

    try {
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
    } finally {
      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        value: originalInnerWidth,
      });
    }
  });

  it.each([
    { label: "phone portrait", width: 390, height: 844 },
    { label: "narrow stacked layout", width: 900, height: 844 },
  ])(
    "resizes the context panel height on $label ($width px)",
    async ({ width, height }) => {
      const originalInnerWidth = window.innerWidth;
      const originalInnerHeight = window.innerHeight;

      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        value: width,
      });
      Object.defineProperty(window, "innerHeight", {
        configurable: true,
        value: height,
      });

      try {
        renderApp();
        await screen.findByPlaceholderText(defaultComposerPlaceholder);

        const openButton = screen.queryByRole("button", { name: "Open context panel" });
        if (openButton) {
          await userEvent.click(openButton);
        }

        const splitter = await screen.findByRole("separator", {
          name: "Resize context panel",
        });
        expect(splitter).toHaveAttribute("aria-orientation", "horizontal");
        const appShell = splitter.closest(".app-shell") as HTMLElement | null;
        if (!appShell) {
          throw new Error("Expected context panel splitter inside app shell");
        }

        const widthBefore = appShell.style.getPropertyValue("--diff-panel-width");
        const heightBefore = Number.parseFloat(
          appShell.style.getPropertyValue("--context-panel-mobile-height") || "280",
        );

        fireEvent.pointerDown(splitter, { clientY: 620, pointerId: 1 });

        await waitFor(() => {
          expect(document.body.style.cursor).toBe("row-resize");
          expect(document.body.style.userSelect).toBe("none");
          expect(appShell.style.getPropertyValue("--context-panel-mobile-height")).toBe(
            `${heightBefore}px`,
          );
        });

        fireEvent.pointerMove(window, { clientY: 560 });

        await waitFor(() => {
          expect(appShell.style.getPropertyValue("--context-panel-mobile-height")).toBe(
            `${heightBefore + 60}px`,
          );
        });

        expect(appShell.style.getPropertyValue("--diff-panel-width")).toBe(widthBefore);

        fireEvent.pointerUp(window);

        await waitFor(() => {
          expect(document.body.style.cursor).toBe("");
          expect(document.body.style.userSelect).toBe("");
          expect(appShell.style.getPropertyValue("--context-panel-mobile-height")).toBe(
            `${heightBefore + 60}px`,
          );
        });
      } finally {
        Object.defineProperty(window, "innerWidth", {
          configurable: true,
          value: originalInnerWidth,
        });
        Object.defineProperty(window, "innerHeight", {
          configurable: true,
          value: originalInnerHeight,
        });
      }
    },
  );

  it("resizes the context panel width on desktop without changing stacked height", async () => {
    const originalInnerWidth = window.innerWidth;
    const originalInnerHeight = window.innerHeight;

    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 1440,
    });
    Object.defineProperty(window, "innerHeight", {
      configurable: true,
      value: 900,
    });

    try {
      renderApp();

      const splitter = await screen.findByRole("separator", {
        name: "Resize context panel",
      });
      expect(splitter).toHaveAttribute("aria-orientation", "vertical");
      const appShell = splitter.closest(".app-shell") as HTMLElement | null;
      if (!appShell) {
        throw new Error("Expected context panel splitter inside app shell");
      }

      const heightBefore = appShell.style.getPropertyValue("--context-panel-mobile-height");
      const widthBefore = Number.parseFloat(
        appShell.style.getPropertyValue("--diff-panel-width") || "360",
      );

      fireEvent.pointerDown(splitter, { clientX: 1100, pointerId: 1 });

      await waitFor(() => {
        expect(document.body.style.cursor).toBe("col-resize");
        expect(document.body.style.userSelect).toBe("none");
      });

      fireEvent.pointerMove(window, { clientX: 1080 });

      await waitFor(() => {
        expect(appShell.style.getPropertyValue("--diff-panel-width")).toBe(
          `${widthBefore + 20}px`,
        );
      });

      fireEvent.pointerMove(window, { clientX: 1000 });

      await waitFor(() => {
        expect(appShell.style.getPropertyValue("--diff-panel-width")).toBe(
          `${widthBefore + 100}px`,
        );
      });

      expect(appShell.style.getPropertyValue("--context-panel-mobile-height")).toBe(heightBefore);

      fireEvent.pointerUp(window);

      await waitFor(() => {
        expect(document.body.style.cursor).toBe("");
        expect(document.body.style.userSelect).toBe("");
        expect(appShell.style.getPropertyValue("--diff-panel-width")).toBe(
          `${widthBefore + 100}px`,
        );
      });
    } finally {
      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        value: originalInnerWidth,
      });
      Object.defineProperty(window, "innerHeight", {
        configurable: true,
        value: originalInnerHeight,
      });
    }
  });

  it("Phase 2 mobile shell CSS contract uses HeroUI surfaces, safe-area, and touch targets", () => {
    const stylesCss = readFileSync("styles.css", "utf8");

    expect(stylesCss).toContain("var(--surface)");
    expect(stylesCss).toContain("var(--background)");
    expect(stylesCss).toContain("env(safe-area-inset-top");
    expect(stylesCss).toContain("env(safe-area-inset-bottom");
    expect(stylesCss).toContain("--foco-touch-target");
    expect(stylesCss).toContain("100dvh");
    expect(stylesCss).not.toMatch(/rgba\(255,\s*252,\s*246/);
    expect(stylesCss).not.toMatch(/rgba\(200,\s*101,\s*27/);
    expect(stylesCss).not.toMatch(/rgba\(232,\s*146,\s*60/);

    expect(stylesCss).toMatch(
      /@media \(max-width: 767px\)[\s\S]*?--foco-touch-target/,
    );
    expect(stylesCss).toMatch(
      /@media \(max-width: 430px\)[\s\S]*?max-width:\s*100vw/,
    );
  });

  it("keeps stacked context panel height and horizontal splitter styles at the 1199px breakpoint", () => {
    const stylesCss = readFileSync("styles.css", "utf8");

    expect(stylesCss).toMatch(
      /@media \(max-width: 1199px\)[\s\S]*?--context-panel-mobile-height/,
    );
    expect(stylesCss).toMatch(
      /@media \(max-width: 1199px\)[\s\S]*?\.context-sidebar-splitter\s*\{[\s\S]*?cursor:\s*row-resize/,
    );
    expect(stylesCss).toMatch(
      /@media \(max-width: 1199px\)[\s\S]*?\.context-sidebar-splitter\s*\{[\s\S]*?top:\s*0;/,
    );
    expect(stylesCss).not.toMatch(
      /@media \(max-width: 1199px\)[\s\S]*?\.context-sidebar-splitter\s*\{[\s\S]*?top:\s*-0\.375rem/,
    );
    expect(stylesCss).not.toMatch(
      /@media \(max-width: 1199px\)[\s\S]*?grid-template-rows:\s*minmax\(0,\s*1fr\)\s*minmax\(18rem,\s*36dvh\)/,
    );

    // Horizontal splitter must not be exclusive to the phone media query only.
    const phoneOnlySplitter = stylesCss.match(
      /@media \(max-width: 767px\)\s*\{[^}]*\.context-sidebar-splitter\s*\{[\s\S]*?cursor:\s*row-resize/,
    );
    expect(phoneOnlySplitter).toBeNull();
  });

  it("resizes the message composer from the splitter on desktop and mobile browsers", async () => {
    const originalInnerWidth = window.innerWidth;

    try {
      renderApp();

      await screen.findByPlaceholderText(defaultComposerPlaceholder);

      const splitter = await screen.findByRole("separator", {
        name: "Resize message composer",
      });
      const chatPanel = splitter.closest(".chat-panel") as HTMLElement | null;
      if (!chatPanel) {
        throw new Error("Expected composer splitter inside chat panel");
      }

      vi.spyOn(chatPanel, "getBoundingClientRect").mockReturnValue({
        bottom: 800,
        height: 800,
        left: 0,
        right: 1000,
        toJSON: () => ({}),
        top: 0,
        width: 1000,
        x: 0,
        y: 0,
      } as DOMRect);

      fireEvent.pointerDown(splitter, { clientY: 700, pointerId: 1 });

      await waitFor(() => {
        expect(document.body.style.cursor).toBe("row-resize");
        expect(document.body.style.userSelect).toBe("none");
      });

      fireEvent.pointerMove(window, { clientY: 620 });

      await waitFor(() => {
        expect(chatPanel.style.getPropertyValue("--composer-editor-height")).toBe(
          "148px",
        );
        expect(splitter).toHaveAttribute("aria-valuenow", "148");
      });

      fireEvent.pointerUp(window);

      await waitFor(() => {
        expect(document.body.style.cursor).toBe("");
        expect(document.body.style.userSelect).toBe("");
      });

      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        value: 390,
      });

      fireEvent.pointerDown(splitter, { clientY: 620, pointerId: 2 });
      fireEvent.pointerMove(window, { clientY: 580 });

      await waitFor(() => {
        expect(chatPanel.style.getPropertyValue("--composer-editor-height")).toBe(
          "188px",
        );
        expect(splitter).toHaveAttribute("aria-valuenow", "188");
      });

      fireEvent.pointerUp(window);
    } finally {
      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        value: originalInnerWidth,
      });
    }
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
    renderApp();

    const modelTrigger = await screen.findByRole("button", { name: /Model:/ });
    const thinkingTrigger = await screen.findByRole("button", { name: /Thinking/ });
    expect(screen.queryByLabelText("Git branch")).not.toBeInTheDocument();

    await user.click(modelTrigger);
    expect(await screen.findByRole("listbox")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    });

    await user.click(thinkingTrigger);
    expect(await screen.findByRole("listbox")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    });

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

  it("loads assembled context usage for completed historical chats", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return Promise.resolve(
          jsonResponse({
            ...chatMessages,
            activeRun: null,
            latestResponseUsage: {
              cacheReadTokens: 1200,
              cacheWriteTokens: 300,
              inputTokens: 70000,
              outputTokens: 900,
            },
          }),
        );
      }

      return mockFetch(input, init);
    });

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    expect(
      await screen.findByRole("status", { name: "Context usage 47%" }),
    ).toHaveTextContent("47%");
    const usageCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCall).toBeDefined();
    expect(JSON.parse(String(usageCall?.[1]?.body))).not.toHaveProperty(
      "latestResponseUsage",
    );
    expect(
      fetchMock.mock.calls.some(
        ([url]) => typeof url === "string" && url.includes("/chat/runs/"),
      ),
    ).toBe(false);
  });

  it("loads context usage without stored response usage and ignores composer drafts", async () => {
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
    expect(usageCallsBeforeDraft).toHaveLength(1);

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

    const workspaceToggle = await screen.findByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Default"),
    });
    await userEvent.click(workspaceToggle);
    expect(workspaceToggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("Tool run")).not.toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "New chat in Default" }),
    );

    expect(workspaceToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.queryByRole("button", { name: "New chat" })).not.toBeInTheDocument();
  });

  it("switches the workspace identity panel to the clicked workspace when starting a new chat", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    expect(
      await screen.findByRole("heading", { name: workspace.name }),
    ).toBeInTheDocument();

    await userEvent.click(
      await screen.findByRole("button", { name: "New chat in Side project" }),
    );

    expect(
      await screen.findByRole("heading", { name: "Side project" }),
    ).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(sideProjectComposerPlaceholder),
    ).toBeInTheDocument();
    expect(aiStatisticsCallUrlsFromMock(fetchMock)).toHaveLength(0);
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
    const chatQueueCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/queue",
    );
    const chatStreamCall = fetchMock.mock.calls.find(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/chat/stream",
    );

    expect(JSON.parse(String(chatQueueCall?.[1]?.body))).toEqual(
      expect.objectContaining({
        chatId: null,
        message: "Fresh task",
      }),
    );
    expect(JSON.parse(String(chatStreamCall?.[1]?.body))).toEqual(
      expect.objectContaining({
        chatId: "queued-chat-1",
        message: "Fresh task",
        queuedUserMessageId: "queued-user-1",
      }),
    );

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("keeps the active non-default workspace expanded after sending a new chat", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const defaultToggle = await screen.findByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Default"),
    });
    const sideToggle = screen.getByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Side project"),
    });
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

    const defaultToggle = screen.getByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Default"),
    });
    const sideToggle = screen.getByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Side project"),
    });
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

    const defaultToggle = screen.getByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Default"),
    });
    const sideToggle = screen.getByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Side project"),
    });

    expect(defaultToggle).toHaveAttribute("aria-expanded", "true");

    await userEvent.click(defaultToggle);
    expect(defaultToggle).toHaveAttribute("aria-expanded", "false");

    await userEvent.click(sideToggle);
    expect(sideToggle).toHaveAttribute("aria-expanded", "true");
    expect(await screen.findByText("Side note")).toBeInTheDocument();
  });

  it("loads additional workspace chats from the server when expanding more", async () => {
    renderApp();

    expect(await screen.findByText("Older chat 3")).toBeInTheDocument();
    expect(screen.queryByText("Older chat 4")).not.toBeInTheDocument();
    expect(screen.getByText("7 hidden chats")).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "Show 5 more chats in Default" }),
    );

    await waitFor(() =>
      expect(
        vi
          .mocked(fetch)
          .mock.calls.some(([input]) =>
            String(input).includes("/api/workspaces/workspace-1/chats?"),
          ),
      ).toBe(true),
    );

    expect(screen.getByText("Older chat 4")).toBeInTheDocument();
    expect(screen.getByText("Older chat 8")).toBeInTheDocument();
    expect(screen.queryByText("Older chat 9")).not.toBeInTheDocument();
    expect(screen.getByText("2 hidden chats")).toBeInTheDocument();

    const defaultToggle = screen.getByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Default"),
    });
    await userEvent.click(defaultToggle);
    expect(defaultToggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("Older chat 4")).not.toBeInTheDocument();

    await userEvent.click(defaultToggle);
    expect(defaultToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Older chat 4")).toBeInTheDocument();
    expect(screen.getByText("2 hidden chats")).toBeInTheDocument();
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

    const scrollIntoView = vi.mocked(HTMLElement.prototype.scrollIntoView);
    scrollIntoView.mockClear();

    await userEvent.click(screen.getByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    expect(currentChatTabs()).toEqual([
      "workspace-1/chat-1",
      "workspace-1/chat-2",
    ]);
    expect(
      within(tabList).getByRole("tab", { name: /Second chat/ }),
    ).toHaveAttribute("aria-selected", "true");
    expect(scrollIntoView).toHaveBeenCalledWith({
      block: "nearest",
      inline: "nearest",
    });
    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toBeInTheDocument();

    await userEvent.click(within(tabList).getByRole("tab", { name: /Tool run/ }));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    await userEvent.click(
      within(tabList).getByRole("button", { name: "Close chat tab Tool run" }),
    );

    expect(
      within(tabList).queryByRole("tab", { name: /Tool run/ }),
    ).not.toBeInTheDocument();
    expect(currentChatTabs()).toEqual([
      "workspace-1/chat-2",
    ]);
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

    expect(await screen.findByRole("heading", { name: workspace.name })).toBeInTheDocument();
    expect(messageList.scrollTop).toBe(0);
  });

  it("restores open chat tabs from the URL after refresh", async () => {
    window.history.replaceState(
      null,
      "",
      "/?tab=workspace-1%2Fchat-1&tab=workspace-1%2Fchat-2",
    );

    renderApp();

    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toBeInTheDocument();
    expect(within(tabList).getByRole("tab", { name: /Second chat/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(currentChatTabs()).toEqual([
      "workspace-1/chat-1",
      "workspace-1/chat-2",
    ]);
  });

  it("closes chat tabs to the right from the tab context menu", async () => {
    window.history.replaceState(
      null,
      "",
      "/?tab=workspace-1%2Fchat-1&tab=workspace-1%2Fchat-2&file=workspace-1%2FREADME.md&activeFile=workspace-1%2FREADME.md",
    );

    renderApp();

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    await waitFor(() =>
      expect(within(tabList).getByRole("tab", { name: /README\.md/ })).toHaveAttribute(
        "aria-selected",
        "true",
      ),
    );

    const secondChatTab = within(tabList).getByRole("tab", { name: /Second chat/ });
    const secondChatItem = secondChatTab.closest(".chat-tab-item");
    expect(secondChatItem).not.toBeNull();

    fireEvent.contextMenu(secondChatItem as HTMLElement);
    const menu = await screen.findByRole("menu", { name: "Second chat" });
    for (const item of [
      "Close current tab",
      "Close other tabs",
      "Close all tabs",
      "Close tabs to the right",
      "Close tabs to the left",
    ]) {
      expect(within(menu).getByRole("menuitem", { name: item })).toBeInTheDocument();
    }

    await userEvent.click(within(menu).getByRole("menuitem", { name: "Close tabs to the right" }));

    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toBeInTheDocument();
    expect(within(tabList).getByRole("tab", { name: /Second chat/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(within(tabList).queryByRole("tab", { name: /README\.md/ })).not.toBeInTheDocument();
    expect(currentChatTabs()).toEqual([
      "workspace-1/chat-1",
      "workspace-1/chat-2",
    ]);
    expect(currentFileTabs()).toEqual([]);
  });

  it("opens and selects a historical chat tab before its messages finish loading", async () => {
    const fetchMock = vi.mocked(fetch);
    const delayedMessages = deferred<Response>();
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-2/messages") {
        return delayedMessages.promise;
      }
      return mockFetch(input, init);
    });
    renderApp();

    await userEvent.click(await screen.findByText("Second chat"));

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(within(tabList).getByRole("tab", { name: /Second chat/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    const messageList = document.querySelector(".message-list");
    expect(messageList).not.toBeNull();
    expect(within(messageList as HTMLElement).getByText("Loading…")).toBeInTheDocument();

    await userEvent.click(screen.getByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    await act(async () => {
      delayedMessages.resolve(jsonResponse(secondChatMessages));
      await delayedMessages.promise;
    });

    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.queryByText("Second answer.")).not.toBeInTheDocument();

    await userEvent.click(within(tabList).getByRole("tab", { name: /Second chat/ }));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
  });

  it("loads historical chat messages by recent page and prepends earlier messages", async () => {
    const fetchMock = vi.mocked(fetch);
    const messageRequests: string[] = [];
    const olderMessage = {
      ...chatMessages.messages[0],
      content: "Earlier note.",
      createdAt: "2026-06-10T07:59:00.000Z",
      id: "message-older",
      parts: [{ text: "Earlier note.", type: "text" }],
    };
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const requestUrl = new URL(url, "http://127.0.0.1");

      if (requestUrl.pathname === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        messageRequests.push(requestUrl.toString());
        if (requestUrl.searchParams.get("beforeSequence") === "200") {
          return Promise.resolve(jsonResponse({
            ...chatMessages,
            messages: [olderMessage],
            pagination: { hasMoreBefore: false, nextBeforeSequence: null },
          }));
        }
        return Promise.resolve(jsonResponse({
          ...chatMessages,
          pagination: { hasMoreBefore: true, nextBeforeSequence: 200 },
        }));
      }

      return mockFetch(input, init);
    });

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    expect(new URL(messageRequests[0]).searchParams.get("limit")).toBe("60");

    await userEvent.click(screen.getByRole("button", { name: "Load earlier messages" }));
    expect(await screen.findByText("Earlier note.")).toBeInTheDocument();
    expect(new URL(messageRequests[1]).searchParams.get("beforeSequence")).toBe("200");
    expect(new URL(messageRequests[1]).searchParams.get("limit")).toBe("100");
  });

  it("auto-loads earlier messages only with near-top upward history intent", async () => {
    const fetchMock = vi.mocked(fetch);
    const messageRequests: string[] = [];
    const olderMessage = {
      ...chatMessages.messages[0],
      content: "Earlier note from scroll.",
      createdAt: "2026-06-10T07:59:00.000Z",
      id: "message-older-scroll",
      parts: [{ text: "Earlier note from scroll.", type: "text" }],
    };
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const requestUrl = new URL(url, "http://127.0.0.1");

      if (requestUrl.pathname === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        messageRequests.push(requestUrl.toString());
        if (requestUrl.searchParams.get("beforeSequence") === "200") {
          return Promise.resolve(jsonResponse({
            ...chatMessages,
            messages: [olderMessage],
            pagination: { hasMoreBefore: false, nextBeforeSequence: null },
          }));
        }
        return Promise.resolve(jsonResponse({
          ...chatMessages,
          pagination: { hasMoreBefore: true, nextBeforeSequence: 200 },
        }));
      }

      return mockFetch(input, init);
    });

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(messageRequests).toHaveLength(1);
    expect(new URL(messageRequests[0]).searchParams.get("limit")).toBe("60");

    const messageList = document.querySelector(".message-list");
    expect(messageList).toBeInstanceOf(HTMLElement);
    const list = messageList as HTMLElement;
    let scrollTopValue = 0;

    Object.defineProperty(list, "clientHeight", {
      configurable: true,
      value: 400,
    });
    Object.defineProperty(list, "scrollHeight", {
      configurable: true,
      value: 2000,
    });
    Object.defineProperty(list, "scrollTop", {
      configurable: true,
      get() {
        return scrollTopValue;
      },
      set(value: number) {
        scrollTopValue = Number(value);
      },
    });

    // Seed lastScrollTop above the top threshold without upward intent.
    scrollTopValue = 80;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(messageRequests).toHaveLength(1);

    // Near top without upward intent must not request history.
    scrollTopValue = 20;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(messageRequests).toHaveLength(1);

    // Downward wheel near top must not request history.
    list.dispatchEvent(new WheelEvent("wheel", { bubbles: true, deltaY: 40 }));
    scrollTopValue = 30;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(messageRequests).toHaveLength(1);

    // Ordinary key without upward keys must not request history.
    list.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "a" }));
    scrollTopValue = 20;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(messageRequests).toHaveLength(1);

    // ArrowUp near top loads history with limit=100.
    list.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "ArrowUp" }));
    scrollTopValue = 10;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    expect(await screen.findByText("Earlier note from scroll.")).toBeInTheDocument();
    expect(messageRequests).toHaveLength(2);
    expect(new URL(messageRequests[1]).searchParams.get("beforeSequence")).toBe("200");
    expect(new URL(messageRequests[1]).searchParams.get("limit")).toBe("100");
  });

  it("auto-loads earlier messages from upward wheel near top", async () => {
    const fetchMock = vi.mocked(fetch);
    const messageRequests: string[] = [];
    const olderMessage = {
      ...chatMessages.messages[0],
      content: "Earlier note from wheel.",
      createdAt: "2026-06-10T07:59:00.000Z",
      id: "message-older-wheel",
      parts: [{ text: "Earlier note from wheel.", type: "text" }],
    };
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const requestUrl = new URL(url, "http://127.0.0.1");

      if (requestUrl.pathname === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        messageRequests.push(requestUrl.toString());
        if (requestUrl.searchParams.get("beforeSequence") === "200") {
          return Promise.resolve(jsonResponse({
            ...chatMessages,
            messages: [olderMessage],
            pagination: { hasMoreBefore: false, nextBeforeSequence: null },
          }));
        }
        return Promise.resolve(jsonResponse({
          ...chatMessages,
          pagination: { hasMoreBefore: true, nextBeforeSequence: 200 },
        }));
      }

      return mockFetch(input, init);
    });

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(messageRequests).toHaveLength(1);

    const messageList = document.querySelector(".message-list");
    expect(messageList).toBeInstanceOf(HTMLElement);
    const list = messageList as HTMLElement;
    let scrollTopValue = 0;

    Object.defineProperty(list, "clientHeight", {
      configurable: true,
      value: 400,
    });
    Object.defineProperty(list, "scrollHeight", {
      configurable: true,
      value: 2000,
    });
    Object.defineProperty(list, "scrollTop", {
      configurable: true,
      get() {
        return scrollTopValue;
      },
      set(value: number) {
        scrollTopValue = Number(value);
      },
    });

    scrollTopValue = 80;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    await act(async () => {
      await Promise.resolve();
    });

    // Downward wheel near top must not request history.
    list.dispatchEvent(new WheelEvent("wheel", { bubbles: true, deltaY: 40 }));
    scrollTopValue = 30;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(messageRequests).toHaveLength(1);

    // Upward wheel + decreasing scrollTop near top loads history with limit=100.
    list.dispatchEvent(new WheelEvent("wheel", { bubbles: true, deltaY: -40 }));
    scrollTopValue = 10;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    expect(await screen.findByText("Earlier note from wheel.")).toBeInTheDocument();
    expect(messageRequests).toHaveLength(2);
    expect(new URL(messageRequests[1]).searchParams.get("beforeSequence")).toBe("200");
    expect(new URL(messageRequests[1]).searchParams.get("limit")).toBe("100");
  });

  it("auto-loads earlier messages from pointer drag near top and clears outside release", async () => {
    const fetchMock = vi.mocked(fetch);
    const messageRequests: string[] = [];
    const olderMessage = {
      ...chatMessages.messages[0],
      content: "Earlier note from pointer.",
      createdAt: "2026-06-10T07:59:00.000Z",
      id: "message-older-pointer",
      parts: [{ text: "Earlier note from pointer.", type: "text" }],
    };
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const requestUrl = new URL(url, "http://127.0.0.1");

      if (requestUrl.pathname === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        messageRequests.push(requestUrl.toString());
        if (requestUrl.searchParams.get("beforeSequence") === "200") {
          return Promise.resolve(jsonResponse({
            ...chatMessages,
            messages: [olderMessage],
            pagination: { hasMoreBefore: false, nextBeforeSequence: null },
          }));
        }
        return Promise.resolve(jsonResponse({
          ...chatMessages,
          pagination: { hasMoreBefore: true, nextBeforeSequence: 200 },
        }));
      }

      return mockFetch(input, init);
    });

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(messageRequests).toHaveLength(1);

    const messageList = document.querySelector(".message-list");
    expect(messageList).toBeInstanceOf(HTMLElement);
    const list = messageList as HTMLElement;
    let scrollTopValue = 0;
    const setPointerCapture = vi.fn();
    Object.defineProperty(list, "setPointerCapture", {
      configurable: true,
      value: setPointerCapture,
    });

    Object.defineProperty(list, "clientHeight", {
      configurable: true,
      value: 400,
    });
    Object.defineProperty(list, "scrollHeight", {
      configurable: true,
      value: 2000,
    });
    Object.defineProperty(list, "scrollTop", {
      configurable: true,
      get() {
        return scrollTopValue;
      },
      set(value: number) {
        scrollTopValue = Number(value);
      },
    });

    // Seed scroll position without intent.
    scrollTopValue = 80;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    await act(async () => {
      await Promise.resolve();
    });

    // Pointer drag that decreases scrollTop near top loads history.
    // Message list must not capture on pointerdown (preserves nested click targets).
    list.dispatchEvent(
      new PointerEvent("pointerdown", {
        bubbles: true,
        button: 0,
        pointerId: 7,
        pointerType: "mouse",
      }),
    );
    expect(setPointerCapture).not.toHaveBeenCalled();
    scrollTopValue = 10;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    expect(await screen.findByText("Earlier note from pointer.")).toBeInTheDocument();
    expect(messageRequests).toHaveLength(2);
    expect(new URL(messageRequests[1]).searchParams.get("limit")).toBe("100");

    // Release outside the list via window; gesture must clear so pure scroll does not reload.
    window.dispatchEvent(
      new PointerEvent("pointerup", {
        bubbles: true,
        pointerId: 7,
        pointerType: "mouse",
      }),
    );
    scrollTopValue = 5;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(messageRequests).toHaveLength(2);
  });

  it("clears pointer history gesture on pointerup so later pure scrolls do not auto-load", async () => {
    const fetchMock = vi.mocked(fetch);
    const messageRequests: string[] = [];
    const olderMessage = {
      ...chatMessages.messages[0],
      content: "Earlier note after pointer clear.",
      createdAt: "2026-06-10T07:59:00.000Z",
      id: "message-older-pointer-clear",
      parts: [{ text: "Earlier note after pointer clear.", type: "text" }],
    };
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const requestUrl = new URL(url, "http://127.0.0.1");

      if (requestUrl.pathname === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        messageRequests.push(requestUrl.toString());
        if (requestUrl.searchParams.get("beforeSequence") === "200") {
          return Promise.resolve(jsonResponse({
            ...chatMessages,
            messages: [olderMessage],
            pagination: { hasMoreBefore: false, nextBeforeSequence: null },
          }));
        }
        return Promise.resolve(jsonResponse({
          ...chatMessages,
          pagination: { hasMoreBefore: true, nextBeforeSequence: 200 },
        }));
      }

      return mockFetch(input, init);
    });

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(messageRequests).toHaveLength(1);

    const messageList = document.querySelector(".message-list");
    expect(messageList).toBeInstanceOf(HTMLElement);
    const list = messageList as HTMLElement;
    let scrollTopValue = 0;
    Object.defineProperty(list, "setPointerCapture", {
      configurable: true,
      value: vi.fn(),
    });

    Object.defineProperty(list, "clientHeight", {
      configurable: true,
      value: 400,
    });
    Object.defineProperty(list, "scrollHeight", {
      configurable: true,
      value: 2000,
    });
    Object.defineProperty(list, "scrollTop", {
      configurable: true,
      get() {
        return scrollTopValue;
      },
      set(value: number) {
        scrollTopValue = Number(value);
      },
    });

    scrollTopValue = 80;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));

    // Press, release outside via window (no list capture), then pure upward scroll must not load.
    list.dispatchEvent(
      new PointerEvent("pointerdown", {
        bubbles: true,
        button: 0,
        pointerId: 9,
        pointerType: "mouse",
      }),
    );
    window.dispatchEvent(
      new PointerEvent("pointerup", {
        bubbles: true,
        pointerId: 9,
        pointerType: "mouse",
      }),
    );
    scrollTopValue = 10;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(messageRequests).toHaveLength(1);

    // Explicit upward wheel still loads after the gesture was cleared.
    list.dispatchEvent(new WheelEvent("wheel", { bubbles: true, deltaY: -30 }));
    scrollTopValue = 5;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    expect(await screen.findByText("Earlier note after pointer clear.")).toBeInTheDocument();
    expect(messageRequests).toHaveLength(2);
  });

  it("clears pointer history gesture on window pointercancel after outside drag", async () => {
    const fetchMock = vi.mocked(fetch);
    const messageRequests: string[] = [];
    const olderMessage = {
      ...chatMessages.messages[0],
      content: "Earlier note after pointer cancel.",
      createdAt: "2026-06-10T07:59:00.000Z",
      id: "message-older-pointer-cancel",
      parts: [{ text: "Earlier note after pointer cancel.", type: "text" }],
    };
    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const requestUrl = new URL(url, "http://127.0.0.1");

      if (requestUrl.pathname === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        messageRequests.push(requestUrl.toString());
        if (requestUrl.searchParams.get("beforeSequence") === "200") {
          return Promise.resolve(jsonResponse({
            ...chatMessages,
            messages: [olderMessage],
            pagination: { hasMoreBefore: false, nextBeforeSequence: null },
          }));
        }
        return Promise.resolve(jsonResponse({
          ...chatMessages,
          pagination: { hasMoreBefore: true, nextBeforeSequence: 200 },
        }));
      }

      return mockFetch(input, init);
    });

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    expect(messageRequests).toHaveLength(1);

    const messageList = document.querySelector(".message-list");
    expect(messageList).toBeInstanceOf(HTMLElement);
    const list = messageList as HTMLElement;
    let scrollTopValue = 0;
    Object.defineProperty(list, "clientHeight", {
      configurable: true,
      value: 400,
    });
    Object.defineProperty(list, "scrollHeight", {
      configurable: true,
      value: 2000,
    });
    Object.defineProperty(list, "scrollTop", {
      configurable: true,
      get() {
        return scrollTopValue;
      },
      set(value: number) {
        scrollTopValue = Number(value);
      },
    });

    scrollTopValue = 80;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));

    list.dispatchEvent(
      new PointerEvent("pointerdown", {
        bubbles: true,
        button: 0,
        pointerId: 11,
        pointerType: "mouse",
      }),
    );
    window.dispatchEvent(
      new PointerEvent("pointercancel", {
        bubbles: true,
        pointerId: 11,
        pointerType: "mouse",
      }),
    );
    scrollTopValue = 10;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(messageRequests).toHaveLength(1);

    list.dispatchEvent(new WheelEvent("wheel", { bubbles: true, deltaY: -30 }));
    scrollTopValue = 5;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    expect(await screen.findByText("Earlier note after pointer cancel.")).toBeInTheDocument();
    expect(messageRequests).toHaveLength(2);
  });

  it("marks upward pointer intent before bottom-lock evaluation", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    const messageList = document.querySelector(".message-list");
    expect(messageList).toBeInstanceOf(HTMLElement);
    const list = messageList as HTMLElement;
    let scrollTopValue = 1600;
    const setPointerCapture = vi.fn();
    Object.defineProperty(list, "setPointerCapture", {
      configurable: true,
      value: setPointerCapture,
    });

    Object.defineProperty(list, "clientHeight", {
      configurable: true,
      value: 400,
    });
    Object.defineProperty(list, "scrollHeight", {
      configurable: true,
      value: 2000,
    });
    Object.defineProperty(list, "scrollTop", {
      configurable: true,
      get() {
        return scrollTopValue;
      },
      set(value: number) {
        scrollTopValue = Number(value);
      },
    });

    // Seed bottom position (locked by initial layout).
    scrollTopValue = 1600;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));

    // Pointer-driven jump away from bottom should unlock; a follow-up pure scroll
    // decrease mid-list must not re-arm bottom lock (isAtBottom is false).
    list.dispatchEvent(
      new PointerEvent("pointerdown", {
        bubbles: true,
        button: 0,
        pointerId: 3,
        pointerType: "mouse",
      }),
    );
    expect(setPointerCapture).not.toHaveBeenCalled();
    scrollTopValue = 1200;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    list.dispatchEvent(
      new PointerEvent("pointerup", {
        bubbles: true,
        pointerId: 3,
        pointerType: "mouse",
      }),
    );

    scrollTopValue = 1100;
    list.dispatchEvent(new Event("scroll", { bubbles: true }));
    // Stay mid-list: programmatic bottom snaps would set ~1600 if lock remained true
    // on the next messages-driven layout effect; here we only assert the scroll
    // position itself was not rewritten by the scroll handler.
    expect(scrollTopValue).toBe(1100);
  });

  it("toggles native tool-call details and message actions without list pointer capture", async () => {
    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));
    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();

    const messageList = document.querySelector(".message-list");
    expect(messageList).toBeInstanceOf(HTMLElement);
    const list = messageList as HTMLElement;
    const setPointerCapture = vi.fn();
    Object.defineProperty(list, "setPointerCapture", {
      configurable: true,
      value: setPointerCapture,
    });

    const toolSummary = await screen.findByLabelText("Edit (edit_file)");
    const toolDetails = toolSummary.closest("details");
    expect(toolDetails).toBeInstanceOf(HTMLDetailsElement);
    const details = toolDetails as HTMLDetailsElement;
    expect(details.open).toBe(false);

    await userEvent.click(toolSummary);
    expect(setPointerCapture).not.toHaveBeenCalled();
    expect(details.open).toBe(true);

    await userEvent.click(within(details).getByRole("button", { name: "Raw" }));
    expect(within(details).getByText("Input")).toBeInTheDocument();
    expect(within(details).getByText("Output")).toBeInTheDocument();
    expect(setPointerCapture).not.toHaveBeenCalled();

    await userEvent.click(within(details).getByRole("button", { name: "Compact" }));
    expect(within(details).queryByText("Input")).not.toBeInTheDocument();

    await userEvent.click(toolSummary);
    expect(details.open).toBe(false);

    const assistantBubble = toolSummary.closest(".message-bubble") as HTMLElement | null;
    if (!assistantBubble) {
      throw new Error("Expected assistant message bubble");
    }
    const copyButton = within(assistantBubble).getByRole("button", { name: "Copy message" });
    await userEvent.click(copyButton);
    expect(setPointerCapture).not.toHaveBeenCalled();
    expect(
      within(assistantBubble.closest(".message-row") as HTMLElement).getByRole("button", {
        name: "Copied message",
      }),
    ).toBeInTheDocument();
  });

  it("activates markdown links after message-list pointer gestures without setPointerCapture", async () => {
    const openSpy = vi.spyOn(window, "open").mockReturnValue(null);
    appTestState.chatMessagesResponsesByChatKey["workspace-1/chat-1"] = {
      ...chatMessages,
      messages: [
        {
          ...chatMessages.messages[0],
          content: "See [docs](https://example.com/message-link).",
          parts: [
            {
              text: "See [docs](https://example.com/message-link).",
              type: "text",
            },
          ],
        },
        chatMessages.messages[1],
      ],
    };

    renderApp();
    await userEvent.click(await screen.findByText("Tool run"));

    const link = await screen.findByRole("link", { name: "docs" });
    expect(link).toHaveAttribute("href", "https://example.com/message-link");
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");

    const messageList = document.querySelector(".message-list");
    expect(messageList).toBeInstanceOf(HTMLElement);
    const list = messageList as HTMLElement;
    const setPointerCapture = vi.fn();
    Object.defineProperty(list, "setPointerCapture", {
      configurable: true,
      value: setPointerCapture,
    });

    // History-pagination gesture: primary pointer on the list must not capture,
    // or nested link clicks would be retargeted away from the anchor.
    list.dispatchEvent(
      new PointerEvent("pointerdown", {
        bubbles: true,
        button: 0,
        pointerId: 11,
        pointerType: "mouse",
      }),
    );
    expect(setPointerCapture).not.toHaveBeenCalled();

    window.dispatchEvent(
      new PointerEvent("pointerup", {
        bubbles: true,
        pointerId: 11,
        pointerType: "mouse",
      }),
    );

    await userEvent.click(link);

    expect(setPointerCapture).not.toHaveBeenCalled();
    expect(openSpy).toHaveBeenCalledWith(
      "https://example.com/message-link",
      "_blank",
      "noopener,noreferrer",
    );
  });

  it("keeps open tabs for chats scrolled out of the recent 5-page window", async () => {
    // First page: chat-1, chat-2, older-chat-1..3. Open older-chat-6 (page 2).
    const offPageChatId = "older-chat-6";
    const offPageTitle = "Older chat 6";
    appTestState.chatMessagesResponsesByChatKey[`workspace-1/${offPageChatId}`] = {
      ...secondChatMessages,
      chat: {
        ...secondChatMessages.chat,
        id: offPageChatId,
        title: offPageTitle,
      },
      messages: secondChatMessages.messages.map((message) => ({
        ...message,
        chatId: offPageChatId,
      })),
    };

    renderApp();

    await userEvent.click(
      await screen.findByRole("button", { name: "Show 5 more chats in Default" }),
    );
    expect(await screen.findByText(offPageTitle)).toBeInTheDocument();

    await userEvent.click(screen.getByText(offPageTitle));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(within(tabList).getByRole("tab", { name: new RegExp(offPageTitle) })).toBeInTheDocument();
    expect(currentChatTabs()).toContain(`workspace-1/${offPageChatId}`);

    // Simulate recent-window roll: a newer chat enters page 1 and older-chat-6 leaves.
    const rolledChats = [
      chatSummary(
        "chat-new",
        "Brand new chat",
        "2026-06-06T12:00:00Z",
        "2026-06-06T12:05:00Z",
      ),
      ...workspaceChats,
    ];
    appTestState.workspaceChatsByWorkspaceId = {
      ...appTestState.workspaceChatsByWorkspaceId,
      "workspace-1": rolledChats,
    };
    appTestState.workspaceResponseWorkspaces = appTestState.workspaceResponseWorkspaces.map(
      (item) => {
        const summary = item as { id?: string };
        if (summary.id !== "workspace-1") {
          return item;
        }
        return {
          ...(item as object),
          chatPagination: {
            hasMore: true,
            limit: 5,
            nextCursor: "workspace-page-2",
            total: rolledChats.length,
          },
          chats: rolledChats.slice(0, 5),
        };
      },
    );

    await userEvent.click(screen.getByRole("button", { name: "Refresh workspaces" }));
    await waitFor(() => {
      expect(screen.getByText("Brand new chat")).toBeInTheDocument();
    });

    const workspaceList = screen.getByRole("navigation", { name: "Workspace list" });
    expect(within(workspaceList).queryByText(offPageTitle)).not.toBeInTheDocument();
    expect(within(workspaceList).getByText("Brand new chat")).toBeInTheDocument();

    expect(
      within(tabList).getByRole("tab", { name: new RegExp(offPageTitle) }),
    ).toBeInTheDocument();
    expect(currentChatTabs()).toContain(`workspace-1/${offPageChatId}`);

    await userEvent.click(
      within(tabList).getByRole("tab", { name: new RegExp(offPageTitle) }),
    );
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
  });

  it("closes open tabs when deleting a non-active open chat", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await screen.findByText("Please inspect README.");
    await userEvent.click(screen.getByText("Second chat"));
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(currentChatTabs()).toEqual([
      "workspace-1/chat-1",
      "workspace-1/chat-2",
    ]);

    const workspaceList = screen.getByRole("navigation", { name: "Workspace list" });
    const toolRunButton = within(workspaceList).getByText("Tool run").closest("button");
    if (!toolRunButton) {
      throw new Error("Expected Tool run history item button");
    }

    fireEvent.contextMenu(toolRunButton);
    const chatMenu = await screen.findByRole("menu", { name: "Tool run" });
    await userEvent.click(within(chatMenu).getByRole("menuitem", { name: "Delete chat" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete this chat?" });
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Confirm delete chat" }),
    );

    await waitFor(() => {
      expect(
        within(tabList).queryByRole("tab", { name: /Tool run/ }),
      ).not.toBeInTheDocument();
    });
    expect(currentChatTabs()).toEqual(["workspace-1/chat-2"]);
    expect(within(tabList).getByRole("tab", { name: /Second chat/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByText("Second answer.")).toBeInTheDocument();
  });

  it("restores URL chat tabs that are outside the recent 5-page window", async () => {
    const offPageChatId = "older-chat-6";
    const offPageTitle = "Older chat 6";
    appTestState.chatMessagesResponsesByChatKey[`workspace-1/${offPageChatId}`] = {
      ...secondChatMessages,
      chat: {
        ...secondChatMessages.chat,
        id: offPageChatId,
        title: offPageTitle,
      },
      messages: secondChatMessages.messages.map((message) => ({
        ...message,
        chatId: offPageChatId,
      })),
    };

    window.history.replaceState(
      null,
      "",
      `/?tab=workspace-1%2Fchat-1&tab=workspace-1%2F${encodeURIComponent(offPageChatId)}`,
    );

    renderApp();

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    await waitFor(() => {
      expect(
        within(tabList).getByRole("tab", { name: new RegExp(offPageTitle) }),
      ).toBeInTheDocument();
    });
    expect(within(tabList).getByRole("tab", { name: /Tool run/ })).toBeInTheDocument();
    expect(currentChatTabs()).toEqual([
      "workspace-1/chat-1",
      `workspace-1/${offPageChatId}`,
    ]);

    await userEvent.click(
      within(tabList).getByRole("tab", { name: new RegExp(offPageTitle) }),
    );
    expect(await screen.findByText("Second answer.")).toBeInTheDocument();
  });

});
