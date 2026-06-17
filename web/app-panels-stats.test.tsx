import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  activeMemory,
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
  workspaceMemory,
} from "./test-utils/app-test-harness";

describe("app-panels-stats verification surfaces", () => {
  beforeEach(resetAppTestEnvironment);

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
    renderApp();

    await userEvent.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "plan");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        chatId: "chat-1",
        type: "todoGraphRefresh",
        workspaceId: "workspace-1",
      });
    });

    const todoTaskButton = await screen.findByRole("button", {
      name: /task-1[\s\S]*Inspect workspace changes/,
    });
    expect(todoTaskButton).toBeInTheDocument();
    await userEvent.click(todoTaskButton);
    expect(await screen.findByText("README.md diff is visible")).toBeInTheDocument();
    expect(screen.queryByText(/hello world/)).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("tab", { name: "Git" }));

    expect(screen.getByText("Source Control")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /README\.md M/ })).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: /new-note\.txt U/ })).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: /asset\.bin M/ }).length).toBeGreaterThan(0);
    expect(screen.queryByText(/hello world/)).not.toBeInTheDocument();

    await userEvent.click(screen.getAllByRole("button", { name: /README\.md M/ })[0]);

    const inlineDiffLine = (await screen.findAllByText(/hello world/))[0];
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
    expect(screen.queryByText("Inspect workspace changes")).not.toBeInTheDocument();

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
    expect(screen.getAllByText("History").length).toBeGreaterThan(0);
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
    renderApp();

    await userEvent.type(await screen.findByPlaceholderText(defaultComposerPlaceholder), "plan");
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(appTestState.activeChatStreamController).not.toBeNull());

    await act(async () => {
      enqueueChatStreamEvent({
        chatId: "chat-1",
        type: "todoGraphRefresh",
        workspaceId: "workspace-1",
      });
    });

    expect(await screen.findByText("ToDo graph")).toBeInTheDocument();
    expect(screen.getByText("Inspect workspace changes")).toBeInTheDocument();
    expect(screen.queryByText("Git diff")).not.toBeInTheDocument();

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
      enqueueChatStreamEvent({
        chatId: "chat-1",
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
    expect(within(table).getByText("openai")).toBeInTheDocument();
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

});
