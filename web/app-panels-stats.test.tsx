import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  activeMemory,
  agentTeamSnapshot,
  aiStatistics,
  aiStatisticsDetail,
  appTestState,
  changeInput,
  defaultComposerPlaceholder,
  type Deferred,
  chatMemory,
  chatMessages,
  deferred,
  enqueueChatStreamEvent,
  enqueueChatStreamEventForRun,
  generatedGitDiff,
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
  workspaceFilesResponse,
  workspaceMemory,
  workspaceSpec,
  workspaceSpecQueuedJob,
} from "./test-utils/app-test-harness";

describe("app-panels-stats verification surfaces", () => {
  beforeEach(resetAppTestEnvironment);

  function aiStatisticsCallUrls() {
    const fetchMock = vi.mocked(fetch);
    return fetchMock.mock.calls
      .map((call) => {
        const rawPath =
          typeof call[0] === "string"
            ? call[0]
            : call[0] instanceof URL
              ? call[0].toString()
              : call[0].url;

        return new URL(rawPath, "http://localhost");
      })
      .filter((url) => url.pathname === "/api/ai-statistics");
  }

  function setDocumentVisibility(visibilityState: DocumentVisibilityState) {
    Object.defineProperty(document, "hidden", {
      configurable: true,
      value: visibilityState === "hidden",
    });
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: visibilityState,
    });
  }

  function latestStreamChatId() {
    const streamCall = vi
      .mocked(fetch)
      .mock.calls.findLast(([input]) => {
        const rawPath =
          typeof input === "string"
            ? input
            : input instanceof URL
              ? input.toString()
              : input.url;
        return new URL(rawPath, "http://localhost").pathname.endsWith(
          "/chat/stream",
        );
      });
    const body = JSON.parse(String(streamCall?.[1]?.body ?? "{}")) as {
      chatId?: string | null;
    };
    return body.chatId ?? "chat-1";
  }

  it("shows git file names before muted directories in the diff panel", async () => {
    appTestState.workspaceGitDiffResponse = generatedGitDiff;

    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Git" }));

    const appRow = await screen.findByRole("button", { name: /web\/App\.tsx M/ });
    const appFileName = within(appRow).getByText("App.tsx");
    const appDirectory = within(appRow).getByText("web");

    expect(appFileName.compareDocumentPosition(appDirectory)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(appFileName).toHaveClass("text-stone-900");
    expect(appDirectory).toHaveClass("text-stone-400");
    expect(within(appRow).queryByText("web/App.tsx")).not.toBeInTheDocument();
  });

  it("toggles the context panel and opens the terminal panel for the active workspace", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("button", { name: "Close context panel" }));
    expect(screen.queryByRole("tab", { name: "ToDo" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Open context panel" }));
    await userEvent.click(screen.getByRole("tab", { name: "Git" }));

    expect(await screen.findAllByRole("button", { name: /README\.md M/ })).toHaveLength(2);
    expect(screen.queryByText(/hello world/)).not.toBeInTheDocument();

    await userEvent.click(screen.getAllByRole("button", { name: /README\.md M/ })[0]);

    expect((await screen.findAllByText(/hello world/))[0]).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /new-note\.txt U/ })).toHaveLength(2);

    await userEvent.click(screen.getByRole("button", { name: "Open terminal" }));

    expect(await screen.findByText("connected")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/workspaces/workspace-1/terminal/session",
      expect.objectContaining({ method: "POST" }),
    );

    await userEvent.click(screen.getByRole("button", { name: "New terminal" }));

    const terminalList = await screen.findByRole("complementary", {
      name: "Terminal sessions",
    });
    expect(within(terminalList).getByText("Terminal 1")).toBeInTheDocument();
    expect(within(terminalList).getByText("Terminal 2")).toBeInTheDocument();
    expect(within(terminalList).getAllByLabelText("connected")).toHaveLength(2);
    expect(within(terminalList).getAllByText(workspace.path)[0]).toHaveAttribute(
      "title",
      workspace.path,
    );
    expect(
      fetchMock.mock.calls.filter(
        ([url]) => url === "/api/workspaces/workspace-1/terminal/session",
      ),
    ).toHaveLength(2);

    await userEvent.click(
      within(terminalList).getByRole("button", { name: /Terminal 1/ }),
    );
    expect(within(terminalList).getByText("Terminal 1")).toBeInTheDocument();

    await userEvent.click(
      within(terminalList).getByRole("button", { name: "Close terminal 2" }),
    );
    expect(within(terminalList).queryByText("Terminal 2")).not.toBeInTheDocument();
    expect(screen.queryByRole("complementary", { name: "Terminal sessions" })).not.toBeInTheDocument();

    await userEvent.click(screen.getAllByRole("button", { name: "Close terminal" })[1]);

    await waitFor(() => {
      expect(screen.queryByText("connected")).not.toBeInTheDocument();
    });
  }, 10000);

  async function openSpecPanel() {
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Spec" }));
    return screen.findAllByRole("heading", { name: "Project Spec" });
  }

  it("marks right panel refresh icons as loading after refresh clicks", async () => {
    const fetchMock = vi.mocked(fetch);
    const heldRequests = {
      agent: [] as Deferred<Response>[],
      diff: [] as Deferred<Response>[],
      files: [] as Deferred<Response>[],
      spec: [] as Deferred<Response>[],
    };
    const holdNextRequest = {
      agent: false,
      diff: false,
      files: false,
      spec: false,
    };

    fetchMock.mockImplementation((input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/files" && holdNextRequest.files) {
        const request = deferred<Response>();
        heldRequests.files.push(request);
        return request.promise;
      }

      if (path === "/api/workspaces/workspace-1/git/diff" && holdNextRequest.diff) {
        const request = deferred<Response>();
        heldRequests.diff.push(request);
        return request.promise;
      }

      if (path === "/api/workspaces/workspace-1/spec" && holdNextRequest.spec) {
        const request = deferred<Response>();
        heldRequests.spec.push(request);
        return request.promise;
      }

      if (
        path === "/api/workspaces/workspace-1/chats/chat-1/agent-team" &&
        holdNextRequest.agent
      ) {
        const request = deferred<Response>();
        heldRequests.agent.push(request);
        return request.promise;
      }

      return mockFetch(input, init);
    });

    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    const expectRefreshIconLoading = async (buttonName: string) => {
      const button = screen.getByRole("button", { name: buttonName });
      await waitFor(() => expect(button).toBeDisabled());
      const icon = button.querySelector("svg");
      if (!(icon instanceof SVGElement)) {
        throw new Error(`${buttonName} refresh icon was not rendered`);
      }
      expect(icon).toHaveClass("lucide-refresh-cw");
      expect(icon).toHaveClass("context-refresh-icon");
      expect(icon).toHaveAttribute("data-loading", "true");
    };

    await screen.findAllByText("Default");

    await userEvent.click(screen.getByRole("tab", { name: "Files" }));
    await screen.findByText("Workspace file tree");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Refresh files" })).not.toBeDisabled(),
    );
    holdNextRequest.files = true;
    await userEvent.click(screen.getByRole("button", { name: "Refresh files" }));
    await waitFor(() => expect(heldRequests.files).toHaveLength(1));
    await expectRefreshIconLoading("Refresh files");
    await act(async () => {
      heldRequests.files[0]?.resolve(jsonResponse(workspaceFilesResponse));
    });

    await userEvent.click(screen.getByRole("tab", { name: "Git" }));
    await screen.findByText("Source Control");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Refresh diff" })).not.toBeDisabled(),
    );
    holdNextRequest.diff = true;
    await userEvent.click(screen.getByRole("button", { name: "Refresh diff" }));
    await waitFor(() => expect(heldRequests.diff).toHaveLength(1));
    await expectRefreshIconLoading("Refresh diff");
    await act(async () => {
      heldRequests.diff[0]?.resolve(jsonResponse(appTestState.workspaceGitDiffResponse));
    });

    await userEvent.click(screen.getByRole("tab", { name: "Spec" }));
    await screen.findAllByRole("heading", { name: "Project Spec" });
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Reload spec" })).not.toBeDisabled(),
    );
    holdNextRequest.spec = true;
    await userEvent.click(screen.getByRole("button", { name: "Reload spec" }));
    await waitFor(() => expect(heldRequests.spec).toHaveLength(1));
    await expectRefreshIconLoading("Reload spec");
    await act(async () => {
      heldRequests.spec[0]?.resolve(jsonResponse(appTestState.workspaceSpecResponse));
    });

    await userEvent.click(screen.getByRole("tab", { name: "Agents" }));
    const agentRefreshButton = await screen.findByRole("button", { name: "Refresh" });
    await waitFor(() => expect(agentRefreshButton).not.toBeDisabled());
    holdNextRequest.agent = true;
    await userEvent.click(agentRefreshButton);
    await waitFor(() => expect(heldRequests.agent).toHaveLength(1));
    await expectRefreshIconLoading("Refresh");
    await act(async () => {
      heldRequests.agent[0]?.resolve(jsonResponse(agentTeamSnapshot));
    });
  });
  it("loads the Project Spec tab in the right panel with markdown preview enabled", async () => {
    await openSpecPanel();

    expect(screen.getByRole("button", { name: "Edit markdown" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("heading", { name: "Purpose" })).toBeInTheDocument();
    expect(screen.getByText("Describe the current workspace.")).toBeInTheDocument();
    expect(screen.queryByLabelText("Project Spec Markdown")).toBeNull();
    expect(screen.getAllByText(/Revision 3/).length).toBeGreaterThan(0);
    expect(screen.getByText(/Latest job: Completed/)).toBeInTheDocument();
  });

  it("localizes the Project Spec tab in the right panel", async () => {
    const zhSettings = {
      ...settings,
      general: { ...settings.general, language: "zh-CN" },
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

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Spec" }));

    expect(await screen.findByRole("heading", { name: "项目 Spec" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "编辑 Markdown" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("heading", { name: "Purpose" })).toBeInTheDocument();
    expect(screen.queryByLabelText("项目 Spec Markdown")).toBeNull();
    expect(screen.queryByRole("checkbox", { name: "启用项目 Spec" })).toBeNull();
    expect(screen.getByRole("button", { name: "注入新会话" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getAllByText(/修订 3/).length).toBeGreaterThan(0);
    expect(screen.getByText(/最近任务: 已完成 · 手动刷新/)).toBeInTheDocument();
  });

  it("toggles Project Spec chat injection from the right panel", async () => {
    const fetchMock = vi.mocked(fetch);
    appTestState.workspaceSpecResponse = {
      ...workspaceSpec,
      settings: { enabled: true, injectEnabled: false },
    };

    await openSpecPanel();

    await userEvent.click(screen.getByRole("button", { name: "Inject into new chats" }));
    await waitFor(() => {
      const call = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/spec/settings",
      );
      expect(call).toBeDefined();
      expect(JSON.parse(String(call?.[1]?.body))).toEqual({
        enabled: true,
        injectEnabled: true,
      });
    });

    await userEvent.click(screen.getByRole("button", { name: "Inject into new chats" }));
    await waitFor(() => {
      const calls = fetchMock.mock.calls.filter(
        ([url]) => url === "/api/workspaces/workspace-1/spec/settings",
      );
      expect(JSON.parse(String(calls.at(-1)?.[1]?.body))).toEqual({
        enabled: true,
        injectEnabled: false,
      });
    });
  });

  it("saves Project Spec Markdown with the current revision", async () => {
    const fetchMock = vi.mocked(fetch);
    await openSpecPanel();
    await userEvent.click(screen.getByRole("button", { name: "Edit markdown" }));
    changeInput(
      screen.getByLabelText("Project Spec Markdown"),
      "# Project Spec\n\n## Purpose\n\nUpdated from the right panel.",
    );
    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      const call = fetchMock.mock.calls.find(
        ([url, init]) =>
          url === "/api/workspaces/workspace-1/spec" && init?.method === "PUT",
      );
      expect(call).toBeDefined();
      expect(JSON.parse(String(call?.[1]?.body))).toEqual({
        contentMarkdown: "# Project Spec\n\n## Purpose\n\nUpdated from the right panel.",
        expectedRevision: 3,
      });
    });
    expect((await screen.findAllByText(/Revision 4/)).length).toBeGreaterThan(0);
  });

  it("queues Project Spec generation from the right panel", async () => {
    const fetchMock = vi.mocked(fetch);
    await openSpecPanel();

    await userEvent.click(screen.getByRole("button", { name: "Regenerate spec" }));

    await waitFor(() => {
      const call = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/spec/generate",
      );
      expect(call).toBeDefined();
      expect(JSON.parse(String(call?.[1]?.body))).toEqual({ modelId: null });
    });
    expect(await screen.findByText(new RegExp(workspaceSpecQueuedJob.id))).toBeInTheDocument();
    expect(screen.getByText(/Latest job: Queued/)).toBeInTheDocument();
  });

  it("auto-reloads Project Spec content after generation completes", async () => {
    appTestState.workspaceSpecGenerateCompletes = true;
    await openSpecPanel();

    await userEvent.click(screen.getByRole("button", { name: "Regenerate spec" }));

    await waitFor(
      () => {
        expect(screen.getByText("Regenerated by the LLM.")).toBeInTheDocument();
      },
      { timeout: 5000 },
    );
  });

  it("shows Project Spec save conflicts with a reload action", async () => {
    appTestState.workspaceSpecSaveConflict = true;
    await openSpecPanel();
    await userEvent.click(screen.getByRole("button", { name: "Edit markdown" }));
    changeInput(
      screen.getByLabelText("Project Spec Markdown"),
      "# Project Spec\n\n## Purpose\n\nConflicting edit.",
    );
    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(
      await screen.findByText("workspace spec revision changed; reload and retry"),
    ).toBeInTheDocument();
    await userEvent.click(screen.getAllByRole("button", { name: "Reload spec" })[1]);
    await waitFor(() => {
      expect(screen.getByText("Describe the current workspace.")).toBeInTheDocument();
      expect(screen.queryByLabelText("Project Spec Markdown")).toBeNull();
    });
  });

  it("keeps workspace terminals mounted while switching workspaces", async () => {
    const fetchMock = vi.mocked(fetch);
    const closeSpy = vi.spyOn(window.WebSocket.prototype, "close");

    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("button", { name: "Open terminal" }));
    expect(await screen.findByText("connected")).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(
        ([url]) => url === "/api/workspaces/workspace-1/terminal/session",
      ),
    ).toHaveLength(1);

    await userEvent.click(screen.getByRole("button", { name: "Side project" }));
    await userEvent.click(screen.getByRole("button", { name: /Side note/ }));
    expect(screen.getByRole("button", { name: "Open terminal" })).toBeInTheDocument();
    expect(closeSpy).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Default" }));
    await userEvent.click(screen.getByRole("button", { name: /Tool run/ }));
    expect(screen.getAllByRole("button", { name: "Close terminal" })).toHaveLength(2);
    expect(closeSpy).not.toHaveBeenCalled();
    expect(
      fetchMock.mock.calls.filter(
        ([url]) => url === "/api/workspaces/workspace-1/terminal/session",
      ),
    ).toHaveLength(1);
  });

  it("runs a workspace common command in the active terminal", async () => {
    const commandWorkspace = {
      ...workspace,
      commonCommands: [{ command: "npm run dev", name: "Dev" }],
    };
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation((input, init) => {
      const path =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;

      if (path === "/api/workspaces") {
        return Promise.resolve(jsonResponse({
          activeWorkspaceId: commandWorkspace.id,
          workspaces: [commandWorkspace, secondaryWorkspace],
        }));
      }

      if (path === "/api/settings") {
        return Promise.resolve(jsonResponse({
          ...settings,
          workspaces: [
            {
              ...settings.workspaces[0],
              commonCommands: commandWorkspace.commonCommands,
            },
          ],
        }));
      }

      return Promise.resolve(mockFetch(input, init));
    });
    const sendSpy = vi.spyOn(window.WebSocket.prototype, "send");

    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("button", { name: "Open terminal" }));
    expect(await screen.findByText("connected")).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "Run common command Dev" }),
    );

    await waitFor(() => {
      const sentInput = sendSpy.mock.calls
        .map(([data]) => JSON.parse(String(data)) as { data?: string; type: string })
        .find(
          (message) =>
            message.type === "input" && message.data?.includes("npm run dev"),
        );

      expect(sentInput?.data).toBe(
        `Set-Location -LiteralPath '${commandWorkspace.path}'\rnpm run dev\r`,
      );
    });
  });

  it("keeps todo graph and git diff in separate context tabs", async () => {
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await userEvent.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "plan");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());
    await act(async () => {
      await Promise.resolve();
    });

    await act(async () => {
      enqueueChatStreamEvent({
        chatId: latestStreamChatId(),
        type: "todoGraphRefresh",
        workspaceId: "workspace-1",
      });
    });

    const todoTaskButton = await screen.findByRole("button", {
      name: /task-1[\s\S]*Inspect workspace changes/,
    });
    expect(todoTaskButton).toBeInTheDocument();
    const contextPanel = todoTaskButton.closest(".context-panel") as HTMLElement;
    await userEvent.click(todoTaskButton);
    expect(await screen.findByText("README.md diff is visible")).toBeInTheDocument();
    expect(within(contextPanel).queryByText(/hello world/)).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("tab", { name: "Git" }));

    expect(screen.getByText("Source Control")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /README\.md M/ })).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: /new-note\.txt U/ })).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: /asset\.bin M/ }).length).toBeGreaterThan(0);
    expect(within(contextPanel).queryByText(/hello world/)).not.toBeInTheDocument();

    await userEvent.click(screen.getAllByRole("button", { name: /README\.md M/ })[0]);

    const inlineDiffLine = (await within(contextPanel).findAllByText(/hello world/))[0];
    expect(inlineDiffLine).toBeInTheDocument();
    const inlineDiffScrollRegion = inlineDiffLine.closest(
      ".panel-scroll",
    ) as HTMLElement | null;
    expect(inlineDiffScrollRegion).not.toBeNull();
    expect(inlineDiffScrollRegion).toHaveClass("overflow-auto");
    expect(inlineDiffScrollRegion?.className).toContain(
      "max-h-[min(30rem,52dvh)]",
    );
    expect(inlineDiffLine.closest(".overflow-y-auto")).toHaveClass(
      "panel-scroll",
    );
    expect(within(contextPanel).queryByText("Inspect workspace changes")).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("shows an inline diff notice for binary changed files", async () => {
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Git" }));
    await userEvent.click(await screen.findByRole("button", { name: /asset\.bin M/ }));

    expect(
      await screen.findByText("Inline diff is unavailable for binary or non-text files."),
    ).toBeInTheDocument();
    expect(screen.queryByText("Binary files a/asset.bin and b/asset.bin differ")).not.toBeInTheDocument();
  });

  it("deletes memories from the right panel memory tab", async () => {
    const fetchMock = vi.mocked(fetch);
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);

    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Memory" }));

    const globalItem = (await screen.findByText(activeMemory.fact)).closest("article");
    const workspaceItem = (await screen.findByText(workspaceMemory.fact)).closest("article");
    expect(globalItem).not.toBeNull();
    expect(workspaceItem).not.toBeNull();

    await userEvent.click(
      within(globalItem!).getByRole("button", { name: "Delete memory" }),
    );

    await waitFor(() => {
      expect(confirmSpy).toHaveBeenCalledWith("Delete memory confirmation");
      const forgetCall = fetchMock.mock.calls.find(([url, init]) => {
        if (url !== "/api/memory/forget") {
          return false;
        }

        return JSON.parse(String(init?.body)).memoryId === activeMemory.id;
      });
      expect(forgetCall).toBeDefined();
      expect(JSON.parse(String(forgetCall?.[1]?.body))).toEqual({
        memoryId: activeMemory.id,
        scope: "global",
        workspaceId: null,
      });
    });

    await userEvent.click(
      within(workspaceItem!).getByRole("button", { name: "Delete memory" }),
    );

    await waitFor(() => {
      const forgetCall = fetchMock.mock.calls.find(([url, init]) => {
        if (url !== "/api/memory/forget") {
          return false;
        }

        return JSON.parse(String(init?.body)).memoryId === workspaceMemory.id;
      });
      expect(forgetCall).toBeDefined();
      expect(JSON.parse(String(forgetCall?.[1]?.body))).toEqual({
        memoryId: workspaceMemory.id,
        scope: "workspace",
        workspaceId: workspace.id,
      });
    });

    confirmSpy.mockRestore();
  });

  it("shows active chat statistics in the right panel", async () => {
    const fetchMock = vi.mocked(fetch);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await userEvent.click(await screen.findByRole("tab", { name: "Stats" }));

    expect(await screen.findByText("Session statistics")).toBeInTheDocument();
    expect(screen.getByText("17.6K")).toBeInTheDocument();
    expect(
      within(screen.getByText("Memory refs").closest(".context-stat-metric")!)
        .getByText("3"),
    ).toBeInTheDocument();
    expect(
      within(screen.getByText("New memories").closest(".context-stat-metric")!)
        .getByText("2"),
    ).toBeInTheDocument();
    expect(screen.getByText("+12 / -3")).toBeInTheDocument();
    expect(
      within(screen.getByText("Model calls").parentElement!).getByText("gpt-test"),
    ).toBeInTheDocument();
    expect(
      within(screen.getByText("Tools and compression").parentElement!)
        .getByText("read_file"),
    ).toBeInTheDocument();
    expect(screen.getByText("Context usage unavailable.")).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.some(([url]) => url === "/api/workspaces/workspace-1/context-usage"),
    ).toBe(false);
  });

  it("updates active chat code change statistics from git diff refresh events", async () => {
    const user = userEvent.setup();
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await user.click(await screen.findByRole("tab", { name: "Stats" }));
    expect(await screen.findByText("+12 / -3")).toBeInTheDocument();

    await user.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "edit the file",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        codeChangeStats: { additions: 5, deletions: 1 },
        type: "gitDiffRefresh",
        workspaceId: "workspace-1",
      });
    });

    expect(await screen.findByText("+17 / -4")).toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("opens the todo graph sidebar when a todo graph refresh arrives", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];

      if (path === "/api/workspaces/workspace-1/chats/chat-1/todo-graph") {
        return jsonResponse({
          ...todoGraph,
          tasks: [
            ...todoGraph.tasks,
            {
              acceptance: [],
              createdAt: "2026-06-05T10:06:00Z",
              dependsOn: [],
              id: "task-2",
              status: "pending",
              subtasks: [],
              summary: "",
              title: "Wait for next step",
              updatedAt: "2026-06-05T10:06:00Z",
            },
          ],
        });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");
    renderApp();

    await userEvent.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "plan");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());
    await act(async () => {
      await Promise.resolve();
    });

    await act(async () => {
      enqueueChatStreamEvent({
        chatId: latestStreamChatId(),
        type: "todoGraphRefresh",
        workspaceId: "workspace-1",
      });
    });

    const todoGraphHeading = await screen.findByText("ToDo graph");
    const todoPanel = todoGraphHeading.closest(".context-panel") as HTMLElement;
    expect(todoGraphHeading).toBeInTheDocument();
    expect(within(todoPanel).getByText("Inspect workspace changes")).toBeInTheDocument();
    expect(within(todoPanel).getByText("running")).toHaveClass("bg-amber-100", "text-amber-800");
    expect(within(todoPanel).getByText("completed")).toHaveClass("bg-emerald-100", "text-emerald-800");
    expect(within(todoPanel).getByText("pending")).toHaveClass("bg-stone-100", "text-stone-600");
    expect(within(todoPanel).queryByText("Git diff")).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("does not keep a stale todo graph fetch error after a refresh succeeds", async () => {
    const todoGraphRequests: Deferred<Response>[] = [];
    const fetchMock = vi.fn(
      (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];

        if (path === "/api/workspaces/workspace-1/chats/chat-1/todo-graph") {
          const request = deferred<Response>();
          todoGraphRequests.push(request);
          return request.promise;
        }

        return mockFetch(input, init);
      },
    );
    vi.stubGlobal("fetch", fetchMock);
    window.history.replaceState(null, "", "/workspace-1/chat-1");

    renderApp();

    expect(await screen.findByText("Please inspect README.")).toBeInTheDocument();
    await waitFor(() => expect(todoGraphRequests).toHaveLength(1));

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "continue",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());
    await act(async () => {
      await Promise.resolve();
    });

    await act(async () => {
      enqueueChatStreamEvent({
        chatId: latestStreamChatId(),
        type: "todoGraphRefresh",
        workspaceId: "workspace-1",
      });
    });
    await waitFor(() => expect(todoGraphRequests).toHaveLength(2));

    await act(async () => {
      todoGraphRequests[0].reject(new TypeError("Failed to fetch"));
    });
    await act(async () => {
      todoGraphRequests[1].resolve(jsonResponse(todoGraph));
    });

    expect(await screen.findByText("Inspect workspace changes")).toBeInTheDocument();
    expect(screen.queryByText("Failed to fetch")).not.toBeInTheDocument();

    await act(async () => {
      appTestState.activeChatStreamController?.close();
    });
  });

  it("loads API overview for the active workspace first", async () => {
    renderApp();

    expect(await screen.findByText("API overview")).toBeInTheDocument();
    await waitFor(() =>
      expect(
        aiStatisticsCallUrls().some(
          (url) => url.searchParams.get("workspaceId") === workspace.id,
        ),
      ).toBe(true),
    );
    expect(
      aiStatisticsCallUrls().every(
        (url) => url.searchParams.get("workspaceId") === workspace.id,
      ),
    ).toBe(true);
  });

  it("shows AI statistics and request details", async () => {
    renderApp();

    expect(await screen.findByText("API overview")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getAllByText("17.6K").length).toBeGreaterThan(0),
    );
    expect(screen.queryByText("Workspace shell is ready")).not.toBeInTheDocument();

    await userEvent.click((await screen.findAllByRole("button", { name: "API details" }))[0]);

    expect(await screen.findByText("API details")).toBeInTheDocument();
    expect(screen.getByText("Request audit")).toBeInTheDocument();
    const table = screen.getByRole("table");
    expect(table.parentElement).toHaveClass("panel-scroll");
    expect(table.parentElement).toHaveClass("overflow-x-auto");
    expect(table.parentElement).not.toHaveClass("overflow-auto");
    expect(table.closest(".overflow-y-auto")).toHaveClass("panel-scroll");
    await waitFor(() =>
      expect(within(table).getByText("openai")).toBeInTheDocument(),
    );
    expect(within(table).getByText("gpt-test")).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Request audit pagination" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Go to page 2" })).toBeInTheDocument();
    expect(screen.getByLabelText("Page size")).toHaveValue(20);

    await userEvent.click(screen.getByText("Columns"));
    await userEvent.click(screen.getByRole("checkbox", { name: "Provider" }));
    expect(within(table).queryByText("openai")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "View request details" }));

    const dialog = await screen.findByRole("dialog", { name: "Request details" });
    expect(within(dialog).getByText("Request body")).toBeInTheDocument();
    expect(within(dialog).getByText("Response body")).toBeInTheDocument();
    const requestBodyBlock = within(dialog)
      .getByText("Request body")
      .closest(".audit-json-block");
    expect(requestBodyBlock).not.toBeNull();
    const requestBodyViewer = requestBodyBlock as HTMLElement;
    expect(requestBodyViewer).toHaveClass("audit-json-block");
    expect(within(requestBodyViewer).getByText('"messages"')).toHaveClass(
      "audit-json-token-key",
    );
    await userEvent.click(
      within(requestBodyViewer).getByRole("button", {
        name: "Collapse all Request body",
      }),
    );
    expect(within(requestBodyViewer).queryByText('"messages"')).not.toBeInTheDocument();
    await userEvent.click(
      within(requestBodyViewer).getByRole("button", {
        name: "Expand all Request body",
      }),
    );
    expect(within(requestBodyViewer).getByText('"messages"')).toHaveClass(
      "audit-json-token-key",
    );
    expect(within(dialog).queryByText("Stream events")).not.toBeInTheDocument();
    fireEvent.click(dialog);
    expect(
      screen.getByRole("dialog", { name: "Request details" }),
    ).toBeInTheDocument();
    fireEvent.click(dialog.parentElement as HTMLElement);
    expect(
      screen.queryByRole("dialog", { name: "Request details" }),
    ).not.toBeInTheDocument();
  });

  it("loads API details from the stats URL page", async () => {
    window.history.replaceState(null, "", "/stats?page=2");

    renderApp();

    expect(await screen.findByText("API details")).toBeInTheDocument();
    await waitFor(() =>
      expect(
        aiStatisticsCallUrls().some(
          (url) =>
            url.searchParams.get("page") === "2" &&
            url.searchParams.get("pageSize") === "20",
        ),
      ).toBe(true),
    );
    expect(screen.getByRole("button", { name: "Go to page 2" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("waits to load API details while the page is hidden", async () => {
    setDocumentVisibility("hidden");
    window.history.replaceState(null, "", "/stats?page=2");

    try {
      renderApp();

      expect(await screen.findByText("Request audit")).toBeInTheDocument();
      expect(aiStatisticsCallUrls()).toHaveLength(0);

      setDocumentVisibility("visible");
      fireEvent(document, new Event("visibilitychange"));

      await waitFor(() =>
        expect(
          aiStatisticsCallUrls().some(
            (url) =>
              url.searchParams.get("page") === "2" &&
              url.searchParams.get("pageSize") === "20",
          ),
        ).toBe(true),
      );
    } finally {
      setDocumentVisibility("visible");
    }
  });

  it("updates the stats URL when request audit pagination changes", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "API details" }))[0]);
    expect(await screen.findByText("API details")).toBeInTheDocument();
    await waitFor(() =>
      expect(window.location.pathname + window.location.search).toBe(
        "/stats?page=1",
      ),
    );

    await userEvent.click(screen.getByRole("button", { name: "Go to page 2" }));

    await waitFor(() =>
      expect(window.location.pathname + window.location.search).toBe(
        "/stats?page=2",
      ),
    );
    await waitFor(() =>
      expect(
        aiStatisticsCallUrls().some((url) => url.searchParams.get("page") === "2"),
      ).toBe(true),
    );
  });

  it("updates request audit pagination when browser navigation changes stats page", async () => {
    window.history.replaceState(null, "", "/stats?page=1");

    renderApp();

    expect(await screen.findByText("API details")).toBeInTheDocument();
    await waitFor(() =>
      expect(
        aiStatisticsCallUrls().some((url) => url.searchParams.get("page") === "1"),
      ).toBe(true),
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Go to page 1" })).toHaveAttribute(
        "aria-current",
        "page",
      ),
    );

    await act(async () => {
      window.history.pushState(null, "", "/stats?page=3");
      fireEvent.popState(window);
    });

    await waitFor(() =>
      expect(
        aiStatisticsCallUrls().some((url) => url.searchParams.get("page") === "3"),
      ).toBe(true),
    );
    expect(screen.getByRole("button", { name: "Go to page 3" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("localizes running status in API request details", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/settings") {
        return Promise.resolve(jsonResponse({
          ...settings,
          general: {
            ...settings.general,
            language: "zh-CN",
          },
        }));
      }

      if (path === "/api/ai-statistics") {
        return Promise.resolve(jsonResponse({
          ...aiStatistics,
          requests: [
            {
              ...aiStatistics.requests[0],
              finalState: "running",
            },
          ],
        }));
      }

      if (path === "/api/workspaces/workspace-1/ai-statistics/request-1") {
        return Promise.resolve(jsonResponse({
          ...aiStatisticsDetail,
          request: {
            ...aiStatisticsDetail.request,
            finalState: "running",
          },
        }));
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "API 详情" }))[0]);
    const table = await screen.findByRole("table");
    expect(within(table).getByText("运行中")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "查看请求详情" }));

    const dialog = await screen.findByRole("dialog", { name: "请求详情" });
    expect(within(dialog).getByText("状态")).toBeInTheDocument();
    expect(within(dialog).getByText("运行中")).toBeInTheDocument();
  });

  it("loads saved API request audit column settings", async () => {
    const { unmount } = renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "API details" }))[0]);
    const table = await screen.findByRole("table");
    expect(within(table).getByText("openai")).toBeInTheDocument();

    await userEvent.click(screen.getByText("Columns"));
    await userEvent.click(screen.getByRole("checkbox", { name: "Provider" }));
    expect(within(table).queryByText("openai")).not.toBeInTheDocument();
    await waitFor(() => {
      const savedColumns = JSON.parse(
        window.localStorage.getItem("foco.aiStats.visibleColumns") ?? "[]",
      );
      expect(savedColumns).not.toContain("provider");
    });

    unmount();
    window.history.replaceState(null, "", "/");
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "API details" }))[0]);
    const reloadedTable = await screen.findByRole("table");
    expect(within(reloadedTable).queryByText("openai")).not.toBeInTheDocument();
    await userEvent.click(screen.getByText("Columns"));
    expect(screen.getByRole("checkbox", { name: "Provider" })).not.toBeChecked();
  });


  it("lazy loads workspace file tree children on demand", async () => {
    const fetchMock = vi.mocked(fetch);

    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));

    expect(await screen.findByText("Workspace file tree")).toBeInTheDocument();
    expect(screen.getByText("main.ts")).toBeInTheDocument();
    expect(screen.getByText("components")).toBeInTheDocument();
    expect(screen.getByText("pages")).toBeInTheDocument();

    expect(
      fetchMock.mock.calls.some((call) => String(call[0]).includes("/files/children")),
    ).toBe(false);

    const pagesRow = screen.getByText("pages").closest("div[role='treeitem']");
    expect(pagesRow).not.toBeNull();
    await userEvent.click(
      within(pagesRow as HTMLElement).getByRole("button", { name: "Expand folder" }),
    );

    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some((call) =>
          String(call[0]).includes("/files/children?path=src%2Fpages"),
        ),
      ).toBe(true),
    );
    expect(await screen.findByText("index.tsx")).toBeInTheDocument();
  });

  it("writes file tabs to the URL and restores them after refresh", async () => {
    const { unmount } = renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));
    await userEvent.click(screen.getByText("main.ts"));

    const tabList = await screen.findByRole("tablist", { name: "Chat" });
    expect(within(tabList).getByRole("tab", { name: /main.ts/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(new URLSearchParams(window.location.search).getAll("file")).toEqual([
      "workspace-1/src%2Fmain.ts",
    ]);
    expect(new URLSearchParams(window.location.search).get("activeFile")).toBe(
      "workspace-1/src%2Fmain.ts",
    );

    vi.mocked(fetch).mockClear();
    unmount();
    renderApp();

    const restoredTabList = await screen.findByRole("tablist", { name: "Chat" });
    await waitFor(() =>
      expect(within(restoredTabList).getByRole("tab", { name: /main.ts/ })).toHaveAttribute(
        "aria-selected",
        "true",
      ),
    );
    await waitFor(() =>
      expect(
        vi.mocked(fetch).mock.calls.some((call) => {
          const url = String(call[0]);
          const body = call[1]?.body;
          return (
            url.includes("/api/workspaces/workspace-1/files/content") &&
            typeof body === "string" &&
            body.includes("src/main.ts")
          );
        }),
      ).toBe(true),
    );
  });

  it("copies file tree context menu values", async () => {
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));

    const componentsRow = screen.getByText("components").closest("div[role='treeitem']");
    expect(componentsRow).not.toBeNull();
    await userEvent.click(
      within(componentsRow as HTMLElement).getByRole("button", { name: "Expand folder" }),
    );

    const fileRow = (await screen.findByText("button.tsx")).closest("div[role='treeitem']");
    expect(fileRow).not.toBeNull();

    fireEvent.contextMenu(fileRow as HTMLElement);
    const menu = await screen.findByRole("menu", { name: "button.tsx" });
    for (const item of [
      "Open",
      "Rename",
      "Delete",
      "Copy file name",
      "Copy relative path",
      "Copy absolute path",
    ]) {
      expect(within(menu).getByRole("menuitem", { name: item })).toBeInTheDocument();
    }

    await userEvent.click(within(menu).getByRole("menuitem", { name: "Copy file name" }));
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith("button.tsx");

    fireEvent.contextMenu(fileRow as HTMLElement);
    await userEvent.click(
      within(await screen.findByRole("menu", { name: "button.tsx" })).getByRole("menuitem", {
        name: "Copy relative path",
      }),
    );
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith("src/components/button.tsx");

    fireEvent.contextMenu(fileRow as HTMLElement);
    await userEvent.click(
      within(await screen.findByRole("menu", { name: "button.tsx" })).getByRole("menuitem", {
        name: "Copy absolute path",
      }),
    );
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(
      `${workspace.path}\\src\\components\\button.tsx`,
    );
  });

  it("toggles markdown file preview from the editor toolbar", async () => {
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));
    await userEvent.click(await screen.findByText("README.md"));

    const previewButton = await screen.findByRole("button", {
      name: "Preview markdown",
    });
    expect(previewButton).not.toHaveAttribute("aria-pressed");
    expect(previewButton.querySelector(".lucide-eye")).toBeInTheDocument();
    expect(previewButton.querySelector(".lucide-eye-off")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Preview title" }),
    ).not.toBeInTheDocument();

    await userEvent.click(previewButton);

    expect(
      await screen.findByRole("heading", { name: "Preview title" }),
    ).toBeInTheDocument();
    const editButton = screen.getByRole("button", { name: "Edit markdown" });
    expect(editButton).toHaveAttribute("aria-pressed", "true");
    expect(editButton.querySelector(".lucide-eye-off")).toBeInTheDocument();
    expect(editButton.querySelector(".lucide-eye")).not.toBeInTheDocument();
    expect(
      screen.queryByText(/<\/?div/i),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Foco" })).toHaveAttribute(
      "src",
      "/api/workspaces/workspace-1/files/blob?path=foco.svg",
    );
    expect(screen.getByRole("img", { name: "Foco" })).toHaveAttribute(
      "width",
      "96",
    );
    expect(screen.getByRole("img", { name: "Remote asset" })).toHaveAttribute(
      "src",
      "https://example.com/asset.png",
    );
    expect(
      screen.getByRole("img", { name: "Inline asset" }).getAttribute("src"),
    ).toMatch(/^data:image\/png;base64,/);
    expect(document.querySelector(".katex")).not.toBeNull();
    expect(await screen.findByTestId("mermaid-svg")).toBeInTheDocument();
    expect(mermaidMock.render).toHaveBeenCalledWith(
      expect.stringMatching(/^foco-mermaid-/),
      "flowchart TD\n  A --> B",
    );

    await userEvent.click(editButton);
    expect(
      screen.queryByRole("heading", { name: "Preview title" }),
    ).not.toBeInTheDocument();
  });
  it("reloads the active file from the leftmost editor toolbar button", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await screen.findAllByText("Default");
    await userEvent.click(screen.getByRole("tab", { name: "Files" }));
    await userEvent.click(await screen.findByText("README.md"));

    const toolbar = await screen.findByRole("toolbar", { name: "Editor toolbar" });
    const toolbarButtons = within(toolbar).getAllByRole("button");
    expect(toolbarButtons[0]).toHaveAccessibleName("Reload file");

    const contentRequestCount = fetchMock.mock.calls.filter(
      ([url]) => url === "/api/workspaces/workspace-1/files/content",
    ).length;

    await userEvent.click(toolbarButtons[0]);

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.filter(
          ([url]) => url === "/api/workspaces/workspace-1/files/content",
        ),
      ).toHaveLength(contentRequestCount + 1);
    });
  });

});
