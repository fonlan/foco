import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  activeMemory,
  appTestState,
  changeInput,
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
  workspaceSpec,
} from "./test-utils/app-test-harness";
type WorkspaceFixture = {
  commonCommands: typeof workspace.commonCommands;
  id: string;
  logoUrl: string | null;
  name: string;
  path: string;
  pinned: boolean;
  terminalShell: string;
};

function configuredWorkspace(item: WorkspaceFixture, isDefault = false) {
  return {
    commonCommands: item.commonCommands,
    id: item.id,
    isDefault,
    logoUrl: item.logoUrl,
    name: item.name,
    path: item.path,
    pinned: item.pinned,
    terminalShell: item.terminalShell,
  };
}

function workspaceButton(name: string) {
  const workspaceList = screen.getByRole("navigation", { name: "Workspace list" });
  return within(workspaceList).getByRole("button", { name });
}

function workspaceDragContainer(name: string) {
  const container = workspaceButton(name).closest("div[draggable='true']");
  if (!container) {
    throw new Error(`Expected draggable workspace container for ${name}`);
  }

  return container;
}

function expectWorkspaceOrder(names: string[]) {
  const buttons = names.map(workspaceButton);
  for (let index = 0; index < buttons.length - 1; index += 1) {
    expect(
      Boolean(
        buttons[index].compareDocumentPosition(buttons[index + 1]) &
          Node.DOCUMENT_POSITION_FOLLOWING,
      ),
    ).toBe(true);
  }
}
function dragDataTransfer() {
  return {
    effectAllowed: "",
    setData: vi.fn(),
  };
}

