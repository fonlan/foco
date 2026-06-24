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
  secondChatMessages,
  todoGraph,
  workspace,
  workspaceMemory,
} from "./test-utils/app-test-harness";

function currentChatTabs() {
  return new URLSearchParams(window.location.search).getAll("tab");
}

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
        ([url]) => typeof url === "string" && url === "/api/scheduled-tasks",
      ),
    ).toBe(true);

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
    expect(within(statusSelect).getByRole("option", { name: "Enabled" })).toBeInTheDocument();
    expect(within(statusSelect).getByRole("option", { name: "Paused" })).toBeInTheDocument();
    expect(within(statusSelect).queryByRole("option", { name: "Completed" })).not.toBeInTheDocument();
    expect(within(statusSelect).queryByRole("option", { name: "Archived" })).not.toBeInTheDocument();
    expect(screen.getByLabelText("Agent")).toHaveValue("agent-definition-coordinator");
    expect(screen.getByLabelText("Model")).toHaveValue("gpt-test");
    expect(screen.getByLabelText("Provider")).toHaveValue("openai");
    expect(screen.getByRole("checkbox", { name: "Enable Team mode" })).toBeChecked();

    const unitSelect = screen.getByLabelText("Unit");
    expect(within(unitSelect).getByRole("option", { name: "Weeks" })).toBeInTheDocument();
    expect(within(unitSelect).getByRole("option", { name: "Months" })).toBeInTheDocument();
    await userEvent.selectOptions(unitSelect, "months");
    await userEvent.selectOptions(screen.getByLabelText("Concurrency"), "force_run");
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
      provider_id: "openai",
    });

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

  it("resizes the context panel height on mobile browsers", async () => {
    const originalInnerWidth = window.innerWidth;
    const originalInnerHeight = window.innerHeight;

    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 390,
    });
    Object.defineProperty(window, "innerHeight", {
      configurable: true,
      value: 844,
    });

    try {
      renderApp();
      await screen.findByPlaceholderText(defaultComposerPlaceholder);
      await userEvent.click(await screen.findByRole("button", { name: "Open context panel" }));

      const splitter = await screen.findByRole("separator", {
        name: "Resize context panel",
      });
      const appShell = splitter.closest(".app-shell") as HTMLElement | null;
      if (!appShell) {
        throw new Error("Expected context panel splitter inside app shell");
      }

      fireEvent.pointerDown(splitter, { clientY: 620, pointerId: 1 });

      await waitFor(() => {
        expect(document.body.style.cursor).toBe("row-resize");
        expect(document.body.style.userSelect).toBe("none");
        expect(appShell.style.getPropertyValue("--context-panel-mobile-height")).toBe("224px");
      });

      fireEvent.pointerMove(window, { clientY: 560 });

      await waitFor(() => {
        expect(appShell.style.getPropertyValue("--context-panel-mobile-height")).toBe("284px");
      });

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
      Object.defineProperty(window, "innerHeight", {
        configurable: true,
        value: originalInnerHeight,
      });
    }
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

  it("does not estimate context usage for opened chats or composer drafts", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    const usage = await screen.findByRole("status", {
      name: "Context usage 0%",
    });
    expect(usage).toHaveTextContent("0%");

    const usageCallsBeforeDraft = fetchMock.mock.calls.filter(
      ([url]) =>
        typeof url === "string" &&
        url === "/api/workspaces/workspace-1/context-usage",
    );
    expect(usageCallsBeforeDraft).toHaveLength(0);

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

    expect(await screen.findByText("API overview")).toBeInTheDocument();
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
    expect(within(messageList as HTMLElement).getByText("Loading...")).toBeInTheDocument();

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

});
