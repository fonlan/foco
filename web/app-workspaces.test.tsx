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
import { browserRouteFromPathname } from "./shared/browser-route";
type WorkspaceFixture = {
  commonCommands: typeof workspace.commonCommands;
  connectionStatus?: string;
  displayPath?: string;
  id: string;
  lastRemoteError?: string | null;
  logoUrl: string | null;
  name: string;
  path: string;
  pinned: boolean;
  remotePath?: string | null;
  serverId?: string | null;
  serverName?: string | null;
  terminalShell: string;
};

function configuredWorkspace(item: WorkspaceFixture, isDefault = false) {
  return {
    commonCommands: item.commonCommands,
    connectionStatus: item.connectionStatus,
    displayPath: item.displayPath,
    id: item.id,
    isDefault,
    lastRemoteError: item.lastRemoteError,
    logoUrl: item.logoUrl,
    name: item.name,
    path: item.path,
    pinned: item.pinned,
    remotePath: item.remotePath,
    serverId: item.serverId,
    serverName: item.serverName,
    terminalShell: item.terminalShell,
  };
}

function remoteWorkspaceFixture() {
  return {
    ...secondaryWorkspace,
    chats: [],
    connectionStatus: "ready",
    displayPath: "dev-box:/home/fonla/repos/remote-project",
    id: "workspace-remote",
    lastRemoteError: null,
    name: "Remote project",
    path: "dev-box:/home/fonla/repos/remote-project",
    remotePath: "/home/fonla/repos/remote-project",
    serverId: "server-1",
    serverName: "dev-box",
  };
}

function configureRemoteWorkspaceSpec() {
  const remoteWorkspace = remoteWorkspaceFixture();
  appTestState.workspaceResponseWorkspaces = [{ ...workspace }, remoteWorkspace];
  appTestState.settingsResponse = {
    ...appTestState.settingsResponse,
    workspaces: [
      configuredWorkspace(workspace, true),
      configuredWorkspace(remoteWorkspace),
    ],
  };
  appTestState.workspaceSpecResponsesByWorkspaceId = {
    ...appTestState.workspaceSpecResponsesByWorkspaceId,
    [remoteWorkspace.id]: {
      ...workspaceSpec,
      settings: { enabled: false, injectEnabled: false },
    },
  };
  return remoteWorkspace;
}