describe("app-workspaces verification surfaces", () => {
  beforeEach(resetAppTestEnvironment);

  it("reorders main workspace buttons within a pinned group and shares the saved order with settings", async () => {
    const thirdWorkspace = {
      ...secondaryWorkspace,
      chats: [],
      id: "workspace-3",
      name: "Pinned project",
      path: "C:\\Users\\fonla\\Documents\\Repos\\PinnedProject",
      pinned: true,
    };
    const fourthWorkspace = {
      ...secondaryWorkspace,
      chats: [],
      id: "workspace-4",
      name: "Another project",
      path: "C:\\Users\\fonla\\Documents\\Repos\\AnotherProject",
      pinned: false,
    };
    appTestState.workspaceResponseWorkspaces = [
      { ...thirdWorkspace },
      { ...workspace },
      { ...secondaryWorkspace },
      { ...fourthWorkspace },
    ];
    appTestState.settingsResponse = {
      ...settings,
      workspaces: [
        configuredWorkspace(thirdWorkspace),
        configuredWorkspace(workspace, true),
        configuredWorkspace(secondaryWorkspace),
        configuredWorkspace(fourthWorkspace),
      ],
    };

    renderApp();

    await screen.findByRole("navigation", { name: "Workspace list" });
    await screen.findByRole("button", { name: "Side project" });
    fireEvent.dragStart(workspaceDragContainer("Side project"), {
      dataTransfer: dragDataTransfer(),
    });
    fireEvent.dragOver(workspaceDragContainer("Another project"));
    expectWorkspaceOrder(["Pinned project", "Default", "Another project", "Side project"]);
    fireEvent.drop(workspaceDragContainer("Another project"));

    await waitFor(() => {
      expect(appTestState.lastWorkspaceOrderRequest).toEqual([
        "workspace-3",
        "workspace-1",
        "workspace-4",
        "workspace-2",
      ]);
    });
    expectWorkspaceOrder(["Pinned project", "Default", "Another project", "Side project"]);

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Workspaces" }));
    const defaultSettingsButton = await screen.findByRole("button", {
      name: "Edit workspace Default",
    });
    const anotherSettingsButton = screen.getByRole("button", {
      name: "Edit workspace Another project",
    });
    const sideSettingsButton = screen.getByRole("button", {
      name: "Edit workspace Side project",
    });
    expect(
      Boolean(
        defaultSettingsButton.compareDocumentPosition(anotherSettingsButton) &
          Node.DOCUMENT_POSITION_FOLLOWING,
      ),
    ).toBe(true);
    expect(
      Boolean(
        anotherSettingsButton.compareDocumentPosition(sideSettingsButton) &
          Node.DOCUMENT_POSITION_FOLLOWING,
      ),
    ).toBe(true);
  });

  it("ignores main workspace drops across pinned groups", async () => {
    const pinnedWorkspace = {
      ...secondaryWorkspace,
      chats: [],
      id: "workspace-3",
      name: "Pinned project",
      path: "C:\\Users\\fonla\\Documents\\Repos\\PinnedProject",
      pinned: true,
    };
    appTestState.workspaceResponseWorkspaces = [
      { ...pinnedWorkspace },
      { ...workspace },
      { ...secondaryWorkspace },
    ];
    appTestState.settingsResponse = {
      ...settings,
      workspaces: [
        configuredWorkspace(pinnedWorkspace),
        configuredWorkspace(workspace, true),
        configuredWorkspace(secondaryWorkspace),
      ],
    };

    renderApp();

    await screen.findByRole("navigation", { name: "Workspace list" });
    await screen.findByRole("button", { name: "Side project" });
    fireEvent.dragStart(workspaceDragContainer("Side project"), {
      dataTransfer: dragDataTransfer(),
    });
    fireEvent.dragOver(workspaceDragContainer("Pinned project"));
    fireEvent.drop(workspaceDragContainer("Pinned project"));
    fireEvent.dragEnd(workspaceDragContainer("Side project"));

    expectWorkspaceOrder(["Pinned project", "Default", "Side project"]);
    expect(appTestState.lastWorkspaceOrderRequest).toBeNull();
  });

  it("refreshes the main workspace list after settings pinning saves grouped order", async () => {
    const anotherWorkspace = {
      ...secondaryWorkspace,
      chats: [],
      id: "workspace-3",
      name: "Another project",
      path: "C:\\Users\\fonla\\Documents\\Repos\\AnotherProject",
      pinned: false,
    };
    appTestState.workspaceResponseWorkspaces = [
      { ...workspace },
      { ...secondaryWorkspace },
      { ...anotherWorkspace },
    ];
    appTestState.settingsResponse = {
      ...settings,
      workspaces: [
        configuredWorkspace(workspace, true),
        configuredWorkspace(secondaryWorkspace),
        configuredWorkspace(anotherWorkspace),
      ],
    };

    renderApp();

    await screen.findByRole("navigation", { name: "Workspace list" });
    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Workspaces" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Pin workspace Another project" }),
    );

    await waitFor(() => {
      expect(appTestState.lastWorkspaceOrderRequest).toEqual([
        "workspace-3",
        "workspace-1",
        "workspace-2",
      ]);
    });
    await userEvent.click(screen.getByRole("button", { name: "Home" }));

    expectWorkspaceOrder(["Another project", "Default", "Side project"]);
  });
  it("sorts workspace chat history by chat creation time and shows seconds", async () => {
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          chatSummary(
            "chat-updated-later",
            "Updated later",
            "2026-06-05T09:00:01Z",
            "2026-06-05T13:00:01Z",
          ),
          chatSummary(
            "chat-created-later",
            "Created later",
            "2026-06-05T10:00:02Z",
            "2026-06-05T10:00:02Z",
          ),
          chatSummary(
            "chat-created-earlier",
            "Created earlier",
            "2026-06-05T08:00:03Z",
            "2026-06-05T14:00:03Z",
          ),
        ],
      },
      secondaryWorkspace,
    ];

    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const createdLaterTitle = await within(workspaceList).findByText("Created later");
    const updatedLaterTitle = await within(workspaceList).findByText("Updated later");
    const createdEarlierTitle = await within(workspaceList).findByText("Created earlier");
    const createdLaterButton = createdLaterTitle.closest("button");
    const updatedLaterButton = updatedLaterTitle.closest("button");
    const createdEarlierButton = createdEarlierTitle.closest("button");
    if (!createdLaterButton || !updatedLaterButton || !createdEarlierButton) {
      throw new Error("Expected workspace chat history item buttons");
    }

    expect(
      createdLaterButton.compareDocumentPosition(updatedLaterButton) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      updatedLaterButton.compareDocumentPosition(createdEarlierButton) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(within(createdLaterButton).getByText(/:02\b/)).toBeInTheDocument();
    expect(within(updatedLaterButton).getByText(/:01\b/)).toBeInTheDocument();
    expect(within(createdEarlierButton).getByText(/:03\b/)).toBeInTheDocument();
  });

  it("places scheduled workspace chats by chat creation time", async () => {
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          chatSummary(
            "chat-created-later",
            "Created later",
            "2026-06-05T13:00:00Z",
            "2026-06-05T13:00:00Z",
          ),
          chatSummary(
            "chat-created-earlier",
            "Created earlier",
            "2026-06-05T11:00:00Z",
            "2026-06-05T15:00:00Z",
          ),
        ],
      },
      secondaryWorkspace,
    ];

    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    await within(workspaceList).findByRole("button", { name: "Default" });
    await userEvent.click(within(workspaceList).getByRole("button", { name: "Default" }));
    await userEvent.click(
      within(workspaceList).getByRole("button", { name: "New chat in Default" }),
    );
    await userEvent.type(
      screen.getByPlaceholderText(defaultComposerPlaceholder),
      "Queued chat",
    );
    fireEvent.click(screen.getByRole("button", { name: "Send message" }), {
      ctrlKey: true,
    });

    const createdLaterButton = (
      await within(workspaceList).findByText("Created later")
    ).closest("button");
    const queuedButton = (await within(workspaceList).findByText("Queued chat")).closest(
      "button",
    );
    const createdEarlierButton = (
      await within(workspaceList).findByText("Created earlier")
    ).closest("button");
    if (!createdLaterButton || !queuedButton || !createdEarlierButton) {
      throw new Error("Expected workspace chat history item buttons");
    }

    expect(
      createdLaterButton.compareDocumentPosition(queuedButton) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      queuedButton.compareDocumentPosition(createdEarlierButton) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
  });

  it("keeps workspace chat dot running from workspace active run summary", async () => {
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          {
            ...workspace.chats[0],
            activeRun: {
              chatId: "chat-1",
              lastSequence: 0,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
          },
          ...workspace.chats.slice(1),
        ],
      },
      secondaryWorkspace,
    ];

    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyTitle = await within(workspaceList).findByText("Tool run");
    const historyButton = historyTitle.closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }

    expect(historyButton.querySelector(".session-status-dot")).toHaveClass(
      "session-status-dot-running",
    );
  });

  it("clears stale workspace active run summary when loaded chat has no active run", async () => {
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          {
            ...workspace.chats[0],
            activeRun: {
              chatId: "chat-1",
              lastSequence: 0,
              runId: "stale-run",
              workspaceId: "workspace-1",
            },
          },
          ...workspace.chats.slice(1),
        ],
      },
      secondaryWorkspace,
    ];

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
    expect(statusDot()).toHaveClass("session-status-dot-running");

    await userEvent.click(historyButton);
    await screen.findByText("Please inspect README.");

    await waitFor(() =>
      expect(statusDot()).toHaveClass("session-status-dot-open"),
    );
  });

  it("shows persisted code line changes beside each workspace chat time", async () => {
    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyTitle = await within(workspaceList).findByText("Tool run");
    const historyButton = historyTitle.closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }

    expect(
      within(historyButton).queryByLabelText("Code changes +3 -2"),
    ).not.toBeInTheDocument();

    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: [
          {
            ...workspace.chats[0],
            codeChangeStats: { additions: 3, deletions: 2 },
          },
          ...workspace.chats.slice(1),
        ],
      },
      secondaryWorkspace,
    ];
    await userEvent.click(historyButton);
    await screen.findByText("Please inspect README.");
    await userEvent.type(screen.getByPlaceholderText(defaultComposerPlaceholder), "continue");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());
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

    const updatedHistoryTitle = await within(workspaceList).findByText("Tool run");
    const updatedHistoryButton = updatedHistoryTitle.closest("button");
    if (!updatedHistoryButton) {
      throw new Error("Expected updated Tool run history item button");
    }

    expect(
      await within(updatedHistoryButton).findByLabelText("Code changes +3 -2"),
    ).toBeInTheDocument();
    expect(within(updatedHistoryButton).getByText("+3")).toHaveClass("chat-diff-add");
    expect(within(updatedHistoryButton).getByText("-2")).toHaveClass(
      "chat-diff-delete",
    );
  });

  it("shows chat tab scroll controls only when tabs overflow and supports wheel scrolling", async () => {
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(screen.getByText("Second chat"));

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    const tabsContainer = tabList.parentElement;
    if (!tabsContainer) {
      throw new Error("Expected chat tab list to have a container");
    }
    expect(tabsContainer).toHaveClass("flex", "flex-nowrap", "overflow-hidden");
    expect(
      screen.queryByRole("button", { name: "Scroll chat tabs left" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Scroll chat tabs right" }),
    ).not.toBeInTheDocument();

    Object.defineProperties(tabsContainer, {
      clientWidth: { configurable: true, value: 360 },
    });
    Object.defineProperties(tabList, {
      clientWidth: { configurable: true, value: 300 },
      scrollWidth: { configurable: true, value: 340 },
    });
    fireEvent.scroll(tabList);
    expect(
      screen.queryByRole("button", { name: "Scroll chat tabs left" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Scroll chat tabs right" }),
    ).not.toBeInTheDocument();

    Object.defineProperties(tabList, {
      clientWidth: { configurable: true, value: 180 },
      scrollWidth: { configurable: true, value: 720 },
    });
    tabList.scrollLeft = 0;
    fireEvent.scroll(tabList);

    const leftButton = await screen.findByRole("button", {
      name: "Scroll chat tabs left",
    });
    const rightButton = screen.getByRole("button", {
      name: "Scroll chat tabs right",
    });
    expect(leftButton).toBeDisabled();
    expect(rightButton).toBeEnabled();

    fireEvent.wheel(tabList, { deltaY: 120 });
    expect(tabList.scrollLeft).toBe(120);
    await waitFor(() => expect(leftButton).toBeEnabled());
  });

  it("asks for confirmation before deleting a chat", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyTitle = await within(workspaceList).findByText("Tool run");
    const historyButton = historyTitle.closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }

    fireEvent.contextMenu(historyButton);
    const chatMenu = await screen.findByRole("menu", { name: "Tool run" });
    await userEvent.click(
      within(chatMenu).getByRole("menuitem", { name: "Delete chat" }),
    );

    const dialog = await screen.findByRole("dialog", {
      name: "Delete this chat?",
    });
    expect(within(dialog).getByText("Tool run")).toBeInTheDocument();
    expect(within(dialog).getByText("Default")).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.some(
        ([url]) =>
          typeof url === "string" &&
          url === "/api/workspaces/workspace-1/chats/chat-1/delete",
      ),
    ).toBe(false);

    await userEvent.click(
      within(dialog).getByRole("button", { name: "Confirm delete chat" }),
    );

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/chats/chat-1/delete",
        expect.objectContaining({ method: "POST" }),
      );
    });
    expect(screen.queryByRole("dialog", { name: "Delete this chat?" })).not.toBeInTheDocument();
    expect(screen.queryByText("Tool run")).not.toBeInTheDocument();
    expect(screen.getByText("Second chat")).toBeInTheDocument();
  });

  it("adds a workspace with a selectable slash-style path", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Add workspace" }));

    const dialog = await screen.findByRole("dialog", { name: "Add workspace" });
    const nameInput = within(dialog).getByPlaceholderText("Workspace name");
    const pathInput = within(dialog).getByPlaceholderText("C:/Users/name/workspace");
    expect(pathInput).toBeInTheDocument();

    const choosePathButton = within(dialog).getByRole("button", {
      name: "Choose workspace path",
    });
    await waitFor(() => expect(choosePathButton).toBeEnabled());
    await userEvent.click(choosePathButton);

    await waitFor(() => {
      expect(pathInput).toHaveValue("C:/Users/fonla/Documents/Repos/NewWorkspace");
      expect(nameInput).toHaveValue("NewWorkspace");
    });

    await userEvent.upload(
      within(dialog).getByLabelText("Workspace icon file"),
      new File([new Uint8Array([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])], "workspace-logo.png", {
        type: "image/png",
      }),
    );

    await waitFor(() => {
      expect(within(dialog).getByText("workspace-logo.png")).toBeInTheDocument();
    });

    await userEvent.click(
      within(dialog).getByRole("checkbox", { name: "Enable Project Spec" }),
    );
    await userEvent.click(within(dialog).getByRole("button", { name: "Add workspace" }));

    await waitFor(() => {
      const addWorkspaceCall = fetchMock.mock.calls.find(
        ([url, init]) => url === "/api/workspaces/add" && init?.method === "POST",
      );
      expect(addWorkspaceCall).toBeDefined();
      expect(JSON.parse(String(addWorkspaceCall?.[1]?.body))).toEqual({
        contentBase64: expect.any(String),
        name: "NewWorkspace",
        path: "C:/Users/fonla/Documents/Repos/NewWorkspace",
      });
      const specSettingsCall = fetchMock.mock.calls.find(
        ([url, init]) =>
          url === "/api/workspaces/new-workspace/spec/settings" &&
          init?.method === "PUT",
      );
      expect(specSettingsCall).toBeDefined();
      expect(JSON.parse(String(specSettingsCall?.[1]?.body))).toEqual({
        enabled: true,
        injectEnabled: false,
      });
    });

    expect(screen.queryByRole("dialog", { name: "Add workspace" })).not.toBeInTheDocument();
  });

  it("uploads and clears a workspace icon in workspace settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Workspaces" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Edit workspace Default" }),
    );

    const iconInput = await screen.findByLabelText("Workspace icon file");
    await userEvent.upload(
      iconInput,
      new File([new Uint8Array([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])], "logo.png", {
        type: "image/png",
      }),
    );

    await waitFor(() => {
      const uploadCall = fetchMock.mock.calls.find(
        ([url, init]) =>
          url === "/api/workspaces/workspace-1/logo" && init?.method === "POST",
      );
      expect(uploadCall).toBeDefined();
      expect(JSON.parse(String(uploadCall?.[1]?.body))).toEqual({
        contentBase64: expect.any(String),
      });
    });

    await userEvent.click(screen.getByRole("button", { name: "Clear workspace icon" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/logo",
        expect.objectContaining({ method: "DELETE" }),
      );
    });
  });

  it("saves Project Spec enablement from workspace settings", async () => {
    const fetchMock = vi.mocked(fetch);
    appTestState.workspaceSpecResponse = {
      ...workspaceSpec,
      settings: { enabled: false, injectEnabled: false },
    };
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Workspaces" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Edit workspace Default" }),
    );

    const specCheckbox = await screen.findByRole("checkbox", {
      name: "Enable Project Spec",
    });
    await waitFor(() => expect(specCheckbox).toBeEnabled());
    expect(specCheckbox).not.toBeChecked();
    await userEvent.click(specCheckbox);
    await userEvent.click(screen.getByRole("button", { name: "Save workspace" }));

    await waitFor(() => {
      const specSettingsCall = fetchMock.mock.calls.find(
        ([url, init]) =>
          url === "/api/workspaces/workspace-1/spec/settings" &&
          init?.method === "PUT",
      );
      expect(specSettingsCall).toBeDefined();
      expect(JSON.parse(String(specSettingsCall?.[1]?.body))).toEqual({
        enabled: true,
        injectEnabled: false,
      });
    });
  });

});