function workspaceButton(name: string) {
  const workspaceList = screen.getByRole("navigation", { name: "Workspace list" });
  return within(workspaceList).getByRole("button", {
    name: (accessibleName, element) =>
      element.hasAttribute("aria-expanded") && accessibleName.startsWith(name),
  });
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

  it("shows local and remote workspace paths under the workspace name", async () => {
    const remoteWorkspace = {
      ...secondaryWorkspace,
      chats: [],
      connectionStatus: "ready",
      displayPath: "server:/home/fonla/repos/remote-project",
      id: "workspace-remote",
      name: "Remote project",
      path: "server:/home/fonla/repos/remote-project",
      remotePath: "/home/fonla/repos/remote-project",
      serverId: "server-1",
      serverName: "dev-box",
    };
    appTestState.workspaceResponseWorkspaces = [{ ...workspace }, remoteWorkspace];

    renderApp();

    const workspaceList = await screen.findByRole("navigation", { name: "Workspace list" });
    const localButton = await within(workspaceList).findByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Default"),
    });
    const remoteButton = within(workspaceList).getByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Remote project"),
    });

    expect(within(localButton).getByText(workspace.displayPath)).toBeInTheDocument();
    expect(within(remoteButton).getByText(remoteWorkspace.displayPath)).toBeInTheDocument();
  });

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
    await screen.findByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Side project"),
    });
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
    await userEvent.click(await screen.findByRole("button", { name: "Workspaces" }));
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

  it("saves the latest main workspace drag preview when drop happens before React re-renders", async () => {
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

    await screen.findByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Side project"),
    });
    fireEvent.dragStart(workspaceDragContainer("Side project"), {
      dataTransfer: dragDataTransfer(),
    });
    await act(async () => {
      fireEvent.dragOver(workspaceDragContainer("Another project"));
      fireEvent.drop(workspaceDragContainer("Another project"));
    });

    await waitFor(() => {
      expect(appTestState.lastWorkspaceOrderRequest).toEqual([
        "workspace-1",
        "workspace-3",
        "workspace-2",
      ]);
    });
    expectWorkspaceOrder(["Default", "Another project", "Side project"]);
  });
  it("commits the main workspace preview on drag end when drop is missed", async () => {
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

    await screen.findByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Side project"),
    });
    fireEvent.dragStart(workspaceDragContainer("Side project"), {
      dataTransfer: dragDataTransfer(),
    });
    fireEvent.dragOver(workspaceDragContainer("Another project"));
    fireEvent.dragEnd(workspaceDragContainer("Side project"));

    await waitFor(() => {
      expect(appTestState.lastWorkspaceOrderRequest).toEqual([
        "workspace-1",
        "workspace-3",
        "workspace-2",
      ]);
    });
    expectWorkspaceOrder(["Default", "Another project", "Side project"]);
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
    await screen.findByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Side project"),
    });
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
    await userEvent.click(await screen.findByRole("button", { name: "Workspaces" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Pin workspace Another project" }),
    );

    await waitFor(() => {
      expect(appTestState.lastManualWorkspaceRequest).toEqual(
        expect.objectContaining({ id: "workspace-3", pinned: true }),
      );
    });
    expect(appTestState.lastWorkspaceOrderRequest).toBeNull();
    expect(
      screen.getByRole("button", { name: "Unpin workspace Another project" }),
    ).toBeInTheDocument();
    const pinnedSettingsButton = screen.getByRole("button", {
      name: "Edit workspace Another project",
    });
    const defaultSettingsButton = screen.getByRole("button", {
      name: "Edit workspace Default",
    });
    expect(
      Boolean(
        pinnedSettingsButton.compareDocumentPosition(defaultSettingsButton) &
          Node.DOCUMENT_POSITION_FOLLOWING,
      ),
    ).toBe(true);
    await userEvent.click(screen.getByRole("button", { name: "Home" }));
    expectWorkspaceOrder(["Another project", "Default", "Side project"]);
    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(await screen.findByRole("button", { name: "Workspaces" }));

    await userEvent.click(
      screen.getByRole("button", { name: "Unpin workspace Another project" }),
    );
    await waitFor(() => {
      expect(appTestState.lastManualWorkspaceRequest).toEqual(
        expect.objectContaining({ id: "workspace-3", pinned: false }),
      );
    });
    expect(
      screen.getByRole("button", { name: "Pin workspace Another project" }),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Pin workspace Another project" }));
    await waitFor(() => {
      expect(appTestState.lastManualWorkspaceRequest).toEqual(
        expect.objectContaining({ id: "workspace-3", pinned: true }),
      );
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
    await within(workspaceList).findByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Default"),
    });
    await userEvent.click(
      within(workspaceList).getByRole("button", {
        name: (accessibleName, element) =>
          element.hasAttribute("aria-expanded") && accessibleName.startsWith("Default"),
      }),
    );
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

  it("clears workspace active run dot immediately after manual cancel", async () => {
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

    vi.mocked(fetch).mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];
      if (path === "/api/workspaces/workspace-1/chats/chat-1/messages") {
        return Promise.resolve(
          jsonResponse({
            ...chatMessages,
            activeRun: {
              acceptingGuidance: false,
              chatId: "chat-1",
              lastSequence: 0,
              runId: "request-stream",
              workspaceId: "workspace-1",
            },
          }),
        );
      }
      return mockFetch(input, init);
    });

    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyButton = (await within(workspaceList).findByText("Tool run")).closest(
      "button",
    );
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }
    const statusDot = () => historyButton.querySelector(".session-status-dot");

    expect(statusDot()).toHaveClass("session-status-dot-running");

    await userEvent.click(historyButton);
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());
    await userEvent.click(await screen.findByRole("button", { name: "Cancel run" }));

    await waitFor(() => expect(statusDot()).not.toHaveClass("session-status-dot-running"));
    expect(statusDot()).toHaveClass("session-status-dot-open");
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

  it("confirms deletion, accepts a lightweight response, and clears the active chat", async () => {
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

    await userEvent.click(historyButton);
    await screen.findByText("Please inspect README.");
    expect(window.location.href).toContain("chat-1");

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
    await waitFor(() => {
      expect(
        fetchMock.mock.calls.filter(([url]) => url === "/api/workspaces"),
      ).toHaveLength(2);
    });
    expect(screen.queryByRole("dialog", { name: "Delete this chat?" })).not.toBeInTheDocument();
    expect(screen.queryByText("Tool run")).not.toBeInTheDocument();
    expect(screen.getByText("Second chat")).toBeInTheDocument();
    expect(screen.queryByText("Please inspect README.")).not.toBeInTheDocument();
    expect(window.location.href).not.toContain("chat-1");
  });

  it("refreshes paginated remote chats after a lightweight delete response", async () => {
    const remoteWorkspace = remoteWorkspaceFixture();
    const remoteChats = Array.from({ length: 6 }, (_, index) =>
      chatSummary(
        `remote-chat-${index + 1}`,
        `Remote chat ${index + 1}`,
        `2026-06-05T${String(16 - index).padStart(2, "0")}:00:00Z`,
        `2026-06-05T${String(16 - index).padStart(2, "0")}:05:00Z`,
      ),
    );
    appTestState.workspaceResponseWorkspaces = [{ ...workspace }, remoteWorkspace];
    appTestState.workspaceChatsByWorkspaceId = {
      [remoteWorkspace.id]: remoteChats,
    };
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    await userEvent.click(
      await within(workspaceList).findByRole("button", {
        name: (accessibleName, element) =>
          element.hasAttribute("aria-expanded") && accessibleName.startsWith("Remote project"),
      }),
    );
    const firstRemoteChat = await within(workspaceList).findByText("Remote chat 1");
    expect(within(workspaceList).queryByText("Remote chat 6")).not.toBeInTheDocument();
    const firstRemoteChatButton = firstRemoteChat.closest("button");
    if (!firstRemoteChatButton) {
      throw new Error("Expected remote chat history item button");
    }

    fireEvent.contextMenu(firstRemoteChatButton);
    const chatMenu = await screen.findByRole("menu", { name: "Remote chat 1" });
    await userEvent.click(
      within(chatMenu).getByRole("menuitem", { name: "Delete chat" }),
    );
    const dialog = await screen.findByRole("dialog", { name: "Delete this chat?" });
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Confirm delete chat" }),
    );

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-remote/chats/remote-chat-1/delete",
        expect.objectContaining({ method: "POST" }),
      );
    });
    expect(await within(workspaceList).findByText("Remote chat 6")).toBeInTheDocument();
    expect(within(workspaceList).queryByText("Remote chat 1")).not.toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(
        ([url]) => typeof url === "string" && url.startsWith("/api/workspaces/workspace-remote/chats?"),
      ).length,
    ).toBeGreaterThanOrEqual(2);
  });

  it("adds a workspace with a selectable slash-style path", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    // Establish a prior active chat so regression can detect leftover chatId.
    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyTitle = await within(workspaceList).findByText("Tool run");
    const historyButton = historyTitle.closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }
    await userEvent.click(historyButton);
    await screen.findByText("Please inspect README.");
    expect(window.location.href).toContain("chat-1");

    const fetchCallsBeforeAdd = fetchMock.mock.calls.length;

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

    const picker = await screen.findByRole("dialog", { name: "Select workspace folder" });
    await userEvent.click(within(picker).getByRole("button", { name: /NewWorkspace/ }));
    await userEvent.click(within(picker).getByRole("button", { name: "Select" }));

    await waitFor(() => {
      expect(pathInput).toHaveValue("C:/Users/fonla/Documents/Repos/NewWorkspace");
      expect(nameInput).toHaveValue("NewWorkspace");
    });

    await userEvent.click(within(dialog).getByRole("button", { name: "Upload icon" }));
    const iconPicker = await screen.findByRole("dialog", { name: "Select workspace icon" });
    await userEvent.click(within(iconPicker).getByRole("button", { name: /workspace-logo\.png/ }));
    await userEvent.click(within(iconPicker).getByRole("button", { name: "Select" }));

    await waitFor(() => {
      expect(within(dialog).getByText("workspace-logo.png")).toBeInTheDocument();
    });

    await userEvent.click(
      within(dialog).getByRole("switch", { name: "Enable Project Spec" }),
    );
    await userEvent.click(within(dialog).getByRole("button", { name: "Add workspace" }));

    await waitFor(() => {
      const addWorkspaceCall = fetchMock.mock.calls.find(
        ([url, init]) => url === "/api/workspaces/add" && init?.method === "POST",
      );
      expect(addWorkspaceCall).toBeDefined();
      expect(JSON.parse(String(addWorkspaceCall?.[1]?.body))).toEqual(
        expect.objectContaining({
          contentBase64: expect.any(String),
          name: "NewWorkspace",
          path: "C:/Users/fonla/Documents/Repos/NewWorkspace",
        }),
      );
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

    // Prior chat messages must leave the main view (activeChatId cleared).
    expect(screen.queryByText("Please inspect README.")).not.toBeInTheDocument();
    expect(await screen.findByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("NewWorkspace"),
    })).toBeInTheDocument();

    // URL matches "new workspace + no active chat" (open tabs may remain in query).
    expect(window.location.pathname).toBe("/new-workspace");
    expect(window.location.pathname + window.location.search).not.toMatch(
      /\/new-workspace\/chat-1/,
    );
    expect(
      browserRouteFromPathname(window.location.pathname, window.location.search),
    ).toMatchObject({
      chatId: null,
      viewMode: "chat",
      workspaceId: "new-workspace",
    });

    // Must not pair the new workspace with the previous chat id.
    const callsAfterAdd = fetchMock.mock.calls.slice(fetchCallsBeforeAdd);
    const staleChatScopedCalls = callsAfterAdd.filter(([url, init]) => {
      if (typeof url !== "string") {
        return false;
      }
      if (url.includes("/api/workspaces/new-workspace/chats/chat-1")) {
        return true;
      }
      if (
        url === "/api/workspaces/new-workspace/context-usage" &&
        typeof init?.body === "string"
      ) {
        try {
          const body = JSON.parse(init.body) as { chatId?: string | null };
          return body.chatId === "chat-1";
        } catch {
          return false;
        }
      }
      return false;
    });
    expect(staleChatScopedCalls).toEqual([]);
  });

  it("adds a remote SSH workspace without reusing the previous chat id", async () => {
    const remoteServer = {
      id: "server-1",
      name: "dev-box",
      hostAlias: "dev-box",
      user: "fonla",
      port: 22,
      identityFile: null,
      authMethod: "key" as const,
      passwordConfigured: false,
      defaultRemoteRoot: "/home/fonla",
      focoCommand: null,
      terminalShell: null,
      connectTimeoutMs: 10000,
      status: "ready",
      lastError: null,
      lastKnownTarget: null,
      sidecarVersion: "0.1.8",
      sidecarInstallState: "available",
      workspaceCount: 0,
      lastCheckedAt: null,
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      remoteServers: [remoteServer],
    };

    const fetchMock = vi.mocked(fetch);
    renderApp();

    // Establish a prior active chat so regression can detect leftover chatId.
    const workspaceList = await screen.findByRole("navigation", {
      name: "Workspace list",
    });
    const historyTitle = await within(workspaceList).findByText("Tool run");
    const historyButton = historyTitle.closest("button");
    if (!historyButton) {
      throw new Error("Expected Tool run history item button");
    }
    await userEvent.click(historyButton);
    await screen.findByText("Please inspect README.");
    expect(window.location.href).toContain("chat-1");

    const fetchCallsBeforeAdd = fetchMock.mock.calls.length;

    await userEvent.click(await screen.findByRole("button", { name: "Add workspace" }));

    const dialog = await screen.findByRole("dialog", { name: "Add workspace" });
    await userEvent.click(within(dialog).getByRole("button", { name: "SSH" }));

    const serverSelect = within(dialog).getByRole("button", {
      name: /Remote Server|Select remote server|dev-box/,
    });
    await userEvent.click(serverSelect);
    const serverOption = await screen.findByRole("option", {
      name: /dev-box/,
    });
    await userEvent.click(serverOption);

    const nameInput = within(dialog).getByPlaceholderText("Workspace name");
    const pathInput = within(dialog).getByPlaceholderText("/home/name/workspace");
    await userEvent.clear(nameInput);
    await userEvent.type(nameInput, "Remote New");
    await userEvent.clear(pathInput);
    await userEvent.type(pathInput, "/home/fonla/repos/remote-new");

    await userEvent.click(within(dialog).getByRole("button", { name: "Add workspace" }));

    await waitFor(() => {
      const addWorkspaceCall = fetchMock.mock.calls.find(
        ([url, init]) => url === "/api/workspaces/add" && init?.method === "POST",
      );
      expect(addWorkspaceCall).toBeDefined();
      expect(JSON.parse(String(addWorkspaceCall?.[1]?.body))).toEqual(
        expect.objectContaining({
          name: "Remote New",
          path: "/home/fonla/repos/remote-new",
          remotePath: "/home/fonla/repos/remote-new",
          serverId: remoteServer.id,
        }),
      );
    });

    expect(screen.queryByRole("dialog", { name: "Add workspace" })).not.toBeInTheDocument();

    // Prior chat messages must leave the main view (activeChatId cleared).
    expect(screen.queryByText("Please inspect README.")).not.toBeInTheDocument();
    expect(await screen.findByRole("button", {
      name: (accessibleName, element) =>
        element.hasAttribute("aria-expanded") && accessibleName.startsWith("Remote New"),
    })).toBeInTheDocument();

    // URL matches "new remote workspace + no active chat".
    expect(window.location.pathname).toBe("/new-remote-workspace");
    expect(window.location.pathname + window.location.search).not.toMatch(
      /\/new-remote-workspace\/chat-1/,
    );
    expect(
      browserRouteFromPathname(window.location.pathname, window.location.search),
    ).toMatchObject({
      chatId: null,
      viewMode: "chat",
      workspaceId: "new-remote-workspace",
    });

    // Must not pair the new workspace with the previous chat id (especially context-usage).
    const callsAfterAdd = fetchMock.mock.calls.slice(fetchCallsBeforeAdd);
    const staleChatScopedCalls = callsAfterAdd.filter(([url, init]) => {
      if (typeof url !== "string") {
        return false;
      }
      if (url.includes("/api/workspaces/new-remote-workspace/chats/chat-1")) {
        return true;
      }
      if (
        url === "/api/workspaces/new-remote-workspace/context-usage" &&
        typeof init?.body === "string"
      ) {
        try {
          const body = JSON.parse(init.body) as { chatId?: string | null };
          return body.chatId === "chat-1";
        } catch {
          return false;
        }
      }
      return false;
    });
    expect(staleChatScopedCalls).toEqual([]);
  });

  it("localizes the add-workspace dialog in local mode for zh-CN", async () => {
    const zhSettings = {
      ...settings,
      general: { ...settings.general, language: "zh-CN" as const },
    };
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        return path === "/api/settings"
          ? jsonResponse(zhSettings)
          : mockFetch(input, init);
      }),
    );

    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "添加工作区" }));

    const dialog = await screen.findByRole("dialog", { name: "添加工作区" });
    expect(within(dialog).getByText("创建或注册本地文件夹。")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "本地" })).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "SSH" })).toBeInTheDocument();
    expect(within(dialog).getByText("名称")).toBeInTheDocument();
    expect(within(dialog).getByPlaceholderText("工作区名称")).toBeInTheDocument();
    expect(within(dialog).getByText("路径")).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", { name: "选择工作区路径" }),
    ).toBeInTheDocument();
    expect(within(dialog).getByText("启用项目 Spec")).toBeInTheDocument();
    expect(within(dialog).getByText("高级")).toBeInTheDocument();
    expect(within(dialog).getByText("工作区图标")).toBeInTheDocument();
    expect(within(dialog).getByText("文件夹图标")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "上传图标" })).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", { name: "关闭工作区弹窗" }),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", { name: "取消工作区弹窗" }),
    ).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "添加工作区" })).toBeInTheDocument();

    // Critical keys must not fall back to English.
    expect(within(dialog).queryByText("Create or register a local folder.")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Local")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Name")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Path")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Advanced")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Folder icon")).not.toBeInTheDocument();
  });

  it("localizes the add-workspace dialog SSH mode for zh-CN", async () => {
    const remoteServer = {
      id: "srv-lab",
      name: "Lab",
      hostAlias: "lab.example",
      user: "root",
      port: 22,
      identityFile: null,
      authMethod: "key" as const,
      passwordConfigured: false,
      defaultRemoteRoot: "/home/lab",
      focoCommand: null,
      terminalShell: null,
      connectTimeoutMs: 10000,
      status: "ready",
      lastError: null,
      lastKnownTarget: null,
      sidecarVersion: "0.1.8",
      sidecarInstallState: "available",
      workspaceCount: 0,
      lastCheckedAt: null,
    };
    const zhSettings = {
      ...settings,
      general: { ...settings.general, language: "zh-CN" as const },
      remoteServers: [remoteServer],
    };
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        return path === "/api/settings"
          ? jsonResponse(zhSettings)
          : mockFetch(input, init);
      }),
    );

    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "添加工作区" }));

    const dialog = await screen.findByRole("dialog", { name: "添加工作区" });
    await userEvent.click(within(dialog).getByRole("button", { name: "SSH" }));

    expect(within(dialog).getByText("注册 SSH 工作区。")).toBeInTheDocument();
    expect(within(dialog).getByText("远程服务器")).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", {
        name: (_accessibleName, element) =>
          element.getAttribute("aria-haspopup") === "listbox",
      }),
    ).toBeInTheDocument();
    expect(within(dialog).getByPlaceholderText("服务器名称")).toBeInTheDocument();
    expect(within(dialog).getByPlaceholderText("SSH 主机名/IP")).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", { name: "添加远程服务器" }),
    ).toBeInTheDocument();
    expect(within(dialog).getByText("远程路径")).toBeInTheDocument();
    expect(within(dialog).getByText("测试连接")).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", { name: "测试连接" }),
    ).toBeInTheDocument();
    expect(within(dialog).getByText("高级")).toBeInTheDocument();
    expect(within(dialog).getByText("远程工作区图标")).toBeInTheDocument();

    // Critical keys must not fall back to English.
    expect(within(dialog).queryByText("Register an SSH workspace.")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Remote Server")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Select remote server")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Remote path")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Test connection")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Remote workspace icon")).not.toBeInTheDocument();
  });

  it("shows a newly added workspace in settings without leaving the page", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(await screen.findByRole("button", { name: "Workspaces" }));

    const workspaceListHeading = await screen.findByText("Workspace list");
    const workspaceListSection = workspaceListHeading.closest("section");
    expect(workspaceListSection).not.toBeNull();
    await userEvent.click(
      within(workspaceListSection as HTMLElement).getByRole("button", {
        name: "Add workspace",
      }),
    );

    const dialog = await screen.findByRole("dialog", { name: "Add workspace" });
    await userEvent.type(
      within(dialog).getByPlaceholderText("Workspace name"),
      "New Workspace",
    );
    await userEvent.type(
      within(dialog).getByPlaceholderText("C:/Users/name/workspace"),
      "C:/Users/fonla/Documents/Repos/NewWorkspace",
    );
    await userEvent.click(within(dialog).getByRole("button", { name: "Add workspace" }));

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Add workspace" })).not.toBeInTheDocument();
    });
    expect(await screen.findByRole("button", { name: "Edit workspace New Workspace" })).toBeInTheDocument();
  });

  it("uploads and clears a workspace icon in workspace settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(await screen.findByRole("button", { name: "Workspaces" }));
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
    await userEvent.click(await screen.findByRole("button", { name: "Workspaces" }));
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

  it("persists remote workspace Project Spec enablement after reopening settings", async () => {
    const fetchMock = vi.mocked(fetch);
    const remoteWorkspace = configureRemoteWorkspaceSpec();
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(await screen.findByRole("button", { name: "Workspaces" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Edit workspace Remote project" }),
    );

    const specCheckbox = await screen.findByRole("checkbox", {
      name: "Enable Project Spec",
    });
    await waitFor(() => expect(specCheckbox).toBeEnabled());
    expect(specCheckbox).not.toBeChecked();
    expect(
      fetchMock.mock.calls.some(
        ([url, init]) =>
          url === `/api/workspaces/${remoteWorkspace.id}/spec` && !init?.method,
      ),
    ).toBe(true);

    await userEvent.click(specCheckbox);
    await userEvent.click(screen.getByRole("button", { name: "Save workspace" }));

    await waitFor(() => {
      expect(
        screen.queryByRole("form", { name: "Workspace configuration" }),
      ).not.toBeInTheDocument();
    });

    const manualSaveIndex = fetchMock.mock.calls.findIndex(
      ([url, init]) => url === "/api/workspaces/manual" && init?.method === "POST",
    );
    const specSaveIndex = fetchMock.mock.calls.findIndex(
      ([url, init]) =>
        url === `/api/workspaces/${remoteWorkspace.id}/spec/settings` &&
        init?.method === "PUT",
    );
    expect(manualSaveIndex).toBeGreaterThanOrEqual(0);
    expect(specSaveIndex).toBeGreaterThan(manualSaveIndex);
    expect(
      JSON.parse(String(fetchMock.mock.calls[manualSaveIndex]?.[1]?.body)),
    ).toEqual(
      expect.objectContaining({
        id: remoteWorkspace.id,
        path: remoteWorkspace.remotePath,
        remotePath: remoteWorkspace.remotePath,
        serverId: remoteWorkspace.serverId,
      }),
    );

    // Refresh workspaces/settings after save (same as page reload).
    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(await screen.findByRole("button", { name: "Workspaces" }));

    await userEvent.click(
      await screen.findByRole("button", { name: "Edit workspace Remote project" }),
    );
    const reopenedSpecCheckbox = await screen.findByRole("checkbox", {
      name: "Enable Project Spec",
    });
    await waitFor(() => expect(reopenedSpecCheckbox).toBeEnabled());
    expect(reopenedSpecCheckbox).toBeChecked();
    expect(
      fetchMock.mock.calls.filter(
        ([url, init]) =>
          url === `/api/workspaces/${remoteWorkspace.id}/spec` && !init?.method,
      ),
    ).toHaveLength(2);
  });

  it("keeps remote workspace settings open when Project Spec saving fails", async () => {
    configureRemoteWorkspaceSpec();
    appTestState.workspaceSpecSettingsResponses.push(
      jsonResponse({ error: "Remote Project Spec settings could not be saved" }, { status: 502 }),
    );
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(await screen.findByRole("button", { name: "Workspaces" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Edit workspace Remote project" }),
    );

    const specCheckbox = await screen.findByRole("checkbox", {
      name: "Enable Project Spec",
    });
    await waitFor(() => expect(specCheckbox).toBeEnabled());
    await userEvent.click(specCheckbox);
    await userEvent.click(screen.getByRole("button", { name: "Save workspace" }));

    const workspaceForm = await screen.findByRole("form", {
      name: "Workspace configuration",
    });
    expect(
      await within(workspaceForm).findByRole("alert"),
    ).toHaveTextContent("Remote Project Spec settings could not be saved");
    expect(
      within(workspaceForm).getByRole("checkbox", { name: "Enable Project Spec" }),
    ).toBeChecked();
    await waitFor(() =>
      expect(
        within(workspaceForm).getByRole("button", { name: "Save workspace" }),
      ).toBeEnabled(),
    );
  });

  it("renders local workspaces without waiting for remote chat hydration", async () => {
    const remoteWorkspace = remoteWorkspaceFixture();
    appTestState.workspaceResponseWorkspaces = [
      {
        ...workspace,
        chats: workspace.chats,
        chatPagination: workspace.chatPagination,
      },
      {
        ...remoteWorkspace,
        chats: [],
        chatPagination: {
          hasMore: false,
          limit: 5,
          nextCursor: null,
          total: 0,
        },
      },
    ];
    const remoteChatsGate = deferred<Response>();
    appTestState.workspaceChatsResponsesByWorkspaceId = {
      [remoteWorkspace.id]: [remoteChatsGate.promise],
    };

    renderApp();

    const workspaceList = await screen.findByRole("navigation", { name: "Workspace list" });
    expect(await within(workspaceList).findByText("Tool run")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh workspaces" })).toBeEnabled();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(within(workspaceList).queryByText("Remote chat 1")).not.toBeInTheDocument();

    remoteChatsGate.resolve(
      jsonResponse({
        chats: [
          chatSummary(
            "remote-chat-1",
            "Remote chat 1",
            "2026-06-05T16:00:00Z",
            "2026-06-05T16:05:00Z",
          ),
        ],
        hasMore: false,
        limit: 5,
        nextCursor: null,
        total: 1,
      }),
    );

    await userEvent.click(
      await within(workspaceList).findByRole("button", {
        name: (accessibleName, element) =>
          element.hasAttribute("aria-expanded") && accessibleName.startsWith("Remote project"),
      }),
    );
    expect(await within(workspaceList).findByText("Remote chat 1")).toBeInTheDocument();
  });

  it("isolates remote chat hydration failures from global loading and errors", async () => {
    const remoteWorkspace = {
      ...remoteWorkspaceFixture(),
      chats: [],
      chatPagination: {
        hasMore: false,
        limit: 5,
        nextCursor: null,
        total: 0,
      },
    };
    appTestState.workspaceResponseWorkspaces = [{ ...workspace }, remoteWorkspace];
    appTestState.workspaceChatsResponsesByWorkspaceId = {
      [remoteWorkspace.id]: [
        jsonResponse({ error: "SSH connection timed out" }, { status: 502 }),
      ],
    };

    renderApp();

    const workspaceList = await screen.findByRole("navigation", { name: "Workspace list" });
    expect(await within(workspaceList).findByText("Tool run")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Refresh workspaces" })).toBeEnabled();
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(within(workspaceList).queryByText("Remote chat 1")).not.toBeInTheDocument();
  });

  it("keeps remote off-page chat tabs while remote chats are still unknown", async () => {
    const remoteWorkspace = {
      ...remoteWorkspaceFixture(),
      chats: [],
      chatPagination: {
        hasMore: false,
        limit: 5,
        nextCursor: null,
        total: 0,
      },
    };
    appTestState.workspaceResponseWorkspaces = [{ ...workspace }, remoteWorkspace];
    appTestState.workspaceChatsByWorkspaceId = {
      [remoteWorkspace.id]: [
        chatSummary(
          "remote-offpage-chat",
          "Remote off-page chat",
          "2026-06-05T09:00:00Z",
          "2026-06-05T09:05:00Z",
        ),
      ],
    };
    appTestState.chatMessagesResponsesByChatKey = {
      ...appTestState.chatMessagesResponsesByChatKey,
      [`${remoteWorkspace.id}/remote-offpage-chat`]: {
        chat: {
          id: "remote-offpage-chat",
          kind: null,
          readOnly: false,
          title: "Remote off-page chat",
        },
        pagination: { hasMoreBefore: false, nextBeforeSequence: null },
        messages: [
          {
            content: "Remote off-page answer.",
            createdAt: "2026-06-05T09:01:00Z",
            extractedMemories: [],
            id: "remote-offpage-message",
            memoriesUsed: [],
            metrics: null,
            parts: [{ text: "Remote off-page answer.", type: "text" }],
            reasoning: null,
            role: "assistant",
            toolCalls: [],
          },
        ],
      },
    };
    const remoteChatsGate = deferred<Response>();
    appTestState.workspaceChatsResponsesByWorkspaceId = {
      [remoteWorkspace.id]: [remoteChatsGate.promise],
    };

    window.history.replaceState(
      null,
      "",
      `/?tab=${encodeURIComponent(`${remoteWorkspace.id}/remote-offpage-chat`)}`,
    );

    renderApp();

    await screen.findByRole("navigation", { name: "Workspace list" });
    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(
      within(tabList).getByRole("tab", { name: /Chat|Remote off-page chat/ }),
    ).toBeInTheDocument();
    expect(currentChatTabsFromLocation()).toContain(
      `${remoteWorkspace.id}/remote-offpage-chat`,
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    remoteChatsGate.resolve(
      jsonResponse({
        chats: [
          chatSummary(
            "remote-offpage-chat",
            "Remote off-page chat",
            "2026-06-05T09:00:00Z",
            "2026-06-05T09:05:00Z",
          ),
        ],
        hasMore: false,
        limit: 5,
        nextCursor: null,
        total: 1,
      }),
    );

    await waitFor(() => {
      expect(
        within(tabList).getByRole("tab", { name: /Remote off-page chat/ }),
      ).toBeInTheDocument();
    });
  });

  it("ignores stale remote chat hydration after a newer refresh generation", async () => {
    const remoteWorkspace = {
      ...remoteWorkspaceFixture(),
      chats: [],
      chatPagination: {
        hasMore: false,
        limit: 5,
        nextCursor: null,
        total: 0,
      },
    };
    appTestState.workspaceResponseWorkspaces = [{ ...workspace }, remoteWorkspace];

    const firstRemotePage = deferred<Response>();
    const secondRemotePage = deferred<Response>();
    appTestState.workspaceChatsResponsesByWorkspaceId = {
      [remoteWorkspace.id]: [firstRemotePage.promise, secondRemotePage.promise],
    };

    renderApp();

    const workspaceList = await screen.findByRole("navigation", { name: "Workspace list" });
    expect(await within(workspaceList).findByText("Tool run")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Refresh workspaces" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Refresh workspaces" })).toBeEnabled();
    });

    secondRemotePage.resolve(
      jsonResponse({
        chats: [
          chatSummary(
            "remote-chat-fresh",
            "Fresh remote chat",
            "2026-06-06T10:00:00Z",
            "2026-06-06T10:05:00Z",
          ),
        ],
        hasMore: false,
        limit: 5,
        nextCursor: null,
        total: 1,
      }),
    );

    await userEvent.click(
      await within(workspaceList).findByRole("button", {
        name: (accessibleName, element) =>
          element.hasAttribute("aria-expanded") && accessibleName.startsWith("Remote project"),
      }),
    );
    expect(await within(workspaceList).findByText("Fresh remote chat")).toBeInTheDocument();

    firstRemotePage.resolve(
      jsonResponse({
        chats: [
          chatSummary(
            "remote-chat-stale",
            "Stale remote chat",
            "2026-06-05T10:00:00Z",
            "2026-06-05T10:05:00Z",
          ),
        ],
        hasMore: false,
        limit: 5,
        nextCursor: null,
        total: 1,
      }),
    );

    await act(async () => {
      await Promise.resolve();
    });
    expect(within(workspaceList).queryByText("Stale remote chat")).not.toBeInTheDocument();
    expect(within(workspaceList).getByText("Fresh remote chat")).toBeInTheDocument();
  });

  it("does not auto-hydrate offline remote workspaces during refresh", async () => {
    const offlineRemote = {
      ...remoteWorkspaceFixture(),
      connectionStatus: "offline",
      chats: [],
      chatPagination: {
        hasMore: false,
        limit: 5,
        nextCursor: null,
        total: 0,
      },
    };
    appTestState.workspaceResponseWorkspaces = [{ ...workspace }, offlineRemote];
    const fetchMock = vi.mocked(fetch);

    renderApp();

    await screen.findByRole("navigation", { name: "Workspace list" });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Refresh workspaces" })).toBeEnabled();
    });

    expect(
      fetchMock.mock.calls.filter(
        ([url]) =>
          typeof url === "string" &&
          url.startsWith(`/api/workspaces/${offlineRemote.id}/chats?`),
      ),
    ).toHaveLength(0);
  });

});

function currentChatTabsFromLocation() {
  return new URLSearchParams(window.location.search).getAll("tab");
}
